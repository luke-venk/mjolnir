#!/usr/bin/env bash
# Interactive on-rig driver for PR #78 (camera ingest replay + manifest +
# assignment resolver).
#
# Walks through every step of PR-78-MANUAL-TEST.md, prompting you between
# steps. Captures stdout/stderr from each command into a log directory so you
# can attach the full trace to the PR.
#
# Run from the repo root. Prereqs:
# - ./scripts/test_pr78_no_cameras.sh has already passed.
# - Both cameras connected to the LAN, PTP-capable switch in between.
# - You know the physical-left and physical-right serials.
# - INTERFACE env var set, e.g. `export INTERFACE=en7`.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

LOG_DIR="$REPO_ROOT/.pr78-test-logs/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$LOG_DIR"

red()    { printf '\033[0;31m%s\033[0m\n' "$*"; }
green()  { printf '\033[0;32m%s\033[0m\n' "$*"; }
yellow() { printf '\033[0;33m%s\033[0m\n' "$*"; }
bold()   { printf '\033[1m%s\033[0m\n' "$*"; }

step() { echo; bold "=== $* ==="; }
prompt() {
    local msg="$1"
    echo
    yellow "$msg"
    read -r -p "Press Enter to continue, or Ctrl-C to abort: " _
}
ask_pass() {
    local msg="$1"
    local resp
    while true; do
        echo
        yellow "$msg"
        read -r -p "[p]ass / [f]ail / [s]kip: " resp
        case "$resp" in
            p|P) return 0 ;;
            f|F) red "Marked as FAIL. Stopping."; exit 1 ;;
            s|S) yellow "Skipped."; return 0 ;;
        esac
    done
}

# --- Sanity checks ---
if [[ -z "${INTERFACE:-}" ]]; then
    red "INTERFACE env var not set."
    yellow "Find your camera interface with: ifconfig | grep flags="
    yellow "Then: export INTERFACE=<your-interface>  (e.g. en7)"
    exit 1
fi
green "INTERFACE=$INTERFACE"
green "Logs will be written to: $LOG_DIR"

# --- Step 1: build ---
step "Step 1 — Build all targets"
bazel build \
    //backend:prod_real_cameras \
    //backend:dev_real_cameras \
    //backend:record \
    //backend:dump_first_frames \
    //backend:tests 2>&1 | tee "$LOG_DIR/01-build.log"
green "Build OK."

# --- Step 2: panic path (no config) ---
step "Step 2 — Panic when no manifest, no CLI, no repo config"
if [[ -f "$REPO_ROOT/camera_assignment.json" ]]; then
    yellow "camera_assignment.json exists; moving aside for the panic test."
    mv "$REPO_ROOT/camera_assignment.json" "$REPO_ROOT/camera_assignment.json.bak.test"
fi

set +e
bazel run //backend:record -- --interface "$INTERFACE" --resolution full \
    --exposure-us 10000 --frame-rate-hz 30 \
    --output-dir /tmp/camera_out --max-frames 5 \
    --max-duration-s 5 --throwaway-duration-s 0 \
    2>&1 | tee "$LOG_DIR/02-panic.log"
PANIC_EXIT=${PIPESTATUS[0]}
set -e
if [[ "$PANIC_EXIT" -eq 0 ]]; then
    red "Recorder did NOT panic. That's a regression."
    exit 1
fi
if ! grep -q "No camera assignment found" "$LOG_DIR/02-panic.log"; then
    red "Panic happened but the message didn't include 'No camera assignment found'. Check the log."
    exit 1
fi
green "Panic emitted setup instructions as expected."

# --- Step 3: repo config from panic output ---
step "Step 3 — Create repo config from panic output"
echo
echo "The panic above should have suggested a JSON template like this:"
echo
grep -A 5 '"left_camera_id"' "$LOG_DIR/02-panic.log" | head -10 || true
echo
yellow "Now write camera_assignment.json yourself, putting your physically-LEFT camera"
yellow "in left_camera_id and physically-RIGHT in right_camera_id."
yellow ""
yellow "Use a separate terminal:"
yellow "  cat > camera_assignment.json <<'EOF'"
yellow "  {"
yellow "    \"left_camera_id\": \"<your-physical-left-serial>\","
yellow "    \"right_camera_id\": \"<your-physical-right-serial>\""
yellow "  }"
yellow "  EOF"
prompt "Done editing? Then continue."

if [[ ! -f "$REPO_ROOT/camera_assignment.json" ]]; then
    red "camera_assignment.json not found. Aborting."
    exit 1
fi
cp "$REPO_ROOT/camera_assignment.json" "$LOG_DIR/03-repo-config.json"
green "Repo config saved to $LOG_DIR/03-repo-config.json"

# --- Step 4: record ---
step "Step 4 — Record a 100-frame session"
bazel run //backend:record -- --interface "$INTERFACE" --resolution full \
    --exposure-us 10000 --frame-rate-hz 30 \
    --output-dir /tmp/camera_out --max-frames 100 \
    --max-duration-s 10 --throwaway-duration-s 1 \
    2>&1 | tee "$LOG_DIR/04-record.log"
SESSION="$(ls -td /tmp/camera_out/*/ | head -1)"
SESSION="${SESSION%/}"
green "Session: $SESSION"

if ! grep -q "RECORDING COMPLETE" "$LOG_DIR/04-record.log"; then
    red "Recording did not complete cleanly."
    exit 1
fi
if [[ ! -f "$SESSION/recording_session.json" ]]; then
    red "$SESSION/recording_session.json missing."
    exit 1
fi
cp "$SESSION/recording_session.json" "$LOG_DIR/04-manifest.json"
echo
echo "Manifest contents:"
cat "$SESSION/recording_session.json"
echo
echo "Per-camera frame counts:"
for d in "$SESSION"/*/; do
    n=$(ls "$d"*.json 2>/dev/null | wc -l | tr -d ' ')
    echo "  $(basename "$d"): $n frame(s)"
done
green "Step 4 looks OK."

# --- Step 5: visual orientation ---
step "Step 5 — Visual orientation check (the actual 'HAVE TO CHECK')"
yellow "Tip: before continuing, point a hand or distinctive object at ONE of"
yellow "the cameras so the two views are visually distinguishable."
prompt "Ready to dump first frames?"

mkdir -p /tmp/camera_check
bazel run //backend:dump_first_frames -- \
    --footage-dir "$SESSION" \
    --output-dir /tmp/camera_check \
    2>&1 | tee "$LOG_DIR/05-dump.log"

if [[ "$(uname)" == "Darwin" ]]; then
    open /tmp/camera_check/left_first.pgm /tmp/camera_check/right_first.pgm
else
    yellow "Open these manually:"
    yellow "  /tmp/camera_check/left_first.pgm"
    yellow "  /tmp/camera_check/right_first.pgm"
fi
cp /tmp/camera_check/left_first.pgm  "$LOG_DIR/05-left_first.pgm"
cp /tmp/camera_check/right_first.pgm "$LOG_DIR/05-right_first.pgm"

ask_pass "Does left_first.pgm match the camera physically mounted on the LEFT side?"

# --- Step 6: replay ---
step "Step 6 — Replay through the pipeline"
yellow "We'll start prod_real_cameras in replay mode for ~10 seconds, then Ctrl-C automatically."
prompt "Ready?"

set +e
timeout 15 bazel run //backend:prod_real_cameras -- --feed-footage-dir "$SESSION" \
    > "$LOG_DIR/06-replay.log" 2>&1
set -e

echo "Last 30 lines of replay output:"
tail -30 "$LOG_DIR/06-replay.log"

if ! grep -q "pipeline: FieldLeft produced output frame" "$LOG_DIR/06-replay.log"; then
    red "FieldLeft pipeline never produced a frame. Replay routing broken?"
    exit 1
fi
if ! grep -q "pipeline: FieldRight produced output frame" "$LOG_DIR/06-replay.log"; then
    red "FieldRight pipeline never produced a frame. Replay routing broken?"
    exit 1
fi
green "Both pipelines received frames."

# Sanity: replay's 'left=' line should match the manifest
MANIFEST_LEFT=$(python3 -c "import json; print(json.load(open('$SESSION/recording_session.json'))['left_camera_id'])")
MANIFEST_RIGHT=$(python3 -c "import json; print(json.load(open('$SESSION/recording_session.json'))['right_camera_id'])")
if ! grep -q "left=${MANIFEST_LEFT} and right=${MANIFEST_RIGHT}" "$LOG_DIR/06-replay.log"; then
    red "Replay's left/right doesn't match the manifest. Check $LOG_DIR/06-replay.log"
    exit 1
fi
green "Replay assignment matches manifest."

# --- Step 7: CLI override beats manifest ---
step "Step 7 — CLI override beats manifest (deliberately swap)"
prompt "About to run replay with --left/--right swapped vs manifest."
set +e
timeout 10 bazel run //backend:prod_real_cameras -- \
    --feed-footage-dir "$SESSION" \
    --left-camera-id "$MANIFEST_RIGHT" \
    --right-camera-id "$MANIFEST_LEFT" \
    > "$LOG_DIR/07-cli-override.log" 2>&1
set -e

if ! grep -q "left=${MANIFEST_RIGHT} and right=${MANIFEST_LEFT}" "$LOG_DIR/07-cli-override.log"; then
    red "CLI override did NOT take effect. Check $LOG_DIR/07-cli-override.log"
    exit 1
fi
green "CLI override beats manifest as expected."

# --- Step 8: single-flag CLI infers other ---
step "Step 8 — Single-flag CLI infers the other camera"
set +e
timeout 10 bazel run //backend:prod_real_cameras -- \
    --feed-footage-dir "$SESSION" \
    --left-camera-id "$MANIFEST_LEFT" \
    > "$LOG_DIR/08-single-flag.log" 2>&1
set -e

if ! grep -q "left=${MANIFEST_LEFT} and right=${MANIFEST_RIGHT}" "$LOG_DIR/08-single-flag.log"; then
    red "Single-flag inference did NOT pick the other camera. Check $LOG_DIR/08-single-flag.log"
    exit 1
fi
green "Single-flag CLI inferred the other camera correctly."

# --- Step 9: manifest validation panic ---
step "Step 9 — Manifest validation panic on unknown camera IDs"
cp "$SESSION/recording_session.json" "$SESSION/recording_session.json.bak"
cat > "$SESSION/recording_session.json" <<'EOF'
{
  "left_camera_id": "bogus-camera-id",
  "right_camera_id": "another-bogus-id"
}
EOF

set +e
timeout 10 bazel run //backend:prod_real_cameras -- --feed-footage-dir "$SESSION" \
    > "$LOG_DIR/09-bad-manifest.log" 2>&1
PANIC_EXIT=$?
set -e

# Restore real manifest
mv "$SESSION/recording_session.json.bak" "$SESSION/recording_session.json"

if [[ "$PANIC_EXIT" -eq 0 ]]; then
    red "Replay didn't panic on bad manifest. That's a regression."
    exit 1
fi
green "Manifest validation panic fired as expected."

# --- Step 10: recorder respects CLI override ---
step "Step 10 — Recorder respects CLI override (swapped)"
prompt "About to record again with --left/--right swapped vs your repo config. Same scene OK."

bazel run //backend:record -- --interface "$INTERFACE" --resolution full \
    --exposure-us 10000 --frame-rate-hz 30 \
    --output-dir /tmp/camera_out_swap --max-frames 30 \
    --max-duration-s 5 --throwaway-duration-s 0 \
    --left-camera-id "$MANIFEST_RIGHT" \
    --right-camera-id "$MANIFEST_LEFT" \
    2>&1 | tee "$LOG_DIR/10-record-swap.log"

SWAP_SESSION="$(ls -td /tmp/camera_out_swap/*/ | head -1)"
SWAP_SESSION="${SWAP_SESSION%/}"
cp "$SWAP_SESSION/recording_session.json" "$LOG_DIR/10-manifest-swap.json"

SWAP_LEFT=$(python3 -c "import json; print(json.load(open('$SWAP_SESSION/recording_session.json'))['left_camera_id'])")
if [[ "$SWAP_LEFT" != "$MANIFEST_RIGHT" ]]; then
    red "Swap manifest's left=$SWAP_LEFT, expected $MANIFEST_RIGHT. CLI override didn't reach the manifest."
    exit 1
fi
green "Recorder honored the CLI swap."

# Visual confirmation: dump first frames from the swapped session
mkdir -p /tmp/camera_check_swap
bazel run //backend:dump_first_frames -- \
    --footage-dir "$SWAP_SESSION" \
    --output-dir /tmp/camera_check_swap \
    >> "$LOG_DIR/10-dump-swap.log" 2>&1

if [[ "$(uname)" == "Darwin" ]]; then
    open /tmp/camera_check/left_first.pgm  /tmp/camera_check_swap/left_first.pgm
    open /tmp/camera_check/right_first.pgm /tmp/camera_check_swap/right_first.pgm
fi
cp /tmp/camera_check_swap/left_first.pgm  "$LOG_DIR/10-left_first.pgm"
cp /tmp/camera_check_swap/right_first.pgm "$LOG_DIR/10-right_first.pgm"

ask_pass "Comparing step-5 PGMs vs step-10 PGMs: are step-10's left/right visually swapped vs step-5's?"

# --- Step 11: cleanup ---
step "Step 11 — Cleanup"
echo "Removing /tmp/camera_out, /tmp/camera_out_swap, /tmp/camera_check, /tmp/camera_check_swap"
rm -rf /tmp/camera_out /tmp/camera_out_swap /tmp/camera_check /tmp/camera_check_swap
echo
yellow "camera_assignment.json is preserved at the repo root (per-machine, not committed)."
yellow "If you moved an existing one aside earlier, it's at camera_assignment.json.bak.test."

# --- Summary ---
step "Summary"
green "All on-rig steps passed."
echo
yellow "Logs and artifacts saved to: $LOG_DIR"
yellow "Suggest attaching the directory contents to PR #78 as evidence the test plan ran clean."
