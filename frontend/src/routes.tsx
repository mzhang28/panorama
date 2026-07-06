/// <reference types="vite/client" />

import { useQuery } from "@tanstack/react-query";
import {
  createRootRoute,
  createRoute,
  createRouter,
  Link,
  Navigate,
  useParams,
} from "@tanstack/react-router";
import { useState, useEffect, Suspense } from "react";
import { listPlugins, listSchemas } from "./api/client";
import { NodeExplorerHome } from "./components/NodeExplorerHome";
import { AppShell } from "./components/AppShell";
import { PluginPanel } from "./components/PluginPanel";
import { SchemaViewer } from "./components/SchemaViewer";
import { JournalApp } from "../../crates/panorama-app-journal/ui/src/JournalApp";
import { loadPluginComponent } from "./api/plugin-loader";

// ── Route definitions ─────────────────────────────────────────────────────────

const rootRoute = createRootRoute({
  component: AppShell,
});

// Index: redirect to /nodes
const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: () => <Navigate to="/nodes" />,
});

// /nodes
const nodesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/nodes",
  component: () => <NodeExplorerHome />,
});

// /schemas
const schemasRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/schemas",
  component: SchemasView,
});

function SchemasView() {
  const { data: schemas = [] } = useQuery({
    queryKey: ["schemas"],
    queryFn: listSchemas,
  });
  return <SchemaViewer schemas={schemas} />;
}

// /plugins
const pluginsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/plugins",
  component: PluginsView,
});

function PluginsView() {
  const { data: plugins = [] } = useQuery({
    queryKey: ["plugins"],
    queryFn: listPlugins,
  });
  return <PluginPanel plugins={plugins} />;
}

// /app/$pluginId
const appRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/app/$pluginId",
  component: PluginAppView,
});

function PluginAppView() {
  const { pluginId } = useParams({ from: "/app/$pluginId" });
  const [PluginComponent, setPluginComponent] =
    useState<React.LazyExoticComponent<
      React.ComponentType<{ pluginId: string }>
    > | null>(null);

  useEffect(() => {
    setPluginComponent(() => loadPluginComponent(pluginId));
  }, [pluginId]);

  // Journal uses the full-featured host component (not the plugin UI remote)
  if (pluginId === "io.mzhang.panorama.journal") {
    return (
      <div>
        <Link
          to="/plugins"
          style={{ marginBottom: 16, display: "inline-block" }}
        >
          ← Back to Plugins
        </Link>
        <JournalApp />
      </div>
    );
  }

  return (
    <div>
      <Link to="/plugins" style={{ marginBottom: 16, display: "inline-block" }}>
        ← Back to Plugins
      </Link>
      {PluginComponent && (
        <Suspense fallback={<p>Loading app...</p>}>
          <PluginComponent pluginId={pluginId} />
        </Suspense>
      )}
    </div>
  );
}

// ── Route tree ────────────────────────────────────────────────────────────────

const routeTree = rootRoute.addChildren([
  indexRoute,
  nodesRoute,
  schemasRoute,
  pluginsRoute,
  appRoute,
]);

// ── Router creation ───────────────────────────────────────────────────────────

export const router = createRouter({ routeTree });

// Register router type for type-safe navigation
declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
