import React, { useState } from "react";
import {
  useHealth,
  useCounter,
  useUsers,
  useIncrementCounter,
  useCreateUser,
  useEcho,
} from "../hooks/useTRPC";
import { serviceWorkerUtils } from "../lib/trpc";

export function TRPCDemo() {
  const [swStatus, setSWStatus] = useState<"checking" | "ready" | "not-ready">(
    "checking",
  );
  const [echoMessage, setEchoMessage] = useState("");
  const [userName, setUserName] = useState("");
  const [userEmail, setUserEmail] = useState("");
  const [initError, setInitError] = useState<string | null>(null);

  // Queries - only run when service worker is ready
  const health = useHealth();
  const counter = useCounter();
  const users = useUsers();

  // Mutations
  const incrementCounter = useIncrementCounter();
  const createUser = useCreateUser();
  const echo = useEcho();

  // Check service worker status on mount
  React.useEffect(() => {
    const checkSW = async () => {
      try {
        if (!serviceWorkerUtils.isAvailable()) {
          setSWStatus("not-ready");
          setInitError("Service Worker not supported in this browser");
          return;
        }

        // Wait for service worker to be ready
        await serviceWorkerUtils.waitForServiceWorker();

        // Try to ping it
        const isReady = await serviceWorkerUtils.ping();
        setSWStatus(isReady ? "ready" : "not-ready");

        if (!isReady) {
          setInitError("Service Worker is not responding to ping");
        } else {
          setInitError(null);
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

  const handleEcho = async () => {
    if (!echoMessage.trim()) return;
    try {
      await echo.mutate({ message: echoMessage });
    } catch (error) {
      console.error("Echo failed:", error);
    }
  };

  const handleCreateUser = async () => {
    if (!userName.trim() || !userEmail.trim()) return;
    try {
      await createUser.mutate({ name: userName, email: userEmail });
      setUserName("");
      setUserEmail("");
      // Refetch users after creating one
      users.refetch();
    } catch (error) {
      console.error("Create user failed:", error);
    }
  };

  const handleIncrement = async (amount: number) => {
    try {
      await incrementCounter.mutate({ amount });
      // Refetch counter after incrementing
      counter.refetch();
    } catch (error) {
      console.error("Increment failed:", error);
    }
  };

  return (
    <div className="p-6 max-w-4xl mx-auto space-y-6">
      <h1 className="text-3xl font-bold text-gray-800 mb-6">
        tRPC Service Worker Demo
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
          {swStatus === "not-ready" && !initError && (
            <div className="text-sm text-yellow-600 bg-yellow-50 p-2 rounded">
              Service Worker is required for tRPC communication. Please refresh
              the page or check your browser support.
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
                The tRPC demo requires a working Service Worker. Please wait for
                it to initialize or refresh the page if the issue persists.
              </p>
            </div>
          </div>
        </div>
      )}

      {/* Health Check */}
      <div className="bg-white rounded-lg shadow p-4 border">
        <h2 className="text-xl font-semibold mb-2">Health Check</h2>
        {!health.isServiceWorkerReady ? (
          <p className="text-yellow-600">Waiting for Service Worker...</p>
        ) : health.isLoading ? (
          <p className="text-gray-600">Loading...</p>
        ) : health.error ? (
          <div className="space-y-2">
            <p className="text-red-600">Error: {health.error.message}</p>
            <details className="text-sm text-gray-600">
              <summary className="cursor-pointer">Error Details</summary>
              <pre className="mt-1 p-2 bg-gray-100 rounded text-xs overflow-auto">
                {health.error.stack || health.error.message}
              </pre>
            </details>
          </div>
        ) : health.data ? (
          <div className="space-y-1">
            <p>
              Status:{" "}
              <span className="font-mono text-green-600">
                {health.data.status}
              </span>
            </p>
            <p>
              Timestamp:{" "}
              <span className="font-mono">{health.data.timestamp}</span>
            </p>
          </div>
        ) : null}
        <button
          onClick={() => health.refetch()}
          disabled={!health.isServiceWorkerReady}
          className="mt-2 px-3 py-1 bg-blue-500 text-white rounded hover:bg-blue-600 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          Refresh
        </button>
      </div>

      {/* Echo Test */}
      <div className="bg-white rounded-lg shadow p-4 border">
        <h2 className="text-xl font-semibold mb-2">Echo Test</h2>
        <div className="space-y-2">
          <div className="flex space-x-2">
            <input
              type="text"
              value={echoMessage}
              onChange={(e) => setEchoMessage(e.target.value)}
              placeholder="Enter message to echo"
              className="flex-1 px-3 py-2 border rounded focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
            <button
              onClick={handleEcho}
              disabled={echo.isLoading || !echoMessage.trim()}
              className="px-4 py-2 bg-green-500 text-white rounded hover:bg-green-600 disabled:opacity-50"
            >
              {echo.isLoading ? "Sending..." : "Echo"}
            </button>
          </div>
          {echo.data && (
            <p className="text-green-600 font-mono">Echo: {echo.data.echo}</p>
          )}
          {echo.error && (
            <p className="text-red-600">Error: {echo.error.message}</p>
          )}
        </div>
      </div>

      {/* Counter */}
      <div className="bg-white rounded-lg shadow p-4 border">
        <h2 className="text-xl font-semibold mb-2">Counter</h2>
        {counter.isLoading ? (
          <p className="text-gray-600">Loading...</p>
        ) : counter.error ? (
          <p className="text-red-600">Error: {counter.error.message}</p>
        ) : counter.data ? (
          <p className="text-2xl font-mono">{counter.data.count}</p>
        ) : null}

        <div className="flex space-x-2 mt-2">
          <button
            onClick={() => handleIncrement(1)}
            disabled={incrementCounter.isLoading}
            className="px-3 py-1 bg-blue-500 text-white rounded hover:bg-blue-600 disabled:opacity-50"
          >
            +1
          </button>
          <button
            onClick={() => handleIncrement(5)}
            disabled={incrementCounter.isLoading}
            className="px-3 py-1 bg-blue-500 text-white rounded hover:bg-blue-600 disabled:opacity-50"
          >
            +5
          </button>
          <button
            onClick={() => handleIncrement(-1)}
            disabled={incrementCounter.isLoading}
            className="px-3 py-1 bg-red-500 text-white rounded hover:bg-red-600 disabled:opacity-50"
          >
            -1
          </button>
        </div>

        {incrementCounter.data && (
          <p className="text-green-600 mt-2">{incrementCounter.data.message}</p>
        )}
        {incrementCounter.error && (
          <p className="text-red-600 mt-2">
            Error: {incrementCounter.error.message}
          </p>
        )}
      </div>

      {/* User Management */}
      <div className="bg-white rounded-lg shadow p-4 border">
        <h2 className="text-xl font-semibold mb-2">User Management</h2>

        {/* Create User Form */}
        <div className="space-y-2 mb-4">
          <h3 className="font-medium">Create User</h3>
          <div className="grid grid-cols-2 gap-2">
            <input
              type="text"
              value={userName}
              onChange={(e) => setUserName(e.target.value)}
              placeholder="Name"
              className="px-3 py-2 border rounded focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
            <input
              type="email"
              value={userEmail}
              onChange={(e) => setUserEmail(e.target.value)}
              placeholder="Email"
              className="px-3 py-2 border rounded focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>
          <button
            onClick={handleCreateUser}
            disabled={
              createUser.isLoading || !userName.trim() || !userEmail.trim()
            }
            className="px-4 py-2 bg-purple-500 text-white rounded hover:bg-purple-600 disabled:opacity-50"
          >
            {createUser.isLoading ? "Creating..." : "Create User"}
          </button>
          {createUser.data && (
            <p className="text-green-600">
              User created: {createUser.data.name}
            </p>
          )}
          {createUser.error && (
            <p className="text-red-600">Error: {createUser.error.message}</p>
          )}
        </div>

        {/* User List */}
        <div>
          <h3 className="font-medium mb-2">Users</h3>
          {users.isLoading ? (
            <p className="text-gray-600">Loading users...</p>
          ) : users.error ? (
            <p className="text-red-600">Error: {users.error.message}</p>
          ) : users.data ? (
            <div className="space-y-2">
              {users.data.map((user) => (
                <div key={user.id} className="p-2 bg-gray-50 rounded">
                  <p className="font-medium">{user.name}</p>
                  <p className="text-sm text-gray-600">{user.email}</p>
                  <p className="text-xs text-gray-500">ID: {user.id}</p>
                </div>
              ))}
            </div>
          ) : null}
          <button
            onClick={() => users.refetch()}
            className="mt-2 px-3 py-1 bg-gray-500 text-white rounded hover:bg-gray-600"
          >
            Refresh Users
          </button>
        </div>
      </div>
    </div>
  );
}
