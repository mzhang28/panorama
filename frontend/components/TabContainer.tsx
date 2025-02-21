import { closeTabAtom, switchTabAtom, tabStateAtom } from "@/lib/tabs";
import { useAtom, useAtomValue, useSetAtom } from "jotai";
import { useEffect, useRef, useState } from "react";
import Calendar from "@/apps/Calendar";
import Home from "@/apps/Home";
import Automate from "@/apps/Automate";
import { cn } from "@/lib/utils";
import { Plus, X } from "lucide-react";
import { Menu, MenuButton, MenuItem, MenuItems } from "@headlessui/react";
import { useCallback } from "react";

/** Contains all the tabs on the main page */
export default function TabContainer() {
  const { current: currentTab, tabs } = useAtomValue(tabStateAtom);
  const closeTab = useSetAtom(closeTabAtom);
  const switchTab = useSetAtom(switchTabAtom);

  return (
    <>
      <div className="flex border-b-1">
        <NewMenu />
        <input placeholder="Search panorama..." className="p-2 outline-none" />
        {tabs.map((tab, idx) => (
          <button
            key={tab.id}
            className={cn(
              "flex items-center gap-2 h-10 rounded-t-md pl-4 py-2",
              idx > 0 ? "pr-3" : "pr-4",
              idx === currentTab
                ? "bg-slate-900 text-slate-50"
                : "hover:bg-gray-100",
            )}
            onClick={() => switchTab(idx)}
          >
            {tab.title}
            {idx > 0 && (
              <button
                onClick={() => closeTab(idx)}
                className={cn(
                  "rounded-lg",
                  idx === currentTab
                    ? "hover:bg-red-600"
                    : "hover:bg-slate-200",
                )}
              >
                <X className="w-3 h-3" />
              </button>
            )}
          </button>
        ))}
      </div>

      <div className="bg-slate-100 grow">
        <TabContent />
      </div>
    </>
  );
}

function TabContent({}) {
  const { current, tabs } = useAtomValue(tabStateAtom);
  console.log("tabs", tabs, "current", current);
  const currentTab = tabs[current];
  if (!currentTab) return <>...</>;

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
  const items = [{ name: "Note" }, { name: "Bookmark" }, { name: "Query" }];

  const setTabs = useSetAtom(tabStateAtom);

  const createNew = useCallback((name: string) => {
    console.log(name);
    setTabs(({ current, tabs }) => ({
      current: tabs.length,
      tabs: [...tabs, { id: "Shiet", page: "Shiet", title: "Shiet" }],
    }));
  }, []);

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
              <button
                onClick={(_) => createNew(name)}
                className="flex px-4 py-2 no-underline text-gray-600 hover:bg-gray-200"
              >
                {name}
              </button>
            </MenuItem>
          ))}
        </div>
      </MenuItems>
    </Menu>
  );
}
