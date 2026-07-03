#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
rustup target add wasm32-wasip1 2>/dev/null || true
cargo build --release -p wasm-plugins --target wasm32-wasip1
echo "WASM modules built."
