import argparse
import time
import serial


def main() -> None:
    """
    Reads a 1-byte stream from the Arduino over USB-Serial and prints bytes as hex.

    Expected production values:
      - 01 = CLEAR
      - FE = INFRACTION

    This script also prints an approximate bytes/sec rate once per second so you can
    confirm you are receiving ~20 bytes/sec when the Arduino is streaming at 20 Hz.
    """
    parser = argparse.ArgumentParser(
        description="Read 1-byte circle infraction stream from Arduino and print hex bytes."
    )
    parser.add_argument(
        "--port",
        required=True,
        help="Windows: COM5 | macOS: /dev/tty.usbmodem* | Linux: /dev/ttyACM0",
    )
    parser.add_argument("--baud", type=int, default=115200)
    parser.add_argument(
        "--timeout",
        type=float,
        default=0.2,
        help="Serial read timeout in seconds (default: 0.2).",
    )
    args = parser.parse_args()

    ser = serial.Serial(args.port, args.baud, timeout=args.timeout)
    time.sleep(2)  # Arduino often resets when the serial port opens

    print("Listening... Ctrl+C to stop")
    last = time.time()
    count = 0

    try:
        while True:
            b = ser.read(1)
            if b:
                print(f"{b[0]:02X}")
                count += 1

            now = time.time()
            if now - last >= 1.0:
                if count:
                    print(f"(~{count} bytes/sec)")
                else:
                    print(".")  # heartbeat: connected but no bytes seen
                count = 0
                last = now

    except KeyboardInterrupt:
        pass
    finally:
        ser.close()


if __name__ == "__main__":
    main()


