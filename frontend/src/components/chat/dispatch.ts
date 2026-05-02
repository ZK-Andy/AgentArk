// Maps a ChatStepCard to a ComputerViewKind and extracts target fields
// (URL, file path, command) from card content.
//
// Preference order: canonical backend fields > structured payload shape >
// legacy display-label fallback.

import type { ChatStepCard, ComputerViewKind, ChipStatus } from "./types";

// Canonical tool / step types we know directly from the backend pipeline.
const STEP_TYPE_TO_VIEW: Array<[RegExp, ComputerViewKind]> = [
  [/file_write|file_edit|file_read|source_write|source_read|source_edit|source_list|source_search/, "file"],
  [/web_search|search_(?:run|files)/, "search"],
  [/browse|watch|http_get|http_post|fetch_url|open_url|webhook/, "browse"],
  [/app_deploy/, "app_deploy"],
  [/code_execute|run_tests|build_check|frontend_build|lint_check|shell_exec|terminal_exec/, "terminal"],
];

// Humanized labels emitted by formatActivityToolName / humanizeStep.
// These are stable display tokens; safe to match on.
const LABEL_DIRECT_MATCH: Record<string, ComputerViewKind> = {
  "open web page": "browse",
  "web search": "search",
  "read files": "file",
  "write files": "file",
  "edit files": "file",
  "list files": "file",
  "search files": "search",
  "frontend build": "terminal",
  "build check": "terminal",
  "test run": "terminal",
  "lint check": "terminal",
  "app deploy": "app_deploy",
  "code execute": "terminal",
  "schedule task": "status",
  "agent planning": "status",
};

const LABEL_FALLBACK_PATTERNS: Array<[RegExp, ComputerViewKind]> = [
  [/\b(file|read|write|edit|patch|diff|source|path)\b/i, "file"],
  [/\b(browse|open page|web page|url|fetch|navigate|website|webhook|watch)\b/i, "browse"],
  [/\b(search|query|find|grep|lookup)\b/i, "search"],
  [/\b(run|exec|build|deploy|test|lint|shell|terminal|command|install|cargo|npm|pnpm|yarn|bash|zsh)\b/i, "terminal"],
];

function viewFromCanonicalName(value: string): ComputerViewKind | null {
  const normalized = (value || "").trim().toLowerCase();
  if (!normalized) return null;
  for (const [pattern, kind] of STEP_TYPE_TO_VIEW) {
    if (pattern.test(normalized)) return kind;
  }
  return null;
}

function tryParseRecord(raw: string | undefined): Record<string, unknown> | null {
  const body = (raw || "").trim();
  if (!body || !body.startsWith("{")) return null;
  try {
    const parsed = JSON.parse(body) as unknown;
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

function structuredRecord(card: ChatStepCard): Record<string, unknown> | null {
  return (
    tryParseRecord(card.payloadView?.body) ||
    tryParseRecord(card.rawDetailFull) ||
    tryParseRecord(card.detailFull) ||
    null
  );
}

function viewFromStructuredRecord(record: Record<string, unknown> | null): ComputerViewKind | null {
  if (!record) return null;
  const kind = str(record.kind).trim().toLowerCase();
  if (kind === "draft_file" || kind === "file_write") return "file";
  if (kind === "console_chunk") return "terminal";

  const canonical =
    viewFromCanonicalName(str(record.tool_name)) ||
    viewFromCanonicalName(str(record.name)) ||
    viewFromCanonicalName(str(record.step_type));
  if (canonical) return canonical;

  const hasFileRef = Boolean(str(record.file) || str(record.path));
  const hasAppRef = Boolean(
    str(record.app_id) ||
      str(record.app_dir) ||
      record.files ||
      str(record.local_url) ||
      str(record.access_url),
  );
  if (hasAppRef) return "app_deploy";
  if (hasFileRef) return "file";
  if (str(record.url) || str(record.href)) return "browse";
  return null;
}

export function pickComputerView(card: ChatStepCard): ComputerViewKind {
  const direct = viewFromCanonicalName(card.stepType || "");
  if (direct) return direct;

  const structured = viewFromStructuredRecord(structuredRecord(card));
  if (structured) return structured;

  const label = (card.label || "").trim().toLowerCase();
  if (label && LABEL_DIRECT_MATCH[label]) return LABEL_DIRECT_MATCH[label];
  for (const [pattern, kind] of LABEL_FALLBACK_PATTERNS) {
    if (pattern.test(label)) return kind;
  }
  return "status";
}

const URL_RE = /https?:\/\/[^\s)<>"']+/i;
const PATH_RE = /(?:^|\s)([\w./\\-]+\.[A-Za-z0-9]{1,8})(?=$|\s|[,)])/;
const COMMAND_RE = /^[$#>%]\s*([^\n]+)/m;

function pickFromSources(sources: Array<string | undefined>, re: RegExp): string {
  for (const src of sources) {
    if (!src) continue;
    const m = src.match(re);
    if (m) return m[1] ?? m[0];
  }
  return "";
}

export function extractUrl(card: ChatStepCard): string {
  return pickFromSources(
    [card.detail, card.summary, card.detailFull, card.rawDetailFull, card.payloadView?.body],
    URL_RE,
  );
}

export function extractFilePath(card: ChatStepCard): string {
  const record = structuredRecord(card);
  if (record) {
    const direct = str(record.file) || str(record.path);
    if (direct) return direct;
    const files = record.files;
    if (files && typeof files === "object" && !Array.isArray(files)) {
      const first = Object.keys(files as Record<string, unknown>).find(Boolean);
      if (first) return first;
    }
    if (Array.isArray(files)) {
      for (const entry of files) {
        if (!entry || typeof entry !== "object") continue;
        const rec = entry as Record<string, unknown>;
        const path = str(rec.path) || str(rec.file) || str(rec.name);
        if (path) return path;
      }
    }
  }
  return pickFromSources(
    [card.label, card.detail, card.summary, card.detailFull],
    PATH_RE,
  );
}

export function extractCommand(card: ChatStepCard): string {
  const explicit = pickFromSources(
    [card.detail, card.summary, card.detailFull, card.payloadView?.body],
    COMMAND_RE,
  );
  if (explicit) return explicit.trim();
  const first = (card.detail || card.summary || "").split(/\r?\n/)[0]?.trim() || "";
  if (!first) return "";
  // Reject JSON-looking candidates; they are not shell commands and render
  // badly as the chip's secondary text (e.g. `{ "flow_kind": "chat", ... }`).
  if (first.startsWith("{") || first.startsWith("[") || first.startsWith('"'))
    return "";
  if (/^[a-z][\w@./-]*\s/i.test(first) && first.length < 200) return first;
  return "";
}

export function chipStatusFromCard(
  card: ChatStepCard,
  isLastLive: boolean,
  runIsLive: boolean = true,
): ChipStatus {
  const k = (card.kind || "").toLowerCase();
  if (k.includes("issue") || k.includes("error") || k.includes("fail")) return "issue";
  if (k.includes("done") || k.includes("complete") || k.includes("success")) return "done";
  if (isLastLive) return "running";
  // Once the run has ended, a card whose kind still says "running" or
  // "planning" is stale — the backend never emitted a closing event for it.
  // Treat it as completed so the strip doesn't keep spinning forever.
  if (!runIsLive) return "done";
  if (k.includes("running") || k.includes("planning")) return "running";
  return "idle";
}

// Collapse consecutive cards that share a label to a single chip,
// keeping the most recent (so its kind/status is the freshest).
export function collapseChipCards(cards: ChatStepCard[]): ChatStepCard[] {
  const out: ChatStepCard[] = [];
  for (const card of cards) {
    const prev = out[out.length - 1];
    if (
      prev &&
      prev.label.trim().toLowerCase() === card.label.trim().toLowerCase()
    ) {
      out[out.length - 1] = card;
      continue;
    }
    out.push(card);
  }
  return out;
}

// Lifecycle / orchestration step types & labels that the agent loop emits as
// phase markers. They belong in the Activity tab, not in the chip strip,
// where each iteration would otherwise add another `Agent planning` chip.
const LIFECYCLE_STEP_TYPE_RE =
  /^(agent_loop|agent_turn_loop|planning|thinking|reasoning_delta|heartbeat|classifier|security[_-]|preparing[_-]|selecting[_-]|calling[_-]?model|processing[_-]|model[_-]?call|run[_-]?status|action[_-]?scope|turn[_-]?(?:request|response|plan))/i;
const LIFECYCLE_LABELS = new Set([
  "agent planning",
  "thinking",
  "preparing context",
  "preparing turn plan",
  "preparing intent plan",
  "selecting actions",
  "calling model",
  "running actions",
  "processing action output",
  "first content",
  "first token",
  "turn request",
  "turn response",
  "run completed",
  "classifier",
]);

function isLifecycleCard(card: ChatStepCard): boolean {
  const record = structuredRecord(card);
  const structuredKind = record ? str(record.kind).trim().toLowerCase() : "";
  const structuredPhase = record ? str(record.phase).trim() : "";
  const hasReasoningPayload = Boolean(
    record &&
      structuredPhase &&
      (str(record.content) ||
        str(record.content_delta) ||
        str(record.content_snapshot)),
  );
  if (
    structuredKind === "agent_loop_progress" ||
    structuredKind === "model_prose" ||
    structuredKind === "reasoning_delta" ||
    structuredKind === "turn_completed" ||
    structuredKind === "classifier" ||
    str(record?.tool_name).trim().toLowerCase() === "turn" ||
    str(record?.name).trim().toLowerCase() === "turn" ||
    str(record?.tool_name).trim().toLowerCase() === "classifier" ||
    str(record?.name).trim().toLowerCase() === "classifier" ||
    hasReasoningPayload
  ) {
    return true;
  }
  const stepType = (card.stepType || "").trim().toLowerCase();
  if (stepType && LIFECYCLE_STEP_TYPE_RE.test(stepType)) return true;
  const label = (card.label || "").trim().toLowerCase();
  return LIFECYCLE_LABELS.has(label);
}

// Keep only the last occurrence of each label (case-insensitive), preserving
// the original order of the survivors. Unlike collapseChipCards this dedupes
// across non-consecutive duplicates. This matters because the agent loop
// alternates planning/file_read/planning/file_read across iterations.
function dedupeByLabel(cards: ChatStepCard[]): ChatStepCard[] {
  const lastIndex = new Map<string, number>();
  cards.forEach((card, idx) => {
    lastIndex.set(card.label.trim().toLowerCase(), idx);
  });
  return cards.filter(
    (card, idx) => lastIndex.get(card.label.trim().toLowerCase()) === idx,
  );
}

// Build the final chip list: drop heartbeats, drop lifecycle/orchestration
// steps (they live in the Activity tab or prose lane), and dedupe what remains.
export function prepareChipCards(cards: ChatStepCard[]): ChatStepCard[] {
  if (!cards || cards.length === 0) return [];
  const meaningful = cards.filter((c) => !c.isHeartbeat);
  const tools = dedupeByLabel(meaningful.filter((c) => !isLifecycleCard(c)));
  if (tools.length > 0) return tools;
  return [];
}
