import { Alert, Box, Button, Chip, Dialog, DialogActions, DialogContent, DialogTitle, Grid2, IconButton, MenuItem, Stack, TextField, Tooltip, Typography } from "@mui/material";
import CloseIcon from "@mui/icons-material/Close";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "../api/client";

const REFRESH_MS = 8000;
const HISTORY_LIMIT = 200;

type JsonRecord = Record<string, unknown>;

type Props = {
  autoRefresh: boolean;
};

type AgentPreset = {
  key: string;
  label: string;
  description: string;
  suggestedName: string;
  capabilities: string[];
  systemPrompt: string;
  icon: string;
};

const AGENT_PRESETS: AgentPreset[] = [
  {
    key: "researcher",
    label: "Researcher",
    description: "Broad questions, source gathering, comparisons, first-pass synthesis.",
    suggestedName: "Research Lead",
    capabilities: ["research", "source review", "comparison", "fact gathering"],
    systemPrompt: "Focus on finding facts, comparing sources, and delivering concise research notes with key evidence.",
    icon: "\uD83D\uDD0D"
  },
  {
    key: "coder",
    label: "Coder",
    description: "Implementation, debugging, patch generation, code execution.",
    suggestedName: "Implementation Agent",
    capabilities: ["coding", "debugging", "refactoring", "implementation"],
    systemPrompt: "Focus on concrete implementation details, debugging, code changes, and actionable technical output.",
    icon: "\uD83D\uDCBB"
  },
  {
    key: "analyst",
    label: "Analyst",
    description: "Breakdowns, diagnosis, metrics, tradeoffs, decisions.",
    suggestedName: "Analysis Agent",
    capabilities: ["analysis", "root cause", "triage", "metrics"],
    systemPrompt: "Focus on diagnosis, decomposition, prioritization, and clear analytical reasoning.",
    icon: "\uD83D\uDCCA"
  },
  {
    key: "writer",
    label: "Writer",
    description: "Polished drafts, summaries, explanations, user-facing content.",
    suggestedName: "Writing Agent",
    capabilities: ["drafting", "editing", "summarization", "copywriting"],
    systemPrompt: "Focus on structured writing, concise summaries, and clear user-facing explanations.",
    icon: "\u270D\uFE0F"
  },
  {
    key: "validator",
    label: "Validator",
    description: "Review, QA, critique, edge cases, catching mistakes.",
    suggestedName: "Validation Agent",
    capabilities: ["validation", "review", "qa", "risk checking"],
    systemPrompt: "Act as a critical reviewer. Focus on edge cases, quality checks, mistakes, and missing validation.",
    icon: "\u2705"
  },
  {
    key: "planner",
    label: "Planner",
    description: "Step-by-step plans, orchestration, converting ideas into workflows.",
    suggestedName: "Planning Agent",
    capabilities: ["planning", "workflow design", "sequencing", "coordination"],
    systemPrompt: "Focus on turning broad goals into ordered steps, milestones, and executable plans.",
    icon: "\uD83D\uDDFA\uFE0F"
  }
];

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

function boolText(value: unknown): string {
  if (typeof value === "boolean") return value ? "true" : "false";
  if (typeof value === "number") return value === 0 ? "false" : "true";
  if (typeof value === "string" && value.trim()) return value;
  return "false";
}

function errMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return "Request failed.";
}

function statusChipColor(status: string): "default" | "success" | "warning" | "error" {
  const normalized = status.trim().toLowerCase();
  if (normalized === "provisioned" || normalized === "idle" || normalized === "completed") return "success";
  if (normalized === "busy" || normalized === "running") return "warning";
  if (normalized === "offline" || normalized === "failed" || normalized === "cancelled") return "error";
  return "default";
}

function normalizeLifecycleStatus(status: unknown): string {
  const normalized = str(status, "").trim().toLowerCase();
  if (normalized === "busy" || normalized === "running") return "running";
  if (normalized === "completed" || normalized === "success") return "completed";
  if (normalized === "failed" || normalized === "error") return "failed";
  if (normalized === "cancelled" || normalized === "canceled") return "cancelled";
  if (normalized === "offline") return "offline";
  if (normalized === "disabled") return "disabled";
  if (normalized === "idle" || normalized === "provisioned") return "provisioned";
  return normalized || "provisioned";
}

function statusChipLabel(status: unknown): string {
  switch (normalizeLifecycleStatus(status)) {
    case "running":
      return "Running";
    case "completed":
      return "Completed";
    case "failed":
      return "Failed";
    case "cancelled":
      return "Cancelled";
    case "offline":
      return "Offline";
    case "disabled":
      return "Disabled";
    default:
      return "Provisioned";
  }
}

function formatTimestamp(value: unknown): string {
  const raw = str(value, "").trim();
  if (!raw) return "-";
  const parsed = new Date(raw);
  if (Number.isNaN(parsed.getTime())) return raw;
  return parsed.toLocaleString();
}

function compactChatId(value: string): string {
  const trimmed = value.trim();
  return trimmed ? trimmed.slice(0, 8) : "";
}

function parseCsv(value: string): string[] {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

function formatCapabilities(value: unknown): string[] {
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
    if (Array.isArray(parsed)) return formatCapabilities(parsed);
  } catch {
    return parseCsv(raw);
  }
  return [];
}

function providerHint(provider: string): string {
  switch (provider) {
    case "ollama":
      return "Local default. Use a local model such as `qwen2.5-coder:7b` or `llama3.1:8b`.";
    case "openai-compatible":
      return "Use this for local or hosted OpenAI-style endpoints. Set the base URL.";
    case "openrouter":
      return "Use this when you want a router-style hosted provider and will provide an API key.";
    case "anthropic":
      return "Use this for Claude-compatible specialist agents.";
    default:
      return "Choose the provider and model this specialist should use.";
  }
}

function presetIcon(agentType: string): string {
  const preset = AGENT_PRESETS.find((p) => p.key === agentType);
  return preset?.icon || "\uD83E\uDD16";
}

export function SwarmManager({ autoRefresh }: Props) {
  const queryClient = useQueryClient();
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [editingAgentId, setEditingAgentId] = useState<string | null>(null);
  const [dialogStep, setDialogStep] = useState<"preset" | "form">("preset");
  const [selectedPreset, setSelectedPreset] = useState<string>("researcher");
  const [name, setName] = useState("Research Lead");
  const [agentType, setAgentType] = useState("researcher");
  const [provider, setProvider] = useState("ollama");
  const [model, setModel] = useState("");
  const [baseUrl, setBaseUrl] = useState("http://localhost:11434");
  const [apiKey, setApiKey] = useState("");
  const [capabilitiesCsv, setCapabilitiesCsv] = useState("research, source review, comparison, fact gathering");
  const [systemPrompt, setSystemPrompt] = useState("Focus on finding facts, comparing sources, and delivering concise research notes with key evidence.");
  const [maxSpecialists, setMaxSpecialists] = useState("5");
  const [defaultTimeoutSecs, setDefaultTimeoutSecs] = useState("60");
  const [configHydrated, setConfigHydrated] = useState(false);
  const [showSettings, setShowSettings] = useState(false);

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

  const saveConfigMutation = useMutation({
    mutationFn: (payload: { max_specialists?: number; default_timeout_secs?: number }) =>
      api.rawPost("/swarm/config", payload),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["swarm-config"] });
      await queryClient.invalidateQueries({ queryKey: ["swarm-status"] });
    }
  });

  const addAgentMutation = useMutation({
    mutationFn: (payload: JsonRecord) => api.rawPost("/swarm/agents", payload),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["swarm-agents"] });
      await queryClient.invalidateQueries({ queryKey: ["swarm-status"] });
      await queryClient.invalidateQueries({ queryKey: ["swarm-delegations"] });
    }
  });
  const updateAgentMutation = useMutation({
    mutationFn: ({ id, payload }: { id: string; payload: JsonRecord }) =>
      api.rawPost(`/swarm/agents/${encodeURIComponent(id)}`, payload),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["swarm-agents"] });
      await queryClient.invalidateQueries({ queryKey: ["swarm-status"] });
    }
  });
  const removeAgentMutation = useMutation({
    mutationFn: (id: string) => api.rawDelete(`/swarm/agents/${encodeURIComponent(id)}`),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["swarm-agents"] });
      await queryClient.invalidateQueries({ queryKey: ["swarm-status"] });
      await queryClient.invalidateQueries({ queryKey: ["swarm-delegations"] });
    }
  });

  useEffect(() => {
    if (configHydrated) return;
    const config = asRecord(configQ.data);
    if (!Object.keys(config).length) return;
    setMaxSpecialists(String(num(config.max_specialists, 5)));
    setDefaultTimeoutSecs(String(num(config.default_timeout_secs, 60)));
    setConfigHydrated(true);
  }, [configHydrated, configQ.data]);

  const status = asRecord(statusQ.data);
  const config = asRecord(configQ.data);
  const agents = pickRecords(agentsQ.data, "agents");
  const delegations = pickRecords(delegationsQ.data, "delegations");
  const liveAgents = pickRecords(status.agents, "agents");
  const liveById = new Map(
    liveAgents.map((agent) => [str(agent.id, ""), normalizeLifecycleStatus(agent.status)])
  );
  const agentNameById = new Map(agents.map((agent) => [str(agent.id, ""), str(agent.name, "Agent")]));
  const swarmEnabled = boolText(status.enabled || config.enabled) === "true";
  const provisionedAgents = agents
    .map((agent) => {
      const id = str(agent.id, "");
      const enabled = boolText(agent.enabled) === "true";
      return {
        id,
        name: str(agent.name, "Agent"),
        type: str(agent.agent_type, "custom"),
        provider: str(agent.llm_provider, "ollama"),
        model: str(agent.llm_model, "-"),
        capabilities: formatCapabilities(agent.capabilities),
        createdAt: str(agent.created_at, ""),
        status: enabled
          ? liveById.get(id) || normalizeLifecycleStatus(agent.status)
          : "disabled"
      };
    })
    .sort((left, right) => {
      const leftTs = Date.parse(left.createdAt || "");
      const rightTs = Date.parse(right.createdAt || "");
      return (Number.isFinite(rightTs) ? rightTs : 0) - (Number.isFinite(leftTs) ? leftTs : 0);
    });
  const runningCount = provisionedAgents.filter((agent) => agent.status === "running").length;
  const delegatedAgentIds = new Set(
    delegations
      .map((row) => str(row.agent_id, "").trim())
      .filter((id) => id.length > 0)
  );
  const historyItems = [
    ...delegations.map((row) => {
      const completedAt = str(row.completed_at, "");
      const createdAt = str(row.created_at, "");
      const success = boolText(row.success) === "true";
      const resultText = str(row.result, "");
      const historicalStatus = !completedAt
        ? "running"
        : success
          ? "completed"
          : resultText.toLowerCase().includes("cancel")
            ? "cancelled"
            : "failed";
      const agentId = str(row.agent_id, "");
      const agentName = agentNameById.get(agentId) || agentId || "Agent";
      const duration = num(row.execution_time_ms, 0);
      const channel = str(row.channel, str(row.source, "")).trim();
      const chatId = str(row.chat_id, str(row.conversation_id, "")).trim();
      const currentStatus = liveById.get(agentId) || historicalStatus;
      const triggerParts: string[] = [];
      if (channel) triggerParts.push(channel);
      if (chatId) triggerParts.push(`chat ${compactChatId(chatId)}`);
      const triggerText =
        triggerParts.length > 0 ? `Triggered by ${triggerParts.join(" • ")}` : "Triggered internally";
      const workText = str(row.task, "Delegated task");
      const detailParts: string[] = [];
      if (duration > 0) detailParts.push(`${duration}ms`);
      if (createdAt && completedAt) detailParts.push(`finished ${formatTimestamp(completedAt)}`);
      const parts = detailParts;
      return {
        id: `delegation-${str(row.id, agentId || "history")}`,
        agentName,
        triggerText,
        workText,
        status: currentStatus,
        timestamp: completedAt || createdAt,
        detail: parts.length > 0 ? parts.join(" • ") : "Delegation run"
      };
    }),
    ...provisionedAgents
      .filter((agent) => !delegatedAgentIds.has(agent.id))
      .map((agent) => ({
        id: `provisioned-${agent.id}`,
        agentName: agent.name,
        triggerText: "Provisioned from swarm settings",
        workText: `${agent.type} specialist ready for delegation`,
        status: agent.status,
        timestamp: agent.createdAt,
        detail: `${agent.provider} / ${agent.model}`
      }))
  ].sort((left, right) => {
    const leftTs = Date.parse(left.timestamp || "");
    const rightTs = Date.parse(right.timestamp || "");
    return (Number.isFinite(rightTs) ? rightTs : 0) - (Number.isFinite(leftTs) ? leftTs : 0);
  });
  const completedCount = historyItems.filter((item) => item.status === "completed").length;
  const cancelledCount = historyItems.filter((item) => item.status === "cancelled").length;
  const failedCount = historyItems.filter((item) => item.status === "failed").length;

  const activeAgents = provisionedAgents.filter((a) => a.status === "running" || a.status === "provisioned");

  return (
    <Stack spacing={2.5}>
      <Stack
        direction={{ xs: "column", sm: "row" }}
        justifyContent="space-between"
        alignItems={{ xs: "flex-start", sm: "center" }}
        gap={1.5}
      >
        <Stack direction="row" spacing={1.5} alignItems="center">
          <Typography variant="h6" sx={{ fontWeight: 700 }}>
            Agents
          </Typography>
          <Chip
            size="small"
            color={swarmEnabled ? "success" : "default"}
            variant={swarmEnabled ? "filled" : "outlined"}
            label={swarmEnabled ? "Swarm enabled" : "Swarm disabled"}
          />
        </Stack>
        <Stack direction="row" spacing={1} alignItems="center">
          <Chip size="small" variant="outlined" label={`${runningCount} running`} color={runningCount > 0 ? "warning" : "default"} />
          <Chip size="small" variant="outlined" label={`${provisionedAgents.length} total`} />
          <Button size="small" variant="outlined" onClick={() => setHistoryOpen(true)} sx={{ textTransform: "none" }}>
            History ({historyItems.length})
          </Button>
        </Stack>
      </Stack>

      {statusQ.error || configQ.error || agentsQ.error || delegationsQ.error || error ? (
        <Alert severity="error">
          {error || errMessage(statusQ.error || configQ.error || agentsQ.error || delegationsQ.error)}
        </Alert>
      ) : null}

      {activeAgents.length === 0 ? (
        <Box
          sx={{
            p: { xs: 2, md: 2.5 },
            borderRadius: "14px",
            background:
              "linear-gradient(180deg, rgba(255,255,255,0.028) 0%, rgba(255,255,255,0.016) 100%)",
            border: "1px solid rgba(255,255,255,0.06)"
          }}
        >
          <Stack spacing={1.5}>
            <Stack
              direction={{ xs: "column", md: "row" }}
              justifyContent="space-between"
              alignItems={{ xs: "flex-start", md: "center" }}
              gap={1}
            >
              <Box>
                <Typography variant="h6" sx={{ fontWeight: 700 }}>
                  No active agents
                </Typography>
                <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5, maxWidth: 720 }}>
                  AgentArk only keeps internal agents visible here while they are provisioned or actively participating in
                  delegation. New agents appear automatically when a request needs parallel work, background coordination,
                  or isolated specialist context.
                </Typography>
              </Box>
              <Button size="small" variant="outlined" onClick={() => setHistoryOpen(true)} sx={{ textTransform: "none" }}>
                View history
              </Button>
            </Stack>

            <Stack direction="row" spacing={1} useFlexGap flexWrap="wrap">
              <Chip size="small" variant="outlined" label={`${provisionedAgents.length} total provisioned`} />
              <Chip size="small" variant="outlined" label={`${completedCount} completed`} color={completedCount > 0 ? "success" : "default"} />
              <Chip size="small" variant="outlined" label={`${failedCount} failed`} color={failedCount > 0 ? "error" : "default"} />
              <Chip size="small" variant="outlined" label={`${cancelledCount} cancelled`} color={cancelledCount > 0 ? "warning" : "default"} />
            </Stack>

            <Box
              sx={{
                px: 1.25,
                py: 1,
                borderRadius: "10px",
                background: "rgba(47, 212, 255, 0.05)",
                border: "1px solid rgba(47, 212, 255, 0.12)"
              }}
            >
              <Typography variant="caption" sx={{ color: "text.secondary", display: "block" }}>
                Ask in chat for monitoring, escalation, deep research, or multi-step execution. AgentArk decides when
                specialist agents are actually needed instead of keeping idle workers around.
              </Typography>
            </Box>
          </Stack>
        </Box>
      ) : (
        <Stack spacing={1}>
          {activeAgents.map((agent) => (
            <Box
              key={agent.id}
              sx={{
                p: 1.5,
                borderRadius: "10px",
                background: "rgba(255,255,255,0.02)",
                border: "1px solid rgba(255,255,255,0.05)"
              }}
            >
              <Stack
                direction={{ xs: "column", md: "row" }}
                justifyContent="space-between"
                alignItems={{ xs: "flex-start", md: "center" }}
                gap={1}
              >
                <Stack direction="row" spacing={1} alignItems="center" flexWrap="wrap" useFlexGap>
                  <Typography variant="body2" sx={{ fontWeight: 700 }}>{agent.name}</Typography>
                  <Chip size="small" color={statusChipColor(agent.status)} label={statusChipLabel(agent.status)} />
                  <Typography variant="caption" color="text.secondary">
                    {agent.provider} / {agent.model}
                  </Typography>
                </Stack>
                <Stack direction="row" spacing={0.5} useFlexGap flexWrap="wrap">
                  {agent.capabilities.slice(0, 5).map((cap) => (
                    <Chip key={`${agent.id}-${cap}`} size="small" variant="outlined" label={cap} sx={{ height: 20, fontSize: "0.65rem" }} />
                  ))}
                </Stack>
              </Stack>
            </Box>
          ))}
        </Stack>
      )}

      {/* ── History Dialog ── */}
      <Dialog
        open={historyOpen}
        onClose={() => setHistoryOpen(false)}
        maxWidth="md"
        fullWidth
        PaperProps={{
          sx: {
            background: "rgba(10, 15, 28, 0.97)",
            border: "1px solid rgba(47, 212, 255, 0.18)",
            backdropFilter: "blur(20px)",
          },
        }}
      >
        <DialogTitle>
          <Stack direction="row" justifyContent="space-between" alignItems="center">
            <Typography variant="h6" sx={{ fontWeight: 600 }}>Agent History</Typography>
            <IconButton size="small" onClick={() => setHistoryOpen(false)}><CloseIcon fontSize="small" /></IconButton>
          </Stack>
        </DialogTitle>
        <DialogContent dividers>
          {historyItems.length === 0 ? (
            <Typography variant="body2" color="text.secondary" sx={{ py: 3, textAlign: "center" }}>
              No agent history recorded yet.
            </Typography>
          ) : (
            <Stack spacing={0} divider={<Box sx={{ borderBottom: "1px solid rgba(62,143,214,0.10)" }} />}>
              {historyItems.map((item) => (
                <Box key={item.id} sx={{ py: 1 }}>
                  <Stack direction="row" spacing={1} alignItems="center" flexWrap="wrap" useFlexGap>
                    <Chip size="small" color={statusChipColor(item.status)} label={statusChipLabel(item.status)} sx={{ height: 20, fontSize: "0.65rem" }} />
                    <Typography variant="body2" sx={{ fontWeight: 600 }}>{item.agentName}</Typography>
                    <Typography variant="caption" color="text.secondary">
                      {item.triggerText} • {item.workText}
                    </Typography>
                    <Typography variant="caption" color="text.secondary" sx={{ ml: "auto !important" }}>{formatTimestamp(item.timestamp)}</Typography>
                  </Stack>
                  {item.detail ? <Typography variant="caption" color="text.secondary" sx={{ display: "block", mt: 0.25 }}>{item.detail}</Typography> : null}
                </Box>
              ))}
            </Stack>
          )}
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setHistoryOpen(false)}>Close</Button>
        </DialogActions>
      </Dialog>
    </Stack>
  );

  function applyPreset(presetKey: string) {
    const preset = AGENT_PRESETS.find((item) => item.key === presetKey);
    if (!preset) return;
    setSelectedPreset(preset.key);
    setAgentType(preset.key);
    setName(preset.suggestedName);
    setCapabilitiesCsv(preset.capabilities.join(", "));
    setSystemPrompt(preset.systemPrompt);
  }

  function resetAgentForm() {
    setEditingAgentId(null);
    setProvider("ollama");
    setModel("");
    setBaseUrl("http://localhost:11434");
    setApiKey("");
    applyPreset("researcher");
  }

  function openCreateDialog() {
    resetAgentForm();
    setDialogStep("preset");
    setDialogOpen(true);
  }

  function openEditDialog(agent: JsonRecord) {
    const nextType = str(agent.agent_type, "custom").toLowerCase();
    setEditingAgentId(str(agent.id, ""));
    setName(str(agent.name, ""));
    setAgentType(nextType || "custom");
    setSelectedPreset(AGENT_PRESETS.some((preset) => preset.key === nextType) ? nextType : "");
    setProvider(str(agent.llm_provider, "ollama"));
    setModel(str(agent.llm_model, ""));
    setBaseUrl(str(agent.llm_base_url, ""));
    setApiKey("");
    setCapabilitiesCsv(formatCapabilities(agent.capabilities).join(", "));
    setSystemPrompt(str(agent.system_prompt, ""));
    setDialogStep("form");
    setDialogOpen(true);
    setError(null);
    setSuccess(null);
  }

  async function handleSaveConfig() {
    setError(null);
    setSuccess(null);
    try {
      await saveConfigMutation.mutateAsync({
        max_specialists: Math.max(1, num(maxSpecialists, 5)),
        default_timeout_secs: Math.max(10, num(defaultTimeoutSecs, 60))
      });
      setSuccess("Multi-agent settings saved.");
    } catch (e) {
      setError(errMessage(e));
    }
  }

  async function handleSubmitAgent() {
    const trimmedName = name.trim();
    const trimmedModel = model.trim();
    if (!trimmedName) {
      setError("Agent name is required.");
      setSuccess(null);
      return;
    }
    if (!trimmedModel) {
      setError("Model is required.");
      setSuccess(null);
      return;
    }
    setError(null);
    setSuccess(null);
    const payload = {
      name: trimmedName,
      agent_type: agentType.trim() || "researcher",
      llm_provider: provider,
      llm_model: trimmedModel,
      llm_base_url: baseUrl.trim() || undefined,
      llm_api_key: apiKey.trim() || undefined,
      system_prompt: systemPrompt.trim() || undefined,
      capabilities: parseCsv(capabilitiesCsv)
    };
    try {
      const response = asRecord(
        editingAgentId
          ? await updateAgentMutation.mutateAsync({ id: editingAgentId, payload })
          : await addAgentMutation.mutateAsync(payload)
      );
      setSuccess(str(response.message, editingAgentId ? "Agent updated." : "Agent added."));
      setApiKey("");
      setDialogOpen(false);
      resetAgentForm();
    } catch (e) {
      setError(errMessage(e));
    }
  }

  async function handleRemoveAgent(id: string, nameValue: string) {
    if (!id) return;
    if (typeof window !== "undefined" && !window.confirm(`Remove specialist agent "${nameValue}"?`)) {
      return;
    }
    setError(null);
    setSuccess(null);
    try {
      const response = asRecord(await removeAgentMutation.mutateAsync(id));
      setSuccess(str(response.message, "Agent removed."));
    } catch (e) {
      setError(errMessage(e));
    }
  }

  return (
    <Stack spacing={2.5}>
      {/* ── Header ── */}
      <Stack direction="row" justifyContent="space-between" alignItems="flex-start" flexWrap="wrap" gap={2}>
        <Box>
          <Typography variant="h6" sx={{ fontWeight: 700, mb: 0.5 }}>Agent Orchestration</Typography>
          <Typography variant="body2" color="text.secondary" sx={{ maxWidth: 600 }}>
            AgentArk launches internal specialists automatically when a chat request, task, watcher, or escalation needs separate contexts, longer-running coordination, or capability isolation. Users should ask for the outcome they want; the framework decides whether extra agents are needed.
          </Typography>
        </Box>
        <Stack direction="row" spacing={1}>
          <Button
            variant="outlined"
            size="small"
            onClick={() => setShowSettings((v) => !v)}
          >
            {showSettings ? "Hide Settings" : "Settings"}
          </Button>
        </Stack>
      </Stack>

      {/* ── Stats ── */}
      <Box sx={{ display: "grid", gridTemplateColumns: { xs: "repeat(2, 1fr)", md: "repeat(4, 1fr)" }, gap: 1.5 }}>
        {[
          { label: "Swarm", value: boolText(status.enabled || config.enabled) === "true" ? "Enabled" : "Disabled", color: boolText(status.enabled || config.enabled) === "true" ? "#14f195" : "#666" },
          { label: "Total Agents", value: String(num(status.total_agents, agents.length)), color: "#2fd4ff" },
          { label: "Active", value: String(num(status.active_agents, 0)), color: "#ffb432" },
          { label: "Delegations", value: String(delegations.length), color: "rgba(180,210,240,0.7)" }
        ].map((stat) => (
          <Box key={stat.label} className="adv-group" sx={{ py: 1.5, px: 2, textAlign: "center" }}>
            <Typography variant="caption" color="text.secondary" sx={{ fontSize: "0.68rem", textTransform: "uppercase", letterSpacing: "0.08em" }}>
              {stat.label}
            </Typography>
            <Typography variant="h5" sx={{ fontWeight: 700, color: stat.color, fontFamily: "'Space Grotesk', monospace" }}>
              {stat.value}
            </Typography>
          </Box>
        ))}
      </Box>

      {/* ── Settings (collapsible) ── */}
      {showSettings ? (
        <Box className="adv-group">
          <Typography variant="body2" sx={{ fontWeight: 600, mb: 1.5 }}>Swarm Configuration</Typography>
          <Box sx={{ display: "grid", gridTemplateColumns: { xs: "1fr", sm: "1fr 1fr auto" }, gap: 1.5, alignItems: "flex-start" }}>
            <TextField
              size="small"
              type="number"
              label="Max specialists"
              value={maxSpecialists}
              onChange={(e) => setMaxSpecialists(e.target.value)}
              helperText="Maximum specialists per delegation."
            />
            <TextField
              size="small"
              type="number"
              label="Default timeout (seconds)"
              value={defaultTimeoutSecs}
              onChange={(e) => setDefaultTimeoutSecs(e.target.value)}
              helperText="Delegation timeout."
            />
            <Button variant="contained" onClick={handleSaveConfig} disabled={saveConfigMutation.isPending} sx={{ mt: 0.5 }}>
              Save
            </Button>
          </Box>
        </Box>
      ) : null}

      {/* ── Alerts ── */}
      {statusQ.error || configQ.error || agentsQ.error || delegationsQ.error || error ? (
        <Alert severity="error">
          {error || errMessage(statusQ.error || configQ.error || agentsQ.error || delegationsQ.error)}
        </Alert>
      ) : null}
      {success ? <Alert severity="success">{success}</Alert> : null}
      <Alert severity="info">
        Use chat to ask for monitoring, escalation, research, or multi-step follow-through. AgentArk will spawn internal specialists only when the request actually needs them.
      </Alert>

      {/* ── Agent Cards ── */}
      {agents.length === 0 ? (
        <Box className="adv-group" sx={{ textAlign: "center", py: 5 }}>
          <Typography variant="h6" color="text.secondary" sx={{ mb: 1, opacity: 0.5 }}>No internal specialists are active</Typography>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 2.5, maxWidth: 400, mx: "auto" }}>
            That is normal. Specialists should appear only when AgentArk decides a user request needs internal delegation or orchestration.
          </Typography>
        </Box>
      ) : (
        <Grid2 container spacing={2}>
          {agents.map((agent, idx) => {
            const caps = formatCapabilities(agent.capabilities);
            const statusText = str(agent.status, "idle");
            const agType = str(agent.agent_type, "custom").toLowerCase();
            const icon = presetIcon(agType);
            return (
              <Grid2 key={str(agent.id, `agent-${idx}`)} size={{ xs: 12, sm: 6, lg: 4 }}>
                <Box className="adv-group" sx={{
                  height: "100%",
                  display: "flex",
                  flexDirection: "column",
                  transition: "border-color 0.22s, transform 0.18s",
                  "&:hover": { borderColor: "rgba(47, 212, 255, 0.35)", transform: "translateY(-2px)" }
                }}>
                  <Stack direction="row" justifyContent="space-between" alignItems="flex-start" mb={1.5}>
                    <Stack direction="row" spacing={1.2} alignItems="center">
                      <Box sx={{
                        width: 36, height: 36, borderRadius: "10px",
                        display: "flex", alignItems: "center", justifyContent: "center",
                        background: "rgba(47, 212, 255, 0.08)",
                        border: "1px solid rgba(47, 212, 255, 0.18)",
                        fontSize: 18
                      }}>
                        {icon}
                      </Box>
                      <Box>
                        <Typography variant="body2" sx={{ fontWeight: 700, lineHeight: 1.2 }}>
                          {str(agent.name, "Agent")}
                        </Typography>
                        <Typography variant="caption" color="text.secondary" sx={{ fontSize: "0.68rem", textTransform: "uppercase", letterSpacing: "0.06em" }}>
                          {str(agent.agent_type, "custom")}
                        </Typography>
                      </Box>
                    </Stack>
                    <Chip size="small" label={statusText} color={statusChipColor(statusText)} />
                  </Stack>

                  <Typography variant="caption" color="text.secondary" sx={{ mb: 1, display: "block" }}>
                    {str(agent.llm_provider, "ollama")} / {str(agent.llm_model, "-")}
                  </Typography>

                  <Stack direction="row" spacing={0.5} useFlexGap flexWrap="wrap" sx={{ mb: 1.5, flex: 1 }}>
                    {caps.slice(0, 5).map((cap) => (
                      <Chip key={`${str(agent.id, `a-${idx}`)}-${cap}`} size="small" variant="outlined" label={cap} />
                    ))}
                    {caps.length > 5 ? <Chip size="small" variant="outlined" label={`+${caps.length - 5}`} /> : null}
                  </Stack>

                  <Stack direction="row" spacing={1} justifyContent="flex-end">
                    <Chip size="small" variant="outlined" label="Internal specialist" />
                  </Stack>
                </Box>
              </Grid2>
            );
          })}
        </Grid2>
      )}

      {/* ── Recent Delegations ── */}
      {delegations.length > 0 ? (
        <Box className="adv-group">
          <Typography variant="body2" sx={{ fontWeight: 600, mb: 1.5 }}>Recent Delegations</Typography>
          <Stack spacing={1}>
            {delegations.slice(0, 10).map((row, idx) => {
              const successText = boolText(row.success);
              const isSuccess = successText === "true";
              return (
                <Box key={str(row.id, `d-${idx}`)} sx={{
                  display: "flex",
                  alignItems: "center",
                  gap: 1.5,
                  padding: "8px 12px",
                  borderRadius: "8px",
                  background: "rgba(255,255,255,0.02)",
                  border: "1px solid rgba(255,255,255,0.04)"
                }}>
                  <Chip size="small" color={isSuccess ? "success" : "error"} label={isSuccess ? "OK" : "Fail"} sx={{ minWidth: 44 }} />
                  <Typography variant="body2" sx={{ fontWeight: 600, flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {str(row.task, "Delegated task")}
                  </Typography>
                  <Chip size="small" variant="outlined" label={`${num(row.execution_time_ms, 0)}ms`} />
                  <Typography variant="caption" color="text.secondary" sx={{ whiteSpace: "nowrap" }}>
                    {str(row.created_at, "")}
                  </Typography>
                </Box>
              );
            })}
          </Stack>
        </Box>
      ) : null}

      {/* ── Create / Edit Agent Dialog ── */}
      <Dialog
        open={dialogOpen}
        onClose={() => { setDialogOpen(false); resetAgentForm(); }}
        maxWidth="sm"
        fullWidth
        PaperProps={{
          sx: {
            background: "linear-gradient(155deg, rgba(9, 21, 39, 0.97), rgba(6, 14, 28, 0.95))",
            border: "1px solid rgba(91, 149, 201, 0.22)",
            borderRadius: "14px",
            backdropFilter: "blur(16px)"
          }
        }}
      >
        <DialogTitle sx={{ display: "flex", justifyContent: "space-between", alignItems: "center", pb: 0.5 }}>
          <Typography variant="h6" sx={{ fontWeight: 700 }}>
            {editingAgentId ? "Edit Agent" : dialogStep === "preset" ? "Choose Agent Type" : "Configure Agent"}
          </Typography>
          <IconButton size="small" onClick={() => { setDialogOpen(false); resetAgentForm(); }}>
            <CloseIcon fontSize="small" />
          </IconButton>
        </DialogTitle>

        <DialogContent sx={{ pt: 2 }}>
          {dialogStep === "preset" && !editingAgentId ? (
            <Stack spacing={1.5}>
              <Typography variant="body2" color="text.secondary" sx={{ mb: 0.5 }}>
                Pick a role to pre-fill the form, or choose Custom.
              </Typography>
              <Grid2 container spacing={1}>
                {AGENT_PRESETS.map((preset) => {
                  const selected = selectedPreset === preset.key;
                  return (
                    <Grid2 key={preset.key} size={{ xs: 6 }}>
                      <Box
                        onClick={() => {
                          applyPreset(preset.key);
                          setDialogStep("form");
                        }}
                        sx={{
                          border: selected ? "1px solid rgba(47,212,255,0.45)" : "1px solid rgba(255,255,255,0.08)",
                          borderRadius: "10px",
                          p: 1.5,
                          cursor: "pointer",
                          background: selected ? "rgba(47,212,255,0.08)" : "rgba(255,255,255,0.02)",
                          transition: "border-color 0.18s, background 0.18s, transform 0.18s",
                          "&:hover": { borderColor: "rgba(47,212,255,0.35)", background: "rgba(47,212,255,0.06)", transform: "translateY(-1px)" }
                        }}
                      >
                        <Typography sx={{ fontSize: 22, mb: 0.5 }}>{preset.icon}</Typography>
                        <Typography variant="body2" sx={{ fontWeight: 700 }}>{preset.label}</Typography>
                        <Typography variant="caption" color="text.secondary" sx={{ fontSize: "0.68rem" }}>
                          {preset.description}
                        </Typography>
                      </Box>
                    </Grid2>
                  );
                })}
                <Grid2 size={{ xs: 6 }}>
                  <Box
                    onClick={() => {
                      setAgentType("custom");
                      setSelectedPreset("");
                      setName("");
                      setCapabilitiesCsv("");
                      setSystemPrompt("");
                      setDialogStep("form");
                    }}
                    sx={{
                      border: "1px solid rgba(255,255,255,0.08)",
                      borderRadius: "10px",
                      p: 1.5,
                      cursor: "pointer",
                      background: "rgba(255,255,255,0.02)",
                      transition: "border-color 0.18s, background 0.18s",
                      "&:hover": { borderColor: "rgba(255,255,255,0.2)", background: "rgba(255,255,255,0.04)" }
                    }}
                  >
                    <Typography sx={{ fontSize: 22, mb: 0.5 }}>{"\uD83E\uDD16"}</Typography>
                    <Typography variant="body2" sx={{ fontWeight: 700 }}>Custom</Typography>
                    <Typography variant="caption" color="text.secondary" sx={{ fontSize: "0.68rem" }}>
                      Start from scratch with your own config.
                    </Typography>
                  </Box>
                </Grid2>
              </Grid2>
            </Stack>
          ) : (
            <Stack spacing={2} sx={{ mt: 1 }}>
              {editingAgentId ? (
                <Alert severity="info" sx={{ py: 0.5 }}>
                  Leave API key blank to keep the existing saved key.
                </Alert>
              ) : null}
              <Box sx={{ display: "grid", gridTemplateColumns: { xs: "1fr", sm: "1fr 1fr" }, gap: 2 }}>
                <TextField
                  size="small"
                  label="Agent name"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="e.g. Research Lead"
                />
                <TextField
                  select
                  size="small"
                  label="Agent type"
                  value={agentType}
                  onChange={(e) => {
                    const next = e.target.value;
                    setAgentType(next);
                    if (AGENT_PRESETS.some((preset) => preset.key === next)) {
                      applyPreset(next);
                    }
                  }}
                >
                  {AGENT_PRESETS.map((preset) => (
                    <MenuItem key={preset.key} value={preset.key}>{preset.icon} {preset.label}</MenuItem>
                  ))}
                  <MenuItem value="custom">{"\uD83E\uDD16"} Custom</MenuItem>
                </TextField>
                <TextField
                  select
                  size="small"
                  label="Provider"
                  value={provider}
                  onChange={(e) => {
                    const next = e.target.value;
                    setProvider(next);
                    if (next === "ollama" && !baseUrl.trim()) setBaseUrl("http://localhost:11434");
                  }}
                  helperText={providerHint(provider)}
                >
                  <MenuItem value="ollama">Ollama</MenuItem>
                  <MenuItem value="openai-compatible">OpenAI-compatible</MenuItem>
                  <MenuItem value="openai">OpenAI</MenuItem>
                  <MenuItem value="anthropic">Anthropic</MenuItem>
                  <MenuItem value="openrouter">OpenRouter</MenuItem>
                </TextField>
                <TextField
                  size="small"
                  label="Model"
                  value={model}
                  onChange={(e) => setModel(e.target.value)}
                  placeholder="e.g. gpt-5-mini, qwen2.5-coder:7b"
                />
                <TextField
                  size="small"
                  label="Base URL"
                  value={baseUrl}
                  onChange={(e) => setBaseUrl(e.target.value)}
                  placeholder="http://localhost:11434"
                />
                <TextField
                  size="small"
                  type="password"
                  label="API key"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder="Optional"
                />
              </Box>
              <TextField
                size="small"
                multiline
                minRows={2}
                label="System prompt"
                value={systemPrompt}
                onChange={(e) => setSystemPrompt(e.target.value)}
                placeholder="Optional. Narrow the role further."
              />
              <TextField
                size="small"
                label="Capabilities"
                value={capabilitiesCsv}
                onChange={(e) => setCapabilitiesCsv(e.target.value)}
                placeholder="Comma-separated keywords"
                helperText="Keywords the swarm router uses when deciding which agent to delegate to."
              />
              {error ? <Alert severity="error">{error}</Alert> : null}
            </Stack>
          )}
        </DialogContent>

        <DialogActions sx={{ px: 3, pb: 2 }}>
          {dialogStep === "form" && !editingAgentId ? (
            <Button onClick={() => setDialogStep("preset")} sx={{ mr: "auto" }}>
              Back
            </Button>
          ) : null}
          <Button onClick={() => { setDialogOpen(false); resetAgentForm(); }}>
            Cancel
          </Button>
          {dialogStep === "form" ? (
            <Button
              variant="contained"
              onClick={handleSubmitAgent}
              disabled={addAgentMutation.isPending || updateAgentMutation.isPending}
            >
              {editingAgentId ? "Save Changes" : "Add Agent"}
            </Button>
          ) : null}
        </DialogActions>
      </Dialog>
    </Stack>
  );
}
