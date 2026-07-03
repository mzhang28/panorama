#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
rustup target add wasm32-wasip1 2>/dev/null || true

# Each app crate compiles its Plugin trait implementation to WASM.
# The binary is named after the app (journal, wakatime, etc.) and
# produced at target/wasm32-wasip1/release/{app}.wasm
for app in journal wakatime grafana trips beli subsonic files; do
  echo "  Building WASM for ${app}..."
  RUSTFLAGS="-C link-arg=--allow-undefined" cargo build --release -p "panorama-app-${app}" --target wasm32-wasip1
done

echo "WASM modules built."
