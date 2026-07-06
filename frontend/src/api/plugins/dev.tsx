// Dev mode: direct workspace package imports. No lazy loading needed —
// Vite bundles everything eagerly and HMR handles hot reloads.

import React from "react";

// Static imports of all plugin UIs — Vite resolves via bun workspace symlinks
import JournalApp from "panorama-plugin-journal-ui";
import DashboardsApp from "panorama-plugin-dashboards-ui";
import CodingApp from "panorama-plugin-coding-ui";
import TripsApp from "panorama-plugin-trips-ui";
import RestaurantsApp from "panorama-plugin-restaurants-ui";
import MusicApp from "panorama-plugin-music-ui";
import FilesApp from "panorama-plugin-files-ui";

type PluginComponent = React.ComponentType<{ pluginId: string }>;

const PLUGINS: Record<string, PluginComponent> = {
  "io.mzhang.panorama.journal": JournalApp,
  "io.mzhang.panorama.dashboards": DashboardsApp,
  "io.mzhang.panorama.coding": CodingApp,
  "io.mzhang.panorama.trips": TripsApp,
  "io.mzhang.panorama.restaurants": RestaurantsApp,
  "io.mzhang.panorama.music": MusicApp,
  "io.mzhang.panorama.files": FilesApp,
};

const FALLBACK: PluginComponent = () => (
  <div className="card" style={{ padding: 20, textAlign: "center" }}>
    <p className="text-muted">Plugin not found in dev workspace.</p>
  </div>
);

export function getPluginComponent(
  pluginId: string,
): Promise<{ default: PluginComponent }> {
  const Component = PLUGINS[pluginId] || FALLBACK;
  return Promise.resolve({ default: Component });
}
