import { test, expect } from "./fixtures";

test.describe("File Manager Plugin UI", () => {
  test("opens file manager and shows upload zone", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("File Manager")');

    await expect(page.locator("h2")).toContainText("File Manager");
    await expect(page.locator("text=Drop files here")).toBeVisible();
  });

  test("shows file count and list area", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("File Manager")');
    await expect(
      page.locator("text=No files uploaded yet").first(),
    ).toBeVisible();
  });
});
