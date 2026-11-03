#![no_std]
#![no_main]

// Plasma on a 16x16 square of 4 boards: waves along x, y and the diagonal, each bouncing back and
// forth, are summed and mapped to the color wheel.

use panic_halt as _;
use rainbowduino_display::{Board, Canvas, Layout, Rgb, Rotation};

// The boards, as seen from the front: the bottom two upside down, to keep the cables short.
const LAYOUT: Layout<2, 2> = Layout {
    boards: [
        [
            Board {
                addr: 0x0A,
                rotation: Rotation::R0,
            },
            Board {
                addr: 0x0B,
                rotation: Rotation::R0,
            },
        ],
        [
            Board {
                addr: 0x09,
                rotation: Rotation::R180,
            },
            Board {
                addr: 0x08,
                rotation: Rotation::R180,
            },
        ],
    ],
};

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

// Triangle wave, 0-254: a cheap stand-in for sin() (no float, no table in RAM).
fn tri(x: u8) -> u8 {
    if x < 128 {
        x << 1
    } else {
        (255 - x) << 1
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
        // SCL
        pins.a5.into_pull_up_input(),
        // 400 kHz
        400_000,
    );

    let mut canvas = Canvas::new(LAYOUT);

    // Paces the animation at 40 fps: Timer1 in CTC mode raises OCF1A every 25ms. Drawing and
    // sending a frame takes up to 20ms (16ms of I2C for the 4 boards).
    // 16MHz / 1024 = 15625 ticks/s.
    let tc1 = peripherals.TC1;
    tc1.tccr1b()
        .write(|w| w.wgm1().set(0b01).cs1().prescale_1024());
    tc1.ocr1a().write(|w| unsafe { w.bits(390) });

    let mut t: u16 = 0;

    loop {
        // Each wave bounces back and forth: its offset follows a triangle wave of time, each at
        // its own pace.
        let horizontal = tri(t as u8);
        let vertical = tri((t.wrapping_mul(3) / 2) as u8);
        let diagonal = tri((t / 2) as u8);

        for y in 0..canvas.height() as u8 {
            for x in 0..canvas.width() as u8 {
                // About one wave across the square in each direction.
                let v = tri(x.wrapping_mul(16).wrapping_add(horizontal)) as u16
                    + tri(y.wrapping_mul(14).wrapping_add(vertical)) as u16
                    + tri((x + y).wrapping_mul(7).wrapping_add(diagonal)) as u16;
                canvas.set_pixel(x as i32, y as i32, wheel((v / 3) as u8));
            }
        }

        let _ = canvas.send(&mut i2c);

        t = t.wrapping_add(1);

        while tc1.tifr1().read().ocf1a().bit_is_clear() {}
        // Writing 1 clears the flag.
        tc1.tifr1().write(|w| w.ocf1a().set_bit());
    }
}
