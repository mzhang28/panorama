---
title: Installation & Setup
description: Learn how to set up the Panorama development environment and run the server.
---

This guide walks you through setting up a local development environment for Panorama, compiling the server, building the frontend, packaging the example applications, and launching the services.

---

## Prerequisites

Before starting, ensure you have the following tools installed on your host system:

*   **Rust (1.80+)**: Required to build the core libraries, the Axum server, and the example plugin WASM modules.
    *   Install via: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
    *   Install the WASM compile target: `rustup target add wasm32-wasip1`
*   **Bun (1.1+)**: The fast JavaScript runtime used to build the React frontend, package `.panoapp` ZIP bundles, and run the developer scripts.
    *   Install via: `curl -fsSL https://bun.sh/install | sh`
*   **Just**: A handy command-line task runner.
    *   Install via cargo: `cargo install just` (or using your system package manager)

---

## Step 1: Clone the Repository

Clone the project repository and navigate into the root directory:

```bash
git clone <repo-url> panorama
cd panorama
```

---

## Step 2: Install Workspace Dependencies

Install the npm packages required by the frontend and developer tooling:

```bash
bun install
```

This installs all dependencies for the main workspace, the frontend package, and the app UI subprojects.

---

## Step 3: Compile and Package Everything

To compile the Rust backend, compile all plugin WASM targets, compile the React UI components, and bundle them into `.panoapp` archives, run:

```bash
just build
```

This command invokes Nx to run parallel build tasks:
1.  **Vite Build**: Packages the React frontend shell (`frontend/dist/`).
2.  **Rust Compile (Plugins)**: Compiles each example app plugin under `crates/` to WASM targets (`target/wasm32-wasip1/`).
3.  **Vite Build (Plugins)**: Packages individual plugin UIs.
4.  **Package Panoapps**: Packages manifests, WASM binaries, and plugin UIs into `.panoapp` ZIP packages in `dist/panoapp/`.
5.  **Rust Compile (Server)**: Compiles the `panorama-server` binary in debug mode.

---

## Step 4: Run the Server

Start the pre-built backend server and automatically copy the example apps into the plugins directory:

```bash
just serve
```

By default, the server will:
*   Bind to `127.0.0.1:3000`.
*   Scan for `.panoapp` files in `./data/plugins/`.
*   Establish its storage database at `./data/panorama.db`.

Access the web interface by visiting **`http://localhost:5173`** (Vite development mode) or directly via **`http://localhost:3000`** (production SPA fallback).
