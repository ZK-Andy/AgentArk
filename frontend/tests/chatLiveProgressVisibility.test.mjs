import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const testDir = path.dirname(fileURLToPath(import.meta.url));
const frontendRoot = path.resolve(testDir, "..");
const chatPageSource = readFileSync(
  path.join(frontendRoot, "src", "components", "pages", "ChatPage.tsx"),
  "utf8",
);
const computerPaneSource = readFileSync(
  path.join(frontendRoot, "src", "components", "chat", "ComputerPane.tsx"),
  "utf8",
);

function assertSourceIncludes(source, needle, message) {
  assert.equal(source.includes(needle), true, message);
}

function functionSource(source, functionName) {
  const start = source.indexOf(`function ${functionName}`);
  assert.notEqual(start, -1, `${functionName} should exist`);
  const braceStart = source.indexOf("{", start);
  assert.notEqual(braceStart, -1, `${functionName} should have a body`);
  let depth = 0;
  for (let index = braceStart; index < source.length; index += 1) {
    const ch = source[index];
    if (ch === "{") depth += 1;
    if (ch === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(start, index + 1);
    }
  }
  assert.fail(`${functionName} body should close`);
}

test("live chat transcript action groups are expanded during streaming", () => {
  assert.equal(
    /const expanded\s*=\s*\(isLiveTranscript && hasRunning\)\s*\|\|\s*expandedTranscriptActions\.has\(groupId\);/.test(
      chatPageSource,
    ),
    true,
    "live transcript action groups should expand while running and auto-collapse once settled",
  );
  assertSourceIncludes(
    chatPageSource,
    '<Collapse in={expanded} timeout="auto" unmountOnExit>',
    "action group body should use the computed expanded state",
  );
});

test("computer console working view receives live assistant token preview", () => {
  assert.equal(
    /<WorkingView[\s\S]{0,420}tokenPreview=\{tokenPreview\}/.test(computerPaneSource),
    true,
    "working view should receive the existing token preview stream",
  );
});

test("chat thread does not render internal reasoning as assistant reply text", () => {
  assert.equal(
    chatPageSource.includes("const visibleLiveModelEmit"),
    false,
    "chat should not derive a live assistant emit from internal reasoning",
  );
  assertSourceIncludes(
    chatPageSource,
    "visibleStreamingResponse.trim()",
    "streaming assistant bubble should render from final/user-visible response tokens",
  );
  assert.equal(
    chatPageSource.includes("visibleLiveModelEmit.trim()"),
    false,
    "reasoning-derived live model emits should not keep a chat reply bubble open",
  );
  assert.equal(
    chatPageSource.includes("deferredLiveModelEmitText"),
    false,
    "streaming markdown should not fall back to internal reasoning text",
  );
});

test("live chat transcript keeps model prose progress alongside tool actions", () => {
  assertSourceIncludes(
    chatPageSource,
    'kind === "model_prose"',
    "chat transcript builder should recognize model_prose progress events",
  );
  assertSourceIncludes(
    chatPageSource,
    'item.kind === "action" || item.kind === "prose"',
    "live transcript should keep model prose rows instead of filtering to actions only",
  );
  assert.equal(
    /pushPendingProse\(\);\s*const finalItems = runLooksComplete\(\)/.test(
      chatPageSource,
    ),
    true,
    "pending model prose should flush even before any tool action arrives",
  );
});

test("live chat transcript preserves model prose across active step limiting", () => {
  assertSourceIncludes(
    chatPageSource,
    "function isTranscriptPreservedActivityStep",
    "step limiting should have a shared predicate for chat-visible prose/reasoning rows",
  );
  assertSourceIncludes(
    chatPageSource,
    "isTranscriptPreservedActivityStep(step)",
    "live and pending step limiters should preserve model_prose rows instead of keeping only tail tool rows",
  );
  assertSourceIncludes(
    chatPageSource,
    "const liveTranscriptSourceSteps = useMemo(",
    "live transcript should derive from a merged source that cannot drop prose when tool rows arrive",
  );
  assertSourceIncludes(
    chatPageSource,
    "mergeActivityStepSourcesForLiveTranscript(",
    "live transcript source should merge preserved prose rows with active tool steps",
  );
});

test("live chat transcript only preserves public model_prose as chat prose", () => {
  const modelNarrationSource = functionSource(
    chatPageSource,
    "modelNarrationTextFromActivityStep",
  );
  assert.equal(
    modelNarrationSource.includes("modelProseTextFromActivityStep(step)"),
    true,
    "public model_prose events should remain chat-visible",
  );
  assert.equal(
    modelNarrationSource.includes("isMainChatReasoningStep"),
    false,
    "internal reasoning_delta events should not become chat transcript prose",
  );
  assert.equal(
    modelNarrationSource.includes("agentLoopProgressPhaseFromStep"),
    false,
    "agent loop model-call progress is console/run-status metadata, not chat prose",
  );
});

test("live chat transcript item cap preserves model prose rows", () => {
  assertSourceIncludes(
    chatPageSource,
    "preserveProse?: boolean",
    "transcript limiting should support preserving model prose independently of the action cap",
  );
  assertSourceIncludes(
    chatPageSource,
    "limitTranscriptItemsForDisplay(finalItems, maxItems, options)",
    "transcript builder should not use a raw tail slice that drops earlier model emits",
  );
  assertSourceIncludes(
    chatPageSource,
    "preserveProse: true",
    "live transcript should keep model emits visible while actions continue streaming",
  );
});
