import {
  headingsPlugin,
  listsPlugin,
  markdownShortcutPlugin,
  MDXEditor,
  type MDXEditorMethods,
  quotePlugin,
  thematicBreakPlugin,
  linkPlugin,
} from "@mdxeditor/editor";
import { useCallback } from "react";
import { useState } from "react";
import type { GetJournalResponse } from "../../backend/bindings/GetJournalResponse";
import { useQuery } from "@tanstack/react-query";
import { useEffect } from "react";
import { useRef } from "react";

export interface JournalPageProps {
  // YYYY-MM-DD
  date: string;
}

export default function JournalPage({ date }: JournalPageProps) {
  const mdxEditorRef = useRef<MDXEditorMethods | null>(null);
  const [lastServerPage, setLastServerPage] = useState<string | null>(null);
  const [localPage, setLocalPage] = useState("");

  const fetchPage = useCallback(async () => {
    const res = await fetch(`/api/apps/journal/by_date/${date}`);
    const data: GetJournalResponse = await res.json();
    console.log("data", data);
    return data;
  }, [date]);

  const savePage = useCallback(
    async (value: string) => {
      await fetch(`/api/apps/journal/by_date/${date}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ content: value }),
      });
    },
    [date]
  );

  const { data: data } = useQuery({
    queryKey: ["journal/page", date],
    queryFn: fetchPage,
  });

  useEffect(() => {
    if (!data) return;
    const { content } = data;

    // This is the first load from the server, so let's load it in
    if (lastServerPage === null) {
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
  }, [data, lastServerPage, localPage]);

  const updateLocalPage = useCallback((value: string) => {
    // setLocalPage(value);
    mdxEditorRef.current?.setMarkdown(value);
    savePage(value);
  }, []);

  if (!data) return <>...</>;

  console.log("localPage", localPage);

  return (
    <div className="border-1 flex flex-col bg-white shadow-md">
      <small className="border-b-1 p-1">Journal Page for {date}</small>

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
    </div>
  );
}
