// Type definitions for the tRPC API
// The actual implementation is in the service worker

import { initTRPC } from "@trpc/server";
import { z } from "zod";

// Initialize tRPC for type definitions only
const t = initTRPC.create();

// Export reusable router and procedure helpers for types
export const router = t.router;
export const publicProcedure = t.procedure;

// Define the type-only app router (implementation is in service worker)
export const appRouter = router({
  // Health check procedure
  health: publicProcedure.query(() => {
    return { status: "ok" as const, timestamp: "" as string, dbReady: true as boolean };
  }),

  // Echo procedure that returns the input
  echo: publicProcedure
    .input(z.object({ message: z.string() }))
    .query(({ input }) => {
      return { echo: input.message };
    }),

  // Counter procedures
  getCounter: publicProcedure.query(() => {
    return { count: 0 as number };
  }),

  incrementCounter: publicProcedure
    .input(z.object({ amount: z.number().default(1) }))
    .mutation(({ input }) => {
      return { count: 0 as number, message: "" as string };
    }),

  // User management
  createUser: publicProcedure
    .input(
      z.object({
        name: z.string(),
        email: z.string().email(),
      }),
    )
    .mutation(({ input }) => {
      return {
        id: "" as string,
        name: input.name,
        email: input.email,
        createdAt: "" as string,
      };
    }),

  getUsers: publicProcedure.query(() => {
    return [] as Array<{
      id: string;
      name: string;
      email: string;
      created_at?: string;
    }>;
  }),

  // SQLite-specific procedures
  getTables: publicProcedure.query(() => {
    return [] as string[];
  }),

  getTableInfo: publicProcedure
    .input(z.object({ tableName: z.string() }))
    .query(({ input }) => {
      return [] as Array<{
        cid: number;
        name: string;
        type: string;
        notnull: number;
        dflt_value: any;
        pk: number;
      }>;
    }),

  queryTable: publicProcedure
    .input(z.object({
      tableName: z.string(),
      limit: z.number().optional().default(100)
    }))
    .query(({ input }) => {
      return [] as any[];
    }),

  executeSql: publicProcedure
    .input(z.object({ sql: z.string() }))
    .mutation(({ input }) => {
      return { type: "select" as "select" | "exec", results: [] as any[], message: "" as string };
    }),

  // GraphQL endpoint
  graphql: publicProcedure
    .input(z.object({
      query: z.string(),
      variables: z.any().optional()
    }))
    .query((): any => {
      return {};
    }),
});

// Export type definition of API
export type AppRouter = typeof appRouter;
