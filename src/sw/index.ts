import { precacheAndRoute } from "workbox-precaching";
import { server } from "../server";

declare const self: ServiceWorkerGlobalScope;

console.log("Service Worker: Starting up...");

try {
  precacheAndRoute(self.__WB_MANIFEST);
  console.log("Service Worker: Precaching complete");
} catch (error) {
  console.error("Service Worker: Precaching failed:", error);
}

console.log("Service Worker: Loaded successfully");
console.log("Service Worker: Server ready:", server.isReady());

self.addEventListener("install", (event: ExtendableEvent) => {
  console.log("Service Worker: Installing...");
  // Skip waiting to activate immediately
  event.waitUntil(self.skipWaiting());
  console.log("Service Worker: Installed and skipping waiting");
});

self.addEventListener("activate", (event: ExtendableEvent) => {
  console.log("Service Worker: Activating...");
  // Claim all clients immediately
  event.waitUntil(
    self.clients.claim().then(() => {
      console.log("Service Worker: Activated and claimed all clients");
    }),
  );
});

self.addEventListener("fetch", (event: FetchEvent) => {
  // Let workbox handle caching, we're using postMessage for tRPC
  // Only log non-chrome-extension requests to reduce noise
  if (!event.request.url.startsWith("chrome-extension://")) {
    console.log("Service Worker: Fetch intercepted:", event.request.url);
  }
});

// Handle all messages through the server instance
self.addEventListener("message", async (event: ExtendableMessageEvent) => {
  console.log("Service Worker: Received message:", event.data);

  try {
    // Handle ping messages directly
    if (
      event.data &&
      typeof event.data === "object" &&
      event.data.type === "ping"
    ) {
      console.log("Service Worker: Handling ping message");
      // Send pong response back to the client
      if (event.source) {
        event.source.postMessage({ type: "pong" });
        console.log("Service Worker: Sent pong response");
      }
      return;
    }

    // Handle tRPC messages through the server
    if (
      event.data &&
      typeof event.data === "object" &&
      event.data.type === "request" &&
      event.data.id &&
      event.data.path
    ) {
      console.log("Service Worker: Handling tRPC request:", event.data.path);
      await server.handleMessage(event as MessageEvent);
    } else {
      console.log("Service Worker: Ignoring non-tRPC message:", event.data);
    }
  } catch (error) {
    console.error("Service Worker: Message handling error:", error);

    // Send error response back if we have a request ID
    if (event.data?.id && event.data?.type === "request" && event.source) {
      const errorResponse = {
        id: event.data.id,
        type: "error",
        error: {
          message: error instanceof Error ? error.message : "Unknown error",
          code: -32603,
        },
      };
      console.log("Service Worker: Sending error response:", errorResponse);
      event.source.postMessage(errorResponse);
    }
  }
});

// Ensure server is ready
console.log("Service Worker: Initializing tRPC server...");
if (server.isReady()) {
  console.log("Service Worker: tRPC server is ready!");
} else {
  console.error("Service Worker: tRPC server failed to initialize!");
}
