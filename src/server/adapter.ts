import { TRPCError } from "@trpc/server";
import { type AnyTRPCRouter, getTRPCErrorFromUnknown } from "@trpc/server";
import { createCaller } from "./router";

export interface PostMessageAdapterOptions<TRouter extends AnyTRPCRouter> {
  router: TRouter;
  createContext?: () => any;
  onError?: (opts: {
    error: TRPCError;
    type: "query" | "mutation" | "subscription" | "unknown";
    path: string | undefined;
    input: unknown;
    ctx: undefined;
    req: MessageEvent;
  }) => void;
}

export interface TRPCMessage {
  id: string;
  type: "request" | "response" | "error";
  path?: string;
  input?: any;
  result?: any;
  error?: {
    message: string;
    code: number;
    data?: any;
  };
}

export function createPostMessageAdapter<TRouter extends AnyTRPCRouter>(
  opts: PostMessageAdapterOptions<TRouter>,
) {
  const { router, createContext, onError } = opts;

  // Validate that router is properly initialized
  if (!router) {
    throw new Error("Router is undefined - cannot create adapter");
  }

  if (!router._def) {
    throw new Error(
      "Router._def is undefined - router may not be properly initialized",
    );
  }

  if (!router._def.procedures) {
    throw new Error(
      "Router._def.procedures is undefined - router procedures not found",
    );
  }

  console.log(
    "Adapter created with router procedures:",
    Object.keys(router._def.procedures),
  );

  // Use the exported createCaller factory
  console.log("Using createCaller factory from router module");

  return {
    handleMessage: async (event: MessageEvent<TRPCMessage>) => {
      const { data: message } = event;

      // Validate message structure
      if (
        !message ||
        typeof message !== "object" ||
        !message.id ||
        message.type !== "request"
      ) {
        return;
      }

      const { id, path, input } = message;

      try {
        if (!path) {
          throw new TRPCError({
            code: "BAD_REQUEST",
            message: "Missing procedure path",
          });
        }

        const ctx = createContext?.() ?? {};

        // Check if procedure exists
        const procedure = router._def.procedures[path];
        if (!procedure) {
          throw new TRPCError({
            code: "NOT_FOUND",
            message: `Procedure "${path}" not found`,
          });
        }

        let result;

        try {
          // Use the exported createCaller factory (tRPC v11 approach)
          const caller = createCaller(ctx);

          // Navigate to the procedure by path (e.g., "health" or "post.list")
          const pathParts = path.split(".");
          let currentCaller = caller;

          for (let i = 0; i < pathParts.length - 1; i++) {
            const segment = pathParts[i];
            if (!segment) {
              throw new Error(`Empty path segment in ${path}`);
            }
            currentCaller = (currentCaller as any)[segment];
            if (!currentCaller) {
              throw new Error(`Invalid path segment: ${segment} in ${path}`);
            }
          }

          const procedureName = pathParts[pathParts.length - 1];
          if (!procedureName) {
            throw new Error(`Empty procedure name in ${path}`);
          }
          const procedureCall = (currentCaller as any)[procedureName];

          if (!procedureCall) {
            throw new Error(`Procedure not found: ${path}`);
          }

          // Call the procedure
          result = await procedureCall(input);
        } catch (callerError) {
          console.error("tRPC caller error:", callerError);
          throw new TRPCError({
            code: "INTERNAL_SERVER_ERROR",
            message: `Failed to call procedure "${path}": ${callerError instanceof Error ? callerError.message : String(callerError)}`,
          });
        }

        // Send response back to main thread
        const response: TRPCMessage = {
          id,
          type: "response",
          result,
        };

        // Send response back to the specific client that made the request
        if (event.source) {
          event.source.postMessage(response);
        } else if (typeof self !== "undefined" && "clients" in self) {
          // Fallback: send to all clients
          const clients = await (self as any).clients.matchAll();
          clients.forEach((client: any) => {
            client.postMessage(response);
          });
        }
      } catch (cause) {
        const error = getTRPCErrorFromUnknown(cause);

        onError?.({
          error,
          type: "unknown",
          path,
          input,
          ctx: undefined,
          req: event,
        });

        // Send error response back
        const errorResponse: TRPCMessage = {
          id,
          type: "error",
          error: {
            message: error.message,
            code: error.code as any,
            data: error.cause,
          },
        };

        // Send error response back to the specific client that made the request
        if (event.source) {
          event.source.postMessage(errorResponse);
        } else if (typeof self !== "undefined" && "clients" in self) {
          // Fallback: send to all clients
          const clients = await (self as any).clients.matchAll();
          clients.forEach((client: any) => {
            client.postMessage(errorResponse);
          });
        }
      }
    },
  };
}

// Helper function to get TRPC error codes
export function getTRPCErrorCode(error: any): number {
  if (error?.code === "PARSE_ERROR") return -32700;
  if (error?.code === "BAD_REQUEST") return -32600;
  if (error?.code === "INTERNAL_SERVER_ERROR") return -32603;
  if (error?.code === "NOT_FOUND") return -32601;
  if (error?.code === "METHOD_NOT_SUPPORTED") return -32601;
  if (error?.code === "UNAUTHORIZED") return -32001;
  if (error?.code === "FORBIDDEN") return -32003;
  if (error?.code === "TIMEOUT") return -32002;
  if (error?.code === "CONFLICT") return -32004;
  if (error?.code === "PRECONDITION_FAILED") return -32005;
  if (error?.code === "PAYLOAD_TOO_LARGE") return -32006;
  if (error?.code === "UNPROCESSABLE_CONTENT") return -32007;
  if (error?.code === "TOO_MANY_REQUESTS") return -32008;
  if (error?.code === "CLIENT_CLOSED_REQUEST") return -32009;
  return -32603; // INTERNAL_SERVER_ERROR
}
