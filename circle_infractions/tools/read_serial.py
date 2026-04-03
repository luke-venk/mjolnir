import time
import serial

PORT = "COM5"   # change if needed
BAUD = 115200

ser = serial.Serial(PORT, BAUD, timeout=1)
time.sleep(2)

print("Listening... Ctrl+C to stop")
while True:
    line = ser.readline().decode("utf-8", errors="ignore").strip()
    if line:
        print(line)