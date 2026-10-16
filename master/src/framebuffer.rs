// A framebuffer for one or more Rainbowduino boards chained left to right on a shared I2C bus (see
// slave/src/main.rs for the board-side protocol and wiring this matches).

use embedded_hal::i2c::I2c;

// Number of columns on a single Rainbowduino board.
const BOARD_COLS: i32 = 8;
// Number of rows on a single Rainbowduino board.
const BOARD_ROWS: i32 = 8;
// Bytes per line: 3 colors x 4 bytes (8 columns, 2 per byte, 4-bit brightness each).
const LINE_BYTES: usize = 12;
// Raw I2C frame size the Rainbowduino firmware expects: 8 rows x LINE_BYTES.
const FRAME_BYTES: usize = LINE_BYTES * BOARD_ROWS as usize;

// Sets one 4-bit nibble of `byte` (high or low half) to `value`, masked to 4 bits.
fn set_nibble(byte: &mut u8, high: bool, value: u8) {
    let value = value & 0x0F;
    if high {
        *byte = (*byte & 0x0F) | (value << 4);
    } else {
        *byte = (*byte & 0xF0) | value;
    }
}

// Per-pixel brightness (0-15) for each color channel.
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

// A framebuffer spanning `N` Rainbowduino boards chained left to right, giving a combined display
// of `N * 8` columns by 8 rows.
pub struct Framebuffer<const N: usize> {
    boards: [[u8; FRAME_BYTES]; N],
}

impl<const N: usize> Framebuffer<N> {
    pub const fn new() -> Self {
        Self {
            boards: [[0; FRAME_BYTES]; N],
        }
    }

    // Sets one pixel's red/green/blue brightness (0-15 each) independently.
    //
    // `col` is the column across the whole chain (0 = leftmost column of the first board), not a
    // per-board column. `col` or `row` outside the visible area are silently ignored, which makes
    // it easy to draw sprites that are partially off-screen.
    pub fn set_pixel(&mut self, col: i32, row: i32, color: Rgb) {
        if row < 0 || row >= BOARD_ROWS {
            return;
        }

        let board_index = col.div_euclid(BOARD_COLS);
        if board_index < 0 || board_index as usize >= N {
            return;
        }
        let board = &mut self.boards[board_index as usize];

        let local_col = col.rem_euclid(BOARD_COLS);
        // see slave/src/main.rs: 2 columns per byte, even column in the high nibble
        let byte_in_line = (local_col / 2) as usize;
        let high = local_col % 2 == 0;
        let line_start = row as usize * LINE_BYTES;

        // channel order: Green, Red, Blue
        set_nibble(&mut board[line_start + byte_in_line], high, color.g);
        set_nibble(&mut board[line_start + 4 + byte_in_line], high, color.r);
        set_nibble(&mut board[line_start + 8 + byte_in_line], high, color.b);
    }

    // Push the current contents to all boards over I2C, left to right. `addrs[i]` is the 7-bit I2C
    // address of board `i`.
    pub fn send<I: I2c>(&self, i2c: &mut I, addrs: &[u8; N]) -> Result<(), I::Error> {
        for (addr, board) in addrs.iter().zip(self.boards.iter()) {
            i2c.write(*addr, board)?;
        }
        Ok(())
    }
}
