import { useEffect, useMemo, useState } from "react";
import type { BriefingResponse, RecommendedAction } from "../../types";
import { NeuralPanel } from "./NeuralPanel";

export type SuggestedStepCardProps = {
  prompts: string[];
  onGoChat?: () => void;
  onRunBriefing?: () => void;
  onViewTasks?: () => void;
  briefingLoading?: boolean;
  briefing?: BriefingResponse | null;
  onExecuteAction?: (action: RecommendedAction) => void;
  executing?: boolean;
};

export function SuggestedStepCard({
  prompts,
  onGoChat,
  onRunBriefing,
  onViewTasks,
  briefingLoading = false,
  briefing,
  onExecuteAction,
  executing = false,
}: SuggestedStepCardProps) {
  const heroPrompts = useMemo(
    () => (prompts && prompts.length > 0 ? prompts : [""]),
    [prompts]
  );
  const promptSignature = heroPrompts.join("\n");

  const [promptIndex, setPromptIndex] = useState(0);
  const [typedPrompt, setTypedPrompt] = useState("");
  const [prefersReducedMotion, setPrefersReducedMotion] = useState(false);

  const activePrompt = heroPrompts[promptIndex] || heroPrompts[0] || "";
  const displayPrompt = useMemo(() => {
    const trimmed = activePrompt.trim();
    return trimmed.length > 96 ? `${trimmed.slice(0, 93).trimEnd()}…` : trimmed;
  }, [activePrompt]);

  useEffect(() => {
    setPromptIndex(0);
  }, [promptSignature]);

  useEffect(() => {
    if (typeof window === "undefined" || !window.matchMedia) {
      return undefined;
    }

    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    const syncPreference = () => {
      setPrefersReducedMotion(media.matches);
    };
    syncPreference();

    if (typeof media.addEventListener === "function") {
      media.addEventListener("change", syncPreference);
      return () => media.removeEventListener("change", syncPreference);
    }

    media.addListener(syncPreference);
    return () => media.removeListener(syncPreference);
  }, []);

  useEffect(() => {
    setTypedPrompt(prefersReducedMotion ? displayPrompt : "");
  }, [displayPrompt, prefersReducedMotion]);

  useEffect(() => {
    if (!displayPrompt || typeof window === "undefined") {
      return undefined;
    }

    if (prefersReducedMotion) {
      if (heroPrompts.length <= 1) {
        return undefined;
      }

      const timer = window.setTimeout(() => {
        setPromptIndex((prev) => (prev + 1) % heroPrompts.length);
      }, 5600);

      return () => window.clearTimeout(timer);
    }

    if (typedPrompt.length < displayPrompt.length) {
      const nextChar = displayPrompt[typedPrompt.length];
      const delay = /[.,!?]/.test(nextChar) ? 48 : nextChar === " " ? 16 : 24;
      const timer = window.setTimeout(() => {
        setTypedPrompt(displayPrompt.slice(0, typedPrompt.length + 1));
      }, delay);

      return () => window.clearTimeout(timer);
    }

    if (heroPrompts.length <= 1) {
      return undefined;
    }

    const holdMs = Math.max(1800, Math.min(3200, displayPrompt.length * 28));
    const timer = window.setTimeout(() => {
      setPromptIndex((prev) => (prev + 1) % heroPrompts.length);
    }, holdMs);

    return () => window.clearTimeout(timer);
  }, [displayPrompt, heroPrompts.length, prefersReducedMotion, typedPrompt]);

  const recommended = briefing?.recommended_actions ?? [];

  return (
    <NeuralPanel title="Suggested Next Step" tag="DAILY USE" tagTone="default" className="nw-panel--suggested">
      <div className="nw-typewriter">
        {typedPrompt}
        {prefersReducedMotion ? null : <span className="nw-typewriter-caret" />}
      </div>
      <div className="nw-panel-muted" style={{ marginTop: 6 }}>
        Rotates through useful OS tasks from your routines, recent work, and unattended runs.
      </div>
      <div className="nw-actions nw-actions--suggested">
        {onGoChat ? (
          <button className="nw-btn nw-btn--primary" onClick={onGoChat}>
            Ask AgentArk <span className="nw-arrow">→</span>
          </button>
        ) : null}
        {onRunBriefing ? (
          <button className="nw-btn" disabled={briefingLoading} onClick={onRunBriefing}>
            {briefingLoading ? "Running..." : "Generate Daily Brief"} <span className="nw-arrow">→</span>
          </button>
        ) : null}
        {onViewTasks ? (
          <button className="nw-btn nw-btn--ghost" onClick={onViewTasks}>
            Review Tasks <span className="nw-arrow">→</span>
          </button>
        ) : null}
      </div>
      {recommended.length > 0 ? (
        <div className="nw-row-list" style={{ marginTop: 10 }}>
          {recommended.slice(0, 2).map((act) => (
            <div key={act.id} className="nw-activity-row">
              <div className="nw-activity-ic">▸</div>
              <div className="nw-activity-meta">
                <div className="nw-activity-ts">RECOMMENDED</div>
                <div className="nw-activity-txt">{act.title}</div>
                {onExecuteAction ? (
                  <button
                    className="nw-btn nw-btn--small"
                    disabled={executing}
                    onClick={() => onExecuteAction(act)}
                    style={{ marginTop: 6 }}
                  >
                    Run <span className="nw-arrow">→</span>
                  </button>
                ) : null}
              </div>
            </div>
          ))}
        </div>
      ) : null}
    </NeuralPanel>
  );
}
