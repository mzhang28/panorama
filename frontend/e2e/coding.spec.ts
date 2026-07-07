import { expect, test } from "./fixtures";

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

  // Helper: POST a heartbeat directly via the plugin API
  async function sendHeartbeat(page: any) {
    // Navigate first so fetch() has a base URL to resolve against
    await page.goto("/");
    await page.waitForTimeout(100);
    await page.evaluate(async () => {
      const res = await fetch("/plugin/io.mzhang.panorama.coding/heartbeat", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          entity: "/src/e2e.ts",
          type: "file",
          category: "coding",
          project: "e2e-test",
          language: "TypeScript",
          editor: "VSCode",
          operating_system: "Linux",
          time: Date.now() / 1000,
          duration: 300,
        }),
      });
      // Wait for the response so the server has processed the heartbeat
      await res.json();
    });
  }

  test("WakaTime stats endpoint returns valid response", async ({ page }) => {
    await sendHeartbeat(page);
    const result = await page.evaluate(async () => {
      const res = await fetch(
        "/plugin/io.mzhang.panorama.coding/compat/wakatime/v1/users/current/stats/7d",
      );
      const text = await res.text();
      return { status: res.status, body: text };
    });
    const resp = JSON.parse(result.body);
    expect(resp.data).toBeDefined();
    expect(resp.data.total_seconds).toBeGreaterThan(0);
    expect(resp.data.status).toBe("ok");
    expect(Array.isArray(resp.data.projects)).toBe(true);
    expect(Array.isArray(resp.data.languages)).toBe(true);
    expect(Array.isArray(resp.data.editors)).toBe(true);
    expect(Array.isArray(resp.data.operating_systems)).toBe(true);
    if (resp.data.projects.length > 0) {
      const p = resp.data.projects[0];
      expect(p.name).toBeDefined();
      expect(p.total_seconds).toBeGreaterThan(0);
      expect(p.percent).toBeGreaterThan(0);
      expect(p.digital).toBeDefined();
      expect(p.text).toBeDefined();
    }
  });

  test("WakaTime all_time_since_today returns valid response", async ({
    page,
  }) => {
    await sendHeartbeat(page);
    const resp = await page.evaluate(async () => {
      const res = await fetch(
        "/plugin/io.mzhang.panorama.coding/compat/wakatime/v1/users/current/all_time_since_today",
      );
      return res.json();
    });
    expect(resp.data).toBeDefined();
    expect(resp.data.total_seconds).toBeGreaterThan(0);
    expect(resp.data.text).toBeDefined();
    expect(resp.data.is_up_to_date).toBe(true);
  });

  test("WakaTime summaries returns per-day array", async ({ page }) => {
    await sendHeartbeat(page);
    const resp = await page.evaluate(async () => {
      const res = await fetch(
        "/plugin/io.mzhang.panorama.coding/compat/wakatime/v1/users/current/summaries?range=7d",
      );
      return res.json();
    });
    expect(Array.isArray(resp.data)).toBe(true);
    expect(resp.cumulative_total).toBeDefined();
    expect(resp.cumulative_total.seconds).toBeGreaterThan(0);
  });

  test("SVG badge returns valid XML", async ({ page }) => {
    await sendHeartbeat(page);
    const data = await page.evaluate(async () => {
      const res = await fetch(
        "/plugin/io.mzhang.panorama.coding/badge/current/interval:7d",
      );
      return res.json();
    });
    expect(data.svg).toBeDefined();
    expect(data.svg).toContain("<svg");
    expect(data.svg).toContain("</svg>");
  });
});
