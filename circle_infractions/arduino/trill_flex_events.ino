/*
  Trill Flex → Circle Infraction Event Stream (Arduino Uno)

  Purpose
  -------
  - Read a Bela Trill Flex capacitive sensor over I2C.
  - Run a simple infraction classifier on-device (threshold + debounce).
  - Stream a 1-byte "state" over UART/USB-Serial at a fixed rate for backend ingestion.

  Output Protocol (binary)
  ------------------------
  At 20 Hz (SAMPLE_MS = 50), Arduino writes exactly ONE byte:
    - 0x00 : CLEAR (no infraction)
    - 0xFE : INFRACTION

  This is intended for Rust-side discovery/ingestion. No ASCII/newlines are emitted
  once running (except a short ASCII boot banner if DEBUG_ASCII is enabled).

  Hardware
  --------
  - Board: Arduino Uno
  - Sensor: Bela Trill Flex
  - Wiring: SDA->A4, SCL->A5, VCC->5V, GND->GND

  Notes on "magic numbers"
  ------------------------
  - I2C_ADDR = 0x48: this is NOT assumed to be the default; we discovered it via an I2C scan
    on our hardware. Trill sensors can have configurable addresses (address pads/jumpers).
    If a different unit is used, re-scan and update I2C_ADDR.
  - SAMPLE_MS = 50: 20 Hz sampling/stream rate (50 ms per update).
  - PRESCALER / NOISE_TH: copied from Bela's Trill Flex example defaults. Prescaler increases
    the Trill's internal acquisition period to handle higher baseline capacitance; noise threshold
    suppresses small fluctuations.

  Tuning
  ------
  - MX_THRESH chosen from our labeled dataset (no_touch / side_touch / top_touch).
    In our capture, side_touch max <= 360, so we use 370 to reject side touches.
*/

#include <Wire.h>
#include <Trill.h>

Trill trill;

// Set to 1 temporarily if you want readable Serial Monitor logs.
// Keep 0 for production integration (binary-only output).
#define DEBUG_ASCII 0

// I2C address discovered on our hardware via I2C scan.
const int I2C_ADDR = 0x48;

// 20 Hz stream rate: one byte every 50 ms
const unsigned long SAMPLE_MS = 50;

// Trill sensor settings (from Bela example; see notes above)
const int PRESCALER = 3;
const int NOISE_TH  = 200;

// Classifier (from our dataset)
const int MX_THRESH = 370;
const unsigned long DEBOUNCE_MS = 80;

// Output bytes required by integration
const uint8_t BYTE_CLEAR = 0x00;
const uint8_t BYTE_INFRACTION = 0xFE;

// State machine for debounce
bool state = false;     // false=CLEAR, true=INFRACTION
bool cand  = false;
unsigned long candSince = 0;

static void dbgln(const char* s) {
#if DEBUG_ASCII
  Serial.println(s);
#else
  (void)s;
#endif
}

static void dbg_status(const char* key, long val) {
#if DEBUG_ASCII
  Serial.print(key);
  Serial.println(val);
#else
  (void)key; (void)val;
#endif
}

void setup() {
  Serial.begin(115200);

  // Uno resets when a serial client connects; give sensor time to settle
  delay(2000);

  Wire.begin();
  Wire.setClock(100000);
  delay(100);

#if DEBUG_ASCII
  Serial.println("CI,STATUS,BOOT");
#endif

  // Retry setup (avoids intermittent ret=2 at boot)
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
    while (1) {}
  }

  trill.setPrescaler(PRESCALER);
  delay(10);
  trill.setNoiseThreshold(NOISE_TH);
  delay(10);
  trill.updateBaseline();

#if DEBUG_ASCII
  Serial.println("CI,STATUS,READY");
#endif
}

void loop() {
  delay(SAMPLE_MS);

  // Read one frame of raw channel data and compute max channel value (mx)
  trill.requestRawData();
  int mx = 0;
  while (trill.rawDataAvailable() > 0) {
    int v = trill.rawDataRead();
    if (v > mx) mx = v;
  }

#if DEBUG_ASCII
  Serial.print("CI,RAW,mx=");
  Serial.println(mx);
#endif

  bool shouldTouch = (mx >= MX_THRESH);

  // Debounce logic
  if (shouldTouch != cand) {
    cand = shouldTouch;
    candSince = millis();
  }

  if ((millis() - candSince) >= DEBOUNCE_MS && state != cand) {
    state = cand;
  }

  // Production integration: one byte every tick
  Serial.write(state ? BYTE_INFRACTION : BYTE_CLEAR);
}