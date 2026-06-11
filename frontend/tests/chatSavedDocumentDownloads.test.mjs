import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

const testDir = path.dirname(fileURLToPath(import.meta.url));
const frontendRoot = path.resolve(testDir, "..");
const outDir = mkdtempSync(path.join(tmpdir(), "agentark-chat-saved-docs-"));

execFileSync(
  process.execPath,
  [
    path.join(frontendRoot, "node_modules", "typescript", "bin", "tsc"),
    "src/components/pages/chatSavedDocumentDownloads.ts",
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

const {
  savedDocumentFilenamesFromAssistantText,
  resolveSavedDocumentDownload,
} = await import(
  pathToFileURL(path.join(outDir, "chatSavedDocumentDownloads.js")).toString()
);

test("extracts saved document filenames from assistant messages", () => {
  assert.deepEqual(
    savedDocumentFilenamesFromAssistantText(
      "The research brief has been saved as `ai-environmental-sustainability-research-brief.md` in your Documents.",
    ),
    ["ai-environmental-sustainability-research-brief.md"],
  );
  assert.deepEqual(
    savedDocumentFilenamesFromAssistantText(
      "Saved as notes with spaces.md in Documents.",
    ),
    ["notes with spaces.md"],
  );
});

test("resolves a saved document to the first matching downloadable row", () => {
  const target = resolveSavedDocumentDownload(
    ["brief.md"],
    [
      { id: "old", filename: "brief.md" },
      { id: "latest", filename: "brief.md", download_url: "/documents/latest/download" },
      { id: "other", filename: "other.md", download_url: "/documents/other/download" },
    ],
  );

  assert.deepEqual(target, {
    id: "latest",
    filename: "brief.md",
    downloadUrl: "/documents/latest/download",
  });
});

test("does not resolve non-downloadable document rows", () => {
  assert.equal(
    resolveSavedDocumentDownload(
      ["image.png"],
      [{ id: "image", filename: "image.png", download_url: "" }],
    ),
    null,
  );
});
