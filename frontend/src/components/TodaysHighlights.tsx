import { Box, Card, CardContent, Stack, Typography } from "@mui/material";
import CheckCircleRoundedIcon from "@mui/icons-material/CheckCircleRounded";
import TrendingUpRoundedIcon from "@mui/icons-material/TrendingUpRounded";
import TrendingDownRoundedIcon from "@mui/icons-material/TrendingDownRounded";
import ScheduleRoundedIcon from "@mui/icons-material/ScheduleRounded";
import { useMemo } from "react";
import type { Task, TraceSummary } from "../types";

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

export function TodaysHighlights({ tasks, traces }: Props) {
  const { completedToday, completedList, nextScheduled, trendPct, weekCounts } = useMemo(() => {
    const now = new Date();
    const todayStr = now.toISOString().slice(0, 10);

    // Completed tasks (best effort: check status and created_at)
    const allTasks = Array.isArray(tasks) ? tasks : [];
    const completed = allTasks.filter((t) => {
      const s = String(t?.status || "").toLowerCase();
      return s.includes("completed") || s.includes("done");
    });

    // Try to filter to today using created_at as proxy
    const todayCompleted = completed.filter((t) =>
      t.created_at ? t.created_at.startsWith(todayStr) : false
    );
    const list = (todayCompleted.length > 0 ? todayCompleted : completed).slice(0, 3);

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
      completedToday: todayCompleted.length || completed.length,
      completedList: list,
      nextScheduled: next,
      trendPct: pct,
      weekCounts: counts,
    };
  }, [tasks, traces]);

  const timeSavedMin = completedToday * 10; // heuristic: ~10min per automated task

  return (
    <Card sx={{ height: "100%" }}>
      <CardContent sx={{ p: 1.5 }}>
        <Typography variant="h6" mb={1.25}>
          Today's Highlights
        </Typography>

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
      </CardContent>
    </Card>
  );
}
