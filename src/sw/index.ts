import { precacheAndRoute } from "workbox-precaching";
import { initTRPC } from "@trpc/server";
import { z } from "zod";
import { sqliteService } from "./sqlite";
import { GraphQLExecutor } from "../lib/graphql";

declare const self: ServiceWorkerGlobalScope;

// Initialize tRPC
const t = initTRPC.create();

// Define procedures
const router = t.router;
const publicProcedure = t.procedure;

// Initialize database and GraphQL executor
let graphQLExecutor: GraphQLExecutor | null = null;
let dbInitialized = false;

const initDatabase = async () => {
  try {
    await sqliteService.init();
    dbInitialized = true;
    graphQLExecutor = new GraphQLExecutor(sqliteService);
    console.log("Service Worker: Database and GraphQL initialized successfully");
  } catch (error) {
    console.error("Service Worker: Failed to initialize database:", error);
    dbInitialized = false;
  }
};

// Initialize immediately
initDatabase();

// Also try to initialize on activate in case the first attempt fails
self.addEventListener("activate", (event: ExtendableEvent) => {
  console.log("Service Worker: Activating...");
  event.waitUntil(
    self.clients.claim().then(async () => {
      console.log("Service Worker: Activated and claimed all clients");
      // Try to initialize again if not already done
      if (!dbInitialized) {
        console.log("Service Worker: Retrying database initialization on activate...");
        await initDatabase();
      }
    }),
  );
});

// Define the app router with all procedures
const appRouter = router({
  // Health check procedure
  health: publicProcedure.query(() => {
    return {
      status: dbInitialized ? "ok" : "initializing",
      timestamp: new Date().toISOString(),
      dbReady: dbInitialized
    };
  }),

  // Echo procedure that returns the input
  echo: publicProcedure
    .input(z.object({ message: z.string() }))
    .query(({ input }) => {
      return { echo: input.message };
    }),

  // Counter procedures (using SQLite for persistence)
  getCounter: publicProcedure.query(async () => {
    if (!dbInitialized) throw new Error("Database not ready");
    try {
      const results = await sqliteService.query("SELECT value FROM counter WHERE id = 1");
      return { count: results.length > 0 ? results[0].value : 0 };
    } catch (error) {
      // If table doesn't exist, create it and return 0
      await sqliteService.exec("CREATE TABLE IF NOT EXISTS counter (id INTEGER PRIMARY KEY, value INTEGER DEFAULT 0)");
      return { count: 0 };
    }
  }),

  incrementCounter: publicProcedure
    .input(z.object({ amount: z.number().default(1) }))
    .mutation(async ({ input }) => {
      if (!dbInitialized) throw new Error("Database not ready");

      // Ensure counter table exists
      await sqliteService.exec("CREATE TABLE IF NOT EXISTS counter (id INTEGER PRIMARY KEY, value INTEGER DEFAULT 0)");

      // Get current value
      const results = await sqliteService.query("SELECT value FROM counter WHERE id = 1");
      const currentValue = results.length > 0 ? results[0].value : 0;
      const newValue = currentValue + input.amount;

      // Update or insert
      await sqliteService.exec(
        "INSERT OR REPLACE INTO counter (id, value) VALUES (1, ?)",
        [newValue]
      );

      return { count: newValue, message: `Incremented by ${input.amount}` };
    }),

  // User management using SQLite
  createUser: publicProcedure
    .input(
      z.object({
        name: z.string(),
        email: z.string().email(),
      }),
    )
    .mutation(async ({ input }) => {
      if (!dbInitialized) throw new Error("Database not ready");

      // Ensure users table exists
      await sqliteService.exec(`
        CREATE TABLE IF NOT EXISTS users (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          email TEXT NOT NULL UNIQUE,
          created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
      `);

      const id = Math.random().toString(36).substring(7);
      await sqliteService.exec(
        "INSERT INTO users (id, name, email) VALUES (?, ?, ?)",
        [id, input.name, input.email]
      );

      return {
        id,
        name: input.name,
        email: input.email,
        createdAt: new Date().toISOString(),
      };
    }),

  getUsers: publicProcedure.query(async () => {
    if (!dbInitialized) throw new Error("Database not ready");

    try {
      const results = await sqliteService.query("SELECT * FROM users ORDER BY created_at DESC");
      return results;
    } catch (error) {
      // If table doesn't exist, return empty array
      return [];
    }
  }),

  // SQLite-specific procedures
  getTables: publicProcedure.query(async () => {
    if (!dbInitialized) throw new Error("Database not ready");

    try {
      const results = await sqliteService.query(`
        SELECT name FROM sqlite_master
        WHERE type='table'
        AND name NOT LIKE 'sqlite_%'
        ORDER BY name
      `);
      return results.map((row: any) => row.name);
    } catch (error) {
      console.error("Error getting tables:", error);
      // Return known tables as fallback
      return ['node', '_panorama_tables', 'counter', 'users'];
    }
  }),

  getTableInfo: publicProcedure
    .input(z.object({ tableName: z.string() }))
    .query(async ({ input }) => {
      if (!dbInitialized) throw new Error("Database not ready");

      try {
        const results = await sqliteService.query(`PRAGMA table_info(${input.tableName})`);
        return results;
      } catch (error) {
        console.error("Error getting table info:", error);
        return [];
      }
    }),

  queryTable: publicProcedure
    .input(z.object({
      tableName: z.string(),
      limit: z.number().optional().default(100)
    }))
    .query(async ({ input }) => {
      if (!dbInitialized) throw new Error("Database not ready");

      try {
        const results = await sqliteService.query(
          `SELECT * FROM ${input.tableName} LIMIT ${input.limit}`
        );
        return results;
      } catch (error) {
        console.error("Error querying table:", error);
        return [];
      }
    }),

  executeSql: publicProcedure
    .input(z.object({ sql: z.string() }))
    .mutation(async ({ input }) => {
      if (!dbInitialized) throw new Error("Database not ready");

      try {
        // For SELECT queries, return results
        if (input.sql.trim().toUpperCase().startsWith('SELECT')) {
          const results = await sqliteService.query(input.sql);
          return { type: 'select', results };
        } else {
          // For other queries (INSERT, UPDATE, DELETE, etc.), execute and return success
          await sqliteService.exec(input.sql);
          return { type: 'exec', message: 'Query executed successfully' };
        }
      } catch (error) {
        console.error("Error executing SQL:", error);
        return { type: 'error', message: error instanceof Error ? error.message : 'Unknown error' };
      }
    }),

  // GraphQL endpoint
  graphql: publicProcedure
    .input(z.object({
      query: z.string(),
      variables: z.any().optional()
    }))
    .query(async ({ input }): Promise<any> => {
      if (!dbInitialized || !graphQLExecutor) {
        throw new Error("Database not ready");
      }
      return await graphQLExecutor.execute(input.query, input.variables);
    }),
});

console.log("Service Worker: Starting up...");

try {
  precacheAndRoute(self.__WB_MANIFEST);
  console.log("Service Worker: Precaching complete");
} catch (error) {
  console.error("Service Worker: Precaching failed:", error);
}

console.log("Service Worker: Loaded successfully");

self.addEventListener("install", (event: ExtendableEvent) => {
  console.log("Service Worker: Installing...");
  // Skip waiting to activate immediately
  event.waitUntil(self.skipWaiting());
  console.log("Service Worker: Installed and skipping waiting");
});

self.addEventListener("activate", (event: ExtendableEvent) => {
  console.log("Service Worker: Activating...");
  event.waitUntil(
    self.clients.claim().then(async () => {
      console.log("Service Worker: Activated and claimed all clients");
      // Try to initialize again if not already done
      if (!dbInitialized) {
        console.log("Service Worker: Retrying database initialization on activate...");
        await initDatabase();
      }
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

// Handle tRPC messages directly
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

    // Handle tRPC messages
    if (
      event.data &&
      typeof event.data === "object" &&
      event.data.type === "request" &&
      event.data.id &&
      event.data.path
    ) {
      console.log("Service Worker: Handling tRPC request:", event.data.path);

      try {
        const { id, path, input } = event.data;

        // Create a caller for this request
        const caller = appRouter.createCaller({});

        // Navigate to the procedure and call it
        const pathParts = path.split(".");
        let currentCaller = caller;

        for (let i = 0; i < pathParts.length - 1; i++) {
          currentCaller = (currentCaller as any)[pathParts[i]];
        }

        const procedureName = pathParts[pathParts.length - 1];
        const procedure = (currentCaller as any)[procedureName];

        if (!procedure) {
          throw new Error(`Procedure "${path}" not found`);
        }

        const result = await procedure(input);

        // Send response back
        const response = {
          id,
          type: "response",
          result
        };

        if (event.source) {
          event.source.postMessage(response);
        }
      } catch (error) {
        console.error("Service Worker: Procedure execution error:", error);

        const errorResponse = {
          id: event.data.id,
          type: "error",
          error: {
            message: error instanceof Error ? error.message : "Unknown error",
            code: -32603,
          },
        };

        if (event.source) {
          event.source.postMessage(errorResponse);
        }
      }
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

console.log("Service Worker: tRPC server initialized with procedures:", Object.keys(appRouter._def.procedures));
