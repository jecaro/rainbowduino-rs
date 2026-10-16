use avr_device::interrupt::{CriticalSection, Mutex};
use core::cell::{Cell, RefCell};

// Bytes per line: 3 colors x 4 bytes (8 columns, 2 per byte, 4-bit brightness each).
pub const LINE_BYTES: usize = 12;
// 8 lines per frame.
pub const FRAME_BYTES: usize = LINE_BYTES * 8;

pub type Frame = [u8; FRAME_BYTES];
pub type Line = [u8; LINE_BYTES];

// Returns the line bytes (0-7) of `buf`.
pub fn line_bytes(buf: &Frame, line: u8) -> &Line {
    let start = line as usize * LINE_BYTES;
    (&buf[start..][..LINE_BYTES]).try_into().unwrap()
}

pub struct DoubleBuffer {
    buffer_a: Mutex<RefCell<Frame>>,
    buffer_b: Mutex<RefCell<Frame>>,
    front_is_a: Mutex<Cell<bool>>,
}

impl DoubleBuffer {
    pub const fn new() -> Self {
        Self {
            buffer_a: Mutex::new(RefCell::new([0; FRAME_BYTES])),
            buffer_b: Mutex::new(RefCell::new([0; FRAME_BYTES])),
            front_is_a: Mutex::new(Cell::new(true)),
        }
    }

    fn front<'cs>(&'cs self, cs: CriticalSection<'cs>) -> &'cs RefCell<Frame> {
        if self.front_is_a.borrow(cs).get() {
            self.buffer_a.borrow(cs)
        } else {
            self.buffer_b.borrow(cs)
        }
    }

    fn back<'cs>(&'cs self, cs: CriticalSection<'cs>) -> &'cs RefCell<Frame> {
        if self.front_is_a.borrow(cs).get() {
            self.buffer_b.borrow(cs)
        } else {
            self.buffer_a.borrow(cs)
        }
    }

    // Writes one byte into the back buffer at `index`.
    pub fn write_byte(&self, index: usize, byte: u8) {
        avr_device::interrupt::free(|cs| {
            self.back(cs).borrow_mut()[index] = byte;
        });
    }

    // Flips front/back: the back buffer becomes the one shown next.
    pub fn publish(&self) {
        avr_device::interrupt::free(|cs| {
            let front_is_a = self.front_is_a.borrow(cs);
            front_is_a.set(!front_is_a.get());
        });
    }

    // Copies the current front buffer into `out`.
    pub fn snapshot(&self, out: &mut Frame) {
        avr_device::interrupt::free(|cs| {
            *out = *self.front(cs).borrow();
        });
    }
}
