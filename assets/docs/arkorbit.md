# ArkOrbit

ArkOrbit is the canvas runtime AgentArk uses to assemble small, live web apps as
chat output. Where the main chat returns prose, ArkOrbit returns mounted code:
widgets, files, and a per-canvas chat that builds and edits them in place.

## Overview

ArkOrbit is a sandboxed, filesystem-backed widget workspace. Each user has a
persistent set of canvases ("orbits"), each canvas is a directory of HTML,
JavaScript modules, and JSON state, and an in-canvas chat agent creates,
reads, and edits those files in response to natural language. The orbit's
`index.html` boots a small host runtime in a sandboxed iframe and mounts
widgets declared in `data/widgets.json`. Layout is dragged-to-place in the
browser and persisted via the HTTP control surface.

## Concepts: orbit, frame, widget

- **Orbit** — a single canvas. A folder under
  `<DATA_DIR>/arkorbit/L2/orbits/<orbit-id>/` containing `orbit.json`
  (manifest), `index.html` (bootstrap), `mod/<name>/index.js` (widget
  modules), `data/widgets.json` (visible widget registry), `messages.jsonl`
  (current transcript), plus free files under `assets/` and `data/`. The
  `Orbit` DTO lives in `src/core/arkorbit/models.rs`.
- **Frame** — the React surface that hosts an orbit. `OrbitFrame` loads the
  registry, lays out widgets on a 12000x8000 canvas, and subscribes to a
  server-sent file-change stream so edits appear without a full reload.
- **Widget** — one entry in `data/widgets.json` pointing at a module under
  `mod/<name>/index.js` whose default export is `render(el, ctx)`. Optional
  fields: `title`, `left`, `top`, `width`, `height`.

The default orbit (`is_default: true`) is the *workspace overview*; every
other orbit is a *canvas*. The Orbit agent treats the overview as read-only
and only mutates files inside a created canvas.

## The orbit agent

`src/core/arkorbit/orbit_agent.rs` runs a deliberately thin agent path. The
header comment is explicit: "It never invokes the main agent turn loop, intent
planner, semantic router, or tool-call envelope path." `stream_orbit_chat_turn`:

1. Resolves the surface kind from `Orbit::is_default` (`WorkspaceOverview` vs.
   `Canvas`).
2. Runs the inbound security guard
   (`security::intent_classifier::classify_inbound_with_metadata`) with an
   arkorbit-specific surface context. A `Block` verdict short-circuits to a
   refusal reply.
3. Runs an Orbit-scope classifier (`classify_orbit_chat_scope`) that returns
   `orbit_ui_work` or `out_of_scope`; out-of-scope turns get a fixed decline.
4. For up to `READ_ROUND_LIMIT` (3) rounds, calls
   `LlmClient::chat_with_history_stream` with one tool —
   `arkorbit_apply_operations` — and applies each operation. The loop ends
   when the model produces no further reads.
5. Persists the assistant message as JSONL into `messages.jsonl` and emits
   SSE events via `OrbitAgentEvent` (`Status`, `Token`, `FileWritten`,
   `ReadRequested`, `Usage`, `Done`, `Error`).

The agent has no tool catalog, plugin/skills routing, or agent-loop
envelope. `arkorbit_apply_operations` is synthesized inline
(`orbit_operations_action`); on the workspace-overview surface its enum is
restricted to `["read"]`. Reads are truncated to `MAX_READ_BYTES` (32 KiB)
and re-injected as the next turn's user message. History is budgeted via
`HistoryTokenBudget` and compacted when it exceeds threshold.

## Widget tools

The agent surface only ever calls two tools. The fast path is
`arkorbit_apply_operations`. The other entries in `src/actions/arkorbit/` are
matched by alias for backward compatibility — the renderer translates them
into the structured form via `legacy_file_write_payload`.

| Tool | Purpose | Arguments |
|------|---------|-----------|
| `arkorbit_apply_operations` | Single action used inside `stream_orbit_chat_turn`. Carries an ordered list of file operations and an optional user-visible `message`. Canvas surface allows `read`/`write`/`edit`; the workspace overview restricts the enum to `read`. | `message?: string`, `operations: [{ operation: "read"\|"write"\|"edit", orbit_id?: string, path: string, content?: string, find?: string, replace?: string }]` |
| `arkorbit_create_orbit` | Creates a new canvas owned by the active user. Handler in `orbit_tools.rs`; writes `orbit.json` and seeds `index.html`. | `name: string`, `icon?: string`, `color?: string`, `agent_instructions?: string` |
| `orbit_file_write` | Fallback primitive in `file_tools.rs`. Validates the path with `validate_writable_orbit_path` and writes raw content. The chat path no longer prefers it. | `orbit_id: string`, `path: string`, `content?: string` |

Operation kinds normalize generously: `write` aliases include `create` and
`replace`; `edit` aliases include `patch` and `update`. Surgical edits
(`apply_surgical_edit`) replace the first exact `find` with `replace`, with
one outer-newline trim as a tolerance pass.

## Frontend: OrbitFrame

`frontend/src/components/arkorbit/OrbitFrame.tsx` is the React canvas. Given
an `orbitId` it:

- Fetches `data/widgets.json` via `arkorbitApi.moduleUrl` and parses entries
  (top-level array or `{ widgets: [...] }`).
- Renders each entry as an `OrbitWidgetSlot`. The slot dynamically imports
  `mod/<name>/index.js`, wraps the source with `buildOrbitFetchShim` that
  proxies cross-origin GET/HEAD through `/api/arkorbit/orbits/{id}/fetch`,
  and imports it via a blob URL. Widget contract: `render(el, ctx)`, with
  `ctx` exposing `resolveText`, `importMod`, `fetchPublic`, `fetchText`,
  `fetchJson`.
- Lays out widgets via `resolveWidgetLayouts`: saved `left`/`top` from
  `data/widgets.json` win over `localStorage`
  (`arkorbit:<id>:widget-positions:v2`); unplaced widgets are auto-placed by
  `findClosestEmptyLayout` (32-px grid scan, 2x-viewport fallback).
- Drags with raw pointer events. `handlePointerDown` ignores interactive
  selectors unless they have `data-orbit-drag-handle`. `finishDrag` calls
  `OrbitFrame.moveWidget`, which optimistically updates state and PUTs
  `/api/arkorbit/orbits/{id}/widgets/{widget_id}`.
- Subscribes to the file-change SSE stream. Changes to `index.html`,
  `data/widgets.json`, `mod/`, or `assets/` trigger a debounced (120 ms)
  reload.
- Mounts a collapsible `OrbitFilesPanel` listing every orbit file with a code
  viewer.

The slot's mount key strips `left`/`top`/`width`/`height` so dragging does
not unmount the widget.

## Frontend: OrbitChat

`frontend/src/components/arkorbit/OrbitChat.tsx` is the per-orbit chat. It
opens as a draggable panel over the canvas, speaks SSE to
`/api/arkorbit/orbits/{id}/chat`, and renders streaming output incrementally:

- A `running`/`completed`/`failed`/`stopped` state model drives the message
  badge and activity strip.
- Tokens accumulate inline as the model streams. The visible strip beneath
  an active assistant turn is a single `<span>` of prose plus a blink-caret
  pulse and a three-dot ellipsis (`orbit-chat-activity-pulse`,
  `orbit-chat-activity-dots`) — this replaces an earlier green status pill.
- `file_written` SSE events become inline file-op chips ("Wrote <path>" /
  "Edited <path>"). Legacy ``[wrote `path`]`` and "I wrote x." lines from
  older transcripts are normalized to the same chips, with the raw line
  replaced by friendlier sentences ("I added this to the canvas." / "I
  updated the canvas.").
- Status events update the activity label only; they are not pushed into the
  visible message body.
- A history flyout reads `/api/arkorbit/orbits/{id}/chat/transcripts` and
  lets the user load any archived transcript read-only. "New chat" hits
  `/api/arkorbit/orbits/{id}/chat/reset`, which archives `messages.jsonl` to
  `data/chat-history/<ts>-<uuid>.jsonl` and rotates the chat session id.
- The composer pulls a one-shot prefill from
  `sessionStorage["arkorbit.composerPrefill"]`, set by the home dashboard
  when the user names a canvas with a description before submitting.

## Persistence

ArkOrbit is filesystem-only — there are no DB tables for orbits.
`src/core/arkorbit/store.rs` defines a `LayeredStore` with three layers: L2
user data at `<DATA_DIR>/arkorbit/L2/orbits/<orbit-id>/` (wins); L0 disk at
`src/core/arkorbit/l0/`; L0 embedded — the same `runtime/`, `widgets/`,
`skills/` files compiled in via `include_str!`.

Each orbit holds `mod/`, `data/`, `assets/`, `.tmp/`, `index.html`,
`orbit.json`, `messages.jsonl`. `validate_writable_orbit_path` restricts
agent writes to `index.html`, `orbit.json`, or anything under
`mod/`/`data/`/`assets/`. Writes are atomic (`atomic_write_under_orbit`),
JavaScript writes are pre-validated with `node --check` when Node is
installed, and every resolved path is canonicalized under the orbit root.
Deleting an orbit removes the directory and any semantic work units
prefixed with `orbit_chat:<id>`.

## HTTP API

Mounted in `src/channels/http.rs` (lines 2249–2306) and implemented in
`src/channels/http/arkorbit_control.rs`. Index and module responses get a
strict CSP and `sandbox allow-scripts allow-forms allow-modals`.

| Method | Path | Purpose |
|--------|------|---------|
| GET / POST | `/api/arkorbit/orbits` | List orbits; create a new orbit. |
| GET / PUT / DELETE | `/api/arkorbit/orbits/{id}` | Read, patch (`name`, `icon`, `color`, `agent_instructions`), or delete. |
| GET | `/api/arkorbit/orbits/{id}/index` | Sandboxed bootstrap `index.html`. |
| GET | `/api/arkorbit/orbits/{id}/messages` | Last 200 chat messages. |
| GET | `/api/arkorbit/orbits/{id}/files` | Recursive file list (path, bytes). |
| GET | `/api/arkorbit/orbits/{id}/files/{*path}` | Read one orbit file as text. |
| PUT / DELETE | `/api/arkorbit/orbits/{id}/widgets/{widget_id}` | Update widget layout, or delete widget and its module if unused elsewhere. |
| GET | `/api/arkorbit/orbits/{id}/fetch?url=...` | Public fetch proxy (GET/HEAD, 2 MiB, 12 s). |
| GET | `/api/arkorbit/orbits/{id}/chat/transcripts` | List current + archived transcripts. |
| GET | `/api/arkorbit/orbits/{id}/chat/transcripts/{transcript_id}` | Read one transcript. |
| POST | `/api/arkorbit/orbits/{id}/chat/reset` | Archive current transcript, rotate session id. |
| GET | `/api/arkorbit/orbits/{id}/events` | SSE `file_changed` stream from a `notify` watcher. |
| POST | `/api/arkorbit/orbits/{id}/chat` | SSE chat turn (`token`, `file_written`, `read`, `status`, `usage`, `error`, `done`). |
| GET | `/api/arkorbit/mod/{orbit_id}/{*path}` | Layered module resolver. Public only for `runtime/host.js` (`is_public_arkorbit_runtime_asset` in `auth.rs`); other modules require session. |

## UI: the ArkOrbit page

`frontend/src/components/pages/ArkOrbitPage.tsx` is the route wrapper. It
calls `arkorbitApi.listOrbits`, picks the first orbit as active, and renders
one of two surfaces:

- If the active orbit `is_default` is true, it renders **OrbitHomeDashboard**
  — a tile-based launcher. Each tile is a 16:9 card with a per-orbit accent
  color, a generated initial badge, a row of dots showing widget count, and
  a footer with widget count, file count, and last-touched relative time.
  The grid uses `repeat(auto-fill, minmax(...))` and pages 10 tiles at a
  time. The trailing cell is a dashed "+ New Canvas" tile that opens an
  inline name + prompt form. On submit the prompt is stashed in
  `sessionStorage["arkorbit.composerPrefill"]`, the canvas is created via
  `arkorbitApi.createOrbit`, and `OrbitChat` opens prefilled.
- If the active orbit is a created canvas, it renders `OrbitFrame` plus a
  draggable, clamped `OrbitChat` floating panel anchored at `chatAnchor`.

Recent renames: the launcher header reads "ArkOrbit · Your canvases", the
count line reads "N canvases" (singular: "canvas"), and the new-orbit
affordance is the literal string "+ New Canvas". `OrbitSwitcher` and
`OrbitSettingsDialog` are mounted but only visible from the canvas header.

## Limits and tradeoffs

- **Single-orbit edits.** From a canvas the agent can only modify that
  canvas; cross-canvas writes are rejected by
  `resolve_operation_target_orbit`. From the workspace overview the agent
  cannot write at all.
- **No backend tools.** The Orbit agent has exactly one structured tool. It
  cannot run a terminal command, hit MCP, schedule a task, or call any other
  AgentArk action. The scope classifier routes such requests back to main
  chat.
- **Read budget.** Each turn allows at most three read rounds and 32 KiB per
  file; long inspection chains terminate with "the Orbit turn kept
  requesting more file reads".
- **Layout is trivial.** Widgets are absolutely positioned on a 12000x8000
  canvas. No grid snapping, no resize handle, no z-order, no multi-select.
- **Public fetch is GET/HEAD only**, capped at 2 MiB and 12 s.
- **Sandbox constraints.** The orbit `index.html` runs under
  `sandbox allow-scripts allow-forms allow-modals`, and a strict
  `permissions-policy` blocks camera, microphone, geolocation, payment, USB,
  serial, bluetooth, and motion sensors.
- **Privacy.** Inbound chat is redacted on the way to classifiers, but
  secrets are still persisted into `messages.jsonl` if the model echoes
  them. The surface-context prompt warns the model not to write credentials
  into orbit files; there is no enforcement beyond that.

## Code map

| File | Purpose |
|------|---------|
| `src/core/arkorbit/models.rs` | `Orbit`, `OrbitManifest`, `OrbitUpdate`, `OrbitFileEntry`, `OrbitChatMessage`, transcript summary types. |
| `src/core/arkorbit/store.rs` | `LayeredStore`, path validators, atomic writes, embedded L0 fallback, default `index.html`. |
| `src/core/arkorbit/service.rs` | `ArkOrbitService` — orbit CRUD, file IO, chat JSONL, transcript archival, session rotation. |
| `src/core/arkorbit/orbit_agent.rs` | Streaming chat turn, security guard, scope classifier, structured operations action, surgical edit, history compaction. |
| `src/core/arkorbit/l0/runtime/host.js` | In-iframe runtime that mounts widgets via `window.__arkorbit.mount`. |
| `src/core/arkorbit/l0/widgets/` | Built-in widgets (`markdown`, `iframe-html`, `chart`, `table`, `todo`, `fetch-proxy`). |
| `src/actions/arkorbit/orbit_tools.rs` | `arkorbit_create_orbit` handler. |
| `src/actions/arkorbit/file_tools.rs` | `orbit_file_write` fallback handler. |
| `src/channels/http/arkorbit_control.rs` | All HTTP endpoints, SSE chat stream, file-change watcher, public fetch proxy, widget registry mutations, trace persistence. |
| `frontend/src/components/pages/ArkOrbitPage.tsx` | Route wrapper, launcher tile dashboard, chat anchor drag, orbit switching. |
| `frontend/src/components/arkorbit/OrbitFrame.tsx` | Canvas, widget mount/drag/remove, files panel, fetch shim, file-change SSE. |
| `frontend/src/components/arkorbit/OrbitChat.tsx` | Streaming chat panel, file-op chips, transcript history, composer prefill. |
| `frontend/src/components/arkorbit/OrbitSwitcher.tsx` | Active-orbit dropdown in the canvas header. |
| `frontend/src/components/arkorbit/OrbitSettingsDialog.tsx` | Per-orbit name/icon/color/instructions editor. |
| `frontend/src/components/arkorbit/api.ts` | Typed wrappers around `/api/arkorbit/*` plus URL helpers. |
| `frontend/src/components/arkorbit/types.ts` | DTO mirrors of the backend models. |
| `frontend/src/components/arkorbit/useChatSplitter.ts` | Hook for the chat panel resize/split affordance. |

See sibling docs `arkmemory.md`, `arkreflect.md`, and `arkevolve.md`.
