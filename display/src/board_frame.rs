// One board's frame, encoded the way the slave firmware expects it over I2C (see
// slave/src/main.rs).

// Columns and rows of one board.
pub const BOARD_SIZE: usize = 8;
// Bytes per line: 3 colors x 4 bytes (8 columns, 2 per byte, 4-bit brightness each).
const LINE_BYTES: usize = 12;
// Raw I2C frame size: 8 lines x LINE_BYTES.
pub const FRAME_BYTES: usize = LINE_BYTES * BOARD_SIZE;

// Per-pixel brightness (0-15) for each color channel. Higher values are masked to 4 bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

// Sets one 4-bit nibble of `byte` (high or low half) to `value`, masked to 4 bits.
fn set_nibble(byte: &mut u8, high: bool, value: u8) {
    let value = value & 0x0F;
    if high {
        *byte = (*byte & 0x0F) | (value << 4);
    } else {
        *byte = (*byte & 0xF0) | value;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardFrame {
    bytes: [u8; FRAME_BYTES],
}

impl BoardFrame {
    pub const fn new() -> Self {
        Self {
            bytes: [0; FRAME_BYTES],
        }
    }

    // Turns every pixel off.
    pub fn clear(&mut self) {
        self.bytes = [0; FRAME_BYTES];
    }

    // Sets one pixel, in the board's own coordinates: `col` and `row` 0-7, (0, 0) at the corner
    // the firmware calls column 0, line 0.
    pub fn set_pixel(&mut self, col: usize, row: usize, color: Rgb) {
        // 2 columns per byte, even column in the high nibble.
        let byte_in_line = col / 2;
        let high = col % 2 == 0;
        let line_start = row * LINE_BYTES;

        // Color order: Green, Red, Blue.
        set_nibble(&mut self.bytes[line_start + byte_in_line], high, color.g);
        set_nibble(
            &mut self.bytes[line_start + 4 + byte_in_line],
            high,
            color.r,
        );
        set_nibble(
            &mut self.bytes[line_start + 8 + byte_in_line],
            high,
            color.b,
        );
    }

    // The raw I2C frame.
    pub fn bytes(&self) -> &[u8; FRAME_BYTES] {
        &self.bytes
    }
}

impl Default for BoardFrame {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn even_column_goes_in_high_nibble_green_red_blue() {
        let mut frame = BoardFrame::new();
        frame.set_pixel(0, 0, Rgb { r: 1, g: 2, b: 3 });
        let mut expected = [0; FRAME_BYTES];
        expected[0] = 0x20; // green
        expected[4] = 0x10; // red
        expected[8] = 0x30; // blue

        assert_eq!(frame.bytes(), &expected);
    }

    #[test]
    fn odd_column_goes_in_low_nibble() {
        let mut frame = BoardFrame::new();
        frame.set_pixel(7, 0, Rgb { r: 1, g: 2, b: 3 });
        let mut expected = [0; FRAME_BYTES];
        expected[3] = 0x02;
        expected[7] = 0x01;
        expected[11] = 0x03;

        assert_eq!(frame.bytes(), &expected);
    }

    #[test]
    fn rows_are_line_bytes_apart() {
        let mut frame = BoardFrame::new();
        frame.set_pixel(0, 7, Rgb { r: 1, g: 2, b: 3 });

        assert_eq!(frame.bytes()[7 * LINE_BYTES], 0x20);
        assert_eq!(frame.bytes()[7 * LINE_BYTES + 8], 0x30);
    }

    #[test]
    fn neighbour_in_same_byte_is_kept() {
        let mut frame = BoardFrame::new();
        frame.set_pixel(0, 0, Rgb { r: 1, g: 2, b: 3 });
        frame.set_pixel(1, 0, Rgb { r: 4, g: 5, b: 6 });

        assert_eq!(
            &frame.bytes()[..12],
            &[0x25, 0, 0, 0, 0x14, 0, 0, 0, 0x36, 0, 0, 0]
        );
    }
}
