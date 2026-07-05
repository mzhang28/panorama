#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p data/plugins
cp dist/panoapp/*.panoapp data/plugins/ 2>/dev/null || true
MODE="${1:-debug}"
if [ "$MODE" = "release" ]; then
  echo "Starting Panorama server (release) on http://localhost:3000"
  PANORAMA_DATA_DIR=./data PANORAMA_LISTEN=127.0.0.1:3000 cargo run --release -p panorama-server
else
  echo "Starting Panorama server (debug) on http://localhost:3000"
  PANORAMA_DATA_DIR=./data PANORAMA_LISTEN=127.0.0.1:3000 cargo run -p panorama-server
fi
