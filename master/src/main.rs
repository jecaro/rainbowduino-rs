#![no_std]
#![no_main]

mod framebuffer;

use framebuffer::{Framebuffer, Rgb};
use panic_halt as _;

// Classic smooth color wheel (0-255 hue -> RGB, red-green-blue-red), scaled down from 8-bit to our
// 4-bit (0-15) per-channel brightness.
fn wheel(pos: u8) -> Rgb {
    let (r, g, b) = if pos < 85 {
        // red -> green
        (255 - pos * 3, pos * 3, 0)
    } else if pos < 170 {
        // green -> blue
        let pos = pos - 85;
        (0, 255 - pos * 3, pos * 3)
    } else {
        // blue -> red
        let pos = pos - 170;
        (pos * 3, 0, 255 - pos * 3)
    };
    Rgb {
        r: r >> 4,
        g: g >> 4,
        b: b >> 4,
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
        // 400 kHz
        400_000,
    );

    let mut fb: Framebuffer<4> = Framebuffer::new();

    // Addresses of the boards, from left to right.
    const ADDRS: [u8; 4] = [0x08, 0x09, 0x0A, 0x0B];
    const TOTAL_COLS: i32 = 4 * 8; // 4 boards x 8 columns

    let mut phase: u8 = 0;

    loop {
        // Diagonal rainbow, one full hue cycle across the display width, drifting over time.
        for col in 0..TOTAL_COLS {
            for row in 0..8 {
                let hue = (col as u8)
                    .wrapping_mul(8)
                    .wrapping_add(row as u8 * 4)
                    .wrapping_add(phase);
                fb.set_pixel(col, row, wheel(hue));
            }
        }

        let _ = fb.send(&mut i2c, &ADDRS);

        phase = phase.wrapping_add(4);
    }
}
