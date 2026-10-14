// A framebuffer for one or more Rainbowduino boards chained left to right on a shared I2C bus (see
// slave/src/main.rs for the board-side protocol and wiring this matches).

use embedded_hal::i2c::I2c;

// Number of columns on a single Rainbowduino board.
const BOARD_COLS: i32 = 8;
// Number of rows on a single Rainbowduino board.
const BOARD_ROWS: i32 = 8;
// Raw I2C frame size the Rainbowduino firmware expects: 8 rows x 3 color bytes.
const FRAME_BYTES: usize = 24;

// Sets or clears bit `bit_pos` of `byte` depending on `value`.
fn set_bit(byte: &mut u8, bit_pos: i32, value: bool) {
    if value {
        *byte |= 1 << bit_pos;
    } else {
        *byte &= !(1 << bit_pos);
    }
}

// Whether each of a pixel's three color channels is on.
pub struct Rgb {
    pub r: bool,
    pub g: bool,
    pub b: bool,
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

    // Toggle one pixel's red/green/blue channels independently.
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
        // see slave/src/main.rs: bit order is reversed
        let bit_pos = 7 - local_col;
        let local_row = row as usize;

        // channel order: Green, Red, Blue
        set_bit(&mut board[local_row * 3], bit_pos, color.g);
        set_bit(&mut board[local_row * 3 + 1], bit_pos, color.r);
        set_bit(&mut board[local_row * 3 + 2], bit_pos, color.b);
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
