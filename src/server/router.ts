import { initTRPC } from "@trpc/server";
import { z } from "zod";

// Initialize tRPC
console.log("Initializing tRPC...");
const t = initTRPC.create();

if (!t || !t.router || !t.procedure) {
  throw new Error(
    "Failed to initialize tRPC - t, t.router, or t.procedure is undefined",
  );
}

// Export reusable router and procedure helpers
export const router = t.router;
export const publicProcedure = t.procedure;
export const createCallerFactory = t.createCallerFactory;

console.log("tRPC initialized successfully, creating router...");

// Define the main app router
export const appRouter = router({
  // Health check procedure
  health: publicProcedure.query(() => {
    return { status: "ok", timestamp: new Date().toISOString() };
  }),

  // Echo procedure that returns the input
  echo: publicProcedure
    .input(z.object({ message: z.string() }))
    .query(({ input }) => {
      return { echo: input.message };
    }),

  // Counter procedures
  getCounter: publicProcedure.query(() => {
    // In a real app, this would come from a database or state store
    return { count: 0 };
  }),

  incrementCounter: publicProcedure
    .input(z.object({ amount: z.number().default(1) }))
    .mutation(({ input }) => {
      // In a real app, this would update a database or state store
      return { count: input.amount, message: `Incremented by ${input.amount}` };
    }),

  // User management example
  createUser: publicProcedure
    .input(
      z.object({
        name: z.string(),
        email: z.string().email(),
      }),
    )
    .mutation(({ input }) => {
      // In a real app, this would save to a database
      return {
        id: Math.random().toString(36).substring(7),
        name: input.name,
        email: input.email,
        createdAt: new Date().toISOString(),
      };
    }),

  getUsers: publicProcedure.query(() => {
    // In a real app, this would query a database
    return [
      {
        id: "1",
        name: "John Doe",
        email: "john@example.com",
        createdAt: "2024-01-01T00:00:00.000Z",
      },
    ];
  }),
});

// Validate the router was created properly
if (!appRouter) {
  throw new Error("appRouter is undefined after creation");
}

if (!appRouter._def) {
  throw new Error(
    "appRouter._def is undefined - router not properly initialized",
  );
}

if (!appRouter._def.procedures) {
  throw new Error(
    "appRouter._def.procedures is undefined - procedures not found",
  );
}

console.log(
  "App router created successfully with procedures:",
  Object.keys(appRouter._def.procedures),
);

// Create caller factory for the app router
export const createCaller = createCallerFactory(appRouter);

// Export type definition of API
export type AppRouter = typeof appRouter;
