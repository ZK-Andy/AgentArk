import { Box, Card, CardContent, Stack, Typography } from "@mui/material";
import CheckCircleRoundedIcon from "@mui/icons-material/CheckCircleRounded";
import TrendingUpRoundedIcon from "@mui/icons-material/TrendingUpRounded";
import TrendingDownRoundedIcon from "@mui/icons-material/TrendingDownRounded";
import ScheduleRoundedIcon from "@mui/icons-material/ScheduleRounded";
import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { api } from "../api/client";
import type { LlmAnalyticsResponse, Task, TraceSummary } from "../types";

type Props = {
  tasks: Task[];
  traces: TraceSummary[];
};

function Sparkline({ values }: { values: number[] }) {
  if (!values || values.length < 2) return null;
  const min = Math.min(...values);
  const max = Math.max(...values);
  const range = Math.max(1e-9, max - min);
  const w = 120;
  const h = 28;
  const xs = values.map((_, i) => (w * i) / (values.length - 1));
  const ys = values.map((v) => 2 + (h - 4) * (1 - (v - min) / range));
  const line = xs.map((x, i) => `${x.toFixed(1)},${ys[i].toFixed(1)}`).join(" ");
  const area = `0,${h} ${line} ${w},${h}`;

  return (
    <svg width="100%" height={h} viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none" aria-hidden>
      <polygon points={area} fill="rgba(20, 241, 149, 0.15)" />
      <polyline
        points={line}
        fill="none"
        stroke="rgba(20, 241, 149, 0.8)"
        strokeWidth="2"
        strokeLinejoin="round"
        strokeLinecap="round"
      />
    </svg>
  );
}

function formatCompact(value: number): string {
  if (!Number.isFinite(value)) return "-";
  return new Intl.NumberFormat("en-US", {
    notation: value >= 1000 ? "compact" : "standard",
    maximumFractionDigits: value >= 1000 ? 1 : 0,
  }).format(value);
}

function formatSpend(value?: number | null): string {
  if (typeof value !== "number" || !Number.isFinite(value)) return "-";
  if (value >= 100) return `$${value.toFixed(0)}`;
  if (value >= 10) return `$${value.toFixed(1)}`;
  return `$${value.toFixed(2)}`;
}

export function TodaysHighlights({ tasks, traces }: Props) {
  const todayAnalyticsQ = useQuery({
    queryKey: ["mission-control-llm-analytics-24h"],
    queryFn: () => api.getLlmAnalytics({ range: "24h", bucket: "hour" }),
    staleTime: 60_000,
    refetchInterval: false,
  });
  const analytics30dQ = useQuery({
    queryKey: ["mission-control-llm-analytics-30d"],
    queryFn: () => api.getLlmAnalytics({ range: "30d", bucket: "day" }),
    staleTime: 60_000,
    refetchInterval: false,
  });

  const { completedToday, completedList, nextScheduled, trendPct, weekCounts, todayTraceCount } = useMemo(() => {
    const now = new Date();
    const todayStr = now.toISOString().slice(0, 10);

    // Completed tasks (best effort: check status and created_at)
    const allTasks = Array.isArray(tasks) ? tasks : [];
    const todayCompleted = allTasks.filter((t) => {
      const s = String(t?.status || "").toLowerCase();
      return (s.includes("completed") || s.includes("done")) && (t.created_at ? t.created_at.startsWith(todayStr) : false);
    });
    const list = todayCompleted.slice(0, 3);

    // Next scheduled
    const pending = allTasks.filter((t) => {
      const s = String(t?.status || "").toLowerCase();
      return s.includes("pending") && t.cron;
    });
    const next = pending.length > 0 ? pending[0] : null;

    // Weekly trend from traces
    const allTraces = Array.isArray(traces) ? traces : [];
    const dayMs = 86_400_000;
    const counts: number[] = [];
    for (let d = 6; d >= 0; d--) {
      const dayStart = new Date(now.getTime() - d * dayMs).toISOString().slice(0, 10);
      counts.push(allTraces.filter((tr) => (tr.started_at || "").startsWith(dayStart)).length);
    }
    const recentAvg = counts.slice(0, 6).reduce((a, b) => a + b, 0) / Math.max(1, 6);
    const todayCount = counts[counts.length - 1] || 0;
    const pct = recentAvg > 0 ? Math.round(((todayCount - recentAvg) / recentAvg) * 100) : 0;

    return {
      completedToday: todayCompleted.length,
      completedList: list,
      nextScheduled: next,
      trendPct: pct,
      weekCounts: counts,
      todayTraceCount: todayCount,
    };
  }, [tasks, traces]);

  const timeSavedMin = completedToday * 10; // heuristic: ~10min per automated task
  const todayAnalytics = todayAnalyticsQ.data as LlmAnalyticsResponse | undefined;
  const analytics30d = analytics30dQ.data as LlmAnalyticsResponse | undefined;
  const todayUsageRows = [
    {
      label: "Today spend",
      value: formatSpend(todayAnalytics?.totals?.cost_usd ?? null),
    },
    {
      label: "Today requests",
      value: formatCompact(todayAnalytics?.totals?.request_count ?? 0),
    },
    {
      label: "Today tokens",
      value: formatCompact(todayAnalytics?.totals?.total_tokens ?? 0),
    },
  ];
  const fallbackRows = [
    {
      label: "Last 30 days spend",
      value: formatSpend(analytics30d?.totals?.cost_usd ?? null),
    },
    {
      label: "Last 30 days requests",
      value: formatCompact(analytics30d?.totals?.request_count ?? 0),
    },
    {
      label: "Last 30 days tokens",
      value: formatCompact(analytics30d?.totals?.total_tokens ?? 0),
    },
  ];
  const todayUsagePresent =
    (todayAnalytics?.totals?.request_count ?? 0) > 0 ||
    (todayAnalytics?.totals?.total_tokens ?? 0) > 0 ||
    (todayAnalytics?.totals?.cost_usd ?? 0) > 0;
  const noTodayData = completedToday === 0 && todayTraceCount === 0 && !todayUsagePresent;

  return (
    <Card sx={{ height: "100%" }}>
      <CardContent sx={{ p: 1.5 }}>
        <Typography variant="h6" mb={1.25}>
          Today's Highlights
        </Typography>

        {noTodayData ? (
          <Stack spacing={1.1}>
            <Typography variant="body2" color="text.secondary">
              No meaningful activity yet today. Showing the last 30 days instead.
            </Typography>
            <Stack direction={{ xs: "column", sm: "row" }} spacing={1} useFlexGap flexWrap="wrap">
              {fallbackRows.map((row) => (
                <Box
                  key={row.label}
                  sx={{
                    flex: "1 1 0",
                    minWidth: 120,
                    px: 1.1,
                    py: 0.95,
                    borderRadius: "12px",
                    border: "1px solid rgba(108,156,212,0.16)",
                    background: "rgba(7, 18, 32, 0.56)",
                  }}
                >
                  <Typography variant="caption" color="text.secondary">
                    {row.label}
                  </Typography>
                  <Typography variant="h5" sx={{ mt: 0.3, fontWeight: 700, color: "#f3fbff" }}>
                    {row.value}
                  </Typography>
                </Box>
              ))}
            </Stack>
          </Stack>
        ) : (
          <>

            <Stack direction="row" alignItems="baseline" spacing={1} mb={1}>
              <Typography variant="h4" fontWeight={700} sx={{ color: "#14f195" }}>
                {completedToday}
              </Typography>
              <Typography variant="body2" color="text.secondary">
                tasks completed
              </Typography>
              {trendPct !== 0 ? (
                <Stack direction="row" alignItems="center" spacing={0.3}>
                  {trendPct > 0 ? (
                    <TrendingUpRoundedIcon sx={{ fontSize: 16, color: "#14f195" }} />
                  ) : (
                    <TrendingDownRoundedIcon sx={{ fontSize: 16, color: "#ff9800" }} />
                  )}
                  <Typography
                    variant="caption"
                    fontWeight={600}
                    sx={{ color: trendPct > 0 ? "#14f195" : "#ff9800" }}
                  >
                    {trendPct > 0 ? "+" : ""}{trendPct}% vs avg
                  </Typography>
                </Stack>
              ) : null}
            </Stack>

            {completedList.length > 0 ? (
              <Stack spacing={0.5} mb={1.25}>
                {completedList.map((t, idx) => (
                  <Stack key={t.id || idx} direction="row" spacing={0.75} alignItems="center">
                    <CheckCircleRoundedIcon sx={{ fontSize: 14, color: "#14f195", flexShrink: 0 }} />
                    <Typography variant="body2" noWrap title={String(t.description || "")}>
                      {String(t.description || "Task completed")}
                    </Typography>
                  </Stack>
                ))}
              </Stack>
            ) : (
              <Typography variant="body2" color="text.secondary" mb={1.25}>
                No completed tasks yet today.
              </Typography>
            )}

            {todayUsagePresent ? (
              <Stack direction={{ xs: "column", sm: "row" }} spacing={1} useFlexGap flexWrap="wrap" mb={1.1}>
                {todayUsageRows.map((row) => (
                  <Box
                    key={row.label}
                    sx={{
                      flex: "1 1 0",
                      minWidth: 120,
                      px: 1.1,
                      py: 0.9,
                      borderRadius: "12px",
                      border: "1px solid rgba(108,156,212,0.14)",
                      background: "rgba(7, 18, 32, 0.5)",
                    }}
                  >
                    <Typography variant="caption" color="text.secondary">
                      {row.label}
                    </Typography>
                    <Typography variant="h6" sx={{ mt: 0.25, fontWeight: 700, color: "#f3fbff" }}>
                      {row.value}
                    </Typography>
                  </Box>
                ))}
              </Stack>
            ) : null}

            {nextScheduled ? (
              <Stack direction="row" spacing={0.75} alignItems="center" mb={1}>
                <ScheduleRoundedIcon sx={{ fontSize: 14, color: "#2fd4ff", flexShrink: 0 }} />
                <Typography variant="body2" color="text.secondary">
                  Next: {String(nextScheduled.description || "Scheduled task").slice(0, 50)}
                </Typography>
              </Stack>
            ) : null}

            <Box sx={{ mt: 0.75, opacity: 0.9 }}>
              <Sparkline values={weekCounts} />
            </Box>

            {timeSavedMin > 0 ? (
              <Typography variant="caption" color="text.secondary" mt={0.75} display="block">
                Estimated ~{timeSavedMin} min saved today
              </Typography>
            ) : null}
          </>
        )}
      </CardContent>
    </Card>
  );
}
