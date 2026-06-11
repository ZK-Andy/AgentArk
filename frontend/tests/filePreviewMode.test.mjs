import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

const testDir = path.dirname(fileURLToPath(import.meta.url));
const frontendRoot = path.resolve(testDir, "..");
const outDir = mkdtempSync(path.join(tmpdir(), "agentark-file-preview-mode-"));

execFileSync(
  process.execPath,
  [
    path.join(frontendRoot, "node_modules", "typescript", "bin", "tsc"),
    "src/components/chat/computerViews/filePreviewMode.ts",
    "--ignoreConfig",
    "--target",
    "ES2020",
    "--module",
    "ES2020",
    "--moduleResolution",
    "Bundler",
    "--outDir",
    outDir,
    "--skipLibCheck",
  ],
  { cwd: frontendRoot, stdio: "inherit" },
);
writeFileSync(path.join(outDir, "package.json"), JSON.stringify({ type: "module" }));

const { shouldRenderFileAsMarkdown } = await import(
  pathToFileURL(path.join(outDir, "filePreviewMode.js")).toString()
);

test("renders markdown extensions as formatted previews", () => {
  assert.equal(
    shouldRenderFileAsMarkdown("workspace/report.md", "# Report\n\nBody"),
    true,
  );
  assert.equal(
    shouldRenderFileAsMarkdown("workspace/RESEARCH.MARKDOWN", "# Report"),
    true,
  );
});

test("keeps non-markdown files in source view", () => {
  assert.equal(
    shouldRenderFileAsMarkdown("workspace/index.html", "# Not a markdown heading"),
    false,
  );
  assert.equal(
    shouldRenderFileAsMarkdown("workspace/report.md.hash", "# Report"),
    false,
  );
});
