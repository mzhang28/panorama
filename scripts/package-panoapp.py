#!/usr/bin/env python3
"""Package a single .panoapp from manifest + WASM + optional UI build output."""
import zipfile, json, os, sys, argparse

parser = argparse.ArgumentParser(description='Package a .panoapp file')
parser.add_argument('manifest', help='Path to manifest.json')
parser.add_argument('wasm', nargs='?', default=None, help='Path to WASM module')
parser.add_argument('out_dir', help='Output directory')
parser.add_argument('--ui-dir', default=None, help='Directory containing built UI files')

args = parser.parse_args()

with open(args.manifest) as f:
    manifest = json.load(f)

out_path = os.path.join(args.out_dir, manifest['id'] + '.panoapp')
with zipfile.ZipFile(out_path, 'w', zipfile.ZIP_DEFLATED) as zf:
    zf.writestr('manifest.json', json.dumps(manifest))
    if args.wasm and os.path.exists(args.wasm):
        zf.write(args.wasm, 'plugin.wasm')
    # Include UI files under ui/ prefix
    if args.ui_dir and os.path.isdir(args.ui_dir):
        for root, _dirs, files in os.walk(args.ui_dir):
            for f in files:
                full = os.path.join(root, f)
                rel = os.path.relpath(full, args.ui_dir)
                zf.write(full, f'ui/{rel}')

print(f'  {manifest["id"]}.panoapp ({os.path.getsize(out_path)} bytes)')
