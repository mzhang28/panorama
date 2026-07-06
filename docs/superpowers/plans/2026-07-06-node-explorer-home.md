# Node Explorer Home — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `NodeViewer.tsx` with a dashboard-style `NodeExplorerHome` inside a revamped `AppShell` (dark sidebar + inset content), plus Storybook stories for every component.

**Architecture:** `AppShell` (sidebar + `<Outlet />`) wraps all routes. `NodeExplorerHome` at `/nodes` composes `NodeStatsBar`, `NodeActivityChart` (nivo), and `NodeTableCondensed` — all receiving `Node[]` as props from a single `useQuery`. UnoCSS with design tokens (radius, width, spacing scales) for consistent visuals.

**Tech Stack:** React 18, TanStack Router + Query + Table, nivo (@nivo/bar + @nivo/core), UnoCSS, Storybook 8

## Global Constraints

- Use `rounded-[var(--radius-lg)]` not ad-hoc `rounded-xl` — always reference design tokens
- All new components in `frontend/src/components/`, stories in `frontend/src/stories/`
- Follow existing Storybook story pattern from `CodingApp.stories.tsx` (Meta + StoryObj)
- Delete `NodeViewer.tsx` and `NodeExplorer.css` only after new components are wired in
- `@nivo/bar` and `@nivo/core` are the only new dependencies
- The `AppShell` must work as a TanStack Router root route component (uses `<Outlet />`)

---

### Task 1: Install nivo + add design tokens

**Files:**
- Modify: `frontend/package.json`
- Modify: `frontend/src/index.css`

**Interfaces:**
- Produces: CSS variables available to all components: `--radius-sm`, `--radius-md`, `--radius-lg`, `--sidebar-width`, `--sidebar-width-collapsed`, `--content-max-width`, `--space-1` through `--space-8`, `--surface`, `--surface-elevated`, `--surface-inset`, `--surface-hover`
- Produces: `@nivo/bar` and `@nivo/core` available for import

- [ ] **Step 1: Install nivo packages**

```bash
cd frontend && npm install @nivo/bar @nivo/core
```

- [ ] **Step 2: Add design tokens to index.css**

Add after the `[data-theme="dark"]` block's closing `}` and before the `[data-theme="light"]` block — add these tokens inside BOTH `:root, [data-theme="dark"]` and `[data-theme="light"]` blocks, and also add them to the `@media (prefers-color-scheme: light)` block.

The tokens are theme-agnostic (same values regardless of theme), so add them once inside a new `:root` block right after the existing `:root, [data-theme="dark"]` block closes (after line 52):

```css
/* Shared design tokens — theme-agnostic */
:root {
  --radius-sm: 0.375rem;
  --radius-md: 0.5rem;
  --radius-lg: 0.75rem;

  --sidebar-width: 16rem;
  --sidebar-width-collapsed: 3rem;
  --content-max-width: 1200px;

  --space-1: 0.25rem;
  --space-2: 0.5rem;
  --space-3: 0.75rem;
  --space-4: 1rem;
  --space-6: 1.5rem;
  --space-8: 2rem;

  --surface: var(--bg);
  --surface-elevated: var(--bg-card);
  --surface-inset: var(--bg);
  --surface-hover: var(--bg-hover);
}
```

- [ ] **Step 3: Update the existing *.css classes that use hardcoded values**

In `index.css`, update the `.sidebar` rule (line 272-273) to use the new token:

```css
.sidebar {
  width: var(--sidebar-width);
  min-width: var(--sidebar-width);
  ...
}
```

Update the `.card` rule (line 317) to use `--radius-md`:

```css
.card {
  ...
  border-radius: var(--radius-md);
  ...
}
```

- [ ] **Step 4: Commit**

```bash
rtk git add frontend/package.json frontend/package-lock.json frontend/src/index.css && rtk git commit -m "feat: add nivo charts and design tokens (radius, width, spacing scales)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: NodeStatsBar component + story

**Files:**
- Create: `frontend/src/components/NodeStatsBar.tsx`
- Create: `frontend/src/stories/mock-nodes.ts`
- Create: `frontend/src/stories/NodeStatsBar.stories.tsx`

**Interfaces:**
- Consumes: `Node` type from `frontend/src/api/client.ts`
- Produces: `NodeStatsBar` component with `{ nodes: Node[] }` props

- [ ] **Step 1: Create mock-nodes helper**

```typescript
// frontend/src/stories/mock-nodes.ts
import { Node } from "../api/client";

export function createMockNode(overrides: Partial<Node> = {}): Node {
  const now = Date.now();
  return {
    id: "00000000-0000-0000-0000-000000000001",
    fields: {
      "system:node_title": { type: "string", value: "Example Node" },
    },
    space_id: "00000000-0000-0000-0000-000000000000",
    preferred_schemas: [
      { schema_node_id: "schema-1", version: { major: 1, minor: 0 } },
    ],
    created_at: new Date(now - 3600000).toISOString(),
    updated_at: new Date(now).toISOString(),
    ...overrides,
  };
}

export function createMockNodes(count: number): Node[] {
  return Array.from({ length: count }, (_, i) =>
    createMockNode({
      id: `node-${String(i).padStart(3, "0")}`,
      fields: {
        "system:node_title": {
          type: "string",
          value: `Node ${i + 1}`,
        },
      },
      preferred_schemas: [
        {
          schema_node_id: i % 3 === 0 ? "schema-blog" : i % 3 === 1 ? "schema-user" : "schema-page",
          version: { major: 1, minor: 0 },
        },
      ],
      space_id: i % 2 === 0 ? "space-personal" : "space-work",
      created_at: new Date(Date.now() - i * 7200000).toISOString(),
      updated_at: new Date(Date.now() - i * 600000).toISOString(),
    }),
  );
}
```

- [ ] **Step 2: Create NodeStatsBar component**

```typescript
// frontend/src/components/NodeStatsBar.tsx
import { useMemo } from "react";
import { Node } from "../api/client";

interface NodeStatsBarProps {
  nodes: Node[];
}

export function NodeStatsBar({ nodes }: NodeStatsBarProps) {
  const stats = useMemo(() => {
    const total = nodes.length;
    const uniqueSchemas = new Set(
      nodes.flatMap((n) =>
        n.preferred_schemas.map((s) => s.schema_node_id),
      ),
    ).size;
    const last24h = nodes.filter((n) => {
      const created = new Date(n.created_at).getTime();
      return Date.now() - created < 24 * 60 * 60 * 1000;
    }).length;
    const uniqueSpaces = new Set(nodes.map((n) => n.space_id)).size;

    return { total, uniqueSchemas, last24h, uniqueSpaces };
  }, [nodes]);

  const items = [
    { label: "Total Nodes", value: stats.total },
    { label: "Schemas", value: stats.uniqueSchemas },
    { label: "Created (24h)", value: stats.last24h },
    { label: "Spaces", value: stats.uniqueSpaces },
  ];

  return (
    <div className="flex gap-[var(--space-4)] flex-wrap">
      {items.map((item) => (
        <div
          key={item.label}
          className="flex flex-col gap-[var(--space-1)] bg-[var(--surface-elevated)] border border-[var(--border)] rounded-[var(--radius-md)] p-[var(--space-4)] flex-1 min-w-[160px]"
        >
          <span className="text-[var(--text-muted)] text-xs font-medium uppercase tracking-wide">
            {item.label}
          </span>
          <span className="text-[var(--text)] text-2xl font-bold tabular-nums">
            {item.value.toLocaleString()}
          </span>
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 3: Create NodeStatsBar story**

```typescript
// frontend/src/stories/NodeStatsBar.stories.tsx
import type { Meta, StoryObj } from "@storybook/react";
import { NodeStatsBar } from "../components/NodeStatsBar";
import { createMockNodes } from "./mock-nodes";

const meta: Meta<typeof NodeStatsBar> = {
  title: "Core/NodeStatsBar",
  component: NodeStatsBar,
};

export default meta;
type Story = StoryObj<typeof NodeStatsBar>;

export const Default: Story = {
  args: {
    nodes: createMockNodes(50),
  },
};

export const Empty: Story = {
  args: {
    nodes: [],
  },
};

export const SingleNode: Story = {
  args: {
    nodes: createMockNodes(1),
  },
};
```

- [ ] **Step 4: Verify story renders**

```bash
cd frontend && npx storybook dev --no-open -p 6006 &
# Wait for ready, then check the story loads
# Kill the dev server after verifying
```

- [ ] **Step 5: Commit**

```bash
rtk git add frontend/src/components/NodeStatsBar.tsx frontend/src/stories/mock-nodes.ts frontend/src/stories/NodeStatsBar.stories.tsx && rtk git commit -m "feat: add NodeStatsBar component with Storybook story

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: NodeActivityChart component + story

**Files:**
- Create: `frontend/src/components/NodeActivityChart.tsx`
- Create: `frontend/src/stories/NodeActivityChart.stories.tsx`

**Interfaces:**
- Consumes: `Node` type, `@nivo/bar` (ResponsiveBar), `mock-nodes.ts`
- Produces: `NodeActivityChart` component with `{ nodes: Node[] }` props

- [ ] **Step 1: Create NodeActivityChart component**

```typescript
// frontend/src/components/NodeActivityChart.tsx
import { useMemo } from "react";
import { ResponsiveBar } from "@nivo/bar";
import { Node } from "../api/client";

interface NodeActivityChartProps {
  nodes: Node[];
}

interface Bucket {
  label: string;
  count: number;
}

function bucketNodes(nodes: Node[]): Bucket[] {
  if (nodes.length === 0) return [];

  const now = Date.now();
  const timestamps = nodes.map((n) => new Date(n.created_at).getTime());
  const oldest = Math.min(...timestamps);
  const rangeMs = now - oldest;

  // Use hourly buckets if range < 24h, otherwise daily
  const useHourly = rangeMs < 24 * 60 * 60 * 1000;
  const bucketMs = useHourly ? 60 * 60 * 1000 : 24 * 60 * 60 * 1000;

  // Determine number of buckets (aim for ~12-24 buckets)
  const numBuckets = Math.min(Math.max(Math.ceil(rangeMs / bucketMs), 6), 24);
  const adjustedBucketMs = rangeMs / numBuckets;

  const buckets: Bucket[] = [];
  for (let i = 0; i < numBuckets; i++) {
    const bucketStart = now - (numBuckets - i) * adjustedBucketMs;
    const bucketEnd = now - (numBuckets - i - 1) * adjustedBucketMs;

    const count = nodes.filter((n) => {
      const t = new Date(n.created_at).getTime();
      return t >= bucketStart && t < bucketEnd;
    }).length;

    const label = useHourly
      ? new Date(bucketStart).toLocaleTimeString(undefined, {
          hour: "numeric",
          minute: "2-digit",
        })
      : new Date(bucketStart).toLocaleDateString(undefined, {
          month: "short",
          day: "numeric",
        });

    buckets.push({ label, count });
  }

  return buckets;
}

export function NodeActivityChart({ nodes }: NodeActivityChartProps) {
  const data = useMemo(() => bucketNodes(nodes), [nodes]);

  if (data.length === 0) {
    return (
      <div className="bg-[var(--surface-elevated)] border border-[var(--border)] rounded-[var(--radius-md)] p-[var(--space-6)]">
        <h3 className="text-[var(--text-muted)] text-xs font-medium uppercase tracking-wide mb-[var(--space-4)]">
          Activity Timeline
        </h3>
        <p className="text-[var(--text-dim)] text-sm text-center py-8">
          No activity data available
        </p>
      </div>
    );
  }

  return (
    <div className="bg-[var(--surface-elevated)] border border-[var(--border)] rounded-[var(--radius-md)] p-[var(--space-6)]">
      <h3 className="text-[var(--text-muted)] text-xs font-medium uppercase tracking-wide mb-[var(--space-2)]">
        Activity Timeline
      </h3>
      <div style={{ height: 200 }}>
        <ResponsiveBar
          data={data}
          keys={["count"]}
          indexBy="label"
          margin={{ top: 8, right: 8, bottom: 40, left: 40 }}
          padding={0.3}
          valueScale={{ type: "linear" }}
          colors={["var(--accent)"]}
          borderRadius={4}
          axisBottom={{
            tickSize: 5,
            tickPadding: 5,
            tickRotation: -45,
            tickValues: Math.min(data.length, 12),
          }}
          axisLeft={{
            tickSize: 5,
            tickPadding: 5,
            tickValues: 5,
          }}
          gridYValues={5}
          theme={{
            text: {
              fontSize: 11,
              fill: "var(--text-muted)",
            },
            grid: {
              line: {
                stroke: "var(--border)",
                strokeWidth: 1,
              },
            },
            axis: {
              domain: {
                line: {
                  stroke: "var(--border)",
                  strokeWidth: 1,
                },
              },
              ticks: {
                line: {
                  stroke: "var(--border)",
                  strokeWidth: 1,
                },
              },
            },
          }}
          enableLabel={false}
          animate={true}
          motionConfig="gentle"
        />
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Create NodeActivityChart story**

```typescript
// frontend/src/stories/NodeActivityChart.stories.tsx
import type { Meta, StoryObj } from "@storybook/react";
import { NodeActivityChart } from "../components/NodeActivityChart";
import { createMockNodes } from "./mock-nodes";

const meta: Meta<typeof NodeActivityChart> = {
  title: "Core/NodeActivityChart",
  component: NodeActivityChart,
};

export default meta;
type Story = StoryObj<typeof NodeActivityChart>;

export const Default: Story = {
  args: {
    nodes: createMockNodes(50),
  },
};

export const Empty: Story = {
  args: {
    nodes: [],
  },
};

export const SingleDay: Story = {
  args: {
    nodes: createMockNodes(20),
  },
};
```

- [ ] **Step 3: Verify story renders**

```bash
cd frontend && npx storybook dev --no-open -p 6006 &
# Wait for Storybook to be ready
# Check http://localhost:6006/?path=/story/core-nodeactivitychart--default
# Verify bars render with the accent color and grid lines
# Kill the dev server after verifying
```

- [ ] **Step 4: Commit**

```bash
rtk git add frontend/src/components/NodeActivityChart.tsx frontend/src/stories/NodeActivityChart.stories.tsx && rtk git commit -m "feat: add NodeActivityChart with nivo bar chart

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: NodeTableCondensed component + story

**Files:**
- Create: `frontend/src/components/NodeTableCondensed.tsx`
- Create: `frontend/src/stories/NodeTableCondensed.stories.tsx`

**Interfaces:**
- Consumes: `Node` type, `@tanstack/react-table` (getCoreRowModel, getFilteredRowModel, getPaginationRowModel, flexRender, etc.)
- Produces: `NodeTableCondensed` component with `{ nodes: Node[] }` props

- [ ] **Step 1: Create NodeTableCondensed component**

```typescript
// frontend/src/components/NodeTableCondensed.tsx
import { useMemo, useState } from "react";
import {
  useReactTable,
  getCoreRowModel,
  getFilteredRowModel,
  getPaginationRowModel,
  flexRender,
  ColumnDef,
} from "@tanstack/react-table";
import { Node } from "../api/client";

interface EnrichedRow {
  id: string;
  title: string;
  schema: string;
  space: string;
  updatedAt: string;
}

function enrichNode(node: Node): EnrichedRow {
  const fields = node.fields || {};
  const title =
    fields["system:node_title"]?.value ||
    fields["files:filename"]?.value ||
    fields["journal:title"]?.value ||
    fields["coding:entity"]?.value ||
    fields["trips:name"]?.value ||
    fields["restaurants:name"]?.value ||
    fields["music:name"]?.value ||
    node.id.slice(0, 8);

  const schemas = node.preferred_schemas.map((s) => s.schema_node_id).join(", ") || "—";

  const updated = new Date(node.updated_at);
  const updatedAt = isNaN(updated.getTime())
    ? "—"
    : updated.toLocaleDateString(undefined, {
        month: "short",
        day: "numeric",
        year: "numeric",
      });

  return {
    id: node.id,
    title,
    schema: schemas,
    space: node.space_id,
    updatedAt,
  };
}

const columns: ColumnDef<EnrichedRow>[] = [
  {
    accessorKey: "title",
    header: "Title",
    cell: (info) => (
      <span className="font-medium text-[var(--text)]">
        {info.getValue<string>()}
      </span>
    ),
  },
  {
    accessorKey: "schema",
    header: "Schema",
    cell: (info) => (
      <span className="inline-block px-[var(--space-2)] py-[var(--space-1)] rounded-[var(--radius-sm)] bg-[var(--badge-bg)] text-[var(--text-muted)] text-xs font-mono">
        {info.getValue<string>()}
      </span>
    ),
  },
  {
    accessorKey: "space",
    header: "Space",
    cell: (info) => (
      <span className="inline-block px-[var(--space-2)] py-[var(--space-1)] rounded-[var(--radius-sm)] bg-[var(--badge-bg)] text-[var(--text-muted)] text-xs font-mono truncate max-w-[120px]">
        {info.getValue<string>()}
      </span>
    ),
  },
  {
    accessorKey: "updatedAt",
    header: "Updated",
    cell: (info) => (
      <span className="text-[var(--text-dim)] text-sm tabular-nums">
        {info.getValue<string>()}
      </span>
    ),
  },
];

interface NodeTableCondensedProps {
  nodes: Node[];
}

export function NodeTableCondensed({ nodes }: NodeTableCondensedProps) {
  const [globalFilter, setGlobalFilter] = useState("");
  const data = useMemo(() => nodes.map(enrichNode), [nodes]);

  const table = useReactTable({
    data,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    state: { globalFilter },
    onGlobalFilterChange: setGlobalFilter,
    initialState: { pagination: { pageSize: 10 } },
  });

  if (nodes.length === 0) {
    return (
      <div className="bg-[var(--surface-elevated)] border border-[var(--border)] rounded-[var(--radius-md)] p-[var(--space-6)]">
        <p className="text-[var(--text-dim)] text-sm text-center py-8">
          No nodes found
        </p>
      </div>
    );
  }

  return (
    <div className="bg-[var(--surface-elevated)] border border-[var(--border)] rounded-[var(--radius-md)] overflow-hidden">
      {/* Search bar */}
      <div className="p-[var(--space-4)] border-b border-[var(--border)]">
        <input
          type="text"
          placeholder="Search nodes..."
          value={globalFilter}
          onChange={(e) => setGlobalFilter(e.target.value)}
          className="w-full max-w-[320px] bg-[var(--bg-input)] border border-[var(--border)] rounded-[var(--radius-sm)] px-[var(--space-3)] py-[var(--space-2)] text-sm text-[var(--text)] placeholder:text-[var(--text-dim)] focus:border-[var(--accent)] outline-none"
        />
      </div>

      {/* Table */}
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            {table.getHeaderGroups().map((headerGroup) => (
              <tr key={headerGroup.id} className="border-b border-[var(--border)]">
                {headerGroup.headers.map((header) => (
                  <th
                    key={header.id}
                    className="text-left px-[var(--space-4)] py-[var(--space-3)] text-[var(--text-muted)] text-xs font-semibold uppercase tracking-wide bg-[var(--telemetry-thead-bg)]"
                  >
                    {flexRender(
                      header.column.columnDef.header,
                      header.getContext(),
                    )}
                  </th>
                ))}
              </tr>
            ))}
          </thead>
          <tbody>
            {table.getRowModel().rows.map((row) => (
              <tr
                key={row.id}
                className="border-b border-[var(--border)] transition-colors hover:bg-[var(--surface-hover)]"
              >
                {row.getVisibleCells().map((cell) => (
                  <td
                    key={cell.id}
                    className="px-[var(--space-4)] py-[var(--space-3)]"
                  >
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* Pagination */}
      <div className="flex items-center justify-between px-[var(--space-4)] py-[var(--space-3)] border-t border-[var(--border)]">
        <span className="text-[var(--text-dim)] text-xs">
          {table.getFilteredRowModel().rows.length} of {nodes.length} nodes
        </span>
        <div className="flex items-center gap-[var(--space-2)]">
          <button
            onClick={() => table.previousPage()}
            disabled={!table.getCanPreviousPage()}
            className="px-[var(--space-3)] py-[var(--space-1)] text-xs border border-[var(--border)] rounded-[var(--radius-sm)] bg-[var(--bg)] text-[var(--text)] disabled:opacity-30 disabled:cursor-not-allowed hover:bg-[var(--surface-hover)]"
          >
            Previous
          </button>
          <span className="text-[var(--text-dim)] text-xs tabular-nums">
            {table.getState().pagination.pageIndex + 1} /{" "}
            {table.getPageCount() || 1}
          </span>
          <button
            onClick={() => table.nextPage()}
            disabled={!table.getCanNextPage()}
            className="px-[var(--space-3)] py-[var(--space-1)] text-xs border border-[var(--border)] rounded-[var(--radius-sm)] bg-[var(--bg)] text-[var(--text)] disabled:opacity-30 disabled:cursor-not-allowed hover:bg-[var(--surface-hover)]"
          >
            Next
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Create NodeTableCondensed story**

```typescript
// frontend/src/stories/NodeTableCondensed.stories.tsx
import type { Meta, StoryObj } from "@storybook/react";
import { NodeTableCondensed } from "../components/NodeTableCondensed";
import { createMockNodes } from "./mock-nodes";

const meta: Meta<typeof NodeTableCondensed> = {
  title: "Core/NodeTableCondensed",
  component: NodeTableCondensed,
};

export default meta;
type Story = StoryObj<typeof NodeTableCondensed>;

export const Default: Story = {
  args: {
    nodes: createMockNodes(25),
  },
};

export const Empty: Story = {
  args: {
    nodes: [],
  },
};

export const ManyNodes: Story = {
  args: {
    nodes: createMockNodes(100),
  },
};
```

- [ ] **Step 3: Verify story renders**

```bash
cd frontend && npx storybook dev --no-open -p 6006 &
# Check http://localhost:6006/?path=/story/core-nodetablecondensed--default
# Verify search, pagination, column rendering
# Kill the dev server after verifying
```

- [ ] **Step 4: Commit**

```bash
rtk git add frontend/src/components/NodeTableCondensed.tsx frontend/src/stories/NodeTableCondensed.stories.tsx && rtk git commit -m "feat: add NodeTableCondensed with search and pagination

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: NodeExplorerHome component + story

**Files:**
- Create: `frontend/src/components/NodeExplorerHome.tsx`
- Create: `frontend/src/stories/NodeExplorerHome.stories.tsx`

**Interfaces:**
- Consumes: `NodeStatsBar`, `NodeActivityChart`, `NodeTableCondensed`, `queryNodes` from API client, `useQuery` from TanStack Query
- Produces: `NodeExplorerHome` component — no props (fetches its own data)

- [ ] **Step 1: Create NodeExplorerHome component**

```typescript
// frontend/src/components/NodeExplorerHome.tsx
import { useQuery } from "@tanstack/react-query";
import { queryNodes } from "../api/client";
import { NodeStatsBar } from "./NodeStatsBar";
import { NodeActivityChart } from "./NodeActivityChart";
import { NodeTableCondensed } from "./NodeTableCondensed";

export function NodeExplorerHome() {
  const {
    data: nodes = [],
    isLoading,
    error,
  } = useQuery({
    queryKey: ["nodes"],
    queryFn: () => queryNodes(),
  });

  return (
    <div className="flex flex-col gap-[var(--space-6)]">
      {/* Top bar */}
      <div className="flex items-center justify-between flex-wrap gap-[var(--space-4)]">
        <h1 className="text-xl font-bold text-[var(--text)]">Nodes</h1>
        <div className="flex items-center gap-[var(--space-3)]">
          <button className="px-[var(--space-4)] py-[var(--space-2)] border border-[var(--border)] rounded-[var(--radius-sm)] bg-[var(--surface-elevated)] text-[var(--text)] text-sm hover:bg-[var(--surface-hover)] transition-colors">
            Explore →
          </button>
        </div>
      </div>

      {/* Loading state */}
      {isLoading && (
        <div className="flex items-center justify-center py-16">
          <p className="text-[var(--text-dim)]">Loading nodes...</p>
        </div>
      )}

      {/* Error state */}
      {error && (
        <div className="bg-[var(--danger-subtle)] border border-[var(--danger)] rounded-[var(--radius-md)] p-[var(--space-4)]">
          <p className="text-[var(--danger)] text-sm">
            Failed to load nodes: {(error as Error).message}
          </p>
        </div>
      )}

      {/* Content */}
      {!isLoading && !error && (
        <>
          <NodeStatsBar nodes={nodes} />
          <NodeActivityChart nodes={nodes} />
          <NodeTableCondensed nodes={nodes} />
        </>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Create NodeExplorerHome story**

```typescript
// frontend/src/stories/NodeExplorerHome.stories.tsx
import type { Meta, StoryObj } from "@storybook/react";
import { NodeExplorerHome } from "../components/NodeExplorerHome";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

// Create a QueryClient that returns mock data
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
    },
  },
});

const meta: Meta<typeof NodeExplorerHome> = {
  title: "Core/NodeExplorerHome",
  component: NodeExplorerHome,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <div className="p-[var(--space-6)]">
          <Story />
        </div>
      </QueryClientProvider>
    ),
  ],
  parameters: {
    // This story requires a mock server or MSW — for now, it shows the loading state
    mockData: true,
  },
};

export default meta;
type Story = StoryObj<typeof NodeExplorerHome>;

export const Default: Story = {};
```

- [ ] **Step 3: Commit**

```bash
rtk git add frontend/src/components/NodeExplorerHome.tsx frontend/src/stories/NodeExplorerHome.stories.tsx && rtk git commit -m "feat: add NodeExplorerHome composing stats, chart, and table

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: AppShell component + story

**Files:**
- Create: `frontend/src/components/AppShell.tsx`
- Create: `frontend/src/components/AppShell.css`
- Create: `frontend/src/stories/AppShell.stories.tsx`

**Interfaces:**
- Consumes: `listPlugins`, `listSchemas`, `PluginInfo` from API client, `ThemeSwitcher` from theme, `Link` + `Outlet` from TanStack Router, `useQuery` from TanStack Query
- Produces: `AppShell` component — renders sidebar + inset `<main>` with `<Outlet />`

- [ ] **Step 1: Create AppShell component**

```typescript
// frontend/src/components/AppShell.tsx
import { useState, useCallback } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link, Outlet } from "@tanstack/react-router";
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
      <div
        className={`app-shell-overlay${sidebarOpen ? " open" : ""}`}
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
                <p className="app-shell-apps-empty">
                  No apps installed
                </p>
              )}
            </div>
          )}

          {/* Footer */}
          <div className="app-shell-footer">
            {!collapsed && <ThemeSwitcher />}
            <button
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
```

- [ ] **Step 2: Create AppShell CSS**

```css
/* frontend/src/components/AppShell.css */

/* ── Layout ───────────────────────────────────────────────── */

.app-shell {
  display: flex;
  height: 100vh;
  max-height: 100vh;
  overflow: hidden;
  position: relative;
  background: var(--surface);
}

/* ── Overlay (mobile) ─────────────────────────────────────── */

.app-shell-overlay {
  display: none;
  position: fixed;
  inset: 0;
  background: var(--overlay-bg);
  z-index: 99;
}

.app-shell-overlay.open {
  display: block;
}

/* ── Sidebar ──────────────────────────────────────────────── */

.app-shell-sidebar {
  width: var(--sidebar-width);
  min-width: var(--sidebar-width);
  height: 100vh;
  max-height: 100vh;
  background: var(--surface-elevated);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  transition: width 0.25s ease, min-width 0.25s ease;
  z-index: 100;
  overflow: hidden;
}

.app-shell-sidebar.collapsed {
  width: var(--sidebar-width-collapsed);
  min-width: var(--sidebar-width-collapsed);
}

.app-shell-sidebar-inner {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: var(--space-4);
  gap: var(--space-2);
  overflow-y: auto;
}

/* ── Logo ─────────────────────────────────────────────────── */

.app-shell-logo {
  margin-bottom: var(--space-4);
}

.app-shell-title {
  font-size: 20px;
  font-weight: 700;
  color: var(--text);
  margin: 0;
  line-height: 1.2;
}

.app-shell-subtitle {
  font-size: 12px;
  color: var(--text-muted);
  margin: 4px 0 0 0;
}

/* ── Navigation ───────────────────────────────────────────── */

.app-shell-nav {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.app-shell-nav-item {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  color: var(--text);
  text-decoration: none;
  font-size: 14px;
  font-weight: 500;
  transition: background-color 0.15s ease;
}

.app-shell-nav-item:hover {
  background: var(--surface-hover);
  color: var(--text);
}

.app-shell-nav-item.active {
  background: var(--accent-subtle);
  color: var(--accent);
}

.app-shell-nav-icon {
  font-size: 16px;
  flex-shrink: 0;
  width: 20px;
  text-align: center;
}

/* ── Installed Apps ───────────────────────────────────────── */

.app-shell-apps {
  margin-top: var(--space-4);
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.app-shell-apps-heading {
  font-size: 11px;
  text-transform: uppercase;
  color: var(--text-muted);
  letter-spacing: 0.5px;
  font-weight: 600;
  margin: 0 0 var(--space-2) 0;
  padding: 0 var(--space-3);
}

.app-shell-app-item {
  font-size: 13px;
}

.app-shell-app-info {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.app-shell-app-name {
  font-weight: 600;
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.app-shell-app-version {
  font-size: 11px;
  color: var(--text-dim);
}

.app-shell-apps-empty {
  font-size: 12px;
  color: var(--text-dim);
  padding: 0 var(--space-3);
}

/* ── Footer ───────────────────────────────────────────────── */

.app-shell-footer {
  margin-top: auto;
  padding-top: var(--space-3);
  border-top: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.app-shell-collapse-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-1) var(--space-2);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg);
  color: var(--text-muted);
  font-size: 14px;
  cursor: pointer;
  min-height: 28px;
  transition: all 0.15s ease;
}

.app-shell-collapse-btn:hover {
  background: var(--surface-hover);
  color: var(--text);
}

/* ── Main content area (inset) ────────────────────────────── */

.app-shell-main {
  flex: 1;
  height: 100vh;
  max-height: 100vh;
  overflow-y: auto;
  padding: var(--space-6);
  min-width: 0;
  background: var(--surface);
  transition: margin 0.25s ease, border-radius 0.25s ease;
}

/* Inset effect when sidebar is expanded (matches inspiration) */
.app-shell-main:not(.sidebar-collapsed) {
  margin: var(--space-3);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
  background: var(--surface-inset);
  height: calc(100vh - 2 * var(--space-3));
}

/* ── Hamburger (mobile) ───────────────────────────────────── */

.app-shell-hamburger {
  display: none;
  background: none;
  border: none;
  font-size: 24px;
  padding: var(--space-1) var(--space-2);
  min-height: unset;
  line-height: 1;
  cursor: pointer;
  color: var(--text);
  margin-bottom: var(--space-4);
}

/* ── Responsive ───────────────────────────────────────────── */

@media (max-width: 768px) {
  .app-shell-hamburger {
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .app-shell-sidebar {
    position: fixed;
    top: 0;
    left: 0;
    bottom: 0;
    transform: translateX(-100%);
  }

  .app-shell-sidebar.open {
    transform: translateX(0);
  }

  .app-shell-main {
    padding: var(--space-4);
    margin: 0 !important;
    border-radius: 0 !important;
    height: 100vh !important;
    box-shadow: none !important;
  }
}
```

- [ ] **Step 3: Create AppShell story**

```typescript
// frontend/src/stories/AppShell.stories.tsx
import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ThemeProvider } from "../theme";
import { AppShell } from "../components/AppShell";

// AppShell uses <Outlet /> from TanStack Router. In Storybook, we render
// it directly without a router, passing children via a wrapper approach.
// Since AppShell's Outlet renders nothing without a router, we wrap it
// in a simple story that shows the shell chrome with dummy content.

// To make the story work, we render AppShell-like markup inline.
// For a proper integration test, use the full app with the real router.

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});

// We render a static version of the sidebar + main area for visual verification.
// The real AppShell is tested via the app itself.
import { useState } from "react";

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
          <h1 style={{ fontSize: 20, fontWeight: 700, color: "var(--text)", margin: 0 }}>Panorama</h1>
          <p style={{ fontSize: 12, color: "var(--text-muted)", margin: "4px 0 0" }}>Data Layer Platform</p>
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
              <span style={{ fontSize: 16, width: 20, textAlign: "center" }}>⊞</span>
              {item}
            </div>
          ))}
        </nav>

        <div style={{ marginTop: "var(--space-4)" }}>
          <h3 style={{ fontSize: 11, textTransform: "uppercase", color: "var(--text-muted)", letterSpacing: "0.5px", fontWeight: 600, margin: "0 0 var(--space-2)", padding: "0 var(--space-3)" }}>Installed Apps</h3>
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
                <div style={{ fontSize: 11, color: "var(--text-dim)" }}>v1.0.0</div>
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
        <div style={{ color: "var(--text-muted)", fontSize: 14 }}>Content area — rendered via &lt;Outlet /&gt;</div>
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
        <div style={{ display: "flex", height: "100vh", background: "var(--bg)" }}>
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
            <h1 style={{ fontSize: 16, fontWeight: 700, color: "var(--text)", writingMode: "vertical-rl" }}>P</h1>
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
            <div style={{ color: "var(--text-muted)", fontSize: 14 }}>Content area — sidebar collapsed</div>
          </main>
        </div>
      </ThemeProvider>
    </QueryClientProvider>
  ),
};
```

- [ ] **Step 4: Verify story renders**

```bash
cd frontend && npx storybook dev --no-open -p 6006 &
# Check http://localhost:6006/?path=/story/core-appshell--default
# Verify dark sidebar, inset rounded main content, shadow
# Check collapsed variant too
# Kill the dev server after verifying
```

- [ ] **Step 5: Commit**

```bash
rtk git add frontend/src/components/AppShell.tsx frontend/src/components/AppShell.css frontend/src/stories/AppShell.stories.tsx && rtk git commit -m "feat: add AppShell with dark sidebar and inset content area

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 7: Update routes.tsx + delete old files

**Files:**
- Modify: `frontend/src/routes.tsx`
- Delete: `frontend/src/components/NodeViewer.tsx`
- Delete: `frontend/src/components/NodeExplorer.css`
- Modify: `frontend/src/index.css` (remove `.app-container`, `.sidebar`, `.main-content`, `.hamburger`, `.sidebar-overlay` rules now handled by AppShell.css)

**Interfaces:**
- Consumes: `AppShell`, `NodeExplorerHome` (replacing `RootLayout`, `NodeViewer`)

- [ ] **Step 1: Update routes.tsx**

Replace the import of `NodeViewer` and the `RootLayout` function. The new file:

```typescript
/// <reference types="vite/client" />

import { useQuery } from "@tanstack/react-query";
import {
  createRootRoute,
  createRoute,
  createRouter,
  Link,
  Navigate,
  Outlet,
  useParams,
} from "@tanstack/react-router";
import { useState, useCallback, useEffect, Suspense } from "react";
import { listPlugins, listSchemas } from "./api/client";
import { NodeExplorerHome } from "./components/NodeExplorerHome";
import { AppShell } from "./components/AppShell";
import { PluginPanel } from "./components/PluginPanel";
import { SchemaViewer } from "./components/SchemaViewer";
import { JournalApp } from "./components/JournalApp";
import { loadPluginComponent } from "./api/plugin-loader";

import { ThemeSwitcher } from "./theme";

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
```

- [ ] **Step 2: Delete old files**

```bash
rm frontend/src/components/NodeViewer.tsx
rm frontend/src/components/NodeExplorer.css
```

- [ ] **Step 3: Clean up index.css — remove deprecated layout classes**

Remove the following rules from `index.css` since they are now in `AppShell.css`:
- `.app-container` (lines 262-268)
- `.sidebar` (lines 271-289)
- `.main-content` (lines 291-299)
- `.hamburger` (lines 302-312)
- `.sidebar-overlay` (lines 355-361)
- The responsive block for these (lines 364-395, specifically the `.hamburger`, `.sidebar`, `.sidebar.open`, `.sidebar-overlay.open`, `.main-content` rules within the `@media (max-width: 768px)` block)

Also remove unused imports that were only in `RootLayout` (the ones that are no longer needed after replacing with `AppShell`).

- [ ] **Step 4: Commit**

```bash
rtk git add frontend/src/routes.tsx frontend/src/index.css && rtk git rm frontend/src/components/NodeViewer.tsx frontend/src/components/NodeExplorer.css && rtk git commit -m "feat: wire AppShell and NodeExplorerHome into routes, remove old NodeViewer

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 8: Verify — build, typecheck, and E2E test

**Files:**
- (no file changes — verification only)

**Interfaces:**
- Consumes: all previous tasks

- [ ] **Step 1: TypeScript check**

```bash
cd frontend && npx tsc --noEmit
```

Expected: no errors. If errors exist, fix them.

- [ ] **Step 2: Storybook build check**

```bash
cd frontend && npx build-storybook
```

Expected: builds successfully, all stories compiled.

- [ ] **Step 3: Full E2E test**

Per CLAUDE.md: "all tests must pass" means `just test-e2e` has zero exit status.

```bash
just test-e2e
```

Expected: zero exit status. Address any failures before completion.

- [ ] **Step 4: Commit any fixes**

If any fixes were needed:

```bash
rtk git add -A && rtk git commit -m "fix: address verification failures

Co-Authored-By: Claude <noreply@anthropic.com>"
```
