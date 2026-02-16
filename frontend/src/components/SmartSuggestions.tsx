import {
  Box,
  Button,
  Card,
  CardContent,
  Collapse,
  Stack,
  Typography
} from "@mui/material";
import AutoAwesomeRoundedIcon from "@mui/icons-material/AutoAwesomeRounded";
import { useState } from "react";
import type { BriefingResponse, PredictiveNudge, RecommendedSkill } from "../types";

type Suggestion = {
  id: string;
  title: string;
  detail: string;
  type: "nudge" | "skill";
  priority: number;
  skill?: RecommendedSkill;
  nudge?: PredictiveNudge;
};

type Props = {
  briefing?: BriefingResponse;
  nudges: PredictiveNudge[];
  onExecuteSkill: (skill: RecommendedSkill) => void;
  onSnooze: (id: string) => void;
  onDismiss: (id: string) => void;
  executing: boolean;
  feedbackPending: boolean;
};

function mergeSuggestions(briefing?: BriefingResponse, nudges: PredictiveNudge[] = []): Suggestion[] {
  const items: Suggestion[] = [];

  // Nudges with recommended skills
  for (const n of nudges) {
    const skill = n.recommended_skill ||
      ((n as unknown as { recommended_action?: RecommendedSkill }).recommended_action);
    items.push({
      id: n.id,
      title: n.title,
      detail: n.detail || "",
      type: "nudge",
      priority: n.priority || 0,
      skill: skill || undefined,
      nudge: n,
    });
  }

  // Briefing recommended skills not already covered by nudges
  const nudgeIds = new Set(nudges.map((n) => n.id));
  const skills: RecommendedSkill[] =
    briefing?.recommended_skills ||
    ((briefing as unknown as { recommended_actions?: RecommendedSkill[] })?.recommended_actions || []);
  for (const s of skills) {
    if (nudgeIds.has(s.id)) continue;
    items.push({
      id: s.id,
      title: s.title,
      detail: s.summary || s.description || "",
      type: "skill",
      priority: 3,
      skill: s,
    });
  }

  // Sort by priority descending, take top 3
  items.sort((a, b) => b.priority - a.priority);
  return items.slice(0, 3);
}

export function SmartSuggestions({
  briefing,
  nudges,
  onExecuteSkill,
  onSnooze,
  onDismiss,
  executing,
  feedbackPending,
}: Props) {
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const suggestions = mergeSuggestions(briefing, nudges);

  return (
    <Card sx={{ height: "100%" }}>
      <CardContent sx={{ p: 1.5 }}>
        <Stack direction="row" alignItems="center" spacing={0.75} mb={1.25}>
          <AutoAwesomeRoundedIcon sx={{ fontSize: 18, color: "#2fd4ff" }} />
          <Typography variant="h6">Suggestions</Typography>
        </Stack>

        {suggestions.length === 0 ? (
          <Typography variant="body2" color="text.secondary">
            No suggestions right now.
          </Typography>
        ) : (
          <Stack spacing={0.75}>
            {suggestions.map((s) => (
              <Box key={s.id} className="action-row" sx={{ p: "8px 10px" }}>
                <Stack spacing={0.5}>
                  <Typography variant="body2" fontWeight={600}>
                    {s.title}
                  </Typography>
                  <Typography variant="caption" color="text.secondary" sx={{ lineHeight: 1.4 }}>
                    {s.detail.length > 100 ? s.detail.slice(0, 97) + "..." : s.detail}
                  </Typography>

                  <Collapse in={expandedId === s.id}>
                    <Box sx={{ mt: 0.5, pl: 0.5, borderLeft: "2px solid rgba(47, 212, 255, 0.3)", py: 0.5 }}>
                      <Typography variant="caption" color="text.secondary" sx={{ display: "block", mb: 0.5 }}>
                        {s.detail}
                      </Typography>
                      {s.nudge?.memory_clues?.map((clue) => (
                        <Typography key={clue.id} variant="caption" color="text.secondary" sx={{ display: "block" }}>
                          {clue.memory_type} memory: {clue.summary}
                        </Typography>
                      ))}
                    </Box>
                  </Collapse>

                  <Stack direction="row" spacing={0.6} mt={0.25}>
                    {s.skill ? (
                      <Button
                        variant="contained"
                        size="small"
                        disabled={executing}
                        onClick={() => onExecuteSkill(s.skill!)}
                        sx={{ textTransform: "none", minWidth: 50 }}
                      >
                        Run
                      </Button>
                    ) : null}
                    <Button
                      variant="text"
                      size="small"
                      onClick={() => setExpandedId(expandedId === s.id ? null : s.id)}
                      sx={{ textTransform: "none", minWidth: 45 }}
                    >
                      {expandedId === s.id ? "Less" : "Why?"}
                    </Button>
                    {s.type === "nudge" ? (
                      <>
                        <Button
                          variant="outlined"
                          size="small"
                          disabled={feedbackPending}
                          onClick={() => onSnooze(s.id)}
                          sx={{ textTransform: "none", minWidth: 55 }}
                        >
                          Snooze
                        </Button>
                        <Button
                          variant="text"
                          size="small"
                          color="warning"
                          disabled={feedbackPending}
                          onClick={() => onDismiss(s.id)}
                          sx={{ textTransform: "none", minWidth: 55 }}
                        >
                          Dismiss
                        </Button>
                      </>
                    ) : null}
                  </Stack>
                </Stack>
              </Box>
            ))}
          </Stack>
        )}
      </CardContent>
    </Card>
  );
}
