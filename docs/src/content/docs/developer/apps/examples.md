---
title: Example Reference Apps
description: Documentation of the 7 example app plugins included in the Panorama codebase.
---

Panorama comes with eight pre-built example applications. These serve as production-grade reference implementations of schemas, endpoints, capabilities, and UIs.

---

## 1. Journal (`io.mzhang.panorama.journal`)
A daily markdown journal with block-level references.
*   **Purpose**: Demonstrates simple markdown content storage, timeline querying, and back-referencing.
*   **Key Schemas**: `JournalEntry` (`journal/JournalEntry`)
*   **Key Endpoints**:
    *   `POST /entries` — Create a new daily entry node.
    *   `GET /entries` — Query matching entries sorted chronologically.
    *   `GET /entries/{id}` — Fetch details for a specific entry.
*   **Capabilities**: Reads and writes under the `journal:` and `system:` namespaces.

---

## 2. Coding Activity (`io.mzhang.panorama.coding`)
A coding activity tracker compatible with wakatime clients.
*   **Purpose**: Demonstrates ingestion of bulk time-series data, timestamp operations, and statistics aggregations.
*   **Key Schemas**: `Heartbeat`, `Duration`, `DailySummary`
*   **Key Endpoints**:
    *   `POST /users/current/heartbeats` — Ingest single editor heartbeat.
    *   `POST /users/current/heartbeats.bulk` — Ingest bulk heartbeats.
    *   `GET /users/current/durations` — Calculate block coding durations.
    *   `GET /summaries` — Retrieve daily coding summaries.
*   **Capabilities**: Holds write-only capability for its own schemas and reads system node-time values.

---

## 3. Dashboards (`io.mzhang.panorama.dashboards`)
Leaderboard and time-series visualization system.
*   **Purpose**: Demonstrates building complex dashboard widgets, importing layouts, and running custom PromQL-like aggregations.
*   **Key Schemas**: `Dashboard`, `Folder`
*   **Key Endpoints**:
    *   `GET /api/dashboards` — List available dashboards.
    *   `POST /api/dashboards` — Create or import dashboard layouts.
    *   `POST /api/ds/query` — Run custom PQL queries for time-series charts.
*   **Capabilities**: Reads across multiple application schemas to display aggregated statistics in unified panels.

---

## 4. Trip Planner (`io.mzhang.panorama.trips`)
Trip planner with calendar scheduling and map visualization.
*   **Purpose**: Demonstrates geocoding, calendar integrations, and map rendering.
*   **Key Schemas**: `Trip`, `Event`
*   **Key Endpoints**:
    *   `POST /trips` — Create new trips.
    *   `POST /events` — Add scheduled events (flights, hotel, meals) to a trip.
    *   `GET /events/map` — Retrieve geocoded coordinate markers for maps.
*   **Capabilities**: Holds grants for `trips:` schema writes and network capability to fetch maps or geocode coordinates.

---

## 5. Restaurant Rankings (`io.mzhang.panorama.restaurants`)
Restaurant ranking engine using pairwise partial comparisons.
*   **Purpose**: Demonstrates computing partial orders using sorting algorithms.
*   **Key Schemas**: `Restaurant`, `Comparison`
*   **Key Endpoints**:
    *   `POST /restaurants` — Add restaurant nodes.
    *   `POST /compare` — Register comparison nodes (e.g. Restaurant A > Restaurant B).
    *   `GET /rankings` — Run topological sort to output a ranked list of restaurants.
*   **Capabilities**: Standard database reads/writes scoped to `restaurants:` namespace.

---

## 6. Music Library (`io.mzhang.panorama.music`)
Subsonic-compatible music streaming library.
*   **Purpose**: Demonstrates streaming binary media blobs and serving Subsonic API requests.
*   **Key Schemas**: `Artist`, `Album`, `Track`
*   **Key Endpoints**:
    *   `GET /rest/ping` / `GET /rest/getArtists` — Subsonic REST API routing.
    *   `GET /rest/stream` — Serve media audio stream directly from Object Storage.
    *   `POST /upload` — Upload raw audio files.
*   **Capabilities**: Requires `object_storage_read` and `object_storage_write` permissions to manage large music files.

---

## 7. File Manager (`io.mzhang.panorama.files`)
General file browser with chunked, resumable upload support.
*   **Purpose**: Demonstrates chunked file uploads, object references, and mime-type handling.
*   **Key Schemas**: `File`
*   **Key Endpoints**:
    *   `POST /upload/initiate` — Get a session UUID for a resumable upload.
    *   `POST /upload/{session}/chunks` — Upload file chunks as base64 data.
    *   `POST /upload/{session}/complete` — Reassemble and commit the object.
    *   `GET /files` — List stored files.
*   **Capabilities**: Full access to object storage buckets (`object_storage_read`/`object_storage_write`) and permission to create app-managed immutable file records.

---

## 8. Test Reactor (`test.reactor.plugin`)
A test-only plugin for deterministic reactor validation during integration testing.
*   **Purpose**: Provides fixed, predictable reactor handler functions used by the E2E reactor test suite. Not intended for production use.
*   **Key Endpoints**:
    *   `POST /eager-validate-approve` — Returns an `Approved` eager validation result.
    *   `POST /eager-validate-reject` — Returns a `Rejected` result with a fixed reason.
    *   `POST /eager-transform` — Returns a `Transformed` result with a test value.
    *   `POST /eager-compute` — Returns a `Computed` result for a given field.
*   **Capabilities**: Holds `test:*` field write and `*` field read grants. Used exclusively by `reactor_tests.rs`.
