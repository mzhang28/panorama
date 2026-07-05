#!/usr/bin/env bun
/**
 * Panorama Instance Launcher.
 *
 * Spawns an isolated instance of Panorama server with a temporary
 * data directory on a random port, loading pre-built .panoapp plugins.
 */

import { spawn, ChildProcess } from 'child_process';
import { createServer } from 'net';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

export interface ServerInstance {
  port: number;
  url: string;
  dataDir: string;
  pid: number;
  stop: () => Promise<void>;
}

export async function findFreePort(): Promise<number> {
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

export async function waitForUrl(url: string, attempts = 150, interval = 100, desc = 'server'): Promise<boolean> {
  for (let i = 0; i < attempts; i++) {
    try {
      const res = await fetch(url, { headers: { 'User-Agent': 'E2E-Instance' } });
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

export async function spawnInstance(customRepoRoot?: string): Promise<ServerInstance> {
  const repoRoot = customRepoRoot ?? path.resolve(__dirname, '..');
  const serverPort = await findFreePort();

  // Create temporary data directory
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'panorama_instance_'));
  const pluginsDir = path.join(tempDir, 'plugins');
  fs.mkdirSync(pluginsDir, { recursive: true });

  // Copy .panoapp files into isolated data dir
  const panoappDir = path.join(repoRoot, 'dist', 'panoapp');
  if (fs.existsSync(panoappDir)) {
    const panoappFiles = fs.readdirSync(panoappDir).filter((f) => f.endsWith('.panoapp'));
    for (const file of panoappFiles) {
      fs.copyFileSync(path.join(panoappDir, file), path.join(pluginsDir, file));
    }
  }

  // Check server binary exists (debug or release)
  const releaseBin = path.join(repoRoot, 'target', 'release', 'panorama-server');
  const debugBin = path.join(repoRoot, 'target', 'debug', 'panorama-server');
  const serverBin = fs.existsSync(releaseBin) ? releaseBin : debugBin;

  if (!fs.existsSync(serverBin)) {
    throw new Error(`Server binary not found at ${debugBin} or ${releaseBin} — build first (e.g. \`just build\`)`);
  }

  const env = {
    ...process.env,
    PANORAMA_DATA_DIR: tempDir,
    PANORAMA_LISTEN: `127.0.0.1:${serverPort}`,
  };

  const serverProcess: ChildProcess = spawn(serverBin, [], { env, stdio: ['ignore', 'pipe', 'pipe'] });
  let serverLogs = '';
  serverProcess.stdout?.on('data', (chunk) => { serverLogs += chunk.toString(); });
  serverProcess.stderr?.on('data', (chunk) => { serverLogs += chunk.toString(); });

  const url = `http://127.0.0.1:${serverPort}`;
  const ready = await waitForUrl(`${url}/api/plugins`, 150, 100, `instance on port ${serverPort}`);

  if (!ready) {
    if (serverLogs) {
      console.error(`--- Server logs for port ${serverPort} ---\n${serverLogs}\n--- End server logs ---`);
    }
    serverProcess.kill('SIGKILL');
    fs.rmSync(tempDir, { recursive: true, force: true });
    throw new Error(`Server failed to start on port ${serverPort}`);
  }

  const stop = async () => {
    try {
      serverProcess.kill('SIGTERM');
    } catch {
      try {
        serverProcess.kill('SIGKILL');
      } catch {
        // ignore
      }
    }
    try {
      fs.rmSync(tempDir, { recursive: true, force: true });
    } catch {
      // ignore
    }
  };

  return {
    port: serverPort,
    url,
    dataDir: tempDir,
    pid: serverProcess.pid!,
    stop,
  };
}

// CLI entrypoint if executed directly
if (process.argv[1] && path.resolve(process.argv[1]) === __filename) {
  const command = process.argv[2] || 'start';

  if (command === 'start') {
    const instance = await spawnInstance();
    console.log(JSON.stringify({
      port: instance.port,
      url: instance.url,
      dataDir: instance.dataDir,
      pid: instance.pid,
    }));
  } else if (command === 'stop') {
    const pid = parseInt(process.argv[3], 10);
    const dataDir = process.argv[4];
    if (pid) {
      try { process.kill(pid, 'SIGTERM'); } catch {}
    }
    if (dataDir && fs.existsSync(dataDir)) {
      try { fs.rmSync(dataDir, { recursive: true, force: true }); } catch {}
    }
  }
}
