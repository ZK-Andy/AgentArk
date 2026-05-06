import { NeuralPanel } from "./NeuralPanel";

export type NeuralWebStatsCardProps = {
  nodes: number;
  edges: number;
  coherence: number;
  pulseMs: number;
};

export function NeuralWebStatsCard({
  nodes,
  edges,
  coherence,
  pulseMs,
}: NeuralWebStatsCardProps) {
  return (
    <NeuralPanel title="Neural Web Stats" tag="NODES · EDGES">
      <div className="nw-panel-muted">
        Topology snapshot of memory, surfaces, runs, and integrations linked to core.
      </div>
      <div className="nw-kv-grid" style={{ marginTop: 12 }}>
        <div className="nw-kv">
          <div className="nw-kv-k">NODES</div>
          <div className="nw-kv-v">{nodes}</div>
        </div>
        <div className="nw-kv">
          <div className="nw-kv-k">EDGES</div>
          <div className="nw-kv-v">{edges}</div>
        </div>
        <div className="nw-kv">
          <div className="nw-kv-k">COHERENCE</div>
          <div className="nw-kv-v nw-kv-v--green">{coherence.toFixed(2)}</div>
        </div>
        <div className="nw-kv">
          <div className="nw-kv-k">PULSE</div>
          <div className="nw-kv-v nw-kv-v--cyan">{pulseMs}ms</div>
        </div>
      </div>
    </NeuralPanel>
  );
}
