#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${PROJECT_ROOT}"

OUTPUT_SVG="${1:-flamegraph.svg}"

# Find the most recently built integration test binary executable
TEST_BIN="$(ls -t target/*/deps/integration_test-* 2>/dev/null | grep -v '\.d$' | head -n 1)"

if [ -z "${TEST_BIN}" ] || [ ! -x "${TEST_BIN}" ]; then
  echo "[ERROR] Could not find executable integration test binary in target/ directories." >&2
  exit 1
fi

echo "[INFO] Profiling binary: ${TEST_BIN}"
echo "=== Running flamegraph ==="

FLAMEGRAPH_BIN="$(command -v flamegraph || find "${HOME}" -name flamegraph -type f -executable 2>/dev/null | head -n 1)"

if [ -z "${FLAMEGRAPH_BIN}" ]; then
  echo "[ERROR] 'flamegraph' executable not found in PATH." >&2
  exit 1
fi

if [ "$(id -u)" -eq 0 ]; then
  echo "[INFO] Running as root. Full kernel symbol resolution enabled."
  "${FLAMEGRAPH_BIN}" -o "${OUTPUT_SVG}" -- "${TEST_BIN}"
else
  echo "[INFO] Running as unprivileged user."
  echo "[HINT] Run with 'sudo ${FLAMEGRAPH_BIN} -o ${OUTPUT_SVG} -- ${TEST_BIN}' for full kernel symbol resolution."
  "${FLAMEGRAPH_BIN}" -o "${OUTPUT_SVG}" -- "${TEST_BIN}"
fi

echo "[SUCCESS] Flamegraph generated: ${OUTPUT_SVG}"
