import { useQuery } from '@tanstack/react-query'
import { listPlugins, listSchemas } from './api/client'
import { NodeViewer } from './components/NodeViewer'
import { PluginPanel } from './components/PluginPanel'
import { SchemaViewer } from './components/SchemaViewer'
import { useState, Suspense, useEffect } from 'react'
import {
  registerPluginRemote,
  loadPluginComponent,
} from './api/plugin-loader'

type View = 'nodes' | 'schemas' | 'plugins' | 'app'

export default function App() {
  const [view, setView] = useState<View>('nodes')
  const [selectedPlugin, setSelectedPlugin] = useState<string | null>(null)

  const { data: plugins = [] } = useQuery({
    queryKey: ['plugins'],
    queryFn: listPlugins,
  })

  const { data: schemas = [] } = useQuery({
    queryKey: ['schemas'],
    queryFn: listSchemas,
  })

  // ── Dynamically load the selected plugin via Module Federation ─────────────
  const [PluginComponent, setPluginComponent] = useState<React.LazyExoticComponent<
    React.ComponentType<{ pluginId: string }>
  > | null>(null)

  useEffect(() => {
    if (selectedPlugin) {
      registerPluginRemote(selectedPlugin)
      setPluginComponent(() => loadPluginComponent(selectedPlugin))
    } else {
      setPluginComponent(null)
    }
  }, [selectedPlugin])

  return (
    <div className="app-container">
      <aside className="sidebar">
        <h1 style={{ fontSize: 20, fontWeight: 700, marginBottom: 8 }}>
          Panorama
        </h1>
        <p className="text-muted">Data Layer Platform</p>

        <nav
          style={{
            marginTop: 20,
            display: 'flex',
            flexDirection: 'column',
            gap: 4,
          }}
        >
          <button
            className={view === 'nodes' ? 'primary' : ''}
            onClick={() => {
              setView('nodes')
              setSelectedPlugin(null)
            }}
          >
            Nodes
          </button>
          <button
            className={view === 'schemas' ? 'primary' : ''}
            onClick={() => {
              setView('schemas')
              setSelectedPlugin(null)
            }}
          >
            Schemas ({schemas.length})
          </button>
          <button
            className={view === 'plugins' ? 'primary' : ''}
            onClick={() => {
              setView('plugins')
              setSelectedPlugin(null)
            }}
          >
            Plugins ({plugins.length})
          </button>
        </nav>

        <div style={{ marginTop: 20 }}>
          <h3
            style={{
              fontSize: 12,
              textTransform: 'uppercase',
              color: 'var(--text-muted)',
              marginBottom: 8,
            }}
          >
            Installed Apps
          </h3>
          {plugins.map((p) => (
            <button
              key={p.id}
              style={{
                display: 'block',
                width: '100%',
                textAlign: 'left',
                marginTop: 4,
                background:
                  selectedPlugin === p.id ? 'var(--bg-hover)' : undefined,
              }}
              onClick={() => {
                setSelectedPlugin(
                  selectedPlugin === p.id ? null : p.id,
                )
                setView(
                  selectedPlugin === p.id ? 'plugins' : 'app',
                )
              }}
            >
              <div style={{ fontWeight: 600 }}>{p.name}</div>
              <div className="text-muted" style={{ fontSize: 11 }}>
                v{p.version}
              </div>
            </button>
          ))}
          {plugins.length === 0 && (
            <p className="text-muted" style={{ fontSize: 12 }}>
              No apps installed. Place .panoapp files in data/plugins/
            </p>
          )}
        </div>
      </aside>

      <main className="main-content">
        {view === 'nodes' && <NodeViewer />}
        {view === 'schemas' && <SchemaViewer schemas={schemas} />}
        {view === 'plugins' && (
          <PluginPanel
            plugins={plugins}
            selectedId={selectedPlugin}
            onSelect={(id) => {
              setSelectedPlugin(id)
              setView('app')
            }}
          />
        )}
        {view === 'app' && PluginComponent && selectedPlugin && (
          <div>
            <button
              onClick={() => {
                setView('plugins')
                setSelectedPlugin(null)
              }}
              style={{ marginBottom: 16 }}
            >
              ← Back to Plugins
            </button>
            <Suspense fallback={<p>Loading app...</p>}>
              <PluginComponent pluginId={selectedPlugin} />
            </Suspense>
          </div>
        )}
      </main>
    </div>
  )
}
