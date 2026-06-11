// File view for source_read / source_write / file_edit etc.
// Renders syntax-colored file content when it is available.

import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import Box from "@mui/material/Box";
import IconButton from "@mui/material/IconButton";
import Tooltip from "@mui/material/Tooltip";
import Typography from "@mui/material/Typography";
import ContentCopyRounded from "@mui/icons-material/ContentCopyRounded";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";

import type { ChatStepCard } from "../types";
import { extractFilePath } from "../dispatch";
import { surfacePayloads } from "../surface";
import {
  guessCodeLanguage,
  renderCodeBlockLines,
  type CodeLanguage,
} from "../codeHighlight";
import { shouldRenderFileAsMarkdown } from "./filePreviewMode";

export interface FileViewProps {
  card: ChatStepCard;
  snippetPath?: string;
  snippetContent?: string;
  /** True while the agent is actively streaming content into this file. Drives
   * a "writing" indicator and auto-scroll-to-bottom so the user watches the
   * latest line append (Bolt/Lovable-style). */
  live?: boolean;
}

function pickCardContent(card: ChatStepCard): string {
  return (
    card.payloadView?.body ||
    card.rawDetailFull ||
    card.detailFull ||
    card.detail ||
    ""
  );
}

function tryParseRecord(raw: string): Record<string, unknown> | null {
  const trimmed = (raw || "").trim();
  if (!trimmed || !trimmed.startsWith("{")) return null;
  try {
    const parsed = JSON.parse(trimmed) as unknown;
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : null;
  } catch {
    return null;
  }
}

function str(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function normalizePath(value: string): string {
  return (value || "").trim().replace(/\\/g, "/").toLowerCase();
}

function recordPath(record: Record<string, unknown>): string {
  return str(record.path) || str(record.file) || str(record.name);
}

function recordMatchesPath(
  record: Record<string, unknown>,
  targetPath: string,
): boolean {
  const target = normalizePath(targetPath);
  if (!target) return true;
  const candidate = normalizePath(recordPath(record));
  return !candidate || candidate === target || candidate.endsWith(`/${target}`) || target.endsWith(`/${candidate}`);
}

function sourceContentFromRecord(
  record: Record<string, unknown>,
  targetPath: string,
): string {
  if (!recordMatchesPath(record, targetPath)) return "";
  const snapshot = str(record.content_snapshot);
  if (snapshot) return snapshot;
  const delta = str(record.content_delta);
  if (delta) return delta;
  const fileContent =
    str(record.raw_content) || str(record.file_content) || str(record.text);
  if (fileContent) return fileContent;
  const kind = str(record.kind).trim().toLowerCase();
  const hasFileIdentity = Boolean(recordPath(record));
  const content = str(record.content);
  if (
    content &&
    hasFileIdentity &&
    (kind === "draft_file" || kind === "file_write" || !str(record.tool_name))
  ) {
    return content;
  }
  return "";
}

function structuredCardContent(card: ChatStepCard, path: string): string {
  const candidates = [
    card.payloadView?.body || "",
    card.rawDetailFull || "",
    card.detailFull || "",
    card.detail || "",
  ];
  for (const candidate of candidates) {
    const parsed = tryParseRecord(candidate);
    if (!parsed) continue;
    const content = sourceContentFromRecord(parsed, path);
    if (content) return content;
  }
  return "";
}

function structuredSurfaceContent(card: ChatStepCard, path: string): string {
  const target = normalizePath(path);
  for (const item of surfacePayloads(card)) {
    const itemPath = normalizePath(
      item.path ||
        str(item.metadata?.path) ||
        str(item.metadata?.file) ||
        str(item.metadata?.name),
    );
    if (target && !itemPath) continue;
    if (
      target &&
      itemPath &&
      itemPath !== target &&
      !itemPath.endsWith(`/${target}`) &&
      !target.endsWith(`/${itemPath}`)
    ) {
      continue;
    }
    if (item.text) return item.text;
    if (typeof item.json === "string") return item.json;
    if (item.json != null) {
      try {
        return JSON.stringify(item.json, null, 2);
      } catch {
        return "";
      }
    }
  }
  return "";
}

function pickContent(
  card: ChatStepCard,
  path: string,
  snippetPath?: string,
  snippetContent?: string,
): string {
  const snippet = snippetContent || "";
  if (snippet.trim()) {
    const cardPath = normalizePath(path);
    const candidatePath = normalizePath(snippetPath || "");
    if (
      !candidatePath ||
      candidatePath === cardPath ||
      candidatePath.endsWith(`/${cardPath}`) ||
      cardPath.endsWith(`/${candidatePath}`)
    ) {
      return snippet;
    }
  }
  const surfaceContent = structuredSurfaceContent(card, path);
  if (surfaceContent) return surfaceContent;
  const structured = structuredCardContent(card, path);
  if (structured) return structured;
  if (snippetPath && snippetPath.trim()) return "";
  return pickCardContent(card);
}

function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0 B";
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

function languageLabel(language: CodeLanguage): string {
  switch (language) {
    case "markup":
      return "HTML";
    case "css":
      return "CSS";
    case "json":
      return "JSON";
    case "python":
      return "Python";
    case "sql":
      return "SQL";
    case "shell":
      return "Shell";
    case "markdown":
      return "Markdown";
    case "config":
      return "Config";
    case "script":
      return "Code";
    default:
      return "Text";
  }
}

const FILE_MARKDOWN_REMARK_PLUGINS = [remarkGfm];

const FILE_MARKDOWN_COMPONENTS: Components = {
  a({ href, children }) {
    return (
      <a href={href || ""} target="_blank" rel="noopener noreferrer">
        {children}
      </a>
    );
  },
  img({ src, alt }) {
    return (
      <img
        src={src || ""}
        alt={alt || ""}
        loading="lazy"
        referrerPolicy="no-referrer"
        onError={(event) => {
          event.currentTarget.style.display = "none";
        }}
      />
    );
  },
};

function FileViewInner({
  card,
  snippetPath,
  snippetContent,
  live = false,
}: FileViewProps) {
  const path =
    (snippetPath && snippetPath.trim()) || extractFilePath(card) || card.label;
  const body = pickContent(card, path, snippetPath, snippetContent);
  const byteCount = useMemo(() => new Blob([body || ""]).size, [body]);
  const detectedLanguage = useMemo(
    () => guessCodeLanguage(path, body),
    [body, path],
  );
  const renderAsMarkdown = useMemo(
    () => shouldRenderFileAsMarkdown(path, body),
    [body, path],
  );
  const highlightedLines = useMemo(
    () => (renderAsMarkdown ? [] : renderCodeBlockLines(body, { fileName: path })),
    [body, path, renderAsMarkdown],
  );
  const meta = body
    ? `${languageLabel(detectedLanguage)} / ${formatBytes(byteCount)}`
    : (card.kind || "").toLowerCase();
  const [copied, setCopied] = useState(false);
  const bodyRef = useRef<HTMLPreElement | HTMLDivElement | null>(null);
  const followTailRef = useRef(true);
  const previousPathRef = useRef(path);
  const setBodyNode = useCallback(
    (node: HTMLPreElement | HTMLDivElement | null) => {
      bodyRef.current = node;
    },
    [],
  );

  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), 1500);
    return () => window.clearTimeout(timer);
  }, [copied]);

  useEffect(() => {
    if (previousPathRef.current !== path) {
      previousPathRef.current = path;
      followTailRef.current = true;
    }
    if (live) followTailRef.current = true;
  }, [live, path]);

  // Auto-scroll the body so the freshly-streamed line stays visible while the
  // agent is writing. If the user scrolls up, stop following until they return
  // near the bottom or a new live file stream begins.
  useEffect(() => {
    if (!live || !followTailRef.current) return;
    const node = bodyRef.current;
    if (!node) return;
    node.scrollTop = node.scrollHeight;
  }, [body, live]);

  function handleBodyScroll() {
    if (!live) return;
    const node = bodyRef.current;
    if (!node) return;
    const distanceFromBottom =
      node.scrollHeight - node.scrollTop - node.clientHeight;
    followTailRef.current = distanceFromBottom < 40;
  }

  async function handleCopy() {
    if (!body) return;
    try {
      await navigator.clipboard.writeText(body);
      setCopied(true);
    } catch {
      // Clipboard access can be denied in insecure contexts.
    }
  }

  return (
    <Box className={`cview cview-file${live ? " is-live" : ""}`}>
      <Box className="cview-file-head">
        <span className="cview-file-icon" aria-hidden="true">
          {"</>"}
        </span>
        <span className="cview-file-path" title={path}>
          {path}
        </span>
        {live ? (
          <span
            className="cview-file-live"
            title="Agent is writing this file"
            aria-live="polite"
          >
            <span className="cview-file-live-dot" aria-hidden="true" />
            writing
          </span>
        ) : null}
        {meta ? <span className="cview-file-meta">{meta}</span> : null}
        <span className="cview-file-actions">
          <Tooltip title={copied ? "Copied" : "Copy file contents"} placement="top" arrow>
            <span>
              <IconButton
                className="cview-file-copy"
                size="small"
                onClick={handleCopy}
                disabled={!body}
                aria-label="Copy file contents"
              >
                <ContentCopyRounded fontSize="inherit" />
              </IconButton>
            </span>
          </Tooltip>
        </span>
      </Box>
      {body ? (
        renderAsMarkdown ? (
          <Box
            ref={setBodyNode}
            className="cview-file-body cview-file-markdown chat-markdown"
            onScroll={handleBodyScroll}
          >
            <ReactMarkdown
              remarkPlugins={FILE_MARKDOWN_REMARK_PLUGINS}
              components={FILE_MARKDOWN_COMPONENTS}
            >
              {body}
            </ReactMarkdown>
            {live ? (
              <span className="cview-file-caret" aria-hidden="true">
                |
              </span>
            ) : null}
          </Box>
        ) : (
          <pre
            ref={setBodyNode}
            className="code-viewer-pre cview-file-body"
            onScroll={handleBodyScroll}
          >
            <code>{highlightedLines}</code>
            {live ? (
              <span className="cview-file-caret" aria-hidden="true">
                |
              </span>
            ) : null}
          </pre>
        )
      ) : (
        <Typography variant="body2" className="cview-file-empty">
          {live
            ? "Drafting file..."
            : "File contents not captured for this step."}
        </Typography>
      )}
    </Box>
  );
}

function areFileViewPropsEqual(prev: FileViewProps, next: FileViewProps) {
  if (
    prev.live !== next.live ||
    prev.snippetPath !== next.snippetPath ||
    prev.snippetContent !== next.snippetContent
  ) {
    return false;
  }

  if (
    (prev.snippetContent || next.snippetContent) &&
    prev.snippetPath === next.snippetPath
  ) {
    return true;
  }

  return (
    prev.card.id === next.card.id &&
    prev.card.label === next.card.label &&
    prev.card.kind === next.card.kind &&
    prev.card.detail === next.card.detail &&
    prev.card.detailFull === next.card.detailFull &&
    prev.card.rawDetailFull === next.card.rawDetailFull &&
    prev.card.payloadView?.body === next.card.payloadView?.body &&
    prev.card.surface === next.card.surface
  );
}

export const FileView = memo(FileViewInner, areFileViewPropsEqual);
FileView.displayName = "FileView";

export default FileView;
