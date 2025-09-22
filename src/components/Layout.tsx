import React from "react";
import { useSidebar } from "../hooks/useSidebar";
import { useProblems } from "../hooks/useProblems";
import { LeftSidebar } from "./LeftSidebar";
import { RightSidebar } from "./RightSidebar";
import { MainContent } from "./MainContent";
import { Header } from "./Header";
import { StatusBar } from "./StatusBar";

export function Layout() {
  const { sidebarState, toggleLeftSidebar, toggleRightSidebar } = useSidebar();
  const { problems } = useProblems();

  return (
    <div className="flex flex-col h-screen bg-gray-50 overflow-hidden">
      {/* Main Layout Area */}
      <div className="flex flex-1 overflow-hidden">
        {/* Left Sidebar */}
        <div
          className={`${
            sidebarState.leftSidebar ? "w-64" : "w-0"
          } transition-all duration-300 ease-in-out border-r border-gray-200 bg-white overflow-hidden`}
        >
          {sidebarState.leftSidebar && <LeftSidebar />}
        </div>

        {/* Main Content Area */}
        <div className="flex-1 flex flex-col min-w-0">
          <Header
            leftSidebarOpen={sidebarState.leftSidebar}
            rightSidebarOpen={sidebarState.rightSidebar}
            toggleLeftSidebar={toggleLeftSidebar}
            toggleRightSidebar={toggleRightSidebar}
          />
          <div className="flex-1 overflow-hidden">
            <MainContent />
          </div>
        </div>

        {/* Right Sidebar */}
        <div
          className={`${
            sidebarState.rightSidebar ? "w-80" : "w-0"
          } transition-all duration-300 ease-in-out border-l border-gray-200 bg-white overflow-hidden`}
        >
          {sidebarState.rightSidebar && <RightSidebar />}
        </div>
      </div>

      {/* Status Bar - spans full width at bottom */}
      <StatusBar problems={problems} />
    </div>
  );
}
