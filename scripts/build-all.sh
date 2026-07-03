#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
echo "=== Building Panorama ==="
echo ""
echo "--- Building WASM plugins ---"
bash scripts/build-wasm.sh
echo ""
echo "--- Packaging .panoapp files ---"
bash scripts/build-panoapp.sh
echo ""
echo "--- Building server ---"
cargo build --release -p panorama-server
echo ""
echo "=== Build complete ==="
echo "Run 'just serve' to start the server."
