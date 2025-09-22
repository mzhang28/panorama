import React from "react";
import { Link, useParams } from "react-router-dom";
import {
  Calendar,
  FileText,
  Hash,
  Clock,
  Plus,
  ChevronRight,
  AlertTriangle,
  Zap,
} from "lucide-react";
import { useJournal } from "../hooks/useJournal";

export function LeftSidebar() {
  const { getRecentEntries, formatDate } = useJournal();
  const recentEntries = getRecentEntries();
  const { date } = useParams();

  const navigationItems = [
    {
      icon: Calendar,
      label: "Today",
      path: `/journal/${new Date().toISOString().split("T")[0]}`,
    },
    {
      icon: FileText,
      label: "All Pages",
      path: "/all-pages",
    },
    {
      icon: Hash,
      label: "Tags",
      path: "/tags",
    },
    {
      icon: Clock,
      label: "Recent",
      path: "/recent",
    },
    {
      icon: AlertTriangle,
      label: "Problems",
      path: "/problems",
    },
    {
      icon: Zap,
      label: "TRPC Demo",
      path: "/trpc-demo",
    },
  ];

  return (
    <div className="h-full flex flex-col bg-white">
      {/* Header */}
      <div className="p-4 border-b border-gray-100">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold text-gray-700 uppercase tracking-wider">
            Navigation
          </h2>
          <button className="p-1 rounded hover:bg-gray-100 transition-colors">
            <Plus size={16} className="text-gray-500" />
          </button>
        </div>
      </div>

      {/* Navigation Items */}
      <div className="px-3 py-2">
        {navigationItems.map((item, index) => (
          <Link
            key={index}
            to={item.path}
            className={`w-full flex items-center gap-3 px-3 py-2 rounded-md text-left transition-colors ${
              date === item.path.split("/").pop() ||
              (item.label === "Today" &&
                date === new Date().toISOString().split("T")[0])
                ? "bg-blue-50 text-blue-700 border border-blue-200"
                : "text-gray-700 hover:bg-gray-50"
            }`}
          >
            <item.icon
              size={16}
              className={
                date === item.path.split("/").pop()
                  ? "text-blue-600"
                  : "text-gray-500"
              }
            />
            <span className="text-sm font-medium">{item.label}</span>
          </Link>
        ))}
      </div>

      {/* Divider */}
      <div className="border-t border-gray-100 my-4" />

      {/* Recent Entries */}
      <div className="px-4 mb-3">
        <h3 className="text-sm font-semibold text-gray-700 uppercase tracking-wider">
          Recent
        </h3>
      </div>

      <div className="px-3 flex-1 overflow-y-auto">
        {recentEntries.length > 0 ? (
          <div className="space-y-1">
            {recentEntries.map((entry) => (
              <Link
                key={entry.id}
                to={`/journal/${entry.date}`}
                className={`w-full flex items-start gap-3 px-3 py-2 rounded-md text-left transition-colors group ${
                  date === entry.date
                    ? "bg-gray-100 border border-gray-200"
                    : "hover:bg-gray-50"
                }`}
              >
                <Calendar
                  size={14}
                  className="text-gray-400 mt-0.5 flex-shrink-0"
                />
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium text-gray-900 truncate">
                      {formatDate(entry.date)}
                    </span>
                    <ChevronRight
                      size={12}
                      className={`text-gray-400 transition-transform ${
                        date === entry.date
                          ? "rotate-90"
                          : "group-hover:translate-x-0.5"
                      }`}
                    />
                  </div>
                  {entry.content && (
                    <p className="text-xs text-gray-500 mt-1 line-clamp-2">
                      {entry.content.substring(0, 60)}
                      {entry.content.length > 60 ? "..." : ""}
                    </p>
                  )}
                </div>
              </Link>
            ))}
          </div>
        ) : (
          <div className="text-center py-8">
            <FileText size={32} className="text-gray-300 mx-auto mb-2" />
            <p className="text-sm text-gray-500">No recent entries</p>
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="p-4 border-t border-gray-100">
        <button className="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-600 hover:bg-gray-50 rounded-md transition-colors">
          <Plus size={14} />
          <span>New Page</span>
        </button>
      </div>
    </div>
  );
}
