#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${PROJECT_ROOT}"

# Ensure Cargo binaries (~/.cargo/bin) are in PATH (needed when run via sudo)
if [ -n "${SUDO_USER:-}" ]; then
  SUDO_USER_HOME="$(getent passwd "${SUDO_USER}" | cut -d: -f6 || echo "/home/${SUDO_USER}")"
  if [ -d "${SUDO_USER_HOME}/.cargo/bin" ]; then
    export PATH="${SUDO_USER_HOME}/.cargo/bin:${PATH}"
  fi
fi
if [ -d "${HOME}/.cargo/bin" ]; then
  export PATH="${HOME}/.cargo/bin:${PATH}"
fi

if ! command -v cargo &>/dev/null; then
  echo "[ERROR] 'cargo' command not found in PATH (${PATH}). Please ensure Rust is installed." >&2
  exit 1
fi

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

# If running as root (or via sudo), pass --root flag for kernel symbol access
if [ "$(id -u)" -eq 0 ]; then
  echo "[INFO] Running as root (sudo). Kernel symbols and un-restricted perf events will be included."
  cargo flamegraph --root "${FLAMEGRAPH_ARGS[@]}"
else
  echo "[INFO] Running as unprivileged user. User-space symbols will be included."
  echo "[HINT] Run with 'sudo ./scripts/profile-tests.sh' for kernel symbol resolution."
  cargo flamegraph "${FLAMEGRAPH_ARGS[@]}"
fi

echo "[SUCCESS] Flamegraph generated: ${OUTPUT_SVG}"
