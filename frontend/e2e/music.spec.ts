import { expect, test } from "./fixtures";

test.describe("Music Library Plugin UI", () => {
  test("opens music library and shows upload section", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Music Library")');

    await expect(page.locator("h2")).toContainText("Music Library");
    await expect(page.locator("text=Upload Music")).toBeVisible();
  });

  test("shows artists and albums sections", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Music Library")');
    await expect(page.locator('h4:has-text("Artists")')).toBeVisible();
    await expect(page.locator('h4:has-text("Albums")')).toBeVisible();
  });
});
