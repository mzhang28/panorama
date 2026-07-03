#!/usr/bin/env bash
# ── Panorama E2E Test Harness ────────────────────────────────────────────────
#
# Builds all plugin UIs, the frontend, and the server (which embeds the
# frontend via rust-embed), then runs Playwright against an isolated instance
# with a temporary data directory on random ports.  Nothing else needs to
# be running — no dev servers, no Docker.
#
# Usage:
#   bash scripts/e2e-harness.sh              # full build + test
#   bash scripts/e2e-harness.sh --no-build   # skip build (use pre-built)
#   bash scripts/e2e-harness.sh -g "Journal" # filter tests
#
set -euo pipefail
cd "$(dirname "$0")/.."

# ── Parse flags ──────────────────────────────────────────────────────────────

BUILD=true
PLAYWRIGHT_ARGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-build) BUILD=false; shift ;;
    *) PLAYWRIGHT_ARGS+=("$1"); shift ;;
  esac
done

# ── Helpers ──────────────────────────────────────────────────────────────────

find_free_port() {
  python3 -c "import socket; s=socket.socket(); s.bind(('127.0.0.1', 0)); print(s.getsockname()[1]); s.close()"
}

wait_for_url() {
  local url="$1" attempts="$2" desc="$3"
  for _ in $(seq 1 "$attempts"); do
    if curl -sf -o /dev/null "$url" 2>/dev/null; then
      return 0
    fi
    sleep 0.5
  done
  echo "  ✗ Timed out waiting for $desc at $url" >&2
  return 1
}

# ── Allocate random port ─────────────────────────────────────────────────────

PORT=$(find_free_port)

echo "=== Panorama E2E Harness ==="
echo "  server   : 127.0.0.1:$PORT"
echo ""

# ── Temp data directory ──────────────────────────────────────────────────────

DATA_DIR=$(mktemp -d)
mkdir -p "$DATA_DIR/plugins"

cleanup() {
  set +e
  if [ -n "${SERVER_PID:-}" ]; then
    kill "$SERVER_PID" 2>/dev/null
    wait "$SERVER_PID" 2>/dev/null
  fi
  rm -rf "$DATA_DIR"
  echo ""
  echo "=== Cleanup complete ==="
}
trap cleanup EXIT INT TERM

# ── Build ────────────────────────────────────────────────────────────────────

if $BUILD; then
  echo "--- Building plugin UIs ---"
  for dir in crates/panorama-app-*/ui/; do
    if [ -f "$dir/package.json" ] && [ -f "$dir/vite.config.ts" ]; then
      name=$(basename "$(dirname "$dir")")
      echo "  Building UI: $name"
      (cd "$dir" && npm ci --silent && npm run build) || {
        echo "  ✗ UI build failed for $name" >&2
        exit 1
      }
    fi
  done

  echo ""
  echo "--- Building frontend ---"
  (cd frontend && npm ci --silent && npm run build)

  echo ""
  echo "--- Building WASM plugins ---"
  bash scripts/build-wasm.sh

  echo ""
  echo "--- Building server (embeds frontend via rust-embed) ---"
  cargo build --release -p panorama-server

  echo ""
  echo "--- Packaging .panoapp files ---"
  bash scripts/build-panoapp.sh
fi

# Copy .panoapp files into isolated data dir
cp dist/panoapp/*.panoapp "$DATA_DIR/plugins/" 2>/dev/null || {
  echo "  ✗ No .panoapp files found in dist/panoapp/ — build first or check paths" >&2
  exit 1
}
echo "  Copied $(ls "$DATA_DIR/plugins/" | wc -l) .panoapp files"
echo ""

# ── Start server (single binary — API + plugins + embedded frontend) ─────────

echo "--- Starting server ---"
PANORAMA_DATA_DIR="$DATA_DIR" \
  PANORAMA_LISTEN="127.0.0.1:$PORT" \
  target/release/panorama-server &
SERVER_PID=$!

wait_for_url "http://127.0.0.1:$PORT/api/plugins" 60 "server /api/plugins"
echo "  ✓ Server ready on port $PORT (pid $SERVER_PID)"
echo ""

# ── Run E2E tests ────────────────────────────────────────────────────────────

echo "--- Running E2E tests ---"
echo ""

(
  cd frontend
  PLAYWRIGHT_BASE_URL="http://127.0.0.1:$PORT" \
    npx playwright test --project=chromium "${PLAYWRIGHT_ARGS[@]}"
)

echo ""
echo "=== Tests complete ==="
