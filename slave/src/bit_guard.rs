// RAII guard for a PORTC bit that follows the "set to 1 on entry, set back to
// 0 on exit" pattern
// - OE (Output Enable): 1 = disabled, 0 = enabled
// - LE (Latch Enable): 1 = armed, 0 = latches data
pub struct BitGuard<'a> {
    portc: &'a avr_device::atmega328p::PORTC,
    bit: u8,
}

impl<'a> BitGuard<'a> {
    pub fn new(portc: &'a avr_device::atmega328p::PORTC, bit: u8) -> Self {
        portc
            .portc()
            .modify(|r, w| unsafe { w.bits(r.bits() | bit) });
        Self { portc, bit }
    }
}

impl<'a> Drop for BitGuard<'a> {
    fn drop(&mut self) {
        self.portc
            .portc()
            .modify(|r, w| unsafe { w.bits(r.bits() & !self.bit) });
    }
}
