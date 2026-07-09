# Evolve

Evolve is the self-improvement subsystem inside AgentArk. It watches the agent's own runs, proposes mutations to the prompts, policies, and routing data that drive behaviour, evaluates each candidate against benchmarks and replay evidence, and either rolls a winner forward through a canary or rejects it with a typed reason. It runs in the Rust control plane next to the agent loop, with one optional offline bridge to a Python optimizer.

## Overview

The goal of Evolve is to close the loop between what the agent did, how that turned out, and what the agent does next. Every mutable learning surface the agent uses at runtime — router decision prompt, primary response prompt, delegated-agent prompts, prompt fragments, routing complexity policy, routing canonical overlay, and tool strategy guidance — has a profile stored in the key-value store. Evolve reads recent operational evidence, generates candidate profiles, scores them against embedded benchmarks plus replay evidence, runs a typed promotion gate, and (if a gate passes) promotes the candidate through a sticky canary rollout. Skills are separately designed or installed capability packages, not Evolve learning artifacts. The runtime never reads "the latest prompt" from a free-form blob; it always reads a versioned profile that Evolve owns.

## What evolves

Each evolver mutates a specific surface. The list below is exhaustive for the current `src/core/self_evolve/` tree.

- **Router / primary-response / delegation-synthesis prompt bundle** — `src/core/self_evolve/prompt_evolution.rs`. Three mutable surfaces (`router`, `primary_response`, `delegation_synthesis`), each with a system prompt, policy block, and instruction template. Aggregated by a weighted score (`ROUTER_WEIGHT = 0.35`, `PRIMARY_RESPONSE_WEIGHT = 0.40`, `SYNTHESIS_WEIGHT = 0.25`).
- **Specialist prompt bundle** — `src/core/self_evolve/specialist_prompt_evolution.rs`. The six built-in delegated roles: `researcher`, `coder`, `analyst`, `writer`, `validator`, `planner`. Custom specialist overrides are preserved and not mutated.
- **Prompt fragment bundle** — `src/core/self_evolve/prompt_fragment_evolution.rs`. Toggleable system-prompt fragments with token-budget invariants (rejects > 15% token regression on enabled / always-on fragments).
- **Routing complexity policy** — `src/core/self_evolve/policy_evolution.rs`. The `RoutingComplexityPolicy` thresholds (`complex_score_threshold`, `medium_score_threshold`, weight terms) that classify a message into `Simple | Medium | Complex` before routing.
- **Routing canonical overlay** — `src/core/self_evolve/routing_canonical_evolution.rs`. Adds and removes semantic descriptors in the `data/security/canonicals.json` overlay (categories: `direct_reply`, `tool_use`, `durable_work`, `managed_app_delivery`, `security_block`). Treats canonicals as data, not prompt text.
- **Tool strategy profile** — `src/core/self_evolve/strategy_runtime.rs`. Per-task guidance lines that ride alongside tool calls (`TOOL_STRATEGY_PROFILE_KEY`).
These prompt/policy paths are the inbound-routing-relevant set. Evolvers like `argument_repair` and `task_router` are not housed here; they are runtime modules that emit evidence Evolve consumes. Skills are managed outside Evolve as manual capability modules.

## The signal: where evolution data comes from

Every evolver reads from `src/storage/` entities populated by the agent loop and channels:

- `experience_run` — one row per turn: `success_state` (`accepted` / `failed` / `corrected` / ...), `correction_state`, `tool_sequence_digest`, `prompt_version`, `policy_version`, `strategy_version`, `model_slot`, redacted `request_text`, `failure_reason`, plus token / cost usage. This is the core acceptance / correction signal.
- `experience_item` — consolidated outcomes after reflection. Used as the "what worked" sample set.
- `procedural_pattern` — repeated tool sequences with success counters.
- `learning_candidate` — the typed candidate object. Evolve only operates on candidates with a known `candidate_type` (e.g. `routing_canonical`); the schema is enforced before promotion (`src/core/self_evolve/routing_canonical_evolution.rs`, `parse_routing_canonical_candidate`).
- The KV store (`Storage::get/set`) holds the live profiles, baseline snapshots, canary state, last-result records, and the GEPA budget ledger. Keys are listed in `strategy_runtime.rs`, `prompt_evolution.rs`, and `gepa_bridge.rs`.

In addition, each evolver writes a JSONL **lineage archive** under `.agentark/self_evolve/`: `prompt_bundle_lineage.jsonl`, `routing_policy_lineage.jsonl`, `specialist_prompt_bundle_lineage.jsonl`, `prompt_fragment_bundle_lineage.jsonl`. These archives are pruned to `MAX_LINEAGE_ARCHIVE_ENTRIES = 400`.

## Mutation pipeline

The shape is the same for every evolver:

1. **Snapshot baseline.** Read the current profile and persist it under the `*_BASELINE_SNAPSHOT_KEY` so a rollback target always exists.
2. **Generate candidates.** A small pool of mutated profiles is produced. The mutators are LLM-driven and trace-conditioned: the engine assembles recent failed and corrected `experience_run` rows, asks the configured LLM to propose a new surface, and adds it to the pool. Static mutator strings (e.g. `ROUTER_DIRECTNESS_MUTATION`, `BOUNDED_SCOPE_MUTATION`) seed direction without freezing the output. Externally proposed candidates can also enter the pool through the GEPA bridge import path.
3. **Evaluate.** Each candidate is scored against an embedded benchmark profile (`benchmarks/prompt_bundle_benchmark_v1.json`, `benchmarks/specialist_prompt_benchmark_v1.json`, `benchmarks/routing_benchmark_v1.json`). Per-case scores are combined into a single bundle score using a **weighted scalar**, with router invalid-JSON rate, prompt token regression ratio, and cache-sensitive token regression ratio tracked as side metrics. See `evaluate_bundle()` at roughly `prompt_evolution.rs:960` and the weight constants near line 48.
4. **Promotion gate.** The best candidate is sent through a typed `PromotionGateReport` (see next section). On rejection it is logged into lineage with reasons and discarded.
5. **Canary.** A passing candidate is promoted into a `CanaryRolloutState` (`strategy_runtime.rs`) — sticky per-conversation, defaulting to `rollout_percent = 20`, `min_samples_per_version = 25`, `min_success_gain = 0.03`, `max_sign_test_p_value = 0.10`. The replay evaluator measures live samples for both arms and either activates the candidate as the new baseline or aborts the canary back to the snapshot.
6. **Rollback.** Any aborted canary, blocking security finding, or replay regression restores the `*_BASELINE_SNAPSHOT_KEY` profile. The agent loop reads the live key without caring whether the last hop was a promotion or a rollback.

**GEPA-style optimization, current state.** Evolve's mutator and proposer paths are LLM- and trace-driven (see `prompt_evolution.rs:337`, `policy_evolution.rs:444`, `specialist_prompt_evolution.rs:477` for the proposal entry points), which is the GEPA-style move. Selection across the candidate pool, however, is still a **weighted scalar over benchmark + side metrics**, not a Pareto-frontier search. The objectives (router accuracy, primary-response quality, synthesis quality, token cost, invalid-JSON rate) are visible per candidate but the engine collapses them into a single comparable number before gating. Treat that as the open gap if you are evaluating Evolve against a textbook GEPA loop.

## Promotion gates

Two gates run at different points in the lifecycle:

- **Replay / evidence gate** — `src/core/self_evolve/replay_gate.rs`. Decides whether a `learning_candidate` row may even be presented for approval. Inputs are **structured runtime evidence**, never user phrasing: `evidence_refs` count, `experience_run.success_state` distribution, correction rate, procedural pattern counts, memory-item counts, and PII-redaction state. Defaults: `MIN_EVIDENCE_SAMPLES = 2`, `MIN_CONFIDENCE = 0.35`, `MIN_SUPPORT_SCORE = 0.50`, `MAX_CORRECTION_RATE = 0.45`. Sensitive runs are excluded; PII-redacted runs are counted but flagged. The output is a `CandidateReplayGateResult` with a status, an `allow_approval` boolean, and a free-text reason.
- **Promotion gate** — `src/core/self_evolve/promotion_gate.rs`. Decides whether a benchmark-passing candidate becomes the new baseline. Returns a `PromotionGateReport { outcome: Passed | Rejected, summary, reasons: Vec<PromotionGateReason> }`. Each `PromotionGateReason` has a stable `code` (e.g. `min_accuracy_gain`, `prompt_token_regression`, `router_invalid_json_rate`, `cache_sensitive_token_regression`) and a human label; the UI renders the labels and the `code`s let other systems route on the failure type.

A typical rejected report looks like `Not promoted: minimum accuracy gain not reached; prompt token regression too high.` with both reasons attached.

## UI: the Evolution page

The page lives at `frontend/src/components/pages/EvolutionPage.tsx` and renders under "Evolve" with `WorkspacePageHeader`. By default it shows a **simplified view** — three lines of plain prose ("Evolve is on/off", "N active experiments", "M changes waiting on you", "K confirmed improvements so far") plus an Active experiments card if any are running. A `Show Evolve internals` switch in the header reveals the full surface.

When internals are on, the page renders:

- A **stat strip** (`EvolutionStatStrip` in `traceEvolutionHelpers.tsx`, line ~1755): improvement mode (on/off), active experiments and the maximum rollout percent currently in test, count of changes needing approval.
- A **Background improvement** panel that surfaces the GEPA queue state (running, pending, last result) and a readiness chip.
- A **rollout bar** (`EvolutionRolloutBar`) per active experiment showing baseline vs candidate samples and the current rollout percent.
- Four tabs (`EVOLUTION_PAGE_TABS`): `Overview`, `Results`, `Live tests`, `Review queue`. The Review queue is where candidates whose replay gate passed are presented for approval; other tabs cover historic evidence, the running experiment list, and per-run impact summaries.
- The experiment list (`activeExperimentItems`, line ~754) — each row is one canary, with its baseline version, candidate version, rollout percent, sample counts per arm, and current status label.

The user can: toggle Evolve on or off (writes through to `SELF_EVOLVE_ENABLED_KEY`), approve or reject a queued candidate, force-abort an active canary, and inspect the underlying `experience_run` traces tied to a candidate's evidence.

## Configuration and toggles

- `self_evolve_enabled_v1` (KV store, `SELF_EVOLVE_ENABLED_KEY` in `strategy_runtime.rs`) — global on/off. Surfaced on the Evolution page header and in `SettingsPageFull.tsx`.
- `gepa_optimizer_config_v1` — `GepaOptimizerConfig` with `enabled`, `auto_mode` (`light` | `medium` | `heavy`, default `light`), `max_metric_calls` (clamped 1..512, default 24), `daily_budget_usd` (default 1.0), `per_run_budget_usd` (default 0.50), `max_runs_per_day` (default 1), `auto_setup` (default true).
- Environment variables consumed by the GEPA worker: `AGENTARK_GEPA_MODEL`, `AGENTARK_GEPA_AUTO`, `AGENTARK_GEPA_MAX_METRIC_CALLS`, `AGENTARK_GEPA_COST_BUDGET_USD`, plus `AGENTARK_GEPA_THREADS` (optional). Provider keys (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `OPENAI_BASE_URL`) are forwarded from the active model slot.
- The agent loop also honours `learning_enabled_v1` for upstream signal capture; with learning off, `experience_run` rows still log but reflection-driven candidates do not flow.

## Storage

Profiles, snapshots, canary state, and last-result blobs live in the KV store under typed keys. Lineage and queue artefacts live on disk under `.agentark/self_evolve/`. Evidence rows live in the relational tables.

| Where | What |
|-------|------|
| KV `prompt_bundle_profile_v1` / `..._canary_v1` / `..._baseline_snapshot_v1` / `..._canary_state_v1` / `..._last_result_v1` | Router + primary-response + synthesis prompt bundle |
| KV `specialist_prompt_bundle_profile_v1` (and friends) | Specialist roles bundle |
| KV `prompt_fragment_bundle_profile_v1` | Prompt fragments |
| KV `routing_complexity_policy_v1` (and canary / snapshot keys) | Complexity classifier policy |
| KV `tool_strategy_profile_v1` (and canary keys) | Per-task tool guidance |
| KV `gepa_optimizer_config_v1` / `gepa_optimizer_budget_ledger_v1` / `gepa_optimizer_auto_state_v1` / `gepa_optimizer_last_result_v1` | GEPA bridge state |
| `experience_run`, `experience_item`, `procedural_pattern`, `learning_candidate` | Operational evidence and typed candidates |
| `data/security/canonicals.json` | Routing canonical overlay (managed by `routing_canonical_evolution.rs`) |
| `.agentark/self_evolve/*_lineage.jsonl` | Per-evolver lineage archive (capped at 400 entries) |
| `.agentark/self_evolve/gepa/{pending,running,completed,failed,runs}` | GEPA job queue, run exports, candidate JSONL |

## GEPA bridge

`src/core/self_evolve/gepa_bridge.rs` is the Rust side of an **offline** bridge to a Python DSPy/GEPA optimizer at `bridges/gepa_optimizer/__main__.py`. The bridge is intentionally out of the hot path. Its job is to seed candidates that the in-process evolvers can then evaluate, gate, and canary using their existing logic.

The flow:

1. `export_optimization_bundle` writes a redacted bundle (current profiles, embedded benchmarks, recent lineage, and PII-redacted recent `experience_run` rows) to `.agentark/self_evolve/gepa/runs/<run_id>/export.json`. Sensitive runs are filtered (`experience_run_export_safe`).
2. A `PendingGepaJob` is dropped in the `pending/` directory. A worker claims it via `claim_next_pending_job`, moves it to `running/`, and invokes `python -m bridges.gepa_optimizer run --export ... --out candidates.jsonl` under a budget-checked `GepaOptimizerRuntime`.
3. The Python entry point (`bridges/gepa_optimizer/__main__.py`) loads DSPy, runs GEPA against the export, and writes `candidates.jsonl` with one record per candidate, each tagged with its `surface` (`prompt_bundle`, `specialist_prompt_bundle`, `prompt_fragment_bundle`).
4. `import_candidates` validates each record against the matching profile schema, sanitizes it, and feeds it into the corresponding evolver as an `External*Candidate`. Rejected candidates are reported in `GepaImportSummary.rejected_candidates`.

The bridge enforces hard caps on file sizes (`MAX_EXPORT_FILE_BYTES = 12 MiB`, `MAX_CANDIDATES_FILE_BYTES = 8 MiB`, `MAX_CANDIDATE_RECORDS = 64`) and a daily budget ledger (`gepa_budget_status_from_ledger`). Stale `running/` jobs are recovered after timeout.

## Limits and tradeoffs

- **Selection is scalar, not Pareto.** Multi-objective scores are visible but combined into a single weighted score before promotion. A candidate that wins router accuracy at the cost of synthesis quality can still win.
- **Benchmarks are static and embedded.** `benchmarks/*.json` are compiled in. They cover the surfaces Evolve mutates but do not catch every regression a real workload could produce — that is what the canary is for.
- **The replay gate trusts structured fields.** It deliberately ignores user phrasing, which is the right call for routing safety, but it also means that nuanced "this answer was technically correct but bad" feedback only surfaces if the channel writes a correction.
- **Skills are not learning artifacts.** Evolve may identify a capability gap, but it records memory, procedure, strategy, prompt, or profile evidence. Skill design/import belongs to the separate skills workflow.
- **Self-modification is opt-in only.** The `SelfEvolveAgent` (`agent.rs`) — the inner loop that rewrites AgentArk's own source code — is gated behind explicit invocation and a security review (`security_review.rs`) that can block promotion and trigger a rollback. The default policy-first path never touches source files.
- **Human supervision still matters.** The Review queue exists for the case where a candidate passed evidence gating but the change is meaningful enough to deserve an explicit approval. Evolve will not silently flip a high-impact mutation.

## Code map

| File | Role |
|------|------|
| `src/core/self_evolve/mod.rs` | Module index and public re-exports for `SelfEvolveAgent`, prompt / specialist / policy keys, gate report types, JSONL pruning helper. |
| `src/core/self_evolve/agent.rs` | Inner self-evolve coding agent (research → plan → implement → build → test → fix), gated behind explicit user request. |
| `src/core/self_evolve/prompt_evolution.rs` | Router + primary-response + synthesis prompt-bundle evolver, weighted scoring, canary lifecycle. |
| `src/core/self_evolve/specialist_prompt_evolution.rs` | Specialist-role prompt-bundle evolver for the six built-in delegated agents. |
| `src/core/self_evolve/prompt_fragment_evolution.rs` | Prompt-fragment bundle evolver with token-regression invariants. |
| `src/core/self_evolve/policy_evolution.rs` | Routing complexity policy evolver; defines `RoutingComplexityPolicy` and benchmark loop. |
| `src/core/self_evolve/routing_canonical_evolution.rs` | Routing canonical overlay evolver — treats canonicals as data, not prompt text. |
| `src/core/self_evolve/strategy_runtime.rs` | Tool strategy profile, canary state primitives, KV keys for profiles / snapshots / canary / safety events. |
| `src/core/self_evolve/promotion_gate.rs` | `PromotionGateReport`, `PromotionGateReason`, typed reason codes. |
| `src/core/self_evolve/replay_gate.rs` | Evidence-driven approval gate for `learning_candidate` rows. |
| `src/core/self_evolve/gepa_bridge.rs` | Export / queue / run / import for the offline DSPy GEPA optimizer; budget ledger; readiness checks. |
| `src/core/self_evolve/security_review.rs` | Static security scan for self-modification diffs (blocking findings rollback the agent). |
| `src/core/self_evolve/coding_guidelines.rs` | Coding rules embedded in the self-evolve agent's system prompt. |
| `bridges/gepa_optimizer/__main__.py` | Python DSPy entry point invoked by `gepa_bridge::run_python_optimizer`. |
| `frontend/src/components/pages/EvolutionPage.tsx` | Evolution UI: simplified view, internals view, stat strip, experiments, review queue. |
| `frontend/src/components/pages/traceEvolutionHelpers.tsx` | Shared UI helpers: `EVOLUTION_PAGE_TABS`, `EvolutionStatStrip`, `EvolutionRolloutBar`, evidence builders. |
