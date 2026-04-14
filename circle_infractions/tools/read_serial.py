import argparse
import time
import serial


def main():
    parser = argparse.ArgumentParser(
        description="Read 1-byte circle infraction stream from Arduino and print hex bytes."
    )
    parser.add_argument(
        "--port",
        required=True,
        help="Windows: COM5 | macOS: /dev/tty.usbmodem* | Linux: /dev/ttyACM0",
    )
    parser.add_argument("--baud", type=int, default=115200)
    args = parser.parse_args()

    ser = serial.Serial(args.port, args.baud, timeout=1)
    time.sleep(2)  # Arduino often resets on connect

    print("Listening... Ctrl+C to stop")
    try:
        while True:
            b = ser.read(1)
            if not b:
                continue
            print(f"{b[0]:02X}")  # prints "01" or "FE"
    except KeyboardInterrupt:
        pass
    finally:
        ser.close()


if __name__ == "__main__":
    main()