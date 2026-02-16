import { Button, Card, CardContent, Chip, Stack, Typography } from "@mui/material";
import type { TraceSummary } from "../types";

type Props = {
  history: TraceSummary[];
  compact?: boolean;
  onHideAdvanced?: () => void;
};

export function LiveEventConsole({ history, compact = false, onHideAdvanced }: Props) {
  const lines = history.slice(0, compact ? 4 : 7);
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
          {lines.length === 0 ? (
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