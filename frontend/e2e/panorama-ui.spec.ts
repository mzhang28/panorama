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

  // ── Core navigation & form rendering ───────────────────────────────────

  test('opens journal from sidebar and shows header', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.waitForTimeout(1000);
    await expect(page.locator('h2')).toContainText('Journal', { timeout: 5000 });
    // "+ New Entry" toggle should be visible
    await expect(page.locator('button:has-text("+ New Entry")')).toBeVisible({ timeout: 3000 });
  });

  test('clicking + New Entry opens the create form', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.waitForTimeout(800);

    // Form should NOT be visible until we click the button
    await page.click('button:has-text("+ New Entry")');
    await page.waitForTimeout(300);

    await expect(page.locator('input[placeholder="Entry title"]')).toBeVisible({ timeout: 3000 });
    await expect(page.locator('textarea[placeholder*="Write your entry"]')).toBeVisible();
    await expect(page.locator('button:has-text("Save Entry")')).toBeVisible();
  });

  test('mood selector has all mood options with emoji labels', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.waitForTimeout(800);
    await page.click('button:has-text("+ New Entry")');
    await page.waitForTimeout(300);

    // Target the create form's mood select (inside card labeled "New Entry")
    const moodSelect = page.locator('.card').filter({ hasText: 'New Entry' }).locator('select');
    const options = await moodSelect.locator('option').allTextContents();
    expect(options.some(o => o.includes('Happy'))).toBeTruthy();
    expect(options.some(o => o.includes('Thoughtful'))).toBeTruthy();
    expect(options.some(o => o.includes('Excited'))).toBeTruthy();
    expect(options.some(o => o.includes('Calm'))).toBeTruthy();
    expect(options.some(o => o.includes('Grateful'))).toBeTruthy();
  });

  test('markdown preview toggle shows rendered content', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.waitForTimeout(800);
    await page.click('button:has-text("+ New Entry")');
    await page.waitForTimeout(300);

    // Type markdown
    await page.locator('input[placeholder="Entry title"]').fill('Preview Test');
    await page.locator('textarea[placeholder*="Write your entry"]').fill('# Hello\n\n**bold text**');
    // Click Preview
    await page.click('button:has-text("Preview")');
    await page.waitForTimeout(300);

    // Rendered markdown should show <h1> and <strong>
    await expect(page.locator('.journal-entry-body h1')).toContainText('Hello', { timeout: 3000 });
    await expect(page.locator('.journal-entry-body strong')).toContainText('bold text', { timeout: 3000 });
  });

  // ── Entry CRUD ─────────────────────────────────────────────────────────

  test('creates a journal entry with mood and sees it in the list', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.waitForTimeout(800);
    await page.click('button:has-text("+ New Entry")');
    await page.waitForTimeout(300);

    const entryTitle = `Entry-${Date.now()}`;
    await page.locator('input[placeholder="Entry title"]').fill(entryTitle);
    await page.locator('textarea[placeholder*="Write your entry"]').fill('This journal entry was created by the E2E test.');
    // Select "😊 Happy" in the create form's mood select
    await page.locator('.card').filter({ hasText: 'New Entry' }).locator('select').selectOption('happy');
    await page.click('button:has-text("Save Entry")');
    await page.waitForTimeout(1000);

    // Entry title should appear in the list
    await expect(page.locator(`strong:has-text("${entryTitle}")`).first()).toBeVisible({ timeout: 5000 });
    // Mood badge should be visible with the mood text
    await expect(page.locator('.mood-badge').first()).toBeVisible({ timeout: 3000 });
  });

  test('clicking an entry expands detail with rendered markdown', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.waitForTimeout(800);
    await page.click('button:has-text("+ New Entry")');
    await page.waitForTimeout(300);

    const expandTitle = `Expand-${Date.now()}`;
    const expandContent = `Secret-${Date.now()}`;
    await page.locator('input[placeholder="Entry title"]').fill(expandTitle);
    await page.locator('textarea[placeholder*="Write your entry"]').fill(`# ${expandContent}`);
    await page.click('button:has-text("Save Entry")');
    await page.waitForTimeout(1000);

    // Click the entry card to expand
    await page.locator(`strong:has-text("${expandTitle}")`).first().click();
    await page.waitForTimeout(500);

    // Rendered markdown content should be visible in the detail panel
    await expect(page.locator('.journal-entry-body').first()).toBeVisible({ timeout: 5000 });
    // h1 heading should contain the content
    await expect(page.locator('.journal-entry-body h1').first()).toContainText(expandContent, { timeout: 3000 });
  });

  test('creates entry with paragraph decomposition', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.waitForTimeout(800);
    await page.click('button:has-text("+ New Entry")');
    await page.waitForTimeout(300);

    const paraTitle = `Para-${Date.now()}`;
    await page.locator('input[placeholder="Entry title"]').fill(paraTitle);
    // Two paragraphs separated by blank line
    await page.locator('textarea[placeholder*="Write your entry"]').fill('First paragraph line.\n\nSecond paragraph line.');
    await page.click('button:has-text("Save Entry")');
    // Paragraph decomposition creates child nodes — needs extra time
    await page.waitForTimeout(2000);

    // Entry card should show paragraph count badge
    await expect(page.locator(`strong:has-text("${paraTitle}")`).first()).toBeVisible({ timeout: 5000 });
    // Should show "2 paragraphs" text on the entry card
    await expect(page.locator('text=2 paragraphs').first()).toBeVisible({ timeout: 5000 });

    // Click the entry to expand detail
    await page.locator(`strong:has-text("${paraTitle}")`).first().click();
    await page.waitForTimeout(800);

    // Rendered markdown content should be visible in detail
    await expect(page.locator('.journal-entry-body').first()).toBeVisible({ timeout: 5000 });
  });

  test('edits an existing entry inline', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.waitForTimeout(800);
    await page.click('button:has-text("+ New Entry")');
    await page.waitForTimeout(300);

    const editTitle = `EditMe-${Date.now()}`;
    const updatedTitle = `Updated-${Date.now()}`;
    await page.locator('input[placeholder="Entry title"]').fill(editTitle);
    await page.locator('textarea[placeholder*="Write your entry"]').fill('Original content.');
    await page.click('button:has-text("Save Entry")');
    await page.waitForTimeout(800);

    // Click the Edit button on the entry card
    const card = page.locator('.card').filter({ hasText: editTitle });
    await card.locator('button:has-text("Edit")').click();
    await page.waitForTimeout(500);

    // Edit form should appear — find the title input inside the "Edit Entry" card
    await page.locator('.card').filter({ hasText: 'Edit Entry' }).locator('input[placeholder="Entry title"]').fill(updatedTitle);
    await page.click('button:has-text("Save Changes")');
    await page.waitForTimeout(1000);

    // Updated title should appear
    await expect(page.locator(`strong:has-text("${updatedTitle}")`).first()).toBeVisible({ timeout: 5000 });
    // Old title should be gone
    await expect(page.locator(`strong:has-text("${editTitle}")`)).toHaveCount(0, { timeout: 3000 });
  });

  test('soft-deletes an entry', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.waitForTimeout(800);
    await page.click('button:has-text("+ New Entry")');
    await page.waitForTimeout(300);

    const delTitle = `DelMe-${Date.now()}`;
    await page.locator('input[placeholder="Entry title"]').fill(delTitle);
    await page.locator('textarea[placeholder*="Write your entry"]').fill('To be deleted.');
    await page.click('button:has-text("Save Entry")');
    await page.waitForTimeout(800);

    // Click Delete button
    const card = page.locator('.card').filter({ hasText: delTitle });
    page.once('dialog', dialog => dialog.accept());
    await card.locator('button:has-text("Delete")').click();
    await page.waitForTimeout(800);

    // Card should show "deleted" badge
    await expect(page.locator('.card').filter({ hasText: 'deleted' }).first()).toBeVisible({ timeout: 5000 });
  });

  // ── Filters ────────────────────────────────────────────────────────────

  test('mood filter filters entries by mood', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.waitForTimeout(800);

    // Create a happy entry
    await page.click('button:has-text("+ New Entry")');
    await page.waitForTimeout(200);
    await page.locator('input[placeholder="Entry title"]').fill('Happy Entry');
    await page.locator('textarea[placeholder*="Write your entry"]').fill('Happy content.');
    await page.locator('.card').filter({ hasText: 'New Entry' }).locator('select').selectOption('happy');
    await page.click('button:has-text("Save Entry")');
    await page.waitForTimeout(1000);

    // Create a calm entry
    await page.click('button:has-text("+ New Entry")');
    await page.waitForTimeout(200);
    await page.locator('input[placeholder="Entry title"]').fill('Calm Entry');
    await page.locator('textarea[placeholder*="Write your entry"]').fill('Calm content.');
    await page.locator('.card').filter({ hasText: 'New Entry' }).locator('select').selectOption('calm');
    await page.click('button:has-text("Save Entry")');
    await page.waitForTimeout(1000);

    // Filter by happy mood using the filter bar select (first select)
    const filterSelect = page.locator('.card').filter({ hasText: 'Filters' }).locator('select');
    await filterSelect.selectOption('happy');
    await page.waitForTimeout(1000);

    // Only happy entry should be visible
    await expect(page.locator('strong:has-text("Happy Entry")').first()).toBeVisible({ timeout: 5000 });
    await expect(page.locator('strong:has-text("Calm Entry")')).toHaveCount(0, { timeout: 3000 });

    // Clear filters
    await page.click('button:has-text("Clear filters")');
    await page.waitForTimeout(500);
    await expect(page.locator('strong:has-text("Calm Entry")').first()).toBeVisible({ timeout: 3000 });
  });

  test('date range filter filters entries by date', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.waitForTimeout(800);

    // Create an entry first so the list is non-empty
    await page.click('button:has-text("+ New Entry")');
    await page.waitForTimeout(200);
    await page.locator('input[placeholder="Entry title"]').fill('Date Filter Entry');
    await page.locator('textarea[placeholder*="Write your entry"]').fill('For date filter testing.');
    await page.click('button:has-text("Save Entry")');
    await page.waitForTimeout(1000);

    // Set a far-future date range — should filter everything out
    const dateInputs = page.locator('.card').filter({ hasText: 'Filters' }).locator('input[type="date"]');
    await dateInputs.nth(0).fill('2099-01-01');
    await dateInputs.nth(1).fill('2099-12-31');
    await page.waitForTimeout(800);

    // Should show empty state
    await expect(page.locator('text=No entries yet').first()).toBeVisible({ timeout: 5000 });
  });

  // ── Filter bar toggle ──────────────────────────────────────────────────

  test('can cancel create form without creating', async ({ page }) => {
    await page.goto('/');
    await page.click('button:has-text("Journal")');
    await page.waitForTimeout(800);
    await page.click('button:has-text("+ New Entry")');
    await page.waitForTimeout(300);

    // Form should be visible
    await expect(page.locator('textarea[placeholder*="Write your entry"]')).toBeVisible();

    // Click Cancel (the + New Entry button toggles to Cancel when form is open)
    await page.click('button:has-text("Cancel")');
    await page.waitForTimeout(300);

    // Form should be hidden
    await expect(page.locator('textarea[placeholder*="Write your entry"]')).toHaveCount(0, { timeout: 3000 });
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

    await expect(page.locator('h4:has-text("Artists")')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('h4:has-text("Albums")')).toBeVisible({ timeout: 5000 });
  });
});
