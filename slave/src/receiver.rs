use crate::frame::{Frames, FRAME_BYTES};
use crate::twi_status::*;

// I2C slave, polled instead of interrupt driven. The TWI holds the bus clock low after each
// event, so the master waits until poll() handles it.
pub struct Receiver {
    // Write cursor into the back frame, reset each time we're freshly addressed.
    index: usize,
}

impl Receiver {
    pub const fn new() -> Self {
        Self { index: 0 }
    }

    // Handles the pending TWI event, if any.
    pub fn poll(&mut self, twi: &avr_device::atmega328p::TWI, frames: &mut Frames) {
        if twi.twcr().read().twint().bit_is_clear() {
            return;
        }

        let status = twi.twsr().read().bits() & 0xF8;

        match status {
            // Own SLA+W received and ACK'd: start of a new transaction.
            TW_SR_SLA_ACK | TW_SR_ARB_LOST_SLA_ACK => {
                // TWDR holds our SLA+W byte. Nothing we need, but it must still be read.
                let _ = twi.twdr().read().bits();
                self.index = 0;
            }

            // Data byte received and ACK'd: stored if there's room.
            TW_SR_DATA_ACK | TW_SR_DATA_NACK => {
                let byte = twi.twdr().read().bits();
                if self.index < FRAME_BYTES {
                    frames.write_back(self.index, byte);
                    self.index += 1;
                }
            }

            // STOP or repeated START: only a full frame is published, a short write (out of
            // protocol) is never shown.
            TW_SR_STOP => {
                if self.index == FRAME_BYTES {
                    frames.publish();
                }
            }

            // Master wants to read from us: we don't support that yet, send an empty byte.
            TW_ST_SLA_ACK | TW_ST_ARB_LOST_SLA_ACK | TW_ST_DATA_ACK | TW_ST_DATA_NACK => {
                twi.twdr().write(|w| unsafe { w.bits(0) });
            }

            _ => {}
        }

        twi.twcr().modify(|_, w| {
            // On a bus error, also set TWSTO, without it the peripheral stays wedged forever.
            if status == TW_BUS_ERROR {
                w.twsto().set_bit();
            }
            // Clear TWINT (writing 1 clears it, AVR quirk) to release the bus clock.
            w.twint().set_bit()
        });
    }
}
