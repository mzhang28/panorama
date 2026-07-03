#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${PROJECT_ROOT}"

OUTPUT_SVG="${1:-flamegraph.svg}"

echo "=== Building integration test binary (release-with-debuginfo) ==="
export RUSTFLAGS="-C force-frame-pointers=yes ${RUSTFLAGS:-}"

# 1. Build the integration test binary (cargo is only used here to compile)
cargo test --profile release-with-debuginfo -p panorama-server --test integration_test --no-run

# 2. Locate the compiled test binary executable
TEST_BIN="$(ls -t target/release-with-debuginfo/deps/integration_test-* 2>/dev/null | grep -v '\.d$' | head -n 1)"

if [ -z "${TEST_BIN}" ] || [ ! -x "${TEST_BIN}" ]; then
  echo "[ERROR] Could not find executable test binary in target/release-with-debuginfo/deps/" >&2
  exit 1
fi

echo "[INFO] Target test binary: ${TEST_BIN}"
echo "=== Running flamegraph tool directly ==="

# Locate standalone flamegraph tool
FLAMEGRAPH_CMD="flamegraph"
if [ -x "${HOME}/.cargo/bin/flamegraph" ]; then
  FLAMEGRAPH_CMD="${HOME}/.cargo/bin/flamegraph"
elif [ -n "${SUDO_USER:-}" ]; then
  SUDO_USER_HOME="$(getent passwd "${SUDO_USER}" | cut -d: -f6 || echo "/home/${SUDO_USER}")"
  if [ -x "${SUDO_USER_HOME}/.cargo/bin/flamegraph" ]; then
    FLAMEGRAPH_CMD="${SUDO_USER_HOME}/.cargo/bin/flamegraph"
  fi
fi

if [ "$(id -u)" -eq 0 ]; then
  echo "[INFO] Running flamegraph as root. Full kernel symbol resolution enabled."
  "${FLAMEGRAPH_CMD}" -o "${OUTPUT_SVG}" -- "${TEST_BIN}"
else
  echo "[INFO] Running flamegraph as unprivileged user."
  echo "[HINT] Run with 'sudo ${FLAMEGRAPH_CMD} -o ${OUTPUT_SVG} -- ${TEST_BIN}' for full kernel symbol resolution."
  "${FLAMEGRAPH_CMD}" -o "${OUTPUT_SVG}" -- "${TEST_BIN}"
fi

echo "[SUCCESS] Flamegraph generated: ${OUTPUT_SVG}"
