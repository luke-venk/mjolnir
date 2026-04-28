#!/usr/bin/env bash
# Smoke test for PR #78 (camera ingest replay + manifest + assignment resolver)
# that DOES NOT need real cameras.
#
# Builds all targets, runs unit tests, then fabricates a tiny 2-camera session
# and exercises every tier of the resolver precedence chain via the
# dump_first_frames binary, asserting on the output PGM bytes.
#
# Run from the repo root. Exits non-zero on any failure.
#
# This script is the prereq for ./scripts/test_pr78.sh — if it doesn't pass,
# don't bother going to the rig.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# Workspace for fabricated test data.
WORKDIR="$(mktemp -d -t pr78-no-cameras-XXXXXX)"
trap 'rm -rf "$WORKDIR"; rm -f "$REPO_ROOT/camera_assignment.json.bak.pr78test"' EXIT

# If a real camera_assignment.json exists at the repo root, set it aside so it
# doesn't bleed into the panic test in tier 4.
if [[ -f "$REPO_ROOT/camera_assignment.json" ]]; then
    mv "$REPO_ROOT/camera_assignment.json" "$REPO_ROOT/camera_assignment.json.bak.pr78test"
    RESTORE_REPO_CONFIG=1
else
    RESTORE_REPO_CONFIG=0
fi
restore_repo_config() {
    if [[ "$RESTORE_REPO_CONFIG" == "1" && -f "$REPO_ROOT/camera_assignment.json.bak.pr78test" ]]; then
        mv "$REPO_ROOT/camera_assignment.json.bak.pr78test" "$REPO_ROOT/camera_assignment.json"
    fi
}
trap 'rm -rf "$WORKDIR"; restore_repo_config' EXIT

red()    { printf '\033[0;31m%s\033[0m\n' "$*"; }
green()  { printf '\033[0;32m%s\033[0m\n' "$*"; }
yellow() { printf '\033[0;33m%s\033[0m\n' "$*"; }
bold()   { printf '\033[1m%s\033[0m\n' "$*"; }

step() { echo; bold "=== $* ==="; }
fail() { red "FAIL: $*"; exit 1; }
pass() { green "OK: $*"; }

# --- Build ---
step "Build: prod_real_cameras / dev_real_cameras / record / dump_first_frames / tests"
bazel build \
    //backend:prod_real_cameras \
    //backend:dev_real_cameras \
    //backend:record \
    //backend:dump_first_frames \
    //backend:tests
pass "all targets built"

# --- Unit tests ---
step "Unit tests"
bazel test //backend:tests --test_output=errors
pass "unit tests passed"

# --- Fabricate a 2-camera session ---
# Camera-X has all 0xc8 (200) pixels; camera-Y has all 0x32 (50) pixels.
# Manifest claims left=camera-Y, right=camera-X (intentionally NOT alphabetical
# so we can tell whether the manifest is being used vs an alphabetical fallback).
step "Fabricating fake session at $WORKDIR/session"
SESSION="$WORKDIR/session"
mkdir -p "$SESSION/camera-X" "$SESSION/camera-Y"
cat > "$SESSION/recording_session.json" <<'EOF'
{
  "left_camera_id": "camera-Y",
  "right_camera_id": "camera-X"
}
EOF
printf '\xc8\xc8\xc8\xc8' > "$SESSION/camera-X/frame_0000.raw"
cat > "$SESSION/camera-X/frame_0000.json" <<'EOF'
{
  "camera_id": "camera-X",
  "frame_index": 0,
  "width": 2,
  "height": 2,
  "payload_bytes": 4,
  "system_timestamp_ns": 100,
  "buffer_timestamp_ns": 50,
  "frame_id": 1
}
EOF
printf '\x32\x32\x32\x32' > "$SESSION/camera-Y/frame_0000.raw"
cat > "$SESSION/camera-Y/frame_0000.json" <<'EOF'
{
  "camera_id": "camera-Y",
  "frame_index": 0,
  "width": 2,
  "height": 2,
  "payload_bytes": 4,
  "system_timestamp_ns": 110,
  "buffer_timestamp_ns": 60,
  "frame_id": 2
}
EOF
pass "session ready"

# --- Helper: read the last byte of a PGM file ---
last_byte_hex() {
    # PGM (P5) has a small text header; the payload is the trailing bytes.
    # We just want the last byte for our 4-byte uniform images.
    local file="$1"
    xxd -s -1 -l 1 -p "$file"
}

# --- Tier 2: manifest (no CLI, no repo config) ---
step "Tier 2: manifest precedence"
OUT="$WORKDIR/out_manifest"
mkdir -p "$OUT"
bazel-bin/backend/dump_first_frames \
    --footage-dir "$SESSION" \
    --output-dir "$OUT" \
    > "$WORKDIR/log_manifest.txt" 2>&1
echo "  log: $WORKDIR/log_manifest.txt"
grep "resolved left=camera-Y right=camera-X" "$WORKDIR/log_manifest.txt" \
    || fail "expected 'resolved left=camera-Y right=camera-X' in manifest tier"
[[ "$(last_byte_hex "$OUT/left_first.pgm")" == "32" ]] \
    || fail "expected left_first.pgm payload to be 0x32 (camera-Y) in manifest tier"
[[ "$(last_byte_hex "$OUT/right_first.pgm")" == "c8" ]] \
    || fail "expected right_first.pgm payload to be 0xc8 (camera-X) in manifest tier"
pass "manifest tier produces left=camera-Y right=camera-X"

# --- Tier 1: CLI override beats manifest ---
step "Tier 1: CLI override beats manifest"
OUT="$WORKDIR/out_cli"
mkdir -p "$OUT"
bazel-bin/backend/dump_first_frames \
    --footage-dir "$SESSION" \
    --output-dir "$OUT" \
    --left-camera-id camera-X \
    --right-camera-id camera-Y \
    > "$WORKDIR/log_cli.txt" 2>&1
grep "resolved left=camera-X right=camera-Y" "$WORKDIR/log_cli.txt" \
    || fail "expected CLI override flip in CLI tier"
[[ "$(last_byte_hex "$OUT/left_first.pgm")" == "c8" ]] \
    || fail "expected left_first.pgm to be camera-X (0xc8) under CLI override"
[[ "$(last_byte_hex "$OUT/right_first.pgm")" == "32" ]] \
    || fail "expected right_first.pgm to be camera-Y (0x32) under CLI override"
pass "CLI override beats manifest"

# --- Tier 1 (single-flag inferred): --left only ---
step "Tier 1 (single-flag): only --left-camera-id, infer right"
OUT="$WORKDIR/out_cli_single"
mkdir -p "$OUT"
bazel-bin/backend/dump_first_frames \
    --footage-dir "$SESSION" \
    --output-dir "$OUT" \
    --left-camera-id camera-X \
    > "$WORKDIR/log_cli_single.txt" 2>&1
grep "resolved left=camera-X right=camera-Y" "$WORKDIR/log_cli_single.txt" \
    || fail "expected inferred right=camera-Y when only --left-camera-id given"
pass "single-flag CLI infers the other camera"

# --- Tier 3: repo config ---
step "Tier 3: repo config beats alphabetical fallback"
# Remove manifest so manifest tier doesn't fire
rm "$SESSION/recording_session.json"
# Write a repo config that picks camera-X as left (NOT alphabetical)
cat > "$REPO_ROOT/camera_assignment.json" <<'EOF'
{
  "left_camera_id": "camera-X",
  "right_camera_id": "camera-Y"
}
EOF
OUT="$WORKDIR/out_repo"
mkdir -p "$OUT"
bazel-bin/backend/dump_first_frames \
    --footage-dir "$SESSION" \
    --output-dir "$OUT" \
    > "$WORKDIR/log_repo.txt" 2>&1
grep "resolved left=camera-X right=camera-Y" "$WORKDIR/log_repo.txt" \
    || fail "expected repo config to pick left=camera-X"
[[ "$(last_byte_hex "$OUT/left_first.pgm")" == "c8" ]] \
    || fail "expected left_first.pgm to be camera-X (0xc8) under repo config"
rm "$REPO_ROOT/camera_assignment.json"
pass "repo config tier works"

# --- Tier 4: panic with instructions ---
step "Tier 4: panic when no manifest, no CLI, no repo config"
OUT="$WORKDIR/out_panic"
mkdir -p "$OUT"
# CD elsewhere so the resolver can't walk up to find MODULE.bazel and any
# stray repo config there. Use an absolute path to the dump_first_frames bin.
DUMP_BIN="$REPO_ROOT/bazel-bin/backend/dump_first_frames"
set +e
(cd "$WORKDIR" && "$DUMP_BIN" --footage-dir "$SESSION" --output-dir "$OUT" > "$WORKDIR/log_panic.txt" 2>&1)
PANIC_EXIT=$?
set -e
[[ "$PANIC_EXIT" -ne 0 ]] || fail "expected non-zero exit from panic path"
grep "No camera assignment found" "$WORKDIR/log_panic.txt" \
    || fail "panic message did not mention 'No camera assignment found'"
grep "camera-X" "$WORKDIR/log_panic.txt" \
    || fail "panic message did not list discovered cameras"
grep -- "--left-camera-id" "$WORKDIR/log_panic.txt" \
    || fail "panic message did not suggest CLI flags"
pass "panic emits clear setup instructions"

# --- Summary ---
step "Summary"
green "All hardware-free smoke checks passed."
yellow "Next: run ./scripts/test_pr78.sh on the rig to exercise the live recording + replay path."
