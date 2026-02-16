import { Box, Button, IconButton, Stack, Typography } from "@mui/material";
import CloseRoundedIcon from "@mui/icons-material/CloseRounded";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useUiStore } from "../store/uiStore";

/* ------------------------------------------------------------------ */
/*  Step definitions                                                   */
/* ------------------------------------------------------------------ */

type TourStepDef = {
  id: string;
  view: string;
  targetSelector: string;
  title: string;
  body: string;
  placement: "bottom" | "top" | "left" | "right";
  spotlightPadding?: number;
};

const TOUR_STEPS: TourStepDef[] = [
  {
    id: "welcome-models",
    view: "settings",
    targetSelector: "[data-tour-target='settings-models']",
    title: "Welcome! Let's add your first AI model",
    body: "AgentArk needs at least one LLM to work. Add an OpenAI, Anthropic, Ollama, or OpenRouter model here. You can configure multiple models for different tasks — primary, fast, code, and more.",
    placement: "left",
    spotlightPadding: 10,
  },
  {
    id: "goals",
    view: "goals",
    targetSelector: "[data-tour-target='nav-goals']",
    title: "Set goals for your agent",
    body: "Goals are high-level objectives you want to achieve. AgentArk breaks them into tasks, plans execution steps, and tracks progress autonomously. Think of them as your strategic intent.",
    placement: "right",
    spotlightPadding: 6,
  },
  {
    id: "skills",
    view: "skills",
    targetSelector: "[data-tour-target='nav-skills']",
    title: "Skills: what your agent can do",
    body: "Skills are reusable workflows — like a daily briefing, web research, or invoice generation. Install community skills or create your own. Each skill is reviewed for safety before activation.",
    placement: "right",
    spotlightPadding: 6,
  },
  {
    id: "apps",
    view: "apps",
    targetSelector: "[data-tour-target='nav-apps']",
    title: "Connect your apps and services",
    body: "Integrations let AgentArk interact with Gmail, Calendar, GitHub, Notion, and more. Connect your accounts here and the agent can read, write, and automate across all of them.",
    placement: "right",
    spotlightPadding: 6,
  },
  {
    id: "overview",
    view: "overview",
    targetSelector: "[data-tour-target='overview-dashboard']",
    title: "Your Mission Control dashboard",
    body: "This is home base. You'll see tasks needing approval, smart suggestions, today's highlights, and the activity feed. The status bar shows real-time system health at a glance.",
    placement: "bottom",
    spotlightPadding: 12,
  },
  {
    id: "done",
    view: "overview",
    targetSelector: "[data-tour-target='welcome-hero']",
    title: "You're all set!",
    body: "Start by chatting with your agent or adding an AI model if you haven't yet. You can re-run this tour anytime from Settings \u2192 Advanced. Happy building!",
    placement: "bottom",
    spotlightPadding: 10,
  },
];

/* ------------------------------------------------------------------ */
/*  Geometry helpers                                                   */
/* ------------------------------------------------------------------ */

type Rect = { top: number; left: number; width: number; height: number };

function getElementRect(selector: string): Rect | null {
  const el = document.querySelector(selector);
  if (!el) return null;
  const r = el.getBoundingClientRect();
  return { top: r.top, left: r.left, width: r.width, height: r.height };
}

function tooltipPosition(
  target: Rect | null,
  placement: TourStepDef["placement"],
  pad: number,
): { top: number; left: number } {
  const TW = 380;
  const TH = 220;
  const GAP = 14;
  const vw = window.innerWidth;
  const vh = window.innerHeight;

  if (!target) {
    return { top: vh / 2 - TH / 2, left: vw / 2 - TW / 2 };
  }

  let top = 0;
  let left = 0;

  switch (placement) {
    case "bottom":
      top = target.top + target.height + pad + GAP;
      left = target.left + target.width / 2 - TW / 2;
      break;
    case "top":
      top = target.top - pad - GAP - TH;
      left = target.left + target.width / 2 - TW / 2;
      break;
    case "right":
      top = target.top + target.height / 2 - TH / 2;
      left = target.left + target.width + pad + GAP;
      break;
    case "left":
      top = target.top + target.height / 2 - TH / 2;
      left = target.left - pad - GAP - TW;
      break;
  }

  // Clamp to viewport
  if (left < 16) left = 16;
  if (left + TW > vw - 16) left = vw - 16 - TW;
  if (top < 16) top = 16;
  if (top + TH > vh - 16) top = vh - 16 - TH;

  return { top, left };
}

/* ------------------------------------------------------------------ */
/*  Component                                                          */
/* ------------------------------------------------------------------ */

type Props = {
  navigateToView: (view: string, replace?: boolean) => void;
  currentView: string;
};

export function GuidedTour({ navigateToView, currentView }: Props) {
  const tourActive = useUiStore((s) => s.tourActive);
  const tourStep = useUiStore((s) => s.tourStep);
  const nextTourStep = useUiStore((s) => s.nextTourStep);
  const prevTourStep = useUiStore((s) => s.prevTourStep);
  const skipTour = useUiStore((s) => s.skipTour);
  const completeTour = useUiStore((s) => s.completeTour);

  const [targetRect, setTargetRect] = useState<Rect | null>(null);
  const [renderKey, setRenderKey] = useState(0);
  const retryRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const stepDef = TOUR_STEPS[tourStep] as TourStepDef | undefined;

  // Navigate to the correct view when step changes
  useEffect(() => {
    if (!tourActive || !stepDef) return;
    if (currentView !== stepDef.view) {
      navigateToView(stepDef.view);
    }
  }, [tourActive, tourStep]);

  // Measure target element position after view settles
  useLayoutEffect(() => {
    if (!tourActive || !stepDef) return;
    setTargetRect(null);

    const measure = (attempt: number) => {
      const rect = getElementRect(stepDef.targetSelector);
      if (rect) {
        setTargetRect(rect);
        setRenderKey((k) => k + 1);
      } else if (attempt < 8) {
        retryRef.current = setTimeout(() => measure(attempt + 1), 200);
      }
    };

    retryRef.current = setTimeout(() => measure(0), 150);
    return () => {
      if (retryRef.current) clearTimeout(retryRef.current);
    };
  }, [tourActive, tourStep, currentView]);

  // Reposition on resize / scroll
  useEffect(() => {
    if (!tourActive || !stepDef) return;
    const update = () => {
      const rect = getElementRect(stepDef.targetSelector);
      if (rect) setTargetRect(rect);
    };
    window.addEventListener("resize", update);
    window.addEventListener("scroll", update, true);
    return () => {
      window.removeEventListener("resize", update);
      window.removeEventListener("scroll", update, true);
    };
  }, [tourActive, stepDef]);

  // Escape to skip
  useEffect(() => {
    if (!tourActive) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") skipTour();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [tourActive, skipTour]);

  const handleNext = useCallback(() => {
    if (tourStep >= TOUR_STEPS.length - 1) {
      completeTour();
    } else {
      nextTourStep();
    }
  }, [tourStep, completeTour, nextTourStep]);

  if (!tourActive || !stepDef) return null;

  const pad = stepDef.spotlightPadding ?? 8;
  const isFirst = tourStep === 0;
  const isLast = tourStep === TOUR_STEPS.length - 1;
  const pos = tooltipPosition(targetRect, stepDef.placement, pad);

  return (
    <>
      {/* Backdrop with spotlight cutout */}
      <Box
        className="tour-backdrop"
        onClick={skipTour}
        sx={{
          position: "fixed",
          inset: 0,
          zIndex: 9998,
          pointerEvents: "auto",
        }}
      >
        <svg
          width="100%"
          height="100%"
          style={{ display: "block", position: "absolute", inset: 0 }}
        >
          <defs>
            <mask id="tour-spotlight-mask">
              <rect width="100%" height="100%" fill="white" />
              {targetRect && (
                <rect
                  x={targetRect.left - pad}
                  y={targetRect.top - pad}
                  width={targetRect.width + pad * 2}
                  height={targetRect.height + pad * 2}
                  rx={12}
                  ry={12}
                  fill="black"
                />
              )}
            </mask>
          </defs>
          <rect
            width="100%"
            height="100%"
            fill="rgba(3, 7, 17, 0.72)"
            mask="url(#tour-spotlight-mask)"
          />
        </svg>
      </Box>

      {/* Glow ring around spotlight */}
      {targetRect && (
        <Box
          sx={{
            position: "fixed",
            zIndex: 9998,
            top: targetRect.top - pad,
            left: targetRect.left - pad,
            width: targetRect.width + pad * 2,
            height: targetRect.height + pad * 2,
            borderRadius: "12px",
            border: "1.5px solid rgba(47, 212, 255, 0.45)",
            boxShadow:
              "0 0 24px 4px rgba(47, 212, 255, 0.18), inset 0 0 12px rgba(47, 212, 255, 0.08)",
            pointerEvents: "none",
            animation: "tour-ring-pulse 2s ease-in-out infinite",
          }}
        />
      )}

      {/* Tooltip card */}
      <Box
        key={`step-${tourStep}-${renderKey}`}
        className="tour-tooltip"
        onClick={(e) => e.stopPropagation()}
        sx={{
          position: "fixed",
          zIndex: 9999,
          top: pos.top,
          left: pos.left,
          width: 380,
          maxWidth: "calc(100vw - 32px)",
          borderRadius: "16px",
          border: "1px solid rgba(108, 156, 212, 0.28)",
          background:
            "linear-gradient(160deg, rgba(9, 21, 39, 0.96), rgba(9, 21, 39, 0.82))",
          backdropFilter: "blur(24px)",
          WebkitBackdropFilter: "blur(24px)",
          boxShadow:
            "0 20px 60px rgba(0, 0, 0, 0.55), 0 0 40px rgba(47, 212, 255, 0.08)",
          p: 2.5,
        }}
      >
        {/* Close */}
        <IconButton
          size="small"
          onClick={skipTour}
          sx={{
            position: "absolute",
            top: 8,
            right: 8,
            color: "rgba(195, 221, 252, 0.5)",
            "&:hover": { color: "rgba(195, 221, 252, 0.9)" },
          }}
          aria-label="Skip tour"
        >
          <CloseRoundedIcon fontSize="small" />
        </IconButton>

        {/* Step counter */}
        <Typography
          variant="caption"
          sx={{
            color: "rgba(47, 212, 255, 0.85)",
            fontWeight: 600,
            letterSpacing: "0.06em",
            textTransform: "uppercase",
            fontSize: "0.68rem",
          }}
        >
          Step {tourStep + 1} of {TOUR_STEPS.length}
        </Typography>

        {/* Title */}
        <Typography
          variant="h6"
          sx={{
            mt: 0.75,
            fontWeight: 700,
            color: "rgba(236, 245, 255, 0.98)",
            lineHeight: 1.3,
            fontSize: "1.05rem",
          }}
        >
          {stepDef.title}
        </Typography>

        {/* Body */}
        <Typography
          variant="body2"
          sx={{
            mt: 1,
            color: "rgba(195, 221, 252, 0.75)",
            lineHeight: 1.55,
          }}
        >
          {stepDef.body}
        </Typography>

        {/* Progress dots + navigation */}
        <Stack
          direction="row"
          alignItems="center"
          justifyContent="space-between"
          sx={{ mt: 2.5 }}
        >
          {/* Dots */}
          <Stack direction="row" spacing={0.75}>
            {TOUR_STEPS.map((_, i) => (
              <Box
                key={i}
                sx={{
                  width: i === tourStep ? 18 : 7,
                  height: 7,
                  borderRadius: "4px",
                  background:
                    i === tourStep
                      ? "linear-gradient(90deg, rgba(47, 212, 255, 0.95), rgba(20, 241, 149, 0.85))"
                      : i < tourStep
                        ? "rgba(47, 212, 255, 0.4)"
                        : "rgba(108, 156, 212, 0.2)",
                  transition: "all 200ms ease",
                }}
              />
            ))}
          </Stack>

          {/* Buttons */}
          <Stack direction="row" spacing={1}>
            {!isFirst ? (
              <Button
                size="small"
                variant="text"
                onClick={prevTourStep}
                sx={{
                  textTransform: "none",
                  color: "rgba(195, 221, 252, 0.6)",
                  "&:hover": { color: "rgba(195, 221, 252, 0.9)" },
                }}
              >
                Back
              </Button>
            ) : (
              <Button
                size="small"
                variant="text"
                onClick={skipTour}
                sx={{
                  textTransform: "none",
                  color: "rgba(195, 221, 252, 0.45)",
                  "&:hover": { color: "rgba(195, 221, 252, 0.7)" },
                }}
              >
                Skip
              </Button>
            )}
            <Button
              size="small"
              variant="contained"
              onClick={handleNext}
              sx={{
                textTransform: "none",
                px: 2.5,
                background:
                  "linear-gradient(135deg, rgba(47, 212, 255, 0.9), rgba(20, 241, 149, 0.75))",
                color: "#030711",
                fontWeight: 700,
                "&:hover": {
                  background:
                    "linear-gradient(135deg, rgba(47, 212, 255, 1), rgba(20, 241, 149, 0.9))",
                },
              }}
            >
              {isLast ? "Get Started" : "Next"}
            </Button>
          </Stack>
        </Stack>
      </Box>
    </>
  );
}
