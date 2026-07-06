# Node Explorer Home — Design Spec

**Date:** 2026-07-06
**Status:** Approved
**Inspiration:** Revenue Dashboard (Shadcn UI Explorer) — visual design only, no code reuse

## Overview

Replace the existing `NodeViewer.tsx` (full-featured data table with sidebar filters, detail panel, histogram) with a new "Node Explorer Home" dashboard. Simultaneously revamp the app shell (`RootLayout` in `routes.tsx`) to match the inspiration's sidebar + inset content layout.

**Key principle:** Port the visuals (dark sidebar, inset rounded main content, stat cards, charts, condensed table) using UnoCSS + design tokens. Do NOT use shadcn code or Tailwind classes.

## Architecture

### Two-Layer Design

```
AppShell (replaces RootLayout in routes.tsx)
├── <aside> — dark sidebar (nav, apps list, theme toggle)
└── <main>  — inset content area (rounded-xl, shadow, margin)
    └── <Outlet />
         └── NodeExplorerHome (at route /nodes, replaces NodeViewer)
              ├── Top bar (title, search, "Explore" button [stub])
              ├── NodeStatsBar (stat cards, derived from nodes[])
              ├── NodeActivityChart (nivo bar chart, bucketed by time)
              └── NodeTableCondensed (TanStack Table, search + paginate)
```

### Data Flow

- Single `useQuery({ queryKey: ["nodes"], queryFn: listNodes })` at `NodeExplorerHome` level
- All three children receive `nodes: Node[]` as props — no duplicate fetching
- Stats are derived: `nodes.length`, unique schema count, 24h count
- Timeline is derived: group nodes by time buckets
- Table receives the raw array

### Component Props Interfaces

```typescript
interface NodeStatsBarProps {
  nodes: Node[];
}

interface NodeActivityChartProps {
  nodes: Node[];
}

interface NodeTableCondensedProps {
  nodes: Node[];
}
```

All components are independently storybook-able with mock data.

## Component Specifications

### AppShell

Replaces the current `RootLayout` component. Sidebar matches the inspiration's structure:

| Element | Content |
|---|---|
| Logo area | "Panorama" + "Data Layer Platform" subtitle |
| Nav items | Nodes, Schemas (count), Plugins (count) — with icons |
| Apps section | "Installed Apps" heading + plugin list (name + version) |
| Footer | Collapse toggle + ThemeSwitcher |

**Sidebar states:**
- Expanded: `var(--sidebar-width)` = 16rem
- Collapsed: `var(--sidebar-width-collapsed)` = 3rem
- Mobile: overlay with backdrop, same as current behavior

**Inset content area:** when sidebar is expanded, `<main>` gets margin (m-2), rounded corners (rounded-[var(--radius-lg)]), and subtle shadow. Content scrolls within this inset area.

**Data dependencies:** Same as current RootLayout — `useQuery(["plugins"], listPlugins)`, `useQuery(["schemas"], listSchemas)`.

### NodeExplorerHome

The content at `/nodes`. Layout (top to bottom):

1. **Top bar** — "Nodes" title on left, global search input center/right, "Explore" button (navigates to full table — wired later, stubbed now)

2. **NodeStatsBar** — Row of 3-4 stat cards:
   - Total nodes count
   - Unique schemas count
   - Nodes created in last 24h
   - (Flex slot for additional stats later)

3. **NodeActivityChart** — nivo `@nivo/bar` bar chart. X-axis: time buckets (auto: hourly for <24h range, daily for >24h). Y-axis: node count. Uses CSS variable tokens for colors.

4. **NodeTableCondensed** — TanStack Table with:
   - Columns: Title, Schema, Space, Updated At
   - Global text search (filters rows client-side)
   - Pagination at bottom
   - Row hover highlight
   - **Stripped out from old NodeViewer:** filter sidebar, column visibility toggles, histogram, detail panel, live mode toggle. These return in a future "Explore" full-table view.

### NodeStatsBar

Simple flex row of cards. Each card: muted label on top, large number below, subtle border + background from design tokens. No sparkline charts (the inspiration had them, but we decided against KPI-style sparklines here).

### NodeActivityChart

nivo `<ResponsiveBar>` with:
- Data: nodes grouped into time buckets (derived from `nodes[]` in-component)
- Theme: fed from CSS variable tokens (text color, grid color)
- Colors: `var(--accent)` for bars
- Responsive container fills parent width, fixed height (~200px)
- Lightweight — no tooltips needed initially, just the bars

### NodeTableCondensed

Stripped-down TanStack Table:
- `getCoreRowModel`, `getPaginationRowModel`, `getFilteredRowModel` (global text filter)
- Column definitions: Title (primary, sortable), Schema (badge), Space (badge), Updated At (relative time)
- Page size: 10-15 rows
- Row click → no-op for now (detail interaction deferred to "Explore" view)

## Design Tokens (added to index.css)

### Radius Scale (3 stops)

```css
--radius-sm: 0.375rem;   /* buttons, inputs, badges */
--radius-md: 0.5rem;     /* cards, table, chart containers */
--radius-lg: 0.75rem;    /* inset shell, large panels */
```

### Width Scale

```css
--sidebar-width: 16rem;
--sidebar-width-collapsed: 3rem;
--content-max-width: 1200px;
```

### Spacing Scale (4px grid)

```css
--space-1: 0.25rem;
--space-2: 0.5rem;
--space-3: 0.75rem;
--space-4: 1rem;
--space-6: 1.5rem;
--space-8: 2rem;
```

### Semantic Surface Colors

```css
--surface: var(--bg);
--surface-elevated: var(--bg-card);
--surface-inset: var(--bg);
--surface-hover: var(--bg-hover);
```

### Usage Rule

Always reference tokens, never arbitrary values:
- `rounded-[var(--radius-lg)]` not `rounded-xl`
- `w-[var(--sidebar-width)]` not `w-64`
- `p-[var(--space-4)]` not `p-4`

## File Changes

### New Files

```
frontend/src/components/
├── AppShell.tsx
├── AppShell.css
├── NodeExplorerHome.tsx
├── NodeStatsBar.tsx
├── NodeActivityChart.tsx
└── NodeTableCondensed.tsx

frontend/src/stories/
├── AppShell.stories.tsx
├── NodeExplorerHome.stories.tsx
├── NodeStatsBar.stories.tsx
├── NodeActivityChart.stories.tsx
└── NodeTableCondensed.stories.tsx
```

### Modified Files

```
frontend/src/routes.tsx        — AppShell replaces RootLayout, NodeExplorerHome at /nodes
frontend/src/index.css         — Add design tokens + inset shell styles + sidebar polish
```

### Deleted Files

```
frontend/src/components/NodeViewer.tsx   (replaced by NodeExplorerHome)
frontend/src/components/NodeExplorer.css (replaced by AppShell.css + index.css tokens)
```

## Dependencies

**New:** `@nivo/bar`, `@nivo/core` — for the activity timeline chart

## Storybook Stories

Each component gets a story with mock data:

- **AppShell** — rendered as a full-page decorator with mock route content. Shows expanded, collapsed, and mobile states.
- **NodeExplorerHome** — mock `nodes[]` array, full dashboard layout.
- **NodeStatsBar** — mock nodes, verify derived stat counts.
- **NodeActivityChart** — mock nodes spread across time, verify bar rendering.
- **NodeTableCondensed** — mock nodes (10-50), verify search + pagination.

## What is NOT in Scope

- The "Explore" full-table view (separate future work)
- Node detail panel / inline editing
- Live mode polling
- Filter sidebar / column visibility toggles (these return in Explore)
- Histogram (replaced by the nivo activity chart)
- Keyboard shortcut modal
- Replacing `SchemaViewer` or `PluginPanel` — they stay as-is for now, just render inside the new AppShell via `<Outlet />`

## Visual Reference

`3rd-party/dashboard.png` — the rendered Shadcn UI Explorer "Revenue Dashboard" block.
Key borrowed elements:
- Dark sidebar with icon nav + collapsible behavior
- Inset main content area (margin + rounded corners + shadow when sidebar is open)
- Clean stat card row
- Chart + table grid layout
