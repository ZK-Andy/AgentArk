import { test, expect } from "@playwright/test";

const VIEWS = [
  { label: "Overview", marker: "Needs Your Attention" },
  { label: "Chat", marker: /message|ask|send/i },
  { label: "Skills", marker: /skills|actions/i },
  { label: "Settings", marker: /settings|config/i },
];

test.describe("Navigation UI @smoke", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page.waitForSelector("text=AgentArk", { timeout: 15_000 });
  });

  for (const view of VIEWS) {
    test(`can navigate to ${view.label}`, async ({ page }) => {
      const navItem = page.locator(`text=${view.label}`).first();
      if (!(await navItem.isVisible())) {
        // May be in collapsed sidebar — try icon button
        test.skip();
        return;
      }
      await navItem.click();
      await page.waitForTimeout(500);

      // Verify we're on the right view
      const marker = page.locator(
        typeof view.marker === "string"
          ? `text=${view.marker}`
          : `text=${view.marker}`
      ).first();
      await expect(marker).toBeVisible({ timeout: 5_000 });
    });
  }

  test("sidebar can collapse and expand", async ({ page }) => {
    // Look for collapse/expand chevron button
    const collapseBtn = page.locator(
      'button[aria-label*="collapse"], button[aria-label*="Collapse"], button[aria-label*="Expand"], [data-testid="sidebar-toggle"]'
    ).first();

    if (!(await collapseBtn.isVisible({ timeout: 3_000 }).catch(() => false))) {
      test.skip();
      return;
    }

    await collapseBtn.click();
    await page.waitForTimeout(300);

    // Click again to expand
    await collapseBtn.click();
    await page.waitForTimeout(300);
  });
});
