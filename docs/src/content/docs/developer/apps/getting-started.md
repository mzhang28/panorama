---
title: Custom App Getting Started
description: Introduction to custom app plugin development and the manifest.json specification.
---

Panorama is designed as an app platform. Third-party applications are compiled to WASM and packaged into `.panoapp` bundles containing manifests, code, and static files. 

This guide details how to structure a new application and write its metadata manifest.

---

## 1. App Bundle Structure

A Panorama app is structured as a directory which is later packaged into a ZIP archive:

```
my-app/
├── manifest.json        # Application metadata, capabilities, schemas, and routes
├── src/                 # Rust source code (compiled to WASM)
│   ├── lib.rs
│   └── Cargo.toml
└── ui/                  # Web UI frontend code (compiled to HTML/JS/CSS)
    ├── package.json
    ├── vite.config.ts
    └── src/
```

---

## 2. Manifest Specification (`manifest.json`)

The `manifest.json` file resides in the root of the app bundle. It outlines the plugin's metadata, schemas, HTTP routes, capability grants, and UI configurations.

Below is an annotated example of a comprehensive manifest:

```json
{
  "manifest_version": 1,
  "id": "com.example.myapp",
  "name": "My Custom App",
  "version": "1.0.0",
  "description": "An example custom application showing schemas and routes.",
  "author": "Panorama",
  "wasm_module": "plugin.wasm",
  "background_tasks": [],
  "env_vars": {},
  "schemas": [
    {
      "name": "MyDataRecord",
      "version": { "major": 1, "minor": 0 },
      "schema_mode": "Required",
      "fields": [
        {
          "name": "title",
          "namespace": "com.example.myapp",
          "required": true,
          "field_type": { "type_tag": "String", "element_type": null },
          "default": null,
          "description": "The record title",
          "computed": null
        },
        {
          "name": "count",
          "namespace": "com.example.myapp",
          "required": false,
          "field_type": { "type_tag": "Integer", "element_type": null },
          "default": null,
          "description": "Optional counter",
          "computed": null
        }
      ]
    }
  ],
  "http_endpoints": [
    {
      "path": "/records",
      "method": "GET",
      "description": "List all records"
    },
    {
      "path": "/records",
      "method": "POST",
      "description": "Create a new record"
    }
  ],
  "ui_components": [
    {
      "id": "com-example-myapp-dashboard",
      "name": "MainDashboard",
      "mount_point": "MainPage",
      "bundle_path": "ui/app.js"
    }
  ],
  "capabilities": {
    "version": 1,
    "reason": null,
    "field_read": ["com.example.myapp:*", "system:*"],
    "field_write": ["com.example.myapp:*", "system:node_title"],
    "network_hosts": ["api.example.com"],
    "object_storage_read": true,
    "object_storage_write": true,
    "file_read": false,
    "file_write": false,
    "execute": false,
    "dns_requests": false,
    "write_own_nodes": true,
    "app_managed_nodes": true
  }
}
```

### Manifest Fields

*   **`manifest_version`**: Schema version of the manifest format itself (currently `1`).
*   **`id`**: Unique reverse-DNS identifier for the app. Used as the namespace key for fields written by this app.
*   **`name`**, **`version`**, **`description`**, **`author`**: Human-readable metadata.
*   **`wasm_module`**: Filename of the compiled WASM binary inside the `.panoapp` bundle (typically `plugin.wasm`).
*   **`schemas`**: Array of schemas declared by the application. Each schema defines its `name`, `version` (`{ major, minor }`), `schema_mode` (`"Preferred"` or `"Required"`), and an array of `fields` objects (each with `name`, `namespace`, `required`, `field_type`, `default`, `description`, `computed`).
*   **`http_endpoints`**: List of HTTP endpoints that the plugin handles. Each endpoint specifies `path`, `method` (`GET`, `POST`, `PUT`, `DELETE`, `PATCH`), and a `description`. Axum dispatches requests matching `/plugin/<app-id>/<path>` to the plugin handler.
*   **`ui_components`**: Specifies frontend interface components. Each has an `id` (unique element identifier), `name` (human-readable), `mount_point` (`MainPage`, `Sidebar`, `Dashboard`, or a custom string), and `bundle_path` (path to the JS bundle within the `ui/` folder).
*   **`background_tasks`**: Array of background task definitions (`name`, `interval_seconds`, `description`) that run on a schedule.
*   **`capabilities`**: Hard security restrictions. The WASM sandbox validates all host-context storage and network operations against these rules. Includes a `version` field (bumped on permission changes) and an optional `reason` for capability changes.
*   **`env_vars`**: Environment variables to pass to the plugin runtime.
