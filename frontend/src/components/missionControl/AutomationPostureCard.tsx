import { NeuralPanel } from "./NeuralPanel";

export type AutomationPostureCardProps = {
  automationCounts: {
    tasks: number;
    watchers: number;
    apps: number;
    integrations: number;
  };
  surfaceTotal: number;
  headline: string;
  primaryIntegrationLabel?: string | null;
  onOpenInventory?: () => void;
};

export function AutomationPostureCard({
  automationCounts,
  surfaceTotal,
  headline,
  primaryIntegrationLabel,
  onOpenInventory,
}: AutomationPostureCardProps) {
  return (
    <NeuralPanel title="Automation Posture" tag={`${surfaceTotal} SURFACES`} className="nw-panel--automation">
      <div className="nw-panel-lead" style={{ fontSize: 14 }}>
        Live surfaces and system drift.
      </div>
      <div className="nw-panel-muted">{headline}</div>
      <div className="nw-kv-grid nw-kv-grid--4" style={{ marginTop: 12 }}>
        <div className="nw-kv">
          <div className="nw-kv-k">TASKS</div>
          <div className="nw-kv-v">{automationCounts.tasks}</div>
        </div>
        <div className="nw-kv">
          <div className="nw-kv-k">WATCHERS</div>
          <div className="nw-kv-v">{automationCounts.watchers}</div>
        </div>
        <div className="nw-kv">
          <div className="nw-kv-k">APPS</div>
          <div className="nw-kv-v">{automationCounts.apps}</div>
        </div>
        <div className="nw-kv">
          <div className="nw-kv-k">INTEGRATIONS</div>
          <div className="nw-kv-v nw-kv-v--green">{automationCounts.integrations}</div>
        </div>
      </div>
      {primaryIntegrationLabel ? (
        <div className="nw-chip-row" style={{ marginTop: 8 }}>
          <span className="nw-chip nw-chip--cyan">
            <span className="nw-chip-dot" />
            INTEGRATION · {primaryIntegrationLabel.toUpperCase()}
          </span>
        </div>
      ) : null}
      {onOpenInventory ? (
        <button
          type="button"
          className="nw-btn nw-btn--small"
          style={{ marginTop: 8 }}
          onClick={onOpenInventory}
        >
          Open inventory <span className="nw-arrow">→</span>
        </button>
      ) : null}
    </NeuralPanel>
  );
}
