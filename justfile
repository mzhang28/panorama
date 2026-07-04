# Panorama — development commands
# Usage: just <recipe>

default:
    @just --list

# Build WASM plugin modules (wasm32-wasip1)
build-wasm:
    bash scripts/build-wasm.sh

# Package .panoapp files (builds plugin UIs + WASM + zips)
build-panoapps:
    bash scripts/build-panoapp.sh

# Build everything (frontend, WASM plugins, server, and .panoapp packages)
build:
    cd frontend && bun install --silent && bun x vite build --mode development
    bash scripts/build-wasm.sh
    cargo build --release -p panorama-server
    bash scripts/build-panoapp.sh

# Build + start the server (loads .panoapp from data/plugins/)
serve: build-panoapps
    cargo build --release -p panorama-server
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

# Run isolated E2E tests against pre-built server & apps (pass arguments directly to e2e.py / playwright)
test-e2e *args:
    python3 scripts/e2e.py {{ args }}

# Alias for test-e2e (runs without building by default)
test-e2e-quick *args:
    python3 scripts/e2e.py {{ args }}

# Run Rust tests
test-rust:
    cargo test --workspace

# Profile Rust integration tests and generate a flamegraph
profile-tests:
    RUSTFLAGS="-C force-frame-pointers=yes" cargo test --profile release-with-debuginfo -p panorama-server --test integration_test --no-run
    bash scripts/profile-tests.sh

# Type-check everything
check:
    cargo check --workspace
    cd frontend && npx tsc --noEmit

# Clean build artifacts
clean:
    cargo clean
    rm -rf dist data frontend/dist
    @echo "Cleaned."

# ── Docker ───────────────────────────────────────────────────

# Build Docker images
docker-build:
    docker compose build

# Start the full stack (background)
docker-up:
    docker compose up -d

# Stop the stack
docker-down:
    docker compose down

# View logs
docker-logs:
    docker compose logs -f

# Rebuild and restart
docker-restart: docker-down docker-build docker-up

# ── AFL++ Fuzzing ─────────────────────────────────────────────

# Build AFL++ fuzzing Docker image
fuzz-build:
    docker build -t panorama-fuzz -f Dockerfile.fuzz .

# Run AFL++ fuzzing on PromQL parser & translator
fuzz-promql:
    docker run -it --rm -v $(pwd)/fuzz/findings:/workspace/fuzz/findings panorama-fuzz promql

# Run AFL++ fuzzing on PanoramaQL (PQL) parser
fuzz-pql:
    docker run -it --rm -v $(pwd)/fuzz/findings:/workspace/fuzz/findings panorama-fuzz pql

