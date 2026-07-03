// Wakatime Plugin — React UI component
// Shows coding activity dashboard with project breakdown.
// Loaded by the Panorama host via Module Federation at runtime.

import { useState, useEffect } from 'react'

// ── Minimal API helper (self-contained; no dependency on host client) ────────

async function callPluginEndpoint(
  pluginId: string,
  endpoint: string,
  method = 'GET',
  body?: unknown,
): Promise<Response> {
  const opts: RequestInit = {
    method,
    headers: body ? { 'Content-Type': 'application/json' } : {},
    body: body ? JSON.stringify(body) : undefined,
  }
  return fetch(`/plugin/${pluginId}/${endpoint}`, opts)
}

// ── Types ───────────────────────────────────────────────────────────────────

interface Node {
  id: string
  fields: Record<string, { type: string; value: unknown }>
  updated_at: string
}

interface WakatimeAppProps {
  pluginId: string
}

// ── Component ───────────────────────────────────────────────────────────────

export default function WakatimeApp({ pluginId }: WakatimeAppProps) {
  const PLUGIN_ID = pluginId || 'com.panorama.wakatime'
  const [stats, setStats] = useState<any>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [heartbeatInput, setHeartbeatInput] = useState(
    '{"entity":"/src/main.rs","project":"panorama","language":"Rust"}',
  )

  const fetchStats = async () => {
    setLoading(true)
    try {
      const res = await callPluginEndpoint(
        'com.panorama.grafana',
        'query',
        'POST',
        { group_by: 'wakatime:project', aggregation: 'leaderboard' },
      )
      setStats(await res.json())
    } catch (_e: any) {
      setError('')
    }
    setLoading(false)
  }

  useEffect(() => {
    fetchStats()
  }, [])

  const sendHeartbeat = async () => {
    try {
      const body = JSON.parse(heartbeatInput)
      body.time = Math.floor(Date.now() / 1000)
      await callPluginEndpoint(PLUGIN_ID, 'heartbeat', 'POST', body)
      fetchStats()
    } catch (e: any) {
      setError(e.message)
    }
  }

  const barColors = [
    '#6c8cff', '#8c6cff', '#6cff8c', '#ff8c6c',
    '#ff6c8c', '#8cff6c', '#6cc8ff',
  ]

  return (
    <div style={{ maxWidth: 720, margin: '0 auto' }}>
      <h2 style={{ marginBottom: 16 }}>Coding Activity</h2>

      <div className="card" style={{ marginBottom: 20 }}>
        <h4>Send Test Heartbeat</h4>
        <textarea
          value={heartbeatInput}
          onChange={(e) => setHeartbeatInput(e.target.value)}
          rows={4}
          style={{
            width: '100%', marginBottom: 8,
            fontFamily: 'monospace', fontSize: 13,
          }}
        />
        <button className="primary" onClick={sendHeartbeat}>
          Send Heartbeat
        </button>
        {error && (
          <p style={{ color: 'var(--danger)', marginTop: 8 }}>{error}</p>
        )}
      </div>

      {loading ? (
        <p>Loading...</p>
      ) : stats && Array.isArray(stats) ? (
        <div className="card">
          <h4 style={{ marginBottom: 12 }}>Project Leaderboard (hours)</h4>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            {stats.map((item: any, i: number) => {
              const maxHours = stats[0]?.hours || 1
              const width = (item.hours / maxHours) * 100
              return (
                <div key={item.project}>
                  <div
                    className="flex-row"
                    style={{ justifyContent: 'space-between', marginBottom: 2 }}
                  >
                    <span>{item.project}</span>
                    <span className="text-muted">
                      {item.hours?.toFixed(1)}h
                    </span>
                  </div>
                  <div style={{
                    height: 8, background: 'var(--bg)',
                    borderRadius: 4, overflow: 'hidden',
                  }}>
                    <div style={{
                      height: '100%', width: `${width}%`,
                      background: barColors[i % barColors.length],
                      borderRadius: 4, transition: 'width 0.3s',
                    }} />
                  </div>
                </div>
              )
            })}
            {stats.length === 0 && (
              <p className="text-muted">
                No data yet. Send heartbeats to populate.
              </p>
            )}
          </div>
        </div>
      ) : (
        <p className="text-muted">
          No activity data yet. Send a test heartbeat above.
        </p>
      )}
    </div>
  )
}
