import { useQuery } from "@tanstack/react-query";
import { Link, Outlet } from "@tanstack/react-router";
import { useCallback, useState } from "react";
import { listPlugins, listSchemas } from "../api/client";
import { ThemeSwitcher } from "../theme";
import "./AppShell.css";

export function AppShell() {
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [collapsed, setCollapsed] = useState(false);
  const closeSidebar = useCallback(() => setSidebarOpen(false), []);

  const { data: plugins = [] } = useQuery({
    queryKey: ["plugins"],
    queryFn: listPlugins,
  });

  const { data: schemas = [] } = useQuery({
    queryKey: ["schemas"],
    queryFn: listSchemas,
  });

  return (
    <div className="app-shell">
      {/* Mobile overlay */}
      <button
        type="button"
        className={`app-shell-overlay${sidebarOpen ? " open" : ""}`}
        aria-label="Close sidebar"
        onClick={closeSidebar}
      />

      {/* Sidebar */}
      <aside
        className={`app-shell-sidebar${sidebarOpen ? " open" : ""}${collapsed ? " collapsed" : ""}`}
      >
        <div className="app-shell-sidebar-inner">
          {/* Logo area */}
          <div className="app-shell-logo">
            <h1 className="app-shell-title">Panorama</h1>
            {!collapsed && (
              <p className="app-shell-subtitle">Data Layer Platform</p>
            )}
          </div>

          {/* Navigation */}
          <nav className="app-shell-nav">
            <Link
              to="/nodes"
              className="app-shell-nav-item"
              activeProps={{ className: "app-shell-nav-item active" }}
              onClick={closeSidebar}
            >
              <span className="app-shell-nav-icon">⊞</span>
              {!collapsed && <span>Nodes</span>}
            </Link>
            <Link
              to="/schemas"
              className="app-shell-nav-item"
              activeProps={{ className: "app-shell-nav-item active" }}
              onClick={closeSidebar}
            >
              <span className="app-shell-nav-icon">⊟</span>
              {!collapsed && <span>Schemas ({schemas.length})</span>}
            </Link>
            <Link
              to="/plugins"
              className="app-shell-nav-item"
              activeProps={{ className: "app-shell-nav-item active" }}
              onClick={closeSidebar}
            >
              <span className="app-shell-nav-icon">⬡</span>
              {!collapsed && <span>Plugins ({plugins.length})</span>}
            </Link>
          </nav>

          {/* Installed Apps */}
          {!collapsed && (
            <div className="app-shell-apps">
              <h3 className="app-shell-apps-heading">Installed Apps</h3>
              {plugins.map((p) => (
                <Link
                  key={p.id}
                  to="/app/$pluginId"
                  params={{ pluginId: p.id }}
                  className="app-shell-nav-item app-shell-app-item"
                  onClick={closeSidebar}
                >
                  <span className="app-shell-nav-icon">📦</span>
                  <div className="app-shell-app-info">
                    <span className="app-shell-app-name">{p.name}</span>
                    <span className="app-shell-app-version">v{p.version}</span>
                  </div>
                </Link>
              ))}
              {plugins.length === 0 && (
                <p className="app-shell-apps-empty">No apps installed</p>
              )}
            </div>
          )}

          {/* Footer */}
          <div className="app-shell-footer">
            {!collapsed && <ThemeSwitcher />}
            <button
              type="button"
              className="app-shell-collapse-btn"
              onClick={() => setCollapsed(!collapsed)}
              title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
              aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            >
              {collapsed ? "→" : "←"}
            </button>
          </div>
        </div>
      </aside>

      {/* Main content */}
      <main
        className={`app-shell-main${collapsed ? " sidebar-collapsed" : ""}`}
      >
        {/* Mobile hamburger */}
        <button
          type="button"
          className="app-shell-hamburger"
          onClick={() => setSidebarOpen(!sidebarOpen)}
          aria-label="Toggle menu"
        >
          ☰
        </button>

        <Outlet />
      </main>
    </div>
  );
}
