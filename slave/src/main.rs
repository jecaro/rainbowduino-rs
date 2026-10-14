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

mod bit_guard;
mod twi_status;

use bit_guard::BitGuard;

// Registers panic_halt's `#[panic_handler]` (required in no_std -- there's no default one). On
// panic it just loops forever, halting the MCU (MicroController Unit); `as _` imports it for this
// side effect only, since nothing here calls it by name.
use panic_halt as _;

// This board's I2C (Inter-Integrated Circuit) slave address. Each physical Rainbowduino on the
// shared bus needs a distinct value here before flashing. Conveniently overriden by
// scripts/flash.sh
const RB_I2C_ADDR: u8 = 0x0B;

// PORTC bits driving the 3x MBI5168 shift registers.
// Serial Data In: the bit value shifted in on each CLK pulse
const SDI: u8 = 0x01;
// Clock: pulsing this shifts SDI's current value into the register
const CLK: u8 = 0x02;
// Latch Enable: high-then-low copies the shifted-in bits to the outputs
const LE: u8 = 0x04;
// Output Enable, active-low: HIGH disables all outputs
const OE: u8 = 0x08;

// Framebuffer: 3 color bytes per line (0-7) back to back, always in this order: Green, Red, Blue.
//
// In one single byte, the bits are organized in the reverse order of the columns on the physical
// board: bit 7 is column 0, bit 6 is column 1, ..., bit 0 is column 7.
//
// Written by the TWI (Two-Wire Interface, Atmel/Microchip's name for I2C) ISR (Interrupt Service
// Routine), one byte per incoming I2C data byte, read by the display-refresh ISR every line-scan.
// No double buffering, no synchronization mechanism: the display ISR always reads whatever is
// currently in this array.
//
// A frame update that lands mid-scan can show a torn frame for one refresh cycle (a few
// milliseconds), creating a visual artifact only, hardly noticeable.
static mut FRAMEBUFFER: [u8; 24] = [0; 24];

// Returns the 3 color bytes (G, R, B) for the given line (0-7).
fn line_bytes(line: u8) -> &'static [u8; 3] {
    let start = line as usize * 3;
    unsafe { (&FRAMEBUFFER[start..][..3]).try_into().unwrap() }
}

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

    // Timer2: This drives the line-scan/shift-out cycle at a fixed rate: 16MHz / 64 / 256 (8-bit
    // counter) ~= 976 overflows/sec, i.e. one line advance every ~1ms, so all 8 lines (one full
    // display refresh) ~= 125 times/sec.
    //
    // Normal mode: no PWM (Pulse Width Modulation), no waveform generation
    peripherals.TC2.tccr2a().write(|w| unsafe { w.bits(0) });
    // CS22:0 = 0b100: prescaler /64
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

// Pulses CLK low then high, clocking SDI's current value into the MBI5168.
fn clk_pulse(portc: &avr_device::atmega328p::PORTC) {
    portc
        .portc()
        .modify(|r, w| unsafe { w.bits(r.bits() & !CLK) });
    portc
        .portc()
        .modify(|r, w| unsafe { w.bits(r.bits() | CLK) });
}

// Shifts a whole line's 3 color bytes (24 bits) into the MBI5168, MSB first.
fn shift_out_bytes(portc: &avr_device::atmega328p::PORTC, bytes: &[u8; 3]) {
    for &byte in bytes {
        for col in (0..8).rev() {
            let bit = (byte >> col) & 1 != 0;
            portc.portc().modify(|r, w| unsafe {
                w.bits(if bit { r.bits() | SDI } else { r.bits() & !SDI })
            });

            // Pulse the clock to shift in the bit.
            clk_pulse(portc);
        }
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

// Fires automatically whenever Timer2's counter overflows. This is the display-refresh ISR: each
// call redraws one line and advances to the next, cycling through all 8 lines to produce the full
// image via persistence of vision.
#[avr_device::interrupt(atmega328p)]
fn TIMER2_OVF() {
    // Line-scan position.
    static mut LINE: u8 = 0;

    let peripherals = unsafe { arduino_hal::Peripherals::steal() };

    // OE (Output Enable) off
    let _oe = BitGuard::new(&peripherals.PORTC, OE);

    select_line(&peripherals.PORTB, &peripherals.PORTD, *LINE);

    {
        // Arm the latch
        let _le = BitGuard::new(&peripherals.PORTC, LE);

        shift_out_bytes(&peripherals.PORTC, line_bytes(*LINE));
    }

    *LINE = (*LINE + 1) % 8;
}

#[avr_device::interrupt(atmega328p)]
fn TWI() {
    // Write cursor into FRAMEBUFFER for the in-progress I2C transaction,
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

        // Data byte received and ACK'd: store it into the framebuffer at the current cursor
        // position, if there's room.
        TW_SR_DATA_ACK | TW_SR_DATA_NACK => {
            let byte = twi.twdr().read().bits();
            if *RX_INDEX < 24 {
                unsafe { FRAMEBUFFER[*RX_INDEX] = byte };
                *RX_INDEX += 1;
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
