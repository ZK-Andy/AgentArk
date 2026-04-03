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
        action: "notes_log",
        description: "E2E test task - safe to delete",
        arguments: {},
      },
    });
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(body).toHaveProperty("status");
    expect(body).toHaveProperty("id");
  });
});

test.describe("Tasks UI", () => {
  test("cancelled chat_request tasks resume in chat from the Tasks panel", async ({ page }) => {
    const conversationId = "conv-task-panel-resume";
    const taskId = "task-panel-resume";
    const userMessage = "continue the stopped run";
    const resumedAssistant = "Resumed from the Tasks panel and finished in chat.";
    let resumed = false;

    await page.route("**/projects", async (route) => {
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify({ projects: [] })
      });
    });

    await page.route("**/tasks?**", async (route) => {
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify({
          tasks: [
            {
              id: taskId,
              description: "Resume my stopped chat task",
              action: "chat_request",
              status: "Cancelled",
              created_at: "2026-03-31T01:00:00.000Z",
              cron: null,
              arguments: {
                _task_kind: "chat_request",
                _origin: "chat",
                message: userMessage,
                channel: "web",
                conversation_id: conversationId,
                project_id: null
              }
            }
          ]
        })
      });
    });

    await page.route("**/conversations?**", async (route) => {
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify({
          conversations: [
            {
              id: conversationId,
              title: "Tasks panel resume chat",
              channel: "web",
              project_id: null,
              created_at: "2026-03-31T01:00:00.000Z",
              updated_at: "2026-03-31T01:03:00.000Z",
              message_count: resumed ? 2 : 1,
              archived: false
            }
          ],
          total: 1,
          limit: 30,
          offset: 0
        })
      });
    });

    await page.route(`**/conversations/${conversationId}`, async (route) => {
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify({
          id: conversationId,
          title: "Tasks panel resume chat",
          channel: "web",
          project_id: null,
          created_at: "2026-03-31T01:00:00.000Z",
          updated_at: "2026-03-31T01:03:00.000Z",
          message_count: resumed ? 2 : 1
        })
      });
    });

    await page.route(`**/conversations/${conversationId}/messages?**`, async (route) => {
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify({
          messages: resumed
            ? [
                {
                  id: "msg-user-task-panel",
                  role: "user",
                  content: userMessage,
                  timestamp: "2026-03-31T01:00:01.000Z",
                  model_used: null,
                  trace_id: null
                },
                {
                  id: "msg-assistant-task-panel",
                  role: "assistant",
                  content: resumedAssistant,
                  timestamp: "2026-03-31T01:03:10.000Z",
                  model_used: "test-model",
                  trace_id: null
                }
              ]
            : [
                {
                  id: "msg-user-task-panel",
                  role: "user",
                  content: userMessage,
                  timestamp: "2026-03-31T01:00:01.000Z",
                  model_used: null,
                  trace_id: null
                }
              ]
        })
      });
    });

    await page.route(`**/tasks/${taskId}/resume-chat/stream`, async (route) => {
      resumed = true;
      await route.fulfill({
        status: 200,
        contentType: "text/event-stream",
        body: [
          `event: task_started\ndata: {"task_id":"${taskId}","description":"Resume my stopped chat task","status":"in_progress","work_type":"task","conversation_id":"${conversationId}"}\n\n`,
          `event: content\ndata: {"conversation_id":"${conversationId}","content":"${resumedAssistant}"}\n\n`,
          "event: done\ndata: {}\n\n"
        ].join("")
      });
    });

    await page.goto("/");
    await page.waitForSelector("text=AgentArk", { timeout: 15_000 });

    const tasksNav = page.locator("text=Tasks").first();
    if (await tasksNav.isVisible()) {
      await tasksNav.click();
    }

    await expect(page.locator("text=Resume my stopped chat task")).toBeVisible({ timeout: 10_000 });
    await page.getByLabel("Task options").first().click();
    await page.getByText("Resume in chat", { exact: true }).click();

    await expect(page.locator(`text=${resumedAssistant}`)).toBeVisible({ timeout: 10_000 });
    await expect(page.locator("text=You | sending...")).toHaveCount(0);
  });
});
