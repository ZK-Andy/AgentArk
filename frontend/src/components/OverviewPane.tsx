import {
  Alert,
  Box,
  Button,
  Chip,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Grid2,
  Stack,
  Typography,
} from "@mui/material";
import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/client";
import { useUiStore } from "../store/uiStore";
import { AgentStatusBar } from "./AgentStatusBar";
import { WelcomeHero } from "./WelcomeHero";
import { NeedsAttentionInbox } from "./NeedsAttentionInbox";
import { TodaysHighlights } from "./TodaysHighlights";
import { SmartSuggestions } from "./SmartSuggestions";
import { QuickActionsStrip } from "./QuickActionsStrip";
import { ActivityFeed } from "./ActivityFeed";
import type { RecommendedSkill } from "../types";

const REFRESH_MS = 8000;
type PausePhase = "idle" | "stopping" | "stopped" | "resuming" | "resumed";

function asRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" ? (value as Record<string, unknown>) : {};
}

function errMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) return error.message;
  return "Failed to update pause state.";
}

type Props = {
  navigateToView: (view: string, replace?: boolean) => void;
  serverStatus?: { at: number; rtt_ms: number; status: import("../types").StatusResponse };
  serverError: boolean;
  serverLoading: boolean;
};

export function OverviewPane({ navigateToView, serverStatus, serverError, serverLoading }: Props) {
  const queryClient = useQueryClient();
  const autoRefresh = useUiStore((s) => s.autoRefresh);
  const interval = autoRefresh ? REFRESH_MS : false;
  const [pauseDialogOpen, setPauseDialogOpen] = useState(false);
  const [pausePhase, setPausePhase] = useState<PausePhase>("idle");
  const [pauseError, setPauseError] = useState<string | null>(null);
  const [pauseTarget, setPauseTarget] = useState<"pause" | "resume" | null>(null);

  // --- Data hooks ---
  const tasksQ = useQuery({ queryKey: ["tasks"], queryFn: api.getTasks, refetchInterval: interval });
  const traceQ = useQuery({ queryKey: ["trace"], queryFn: api.getTrace, refetchInterval: interval });
  const briefingQ = useQuery({ queryKey: ["briefing"], queryFn: api.getBriefing, refetchInterval: interval });
  const nudgesQ = useQuery({ queryKey: ["nudges"], queryFn: api.getNudges, refetchInterval: interval });
  const notificationsQ = useQuery({ queryKey: ["notifications"], queryFn: api.getNotifications, refetchInterval: interval });
  const securityQ = useQuery({
    queryKey: ["security-logs-dashboard"],
    queryFn: () => api.getSecurityLogs(5),
    refetchInterval: autoRefresh ? 30_000 : false,
  });
  const settingsQ = useQuery({
    queryKey: ["settings-dashboard"],
    queryFn: api.getSettings,
    refetchInterval: false,
    staleTime: 60_000,
  });
  const autonomySettingsQ = useQuery({
    queryKey: ["autonomy-settings-dashboard"],
    queryFn: () => api.rawGet("/autonomy/settings"),
    refetchInterval: interval,
    staleTime: 10_000,
  });

  // --- Derived data ---
  const tasks = Array.isArray(tasksQ.data) ? tasksQ.data : [];
  const traces = traceQ.data?.history || [];
  const notifications = Array.isArray(notificationsQ.data) ? notificationsQ.data : [];
  const securityLogs = (securityQ.data as { logs?: Array<{ event_type: string; severity: string; message: string }> })?.logs || [];
  const nudges = nudgesQ.data?.nudges || [];

  const currentTask = useMemo(() => {
    const inProgress = tasks.find((t) => {
      const s = String(t?.status || "").toLowerCase();
      return s.includes("inprogress");
    });
    return inProgress?.description;
  }, [tasks]);

  // Check if LLM is configured from settings
  const hasLlmConfigured = useMemo(() => {
    if (!settingsQ.data) return true; // Assume OK while loading
    const settings = settingsQ.data as Record<string, unknown>;
    // Check various possible fields for LLM configuration
    const pool = settings.model_pool || settings.llm_pool || settings.models;
    if (Array.isArray(pool)) return pool.length > 0;
    const provider = settings.llm_provider || settings.provider;
    if (provider && String(provider).trim()) return true;
    const apiKey = settings.openai_api_key || settings.anthropic_api_key || settings.api_key;
    if (apiKey && String(apiKey).trim()) return true;
    // If we got settings but no LLM-related fields exist, it might be structured differently
    // Be conservative: only flag if settings loaded successfully and look clearly empty
    return Object.keys(settings).length === 0 ? false : true;
  }, [settingsQ.data]);

  const autonomySettings = useMemo(() => {
    const root = asRecord(autonomySettingsQ.data);
    return asRecord(root.settings);
  }, [autonomySettingsQ.data]);

  const agentPaused = Boolean(autonomySettings.agent_paused ?? false);
  const pauseScopeItems = [
    "Scheduled tasks",
    "Watchers",
    "ArkPulse runs",
    "Autopilot/background analysis",
    "Proactive outbound notifications",
  ];

  // --- Mutations ---
  const approveMutation = useMutation({
    mutationFn: api.approveTask,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["tasks"] }),
  });
  const rejectMutation = useMutation({
    mutationFn: api.rejectTask,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["tasks"] }),
  });
  const retryMutation = useMutation({
    mutationFn: api.retryTask,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["tasks"] }),
  });
  const executeSkillMutation = useMutation({
    mutationFn: api.executeRecommendedSkill,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["nudges"] });
      await queryClient.invalidateQueries({ queryKey: ["briefing"] });
      await queryClient.invalidateQueries({ queryKey: ["tasks"] });
      await queryClient.invalidateQueries({ queryKey: ["trace"] });
    },
  });
  const nudgeFeedbackMutation = useMutation({
    mutationFn: ({ id, action }: { id: string; action: "dismiss" | "snooze" }) =>
      api.feedbackNudge(id, { action, snooze_minutes: action === "snooze" ? 24 * 60 : undefined }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["nudges"] }),
  });
  const runBriefingMutation = useMutation({
    mutationFn: () =>
      api.executeRecommendedSkill({
        id: "daily_brief_now",
        title: "Run Daily Brief",
        skill_kind: "daily_brief_now",
        payload: {},
      } as RecommendedSkill),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["briefing"] });
      await queryClient.invalidateQueries({ queryKey: ["tasks"] });
      await queryClient.invalidateQueries({ queryKey: ["trace"] });
    },
  });
  const pauseMutation = useMutation({
    mutationFn: (nextPaused: boolean) =>
      api.rawPost("/autonomy/settings", {
        agent_paused: nextPaused,
        pause_mode: "autonomous_only",
      }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["autonomy-settings-dashboard"] });
      await queryClient.invalidateQueries({ queryKey: ["autonomy-settings"] });
      await queryClient.invalidateQueries({ queryKey: ["briefing"] });
      await queryClient.invalidateQueries({ queryKey: ["tasks"] });
      await queryClient.invalidateQueries({ queryKey: ["trace"] });
    },
  });

  async function handleTogglePause() {
    if (pauseMutation.isPending) return;
    const nextPaused = !agentPaused;
    setPauseDialogOpen(true);
    setPauseError(null);
    setPauseTarget(nextPaused ? "pause" : "resume");
    setPausePhase(nextPaused ? "stopping" : "resuming");
    try {
      await pauseMutation.mutateAsync(nextPaused);
      setPausePhase(nextPaused ? "stopped" : "resumed");
    } catch (error) {
      setPauseError(errMessage(error));
      setPausePhase("idle");
    }
  }

  const hasErrors = !!(tasksQ.error || traceQ.error || briefingQ.error || autonomySettingsQ.error);

  return (
    <Box
      data-tour-target="overview-dashboard"
      sx={{
        height: "auto",
        p: { xs: 1, md: 1.25 },
        overflow: "visible",
        display: "flex",
        flexDirection: "column",
        gap: 1.25,
      }}
    >
      <Box data-tour-target="welcome-hero">
        <WelcomeHero onGoChat={() => navigateToView("chat")} />
      </Box>

      <AgentStatusBar
        serverStatus={serverStatus}
        serverError={serverError}
        serverLoading={serverLoading}
        currentTaskDesc={currentTask}
      />

      <NeedsAttentionInbox
        tasks={tasks}
        notifications={notifications}
        securityLogs={securityLogs}
        settingsLoaded={!settingsQ.isLoading}
        hasLlmConfigured={hasLlmConfigured}
        onApprove={(id) => approveMutation.mutate(id)}
        onReject={(id) => rejectMutation.mutate(id)}
        onRetry={(id) => retryMutation.mutate(id)}
        onNavigate={navigateToView}
        approving={approveMutation.isPending}
        rejecting={rejectMutation.isPending}
        retrying={retryMutation.isPending}
      />

      <Grid2 container spacing={1.2} alignItems="stretch">
        <Grid2 size={{ xs: 12, lg: 7 }}>
          <TodaysHighlights tasks={tasks} traces={traces} />
        </Grid2>
        <Grid2 size={{ xs: 12, lg: 5 }}>
          <SmartSuggestions
            briefing={briefingQ.data}
            nudges={nudges}
            onExecuteSkill={(skill) => executeSkillMutation.mutate(skill)}
            onSnooze={(id) => nudgeFeedbackMutation.mutate({ id, action: "snooze" })}
            onDismiss={(id) => nudgeFeedbackMutation.mutate({ id, action: "dismiss" })}
            executing={executeSkillMutation.isPending}
            feedbackPending={nudgeFeedbackMutation.isPending}
          />
        </Grid2>
      </Grid2>

      <QuickActionsStrip
        agentPaused={agentPaused}
        onAskAgent={() => navigateToView("chat")}
        onRunBriefing={() => runBriefingMutation.mutate()}
        onTogglePause={() => {
          void handleTogglePause();
        }}
        onViewTasks={() => navigateToView("skills")}
        briefingLoading={runBriefingMutation.isPending}
        pauseLoading={pauseMutation.isPending}
      />

      <ActivityFeed
        traces={traces}
        onViewAll={() => navigateToView("settings")}
      />

      {hasErrors ? (
        <Alert severity="error">
          One or more data sources failed to load. Retrying automatically.
        </Alert>
      ) : null}

      <Dialog
        open={pauseDialogOpen}
        onClose={() => {
          if (pauseMutation.isPending) return;
          setPauseDialogOpen(false);
          setPauseError(null);
          setPausePhase("idle");
          setPauseTarget(null);
        }}
        maxWidth="sm"
        fullWidth
      >
        <DialogTitle>
          {pausePhase === "stopping" && "Pausing Agent"}
          {pausePhase === "stopped" && "Agent Paused"}
          {pausePhase === "resuming" && "Resuming Agent"}
          {pausePhase === "resumed" && "Agent Resumed"}
          {pausePhase === "idle" && (pauseTarget === "resume" ? "Resume Agent" : "Pause Agent")}
        </DialogTitle>
        <DialogContent dividers>
          <Stack spacing={1.25}>
            {pausePhase === "stopping" || pausePhase === "resuming" ? (
              <Stack direction="row" spacing={1} alignItems="center">
                <CircularProgress size={18} />
                <Typography variant="body2">
                  {pausePhase === "stopping"
                    ? "stopping: disabling autonomous background activity..."
                    : "resuming: re-enabling autonomous background activity..."}
                </Typography>
              </Stack>
            ) : null}

            {pausePhase === "stopped" || pausePhase === "resumed" ? (
              <Chip
                size="small"
                color="success"
                label={pausePhase === "stopped" ? "stopped" : "resumed"}
                sx={{ width: "fit-content" }}
              />
            ) : null}

            <Typography variant="body2" color="text.secondary">
              {pauseTarget === "pause"
                ? "When paused, these systems are suspended:"
                : "On resume, these systems are active again:"}
            </Typography>

            <Stack spacing={0.5}>
              {pauseScopeItems.map((item) => (
                <Typography key={item} variant="body2">
                  - {item}
                </Typography>
              ))}
            </Stack>

            {pauseError ? <Alert severity="error">{pauseError}</Alert> : null}
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button
            onClick={() => {
              setPauseDialogOpen(false);
              setPauseError(null);
              setPausePhase("idle");
              setPauseTarget(null);
            }}
            disabled={pauseMutation.isPending}
          >
            Close
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
