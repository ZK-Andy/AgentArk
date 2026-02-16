import { Box, Button, Card, CardContent, Stack, Typography } from "@mui/material";
import { useEffect, useMemo, useState } from "react";

type Props = {
  onGoChat?: () => void;
};

function useTypewriter(text: string, msPerChar: number) {
  const [idx, setIdx] = useState(0);
  useEffect(() => {
    setIdx(0);
    if (!text) return;
    const t = setInterval(() => {
      setIdx((v) => (v >= text.length ? v : v + 1));
    }, Math.max(12, msPerChar));
    return () => clearInterval(t);
  }, [text, msPerChar]);
  return text.slice(0, idx);
}

function buildFriendlyVariants(greeting: string, max: number): string[][] {
  const intros = [
    `${greeting}. AgentArk is online.`,
    `${greeting}. Ready when you are.`,
    `${greeting}. Your AGI copilot is standing by.`,
    `${greeting}. Let's build momentum.`,
    `${greeting}. We can start from any rough idea.`,
    `${greeting}. I can take this from concept to done.`,
    `${greeting}. You lead the goal, I handle the execution.`,
    `${greeting}. Mission status: ready.`,
    `${greeting}. I can turn your request into shipped output.`,
    `${greeting}. We can solve this step by step.`,
    `${greeting}. Bring the objective and constraints.`,
    `${greeting}. I am ready to run.`
  ];

  const valueProps = [
    "Tell me the outcome you want and I will plan, execute, and report clearly.",
    "Share the target result and I will drive the work with minimal back-and-forth.",
    "I can break big goals into tasks and move through them fast and safely.",
    "Start in plain language. I will ask only for missing constraints.",
    "Give me a priority and I will turn it into an actionable runbook.",
    "I can handle research, implementation, validation, and status updates in one flow.",
    "If something is risky, I will stop and ask before proceeding.",
    "You can change direction anytime. I will adapt and keep context organized.",
    "I optimize for outcomes: clear plan, reliable execution, concise updates.",
    "I can coordinate tools, code edits, and verification while you stay focused on goals."
  ];

  const prompts = [
    "Help me finish today's top priority from this workspace.",
    "Summarize this codebase and propose the fastest path to ship.",
    "Find the blocker in this project and fix it end to end.",
    "Create a production-ready action for my API and test it.",
    "Review recent changes and list only critical risks.",
    "Set up a daily brief for 8:00 AM with top priorities.",
    "Plan my next 3 highest-impact tasks and start with task 1.",
    "Audit this feature and implement missing pieces with tests.",
    "Connect my integration and verify it with a real test run.",
    "Turn this rough idea into a concrete execution plan."
  ];

  const variants: string[][] = [];
  for (const intro of intros) {
    for (const value of valueProps) {
      for (const prompt of prompts) {
        variants.push([intro, value, `Try: "${prompt}"`]);
        if (variants.length >= max) {
          return variants;
        }
      }
    }
  }
  return variants;
}

export function WelcomeHero({ onGoChat }: Props) {
  const greeting = useMemo(() => {
    const h = new Date().getHours();
    if (h < 5) return "Welcome back";
    if (h < 12) return "Good morning";
    if (h < 18) return "Good afternoon";
    return "Good evening";
  }, []);

  const generatedFriendlyVariants = useMemo(() => buildFriendlyVariants(greeting, 220), [greeting]);

  const guidanceVariants = useMemo(
    () => [
      [
        "SECURE CREDENTIALS",
        "Never paste keys into normal chat content.",
        "Telegram/WhatsApp: `/setsecret KEY=VALUE`",
        "Web: `set secret KEY=VALUE`"
      ],
      [
        "INTEGRATIONS (MULTI-TURN)",
        "Ask: \"Connect me to <service>\"",
        "I will tell you which secrets are required, then validate the connection after you set them.",
        "If something looks risky, I will stop and ask for explicit approval."
      ],
      [
        "SKILLS / ACTIONS",
        "Ask: \"Create a new action called <name> that calls <endpoint> and stores results.\"",
        "I will scaffold it, run a security check, then confirm before enabling it."
      ],
      [
        "WELCOME",
        "Start in plain language. I will ask only what is needed, then move fast and keep things organized.",
        "You can change direction anytime and I will adapt without losing context."
      ]
    ],
    []
  );

  const variants = useMemo(
    () => [...generatedFriendlyVariants, ...guidanceVariants],
    [generatedFriendlyVariants, guidanceVariants]
  );

  const [variantIdx] = useState(() => Math.floor(Math.random() * variants.length));
  const full = variants[Math.min(Math.max(variantIdx, 0), variants.length - 1)].join("\n");
  const typed = useTypewriter(full, 18);

  return (
    <Card
      sx={{
        borderRadius: 2,
        border: "1px solid rgba(108, 156, 212, 0.22)",
        background:
          "linear-gradient(160deg, rgba(9, 21, 39, 0.96), rgba(9, 21, 39, 0.70))," +
          "radial-gradient(circle at 20% 20%, rgba(47, 212, 255, 0.18), rgba(0,0,0,0) 40%)," +
          "radial-gradient(circle at 80% 70%, rgba(20, 241, 149, 0.14), rgba(0,0,0,0) 45%)"
      }}
    >
      <CardContent sx={{ p: { xs: 2, md: 2.25 } }}>
        <Stack
          direction={{ xs: "column", md: "row" }}
          spacing={{ xs: 1, md: 0.5 }}
          alignItems={{ xs: "flex-start", md: "center" }}
        >
          <img
            src="/logo.svg"
            alt="AgentArk"
            width={120}
            height={120}
            style={{
              display: "block",
              width: "clamp(96px, 12vw, 120px)",
              height: "clamp(96px, 12vw, 120px)",
              flexShrink: 0,
              marginTop: 18,
              filter: "drop-shadow(0 0 16px rgba(47, 212, 255, 0.28))"
            }}
          />

          <Box sx={{ flex: 1, minWidth: 0 }}>
            <Typography
              variant="h5"
              fontWeight={800}
              sx={{
                letterSpacing: 1.2,
                textTransform: "uppercase",
                fontFamily: '"Orbitron", "Space Grotesk", "Segoe UI", sans-serif',
                textShadow: "0 0 14px rgba(47, 212, 255, 0.28)"
              }}
            >
              AgentArk
            </Typography>
            <Box
              sx={{
                mt: 0.75,
                p: 1.1,
                borderRadius: 1.25,
                minWidth: 0,
                position: "relative",
                border: "1px solid rgba(47, 212, 255, 0.28)",
                background:
                  "linear-gradient(180deg, rgba(8, 22, 42, 0.88), rgba(8, 22, 42, 0.56))," +
                  "repeating-linear-gradient(180deg, rgba(255,255,255,0.035) 0px, rgba(255,255,255,0.035) 1px, rgba(0,0,0,0) 3px, rgba(0,0,0,0) 7px)"
              }}
            >
              {/* Invisible full text to reserve height — prevents layout shift during typing */}
              <Typography
                component="pre"
                aria-hidden
                sx={{
                  m: 0,
                  whiteSpace: "pre-wrap",
                  overflowWrap: "anywhere",
                  wordBreak: "break-word",
                  fontFamily: '"Rajdhani", "Orbitron", "IBM Plex Sans", "Segoe UI", sans-serif',
                  fontWeight: 600,
                  letterSpacing: 0.45,
                  lineHeight: 1.45,
                  fontSize: { xs: 15.2, md: 17 },
                  visibility: "hidden"
                }}
              >
                {full}
              </Typography>
              {/* Visible typed text overlaid on top */}
              <Typography
                component="pre"
                sx={{
                  m: 0,
                  position: "absolute",
                  top: (theme) => theme.spacing(1.1),
                  left: (theme) => theme.spacing(1.1),
                  right: (theme) => theme.spacing(1.1),
                  whiteSpace: "pre-wrap",
                  overflowWrap: "anywhere",
                  wordBreak: "break-word",
                  fontFamily: '"Rajdhani", "Orbitron", "IBM Plex Sans", "Segoe UI", sans-serif',
                  fontWeight: 600,
                  letterSpacing: 0.45,
                  lineHeight: 1.45,
                  fontSize: { xs: 15.2, md: 17 },
                  color: "rgba(188, 226, 255, 0.98)",
                  textShadow: "0 0 10px rgba(47, 212, 255, 0.28)"
                }}
              >
                {typed}
                {typed.length < full.length ? "|" : ""}
              </Typography>
            </Box>
          </Box>

          {null}
        </Stack>
      </CardContent>
    </Card>
  );
}
