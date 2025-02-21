import "@mdxeditor/editor/style.css";
import { format } from "date-fns";
import { useEffect } from "react";
import { useState } from "react";
import JournalPage from "./JournalPage";
import { ArrowRightIcon } from "lucide-react";
import AllJournalPages from "./AllJournalPages";

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

        <div className="flex justify-center text-slate-400">━━━━❰❉❱━━━━</div>

        <AllJournalPages />
      </div>
      <div className="flex flex-col min-w-3">Sidebar</div>
    </div>
  );
}
