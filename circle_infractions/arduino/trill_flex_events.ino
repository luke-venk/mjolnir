#include <Wire.h>
#include <Trill.h>

const uint8_t ADDR_A = 0x48;
const uint8_t ADDR_B = 0x4E;

const unsigned long SAMPLE_PERIOD_MS = 50;   // 20 Hz
const unsigned long DEBOUNCE_MS = 80;

const int PRESCALER = 3;
const int NOISE_THRESHOLD = 200;
const int THRESH_MARGIN = 80;                // add to baseline max

const uint8_t BYTE_CLEAR = 0x01;
const uint8_t BYTE_INFRACTION = 0xFE;

Trill sA, sB;

bool committedA = false, candidateA = false;
unsigned long candidateSinceA_ms = 0;

bool committedB = false, candidateB = false;
unsigned long candidateSinceB_ms = 0;

int threshA = 0;
int threshB = 0;

int readMaxRaw(Trill& t) {
  t.requestRawData();
  int mx = 0;
  while (t.rawDataAvailable() > 0) {
    int v = t.rawDataRead();
    if (v > mx) mx = v;
  }
  return mx;
}

void debounceUpdate(bool shouldPressed, bool& committed, bool& candidate, unsigned long& candidateSince_ms) {
  if (shouldPressed != candidate) {
    candidate = shouldPressed;
    candidateSince_ms = millis();
  }
  if ((millis() - candidateSince_ms) >= DEBOUNCE_MS) {
    committed = candidate;
  }
}

int calibrateBaselineMax(Trill& t, unsigned long ms) {
  long sum = 0;
  int n = 0;
  unsigned long start = millis();
  while (millis() - start < ms) {
    sum += readMaxRaw(t);
    n++;
    delay(10);
  }
  return (n > 0) ? (int)(sum / n) : 0;
}

void setup() {
  Serial.begin(115200);
  delay(1500);

  Wire.begin();
  Wire.setClock(100000);

  int rA = sA.setup(Trill::TRILL_FLEX, ADDR_A);
  int rB = sB.setup(Trill::TRILL_FLEX, ADDR_B);
  if (rA != 0 || rB != 0) {
    while (1) { delay(1000); } // fail silent (no extra bytes)
  }

  sA.setPrescaler(PRESCALER);
  sA.setNoiseThreshold(NOISE_THRESHOLD);
  sA.updateBaseline();

  sB.setPrescaler(PRESCALER);
  sB.setNoiseThreshold(NOISE_THRESHOLD);
  sB.updateBaseline();

  // IMPORTANT: don't touch sensors during calibration
  int baseA = calibrateBaselineMax(sA, 1500);
  int baseB = calibrateBaselineMax(sB, 1500);

  threshA = baseA + THRESH_MARGIN;
  threshB = baseB + THRESH_MARGIN;
}

void loop() {
  delay(SAMPLE_PERIOD_MS);

  int maxA = readMaxRaw(sA);
  int maxB = readMaxRaw(sB);

  bool shouldA = (maxA >= threshA);
  bool shouldB = (maxB >= threshB);

  debounceUpdate(shouldA, committedA, candidateA, candidateSinceA_ms);
  debounceUpdate(shouldB, committedB, candidateB, candidateSinceB_ms);

  bool anyInfraction = committedA || committedB;
  Serial.write(anyInfraction ? BYTE_INFRACTION : BYTE_CLEAR);
}
