// Dev mode: direct workspace package imports. No lazy loading needed —
// Vite bundles everything eagerly and HMR handles hot reloads.

import React from 'react'

// Static imports of all plugin UIs — Vite resolves via bun workspace symlinks
import JournalApp from 'panorama-plugin-journal-ui'
import GrafanaApp from 'panorama-plugin-grafana-ui'
import WakatimeApp from 'panorama-plugin-wakatime-ui'
import TripsApp from 'panorama-plugin-trips-ui'
import BeliApp from 'panorama-plugin-beli-ui'
import SubsonicApp from 'panorama-plugin-subsonic-ui'
import FilesApp from 'panorama-plugin-files-ui'

type PluginComponent = React.ComponentType<{ pluginId: string }>

const PLUGINS: Record<string, PluginComponent> = {
  'io.mzhang.panorama.journal':  JournalApp,
  'io.mzhang.panorama.grafana':  GrafanaApp,
  'io.mzhang.panorama.wakatime': WakatimeApp,
  'io.mzhang.panorama.trips':    TripsApp,
  'io.mzhang.panorama.beli':     BeliApp,
  'io.mzhang.panorama.subsonic': SubsonicApp,
  'io.mzhang.panorama.files':    FilesApp,
}

const FALLBACK: PluginComponent = () => (
  <div className="card" style={{ padding: 20, textAlign: 'center' }}>
    <p className="text-muted">Plugin not found in dev workspace.</p>
  </div>
)

export function loadPluginComponent(
  pluginId: string,
): React.LazyExoticComponent<PluginComponent> {
  const Component = PLUGINS[pluginId] || FALLBACK
  // Wrap in React.lazy for API consistency with prod.tsx,
  // but the component is already loaded — it resolves instantly.
  return React.lazy(() => Promise.resolve({ default: Component }))
}
