import * as fs from "node:fs";
import * as path from "node:path";
import { defineConfig, devices } from "@playwright/test";

if (!process.env.E2E_WORKERS) {
  const envDir = import.meta.dirname ?? path.resolve(process.cwd(), "..");
  const rootEnv = path.resolve(envDir, "../.env");
  if (fs.existsSync(rootEnv)) {
    const lines = fs.readFileSync(rootEnv, "utf-8").split("\n");
    for (const line of lines) {
      const trimmed = line.trim();
      if (trimmed && !trimmed.startsWith("#") && trimmed.includes("=")) {
        const [k, ...v] = trimmed.split("=");
        const key = k.trim();
        const val = v
          .join("=")
          .trim()
          .replace(/^["']|["']$/g, "");
        if (key && !(key in process.env)) {
          process.env[key] = val;
        }
      }
    }
  }
}

const e2eWorkers = process.env.E2E_WORKERS
  ? parseInt(process.env.E2E_WORKERS, 10)
  : undefined;

export default defineConfig({
  testDir: "./e2e",
  globalSetup: "./e2e/global-setup.ts",
  timeout: 60_000,
  expect: { timeout: 30_000 },
  fullyParallel: false,
  retries: 0,
  workers: e2eWorkers,
  reporter: "list",
  use: {
    baseURL: process.env.PLAYWRIGHT_BASE_URL || "http://localhost:5173",
    viewport: { width: 1280, height: 720 },
    trace: "on",
    screenshot: "only-on-failure",
    video: "on",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  // Don't auto-start webServer — tests expect externally running server+frontend
});
