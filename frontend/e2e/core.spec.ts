import { test, expect } from "./fixtures";

test.describe("Panorama Core UI", () => {
  test("page loads with title and sidebar", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".app-shell-title")).toContainText("Panorama");
    await expect(page.locator(".app-shell-sidebar")).toBeVisible();
  });

  test("sidebar has navigation links", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator('a:has-text("Nodes")').first()).toBeVisible();
    await expect(page.locator('a:has-text("Schemas")').first()).toBeVisible();
    await expect(page.locator('a:has-text("Plugins")').first()).toBeVisible();
  });

  test("sidebar has Installed Apps section", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator('h3:has-text("Installed Apps")')).toBeVisible();
  });

  test("can navigate between views", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByRole("heading", { name: "Nodes" })).toBeVisible();
    await page.click('a:has-text("Schemas")');
    await expect(page.locator("h2")).toContainText("Schemas");
    await page.click('a:has-text("Nodes")');
    await expect(page.getByRole("heading", { name: "Nodes" })).toBeVisible();
  });
});

test.describe("Node Explorer Home", () => {
  test("shows nodes table with search", async ({ page }) => {
    await page.goto("/");
    await expect(
      page.locator('input[placeholder="Search nodes..."]'),
    ).toBeVisible();
    await expect(page.locator("table")).toBeVisible();
  });

  test("shows stats bar with node counts", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Total Nodes", { exact: true })).toBeVisible();
    await expect(page.getByText("Schemas", { exact: true })).toBeVisible();
  });

  test("shows activity chart", async ({ page }) => {
    await page.goto("/");
    await expect(
      page.locator('h3:has-text("Activity Timeline")'),
    ).toBeVisible();
  });

  test("search filters the node table", async ({ page }) => {
    await page.goto("/");
    const searchInput = page.locator('input[placeholder="Search nodes..."]');
    await searchInput.fill("nonexistent-node-xyz");
    // Pagination should show filtered count
    await expect(page.locator("text=0 of")).toBeVisible();
  });
});

test.describe("Schema Viewer", () => {
  test("shows system schemas", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Schemas")');
    await expect(page.locator("h2")).toContainText("Schemas");
    await expect(page.locator('strong:has-text("NodeTime")')).toBeVisible();
    await expect(page.locator('strong:has-text("NodeInfo")')).toBeVisible();
  });

  test("shows schema fields with namespaces", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Schemas")');
    await expect(page.locator("text=system:node_time").first()).toBeVisible();
    await expect(page.locator("text=system:node_title").first()).toBeVisible();
  });
});

test.describe("Plugin Panel", () => {
  test("plugins view shows plugin browser", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Plugins")');
    await expect(page.locator("h2")).toContainText("Plugins");
    await expect(
      page.locator("text=Plugins are third-party apps"),
    ).toBeVisible();
  });
});
