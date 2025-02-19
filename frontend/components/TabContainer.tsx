import { currentTabAtom, tabsAtom } from "@/lib/tabs";
import { useAtom, useAtomValue } from "jotai";
import { useEffect, useRef, useState } from "react";
import Calendar from "@/apps/Calendar";
import Home from "@/apps/Home";
import Automate from "@/apps/Automate";
import { cn } from "@/lib/utils";

/** Contains all the tabs on the main page */
export default function TabContainer() {
  const tabs = useAtomValue(tabsAtom);
  const [currentTab, setCurrentTab] = useAtom(currentTabAtom);

  return (
    <>
      <div className="flex border-b-1">
        <input placeholder="Search panorama..." className="p-2 outline-none" />
        {tabs.map((tab, idx) => (
          <button
            key={tab.id}
            className={cn(
              "h-10 rounded-t-md px-3 py-2",
              idx === currentTab
                ? "bg-slate-900 text-slate-50"
                : "hover:bg-gray-100",
            )}
            onClick={() => setCurrentTab(idx)}
          >
            {tab.title}
            {/* {idx === currentTab && "*"} */}
          </button>
        ))}
      </div>
      {/* <Tabs
        value={currentTab.toString()}
        onValueChange={(v) => setCurrentTab(parseInt(v))}
        className="w-full"
      >
        <TabsList>
          <Input placeholder="Search panorama..." />

        </TabsList>
      </Tabs> */}
      <TabContent />
    </>
  );
}

function TabContent({}) {
  const tabs = useAtomValue(tabsAtom);
  const currentTabIdx = useAtomValue(currentTabAtom);
  const currentTab = tabs[currentTabIdx];
  console.log("current tab", currentTabIdx, currentTab);

  switch (currentTab.page) {
    case "home":
      return <Home />;
    case "calendar":
      return <Calendar />;
    case "automate":
      return <Automate />;
    default:
      return <>how did u find this</>;
  }
}
