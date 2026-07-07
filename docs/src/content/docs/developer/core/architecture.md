---
title: Architecture Overview
description: High-level overview of the Panorama system architecture and code repository layout.
---

This document outlines the codebase layout, system architecture, and query/data flows for the core Panorama platform.

---

## 1. Repository Layout

The codebase is organized as a Cargo and Bun monorepo workspace:

```
panorama/
├── crates/
│   ├── panorama-core/          # Core abstractions (AST, parser, Plugin traits, capabilities)
│   ├── panorama-server/        # Axum server, SQLite storage, Query compiler, Wasmtime sandbox
│   ├── wasm-types/             # WASM boundary serialization types
│   └── panorama-app-*/         # Native/WASM implementation crates for example apps
├── frontend/                   # Main Vite + React + Module Federation web shell
├── design/                     # Architectural design, hook system, and query specifications
├── dist/panoapp/               # Packaging output folder for .panoapp files
└── justfile                    # Build and E2E runner recipes
```

---

## 2. Core Components

### Panorama Core (`panorama-core`)
The shared crate that defines the core abstractions of the platform. It has no dependencies on server components, allowing it to be compiled for the target `wasm32-wasip1` to be used in plugin development. It includes:
*   **AST and Parser**: Code that parses PQL query strings into an Abstract Syntax Tree.
*   **Plugin Traits**: Interface contracts (e.g. `Plugin` and `PluginContext`) that plugins implement and query.
*   **Capabilities Model**: Structural verification rules for plugin data read/write grants and networking limits.

### Panorama Server (`panorama-server`)
The host server built on Axum that coordinates runtime operations. It handles:
*   **Axum Web Router**: Serves REST APIs (`/api/*`) and handles plugin request dispatch routing.
*   **Node Storage**: Interface to SQLite, executing physical SQL operations.
*   **Query Compiler**: The PQL parser client that translates ASTs to SQL queries.
*   **Wasmtime Sandbox**: Integrates an embedded Wasmtime runtime that runs untrusted plugin WASM binaries inside a capability-checked sandbox.
*   **Reactor Engine**: Runs pre-commit Eager hooks and schedules Deferred post-commit background tasks.

### Frontend Web Shell (`frontend`)
A React single-page application built with Vite and UnoCSS. It includes:
*   **Core UI views**: Nodes table explorer, Schemas dictionary viewer, and Plugin manager panel.
*   **Module Federation**: Dynamically loads plugin UI remote entries (packaged inside `.panoapp` files) directly into the app shell at runtime, providing a unified single-window experience.

---

## 3. System Architecture & Boundaries

```
┌─────────────────────────────────────────────────────────┐
│                    Panorama Frontend                     │
│              (Vite + React + TanStack)                   │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐  │
│  │ Node     │  │ Schema   │  │ Plugin UI            │  │
│  │ Viewer   │  │ Viewer   │  │ (Module Federation)  │  │
│  └──────────┘  └──────────┘  └──────────────────────┘  │
└──────────────────────┬──────────────────────────────────┘
                       │ HTTP (REST + plugin subpaths)
┌──────────────────────┴──────────────────────────────────┐
│                   Panorama Server (Rust)                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐  │
│  │ REST API │  │ Plugin   │  │ Embedded Wasmtime    │  │
│  │ /api/*   │  │ Loader   │  │ WASI preview1 Engine │  │
│  └──────────┘  └──────────┘  └──────────────────────┘  │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐  │
│  │ Node     │  │ Schema   │  │ Object Storage       │  │
│  │ Storage  │  │ Registry │  │ (SQLite + Files)     │  │
│  └──────────┘  └──────────┘  └──────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

---

## 4. End-to-End Data & Query Flow

### Write Flow (Creating/Updating Nodes)
1.  **Request**: Client sends a JSON payload to `POST /api/nodes`.
2.  **Authentication**: Server checks user membership for the target `space_id`.
3.  **Eager Hook Execution**: Eager reactors registered on `before_node_create` are executed synchronously in priority order. They can inspect, reject (`validate`), or rewrite (`transform`) the payload.
4.  **Transaction Gate**: If a `Required` schema is attached, the schema engine validates field types and requirements. Any schema violation aborts the transaction.
5.  **Storage Write**: Node data is inserted as a JSON blob in SQLite; associated meta indexes (`field_presence` and `node_schema_conformance`) are updated.
6.  **Deferred Dispatch**: Post-commit operations (e.g. `FieldWatch` or `PendingWork` deferred reactors) are queued on the background worker stream.

### Read Flow (Executing PQL Queries)
1.  **Request**: Client calls `POST /api/query` with a PQL string.
2.  **Phase 1 (Meta Resolution)**: The PQL compiler checks schema existence, field names, and namespace IDs in meta tables.
3.  **Phase 2 (SQL Generation)**: The compiler converts the query AST into a single SQLite query joining metadata tables (`field_presence`) with promoted schema tables.
4.  **Database Execution**: SQLite executes the generated query utilizing indexes.
5.  **Capability Filtering**: Result fields are filtered according to caller permissions on output serialization.
