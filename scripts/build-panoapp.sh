#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

mkdir -p dist/panoapp
WASM_DIR="target/wasm32-wasip1/release"

for crate in journal wakatime grafana trips beli subsonic files; do
  MANIFEST="crates/panorama-app-${crate}/manifest.json"
  WASM="${WASM_DIR}/${crate}.wasm"
  [ -f "$MANIFEST" ] || continue
  
  APP_ID=$(python3 -c "import json; print(json.load(open('$MANIFEST'))['id'])")
  PANOAPP="dist/panoapp/${APP_ID}.panoapp"
  
  python3 -c "
import zipfile, json, os
with open('${MANIFEST}') as f: manifest = json.load(f)
with zipfile.ZipFile('${PANOAPP}', 'w', zipfile.ZIP_DEFLATED) as zf:
    zf.writestr('manifest.json', json.dumps(manifest, indent=2))
    if os.path.exists('${WASM}'): zf.write('${WASM}', 'plugin.wasm')
print(f'  ${PANOAPP} ({os.path.getsize(\"${PANOAPP}\")} bytes)')
"
done
echo "Done: $(ls dist/panoapp/*.panoapp 2>/dev/null | wc -l) .panoapp files"
