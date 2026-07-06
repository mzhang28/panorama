/// <reference types="vite/client" />

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { callPluginEndpoint } from '../api/client'
import { useState, useMemo } from 'react'
import { marked } from 'marked'

// ── Types ────────────────────────────────────────────────────────────────────

interface Block {
  id: string
  fields: Record<string, { type: string; value: any }>
  created_at: string
  updated_at: string
}

interface BlockNode {
  n?: Block
  id?: string
  fields?: Record<string, { type: string; value: any }>
  created_at?: string
  updated_at?: string
}

const PLUGIN_ID = 'io.mzhang.panorama.journal'

// ── Field helpers ─────────────────────────────────────────────────────────────

function f(node: BlockNode, key: string): any {
  const n = node?.n || node
  const fields = n?.fields ?? {}
  return fields[key]?.value
}

function fStr(node: BlockNode, key: string): string {
  const v = f(node, key)
  return v != null ? String(v) : ''
}

function fBool(node: BlockNode, key: string): boolean {
  const v = f(node, key)
  return v === true || v === 'true'
}

function fJson(node: BlockNode, key: string): any {
  const v = f(node, key)
  if (typeof v === 'string') {
    try { return JSON.parse(v) } catch { return v }
  }
  return v
}

function normalizeBlock(raw: any): Block {
  return raw?.n || raw
}

// ── API helpers ───────────────────────────────────────────────────────────────

async function api(path: string, method = 'GET', body?: any): Promise<any> {
  const res = await callPluginEndpoint(PLUGIN_ID, path, method, body)
  if (!res.ok) {
    const err = await res.text()
    throw new Error(err)
  }
  return res.json()
}

async function listBlocks(params: Record<string, string> = {}): Promise<Block[]> {
  const qs = new URLSearchParams(params).toString()
  const data = await api(`blocks?${qs}`)
  return (Array.isArray(data) ? data : data?.rows ?? []).map(normalizeBlock)
}

async function listPages(): Promise<Block[]> {
  const data = await api('pages')
  return (Array.isArray(data) ? data : data?.rows ?? []).map(normalizeBlock)
}

async function getBlock(id: string): Promise<Block> {
  return normalizeBlock(await api(`blocks/${id}`))
}

async function getChildren(id: string): Promise<Block[]> {
  const data = await api(`blocks/${id}/children`)
  return (Array.isArray(data) ? data : data?.rows ?? []).map(normalizeBlock)
}

async function getTodayPage(): Promise<Block> {
  return normalizeBlock(await api('pages/today'))
}

async function getBacklinks(pageId: string): Promise<Block[]> {
  const data = await api(`pages/${pageId}/backlinks`)
  return (Array.isArray(data) ? data : data?.rows ?? []).map(normalizeBlock)
}

async function renderMarkdown(content: string): Promise<string> {
  const data = await api('blocks/render', 'POST', { content })
  return data.html ?? marked.parse(content) ?? ''
}

// ── Journal App ───────────────────────────────────────────────────────────────

const TODAY = new Date().toISOString().slice(0, 10)

export function JournalApp() {
  const queryClient = useQueryClient()
  const [selectedPageId, setSelectedPageId] = useState<string | null>(null)
  const [showNewBlock, setShowNewBlock] = useState(false)
  const [editingBlockId, setEditingBlockId] = useState<string | null>(null)
  const [showBacklinks, setShowBacklinks] = useState(false)

  // ── Queries ────────────────────────────────────────────────────────────

  const { data: pages = [], isLoading: pagesLoading } = useQuery({
    queryKey: ['journal-pages'],
    queryFn: listPages,
    refetchInterval: 15_000,
  })

  const { data: todayPage } = useQuery({
    queryKey: ['journal-today'],
    queryFn: getTodayPage,
  })

  const { data: selectedPage } = useQuery({
    queryKey: ['journal-page', selectedPageId],
    queryFn: () => selectedPageId ? getBlock(selectedPageId) : null,
    enabled: !!selectedPageId,
  })

  const { data: pageChildren = [] } = useQuery({
    queryKey: ['journal-children', selectedPageId],
    queryFn: () => selectedPageId ? getChildren(selectedPageId) : [],
    enabled: !!selectedPageId,
  })

  const { data: backlinks = [] } = useQuery({
    queryKey: ['journal-backlinks', selectedPageId],
    queryFn: () => selectedPageId ? getBacklinks(selectedPageId) : [],
    enabled: !!selectedPageId && showBacklinks,
  })

  // ── Mutations ──────────────────────────────────────────────────────────

  const createBlock = useMutation({
    mutationFn: (body: any) => api('blocks', 'POST', body),
    onSuccess: (data: any, vars: any) => {
      const block = normalizeBlock(data)
      // Immediately populate caches so the UI is instant
      if (block.id) {
        queryClient.setQueryData(['journal-page', block.id], block)
      }
      queryClient.invalidateQueries({ queryKey: ['journal-pages'] })
      queryClient.invalidateQueries({ queryKey: ['journal-children'] })
      setShowNewBlock(false)
      // Manually add the new page to the cached pages list so the sidebar updates
      queryClient.setQueryData(['journal-pages'], (old: any) => {
        const oldPages = (Array.isArray(old) ? old : old?.rows ?? []);
        return [block, ...oldPages];
      });
      // Navigate to newly created root page (not child blocks)
      if (!vars.parent_id && block.id) {
        setSelectedPageId(block.id)
      }
    },
  })

  const updateBlock = useMutation({
    mutationFn: ({ id, ...body }: any) => api(`blocks/${id}`, 'PUT', body),
    onSuccess: (data: any) => {
      const block = normalizeBlock(data)
      if (block.id) {
        queryClient.setQueryData(['journal-page', block.id], block)
      }
      queryClient.invalidateQueries({ queryKey: ['journal-pages'] })
      queryClient.invalidateQueries({ queryKey: ['journal-children'] })
      setEditingBlockId(null)
    },
  })

  const deleteBlock = useMutation({
    mutationFn: (id: string) => api(`blocks/${id}`, 'DELETE'),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['journal-pages'] })
      queryClient.invalidateQueries({ queryKey: ['journal-page'] })
    },
  })

  // ── Determine active page ──────────────────────────────────────────────

  const activePageId = selectedPageId || (todayPage as any)?.id
  const activePage = selectedPageId ? selectedPage : todayPage as Block | null

  // ── Render ─────────────────────────────────────────────────────────────

  return (
    <div className="journal-layout">
      {/* Left sidebar: page list */}
      <aside className="journal-sidebar">
        <div className="journal-sidebar-section">
          <button
            className="journal-today-btn"
            onClick={() => {
              setSelectedPageId(null)
              queryClient.invalidateQueries({ queryKey: ['journal-today'] })
            }}
          >
            📅 Today
          </button>
          <button
            className="journal-new-page-btn"
            onClick={() => {
              setSelectedPageId(null)
              setShowNewBlock(true)
            }}
          >
            + New Page
          </button>
        </div>

        <h3 className="journal-sidebar-heading">All Pages</h3>
        {pagesLoading ? (
          <p className="text-muted" style={{ fontSize: 13 }}>Loading...</p>
        ) : pages.length === 0 ? (
          <p className="text-muted" style={{ fontSize: 13 }}>No pages yet</p>
        ) : (
          <div className="journal-page-list">
            {pages.map((p) => {
              const title = fStr(p, 'system:node_title') || 'Untitled'
              const day = fStr(p, 'journal:journal_day')
              const isDeleted = fBool(p, 'journal:deleted')
              return (
                <button
                  key={normalizeBlock(p).id || title}
                  className={`journal-page-link${activePageId === normalizeBlock(p).id ? ' active' : ''}${isDeleted ? ' deleted' : ''}`}
                  onClick={() => {
                    setSelectedPageId(normalizeBlock(p).id)
                    setShowBacklinks(false)
                  }}
                  title={day || undefined}
                >
                  <span className="journal-page-link-icon">{day ? '📅' : '📄'}</span>
                  <span className="journal-page-link-title">{title || 'Untitled'}</span>
                  {isDeleted && <span className="journal-deleted-badge">deleted</span>}
                </button>
              )
            })}
          </div>
        )}
      </aside>

      {/* Main content */}
      <main className="journal-main">
        {showNewBlock ? (
          <NewBlockForm
            parentId={null}
            pageId={null}
            createBlock={(body) => createBlock.mutateAsync(body)}
            isPending={createBlock.isPending}
            onCreated={(data) => {
              queryClient.invalidateQueries({ queryKey: ['journal-pages'] })
              setShowNewBlock(false)
              const newId = data?.n?.id || data?.id
              if (newId) setSelectedPageId(newId)
            }}
          />
        ) : activePage ? (
          <>
            {/* Page header */}
            <div className="journal-page-header">
              <h2 className="journal-page-title">
                {fStr(activePage, 'system:node_title') || 'Untitled'}
              </h2>
              <div className="journal-page-meta">
                {fStr(activePage, 'journal:journal_day') && (
                  <span className="journal-date-badge">
                    📅 {fStr(activePage, 'journal:journal_day')}
                  </span>
                )}
                <button
                  className="journal-icon-btn"
                  onClick={() => setShowBacklinks(!showBacklinks)}
                  title="Toggle backlinks"
                >
                  🔗 Backlinks
                </button>
                <button
                  className="journal-icon-btn"
                  onClick={() => {
                    if (activePageId) {
                      if (confirm('Delete this page?')) {
                        deleteBlock.mutate(activePageId)
                      }
                    }
                  }}
                  title="Delete page"
                  data-active-page-id={activePageId || ''}
                >
                  🗑️
                </button>
              </div>
              {/* Tags */}
              {fJson(activePage, 'journal:tags') && (
                <div className="journal-tags">
                  {(Array.isArray(fJson(activePage, 'journal:tags'))
                    ? fJson(activePage, 'journal:tags')
                    : []
                  ).map((tag: string) => (
                    <span key={tag} className="journal-tag">#{tag}</span>
                  ))}
                </div>
              )}
            </div>

            {/* Properties */}
            {fJson(activePage, 'journal:properties') && (
              <PropertiesBlock
                properties={fJson(activePage, 'journal:properties')}
              />
            )}

            {/* Page content block */}
            <BlockView
              block={activePage as BlockNode}
              depth={0}
              onEdit={(id) => setEditingBlockId(id)}
              editingBlockId={editingBlockId}
              onSave={(id, content) => {
                updateBlock.mutate({ id, content })
              }}
              onCancel={() => setEditingBlockId(null)}
              onDelete={(id) => {
                if (confirm('Delete this block?')) deleteBlock.mutate(id)
              }}
            />

            {/* Child blocks (tree) */}
            {pageChildren.map((child: BlockNode) => (
              <BlockView
                key={normalizeBlock(child as any).id || fStr(child, 'journal:order')}
                block={child}
                depth={1}
                onEdit={(id) => setEditingBlockId(id)}
                editingBlockId={editingBlockId}
                onSave={(id, content) => {
                  updateBlock.mutate({ id, content })
                }}
                onCancel={() => setEditingBlockId(null)}
                onDelete={(id) => {
                  if (confirm('Delete this block?')) deleteBlock.mutate(id)
                }}
              />
            ))}

            {/* New child block */}
            <NewBlockForm
              parentId={activePageId}
              pageId={activePageId}
              createBlock={(body) => createBlock.mutateAsync(body)}
              isPending={createBlock.isPending}
              onCreated={() => {
                queryClient.invalidateQueries({ queryKey: ['journal-children'] })
                queryClient.invalidateQueries({ queryKey: ['journal-page'] })
              }}
            />

            {/* Backlinks panel */}
            {showBacklinks && (
              <div className="journal-backlinks">
                <h3>🔗 Backlinks</h3>
                {backlinks.length === 0 ? (
                  <p className="text-muted">No backlinks yet. Link to this page with [[page name]].</p>
                ) : (
                  backlinks.map((bl: BlockNode) => (
                    <div key={normalizeBlock(bl as any).id} className="journal-backlink-item">
                      <div className="journal-backlink-title">
                        {fStr(bl, 'system:node_title') || 'Untitled'}
                      </div>
                      <div className="journal-backlink-preview">
                        {(fStr(bl, 'journal:content') || '').slice(0, 200)}
                      </div>
                    </div>
                  ))
                )}
              </div>
            )}
          </>
        ) : (
          <div className="journal-empty">
            <p className="text-muted">Select a page or create a new one.</p>
          </div>
        )}
      </main>
    </div>
  )
}

// ── Block View (Logseq-style bullet + indentation) ───────────────────────────

function BlockView({
  block,
  depth,
  onEdit,
  editingBlockId,
  onSave,
  onCancel,
  onDelete,
}: {
  block: BlockNode
  depth: number
  onEdit: (id: string) => void
  editingBlockId: string | null
  onSave: (id: string, content: string) => void
  onCancel: () => void
  onDelete: (id: string) => void
}) {
  const id = normalizeBlock(block as any).id || ''
  const content = fStr(block, 'journal:content') || ''
  const title = fStr(block, 'system:node_title') || ''
  const isEditing = editingBlockId === id
  const isDeleted = fBool(block, 'journal:deleted')

  const [editContent, setEditContent] = useState(content)
  const [previewHtml, setPreviewHtml] = useState('')
  const [showPreview, setShowPreview] = useState(false)

  const indent = depth * 28

  // Render [[links]] as styled spans
  const renderContent = (text: string) => {
    return text.replace(
      /\[\[([^\]]+)\]\]/g,
      '<span class="journal-ref">$1</span>'
    )
  }

  if (isDeleted) {
    return (
      <div className="journal-block deleted" style={{ marginLeft: indent }}>
        <span className="journal-bullet">·</span>
        <span className="text-muted" style={{ textDecoration: 'line-through' }}>
          {title || content.slice(0, 80)}
        </span>
      </div>
    )
  }

  return (
    <div className="journal-block" style={{ marginLeft: indent }}>
      {/* Bullet + content */}
      <div className="journal-block-row">
        <span className="journal-bullet">{depth === 0 ? '◆' : '•'}</span>

        {isEditing ? (
          <div className="journal-block-editor">
            <textarea
              value={editContent}
              onChange={(e) => setEditContent(e.target.value)}
              rows={Math.max(3, editContent.split('\n').length)}
              className="journal-block-textarea"
              autoFocus
            />
            <div className="journal-block-editor-actions">
              <button onClick={() => onSave(id, editContent)}>Save</button>
              <button onClick={onCancel}>Cancel</button>
              <button
                onClick={async () => {
                  const html = await renderMarkdown(editContent)
                  setPreviewHtml(html)
                  setShowPreview(!showPreview)
                }}
              >
                {showPreview ? 'Edit' : 'Preview'}
              </button>
            </div>
            {showPreview && (
              <div
                className="journal-markdown-preview"
                dangerouslySetInnerHTML={{ __html: previewHtml }}
              />
            )}
          </div>
        ) : (
          <div className="journal-block-content" onClick={() => onEdit(id)}>
            {title && depth === 0 ? (
              <div
                className="journal-markdown-body"
                dangerouslySetInnerHTML={{
                  __html: renderContent(content) || '<span class="text-muted">Empty page — click to edit</span>',
                }}
              />
            ) : (
              <div
                className="journal-block-text"
                dangerouslySetInnerHTML={{
                  __html: renderContent(content) || '<span class="text-muted">Click to edit</span>',
                }}
              />
            )}
          </div>
        )}

        {/* Hover actions */}
        {!isEditing && (
          <div className="journal-block-actions">
            <button
              className="journal-icon-btn-sm"
              onClick={(e) => { e.stopPropagation(); onEdit(id) }}
              title="Edit"
            >
              ✏️
            </button>
            <button
              className="journal-icon-btn-sm"
              onClick={(e) => { e.stopPropagation(); onDelete(id) }}
              title="Delete"
            >
              🗑️
            </button>
          </div>
        )}
      </div>
    </div>
  )
}

// ── New Block Form ────────────────────────────────────────────────────────────

function NewBlockForm({
  parentId,
  pageId,
  onCreated,
  createBlock,
  isPending,
}: {
  parentId: string | null
  pageId: string | null
  onCreated: (data: any) => void
  createBlock: (body: any) => Promise<any>
  isPending: boolean
}) {
  const [title, setTitle] = useState('')
  const [content, setContent] = useState('')
  const [tags, setTags] = useState('')
  const [journalDay, setJournalDay] = useState(parentId === null ? TODAY : '')

  const isPage = parentId === null

  const handleSubmit = async () => {
    if (!content.trim() && !title.trim()) return
    const body: any = { content }
    if (title) body.title = title
    if (parentId) body.parent_id = parentId
    if (tags) body.tags = tags.split(',').map((t) => t.trim()).filter(Boolean)
    if (journalDay) body.journal_day = journalDay

    try {
      const data = await createBlock(body)
      setTitle('')
      setContent('')
      setTags('')
      onCreated(data)
    } catch (e: any) {
      // Error handling via mutation
    }
  }

  return (
    <div className="journal-new-block" style={{ marginLeft: parentId ? 28 : 0 }}>
      <span className="journal-bullet">{isPage ? '◆' : '•'}</span>
      <div className="journal-new-block-form">
        {isPage && (
          <input
            type="text"
            placeholder="Page title"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            className="journal-new-title-input"
          />
        )}
        <textarea
          placeholder={isPage ? 'Start writing...' : 'New block...'}
          value={content}
          onChange={(e) => setContent(e.target.value)}
          rows={2}
          className="journal-new-textarea"
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
              e.preventDefault()
              handleSubmit()
            }
          }}
        />
        <div className="journal-new-block-meta">
          <input
            type="text"
            placeholder="tags (comma-separated)"
            value={tags}
            onChange={(e) => setTags(e.target.value)}
            style={{ flex: 1, fontSize: 11, padding: '2px 6px' }}
          />
          {isPage && (
            <input
              type="date"
              value={journalDay}
              onChange={(e) => setJournalDay(e.target.value)}
              style={{ fontSize: 11, padding: '2px 6px' }}
            />
          )}
          <button onClick={handleSubmit} className="primary" style={{ fontSize: 12 }}>
            {isPage ? 'Create Page' : 'Add Block'}
          </button>
        </div>
      </div>
    </div>
  )
}

// ── Properties Block ──────────────────────────────────────────────────────────

function PropertiesBlock({ properties }: { properties: Record<string, any> }) {
  if (!properties || Object.keys(properties).length === 0) return null

  return (
    <div className="journal-properties">
      {Object.entries(properties).map(([key, value]) => (
        <div key={key} className="journal-property-row">
          <span className="journal-property-key">{key}</span>
          <span className="journal-property-value">
            {typeof value === 'boolean' ? (value ? '✅' : '⬜') : String(value)}
          </span>
        </div>
      ))}
    </div>
  )
}
