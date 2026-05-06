import {
  Alert,
  Box,
  Button,
  Chip,
  Divider,
  LinearProgress,
  Stack,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Tooltip,
  Typography,
} from "@mui/material";
import Grid2 from "@mui/material/Grid";
import AutoGraphRoundedIcon from "@mui/icons-material/AutoGraphRounded";
import BubbleChartRoundedIcon from "@mui/icons-material/BubbleChartRounded";
import CalendarMonthRoundedIcon from "@mui/icons-material/CalendarMonthRounded";
import ChatRoundedIcon from "@mui/icons-material/ChatRounded";
import HubRoundedIcon from "@mui/icons-material/HubRounded";
import MemoryRoundedIcon from "@mui/icons-material/MemoryRounded";
import MonitorHeartRoundedIcon from "@mui/icons-material/MonitorHeartRounded";
import RefreshRoundedIcon from "@mui/icons-material/RefreshRounded";
import WorkHistoryRoundedIcon from "@mui/icons-material/WorkHistoryRounded";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import ReactECharts from "echarts-for-react";
import { api } from "../../api/client";
import {
  formatUiDateOnly,
  formatUiDateRange,
  formatUiDateTime,
} from "../../lib/dateFormat";
import { WorkspacePageHeader, WorkspacePageShell } from "../WorkspacePage";
import { asRecord, errMessage, num, pickRecords, str } from "./pageHelpers";

type ArkReflectPageProps = {
  autoRefresh: boolean;
};

type ReflectPeriod = "daily" | "weekly" | "monthly";
type ReflectStoryTab = "studio" | "patterns" | "achievements" | "dreams" | "replay";

type ReflectUnit = {
  id: string;
  source_kind: string;
  source_label: string;
  channel: string;
  title: string;
  summary: string;
  content_preview: string;
  occurred_at: string;
  message_count: number;
  has_embedding: boolean;
};

type ReflectRelatedUnit = {
  id: string;
  source_label: string;
  title: string;
  occurred_at: string;
  similarity: number;
};

type ReflectRelatedHistory = {
  mode: string;
  similar_count: number;
  most_recent_at: string;
  top_similarity: number | null;
  detail: string;
  items: ReflectRelatedUnit[];
};

type ReflectCluster = {
  id: string;
  label: string;
  plain_summary: string;
  unit_count: number;
  message_count: number;
  source_mix: Record<string, number>;
  color: string;
  related_history: ReflectRelatedHistory;
  units: ReflectUnit[];
};

type ReflectSourceCounts = {
  main_chat: number;
  orbit_chat: number;
  memory: number;
  procedures: number;
  apps: number;
  goals: number;
  watchers: number;
  sentinel: number;
  arkpulse: number;
  arkevolve: number;
  usage: number;
};

type ReflectSuggestedFollowup = {
  id: string;
  kind: string;
  title: string;
  detail: string;
  prompt: string;
  status: string;
  source_label: string;
  occurred_at: string;
  conversation_id?: string | null;
  source_unit_id?: string | null;
  rank_score: number;
};

type ReflectResponse = {
  period: ReflectPeriod;
  from: string;
  to: string;
  generated_at: string;
  source_counts: ReflectSourceCounts;
  baseline_source_counts: ReflectSourceCounts;
  embedding_status: {
    mode: string;
    embedded_units: number;
    total_units: number;
    detail: string;
  };
  refresh_status: {
    running: boolean;
    status: string;
    trigger: string;
    completed_at: string;
    last_error: string;
  };
  cache_status: {
    mode: string;
    cached_units: number;
    stale: boolean;
    detail: string;
  };
  daily_digest_status: {
    enabled: boolean;
    status: string;
    target_date: string;
    today_date: string;
    meaningful: boolean;
    unit_count: number;
    cluster_count: number;
    source_counts: ReflectSourceCounts;
    summary: string;
    detail: string;
    last_checked_at: string;
    last_sent_at: string;
    last_skipped_at: string;
    last_error: string;
  };
  suggested_followups: ReflectSuggestedFollowup[];
  clusters: ReflectCluster[];
  unclustered_units: ReflectUnit[];
};

const PERIOD_OPTIONS: { value: ReflectPeriod; label: string }[] = [
  { value: "daily", label: "Day" },
  { value: "weekly", label: "Week" },
  { value: "monthly", label: "Month" },
];

const SOURCE_DISPLAY: Record<string, { label: string; group: string; color: string }> = {
  conversation: { label: "Chat", group: "Conversation work", color: "#4E8DFF" },
  orbit_chat: { label: "ArkOrbit", group: "Orbit conversations", color: "#7C5CFF" },
  experience_item: { label: "Memory", group: "What AgentArk learned", color: "#21B573" },
  procedural_pattern: { label: "Workflows", group: "Working patterns", color: "#E6A93D" },
  app: { label: "Apps", group: "Apps built", color: "#00A8A8" },
  goal: { label: "Goals", group: "Goals and progress", color: "#FF7A45" },
  watcher: { label: "Watchers", group: "Background watchers", color: "#D94F70" },
  sentinel: { label: "Sentinel", group: "Safety and checks", color: "#A96DFF" },
  arkpulse: { label: "ArkPulse", group: "System health", color: "#00B8D9" },
  arkevolve: { label: "ArkEvolve", group: "Agent improvements", color: "#C58A00" },
  llm_usage: { label: "Usage", group: "Agent usage", color: "#8FA3BF" },
};

const SOURCE_ORDER = [
  "conversation",
  "orbit_chat",
  "experience_item",
  "procedural_pattern",
  "app",
  "goal",
  "watcher",
  "sentinel",
  "arkpulse",
  "arkevolve",
  "llm_usage",
] as const;

function pad(value: number): string {
  return String(value).padStart(2, "0");
}

function toDateInputValue(date: Date): string {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

function parseDateInput(value: string): Date {
  const [yearRaw, monthRaw, dayRaw] = value.split("-").map((part) => Number(part));
  const year = Number.isFinite(yearRaw) ? yearRaw : new Date().getFullYear();
  const month = Number.isFinite(monthRaw) ? monthRaw - 1 : new Date().getMonth();
  const day = Number.isFinite(dayRaw) ? dayRaw : new Date().getDate();
  return new Date(year, month, day);
}

function addDays(date: Date, days: number): Date {
  const next = new Date(date);
  next.setDate(next.getDate() + days);
  return next;
}

function startOfLocalDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

function periodBounds(period: ReflectPeriod, anchorValue: string): { from: Date; to: Date } {
  const anchor = startOfLocalDay(parseDateInput(anchorValue));
  if (period === "daily") {
    return { from: anchor, to: addDays(anchor, 1) };
  }
  if (period === "monthly") {
    return {
      from: new Date(anchor.getFullYear(), anchor.getMonth(), 1),
      to: new Date(anchor.getFullYear(), anchor.getMonth() + 1, 1),
    };
  }
  const dayOffset = (anchor.getDay() + 6) % 7;
  const from = addDays(anchor, -dayOffset);
  return { from, to: addDays(from, 7) };
}

function asReflectUnit(value: unknown): ReflectUnit | null {
  const raw = asRecord(value);
  const id = str(raw.id, "");
  if (!id) return null;
  return {
    id,
    source_kind: str(raw.source_kind, "work"),
    source_label: str(raw.source_label, "Work"),
    channel: str(raw.channel, ""),
    title: str(raw.title, "Untitled work"),
    summary: str(raw.summary, ""),
    content_preview: str(raw.content_preview, ""),
    occurred_at: str(raw.occurred_at, ""),
    message_count: num(raw.message_count, 0),
    has_embedding: Boolean(raw.has_embedding),
  };
}

function asRelatedUnit(value: unknown): ReflectRelatedUnit | null {
  const raw = asRecord(value);
  const id = str(raw.id, "");
  if (!id) return null;
  return {
    id,
    source_label: str(raw.source_label, "Work"),
    title: str(raw.title, "Related work"),
    occurred_at: str(raw.occurred_at, ""),
    similarity: num(raw.similarity, 0),
  };
}

function asRelatedHistory(value: unknown): ReflectRelatedHistory {
  const raw = asRecord(value);
  const topSimilarityRaw = raw.top_similarity;
  const topSimilarity = typeof topSimilarityRaw === "number" && Number.isFinite(topSimilarityRaw)
    ? topSimilarityRaw
    : null;
  return {
    mode: str(raw.mode, "unavailable"),
    similar_count: num(raw.similar_count, 0),
    most_recent_at: str(raw.most_recent_at, ""),
    top_similarity: topSimilarity,
    detail: str(raw.detail, ""),
    items: pickRecords(raw, "items")
      .map(asRelatedUnit)
      .filter((item): item is ReflectRelatedUnit => item !== null),
  };
}

function asSuggestedFollowup(value: unknown): ReflectSuggestedFollowup | null {
  const raw = asRecord(value);
  const id = str(raw.id, "");
  if (!id) return null;
  return {
    id,
    kind: str(raw.kind, "followup"),
    title: str(raw.title, "Suggested follow-up"),
    detail: str(raw.detail, ""),
    prompt: str(raw.prompt, ""),
    status: str(raw.status, "ready"),
    source_label: str(raw.source_label, "ArkReflect"),
    occurred_at: str(raw.occurred_at, ""),
    conversation_id: str(raw.conversation_id, "") || null,
    source_unit_id: str(raw.source_unit_id, "") || null,
    rank_score: num(raw.rank_score, 0),
  };
}

function asReflectCluster(value: unknown): ReflectCluster | null {
  const raw = asRecord(value);
  const id = str(raw.id, "");
  if (!id) return null;
  const sourceMixRaw = asRecord(raw.source_mix);
  const source_mix = Object.fromEntries(
    Object.entries(sourceMixRaw).map(([key, value]) => [key, num(value, 0)]),
  );
  return {
    id,
    label: str(raw.label, "Related work"),
    plain_summary: str(raw.plain_summary, ""),
    unit_count: num(raw.unit_count, 0),
    message_count: num(raw.message_count, 0),
    source_mix,
    color: str(raw.color, "#2F80ED"),
    related_history: asRelatedHistory(raw.related_history),
    units: pickRecords(raw, "units")
      .map(asReflectUnit)
      .filter((unit): unit is ReflectUnit => unit !== null),
  };
}

function parseSourceCounts(value: unknown): ReflectSourceCounts {
  const sourceCounts = asRecord(value);
  return {
    main_chat: num(sourceCounts.main_chat, 0),
    orbit_chat: num(sourceCounts.orbit_chat, 0),
    memory: num(sourceCounts.memory, 0),
    procedures: num(sourceCounts.procedures, 0),
    apps: num(sourceCounts.apps, 0),
    goals: num(sourceCounts.goals, 0),
    watchers: num(sourceCounts.watchers, 0),
    sentinel: num(sourceCounts.sentinel, 0),
    arkpulse: num(sourceCounts.arkpulse, 0),
    arkevolve: num(sourceCounts.arkevolve, 0),
    usage: num(sourceCounts.usage, 0),
  };
}

function parseReflectResponse(value: unknown, period: ReflectPeriod): ReflectResponse {
  const raw = asRecord(value);
  const embedding = asRecord(raw.embedding_status);
  const digest = asRecord(raw.daily_digest_status);
  return {
    period,
    from: str(raw.from, ""),
    to: str(raw.to, ""),
    generated_at: str(raw.generated_at, ""),
    source_counts: parseSourceCounts(raw.source_counts),
    baseline_source_counts: parseSourceCounts(raw.baseline_source_counts),
    embedding_status: {
      mode: str(embedding.mode, "activity"),
      embedded_units: num(embedding.embedded_units, 0),
      total_units: num(embedding.total_units, 0),
      detail: str(embedding.detail, ""),
    },
    refresh_status: {
      running: Boolean(asRecord(raw.refresh_status).running),
      status: str(asRecord(raw.refresh_status).status, "idle"),
      trigger: str(asRecord(raw.refresh_status).trigger, ""),
      completed_at: str(asRecord(raw.refresh_status).completed_at, ""),
      last_error: str(asRecord(raw.refresh_status).last_error, ""),
    },
    cache_status: {
      mode: str(asRecord(raw.cache_status).mode, "empty"),
      cached_units: num(asRecord(raw.cache_status).cached_units, 0),
      stale: Boolean(asRecord(raw.cache_status).stale),
      detail: str(asRecord(raw.cache_status).detail, ""),
    },
    daily_digest_status: {
      enabled: Boolean(digest.enabled),
      status: str(digest.status, "disabled"),
      target_date: str(digest.target_date, ""),
      today_date: str(digest.today_date, ""),
      meaningful: Boolean(digest.meaningful),
      unit_count: num(digest.unit_count, 0),
      cluster_count: num(digest.cluster_count, 0),
      source_counts: parseSourceCounts(digest.source_counts),
      summary: str(digest.summary, ""),
      detail: str(digest.detail, ""),
      last_checked_at: str(digest.last_checked_at, ""),
      last_sent_at: str(digest.last_sent_at, ""),
      last_skipped_at: str(digest.last_skipped_at, ""),
      last_error: str(digest.last_error, ""),
    },
    suggested_followups: pickRecords(raw, "suggested_followups")
      .map(asSuggestedFollowup)
      .filter((item): item is ReflectSuggestedFollowup => item !== null),
    clusters: pickRecords(raw, "clusters")
      .map(asReflectCluster)
      .filter((cluster): cluster is ReflectCluster => cluster !== null),
    unclustered_units: pickRecords(raw, "unclustered_units")
      .map(asReflectUnit)
      .filter((unit): unit is ReflectUnit => unit !== null),
  };
}

function sourceIcon(label: string) {
  const lower = label.toLowerCase();
  if (lower.includes("orbit")) return <HubRoundedIcon fontSize="small" />;
  if (lower.includes("memory")) return <MemoryRoundedIcon fontSize="small" />;
  return <ChatRoundedIcon fontSize="small" />;
}

function relatedHistoryText(history: ReflectRelatedHistory): string {
  if (history.mode === "recurring") {
    const when = history.most_recent_at
      ? `, most recently ${formatUiDateOnly(history.most_recent_at, { fallback: history.most_recent_at })}`
      : "";
    return `Similar work appeared ${history.similar_count} time${history.similar_count === 1 ? "" : "s"} before${when}.`;
  }
  if (history.mode === "new") return "No close match found in reflection history.";
  return "History comparison appears when enough cached data exists.";
}

function unitDisplayTitle(unit: ReflectUnit): string {
  const title = unit.title.trim();
  if (unit.source_kind === "llm_usage") return "Usage summary";
  if (title.length < 8) return sourceMeta(unit.source_kind).group;
  return title;
}

type StyleSignal = {
  key: string;
  label: string;
  current: number;
  baseline: number;
  delta: number;
};

function styleBuckets(counts: ReflectSourceCounts | undefined): Record<string, number> {
  return {
    Conversations:
      countForSourceCounts(counts, "conversation") + countForSourceCounts(counts, "orbit_chat"),
    Building: countForSourceCounts(counts, "app") + countForSourceCounts(counts, "goal"),
    Memory:
      countForSourceCounts(counts, "experience_item") +
      countForSourceCounts(counts, "procedural_pattern"),
    Background:
      countForSourceCounts(counts, "watcher") +
      countForSourceCounts(counts, "sentinel") +
      countForSourceCounts(counts, "arkpulse") +
      countForSourceCounts(counts, "arkevolve"),
    Usage: countForSourceCounts(counts, "llm_usage"),
  };
}

function workingStyleSignals(response: ReflectResponse | undefined): StyleSignal[] {
  const current = styleBuckets(response?.source_counts);
  const baseline = styleBuckets(response?.baseline_source_counts);
  const currentTotal = Object.values(current).reduce((sum, value) => sum + value, 0);
  const baselineTotal = Object.values(baseline).reduce((sum, value) => sum + value, 0);
  return Object.keys(current).map((key) => {
    const currentShare = currentTotal > 0 ? current[key] / currentTotal : 0;
    const baselineShare = baselineTotal > 0 ? baseline[key] / baselineTotal : 1 / Object.keys(current).length;
    const delta = currentShare - baselineShare;
    return {
      key,
      label: key,
      current: currentShare,
      baseline: baselineShare,
      delta,
    };
  });
}

function narrativeLines(
  response: ReflectResponse | undefined,
  focusLabel: string,
  totalUnits: number,
  learnedCount: number,
  backgroundCount: number,
  recurringCount: number,
): string[] {
  if (!response || totalUnits === 0) {
    return [
      "I do not have enough cached activity for this range yet.",
      "When the background refresh finishes, I will summarize the main focus areas, working style, background activity, and recurring themes here.",
    ];
  }
  const style = workingStyleSignals(response)
    .slice()
    .sort((left, right) => Math.abs(right.delta) - Math.abs(left.delta))[0];
  const styleText =
    style && Math.abs(style.delta) > 0.08
      ? `${style.label.toLowerCase()} stood out compared with your recent baseline`
      : "your activity stayed close to your recent baseline";
  return [
    `I saw ${totalUnits} reflected item${totalUnits === 1 ? "" : "s"} in this range, with ${focusLabel.toLowerCase()} as the clearest focus.`,
    `${styleText}.`,
    `AgentArk also captured ${learnedCount} learned signal${learnedCount === 1 ? "" : "s"} and ${backgroundCount} background event${backgroundCount === 1 ? "" : "s"}.`,
    recurringCount > 0
      ? `${recurringCount} theme${recurringCount === 1 ? "" : "s"} connected back to earlier work.`
      : "Most visible themes look new for this cached history window.",
  ];
}

function countForSource(response: ReflectResponse | undefined, source: string): number {
  if (!response) return 0;
  return countForSourceCounts(response.source_counts, source);
}

function countForSourceCounts(counts: ReflectSourceCounts | undefined, source: string): number {
  if (!counts) return 0;
  switch (source) {
    case "conversation":
      return counts.main_chat;
    case "orbit_chat":
      return counts.orbit_chat;
    case "experience_item":
      return counts.memory;
    case "procedural_pattern":
      return counts.procedures;
    case "app":
      return counts.apps;
    case "goal":
      return counts.goals;
    case "watcher":
      return counts.watchers;
    case "sentinel":
      return counts.sentinel;
    case "arkpulse":
      return counts.arkpulse;
    case "arkevolve":
      return counts.arkevolve;
    case "llm_usage":
      return counts.usage;
    default:
      return 0;
  }
}

function totalForSourceCounts(counts: ReflectSourceCounts | undefined): number {
  if (!counts) return 0;
  return SOURCE_ORDER.reduce((sum, source) => sum + countForSourceCounts(counts, source), 0);
}

function meaningfulForSourceCounts(counts: ReflectSourceCounts | undefined): number {
  return Math.max(0, totalForSourceCounts(counts) - countForSourceCounts(counts, "llm_usage"));
}

function sourceMeta(source: string) {
  return SOURCE_DISPLAY[source] ?? { label: "Work", group: "Mixed work", color: "#8FA3BF" };
}

function dominantSource(cluster: ReflectCluster): string {
  const counts = new Map<string, number>();
  for (const unit of cluster.units) {
    counts.set(unit.source_kind, (counts.get(unit.source_kind) ?? 0) + 1);
  }
  return [...counts.entries()].sort((a, b) => b[1] - a[1])[0]?.[0] ?? "work";
}

function hexToHsl(hex: string): { h: number; s: number; l: number } | null {
  const m = hex.match(/^#([0-9a-f]{6})$/i);
  if (!m) return null;
  const n = parseInt(m[1], 16);
  const r = ((n >> 16) & 0xff) / 255;
  const g = ((n >> 8) & 0xff) / 255;
  const b = (n & 0xff) / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return { h: 0, s: 0, l };
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h = 0;
  if (max === r) h = ((g - b) / d + (g < b ? 6 : 0)) / 6;
  else if (max === g) h = ((b - r) / d + 2) / 6;
  else h = ((r - g) / d + 4) / 6;
  return { h, s, l };
}

function hslToHex(h: number, s: number, l: number): string {
  const hue2rgb = (p: number, q: number, t: number) => {
    let tt = t;
    if (tt < 0) tt += 1;
    if (tt > 1) tt -= 1;
    if (tt < 1 / 6) return p + (q - p) * 6 * tt;
    if (tt < 1 / 2) return q;
    if (tt < 2 / 3) return p + (q - p) * (2 / 3 - tt) * 6;
    return p;
  };
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  const r = Math.round(hue2rgb(p, q, h + 1 / 3) * 255);
  const g = Math.round(hue2rgb(p, q, h) * 255);
  const b = Math.round(hue2rgb(p, q, h - 1 / 3) * 255);
  const toHex = (x: number) => x.toString(16).padStart(2, "0");
  return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
}

function tacticalAccent(hex: string): string {
  const hsl = hexToHsl(hex);
  if (!hsl) return hex;
  return hslToHex(hsl.h, Math.min(0.7, hsl.s * 0.78), Math.min(0.78, hsl.l * 0.95 + 0.18));
}

function tacticalSymbol(source: string): string {
  const HEXAGON = "path://M50,3 L93,26 L93,74 L50,97 L7,74 L7,26 Z";
  const DIAMOND = "path://M50,3 L97,50 L50,97 L3,50 Z";
  const TRIANGLE = "path://M50,6 L94,88 L6,88 Z";
  const SQUARE = "path://M10,10 L90,10 L90,90 L10,90 Z";
  if (source === "conversation" || source === "orbit_chat") return HEXAGON;
  if (source === "watcher" || source === "sentinel" || source === "arkpulse") return DIAMOND;
  if (source === "experience_item" || source === "procedural_pattern") return TRIANGLE;
  if (source === "app" || source === "goal" || source === "arkevolve") return SQUARE;
  return HEXAGON;
}

function tacticalCode(source: string): string {
  const map: Record<string, string> = {
    conversation: "CHT",
    orbit_chat: "ORB",
    experience_item: "MEM",
    procedural_pattern: "PRC",
    app: "APP",
    goal: "GOL",
    watcher: "WCH",
    sentinel: "SNT",
    arkpulse: "PLS",
    arkevolve: "EVO",
    llm_usage: "USG",
  };
  return map[source] ?? "WRK";
}

function clusterDisplayLabel(cluster: ReflectCluster): string {
  const explicit = cluster.label?.trim();
  if (explicit) return explicit;
  const sourceKinds = new Set(cluster.units.map((unit) => unit.source_kind));
  if (sourceKinds.size === 1) return sourceMeta(dominantSource(cluster)).group;
  if (sourceKinds.has("conversation") || sourceKinds.has("orbit_chat")) return "Conversation-led work";
  if (sourceKinds.has("watcher") || sourceKinds.has("sentinel") || sourceKinds.has("arkpulse")) {
    return "Background operations";
  }
  return "Mixed AgentArk activity";
}

function clusterDistinguishingHint(cluster: ReflectCluster): string {
  const firstUnit = cluster.units[0];
  const title = firstUnit?.title?.trim() ?? "";
  if (title) {
    const words = title.split(/\s+/).slice(0, 4).join(" ");
    return words.length > 32 ? `${words.slice(0, 29)}...` : words;
  }
  return cluster.id.slice(0, 6);
}

function buildClusterLabelMap(clusters: ReflectCluster[]): Record<string, string> {
  const counts = new Map<string, number>();
  for (const cluster of clusters) {
    const primary = clusterDisplayLabel(cluster);
    counts.set(primary, (counts.get(primary) ?? 0) + 1);
  }
  const result: Record<string, string> = {};
  for (const cluster of clusters) {
    const primary = clusterDisplayLabel(cluster);
    const collision = (counts.get(primary) ?? 1) > 1;
    if (!collision) {
      result[cluster.id] = primary;
      continue;
    }
    const hint = clusterDistinguishingHint(cluster);
    result[cluster.id] = hint ? `${primary}: ${hint}` : primary;
  }
  return result;
}

function digestStatusTitle(response: ReflectResponse | undefined): string {
  const digest = response?.daily_digest_status;
  if (!digest || !digest.enabled) return "Daily digest is off";
  const appliesToToday = !digest.target_date || digest.target_date === digest.today_date;
  if (!appliesToToday) {
    const meaningful = meaningfulForSourceCounts(response?.source_counts);
    return meaningful > 0 ? "Today has activity to reflect" : "Waiting for today's activity";
  }
  if (digest.status === "sent") return "Daily digest sent";
  if (digest.status === "preparing") return "Preparing today's digest";
  if (digest.status === "skipped_quiet") return "No digest sent for a quiet day";
  if (digest.status === "delivery_failed") return "Digest delivery needs attention";
  const meaningful = meaningfulForSourceCounts(response?.source_counts);
  if (meaningful > 0) return "Today has activity to reflect";
  return "Waiting for meaningful activity";
}

function digestStatusDetail(response: ReflectResponse | undefined, fetching: boolean): string {
  if (!response) {
    return fetching
      ? "Loading today's ArkReflect status."
      : "Today status appears here after ArkReflect has cached activity.";
  }
  const digest = response.daily_digest_status;
  const total = totalForSourceCounts(response.source_counts);
  const meaningful = meaningfulForSourceCounts(response.source_counts);
  const appliesToToday = !digest.target_date || digest.target_date === digest.today_date;
  if (!digest.enabled) {
    return total > 0
      ? `${meaningful} meaningful signal${meaningful === 1 ? "" : "s"} cached today. Enable the daily digest in Settings to send recaps.`
      : "Enable the digest in Settings if you want meaningful days sent to your notification channel.";
  }
  if (!appliesToToday) {
    return total > 0
      ? `${meaningful} meaningful signal${meaningful === 1 ? "" : "s"} cached today; today's digest will wait for a quiet end-of-day window.`
      : "No meaningful activity has been cached for today yet.";
  }
  if (digest.status === "sent" && digest.last_sent_at) {
    return `Sent for ${digest.target_date || "the selected day"} at ${formatUiDateTime(digest.last_sent_at)}.`;
  }
  if (digest.status === "skipped_quiet") {
    return "ArkReflect checked the day and found nothing worth notifying you about.";
  }
  if (digest.status === "preparing") {
    return "AgentArk is refreshing the daily work units in the background.";
  }
  if (digest.status === "delivery_failed") {
    return digest.last_error || "The digest was prepared, but no notification channel accepted it.";
  }
  return total > 0
    ? `${meaningful} meaningful signal${meaningful === 1 ? "" : "s"} cached today; the digest waits for a quiet end-of-day window.`
    : "No meaningful activity has been cached for today yet.";
}

function clusterPlainSummary(cluster: ReflectCluster): string {
  const sources = [...new Set(cluster.units.map((unit) => sourceMeta(unit.source_kind).label))];
  const sourceText = sources.slice(0, 3).join(", ");
  return `${cluster.unit_count} item${cluster.unit_count === 1 ? "" : "s"} from ${sourceText || "AgentArk"}.`;
}

function quietStatus(
  response: ReflectResponse | undefined,
  fetching: boolean,
  refreshing: boolean,
): { title: string; detail: string; active: boolean } {
  if (!response) {
    return {
      title: fetching ? "Loading your recap" : "Recap is ready when activity is available",
      detail: "ArkReflect reads cached reflection data first, then updates quietly in the background.",
      active: fetching,
    };
  }
  if (refreshing || response.refresh_status.running) {
    return {
      title: response.cache_status.cached_units > 0 ? "Updating quietly" : "Preparing first recap",
      detail:
        response.cache_status.cached_units > 0
          ? "The current view stays usable while AgentArk refreshes the cached recap."
          : "AgentArk is gathering enough activity to build this view.",
      active: true,
    };
  }
  if (response.cache_status.mode === "empty") {
    return {
      title: "Still collecting data",
      detail:
        "ArkReflect does not have enough cached work units for this range yet. The recap will appear here once activity is available.",
      active: false,
    };
  }
  if (response.cache_status.mode === "stale") {
    return {
      title: "Showing the latest cached recap",
      detail: "Recent changes may appear after the next background refresh.",
      active: false,
    };
  }
  return {
    title: "Recap ready",
    detail: `${response.cache_status.cached_units} cached item${response.cache_status.cached_units === 1 ? "" : "s"} summarized for this range.`,
    active: false,
  };
}

export default function ArkReflectPage({ autoRefresh }: ArkReflectPageProps) {
  const queryClient = useQueryClient();
  const [period, setPeriod] = useState<ReflectPeriod>("weekly");
  const [anchor, setAnchor] = useState(() => toDateInputValue(new Date()));
  const [storyTab, setStoryTab] = useState<ReflectStoryTab>("studio");
  const bounds = useMemo(() => periodBounds(period, anchor), [period, anchor]);
  const fromIso = bounds.from.toISOString();
  const toIso = bounds.to.toISOString();
  const todayBounds = useMemo(
    () => periodBounds("daily", toDateInputValue(new Date())),
    [],
  );
  const todayFromIso = todayBounds.from.toISOString();
  const todayToIso = todayBounds.to.toISOString();
  const reflectQueryKey = useMemo(
    () => ["arkreflect", period, fromIso, toIso] as const,
    [period, fromIso, toIso],
  );
  const todayQueryKey = useMemo(
    () => ["arkreflect", "today", todayFromIso, todayToIso] as const,
    [todayFromIso, todayToIso],
  );

  const reflectQ = useQuery({
    queryKey: reflectQueryKey,
    queryFn: async () => {
      const raw = await api.rawGet(
        `/reflect?period=${encodeURIComponent(period)}&from=${encodeURIComponent(fromIso)}&to=${encodeURIComponent(toIso)}`,
      );
      return parseReflectResponse(raw, period);
    },
    refetchInterval: autoRefresh ? 120000 : false,
  });

  const refreshMutation = useMutation({
    mutationFn: async () => {
      await api.rawPost(
        `/reflect/refresh?period=${encodeURIComponent(period)}&from=${encodeURIComponent(fromIso)}&to=${encodeURIComponent(toIso)}`,
      );
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: reflectQueryKey });
      void queryClient.invalidateQueries({ queryKey: todayQueryKey });
    },
  });

  const response = reflectQ.data;
  const todayQ = useQuery({
    queryKey: todayQueryKey,
    queryFn: async () => {
      const raw = await api.rawGet(
        `/reflect?period=daily&from=${encodeURIComponent(todayFromIso)}&to=${encodeURIComponent(todayToIso)}`,
      );
      return parseReflectResponse(raw, "daily");
    },
    refetchInterval: autoRefresh ? 120000 : false,
  });
  const todayResponse = todayQ.data;

  useEffect(() => {
    if (!response?.refresh_status.running && !refreshMutation.isPending) return undefined;
    const id = window.setInterval(() => {
      void queryClient.invalidateQueries({ queryKey: reflectQueryKey });
    }, 5000);
    return () => window.clearInterval(id);
  }, [queryClient, reflectQueryKey, refreshMutation.isPending, response?.refresh_status.running]);

  const clusters = response?.clusters ?? [];
  const clusterLabelById = useMemo(() => buildClusterLabelMap(clusters), [clusters]);
  const allUnits = useMemo(() => {
    const byId = new Map<string, ReflectUnit>();
    for (const cluster of clusters) {
      for (const unit of cluster.units) byId.set(unit.id, unit);
    }
    for (const unit of response?.unclustered_units ?? []) byId.set(unit.id, unit);
    return [...byId.values()];
  }, [clusters, response?.unclustered_units]);
  const suggestedFollowups = response?.suggested_followups ?? [];

  useEffect(() => {
    const waitingForDailyLatest = suggestedFollowups.some(
      (item) => item.kind === "latest_developments" && item.status === "queued",
    );
    if (!waitingForDailyLatest) return undefined;
    const id = window.setInterval(() => {
      void queryClient.invalidateQueries({ queryKey: reflectQueryKey });
    }, 30000);
    return () => window.clearInterval(id);
  }, [queryClient, reflectQueryKey, suggestedFollowups]);

  const totalUnits = allUnits.length;
  const strongestCluster = clusters[0] ?? null;
  const embeddingCoverage =
    response && response.embedding_status.total_units > 0
      ? response.embedding_status.embedded_units / response.embedding_status.total_units
      : 0;

  const rangeLabel = formatUiDateRange(response?.from || fromIso, response?.to || toIso);
  const status = quietStatus(response, reflectQ.isFetching, refreshMutation.isPending);
  const todayDigestTitle = digestStatusTitle(todayResponse);
  const todayDigestDetail = digestStatusDetail(todayResponse, todayQ.isFetching);
  const todayMeaningful = meaningfulForSourceCounts(todayResponse?.source_counts);
  const todayTotal = totalForSourceCounts(todayResponse?.source_counts);
  const focusLabel = strongestCluster
    ? (clusterLabelById[strongestCluster.id] ?? clusterDisplayLabel(strongestCluster))
    : "No activity yet";
  const recurringCount = clusters.filter((cluster) => cluster.related_history.mode === "recurring").length;
  const sourceRows = useMemo(
    () =>
      SOURCE_ORDER.map((source) => ({
        source,
        ...sourceMeta(source),
        count: countForSource(response, source),
      })).filter((item) => item.count > 0),
    [response],
  );
  const backgroundCount =
    countForSource(response, "app") +
    countForSource(response, "goal") +
    countForSource(response, "watcher") +
    countForSource(response, "sentinel") +
    countForSource(response, "arkpulse") +
    countForSource(response, "arkevolve");
  const learnedCount =
    countForSource(response, "experience_item") + countForSource(response, "procedural_pattern");
  const styleSignals = useMemo(() => workingStyleSignals(response), [response]);
  const narrative = useMemo(
    () => narrativeLines(response, focusLabel, totalUnits, learnedCount, backgroundCount, recurringCount),
    [backgroundCount, focusLabel, learnedCount, recurringCount, response, totalUnits],
  );
  const hasReflectContent = clusters.length > 0;
  const selectedRangeLabel = rangeLabel || formatUiDateRange(fromIso, toIso);
  const sourceSignalCount = totalForSourceCounts(response?.source_counts);
  const emptyStateDetail = response
    ? totalUnits > 0
      ? `ArkReflect has ${totalUnits} reflected work unit${totalUnits === 1 ? "" : "s"} for this range and is still grouping them into focus areas.`
      : sourceSignalCount > 0
      ? `ArkReflect has ${sourceSignalCount} source signal${sourceSignalCount === 1 ? "" : "s"} in this range and is preparing the reflected work units for the recap.`
      : "No reflected work units are cached for this range yet. Keep working normally; this panel will turn into the recap after chat, ArkOrbit, apps, goals, watchers, or background systems produce activity."
    : status.detail;
  const emptyStateChip =
    reflectQ.isFetching || refreshMutation.isPending || Boolean(response?.refresh_status.running)
      ? "Collecting"
      : "Waiting for activity";

  const constellationOption = useMemo(() => {
    const nodes: Array<Record<string, unknown>> = [];
    const links: Array<Record<string, unknown>> = [];
    const seen = new Set<string>();
    const clusterNodeIds: string[] = [];
    clusters.forEach((cluster, index) => {
      const source = dominantSource(cluster);
      const meta = sourceMeta(source);
      const clusterName = clusterLabelById[cluster.id] ?? clusterDisplayLabel(cluster);
      const nodeId = `cluster-${cluster.id}`;
      seen.add(nodeId);
      clusterNodeIds.push(nodeId);
      const nodeSize = Math.max(14, Math.min(28, 12 + cluster.unit_count * 3));
      const stroke = tacticalAccent(meta.color);
      const code = tacticalCode(source);
      const idx = String(index + 1).padStart(2, "0");
      const truncated = clusterName.length > 38 ? `${clusterName.slice(0, 36)}…` : clusterName;
      nodes.push({
        id: nodeId,
        name: clusterName,
        value: cluster.unit_count,
        symbol: tacticalSymbol(source),
        symbolSize: nodeSize,
        category: 0,
        itemStyle: {
          color: "rgba(0,0,0,0)",
          borderColor: stroke,
          borderWidth: 1,
          shadowBlur: 6,
          shadowColor: stroke,
        },
        label: {
          show: true,
          position: "right",
          distance: 8,
          formatter: `{code|${idx}·${code}}  {name|${truncated.toUpperCase()}}`,
          rich: {
            code: {
              color: stroke,
              fontSize: 8.5,
              fontFamily: "'JetBrains Mono', 'IBM Plex Mono', Menlo, monospace",
              fontWeight: 500,
              letterSpacing: 1,
              backgroundColor: "rgba(0,0,0,0.35)",
              padding: [2, 4, 2, 4],
              borderRadius: 1,
            },
            name: {
              color: "rgba(210, 226, 238, 0.78)",
              fontSize: 9.5,
              fontFamily: "'JetBrains Mono', 'IBM Plex Mono', Menlo, monospace",
              fontWeight: 400,
              letterSpacing: 0.6,
            },
          },
        },
        emphasis: {
          scale: 1.4,
          itemStyle: {
            borderColor: stroke,
            borderWidth: 1.4,
            shadowBlur: 14,
            shadowColor: stroke,
          },
          label: {
            rich: {
              name: { color: "#f4fbff" },
              code: { color: stroke },
            },
          },
        },
        x: Math.cos((index / Math.max(clusters.length, 1)) * Math.PI * 2 - Math.PI / 2) * 240,
        y: Math.sin((index / Math.max(clusters.length, 1)) * Math.PI * 2 - Math.PI / 2) * 150,
      });
      const angle = (index / Math.max(clusters.length, 1)) * Math.PI * 2 - Math.PI / 2;
      cluster.related_history.items.slice(0, 2).forEach((item, itemIndex) => {
        const historyId = `history-${item.id}`;
        const satOffset = 36 + itemIndex * 18;
        const satAngle = angle + (itemIndex === 0 ? -0.22 : 0.22);
        if (!seen.has(historyId)) {
          seen.add(historyId);
          nodes.push({
            id: historyId,
            name: unitDisplayTitle({
              id: item.id,
              source_kind: "history",
              source_label: item.source_label,
              channel: "",
              title: item.title,
              summary: "",
              content_preview: "",
              occurred_at: item.occurred_at,
              message_count: 1,
              has_embedding: true,
            }),
            value: 1,
            symbol: "path://M50,8 L92,50 L50,92 L8,50 Z",
            symbolSize: 6,
            category: 1,
            itemStyle: {
              color: "rgba(0,0,0,0)",
              borderColor: "rgba(170, 200, 220, 0.4)",
              borderWidth: 0.8,
            },
            label: { show: false },
            x: Math.cos(satAngle) * (240 + satOffset),
            y: Math.sin(satAngle) * (150 + satOffset * 0.6),
          });
        }
        links.push({
          source: nodeId,
          target: historyId,
          value: item.similarity,
          lineStyle: {
            width: 0.8 + item.similarity * 1.4,
            color: stroke,
            opacity: 0.42,
            curveness: 0.14 + itemIndex * 0.06,
            type: "solid",
          },
        });
      });
    });
    if (links.length === 0 && clusterNodeIds.length >= 2) {
      for (let i = 0; i < clusterNodeIds.length; i += 1) {
        for (let j = i + 1; j < clusterNodeIds.length; j += 1) {
          links.push({
            source: clusterNodeIds[i],
            target: clusterNodeIds[j],
            lineStyle: {
              width: 0.6,
              color: "rgba(140, 200, 220, 0.16)",
              curveness: 0.18,
              type: [3, 5],
              dashOffset: 0,
            },
          });
        }
      }
    }
    return {
      backgroundColor: "transparent",
      tooltip: {
        backgroundColor: "rgba(6, 11, 16, 0.96)",
        borderColor: "rgba(120, 200, 220, 0.4)",
        borderWidth: 1,
        padding: [8, 12],
        textStyle: {
          color: "#dceaf2",
          fontSize: 11.5,
          fontFamily: "'JetBrains Mono', 'IBM Plex Mono', Menlo, monospace",
        },
        formatter: (info: { data?: { name?: string; value?: number } }) => {
          const name = (info.data?.name || "node").toUpperCase();
          const v = info.data?.value;
          return v
            ? `<span style="opacity:0.6">› TRACE</span> ${name}<br/><span style="opacity:0.6">› UNITS</span> ${v}`
            : `<span style="opacity:0.6">› NODE</span> ${name}`;
        },
      },
      graphic: {
        elements: [
          {
            type: "group",
            left: "center",
            top: "middle",
            children: [
              { type: "circle", shape: { cx: 0, cy: 0, r: 3 }, style: { fill: "transparent", stroke: "rgba(120,200,220,0.55)", lineWidth: 1 } },
              { type: "circle", shape: { cx: 0, cy: 0, r: 1 }, style: { fill: "rgba(120,200,220,0.7)" } },
              { type: "line", shape: { x1: -10, y1: 0, x2: -5, y2: 0 }, style: { stroke: "rgba(120,200,220,0.45)", lineWidth: 1 } },
              { type: "line", shape: { x1: 5, y1: 0, x2: 10, y2: 0 }, style: { stroke: "rgba(120,200,220,0.45)", lineWidth: 1 } },
              { type: "line", shape: { x1: 0, y1: -10, x2: 0, y2: -5 }, style: { stroke: "rgba(120,200,220,0.45)", lineWidth: 1 } },
              { type: "line", shape: { x1: 0, y1: 5, x2: 0, y2: 10 }, style: { stroke: "rgba(120,200,220,0.45)", lineWidth: 1 } },
            ],
          },
          {
            type: "text",
            left: 14,
            top: 12,
            style: {
              text: `◢ PANORAMA · ${clusters.length.toString().padStart(2, "0")} TRACES`,
              fill: "rgba(120, 200, 220, 0.55)",
              font: "500 9.5px 'JetBrains Mono', 'IBM Plex Mono', Menlo, monospace",
            },
          },
          {
            type: "text",
            right: 14,
            bottom: 12,
            style: {
              text: "◣ FOCUS·MAP",
              fill: "rgba(120, 200, 220, 0.45)",
              font: "500 9.5px 'JetBrains Mono', 'IBM Plex Mono', Menlo, monospace",
              textAlign: "right",
            },
          },
        ],
      },
      animationDurationUpdate: 900,
      animationEasingUpdate: "cubicInOut",
      series: [
        {
          type: "graph",
          layout: "none",
          roam: false,
          draggable: false,
          categories: [{ name: "Active" }, { name: "Bridge" }],
          data: nodes,
          links,
          edgeSymbol: ["none", "none"],
          lineStyle: { opacity: 0.4, curveness: 0.08 },
          zlevel: 2,
        },
      ],
    };
  }, [clusters, clusterLabelById]);

  const activityOption = useMemo(() => {
    const TIMELINE_BUCKETS = period === "daily" ? 24 : period === "weekly" ? 28 : 36;
    const fromTs = response?.from ? Date.parse(response.from) : NaN;
    const toTs = response?.to ? Date.parse(response.to) : NaN;
    const haveBounds = Number.isFinite(fromTs) && Number.isFinite(toTs) && toTs > fromTs;
    const span = haveBounds ? toTs - fromTs : 1;
    const buckets = new Array(TIMELINE_BUCKETS).fill(0);
    for (const unit of allUnits) {
      const ts = Date.parse(unit.occurred_at);
      if (!Number.isFinite(ts)) continue;
      if (!haveBounds) continue;
      const ratio = (ts - fromTs) / span;
      const idx = Math.min(TIMELINE_BUCKETS - 1, Math.max(0, Math.floor(ratio * TIMELINE_BUCKETS)));
      buckets[idx] += 1;
    }
    const peak = Math.max(1, ...buckets);
    const startLabel = haveBounds
      ? formatUiDateOnly(new Date(fromTs).toISOString(), { fallback: "start" })
      : "start";
    const endLabel = haveBounds
      ? formatUiDateOnly(new Date(toTs).toISOString(), { fallback: "now" })
      : "now";
    const data = buckets.map((count) => ({
      value: count,
      itemStyle: {
        color: count === 0 ? "rgba(120, 200, 220, 0.10)" : "rgba(120, 200, 220, 0.78)",
        borderColor: count === peak ? "rgba(180, 230, 250, 0.95)" : "transparent",
        borderWidth: count === peak ? 0.6 : 0,
      },
    }));
    return {
      backgroundColor: "transparent",
      tooltip: {
        trigger: "axis",
        backgroundColor: "rgba(6, 11, 16, 0.96)",
        borderColor: "rgba(120, 200, 220, 0.4)",
        borderWidth: 1,
        padding: [6, 10],
        textStyle: {
          color: "#dceaf2",
          fontSize: 11,
          fontFamily: "'JetBrains Mono', 'IBM Plex Mono', Menlo, monospace",
        },
        axisPointer: { type: "shadow", shadowStyle: { color: "rgba(120, 200, 220, 0.06)" } },
        formatter: (params: Array<{ dataIndex: number; value: number }>) => {
          const p = params?.[0];
          if (!p) return "";
          const i = p.dataIndex;
          const tBucket = haveBounds ? new Date(fromTs + ((i + 0.5) / TIMELINE_BUCKETS) * span) : null;
          const stamp = tBucket ? tBucket.toISOString().slice(0, 16).replace("T", " ") : `BIN ${i + 1}`;
          return `<span style="opacity:0.55">› T</span> ${stamp}<br/><span style="opacity:0.55">› N</span> ${p.value}`;
        },
      },
      grid: { left: 28, right: 12, top: 14, bottom: 22, containLabel: false },
      xAxis: {
        type: "category",
        data: buckets.map((_, i) => i),
        boundaryGap: true,
        axisTick: { show: false },
        axisLine: { lineStyle: { color: "rgba(120, 200, 220, 0.18)" } },
        axisLabel: {
          color: "rgba(180, 210, 225, 0.5)",
          fontSize: 9,
          fontFamily: "'JetBrains Mono', 'IBM Plex Mono', Menlo, monospace",
          letterSpacing: 0.6,
          interval: TIMELINE_BUCKETS - 2,
          formatter: (val: string) => {
            const i = Number(val);
            if (i === 0) return startLabel.toUpperCase();
            if (i === TIMELINE_BUCKETS - 1) return endLabel.toUpperCase();
            return "";
          },
          align: (val: string) => (Number(val) === 0 ? "left" : "right"),
        },
      },
      yAxis: {
        type: "value",
        min: 0,
        max: peak,
        interval: peak,
        axisTick: { show: false },
        axisLine: { show: false },
        axisLabel: {
          color: "rgba(180, 210, 225, 0.45)",
          fontSize: 9,
          fontFamily: "'JetBrains Mono', 'IBM Plex Mono', Menlo, monospace",
          showMinLabel: true,
          showMaxLabel: true,
          formatter: (val: number) => String(val),
        },
        splitLine: { show: false },
      },
      series: [
        {
          type: "bar",
          data,
          barWidth: 2,
          barCategoryGap: "60%",
          silent: false,
          animationDuration: 600,
          animationEasing: "cubicOut",
        },
      ],
    };
  }, [allUnits, period, response?.from, response?.to]);

  const sortedClusters = useMemo(
    () => [...clusters].sort((left, right) => right.unit_count - left.unit_count),
    [clusters],
  );
  const topClusters = sortedClusters.slice(0, 5);
  const leadCluster = sortedClusters[0] ?? null;
  const mostChangedStyle = styleSignals
    .slice()
    .sort((left, right) => Math.abs(right.delta) - Math.abs(left.delta))[0];
  const replayUnits = useMemo(
    () =>
      allUnits
        .filter((unit) => Number.isFinite(Date.parse(unit.occurred_at)))
        .sort((left, right) => Date.parse(left.occurred_at) - Date.parse(right.occurred_at))
        .slice(0, 6),
    [allUnits],
  );
  const showWeeklyReplay = replayUnits.length >= 3;
  const recoveryFollowups = suggestedFollowups.filter((item) => item.kind === "recovery_advice");
  const latestFollowups = suggestedFollowups.filter((item) => item.kind === "latest_developments");
  const hasProblems =
    recoveryFollowups.length > 0 ||
    Boolean(response?.refresh_status.last_error) ||
    (Boolean(response) && response?.embedding_status.mode !== "semantic" && totalUnits > 0);
  const hasTodayStatus =
    todayQ.isFetching ||
    todayTotal > 0 ||
    todayMeaningful > 0 ||
    Boolean(todayResponse?.daily_digest_status.enabled) ||
    Boolean(todayResponse?.daily_digest_status.summary);
  const hasGroupingStatus =
    (response?.embedding_status.total_units ?? 0) > 0 ||
    Boolean(response?.embedding_status.detail);
  const hasStudioSide = hasTodayStatus || suggestedFollowups.length > 0 || hasGroupingStatus;
  const whatWentWrong =
    response?.refresh_status.last_error ||
    recoveryFollowups[0]?.detail ||
    (response?.embedding_status.mode !== "semantic" && totalUnits > 0
      ? "Semantic grouping is still catching up, so some patterns may be grouped by source activity first."
      : "No major failure stood out in the reflected data. The main risk is letting the next step remain implicit.");
  const achievementCards = [
    {
      label: "What you achieved",
      value: `${clusters.length}`,
      detail: `focus area${clusters.length === 1 ? "" : "s"} clarified from ${totalUnits} reflected item${totalUnits === 1 ? "" : "s"}.`,
      tone: "var(--green)",
    },
    {
      label: "What went well",
      value: `${Math.max(0, totalUnits - recoveryFollowups.length)}`,
      detail: "signals moved cleanly enough to become a readable recap.",
      tone: "var(--cyan)",
    },
    ...(hasProblems
      ? [
          {
            label: "What went wrong",
            value: `${recoveryFollowups.length}`,
            detail:
              recoveryFollowups.length > 0
                ? "recovery follow-up surfaced from the period."
                : "a system caveat needs attention.",
            tone: "var(--red)",
          },
        ]
      : []),
    ...(recurringCount > 0
      ? [
          {
            label: "Observations",
            value: `${recurringCount}`,
            detail: `recurring theme${recurringCount === 1 ? "" : "s"} connected to earlier work.`,
            tone: "var(--orange)",
          },
        ]
      : []),
  ];
  const dreamCards = [
    {
      title: leadCluster
        ? `Carry ${clusterLabelById[leadCluster.id] ?? clusterDisplayLabel(leadCluster)} forward`
        : "Let the next meaningful cluster emerge",
      detail: leadCluster
        ? leadCluster.plain_summary || clusterPlainSummary(leadCluster)
        : "ArkReflect will turn the next real activity into a focused story once enough work is cached.",
    },
    ...(latestFollowups[0]
      ? [
          {
            title: latestFollowups[0].title,
            detail: latestFollowups[0].detail,
          },
        ]
      : []),
    ...(recurringCount > 0 && topClusters[0]
      ? [
          {
            title: "Watch the recurring thread",
            detail: relatedHistoryText(topClusters[0].related_history),
          },
        ]
      : []),
  ].filter((card) => card.title.trim() && card.detail.trim());
  const storyTabs = [
    { value: "studio" as const, label: "Reflection Studio", short: "Studio", count: totalUnits },
    ...(topClusters.length > 0
      ? [{ value: "patterns" as const, label: "Pattern Observatory", short: "Patterns", count: topClusters.length }]
      : []),
    ...(achievementCards.length > 0
      ? [{ value: "achievements" as const, label: "Achievement Canvas", short: "Achievements", count: achievementCards.length }]
      : []),
    ...(dreamCards.length > 0
      ? [{ value: "dreams" as const, label: "Dream Board", short: "Dreams", count: dreamCards.length }]
      : []),
    ...(showWeeklyReplay
      ? [{ value: "replay" as const, label: "Weekly Replay", short: "Replay", count: replayUnits.length }]
      : []),
  ];

  useEffect(() => {
    if (storyTabs.some((tab) => tab.value === storyTab)) return;
    setStoryTab("studio");
  }, [storyTab, storyTabs]);

  const renderStoryView = () => {
    const panelSx = {
      border: "1px solid var(--surface-border)",
      borderRadius: "8px",
      background:
        "radial-gradient(circle at top left, var(--ui-rgba-0-255-170-040), transparent 38%), linear-gradient(180deg, var(--cyber-panel-raised), var(--cyber-panel))",
      boxShadow: "var(--surface-shadow-soft)",
    };
    const labelSx = {
      fontFamily: "var(--font-mono)",
      fontSize: "0.68rem",
      letterSpacing: "0.14em",
      textTransform: "uppercase",
      color: "var(--text-dim)",
      lineHeight: 1.35,
    };
    const titleSx = {
      fontFamily: "var(--font-display)",
      fontWeight: 750,
      letterSpacing: 0,
      lineHeight: 1.18,
    };
    const bodySx = {
      color: "var(--text-secondary)",
      lineHeight: 1.55,
      fontSize: "0.9rem",
    };
    const periodName = period === "daily" ? "day" : period === "weekly" ? "week" : "month";
    const focusTitle =
      focusLabel === "No activity yet"
        ? "ArkReflect is waiting for a clear focus."
        : `This ${periodName} centered on ${focusLabel.toLowerCase()}.`;

    return (
      <Stack spacing={1.4}>
        <Box
          sx={{
            ...panelSx,
            p: { xs: 1.5, md: 2 },
            background:
              "linear-gradient(90deg, var(--ui-rgba-0-255-170-060), transparent 68%), linear-gradient(180deg, var(--cyber-panel-raised), var(--cyber-panel))",
          }}
        >
          <Stack
            direction={{ xs: "column", md: "row" }}
            spacing={1.4}
            sx={{ alignItems: { xs: "flex-start", md: "center" }, justifyContent: "space-between" }}
          >
            <Box sx={{ minWidth: 0 }}>
              <Typography sx={labelSx}>Reflection Studio</Typography>
              <Typography sx={{ ...titleSx, fontSize: { xs: "1.45rem", md: "2rem" }, mt: 0.45 }}>
                {focusTitle}
              </Typography>
              <Typography sx={{ ...bodySx, mt: 0.7, maxWidth: 880 }}>
                {narrative[0] || "ArkReflect will summarize the period once enough activity is available."}
              </Typography>
            </Box>
            <Stack direction="row" spacing={0.75} sx={{ flexWrap: "wrap", rowGap: 0.75 }}>
              <Chip className="arkreflect-pill" icon={<WorkHistoryRoundedIcon />} label={`${totalUnits} reflected`} />
              <Chip className="arkreflect-pill" icon={<BubbleChartRoundedIcon />} label={`${clusters.length} focus areas`} />
              <Chip className="arkreflect-pill" icon={<RefreshRoundedIcon />} label={status.title} />
            </Stack>
          </Stack>
        </Box>

        <Box
          sx={{
            ...panelSx,
            p: 0.75,
            display: "flex",
            gap: 0.75,
            flexWrap: "wrap",
            alignItems: "center",
          }}
        >
          {storyTabs.map((tab) => {
            const active = storyTab === tab.value;
            return (
              <Button
                key={tab.value}
                variant={active ? "contained" : "outlined"}
                onClick={() => setStoryTab(tab.value)}
                sx={{
                  minHeight: 34,
                  borderRadius: "8px",
                  color: active ? "#06100d" : "var(--button-text)",
                  bgcolor: active ? "var(--green)" : "transparent",
                  borderColor: active ? "var(--green)" : "var(--surface-border)",
                  "&:hover": {
                    bgcolor: active ? "var(--green)" : "var(--ui-rgba-0-255-170-060)",
                    borderColor: "var(--surface-border-strong)",
                  },
                }}
              >
                {tab.short}
                <Box component="span" sx={{ ml: 0.75, opacity: 0.72, fontFamily: "var(--font-mono)" }}>
                  {tab.count}
                </Box>
              </Button>
            );
          })}
        </Box>

        {storyTab === "studio" ? (
        <Grid2 container spacing={1.4}>
          {sourceRows.length > 0 ? (
          <Grid2 size={{ xs: 12, lg: 3 }}>
            <Box sx={{ ...panelSx, p: 1.35, height: "100%" }}>
              <Typography sx={labelSx}>Activity mix</Typography>
              <Stack spacing={0.9} sx={{ mt: 1 }}>
                {sourceRows.slice(0, 5).map((source) => {
                  const pct = totalUnits > 0 ? Math.round((source.count / totalUnits) * 100) : 0;
                  return (
                    <Box key={source.source}>
                      <Stack direction="row" sx={{ justifyContent: "space-between", mb: 0.45 }}>
                        <Typography variant="caption">{source.label}</Typography>
                        <Typography variant="caption">{source.count}</Typography>
                      </Stack>
                      <Box sx={{ height: 5, borderRadius: 999, bgcolor: "var(--ui-rgba-255-255-255-040)", overflow: "hidden" }}>
                        <Box sx={{ height: "100%", width: `${Math.max(6, pct)}%`, bgcolor: tacticalAccent(source.color) }} />
                      </Box>
                    </Box>
                  );
                })}
              </Stack>
            </Box>
          </Grid2>
          ) : null}

          <Grid2
            size={{
              xs: 12,
              lg: sourceRows.length > 0 && hasStudioSide ? 6 : sourceRows.length > 0 || hasStudioSide ? 9 : 12,
            }}
          >
            <Box sx={{ ...panelSx, p: { xs: 1.4, md: 1.8 }, minHeight: 430 }}>
              <Stack direction={{ xs: "column", md: "row" }} spacing={1.2} sx={{ justifyContent: "space-between", mb: 1.5 }}>
                <Box>
                  <Typography sx={labelSx}>What we did</Typography>
                  <Typography sx={{ ...titleSx, fontSize: { xs: "1.25rem", md: "1.55rem" }, mt: 0.45 }}>
                    {focusLabel === "No activity yet" ? "Waiting for a useful story." : focusLabel}
                  </Typography>
                </Box>
                <Typography variant="caption" sx={{ alignSelf: { md: "end" } }}>
                  {rangeLabel}
                </Typography>
              </Stack>
              <Grid2 container spacing={1}>
                {[
                  {
                    label: "What you achieved",
                    text: `You moved ${clusters.length} focus area${clusters.length === 1 ? "" : "s"} from raw activity into a readable recap.`,
                  },
                  {
                    label: "What went good",
                    text:
                      mostChangedStyle && Math.abs(mostChangedStyle.delta) > 0.08
                        ? `${mostChangedStyle.label} stood out compared with your baseline.`
                        : "The activity stayed balanced enough to summarize without one source overwhelming the range.",
                  },
                  ...(hasProblems ? [{ label: "What went wrong", text: whatWentWrong }] : []),
                  ...(narrative[1] ? [{ label: "Observation", text: narrative[1] }] : []),
                  ...(dreamCards[0] ? [{ label: "Dream", text: dreamCards[0].detail }] : []),
                  {
                    label: "Evidence",
                    text: leadCluster
                      ? `${leadCluster.unit_count} item${leadCluster.unit_count === 1 ? "" : "s"} support the leading focus.`
                      : "Evidence appears here once a focus area is available.",
                  },
                ].map((item) => (
                  <Grid2 key={item.label} size={{ xs: 12, sm: 6 }}>
                    <Box
                      sx={{
                        p: 1.2,
                        minHeight: 126,
                        border: "1px solid var(--surface-border)",
                        borderRadius: "8px",
                        background: "var(--ui-rgba-255-255-255-020)",
                      }}
                    >
                      <Typography sx={labelSx}>{item.label}</Typography>
                      <Typography sx={{ ...bodySx, mt: 0.75 }}>{item.text}</Typography>
                    </Box>
                  </Grid2>
                ))}
              </Grid2>
            </Box>
          </Grid2>

          {hasStudioSide ? (
          <Grid2 size={{ xs: 12, lg: 3 }}>
            <Stack spacing={1.4}>
              {hasTodayStatus ? (
                <Box sx={{ ...panelSx, p: 1.35 }}>
                  <Typography sx={labelSx}>Today status</Typography>
                  <Typography sx={{ ...titleSx, fontSize: "1rem", mt: 0.55 }}>{todayDigestTitle}</Typography>
                  <Typography sx={{ ...bodySx, mt: 0.65 }}>{todayDigestDetail}</Typography>
                  <Stack direction="row" spacing={0.7} sx={{ flexWrap: "wrap", rowGap: 0.7, mt: 1 }}>
                    {todayTotal > 0 ? <Chip size="small" label={`${todayTotal} cached`} variant="outlined" /> : null}
                    {todayMeaningful > 0 ? <Chip size="small" label={`${todayMeaningful} meaningful`} variant="outlined" /> : null}
                  </Stack>
                </Box>
              ) : null}
              {suggestedFollowups.length > 0 ? (
                <Box sx={{ ...panelSx, p: 1.35 }}>
                  <Typography sx={labelSx}>Follow-ups</Typography>
                  <Typography sx={{ ...titleSx, fontSize: "1.35rem", mt: 0.55 }}>
                    {suggestedFollowups.length}
                  </Typography>
                  <Typography sx={{ ...bodySx, mt: 0.65 }}>
                    {suggestedFollowups[0].title}
                  </Typography>
                </Box>
              ) : null}
              {hasGroupingStatus ? (
                <Box sx={{ ...panelSx, p: 1.35 }}>
                  <Typography sx={labelSx}>Grouping</Typography>
                  <Typography sx={{ ...titleSx, fontSize: "1.35rem", mt: 0.55 }}>
                    {Math.round(embeddingCoverage * 100)}%
                  </Typography>
                  {response?.embedding_status.detail ? (
                    <Typography sx={{ ...bodySx, mt: 0.65 }}>{response.embedding_status.detail}</Typography>
                  ) : null}
                </Box>
              ) : null}
            </Stack>
          </Grid2>
          ) : null}
        </Grid2>
        ) : null}

        {storyTab === "patterns" ? (
        <Grid2 container spacing={1.4}>
          <Grid2 size={{ xs: 12, lg: 7 }}>
            <Box className="arkreflect-panorama" sx={{ ...panelSx, p: 1.35, minHeight: 430 }}>
              <Stack direction={{ xs: "column", sm: "row" }} spacing={1} sx={{ justifyContent: "space-between", mb: 1 }}>
                <Box>
                  <Typography sx={labelSx}>Pattern Observatory</Typography>
                  <Typography sx={{ ...titleSx, fontSize: "1.2rem", mt: 0.35 }}>
                    Themes and their evidence links
                  </Typography>
                </Box>
                <Chip className="arkreflect-pill" icon={<BubbleChartRoundedIcon />} label={`${clusters.length} patterns`} />
              </Stack>
              <ReactECharts option={constellationOption} style={{ height: 345, width: "100%" }} />
            </Box>
          </Grid2>
          <Grid2 size={{ xs: 12, lg: 5 }}>
            <Box sx={{ ...panelSx, p: 1.35, minHeight: 430 }}>
              <Typography sx={labelSx}>Observed patterns</Typography>
              <Stack spacing={0.9} sx={{ mt: 1 }}>
                {topClusters.map((cluster, index) => {
                  const name = clusterLabelById[cluster.id] ?? clusterDisplayLabel(cluster);
                  const source = sourceMeta(dominantSource(cluster));
                  return (
                    <Box
                      key={cluster.id}
                      sx={{
                        display: "grid",
                        gridTemplateColumns: "34px 1fr auto",
                        gap: 1,
                        alignItems: "center",
                        p: 1,
                        border: "1px solid var(--surface-border)",
                        borderRadius: "8px",
                        background: "var(--ui-rgba-255-255-255-020)",
                      }}
                    >
                      <Box sx={{ color: tacticalAccent(source.color) }}>{sourceIcon(source.label)}</Box>
                      <Box sx={{ minWidth: 0 }}>
                        <Typography sx={{ fontWeight: 800, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                          {name}
                        </Typography>
                        <Typography variant="caption">{relatedHistoryText(cluster.related_history)}</Typography>
                      </Box>
                      <Typography sx={{ fontFamily: "var(--font-mono)", color: tacticalAccent(source.color), fontWeight: 800 }}>
                        {String(index + 1).padStart(2, "0")}
                      </Typography>
                    </Box>
                  );
                })}
              </Stack>
            </Box>
          </Grid2>
        </Grid2>
        ) : null}

        {storyTab === "achievements" ? (
        <Box sx={{ ...panelSx, p: 1.35 }}>
          <Stack direction={{ xs: "column", sm: "row" }} spacing={1} sx={{ justifyContent: "space-between", mb: 1 }}>
            <Box>
              <Typography sx={labelSx}>Achievement Canvas</Typography>
              <Typography sx={{ ...titleSx, fontSize: "1.2rem", mt: 0.35 }}>
                Wins, misses, and momentum
              </Typography>
            </Box>
            <Chip className="arkreflect-pill" icon={<AutoGraphRoundedIcon />} label={`${learnedCount} learned signals`} />
          </Stack>
          <Grid2 container spacing={1}>
            {achievementCards.map((card) => (
              <Grid2 key={card.label} size={{ xs: 12, sm: 6, lg: 3 }}>
                <Box
                  sx={{
                    p: 1.25,
                    minHeight: 138,
                    border: "1px solid var(--surface-border)",
                    borderRadius: "8px",
                    background: "var(--ui-rgba-255-255-255-020)",
                  }}
                >
                  <Typography sx={labelSx}>{card.label}</Typography>
                  <Typography sx={{ fontFamily: "var(--font-mono)", fontSize: "2rem", fontWeight: 850, color: card.tone, mt: 0.8 }}>
                    {card.value}
                  </Typography>
                  <Typography sx={{ ...bodySx, mt: 0.5 }}>{card.detail}</Typography>
                </Box>
              </Grid2>
            ))}
          </Grid2>
        </Box>
        ) : null}

        {storyTab === "dreams" ? (
        <Box sx={{ ...panelSx, p: 1.35 }}>
          <Typography sx={labelSx}>Dream Board</Typography>
          <Typography sx={{ ...titleSx, fontSize: "1.2rem", mt: 0.35, mb: 1 }}>
            What this period points toward
          </Typography>
          <Grid2 container spacing={1}>
            {dreamCards.map((card, index) => (
              <Grid2 key={card.title} size={{ xs: 12, md: index === 0 ? 6 : 3 }}>
                <Box
                  sx={{
                    p: 1.25,
                    minHeight: 150,
                    border: "1px solid var(--surface-border)",
                    borderRadius: "8px",
                    background:
                      index === 0
                        ? "radial-gradient(circle at top left, var(--ui-rgba-100-160-230-180), transparent 46%), var(--ui-rgba-255-255-255-020)"
                        : "var(--ui-rgba-255-255-255-020)",
                  }}
                >
                  <Typography sx={labelSx}>Dream {index + 1}</Typography>
                  <Typography sx={{ ...titleSx, fontSize: "1rem", mt: 0.65 }}>{card.title}</Typography>
                  <Typography sx={{ ...bodySx, mt: 0.65 }}>{card.detail}</Typography>
                </Box>
              </Grid2>
            ))}
          </Grid2>
        </Box>
        ) : null}

        {storyTab === "replay" && showWeeklyReplay ? (
          <Box sx={{ ...panelSx, p: 1.35 }}>
            <Stack direction={{ xs: "column", md: "row" }} spacing={1.2} sx={{ justifyContent: "space-between", mb: 1 }}>
              <Box>
                <Typography sx={labelSx}>Weekly Replay</Typography>
                <Typography sx={{ ...titleSx, fontSize: "1.2rem", mt: 0.35 }}>
                  Step through the story when enough timestamps exist
                </Typography>
              </Box>
              <Chip className="arkreflect-pill" icon={<MonitorHeartRoundedIcon />} label={`${replayUnits.length} scenes`} />
            </Stack>
            <Grid2 container spacing={1.2}>
              <Grid2 size={{ xs: 12, lg: 5 }}>
                <ReactECharts option={activityOption} style={{ height: 170, width: "100%" }} />
              </Grid2>
              <Grid2 size={{ xs: 12, lg: 7 }}>
                <Box
                  sx={{
                    display: "grid",
                    gridTemplateColumns: { xs: "1fr", sm: "repeat(2, minmax(0, 1fr))", xl: "repeat(3, minmax(0, 1fr))" },
                    gap: 1,
                  }}
                >
                  {replayUnits.map((unit, index) => (
                    <Box
                      key={unit.id}
                      sx={{
                        p: 1,
                        minHeight: 112,
                        border: "1px solid var(--surface-border)",
                        borderRadius: "8px",
                        background: index === 0 ? "var(--ui-rgba-0-255-170-060)" : "var(--ui-rgba-255-255-255-020)",
                      }}
                    >
                      <Typography sx={labelSx}>
                        Scene {String(index + 1).padStart(2, "0")} - {formatUiDateTime(unit.occurred_at, { fallback: "time pending" })}
                      </Typography>
                      <Typography sx={{ fontWeight: 800, mt: 0.65 }}>{unitDisplayTitle(unit)}</Typography>
                      <Typography sx={{ ...bodySx, mt: 0.45 }}>
                        {(unit.summary || unit.content_preview || sourceMeta(unit.source_kind).group).slice(0, 132)}
                      </Typography>
                    </Box>
                  ))}
                </Box>
              </Grid2>
            </Grid2>
          </Box>
        ) : null}
      </Stack>
    );
  };

  return (
    <WorkspacePageShell spacing={1.4}>
      <WorkspacePageHeader
        eyebrow="ArkReflect"
        title="Your work, clustered into a clear recap"
        description={
          <span>
            See where chat, ArkOrbit, apps, goals, watchers, Sentinel, ArkPulse,
            ArkEvolve, usage, memory, and learned workflows concentrated.
          </span>
        }
        actions={
          <Stack
            direction={{ xs: "column", sm: "row" }}
            spacing={1}
            sx={{ minWidth: { xs: "100%", md: 460 } }}
          >
            <ToggleButtonGroup
              exclusive
              size="small"
              value={period}
              onChange={(_, value) => value && setPeriod(value)}
              aria-label="Reflection period"
              sx={{
                bgcolor: "rgba(255,255,255,0.06)",
                borderRadius: 2,
                "& .MuiToggleButton-root": {
                  minHeight: 40,
                  px: 1.6,
                  color: "text.secondary",
                  borderColor: "rgba(255,255,255,0.12)",
                },
                "& .Mui-selected": {
                  color: "primary.contrastText",
                  bgcolor: "primary.main",
                },
              }}
            >
              {PERIOD_OPTIONS.map((option) => (
                <ToggleButton key={option.value} value={option.value}>
                  {option.label}
                </ToggleButton>
              ))}
            </ToggleButtonGroup>
            <TextField
              size="small"
              type="date"
              value={anchor}
              onChange={(event) => setAnchor(event.target.value)}
              sx={{ minWidth: 164 }}
              slotProps={{
                input: {
                  startAdornment: <CalendarMonthRoundedIcon fontSize="small" />,
                },
              }}
            />
            <Tooltip title="Refresh recap in the background">
              <Button
                variant="outlined"
                onClick={() => refreshMutation.mutate()}
                disabled={refreshMutation.isPending || response?.refresh_status.running}
                startIcon={<RefreshRoundedIcon />}
                sx={{ minHeight: 40 }}
              >
                {response?.refresh_status.running || refreshMutation.isPending ? "Refreshing" : "Refresh"}
              </Button>
            </Tooltip>
          </Stack>
        }
      />

      {reflectQ.error ? <Alert severity="error">{errMessage(reflectQ.error)}</Alert> : null}
      {refreshMutation.error ? <Alert severity="error">{errMessage(refreshMutation.error)}</Alert> : null}

      {/* === ARKREFLECT STORY VIEW === */}
      {!hasReflectContent ? (
        <Box
          className="arkreflect-status"
          sx={{
            p: { xs: 2.2, md: 3 },
            border: "1px solid rgba(120, 200, 220, 0.18)",
            borderRadius: "3px",
            background:
              "linear-gradient(180deg, rgba(7, 13, 18, 0.96), rgba(5, 9, 12, 0.94))",
            boxShadow: "0 24px 60px rgba(0, 0, 0, 0.34)",
          }}
        >
          <Stack
            direction={{ xs: "column", md: "row" }}
            spacing={2}
            sx={{ alignItems: { xs: "flex-start", md: "center" } }}
          >
            <Box
              sx={{
                width: 46,
                height: 46,
                borderRadius: "6px",
                border: "1px solid rgba(120, 200, 220, 0.28)",
                color: "var(--cyan-glow)",
                display: "grid",
                placeItems: "center",
                background: "rgba(120, 200, 220, 0.07)",
                flex: "0 0 auto",
              }}
            >
              <AutoGraphRoundedIcon />
            </Box>
            <Box sx={{ flex: 1, minWidth: 0 }}>
              <Typography
                sx={{
                  fontFamily: "var(--font-display)",
                  fontSize: { xs: "1.25rem", md: "1.45rem" },
                  fontWeight: 750,
                  color: "rgba(237,247,244,0.96)",
                  mb: 0.5,
                }}
              >
                {status.active ? status.title : "Still collecting data"}
              </Typography>
              <Typography
                sx={{
                  maxWidth: 820,
                  color: "rgba(213,228,225,0.72)",
                  lineHeight: 1.55,
                }}
              >
                {emptyStateDetail}
              </Typography>
            </Box>
            <Chip
              className="arkreflect-pill"
              label={emptyStateChip}
              icon={status.active ? <RefreshRoundedIcon /> : <WorkHistoryRoundedIcon />}
              sx={{ flex: "0 0 auto" }}
            />
          </Stack>
          <LinearProgress
            variant={status.active ? "indeterminate" : "determinate"}
            value={status.active ? undefined : 0}
            sx={{ mt: 2.4, mb: 2 }}
          />
          <Grid2 container spacing={1.2}>
            {[
              { label: "Range", value: selectedRangeLabel || "Selected period" },
              {
                label: "Cached units",
                value: String(response?.cache_status.cached_units ?? totalUnits),
              },
              {
                label: "Source signals",
                value: String(sourceSignalCount),
              },
            ].map((item) => (
              <Grid2 key={item.label} size={{ xs: 12, sm: 4 }}>
                <Box
                  sx={{
                    p: 1.4,
                    border: "1px solid rgba(120, 200, 220, 0.12)",
                    borderRadius: "3px",
                    background: "rgba(255,255,255,0.025)",
                    minHeight: 78,
                  }}
                >
                  <Typography
                    sx={{
                      fontFamily: "'JetBrains Mono', 'IBM Plex Mono', Menlo, monospace",
                      fontSize: "0.66rem",
                      letterSpacing: "0.16em",
                      textTransform: "uppercase",
                      color: "rgba(180, 210, 225, 0.52)",
                      mb: 0.8,
                    }}
                  >
                    {item.label}
                  </Typography>
                  <Typography
                    sx={{
                      color: "rgba(237,247,244,0.9)",
                      fontWeight: 700,
                      lineHeight: 1.25,
                    }}
                  >
                    {item.value}
                  </Typography>
                </Box>
              </Grid2>
            ))}
          </Grid2>
          {response?.refresh_status.last_error ? (
            <Alert severity="warning" sx={{ mt: 2 }}>
              {response.refresh_status.last_error}
            </Alert>
          ) : null}
        </Box>
      ) : (
        renderStoryView()
      )}

      {response?.generated_at ? (
        <Typography variant="caption" color="text.secondary" sx={{ px: 0.5 }}>
          Cached view generated {formatUiDateTime(response.generated_at, { fallback: response.generated_at })}
          {response.refresh_status.completed_at
            ? ` - Last background refresh ${formatUiDateTime(response.refresh_status.completed_at, { fallback: response.refresh_status.completed_at })}`
            : ""}
        </Typography>
      ) : null}
    </WorkspacePageShell>
  );
}
