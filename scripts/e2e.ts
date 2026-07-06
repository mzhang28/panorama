#!/usr/bin/env bun
/**
 * Panorama E2E Test Harness.
 *
 * Prepares pre-built artifacts (via --build or verification) and runs Playwright.
 * Each Playwright test job automatically spawns its own isolated Panorama server instance
 * via scripts/instance.ts fixtures.
 */

import {
  command,
  flag,
  option,
  optional,
  number,
  boolean,
  string,
  restPositionals,
  run,
} from "cmd-ts";
import { spawnSync } from "child_process";
import * as fs from "fs";
import * as path from "path";

function runCommand(
  cmd: string,
  args: string[],
  cwd: string,
  env?: Record<string, string>,
): void {
  const result = spawnSync(cmd, args, {
    cwd,
    stdio: "inherit",
    env: { ...process.env, ...env },
  });
  if (result.status !== 0) {
    throw new Error(
      `Command failed with status code ${result.status}: ${cmd} ${args.join(" ")}`,
    );
  }
}

const app = command({
  name: "e2e",
  description: "Run E2E tests against isolated Panorama server instances.",
  args: {
    build: flag({
      type: boolean,
      long: "build",
      defaultValue: () => false,
      description:
        "Trigger a full build of frontend, WASM plugins, server, and panoapps before testing.",
    }),
    config: option({
      type: optional(string),
      long: "config",
      short: "c",
      defaultValue: () => "release",
      description:
        "Build configuration to use with --build (development or release).",
    }),
    playwrightArgs: restPositionals({
      type: string,
      displayName: "playwright args",
      description: "Extra arguments passed to playwright test.",
    }),
  },
  handler: async (args) => {
    const repoRoot = path.resolve(import.meta.dir, "..");
    process.chdir(repoRoot);

    // Load .env if present
    const envPath = path.join(repoRoot, ".env");
    if (fs.existsSync(envPath)) {
      const content = fs.readFileSync(envPath, "utf-8");
      for (const line of content.split("\n")) {
        const trimmed = line.trim();
        if (trimmed && !trimmed.startsWith("#") && trimmed.includes("=")) {
          const idx = trimmed.indexOf("=");
          const key = trimmed.slice(0, idx).trim();
          const val = trimmed
            .slice(idx + 1)
            .trim()
            .replace(/^["']|["']$/g, "");
          if (key && !(key in process.env)) {
            process.env[key] = val;
          }
        }
      }
    }

    const playwrightArgs = args.playwrightArgs;
    const e2eWorkers = process.env.E2E_WORKERS;

    console.log("=== Panorama E2E Harness ===");
    if (e2eWorkers) {
      console.log(`  workers  : ${e2eWorkers}`);
    }
    if (playwrightArgs.length > 0) {
      console.log(`  playwright args: ${playwrightArgs.join(" ")}`);
    }
    console.log("");

    if (args.build) {
      const buildConfig = args.config ?? "release";
      console.log(
        `--- Packaging .panoapp files (${buildConfig} mode) via Nx ---`,
      );
      runCommand(
        "bun",
        ["x", "nx", "run-many", "-t", "package-panoapp", "-c", buildConfig],
        repoRoot,
      );

      console.log(`\n--- Building server (${buildConfig} mode) via Nx ---`);
      runCommand(
        "bun",
        ["x", "nx", "build", "panorama-server", "-c", buildConfig],
        repoRoot,
      );
      console.log("");
    }

    // Verify .panoapp files exist
    const panoappDir = path.join(repoRoot, "dist", "panoapp");
    const panoappFiles = fs.existsSync(panoappDir)
      ? fs.readdirSync(panoappDir).filter((f) => f.endsWith(".panoapp"))
      : [];

    if (panoappFiles.length === 0) {
      console.error(
        "  ✗ No .panoapp files found in dist/panoapp/ — build first (e.g. `just build`) or pass --build",
      );
      process.exit(1);
    }

    // Verify server binary exists (debug or release)
    const debugBin = path.join(repoRoot, "target", "debug", "panorama-server");
    const releaseBin = path.join(
      repoRoot,
      "target",
      "release",
      "panorama-server",
    );
    if (!fs.existsSync(debugBin) && !fs.existsSync(releaseBin)) {
      console.error(
        `  ✗ Server binary not found at ${debugBin} or ${releaseBin} — build first (e.g. \`just build\`) or pass --build`,
      );
      process.exit(1);
    }

    // Run E2E tests via Playwright
    console.log("--- Running E2E tests ---");
    console.log("");

    const hasWorkersArg = playwrightArgs.some(
      (arg) => arg.includes("--workers") || arg.startsWith("-j"),
    );
    const extraPlaywrightArgs: string[] = [];
    if (!hasWorkersArg) {
      extraPlaywrightArgs.push(`--workers=${e2eWorkers || 1}`);
    }

    const playwrightCmdArgs = [
      "x",
      "playwright",
      "test",
      "--project=chromium",
      ...extraPlaywrightArgs,
      ...playwrightArgs,
    ];
    const testResult = spawnSync("bun", playwrightCmdArgs, {
      cwd: path.join(repoRoot, "frontend"),
      stdio: "inherit",
      env: {
        ...process.env,
      },
    });

    console.log("");
    console.log("=== Tests complete ===");
    process.exit(testResult.status ?? 0);
  },
});

run(app, process.argv.slice(2));
