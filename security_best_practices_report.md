# Security Review Report

## Executive Summary
The app creation/deletion flow is **not isolated** and currently executes untrusted app commands on the host via shell. Cleanup behavior is also non-deterministic (best-effort kill + delete), which can leave orphaned processes or residual data. There is also a permission-approval UX gap that makes required approvals hard or impossible to complete from the UI.

## Critical Findings

### SEC-001: No VM/container isolation for created apps
- Severity: Critical
- Impact: A generated/deployed app can execute with host process privileges instead of isolated VM/container boundaries.
- Evidence:
  - `src/actions/app.rs:684`
  - `src/actions/app.rs:730`
  - `src/actions/app.rs:430`
  - `src/channels/http.rs:3344`
- Details: Dynamic app install/start and restart paths use `tokio::process::Command::new("sh")` directly on the host and run in app directories, not in an isolated VM/container runtime.
- Recommendation: Run app workloads in per-app isolated compute (ephemeral VM or sandboxed container) with strict filesystem/network policy and resource limits.

### SEC-002: Untrusted shell command execution (`entry_command` / `install_command`)
- Severity: Critical
- Impact: Prompt/content-driven command strings can directly execute arbitrary host shell commands.
- Evidence:
  - `src/actions/app.rs:536`
  - `src/actions/app.rs:537`
  - `src/actions/app.rs:660`
  - `src/actions/app.rs:684`
  - `src/actions/app.rs:720`
  - `src/actions/app.rs:731`
- Details: `entry_command` and `install_command` are passed into `sh -c` without command allowlisting, structured execution, or policy enforcement.
- Recommendation: Remove `sh -c` execution for untrusted command text. Use constrained launch templates (per runtime type) and explicit argument vectors with policy checks.

## High Findings

### SEC-003: LLM/API secrets are injected into deployed app processes by default
- Severity: High
- Evidence:
  - `src/actions/app.rs:599`
  - `src/actions/app.rs:739`
  - `src/actions/app.rs:740`
  - `src/actions/app.rs:741`
  - `src/channels/http.rs:3354`
  - `src/channels/http.rs:3359`
- Details: Apps are marked `needs_llm` based on non-empty global LLM env set, and those env vars are injected into app processes. Any compromised/malicious app code can exfiltrate these secrets.
- Recommendation: Default to no secret injection. Require explicit per-app scoped secret grants with least-privilege and rotation support.

### SEC-004: Delete flow ignores stop errors before recursive delete
- Severity: High
- Evidence:
  - `src/channels/http.rs:3455`
  - `src/channels/http.rs:3456`
  - `src/actions/app.rs:312`
  - `src/actions/app.rs:332`
- Details: Delete performs `let _ = stop(...)` and then `remove_dir_all(...)`. Stop paths also ignore kill errors. This can produce partially deleted state and leave running descendants.
- Recommendation: Fail closed on stop errors, verify process termination, then delete. Return actionable errors if cleanup is incomplete.

### SEC-005: Process-tree teardown is incomplete
- Severity: High
- Evidence:
  - `src/actions/app.rs:730`
  - `src/actions/app.rs:731`
  - `src/actions/app.rs:312`
  - `src/actions/app.rs:332`
- Details: Commands are launched via shell and only the tracked child is killed. Child-spawned descendants may survive.
- Recommendation: Launch in dedicated process groups/cgroups and kill the full group on stop/delete, then verify with wait + timeout.

## Medium Findings

### SEC-006: Permission-approval path for action security is incomplete in UI/API
- Severity: Medium
- Evidence:
  - `src/safety/mod.rs:192`
  - `src/safety/mod.rs:443`
  - `src/channels/http.rs:1855`
  - `src/channels/http.rs:2051`
- Details: Safety engine records pending approvals in memory, but no retrieval/decision endpoint exists for these safety approval requests. Existing approval endpoints target tasks/audit log, not safety pending requests.
- Recommendation: Add explicit approval queue APIs and UI for safety permission prompts (approve/reject with scope and audit trail).

### SEC-007: Unused apps are only notified, not auto-cleaned
- Severity: Medium
- Evidence:
  - `src/sentinel.rs:2661`
  - `src/sentinel.rs:2665`
  - `src/sentinel.rs:2700`
  - `src/sentinel.rs:2707`
- Details: Idle apps trigger notifications but are not stopped/deleted automatically, increasing long-lived exposure and resource drift.
- Recommendation: Add optional policy-based lifecycle cleanup (stop/archive/delete) with grace period and clear user controls.

## Positive Controls Noted
- App ID validation is present in management endpoints: `src/channels/http.rs:2659`.
- Static file serving canonicalizes paths and enforces app-root boundary: `src/channels/http.rs:3165`, `src/channels/http.rs:3174`.
- App management APIs are under protected routes with auth middleware: `src/channels/http.rs:1832`, `src/channels/http.rs:2087`.

