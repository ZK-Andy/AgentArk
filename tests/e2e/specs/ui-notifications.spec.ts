import { test, expect } from "@playwright/test";

test.describe("Notifications UI", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page.waitForSelector("text=AgentArk", { timeout: 15_000 });
  });

  test("mark all read clears badge", async ({ page }) => {
    const bell = page.locator('button[aria-label="Open notifications"]');

    // Open notification popover
    await bell.click();
    await page.waitForSelector("text=Mark all read", { timeout: 5_000 });

    // Click mark all read
    await page.locator("text=Mark all read").click();

    // Close and reopen popover
    await page.keyboard.press("Escape");
    await page.waitForTimeout(1000);

    // Badge should now show 0 or not exist
    const badge = bell.locator(".MuiBadge-badge");
    const badgeCount = await badge.count();
    if (badgeCount > 0) {
      const text = await badge.textContent();
      // Badge should be 0 or invisible
      expect(text === "0" || text === "").toBeTruthy();
    }
  });

  test("clicking notification opens detail drawer", async ({ page }) => {
    const bell = page.locator('button[aria-label="Open notifications"]');
    await bell.click();
    await page.waitForSelector("text=Mark all read", { timeout: 5_000 });

    // Find notification items inside the popover
    const popover = page.locator(".MuiPopover-root");
    const notifItems = popover.locator(".MuiListItemButton-root");
    const count = await notifItems.count();
    if (count === 0) {
      test.skip();
      return;
    }

    await notifItems.first().click();

    // Detail drawer should open
    await expect(
      page.locator(".MuiDrawer-root").first()
    ).toBeVisible({ timeout: 5_000 });
  });
});
