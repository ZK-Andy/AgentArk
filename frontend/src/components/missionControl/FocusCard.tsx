import { NeuralPanel, type NeuralPanelTagTone } from "./NeuralPanel";

export type FocusState = "ready" | "active" | "paused" | "issue";

export type FocusCardProps = {
  state: FocusState;
  body: string;
};

const STATE_TAG: Record<FocusState, { label: string; tone: NeuralPanelTagTone }> = {
  ready: { label: "READY", tone: "good" },
  active: { label: "ACTIVE NOW", tone: "good" },
  paused: { label: "PAUSED", tone: "warn" },
  issue: { label: "NEEDS REVIEW", tone: "crit" },
};

export function FocusCard({ state, body }: FocusCardProps) {
  const { label, tone } = STATE_TAG[state];
  return (
    <NeuralPanel title="Current Focus" tag={label} tagTone={tone} className="nw-panel--focus">
      <div className="nw-panel-muted">{body}</div>
    </NeuralPanel>
  );
}
