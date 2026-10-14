#![no_std]
#![no_main]

mod framebuffer;

use framebuffer::{Framebuffer, Rgb};
use panic_halt as _;

// Maps an index to one of the 7 non-black on/off R/G/B combinations.
fn color_from_index(idx: u8) -> Rgb {
    let rgb_bits = idx % 7 + 1; // 1..7

    // rgb_bits' 3 low bits map directly to R, G, B on/off.
    Rgb {
        r: rgb_bits & 1 != 0,
        g: rgb_bits & 2 != 0,
        b: rgb_bits & 4 != 0,
    }
}

#[arduino_hal::entry]
fn main() -> ! {
    let peripherals = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(peripherals);

    let mut i2c = arduino_hal::I2c::new(
        peripherals.TWI,
        // SDA
        pins.a4.into_pull_up_input(),
        // SDL
        pins.a5.into_pull_up_input(),
        // 50 kHz
        50_000,
    );

    let mut fb: Framebuffer<4> = Framebuffer::new();

    loop {
        // Cycle through the 7 colors.
        for color_idx in 0..7u8 {
            // Addresses of the boards, from left to right.
            const ADDRS: [u8; 4] = [0x08, 0x09, 0x0A, 0x0B];

            // One board's worth of columns per address.
            const TOTAL_COLS: u8 = ADDRS.len() as u8 * 8;

            // 7 colors but 8 columns/rows: the color band doesn't line up with the grid, so each
            // diagonal gets a different color, producing a rainbow pattern.
            for col in 0..TOTAL_COLS {
                for row in 0..8u8 {
                    fb.set_pixel(
                        col as i32,
                        row as i32,
                        color_from_index(col + row + color_idx),
                    );
                }
            }

            let _ = fb.send(&mut i2c, &ADDRS);

            arduino_hal::delay_ms(100);
        }
    }
}
