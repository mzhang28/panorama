import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { createNode, deleteNode, getNode, Node, queryNodes, updateNode } from '../api/client'
import { useState } from 'react'

export function NodeViewer() {
  const queryClient = useQueryClient()
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [showCreate, setShowCreate] = useState(false)

  const { data: nodes = [], isLoading } = useQuery({
    queryKey: ['nodes'],
    queryFn: () => queryNodes({ limit: '50', sort_by: '-system:updated_at' }),
  })

  const { data: selected } = useQuery({
    queryKey: ['node', selectedId],
    queryFn: () => getNode(selectedId!),
    enabled: !!selectedId,
  })

  const deleteMut = useMutation({
    mutationFn: deleteNode,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['nodes'] })
      setSelectedId(null)
    },
  })

  if (isLoading) return <p>Loading nodes...</p>

  return (
    <div>
      <div className="flex-row mb-1">
        <h2>Nodes</h2>
        <button className="primary" onClick={() => setShowCreate(!showCreate)}>
          + New Node
        </button>
      </div>

      {showCreate && (
        <CreateNodeForm
          onCreated={() => {
            setShowCreate(false)
            queryClient.invalidateQueries({ queryKey: ['nodes'] })
          }}
        />
      )}

      <div style={{ display: 'grid', gap: 8, marginTop: 16 }}>
        {nodes.map((node) => (
          <div
            key={node.id}
            className="card flex-row"
            style={{ justifyContent: 'space-between', cursor: 'pointer' }}
            onClick={() => setSelectedId(node.id)}
          >
            <div>
              <strong>
                {node.fields['system:node_title']?.value || node.id.slice(0, 8)}
              </strong>
              <span className="text-muted" style={{ marginLeft: 8 }}>
                {new Date(node.updated_at).toLocaleString()}
              </span>
            </div>
            <div className="flex-row">
              <span className="text-muted">{Object.keys(node.fields).length} fields</span>
              <button
                onClick={(e) => {
                  e.stopPropagation()
                  if (confirm('Delete this node?')) deleteMut.mutate(node.id)
                }}
              >
                Delete
              </button>
            </div>
          </div>
        ))}
      </div>

      {selected && (
        <NodeDetail
          node={selected}
          onClose={() => setSelectedId(null)}
          onUpdate={() => {
            queryClient.invalidateQueries({ queryKey: ['node', selectedId] })
            queryClient.invalidateQueries({ queryKey: ['nodes'] })
          }}
        />
      )}
    </div>
  )
}

function NodeDetail({
  node,
  onClose,
  onUpdate,
}: {
  node: Node
  onClose: () => void
  onUpdate: () => void
}) {
  const queryClient = useQueryClient()
  const [editingField, setEditingField] = useState<string | null>(null)
  const [editValue, setEditValue] = useState('')

  const updateMut = useMutation({
    mutationFn: ({ id, fields }: { id: string; fields: Record<string, any> }) =>
      updateNode(id, fields),
    onSuccess: onUpdate,
  })

  const startEdit = (key: string, value: any) => {
    setEditingField(key)
    setEditValue(typeof value === 'string' ? value : JSON.stringify(value))
  }

  const saveField = () => {
    if (!editingField) return
    try {
      const parsed = JSON.parse(editValue)
      updateMut.mutate({ id: node.id, fields: { [editingField]: parsed } })
    } catch {
      updateMut.mutate({
        id: node.id,
        fields: { [editingField]: { type: 'String', value: editValue } },
      })
    }
    setEditingField(null)
  }

  return (
    <div
      className="card"
      style={{ marginTop: 16, position: 'relative' }}
    >
      <button
        onClick={onClose}
        style={{ position: 'absolute', top: 8, right: 8 }}
      >
        Close
      </button>
      <h3>Node: {node.id}</h3>
      <p className="text-muted">
        Space: {node.space_id} · Created: {new Date(node.created_at).toLocaleString()}
      </p>

      <h4 style={{ marginTop: 16 }}>Fields</h4>
      <div className="table-wrap">
        <table style={{ width: '100%', borderCollapse: 'collapse' }}>
          <thead>
            <tr>
              <th style={{ textAlign: 'left', padding: 4 }}>Key</th>
              <th style={{ textAlign: 'left', padding: 4 }}>Type</th>
              <th style={{ textAlign: 'left', padding: 4 }}>Value</th>
              <th style={{ textAlign: 'left', padding: 4 }}>Actions</th>
            </tr>
          </thead>
          <tbody>
            {Object.entries(node.fields).map(([key, field]) => (
              <tr key={key} style={{ borderTop: '1px solid var(--border)' }}>
                <td data-label="Key" style={{ padding: 4 }}>{key}</td>
                <td data-label="Type" style={{ padding: 4 }}>{field.type}</td>
                <td data-label="Value" style={{ padding: 4 }}>
                  {editingField === key ? (
                    <input
                      value={editValue}
                      onChange={(e) => setEditValue(e.target.value)}
                      onKeyDown={(e) => e.key === 'Enter' && saveField()}
                      style={{ width: '100%' }}
                    />
                  ) : (
                    <span style={{ wordBreak: 'break-all' }}>
                      {typeof field.value === 'object'
                        ? JSON.stringify(field.value)
                        : String(field.value ?? '')}
                    </span>
                  )}
                </td>
                <td data-label="Actions" style={{ padding: 4 }}>
                  {editingField === key ? (
                    <button onClick={saveField}>Save</button>
                  ) : (
                    <button onClick={() => startEdit(key, field.value)}>Edit</button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

function CreateNodeForm({ onCreated }: { onCreated: () => void }) {
  const [title, setTitle] = useState('')
  const [content, setContent] = useState('')

  const createMut = useMutation({
    mutationFn: () =>
      createNode({
        'system:node_title': { type: 'String', value: title },
        'system:node_time': { type: 'DateTime', value: new Date().toISOString() },
      }),
    onSuccess: onCreated,
  })

  return (
    <div className="card mt-1">
      <div className="flex-col">
        <input
          placeholder="Node title"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
        />
        <button className="primary" onClick={() => createMut.mutate()}>
          Create
        </button>
      </div>
    </div>
  )
}
