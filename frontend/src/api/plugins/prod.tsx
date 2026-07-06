// Production mode: Module Federation runtime loading.
// Plugins are loaded from .panoapp archives served by the backend at
// /plugin/{pluginId}/ui/remoteEntry.js.

import {
  init,
  loadRemote,
  registerRemotes,
} from "@module-federation/enhanced/runtime";
import type React from "react";

type PluginComponent = React.ComponentType<{
  pluginId: string;
  subpath?: string;
  navigate?: (subpath: string) => void;
}>;

// Initialize the Module Federation runtime once at module load time
init({
  name: "panorama_host",
  remotes: [],
});

const registeredRemotes = new Set<string>();

function pluginIdToRemoteName(pluginId: string): string {
  return pluginId.replace(/[^a-zA-Z0-9_]/g, "_");
}

function registerPluginRemote(pluginId: string): void {
  const name = pluginIdToRemoteName(pluginId);
  if (registeredRemotes.has(name)) return;
  const entry = `/plugin/${pluginId}/ui/remoteEntry.js`;
  registerRemotes([{ name, entry, type: "module" }]);
  registeredRemotes.add(name);
}

export function getPluginComponent(
  pluginId: string,
): Promise<{ default: PluginComponent }> {
  registerPluginRemote(pluginId);
  const remoteName = pluginIdToRemoteName(pluginId);

  return loadRemote<{ default: PluginComponent }>(`${remoteName}/App`)
    .then((mod) => ({ default: mod?.default ?? (() => null) }))
    .catch((err) => {
      console.error(
        `Failed to load federated plugin UI for "${pluginId}":`,
        err,
      );
      return {
        default: (() => (
          <div className="card" style={{ padding: 20, textAlign: "center" }}>
            <p className="text-muted">
              Plugin UI "{pluginId}" could not be loaded.
            </p>
            <p className="text-muted" style={{ fontSize: 12 }}>
              Check that the plugin is installed and its UI bundle is available.
            </p>
          </div>
        )) as PluginComponent,
      };
    });
}
