# Panorama — App Evaluation & Progress Roadmap

This document evaluates how closely the current application suite adheres to the design goals outlined in [DESIGN.md](file:///home/michael/Projects/panorama/DESIGN.md). It outlines actionable next features for **each** app and provides a comprehensive list of End-to-End (E2E) test specifications for each proposed feature.

---

## 1. Overall System & Architecture Alignment

### High-Level Alignment Summary

Panorama's core data engine and plugin architecture align exceptionally well with the foundational requirements defined in [DESIGN.md](file:///home/michael/Projects/panorama/DESIGN.md):

| Design Goal in `DESIGN.md` | Implementation Status | Evaluation & Notes |
| :--- | :--- | :--- |
| **Node Data Structure** | ✅ **Fully Aligned** | Nodes use UUIDs, arbitrary namespaced fields (`system:`, `user:`, `app:`), non-relational storage in SQLite with JSON extract helpers. |
| **Self-Hosted Schemas** | ✅ **Fully Aligned** | Schemas are nodes, versioned, supporting Preferred vs Required modes. System fields like `node_time`, `node_title`, `created_at` are standardized. |
| **Plugin Capabilities System** | ✅ **Fully Aligned** | Fine-grained capability manifest (`field_read`, `field_write`, `object_storage_read`, `write_own_nodes`). Sandbox enforcement via WASM (wasmtime). |
| **Object Storage API** | ✅ **Fully Aligned** | S3-like blob storage (`put_object`, `get_object`, `delete_object`) with resumable upload API hooks. |
| **Spaces & Permissions** | ⚠️ **Partial (v0.0)** | Default space (`default`) is supported across queries and nodes; multi-tenant space ACLs planned for v0.x. |
| **App Ecosystem** | 🔄 **Functional Prototypes** | All 7 target v0.0 applications are built as standalone crates relying *only* on `panorama-core`. |

### Summary of App Fidelity (v0.0 Assessment)

While the platform foundation is robust, individual apps currently represent baseline v0.0 prototypes. Core API endpoints and schemas are implemented, but rich UI interactions (e.g. block-level node references in Journal, interactive Leaflet maps in Trips, HTML5 audio player in Subsonic, visual charts in Dashboards) remain to be built.

---

## 2. Detailed Per-App Evaluation, Next Features & E2E Tests

---

### 2.1 Journal App (`panorama-app-journal`)

#### Evaluation against `DESIGN.md` Goals
- **Goal**: Daily journal where entries are stacked vertically with most recent on top, notes stored as markdown, and block-level breakdown into paragraph nodes for cross-referencing.
- **Current Status**:
  - Implements `journal/JournalEntry` schema with `system:node_title`, `system:node_time`, `journal:content`, `journal:mood`, `journal:paragraph_refs`.
  - Exposes `POST /entries` and `GET /entries` (queries ordered by `system:node_time DESC`).
  - Web UI renders simple entry submission form and raw text list.
- **Gaps**: Paragraph breakdown into child nodes is un-implemented (only a field schema placeholder exists). No rich markdown rendering (renders raw text in `<pre>`). No entry editing or search/filter by mood or date.

#### Actionable Next Features
1. **Block-Level Paragraph Decomposition & Linking**: Automatically split markdown content into individual paragraph child nodes upon entry creation, record node UUID links in `journal:paragraph_refs`, and allow embedding/referencing individual paragraphs in other notes.
2. **Rich Markdown Rendering & WYSIWYG Editor**: Replace plain text areas with a rich markdown editor and live viewer (headings, lists, bold/italic, blockquotes, code blocks).
3. **Journal Entry Editing & Soft Deletion**: Support inline editing of saved journal entries and updating node fields via `PUT /entries/{id}`.
4. **Timeline & Mood Filter Bar**: Filter journal entries dynamically by date range (calendar picker) or mood tags (`happy`, `thoughtful`, etc.).

#### Possible E2E Tests
```typescript
// E2E Test Suite: Journal Next Features

test.describe('Journal - Block-Level Paragraph Decomposition', () => {
  test('creates journal entry and verifies paragraph child nodes are generated', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.fill('input[placeholder="Entry title"]', 'Multi-paragraph Entry');
    await page.fill('textarea[placeholder*="Write your entry"]', 'First paragraph line.\n\nSecond paragraph line.');
    await page.click('button:has-text("Save Entry")');

    await page.click('strong:has-text("Multi-paragraph Entry")');
    await expect(page.locator('.paragraph-ref-link')).toHaveCount(2);
  });
});

test.describe('Journal - Rich Markdown Rendering', () => {
  test('renders markdown headers and bullet lists in entry view', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.fill('input[placeholder="Entry title"]', 'Markdown Test');
    await page.fill('textarea[placeholder*="Write your entry"]', '# Header 1\n- Item A\n- Item B');
    await page.click('button:has-text("Save Entry")');

    await page.click('strong:has-text("Markdown Test")');
    await expect(page.locator('.journal-entry-body h1')).toHaveText('Header 1');
    await expect(page.locator('.journal-entry-body ul li')).toHaveCount(2);
  });
});

test.describe('Journal - Entry Editing', () => {
  test('allows editing an existing entry title and content', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.click('.journal-entry-card:first-child .btn-edit');
    await page.fill('.edit-title-input', 'Updated Title');
    await page.click('button:has-text("Save Changes")');

    await expect(page.locator('strong:has-text("Updated Title")')).toBeVisible();
  });
});

test.describe('Journal - Timeline & Mood Filtering', () => {
  test('filters entry list by selected mood tag', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.selectOption('select.mood-filter', 'excited');

    const entryCards = page.locator('.journal-entry-card');
    for (const card of await entryCards.all()) {
      await expect(card.locator('.mood-badge')).toHaveText('Mood: excited');
    }
  });
});
```

---

### 2.2 Wakatime App (`panorama-app-wakatime`)

#### Evaluation against `DESIGN.md` Goals
- **Goal**: Expose an endpoint that real Wakatime clients/IDE plugins can submit heartbeats to, translating them into time-series nodes for project/language tracking.
- **Current Status**:
  - Implements `wakatime/Heartbeat` schema (`system:node_time`, `wakatime:entity`, `wakatime:project`, `wakatime:language`, `wakatime:duration`, `wakatime:category`).
  - Exposes `POST /heartbeat` and `POST /heartbeats` endpoints supporting standard Wakatime JSON payloads.
  - Basic manual JSON heartbeat submission UI.
- **Gaps**: Lacks Wakatime API key authentication header support, real-time activity status widget ("Currently coding..."), and session idle duration aggregation logic.

#### Actionable Next Features
1. **Automatic Heartbeat Duration & Idle Calculation**: Compute actual coding session durations based on timestamp gaps between consecutive heartbeats (using a configurable 2-minute idle cutoff).
2. **Live Activity Pulse Widget**: Real-time status indicator on the dashboard showing active file, project, and programming language based on heartbeats received within the last 5 minutes.
3. **Wakatime CLI API Key Authentication**: Validate incoming Wakatime CLI HTTP `Authorization: Basic <api_key>` headers against user security tokens.
4. **Project & Language Summary Statistics API**: Endpoint `GET /stats/summary?range=7d` returning pre-aggregated coding time totals per project and language.

#### Possible E2E Tests
```typescript
// E2E Test Suite: Wakatime Next Features

test.describe('Wakatime - Session Duration Calculation', () => {
  test('aggregates consecutive heartbeats into session duration', async ({ page, request }) => {
    // Send 3 heartbeats 1 minute apart via API
    const now = Math.floor(Date.now() / 1000);
    await request.post('/plugin/com.panorama.wakatime/heartbeats', {
      data: [
        { entity: '/src/main.rs', project: 'panorama', language: 'Rust', time: now - 120 },
        { entity: '/src/main.rs', project: 'panorama', language: 'Rust', time: now - 60 },
        { entity: '/src/main.rs', project: 'panorama', language: 'Rust', time: now }
      ]
    });

    await page.goto('/');
    await page.click('button:has-text("Wakatime")');
    await expect(page.locator('.total-session-time')).toContainText('2 mins');
  });
});

test.describe('Wakatime - Live Activity Pulse Widget', () => {
  test('displays live coding indicator when recent heartbeat exists', async ({ page, request }) => {
    await request.post('/plugin/com.panorama.wakatime/heartbeat', {
      data: { entity: 'App.tsx', project: 'frontend-v2', language: 'TypeScript', time: Date.now() / 1000 }
    });

    await page.goto('/');
    await page.click('button:has-text("Wakatime")');
    await expect(page.locator('.live-status-badge')).toBeVisible();
    await expect(page.locator('.live-status-badge')).toContainText('Coding on frontend-v2 (TypeScript)');
  });
});

test.describe('Wakatime - API Key Authentication', () => {
  test('rejects heartbeat submission without valid API key header when auth is enabled', async ({ request }) => {
    const response = await request.post('/plugin/com.panorama.wakatime/heartbeat', {
      headers: { 'Authorization': 'Basic invalid_key' },
      data: { entity: 'test.py' }
    });
    expect(response.status()).toBe(401);
  });
});

test.describe('Wakatime - Summary Statistics View', () => {
  test('displays weekly project coding time breakdown', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Wakatime")');
    await page.click('button:has-text("Weekly Summary")');
    await expect(page.locator('.project-summary-table')).toBeVisible();
  });
});
```

---

### 2.3 Dashboards / Grafana App (`panorama-app-grafana`)

#### Evaluation against `DESIGN.md` Goals
- **Goal**: Grafana-like dashboards for querying and visualizing time-series data (e.g. Wakatime coding hours, top projects leaderboard) over arbitrary time ranges.
- **Current Status**:
  - Implements `grafana/Dashboard` schema (`system:node_title`, `grafana:config`).
  - Endpoints `POST /query`, `POST /dashboards`, `GET /dashboards`.
  - Supports backend aggregation algorithms (`count`, `sum_duration`, `leaderboard`).
  - Web UI renders plain HTML table with dropdowns.
- **Gaps**: Lacks visual chart rendering (line/bar/pie graphs), preset time range selectors (`24h`, `7d`, `30d`), multi-panel custom dashboard layout grid, and a PromQL/Panorama query expression builder.

#### Actionable Next Features
1. **Visual Charting Engine**: Embed SVG/Canvas bar and line charts (e.g. using Recharts or Chart.js) to render time-series trend lines and leaderboard breakdowns.
2. **Quick Time Range Selectors UI**: Interactive time range buttons (`Last 24 Hours`, `Last 7 Days`, `Last 30 Days`, `Custom Range`) that automatically bind to query payloads.
3. **Multi-Panel Drag & Drop Dashboard Builder**: Interactive grid UI allowing users to create, rearrange, resize, and save dashboards containing multiple graph panels.
4. **PromQL / Expression Query Builder**: UI visual query builder for constructing aggregation functions (`sum_over_time`, `rate`, `topk`) over node fields.

#### Possible E2E Tests
```typescript
// E2E Test Suite: Dashboards Next Features

test.describe('Dashboards - Visual Charting Engine', () => {
  test('renders bar chart canvas element for project leaderboard query', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Dashboards")');
    await page.selectOption('select.aggregation-select', 'leaderboard');
    await expect(page.locator('canvas.chart-canvas, svg.recharts-surface')).toBeVisible();
  });
});

test.describe('Dashboards - Quick Time Range Selectors', () => {
  test('updates query results when switching between 24h and 7d time ranges', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Dashboards")');
    await page.click('button:has-text("7 Days")');
    const text7d = await page.locator('.dashboard-metric-total').textContent();
    await page.click('button:has-text("24 Hours")');
    const text24h = await page.locator('.dashboard-metric-total').textContent();
    expect(text7d).not.toEqual(text24h);
  });
});

test.describe('Dashboards - Multi-Panel Builder', () => {
  test('adds a new chart panel and saves dashboard configuration', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Dashboards")');
    await page.click('button:has-text("+ Add Panel")');
    await page.fill('input[placeholder="Panel Title"]', 'Language Distribution');
    await page.click('button:has-text("Save Panel")');
    await page.click('button:has-text("Save Dashboard")');

    await page.reload();
    await expect(page.locator('.dashboard-panel:has-text("Language Distribution")')).toBeVisible();
  });
});

test.describe('Dashboards - Query Expression Builder', () => {
  test('builds and executes a topk query expression', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Dashboards")');
    await page.click('button:has-text("Query Builder")');
    await page.selectOption('select.func-select', 'topk');
    await page.fill('input[placeholder="k value"]', '3');
    await page.click('button:has-text("Run Query")');
    await expect(page.locator('table tbody tr')).toHaveCount(3);
  });
});
```

---

### 2.4 Trip Planner App (`panorama-app-trips`)

#### Evaluation against `DESIGN.md` Goals
- **Goal**: Plan trips with scheduled events, incorporating calendar grid views and interactive map views for geographical event coordinates.
- **Current Status**:
  - Implements `trips/Trip` and `trips/Event` schemas (`system:node_title`, `system:node_start_time`, `system:node_end_time`, `trips:latitude`, `trips:longitude`, `trips:location_name`, `trips:notes`).
  - Endpoints `POST /trips`, `GET /trips`, `POST /events`, `GET /events`, `GET /events/map`.
  - UI allows creating trips, adding events with lat/lng inputs, and viewing location lists.
- **Gaps**: Map view is a plain text list rather than an interactive Leaflet/OpenStreetMap container. Calendar view lacks a visual month/week grid. Lacks travel route sequencing and event attachment support.

#### Actionable Next Features
1. **Interactive Leaflet / OpenStreetMap Container**: Integrate Leaflet.js to render interactive map tiles, pin event markers, display popups, and draw travel route polylines.
2. **Visual Calendar Grid View**: Render an interactive monthly/weekly calendar grid sorting events by `system:node_start_time` and `system:node_end_time`.
3. **Itinerary Drag-and-Drop Sequencer**: Interface to re-order daily trip events and auto-calculate distance between successive coordinates.
4. **Trip Event Attachments**: Support linking object storage files (flight tickets, hotel reservations, photos) directly to event nodes via `ObjectRef`.

#### Possible E2E Tests
```typescript
// E2E Test Suite: Trip Planner Next Features

test.describe('Trip Planner - Leaflet Map Integration', () => {
  test('renders interactive map container and pins event markers', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Trip Planner")');
    await page.click('button:has-text("Map View")');

    await expect(page.locator('.leaflet-container')).toBeVisible();
    await expect(page.locator('.leaflet-marker-icon')).toBeVisible();
  });
});

test.describe('Trip Planner - Calendar Grid View', () => {
  test('displays scheduled trip events in month grid cell', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Trip Planner")');
    await page.click('button:has-text("Calendar View")');

    await expect(page.locator('.calendar-month-grid')).toBeVisible();
    await expect(page.locator('.calendar-event-pill')).toBeVisible();
  });
});

test.describe('Trip Planner - Itinerary Sequencer', () => {
  test('reorders events and recalculates daily itinerary sequence', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Trip Planner")');
    await page.click('.trip-card:first-child');
    await page.dragAndDrop('.event-item:nth-child(2)', '.event-item:nth-child(1)');

    await page.click('button:has-text("Save Itinerary")');
    await expect(page.locator('.event-item:first-child')).toContainText('Event 2');
  });
});

test.describe('Trip Planner - Event Media Attachments', () => {
  test('attaches boarding pass PDF to an event and renders attachment link', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Trip Planner")');
    await page.click('.event-item:first-child');
    await page.setInputFiles('input[type="file"].event-attachment-input', {
      name: 'ticket.pdf',
      mimeType: 'application/pdf',
      buffer: Buffer.from('%PDF-1.4 ...')
    });

    await expect(page.locator('.attachment-chip:has-text("ticket.pdf")')).toBeVisible();
  });
});
```

---

### 2.5 Beli Alternative App (`panorama-app-beli`)

#### Evaluation against `DESIGN.md` Goals
- **Goal**: Rate restaurants using a PARTIAL ORDERING model (pairwise comparisons $A > B$) computed via topological sorting into tiers, rather than arbitrary 5-star ratings. Pull info from OSM.
- **Current Status**:
  - Implements `beli/Restaurant` and `beli/Comparison` schemas.
  - Implements Kahn's algorithm in `compute_rankings()` to generate topological tiers.
  - Endpoints `POST /restaurants`, `GET /restaurants`, `POST /compare`, `GET /rankings`.
  - Basic UI for adding restaurants, submitting comparisons, and viewing tier lists.
- **Gaps**: Lacks OpenStreetMap / Overpass API integration for restaurant auto-complete, conflict/cycle resolution UI for contradictory comparisons ($A > B > C > A$), context-based rankings (e.g. "Best Pizza" vs "Best Sushi"), and an interactive pairwise matchmaker wizard.

#### Actionable Next Features
1. **OpenStreetMap / Overpass API Integration**: Search input connected to OSM to auto-fill restaurant name, cuisine type, address, and GPS location.
2. **Cycle & Conflict Resolution Modal**: Detect directed graph cycles when users record conflicting comparisons and offer an interactive UI modal to resolve the conflict.
3. **Category-Specific Partial Orders**: Filter comparisons and calculate distinct ranking DAGs by context/cuisine (`beli:context`, e.g., "Ramen", "Cocktail Bars").
4. **Interactive Pairwise Matchmaker Wizard**: "Matchmaker" UI mode that presents uncompared restaurant pairs ("Which was better?") to guide the user in completing the partial order graph efficiently.

#### Possible E2E Tests
```typescript
// E2E Test Suite: Beli Next Features

test.describe('Beli - OSM Search Integration', () => {
  test('searches OSM and auto-populates restaurant location and cuisine', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Beli")');
    await page.fill('input[placeholder*="Search OSM"]', 'Joe\'s Pizza NYC');
    await page.click('button:has-text("Search")');
    await page.click('.osm-result-item:first-child');

    await expect(page.locator('input[placeholder="Restaurant name"]')).toHaveValue('Joe\'s Pizza');
    await expect(page.locator('input[placeholder="Cuisine"]')).toHaveValue('Pizza');
  });
});

test.describe('Beli - Cycle Resolution UI', () => {
  test('detects comparison cycle and prompts user to resolve conflict', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Beli")');

    // Submit cyclic comparison
    await page.selectOption('select.better-select', 'RestA');
    await page.selectOption('select.worse-select', 'RestB');
    await page.click('button:has-text("Compare")');

    await page.selectOption('select.better-select', 'RestB');
    await page.selectOption('select.worse-select', 'RestA');
    await page.click('button:has-text("Compare")');

    await expect(page.locator('.conflict-modal')).toBeVisible();
    await expect(page.locator('.conflict-modal')).toContainText('Cycle detected');
  });
});

test.describe('Beli - Category-Specific Rankings', () => {
  test('filters partial order tiers by cuisine category', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Beli")');
    await page.selectOption('select.context-filter', 'Ramen');

    await expect(page.locator('.tier-container')).toBeVisible();
    await expect(page.locator('.restaurant-card')).toHaveAttribute('data-cuisine', 'Ramen');
  });
});

test.describe('Beli - Matchmaker Wizard', () => {
  test('presents unranked pair and updates tier ranking upon user pick', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Beli")');
    await page.click('button:has-text("Start Matchmaker")');

    await expect(page.locator('.matchmaker-card-a')).toBeVisible();
    await expect(page.locator('.matchmaker-card-b')).toBeVisible();
    await page.click('.matchmaker-card-a');

    await expect(page.locator('.matchmaker-toast')).toContainText('Comparison recorded');
  });
});
```

---

### 2.6 Subsonic Music Streaming App (`panorama-app-subsonic`)

#### Evaluation against `DESIGN.md` Goals
- **Goal**: Subsonic-compatible music server interface for streaming audio files stored in object storage.
- **Current Status**:
  - Implements `subsonic/Artist`, `subsonic/Album`, `subsonic/Track` schemas.
  - Stores audio blobs in object storage via `put_object("subsonic-audio", ...)` with `subsonic:audio_ref`.
  - Endpoints `GET /rest/ping`, `GET /rest/getArtists`, `GET /rest/getAlbumList2`, `GET /rest/stream`, `POST /upload`.
  - Web UI lists artists and album metadata.
- **Gaps**: Web UI lacks an embedded HTML5 audio player widget (play/pause/seek controls). Missing key Subsonic client endpoints (`getMusicFolders`, `getSong`, `search3`, `getCoverArt`, `scrobble`). Upload endpoint relies on query string parameters instead of automatic ID3 audio tag parsing.

#### Actionable Next Features
1. **Embedded Web Audio Player Widget**: Persistent HTML5 audio player bar with play/pause, volume control, scrub bar, current track info, and playback queue management.
2. **Automatic ID3 Tag & Cover Art Extractor**: Server-side parsing of uploaded MP3/FLAC audio files to extract ID3 tags (artist, album, track title, track number, duration) and store embedded cover art images in object storage.
3. **Subsonic Client API Expansion**: Implement missing API endpoints (`/rest/getSong`, `/rest/search3`, `/rest/getCoverArt`, `/rest/scrobble`) to support third-party mobile clients (e.g. Navidrome, Dsub, Ultrasonic).
4. **Playlists & Favorite Tracks Management**: Allow creating, editing, and streaming custom user playlist nodes.

#### Possible E2E Tests
```typescript
// E2E Test Suite: Subsonic Music Next Features

test.describe('Subsonic - Web Audio Player Widget', () => {
  test('plays audio track and updates persistent player UI state', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Subsonic Music")');
    await page.click('.track-row:first-child .btn-play');

    await expect(page.locator('.audio-player-bar')).toBeVisible();
    await expect(page.locator('.audio-player-bar .now-playing-title')).toBeVisible();
    await expect(page.locator('.audio-player-bar button.btn-pause')).toBeVisible();
  });
});

test.describe('Subsonic - Automatic ID3 Tag Extraction', () => {
  test('extracts metadata from uploaded MP3 file and creates artist/album nodes', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Subsonic Music")');
    await page.setInputFiles('input[type="file"].music-upload-input', {
      name: 'sample-track.mp3',
      mimeType: 'audio/mpeg',
      buffer: Buffer.from('ID3...') // Mock ID3 binary data
    });

    await expect(page.locator('h4:has-text("Artists") + div')).toContainText('Sample Artist');
    await expect(page.locator('h4:has-text("Albums") + div')).toContainText('Sample Album');
  });
});

test.describe('Subsonic - Client API Endpoint Compatibility', () => {
  test('serves cover art image via /rest/getCoverArt endpoint', async ({ request }) => {
    const response = await request.get('/plugin/com.panorama.subsonic/rest/getCoverArt?id=album-123');
    expect(response.status()).toBe(200);
    expect(response.headers()['content-type']).toMatch(/image\/(jpeg|png)/);
  });
});

test.describe('Subsonic - Playlist Management', () => {
  test('creates playlist node and adds tracks', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Subsonic Music")');
    await page.click('button:has-text("+ New Playlist")');
    await page.fill('input[placeholder="Playlist Name"]', 'Chill Beats');
    await page.click('button:has-text("Create")');

    await expect(page.locator('.playlist-card:has-text("Chill Beats")')).toBeVisible();
  });
});
```

---

### 2.7 File Manager App (`panorama-app-files`)

#### Evaluation against `DESIGN.md` Goals
- **Goal**: Support uploading files, resumable transfers, and managing large files in object storage.
- **Current Status**:
  - Implements `files/File` schema (`system:node_title`, `files:object_ref`, `files:file_size`, `files:mime_type`, `files:folder`).
  - Endpoints `POST /upload`, `POST /upload/initiate`, `GET /files`, `GET /files/{id}`, `DELETE /files/{id}`.
  - Downloads serve correct `Content-Disposition: attachment` headers.
  - Web UI features drag-and-drop zone and file list.
- **Gaps**: Resumable upload UI and endpoint logic (`POST /upload/initiate`) is currently a stub pointing to `/api/uploads` without chunked client upload handling. Lacks inline file preview modals, folder tree navigation, and bulk actions (multi-delete, zip download).

#### Actionable Next Features
1. **Resumable Chunked Upload Manager**: Client-side chunked uploader utilizing `/api/uploads` with progress tracking, pause/resume controls, and retry handling for large files.
2. **In-Browser File Previewer Modal**: Inline preview modal supporting image display (PNG/JPG/WEBP), PDF document viewing, text/code inspection, and audio/video playback.
3. **Folder Tree Navigation & Drag-and-Drop Organization**: Interactive folder sidebar allowing directory creation, renaming, and drag-and-drop file relocation.
4. **Bulk Selection & ZIP Download**: Multi-select toolbar for batch deleting files or downloading selected files as a ZIP archive.

#### Possible E2E Tests
```typescript
// E2E Test Suite: File Manager Next Features

test.describe('File Manager - Resumable Chunked Upload Manager', () => {
  test('executes chunked upload with pause and resume controls', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("File Manager")');
    await page.setInputFiles('input[type="file"].file-upload-input', {
      name: 'large-video.mp4',
      mimeType: 'video/mp4',
      buffer: Buffer.alloc(5 * 1024 * 1024) // 5MB mock buffer
    });

    await expect(page.locator('.upload-progress-bar')).toBeVisible();
    await page.click('button:has-text("Pause")');
    await expect(page.locator('.upload-status')).toHaveText('Paused');
    await page.click('button:has-text("Resume")');
    await expect(page.locator('.upload-status')).toHaveText('Completed');
  });
});

test.describe('File Manager - In-Browser File Previewer', () => {
  test('opens image file preview modal when clicking file row', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("File Manager")');
    await page.click('.file-row[data-mime^="image/"]');

    await expect(page.locator('.preview-modal')).toBeVisible();
    await expect(page.locator('.preview-modal img.preview-image')).toBeVisible();
  });
});

test.describe('File Manager - Folder Tree Navigation', () => {
  test('creates a folder and drags file into folder', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("File Manager")');
    await page.click('button:has-text("New Folder")');
    await page.fill('input[placeholder="Folder name"]', 'Documents');
    await page.click('button:has-text("Create Folder")');

    await page.dragAndDrop('.file-row:first-child', '.folder-item:has-text("Documents")');
    await page.click('.folder-item:has-text("Documents")');
    await expect(page.locator('.file-row')).toHaveCount(1);
  });
});

test.describe('File Manager - Bulk Selection & ZIP Download', () => {
  test('selects multiple files and triggers bulk delete', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("File Manager")');
    await page.check('.file-row:nth-child(1) input[type="checkbox"]');
    await page.check('.file-row:nth-child(2) input[type="checkbox"]');

    page.once('dialog', dialog => dialog.accept());
    await page.click('button:has-text("Delete Selected (2)")');
    await page.waitForTimeout(500);

    await expect(page.locator('.file-row')).toHaveCount(0);
  });
});
```

---

## 3. Summary Roadmap & Recommended Implementation Order

To maintain steady progress towards full alignment with [DESIGN.md](file:///home/michael/Projects/panorama/DESIGN.md), the recommended execution priority is:

1. **Phase 1: High Impact UI/UX Enhancements**
   - Interactive Leaflet maps in **Trip Planner**
   - Canvas/SVG charting in **Dashboards**
   - HTML5 Audio Player in **Subsonic Music**
2. **Phase 2: Data & Core Workflow Features**
   - Block-level paragraph child node breakdown in **Journal**
   - OpenStreetMap live search in **Beli**
   - Automatic ID3 tag extraction in **Subsonic Music**
3. **Phase 3: Resilience & Platform Depth**
   - Chunked resumable upload UI manager in **File Manager**
   - Automatic heartbeat duration/idle tracking in **Wakatime**
   - Cycle detection and conflict resolution UI in **Beli**
