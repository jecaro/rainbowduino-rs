#!/usr/bin/env bash
# Build and flash master/ to the Arduino acting as I2C master.
#
# Usage:
#   scripts/flash-master.sh <serial-port> <binary>

set -euo pipefail

USAGE="Usage: scripts/flash-master.sh <serial-port> <binary>"
PORT="${1:?$USAGE}"
BIN="${2:?$USAGE}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MASTER_DIR="$REPO_ROOT/master"

cd "$MASTER_DIR"

echo "Building..."
cargo build --release --bin "$BIN"

ELF="target/avr-none/release/$BIN.elf"
HEX="target/avr-none/release/$BIN.hex"

echo "Generating hex..."
avr-objcopy -O ihex -R .eeprom "$ELF" "$HEX"

echo "Flashing to $PORT..."
avrdude -p atmega328p -c arduino -b 57600 -P "$PORT" -D -U "flash:w:$HEX:i"

echo "Done."
