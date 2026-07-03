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

test("ArkMemory queue rows show the proposed memory key and value", () => {
  assert.equal(
    arkMemoryPageSource.includes("arkmemoryQueueCandidateDetails"),
    true,
    "queue rows should normalize proposed_content before rendering",
  );
  assert.match(
    arkMemoryPageSource,
    /asRecord\(\s*item\.proposed_content\s*\)/,
    "queue rows should read the backend proposed_content payload",
  );
  assert.match(
    arkMemoryPageSource,
    /candidateDetails\.key[\s\S]{0,800}candidateDetails\.value/,
    "queue rows should render the candidate key and proposed value",
  );
});

test("ArkMemory queue rows do not show Apply for in-flight approvals", () => {
  assert.match(
    arkMemoryPageSource,
    /approvalStatus\s*=\s*str\(\s*item\.approval_status/,
    "queue rows should read approval_status",
  );
  assert.match(
    arkMemoryPageSource,
    /isApplyingQueueItem\s*=\s*approvalStatus\s*===\s*"applying"/,
    "queue rows should derive an in-flight approval state",
  );
  assert.equal(
    arkMemoryPageSource.includes('label="Applying..."'),
    true,
    "in-flight approvals should render an applying label",
  );
  assert.match(
    arkMemoryPageSource,
    /disabled=\{busy \|\| !canApproveQueueItem\}/,
    "Apply should be disabled unless the row is an approvable draft",
  );
});
