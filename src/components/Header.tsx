import React from "react";
import { Menu, PanelLeftClose, PanelRightClose, Search } from "lucide-react";

interface HeaderProps {
  leftSidebarOpen: boolean;
  rightSidebarOpen: boolean;
  toggleLeftSidebar: () => void;
  toggleRightSidebar: () => void;
}

export function Header({
  leftSidebarOpen,
  rightSidebarOpen,
  toggleLeftSidebar,
  toggleRightSidebar,
}: HeaderProps) {
  return (
    <header className="h-12 bg-white border-b border-gray-200 flex items-center px-4 justify-between">
      {/* Left side - Sidebar toggle and title */}
      <div className="flex items-center gap-3">
        <button
          onClick={toggleLeftSidebar}
          className="p-1.5 rounded-md hover:bg-gray-100 transition-colors"
          aria-label={leftSidebarOpen ? "Close sidebar" : "Open sidebar"}
        >
          <Menu size={18} className="text-gray-600" />
        </button>

        <div className="flex items-center gap-2">
          <div className="w-6 h-6 bg-blue-600 rounded flex items-center justify-center">
            <div className="w-3 h-3 bg-white rounded-sm"></div>
          </div>
          <h1 className="text-lg font-semibold text-gray-900">Panorama</h1>
        </div>
      </div>

      {/* Center - Search bar */}
      <div className="flex-1 max-w-md mx-8">
        <div className="relative">
          <Search
            size={16}
            className="absolute left-3 top-1/2 transform -translate-y-1/2 text-gray-400"
          />
          <input
            type="text"
            placeholder="Search pages, blocks, or create new..."
            className="w-full pl-10 pr-4 py-1.5 bg-gray-50 border border-gray-200 rounded-md text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
          />
        </div>
      </div>

      {/* Right side - Right sidebar toggle */}
      <div className="flex items-center">
        <button
          onClick={toggleRightSidebar}
          className="p-1.5 rounded-md hover:bg-gray-100 transition-colors"
          aria-label={
            rightSidebarOpen ? "Close right panel" : "Open right panel"
          }
        >
          <PanelRightClose
            size={18}
            className={`text-gray-600 transition-transform ${rightSidebarOpen ? "" : "rotate-180"}`}
          />
        </button>
      </div>
    </header>
  );
}
