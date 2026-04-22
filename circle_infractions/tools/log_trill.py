
import time
import csv
import serial
import msvcrt  # Windows-only (used for non-blocking keypress labels)

# NOTE: Windows-only because it uses msvcrt for keypress handling.
# macOS/Linux users can:
#  - use tools/read_serial.py to validate the binary byte protocol, or
#  - rewrite label input using an alternative input method.
PORT = "COM5"
BAUD = 115200
OUT = "trill_capture.csv"

label = "no_touch"

ser = serial.Serial(PORT, BAUD, timeout=1)
time.sleep(2)

print("Logging to", OUT)
print("Press: n=no_touch, t=top_touch, s=side_touch, q=quit")

with open(OUT, "w", newline="") as f:
    w = csv.writer(f)
    w.writerow(["pc_ms", "label", "arduino_line"])

    while True:
        if msvcrt.kbhit():
            k = msvcrt.getch().decode(errors="ignore").lower()
            if k == "q":
                break
            if k == "n":
                label = "no_touch"
            if k == "t":
                label = "top_touch"
            if k == "s":
                label = "side_touch"
            print("label =", label)

        line = ser.readline().decode("utf-8", errors="ignore").strip()
        if not line:
            continue
        if not line.startswith("CI,RAW"):
            continue

        w.writerow([int(time.time() * 1000), label, line])
        f.flush()
        print(line)


