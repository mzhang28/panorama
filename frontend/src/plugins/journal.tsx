// Journal Plugin — React UI component
// Displays journal entries stacked newest-first with a create form

import { useState, useEffect } from 'react'
import { callPluginEndpoint, type Node } from '../api/client'

export default function JournalUI() {
  const [entries, setEntries] = useState<Node[]>([])
  const [loading, setLoading] = useState(true)
  const [title, setTitle] = useState('')
  const [content, setContent] = useState('')
  const [mood, setMood] = useState('')
  const [error, setError] = useState('')
  const [selectedEntry, setSelectedEntry] = useState<Node | null>(null)

  const PLUGIN_ID = 'com.panorama.journal'

  const fetchEntries = async () => {
    setLoading(true)
    try {
      const res = await callPluginEndpoint(PLUGIN_ID, 'entries')
      const data = await res.json()
      setEntries(data)
    } catch (e: any) {
      setError(e.message)
    }
    setLoading(false)
  }

  useEffect(() => { fetchEntries() }, [])

  const handleCreate = async () => {
    if (!title || !content) return
    setError('')
    try {
      await callPluginEndpoint(PLUGIN_ID, 'entries', 'POST', { title, content, mood: mood || undefined })
      setTitle(''); setContent(''); setMood('')
      fetchEntries()
    } catch (e: any) {
      setError(e.message)
    }
  }

  const moods = ['happy', 'thoughtful', 'tired', 'excited', 'anxious', 'grateful', 'neutral']

  return (
    <div style={{ maxWidth: 720, margin: '0 auto' }}>
      <h2 style={{ marginBottom: 16 }}>Journal</h2>

      {/* Create form */}
      <div className="card" style={{ marginBottom: 20 }}>
        <input
          placeholder="Entry title"
          value={title}
          onChange={e => setTitle(e.target.value)}
          style={{ width: '100%', marginBottom: 8 }}
        />
        <textarea
          placeholder="Write your entry (markdown supported)..."
          value={content}
          onChange={e => setContent(e.target.value)}
          rows={5}
          style={{ width: '100%', marginBottom: 8 }}
        />
        <div className="flex-row" style={{ marginBottom: 8 }}>
          <select value={mood} onChange={e => setMood(e.target.value)}>
            <option value="">No mood</option>
            {moods.map(m => <option key={m} value={m}>{m}</option>)}
          </select>
          <button className="primary" onClick={handleCreate}>Save Entry</button>
        </div>
        {error && <p style={{ color: 'var(--danger)' }}>{error}</p>}
      </div>

      {/* Entry list */}
      {loading ? <p>Loading...</p> : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          {entries.map(entry => (
            <div key={entry.id} className="card" style={{ cursor: 'pointer' }}
                 onClick={() => setSelectedEntry(selectedEntry?.id === entry.id ? null : entry)}>
              <div className="flex-row" style={{ justifyContent: 'space-between' }}>
                <strong>{entry.fields['system:node_title']?.value || 'Untitled'}</strong>
                <span className="text-muted">{new Date(entry.updated_at).toLocaleDateString()}</span>
              </div>
              {entry.fields['journal:mood'] && (
                <span className="text-muted" style={{ fontSize: 12 }}>
                  Mood: {entry.fields['journal:mood'].value}
                </span>
              )}
              {selectedEntry?.id === entry.id && (
                <div style={{ marginTop: 12, padding: 12, background: 'var(--bg)', borderRadius: 6 }}>
                  <pre style={{ whiteSpace: 'pre-wrap', fontFamily: 'inherit' }}>
                    {entry.fields['journal:content']?.value || '(no content)'}
                  </pre>
                </div>
              )}
            </div>
          ))}
          {entries.length === 0 && <p className="text-muted">No entries yet. Write your first journal entry!</p>}
        </div>
      )}
    </div>
  )
}
