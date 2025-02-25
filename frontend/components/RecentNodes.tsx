import { useCallback } from "react";
import type { RecentNodesResponse } from "../../backend/bindings/RecentNodesResponse";
import { useQuery } from "@tanstack/react-query";
import GenericNode from "./GenericNode";

export default function RecentNodes() {
  const fetchRecent = useCallback(async () => {
    const res = await fetch("/api/node/recent");
    const data: RecentNodesResponse = await res.json();
    return data.nodes;
  }, []);

  const { data: nodes } = useQuery({
    queryKey: ["recent"],
    queryFn: fetchRecent,
  });

  if (!nodes) return <>...</>;

  return (
    <div className="flex flex-col gap-3">
      {nodes
        .toSorted((a, b) => b.last_updated_at.localeCompare(a.last_updated_at))
        .map((node) => (
          <GenericNode key={node.id} id={node.id} />
        ))}
    </div>
  );
}
