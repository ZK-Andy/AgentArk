import { Button, Card, CardContent, Chip, Stack, Typography } from "@mui/material";
import type { TraceOperationalEvent, TraceSummary } from "../types";

type Props = {
  history: TraceSummary[];
  events?: TraceOperationalEvent[];
  compact?: boolean;
  onHideAdvanced?: () => void;
};

function eventLabel(event: TraceOperationalEvent): string {
  const parts = [
    event.event_type.replace(/_/g, " "),
    event.tool_name || "",
    event.outcome || "",
  ].filter(Boolean);
  return parts.join(" • ");
}

function eventColor(event: TraceOperationalEvent): "success" | "error" | "warning" | "default" {
  if (!event.success) return "error";
  const normalized = `${event.event_type} ${event.outcome}`.toLowerCase();
  if (normalized.includes("blocked") || normalized.includes("warning")) return "warning";
  if (normalized.includes("complete") || normalized.includes("ok")) return "success";
  return "default";
}

function shortTimestamp(value: string): string {
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return "--";
  return parsed.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

export function LiveEventConsole({ history, events = [], compact = false, onHideAdvanced }: Props) {
  const lines = history.slice(0, compact ? 4 : 7);
  const eventLines = events.slice(0, compact ? 5 : 8);
  return (
    <Card sx={compact ? { minHeight: 0, height: "100%" } : { minHeight: 270 }}>
      <CardContent sx={compact ? { p: 1.25, height: "100%", overflow: "auto" } : undefined}>
        <Stack direction="row" justifyContent="space-between" alignItems="center" mb={compact ? 1 : 2}>
          <Typography variant="h6">Live Execution Console</Typography>
          <Stack direction="row" spacing={1} alignItems="center">
            {onHideAdvanced ? (
              <Button
                size="small"
                variant="outlined"
                color="warning"
                onClick={onHideAdvanced}
                sx={{ textTransform: "none" }}
              >
                Hide advanced
              </Button>
            ) : null}
            <Chip
              size="small"
              color="secondary"
              variant="outlined"
              label={history.length > 0 ? "Streaming" : "Idle"}
            />
          </Stack>
        </Stack>

        <Stack spacing={1} className="console-scroll" sx={compact ? { maxHeight: "none" } : undefined}>
          {eventLines.length > 0 ? (
            eventLines.map((item) => (
              <Stack key={item.id} direction="row" spacing={1.5} className="console-line" alignItems="center">
                <Typography className="console-index">{shortTimestamp(item.created_at)}</Typography>
                <Typography className="console-message" sx={{ flex: 1 }}>
                  {eventLabel(item)}
                </Typography>
                <Chip
                  size="small"
                  variant="outlined"
                  color={eventColor(item)}
                  label={item.success ? "ok" : "attention"}
                />
              </Stack>
            ))
          ) : lines.length === 0 ? (
            <Typography variant="body2" color="text.secondary">
              No recent traces yet.
            </Typography>
          ) : (
            lines.map((item, i) => (
              <Stack key={item.id} direction="row" spacing={1.5} className="console-line">
                <Typography className="console-index">{String(i + 1).padStart(2, "0")}</Typography>
                <Typography className="console-message">
                  {item.message_preview || "(empty prompt)"} ({item.channel})
                </Typography>
              </Stack>
            ))
          )}
        </Stack>
      </CardContent>
    </Card>
  );
}
