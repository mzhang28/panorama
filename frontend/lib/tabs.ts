import { atom } from "jotai";

export const tabStateAtom = atom({
  current: 0,
  tabs: [
    // TODO: have customizable home behavior
    { id: "home", page: "home", title: "Home" },
    { id: "calendar", page: "calendar", title: "Calendar" },
    { id: "automate", page: "automate", title: "Automate" },
  ],
});

export const switchTabAtom = atom(null, (get, set, idx: number) => {
  const { current, tabs } = get(tabStateAtom);
  set(tabStateAtom, { current: idx, tabs });
});

export const closeTabAtom = atom(null, (get, set, idx: number) => {
  const { current, tabs } = get(tabStateAtom);
  const newCurrent = current >= idx ? current - 1 : current;
  const newTabs = [...tabs.slice(0, idx), ...tabs.slice(idx + 1)];
  console.log("FUCKING NEXT", newCurrent, newTabs);
  set(tabStateAtom, {
    current: newCurrent,
    tabs: newTabs,
  });
});
