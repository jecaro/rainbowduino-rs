// One image spanning all the boards of a layout: draw on it as a single display, it splits it into
// per-board frames, each in its board's orientation.

use embedded_hal::i2c::I2c;

use crate::{Board, BoardFrame, Layout, Rgb, BOARD_SIZE};

// One tile of the canvas: its board and what it shows.
struct Tile {
    board: Board,
    frame: BoardFrame,
}

impl Tile {
    // `x`, `y`: position on the board as seen from the front.
    fn set_pixel(&mut self, x: usize, y: usize, color: Rgb) {
        let (col, row) = self.board.rotation.to_board(x, y);
        self.frame.set_pixel(col, row, color);
    }

    fn clear(&mut self) {
        self.frame.clear();
    }

    fn send<I: I2c>(&self, i2c: &mut I) -> Result<(), I::Error> {
        i2c.write(self.board.addr, self.frame.bytes())
    }
}

pub struct Canvas<const COLS: usize, const ROWS: usize> {
    tiles: [[Tile; COLS]; ROWS],
}

impl<const COLS: usize, const ROWS: usize> Canvas<COLS, ROWS> {
    pub fn new(layout: Layout<COLS, ROWS>) -> Self {
        Self {
            tiles: layout.boards.map(|row| {
                row.map(|board| Tile {
                    board,
                    frame: BoardFrame::new(),
                })
            }),
        }
    }

    pub const fn width(&self) -> usize {
        COLS * BOARD_SIZE
    }

    pub const fn height(&self) -> usize {
        ROWS * BOARD_SIZE
    }

    // Turns every pixel off.
    pub fn clear(&mut self) {
        for tile in self.tiles.iter_mut().flatten() {
            tile.clear();
        }
    }

    // Sets one pixel, (0, 0) being the top-left of the whole display as seen from the front.
    // Pixels outside the display are ignored, which makes it easy to draw sprites that are
    // partially off-screen.
    pub fn set_pixel(&mut self, x: i32, y: i32, color: Rgb) {
        if x < 0 || y < 0 || x as usize >= self.width() || y as usize >= self.height() {
            return;
        }
        let (x, y) = (x as usize, y as usize);

        let tile = &mut self.tiles[y / BOARD_SIZE][x / BOARD_SIZE];
        tile.set_pixel(x % BOARD_SIZE, y % BOARD_SIZE, color);
    }

    // Sends each board its frame, row by row from the top-left board.
    pub fn send<I: I2c>(&self, i2c: &mut I) -> Result<(), I::Error> {
        for tile in self.tiles.iter().flatten() {
            tile.send(i2c)?;
        }
        Ok(())
    }

    #[cfg(test)]
    fn frame(&self, tile_col: usize, tile_row: usize) -> &BoardFrame {
        &self.tiles[tile_row][tile_col].frame
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use embedded_hal::i2c::{ErrorType, Operation};

    use super::*;
    use crate::{Board, Rotation};

    const RGB: Rgb = Rgb { r: 1, g: 2, b: 3 };

    fn normal(addr: u8) -> Board {
        Board {
            addr,
            rotation: Rotation::R0,
        }
    }

    fn upside_down(addr: u8) -> Board {
        Board {
            addr,
            rotation: Rotation::R180,
        }
    }

    // The frame a board should receive with only (`col`, `row`) of its own coordinates lit.
    fn only(col: usize, row: usize) -> BoardFrame {
        let mut frame = BoardFrame::new();
        frame.set_pixel(col, row, RGB);
        frame
    }

    #[test]
    fn row_of_boards_splits_by_column() {
        let mut canvas = Canvas::new(Layout {
            boards: [[normal(8), normal(9), normal(10), normal(11)]],
        });
        canvas.set_pixel(9, 2, RGB);
        assert_eq!(canvas.frame(1, 0), &only(1, 2));
        for other in [0, 2, 3] {
            assert_eq!(canvas.frame(other, 0), &BoardFrame::new());
        }
    }

    #[test]
    fn rotated_board_gets_rotated_coordinates() {
        // A 2x2 square: 3 4 upside down on top, 2 1 the right way below.
        let mut canvas = Canvas::new(Layout {
            boards: [[upside_down(10), upside_down(11)], [normal(9), normal(8)]],
        });
        canvas.set_pixel(0, 0, RGB);
        assert_eq!(canvas.frame(0, 0), &only(7, 7));
        canvas.set_pixel(15, 15, RGB);
        assert_eq!(canvas.frame(1, 1), &only(7, 7));
        canvas.set_pixel(8, 7, RGB);
        assert_eq!(canvas.frame(1, 0), &only(7, 0));
    }

    #[test]
    fn outside_pixels_are_ignored() {
        let mut canvas = Canvas::new(Layout {
            boards: [[normal(8), normal(9)]],
        });
        for (x, y) in [(-1, 0), (0, -1), (16, 0), (0, 8)] {
            canvas.set_pixel(x, y, RGB);
        }
        assert_eq!(canvas.frame(0, 0), &BoardFrame::new());
        assert_eq!(canvas.frame(1, 0), &BoardFrame::new());
    }

    // Records every write as (address, bytes).
    #[derive(Default)]
    struct RecordingI2c {
        writes: Vec<(u8, Vec<u8>)>,
    }

    impl ErrorType for RecordingI2c {
        type Error = core::convert::Infallible;
    }

    impl I2c for RecordingI2c {
        fn transaction(&mut self, addr: u8, ops: &mut [Operation<'_>]) -> Result<(), Self::Error> {
            for op in ops {
                if let Operation::Write(bytes) = op {
                    self.writes.push((addr, bytes.to_vec()));
                }
            }
            Ok(())
        }
    }

    #[test]
    fn send_writes_each_frame_to_its_address_from_top_left() {
        let mut canvas = Canvas::new(Layout {
            boards: [[normal(8), normal(9), normal(10), normal(11)]],
        });
        // A different pixel on each board, so that a frame sent to the wrong board shows.
        canvas.set_pixel(0, 0, RGB); // board 8: its (0, 0)
        canvas.set_pixel(9, 0, RGB); // board 9: its (1, 0)
        canvas.set_pixel(18, 0, RGB); // board 10: its (2, 0)
        canvas.set_pixel(27, 0, RGB); // board 11: its (3, 0)

        let mut i2c = RecordingI2c::default();
        canvas.send(&mut i2c).unwrap();

        let addrs: Vec<u8> = i2c.writes.iter().map(|(addr, _)| *addr).collect();
        assert_eq!(addrs, [8, 9, 10, 11]);
        assert_eq!(i2c.writes[0].1, only(0, 0).bytes());
        assert_eq!(i2c.writes[1].1, only(1, 0).bytes());
        assert_eq!(i2c.writes[2].1, only(2, 0).bytes());
        assert_eq!(i2c.writes[3].1, only(3, 0).bytes());
    }
}
