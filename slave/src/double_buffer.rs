use avr_device::interrupt::{CriticalSection, Mutex};
use core::cell::{Cell, RefCell};

pub struct DoubleBuffer {
    buffer_a: Mutex<RefCell<[u8; 24]>>,
    buffer_b: Mutex<RefCell<[u8; 24]>>,
    front_is_a: Mutex<Cell<bool>>,
}

impl DoubleBuffer {
    pub const fn new() -> Self {
        Self {
            buffer_a: Mutex::new(RefCell::new([0; 24])),
            buffer_b: Mutex::new(RefCell::new([0; 24])),
            front_is_a: Mutex::new(Cell::new(true)),
        }
    }

    fn front<'cs>(&'cs self, cs: CriticalSection<'cs>) -> &'cs RefCell<[u8; 24]> {
        if self.front_is_a.borrow(cs).get() {
            self.buffer_a.borrow(cs)
        } else {
            self.buffer_b.borrow(cs)
        }
    }

    fn back<'cs>(&'cs self, cs: CriticalSection<'cs>) -> &'cs RefCell<[u8; 24]> {
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
    pub fn snapshot(&self, out: &mut [u8; 24]) {
        avr_device::interrupt::free(|cs| {
            *out = *self.front(cs).borrow();
        });
    }
}
