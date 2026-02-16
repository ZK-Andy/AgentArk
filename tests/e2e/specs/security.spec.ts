import { test, expect } from "@playwright/test";

test.describe("Security API @api", () => {
  test("GET /security/logs returns logs", async ({ request }) => {
    const res = await request.get("/security/logs?limit=5");
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(body).toHaveProperty("logs");
    expect(Array.isArray(body.logs)).toBe(true);
  });

  test("GET /security/master-password/status returns response", async ({
    request,
  }) => {
    const res = await request.get("/security/master-password/status");
    // Endpoint may return 200 or 404 depending on configuration
    expect([200, 404]).toContain(res.status());
  });
});

test.describe("Autonomy API @api", () => {
  test("GET /autonomy/briefing returns briefing", async ({ request }) => {
    const res = await request.get("/autonomy/briefing");
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(typeof body).toBe("object");
  });

  test("GET /autonomy/nudges returns nudges", async ({ request }) => {
    const res = await request.get("/autonomy/nudges");
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(body).toHaveProperty("nudges");
  });

  test("GET /autonomy/settings returns settings", async ({ request }) => {
    const res = await request.get("/autonomy/settings");
    expect(res.ok()).toBeTruthy();
  });
});
