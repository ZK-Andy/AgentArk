// Pure helpers for the Evolution page's ArkEvolve candidate workbench.
// Everything here is shape/intent-based over the backend payload — no
// phrasing-dependent behavior — and self-contained so node tests can
// tsc-compile this single file.

export type JsonRecord = Record<string, unknown>;

export type EvolutionCandidateTab =
  | "review"
  | "testing"
  | "deployed"
  | "history";

export type StatTone = "default" | "good" | "warn" | "info";

function asRecord(value: unknown): JsonRecord {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return value as JsonRecord;
  }
  return {};
}

function asRecordList(value: unknown): JsonRecord[] {
  if (!Array.isArray(value)) return [];
  return value
    .filter((item) => item && typeof item === "object" && !Array.isArray(item))
    .map((item) => item as JsonRecord);
}

function finiteNumber(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim()) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

function textField(value: unknown): string {
  if (typeof value === "string") return value.trim();
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return "";
}

// ---------------------------------------------------------------------------
// Status → tab routing
// ---------------------------------------------------------------------------

export function evolveCandidateStatus(row: JsonRecord): string {
  return textField(row.status).toLowerCase();
}

/** Lifecycle statuses route each ArkEvolve candidate to exactly one tab. */
export function evolveCandidateTabFor(status: string): EvolutionCandidateTab | null {
  switch (status.trim().toLowerCase()) {
    case "surfaced":
      return "review";
    case "approved":
    case "testing":
      return "testing";
    case "deployed":
      return "deployed";
    case "dismissed":
    case "reverted":
    case "rejected":
      return "history";
    default:
      return null;
  }
}

/** Rows for one tab, newest activity first. */
export function filterEvolveCandidatesForTab(
  rows: JsonRecord[],
  tab: EvolutionCandidateTab,
): JsonRecord[] {
  return rows
    .filter((row) => evolveCandidateTabFor(evolveCandidateStatus(row)) === tab)
    .sort((a, b) => {
      const left = textField(b.updated_at) || textField(b.created_at);
      const right = textField(a.updated_at) || textField(a.created_at);
      return left.localeCompare(right);
    });
}

export function evolveCandidateCountForTab(
  rows: JsonRecord[],
  tab: EvolutionCandidateTab,
): number {
  return rows.filter(
    (row) => evolveCandidateTabFor(evolveCandidateStatus(row)) === tab,
  ).length;
}

export function evolveCandidateStatusPresentation(status: string): {
  label: string;
  tone: StatTone;
} {
  switch (status.trim().toLowerCase()) {
    case "surfaced":
      return { label: "Needs decision", tone: "warn" };
    case "approved":
      return { label: "Queued for optimization", tone: "info" };
    case "testing":
      return { label: "Testing on real usage", tone: "info" };
    case "deployed":
      return { label: "Deployed", tone: "good" };
    case "dismissed":
      return { label: "Dismissed", tone: "default" };
    case "reverted":
      return { label: "Auto-reverted", tone: "warn" };
    case "rejected":
      return { label: "Judged not useful", tone: "default" };
    default:
      return { label: "Tracked", tone: "default" };
  }
}

/** "from your <segment_label>" without doubling a leading "your". */
export function evolveCandidateSegmentChipLabel(row: JsonRecord): string {
  const label = textField(row.segment_label);
  if (!label) return "from your recent usage";
  return /^your\b/i.test(label) ? `from ${label}` : `from your ${label}`;
}

export function evolveCandidateKindLabel(row: JsonRecord): string {
  const key = textField(row.miner_key);
  if (!key) return "Evolve candidate";
  const known: Record<string, string> = {
    token_hotspot: "Token saving",
    latency_hotspot: "Latency saving",
    failure_cluster: "Correction reduction",
    repeat_pattern: "Learned behavior",
    router_miss: "Routing/tool selection",
    operation_contract_repair: "Tool contract repair",
  };
  if (known[key]) return known[key];
  const normalized = key.replace(/[_-]+/g, " ").replace(/\s+/g, " ").trim();
  if (!normalized) return "Evolve candidate";
  return normalized.replace(/\b\w/g, (ch) => ch.toUpperCase());
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

export function formatCompactCount(value: number): string {
  const rounded = Math.round(Math.abs(value));
  const sign = value < 0 ? "-" : "";
  if (rounded >= 10_000) {
    return `${sign}${(rounded / 1000).toFixed(rounded >= 100_000 ? 0 : 1)}k`;
  }
  if (rounded >= 1_000) return `${sign}${(rounded / 1000).toFixed(1)}k`;
  return `${sign}${rounded.toLocaleString()}`;
}

export function formatTokensPerTurn(value: number): string {
  return `~${formatCompactCount(value)} tokens/turn`;
}

export function formatDurationMs(value: number): string {
  const abs = Math.abs(value);
  const sign = value < 0 ? "-" : "";
  if (abs >= 60_000) return `${sign}${(abs / 60_000).toFixed(1)} min`;
  if (abs >= 1_000) return `${sign}${(abs / 1_000).toFixed(1)}s`;
  return `${sign}${Math.round(abs).toLocaleString()} ms`;
}

export function formatCostMicrousd(value: number): string {
  const usd = value / 1_000_000;
  const sign = usd < 0 ? "-" : "";
  return `${sign}$${Math.abs(usd).toFixed(4)}`;
}

export function formatPercent01(value: number, digits = 0): string {
  return `${(Math.max(0, Math.min(1, value)) * 100).toFixed(digits)}%`;
}

export function formatSignedPoints(value: number, digits = 1): string {
  const points = value * 100;
  return `${points > 0 ? "+" : ""}${points.toFixed(digits)} pts`;
}

// ---------------------------------------------------------------------------
// Expected benefit + evidence
// ---------------------------------------------------------------------------

export type BenefitDimension = {
  key: "tokens" | "latency" | "corrected_rate";
  label: string;
  value: string;
  helper: string;
  tone: StatTone;
  /** Emphasized number for stat tiles, with its dimmed unit. */
  accent: string;
  unit: string;
};

/**
 * Expected-benefit tiles for the dimensions that actually carry evidence.
 * tokens_per_turn / ms_per_turn are projected per-turn savings;
 * corrected_rate_delta is a projected change (negative = fewer corrections).
 */
export function expectedBenefitDimensions(row: JsonRecord): BenefitDimension[] {
  const benefit = asRecord(row.expected_benefit_json);
  const dimensions: BenefitDimension[] = [];
  const tokens = finiteNumber(benefit.tokens_per_turn);
  if (tokens != null) {
    dimensions.push({
      key: "tokens",
      label: "Tokens",
      value: formatTokensPerTurn(tokens),
      helper: "Projected saving",
      tone: tokens > 0 ? "good" : "info",
      accent: `~${formatCompactCount(tokens)}`,
      unit: "tok/turn",
    });
  }
  const ms = finiteNumber(benefit.ms_per_turn);
  if (ms != null) {
    dimensions.push({
      key: "latency",
      label: "Latency",
      value: `~${formatDurationMs(ms)}/turn`,
      helper: "Projected saving",
      tone: ms > 0 ? "good" : "info",
      accent: `~${formatDurationMs(ms)}`,
      unit: "/turn",
    });
  }
  const corrected = finiteNumber(benefit.corrected_rate_delta);
  if (corrected != null) {
    const points = corrected * 100;
    dimensions.push({
      key: "corrected_rate",
      label: "Corrected rate",
      value: formatSignedPoints(corrected),
      helper: "Projected change",
      tone: corrected < 0 ? "good" : "info",
      accent: `${points > 0 ? "+" : ""}${points.toFixed(1)}`,
      unit: "pts",
    });
  }
  return dimensions;
}

export function evolveCandidateConfidence(row: JsonRecord): number | null {
  const confidence = finiteNumber(asRecord(row.expected_benefit_json).confidence);
  if (confidence == null) return null;
  return Math.max(0, Math.min(1, confidence));
}

export type EvolveCandidateEvidenceView = {
  sampleRuns: number | null;
  correctedRuns: number | null;
  correctedRate: number | null;
  avgTokensPerTurn: number | null;
  p95WallMs: number | null;
  avgCostMicrousd: number | null;
  windowRuns: number | null;
  exampleRunCount: number;
};

export function evolveCandidateEvidenceView(row: JsonRecord): EvolveCandidateEvidenceView {
  const evidence = asRecord(row.evidence_json);
  const exampleRunIds = Array.isArray(evidence.example_run_ids)
    ? evidence.example_run_ids.filter((id) => textField(id))
    : [];
  return {
    sampleRuns: finiteNumber(evidence.sample_runs),
    correctedRuns: finiteNumber(evidence.corrected_runs),
    correctedRate: finiteNumber(evidence.corrected_rate),
    avgTokensPerTurn: finiteNumber(evidence.avg_tokens_per_turn),
    p95WallMs: finiteNumber(evidence.p95_wall_ms),
    avgCostMicrousd: finiteNumber(evidence.avg_cost_microusd),
    windowRuns: finiteNumber(evidence.window_runs),
    exampleRunCount: exampleRunIds.length,
  };
}

export function evolveCandidateRiskSummary(row: JsonRecord): string {
  return textField(asRecord(row.risk_json).summary);
}

export function evolveCandidateVerdictReason(row: JsonRecord): string {
  return textField(asRecord(row.verdict_json).reason);
}

// ---------------------------------------------------------------------------
// Value ledger: measured + realized
// ---------------------------------------------------------------------------

export type MeasuredDimensionDelta = {
  key: "success" | "tokens" | "latency" | "cost";
  label: string;
  baseline: string;
  candidate: string;
  delta: string;
  /** Emphasized delta for stat tiles, with its dimmed unit. */
  accent: string;
  unit: string;
  /** null = no direction judgement possible. */
  improved: boolean | null;
};

export type MeasuredLedgerView = {
  judgedAt: string;
  baselineSamples: number | null;
  candidateSamples: number | null;
  wins: number | null;
  losses: number | null;
  pValue: number | null;
  deltas: MeasuredDimensionDelta[];
};

/** Baseline-vs-candidate deltas for every dimension both arms measured. */
export function measuredLedgerView(row: JsonRecord): MeasuredLedgerView | null {
  const measured = asRecord(asRecord(row.ledger_json).measured);
  if (Object.keys(measured).length === 0) return null;
  const baseline = asRecord(measured.baseline);
  const candidate = asRecord(measured.candidate);
  const deltas: MeasuredDimensionDelta[] = [];

  const baselineSuccess = finiteNumber(baseline.success_rate);
  const candidateSuccess = finiteNumber(candidate.success_rate);
  if (baselineSuccess != null && candidateSuccess != null) {
    const delta = candidateSuccess - baselineSuccess;
    const points = delta * 100;
    deltas.push({
      key: "success",
      label: "Success rate",
      baseline: formatPercent01(baselineSuccess, 1),
      candidate: formatPercent01(candidateSuccess, 1),
      delta: formatSignedPoints(delta),
      accent: `${points > 0 ? "+" : ""}${points.toFixed(1)}`,
      unit: "pts",
      improved: delta === 0 ? null : delta > 0,
    });
  }
  const baselineTokens = finiteNumber(baseline.avg_tokens_per_turn);
  const candidateTokens = finiteNumber(candidate.avg_tokens_per_turn);
  if (baselineTokens != null && candidateTokens != null) {
    const delta = candidateTokens - baselineTokens;
    deltas.push({
      key: "tokens",
      label: "Tokens/turn",
      baseline: formatCompactCount(baselineTokens),
      candidate: formatCompactCount(candidateTokens),
      delta: `${delta > 0 ? "+" : ""}${formatCompactCount(delta)}`,
      accent: `${delta > 0 ? "+" : ""}${formatCompactCount(delta)}`,
      unit: "tok/turn",
      improved: delta === 0 ? null : delta < 0,
    });
  }
  const baselineWall = finiteNumber(baseline.p95_wall_ms);
  const candidateWall = finiteNumber(candidate.p95_wall_ms);
  if (baselineWall != null && candidateWall != null) {
    const delta = candidateWall - baselineWall;
    deltas.push({
      key: "latency",
      label: "p95 wall time",
      baseline: formatDurationMs(baselineWall),
      candidate: formatDurationMs(candidateWall),
      delta: `${delta > 0 ? "+" : ""}${formatDurationMs(delta)}`,
      accent: `${delta > 0 ? "+" : ""}${formatDurationMs(delta)}`,
      unit: "p95",
      improved: delta === 0 ? null : delta < 0,
    });
  }
  const baselineCost = finiteNumber(baseline.avg_cost_microusd);
  const candidateCost = finiteNumber(candidate.avg_cost_microusd);
  if (baselineCost != null && candidateCost != null) {
    const delta = candidateCost - baselineCost;
    deltas.push({
      key: "cost",
      label: "Cost/turn",
      baseline: formatCostMicrousd(baselineCost),
      candidate: formatCostMicrousd(candidateCost),
      delta: `${delta > 0 ? "+" : ""}${formatCostMicrousd(delta)}`,
      accent: `${delta > 0 ? "+" : ""}${formatCostMicrousd(delta)}`,
      unit: "/turn",
      improved: delta === 0 ? null : delta < 0,
    });
  }

  return {
    judgedAt: textField(measured.judged_at),
    baselineSamples: finiteNumber(baseline.samples),
    candidateSamples: finiteNumber(candidate.samples),
    wins: finiteNumber(measured.wins),
    losses: finiteNumber(measured.losses),
    pValue: finiteNumber(measured.p_value),
    deltas,
  };
}

export type RealizedLedgerView = {
  promotedAt: string;
  surface: string;
  candidateVersion: string;
};

export function realizedLedgerView(row: JsonRecord): RealizedLedgerView | null {
  const realized = asRecord(asRecord(row.ledger_json).realized);
  if (Object.keys(realized).length === 0) return null;
  return {
    promotedAt: textField(realized.promoted_at),
    surface: textField(realized.surface),
    candidateVersion: textField(realized.candidate_version),
  };
}

// ---------------------------------------------------------------------------
// Aggregate realized savings (Deployed headline)
// ---------------------------------------------------------------------------

export type RealizedSavings = {
  /** Per-turn savings summed across deployed candidates; null = no data. */
  tokensPerTurn: number | null;
  msPerTurn: number | null;
  costMicrousdPerTurn: number | null;
  measuredCount: number;
  segmentLabels: string[];
};

/**
 * Sum measured baseline-minus-candidate per-turn deltas across deployed
 * candidates. Positive = saving. Dimensions only count when both arms
 * carry data (NULL metrics are "no evidence", never zero).
 */
export function aggregateRealizedSavings(rows: JsonRecord[]): RealizedSavings {
  let tokens: number | null = null;
  let ms: number | null = null;
  let cost: number | null = null;
  let measuredCount = 0;
  const segmentLabels: string[] = [];
  for (const row of rows) {
    if (evolveCandidateStatus(row) !== "deployed") continue;
    const measured = asRecord(asRecord(row.ledger_json).measured);
    if (Object.keys(measured).length === 0) continue;
    const baseline = asRecord(measured.baseline);
    const candidate = asRecord(measured.candidate);
    let contributed = false;
    const baselineTokens = finiteNumber(baseline.avg_tokens_per_turn);
    const candidateTokens = finiteNumber(candidate.avg_tokens_per_turn);
    if (baselineTokens != null && candidateTokens != null) {
      tokens = (tokens ?? 0) + (baselineTokens - candidateTokens);
      contributed = true;
    }
    const baselineWall = finiteNumber(baseline.p95_wall_ms);
    const candidateWall = finiteNumber(candidate.p95_wall_ms);
    if (baselineWall != null && candidateWall != null) {
      ms = (ms ?? 0) + (baselineWall - candidateWall);
      contributed = true;
    }
    const baselineCost = finiteNumber(baseline.avg_cost_microusd);
    const candidateCost = finiteNumber(candidate.avg_cost_microusd);
    if (baselineCost != null && candidateCost != null) {
      cost = (cost ?? 0) + (baselineCost - candidateCost);
      contributed = true;
    }
    if (contributed) {
      measuredCount += 1;
      const label = textField(row.segment_label);
      if (label && !segmentLabels.includes(label)) segmentLabels.push(label);
    }
  }
  return {
    tokensPerTurn: tokens,
    msPerTurn: ms,
    costMicrousdPerTurn: cost,
    measuredCount,
    segmentLabels,
  };
}

/** Human parts for the dimensions that have data, e.g. "~1.2k tokens/turn". */
export function realizedSavingsParts(savings: RealizedSavings): string[] {
  const parts: string[] = [];
  if (savings.tokensPerTurn != null) {
    parts.push(formatTokensPerTurn(savings.tokensPerTurn));
  }
  if (savings.msPerTurn != null) {
    parts.push(`~${formatDurationMs(savings.msPerTurn)}/turn`);
  }
  if (savings.costMicrousdPerTurn != null) {
    parts.push(`~${formatCostMicrousd(savings.costMicrousdPerTurn)}/turn`);
  }
  return parts;
}

/** Third-person headline; null when no measured dimension has data. */
export function realizedSavingsHeadline(savings: RealizedSavings): string | null {
  const parts = realizedSavingsParts(savings);
  if (parts.length === 0) return null;
  const joined =
    parts.length === 1
      ? parts[0]
      : `${parts.slice(0, -1).join(", ")} and ${parts[parts.length - 1]}`;
  const where =
    savings.segmentLabels.length === 1
      ? /^your\b/i.test(savings.segmentLabels[0])
        ? ` in ${savings.segmentLabels[0]}`
        : ` in your ${savings.segmentLabels[0]}`
      : savings.segmentLabels.length > 1
        ? ` across ${savings.segmentLabels.length} usage segments`
        : "";
  return `ArkEvolve is saving ${joined}${where}.`;
}

// ---------------------------------------------------------------------------
// Stable-promotion recommendation + snapshot history
// ---------------------------------------------------------------------------

export type StableRecommendationView = {
  recommendedAt: string;
  baselineVersion: string;
  candidateVersion: string;
  successGain: number | null;
  wins: number | null;
  losses: number | null;
  pValue: number | null;
  baselineSamples: number | null;
  candidateSamples: number | null;
};

export function stableRecommendationView(
  value: unknown,
): StableRecommendationView | null {
  const record = asRecord(value);
  if (record.recommended !== true) return null;
  const evaluation = asRecord(record.evaluation);
  return {
    recommendedAt: textField(record.recommended_at),
    baselineVersion: textField(evaluation.baseline_version),
    candidateVersion: textField(evaluation.candidate_version),
    successGain: finiteNumber(evaluation.success_gain),
    wins: finiteNumber(evaluation.wins),
    losses: finiteNumber(evaluation.losses),
    pValue: finiteNumber(evaluation.p_value),
    baselineSamples: finiteNumber(asRecord(evaluation.baseline).samples),
    candidateSamples: finiteNumber(asRecord(evaluation.candidate).samples),
  };
}

export type SnapshotHistoryRow = { version: string; savedAt: string };

export function snapshotHistoryRows(value: unknown): SnapshotHistoryRow[] {
  return asRecordList(value)
    .map((entry) => ({
      version: textField(entry.version),
      savedAt: textField(entry.saved_at),
    }))
    .filter((entry) => entry.version);
}

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

export type PageSlice<T> = {
  items: T[];
  page: number;
  pageCount: number;
  total: number;
  rangeStart: number;
  rangeEnd: number;
};

export function pageSlice<T>(
  rows: T[],
  requestedPage: number,
  pageSize: number,
): PageSlice<T> {
  const size = Math.max(1, Math.floor(pageSize));
  const pageCount = Math.max(1, Math.ceil(rows.length / size));
  const page = Math.max(0, Math.min(requestedPage, pageCount - 1));
  const items = rows.slice(page * size, page * size + size);
  return {
    items,
    page,
    pageCount,
    total: rows.length,
    rangeStart: rows.length === 0 ? 0 : page * size + 1,
    rangeEnd: Math.min((page + 1) * size, rows.length),
  };
}
