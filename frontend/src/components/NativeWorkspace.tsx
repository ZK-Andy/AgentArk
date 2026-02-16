import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Alert,
  Autocomplete,
  Avatar,
  Box,
  Button,
  Checkbox,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  FormControlLabel,
  Grid2,
  IconButton,
  List,
  ListItem,
  ListItemText,
  Menu,
  MenuItem,
  Stack,
  Switch,
  Tab,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Tabs,
  TextField,
  Tooltip,
  Typography
} from "@mui/material";
import ExpandMoreIcon from "@mui/icons-material/ExpandMore";
import ArrowDropDownRoundedIcon from "@mui/icons-material/ArrowDropDownRounded";
import ContentCopyRoundedIcon from "@mui/icons-material/ContentCopyRounded";
import MoreVertIcon from "@mui/icons-material/MoreVert";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState, type ChangeEvent, type DragEvent, type MouseEvent } from "react";
import ReactECharts from "echarts-for-react";
import { api } from "../api/client";
import AgentLogo from "../assets/logo.svg";
import { IntegrationsPanel } from "./IntegrationsPanel";
import type { SkillImportResponse, LlmAnalyticsResponse } from "../types";
import { useUiStore } from "../store/uiStore";

const REFRESH_MS = 8000;
const IMPORT_SECURITY_FORCE_THRESHOLD = 20;
const DEVELOPER_MODE_STORAGE_KEY = "agentark.developer_mode";
const DEVELOPER_MODE_EVENT = "agentark:developer-mode-change";
const OLLAMA_DEFAULT_BASE_URL = "http://localhost:11434";
const OPENROUTER_DEFAULT_BASE_URL = "https://openrouter.ai/api/v1";
const SHOW_EXPERIMENTAL_AUTONOMY_TOOLS = false;

function getDeveloperModeEnabled(): boolean {
  if (typeof window === "undefined") return false;
  try {
    return window.localStorage.getItem(DEVELOPER_MODE_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

function setDeveloperModeEnabled(next: boolean): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(DEVELOPER_MODE_STORAGE_KEY, next ? "1" : "0");
  } catch {
    // Ignore storage write errors and still emit event for current session.
  }
  window.dispatchEvent(new CustomEvent(DEVELOPER_MODE_EVENT, { detail: { enabled: next } }));
}

type JsonRecord = Record<string, unknown>;
type PasswordDialogMode = "set" | "change" | "remove";
type VaultEditorMode = "add" | "edit";
type RowMenuAction = {
  label: string;
  onClick: () => void | Promise<void>;
  disabled?: boolean;
  tone?: "default" | "warning" | "error";
  divider?: boolean;
};

type TrustApprovalPreset = {
  id: string;
  label: string;
  actionKind: string;
  detailLabel: string;
  detailPlaceholder: string;
  buildPayload: (detail: string) => JsonRecord;
};

const TRUST_APPROVAL_PRESETS: TrustApprovalPreset[] = [
  {
    id: "run_terminal_command",
    label: "Run a terminal command",
    actionKind: "shell",
    detailLabel: "Command",
    detailPlaceholder: "ls -la",
    buildPayload: (detail) => ({ command: detail })
  },
  {
    id: "read_file",
    label: "Read a file",
    actionKind: "file_read",
    detailLabel: "File path",
    detailPlaceholder: "/app/data/report.txt",
    buildPayload: (detail) => ({ path: detail })
  },
  {
    id: "write_file",
    label: "Create or edit a file",
    actionKind: "file_write",
    detailLabel: "File path",
    detailPlaceholder: "/app/data/notes.txt",
    buildPayload: (detail) => ({ path: detail, operation: "write" })
  },
  {
    id: "open_url",
    label: "Open a URL or call an API",
    actionKind: "http_get",
    detailLabel: "URL",
    detailPlaceholder: "https://api.example.com/status",
    buildPayload: (detail) => ({ url: detail })
  },
  {
    id: "run_code",
    label: "Run generated code",
    actionKind: "code_execute",
    detailLabel: "What should the code do?",
    detailPlaceholder: "Summarize CSV rows and return totals",
    buildPayload: (detail) => ({ instruction: detail })
  },
  {
    id: "email_action",
    label: "Read or send an email",
    actionKind: "gmail_reply",
    detailLabel: "Email task",
    detailPlaceholder: "Reply with a short status update",
    buildPayload: (detail) => ({ message: detail })
  }
];

type SkillImportSummary = {
  result: SkillImportResponse;
  message?: string;
};

type ImportCallback = (summary: SkillImportSummary) => Promise<void> | void;

type SkillEditorForm = {
  name: string;
  description: string;
  version: string;
  requiredInputsCsv: string;
  emoji: string;
  toolsCsv: string;
  workflow: string;
};

export type WorkspaceView =
  | "chat"
  | "tasks"
  | "skills"
  | "apps"
  | "goals"
  | "autonomy"
  | "documents"
  | "memory"
  | "projects"
  | "swarm"
  | "trace"
  | "status"
  | "settings";

function isRecord(value: unknown): value is JsonRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function asRecord(value: unknown): JsonRecord {
  return isRecord(value) ? value : {};
}

function asRecords(value: unknown): JsonRecord[] {
  if (!Array.isArray(value)) return [];
  return value.filter(isRecord);
}

function pickRecords(value: unknown, ...keys: string[]): JsonRecord[] {
  if (Array.isArray(value)) return asRecords(value);
  const obj = asRecord(value);
  for (const key of keys) {
    if (Array.isArray(obj[key])) return asRecords(obj[key]);
  }
  return [];
}

function str(value: unknown, fallback = "-"): string {
  if (typeof value === "string" && value.trim()) return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return fallback;
}

function num(value: unknown, fallback = 0): number {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string") {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return fallback;
}

function boolText(value: unknown): string {
  if (typeof value === "boolean") return value ? "true" : "false";
  if (typeof value === "string") return value;
  if (typeof value === "number") return value === 0 ? "false" : "true";
  return "false";
}

function toBool(value: unknown): boolean {
  if (typeof value === "boolean") return value;
  if (typeof value === "number") return value !== 0;
  if (typeof value === "string") {
    const normalized = value.trim().toLowerCase();
    return normalized === "true" || normalized === "1" || normalized === "yes";
  }
  return false;
}

function formatBytes(value: unknown): string {
  const bytes = num(value, -1);
  if (bytes < 0) return "-";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function generateConversationId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `conv-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

function stripAttachmentContextMarker(text: string): string {
  return text
    .replace(/\n\n\[Attached documents indexed for retrieval:[\s\S]*\]$/i, "")
    .trimEnd();
}

const CHAT_ATTACHMENT_EXTENSIONS = new Set([
  "txt",
  "md",
  "markdown",
  "json",
  "csv",
  "tsv",
  "xml",
  "yaml",
  "yml",
  "pdf",
  "docx",
  "log",
  "html",
  "htm"
]);

function splitSupportedChatAttachments(files: File[]): { accepted: File[]; rejected: string[] } {
  const accepted: File[] = [];
  const rejected: string[] = [];
  for (const file of files) {
    const name = (file.name || "").trim();
    const dotIdx = name.lastIndexOf(".");
    const ext = dotIdx >= 0 ? name.slice(dotIdx + 1).toLowerCase() : "";
    if (CHAT_ATTACHMENT_EXTENSIONS.has(ext)) {
      accepted.push(file);
    } else {
      rejected.push(name || "unnamed-file");
    }
  }
  return { accepted, rejected };
}

function formatDurationClock(totalSeconds: number): string {
  const seconds = Math.max(0, Math.floor(totalSeconds));
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;
  if (days > 0) return `${days}d ${hours}h ${minutes}m`;
  if (hours > 0) return `${hours}h ${minutes}m ${secs}s`;
  if (minutes > 0) return `${minutes}m ${secs}s`;
  return `${secs}s`;
}

function errMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  return "Request failed";
}

function defaultSkillEditorForm(name = ""): SkillEditorForm {
  return {
    name: name || "new-action",
    description: "",
    version: "1.0.0",
    requiredInputsCsv: "",
    emoji: "",
    toolsCsv: "",
    workflow: ""
  };
}

function splitActionFrontmatter(content: string): { frontmatter: string | null; body: string } {
  const match = content.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$/);
  if (!match) return { frontmatter: null, body: content };
  return { frontmatter: match[1] ?? "", body: match[2] ?? "" };
}

function unquoteYamlScalar(value: string): string {
  const v = value.trim();
  if (!v) return "";
  if (v.startsWith("\"") && v.endsWith("\"")) {
    try {
      const parsed = JSON.parse(v);
      return typeof parsed === "string" ? parsed : v.slice(1, -1);
    } catch {
      return v.slice(1, -1);
    }
  }
  if (v.startsWith("'") && v.endsWith("'")) return v.slice(1, -1).replace(/''/g, "'");
  return v;
}

function quoteYamlScalar(value: string): string {
  return JSON.stringify(value ?? "");
}

function parseInlineStringArray(value: string): string[] {
  const trimmed = value.trim();
  if (!trimmed) return [];
  if (trimmed.startsWith("[") && trimmed.endsWith("]")) {
    try {
      const parsed = JSON.parse(trimmed);
      if (Array.isArray(parsed)) {
        return parsed
          .map((item) => (typeof item === "string" ? item.trim() : ""))
          .filter(Boolean);
      }
    } catch {
      // Fall through to a tolerant split below.
    }
    const raw = trimmed.slice(1, -1);
    return raw
      .split(",")
      .map((item) => unquoteYamlScalar(item))
      .map((item) => item.trim())
      .filter(Boolean);
  }
  return trimmed
    .split(",")
    .map((item) => unquoteYamlScalar(item))
    .map((item) => item.trim())
    .filter(Boolean);
}

function dedupeStrings(values: string[]): string[] {
  return Array.from(new Set(values.map((item) => item.trim()).filter(Boolean)));
}

function parseToolsCsv(csv: string): string[] {
  return dedupeStrings(
    csv
      .split(",")
      .map((item) => item.trim())
      .filter(Boolean)
  );
}

function parseRequiredInputsCsv(csv: string): string[] {
  return dedupeStrings(
    csv
      .split(",")
      .map((item) =>
        item
          .trim()
          .replace(/[^A-Za-z0-9_-]/g, "")
      )
      .filter(Boolean)
  );
}

type HookTriggerValue =
  | "pre_message"
  | "post_message"
  | "pre_action"
  | "post_action"
  | "on_consolidate"
  | "on_error";

function sanitizeHookName(value: string): string {
  return (value || "")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9-_\s]/g, "")
    .replace(/[_\s]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
}

function inferHookTriggerFromInstruction(text: string, defaultTrigger: HookTriggerValue = "post_action"): HookTriggerValue {
  const t = (text || "").toLowerCase();
  if (!t) return defaultTrigger;
  if (t.includes("on error") || t.includes("error") || t.includes("fail")) return "on_error";
  if (t.includes("before action") || t.includes("pre action")) return "pre_action";
  if (t.includes("after action") || t.includes("post action") || t.includes("on success") || t.includes("when done")) return "post_action";
  if (t.includes("before message") || t.includes("pre message")) return "pre_message";
  if (t.includes("after message") || t.includes("post message") || t.includes("after reply")) return "post_message";
  if (t.includes("consolidate") || t.includes("memory")) return "on_consolidate";
  return defaultTrigger;
}

function extractFirstUrl(text: string): string {
  const m = (text || "").match(/https?:\/\/[^\s]+/i);
  return m ? m[0] : "";
}

function extractCronExpression(text: string): string {
  const tokens = (text || "")
    .trim()
    .split(/\s+/)
    .filter(Boolean);
  const isCronToken = (token: string) => /^[0-9A-Za-z*/,\-]+$/.test(token);
  for (let i = 0; i < tokens.length; i += 1) {
    for (const width of [6, 5]) {
      if (i + width > tokens.length) continue;
      const slice = tokens.slice(i, i + width);
      if (slice.every(isCronToken)) {
        return slice.join(" ");
      }
    }
  }
  return "";
}

function inferTaskCronFromInstruction(text: string): string {
  const t = (text || "").trim().toLowerCase();
  if (!t) return "";
  const explicitCron = extractCronExpression(text);
  if (explicitCron) return explicitCron;

  if (t.includes("every 5") && t.includes("min")) return "*/5 * * * *";
  if (t.includes("every 10") && t.includes("min")) return "*/10 * * * *";
  if (t.includes("every 15") && t.includes("min")) return "*/15 * * * *";
  if (t.includes("every 30") && t.includes("min")) return "*/30 * * * *";
  if (t.includes("hourly") || t.includes("every hour")) return "0 * * * *";
  if (t.includes("weekday")) return "0 9 * * 1-5";
  if (t.includes("weekly")) return "0 9 * * 1";
  if (t.includes("monthly")) return "0 9 1 * *";
  if (t.includes("daily") || t.includes("every day")) return "0 9 * * *";
  return "";
}

function isRunOnceInstruction(text: string): boolean {
  const t = (text || "").toLowerCase();
  return t.includes("once") || t.includes("now") || t.includes("immediately");
}

function isHookAttachedToAction(hookName: string, actionName: string): boolean {
  const hn = sanitizeHookName(hookName);
  const an = sanitizeHookName(actionName);
  if (!hn || !an) return false;
  return hn.startsWith(`action-${an}-`);
}

function isHookRecordAttachedToAction(hook: JsonRecord, actionName: string): boolean {
  const explicit = sanitizeHookName(str(hook.action_name, ""));
  const an = sanitizeHookName(actionName);
  if (explicit && an && explicit === an) return true;
  return isHookAttachedToAction(str(hook.name, ""), actionName);
}

function parseSkillEditorForm(content: string, fallbackName: string): SkillEditorForm {
  const { frontmatter, body } = splitActionFrontmatter(content);
  const form = defaultSkillEditorForm(fallbackName);
  if (!frontmatter) {
    form.workflow = content.trim();
    return form;
  }

  const tools: string[] = [];
  const requiredInputs: string[] = [];
  let section: string | null = null;
  let listTarget: "tools" | "requiredInputs" | null = null;
  const lines = frontmatter.split(/\r?\n/);
  for (const rawLine of lines) {
    const line = rawLine.replace(/\t/g, "  ");
    if (!line.trim()) continue;

    const top = line.match(/^([A-Za-z0-9_-]+):\s*(.*)$/);
    if (top) {
      const key = top[1];
      const value = top[2].trim();
      section = null;
      listTarget = null;

      if (key === "name") {
        if (value) form.name = unquoteYamlScalar(value);
        continue;
      }
      if (key === "description") {
        if (value) form.description = unquoteYamlScalar(value);
        continue;
      }
      if (key === "version") {
        if (value) form.version = unquoteYamlScalar(value);
        continue;
      }
      if (key === "required_inputs" || key === "requiredInputs" || key === "required") {
        if (value) {
          requiredInputs.push(...parseInlineStringArray(value));
        } else {
          section = "required_inputs";
          listTarget = "requiredInputs";
        }
        continue;
      }
      if (key === "metadata") {
        if (value) {
          const m = value.match(/emoji\s*:\s*(.+)$/);
          if (m) form.emoji = unquoteYamlScalar(m[1]);
        } else {
          section = "metadata";
        }
        continue;
      }
      if (key === "requires") {
        if (value) {
          const m = value.match(/tools\s*:\s*(.+)$/);
          if (m) tools.push(...parseInlineStringArray(m[1]));
        } else {
          section = "requires";
        }
        continue;
      }
      continue;
    }

    const nested = line.match(/^\s{2,}([A-Za-z0-9_-]+):\s*(.*)$/);
    if (nested && section) {
      const key = nested[1];
      const value = nested[2].trim();
      listTarget = null;
      if (section === "metadata" && key === "emoji") {
        form.emoji = unquoteYamlScalar(value);
        continue;
      }
      if (section === "requires" && key === "tools") {
        if (value) {
          tools.push(...parseInlineStringArray(value));
        } else {
          listTarget = "tools";
        }
        continue;
      }
      continue;
    }

    const listItem = line.match(/^\s*-\s*(.+)$/);
    if (listItem && section === "requires" && listTarget === "tools") {
      tools.push(unquoteYamlScalar(listItem[1]));
      continue;
    }
    if (listItem && section === "required_inputs" && listTarget === "requiredInputs") {
      requiredInputs.push(unquoteYamlScalar(listItem[1]));
      continue;
    }
  }

  form.toolsCsv = dedupeStrings(tools).join(", ");
  form.requiredInputsCsv = parseRequiredInputsCsv(requiredInputs.join(", ")).join(", ");
  form.workflow = body.trim();
  if (!form.name.trim()) form.name = fallbackName || "new-action";
  if (!form.version.trim()) form.version = "1.0.0";
  return form;
}

function extractUnknownFrontmatterLines(frontmatter: string): string[] {
  const lines = frontmatter.split(/\r?\n/);
  const kept: string[] = [];
  let skipKnownBlock = false;

  for (const line of lines) {
    const top = line.match(/^([A-Za-z0-9_-]+):\s*(.*)$/);
    if (top) {
      const key = top[1];
      if (
        key === "name" ||
        key === "description" ||
        key === "version" ||
        key === "required_inputs" ||
        key === "requiredInputs" ||
        key === "required"
      ) {
        skipKnownBlock = false;
        continue;
      }
      if (key === "metadata" || key === "requires") {
        skipKnownBlock = true;
        continue;
      }
      skipKnownBlock = false;
      kept.push(line);
      continue;
    }

    if (skipKnownBlock) {
      if (line.trim() === "" || /^\s+/.test(line)) continue;
    }
    kept.push(line);
  }

  while (kept.length > 0 && !kept[0].trim()) kept.shift();
  while (kept.length > 0 && !kept[kept.length - 1].trim()) kept.pop();
  return kept;
}

function buildSkillMdFromForm(currentContent: string, form: SkillEditorForm): string {
  const { frontmatter } = splitActionFrontmatter(currentContent);
  const unknownLines = frontmatter ? extractUnknownFrontmatterLines(frontmatter) : [];
  const tools = parseToolsCsv(form.toolsCsv);
  const requiredInputs = parseRequiredInputsCsv(form.requiredInputsCsv);
  const frontmatterLines = [
    `name: ${quoteYamlScalar((form.name || "").trim())}`,
    `description: ${quoteYamlScalar((form.description || "").trim())}`,
    `version: ${quoteYamlScalar((form.version || "").trim() || "1.0.0")}`,
    `required_inputs: [${requiredInputs.map((item) => quoteYamlScalar(item)).join(", ")}]`,
    "metadata:",
    `  emoji: ${quoteYamlScalar((form.emoji || "").trim())}`,
    "requires:",
    `  tools: [${tools.map((tool) => quoteYamlScalar(tool)).join(", ")}]`
  ];

  if (unknownLines.length > 0) {
    frontmatterLines.push("");
    frontmatterLines.push(...unknownLines);
  }

  const workflow = (form.workflow || "").trim();
  return `---\n${frontmatterLines.join("\n")}\n---\n\n${workflow}\n`;
}

function normalizeActionName(value: string): string {
  return (value || "")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9-_\s]/g, "")
    .replace(/[_\s]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
}

function isValidActionName(value: string): boolean {
  return /^[a-z0-9-]+$/.test((value || "").trim());
}

function extractActionMdFromModelOutput(text: string): string {
  const raw = (text || "").trim();
  if (!raw) return "";

  // Prefer fenced markdown/code blocks when present.
  const fenceRegex = /```(?:md|markdown|txt|yaml)?\s*([\s\S]*?)```/gi;
  const blocks: string[] = [];
  let match: RegExpExecArray | null = null;
  while ((match = fenceRegex.exec(raw)) !== null) {
    blocks.push((match[1] || "").trim());
  }
  for (const block of blocks) {
    if (block.startsWith("---")) return block;
  }
  if (blocks.length > 0) return blocks[0];

  if (raw.startsWith("---")) return raw;
  const frontmatterIdx = raw.indexOf("\n---\n");
  if (frontmatterIdx >= 0) {
    const maybe = raw.slice(raw.lastIndexOf("---", frontmatterIdx - 1)).trim();
    if (maybe.startsWith("---")) return maybe;
  }
  return raw;
}

function formatCompactValue(value: unknown): { text: string; tooltip?: string } {
  if (value == null) return { text: "-" };
  if (typeof value === "string") return { text: value };
  if (typeof value === "number") return { text: Number.isFinite(value) ? String(value) : "-" };
  if (typeof value === "boolean") return { text: value ? "true" : "false" };

  if (Array.isArray(value)) {
    // Avoid dumping JSON; just give a hint.
    const sample = value
      .slice(0, 4)
      .map((v) => (typeof v === "string" ? v : typeof v === "number" ? String(v) : typeof v === "boolean" ? (v ? "true" : "false") : "…"))
      .join(", ");
    const tooltip = sample ? `Examples: ${sample}${value.length > 4 ? ` (+${value.length - 4})` : ""}` : undefined;
    return { text: `List (${value.length})`, tooltip };
  }

  if (typeof value === "object") {
    const rec = asRecord(value);
    const title = str(rec.title, "") || str(rec.name, "") || str(rec.id, "");
    const keys = Object.keys(rec);
    const keyHint = keys.slice(0, 4).join(", ");
    const more = keys.length > 4 ? `, +${keys.length - 4}` : "";
    const tooltip = keys.length ? `Fields: ${keyHint}${more}` : undefined;
    if (title) return { text: title, tooltip };
    return { text: keys.length ? `Object(${keyHint}${more})` : "Object", tooltip };
  }

  return { text: String(value) };
}

function looksLikeUrl(value: string): boolean {
  const v = (value || "").trim();
  return v.startsWith("http://") || v.startsWith("https://");
}

function looksLikeUuid(value: string): boolean {
  const v = (value || "").trim();
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(v);
}

function looksLikeIsoTimestamp(value: string): boolean {
  const v = (value || "").trim();
  if (!/^\d{4}-\d{2}-\d{2}T/.test(v)) return false;
  const dt = new Date(v);
  return !Number.isNaN(dt.getTime());
}

function formatTimestampForHumans(value: string): { label: string; tooltip: string } {
  const dt = new Date(value);
  if (Number.isNaN(dt.getTime())) return { label: value, tooltip: value };
  const now = new Date();
  const sameYear = dt.getFullYear() === now.getFullYear();
  const fmt = new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "2-digit",
    ...(sameYear ? {} : { year: "numeric" }),
    hour: "2-digit",
    minute: "2-digit"
  });
  return { label: fmt.format(dt), tooltip: value };
}

function formatDurationFromSeconds(value: unknown): string {
  const total = num(value, -1);
  if (total < 0) return "-";
  const sec = Math.floor(total);
  if (sec < 60) return `${sec}s`;
  const mins = Math.floor(sec / 60);
  const remSec = sec % 60;
  if (mins < 60) return remSec > 0 ? `${mins}m ${remSec}s` : `${mins}m`;
  const hours = Math.floor(mins / 60);
  const remMins = mins % 60;
  if (hours < 24) return remMins > 0 ? `${hours}h ${remMins}m` : `${hours}h`;
  const days = Math.floor(hours / 24);
  const remHours = hours % 24;
  return remHours > 0 ? `${days}d ${remHours}h` : `${days}d`;
}

function boolLabelForKey(key: string, value: boolean): { label: string; color: "success" | "warning" | "default" } {
  const k = (key || "").toLowerCase();
  if (k.includes("enabled")) return { label: value ? "Enabled" : "Disabled", color: value ? "success" : "warning" };
  if (k.includes("active")) return { label: value ? "Active" : "Inactive", color: value ? "success" : "warning" };
  if (k.includes("connected")) return { label: value ? "Connected" : "Not connected", color: value ? "success" : "warning" };
  return { label: value ? "Yes" : "No", color: value ? "success" : "default" };
}

function DataTable({ rows, columns }: { rows: JsonRecord[]; columns: string[] }) {
  return (
    <TableContainer className="table-shell">
      <Table size="small">
        <TableHead>
          <TableRow>
            {columns.map((column) => (
              <TableCell key={column}>{column}</TableCell>
            ))}
          </TableRow>
        </TableHead>
        <TableBody>
          {rows.map((row, index) => (
            <TableRow key={str(row.id, String(index))}>
              {columns.map((column) => (
                <TableCell key={`${index}-${column}`}>
                  <Typography variant="caption" sx={{ whiteSpace: "pre-wrap" }}>
                    {(() => {
                      const v = row[column];
                      const out = formatCompactValue(v);
                      return (
                        <span title={out.tooltip || ""}>
                          {out.text}
                        </span>
                      );
                    })()}
                  </Typography>
                </TableCell>
              ))}
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </TableContainer>
  );
}

function KeyValuePanel({
  title,
  data,
  emptyLabel,
  maxRows
}: {
  title: string;
  data: JsonRecord;
  emptyLabel?: string;
  maxRows?: number;
}) {
  const entries = Object.entries(data || {});
  const shown = entries.slice(0, maxRows ?? 14);
  return (
    <Box className="metadata-box">
      <Typography variant="caption" color="text.secondary">
        {title}
      </Typography>
      <Stack spacing={0.6} sx={{ mt: 0.75 }}>
        {shown.length === 0 ? (
          <Typography variant="body2" color="text.secondary">
            {emptyLabel || "No details available."}
          </Typography>
        ) : (
          shown.map(([k, v]) => {
            const out = formatCompactValue(v);
            const keyLower = (k || "").toLowerCase();
            const renderValue = () => {
              if (typeof v === "string" && looksLikeUrl(v)) {
                const trimmed = v.trim();
                const label = trimmed.length > 54 ? `${trimmed.slice(0, 54)}…` : trimmed;
                return (
                  <Typography variant="body2" sx={{ wordBreak: "break-all" }} title={trimmed}>
                    <a href={trimmed} target="_blank" rel="noreferrer" style={{ color: "inherit", textDecoration: "underline" }}>
                      {label}
                    </a>
                  </Typography>
                );
              }
              if (typeof v === "string" && (looksLikeIsoTimestamp(v) || keyLower.endsWith("_at") || keyLower.includes("timestamp"))) {
                const t = formatTimestampForHumans(v);
                return <Chip size="small" variant="outlined" label={t.label} title={t.tooltip} />;
              }
              if (typeof v === "boolean") {
                const b = boolLabelForKey(k, v);
                return <Chip size="small" label={b.label} color={b.color} variant={v ? "filled" : "outlined"} />;
              }
              if (typeof v === "number" && Number.isFinite(v)) {
                if (keyLower.includes("ms") || keyLower.includes("duration")) {
                  return <Chip size="small" variant="outlined" label={`${Math.round(v)} ms`} />;
                }
                if (keyLower.includes("count") || keyLower.includes("total") || keyLower.includes("remaining")) {
                  return <Chip size="small" variant="outlined" label={String(v)} />;
                }
              }
              if (typeof v === "string" && (looksLikeUuid(v) || keyLower.endsWith("_id") || keyLower === "id")) {
                const trimmed = v.trim();
                const label = trimmed.length > 22 ? `${trimmed.slice(0, 8)}…${trimmed.slice(-6)}` : trimmed;
                return (
                  <Chip
                    size="small"
                    variant="outlined"
                    label={label}
                    title={trimmed}
                    onClick={async () => {
                      try {
                        await navigator.clipboard.writeText(trimmed);
                      } catch {
                        // ignore
                      }
                    }}
                    sx={{ cursor: "pointer" }}
                  />
                );
              }
              return (
                <Typography
                  variant="body2"
                  sx={{ minWidth: 0, flex: "1 1 auto", wordBreak: "break-word" }}
                  title={out.tooltip || ""}
                >
                  {out.text}
                </Typography>
              );
            };
            return (
              <Stack key={k} direction="row" spacing={1} alignItems="baseline">
                <Typography variant="caption" color="text.secondary" sx={{ width: 160, flex: "0 0 auto" }}>
                  {k}
                </Typography>
                {renderValue()}
              </Stack>
            );
          })
        )}
        {entries.length > shown.length ? (
          <Typography variant="caption" color="text.secondary">
            {entries.length - shown.length} more field(s) not shown.
          </Typography>
        ) : null}
      </Stack>
    </Box>
  );
}

type BulkImportItem = {
  url: string;
  selected: boolean;
  status?: string;
  result?: SkillImportResponse;
};

function BulkImportDialog({
  open,
  onClose,
  onImported,
  onAfterImport
}: {
  open: boolean;
  onClose: () => void;
  onImported?: ImportCallback;
  onAfterImport?: (name: string, importResult: SkillImportResponse) => Promise<void>;
}) {
  const [urlsText, setUrlsText] = useState("");
  const [items, setItems] = useState<BulkImportItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [force, setForce] = useState(false);
  const [model, setModel] = useState("");

  const parseUrlsFromText = (text: string): string[] => {
    const urls = text
      .split(/\r?\n/)
      .map((l) => l.trim())
      .filter((l) => l.length > 0 && !l.startsWith("#"))
      .map((u) => {
        // Auto-fix common GitHub mistake: /blob/ → /tree/ for folder URLs
        if (u.includes("github.com/") && u.includes("/blob/") && !u.match(/\.\w+$/)) {
          return u.replace("/blob/", "/tree/");
        }
        return u;
      });
    const uniq: string[] = [];
    for (const u of urls) {
      if (!uniq.includes(u)) uniq.push(u);
    }
    return uniq;
  };

  useEffect(() => {
    if (!open) {
      setError(null);
      setLoading(false);
      setImporting(false);
      return;
    }
    setUrlsText("");
    setItems([]);
  }, [open]);

  const parseUrls = () => {
    const uniq = parseUrlsFromText(urlsText);
    setItems(uniq.map((url) => ({ url, selected: true })));
  };

  const handleImportSelected = async () => {
    // If the user never clicked "Preview list", build the list automatically.
    const effectiveItems =
      items.length > 0 ? items : parseUrlsFromText(urlsText).map((url) => ({ url, selected: true } as BulkImportItem));

    if (items.length === 0 && effectiveItems.length > 0) {
      setItems(effectiveItems);
    }

    const toImport = effectiveItems.filter((item) => item.selected);
    if (!toImport.length) return;
    setImporting(true);
    for (const item of toImport) {
      setItems((prev) =>
        prev.map((x) => (x.url === item.url ? { ...x, status: "Importing..." } : x))
      );
      try {
        const result = await api.importSkill({ url: item.url, force, model: model.trim() || undefined });
        let statusMessage = result.message || `Imported ${result.name}`;
        if (result.status === "blocked") {
          statusMessage = result.message || "Blocked by security verification (toggle Force and retry).";
        } else if (result.status === "needs_secrets") {
          statusMessage = result.message || `Imported ${result.name} (disabled until secrets are configured)`;
        }
        setItems((prev) =>
          prev.map((x) => (x.url === item.url ? { ...x, status: statusMessage, result } : x))
        );
        const importedChildren = Array.isArray(result.imported) ? result.imported : [];
        if (importedChildren.length > 0) {
          for (const child of importedChildren) {
            const childResult = child?.result;
            if (!childResult?.name) continue;
            const childMessage =
              childResult.message ||
              (childResult.status === "needs_secrets"
                ? `Imported ${childResult.name} (disabled until secrets are configured)`
                : `Imported ${childResult.name}`);
            await onAfterImport?.(childResult.name, childResult);
            await onImported?.({ result: childResult, message: childMessage });
          }
        } else {
          await onAfterImport?.(result.name, result);
          await onImported?.({ result, message: statusMessage });
        }
      } catch (err) {
        const message = `Error: ${errMessage(err)}`;
        setItems((prev) =>
          prev.map((x) => (x.url === item.url ? { ...x, status: message } : x))
        );
      }
    }
    setImporting(false);
  };

  return (
    <Dialog open={open} onClose={onClose} maxWidth="md" fullWidth>
      <DialogTitle>Bulk Import</DialogTitle>
      <DialogContent dividers>
        <Stack spacing={1.25}>
          {error ? <Alert severity="error">{error}</Alert> : null}
          <Typography variant="body2" color="text.secondary">
            Paste one or more skill URLs (one per line). Use <code>/tree/</code> for GitHub folders, not <code>/blob/</code>.
          </Typography>
          <Alert severity="info" variant="outlined" sx={{ py: 0.25, "& .MuiAlert-message": { fontSize: "0.75rem" } }}>
            Getting 403 errors? GitHub rate-limits unauthenticated requests. Go to Settings &gt; Integrations &gt; GitHub and add a Personal Access Token for higher limits.
          </Alert>
          <Typography variant="caption" color="text.secondary" sx={{ whiteSpace: "pre-line" }}>
            {`Examples:
https://github.com/org/repo/tree/main/skills
https://raw.githubusercontent.com/org/repo/main/skills/my-skill/SKILL.md`}
          </Typography>
          <TextField
            fullWidth
            multiline
            minRows={3}
            maxRows={8}
            label="Import URLs"
            value={urlsText}
            onChange={(e) => setUrlsText(e.target.value)}
            placeholder={"https://github.com/openclaw/skills/tree/main/skills"}
          />
          <TextField
            fullWidth
            size="small"
            label="Model override (optional)"
            value={model}
            onChange={(e) => setModel(e.target.value)}
          />
          <FormControlLabel control={<Switch checked={force} onChange={(e) => setForce(e.target.checked)} />} label="Force import (skip security checks)" />
          {items.length > 0 ? (
            <Stack spacing={0.5}>
              {items.map((it) => (
                <Box key={it.url} className="console-line">
                  <Typography variant="body2" noWrap title={it.url}>
                    {it.url}
                  </Typography>
                  <Typography variant="caption" color={it.status?.startsWith("Error") ? "error" : "text.secondary"}>
                    {it.status || "Pending"}
                  </Typography>
                </Box>
              ))}
            </Stack>
          ) : null}
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>Close</Button>
        <Button
          variant="contained"
          disabled={importing || loading || !urlsText.trim()}
          onClick={handleImportSelected}
        >
          {importing ? "Importing..." : "Import"}
        </Button>
      </DialogActions>
    </Dialog>
  );
}

function ImportUrlDialog({
  open,
  onClose,
  onImported,
  onAfterImport
}: {
  open: boolean;
  onClose: () => void;
  onImported?: ImportCallback;
  onAfterImport?: (name: string, importResult: SkillImportResponse) => Promise<void>;
}) {
  const [url, setUrl] = useState("");
  const [model, setModel] = useState("");
  const [force, setForce] = useState(false);
  const [loading, setLoading] = useState(false);
  const [previewReady, setPreviewReady] = useState(false);
  const [importCommitted, setImportCommitted] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [importResult, setImportResult] = useState<SkillImportResponse | null>(null);
  const [secretDrafts, setSecretDrafts] = useState<Record<string, { storeAs: string; value: string; useBuiltin: boolean }>>({});
  const [savingSecrets, setSavingSecrets] = useState(false);
  const [secretsSaved, setSecretsSaved] = useState(false);
  const severityScore = useMemo(() => {
    if (!importResult?.security) return 0;
    const explicit = num(importResult.security.total_severity, 0);
    if (explicit > 0) return explicit;
    if (!Array.isArray(importResult.security.findings)) return 0;
    return importResult.security.findings.reduce((sum, finding) => {
      const f = asRecord(finding);
      return sum + Math.max(0, num(f.severity, 0));
    }, 0);
  }, [importResult]);
  const securityBlocked = toBool(importResult?.security?.blocked);
  const importRequiresForce =
    previewReady && !force && (securityBlocked || severityScore >= IMPORT_SECURITY_FORCE_THRESHOLD);

  const buildSecretDraftsFromResult = (result: SkillImportResponse) => {
    const required = result.secrets?.required_env || [];
    const bindings = result.secrets?.bindings || {};
    const drafts: Record<string, { storeAs: string; value: string; useBuiltin: boolean }> = {};
    for (const env of required) {
      const binding = bindings[env];
      drafts[env] = {
        storeAs: binding && binding !== "builtin" ? binding : env,
        value: "",
        useBuiltin: binding === "builtin"
      };
    }
    setSecretDrafts(drafts);
  };

  const runImport = async (previewOnly: boolean) => {
    if (!url.trim()) return;
    setLoading(true);
    setError(null);
    setInfo(null);
    setSecretsSaved(false);
    if (previewOnly) {
      setImportCommitted(false);
    }
    try {
      const result = await api.importSkill({
        url: url.trim(),
        model: model.trim() || undefined,
        force,
        preview_only: previewOnly
      });

      setImportResult(result);
      buildSecretDraftsFromResult(result);

      let message = result.message || (previewOnly ? `Preview ready for ${result.name}` : `Imported ${result.name}`);
      if (result.status === "blocked") {
        message = result.message || "Blocked by security verification. Toggle Force to override.";
      } else if (!previewOnly && result.status === "needs_secrets") {
        message = result.message || `Imported ${result.name} (disabled until secrets are configured)`;
      }
      setInfo(message);

      if (previewOnly) {
        setPreviewReady(true);
        return;
      }

      setPreviewReady(false);
      setImportCommitted(true);
      const importedChildren = Array.isArray(result.imported) ? result.imported : [];
      if (importedChildren.length > 0) {
        for (const child of importedChildren) {
          const childResult = child?.result;
          if (!childResult?.name) continue;
          const childMessage =
            childResult.message ||
            (childResult.status === "needs_secrets"
              ? `Imported ${childResult.name} (disabled until secrets are configured)`
              : `Imported ${childResult.name}`);
          await onAfterImport?.(childResult.name, childResult);
          await onImported?.({ result: childResult, message: childMessage });
        }
      } else {
        await onAfterImport?.(result.name, result);
        await onImported?.({ result, message });
      }
    } catch (err) {
      setError(errMessage(err));
    } finally {
      setLoading(false);
    }
  };

  const handleAnalyze = async () => runImport(true);
  const handleImport = async () => runImport(false);

  const handleSaveSecrets = async () => {
    if (!importResult?.name) return;
    if (!importCommitted) {
      setError("Import template first, then save secrets.");
      return;
    }
    const required = importResult.secrets?.required_env || [];
    if (required.length === 0) return;
    setSavingSecrets(true);
    setError(null);
    try {
      const payload = required.map((env) => {
        const d = secretDrafts[env] || { storeAs: env, value: "", useBuiltin: false };
        if (d.useBuiltin) return { env, store_as: "builtin" };
        const storeAs = (d.storeAs || env).trim();
        const value = (d.value || "").trim();
        return value ? { env, store_as: storeAs, value } : { env, store_as: storeAs };
      });
      const secretsOut = await api.setSkillSecrets(importResult.name, { secrets: payload });
      if ((secretsOut.missing_env || []).length > 0) {
        setError(`Some keys are still missing: ${secretsOut.missing_env.join(", ")}`);
      } else {
        setSecretsSaved(true);
        setInfo("Secrets saved. The skill remains disabled until you manually enable it in Skills.");
      }
    } catch (err) {
      setError(errMessage(err));
    } finally {
      setSavingSecrets(false);
    }
  };

  const handleClose = () => {
    if (loading) return;
    setError(null);
    setInfo(null);
    setImportResult(null);
    setPreviewReady(false);
    setImportCommitted(false);
    setSecretDrafts({});
    setSavingSecrets(false);
    setSecretsSaved(false);
    onClose();
  };

  return (
    <Dialog open={open} onClose={handleClose} maxWidth="sm" fullWidth>
      <DialogTitle>Import from URL</DialogTitle>
      <DialogContent dividers>
        <Stack spacing={1}>
          {error && <Alert severity="error">{error}</Alert>}
          {info && <Alert severity="info">{info}</Alert>}
          <Typography variant="caption" color="text.secondary">
            Supports direct SKILL.md links and GitHub folder/repo URLs.
          </Typography>
          <Typography variant="caption" color="text.secondary" sx={{ whiteSpace: "pre-line" }}>
            {`Examples:
1. https://github.com/org/repo/tree/main/skills/market-analysis
2. https://raw.githubusercontent.com/org/repo/main/skills/market-analysis/SKILL.md`}
          </Typography>
          <TextField
            fullWidth
            size="small"
            label="Import URL"
            value={url}
            onChange={(event) => {
              setUrl(event.target.value);
              setPreviewReady(false);
              setImportCommitted(false);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") event.preventDefault();
            }}
          />
          <TextField
            fullWidth
            size="small"
            label="Model override (optional)"
            value={model}
            onChange={(event) => {
              setModel(event.target.value);
              setPreviewReady(false);
              setImportCommitted(false);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") event.preventDefault();
            }}
          />
          <FormControlLabel control={<Switch checked={force} onChange={(event) => setForce(event.target.checked)} />} label="Force import" />
          {importRequiresForce ? (
            <Alert severity="warning">
              Security severity score {severityScore} exceeds threshold {IMPORT_SECURITY_FORCE_THRESHOLD}. Enable Force import to continue.
            </Alert>
          ) : null}
          {importResult?.security?.warnings?.length ? (
            <Alert severity={importResult.security.blocked ? "warning" : "info"}>
              {importResult.security.warnings.join("\n")}
            </Alert>
          ) : null}
          {importResult?.security ? (
            <Box sx={{ mt: 1 }}>
              <Typography variant="subtitle2" mb={1}>
                Security scan
              </Typography>
              <Stack spacing={1}>
                <Typography variant="body2" color="text.secondary">
                  Threat level: {str(importResult.security.threat_level, "-")} | Blocked: {boolText(importResult.security.blocked)}
                </Typography>
                {Array.isArray(importResult.security.findings) && importResult.security.findings.length > 0 ? (
                  <TableContainer className="table-shell">
                    <Table size="small">
                      <TableHead>
                        <TableRow>
                          <TableCell>Severity</TableCell>
                          <TableCell>Category</TableCell>
                          <TableCell>Line</TableCell>
                          <TableCell>Description</TableCell>
                        </TableRow>
                      </TableHead>
                      <TableBody>
                        {(importResult.security.findings as unknown[]).slice(0, 30).map((rawFinding, idx) => {
                          const f = asRecord(rawFinding);
                          return (
                            <TableRow key={`${idx}-${str(f.category, "")}`}>
                              <TableCell sx={{ whiteSpace: "nowrap" }}>{str(f.severity, "-")}</TableCell>
                              <TableCell sx={{ whiteSpace: "nowrap" }}>{str(f.category, "-")}</TableCell>
                              <TableCell sx={{ whiteSpace: "nowrap" }}>{num(f.line, -1) >= 0 ? num(f.line) : "-"}</TableCell>
                              <TableCell>
                                <Typography variant="body2">{str(f.description, "-")}</Typography>
                                {str(f.matched_text, "").trim() ? (
                                  <Typography variant="caption" color="text.secondary" sx={{ display: "block" }}>
                                    Match: {str(f.matched_text).slice(0, 120)}
                                  </Typography>
                                ) : null}
                              </TableCell>
                            </TableRow>
                          );
                        })}
                      </TableBody>
                    </Table>
                  </TableContainer>
                ) : (
                  <Typography variant="body2" color="text.secondary">
                    No findings.
                  </Typography>
                )}
              </Stack>
            </Box>
          ) : null}
          {Array.isArray(importResult?.imported) && importResult.imported.length > 0 ? (
            <Box sx={{ mt: 1 }}>
              <Typography variant="subtitle2" mb={1}>
                Per-skill security
              </Typography>
              <TableContainer className="table-shell">
                <Table size="small">
                  <TableHead>
                    <TableRow>
                      <TableCell>Skill</TableCell>
                      <TableCell>Status</TableCell>
                      <TableCell>Threat</TableCell>
                      <TableCell>Blocked</TableCell>
                      <TableCell>Warnings</TableCell>
                      <TableCell>Findings</TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {importResult.imported.map((entry, idx) => {
                      const child = entry?.result;
                      const sec = child?.security;
                      const warningsCount = Array.isArray(sec?.warnings) ? sec?.warnings.length : 0;
                      const findingsCount = Array.isArray(sec?.findings) ? sec?.findings.length : 0;
                      return (
                        <TableRow key={`${entry?.url || child?.name || idx}-${idx}`}>
                          <TableCell sx={{ wordBreak: "break-word" }}>{child?.name || "-"}</TableCell>
                          <TableCell>{child?.status || "-"}</TableCell>
                          <TableCell>{str(sec?.threat_level, "-")}</TableCell>
                          <TableCell>{boolText(sec?.blocked)}</TableCell>
                          <TableCell>{warningsCount}</TableCell>
                          <TableCell>{findingsCount}</TableCell>
                        </TableRow>
                      );
                    })}
                  </TableBody>
                </Table>
              </TableContainer>
              <Stack spacing={0.75} sx={{ mt: 1 }}>
                {importResult.imported.map((entry, idx) => {
                  const child = entry?.result;
                  if (!child) return null;
                  const sec = child.security;
                  const warnings = Array.isArray(sec?.warnings) ? sec?.warnings : [];
                  const findings = Array.isArray(sec?.findings) ? sec?.findings : [];
                  if (warnings.length === 0 && findings.length === 0) return null;
                  return (
                    <Box key={`skill-sec-${entry?.url || child?.name || idx}-${idx}`} sx={{ border: "1px solid rgba(108,156,212,0.18)", borderRadius: 1, p: 1 }}>
                      <Typography variant="caption" sx={{ display: "block", mb: 0.5 }}>
                        {child.name || "-"} details
                      </Typography>
                      {warnings.length > 0 ? (
                        <Typography variant="caption" color="text.secondary" sx={{ display: "block" }}>
                          Warnings: {warnings.slice(0, 3).join(" | ")}
                        </Typography>
                      ) : null}
                      {findings.length > 0 ? (
                        <Stack spacing={0.25} sx={{ mt: 0.5 }}>
                          {findings.slice(0, 3).map((rawFinding, fidx) => {
                            const f = asRecord(rawFinding);
                            return (
                              <Typography key={`finding-${fidx}-${str(f.category, "")}`} variant="caption" color="text.secondary" sx={{ display: "block" }}>
                                [{str(f.category, "-")}] line {num(f.line, -1) >= 0 ? num(f.line) : "-"}: {str(f.description, "-").slice(0, 180)}
                              </Typography>
                            );
                          })}
                        </Stack>
                      ) : null}
                    </Box>
                  );
                })}
              </Stack>
              {Array.isArray(importResult.failed) && importResult.failed.length > 0 ? (
                <Alert severity="warning" sx={{ mt: 1 }}>
                  Failed imports: {importResult.failed.length}
                </Alert>
              ) : null}
            </Box>
          ) : null}
          {(importResult?.secrets?.required_env || []).length > 0 ? (
            <Box sx={{ mt: 1 }}>
              <Typography variant="subtitle2" mb={1}>
                Required credentials
              </Typography>
              {!importCommitted ? (
                <Typography variant="caption" color="text.secondary" sx={{ display: "block", mb: 1 }}>
                  Credentials can be edited now, but are saved only after Import Template completes.
                </Typography>
              ) : null}
              <Stack spacing={1}>
                {(importResult?.secrets?.required_env || []).map((env) => {
                  const d = secretDrafts[env] || { storeAs: env, value: "", useBuiltin: false };
                  const missing = (importResult?.secrets?.missing_env || []).includes(env);
                  return (
                    <Box key={env} sx={{ border: "1px solid rgba(108,156,212,0.18)", borderRadius: 1, p: 1 }}>
                      <Stack direction="row" justifyContent="space-between" alignItems="center">
                        <Typography variant="body2" fontWeight={700}>
                          {env}
                        </Typography>
                        <Chip size="small" color={missing ? "warning" : "success"} label={missing ? "missing" : "configured"} />
                      </Stack>
                      <Stack direction={{ xs: "column", md: "row" }} spacing={1} mt={1}>
                        <TextField
                          fullWidth
                          size="small"
                          label="Store as"
                          value={d.storeAs}
                          disabled={d.useBuiltin}
                          onChange={(e) =>
                            setSecretDrafts((prev) => ({ ...prev, [env]: { ...d, storeAs: e.target.value } }))
                          }
                        />
                        <TextField
                          fullWidth
                          size="small"
                          type="password"
                          label="Value (optional)"
                          value={d.value}
                          disabled={d.useBuiltin}
                          onChange={(e) =>
                            setSecretDrafts((prev) => ({ ...prev, [env]: { ...d, value: e.target.value } }))
                          }
                        />
                      </Stack>
                      <FormControlLabel
                        control={
                          <Switch
                            checked={d.useBuiltin}
                            onChange={(e) =>
                              setSecretDrafts((prev) => ({ ...prev, [env]: { ...d, useBuiltin: e.target.checked } }))
                            }
                          />
                        }
                        label="Use builtin provider key"
                      />
                    </Box>
                  );
                })}
                <Button
                  variant="outlined"
                  disabled={savingSecrets || secretsSaved || !importCommitted}
                  onClick={handleSaveSecrets}
                >
                  {savingSecrets ? "Saving..." : "Save secrets"}
                </Button>
              </Stack>
            </Box>
          ) : null}
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={handleClose} disabled={loading}>
          Close
        </Button>
        <Button
          variant="contained"
          disabled={loading || !url.trim() || importRequiresForce}
          onClick={previewReady ? handleImport : handleAnalyze}
        >
          {loading
            ? previewReady
              ? "Importing..."
              : "Analyzing..."
            : previewReady
              ? "Import Template"
              : "Analyze Template"}
        </Button>
      </DialogActions>
    </Dialog>
  );
}

function SkillSecretsDialog({
  open,
  skillName,
  onClose
}: {
  open: boolean;
  skillName: string | null;
  onClose: () => void;
}) {
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [secrets, setSecrets] = useState<{ required_env: string[]; missing_env: string[]; bindings: Record<string, string> } | null>(null);
  const [drafts, setDrafts] = useState<Record<string, { storeAs: string; value: string; useBuiltin: boolean }>>({});

  useEffect(() => {
    if (!open || !skillName) return;
    setLoading(true);
    setError(null);
    setInfo(null);
    setSecrets(null);
    setDrafts({});
    api
      .getSkillSecrets(skillName)
      .then((out) => {
        setSecrets(out);
        const next: Record<string, { storeAs: string; value: string; useBuiltin: boolean }> = {};
        for (const env of out.required_env || []) {
          const binding = (out.bindings || {})[env];
          next[env] = {
            storeAs: binding && binding !== "builtin" ? binding : env,
            value: "",
            useBuiltin: binding === "builtin"
          };
        }
        setDrafts(next);
      })
      .catch((err) => setError(errMessage(err)))
      .finally(() => setLoading(false));
  }, [open, skillName]);

  const save = async () => {
    if (!skillName || !secrets) return;
    setSaving(true);
    setError(null);
    setInfo(null);
    try {
      const payload = (secrets.required_env || []).map((env) => {
        const d = drafts[env] || { storeAs: env, value: "", useBuiltin: false };
        if (d.useBuiltin) return { env, store_as: "builtin" };
        const storeAs = (d.storeAs || env).trim();
        const value = (d.value || "").trim();
        return value ? { env, store_as: storeAs, value } : { env, store_as: storeAs };
      });
      const out = await api.setSkillSecrets(skillName, { secrets: payload });
      setSecrets(out);
      if ((out.missing_env || []).length > 0) {
        setError(`Some keys are still missing: ${out.missing_env.join(", ")}`);
      } else {
        setInfo("Secrets saved. The skill remains disabled until you manually enable it in Skills.");
      }
    } catch (err) {
      setError(errMessage(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onClose={onClose} maxWidth="md" fullWidth>
      <DialogTitle>Secrets: {skillName || ""}</DialogTitle>
      <DialogContent dividers>
        <Typography variant="caption" color="text.secondary" sx={{ display: "block", mb: 1 }}>
          Secrets are private API keys or tokens used by this skill at runtime.
        </Typography>
        {loading ? <Typography variant="body2" color="text.secondary">Loading...</Typography> : null}
        {error ? <Alert severity="error">{error}</Alert> : null}
        {info ? <Alert severity="info">{info}</Alert> : null}
        {!loading && secrets ? (
          <Stack spacing={1.25}>
            {(secrets.required_env || []).length === 0 ? (
              <Typography variant="body2" color="text.secondary">
                No required credentials detected for this skill.
              </Typography>
            ) : (
              (secrets.required_env || []).map((env) => {
                const d = drafts[env] || { storeAs: env, value: "", useBuiltin: false };
                const missing = (secrets.missing_env || []).includes(env);
                return (
                  <Box key={env} sx={{ border: "1px solid rgba(108,156,212,0.18)", borderRadius: 1, p: 1 }}>
                    <Stack direction="row" justifyContent="space-between" alignItems="center">
                      <Typography variant="body2" fontWeight={700}>
                        {env}
                      </Typography>
                      <Chip size="small" color={missing ? "warning" : "success"} label={missing ? "missing" : "configured"} />
                    </Stack>
                    <Stack direction={{ xs: "column", md: "row" }} spacing={1} mt={1}>
                      <TextField
                        fullWidth
                        size="small"
                        label="Store as"
                        value={d.storeAs}
                        disabled={d.useBuiltin}
                        onChange={(e) => setDrafts((prev) => ({ ...prev, [env]: { ...d, storeAs: e.target.value } }))}
                      />
                      <TextField
                        fullWidth
                        size="small"
                        type="password"
                        label="Value (optional)"
                        value={d.value}
                        disabled={d.useBuiltin}
                        onChange={(e) => setDrafts((prev) => ({ ...prev, [env]: { ...d, value: e.target.value } }))}
                      />
                    </Stack>
                    <FormControlLabel
                      control={
                        <Switch
                          checked={d.useBuiltin}
                          onChange={(e) => setDrafts((prev) => ({ ...prev, [env]: { ...d, useBuiltin: e.target.checked } }))}
                        />
                      }
                      label="Use builtin provider key"
                    />
                  </Box>
                );
              })
            )}
          </Stack>
        ) : null}
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>Close</Button>
        <Button variant="contained" onClick={save} disabled={saving || loading || !secrets || (secrets.required_env || []).length === 0}>
          {saving ? "Saving..." : "Save"}
        </Button>
      </DialogActions>
    </Dialog>
  );
}

function QueryTable({
  title,
  path,
  arrayKey,
  columns,
  autoRefresh,
  emptyLabel,
  queryKey
}: {
  title: string;
  path: string;
  arrayKey: string;
  columns: string[];
  autoRefresh: boolean;
  emptyLabel: string;
  queryKey: string;
}) {
  const q = useQuery({
    queryKey: [queryKey],
    queryFn: () => api.rawGet(path),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });

  const rows = pickRecords(q.data, arrayKey);

  return (
    <Box className="list-shell">
      <Typography variant="h6" mb={1}>
        {title}
      </Typography>
      {q.error ? (
        <Alert severity="error">{errMessage(q.error)}</Alert>
      ) : rows.length === 0 ? (
        <Typography variant="body2" color="text.secondary">
          {emptyLabel}
        </Typography>
      ) : (
        <DataTable rows={rows} columns={columns} />
      )}
    </Box>
  );
}

function RowOpsMenu({ actions, ariaLabel = "Row actions" }: { actions: RowMenuAction[]; ariaLabel?: string }) {
  const [anchorEl, setAnchorEl] = useState<HTMLElement | null>(null);
  const open = Boolean(anchorEl);
  const closeMenu = () => setAnchorEl(null);
  return (
    <>
      <IconButton size="small" aria-label={ariaLabel} onClick={(e) => setAnchorEl(e.currentTarget)}>
        <MoreVertIcon fontSize="small" />
      </IconButton>
      <Menu anchorEl={anchorEl} open={open} onClose={closeMenu}>
        {actions.map((action, idx) => (
          <MenuItem
            key={`${action.label}-${idx}`}
            divider={action.divider}
            disabled={action.disabled}
            onClick={() => {
              closeMenu();
              if (action.disabled) return;
              void action.onClick();
            }}
            sx={
              action.tone === "error"
                ? { color: "error.main" }
                : action.tone === "warning"
                  ? { color: "warning.main" }
                  : undefined
            }
          >
            {action.label}
          </MenuItem>
        ))}
      </Menu>
    </>
  );
}

function ChatManager({ autoRefresh }: { autoRefresh: boolean }) {
  const queryClient = useQueryClient();
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [draftProjectId, setDraftProjectId] = useState("");
  const [prompt, setPrompt] = useState("");
  const [attachedFiles, setAttachedFiles] = useState<File[]>([]);
  const [chatError, setChatError] = useState<string | null>(null);
  const [chatNotice, setChatNotice] = useState<string | null>(null);
  const [isStreaming, setIsStreaming] = useState(false);
  const [pendingUserMessage, setPendingUserMessage] = useState<string | null>(null);
  const [streamingResponse, setStreamingResponse] = useState("");
  const [streamingSteps, setStreamingSteps] = useState<JsonRecord[]>([]);
  const [streamTraceOpen, setStreamTraceOpen] = useState(false);
  const [isDragOverChat, setIsDragOverChat] = useState(false);
  const [messageTraceOpen, setMessageTraceOpen] = useState<Record<string, boolean>>({});
  const [traceStepsById, setTraceStepsById] = useState<Record<string, JsonRecord[]>>({});
  const [traceLoadingById, setTraceLoadingById] = useState<Record<string, boolean>>({});
  const [traceErrorById, setTraceErrorById] = useState<Record<string, string>>({});
  const [conversationMenuAnchor, setConversationMenuAnchor] = useState<HTMLElement | null>(null);
  const [conversationMenuTarget, setConversationMenuTarget] = useState<JsonRecord | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const dragDepthRef = useRef(0);
  const threadRef = useRef<HTMLDivElement | null>(null);
  const streamLockRef = useRef(false);
  const recentSendRef = useRef<{ fingerprint: string; at: number } | null>(null);

  const convQ = useQuery({
    queryKey: ["chat-conversations"],
    queryFn: () => api.rawGet("/conversations?limit=30"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const projectsQ = useQuery({
    queryKey: ["chat-projects"],
    queryFn: () => api.rawGet("/projects"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });

  const messagesQ = useQuery({
    queryKey: ["chat-messages", conversationId],
    queryFn: () => api.rawGet(`/conversations/${encodeURIComponent(conversationId || "")}/messages?limit=100`),
    enabled: !!conversationId,
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });

  const conversations = pickRecords(convQ.data, "conversations");
  const projects = pickRecords(projectsQ.data, "projects");
  const messages = conversationId ? pickRecords(messagesQ.data, "messages") : [];
  const selectedConversation = useMemo(
    () => conversations.find((conv) => str(conv.id, "") === conversationId) ?? null,
    [conversations, conversationId]
  );
  const projectNameById = useMemo(() => {
    const map = new Map<string, string>();
    for (const project of projects) {
      const id = str(project.id, "").trim();
      if (!id) continue;
      map.set(id, str(project.name, id));
    }
    return map;
  }, [projects]);
  const selectedConversationProjectId = str(selectedConversation?.project_id, "").trim();
  const activeProjectId = selectedConversationProjectId || draftProjectId;

  const buildStepCard = (step: JsonRecord, index: number) => {
    const stepType = str(step.step_type, str(step.type, "step")).toLowerCase();
    const title = str(step.title, "").trim();
    const detail = (str(step.detail, "").trim() || str(step.data, "").trim()).slice(0, 320);
    const time = str(step.time, "");
    const baseLabel = stepType.replace(/[_-]+/g, " ").trim() || "step";
    const label = title || baseLabel.replace(/\b\w/g, (ch) => ch.toUpperCase());
    let tone = "tone-neutral";
    let kind = "STEP";
    if (stepType.includes("tool_start")) {
      tone = "tone-tool";
      kind = "TOOL START";
    } else if (stepType.includes("tool_result") || stepType.includes("result") || stepType.includes("complete")) {
      tone = "tone-success";
      kind = "TOOL RESULT";
    } else if (stepType.includes("error") || stepType.includes("fail")) {
      tone = "tone-error";
      kind = "ERROR";
    } else if (stepType.includes("think") || stepType.includes("plan") || stepType.includes("reason")) {
      tone = "tone-thinking";
      kind = "THINK";
    } else if (stepType.includes("action") || stepType.includes("execute")) {
      tone = "tone-action";
      kind = "ACTION";
    } else if (stepType.includes("response") || stepType.includes("final") || stepType.includes("summary")) {
      tone = "tone-synthesis";
      kind = "SYNTH";
    }
    return {
      id: `${time || "live"}-${index}-${label}`,
      index,
      tone,
      kind,
      label,
      detail,
      time
    };
  };

  const streamingTraceCards = useMemo(
    () => streamingSteps.map((step, idx) => buildStepCard(step, idx)).slice(-24),
    [streamingSteps]
  );

  const traceSummaryText = (
    cards: Array<ReturnType<typeof buildStepCard>>,
    opts?: { loading?: boolean; streaming?: boolean; error?: string }
  ) => {
    if (opts?.error) return "Trace unavailable";
    if (opts?.loading) return "Trace loading...";
    if (cards.length === 0) return opts?.streaming ? "Trace | waiting for first step..." : "Trace";
    const last = cards[cards.length - 1];
    return `Trace | ${cards.length} step${cards.length === 1 ? "" : "s"} | ${last.kind}: ${last.label}`;
  };

  const parseTraceSteps = (payload: unknown): JsonRecord[] => {
    const rec = asRecord(payload);
    const raw = Array.isArray(rec.steps) ? rec.steps : Array.isArray(rec.trace) ? rec.trace : [];
    return raw.filter((x) => x && typeof x === "object") as JsonRecord[];
  };

  const loadTraceForId = async (traceId: string) => {
    if (!traceId) return;
    if (traceStepsById[traceId] || traceLoadingById[traceId]) return;
    setTraceLoadingById((prev) => ({ ...prev, [traceId]: true }));
    setTraceErrorById((prev) => ({ ...prev, [traceId]: "" }));
    try {
      const payload = await api.rawGet(`/trace/${encodeURIComponent(traceId)}`);
      const steps = parseTraceSteps(payload);
      setTraceStepsById((prev) => ({ ...prev, [traceId]: steps }));
    } catch (err) {
      const raw = errMessage(err);
      let normalized = raw;
      try {
        const parsed = JSON.parse(raw) as { error?: string; message?: string };
        normalized = parsed.error || parsed.message || raw;
      } catch {
        // keep raw
      }
      if (/trace/i.test(normalized) && /not found/i.test(normalized)) {
        normalized = "Trace is not available for this response.";
      }
      setTraceErrorById((prev) => ({ ...prev, [traceId]: normalized }));
    } finally {
      setTraceLoadingById((prev) => ({ ...prev, [traceId]: false }));
    }
  };

  const startNewConversation = () => {
    dragDepthRef.current = 0;
    setIsDragOverChat(false);
    setConversationId(null);
    setDraftProjectId("");
    setPrompt("");
    setAttachedFiles([]);
    setChatError(null);
    setChatNotice(null);
    setPendingUserMessage(null);
    setStreamingResponse("");
    setStreamingSteps([]);
    setStreamTraceOpen(false);
    setMessageTraceOpen({});
  };

  const queueAttachedFiles = (files: FileList | null) => {
    if (!files || files.length === 0) return;
    const incoming = Array.from(files);
    const { accepted, rejected } = splitSupportedChatAttachments(incoming);
    if (rejected.length > 0) {
      const preview = rejected.slice(0, 3).join(", ");
      const extra = rejected.length > 3 ? ` (+${rejected.length - 3} more)` : "";
      setChatNotice(`Skipped unsupported files: ${preview}${extra}`);
    }
    if (accepted.length === 0) return;
    setAttachedFiles((prev) => {
      const merged = [...prev];
      for (const file of accepted) {
        const exists = merged.some(
          (f) =>
            f.name === file.name &&
            f.size === file.size &&
            f.lastModified === file.lastModified
        );
        if (!exists) merged.push(file);
      }
      return merged.slice(0, 8);
    });
  };

  const handleChatDragEnter = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.stopPropagation();
    dragDepthRef.current += 1;
    if (!isStreaming) setIsDragOverChat(true);
  };

  const handleChatDragOver = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.stopPropagation();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "copy";
    if (!isStreaming && !isDragOverChat) setIsDragOverChat(true);
  };

  const handleChatDragLeave = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.stopPropagation();
    dragDepthRef.current = Math.max(0, dragDepthRef.current - 1);
    if (dragDepthRef.current === 0) setIsDragOverChat(false);
  };

  const handleChatDrop = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.stopPropagation();
    dragDepthRef.current = 0;
    setIsDragOverChat(false);
    if (isStreaming) return;
    queueAttachedFiles(event.dataTransfer?.files ?? null);
  };

  const removeAttachedFile = (idx: number) => {
    setAttachedFiles((prev) => prev.filter((_, i) => i !== idx));
  };

  const uploadAttachmentsForKnowledge = async (files: File[]) => {
    if (files.length === 0) return [] as Array<{ id: string; filename: string; chunks: number }>;
    const projectId = activeProjectId.trim();
    const uploaded: Array<{ id: string; filename: string; chunks: number }> = [];
    for (const file of files) {
      const formData = new FormData();
      formData.append("file", file, file.name);
      if (projectId) formData.append("project_id", projectId);
      const out = asRecord(await api.rawPostForm("/documents/upload-file", formData));
      const id = str(out.id, "");
      if (!id) {
        throw new Error(`Failed to index '${file.name}'.`);
      }
      uploaded.push({
        id,
        filename: str(out.filename, file.name),
        chunks: num(out.chunks, 0)
      });
    }
    return uploaded;
  };

  const copyText = async (value: string) => {
    const text = value.trim();
    if (!text) throw new Error("Nothing to copy.");
    const nav = typeof navigator !== "undefined" ? navigator : null;
    if (nav && nav.clipboard?.writeText) {
      await nav.clipboard.writeText(text);
      return;
    }
    const doc = typeof document !== "undefined" ? document : null;
    if (!doc) throw new Error("Clipboard is not available.");
    const ta = doc.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.left = "-9999px";
    doc.body.appendChild(ta);
    ta.focus();
    ta.select();
    const ok = doc.execCommand("copy");
    doc.body.removeChild(ta);
    if (!ok) throw new Error("Copy failed.");
  };

  const exportConversationById = async (targetId: string, titleHint?: string) => {
    if (!targetId) return;
    setChatError(null);
    try {
      let exportMessages = messages;
      if (conversationId !== targetId || exportMessages.length === 0) {
        const payload = await api.rawGet(`/conversations/${encodeURIComponent(targetId)}/messages?limit=200`);
        exportMessages = pickRecords(payload, "messages");
      }
      const title = (titleHint || str(selectedConversation?.title, "chat")).trim() || "chat";
      const safe = title.replace(/[^\w.-]+/g, "_").replace(/^_+|_+$/g, "").toLowerCase() || "chat";
      const stamp = new Date().toISOString().replace(/[:.]/g, "-");
      const lines: string[] = [];
      lines.push(`# ${title}`);
      lines.push(`conversation_id: ${targetId}`);
      lines.push(`exported_at: ${new Date().toISOString()}`);
      lines.push("");
      for (const message of exportMessages) {
        const role = str(message.role, "assistant");
        const ts = str(message.timestamp, "");
        const content = str(message.content, "");
        lines.push(`${role.toUpperCase()}${ts ? ` (${ts})` : ""}`);
        lines.push(content);
        lines.push("");
      }
      const blob = new Blob([lines.join("\n")], { type: "text/plain;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${safe}-${stamp}.txt`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      setChatNotice("Chat exported.");
    } catch (err) {
      setChatError(errMessage(err));
    }
  };

  const copyMessage = async (message: JsonRecord) => {
    try {
      const role = str(message.role, "").toLowerCase();
      const content = str(message.content, "");
      await copyText(role === "user" ? stripAttachmentContextMarker(content) : content);
      setChatNotice("Message copied.");
    } catch (err) {
      setChatError(errMessage(err));
    }
  };

  const deleteConversationMutation = useMutation({
    mutationFn: (id: string) => api.rawDelete(`/conversations/${encodeURIComponent(id)}`),
    onSuccess: async (_data, id) => {
      if (conversationId === id) startNewConversation();
      await queryClient.invalidateQueries({ queryKey: ["chat-conversations"] });
      await queryClient.invalidateQueries({ queryKey: ["chat-messages", id] });
      setChatNotice("Chat deleted.");
    },
    onError: (err) => {
      setChatError(errMessage(err));
    }
  });

  const deleteConversation = async (id: string) => {
    if (!id || isStreaming || deleteConversationMutation.isPending) return;
    const shouldDelete = typeof window === "undefined" ? true : window.confirm("Delete this chat and all its messages?");
    if (!shouldDelete) return;
    setChatError(null);
    await deleteConversationMutation.mutateAsync(id);
  };

  const openConversationMenu = (event: MouseEvent<HTMLElement>, conv: JsonRecord) => {
    event.stopPropagation();
    setConversationMenuAnchor(event.currentTarget);
    setConversationMenuTarget(conv);
  };

  const closeConversationMenu = () => {
    setConversationMenuAnchor(null);
    setConversationMenuTarget(null);
  };

  const pushStreamingStep = (step: JsonRecord) => {
    setStreamingSteps((prev) => {
      const next = [...prev, step];
      if (next.length > 32) next.splice(0, next.length - 32);
      return next;
    });
  };

  const runStreamingChat = async (message: string, files: File[] = []) => {
    const activeMessage =
      message.trim() ||
      (files.length > 0
        ? "Please analyze the attached documents and answer using them."
        : "");
    if (!activeMessage || isStreaming || streamLockRef.current) return;
    const now = Date.now();
    const fingerprint = `${conversationId || "__new__"}::${activeProjectId || "__no_project__"}::${activeMessage
      .toLowerCase()
      .replace(/\s+/g, " ")
      .trim()}`;
    const lastSend = recentSendRef.current;
    if (lastSend && lastSend.fingerprint === fingerprint && now - lastSend.at < 1500) {
      setChatNotice("Duplicate send ignored.");
      return;
    }
    recentSendRef.current = { fingerprint, at: now };
    streamLockRef.current = true;

    setChatError(null);
    setPendingUserMessage(activeMessage);
    setStreamingResponse("");
    setStreamingSteps([]);
    setStreamTraceOpen(false);
    setIsStreaming(true);

    let resolvedConversationId = conversationId || generateConversationId();
    let payloadMessage = activeMessage;
    let streamError: string | null = null;
    const absorbConversationId = (payload: unknown) => {
      const obj = asRecord(payload);
      const cid = str(obj.conversation_id, str(obj.cid, str(obj.conversationId, "")));
      if (cid) resolvedConversationId = cid;
    };

    try {
      if (files.length > 0) {
        setChatNotice(`Indexing ${files.length} attachment${files.length === 1 ? "" : "s"}...`);
        const uploaded = await uploadAttachmentsForKnowledge(files);
        if (uploaded.length > 0) {
          const refs = uploaded.map((item) => `doc:${item.id}`).join(", ");
          const names = uploaded.map((item) => item.filename).join(", ");
          payloadMessage = `${activeMessage}\n\n[Attached documents indexed for retrieval: ${refs}; files: ${names}]`;
          setChatNotice(
            `Indexed ${uploaded.length} attachment${uploaded.length === 1 ? "" : "s"} for retrieval.`
          );
        }
      }
      await api.chatStream(
        {
          message: payloadMessage,
          channel: "web",
          conversation_id: resolvedConversationId,
          project_id: activeProjectId || undefined
        },
        {
          onEvent: (_eventName, payload) => {
            absorbConversationId(payload);
          },
          onToken: (token) => {
            setStreamingResponse((prev) => prev + token);
          },
          onThinking: (step) => {
            absorbConversationId(step);
            pushStreamingStep(step);
          },
          onToolStart: (name) => {
            pushStreamingStep({
              step_type: "tool_start",
              title: `Tool started: ${name}`,
              data: name
            });
          },
          onToolResult: (name, content) => {
            const preview = content.trim().slice(0, 180);
            pushStreamingStep({
              step_type: "tool_result",
              title: `Tool finished: ${name || "tool"}`,
              data: preview
            });
          },
          onContent: (payload) => {
            const text = str(payload.content, "");
            if (text) setStreamingResponse(text);
            absorbConversationId(payload);
          },
          onError: (messageText) => {
            streamError = messageText;
          }
        }
      );
    } catch (err) {
      streamError = errMessage(err);
    } finally {
      if (streamError) setChatError(streamError);
      if (!resolvedConversationId) {
        try {
          const latest = await api.rawGet("/conversations?limit=1");
          const newest = pickRecords(latest, "conversations")[0];
          const newestId = str(newest?.id, "");
          if (newestId) resolvedConversationId = newestId;
        } catch {
          // Ignore fallback lookup failures; chat can still be selected manually.
        }
      }
      if (resolvedConversationId) {
        setConversationId(resolvedConversationId);
        await queryClient.invalidateQueries({ queryKey: ["chat-messages", resolvedConversationId] });
      }
      await queryClient.invalidateQueries({ queryKey: ["chat-conversations"] });
      if (!streamError) setAttachedFiles([]);
      setPendingUserMessage(null);
      setIsStreaming(false);
      setStreamingSteps([]);
      setStreamingResponse("");
      streamLockRef.current = false;
    }
  };

  useEffect(() => {
    const thread = threadRef.current;
    if (!thread) return;
    thread.scrollTop = thread.scrollHeight;
  }, [messages.length, pendingUserMessage, streamingResponse, isStreaming]);

  useEffect(() => {
    if (!chatNotice) return;
    const timer = window.setTimeout(() => setChatNotice(null), 2200);
    return () => window.clearTimeout(timer);
  }, [chatNotice]);

  const hasLiveThreadActivity = Boolean(pendingUserMessage || isStreaming || streamingResponse.trim());
  const hasRenderableThread = messages.length > 0 || hasLiveThreadActivity;

  return (
    <Box
      sx={{
        height: "100%",
        minHeight: 0,
        display: "grid",
        gridTemplateColumns: { xs: "1fr", md: "320px 1fr" },
        gap: 1.5
      }}
    >
      <Box className="list-shell chat-sidebar" sx={{ minHeight: 0, display: "flex", flexDirection: "column" }}>
        <Stack direction="row" justifyContent="space-between" alignItems="center" mb={1}>
          <Typography variant="h6">Conversations</Typography>
          <Button size="small" onClick={startNewConversation} disabled={isStreaming}>
            New
          </Button>
        </Stack>

        <Box sx={{ flex: 1, minHeight: 0, overflow: "auto", pr: 0.5 }}>
          <Stack spacing={1}>
            {conversations.length === 0 ? (
              <Typography variant="body2" color="text.secondary">
                No conversations yet.
              </Typography>
            ) : (
              conversations.map((conv) => {
                const id = str(conv.id, "");
                const active = conversationId === id;
                const convProjectId = str(conv.project_id, "").trim();
                const convProjectName = convProjectId ? (projectNameById.get(convProjectId) || convProjectId) : "";
                return (
                  <Box
                    key={id}
                    className={active ? "conversation-card active" : "conversation-card"}
                    onClick={() => setConversationId(id)}
                    role="button"
                    tabIndex={0}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") setConversationId(id);
                    }}
                  >
                    <Stack direction="row" justifyContent="space-between" alignItems="center" spacing={0.5}>
                      <div className="conversation-card-title">{str(conv.title, "Untitled")}</div>
                      <Tooltip title="Chat options">
                        <span>
                          <IconButton
                            size="small"
                            onClick={(e) => {
                              openConversationMenu(e, conv);
                            }}
                            disabled={deleteConversationMutation.isPending}
                            sx={{ color: "rgba(188, 211, 242, 0.85)" }}
                          >
                            <MoreVertIcon fontSize="small" />
                          </IconButton>
                        </span>
                      </Tooltip>
                    </Stack>
                    <div className="conversation-card-meta">
                      {str(conv.channel)} | {str(conv.updated_at)}
                    </div>
                    {convProjectName ? (
                      <Typography variant="caption" color="text.secondary" sx={{ display: "block", mt: 0.35 }}>
                        Project: {convProjectName}
                      </Typography>
                    ) : null}
                  </Box>
                );
              })
            )}
          </Stack>
        </Box>
        <Menu anchorEl={conversationMenuAnchor} open={Boolean(conversationMenuAnchor)} onClose={closeConversationMenu}>
          <MenuItem
            onClick={() => {
              const id = str(conversationMenuTarget?.id, "");
              const title = str(conversationMenuTarget?.title, "chat");
              closeConversationMenu();
              if (id) void exportConversationById(id, title);
            }}
          >
            Export chat
          </MenuItem>
          <MenuItem
            disabled={isStreaming || deleteConversationMutation.isPending}
            onClick={() => {
              const id = str(conversationMenuTarget?.id, "");
              closeConversationMenu();
              if (id) void deleteConversation(id);
            }}
          >
            Delete chat
          </MenuItem>
        </Menu>
      </Box>

      <Box
        className={`list-shell chat-shell chat-density-immersive${isDragOverChat ? " chat-shell-drop-active" : ""}`}
        sx={{ minHeight: 0, display: "flex", flexDirection: "column", position: "relative" }}
        onDragEnter={handleChatDragEnter}
        onDragOver={handleChatDragOver}
        onDragLeave={handleChatDragLeave}
        onDrop={handleChatDrop}
      >
        <Stack direction="row" justifyContent="space-between" alignItems="center" mb={1}>
          <Stack direction="row" spacing={1} alignItems="center">
            <Avatar src={AgentLogo} variant="rounded" sx={{ width: 28, height: 28, bgcolor: "rgba(12,22,40,0.85)" }} />
            <Typography variant="h6">Chat</Typography>
          </Stack>
          {conversationId ? (
            <Typography variant="caption" color="text.secondary" sx={{ fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace" }}>
              ID: {conversationId}
            </Typography>
          ) : (
            <Typography variant="caption" color="text.secondary">
              Draft chat
            </Typography>
          )}
        </Stack>
        <Stack direction={{ xs: "column", md: "row" }} spacing={1} sx={{ mb: 1 }}>
          <TextField
            fullWidth
            size="small"
            select
            label="Project"
            value={selectedConversationProjectId || draftProjectId}
            onChange={(e) => setDraftProjectId(e.target.value)}
            disabled={Boolean(selectedConversation)}
            helperText={
              selectedConversation
                ? "Project is fixed for this conversation."
                : "Optional. Leave as No project for general chat."
            }
          >
            <MenuItem value="">No project</MenuItem>
            {projects.map((project) => {
              const id = str(project.id, "");
              if (!id) return null;
              return (
                <MenuItem key={id} value={id}>
                  {str(project.name, id)}
                </MenuItem>
              );
            })}
          </TextField>
        </Stack>

        <Box
          ref={threadRef}
          sx={{ flex: 1, minHeight: 0, overflow: "auto" }}
          className="chat-thread chat-thread-immersive"
        >
          {conversationId == null && !hasRenderableThread ? (
            <Typography variant="body2" color="text.secondary">
              Start with a message to open a new conversation.
            </Typography>
          ) : !hasRenderableThread ? (
            <Typography variant="body2" color="text.secondary">
              No messages in this conversation yet.
            </Typography>
          ) : (
            <Stack spacing={1.2}>
              {messages.map((message, idx) => {
                const role = str(message.role, "").toLowerCase();
                const isUser = role === "user";
                const messageId = str(message.id, String(idx));
                const ts = str(message.timestamp, "");
                const content = str(message.content);
                const renderedContent = isUser ? stripAttachmentContextMarker(content) : content;
                const traceId = str(message.trace_id, "").trim();
                const hasTrace = !isUser && !!traceId;
                const traceLoading = hasTrace ? Boolean(traceLoadingById[traceId]) : false;
                const traceError = hasTrace ? str(traceErrorById[traceId], "").trim() : "";
                const rawTraceSteps = hasTrace ? traceStepsById[traceId] || [] : [];
                const traceCards = rawTraceSteps.map((step, sIdx) => buildStepCard(step, sIdx)).slice(-24);
                const traceExpanded = Boolean(messageTraceOpen[messageId]);
                const traceSummary = traceSummaryText(traceCards, { loading: traceLoading, error: traceError });
                return (
                  <Box key={messageId} className={isUser ? "chat-row chat-row-user" : "chat-row"}>
                    {!isUser ? (
                      <Avatar
                        src={AgentLogo}
                        variant="rounded"
                        className="chat-avatar"
                        sx={{ width: 30, height: 30, bgcolor: "rgba(12,22,40,0.85)" }}
                      />
                    ) : null}
                    <Box className={isUser ? "chat-bubble chat-bubble-user" : "chat-bubble chat-bubble-assistant"}>
                      <Stack direction="row" justifyContent="space-between" alignItems="center" spacing={0.5}>
                        <Typography variant="caption" color="text.secondary">
                          {isUser ? "You" : "AgentArk"}{ts ? ` | ${ts}` : ""}
                        </Typography>
                        <Tooltip title="Copy message">
                          <IconButton
                            size="small"
                            onClick={() => {
                              void copyMessage(message);
                            }}
                            sx={{ color: "rgba(189, 216, 249, 0.9)" }}
                          >
                            <ContentCopyRoundedIcon fontSize="small" />
                          </IconButton>
                        </Tooltip>
                      </Stack>
                      {hasTrace ? (
                        <Box className="chat-inline-trace">
                          <Button
                            size="small"
                            className="chat-inline-trace-toggle"
                            onClick={() => {
                              const nextExpanded = !Boolean(messageTraceOpen[messageId]);
                              setMessageTraceOpen((prev) => ({ ...prev, [messageId]: nextExpanded }));
                              if (nextExpanded && traceId) {
                                void loadTraceForId(traceId);
                              }
                            }}
                            endIcon={
                              <ArrowDropDownRoundedIcon
                                sx={{
                                  transform: traceExpanded ? "rotate(180deg)" : "rotate(0deg)",
                                  transition: "transform 160ms ease"
                                }}
                              />
                            }
                          >
                            <span className="chat-inline-trace-summary">{traceSummary}</span>
                          </Button>
                          {traceExpanded ? (
                            <Box className="chat-inline-trace-expanded">
                              {traceError ? (
                                <Typography variant="caption" color="error.main">
                                  {traceError}
                                </Typography>
                              ) : traceCards.length === 0 ? (
                                <Typography variant="caption" color="text.secondary">
                                  {traceLoading ? "Loading trace..." : "No trace steps captured for this message."}
                                </Typography>
                              ) : (
                                <Stack direction="row" spacing={0.75} useFlexGap flexWrap="wrap">
                                  {traceCards.map((step) => (
                                    <Box key={step.id} className={`chat-inline-step ${step.tone}`} title={step.detail || step.label}>
                                      <span className="chat-inline-step-kind">{step.kind}</span>
                                      <span className="chat-inline-step-label">{step.label}</span>
                                    </Box>
                                  ))}
                                </Stack>
                              )}
                            </Box>
                          ) : null}
                        </Box>
                      ) : null}
                    <Typography variant="body2" sx={{ whiteSpace: "pre-wrap" }}>
                      {renderedContent}
                    </Typography>
                    </Box>
                    {isUser ? (
                      <Avatar className="chat-avatar chat-avatar-user" sx={{ width: 30, height: 30, bgcolor: "rgba(47,212,255,0.18)" }}>
                        U
                      </Avatar>
                    ) : null}
                  </Box>
                );
              })}

              {pendingUserMessage ? (
                <Box className="chat-row chat-row-user">
                  <Box className="chat-bubble chat-bubble-user">
                    <Typography variant="caption" color="text.secondary">
                      You | sending...
                    </Typography>
                    <Typography variant="body2" sx={{ whiteSpace: "pre-wrap" }}>
                      {pendingUserMessage}
                    </Typography>
                  </Box>
                  <Avatar className="chat-avatar chat-avatar-user" sx={{ width: 30, height: 30, bgcolor: "rgba(47,212,255,0.18)" }}>
                    U
                  </Avatar>
                </Box>
              ) : null}

              {isStreaming ? (
                <Box className="chat-row">
                  <Avatar
                    src={AgentLogo}
                    variant="rounded"
                    className="chat-avatar"
                    sx={{ width: 30, height: 30, bgcolor: "rgba(12,22,40,0.85)" }}
                  />
                  <Box className="chat-bubble chat-bubble-assistant chat-bubble-streaming">
                    <Box className="chat-inline-trace">
                      <Button
                        size="small"
                        className="chat-inline-trace-toggle"
                        onClick={() => setStreamTraceOpen((prev) => !prev)}
                        endIcon={
                          <ArrowDropDownRoundedIcon
                            sx={{
                              transform: streamTraceOpen ? "rotate(180deg)" : "rotate(0deg)",
                              transition: "transform 160ms ease"
                            }}
                          />
                        }
                      >
                        <span className="chat-inline-trace-summary">
                          {traceSummaryText(streamingTraceCards, { streaming: true })}
                        </span>
                      </Button>
                      {streamTraceOpen ? (
                        <Box className="chat-inline-trace-expanded">
                          {streamingTraceCards.length === 0 ? (
                            <Typography variant="caption" color="text.secondary">
                              Waiting for first execution step...
                            </Typography>
                          ) : (
                            <Stack direction="row" spacing={0.75} useFlexGap flexWrap="wrap">
                              {streamingTraceCards.map((step) => (
                                <Box key={step.id} className={`chat-inline-step ${step.tone}`} title={step.detail || step.label}>
                                  <span className="chat-inline-step-kind">{step.kind}</span>
                                  <span className="chat-inline-step-label">{step.label}</span>
                                </Box>
                              ))}
                            </Stack>
                          )}
                        </Box>
                      ) : null}
                    </Box>
                    <Typography variant="caption" color="text.secondary">
                      {streamingResponse.trim() ? "AgentArk is streaming..." : "AgentArk is thinking..."}
                    </Typography>
                    {streamingResponse.trim() ? (
                      <Typography variant="body2" sx={{ whiteSpace: "pre-wrap" }}>
                        {streamingResponse}
                        <span className="stream-caret" />
                      </Typography>
                    ) : (
                      <div className="typing-dots" aria-label="typing">
                        <span />
                        <span />
                        <span />
                      </div>
                    )}
                  </Box>
                </Box>
              ) : null}
            </Stack>
          )}
        </Box>

        {convQ.error || messagesQ.error || chatError ? (
          <Alert severity="error" sx={{ mt: 1 }}>
            {chatError || errMessage(convQ.error || messagesQ.error)}
          </Alert>
        ) : null}
        {chatNotice && !(convQ.error || messagesQ.error || chatError) ? (
          <Alert severity="info" sx={{ mt: 1 }}>
            {chatNotice}
          </Alert>
        ) : null}
        {isDragOverChat ? (
          <Box className="chat-drop-overlay">
            <Typography variant="subtitle2">Drop files to attach</Typography>
            <Typography variant="caption" color="text.secondary">
              Supported: TXT, MD, JSON, CSV, XML, YAML, PDF, DOCX, LOG, HTML
            </Typography>
          </Box>
        ) : null}

        <input
          ref={fileInputRef}
          type="file"
          multiple
          accept=".txt,.md,.markdown,.json,.csv,.tsv,.xml,.yaml,.yml,.pdf,.docx,.log,.html,.htm"
          style={{ display: "none" }}
          onChange={(e) => {
            queueAttachedFiles(e.target.files);
            e.currentTarget.value = "";
          }}
        />
        <Stack direction="row" spacing={1} alignItems="center" sx={{ mt: 1, mb: 0.5 }} useFlexGap flexWrap="wrap">
          <Button
            size="small"
            variant="outlined"
            onClick={() => fileInputRef.current?.click()}
            disabled={isStreaming}
          >
            Attach Docs
          </Button>
          {attachedFiles.length > 0 ? (
            <Typography variant="caption" color="text.secondary">
              {attachedFiles.length} file{attachedFiles.length === 1 ? "" : "s"} ready. They will be indexed before send.
            </Typography>
          ) : null}
        </Stack>
        {attachedFiles.length > 0 ? (
          <Stack direction="row" spacing={0.75} useFlexGap flexWrap="wrap" sx={{ mb: 0.5 }}>
            {attachedFiles.map((file, idx) => (
              <Chip
                key={`${file.name}-${file.size}-${file.lastModified}-${idx}`}
                size="small"
                label={file.name}
                onDelete={isStreaming ? undefined : () => removeAttachedFile(idx)}
              />
            ))}
          </Stack>
        ) : null}

        <Stack direction={{ xs: "column", md: "row" }} spacing={1} sx={{ mt: 1 }}>
          <TextField
            fullWidth
            multiline
            minRows={2}
            maxRows={6}
            label="Message"
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            onKeyDown={(e) => {
              if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
                e.preventDefault();
                const btn = document.getElementById("chat-send-btn") as HTMLButtonElement | null;
                btn?.click();
              }
            }}
          />
          <Button
            id="chat-send-btn"
            variant="contained"
            disabled={isStreaming || (!prompt.trim() && attachedFiles.length === 0)}
            onClick={async () => {
              setChatError(null);
              const msg = prompt.trim();
              setPrompt("");
              await runStreamingChat(msg, attachedFiles);
            }}
            sx={{ minWidth: 120 }}
          >
            {isStreaming ? "Streaming..." : "Send"}
          </Button>
        </Stack>
      </Box>
    </Box>
  );
}
function TasksManager({ autoRefresh }: { autoRefresh: boolean }) {
  const queryClient = useQueryClient();
  const [quickIntent, setQuickIntent] = useState("");
  const [schedulePreset, setSchedulePreset] = useState("once");
  const [customCron, setCustomCron] = useState("");
  const [requireApproval, setRequireApproval] = useState(false);
  const [manualOpen, setManualOpen] = useState(false);
  const [description, setDescription] = useState("");
  const [action, setAction] = useState("daily_brief");
  const [argumentsJson, setArgumentsJson] = useState("{}");
  const [cron, setCron] = useState("");
  const [approval, setApproval] = useState("auto");
  const [formError, setFormError] = useState<string | null>(null);
  const [selectedTask, setSelectedTask] = useState<JsonRecord | null>(null);

  function statusLabel(raw: string): string {
    const s = (raw || "").toLowerCase();
    if (s.includes("awaitingapproval")) return "Needs approval";
    if (s.includes("inprogress")) return "Running";
    if (s.includes("pending")) return "Queued";
    if (s.includes("completed")) return "Done";
    if (s.includes("failed")) return "Failed";
    if (s.includes("cancelled")) return "Cancelled";
    return raw || "-";
  }

  function statusColor(raw: string): "success" | "warning" | "error" | "default" | "info" {
    const s = (raw || "").toLowerCase();
    if (s.includes("failed")) return "error";
    if (s.includes("awaitingapproval")) return "warning";
    if (s.includes("inprogress")) return "info";
    if (s.includes("pending")) return "default";
    if (s.includes("completed")) return "success";
    return "default";
  }

  const tasksQ = useQuery({
    queryKey: ["tasks-manager"],
    queryFn: () => api.rawGet("/tasks?limit=100"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });

  const opMutation = useMutation({
    mutationFn: ({
      path,
      method,
      payload
    }: {
      path: string;
      method: "POST" | "DELETE";
      payload?: unknown;
    }) => (method === "DELETE" ? api.rawDelete(path) : api.rawPost(path, payload ?? {})),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["tasks-manager"] });
    }
  });

  const aiCreateMutation = useMutation({
    mutationFn: async () => {
      const intent = quickIntent.trim();
      if (!intent) throw new Error("Describe what you want to automate.");

      const planRaw = await api.rawPost("/tasks/plan", { description: intent });
      const plan = asRecord(asRecord(planRaw).plan);
      const rawSteps = Array.isArray(plan.steps) ? plan.steps : [];
      const steps = rawSteps
        .map(asRecord)
        .map((step) => ({
          action: str(step.action, "").trim(),
          arguments: asRecord(step.arguments)
        }))
        .filter((step) => !!step.action);

      if (steps.length === 0) {
        throw new Error("AI planner returned no runnable steps. Try a more specific request.");
      }

      let cronValue: string | null = null;
      if (schedulePreset === "every_15") cronValue = "*/15 * * * *";
      else if (schedulePreset === "hourly") cronValue = "0 * * * *";
      else if (schedulePreset === "daily_9") cronValue = "0 9 * * *";
      else if (schedulePreset === "weekday_9") cronValue = "0 9 * * 1-5";
      else if (schedulePreset === "custom") cronValue = customCron.trim() || null;

      const summary = str(plan.summary, "").trim();
      await opMutation.mutateAsync({
        path: "/tasks",
        method: "POST",
        payload: {
          description: summary || intent,
          action: "plan",
          arguments: { steps },
          cron: cronValue,
          approval: requireApproval ? "require" : "auto"
        }
      });
    },
    onSuccess: async () => {
      setQuickIntent("");
      setFormError(null);
      await queryClient.invalidateQueries({ queryKey: ["tasks-manager"] });
    }
  });

  const tasks = pickRecords(tasksQ.data, "tasks");
  const counts = useMemo(() => {
    const by = { total: tasks.length, queued: 0, running: 0, needs_approval: 0, done: 0 };
    for (const t of tasks) {
      const s = str(t.status, "").toLowerCase();
      if (s.includes("awaitingapproval")) by.needs_approval += 1;
      else if (s.includes("inprogress")) by.running += 1;
      else if (s.includes("pending")) by.queued += 1;
      else if (s.includes("completed")) by.done += 1;
    }
    return by;
  }, [tasks]);

  return (
    <Stack spacing={2}>
      <Box className="list-shell">
        <Typography variant="h6">Tasks</Typography>
        <Typography variant="body2" color="text.secondary">
          Describe what you want in plain English. AgentArk can generate a runnable task for you.
        </Typography>
      </Box>

      <Grid2 container spacing={2}>
        <Grid2 size={{ xs: 12, sm: 6, md: 3 }}>
          <Box className="list-shell" sx={{ minHeight: 120 }}>
            <Typography variant="caption" color="text.secondary">Total</Typography>
            <Typography variant="h5">{counts.total}</Typography>
          </Box>
        </Grid2>
        <Grid2 size={{ xs: 12, sm: 6, md: 3 }}>
          <Box className="list-shell" sx={{ minHeight: 120 }}>
            <Typography variant="caption" color="text.secondary">Queued</Typography>
            <Typography variant="h5">{counts.queued}</Typography>
          </Box>
        </Grid2>
        <Grid2 size={{ xs: 12, sm: 6, md: 3 }}>
          <Box className="list-shell" sx={{ minHeight: 120 }}>
            <Typography variant="caption" color="text.secondary">Needs Approval</Typography>
            <Typography variant="h5">{counts.needs_approval}</Typography>
          </Box>
        </Grid2>
        <Grid2 size={{ xs: 12, sm: 6, md: 3 }}>
          <Box className="list-shell" sx={{ minHeight: 120 }}>
            <Typography variant="caption" color="text.secondary">Done</Typography>
            <Typography variant="h5">{counts.done}</Typography>
          </Box>
        </Grid2>
      </Grid2>

      <Box className="list-shell">
        <Typography variant="h6" mb={1}>
          Create Task (Easy)
        </Typography>
        <Grid2 container spacing={1}>
          <Grid2 size={{ xs: 12 }}>
            <TextField
              fullWidth
              multiline
              minRows={2}
              label="What should AgentArk do?"
              placeholder="Example: Every weekday at 9am send me a daily brief in Telegram."
              value={quickIntent}
              onChange={(e) => setQuickIntent(e.target.value)}
            />
          </Grid2>
          <Grid2 size={{ xs: 12, md: 4 }}>
            <TextField
              fullWidth
              size="small"
              select
              label="When"
              value={schedulePreset}
              onChange={(e) => setSchedulePreset(e.target.value)}
            >
              <MenuItem value="once">One-time (run once)</MenuItem>
              <MenuItem value="every_15">Every 15 minutes</MenuItem>
              <MenuItem value="hourly">Hourly</MenuItem>
              <MenuItem value="daily_9">Daily at 9:00</MenuItem>
              <MenuItem value="weekday_9">Weekdays at 9:00</MenuItem>
              <MenuItem value="custom">Custom cron</MenuItem>
            </TextField>
          </Grid2>
          {schedulePreset === "custom" ? (
            <Grid2 size={{ xs: 12, md: 8 }}>
              <TextField
                fullWidth
                size="small"
                label="Custom cron"
                placeholder="*/10 * * * *"
                value={customCron}
                onChange={(e) => setCustomCron(e.target.value)}
                helperText="Use 5 fields (min hour day month weekday)."
              />
            </Grid2>
          ) : null}
          <Grid2 size={{ xs: 12 }}>
            <FormControlLabel
              control={<Switch checked={requireApproval} onChange={(e) => setRequireApproval(e.target.checked)} />}
              label="Require approval before execution"
            />
          </Grid2>
          <Grid2 size={{ xs: 12 }}>
            <Button
              variant="contained"
              disabled={aiCreateMutation.isPending || opMutation.isPending || !quickIntent.trim()}
              onClick={async () => {
                setFormError(null);
                try {
                  await aiCreateMutation.mutateAsync();
                } catch (e) {
                  const msg = errMessage(e);
                  if (msg.toLowerCase().includes("llm planning failed")) {
                    setFormError("AI planner needs an active LLM model. Configure one in Settings > Models, or use Manual mode below.");
                  } else {
                    setFormError(msg);
                  }
                }
              }}
            >
              {aiCreateMutation.isPending ? "Creating..." : "Create with AI"}
            </Button>
          </Grid2>
        </Grid2>
        {formError ? <Alert severity="error" sx={{ mt: 1 }}>{formError}</Alert> : null}
      </Box>

      <Accordion expanded={manualOpen} onChange={() => setManualOpen((p) => !p)} className="accordion-shell">
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography variant="body2" sx={{ fontWeight: 600 }}>Manual Mode (Optional)</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Grid2 container spacing={1}>
            <Grid2 size={{ xs: 12, md: 4 }}>
              <TextField fullWidth size="small" label="Description" value={description} onChange={(e) => setDescription(e.target.value)} />
            </Grid2>
            <Grid2 size={{ xs: 12, md: 2 }}>
              <TextField fullWidth size="small" label="Action" value={action} onChange={(e) => setAction(e.target.value)} />
            </Grid2>
            <Grid2 size={{ xs: 12, md: 3 }}>
              <TextField fullWidth size="small" label="Cron" value={cron} onChange={(e) => setCron(e.target.value)} placeholder="*/10 * * * *" />
            </Grid2>
            <Grid2 size={{ xs: 12, md: 3 }}>
              <TextField fullWidth size="small" select label="Approval" value={approval} onChange={(e) => setApproval(e.target.value)}>
                <MenuItem value="auto">auto</MenuItem>
                <MenuItem value="require">require</MenuItem>
              </TextField>
            </Grid2>
            <Grid2 size={{ xs: 12 }}>
              <TextField fullWidth multiline minRows={2} label="Arguments JSON" value={argumentsJson} onChange={(e) => setArgumentsJson(e.target.value)} />
            </Grid2>
            <Grid2 size={{ xs: 12 }}>
              <Button
                variant="outlined"
                disabled={opMutation.isPending || !description.trim()}
                onClick={async () => {
                  setFormError(null);
                  try {
                    const parsed = JSON.parse(argumentsJson || "{}");
                    await opMutation.mutateAsync({
                      path: "/tasks",
                      method: "POST",
                      payload: { description: description.trim(), action: action.trim(), arguments: parsed, cron: cron.trim() || null, approval }
                    });
                    setDescription("");
                  } catch (e) {
                    setFormError(errMessage(e));
                  }
                }}
              >
                Add Manual Task
              </Button>
            </Grid2>
          </Grid2>
        </AccordionDetails>
      </Accordion>

      <Box className="list-shell">
        <Typography variant="h6" mb={1}>
          Task List
        </Typography>
        <TableContainer className="table-shell" sx={{ width: "100%", overflowX: "auto" }}>
          <Table size="small" sx={{ minWidth: 860 }}>
            <TableHead>
              <TableRow>
                <TableCell>Description</TableCell>
                <TableCell>Action</TableCell>
                <TableCell>Status</TableCell>
                <TableCell>Schedule</TableCell>
                <TableCell>Created</TableCell>
                <TableCell align="right">Ops</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {tasks.map((task) => {
                const id = str(task.id, "");
                const cronExpr = str(task.cron, "");
                const schedule = cronExpr ? `cron: ${cronExpr}` : "manual";
                const rawStatus = str(task.status, "-");
                return (
                  <TableRow key={id}>
                    <TableCell sx={{ maxWidth: 520 }}>
                      <Typography variant="body2" noWrap title={str(task.description)}>
                        {str(task.description)}
                      </Typography>
                    </TableCell>
                    <TableCell sx={{ maxWidth: 220 }}>
                      <Typography variant="body2" noWrap title={str(task.action)}>
                        {str(task.action)}
                      </Typography>
                    </TableCell>
                    <TableCell>
                      <Chip size="small" label={statusLabel(rawStatus)} color={statusColor(rawStatus)} />
                    </TableCell>
                    <TableCell sx={{ maxWidth: 220 }}>
                      <Typography variant="body2" noWrap title={schedule}>
                        {schedule}
                      </Typography>
                    </TableCell>
                    <TableCell sx={{ whiteSpace: "nowrap" }}>{str(task.created_at)}</TableCell>
                    <TableCell align="right">
                      <RowOpsMenu
                        actions={[
                          {
                            label: "View",
                            onClick: () => setSelectedTask(asRecord(task))
                          },
                          {
                            label: "Approve",
                            disabled: !rawStatus.toLowerCase().includes("awaitingapproval"),
                            onClick: () => opMutation.mutate({ path: `/tasks/${encodeURIComponent(id)}/approve`, method: "POST" })
                          },
                          {
                            label: "Reject",
                            tone: "warning",
                            disabled: !rawStatus.toLowerCase().includes("awaitingapproval"),
                            onClick: () => opMutation.mutate({ path: `/tasks/${encodeURIComponent(id)}/reject`, method: "POST" })
                          },
                          {
                            label: "Delete",
                            tone: "error",
                            divider: true,
                            onClick: async () => {
                              const ok = window.confirm("Delete this task? This cannot be undone.");
                              if (!ok) return;
                              opMutation.mutate({ path: `/tasks/${encodeURIComponent(id)}`, method: "DELETE" });
                            }
                          }
                        ]}
                        ariaLabel="Task options"
                      />
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </TableContainer>
      </Box>

      <Dialog open={selectedTask != null} onClose={() => setSelectedTask(null)} maxWidth="md" fullWidth>
        <DialogTitle>{str(selectedTask?.description, "Task")}</DialogTitle>
        <DialogContent>
          <Stack spacing={1}>
            <Stack direction="row" spacing={1} flexWrap="wrap" alignItems="center">
              <Chip
                size="small"
                label={statusLabel(str(selectedTask?.status, ""))}
                color={statusColor(str(selectedTask?.status, ""))}
              />
              <Chip size="small" variant="outlined" label={str(selectedTask?.cron, "") ? "Scheduled" : "Manual"} />
              <Chip size="small" variant="outlined" label={`Action: ${str(selectedTask?.action, "-")}`} />
            </Stack>

            <Typography variant="caption" color="text.secondary">
              Created: {str(selectedTask?.created_at, "-")}
            </Typography>

            {str(selectedTask?.cron, "") ? (
              <Box className="metadata-box">
                <Typography variant="caption" color="text.secondary">
                  Schedule
                </Typography>
                <Typography variant="body2" sx={{ whiteSpace: "pre-wrap" }}>
                  {str(selectedTask?.cron)}
                </Typography>
              </Box>
            ) : null}

            {str(selectedTask?.result, "") ? (
              <Box className="metadata-box">
                <Typography variant="caption" color="text.secondary">
                  Last Result
                </Typography>
                <Typography variant="body2" sx={{ whiteSpace: "pre-wrap" }}>
                  {str(selectedTask?.result)}
                </Typography>
              </Box>
            ) : (
              <Typography variant="body2" color="text.secondary">
                No result yet.
              </Typography>
            )}

            <KeyValuePanel title="Arguments" data={asRecord(selectedTask?.arguments)} emptyLabel="No arguments." maxRows={18} />
            <KeyValuePanel title="System fields" data={asRecord(selectedTask)} emptyLabel="No extra fields." maxRows={10} />
          </Stack>
        </DialogContent>
      </Dialog>
    </Stack>
  );
}

function SkillsManager({ autoRefresh }: { autoRefresh: boolean }) {
  const queryClient = useQueryClient();
  const [lastImport, setLastImport] = useState<SkillImportSummary | null>(null);
  const [testResults, setTestResults] = useState<Record<string, string>>({});
  const [skillMenuAnchor, setSkillMenuAnchor] = useState<{ el: HTMLElement; name: string } | null>(null);
  const [importOpen, setImportOpen] = useState(false);
  const [bulkOpen, setBulkOpen] = useState(false);
  const [editOpen, setEditOpen] = useState(false);
  const [editTargetName, setEditTargetName] = useState<string | null>(null);
  const [developerModeEnabled, setDeveloperModeEnabledState] = useState(getDeveloperModeEnabled);
  const [editForm, setEditForm] = useState<SkillEditorForm>(defaultSkillEditorForm());
  const [editContent, setEditContent] = useState("");
  const [editError, setEditError] = useState<string | null>(null);
  const [createWizardEnabled, setCreateWizardEnabled] = useState(true);
  const [createWizardStep, setCreateWizardStep] = useState(0);
  const [editAttachHook, setEditAttachHook] = useState(false);
  const [editHookInstruction, setEditHookInstruction] = useState("");
  const [editHookTrigger, setEditHookTrigger] = useState<HookTriggerValue>("on_error");
  const [editHookUrl, setEditHookUrl] = useState("");
  const [editAttachTask, setEditAttachTask] = useState(false);
  const [editTaskInstruction, setEditTaskInstruction] = useState("");
  const [editTaskCron, setEditTaskCron] = useState("");
  const [aiCreateOpen, setAiCreateOpen] = useState(false);
  const [aiPrompt, setAiPrompt] = useState("");
  const [aiNameHint, setAiNameHint] = useState("");
  const [aiError, setAiError] = useState<string | null>(null);
  const [skillsTab, setSkillsTab] = useState<"manage" | "system">("manage");
  const [secretsName, setSecretsName] = useState<string | null>(null);
  const [hooksOpen, setHooksOpen] = useState(false);
  const [hooksTargetAction, setHooksTargetAction] = useState<string | null>(null);
  const [hookInstruction, setHookInstruction] = useState("");
  const [hookName, setHookName] = useState("");
  const [hookTrigger, setHookTrigger] = useState<HookTriggerValue>("post_action");
  const [hookUrl, setHookUrl] = useState("");
  const [hookError, setHookError] = useState<string | null>(null);
  const editRawMode = developerModeEnabled;

  useEffect(() => {
    const refreshDeveloperMode = () => setDeveloperModeEnabledState(getDeveloperModeEnabled());
    window.addEventListener(DEVELOPER_MODE_EVENT, refreshDeveloperMode as EventListener);
    window.addEventListener("storage", refreshDeveloperMode);
    return () => {
      window.removeEventListener(DEVELOPER_MODE_EVENT, refreshDeveloperMode as EventListener);
      window.removeEventListener("storage", refreshDeveloperMode);
    };
  }, []);

  const skillsQ = useQuery({
    queryKey: ["skills-manager"],
    queryFn: () => api.rawGet("/skills"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const hooksQ = useQuery({
    queryKey: ["skills-hooks"],
    queryFn: () => api.rawGet("/hooks"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const hookRunsQ = useQuery({
    queryKey: ["skills-hook-runs"],
    queryFn: () => api.rawGet("/hooks/runs?limit=200"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });

  const handleImported = async (summary: SkillImportSummary) => {
    setLastImport(summary);
    await queryClient.invalidateQueries({ queryKey: ["skills-manager"] });
  };
  const afterImport = async () => {
    await queryClient.invalidateQueries({ queryKey: ["skills-manager"] });
  };

  const setEnabledMutation = useMutation({
    mutationFn: ({ name, enabled }: { name: string; enabled: boolean }) => api.setSkillEnabled(name, enabled),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["skills-manager"] });
    }
  });

  const testMutation = useMutation({
    mutationFn: ({ name }: { name: string }) => api.testSkill(name),
    onMutate: ({ name }) => {
      setTestResults((prev) => ({ ...prev, [name]: "Running..." }));
    },
    onSuccess: (out, { name }) => {
      const status =
        out.status === "needs_input"
          ? out.message || "Needs required input."
          : out.status === "ok"
            ? out.mode === "workflow"
              ? "Workflow test completed"
              : "Skill test completed"
            : out.error || out.message || "Test returned";
      setTestResults((prev) => ({ ...prev, [name]: status }));
    },
    onError: (err, { name }) => {
      setTestResults((prev) => ({ ...prev, [name]: errMessage(err) }));
    }
  });

  const deleteSkillMutation = useMutation({
    mutationFn: (name: string) => api.deleteSkill(name),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["skills-manager"] });
    }
  });

  const addHookMutation = useMutation({
    mutationFn: (payload: {
      name: string;
      trigger: HookTriggerValue;
      hook_type: string;
      url?: string;
      action_name?: string;
    }) => api.rawPost("/hooks", payload),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["skills-hooks"] });
      await queryClient.invalidateQueries({ queryKey: ["skills-hook-runs"] });
    }
  });

  const removeHookMutation = useMutation({
    mutationFn: (id: string) => api.rawDelete(`/hooks/${encodeURIComponent(id)}`),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["skills-hooks"] });
      await queryClient.invalidateQueries({ queryKey: ["skills-hook-runs"] });
    }
  });

  const skills = pickRecords(skillsQ.data, "skills");
  const hooks = asRecords(hooksQ.data);
  const hookRuns = asRecords(hookRunsQ.data);
  const hooksForSelectedAction = hooksTargetAction
    ? hooks.filter((h) => isHookRecordAttachedToAction(h, hooksTargetAction))
    : hooks;
  const systemSkills = skills.filter((a) => str(a.source).toLowerCase() === "system");
  const bundledSkills = skills.filter((a) => str(a.source).toLowerCase() === "bundled");
  const customSkills = skills.filter((a) => str(a.source).toLowerCase() === "custom");
  const availableToolNames = dedupeStrings(systemSkills.map((a) => str(a.name, "").trim()).filter(Boolean));
  const allSkillNames = dedupeStrings(skills.map((a) => str(a.name, "").trim()).filter(Boolean));
  const hookLastRunById = useMemo(() => {
    const map: Record<string, JsonRecord> = {};
    for (const run of hookRuns) {
      const id = str(run.hook_id, "");
      if (!id || map[id]) continue;
      map[id] = run;
    }
    return map;
  }, [hookRuns]);

  const closeEditor = () => {
    setEditOpen(false);
    setEditTargetName(null);
    setEditForm(defaultSkillEditorForm());
    setEditContent("");
    setEditError(null);
    setCreateWizardEnabled(true);
    setCreateWizardStep(0);
    setEditAttachHook(false);
    setEditHookInstruction("");
    setEditHookTrigger("on_error");
    setEditHookUrl("");
    setEditAttachTask(false);
    setEditTaskInstruction("");
    setEditTaskCron("");
  };

  const closeHooksDialog = () => {
    setHooksOpen(false);
    setHooksTargetAction(null);
    setHookInstruction("");
    setHookName("");
    setHookTrigger("post_action");
    setHookUrl("");
    setHookError(null);
  };

  const openHooksDialog = (actionName?: string) => {
    const target = actionName?.trim() || null;
    const baseName = target ? "hook" : "custom-hook";
    setHooksTargetAction(target);
    setHookInstruction(target ? `notify me when ${target} fails` : "");
    setHookTrigger(target ? "on_error" : "post_action");
    setHookName(baseName);
    setHookUrl("");
    setHookError(null);
    setHooksOpen(true);
  };

  const applyHookInstruction = () => {
    const trigger = inferHookTriggerFromInstruction(hookInstruction, hooksTargetAction ? "on_error" : "post_action");
    const extractedUrl = extractFirstUrl(hookInstruction);
    const actionPart = hooksTargetAction ? "" : "custom-";
    const triggerPart = trigger.replace(/_/g, "-");
    const suggestedName = sanitizeHookName(`${actionPart}${triggerPart}`) || "custom-hook";
    setHookTrigger(trigger);
    if (!hookName.trim()) setHookName(suggestedName);
    if (extractedUrl && !hookUrl.trim()) setHookUrl(extractedUrl);
  };

  const applyEditHookInstruction = () => {
    const trigger = inferHookTriggerFromInstruction(editHookInstruction, "on_error");
    const extractedUrl = extractFirstUrl(editHookInstruction);
    setEditHookTrigger(trigger);
    if (extractedUrl && !editHookUrl.trim()) setEditHookUrl(extractedUrl);
  };

  const applyEditTaskInstruction = () => {
    const inferredCron = inferTaskCronFromInstruction(editTaskInstruction);
    if (inferredCron && !editTaskCron.trim()) setEditTaskCron(inferredCron);
  };

  const saveHookFromDialog = async () => {
    setHookError(null);
    try {
      const effectiveTrigger = inferHookTriggerFromInstruction(hookInstruction, hookTrigger);
      const effectiveUrl = hookUrl.trim() || extractFirstUrl(hookInstruction);
      if (!effectiveUrl) {
        setHookError("Send update URL is required.");
        return;
      }
      const rawName = sanitizeHookName(hookName) || "hook";
      const finalName = hooksTargetAction
        ? (isHookAttachedToAction(rawName, hooksTargetAction)
            ? rawName
            : sanitizeHookName(`action-${hooksTargetAction}-${rawName}`))
        : rawName;
      await addHookMutation.mutateAsync({
        name: finalName,
        trigger: effectiveTrigger,
        hook_type: "webhook",
        url: effectiveUrl,
        action_name: hooksTargetAction || undefined
      });
      closeHooksDialog();
    } catch (err) {
      setHookError(errMessage(err));
    }
  };

  const openEditor = async (name: string) => {
    setEditError(null);
    setEditTargetName(name);
    setEditOpen(true);
    setCreateWizardEnabled(false);
    setCreateWizardStep(0);
    setEditAttachHook(false);
    setEditHookInstruction("");
    setEditHookTrigger("on_error");
    setEditHookUrl("");
    setEditAttachTask(false);
    setEditTaskInstruction("");
    setEditTaskCron("");
    try {
      const out = (await api.rawGet(`/skills/${encodeURIComponent(name)}`)) as JsonRecord;
      const content = str(out.content, "");
      const parsed = parseSkillEditorForm(content, name);
      setEditContent(content);
      setEditForm({ ...parsed, name });
    } catch (err) {
      setEditError(errMessage(err));
    }
  };

  const openNewEditor = (initial?: { name?: string; content?: string }) => {
    const initialName = normalizeActionName(initial?.name || "new-action") || "new-action";
    const initialContent = (initial?.content || "").trim();
    const parsed = initialContent
      ? parseSkillEditorForm(initialContent, initialName)
      : defaultSkillEditorForm(initialName);
    const normalizedName = normalizeActionName(parsed.name || initialName) || "new-action";
    const form = { ...parsed, name: normalizedName };
    const content = initialContent || buildSkillMdFromForm("", form);
    setEditTargetName(null);
    setEditForm(form);
    setEditContent(content);
    setEditError(null);
    setCreateWizardEnabled(true);
    setCreateWizardStep(0);
    setEditAttachHook(false);
    setEditHookInstruction("");
    setEditHookTrigger("on_error");
    setEditHookUrl("");
    setEditAttachTask(false);
    setEditTaskInstruction("");
    setEditTaskCron("");
    setEditOpen(true);
  };

  const aiGenerateMutation = useMutation({
    mutationFn: async ({ prompt, nameHint }: { prompt: string; nameHint: string }) => {
      const fallbackName = normalizeActionName(nameHint || "new-action") || "new-action";
      const toolsText = availableToolNames.length > 0 ? availableToolNames.join(", ") : "web_search";
      const existingText = allSkillNames.length > 0 ? allSkillNames.join(", ") : "(none)";
      const generationPrompt = [
        "Create a complete SKILL.md for AgentArk.",
        "",
        "Return ONLY the SKILL.md content. No explanation, no markdown fences.",
        "The file must use YAML frontmatter exactly with keys: name, description, version, required_inputs, metadata.emoji, requires.tools.",
        "Use version \"1.0.0\".",
        "Skill name must be lowercase letters, numbers, and hyphens only.",
        `Name hint: ${fallbackName}`,
        `Available tool skills to reference in workflow guidance: ${toolsText}`,
        `Existing skill names (avoid collisions): ${existingText}`,
        "",
        "Task request:",
        prompt.trim()
      ].join("\n");

      const out = (await api.chat({ message: generationPrompt, channel: "web" })) as JsonRecord;
      const raw = str(out.response, "");
      const actionMd = extractActionMdFromModelOutput(raw);
      if (!actionMd.trim()) throw new Error("AI did not return skill content.");
      return { actionMd, fallbackName };
    },
    onSuccess: ({ actionMd, fallbackName }) => {
      const parsed = parseSkillEditorForm(actionMd, fallbackName);
      const normalizedName = normalizeActionName(parsed.name || fallbackName) || "new-action";
      const normalizedForm = { ...parsed, name: normalizedName };
      const normalizedContent = buildSkillMdFromForm(actionMd, normalizedForm);
      setAiError(null);
      setAiCreateOpen(false);
      setAiPrompt("");
      setAiNameHint("");
      openNewEditor({ name: normalizedName, content: normalizedContent });
    },
    onError: (err) => {
      setAiError(errMessage(err));
    }
  });

  const saveEditor = async () => {
    setEditError(null);
    try {
      const createMode = !editTargetName;
      let targetName = editTargetName || normalizeActionName(editForm.name);
      if (createMode && editRawMode) {
        const parsed = parseSkillEditorForm(editContent, targetName || "new-action");
        const parsedName = normalizeActionName(parsed.name);
        if (parsedName) targetName = parsedName;
      }
      if (!targetName) targetName = "new-action";

      if (createMode && !isValidActionName(targetName)) {
        setEditError("Skill name must use lowercase letters, numbers, and hyphens only.");
        return;
      }

      const formForSave: SkillEditorForm = {
        ...editForm,
        name: targetName,
        version: (editForm.version || "").trim() || "1.0.0"
      };
      const finalContent = editRawMode ? editContent : buildSkillMdFromForm(editContent, formForSave);

      if (createMode) {
        const out = (await api.rawPost("/skills", { name: targetName, content: finalContent, force: false })) as JsonRecord;
        const status = str(out.status, "ok").toLowerCase();
        if (status === "blocked") {
          setEditError(str(out.error, str(out.message, "Skill was blocked by security verification.")));
          return;
        }
      } else {
        await api.rawPost(`/skills/${encodeURIComponent(targetName)}`, { content: finalContent });
      }

      const editEffectiveUrl = editHookUrl.trim() || extractFirstUrl(editHookInstruction);
      if (editAttachHook && editEffectiveUrl) {
        const hookBase = sanitizeHookName(inferHookTriggerFromInstruction(editHookInstruction, editHookTrigger).replace(/_/g, "-")) || "hook";
        const hookName = sanitizeHookName(`action-${targetName}-${hookBase}`) || `action-${targetName}-hook`;
        await addHookMutation.mutateAsync({
          name: hookName,
          trigger: inferHookTriggerFromInstruction(editHookInstruction, editHookTrigger),
          hook_type: "webhook",
          url: editEffectiveUrl,
          action_name: targetName
        });
      }

      if (editAttachTask) {
        const inferredCron = inferTaskCronFromInstruction(editTaskInstruction);
        const effectiveCron = editTaskCron.trim() || inferredCron;
        const runOnce = isRunOnceInstruction(editTaskInstruction);
        if (!effectiveCron && !runOnce) {
          setEditError("Could not understand schedule. Try: every day at 9am, hourly, weekdays, or paste a cron.");
          return;
        }
        await api.rawPost("/tasks", {
          description: `Run skill '${targetName}' automatically`,
          action: targetName,
          arguments: {},
          cron: runOnce ? null : effectiveCron,
          approval: "auto"
        });
      }

      closeEditor();
      await queryClient.invalidateQueries({ queryKey: ["skills-manager"] });
      await queryClient.invalidateQueries({ queryKey: ["tasks-manager"] });
    } catch (err) {
      setEditError(errMessage(err));
    }
  };

  const toggleEnabled = async (name: string, nextEnabled: boolean) => {
    if (nextEnabled) {
      try {
        const secrets = await api.getSkillSecrets(name);
        if ((secrets.missing_env || []).length > 0) {
          setLastImport({
            result: { status: "needs_secrets", name, message: "Missing secrets", secrets: { missing_env: secrets.missing_env, required_env: secrets.required_env, bindings: secrets.bindings } },
            message: `Cannot enable '${name}' until secrets are configured: ${secrets.missing_env.join(", ")}`
          });
          setSecretsName(name);
          return;
        }
      } catch (err) {
        setLastImport({
          result: { status: "error", name, message: "Secrets check failed" },
          message: `Cannot enable '${name}': ${errMessage(err)}`
        });
        return;
      }
    }
    await setEnabledMutation.mutateAsync({ name, enabled: nextEnabled });
  };

  const renderActionRow = (action: JsonRecord, type: "system" | "bundled" | "custom") => {
    const name = str(action.name, "Untitled");
    const description = str(action.description, "No description");
    const version = str(action.version, "?");
    const enabled = toBool(action.enabled);
    const testMessage = testResults[name];
    const isTesting = testMutation.isPending && testMutation.variables?.name === name;
    const isSystem = type === "system";

    const menuOpen = skillMenuAnchor?.name === name;

    return (
      <Box
        key={`${type}-${name}`}
        className="action-row"
        sx={{
          width: "100%",
          opacity: isSystem ? 0.7 : 1,
          filter: isSystem ? "saturate(0.85)" : "none"
        }}
      >
        <Stack direction="row" alignItems="center" justifyContent="space-between" spacing={2}>
          <Stack spacing={0.5} sx={{ flex: 1, minWidth: 0 }}>
            <Stack direction="row" alignItems="center" spacing={1}>
              <Typography variant="subtitle1" fontWeight={600} noWrap>
                {name}
              </Typography>
              {!enabled && !isSystem ? (
                <Chip label="Disabled" size="small" color="warning" variant="outlined" sx={{ height: 20, fontSize: "0.65rem" }} />
              ) : null}
            </Stack>
            <Typography variant="caption" color="text.secondary" noWrap>
              {description}
            </Typography>
            {testMessage ? (
              <Typography variant="caption" color="text.secondary">
                {testMessage}
              </Typography>
            ) : null}
          </Stack>
          <Stack direction="row" spacing={0.5} alignItems="center">
            <Typography variant="caption" color="text.secondary" sx={{ whiteSpace: "nowrap" }}>
              v{version}
            </Typography>
            {!isSystem ? (
              <>
                <IconButton
                  size="small"
                  onClick={(e: MouseEvent<HTMLButtonElement>) => setSkillMenuAnchor({ el: e.currentTarget, name })}
                >
                  <MoreVertIcon fontSize="small" />
                </IconButton>
                <Menu
                  anchorEl={menuOpen ? skillMenuAnchor.el : null}
                  open={menuOpen}
                  onClose={() => setSkillMenuAnchor(null)}
                  slotProps={{ paper: { sx: { minWidth: 160 } } }}
                >
                  <MenuItem onClick={() => { setSkillMenuAnchor(null); openEditor(name); }}>
                    Edit
                  </MenuItem>
                  <MenuItem onClick={() => { setSkillMenuAnchor(null); setSecretsName(name); }}>
                    Secrets
                  </MenuItem>
                  <MenuItem
                    disabled={isTesting || !enabled}
                    onClick={() => { setSkillMenuAnchor(null); testMutation.mutate({ name }); }}
                  >
                    {isTesting ? "Testing..." : "Test"}
                  </MenuItem>
                  <MenuItem
                    disabled={setEnabledMutation.isPending}
                    onClick={() => { setSkillMenuAnchor(null); toggleEnabled(name, !enabled); }}
                  >
                    {enabled ? "Disable" : "Enable"}
                  </MenuItem>
                  {developerModeEnabled ? (
                    <MenuItem onClick={() => { setSkillMenuAnchor(null); openHooksDialog(name); }}>
                      Automations
                    </MenuItem>
                  ) : null}
                  <Divider />
                  <MenuItem
                    disabled={deleteSkillMutation.isPending}
                    sx={{ color: "error.main" }}
                    onClick={async () => {
                      setSkillMenuAnchor(null);
                      const ok = window.confirm(`Delete skill "${name}"? This cannot be undone.`);
                      if (ok) deleteSkillMutation.mutate(name);
                    }}
                  >
                    Delete
                  </MenuItem>
                </Menu>
              </>
            ) : null}
          </Stack>
        </Stack>
      </Box>
    );
  };

  const isCreateMode = !editTargetName;
  const useCreateWizard = isCreateMode && !editRawMode && createWizardEnabled;
  const scheduleInference = editTaskCron.trim() || inferTaskCronFromInstruction(editTaskInstruction);
  const scheduleBlocked = editAttachTask && !scheduleInference && !isRunOnceInstruction(editTaskInstruction);
  const hookBlocked = editAttachHook && !(editHookUrl.trim() || extractFirstUrl(editHookInstruction));
  const wizardStepBlocked =
    createWizardStep === 0
      ? !editForm.name.trim() || !isValidActionName(editForm.name) || !editForm.description.trim()
      : createWizardStep === 2
        ? hookBlocked || scheduleBlocked
        : false;

  return (
    <Stack spacing={2}>
      <Box className="list-shell">
        <Stack direction="row" justifyContent="space-between" alignItems="center" mb={1}>
          <Typography variant="h6">Skills</Typography>
          {skillsTab === "manage" ? (
            <Stack direction="row" spacing={1}>
              <Button size="small" variant="outlined" onClick={() => setAiCreateOpen(true)}>
                Create Skill
              </Button>
              <Button size="small" variant="outlined" onClick={() => setImportOpen(true)}>
                Import URL
              </Button>
              <Button size="small" variant="outlined" onClick={() => setBulkOpen(true)}>
                Bulk Import
              </Button>
            </Stack>
          ) : null}
        </Stack>
        <Typography variant="body2" color="text.secondary">
          System skills: {systemSkills.length}, custom skills: {customSkills.length}, automations: {hooks.length}.
        </Typography>
        {skillsTab === "manage" ? (
          <Typography variant="caption" color="text.secondary" sx={{ display: "block", mt: 0.5 }}>
            Start with AI Quick Create. Use Advanced Editor only when you need manual SKILL.md control.
          </Typography>
        ) : null}
        {lastImport?.message ? (
          <Alert sx={{ mt: 1 }} severity={lastImport.result.status === "blocked" ? "warning" : "info"}>
            {lastImport.message}
          </Alert>
        ) : null}
        <Tabs
          value={skillsTab}
          onChange={(_, value: "manage" | "system") => setSkillsTab(value)}
          sx={{ mt: 1 }}
        >
          <Tab value="manage" label="My Skills" />
          <Tab value="system" label="System Skills" />
        </Tabs>
        <Typography variant="caption" color="text.secondary" sx={{ mt: 1 }}>
          These are pre-built skills. You can always chat with the agent to build anything custom on your own.
        </Typography>
      </Box>

      {skillsTab === "manage" ? (
        <>
          {customSkills.length > 0 ? (
            <Box className="list-shell">
              <Stack spacing={1}>
                <Typography variant="h6">Custom Skills</Typography>
                <Stack spacing={1}>{customSkills.map((act) => renderActionRow(act, "custom"))}</Stack>
              </Stack>
            </Box>
          ) : null}

          {developerModeEnabled ? (
            <Box className="list-shell">
              <Stack spacing={1}>
                <Stack direction="row" justifyContent="space-between" alignItems="center">
                  <Stack spacing={0.25}>
                    <Typography variant="h6">Automations</Typography>
                    <Typography variant="caption" color="text.secondary">
                      Advanced automation manager (Developer mode). Create from an action row.
                    </Typography>
                  </Stack>
                </Stack>
                {hooksQ.error ? (
                  <Alert severity="error">{errMessage(hooksQ.error)}</Alert>
                ) : hookRunsQ.error ? (
                  <Alert severity="warning">Automations loaded, but run reports failed: {errMessage(hookRunsQ.error)}</Alert>
                ) : hooks.length === 0 ? (
                  <Typography variant="body2" color="text.secondary">
                    No automations yet.
                  </Typography>
                ) : (
                  <TableContainer className="table-shell">
                    <Table size="small">
                      <TableHead>
                        <TableRow>
                          <TableCell>Name</TableCell>
                          <TableCell>Trigger</TableCell>
                          <TableCell>Type</TableCell>
                          <TableCell>URL</TableCell>
                          <TableCell>Enabled</TableCell>
                          <TableCell>Last run</TableCell>
                          <TableCell align="right">Ops</TableCell>
                        </TableRow>
                      </TableHead>
                      <TableBody>
                        {hooks.map((hook, idx) => {
                          const id = str(hook.id, `hook-${idx}`);
                          const lastRun = hookLastRunById[id];
                          const runStatus = str(lastRun?.status, "-");
                          const runAttempts = num(lastRun?.attempts, 0);
                          const runError = str(lastRun?.error, "");
                          return (
                            <TableRow key={id}>
                              <TableCell>{str(hook.name, "-")}</TableCell>
                              <TableCell>{str(hook.trigger, "-")}</TableCell>
                              <TableCell>{str(hook.hook_type, "-")}</TableCell>
                              <TableCell sx={{ maxWidth: 280 }}>
                                <Typography variant="caption" color="text.secondary" noWrap title={str(hook.url, "-")}>
                                  {str(hook.url, "-")}
                                </Typography>
                              </TableCell>
                              <TableCell>{boolText(hook.enabled)}</TableCell>
                              <TableCell sx={{ maxWidth: 240 }}>
                                {lastRun ? (
                                  <Typography
                                    variant="caption"
                                    color={runStatus === "failed" ? "error.main" : "text.secondary"}
                                    noWrap
                                    title={runError || str(lastRun?.timestamp, "")}
                                  >
                                    {runStatus}
                                    {runAttempts > 0 ? ` (${runAttempts})` : ""}
                                  </Typography>
                                ) : (
                                  <Typography variant="caption" color="text.secondary">
                                    never
                                  </Typography>
                                )}
                              </TableCell>
                              <TableCell align="right">
                                <RowOpsMenu
                                  actions={[
                                    {
                                      label: "Remove",
                                      tone: "error",
                                      disabled: removeHookMutation.isPending,
                                      onClick: async () => {
                                        const ok = window.confirm("Remove this automation?");
                                        if (!ok) return;
                                        try {
                                          await removeHookMutation.mutateAsync(id);
                                        } catch (err) {
                                          setLastImport({
                                            result: { status: "error", name: str(hook.name, "automation"), message: errMessage(err) },
                                            message: `Failed to remove automation '${str(hook.name, "automation")}': ${errMessage(err)}`
                                          });
                                        }
                                      }
                                    }
                                  ]}
                                  ariaLabel="Automation options"
                                />
                              </TableCell>
                            </TableRow>
                          );
                        })}
                      </TableBody>
                    </Table>
                  </TableContainer>
                )}
              </Stack>
            </Box>
          ) : null}

          <Box className="list-shell">
            <Accordion
              defaultExpanded={false}
              elevation={0}
              sx={{
                background: "transparent",
                "&::before": { display: "none" }
              }}
            >
              <AccordionSummary expandIcon={<ExpandMoreIcon />} sx={{ px: 0 }}>
                <Stack spacing={0.25}>
                  <Typography variant="h6">Bundled Skills</Typography>
                  <Typography variant="caption" color="text.secondary">
                    Ready-made skills you can enable and use.
                  </Typography>
                </Stack>
              </AccordionSummary>
              <AccordionDetails sx={{ px: 0, pt: 0 }}>
                {bundledSkills.length === 0 ? (
                  <Typography variant="body2" color="text.secondary">
                    No bundled skills detected.
                  </Typography>
                ) : (
                  <Stack spacing={1}>{bundledSkills.map((act) => renderActionRow(act, "bundled"))}</Stack>
                )}
              </AccordionDetails>
            </Accordion>
          </Box>
        </>
      ) : (
        <Box className="list-shell">
          <Stack spacing={1}>
            <Typography variant="h6">System Skills</Typography>
            <Typography variant="caption" color="text.secondary">
              Built-in and locked. Always available.
            </Typography>
            {systemSkills.length === 0 ? (
              <Typography variant="body2" color="text.secondary">
                No system skills detected.
              </Typography>
            ) : (
              <Stack spacing={1}>{systemSkills.map((act) => renderActionRow(act, "system"))}</Stack>
            )}
          </Stack>
        </Box>
      )}

      <ImportUrlDialog
        open={importOpen}
        onClose={() => setImportOpen(false)}
        onImported={handleImported}
        onAfterImport={afterImport}
      />
      <BulkImportDialog
        open={bulkOpen}
        onClose={() => setBulkOpen(false)}
        onImported={handleImported}
        onAfterImport={afterImport}
      />

      <Dialog open={aiCreateOpen} onClose={() => setAiCreateOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>Create Skill</DialogTitle>
        <DialogContent dividers>
          <Stack spacing={1.25}>
            <Alert severity="info">
              AI Quick Create is recommended for beginners. Describe your goal in plain language.
            </Alert>
            <Typography variant="caption" color="text.secondary" sx={{ whiteSpace: "pre-line" }}>
              {`Prompt examples:
1. Track top 10 AI startups weekly, compare funding/news changes, and output a ranked briefing with sources.
2. Review competitor pricing pages every day and generate a change log with impact notes.
3. Generate a pre-meeting research brief for a company from latest news, filings, and leadership updates.
4. If this analysis fails, send update to URL (for example: your Twilio/Telegram notifier endpoint).
5. Run this every weekday at 9am and send a summary after each run.`}
            </Typography>
            {aiError ? <Alert severity="error">{aiError}</Alert> : null}
            <TextField
              fullWidth
              size="small"
              label="Skill name (optional)"
              placeholder="example: market-analyzer"
              value={aiNameHint}
              onChange={(e) => setAiNameHint(normalizeActionName(e.target.value))}
              helperText="If blank, AI will suggest a name."
            />
            <TextField
              fullWidth
              multiline
              minRows={6}
              label="What should this skill do?"
              placeholder="Example: Find small-cap momentum stocks, validate with latest filings, then output top 5 picks with risks."
              value={aiPrompt}
              onChange={(e) => setAiPrompt(e.target.value)}
            />
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button
            onClick={() => {
              setAiCreateOpen(false);
              openNewEditor();
            }}
          >
            Advanced Editor
          </Button>
          <Button onClick={() => setAiCreateOpen(false)}>Close</Button>
          <Button
            variant="contained"
            disabled={aiGenerateMutation.isPending || !aiPrompt.trim()}
            onClick={() => {
              setAiError(null);
              aiGenerateMutation.mutate({ prompt: aiPrompt.trim(), nameHint: aiNameHint.trim() });
            }}
          >
            {aiGenerateMutation.isPending ? "Creating..." : "Create with AI"}
          </Button>
        </DialogActions>
      </Dialog>

      <Dialog open={editOpen} onClose={closeEditor} maxWidth="md" fullWidth>
        <DialogTitle>{editTargetName ? `Edit skill: ${editTargetName}` : "Create skill"}</DialogTitle>
        <DialogContent dividers>
          <Stack spacing={1.25}>
            {editError ? <Alert severity="error">{editError}</Alert> : null}
            {editRawMode ? (
              <Alert severity="warning">
                Developer mode is enabled. You are editing raw SKILL.md directly.
              </Alert>
            ) : (
              <Alert severity="info">
                Beginner mode is on. Fill simple fields and AgentArk will generate the SKILL file for you.
                Need raw SKILL.md editing? Enable Developer mode in Settings -&gt; Advanced.
              </Alert>
            )}

            {isCreateMode && !editRawMode ? (
              <FormControlLabel
                control={<Switch checked={createWizardEnabled} onChange={(e) => setCreateWizardEnabled(e.target.checked)} />}
                label="Use 3-step wizard (recommended). Turn off to use the classic editor."
              />
            ) : null}

            {editRawMode ? (
              <TextField
                fullWidth
                multiline
                minRows={16}
                value={editContent}
                onChange={(e) => setEditContent(e.target.value)}
                label="SKILL.md"
              />
            ) : useCreateWizard ? (
              <Stack spacing={1.25}>
                <Tabs value={createWizardStep} onChange={(_, v) => setCreateWizardStep(Number(v) || 0)} variant="fullWidth">
                  <Tab label="1. What it does" value={0} />
                  <Tab label="2. Inputs" value={1} />
                  <Tab label="3. Run automatically" value={2} />
                </Tabs>

                {createWizardStep === 0 ? (
                  <Grid2 container spacing={1.25}>
                    <Grid2 size={{ xs: 12, md: 6 }}>
                      <TextField
                        fullWidth
                        size="small"
                        label="Skill name"
                        value={editForm.name}
                        onChange={(e) => setEditForm((prev) => ({ ...prev, name: normalizeActionName(e.target.value) }))}
                        helperText="Use lowercase letters, numbers, and hyphens. Example: market-analysis"
                      />
                    </Grid2>
                    <Grid2 size={{ xs: 12, md: 6 }}>
                      <TextField
                        fullWidth
                        size="small"
                        label="Version"
                        value={editForm.version}
                        onChange={(e) => setEditForm((prev) => ({ ...prev, version: e.target.value }))}
                        helperText="Default: 1.0.0"
                      />
                    </Grid2>
                    <Grid2 size={{ xs: 12 }}>
                      <TextField
                        fullWidth
                        size="small"
                        label="Description"
                        value={editForm.description}
                        onChange={(e) => setEditForm((prev) => ({ ...prev, description: e.target.value }))}
                        helperText="One line: what this skill does."
                      />
                    </Grid2>
                    <Grid2 size={{ xs: 12 }}>
                      <TextField
                        fullWidth
                        multiline
                        minRows={10}
                        label="Workflow instructions"
                        value={editForm.workflow}
                        onChange={(e) => setEditForm((prev) => ({ ...prev, workflow: e.target.value }))}
                        helperText="Write clear instructions for how this skill should execute."
                      />
                    </Grid2>
                  </Grid2>
                ) : null}

                {createWizardStep === 1 ? (
                  <Grid2 container spacing={1.25}>
                    <Grid2 size={{ xs: 12 }}>
                      <TextField
                        fullWidth
                        size="small"
                        label="Required inputs (optional)"
                        placeholder="from, to, budget"
                        value={editForm.requiredInputsCsv}
                        onChange={(e) => setEditForm((prev) => ({ ...prev, requiredInputsCsv: e.target.value }))}
                        helperText="Comma separated field names. If missing at runtime, user will be asked (or fallback used in scheduled runs)."
                      />
                    </Grid2>
                    <Grid2 size={{ xs: 12, md: 4 }}>
                      <TextField
                        fullWidth
                        size="small"
                        label="Emoji (optional)"
                        value={editForm.emoji}
                        onChange={(e) => setEditForm((prev) => ({ ...prev, emoji: e.target.value }))}
                      />
                    </Grid2>
                    <Grid2 size={{ xs: 12, md: 8 }}>
                      <TextField
                        fullWidth
                        size="small"
                        label="Tools (comma separated)"
                        placeholder="web_search, file_read"
                        value={editForm.toolsCsv}
                        onChange={(e) => setEditForm((prev) => ({ ...prev, toolsCsv: e.target.value }))}
                        helperText="These are skills/tools your workflow may rely on."
                      />
                    </Grid2>
                  </Grid2>
                ) : null}
              </Stack>
            ) : (
              <Grid2 container spacing={1.25}>
                <Grid2 size={{ xs: 12, md: 6 }}>
                  <TextField
                    fullWidth
                    size="small"
                    label="Skill name"
                    value={editForm.name}
                    disabled={!!editTargetName}
                    onChange={(e) => setEditForm((prev) => ({ ...prev, name: normalizeActionName(e.target.value) }))}
                    helperText={
                      editTargetName
                        ? "Skill name is fixed for existing skills."
                        : "Use lowercase letters, numbers, and hyphens. Example: market-analysis"
                    }
                  />
                </Grid2>
                <Grid2 size={{ xs: 12, md: 6 }}>
                  <TextField
                    fullWidth
                    size="small"
                    label="Version"
                    value={editForm.version}
                    onChange={(e) => setEditForm((prev) => ({ ...prev, version: e.target.value }))}
                    helperText="Default: 1.0.0"
                  />
                </Grid2>
                <Grid2 size={{ xs: 12 }}>
                  <TextField
                    fullWidth
                    size="small"
                    label="Description"
                    value={editForm.description}
                    onChange={(e) => setEditForm((prev) => ({ ...prev, description: e.target.value }))}
                    helperText="One line: what this skill does."
                  />
                </Grid2>
                <Grid2 size={{ xs: 12 }}>
                  <TextField
                    fullWidth
                    size="small"
                    label="Required inputs (optional)"
                    placeholder="from, to, budget"
                    value={editForm.requiredInputsCsv}
                    onChange={(e) => setEditForm((prev) => ({ ...prev, requiredInputsCsv: e.target.value }))}
                    helperText="Comma separated field names. If missing at runtime, user will be asked (or fallback used in scheduled runs)."
                  />
                </Grid2>
                <Grid2 size={{ xs: 12, md: 4 }}>
                  <TextField
                    fullWidth
                    size="small"
                    label="Emoji (optional)"
                    value={editForm.emoji}
                    onChange={(e) => setEditForm((prev) => ({ ...prev, emoji: e.target.value }))}
                  />
                </Grid2>
                <Grid2 size={{ xs: 12, md: 8 }}>
                  <TextField
                    fullWidth
                    size="small"
                    label="Tools (comma separated)"
                    placeholder="web_search, file_read"
                    value={editForm.toolsCsv}
                    onChange={(e) => setEditForm((prev) => ({ ...prev, toolsCsv: e.target.value }))}
                    helperText="These are skills/tools your workflow may rely on."
                  />
                </Grid2>
                <Grid2 size={{ xs: 12 }}>
                  <TextField
                    fullWidth
                    multiline
                    minRows={14}
                    label="Workflow instructions"
                    value={editForm.workflow}
                    onChange={(e) => setEditForm((prev) => ({ ...prev, workflow: e.target.value }))}
                    helperText="Write clear instructions for how this skill should execute."
                  />
                </Grid2>
              </Grid2>
            )}

            {!useCreateWizard || createWizardStep === 2 ? (
              <Box className="metadata-box">
              <Stack spacing={1}>
                <FormControlLabel
                  control={<Switch checked={editAttachHook} onChange={(e) => setEditAttachHook(e.target.checked)} />}
                  label="Run automatically (optional)"
                />
                {editAttachHook ? (
                  <Stack spacing={1}>
                    <TextField
                      fullWidth
                      size="small"
                      multiline
                      minRows={2}
                      label="When should this run? (plain language)"
                      value={editHookInstruction}
                      onChange={(e) => setEditHookInstruction(e.target.value)}
                      placeholder="Examples: when this skill fails | after each run | before this skill starts"
                    />
                    {developerModeEnabled ? (
                      <>
                        <Stack direction="row" spacing={1}>
                          <Button size="small" variant="outlined" onClick={applyEditHookInstruction}>
                            Interpret Text
                          </Button>
                          <Typography variant="caption" color="text.secondary" sx={{ alignSelf: "center" }}>
                            Infers trigger and URL.
                          </Typography>
                        </Stack>
                        <Grid2 container spacing={1}>
                          <Grid2 size={{ xs: 12, md: 4 }}>
                            <TextField
                              fullWidth
                              size="small"
                              select
                              label="When to run"
                              value={editHookTrigger}
                              onChange={(e) => setEditHookTrigger((e.target.value as HookTriggerValue) || "on_error")}
                            >
                              <MenuItem value="pre_message">pre_message</MenuItem>
                              <MenuItem value="post_message">post_message</MenuItem>
                              <MenuItem value="pre_action">pre_action</MenuItem>
                              <MenuItem value="post_action">post_action</MenuItem>
                              <MenuItem value="on_consolidate">on_consolidate</MenuItem>
                              <MenuItem value="on_error">on_error</MenuItem>
                            </TextField>
                          </Grid2>
                          <Grid2 size={{ xs: 12, md: 8 }}>
                            <TextField
                              fullWidth
                              size="small"
                              label="Send update to URL"
                              value={editHookUrl}
                              onChange={(e) => setEditHookUrl(e.target.value)}
                              placeholder="https://example.com/hook"
                            />
                          </Grid2>
                        </Grid2>
                      </>
                    ) : (
                      <TextField
                        fullWidth
                        size="small"
                        label="Send update to URL"
                        value={editHookUrl}
                        onChange={(e) => setEditHookUrl(e.target.value)}
                        placeholder="https://example.com/hook"
                        helperText="Required to enable this automation."
                      />
                    )}
                    <Typography variant="caption" color="text.secondary" sx={{ whiteSpace: "pre-line" }}>
                      {`Automation examples:
1. when this skill fails
2. after each successful run
3. before this skill starts
4. when this skill fails, send update to URL https://example.com/hook
5. when this skill fails, send update to URL https://your-notifier.example/twilio`}
                    </Typography>
                    <Typography variant="caption" color="text.secondary">
                      For phone/SMS/WhatsApp/Telegram alerts, use your notification URL endpoint to forward via Twilio or your preferred channel integration.
                    </Typography>
                  </Stack>
                ) : null}
                <Divider />
                <FormControlLabel
                  control={<Switch checked={editAttachTask} onChange={(e) => setEditAttachTask(e.target.checked)} />}
                  label="Schedule this skill (optional)"
                />
                {editAttachTask ? (
                  <Stack spacing={1}>
                    <TextField
                      fullWidth
                      size="small"
                      multiline
                      minRows={2}
                      label="When should this run? (plain language)"
                      value={editTaskInstruction}
                      onChange={(e) => setEditTaskInstruction(e.target.value)}
                      placeholder="Examples: every day at 9am | hourly | weekdays at 9am | once now"
                    />
                    <Stack direction="row" spacing={1}>
                      <Button size="small" variant="outlined" onClick={applyEditTaskInstruction}>
                        Interpret Text
                      </Button>
                      <Typography variant="caption" color="text.secondary" sx={{ alignSelf: "center" }}>
                        Infers cron schedule.
                      </Typography>
                    </Stack>
                    <TextField
                      fullWidth
                      size="small"
                      label="Cron (optional, auto-filled)"
                      value={editTaskCron}
                      onChange={(e) => setEditTaskCron(e.target.value)}
                      placeholder="0 9 * * *"
                      helperText="Use 5-field cron. Leave blank if you prefer plain language."
                    />
                    <Typography variant="caption" color="text.secondary" sx={{ whiteSpace: "pre-line" }}>
                      {`Schedule examples:
1. every day at 9am
2. every 15 minutes
3. weekdays at 9am
4. once now`}
                    </Typography>
                  </Stack>
                ) : null}
              </Stack>
            </Box>
            ) : null}
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={closeEditor}>Close</Button>
          {useCreateWizard && createWizardStep > 0 ? (
            <Button onClick={() => setCreateWizardStep((s) => Math.max(0, s - 1))}>
              Back
            </Button>
          ) : null}
          {useCreateWizard ? (
            <Button
              variant="contained"
              onClick={() => {
                if (createWizardStep < 2) {
                  setCreateWizardStep((s) => Math.min(2, s + 1));
                } else {
                  saveEditor();
                }
              }}
              disabled={wizardStepBlocked}
            >
              {createWizardStep < 2 ? "Next" : "Save"}
            </Button>
          ) : (
            <Button
              variant="contained"
              onClick={saveEditor}
              disabled={
                (editRawMode ? !editContent.trim() : !editForm.description.trim()) ||
                hookBlocked ||
                scheduleBlocked
              }
            >
              Save
            </Button>
          )}
        </DialogActions>
      </Dialog>

      <Dialog open={hooksOpen} onClose={closeHooksDialog} maxWidth="sm" fullWidth>
        <DialogTitle>{hooksTargetAction ? `Automations for ${hooksTargetAction}` : "Create Automation"}</DialogTitle>
        <DialogContent dividers>
          <Stack spacing={1.25}>
            {hookError ? <Alert severity="error">{hookError}</Alert> : null}
            <Alert severity="info">
              Describe in plain language and AgentArk will infer trigger defaults.
            </Alert>
            <Typography variant="caption" color="text.secondary">
              Advanced automation editor (Developer mode).
            </Typography>
            <TextField
              fullWidth
              multiline
              minRows={2}
              label="When should this run? (plain language)"
              value={hookInstruction}
              onChange={(e) => setHookInstruction(e.target.value)}
              placeholder={hooksTargetAction ? `when ${hooksTargetAction} fails` : "after each run"}
            />
            <Stack direction="row" spacing={1}>
              <Button size="small" variant="outlined" onClick={applyHookInstruction}>
                Interpret Text
              </Button>
              <Typography variant="caption" color="text.secondary" sx={{ alignSelf: "center" }}>
                Fills trigger and URL when detectable.
              </Typography>
            </Stack>
            <TextField
              fullWidth
              size="small"
              label="Automation name"
              value={hookName}
              onChange={(e) => setHookName(sanitizeHookName(e.target.value))}
            />
            <TextField
              fullWidth
              size="small"
              select
              label="When to run"
              value={hookTrigger}
              onChange={(e) => setHookTrigger((e.target.value as HookTriggerValue) || "post_action")}
            >
              <MenuItem value="pre_message">pre_message</MenuItem>
              <MenuItem value="post_message">post_message</MenuItem>
              <MenuItem value="pre_action">pre_action</MenuItem>
              <MenuItem value="post_action">post_action</MenuItem>
              <MenuItem value="on_consolidate">on_consolidate</MenuItem>
              <MenuItem value="on_error">on_error</MenuItem>
            </TextField>
            <TextField
              fullWidth
              size="small"
              label="Send update to URL"
              value={hookUrl}
              onChange={(e) => setHookUrl(e.target.value)}
              placeholder="https://example.com/hook"
            />
            {hooksTargetAction ? (
              <>
                <Divider />
                <Typography variant="subtitle2">Existing automations for this skill</Typography>
                {hooksForSelectedAction.length === 0 ? (
                  <Typography variant="body2" color="text.secondary">
                    No automations attached yet.
                  </Typography>
                ) : (
                  <Stack spacing={0.6}>
                    {hooksForSelectedAction.map((h, idx) => (
                      <Box key={str(h.id, `dialog-hook-${idx}`)} className="console-line">
                        <Typography variant="caption" color="text.secondary">
                          {str(h.trigger, "-")} | {boolText(h.enabled)}
                        </Typography>
                        <Typography variant="body2" noWrap title={str(h.name, "-")}>
                          {str(h.name, "-")}
                        </Typography>
                      </Box>
                    ))}
                  </Stack>
                )}
              </>
            ) : null}
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={closeHooksDialog}>Close</Button>
          <Button
            variant="contained"
            disabled={addHookMutation.isPending || !(hookUrl.trim() || extractFirstUrl(hookInstruction))}
            onClick={saveHookFromDialog}
          >
            {addHookMutation.isPending ? "Saving..." : "Save Automation"}
          </Button>
        </DialogActions>
      </Dialog>

      <SkillSecretsDialog open={secretsName != null} skillName={secretsName} onClose={() => setSecretsName(null)} />
    </Stack>
  );
}

function AppsManager({ autoRefresh }: { autoRefresh: boolean }) {
  const queryClient = useQueryClient();
  const appsQ = useQuery({ queryKey: ["apps-manager"], queryFn: () => api.rawGet("/api/apps"), refetchInterval: autoRefresh ? REFRESH_MS : false });

  const opMutation = useMutation({
    mutationFn: ({ path, method }: { path: string; method: "POST" | "DELETE" }) => (method === "DELETE" ? api.rawDelete(path) : api.rawPost(path, {})),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["apps-manager"] });
    }
  });

  const apps = pickRecords(appsQ.data, "apps");
  const origin = typeof window !== "undefined" ? window.location.origin : "";

  return (
    <Stack spacing={2}>
      <Box className="list-shell">
        <Typography variant="h6" mb={1}>Deployed Apps</Typography>
        <TableContainer className="table-shell">
          <Table size="small">
            <TableHead><TableRow><TableCell>Title</TableCell><TableCell>ID</TableCell><TableCell>Running</TableCell><TableCell>Links</TableCell><TableCell>Ops</TableCell></TableRow></TableHead>
            <TableBody>
              {apps.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={5}>
                    <Typography variant="body2" color="text.secondary">
                      There are no deployed apps at this time. When you create any app with agent, it will show here.
                    </Typography>
                  </TableCell>
                </TableRow>
              ) : (
                apps.map((appItem) => {
                  const id = str(appItem.id, "");
                  const url = str(appItem.url, "");
                  const accessUrl = str(appItem.access_url, "");
                  return (
                    <TableRow key={id}>
                      <TableCell>{str(appItem.title)}</TableCell>
                      <TableCell>{id}</TableCell>
                      <TableCell>{str(appItem.running)}</TableCell>
                      <TableCell sx={{ maxWidth: 320 }}>
                        <Typography variant="body2" noWrap title={`${origin}${accessUrl || url}`}>
                          {accessUrl || url}
                        </Typography>
                      </TableCell>
                      <TableCell align="right">
                        <RowOpsMenu
                          actions={[
                            {
                              label: "Open",
                              onClick: () => {
                                window.open(`${origin}${url}`, "_blank", "noopener,noreferrer");
                              }
                            },
                            ...(accessUrl
                              ? [
                                  {
                                    label: "Open (Key)",
                                    onClick: () => {
                                      window.open(`${origin}${accessUrl}`, "_blank", "noopener,noreferrer");
                                    }
                                  }
                                ]
                              : []),
                            {
                              label: "Share Link",
                              onClick: () => {
                                window.open(`${origin}${accessUrl || url}`, "_blank", "noopener,noreferrer");
                              }
                            },
                            {
                              label: "Stop",
                              divider: true,
                              onClick: () => opMutation.mutate({ path: `/api/apps/${encodeURIComponent(id)}/stop`, method: "POST" })
                            },
                            {
                              label: "Restart",
                              onClick: () => opMutation.mutate({ path: `/api/apps/${encodeURIComponent(id)}/restart`, method: "POST" })
                            },
                            {
                              label: "Delete",
                              tone: "error",
                              divider: true,
                              onClick: () => opMutation.mutate({ path: `/api/apps/${encodeURIComponent(id)}`, method: "DELETE" })
                            }
                          ]}
                          ariaLabel="App options"
                        />
                      </TableCell>
                    </TableRow>
                  );
                })
              )}
            </TableBody>
          </Table>
        </TableContainer>
      </Box>
    </Stack>
  );
}

function GoalsManager({ autoRefresh }: { autoRefresh: boolean }) {
  const queryClient = useQueryClient();
  const [description, setDescription] = useState("");
  const [dueDate, setDueDate] = useState("");
  const [autopilotEnabled, setAutopilotEnabled] = useState(true);
  const [guardrails, setGuardrails] = useState("");
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [scheduleKey, setScheduleKey] = useState("daily_9");
  const [reportCron, setReportCron] = useState("0 0 9 * * *"); // 09:00 daily (UTC unless server uses user tz)
  const [selectedGoalId, setSelectedGoalId] = useState<string | null>(null); // goal_id from arguments
  const [planPreview, setPlanPreview] = useState<JsonRecord | null>(null);
  const [goalCreateOpen, setGoalCreateOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const schedulePresets: { key: string; label: string; cron: string | null; hint?: string }[] = [
    { key: "run_5", label: "Every 5 minutes", cron: "0 */5 * * * *" },
    { key: "run_10", label: "Every 10 minutes", cron: "0 */10 * * * *" },
    { key: "run_30", label: "Every 30 minutes", cron: "0 */30 * * * *" },
    { key: "hourly", label: "Hourly", cron: "0 0 * * * *" },
    { key: "daily_9", label: "Daily (09:00)", cron: "0 0 9 * * *" },
    { key: "weekly_mon_9", label: "Weekly (Mon 09:00)", cron: "0 0 9 * * 1" },
    { key: "monthly_1_9", label: "Monthly (1st 09:00)", cron: "0 0 9 1 * *" },
    { key: "custom", label: "Custom", cron: null, hint: "Cron uses 6 fields: sec min hour day month weekday" }
  ];
  const scheduleLabel = (key: string) => {
    for (const p of schedulePresets) {
      if (p.key === key) return p.label;
    }
    return "Custom";
  };

  const goalsQ = useQuery({
    queryKey: ["goals-list"],
    queryFn: () => api.rawGet("/goals?limit=100"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });

  const progressPath = selectedGoalId ? `/autonomy/goals/progress?goal_id=${encodeURIComponent(selectedGoalId)}` : "/autonomy/goals/progress";
  const progressQ = useQuery({
    queryKey: ["goals-progress", selectedGoalId],
    queryFn: () => api.rawGet(progressPath),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });

  const createMutation = useMutation({
    mutationFn: (payload: { description: string; due_date?: string }) => api.rawPost("/goals", payload),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["goals-list"] });
      await queryClient.invalidateQueries({ queryKey: ["goals-progress"] });
    }
  });

  const autopilotMutation = useMutation({
    mutationFn: (payload: { goal: string; constraints?: string; due_date?: string; report_cron?: string }) => api.rawPost("/autonomy/goals/loop", payload),
    onSuccess: async (out) => {
      const preview = asRecord(asRecord(out).plan_preview);
      setPlanPreview(Object.keys(preview).length ? preview : null);
      const gid = str(asRecord(out).goal_id, "");
      if (gid) setSelectedGoalId(gid);
      await queryClient.invalidateQueries({ queryKey: ["goals-list"] });
      await queryClient.invalidateQueries({ queryKey: ["goals-progress"] });
    }
  });

  const runNowMutation = useMutation({
    mutationFn: (goalId: string) => api.rawPost("/autonomy/goals/report_now", { goal_id: goalId }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["goals-progress"] });
    }
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => api.rawDelete(`/goals/${encodeURIComponent(id)}`),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["goals-list"] });
      await queryClient.invalidateQueries({ queryKey: ["goals-progress"] });
      await queryClient.invalidateQueries({ queryKey: ["tasks-manager"] });
    }
  });

  const summary = asRecord(asRecord(progressQ.data).summary);
  const goals = pickRecords(goalsQ.data, "goals");
  const progressItems = pickRecords(progressQ.data, "items");

  const examples = [
    "Build a weekly arXiv dashboard for RL + time series",
    "Ship a working prototype by Friday",
    "Audit the app for security issues and write a fix plan"
  ];

  const resetGoalDraft = (nextAutopilot: boolean, nextDescription = "") => {
    setDescription(nextDescription);
    setDueDate("");
    setGuardrails("");
    setScheduleKey("daily_9");
    setReportCron("0 0 9 * * *");
    setAdvancedOpen(false);
    setAutopilotEnabled(nextAutopilot);
    setError(null);
  };

  const openGoalDialog = (nextAutopilot = true, nextDescription = "") => {
    resetGoalDraft(nextAutopilot, nextDescription);
    setGoalCreateOpen(true);
  };

  const submitGoalDraft = async () => {
    setError(null);
    setPlanPreview(null);
    try {
      const goalText = description.trim();
      if (autopilotEnabled) {
        await autopilotMutation.mutateAsync({
          goal: goalText,
          constraints: guardrails.trim() || undefined,
          due_date: dueDate.trim() || undefined,
          report_cron: reportCron.trim() || undefined
        });
      } else {
        await createMutation.mutateAsync({
          description: goalText,
          due_date: dueDate.trim() || undefined
        });
      }
      setGoalCreateOpen(false);
      resetGoalDraft(true);
    } catch (e) {
      setError(errMessage(e));
    }
  };

  return (
    <Stack spacing={2}>
      <Box className="list-shell">
        <Stack direction={{ xs: "column", md: "row" }} justifyContent="space-between" alignItems={{ xs: "flex-start", md: "center" }} spacing={1} mb={1}>
          <Stack spacing={0.25}>
            <Typography variant="h6">Goals</Typography>
            <Typography variant="caption" color="text.secondary">
              Track outcomes and spin up AI autopilot loops when needed.
            </Typography>
          </Stack>
          <Stack direction="row" spacing={1}>
            <Button size="small" variant="outlined" onClick={() => openGoalDialog(true)}>
              Create Goal
            </Button>
          </Stack>
        </Stack>
      </Box>

      <Grid2 container spacing={2}>
        <Grid2 size={{ xs: 12, md: 3 }}><Box className="list-shell" sx={{ minHeight: 110 }}><Typography variant="caption" color="text.secondary">Autopilot Items</Typography><Typography variant="h5">{num(summary.total)}</Typography><Typography variant="caption" color="text.secondary">Recent tasks tied to goals</Typography></Box></Grid2>
        <Grid2 size={{ xs: 12, md: 3 }}><Box className="list-shell" sx={{ minHeight: 110 }}><Typography variant="caption" color="text.secondary">Completed</Typography><Typography variant="h5">{num(summary.completed)}</Typography></Box></Grid2>
        <Grid2 size={{ xs: 12, md: 3 }}><Box className="list-shell" sx={{ minHeight: 110 }}><Typography variant="caption" color="text.secondary">Pending/Running</Typography><Typography variant="h5">{num(summary.pending_or_running)}</Typography></Box></Grid2>
        <Grid2 size={{ xs: 12, md: 3 }}><Box className="list-shell" sx={{ minHeight: 110 }}><Typography variant="caption" color="text.secondary">Failed</Typography><Typography variant="h5">{num(summary.failed)}</Typography></Box></Grid2>
      </Grid2>

      <Grid2 container spacing={2}>
        <Grid2 size={{ xs: 12, lg: 6 }}>
          <Box className="list-shell">
            <Stack direction="row" justifyContent="space-between" alignItems="center" mb={1}>
              <Typography variant="h6">Goals</Typography>
            </Stack>
            {goalsQ.error ? (
              <Alert severity="error">{errMessage(goalsQ.error)}</Alert>
            ) : goals.length === 0 ? (
              <Typography variant="body2" color="text.secondary">No goals yet.</Typography>
            ) : (
              <Box className="metadata-box" sx={{ maxHeight: 520 }}>
                <Stack spacing={1}>
                  {goals.map((g) => {
                    const id = str(g.id, "");
                    const goalId = str(g.goal_id, "");
                    const hasAutopilot = g.autopilot === true && !!goalId;
                    const isSelected = hasAutopilot && selectedGoalId === goalId;
                    const title = str(g.goal, "").trim() || str(g.description, "Goal").replace(/^Goal:\\s*/i, "");
                    return (
                      <Box key={id} className="action-row">
                        <Stack direction="row" justifyContent="space-between" alignItems="center" spacing={1}>
                          <Button
                            variant={isSelected ? "contained" : "text"}
                            size="small"
                            sx={{ justifyContent: "flex-start", textAlign: "left", flex: 1 }}
                            onClick={() => setSelectedGoalId(hasAutopilot ? goalId : null)}
                          >
                            <Stack alignItems="flex-start" spacing={0.3}>
                              <Stack direction="row" spacing={1} alignItems="center">
                                <Typography variant="body2" fontWeight={700}>{title}</Typography>
                                {hasAutopilot ? <Chip size="small" label="Autopilot" /> : <Chip size="small" label="Manual" variant="outlined" />}
                              </Stack>
                              <Typography variant="caption" color="text.secondary">
                                {str(g.status)}{str(g.due_date) ? ` | due ${str(g.due_date)}` : ""}{str(g.created_at) ? ` | created ${str(g.created_at)}` : ""}
                              </Typography>
                            </Stack>
                          </Button>
                          <Stack direction="row" spacing={1} alignItems="center">
                            {!hasAutopilot ? (
                              <Button
                                size="small"
                                disabled={autopilotMutation.isPending}
                                onClick={async () => {
                                  setError(null);
                                  setPlanPreview(null);
                                  try {
                                    const out = await autopilotMutation.mutateAsync({
                                      goal: title,
                                      due_date: str(g.due_date) || undefined,
                                      constraints: guardrails.trim() || undefined,
                                      report_cron: reportCron.trim() || undefined
                                    });
                                    const newGoalId = str(asRecord(out).goal_id, "");
                                    if (newGoalId) setSelectedGoalId(newGoalId);
                                  } catch (e) {
                                    setError(errMessage(e));
                                  }
                                }}
                              >
                                Start Autopilot
                              </Button>
                            ) : (
                              <Button size="small" onClick={() => setSelectedGoalId(goalId)}>View</Button>
                            )}
                            <Button size="small" color="error" disabled={deleteMutation.isPending} onClick={() => deleteMutation.mutate(id)}>
                              Delete
                            </Button>
                          </Stack>
                        </Stack>
                      </Box>
                    );
                  })}
                </Stack>
              </Box>
            )}
          </Box>
        </Grid2>
        <Grid2 size={{ xs: 12, lg: 6 }}>
          <Box className="list-shell">
            <Stack direction="row" justifyContent="space-between" alignItems="center" mb={1}>
              <Typography variant="h6">{selectedGoalId ? "Autopilot Activity (selected goal)" : "Autopilot Activity (all goals)"}</Typography>
              <Stack direction="row" spacing={1} alignItems="center">
                {selectedGoalId ? (
                  <Button size="small" disabled={runNowMutation.isPending} onClick={() => runNowMutation.mutate(selectedGoalId)}>
                    Run now
                  </Button>
                ) : null}
                {selectedGoalId ? <Button size="small" onClick={() => setSelectedGoalId(null)}>Clear</Button> : null}
              </Stack>
            </Stack>
            {progressQ.error ? (
              <Alert severity="error">{errMessage(progressQ.error)}</Alert>
            ) : progressItems.length === 0 ? (
              <Typography variant="body2" color="text.secondary">No goal-linked items yet.</Typography>
            ) : (
              <Box className="metadata-box" sx={{ maxHeight: 520 }}>
                <Stack spacing={1}>
                  {progressItems.map((it) => {
                    const id = str(it.id, "");
                    const status = str(it.status, "");
                    const statusColor = status.includes("Failed") ? "error" : status.includes("Completed") ? "success" : "warning";
                    return (
                      <Box key={id} className="action-row">
                        <Stack direction="row" justifyContent="space-between" alignItems="center" spacing={1}>
                          <Stack spacing={0.3} sx={{ minWidth: 0 }}>
                            <Typography variant="body2" fontWeight={700} noWrap>{str(it.description, "Task")}</Typography>
                            <Typography variant="caption" color="text.secondary" noWrap>
                              {str(it.action)} | {str(it.created_at)}
                            </Typography>
                          </Stack>
                          <Chip size="small" label={status || "Unknown"} color={statusColor as any} />
                        </Stack>
                      </Box>
                    );
                  })}
                </Stack>
              </Box>
            )}
          </Box>
        </Grid2>
      </Grid2>

      {error ? <Alert severity="error">{error}</Alert> : null}

      <Dialog open={goalCreateOpen} onClose={() => setGoalCreateOpen(false)} maxWidth="md" fullWidth>
        <DialogTitle>Set a Goal</DialogTitle>
        <DialogContent dividers>
          <Stack spacing={1.25}>
            <Stack direction={{ xs: "column", sm: "row" }} justifyContent="space-between" alignItems={{ xs: "flex-start", sm: "center" }}>
              <Typography variant="caption" color="text.secondary">
                Use plain language. Autopilot enables AI planning and scheduled progress loops.
              </Typography>
              <FormControlLabel
                control={<Switch checked={autopilotEnabled} onChange={(e) => setAutopilotEnabled(e.target.checked)} />}
                label="Autopilot"
              />
            </Stack>
            <Grid2 container spacing={1} alignItems="stretch">
              <Grid2 size={{ xs: 12, md: 8 }}>
                <TextField
                  fullWidth
                  label="What do you want to achieve?"
                  placeholder="Describe your goal in one sentence."
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                />
              </Grid2>
              <Grid2 size={{ xs: 12, md: 4 }}>
                <TextField fullWidth label="Due date (optional)" placeholder="YYYY-MM-DD" value={dueDate} onChange={(e) => setDueDate(e.target.value)} />
              </Grid2>
              {autopilotEnabled ? (
                <Grid2 size={{ xs: 12 }}>
                  <TextField
                    fullWidth
                    size="small"
                    label="Guardrails (optional)"
                    placeholder="Example: Ask before deleting files. Keep it under 3 steps. No external posting."
                    value={guardrails}
                    onChange={(e) => setGuardrails(e.target.value)}
                  />
                </Grid2>
              ) : null}
              {autopilotEnabled ? (
                <Grid2 size={{ xs: 12 }}>
                  <Accordion expanded={advancedOpen} onChange={() => setAdvancedOpen((p) => !p)} className="accordion-shell">
                    <AccordionSummary expandIcon={<ExpandMoreIcon />}>
                      <Typography variant="body2" sx={{ fontWeight: 600 }}>Advanced</Typography>
                    </AccordionSummary>
                    <AccordionDetails>
                      <Stack spacing={1}>
                        <TextField
                          fullWidth
                          size="small"
                          select
                          label="Check-in schedule"
                          value={scheduleKey}
                          onChange={(e) => {
                            const next = e.target.value;
                            setScheduleKey(next);
                            let preset: (typeof schedulePresets)[number] | undefined = undefined;
                            for (const p of schedulePresets) {
                              if (p.key === next) {
                                preset = p;
                                break;
                              }
                            }
                            if (preset && preset.cron) setReportCron(preset.cron);
                          }}
                          helperText="When Autopilot is enabled, this schedules a periodic progress report task."
                        >
                          {schedulePresets.map((p) => (
                            <MenuItem key={p.key} value={p.key}>
                              {p.label}
                            </MenuItem>
                          ))}
                        </TextField>
                        {scheduleKey === "custom" ? (
                          <TextField
                            fullWidth
                            size="small"
                            label="Custom cron (6 fields)"
                            value={reportCron}
                            onChange={(e) => setReportCron(e.target.value)}
                            helperText={(() => {
                              for (const p of schedulePresets) {
                                if (p.key === "custom") return p.hint || "";
                              }
                              return "";
                            })()}
                          />
                        ) : (
                          <Typography variant="caption" color="text.secondary">
                            Selected: {scheduleLabel(scheduleKey)} ({reportCron})
                          </Typography>
                        )}
                      </Stack>
                    </AccordionDetails>
                  </Accordion>
                </Grid2>
              ) : null}
            </Grid2>
            <Stack direction="row" spacing={1} flexWrap="wrap" sx={{ opacity: 0.9 }}>
              {examples.map((ex) => (
                <Chip
                  key={ex}
                  size="small"
                  label={ex}
                  onClick={() => setDescription(ex)}
                  variant="outlined"
                  sx={{ mb: 0.5 }}
                />
              ))}
            </Stack>
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setGoalCreateOpen(false)}>Cancel</Button>
          <Button
            variant="contained"
            disabled={!description.trim() || createMutation.isPending || autopilotMutation.isPending}
            onClick={submitGoalDraft}
          >
            {autopilotEnabled ? "Create with AI" : "Save Goal"}
          </Button>
        </DialogActions>
      </Dialog>

      <Dialog open={planPreview != null} onClose={() => setPlanPreview(null)} maxWidth="md" fullWidth>
        <DialogTitle>Autopilot Plan Preview</DialogTitle>
        <DialogContent>
          {planPreview ? (
            <Stack spacing={1.25}>
              {str(planPreview.summary, "").trim() ? (
                <Alert severity="info">{str(planPreview.summary)}</Alert>
              ) : null}
              {Array.isArray(planPreview.steps) && planPreview.steps.length > 0 ? (
                <TableContainer className="table-shell">
                  <Table size="small">
                    <TableHead>
                      <TableRow>
                        <TableCell>Step</TableCell>
                        <TableCell>Action</TableCell>
                        <TableCell>Why</TableCell>
                        <TableCell>Args</TableCell>
                      </TableRow>
                    </TableHead>
                    <TableBody>
                      {(planPreview.steps as unknown[]).slice(0, 25).map((rawStep, idx) => {
                        const step = asRecord(rawStep);
                        const args = asRecord(step.arguments);
                        const argKeys = Object.keys(args);
                        return (
                          <TableRow key={str(step.title, String(idx))}>
                            <TableCell sx={{ maxWidth: 260 }}>
                              <Typography variant="body2" noWrap title={str(step.title, `Step ${idx + 1}`)}>
                                {str(step.title, `Step ${idx + 1}`)}
                              </Typography>
                            </TableCell>
                            <TableCell sx={{ maxWidth: 220 }}>
                              <Typography variant="body2" noWrap title={str(step.action, "-")}>
                                {str(step.action, "-")}
                              </Typography>
                            </TableCell>
                            <TableCell sx={{ maxWidth: 360 }}>
                              <Typography
                                variant="caption"
                                color="text.secondary"
                                sx={{
                                  display: "-webkit-box",
                                  WebkitBoxOrient: "vertical",
                                  WebkitLineClamp: 2,
                                  overflow: "hidden",
                                  wordBreak: "break-word"
                                }}
                                title={str(step.why, "")}
                              >
                                {str(step.why, "-")}
                              </Typography>
                            </TableCell>
                            <TableCell sx={{ maxWidth: 240 }}>
                              <Typography variant="caption" color="text.secondary" noWrap title={argKeys.join(", ")}>
                                {argKeys.length ? argKeys.slice(0, 6).join(", ") : "-"}
                                {argKeys.length > 6 ? ` (+${argKeys.length - 6})` : ""}
                              </Typography>
                            </TableCell>
                          </TableRow>
                        );
                      })}
                    </TableBody>
                  </Table>
                </TableContainer>
              ) : (
                <Typography variant="body2" color="text.secondary">
                  No steps found in plan preview.
                </Typography>
              )}
            </Stack>
          ) : null}
        </DialogContent>
      </Dialog>
    </Stack>
  );
}

function AutonomyManager({ autoRefresh }: { autoRefresh: boolean }) {
  const queryClient = useQueryClient();
  const [tab, setTab] = useState(0);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [autonomyMode, setAutonomyMode] = useState<"off" | "assist" | "auto">("assist");
  const [alwaysAskHighRisk, setAlwaysAskHighRisk] = useState(true);
  const [onlyApprovedSkills, setOnlyApprovedSkills] = useState(true);
  const [quietHoursStart, setQuietHoursStart] = useState("");
  const [quietHoursEnd, setQuietHoursEnd] = useState("");
  const [dailyRunLimit, setDailyRunLimit] = useState("40");
  const [settingsHydrated, setSettingsHydrated] = useState(false);

  const [incidentResult, setIncidentResult] = useState<JsonRecord | null>(null);
  const [rollingBackEventId, setRollingBackEventId] = useState<string | null>(null);

  const [triageLabelsCsv, setTriageLabelsCsv] = useState("Act now, Delegate, Ignore");
  const [triageMessagesJson, setTriageMessagesJson] = useState("");
  const [triageResult, setTriageResult] = useState<JsonRecord | null>(null);

  const [delegateTask, setDelegateTask] = useState("");
  const [delegateContext, setDelegateContext] = useState("");
  const [delegateRequireApproval, setDelegateRequireApproval] = useState(false);
  const [delegateResult, setDelegateResult] = useState<JsonRecord | null>(null);

  const [selectedSessionId, setSelectedSessionId] = useState("");
  const [sessionResponse, setSessionResponse] = useState("");
  const [browserRespondResult, setBrowserRespondResult] = useState<JsonRecord | null>(null);

  const settingsQ = useQuery({
    queryKey: ["autonomy-settings"],
    queryFn: () => api.rawGet("/autonomy/settings")
  });
  const briefingQ = useQuery({
    queryKey: ["autonomy-briefing"],
    queryFn: () => api.rawGet("/autonomy/briefing"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const notificationsQ = useQuery({
    queryKey: ["autonomy-unread-notifications"],
    queryFn: () => api.rawGet("/notifications?unread=true&limit=120"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const incidentsQ = useQuery({
    queryKey: ["autonomy-incidents-live"],
    queryFn: () => api.rawGet("/autonomy/incidents/live"),
    enabled: showAdvanced,
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const timelineQ = useQuery({
    queryKey: ["autonomy-timeline"],
    queryFn: () => api.rawGet("/autonomy/timeline?limit=120"),
    enabled: showAdvanced && SHOW_EXPERIMENTAL_AUTONOMY_TOOLS,
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const browserSessionsQ = useQuery({
    queryKey: ["autonomy-browser-sessions"],
    queryFn: () => api.rawGet("/browser/sessions"),
    enabled: showAdvanced,
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const browserStatusQ = useQuery({
    queryKey: ["autonomy-browser-session-status", selectedSessionId],
    queryFn: () => api.rawGet(`/browser/sessions/${encodeURIComponent(selectedSessionId)}/status`),
    enabled: showAdvanced && !!selectedSessionId,
    refetchInterval: autoRefresh && !!selectedSessionId ? REFRESH_MS : false
  });

  const saveAutonomySettingsMutation = useMutation({
    mutationFn: (payload: JsonRecord) => api.rawPost("/autonomy/settings", payload),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["autonomy-settings"] });
      await queryClient.invalidateQueries({ queryKey: ["autonomy-briefing"] });
      await queryClient.invalidateQueries({ queryKey: ["autonomy-unread-notifications"] });
    }
  });
  const executeIncidentMutation = useMutation({
    mutationFn: (id: string) => api.rawPost(`/autonomy/incidents/${encodeURIComponent(id)}/execute`, {}),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["autonomy-incidents-live"] });
      await queryClient.invalidateQueries({ queryKey: ["autonomy-timeline"] });
      await queryClient.invalidateQueries({ queryKey: ["tasks-manager"] });
    }
  });
  const rollbackMutation = useMutation({
    mutationFn: (payload: { event_id: string; operation?: string }) => api.rawPost("/autonomy/timeline/rollback", payload),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["autonomy-timeline"] });
      await queryClient.invalidateQueries({ queryKey: ["tasks-manager"] });
      await queryClient.invalidateQueries({ queryKey: ["autonomy-unread-notifications"] });
      await queryClient.invalidateQueries({ queryKey: ["notifications"] });
      await queryClient.invalidateQueries({ queryKey: ["notifications-count"] });
    }
  });
  const triageMutation = useMutation({
    mutationFn: (payload: { labels?: string[]; messages: unknown[] }) => api.rawPost("/autonomy/inbox/triage", payload)
  });
  const delegateMutation = useMutation({
    mutationFn: (payload: { task: string; context?: string; require_approval?: boolean }) => api.rawPost("/autonomy/delegate", payload),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["swarm-delegations"] });
      await queryClient.invalidateQueries({ queryKey: ["tasks-manager"] });
    }
  });
  const browserRespondMutation = useMutation({
    mutationFn: (payload: { id: string; response: string }) =>
      api.rawPost(`/browser/sessions/${encodeURIComponent(payload.id)}/respond`, { response: payload.response }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["autonomy-browser-sessions"] });
      await queryClient.invalidateQueries({ queryKey: ["autonomy-browser-session-status", selectedSessionId] });
    }
  });

  const incidents = pickRecords(incidentsQ.data, "incidents");
  const timelineEvents = pickRecords(timelineQ.data, "events");
  const triageRows = pickRecords(triageResult, "triage");
  const browserSessions = pickRecords(browserSessionsQ.data, "sessions");
  const browserStatus = asRecord(browserStatusQ.data);

  function severityChipColor(sev: string): "error" | "warning" | "info" | "success" | "default" {
    const s = (sev || "").toLowerCase();
    if (s === "critical" || s === "high" || s === "error") return "error";
    if (s === "medium" || s === "warn" || s === "warning") return "warning";
    if (s === "low") return "info";
    if (s === "ok" || s === "info") return "success";
    return "default";
  }

  function parseCsv(value: string): string[] {
    return value
      .split(",")
      .map((x) => x.trim())
      .filter((x) => x.length > 0);
  }

  function parseTriageMessages(value: string): unknown[] {
    const trimmed = value.trim();
    if (!trimmed) return [];
    const parsed: unknown = JSON.parse(trimmed);
    if (!Array.isArray(parsed)) {
      throw new Error("Messages JSON must be an array.");
    }
    return parsed;
  }

  function effectiveRollbackOperation(operation: string, status: string): string {
    if (operation !== "toggle_notification_read") return operation;
    return status.toLowerCase() === "read" ? "mark_unread" : "mark_read";
  }

  function rollbackLabel(operation: string): string {
    const op = (operation || "").toLowerCase();
    if (op === "cancel_task") return "Cancel task";
    if (op === "cancel_watcher") return "Cancel watcher";
    if (op === "mark_unread") return "Mark unread";
    if (op === "mark_read") return "Mark read";
    if (op === "toggle_notification_read") return "Toggle read";
    return "Rollback";
  }

  const settingsRecord = asRecord(asRecord(settingsQ.data).settings);
  const briefingRecord = asRecord(briefingQ.data);
  const queueSummary = asRecord(asRecord(briefingRecord.trust_summary).queue);
  const topRisks = pickRecords(briefingRecord, "top_risks");
  const unreadNotifications = pickRecords(notificationsQ.data, "notifications");
  const awaitingApprovals = num(queueSummary.awaiting_approval, 0);
  const missingInputs = unreadNotifications.filter((row) => {
    const source = str(row.source, "").toLowerCase();
    const title = str(row.title, "").toLowerCase();
    const body = str(row.body, "").toLowerCase();
    return (
      source === "workflow_inputs" ||
      title.includes("missing input") ||
      body.includes("missing input") ||
      title.includes("required input") ||
      body.includes("required input")
    );
  }).length;
  const modeIndicator = autonomyMode === "auto" ? "Auto" : autonomyMode === "assist" ? "Assist" : "Off";
  const timelineTabIndex = 1;
  const triageTabIndex = 2;
  const delegateTabIndex = SHOW_EXPERIMENTAL_AUTONOMY_TOOLS ? 3 : 1;
  const browserTabIndex = SHOW_EXPERIMENTAL_AUTONOMY_TOOLS ? 4 : 2;
  const waitingStatusLine =
    awaitingApprovals === 0 && missingInputs === 0
      ? `Mode: ${modeIndicator} | You're all set. Nothing is waiting on you.`
      : `Mode: ${modeIndicator} | Waiting on you: ${awaitingApprovals} approval${awaitingApprovals === 1 ? "" : "s"}, ${missingInputs} required input${missingInputs === 1 ? "" : "s"}`;
  const modePlainHint =
    autonomyMode === "off"
      ? "You start everything manually."
      : autonomyMode === "assist"
      ? "Agent prepares work and asks before sensitive actions."
      : "Agent runs allowed work automatically and only asks when required.";
  const configuredModeRaw = str(settingsRecord.autonomy_mode, "assist").toLowerCase();
  const configuredMode: "off" | "assist" | "auto" =
    configuredModeRaw === "off" || configuredModeRaw === "auto" || configuredModeRaw === "assist"
      ? configuredModeRaw
      : "assist";
  const configuredAlwaysAskHighRisk = Boolean(settingsRecord.always_ask_high_risk ?? true);
  const configuredOnlyApprovedSkills = Boolean(settingsRecord.only_approved_skills ?? true);
  const configuredQuietHoursStart = str(settingsRecord.quiet_hours_start, "").trim();
  const configuredQuietHoursEnd = str(settingsRecord.quiet_hours_end, "").trim();
  const configuredDailyRunLimit =
    typeof settingsRecord.daily_run_limit === "number" && Number.isFinite(settingsRecord.daily_run_limit)
      ? Math.round(settingsRecord.daily_run_limit)
      : null;
  const normalizedQuietHoursStart = quietHoursStart.trim();
  const normalizedQuietHoursEnd = quietHoursEnd.trim();
  const normalizedLimitText = dailyRunLimit.trim();
  let parsedLimitForUi: number | null = null;
  let dailyRunLimitInvalid = false;
  if (normalizedLimitText.length > 0) {
    const n = Number(normalizedLimitText);
    if (!Number.isFinite(n) || n < 1) {
      dailyRunLimitInvalid = true;
    } else {
      parsedLimitForUi = Math.round(n);
    }
  }
  const guardrailsDirty =
    settingsHydrated &&
    (autonomyMode !== configuredMode ||
      alwaysAskHighRisk !== configuredAlwaysAskHighRisk ||
      onlyApprovedSkills !== configuredOnlyApprovedSkills ||
      normalizedQuietHoursStart !== configuredQuietHoursStart ||
      normalizedQuietHoursEnd !== configuredQuietHoursEnd ||
      parsedLimitForUi !== configuredDailyRunLimit);

  function openSettingsTab(tabName: string) {
    const nextPath = "/ui/settings";
    const nextSearch = `?settings_tab=${encodeURIComponent(tabName)}`;
    const nextUrl = `${nextPath}${nextSearch}`;
    const current = `${window.location.pathname}${window.location.search}`;
    if (current !== nextUrl) {
      window.history.pushState(null, "", nextUrl);
      window.dispatchEvent(new PopStateEvent("popstate"));
    }
  }

  function recommendedTabForRisk(risk: JsonRecord): string {
    const bag = `${str(risk.type, "")} ${str(risk.title, "")} ${str(risk.detail, "")}`.toLowerCase();
    if (bag.includes("auth") || bag.includes("security")) return "security";
    return "system";
  }

  useEffect(() => {
    if (settingsHydrated) return;
    if (!Object.keys(settingsRecord).length) return;
    const rawMode = str(settingsRecord.autonomy_mode, "assist").toLowerCase();
    if (rawMode === "off" || rawMode === "auto" || rawMode === "assist") {
      setAutonomyMode(rawMode);
    } else {
      setAutonomyMode("assist");
    }
    setAlwaysAskHighRisk(Boolean(settingsRecord.always_ask_high_risk ?? true));
    setOnlyApprovedSkills(Boolean(settingsRecord.only_approved_skills ?? true));
    setQuietHoursStart(str(settingsRecord.quiet_hours_start, ""));
    setQuietHoursEnd(str(settingsRecord.quiet_hours_end, ""));
    const configuredLimit = settingsRecord.daily_run_limit;
    if (typeof configuredLimit === "number" && Number.isFinite(configuredLimit)) {
      setDailyRunLimit(String(configuredLimit));
    } else {
      setDailyRunLimit("");
    }
    setSettingsHydrated(true);
  }, [settingsHydrated, settingsRecord]);

  useEffect(() => {
    if (!showAdvanced) return;
    const maxAllowedTab = SHOW_EXPERIMENTAL_AUTONOMY_TOOLS ? 4 : 2;
    if (tab > maxAllowedTab) {
      setTab(0);
    }
  }, [showAdvanced, tab]);

  async function saveBeginnerAutonomySettings(modeOverride?: "off" | "assist" | "auto") {
    setError(null);
    setSuccess(null);
    const selectedMode = modeOverride ?? autonomyMode;
    const normalizedLimit = dailyRunLimit.trim();
    let parsedLimit: number | null = null;
    if (normalizedLimit.length > 0) {
      const n = Number(normalizedLimit);
      if (!Number.isFinite(n) || n < 1) {
        setError("Daily run limit must be a positive number.");
        return;
      }
      parsedLimit = Math.round(n);
    }
    try {
      await saveAutonomySettingsMutation.mutateAsync({
        autonomy_mode: selectedMode,
        always_ask_high_risk: alwaysAskHighRisk,
        only_approved_skills: onlyApprovedSkills,
        quiet_hours_start: quietHoursStart.trim() || null,
        quiet_hours_end: quietHoursEnd.trim() || null,
        daily_run_limit: parsedLimit
      });
      setSuccess("Autonomy settings saved.");
    } catch (e) {
      setError(errMessage(e));
    }
  }

  return (
    <Stack spacing={2}>
      <Box className="list-shell">
        <Stack spacing={1.25}>
          <Typography variant="h6">Automation Mode</Typography>
          <Typography variant="caption" color="text.secondary">
            Choose how hands-off you want this agent to be. Anything that needs your decision appears here.
          </Typography>
          <Alert severity="info" sx={{ py: 0.75 }}>
            {waitingStatusLine}
          </Alert>
          <Typography variant="body2" color="text.secondary">
            {modePlainHint}
          </Typography>
          <Stack direction={{ xs: "column", md: "row" }} spacing={1}>
            <Button
              variant={autonomyMode === "off" ? "contained" : "outlined"}
              onClick={async () => {
                setAutonomyMode("off");
                await saveBeginnerAutonomySettings("off");
              }}
              disabled={saveAutonomySettingsMutation.isPending}
            >
              Off
            </Button>
            <Button
              variant={autonomyMode === "assist" ? "contained" : "outlined"}
              onClick={async () => {
                setAutonomyMode("assist");
                await saveBeginnerAutonomySettings("assist");
              }}
              disabled={saveAutonomySettingsMutation.isPending}
            >
              Assist (Recommended)
            </Button>
            <Button
              variant={autonomyMode === "auto" ? "contained" : "outlined"}
              onClick={async () => {
                setAutonomyMode("auto");
                await saveBeginnerAutonomySettings("auto");
              }}
              disabled={saveAutonomySettingsMutation.isPending}
            >
              Auto
            </Button>
          </Stack>
          <Box component="ul" sx={{ m: 0, pl: 2, color: "text.secondary" }}>
            <Typography component="li" variant="caption" color="text.secondary">
              Off: You review and start every run manually.
            </Typography>
            <Typography component="li" variant="caption" color="text.secondary">
              Assist (recommended): Agent drafts work first, then asks before sensitive steps.
            </Typography>
            <Typography component="li" variant="caption" color="text.secondary">
              Auto: Agent proceeds end-to-end within your safety limits.
            </Typography>
            <Typography component="li" variant="caption" color="text.secondary">
              Tip: Keep Assist on until you're comfortable with Auto.
            </Typography>
          </Box>
          {!showAdvanced ? (
            <Stack spacing={1}>
              <Alert severity="success" sx={{ py: 0.75 }}>
                Beginner safety defaults are active: ask before risky actions, approved skills only, and daily run cap.
              </Alert>
              <Stack direction="row" spacing={1} flexWrap="wrap" useFlexGap>
                <Button
                  color="warning"
                  variant="outlined"
                  onClick={async () => {
                    setAutonomyMode("off");
                    await saveBeginnerAutonomySettings("off");
                  }}
                  disabled={saveAutonomySettingsMutation.isPending}
                >
                  Turn Off
                </Button>
                  <Button size="small" onClick={() => { setTab(0); setShowAdvanced(true); }}>
                    Show developer mode
                  </Button>
              </Stack>
            </Stack>
          ) : (
            <Grid2 container spacing={1}>
              <Grid2 size={{ xs: 12 }}>
                <Alert severity="info" sx={{ py: 0.75 }}>
                  Developer mode: full controls for safety policies, limits, and advanced automation tools.
                </Alert>
              </Grid2>
              <Grid2 size={{ xs: 12, md: 6 }}>
                <FormControlLabel
                  control={
                    <Switch
                      checked={alwaysAskHighRisk}
                      onChange={(e) => setAlwaysAskHighRisk(e.target.checked)}
                    />
                  }
                  label="Ask me before risky actions"
                />
              </Grid2>
              <Grid2 size={{ xs: 12, md: 6 }}>
                <FormControlLabel
                  control={
                    <Switch
                      checked={onlyApprovedSkills}
                      onChange={(e) => setOnlyApprovedSkills(e.target.checked)}
                    />
                  }
                  label="Use only approved skills"
                />
              </Grid2>
              <Grid2 size={{ xs: 12, md: 4 }}>
                <TextField
                  fullWidth
                  size="small"
                  type="time"
                  label="Quiet hours start (local)"
                  value={quietHoursStart}
                  onChange={(e) => setQuietHoursStart(e.target.value)}
                  InputLabelProps={{ shrink: true }}
                  helperText="Agent avoids starting new runs after this time."
                />
              </Grid2>
              <Grid2 size={{ xs: 12, md: 4 }}>
                <TextField
                  fullWidth
                  size="small"
                  type="time"
                  label="Quiet hours end (local)"
                  value={quietHoursEnd}
                  onChange={(e) => setQuietHoursEnd(e.target.value)}
                  InputLabelProps={{ shrink: true }}
                  helperText="Agent resumes normal runs after this time."
                />
              </Grid2>
              <Grid2 size={{ xs: 12, md: 4 }}>
                <TextField
                  fullWidth
                  size="small"
                  type="number"
                  label="Daily run limit"
                  value={dailyRunLimit}
                  onChange={(e) => setDailyRunLimit(e.target.value)}
                  inputProps={{ min: 1, max: 1000 }}
                  error={dailyRunLimitInvalid}
                  helperText={
                    dailyRunLimitInvalid
                      ? "Enter a positive number (1 or more), or leave blank."
                      : "Safety cap for total runs each day. Leave blank for no cap."
                  }
                />
              </Grid2>
              <Grid2 size={{ xs: 12 }}>
                <Stack direction="row" spacing={1} flexWrap="wrap" useFlexGap>
                  <Button
                    variant="contained"
                    onClick={() => saveBeginnerAutonomySettings()}
                    disabled={
                      saveAutonomySettingsMutation.isPending ||
                      settingsQ.isFetching ||
                      !guardrailsDirty ||
                      dailyRunLimitInvalid
                    }
                  >
                    {saveAutonomySettingsMutation.isPending ? "Saving..." : "Save Safety Settings"}
                  </Button>
                  <Button
                    color="warning"
                    variant="outlined"
                    onClick={async () => {
                      setAutonomyMode("off");
                      await saveBeginnerAutonomySettings("off");
                    }}
                    disabled={saveAutonomySettingsMutation.isPending}
                  >
                    Turn Off
                  </Button>
                  <Button size="small" onClick={() => { setShowAdvanced(false); setTab(0); }}>
                    Hide developer mode
                  </Button>
                </Stack>
              </Grid2>
            </Grid2>
          )}
        </Stack>
      </Box>

      {topRisks.length > 0 ? (
        <Box className="list-shell">
          <Typography variant="subtitle2" mb={0.75}>Needs Your Attention</Typography>
          <Stack spacing={0.75}>
            {topRisks.slice(0, 4).map((risk, idx) => (
              <Stack
                key={`risk-${idx}`}
                direction={{ xs: "column", sm: "row" }}
                spacing={1}
                alignItems={{ xs: "flex-start", sm: "center" }}
                justifyContent="space-between"
                className="action-row"
              >
                <Typography variant="body2" color="text.secondary">
                  {str(risk.title, "Risk")} - {str(risk.detail, "")}
                </Typography>
                <Button
                  size="small"
                  variant="outlined"
                  onClick={() => openSettingsTab(recommendedTabForRisk(risk))}
                >
                  Review
                </Button>
              </Stack>
            ))}
          </Stack>
        </Box>
      ) : null}

      {showAdvanced ? (
        <Box className="list-shell">
          <Typography variant="subtitle2" mb={0.5}>Developer Tools</Typography>
          <Tabs
            value={tab}
            onChange={(_, value) => setTab(Number(value) || 0)}
            variant="scrollable"
            scrollButtons="auto"
            allowScrollButtonsMobile
            sx={{ mt: 1 }}
          >
            <Tab label="Live Incidents" value={0} />
            {SHOW_EXPERIMENTAL_AUTONOMY_TOOLS ? <Tab label="Timeline & Rollback" value={timelineTabIndex} /> : null}
            {SHOW_EXPERIMENTAL_AUTONOMY_TOOLS ? <Tab label="Inbox Triage" value={triageTabIndex} /> : null}
            <Tab label="Delegate" value={delegateTabIndex} />
            <Tab label="Browser Sessions" value={browserTabIndex} />
          </Tabs>
          {!SHOW_EXPERIMENTAL_AUTONOMY_TOOLS ? (
            <Typography variant="caption" color="text.secondary" sx={{ mt: 0.75, display: "block" }}>
              Timeline rollback and inbox triage are hidden by default to keep this view focused.
            </Typography>
          ) : null}
        </Box>
      ) : null}

      {showAdvanced && tab === 0 ? (
        <Box className="list-shell">
          <Stack direction="row" justifyContent="space-between" alignItems="center" mb={1}>
            <Typography variant="h6">Live Incidents</Typography>
            <Button size="small" onClick={() => queryClient.invalidateQueries({ queryKey: ["autonomy-incidents-live"] })}>
              Refresh
            </Button>
          </Stack>
          {incidentsQ.error ? (
            <Alert severity="error">{errMessage(incidentsQ.error)}</Alert>
          ) : incidents.length === 0 ? (
            <Typography variant="body2" color="text.secondary">No incidents right now.</Typography>
          ) : (
            <TableContainer className="table-shell">
              <Table size="small">
                <TableHead>
                  <TableRow>
                    <TableCell>Severity</TableCell>
                    <TableCell>Title</TableCell>
                    <TableCell>Detail</TableCell>
                    <TableCell>ID</TableCell>
                    <TableCell align="right">Ops</TableCell>
                  </TableRow>
                </TableHead>
                <TableBody>
                  {incidents.map((incident, idx) => {
                    const id = str(incident.id, `incident-${idx}`);
                    return (
                      <TableRow key={id}>
                        <TableCell>
                          <Chip size="small" label={str(incident.severity, "-")} color={severityChipColor(str(incident.severity, ""))} />
                        </TableCell>
                        <TableCell sx={{ maxWidth: 260 }}>
                          <Typography variant="body2" noWrap title={str(incident.title, "-")}>
                            {str(incident.title, "-")}
                          </Typography>
                        </TableCell>
                        <TableCell sx={{ maxWidth: 420 }}>
                          <Typography variant="body2" noWrap title={str(incident.detail, "-")}>
                            {str(incident.detail, "-")}
                          </Typography>
                        </TableCell>
                        <TableCell sx={{ maxWidth: 180 }}>
                          <Typography variant="caption" color="text.secondary" noWrap title={id}>
                            {id}
                          </Typography>
                        </TableCell>
                        <TableCell align="right">
                          <RowOpsMenu
                            actions={[
                              {
                                label: "Run Playbook",
                                disabled: executeIncidentMutation.isPending,
                                onClick: async () => {
                                  setError(null);
                                  setSuccess(null);
                                  setIncidentResult(null);
                                  try {
                                    const out = asRecord(await executeIncidentMutation.mutateAsync(id));
                                    setIncidentResult(out);
                                    setSuccess("Incident playbook executed.");
                                  } catch (e) {
                                    setError(errMessage(e));
                                  }
                                }
                              }
                            ]}
                            ariaLabel="Incident options"
                          />
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </TableContainer>
          )}
          {incidentResult ? (
            <Box sx={{ mt: 1 }}>
              <KeyValuePanel title="Last playbook result" data={incidentResult} />
            </Box>
          ) : null}
        </Box>
      ) : null}

      {showAdvanced && SHOW_EXPERIMENTAL_AUTONOMY_TOOLS && tab === timelineTabIndex ? (
        <Box className="list-shell">
          <Stack direction="row" justifyContent="space-between" alignItems="center" mb={1}>
            <Typography variant="h6">Timeline & Rollback</Typography>
            <Button size="small" onClick={() => queryClient.invalidateQueries({ queryKey: ["autonomy-timeline"] })}>
              Refresh
            </Button>
          </Stack>
          {timelineQ.error ? (
            <Alert severity="error">{errMessage(timelineQ.error)}</Alert>
          ) : timelineEvents.length === 0 ? (
            <Typography variant="body2" color="text.secondary">No timeline events yet.</Typography>
          ) : (
            <TableContainer className="table-shell">
              <Table size="small">
                <TableHead>
                  <TableRow>
                    <TableCell>Time</TableCell>
                    <TableCell>Source</TableCell>
                    <TableCell>Title</TableCell>
                    <TableCell>Status</TableCell>
                    <TableCell>Detail</TableCell>
                    <TableCell align="right">Ops</TableCell>
                  </TableRow>
                </TableHead>
                <TableBody>
                  {timelineEvents.map((event, idx) => {
                    const eventId = str(event.id, `event-${idx}`);
                    const status = str(event.status, "");
                    const rollback = asRecord(event.rollback);
                    const operation = str(rollback.operation, "");
                    const effectiveOp = effectiveRollbackOperation(operation, status);
                    const canRollback = !!operation && operation !== "none";
                    return (
                      <TableRow key={eventId}>
                        <TableCell sx={{ whiteSpace: "nowrap" }}>{str(event.timestamp, "-")}</TableCell>
                        <TableCell>{str(event.source, "-")}</TableCell>
                        <TableCell sx={{ maxWidth: 280 }}>
                          <Typography variant="body2" noWrap title={str(event.title, "-")}>
                            {str(event.title, "-")}
                          </Typography>
                        </TableCell>
                        <TableCell>{status || "-"}</TableCell>
                        <TableCell sx={{ maxWidth: 360 }}>
                          <Typography variant="caption" color="text.secondary" noWrap title={str(event.detail, "-")}>
                            {str(event.detail, "-")}
                          </Typography>
                        </TableCell>
                        <TableCell align="right">
                          {canRollback ? (
                            <RowOpsMenu
                              actions={[
                                {
                                  label: rollingBackEventId === eventId ? "Applying..." : rollbackLabel(effectiveOp || operation),
                                  disabled: rollbackMutation.isPending || rollingBackEventId === eventId,
                                  onClick: async () => {
                                    setError(null);
                                    setSuccess(null);
                                    setRollingBackEventId(eventId);
                                    try {
                                      await rollbackMutation.mutateAsync({
                                        event_id: eventId,
                                        operation: effectiveOp || undefined
                                      });
                                      setSuccess(`Rollback applied: ${rollbackLabel(effectiveOp || operation)}.`);
                                    } catch (e) {
                                      setError(errMessage(e));
                                    } finally {
                                      setRollingBackEventId(null);
                                    }
                                  }
                                }
                              ]}
                              ariaLabel="Timeline event options"
                            />
                          ) : (
                            <Typography variant="caption" color="text.secondary">n/a</Typography>
                          )}
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </TableContainer>
          )}
        </Box>
      ) : null}

      {showAdvanced && SHOW_EXPERIMENTAL_AUTONOMY_TOOLS && tab === triageTabIndex ? (
        <Stack spacing={2}>
          <Box className="list-shell">
            <Typography variant="h6" mb={1}>Inbox Triage</Typography>
            <Grid2 container spacing={1}>
              <Grid2 size={{ xs: 12 }}>
                <TextField
                  fullWidth
                  size="small"
                  label="Labels"
                  value={triageLabelsCsv}
                  onChange={(e) => setTriageLabelsCsv(e.target.value)}
                  helperText="Comma-separated labels. Default: Act now, Delegate, Ignore"
                />
              </Grid2>
              <Grid2 size={{ xs: 12 }}>
                <TextField
                  fullWidth
                  size="small"
                  multiline
                  minRows={5}
                  label="Messages JSON (optional)"
                  value={triageMessagesJson}
                  onChange={(e) => setTriageMessagesJson(e.target.value)}
                  placeholder='[{"id":"m1","from":"boss@company.com","subject":"Budget","snippet":"Need approval today"}]'
                  helperText="Leave empty to triage recent notifications automatically."
                />
              </Grid2>
              <Grid2 size={{ xs: 12 }}>
                <Button
                  variant="contained"
                  disabled={triageMutation.isPending}
                  onClick={async () => {
                    setError(null);
                    setSuccess(null);
                    setTriageResult(null);
                    try {
                      const out = asRecord(
                        await triageMutation.mutateAsync({
                          labels: parseCsv(triageLabelsCsv),
                          messages: parseTriageMessages(triageMessagesJson)
                        })
                      );
                      setTriageResult(out);
                      setSuccess("Inbox triage complete.");
                    } catch (e) {
                      setError(errMessage(e));
                    }
                  }}
                >
                  {triageMutation.isPending ? "Running..." : "Run Triage"}
                </Button>
              </Grid2>
            </Grid2>
          </Box>

          <Box className="list-shell">
            <Typography variant="h6" mb={1}>Triage Results</Typography>
            {triageRows.length === 0 ? (
              <Typography variant="body2" color="text.secondary">Run triage to see classification and draft replies.</Typography>
            ) : (
              <TableContainer className="table-shell">
                <Table size="small">
                  <TableHead>
                    <TableRow>
                      <TableCell>Message</TableCell>
                      <TableCell>Label</TableCell>
                      <TableCell>Reason</TableCell>
                      <TableCell>Draft Reply</TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {triageRows.map((row, idx) => (
                      <TableRow key={str(row.message_id, `triage-${idx}`)}>
                        <TableCell sx={{ maxWidth: 180 }}>
                          <Typography variant="caption" color="text.secondary" noWrap title={str(row.message_id, "-")}>
                            {str(row.message_id, "-")}
                          </Typography>
                        </TableCell>
                        <TableCell>
                          <Chip size="small" label={str(row.label, "-")} variant="outlined" />
                        </TableCell>
                        <TableCell sx={{ maxWidth: 320 }}>
                          <Typography variant="body2" noWrap title={str(row.reason, "-")}>
                            {str(row.reason, "-")}
                          </Typography>
                        </TableCell>
                        <TableCell sx={{ maxWidth: 480 }}>
                          <Typography variant="body2" noWrap title={str(row.draft_reply, "-")}>
                            {str(row.draft_reply, "-")}
                          </Typography>
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </TableContainer>
            )}
          </Box>
        </Stack>
      ) : null}

      {showAdvanced && tab === delegateTabIndex ? (
        <Stack spacing={2}>
          <Box className="list-shell">
            <Typography variant="h6" mb={1}>One-Click Delegate</Typography>
            <Grid2 container spacing={1}>
              <Grid2 size={{ xs: 12 }}>
                <TextField
                  fullWidth
                  label="Task"
                  value={delegateTask}
                  onChange={(e) => setDelegateTask(e.target.value)}
                  placeholder="Example: Analyze top customer complaints and suggest fixes."
                />
              </Grid2>
              <Grid2 size={{ xs: 12 }}>
                <TextField
                  fullWidth
                  multiline
                  minRows={4}
                  label="Context (optional)"
                  value={delegateContext}
                  onChange={(e) => setDelegateContext(e.target.value)}
                  placeholder="Constraints, links, preferred style, deadlines..."
                />
              </Grid2>
              <Grid2 size={{ xs: 12 }}>
                <FormControlLabel
                  control={
                    <Switch
                      checked={delegateRequireApproval}
                      onChange={(e) => setDelegateRequireApproval(e.target.checked)}
                    />
                  }
                  label="Require approval before delegation"
                />
              </Grid2>
              <Grid2 size={{ xs: 12 }}>
                <Button
                  variant="contained"
                  disabled={!delegateTask.trim() || delegateMutation.isPending}
                  onClick={async () => {
                    setError(null);
                    setSuccess(null);
                    setDelegateResult(null);
                    try {
                      const out = asRecord(
                        await delegateMutation.mutateAsync({
                          task: delegateTask.trim(),
                          context: delegateContext.trim() || undefined,
                          require_approval: delegateRequireApproval
                        })
                      );
                      setDelegateResult(out);
                      setSuccess("Delegation submitted.");
                    } catch (e) {
                      setError(errMessage(e));
                    }
                  }}
                >
                  {delegateMutation.isPending ? "Submitting..." : "Delegate"}
                </Button>
              </Grid2>
            </Grid2>
          </Box>

          {delegateResult ? (
            <Box className="list-shell">
              <KeyValuePanel title="Delegation result" data={delegateResult} />
              {isRecord(delegateResult.result) ? (
                <Box sx={{ mt: 1 }}>
                  <KeyValuePanel title="Result detail" data={asRecord(delegateResult.result)} />
                </Box>
              ) : null}
            </Box>
          ) : null}
        </Stack>
      ) : null}

      {showAdvanced && tab === browserTabIndex ? (
        <Stack spacing={2}>
          <Box className="list-shell">
            <Stack direction="row" justifyContent="space-between" alignItems="center" mb={1}>
              <Typography variant="h6">Browser Sessions</Typography>
              <Button size="small" onClick={() => queryClient.invalidateQueries({ queryKey: ["autonomy-browser-sessions"] })}>
                Refresh
              </Button>
            </Stack>
            {browserSessionsQ.error ? (
              <Alert severity="error">{errMessage(browserSessionsQ.error)}</Alert>
            ) : browserSessions.length === 0 ? (
              <Typography variant="body2" color="text.secondary">No active browser sessions.</Typography>
            ) : (
              <TableContainer className="table-shell">
                <Table size="small">
                  <TableHead>
                    <TableRow>
                      <TableCell>ID</TableCell>
                      <TableCell>Task</TableCell>
                      <TableCell>Status</TableCell>
                      <TableCell align="right">Ops</TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {browserSessions.map((session, idx) => {
                      const id = str(session.id, `session-${idx}`);
                      return (
                        <TableRow key={id}>
                          <TableCell sx={{ maxWidth: 180 }}>
                            <Typography variant="caption" color="text.secondary" noWrap title={id}>
                              {id}
                            </Typography>
                          </TableCell>
                          <TableCell sx={{ maxWidth: 360 }}>
                            <Typography variant="body2" noWrap title={str(session.task, "-")}>
                              {str(session.task, "-")}
                            </Typography>
                          </TableCell>
                          <TableCell sx={{ maxWidth: 260 }}>
                            <Typography variant="body2" noWrap title={str(session.status, "-")}>
                              {str(session.status, "-")}
                            </Typography>
                          </TableCell>
                          <TableCell align="right">
                            <RowOpsMenu
                              actions={[
                                {
                                  label: "Select",
                                  onClick: () => {
                                    setSelectedSessionId(id);
                                    setSessionResponse("");
                                  }
                                },
                                {
                                  label: "Status",
                                  onClick: async () => {
                                    if (selectedSessionId !== id) {
                                      setSelectedSessionId(id);
                                      return;
                                    }
                                    await browserStatusQ.refetch();
                                  }
                                }
                              ]}
                              ariaLabel="Browser session options"
                            />
                          </TableCell>
                        </TableRow>
                      );
                    })}
                  </TableBody>
                </Table>
              </TableContainer>
            )}
          </Box>

          <Box className="list-shell">
            <Typography variant="h6" mb={1}>Respond to Session</Typography>
            <Grid2 container spacing={1}>
              <Grid2 size={{ xs: 12, md: 8 }}>
                <TextField
                  fullWidth
                  size="small"
                  label="Selected session ID"
                  value={selectedSessionId}
                  onChange={(e) => setSelectedSessionId(e.target.value)}
                />
              </Grid2>
              <Grid2 size={{ xs: 12, md: 4 }}>
                <Button
                  fullWidth
                  variant="outlined"
                  disabled={!selectedSessionId.trim() || browserStatusQ.isFetching}
                  onClick={() => browserStatusQ.refetch()}
                >
                  {browserStatusQ.isFetching ? "Checking..." : "Check Status"}
                </Button>
              </Grid2>
              <Grid2 size={{ xs: 12 }}>
                <Typography variant="body2" color="text.secondary">
                  Current status: {str(browserStatus.status, str(browserStatus.error, selectedSessionId ? "unknown" : "select a session"))}
                </Typography>
              </Grid2>
              <Grid2 size={{ xs: 12 }}>
                <TextField
                  fullWidth
                  multiline
                  minRows={3}
                  label="Response"
                  value={sessionResponse}
                  onChange={(e) => setSessionResponse(e.target.value)}
                  placeholder="Example: Continue with the first result and summarize key points."
                />
              </Grid2>
              <Grid2 size={{ xs: 12 }}>
                <Button
                  variant="contained"
                  disabled={!selectedSessionId.trim() || !sessionResponse.trim() || browserRespondMutation.isPending}
                  onClick={async () => {
                    setError(null);
                    setSuccess(null);
                    setBrowserRespondResult(null);
                    try {
                      const out = asRecord(
                        await browserRespondMutation.mutateAsync({
                          id: selectedSessionId.trim(),
                          response: sessionResponse.trim()
                        })
                      );
                      setBrowserRespondResult(out);
                      setSuccess("Response sent to browser session.");
                    } catch (e) {
                      setError(errMessage(e));
                    }
                  }}
                >
                  {browserRespondMutation.isPending ? "Sending..." : "Send Response"}
                </Button>
              </Grid2>
            </Grid2>
            {browserRespondResult ? (
              <Box sx={{ mt: 1 }}>
                <KeyValuePanel title="Last response result" data={browserRespondResult} />
              </Box>
            ) : null}
          </Box>
        </Stack>
      ) : null}

      {settingsQ.error || briefingQ.error || notificationsQ.error || error || (showAdvanced && (timelineQ.error || browserStatusQ.error)) ? (
        <Alert severity="error">
          {error ||
            errMessage(
              settingsQ.error ||
              briefingQ.error ||
              notificationsQ.error ||
              (showAdvanced ? timelineQ.error || browserStatusQ.error : null)
            )}
        </Alert>
      ) : null}
      {success ? <Alert severity="success">{success}</Alert> : null}
    </Stack>
  );
}

function DocumentsManager({ autoRefresh }: { autoRefresh: boolean }) {
  const queryClient = useQueryClient();
  const [projectId, setProjectId] = useState("");
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [selectedFileName, setSelectedFileName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  const docsQ = useQuery({ queryKey: ["documents-manager"], queryFn: () => api.rawGet("/documents?limit=100"), refetchInterval: autoRefresh ? REFRESH_MS : false });
  const projectsQ = useQuery({ queryKey: ["documents-projects"], queryFn: () => api.rawGet("/projects"), refetchInterval: autoRefresh ? REFRESH_MS : false });

  const uploadFileMutation = useMutation({
    mutationFn: async () => {
      if (!selectedFile) throw new Error("No file selected");
      const formData = new FormData();
      formData.append("file", selectedFile, selectedFile.name);
      if (projectId.trim()) formData.append("project_id", projectId.trim());
      return api.rawPostForm("/documents/upload-file", formData);
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["documents-manager"] });
    }
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => api.rawDelete(`/documents/${encodeURIComponent(id)}`),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["documents-manager"] });
    }
  });

  const docs = pickRecords(docsQ.data, "documents");
  const projects = pickRecords(projectsQ.data, "projects");
  const projectMap = useMemo(() => {
    const m = new Map<string, string>();
    projects.forEach((project) => {
      m.set(str(project.id, ""), str(project.name, "Untitled"));
    });
    return m;
  }, [projects]);

  const handleFileSelected = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    setError(null);
    setSelectedFile(file);
    setSelectedFileName(file.name);
    event.target.value = "";
  };

  return (
    <Stack spacing={2}>
      <Box className="list-shell">
        <input
          ref={fileInputRef}
          type="file"
          hidden
          accept=".txt,.md,.markdown,.json,.csv,.tsv,.xml,.html,.htm,.yaml,.yml,.log,.ini,.toml,.sql,.js,.ts,.tsx,.jsx,.py,.rs,.go,.java,.c,.cpp,.h,.hpp,.sh,.bat,.ps1,.pdf,.docx,text/*,application/pdf,application/vnd.openxmlformats-officedocument.wordprocessingml.document"
          onChange={handleFileSelected}
        />
        <Stack direction={{ xs: "column", sm: "row" }} spacing={1} alignItems={{ xs: "flex-start", sm: "center" }} mb={1}>
          <Typography variant="h6" sx={{ flex: 1 }}>
            Documents
          </Typography>
          <Button
            variant="contained"
            size="small"
            disabled={uploadFileMutation.isPending}
            onClick={() => fileInputRef.current?.click()}
          >
            Upload Document
          </Button>
        </Stack>

        {selectedFile ? (
          <Box className="metadata-box" sx={{ mb: 1.25 }}>
            <Grid2 container spacing={1} alignItems="center">
              <Grid2 size={{ xs: 12, md: projects.length > 0 ? 4 : 8 }}>
                <Typography variant="body2" sx={{ wordBreak: "break-word" }}>
                  Selected: {selectedFileName}
                </Typography>
                <Typography variant="caption" color="text.secondary">
                  Supports PDF, DOCX, TXT, MD, JSON, CSV and code/text files.
                </Typography>
              </Grid2>
              {projects.length > 0 ? (
                <Grid2 size={{ xs: 12, md: 4 }}>
                  <TextField
                    fullWidth
                    size="small"
                    select
                    label="Project (optional)"
                    value={projectId}
                    onChange={(e) => setProjectId(e.target.value)}
                    InputLabelProps={{ shrink: true }}
                    SelectProps={{ displayEmpty: true }}
                  >
                    <MenuItem value="">Global</MenuItem>
                    {projects.map((project) => <MenuItem key={str(project.id, "")} value={str(project.id, "")}>{str(project.name)}</MenuItem>)}
                  </TextField>
                </Grid2>
              ) : null}
              <Grid2 size={{ xs: 12, md: projects.length > 0 ? 4 : 4 }}>
                <Stack direction="row" spacing={1}>
                  <Button
                    variant="contained"
                    disabled={uploadFileMutation.isPending || !selectedFile}
                    onClick={async () => {
                      setError(null);
                      try {
                        await uploadFileMutation.mutateAsync();
                        setSelectedFile(null);
                        setSelectedFileName("");
                      } catch (e) {
                        setError(errMessage(e));
                      }
                    }}
                  >
                    {uploadFileMutation.isPending ? "Uploading..." : "Upload"}
                  </Button>
                  <Button
                    variant="text"
                    onClick={() => {
                      setSelectedFile(null);
                      setSelectedFileName("");
                      setError(null);
                      if (fileInputRef.current) fileInputRef.current.value = "";
                    }}
                  >
                    Clear
                  </Button>
                </Stack>
              </Grid2>
            </Grid2>
          </Box>
        ) : null}

        <TableContainer className="table-shell">
          <Table size="small">
            <TableHead><TableRow><TableCell>Filename</TableCell><TableCell>Project</TableCell><TableCell>Type</TableCell><TableCell>Chunks</TableCell><TableCell>Size</TableCell><TableCell>Created</TableCell><TableCell>Ops</TableCell></TableRow></TableHead>
            <TableBody>
              {docs.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={7}>
                    <Typography variant="body2" color="text.secondary">
                      No documents yet. Click "Upload Document" to add your first file.
                    </Typography>
                  </TableCell>
                </TableRow>
              ) : (
                docs.map((doc) => {
                  const id = str(doc.id, "");
                  const pid = str(doc.project_id, "");
                  return (
                    <TableRow key={id}>
                      <TableCell>{str(doc.filename)}</TableCell>
                      <TableCell>{pid ? projectMap.get(pid) || pid : "-"}</TableCell>
                      <TableCell>{str(doc.content_type)}</TableCell>
                      <TableCell>{str(doc.chunk_count)}</TableCell>
                      <TableCell>{formatBytes(doc.file_size)}</TableCell>
                      <TableCell>{str(doc.created_at)}</TableCell>
                      <TableCell align="right">
                        <RowOpsMenu
                          actions={[
                            {
                              label: "Delete",
                              tone: "error",
                              onClick: () => deleteMutation.mutate(id)
                            }
                          ]}
                          ariaLabel="Document options"
                        />
                      </TableCell>
                    </TableRow>
                  );
                })
              )}
            </TableBody>
          </Table>
        </TableContainer>
      </Box>

      {docsQ.error || projectsQ.error || error ? <Alert severity="error">{error || errMessage(docsQ.error || projectsQ.error)}</Alert> : null}
    </Stack>
  );
}

function MemoryManager({ autoRefresh }: { autoRefresh: boolean }) {
  const queryClient = useQueryClient();
  const [error, setError] = useState<string | null>(null);
  const [selectedFact, setSelectedFact] = useState<JsonRecord | null>(null);
  const [memoryTab, setMemoryTab] = useState(0);
  const [prefKey, setPrefKey] = useState("");
  const [prefValue, setPrefValue] = useState("");
  const [prefConfidence, setPrefConfidence] = useState("0.85");
  const [prefSource, setPrefSource] = useState("");
  const [dataKind, setDataKind] = useState("note");
  const [dataTitle, setDataTitle] = useState("");
  const [dataContent, setDataContent] = useState("");
  const [dataUrl, setDataUrl] = useState("");
  const [knowledgeTitle, setKnowledgeTitle] = useState("");
  const [knowledgeContent, setKnowledgeContent] = useState("");
  const [knowledgeSource, setKnowledgeSource] = useState("");
  const [knowledgeUrl, setKnowledgeUrl] = useState("");
  const [knowledgeTags, setKnowledgeTags] = useState("");

  const invalidateMemoryQueries = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["memory-stats"] }),
      queryClient.invalidateQueries({ queryKey: ["memory-facts"] }),
      queryClient.invalidateQueries({ queryKey: ["memory-preferences"] }),
      queryClient.invalidateQueries({ queryKey: ["memory-user-data"] }),
      queryClient.invalidateQueries({ queryKey: ["memory-knowledge"] })
    ]);
  };

  const statsQ = useQuery({
    queryKey: ["memory-stats"],
    queryFn: () => api.rawGet("/memory/stats"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const factsQ = useQuery({
    queryKey: ["memory-facts"],
    queryFn: () => api.rawGet("/memory/facts?limit=50"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const preferencesQ = useQuery({
    queryKey: ["memory-preferences"],
    queryFn: () => api.rawGet("/memory/preferences?limit=100"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const userDataQ = useQuery({
    queryKey: ["memory-user-data"],
    queryFn: () => api.rawGet("/memory/user-data?limit=100"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const knowledgeQ = useQuery({
    queryKey: ["memory-knowledge"],
    queryFn: () => api.rawGet("/memory/knowledge?limit=100"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });

  const createPreferenceMutation = useMutation({
    mutationFn: (payload: JsonRecord) => api.rawPost("/memory/preferences", payload),
    onSuccess: async () => {
      await invalidateMemoryQueries();
    }
  });
  const deletePreferenceMutation = useMutation({
    mutationFn: (endpoint: string) => api.rawDelete(endpoint),
    onSuccess: async () => {
      await invalidateMemoryQueries();
    }
  });
  const createUserDataMutation = useMutation({
    mutationFn: (payload: JsonRecord) => api.rawPost("/memory/user-data", payload),
    onSuccess: async () => {
      await invalidateMemoryQueries();
    }
  });
  const deleteUserDataMutation = useMutation({
    mutationFn: (id: string) => api.rawDelete(`/memory/user-data/${encodeURIComponent(id)}`),
    onSuccess: async () => {
      await invalidateMemoryQueries();
    }
  });
  const createKnowledgeMutation = useMutation({
    mutationFn: (payload: JsonRecord) => api.rawPost("/memory/knowledge", payload),
    onSuccess: async () => {
      await invalidateMemoryQueries();
    }
  });
  const deleteKnowledgeMutation = useMutation({
    mutationFn: (id: string) => api.rawDelete(`/memory/knowledge/${encodeURIComponent(id)}`),
    onSuccess: async () => {
      await invalidateMemoryQueries();
    }
  });

  const stats = asRecord(statsQ.data);
  const facts = pickRecords(factsQ.data, "facts");
  const preferences = pickRecords(preferencesQ.data, "preferences");
  const userDataItems = pickRecords(userDataQ.data, "items");
  const knowledgeItems = pickRecords(knowledgeQ.data, "items");

  const parseSources = (value: unknown): string[] => {
    if (Array.isArray(value)) return value.map((v) => String(v));
    if (typeof value !== "string" || !value.trim()) return [];
    try {
      const parsed = JSON.parse(value);
      if (Array.isArray(parsed)) return parsed.map((v) => String(v));
    } catch {
      // Keep fallback below.
    }
    return [value];
  };

  return (
    <Stack spacing={2}>
      <Grid2 container spacing={2} alignItems="stretch">
        <Grid2 size={{ xs: 12, sm: 6, md: 4, lg: 3 }} sx={{ display: "flex" }}>
          <Box className="list-shell" sx={{ minHeight: 120, height: "100%", width: "100%" }}>
            <Typography variant="caption" color="text.secondary">
              Episodic Memory
            </Typography>
            <Typography variant="h5">{num(stats.episodes)}</Typography>
          </Box>
        </Grid2>
        <Grid2 size={{ xs: 12, sm: 6, md: 4, lg: 3 }} sx={{ display: "flex" }}>
          <Box className="list-shell" sx={{ minHeight: 120, height: "100%", width: "100%" }}>
            <Typography variant="caption" color="text.secondary">
              Semantic Facts
            </Typography>
            <Typography variant="h5">{num(stats.facts)}</Typography>
          </Box>
        </Grid2>
        <Grid2 size={{ xs: 12, sm: 6, md: 4, lg: 3 }} sx={{ display: "flex" }}>
          <Box className="list-shell" sx={{ minHeight: 120, height: "100%", width: "100%" }}>
            <Typography variant="caption" color="text.secondary">
              Preferences
            </Typography>
            <Typography variant="h5">{num(stats.preferences)}</Typography>
          </Box>
        </Grid2>
        <Grid2 size={{ xs: 12, sm: 6, md: 6, lg: 3 }} sx={{ display: "flex" }}>
          <Box className="list-shell" sx={{ minHeight: 120, height: "100%", width: "100%" }}>
            <Typography variant="caption" color="text.secondary">
              User Data Items
            </Typography>
            <Typography variant="h5">{num(stats.user_data)}</Typography>
          </Box>
        </Grid2>
        <Grid2 size={{ xs: 12, sm: 6, md: 6, lg: 3 }} sx={{ display: "flex" }}>
          <Box className="list-shell" sx={{ minHeight: 120, height: "100%", width: "100%" }}>
            <Typography variant="caption" color="text.secondary">
              Knowledge Base
            </Typography>
            <Typography variant="h5">{num(stats.knowledge)}</Typography>
          </Box>
        </Grid2>
      </Grid2>

      <Box className="list-shell">
        <Stack direction={{ xs: "column", md: "row" }} justifyContent="space-between" alignItems={{ xs: "flex-start", md: "center" }} gap={1}>
          <Box>
            <Typography variant="h6">Memory Layer</Typography>
            <Typography variant="body2" color="text.secondary">
              Manage what the agent remembers: preferences, user-owned data, and durable knowledge.
            </Typography>
          </Box>
          <Tabs
            value={memoryTab}
            onChange={(_e, next) => setMemoryTab(next)}
            variant="scrollable"
            allowScrollButtonsMobile
            sx={{ minHeight: 0, "& .MuiTab-root": { minHeight: 0, py: 0.5 } }}
          >
            <Tab label={`Facts (${facts.length})`} />
            <Tab label={`Preferences (${preferences.length})`} />
            <Tab label={`User Data (${userDataItems.length})`} />
            <Tab label={`Knowledge (${knowledgeItems.length})`} />
          </Tabs>
        </Stack>
      </Box>

      {memoryTab === 0 ? (
        <Box className="list-shell">
          <Typography variant="h6" mb={1}>
            Semantic Facts
          </Typography>
          {factsQ.error ? <Alert severity="error">{errMessage(factsQ.error)}</Alert> : null}
          {facts.length === 0 ? (
            <Typography variant="body2" color="text.secondary">
              No facts yet.
            </Typography>
          ) : (
            <TableContainer className="table-shell">
              <Table size="small">
                <TableHead>
                  <TableRow>
                    <TableCell>Fact</TableCell>
                    <TableCell>Confidence</TableCell>
                    <TableCell>Created</TableCell>
                    <TableCell>Sources</TableCell>
                    <TableCell align="right">Ops</TableCell>
                  </TableRow>
                </TableHead>
                <TableBody>
                  {facts.slice(0, 50).map((f, idx) => {
                    const id = str(f.id, String(idx));
                    const sources = parseSources(f.sources);
                    return (
                      <TableRow key={id}>
                        <TableCell sx={{ maxWidth: 640 }}>
                          <Typography variant="body2" noWrap title={str(f.fact, "-")}>
                            {str(f.fact, "-")}
                          </Typography>
                        </TableCell>
                        <TableCell>{num(f.confidence, 0).toFixed(2)}</TableCell>
                        <TableCell sx={{ whiteSpace: "nowrap" }}>{str(f.created_at, "-")}</TableCell>
                        <TableCell>{sources.length}</TableCell>
                        <TableCell align="right">
                          <RowOpsMenu
                            actions={[
                              {
                                label: "View",
                                onClick: () => setSelectedFact(asRecord(f))
                              }
                            ]}
                            ariaLabel="Fact options"
                          />
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </TableContainer>
          )}
        </Box>
      ) : null}

      {memoryTab === 1 ? (
        <Stack spacing={2}>
          <Box className="list-shell">
            <Typography variant="h6" mb={1}>
              Add Preference
            </Typography>
            <Grid2 container spacing={1}>
              <Grid2 size={{ xs: 12, md: 3 }}>
                <TextField fullWidth size="small" label="Key" placeholder="timezone" value={prefKey} onChange={(e) => setPrefKey(e.target.value)} />
              </Grid2>
              <Grid2 size={{ xs: 12, md: 4 }}>
                <TextField fullWidth size="small" label="Value" placeholder="Asia/Kolkata" value={prefValue} onChange={(e) => setPrefValue(e.target.value)} />
              </Grid2>
              <Grid2 size={{ xs: 12, md: 2 }}>
                <TextField fullWidth size="small" type="number" label="Confidence" inputProps={{ min: 0, max: 1, step: 0.05 }} value={prefConfidence} onChange={(e) => setPrefConfidence(e.target.value)} />
              </Grid2>
              <Grid2 size={{ xs: 12, md: 3 }}>
                <TextField fullWidth size="small" label="Source (optional)" placeholder="user_message" value={prefSource} onChange={(e) => setPrefSource(e.target.value)} />
              </Grid2>
              <Grid2 size={{ xs: 12 }} sx={{ display: "flex", justifyContent: "flex-end" }}>
                <Button
                  variant="contained"
                  disabled={createPreferenceMutation.isPending || !prefKey.trim() || !prefValue.trim()}
                  onClick={async () => {
                    setError(null);
                    try {
                      const parsedConfidence = Number(prefConfidence);
                      await createPreferenceMutation.mutateAsync({
                        key: prefKey.trim(),
                        value: prefValue.trim(),
                        confidence: Number.isFinite(parsedConfidence) ? parsedConfidence : 0.85,
                        source: prefSource.trim() || undefined
                      });
                      setPrefKey("");
                      setPrefValue("");
                      setPrefSource("");
                    } catch (e) {
                      setError(errMessage(e));
                    }
                  }}
                >
                  Save Preference
                </Button>
              </Grid2>
            </Grid2>
          </Box>

          <Box className="list-shell">
            <Typography variant="h6" mb={1}>
              Preferences
            </Typography>
            {preferencesQ.error ? <Alert severity="error">{errMessage(preferencesQ.error)}</Alert> : null}
            {preferences.length === 0 ? (
              <Typography variant="body2" color="text.secondary">
                No preferences yet.
              </Typography>
            ) : (
              <TableContainer className="table-shell">
                <Table size="small">
                  <TableHead>
                    <TableRow>
                      <TableCell>Key</TableCell>
                      <TableCell>Value</TableCell>
                      <TableCell>Confidence</TableCell>
                      <TableCell>Source</TableCell>
                      <TableCell>Scope</TableCell>
                      <TableCell>Updated</TableCell>
                      <TableCell align="right">Ops</TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {preferences.map((pref, idx) => {
                      const key = str(pref.key, String(idx));
                      const projectId = typeof pref.project_id === "string" ? pref.project_id : "";
                      const endpoint = projectId
                        ? `/memory/preferences/${encodeURIComponent(key)}?project_id=${encodeURIComponent(projectId)}`
                        : `/memory/preferences/${encodeURIComponent(key)}`;
                      return (
                        <TableRow key={`${projectId || "global"}-${key}-${idx}`}>
                          <TableCell sx={{ whiteSpace: "nowrap" }}>{key}</TableCell>
                          <TableCell sx={{ maxWidth: 480 }}>
                            <Typography variant="body2" noWrap title={str(pref.value, "-")}>
                              {str(pref.value, "-")}
                            </Typography>
                          </TableCell>
                          <TableCell>{num(pref.confidence, 0).toFixed(2)}</TableCell>
                          <TableCell>{str(pref.source, "-")}</TableCell>
                          <TableCell>{projectId || "Global"}</TableCell>
                          <TableCell sx={{ whiteSpace: "nowrap" }}>{str(pref.updated_at, "-")}</TableCell>
                          <TableCell align="right">
                            <RowOpsMenu
                              actions={[
                                {
                                  label: "Delete",
                                  tone: "error",
                                  divider: true,
                                  onClick: async () => {
                                    setError(null);
                                    try {
                                      await deletePreferenceMutation.mutateAsync(endpoint);
                                    } catch (e) {
                                      setError(errMessage(e));
                                    }
                                  }
                                }
                              ]}
                              ariaLabel="Preference options"
                            />
                          </TableCell>
                        </TableRow>
                      );
                    })}
                  </TableBody>
                </Table>
              </TableContainer>
            )}
          </Box>
        </Stack>
      ) : null}

      {memoryTab === 2 ? (
        <Stack spacing={2}>
          <Box className="list-shell">
            <Typography variant="h6" mb={1}>
              Add User Data
            </Typography>
            <Grid2 container spacing={1}>
              <Grid2 size={{ xs: 12, md: 3 }}>
                <TextField fullWidth size="small" label="Kind" placeholder="note | link | file" value={dataKind} onChange={(e) => setDataKind(e.target.value)} />
              </Grid2>
              <Grid2 size={{ xs: 12, md: 5 }}>
                <TextField fullWidth size="small" label="Title" placeholder="Quarterly roadmap doc" value={dataTitle} onChange={(e) => setDataTitle(e.target.value)} />
              </Grid2>
              <Grid2 size={{ xs: 12, md: 4 }}>
                <TextField fullWidth size="small" label="URL (optional)" placeholder="https://..." value={dataUrl} onChange={(e) => setDataUrl(e.target.value)} />
              </Grid2>
              <Grid2 size={{ xs: 12 }}>
                <TextField fullWidth size="small" multiline minRows={3} label="Content" placeholder="Summary or notes" value={dataContent} onChange={(e) => setDataContent(e.target.value)} />
              </Grid2>
              <Grid2 size={{ xs: 12 }} sx={{ display: "flex", justifyContent: "flex-end" }}>
                <Button
                  variant="contained"
                  disabled={createUserDataMutation.isPending || !dataKind.trim() || !dataTitle.trim()}
                  onClick={async () => {
                    setError(null);
                    try {
                      await createUserDataMutation.mutateAsync({
                        kind: dataKind.trim(),
                        title: dataTitle.trim(),
                        content: dataContent.trim(),
                        url: dataUrl.trim() || undefined
                      });
                      setDataKind("note");
                      setDataTitle("");
                      setDataContent("");
                      setDataUrl("");
                    } catch (e) {
                      setError(errMessage(e));
                    }
                  }}
                >
                  Save User Data
                </Button>
              </Grid2>
            </Grid2>
          </Box>

          <Box className="list-shell">
            <Typography variant="h6" mb={1}>
              User Data
            </Typography>
            {userDataQ.error ? <Alert severity="error">{errMessage(userDataQ.error)}</Alert> : null}
            {userDataItems.length === 0 ? (
              <Typography variant="body2" color="text.secondary">
                No user data items yet.
              </Typography>
            ) : (
              <TableContainer className="table-shell">
                <Table size="small">
                  <TableHead>
                    <TableRow>
                      <TableCell>Kind</TableCell>
                      <TableCell>Title</TableCell>
                      <TableCell>Content</TableCell>
                      <TableCell>URL</TableCell>
                      <TableCell>Updated</TableCell>
                      <TableCell align="right">Ops</TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {userDataItems.map((item, idx) => {
                      const id = str(item.id, String(idx));
                      const url = str(item.url, "");
                      return (
                        <TableRow key={id}>
                          <TableCell>{str(item.kind, "-")}</TableCell>
                          <TableCell sx={{ maxWidth: 220 }}>
                            <Typography variant="body2" noWrap title={str(item.title, "-")}>
                              {str(item.title, "-")}
                            </Typography>
                          </TableCell>
                          <TableCell sx={{ maxWidth: 380 }}>
                            <Typography variant="body2" noWrap title={str(item.content, "-")}>
                              {str(item.content, "-")}
                            </Typography>
                          </TableCell>
                          <TableCell sx={{ maxWidth: 260 }}>
                            {url ? (
                              <Typography component="a" href={url} target="_blank" rel="noopener noreferrer" variant="body2" sx={{ color: "var(--mui-palette-info-main)", textDecoration: "none" }}>
                                Open
                              </Typography>
                            ) : (
                              <Typography variant="body2" color="text.secondary">-</Typography>
                            )}
                          </TableCell>
                          <TableCell sx={{ whiteSpace: "nowrap" }}>{str(item.updated_at, "-")}</TableCell>
                          <TableCell align="right">
                            <RowOpsMenu
                              actions={[
                                {
                                  label: "Delete",
                                  tone: "error",
                                  divider: true,
                                  onClick: async () => {
                                    setError(null);
                                    try {
                                      await deleteUserDataMutation.mutateAsync(id);
                                    } catch (e) {
                                      setError(errMessage(e));
                                    }
                                  }
                                }
                              ]}
                              ariaLabel="User data options"
                            />
                          </TableCell>
                        </TableRow>
                      );
                    })}
                  </TableBody>
                </Table>
              </TableContainer>
            )}
          </Box>
        </Stack>
      ) : null}

      {memoryTab === 3 ? (
        <Stack spacing={2}>
          <Box className="list-shell">
            <Typography variant="h6" mb={1}>
              Add Knowledge Base Item
            </Typography>
            <Grid2 container spacing={1}>
              <Grid2 size={{ xs: 12, md: 5 }}>
                <TextField fullWidth size="small" label="Title" placeholder="How we deploy production apps" value={knowledgeTitle} onChange={(e) => setKnowledgeTitle(e.target.value)} />
              </Grid2>
              <Grid2 size={{ xs: 12, md: 3 }}>
                <TextField fullWidth size="small" label="Source (optional)" placeholder="runbook" value={knowledgeSource} onChange={(e) => setKnowledgeSource(e.target.value)} />
              </Grid2>
              <Grid2 size={{ xs: 12, md: 4 }}>
                <TextField fullWidth size="small" label="URL (optional)" placeholder="https://..." value={knowledgeUrl} onChange={(e) => setKnowledgeUrl(e.target.value)} />
              </Grid2>
              <Grid2 size={{ xs: 12 }}>
                <TextField fullWidth size="small" multiline minRows={3} label="Content" placeholder="Durable, reusable knowledge" value={knowledgeContent} onChange={(e) => setKnowledgeContent(e.target.value)} />
              </Grid2>
              <Grid2 size={{ xs: 12, md: 9 }}>
                <TextField fullWidth size="small" label="Tags (optional)" placeholder="ops, deployment, production" value={knowledgeTags} onChange={(e) => setKnowledgeTags(e.target.value)} />
              </Grid2>
              <Grid2 size={{ xs: 12, md: 3 }} sx={{ display: "flex", justifyContent: { xs: "flex-end", md: "stretch" }, alignItems: "stretch" }}>
                <Button
                  fullWidth
                  variant="contained"
                  disabled={createKnowledgeMutation.isPending || !knowledgeTitle.trim() || !knowledgeContent.trim()}
                  onClick={async () => {
                    setError(null);
                    try {
                      await createKnowledgeMutation.mutateAsync({
                        title: knowledgeTitle.trim(),
                        content: knowledgeContent.trim(),
                        source: knowledgeSource.trim() || undefined,
                        url: knowledgeUrl.trim() || undefined,
                        tags: knowledgeTags.trim() || undefined
                      });
                      setKnowledgeTitle("");
                      setKnowledgeContent("");
                      setKnowledgeSource("");
                      setKnowledgeUrl("");
                      setKnowledgeTags("");
                    } catch (e) {
                      setError(errMessage(e));
                    }
                  }}
                >
                  Save Knowledge
                </Button>
              </Grid2>
            </Grid2>
          </Box>

          <Box className="list-shell">
            <Typography variant="h6" mb={1}>
              Knowledge Base
            </Typography>
            {knowledgeQ.error ? <Alert severity="error">{errMessage(knowledgeQ.error)}</Alert> : null}
            {knowledgeItems.length === 0 ? (
              <Typography variant="body2" color="text.secondary">
                No knowledge items yet.
              </Typography>
            ) : (
              <TableContainer className="table-shell">
                <Table size="small">
                  <TableHead>
                    <TableRow>
                      <TableCell>Title</TableCell>
                      <TableCell>Content</TableCell>
                      <TableCell>Source</TableCell>
                      <TableCell>Tags</TableCell>
                      <TableCell>Updated</TableCell>
                      <TableCell align="right">Ops</TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {knowledgeItems.map((item, idx) => {
                      const id = str(item.id, String(idx));
                      return (
                        <TableRow key={id}>
                          <TableCell sx={{ maxWidth: 260 }}>
                            <Typography variant="body2" noWrap title={str(item.title, "-")}>
                              {str(item.title, "-")}
                            </Typography>
                          </TableCell>
                          <TableCell sx={{ maxWidth: 420 }}>
                            <Typography variant="body2" noWrap title={str(item.content, "-")}>
                              {str(item.content, "-")}
                            </Typography>
                          </TableCell>
                          <TableCell>{str(item.source, "-")}</TableCell>
                          <TableCell>{str(item.tags, "-")}</TableCell>
                          <TableCell sx={{ whiteSpace: "nowrap" }}>{str(item.updated_at, "-")}</TableCell>
                          <TableCell align="right">
                            <RowOpsMenu
                              actions={[
                                {
                                  label: "Delete",
                                  tone: "error",
                                  divider: true,
                                  onClick: async () => {
                                    setError(null);
                                    try {
                                      await deleteKnowledgeMutation.mutateAsync(id);
                                    } catch (e) {
                                      setError(errMessage(e));
                                    }
                                  }
                                }
                              ]}
                              ariaLabel="Knowledge options"
                            />
                          </TableCell>
                        </TableRow>
                      );
                    })}
                  </TableBody>
                </Table>
              </TableContainer>
            )}
          </Box>
        </Stack>
      ) : null}

      {statsQ.error || factsQ.error || preferencesQ.error || userDataQ.error || knowledgeQ.error || error ? (
        <Alert severity="error">
          {error || errMessage(statsQ.error || factsQ.error || preferencesQ.error || userDataQ.error || knowledgeQ.error)}
        </Alert>
      ) : null}

      <Dialog open={selectedFact != null} onClose={() => setSelectedFact(null)} maxWidth="md" fullWidth>
        <DialogTitle>Fact</DialogTitle>
        <DialogContent>
          <Stack spacing={1}>
            <Typography variant="caption" color="text.secondary">
              Confidence: {num(selectedFact?.confidence, 0)} | Created: {str(selectedFact?.created_at, "-")}
            </Typography>
            <Typography variant="body2" sx={{ whiteSpace: "pre-wrap" }}>
              {str(selectedFact?.fact, "-")}
            </Typography>
            <Divider />
            <Typography variant="subtitle2">Sources</Typography>
            {parseSources(selectedFact?.sources).length ? (
              <Stack spacing={0.5}>
                {parseSources(selectedFact?.sources).slice(0, 50).map((s, i) => (
                  <Box key={`src-${i}`} className="console-line">
                    <Typography variant="body2" sx={{ fontFamily: "JetBrains Mono, monospace" }}>
                      {String(s)}
                    </Typography>
                  </Box>
                ))}
              </Stack>
            ) : (
              <Typography variant="body2" color="text.secondary">
                No sources recorded.
              </Typography>
            )}
          </Stack>
        </DialogContent>
      </Dialog>
    </Stack>
  );
}
function ProjectsManager({ autoRefresh }: { autoRefresh: boolean }) {
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [createOpen, setCreateOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedProject, setSelectedProject] = useState<JsonRecord | null>(null);
  const [deleteProject, setDeleteProject] = useState<JsonRecord | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState("");
  const [editForm, setEditForm] = useState({
    name: "",
    description: "",
    system_prompt: "",
    personality: "",
    tools_filter: "",
    active: true
  });

  const projectsQ = useQuery({ queryKey: ["projects-manager"], queryFn: () => api.rawGet("/projects"), refetchInterval: autoRefresh ? REFRESH_MS : false });
  const conversationsQ = useQuery({ queryKey: ["projects-conversations"], queryFn: () => api.rawGet("/conversations?limit=100"), refetchInterval: autoRefresh ? REFRESH_MS : false });

  const createMutation = useMutation({ mutationFn: () => api.rawPost("/projects", { name: name.trim(), description: description.trim() }), onSuccess: async () => { await queryClient.invalidateQueries({ queryKey: ["projects-manager"] }); } });
  const deleteMutation = useMutation({
    mutationFn: (id: string) => api.rawDelete(`/projects/${encodeURIComponent(id)}`),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["projects-manager"] });
      await queryClient.invalidateQueries({ queryKey: ["projects-conversations"] });
      await queryClient.invalidateQueries({ queryKey: ["documents-manager"] });
      await queryClient.invalidateQueries({ queryKey: ["memory-stats"] });
      await queryClient.invalidateQueries({ queryKey: ["memory-facts"] });
      setDeleteProject(null);
      setDeleteConfirm("");
    }
  });
  const updateMutation = useMutation({
    mutationFn: (payload: { id: string; body: Record<string, unknown> }) =>
      api.rawPut(`/projects/${encodeURIComponent(payload.id)}`, payload.body),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["projects-manager"] });
      setSelectedProject(null);
    }
  });

  const projects = pickRecords(projectsQ.data, "projects");
  const conversations = pickRecords(conversationsQ.data, "conversations");
  const counts = useMemo(() => {
    const map = new Map<string, number>();
    conversations.forEach((conv) => {
      const pid = str(conv.project_id, "");
      if (!pid) return;
      map.set(pid, (map.get(pid) || 0) + 1);
    });
    return map;
  }, [conversations]);

  return (
    <Stack spacing={2}>
      <Dialog open={createOpen} onClose={() => setCreateOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>Create Project</DialogTitle>
        <DialogContent>
          <Stack spacing={2} sx={{ mt: 1 }}>
            <TextField fullWidth size="small" label="Name" value={name} onChange={(e) => setName(e.target.value)} />
            <TextField fullWidth size="small" label="Description" value={description} onChange={(e) => setDescription(e.target.value)} />
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setCreateOpen(false)}>Cancel</Button>
          <Button variant="contained" disabled={createMutation.isPending || !name.trim()} onClick={async () => { setError(null); try { await createMutation.mutateAsync(); setName(""); setDescription(""); setCreateOpen(false); } catch (e) { setError(errMessage(e)); } }}>Create</Button>
        </DialogActions>
      </Dialog>

      <Grid2 container spacing={2}>
        <Grid2 size={{ xs: 12, lg: 7 }}>
          <Box className="list-shell">
            <Stack direction="row" justifyContent="space-between" alignItems="center" mb={1}>
              <Typography variant="h6">Projects</Typography>
              <Button size="small" variant="contained" onClick={() => setCreateOpen(true)}>New Project</Button>
            </Stack>
            <TableContainer className="table-shell">
              <Table size="small">
                <TableHead>
                  <TableRow>
                    <TableCell>Name</TableCell>
                    <TableCell>Description</TableCell>
                    <TableCell>Conversations</TableCell>
                    <TableCell>Updated</TableCell>
                    <TableCell align="right">Ops</TableCell>
                  </TableRow>
                </TableHead>
                <TableBody>
                  {projects.map((project) => {
                    const id = str(project.id, "");
                    const pname = str(project.name, "");
                    return (
                      <TableRow key={id}>
                        <TableCell>{str(project.name)}</TableCell>
                        <TableCell>{str(project.description)}</TableCell>
                        <TableCell>{counts.get(id) || 0}</TableCell>
                        <TableCell>{str(project.updated_at, str(project.created_at))}</TableCell>
                        <TableCell align="right">
                          <RowOpsMenu
                            actions={[
                              {
                                label: "Edit",
                                onClick: () => {
                                  const pr = asRecord(project);
                                  setSelectedProject(pr);
                                  setEditForm({
                                    name: str(pr.name, ""),
                                    description: str(pr.description, ""),
                                    system_prompt: str(pr.system_prompt, ""),
                                    personality: str(pr.personality, ""),
                                    tools_filter: str(pr.tools_filter, ""),
                                    active: pr.active !== false
                                  });
                                }
                              },
                              {
                                label: "Delete",
                                tone: "error",
                                divider: true,
                                onClick: () => {
                                  setDeleteProject(asRecord(project));
                                  setDeleteConfirm("");
                                }
                              }
                            ]}
                            ariaLabel="Project options"
                          />
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </TableContainer>
          </Box>
        </Grid2>
        <Grid2 size={{ xs: 12, lg: 5 }}><QueryTable title="Project Conversations" path="/conversations?limit=100" arrayKey="conversations" columns={["title", "project_id", "channel", "updated_at"]} autoRefresh={autoRefresh} emptyLabel="No conversations mapped to projects." queryKey="projects-conversation-table" /></Grid2>
      </Grid2>

      {projectsQ.error || conversationsQ.error || error ? <Alert severity="error">{error || errMessage(projectsQ.error || conversationsQ.error)}</Alert> : null}

      <Dialog open={selectedProject != null} onClose={() => setSelectedProject(null)} maxWidth="md" fullWidth>
        <DialogTitle>Edit Project</DialogTitle>
        <DialogContent>
          <Stack spacing={1.2}>
            <Grid2 container spacing={1}>
              <Grid2 size={{ xs: 12, md: 6 }}>
                <TextField
                  fullWidth
                  size="small"
                  label="Name"
                  value={editForm.name}
                  onChange={(e) => setEditForm((p) => ({ ...p, name: e.target.value }))}
                />
              </Grid2>
              <Grid2 size={{ xs: 12, md: 6 }}>
                <FormControlLabel
                  control={
                    <Switch
                      checked={editForm.active}
                      onChange={(e) => setEditForm((p) => ({ ...p, active: e.target.checked }))}
                    />
                  }
                  label="Active"
                />
              </Grid2>
              <Grid2 size={{ xs: 12 }}>
                <TextField
                  fullWidth
                  size="small"
                  label="Description"
                  value={editForm.description}
                  onChange={(e) => setEditForm((p) => ({ ...p, description: e.target.value }))}
                />
              </Grid2>
              <Grid2 size={{ xs: 12 }}>
                <TextField
                  fullWidth
                  size="small"
                  multiline
                  minRows={4}
                  label="System Prompt (optional)"
                  value={editForm.system_prompt}
                  onChange={(e) => setEditForm((p) => ({ ...p, system_prompt: e.target.value }))}
                />
              </Grid2>
              <Grid2 size={{ xs: 12, md: 6 }}>
                <TextField
                  fullWidth
                  size="small"
                  label="Personality (optional)"
                  value={editForm.personality}
                  onChange={(e) => setEditForm((p) => ({ ...p, personality: e.target.value }))}
                  placeholder="e.g. friendly"
                />
              </Grid2>
              <Grid2 size={{ xs: 12, md: 6 }}>
                <TextField
                  fullWidth
                  size="small"
                  label="Tools Filter (optional)"
                  value={editForm.tools_filter}
                  onChange={(e) => setEditForm((p) => ({ ...p, tools_filter: e.target.value }))}
                  placeholder="Comma-separated allowlist"
                />
              </Grid2>
            </Grid2>

            <Stack direction="row" spacing={1} justifyContent="flex-end">
              <Button onClick={() => setSelectedProject(null)}>Cancel</Button>
              <Button
                variant="contained"
                disabled={updateMutation.isPending || !editForm.name.trim()}
                onClick={async () => {
                  const id = str(selectedProject?.id, "");
                  if (!id) return;
                  setError(null);
                  try {
                    await updateMutation.mutateAsync({
                      id,
                      body: {
                        name: editForm.name.trim(),
                        description: editForm.description.trim(),
                        system_prompt: editForm.system_prompt.trim() || undefined,
                        personality: editForm.personality.trim() || undefined,
                        tools_filter: editForm.tools_filter.trim() || undefined,
                        active: editForm.active
                      }
                    });
                  } catch (e) {
                    setError(errMessage(e));
                  }
                }}
              >
                Save
              </Button>
            </Stack>
          </Stack>
        </DialogContent>
      </Dialog>

      <Dialog open={deleteProject != null} onClose={() => setDeleteProject(null)} maxWidth="sm" fullWidth>
        <DialogTitle>Delete Project</DialogTitle>
        <DialogContent>
          <Stack spacing={1}>
            <Alert severity="warning">
              This permanently deletes the project and ALL associated data: conversations, messages, documents, document chunks, episodic memories, and semantic facts.
            </Alert>
            <Typography variant="body2">
              Type the project name to confirm deletion: <b>{str(deleteProject?.name, "")}</b>
            </Typography>
            <TextField
              fullWidth
              size="small"
              label="Project name"
              value={deleteConfirm}
              onChange={(e) => setDeleteConfirm(e.target.value)}
            />
            <Stack direction="row" spacing={1} justifyContent="flex-end">
              <Button onClick={() => setDeleteProject(null)}>Cancel</Button>
              <Button
                color="error"
                variant="contained"
                disabled={
                  deleteMutation.isPending ||
                  !str(deleteProject?.id, "").trim() ||
                  deleteConfirm.trim() !== str(deleteProject?.name, "")
                }
                onClick={async () => {
                  const id = str(deleteProject?.id, "");
                  if (!id) return;
                  setError(null);
                  try {
                    await deleteMutation.mutateAsync(id);
                  } catch (e) {
                    setError(errMessage(e));
                  }
                }}
              >
                Delete Permanently
              </Button>
            </Stack>
          </Stack>
        </DialogContent>
      </Dialog>
    </Stack>
  );
}

function SwarmManager({ autoRefresh }: { autoRefresh: boolean }) {
  const statusQ = useQuery({ queryKey: ["swarm-status"], queryFn: () => api.rawGet("/swarm/status"), refetchInterval: autoRefresh ? REFRESH_MS : false });
  const agentsQ = useQuery({ queryKey: ["swarm-agents"], queryFn: () => api.rawGet("/swarm/agents"), refetchInterval: autoRefresh ? REFRESH_MS : false });

  const status = asRecord(statusQ.data);

  return (
    <Stack spacing={2}>
      <Grid2 container spacing={2}>
        <Grid2 size={{ xs: 12, md: 3 }}><Box className="list-shell" sx={{ minHeight: 120 }}><Typography variant="caption" color="text.secondary">Swarm Enabled</Typography><Typography variant="h5">{boolText(status.enabled)}</Typography></Box></Grid2>
        <Grid2 size={{ xs: 12, md: 3 }}><Box className="list-shell" sx={{ minHeight: 120 }}><Typography variant="caption" color="text.secondary">Total Agents</Typography><Typography variant="h5">{num(status.total_agents)}</Typography></Box></Grid2>
        <Grid2 size={{ xs: 12, md: 3 }}><Box className="list-shell" sx={{ minHeight: 120 }}><Typography variant="caption" color="text.secondary">Active Agents</Typography><Typography variant="h5">{num(status.active_agents)}</Typography></Box></Grid2>
        <Grid2 size={{ xs: 12, md: 3 }}><Box className="list-shell" sx={{ minHeight: 120 }}><Typography variant="caption" color="text.secondary">Delegations</Typography><Typography variant="h5">{num(asRecord(agentsQ.data).total, pickRecords(agentsQ.data, "agents").length)}</Typography></Box></Grid2>
      </Grid2>

      <Grid2 container spacing={2}>
        <Grid2 size={{ xs: 12, lg: 6 }}><QueryTable title="Agents" path="/swarm/agents" arrayKey="agents" columns={["name", "agent_type", "status", "enabled", "capabilities"]} autoRefresh={autoRefresh} emptyLabel="No swarm agents configured." queryKey="swarm-agents-table" /></Grid2>
        <Grid2 size={{ xs: 12, lg: 6 }}><QueryTable title="Delegations" path="/swarm/delegations?limit=30" arrayKey="delegations" columns={["task", "agent_id", "success", "confidence", "execution_time_ms", "created_at"]} autoRefresh={autoRefresh} emptyLabel="No delegations yet." queryKey="swarm-delegations-table" /></Grid2>
      </Grid2>

      {statusQ.error || agentsQ.error ? <Alert severity="error">{errMessage(statusQ.error || agentsQ.error)}</Alert> : null}
    </Stack>
  );
}
function TraceManager({ autoRefresh }: { autoRefresh: boolean }) {
  const [selectedTraceId, setSelectedTraceId] = useState<string | null>(null);

  const traceQ = useQuery({ queryKey: ["trace-manager"], queryFn: () => api.rawGet("/trace?limit=40"), refetchInterval: autoRefresh ? REFRESH_MS : false });
  const traceDetailQ = useQuery({ queryKey: ["trace-detail", selectedTraceId], queryFn: () => api.rawGet(`/trace/${encodeURIComponent(selectedTraceId || "")}`), enabled: !!selectedTraceId });
  const approvalsQ = useQuery({
    queryKey: ["approvals-log"],
    queryFn: () => api.rawGet("/approvals/log?limit=40"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });

  const history = pickRecords(traceQ.data, "history");
  const selectedTrace = asRecord(traceDetailQ.data);
  const steps = pickRecords(traceDetailQ.data, "steps");
  const approvals = pickRecords(approvalsQ.data, "approvals");

  return (
    <Stack spacing={2}>
      <Box className="list-shell">
        <Typography variant="h6" mb={1}>Trace History</Typography>
        <TableContainer className="table-shell">
          <Table size="small">
            <TableHead><TableRow><TableCell>Message</TableCell><TableCell>Channel</TableCell><TableCell>Status</TableCell><TableCell>Duration</TableCell><TableCell>Started</TableCell><TableCell>Ops</TableCell></TableRow></TableHead>
            <TableBody>
              {history.map((item) => {
                const id = str(item.id, "");
                return (
                  <TableRow key={id}>
                    <TableCell>{str(item.message_preview)}</TableCell>
                    <TableCell>{str(item.channel)}</TableCell>
                    <TableCell>{str(item.status)}</TableCell>
                    <TableCell>{str(item.duration_ms)}</TableCell>
                    <TableCell>{str(item.started_at)}</TableCell>
                    <TableCell align="right">
                      <RowOpsMenu
                        actions={[
                          {
                            label: "View",
                            onClick: () => setSelectedTraceId(id)
                          }
                        ]}
                        ariaLabel="Trace options"
                      />
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </TableContainer>
      </Box>

      <Box className="list-shell">
        <Typography variant="h6" mb={1}>Approval History</Typography>
        {approvals.length === 0 ? (
          <Typography variant="body2" color="text.secondary">No approval events yet.</Typography>
        ) : (
          <TableContainer className="table-shell">
            <Table size="small">
              <TableHead>
                <TableRow>
                  <TableCell>Action</TableCell>
                  <TableCell>Rule</TableCell>
                  <TableCell>Status</TableCell>
                  <TableCell>Requested</TableCell>
                  <TableCell>Resolved By</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {approvals.map((item, idx) => (
                  <TableRow key={str(item.id, `approval-${idx}`)}>
                    <TableCell sx={{ maxWidth: 280 }}>
                      <Typography variant="body2" noWrap title={str(item.action_name, "-")}>
                        {str(item.action_name, "-")}
                      </Typography>
                    </TableCell>
                    <TableCell>{str(item.rule_name, "-")}</TableCell>
                    <TableCell>{str(item.status, "-")}</TableCell>
                    <TableCell>{str(item.requested_at, "-")}</TableCell>
                    <TableCell>{str(item.resolved_by, "-")}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        )}
      </Box>

      {traceQ.error || traceDetailQ.error || approvalsQ.error ? (
        <Alert severity="error">{errMessage(traceQ.error || traceDetailQ.error || approvalsQ.error)}</Alert>
      ) : null}

      <Dialog open={selectedTraceId != null} onClose={() => setSelectedTraceId(null)} maxWidth="md" fullWidth>
        <DialogTitle>Trace Detail</DialogTitle>
        <DialogContent>
          <Stack spacing={1}>
            <Typography variant="caption" color="text.secondary">{str(selectedTrace.started_at)} | {str(selectedTrace.channel)}</Typography>
            <Typography variant="body2" sx={{ whiteSpace: "pre-wrap" }}>{str(selectedTrace.message)}</Typography>
            <Box className="metadata-box" sx={{ maxHeight: 340 }}>
              {steps.length === 0 ? (
                <Typography variant="body2" color="text.secondary">No steps.</Typography>
              ) : (
                <Stack spacing={1}>
                  {steps.map((step, idx) => (
                    <Box key={str(step.id, `step-${idx}`)} className="console-line">
                      <Typography variant="caption" color="text.secondary">{str(step.time)} | {str(step.type)}</Typography>
                      <Typography variant="body2">{str(step.title)}</Typography>
                      <Typography variant="caption" color="text.secondary" sx={{ whiteSpace: "pre-wrap" }}>{str(step.detail)}</Typography>
                    </Box>
                  ))}
                </Stack>
              )}
            </Box>
            {selectedTrace.response ? (
              <>
                <Typography variant="subtitle2">Response</Typography>
                <Typography variant="body2" sx={{ whiteSpace: "pre-wrap" }}>{str(selectedTrace.response)}</Typography>
              </>
            ) : null}
          </Stack>
        </DialogContent>
      </Dialog>
    </Stack>
  );
}

function StatusManager({ autoRefresh }: { autoRefresh: boolean }) {
  const queryClient = useQueryClient();
  const [error, setError] = useState<string | null>(null);

  const statusQ = useQuery({ queryKey: ["status-page-status"], queryFn: () => api.rawGet("/status"), refetchInterval: autoRefresh ? REFRESH_MS : false });
  const profileQ = useQuery({ queryKey: ["status-page-profile"], queryFn: () => api.rawGet("/profile"), refetchInterval: autoRefresh ? REFRESH_MS : false });
  const securityQ = useQuery({ queryKey: ["status-page-security"], queryFn: () => api.rawGet("/security/status"), refetchInterval: autoRefresh ? REFRESH_MS : false });
  const watchersQ = useQuery({ queryKey: ["status-page-watchers"], queryFn: () => api.rawGet("/watchers"), refetchInterval: autoRefresh ? REFRESH_MS : false });
  const securityLogsQ = useQuery({ queryKey: ["status-page-security-logs"], queryFn: () => api.rawGet("/security/logs?limit=20"), refetchInterval: autoRefresh ? REFRESH_MS : false });

  const cancelMutation = useMutation({
    mutationFn: (id: string) => api.rawPost(`/watchers/${encodeURIComponent(id)}/cancel`, {}),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["status-page-watchers"] });
    }
  });

  const status = asRecord(statusQ.data);
  const profile = asRecord(profileQ.data);
  const security = asRecord(securityQ.data);
  const watchers = pickRecords(watchersQ.data, "watchers");

  return (
    <Stack spacing={2}>
      <Grid2 container spacing={2} alignItems="stretch">
        <Grid2 size={{ xs: 12, md: 3 }} sx={{ display: "flex" }}><Box className="list-shell" sx={{ minHeight: 120, height: "100%", width: "100%" }}><Typography variant="caption" color="text.secondary">DID</Typography><Typography variant="body2" sx={{ wordBreak: "break-all" }}>{str(status.did)}</Typography></Box></Grid2>
        <Grid2 size={{ xs: 12, md: 3 }} sx={{ display: "flex" }}><Box className="list-shell" sx={{ minHeight: 120, height: "100%", width: "100%" }}><Typography variant="caption" color="text.secondary">Tasks Pending</Typography><Typography variant="h5">{num(status.tasks_pending)}</Typography></Box></Grid2>
        <Grid2 size={{ xs: 12, md: 3 }} sx={{ display: "flex" }}><Box className="list-shell" sx={{ minHeight: 120, height: "100%", width: "100%" }}><Typography variant="caption" color="text.secondary">Skills Loaded</Typography><Typography variant="h5">{num(status.skills_loaded, num(status.actions_loaded))}</Typography></Box></Grid2>
        <Grid2 size={{ xs: 12, md: 3 }} sx={{ display: "flex" }}><Box className="list-shell" sx={{ minHeight: 120, height: "100%", width: "100%" }}><Typography variant="caption" color="text.secondary">Memory Entries</Typography><Typography variant="h5">{num(status.memory_entries)}</Typography></Box></Grid2>
      </Grid2>

      <Grid2 container spacing={2} alignItems="stretch">
        <Grid2 size={{ xs: 12, lg: 4 }} sx={{ display: "flex" }}>
          <Box className="list-shell" sx={{ height: "100%", width: "100%" }}>
            <Typography variant="h6" mb={1}>Profile</Typography>
            <Stack spacing={0.5}>
              <Typography variant="body2">Name: {str(profile.name, "-")}</Typography>
              <Typography variant="body2">Location: {str(profile.location, "-")}</Typography>
              <Typography variant="body2">Timezone: {str(profile.timezone, "-")}</Typography>
              <Typography variant="body2">Language: {str(profile.language, "-")}</Typography>
              <Typography variant="body2">Tone: {str(profile.tone, "-")}</Typography>
            </Stack>
          </Box>
        </Grid2>
        <Grid2 size={{ xs: 12, lg: 4 }} sx={{ display: "flex" }}>
          <Box className="list-shell" sx={{ height: "100%", width: "100%" }}>
            <Typography variant="h6" mb={1}>Security</Typography>
            <Stack spacing={0.5}>
              <Typography variant="body2">Mode: {str(security.encryption_mode)}</Typography>
              {toBool(security.using_default) ? (
                <Typography variant="body2" color="warning.main">Using default password — set a custom one in Settings.</Typography>
              ) : (
                <Typography variant="body2" color="success.main">Custom master password active.</Typography>
              )}
            </Stack>
          </Box>
        </Grid2>
        <Grid2 size={{ xs: 12, lg: 4 }} sx={{ display: "flex" }}>
          <Box className="list-shell" sx={{ height: "100%", width: "100%" }}>
            <Typography variant="h6" mb={1}>Watchers</Typography>
            {watchers.length === 0 ? (
              <Typography variant="body2" color="text.secondary">No active watchers.</Typography>
            ) : (
              <Stack spacing={1}>
                {watchers.map((w) => {
                  const id = str(w.id, "");
                  return (
                    <Box key={id} className="action-row">
                      <Stack direction="row" justifyContent="space-between" alignItems="center" spacing={1}>
                        <Stack>
                          <Typography variant="body2">{str(w.description)}</Typography>
                          <Typography variant="caption" color="text.secondary">{str(w.status)} | {str(w.interval_secs)}s</Typography>
                        </Stack>
                        <Button size="small" color="warning" onClick={async () => { setError(null); try { await cancelMutation.mutateAsync(id); } catch (e) { setError(errMessage(e)); } }}>Cancel</Button>
                      </Stack>
                    </Box>
                  );
                })}
              </Stack>
            )}
          </Box>
        </Grid2>
      </Grid2>

      <QueryTable title="Security Logs" path="/security/logs?limit=20" arrayKey="logs" columns={["event_type", "severity", "message", "source", "created_at", "count"]} autoRefresh={autoRefresh} emptyLabel="No security logs yet." queryKey="security-logs-table" />

      {statusQ.error || profileQ.error || securityQ.error || watchersQ.error || securityLogsQ.error || error ? (
        <Alert severity="error">{error || errMessage(statusQ.error || profileQ.error || securityQ.error || watchersQ.error || securityLogsQ.error)}</Alert>
      ) : null}
    </Stack>
  );
}

function SettingsManager({ autoRefresh }: { autoRefresh: boolean }) {
  const queryClient = useQueryClient();
  const [tab, setTab] = useState(() => {
    if (typeof window === "undefined") return 0;
    const raw = new URLSearchParams(window.location.search).get("settings_tab");
    if (!raw) return 0;
    const normalized = raw.trim().toLowerCase();
    const byName: Record<string, number> = {
      quick: 0,
      setup: 0,
      models: 1,
      channels: 2,
      integrations: 2,
      media: 3,
      security: 4,
      advanced: 5,
      analytics: 6,
      moltbook: 7,
      mcp: 8,
      memory: 12,
      system: 9,
      trace: 11
    };
    if (normalized in byName) return byName[normalized];
    const asNumber = Number(normalized);
    if (Number.isFinite(asNumber) && Math.trunc(asNumber) === 10) return 2;
    return Number.isFinite(asNumber) ? Math.max(0, Math.trunc(asNumber)) : 0;
  });
  const [dirty, setDirty] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [modelConnectivityWarning, setModelConnectivityWarning] = useState<string | null>(null);
  const [initialized, setInitialized] = useState(false);
  const [apiKeyRevealed, setApiKeyRevealed] = useState(false);
  const [apiKeyNowMs, setApiKeyNowMs] = useState(() => Date.now());
  const [secCurrentPassword, setSecCurrentPassword] = useState("");
  const [secNewPassword, setSecNewPassword] = useState("");
  const [secConfirmPassword, setSecConfirmPassword] = useState("");
  const [showPasswordInputs, setShowPasswordInputs] = useState(false);
  const [passwordDialogMode, setPasswordDialogMode] = useState<PasswordDialogMode | null>(null);
  const [vaultPassword, setVaultPassword] = useState("");
  const [vaultRevealedValues, setVaultRevealedValues] = useState<Record<string, string>>({});
  const [vaultEditorOpen, setVaultEditorOpen] = useState(false);
  const [vaultEditorMode, setVaultEditorMode] = useState<VaultEditorMode>("add");
  const [vaultEditorKey, setVaultEditorKey] = useState("");
  const [vaultEditorValue, setVaultEditorValue] = useState("");
  const [showVaultSecretValue, setShowVaultSecretValue] = useState(false);
  const [selectedPulseEvent, setSelectedPulseEvent] = useState<JsonRecord | null>(null);
  const [selectedMoltbookEvent, setSelectedMoltbookEvent] = useState<JsonRecord | null>(null);
  const [developerModeEnabled, setDeveloperModeEnabledState] = useState(getDeveloperModeEnabled);
  const [trustPresetId, setTrustPresetId] = useState(TRUST_APPROVAL_PRESETS[0]?.id ?? "run_terminal_command");
  const [trustPresetDetail, setTrustPresetDetail] = useState("ls -la");
  const [trustUseAdvancedInput, setTrustUseAdvancedInput] = useState(false);
  const [trustActionKind, setTrustActionKind] = useState("shell");
  const [trustPayloadJson, setTrustPayloadJson] = useState("{}");
  const [trustResult, setTrustResult] = useState<JsonRecord | null>(null);

  useEffect(() => {
    const refreshDeveloperMode = () => setDeveloperModeEnabledState(getDeveloperModeEnabled());
    window.addEventListener(DEVELOPER_MODE_EVENT, refreshDeveloperMode as EventListener);
    window.addEventListener("storage", refreshDeveloperMode);
    return () => {
      window.removeEventListener(DEVELOPER_MODE_EVENT, refreshDeveloperMode as EventListener);
      window.removeEventListener("storage", refreshDeveloperMode);
    };
  }, []);

  useEffect(() => {
    if (!success) return;
    const timer = window.setTimeout(() => setSuccess(null), 3500);
    return () => window.clearTimeout(timer);
  }, [success]);

  useEffect(() => {
    const timer = window.setInterval(() => setApiKeyNowMs(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);

  const settingsQ = useQuery({
    queryKey: ["settings"],
    queryFn: () => api.rawGet("/settings"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const mediaQ = useQuery({
    queryKey: ["settings-media"],
    queryFn: () => api.rawGet("/settings/media"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const modelsQ = useQuery({
    queryKey: ["models"],
    queryFn: () => api.rawGet("/models"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const apiKeyQ = useQuery({
    queryKey: ["settings-api-key"],
    queryFn: () => api.rawGet("/settings/api-key"),
    refetchInterval: 10000,
    refetchIntervalInBackground: true
  });
  const tunnelQ = useQuery({
    queryKey: ["tunnel-status"],
    queryFn: () => api.rawGet("/tunnel/status"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const securityStatusQ = useQuery({
    queryKey: ["security-status"],
    queryFn: () => api.rawGet("/security/status"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const vaultSecretsQ = useQuery({
    queryKey: ["settings-secrets"],
    queryFn: () => api.rawGet("/settings/secrets"),
    refetchInterval: false
  });
  const pulseQ = useQuery({
    queryKey: ["arkpulse-log"],
    queryFn: () => api.rawGet("/arkpulse?limit=40"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const moltbookStatusQ = useQuery({
    queryKey: ["moltbook-status"],
    queryFn: () => api.rawGet("/moltbook/status"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const moltbookLogQ = useQuery({ 
    queryKey: ["moltbook-log"], 
    queryFn: () => api.rawGet("/moltbook/log?limit=40"), 
    refetchInterval: autoRefresh ? REFRESH_MS : false 
  }); 
 
  const llmAnalyticsHourQ = useQuery({
    queryKey: ["llm-analytics", "24h", "hour"],
    queryFn: () => api.getLlmAnalytics({ range: "24h", bucket: "hour" }),
    refetchInterval: autoRefresh ? 30000 : false
  });
  const llmAnalyticsDayQ = useQuery({
    queryKey: ["llm-analytics", "30d", "day"],
    queryFn: () => api.getLlmAnalytics({ range: "30d", bucket: "day" }),
    refetchInterval: autoRefresh ? 120000 : false
  });
  const llmAnalyticsWeekQ = useQuery({
    queryKey: ["llm-analytics", "90d", "week"],
    queryFn: () => api.getLlmAnalytics({ range: "90d", bucket: "week" }),
    refetchInterval: autoRefresh ? 300000 : false
  });

  const settings = asRecord(settingsQ.data);
  const media = asRecord(mediaQ.data);
  const modelsPayload = asRecord(modelsQ.data);

  const configuredProviders = useMemo(() => { 
    const raw = media.configured; 
    if (!Array.isArray(raw)) return []; 
    return raw.filter((x) => typeof x === "string") as string[]; 
  }, [media.configured]); 
 
  const llmHour = llmAnalyticsHourQ.data as LlmAnalyticsResponse | undefined;
  const llmDay = llmAnalyticsDayQ.data as LlmAnalyticsResponse | undefined;
  const llmWeek = llmAnalyticsWeekQ.data as LlmAnalyticsResponse | undefined;
 
  const analyticsOption = useMemo(() => {
    const build = (resp: LlmAnalyticsResponse | undefined, label: string) => {
      const series = resp?.series || [];
      const x = series.map((p) => {
        const s = p.bucket_start || "";
        if (resp?.range?.bucket === "hour") return s.slice(11, 16);
        if (resp?.range?.bucket === "day") return s.slice(0, 10);
        return s.slice(0, 10);
      });
      const yTokens = series.map((p) => p.total_tokens || 0);
      const yCost = series.map((p) => (typeof p.cost_usd === "number" ? p.cost_usd : null));
      const hasCost = yCost.some((v) => typeof v === "number");
      return {
        backgroundColor: "transparent",
        textStyle: { color: "#9bb4d6" },
        animationDuration: 650,
        animationDurationUpdate: 420,
        grid: { left: 40, right: hasCost ? 54 : 20, top: 34, bottom: 30, containLabel: true },
        title: { text: label, left: 6, top: 0, textStyle: { color: "#cce3ff", fontSize: 12 } },
        tooltip: { trigger: "axis" },
        xAxis: {
          type: "category",
          data: x,
          axisLine: { lineStyle: { color: "rgba(155,180,214,0.35)" } },
          axisLabel: { color: "#8ea9cf" }
        },
        yAxis: [
          {
            type: "value",
            axisLine: { lineStyle: { color: "rgba(155,180,214,0.35)" } },
            splitLine: { lineStyle: { color: "rgba(155,180,214,0.12)" } },
            axisLabel: { color: "#8ea9cf" }
          },
          ...(hasCost
            ? [
                {
                  type: "value",
                  axisLine: { lineStyle: { color: "rgba(155,180,214,0.25)" } },
                  splitLine: { show: false },
                  axisLabel: { color: "#8ea9cf", formatter: (v: number) => `$${Number(v).toFixed(2)}` }
                }
              ]
            : [])
        ],
        series: [
          {
            name: "Tokens",
            type: "line",
            smooth: true,
            data: yTokens,
            lineStyle: { width: 2, color: "#2fd4ff" },
            areaStyle: { color: "rgba(47, 212, 255, 0.18)" },
            itemStyle: { color: "#14f195" }
          },
          ...(hasCost
            ? [
                {
                  name: "Cost (USD)",
                  type: "line",
                  smooth: true,
                  yAxisIndex: 1,
                  data: yCost,
                  lineStyle: { width: 2, color: "rgba(255, 193, 7, 0.9)" },
                  itemStyle: { color: "rgba(255, 193, 7, 0.9)" }
                }
              ]
            : [])
        ]
      };
    };
    return {
      hour: build(llmHour, "Last 24h (hourly)"),
      day: build(llmDay, "Last 30d (daily)"),
      week: build(llmWeek, "Last 90d (weekly)"),
    };
  }, [llmHour, llmDay, llmWeek]);

  const [form, setForm] = useState({
    bot_name: "AgentArk",
    personality: "friendly",
    timezone: "",
    language: "English",
    tone: "",
    email_format: "",
    daily_brief_channel: "telegram",
    smart_routing: true,

    llm_provider: "ollama",
    llm_model: "",
    llm_base_url: "http://localhost:11434",
    llm_api_key: "",

    llm_fallback_provider: "",
    llm_fallback_model: "",
    llm_fallback_base_url: "",
    llm_fallback_api_key: "",

    telegram_enabled: false,
    telegram_bot_token: "",
    telegram_allowed_users_csv: "",

    whatsapp_enabled: false,
    whatsapp_mode: "baileys",
    whatsapp_access_token: "",
    whatsapp_phone_number_id: "",
    whatsapp_verify_token: "agentark_verify",
    whatsapp_bridge_url: "http://127.0.0.1:8999",
    whatsapp_dm_policy: "pairing",
    whatsapp_allowed_numbers_csv: "",

    auto_approve_csv: "",

    default_image_provider: "",
    image_model: "",
    fallback_image_provider: "",
    default_video_provider: "",
    fallback_video_provider: "",
    media_provider_keys_json: "",
    media_key_replicate: "",
    media_key_fal: "",
    media_key_stability_ai: "",
    media_key_together: "",
    media_key_openai_dalle: "",
    media_key_google_gemini: "",
    media_key_runway: "",
    media_key_luma: "",

    search_primary: "playwright",
    search_fallback1: "duckduckgo",
    search_fallback2: "none",
    search_serper_key: "",
    search_searxng_url: "",
    search_brave_key: "",

    moltbook_api_key: "",
    moltbook_enabled: false,
    moltbook_mode: "read_only",
    moltbook_sync_frequency: "twice_daily",
    moltbook_write_enabled: false,
    moltbook_defer_when_busy: true
  });

  function setField<K extends keyof typeof form>(key: K, value: (typeof form)[K]) {
    setForm((prev) => ({ ...prev, [key]: value }));
    setDirty(true);
    setSuccess(null);
  }

  function parseCsvList(csv: string): string[] {
    return csv
      .split(/[,\\n]/g)
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
  }

  function parseTelegramUsers(csv: string): number[] {
    const parts = parseCsvList(csv);
    const out: number[] = [];
    for (const p of parts) {
      const n = Number(p);
      if (!Number.isFinite(n)) throw new Error(`Invalid Telegram user id: '${p}'`);
      out.push(n);
    }
    return out;
  }

  function parseMediaProvidersJson(input: string): Record<string, string> {
    const trimmed = input.trim();
    if (!trimmed) return {};
    let parsed: unknown;
    try {
      parsed = JSON.parse(trimmed);
    } catch {
      throw new Error("Media provider keys must be valid JSON (object mapping provider -> api_key).");
    }
    if (!isRecord(parsed)) throw new Error("Media provider keys must be a JSON object.");
    const out: Record<string, string> = {};
    for (const [k, v] of Object.entries(parsed)) {
      if (typeof v !== "string") throw new Error(`Media provider key for '${k}' must be a string.`);
      out[k] = v;
    }
    return out;
  }

  function hydrateFromServer() {
    const tgUsers = Array.isArray(settings.telegram_allowed_users) ? (settings.telegram_allowed_users as unknown[]) : [];
    const waNums = Array.isArray(settings.whatsapp_allowed_numbers) ? (settings.whatsapp_allowed_numbers as unknown[]) : [];
    const autoApprove = Array.isArray(settings.auto_approve) ? (settings.auto_approve as unknown[]) : [];

    setForm((prev) => ({
      ...prev,
      bot_name: str(settings.bot_name, prev.bot_name),
      personality: str(settings.personality, prev.personality),
      timezone: str(settings.timezone, ""),
      language: str(settings.language, prev.language),
      tone: str(settings.tone, prev.tone),
      email_format: str(settings.email_format, prev.email_format),
      daily_brief_channel: str(settings.daily_brief_channel, "telegram"),
      smart_routing: toBool(settings.smart_routing),

      llm_provider: str(settings.llm_provider, "ollama"),
      llm_model: str(settings.llm_model, ""),
      llm_base_url: str(settings.llm_base_url, "http://localhost:11434"),
      llm_api_key: "",

      llm_fallback_provider: str(settings.llm_fallback_provider, ""),
      llm_fallback_model: str(settings.llm_fallback_model, ""),
      llm_fallback_base_url: str(settings.llm_fallback_base_url, ""),
      llm_fallback_api_key: "",

      telegram_enabled: toBool(settings.telegram_enabled),
      telegram_bot_token: "",
      telegram_allowed_users_csv: tgUsers
        .map((v) => (typeof v === "number" ? String(v) : typeof v === "string" ? v : ""))
        .filter((v) => v.trim().length > 0)
        .join(", "),

      whatsapp_enabled: toBool(settings.whatsapp_enabled),
      whatsapp_mode: str(settings.whatsapp_mode, "baileys"),
      whatsapp_access_token: "",
      whatsapp_phone_number_id: str(settings.whatsapp_phone_number_id, ""),
      whatsapp_verify_token: str(settings.whatsapp_verify_token, "agentark_verify"),
      whatsapp_bridge_url: str(settings.whatsapp_bridge_url, "http://127.0.0.1:8999"),
      whatsapp_dm_policy: str(settings.whatsapp_dm_policy, "pairing"),
      whatsapp_allowed_numbers_csv: waNums
        .map((v) => (typeof v === "string" ? v : ""))
        .filter((v) => v.trim().length > 0)
        .join(", "),

      auto_approve_csv: autoApprove
        .map((v) => (typeof v === "string" ? v : ""))
        .filter((v) => v.trim().length > 0)
        .join(", "),

      default_image_provider: str(media.default_image_provider ?? settings.default_image_provider, ""),
      image_model: str(media.image_model ?? settings.image_model, ""),
      fallback_image_provider: str(media.fallback_image_provider ?? settings.fallback_image_provider, ""),
      default_video_provider: str(media.default_video_provider ?? settings.default_video_provider, ""),
      fallback_video_provider: str(media.fallback_video_provider ?? settings.fallback_video_provider, ""),
      media_provider_keys_json: "",
      media_key_replicate: "",
      media_key_fal: "",
      media_key_stability_ai: "",
      media_key_together: "",
      media_key_openai_dalle: "",
      media_key_google_gemini: "",
      media_key_runway: "",
      media_key_luma: "",

      search_primary: str(settings.search_primary, "playwright"),
      search_fallback1: str(settings.search_fallback1, "duckduckgo"),
      search_fallback2: str(settings.search_fallback2, "none"),
      search_serper_key: "",
      search_searxng_url: str(settings.search_searxng_url, ""),
      search_brave_key: "",

      moltbook_api_key: "",
      moltbook_enabled: toBool(settings.moltbook_enabled),
      moltbook_mode: str(settings.moltbook_mode, "read_only"),
      moltbook_sync_frequency: str(settings.moltbook_sync_frequency, "twice_daily"),
      moltbook_write_enabled: toBool(settings.moltbook_write_enabled),
      moltbook_defer_when_busy: toBool(settings.moltbook_defer_when_busy)
    }));

    setDirty(false);
    setError(null);
    setSuccess(null);
  }

  // Initialize form from backend once; keep defaults if backend is down.
  useEffect(() => {
    if (initialized) return;
    if (!settingsQ.isSuccess) return;
    hydrateFromServer();
    setInitialized(true);
    setDirty(false);
  }, [initialized, settingsQ.isSuccess, settingsQ.dataUpdatedAt]);

  useEffect(() => {
    if (initialized) return;
    if (!settingsQ.data || !mediaQ.data) return;
    hydrateFromServer();
    setInitialized(true);
  }, [initialized, settingsQ.data, mediaQ.data]); // eslint-disable-line react-hooks/exhaustive-deps

  const saveMutation = useMutation({
    mutationFn: async () => {
      const mediaKeys = parseMediaProvidersJson(form.media_provider_keys_json);
      const mediaProviders: Record<string, string> = { ...mediaKeys };
      const mediaFieldKeys: Array<[string, string]> = [
        ["replicate", form.media_key_replicate],
        ["fal", form.media_key_fal],
        ["stability_ai", form.media_key_stability_ai],
        ["together", form.media_key_together],
        ["openai_dalle", form.media_key_openai_dalle],
        ["google_gemini", form.media_key_google_gemini],
        ["runway", form.media_key_runway],
        ["luma", form.media_key_luma]
      ];
      for (const [k, v] of mediaFieldKeys) {
        const trimmed = (v || "").trim();
        if (trimmed) {
          mediaProviders[k] = trimmed;
          if (k === "openai_dalle") mediaProviders["openai_sora"] = trimmed;
          if (k === "google_gemini") mediaProviders["google_veo"] = trimmed;
        }
      }
      const payload: Record<string, unknown> = {
        bot_name: form.bot_name || "AgentArk",
        personality: form.personality || "friendly",
        // Send empty strings to clear fields (null means "skip update" on backend).
        timezone: form.timezone,
        language: form.language,
        tone: form.tone,
        email_format: form.email_format,
        daily_brief_channel: form.daily_brief_channel || "telegram",
        smart_routing: form.smart_routing,

        llm_provider: form.llm_provider,
        llm_model: form.llm_model,
        llm_base_url: form.llm_base_url || null,
        llm_api_key: form.llm_api_key || null,

        llm_fallback_provider: form.llm_fallback_provider || null,
        llm_fallback_model: form.llm_fallback_model || null,
        llm_fallback_base_url: form.llm_fallback_base_url || null,
        llm_fallback_api_key: form.llm_fallback_api_key || null,

        telegram_enabled: !!form.telegram_enabled,
        telegram_bot_token: form.telegram_bot_token || null,
        telegram_allowed_users: parseTelegramUsers(form.telegram_allowed_users_csv),

        whatsapp_enabled: !!form.whatsapp_enabled,
        whatsapp_mode: form.whatsapp_mode || null,
        whatsapp_access_token: form.whatsapp_access_token || null,
        whatsapp_phone_number_id: form.whatsapp_phone_number_id || null,
        whatsapp_verify_token: form.whatsapp_verify_token || null,
        whatsapp_bridge_url: form.whatsapp_bridge_url || null,
        whatsapp_dm_policy: form.whatsapp_dm_policy || null,
        whatsapp_allowed_numbers: parseCsvList(form.whatsapp_allowed_numbers_csv),

        auto_approve: parseCsvList(form.auto_approve_csv),

        media_providers: mediaProviders,
        default_image_provider: form.default_image_provider || null,
        image_model: form.image_model || null,
        fallback_image_provider: form.fallback_image_provider || null,
        default_video_provider: form.default_video_provider || null,
        fallback_video_provider: form.fallback_video_provider || null,

        search_primary: form.search_primary || null,
        search_fallback1: form.search_fallback1 || null,
        search_fallback2: form.search_fallback2 || null,
        search_serper_key: form.search_serper_key || null,
        search_searxng_url: form.search_searxng_url || null,
        search_brave_key: form.search_brave_key || null,

        moltbook_api_key: form.moltbook_api_key || null,
        moltbook_enabled: form.moltbook_enabled,
        moltbook_mode: form.moltbook_mode || null,
        moltbook_sync_frequency: form.moltbook_sync_frequency || null,
        moltbook_write_enabled: form.moltbook_write_enabled,
        moltbook_defer_when_busy: form.moltbook_defer_when_busy
      };

      return await api.rawPost("/settings", payload);
    },
    onSuccess: async () => {
      setError(null);
      setSuccess("Saved settings.");
      setDirty(false);
      setForm((prev) => ({
        ...prev,
        llm_api_key: "",
        llm_fallback_api_key: "",
        telegram_bot_token: "",
        whatsapp_access_token: "",
        media_provider_keys_json: "",
        media_key_replicate: "",
        media_key_fal: "",
        media_key_stability_ai: "",
        media_key_together: "",
        media_key_openai_dalle: "",
        media_key_google_gemini: "",
        media_key_runway: "",
        media_key_luma: "",
        search_serper_key: "",
        search_brave_key: ""
      }));
      await queryClient.invalidateQueries({ queryKey: ["settings"] });
      await queryClient.invalidateQueries({ queryKey: ["settings-media"] });
      await queryClient.invalidateQueries({ queryKey: ["models"] });
    },
    onError: (e) => {
      setSuccess(null);
      setError(errMessage(e));
    }
  });

  const runMoltbookMutation = useMutation({
    mutationFn: () => api.rawPost("/moltbook/run", {}),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["moltbook-log"] });
      await queryClient.invalidateQueries({ queryKey: ["moltbook-status"] });
    }
  });

  const modelSlots = useMemo(() => pickRecords(modelsPayload, "models"), [modelsPayload]);
  const moltbookEvents = pickRecords(moltbookLogQ.data, "events");

  const [modelDialogOpen, setModelDialogOpen] = useState(false);
  const [modelEditingId, setModelEditingId] = useState<string | null>(null);
  const [modelAdvancedOpen, setModelAdvancedOpen] = useState(false);
  const [modelForm, setModelForm] = useState({
    label: "",
    role: "primary",
    provider: "ollama",
    model: "",
    base_url: OLLAMA_DEFAULT_BASE_URL,
    api_key: "",
    enabled: true
  });
  const previousModelProviderRef = useRef(modelForm.provider);

  useEffect(() => {
    if (modelForm.role !== "research") return;
    setModelForm((p) => ({
      ...p,
      provider: "openrouter",
      model: p.model || "perplexity/sonar-deep-research",
      base_url: p.base_url || OPENROUTER_DEFAULT_BASE_URL
    }));
  }, [modelForm.role]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    const prevProvider = previousModelProviderRef.current;
    if (prevProvider === modelForm.provider) return;
    previousModelProviderRef.current = modelForm.provider;

    setModelForm((p) => {
      const current = p.base_url.trim();
      let next = p.base_url;
      if (p.provider === "openrouter") {
        if (!current || current === OLLAMA_DEFAULT_BASE_URL) next = OPENROUTER_DEFAULT_BASE_URL;
      } else if (p.provider === "ollama") {
        if (!current || current === OPENROUTER_DEFAULT_BASE_URL) next = OLLAMA_DEFAULT_BASE_URL;
      } else if (
        (p.provider === "openai" || p.provider === "anthropic") &&
        (current === OLLAMA_DEFAULT_BASE_URL || current === OPENROUTER_DEFAULT_BASE_URL)
      ) {
        next = "";
      }
      return next === p.base_url ? p : { ...p, base_url: next };
    });
  }, [modelForm.provider]);

  function openAddModel() {
    setModelEditingId(null);
    setModelAdvancedOpen(false);
    setModelConnectivityWarning(null);
    setModelForm({
      label: "",
      role: "primary",
      provider: "ollama",
      model: "",
      base_url: OLLAMA_DEFAULT_BASE_URL,
      api_key: "",
      enabled: true
    });
    setModelDialogOpen(true);
  }

  function openEditModel(slot: JsonRecord) {
    setModelEditingId(str(slot.id, ""));
    setModelAdvancedOpen(false);
    setModelConnectivityWarning(null);
    setModelForm({
      label: str(slot.label, ""),
      role: str(slot.role, "primary"),
      provider: str(slot.provider, "ollama"),
      model: str(slot.model, ""),
      base_url: str(slot.base_url, ""),
      api_key: "",
      enabled: toBool(slot.enabled)
    });
    setModelDialogOpen(true);
  }

  const saveModelMutation = useMutation({
    mutationFn: async () => {
      const provider = modelForm.provider;
      const baseUrl = modelForm.base_url.trim();
      const normalizedBaseUrl =
        provider === "openrouter"
          ? baseUrl || OPENROUTER_DEFAULT_BASE_URL
          : provider === "ollama"
            ? baseUrl || OLLAMA_DEFAULT_BASE_URL
            : provider === "openai-compatible"
              ? baseUrl
              : "";
      const payload: Record<string, unknown> = {
        label: modelForm.label.trim(),
        role: modelForm.role,
        provider,
        model: modelForm.model.trim(),
        base_url: normalizedBaseUrl || null,
        api_key: modelForm.api_key.trim() || null,
        enabled: modelForm.enabled
      };

      if (!payload.label || !payload.model) throw new Error("Label and model are required.");

      if (modelEditingId) {
        const response = asRecord(await api.rawPut(`/models/${encodeURIComponent(modelEditingId)}`, payload));
        const connectivityRaw = response.connectivity;
        const hasConnectivity = connectivityRaw !== undefined && connectivityRaw !== null;
        const connectivity = asRecord(connectivityRaw);
        return {
          connectivityOk: hasConnectivity ? toBool(connectivity.ok) : true,
          connectivityError: hasConnectivity ? str(connectivity.error, "").trim() : ""
        };
      }
      const response = asRecord(await api.rawPost("/models", payload));
      const connectivityRaw = response.connectivity;
      const hasConnectivity = connectivityRaw !== undefined && connectivityRaw !== null;
      const connectivity = asRecord(connectivityRaw);
      return {
        connectivityOk: hasConnectivity ? toBool(connectivity.ok) : true,
        connectivityError: hasConnectivity ? str(connectivity.error, "").trim() : ""
      };
    },
    onSuccess: async (result: { connectivityOk: boolean; connectivityError: string }) => {
      setModelDialogOpen(false);
      if (!result.connectivityOk) {
        setModelConnectivityWarning(
          `Model saved, but connection test failed: ${result.connectivityError || "could not reach provider"}. Runs may fail until fixed.`
        );
      } else {
        setModelConnectivityWarning(null);
      }
      await queryClient.invalidateQueries({ queryKey: ["models"] });
      await queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
    onError: (e) => setError(errMessage(e))
  });

  const deleteModelMutation = useMutation({
    mutationFn: (id: string) => api.rawDelete(`/models/${encodeURIComponent(id)}`),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["models"] });
      await queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
    onError: (e) => setError(errMessage(e))
  });

  const toggleModelEnabledMutation = useMutation({
    mutationFn: async (slot: JsonRecord) => {
      const id = str(slot.id, "");
      const payload: Record<string, unknown> = {
        label: str(slot.label, ""),
        role: str(slot.role, "primary"),
        provider: str(slot.provider, "ollama"),
        model: str(slot.model, ""),
        base_url: str(slot.base_url, "") || null,
        enabled: !toBool(slot.enabled)
      };
      return await api.rawPut(`/models/${encodeURIComponent(id)}`, payload);
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["models"] });
      await queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
    onError: (e) => setError(errMessage(e))
  });

  const hasTelegramToken = toBool(settings.has_telegram_token);
  const hasWhatsAppToken = toBool(settings.has_whatsapp_token);
  const hasPrimaryApiKey = toBool(settings.has_api_key);
  const hasFallbackApiKey = toBool(settings.has_fallback_api_key);
  const settingsComplete = toBool(settings.settings_complete);

  const apiKeyPayload = asRecord(apiKeyQ.data);
  const apiKeyIssuedAtUnix = num(apiKeyPayload.issued_at_unix, 0);
  const apiKeyExpiresAtUnix = num(apiKeyPayload.expires_at_unix, 0);
  const apiKeyRemainingFromServer = num(apiKeyPayload.remaining_seconds, 0);
  const apiKeyRemainingSeconds = useMemo(() => {
    if (apiKeyExpiresAtUnix > 0) {
      return Math.max(0, apiKeyExpiresAtUnix - Math.floor(apiKeyNowMs / 1000));
    }
    return Math.max(0, apiKeyRemainingFromServer);
  }, [apiKeyExpiresAtUnix, apiKeyNowMs, apiKeyRemainingFromServer]);
  const apiKeyRotated = toBool(apiKeyPayload.rotated);
  const tunnel = asRecord(tunnelQ.data);
  const sec = asRecord(securityStatusQ.data);
  const hasCustomMasterPassword = toBool(sec.master_password_set) && !toBool(sec.using_default);
  const vaultSecrets = pickRecords(vaultSecretsQ.data, "entries");
  const pulseEvents = pickRecords(pulseQ.data, "events");
  const pulseMeta = asRecord(pulseQ.data);
  const pulseRunning = toBool(pulseMeta.running);
  const moltbookStatus = asRecord(moltbookStatusQ.data);
  const moltbookRunning = toBool(moltbookStatus.running);
  const moltbookLastStatus = str(moltbookStatus.last_status, "").toLowerCase();
  const moltbookNeedsConnection =
    moltbookLastStatus === "not_connected" ||
    moltbookLastStatus === "not_configured" ||
    moltbookLastStatus === "error";

  const selectedPulseDetails = asRecord(selectedPulseEvent?.details);
  const selectedPulseFindings = Array.isArray(selectedPulseDetails.doctor_findings)
    ? (selectedPulseDetails.doctor_findings as unknown[])
    : [];
  const selectedPulseScore = num(selectedPulseDetails.doctor_score, -1);
  const selectedPulseStatus = str(selectedPulseEvent?.status, "-");
  const selectedPulseStatusOk = selectedPulseStatus.toLowerCase() === "ok";
  const selectedPulseTimestampRaw = str(selectedPulseEvent?.timestamp, "-");
  const selectedPulseCaptured = looksLikeIsoTimestamp(selectedPulseTimestampRaw)
    ? formatTimestampForHumans(selectedPulseTimestampRaw)
    : { label: selectedPulseTimestampRaw, tooltip: selectedPulseTimestampRaw };
  const selectedPulseGuidance = (() => {
    if (selectedPulseFindings.length === 0 && (selectedPulseStatusOk || selectedPulseScore >= 90)) {
      return {
        severity: "success" as const,
        title: "System health looks good.",
        detail: "No active issues were detected in this run."
      };
    }
    if (selectedPulseFindings.length > 0) {
      const issueLabel = selectedPulseFindings.length === 1 ? "issue" : "issues";
      return {
        severity: "warning" as const,
        title: `${selectedPulseFindings.length} ${issueLabel} need attention.`,
        detail: "Use the fix command under each issue, then run ArkPulse again."
      };
    }
    return {
      severity: "info" as const,
      title: "No direct findings were returned.",
      detail: "Review the snapshot for context and run another check after changes."
    };
  })();
  const selectedPulseSnapshot: { label: string; value: string }[] = [
    { label: "Pending tasks", value: String(num(selectedPulseDetails.pending_tasks, 0)) },
    { label: "Running tasks", value: String(num(selectedPulseDetails.running_tasks, 0)) },
    { label: "Completed tasks", value: String(num(selectedPulseDetails.completed_tasks, 0)) },
    { label: "Deployed apps", value: String(Array.isArray(selectedPulseDetails.deployed_apps) ? selectedPulseDetails.deployed_apps.length : 0) },
    { label: "Health checks", value: String(Array.isArray(selectedPulseDetails.health_checks) ? selectedPulseDetails.health_checks.length : 0) },
    { label: "Memories", value: String(num(selectedPulseDetails.total_memories, 0)) },
    { label: "Watchers", value: String(num(selectedPulseDetails.active_watchers, 0)) },
    { label: "Uptime", value: formatDurationFromSeconds(selectedPulseDetails.uptime_secs) }
  ];
  const latestPulseEvent = asRecord(pulseEvents[0]);
  const latestPulseDetails = asRecord(latestPulseEvent.details);
  const latestPulseFindingsCount = Array.isArray(latestPulseDetails.doctor_findings)
    ? (latestPulseDetails.doctor_findings as unknown[]).length
    : 0;
  const latestPulseScore = num(latestPulseDetails.doctor_score, -1);
  const latestPulseStatus = str(latestPulseEvent.status, "").toLowerCase();
  const latestPulseHeadline =
    pulseRunning
      ? "ArkPulse is currently running."
      : pulseEvents.length === 0
      ? "No health checks yet."
      : latestPulseFindingsCount > 0
      ? `${latestPulseFindingsCount} issue${latestPulseFindingsCount === 1 ? "" : "s"} need attention.`
      : latestPulseStatus === "ok" || latestPulseScore >= 90
      ? "System health looks good."
      : "Health check completed.";
  const latestPulseSubtitle =
    pulseRunning
      ? "Please wait for this run to finish before starting another."
      : pulseEvents.length === 0
      ? "Click Run now to generate your first ArkPulse report."
      : latestPulseFindingsCount > 0
      ? "Open the latest report and start with Fix #1."
      : "No urgent action needed right now.";

  function severityChipColor(sev: string): "error" | "warning" | "info" | "success" | "default" {
    const s = (sev || "").toLowerCase();
    if (s === "critical" || s === "high" || s === "error") return "error";
    if (s === "medium" || s === "warn" || s === "warning") return "warning";
    if (s === "low") return "info";
    if (s === "ok" || s === "info") return "success";
    return "default";
  }

  function moltbookTriggerLabel(raw: string): string {
    const t = (raw || "").toLowerCase();
    if (t === "manual") return "Manual";
    if (t === "scheduler") return "Scheduled";
    return raw || "-";
  }

  function moltbookActionLabel(action: string, details: JsonRecord): string {
    const a = (action || "").toLowerCase();
    if (a === "skipped_disabled") return "Skipped: Disabled";
    if (a === "skipped_off_mode") return "Skipped: Mode off";
    if (a === "deferred_busy") return "Deferred: Busy";
    if (a === "skipped_busy_max_defers") return "Skipped: Busy (max defers)";
    if (a === "not_connected") return "Not connected";
    if (a === "run_started") return "Run started";
    if (a === "run_completed") return "Run completed";
    if (a === "status_checked") return "Status checked";
    if (a === "feed_fetched" || a === "feed_read") return "Feed fetched";
    if (a === "post_created") return "Post created";
    if (a.startsWith("error_")) return `Error: ${action}`;
    // Fall back to the raw action code.
    return action || "-";
  }

  function moltbookReason(action: string, details: JsonRecord): string | null {
    const explicit = str(details.reason, "").trim();
    if (explicit) return explicit;
    const a = (action || "").toLowerCase();
    if (a === "skipped_disabled") return "Moltbook is disabled in Settings.";
    if (a === "skipped_off_mode") return "Moltbook mode is set to off.";
    if (a === "deferred_busy") return "Deferred because the server was busy.";
    if (a === "skipped_busy_max_defers") return "Skipped because the server stayed busy after multiple defers.";
    if (a === "not_connected") {
      const status = str(details.status, "").toLowerCase();
      const err = str(details.error, "").trim();
      if (status === "not_configured") {
        return "Moltbook API key is not configured. Enter it in the Moltbook settings tab.";
      }
      if (status === "error") {
        return err
          ? `Moltbook authentication failed: ${err}`
          : "Moltbook authentication failed (invalid API key or unclaimed agent).";
      }
      return "Could not connect to Moltbook.";
    }
    return null;
  }

  const regenerateApiKeyMutation = useMutation({
    mutationFn: () => api.rawPost("/settings/api-key/regenerate", {}),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["settings-api-key"] });
    },
    onError: (e) => setError(errMessage(e))
  });

  const tunnelStartMutation = useMutation({
    mutationFn: () => api.rawPost("/tunnel/start", {}),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["tunnel-status"] });
    },
    onError: (e) => setError(errMessage(e))
  });

  const tunnelStopMutation = useMutation({
    mutationFn: () => api.rawPost("/tunnel/stop", {}),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["tunnel-status"] });
    },
    onError: (e) => setError(errMessage(e))
  });

  const restartMutation = useMutation({
    mutationFn: () => api.rawPost("/restart", {}),
    onSuccess: () => setSuccess("Restart scheduled. Page will reload shortly."),
    onError: (e) => setError(errMessage(e))
  });

  const triggerPulseMutation = useMutation({
    mutationFn: () => api.rawPost("/arkpulse/trigger", {}),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["arkpulse-log"] });
    },
    onError: (e) => setError(errMessage(e))
  });

  const trustEvaluateMutation = useMutation({
    mutationFn: (payload: { action_kind: string; payload: unknown }) =>
      api.rawPost("/autonomy/trust/evaluate", payload)
  });
  const selectedTrustPreset =
    TRUST_APPROVAL_PRESETS.find((item) => item.id === trustPresetId) ?? TRUST_APPROVAL_PRESETS[0];

  const setPasswordMutation = useMutation({
    mutationFn: (password: string) => api.rawPost("/security/set-password", { password }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["security-status"] });
      setSuccess("Master password set. Server will restart.");
      setSecCurrentPassword("");
      setSecNewPassword("");
      setSecConfirmPassword("");
    },
    onError: (e) => setError(errMessage(e))
  });

  const changePasswordMutation = useMutation({
    mutationFn: (payload: { current_password: string; new_password: string }) =>
      api.rawPost("/security/change-password", payload),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["security-status"] });
      setSuccess("Master password changed. Server will restart.");
      setSecCurrentPassword("");
      setSecNewPassword("");
      setSecConfirmPassword("");
    },
    onError: (e) => setError(errMessage(e))
  });

  const removePasswordMutation = useMutation({
    mutationFn: (password: string) => api.rawPost("/security/remove-password", { password }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["security-status"] });
      setSuccess("Master password removed. Server will restart.");
      setSecCurrentPassword("");
      setSecNewPassword("");
      setSecConfirmPassword("");
    },
    onError: (e) => setError(errMessage(e))
  });

  const passwordMutationPending =
    setPasswordMutation.isPending || changePasswordMutation.isPending || removePasswordMutation.isPending;

  const revealVaultSecretsMutation = useMutation({
    mutationFn: (payload: { password?: string; keys?: string[] }) =>
      api.rawPost("/settings/secrets/reveal", payload),
    onSuccess: (raw) => {
      const entries = pickRecords(raw, "entries");
      if (entries.length === 0) return;
      setVaultRevealedValues((prev) => {
        const next = { ...prev };
        for (const row of entries) {
          const key = str(row.key, "").trim();
          if (!key) continue;
          next[key] = str(row.value, "");
        }
        return next;
      });
    },
    onError: (e) => setError(errMessage(e))
  });

  const upsertVaultSecretMutation = useMutation({
    mutationFn: (payload: { key: string; value: string; password?: string }) =>
      api.rawPost("/settings/secrets/upsert", payload),
    onSuccess: async (_, vars) => {
      await queryClient.invalidateQueries({ queryKey: ["settings-secrets"] });
      setSuccess("Secret saved.");
      setVaultRevealedValues((prev) => (Object.prototype.hasOwnProperty.call(prev, vars.key) ? { ...prev, [vars.key]: vars.value } : prev));
    },
    onError: (e) => setError(errMessage(e))
  });

  const deleteVaultSecretMutation = useMutation({
    mutationFn: (payload: { key: string; password?: string }) =>
      api.rawPost("/settings/secrets/delete", payload),
    onSuccess: async (_, vars) => {
      await queryClient.invalidateQueries({ queryKey: ["settings-secrets"] });
      setVaultRevealedValues((prev) => {
        const next = { ...prev };
        delete next[vars.key];
        return next;
      });
      setSuccess("Secret deleted.");
    },
    onError: (e) => setError(errMessage(e))
  });

  function resolveVaultPasswordForSensitiveOps(): string | null | undefined {
    if (!hasCustomMasterPassword) return undefined;
    const pw = vaultPassword.trim();
    if (!pw) {
      setError("Master password is required for secret reveal/edit operations.");
      return null;
    }
    return pw;
  }

  function openVaultEditor(mode: VaultEditorMode, key?: string) {
    setError(null);
    setSuccess(null);
    setVaultEditorMode(mode);
    setVaultEditorKey(key || "");
    setVaultEditorValue(mode === "edit" && key ? (vaultRevealedValues[key] || "") : "");
    setShowVaultSecretValue(false);
    setVaultEditorOpen(true);
  }

  function closeVaultEditor() {
    if (upsertVaultSecretMutation.isPending) return;
    setVaultEditorOpen(false);
    setVaultEditorMode("add");
    setVaultEditorKey("");
    setVaultEditorValue("");
    setShowVaultSecretValue(false);
  }

  async function submitVaultEditor() {
    const key = vaultEditorKey.trim();
    const value = vaultEditorValue;
    if (!key) {
      setError("Secret key is required.");
      return;
    }
    if (!value.trim()) {
      setError("Secret value is required.");
      return;
    }
    const pw = resolveVaultPasswordForSensitiveOps();
    if (pw === null) return;
    setError(null);
    try {
      await upsertVaultSecretMutation.mutateAsync({
        key,
        value,
        password: pw || undefined
      });
      closeVaultEditor();
    } catch {
      // handled by mutation onError
    }
  }

  function resetPasswordInputs() {
    setSecCurrentPassword("");
    setSecNewPassword("");
    setSecConfirmPassword("");
    setShowPasswordInputs(false);
  }

  function openPasswordDialog(mode: PasswordDialogMode) {
    setError(null);
    setSuccess(null);
    resetPasswordInputs();
    setPasswordDialogMode(mode);
  }

  function closePasswordDialog() {
    if (passwordMutationPending) return;
    setPasswordDialogMode(null);
    resetPasswordInputs();
  }

  async function submitPasswordDialog() {
    if (!passwordDialogMode) return;
    setError(null);
    setSuccess(null);
    try {
      if (passwordDialogMode === "set") {
        const pw = secNewPassword;
        if (pw.length < 8) {
          setError("Password must be at least 8 characters.");
          return;
        }
        if (pw !== secConfirmPassword) {
          setError("Passwords do not match.");
          return;
        }
        await setPasswordMutation.mutateAsync(pw);
      } else if (passwordDialogMode === "change") {
        const pw = secNewPassword;
        if (pw.length < 8) {
          setError("New password must be at least 8 characters.");
          return;
        }
        if (pw !== secConfirmPassword) {
          setError("Passwords do not match.");
          return;
        }
        await changePasswordMutation.mutateAsync({
          current_password: secCurrentPassword,
          new_password: pw
        });
      } else if (passwordDialogMode === "remove") {
        await removePasswordMutation.mutateAsync(secCurrentPassword);
      }
      setPasswordDialogMode(null);
    } catch (e) {
      setError(errMessage(e));
    }
  }

  return (
    <Stack spacing={2}>
      <Stack direction="row" justifyContent="space-between" alignItems="center">
        <Stack spacing={0.5}>
          <Typography variant="h6">Settings</Typography>
          <Typography variant="caption" color="text.secondary">
            {dirty ? "Unsaved changes" : "Up to date"}
          </Typography>
        </Stack>
        <Stack direction="row" spacing={1}>
          <Button
            size="small"
            variant="contained"
            onClick={async () => {
              setError(null);
              setSuccess(null);
              try {
                await saveMutation.mutateAsync();
              } catch (e) {
                setError(errMessage(e));
              }
            }}
            disabled={saveMutation.isPending || !dirty}
          >
            Save
          </Button>
        </Stack>
      </Stack>

      {!settingsComplete ? (
        <Alert severity="warning">
          Setup required: configure at least one model in the Models tab, then Save Settings.
        </Alert>
      ) : null}

      <Tabs value={tab} onChange={(_, v) => setTab(Number(v) || 0)} variant="scrollable" scrollButtons="auto">
        <Tab value={0} label="Quick Setup" />
        <Tab value={1} label="Models" />
        <Tab value={2} label="Integrations" />
        <Tab value={3} label="Media" />
        <Tab value={4} label="Security" />
        <Tab value={6} label="Analytics" />
        <Tab value={7} label="Moltbook" />
        <Tab value={8} label="MCP Servers" />
        <Tab value={12} label="Memory" />
        <Tab value={11} label="Trace" />
        <Tab value={9} label="ArkPulse" />
        <Tab value={5} label="Advanced" />
      </Tabs>

      {tab === 0 ? (
        <Grid2 container spacing={2} alignItems="stretch">
          <Grid2 size={{ xs: 12, md: 6 }} sx={{ display: "flex" }}>
            <Box className="list-shell" sx={{ minHeight: 0, width: "100%" }}>
              <Typography variant="h6" mb={1}>
                Core
              </Typography>
              <Stack spacing={1.5}>
                <TextField label="Bot Name" value={form.bot_name} onChange={(e) => setField("bot_name", e.target.value)} fullWidth />
                <TextField
                  label="Personality"
                  select
                  value={form.personality}
                  onChange={(e) => setField("personality", e.target.value)}
                  fullWidth
                >
                  <MenuItem value="friendly">friendly</MenuItem>
                  <MenuItem value="professional">professional</MenuItem>
                  <MenuItem value="casual">casual</MenuItem>
                  <MenuItem value="technical">technical</MenuItem>
                  <MenuItem value="creative">creative</MenuItem>
                  <MenuItem value="concise">concise</MenuItem>
                </TextField>

                <Autocomplete
                  freeSolo
                  options={[
                    "UTC",
                    "America/New_York",
                    "America/Chicago",
                    "America/Denver",
                    "America/Los_Angeles",
                    "America/Phoenix",
                    "America/Toronto",
                    "America/Vancouver",
                    "Europe/London",
                    "Europe/Paris",
                    "Europe/Berlin",
                    "Asia/Dubai",
                    "Asia/Kolkata",
                    "Asia/Singapore",
                    "Asia/Tokyo",
                    "Australia/Sydney"
                  ]}
                  value={form.timezone || ""}
                  onChange={(_, v) => setField("timezone", String(v ?? ""))}
                  inputValue={form.timezone || ""}
                  onInputChange={(_, v) => setField("timezone", v)}
                  renderInput={(params) => (
                    <TextField
                      {...params}
                      label="Timezone (IANA)"
                      placeholder="e.g. America/New_York"
                      fullWidth
                    />
                  )}
                />

                <TextField label="Language" value={form.language} onChange={(e) => setField("language", e.target.value)} fullWidth placeholder="e.g. English" />
                <TextField
                  label="Tone"
                  select
                  value={form.tone}
                  onChange={(e) => setField("tone", e.target.value)}
                  fullWidth
                  InputLabelProps={{ shrink: true }}
                  SelectProps={{ displayEmpty: true }}
                >
                  <MenuItem value="">Default</MenuItem>
                  <MenuItem value="concise">Concise</MenuItem>
                  <MenuItem value="friendly">Friendly</MenuItem>
                  <MenuItem value="professional">Professional</MenuItem>
                  <MenuItem value="casual">Casual</MenuItem>
                  <MenuItem value="technical">Technical</MenuItem>
                  <MenuItem value="creative">Creative</MenuItem>
                </TextField>
                <TextField
                  label="Email Format"
                  select
                  value={form.email_format}
                  onChange={(e) => setField("email_format", e.target.value)}
                  fullWidth
                  InputLabelProps={{ shrink: true }}
                  SelectProps={{ displayEmpty: true }}
                >
                  <MenuItem value="">Default</MenuItem>
                  <MenuItem value="bullets">Bullets</MenuItem>
                  <MenuItem value="sections">Sections</MenuItem>
                  <MenuItem value="narrative">Narrative</MenuItem>
                </TextField>
                <TextField
                  label="Daily Brief Channel"
                  select
                  value={form.daily_brief_channel}
                  onChange={(e) => setField("daily_brief_channel", e.target.value)}
                  fullWidth
                  InputLabelProps={{ shrink: true }}
                >
                  <MenuItem value="telegram">Telegram</MenuItem>
                  <MenuItem value="whatsapp">WhatsApp</MenuItem>
                  <MenuItem value="email">Email</MenuItem>
                </TextField>
              </Stack>
            </Box>
          </Grid2>
          <Grid2 size={{ xs: 12, md: 6 }} sx={{ display: "flex" }}>
            <Box className="list-shell" sx={{ minHeight: 0, width: "100%" }}>
              <Typography variant="h6" mb={1}>
                Snapshot
              </Typography>
              <Stack spacing={0.5}>
                <Typography variant="body2">Primary API key configured: {hasPrimaryApiKey ? "yes" : "no"}</Typography>
                <Typography variant="body2">Fallback API key configured: {hasFallbackApiKey ? "yes" : "no"}</Typography>
                <Typography variant="body2">Telegram token configured: {hasTelegramToken ? "yes" : "no"}</Typography>
                <Typography variant="body2">WhatsApp token configured: {hasWhatsAppToken ? "yes" : "no"}</Typography>
                <Typography variant="body2">Settings complete: {settingsComplete ? "yes" : "no"}</Typography>
                <Typography variant="body2">Model slots: {modelSlots.length}</Typography>
                <Typography variant="body2">Media providers: {configuredProviders.length ? configuredProviders.join(", ") : "-"}</Typography>
              </Stack>
            </Box>
          </Grid2>
        </Grid2>
      ) : null}

      {tab === 1 ? (
        <Stack spacing={2} data-tour-target="settings-models">
          <Box className="list-shell" sx={{ minHeight: 0 }}>
            <Stack direction="row" justifyContent="space-between" alignItems="center" mb={1}>
              <Stack spacing={0.3}>
                <Typography variant="h6">Model Pool</Typography>
                <Typography variant="caption" color="text.secondary">
                  Configure multiple models for different roles. Changes apply immediately.
                </Typography>
              </Stack>
              <Button size="small" variant="contained" onClick={openAddModel}>
                Add Model
              </Button>
            </Stack>

            <Stack direction="row" spacing={2} alignItems="center" sx={{ mb: 1 }}>
              <FormControlLabel
                control={
                  <Switch
                    checked={form.smart_routing}
                    onChange={(e) => setField("smart_routing", e.target.checked)}
                  />
                }
                label="Smart Routing"
              />
              <Typography variant="caption" color="text.secondary">
                When off, the agent uses the primary model for everything.
              </Typography>
            </Stack>

            {modelsQ.isLoading ? (
              <Typography variant="body2" color="text.secondary">
                Loading models...
              </Typography>
            ) : modelSlots.length === 0 ? (
              <Typography variant="body2" color="text.secondary">
                No models configured. Add a model to complete setup.
              </Typography>
            ) : (
              <TableContainer className="table-shell">
                <Table size="small">
                  <TableHead>
                    <TableRow>
                      <TableCell>Label</TableCell>
                      <TableCell>Role</TableCell>
                      <TableCell>Provider</TableCell>
                      <TableCell>Model</TableCell>
                      <TableCell>Enabled</TableCell>
                      <TableCell>API Key</TableCell>
                      <TableCell align="right">Ops</TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {modelSlots.map((slot) => {
                      const id = str(slot.id, "");
                      const enabled = toBool(slot.enabled);
                      return (
                        <TableRow key={id}>
                          <TableCell>{str(slot.label, "-")}</TableCell>
                          <TableCell>{str(slot.role, "-")}</TableCell>
                          <TableCell>{str(slot.provider, "-")}</TableCell>
                          <TableCell sx={{ wordBreak: "break-word" }}>{str(slot.model, "-")}</TableCell>
                          <TableCell>{enabled ? "yes" : "no"}</TableCell>
                          <TableCell>{toBool(slot.has_api_key) ? "configured" : "-"}</TableCell>
                          <TableCell align="right">
                            <RowOpsMenu
                              actions={[
                                {
                                  label: "Edit",
                                  onClick: () => openEditModel(slot)
                                },
                                {
                                  label: enabled ? "Disable" : "Enable",
                                  disabled: toggleModelEnabledMutation.isPending,
                                  onClick: async () => {
                                    setError(null);
                                    try {
                                      await toggleModelEnabledMutation.mutateAsync(slot);
                                    } catch (e) {
                                      setError(errMessage(e));
                                    }
                                  }
                                },
                                {
                                  label: "Delete",
                                  tone: "error",
                                  divider: true,
                                  disabled: deleteModelMutation.isPending,
                                  onClick: async () => {
                                    const ok = window.confirm("Delete this model slot?");
                                    if (!ok) return;
                                    setError(null);
                                    try {
                                      await deleteModelMutation.mutateAsync(id);
                                    } catch (e) {
                                      setError(errMessage(e));
                                    }
                                  }
                                }
                              ]}
                              ariaLabel="Model options"
                            />
                          </TableCell>
                        </TableRow>
                      );
                    })}
                  </TableBody>
                </Table>
              </TableContainer>
            )}
          </Box>

          <Dialog open={modelDialogOpen} onClose={() => setModelDialogOpen(false)} fullWidth maxWidth="sm">
            <DialogTitle>{modelEditingId ? "Edit Model" : "Add Model"}</DialogTitle>
            <DialogContent>
              <Stack spacing={1.5} sx={{ mt: 1 }}>
                <TextField
                  label="Label"
                  value={modelForm.label}
                  onChange={(e) => setModelForm((p) => ({ ...p, label: e.target.value }))}
                  fullWidth
                />
                <TextField
                  label="Role"
                  select
                  value={modelForm.role}
                  onChange={(e) => setModelForm((p) => ({ ...p, role: e.target.value }))}
                  fullWidth
                >
                  <MenuItem value="primary">primary</MenuItem>
                  <MenuItem value="fast">fast</MenuItem>
                  <MenuItem value="code">code</MenuItem>
                  <MenuItem value="research">research</MenuItem>
                  <MenuItem value="fallback">fallback</MenuItem>
                </TextField>
                <TextField
                  label="Provider"
                  select
                  value={modelForm.provider}
                  onChange={(e) => setModelForm((p) => ({ ...p, provider: e.target.value }))}
                  fullWidth
                >
                  <MenuItem value="ollama">ollama</MenuItem>
                  <MenuItem value="anthropic">anthropic</MenuItem>
                  <MenuItem value="openai">openai</MenuItem>
                  <MenuItem value="openrouter">openrouter</MenuItem>
                  <MenuItem value="openai-compatible">openai-compatible</MenuItem>
                </TextField>
                <TextField
                  label="Model"
                  value={modelForm.model}
                  onChange={(e) => setModelForm((p) => ({ ...p, model: e.target.value }))}
                  fullWidth
                />
                <TextField
                  label="API Key (optional)"
                  value={modelForm.api_key}
                  onChange={(e) => setModelForm((p) => ({ ...p, api_key: e.target.value }))}
                  fullWidth
                  type="password"
                />
                <Accordion expanded={modelAdvancedOpen} onChange={(_, expanded) => setModelAdvancedOpen(expanded)} disableGutters>
                  <AccordionSummary expandIcon={<ExpandMoreIcon />}>
                    <Typography variant="body2">Advanced</Typography>
                  </AccordionSummary>
                  <AccordionDetails>
                    {["ollama", "openrouter", "openai-compatible"].includes(modelForm.provider) ? (
                      <TextField
                        label={modelForm.provider === "openai-compatible" ? "Base URL" : "Base URL (optional)"}
                        value={modelForm.base_url}
                        onChange={(e) => setModelForm((p) => ({ ...p, base_url: e.target.value }))}
                        fullWidth
                        helperText={
                          modelForm.provider === "openrouter"
                            ? `Default: ${OPENROUTER_DEFAULT_BASE_URL}`
                            : modelForm.provider === "ollama"
                              ? `Default: ${OLLAMA_DEFAULT_BASE_URL}`
                              : "Required for OpenAI-compatible providers."
                        }
                      />
                    ) : (
                      <Typography variant="caption" color="text.secondary">
                        No advanced provider settings for this model.
                      </Typography>
                    )}
                  </AccordionDetails>
                </Accordion>
                <FormControlLabel
                  control={<Switch checked={modelForm.enabled} onChange={(e) => setModelForm((p) => ({ ...p, enabled: e.target.checked }))} />}
                  label="Enabled"
                />
                <Stack direction="row" spacing={1} justifyContent="flex-end">
                  <Button onClick={() => setModelDialogOpen(false)}>Cancel</Button>
                  <Button
                    variant="contained"
                    onClick={async () => {
                      setError(null);
                      setModelConnectivityWarning(null);
                      try {
                        await saveModelMutation.mutateAsync();
                      } catch (e) {
                        setError(errMessage(e));
                      }
                    }}
                    disabled={saveModelMutation.isPending}
                  >
                    Save
                  </Button>
                </Stack>
              </Stack>
            </DialogContent>
          </Dialog>
        </Stack>
      ) : null}

      {tab === 3 ? (
        <Grid2 container spacing={2} alignItems="stretch">
          <Grid2 size={{ xs: 12, lg: 6 }} sx={{ display: "flex" }}>
            <Box className="list-shell" sx={{ minHeight: 0, width: "100%" }}>
              <Typography variant="h6" mb={1}>
                Provider Keys
              </Typography>
              <Typography variant="caption" color="text.secondary">
                Keys are stored encrypted. Leave blank to keep current keys.
              </Typography>
              <Stack spacing={1.2} sx={{ mt: 1 }}>
                <TextField label="Replicate API Key" value={form.media_key_replicate} onChange={(e) => setField("media_key_replicate", e.target.value)} fullWidth size="small" type="password" />
                <TextField label="FAL API Key" value={form.media_key_fal} onChange={(e) => setField("media_key_fal", e.target.value)} fullWidth size="small" type="password" />
                <TextField label="Stability AI API Key" value={form.media_key_stability_ai} onChange={(e) => setField("media_key_stability_ai", e.target.value)} fullWidth size="small" type="password" />
                <TextField label="Together API Key" value={form.media_key_together} onChange={(e) => setField("media_key_together", e.target.value)} fullWidth size="small" type="password" />
                <TextField label="OpenAI API Key (DALL-E/Sora)" value={form.media_key_openai_dalle} onChange={(e) => setField("media_key_openai_dalle", e.target.value)} fullWidth size="small" type="password" />
                <TextField label="Google AI API Key (Gemini/Veo)" value={form.media_key_google_gemini} onChange={(e) => setField("media_key_google_gemini", e.target.value)} fullWidth size="small" type="password" />
                <TextField label="Runway API Key" value={form.media_key_runway} onChange={(e) => setField("media_key_runway", e.target.value)} fullWidth size="small" type="password" />
                <TextField label="Luma API Key" value={form.media_key_luma} onChange={(e) => setField("media_key_luma", e.target.value)} fullWidth size="small" type="password" />
              </Stack>
              <Divider sx={{ my: 2 }} />
              <Typography variant="caption" color="text.secondary">
                Detected configured providers: {configuredProviders.length ? configuredProviders.join(", ") : "(none detected)"}
              </Typography>
            </Box>
          </Grid2>

          <Grid2 size={{ xs: 12, lg: 6 }} sx={{ display: "flex" }}>
            <Box className="list-shell" sx={{ minHeight: 0, width: "100%" }}>
              <Typography variant="h6" mb={1}>
                Defaults
              </Typography>
              <Stack spacing={1.2}>
                <TextField label="Default Image Provider" value={form.default_image_provider} onChange={(e) => setField("default_image_provider", e.target.value)} fullWidth size="small" />
                <TextField label="Image Model" value={form.image_model} onChange={(e) => setField("image_model", e.target.value)} fullWidth size="small" />
                <TextField label="Fallback Image Provider" value={form.fallback_image_provider} onChange={(e) => setField("fallback_image_provider", e.target.value)} fullWidth size="small" />
                <TextField label="Default Video Provider" value={form.default_video_provider} onChange={(e) => setField("default_video_provider", e.target.value)} fullWidth size="small" />
                <TextField label="Fallback Video Provider" value={form.fallback_video_provider} onChange={(e) => setField("fallback_video_provider", e.target.value)} fullWidth size="small" />
              </Stack>
              <Divider sx={{ my: 2 }} />
              <Typography variant="h6" mb={1}>
                Advanced (JSON)
              </Typography>
              <Typography variant="caption" color="text.secondary">
                Optional JSON mapping provider to key, e.g. {"{\"openai\":\"sk-...\",\"replicate\":\"...\"}"}
              </Typography>
              <TextField
                label="media_providers JSON"
                value={form.media_provider_keys_json}
                onChange={(e) => setField("media_provider_keys_json", e.target.value)}
                fullWidth
                multiline
                minRows={6}
                sx={{ mt: 1 }}
              />
            </Box>
          </Grid2>
        </Grid2>
      ) : null}

      {tab === 4 ? (
        <Grid2 container spacing={2}>
          <Grid2 size={{ xs: 12, lg: 6 }}>
            <Stack spacing={2}>
              <Box className="list-shell" sx={{ minHeight: 0 }}>
                <Stack spacing={1}>
                  <Typography variant="h6">Security & Master Password</Typography>
                  {securityStatusQ.isLoading ? (
                    <Typography variant="body2" color="text.secondary">
                      Loading security status...
                    </Typography>
                  ) : securityStatusQ.error ? (
                    <Alert severity="error">{errMessage(securityStatusQ.error)}</Alert>
                  ) : (
                    <Stack spacing={1}>
                      <Typography variant="caption" color="text.secondary">
                        Mode: {str(sec.encryption_mode) === "password" ? "password" : "keyfile"}
                      </Typography>
                      {str(sec.encryption_mode) !== "password" ? (
                        <Alert
                          severity="warning"
                          sx={{ py: 0.25, "& .MuiAlert-message": { fontSize: "0.75rem", lineHeight: 1.35 } }}
                        >
                          No master password is active yet.
                        </Alert>
                      ) : toBool(sec.using_default) ? (
                        <Alert
                          severity="warning"
                          sx={{ py: 0.25, "& .MuiAlert-message": { fontSize: "0.75rem", lineHeight: 1.35 } }}
                        >
                          Default password is active. Treat this as not configured until you set your own custom master password.
                        </Alert>
                      ) : (
                        <Alert
                          severity="success"
                          sx={{ py: 0.25, "& .MuiAlert-message": { fontSize: "0.75rem", lineHeight: 1.35 } }}
                        >
                          Custom master password active.
                        </Alert>
                      )}
                      <Typography variant="caption" color="text.secondary">
                        Password setup opens a secure dialog. Changes apply immediately and restart the server.
                      </Typography>
                      <Stack direction={{ xs: "column", sm: "row" }} spacing={1}>
                        {hasCustomMasterPassword ? (
                          <Button
                            variant="contained"
                            size="large"
                            onClick={() => openPasswordDialog("change")}
                            disabled={passwordMutationPending}
                          >
                            Change Password
                          </Button>
                        ) : (
                          <Button
                            variant="contained"
                            size="large"
                            onClick={() => openPasswordDialog(toBool(sec.master_password_set) ? "change" : "set")}
                            disabled={passwordMutationPending}
                          >
                            Set Custom Password
                          </Button>
                        )}
                        {hasCustomMasterPassword ? (
                          <Button
                            color="error"
                            variant="outlined"
                            size="large"
                            onClick={() => openPasswordDialog("remove")}
                            disabled={passwordMutationPending}
                          >
                            Remove Password
                          </Button>
                        ) : null}
                      </Stack>
                    </Stack>
                  )}
                </Stack>
              </Box>

              <Box className="list-shell" sx={{ minHeight: 0 }}>
                <Typography variant="h6" mb={1}>
                  Remote Access (Tunnel)
                </Typography>
                {tunnelQ.isLoading ? (
                  <Typography variant="body2" color="text.secondary">
                    Loading tunnel status...
                  </Typography>
                ) : tunnelQ.error ? (
                  <Alert severity="error">{errMessage(tunnelQ.error)}</Alert>
                ) : (
                  <Stack spacing={1}>
                    <Typography variant="caption" color="text.secondary">
                      Active: {boolText(tunnel.active)} | Available: {boolText(tunnel.available)}
                    </Typography>
                    {str(tunnel.url, "").trim() ? (
                      <TextField
                        label="Public URL"
                        value={str(tunnel.url)}
                        fullWidth
                        size="small"
                        InputProps={{ readOnly: true }}
                      />
                    ) : null}
                    {str(tunnel.error, "").trim() ? <Alert severity="error">{str(tunnel.error)}</Alert> : null}
                    <Stack direction="row" spacing={1}>
                      <Button
                        size="small"
                        variant="contained"
                        onClick={async () => {
                          setError(null);
                          try {
                            await tunnelStartMutation.mutateAsync();
                          } catch (e) {
                            setError(errMessage(e));
                          }
                        }}
                        disabled={tunnelStartMutation.isPending || toBool(tunnel.active) || !toBool(tunnel.available)}
                      >
                        Start Tunnel
                      </Button>
                      <Button
                        size="small"
                        onClick={async () => {
                          setError(null);
                          try {
                            await tunnelStopMutation.mutateAsync();
                          } catch (e) {
                            setError(errMessage(e));
                          }
                        }}
                        disabled={tunnelStopMutation.isPending || !toBool(tunnel.active)}
                      >
                        Stop Tunnel
                      </Button>
                      <Button
                        size="small"
                        onClick={async () => {
                          const url = str(tunnel.url, "");
                          if (!url) return;
                          await navigator.clipboard.writeText(url);
                          setSuccess("Tunnel URL copied.");
                        }}
                        disabled={!str(tunnel.url, "").trim()}
                      >
                        Copy URL
                      </Button>
                    </Stack>
                    <Alert
                      severity="warning"
                      sx={{ py: 0.25, "& .MuiAlert-message": { fontSize: "0.75rem", lineHeight: 1.35 } }}
                    >
                      Anyone with the URL can control your agent. Use an API key and stop the tunnel when not needed.
                    </Alert>
                  </Stack>
                )}
              </Box>

              <Box className="list-shell" sx={{ minHeight: 0 }}>
                <Stack spacing={1}>
                  <Typography variant="h6">Secrets Vault</Typography>
                  <Typography variant="caption" color="text.secondary">
                    Manage encrypted custom secrets used by skills, integrations, and agent workflows.
                  </Typography>
                  {hasCustomMasterPassword ? (
                    <TextField
                      label="Master password (required for reveal/edit/delete)"
                      value={vaultPassword}
                      onChange={(e) => setVaultPassword(e.target.value)}
                      fullWidth
                      size="small"
                      type="password"
                    />
                  ) : (
                    <Alert severity="info">
                      No custom master password is set. Secrets are still encrypted at rest, and reveal is available in this session.
                    </Alert>
                  )}
                  <Stack direction={{ xs: "column", sm: "row" }} spacing={1}>
                    <Button
                      size="small"
                      variant="contained"
                      onClick={async () => {
                        const pw = resolveVaultPasswordForSensitiveOps();
                        if (pw === null) return;
                        setError(null);
                        try {
                          await revealVaultSecretsMutation.mutateAsync({ password: pw || undefined });
                          setSuccess("Secrets revealed.");
                        } catch {
                          // handled by mutation onError
                        }
                      }}
                      disabled={revealVaultSecretsMutation.isPending || vaultSecrets.length === 0}
                    >
                      Reveal all
                    </Button>
                    <Button
                      size="small"
                      onClick={() => setVaultRevealedValues({})}
                      disabled={Object.keys(vaultRevealedValues).length === 0}
                    >
                      Hide all
                    </Button>
                    <Button
                      size="small"
                      onClick={async () => {
                        setError(null);
                        await queryClient.invalidateQueries({ queryKey: ["settings-secrets"] });
                      }}
                      disabled={vaultSecretsQ.isLoading}
                    >
                      Refresh
                    </Button>
                    <Button
                      size="small"
                      variant="outlined"
                      onClick={() => openVaultEditor("add")}
                    >
                      Add Secret
                    </Button>
                  </Stack>

                  {vaultSecretsQ.isLoading ? (
                    <Typography variant="body2" color="text.secondary">
                      Loading secrets...
                    </Typography>
                  ) : vaultSecretsQ.error ? (
                    <Alert severity="error">{errMessage(vaultSecretsQ.error)}</Alert>
                  ) : vaultSecrets.length === 0 ? (
                    <Typography variant="body2" color="text.secondary">
                      No custom secrets yet.
                    </Typography>
                  ) : (
                    <TableContainer className="table-shell">
                      <Table size="small">
                        <TableHead>
                          <TableRow>
                            <TableCell>Key</TableCell>
                            <TableCell>Value</TableCell>
                            <TableCell align="right">Ops</TableCell>
                          </TableRow>
                        </TableHead>
                        <TableBody>
                          {vaultSecrets.map((row, idx) => {
                            const key = str(row.key, "");
                            const revealed = Object.prototype.hasOwnProperty.call(vaultRevealedValues, key);
                            const shownValue = revealed ? vaultRevealedValues[key] : str(row.masked, "");
                            return (
                              <TableRow key={`${key}-${idx}`}>
                                <TableCell sx={{ fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace" }}>{key}</TableCell>
                                <TableCell sx={{ maxWidth: 360 }}>
                                  <Typography
                                    variant="body2"
                                    title={shownValue}
                                    sx={{
                                      whiteSpace: "nowrap",
                                      overflow: "hidden",
                                      textOverflow: "ellipsis",
                                      fontFamily: revealed ? "ui-monospace, SFMono-Regular, Menlo, monospace" : "inherit"
                                    }}
                                  >
                                    {shownValue || "-"}
                                  </Typography>
                                </TableCell>
                                <TableCell align="right">
                                  <Stack direction="row" spacing={0.5} justifyContent="flex-end">
                                    <Button
                                      size="small"
                                      onClick={async () => {
                                        if (revealed) {
                                          setVaultRevealedValues((prev) => {
                                            const next = { ...prev };
                                            delete next[key];
                                            return next;
                                          });
                                          return;
                                        }
                                        const pw = resolveVaultPasswordForSensitiveOps();
                                        if (pw === null) return;
                                        setError(null);
                                        try {
                                          await revealVaultSecretsMutation.mutateAsync({
                                            password: pw || undefined,
                                            keys: [key]
                                          });
                                        } catch {
                                          // handled by mutation onError
                                        }
                                      }}
                                      disabled={revealVaultSecretsMutation.isPending}
                                    >
                                      {revealed ? "Hide" : "Reveal"}
                                    </Button>
                                    <Button
                                      size="small"
                                      onClick={() => openVaultEditor("edit", key)}
                                    >
                                      Edit
                                    </Button>
                                    <Button
                                      size="small"
                                      color="error"
                                      onClick={async () => {
                                        const ok = window.confirm(`Delete secret '${key}'?`);
                                        if (!ok) return;
                                        const pw = resolveVaultPasswordForSensitiveOps();
                                        if (pw === null) return;
                                        setError(null);
                                        try {
                                          await deleteVaultSecretMutation.mutateAsync({
                                            key,
                                            password: pw || undefined
                                          });
                                        } catch {
                                          // handled by mutation onError
                                        }
                                      }}
                                      disabled={deleteVaultSecretMutation.isPending}
                                    >
                                      Delete
                                    </Button>
                                  </Stack>
                                </TableCell>
                              </TableRow>
                            );
                          })}
                        </TableBody>
                      </Table>
                    </TableContainer>
                  )}

                </Stack>
              </Box>
            </Stack>
          </Grid2>

          <Grid2 size={{ xs: 12 }}>
            <QueryTable title="Security Logs" path="/security/logs?limit=20" arrayKey="logs" columns={["event_type", "severity", "message", "source", "created_at", "count"]} autoRefresh={autoRefresh} emptyLabel="No security logs yet." queryKey="settings-security-logs-table" />
          </Grid2>
        </Grid2>
      ) : null}

      {tab === 5 ? (
        <Grid2 container spacing={2}>
          <Grid2 size={{ xs: 12 }}>
            <Alert severity="warning">
              Advanced settings are powerful and can impact stability or security. Change only if you understand the effect.
            </Alert>
          </Grid2>
          <Grid2 size={{ xs: 12 }}>
            <Box className="list-shell" sx={{ minHeight: 0 }}>
              <Stack direction="row" justifyContent="space-between" alignItems="center" spacing={2}>
                <Stack spacing={0.35}>
                  <Typography variant="h6">Restart Bot</Typography>
                  <Typography variant="caption" color="text.secondary">
                    Restarts AgentArk to apply runtime and security changes.
                  </Typography>
                </Stack>
                <Button
                  size="small"
                  color="warning"
                  variant="outlined"
                  onClick={async () => {
                    const ok = window.confirm("Restart AgentArk?");
                    if (!ok) return;
                    setError(null);
                    setSuccess(null);
                    try {
                      await restartMutation.mutateAsync();
                      setTimeout(() => window.location.reload(), 2000);
                    } catch (e) {
                      setError(errMessage(e));
                    }
                  }}
                  disabled={restartMutation.isPending}
                >
                  Restart Bot
                </Button>
              </Stack>
            </Box>
          </Grid2>
          <Grid2 size={{ xs: 12 }}>
            <Box className="list-shell" sx={{ minHeight: 0 }}>
              <Stack direction="row" justifyContent="space-between" alignItems="center" spacing={2}>
                <Stack spacing={0.35}>
                  <Typography variant="h6">Developer Mode</Typography>
                  <Typography variant="caption" color="text.secondary">
                    Enables raw SKILL.md editing in Skills. Keep this off for beginner-friendly forms.
                  </Typography>
                </Stack>
                <FormControlLabel
                  control={
                    <Switch
                      checked={developerModeEnabled}
                      onChange={(e) => {
                        const next = e.target.checked;
                        setDeveloperModeEnabled(next);
                        setDeveloperModeEnabledState(next);
                        setError(null);
                        setSuccess(next ? "Developer mode enabled." : "Developer mode disabled.");
                      }}
                    />
                  }
                  label={developerModeEnabled ? "Enabled" : "Disabled"}
                />
              </Stack>
            </Box>
          </Grid2>
          <Grid2 size={{ xs: 12 }}>
            <Box className="list-shell" sx={{ minHeight: 0 }}>
              <Stack direction="row" justifyContent="space-between" alignItems="center" spacing={2}>
                <Stack spacing={0.35}>
                  <Typography variant="h6">Guided Tour</Typography>
                  <Typography variant="caption" color="text.secondary">
                    Re-run the onboarding walkthrough to review core features.
                  </Typography>
                </Stack>
                <Button
                  size="small"
                  variant="outlined"
                  onClick={() => {
                    try { window.localStorage.setItem("agentark.tour.completed", "0"); } catch {}
                    const { startTour } = useUiStore.getState();
                    startTour();
                  }}
                >
                  Restart Tour
                </Button>
              </Stack>
            </Box>
          </Grid2>

          <Grid2 size={{ xs: 12 }}>
            <Box className="list-shell" sx={{ minHeight: 0 }}>
              <Typography variant="h6" mb={1}>Will This Need Approval?</Typography>
              <Typography variant="caption" color="text.secondary">
                Pick what you want to do and check the likely approval requirement. This is only a preview and does not run anything.
              </Typography>
              <Grid2 container spacing={1} sx={{ mt: 0.75 }}>
                <Grid2 size={{ xs: 12, md: 5 }}>
                  <TextField
                    fullWidth
                    size="small"
                    select
                    label="What do you want to do?"
                    value={trustPresetId}
                    onChange={(e) => {
                      const nextId = e.target.value;
                      setTrustPresetId(nextId);
                      const nextPreset =
                        TRUST_APPROVAL_PRESETS.find((item) => item.id === nextId) ?? TRUST_APPROVAL_PRESETS[0];
                      if (nextPreset) {
                        setTrustActionKind(nextPreset.actionKind);
                      }
                    }}
                  >
                    {TRUST_APPROVAL_PRESETS.map((preset) => (
                      <MenuItem key={preset.id} value={preset.id}>
                        {preset.label}
                      </MenuItem>
                    ))}
                  </TextField>
                </Grid2>
                <Grid2 size={{ xs: 12, md: 7 }}>
                  <TextField
                    fullWidth
                    size="small"
                    label={selectedTrustPreset ? selectedTrustPreset.detailLabel : "Details"}
                    value={trustPresetDetail}
                    onChange={(e) => setTrustPresetDetail(e.target.value)}
                    placeholder={selectedTrustPreset ? selectedTrustPreset.detailPlaceholder : "Add a short detail"}
                  />
                </Grid2>
                <Grid2 size={{ xs: 12 }}>
                  <FormControlLabel
                    control={
                      <Switch
                        checked={trustUseAdvancedInput}
                        onChange={(e) => setTrustUseAdvancedInput(e.target.checked)}
                      />
                    }
                    label="Use advanced input (action name + JSON)"
                  />
                </Grid2>
                {trustUseAdvancedInput ? (
                  <>
                    <Grid2 size={{ xs: 12, md: 4 }}>
                      <TextField
                        fullWidth
                        size="small"
                        label="Technical action name"
                        value={trustActionKind}
                        onChange={(e) => setTrustActionKind(e.target.value)}
                        placeholder="shell"
                      />
                    </Grid2>
                    <Grid2 size={{ xs: 12, md: 8 }}>
                      <TextField
                        fullWidth
                        size="small"
                        multiline
                        minRows={3}
                        label="Technical payload (JSON)"
                        value={trustPayloadJson}
                        onChange={(e) => setTrustPayloadJson(e.target.value)}
                        placeholder='{"command":"ls -la"}'
                      />
                    </Grid2>
                  </>
                ) : null}
                <Grid2 size={{ xs: 12 }}>
                  <Button
                    variant="contained"
                    disabled={
                      trustEvaluateMutation.isPending ||
                      (trustUseAdvancedInput ? !trustActionKind.trim() : !trustPresetDetail.trim())
                    }
                    onClick={async () => {
                      setError(null);
                      setSuccess(null);
                      setTrustResult(null);
                      let actionKind = "";
                      let parsedPayload: unknown = {};
                      if (trustUseAdvancedInput) {
                        actionKind = trustActionKind.trim();
                        const raw = trustPayloadJson.trim();
                        if (raw) {
                          try {
                            parsedPayload = JSON.parse(raw);
                          } catch {
                            setError("Technical payload JSON is invalid.");
                            return;
                          }
                        }
                      } else {
                        const preset = selectedTrustPreset;
                        const detail = trustPresetDetail.trim();
                        if (!preset) {
                          setError("Select an action first.");
                          return;
                        }
                        if (!detail) {
                          setError("Add a short detail so risk can be estimated.");
                          return;
                        }
                        actionKind = preset.actionKind;
                        parsedPayload = preset.buildPayload(detail);
                        setTrustActionKind(actionKind);
                        setTrustPayloadJson(JSON.stringify(parsedPayload, null, 2));
                      }
                      try {
                        const out = asRecord(
                          await trustEvaluateMutation.mutateAsync({
                            action_kind: actionKind,
                            payload: parsedPayload
                          })
                        );
                        setTrustResult(asRecord(out.risk));
                      } catch (e) {
                        setError(errMessage(e));
                      }
                    }}
                  >
                    {trustEvaluateMutation.isPending ? "Checking..." : "Check Approval"}
                  </Button>
                </Grid2>
                {trustResult ? (
                  <Grid2 size={{ xs: 12 }}>
                    <Stack spacing={1}>
                      <Alert severity={toBool(trustResult.requires_approval) ? "warning" : "success"}>
                        {toBool(trustResult.requires_approval)
                          ? "This will likely require your approval before running."
                          : "This is likely safe to run without manual approval."}
                      </Alert>
                      <KeyValuePanel title="Risk details" data={trustResult} />
                    </Stack>
                  </Grid2>
                ) : null}
              </Grid2>
            </Box>
          </Grid2>

          <Grid2 size={{ xs: 12, lg: 6 }}>
            <Box className="list-shell" sx={{ minHeight: 0 }}>
              <Typography variant="h6" mb={1}>
                Auto-Approve Skills
              </Typography>
              <Typography variant="caption" color="text.secondary">
                Select skills that can run without approval. Backend validates and may reject dangerous entries.
              </Typography>
              {(() => {
                const items = [
                  "web_search",
                  "research",
                  "generate_image",
                  "generate_video",
                  "browse",
                  "file_read",
                  "file_write",
                  "http_get",
                  "shell",
                  "code_execute",
                  "schedule_task",
                  "list_tasks",
                  "clipboard_read",
                  "clipboard_write",
                  "gmail_scan",
                  "gmail_reply"
                ];
                const set = new Set(parseCsvList(form.auto_approve_csv));
                const update = (name: string, checked: boolean) => {
                  const next = new Set(set);
                  if (checked) next.add(name);
                  else next.delete(name);
                  setField("auto_approve_csv", Array.from(next).sort().join(", "));
                };
                return (
                  <Grid2 container spacing={1} sx={{ mt: 1 }}>
                    {items.map((name) => (
                      <Grid2 key={name} size={{ xs: 12, md: 6 }}>
                        <FormControlLabel
                          control={<Switch checked={set.has(name)} onChange={(e) => update(name, e.target.checked)} />}
                          label={name}
                        />
                      </Grid2>
                    ))}
                    <Grid2 size={{ xs: 12 }}>
                      <TextField
                        label="Auto-Approve (manual CSV)"
                        value={form.auto_approve_csv}
                        onChange={(e) => setField("auto_approve_csv", e.target.value)}
                        fullWidth
                        size="small"
                        placeholder="comma separated action names"
                      />
                    </Grid2>
                  </Grid2>
                );
              })()}
            </Box>
          </Grid2>

          <Grid2 size={{ xs: 12, lg: 6 }}>
            <Box className="list-shell" sx={{ minHeight: 0 }}>
              <Typography variant="h6" mb={1}>
                API Key (HTTP)
              </Typography>
              {apiKeyQ.isLoading ? (
                <Typography variant="body2" color="text.secondary">
                  Loading API key...
                </Typography>
              ) : apiKeyQ.error ? (
                <Alert severity="error">{errMessage(apiKeyQ.error)}</Alert>
              ) : (
                <Stack spacing={1}>
                  <Typography variant="caption" color="text.secondary">
                    Used as `Authorization: Bearer &lt;key&gt;` for all HTTP API requests.
                  </Typography>
                  <Typography variant="caption" color={apiKeyRemainingSeconds > 0 ? "text.secondary" : "warning.main"}>
                    Rotates in {formatDurationClock(apiKeyRemainingSeconds)}
                    {apiKeyExpiresAtUnix > 0
                      ? ` (next: ${new Date(apiKeyExpiresAtUnix * 1000).toLocaleString()})`
                      : ""}
                  </Typography>
                  {apiKeyRotated ? (
                    <Typography variant="caption" color="info.main">
                      API key rotated automatically.
                    </Typography>
                  ) : null}
                  <TextField
                    label="Key"
                    value={apiKeyRevealed ? str(apiKeyPayload.key, "") : str(apiKeyPayload.masked, "")}
                    fullWidth
                    size="small"
                    InputProps={{ readOnly: true }}
                  />
                  {apiKeyIssuedAtUnix > 0 ? (
                    <Typography variant="caption" color="text.secondary">
                      Issued: {new Date(apiKeyIssuedAtUnix * 1000).toLocaleString()}
                    </Typography>
                  ) : null}
                  <Stack direction="row" spacing={1}>
                    <Button size="small" onClick={() => setApiKeyRevealed((v) => !v)}>
                      {apiKeyRevealed ? "Hide" : "Reveal"}
                    </Button>
                    <Button
                      size="small"
                      onClick={async () => {
                        const key = str(apiKeyPayload.key, "");
                        if (!key) return;
                        await navigator.clipboard.writeText(key);
                        setSuccess("API key copied.");
                      }}
                      disabled={!str(apiKeyPayload.key, "").trim()}
                    >
                      Copy
                    </Button>
                    <Button
                      size="small"
                      color="warning"
                      onClick={async () => {
                        const ok = window.confirm("Regenerate API key? Old key will stop working.");
                        if (!ok) return;
                        setError(null);
                        setSuccess(null);
                        try {
                          await regenerateApiKeyMutation.mutateAsync();
                          setApiKeyRevealed(true);
                          setSuccess("API key regenerated.");
                        } catch (e) {
                          setError(errMessage(e));
                        }
                      }}
                      disabled={regenerateApiKeyMutation.isPending}
                    >
                      Regenerate
                    </Button>
                  </Stack>
                </Stack>
              )}
            </Box>
          </Grid2>

        </Grid2>
      ) : null}

      {tab === 6 ? (
        <Stack spacing={2}>
          <Grid2 container spacing={2}>
            <Grid2 size={{ xs: 12, md: 4 }}>
              <Box className="list-shell" sx={{ minHeight: 0 }}>
                <Typography variant="h6" mb={1}>
                  Summary (24h)
                </Typography>
                {llmAnalyticsHourQ.error ? (
                  <Alert severity="error">{errMessage(llmAnalyticsHourQ.error)}</Alert>
                ) : (
                  <Stack spacing={0.5}>
                    <Typography variant="body2">
                      Requests: {num(llmHour?.totals?.request_count, 0)}
                    </Typography>
                    <Typography variant="body2">
                      Tokens: {num(llmHour?.totals?.total_tokens, 0)}
                    </Typography>
                    <Typography variant="body2">
                      Estimated rows: {num(llmHour?.totals?.estimated_count, 0)}
                    </Typography>
                    <Typography variant="body2">
                      Cost (USD):{" "}
                      {typeof llmHour?.totals?.cost_usd === "number"
                        ? `$${llmHour.totals.cost_usd.toFixed(4)}`
                        : "n/a"}
                    </Typography>
                  </Stack>
                )}
              </Box>
            </Grid2>

            <Grid2 size={{ xs: 12, md: 4 }}>
              <Box className="list-shell" sx={{ minHeight: 0 }}>
                <Typography variant="h6" mb={1}>
                  Summary (30d)
                </Typography>
                {llmAnalyticsDayQ.error ? (
                  <Alert severity="error">{errMessage(llmAnalyticsDayQ.error)}</Alert>
                ) : (
                  <Stack spacing={0.5}>
                    <Typography variant="body2">
                      Requests: {num(llmDay?.totals?.request_count, 0)}
                    </Typography>
                    <Typography variant="body2">
                      Tokens: {num(llmDay?.totals?.total_tokens, 0)}
                    </Typography>
                    <Typography variant="body2">
                      Cost (USD):{" "}
                      {typeof llmDay?.totals?.cost_usd === "number"
                        ? `$${llmDay.totals.cost_usd.toFixed(4)}`
                        : "n/a"}
                    </Typography>
                  </Stack>
                )}
              </Box>
            </Grid2>

            <Grid2 size={{ xs: 12, md: 4 }}>
              <Box className="list-shell" sx={{ minHeight: 0 }}>
                <Typography variant="h6" mb={1}>
                  Summary (90d)
                </Typography>
                {llmAnalyticsWeekQ.error ? (
                  <Alert severity="error">{errMessage(llmAnalyticsWeekQ.error)}</Alert>
                ) : (
                  <Stack spacing={0.5}>
                    <Typography variant="body2">
                      Requests: {num(llmWeek?.totals?.request_count, 0)}
                    </Typography>
                    <Typography variant="body2">
                      Tokens: {num(llmWeek?.totals?.total_tokens, 0)}
                    </Typography>
                    <Typography variant="body2">
                      Cost (USD):{" "}
                      {typeof llmWeek?.totals?.cost_usd === "number"
                        ? `$${llmWeek.totals.cost_usd.toFixed(4)}`
                        : "n/a"}
                    </Typography>
                  </Stack>
                )}
              </Box>
            </Grid2>
          </Grid2>

          <Grid2 container spacing={2}>
            <Grid2 size={{ xs: 12, lg: 4 }}>
              <Box className="chart-shell">
                <ReactECharts option={analyticsOption.hour} style={{ height: 280 }} />
              </Box>
            </Grid2>
            <Grid2 size={{ xs: 12, lg: 4 }}>
              <Box className="chart-shell">
                <ReactECharts option={analyticsOption.day} style={{ height: 280 }} />
              </Box>
            </Grid2>
            <Grid2 size={{ xs: 12, lg: 4 }}>
              <Box className="chart-shell">
                <ReactECharts option={analyticsOption.week} style={{ height: 280 }} />
              </Box>
            </Grid2>
          </Grid2>

          <Grid2 container spacing={2} alignItems="stretch">
            <Grid2 size={{ xs: 12, lg: 6 }} sx={{ display: "flex" }}>
              <Box className="list-shell" sx={{ minHeight: 0, flex: 1 }}>
                <Typography variant="h6" mb={1}>
                  Top Models (30d)
                </Typography>
                {llmDay?.by_model?.length ? (
                  <TableContainer className="table-shell">
                    <Table size="small">
                      <TableHead>
                        <TableRow>
                          <TableCell>Provider</TableCell>
                          <TableCell>Model</TableCell>
                          <TableCell align="right">Requests</TableCell>
                          <TableCell align="right">Tokens</TableCell>
                          <TableCell align="right">Cost</TableCell>
                        </TableRow>
                      </TableHead>
                      <TableBody>
                        {llmDay.by_model.slice(0, 12).map((r, idx) => (
                          <TableRow key={`${r.provider}:${r.model}:${idx}`}>
                            <TableCell>{r.provider || "-"}</TableCell>
                            <TableCell sx={{ wordBreak: "break-word" }}>{r.model || "-"}</TableCell>
                            <TableCell align="right">{num(r.request_count, 0)}</TableCell>
                            <TableCell align="right">{num(r.total_tokens, 0)}</TableCell>
                            <TableCell align="right">
                              {typeof r.cost_usd === "number" ? `$${r.cost_usd.toFixed(4)}` : "n/a"}
                            </TableCell>
                          </TableRow>
                        ))}
                      </TableBody>
                    </Table>
                  </TableContainer>
                ) : (
                  <Typography variant="body2" color="text.secondary">
                    No usage yet.
                  </Typography>
                )}
              </Box>
            </Grid2>
            <Grid2 size={{ xs: 12, lg: 6 }} sx={{ display: "flex" }}>
              <Box className="list-shell" sx={{ minHeight: 0, flex: 1 }}>
                <Typography variant="h6" mb={1}>
                  By Channel (30d)
                </Typography>
                {llmDay?.by_channel?.length ? (
                  <TableContainer className="table-shell">
                    <Table size="small">
                      <TableHead>
                        <TableRow>
                          <TableCell>Channel</TableCell>
                          <TableCell align="right">Requests</TableCell>
                          <TableCell align="right">Tokens</TableCell>
                          <TableCell align="right">Cost</TableCell>
                        </TableRow>
                      </TableHead>
                      <TableBody>
                        {llmDay.by_channel.slice(0, 12).map((r, idx) => (
                          <TableRow key={`${r.channel || "?"}:${idx}`}>
                            <TableCell>{r.channel || "-"}</TableCell>
                            <TableCell align="right">{num(r.request_count, 0)}</TableCell>
                            <TableCell align="right">{num(r.total_tokens, 0)}</TableCell>
                            <TableCell align="right">
                              {typeof r.cost_usd === "number" ? `$${r.cost_usd.toFixed(4)}` : "n/a"}
                            </TableCell>
                          </TableRow>
                        ))}
                      </TableBody>
                    </Table>
                  </TableContainer>
                ) : (
                  <Typography variant="body2" color="text.secondary">
                    No usage yet.
                  </Typography>
                )}
              </Box>
            </Grid2>
          </Grid2>
        </Stack>
      ) : null}

      {tab === 7 ? ( 
        <Stack spacing={2}>
          <Box className="list-shell">
            <Stack spacing={0.6}>
              <Stack direction="row" justifyContent="space-between" alignItems="center">
                <Typography variant="h6">Moltbook</Typography>
                <FormControlLabel
                  control={
                    <Switch
                      checked={form.moltbook_enabled}
                      onChange={(e) => setField("moltbook_enabled", e.target.checked)}
                    />
                  }
                  label="Enabled"
                />
              </Stack>
              <Typography variant="body2" color="text.secondary">
                Moltbook is a decentralized social network for autonomous AI agents. When enabled, AgentArk
                can discover and collaborate with other agents, negotiate task delegation, share capabilities,
                and participate in multi-agent workflows across the network. All communication is
                zero-knowledge, and no user data, secrets, PII, or conversation content ever leaves your instance.
                Only capability metadata, anonymized skill signatures, and agent availability are shared.
              </Typography>
              <Typography variant="caption" color="text.secondary">
                Disabled by default. Your agent joins the network as a peer, and all inbound requests
                go through the same approval and action-guard rules as any other task.
              </Typography>
              {form.moltbook_enabled ? (
                <TextField
                  label="Moltbook API Key"
                  type="password"
                  value={form.moltbook_api_key}
                  onChange={(e) => setField("moltbook_api_key", e.target.value)}
                  fullWidth
                  size="small"
                  placeholder="Enter your Moltbook API key"
                  helperText="Required to connect to the Moltbook network. Get your key at moltbook.com"
                  sx={{ mt: 1 }}
                />
              ) : (
                <Typography variant="caption" color="text.secondary" sx={{ mt: 1 }}>
                  Turn on Moltbook to add your API key.
                </Typography>
              )}
            </Stack>
          </Box>

          <Grid2 container spacing={2}>
            <Grid2 size={{ xs: 12, lg: 6 }}>
              <Box className="list-shell" sx={{ minHeight: 0 }}>
                <Stack direction="row" justifyContent="space-between" alignItems="center" mb={1}>
                  <Stack spacing={0.2}>
                    <Typography variant="h6">Sync Settings</Typography>
                    <Typography variant="caption" color="text.secondary">
                      Controls background cadence and write behavior.
                    </Typography>
                  </Stack>
                </Stack>

                <Grid2 container spacing={1}>
                  <Grid2 size={{ xs: 12, md: 6 }}>
                    <FormControlLabel
                      control={
                        <Switch
                          checked={form.moltbook_defer_when_busy}
                          onChange={(e) => setField("moltbook_defer_when_busy", e.target.checked)}
                        />
                      }
                      label="Defer When Busy"
                    />
                  </Grid2>
                  <Grid2 size={{ xs: 12, md: 6 }}>
                    <FormControlLabel
                      control={
                        <Switch
                          checked={form.moltbook_write_enabled}
                          onChange={(e) => setField("moltbook_write_enabled", e.target.checked)}
                        />
                      }
                      label="Write Enabled"
                    />
                  </Grid2>
                  <Grid2 size={{ xs: 12, md: 6 }}>
                    <TextField
                      label="Mode"
                      select
                      value={form.moltbook_mode}
                      onChange={(e) => setField("moltbook_mode", e.target.value)}
                      fullWidth
                      size="small"
                    >
                      <MenuItem value="off">off</MenuItem>
                      <MenuItem value="read_only">read_only</MenuItem>
                      <MenuItem value="assist">assist</MenuItem>
                      <MenuItem value="autopost">autopost</MenuItem>
                    </TextField>
                  </Grid2>
                  <Grid2 size={{ xs: 12, md: 6 }}>
                    <TextField
                      label="Sync Frequency"
                      select
                      value={form.moltbook_sync_frequency}
                      onChange={(e) => setField("moltbook_sync_frequency", e.target.value)}
                      fullWidth
                      size="small"
                    >
                      <MenuItem value="twice_daily">twice_daily</MenuItem>
                      <MenuItem value="daily">daily</MenuItem>
                    </TextField>
                  </Grid2>
                </Grid2>
              </Box>
            </Grid2>

            <Grid2 size={{ xs: 12, lg: 6 }}>
              <Box className="list-shell" sx={{ minHeight: 0 }}>
                <Stack direction="row" justifyContent="space-between" alignItems="center" mb={1}>
                  <Stack spacing={0.2}>
                    <Typography variant="h6">Connector Status</Typography>
                    <Typography variant="caption" color="text.secondary">
                      Registration and recent runs.
                    </Typography>
                  </Stack>
                  <Button
                    size="small"
                    variant="outlined"
                    onClick={async () => {
                      setError(null);
                      setSuccess(null);
                      try {
                        const out = asRecord(await runMoltbookMutation.mutateAsync());
                        const status = str(out.status, "ok").toLowerCase();
                        if (status === "ok") {
                          const readCount = num(out.read_count, 0);
                          setSuccess(`Moltbook run completed. Read ${readCount} post${readCount === 1 ? "" : "s"}.`);
                        } else if (status === "running") {
                          setSuccess(str(out.message, "Moltbook run is already in progress."));
                        } else if (status === "not_connected") {
                          setError(str(out.reason, "Moltbook is not connected. Enter your API key above, save settings, then run again."));
                        } else if (status === "disabled") {
                          setError("Moltbook is disabled in Settings.");
                        } else if (status === "off_mode") {
                          setError("Moltbook mode is off.");
                        } else if (status === "deferred_busy" || status === "skipped_busy") {
                          setError("Moltbook run deferred because the system is busy.");
                        } else if (status === "not_due") {
                          setError("Moltbook run skipped because next scheduled run is not due yet.");
                        } else {
                          setSuccess(`Moltbook run returned status: ${status}.`);
                        }
                      } catch (e) {
                        setError(errMessage(e));
                      }
                    }}
                    disabled={runMoltbookMutation.isPending || moltbookRunning}
                  >
                    {runMoltbookMutation.isPending || moltbookRunning ? "Running..." : "Run now"}
                  </Button>
                </Stack>

                {moltbookStatusQ.error ? <Alert severity="error">{errMessage(moltbookStatusQ.error)}</Alert> : null}
                {moltbookNeedsConnection ? (
                  <Alert
                    severity="warning"
                    sx={{ mb: 1 }}
                    action={
                      <Button size="small" variant="outlined" onClick={() => setTab(2)}>
                        Open Integrations
                      </Button>
                    }
                  >
                    Moltbook is not connected. Enter your API key above, save settings, then run again.
                  </Alert>
                ) : null}
                <Grid2 container spacing={1}>
                  <Grid2 size={{ xs: 12, md: 6 }}>
                    <Typography variant="body2">Connector: {boolText(moltbookStatus.connector_registered)}</Typography>
                  </Grid2>
                  <Grid2 size={{ xs: 12, md: 6 }}>
                    <Typography variant="body2">Last status: {str(moltbookStatus.last_status, "-")}</Typography>
                  </Grid2>
                  <Grid2 size={{ xs: 12, md: 6 }}>
                    <Typography variant="body2">Last run: {str(moltbookStatus.last_run_at, "-")}</Typography>
                  </Grid2>
                  <Grid2 size={{ xs: 12, md: 6 }}>
                    <Typography variant="body2">Next run: {str(moltbookStatus.next_run_at, "-")}</Typography>
                  </Grid2>
                </Grid2>
              </Box>
            </Grid2>
          </Grid2>

          <Box className="list-shell" sx={{ minHeight: 0 }}>
            <Stack direction="row" justifyContent="space-between" alignItems="center" mb={1}>
              <Typography variant="h6">Moltbook Activity</Typography>
              <Typography variant="caption" color="text.secondary">
                Recent sync runs.
              </Typography>
            </Stack>
            {moltbookLogQ.error ? <Alert severity="error">{errMessage(moltbookLogQ.error)}</Alert> : null}
            {moltbookEvents.length === 0 ? (
              <Typography variant="body2" color="text.secondary">
                No Moltbook events yet.
              </Typography>
            ) : (
              <TableContainer className="table-shell">
                <Table size="small">
                  <TableHead>
                    <TableRow>
                      <TableCell>Timestamp</TableCell>
                      <TableCell>Level</TableCell>
                      <TableCell>Action</TableCell>
                      <TableCell>Run</TableCell>
                      <TableCell align="right">Ops</TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {moltbookEvents.slice(0, 40).map((ev, idx) => {
                      const details = asRecord(ev.details);
                      const rawAction = str(ev.action, "-");
                      const label = moltbookActionLabel(rawAction, details);
                      const reason = moltbookReason(rawAction, details);
                      const trigger = str(details.trigger, "");
                      const triggerLabel = trigger ? moltbookTriggerLabel(trigger) : "";
                      const hover = [label, triggerLabel ? `Trigger: ${triggerLabel}` : "", reason ? `Reason: ${reason}` : ""]
                        .filter(Boolean)
                        .join("\n");
                      return (
                      <TableRow key={str(ev.id, String(idx))}>
                        <TableCell sx={{ whiteSpace: "nowrap" }}>{str(ev.timestamp, "-")}</TableCell>
                        <TableCell>
                          <Chip size="small" label={str(ev.level, "-")} color={severityChipColor(str(ev.level, ""))} />
                        </TableCell>
                        <TableCell sx={{ maxWidth: 420 }}>
                          <Stack spacing={0.25}>
                            <Typography variant="body2" noWrap title={hover}>
                              {label}
                            </Typography>
                            {triggerLabel || reason ? (
                              <Typography variant="caption" color="text.secondary" noWrap title={hover}>
                                {triggerLabel ? triggerLabel : ""}{triggerLabel && reason ? " | " : ""}{reason ? reason : ""}
                              </Typography>
                            ) : null}
                          </Stack>
                        </TableCell>
                        <TableCell sx={{ maxWidth: 260 }}>
                          <Typography variant="body2" noWrap title={str(ev.run_id, "-")}>
                            {str(ev.run_id, "-")}
                          </Typography>
                        </TableCell>
                        <TableCell align="right">
                          <RowOpsMenu
                            actions={[
                              {
                                label: "View",
                                onClick: () => setSelectedMoltbookEvent(ev)
                              }
                            ]}
                            ariaLabel="Moltbook event options"
                          />
                        </TableCell>
                      </TableRow>
                      );
                    })}
                  </TableBody>
                </Table>
              </TableContainer>
            )}
          </Box>
        </Stack>
      ) : null}

      {tab === 2 ? (
        <Box className="list-shell">
          <IntegrationsPanel autoRefresh={autoRefresh} embedded mode="integrations" />
        </Box>
      ) : null}

      {tab === 11 ? <TraceManager autoRefresh={autoRefresh} /> : null}

      {tab === 8 ? (
        <Box className="list-shell">
          <IntegrationsPanel autoRefresh={autoRefresh} embedded mode="mcp" />
        </Box>
      ) : null}

      {tab === 12 ? <MemoryManager autoRefresh={autoRefresh} /> : null}

      {tab === 9 ? ( 
        <Stack spacing={2}>
          <Grid2 container spacing={2} alignItems="stretch">
            <Grid2 size={{ xs: 12 }}>
              <Box className="list-shell" sx={{ minHeight: 0, height: "100%", display: "flex", flexDirection: "column" }}>
                <Stack direction="row" justifyContent="space-between" alignItems="center" mb={1}>
                  <Typography variant="h6">ArkPulse</Typography>
                  <Button
                    size="small"
                    onClick={async () => {
                      setError(null);
                      try {
                        const out = asRecord(await triggerPulseMutation.mutateAsync());
                        const status = str(out.status, "").toLowerCase();
                        if (status === "running") {
                          setSuccess(str(out.message, "ArkPulse is already running."));
                        } else {
                          setSuccess(str(out.message, "ArkPulse check started."));
                        }
                      } catch (e) {
                        setError(errMessage(e));
                      }
                    }}
                    disabled={triggerPulseMutation.isPending || pulseRunning}
                  >
                    {triggerPulseMutation.isPending || pulseRunning ? "Running..." : "Run now"}
                  </Button>
                </Stack>
                {pulseQ.error ? <Alert severity="error">{errMessage(pulseQ.error)}</Alert> : null}
                {!pulseQ.error ? (
                  <Alert severity={pulseRunning ? "info" : latestPulseFindingsCount > 0 ? "warning" : "success"} sx={{ mb: 1 }}>
                    <Typography variant="subtitle2">{latestPulseHeadline}</Typography>
                    <Typography variant="body2" color="text.secondary">
                      {latestPulseSubtitle}
                    </Typography>
                  </Alert>
                ) : null}
                {pulseEvents.length === 0 ? (
                  <Stack spacing={1} sx={{ flex: 1 }}>
                    <Typography variant="body2" color="text.secondary">
                      No ArkPulse events yet.
                    </Typography>
                    <Box className="metadata-box" sx={{ maxHeight: "none" }}>
                      <Typography variant="caption" color="text.secondary">
                        What is ArkPulse?
                      </Typography>
                      <Stack spacing={0.6} sx={{ mt: 0.75 }}>
                        <Typography variant="body2" color="text.secondary">
                          Periodic system check that summarizes operational health, safety posture, and execution drift.
                        </Typography>
                        <Typography variant="body2" color="text.secondary">
                          Run it after changing models, channels, or adding a new integration.
                        </Typography>
                        <Typography variant="body2" color="text.secondary">
                          Results show up here as an event stream with findings and a score.
                        </Typography>
                      </Stack>
                    </Box>
                    <Box sx={{ flex: 1 }} />
                  </Stack>
                ) : (
                  <TableContainer className="table-shell" sx={{ flex: 1, minHeight: 0 }}>
                    <Table size="small">
                      <TableHead>
                        <TableRow>
                          <TableCell>Captured</TableCell>
                          <TableCell>Result</TableCell>
                          <TableCell>Health</TableCell>
                          <TableCell>Issues</TableCell>
                          <TableCell>Next step</TableCell>
                          <TableCell align="right">Ops</TableCell>
                        </TableRow>
                      </TableHead>
                      <TableBody>
                        {pulseEvents.slice(0, 40).map((ev, idx) => {
                          const details = asRecord(ev.details);
                          const findings = Array.isArray(details.doctor_findings) ? details.doctor_findings : [];
                          const score = num(details.doctor_score, -1);
                          const status = str(ev.status, "-");
                          const ok = status.toLowerCase() === "ok";
                          const nextStep =
                            Array.isArray(findings) && findings.length > 0
                              ? "Open details and run Fix #1"
                              : "No action needed";
                          return (
                            <TableRow key={str(ev.id, String(idx))}>
                              <TableCell sx={{ whiteSpace: "nowrap" }}>{str(ev.timestamp, "-")}</TableCell>
                              <TableCell>
                                <Chip
                                  size="small"
                                  label={ok ? "OK" : status || "check"}
                                  color={ok ? "success" : "warning"}
                                  variant={ok ? "filled" : "outlined"}
                                />
                              </TableCell>
                              <TableCell>{score >= 0 ? score : "-"}</TableCell>
                              <TableCell>{Array.isArray(findings) ? findings.length : 0}</TableCell>
                              <TableCell sx={{ maxWidth: 320 }}>
                                <Typography variant="body2" noWrap title={nextStep}>
                                  {nextStep}
                                </Typography>
                              </TableCell>
                              <TableCell align="right">
                                <RowOpsMenu
                                  actions={[
                                    {
                                      label: "View",
                                      onClick: () => setSelectedPulseEvent(ev)
                                    }
                                  ]}
                                  ariaLabel="ArkPulse event options"
                                />
                              </TableCell>
                            </TableRow>
                          );
                        })}
                      </TableBody>
                    </Table>
                  </TableContainer>
                )}
              </Box>
            </Grid2>
          </Grid2>
        </Stack>
      ) : null}

      <Dialog open={selectedPulseEvent != null} onClose={() => setSelectedPulseEvent(null)} maxWidth="lg" fullWidth>
        <DialogTitle>{str(selectedPulseEvent?.summary, "ArkPulse Details")}</DialogTitle>
        <DialogContent>
          <Stack spacing={1.25}>
            <Stack direction={{ xs: "column", sm: "row" }} spacing={1} alignItems={{ xs: "flex-start", sm: "center" }}>
              <Chip size="small" variant="outlined" label={`Captured: ${selectedPulseCaptured.label}`} title={selectedPulseCaptured.tooltip} />
              <Chip
                size="small"
                label={`Status: ${selectedPulseStatus}`}
                color={selectedPulseStatusOk ? "success" : "warning"}
                variant={selectedPulseStatusOk ? "filled" : "outlined"}
              />
            </Stack>
            <Alert severity={selectedPulseGuidance.severity} variant="outlined">
              <Typography variant="subtitle2">{selectedPulseGuidance.title}</Typography>
              <Typography variant="body2" color="text.secondary">
                {selectedPulseGuidance.detail}
              </Typography>
            </Alert>
            <Divider />
            <Grid2 container spacing={1}>
              <Grid2 size={{ xs: 12, md: 4 }}>
                <Box className="metadata-box">
                  <Typography variant="caption" color="text.secondary">
                    Health score
                  </Typography>
                  <Typography variant="h5">
                    {selectedPulseScore >= 0 ? selectedPulseScore : "-"}
                  </Typography>
                </Box>
              </Grid2>
              <Grid2 size={{ xs: 12, md: 4 }}>
                <Box className="metadata-box">
                  <Typography variant="caption" color="text.secondary">
                    Findings
                  </Typography>
                  <Typography variant="h5">{selectedPulseFindings.length}</Typography>
                </Box>
              </Grid2>
              <Grid2 size={{ xs: 12, md: 4 }}>
                <Box className="metadata-box">
                  <Typography variant="caption" color="text.secondary">
                    Watchers
                  </Typography>
                  <Typography variant="h5">{num(selectedPulseDetails.active_watchers, 0)}</Typography>
                </Box>
              </Grid2>
            </Grid2>

            <Typography variant="subtitle2" mt={1}>
              Fix these first
            </Typography>
            {selectedPulseFindings.length === 0 ? (
              <Alert severity="success" variant="outlined">
                No findings in this run.
              </Alert>
            ) : (
              <Stack spacing={1}>
                {selectedPulseFindings.slice(0, 20).map((f, idx) => {
                  const fr = asRecord(f);
                  const sev = str(fr.severity, "");
                  const title = str(fr.title, "Issue");
                  const target = str(fr.target, "-");
                  const cause = str(fr.root_cause, "-");
                  const fix = str(fr.fix_command, "-");
                  return (
                    <Box key={`${title}-${idx}`} className="metadata-box">
                      <Stack spacing={0.75}>
                        <Stack direction="row" spacing={1} alignItems="center" flexWrap="wrap" useFlexGap>
                          <Chip size="small" label={sev || "-"} color={severityChipColor(sev)} />
                          <Typography variant="subtitle2">{`Fix #${idx + 1}: ${title}`}</Typography>
                        </Stack>
                        <Typography variant="body2" color="text.secondary">
                          Target: {target}
                        </Typography>
                        <Typography variant="body2" color="text.secondary">
                          Why this matters: {cause}
                        </Typography>
                        <Box
                          sx={{
                            border: "1px solid rgba(62,143,214,0.24)",
                            borderRadius: 1,
                            p: 1,
                            background: "rgba(5,16,31,0.45)"
                          }}
                        >
                          <Typography variant="caption" color="text.secondary">
                            Recommended fix command
                          </Typography>
                          <Typography
                            variant="body2"
                            sx={{
                              mt: 0.5,
                              fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
                              whiteSpace: "pre-wrap",
                              overflowWrap: "anywhere"
                            }}
                          >
                            {fix}
                          </Typography>
                        </Box>
                      </Stack>
                    </Box>
                  );
                })}
              </Stack>
            )}

            <Typography variant="subtitle2" mt={0.5}>
              Current system snapshot
            </Typography>
            <Grid2 container spacing={1}>
              {selectedPulseSnapshot.map((item) => (
                <Grid2 key={item.label} size={{ xs: 6, md: 3 }}>
                  <Box className="metadata-box" sx={{ minHeight: 86 }}>
                    <Typography variant="caption" color="text.secondary">
                      {item.label}
                    </Typography>
                    <Typography variant="h6">{item.value}</Typography>
                  </Box>
                </Grid2>
              ))}
            </Grid2>

            {developerModeEnabled ? (
              <Accordion disableGutters sx={{ background: "transparent", boxShadow: "none", border: "1px solid rgba(62,143,214,0.24)", borderRadius: 1 }}>
                <AccordionSummary expandIcon={<ExpandMoreIcon />}>
                  <Typography variant="subtitle2">Technical signals (developer mode)</Typography>
                </AccordionSummary>
                <AccordionDetails sx={{ pt: 0 }}>
                  <KeyValuePanel title="Raw signals" data={asRecord(selectedPulseEvent?.details)} emptyLabel="No extra signals." maxRows={24} />
                </AccordionDetails>
              </Accordion>
            ) : null}
          </Stack>
        </DialogContent>
      </Dialog>

      {settingsQ.isLoading || mediaQ.isLoading ? (
        <Typography variant="body2" color="text.secondary">
          Loading settings...
        </Typography>
      ) : null}
      <Dialog
        open={selectedMoltbookEvent != null}
        onClose={() => setSelectedMoltbookEvent(null)}
        maxWidth="md"
        fullWidth
      >
        <DialogTitle>
          {moltbookActionLabel(str(selectedMoltbookEvent?.action, ""), asRecord(selectedMoltbookEvent?.details))}
        </DialogTitle>
        <DialogContent>
          <Stack spacing={1}>
            <Typography variant="caption" color="text.secondary">
              {str(selectedMoltbookEvent?.timestamp)} | Level: {str(selectedMoltbookEvent?.level)} | Run:{" "}
              {str(selectedMoltbookEvent?.run_id, "-")}
            </Typography>
            {(() => {
              const details = asRecord(selectedMoltbookEvent?.details);
              const action = str(selectedMoltbookEvent?.action, "");
              const reason = moltbookReason(action, details);
              const trigger = str(details.trigger, "");
              const apiUrl = str(details.api_url, "");
              const postApiUrl = str(details.post_api_url, "");
              const extraUrls = [apiUrl, postApiUrl].filter((u) => !!u.trim());
              return (
                <Stack spacing={0.75}>
                  {trigger ? (
                    <Typography variant="body2" color="text.secondary">
                      Trigger: {moltbookTriggerLabel(trigger)}
                    </Typography>
                  ) : null}
                  {reason ? <Alert severity="info">Reason: {reason}</Alert> : null}
                  {extraUrls.length ? (
                    <Box className="metadata-box">
                      <Typography variant="caption" color="text.secondary">
                        URLs
                      </Typography>
                      <Stack spacing={0.4} sx={{ mt: 0.6 }}>
                        {extraUrls.map((u) => (
                          <Typography key={u} variant="body2" sx={{ wordBreak: "break-all" }}>
                            <a href={u} target="_blank" rel="noreferrer" style={{ color: "inherit" }}>
                              {u}
                            </a>
                          </Typography>
                        ))}
                      </Stack>
                    </Box>
                  ) : null}
                </Stack>
              );
            })()}
            <Divider />
            <KeyValuePanel
              title="Details"
              data={asRecord(selectedMoltbookEvent?.details)}
              emptyLabel="No extra details."
              maxRows={18}
            />
          </Stack>
        </DialogContent>
      </Dialog>
      <Dialog
        open={vaultEditorOpen}
        onClose={closeVaultEditor}
        maxWidth="sm"
        fullWidth
      >
        <DialogTitle>{vaultEditorMode === "edit" ? "Edit Secret" : "Add New Secret"}</DialogTitle>
        <DialogContent>
          <Stack spacing={1.2} sx={{ mt: 0.5 }}>
            <TextField
              label="Secret key"
              value={vaultEditorKey}
              onChange={(e) => setVaultEditorKey(e.target.value)}
              fullWidth
              size="small"
              disabled={vaultEditorMode === "edit"}
              helperText="Allowed: letters, numbers, _, -, :, ."
            />
            <TextField
              label="Secret value"
              value={vaultEditorValue}
              onChange={(e) => setVaultEditorValue(e.target.value)}
              fullWidth
              size="small"
              multiline
              minRows={3}
              type={showVaultSecretValue ? "text" : "password"}
              placeholder={vaultEditorMode === "edit" ? "Enter new value" : "Paste secret value"}
            />
            <FormControlLabel
              control={
                <Switch
                  checked={showVaultSecretValue}
                  onChange={(e) => setShowVaultSecretValue(e.target.checked)}
                />
              }
              label="Show secret value"
            />
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={closeVaultEditor} disabled={upsertVaultSecretMutation.isPending}>
            Cancel
          </Button>
          <Button
            variant="contained"
            onClick={submitVaultEditor}
            disabled={upsertVaultSecretMutation.isPending}
          >
            {upsertVaultSecretMutation.isPending
              ? "Saving..."
              : vaultEditorMode === "edit"
                ? "Update Secret"
                : "Save Secret"}
          </Button>
        </DialogActions>
      </Dialog>
      <Dialog
        open={passwordDialogMode != null}
        onClose={closePasswordDialog}
        maxWidth="sm"
        fullWidth
      >
        <DialogTitle>
          {passwordDialogMode === "set"
            ? "Set Master Password"
            : passwordDialogMode === "change"
              ? "Change Master Password"
              : "Remove Master Password"}
        </DialogTitle>
        <DialogContent>
          <Stack spacing={1.2} sx={{ mt: 0.5 }}>
            <Alert severity="warning">
              Saving this will restart the server and reconnect active sessions.
            </Alert>
            <FormControlLabel
              control={
                <Switch
                  checked={showPasswordInputs}
                  onChange={(e) => setShowPasswordInputs(e.target.checked)}
                />
              }
              label="Show password text"
            />
            {passwordDialogMode === "set" ? (
              <>
                <TextField
                  label="New password (min 8 chars)"
                  value={secNewPassword}
                  onChange={(e) => setSecNewPassword(e.target.value)}
                  fullWidth
                  type={showPasswordInputs ? "text" : "password"}
                  size="small"
                />
                <TextField
                  label="Confirm new password"
                  value={secConfirmPassword}
                  onChange={(e) => setSecConfirmPassword(e.target.value)}
                  fullWidth
                  type={showPasswordInputs ? "text" : "password"}
                  size="small"
                />
              </>
            ) : null}
            {passwordDialogMode === "change" ? (
              <>
                <TextField
                  label="Current password (blank uses default, if applicable)"
                  value={secCurrentPassword}
                  onChange={(e) => setSecCurrentPassword(e.target.value)}
                  fullWidth
                  type={showPasswordInputs ? "text" : "password"}
                  size="small"
                />
                <TextField
                  label="New password (min 8 chars)"
                  value={secNewPassword}
                  onChange={(e) => setSecNewPassword(e.target.value)}
                  fullWidth
                  type={showPasswordInputs ? "text" : "password"}
                  size="small"
                />
                <TextField
                  label="Confirm new password"
                  value={secConfirmPassword}
                  onChange={(e) => setSecConfirmPassword(e.target.value)}
                  fullWidth
                  type={showPasswordInputs ? "text" : "password"}
                  size="small"
                />
              </>
            ) : null}
            {passwordDialogMode === "remove" ? (
              <>
                <Typography variant="body2" color="text.secondary">
                  Removes the master password and returns to keyfile-based encryption.
                </Typography>
                <TextField
                  label="Current password"
                  value={secCurrentPassword}
                  onChange={(e) => setSecCurrentPassword(e.target.value)}
                  fullWidth
                  type={showPasswordInputs ? "text" : "password"}
                  size="small"
                />
              </>
            ) : null}
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={closePasswordDialog} disabled={passwordMutationPending}>
            Cancel
          </Button>
          <Button
            variant="contained"
            color={passwordDialogMode === "remove" ? "error" : "primary"}
            onClick={submitPasswordDialog}
            disabled={passwordMutationPending}
          >
            {passwordMutationPending
              ? "Saving..."
              : passwordDialogMode === "set"
                ? "Set Password"
                : passwordDialogMode === "change"
                  ? "Change Password"
                  : "Remove Password"}
          </Button>
        </DialogActions>
      </Dialog>
      {settingsQ.error || mediaQ.error || modelsQ.error || moltbookLogQ.error ? (
        <Alert severity="error">{errMessage(settingsQ.error || mediaQ.error || modelsQ.error || moltbookLogQ.error)}</Alert>
      ) : null}
      {error ? <Alert severity="error">{error}</Alert> : null}
      {modelConnectivityWarning ? <Alert severity="warning">{modelConnectivityWarning}</Alert> : null}
      {success ? <Alert severity="success">{success}</Alert> : null}
    </Stack>
  );
}

export function NativeWorkspace({
  view,
  autoRefresh,
  showAdvanced
}: {
  view: WorkspaceView;
  autoRefresh: boolean;
  showAdvanced: boolean;
}) {
  const isChat = view === "chat";
  return (
    <Box
      sx={{
        p: 1,
        height: "calc(100vh - 84px)",
        overflow: isChat ? "hidden" : "auto",
        display: "flex",
        flexDirection: "column",
        minHeight: 0
      }}
    >
      {view === "chat" ? <ChatManager autoRefresh={autoRefresh} /> : null}
      {view === "tasks" ? <TasksManager autoRefresh={autoRefresh} /> : null}
      {view === "skills" ? <SkillsManager autoRefresh={autoRefresh} /> : null}
      {view === "apps" ? <AppsManager autoRefresh={autoRefresh} /> : null}
      {view === "goals" ? <GoalsManager autoRefresh={autoRefresh} /> : null}
      {view === "autonomy" ? <AutonomyManager autoRefresh={autoRefresh} /> : null}
      {view === "documents" ? <DocumentsManager autoRefresh={autoRefresh} /> : null}
      {view === "projects" ? <ProjectsManager autoRefresh={autoRefresh} /> : null}
      {view === "swarm" ? <SwarmManager autoRefresh={autoRefresh} /> : null}
      {view === "trace" ? <TraceManager autoRefresh={autoRefresh} /> : null}
      {view === "status" ? <StatusManager autoRefresh={autoRefresh} /> : null}
      {view === "settings" ? <SettingsManager autoRefresh={autoRefresh} /> : null}
      {["tasks", "skills", "apps"].includes(view) ? <Divider sx={{ mt: 2 }} /> : null}
    </Box>
  );
}

