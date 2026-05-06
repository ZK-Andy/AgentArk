import { useMemo } from "react";
import { NeuralPanel } from "./NeuralPanel";
import type { TraceOperationalEvent } from "../../types";

export type ToolActivityCardProps = {
  events?: TraceOperationalEvent[];
};

type ToolRow = {
  name: string;
  count: number;
  latencyMs: number | null;
};

function formatToolLabel(value: string): string {
  return value
    .replace(/[_-]+/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/\b\w/g, (char) => char.toUpperCase());
}

function formatLatency(value: number | null): string {
  if (value == null) return "-";
  return value >= 1000 ? `${(value / 1000).toFixed(1)}s` : `${Math.round(value)}ms`;
}

export function ToolActivityCard({ events }: ToolActivityCardProps) {
  const rows = useMemo<ToolRow[]>(() => {
    const map = new Map<string, { count: number; latencyTotal: number; latencySamples: number }>();
    for (const event of Array.isArray(events) ? events : []) {
      const rawName = String(event.tool_name || event.event_type || event.channel || "").trim();
      if (!rawName) continue;
      const current = map.get(rawName) || { count: 0, latencyTotal: 0, latencySamples: 0 };
      current.count += 1;
      if (typeof event.latency_ms === "number" && Number.isFinite(event.latency_ms)) {
        current.latencyTotal += Math.max(0, event.latency_ms);
        current.latencySamples += 1;
      }
      map.set(rawName, current);
    }
    return Array.from(map.entries())
      .map(([name, value]) => ({
        name: formatToolLabel(name),
        count: value.count,
        latencyMs:
          value.latencySamples > 0 ? value.latencyTotal / value.latencySamples : null,
      }))
      .sort((a, b) => b.count - a.count || a.name.localeCompare(b.name))
      .slice(0, 5);
  }, [events]);

  const maxCount = rows.reduce((max, row) => Math.max(max, row.count), 0) || 1;

  return (
    <NeuralPanel title="Tool Activity" tag="LIVE" tagTone="cyan" className="nw-card--tool-activity">
      <div className="nw-tool-list">
        {rows.length === 0 ? (
          <div className="nw-panel-muted">No tool events have landed yet.</div>
        ) : (
          rows.map((row) => (
            <div className="nw-tool-row" key={row.name}>
              <div className="nw-tool-mark">{row.name.slice(0, 2).toUpperCase()}</div>
              <div className="nw-tool-main">
                <div className="nw-tool-line">
                  <span className="nw-tool-name">{row.name}</span>
                  <span className="nw-tool-count">{row.count}</span>
                </div>
                <div className="nw-meter" aria-hidden="true">
                  <div className="nw-meter-fill" style={{ width: `${(row.count / maxCount) * 100}%` }} />
                </div>
              </div>
              <div className="nw-tool-latency">{formatLatency(row.latencyMs)}</div>
            </div>
          ))
        )}
      </div>
    </NeuralPanel>
  );
}
