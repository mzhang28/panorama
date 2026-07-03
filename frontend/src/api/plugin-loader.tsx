/// <reference types="vite/client" />

// Dynamic plugin loading via Module Federation v2
// Handles registering remotes at runtime and lazy-loading plugin components

import React from 'react'
import { init, loadRemote, registerRemotes } from '@module-federation/enhanced/runtime'

// Initialize the Module Federation runtime once at module load time
init({
  name: 'panorama_host',
  remotes: [],
})

// Set of already-registered remote names to avoid re-registration
const registeredRemotes = new Set<string>()

// Dev mode: map plugin IDs to their local Vite dev server URLs.
// In production (vite build + embedded frontend) these are ignored because
// import.meta.env.DEV is false — plugins always load from the backend.
const DEV_PLUGIN_URLS: Record<string, string> = {
  'io.mzhang.panorama.journal': 'http://localhost:5174',
  'io.mzhang.panorama.wakatime': 'http://localhost:5175',
  'io.mzhang.panorama.grafana': 'http://localhost:5176',
  'io.mzhang.panorama.trips': 'http://localhost:5177',
  'io.mzhang.panorama.beli': 'http://localhost:5178',
  'io.mzhang.panorama.subsonic': 'http://localhost:5179',
  'io.mzhang.panorama.files': 'http://localhost:5180',
}

function getRemoteEntryUrl(pluginId: string): string {
  if (import.meta.env.DEV && DEV_PLUGIN_URLS[pluginId]) {
    return `${DEV_PLUGIN_URLS[pluginId]}/remoteEntry.js`
  }
  // Production: served by the Rust backend from the .panoapp archive
  return `/plugin/${pluginId}/ui/remoteEntry.js`
}

/** Sanitize a plugin ID into a valid Module Federation remote name */
function pluginIdToRemoteName(pluginId: string): string {
  return pluginId.replace(/[^a-zA-Z0-9_]/g, '_')
}

/**
 * Register a plugin's remote entry with the Module Federation runtime.
 * Idempotent — safe to call multiple times for the same plugin.
 * Must be called before attempting to load the plugin's components.
 */
export function registerPluginRemote(pluginId: string): void {
  const name = pluginIdToRemoteName(pluginId)
  if (registeredRemotes.has(name)) return

  const entry = getRemoteEntryUrl(pluginId)
  // type: 'module' — the Vite federation plugin outputs ESM remote entries
  registerRemotes([{ name, entry, type: 'module' }])
  registeredRemotes.add(name)
}

/**
 * Returns a lazy-loaded React component for a plugin's exposed App module.
 * The exposed module name is always `./App` by convention.
 */
export function loadPluginComponent(
  pluginId: string,
): React.LazyExoticComponent<React.ComponentType<{ pluginId: string }>> {
  const remoteName = pluginIdToRemoteName(pluginId)

  return React.lazy(() =>
    loadRemote<{ default: React.ComponentType<{ pluginId: string }> }>(`${remoteName}/App`)
      .then((mod) => ({ default: mod?.default ?? (() => null) }))
      .catch((err) => {
        console.error(`Failed to load federated plugin UI for "${pluginId}":`, err)
        // Return a fallback component on failure
        return {
          default: (() => (
            <div className="card" style={{ padding: 20, textAlign: 'center' }}>
              <p className="text-muted">
                Plugin UI &quot;{pluginId}&quot; could not be loaded.
              </p>
              <p className="text-muted" style={{ fontSize: 12 }}>
                Check that the plugin is installed and its UI bundle is available.
              </p>
            </div>
          )) as React.ComponentType<{ pluginId: string }>,
        }
      }),
  )
}
