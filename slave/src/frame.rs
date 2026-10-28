// 3 color bytes per line (0-7) back to back, always in this order: Green, Red, Blue.
//
// Each byte packs 2 columns' brightness as 4-bit nibbles, in the reverse order of the columns on
// the physical board: byte 0 holds columns 0-1 (high nibble = column 0), ..., byte 3 holds columns
// 6-7 (high nibble = column 6).

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

// The frame shown, the last one fully received and the one being received. Swaps exchange indices
// for performance reasons.
pub struct Frames {
    buffers: [Frame; 3],
    front: usize,
    ready: usize,
    back: usize,
    fresh: bool,
}

impl Frames {
    pub const fn new() -> Self {
        Self {
            buffers: [[0; FRAME_BYTES]; 3],
            front: 0,
            ready: 1,
            back: 2,
            fresh: false,
        }
    }

    pub fn front(&self) -> &Frame {
        &self.buffers[self.front]
    }

    // Stores a byte of the frame being received.
    pub fn write_back(&mut self, index: usize, byte: u8) {
        self.buffers[self.back][index] = byte;
    }

    // The back frame is complete. Replaces a ready one not shown yet: the newest wins.
    pub fn publish(&mut self) {
        core::mem::swap(&mut self.back, &mut self.ready);
        self.fresh = true;
    }

    // Shows the last published frame, if any.
    pub fn show_latest(&mut self) {
        if self.fresh {
            core::mem::swap(&mut self.front, &mut self.ready);
            self.fresh = false;
        }
    }
}
