import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
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
import DonutLargeRoundedIcon from "@mui/icons-material/DonutLargeRounded";
import ExpandMoreRoundedIcon from "@mui/icons-material/ExpandMoreRounded";
import HubRoundedIcon from "@mui/icons-material/HubRounded";
import InsightsRoundedIcon from "@mui/icons-material/InsightsRounded";
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

function parseReflectResponse(value: unknown, period: ReflectPeriod): ReflectResponse {
  const raw = asRecord(value);
  const sourceCounts = asRecord(raw.source_counts);
  const baselineSourceCounts = asRecord(raw.baseline_source_counts);
  const embedding = asRecord(raw.embedding_status);
  return {
    period,
    from: str(raw.from, ""),
    to: str(raw.to, ""),
    generated_at: str(raw.generated_at, ""),
    source_counts: {
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
    },
    baseline_source_counts: {
      main_chat: num(baselineSourceCounts.main_chat, 0),
      orbit_chat: num(baselineSourceCounts.orbit_chat, 0),
      memory: num(baselineSourceCounts.memory, 0),
      procedures: num(baselineSourceCounts.procedures, 0),
      apps: num(baselineSourceCounts.apps, 0),
      goals: num(baselineSourceCounts.goals, 0),
      watchers: num(baselineSourceCounts.watchers, 0),
      sentinel: num(baselineSourceCounts.sentinel, 0),
      arkpulse: num(baselineSourceCounts.arkpulse, 0),
      arkevolve: num(baselineSourceCounts.arkevolve, 0),
      usage: num(baselineSourceCounts.usage, 0),
    },
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

function relatedHistoryLabel(history: ReflectRelatedHistory): string {
  if (history.mode === "recurring") return "Recurring theme";
  if (history.mode === "new") return "New this period";
  return "History pending";
}

function relatedHistoryColor(history: ReflectRelatedHistory): "default" | "primary" | "success" {
  if (history.mode === "recurring") return "primary";
  if (history.mode === "new") return "success";
  return "default";
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

function clusterDisplayLabel(cluster: ReflectCluster): string {
  const sourceKinds = new Set(cluster.units.map((unit) => unit.source_kind));
  if (sourceKinds.size === 1) return sourceMeta(dominantSource(cluster)).group;
  if (sourceKinds.has("conversation") || sourceKinds.has("orbit_chat")) return "Conversation-led work";
  if (sourceKinds.has("watcher") || sourceKinds.has("sentinel") || sourceKinds.has("arkpulse")) {
    return "Background operations";
  }
  return "Mixed AgentArk activity";
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
      title: "No recap for this range yet",
      detail: "Choose another range or refresh when AgentArk is idle.",
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
  const bounds = useMemo(() => periodBounds(period, anchor), [period, anchor]);
  const fromIso = bounds.from.toISOString();
  const toIso = bounds.to.toISOString();
  const reflectQueryKey = useMemo(
    () => ["arkreflect", period, fromIso, toIso] as const,
    [period, fromIso, toIso],
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
    },
  });

  const response = reflectQ.data;

  useEffect(() => {
    if (!response?.refresh_status.running && !refreshMutation.isPending) return undefined;
    const id = window.setInterval(() => {
      void queryClient.invalidateQueries({ queryKey: reflectQueryKey });
    }, 5000);
    return () => window.clearInterval(id);
  }, [queryClient, reflectQueryKey, refreshMutation.isPending, response?.refresh_status.running]);

  const clusters = response?.clusters ?? [];
  const allUnits = useMemo(() => {
    const byId = new Map<string, ReflectUnit>();
    for (const cluster of clusters) {
      for (const unit of cluster.units) byId.set(unit.id, unit);
    }
    for (const unit of response?.unclustered_units ?? []) byId.set(unit.id, unit);
    return [...byId.values()];
  }, [clusters, response?.unclustered_units]);

  const totalUnits = allUnits.length;
  const strongestCluster = clusters[0] ?? null;
  const embeddingCoverage =
    response && response.embedding_status.total_units > 0
      ? response.embedding_status.embedded_units / response.embedding_status.total_units
      : 0;

  const rangeLabel = formatUiDateRange(response?.from || fromIso, response?.to || toIso);
  const status = quietStatus(response, reflectQ.isFetching, refreshMutation.isPending);
  const focusLabel = strongestCluster ? clusterDisplayLabel(strongestCluster) : "No activity yet";
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

  const constellationOption = useMemo(() => {
    const nodes: Array<Record<string, unknown>> = [];
    const links: Array<Record<string, unknown>> = [];
    const seen = new Set<string>();
    clusters.forEach((cluster, index) => {
      const source = dominantSource(cluster);
      const meta = sourceMeta(source);
      const clusterName = clusterDisplayLabel(cluster);
      const nodeId = `cluster-${cluster.id}`;
      seen.add(nodeId);
      nodes.push({
        id: nodeId,
        name: clusterName,
        value: cluster.unit_count,
        symbolSize: Math.max(46, Math.min(112, 44 + cluster.unit_count * 16)),
        category: 0,
        itemStyle: {
          color: meta.color,
          shadowBlur: 24,
          shadowColor: meta.color,
          opacity: 0.95,
        },
        label: {
          show: true,
          formatter: clusterName,
          color: "#fff",
          fontWeight: 800,
          fontSize: 12,
          width: 118,
          overflow: "break",
        },
        emphasis: { scale: true },
        x: Math.cos((index / Math.max(clusters.length, 1)) * Math.PI * 2) * 180,
        y: Math.sin((index / Math.max(clusters.length, 1)) * Math.PI * 2) * 100,
      });
      cluster.related_history.items.slice(0, 2).forEach((item, itemIndex) => {
        const historyId = `history-${item.id}`;
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
            symbolSize: 22,
            category: 1,
            itemStyle: {
              color: "rgba(255,255,255,0.38)",
              shadowBlur: 12,
              shadowColor: "rgba(255,255,255,0.35)",
            },
            label: { show: false },
          });
        }
        links.push({
          source: nodeId,
          target: historyId,
          value: item.similarity,
          lineStyle: {
            width: 1 + item.similarity * 2,
            color: "rgba(255,255,255,0.28)",
            curveness: 0.18 + itemIndex * 0.08,
          },
        });
      });
    });
    return {
      tooltip: {
        formatter: (info: { data?: { name?: string; value?: number } }) =>
          `${info.data?.name || "Theme"}${info.data?.value ? `<br/>${info.data.value} item${info.data.value === 1 ? "" : "s"}` : ""}`,
      },
      animationDurationUpdate: 900,
      series: [
        {
          type: "graph",
          layout: "force",
          roam: false,
          draggable: false,
          force: {
            repulsion: 360,
            edgeLength: [80, 180],
            gravity: 0.08,
          },
          categories: [{ name: "This range" }, { name: "History" }],
          data: nodes,
          links,
          edgeSymbol: ["none", "none"],
          lineStyle: { opacity: 0.42 },
        },
      ],
    };
  }, [clusters]);

  const activityOption = useMemo(() => {
    const counts = new Map<string, number>();
    for (const unit of allUnits) {
      const date = new Date(unit.occurred_at);
      if (Number.isNaN(date.getTime())) continue;
      const key =
        period === "daily"
          ? `${pad(date.getHours())}:00`
          : formatUiDateOnly(date.toISOString(), { fallback: date.toISOString().slice(0, 10) });
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    const labels = [...counts.keys()];
    return {
      tooltip: { trigger: "axis" },
      grid: { left: 32, right: 16, top: 18, bottom: 28 },
      xAxis: {
        type: "category",
        data: labels,
        axisLabel: { color: "rgba(255,255,255,0.68)" },
        axisLine: { lineStyle: { color: "rgba(255,255,255,0.16)" } },
      },
      yAxis: {
        type: "value",
        minInterval: 1,
        axisLabel: { color: "rgba(255,255,255,0.68)" },
        splitLine: { lineStyle: { color: "rgba(255,255,255,0.1)" } },
      },
      series: [
        {
          type: "line",
          smooth: true,
          data: labels.map((label) => counts.get(label) ?? 0),
          showSymbol: false,
          lineStyle: { color: "#00A8A8", width: 3 },
          areaStyle: {
            color: {
              type: "linear",
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: "rgba(0,168,168,0.42)" },
                { offset: 1, color: "rgba(0,168,168,0.02)" },
              ],
            },
          },
          itemStyle: {
            color: "#00A8A8",
          },
        },
      ],
    };
  }, [allUnits, period]);

  const sourceDonutOption = useMemo(
    () => ({
      tooltip: { trigger: "item" },
      legend: {
        bottom: 0,
        textStyle: { color: "rgba(255,255,255,0.7)" },
      },
      series: [
        {
          type: "pie",
          radius: ["48%", "72%"],
          center: ["50%", "43%"],
          avoidLabelOverlap: true,
          label: {
            color: "rgba(255,255,255,0.86)",
            formatter: "{b}",
          },
          labelLine: { lineStyle: { color: "rgba(255,255,255,0.28)" } },
          data: sourceRows.map((item) => ({
            name: item.label,
            value: item.count,
            itemStyle: { color: item.color },
          })),
        },
      ],
    }),
    [sourceRows],
  );

  const radarOption = useMemo(
    () => ({
      tooltip: {
        formatter: () =>
          "Working style is shown as change versus your recent baseline, not raw counts.",
      },
      radar: {
        radius: "68%",
        indicator: styleSignals.map((signal) => ({
          name: signal.label,
          max: 100,
        })),
        splitNumber: 4,
        axisName: { color: "rgba(255,255,255,0.72)", fontSize: 11 },
        splitLine: { lineStyle: { color: "rgba(255,255,255,0.13)" } },
        splitArea: { areaStyle: { color: ["rgba(255,255,255,0.02)", "rgba(255,255,255,0.05)"] } },
        axisLine: { lineStyle: { color: "rgba(255,255,255,0.13)" } },
      },
      series: [
        {
          type: "radar",
          data: [
            {
              name: "Change",
              value: styleSignals.map((signal) => Math.max(0, Math.min(100, 50 + signal.delta * 160))),
              areaStyle: { color: "rgba(78,141,255,0.22)" },
              lineStyle: { color: "#4E8DFF", width: 2 },
              itemStyle: { color: "#4E8DFF" },
            },
            {
              name: "Baseline",
              value: styleSignals.map(() => 50),
              areaStyle: { color: "rgba(255,255,255,0.04)" },
              lineStyle: { color: "rgba(255,255,255,0.38)", width: 1, type: "dashed" },
              itemStyle: { color: "rgba(255,255,255,0.55)" },
            },
          ],
        },
      ],
    }),
    [styleSignals],
  );

  const backgroundOption = useMemo(() => {
    const rows = SOURCE_ORDER.filter((source) =>
      ["app", "goal", "watcher", "sentinel", "arkpulse", "arkevolve"].includes(source),
    )
      .map((source) => ({ ...sourceMeta(source), count: countForSource(response, source) }))
      .filter((row) => row.count > 0);
    return {
      tooltip: { trigger: "axis", axisPointer: { type: "shadow" } },
      grid: { left: 92, right: 18, top: 10, bottom: 24 },
      xAxis: {
        type: "value",
        minInterval: 1,
        axisLabel: { color: "rgba(255,255,255,0.64)" },
        splitLine: { lineStyle: { color: "rgba(255,255,255,0.1)" } },
      },
      yAxis: {
        type: "category",
        data: rows.map((row) => row.label),
        axisLabel: { color: "rgba(255,255,255,0.72)" },
        axisLine: { show: false },
        axisTick: { show: false },
      },
      series: [
        {
          type: "bar",
          data: rows.map((row) => ({
            value: row.count,
            itemStyle: { color: row.color, borderRadius: [0, 6, 6, 0] },
          })),
          barMaxWidth: 24,
        },
      ],
    };
  }, [response]);

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
      <Box
        className="list-shell"
        sx={{
          p: 1.25,
          borderColor: status.active ? "rgba(0,168,168,0.34)" : "rgba(255,255,255,0.1)",
          bgcolor: status.active ? "rgba(0,168,168,0.08)" : "rgba(255,255,255,0.035)",
        }}
      >
        <Stack direction="row" spacing={1.1} sx={{ alignItems: "center" }}>
          <InsightsRoundedIcon color={status.active ? "primary" : "disabled"} fontSize="small" />
          <Box sx={{ minWidth: 0, flex: 1 }}>
            <Typography variant="body2" sx={{ fontWeight: 800 }}>
              {status.title}
            </Typography>
            <Typography variant="caption" color="text.secondary">
              {status.detail}
            </Typography>
          </Box>
          <Chip
            size="small"
            label={
              response?.embedding_status.mode === "semantic"
                ? `${Math.round(embeddingCoverage * 100)}% grouped`
                : "Preparing"
            }
            variant="outlined"
          />
        </Stack>
        {status.active || reflectQ.isFetching ? <LinearProgress sx={{ mt: 1.1, borderRadius: 999 }} /> : null}
      </Box>

      <Box className="list-shell" sx={{ p: { xs: 1.4, md: 2 }, borderColor: "rgba(78,141,255,0.2)" }}>
        <Stack spacing={1.3}>
          <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
            <InsightsRoundedIcon color="primary" />
            <Box>
              <Typography variant="h6" sx={{ fontWeight: 850 }}>
                What I noticed
              </Typography>
              <Typography variant="body2" color="text.secondary">
                A plain-language read of this period before the charts.
              </Typography>
            </Box>
          </Stack>
          <Stack spacing={0.8}>
            {narrative.map((line) => (
              <Typography key={line} variant="body1" sx={{ lineHeight: 1.65 }}>
                {line}
              </Typography>
            ))}
          </Stack>
        </Stack>
      </Box>

      <Box
        className="list-shell"
        sx={{
          p: { xs: 1.2, md: 1.6 },
          minHeight: 460,
          overflow: "hidden",
          background:
            "radial-gradient(circle at 30% 25%, rgba(78,141,255,0.16), transparent 34%), radial-gradient(circle at 72% 34%, rgba(0,168,168,0.12), transparent 30%), rgba(255,255,255,0.025)",
        }}
      >
        <Stack direction="row" sx={{ justifyContent: "space-between", alignItems: "flex-start", mb: 1 }}>
          <Box>
            <Typography variant="h6" sx={{ fontWeight: 850 }}>
              Panorama
            </Typography>
            <Typography variant="body2" color="text.secondary">
              Islands are focus areas. Bridges connect this period to similar history.
            </Typography>
          </Box>
          <Stack direction="row" spacing={0.7} sx={{ flexWrap: "wrap", justifyContent: "flex-end", gap: 0.7 }}>
            <Chip size="small" icon={<BubbleChartRoundedIcon />} label={`${clusters.length} focus areas`} />
            <Chip size="small" icon={<WorkHistoryRoundedIcon />} label={`${recurringCount} recurring`} />
          </Stack>
        </Stack>
        {clusters.length > 0 ? (
          <ReactECharts option={constellationOption} style={{ height: 385, width: "100%" }} />
        ) : (
          <Box sx={{ height: 385, display: "grid", placeItems: "center", textAlign: "center" }}>
            <Stack spacing={0.8} sx={{ alignItems: "center" }}>
              <BubbleChartRoundedIcon color="disabled" />
              <Typography color="text.secondary">
                {status.active ? "Preparing the first panorama." : "No activity found in this range."}
              </Typography>
            </Stack>
          </Box>
        )}
      </Box>

      <Grid2 container spacing={1.2}>
        <Grid2 size={{ xs: 12, lg: 5 }}>
          <Box className="list-shell" sx={{ p: 1.2, minHeight: 360 }}>
            <Stack direction="row" spacing={1} sx={{ alignItems: "center", px: 0.4 }}>
              <AutoGraphRoundedIcon color="success" />
              <Box>
                <Typography variant="subtitle1" sx={{ fontWeight: 800 }}>
                  Working style
                </Typography>
                <Typography variant="body2" color="text.secondary">
                  Change versus your recent baseline.
                </Typography>
              </Box>
            </Stack>
            <ReactECharts option={radarOption} style={{ height: 292, width: "100%" }} />
          </Box>
        </Grid2>
        <Grid2 size={{ xs: 12, lg: 7 }}>
          <Box className="list-shell" sx={{ p: 1.2, minHeight: 360 }}>
            <Stack direction="row" spacing={1} sx={{ alignItems: "center", px: 0.4 }}>
              <MonitorHeartRoundedIcon color="info" />
              <Box>
                <Typography variant="subtitle1" sx={{ fontWeight: 800 }}>
                  Background agent lane
                </Typography>
                <Typography variant="body2" color="text.secondary">
                  Apps, goals, watchers, Sentinel, ArkPulse, and ArkEvolve.
                </Typography>
              </Box>
            </Stack>
            {backgroundCount > 0 ? (
              <ReactECharts option={backgroundOption} style={{ height: 292, width: "100%" }} />
            ) : (
              <Box sx={{ height: 292, display: "grid", placeItems: "center", textAlign: "center" }}>
                <Typography color="text.secondary">No background activity in this range.</Typography>
              </Box>
            )}
          </Box>
        </Grid2>
      </Grid2>

      <Grid2 container spacing={1.2}>
        <Grid2 size={{ xs: 12, lg: 7 }}>
          <Box className="list-shell" sx={{ p: 1.2, minHeight: 330 }}>
            <Typography variant="subtitle1" sx={{ fontWeight: 800, px: 0.4 }}>
              Timeline ribbon
            </Typography>
            <Typography variant="body2" color="text.secondary" sx={{ px: 0.4 }}>
              The rhythm of this period.
            </Typography>
            {allUnits.length > 0 ? (
              <ReactECharts option={activityOption} style={{ height: 265, width: "100%" }} />
            ) : (
              <Box sx={{ height: 265, display: "grid", placeItems: "center", textAlign: "center" }}>
                <Typography color="text.secondary">No rhythm to show yet.</Typography>
              </Box>
            )}
          </Box>
        </Grid2>
        <Grid2 size={{ xs: 12, lg: 5 }}>
          <Box className="list-shell" sx={{ p: 1.2, minHeight: 330 }}>
            <Stack direction="row" spacing={1} sx={{ alignItems: "center", px: 0.4 }}>
              <DonutLargeRoundedIcon color="warning" />
              <Box>
                <Typography variant="subtitle1" sx={{ fontWeight: 800 }}>
                  What contributed
                </Typography>
                <Typography variant="body2" color="text.secondary">
                  The sources behind the recap.
                </Typography>
              </Box>
            </Stack>
            {sourceRows.length > 0 ? (
              <ReactECharts option={sourceDonutOption} style={{ height: 265, width: "100%" }} />
            ) : (
              <Box sx={{ height: 265, display: "grid", placeItems: "center", textAlign: "center" }}>
                <Typography color="text.secondary">Sources will appear after the recap is prepared.</Typography>
              </Box>
            )}
          </Box>
        </Grid2>
      </Grid2>

      <Accordion
        disableGutters
        sx={{
          bgcolor: "rgba(255,255,255,0.035)",
          border: "1px solid rgba(255,255,255,0.1)",
          borderRadius: "8px !important",
          color: "text.primary",
          boxShadow: "none",
          "&:before": { display: "none" },
        }}
      >
        <AccordionSummary expandIcon={<ExpandMoreRoundedIcon />}>
          <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
            <WorkHistoryRoundedIcon color="info" fontSize="small" />
            <Typography sx={{ fontWeight: 800 }}>Examples and evidence</Typography>
            <Chip size="small" label={`${totalUnits} item${totalUnits === 1 ? "" : "s"}`} />
          </Stack>
        </AccordionSummary>
        <AccordionDetails>
          <Grid2 container spacing={1.2}>
            {clusters.map((cluster) => {
              const sourceEntries = Object.entries(cluster.source_mix).sort((a, b) => b[1] - a[1]);
              return (
                <Grid2 size={{ xs: 12, md: 6, xl: 4 }} key={cluster.id}>
                  <Box className="list-shell" sx={{ p: 1.35, minHeight: 240 }}>
                    <Stack spacing={1}>
                      <Stack direction="row" sx={{ alignItems: "flex-start", justifyContent: "space-between", gap: 1 }}>
                        <Box sx={{ minWidth: 0 }}>
                          <Typography variant="subtitle1" sx={{ fontWeight: 850, lineHeight: 1.2 }}>
                            {clusterDisplayLabel(cluster)}
                          </Typography>
                          <Typography variant="body2" color="text.secondary" sx={{ mt: 0.35 }}>
                            {clusterPlainSummary(cluster)}
                          </Typography>
                        </Box>
                        <Chip size="small" label={cluster.unit_count} sx={{ fontWeight: 800 }} />
                      </Stack>
                      <Stack direction="row" sx={{ flexWrap: "wrap", gap: 0.65 }}>
                        {sourceEntries.map(([label, count]) => (
                          <Chip
                            key={label}
                            size="small"
                            icon={sourceIcon(label)}
                            label={`${label} ${count}`}
                            variant="outlined"
                          />
                        ))}
                        <Chip
                          size="small"
                          color={relatedHistoryColor(cluster.related_history)}
                          label={relatedHistoryLabel(cluster.related_history)}
                          variant={cluster.related_history.mode === "unavailable" ? "outlined" : "filled"}
                        />
                      </Stack>
                      <Typography variant="body2" color="text.secondary">
                        {relatedHistoryText(cluster.related_history)}
                      </Typography>
                      <Divider />
                      <Stack spacing={0.9}>
                        {cluster.units.slice(0, 4).map((unit) => (
                          <Box key={unit.id} sx={{ minWidth: 0 }}>
                            <Stack direction="row" spacing={0.75} sx={{ alignItems: "center" }}>
                              <Chip size="small" label={unit.source_label} sx={{ height: 22 }} />
                              <Typography variant="caption" color="text.secondary">
                                {formatUiDateTime(unit.occurred_at, { fallback: unit.occurred_at })}
                              </Typography>
                            </Stack>
                            <Typography variant="body2" sx={{ mt: 0.35, fontWeight: 700 }}>
                              {unitDisplayTitle(unit)}
                            </Typography>
                            <Typography variant="body2" color="text.secondary">
                              {unit.content_preview || unit.summary}
                            </Typography>
                          </Box>
                        ))}
                      </Stack>
                    </Stack>
                  </Box>
                </Grid2>
              );
            })}
          </Grid2>
        </AccordionDetails>
      </Accordion>

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
