import { atom } from "jotai";

export const currentTabAtom = atom(0);

export const tabsAtom = atom([
  // TODO: have customizable home behavior
  { id: "home", page: "home", title: "Home" },
  { id: "calendar", page: "calendar", title: "Calendar" },
  { id: "automate", page: "automate", title: "Automate" },
]);
