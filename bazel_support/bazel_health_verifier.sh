#!/usr/bin/env bash
set -euo pipefail

KNOWN_GETCONF_ERROR="ERROR: Failed to initialize sandbox: getconf failed"

if [[ "$(uname)" != "Darwin" ]]; then
  echo "Not macOS; skipping Bazel health check"
  exit 0
fi

check_bazel_health() {
  if bazel build //bazel_support:noop >/dev/null 2>&1; then
    return 0
  else
    return 1
  fi
}

repair_bazel() {
  bazel shutdown >/dev/null 2>&1 || {
    echo "Bazel shutdown failed; unable to repair automatically"
    return 1
  }
}

echo "Checking Bazel health..."

if check_bazel_health; then
  echo "Bazel healthy! Let's go!"
  exit 0
fi

echo "Bazel unhealthy; attempting repair..."
repair_bazel || exit 1

if check_bazel_health; then
  echo "Bazel healthy after shutdown! Let's go!"
  exit 0
else
  echo "Bazel still unhealthy!"
  echo "Known getconf error:" 
  if bazel build //bazel_support:noop 2>&1 | grep -Fxq "$KNOWN_GETCONF_ERROR"; then
    echo "$KNOWN_GETCONF_ERROR"
  else
    bazel build //bazel_support:noop 2>&1
  fi
  exit 1
fi
