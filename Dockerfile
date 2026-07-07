# Panorama — single-image production build
#
# Stage 1 (build): Rust + Bun + Nx → WASM plugins, .panoapp files, server binary
# Stage 2 (runtime): Debian slim with baked-in plugins
#
# The server binary embeds the frontend SPA (rust-embed).
# .panoapp plugin files are baked into the image at /usr/local/share/panorama/plugins/
# and loaded at startup via PANORAMA_PLUGINS_DIR.

# ── Stage 1: Build ───────────────────────────────────────────────────────────
FROM rust:1-slim-bookworm AS builder

# Install bun
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl openssl pkg-config unzip \
    && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL https://bun.sh/install | bash
ENV PATH="/root/.bun/bin:${PATH}"

# Install wasm32-wasip1 target for WASM plugin compilation
RUN rustup target add wasm32-wasip1

WORKDIR /build

# Install JS dependencies (root + all workspaces)
COPY package.json bun.lock ./
COPY frontend/package.json frontend/
COPY crates/panorama-app-journal/ui/package.json crates/panorama-app-journal/ui/
COPY crates/panorama-app-coding/ui/package.json crates/panorama-app-coding/ui/
COPY crates/panorama-app-dashboards/ui/package.json crates/panorama-app-dashboards/ui/
COPY crates/panorama-app-trips/ui/package.json crates/panorama-app-trips/ui/
COPY crates/panorama-app-restaurants/ui/package.json crates/panorama-app-restaurants/ui/
COPY crates/panorama-app-music/ui/package.json crates/panorama-app-music/ui/
COPY crates/panorama-app-files/ui/package.json crates/panorama-app-files/ui/
RUN bun install

# Copy full source tree
COPY . .

# Build all .panoapp files (plugin UIs + WASM + package)
RUN bun x nx run-many -t package-panoapp -c release

# Build server binary with embedded frontend SPA
RUN bun x nx build panorama-server -c release

# ── Stage 2: Runtime ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/panorama-server /usr/local/bin/panorama-server

# Bake .panoapp files into the image
COPY --from=builder /build/dist/panoapp /usr/local/share/panorama/plugins

ENV PANORAMA_DATA_DIR=/var/lib/panorama/data
ENV PANORAMA_LISTEN=0.0.0.0:3000
ENV PANORAMA_PLUGINS_DIR=/usr/local/share/panorama/plugins:${PANORAMA_DATA_DIR}/plugins

EXPOSE 3000

HEALTHCHECK --interval=15s --timeout=5s --retries=5 --start-period=10s \
    CMD curl -f http://localhost:3000/api/plugins || exit 1

CMD ["panorama-server"]
