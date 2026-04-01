Capture diagnostic frame output from LUCID cameras using Aravis and Python.

`capture_aravis.py` is a small acquisition and debugging tool for LUCID cameras. It opens a selected camera, receives frames through Aravis, validates them, and optionally writes per-frame output to disk.

It is designed primarily for:

- camera bring-up
- acquisition debugging
- single-camera and dual-camera stability testing
- throughput measurement
- isolating capture-path issues from file I/O overhead

It is **not** a final video recorder.

---

## Features

- Capture from a selected camera by index
- Stop by:
  - capture duration
  - maximum saved frame count
- Save per-frame:
  - RAW bytes
  - PNG preview (SIGNIFICANTLY SLOWS)
  - JSON metadata
- Run in **stats-only** mode for acquisition benchmarking
- Reduce output pressure with:
  - raw-only mode
  - no-preview mode
  - no-json mode
  - save-every-N sampling
- Print ongoing capture diagnostics:
  - attempts
  - good buffers
  - saved frames
  - timeouts
  - bad statuses
  - size mismatches
  - write errors
  - effective FPS

---

## Requirements

### Software
- Python 3
- [Aravis](https://github.com/AravisProject/aravis) with Python GI bindings
- Pillow

### Python imports used
- `gi.repository.Aravis`
- `PIL.Image`

### Camera requirements
- An Aravis-compatible camera visible to the host
- Camera must be configured to produce **Mono8** frames

The current script expects `Mono8` pixel format and will fail on other formats.

---

## Installation Notes

You will need Aravis and the relevant Python bindings available in the Python environment that runs the script.

If you want to sanity-check imports before running, try:

```bash
python3 -c "import gi; gi.require_version('Aravis', '0.8'); from gi.repository import Aravis; print('Aravis OK')"
python3 -c "from PIL import Image; print('Pillow OK')"
```

---

## Basic Usage

```bash
python3 capture_aravis.py --camera-index <N> [options]
```

At least one stopping condition is required:

- `--duration <seconds>`
- or `--max-frames <count>`

---

## Command-Line Arguments

### Required

#### `--camera-index`
Zero-based camera index to use.

Example:
```bash
--camera-index 0
```

---

### Stopping Conditions

#### `--duration <seconds>`
Capture for a fixed number of seconds.

Example:
```bash
--duration 5
```

#### `--max-frames <count>`
Stop after saving a fixed number of frames.

Example:
```bash
--max-frames 100
```

You must provide at least one stopping condition.

---

### Output Options

#### `--output <dir>`
Base output directory.

Default:
```bash
camera_output
```

#### `--separate-folders`
Create a per-camera subfolder under the output directory.

Useful when testing multiple cameras into the same output root.

---

### Performance / Write Control

These options are especially useful when diagnosing whether a bottleneck is in acquisition or in file writing.

#### `--stats-only`
Capture and validate frames, but do not write files.

Use this to measure acquisition performance with minimal disk overhead.

#### `--no-preview`
Do not write PNG preview files.

#### `--no-json`
Do not write JSON metadata files.

#### `--no-raw`
Do not write RAW frame files.

#### `--save-every <N>`
Only save every Nth good frame.

Examples:
- `--save-every 1` → save every good frame
- `--save-every 5` → save every 5th good frame

This is useful when the acquisition path is healthy but full per-frame persistence is too expensive.

---

### Capture / Buffer Tuning

#### `--buffer-count <N>`
Number of stream buffers to allocate.

Default:
```bash
64
```

Larger values may help during sustained capture or dual-camera testing.

#### `--timeout-us <microseconds>`
Timeout for waiting on a frame buffer.

Default:
```bash
1000000
```

#### `--warmup-seconds <seconds>`
Discard frames during an initial warm-up period before statistics begin.

Default:
```bash
0.5
```

#### `--stats-interval <seconds>`
Interval between runtime stats prints.

Default:
```bash
0.5
```

#### `--debug`
Enable verbose per-attempt capture logging.

Useful for detailed troubleshooting, but usually too noisy for normal testing.

---

## Output Layout

Depending on flags, the script may write these files for each saved frame:

- `*.raw` — raw frame bytes
- `*.png` — PNG preview image
- `*.json` — frame metadata

If `--separate-folders` is enabled, output is grouped into a per-camera directory based on camera model and serial number.

---

## What the Script Measures

The script reports several counters and rates while running and again at the end.

### Key fields

- **attempts**
  Number of buffer-pop attempts made by the script

- **good_buffers**
  Number of successfully received and validated buffers

- **frames_saved**
  Number of frames actually written to disk after sampling/output rules

- **frames_skipped_by_sampling**
  Good frames intentionally skipped due to `--save-every`

- **timeouts**
  Number of times no buffer arrived before timeout

- **empty_buffers**
  Buffers received with no usable data

- **bad_status**
  Buffers with a non-success Aravis status

- **size_mismatch**
  Buffers whose data length did not match the expected image size

- **write_errors**
  Exceptions during file output

- **avg_good_fps**
  Effective validated acquisition rate

- **avg_saved_fps**
  Effective persisted frame rate

- **avg_write_MBps**
  Average RAW write throughput in MB/s

---

## Interpreting Results

### If `avg_good_fps` is high but `avg_saved_fps` is low
The camera/stream path is probably healthy, but disk writes or image encoding are slowing persistence.

Most likely fixes:
- use `--stats-only`
- use `--no-preview`
- use `--no-json`
- increase `--save-every`

### If both `avg_good_fps` and `avg_saved_fps` are low
The bottleneck is probably earlier in the path:

- camera configuration
- acquisition settings
- transport issues
- stream/buffer behavior
- host receive path

### If `timeouts` are high
This suggests the script is waiting for frames that are not arriving reliably.

Likely causes:
- camera is not actually free-running
- throughput is constrained upstream
- network/stream instability
- acquisition mode mismatch

### If `bad_status` or `size_mismatch` are high
This suggests frame integrity or transport problems.

Likely causes:
- camera/stream instability
- packet loss or malformed buffers
- unexpected pixel format / payload assumptions

---

## Practical Usage Patterns

## 1. Throughput baseline with no disk writes
Use this first when validating whether the acquisition path itself is healthy.

```bash
python3 capture_aravis.py \
  --camera-index 0 \
  --output stats_test \
  --duration 5 \
  --stats-only \
  --buffer-count 64 \
  --warmup-seconds 0.5 \
  --stats-interval 1.0 \
  --separate-folders
```

This should give the cleanest view of how many good frames the system can actually receive.

---

## 2. Raw-only capture
Use this when you want to persist data but avoid the heavier PNG and JSON overhead.

```bash
python3 capture_aravis.py \
  --camera-index 0 \
  --output raw_only_test \
  --duration 5 \
  --no-preview \
  --no-json \
  --buffer-count 64 \
  --warmup-seconds 0.5 \
  --stats-interval 1.0 \
  --separate-folders
```

---

## 3. Sampled capture
Use this when full-rate saving is too expensive but you still want representative saved frames.

```bash
python3 capture_aravis.py \
  --camera-index 0 \
  --output sampled_test \
  --duration 5 \
  --no-preview \
  --save-every 5 \
  --buffer-count 64 \
  --warmup-seconds 0.5 \
  --stats-interval 1.0 \
  --separate-folders
```

This captures all good frames internally, but only saves every fifth good one.

---

## 4. Dual-camera stats test
Run one command per terminal.

### Terminal A
```bash
python3 capture_aravis.py \
  --camera-index 0 \
  --output dual_stats_test \
  --duration 5 \
  --stats-only \
  --buffer-count 64 \
  --warmup-seconds 0.5 \
  --stats-interval 1.0 \
  --separate-folders
```

### Terminal B
```bash
python3 capture_aravis.py \
  --camera-index 1 \
  --output dual_stats_test \
  --duration 5 \
  --stats-only \
  --buffer-count 64 \
  --warmup-seconds 0.5 \
  --stats-interval 1.0 \
  --separate-folders
```

This is the best first simultaneous-capture test because it minimizes file I/O overhead.

---

## 5. Dual-camera raw-only test
Once stats-only is stable, test with disk writes but without preview generation.

### Terminal A
```bash
python3 capture_aravis.py \
  --camera-index 0 \
  --output dual_raw_only \
  --duration 5 \
  --no-preview \
  --no-json \
  --buffer-count 64 \
  --warmup-seconds 0.5 \
  --stats-interval 1.0 \
  --separate-folders
```

### Terminal B
```bash
python3 capture_aravis.py \
  --camera-index 1 \
  --output dual_raw_only \
  --duration 5 \
  --no-preview \
  --no-json \
  --buffer-count 64 \
  --warmup-seconds 0.5 \
  --stats-interval 1.0 \
  --separate-folders
```

---

## Recommended Test Progression

When debugging stability, use this order:

1. **Single camera, stats-only**
2. **Other camera, stats-only**
3. **Dual camera, stats-only**
4. **Single camera, raw-only**
5. **Dual camera, raw-only**
6. **Sampled save**
7. **Full output with PNG + JSON**, only if everything above is stable

This progression helps separate:
- acquisition issues
- from persistence overhead
- from dual-camera contention

---

## Known Limitations

### 1. Mono8 only
The script currently assumes the camera is producing `Mono8` frames.

If the camera is using a different pixel format, the script will fail.

### 2. Not a true video recorder
This script writes per-frame artifacts. It does **not** produce:
- MP4
- AVI
- MKV
- or chunked encoded video output

That makes it good for debugging and validation, but not ideal for final recording workflows.

### 3. PNG generation is expensive
PNG preview creation can significantly reduce effective saved FPS.

If you are debugging throughput, disable previews first:
```bash
--no-preview
```

### 4. Many small files are expensive
Saving RAW + PNG + JSON for every frame creates substantial filesystem overhead, especially at higher FPS or during multi-camera tests.

---

## When to Use This Script

Use `capture_aravis.py` when you want to answer questions like:

- Is the camera visible through Aravis?
- Are frames arriving reliably?
- Can the host keep up with one camera?
- Can the host keep up with two cameras simultaneously?
- Is file writing the bottleneck?
- Do timeouts or bad buffer statuses increase under load?

---

## When Not to Use This Script

Do not treat this as your final production recording solution if you need:

- efficient 30-second video chunks
- encoded video output
- synchronized multi-camera video recording
- long-duration storage efficiency

For those use cases, this script is best used as a diagnostic stepping stone before building or adopting a chunked recorder.

---

## Troubleshooting Tips

### Very low saved FPS
Try:

- `--stats-only`
- `--no-preview`
- `--no-json`
- `--save-every 5`

If `stats-only` is healthy but full-output mode is slow, the bottleneck is write overhead.

### Very low good buffer rate
Check:

- camera is in continuous acquisition mode
- trigger mode is off
- exposure is not limiting frame rate
- frame rate is actually configured as expected
- transport settings are sane
- host buffers are large enough

### High timeout count
This usually means buffers are not arriving as often as expected. Investigate camera-side acquisition configuration and host/stream transport health.

---

## Future Improvements

Potential next steps for this tool:

- add direct encoded video output
- support chunked recording
- support more pixel formats
- support dual-camera capture inside one process
- decouple acquisition from disk writes with a producer/consumer queue
- add CSV or summary export for repeated test runs

---

## Summary

`capture_aravis.py` is best thought of as a **camera acquisition diagnostic and benchmarking tool**.

Use it to:
- prove that frames are arriving
- quantify how well they are arriving
- separate capture-path problems from write-path problems

Then, once the transport path is stable, move on to a more purpose-built recorder for final video capture workflows.