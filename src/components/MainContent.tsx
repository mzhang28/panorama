import React, { useState, useEffect } from "react";
import { Calendar, Edit3, Save, Database } from "lucide-react";
import { useJournal } from "../hooks/useJournal";
import { ProblemsDemo } from "./ProblemsDemo";
import { TRPCDemo } from "./TRPCDemo";

export function MainContent() {
  const { currentDate, getEntryByDate, updateEntry, formatDate } = useJournal();

  const [content, setContent] = useState("");
  const [isEditing, setIsEditing] = useState(false);
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);
  const [activeTab, setActiveTab] = useState<"journal" | "trpc" | "problems">(
    "journal",
  );

  const currentEntry = currentDate ? getEntryByDate(currentDate) : undefined;

  // Load content when date changes
  useEffect(() => {
    if (currentEntry) {
      setContent(currentEntry.content);
    } else {
      setContent("");
    }
    setHasUnsavedChanges(false);
    setIsEditing(false);
  }, [currentDate, currentEntry]);

  // Handle content changes
  const handleContentChange = (value: string) => {
    setContent(value);
    setHasUnsavedChanges(value !== (currentEntry?.content || ""));
  };

  // Save content
  const handleSave = () => {
    if (currentDate) {
      updateEntry(currentDate, content);
      setHasUnsavedChanges(false);
      setIsEditing(false);
    }
  };

  // Auto-save after 2 seconds of inactivity
  useEffect(() => {
    if (hasUnsavedChanges) {
      const timer = setTimeout(() => {
        handleSave();
      }, 2000);

      return () => clearTimeout(timer);
    }
  }, [content, hasUnsavedChanges]);

  const isToday = currentDate === new Date().toISOString().split("T")[0];

  return (
    <div className="h-full flex flex-col bg-white">
      {/* Header with Tabs */}
      <div className="px-8 py-6 border-b border-gray-100 bg-gray-50">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-4">
            <div className="flex items-center gap-3">
              {activeTab === "journal" && (
                <Calendar size={20} className="text-blue-600" />
              )}
              {activeTab === "trpc" && (
                <Database size={20} className="text-purple-600" />
              )}
              {activeTab === "problems" && (
                <Edit3 size={20} className="text-green-600" />
              )}
              <h1 className="text-2xl font-bold text-gray-900">
                {activeTab === "journal" &&
                  (currentDate ? formatDate(currentDate) : "No Date Selected")}
                {activeTab === "trpc" && "tRPC Service Worker"}
                {activeTab === "problems" && "Problems Demo"}
              </h1>
              {activeTab === "journal" && isToday && (
                <span className="px-2 py-1 bg-blue-100 text-blue-700 text-xs font-medium rounded-full">
                  Today
                </span>
              )}
            </div>
          </div>

          <div className="flex items-center gap-3">
            {activeTab === "journal" && hasUnsavedChanges && (
              <div className="flex items-center gap-2 text-amber-600">
                <div className="w-2 h-2 bg-amber-500 rounded-full animate-pulse"></div>
                <span className="text-sm font-medium">Unsaved changes</span>
              </div>
            )}

            {activeTab === "journal" && (
              <button
                onClick={handleSave}
                disabled={!hasUnsavedChanges}
                className={`flex items-center gap-2 px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${
                  hasUnsavedChanges
                    ? "bg-blue-600 text-white hover:bg-blue-700"
                    : "bg-gray-100 text-gray-400 cursor-not-allowed"
                }`}
              >
                <Save size={14} />
                Save
              </button>
            )}
          </div>
        </div>

        {/* Tab Navigation */}
        <div className="flex gap-1 bg-white rounded-lg p-1 border border-gray-200">
          <button
            onClick={() => setActiveTab("journal")}
            className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-colors ${
              activeTab === "journal"
                ? "bg-blue-600 text-white"
                : "text-gray-600 hover:text-gray-900 hover:bg-gray-50"
            }`}
          >
            <Calendar size={16} />
            Journal
          </button>
          <button
            onClick={() => setActiveTab("trpc")}
            className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-colors ${
              activeTab === "trpc"
                ? "bg-purple-600 text-white"
                : "text-gray-600 hover:text-gray-900 hover:bg-gray-50"
            }`}
          >
            <Database size={16} />
            tRPC Demo
          </button>
          <button
            onClick={() => setActiveTab("problems")}
            className={`flex items-center gap-2 px-4 py-2 rounded-md text-sm font-medium transition-colors ${
              activeTab === "problems"
                ? "bg-green-600 text-white"
                : "text-gray-600 hover:text-gray-900 hover:bg-gray-50"
            }`}
          >
            <Edit3 size={16} />
            Problems
          </button>
        </div>
      </div>

      {/* Content Area */}
      <div className="flex-1 overflow-y-auto">
        {activeTab === "journal" && (
          <div className="max-w-4xl mx-auto p-8">
            {/* Quick Actions */}
            <div className="mb-6 p-4 bg-blue-50 rounded-lg border border-blue-100">
              <div className="flex items-center gap-2 mb-2">
                <Edit3 size={16} className="text-blue-600" />
                <span className="text-sm font-medium text-blue-900">
                  Daily Journal
                </span>
              </div>
              <p className="text-sm text-blue-700">
                {isToday
                  ? "Start writing about your day, thoughts, or plans..."
                  : `Viewing journal entry for ${currentDate ? formatDate(currentDate) : "unknown date"}`}
              </p>
            </div>

            {/* Editor */}
            <div className="space-y-4">
              <textarea
                value={content}
                onChange={(e) => handleContentChange(e.currentTarget.value)}
                onFocus={() => setIsEditing(true)}
                placeholder={
                  isToday
                    ? "What's on your mind today?\n\n- Write about your thoughts\n- Plan your day\n- Reflect on experiences\n- Set goals and intentions"
                    : "Start writing..."
                }
                className="w-full h-96 p-6 border border-gray-200 rounded-lg resize-none focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent text-gray-900 leading-relaxed"
                style={{
                  fontSize: "16px",
                  lineHeight: "1.6",
                  fontFamily:
                    '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
                }}
              />

              {/* Editor Footer */}
              <div className="flex justify-between items-center text-sm text-gray-500 pt-2">
                <div className="flex items-center gap-4">
                  <span>{content.length} characters</span>
                  <span>{content.split("\n").length} lines</span>
                </div>

                <div className="flex items-center gap-2">
                  {currentEntry && (
                    <>
                      <span>
                        Created: {currentEntry.createdAt.toLocaleDateString()}
                      </span>
                      {currentEntry.updatedAt !== currentEntry.createdAt && (
                        <span>
                          • Updated:{" "}
                          {currentEntry.updatedAt.toLocaleDateString()}
                        </span>
                      )}
                    </>
                  )}
                </div>
              </div>
            </div>

            {/* Suggestions (when empty) */}
            {!content && !isEditing && (
              <div className="mt-8 grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="p-4 bg-gray-50 rounded-lg border border-gray-200">
                  <h3 className="font-medium text-gray-900 mb-2">
                    Morning Pages
                  </h3>
                  <p className="text-sm text-gray-600">
                    Write three pages of longhand, stream-of-consciousness
                    writing
                  </p>
                </div>
                <div className="p-4 bg-gray-50 rounded-lg border border-gray-200">
                  <h3 className="font-medium text-gray-900 mb-2">
                    Daily Reflection
                  </h3>
                  <p className="text-sm text-gray-600">
                    What went well today? What could be improved?
                  </p>
                </div>
                <div className="p-4 bg-gray-50 rounded-lg border border-gray-200">
                  <h3 className="font-medium text-gray-900 mb-2">
                    Goals & Plans
                  </h3>
                  <p className="text-sm text-gray-600">
                    Set intentions and plan your upcoming tasks and projects
                  </p>
                </div>
                <div className="p-4 bg-gray-50 rounded-lg border border-gray-200">
                  <h3 className="font-medium text-gray-900 mb-2">Gratitude</h3>
                  <p className="text-sm text-gray-600">
                    Note three things you're grateful for today
                  </p>
                </div>
              </div>
            )}
          </div>
        )}

        {activeTab === "trpc" && <TRPCDemo />}

        {activeTab === "problems" && <ProblemsDemo />}
      </div>
    </div>
  );
}
