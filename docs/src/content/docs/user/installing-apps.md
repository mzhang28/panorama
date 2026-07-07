---
title: Installing Apps
description: Guide to installing third-party applications (.panoapp bundles) in Panorama.
---

Panorama features an extensible plugin system. Third-party applications are distributed as single-file bundles called **`.panoapp`** files.

---

## What is a `.panoapp`?

A `.panoapp` file is a standard ZIP archive containing everything required to execute the application:

```
my-app.panoapp
├── manifest.json        # Metadata, schema declarations, endpoints, and capabilities
├── plugin.wasm          # Compiled WASM backend logic (wasm32-wasip1 target)
└── ui/                  # Built static web UI assets (Module Federation bundle)
    ├── index.html
    ├── app.js
    └── app.css
```

---

## Installation Steps

Installing an app is a matter of copying its bundle file into the platform directory:

1.  **Obtain the `.panoapp` file**:
    *   Build it locally from source (see the [Building & Packaging](/developer/apps/building-packaging) developer guide).
    *   Download a pre-packaged bundle from a trusted source.
2.  **Locate the Plugins folder**:
    *   By default, this directory is located at `./data/plugins/` relative to the workspace root.
    *   If you have configured `PANORAMA_DATA_DIR` to point to a custom path (e.g. `/var/lib/panorama/data`), the plugins folder resides at `$PANORAMA_DATA_DIR/plugins/`.
3.  **Copy the file**:
    *   Place the `.panoapp` file (e.g., `io.mzhang.panorama.journal.panoapp`) directly into the `plugins/` directory.
4.  **Restart the Server**:
    *   Plugin discovery happens at server startup. Restart the `panorama-server` process to load the new application.
    *   The HTTP server starts immediately; plugins are loaded asynchronously in the background and become available as each one finishes loading.
    *   Check `GET /api/plugins/status` to monitor the load progress of individual plugins.
    ```bash
    # Restart if using just
    just serve
    ```

---

## Verifying the Installation

Once the server completes its boot sequence:

1.  **Check Terminal logs**:
    *   Look for startup logs indicating successful plugin discovery and load:
    ```
    [INFO] Scanning for plugins
    [INFO] Discovered .panoapp
    [INFO] Loaded plugin: Daily Journal v0.1.0 (io.mzhang.panorama.journal)
    ```
2.  **Inspect via the Web UI**:
    *   Navigate to the **Plugins** view from the sidebar (`http://localhost:3000/plugins`).
    *   You should see your newly installed app listed in the browser, showing its description, registered schemas, endpoints, and UI configuration.
    *   The app's interface will automatically appear under the **Installed Apps** section in the main sidebar.
3.  **Check plugin status via API**:
    *   `GET /api/plugins/status` returns the load status of each plugin (pending, loading, loaded, or failed).
