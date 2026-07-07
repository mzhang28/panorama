/// <reference types="vite/client" />

import { useQuery } from "@tanstack/react-query";
import {
  createRootRoute,
  createRoute,
  createRouter,
  Link,
  Navigate,
  useLocation,
  useNavigate,
  useParams,
} from "@tanstack/react-router";
import { Suspense, useCallback, useMemo } from "react";
import { listPlugins, listSchemas } from "./api/client";
import { loadPluginComponent } from "./api/plugin-loader";
import { AppShell } from "./components/AppShell";
import { NodeExplorerHome } from "./components/NodeExplorerHome";
import { PluginPanel } from "./components/PluginPanel";
import { SchemaViewer } from "./components/SchemaViewer";

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

// /app/$pluginId and splat
const appBaseRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/app/$pluginId",
  component: PluginAppView,
});

const appIndexRoute = createRoute({
  getParentRoute: () => appBaseRoute,
  path: "/",
});

const appSplatRoute = createRoute({
  getParentRoute: () => appBaseRoute,
  path: "$",
});

function PluginAppView() {
  const { pluginId } = useParams({ from: "/app/$pluginId" });
  const navigate = useNavigate();
  const location = useLocation();

  // Extract subpath from location.pathname
  // e.g. /app/io.mzhang.panorama.journal/page/123 -> "page/123"
  const prefix = `/app/${pluginId}`;
  let subpath = "";
  if (location.pathname.startsWith(prefix)) {
    subpath = location.pathname.slice(prefix.length);
    if (subpath.startsWith("/")) subpath = subpath.slice(1);
  }

  const PluginComponent = useMemo(
    () => loadPluginComponent(pluginId as string),
    [pluginId],
  );

  const handleNavigate = useCallback(
    (subpath: string) => {
      if (subpath) {
        navigate({ to: `/app/${pluginId}/${subpath}` });
      } else {
        navigate({ to: `/app/${pluginId}` });
      }
    },
    [navigate, pluginId],
  );

  return (
    <div key={pluginId}>
      <Link to="/plugins" style={{ marginBottom: 16, display: "inline-block" }}>
        ← Back to Plugins
      </Link>
      <Suspense fallback={<p>Loading app...</p>}>
        <PluginComponent
          pluginId={pluginId as string}
          subpath={subpath}
          navigate={handleNavigate}
        />
      </Suspense>
    </div>
  );
}

// ── Route tree ────────────────────────────────────────────────────────────────

const routeTree = rootRoute.addChildren([
  indexRoute,
  nodesRoute,
  schemasRoute,
  pluginsRoute,
  appBaseRoute.addChildren([appIndexRoute, appSplatRoute]),
]);

// ── Router creation ───────────────────────────────────────────────────────────

export const router = createRouter({ routeTree });

// Register router type for type-safe navigation
declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
