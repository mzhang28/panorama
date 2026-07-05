#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

mkdir -p dist/panoapp
WASM_DIR="target/wasm32-wasip1/release"

# Step 1: Build plugin UIs (Module Federation remotes)
echo "--- Building plugin UIs ---"
for crate in journal wakatime grafana trips beli subsonic files; do
  UI_DIR="crates/panorama-app-${crate}/ui"
  if [ -f "$UI_DIR/package.json" ] && [ -f "$UI_DIR/vite.config.ts" ]; then
    echo "  Building UI for ${crate}..."
    (cd "$UI_DIR" && bun install --silent && bun run build)
  fi
done

# Step 2: Package .panoapp files (manifest + WASM + UI)
echo "--- Packaging .panoapp files ---"
for crate in journal wakatime grafana trips beli subsonic files; do
  MANIFEST="crates/panorama-app-${crate}/manifest.json"
  WASM="${WASM_DIR}/${crate}.wasm"
  UI_DIST="crates/panorama-app-${crate}/ui/dist"
  [ -f "$MANIFEST" ] || continue

  UI_ARG=""
  [ -d "$UI_DIST" ] && UI_ARG="--ui-dir $UI_DIST"

  bun scripts/package-panoapp.ts \
    "$MANIFEST" \
    "$WASM" \
    "dist/panoapp" \
    $UI_ARG
done

echo "Done: $(ls dist/panoapp/*.panoapp 2>/dev/null | wc -l) .panoapp files"
