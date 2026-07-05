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
    await expect(page.locator(`strong:has-text("${uniqueTitle}")`).first()).toBeVisible();
  });

  test('clicking a node shows its detail', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("+ New Node")');
    const uniqueTitle = `Detail-${Date.now()}`;
    await page.locator('input[placeholder="Node title"]').fill(uniqueTitle);
    await page.click('button:has-text("Create")');
    const card = page.locator('.card').filter({ hasText: uniqueTitle });
    await card.click();
    await expect(page.locator('h3:has-text("Node:")').first()).toBeVisible();
    // The detail panel shows field data in a table
    await expect(page.locator('table')).toBeVisible();
  });

  test('can delete a node', async ({ page }) => {
    await page.goto('/');
    // First create one so we have something to delete
    const uniqueTitle = `DelMe-${Date.now()}`;
    await page.click('button:has-text("+ New Node")');
    await page.locator('input[placeholder="Node title"]').fill(uniqueTitle);
    await page.click('button:has-text("Create")');
    // Target the specific node card created for deletion
    const card = page.locator('.card').filter({ hasText: uniqueTitle });
    page.once('dialog', dialog => dialog.accept());
    await card.locator('button:has-text("Delete")').click();
    // Node should be gone
    await expect(page.locator(`strong:has-text("${uniqueTitle}")`)).toHaveCount(0);
  });
});

test.describe('Schema Viewer', () => {

  test('shows system schemas', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Schemas")');
    await expect(page.locator('h2')).toContainText('Schemas');
    await expect(page.locator('strong:has-text("NodeTime")')).toBeVisible();
    await expect(page.locator('strong:has-text("NodeInfo")')).toBeVisible();
  });

  test('shows schema fields with namespaces', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Schemas")');
    await expect(page.locator('text=system:node_time').first()).toBeVisible();
    await expect(page.locator('text=system:node_title').first()).toBeVisible();
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

    await expect(page.locator('.journal-sidebar')).toBeVisible();
    await expect(page.locator('button:has-text("Today")')).toBeVisible();
  });

  test('can create a new page', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Journal")');
    const pageTitle = `Page-${Date.now()}`;
    await page.click('button:has-text("+ New Page")');

    await page.locator('.journal-new-title-input').fill(pageTitle);
    await page.locator('.journal-new-textarea').fill('Hello world from the new journal!');
    await page.click('button:has-text("Create Page")');
    // Page title should appear in sidebar
    await expect(page.locator(`.journal-page-link-title:has-text("${pageTitle}")`).first()).toBeVisible();
    // Content should be visible in the main area
    await expect(page.locator('text=Hello world from the new journal!')).toBeVisible();
  });

  test('page appears in sidebar list', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Journal")');
    // Create a page
    const pageTitle = `Sidebar-${Date.now()}`;
    await page.click('button:has-text("+ New Page")');

    await page.locator('.journal-new-title-input').fill(pageTitle);
    await page.locator('.journal-new-textarea').fill('test');
    await page.click('button:has-text("Create Page")');
    // Should appear in sidebar
    await expect(page.locator('.journal-page-list')).toContainText(pageTitle);
  });

  test('clicking a page in sidebar shows its content', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Journal")');
    // Create a page with distinct content
    const pageTitle = `Click-${Date.now()}`;
    const pageContent = `Distinct-${Date.now()}`;
    await page.click('button:has-text("+ New Page")');

    await page.locator('.journal-new-title-input').fill(pageTitle);
    await page.locator('.journal-new-textarea').fill(pageContent);
    await page.click('button:has-text("Create Page")');
    // Click another nav item to navigate away, then click back
    await page.click('a:has-text("Nodes")');

    await page.click('a:has-text("Journal")');
    // Click the page in the sidebar
    await page.locator(`.journal-page-link-title:has-text("${pageTitle}")`).click();
    // Content should be visible
    await expect(page.locator('.journal-main')).toContainText(pageContent);
  });

  test('inline block editing works', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Journal")');
    // Create a page
    await page.click('button:has-text("+ New Page")');

    const pageTitle = `Edit-${Date.now()}`;
    await page.locator('.journal-new-title-input').fill(pageTitle);
    await page.locator('.journal-new-textarea').fill('Original content');
    await page.click('button:has-text("Create Page")');
    // Click the block content area to edit
    await page.locator('.journal-block-content').first().click();
    // Textarea should appear with current content
    const textarea = page.locator('.journal-block-textarea');
    await expect(textarea).toBeVisible();
    // Edit content
    await textarea.fill('Edited content');
    await page.click('button:has-text("Save")');
    // Updated content should be visible
    await expect(page.locator('.journal-main')).toContainText('Edited content');
  });

  test('can delete a page', async ({ page, baseURL }) => {

    await page.goto('/');
    await page.click('a:has-text("Journal")');
    // Create a page to delete
    await page.click('button:has-text("+ New Page")');

    const delTitle = `Del-${Date.now()}`;
    await page.locator('.journal-new-title-input').fill(delTitle);
    await page.locator('.journal-new-textarea').fill('to be deleted');
    await page.click('button:has-text("Create Page")');

    // Wait for the page to appear in sidebar and be selected
    await expect(page.locator(`.journal-page-link-title:has-text("${delTitle}")`).first()).toBeVisible();

    // Wait for the page header with delete button to render
    await expect(page.locator('.journal-page-meta button[title="Delete page"]')).toBeVisible();

    // Click the delete button in the page header
    page.once('dialog', (dialog) => dialog.accept());
    await page.locator('.journal-page-meta button[title="Delete page"]').click();

    // Page should show as soft-deleted in sidebar (strikethrough)
    await expect(page.locator('.journal-page-link.deleted').first()).toBeVisible();
  });

  test('adding a child block to a page', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Journal")');
    // Create a page
    await page.click('button:has-text("+ New Page")');

    await page.locator('.journal-new-title-input').fill('Parent Page');
    await page.locator('.journal-new-textarea').fill('Parent content');
    await page.click('button:has-text("Create Page")');
    // Add a child block
    const childContent = `Child-${Date.now()}`;
    const newBlockTextarea = page.locator('.journal-new-block .journal-new-textarea').first();
    await newBlockTextarea.fill(childContent);
    await newBlockTextarea.press('Enter');
    // Child block should appear
    await expect(page.locator('.journal-main')).toContainText(childContent);
  });

  test('Today button shows journal page for current date', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Journal")');
    // Click Today button
    await page.click('button:has-text("Today")');
    // Should show a date badge with today's date
    const today = new Date().toISOString().slice(0, 10);
    await expect(page.locator('.journal-date-badge')).toContainText(today);
  });
});

// ═══ Wakatime Plugin — full UI interaction ═══

test.describe('Wakatime Plugin UI', () => {

  test('opens coding activity and shows heartbeat form', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Coding Activity")');

    await expect(page.locator('h2')).toContainText('Coding Activity');
    await expect(page.locator('textarea')).toBeVisible();
    await expect(page.locator('button:has-text("Send Heartbeat")')).toBeVisible();
  });

  test('can send a heartbeat and see stats panels', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Coding Activity")');
    await page.click('button:has-text("Send Heartbeat")');
    // Should show the leaderboard section (even if empty initially)
    // New UI uses "Per Project" instead of "Project Leaderboard"
    await expect(page.locator('text=Per Project')).toBeVisible();
  });

  test('heartbeat form is pre-populated with JSON', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Coding Activity")');
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

    // New dashboard UI shows the home dashboard with panel titles
    await expect(page.locator('h2')).toBeVisible();
    // Should show dashboard panels (at minimum Per Project)
    await expect(page.locator('text=Per Project')).toBeVisible();
  });

  test('has time range presets and refresh selector', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Dashboards")');
    // Time range preset buttons — first 6 are shown by default
    await expect(page.locator('button:has-text("Last 24h")')).toBeVisible();
    await expect(page.locator('button:has-text("Custom...")')).toBeVisible();
    // Refresh interval select
    const selects = page.locator('select');
    await expect(selects.first()).toBeVisible();
  });

  test('shows panel grid with leaderboard and timeseries panels', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Dashboards")');
    // Should see coding activity panels
    await expect(page.locator('text=Per Project')).toBeVisible();
    await expect(page.locator('text=Per Language')).toBeVisible();
  });

  test('has Edit button to open dashboard builder', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Dashboards")');
    const editBtn = page.locator('button:has-text("Edit")');
    await expect(editBtn).toBeVisible();
  });

  test('can open builder modal to edit dashboard', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Dashboards")');
    await page.click('button:has-text("Edit")');

    // Builder modal should appear with panel editor
    await expect(page.locator('text=Panels')).toBeVisible();
  });
});

// ═══ Beli Plugin — full UI interaction ═══

test.describe('Beli Plugin UI', () => {

  test('opens restaurant rankings and shows explanation', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Restaurant Rankings")');

    await expect(page.locator('h2')).toContainText('Restaurant Rankings');
    await expect(page.locator('text=partial order')).toBeVisible();
    await expect(page.locator('text=pairwise comparisons')).toBeVisible();
    await expect(page.locator('text=star ratings')).toBeVisible();
  });

  test('can add a restaurant', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Restaurant Rankings")');
    await page.locator('input[placeholder="Restaurant name"]').fill('E2E Sushi Place');
    await page.locator('input[placeholder="Cuisine"]').fill('Sushi');
    await page.locator('input[placeholder="Location"]').fill('Test District');
    await page.click('button:has-text("+ Add")');

  });

  test('comparison form shows restaurant options after adding', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Restaurant Rankings")');
    // Add two restaurants
    await page.locator('input[placeholder="Restaurant name"]').fill('Ramen A');
    await page.locator('input[placeholder="Cuisine"]').fill('Ramen');
    await page.locator('input[placeholder="Location"]').fill('Shibuya');
    await page.click('button:has-text("+ Add")');
    // Wait for form to reset before filling next restaurant
    await expect(page.locator('input[placeholder="Restaurant name"]')).toBeEmpty();
    await page.locator('input[placeholder="Restaurant name"]').fill('Ramen B');
    await page.locator('input[placeholder="Cuisine"]').fill('Ramen');
    await page.locator('input[placeholder="Location"]').fill('Shinjuku');
    await page.click('button:has-text("+ Add")');
    // Wait for form to reset, confirming the add was processed
    await expect(page.locator('input[placeholder="Restaurant name"]')).toBeEmpty();
    // The comparison dropdowns should now have both restaurants
    const betterSelect = page.locator('select').first();
    await expect(betterSelect).toContainText('Ramen A');
    await expect(betterSelect).toContainText('Ramen B');
  });

  test('can record a comparison between two restaurants', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Restaurant Rankings")');
    // Add restaurants
    await page.locator('input[placeholder="Restaurant name"]').fill('Pizza Place');
    await page.locator('input[placeholder="Cuisine"]').fill('Italian');
    await page.locator('input[placeholder="Location"]').fill('Test');
    await page.click('button:has-text("+ Add")');
    await page.locator('input[placeholder="Restaurant name"]').fill('Burger Joint');
    await page.locator('input[placeholder="Cuisine"]').fill('American');
    await page.locator('input[placeholder="Location"]').fill('Test');
    await page.click('button:has-text("+ Add")');
    // Select better and worse
    await page.locator('select').first().selectOption({ index: 1 }); // first real option
    await page.locator('select').nth(1).selectOption({ index: 2 }); // second real option
    await page.click('button:has-text("Compare")');
    // Should show ranking tiers
    await expect(page.locator('strong:has-text("Tier")').first()).toBeVisible();
  });
});

// ═══ Trip Planner Plugin — full UI interaction ═══

test.describe('Trip Planner Plugin UI', () => {

  test('opens trip planner and creates a trip', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Trip Planner")');

    await expect(page.locator('h2')).toContainText('Trip Planner');

    const tripName = `Trip-${Date.now()}`;
    await page.locator('input[placeholder="Trip name"]').fill(tripName);
    await page.click('button:has-text("+ Trip")');
    await expect(page.locator(`button:has-text("${tripName}")`).first()).toBeVisible();
  });

  test('shows Map Locations section', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Trip Planner")');

    await expect(page.locator('text=Map Locations')).toBeVisible();
  });

  test('selecting a trip shows add-event form', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Trip Planner")');
    // Create a trip first
    await page.locator('input[placeholder="Trip name"]').fill('Event Test Trip');
    await page.click('button:has-text("+ Trip")');
    // Click the trip button to select it
    await page.click('button:has-text("Event Test Trip")');
    // Event form should appear
    await expect(page.locator('input[placeholder="Event name"]')).toBeVisible();
    await expect(page.locator('input[placeholder="Latitude"]')).toBeVisible();
    await expect(page.locator('input[placeholder="Longitude"]')).toBeVisible();
    await expect(page.locator('input[placeholder="Location name"]')).toBeVisible();
  });

  test('can add an event with geo coordinates', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Trip Planner")');
    await page.locator('input[placeholder="Trip name"]').fill('Geo Trip');
    await page.click('button:has-text("+ Trip")');
    await page.click('button:has-text("Geo Trip")');
    await page.locator('input[placeholder="Event name"]').fill('Tokyo Tower');
    await page.locator('input[placeholder="Latitude"]').fill('35.6586');
    await page.locator('input[placeholder="Longitude"]').fill('139.7454');
    await page.locator('input[placeholder="Location name"]').fill('Minato, Tokyo');
    await page.click('button:has-text("+ Event")');
    // Event should appear in the list
    await expect(page.locator('strong:has-text("Tokyo Tower")')).toBeVisible();
    // Map section should show the location
    await expect(page.locator('text=Minato, Tokyo').first()).toBeVisible();
  });
});

// ═══ File Manager Plugin — full UI interaction ═══

test.describe('File Manager Plugin UI', () => {

  test('opens file manager and shows upload zone', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("File Manager")');

    await expect(page.locator('h2')).toContainText('File Manager');
    await expect(page.locator('text=Drop files here')).toBeVisible();
  });

  test('shows file count and list area', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("File Manager")');
    await expect(page.locator('text=No files uploaded yet').first()).toBeVisible();
  });
});

// ═══ Subsonic Plugin — full UI interaction ═══

test.describe('Subsonic Music Plugin UI', () => {

  test('opens music library and shows upload section', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Music Library")');

    await expect(page.locator('h2')).toContainText('Music Library');
    await expect(page.locator('text=Upload Music')).toBeVisible();
  });

  test('shows artists and albums sections', async ({ page }) => {
    await page.goto('/');
    await page.click('a:has-text("Music Library")');
    await expect(page.locator('h4:has-text("Artists")')).toBeVisible();
    await expect(page.locator('h4:has-text("Albums")')).toBeVisible();
  });
});
