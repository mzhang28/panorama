#!/usr/bin/env python3
"""Package a single .panoapp from manifest + WASM."""
import zipfile, json, os, sys

m_path = sys.argv[1]
wasm_path = sys.argv[2]
out_dir = sys.argv[3]

with open(m_path) as f:
    manifest = json.load(f)

panoapp = os.path.join(out_dir, manifest['id'] + '.panoapp')
with zipfile.ZipFile(panoapp, 'w', zipfile.ZIP_DEFLATED) as zf:
    zf.writestr('manifest.json', json.dumps(manifest))
    if os.path.exists(wasm_path):
        zf.write(wasm_path, 'plugin.wasm')

print(f'  {manifest["id"]}.panoapp ({os.path.getsize(panoapp)} bytes)')
