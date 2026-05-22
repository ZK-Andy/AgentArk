# Browser, Trace Receipt, and Memory Edit MVP Design

Date: 2026-05-22
Status: Approved for implementation planning

## Purpose

This MVP makes three AgentArk surfaces feel concrete and usable without expanding scope into migration tooling or a full integration marketplace rewrite:

- Browser automation becomes a visible first-class workflow.
- Trace details become understandable as normal-user run receipts.
- Memory items can be corrected by editing their value/content only.

## Scope

### Browser automation flagship

AgentArk already has browser session, handoff, and profile concepts in the codebase. The MVP should expose those capabilities more directly instead of rebuilding browser automation.

User-facing changes:

- Add or restore an obvious Browser page entry that leads to current browser sessions and profiles.
- Show active sessions, profile state, login handoff actions, screenshots, console errors, and recent session metadata in one place where existing APIs support it.
- Add canned browser task starters in chat for:
  - research
  - form fill
  - scrape/extract
  - login-needed workflow
  - deployed app verification

Non-goals:

- No new browser engine.
- No full browser replay timeline if existing trace/session data cannot support it yet.
- No credential management changes beyond the current login handoff/profile flow.

### Normal-user trace receipts

Trace detail should answer:

- What did the agent do?
- Why did it do it?
- What evidence, artifacts, or tool results did it use?
- What failed or needs attention?

User-facing changes:

- Add a "Run receipt" block to trace detail.
- Summarize outcome, duration, step count, important tools, evidence, artifacts, and first meaningful failure.
- Prefer plain labels and short summaries over raw JSON.
- Keep raw trace/event data available for power users where it already exists.

Non-goals:

- No new tracing storage model.
- No attempt to reconstruct perfect causality from incomplete historical traces.
- No LLM-generated receipt text for MVP unless the existing trace already contains suitable summaries.

### Memory value editing

Users should be able to correct a learned memory without editing internal fields.

User-facing changes:

- Add an edit action for memory items.
- Let the user edit only the memory value/content.
- Preserve metadata such as id, kind, source, status, confidence, evidence, and timestamps unless the backend already updates them as part of an edit event.
- Show success/failure state after save.

Backend changes:

- Add or reuse an endpoint that updates the memory item content by id.
- Validate non-empty edited content.
- Record the change as a memory update event so history/audit views remain accurate.

Non-goals:

- No full schema editor.
- No manual embedding controls.
- No editing hidden provenance or confidence fields from the UI.

## Architecture

The MVP should reuse existing boundaries:

- Frontend pages/components remain responsible for presentation and local UI state.
- `frontend/src/api/client.ts` should expose any new HTTP client function needed by the UI.
- HTTP control modules remain responsible for request validation and API response shape.
- Storage remains responsible for persisted memory changes and audit/history events.
- Existing browser bridge/session/profile APIs should be surfaced before adding new endpoints.
- Existing trace artifacts, summaries, and evidence extraction should feed the receipt UI.

## Data Flow

### Browser

1. User opens Browser page or selects a browser starter in chat.
2. Browser page loads existing sessions/profiles through current API client calls.
3. For login-needed work, user receives the existing handoff action/link.
4. Screenshots and console errors are displayed from current browser bridge/session diagnostics where available.

### Trace receipt

1. User opens a trace detail view.
2. UI computes a receipt from the selected trace and existing step/artifact/evidence helpers.
3. Receipt renders above lower-level event/artifact details.
4. Failures are shown from failed steps, error fields, or unsuccessful outcome status when available.

### Memory edit

1. User opens memory item edit action.
2. UI presents a textarea containing only the current memory content/value.
3. User saves a non-empty value.
4. API validates and persists the new content.
5. Storage records a memory update event.
6. UI refreshes the memory list/history and closes the editor on success.

## Error Handling

- Browser page should handle missing bridge/session data with empty states, not broken panels.
- Browser screenshot or console-error fetch failures should degrade to unavailable state for that section.
- Trace receipt should still render partial receipts when evidence or failure details are missing.
- Memory edit should reject empty values before the request and handle backend validation errors after the request.
- Failed memory saves should leave the editor open with the user's attempted text intact.

## Testing

Implementation should include focused coverage around the changed surfaces:

- Browser UI smoke test or component-level check for visible task starters and browser page access.
- Trace receipt helper tests for successful traces, failed traces, traces with evidence/artifacts, and sparse traces.
- Memory API test for content update success, empty content rejection, missing memory id, and update event recording where the existing test harness supports it.
- Frontend build/typecheck after UI changes.
- Backend compile or targeted tests after API/storage changes.

## Acceptance Criteria

- Browser automation is reachable from the main UI without knowing hidden routes.
- Chat exposes canned browser tasks for the five approved workflows.
- Trace detail includes a concise run receipt that can be understood without reading raw JSON.
- Memory UI lets a user edit the memory value/content only.
- Memory edits persist and appear in history/audit as updates.
- Existing browser, trace, and memory flows continue to work.

## Implementation Order

1. Expose browser page/profile/session surfaces using existing components and APIs.
2. Add browser task starters to chat.
3. Add trace receipt helper and UI block.
4. Add memory content update API/storage path if one does not already exist.
5. Add memory edit UI.
6. Add focused tests and run verification.

## Risks

- Existing browser APIs may expose diagnostics inconsistently across live and completed sessions. The UI should render only data that is available.
- Trace records may vary by task type. Receipt logic must tolerate missing summaries, evidence, artifacts, and failure fields.
- Memory storage may have multiple item types. The edit endpoint should target the currently displayed item model first and avoid broad schema assumptions.

## Spec Self-Review

- No placeholders remain.
- Scope is limited to three approved MVP surfaces.
- Non-goals explicitly exclude OpenClaw/Hermes migration and full browser replay.
- Data flow and ownership boundaries follow existing frontend, API, and storage structure.
- Acceptance criteria are testable without requiring unrelated product work.
