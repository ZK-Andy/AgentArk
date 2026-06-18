import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const testDir = path.dirname(fileURLToPath(import.meta.url));
const frontendRoot = path.resolve(testDir, "..");
const settingsMetaSource = readFileSync(
  path.join(frontendRoot, "src", "components", "pages", "settingsMeta.ts"),
  "utf8",
);
const settingsNavigationSource = readFileSync(
  path.join(frontendRoot, "src", "components", "pages", "settingsNavigation.tsx"),
  "utf8",
);

test("companion devices settings tab is hidden from direct routing", () => {
  assert.match(settingsMetaSource, /if \(tab === 26\) return 20;/);
  assert.equal(settingsMetaSource.includes("companion: 26"), false);
  assert.equal(settingsMetaSource.includes("devices: 26"), false);
});

test("companion devices settings nav item stays hidden", () => {
  assert.equal(settingsNavigationSource.includes('label: "Companion Devices"'), false);
});
