#!/usr/bin/env bun
/**
 * Panorama E2E Test Harness.
 *
 * Runs Playwright against an isolated instance of Panorama server with a temporary
 * data directory on a random port. By default, assumes server and plugins are pre-built.
 * Use --build to run the build phase prior to starting the test server.
 */

import { command, flag, option, optional, number, boolean, string, restPositionals, run } from 'cmd-ts';
import { spawn, spawnSync, ChildProcess } from 'child_process';
import { createServer } from 'net';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';

async function findFreePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.listen(0, '127.0.0.1', () => {
      const address = server.address();
      if (typeof address === 'object' && address !== null) {
        const port = address.port;
        server.close(() => resolve(port));
      } else {
        server.close(() => reject(new Error('Failed to get port')));
      }
    });
    server.on('error', reject);
  });
}

async function waitForUrl(url: string, attempts = 60, interval = 500, desc = 'server'): Promise<boolean> {
  for (let i = 0; i < attempts; i++) {
    try {
      const res = await fetch(url, { headers: { 'User-Agent': 'E2E-Harness' } });
      if (res.status >= 200 && res.status < 400) {
        return true;
      }
    } catch {
      // ignore network errors during boot
    }
    await new Promise((r) => setTimeout(r, interval));
  }
  console.error(`  ✗ Timed out waiting for ${desc} at ${url}`);
  return false;
}

function runCommand(cmd: string, args: string[], cwd: string, env?: Record<string, string>): void {
  const result = spawnSync(cmd, args, {
    cwd,
    stdio: 'inherit',
    env: { ...process.env, ...env },
  });
  if (result.status !== 0) {
    throw new Error(`Command failed with status code ${result.status}: ${cmd} ${args.join(' ')}`);
  }
}

const app = command({
  name: 'e2e',
  description: 'Run E2E tests against an isolated Panorama server instance.',
  args: {
    build: flag({
      type: boolean,
      long: 'build',
      defaultValue: () => false,
      description: 'Trigger a full build of frontend, WASM plugins, server, and panoapps before testing.',
    }),
    port: option({
      type: optional(number),
      long: 'port',
      description: 'Port to run the server on (default: random free port).',
    }),
    playwrightArgs: restPositionals({
      type: string,
      displayName: 'playwright args',
      description: 'Extra arguments passed to playwright test.',
    }),
  },
  handler: async (args) => {
    const repoRoot = path.resolve(import.meta.dir, '..');
    process.chdir(repoRoot);

    // Load .env if present
    const envPath = path.join(repoRoot, '.env');
    if (fs.existsSync(envPath)) {
      const content = fs.readFileSync(envPath, 'utf-8');
      for (const line of content.split('\n')) {
        const trimmed = line.trim();
        if (trimmed && !trimmed.startsWith('#') && trimmed.includes('=')) {
          const idx = trimmed.indexOf('=');
          const key = trimmed.slice(0, idx).trim();
          const val = trimmed.slice(idx + 1).trim().replace(/^["']|["']$/g, '');
          if (key && !(key in process.env)) {
            process.env[key] = val;
          }
        }
      }
    }

    const serverPort = args.port ?? (await findFreePort());
    const playwrightArgs = args.playwrightArgs;
    const e2eWorkers = process.env.E2E_WORKERS;

    console.log('=== Panorama E2E Harness ===');
    console.log(`  server   : 127.0.0.1:${serverPort}`);
    if (e2eWorkers) {
      console.log(`  workers  : ${e2eWorkers}`);
    }
    if (playwrightArgs.length > 0) {
      console.log(`  playwright args: ${playwrightArgs.join(' ')}`);
    }
    console.log('');

    // Create temporary data directory
    const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'panorama_e2e_'));
    const pluginsDir = path.join(tempDir, 'plugins');
    fs.mkdirSync(pluginsDir, { recursive: true });

    let serverProcess: ChildProcess | null = null;

    try {
      if (args.build) {
        console.log('--- Packaging .panoapp files via Nx ---');
        runCommand('bun', ['x', 'nx', 'run-many', '-t', 'package-panoapp'], repoRoot);

        console.log('\n--- Building server via Nx ---');
        runCommand('bun', ['x', 'nx', 'build', 'panorama-server'], repoRoot);
        console.log('');
      }

      // Copy .panoapp files into isolated data dir
      const panoappDir = path.join(repoRoot, 'dist', 'panoapp');
      const panoappFiles = fs.existsSync(panoappDir)
        ? fs.readdirSync(panoappDir).filter((f) => f.endsWith('.panoapp'))
        : [];

      if (panoappFiles.length === 0) {
        console.error('  ✗ No .panoapp files found in dist/panoapp/ — build first (e.g. `just build`) or pass --build');
        process.exit(1);
      }

      for (const file of panoappFiles) {
        fs.copyFileSync(path.join(panoappDir, file), path.join(pluginsDir, file));
      }

      console.log(`  Copied ${panoappFiles.length} .panoapp files`);
      console.log('');

      // Check server binary exists (check debug or release)
      const debugBin = path.join(repoRoot, 'target', 'debug', 'panorama-server');
      const releaseBin = path.join(repoRoot, 'target', 'release', 'panorama-server');
      const serverBin = fs.existsSync(debugBin) ? debugBin : releaseBin;

      if (!fs.existsSync(serverBin)) {
        console.error(`  ✗ Server binary not found at ${debugBin} or ${releaseBin} — build first (e.g. \`just build\`) or pass --build`);
        process.exit(1);
      }

      // Start server
      console.log('--- Starting server ---');
      const env = {
        ...process.env,
        PANORAMA_DATA_DIR: tempDir,
        PANORAMA_LISTEN: `127.0.0.1:${serverPort}`,
      };

      serverProcess = spawn(serverBin, [], { env, stdio: 'inherit' });

      const ready = await waitForUrl(
        `http://127.0.0.1:${serverPort}/api/plugins`,
        60,
        500,
        'server /api/plugins'
      );
      if (!ready) {
        process.exit(1);
      }

      console.log(`  ✓ Server ready on port ${serverPort} (pid ${serverProcess.pid})`);
      console.log('');

      // Run E2E tests
      console.log('--- Running E2E tests ---');
      console.log('');

      const playwrightEnv = {
        ...env,
        PLAYWRIGHT_BASE_URL: `http://127.0.0.1:${serverPort}`,
      };

      const hasWorkersArg = playwrightArgs.some((arg) => arg.includes('--workers') || arg.startsWith('-j'));
      const extraPlaywrightArgs: string[] = [];
      if (e2eWorkers && !hasWorkersArg) {
        extraPlaywrightArgs.push(`--workers=${e2eWorkers}`);
      }

      const playwrightCmdArgs = ['x', 'playwright', 'test', '--project=chromium', ...extraPlaywrightArgs, ...playwrightArgs];
      const testResult = spawnSync('bun', playwrightCmdArgs, {
        cwd: path.join(repoRoot, 'frontend'),
        stdio: 'inherit',
        env: playwrightEnv,
      });

      console.log('');
      console.log('=== Tests complete ===');
      process.exit(testResult.status ?? 0);
    } finally {
      if (serverProcess !== null) {
        try {
          serverProcess.kill('SIGTERM');
        } catch {
          try {
            serverProcess.kill('SIGKILL');
          } catch {
            // ignore
          }
        }
      }
      try {
        fs.rmSync(tempDir, { recursive: true, force: true });
      } catch {
        // ignore cleanup errors
      }
    }
  },
});

run(app, process.argv.slice(2));
