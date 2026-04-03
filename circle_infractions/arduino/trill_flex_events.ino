#include <Wire.h>
#include <Trill.h>

Trill trill;

const int I2C_ADDR = 0x48;

// 
const unsigned long SAMPLE_MS = 50;   // 20 Hz (stable)

// 
const int PRESCALER = 3;
const int NOISE_TH = 200;

// ---- Infraction rule (from our dataset) ----
//  capture showed side_touch max <= 360, so 370 rejects side-touch in your data.
const int MX_THRESH = 370;
const unsigned long DEBOUNCE_MS = 80;

// ---- State ----
bool state = false;     // false=CLEAR, true=INFRACTION
bool cand = false;
unsigned long candSince = 0;

void setup() {
  Serial.begin(115200);

  // UNO resets when a serial client connects; give sensor time to settle
  delay(2000);

  Wire.begin();
  Wire.setClock(100000);
  delay(100);

  Serial.println("CI,STATUS,0,BOOT");

  
  int ret = -1;
  for (int attempt = 1; attempt <= 10; attempt++) {
    ret = trill.setup(Trill::TRILL_FLEX, I2C_ADDR);

    Serial.print("CI,STATUS,");
    Serial.print(millis());
    Serial.print(",SETUP_TRY=");
    Serial.print(attempt);
    Serial.print(",RET=");
    Serial.println(ret);

    if (ret == 0) break;
    delay(200);
  }

  if (ret != 0) {
    Serial.println("CI,STATUS,0,SETUP_FAILED");
    while (1) {}
  }

  trill.setPrescaler(PRESCALER);
  delay(10);
  trill.setNoiseThreshold(NOISE_TH);
  delay(10);
  trill.updateBaseline();

  Serial.println("CI,STATUS,0,READY");
}

void loop() {
  delay(SAMPLE_MS);

  // Read one frame of raw channel data and take max channel value
  trill.requestRawData();
  int mx = 0;
  while (trill.rawDataAvailable() > 0) {
    int v = trill.rawDataRead();
    if (v > mx) mx = v;
  }

  // Optional heartbeat (once per second)
  static unsigned long lastBeat = 0;
  if (millis() - lastBeat > 1000) {
    lastBeat = millis();
    Serial.print("CI,HEARTBEAT,");
    Serial.println(millis());
  }

  bool shouldTouch = (mx >= MX_THRESH);

  // debounce
  if (shouldTouch != cand) {
    cand = shouldTouch;
    candSince = millis();
  }


  if ((millis() - candSince) >= DEBOUNCE_MS && state != cand) {
    state = cand;
    if (state) {
      Serial.print("CI,INFRACTION,");
      Serial.println(millis());
    } else {
      Serial.print("CI,CLEAR,");
      Serial.println(millis());
    }
  }
}