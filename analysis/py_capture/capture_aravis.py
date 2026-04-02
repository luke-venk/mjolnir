#!/usr/bin/env python3

import argparse
import json
import os
import re
import sys
import time
from datetime import datetime

import gi
from PIL import Image

gi.require_version("Aravis", "0.8")
from gi.repository import Aravis


PFNC_MONO8 = 17301505
SUCCESS_STATUS_CANDIDATES = {0}


def utc_ts():
    return datetime.utcnow().strftime("%Y%m%dT%H%M%S_%fZ")


def ensure_dir(path):
    os.makedirs(path, exist_ok=True)


def safe_name(value):
    value = str(value) if value is not None else "unknown"
    return re.sub(r"[^A-Za-z0-9.-]+", "", value)


def get_device_ids():
    Aravis.update_device_list()
    n = Aravis.get_n_devices()
    return [Aravis.get_device_id(i) for i in range(n)]


def camera_label(camera, fallback_id, index):
    vendor = None
    model = None
    serial = None

    try:
        vendor = camera.get_vendor_name()
    except Exception:
        pass

    try:
        model = camera.get_model_name()
    except Exception:
        pass

    try:
        serial = camera.get_device_serial_number()
    except Exception:
        pass

    model = model or f"camera_{index}"
    serial = serial or fallback_id or f"id_{index}"
    vendor = vendor or "unknown_vendor"

    return {
        "index": index,
        "vendor": vendor,
        "model": model,
        "serial": serial,
        "device_id": fallback_id,
        "folder": f"{safe_name(model)}_{safe_name(serial)}",
    }


def get_camera_dimensions(camera):
    x = y = width = height = None
    try:
        x, y, width, height = camera.get_region()
    except Exception:
        pass
    return x, y, width, height


def get_camera_pixel_format(camera):
    try:
        return int(camera.get_pixel_format())
    except Exception:
        return None


def get_camera_payload(camera):
    try:
        return int(camera.get_payload())
    except Exception:
        return None


def open_camera(device_id):
    camera = Aravis.Camera.new(device_id)
    if camera is None:
        raise RuntimeError(f"Failed to open camera: {device_id}")
    return camera


def create_stream(camera):
    stream = camera.create_stream(None, None)
    if stream is None:
        raise RuntimeError("Failed to create stream")
    return stream


def configure_buffers(stream, payload, count=16):
    if payload is None or payload <= 0:
        raise RuntimeError(f"Invalid payload size: {payload}")
    for _ in range(count):
        stream.push_buffer(Aravis.Buffer.new_allocate(payload))


def save_mono8_preview(raw_bytes, width, height, png_path, jpg_path=None):
    expected = width * height
    actual = len(raw_bytes)

    if actual != expected:
        raise ValueError(
            f"Mono8 size mismatch: got {actual}, expected {expected} "
            f"(width={width}, height={height})"
        )

    img = Image.frombytes("L", (width, height), raw_bytes)
    img.save(png_path)

    if jpg_path:
        img.save(jpg_path, quality=95)


def save_preview(raw_bytes, width, height, pixel_format, png_path, jpg_path=None):
    if int(pixel_format) == PFNC_MONO8:
        save_mono8_preview(raw_bytes, width, height, png_path, jpg_path)
        return
    raise ValueError(f"Unsupported pixel format for preview: {pixel_format}")


def buffer_status_to_int(buffer):
    try:
        return int(buffer.get_status())
    except Exception:
        return None


def get_buffer_data_bytes(buffer):
    data = None
    if hasattr(buffer, "get_data"):
        data = buffer.get_data()
    elif hasattr(buffer, "data"):
        data = buffer.data

    if data is None:
        return None

    return bytes(data)


def is_success_status(status):
    return status in SUCCESS_STATUS_CANDIDATES


def debug_print(args, message):
    if args.debug:
        print(message)


def warmup_stream(camera, stream, args, width, height, payload):
    if args.warmup_seconds <= 0:
        return

    print(f"Warm-up: discarding buffers for {args.warmup_seconds:.2f}s")
    warmup_end = time.time() + args.warmup_seconds

    warmup_attempts = 0
    warmup_timeouts = 0
    warmup_empty = 0
    warmup_bad_status = 0
    warmup_ok = 0

    while time.time() < warmup_end:
        buffer = None
        try:
            warmup_attempts += 1
            buffer = stream.timeout_pop_buffer(args.timeout_us)

            if buffer is None:
                warmup_timeouts += 1
                debug_print(args, f"[warmup] attempt={warmup_attempts} timeout")
                continue

            status = buffer_status_to_int(buffer)
            raw_bytes = get_buffer_data_bytes(buffer)
            raw_size = len(raw_bytes) if raw_bytes is not None else -1

            if raw_bytes is None or raw_size == 0:
                warmup_empty += 1
                debug_print(
                    args,
                    f"[warmup] attempt={warmup_attempts} status={status} raw_size={raw_size} empty",
                )
                continue

            if not is_success_status(status):
                warmup_bad_status += 1
                debug_print(
                    args,
                    f"[warmup] attempt={warmup_attempts} status={status} raw_size={raw_size} bad_status",
                )
                continue

            warmup_ok += 1
            debug_print(
                args,
                f"[warmup] attempt={warmup_attempts} status={status} raw_size={raw_size} ok",
            )

        finally:
            if buffer is not None:
                try:
                    stream.push_buffer(buffer)
                except Exception:
                    pass

    print(
        f"Warm-up summary: attempts={warmup_attempts} ok={warmup_ok} "
        f"timeouts={warmup_timeouts} empty={warmup_empty} bad_status={warmup_bad_status}"
    )


def save_frame(output_dir, base, raw_bytes, metadata, args):
    raw_path = None
    json_path = None
    png_path = None

    if not args.stats_only:
        if not args.no_raw:
            raw_path = os.path.join(output_dir, f"{base}.raw")
            with open(raw_path, "wb") as f:
                f.write(raw_bytes)
            metadata["raw_file"] = os.path.basename(raw_path)

        if not args.no_preview:
            png_path = os.path.join(output_dir, f"{base}.png")
            save_preview(
                raw_bytes,
                metadata["width"],
                metadata["height"],
                metadata["pixel_format"],
                png_path,
            )
            metadata["preview_png"] = os.path.basename(png_path)

        if not args.no_json:
            json_path = os.path.join(output_dir, f"{base}.json")
            with open(json_path, "w", encoding="utf-8") as f:
                json.dump(metadata, f, indent=2)

    return raw_path, json_path, png_path


def capture_loop(camera, stream, camera_info, output_dir, args):
    x, y, width, height = get_camera_dimensions(camera)
    pixel_format = get_camera_pixel_format(camera)
    payload = get_camera_payload(camera)

    if width is None or height is None:
        raise RuntimeError("Camera width/height unavailable")

    if pixel_format != PFNC_MONO8:
        raise RuntimeError(
            f"Expected Mono8 ({PFNC_MONO8}), got pixel_format={pixel_format}"
        )

    expected_image_bytes = width * height

    start_time = time.time()
    deadline = start_time + args.duration if args.duration is not None else None

    attempts = 0
    good_buffers = 0
    frames_saved = 0
    timeouts = 0
    empty_buffers = 0
    bad_status = 0
    size_mismatch = 0
    write_errors = 0
    bytes_written = 0
    frames_skipped_by_sampling = 0

    last_stats_time = start_time
    last_stats_saved = 0
    last_stats_bytes = 0
    last_stats_good = 0

    camera.start_acquisition()
    print(
        f"Started acquisition: width={width}, height={height}, "
        f"pixel_format={pixel_format}, payload={payload}, "
        f"expected_image_bytes={expected_image_bytes}"
    )

    try:
        warmup_stream(camera, stream, args, width, height, payload)

        while True:
            if deadline is not None and time.time() >= deadline:
                break

            if args.max_frames is not None and frames_saved >= args.max_frames:
                break

            buffer = None
            try:
                attempts += 1
                buffer = stream.timeout_pop_buffer(args.timeout_us)

                if buffer is None:
                    timeouts += 1
                    debug_print(args, f"[capture] attempt={attempts} timeout")
                    continue

                status = buffer_status_to_int(buffer)
                raw_bytes = get_buffer_data_bytes(buffer)
                raw_size = len(raw_bytes) if raw_bytes is not None else -1

                debug_print(
                    args,
                    f"[capture] attempt={attempts} status={status} raw_size={raw_size} "
                    f"payload={payload} expected={expected_image_bytes}",
                )

                if raw_bytes is None or raw_size == 0:
                    empty_buffers += 1
                    continue

                if not is_success_status(status):
                    bad_status += 1
                    continue

                if raw_size != expected_image_bytes:
                    size_mismatch += 1
                    continue

                good_buffers += 1

                should_save = True
                if args.save_every > 1 and (good_buffers % args.save_every) != 0:
                    should_save = False

                if not should_save:
                    frames_skipped_by_sampling += 1
                    continue

                ts = utc_ts()
                base = f"frame_{frames_saved:06d}_{ts}"

                metadata = {
                    "timestamp_utc": ts,
                    "camera_index": camera_info["index"],
                    "camera_vendor": camera_info["vendor"],
                    "camera_model": camera_info["model"],
                    "camera_serial": camera_info["serial"],
                    "device_id": camera_info["device_id"],
                    "frame_index": frames_saved,
                    "good_buffer_index": good_buffers,
                    "width": width,
                    "height": height,
                    "offset_x": x,
                    "offset_y": y,
                    "pixel_format": pixel_format,
                    "payload_bytes_expected": payload,
                    "expected_image_bytes": expected_image_bytes,
                    "raw_size_bytes_actual": raw_size,
                    "buffer_status": status,
                }

                save_frame(output_dir, base, raw_bytes, metadata, args)

                frames_saved += 1
                if not args.stats_only and not args.no_raw:
                    bytes_written += raw_size

            except Exception as e:
                write_errors += 1
                print(f"capture error: {e}", file=sys.stderr)

            finally:
                if buffer is not None:
                    try:
                        stream.push_buffer(buffer)
                    except Exception:
                        pass

            now = time.time()
            if now - last_stats_time >= args.stats_interval:
                elapsed = now - start_time
                interval = now - last_stats_time
                total_fps = frames_saved / elapsed if elapsed > 0 else 0.0
                good_total_fps = good_buffers / elapsed if elapsed > 0 else 0.0
                interval_saved = frames_saved - last_stats_saved
                interval_good = good_buffers - last_stats_good
                interval_bytes = bytes_written - last_stats_bytes
                interval_saved_fps = interval_saved / interval if interval > 0 else 0.0
                interval_good_fps = interval_good / interval if interval > 0 else 0.0
                mbps = (interval_bytes / interval) / (1024 * 1024) if interval > 0 else 0.0

                print(
                    f"[stats] elapsed={elapsed:.2f}s attempts={attempts} good={good_buffers} saved={frames_saved} "
                    f"timeouts={timeouts} empty={empty_buffers} bad_status={bad_status} "
                    f"size_mismatch={size_mismatch} skipped={frames_skipped_by_sampling} "
                    f"write_errors={write_errors} fps_good_total={good_total_fps:.2f} "
                    f"fps_good_now={interval_good_fps:.2f} fps_saved_total={total_fps:.2f} "
                    f"fps_saved_now={interval_saved_fps:.2f} write_MBps={mbps:.2f}"
                )

                last_stats_time = now
                last_stats_saved = frames_saved
                last_stats_good = good_buffers
                last_stats_bytes = bytes_written

    finally:
        camera.stop_acquisition()

    total_elapsed = time.time() - start_time
    avg_saved_fps = frames_saved / total_elapsed if total_elapsed > 0 else 0.0
    avg_good_fps = good_buffers / total_elapsed if total_elapsed > 0 else 0.0
    avg_mbps = (
        (bytes_written / total_elapsed) / (1024 * 1024) if total_elapsed > 0 else 0.0
    )

    print("\nCapture complete")
    print(f" attempts : {attempts}")
    print(f" good_buffers : {good_buffers}")
    print(f" frames_saved : {frames_saved}")
    print(f" frames_skipped_by_sampling : {frames_skipped_by_sampling}")
    print(f" timeouts : {timeouts}")
    print(f" empty_buffers : {empty_buffers}")
    print(f" bad_status : {bad_status}")
    print(f" size_mismatch : {size_mismatch}")
    print(f" write_errors : {write_errors}")
    print(f" elapsed_s : {total_elapsed:.2f}")
    print(f" avg_good_fps : {avg_good_fps:.2f}")
    print(f" avg_saved_fps : {avg_saved_fps:.2f}")
    print(f" bytes_written : {bytes_written}")
    print(f" avg_write_MBps : {avg_mbps:.2f}")


def main():
    parser = argparse.ArgumentParser(
        description="Capture Mono8 frames from Aravis cameras as RAW+JSON+PNG with diagnostics"
    )
    parser.add_argument("--camera-index", type=int, required=True, help="Camera index to use")
    parser.add_argument("--output", default="camera_output", help="Base output directory")
    parser.add_argument("--duration", type=float, default=None, help="Capture duration in seconds")
    parser.add_argument("--max-frames", type=int, default=None, help="Maximum saved frames")
    parser.add_argument(
        "--timeout-us",
        type=int,
        default=1000000,
        help="Buffer timeout in microseconds",
    )
    parser.add_argument(
        "--buffer-count", type=int, default=64, help="Number of stream buffers"
    )
    parser.add_argument(
        "--stats-interval", type=float, default=0.5, help="Stats print interval in seconds"
    )
    parser.add_argument(
        "--warmup-seconds", type=float, default=0.5, help="Warm-up discard duration"
    )
    parser.add_argument(
        "--save-every", type=int, default=1, help="Save every Nth good frame"
    )
    parser.add_argument(
        "--no-preview", action="store_true", help="Do not write PNG previews"
    )
    parser.add_argument(
        "--no-json", action="store_true", help="Do not write JSON metadata"
    )
    parser.add_argument("--no-raw", action="store_true", help="Do not write RAW frame files")
    parser.add_argument(
        "--stats-only",
        action="store_true",
        help="Capture and report stats without writing files",
    )
    parser.add_argument(
        "--separate-folders",
        action="store_true",
        help="Put output in per-camera folder",
    )
    parser.add_argument(
        "--debug", action="store_true", help="Print per-attempt debug logging"
    )

    args = parser.parse_args()

    if args.duration is None and args.max_frames is None:
        parser.error(
            "You must provide at least one stopping condition: --duration or --max-frames"
        )

    if args.save_every < 1:
        parser.error("--save-every must be >= 1")

    if args.stats_only and args.no_raw and args.no_json and args.no_preview:
        pass

    ensure_dir(args.output)

    device_ids = get_device_ids()
    if not device_ids:
        raise RuntimeError("No cameras found")

    if args.camera_index < 0 or args.camera_index >= len(device_ids):
        raise RuntimeError(
            f"camera index {args.camera_index} out of range; found {len(device_ids)} camera(s)"
        )

    device_id = device_ids[args.camera_index]
    camera = open_camera(device_id)
    info = camera_label(camera, device_id, args.camera_index)

    output_dir = args.output
    if args.separate_folders:
        output_dir = os.path.join(output_dir, info["folder"])
        ensure_dir(output_dir)

    print(f"Using camera {args.camera_index}")
    print(f" vendor : {info['vendor']}")
    print(f" model : {info['model']}")
    print(f" serial : {info['serial']}")
    print(f" device_id: {info['device_id']}")
    print(f" output : {output_dir}")
    print(f" stats_only : {args.stats_only}")
    print(f" save_every : {args.save_every}")
    print(f" no_preview : {args.no_preview}")
    print(f" no_json : {args.no_json}")
    print(f" no_raw : {args.no_raw}")

    stream = create_stream(camera)
    payload = get_camera_payload(camera)
    configure_buffers(stream, payload, args.buffer_count)

    capture_loop(camera, stream, info, output_dir, args)


if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print(f"ERROR: {e}", file=sys.stderr)
        sys.exit(1)