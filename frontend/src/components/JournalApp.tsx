/// <reference types="vite/client" />

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { callPluginEndpoint } from '../api/client'
import { useState, useMemo } from 'react'
import { marked } from 'marked'

// ── Types ────────────────────────────────────────────────────────────────────

interface JournalEntry {
  id: string
  fields: Record<string, { type: string; value: any }>
  created_at: string
  updated_at: string
}

interface Paragraph {
  id: string
  fields: Record<string, { type: string; value: any }>
}

const PLUGIN_ID = 'com.panorama.journal'

function fieldStr(entry: JournalEntry, key: string): string | undefined {
  const f = entry.fields[key]
  if (!f || f.value === null || f.value === undefined) return undefined
  return String(f.value)
}

function fieldBool(entry: JournalEntry, key: string): boolean {
  const f = entry.fields[key]
  if (!f) return false
  return f.value === true || f.value === 'true'
}

// ── API helpers ──────────────────────────────────────────────────────────────

async function listEntries(params: Record<string, string> = {}): Promise<JournalEntry[]> {
  const qs = new URLSearchParams(params).toString()
  const res = await callPluginEndpoint(PLUGIN_ID, `entries?${qs}`)
  if (!res.ok) throw new Error(await res.text())
  const data = await res.json()
  // Plugin endpoint returns array of JSON rows
  if (Array.isArray(data)) return data
  if (data && Array.isArray(data.rows)) return data.rows
  return []
}

async function createEntry(body: {
  title: string
  content: string
  mood?: string
  time?: string
}): Promise<JournalEntry> {
  const res = await callPluginEndpoint(PLUGIN_ID, 'entries', 'POST', body)
  if (!res.ok) {
    const err = await res.text()
    throw new Error(err)
  }
  return res.json()
}

async function updateEntry(
  id: string,
  body: { title?: string; content?: string; mood?: string },
): Promise<JournalEntry> {
  const res = await callPluginEndpoint(PLUGIN_ID, `entries/${id}`, 'PUT', body)
  if (!res.ok) {
    const err = await res.text()
    throw new Error(err)
  }
  return res.json()
}

async function deleteEntry(id: string): Promise<JournalEntry> {
  const res = await callPluginEndpoint(PLUGIN_ID, `entries/${id}`, 'DELETE')
  if (!res.ok) {
    const err = await res.text()
    throw new Error(err)
  }
  return res.json()
}

async function getParagraphs(entryId: string): Promise<Paragraph[]> {
  const res = await callPluginEndpoint(PLUGIN_ID, `entries/${entryId}/paragraphs`)
  if (!res.ok) return []
  const data = await res.json()
  return Array.isArray(data) ? data : []
}

// ── Mood colours ─────────────────────────────────────────────────────────────

const MOOD_COLORS: Record<string, string> = {
  happy: '#6cff8c',
  excited: '#ffcc6c',
  thoughtful: '#6c8cff',
  melancholic: '#aa88ff',
  anxious: '#ff8c6c',
  calm: '#6cffcc',
  grateful: '#ff6cff',
}

// ── Markdown renderer ────────────────────────────────────────────────────────

function renderMarkdown(md: string): string {
  return marked.parse(md, { async: false }) as string
}

// ── Main component ───────────────────────────────────────────────────────────

export function JournalApp() {
  const queryClient = useQueryClient()
  const [showCreate, setShowCreate] = useState(false)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [editingId, setEditingId] = useState<string | null>(null)

  // Filters
  const [moodFilter, setMoodFilter] = useState('')
  const [fromDate, setFromDate] = useState('')
  const [toDate, setToDate] = useState('')

  const queryParams = useMemo(() => {
    const p: Record<string, string> = {}
    if (moodFilter) p.mood = moodFilter
    if (fromDate) p.from = fromDate
    if (toDate) p.to = toDate
    return p
  }, [moodFilter, fromDate, toDate])

  const {
    data: entries = [],
    isLoading,
    error,
  } = useQuery({
    queryKey: ['journal-entries', queryParams],
    queryFn: () => listEntries(queryParams),
  })

  const {
    data: selectedEntry,
  } = useQuery({
    queryKey: ['journal-entry', selectedId],
    queryFn: async () => {
      if (!selectedId) return null
      const res = await callPluginEndpoint(PLUGIN_ID, `entries/${selectedId}`)
      if (!res.ok) throw new Error(await res.text())
      return res.json() as Promise<JournalEntry>
    },
    enabled: !!selectedId,
  })

  const {
    data: paragraphs = [],
  } = useQuery({
    queryKey: ['journal-paragraphs', selectedId],
    queryFn: () => getParagraphs(selectedId!),
    enabled: !!selectedId,
  })

  // Mutations
  const createMut = useMutation({
    mutationFn: createEntry,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['journal-entries'] })
      setShowCreate(false)
    },
  })

  const updateMut = useMutation({
    mutationFn: ({ id, body }: { id: string; body: any }) => updateEntry(id, body),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['journal-entries'] })
      queryClient.invalidateQueries({ queryKey: ['journal-entry', selectedId] })
      queryClient.invalidateQueries({ queryKey: ['journal-paragraphs', selectedId] })
      setEditingId(null)
    },
  })

  const deleteMut = useMutation({
    mutationFn: deleteEntry,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['journal-entries'] })
      setSelectedId(null)
    },
  })

  // ── Mood options ─────────────────────────────────────────────────────────

  const moodOptions = ['', 'happy', 'excited', 'thoughtful', 'melancholic', 'anxious', 'calm', 'grateful']

  return (
    <div>
      {/* Header */}
      <div className="flex-row mb-1" style={{ justifyContent: 'space-between', flexWrap: 'wrap' }}>
        <div>
          <h2 style={{ margin: 0 }}>📓 Journal</h2>
          <p className="text-muted">Daily entries with markdown, mood tagging &amp; paragraph references</p>
        </div>
        <button className="primary" onClick={() => setShowCreate(!showCreate)}>
          {showCreate ? 'Cancel' : '+ New Entry'}
        </button>
      </div>

      {/* Create form */}
      {showCreate && (
        <EntryForm
          onSubmit={(data) => createMut.mutate(data)}
          isPending={createMut.isPending}
        />
      )}

      {/* Filter bar */}
      <FilterBar
        moodFilter={moodFilter}
        onMoodChange={setMoodFilter}
        fromDate={fromDate}
        onFromChange={setFromDate}
        toDate={toDate}
        onToChange={setToDate}
        moodOptions={moodOptions}
      />

      {/* Entry list */}
      {isLoading && <p>Loading entries...</p>}
      {error && <p className="text-muted" style={{ color: 'var(--danger)' }}>Error: {String(error)}</p>}

      <div style={{ display: 'grid', gap: 12, marginTop: 16 }}>
        {entries.map((entry) => (
          <div key={entry.id}>
            <EntryCard
              entry={entry}
              isSelected={selectedId === entry.id}
              isEditing={editingId === entry.id}
              onClick={() => {
                setSelectedId(selectedId === entry.id ? null : entry.id)
                setEditingId(null)
              }}
              onEdit={() => setEditingId(entry.id)}
              onDelete={() => {
                if (confirm('Soft-delete this entry? It can be recovered.')) {
                  deleteMut.mutate(entry.id)
                }
              }}
            />

            {/* Inline edit form */}
            {editingId === entry.id && (
              <EntryForm
                initialTitle={fieldStr(entry, 'system:node_title') || ''}
                initialContent={fieldStr(entry, 'journal:content') || ''}
                initialMood={fieldStr(entry, 'journal:mood') || ''}
                isEdit
                onSubmit={(data) => updateMut.mutate({ id: entry.id, body: data })}
                onCancel={() => setEditingId(null)}
                isPending={updateMut.isPending}
              />
            )}

            {/* Expanded detail */}
            {selectedId === entry.id && selectedEntry && (
              <EntryDetail
                entry={selectedEntry}
                paragraphs={paragraphs}
                onClose={() => setSelectedId(null)}
              />
            )}
          </div>
        ))}

        {!isLoading && entries.length === 0 && (
          <div className="card" style={{ textAlign: 'center', padding: 40 }}>
            <p className="text-muted">No entries yet. Write your first journal entry!</p>
          </div>
        )}
      </div>
    </div>
  )
}

// ── Filter bar ───────────────────────────────────────────────────────────────

function FilterBar({
  moodFilter,
  onMoodChange,
  fromDate,
  onFromChange,
  toDate,
  onToChange,
  moodOptions,
}: {
  moodFilter: string
  onMoodChange: (v: string) => void
  fromDate: string
  onFromChange: (v: string) => void
  toDate: string
  onToChange: (v: string) => void
  moodOptions: string[]
}) {
  const hasFilters = moodFilter || fromDate || toDate

  return (
    <div
      className="card"
      style={{ marginTop: 16, display: 'flex', gap: 12, alignItems: 'center', flexWrap: 'wrap' }}
    >
      <span className="text-muted" style={{ fontWeight: 600, minWidth: 40 }}>
        Filters
      </span>

      <select
        value={moodFilter}
        onChange={(e) => onMoodChange(e.target.value)}
        style={{ minWidth: 140 }}
      >
        <option value="">All moods</option>
        {moodOptions.filter(Boolean).map((m) => (
          <option key={m} value={m}>
            {m.charAt(0).toUpperCase() + m.slice(1)}
          </option>
        ))}
      </select>

      <input
        type="date"
        value={fromDate}
        onChange={(e) => onFromChange(e.target.value)}
        placeholder="From date"
        style={{ minWidth: 150 }}
      />
      <input
        type="date"
        value={toDate}
        onChange={(e) => onToChange(e.target.value)}
        placeholder="To date"
        style={{ minWidth: 150 }}
      />

      {hasFilters && (
        <button
          onClick={() => {
            onMoodChange('')
            onFromChange('')
            onToChange('')
          }}
          style={{ fontSize: 12 }}
        >
          Clear filters
        </button>
      )}
    </div>
  )
}

// ── Entry card (list item) ──────────────────────────────────────────────────

function EntryCard({
  entry,
  isSelected,
  isEditing,
  onClick,
  onEdit,
  onDelete,
}: {
  entry: JournalEntry
  isSelected: boolean
  isEditing: boolean
  onClick: () => void
  onEdit: () => void
  onDelete: () => void
}) {
  const title = fieldStr(entry, 'system:node_title') || 'Untitled'
  const content = fieldStr(entry, 'journal:content') || ''
  const mood = fieldStr(entry, 'journal:mood')
  const isDeleted = fieldBool(entry, 'journal:deleted')
  const date = fieldStr(entry, 'system:node_time')

  // Truncate content for preview
  const preview =
    content.length > 120 ? content.slice(0, 120).replace(/\n/g, ' ') + '…' : content.replace(/\n/g, ' ')

  return (
    <div
      className="card"
      style={{
        cursor: 'pointer',
        borderColor: isSelected ? 'var(--accent)' : isDeleted ? 'var(--danger)' : undefined,
        opacity: isDeleted ? 0.6 : 1,
      }}
      onClick={onClick}
    >
      <div className="flex-row" style={{ justifyContent: 'space-between', flexWrap: 'wrap' }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <strong style={{ fontSize: 16 }}>
            {isDeleted && '🗑 '}
            {title}
          </strong>
          <span className="text-muted" style={{ marginLeft: 12 }}>
            {date ? new Date(date).toLocaleDateString('en-US', {
              weekday: 'short',
              year: 'numeric',
              month: 'short',
              day: 'numeric',
            }) : ''}
          </span>
          {mood && (
            <span
              className="mood-badge"
              style={{
                marginLeft: 8,
                padding: '2px 8px',
                borderRadius: 12,
                fontSize: 12,
                fontWeight: 600,
                background: MOOD_COLORS[mood] || 'var(--bg-hover)',
                color: '#0f0f14',
              }}
            >
              {mood}
            </span>
          )}
          {isDeleted && (
            <span
              style={{
                marginLeft: 8,
                padding: '2px 8px',
                borderRadius: 12,
                fontSize: 12,
                background: 'var(--danger)',
                color: 'white',
              }}
            >
              deleted
            </span>
          )}
        </div>

        <div className="flex-row" style={{ gap: 8 }}>
          {!isDeleted && (
            <>
              <button
                onClick={(e) => {
                  e.stopPropagation()
                  onEdit()
                }}
                style={{ fontSize: 12, padding: '4px 10px', minHeight: 32 }}
              >
                Edit
              </button>
              <button
                onClick={(e) => {
                  e.stopPropagation()
                  onDelete()
                }}
                style={{
                  fontSize: 12,
                  padding: '4px 10px',
                  minHeight: 32,
                  color: 'var(--danger)',
                  borderColor: 'var(--danger)',
                }}
              >
                Delete
              </button>
            </>
          )}
        </div>
      </div>

      <p className="text-muted" style={{ marginTop: 8, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
        {preview || '(empty entry)'}
      </p>

      {/* Paragraph count */}
      {(() => {
        const refs = entry.fields['journal:paragraph_refs']
        const count =
          refs && refs.type === 'Array' && Array.isArray(refs.value) ? refs.value.length : 0
        if (count > 0) {
          return (
            <span className="text-muted" style={{ fontSize: 11 }}>
              {count} paragraph{count !== 1 ? 's' : ''}
            </span>
          )
        }
        return null
      })()}
    </div>
  )
}

// ── Entry form (create / edit) ──────────────────────────────────────────────

function EntryForm({
  initialTitle = '',
  initialContent = '',
  initialMood = '',
  isEdit = false,
  onSubmit,
  onCancel,
  isPending,
}: {
  initialTitle?: string
  initialContent?: string
  initialMood?: string
  isEdit?: boolean
  onSubmit: (data: { title: string; content: string; mood?: string }) => void
  onCancel?: () => void
  isPending: boolean
}) {
  const [title, setTitle] = useState(initialTitle)
  const [content, setContent] = useState(initialContent)
  const [mood, setMood] = useState(initialMood)
  const [preview, setPreview] = useState(false)

  const handleSubmit = () => {
    if (!title.trim() && !content.trim()) return
    const data: { title: string; content: string; mood?: string } = {
      title: title.trim() || 'Untitled Entry',
      content: content.trim(),
    }
    if (mood) data.mood = mood
    onSubmit(data)
  }

  return (
    <div className="card" style={{ marginTop: 8, marginBottom: 8 }}>
      <div className="flex-row mb-1" style={{ justifyContent: 'space-between' }}>
        <strong>{isEdit ? 'Edit Entry' : 'New Entry'}</strong>
        <div className="flex-row" style={{ gap: 8 }}>
          {!isEdit && (
            <button
              onClick={() => setPreview(!preview)}
              style={{ fontSize: 12, padding: '4px 10px', minHeight: 32 }}
            >
              {preview ? 'Edit' : 'Preview'}
            </button>
          )}
          {onCancel && (
            <button
              onClick={onCancel}
              style={{ fontSize: 12, padding: '4px 10px', minHeight: 32 }}
            >
              Cancel
            </button>
          )}
        </div>
      </div>

      <div className="flex-col">
        <input
          placeholder="Entry title"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          style={{ fontWeight: 600 }}
        />

        {preview ? (
          <div
            className="journal-entry-body"
            style={{
              minHeight: 120,
              padding: '12px',
              background: 'var(--bg)',
              borderRadius: 6,
              border: '1px solid var(--border)',
              overflow: 'auto',
            }}
            dangerouslySetInnerHTML={{ __html: renderMarkdown(content || '*Nothing written yet*') }}
          />
        ) : (
          <textarea
            placeholder="Write your entry in markdown…&#10;&#10;# Heading&#10;- bullet&#10;- list&#10;&#10;**bold** *italic* `code`"
            value={content}
            onChange={(e) => setContent(e.target.value)}
            rows={8}
            style={{ fontFamily: 'monospace', fontSize: 14, resize: 'vertical' }}
          />
        )}

        <div className="flex-row" style={{ justifyContent: 'space-between', flexWrap: 'wrap' }}>
          <select
            value={mood}
            onChange={(e) => setMood(e.target.value)}
            style={{ minWidth: 140 }}
          >
            <option value="">Mood (optional)</option>
            <option value="happy">😊 Happy</option>
            <option value="excited">🎉 Excited</option>
            <option value="thoughtful">🤔 Thoughtful</option>
            <option value="melancholic">🌧 Melancholic</option>
            <option value="anxious">😰 Anxious</option>
            <option value="calm">🧘 Calm</option>
            <option value="grateful">🙏 Grateful</option>
          </select>

          <button className="primary" onClick={handleSubmit} disabled={isPending}>
            {isPending ? 'Saving…' : isEdit ? 'Save Changes' : 'Save Entry'}
          </button>
        </div>
      </div>
    </div>
  )
}

// ── Entry detail (expanded view) ────────────────────────────────────────────

function EntryDetail({
  entry,
  paragraphs,
  onClose,
}: {
  entry: JournalEntry
  paragraphs: Paragraph[]
  onClose: () => void
}) {
  const title = fieldStr(entry, 'system:node_title') || 'Untitled'
  const content = fieldStr(entry, 'journal:content') || ''
  const mood = fieldStr(entry, 'journal:mood')
  const date = fieldStr(entry, 'system:node_time')
  const isDeleted = fieldBool(entry, 'journal:deleted')

  return (
    <div className="card" style={{ marginTop: 12, position: 'relative' }}>
      <button
        onClick={onClose}
        style={{ position: 'absolute', top: 8, right: 8, fontSize: 12, padding: '4px 10px', minHeight: 32 }}
      >
        ✕ Close
      </button>

      <h3 style={{ paddingRight: 60 }}>{title}</h3>
      <div className="flex-row text-muted" style={{ gap: 12, flexWrap: 'wrap', marginTop: 4 }}>
        <span>
          {date
            ? new Date(date).toLocaleString('en-US', {
                weekday: 'long',
                year: 'numeric',
                month: 'long',
                day: 'numeric',
                hour: '2-digit',
                minute: '2-digit',
              })
            : ''}
        </span>
        {mood && (
          <span
            style={{
              padding: '2px 10px',
              borderRadius: 12,
              fontSize: 12,
              fontWeight: 600,
              background: MOOD_COLORS[mood] || 'var(--bg-hover)',
              color: '#0f0f14',
            }}
          >
            {mood}
          </span>
        )}
        {isDeleted && (
          <span
            style={{
              padding: '2px 10px',
              borderRadius: 12,
              fontSize: 12,
              background: 'var(--danger)',
              color: 'white',
            }}
          >
            Deleted
          </span>
        )}
        <span>ID: {entry.id.slice(0, 8)}…</span>
      </div>

      {/* Rendered markdown */}
      <div
        className="journal-entry-body"
        style={{
          marginTop: 16,
          padding: '16px 20px',
          background: 'var(--bg)',
          borderRadius: 8,
          border: '1px solid var(--border)',
          lineHeight: 1.8,
          fontSize: 15,
          overflow: 'auto',
        }}
        dangerouslySetInnerHTML={{ __html: renderMarkdown(content) }}
      />

      {/* Paragraph child nodes */}
      {paragraphs.length > 0 && (
        <div style={{ marginTop: 20 }}>
          <h4>📄 Paragraph Nodes ({paragraphs.length})</h4>
          <p className="text-muted">
            Each paragraph is a separate node that can be referenced from other entries.
          </p>
          <div style={{ display: 'grid', gap: 8, marginTop: 8 }}>
            {paragraphs.map((p, i) => (
              <div
                key={p.id}
                className="card"
                style={{ padding: 12, borderLeft: '3px solid var(--accent)' }}
              >
                <div className="flex-row text-muted" style={{ justifyContent: 'space-between', fontSize: 12 }}>
                  <span>Paragraph {i + 1}</span>
                  <span style={{ fontFamily: 'monospace' }}>{p.id.slice(0, 8)}…</span>
                </div>
                <p style={{ marginTop: 6, fontSize: 14 }}>
                  {String(p.fields?.['journal:content']?.value || '')}
                </p>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}
