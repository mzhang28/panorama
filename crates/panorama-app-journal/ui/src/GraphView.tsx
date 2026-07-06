import { useQuery } from "@tanstack/react-query";
import { useCallback, useMemo } from "react";
import ForceGraph2D from "react-force-graph-2d";
import { api, type BlockNode, normalizeBlock } from "./api"; // I will extract API helpers to api.ts

export function GraphView({
  onNodeClick,
}: {
  onNodeClick: (id: string) => void;
}) {
  const { data: blocks = [] } = useQuery({
    queryKey: ["journal-blocks-all"],
    queryFn: async () => {
      const data = await api("blocks");
      return (Array.isArray(data) ? data : (data?.rows ?? [])).map(
        normalizeBlock,
      );
    },
  });

  const graphData = useMemo(() => {
    const nodes: any[] = [];
    const links: any[] = [];
    const nodeMap = new Set();

    blocks.forEach((b: BlockNode) => {
      const block = normalizeBlock(b as any);
      if (!block.id) return;
      nodeMap.add(block.id);

      const title = block.fields?.["system:node_title"]?.value || "";
      const content = block.fields?.["journal:content"]?.value || "";
      const isPage = !block.fields?.["journal:parent_id"]?.value;

      nodes.push({
        id: block.id,
        name: isPage ? title || "Untitled Page" : content.slice(0, 30),
        val: isPage ? 5 : 2, // Larger node for pages
        color: isPage ? "var(--accent)" : "var(--text-muted)",
      });

      // Child link
      const parentId = block.fields?.["journal:parent_id"]?.value;
      if (parentId) {
        links.push({
          source: block.id,
          target: parentId,
          type: "child",
        });
      }

      // Ref link
      const refs = block.fields?.["journal:refs"]?.value || [];
      if (Array.isArray(refs)) {
        refs.forEach((refId) => {
          links.push({
            source: block.id,
            target: refId,
            type: "ref",
          });
        });
      }
    });

    // Ensure all targets exist in nodes
    const validLinks = links.filter(
      (l) => nodeMap.has(l.source) && nodeMap.has(l.target),
    );

    return { nodes, links: validLinks };
  }, [blocks]);

  const handleNodeClick = useCallback(
    (node: any) => {
      onNodeClick(node.id);
    },
    [onNodeClick],
  );

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        background: "var(--bg-card)",
        borderRadius: "var(--radius-md)",
        overflow: "hidden",
        border: "1px solid var(--border)",
      }}
    >
      <ForceGraph2D
        graphData={graphData}
        nodeLabel="name"
        nodeColor="color"
        nodeVal="val"
        linkColor={(link) =>
          link.type === "child" ? "var(--border)" : "var(--accent)"
        }
        linkDirectionalArrowLength={(link) => (link.type === "ref" ? 3 : 0)}
        onNodeClick={handleNodeClick}
        width={800}
        height={600}
        backgroundColor="transparent"
      />
    </div>
  );
}
