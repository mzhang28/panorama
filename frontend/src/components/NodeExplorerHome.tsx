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
