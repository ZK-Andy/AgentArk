import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const testDir = path.dirname(fileURLToPath(import.meta.url));
const frontendRoot = path.resolve(testDir, "..");
const arkMemoryPageSource = readFileSync(
  path.join(frontendRoot, "src", "components", "pages", "ArkMemoryPage.tsx"),
  "utf8",
);

test("ArkMemory health actions use finding-provided labels for dismissible maintenance findings", () => {
  assert.equal(
    arkMemoryPageSource.includes("reviewActionLabel"),
    true,
    "health rows should derive the button label from the finding contract",
  );
  assert.match(
    arkMemoryPageSource,
    /str\(\s*finding\.review_action_label[\s\S]{0,120}toBool\(finding\.dismissible\)\s*\?\s*"Dismiss"/,
    "the UI should read review_action_label and fall back to Dismiss for dismissible findings",
  );
});
