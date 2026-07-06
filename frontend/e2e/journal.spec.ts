import { test, expect } from "./fixtures";

test.describe("Journal Plugin UI", () => {
  test("opens journal and shows sidebar with Today button", async ({
    page,
  }) => {
    await page.goto("/");
    await page.click('a:has-text("Journal")');

    await expect(page.locator(".journal-sidebar")).toBeVisible();
    await expect(page.locator('button:has-text("Today")')).toBeVisible();
  });

  test("can create a new page", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Journal")');
    const pageTitle = `Page-${Date.now()}`;
    await page.click('button:has-text("+ New Page")');

    await page.locator(".journal-new-title-input").fill(pageTitle);
    await page
      .locator(".journal-new-textarea")
      .fill("Hello world from the new journal!");
    await page.click('button:has-text("Create Page")');
    // Page title should appear in sidebar
    await expect(
      page.locator(`.journal-page-link-title:has-text("${pageTitle}")`).first(),
    ).toBeVisible();
    // Content should be visible in the main area
    await expect(
      page.locator("text=Hello world from the new journal!"),
    ).toBeVisible();
  });

  test("page appears in sidebar list", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Journal")');
    // Create a page
    const pageTitle = `Sidebar-${Date.now()}`;
    await page.click('button:has-text("+ New Page")');

    await page.locator(".journal-new-title-input").fill(pageTitle);
    await page.locator(".journal-new-textarea").fill("test");
    await page.click('button:has-text("Create Page")');
    // Should appear in sidebar
    await expect(page.locator(".journal-page-list")).toContainText(pageTitle);
  });

  test("clicking a page in sidebar shows its content", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Journal")');
    // Create a page with distinct content
    const pageTitle = `Click-${Date.now()}`;
    const pageContent = `Distinct-${Date.now()}`;
    await page.click('button:has-text("+ New Page")');

    await page.locator(".journal-new-title-input").fill(pageTitle);
    await page.locator(".journal-new-textarea").fill(pageContent);
    await page.click('button:has-text("Create Page")');
    // Click another nav item to navigate away, then click back
    await page.click('a:has-text("Nodes")');

    await page.click('a:has-text("Journal")');
    // Click the page in the sidebar
    await page
      .locator(`.journal-page-link-title:has-text("${pageTitle}")`)
      .click();
    // Content should be visible
    await expect(page.locator(".journal-main")).toContainText(pageContent);
  });

  test("inline block editing works", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Journal")');
    // Create a page
    await page.click('button:has-text("+ New Page")');

    const pageTitle = `Edit-${Date.now()}`;
    await page.locator(".journal-new-title-input").fill(pageTitle);
    await page.locator(".journal-new-textarea").fill("Original content");
    await page.click('button:has-text("Create Page")');
    // Click the block content area to edit
    await page.locator(".journal-block-content").first().click();
    // Textarea should appear with current content
    const textarea = page.locator(".journal-block-textarea");
    await expect(textarea).toBeVisible();
    // Edit content
    await textarea.fill("Edited content");
    await page.click('button:has-text("Save")');
    // Updated content should be visible
    await expect(page.locator(".journal-main")).toContainText("Edited content");
  });

  test("can delete a page", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Journal")');
    // Create a page to delete
    await page.click('button:has-text("+ New Page")');

    const delTitle = `Del-${Date.now()}`;
    await page.locator(".journal-new-title-input").fill(delTitle);
    await page.locator(".journal-new-textarea").fill("to be deleted");
    await page.click('button:has-text("Create Page")');

    // Wait for the page to appear in sidebar and be selected
    await expect(
      page.locator(`.journal-page-link-title:has-text("${delTitle}")`).first(),
    ).toBeVisible();

    // Wait for the page header with delete button to render
    await expect(
      page.locator('.journal-page-meta button[title="Delete page"]'),
    ).toBeVisible();

    // Click the delete button in the page header
    page.once("dialog", (dialog) => dialog.accept());
    await page
      .locator('.journal-page-meta button[title="Delete page"]')
      .click();

    // Page should show as soft-deleted in sidebar (strikethrough)
    await expect(
      page.locator(".journal-page-link.deleted").first(),
    ).toBeVisible({ timeout: 10_000 });
  });

  test("adding a child block to a page", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Journal")');
    // Create a page
    await page.click('button:has-text("+ New Page")');

    await page.locator(".journal-new-title-input").fill("Parent Page");
    await page.locator(".journal-new-textarea").fill("Parent content");
    await page.click('button:has-text("Create Page")');
    // Add a child block
    const childContent = `Child-${Date.now()}`;
    const newBlockTextarea = page
      .locator(".journal-new-block .journal-new-textarea")
      .first();
    await newBlockTextarea.fill(childContent);
    await newBlockTextarea.press("Enter");
    // Child block should appear
    await expect(page.locator(".journal-main")).toContainText(childContent);
  });

  test("Today button shows journal page for current date", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Journal")');
    // Click Today button
    await page.click('button:has-text("Today")');
    // Should show a date badge with today's date
    const today = new Date().toISOString().slice(0, 10);
    await expect(page.locator(".journal-date-badge")).toContainText(today);
  });
});
