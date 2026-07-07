# Coding App Gap Analysis — Wakapi vs panorama-app-coding

**Date:** 2026-07-07
**Comparison baseline:** [wakapi](https://github.com/muety/wakapi) (cloned to `3rd-party/wakapi/`) vs `crates/panorama-app-coding/` v0.2.0

---

## Executive Summary

`panorama-app-coding` implements the **heartbeat ingestion + basic stats query** core of a WakaTime-compatible backend (~30% feature coverage). It is missing the full WakaTime-compatible API surface, heartbeat deduplication, User-Agent parsing, editor/OS tracking, badges, leaderboards, WakaTime relay/import, data retention, and a rich dashboard. Multi-user support is explicitly out of scope for now. The biggest architectural blocker is:

1. **Background task scheduler is not implemented** — the daily summary rollup, leaderboard recalculation, and data cleanup never actually run.

---

## Gap Severity Legend

| Tag | Meaning |
|---|---|
| 🔴 Critical | Core functionality missing; feature is non-functional without it |
| 🟡 Major | Important feature; significantly limits usefulness |
| 🟢 Minor | Nice-to-have, polish, edge case |
| `[needs API]` | Blocked on Panorama plugin API changes (framework work) |
| `[app-side]` | Implementable entirely within the coding app today |
| `[PQL limit]` | Blocked on PQL query language lacking features (aggregation, etc.) |

---

## 1. Heartbeat Processing Pipeline 🟡 `[app-side]`

The coding app accepts and stores heartbeats, but is missing critical processing steps that Wakapi performs:

| Processing Step | Coding App | Wakapi | Impact |
|---|---|---|---|
| **Placeholder resolution** (`<<LAST_PROJECT>>`, `<<LAST_LANGUAGE>>`, `<<LAST_BRANCH>>`) | ❌ | ✅ Queries user's most recent heartbeats | Editor plugins rely on this |
| **User-Agent parsing** → extract Editor + OS | ❌ | ✅ Parses UA string to set editor/OS fields | No editor/OS dimension data |
| **Deduplication by hash** (xxhash over relevant fields) | ❌ | ✅ Unique index on hash, silently drops dupes | Duplicate heartbeats inflate stats |
| **Language mapping** (file extension → language) | ❌ | ✅ Configurable globally and per-user | `.tsx` files won't be recognized as TypeScript |
| **Canonical name normalization** (editor/OS/language) | ❌ | ✅ e.g. "VSCode" → "VS Code", "java" → "Java" | Stats fragmentation from inconsistent names |
| **Entity sanitization** (canonical path forms) | ❌ | ✅ | |
| **Heartbeat timeout** | ⚠️ Hardcoded 900s (15 min) | ✅ Configurable per-user, default 600s (10 min) | Different session grouping |
| **Bulk heartbeat max** | 25 | 50 | Minor |
| **Heartbeat max age validation** | ❌ | ✅ Rejects >180 days old | Could accept garbage timestamps |
| **WakaTime relay** (forward to WakaTime API) | ❌ | ✅ Optional per-user forwarding | |
| **Category auto-assignment** (domain→browsing, file→coding) | ❌ | ✅ | |
| **WakaTime CLI diagnostics** (`POST /api/plugins/errors`) | ❌ | ✅ | |

### Missing Schema Fields

These WakaTime heartbeat fields have **no corresponding field** in the coding app's Heartbeat schema:

| Field | Wakapi Field | Should map to |
|---|---|---|
| Editor | `editor` | `coding:editor` (new) |
| Operating System | `operating_system` | `coding:operating_system` (new) |
| User Agent | `user_agent` | `coding:user_agent` (new) |
| Dedup hash | `hash` | `coding:hash` (new) |
| Origin | `origin` | `coding:origin` (new) |
| Line additions (human) | `line_additions` | Already have `coding:human_line_changes` but no split |
| Line deletions (human) | `line_deletions` | Already have `coding:human_line_changes` but no split |
| Created at (server) | `created_at` | `system:created_at` (new) |

Note: the coding app already has fields for `coding:is_write`, `coding:lines`, `coding:lineno`, `coding:cursorpos`, and all AI fields — these are up to parity.

---

## 2. Data Dimensions Tracked 🟡 `[app-side]`

Wakapi tracks 9 dimensions for statistics. The coding app covers some but misses key ones:

| Dimension | In Coding Schema? | In Stats `group_by`? | In UI? |
|---|---|---|---|
| Project | ✅ `coding:project` | ✅ | ✅ Leaderboard bar |
| Language | ✅ `coding:language` | ✅ | ✅ Leaderboard bar |
| Entity (file) | ✅ `coding:entity` | ✅ | ✅ Leaderboard bar |
| Category | ✅ `coding:category` | ✅ | ❌ Not shown |
| Branch | ✅ `coding:branch` | ✅ | ❌ Not shown |
| Machine | ✅ `coding:machine_name_id` | ✅ | ❌ Not shown |
| Date | ✅ (computed from time) | ✅ | ❌ Not shown |
| Hour | ❌ No field | ✅ (computed) | ❌ Not shown |
| **Editor** | ❌ **Not in schema** | ❌ | ❌ |
| **Operating System** | ❌ **Not in schema** | ❌ | ❌ |
| **Label** (project grouping) | ❌ **Not in schema** | ❌ | ❌ |

---

## 3. API Endpoint Coverage 🟡 `[app-side]`

### 4a. Currently Implemented (12 endpoints)

#### WakaTime-Compatible
| Method | Path | Status |
|---|---|---|
| POST | `/users/current/heartbeats` | ✅ Single heartbeat |
| POST | `/users/current/heartbeats.bulk` | ✅ Bulk (max 25) |
| GET | `/users/current/heartbeats?date=` | ✅ |
| GET | `/users/current/durations?date=` | ✅ |
| DELETE | `/users/current/heartbeats.bulk` | ✅ |

#### Shorthand/Convenience
| Method | Path | Status |
|---|---|---|
| POST | `/heartbeat` | ✅ |
| POST | `/heartbeats` | ✅ |
| GET | `/heartbeats?date=` | ✅ |
| GET | `/durations?date=` | ✅ |

#### Stats/Summaries (custom, not WakaTime-compatible)
| Method | Path | Status |
|---|---|---|
| GET | `/stats` | ✅ Custom params |
| POST | `/stats` | ✅ Custom JSON body |
| GET | `/summaries?range=` | ✅ Custom format |

### 4b. Missing WakaTime-Compatible Endpoints

These are needed for compatibility with existing WakaTime editor plugins and CLI tools:

| Endpoint | Wakapi Handler | Priority | Notes |
|---|---|---|---|
| `GET /users/{user}/stats[/{range}]` | `StatsHandler` | 🔴 | **This is THE main WakaTime API** — returns projects, languages, editors, OS, machines all at once in WakaTime format. Editor plugins call this. |
| `GET /users/{user}/stats` (auto-range) | `StatsHandler` | 🟡 | Stats with auto-detected range |
| `GET /users/{user}/summaries` | `SummariesHandler` | 🟡 | WakaTime-format daily summaries |
| `GET /users/{user}/all_time_since_today` | `AllTimeHandler` | 🟡 | Simple total-seconds response |
| `GET /users/{user}/projects` | `ProjectsHandler` | 🟡 | Project list with first/last/count/top-language |
| `GET /users/{user}/projects/{id}` | `ProjectsHandler` | 🟢 | Single project detail |
| `GET /users/{user}/user_agents` | `UserAgentsHandler` | 🟢 | List of user agents used |
| `GET /users/{user}/statusbar/{range}` | `StatusBarHandler` | 🟡 | Status bar widget data for desktop |
| `GET /users/{user}` | `UsersHandler` | 🟡 | User profile with latest heartbeat |

### 4c. Missing Badge/Chart Endpoints

| Endpoint | Purpose | Priority |
|---|---|---|
| `GET /badge/{user}/*` | SVG badge for coding time (Shields.io style) | 🟡 |
| `GET /compat/shields/v1/{user}/{interval}/{filter}` | JSON endpoint for Shields.io | 🟡 |
| `GET /activity/chart/{user}.svg` | GitHub-style activity heatmap SVG | 🟡 |

### 4d. Missing Misc API Endpoints

| Endpoint | Purpose |
|---|---|
| `GET /health` | Health check (app + DB status) |
| `GET /metrics` | Prometheus metrics |
| `POST /plugins/errors` | WakaTime CLI diagnostics |
| `GET /avatar/{hash}.svg` | Avatar generation |

### 4e. Missing WakaTime Compat Route Aliases

Wakapi mounts the same handlers under multiple path prefixes for maximum compatibility:
- `/api/heartbeat` — native
- `/api/users/{user}/heartbeats` — user-scoped
- `/api/v1/users/{user}/heartbeats` — v1 compat
- `/api/compat/wakatime/v1/users/{user}/heartbeats` — full compat

The coding app only mounts `/heartbeat` and `/heartbeats` shorthand + `/users/current/heartbeats` and `/users/current/heartbeats.bulk`.

---

## 4. Stats Engine 🟡 `[app-side]` + 🟡 `[PQL limit]`

### 5a. What the Coding App Stats Engine DOES Support

The custom `/stats` endpoint has a rich parameter set:

| Parameter | Values |
|---|---|
| `range` | `24h`, `7d`, `30d`, `90d`, `365d`, `all`, `YYYY-MM-DD`, `YYYY-MM-DD..YYYY-MM-DD` |
| `group_by` | `project`, `language`, `entity`, `category`, `date`, `hour`, `machine`, `branch` |
| `sub_group_by` | Any of the above (two-level grouping) |
| `aggregation` | `leaderboard`, `sum`, `count`, `avg_daily`, `timeseries` |
| `filter` | Simple `field=value` filter |
| `bucket` | `hour`, `day`, `week` (for timeseries) |
| `limit` | Integer, default 25 |

Aggregation modes:
- `leaderboard` — top N by total_seconds, sorted descending, with sub_group_by support
- `sum` — map of key → total_seconds
- `count` — count of heartbeats per group
- `avg_daily` — average per day
- `timeseries` — bucketed with gap-filling (zeros for missing buckets)

### 5b. What Wakapi's Stats Engine Does

The WakaTime-compatible stats endpoint (`GET /users/{user}/stats/{range}`) returns a fixed-format response:

```json
{
  "data": {
    "total_seconds": 12345.0,
    "daily_average": 1763.0,
    "human_readable_daily_average": "29 mins",
    "categories": [
      { "name": "coding", "total_seconds": 10000.0, "percent": 81.0 }
    ],
    "projects": [
      { "name": "panorama", "total_seconds": 8000.0, "percent": 64.8 }
    ],
    "languages": [ ... ],
    "editors": [ ... ],
    "operating_systems": [ ... ],
    "machines": [ ... ],
    "best_day": { "date": "2026-07-01", "total_seconds": 5000.0 }
  }
}
```

The coding app's `/stats` endpoint returns a different (custom) format — it's not WakaTime-compatible. Editor plugins that call `/users/{user}/stats/last_7_days` will get a 404.

### 5c. Stats Features Missing

| Feature | Coding App | Wakapi |
|---|---|---|
| WakaTime-compatible response format | ❌ Custom format only | ✅ Exact WakaTime format |
| `best_day` calculation | ❌ | ✅ |
| `daily_average` computation | ✅ (separate mode) | ✅ (built into stats) |
| `human_readable_*` fields | ❌ | ✅ |
| `percent` per category | ❌ | ✅ |
| `total_seconds` global + per-category | ❌ | ✅ All-in-one response |
| Per-project stats (first/last/count/top-language) | ❌ | ✅ Separate endpoint |
| Hourly breakdown (24h distribution) | ❌ | ✅ For ranges >1 day |
| Filter by multiple values (OR filter) | ❌ Single value only | ✅ Comma-separated OR |
| Date range via `start`/`end` params | ❌ Only via `range` string | ✅ Both |
| `recompute` flag (regenerate from raw) | ❌ | ✅ |
| Cached summaries (24h TTL) | ❌ | ✅ Alias-resolved cache |

---

## 5. Badges, Leaderboards & Activity Charts `[app-side]`

| Feature | Coding App | Notes |
|---|---|---|
| SVG badges for coding time | ❌ | Wakapi: `/api/badge/{user}/*` with label, color, interval, filter params |
| Shields.io JSON endpoint | ❌ | Wakapi: `/api/compat/shields/v1/{user}/{interval}/{filter}` |
| Activity chart SVG (GitHub heatmap style) | ❌ | Wakapi: `/api/activity/chart/{user}.svg` |
| Public leaderboard | ❌ | Wakapi: ranked users, configurable scope, per-language aggregation, pagination |

All of these can query the existing heartbeat data — no multi-user isolation needed.

---

## 6. Dashboard / UI `[app-side]`

### 7a. What the Coding App UI Has

A single React page (`CodingApp`) with:
- Time range selector buttons (`24h`, `7d`, `30d`, `90d`)
- "Send Test Heartbeat" textarea + button (for manual testing)
- Per Project leaderboard (horizontal bar chart)
- Per Language leaderboard (horizontal bar chart)
- Time series area chart (SVG, daily buckets)
- Per File leaderboard (horizontal bar chart)
- Error display text

### 7b. What Wakapi's Dashboard Has

Multiple pages rendered server-side with Go templates:

| Page/Feature | What It Shows |
|---|---|
| **Summary** (`/summary`) | Time picker, filter bar, per-category tables with colored bars, hourly breakdown, daily timeline |
| **Projects** (`/projects`) | Per-project stats with search, top language, heartbeat count, last active |
| **Leaderboard** (`/leaderboard`) | Ranked users, per-language aggregation, pagination |
| **Settings** (`/settings`) | Tabs: General, Account, Permissions, Integrations, Projects (aliases/labels/mappings), API Keys, Subscriptions, Advanced, WebAuthn |
| **Landing page** (`/`) | Total hours, users, currently online |
| **Setup wizard** | First-time configuration |

### 7c. Missing UI Features

| Feature | Priority | Notes |
|---|---|---|
| Time picker with custom date range | 🟡 | Currently only preset buttons |
| Filter bar (by project, language, category, etc.) | 🟡 | Backend supports it, UI doesn't expose it |
| Editor breakdown | 🔴 | No data (no editor field in schema) |
| OS breakdown | 🔴 | No data (no OS field in schema) |
| Category breakdown | 🟡 | Data exists, UI doesn't show it |
| Machine breakdown | 🟡 | Data exists, UI doesn't show it |
| Hourly breakdown chart (24h distribution) | 🟡 | Wakapi shows this for multi-day ranges |
| Daily timeline / contribution-style heatmap | 🟡 | Wakapi has per-day bar chart |
| All-time total display | 🟡 | Simple stat card |
| Best day display | 🟢 | |
| Daily average display | 🟡 | Stats engine computes it, UI doesn't show |
| Settings page (aliases, language mappings, etc.) | 🟡 | |
| Projects list page with per-project stats | 🟡 | |
| Duration formatting (WakaTime-style: "3 hrs 15 mins") | 🟢 | Currently shows decimal hours ("3.5h") |

---

## 7. Data Management `[app-side]`

| Feature | Coding App | Notes |
|---|---|---|
| **Duration computation** | ✅ Grouped by 15-min timeout | Wakapi: configurable timeout (default 10 min) |
| **Daily summary rollup** | ⚠️ Declared as background task but **never scheduled** | The server doesn't implement the background task scheduler |
| **Summary regeneration** (from raw heartbeats) | ❌ | Wakapi: "Regenerate summaries" button in settings |
| **Data retention** (auto-delete old heartbeats) | ❌ | Wakapi: configurable months, cron job |
| **Data export** | ❌ | Wakapi: Python download script |
| **Data import from WakaTime** | ❌ | Wakapi: rate-limited batch import |
| **Clear all data** | ❌ (DELETE endpoint removes specific heartbeats) | Wakapi: "Clear data" button |
| **Database optimization** (VACUUM) | ❌ | Wakapi: monthly cron |
| **Warm caches on startup** | ❌ | Wakapi: pre-computes project stats |
| **Cache invalidation** on new heartbeats | ❌ | Wakapi: summary cache invalidated on insert |

---

## 8. Configuration & Settings `[app-side]`

Wakapi has a rich configuration system (YAML + env vars) with these sections that the coding app has no equivalent for:

| Config Section | What It Controls |
|---|---|
| `app.leaderboard_*` | Leaderboard scope, generation time |
| `app.aggregation_time` | When daily summaries are generated |
| `app.data_retention_months` | Auto-delete threshold |
| `app.import_*` | WakaTime import rate limiting |
| `app.custom_languages` | File extension → language mappings |
| `app.canonical_language_names` | Name normalization |

These are app-configuration concerns — some could be plugin config manifest entries, others could be runtime settings stored in nodes.

---

## 9. Integration & Ecosystem Features

| Feature | Priority | Notes |
|---|---|---|
| WakaTime import | 🟢 | Nice-to-have; rate-limited, incremental |
| WakaTime relay | 🟢 | Forward heartbeats to upstream WakaTime |
| GitHub Readme Stats integration | 🟢 | Compat with `github-readme-stats` WakaTime plugin |
| Prometheus metrics (`/api/metrics`) | 🟢 | Global metrics |
| Swagger/OpenAPI docs | 🟢 | Auto-generated API docs |
| Sentry error tracking | 🟢 | |
| Health check endpoint | 🟡 | Simple, useful for monitoring |

---

## 10. Testability — How to Test Properly

### 11a. What Tests Exist Today

The coding app has 4 unit tests in `lib.rs`:
- `test_parse_time_range_relative` — verifies `7d` = 7 days, `24h` = 24 hours
- `test_parse_time_range_explicit` — verifies `YYYY-MM-DD..YYYY-MM-DD` range parsing
- `test_parse_time_range_single_date` — verifies single date `YYYY-MM-DD`
- `test_heartbeat_is_ai` — verifies AI detection logic (ai_session, ai_line_changes, etc.)

There are **no integration tests**, **no e2e tests**, **no API-level tests**, and **no tests for the stats engine**.

### 11b. Recommended Test Pyramid

```
          ╱ E2E ╲          Spin up Panorama server, send real HTTP requests
         ╱  API  ╲         Test each endpoint handler in isolation
        ╱  Stats  ╲        Test the stats engine with known data
       ╱   Unit    ╲       Test individual functions
```

### 11c. Unit Tests Needed (no server required)

| Test Subject | What to Verify |
|---|---|
| `parse_time_range()` | All relative ranges, explicit ranges, single dates, invalid input |
| `ft()` field type inference | Maps field names to correct types |
| `node_duration_seconds()` | Falls back to 120s default when no duration field |
| Heartbeat validation | Missing entity → error, missing time → defaults to now |
| AI detection (`is_ai_generated`) | All trigger fields, none, partial |
| Duration grouping logic | 15-min window, exact boundary, far-apart heartbeats |
| `StatsQuery` deserialization | All params present, defaults, invalid combos |
| Bulk limit enforcement | Exactly 25, 26 → error, 0 → error |

### 11d. Stats Engine Tests (need test data but no HTTP)

The stats engine is the most complex piece and currently has **zero tests**. This is the highest-risk area.

| Test | Setup | Assertions |
|---|---|---|
| Leaderboard by project | Insert 10 heartbeats across 3 projects with varying durations | Correct ranking, correct total_seconds, correct hours rounding |
| Leaderboard limit | Insert 50 heartbeats across 30 projects | Only top N returned, correct cutoff |
| Timeseries with gap filling | Insert heartbeats on days 1, 3, 5 (skip 2, 4) | Missing days have 0 seconds, correct bucket boundaries |
| Timeseries bucket sizes | Same data, queried with hour/day/week | Different granularity, correct binning |
| Filter by project | Heartbeats for "proj-a" and "proj-b" | Only matching project returned |
| Sub-group-by (project → language) | Heartbeats with varying project+language combos | Correct two-level nesting |
| `avg_daily` aggregation | Heartbeats spanning 3 days | Correct daily average per group |
| `count` aggregation | Known heartbeats | Correct count per group |
| `sum` aggregation | Known durations | Correct sum per group |
| Range boundaries | Heartbeat exactly at range start/end | Inclusive/exclusive correct |
| Empty result | No heartbeats in range | Empty result, not error |
| Duration fallback (120s default) | Heartbeats with no duration field | Each counts as 120s |

### 11e. API-Level Integration Tests

Test each HTTP endpoint handler by calling `handle_http_request()` directly with a mock/fake `PluginContext`:

| Endpoint | Test Cases |
|---|---|
| `POST /heartbeat` | Valid single heartbeat → 201; missing entity → 400; extra fields preserved; all optional fields round-trip; AI fields → is_ai_generated=true |
| `POST /heartbeats` (bulk) | 2 heartbeats → both stored; 25 → ok; 26 → error; empty array → error |
| `GET /heartbeats?date=` | Date with data → returns list; date with no data → empty list; missing date param → error |
| `GET /durations?date=` | Heartbeats within 15min → merged; heartbeats 16min apart → separate; correct duration_seconds |
| `DELETE /heartbeats.bulk` | Delete by ID list; delete all for date |
| `GET /stats` (all aggregations) | Each aggregation mode with known test data |
| `GET /summaries` | With and without pre-computed summaries |

### 11f. E2E Tests (full server)

Spin up the Panorama server with the coding app loaded:

| Scenario | Steps |
|---|---|
| Heartbeat → stats round-trip | 1. POST heartbeat 2. GET stats → heartbeat contributes to totals |
| Bulk heartbeat ingestion | 1. POST 5 heartbeats 2. GET heartbeats for date → all 5 present |
| Duration computation | 1. POST 3 heartbeats 2min apart 2. GET durations → 1 duration with duration = ~4min |
| Time range filtering | 1. POST heartbeat with time 7 days ago 2. GET stats range=24h → heartbeat not counted 3. GET stats range=30d → heartbeat counted |
| AI detection round-trip | 1. POST heartbeat with ai_session set 2. Query heartbeats → is_ai_generated=true |

### 11g. Test Data Generator

Wakapi includes `scripts/sample_data.py` for generating test data. The coding app would benefit from a similar utility — either:
- A Rust test helper that inserts N heartbeats with randomized realistic data
- A shell script that curls the heartbeat endpoint in a loop
- A Bun/Node script (matching the project's `bun` preference)

A good test data generator should produce:
- Multiple projects with realistic names
- Multiple languages per project
- Varying durations (30s to 2h)
- Realistic timestamps spread across a configurable date range
- Both AI and non-AI heartbeats
- Multiple machines/editors (once those fields are added)

### 11h. Wakapi Itself as a Test Oracle

Since wakapi is cloned to `3rd-party/wakapi/`, a powerful testing strategy:
1. Start a local wakapi instance (it's a single Go binary with SQLite)
2. Send identical heartbeat data to both wakapi and the coding app
3. Compare the stats/summaries/durations responses
4. Flag any discrepancies

This gives you a **ground truth** for WakaTime compatibility without needing to reverse-engineer the spec.

---

## 11. Prioritized Implementation Roadmap

### Phase 1: Framework Prerequisites `[needs API]`
1. Implement the background task scheduler in the server (blocker for daily summary rollup, leaderboard recalculation, data retention)
2. Add aggregation support to PQL (COUNT, SUM, GROUP BY) — currently the stats engine does aggregation in Rust by fetching ALL nodes and computing in memory, which won't scale

### Phase 2: Heartbeat Pipeline & Schema 🔴
1. Add `coding:editor`, `coding:operating_system`, `coding:user_agent`, `coding:hash` to the Heartbeat schema
2. Implement User-Agent parsing (extract editor + OS from UA string)
3. Implement heartbeat deduplication (xxhash over relevant fields, silently drop dupes)
4. Implement placeholder resolution (`<<LAST_PROJECT>>`, `<<LAST_LANGUAGE>>`, `<<LAST_BRANCH>>`)
5. Implement language mapping (file extension → language)
6. Implement canonical name normalization (editor/OS/language names)
7. Add editor and OS to the stats engine `group_by` options

### Phase 3: WakaTime-Compatible API 🔴
1. Implement `GET /users/{user}/stats[/{range}]` — THE main WakaTime stats endpoint
2. Implement `GET /users/{user}/summaries` — WakaTime-format summaries
3. Implement `GET /users/{user}/all_time_since_today` — all-time total
4. Implement `GET /users/{user}/projects` and `/projects/{id}` — project list with stats
5. Implement `GET /users/{user}/statusbar/{range}` — status bar widget data
6. Add WakaTime-compatible route aliases (`/api/v1/...`, `/api/compat/wakatime/v1/...`)

### Phase 4: Dashboard & UI Expansion 🟡
1. Add editor, OS, category, machine breakdowns to the UI
2. Add hourly breakdown chart (24h activity distribution)
3. Add daily timeline / contribution view
4. Add filter bar (by project, language, category, etc.)
5. Add time picker with custom date range
6. Add best day, daily average, all-time total stat cards
7. Add category auto-assignment (domain→browsing, file→coding)

### Phase 5: Badges, Leaderboards & Activity Charts 🟡
1. Implement SVG badge endpoint (`/badge/{user}/*`)
2. Implement Shields.io JSON endpoint
3. Implement GitHub-style activity chart SVG
4. Implement public leaderboard (ranked by coding time, configurable scope)

### Phase 6: Data Management & Polish 🟢
1. Implement data retention/cleanup (auto-delete old heartbeats)
2. Health check endpoint
3. Prometheus metrics
4. Swagger/OpenAPI docs
5. Configurable heartbeat timeout
6. Language mappings config (extension → language, global)
7. Aliases (entity name normalization)
8. Project labels (grouping)
9. WakaTime import (rate-limited batch import from WakaTime API)
10. WakaTime relay (forward heartbeats to upstream WakaTime)

---

## 12. Key Architectural Differences

| Aspect | Wakapi | panorama-app-coding |
|---|---|---|
| **Language** | Go | Rust (compiled to WASM) |
| **Database** | GORM: SQLite/MySQL/Postgres | Node store via `PluginContext` (SQLite-backed) |
| **Query model** | Direct SQL via GORM | PQL (Panorama Query Language) — in-memory aggregation for stats |
| **Frontend** | Server-rendered Go templates + TailwindCSS | React 19 SPA via Module Federation |
| **Deployment** | Standalone binary | `.panoapp` plugin loaded by Panorama server |
| **Background jobs** | Cron-based (robfig/cron) | Declared in API but scheduler not implemented |
| **Caching** | In-memory Go map with TTL | None |
| **Scaling** | Single binary, can run behind load balancer with shared DB | Limited by single SQLite + WASM runtime |

---

## 13. Summary Statistics

| Category | Wakapi | Coding App | Coverage |
|---|---|---|---|
| API endpoints (total) | ~45 | 12 | 27% |
| WakaTime-compat endpoints | 11 | 2 (partial) | ~18% |
| Data dimensions tracked | 9 | 6 (+ date/hour computed) | ~67% |
| Heartbeat processing steps | 10 | 3 | 30% |
| UI pages/views | 8 | 1 | 13% |
| Badge/chart endpoints | 3 | 0 | 0% |
| Background jobs | 7 | 1 (declared, never runs) | ~14% |
| Configurable settings | 20+ | 0 | 0% |
| Test coverage | Unknown | 4 unit tests | — |

**Overall feature coverage: ~30%** of Wakapi's feature set (multi-user intentionally excluded).
