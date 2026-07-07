# Coding Activity Plugin

WakaTime-compatible coding activity tracker for Panorama. Collects heartbeats from
editor plugins and the WakaTime CLI, with full stats, leaderboards, and dashboards.

## Editor Setup

Configure your editor's WakaTime plugin to send heartbeats to Panorama by creating
or editing `~/.wakatime.cfg`:

```ini
[settings]
# Point at your Panorama server. The plugin ID is part of the URL.
# If Panorama runs on a different host/port, change accordingly.
api_url = http://localhost:3000/plugin/io.mzhang.panorama.coding

# Any value works — Panorama doesn't require auth yet.
api_key = panorama
```

### What this does

The WakaTime plugin sends heartbeats to:
```
{api_url}/users/current/heartbeats
{api_url}/users/current/heartbeats.bulk
```

Which resolves to:
```
http://localhost:3000/plugin/io.mzhang.panorama.coding/users/current/heartbeats
http://localhost:3000/plugin/io.mzhang.panorama.coding/users/current/heartbeats.bulk
```

Both are handled by the coding plugin's WakaTime-compatible ingest endpoint.

### Per-project overrides

You can send different projects to different servers:

```ini
[settings]
api_key = panorama

[api_urls]
# Send everything to Panorama
.* = http://localhost:3000/plugin/io.mzhang.panorama.coding|panorama
```

### Verifying it works

1. Start Panorama: `cargo run -p panorama-server`
2. Open http://localhost:3000, click "Coding Activity" in the sidebar
3. Start coding in your editor — heartbeats appear as stats within seconds
4. Or test manually:

```bash
curl -X POST http://localhost:3000/plugin/io.mzhang.panorama.coding/heartbeat \
  -H 'Content-Type: application/json' \
  -d '{"entity":"/src/main.rs","type":"file","language":"Rust","project":"panorama","time":'$(date +%s)'}'
```

Then check:
```bash
curl http://localhost:3000/plugin/io.mzhang.panorama.coding/compat/wakatime/v1/users/current/stats/7d | jq .
```

## API Endpoints

### WakaTime-compatible (matches `~/.wakatime.cfg` config)

| Method | Path |
|--------|------|
| POST | `/users/{user}/heartbeats` |
| POST | `/users/{user}/heartbeats.bulk` |
| GET | `/users/{user}/stats/{range}` |
| GET | `/users/{user}/summaries` |
| GET | `/users/{user}/all_time_since_today` |
| GET | `/users/{user}/projects` |
| GET | `/users/{user}/statusbar/{range}` |
| GET | `/compat/wakatime/v1/users/{user}/...` |
| GET | `/compat/wakatime/v1/leaders` |

### Convenience paths (no user prefix)

| Method | Path |
|--------|------|
| POST | `/heartbeat` |
| POST | `/heartbeats` |
| GET | `/stats` |
| GET | `/summaries` |

### Badges & Charts

| Method | Path |
|--------|------|
| GET | `/badge/{user}/{*rest}` — SVG coding-time badge |
| GET | `/compat/shields/v1/{user}/{interval}/{filter}` — Shields.io JSON |
| GET | `/activity/chart/{user}.svg` — GitHub-style heatmap |
