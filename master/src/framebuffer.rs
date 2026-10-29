// A framebuffer for one or more Rainbowduino boards chained left to right on a shared I2C bus (see
// slave/src/main.rs for the board-side protocol and wiring this matches).

use embedded_hal::i2c::I2c;
use rainbowduino_display::{BoardFrame, BOARD_SIZE};

pub use rainbowduino_display::Rgb;

const BOARD_COLS: i32 = BOARD_SIZE as i32;
const BOARD_ROWS: i32 = BOARD_SIZE as i32;

// A framebuffer spanning `N` Rainbowduino boards chained left to right, giving a combined display
// of `N * 8` columns by 8 rows.
pub struct Framebuffer<const N: usize> {
    boards: [BoardFrame; N],
}

impl<const N: usize> Framebuffer<N> {
    pub const fn new() -> Self {
        Self {
            boards: [const { BoardFrame::new() }; N],
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

        let local_col = col.rem_euclid(BOARD_COLS);
        self.boards[board_index as usize].set_pixel(local_col as usize, row as usize, color);
    }

    // Push the current contents to all boards over I2C, left to right. `addrs[i]` is the 7-bit I2C
    // address of board `i`.
    pub fn send<I: I2c>(&self, i2c: &mut I, addrs: &[u8; N]) -> Result<(), I::Error> {
        for (addr, board) in addrs.iter().zip(self.boards.iter()) {
            i2c.write(*addr, board.bytes())?;
        }
        Ok(())
    }
}
