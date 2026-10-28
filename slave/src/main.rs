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
mod frame;
mod receiver;
mod twi_status;

use bcm_cursor::{BcmCursor, LEVELS};
use bit_guard::BitGuard;
use frame::{line_bytes, Frames, Line};
use receiver::Receiver;

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

// Outputs kept off after switching lines, so the previous line is fully off before the new data
// lights up (otherwise it shows a faint copy of it: ghosting). Found empirically.
const BLANKING_US: u32 = 10;

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
    });

    // Timer2. Set the mode to normal (overflow). And the speed: 16MHz / 64 = 250kHz, 4us/tick.
    peripherals
        .TC2
        .tccr2b()
        .write(|w| unsafe { w.bits(0b0000_0100) });
    // TOIE2: enable the overflow interrupt (calls TIMER2_OVF below)
    peripherals.TC2.timsk2().write(|w| w.toie2().set_bit());

    // TIMER2_OVF is the only interrupt. TWI is polled.
    unsafe { avr_device::interrupt::enable() };

    // Display loop.
    let mut cursor = BcmCursor::new();
    let mut frames = Frames::new();
    let mut receiver = Receiver::new();
    loop {
        // A new frame only between full cycles (8 lines x 16 levels).
        if cursor.is_cycle_start() {
            frames.show_latest();
        }

        // Copied so that polling can borrow `frames` meanwhile.
        let line = *line_bytes(frames.front(), cursor.line);
        let mut poll = || receiver.poll(&peripherals.TWI, &mut frames);

        // Write the current line to the shift registers.
        shift_out_bytes(&peripherals.PORTC, &line, cursor.level, &mut poll);

        // Serve I2C until the current step is over.
        while !dwell_ended(&peripherals.PORTC) {
            poll();
        }

        // Show the just-shifted line.
        latch(&peripherals, cursor.line);

        // After latch(), so blanking doesn't eat into the dwell time.
        start_dwell(&peripherals.TC2, cursor.level);

        cursor.advance();
    }
}

// Shifts a whole line's 12 bytes (24 nibble-packed pixel values) into the MBI5168, MSB first. A
// nibble lights its column only if its value exceeds `level`, the current BCM pass.
//
// Toggles SDI and CLK through PINC (writing 1 to a PINC bit flips that PORTC bit), never writing
// PORTC itself: TIMER2_OVF may set OE meanwhile, and a PORTC write would undo it.
//
// Calls `poll` between bytes, so I2C isn't kept waiting for the whole line.
fn shift_out_bytes(
    portc: &avr_device::atmega328p::PORTC,
    bytes: &Line,
    level: u8,
    poll: &mut impl FnMut(),
) {
    let toggle = |bits: u8| portc.pinc().write(|w| unsafe { w.bits(bits) });
    let mut sdi_on = portc.portc().read().bits() & SDI != 0;

    // Two CLK toggles give exactly one rising edge, which shifts SDI in, whatever CLK's level.
    let mut shift_bit = |on: bool| {
        if on != sdi_on {
            toggle(SDI);
            sdi_on = on;
        }
        toggle(CLK);
        toggle(CLK);
    };

    // Process the two nibbles for each byte.
    for &byte in bytes {
        shift_bit(byte >> 4 > level);
        shift_bit(byte & 0x0F > level);
        poll();
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

// Starts this level's gamma-corrected dwell time: preloads TCNT2 so that Timer2 overflows after
// DWELL_TICKS[level] ticks.
fn start_dwell(tc2: &avr_device::atmega328p::TC2, level: u8) {
    let preload = (256u16 - DWELL_TICKS[level as usize] as u16) as u8;
    // Restart the prescaler, otherwise the first tick from TCNT2 comes early.
    tc2.gtccr().write(|w| w.psrasy().set_bit());
    tc2.tcnt2().write(|w| unsafe { w.bits(preload) });
}

// True once TIMER2_OVF has turned the outputs off.
fn dwell_ended(portc: &avr_device::atmega328p::PORTC) -> bool {
    portc.portc().read().bits() & OE != 0
}

// Fires when the current step's dwell time is over. Turns the outputs off right away, even if the
// next step isn't shifted in yet: otherwise short levels would stay on for as long as shifting takes
// (~35us instead of 4-28us), much too bright.
#[avr_device::interrupt(atmega328p)]
fn TIMER2_OVF() {
    let portc = unsafe { arduino_hal::Peripherals::steal() }.PORTC;
    portc
        .portc()
        .modify(|r, w| unsafe { w.bits(r.bits() | OE) });
}

// Switches the display to `line`, with the data shifted into the MBI5168.
fn latch(peripherals: &arduino_hal::Peripherals, line: u8) {
    // OE (Output Enable) off
    let _oe = BitGuard::new(&peripherals.PORTC, OE);

    select_line(&peripherals.PORTB, &peripherals.PORTD, line);

    // Pulse LE: copies the shift register to the outputs.
    drop(BitGuard::new(&peripherals.PORTC, LE));

    arduino_hal::delay_us(BLANKING_US);
}
