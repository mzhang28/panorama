import {
  type ColumnDef,
  flexRender,
  getCoreRowModel,
  getFilteredRowModel,
  getPaginationRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { useMemo, useState } from "react";
import type { Node } from "../api/client";

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

  const schemas =
    node.preferred_schemas.map((s) => s.schema_node_id).join(", ") || "—";

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
        <p className="text-[var(--text-dim)] text-sm text-center py-[var(--space-8)]">
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
              <tr
                key={headerGroup.id}
                className="border-b border-[var(--border)]"
              >
                {headerGroup.headers.map((header) => (
                  <th
                    key={header.id}
                    className="text-left px-[var(--space-4)] py-[var(--space-3)] text-[var(--text-muted)] text-xs font-semibold uppercase tracking-wide bg-[var(--surface-elevated)]"
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
            {table.getState().pagination.pageIndex + 1} / {table.getPageCount()}
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
