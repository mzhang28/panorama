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
import CreateNode from "@/components/CreateNode";
import RecentNodes from "@/components/RecentNodes";

export default function Home() {
  const todaysDate = format(new Date(), "yyyy-MM-dd");

  return (
    <div className="flex p-3 gap-3">
      <div className="flex flex-col grow gap-3">
        <CreateNode />

        <div className="flex justify-center text-slate-400 text-2xl select-none">
          ❅────────❅•°•°•❅────────❅
        </div>

        <h2>Daily Journal</h2>
        <AllJournalPages />
      </div>
      <div className="flex flex-col grow min-w-60">
        <h3>Recent Nodes</h3>
        <RecentNodes />
      </div>
    </div>
  );
}
