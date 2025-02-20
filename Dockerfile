# Build the rust
FROM rust:1.84 AS backend-builder
RUN mkdir /app
WORKDIR /app
COPY Cargo.toml /app
COPY Cargo.lock /app
COPY backend/ /app/backend/
RUN cargo build --release --locked

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

FROM ubuntu:latest
RUN mkdir /app
COPY --from=backend-builder /app/target/release/backend /app/backend
COPY --from=frontend-builder /usr/local/bin/bun /root/.local/state/panorama/bin/bun
CMD ["/app/backend"]
