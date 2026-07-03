import { useQuery } from '@tanstack/react-query'
import { listPlugins, listSchemas, type PluginInfo } from './api/client'
import { NodeViewer } from './components/NodeViewer'
import { PluginPanel } from './components/PluginPanel'
import { SchemaViewer } from './components/SchemaViewer'
import { useState, lazy, Suspense, useEffect, useRef, useMemo } from 'react'
import {
  registerPluginRemote,
  loadPluginComponent,
} from './api/plugin-loader'

type View = 'nodes' | 'schemas' | 'plugins' | 'app'

// ── Legacy hardcoded plugin UI map ──────────────────────────────────────────
// Entries here are removed as plugins are migrated to Module Federation.
// Once all are migrated, this map and the fallback logic can be deleted.
const legacyComponents: Record<
  string,
  React.LazyExoticComponent<React.ComponentType<{}>>
> = {
  'com.panorama.journal': lazy(() => import('./plugins/journal')),
  'com.panorama.wakatime': lazy(() => import('./plugins/wakatime')),
  'com.panorama.grafana': lazy(() => import('./plugins/grafana')),
  'com.panorama.trips': lazy(() => import('./plugins/trips')),
  'com.panorama.beli': lazy(() => import('./plugins/beli')),
  'com.panorama.subsonic': lazy(() => import('./plugins/subsonic')),
  'com.panorama.files': lazy(() => import('./plugins/files')),
}

// ── Federated plugins ───────────────────────────────────────────────────────
// Add plugin IDs here as they are migrated to Module Federation.
// The host will try federation first; if a plugin is NOT in this set,
// it falls back to the legacy hardcoded import above.
const FEDERATED_PLUGINS: Set<string> = new Set([
  // 'com.panorama.journal',  // enable when backend serves UI assets
])

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

  const selectedPluginInfo = plugins.find((p) => p.id === selectedPlugin)

  // ── Resolve plugin component ──────────────────────────────────────────────
  const useFederation =
    selectedPlugin != null && FEDERATED_PLUGINS.has(selectedPlugin)

  const [FederatedComponent, setFederatedComponent] =
    useState<React.LazyExoticComponent<
      React.ComponentType<{ pluginId: string }>
    > | null>(null)

  useEffect(() => {
    if (useFederation && selectedPlugin) {
      registerPluginRemote(selectedPlugin)
      setFederatedComponent(() => loadPluginComponent(selectedPlugin))
    } else {
      setFederatedComponent(null)
    }
  }, [selectedPlugin, useFederation])

  const PluginComponent = useMemo((): React.ComponentType<{
    pluginId: string
  }> | null => {
    if (!selectedPlugin) return null
    if (useFederation && FederatedComponent) return FederatedComponent
    // Fall back to legacy hardcoded import — wrap to accept pluginId prop
    const Legacy = legacyComponents[selectedPlugin]
    if (!Legacy) return null
    return ((props: { pluginId: string }) => (
      <Suspense fallback={<p>Loading app...</p>}>
        <Legacy />
      </Suspense>
    )) as unknown as React.ComponentType<{ pluginId: string }>
  }, [selectedPlugin, useFederation, FederatedComponent])

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
        {view === 'app' && PluginComponent && (
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
              <PluginComponent pluginId={selectedPlugin!} />
            </Suspense>
          </div>
        )}
      </main>
    </div>
  )
}
