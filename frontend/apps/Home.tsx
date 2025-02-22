import "@mdxeditor/editor/style.css";
import { format } from "date-fns";
import { useEffect } from "react";
import { useState } from "react";
import JournalPage from "./JournalPage";
import { ArrowRightIcon } from "lucide-react";
import AllJournalPages from "./AllJournalPages";
import { useCallback } from "react";
import type { RecentNodesResponse } from "../../backend/bindings/RecentNodesResponse";
import { useQuery } from "@tanstack/react-query";
import GenericNode from "@/components/GenericNode";

export default function Home() {
  const todaysDate = format(new Date(), "yyyy-MM-dd");

  return (
    <div className="flex p-3 gap-3">
      <div className="flex flex-col grow gap-3">
        <div className="flex flex-col gap-2">
          <textarea
            className="border-1 outline-none p-2 bg-slate-50"
            placeholder="What's new?"
          />
          <div className="flex justify-end">
            <button className="flex bg-slate-600 text-white px-3 py-2">
              Save to panorama <ArrowRightIcon />
            </button>
          </div>
        </div>

        <div className="flex justify-center text-slate-400 text-2xl select-none">
          ❅────────❅•°•°•❅────────❅
        </div>

        <AllJournalPages />
      </div>
      <div className="flex flex-col min-w-60 gap-3">
        <h3>Recent Nodes</h3>
        <RecentNodes />
      </div>
    </div>
  );
}

function RecentNodes() {
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

  return nodes.map((node) => <GenericNode id={node.id} />);
}
