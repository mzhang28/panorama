import { useState, useEffect, useCallback } from "react";
import { trpc, serviceWorkerUtils } from "../lib/trpc";

// Generic hook for tRPC queries with service worker readiness check
export function useTRPCQuery<T>(
  procedure: () => Promise<T>,
  options: {
    enabled?: boolean;
    refetchInterval?: number;
    onSuccess?: (data: T) => void;
    onError?: (error: Error) => void;
    requireServiceWorker?: boolean;
  } = {},
) {
  const [data, setData] = useState<T | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [isSuccess, setIsSuccess] = useState(false);
  const [isServiceWorkerReady, setIsServiceWorkerReady] = useState(false);

  const {
    enabled = true,
    refetchInterval,
    onSuccess,
    onError,
    requireServiceWorker = true,
  } = options;

  // Check service worker status
  useEffect(() => {
    if (!requireServiceWorker) {
      setIsServiceWorkerReady(true);
      return;
    }

    const checkServiceWorker = async () => {
      try {
        if (!serviceWorkerUtils.isAvailable()) {
          setError(new Error("Service Worker not supported"));
          return;
        }

        await serviceWorkerUtils.waitForServiceWorker();
        const isReady = await serviceWorkerUtils.ping();
        setIsServiceWorkerReady(isReady);

        if (!isReady) {
          setError(new Error("Service Worker is not responding"));
        }
      } catch (err) {
        console.error("Service Worker check failed:", err);
        setError(
          err instanceof Error ? err : new Error("Service Worker check failed"),
        );
      }
    };

    checkServiceWorker();
  }, [requireServiceWorker]);

  const refetch = useCallback(async () => {
    if (!enabled) return;
    if (requireServiceWorker && !isServiceWorkerReady) {
      setError(new Error("Service Worker not ready"));
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      const result = await procedure();
      setData(result);
      setIsSuccess(true);
      onSuccess?.(result);
    } catch (err) {
      const error = err instanceof Error ? err : new Error("Unknown error");
      console.error("tRPC query error:", error);
      setError(error);
      setIsSuccess(false);
      onError?.(error);
    } finally {
      setIsLoading(false);
    }
  }, [
    procedure,
    enabled,
    onSuccess,
    onError,
    requireServiceWorker,
    isServiceWorkerReady,
  ]);

  useEffect(() => {
    if (enabled && (!requireServiceWorker || isServiceWorkerReady)) {
      refetch();
    }
  }, [refetch, enabled, isServiceWorkerReady, requireServiceWorker]);

  useEffect(() => {
    if (
      refetchInterval &&
      enabled &&
      (!requireServiceWorker || isServiceWorkerReady)
    ) {
      const interval = setInterval(refetch, refetchInterval);
      return () => clearInterval(interval);
    }
  }, [
    refetch,
    refetchInterval,
    enabled,
    isServiceWorkerReady,
    requireServiceWorker,
  ]);

  return {
    data,
    isLoading,
    error,
    isSuccess,
    isServiceWorkerReady,
    refetch,
  };
}

// Generic hook for tRPC mutations with service worker readiness check
export function useTRPCMutation<TInput, TOutput>(
  procedure: (input: TInput) => Promise<TOutput>,
  options: {
    onSuccess?: (data: TOutput, variables: TInput) => void;
    onError?: (error: Error, variables: TInput) => void;
    onSettled?: (
      data: TOutput | null,
      error: Error | null,
      variables: TInput,
    ) => void;
    requireServiceWorker?: boolean;
  } = {},
) {
  const [data, setData] = useState<TOutput | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [isSuccess, setIsSuccess] = useState(false);
  const [isServiceWorkerReady, setIsServiceWorkerReady] = useState(false);

  const {
    onSuccess,
    onError,
    onSettled,
    requireServiceWorker = true,
  } = options;

  // Check service worker status
  useEffect(() => {
    if (!requireServiceWorker) {
      setIsServiceWorkerReady(true);
      return;
    }

    const checkServiceWorker = async () => {
      try {
        if (!serviceWorkerUtils.isAvailable()) {
          setError(new Error("Service Worker not supported"));
          return;
        }

        await serviceWorkerUtils.waitForServiceWorker();
        const isReady = await serviceWorkerUtils.ping();
        setIsServiceWorkerReady(isReady);

        if (!isReady) {
          setError(new Error("Service Worker is not responding"));
        }
      } catch (err) {
        console.error("Service Worker check failed:", err);
        setError(
          err instanceof Error ? err : new Error("Service Worker check failed"),
        );
      }
    };

    checkServiceWorker();
  }, [requireServiceWorker]);

  const mutate = useCallback(
    async (variables: TInput) => {
      if (requireServiceWorker && !isServiceWorkerReady) {
        const error = new Error("Service Worker not ready");
        setError(error);
        onError?.(error, variables);
        onSettled?.(null, error, variables);
        throw error;
      }

      setIsLoading(true);
      setError(null);

      try {
        const result = await procedure(variables);
        setData(result);
        setIsSuccess(true);
        onSuccess?.(result, variables);
        onSettled?.(result, null, variables);
        return result;
      } catch (err) {
        const error = err instanceof Error ? err : new Error("Unknown error");
        console.error("tRPC mutation error:", error);
        setError(error);
        setIsSuccess(false);
        onError?.(error, variables);
        onSettled?.(null, error, variables);
        throw error;
      } finally {
        setIsLoading(false);
      }
    },
    [
      procedure,
      onSuccess,
      onError,
      onSettled,
      requireServiceWorker,
      isServiceWorkerReady,
    ],
  );

  const reset = useCallback(() => {
    setData(null);
    setError(null);
    setIsSuccess(false);
    setIsLoading(false);
  }, []);

  return {
    mutate,
    data,
    isLoading,
    error,
    isSuccess,
    isServiceWorkerReady,
    reset,
  };
}

// Specific hooks for common queries
export function useHealth() {
  return useTRPCQuery(
    async () => {
      console.log("Calling health check...");
      const result = await trpc.health.query();
      console.log("Health check result:", result);
      return result;
    },
    {
      refetchInterval: 30000, // Refetch every 30 seconds
      onError: (error) => {
        console.error("Health check failed:", error);
      },
    },
  );
}

export function useCounter() {
  return useTRPCQuery(
    async () => {
      console.log("Calling getCounter...");
      const result = await trpc.getCounter.query();
      console.log("Counter result:", result);
      return result;
    },
    {
      onError: (error) => {
        console.error("Get counter failed:", error);
      },
    },
  );
}

export function useUsers() {
  return useTRPCQuery(
    async () => {
      console.log("Calling getUsers...");
      const result = await trpc.getUsers.query();
      console.log("Users result:", result);
      return result;
    },
    {
      onError: (error) => {
        console.error("Get users failed:", error);
      },
    },
  );
}

// Specific hooks for mutations
export function useIncrementCounter() {
  return useTRPCMutation(
    async (input: { amount?: number }) => {
      console.log("Calling incrementCounter with:", input);
      const result = await trpc.incrementCounter.mutate(input);
      console.log("Increment counter result:", result);
      return result;
    },
    {
      onError: (error, variables) => {
        console.error(
          "Increment counter failed:",
          error,
          "Variables:",
          variables,
        );
      },
    },
  );
}

export function useCreateUser() {
  return useTRPCMutation(
    async (input: { name: string; email: string }) => {
      console.log("Calling createUser with:", input);
      const result = await trpc.createUser.mutate(input);
      console.log("Create user result:", result);
      return result;
    },
    {
      onError: (error, variables) => {
        console.error("Create user failed:", error, "Variables:", variables);
      },
    },
  );
}

export function useEcho() {
  return useTRPCMutation(
    async (input: { message: string }) => {
      console.log("Calling echo with:", input);
      const result = await trpc.echo.query(input);
      console.log("Echo result:", result);
      return result;
    },
    {
      onError: (error, variables) => {
        console.error("Echo failed:", error, "Variables:", variables);
      },
    },
  );
}
