import { useMemo } from "react";
import { NeuralPanel } from "./NeuralPanel";
import type { TraceSummary } from "../../types";
import { formatUiRelativeDateTimeMeta } from "../../lib/dateFormat";

export type RuntimeActivityCardProps = {
  traces: TraceSummary[];
  onOpenActivity?: () => void;
};

type Tone = "good" | "warn" | "crit" | "cyan" | "default";

function pickToneForStatus(status: string): Tone {
  const s = String(status || "").toLowerCase();
  if (
    s.includes("fail") ||
    s.includes("error") ||
    s.includes("timed_out") ||
    s.includes("denied")
  ) {
    return "crit";
  }
  if (
    s.includes("needs_auth") ||
    s.includes("not_configured") ||
    s.includes("needs_setup") ||
    s.includes("warning") ||
    s.includes("issue")
  ) {
    return "warn";
  }
  if (
    s.includes("pending") ||
    s.includes("running") ||
    s.includes("in_progress") ||
    s.includes("active") ||
    s.includes("live")
  ) {
    return "cyan";
  }
  if (
    s.includes("done") ||
    s.includes("completed") ||
    s.includes("ok") ||
    s.includes("success")
  ) {
    return "good";
  }
  return "default";
}

function iconToneCls(tone: Tone): string {
  if (tone === "warn") return "nw-activity-ic--warn";
  if (tone === "crit") return "nw-activity-ic--crit";
  if (tone === "cyan") return "nw-activity-ic--cyan";
  return "";
}

function statusToneCls(tone: Tone): string {
  if (tone === "warn") return "nw-activity-status--warn";
  if (tone === "crit") return "nw-activity-status--crit";
  return "";
}

export function RuntimeActivityCard({ traces, onOpenActivity }: RuntimeActivityCardProps) {
  const recentTraces = useMemo(() => {
    const all = Array.isArray(traces) ? traces.slice() : [];
    all.sort((a, b) => {
      const aTs = a.started_at || "";
      const bTs = b.started_at || "";
      if (aTs === bTs) return 0;
      return aTs < bTs ? 1 : -1;
    });
    return all.slice(0, 2);
  }, [traces]);

  return (
    <NeuralPanel title="Runtime Activity" tag="LIVE" tagTone="cyan" className="nw-panel--runtime">
      <div className="nw-panel-muted">
        Recent supervised runs and operator-visible outcomes.
      </div>
      <div className="nw-row-list" style={{ marginTop: 8 }}>
        {recentTraces.length === 0 ? (
          <div className="nw-panel-muted" style={{ paddingTop: 12 }}>
            No recent runs landed yet.
          </div>
        ) : (
          recentTraces.map((trace) => {
            const tone = pickToneForStatus(trace.status);
            const icCls = ["nw-activity-ic", iconToneCls(tone)].filter(Boolean).join(" ");
            const stCls = ["nw-activity-status", statusToneCls(tone)]
              .filter(Boolean)
              .join(" ");
            return (
              <div className="nw-activity-row" key={trace.id}>
                <div className={icCls}>.</div>
                <div className="nw-activity-meta">
                  <div className="nw-activity-ts" title={formatUiRelativeDateTimeMeta(trace.started_at).tip}>
                    {formatUiRelativeDateTimeMeta(trace.started_at).label}
                  </div>
                  <div className="nw-activity-txt">
                    {trace.message_preview || "(no preview)"}
                  </div>
                  <div className={stCls}>{String(trace.status || "").toUpperCase()}</div>
                </div>
              </div>
            );
          })
        )}
      </div>
      {onOpenActivity ? (
        <button
          type="button"
          className="nw-btn nw-btn--small"
          style={{ marginTop: 12 }}
          onClick={onOpenActivity}
        >
          Activity feed <span className="nw-arrow">-&gt;</span>
        </button>
      ) : null}
    </NeuralPanel>
  );
}
