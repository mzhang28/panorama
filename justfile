# Panorama — development commands
# Usage: just <recipe>

default:
    @just --list

# Build WASM plugin modules (wasm32-wasip1)
build-wasm:
    bash scripts/build-wasm.sh

# Package .panoapp files from manifests + WASM
build-panoapps:
    bash scripts/build-panoapp.sh

# Build everything (WASM + .panoapp + server binary)
build:
    bash scripts/build-wasm.sh
    bash scripts/build-panoapp.sh
    cargo build -p panorama-server

# Start the server (loads .panoapp from data/plugins/)
serve: build-panoapps
    bash scripts/serve.sh

# Install frontend dependencies  
install-frontend:
    cd frontend && npm install

# Start the frontend dev server (open http://localhost:5173)
frontend: install-frontend
    cd frontend && npm run dev

# Install Playwright browsers (one-time)
test-setup: install-frontend
    cd frontend && npx playwright install chromium

# Run E2E tests (server + frontend must be running)
test-e2e:
    cd frontend && npx playwright test

# Run Rust tests
test-rust:
    cargo test --workspace

# Type-check everything
check:
    cargo check
    cd frontend && npx tsc --noEmit

# Clean build artifacts
clean:
    cargo clean
    rm -rf dist data
    @echo "Cleaned."
