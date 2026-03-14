import {
  Alert,
  Badge,
  Box,
  Button,
  Card,
  CardContent,
  Stack,
  Typography
} from "@mui/material";
import CheckCircleOutlineRoundedIcon from "@mui/icons-material/CheckCircleOutlineRounded";
import WarningAmberRoundedIcon from "@mui/icons-material/WarningAmberRounded";
import type { Notification, Task } from "../types";

type AttentionItem = {
  id: string;
  kind: "approval" | "failed" | "security" | "setup";
  title: string;
  detail?: string;
  targetView?: string;
};

function notificationTargetView(notification: Notification): string {
  const title = String(notification.title || "").toLowerCase();
  const body = String(notification.body || "").toLowerCase();
  const source = String(notification.source || notification.metadata?.source || "").toLowerCase();
  const hay = `${title} ${body} ${source}`;
  if (hay.includes("arkpulse")) return "arkpulse";
  return "settings";
}

type Props = {
  tasks: Task[];
  notifications: Notification[];
  securityLogs: Array<{ event_type: string; severity: string; message: string }>;
  settingsLoaded: boolean;
  hasLlmConfigured: boolean;
  onApprove: (id: string) => void;
  onReject: (id: string) => void;
  onRetry: (id: string) => void;
  onNavigate: (view: string) => void;
  approving: boolean;
  rejecting: boolean;
  retrying: boolean;
};

function isTestArtifactTask(task: Task): boolean {
  const desc = String(task?.description || "")
    .toLowerCase()
    .replace(/\u2014/g, "-")
    .trim();
  if (!desc.includes("safe to delete")) return false;
  return desc.includes("e2e test task") || desc.includes("integration test task");
}

function buildItems(
  tasks: Task[],
  notifications: Notification[],
  securityLogs: Array<{ event_type: string; severity: string; message: string }>,
  settingsLoaded: boolean,
  hasLlmConfigured: boolean
): AttentionItem[] {
  const items: AttentionItem[] = [];

  // Setup nudge: no LLM model pool configured
  if (settingsLoaded && !hasLlmConfigured) {
    items.push({
      id: "__setup_llm",
      kind: "setup",
      title: "Set up your AI model",
      detail: "No LLM model is configured yet. Go to Settings > LLM Config to get started."
    });
  }

  // Tasks awaiting approval
  for (const t of tasks) {
    if (isTestArtifactTask(t)) continue;
    const s = String(t?.status || "").toLowerCase();
    if (s.includes("awaitingapproval")) {
      items.push({
        id: t.id,
        kind: "approval",
        title: t.description || "Task needs approval",
      });
    }
  }

  // Failed tasks
  for (const t of tasks) {
    if (isTestArtifactTask(t)) continue;
    const s = String(t?.status || "").toLowerCase();
    if (s.includes("failed")) {
      items.push({
        id: t.id,
        kind: "failed",
        title: t.description || "Task failed",
      });
    }
  }

  // Critical security alerts
  for (const log of securityLogs) {
    const sev = (log.severity || "").toLowerCase();
    if (sev === "high" || sev === "critical") {
      items.push({
        id: `sec_${log.event_type}_${log.message?.slice(0, 20)}`,
        kind: "security",
        title: log.message || `Security: ${log.event_type}`,
        detail: `Severity: ${log.severity}`,
      });
    }
  }

  // Critical unread notifications
  for (const n of notifications) {
    if (!n.read && (n.level === "error" || n.level === "critical")) {
      items.push({
        id: `notif_${n.id}`,
        kind: "security",
        title: n.title || "Alert",
        detail: n.body?.slice(0, 80),
        targetView: notificationTargetView(n),
      });
    }
  }

  return items.slice(0, 6);
}

export function NeedsAttentionInbox({
  tasks,
  notifications,
  securityLogs,
  settingsLoaded,
  hasLlmConfigured,
  onApprove,
  onReject,
  onRetry,
  onNavigate,
  approving,
  rejecting,
  retrying,
}: Props) {
  const items = buildItems(tasks, notifications, securityLogs, settingsLoaded, hasLlmConfigured);
  const count = items.length;

  return (
    <Card className="attention-card">
      <CardContent sx={{ p: 1.5 }}>
        <Stack direction="row" alignItems="center" spacing={1} mb={count > 0 ? 1.25 : 0}>
          <WarningAmberRoundedIcon
            sx={{ color: count > 0 ? "rgba(255, 167, 38, 0.9)" : "rgba(155, 180, 214, 0.4)", fontSize: 20 }}
          />
          <Typography variant="h6" sx={{ flex: 1 }}>
            Needs Your Attention
          </Typography>
          {count > 0 ? (
            <Badge badgeContent={count} color="warning" />
          ) : null}
        </Stack>

        {count === 0 ? (
          <Box className="empty-state" sx={{ py: 2.5 }}>
            <CheckCircleOutlineRoundedIcon
              sx={{ fontSize: 36, color: "rgba(20, 241, 149, 0.6)" }}
            />
            <Typography variant="body2" color="text.secondary">
              All caught up!
            </Typography>
          </Box>
        ) : (
          <Stack spacing={0.75}>
            {items.map((item) => (
              <Box key={item.id} className="action-row" sx={{ p: "8px 10px" }}>
                <Stack
                  direction={{ xs: "column", sm: "row" }}
                  spacing={1}
                  justifyContent="space-between"
                  alignItems={{ xs: "flex-start", sm: "center" }}
                >
                  <Box sx={{ flex: 1, minWidth: 0 }}>
                    <Typography variant="body2" fontWeight={600} noWrap title={item.title}>
                      {item.title}
                    </Typography>
                    {item.detail ? (
                      <Typography variant="caption" color="text.secondary" noWrap>
                        {item.detail}
                      </Typography>
                    ) : null}
                  </Box>

                  <Stack direction="row" spacing={0.6} flexShrink={0}>
                    {item.kind === "approval" ? (
                      <>
                        <Button
                          variant="contained"
                          size="small"
                          color="success"
                          disabled={approving}
                          onClick={() => onApprove(item.id)}
                          sx={{ minWidth: 70, textTransform: "none" }}
                        >
                          Approve
                        </Button>
                        <Button
                          variant="outlined"
                          size="small"
                          color="warning"
                          disabled={rejecting}
                          onClick={() => onReject(item.id)}
                          sx={{ minWidth: 60, textTransform: "none" }}
                        >
                          Reject
                        </Button>
                      </>
                    ) : item.kind === "failed" ? (
                      <Button
                        variant="outlined"
                        size="small"
                        disabled={retrying}
                        onClick={() => onRetry(item.id)}
                        sx={{ textTransform: "none" }}
                      >
                        Retry
                      </Button>
                    ) : item.kind === "setup" ? (
                      <Button
                        variant="contained"
                        size="small"
                        onClick={() => onNavigate("settings")}
                        sx={{ textTransform: "none" }}
                      >
                        Set Up
                      </Button>
                    ) : (
                      <Button
                        variant="text"
                        size="small"
                        onClick={() => onNavigate(item.targetView || "settings")}
                        sx={{ textTransform: "none" }}
                      >
                        View
                      </Button>
                    )}
                  </Stack>
                </Stack>
              </Box>
            ))}

            {count >= 5 ? (
              <Button
                size="small"
                onClick={() => onNavigate("skills")}
                sx={{ textTransform: "none", alignSelf: "flex-start", mt: 0.5 }}
              >
                View all tasks
              </Button>
            ) : null}
          </Stack>
        )}
      </CardContent>
    </Card>
  );
}
