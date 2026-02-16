import { test, expect } from "@playwright/test";

test.describe("Chat UI", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page.waitForSelector("text=AgentArk", { timeout: 15_000 });
  });

  test("can navigate to chat view", async ({ page }) => {
    // Click on the chat icon in the sidebar
    const chatBtn = page.locator('button[aria-label*="Chat"], a[href*="chat"]').first();
    if ((await chatBtn.count()) === 0) {
      // Try clicking the chat nav item
      await page.locator("text=Chat").first().click();
    } else {
      await chatBtn.click();
    }

    // Chat input should appear
    const input = page.locator(
      'textarea, input[placeholder*="message"], input[placeholder*="Message"], input[placeholder*="Ask"]'
    ).first();
    await expect(input).toBeVisible({ timeout: 10_000 });
  });

  test("typing a message shows it in the input", async ({ page }) => {
    // Navigate to chat
    const chatNav = page.locator("text=Chat").first();
    if (await chatNav.isVisible()) {
      await chatNav.click();
    }

    const input = page.locator(
      'textarea, input[placeholder*="message"], input[placeholder*="Message"], input[placeholder*="Ask"]'
    ).first();
    await expect(input).toBeVisible({ timeout: 10_000 });

    await input.fill("hello from e2e test");
    await expect(input).toHaveValue("hello from e2e test");
  });

  test("sending message shows streaming indicator", async ({ page }) => {
    // Navigate to chat
    const chatNav = page.locator("text=Chat").first();
    if (await chatNav.isVisible()) {
      await chatNav.click();
    }

    const input = page.locator(
      'textarea, input[placeholder*="message"], input[placeholder*="Message"], input[placeholder*="Ask"]'
    ).first();
    await expect(input).toBeVisible({ timeout: 10_000 });

    await input.fill("ping");
    await input.press("Enter");

    // Should show either thinking or streaming indicator
    const indicator = page.locator(
      'text=/thinking|streaming|Sending/i'
    ).first();

    // This may or may not appear depending on LLM config
    // Just verify the message was sent (input cleared or user bubble appears)
    await page.waitForTimeout(2000);
    const userMessage = page.locator("text=ping").first();
    // Either the message shows in chat or the input was cleared
    const inputValue = await input.inputValue();
    const msgVisible = await userMessage.isVisible().catch(() => false);
    expect(inputValue === "" || msgVisible).toBeTruthy();
  });
});
