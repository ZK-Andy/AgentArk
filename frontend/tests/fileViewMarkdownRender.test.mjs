import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

const testDir = path.dirname(fileURLToPath(import.meta.url));
const frontendRoot = path.resolve(testDir, "..");
const outDir = mkdtempSync(path.join(tmpdir(), "agentark-fileview-render-"));

execFileSync(
  process.execPath,
  [
    path.join(frontendRoot, "node_modules", "typescript", "bin", "tsc"),
    "src/components/chat/computerViews/FileView.tsx",
    "--ignoreConfig",
    "--jsx",
    "react-jsx",
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
symlinkSync(path.join(frontendRoot, "node_modules"), path.join(outDir, "node_modules"), "junction");
for (const file of ["codeHighlight", "dispatch", "surface", "types"]) {
  const emitted = path.join(outDir, `${file}.js`);
  if (existsSync(emitted)) copyFileSync(emitted, path.join(outDir, file));
}
for (const file of ["filePreviewMode"]) {
  const emitted = path.join(outDir, "computerViews", `${file}.js`);
  if (existsSync(emitted)) copyFileSync(emitted, path.join(outDir, "computerViews", file));
}

const React = await import("react");
const { renderToStaticMarkup } = await import("react-dom/server");
const { FileView } = await import(
  pathToFileURL(path.join(outDir, "computerViews", "FileView.js")).toString()
);

function fileCard(label) {
  return {
    id: `file:${label}`,
    index: 0,
    stepType: "file_read",
    rawTitle: "",
    tone: "default",
    kind: "File",
    label,
    detail: "",
    detailFull: "",
    summary: "",
    rawDetailFull: "",
    payloadView: null,
    isHeartbeat: false,
    time: "",
  };
}

test("FileView renders markdown files as formatted document previews", () => {
  const html = renderToStaticMarkup(
    React.createElement(FileView, {
      card: fileCard("report.md"),
      snippetPath: "report.md",
      snippetContent:
        "# Is Aggressive AI Expansion Environmentally Sustainable?\n\n| Area | Status |\n| --- | --- |\n| Electricity | Constrained |\n",
    }),
  );

  assert.match(html, /cview-file-markdown/);
  assert.match(html, /<h1>Is Aggressive AI Expansion Environmentally Sustainable\?<\/h1>/);
  assert.match(html, /<table>/);
  assert.doesNotMatch(html, /code-line-number/);
});

test("FileView keeps non-markdown files in source mode", () => {
  const html = renderToStaticMarkup(
    React.createElement(FileView, {
      card: fileCard("index.html"),
      snippetPath: "index.html",
      snippetContent: "# Not a markdown heading\n<section>Raw source</section>",
    }),
  );

  assert.doesNotMatch(html, /cview-file-markdown/);
  assert.match(html, /code-line-number/);
});
