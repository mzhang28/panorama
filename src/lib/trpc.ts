import { createTRPCProxyClient } from "@trpc/client";
import { observable } from "@trpc/server/observable";
import type { AppRouter } from "../server";

// Custom link for communicating with service worker via postMessage
function createServiceWorkerLink() {
  return () => {
    return ({ op }: { op: any }) => {
      return observable((observer) => {
        // Get the service worker
        const controller = navigator.serviceWorker.controller;
        if (!controller) {
          observer.error(new Error("No service worker controller available"));
          return;
        }

        const id = Math.random().toString(36).substring(7);

        // Set up message handler
        const messageHandler = (event: MessageEvent) => {
          const message = event.data;

          if (!message || !message.id || message.id !== id) {
            return;
          }

          // Remove the event listener
          navigator.serviceWorker.removeEventListener(
            "message",
            messageHandler,
          );

          if (message.type === "response") {
            observer.next({ result: { data: message.result } });
            observer.complete();
          } else if (message.type === "error") {
            observer.error(
              new Error(message.error?.message || "Unknown error"),
            );
          }
        };

        // Add message listener
        navigator.serviceWorker.addEventListener("message", messageHandler);

        // Send the request
        const message = {
          id,
          type: "request",
          path: op.path,
          input: op.input,
        };

        controller.postMessage(message);

        // Add timeout
        const timeoutId = setTimeout(() => {
          navigator.serviceWorker.removeEventListener(
            "message",
            messageHandler,
          );
          observer.error(new Error("Request timeout"));
        }, 30000);

        // Return cleanup function
        return () => {
          clearTimeout(timeoutId);
          navigator.serviceWorker.removeEventListener(
            "message",
            messageHandler,
          );
        };
      });
    };
  };
}

// Create the tRPC client
export const trpc = createTRPCProxyClient<AppRouter>({
  links: [createServiceWorkerLink()],
});

// Hook for React components to use tRPC easily
export function useTRPC() {
  return trpc;
}

// Service worker utilities
export const serviceWorkerUtils = {
  // Check if service worker is available
  isAvailable(): boolean {
    return "serviceWorker" in navigator;
  },

  // Wait for service worker to be ready
  async waitForServiceWorker(): Promise<ServiceWorkerRegistration> {
    if (!this.isAvailable()) {
      throw new Error("Service Worker not supported");
    }

    return navigator.serviceWorker.ready;
  },

  // Send a ping to test communication
  async ping(): Promise<boolean> {
    try {
      if (!navigator.serviceWorker.controller) {
        return false;
      }

      return new Promise((resolve) => {
        const messageHandler = (event: MessageEvent) => {
          if (event.data?.type === "pong") {
            navigator.serviceWorker.removeEventListener(
              "message",
              messageHandler,
            );
            resolve(true);
          }
        };

        navigator.serviceWorker.addEventListener("message", messageHandler);
        if (navigator.serviceWorker.controller) {
          navigator.serviceWorker.controller.postMessage({ type: "ping" });
        } else {
          resolve(false);
          return;
        }

        setTimeout(() => {
          navigator.serviceWorker.removeEventListener(
            "message",
            messageHandler,
          );
          resolve(false);
        }, 5000);
      });
    } catch (error) {
      console.error("Service worker ping failed:", error);
      return false;
    }
  },
};
