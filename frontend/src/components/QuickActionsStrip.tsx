import { Button, Stack } from "@mui/material";
import ChatRoundedIcon from "@mui/icons-material/ChatRounded";
import AutoAwesomeRoundedIcon from "@mui/icons-material/AutoAwesomeRounded";
import PauseCircleOutlineRoundedIcon from "@mui/icons-material/PauseCircleOutlineRounded";
import PlayCircleOutlineRoundedIcon from "@mui/icons-material/PlayCircleOutlineRounded";
import ListAltRoundedIcon from "@mui/icons-material/ListAltRounded";

type Props = {
  agentPaused: boolean;
  onAskAgent: () => void;
  onRunBriefing: () => void;
  onTogglePause: () => void;
  onViewTasks: () => void;
  briefingLoading: boolean;
  pauseLoading: boolean;
};

export function QuickActionsStrip({
  agentPaused,
  onAskAgent,
  onRunBriefing,
  onTogglePause,
  onViewTasks,
  briefingLoading,
  pauseLoading,
}: Props) {
  return (
    <Stack
      direction="row"
      spacing={1}
      sx={{ flexWrap: "wrap", gap: 1, "& > *": { flex: { xs: "1 1 calc(50% - 8px)", sm: "0 1 auto" } } }}
    >
      <Button
        variant="contained"
        size="small"
        startIcon={<ChatRoundedIcon />}
        onClick={onAskAgent}
        sx={{ textTransform: "none" }}
      >
        Ask Agent
      </Button>
      <Button
        variant="outlined"
        size="small"
        startIcon={<AutoAwesomeRoundedIcon />}
        onClick={onRunBriefing}
        disabled={briefingLoading}
        sx={{ textTransform: "none" }}
      >
        {briefingLoading ? "Running..." : "Run Briefing"}
      </Button>
      <Button
        variant="outlined"
        size="small"
        startIcon={agentPaused ? <PlayCircleOutlineRoundedIcon /> : <PauseCircleOutlineRoundedIcon />}
        onClick={onTogglePause}
        disabled={pauseLoading}
        sx={{ textTransform: "none" }}
      >
        {agentPaused ? "Resume Agent" : "Pause Agent"}
      </Button>
      <Button
        variant="outlined"
        size="small"
        startIcon={<ListAltRoundedIcon />}
        onClick={onViewTasks}
        sx={{ textTransform: "none" }}
      >
        View Tasks
      </Button>
    </Stack>
  );
}
