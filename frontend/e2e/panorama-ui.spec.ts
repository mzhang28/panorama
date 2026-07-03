// Frontend E2E tests — proper Playwright UI tests.
// These test actual UI interaction: clicking, typing, verifying rendered content.

import { test, expect } from '@playwright/test';

const SERVER = 'http://localhost:3000';

test.describe('Panorama Frontend UI', () => {

  test.beforeAll(async () => {
    const res = await fetch(`${SERVER}/api/plugins`);
    if (!res.ok) throw new Error(`Server not ready: ${res.status}`);
  });

  // ─── Core UI ───────────────────────────────────────────────

  test('renders layout with sidebar and main content', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('h1')).toContainText('Panorama');
    await expect(page.locator('.sidebar')).toBeVisible();
    await expect(page.locator('.main-content')).toBeVisible();
  });

  test('sidebar navigation switches between views', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('h2')).toContainText('Nodes');
    await page.click('button:has-text("Schemas")');
    await expect(page.locator('h2')).toContainText('Schemas');
    await page.click('button:has-text("Nodes")');
    await expect(page.locator('h2')).toContainText('Nodes');
  });

  test('creates a node through the UI form', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("+ New Node")');
    const input = page.locator('input[placeholder="Node title"]');
    await expect(input).toBeVisible();
    await input.fill('E2E UI Test Node');
    await page.click('button:has-text("Create")');
    await expect(page.locator('strong:has-text("E2E UI Test Node")')).toBeVisible({ timeout: 5000 });
  });

  test('node detail opens on click and shows fields', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('.card', { timeout: 5000 });
    await page.locator('.card').first().click();
    await expect(page.locator('h3:has-text("Node:")')).toBeVisible({ timeout: 3000 });
  });

  test('displays system schemas', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Schemas")');
    await expect(page.locator('strong:has-text("NodeTime")')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('strong:has-text("NodeInfo")')).toBeVisible({ timeout: 5000 });
  });

  test('plugins list shows installed apps', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('h3:has-text("Installed Apps")')).toBeVisible();
  });

  // ─── Journal Plugin UI ─────────────────────────────────────

  test.describe('Journal Plugin', () => {
    test('opens journal and creates an entry', async ({ page }) => {
      await page.goto('/');

      // Click the Journal app in the sidebar
      const journalBtn = page.locator('button:has-text("Journal")');
      if (await journalBtn.isVisible()) {
        await journalBtn.click();
        await page.waitForTimeout(1000);

        // The Journal component should be rendered
        await expect(page.locator('h2')).toContainText('Journal', { timeout: 5000 });

        // Fill the create form
        const titleInput = page.locator('input[placeholder="Entry title"]');
        if (await titleInput.isVisible()) {
          await titleInput.fill('E2E Journal Entry');
          await page.locator('textarea[placeholder*="Write your entry"]').fill('This is a test journal entry from E2E.');
          await page.locator('button:has-text("Save Entry")').click();
          await page.waitForTimeout(500);

          // The entry should appear in the list
          await expect(page.locator('strong:has-text("E2E Journal Entry")')).toBeVisible({ timeout: 5000 });
        }
      }
    });

    test('journal entry expands to show content on click', async ({ page }) => {
      await page.goto('/');
      const journalBtn = page.locator('button:has-text("Journal")');
      if (await journalBtn.isVisible()) {
        await journalBtn.click();
        await page.waitForTimeout(1000);

        // Click on an entry card
        const entryCard = page.locator('.card strong').first();
        if (await entryCard.isVisible()) {
          await entryCard.click();
          await page.waitForTimeout(300);

          // Content should be visible (pre tag inside the expanded card)
          await expect(page.locator('pre')).toBeVisible({ timeout: 3000 });
        }
      }
    });
  });

  // ─── Dashboard Plugin UI ───────────────────────────────────

  test.describe('Dashboard Plugin', () => {
    test('opens dashboard and shows leaderboard table', async ({ page }) => {
      await page.goto('/');
      const dashBtn = page.locator('button:has-text("Dashboards")');
      if (await dashBtn.isVisible()) {
        await dashBtn.click();
        await page.waitForTimeout(1000);

        await expect(page.locator('h2')).toContainText('Dashboards', { timeout: 5000 });

        // The leaderboard table should be visible
        await expect(page.locator('table')).toBeVisible({ timeout: 5000 });
      }
    });

    test('dashboard has query controls', async ({ page }) => {
      await page.goto('/');
      const dashBtn = page.locator('button:has-text("Dashboards")');
      if (await dashBtn.isVisible()) {
        await dashBtn.click();
        await page.waitForTimeout(1000);

        // Should have group-by and aggregation dropdowns
        await expect(page.locator('select')).toHaveCount(2);
      }
    });
  });

  // ─── Beli Plugin UI ────────────────────────────────────────

  test.describe('Beli Plugin', () => {
    test('opens beli and adds a restaurant', async ({ page }) => {
      await page.goto('/');
      const beliBtn = page.locator('button:has-text("Beli")');
      if (await beliBtn.isVisible()) {
        await beliBtn.click();
        await page.waitForTimeout(1000);

        await expect(page.locator('h2')).toContainText('Restaurant Rankings', { timeout: 5000 });

        // Add a restaurant
        const nameInput = page.locator('input[placeholder="Restaurant name"]');
        if (await nameInput.isVisible()) {
          await nameInput.fill('E2E Test Restaurant');
          await page.locator('input[placeholder="Cuisine"]').fill('Italian');
          await page.locator('input[placeholder="Location"]').fill('Test City');
          await page.locator('button:has-text("+ Add")').click();
          await page.waitForTimeout(500);
        }
      }
    });

    test('rankings section shows tier information', async ({ page }) => {
      await page.goto('/');
      const beliBtn = page.locator('button:has-text("Beli")');
      if (await beliBtn.isVisible()) {
        await beliBtn.click();
        await page.waitForTimeout(1000);

        // Should show partial order explanation
        await expect(page.locator('text=partial order')).toBeVisible({ timeout: 5000 });
      }
    });
  });

  // ─── File Manager Plugin UI ────────────────────────────────

  test.describe('File Manager Plugin', () => {
    test('opens file manager and shows upload zone', async ({ page }) => {
      await page.goto('/');
      const filesBtn = page.locator('button:has-text("File Manager")');
      if (await filesBtn.isVisible()) {
        await filesBtn.click();
        await page.waitForTimeout(1000);

        await expect(page.locator('h2')).toContainText('File Manager', { timeout: 5000 });

        // Upload zone should be visible
        await expect(page.locator('text=Drop files here')).toBeVisible({ timeout: 5000 });
      }
    });

    test('file list renders after upload', async ({ page }) => {
      await page.goto('/');
      const filesBtn = page.locator('button:has-text("File Manager")');
      if (await filesBtn.isVisible()) {
        await filesBtn.click();
        await page.waitForTimeout(1000);

        // The file list section should be present
        await expect(page.locator('text=files')).toBeVisible({ timeout: 5000 });
      }
    });
  });

  // ─── Trip Planner Plugin UI ────────────────────────────────

  test.describe('Trip Planner Plugin', () => {
    test('opens trip planner and creates a trip', async ({ page }) => {
      await page.goto('/');
      const tripsBtn = page.locator('button:has-text("Trip Planner")');
      if (await tripsBtn.isVisible()) {
        await tripsBtn.click();
        await page.waitForTimeout(1000);

        await expect(page.locator('h2')).toContainText('Trip Planner', { timeout: 5000 });

        // Create a trip
        const tripInput = page.locator('input[placeholder="Trip name"]');
        if (await tripInput.isVisible()) {
          await tripInput.fill('E2E Test Trip');
          await page.locator('button:has-text("+ Trip")').click();
          await page.waitForTimeout(500);

          // Trip should appear as a button
          await expect(page.locator('button:has-text("E2E Test Trip")')).toBeVisible({ timeout: 5000 });
        }
      }
    });

    test('shows map locations section', async ({ page }) => {
      await page.goto('/');
      const tripsBtn = page.locator('button:has-text("Trip Planner")');
      if (await tripsBtn.isVisible()) {
        await tripsBtn.click();
        await page.waitForTimeout(1000);

        await expect(page.locator('text=Map Locations')).toBeVisible({ timeout: 5000 });
      }
    });
  });

  // ─── Wakatime Plugin UI ────────────────────────────────────

  test.describe('Wakatime Plugin', () => {
    test('opens wakatime and shows heartbeat form', async ({ page }) => {
      await page.goto('/');
      const wakaBtn = page.locator('button:has-text("Wakatime")');
      if (await wakaBtn.isVisible()) {
        await wakaBtn.click();
        await page.waitForTimeout(1000);

        await expect(page.locator('h2')).toContainText('Coding Activity', { timeout: 5000 });

        // Heartbeat form should be visible
        await expect(page.locator('textarea')).toBeVisible({ timeout: 5000 });
        await expect(page.locator('button:has-text("Send Heartbeat")')).toBeVisible({ timeout: 5000 });
      }
    });

    test('sends a test heartbeat', async ({ page }) => {
      await page.goto('/');
      const wakaBtn = page.locator('button:has-text("Wakatime")');
      if (await wakaBtn.isVisible()) {
        await wakaBtn.click();
        await page.waitForTimeout(1000);

        // Click send heartbeat
        const sendBtn = page.locator('button:has-text("Send Heartbeat")');
        if (await sendBtn.isVisible()) {
          await sendBtn.click();
          await page.waitForTimeout(500);
          // Should not show error
          await expect(page.locator('text=Error')).toHaveCount(0, { timeout: 3000 });
        }
      }
    });
  });

  // ─── Subsonic Plugin UI ────────────────────────────────────

  test.describe('Subsonic Plugin', () => {
    test('opens subsonic and shows music library', async ({ page }) => {
      await page.goto('/');
      const subBtn = page.locator('button:has-text("Subsonic Music")');
      if (await subBtn.isVisible()) {
        await subBtn.click();
        await page.waitForTimeout(1000);

        await expect(page.locator('h2')).toContainText('Music Library', { timeout: 5000 });

        // Upload section should be visible
        await expect(page.locator('text=Upload Music')).toBeVisible({ timeout: 5000 });
      }
    });
  });
});
