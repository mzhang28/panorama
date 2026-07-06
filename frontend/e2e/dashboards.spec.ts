import { expect, test } from "./fixtures";

test.describe("Dashboards Plugin UI", () => {
  test("opens dashboards and shows home dashboard with panels", async ({
    page,
  }) => {
    await page.goto("/");
    await page.click('a:has-text("Dashboards")');

    await expect(page.locator("h2")).toBeVisible();
    await expect(page.locator("text=Per Project")).toBeVisible();
  });

  test("has time range presets and refresh selector", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Dashboards")');
    await expect(page.locator('button:has-text("Last 24h")')).toBeVisible();
    await expect(page.locator('button:has-text("Custom...")')).toBeVisible();
    const selects = page.locator("select");
    await expect(selects.first()).toBeVisible();
  });

  test("shows panel grid with leaderboard and timeseries panels", async ({
    page,
  }) => {
    await page.goto("/");
    await page.click('a:has-text("Dashboards")');
    await expect(page.locator("text=Per Project")).toBeVisible();
    await expect(page.locator("text=Per Language")).toBeVisible();
  });

  test("has Edit button to open dashboard builder", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Dashboards")');
    const editBtn = page.locator('button:has-text("Edit")');
    await expect(editBtn).toBeVisible();
  });

  test("can open builder modal to edit dashboard", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Dashboards")');
    await page.click('button:has-text("Edit")');

    await expect(page.locator("text=Panels")).toBeVisible();
  });
});
