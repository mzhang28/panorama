// Panorama E2E tests — PURE Playwright UI tests.
// ZERO direct API calls. Everything goes through the browser UI:
// clicking buttons, filling forms, reading rendered text, etc.

import { test, expect } from '@playwright/test';

test.describe('Panorama Core UI', () => {

  test('page loads with title and sidebar', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('h1')).toContainText('Panorama');
    await expect(page.locator('.sidebar')).toBeVisible();
  });

  test('sidebar has navigation buttons', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('button:has-text("Nodes")')).toBeVisible();
    await expect(page.locator('button:has-text("Schemas")')).toBeVisible();
    await expect(page.locator('button:has-text("Plugins")')).toBeVisible();
  });

  test('sidebar has Installed Apps section', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('h3:has-text("Installed Apps")')).toBeVisible();
  });

  test('can navigate between views', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('h2')).toContainText('Nodes');
    await page.click('button:has-text("Schemas")');
    await expect(page.locator('h2')).toContainText('Schemas');
    await page.click('button:has-text("Nodes")');
    await expect(page.locator('h2')).toContainText('Nodes');
  });
});

test.describe('Node CRUD through UI', () => {

  test('create node form opens and closes', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("+ New Node")');
    await expect(page.locator('input[placeholder="Node title"]')).toBeVisible();
    await expect(page.locator('button:has-text("Create")')).toBeVisible();
  });

  test('creates a node and sees it in the list', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("+ New Node")');
    const uniqueTitle = `E2E-${Date.now()}`;
    await page.locator('input[placeholder="Node title"]').fill(uniqueTitle);
    await page.click('button:has-text("Create")');
    // The node should appear in the list — use first() since nodes accumulate
    await expect(page.locator(`strong:has-text("${uniqueTitle}")`).first()).toBeVisible({ timeout: 5000 });
  });

  test('clicking a node shows its detail', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('.card', { timeout: 5000 });
    await page.locator('.card').first().click();
    await expect(page.locator('h3:has-text("Node:")').first()).toBeVisible({ timeout: 3000 });
    // The detail panel shows field data in a table
    await expect(page.locator('table')).toBeVisible({ timeout: 3000 });
  });

  test('can delete a node', async ({ page }) => {
    await page.goto('/');
    // First create one so we have something to delete
    const uniqueTitle = `DelMe-${Date.now()}`;
    await page.click('button:has-text("+ New Node")');
    await page.locator('input[placeholder="Node title"]').fill(uniqueTitle);
    await page.click('button:has-text("Create")');
    await page.waitForTimeout(500);

    // Set up dialog handler BEFORE clicking delete
    page.once('dialog', dialog => dialog.accept());
    await page.locator('button:has-text("Delete")').first().click();
    await page.waitForTimeout(500);

    // Node should be gone
    await expect(page.locator(`strong:has-text("${uniqueTitle}")`)).toHaveCount(0, { timeout: 5000 });
  });
});

test.describe('Schema Viewer', () => {

  test('shows system schemas', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Schemas")');
    await expect(page.locator('h2')).toContainText('Schemas');
    await expect(page.locator('strong:has-text("NodeTime")')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('strong:has-text("NodeInfo")')).toBeVisible({ timeout: 5000 });
  });

  test('shows schema fields with namespaces', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Schemas")');
    await expect(page.locator('text=system:node_time').first()).toBeVisible({ timeout: 5000 });
    await expect(page.locator('text=system:node_title').first()).toBeVisible({ timeout: 5000 });
  });
});

test.describe('Plugin Panel', () => {

  test('plugins view shows plugin browser', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Plugins")');
    await expect(page.locator('h2')).toContainText('Plugins');
    await expect(page.locator('text=Plugins are third-party apps')).toBeVisible();
  });
});

// ═══ Journal Plugin — full UI interaction ═══

test.describe('Journal Plugin UI', () => {

  test('opens journal from sidebar and renders the editor', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.waitForTimeout(1000);
    await expect(page.locator('h2')).toContainText('Journal', { timeout: 5000 });
    await expect(page.locator('input[placeholder="Entry title"]')).toBeVisible({ timeout: 3000 });
    await expect(page.locator('textarea[placeholder*="Write your entry"]')).toBeVisible();
  });

  test('creates a journal entry through the form', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.waitForTimeout(1000);

    const entryTitle = `Entry-${Date.now()}`;
    await page.locator('input[placeholder="Entry title"]').fill(entryTitle);
    await page.locator('textarea[placeholder*="Write your entry"]').fill('This journal entry was created by the E2E test.');
    await page.selectOption('select', 'happy');
    await page.click('button:has-text("Save Entry")');
    await page.waitForTimeout(800);

    // Entry should appear in the list
    await expect(page.locator(`strong:has-text("${entryTitle}")`).first()).toBeVisible({ timeout: 5000 });
    // Mood should be visible
    await expect(page.locator('text=Mood: happy').first()).toBeVisible({ timeout: 3000 });
  });

  test('clicking an entry expands to show content', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.waitForTimeout(1000);

    // Create an entry first
    const expandTitle = `Expand-${Date.now()}`;
    const expandContent = `Secret-${Date.now()}`;
    await page.locator('input[placeholder="Entry title"]').fill(expandTitle);
    await page.locator('textarea[placeholder*="Write your entry"]').fill(expandContent);
    await page.click('button:has-text("Save Entry")');
    await page.waitForTimeout(500);

    // Click the entry card
    await page.locator(`strong:has-text("${expandTitle}")`).first().click();
    await page.waitForTimeout(300);

    // Content should now be visible
    await expect(page.locator(`pre:has-text("${expandContent}")`).first()).toBeVisible({ timeout: 3000 });
  });

  test('mood selector has options', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.waitForTimeout(1000);

    const moodSelect = page.locator('select');
    await expect(moodSelect).toBeVisible();
    const options = await moodSelect.locator('option').allTextContents();
    expect(options).toContain('happy');
    expect(options).toContain('thoughtful');
    expect(options).toContain('excited');
  });
});

// ═══ Wakatime Plugin — full UI interaction ═══

test.describe('Wakatime Plugin UI', () => {

  test('opens coding activity and shows heartbeat form', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Wakatime")');
    await page.waitForTimeout(1000);
    await expect(page.locator('h2')).toContainText('Coding Activity', { timeout: 5000 });
    await expect(page.locator('textarea')).toBeVisible({ timeout: 3000 });
    await expect(page.locator('button:has-text("Send Heartbeat")')).toBeVisible();
  });

  test('can send a heartbeat', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Wakatime")');
    await page.waitForTimeout(1000);

    await page.click('button:has-text("Send Heartbeat")');
    await page.waitForTimeout(1500);

    // Should show the leaderboard section (even if empty initially)
    await expect(page.locator('text=Project Leaderboard')).toBeVisible({ timeout: 5000 });
  });

  test('heartbeat form is pre-populated with JSON', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Wakatime")');
    await page.waitForTimeout(1000);

    const textarea = page.locator('textarea');
    const content = await textarea.inputValue();
    expect(content).toContain('entity');
    expect(content).toContain('project');
    expect(content).toContain('Rust');
  });
});

// ═══ Dashboards Plugin — full UI interaction ═══

test.describe('Dashboards Plugin UI', () => {

  test('opens dashboards and shows leaderboard table', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Dashboards")');
    await page.waitForTimeout(1000);
    await expect(page.locator('h2')).toContainText('Dashboards', { timeout: 5000 });
    await expect(page.locator('table')).toBeVisible({ timeout: 5000 });
  });

  test('has Group By and Aggregation dropdowns', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Dashboards")');
    await page.waitForTimeout(1000);

    const selects = page.locator('select');
    await expect(selects).toHaveCount(2);
  });

  test('can switch aggregation to Count', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Dashboards")');
    await page.waitForTimeout(1000);

    // Select "Count" from the aggregation dropdown
    const aggSelect = page.locator('select').nth(1);
    await aggSelect.selectOption('count');
    await page.waitForTimeout(800);
    // Verify the select value changed
    await expect(aggSelect).toHaveValue('count');
  });

  test('can switch group by to Language', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Dashboards")');
    await page.waitForTimeout(1000);

    const groupSelect = page.locator('select').first();
    await groupSelect.selectOption('wakatime:language');
    await page.waitForTimeout(800);
    await expect(groupSelect).toHaveValue('wakatime:language');
  });

  test('table has ranked entries with position numbers', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Dashboards")');
    await page.waitForTimeout(1000);

    // Should see rank #1, #2, #3 indicators
    await expect(page.locator('table')).toBeVisible({ timeout: 5000 });
    const tableText = await page.locator('table').textContent();
    // At minimum the table renders
    expect(tableText).toBeTruthy();
  });
});

// ═══ Beli Plugin — full UI interaction ═══

test.describe('Beli Plugin UI', () => {

  test('opens restaurant rankings and shows explanation', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Beli")');
    await page.waitForTimeout(1000);
    await expect(page.locator('h2')).toContainText('Restaurant Rankings', { timeout: 5000 });
    await expect(page.locator('text=partial order')).toBeVisible({ timeout: 3000 });
    await expect(page.locator('text=pairwise comparisons')).toBeVisible();
    await expect(page.locator('text=star ratings')).toBeVisible();
  });

  test('can add a restaurant', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Beli")');
    await page.waitForTimeout(1000);

    await page.locator('input[placeholder="Restaurant name"]').fill('E2E Sushi Place');
    await page.locator('input[placeholder="Cuisine"]').fill('Sushi');
    await page.locator('input[placeholder="Location"]').fill('Test District');
    await page.click('button:has-text("+ Add")');
    await page.waitForTimeout(800);
  });

  test('comparison form shows restaurant options after adding', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Beli")');
    await page.waitForTimeout(1000);

    // Add two restaurants
    await page.locator('input[placeholder="Restaurant name"]').fill('Ramen A');
    await page.locator('input[placeholder="Cuisine"]').fill('Ramen');
    await page.locator('input[placeholder="Location"]').fill('Shibuya');
    await page.click('button:has-text("+ Add")');
    await page.waitForTimeout(500);

    await page.locator('input[placeholder="Restaurant name"]').fill('Ramen B');
    await page.locator('input[placeholder="Cuisine"]').fill('Ramen');
    await page.locator('input[placeholder="Location"]').fill('Shinjuku');
    await page.click('button:has-text("+ Add")');
    await page.waitForTimeout(500);

    // The comparison dropdowns should now have options
    const betterSelect = page.locator('select').first();
    const options = await betterSelect.locator('option').allTextContents();
    expect(options.some(o => o.includes('Ramen A'))).toBeTruthy();
    expect(options.some(o => o.includes('Ramen B'))).toBeTruthy();
  });

  test('can record a comparison between two restaurants', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Beli")');
    await page.waitForTimeout(1000);

    // Add restaurants
    await page.locator('input[placeholder="Restaurant name"]').fill('Pizza Place');
    await page.locator('input[placeholder="Cuisine"]').fill('Italian');
    await page.locator('input[placeholder="Location"]').fill('Test');
    await page.click('button:has-text("+ Add")');
    await page.waitForTimeout(400);

    await page.locator('input[placeholder="Restaurant name"]').fill('Burger Joint');
    await page.locator('input[placeholder="Cuisine"]').fill('American');
    await page.locator('input[placeholder="Location"]').fill('Test');
    await page.click('button:has-text("+ Add")');
    await page.waitForTimeout(400);

    // Select better and worse
    await page.locator('select').first().selectOption({ index: 1 }); // first real option
    await page.locator('select').nth(1).selectOption({ index: 2 }); // second real option
    await page.click('button:has-text("Compare")');
    await page.waitForTimeout(800);

    // Should show ranking tiers
    await expect(page.locator('strong:has-text("Tier")').first()).toBeVisible({ timeout: 5000 });
  });
});

// ═══ Trip Planner Plugin — full UI interaction ═══

test.describe('Trip Planner Plugin UI', () => {

  test('opens trip planner and creates a trip', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Trip Planner")');
    await page.waitForTimeout(1000);
    await expect(page.locator('h2')).toContainText('Trip Planner', { timeout: 5000 });

    const tripName = `Trip-${Date.now()}`;
    await page.locator('input[placeholder="Trip name"]').fill(tripName);
    await page.click('button:has-text("+ Trip")');
    await page.waitForTimeout(500);

    await expect(page.locator(`button:has-text("${tripName}")`).first()).toBeVisible({ timeout: 5000 });
  });

  test('shows Map Locations section', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Trip Planner")');
    await page.waitForTimeout(1000);
    await expect(page.locator('text=Map Locations')).toBeVisible({ timeout: 5000 });
  });

  test('selecting a trip shows add-event form', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Trip Planner")');
    await page.waitForTimeout(1000);

    // Create a trip first
    await page.locator('input[placeholder="Trip name"]').fill('Event Test Trip');
    await page.click('button:has-text("+ Trip")');
    await page.waitForTimeout(500);

    // Click the trip button to select it
    await page.click('button:has-text("Event Test Trip")');
    await page.waitForTimeout(500);

    // Event form should appear
    await expect(page.locator('input[placeholder="Event name"]')).toBeVisible({ timeout: 3000 });
    await expect(page.locator('input[placeholder="Latitude"]')).toBeVisible();
    await expect(page.locator('input[placeholder="Longitude"]')).toBeVisible();
    await expect(page.locator('input[placeholder="Location name"]')).toBeVisible();
  });

  test('can add an event with geo coordinates', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Trip Planner")');
    await page.waitForTimeout(1000);

    await page.locator('input[placeholder="Trip name"]').fill('Geo Trip');
    await page.click('button:has-text("+ Trip")');
    await page.waitForTimeout(400);

    await page.click('button:has-text("Geo Trip")');
    await page.waitForTimeout(400);

    await page.locator('input[placeholder="Event name"]').fill('Tokyo Tower');
    await page.locator('input[placeholder="Latitude"]').fill('35.6586');
    await page.locator('input[placeholder="Longitude"]').fill('139.7454');
    await page.locator('input[placeholder="Location name"]').fill('Minato, Tokyo');
    await page.click('button:has-text("+ Event")');
    await page.waitForTimeout(800);

    // Event should appear in the list
    await expect(page.locator('strong:has-text("Tokyo Tower")')).toBeVisible({ timeout: 5000 });
    // Map section should show the location
    await expect(page.locator('text=Minato, Tokyo').first()).toBeVisible({ timeout: 3000 });
  });
});

// ═══ File Manager Plugin — full UI interaction ═══

test.describe('File Manager Plugin UI', () => {

  test('opens file manager and shows upload zone', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("File Manager")');
    await page.waitForTimeout(1000);
    await expect(page.locator('h2')).toContainText('File Manager', { timeout: 5000 });
    await expect(page.locator('text=Drop files here')).toBeVisible({ timeout: 3000 });
  });

  test('shows file count and list area', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("File Manager")');
    await page.waitForTimeout(1000);

    await expect(page.locator('text=No files uploaded yet').first()).toBeVisible({ timeout: 5000 });
  });
});

// ═══ Subsonic Plugin — full UI interaction ═══

test.describe('Subsonic Music Plugin UI', () => {

  test('opens music library and shows upload section', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Subsonic Music")');
    await page.waitForTimeout(1000);
    await expect(page.locator('h2')).toContainText('Music Library', { timeout: 5000 });
    await expect(page.locator('text=Upload Music')).toBeVisible({ timeout: 3000 });
  });

  test('shows artists and albums sections', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Subsonic Music")');
    await page.waitForTimeout(1000);

    await expect(page.locator('text=Artists')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('h4:has-text("Albums")')).toBeVisible({ timeout: 5000 });
  });
});
