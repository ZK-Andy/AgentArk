import { test, expect } from "@playwright/test";

test.describe("Security API @api", () => {
  test("GET /security/logs returns logs", async ({ request }) => {
    const res = await request.get("/security/logs?limit=5");
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(body).toHaveProperty("logs");
    expect(Array.isArray(body.logs)).toBe(true);
  });

  test("GET /security/status returns response", async ({
    request,
  }) => {
    const res = await request.get("/security/status");
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(typeof body.master_password_set).toBe("boolean");
    expect(typeof body.custom_master_password_set).toBe("boolean");
    expect(typeof body.encryption_mode).toBe("string");
  });
});

test.describe("Autonomy API @api", () => {
  test("GET /autonomy/briefing returns briefing", async ({ request }) => {
    const res = await request.get("/autonomy/briefing");
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(typeof body).toBe("object");
  });

  test("GET /autonomy/sentinel/feed returns feed", async ({ request }) => {
    const res = await request.get("/autonomy/sentinel/feed");
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(typeof body).toBe("object");
  });

  test("GET /autonomy/settings returns settings", async ({ request }) => {
    const res = await request.get("/autonomy/settings");
    expect(res.ok()).toBeTruthy();
  });
});
