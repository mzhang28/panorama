import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ThemeProvider } from "../theme";

// AppShell uses <Outlet /> from TanStack Router. In Storybook, we render
// it directly without a router, passing children via a wrapper approach.
// Since AppShell's Outlet renders nothing without a router, we wrap it
// in a simple story that shows the shell chrome with dummy content.

// To make the story work, we render AppShell-like markup inline.
// For a proper integration test, use the full app with the real router.

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});

function AppShellDemo() {
  return (
    <div style={{ display: "flex", height: "100vh", background: "var(--bg)" }}>
      {/* Sidebar (static demo) */}
      <aside
        style={{
          width: "var(--sidebar-width)",
          minWidth: "var(--sidebar-width)",
          height: "100vh",
          background: "var(--surface-elevated)",
          borderRight: "1px solid var(--border)",
          padding: "var(--space-4)",
          display: "flex",
          flexDirection: "column",
          gap: "var(--space-2)",
        }}
      >
        <div style={{ marginBottom: "var(--space-4)" }}>
          <h1
            style={{
              fontSize: 20,
              fontWeight: 700,
              color: "var(--text)",
              margin: 0,
            }}
          >
            Panorama
          </h1>
          <p
            style={{
              fontSize: 12,
              color: "var(--text-muted)",
              margin: "4px 0 0",
            }}
          >
            Data Layer Platform
          </p>
        </div>

        <nav style={{ display: "flex", flexDirection: "column", gap: 2 }}>
          {["Nodes", "Schemas (3)", "Plugins (7)"].map((item) => (
            <div
              key={item}
              style={{
                padding: "var(--space-2) var(--space-3)",
                borderRadius: "var(--radius-md)",
                color: "var(--text)",
                fontSize: 14,
                fontWeight: 500,
                display: "flex",
                alignItems: "center",
                gap: "var(--space-3)",
              }}
            >
              <span style={{ fontSize: 16, width: 20, textAlign: "center" }}>
                ⊞
              </span>
              {item}
            </div>
          ))}
        </nav>

        <div style={{ marginTop: "var(--space-4)" }}>
          <h3
            style={{
              fontSize: 11,
              textTransform: "uppercase",
              color: "var(--text-muted)",
              letterSpacing: "0.5px",
              fontWeight: 600,
              margin: "0 0 var(--space-2)",
              padding: "0 var(--space-3)",
            }}
          >
            Installed Apps
          </h3>
          {["Coding", "Journal", "Music"].map((name) => (
            <div
              key={name}
              style={{
                padding: "var(--space-2) var(--space-3)",
                borderRadius: "var(--radius-md)",
                display: "flex",
                alignItems: "center",
                gap: "var(--space-3)",
                fontSize: 13,
                color: "var(--text)",
              }}
            >
              <span>📦</span>
              <div>
                <div style={{ fontWeight: 600, fontSize: 13 }}>{name}</div>
                <div style={{ fontSize: 11, color: "var(--text-dim)" }}>
                  v1.0.0
                </div>
              </div>
            </div>
          ))}
        </div>
      </aside>

      {/* Main content area (inset) */}
      <main
        style={{
          flex: 1,
          margin: "var(--space-3)",
          borderRadius: "var(--radius-lg)",
          boxShadow: "var(--shadow-lg)",
          background: "var(--surface-inset)",
          padding: "var(--space-6)",
          overflow: "auto",
        }}
      >
        <div style={{ color: "var(--text-muted)", fontSize: 14 }}>
          Content area — rendered via &lt;Outlet /&gt;
        </div>
      </main>
    </div>
  );
}

const meta: Meta<typeof AppShellDemo> = {
  title: "Core/AppShell",
  component: AppShellDemo,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <ThemeProvider>
          <Story />
        </ThemeProvider>
      </QueryClientProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof AppShellDemo>;

export const Default: Story = {};

export const Collapsed: Story = {
  render: () => (
    <QueryClientProvider client={queryClient}>
      <ThemeProvider>
        <div
          style={{ display: "flex", height: "100vh", background: "var(--bg)" }}
        >
          {/* Collapsed sidebar */}
          <aside
            style={{
              width: "var(--sidebar-width-collapsed)",
              minWidth: "var(--sidebar-width-collapsed)",
              height: "100vh",
              background: "var(--surface-elevated)",
              borderRight: "1px solid var(--border)",
              padding: "var(--space-4)",
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
            }}
          >
            <h1
              style={{
                fontSize: 16,
                fontWeight: 700,
                color: "var(--text)",
                writingMode: "vertical-rl",
              }}
            >
              P
            </h1>
          </aside>

          <main
            style={{
              flex: 1,
              margin: "var(--space-3)",
              borderRadius: "var(--radius-lg)",
              boxShadow: "var(--shadow-lg)",
              background: "var(--surface-inset)",
              padding: "var(--space-6)",
              overflow: "auto",
            }}
          >
            <div style={{ color: "var(--text-muted)", fontSize: 14 }}>
              Content area — sidebar collapsed
            </div>
          </main>
        </div>
      </ThemeProvider>
    </QueryClientProvider>
  ),
};
