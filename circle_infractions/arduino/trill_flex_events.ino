/*
  Circle Infractions – Trill Flex → Arduino Uno (I2C)

  Purpose
  -------
  - Read two Bela Trill Flex sensors on a shared I2C bus (unique addresses).
  - Convert raw readings into a stable "pressed" state using:
      (1) per-sensor thresholding (baseline-calibrated at boot)
      (2) debounce (state must remain stable for DEBOUNCE_MS)
  - Stream a 1-byte state at 20 Hz for Rust ingestion.

  Output (binary, 1 byte @ 20 Hz)
  -------------------------------
    0x01 = CLEAR
    0xFE = INFRACTION   (if either sensor is pressed)

  Wiring (Arduino Uno)
  --------------------
    SDA = A4 (or SDA header)
    SCL = A5 (or SCL header)
    VCC = 5V
    GND = GND
*/

#include <Wire.h>
#include <Trill.h>

// ===== Addresses (must match your I2C scan) =====
const uint8_t SENSOR_A_I2C_ADDRESS = 0x48;
const uint8_t SENSOR_B_I2C_ADDRESS = 0x4E;

// ===== Timing =====
const unsigned long OUTPUT_PERIOD_MS = 50;       // 20 Hz output
const unsigned long I2C_CLOCK_HZ = 100000;       // stable on jumper wires

// ===== Trill settings (known-good defaults from Bela examples) =====
const int TRILL_PRESCALER = 3;
const int TRILL_NOISE_THRESHOLD = 200;

// ===== Detection tuning =====
const unsigned long DEBOUNCE_MS = 80;

// Baseline calibration: threshold = baseline + margin (per sensor)
const unsigned long CALIBRATION_WINDOW_MS = 1500;
const int THRESHOLD_MARGIN = 80; // raise if too sensitive; lower if not sensitive enough

// ===== Output bytes (Rust integration) =====
const uint8_t BYTE_CLEAR = 0x01;
const uint8_t BYTE_INFRACTION = 0xFE;

// Optional debug markers (disabled by default to keep output strictly 01/FE)
#define ENABLE_DEBUG_MARKERS 0
const uint8_t BYTE_BOOT = 0xAA;
const uint8_t BYTE_SETUP_OK = 0xAB;
const uint8_t BYTE_CALIBRATION_OK = 0xAC;
const uint8_t BYTE_SETUP_FAIL = 0xE0;

Trill sensorA;
Trill sensorB;

// Debounce state per sensor
bool sensorA_isPressed_committed = false;
bool sensorA_isPressed_candidate = false;
unsigned long sensorA_candidateStateStartTimeMs = 0;

bool sensorB_isPressed_committed = false;
bool sensorB_isPressed_candidate = false;
unsigned long sensorB_candidateStateStartTimeMs = 0;

// Calibrated thresholds (per sensor)
int sensorA_threshold = 0;
int sensorB_threshold = 0;

// Read one raw frame and return max channel value.
// Includes a short non-blocking wait to avoid hanging if data is momentarily unavailable.
int readMaxRawValue(Trill& trillSensor) {
  trillSensor.requestRawData();

  int maxRawValue = 0;

  unsigned long startWaitMs = millis();
  while (trillSensor.rawDataAvailable() == 0) {
    if (millis() - startWaitMs > 30) break; // don't block forever
  }

  while (trillSensor.rawDataAvailable() > 0) {
    int value = trillSensor.rawDataRead();
    if (value > maxRawValue) maxRawValue = value;
  }

  return maxRawValue;
}

void updateDebouncedState(
  bool shouldBePressed,
  bool& pressedStateCommitted,
  bool& pressedStateCandidate,
  unsigned long& candidateStateStartTimeMs
) {
  if (shouldBePressed != pressedStateCandidate) {
    pressedStateCandidate = shouldBePressed;
    candidateStateStartTimeMs = millis();
  }

  if ((millis() - candidateStateStartTimeMs) >= DEBOUNCE_MS) {
    pressedStateCommitted = pressedStateCandidate;
  }
}

int calibrateBaselineMax(Trill& trillSensor, unsigned long calibrationMs) {
  long sumOfMaxValues = 0;
  int sampleCount = 0;
  unsigned long startMs = millis();

  while (millis() - startMs < calibrationMs) {
    sumOfMaxValues += readMaxRawValue(trillSensor);
    sampleCount++;
    delay(10);
  }

  return (sampleCount > 0) ? (int)(sumOfMaxValues / sampleCount) : 0;
}

void setup() {
  Serial.begin(115200);
  delay(1500);

#if ENABLE_DEBUG_MARKERS
  Serial.write(BYTE_BOOT);
#endif

  Wire.begin();
  Wire.setClock(I2C_CLOCK_HZ);

  int sensorA_setupResult = sensorA.setup(Trill::TRILL_FLEX, SENSOR_A_I2C_ADDRESS);
  int sensorB_setupResult = sensorB.setup(Trill::TRILL_FLEX, SENSOR_B_I2C_ADDRESS);

  if (sensorA_setupResult != 0 || sensorB_setupResult != 0) {
#if ENABLE_DEBUG_MARKERS
    Serial.write(BYTE_SETUP_FAIL);
#endif
    while (1) {}
  }

  sensorA.setPrescaler(TRILL_PRESCALER);
  sensorA.setNoiseThreshold(TRILL_NOISE_THRESHOLD);
  sensorA.updateBaseline();

  sensorB.setPrescaler(TRILL_PRESCALER);
  sensorB.setNoiseThreshold(TRILL_NOISE_THRESHOLD);
  sensorB.updateBaseline();

#if ENABLE_DEBUG_MARKERS
  Serial.write(BYTE_SETUP_OK);
#endif

  // Calibration (don't touch sensors during this window)
  int sensorA_baseline = calibrateBaselineMax(sensorA, CALIBRATION_WINDOW_MS);
  int sensorB_baseline = calibrateBaselineMax(sensorB, CALIBRATION_WINDOW_MS);

  sensorA_threshold = sensorA_baseline + THRESHOLD_MARGIN;
  sensorB_threshold = sensorB_baseline + THRESHOLD_MARGIN;

#if ENABLE_DEBUG_MARKERS
  Serial.write(BYTE_CALIBRATION_OK);
#endif
}

void loop() {
  delay(OUTPUT_PERIOD_MS);

  int sensorA_maxRaw = readMaxRawValue(sensorA);
  int sensorB_maxRaw = readMaxRawValue(sensorB);

  bool sensorA_shouldBePressed = (sensorA_maxRaw >= sensorA_threshold);
  bool sensorB_shouldBePressed = (sensorB_maxRaw >= sensorB_threshold);

  updateDebouncedState(
    sensorA_shouldBePressed,
    sensorA_isPressed_committed,
    sensorA_isPressed_candidate,
    sensorA_candidateStateStartTimeMs
  );

  updateDebouncedState(
    sensorB_shouldBePressed,
    sensorB_isPressed_committed,
    sensorB_isPressed_candidate,
    sensorB_candidateStateStartTimeMs
  );

  bool anyInfraction = sensorA_isPressed_committed || sensorB_isPressed_committed;

  // Production: 1 byte @ 20 Hz
  Serial.write(anyInfraction ? BYTE_INFRACTION : BYTE_CLEAR);
}

