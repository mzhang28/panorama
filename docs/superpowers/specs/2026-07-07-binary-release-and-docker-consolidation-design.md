# Binary Release & Docker Consolidation

**Date:** 2026-07-07
**Status:** design

## Overview

Two related goals:
1. A single Nx command that produces a self-contained release binary (frontend embedded, no panoapp packaging)
2. A single Dockerfile that replicates the Nx build process and ships a production image with baked-in .panoapp plugins

---

## Part 1: `package-all-panoapps` synthetic target

**Goal:** One target that builds every .panoapp file.

**Location:** `release/project.json` (new directory)

**Definition:**

```json
{
  "name": "package-all-panoapps",
  "targets": {
    "package-all": {
      "executor": "nx:run-commands",
      "options": {
        "command": "echo 'All .panoapp files packaged'"
      },
      "dependsOn": [
        "panorama-app-restaurants:package-panoapp",
        "panorama-app-files:package-panoapp",
        "panorama-app-dashboards:package-panoapp",
        "panorama-app-journal:package-panoapp",
        "panorama-app-music:package-panoapp",
        "panorama-app-trips:package-panoapp",
        "panorama-app-coding:package-panoapp"
      ]
    }
  }
}
```

The `command` is a no-op echo; all work happens via `dependsOn`. Outputs land in `dist/panoapp/` as each per-app target produces its `.panoapp` there.

**No changes needed to `panorama-server:build`.** The existing release configuration (`cargo build --release -p panorama-server`) with its `dependsOn: ["panorama-frontend:build"]` already builds the frontend first, and `build.rs` detects `frontend/dist/index.html` to set `cfg(frontend_embedded)`. The binary is self-contained.

---

## Part 2: `PANORAMA_PLUGINS_DIR` (colon-separated)

**Server change in `crates/panorama-server/src/main.rs`:**

Replace the single `plugins_dir` derived from `PANORAMA_DATA_DIR` with a colon-separated scan over `PANORAMA_PLUGINS_DIR`.

- **Env var:** `PANORAMA_PLUGINS_DIR`
- **Format:** colon-separated paths, like `PATH`
- **Default:** `$PANORAMA_DATA_DIR/plugins` (fully backwards compatible)
- **Docker value:** `/usr/local/share/panorama/plugins:$PANORAMA_DATA_DIR/plugins`
- **Behavior:** The `load_plugins_in_background` function iterates over each path segment, scans for `.panoapp` files in each, and loads them. If a plugin with the same `id` appears in multiple directories, first wins (system plugins can be overridden by user plugins in the data dir by ordering the colon list appropriately — though the default Docker ordering puts system first so baked-in apps load reliably).

---

## Part 3: Single Dockerfile

**Replaces both `Dockerfile.backend` and `Dockerfile.frontend`.**

### Stage 1: Build

Base: `rust:1-slim-bookworm`

- Install bun, wasm32-wasip1 target
- Copy package.json, bun.lock → `bun install`
- Copy full source tree
- `bun x nx run-many -t package-panoapp -c release` (builds all plugin UIs, WASM, and packages .panoapp)
- `bun x nx build panorama-server -c release` (builds frontend → release binary with embedded SPA)

### Stage 2: Runtime

Base: `debian:bookworm-slim`

- Install ca-certificates + wasmtime CLI
- Copy `panorama-server` binary from builder
- Copy `dist/panoapp/*.panoapp` to `/usr/local/share/panorama/plugins/`
- Set env: `PANORAMA_PLUGINS_DIR=/usr/local/share/panorama/plugins:$PANORAMA_DATA_DIR/plugins`
- No entrypoint script needed — the server directly scans both dirs
- Volume mount `$PANORAMA_DATA_DIR` for user data persistence (nodes, objects, user-installed plugins)

### Stage 3 (deprecated): No separate frontend stage

The frontend is embedded in the binary. No nginx, no separate service.

---

## Part 4: Cleanup

### Remove

- `Dockerfile.backend`
- `Dockerfile.frontend`

### Update

- `docker-compose.yml` — single service (`panorama`), no frontend container, volume for data dir
- `justfile` — update Docker recipes to reference the single Dockerfile; add `docker-build-release` recipe that handles the Nx build

### Keep untouched

- `Dockerfile.fuzz` — unrelated AFL fuzzing image

---

## Part 5: compose changes

```yaml
services:
  panorama:
    build:
      context: .
      dockerfile: Dockerfile
    image: panorama:latest
    ports:
      - "3000:3000"
    volumes:
      - panorama-data:/var/lib/panorama/data
    environment:
      - PANORAMA_DATA_DIR=/var/lib/panorama/data
      - PANORAMA_LISTEN=0.0.0.0:3000
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/api/plugins"]
      interval: 15s
      timeout: 5s
      retries: 5
      start_period: 10s

volumes:
  panorama-data:
    driver: local
```

---

## Implementation order

1. Add `PANORAMA_PLUGINS_DIR` support to server
2. Create `release/project.json` with the `package-all-panoapps` project
3. Create consolidated `Dockerfile`
4. Update `docker-compose.yml`
5. Remove old Dockerfiles
6. Update `justfile`
7. Verify: `just docker-build && just docker-up`
