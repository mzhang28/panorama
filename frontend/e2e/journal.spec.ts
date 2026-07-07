import { expect, test } from "./fixtures";

// Tiptap editor renders a contenteditable div with this class.
const NEW_BLOCK_EDITOR = ".journal-new-block .tiptap-editor-prose";
const INLINE_EDITOR = ".journal-block-editor .tiptap-editor-prose";

test.describe("Journal Plugin UI", () => {
  test("opens journal and shows sidebar with Today button", async ({
    page,
  }) => {
    await page.goto("/");
    await page.click('a:has-text("Journal")');
    await expect(page.getByTestId("journal-sidebar")).toBeVisible();
    await expect(page.getByTestId("journal-today-btn")).toBeVisible();
  });

  test("can create a new page", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Journal")');
    const pageTitle = `Page-${Date.now()}`;
    await page.getByTestId("journal-new-page-btn").click();
    await page.getByTestId("journal-new-title-input").fill(pageTitle);
    await page
      .locator(NEW_BLOCK_EDITOR)
      .fill("Hello world from the new journal!");
    await page.getByTestId("journal-create-page-btn").click();
    // Page title should appear in sidebar
    await expect(
      page.locator(`.journal-page-link-title:has-text("${pageTitle}")`).first(),
    ).toBeVisible();
    await expect(
      page.locator("text=Hello world from the new journal!"),
    ).toBeVisible();
  });

  test("page appears in sidebar list", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Journal")');
    const pageTitle = `Sidebar-${Date.now()}`;
    await page.getByTestId("journal-new-page-btn").click();
    await page.getByTestId("journal-new-title-input").fill(pageTitle);
    await page.locator(NEW_BLOCK_EDITOR).fill("test");
    await page.getByTestId("journal-create-page-btn").click();
    await expect(page.getByTestId("journal-page-list")).toContainText(
      pageTitle,
    );
  });

  test("clicking a page in sidebar shows its content", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Journal")');
    const pageTitle = `Click-${Date.now()}`;
    const pageContent = `Distinct-${Date.now()}`;
    await page.getByTestId("journal-new-page-btn").click();
    await page.getByTestId("journal-new-title-input").fill(pageTitle);
    await page.locator(NEW_BLOCK_EDITOR).fill(pageContent);
    await page.getByTestId("journal-create-page-btn").click();

    // Wait for the new page to appear in the main area
    await expect(page.locator(".journal-main")).toContainText(pageContent);

    // Navigate away and back.  page.goto("/nodes") is fine for leaving
    // the Journal, but page.goto("/app/io.mzhang.panorama.journal")
    // doesn't load the plugin—use SPA navigation to return instead.
    await page.goto("/nodes");
    await expect(page.locator("table")).toBeVisible();
    await page.goto("/");
    await page.click('a:has-text("Journal")');
    await expect(page.getByTestId("journal-sidebar")).toBeVisible();
    await expect(page.getByTestId("journal-page-list")).toContainText(
      pageTitle,
      {
        timeout: 10_000,
      },
    );
  });

  test("inline block editing works", async ({ page }) => {
    page.on("console", (msg) => console.log("BROWSER LOG:", msg.text()));
    await page.goto("/");
    await page.click('a:has-text("Journal")');
    await page.getByTestId("journal-new-page-btn").click();
    const pageTitle = `Edit-${Date.now()}`;
    await page.getByTestId("journal-new-title-input").fill(pageTitle);
    await page.locator(NEW_BLOCK_EDITOR).fill("Original content");
    await page.getByTestId("journal-create-page-btn").click();
    await page.locator(".journal-block-content").first().click();
    const editor = page.locator(INLINE_EDITOR);
    await expect(editor).toBeVisible();
    await editor.fill("Edited content");
    await page.click('button:has-text("Save")');
    await expect(page.locator(".journal-main")).toContainText("Edited content");
  });

  test("can delete a page", async ({ page }) => {
    page.on("console", (msg) => console.log("BROWSER LOG:", msg.text()));
    await page.goto("/");
    await page.click('a:has-text("Journal")');
    await page.getByTestId("journal-new-page-btn").click();
    const delTitle = `Del-${Date.now()}`;
    await page.getByTestId("journal-new-title-input").fill(delTitle);
    await page.locator(NEW_BLOCK_EDITOR).fill("to be deleted");
    await page.getByTestId("journal-create-page-btn").click();

    // Wait for the page to appear in sidebar and be selected
    await expect(
      page.locator(`.journal-page-link-title:has-text("${delTitle}")`).first(),
    ).toBeVisible();
    await expect(page.getByTestId("journal-delete-btn")).toBeVisible();

    // Click delete — wait for the subsequent listPages refetch so the
    // sidebar renders the soft-deleted state (.deleted CSS class).
    const pagesRefetch = page.waitForResponse(
      (resp) =>
        resp.url().includes("/plugin/io.mzhang.panorama.journal/pages") &&
        resp.request().method() === "GET",
      { timeout: 15_000 },
    );
    page.once("dialog", (dialog) => dialog.accept());
    await page.getByTestId("journal-delete-btn").click();
    await pagesRefetch;

    await expect(
      page.locator(".journal-page-link.deleted").first(),
    ).toBeVisible({ timeout: 10_000 });
  });

  test("adding a child block to a page", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Journal")');
    await page.getByTestId("journal-new-page-btn").click();
    await page.getByTestId("journal-new-title-input").fill("Parent Page");
    await page.locator(NEW_BLOCK_EDITOR).fill("Parent content");
    await page.getByTestId("journal-create-page-btn").click();
    const childContent = `Child-${Date.now()}`;
    const childEditor = page.locator(NEW_BLOCK_EDITOR).first();
    await childEditor.fill(childContent);
    await childEditor.press("Enter");
    await expect(page.locator(".journal-main")).toContainText(childContent);
  });

  test("Today button shows journal page for current date", async ({ page }) => {
    await page.goto("/");
    await page.click('a:has-text("Journal")');
    await page.getByTestId("journal-today-btn").click();
    const today = new Date().toISOString().slice(0, 10);
    await expect(page.getByTestId("journal-date-badge")).toContainText(today);
  });
});
