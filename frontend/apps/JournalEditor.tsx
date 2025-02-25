import { fetchNode } from "@/lib/node";
import {
  headingsPlugin,
  linkPlugin,
  listsPlugin,
  markdownShortcutPlugin,
  MDXEditor,
  quotePlugin,
  thematicBreakPlugin,
  type MDXEditorMethods,
} from "@mdxeditor/editor";
import { useQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useRef, useState } from "react";

export interface JournalEditorProps {
  id: string;
  date?: string;
}

export default function JournalEditor({ id, date }: JournalEditorProps) {
  const mdxEditorRef = useRef<MDXEditorMethods | null>(null);
  const [lastServerPage, setLastServerPage] = useState<string | null>(null);
  const [localPage, setLocalPage] = useState("");

  const { data: nodeJson, status } = useQuery({
    queryKey: ["node", id, "fetch"],
    queryFn: () => fetchNode(id),
  });

  const savePage = useCallback(
    async (value: string) => {
      if (date) {
        await fetch(`/api/apps/journal/by_date/${date}`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ content: value }),
        });
      } else {
        await fetch(`/api/node/${id}`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ content: value }),
        });
      }
    },
    [date],
  );

  useEffect(() => {
    if (!nodeJson) return;
    const { content } = nodeJson;

    // This is the first load from the server, so let's load it in
    if (lastServerPage === null && content !== null && content !== undefined) {
      setLastServerPage(content);
      // setLocalPage(content);
      mdxEditorRef.current?.setMarkdown(content);
    }

    // DON'T DO ANYTHING IF THE LOCAL PAGE HAS BEEN TOUCHED!
    // TODO: Implement CRDT
    if (lastServerPage === localPage) {
      setLastServerPage(content);
      // setLocalPage(content);
      mdxEditorRef.current?.setMarkdown(content);
    }
  }, [nodeJson, lastServerPage, localPage]);

  const updateLocalPage = useCallback((value: string) => {
    // setLocalPage(value);
    mdxEditorRef.current?.setMarkdown(value);
    savePage(value);
  }, []);

  if (!nodeJson) return <>...</>;

  console.log("localPage", localPage);

  return (
    <MDXEditor
      markdown=""
      ref={mdxEditorRef}
      placeholder="What are you thinking about?"
      onChange={(value) => updateLocalPage(value)}
      className="journalMdxEditor"
      plugins={[
        headingsPlugin(),
        listsPlugin(),
        quotePlugin(),
        thematicBreakPlugin(),
        markdownShortcutPlugin(),
        linkPlugin(),
      ]}
    />
  );
}
