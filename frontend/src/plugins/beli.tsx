// Beli Plugin — React UI component with partial-order restaurant rankings

import { useState, useEffect } from 'react'
import { callPluginEndpoint, type Node } from '../api/client'

export default function BeliUI() {
  const [restaurants, setRestaurants] = useState<Node[]>([])
  const [rankings, setRankings] = useState<any>(null)
  const [loading, setLoading] = useState(true)
  const [name, setName] = useState('')
  const [cuisine, setCuisine] = useState('')
  const [location, setLocation] = useState('')
  const [betterId, setBetterId] = useState('')
  const [worseId, setWorseId] = useState('')
  const [context, setContext] = useState('')
  const [error, setError] = useState('')

  const PLUGIN_ID = 'com.panorama.beli'

  const fetchData = async () => {
    setLoading(true)
    try {
      const rRes = await callPluginEndpoint(PLUGIN_ID, 'restaurants')
      setRestaurants(await rRes.json())
      const rankRes = await callPluginEndpoint(PLUGIN_ID, 'rankings')
      setRankings(await rankRes.json())
    } catch (e: any) { setError(e.message) }
    setLoading(false)
  }

  useEffect(() => { fetchData() }, [])

  const addRestaurant = async () => {
    if (!name) return
    await callPluginEndpoint(PLUGIN_ID, 'restaurants', 'POST', { name, cuisine, location })
    setName(''); setCuisine(''); setLocation('')
    fetchData()
  }

  const addComparison = async () => {
    if (!betterId || !worseId) return
    setError('')
    await callPluginEndpoint(PLUGIN_ID, 'compare', 'POST', { better_id: betterId, worse_id: worseId, context: context || undefined })
    setContext('')
    fetchData()
  }

  const tierColors = ['#6cff8c', '#8cc8ff', '#ffcc6c', '#ff8c8c', '#cc8cff', '#8cffcc', '#ffcc8c']

  return (
    <div style={{ maxWidth: 800, margin: '0 auto' }}>
      <h2 style={{ marginBottom: 16 }}>Restaurant Rankings</h2>
      <p className="text-muted" style={{ marginBottom: 16 }}>
        Partial order ranking via pairwise comparisons — not star ratings!
        Higher tiers are better. Within a tier, items are incomparable.
      </p>

      {/* Add restaurant */}
      <div className="card flex-row" style={{ marginBottom: 12, gap: 8 }}>
        <input placeholder="Restaurant name" value={name} onChange={e => setName(e.target.value)} style={{ flex: 1 }} />
        <input placeholder="Cuisine" value={cuisine} onChange={e => setCuisine(e.target.value)} style={{ width: 120 }} />
        <input placeholder="Location" value={location} onChange={e => setLocation(e.target.value)} style={{ width: 120 }} />
        <button className="primary" onClick={addRestaurant}>+ Add</button>
      </div>

      {/* Comparison form */}
      <div className="card flex-row" style={{ marginBottom: 20, gap: 8, flexWrap: 'wrap' }}>
        <select value={betterId} onChange={e => setBetterId(e.target.value)}>
          <option value="">Better (A)</option>
          {restaurants.map(r => (
            <option key={r.id} value={r.id}>{r.fields['system:node_title']?.value}</option>
          ))}
        </select>
        <span className="text-muted">&gt;</span>
        <select value={worseId} onChange={e => setWorseId(e.target.value)}>
          <option value="">Worse (B)</option>
          {restaurants.map(r => (
            <option key={r.id} value={r.id}>{r.fields['system:node_title']?.value}</option>
          ))}
        </select>
        <input placeholder="Context (optional)" value={context} onChange={e => setContext(e.target.value)} style={{ width: 150 }} />
        <button className="primary" onClick={addComparison}>Compare</button>
      </div>

      {error && <p style={{ color: 'var(--danger)', marginBottom: 12 }}>{error}</p>}

      {/* Rankings tiers */}
      {loading ? <p>Loading...</p> : rankings?.tiers ? (
        <div>
          <p className="text-muted" style={{ marginBottom: 12 }}>
            {rankings.total_restaurants} restaurants · {rankings.total_comparisons} comparisons · {rankings.tiers.length} tiers
          </p>
          {rankings.tiers.map((tier: any[], idx: number) => (
            <div key={idx} className="card" style={{
              marginBottom: 8,
              borderLeft: `4px solid ${tierColors[idx % tierColors.length]}`,
            }}>
              <div className="flex-row" style={{ justifyContent: 'space-between', marginBottom: 4 }}>
                <strong>Tier {idx + 1}</strong>
                <span className="text-muted">{tier.length} restaurant{tier.length !== 1 ? 's' : ''}</span>
              </div>
              <div className="flex-row" style={{ gap: 8, flexWrap: 'wrap' }}>
                {tier.map((r: any) => (
                  <span key={r.id} style={{
                    padding: '4px 12px', borderRadius: 16,
                    background: 'var(--bg-hover)', fontSize: 14,
                  }}>{r.name}</span>
                ))}
              </div>
            </div>
          ))}
          {rankings.tiers.length === 0 && <p className="text-muted">Add restaurants and comparisons to see rankings.</p>}
        </div>
      ) : <p className="text-muted">No data yet.</p>}
    </div>
  )
}
