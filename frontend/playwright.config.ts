import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  timeout: 60_000,
  expect: { timeout: 10_000 },
  fullyParallel: false,
  retries: 1,
  workers: 1,
  reporter: 'list',

  use: {
    baseURL: 'http://localhost:5173',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },

  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],

  // Start the dev servers before tests
  webServer: [
    {
      command: 'cd ../ && cargo run --bin panorama-server',
      port: 3000,
      timeout: 60_000,
      reuseExistingServer: true,
      env: {
        PANORAMA_LISTEN: '127.0.0.1:3000',
        PANORAMA_DATA_DIR: '/tmp/panorama-test-data',
      },
    },
    {
      command: 'npm run dev',
      port: 5173,
      timeout: 30_000,
      reuseExistingServer: true,
    },
  ],
});
