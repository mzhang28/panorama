#!/usr/bin/env python3
"""Generate manifest.json for a .panoapp package from plugin definitions."""
import json, sys

MANIFESTS = {
    "io.mzhang.panorama.journal": {
        "manifest_version": 1,
        "id": "io.mzhang.panorama.journal",
        "name": "Journal",
        "version": "0.1.0",
        "description": "Daily journal with markdown entries and block-level references",
        "schemas": [{
            "name": "JournalEntry",
            "version": {"major": 1, "minor": 0},
            "schema_mode": "Preferred",
            "fields": [
                {"name": "node_title", "namespace": "system", "required": False, "field_type": None, "default": None, "description": "Journal entry title", "computed": None},
                {"name": "node_time", "namespace": "system", "required": True, "field_type": None, "default": None, "description": "Date of the journal entry", "computed": None},
                {"name": "content", "namespace": "journal", "required": True, "field_type": None, "default": None, "description": "Markdown content", "computed": None},
                {"name": "mood", "namespace": "journal", "required": False, "field_type": None, "default": None, "description": "Optional mood tag", "computed": None},
                {"name": "paragraph_refs", "namespace": "journal", "required": False, "field_type": None, "default": None, "description": "References to paragraph-level nodes", "computed": None},
            ]
        }],
        "http_endpoints": [
            {"method": "POST", "path": "/entries", "description": "Create a new journal entry"},
            {"method": "GET", "path": "/entries", "description": "List all journal entries"},
            {"method": "GET", "path": "/entries/{id}", "description": "Get a specific journal entry"},
        ],
        "ui_components": [
            {"id": "journal-main", "name": "Journal View", "mount_point": "MainPage", "bundle_path": "ui/journal.js"},
            {"id": "journal-sidebar", "name": "Journal Sidebar", "mount_point": "Sidebar", "bundle_path": "ui/journal-sidebar.js"},
        ],
        "capabilities": {
            "version": 1, "reason": None, "network_hosts": [],
            "field_read": ["journal:*", "system:node_title", "system:node_time"],
            "field_write": ["journal:*", "system:node_title", "system:node_time"],
            "write_own_nodes": True, "app_managed_nodes": False,
            "file_read": False, "file_write": False, "execute": False,
            "dns_requests": False, "object_storage_read": False, "object_storage_write": False,
        },
        "wasm_module": "plugin.wasm",
        "background_tasks": [], "env_vars": {},
        "author": None, "homepage": None, "icon": None, "min_platform_version": None,
    },
    "io.mzhang.panorama.wakatime": {
        "manifest_version": 1,
        "id": "io.mzhang.panorama.wakatime",
        "name": "Coding Activity",
        "version": "0.1.0",
        "description": "Receives Wakatime-compatible heartbeats and stores them as time-series nodes",
        "schemas": [{
            "name": "Heartbeat",
            "version": {"major": 1, "minor": 0},
            "schema_mode": "Preferred",
            "fields": [
                {"name": "node_time", "namespace": "system", "required": True, "field_type": None, "default": None, "description": "When the heartbeat was recorded", "computed": None},
                {"name": "entity", "namespace": "wakatime", "required": True, "field_type": None, "default": None, "description": "File path or entity being edited", "computed": None},
                {"name": "project", "namespace": "wakatime", "required": False, "field_type": None, "default": None, "description": "Project name", "computed": None},
                {"name": "language", "namespace": "wakatime", "required": False, "field_type": None, "default": None, "description": "Programming language", "computed": None},
                {"name": "duration", "namespace": "wakatime", "required": False, "field_type": None, "default": None, "description": "Duration in seconds", "computed": None},
                {"name": "category", "namespace": "wakatime", "required": False, "field_type": None, "default": None, "description": "Coding activity category", "computed": None},
            ]
        }],
        "http_endpoints": [
            {"method": "POST", "path": "/heartbeat", "description": "Receive a Wakatime-compatible heartbeat"},
            {"method": "POST", "path": "/heartbeats", "description": "Receive multiple heartbeats (bulk)"},
        ],
        "ui_components": [{"id": "wakatime-dashboard", "name": "Coding Activity", "mount_point": "Dashboard", "bundle_path": "ui/wakatime.js"}],
        "capabilities": {"version": 1, "reason": None, "network_hosts": [], "field_read": ["wakatime:*"], "field_write": ["wakatime:*", "system:node_time"], "write_own_nodes": True, "app_managed_nodes": False, "file_read": False, "file_write": False, "execute": False, "dns_requests": False, "object_storage_read": False, "object_storage_write": False},
        "wasm_module": "plugin.wasm", "background_tasks": [], "env_vars": {}, "author": None, "homepage": None, "icon": None, "min_platform_version": None,
    },
    "io.mzhang.panorama.grafana": {
        "manifest_version": 1, "id": "io.mzhang.panorama.grafana", "name": "Dashboards", "version": "0.1.0",
        "description": "Grafana-like dashboards for time-series data visualization",
        "schemas": [{
            "name": "Dashboard", "version": {"major": 1, "minor": 0}, "schema_mode": "Preferred",
            "fields": [
                {"name": "node_title", "namespace": "system", "required": True, "field_type": None, "default": None, "description": "Dashboard name", "computed": None},
                {"name": "config", "namespace": "grafana", "required": True, "field_type": None, "default": None, "description": "Dashboard JSON configuration", "computed": None},
            ]
        }],
        "http_endpoints": [
            {"method": "POST", "path": "/query", "description": "Execute a dashboard query"},
            {"method": "POST", "path": "/dashboards", "description": "Save a dashboard configuration"},
            {"method": "GET", "path": "/dashboards", "description": "List saved dashboards"},
        ],
        "ui_components": [{"id": "grafana-main", "name": "Dashboard View", "mount_point": "MainPage", "bundle_path": "ui/grafana.js"}],
        "capabilities": {"version": 1, "reason": None, "network_hosts": [], "field_read": ["*"], "field_write": ["grafana:*", "system:node_title"], "write_own_nodes": True, "app_managed_nodes": False, "file_read": False, "file_write": False, "execute": False, "dns_requests": False, "object_storage_read": False, "object_storage_write": False},
        "wasm_module": "plugin.wasm", "background_tasks": [], "env_vars": {}, "author": None, "homepage": None, "icon": None, "min_platform_version": None,
    },
    "io.mzhang.panorama.trips": {
        "manifest_version": 1, "id": "io.mzhang.panorama.trips", "name": "Trip Planner", "version": "0.1.0",
        "description": "Trip planner with events, calendar and map views",
        "schemas": [
            {
                "name": "Trip", "version": {"major": 1, "minor": 0}, "schema_mode": "Preferred",
                "fields": [
                    {"name": "node_title", "namespace": "system", "required": True, "field_type": None, "default": None, "description": "Trip name", "computed": None},
                    {"name": "start_date", "namespace": "trips", "required": True, "field_type": None, "default": None, "description": "Trip start date", "computed": None},
                    {"name": "end_date", "namespace": "trips", "required": False, "field_type": None, "default": None, "description": "Trip end date", "computed": None},
                ]
            },
            {
                "name": "Event", "version": {"major": 1, "minor": 0}, "schema_mode": "Preferred",
                "fields": [
                    {"name": "node_title", "namespace": "system", "required": True, "field_type": None, "default": None, "description": "Event name", "computed": None},
                    {"name": "node_start_time", "namespace": "system", "required": True, "field_type": None, "default": None, "description": "Event start time", "computed": None},
                    {"name": "node_end_time", "namespace": "system", "required": False, "field_type": None, "default": None, "description": "Event end time", "computed": None},
                    {"name": "trip_id", "namespace": "trips", "required": True, "field_type": None, "default": None, "description": "Reference to parent trip", "computed": None},
                    {"name": "latitude", "namespace": "trips", "required": False, "field_type": None, "default": None, "description": "Event location latitude", "computed": None},
                    {"name": "longitude", "namespace": "trips", "required": False, "field_type": None, "default": None, "description": "Event location longitude", "computed": None},
                    {"name": "location_name", "namespace": "trips", "required": False, "field_type": None, "default": None, "description": "Human-readable location name", "computed": None},
                    {"name": "notes", "namespace": "trips", "required": False, "field_type": None, "default": None, "description": "Event notes/description", "computed": None},
                ]
            }
        ],
        "http_endpoints": [
            {"method": "POST", "path": "/trips", "description": "Create a trip"},
            {"method": "GET", "path": "/trips", "description": "List trips"},
            {"method": "POST", "path": "/events", "description": "Create an event"},
            {"method": "GET", "path": "/events", "description": "List events"},
            {"method": "GET", "path": "/events/map", "description": "Get events with geo data for map view"},
        ],
        "ui_components": [
            {"id": "trips-main", "name": "Trip Planner", "mount_point": "MainPage", "bundle_path": "ui/trips.js"},
            {"id": "trips-calendar", "name": "Calendar View", "mount_point": {"Custom": "calendar"}, "bundle_path": "ui/calendar.js"},
            {"id": "trips-map", "name": "Map View", "mount_point": {"Custom": "map"}, "bundle_path": "ui/map.js"},
        ],
        "capabilities": {"version": 1, "reason": None, "network_hosts": [], "field_read": ["trips:*", "system:node_title", "system:node_start_time", "system:node_end_time"], "field_write": ["trips:*", "system:node_title", "system:node_start_time", "system:node_end_time"], "write_own_nodes": True, "app_managed_nodes": False, "file_read": False, "file_write": False, "execute": False, "dns_requests": False, "object_storage_read": False, "object_storage_write": False},
        "wasm_module": "plugin.wasm", "background_tasks": [], "env_vars": {}, "author": None, "homepage": None, "icon": None, "min_platform_version": None,
    },
    "io.mzhang.panorama.beli": {
        "manifest_version": 1, "id": "io.mzhang.panorama.beli", "name": "Restaurant Rankings", "version": "0.1.0",
        "description": "Restaurant ratings with partial ordering (pairwise comparisons)",
        "schemas": [
            {
                "name": "Restaurant", "version": {"major": 1, "minor": 0}, "schema_mode": "Preferred",
                "fields": [
                    {"name": "node_title", "namespace": "system", "required": True, "field_type": None, "default": None, "description": "Restaurant name", "computed": None},
                    {"name": "cuisine", "namespace": "beli", "required": False, "field_type": None, "default": None, "description": "Type of cuisine", "computed": None},
                    {"name": "location", "namespace": "beli", "required": False, "field_type": None, "default": None, "description": "Restaurant address/location", "computed": None},
                    {"name": "notes", "namespace": "beli", "required": False, "field_type": None, "default": None, "description": "User notes", "computed": None},
                ]
            },
            {
                "name": "Comparison", "version": {"major": 1, "minor": 0}, "schema_mode": "Preferred",
                "fields": [
                    {"name": "better_id", "namespace": "beli", "required": True, "field_type": None, "default": None, "description": "ID of the preferred restaurant", "computed": None},
                    {"name": "worse_id", "namespace": "beli", "required": True, "field_type": None, "default": None, "description": "ID of the less preferred restaurant", "computed": None},
                    {"name": "context", "namespace": "beli", "required": False, "field_type": None, "default": None, "description": "Optional context (e.g., best pizza)", "computed": None},
                ]
            }
        ],
        "http_endpoints": [
            {"method": "POST", "path": "/restaurants", "description": "Add a restaurant"},
            {"method": "GET", "path": "/restaurants", "description": "List restaurants"},
            {"method": "POST", "path": "/compare", "description": "Record a comparison (A > B)"},
            {"method": "GET", "path": "/rankings", "description": "Get partial order rankings"},
        ],
        "ui_components": [{"id": "beli-main", "name": "Restaurant Rankings", "mount_point": "MainPage", "bundle_path": "ui/beli.js"}],
        "capabilities": {"version": 1, "reason": None, "network_hosts": [], "field_read": ["beli:*", "system:node_title"], "field_write": ["beli:*", "system:node_title"], "write_own_nodes": True, "app_managed_nodes": False, "file_read": False, "file_write": False, "execute": False, "dns_requests": False, "object_storage_read": False, "object_storage_write": False},
        "wasm_module": "plugin.wasm", "background_tasks": [], "env_vars": {}, "author": None, "homepage": None, "icon": None, "min_platform_version": None,
    },
    "io.mzhang.panorama.subsonic": {
        "manifest_version": 1, "id": "io.mzhang.panorama.subsonic", "name": "Music Library", "version": "0.1.0",
        "description": "Subsonic-compatible music streaming interface",
        "schemas": [
            {"name": "Artist", "version": {"major": 1, "minor": 0}, "schema_mode": "Preferred", "fields": [{"name": "node_title", "namespace": "system", "required": True, "field_type": None, "default": None, "description": "Artist name", "computed": None}]},
            {"name": "Album", "version": {"major": 1, "minor": 0}, "schema_mode": "Preferred", "fields": [
                {"name": "node_title", "namespace": "system", "required": True, "field_type": None, "default": None, "description": "Album name", "computed": None},
                {"name": "artist_id", "namespace": "subsonic", "required": True, "field_type": None, "default": None, "description": "Reference to artist node", "computed": None},
                {"name": "year", "namespace": "subsonic", "required": False, "field_type": None, "default": None, "description": "Release year", "computed": None},
                {"name": "cover_art_ref", "namespace": "subsonic", "required": False, "field_type": None, "default": None, "description": "Object storage ref for cover art", "computed": None},
            ]},
            {"name": "Track", "version": {"major": 1, "minor": 0}, "schema_mode": "Preferred", "fields": [
                {"name": "node_title", "namespace": "system", "required": True, "field_type": None, "default": None, "description": "Track title", "computed": None},
                {"name": "album_id", "namespace": "subsonic", "required": True, "field_type": None, "default": None, "description": "Reference to album node", "computed": None},
                {"name": "artist_id", "namespace": "subsonic", "required": True, "field_type": None, "default": None, "description": "Reference to artist node", "computed": None},
                {"name": "track_number", "namespace": "subsonic", "required": False, "field_type": None, "default": None, "description": "Track number in album", "computed": None},
                {"name": "duration", "namespace": "subsonic", "required": False, "field_type": None, "default": None, "description": "Duration in seconds", "computed": None},
                {"name": "audio_ref", "namespace": "subsonic", "required": True, "field_type": None, "default": None, "description": "Object storage ref for audio file", "computed": None},
            ]}
        ],
        "http_endpoints": [
            {"method": "GET", "path": "/rest/ping", "description": "Subsonic ping"},
            {"method": "GET", "path": "/rest/getArtists", "description": "List artists"},
            {"method": "GET", "path": "/rest/getAlbumList2", "description": "List albums"},
            {"method": "GET", "path": "/rest/stream", "description": "Stream audio file"},
            {"method": "POST", "path": "/upload", "description": "Upload a music file"},
        ],
        "ui_components": [{"id": "subsonic-library", "name": "Music Library", "mount_point": "MainPage", "bundle_path": "ui/subsonic.js"}],
        "capabilities": {"version": 1, "reason": None, "network_hosts": [], "field_read": ["subsonic:*", "system:node_title"], "field_write": ["subsonic:*", "system:node_title"], "write_own_nodes": True, "app_managed_nodes": False, "file_read": False, "file_write": False, "execute": False, "dns_requests": False, "object_storage_read": True, "object_storage_write": True},
        "wasm_module": "plugin.wasm", "background_tasks": [], "env_vars": {}, "author": None, "homepage": None, "icon": None, "min_platform_version": None,
    },
    "io.mzhang.panorama.files": {
        "manifest_version": 1, "id": "io.mzhang.panorama.files", "name": "File Manager", "version": "0.1.0",
        "description": "File uploads with resumable transfers and object storage management",
        "schemas": [{
            "name": "File", "version": {"major": 1, "minor": 0}, "schema_mode": "Preferred",
            "fields": [
                {"name": "node_title", "namespace": "system", "required": True, "field_type": None, "default": None, "description": "File name", "computed": None},
                {"name": "object_ref", "namespace": "files", "required": True, "field_type": None, "default": None, "description": "Object storage reference", "computed": None},
                {"name": "file_size", "namespace": "files", "required": False, "field_type": None, "default": None, "description": "File size in bytes", "computed": None},
                {"name": "mime_type", "namespace": "files", "required": False, "field_type": None, "default": None, "description": "MIME type", "computed": None},
                {"name": "folder", "namespace": "files", "required": False, "field_type": None, "default": None, "description": "Virtual folder path", "computed": None},
            ]
        }],
        "http_endpoints": [
            {"method": "POST", "path": "/upload", "description": "Upload a file"},
            {"method": "POST", "path": "/upload/initiate", "description": "Initiate resumable upload"},
            {"method": "GET", "path": "/files", "description": "List files"},
            {"method": "GET", "path": "/files/{id}", "description": "Download a file"},
            {"method": "DELETE", "path": "/files/{id}", "description": "Delete a file"},
        ],
        "ui_components": [
            {"id": "files-main", "name": "File Browser", "mount_point": "MainPage", "bundle_path": "ui/files.js"},
            {"id": "files-upload", "name": "File Upload", "mount_point": "Sidebar", "bundle_path": "ui/upload.js"},
        ],
        "capabilities": {"version": 1, "reason": None, "network_hosts": [], "field_read": ["files:*", "system:node_title"], "field_write": ["files:*", "system:node_title"], "write_own_nodes": True, "app_managed_nodes": False, "file_read": False, "file_write": False, "execute": False, "dns_requests": False, "object_storage_read": True, "object_storage_write": True},
        "wasm_module": "plugin.wasm", "background_tasks": [], "env_vars": {}, "author": None, "homepage": None, "icon": None, "min_platform_version": None,
    },
}

if __name__ == "__main__":
    app_id = sys.argv[1] if len(sys.argv) > 1 else "io.mzhang.panorama.journal"
    manifest = MANIFESTS.get(app_id, MANIFESTS["io.mzhang.panorama.journal"])
    json.dump(manifest, sys.stdout, indent=2)
