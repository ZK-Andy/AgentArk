import { test, expect } from "@playwright/test";

test.describe("Health & Status @smoke @api", () => {
  test("GET /health returns OK", async ({ request }) => {
    const res = await request.get("/health");
    expect(res.status()).toBe(200);
    expect(await res.text()).toBe("OK");
  });

  test("GET /status returns agent status", async ({ request }) => {
    const res = await request.get("/status");
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(body).toHaveProperty("version");
    expect(body).toHaveProperty("skills_loaded");
  });

  test("GET /settings returns config", async ({ request }) => {
    const res = await request.get("/settings");
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(typeof body).toBe("object");
  });
});
