import { test, expect } from "@playwright/test";

test.describe("Notifications API @api", () => {
  test("GET /notifications returns list", async ({ request }) => {
    const res = await request.get("/notifications");
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(body).toHaveProperty("notifications");
    expect(Array.isArray(body.notifications)).toBe(true);
    expect(body).toHaveProperty("total");
  });

  test("GET /notifications/count returns count", async ({ request }) => {
    const res = await request.get("/notifications/count");
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(typeof body.unread).toBe("number");
  });

  test("POST /notifications/read-all succeeds", async ({ request }) => {
    const res = await request.post("/notifications/read-all");
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(body.status).toBe("ok");
  });

  test("mark-all-read actually clears unread count", async ({ request }) => {
    // Mark all as read
    await request.post("/notifications/read-all");

    // Verify count is now 0
    const countRes = await request.get("/notifications/count");
    expect(countRes.ok()).toBeTruthy();
    const countBody = await countRes.json();
    expect(countBody.unread).toBe(0);

    // Verify list shows all as read
    const listRes = await request.get("/notifications?unread=true");
    expect(listRes.ok()).toBeTruthy();
    const listBody = await listRes.json();
    expect(listBody.notifications.length).toBe(0);
  });

  test("mark single notification read persists", async ({ request }) => {
    // Get current notifications
    const listRes = await request.get("/notifications");
    const { notifications } = await listRes.json();

    if (notifications.length === 0) {
      test.skip();
      return;
    }

    const target = notifications[0];
    const markRes = await request.post(`/notifications/${target.id}/read`);
    expect(markRes.ok()).toBeTruthy();

    // Re-fetch and verify
    const verifyRes = await request.get("/notifications");
    const verifyBody = await verifyRes.json();
    const updated = verifyBody.notifications.find(
      (n: { id: string }) => n.id === target.id
    );
    expect(updated).toBeDefined();
    expect(updated.read).toBe(true);
  });
});
