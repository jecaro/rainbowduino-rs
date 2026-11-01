#![no_std]
#![no_main]

use panic_halt as _;
use rainbowduino_display::{Board, Canvas, Layout, Rgb, Rotation};

// The boards, as seen from the front: 4 in a row, the right way up.
const LAYOUT: Layout<4, 1> = Layout {
    boards: [[
        Board {
            addr: 0x08,
            rotation: Rotation::R0,
        },
        Board {
            addr: 0x09,
            rotation: Rotation::R0,
        },
        Board {
            addr: 0x0A,
            rotation: Rotation::R0,
        },
        Board {
            addr: 0x0B,
            rotation: Rotation::R0,
        },
    ]],
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

    // Paces the animation: Timer1 in CTC mode raises OCF1A every 20ms (50 fps), whatever time
    // drawing and I2C take. 16MHz / 1024 = 15625 ticks/s.
    let tc1 = peripherals.TC1;
    tc1.tccr1b()
        .write(|w| w.wgm1().set(0b01).cs1().prescale_1024());
    tc1.ocr1a().write(|w| unsafe { w.bits(311) });

    let mut phase: u8 = 0;

    loop {
        // Diagonal rainbow, one full hue cycle every 32 columns, drifting over time.
        for x in 0..canvas.width() as i32 {
            for y in 0..canvas.height() as i32 {
                let hue = (x as u8)
                    .wrapping_mul(8)
                    .wrapping_add(y as u8 * 4)
                    .wrapping_add(phase);
                canvas.set_pixel(x, y, wheel(hue));
            }
        }

        let _ = canvas.send(&mut i2c);

        phase = phase.wrapping_add(4);

        while tc1.tifr1().read().ocf1a().bit_is_clear() {}
        // Writing 1 clears the flag.
        tc1.tifr1().write(|w| w.ocf1a().set_bit());
    }
}
