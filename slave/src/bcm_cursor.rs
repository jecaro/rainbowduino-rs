// Number of Binary Code Modulation levels: brightness is per-pixel, per-channel, 4-bit (0-15),
// built up by redrawing each line once per level and lighting a column only if its value exceeds
// the current level.
pub const LEVELS: u8 = 16;

// Position in the BCM cycle: which line (0-7) and which brightness level (0-15) is live.
pub struct BcmCursor {
    pub line: u8,
    pub level: u8,
}

impl BcmCursor {
    pub const fn new() -> Self {
        Self { line: 0, level: 0 }
    }

    // True at the start of a full cycle (all lines x all levels).
    pub fn is_cycle_start(&self) -> bool {
        self.line == 0 && self.level == 0
    }

    // Moves to the next line, wrapping to the next level after the last line.
    pub fn advance(&mut self) {
        self.line += 1;
        if self.line >= 8 {
            self.line = 0;
            self.level = (self.level + 1) % LEVELS;
        }
    }
}
