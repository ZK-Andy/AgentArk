import * as React from "react";
import { PRODUCT_NAME, PRODUCT_CATEGORY, PRODUCT_TAGLINE } from "../../brand";

void PRODUCT_NAME;
void PRODUCT_CATEGORY;
void PRODUCT_TAGLINE;

type MissionHeadlineProps = {
  rttMs?: number | null;
  liveRunCount: number;
};

export function MissionHeadline(props: MissionHeadlineProps) {
  const { rttMs, liveRunCount } = props;
  const rttLabel =
    rttMs === null || rttMs === undefined ? "RTT —" : `RTT ${Math.round(rttMs)}MS`;
  const showRunChip = liveRunCount > 0;
  const runLabel =
    liveRunCount === 1 ? "1 LIVE RUN" : `${liveRunCount} LIVE RUNS`;

  return (
    <section className="nw-headline">
      <h1>
        Private OS for memory, agents, apps,&nbsp;and <em>automation</em>.
      </h1>
      <p>
        One private system for memory, agents, automations, connected tools,
        and reviewable actions.
      </p>
      <div className="nw-chip-row">
        <span className="nw-chip">
          <span className="nw-chip-dot" />
          NEURAL WEB &middot; LIVE
        </span>
        <span className="nw-chip nw-chip--cyan">
          <span className="nw-chip-dot" />
          {rttLabel}
        </span>
        {showRunChip ? (
          <span className="nw-chip nw-chip--cyan">
            <span className="nw-chip-dot" />
            {runLabel}
          </span>
        ) : null}
      </div>
    </section>
  );
}

export default MissionHeadline;
