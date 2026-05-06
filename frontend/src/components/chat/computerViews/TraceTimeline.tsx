import { useEffect, useRef } from "react";
import type { ReactElement } from "react";
import type { ChatStepCard } from "../types";
import { buildReadableToolPresentation } from "./presentation";

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

function looksLikeStructuredPayload(text: string): boolean {
  const trimmed = (text || "").trim();
  if (!trimmed) return false;
  return (
    ((trimmed.startsWith("{") || trimmed.startsWith("[")) &&
      /["}\]]\s*[:,]|^\{\s*"|^\[\s*(\{|"|\])/.test(trimmed)) ||
    /^<artifact\b/i.test(trimmed)
  );
}

function safeTraceText(text: string): string {
  const trimmed = (text || "").replace(/\s+/g, " ").trim();
  if (!trimmed) return "";
  if (looksLikeStructuredPayload(trimmed)) return "";
  return truncate(trimmed, 220);
}

function traceDetailRows(card: ChatStepCard, subline: string): string[] {
  const presentation = buildReadableToolPresentation(card);
  const candidates = [
    presentation.summary,
    ...presentation.rows,
    card.summary,
    card.detail,
    card.detailFull,
  ];
  const seen = new Set<string>();
  const rows: string[] = [];
  for (const candidate of candidates) {
    const text = safeTraceText(candidate || "");
    if (!text || text === subline || seen.has(text.toLowerCase())) continue;
    seen.add(text.toLowerCase());
    rows.push(text);
    if (rows.length >= 4) break;
  }
  return rows;
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

  const subline =
    safeTraceText(card.summary || card.detail || "") ||
    "Trace event recorded.";
  const detailRows = traceDetailRows(card, subline);

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
      {detailRows.length > 0 ? (
        <details className="trace-row-expander" open={isActive}>
          <summary>View details</summary>
          <div className="trace-row-payload">
            {detailRows.map((row, index) => (
              <div key={index} className="trace-row-payload-line">
                {row}
              </div>
            ))}
          </div>
        </details>
      ) : null}
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
