import { test, expect } from "@playwright/test";

test.describe("Dashboard UI @smoke", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page.waitForSelector('img[alt="AgentArk"]', { timeout: 15_000 });
  });

  test("renders welcome hero with logo and typewriter", async ({ page }) => {
    const logo = page.locator('img[alt="AgentArk"]').nth(1);
    await expect(logo).toBeVisible();

    const hero = page.locator(".MuiCard-root").first();
    await expect(hero).toBeVisible();
  });

  test("welcome hero does not resize during typewriter", async ({ page }) => {
    // Get the hero card
    const heroCard = page.locator(".MuiCard-root").first();
    await expect(heroCard).toBeVisible();

    // Measure initial height
    const initialBox = await heroCard.boundingBox();
    expect(initialBox).not.toBeNull();

    // Wait for typing to progress
    await page.waitForTimeout(2000);

    // Measure again — height should be the same (no layout shift)
    const laterBox = await heroCard.boundingBox();
    expect(laterBox).not.toBeNull();
    expect(laterBox!.height).toBe(initialBox!.height);
  });

  test("status bar is present", async ({ page }) => {
    // The status bar component contains memory/skills/pending stats
    const statsText = page.locator("text=/memories|skills|pending/i").first();
    await expect(statsText).toBeVisible({ timeout: 10_000 });
  });

  test("needs attention section is visible", async ({ page }) => {
    const attention = page.locator("text=Needs Your Attention");
    await expect(attention).toBeVisible();
  });

  test("notification bell icon exists", async ({ page }) => {
    const bell = page.locator('button[aria-label="Open notifications"]');
    await expect(bell).toBeVisible();
  });

  test("singular workspace aliases canonicalize to real pages", async ({ page }) => {
    await page.goto("/ui/task");
    await expect(page).toHaveURL(/\/ui\/tasks$/);

    await page.goto("/ui/app");
    await expect(page).toHaveURL(/\/ui\/apps$/);

    await page.goto("/ui/watcher");
    await expect(page).toHaveURL(/\/ui\/watchers$/);
  });

  test("clicking notification bell opens popover", async ({ page }) => {
    const bell = page.locator('button[aria-label="Open notifications"]');
    await bell.click();

    const popover = page.locator("text=Notifications").last();
    await expect(popover).toBeVisible();

    const markAllBtn = page.locator("text=Mark all read");
    await expect(markAllBtn).toBeVisible();
  });
});
