// Drawing on one or more Rainbowduino boards driven by the slave firmware (slave/), over I2C.

#![no_std]

mod board_frame;

pub use board_frame::{BoardFrame, Rgb, BOARD_SIZE, FRAME_BYTES};
