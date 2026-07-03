// Dashboard Plugin — React UI component
// Grafana-like dashboard for time-series data visualization

import { useState, useEffect } from 'react'
import { callPluginEndpoint } from '../api/client'

export default function DashboardUI() {
  const [dashboards, setDashboards] = useState<any[]>([])
  const [leaderboard, setLeaderboard] = useState<any[]>([])
  const [groupBy, setGroupBy] = useState('wakatime:project')
  const [aggregation, setAggregation] = useState('leaderboard')
  const [loading, setLoading] = useState(true)

  const fetchData = async () => {
    setLoading(true)
    try {
      const res = await callPluginEndpoint('com.panorama.grafana', 'query', 'POST', {
        group_by: groupBy,
        aggregation,
        filter: '',
      })
      const data = await res.json()
      if (Array.isArray(data)) setLeaderboard(data)

      const dashRes = await callPluginEndpoint('com.panorama.grafana', 'dashboards')
      setDashboards(await dashRes.json())
    } catch (e) {}
    setLoading(false)
  }

  useEffect(() => { fetchData() }, [groupBy, aggregation])

  const queryOptions = [
    { groupBy: 'wakatime:project', label: 'Project' },
    { groupBy: 'wakatime:language', label: 'Language' },
    { groupBy: 'wakatime:entity', label: 'File' },
  ]

  const aggOptions = [
    { value: 'leaderboard', label: 'Leaderboard (hours)' },
    { value: 'count', label: 'Count' },
    { value: 'sum_duration', label: 'Total Duration' },
  ]

  return (
    <div style={{ maxWidth: 800, margin: '0 auto' }}>
      <h2 style={{ marginBottom: 16 }}>Dashboards</h2>

      {/* Query controls */}
      <div className="card" style={{ marginBottom: 20 }}>
        <div className="flex-row" style={{ gap: 12, flexWrap: 'wrap' }}>
          <div>
            <label className="text-muted" style={{ display: 'block', marginBottom: 4 }}>Group By</label>
            <select value={groupBy} onChange={e => setGroupBy(e.target.value)}>
              {queryOptions.map(o => <option key={o.groupBy} value={o.groupBy}>{o.label}</option>)}
            </select>
          </div>
          <div>
            <label className="text-muted" style={{ display: 'block', marginBottom: 4 }}>Aggregation</label>
            <select value={aggregation} onChange={e => setAggregation(e.target.value)}>
              {aggOptions.map(o => <option key={o.value} value={o.value}>{o.label}</option>)}
            </select>
          </div>
        </div>
      </div>

      {/* Results table */}
      {loading ? <p>Loading...</p> : (
        <div className="card">
          <h4 style={{ marginBottom: 12 }}>
            {aggregation === 'leaderboard' ? 'Leaderboard' : aggregation === 'count' ? 'Count by ' + groupBy : 'Total Duration'}
          </h4>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead>
              <tr>
                <th style={{ textAlign: 'left', padding: '8px 4px', borderBottom: '1px solid var(--border)' }}>#</th>
                <th style={{ textAlign: 'left', padding: '8px 4px', borderBottom: '1px solid var(--border)' }}>{groupBy.split(':')[1]}</th>
                <th style={{ textAlign: 'right', padding: '8px 4px', borderBottom: '1px solid var(--border)' }}>
                  {aggregation === 'leaderboard' ? 'Hours' : aggregation === 'count' ? 'Count' : 'Seconds'}
                </th>
              </tr>
            </thead>
            <tbody>
              {leaderboard.map((item: any, i: number) => (
                <tr key={item.key || item.project || i} style={{ borderBottom: '1px solid var(--border)' }}>
                  <td style={{ padding: '8px 4px' }}>
                    <span style={{
                      display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
                      width: 24, height: 24, borderRadius: 12,
                      background: i < 3 ? 'var(--accent)' : 'var(--bg-hover)',
                      fontSize: 12, fontWeight: 700,
                    }}>{i + 1}</span>
                  </td>
                  <td style={{ padding: '8px 4px' }}>{item.key || item.project}</td>
                  <td style={{ padding: '8px 4px', textAlign: 'right', fontFamily: 'monospace' }}>
                    {aggregation === 'leaderboard' ? (item.hours?.toFixed(1) ?? '0.0') : item.count ?? item.total_duration_seconds ?? '-'}
                  </td>
                </tr>
              ))}
              {leaderboard.length === 0 && (
                <tr><td colSpan={3} style={{ padding: 16, textAlign: 'center' }} className="text-muted">No data</td></tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}
