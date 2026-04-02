Capture diagnostic frame output from LUCID cameras using Python and Aravis.

`capture_aravis.py` is a small acquisition and debugging tool for our cameras. It opens a selected camera, receives frames through Aravis, validates them, and optionally writes per-frame output to disk.

It is designed primarily for:

- camera bring-up
- acquisition debugging
- single-camera and dual-camera stability testing
- throughput measurement
- isolating capture-path issues from file I/O overhead

It is **not** a final video recorder. By default, it writes per-frame artifacts rather than an encoded video stream.

---

## Features

- Capture from a selected camera by index
- Stop by:
  - capture duration
  - maximum saved frame count
- Save per-frame:
  - RAW bytes
  - PNG preview
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

## Camera / Network Validation Checklist with Aravis 0.8 Tools

Before blaming Python, disk I/O, or Aravis bindings, first prove the camera and transport path are configured correctly.

This section focuses on the `arv-tool-0.8` commands that are useful during bring-up and throughput validation.

### 1. Verify `AcquisitionMode` is `Continuous`

Read the current value:

```bash
arv-tool-0.8 control AcquisitionMode
```

Expected result:

```text
AcquisitionMode = Continuous
```

If it is not continuous, set it:

```bash
arv-tool-0.8 control AcquisitionMode Continuous
```

Then read it back again to verify the write actually stuck:

```bash
arv-tool-0.8 control AcquisitionMode
```

### 2. Verify `TriggerMode` is `Off`

Read the current value:

```bash
arv-tool-0.8 control TriggerMode
```

Expected result:

```text
TriggerMode = Off
```

If trigger mode is enabled, streaming may stall or wait for external/software triggers instead of free-running.

Disable it:

```bash
arv-tool-0.8 control TriggerMode Off
```

Verify again:

```bash
arv-tool-0.8 control TriggerMode
```

### 3. Verify PTP is enabled

Read the enable state:

```bash
arv-tool-0.8 control PtpEnable
```

Expected result:

```text
PtpEnable = true
```

If needed, enable it:

```bash
arv-tool-0.8 control PtpEnable true
```

Then read it back again:

```bash
arv-tool-0.8 control PtpEnable
```

To validate that PTP traffic is actually present on the wire, use `tcpdump` on the camera network interface:

```bash
sudo tcpdump -i <interface> -nn udp port 319 or udp port 320
```

You should see PTP event and general messages while the cameras are connected and PTP is enabled.

### 4. Verify jumbo frames / packet size

There are two separate things to validate:

1. the camera GVSP packet size
2. the host NIC MTU

They must agree. Setting one without the other is a common half-fix.

Read the camera packet size, commonly exposed as `GevSCPSPacketSize`:

```bash
arv-tool-0.8 control GevSCPSPacketSize
```

Expected result:

```text
GevSCPSPacketSize = 9000
```

If needed, set it:

```bash
arv-tool-0.8 control GevSCPSPacketSize 9000
```

Then verify:

```bash
arv-tool-0.8 control GevSCPSPacketSize
```

### 5. Verify packet delay

Read the current inter-packet delay, commonly exposed as `GevSCPD`:

```bash
arv-tool-0.8 control GevSCPD
```

If needed, set it:

```bash
arv-tool-0.8 control GevSCPD <value>
```

Increasing inter-packet delay reduces burst pressure on the network path, but it also lowers the maximum achievable FPS. Use the smallest delay that gives stable capture.

### 6. Set the host NIC MTU to 9000

This must be done on the network interface connected to the camera or the camera network.

#### macOS

List interfaces:

```bash
networksetup -listallhardwareports
ifconfig
```

Temporarily set MTU 9000:

```bash
sudo ifconfig <interface> mtu 9000
```

Verify:

```bash
ifconfig <interface>
```

Look for `mtu 9000` in the interface output.

#### Linux

List interfaces:

```bash
ip link show
```

Temporarily set MTU 9000:

```bash
sudo ip link set dev <interface> mtu 9000
```

Verify:

```bash
ip link show dev <interface>
```

For NetworkManager-based systems, make it persistent with either the GUI or your distro’s network config. For direct `systemd-networkd` or `/etc/network/interfaces` setups, persist MTU there.

#### Windows

On Windows, set jumbo frames through the adapter GUI:

1. Open **Device Manager**
2. Expand **Network adapters**
3. Open the camera NIC’s **Properties**
4. Go to the **Advanced** tab
5. Find **Jumbo Packet** or **Jumbo Frames**
6. Set it to `9000`, `9014 Bytes`, or the closest supported value around 9000
7. Apply the change

Use the NIC status/details views or the adapter properties page to confirm the setting was applied.

### 7. Other `arv-tools-0.8` commands worth using during bring-up

These are the main Aravis 0.8 CLI tools and options that are useful for this workflow.

#### `arv-tool-0.8`

Read a feature:

```bash
arv-tool-0.8 control <FeatureName>
```

Write a feature:

```bash
arv-tool-0.8 control <FeatureName> <value>
```

Inspect GenICam data:

```bash
arv-tool-0.8 genicam
```

Useful features to inspect while tuning:

```bash
arv-tool-0.8 control AcquisitionMode
arv-tool-0.8 control TriggerMode
arv-tool-0.8 control AcquisitionFrameRate
arv-tool-0.8 control AcquisitionFrameRateEnable
arv-tool-0.8 control ExposureTime
arv-tool-0.8 control GevSCPSPacketSize
arv-tool-0.8 control GevSCPD
arv-tool-0.8 control PtpEnable
```

### 8. Set and verify frame rate

Frame rate can be limited by several things at once:

- the configured acquisition frame rate
- exposure time
- link bandwidth
- packet size / MTU mismatch
- packet delay
- host receive path issues
- disk write overhead

Read the relevant values:

```bash
arv-tool-0.8 control AcquisitionFrameRate
arv-tool-0.8 control AcquisitionFrameRateEnable
arv-tool-0.8 control ExposureTime
```

If the camera requires an enable node for manual frame-rate control, turn that on first:

```bash
arv-tool-0.8 control AcquisitionFrameRateEnable true
```

Set the frame rate:

```bash
arv-tool-0.8 control AcquisitionFrameRate 30
```

Verify:

```bash
arv-tool-0.8 control AcquisitionFrameRate
```

Make sure exposure time is short enough that it is not the real frame-rate limiter.

### 9. Prove you are not missing frames as frame rate increases

Start with the acquisition path without disk overhead:

```bash
python3 capture_aravis.py \
  --camera-index 0 \
  --output fps_validation \
  --duration 10 \
  --stats-only \
  --buffer-count 64 \
  --warmup-seconds 0.5 \
  --stats-interval 1.0
```

At each tested frame rate, you want:

- `good_buffers` increasing steadily
- `timeouts = 0` or very close to zero
- `bad_status = 0`
- `size_mismatch = 0`
- `avg_good_fps` close to the configured rate

If stats-only fails, do not trust any saved-frame result yet. Fix transport/acquisition first.

Then test persistence:

```bash
python3 capture_aravis.py \
  --camera-index 0 \
  --output fps_validation_saved \
  --duration 10 \
  --no-preview \
  --no-json \
  --buffer-count 64 \
  --warmup-seconds 0.5 \
  --stats-interval 1.0
```

For a healthy path with raw saving enabled, compare:

- `good_buffers`
- `frames_saved`
- `frames_skipped_by_sampling`
- `avg_good_fps`
- `avg_saved_fps`

If `save-every 1` is in effect and there are no write errors, then `frames_saved` should closely track `good_buffers` after warmup.

If you run for a fixed duration, expected frame count is approximately:

```text
expected_frames ≈ frame_rate × capture_seconds
```

Allow for:

- warmup discard period
- startup/shutdown edge effects
- any intentional sampling via `--save-every`

A practical check is:

```text
good_buffers ≈ expected_frames
frames_saved ≈ good_buffers          (when saving every frame)
frames_saved ≈ good_buffers / N      (when using --save-every N)
```

Inspect saved files directly:

```bash
find fps_validation_saved -name '*.raw' | wc -l
find fps_validation_saved -name '*.png' | wc -l
find fps_validation_saved -name '*.json' | wc -l
```

These counts should agree with the script summary for the enabled outputs.

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
