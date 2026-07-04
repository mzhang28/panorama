/// <reference types="vite/client" />

// Plugin component loader — dispatches to dev or prod implementation
// based on Vite build mode.
//
//   vite dev                      → workspace imports + HMR
//   vite build --mode development → workspace imports (E2E)
//   vite build                    → Module Federation (production)
//
// Uses a dynamic import so Rollup only bundles the relevant module:
// production mode never even parses dev.tsx.

import React from 'react'

type PluginComponent = React.ComponentType<{ pluginId: string }>

export function loadPluginComponent(
  pluginId: string,
): React.LazyExoticComponent<PluginComponent> {
  if (import.meta.env.MODE === 'production') {
    // Dynamic import — Rollup bundles only this path in production
    const mod = import('./plugins/prod')
    return React.lazy(() => mod.then(m => m.loadPluginComponent(pluginId) as any))
  }
  // Dynamic import — Rollup bundles only this path in dev/E2E
  const mod = import('./plugins/dev')
  return React.lazy(() => mod.then(m => m.loadPluginComponent(pluginId) as any))
}
