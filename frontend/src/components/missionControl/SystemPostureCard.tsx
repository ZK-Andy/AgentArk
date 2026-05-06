import { NeuralPanel } from "./NeuralPanel";
import type { NeuralPanelTagTone } from "./NeuralPanel";
import type { StatusResponse } from "../../types";

type AutomationCounts = {
  tasks: number;
  watchers: number;
  apps: number;
  integrations: number;
};

export type SystemPostureCardProps = {
  serverStatus?: { at: number; rtt_ms: number; status: StatusResponse };
  serverError: boolean;
  serverLoading: boolean;
  currentTaskDesc?: string;
  agentPaused?: boolean;
  hasLlmConfigured?: boolean;
  automationCounts?: AutomationCounts;
  recentFailureTitle?: string | null;
};

export function SystemPostureCard({
  serverStatus,
  serverError,
  serverLoading,
  currentTaskDesc,
  agentPaused = false,
  hasLlmConfigured = true,
  automationCounts,
  recentFailureTitle,
}: SystemPostureCardProps) {
  let tagLabel = "CONNECTING…";
  let tagTone: NeuralPanelTagTone = "warn";

  if (serverError) {
    tagLabel = "OFFLINE";
    tagTone = "crit";
  } else if (currentTaskDesc) {
    tagLabel = "WORKING";
    tagTone = "cyan";
  } else if (serverStatus) {
    tagLabel = "IDLE · READY";
    tagTone = "good";
  } else if (serverLoading) {
    tagLabel = "CONNECTING…";
    tagTone = "warn";
  }

  const autonomyVal = agentPaused ? "PAUSED" : "ACTIVE";
  const autonomyValCls = agentPaused
    ? "nw-row-v nw-row-v--warn"
    : "nw-row-v nw-row-v--good";

  const modelConfigured = hasLlmConfigured ?? true;
  const modelVal = modelConfigured ? "CONFIGURED" : "NEEDS SETUP";
  const modelValCls = modelConfigured
    ? "nw-row-v nw-row-v--good"
    : "nw-row-v nw-row-v--warn";

  const pendingCount = serverStatus?.status?.tasks_pending ?? 0;
  let runtimeVal: string;
  let runtimeValCls: string;
  if (!automationCounts) {
    runtimeVal = "NO INVENTORY";
    runtimeValCls = "nw-row-v nw-row-v--dim";
  } else {
    const surfaceTotal =
      automationCounts.tasks +
      automationCounts.watchers +
      automationCounts.apps +
      automationCounts.integrations;
    if (pendingCount > 0) {
      runtimeVal = `${surfaceTotal} SURFACES · ${pendingCount} PENDING`;
      runtimeValCls = "nw-row-v nw-row-v--warn";
    } else {
      runtimeVal = `${surfaceTotal} SURFACES`;
      runtimeValCls = "nw-row-v nw-row-v--cyan";
    }
  }

  const status = serverStatus?.status;
  const memCount = status?.memory_entries ?? 0;
  const skillCount = status?.skills_loaded ?? status?.actions_loaded ?? 0;
  const rttDisplay =
    serverStatus?.rtt_ms !== undefined && serverStatus?.rtt_ms !== null
      ? String(serverStatus.rtt_ms)
      : "—";

  return (
    <NeuralPanel title="System Posture" tag={tagLabel} tagTone={tagTone} className="nw-panel--system">
      <div className="nw-panel-muted">
        Live reasoning posture, queue pressure, model readiness, and runtime health.
      </div>
      <div className="nw-row-list">
        <div className="nw-row">
          <span className="nw-row-k">AUTONOMY</span>
          <span className={autonomyValCls}>{autonomyVal}</span>
        </div>
        <div className="nw-row">
          <span className="nw-row-k">MODEL</span>
          <span className={modelValCls}>{modelVal}</span>
        </div>
        <div className="nw-row">
          <span className="nw-row-k">RUNTIME</span>
          <span className={runtimeValCls}>{runtimeVal}</span>
        </div>
      </div>
      {recentFailureTitle ? (
        <div
          className="nw-panel-muted"
          style={{ color: "var(--nw-crit)", marginTop: 8 }}
        >
          Latest degraded run: {recentFailureTitle}
        </div>
      ) : null}
      <div className="nw-footline">
        <span>
          {memCount} MEMORIES · {skillCount} SKILLS
        </span>
        <span>RTT {rttDisplay}MS</span>
      </div>
    </NeuralPanel>
  );
}
