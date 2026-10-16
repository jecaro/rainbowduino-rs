// TWI status codes (ATmega328P datasheet, TWI slave mode status tables).
// Named after avr-libc's <util/twi.h> convention.

// Slave Receiver mode:

// Own SLA+W received, ACK returned
pub const TW_SR_SLA_ACK: u8 = 0x60;
// Arbitration lost as master, own SLA+W received, ACK returned
pub const TW_SR_ARB_LOST_SLA_ACK: u8 = 0x68;
// General call address received, ACK returned
#[allow(dead_code)]
pub const TW_SR_GCALL_ACK: u8 = 0x70;
// Arbitration lost as master, general call received, ACK returned
#[allow(dead_code)]
pub const TW_SR_ARB_LOST_GCALL_ACK: u8 = 0x78;
// Data received after own SLA+W, ACK returned
pub const TW_SR_DATA_ACK: u8 = 0x80;
// Data received after own SLA+W, NACK returned
pub const TW_SR_DATA_NACK: u8 = 0x88;
// Data received after general call, ACK returned
#[allow(dead_code)]
pub const TW_SR_GCALL_DATA_ACK: u8 = 0x90;
// Data received after general call, NACK returned
#[allow(dead_code)]
pub const TW_SR_GCALL_DATA_NACK: u8 = 0x98;
// STOP or repeated START received while addressed as slave
pub const TW_SR_STOP: u8 = 0xA0;

// Slave Transmitter mode:

// Own SLA+R received, ACK returned
pub const TW_ST_SLA_ACK: u8 = 0xA8;
// Arbitration lost as master, own SLA+R received, ACK returned
pub const TW_ST_ARB_LOST_SLA_ACK: u8 = 0xB0;
// Data byte transmitted, ACK received
pub const TW_ST_DATA_ACK: u8 = 0xB8;
// Data byte transmitted, NACK received
pub const TW_ST_DATA_NACK: u8 = 0xC0;
// Last data byte transmitted (TWEA=0), ACK received
#[allow(dead_code)]
pub const TW_ST_LAST_DATA: u8 = 0xC8;

// Miscellaneous:

// Illegal START/STOP condition detected
pub const TW_BUS_ERROR: u8 = 0x00;
