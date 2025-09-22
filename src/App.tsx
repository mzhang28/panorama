import React from "react";
import { Routes, Route, Navigate, useLocation } from "react-router-dom";
import { LeftSidebar } from "./components/LeftSidebar";
import { StatusBar } from "./components/StatusBar";
import { Header } from "./components/Header";
import { MainContent } from "./components/MainContent";
import { RightSidebar } from "./components/RightSidebar";
import { useSidebar } from "./hooks/useSidebar";
import { useProblems } from "./hooks/useProblems";
import { ProblemsDemo } from "./components/ProblemsDemo";
import { TRPCDemo } from "./components/TRPCDemo";
import { SQLiteDemo } from "./components/SQLiteDemo";

const JournalRedirect = () => {
  const today = new Date().toISOString().slice(0, 10);
  return <Navigate to={`/journal/${today}`} replace />;
};

const AppRoutes = () => (
  <Routes>
    <Route path="/" element={<Navigate to="/journal" replace />} />
    <Route path="/journal" element={<JournalRedirect />} />
    <Route path="/journal/:date" element={<MainContent />} />
    <Route path="/problems" element={<ProblemsDemo />} />
    <Route path="/trpc-demo" element={<TRPCDemo />} />
    <Route path="/sqlite-demo" element={<SQLiteDemo />} />
  </Routes>
);

export default function App() {
  const { sidebarState, toggleLeftSidebar, toggleRightSidebar } = useSidebar();
  const { problems } = useProblems();
  const location = useLocation();
  const showRightSidebar = location.pathname.startsWith("/journal");

  return (
    <div className="flex flex-col h-screen bg-gray-50 overflow-hidden">
      <div className="flex flex-1 overflow-hidden">
        <div
          className={`${
            sidebarState.leftSidebar ? "w-64" : "w-0"
          } transition-all duration-300 ease-in-out border-r border-gray-200 bg-white overflow-hidden`}
        >
          {sidebarState.leftSidebar && <LeftSidebar />}
        </div>
        <div className="flex-1 flex flex-col min-w-0">
          <Header
            leftSidebarOpen={sidebarState.leftSidebar}
            rightSidebarOpen={sidebarState.rightSidebar}
            toggleLeftSidebar={toggleLeftSidebar}
            toggleRightSidebar={toggleRightSidebar}
          />
          <div className="flex flex-1 overflow-hidden">
            <div className="flex-1 overflow-y-auto">
              <AppRoutes />
            </div>
            {showRightSidebar && (
              <div
                className={`${
                  sidebarState.rightSidebar ? "w-80" : "w-0"
                } transition-all duration-300 ease-in-out border-l border-gray-200 bg-white overflow-hidden`}
              >
                {sidebarState.rightSidebar && <RightSidebar />}
              </div>
            )}
          </div>
        </div>
      </div>
      <StatusBar problems={problems} />
    </div>
  );
}
