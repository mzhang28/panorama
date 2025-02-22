import { useQuery } from "@tanstack/react-query";
import { useCallback, useRef } from "react";
import { Copy } from "lucide-react";
import CopyButton from "./CopyButton";

export interface GenericNodeProps {
  abbreviated?: boolean; // false by default
  id: string;
}

export default function GenericNode({ abbreviated, id }: GenericNodeProps) {
  const fetchNode = useCallback(async () => {
    const res = await fetch(`/api/node/${id}`);
    return await res.json();
  }, [id]);

  const { data: nodeJson, status } = useQuery({
    queryKey: ["node", id, "fetch"],
    queryFn: fetchNode,
  });

  if (!nodeJson) return <>{status}</>;

  return (
    <div className="border-1 shadow-md p-2 bg-white flex flex-col">
      <div className="text-sm">
        {nodeJson.title}
        <CopyButton contents={id} />
        <details>
          <summary>JSON</summary>
          <pre>{JSON.stringify(nodeJson, null, 2)}</pre>
        </details>
      </div>
      <div></div>
    </div>
  );
}
