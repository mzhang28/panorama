import { currentTabAtom, tabsAtom } from "@/lib/tabs";
import { useAtom, useAtomValue } from "jotai";
import { useEffect, useRef, useState } from "react";
import Calendar from "@/apps/Calendar";
import Home from "@/apps/Home";
import Automate from "@/apps/Automate";
import { cn } from "@/lib/utils";
import { Plus } from "lucide-react";
import { Menu, MenuButton, MenuItem, MenuItems } from "@headlessui/react";

/** Contains all the tabs on the main page */
export default function TabContainer() {
  const tabs = useAtomValue(tabsAtom);
  const [currentTab, setCurrentTab] = useAtom(currentTabAtom);

  return (
    <>
      <div className="flex border-b-1">
        <NewMenu />
        {/* <button className="p-2">
          <Plus />
        </button> */}
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

function NewMenu() {
  const items = [{ name: "Note" }, { name: "Bookmark" }];

  return (
    <Menu>
      <MenuButton className="p-2">
        <Plus />
      </MenuButton>
      <MenuItems
        anchor="bottom"
        className="absolute flex ring-1 mt-2 w-56 origin-top-left bg-white border-1 shadow-lg ring-black/5 focus:outline-hidden"
      >
        <div className="py-1 w-full flex flex-col items-stretch">
          {items.map(({ name }) => (
            <MenuItem>
              <button className="flex px-4 py-2 no-underline text-gray-600 hover:bg-gray-200">
                {name}
              </button>
            </MenuItem>
          ))}
        </div>
      </MenuItems>
    </Menu>
  );
}
