import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

const testDir = path.dirname(fileURLToPath(import.meta.url));
const frontendRoot = path.resolve(testDir, "..");
const outDir = mkdtempSync(path.join(tmpdir(), "agentark-evolution-candidates-"));

execFileSync(
  process.execPath,
  [
    path.join(frontendRoot, "node_modules", "typescript", "bin", "tsc"),
    "src/components/pages/evolutionCandidateHelpers.ts",
    "--ignoreConfig",
    "--target",
    "ES2020",
    "--module",
    "ES2020",
    "--moduleResolution",
    "Bundler",
    "--outDir",
    outDir,
    "--skipLibCheck",
  ],
  { cwd: frontendRoot, stdio: "inherit" },
);
writeFileSync(path.join(outDir, "package.json"), JSON.stringify({ type: "module" }));

const {
  aggregateRealizedSavings,
  expectedBenefitDimensions,
  filterEvolveCandidatesForTab,
  measuredLedgerView,
  evolveCandidateConfidence,
  evolveCandidateCountForTab,
  evolveCandidateEvidenceView,
  evolveCandidateKindLabel,
  evolveCandidateSegmentChipLabel,
  evolveCandidateStatusPresentation,
  evolveCandidateTabFor,
  pageSlice,
  realizedLedgerView,
  realizedSavingsHeadline,
  snapshotHistoryRows,
  stableRecommendationView,
} = await import(
  pathToFileURL(path.join(outDir, "evolutionCandidateHelpers.js")).toString()
);

function candidate(overrides = {}) {
  return {
    id: "evolve-opp-abc123",
    miner_key: "token_hotspot",
    status: "surfaced",
    title: "Trim repeated context in long analysis turns",
    description: "These turns carry far more prompt than their outcomes need.",
    segment_label: "your CSV analysis sessions",
    segment_key: "intent:analyze-csv",
    target_surface: "prompt",
    evidence_json: {
      sample_runs: 42,
      corrected_runs: 6,
      corrected_rate: 0.142857,
      avg_tokens_per_turn: 1840.5,
      p95_wall_ms: 12400,
      avg_cost_microusd: 950.0,
      example_run_ids: ["run-1", "run-2", "run-3"],
      window_runs: 480,
    },
    expected_benefit_json: {
      tokens_per_turn: 620.0,
      ms_per_turn: null,
      corrected_rate_delta: -0.03,
      confidence: 0.7,
    },
    risk_json: { summary: "Prompt bundle only; canary with auto-revert." },
    holdout_run_ids_json: ["run-9"],
    verdict_json: {
      useful: true,
      reason: "Material token excess concentrated in one segment.",
      judged_at: "2026-06-10T01:00:00Z",
      topic_text: "csv analysis",
    },
    ledger_json: {},
    gepa_job_id: null,
    decided_at: null,
    created_at: "2026-06-09T10:00:00Z",
    updated_at: "2026-06-09T10:00:00Z",
    ...overrides,
  };
}

const MEASURED = {
  judged_at: "2026-06-10T02:00:00Z",
  baseline: {
    samples: 30,
    successes: 24,
    success_rate: 0.8,
    p95_latency_ms: 9000,
    avg_tokens_per_turn: 1800,
    p95_wall_ms: 12000,
    avg_cost_microusd: 900,
  },
  candidate: {
    samples: 28,
    successes: 24,
    success_rate: 0.857,
    p95_latency_ms: 8000,
    avg_tokens_per_turn: 1460,
    p95_wall_ms: 9000,
    avg_cost_microusd: 700,
  },
  success_gain: 0.057,
  wins: 9,
  losses: 3,
  p_value: 0.073,
};

test("statuses route candidates to exactly one workbench tab", () => {
  assert.equal(evolveCandidateTabFor("surfaced"), "review");
  assert.equal(evolveCandidateTabFor("approved"), "testing");
  assert.equal(evolveCandidateTabFor("testing"), "testing");
  assert.equal(evolveCandidateTabFor("deployed"), "deployed");
  assert.equal(evolveCandidateTabFor("dismissed"), "history");
  assert.equal(evolveCandidateTabFor("reverted"), "history");
  assert.equal(evolveCandidateTabFor("rejected"), "history");
  assert.equal(evolveCandidateTabFor("mined"), null);
  assert.equal(evolveCandidateTabFor("  Surfaced "), "review");
});

test("filterEvolveCandidatesForTab selects by status and sorts newest first", () => {
  const rows = [
    candidate({ id: "a", status: "surfaced", updated_at: "2026-06-08T00:00:00Z" }),
    candidate({ id: "b", status: "deployed", updated_at: "2026-06-09T00:00:00Z" }),
    candidate({ id: "c", status: "surfaced", updated_at: "2026-06-10T00:00:00Z" }),
    candidate({ id: "d", status: "dismissed", updated_at: "2026-06-07T00:00:00Z" }),
  ];
  const surfaced = filterEvolveCandidatesForTab(rows, "review");
  assert.deepEqual(
    surfaced.map((row) => row.id),
    ["c", "a"],
  );
  assert.deepEqual(
    filterEvolveCandidatesForTab(rows, "history").map((row) => row.id),
    ["d"],
  );
  assert.equal(evolveCandidateCountForTab(rows, "deployed"), 1);
});

test("status presentation maps lifecycle to label and tone, never raw strings", () => {
  assert.deepEqual(evolveCandidateStatusPresentation("surfaced"), {
    label: "Needs decision",
    tone: "warn",
  });
  assert.deepEqual(evolveCandidateStatusPresentation("deployed"), {
    label: "Deployed",
    tone: "good",
  });
  assert.deepEqual(evolveCandidateStatusPresentation("reverted"), {
    label: "Auto-reverted",
    tone: "warn",
  });
  assert.deepEqual(evolveCandidateStatusPresentation("unknown_thing"), {
    label: "Tracked",
    tone: "default",
  });
});

test("segment chip never doubles a leading 'your'", () => {
  assert.equal(
    evolveCandidateSegmentChipLabel(candidate()),
    "from your CSV analysis sessions",
  );
  assert.equal(
    evolveCandidateSegmentChipLabel(candidate({ segment_label: "long research threads" })),
    "from your long research threads",
  );
  assert.equal(
    evolveCandidateSegmentChipLabel(candidate({ segment_label: "" })),
    "from your recent usage",
  );
});

test("candidate kind label comes from evolve miner key", () => {
  assert.equal(evolveCandidateKindLabel(candidate()), "Token saving");
  assert.equal(
    evolveCandidateKindLabel(candidate({ miner_key: "operation_contract_repair" })),
    "Tool contract repair",
  );
  assert.equal(evolveCandidateKindLabel(candidate({ miner_key: "" })), "Evolve candidate");
});

test("expected benefit only includes dimensions with evidence", () => {
  const dims = expectedBenefitDimensions(candidate());
  assert.deepEqual(
    dims.map((dim) => dim.key),
    ["tokens", "corrected_rate"],
  );
  const tokens = dims.find((dim) => dim.key === "tokens");
  assert.equal(tokens.tone, "good");
  assert.equal(tokens.unit, "tok/turn");
  const corrected = dims.find((dim) => dim.key === "corrected_rate");
  assert.equal(corrected.tone, "good");
  assert.match(corrected.value, /-3\.0 pts/);

  const empty = expectedBenefitDimensions(
    candidate({ expected_benefit_json: { confidence: 0.2 } }),
  );
  assert.equal(empty.length, 0);
  assert.equal(
    evolveCandidateConfidence(candidate({ expected_benefit_json: { confidence: 0.2 } })),
    0.2,
  );
  assert.equal(
    evolveCandidateConfidence(candidate({ expected_benefit_json: {} })),
    null,
  );
});

test("evidence view treats missing metrics as no evidence, never zero", () => {
  const view = evolveCandidateEvidenceView(
    candidate({
      evidence_json: {
        sample_runs: 10,
        corrected_runs: 2,
        corrected_rate: 0.2,
        example_run_ids: [],
        window_runs: 100,
      },
    }),
  );
  assert.equal(view.sampleRuns, 10);
  assert.equal(view.avgTokensPerTurn, null);
  assert.equal(view.p95WallMs, null);
  assert.equal(view.avgCostMicrousd, null);
  assert.equal(view.exampleRunCount, 0);
});

test("measured ledger view computes baseline-vs-candidate deltas per dimension", () => {
  assert.equal(measuredLedgerView(candidate()), null);

  const view = measuredLedgerView(
    candidate({ status: "testing", ledger_json: { measured: MEASURED } }),
  );
  assert.equal(view.wins, 9);
  assert.equal(view.losses, 3);
  assert.deepEqual(
    view.deltas.map((delta) => delta.key),
    ["success", "tokens", "latency", "cost"],
  );
  const success = view.deltas.find((delta) => delta.key === "success");
  assert.equal(success.improved, true);
  const tokens = view.deltas.find((delta) => delta.key === "tokens");
  assert.equal(tokens.improved, true);
  assert.match(tokens.delta, /^-340/);
});

test("measured deltas skip dimensions missing on either arm", () => {
  const measured = {
    baseline: { samples: 20, successes: 16, success_rate: 0.8 },
    candidate: {
      samples: 20,
      successes: 14,
      success_rate: 0.7,
      avg_tokens_per_turn: 1200,
    },
    wins: 2,
    losses: 6,
    p_value: 0.4,
  };
  const view = measuredLedgerView(
    candidate({ status: "testing", ledger_json: { measured } }),
  );
  assert.deepEqual(
    view.deltas.map((delta) => delta.key),
    ["success"],
  );
  assert.equal(view.deltas[0].improved, false);
});

test("realized ledger view reads promotion facts", () => {
  assert.equal(realizedLedgerView(candidate()), null);
  const view = realizedLedgerView(
    candidate({
      status: "deployed",
      ledger_json: {
        realized: {
          promoted_at: "2026-06-10T03:00:00Z",
          surface: "prompt",
          candidate_version: "prompt-v42",
        },
      },
    }),
  );
  assert.equal(view.surface, "prompt");
  assert.equal(view.candidateVersion, "prompt-v42");
});

test("realized savings aggregate only deployed rows with measured evidence", () => {
  const rows = [
    candidate({
      id: "deployed-measured",
      status: "deployed",
      segment_label: "your CSV analysis sessions",
      ledger_json: { measured: MEASURED },
    }),
    candidate({
      id: "deployed-unmeasured",
      status: "deployed",
      ledger_json: { realized: { promoted_at: "2026-06-10T03:00:00Z" } },
    }),
    candidate({
      id: "testing-measured",
      status: "testing",
      ledger_json: { measured: MEASURED },
    }),
  ];
  const savings = aggregateRealizedSavings(rows);
  assert.equal(savings.measuredCount, 1);
  assert.equal(Math.round(savings.tokensPerTurn), 340);
  assert.equal(Math.round(savings.msPerTurn), 3000);
  assert.equal(Math.round(savings.costMicrousdPerTurn), 200);
  assert.deepEqual(savings.segmentLabels, ["your CSV analysis sessions"]);

  const headline = realizedSavingsHeadline(savings);
  assert.match(headline, /^ArkEvolve is saving /);
  assert.match(headline, /tokens\/turn/);
  assert.match(headline, /in your CSV analysis sessions/);
  assert.doesNotMatch(headline, /your your/);
});

test("savings headline is null when no dimension has data", () => {
  const savings = aggregateRealizedSavings([
    candidate({ status: "deployed", ledger_json: {} }),
  ]);
  assert.equal(savings.tokensPerTurn, null);
  assert.equal(savings.msPerTurn, null);
  assert.equal(savings.costMicrousdPerTurn, null);
  assert.equal(realizedSavingsHeadline(savings), null);
});

test("partial measured dims aggregate without inventing the missing ones", () => {
  const savings = aggregateRealizedSavings([
    candidate({
      status: "deployed",
      ledger_json: {
        measured: {
          baseline: { samples: 10, successes: 9, success_rate: 0.9, avg_tokens_per_turn: 1000 },
          candidate: { samples: 10, successes: 9, success_rate: 0.9, avg_tokens_per_turn: 900 },
        },
      },
    }),
  ]);
  assert.equal(Math.round(savings.tokensPerTurn), 100);
  assert.equal(savings.msPerTurn, null);
  assert.equal(savings.costMicrousdPerTurn, null);
});

test("stable recommendation view requires an affirmative recommendation", () => {
  assert.equal(stableRecommendationView(null), null);
  assert.equal(stableRecommendationView({ recommended: false }), null);
  const view = stableRecommendationView({
    recommended: true,
    recommended_at: "2026-06-10T04:00:00Z",
    evaluation: {
      baseline_version: "prompt-v41",
      candidate_version: "prompt-v42",
      baseline: MEASURED.baseline,
      candidate: MEASURED.candidate,
      success_gain: 0.057,
      wins: 9,
      losses: 3,
      p_value: 0.073,
    },
  });
  assert.equal(view.candidateVersion, "prompt-v42");
  assert.equal(view.wins, 9);
  assert.equal(view.baselineSamples, 30);
  assert.equal(view.pValue, 0.073);
});

test("snapshot history rows keep only versioned entries", () => {
  const rows = snapshotHistoryRows([
    { version: "prompt-v42", saved_at: "2026-06-10T03:00:00Z" },
    { version: "", saved_at: "2026-06-09T03:00:00Z" },
    { saved_at: "2026-06-08T03:00:00Z" },
    "garbage",
  ]);
  assert.deepEqual(rows, [
    { version: "prompt-v42", savedAt: "2026-06-10T03:00:00Z" },
  ]);
  assert.deepEqual(snapshotHistoryRows(null), []);
});

test("pageSlice clamps out-of-range pages and reports ranges", () => {
  const rows = Array.from({ length: 13 }, (_unused, i) => i);
  const first = pageSlice(rows, 0, 6);
  assert.deepEqual(first.items, [0, 1, 2, 3, 4, 5]);
  assert.equal(first.pageCount, 3);
  assert.equal(first.rangeStart, 1);
  assert.equal(first.rangeEnd, 6);

  const clamped = pageSlice(rows, 99, 6);
  assert.equal(clamped.page, 2);
  assert.deepEqual(clamped.items, [12]);
  assert.equal(clamped.rangeStart, 13);
  assert.equal(clamped.rangeEnd, 13);

  const empty = pageSlice([], 0, 6);
  assert.equal(empty.pageCount, 1);
  assert.equal(empty.rangeStart, 0);
  assert.equal(empty.rangeEnd, 0);
});
