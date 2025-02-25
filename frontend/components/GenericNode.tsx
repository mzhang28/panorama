import { useQuery } from "@tanstack/react-query";
import { useCallback, useRef } from "react";
import { Copy } from "lucide-react";
import CopyButton from "./CopyButton";
import JournalEditor from "@/apps/JournalEditor";
import { fetchNode } from "@/lib/node";
import TagEditor from "./TagEditor";

export interface GenericNodeProps {
  abbreviated?: boolean; // false by default
  id: string;
  displayStyle?: string;
}

export default function GenericNode({
  abbreviated,
  id,
  displayStyle,
}: GenericNodeProps) {
  const { data: nodeJson, status } = useQuery({
    queryKey: ["node", id, "fetch"],
    queryFn: () => fetchNode(id),
  });

  if (!nodeJson) return <>{status}</>;

  const computedDisplayStyle = displayStyle ?? guessedDisplayStyle(nodeJson);

  return (
    <div className="border-1 shadow-md bg-white flex flex-col">
      <div className="text-sm border-b-1 flex">
        <div className="p-1">
          {nodeJson.title}
          <CopyButton contents={id} />
          <br />
          <small>{nodeJson.last_updated_at}</small>
        </div>
        <div className="grow" />
        <div>
          <TagEditor id={id} />
        </div>
      </div>

      <NodeDisplay id={id} displayStyle={computedDisplayStyle} />
    </div>
  );
}

export function NodeDisplay({ id, displayStyle }) {
  const { data: nodeJson, status } = useQuery({
    queryKey: ["node", id, "fetch"],
    queryFn: () => fetchNode(id),
  });

  if (!nodeJson) return <>{status}</>;

  switch (displayStyle) {
    case "journalPage":
      return <JournalEditor id={id} date={nodeJson.journal_date} />;
    case "contentPage":
      return <JournalEditor id={id} date={nodeJson.journal_date} />;

    default:
      return (
        <div className="p-2">
          <details>
            <summary>JSON</summary>
            <pre>{JSON.stringify(nodeJson, null, 2)}</pre>
          </details>
        </div>
      );
  }
}

function guessedDisplayStyle(nodeJson) {
  if (nodeJson.journal_date) return "journalPage";
  else if (nodeJson.content) return "contentPage";
  return "";
}
