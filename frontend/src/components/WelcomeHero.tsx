import { Box, Button, Card, CardContent, Chip, Stack, Typography } from "@mui/material";
import ChatRoundedIcon from "@mui/icons-material/ChatRounded";
import AutoAwesomeRoundedIcon from "@mui/icons-material/AutoAwesomeRounded";
import PauseCircleOutlineRoundedIcon from "@mui/icons-material/PauseCircleOutlineRounded";
import PlayCircleOutlineRoundedIcon from "@mui/icons-material/PlayCircleOutlineRounded";
import ListAltRoundedIcon from "@mui/icons-material/ListAltRounded";
import { useEffect, useMemo, useState } from "react";

type Props = {
  onGoChat?: () => void;
  onRunBriefing?: () => void;
  onViewTasks?: () => void;
  onTogglePause?: () => void;
  agentPaused?: boolean;
  briefingLoading?: boolean;
  pauseLoading?: boolean;
  prompts?: string[];
};

export function WelcomeHero({
  onGoChat,
  onRunBriefing,
  onViewTasks,
  onTogglePause,
  agentPaused = false,
  briefingLoading = false,
  pauseLoading = false,
  prompts,
}: Props) {
  const heroPrompts = useMemo(
    () =>
      prompts && prompts.length > 0
        ? prompts
        : [
            "Review recent changes and list only the critical risks.",
            "Build a small app to track competitor launches and deploy it.",
            "Import this skill URL and wire up any required secrets.",
            "Summarize the current project state and name the next decision.",
            "Inspect active automations and surface anything that needs intervention.",
          ],
    [prompts]
  );
  const greeting = useMemo(() => {
    const h = new Date().getHours();
    if (h < 5) return "Welcome back";
    if (h < 12) return "Good morning";
    if (h < 18) return "Good afternoon";
    return "Good evening";
  }, []);
  const [promptIndex, setPromptIndex] = useState(0);
  const [typedPrompt, setTypedPrompt] = useState("");
  const [isDeletingPrompt, setIsDeletingPrompt] = useState(false);
  const promptSignature = heroPrompts.join("\n");

  useEffect(() => {
    setPromptIndex(0);
    setTypedPrompt("");
    setIsDeletingPrompt(false);
  }, [promptSignature]);

  useEffect(() => {
    if (typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setTypedPrompt(heroPrompts[promptIndex] || "");
      return;
    }

    const activePrompt = heroPrompts[promptIndex] || "";
    const nextDelay = isDeletingPrompt ? 16 : 32;
    const holdDelay = 1700;
    const resetDelay = 240;

    const timer = window.setTimeout(() => {
      if (!isDeletingPrompt && typedPrompt !== activePrompt) {
        setTypedPrompt(activePrompt.slice(0, typedPrompt.length + 1));
        return;
      }
      if (!isDeletingPrompt) {
        setIsDeletingPrompt(true);
        return;
      }
      if (typedPrompt.length > 0) {
        setTypedPrompt(activePrompt.slice(0, typedPrompt.length - 1));
        return;
      }
      setIsDeletingPrompt(false);
      setPromptIndex((prev) => (prev + 1) % heroPrompts.length);
    }, !isDeletingPrompt && typedPrompt === activePrompt ? holdDelay : typedPrompt.length === 0 && isDeletingPrompt ? resetDelay : nextDelay);

    return () => window.clearTimeout(timer);
  }, [heroPrompts, isDeletingPrompt, promptIndex, typedPrompt]);

  return (
    <Card
      className="welcome-hero-card"
      sx={{
        borderRadius: 5,
        border: "1px solid rgba(108, 156, 212, 0.18)",
        background:
          "radial-gradient(circle at 50% 0%, rgba(47, 212, 255, 0.2), rgba(0,0,0,0) 40%)," +
          "linear-gradient(160deg, rgba(9, 21, 39, 0.97), rgba(8, 18, 33, 0.78))",
        boxShadow: "0 28px 60px rgba(0, 0, 0, 0.24)",
        overflow: "hidden",
      }}
    >
      <CardContent sx={{ p: { xs: 1.4, md: 1.8 }, textAlign: { xs: "left", md: "center" }, position: "relative" }}>
<Stack spacing={{ xs: 0.8, md: 1 }} alignItems={{ xs: "flex-start", md: "center" }} sx={{ position: "relative", zIndex: 1 }}>
          <Box
            component="img"
            src="/logo.svg"
            alt="AgentArk"
            sx={{
              width: { xs: 52, md: 64 },
              height: { xs: 52, md: 64 },
              flexShrink: 0,
              filter: "drop-shadow(0 0 18px rgba(47, 212, 255, 0.26))"
            }}
          />
          <Stack direction="row" spacing={0.75} useFlexGap flexWrap="wrap" justifyContent="center">
            <Chip size="small" color={agentPaused ? "warning" : "success"} label={agentPaused ? "Autonomy Paused" : "Autonomy Active"} />
            <Chip size="small" label="Chat-first console" />
            <Chip size="small" label="Tools, traces, automations" />
          </Stack>
          <Box sx={{ maxWidth: 760 }}>
            <Typography
              variant="h2"
              sx={{
                fontWeight: 700,
                lineHeight: 1.08,
                letterSpacing: "-0.04em",
                fontSize: { xs: "1.7rem", md: "2.4rem" }
              }}
            >
              {greeting}. What should AgentArk handle next?
            </Typography>
            <Typography variant="body1" color="text.secondary" sx={{ mt: 0.75, maxWidth: 620, mx: { md: "auto" } }}>
              Describe the result once. AgentArk keeps the active task centered, while tools, automations, and traces stay one click away instead of competing for space.
            </Typography>
            <Typography
              variant="body2"
              sx={{
                mt: 0.8,
                color: "rgba(196, 230, 255, 0.96)",
                px: 1.05,
                py: 0.65,
                borderRadius: 999,
                display: "inline-flex",
                alignItems: "center",
                maxWidth: { xs: "100%", md: 760 },
                width: { xs: "100%", md: "auto" },
                border: "1px solid rgba(108, 156, 212, 0.22)",
                background: "rgba(8, 19, 34, 0.58)",
                whiteSpace: "nowrap",
                overflow: "hidden"
              }}
            >
              <Box
                component="span"
                sx={{
                  minWidth: 0,
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap"
                }}
              >
                Try: "{typedPrompt || heroPrompts[0]}"
              </Box>
              <Box
                component="span"
                sx={{
                  display: "inline-block",
                  width: "0.7ch",
                  flex: "0 0 auto",
                  ml: 0.15,
                  opacity: 0.9,
                  animation: "welcomeHeroCursorBlink 1s steps(1, end) infinite"
                }}
              >
                |
              </Box>
            </Typography>
          </Box>
          <Stack direction={{ xs: "column", sm: "row" }} spacing={0.85} sx={{ width: { xs: "100%", sm: "auto" } }}>
            {onGoChat ? (
              <Button
                size="medium"
                variant="contained"
                startIcon={<ChatRoundedIcon />}
                onClick={onGoChat}
                sx={{ borderRadius: 999, px: 2.5, textTransform: "none" }}
              >
                Open Chat
              </Button>
            ) : null}
            {onRunBriefing ? (
              <Button
                size="medium"
                variant="outlined"
                startIcon={<AutoAwesomeRoundedIcon />}
                onClick={onRunBriefing}
                disabled={briefingLoading}
                sx={{ borderRadius: 999, px: 2.3, textTransform: "none" }}
              >
                {briefingLoading ? "Running..." : "Run Briefing"}
              </Button>
            ) : null}
            {onViewTasks ? (
              <Button
                size="medium"
                variant="outlined"
                startIcon={<ListAltRoundedIcon />}
                onClick={onViewTasks}
                sx={{ borderRadius: 999, px: 2.3, textTransform: "none" }}
              >
                View Tasks
              </Button>
            ) : null}
            {onTogglePause ? (
              <Button
                size="medium"
                variant="text"
                startIcon={agentPaused ? <PlayCircleOutlineRoundedIcon /> : <PauseCircleOutlineRoundedIcon />}
                onClick={onTogglePause}
                disabled={pauseLoading}
                sx={{ borderRadius: 999, px: 1.5, textTransform: "none" }}
              >
                {agentPaused ? "Resume Autonomy" : "Pause Autonomy"}
              </Button>
            ) : null}
          </Stack>
        </Stack>
      </CardContent>
    </Card>
  );
}
