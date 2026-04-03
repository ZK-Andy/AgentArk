import { Alert, Box, Chip, Grid2, Stack, Typography } from "@mui/material";
import { useQuery } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { api } from "../api/client";

const REFRESH_MS = 8000;

type JsonRecord = Record<string, unknown>;

type Props = {
  autoRefresh: boolean;
};

type ProvisionedAgent = {
  id: string;
  name: string;
  displayName: string;
  agentType: string;
  provider: string;
  model: string;
  capabilities: string[];
  createdAt: string;
  status: string;
  enabled: boolean;
  lastTask: string;
  lastSummary: string;
  lastUpdate: string;
  lastActivityAt: string;
};

type SwarmRunAgent = {
  id: string;
  agentName: string;
  agentRole: string;
  modelName: string;
  task: string;
  status: string;
  summary: string;
  latestUpdate: string;
  isSpecialist: boolean;
  elapsedMs?: number;
};

type SwarmRun = {
  id: string;
  conversationId: string;
  channel: string;
  request: string;
  status: string;
  summary: string;
  startedAt: string;
  updatedAt: string;
  completedAt: string;
  agentCount: number;
  agents: SwarmRunAgent[];
};

function isRecord(value: unknown): value is JsonRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function asRecord(value: unknown): JsonRecord {
  return isRecord(value) ? value : {};
}

function asRecords(value: unknown): JsonRecord[] {
  return Array.isArray(value) ? value.filter(isRecord) : [];
}

function pickRecords(value: unknown, key: string): JsonRecord[] {
  if (Array.isArray(value)) return asRecords(value);
  const obj = asRecord(value);
  return asRecords(obj[key]);
}

function str(value: unknown, fallback = ""): string {
  if (typeof value === "string") return value;
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

function bool(value: unknown): boolean {
  if (typeof value === "boolean") return value;
  if (typeof value === "number") return value !== 0;
  if (typeof value === "string") {
    return ["1", "true", "yes", "on"].includes(value.trim().toLowerCase());
  }
  return false;
}

function errMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return "Request failed.";
}

function formatTimestamp(value: unknown): string {
  const raw = str(value, "").trim();
  if (!raw) return "-";
  const parsed = new Date(raw);
  if (Number.isNaN(parsed.getTime())) return raw;
  return parsed.toLocaleString();
}

function formatElapsedMs(value: unknown): string {
  const ms = Math.max(0, num(value, 0));
  if (!ms) return "";
  if (ms < 1000) return `${ms}ms`;
  const secs = ms / 1000;
  if (secs < 60) return `${secs.toFixed(secs >= 10 ? 0 : 1)}s`;
  const mins = Math.floor(secs / 60);
  const remSecs = Math.round(secs % 60);
  return remSecs > 0 ? `${mins}m ${remSecs}s` : `${mins}m`;
}

function parseCapabilities(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value
      .map((item) => {
        if (typeof item === "string") return item.trim();
        const rec = asRecord(item);
        return str(rec.name, "").trim() || str(rec.description, "").trim();
      })
      .filter(Boolean);
  }
  const raw = str(value, "").trim();
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw) as unknown;
    return parseCapabilities(parsed);
  } catch {
    return raw
      .split(",")
      .map((item) => item.trim())
      .filter(Boolean);
  }
}

function normalizeLifecycleStatus(status: unknown): string {
  const normalized = str(status, "").trim().toLowerCase();
  if (!normalized) return "idle";
  if (normalized === "busy") return "running";
  if (normalized === "success") return "completed";
  if (normalized === "cancelled" || normalized === "canceled") return "interrupted";
  if (normalized === "degraded") return "partial";
  return normalized;
}

function statusChipColor(status: unknown): "default" | "success" | "warning" | "error" {
  switch (normalizeLifecycleStatus(status)) {
    case "completed":
    case "provisioned":
    case "idle":
      return "success";
    case "running":
    case "assigned":
    case "synthesizing":
    case "partial":
      return "warning";
    case "failed":
    case "timed_out":
    case "panicked":
    case "interrupted":
    case "offline":
    case "disabled":
      return "error";
    default:
      return "default";
  }
}

function statusChipLabel(status: unknown): string {
  switch (normalizeLifecycleStatus(status)) {
    case "assigned":
      return "Assigned";
    case "running":
      return "Running";
    case "synthesizing":
      return "Synthesizing";
    case "completed":
      return "Completed";
    case "partial":
      return "Partial";
    case "failed":
      return "Failed";
    case "timed_out":
      return "Timed out";
    case "panicked":
      return "Panicked";
    case "interrupted":
      return "Stopped";
    case "offline":
      return "Offline";
    case "disabled":
      return "Disabled";
    case "provisioned":
      return "Provisioned";
    default:
      return "Idle";
  }
}

function toProvisionedAgents(data: unknown): ProvisionedAgent[] {
  return pickRecords(data, "agents")
    .map((agent) => ({
      id: str(agent.id, ""),
      name: str(agent.name, "Agent"),
      displayName: str(agent.display_name, str(agent.name, "Agent")),
      agentType: str(agent.agent_type, "Agent"),
      provider: str(agent.llm_provider, "-"),
      model: str(agent.llm_model, "-"),
      capabilities: parseCapabilities(agent.capabilities),
      createdAt: str(agent.created_at, ""),
      status: normalizeLifecycleStatus(agent.status),
      enabled: bool(agent.enabled),
      lastTask: str(agent.last_task, ""),
      lastSummary: str(agent.last_summary, ""),
      lastUpdate: str(agent.last_update, ""),
      lastActivityAt: str(agent.last_activity_at, "")
    }))
    .sort((left, right) => {
      const leftTs = Date.parse(left.lastActivityAt || left.createdAt || "");
      const rightTs = Date.parse(right.lastActivityAt || right.createdAt || "");
      return (Number.isFinite(rightTs) ? rightTs : 0) - (Number.isFinite(leftTs) ? leftTs : 0);
    });
}

function toSwarmRuns(data: unknown): SwarmRun[] {
  return pickRecords(data, "runs")
    .map((run) => ({
      id: str(run.id, ""),
      conversationId: str(run.conversation_id, ""),
      channel: str(run.channel, ""),
      request: str(run.request, "Delegated run"),
      status: normalizeLifecycleStatus(run.status),
      summary: str(run.summary, ""),
      startedAt: str(run.started_at, ""),
      updatedAt: str(run.updated_at, ""),
      completedAt: str(run.completed_at, ""),
      agentCount: Math.max(0, num(run.agent_count, 0)),
      agents: pickRecords(run.agents, "agents").map((agent) => ({
        id: str(agent.id, ""),
        agentName: str(agent.agent_name, "Agent"),
        agentRole: str(agent.agent_role, ""),
        modelName: str(agent.model_name, ""),
        task: str(agent.task, ""),
        status: normalizeLifecycleStatus(agent.status),
        summary: str(agent.summary, ""),
        latestUpdate: str(agent.latest_update, ""),
        isSpecialist: bool(agent.is_specialist),
        elapsedMs: num(agent.elapsed_ms, 0) || undefined
      }))
    }))
    .filter((run) => run.id)
    .sort((left, right) => {
      const leftTs = Date.parse(left.updatedAt || left.startedAt || "");
      const rightTs = Date.parse(right.updatedAt || right.startedAt || "");
      return (Number.isFinite(rightTs) ? rightTs : 0) - (Number.isFinite(leftTs) ? leftTs : 0);
    });
}

function SectionShell({
  eyebrow,
  title,
  detail,
  children
}: {
  eyebrow: string;
  title: string;
  detail: string;
  children: ReactNode;
}) {
  return (
    <Box
      sx={{
        p: { xs: 2, md: 2.35 },
        borderRadius: "18px",
        border: "1px solid rgba(255,255,255,0.07)",
        background:
          "linear-gradient(180deg, rgba(255,255,255,0.05) 0%, rgba(255,255,255,0.025) 100%)",
        boxShadow: "0 18px 40px rgba(7, 16, 32, 0.22)"
      }}
    >
      <Stack spacing={1.4}>
        <Box>
          <Typography variant="overline" sx={{ letterSpacing: "0.16em", color: "info.light" }}>
            {eyebrow}
          </Typography>
          <Typography variant="h6" sx={{ fontWeight: 800 }}>
            {title}
          </Typography>
          <Typography variant="body2" color="text.secondary" sx={{ mt: 0.35, maxWidth: 860 }}>
            {detail}
          </Typography>
        </Box>
        {children}
      </Stack>
    </Box>
  );
}

function RunCard({ run, live = false }: { run: SwarmRun; live?: boolean }) {
  const trackedAgents = Math.max(run.agentCount, run.agents.length);
  return (
    <Box
      sx={{
        p: 1.5,
        borderRadius: "16px",
        border: live
          ? "1px solid rgba(88, 174, 255, 0.18)"
          : "1px solid rgba(255,255,255,0.07)",
        background: live
          ? "linear-gradient(180deg, rgba(88, 174, 255, 0.10) 0%, rgba(255,255,255,0.03) 100%)"
          : "linear-gradient(180deg, rgba(255,255,255,0.04) 0%, rgba(255,255,255,0.02) 100%)"
      }}
    >
      <Stack spacing={1.2}>
        <Stack
          direction={{ xs: "column", md: "row" }}
          alignItems={{ xs: "flex-start", md: "center" }}
          justifyContent="space-between"
          gap={1}
        >
          <Box sx={{ minWidth: 0 }}>
            <Typography variant="body1" sx={{ fontWeight: 700 }}>
              {run.request}
            </Typography>
            <Typography variant="caption" color="text.secondary" sx={{ display: "block", mt: 0.35 }}>
              {run.summary || "Delegated run details available below."}
            </Typography>
          </Box>
          <Stack direction="row" spacing={0.75} useFlexGap flexWrap="wrap">
            <Chip size="small" color={statusChipColor(run.status)} label={statusChipLabel(run.status)} />
            <Chip
              size="small"
              variant="outlined"
              label={`${trackedAgents} agent${trackedAgents === 1 ? "" : "s"}`}
            />
            {run.channel ? <Chip size="small" variant="outlined" label={run.channel} /> : null}
          </Stack>
        </Stack>

        <Stack direction="row" spacing={0.75} useFlexGap flexWrap="wrap">
          {run.startedAt ? (
            <Chip size="small" variant="outlined" label={`Started ${formatTimestamp(run.startedAt)}`} />
          ) : null}
          {run.completedAt ? (
            <Chip size="small" variant="outlined" label={`Finished ${formatTimestamp(run.completedAt)}`} />
          ) : null}
          {run.conversationId ? (
            <Chip size="small" variant="outlined" label={`Chat ${run.conversationId.slice(0, 8)}`} />
          ) : null}
        </Stack>

        <Grid2 container spacing={1}>
          {run.agents.map((agent) => (
            <Grid2 key={`${run.id}-${agent.id}`} size={{ xs: 12, xl: 6 }}>
              <Box
                sx={{
                  height: "100%",
                  p: 1.1,
                  borderRadius: "12px",
                  border: "1px solid rgba(255,255,255,0.06)",
                  background: "rgba(7, 12, 24, 0.58)"
                }}
              >
                <Stack spacing={0.75}>
                  <Stack
                    direction={{ xs: "column", sm: "row" }}
                    alignItems={{ xs: "flex-start", sm: "center" }}
                    justifyContent="space-between"
                    gap={0.75}
                  >
                    <Box sx={{ minWidth: 0 }}>
                      <Typography variant="body2" sx={{ fontWeight: 700 }}>
                        {agent.agentRole
                          ? `${agent.agentName} · ${agent.agentRole}`
                          : agent.agentName}
                      </Typography>
                      <Typography variant="caption" color="text.secondary">
                        {agent.modelName || (agent.isSpecialist ? "Specialist model" : "Auto agent")}
                      </Typography>
                    </Box>
                    <Stack direction="row" spacing={0.75} useFlexGap flexWrap="wrap">
                      <Chip
                        size="small"
                        color={statusChipColor(agent.status)}
                        label={statusChipLabel(agent.status)}
                        sx={{ height: 22 }}
                      />
                      {agent.elapsedMs ? (
                        <Chip
                          size="small"
                          variant="outlined"
                          label={formatElapsedMs(agent.elapsedMs)}
                          sx={{ height: 22 }}
                        />
                      ) : null}
                    </Stack>
                  </Stack>
                  {agent.task ? (
                    <Typography variant="body2" sx={{ color: "rgba(231, 239, 251, 0.94)" }}>
                      {agent.task}
                    </Typography>
                  ) : null}
                  <Typography variant="caption" color="text.secondary">
                    {agent.latestUpdate || agent.summary || "No extra detail recorded."}
                  </Typography>
                </Stack>
              </Box>
            </Grid2>
          ))}
        </Grid2>
      </Stack>
    </Box>
  );
}

export function SwarmManager({ autoRefresh }: Props) {
  const statusQ = useQuery({
    queryKey: ["swarm-status"],
    queryFn: () => api.rawGet("/swarm/status"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const agentsQ = useQuery({
    queryKey: ["swarm-agents"],
    queryFn: () => api.rawGet("/swarm/agents"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const configQ = useQuery({
    queryKey: ["swarm-config"],
    queryFn: () => api.rawGet("/swarm/config"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });
  const delegationsQ = useQuery({
    queryKey: ["swarm-delegations"],
    queryFn: () => api.rawGet("/swarm/delegations?limit=all"),
    refetchInterval: autoRefresh ? REFRESH_MS : false
  });

  const status = asRecord(statusQ.data);
  const config = asRecord(configQ.data);
  const agents = toProvisionedAgents(agentsQ.data);
  const activeRuns = toSwarmRuns({ runs: pickRecords(status.active_runs, "active_runs") });
  const recentRuns = toSwarmRuns(delegationsQ.data).filter(
    (run) => !activeRuns.some((active) => active.id === run.id)
  );
  const swarmEnabled = bool(status.enabled) || bool(config.enabled);
  const activeAgentCount = Math.max(0, num(status.active_agents, 0));
  const totalAgentCount = Math.max(agents.length, num(status.total_agents, 0));
  const interruptedRuns = recentRuns.filter((run) => run.status === "interrupted").length;
  const failedRuns = recentRuns.filter((run) =>
    ["failed", "timed_out", "panicked"].includes(run.status)
  ).length;
  const queryError = statusQ.error || configQ.error || agentsQ.error || delegationsQ.error;

  return (
    <Stack spacing={2.25}>
      <Box
        sx={{
          p: { xs: 2, md: 2.5 },
          borderRadius: "22px",
          border: "1px solid rgba(85, 177, 255, 0.18)",
          background:
            "radial-gradient(circle at top right, rgba(82, 180, 255, 0.16), transparent 38%), linear-gradient(180deg, rgba(13, 21, 40, 0.96) 0%, rgba(8, 14, 28, 0.98) 100%)",
          boxShadow: "0 28px 60px rgba(4, 10, 22, 0.34)"
        }}
      >
        <Stack spacing={2}>
          <Stack
            direction={{ xs: "column", md: "row" }}
            justifyContent="space-between"
            alignItems={{ xs: "flex-start", md: "center" }}
            gap={1.5}
          >
            <Box>
              <Typography variant="overline" sx={{ letterSpacing: "0.16em", color: "info.light" }}>
                Multi-agent control
              </Typography>
              <Typography variant="h5" sx={{ fontWeight: 800 }}>
                Agents
              </Typography>
              <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5, maxWidth: 820 }}>
                Live delegated runs, specialist roster, and recent swarm history stay visible here.
                Chat and this view now share the same execution state instead of splitting live work
                from history.
              </Typography>
            </Box>
            <Stack direction="row" spacing={1} useFlexGap flexWrap="wrap">
              <Chip
                size="small"
                color={swarmEnabled ? "success" : "default"}
                variant={swarmEnabled ? "filled" : "outlined"}
                label={swarmEnabled ? "Swarm enabled" : "Swarm disabled"}
              />
              <Chip size="small" variant="outlined" label={`${activeRuns.length} live run${activeRuns.length === 1 ? "" : "s"}`} />
            </Stack>
          </Stack>

          <Grid2 container spacing={1}>
            {[
              {
                label: "Active agents",
                value: String(activeAgentCount),
                tone: activeAgentCount > 0 ? "warning.main" : "text.primary"
              },
              {
                label: "Specialists",
                value: String(totalAgentCount),
                tone: "text.primary"
              },
              {
                label: "Interrupted runs",
                value: String(interruptedRuns),
                tone: interruptedRuns > 0 ? "warning.light" : "text.primary"
              },
              {
                label: "Failed runs",
                value: String(failedRuns),
                tone: failedRuns > 0 ? "error.light" : "text.primary"
              }
            ].map((item) => (
              <Grid2 key={item.label} size={{ xs: 6, lg: 3 }}>
                <Box
                  sx={{
                    p: 1.35,
                    borderRadius: "14px",
                    border: "1px solid rgba(255,255,255,0.07)",
                    background: "rgba(255,255,255,0.035)"
                  }}
                >
                  <Typography variant="caption" color="text.secondary">
                    {item.label}
                  </Typography>
                  <Typography variant="h5" sx={{ fontWeight: 800, color: item.tone }}>
                    {item.value}
                  </Typography>
                </Box>
              </Grid2>
            ))}
          </Grid2>
        </Stack>
      </Box>

      {queryError ? <Alert severity="error">{errMessage(queryError)}</Alert> : null}

      <SectionShell
        eyebrow="Live now"
        title="Delegated runs in progress"
        detail="Every active multi-agent run appears here with the same per-agent state shown in chat."
      >
        {activeRuns.length === 0 ? (
          <Box
            sx={{
              p: 1.5,
              borderRadius: "14px",
              border: "1px dashed rgba(255,255,255,0.12)",
              background: "rgba(255,255,255,0.02)"
            }}
          >
            <Typography variant="body2" sx={{ fontWeight: 700 }}>
              No live delegated runs
            </Typography>
            <Typography variant="body2" color="text.secondary" sx={{ mt: 0.45 }}>
              Ask for swarm explicitly in chat or let the router delegate a genuinely parallel task.
              Live runs will appear here immediately and update as agents work.
            </Typography>
          </Box>
        ) : (
          <Stack spacing={1.2}>
            {activeRuns.map((run) => (
              <RunCard key={run.id} run={run} live />
            ))}
          </Stack>
        )}
      </SectionShell>

      <SectionShell
        eyebrow="Roster"
        title="All specialist agents"
        detail="Configured specialists stay visible even while idle, with their latest task and status nearby."
      >
        {agents.length === 0 ? (
          <Typography variant="body2" color="text.secondary">
            No specialist agents have been provisioned yet.
          </Typography>
        ) : (
          <Grid2 container spacing={1.15}>
            {agents.map((agent) => (
              <Grid2 key={agent.id} size={{ xs: 12, md: 6, xl: 4 }}>
                <Box
                  sx={{
                    height: "100%",
                    p: 1.4,
                    borderRadius: "16px",
                    border: "1px solid rgba(255,255,255,0.07)",
                    background: "linear-gradient(180deg, rgba(255,255,255,0.04) 0%, rgba(255,255,255,0.02) 100%)"
                  }}
                >
                  <Stack spacing={1}>
                    <Stack
                      direction={{ xs: "column", sm: "row" }}
                      alignItems={{ xs: "flex-start", sm: "center" }}
                      justifyContent="space-between"
                      gap={0.9}
                    >
                      <Box sx={{ minWidth: 0 }}>
                        <Typography variant="body1" sx={{ fontWeight: 700 }}>
                          {agent.agentType
                            ? `${agent.displayName} · ${agent.agentType}`
                            : agent.displayName}
                        </Typography>
                        <Typography variant="caption" color="text.secondary">
                          {agent.provider} / {agent.model}
                        </Typography>
                      </Box>
                      <Chip
                        size="small"
                        color={statusChipColor(agent.enabled ? agent.status : "disabled")}
                        label={statusChipLabel(agent.enabled ? agent.status : "disabled")}
                      />
                    </Stack>

                    <Stack direction="row" spacing={0.75} useFlexGap flexWrap="wrap">
                      {agent.capabilities.slice(0, 5).map((capability) => (
                        <Chip
                          key={`${agent.id}-${capability}`}
                          size="small"
                          variant="outlined"
                          label={capability}
                          sx={{ height: 22 }}
                        />
                      ))}
                    </Stack>

                    <Box
                      sx={{
                        p: 1,
                        borderRadius: "12px",
                        background: "rgba(6, 11, 23, 0.48)",
                        border: "1px solid rgba(255,255,255,0.05)"
                      }}
                    >
                      <Typography variant="caption" color="text.secondary">
                        Latest task
                      </Typography>
                      <Typography variant="body2" sx={{ mt: 0.25, color: "rgba(231, 239, 251, 0.94)" }}>
                        {agent.lastTask || "No delegated task recorded yet."}
                      </Typography>
                      <Typography variant="caption" color="text.secondary" sx={{ display: "block", mt: 0.45 }}>
                        {agent.lastUpdate || agent.lastSummary || "This specialist is ready for new work."}
                      </Typography>
                    </Box>

                    <Stack direction="row" spacing={0.75} useFlexGap flexWrap="wrap">
                      {agent.lastActivityAt ? (
                        <Chip size="small" variant="outlined" label={`Last active ${formatTimestamp(agent.lastActivityAt)}`} />
                      ) : null}
                      <Chip size="small" variant="outlined" label={`Created ${formatTimestamp(agent.createdAt)}`} />
                    </Stack>
                  </Stack>
                </Box>
              </Grid2>
            ))}
          </Grid2>
        )}
      </SectionShell>

      <SectionShell
        eyebrow="History"
        title="Recent swarm runs"
        detail="Completed, interrupted, and failed runs stay here so you can review exactly which agents worked on each request."
      >
        {recentRuns.length === 0 ? (
          <Typography variant="body2" color="text.secondary">
            No completed swarm history has been recorded yet.
          </Typography>
        ) : (
          <Stack spacing={1.2}>
            {recentRuns.slice(0, 18).map((run) => (
              <RunCard key={run.id} run={run} />
            ))}
          </Stack>
        )}
      </SectionShell>
    </Stack>
  );
}
