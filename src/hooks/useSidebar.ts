import { useState } from "react";

export interface SidebarState {
  leftSidebar: boolean;
  rightSidebar: boolean;
}

export function useSidebar(
  initialState: SidebarState = { leftSidebar: true, rightSidebar: true },
) {
  const [sidebarState, setSidebarState] = useState<SidebarState>(initialState);

  const toggleLeftSidebar = () => {
    setSidebarState((prev) => ({
      ...prev,
      leftSidebar: !prev.leftSidebar,
    }));
  };

  const toggleRightSidebar = () => {
    setSidebarState((prev) => ({
      ...prev,
      rightSidebar: !prev.rightSidebar,
    }));
  };

  const setLeftSidebar = (open: boolean) => {
    setSidebarState((prev) => ({
      ...prev,
      leftSidebar: open,
    }));
  };

  const setRightSidebar = (open: boolean) => {
    setSidebarState((prev) => ({
      ...prev,
      rightSidebar: open,
    }));
  };

  const closeBothSidebars = () => {
    setSidebarState({
      leftSidebar: false,
      rightSidebar: false,
    });
  };

  const openBothSidebars = () => {
    setSidebarState({
      leftSidebar: true,
      rightSidebar: true,
    });
  };

  return {
    sidebarState,
    toggleLeftSidebar,
    toggleRightSidebar,
    setLeftSidebar,
    setRightSidebar,
    closeBothSidebars,
    openBothSidebars,
  };
}
