// Panorama E2E tests — PURE Playwright UI tests.
// ZERO direct API calls. Everything goes through the browser UI:
// clicking buttons, filling forms, reading rendered text, etc.

import { test, expect } from './fixtures';

test.describe('Panorama Core UI', () => {

  test('page loads with title and sidebar', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('h1')).toContainText('Panorama');
    await expect(page.locator('.sidebar')).toBeVisible();
  });

  test('sidebar has navigation links', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('a:has-text("Nodes")').first()).toBeVisible();
    await expect(page.locator('a:has-text("Schemas")').first()).toBeVisible();
    await expect(page.locator('a:has-text("Plugins")').first()).toBeVisible();
  });

  test('sidebar has Installed Apps section', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('h3:has-text("Installed Apps")')).toBeVisible();
  });

  test('can navigate between views', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('h2')).toContainText('Nodes');
    await page.click('a:has-text("Schemas")');
    await expect(page.locator('h2')).toContainText('Schemas');
    await page.click('a:has-text("Nodes")');
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

    // Target the specific node card created for deletion
    const card = page.locator('.card').filter({ hasText: uniqueTitle });
    page.once('dialog', dialog => dialog.accept());
    await card.locator('button:has-text("Delete")').click();
    await page.waitForTimeout(500);

    // Node should be gone
    await expect(page.locator(`strong:has-text("${uniqueTitle}")`)).toHaveCount(0, { timeout: 5000 });
  });
});

test.describe('Schema Viewer', () => {

  test('shows system schemas', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Schemas")');
    await expect(page.locator('h2')).toContainText('Schemas');
    await expect(page.locator('strong:has-text("NodeTime")')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('strong:has-text("NodeInfo")')).toBeVisible({ timeout: 5000 });
  });

  test('shows schema fields with namespaces', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Schemas")');
    await expect(page.locator('text=system:node_time').first()).toBeVisible({ timeout: 5000 });
    await expect(page.locator('text=system:node_title').first()).toBeVisible({ timeout: 5000 });
  });
});

test.describe('Plugin Panel', () => {

  test('plugins view shows plugin browser', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Plugins")');
    await expect(page.locator('h2')).toContainText('Plugins');
    await expect(page.locator('text=Plugins are third-party apps')).toBeVisible();
  });
});

// ═══ Journal Plugin -- full UI interaction ═══

test.describe('Journal Plugin UI', () => {

  test('opens journal and shows sidebar with Today button', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Journal")');
    await page.waitForTimeout(1000);
    await expect(page.locator('.journal-sidebar')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('button:has-text("Today")')).toBeVisible({ timeout: 3000 });
  });

  test('can create a new page', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Journal")');
    await page.waitForTimeout(1000);

    const pageTitle = `Page-${Date.now()}`;
    await page.click('button:has-text("+ New Page")');
    await page.waitForTimeout(500);
    await page.locator('.journal-new-title-input').fill(pageTitle);
    await page.locator('.journal-new-textarea').fill('Hello world from the new journal!');
    await page.click('button:has-text("Create Page")');
    await page.waitForTimeout(1000);

    // Page title should appear in sidebar
    await expect(page.locator(`.journal-page-link-title:has-text("${pageTitle}")`).first()).toBeVisible({ timeout: 5000 });
    // Content should be visible in the main area
    await expect(page.locator('text=Hello world from the new journal!')).toBeVisible({ timeout: 3000 });
  });

  test('page appears in sidebar list', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Journal")');
    await page.waitForTimeout(1000);

    // Create a page
    const pageTitle = `Sidebar-${Date.now()}`;
    await page.click('button:has-text("+ New Page")');
    await page.waitForTimeout(300);
    await page.locator('.journal-new-title-input').fill(pageTitle);
    await page.locator('.journal-new-textarea').fill('test');
    await page.click('button:has-text("Create Page")');
    await page.waitForTimeout(800);

    // Should appear in sidebar
    await expect(page.locator('.journal-page-list')).toContainText(pageTitle, { timeout: 5000 });
  });

  test('clicking a page in sidebar shows its content', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Journal")');
    await page.waitForTimeout(1000);

    // Create a page with distinct content
    const pageTitle = `Click-${Date.now()}`;
    const pageContent = `Distinct-${Date.now()}`;
    await page.click('button:has-text("+ New Page")');
    await page.waitForTimeout(300);
    await page.locator('.journal-new-title-input').fill(pageTitle);
    await page.locator('.journal-new-textarea').fill(pageContent);
    await page.click('button:has-text("Create Page")');
    await page.waitForTimeout(800);

    // Click another nav item to navigate away, then click back
    await page.click('a:has-text("Nodes")');
    await page.waitForTimeout(300);
    await page.click('a:has-text("Journal")');
    await page.waitForTimeout(800);

    // Click the page in the sidebar
    await page.locator(`.journal-page-link-title:has-text("${pageTitle}")`).click();
    await page.waitForTimeout(500);

    // Content should be visible
    await expect(page.locator('.journal-main')).toContainText(pageContent, { timeout: 5000 });
  });

  test('inline block editing works', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Journal")');
    await page.waitForTimeout(1000);

    // Create a page
    await page.click('button:has-text("+ New Page")');
    await page.waitForTimeout(300);
    const pageTitle = `Edit-${Date.now()}`;
    await page.locator('.journal-new-title-input').fill(pageTitle);
    await page.locator('.journal-new-textarea').fill('Original content');
    await page.click('button:has-text("Create Page")');
    await page.waitForTimeout(1000);

    // Click the block content area to edit
    await page.locator('.journal-block-content').first().click();
    await page.waitForTimeout(300);

    // Textarea should appear with current content
    const textarea = page.locator('.journal-block-textarea');
    await expect(textarea).toBeVisible({ timeout: 3000 });
    // Edit content
    await textarea.fill('Edited content');
    await page.click('button:has-text("Save")');
    await page.waitForTimeout(800);

    // Updated content should be visible
    await expect(page.locator('.journal-main')).toContainText('Edited content', { timeout: 5000 });
  });

  test('can delete a page', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Journal")');
    await page.waitForTimeout(1000);

    // Create a page to delete
    await page.click('button:has-text("+ New Page")');
    await page.waitForTimeout(300);
    const delTitle = `Del-${Date.now()}`;
    await page.locator('.journal-new-title-input').fill(delTitle);
    await page.locator('.journal-new-textarea').fill('to be deleted');
    await page.click('button:has-text("Create Page")');
    await page.waitForTimeout(800);

    // Click the delete button in the page header
    page.once('dialog', (dialog) => dialog.accept());
    await page.locator('.journal-page-meta button[title="Delete page"]').click();
    await page.waitForTimeout(800);

    // Page should show as deleted in sidebar
    await expect(page.locator('.journal-page-link.deleted').first()).toBeVisible({ timeout: 5000 });
  });

  test('adding a child block to a page', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Journal")');
    await page.waitForTimeout(1000);

    // Create a page
    await page.click('button:has-text("+ New Page")');
    await page.waitForTimeout(300);
    await page.locator('.journal-new-title-input').fill('Parent Page');
    await page.locator('.journal-new-textarea').fill('Parent content');
    await page.click('button:has-text("Create Page")');
    await page.waitForTimeout(1000);

    // Add a child block
    const childContent = `Child-${Date.now()}`;
    const newBlockTextarea = page.locator('.journal-new-block .journal-new-textarea').first();
    await newBlockTextarea.fill(childContent);
    await newBlockTextarea.press('Enter');
    await page.waitForTimeout(1000);

    // Child block should appear
    await expect(page.locator('.journal-main')).toContainText(childContent, { timeout: 5000 });
  });

  test('Today button shows journal page for current date', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Journal")');
    await page.waitForTimeout(1000);

    // Click Today button
    await page.click('button:has-text("Today")');
    await page.waitForTimeout(800);

    // Should show a date badge with today's date
    const today = new Date().toISOString().slice(0, 10);
    await expect(page.locator('.journal-date-badge')).toContainText(today, { timeout: 5000 });
  });
});

// ═══ Wakatime Plugin — full UI interaction ═══

test.describe('Wakatime Plugin UI', () => {

  test('opens coding activity and shows heartbeat form', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Coding Activity")');
    await page.waitForTimeout(1000);
    await expect(page.locator('h2')).toContainText('Coding Activity', { timeout: 5000 });
    await expect(page.locator('textarea')).toBeVisible({ timeout: 3000 });
    await expect(page.locator('button:has-text("Send Heartbeat")')).toBeVisible();
  });

  test('can send a heartbeat and see stats panels', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Coding Activity")');
    await page.waitForTimeout(1000);

    await page.click('button:has-text("Send Heartbeat")');
    await page.waitForTimeout(1500);

    // Should show the leaderboard section (even if empty initially)
    // New UI uses "Per Project" instead of "Project Leaderboard"
    await expect(page.locator('text=Per Project')).toBeVisible({ timeout: 5000 });
  });

  test('heartbeat form is pre-populated with JSON', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Coding Activity")');
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

  test('opens dashboards and shows home dashboard with panels', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Dashboards")');
    await page.waitForTimeout(2000);
    // New dashboard UI shows the home dashboard with panel titles
    await expect(page.locator('h2')).toBeVisible({ timeout: 5000 });
    // Should show dashboard panels (at minimum Per Project)
    await expect(page.locator('text=Per Project')).toBeVisible({ timeout: 5000 });
  });

  test('has time range presets and refresh selector', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Dashboards")');
    await page.waitForTimeout(2000);

    // Time range preset buttons — first 6 are shown by default
    await expect(page.locator('button:has-text("Last 24h")')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('button:has-text("Custom...")')).toBeVisible({ timeout: 3000 });
    // Refresh interval select
    const selects = page.locator('select');
    await expect(selects.first()).toBeVisible({ timeout: 3000 });
  });

  test('shows panel grid with leaderboard and timeseries panels', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Dashboards")');
    await page.waitForTimeout(2000);

    // Should see coding activity panels
    await expect(page.locator('text=Per Project')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('text=Per Language')).toBeVisible({ timeout: 3000 });
  });

  test('has Edit button to open dashboard builder', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Dashboards")');
    await page.waitForTimeout(2000);

    const editBtn = page.locator('button:has-text("Edit")');
    await expect(editBtn).toBeVisible({ timeout: 5000 });
  });

  test('can open builder modal to edit dashboard', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Dashboards")');
    await page.waitForTimeout(2000);

    await page.click('button:has-text("Edit")');
    await page.waitForTimeout(1000);
    // Builder modal should appear with panel editor
    await expect(page.locator('text=Panels')).toBeVisible({ timeout: 5000 });
  });
});

// ═══ Beli Plugin — full UI interaction ═══

test.describe('Beli Plugin UI', () => {

  test('opens restaurant rankings and shows explanation', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Restaurant Rankings")');
    await page.waitForTimeout(1000);
    await expect(page.locator('h2')).toContainText('Restaurant Rankings', { timeout: 5000 });
    await expect(page.locator('text=partial order')).toBeVisible({ timeout: 3000 });
    await expect(page.locator('text=pairwise comparisons')).toBeVisible();
    await expect(page.locator('text=star ratings')).toBeVisible();
  });

  test('can add a restaurant', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Restaurant Rankings")');
    await page.waitForTimeout(1000);

    await page.locator('input[placeholder="Restaurant name"]').fill('E2E Sushi Place');
    await page.locator('input[placeholder="Cuisine"]').fill('Sushi');
    await page.locator('input[placeholder="Location"]').fill('Test District');
    await page.click('button:has-text("+ Add")');
    await page.waitForTimeout(800);
  });

  test('comparison form shows restaurant options after adding', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Restaurant Rankings")');
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
    await page.click('a:has-text("Restaurant Rankings")');
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
    await page.click('a:has-text("Trip Planner")');
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
    await page.click('a:has-text("Trip Planner")');
    await page.waitForTimeout(1000);
    await expect(page.locator('text=Map Locations')).toBeVisible({ timeout: 5000 });
  });

  test('selecting a trip shows add-event form', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Trip Planner")');
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
    await page.click('a:has-text("Trip Planner")');
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
    await page.click('a:has-text("File Manager")');
    await page.waitForTimeout(1000);
    await expect(page.locator('h2')).toContainText('File Manager', { timeout: 5000 });
    await expect(page.locator('text=Drop files here')).toBeVisible({ timeout: 3000 });
  });

  test('shows file count and list area', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("File Manager")');
    await page.waitForTimeout(1000);

    await expect(page.locator('text=No files uploaded yet').first()).toBeVisible({ timeout: 5000 });
  });
});

// ═══ Subsonic Plugin — full UI interaction ═══

test.describe('Subsonic Music Plugin UI', () => {

  test('opens music library and shows upload section', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Music Library")');
    await page.waitForTimeout(1000);
    await expect(page.locator('h2')).toContainText('Music Library', { timeout: 5000 });
    await expect(page.locator('text=Upload Music')).toBeVisible({ timeout: 3000 });
  });

  test('shows artists and albums sections', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Music Library")');
    await page.waitForTimeout(1000);

    await expect(page.locator('h4:has-text("Artists")')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('h4:has-text("Albums")')).toBeVisible({ timeout: 5000 });
  });
});
