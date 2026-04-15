#!/usr/bin/env python3
import argparse
import re
import shutil
import subprocess
import sys
from collections import Counter

PTP_FILTER = "udp port 319 or udp port 320 or ether proto 0x88f7"
PTP_PATTERNS = [
    re.compile(r"\b319\b"),
    re.compile(r"\b320\b"),
    re.compile(r"0x88f7", re.IGNORECASE),
    re.compile(r"\bPTP\b", re.IGNORECASE),
]
HEADER = "------------------------------------------------------------------------"


def parse_args():
    parser = argparse.ArgumentParser(
        description="Check whether PTP traffic is present between cameras."
    )
    parser.add_argument("interface", help="Network interface to sniff on")
    parser.add_argument(
        "-c", "--count", type=int, default=50,
        help="Number of matching packets to wait for before stopping"
    )
    parser.add_argument(
        "-t", "--timeout", type=int, default=10,
        help="Seconds to allow tcpdump to run"
    )
    parser.add_argument("--sudo", action="store_true", help="Run tcpdump through sudo")
    return parser.parse_args()


def build_command(args):
    command = []
    if args.sudo:
        command.append("sudo")
    command.extend([
        "tcpdump",
        "-i", args.interface,
        "-nn",
        "-l",
        "-c", str(args.count),
        PTP_FILTER,
    ])
    return command


def is_ptp_line(line):
    return any(pattern.search(line) for pattern in PTP_PATTERNS)


def summarize(lines):
    peers = Counter()
    packet_pattern = re.compile(
        r"(\d+\.\d+\.\d+\.\d+)(?:\.(\d+))?\s*>\s*"
        r"(\d+\.\d+\.\d+\.\d+)(?:\.(\d+))?"
    )
    for line in lines:
        match = packet_pattern.search(line)
        if match:
            src, sport, dst, dport = match.groups()
            peers[(src, sport or "-", dst, dport or "-")] += 1
    return peers


def print_header():
    print(HEADER)
    print("CHECKING FOR PTP COMMUNICATION OVER SWITCH...")
    print(HEADER)


def main():
    args = parse_args()
    print_header()
    print(f"Interface: {args.interface}")
    print(f"Target packet count: {args.count}")
    print(f"Timeout: {args.timeout}s")

    if shutil.which("tcpdump") is None:
        print("tcpdump not found in PATH", file=sys.stderr)
        return 2

    try:
        result = subprocess.run(
            build_command(args),
            capture_output=True,
            text=True,
            timeout=args.timeout + 2,
        )
    except subprocess.TimeoutExpired:
        print("tcpdump timed out before completion", file=sys.stderr)
        return 2

    if result.stderr.strip():
        print(result.stderr.strip(), file=sys.stderr)

    if result.returncode not in (0, 1):
        return result.returncode

    lines = [line for line in result.stdout.splitlines() if line.strip()]
    ptp_lines = [line for line in lines if is_ptp_line(line)]

    if not ptp_lines:
        print("No PTP traffic detected.")
        return 1

    peers = summarize(ptp_lines)
    print("PTP communication detected successfully.")
    print(f"Matched packets: {len(ptp_lines)}")
    print(f"Unique flows observed: {len(peers)}")
    print("Observed peers:")
    for (src, sport, dst, dport), count in peers.most_common():
        print(f"  - {src}:{sport} -> {dst}:{dport} ({count} packets)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
