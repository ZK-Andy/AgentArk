import { test, expect } from "@playwright/test";

test.describe("Integrations API @api", () => {
  test("GET /integrations returns list", async ({ request }) => {
    const res = await request.get("/integrations");
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(body).toHaveProperty("integrations");
    expect(Array.isArray(body.integrations)).toBe(true);
  });

  test("each integration has required fields", async ({ request }) => {
    const res = await request.get("/integrations");
    const { integrations } = await res.json();
    for (const item of integrations) {
      expect(item).toHaveProperty("id");
      expect(item).toHaveProperty("name");
      expect(typeof item.enabled).toBe("boolean");
    }
  });
});
