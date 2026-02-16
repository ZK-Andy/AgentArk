import { test, expect } from "@playwright/test";

test.describe("Tasks API @api", () => {
  test("GET /tasks returns list", async ({ request }) => {
    const res = await request.get("/tasks");
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    // Could be array or { tasks: [] }
    const tasks = Array.isArray(body) ? body : body.tasks;
    expect(Array.isArray(tasks)).toBe(true);
  });

  test("POST /tasks creates a task", async ({ request }) => {
    const res = await request.post("/tasks", {
      data: {
        action: "e2e-test",
        description: "E2E test task - safe to delete",
        schedule: "manual",
        arguments: {},
      },
    });
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(body).toHaveProperty("status");
  });
});
