const express = require('express');
const { chromium } = require('playwright');
const { v4: uuidv4 } = require('uuid');

const app = express();
app.use(express.json({ limit: '10mb' }));

const PORT = process.env.PORT || 3100;
const HOST = process.env.PLAYWRIGHT_BRIDGE_HOST || process.env.HOST || '127.0.0.1';
const SESSION_TIMEOUT_MS = 15 * 60 * 1000; // 15 min inactivity timeout

// Active browser sessions: id -> { context, page, lastActivity, cleanupTimer }
const sessions = new Map();

let browser = null;

async function ensureBrowser() {
  if (!browser || !browser.isConnected()) {
    browser = await chromium.launch({
      headless: true,
      args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-dev-shm-usage'],
    });
  }
  return browser;
}

function touchSession(session) {
  session.lastActivity = Date.now();
  if (session.cleanupTimer) clearTimeout(session.cleanupTimer);
  session.cleanupTimer = setTimeout(() => destroySession(session.id), SESSION_TIMEOUT_MS);
}

async function destroySession(id) {
  const session = sessions.get(id);
  if (!session) return;
  if (session.cleanupTimer) clearTimeout(session.cleanupTimer);
  try { await session.context.close(); } catch (_) {}
  sessions.delete(id);
  console.log(`Session ${id} destroyed (${sessions.size} remaining)`);
}

// Health check
app.get('/health', (req, res) => {
  res.json({ status: 'ok', sessions: sessions.size });
});

// Create a new browser session
app.post('/session', async (req, res) => {
  try {
    const b = await ensureBrowser();
    const context = await b.newContext({
      viewport: { width: 1280, height: 720 },
      userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
    });
    const page = await context.newPage();
    const id = uuidv4();
    const session = { id, context, page, lastActivity: Date.now(), cleanupTimer: null };
    sessions.set(id, session);
    touchSession(session);
    console.log(`Session ${id} created (${sessions.size} total)`);
    res.json({ session_id: id });
  } catch (e) {
    res.status(500).json({ error: e.message });
  }
});

// Close a session
app.delete('/session/:id', async (req, res) => {
  const { id } = req.params;
  if (!sessions.has(id)) return res.status(404).json({ error: 'Session not found' });
  await destroySession(id);
  res.json({ status: 'closed' });
});

// Navigate to URL
app.post('/session/:id/navigate', async (req, res) => {
  const session = sessions.get(req.params.id);
  if (!session) return res.status(404).json({ error: 'Session not found' });
  touchSession(session);
  try {
    const { url } = req.body;
    await session.page.goto(url, { waitUntil: 'domcontentloaded', timeout: 30000 });
    res.json({ status: 'ok', url: session.page.url(), title: await session.page.title() });
  } catch (e) {
    res.status(500).json({ error: e.message });
  }
});

// Take screenshot
app.get('/session/:id/screenshot', async (req, res) => {
  const session = sessions.get(req.params.id);
  if (!session) return res.status(404).json({ error: 'Session not found' });
  touchSession(session);
  try {
    const buffer = await session.page.screenshot({ type: 'png', fullPage: false });
    res.set('Content-Type', 'image/png');
    res.send(buffer);
  } catch (e) {
    res.status(500).json({ error: e.message });
  }
});

// Click element
app.post('/session/:id/click', async (req, res) => {
  const session = sessions.get(req.params.id);
  if (!session) return res.status(404).json({ error: 'Session not found' });
  touchSession(session);
  try {
    const { selector, text, x, y } = req.body;
    if (x !== undefined && y !== undefined) {
      await session.page.mouse.click(x, y);
    } else if (text) {
      await session.page.getByText(text, { exact: false }).first().click({ timeout: 5000 });
    } else if (selector) {
      await session.page.click(selector, { timeout: 5000 });
    } else {
      return res.status(400).json({ error: 'Provide selector, text, or x/y coordinates' });
    }
    await session.page.waitForTimeout(500); // brief settle
    res.json({ status: 'ok' });
  } catch (e) {
    res.status(500).json({ error: e.message });
  }
});

// Type text
app.post('/session/:id/type', async (req, res) => {
  const session = sessions.get(req.params.id);
  if (!session) return res.status(404).json({ error: 'Session not found' });
  touchSession(session);
  try {
    const { selector, text, clear } = req.body;
    if (selector) {
      if (clear) await session.page.fill(selector, '');
      await session.page.fill(selector, text || '');
    } else {
      // Type into currently focused element
      if (clear) {
        await session.page.keyboard.down('Control');
        await session.page.keyboard.press('a');
        await session.page.keyboard.up('Control');
        await session.page.keyboard.press('Backspace');
      }
      await session.page.keyboard.type(text || '', { delay: 30 });
    }
    res.json({ status: 'ok' });
  } catch (e) {
    res.status(500).json({ error: e.message });
  }
});

// Scroll page
app.post('/session/:id/scroll', async (req, res) => {
  const session = sessions.get(req.params.id);
  if (!session) return res.status(404).json({ error: 'Session not found' });
  touchSession(session);
  try {
    const { direction, amount } = req.body;
    const pixels = amount || 500;
    const dy = direction === 'up' ? -pixels : pixels;
    await session.page.mouse.wheel(0, dy);
    await session.page.waitForTimeout(300);
    res.json({ status: 'ok' });
  } catch (e) {
    res.status(500).json({ error: e.message });
  }
});

// Press keyboard key
app.post('/session/:id/press', async (req, res) => {
  const session = sessions.get(req.params.id);
  if (!session) return res.status(404).json({ error: 'Session not found' });
  touchSession(session);
  try {
    const { key } = req.body;
    await session.page.keyboard.press(key);
    await session.page.waitForTimeout(300);
    res.json({ status: 'ok' });
  } catch (e) {
    res.status(500).json({ error: e.message });
  }
});

// Get page content and interactive elements
app.get('/session/:id/content', async (req, res) => {
  const session = sessions.get(req.params.id);
  if (!session) return res.status(404).json({ error: 'Session not found' });
  touchSession(session);
  try {
    const page = session.page;
    const title = await page.title();
    const url = page.url();

    // Extract visible text (truncated)
    const bodyText = await page.evaluate(() => {
      const body = document.body;
      if (!body) return '';
      // Get text, strip excess whitespace
      return body.innerText.substring(0, 5000);
    });

    // Extract interactive elements with their labels
    const elements = await page.evaluate(() => {
      const results = [];
      const interactiveSelectors = 'a, button, input, select, textarea, [role="button"], [role="link"], [onclick]';
      const els = document.querySelectorAll(interactiveSelectors);
      for (let i = 0; i < Math.min(els.length, 50); i++) {
        const el = els[i];
        const rect = el.getBoundingClientRect();
        if (rect.width === 0 || rect.height === 0) continue;
        const tag = el.tagName.toLowerCase();
        const type = el.getAttribute('type') || '';
        const text = (el.innerText || el.value || el.getAttribute('aria-label') || el.getAttribute('placeholder') || '').trim().substring(0, 80);
        const name = el.getAttribute('name') || '';
        const id = el.id || '';
        const href = el.getAttribute('href') || '';
        results.push({
          index: results.length,
          tag, type, text, name, id, href,
          x: Math.round(rect.x + rect.width / 2),
          y: Math.round(rect.y + rect.height / 2),
        });
      }
      return results;
    });

    res.json({ title, url, body_text: bodyText, elements });
  } catch (e) {
    res.status(500).json({ error: e.message });
  }
});

// Evaluate JavaScript on the page
app.post('/session/:id/evaluate', async (req, res) => {
  const session = sessions.get(req.params.id);
  if (!session) return res.status(404).json({ error: 'Session not found' });
  touchSession(session);
  try {
    const { expression } = req.body;
    const result = await session.page.evaluate(expression);
    res.json({ result });
  } catch (e) {
    res.status(500).json({ error: e.message });
  }
});

// Wait for navigation/selector
app.post('/session/:id/wait', async (req, res) => {
  const session = sessions.get(req.params.id);
  if (!session) return res.status(404).json({ error: 'Session not found' });
  touchSession(session);
  try {
    const { selector, timeout } = req.body;
    const ms = timeout || 10000;
    if (selector) {
      await session.page.waitForSelector(selector, { timeout: ms });
    } else {
      await session.page.waitForLoadState('domcontentloaded', { timeout: ms });
    }
    res.json({ status: 'ok' });
  } catch (e) {
    res.status(500).json({ error: e.message });
  }
});

// List active sessions
app.get('/sessions', (req, res) => {
  const list = [];
  for (const [id, s] of sessions) {
    list.push({ id, lastActivity: s.lastActivity, age_ms: Date.now() - s.lastActivity });
  }
  res.json({ sessions: list });
});

// Graceful shutdown
process.on('SIGTERM', async () => {
  console.log('Shutting down...');
  for (const [id] of sessions) await destroySession(id);
  if (browser) await browser.close();
  process.exit(0);
});

app.listen(PORT, HOST, () => {
  console.log(`Playwright bridge listening on ${HOST}:${PORT}`);
});
