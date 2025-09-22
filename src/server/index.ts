import { appRouter, type AppRouter } from "./router";
import { createPostMessageAdapter } from "./adapter";

export { appRouter, type AppRouter };

// Server class that can be instantiated in the service worker
export class Server {
  private adapter: ReturnType<typeof createPostMessageAdapter>;

  constructor() {
    // Ensure router is properly initialized before creating adapter
    if (!appRouter) {
      throw new Error("AppRouter is not properly initialized");
    }

    console.log("Creating tRPC adapter with router:", appRouter);

    this.adapter = createPostMessageAdapter({
      router: appRouter,
      createContext: () => ({}),
      onError: ({ error, type, path, input }) => {
        console.error("tRPC Error:", {
          error: error.message,
          type,
          path,
          input,
        });
      },
    });
  }

  // Handle incoming messages from the main thread
  async handleMessage(event: MessageEvent) {
    try {
      return await this.adapter.handleMessage(event);
    } catch (error) {
      console.error("Error handling tRPC message:", error);
      throw error;
    }
  }

  // Check if the server is properly initialized
  isReady(): boolean {
    return !!this.adapter && !!appRouter;
  }
}

// Export a default server instance
export const server = new Server();

// Log server readiness
if (server.isReady()) {
  console.log("tRPC server initialized successfully");
} else {
  console.error("tRPC server failed to initialize");
}
