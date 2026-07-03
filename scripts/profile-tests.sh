#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${PROJECT_ROOT}"

OUTPUT_SVG="${1:-flamegraph.svg}"

echo "=== Building integration test binary (release-with-debuginfo) ==="
export RUSTFLAGS="-C force-frame-pointers=yes ${RUSTFLAGS:-}"

# Build the integration test binary without running it
cargo test --profile release-with-debuginfo -p panorama-server --test integration_test --no-run

# Locate the compiled integration test executable
TEST_BIN="$(ls -t target/release-with-debuginfo/deps/integration_test-* 2>/dev/null | grep -v '\.d$' | head -n 1)"

if [ -z "${TEST_BIN}" ] || [ ! -x "${TEST_BIN}" ]; then
  echo "[ERROR] Could not find executable test binary in target/release-with-debuginfo/deps/" >&2
  exit 1
fi

echo "[INFO] Located test binary: ${TEST_BIN}"
echo "=== Running perf record ==="

# Determine perf command prefix (sudo if root or available)
PERF_PREFIX=""
if [ "$(id -u)" -eq 0 ]; then
  echo "[INFO] Running as root. Full kernel symbol resolution enabled."
elif command -v sudo &>/dev/null && [ "${USE_SUDO:-1}" -eq 1 ]; then
  echo "[INFO] Running perf with sudo for kernel symbol resolution..."
  PERF_PREFIX="sudo"
else
  echo "[INFO] Running as unprivileged user."
  echo "[HINT] Run with 'sudo ./scripts/profile-tests.sh' for kernel symbol resolution."
fi

# Run perf record directly on the binary
${PERF_PREFIX} perf record -F 997 -g -- "${TEST_BIN}"

# Ensure perf.data created by sudo is readable
if [ -n "${PERF_PREFIX}" ]; then
  sudo chmod 644 perf.data 2>/dev/null || true
fi

echo "=== Generating flamegraph SVG ==="

# Locate flamegraph tool
FLAMEGRAPH_BIN="$(command -v flamegraph || echo "${HOME}/.cargo/bin/flamegraph")"

if command -v "${FLAMEGRAPH_BIN}" &>/dev/null; then
  "${FLAMEGRAPH_BIN}" --perfdata perf.data -o "${OUTPUT_SVG}"
elif command -v perf &>/dev/null; then
  perf script | flamegraph -o "${OUTPUT_SVG}"
else
  echo "[ERROR] Could not find flamegraph tool." >&2
  exit 1
fi

echo "[SUCCESS] Flamegraph generated: ${OUTPUT_SVG}"
