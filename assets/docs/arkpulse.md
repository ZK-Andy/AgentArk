# Pulse

Pulse is the operational telemetry stream at the centre of AgentArk's self-monitoring loop. Every run scans the live runtime for breakage, drift, and capacity pressure, then writes a structured event so the UI, the briefing system, and Reflect can reason about runtime health over time.

## Overview

Pulse is a periodic, code-first health probe that emits one rich `PulseEvent` per run, not a raw metrics pipeline. Each event captures a system snapshot (tasks, watchers, knowledge counts, deployed apps, security signal), a list of deterministic doctor findings with optional one-click remediations, and a scan ledger of what was checked and how long each phase took. A pulse run never wakes the agent loop directly, but it can wake the user via a critical notification or trigger a one-click remediation.

## Event model

The on-disk row in `src/storage/entities/arkpulse_event.rs` is small. Heavy structure (scan log, health checks, doctor findings, security snapshot, deployed apps) is folded into `details_json`; the four free-form text columns are encrypted at rest via `encrypt_storage_string`.

| Field | Type | Purpose |
|-------|------|---------|
| `id` | `String` | Primary key derived from the event timestamp by `pulse_event_storage_id`. |
| `timestamp` | `String` (RFC3339, UTC) | Run-start time; used for ordering, retention, and time-range queries. |
| `status` | `String` | `ok`, `alert`, or `error`. |
| `message` | `String` (encrypted) | Headline produced by `build_pulse_log_summary` / `build_noncritical_summary`. |
| `summary` | `String` (encrypted) | Stable summary; persisted alongside `message` so UI text can change shape without rewrites. |
| `flags_json` | `String` (encrypted JSON array) | `critical`, `non_critical`, plus category tags from `build_critical_notification`. |
| `overdue_tasks` | `i32` | Pending tasks whose `scheduled_for` is older than 1 hour. |
| `failed_tasks` | `i32` | Tasks in `TaskStatus::Failed` at scan time. |
| `details_json` | `String` (encrypted JSON) | Serialized `PulseDetails`: scan log, health checks, deployed apps, doctor findings, score, security snapshot, knowledge totals, timing. |

`PulseDetails` in `src/sentinel.rs` adds `scan_started_at`/`scan_finished_at`/`scan_duration_ms`, `notification_outcome`, task counts, `active_watchers`, `total_memories`, `overdue_list`, `failed_list`, `uptime_secs`, `health_checks`, optional `SecuritySnapshot`, `deployed_apps`, `doctor_findings`, and `doctor_score: u32` (0..100).

## Emitters

Pulse events are produced exclusively by `crate::sentinel::run_pulse` (`src/sentinel.rs:5183`). What varies is who calls it:

- **Background scheduler** at `src/sentinel.rs:3753`, driven by `SentinelConfig::pulse_interval` (default 1800s). Defers when the runtime is busy and respects `is_agent_autonomy_paused`.
- **App-state changes.** `trigger_arkpulse_after_app_change` (`src/channels/http/arkpulse_control.rs:1203`) fires after app disable, access-guard updates, external publish, restart, and delete, all from `src/channels/http/app_serving.rs`.
- **Manual trigger.** `POST /arkpulse/trigger` -> `trigger_pulse`.
- **CLI / first-boot smoke.** `src/cli.rs:638` and `src/lib.rs:1184`.
- **Doctor sub-modules.** `run_pulse` itself runs `integration_sync::run_due_syncs` and `run_doctor_checks` before computing `doctor_findings` and `doctor_score`; findings are part of the event, not a separate stream.

A `PULSE_RUNNING` atomic plus `PulseRunGuard` provides single-flight; `is_pulse_running` (`src/sentinel.rs:221`) is the probe.

## Consumers

- **Pulse page.** Polls `GET /arkpulse` and renders history, headline, and remediations.
- **Reflect.** `reflect_control.rs:2674` pulls up to `REFLECT_MAX_PULSE_EVENTS` (160) events per window and folds them into the telemetry view via `arkpulse_candidate`.
- **ArkInspect.** `inspect_arkpulse_json` in `src/runtime/mod.rs` exposes `surface: "arkpulse"` with stored count, latest status, latest flags, anomalies derived from `doctor_findings`, and recent events.
- **Briefing / notifications.** `src/core/agent/notifications.rs` reads `arkpulse_last_run_at` for brief freshness gating; critical findings emit `Pulse Critical` and the throttled `Knowledge growth warning`.
- **Gateway-ops overview.** `src/core/gateway_ops.rs` (`arkpulse`, `arkpulse_recent`) consumes recent rows for its dashboard payload.
- **Sentinel runtime health.** `runtime_control.rs:136` reports the Pulse loop heartbeat (`SENTINEL_ARKPULSE_HEARTBEAT_KEY`) under `arkpulse_loop`.

## Aggregation and rollup

There is no time-bucketed rollup table. Aggregation happens at read time:

- `get_pulse_log` (`arkpulse_control.rs:1156`) sorts newest-first and applies `limit`/`offset` in memory.
- Retention runs on the row store via `delete_arkpulse_events_before` and `list_arkpulse_event_ids_beyond_latest`, bounded by `MAX_PULSE_EVENTS = 100` and `MAX_PULSE_EVENT_AGE_DAYS = 30`.
- Reflect computes its own clusters from raw rows; any "rollup" in the UI is a downstream view.
- Notification de-duplication uses signature keys `arkpulse_critical_last_sig_v1` (24h cooldown) and `arkpulse_growth_last_sig_v1` (7d), not numeric aggregation.

## HTTP API

Routes are registered in `src/channels/http.rs` lines 2484-2486 and implemented in `src/channels/http/arkpulse_control.rs`.

| Method | Path | Notes |
|--------|------|-------|
| `GET` | `/arkpulse` | `get_pulse_log`. Returns `events`, `total`, `limit`, `offset`, `running`, `history_unavailable`. Query params `limit` (default 20) and `offset` (default 0). |
| `POST` | `/arkpulse/trigger` | `trigger_pulse`. Starts a run if none is in flight; returns `running`, `paused`, or `triggered`. Refuses when autonomy is disabled. |
| `POST` | `/arkpulse/fix` | `run_arkpulse_fix`. Executes a `DoctorRemediationSpec` (`tunnel_start_verify`, `tunnel_restart_verify`, `app_restart`, `managed_app_operation`, `readonly_investigation`). Requires `event_timestamp` + `finding_index` or an explicit `remediation`. App restarts run in the background (45s timeout); other plans run inline (60s). Every call writes an `arkpulse_fix` row to `operational_log` via `persist_arkpulse_fix_audit`. |

## UI: Pulse page

`frontend/src/components/pages/PulsePage.tsx` uses `react-query` against `/arkpulse` with `REFRESH_MS = 8000` background polling, falling to a 2s poll while a manual run settles.

- **Header.** `WorkspacePageHeader` (eyebrow "Ark Core") with a `Run now` button hitting `POST /arkpulse/trigger`.
- **Headline alert.** Severity flips between `info` (running), `warning` (issues or unavailable history), and `success` (clean), keyed off actionable-finding count and `doctor_score >= 90`.
- **Event list.** Up to 40 most-recent events as `ButtonBase` rows with status dot, derived title, captured timestamp, and meta line (`Score N`, `K findings`, `M overdue`, `F failed`).
- **Run dialog.** Three sections: hero card (health-score / findings / watchers stats and captured/status/scan-duration chips); a `Priority actions` grid of finding cards (severity, remediation-mode chip, next-step block, evidence, `Copy next step`, and a contextual `Restart app` / `Run app fix` / `Run diagnostic` button); and a `Run ledger` rendering `scan_log` as collapsible accordions with metric chips and a notification-outcome alert.
- **No charts and no time-range picker** by design.
- **Inline result feedback.** Each finding card stores a per-fix inline `Alert` so remediation outcomes show without leaving the dialog.

## Relationship to other subsystems

- **Sentinel.** Pulse runs share the sentinel maintenance loop with watcher, integration-sync, and approval-expiry jobs. Pulse owns the runtime breakage/drift signal; sentinel proposals live in their own panel.
- **Evolve.** Pulse does not trigger promotion directly; it supplies `doctor_findings` and `operational_log` audit rows that evolution paths and gateway-ops read as a stability signal.
- **Daily brief.** Reads `arkpulse_last_run_at` for freshness gating and uses `notification_outcome` to avoid double-notifying on breakage already pushed via `Pulse Critical`.
- **Reflect.** Reads pulse rows time-windowed via `list_arkpulse_events_between` as candidate work units.

## Limits and tradeoffs

- **Retention.** 100 rows or 30 days, whichever comes first; no archival path.
- **Single-flight.** Only one pulse can run at a time; concurrent triggers return `already_running` or are skipped.
- **Volume.** Rows are small, but encrypted `details_json` can be tens of kilobytes with many findings and a full scan ledger.
- **Privacy posture.** All four text columns are encrypted at rest. Pulse rows are local-only and only leave the host if the user opts into a remote tunnel or external notification channel.
- **No metrics export.** No Prometheus or OTLP path; read surfaces are the HTTP endpoints and ArkInspect.
- **Auto-fix scope.** `POST /arkpulse/fix` only executes remediations mapped to a known `DoctorRemediationSpec`. Free-form `ShellCommand` remediations are rejected; managed-app fixes require an originating `event_timestamp` + `finding_index`.

## Code map

| Path | Purpose |
|------|---------|
| `src/sentinel.rs` | `run_pulse`, `PulseEvent`/`PulseDetails`/`PulseScanSection` types, retention constants, scheduler loop, notification de-dup keys. |
| `src/storage/entities/arkpulse_event.rs` | Sea-ORM model for the `arkpulse_events` table. |
| `src/storage/mod.rs` | `insert_arkpulse_event`, `list_arkpulse_events`, `list_arkpulse_events_between`, retention helpers. |
| `src/channels/http/arkpulse_control.rs` | HTTP handlers, fix-plan execution, audit persistence, `trigger_arkpulse_after_app_change`. |
| `src/channels/http.rs` | Router registration for `/arkpulse`, `/arkpulse/trigger`, `/arkpulse/fix`. |
| `src/channels/http/app_serving.rs` | Call sites that fire `trigger_arkpulse_after_app_change` after app lifecycle events. |
| `src/channels/http/reflect_control.rs` | Reflect ingestion (`REFLECT_MAX_PULSE_EVENTS = 160`). |
| `src/channels/http/runtime_control.rs` | Surfaces the `arkpulse_loop` heartbeat in runtime health responses. |
| `src/runtime/mod.rs` | `inspect_arkpulse_json` and `summarize_pulse_event` for ArkInspect. |
| `src/core/agent/notifications.rs` | Reads `arkpulse_last_run_at` for brief freshness gating. |
| `frontend/src/components/pages/PulsePage.tsx` | Workspace page: history list, run dialog, finding remediations, scan ledger. |
