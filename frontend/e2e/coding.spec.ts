import { test, expect } from "./fixtures";

test.describe("Coding Activity Plugin UI", () => {
  test("opens coding activity and shows heartbeat form", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Coding Activity")');

    await expect(page.locator("h2")).toContainText("Coding Activity");
    await expect(page.locator("textarea")).toBeVisible();
    await expect(
      page.locator('button:has-text("Send Heartbeat")'),
    ).toBeVisible();
  });

  test("can send a heartbeat and see stats panels", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Coding Activity")');
    await page.click('button:has-text("Send Heartbeat")');
    // Should show the leaderboard section (even if empty initially)
    await expect(page.locator("text=Per Project")).toBeVisible();
  });

  test("heartbeat form is pre-populated with JSON", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Coding Activity")');
    const textarea = page.locator("textarea");
    const content = await textarea.inputValue();
    expect(content).toContain("entity");
    expect(content).toContain("project");
    expect(content).toContain("Rust");
  });
});
