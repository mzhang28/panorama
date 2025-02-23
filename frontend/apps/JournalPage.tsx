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
import GenericNode from "@/components/GenericNode";

export interface JournalPageProps {
  // YYYY-MM-DD
  date: string;
}

export default function JournalPage({ date }: JournalPageProps) {
  const fetchPage = useCallback(async () => {
    const res = await fetch(`/api/apps/journal/by_date/${date}`);
    const data: GetJournalResponse = await res.json();
    return data;
  }, [date]);

  const { data: data } = useQuery({
    queryKey: ["journal/page", date],
    queryFn: fetchPage,
  });

  if (!data) return <>...</>;

  return (
    <GenericNode id={data.node_id} displayStyle="journalPage" />
    // <div className="border-1 flex flex-col bg-white shadow-md">
    //   <small className="border-b-1 p-1">Journal Page for {date}</small>

    // </div>
  );
}
