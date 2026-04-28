# PR #78 — Manual on-rig test plan

End-to-end checklist for verifying the camera ingest replay pipeline + manifest
+ left/right assignment resolver on real hardware. Designed to be runnable as a
checklist; each step has a copy-pasteable command, expected output, and a
pass/fail criterion.

For the parts that don't need hardware, run `scripts/test_pr78_no_cameras.sh`
first as a smoke test — it exercises the resolver, manifest, and PGM dump
against a fabricated session. Use it to catch regressions before going to the
rig.

For the parts that do need hardware, `scripts/test_pr78.sh` walks you through
the whole flow interactively.

## Prereqs

- Both cameras connected to the LAN, PTP-capable switch in between.
- You know **physically** which camera serial is on the left side of the
  field and which is on the right. Write this down before starting — half the
  test is verifying the code agrees with reality.
- macOS / Linux with Bazel set up per `scripts/setup_mac.sh`.
- Working directory is the repo root (so the resolver finds `MODULE.bazel` and
  the optional `camera_assignment.json`).

```bash
# Identify your network interface (on macOS):
ifconfig | grep "flags=" | head -10
# Pick the one your cameras are on. Common: en0, en7, etc.
export INTERFACE=en7    # adjust to yours
```

## Step 0 — No-camera smoke test (do this first)

Catches regressions without needing the rig.

```bash
./scripts/test_pr78_no_cameras.sh
```

**Pass criteria:**
- `bazel test //backend:tests` reports 39 passed, 0 failed.
- All 4 PGM precedence tests at the end print `OK`.

If this fails, **don't proceed to the rig** — fix the regression first.

## Step 1 — Build everything

```bash
bazel build //backend:prod_real_cameras //backend:dev_real_cameras \
    //backend:record //backend:dump_first_frames //backend:tests
```

**Pass criteria:** `Build completed successfully` for all 5 targets.

## Step 2 — Verify the panic path (no manifest, no CLI, no repo config)

```bash
# Make sure no repo config exists yet
rm -f camera_assignment.json

# Run the recorder with no overrides
bazel run //backend:record -- --interface "$INTERFACE" --resolution full \
    --exposure-us 10000 --frame-rate-hz 30 \
    --output-dir /tmp/camera_out --max-frames 5 \
    --max-duration-s 5 --throwaway-duration-s 0
```

**Expected:** Panic. The error message should include:
- The two discovered camera serial IDs (`Available cameras: [...]`).
- A copy-pasteable JSON template for `camera_assignment.json`.
- The path where the file should be created.
- The CLI flag names (`--left-camera-id` / `--right-camera-id`) as an alternative.

**Pass criteria:** Panic message is shaped like the above. **Verifies:** camera
discovery, the no-config panic flow, and the resolver's instruction-emitting
fallback.

## Step 3 — Use the panic output to create the repo config

Copy the JSON template the panic emitted into `camera_assignment.json` at the
repo root. **Edit the values** to put your physically-left camera's serial in
`left_camera_id` and the physically-right one in `right_camera_id`.

```bash
# Example after editing — replace with your actual serials:
cat > camera_assignment.json <<'EOF'
{
  "left_camera_id": "Lucid_Vision_Labs-ATP124S-M-PHYSICALLY-LEFT",
  "right_camera_id": "Lucid_Vision_Labs-ATP124S-M-PHYSICALLY-RIGHT"
}
EOF

# Sanity check
cat camera_assignment.json
```

**Pass criteria:** File exists at the repo root with the correct two serials.

## Step 4 — Record a short session

```bash
bazel run //backend:record -- --interface "$INTERFACE" --resolution full \
    --exposure-us 10000 --frame-rate-hz 30 \
    --output-dir /tmp/camera_out --max-frames 100 \
    --max-duration-s 10 --throwaway-duration-s 1
```

**Expected stdout includes:**

```
camera_ingest: recording with left=<your-physically-left-id> right=<your-physically-right-id>
RECORDING COMPLETE!
```

**Verify on disk:**

```bash
SESSION=$(ls -td /tmp/camera_out/*/ | head -1)
echo "Latest session: $SESSION"
ls "$SESSION"
# Should contain: recording_session.json, <left-camera-id>/, <right-camera-id>/

cat "$SESSION/recording_session.json"
# Should match the assignment from step 3

ls "$SESSION"/*/ | head -5
# Should contain frame_0000.json + frame_0000.tiff (or .raw) etc.
```

**Pass criteria:**
- `recording_session.json` matches the repo config.
- Two per-camera subdirectories exist, each with frame metadata + payload files.
- Frame count > 0 in each subdir.

**Verifies:** repo config tier of the precedence chain, manifest write, frames
split into per-camera subdirs, recording loop terminates cleanly.

## Step 5 — Visual orientation check (the actual "HAVE TO CHECK")

This is the step that closes the original "left/right assignment is correct"
question.

```bash
SESSION=$(ls -td /tmp/camera_out/*/ | head -1)
rm -rf /tmp/camera_check && mkdir /tmp/camera_check

bazel run //backend:dump_first_frames -- \
    --footage-dir "$SESSION" \
    --output-dir /tmp/camera_check

# macOS:
open /tmp/camera_check/left_first.pgm /tmp/camera_check/right_first.pgm
# Linux:
xdg-open /tmp/camera_check/left_first.pgm
xdg-open /tmp/camera_check/right_first.pgm
```

**Suggestion:** before running, point a hand or distinctive object at one of
the cameras so the two views are visually distinguishable. Otherwise they may
look identical.

**Pass criteria:** `left_first.pgm` shows the view from the camera physically
mounted on the left side of the field. `right_first.pgm` shows the right.

**If they're swapped:** the physical mounting and the repo config disagree.
Two ways to fix — pick one:

- **Edit `camera_assignment.json`** to swap the two values, then re-record from
  step 4. Easier — no physical changes.
- **Physically swap the cables.** More work, but matches the original physical
  labeling.

After fixing, re-record and re-run dump_first_frames to confirm.

**Verifies:** the entire end-to-end orientation contract.

## Step 6 — Replay through the pipeline

```bash
SESSION=$(ls -td /tmp/camera_out/*/ | head -1)

bazel run //backend:prod_real_cameras -- --feed-footage-dir "$SESSION"
```

**Expected stdout includes:**

```
Starting real prod backend in recorded-footage replay mode from /tmp/camera_out/<timestamp>.
camera_ingest: replaying <N> recorded frame(s) from /tmp/camera_out/<timestamp> with left=<left-id> and right=<right-id>
camera_ingest: forwarded recorded frame 0 from <left-id> into left pipeline
camera_ingest: forwarded recorded frame 0 from <right-id> into right pipeline
pipeline: FieldLeft produced output frame at timestamp ...
pipeline: FieldRight produced output frame at timestamp ...
... (more output lines as frames flow through both pipelines)
```

The `replaying ... with left=X and right=Y` line should match the manifest from
step 4, which should match the repo config from step 3. **All three should
agree.**

Hit `Ctrl-C` to stop after a few seconds of pipeline output.

**Pass criteria:**
- Both `pipeline: FieldLeft` and `pipeline: FieldRight` lines appear, proving
  both pipelines actually consume frames.
- The left/right serials in the log match the manifest exactly.
- No panics.

**Verifies:** manifest read, replay routing, both pipelines actually consume
frames, all 5 stages execute (output thread is what logs the per-frame
"produced output frame" line).

## Step 7 — CLI override beats manifest

```bash
SESSION=$(ls -td /tmp/camera_out/*/ | head -1)

# Read the manifest's two camera IDs and swap them in the CLI:
LEFT=$(python3 -c "import json; print(json.load(open('$SESSION/recording_session.json'))['left_camera_id'])")
RIGHT=$(python3 -c "import json; print(json.load(open('$SESSION/recording_session.json'))['right_camera_id'])")

# Pass the manifest's RIGHT as --left-camera-id, and vice versa
bazel run //backend:prod_real_cameras -- \
    --feed-footage-dir "$SESSION" \
    --left-camera-id "$RIGHT" \
    --right-camera-id "$LEFT"
```

**Expected:** the `replaying ... with left=<X> and right=<Y>` line should now
show `left=<the manifest's right>` and `right=<the manifest's left>` —
the swap. No panic. Both pipelines still receive frames.

`Ctrl-C` to stop.

**Pass criteria:** the printed left/right are swapped vs. the manifest.

**Verifies:** CLI override precedence (tier 1) beats manifest (tier 2).

## Step 8 — Inferred-other-camera CLI

```bash
SESSION=$(ls -td /tmp/camera_out/*/ | head -1)
LEFT=$(python3 -c "import json; print(json.load(open('$SESSION/recording_session.json'))['left_camera_id'])")

# Pass only --left-camera-id; the resolver should infer right from the recorded camera IDs
bazel run //backend:prod_real_cameras -- \
    --feed-footage-dir "$SESSION" \
    --left-camera-id "$LEFT"
```

**Expected:** runs cleanly. The `replaying ... with left=<LEFT>` matches what
you passed. `right=<the other camera>` is inferred. No panic.

`Ctrl-C` to stop.

**Pass criteria:** runs without panic, both pipelines receive frames.

**Verifies:** the "exactly two cameras → infer the other" path in the resolver.

## Step 9 — Manifest validation panic

```bash
SESSION=$(ls -td /tmp/camera_out/*/ | head -1)

# Back up the real manifest
cp "$SESSION/recording_session.json" "$SESSION/recording_session.json.bak"

# Write a manifest that references a camera ID not in the recording
cat > "$SESSION/recording_session.json" <<'EOF'
{
  "left_camera_id": "bogus-camera-id",
  "right_camera_id": "another-bogus-id"
}
EOF

# Run replay; should panic
bazel run //backend:prod_real_cameras -- --feed-footage-dir "$SESSION"

# Restore the real manifest
mv "$SESSION/recording_session.json.bak" "$SESSION/recording_session.json"
```

**Expected:** Panic with a message about the manifest referencing camera IDs
not in the available set.

**Pass criteria:** panic, real manifest restored.

**Verifies:** manifest validation path.

## Step 10 — Live recorder with CLI overrides

```bash
# Read the repo config's two values and swap them via CLI
LEFT=$(python3 -c "import json; print(json.load(open('camera_assignment.json'))['left_camera_id'])")
RIGHT=$(python3 -c "import json; print(json.load(open('camera_assignment.json'))['right_camera_id'])")

# Force the recorder to swap left/right via CLI (overrides repo config)
bazel run //backend:record -- --interface "$INTERFACE" --resolution full \
    --exposure-us 10000 --frame-rate-hz 30 \
    --output-dir /tmp/camera_out_swap --max-frames 5 \
    --max-duration-s 5 --throwaway-duration-s 0 \
    --left-camera-id "$RIGHT" \
    --right-camera-id "$LEFT"
```

**Then verify:**

```bash
SWAP_SESSION=$(ls -td /tmp/camera_out_swap/*/ | head -1)
cat "$SWAP_SESSION/recording_session.json"
# Should show the swapped assignment, not the repo config

# Then dump first frames to visually confirm
rm -rf /tmp/camera_check_swap && mkdir /tmp/camera_check_swap
bazel run //backend:dump_first_frames -- \
    --footage-dir "$SWAP_SESSION" \
    --output-dir /tmp/camera_check_swap
open /tmp/camera_check_swap/left_first.pgm /tmp/camera_check_swap/right_first.pgm
```

**Pass criteria:**
- `recording_session.json` reflects the CLI override (swapped vs. step 4's manifest).
- The PGMs are visually swapped vs. step 5's PGMs (left view in right.pgm and vice versa).

**Verifies:** CLI > repo config in the recorder, and the swap actually took
effect during capture.

## Step 11 — Cleanup

```bash
rm -rf /tmp/camera_out /tmp/camera_out_swap /tmp/camera_check /tmp/camera_check_swap
# Decide whether to keep or remove camera_assignment.json
# If kept: it's per-machine and doesn't need to be committed.
```

## What to do if anything fails

| Step fails | Likely cause | Action |
|---|---|---|
| Step 2 doesn't panic | Resolver fallback is wrong | Capture stderr, check that `camera_assignment.json` doesn't exist and no CLI flags were passed |
| Step 4 prints wrong serials | Repo config not picked up | Check `pwd` is repo root, `camera_assignment.json` is well-formed JSON |
| Step 5 PGMs are swapped | Physical mounting and code labels disagree | Edit `camera_assignment.json` to swap, re-record from step 4 |
| Step 5 PGMs identical / black | Cameras not pointed at distinguishable scenes | Point one at a hand/object, re-record from step 4 |
| Step 6 missing left or right `pipeline:` lines | Replay routing broken | Capture stderr; check that the recorded frames actually contain both camera IDs (`ls $SESSION/*/`) |
| Step 6 / 7 / 8 hangs | Pipeline output thread spinning on a slow stage | Acceptable if frames are still being processed; let it run for 30s before assuming hang |
| Step 9 doesn't panic | Manifest validation broken | Capture full stderr |

## Summary checklist

When all pass, leave a comment on PR #78 with the run results:

- [ ] Step 0: `test_pr78_no_cameras.sh` clean (39 tests passed, 4 PGM precedence checks OK)
- [ ] Step 1: All 5 Bazel targets build
- [ ] Step 2: Panic with clear instructions when no config / CLI / manifest
- [ ] Step 3: Repo config created from panic template
- [ ] Step 4: Recording produces session dir + manifest + per-camera subdirs
- [ ] Step 5: PGM left/right matches physical orientation (or fix and re-verify)
- [ ] Step 6: Replay produces `pipeline: FieldLeft` and `pipeline: FieldRight` output, manifest matches
- [ ] Step 7: CLI override flips assignment vs. manifest
- [ ] Step 8: Single-flag CLI infers the other
- [ ] Step 9: Manifest with unknown camera IDs panics
- [ ] Step 10: Recorder respects CLI override; PGMs visually swapped vs. step 5
- [ ] Step 11: Cleanup
