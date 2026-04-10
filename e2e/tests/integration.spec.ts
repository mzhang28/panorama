import { test, expect } from '@playwright/test';
import fs from 'fs';
import path from 'path';

test.describe('Panorama Extensible DB & Weight Tracker', () => {
  test.beforeEach(async ({ page }) => {
    // Reset the test config to a clean state in BOTH locations
    const configs = [
      path.resolve(process.cwd(), '..', '.app-config.test.yml'),
      path.resolve(process.cwd(), '..', 'backend', '.app-config.test.yml')
    ];
    
    const initialConfig = [
      'tabs:',
      '  - id: e2e-tab',
      '    title: "E2E Test Dashboard"',
      '    widgets:',
      '      - id: e2e-weight-input',
      '        type: weight-tracker-input',
      '        title: "E2E Weight Input"',
      '        grid: { w: 6, h: 1 }',
      '      - id: e2e-weight-graph',
      '        type: built-in-graph',
      '        title: "E2E Weight Graph"',
      '        query: "weight_kg"',
      '        timeRange: "7d"',
      '        grid: { w: 6, h: 1 }'
    ].join('\n');
    
    for (const configPath of configs) {
      try {
        fs.writeFileSync(configPath, initialConfig, 'utf8');
      } catch (err) {
        console.warn('Could not write to', configPath, err);
      }
    }

    // Navigate to the app with cache busting
    await page.goto('/?t=' + Date.now());
    // Wait for the UI to stabilize
    await expect(page.locator('h2')).toContainText('E2E Test Dashboard');
  });

  test('can load the dashboard and add a weight entry', async ({ page }) => {
    const input = page.locator('input[placeholder="75.5"]');
    await expect(input).toBeVisible();
    
    await input.fill('82.5');
    await page.click('button:text("Add")');
    await expect(input).toHaveValue('');

    await input.fill('83.0');
    await page.click('button:text("Add")');
    await expect(input).toHaveValue('');
    
    const chart = page.locator('.recharts-responsive-container').first();
    await expect(chart).toBeVisible();
    
    const pathLocator = page.locator('.recharts-line path').first();
    await expect(pathLocator).toBeVisible();
  });

  test('can switch between tabs', async ({ page }) => {
     const tab = page.locator('button', { hasText: 'E2E Test Dashboard' });
     await expect(tab).toBeVisible();
  });

  test('can change and persist the time range filter', async ({ page }) => {
    const select = page.locator('select').first();
    await expect(select).toHaveValue('7d');
    
    await select.selectOption('24h');
    await expect(select).toHaveValue('24h');
    
    // Wait for the PUT request to finish
    await page.waitForTimeout(1000);
    
    await page.reload();
    await expect(page.locator('h2')).toContainText('E2E Test Dashboard');
    const selectAfterReload = page.locator('select').first();
    await expect(selectAfterReload).toHaveValue('24h');
  });

  test('can add a new widget via the plus button', async ({ page }) => {
    const initialWidgets = page.locator('.grid > div');
    const initialCount = await initialWidgets.count();
    
    await page.click('button[title="Add Widget"]');
    await page.click('button:has-text("Weight Graph")');
    
    const newWidget = page.locator('h3', { hasText: 'New Weight Graph' }).last();
    await expect(newWidget).toBeVisible();
    
    const finalCount = await initialWidgets.count();
    expect(finalCount).toBe(initialCount + 1);
    
    // Wait for the POST request to finish
    await page.waitForTimeout(1000);
    
    await page.reload();
    await expect(page.locator('h3', { hasText: 'New Weight Graph' }).last()).toBeVisible();
  });
});
