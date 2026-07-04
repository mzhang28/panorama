/// <reference types="vite/client" />

import { useQuery } from '@tanstack/react-query'
import {
  createRootRoute,
  createRoute,
  createRouter,
  Link,
  Navigate,
  Outlet,
  useParams,
} from '@tanstack/react-router'
import { useState, useCallback, useEffect, Suspense } from 'react'
import { listPlugins, listSchemas } from './api/client'
import { NodeViewer } from './components/NodeViewer'
import { PluginPanel } from './components/PluginPanel'
import { SchemaViewer } from './components/SchemaViewer'
import { JournalApp } from './components/JournalApp'
import {
  registerPluginRemote,
  loadPluginComponent,
} from './api/plugin-loader'

// ── Root layout ───────────────────────────────────────────────────────────────

function RootLayout() {
  const [sidebarOpen, setSidebarOpen] = useState(false)
  const closeSidebar = useCallback(() => setSidebarOpen(false), [])

  const { data: plugins = [] } = useQuery({
    queryKey: ['plugins'],
    queryFn: listPlugins,
  })

  const { data: schemas = [] } = useQuery({
    queryKey: ['schemas'],
    queryFn: listSchemas,
  })

  return (
    <div className="app-container">
      {/* Sidebar overlay for mobile */}
      <div
        className={`sidebar-overlay${sidebarOpen ? ' open' : ''}`}
        onClick={closeSidebar}
      />

      <aside className={`sidebar${sidebarOpen ? ' open' : ''}`}>
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
          <Link to="/nodes" className="nav-btn" activeProps={{ className: 'nav-btn primary' }} onClick={closeSidebar}>
            Nodes
          </Link>
          <Link to="/schemas" className="nav-btn" activeProps={{ className: 'nav-btn primary' }} onClick={closeSidebar}>
            Schemas ({schemas.length})
          </Link>
          <Link to="/plugins" className="nav-btn" activeProps={{ className: 'nav-btn primary' }} onClick={closeSidebar}>
            Plugins ({plugins.length})
          </Link>
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
            <Link
              key={p.id}
              to="/app/$pluginId"
              params={{ pluginId: p.id }}
              className="nav-btn plugin-nav-btn"
              onClick={closeSidebar}
              style={{
                display: 'block',
                width: '100%',
                textAlign: 'left',
                marginTop: 4,
              }}
            >
              <div style={{ fontWeight: 600 }}>{p.name}</div>
              <div className="text-muted" style={{ fontSize: 11 }}>
                v{p.version}
              </div>
            </Link>
          ))}
          {plugins.length === 0 && (
            <p className="text-muted" style={{ fontSize: 12 }}>
              No apps installed. Place .panoapp files in data/plugins/
            </p>
          )}
        </div>
      </aside>

      <main className="main-content">
        {/* Hamburger */}
        <button
          className="hamburger"
          onClick={() => setSidebarOpen(!sidebarOpen)}
          aria-label="Toggle menu"
          style={{ marginBottom: 12 }}
        >
          ☰
        </button>

        <Outlet />
      </main>
    </div>
  )
}

// ── Route definitions ─────────────────────────────────────────────────────────

const rootRoute = createRootRoute({
  component: RootLayout,
})

// Index: redirect to /nodes
const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  component: () => <Navigate to="/nodes" />,
})

// /nodes
const nodesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/nodes',
  component: () => <NodeViewer />,
})

// /schemas
const schemasRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/schemas',
  component: SchemasView,
})

function SchemasView() {
  const { data: schemas = [] } = useQuery({
    queryKey: ['schemas'],
    queryFn: listSchemas,
  })
  return <SchemaViewer schemas={schemas} />
}

// /plugins
const pluginsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/plugins',
  component: PluginsView,
})

function PluginsView() {
  const { data: plugins = [] } = useQuery({
    queryKey: ['plugins'],
    queryFn: listPlugins,
  })
  return <PluginPanel plugins={plugins} />
}

// /app/$pluginId
const appRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/app/$pluginId',
  component: PluginAppView,
})

function PluginAppView() {
  const { pluginId } = useParams({ from: '/app/$pluginId' })
  const [PluginComponent, setPluginComponent] = useState<
    React.LazyExoticComponent<React.ComponentType<{ pluginId: string }>> | null
  >(null)

  useEffect(() => {
    registerPluginRemote(pluginId)
    setPluginComponent(() => loadPluginComponent(pluginId))
  }, [pluginId])

  // Hardcoded Journal app (the full-featured host component)
  if (pluginId === 'io.mzhang.panorama.journal') {
    return (
      <div>
        <Link to="/plugins" style={{ marginBottom: 16, display: 'inline-block' }}>
          ← Back to Plugins
        </Link>
        <JournalApp />
      </div>
    )
  }

  // Dynamic Module Federation plugin
  return (
    <div>
      <Link to="/plugins" style={{ marginBottom: 16, display: 'inline-block' }}>
        ← Back to Plugins
      </Link>
      {PluginComponent && (
        <Suspense fallback={<p>Loading app...</p>}>
          <PluginComponent pluginId={pluginId} />
        </Suspense>
      )}
    </div>
  )
}

// ── Route tree ────────────────────────────────────────────────────────────────

const routeTree = rootRoute.addChildren([
  indexRoute,
  nodesRoute,
  schemasRoute,
  pluginsRoute,
  appRoute,
])

// ── Router creation ───────────────────────────────────────────────────────────

export const router = createRouter({ routeTree })

// Register router type for type-safe navigation
declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}
