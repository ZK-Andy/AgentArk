import { useEffect, useRef } from "react";
import type { ReactElement } from "react";
import type { ChatStepCard } from "../types";
import { highlightCodeLine } from "../codeHighlight";

export interface TraceTimelineProps {
  cards: ChatStepCard[];
  activeStepId: string | null;
  onActivate?: (id: string) => void;
}

const SUMMARY_LIMIT = 160;

function truncate(text: string, limit = SUMMARY_LIMIT): string {
  const trimmed = (text ?? "").trim();
  if (trimmed.length <= limit) return trimmed;
  return `${trimmed.slice(0, limit - 3)}...`;
}

function pickPayload(card: ChatStepCard): string {
  return (
    card.payloadView?.body ||
    card.rawDetailFull ||
    card.detailFull ||
    card.detail ||
    ""
  );
}

function renderHighlighted(payload: string): ReactElement[] {
  const lines = payload.split("\n");
  return lines.map((line, lineIdx) => {
    const tokens = highlightCodeLine(line, "json");
    return (
      <div key={lineIdx} className="trace-row-payload-line">
        {tokens.map((token, tokenIdx) =>
          token.className ? (
            <span
              key={tokenIdx}
              className={`code-token code-token-${token.className}`}
            >
              {token.text}
            </span>
          ) : (
            <span key={tokenIdx}>{token.text}</span>
          ),
        )}
        {line.length === 0 ? " " : null}
      </div>
    );
  });
}

interface TraceRowProps {
  card: ChatStepCard;
  isActive: boolean;
  onActivate?: (id: string) => void;
}

function TraceRow({ card, isActive, onActivate }: TraceRowProps) {
  const ref = useRef<HTMLLIElement | null>(null);

  useEffect(() => {
    if (isActive) {
      ref.current?.scrollIntoView({ block: "nearest" });
    }
  }, [isActive]);

  const subline = truncate(card.summary || card.detail || "");
  const payload = pickPayload(card);

  return (
    <li
      ref={ref}
      className={`trace-row${isActive ? " is-active" : ""}`}
      data-step-id={card.id}
    >
      <button
        type="button"
        className="trace-row-button"
        onClick={() => onActivate?.(card.id)}
      >
        <div className="trace-row-head">
          <span className="trace-row-kind" data-kind={card.kind}>
            {card.kind}
          </span>
          <span className="trace-row-label">{card.label}</span>
          <span className="trace-row-time">{card.time}</span>
        </div>
        {subline ? <div className="trace-row-detail">{subline}</div> : null}
      </button>
      <details className="trace-row-expander" open={isActive}>
        <summary>View raw payload</summary>
        <div className="trace-row-payload">
          {payload ? (
            renderHighlighted(payload)
          ) : (
            <div className="trace-row-payload-empty">
              (no payload captured)
            </div>
          )}
        </div>
      </details>
    </li>
  );
}

export function TraceTimeline({
  cards,
  activeStepId,
  onActivate,
}: TraceTimelineProps): ReactElement {
  if (!cards || cards.length === 0) {
    return <div className="trace-empty">No trace events yet.</div>;
  }

  return (
    <ol className="trace-timeline">
      {cards.map((card) => (
        <TraceRow
          key={card.id}
          card={card}
          isActive={card.id === activeStepId}
          onActivate={onActivate}
        />
      ))}
    </ol>
  );
}

export default TraceTimeline;
