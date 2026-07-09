# ArkDistill

ArkDistill is the deterministic context-saving layer inside AgentArk: the place where noisy tool output is compacted before it reaches the next model turn. This document describes the runtime distiller in `src/core/agent/ark_distill.rs`, the spine integration in `src/core/agent/spine.rs`, the Evolve/GEPA bridge in `src/core/self_evolve/gepa_bridge.rs`, the analytics aggregation in `src/channels/http/arkdistill_analytics.rs`, and the workspace analytics surface in `frontend/src/components/pages/AnalyticsPage.tsx`.

## Overview

ArkDistill runs after a tool returns structured data and before that data is handed into model context. It keeps required evidence, IDs, status fields, and result summaries visible while shrinking browser pages, HTML, traces, command output, and integration dumps. On noisy outputs, it is expected to often save 60-90% of model-visible context. Already-structured JSON or compact integration responses usually save less.

ArkDistill is not a model summarizer. Profiles are JSON data with explicit limits and field rules. That keeps output deterministic across retries, preserves prompt-cache stability, and avoids adding a live model gate to every tool call.

## Runtime pipeline

The hot path is:

1. A tool returns a `ToolResult`.
2. The spine normalizes it into the model-visible JSON shape.
3. ArkDistill loads the active profile from KV key `tool_output_distill_profile_v1`, or falls back to the built-in profile.
4. The distiller walks the normalized JSON and applies deterministic operations: line deduplication, HTML-to-text conversion, whitespace folding, head/tail excerpts, array caps, object-key caps, and configured blob omission.
5. The spine logs an `arkdistill_tool_output` operational event with original size, distilled size, estimated saved tokens, estimated prompt cost saved, tool primitive, action, and profile ID.
6. The distilled JSON is appended to model context. Full raw artifacts remain available through traces and task/session records.

## Profile contract

The live profile is stored under `tool_output_distill_profile_v1`.

| Field | Meaning |
| --- | --- |
| `profile_id` | Stable profile identifier for analytics and rollback records. |
| `version` | Profile schema version. |
| `enabled` | Runtime kill switch for immediate rollback behavior. |
| `required_fields` | Field names or paths that must be retained when present. |
| `generic_limits` | Fallback caps for string length, array items, and object keys. |
| `rules` | Structured selectors for tool primitive, action, field name, and field path. |

Rules can tune maximum string length, head/tail excerpt size, repeated-line deduplication, HTML-to-text conversion, whitespace folding, and blob-field omission. Rule selection uses structured tool/action metadata and field paths, not user wording.

## Evolve integration

Evolve can optimize ArkDistill as a context-saving profile surface. The GEPA bridge exports the current profile and the `arkdistill_profile` candidate contract. Imported candidates are sanitized, replayed against noisy synthetic tool fixtures, and scored on saved context while preserving required structured fields.

Promotion is conservative:

- Candidate JSON must parse into the profile schema.
- Replay output must remain valid JSON.
- Required fields must survive replay fixtures.
- Estimated saved tokens must improve beyond the current profile.
- The current profile is stored under `tool_output_distill_profile_baseline_snapshot_v1` before a promoted profile is applied.
- The latest import/evaluation result is stored under `tool_output_distill_profile_last_result_v1`.

## Analytics

The `/analytics/llm` endpoint includes an `arkdistill` section derived from `operational_logs` rows with event type `arkdistill_tool_output`.

The selected time range aggregates:

- Result count.
- Original, distilled, and saved characters.
- Estimated original, distilled, and saved tokens.
- Estimated prompt cost avoided when pricing metadata is available.
- Aggregate savings percentage.
- Time-series savings by bucket.
- Savings by tool/action surface.

The savings percentage is computed as total saved characters divided by total original characters for the selected range, so the UI can display values like `60% saved` or `70% saved` across all runs in that window.

## Storage

ArkDistill does not require a database migration. It uses the existing KV store and the existing `operational_logs.payload` JSON column, so schema version remains 1.

| Where | What |
| --- | --- |
| KV `tool_output_distill_profile_v1` | Active runtime profile. |
| KV `tool_output_distill_profile_baseline_snapshot_v1` | Rollback snapshot before a promoted profile is applied. |
| KV `tool_output_distill_profile_last_result_v1` | Last GEPA import/evaluation result. |
| `operational_logs` event `arkdistill_tool_output` | Per-tool compaction stats for analytics. |

## Limits and tradeoffs

- ArkDistill is deterministic and cheap, but unknown tools fall back to generic compaction until a profile adds a more specific rule.
- The 60-90% savings range is expected on noisy pages, logs, traces, HTML, and large dumps. Compact JSON responses usually save less.
- Full raw artifacts should stay accessible through traces or task/session records; ArkDistill only changes what is handed to the next model turn.
- Candidate profiles are evaluated on structured replay fixtures and required-field invariants, not hardcoded user phrasing.
