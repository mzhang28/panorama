import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  createNode,
  deleteNode,
  getNode,
  Node,
  queryNodes,
  updateNode,
} from "../api/client";
import { useState, useMemo } from "react";
import {
  useReactTable,
  getCoreRowModel,
  getFilteredRowModel,
  getSortedRowModel,
  getPaginationRowModel,
  flexRender,
  ColumnDef,
  SortingState,
  VisibilityState,
  RowSelectionState,
} from "@tanstack/react-table";
import "./NodeExplorer.css";

interface EnrichedNodeRow {
  original: Node;
  id: string;
  title: string;
  space_id: string;
  created_at: string;
  updated_at: string;
  created_timestamp: number;
  updated_timestamp: number;
  schemas: string[];
  namespaces: string[];
  field_count: number;
  fields: Record<string, any>;
}

function enrichNode(node: Node): EnrichedNodeRow {
  const fields = node.fields || {};
  const fieldKeys = Object.keys(fields);

  // Extract human-readable title
  const title =
    fields["system:node_title"]?.value ||
    fields["files:filename"]?.value ||
    fields["journal:title"]?.value ||
    fields["coding:entity"]?.value ||
    fields["trips:name"]?.value ||
    fields["restaurants:name"]?.value ||
    fields["music:name"]?.value ||
    node.id.slice(0, 8);

  const createdDate = new Date(node.created_at || Date.now());
  const updatedDate = new Date(node.updated_at || Date.now());

  const created_timestamp = isNaN(createdDate.getTime())
    ? Date.now()
    : createdDate.getTime();
  const updated_timestamp = isNaN(updatedDate.getTime())
    ? Date.now()
    : updatedDate.getTime();

  const formattedUpdated =
    updatedDate.toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
    }) +
    " " +
    updatedDate.toLocaleTimeString("en-US", { hour12: false });

  const formattedCreated =
    createdDate.toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
    }) +
    " " +
    createdDate.toLocaleTimeString("en-US", { hour12: false });

  const schemas = (node.preferred_schemas || []).map((s) => s.schema_node_id);

  const namespaces = Array.from(
    new Set(
      fieldKeys
        .map((k) => (k.includes(":") ? k.split(":")[0] : "system"))
        .concat(schemas),
    ),
  );

  // Raw field values map
  const rawFieldsMap: Record<string, any> = {};
  Object.entries(fields).forEach(([k, v]) => {
    rawFieldsMap[k] = v?.value !== undefined ? v.value : v;
  });

  return {
    original: node,
    id: node.id,
    title,
    space_id: node.space_id || "default",
    created_at: formattedCreated,
    updated_at: formattedUpdated,
    created_timestamp,
    updated_timestamp,
    schemas,
    namespaces,
    field_count: fieldKeys.length,
    fields: rawFieldsMap,
  };
}

export function NodeViewer() {
  const queryClient = useQueryClient();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [liveMode, setLiveMode] = useState(false);
  const [hideControls, setHideControls] = useState(false);
  const [showColumnMenu, setShowColumnMenu] = useState(false);

  // Filtering states
  const [globalFilter, setGlobalFilter] = useState("");
  const [selectedSpaces, setSelectedSpaces] = useState<Set<string>>(new Set());
  const [selectedNamespaces, setSelectedNamespaces] = useState<Set<string>>(
    new Set(),
  );
  const [selectedFields, setSelectedFields] = useState<Set<string>>(new Set());
  const [timeRange, setTimeRange] = useState<string>("3h");

  // TanStack Table states
  const [sorting, setSorting] = useState<SortingState>([
    { id: "updated_at", desc: true },
  ]);
  const [columnVisibility, setColumnVisibility] = useState<VisibilityState>({
    space_id: true,
    created_at: false,
    schemas: true,
    field_count: true,
  });
  const [rowSelection, setRowSelection] = useState<RowSelectionState>({});

  // Accordion expansion states
  const [accordionOpen, setAccordionOpen] = useState({
    columns: true,
    time: true,
    namespaces: true,
    spaces: false,
    fields: false,
  });

  // Fetch nodes
  const {
    data: rawNodes = [],
    isLoading,
    refetch,
  } = useQuery({
    queryKey: ["nodes"],
    queryFn: () => queryNodes({ limit: "200", sort_by: "-system:updated_at" }),
    refetchInterval: liveMode ? 3000 : false,
  });

  const { data: selected } = useQuery({
    queryKey: ["node", selectedId],
    queryFn: () => getNode(selectedId!),
    enabled: !!selectedId,
  });

  const deleteMut = useMutation({
    mutationFn: deleteNode,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["nodes"] });
      setSelectedId(null);
    },
  });

  const enrichedRows = useMemo(() => {
    return rawNodes.map(enrichNode);
  }, [rawNodes]);

  // Discover all unique field keys present across the dataset
  const allFieldKeys = useMemo(() => {
    const keys = new Set<string>();
    enrichedRows.forEach((r) => {
      Object.keys(r.fields).forEach((k) => keys.add(k));
    });
    return Array.from(keys).sort();
  }, [enrichedRows]);

  // Filter counts
  const spaceCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    enrichedRows.forEach((r) => {
      counts[r.space_id] = (counts[r.space_id] || 0) + 1;
    });
    return counts;
  }, [enrichedRows]);

  const namespaceCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    enrichedRows.forEach((r) => {
      r.namespaces.forEach((ns) => {
        counts[ns] = (counts[ns] || 0) + 1;
      });
    });
    return counts;
  }, [enrichedRows]);

  const fieldKeyCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    enrichedRows.forEach((r) => {
      Object.keys(r.fields).forEach((fk) => {
        counts[fk] = (counts[fk] || 0) + 1;
      });
    });
    return counts;
  }, [enrichedRows]);

  // Filter dataset based on active user selections
  const filteredData = useMemo(() => {
    return enrichedRows.filter((r) => {
      // Space filter
      if (selectedSpaces.size > 0 && !selectedSpaces.has(r.space_id)) {
        return false;
      }

      // Namespace filter
      if (
        selectedNamespaces.size > 0 &&
        !r.namespaces.some((ns) => selectedNamespaces.has(ns))
      ) {
        return false;
      }

      // Field presence filter
      if (selectedFields.size > 0) {
        const rowKeys = Object.keys(r.fields);
        if (!Array.from(selectedFields).every((f) => rowKeys.includes(f))) {
          return false;
        }
      }

      // Time range filter
      if (timeRange !== "all") {
        const now = Date.now();
        const diff = now - r.updated_timestamp;
        if (timeRange === "3h" && diff > 3 * 3600 * 1000) return false;
        if (timeRange === "24h" && diff > 24 * 3600 * 1000) return false;
        if (timeRange === "7d" && diff > 7 * 24 * 3600 * 1000) return false;
        if (timeRange === "30d" && diff > 30 * 24 * 3600 * 1000) return false;
      }

      // Global search filter
      if (globalFilter.trim()) {
        const q = globalFilter.toLowerCase();
        const matchTitle = r.title.toLowerCase().includes(q);
        const matchId = r.id.toLowerCase().includes(q);
        const matchSpace = r.space_id.toLowerCase().includes(q);
        const matchFields = Object.entries(r.fields).some(
          ([k, v]) =>
            k.toLowerCase().includes(q) || String(v).toLowerCase().includes(q),
        );
        if (!matchTitle && !matchId && !matchSpace && !matchFields) {
          return false;
        }
      }

      return true;
    });
  }, [
    enrichedRows,
    selectedSpaces,
    selectedNamespaces,
    selectedFields,
    timeRange,
    globalFilter,
  ]);

  // Build TanStack Table columns including standard metadata and dynamic node field columns
  const columns = useMemo<ColumnDef<EnrichedNodeRow>[]>(() => {
    const baseCols: ColumnDef<EnrichedNodeRow>[] = [
      {
        id: "select",
        header: ({ table }) => (
          <input
            type="checkbox"
            checked={table.getIsAllPageRowsSelected()}
            onChange={table.getToggleAllPageRowsSelectedHandler()}
          />
        ),
        cell: ({ row }) => (
          <input
            type="checkbox"
            checked={row.getIsSelected()}
            onChange={row.getToggleSelectedHandler()}
            onClick={(e) => e.stopPropagation()}
          />
        ),
      },
      {
        accessorKey: "title",
        header: ({ column }) => (
          <span
            className="sortable"
            onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
          >
            Title / Node{" "}
            {column.getIsSorted() === "asc"
              ? "↑"
              : column.getIsSorted() === "desc"
                ? "↓"
                : "↕"}
          </span>
        ),
        cell: ({ row }) => (
          <div className="pathname-cell flex-row">
            <strong>{row.original.title}</strong>
            <span className="text-muted mono" style={{ fontSize: 11 }}>
              {row.original.id.slice(0, 8)}
            </span>
          </div>
        ),
      },
      {
        accessorKey: "space_id",
        header: "Space",
        cell: (info) => (
          <span className="mono host-cell">{info.getValue() as string}</span>
        ),
      },
      {
        accessorKey: "updated_at",
        header: ({ column }) => (
          <span
            className="sortable"
            onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
          >
            Updated{" "}
            {column.getIsSorted() === "asc"
              ? "↑"
              : column.getIsSorted() === "desc"
                ? "↓"
                : "↕"}
          </span>
        ),
        cell: (info) => (
          <span className="mono">{info.getValue() as string}</span>
        ),
      },
      {
        accessorKey: "created_at",
        header: "Created",
        cell: (info) => (
          <span className="mono text-muted">{info.getValue() as string}</span>
        ),
      },
      {
        id: "schemas",
        header: "Schemas",
        cell: ({ row }) => (
          <div className="flex-row" style={{ gap: 4, flexWrap: "wrap" }}>
            {row.original.schemas.length > 0 ? (
              row.original.schemas.map((s) => (
                <span key={s} className="field-pill-badge">
                  {s}
                </span>
              ))
            ) : (
              <span className="text-muted" style={{ fontSize: 11 }}>
                none
              </span>
            )}
          </div>
        ),
      },
      {
        accessorKey: "field_count",
        header: ({ column }) => (
          <span
            className="sortable"
            onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
          >
            Fields{" "}
            {column.getIsSorted() === "asc"
              ? "↑"
              : column.getIsSorted() === "desc"
                ? "↓"
                : "↕"}
          </span>
        ),
        cell: (info) => (
          <span className="mono">{info.getValue() as number} fields</span>
        ),
      },
    ];

    // Dynamic field columns generated from dataset
    const dynamicCols: ColumnDef<EnrichedNodeRow>[] = allFieldKeys.map(
      (fk) => ({
        id: `field_${fk}`,
        header: fk,
        cell: ({ row }) => {
          const val = row.original.fields[fk];
          if (val === undefined) return <span className="text-muted">-</span>;
          return (
            <span className="mono" style={{ fontSize: 12 }}>
              {typeof val === "object" ? JSON.stringify(val) : String(val)}
            </span>
          );
        },
      }),
    );

    const actionsCol: ColumnDef<EnrichedNodeRow> = {
      id: "actions",
      header: "Actions",
      cell: ({ row }) => (
        <button
          className="explorer-btn"
          style={{ padding: "2px 8px", fontSize: 11, minHeight: "unset" }}
          onClick={(e) => {
            e.stopPropagation();
            if (confirm("Delete this node?")) deleteMut.mutate(row.original.id);
          }}
        >
          Delete
        </button>
      ),
    };

    return [...baseCols, ...dynamicCols, actionsCol];
  }, [allFieldKeys, deleteMut]);

  const table = useReactTable({
    data: filteredData,
    columns,
    state: {
      sorting,
      columnVisibility,
      rowSelection,
    },
    onSortingChange: setSorting,
    onColumnVisibilityChange: setColumnVisibility,
    onRowSelectionChange: setRowSelection,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    initialState: {
      pagination: {
        pageSize: 15,
      },
    },
  });

  // Histogram buckets computation over time
  const histogramBuckets = useMemo(() => {
    const BUCKET_COUNT = 24;
    if (enrichedRows.length === 0) return [];

    const timestamps = enrichedRows.map((r) => r.updated_timestamp);
    const minTime = Math.min(...timestamps);
    const maxTime = Math.max(...timestamps);
    const range = Math.max(1, maxTime - minTime);
    const bucketSize = range / BUCKET_COUNT;

    const buckets = Array.from({ length: BUCKET_COUNT }, (_, i) => ({
      index: i,
      startTime: minTime + i * bucketSize,
      endTime: minTime + (i + 1) * bucketSize,
      count: 0,
    }));

    enrichedRows.forEach((r) => {
      const idx = Math.min(
        BUCKET_COUNT - 1,
        Math.floor((r.updated_timestamp - minTime) / bucketSize),
      );
      if (buckets[idx]) {
        buckets[idx].count += 1;
      }
    });

    const maxBucketTotal = Math.max(1, ...buckets.map((b) => b.count));
    return buckets.map((b) => ({
      ...b,
      heightPct: (b.count / maxBucketTotal) * 100,
    }));
  }, [enrichedRows]);

  const toggleSpaceFilter = (s: string) => {
    setSelectedSpaces((prev) => {
      const next = new Set(prev);
      if (next.has(s)) next.delete(s);
      else next.add(s);
      return next;
    });
  };

  const toggleNamespaceFilter = (ns: string) => {
    setSelectedNamespaces((prev) => {
      const next = new Set(prev);
      if (next.has(ns)) next.delete(ns);
      else next.add(ns);
      return next;
    });
  };

  const toggleFieldFilter = (fk: string) => {
    setSelectedFields((prev) => {
      const next = new Set(prev);
      if (next.has(fk)) next.delete(fk);
      else next.add(fk);
      return next;
    });
  };

  if (isLoading)
    return <p style={{ padding: 16 }}>Loading nodes explorer...</p>;

  return (
    <div className="node-explorer-root">
      {/* Top Search Header */}
      <div className="explorer-top-bar">
        <h2 style={{ fontSize: 20, fontWeight: 700, marginRight: 8 }}>Nodes</h2>
        <div className="explorer-search-input-wrap">
          <span className="explorer-search-icon">🔍</span>
          <input
            className="explorer-search-input"
            placeholder="Search data table..."
            value={globalFilter}
            onChange={(e) => setGlobalFilter(e.target.value)}
          />
          <span className="explorer-search-shortcut">⌘K</span>
        </div>
        <button
          className="explorer-btn primary"
          onClick={() => setShowCreate(!showCreate)}
        >
          + New Node
        </button>
      </div>

      {showCreate && (
        <CreateNodeForm
          onCreated={() => {
            setShowCreate(false);
            queryClient.invalidateQueries({ queryKey: ["nodes"] });
          }}
        />
      )}

      {/* Sub Controls Toolbar */}
      <div className="explorer-controls-bar">
        <div className="controls-left">
          <button
            className={`explorer-btn ${hideControls ? "active" : ""}`}
            onClick={() => setHideControls(!hideControls)}
          >
            📖 {hideControls ? "Show Controls" : "Hide Controls"}
          </button>
          <span className="filtered-count-badge">
            {filteredData.length} of {enrichedRows.length} row(s) filtered
          </span>
        </div>

        <div className="controls-right">
          <button
            className="explorer-btn"
            onClick={() => refetch()}
            title="Refresh data"
          >
            ↻
          </button>
          <button
            className={`explorer-btn live-btn ${liveMode ? "active" : ""}`}
            onClick={() => setLiveMode(!liveMode)}
          >
            {liveMode && <span className="pulse-dot" />}
            {liveMode ? "Live" : "Go Live"}
          </button>
          <div style={{ position: "relative" }}>
            <button
              className="explorer-btn"
              onClick={() => setShowColumnMenu(!showColumnMenu)}
            >
              ⚙ Choose Columns ({table.getVisibleLeafColumns().length})
            </button>
            {showColumnMenu && (
              <div className="column-menu-popover">
                <strong
                  style={{
                    fontSize: 12,
                    color: "var(--telemetry-text)",
                    marginBottom: 4,
                  }}
                >
                  Display Columns
                </strong>
                {table.getAllLeafColumns().map((col) => (
                  <label key={col.id} className="checkbox-label">
                    <input
                      type="checkbox"
                      checked={col.getIsVisible()}
                      onChange={col.getToggleVisibilityHandler()}
                    />
                    <span className="mono">{col.id.replace("field_", "")}</span>
                  </label>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Main Split Layout */}
      <div className="explorer-main-layout">
        {/* Left Filters Sidebar */}
        <aside
          className={`explorer-sidebar ${hideControls ? "collapsed" : ""}`}
        >
          <div className="sidebar-title">Filters</div>

          {/* Column Display Picker Accordion */}
          <div className="filter-accordion">
            <div
              className="accordion-header"
              onClick={() =>
                setAccordionOpen((p) => ({ ...p, columns: !p.columns }))
              }
            >
              <span className="accordion-title">Choose Columns</span>
              <span
                className={`accordion-chevron ${accordionOpen.columns ? "expanded" : ""}`}
              >
                ▼
              </span>
            </div>
            {accordionOpen.columns && (
              <div className="accordion-content">
                {table.getAllLeafColumns().map((col) => (
                  <label key={col.id} className="checkbox-label">
                    <input
                      type="checkbox"
                      checked={col.getIsVisible()}
                      onChange={col.getToggleVisibilityHandler()}
                    />
                    <span className="mono">{col.id.replace("field_", "")}</span>
                  </label>
                ))}
              </div>
            )}
          </div>

          {/* Time Range Accordion */}
          <div className="filter-accordion">
            <div
              className="accordion-header"
              onClick={() => setAccordionOpen((p) => ({ ...p, time: !p.time }))}
            >
              <span className="accordion-title">Time Range</span>
              <span
                className={`accordion-chevron ${accordionOpen.time ? "expanded" : ""}`}
              >
                ▼
              </span>
            </div>
            {accordionOpen.time && (
              <div className="accordion-content">
                <select
                  style={{ width: "100%", fontSize: 13 }}
                  value={timeRange}
                  onChange={(e) => setTimeRange(e.target.value)}
                >
                  <option value="3h">Last 3 hours</option>
                  <option value="24h">Last 24 hours</option>
                  <option value="7d">Last 7 days</option>
                  <option value="30d">Last 30 days</option>
                  <option value="all">Pick a date / All time</option>
                </select>
              </div>
            )}
          </div>

          {/* Schema / Namespace Accordion */}
          {Object.keys(namespaceCounts).length > 0 && (
            <div className="filter-accordion">
              <div
                className="accordion-header"
                onClick={() =>
                  setAccordionOpen((p) => ({
                    ...p,
                    namespaces: !p.namespaces,
                  }))
                }
              >
                <span className="accordion-title">Schema / Namespace</span>
                <span
                  className={`accordion-chevron ${accordionOpen.namespaces ? "expanded" : ""}`}
                >
                  ▼
                </span>
              </div>
              {accordionOpen.namespaces && (
                <div className="accordion-content">
                  {Object.entries(namespaceCounts).map(([ns, count]) => (
                    <label key={ns} className="checkbox-label">
                      <input
                        type="checkbox"
                        checked={selectedNamespaces.has(ns)}
                        onChange={() => toggleNamespaceFilter(ns)}
                      />
                      <span className="mono">{ns}</span>
                      <span className="item-count">{count}</span>
                    </label>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Space Accordion */}
          <div className="filter-accordion">
            <div
              className="accordion-header"
              onClick={() =>
                setAccordionOpen((p) => ({ ...p, spaces: !p.spaces }))
              }
            >
              <span className="accordion-title">Space</span>
              <span
                className={`accordion-chevron ${accordionOpen.spaces ? "expanded" : ""}`}
              >
                ▼
              </span>
            </div>
            {accordionOpen.spaces && (
              <div className="accordion-content">
                {Object.entries(spaceCounts).map(([sp, count]) => (
                  <label key={sp} className="checkbox-label">
                    <input
                      type="checkbox"
                      checked={selectedSpaces.has(sp)}
                      onChange={() => toggleSpaceFilter(sp)}
                    />
                    <span className="mono">{sp}</span>
                    <span className="item-count">{count}</span>
                  </label>
                ))}
              </div>
            )}
          </div>

          {/* Filter by Field Presence */}
          {allFieldKeys.length > 0 && (
            <div className="filter-accordion">
              <div
                className="accordion-header"
                onClick={() =>
                  setAccordionOpen((p) => ({ ...p, fields: !p.fields }))
                }
              >
                <span className="accordion-title">Has Field</span>
                <span
                  className={`accordion-chevron ${accordionOpen.fields ? "expanded" : ""}`}
                >
                  ▼
                </span>
              </div>
              {accordionOpen.fields && (
                <div className="accordion-content">
                  {allFieldKeys.map((fk) => (
                    <label key={fk} className="checkbox-label">
                      <input
                        type="checkbox"
                        checked={selectedFields.has(fk)}
                        onChange={() => toggleFieldFilter(fk)}
                      />
                      <span className="mono">{fk}</span>
                      <span className="item-count">
                        {fieldKeyCounts[fk] || 0}
                      </span>
                    </label>
                  ))}
                </div>
              )}
            </div>
          )}
        </aside>

        {/* Center Content View */}
        <div className="explorer-center-area">
          {/* Timeline Histogram Chart */}
          <div className="telemetry-histogram-container">
            <div className="histogram-bars">
              {histogramBuckets.map((b) => (
                <div
                  key={b.index}
                  className="histogram-bar-col"
                  title={`Time: ${new Date(b.startTime).toLocaleTimeString()} - ${new Date(b.endTime).toLocaleTimeString()}\nNodes: ${b.count}`}
                >
                  <div
                    className="histogram-segment"
                    style={{
                      height: `${b.heightPct}%`,
                    }}
                  />
                </div>
              ))}
            </div>
            <div className="histogram-dates">
              <span>Past Timeline</span>
              <span>Distribution</span>
              <span>Recent</span>
            </div>
          </div>

          {/* TanStack Table Card */}
          <div className="telemetry-table-card">
            <div className="telemetry-table-wrap">
              <div role="table" className="telemetry-table">
                <div role="rowgroup" className="telemetry-thead">
                  {table.getHeaderGroups().map((headerGroup) => (
                    <div
                      role="row"
                      key={headerGroup.id}
                      style={{ display: "flex", width: "100%" }}
                    >
                      {headerGroup.headers.map((header) => (
                        <div
                          role="columnheader"
                          key={header.id}
                          style={{ flex: 1, minWidth: 100 }}
                          className={`telemetry-th ${header.column.getCanSort() ? "sortable" : ""}`}
                        >
                          {header.isPlaceholder
                            ? null
                            : flexRender(
                                header.column.columnDef.header,
                                header.getContext(),
                              )}
                        </div>
                      ))}
                    </div>
                  ))}
                </div>
                <div role="rowgroup" className="telemetry-tbody">
                  {liveMode && (
                    <div className="live-mode-banner-row">
                      🔵 Live Mode — Streaming incoming telemetry nodes
                    </div>
                  )}
                  {table.getRowModel().rows.map((row) => (
                    <div
                      role="row"
                      key={row.id}
                      className={`card telemetry-tr ${selectedId === row.original.id ? "selected" : ""}`}
                      onClick={() => setSelectedId(row.original.id)}
                    >
                      {row.getVisibleCells().map((cell) => (
                        <div
                          role="cell"
                          key={cell.id}
                          style={{ flex: 1, minWidth: 100 }}
                          className="telemetry-td"
                        >
                          {flexRender(
                            cell.column.columnDef.cell,
                            cell.getContext(),
                          )}
                        </div>
                      ))}
                    </div>
                  ))}
                  {table.getRowModel().rows.length === 0 && (
                    <div
                      style={{ textAlign: "center", padding: 24 }}
                      className="text-muted"
                    >
                      No nodes found matching current filters.
                    </div>
                  )}
                </div>
              </div>
            </div>

            {/* Pagination Controls */}
            <div className="table-pagination-bar">
              <div className="page-select-wrap">
                <span>Rows per page:</span>
                <select
                  value={table.getState().pagination.pageSize}
                  onChange={(e) => table.setPageSize(Number(e.target.value))}
                >
                  {[10, 15, 25, 50, 100].map((pageSize) => (
                    <option key={pageSize} value={pageSize}>
                      {pageSize}
                    </option>
                  ))}
                </select>
                <span style={{ marginLeft: 12 }}>
                  Showing{" "}
                  {table.getState().pagination.pageIndex *
                    table.getState().pagination.pageSize +
                    1}{" "}
                  -{" "}
                  {Math.min(
                    (table.getState().pagination.pageIndex + 1) *
                      table.getState().pagination.pageSize,
                    filteredData.length,
                  )}{" "}
                  of {filteredData.length} rows
                </span>
              </div>

              <div className="pagination-controls">
                <button
                  className="explorer-btn pagination-btn"
                  onClick={() => table.setPageIndex(0)}
                  disabled={!table.getCanPreviousPage()}
                >
                  «
                </button>
                <button
                  className="explorer-btn pagination-btn"
                  onClick={() => table.previousPage()}
                  disabled={!table.getCanPreviousPage()}
                >
                  ‹
                </button>
                <span
                  className="mono"
                  style={{ fontSize: 12, margin: "0 8px" }}
                >
                  Page {table.getState().pagination.pageIndex + 1} of{" "}
                  {Math.max(1, table.getPageCount())}
                </span>
                <button
                  className="explorer-btn pagination-btn"
                  onClick={() => table.nextPage()}
                  disabled={!table.getCanNextPage()}
                >
                  ›
                </button>
                <button
                  className="explorer-btn pagination-btn"
                  onClick={() => table.setPageIndex(table.getPageCount() - 1)}
                  disabled={!table.getCanNextPage()}
                >
                  »
                </button>
              </div>
            </div>
          </div>
        </div>

        {/* Right-Side Node Detail Panel (Pulled up on the right, side-by-side) */}
        {selected && (
          <>
            <div
              className="panel-backdrop"
              onClick={() => setSelectedId(null)}
            />
            <div className="explorer-right-panel">
              <NodeDetail
                node={selected}
                onClose={() => setSelectedId(null)}
                onUpdate={() => {
                  queryClient.invalidateQueries({
                    queryKey: ["node", selectedId],
                  });
                  queryClient.invalidateQueries({ queryKey: ["nodes"] });
                }}
              />
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function NodeDetail({
  node,
  onClose,
  onUpdate,
}: {
  node: Node;
  onClose: () => void;
  onUpdate: () => void;
}) {
  const [editingField, setEditingField] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");

  const updateMut = useMutation({
    mutationFn: ({ id, fields }: { id: string; fields: Record<string, any> }) =>
      updateNode(id, fields),
    onSuccess: onUpdate,
  });

  const startEdit = (key: string, value: any) => {
    setEditingField(key);
    setEditValue(typeof value === "string" ? value : JSON.stringify(value));
  };

  const saveField = () => {
    if (!editingField) return;
    try {
      const parsed = JSON.parse(editValue);
      updateMut.mutate({ id: node.id, fields: { [editingField]: parsed } });
    } catch {
      updateMut.mutate({
        id: node.id,
        fields: { [editingField]: { type: "String", value: editValue } },
      });
    }
    setEditingField(null);
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      <div className="panel-header">
        <div>
          <h3>Node: {node.id}</h3>
          <p className="text-muted" style={{ fontSize: 12, marginTop: 4 }}>
            Space: {node.space_id}
          </p>
        </div>
        <button onClick={onClose} className="explorer-btn">
          Close
        </button>
      </div>

      <div className="text-muted" style={{ fontSize: 12 }}>
        Created: {new Date(node.created_at).toLocaleString()}
        <br />
        Updated: {new Date(node.updated_at).toLocaleString()}
      </div>

      <h4 style={{ marginTop: 8, fontSize: 14, fontWeight: 700 }}>Fields</h4>
      <div className="table-wrap">
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr>
              <th
                style={{ textAlign: "left", padding: "6px 8px", fontSize: 12 }}
              >
                Key
              </th>
              <th
                style={{ textAlign: "left", padding: "6px 8px", fontSize: 12 }}
              >
                Type
              </th>
              <th
                style={{ textAlign: "left", padding: "6px 8px", fontSize: 12 }}
              >
                Value
              </th>
              <th
                style={{ textAlign: "left", padding: "6px 8px", fontSize: 12 }}
              >
                Actions
              </th>
            </tr>
          </thead>
          <tbody>
            {Object.entries(node.fields).map(([key, field]) => (
              <tr
                key={key}
                style={{ borderTop: "1px solid var(--telemetry-border)" }}
              >
                <td
                  data-label="Key"
                  style={{ padding: "6px 8px" }}
                  className="mono"
                >
                  {key}
                </td>
                <td
                  data-label="Type"
                  style={{ padding: "6px 8px" }}
                  className="mono"
                >
                  {field.type}
                </td>
                <td data-label="Value" style={{ padding: "6px 8px" }}>
                  {editingField === key ? (
                    <input
                      value={editValue}
                      onChange={(e) => setEditValue(e.target.value)}
                      onKeyDown={(e) => e.key === "Enter" && saveField()}
                      style={{ width: "100%" }}
                    />
                  ) : (
                    <span style={{ wordBreak: "break-all" }} className="mono">
                      {typeof field.value === "object"
                        ? JSON.stringify(field.value)
                        : String(field.value ?? "")}
                    </span>
                  )}
                </td>
                <td data-label="Actions" style={{ padding: "6px 8px" }}>
                  {editingField === key ? (
                    <button
                      className="explorer-btn primary"
                      onClick={saveField}
                    >
                      Save
                    </button>
                  ) : (
                    <button
                      className="explorer-btn"
                      onClick={() => startEdit(key, field.value)}
                    >
                      Edit
                    </button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function CreateNodeForm({ onCreated }: { onCreated: () => void }) {
  const [title, setTitle] = useState("");

  const createMut = useMutation({
    mutationFn: () =>
      createNode({
        "system:node_title": { type: "String", value: title },
        "system:node_time": {
          type: "DateTime",
          value: new Date().toISOString(),
        },
      }),
    onSuccess: onCreated,
  });

  return (
    <div
      className="telemetry-create-panel mt-1"
      style={{
        background: "var(--telemetry-panel)",
        border: "1px solid var(--telemetry-border)",
        borderRadius: 8,
        padding: 16,
      }}
    >
      <div className="flex-col">
        <input
          placeholder="Node title"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
        />
        <button
          className="explorer-btn primary"
          onClick={() => createMut.mutate()}
        >
          Create
        </button>
      </div>
    </div>
  );
}
