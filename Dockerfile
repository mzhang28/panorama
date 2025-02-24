# Build the frontend
FROM oven/bun:1.2.2 AS frontend-builder
RUN mkdir /app
WORKDIR /app
COPY bun.lockb /app
COPY package.json /app
RUN bun install --frozen-lockfile --verbose
COPY index.html /app
COPY tsconfig.json /app
COPY vite.config.ts /app
COPY frontend/ /app/frontend/
RUN bunx vite build

# Build the rust
FROM rust:1.85 AS backend-builder
RUN mkdir /app
COPY --from=frontend-builder /app/dist /app/dist
WORKDIR /app
COPY Cargo.toml /app
COPY Cargo.lock /app
COPY backend/ /app/backend/
COPY src-tauri/ /app/src-tauri/
RUN PANORAMA_STATIC_ASSETS=../dist cargo build \
  --release \
  --locked \
  -p panorama-backend \
  --features static-build

FROM ubuntu:latest
RUN mkdir /app
COPY --from=backend-builder /app/target/release/panorama-backend /app/server
COPY --from=frontend-builder /usr/local/bin/bun /root/.local/state/panorama/bin/bun
CMD ["/app/server"]
