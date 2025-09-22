import React, { useState } from "react";

export interface ProblemStatus {
  errors: number;
  warnings: number;
  info: number;
}

interface StatusBarProps {
  problems?: ProblemStatus;
}

export function StatusBar({ problems = { errors: 0, warnings: 0, info: 0 } }: StatusBarProps) {
  const [showPopup, setShowPopup] = useState(false);

  const totalProblems = problems.errors + problems.warnings + problems.info;

  const getStatusIcon = () => {
    if (problems.errors > 0) {
      return (
        <svg className="w-4 h-4 text-red-500" fill="currentColor" viewBox="0 0 20 20">
          <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
        </svg>
      );
    }
    if (problems.warnings > 0) {
      return (
        <svg className="w-4 h-4 text-yellow-500" fill="currentColor" viewBox="0 0 20 20">
          <path fillRule="evenodd" d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
        </svg>
      );
    }
    return (
      <svg className="w-4 h-4 text-green-500" fill="currentColor" viewBox="0 0 20 20">
        <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
      </svg>
    );
  };

  const getStatusText = () => {
    if (problems.errors > 0) return "error";
    if (problems.warnings > 0) return "warning";
    return "ok";
  };

  return (
    <>
      <div className="h-6 bg-blue-600 border-t border-gray-200 flex items-center justify-between px-4 text-xs relative">
        {/* Left side - Problems button */}
        <div className="flex items-center">
          <button
            onClick={() => setShowPopup(!showPopup)}
            className="flex items-center space-x-2 hover:bg-blue-700 px-2 py-1 rounded transition-colors duration-150 text-white"
          >
            {getStatusIcon()}
            <span className="font-medium">Problems</span>
            {totalProblems > 0 && (
              <span className="bg-white text-blue-600 px-1.5 py-0.5 rounded-full text-xs font-semibold min-w-[1.25rem] text-center">
                {totalProblems}
              </span>
            )}
          </button>
        </div>

        {/* Right side - Additional status info (optional) */}
        <div className="flex items-center space-x-4 text-white">
          <span className="opacity-75">Ready</span>
        </div>

        {/* Popup */}
        {showPopup && (
          <div className="absolute bottom-full left-4 mb-1 w-80 bg-white border border-gray-200 rounded-lg shadow-lg z-50">
            <div className="p-4">
              <div className="flex items-center justify-between mb-3">
                <h3 className="font-semibold text-gray-900">System Status</h3>
                <button
                  onClick={() => setShowPopup(false)}
                  className="text-gray-400 hover:text-gray-600 transition-colors"
                >
                  <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
                    <path fillRule="evenodd" d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z" clipRule="evenodd" />
                  </svg>
                </button>
              </div>

              <div className="space-y-3">
                {/* Problems Summary */}
                <div className="bg-gray-50 rounded-lg p-3">
                  <h4 className="font-medium text-gray-700 mb-2">Problems Summary</h4>
                  <div className="space-y-1 text-sm">
                    {problems.errors > 0 && (
                      <div className="flex items-center space-x-2">
                        <svg className="w-3 h-3 text-red-500" fill="currentColor" viewBox="0 0 20 20">
                          <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
                        </svg>
                        <span className="text-gray-600">{problems.errors} error{problems.errors !== 1 ? 's' : ''}</span>
                      </div>
                    )}
                    {problems.warnings > 0 && (
                      <div className="flex items-center space-x-2">
                        <svg className="w-3 h-3 text-yellow-500" fill="currentColor" viewBox="0 0 20 20">
                          <path fillRule="evenodd" d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
                        </svg>
                        <span className="text-gray-600">{problems.warnings} warning{problems.warnings !== 1 ? 's' : ''}</span>
                      </div>
                    )}
                    {totalProblems === 0 && (
                      <div className="flex items-center space-x-2">
                        <svg className="w-3 h-3 text-green-500" fill="currentColor" viewBox="0 0 20 20">
                          <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
                        </svg>
                        <span className="text-gray-600">No problems detected</span>
                      </div>
                    )}
                  </div>
                </div>

                {/* Background Service Status */}
                <div className="bg-gray-50 rounded-lg p-3">
                  <h4 className="font-medium text-gray-700 mb-2">Background Service</h4>
                  <div className="flex items-center space-x-2 text-sm">
                    <div className="w-2 h-2 bg-green-500 rounded-full"></div>
                    <span className="text-gray-600">Running</span>
                  </div>
                  <p className="text-xs text-gray-500 mt-1">Last sync: Just now</p>
                </div>

                {/* Sync Status */}
                <div className="bg-gray-50 rounded-lg p-3">
                  <h4 className="font-medium text-gray-700 mb-2">Sync Status</h4>
                  <div className="flex items-center space-x-2 text-sm">
                    <div className="w-2 h-2 bg-green-500 rounded-full"></div>
                    <span className="text-gray-600">Up to date</span>
                  </div>
                  <p className="text-xs text-gray-500 mt-1">All changes synchronized</p>
                </div>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Backdrop to close popup when clicking outside */}
      {showPopup && (
        <div
          className="fixed inset-0 z-40"
          onClick={() => setShowPopup(false)}
        />
      )}
    </>
  );
}
