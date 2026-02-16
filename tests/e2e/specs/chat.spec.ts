import { test, expect } from "@playwright/test";

test.describe("Chat API @api", () => {
  test("POST /chat returns a response", async ({ request }) => {
    const res = await request.post("/chat", {
      data: {
        message: "ping",
        channel: "web",
      },
      timeout: 60_000,
    });
    // May fail if no LLM configured — that's expected
    if (res.status() === 500) {
      const body = await res.json();
      expect(body).toHaveProperty("error");
      return;
    }
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(body).toHaveProperty("response");
    expect(typeof body.response).toBe("string");
  });

  test("POST /chat/stream returns SSE events", async ({ request }) => {
    const res = await request.fetch("/chat/stream", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "text/event-stream",
      },
      data: JSON.stringify({
        message: "ping",
        channel: "web",
      }),
      timeout: 60_000,
    });

    // If no LLM configured, 500 is expected
    if (res.status() === 500) return;

    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    // SSE should contain at least a done event
    expect(text).toContain("event:");
  });
});

test.describe("Chat Conversations @api", () => {
  test("GET /conversations returns list", async ({ request }) => {
    const res = await request.get("/conversations");
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    const convos = Array.isArray(body) ? body : body.conversations;
    expect(Array.isArray(convos)).toBe(true);
  });
});
