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
          schema_node_id:
            i % 3 === 0
              ? "schema-blog"
              : i % 3 === 1
                ? "schema-user"
                : "schema-page",
          version: { major: 1, minor: 0 },
        },
      ],
      space_id: i % 2 === 0 ? "space-personal" : "space-work",
      created_at: new Date(Date.now() - i * 7200000).toISOString(),
      updated_at: new Date(Date.now() - i * 600000).toISOString(),
    }),
  );
}
