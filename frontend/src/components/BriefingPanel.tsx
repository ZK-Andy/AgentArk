import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Alert,
  Button,
  Card,
  CardContent,
  Chip,
  Divider,
  Grid2,
  Stack,
  Typography
} from "@mui/material";
import type { BriefingResponse, RecommendedSkill } from "../types";
import { api } from "../api/client";
import { useState } from "react";

type Props = {
  briefing?: BriefingResponse;
  compact?: boolean;
};

export function BriefingPanel({ briefing, compact = false }: Props) {
  const queryClient = useQueryClient();
  const [execNotice, setExecNotice] = useState<{ kind: "success" | "error" | "info"; text: string } | null>(null);
  const actionableRisks = (briefing?.top_risks || []).filter((risk) => {
    const typedRisk = risk as Record<string, unknown>;
    const type = String(typedRisk.type || "").toLowerCase();
    const title = String(typedRisk.title || "").toLowerCase();
    return type !== "none" && title !== "no critical risks detected";
  });
  const visibleOpportunities = briefing?.top_opportunities || [];
  const visibleSkills: RecommendedSkill[] =
    briefing?.recommended_skills ||
    (((briefing as unknown as { recommended_actions?: RecommendedSkill[] })?.recommended_actions || []) as RecommendedSkill[]);
  const showSignalRow = actionableRisks.length > 0 || visibleOpportunities.length > 0;

  function asErrorMessage(err: unknown): string {
    if (!(err instanceof Error)) return "Request failed";
    const raw = err.message || "Request failed";
    try {
      const parsed = JSON.parse(raw) as { error?: string; message?: string };
      if (parsed.error && parsed.error.trim()) return parsed.error;
      if (parsed.message && parsed.message.trim()) return parsed.message;
    } catch {
      // ignore
    }
    return raw;
  }

  function summarizeExecResult(payload: unknown): string {
    const obj = (payload && typeof payload === "object") ? (payload as Record<string, unknown>) : {};
    const result = (obj.result && typeof obj.result === "object") ? (obj.result as Record<string, unknown>) : obj;
    const kind = String(result.kind || "");
    if (kind === "daily_brief_now") return "Daily Command Brief generated and pushed to your preferred channel.";
    if (kind === "create_task") return `Task queued: ${String(result.task_id || "") || "created"}.`;
    if (kind === "delegate") return "Delegation completed. Check Swarm for details.";
    if (String(result.status || "").includes("queued_for_approval")) return "Queued for approval. Review it in Tasks.";
    return "Executed.";
  }

  const executeSkill = useMutation({
    mutationFn: api.executeRecommendedSkill,
    onSuccess: async (out) => {
      setExecNotice({ kind: "success", text: summarizeExecResult(out) });
      await queryClient.invalidateQueries({ queryKey: ["nudges"] });
      await queryClient.invalidateQueries({ queryKey: ["briefing"] });
      await queryClient.invalidateQueries({ queryKey: ["status"] });
      await queryClient.invalidateQueries({ queryKey: ["tasks"] });
      await queryClient.invalidateQueries({ queryKey: ["trace"] });
    },
    onError: (err) => {
      setExecNotice({ kind: "error", text: asErrorMessage(err) });
    }
  });
  const nudgesQ = useQuery({
    queryKey: ["nudges"],
    queryFn: api.getNudges
  });
  const nudgeFeedback = useMutation({
    mutationFn: ({
      id,
      action,
      snoozeMinutes
    }: {
      id: string;
      action: "dismiss" | "snooze" | "interested" | "reset";
      snoozeMinutes?: number;
    }) =>
      api.feedbackNudge(id, {
        action,
        snooze_minutes: snoozeMinutes
      }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["nudges"] });
    }
  });
  const planNudges = useMutation({
    mutationFn: api.planNudges,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["nudges"] });
      await queryClient.invalidateQueries({ queryKey: ["tasks"] });
    }
  });

  if (!briefing) {
    return (
      <Card sx={compact ? { minHeight: 0 } : undefined}>
        <CardContent sx={compact ? { p: 1.25 } : undefined}>
          <Typography variant="h6">Daily Command Brief</Typography>
          <Typography color="text.secondary" mt={1}>
            Waiting for briefing data...
          </Typography>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card sx={compact ? { minHeight: 0 } : undefined}>
      <CardContent
        sx={
          compact
            ? {
                p: 1.25,
                overflow: "auto"
              }
            : undefined
        }
      >
        <Stack direction="row" justifyContent="space-between" alignItems="center" mb={1.5}>
          <Typography variant="h6">Daily Command Brief</Typography>
          <Chip size="small" label={briefing.scope.toUpperCase()} />
        </Stack>
        {showSignalRow ? (
          <Grid2 container spacing={2}>
            {actionableRisks.length > 0 ? (
              <Grid2 size={{ xs: 12, md: visibleOpportunities.length > 0 ? 6 : 12 }}>
                <Typography variant="subtitle2" color="warning.main" mb={1}>
                  Top Risks
                </Typography>
                <Stack spacing={1}>
                  {actionableRisks.slice(0, compact ? 2 : 3).map((risk, idx) => (
                    <Alert key={idx} severity="warning" variant="outlined">
                      {risk.title || "Risk"}: {risk.summary || risk.detail || "No summary"}
                    </Alert>
                  ))}
                </Stack>
              </Grid2>
            ) : null}
            {visibleOpportunities.length > 0 ? (
              <Grid2 size={{ xs: 12, md: actionableRisks.length > 0 ? 6 : 12 }}>
                <Typography variant="subtitle2" color="success.main" mb={1}>
                  Top Opportunities
                </Typography>
                <Stack spacing={1}>
                  {visibleOpportunities.slice(0, compact ? 2 : 3).map((opp, idx) => (
                    <Alert key={idx} severity="success" variant="outlined">
                      {opp.title || "Opportunity"}: {opp.summary || opp.detail || "No summary"}
                    </Alert>
                  ))}
                </Stack>
              </Grid2>
            ) : null}
          </Grid2>
        ) : null}

        {showSignalRow ? <Divider sx={{ my: 2 }} /> : null}

        {visibleSkills.length > 0 ? (
          <Typography variant="subtitle2" mb={1}>
            Recommended Skills
          </Typography>
        ) : null}
        {execNotice ? <Alert severity={execNotice.kind}>{execNotice.text}</Alert> : null}
        <Stack spacing={1}>
          {visibleSkills.slice(0, compact ? 2 : 3).map((skill) => (
            <Stack
              key={skill.id}
              direction={{ xs: "column", md: "row" }}
              spacing={1}
              justifyContent="space-between"
              alignItems={{ xs: "flex-start", md: "center" }}
              className="action-row"
            >
              <Stack spacing={0.3}>
                <Typography variant="body2" fontWeight={700}>
                  {skill.title}
                </Typography>
                <Typography variant="caption" color="text.secondary">
                  {skill.summary || skill.description || "No description"}
                </Typography>
              </Stack>
              <Button
                variant="contained"
                size="small"
                disabled={executeSkill.isPending}
                onClick={() => executeSkill.mutate(skill)}
              >
                {executeSkill.isPending ? "Executing..." : "Execute"}
              </Button>
            </Stack>
          ))}
        </Stack>
        {!compact ? (
          <>
            <Divider sx={{ my: 2 }} />
            <Stack direction="row" alignItems="center" justifyContent="space-between" mb={1}>
              <Typography variant="subtitle2">What To Improve Now</Typography>
              <Button
                variant="outlined"
                size="small"
                disabled={planNudges.isPending}
                onClick={() => planNudges.mutate({ max_items: 3, dry_run: false })}
              >
                {planNudges.isPending ? "Planning..." : "Plan Top 3"}
              </Button>
            </Stack>
            {nudgesQ.error ? (
              <Alert severity="error">
                Failed to load predictive nudges.
              </Alert>
            ) : null}
            <Stack spacing={1}>
              {(nudgesQ.data?.nudges || []).slice(0, 6).map((nudge) => (
                <Stack
                  key={nudge.id}
                  direction={{ xs: "column", md: "row" }}
                  spacing={1}
                  justifyContent="space-between"
                  alignItems={{ xs: "flex-start", md: "center" }}
                  className="action-row"
                >
              <Stack spacing={0.3}>
                <Stack direction="row" spacing={0.8} alignItems="center" flexWrap="wrap">
                  <Typography variant="body2" fontWeight={700}>
                    {nudge.title}
                  </Typography>
                  <Chip size="small" label={`P${nudge.priority}`} color={nudge.priority >= 4 ? "warning" : "default"} />
                  <Chip size="small" label={`${Math.round((nudge.confidence || 0) * 100)}%`} />
                </Stack>
                <Typography variant="caption" color="text.secondary">
                  {nudge.detail}
                </Typography>
                {nudge.memory_clues && nudge.memory_clues.length > 0 ? (
                  <Stack spacing={0.4} mt={0.5}>
                    {nudge.memory_clues.map((clue) => (
                      <Typography key={clue.id} variant="caption" color="text.secondary">
                        {clue.memory_type} memory
                        {clue.channel ? ` from ${clue.channel}` : ""}
                        {" "}
                        at {new Date(clue.timestamp).toLocaleString()} - {clue.summary}
                        {" "}
                        ({Math.round((clue.importance || 0) * 100)}%)
                      </Typography>
                    ))}
                  </Stack>
                ) : null}
              </Stack>
                  <Stack direction="row" spacing={0.8}>
                    <Button
                      size="small"
                      variant="contained"
                      disabled={executeSkill.isPending || !(nudge.recommended_skill || (nudge as unknown as { recommended_action?: unknown }).recommended_action)}
                      onClick={() => {
                        const skill =
                          nudge.recommended_skill ||
                          ((nudge as unknown as { recommended_action?: unknown }).recommended_action as Record<string, unknown> | undefined);
                        if (!skill) return;
                        executeSkill.mutate(skill as never);
                      }}
                    >
                      Run
                    </Button>
                    <Button
                      size="small"
                      variant="outlined"
                      disabled={nudgeFeedback.isPending}
                      onClick={() =>
                        nudgeFeedback.mutate({ id: nudge.id, action: "snooze", snoozeMinutes: 24 * 60 })
                      }
                    >
                      Snooze
                    </Button>
                    <Button
                      size="small"
                      variant="text"
                      color="warning"
                      disabled={nudgeFeedback.isPending}
                      onClick={() => nudgeFeedback.mutate({ id: nudge.id, action: "dismiss" })}
                    >
                      Dismiss
                    </Button>
                  </Stack>
                </Stack>
              ))}
              {(nudgesQ.data?.nudges || []).length === 0 ? (
                <Typography variant="body2" color="text.secondary">
                  No immediate improvements detected.
                </Typography>
              ) : null}
            </Stack>
          </>
        ) : null}
      </CardContent>
    </Card>
  );
}
