#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${PROJECT_ROOT}"

OUTPUT_SVG="${1:-flamegraph.svg}"

echo "=== Profiling Rust Integration Tests ==="

# Force frame pointers in Rust release build so stacks are accurately recorded
export RUSTFLAGS="-C force-frame-pointers=yes ${RUSTFLAGS:-}"

FLAMEGRAPH_ARGS=(
  "--profile" "release-with-debuginfo"
  "-p" "panorama-server"
  "--test" "integration_test"
  "-o" "${OUTPUT_SVG}"
)

# If running as root (or via sudo), enable cargo flamegraph --root flag for kernel symbol access
if [ "$(id -u)" -eq 0 ]; then
  echo "[INFO] Running as root (sudo). Kernel symbols and un-restricted perf events will be included."
  cargo flamegraph --root "${FLAMEGRAPH_ARGS[@]}"
else
  echo "[INFO] Running as unprivileged user. User-space symbols will be included."
  echo "[HINT] Run with 'sudo ./scripts/profile-tests.sh' or 'sudo -E ./scripts/profile-tests.sh' for kernel symbol resolution."
  cargo flamegraph "${FLAMEGRAPH_ARGS[@]}"
fi

echo "[SUCCESS] Flamegraph generated: ${OUTPUT_SVG}"
