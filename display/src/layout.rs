// How the boards are physically arranged: a grid of boards, each with its I2C address and how it is
// rotated.

use crate::BOARD_SIZE;

// How a board is mounted, clockwise, relative to its normal orientation: the one drawn in
// slave/src/main.rs, where its column 0, line 0 is the top-left pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    R0,
    R90,
    R180,
    R270,
}

impl Rotation {
    // Maps a pixel position on the board as seen from the front (`x` to the right, `y` down, 0-7)
    // to the board's own column and row.
    pub(crate) fn to_board(self, x: usize, y: usize) -> (usize, usize) {
        const MAX: usize = BOARD_SIZE - 1;
        match self {
            Rotation::R0 => (x, y),
            // Its top row ends up as the rightmost column.
            Rotation::R90 => (y, MAX - x),
            Rotation::R180 => (MAX - x, MAX - y),
            // Its top row ends up as the leftmost column.
            Rotation::R270 => (MAX - y, x),
        }
    }
}

// One board in the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Board {
    pub addr: u8,
    pub rotation: Rotation,
}

// `ROWS` rows of `COLS` boards, as seen from the front: `boards[0][0]` is the top-left board.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout<const COLS: usize, const ROWS: usize> {
    pub boards: [[Board; COLS]; ROWS],
}

#[cfg(test)]
mod tests {
    use super::*;

    // The board's own coordinates of the 4 corners seen from the front, in the order top-left,
    // top-right, bottom-left, bottom-right.
    fn corners(rotation: Rotation) -> [(usize, usize); 4] {
        [(0, 0), (7, 0), (0, 7), (7, 7)].map(|(x, y)| rotation.to_board(x, y))
    }

    #[test]
    fn r90_moves_board_top_left_to_top_right() {
        assert_eq!(corners(Rotation::R90), [(0, 7), (0, 0), (7, 7), (7, 0)]);
    }

    #[test]
    fn r180_swaps_opposite_corners() {
        assert_eq!(corners(Rotation::R180), [(7, 7), (0, 7), (7, 0), (0, 0)]);
    }

    #[test]
    fn r270_moves_board_top_left_to_bottom_left() {
        assert_eq!(corners(Rotation::R270), [(7, 0), (7, 7), (0, 0), (0, 7)]);
    }
}
