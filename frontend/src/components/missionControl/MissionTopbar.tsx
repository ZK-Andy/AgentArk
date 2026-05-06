import type { ReactElement } from "react";
import { PRODUCT_NAME, PRODUCT_CATEGORY, PRODUCT_TAGLINE } from "../../brand";

void PRODUCT_TAGLINE;

type MissionTopbarProps = {
  agentPaused?: boolean;
  hasLlmConfigured?: boolean;
};

export function MissionTopbar(props: MissionTopbarProps): ReactElement {
  const { agentPaused = false } = props;
  const subLabel = PRODUCT_CATEGORY.toUpperCase().split(" ").join(" · ");

  return (
    <header className="nw-topbar" data-tour-target="welcome-hero">
      <div className="nw-brand">
        <img className="nw-brand-mark" src="/logo.svg" alt={PRODUCT_NAME} />
        <div className="nw-brand-text">
          <div className="nw-brand-name">{PRODUCT_NAME}</div>
          <div className="nw-brand-sub">{subLabel}</div>
        </div>
      </div>
      <div className="nw-chip-row">
        <span className={`nw-chip${agentPaused ? " nw-chip--warn" : ""}`}>
          <span className="nw-chip-dot" />
          {agentPaused ? "BACKGROUND OS PAUSED" : "BACKGROUND OS ON"}
        </span>
        <span className="nw-chip nw-chip--cyan">
          <span className="nw-chip-dot" />
          PRIVATE BY DEFAULT
        </span>
        <span className="nw-chip">
          <span className="nw-chip-dot" />
          REVIEWABLE ACTIONS
        </span>
      </div>
    </header>
  );
}

export default MissionTopbar;
