/*
  Trill Flex → Circle Infraction Event Stream (Arduino Uno)

  Purpose
  -------
  - Read Bela Trill Flex capacitive sensor over I2C.
  - Compute a simple infraction state (threshold + debounce).
  - Stream a 1-byte state over UART/USB-Serial at a fixed rate for Rust ingestion.

  Output Protocol (binary)
  ------------------------
  At 20 Hz (SAMPLE_PERIOD_MS = 50), Arduino writes exactly ONE byte:
    - 0x01 : CLEAR (no infraction)
    - 0xFE : INFRACTION

  Hardware
  --------
  - Board: Arduino Uno
  - Sensor: Bela Trill Flex
  - Wiring: SDA->A4, SCL->A5, VCC->5V, GND->GND

  Address note (Owen)
  -------------------
  - Trill default I2C address is 0x48.
  - To use multiple Trill sensors on the same I2C bus, you must assign unique addresses
    (typically by soldering address/jumper pads on the Trill board).
  - Long-term: maintain a map of sensor address -> physical location on the circle.
    (And optionally require agreement across sensors to reduce false positives.)

  Threshold note
  --------------
  - MAX_THRESHOLD is in raw Trill units (unitless integer readings returned by Trill).
    MAX_THRESHOLD=370 means: trigger when the max channel reading >= 370.
*/

#include <Wire.h>
#include <Trill.h>

Trill trill;

// 0 = production (binary-only output). 1 = debug (prints ASCII status + raw max).
#define DEBUG_ASCII 0

// Serial settings
const unsigned long SERIAL_BAUD = 115200; // Using 115200 for fast debug output when enabled

// I2C settings
const int I2C_ADDR = 0x48;                   // Default Trill address (other addresses require soldering pads)
const unsigned long I2C_CLOCK_HZ = 100000;   // Standard-mode I2C (stable on jumper wires)

// 20 Hz stream rate: one byte every 50 ms
const unsigned long SAMPLE_PERIOD_MS = 50;

// Trill sensor settings (from Bela Trill Flex examples)
// - Prescaler increases acquisition time (helps with higher baseline capacitance).
// - Noise threshold suppresses small fluctuations in raw readings.
const int PRESCALER = 3;
const int NOISE_THRESHOLD = 200;

// Classifier threshold (raw Trill units, unitless integer reading).
// Chosen from a labeled capture (no_touch / side_touch / top_touch).
const int MAX_THRESHOLD = 200;

// Debounce time (ms): candidate state must be stable for this long before committing.
const unsigned long DEBOUNCE_MS = 80;

// Output bytes required by integration
const uint8_t BYTE_CLEAR = 0x01;
const uint8_t BYTE_INFRACTION = 0xFE;

// Debounce state machine
bool committedInfraction = false;       // committed output state (what we are currently transmitting)
bool candidateInfraction = false;       // candidate state (pending debounce)
unsigned long candidateSinceMs = 0;     // when the candidate last changed (millis)

// Soft reset helper (Arduino Uno / AVR). Used instead of a permanent hot loop on setup failure.
static void resetMcu() {
  void (*resetFunc)(void) = 0;
  resetFunc();
}

void setup() {
  Serial.begin(SERIAL_BAUD);

  // Uno resets when a serial client connects; give sensor time to settle
  delay(2000);

  Wire.begin();
  Wire.setClock(I2C_CLOCK_HZ);
  delay(100);

#if DEBUG_ASCII
  // "CI" prefix = Circle Infractions subsystem.
  // We only print these status lines in DEBUG mode; production output is binary-only.
  Serial.println("CI,STATUS,BOOT");
#endif

  // Retry setup to avoid intermittent Trill init failures at boot.
  int ret = -1;
  for (int attempt = 1; attempt <= 10; attempt++) {
    ret = trill.setup(Trill::TRILL_FLEX, I2C_ADDR);

#if DEBUG_ASCII
    Serial.print("CI,STATUS,SETUP_TRY=");
    Serial.print(attempt);
    Serial.print(",RET=");
    Serial.println(ret);
#endif

    if (ret == 0) break;
    delay(200);
  }

  if (ret != 0) {
#if DEBUG_ASCII
    Serial.println("CI,STATUS,SETUP_FAILED");
#endif
    // Reset MCU so we can retry setup on next boot (instead of freezing in a hot loop forever).
    delay(200);
    resetMcu();
  }

  trill.setPrescaler(PRESCALER);
  delay(10);
  trill.setNoiseThreshold(NOISE_THRESHOLD);
  delay(10);
  trill.updateBaseline();

#if DEBUG_ASCII
  Serial.println("CI,STATUS,READY");
#endif
}

void loop() {
  delay(SAMPLE_PERIOD_MS);

  // Read one frame of raw channel data and take the max channel value (raw Trill units).
  trill.requestRawData();

  int maxRaw = 0;
  while (trill.rawDataAvailable() > 0) {
    int v = trill.rawDataRead();
    if (v > maxRaw) maxRaw = v;
  }

#if DEBUG_ASCII
  Serial.print("CI,RAW,maxRaw=");
  Serial.println(maxRaw);
#endif

  bool shouldInfraction = (maxRaw >= MAX_THRESHOLD);

  // Debounce:
  // If the instantaneous "shouldInfraction" state flips, mark it as a new candidate and start timing.
  if (shouldInfraction != candidateInfraction) {
    candidateInfraction = shouldInfraction;
    candidateSinceMs = millis();
  }

  // If the candidate state has stayed stable long enough, commit it.
  // (This rejects quick spikes / brief brushes.)
  if ((millis() - candidateSinceMs) >= DEBOUNCE_MS) {
    committedInfraction = candidateInfraction;
  }

  // Production integration: send one byte every tick (20 Hz).
  Serial.write(committedInfraction ? BYTE_INFRACTION : BYTE_CLEAR);
}