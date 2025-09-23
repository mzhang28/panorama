import React, { useState, useEffect } from "react";
import { useParams } from "react-router-dom";
import { Calendar, Edit3, Save } from "lucide-react";
import { useJournalGraphQL } from "../hooks/useJournalGraphQL";
import { JournalEditor } from "./JournalEditor";

export function MainContent() {
  const { date } = useParams();
  const { getEntryByDateAsync, updateEntry, formatDate, isLoading, error } = useJournalGraphQL();

  const [content, setContent] = useState("");
  const [isEditing, setIsEditing] = useState(false);
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);
  const [entryLoading, setEntryLoading] = useState(false);
  const [currentEntry, setCurrentEntry] = useState<any>(null);

  // Load content when date changes
  useEffect(() => {
    const loadEntry = async () => {
      if (!date) {
        setContent("");
        setCurrentEntry(null);
        setHasUnsavedChanges(false);
        setIsEditing(false);
        return;
      }

      setEntryLoading(true);
      try {
        const entry = await getEntryByDateAsync(date);
        setCurrentEntry(entry);
        if (entry) {
          setContent(entry.content);
        } else {
          setContent("");
        }
      } catch (err) {
        console.error("Failed to load entry:", err);
        setContent("");
        setCurrentEntry(null);
      } finally {
        setEntryLoading(false);
      }

      setHasUnsavedChanges(false);
      setIsEditing(false);
    };

    loadEntry();
  }, [date, getEntryByDateAsync]);

  // Handle content changes
  const handleContentChange = (value: string) => {
    setContent(value);
    setHasUnsavedChanges(value !== (currentEntry?.content || ""));
  };

  // Save content
  const handleSave = () => {
    if (date) {
      updateEntry(date, content);
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

  const isToday = date === new Date().toISOString().split("T")[0];

  return (
    <div className="h-full flex flex-col bg-white">
      {/* Header */}
      <div className="px-8 py-6 border-b border-gray-100 bg-gray-50">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-4">
            <div className="flex items-center gap-3">
              <Calendar size={20} className="text-blue-600" />
              <h1 className="text-2xl font-bold text-gray-900">
                {date ? formatDate(date) : "No Date Selected"}
              </h1>
              {isToday && (
                <span className="px-2 py-1 bg-blue-100 text-blue-700 text-xs font-medium rounded-full">
                  Today
                </span>
              )}
            </div>
          </div>

          <div className="flex items-center gap-3">
            {hasUnsavedChanges && (
              <div className="flex items-center gap-2 text-amber-600">
                <div className="w-2 h-2 bg-amber-500 rounded-full animate-pulse"></div>
                <span className="text-sm font-medium">Unsaved changes</span>
              </div>
            )}

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
          </div>
        </div>
      </div>

      {/* Content Area */}
      <div className="flex-1 overflow-y-auto">
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
                : `Viewing journal entry for ${date ? formatDate(date) : "unknown date"}`}
            </p>
          </div>

           {/* Editor */}
           <div className="space-y-4">
             {isLoading ? (
               <div className="min-h-[24rem] flex items-center justify-center border border-gray-200 rounded-lg bg-gray-50">
                 <div className="text-center">
                   <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600 mx-auto mb-4"></div>
                   <p className="text-gray-600">Loading journal entry...</p>
                 </div>
               </div>
             ) : error ? (
               <div className="min-h-[24rem] flex items-center justify-center border border-red-200 rounded-lg bg-red-50">
                 <div className="text-center">
                   <p className="text-red-600 mb-2">Failed to load journal entry</p>
                   <p className="text-sm text-red-500">{error.message}</p>
                 </div>
               </div>
             ) : (
               <JournalEditor
                 value={content}
                 onChange={handleContentChange}
                 onFocus={() => setIsEditing(true)}
                 // placeholder={
                 //   isToday
                 //     ? "What's on your mind today?\n\n- Write about your thoughts\n- Plan your day\n- Reflect on experiences\n- Set goals and intentions"
                 //     : "Start writing..."
                 // }
                 className="w-full"
               />
             )}

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
                        • Updated: {currentEntry.updatedAt.toLocaleDateString()}
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
                  Write three pages of longhand, stream-of-consciousness writing
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
      </div>
    </div>
  );
}
