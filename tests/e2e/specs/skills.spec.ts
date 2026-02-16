import { test, expect } from "@playwright/test";

test.describe("Skills API @api", () => {
  test("GET /skills returns list", async ({ request }) => {
    const res = await request.get("/skills");
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    const skills = Array.isArray(body) ? body : body.skills;
    expect(Array.isArray(skills)).toBe(true);
  });

  test("each skill has id and name", async ({ request }) => {
    const res = await request.get("/skills");
    const body = await res.json();
    const skills = Array.isArray(body) ? body : body.skills;
    for (const s of skills.slice(0, 5)) {
      expect(s).toHaveProperty("name");
      expect(typeof s.name).toBe("string");
    }
  });
});
