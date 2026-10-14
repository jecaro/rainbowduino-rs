#!/usr/bin/env bash
# Build and flash the firmware to a Rainbowduino.
#
# Usage:
#   scripts/flash-slave.sh <serial-port> [i2c-address-hex]
#
# Examples:
#   scripts/flash-slave.sh /dev/ttyUSB0      # flash with whatever RB_I2C_ADDR is in src/main.rs
#   scripts/flash-slave.sh /dev/ttyUSB0 0x09 # patch RB_I2C_ADDR to 0x09 first, then flash

set -euo pipefail

PORT="${1:?Usage: scripts/flash-slave.sh <serial-port> [i2c-address-hex]}"
ADDR="${2:-}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SLAVE_DIR="$REPO_ROOT/slave"
MAIN_RS="$SLAVE_DIR/src/main.rs"

cd "$SLAVE_DIR"

if [ -n "$ADDR" ]; then
  echo "Setting RB_I2C_ADDR = $ADDR in src/main.rs"
  sed -i "s/^const RB_I2C_ADDR: u8 = .*/const RB_I2C_ADDR: u8 = $ADDR;/" "$MAIN_RS"
fi

echo "Building..."
cargo build --release

ELF="target/avr-none/release/slave.elf"
HEX="target/avr-none/release/slave.hex"

echo "Generating hex..."
avr-objcopy -O ihex -R .eeprom "$ELF" "$HEX"

echo "Flashing to $PORT..."
avrdude -p atmega328p -c arduino -b 57600 -P "$PORT" -D -U "flash:w:$HEX:i"

echo "Done."
