import { expect, test } from "./fixtures";

test.describe("Restaurant Rankings Plugin UI", () => {
  test("opens restaurant rankings and shows explanation", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Restaurant Rankings")');

    await expect(page.locator("h2")).toContainText("Restaurant Rankings");
    await expect(page.locator("text=partial order")).toBeVisible();
    await expect(page.locator("text=pairwise comparisons")).toBeVisible();
    await expect(page.locator("text=star ratings")).toBeVisible();
  });

  test("can add a restaurant", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Restaurant Rankings")');
    await page
      .locator('input[placeholder="Restaurant name"]')
      .fill("E2E Sushi Place");
    await page.locator('input[placeholder="Cuisine"]').fill("Sushi");
    await page.locator('input[placeholder="Location"]').fill("Test District");
    await page.click('button:has-text("+ Add")');
  });

  test("comparison form shows restaurant options after adding", async ({
    page,
  }) => {
    await page.goto("/");
    await page.click('a:has-text("Restaurant Rankings")');
    await page.locator('input[placeholder="Restaurant name"]').fill("Ramen A");
    await page.locator('input[placeholder="Cuisine"]').fill("Ramen");
    await page.locator('input[placeholder="Location"]').fill("Shibuya");
    await page.click('button:has-text("+ Add")');
    await expect(
      page.locator('input[placeholder="Restaurant name"]'),
    ).toBeEmpty();
    await page.locator('input[placeholder="Restaurant name"]').fill("Ramen B");
    await page.locator('input[placeholder="Cuisine"]').fill("Ramen");
    await page.locator('input[placeholder="Location"]').fill("Shinjuku");
    await page.click('button:has-text("+ Add")');
    await expect(
      page.locator('input[placeholder="Restaurant name"]'),
    ).toBeEmpty();
    const betterSelect = page.locator("select").first();
    await expect(betterSelect).toContainText("Ramen A");
    await expect(betterSelect).toContainText("Ramen B");
  });

  test("can record a comparison between two restaurants", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Restaurant Rankings")');
    await page
      .locator('input[placeholder="Restaurant name"]')
      .fill("Pizza Place");
    await page.locator('input[placeholder="Cuisine"]').fill("Italian");
    await page.locator('input[placeholder="Location"]').fill("Test");
    await page.click('button:has-text("+ Add")');
    await page
      .locator('input[placeholder="Restaurant name"]')
      .fill("Burger Joint");
    await page.locator('input[placeholder="Cuisine"]').fill("American");
    await page.locator('input[placeholder="Location"]').fill("Test");
    await page.click('button:has-text("+ Add")');
    await page.locator("select").first().selectOption({ index: 1 });
    await page.locator("select").nth(1).selectOption({ index: 2 });
    await page.click('button:has-text("Compare")');
    await expect(page.locator('strong:has-text("Tier")').first()).toBeVisible();
  });
});
