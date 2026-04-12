# Circle Infractions – Arduino (Trill Flex)

## What this does
Reads a Bela Trill Flex sensor on an Arduino Uno over I2C and streams a **1-byte state**
at **20 Hz** for backend ingestion:

- `0x00` = CLEAR
- `0xFE` = INFRACTION
Hardware validation: run tools/read_serial.py --port COMx and verify output bytes are 00 (clear) and FE (infraction)
## Hardware
- Arduino Uno
- Bela Trill Flex
- Wiring:
  - Trill SDA → Arduino A4
  - Trill SCL → Arduino A5
  - Trill VCC → 5V
  - Trill GND → GND

## Flashing the Arduino
1. Install **Arduino IDE** (2.x is fine).
2. Install the **Trill** library:
   - Arduino IDE → Tools → Manage Libraries → search `Trill` → install Bela/Trill.
3. Open the sketch:
   - `circle_infractions/arduino/trill_flex_events.ino`
4. Select board:
   - Tools → Board → **Arduino Uno**
5. Select port:
   - Tools → Port → the Uno’s COM/tty port
6. Upload (→ button).

### If setup fails (RET=2)
- Re-seat jumpers (SDA/SCL/VCC/GND), confirm VCC is on **5V** (not VIN).
- Power-cycle the board.
- Confirm I2C address (our hardware uses `0x48`). If needed, run an I2C scanner and update `I2C_ADDR`.

## Python tools (optional)
These are only for quick local testing. The Rust backend will ingest the binary stream directly.

### `tools/read_serial.py` (cross-platform)
Reads bytes and prints them as hex so you can verify:
- `00` when clear
- `FE` when infraction

Usage:
```bash
pip install pyserial
python tools/read_serial.py --port <PORT>