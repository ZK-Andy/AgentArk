import {
  Box,
  Button,
  Card,
  CardContent,
  Stack,
  Typography,
} from "@mui/material";
import AutoAwesomeRoundedIcon from "@mui/icons-material/AutoAwesomeRounded";
import type { BriefingResponse, RecommendedSkill } from "../types";

type Suggestion = {
  id: string;
  title: string;
  detail: string;
  priority: number;
  skill: RecommendedSkill;
};

type Props = {
  briefing?: BriefingResponse;
  onExecuteSkill: (skill: RecommendedSkill) => void;
  executing: boolean;
};

function mergeSuggestions(briefing?: BriefingResponse): Suggestion[] {
  const items: Suggestion[] = [];
  const skills: RecommendedSkill[] =
    briefing?.recommended_skills ||
    ((briefing as unknown as { recommended_actions?: RecommendedSkill[] })?.recommended_actions ||
      []);

  for (const skill of skills) {
    items.push({
      id: skill.id,
      title: skill.title,
      detail: skill.summary || skill.description || "",
      priority: 3,
      skill,
    });
  }

  items.sort((a, b) => b.priority - a.priority);
  return items.slice(0, 2);
}

export function SmartSuggestions({
  briefing,
  onExecuteSkill,
  executing,
}: Props) {
  const suggestions = mergeSuggestions(briefing);

  if (suggestions.length === 0) return null;

  return (
    <Card className="mission-panel mission-panel--adaptive mission-side-panel mission-side-panel--suggestions">
      <CardContent sx={{ p: 1.2, display: "flex", flexDirection: "column" }}>
        <Stack spacing={1.15} className="mission-panel-content">
          <Stack direction="row" alignItems="center" spacing={0.75}>
            <AutoAwesomeRoundedIcon sx={{ fontSize: 18, color: "#2fd4ff" }} />
            <Box sx={{ flex: 1 }}>
              <Typography variant="body1" sx={{ fontWeight: 700 }}>
                Recommended Skills
              </Typography>
              <Typography variant="caption" color="text.secondary">
                Available next actions from the current brief and active runtime context.
              </Typography>
            </Box>
          </Stack>

          <Stack spacing={0.85} className="mission-panel-section">
            {suggestions.map((suggestion, index) => (
              <Box
                key={suggestion.id}
                className="action-row"
                sx={{
                  p: "8px 10px",
                  background:
                    "linear-gradient(180deg, rgba(8, 18, 34, 0.72), rgba(6, 14, 28, 0.66))",
                }}
              >
                <Stack spacing={0.5}>
                  <Stack
                    direction="row"
                    spacing={0.75}
                    alignItems="center"
                    useFlexGap
                    flexWrap="wrap"
                  >
                    <Box
                      sx={{
                        width: 22,
                        height: 22,
                        borderRadius: "50%",
                        border: "1px solid rgba(94, 184, 243, 0.28)",
                        display: "inline-flex",
                        alignItems: "center",
                        justifyContent: "center",
                        color: "rgba(144, 221, 255, 0.98)",
                        fontSize: "0.68rem",
                        fontWeight: 700,
                        flexShrink: 0,
                      }}
                    >
                      {index + 1}
                    </Box>
                    <Typography
                      variant="body2"
                      fontWeight={700}
                      className="mission-title-clamp"
                    >
                      {suggestion.title}
                    </Typography>
                    <Typography
                      variant="caption"
                      sx={{
                        color: "rgba(141, 192, 231, 0.72)",
                        textTransform: "uppercase",
                        letterSpacing: "0.08em",
                      }}
                    >
                      Recommended skill
                    </Typography>
                  </Stack>
                  <Typography
                    variant="caption"
                    color="text.secondary"
                    sx={{ lineHeight: 1.45 }}
                    className="mission-detail-clamp"
                  >
                    {suggestion.detail}
                  </Typography>

                  <Stack direction="row" spacing={0.6} mt={0.35} useFlexGap flexWrap="wrap">
                    <Button
                      variant="contained"
                      size="small"
                      disabled={executing}
                      onClick={() => onExecuteSkill(suggestion.skill)}
                      sx={{ textTransform: "none", minWidth: 52 }}
                    >
                      Run
                    </Button>
                  </Stack>
                </Stack>
              </Box>
            ))}
          </Stack>
        </Stack>
      </CardContent>
    </Card>
  );
}
