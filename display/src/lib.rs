// Drawing on one or more Rainbowduino boards driven by the slave firmware (slave/), over I2C.
//
// Describe how the boards are arranged with a `Layout`, draw on a `Canvas` as on a single display,
// and `send` it: each board gets its part, in its orientation.

#![no_std]

mod board_frame;
mod canvas;
mod layout;

pub use board_frame::{BoardFrame, Rgb, BOARD_SIZE, FRAME_BYTES};
pub use canvas::Canvas;
pub use layout::{Board, Layout, Rotation};
