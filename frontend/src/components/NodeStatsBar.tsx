import { useMemo } from "react";
import { Node } from "../api/client";

interface NodeStatsBarProps {
  nodes: Node[];
}

export function NodeStatsBar({ nodes }: NodeStatsBarProps) {
  const stats = useMemo(() => {
    const total = nodes.length;
    const uniqueSchemas = new Set(
      nodes.flatMap((n) => n.preferred_schemas.map((s) => s.schema_node_id)),
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
