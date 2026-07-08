# Panorama — development commands
# Usage: just <recipe>

default:
    @just --list

# Build WASM plugin modules in debug mode
build-wasm:
    bun x nx run-many -t build-wasm -c development

# Build WASM plugin modules in release mode
build-wasm-release:
    bun x nx run-many -t build-wasm -c release

# Package .panoapp files in debug mode (builds plugin UIs + WASM + zips)
build-panoapps:
    bun x nx run-many -t package-panoapp -c development

# Package .panoapp files in release mode
build-panoapps-release:
    bun x nx run-many -t package-panoapp -c release

# Build everything in debug mode (fastest for development)
build:
    bun x nx build release -c development

# Build everything in release mode (production binaries)
build-release:
    bun x nx build release -c release

# Build + start the server in debug mode
serve: build
    bash scripts/serve.sh debug

# Build + start the server in release mode
serve-release: build-release
    bash scripts/serve.sh release

# Install frontend dependencies
install-frontend:
    cd frontend && bun install

# Start the frontend dev server (open http://localhost:5173)
frontend: install-frontend
    cd frontend && bun run dev

# Install Playwright browsers (one-time)
test-setup: install-frontend
    cd frontend && bun x playwright install chromium

# Run isolated E2E tests against pre-built server & apps (pass arguments directly to e2e.ts / playwright)
test-e2e *args:
    NX_DAEMON=false NX_ISOLATE_PLUGINS=false bun x nx run e2e:test -- {{ args }}

# Alias for test-e2e (runs without building by default)
test-e2e-quick *args:
    bun scripts/e2e.ts {{ args }}

test-proptest:
    cargo test -p panorama-server --test query_eval_proptest

# Run Rust tests
test-rust:
    cargo test --workspace

# Run Storybook dev server for all plugin UIs
storybook:
    bun run --cwd frontend storybook

# Profile Rust integration tests and generate a flamegraph
profile-tests:
    RUSTFLAGS="-C force-frame-pointers=yes" cargo test --profile release-with-debuginfo -p panorama-server --test integration_test --no-run
    bash scripts/profile-tests.sh

# Type-check everything (Rust workspace + TS projects)
check:
    cargo check --workspace
    bun x nx run-many -t check

# Clean build artifacts
clean:
    cargo clean
    rm -rf dist data frontend/dist .nx
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
