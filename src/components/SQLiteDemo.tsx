import React, { useState, useEffect } from "react";
import { trpc } from "../lib/trpc";
import { serviceWorkerUtils } from "../lib/trpc";

export function SQLiteDemo() {
  const [swStatus, setSWStatus] = useState<"checking" | "ready" | "not-ready">(
    "checking",
  );
  const [initError, setInitError] = useState<string | null>(null);
  const [tables, setTables] = useState<string[]>([]);
  const [selectedTable, setSelectedTable] = useState<string>("");
  const [tableData, setTableData] = useState<any[]>([]);
  const [tableInfo, setTableInfo] = useState<any[]>([]);
  const [sqlCommand, setSqlCommand] = useState("");
  const [sqlResult, setSqlResult] = useState<any>(null);
  const [isLoading, setIsLoading] = useState(false);

  // Check service worker status on mount
  useEffect(() => {
    const checkSW = async () => {
      try {
        if (!serviceWorkerUtils.isAvailable()) {
          setSWStatus("not-ready");
          setInitError("Service Worker not supported in this browser");
          return;
        }

        await serviceWorkerUtils.waitForServiceWorker();
        const isReady = await serviceWorkerUtils.ping();
        setSWStatus(isReady ? "ready" : "not-ready");

        if (!isReady) {
          setInitError("Service Worker is not responding to ping");
        } else {
          setInitError(null);
          // Load tables when service worker is ready
          setTimeout(() => loadTables(), 500); // Small delay to ensure DB is ready
        }
      } catch (error) {
        console.error("Service worker initialization error:", error);
        setSWStatus("not-ready");
        setInitError(
          `Service Worker error: ${error instanceof Error ? error.message : "Unknown error"}`,
        );
      }
    };
    checkSW();
  }, []);

  const loadTables = async () => {
    if (swStatus !== "ready") return;

    try {
      setIsLoading(true);
      const result = await trpc.getTables.query();
      setTables(result);
    } catch (error) {
      console.error("Failed to load tables:", error);
      // Retry after a short delay if database is not ready
      if (error instanceof Error && error.message?.includes("Database not ready")) {
        setTimeout(() => loadTables(), 1000);
      }
    } finally {
      setIsLoading(false);
    }
  };

  const loadTableData = async (tableName: string) => {
    if (!tableName || swStatus !== "ready") return;

    setIsLoading(true);
    try {
      const dataResult = await trpc.queryTable.query({ tableName, limit: 50 });

      setTableData(dataResult);
      // Extract column names from the data itself
      if (dataResult.length > 0) {
        const columns = Object.keys(dataResult[0]).map(name => ({
          name,
          type: 'TEXT', // We don't know the actual type, but this works for display
          notnull: 0,
          pk: 0,
          dflt_value: null
        }));
        setTableInfo(columns);
      } else {
        // No data, but table exists - we can't determine columns
        setTableInfo([]);
      }
    } catch (error) {
      console.error("Failed to load table data:", error);
      // Retry if database is not ready
      if (error instanceof Error && error.message?.includes("Database not ready")) {
        setTimeout(() => loadTableData(tableName), 1000);
      }
    } finally {
      setIsLoading(false);
    }
  };

  const executeSql = async () => {
    if (!sqlCommand.trim() || swStatus !== "ready") return;

    setIsLoading(true);
    try {
      const result = await trpc.executeSql.mutate({ sql: sqlCommand });
      setSqlResult(result);

      // Refresh tables if this was a DDL command
      if (sqlCommand.toUpperCase().includes('CREATE') ||
          sqlCommand.toUpperCase().includes('DROP') ||
          sqlCommand.toUpperCase().includes('ALTER')) {
        loadTables();
      }

      // Refresh current table data if it was modified
      if (selectedTable && (
        sqlCommand.toUpperCase().includes('INSERT') ||
        sqlCommand.toUpperCase().includes('UPDATE') ||
        sqlCommand.toUpperCase().includes('DELETE')
      )) {
        loadTableData(selectedTable);
      }
    } catch (error) {
      console.error("Failed to execute SQL:", error);
      setSqlResult({ error: error instanceof Error ? error.message : "Unknown error" });
    } finally {
      setIsLoading(false);
    }
  };

  const handleTableSelect = (tableName: string) => {
    setSelectedTable(tableName);
    loadTableData(tableName);
  };

  return (
    <div className="p-6 max-w-7xl mx-auto space-y-6">
      <h1 className="text-3xl font-bold text-gray-800 mb-6">
        SQLite Database Explorer
      </h1>

      {/* Service Worker Status */}
      <div className="bg-white rounded-lg shadow p-4 border">
        <h2 className="text-xl font-semibold mb-2">Service Worker Status</h2>
        <div className="space-y-2">
          <div className="flex items-center space-x-2">
            <div
              className={`w-3 h-3 rounded-full ${
                swStatus === "ready"
                  ? "bg-green-500"
                  : swStatus === "not-ready"
                    ? "bg-red-500"
                    : "bg-yellow-500"
              }`}
            />
            <span>
              {swStatus === "ready"
                ? "Connected"
                : swStatus === "not-ready"
                  ? "Not Connected"
                  : "Checking..."}
            </span>
          </div>
          {initError && (
            <div className="text-sm text-red-600 bg-red-50 p-2 rounded">
              Error: {initError}
            </div>
          )}
        </div>
      </div>

      {/* Show warning if service worker is not ready */}
      {swStatus !== "ready" && (
        <div className="bg-yellow-50 border-l-4 border-yellow-400 p-4">
          <div className="flex">
            <div className="ml-3">
              <p className="text-sm text-yellow-700">
                The SQLite demo requires a working Service Worker. Please wait for
                it to initialize or refresh the page if the issue persists.
              </p>
            </div>
          </div>
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Tables List */}
        <div className="bg-white rounded-lg shadow p-4 border">
          <h2 className="text-xl font-semibold mb-4">Tables</h2>
          {tables.length === 0 ? (
            <p className="text-gray-500">No tables found</p>
          ) : (
            <div className="space-y-2">
              {tables.map((table) => (
                <button
                  key={table}
                  onClick={() => handleTableSelect(table)}
                  className={`w-full text-left px-3 py-2 rounded ${
                    selectedTable === table
                      ? "bg-blue-100 text-blue-800"
                      : "hover:bg-gray-100"
                  }`}
                >
                  {table}
                </button>
              ))}
            </div>
          )}
          <button
            onClick={loadTables}
            className="mt-4 px-3 py-1 bg-gray-500 text-white rounded hover:bg-gray-600 text-sm"
          >
            Refresh Tables
          </button>
        </div>

        {/* Table Data */}
        <div className="bg-white rounded-lg shadow p-4 border lg:col-span-2">
          <h2 className="text-xl font-semibold mb-4">
            {selectedTable ? `Table: ${selectedTable}` : "Select a table"}
          </h2>

          {selectedTable && (
            <>
              {/* Table Info */}
              {tableInfo.length > 0 && (
                <div className="mb-4">
                  <h3 className="font-medium mb-2">Schema</h3>
                  <div className="overflow-x-auto">
                    <table className="min-w-full text-sm">
                      <thead>
                        <tr className="border-b">
                          <th className="text-left py-1">Column</th>
                          <th className="text-left py-1">Type</th>
                          <th className="text-left py-1">Not Null</th>
                          <th className="text-left py-1">Primary Key</th>
                        </tr>
                      </thead>
                      <tbody>
                        {tableInfo.map((col, index) => (
                          <tr key={index} className="border-b">
                            <td className="py-1 font-mono">{col.name}</td>
                            <td className="py-1">{col.type}</td>
                            <td className="py-1">{col.notnull ? "Yes" : "No"}</td>
                            <td className="py-1">{col.pk ? "Yes" : "No"}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </div>
              )}

               {/* Table Data */}
               <div>
                 <h3 className="font-medium mb-2">Data ({tableData.length} rows), Schema ({tableInfo.length} columns)</h3>
                 {isLoading ? (
                   <p className="text-gray-500">Loading...</p>
                 ) : tableInfo.length === 0 ? (
                   <p className="text-gray-500">No table schema available</p>
                 ) : (
                   <div className="overflow-x-auto max-h-96">
                     <table className="min-w-full text-sm border">
                       <thead>
                         <tr className="border-b bg-gray-50">
                           {tableInfo.map((col) => (
                             <th key={col.name} className="text-left py-2 px-2 border-r">{col.name}</th>
                           ))}
                         </tr>
                       </thead>
                       <tbody>
                         {tableData.length === 0 ? (
                           <tr>
                             <td colSpan={tableInfo.length} className="py-4 text-center text-gray-500 border">
                               No data
                             </td>
                           </tr>
                         ) : (
                           tableData.map((row, index) => (
                             <tr key={index} className="border-b">
                               {tableInfo.map((col) => (
                                 <td key={col.name} className="py-1 px-2 font-mono text-xs border-r">
                                   {row[col.name] === null ? "NULL" : String(row[col.name] || "")}
                                 </td>
                               ))}
                             </tr>
                           ))
                         )}
                       </tbody>
                     </table>
                   </div>
                 )}
               </div>
            </>
          )}
        </div>
      </div>

      {/* SQL Command Executor */}
      <div className="bg-white rounded-lg shadow p-4 border">
        <h2 className="text-xl font-semibold mb-4">SQL Command Executor</h2>
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium mb-2">SQL Command</label>
            <textarea
              value={sqlCommand}
              onChange={(e) => setSqlCommand(e.target.value)}
              placeholder="Enter SQL command (SELECT, INSERT, UPDATE, DELETE, CREATE, etc.)"
              className="w-full h-32 px-3 py-2 border rounded font-mono text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>

          <div className="flex space-x-2">
            <button
              onClick={executeSql}
              disabled={isLoading || !sqlCommand.trim()}
              className="px-4 py-2 bg-green-500 text-white rounded hover:bg-green-600 disabled:opacity-50"
            >
              {isLoading ? "Executing..." : "Execute SQL"}
            </button>
            <button
              onClick={() => setSqlCommand("")}
              className="px-4 py-2 bg-gray-500 text-white rounded hover:bg-gray-600"
            >
              Clear
            </button>
          </div>

          {/* SQL Result */}
          {sqlResult && (
            <div className="border rounded p-4">
              <h3 className="font-medium mb-2">Result</h3>
              {sqlResult.error ? (
                <div className="text-red-600 bg-red-50 p-2 rounded">
                  Error: {sqlResult.error}
                </div>
               ) : sqlResult.type === 'select' ? (
                 <div>
                   <p className="text-sm text-gray-600 mb-2">
                     Query returned {sqlResult.results.length} rows
                   </p>
                   {(sqlResult.results.length > 0 || (sqlResult.columns && sqlResult.columns.length > 0)) && (
                     <div className="overflow-x-auto max-h-48">
                       <table className="min-w-full text-sm">
                         <thead>
                           <tr className="border-b">
                             {(sqlResult.results.length > 0 ? Object.keys(sqlResult.results[0]) : sqlResult.columns || []).map((key: string) => (
                               <th key={key} className="text-left py-1">{key}</th>
                             ))}
                           </tr>
                         </thead>
                         <tbody>
                           {sqlResult.results.length === 0 ? (
                             <tr>
                               <td colSpan={(sqlResult.results.length > 0 ? Object.keys(sqlResult.results[0]).length : sqlResult.columns?.length || 1)} className="py-4 text-center text-gray-500">
                                 No data
                               </td>
                             </tr>
                           ) : (
                             sqlResult.results.slice(0, 10).map((row: any, index: number) => (
                               <tr key={index} className="border-b">
                                 {Object.values(row).map((value: any, cellIndex: number) => (
                                   <td key={cellIndex} className="py-1 font-mono text-xs">
                                     {value === null ? "NULL" : String(value)}
                                   </td>
                                 ))}
                               </tr>
                             ))
                           )}
                         </tbody>
                       </table>
                       {sqlResult.results.length > 10 && (
                         <p className="text-sm text-gray-500 mt-2">
                           ... and {sqlResult.results.length - 10} more rows
                         </p>
                       )}
                     </div>
                   )}
                 </div>
              ) : (
                <div className="text-green-600 bg-green-50 p-2 rounded">
                  {sqlResult.message}
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Quick SQL Examples */}
      <div className="bg-white rounded-lg shadow p-4 border">
        <h2 className="text-xl font-semibold mb-4">Quick Examples</h2>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <h3 className="font-medium mb-2">Create a test table</h3>
            <button
              onClick={() => setSqlCommand(`CREATE TABLE test_table (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  value INTEGER DEFAULT 0,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
)`)}
              className="px-3 py-1 bg-blue-500 text-white rounded hover:bg-blue-600 text-sm"
            >
              Load SQL
            </button>
          </div>

          <div>
            <h3 className="font-medium mb-2">Insert test data</h3>
            <button
              onClick={() => setSqlCommand(`INSERT INTO test_table (name, value) VALUES
  ('Item 1', 100),
  ('Item 2', 200),
  ('Item 3', 300)`)}
              className="px-3 py-1 bg-blue-500 text-white rounded hover:bg-blue-600 text-sm"
            >
              Load SQL
            </button>
          </div>

          <div>
            <h3 className="font-medium mb-2">Query all users</h3>
            <button
              onClick={() => setSqlCommand("SELECT * FROM users")}
              className="px-3 py-1 bg-blue-500 text-white rounded hover:bg-blue-600 text-sm"
            >
              Load SQL
            </button>
          </div>

          <div>
            <h3 className="font-medium mb-2">Show all tables</h3>
            <button
              onClick={() => setSqlCommand("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")}
              className="px-3 py-1 bg-blue-500 text-white rounded hover:bg-blue-600 text-sm"
            >
              Load SQL
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}