import { test, expect } from "./fixtures";

test.describe("Trip Planner Plugin UI", () => {
  test("opens trip planner and creates a trip", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Trip Planner")');

    await expect(page.locator("h2")).toContainText("Trip Planner");

    const tripName = `Trip-${Date.now()}`;
    await page.locator('input[placeholder="Trip name"]').fill(tripName);
    await page.click('button:has-text("+ Trip")');
    await expect(
      page.locator(`button:has-text("${tripName}")`).first(),
    ).toBeVisible();
  });

  test("shows Map Locations section", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Trip Planner")');

    await expect(page.locator("text=Map Locations")).toBeVisible();
  });

  test("selecting a trip shows add-event form", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Trip Planner")');
    await page
      .locator('input[placeholder="Trip name"]')
      .fill("Event Test Trip");
    await page.click('button:has-text("+ Trip")');
    await page.click('button:has-text("Event Test Trip")');
    await expect(page.locator('input[placeholder="Event name"]')).toBeVisible();
    await expect(page.locator('input[placeholder="Latitude"]')).toBeVisible();
    await expect(page.locator('input[placeholder="Longitude"]')).toBeVisible();
    await expect(
      page.locator('input[placeholder="Location name"]'),
    ).toBeVisible();
  });

  test("can add an event with geo coordinates", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Trip Planner")');
    await page.locator('input[placeholder="Trip name"]').fill("Geo Trip");
    await page.click('button:has-text("+ Trip")');
    await page.click('button:has-text("Geo Trip")');
    await page.locator('input[placeholder="Event name"]').fill("Tokyo Tower");
    await page.locator('input[placeholder="Latitude"]').fill("35.6586");
    await page.locator('input[placeholder="Longitude"]').fill("139.7454");
    await page
      .locator('input[placeholder="Location name"]')
      .fill("Minato, Tokyo");
    await page.click('button:has-text("+ Event")');
    await expect(page.locator('strong:has-text("Tokyo Tower")')).toBeVisible();
    await expect(page.locator("text=Minato, Tokyo").first()).toBeVisible();
  });
});
