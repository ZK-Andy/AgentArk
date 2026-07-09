# Sentinel

Sentinel is the supervisory plane wrapping every agent action AgentArk takes. It runs as a set of background loops launched at process boot and a synchronous guard pipeline that intercepts inbound messages, tool arguments, capability declarations, and outbound responses. It is the subsystem that decides whether a given step is allowed and whether it needs human approval first.

## Overview

Sentinel is the security and supervision layer between the user, the agent loop, and the outside world. It owns three responsibilities: per-action policy enforcement built on intent classification and a deterministic capability vocabulary; a queue of human-reviewable proposals surfaced through the autonomy briefing; and a fleet of background loops coordinated by `pub fn start` in `src/sentinel.rs`. The work is split between `src/security/` for synchronous guards and `src/sentinel.rs` plus `src/channels/http/sentinel_panel.rs` for the proposal store and scan loop.

## Threat model & invariants

Sentinel defends a concrete set of threats:

- **Prompt injection from untrusted content.** External text (search, fetched pages, MCP output, integrations) is wrapped via `trust_boundary::sanitize_untrusted_output` and tagged `[UNTRUSTED_*_OUTPUT]`. Inbound messages are graded by `intent_classifier` against `MESSAGE_INTENT_VOCABULARY` — `override-instructions`, `extract-system-prompt`, `extract-credentials`, `role-hijack`, `data-exfiltration-request`, plus `benign` / `ambiguous`.
- **Credential and PII exfiltration.** Structural regexes for AWS / OpenAI / GitHub / PEM keys (`RE_AWS_ACCESS_KEY`, `RE_OPENAI_KEY`, `RE_GITHUB_TOKEN`, `RE_PRIVATE_KEY`, `RE_GENERIC_SECRET_ASSIGN` in `src/sentinel.rs`); PII through `src/security/pii.rs` and `outbound::check_outbound_text`.
- **Capability boundary violations.** `CAPABILITY_VOCABULARY` in `src/security/capabilities.rs` makes sensitive operations first-class; `evaluate_capability_correlation` requires approval or blocks on combinations.
- **SSRF and internal-network egress.** `src/security/tool_args_guard.rs` rejects outward tool arguments resolving to loopback, RFC1918, link-local, or cloud-metadata unless whitelisted.
- **Runaway autonomy.** `MAX_SENTINEL_OBSERVATIONS = 120`, `MAX_SENTINEL_PROPOSALS = 96`, `SENTINEL_RETENTION_DAYS = 30`, `SENTINEL_SCAN_COOLDOWN_SECS = 1800` are hard ceilings; `is_agent_autonomy_paused` and `sentinel.enabled` short-circuit any tick.
- **Repeated probing.** `src/security/abuse_tracker.rs` aggregates blocked attempts per `SourceKey` and, on threshold, writes to `approval_log` and moves the source to `pending_approval` until an operator unblocks it.

## Pipeline

Every agent action runs pre-check, execution, post-check; background loops in `src/sentinel.rs` sweep up the trail.

### Pre-check

1. **Inbound classification.** `intent_classifier::classify_inbound` calls the configured model with `MESSAGE_INTENT_VOCABULARY` and produces an `InboundClassification`. The deterministic policy engine — not the model — decides what the tag means.
2. **Capability lookup.** Action manifests, skill bindings, and MCP tool schemas emit `CapabilityObservation`s into a `RunCapabilityContext`. `evaluate_declared_capabilities`, `evaluate_cross_layer_capabilities`, and `evaluate_capability_correlation` assemble a `CapabilityLayerReport` and return `CapabilityCorrelationDecision { Allow | RequireApproval | Block }`.
3. **Tool-argument guard.** `tool_args_guard.rs` rejects URLs, hostnames, and shell commands targeting internal infrastructure unless the operator's `host_whitelist` allows them.
4. **Action guard.** `src/security/action_guard.rs` performs bundle hashing, Ed25519 signature verification, static threat detection, and prompt-injection scanning before a skill or extension can load.

### Execution

The agent loop proceeds. Untrusted return values pass through `trust_boundary::sanitize_untrusted_output` before re-entering the model context.

### Post-check

1. **Output guard.** `outbound::check_outbound_text` / `sanitize_outbound_json` scan for SPII, addresses, and structurally-shaped secrets, returning `Allow`, `RedactedAllow`, or `Block`.
2. **Audit.** Every action lands in the execution-run trail; guards write to `approval_log`.
3. **Drift detection.** `build_in_app_candidates`, `in_app_attention_for_run`, and `run_degradation_summary` in `src/channels/http/sentinel_panel.rs` crawl recent runs, classify failures, and turn persistent degradation into `SentinelObservation` and `SentinelProposal` rows.

## Intent classification & capability boundary

Routing is driven by intent, not keywords. The classifier emits one of eleven stable tags from `MESSAGE_INTENT_VOCABULARY`. Paraphrasing, non-English input, Unicode obfuscation (handled in `src/security/normalize.rs`), and encoded payloads are covered because the policy engine reasons over intent labels, not surface phrasing.

Capabilities use the same shape. A `CapabilityObservation` carries `{ layer, entity_id, kind, target?, evidence?, confidence? }` and is normalized by `normalize_capability_kind` / `normalize_capability_target` / `normalize_capability_selector`. `default_capability_policy()` returns the static `CapabilityRule` set; `evaluate_capability_correlation` combines observations across layers (Skills, Packs, Plugins, MCP, custom channels) and returns `RequireApproval` when, for example, `reads-secrets` co-occurs with `sends-external`. The `MatchedCapabilityRule { id, effect, message, severity }` payload is rendered into the proposal so the operator can see which combination tripped the gate.

## Output guards

Outbound text and JSON pass through `src/security/outbound.rs`. `ADDRESS_RE` and `SPII_RE` match structurally-shaped addresses and SPII keywords (SSN, passport, DOB, driver's licence); `redact_pii` from `src/security/pii.rs` redacts emails, phones, and identifiers; the wire-format secret regexes from `src/sentinel.rs` flag API keys. `OutboundPrivacyPolicy { auto_redact_enabled, public_learning_fenced }` switches between `RedactedAllow` (rewrite and ship) and `Block` (refuse), with `format_outbound_privacy_block` producing the user-visible refusal. The guard runs on every model output that crosses an external boundary, including channel replies, integration writes, and any payload destined for the public-learning sink.

## Approval gates

Approval is required when a capability correlation returns `RequireApproval`, when autonomy mode is `assist` (suggest first) rather than `auto`, when the abuse tracker trips threshold and writes a `security.abuse_review` request via `APPROVAL_ACTION_NAME`, or when a skill / pack / plugin import flags `Suspicious` or `Malicious` in `action_guard::ThreatLevel`.

The proposal queue lives under `SENTINEL_PROPOSALS_KEY` (`sentinel_proposals_v1`). Each `SentinelProposal` carries `id`, `fingerprint`, `proposal_kind`, `status` (`open`, `running`, `queued_for_approval`, `snoozed`, `completed`, `failed`, `dismissed`), `title`, `detail`, `rationale`, `source_kind`, `source_id`, `confidence`, `priority`, `created_at`, `updated_at`, `snoozed_until`, `approved_at`, `dismissed_at`, plus optional `trace_id` and `chat_suggestion_id`. Observations live in parallel under `SENTINEL_OBSERVATIONS_KEY` and feed the builder. `SENTINEL_PROPOSAL_RECREATE_HOURS = 24` prevents a dismissed proposal from instantly re-spawning.

## Audit trail

Guarded actions are reviewable through several stores: execution runs (`storage::list_recent_execution_runs`, the canonical trace; Sentinel reads up to forty per scan); the `approval_log` table (blocks and review requests written by `abuse_tracker` and capability gates); the Sentinel observations and proposals stores (`sentinel_observations_v1` and `sentinel_proposals_v1`, retained thirty days); `arkpulse_log` for doctor findings (capped at `MAX_PULSE_EVENTS = 100`); and per-loop heartbeats (`SENTINEL_SCHEDULER_HEARTBEAT_KEY`, `SENTINEL_WATCHER_HEARTBEAT_KEY`, `SENTINEL_INTEGRATION_SYNC_HEARTBEAT_KEY`, `SENTINEL_APPROVAL_EXPIRY_HEARTBEAT_KEY`, `SENTINEL_ARKPULSE_HEARTBEAT_KEY`, `SENTINEL_AUTO_ANALYSIS_HEARTBEAT_KEY`) that the runtime-control endpoint reads to surface stale loops.

## Drift detection & failure classification

`src/channels/http/sentinel_panel.rs` is the drift surface. `in_app_attention_for_run` walks each recent `ExecutionRun`, calls `run_degradation_summary` to extract the agent-reported reason, and uses `run_is_transient_router_failure` to discard noise. Persistent attention-worthy runs become `in_app_run_attention` observations with a clarification prompt; `should_create_in_app_execution_proposal` decides escalation. `IN_APP_EXECUTION_SCAN_LIMIT = 48` and `IN_APP_STALE_RUN_MINUTES = 15` bound the scan. `collapse_semantically_equivalent_chat_suggestions` clusters duplicates so the operator sees a deduped queue.

## Sentinel UI panel

`frontend/src/components/SentinelPanel.tsx` is built on `WorkspacePageShell` with eyebrow "Ark Core" and title "Sentinel". The header carries three live chips (autonomy mode, waiting count, last-scan label) plus a `Show Sentinel internals` toggle that exposes raw `fingerprint` and `metadata` JSON.

A four-cell stat strip shows `Waiting for you`, `Last check`, `Connected apps`, `In-app signals`. The `Needs your attention` list groups proposals by similarity (an `N similar` chip appears when `group.proposals.length > 1`) and paginates at `SENTINEL_SECTION_PAGE_SIZE = 12`. Selecting a proposal opens a dialog with a clarification-choice section, a Details section, and a Technical Details grid (`Proposal ID`, `Kind`, `Source`, `Status`, `Run status`, `Priority`, `Confidence`, `Created`, `Updated`, `Trace ID`, `Action`, optional `Later until`). Dialog actions are `Close`, `Snooze`, `Dismiss`, and either `Run` or `Launch`.

The `Launch` path is significant: when `proposal_kind === "chat_suggestion_accept"`, the panel writes a `CHAT_PENDING_LAUNCH_STORAGE_KEY` snapshot to `sessionStorage` and calls `navigateToView("chat")` so the chat surface picks up the suggestion on mount. Clarification choices use the same `navigateToView("chat")` handoff. The feed auto-refreshes at `REFRESH_MS = 8000` while `autoRefresh` is on; run traces stream into `SuggestionRunDialog` via `api.rawGet("/trace/{id}")`.

## HTTP API

Routes are wired in `src/channels/http.rs` (lines 1861–1885) and handled by `src/channels/http/sentinel_panel.rs`. All paths are prefixed by the autonomy control plane base.

| Method | Path | Handler | Purpose |
|---|---|---|---|
| GET | `/autonomy/sentinel/settings` | `get_sentinel_settings` | Return the persisted `sentinel` block of `AutonomySettings`. |
| POST | `/autonomy/sentinel/settings` | `update_sentinel_settings` | Update enabled flag, `watch_in_app`, `infer_new_automations`, mode. |
| GET | `/autonomy/sentinel/feed` | `get_sentinel_feed` | Return `{ generated_at, scan, background_learning, observations, proposals, stats }`. |
| POST | `/autonomy/sentinel/proposals/{id}/approve` | `approve_sentinel_proposal` | Move proposal to `running`, dispatch action, emit trace. |
| POST | `/autonomy/sentinel/proposals/{id}/dismiss` | `dismiss_sentinel_proposal` | Mark dismissed; honored against `SENTINEL_PROPOSAL_RECREATE_HOURS`. |
| POST | `/autonomy/sentinel/proposals/{id}/snooze` | `snooze_sentinel_proposal` | Push `snoozed_until` six hours forward. |

`run_sentinel_scan_tick` is the internal driver, invoked by the scheduler loop and by `/autonomy/scan-now` in `src/channels/http/autonomy_control.rs`. It loads settings, short-circuits on disable or cooldown, builds candidates, reconciles, optionally auto-executes when mode is `auto`, prunes, persists, and emits a daily-opportunity nudge via `agent.notify_preferred_channel`.

## Configuration & policy authoring

Operators tune Sentinel via `SentinelConfig` in `src/sentinel.rs`, which exposes intervals (scheduler 30s, watcher 15min, integration sync 2min, approval expiry 5min, Pulse 30min, auto-analysis 30min, container reaper 5min). Per-tenant settings (`autonomy_mode`, `agent_paused`, `sentinel.enabled`, `watch_in_app`, `infer_new_automations`) live in `AutonomySettings` and are mutated through `/autonomy/sentinel/settings`.

To add a capability rule, append to `default_capability_policy()` in `src/security/capabilities.rs` with `{ id, effect, all, any, message, severity }`; selectors are `kind` or `kind:target` strings from `CAPABILITY_VOCABULARY`. Adding a new capability kind requires extending `CAPABILITY_VOCABULARY` and updating `capability_severity` and `capability_category`. Adding an inbound intent tag means extending `MESSAGE_INTENT_VOCABULARY` and teaching `normalize_intent_kind` and the policy engine what it means; the classifier system prompt is regenerated from the vocabulary. Tool-argument host whitelisting is per-project as `ToolArgsGuardConfig.host_whitelist`; outbound privacy is per-project as `OutboundPrivacyPolicy.auto_redact_enabled` and `public_learning_fenced`.

## Limits and tradeoffs

Sentinel cannot stop an operator who explicitly pastes secrets into chat — inbound classification flags it, but a human-issued instruction overrides the gate by design. It cannot detect novel exfiltration channels that don't trip a structural pattern; the output guard catches wire-format keys, not arbitrary obfuscated payloads. It does not prove model alignment, only reduces the odds of the planner being talked into a bad action by untrusted content. It does not replace operator review for high-impact changes (auto-execute is gated to mode `auto` and skips anything that hits a capability correlation rule). And it cannot guarantee freshness across all surfaces — heartbeats are minute-grained, so a stalled loop is detected by the runtime-control endpoint, not by the loop itself. The deliberate tradeoff is determinism over coverage: every guard fails closed, every block is logged, every proposal is traceable to a fingerprint hash or execution run.

## Code map

| File | Purpose |
|---|---|
| `src/sentinel.rs` | Background-loop runtime, `SentinelConfig`, scheduler/watcher/integration/approval/pulse/auto-analysis loops, secret regexes, doctor checks, heartbeat keys. |
| `src/channels/http/sentinel_panel.rs` | Proposal/observation store, `run_sentinel_scan_tick`, candidate builder, in-app run attention, panel HTTP handlers. |
| `src/security/mod.rs` | Module index, `SecurityError`, secret-redaction entry points. |
| `src/security/intent_classifier.rs` | `MESSAGE_INTENT_VOCABULARY`, model-driven inbound classification, deterministic policy verdict. |
| `src/security/capabilities.rs` | `CAPABILITY_VOCABULARY`, capability rules, `evaluate_capability_correlation`. |
| `src/security/action_guard.rs` | Bundle hashing, Ed25519 signing, static analysis, `ThreatLevel`. |
| `src/security/tool_args_guard.rs` | SSRF / internal-network guard for outward-facing tool arguments. |
| `src/security/outbound.rs` | Outbound text/JSON privacy guard, `OutboundPrivacyDecision`. |
| `src/security/trust_boundary.rs` | Untrusted-content sanitization, `[UNTRUSTED_*_OUTPUT]` envelope. |
| `src/security/abuse_tracker.rs` | Per-source trip aggregation, `pending_approval` transition, `security.abuse_review` log. |
| `src/security/normalize.rs` | Unicode canonicalization shared by every detector. |
| `src/security/skill_review.rs` | Model-driven semantic review for skill/extension imports. |
| `frontend/src/components/SentinelPanel.tsx` | Operator UI: header, stat strip, attention list, diagnostics dialog. |
| `src/channels/http.rs` | Route registration for `/autonomy/sentinel/*`. |

Related: `arkpulse.md` (doctor scans), `arkevolve.md` (mutation candidates flowing through this approval queue), `arkmemory.md` (memory Sentinel reads during classification), `arkorbit.md` (chat surface that receives `navigateToView("chat")` handoffs).
