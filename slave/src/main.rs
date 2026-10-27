// Firmware for the Rainbowduino V2.
//
// Receives a full framebuffer over I2C and displays it continuously.
//
// Board layout (top view)
//
//             +--------------------------------+
//    IN ( VCC | 0   1   2   3   4   5   6   7  | VCC ) OUT
//       ( VCC |                                | VCC )
//       ( GND | 1                              | GND )
//       ( GND |                                | GND )
//             | 2                              |
//             |                                |
//             | 3                              |
//             |                                |
//             | 4                              |
//         DTR |                                |
//         GND | 5                              |
//         TXD |                                |
//         RXD | 6                              |
//    IN ( GND |                                | GND ) OUT
//       ( SCL | 7                              | SCL )
//       ( SDA |                                | SDA )
//             +--------------------------------+
//

#![no_std]
#![no_main]
#![feature(abi_avr_interrupt)]

mod bcm_cursor;
mod bit_guard;
mod double_buffer;
mod twi_status;

use bcm_cursor::{BcmCursor, LEVELS};
use bit_guard::BitGuard;
use double_buffer::{line_bytes, DoubleBuffer, Frame, Line, FRAME_BYTES};

// Registers panic_halt's `#[panic_handler]` (required in no_std -- there's no default one). On
// panic it just loops forever, halting the MCU (MicroController Unit); `as _` imports it for this
// side effect only, since nothing here calls it by name.
use panic_halt as _;

// This board's I2C (Inter-Integrated Circuit) slave address. Each physical Rainbowduino on the
// shared bus needs a distinct value here before flashing. Conveniently overriden by
// scripts/flash.sh
const RB_I2C_ADDR: u8 = 08;

// PORTC bits driving the 3x MBI5168 shift registers.
// Serial Data In: the bit value shifted in on each CLK pulse
const SDI: u8 = 0x01;
// Clock: pulsing this shifts SDI's current value into the register
const CLK: u8 = 0x02;
// Latch Enable: high-then-low copies the shifted-in bits to the outputs
const LE: u8 = 0x04;
// Output Enable, active-low: HIGH disables all outputs
const OE: u8 = 0x08;

// Per-level dwell time (Timer2 ticks, prescaler /64, 4us/tick) before the next line/level advance.
// Gamma corrected (gamma ~= 2.2): the eye is far more sensitive to brightness differences at the
// low end than the high end, so low levels get short dwell times and high levels get long ones.
const DWELL_TICKS: [u8; LEVELS as usize] =
    [1, 1, 4, 5, 7, 9, 10, 13, 15, 17, 19, 22, 23, 26, 28, 31];

// Framebuffer: 3 color bytes per line (0-7) back to back, always in this order: Green, Red, Blue.
//
// Each byte packs 2 columns' brightness as 4-bit nibbles, in the reverse order of the columns on
// the physical board: byte 0 holds columns 0-1 (high nibble = column 0), ..., byte 3 holds columns
// 6-7 (high nibble = column 6).
static FRAMEBUFFER: DoubleBuffer = DoubleBuffer::new();

#[arduino_hal::entry]
fn main() -> ! {
    let peripherals = arduino_hal::Peripherals::take().unwrap();

    // Shift register control pins as outputs.
    peripherals
        .PORTC
        .ddrc()
        .write(|w| unsafe { w.bits(SDI | CLK | LE | OE) });
    // Line-select pins: PB0-2 (lines 0-2) and PD3-7 (lines 3-7) as outputs.
    peripherals.PORTB.ddrb().write(|w| unsafe { w.bits(0x07) });
    peripherals.PORTD.ddrd().write(|w| unsafe { w.bits(0xF8) });

    // Timer2: drives the line-scan/shift-out cycle. Normal mode, prescaler /64 (16MHz / 64 =
    // 250kHz, 4us/tick). TIMER2_OVF reloads TCNT2 on every call to set each BCM level's dwell time.
    peripherals.TC2.tccr2a().write(|w| unsafe { w.bits(0) });
    peripherals
        .TC2
        .tccr2b()
        .write(|w| unsafe { w.bits(0b0000_0100) });
    // TOIE2: enable the overflow interrupt (calls TIMER2_OVF below)
    peripherals.TC2.timsk2().write(|w| w.toie2().set_bit());

    // TWI slave setup. The address
    peripherals
        .TWI
        .twar()
        .write(|w| unsafe { w.bits(RB_I2C_ADDR << 1) });
    // and the settings.
    peripherals.TWI.twcr().write(|w| {
        w.twea()
            .set_bit() // TWEA: auto-acknowledge our address and incoming data bytes
            .twen()
            .set_bit() // TWEN: enable the TWI peripheral itself
            .twie()
            .set_bit() // TWIE: enable the TWI interrupt (calls fn TWI() below)
    });

    unsafe { avr_device::interrupt::enable() };

    loop {
        avr_device::asm::sleep();
    }
}

// Shifts a whole line's 12 bytes (24 nibble-packed pixel values) into the MBI5168, MSB first. A
// nibble lights its column only if its value exceeds `level`, the current BCM pass.
//
// PORTC is read once and then written directly: this runs in the display ISR, where nothing else
// touches PORTC.
fn shift_out_bytes(portc: &avr_device::atmega328p::PORTC, bytes: &Line, level: u8) {
    let base = portc.portc().read().bits() & !(SDI | CLK);
    // CLK low with the data bit, then CLK high: the rising edge shifts it in.
    let shift_bit = |on: bool| {
        let data = if on { base | SDI } else { base };
        portc.portc().write(|w| unsafe { w.bits(data) });
        portc.portc().write(|w| unsafe { w.bits(data | CLK) });
    };
    // Process the two nibbles for each byte.
    for &byte in bytes {
        shift_bit(byte >> 4 > level);
        shift_bit(byte & 0x0F > level);
    }
}

// Selects line 0-7 and deselects every other line in the same write: lines 0-2 live on PORTB, lines
// 3-7 on PORTD.
fn select_line(
    portb: &avr_device::atmega328p::PORTB,
    portd: &avr_device::atmega328p::PORTD,
    line: u8,
) {
    let portb_bit = if line < 3 { 0x04 >> line } else { 0 };
    portb
        .portb()
        .modify(|r, w| unsafe { w.bits((r.bits() & !0x07) | portb_bit) });

    let portd_bit = if line >= 3 { 0x80 >> (line - 3) } else { 0 };
    portd
        .portd()
        .modify(|r, w| unsafe { w.bits((r.bits() & !0xF8) | portd_bit) });
}

// Gamma-corrected dwell time for this level: the TCNT2 preload that makes it take
// DWELL_TICKS[level] ticks to reach the next overflow.
fn dwell_preload(level: u8) -> u8 {
    (256u16 - DWELL_TICKS[level as usize] as u16) as u8
}

// Fires automatically whenever Timer2's counter overflows. This is the display-refresh ISR: each
// call redraws one line at the current BCM level and advances, cycling through all 8 lines x 16
// levels to produce both persistence-of-vision and per-pixel grayscale.
#[avr_device::interrupt(atmega328p)]
fn TIMER2_OVF() {
    static mut CURSOR: BcmCursor = BcmCursor::new();
    // Snapshot of the current front buffer, refreshed once per full cycle (8 lines x 16 levels).
    static mut WORKING: Frame = [0; FRAME_BYTES];

    // Get the front buffer.
    if CURSOR.is_cycle_start() {
        FRAMEBUFFER.snapshot(WORKING);
    }

    let peripherals = unsafe { arduino_hal::Peripherals::steal() };

    peripherals
        .TC2
        .tcnt2()
        .write(|w| unsafe { w.bits(dwell_preload(CURSOR.level)) });

    // OE (Output Enable) off
    let _oe = BitGuard::new(&peripherals.PORTC, OE);

    select_line(&peripherals.PORTB, &peripherals.PORTD, CURSOR.line);

    // The scope here ensures _le latches before re-enabling the output
    {
        // Arm the latch
        let _le = BitGuard::new(&peripherals.PORTC, LE);

        shift_out_bytes(
            &peripherals.PORTC,
            line_bytes(WORKING, CURSOR.line),
            CURSOR.level,
        );
    }

    CURSOR.advance();
}

#[avr_device::interrupt(atmega328p)]
fn TWI() {
    // Write cursor into the back buffer for the in-progress I2C transaction,
    // reset to 0 each time we're freshly addressed.
    static mut RX_INDEX: usize = 0;

    use twi_status::*;

    let peripherals = unsafe { arduino_hal::Peripherals::steal() };
    let twi = peripherals.TWI;
    let status = twi.twsr().read().bits() & 0xF8;

    match status {
        // Own SLA+W received and ACK'd: start of a new transaction, reset the write cursor.
        TW_SR_SLA_ACK | TW_SR_ARB_LOST_SLA_ACK => {
            // TWDR holds the SLA+W byte here (our own address + the W bit)
            // Nothing we need, but it must still be read to clear it out.
            let _ = twi.twdr().read().bits();

            // New transaction starting: reset the write cursor so the next data byte lands at the
            // start of the framebuffer.
            *RX_INDEX = 0;
        }

        // Data byte received and ACK'd: store it into the back buffer at the current cursor
        // position, if there's room.
        TW_SR_DATA_ACK | TW_SR_DATA_NACK => {
            let byte = twi.twdr().read().bits();
            if *RX_INDEX < FRAME_BYTES {
                FRAMEBUFFER.write_byte(*RX_INDEX, byte);
                *RX_INDEX += 1;
            }
        }

        // STOP or repeated START: if a full frame was received, publish it so the display-refresh
        // ISR starts showing it on its next pass. A short write (out of protocol) is left in the
        // back buffer rather than shown half-updated.
        TW_SR_STOP => {
            if *RX_INDEX == FRAME_BYTES {
                FRAMEBUFFER.publish();
            }
        }

        // Master wants to read from us: we don't support that yet, send an empty byte.
        TW_ST_SLA_ACK | TW_ST_ARB_LOST_SLA_ACK | TW_ST_DATA_ACK | TW_ST_DATA_NACK => {
            twi.twdr().write(|w| unsafe { w.bits(0) });
        }

        // Everything else: nothing to do beyond the common re-arm below.
        _ => {}
    }

    twi.twcr().modify(|_, w| {
        // On a bus error, also set TWSTO, without it the peripheral stays wedged forever.
        if status == TW_BUS_ERROR {
            w.twsto().set_bit();
        }
        // Clear TWINT (writing 1 clears it, AVR quirk) to release the clock-stretched bus and let
        // the next event come in.
        w.twint().set_bit()
    });
}
