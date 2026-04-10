import { test, expect } from '@playwright/test';

test.describe('Panorama Fullstack Integration', () => {
  test('frontend can greet using ConnectRPC and see result from DB API', async ({ page }) => {
    await page.goto('/');
    
    // Check initial state
    await expect(page.locator('h1')).toContainText('Panorama Fullstack');
    
    // Fill the name input
    const testName = `E2E-User-${Date.now()}`;
    await page.fill('input[placeholder="Enter name"]', testName);
    
    // Click Greet (ConnectRPC flow)
    await page.click('button:text("Greet")');
    
    // Verify that the greeting appears in the list (API flow + DB)
    const listItem = page.locator('li', { hasText: testName });
    await expect(listItem).toBeVisible();
    await expect(listItem).toContainText(`Hello ${testName} from ConnectRPC!`);
  });
});
