import React from "react";
import { useProblems } from "../hooks/useProblems";

export function ProblemsDemo() {
  const { problems, addProblem, clearProblems } = useProblems();

  const handleAddError = () => {
    addProblem({
      type: "error",
      message: "Failed to connect to database",
      file: "src/lib/database.ts",
      line: 42,
    });
  };

  const handleAddWarning = () => {
    addProblem({
      type: "warning",
      message: "Deprecated API usage detected",
      file: "src/components/Header.tsx",
      line: 15,
    });
  };

  const handleAddInfo = () => {
    addProblem({
      type: "info",
      message: "Background sync completed successfully",
    });
  };

  return (
    <div className="p-6 bg-white rounded-lg border border-gray-200 shadow-sm">
      <h3 className="text-lg font-semibold text-gray-900 mb-4">
        Problems System Demo
      </h3>

      <div className="space-y-4">
        <div className="flex flex-wrap gap-2">
          <button
            onClick={handleAddError}
            className="px-3 py-2 bg-red-600 text-white text-sm rounded-md hover:bg-red-700 transition-colors"
          >
            Add Error
          </button>
          <button
            onClick={handleAddWarning}
            className="px-3 py-2 bg-yellow-600 text-white text-sm rounded-md hover:bg-yellow-700 transition-colors"
          >
            Add Warning
          </button>
          <button
            onClick={handleAddInfo}
            className="px-3 py-2 bg-blue-600 text-white text-sm rounded-md hover:bg-blue-700 transition-colors"
          >
            Add Info
          </button>
          <button
            onClick={() => clearProblems()}
            className="px-3 py-2 bg-gray-600 text-white text-sm rounded-md hover:bg-gray-700 transition-colors"
          >
            Clear All
          </button>
        </div>

        <div className="p-3 bg-gray-50 rounded-md">
          <h4 className="font-medium text-gray-700 mb-2">Current Problems:</h4>
          <div className="text-sm text-gray-600">
            <div>Errors: {problems.errors}</div>
            <div>Warnings: {problems.warnings}</div>
            <div>Info: {problems.info}</div>
          </div>
        </div>

        <div className="text-sm text-gray-500">
          Click the buttons above to simulate different types of problems.
          Check the status bar at the bottom to see how the problems button updates!
        </div>
      </div>
    </div>
  );
}
