# Memory

Memory is the persistent learning store inside AgentArk: the place where the agent records facts, preferences, and reusable knowledge so it can recall them in later sessions without rereading full conversation history. This document describes the data model in `src/storage/entities/`, the capture and dedup pipelines in `src/core/agent/memory.rs` and `src/core/memory_dedup.rs`, the prompt-time type in `src/core/prompt_memory.rs`, the HTTP surface in `src/channels/http/memory_control.rs`, and the workspace page in `frontend/src/components/pages/MemoryPage.tsx`. It complements `arkevolve.md`, `arksentinel.md`, `arkpulse.md`, and `arkorbit.md`.

## Overview

Memory is the durable side of the agent. It captures structured user memories (personal facts, preferences, recurring patterns, knowledge items) from active conversations and from background signals, deduplicates them so the same intent does not produce multiple rows, links every entry back to the message or capture event that produced it, and exposes a review/rollback surface so the user can audit and reverse anything Memory has decided to remember. Rows live in the same encrypted store as the rest of AgentArk; the workspace UI exposes current memory, a pending-review queue, a history ledger, and a memory-health panel for failed captures.

## Data model

Memory uses three lifecycle entities on top of the `experience_item` row that stores the canonical memory text. The lifecycle entities exist so capture, application, and provenance can be observed and rolled back independently of the canonical row.

### `memory_capture_events` (`src/storage/entities/memory_capture_event.rs`)

A durable record of one capture attempt. Status moves through `pending_consolidation`, `processing_deferred`, `completed_deferred`, `failed_deferred`, and review outcomes; the UI's queued/failed/health surfaces read this table.

| Field | Meaning |
| --- | --- |
| `id` | Capture-event primary key. |
| `source_message_id` | Message that triggered the capture, if any. |
| `conversation_id` / `project_id` | Scope. |
| `channel` | Origin (chat, hook, background signal). |
| `status` | Lifecycle state of the attempt. |
| `capture_kind` | What the capture was extracting. |
| `source_hash` | Hash to suppress duplicate captures of the same source. |
| `attempt_metadata` | Per-attempt JSON: prompt, model, decisions, user-review record. |
| `error_history` | Append-only error log. |
| `replay_count`, `next_retry_at`, `completed_at` | Retry scheduling and terminal time. |

### `memory_operations` (`src/storage/entities/memory_operation.rs`)

A structured lifecycle operation emitted by capture and review flows: insert, update, deprecate, merge. It carries the candidate payload plus the metadata needed to apply or revert.

| Field | Meaning |
| --- | --- |
| `id` | Operation id. |
| `capture_event_id` | Capture attempt that emitted this operation. |
| `operation_type` / `status` | What is being applied and where it sits in the queue. |
| `target_memory_id` / `applied_memory_id` | Targeted row vs. the row it actually landed on (may differ when absorbed into a near-duplicate). |
| `key`, `value`, `memory_kind` | Payload (e.g. `personal_fact`, `constraint`). |
| `durability` / `scope` | How long it lives and who it applies to. |
| `confidence` | Source confidence in `[0, 1]`. |
| `looks_sensitive`, `sensitive_reason` | Sentinel-classifier output. |
| `valid_from`, `expires_at`, `review_at` | Validity windows. |
| `evidence_refs` | Inline JSON pointers, mirrored to evidence-link rows. |
| `model_metadata`, `apply_metadata` | Capture-time and apply-time JSON detail. |
| `applied_at`, `reviewed_at`, `review_notes` | Apply and human-review timestamps. |

### `memory_evidence_links` (`src/storage/entities/memory_evidence_link.rs`)

Provenance edges between an operation, a memory, and a source message or capture event. The UI uses these rows to answer "where did this come from?".

| Field | Meaning |
| --- | --- |
| `id` | Edge id. |
| `operation_id` / `memory_id` | One required; both for a fully-applied memory. |
| `evidence_kind` / `evidence_ref` | Discriminator + reference (e.g. `message:<id>`, `trace:<id>`). |
| `source_message_id`, `capture_event_id` | Denormalised join keys. |
| `project_id`, `conversation_id` | Scope of the evidence. |
| `metadata` | Free-form JSON (preview text, char counts, redaction flags). |

## Capture pipeline

Memories enter through the agent loop's user-memory capture, anchored at `src/core/agent/memory.rs:3320` (`upsert_learned_user_memory`). After each user turn the loop builds a capture prompt (`build_user_memory_capture_prompt`, line 1029), parses the model response (`recover_user_memory_capture_payload_shape` is the tolerant fallback at line 1210), classifies sensitivity (`user_memory_capture_item_looks_sensitive`, line 756), and writes a `memory_capture_event` with status `pending_consolidation`. A background drainer processes deferred captures in batches of `USER_MEMORY_CAPTURE_DEFERRED_BATCH_LIMIT = 16`, gated by a single-permit semaphore (`USER_MEMORY_CAPTURE_DRAIN_SEMAPHORE`).

Each accepted item produces a `memory_operation`, which is applied to a canonical `experience_item` and accompanied by `memory_evidence_link` rows back to the source. There is no separate "remember this" channel — explicit user requests flow through the same capture prompt. Failed attempts are replayed up to a per-event cap and surfaced in the UI's Memory Health accordion.

## Evidence and provenance

Every memory has at least one row in `memory_evidence_links`. The link records `evidence_kind` (e.g. `message`, `trace`, `capture_event`), the `evidence_ref` pointer, and the originating `source_message_id` and `capture_event_id`. `GET /arkmemory/sources/{memory_id}` (`arkmemory_sources`, `memory_control.rs:2218`) returns the link rows joined with sentinel-redacted source previews (`arkmemory_safe_source_message_preview`, line 941). Rolled-back operations keep their evidence rows so the historical trail survives.

## Deduplication

`src/core/memory_dedup.rs` is the write-time dedup gate. It is intent-based, not keyword-based: similarity comes from embeddings, borderline cases are resolved by an LLM equivalence judge.

`attempt_absorb_into_canonical` (line 518):

1. Strip the legacy `key: ` slug prefix (`embeddable_text_from_content`) and embed via `MemoryEmbedder`.
2. Run `nearest_active_experience_items_semantic_txn` over rows scoped to the same `kind`, `scope`, `project_id`, and `conversation_id`, returning up to three neighbours.
3. For each neighbour, compute cosine similarity. Below `NO_MERGE_COSINE_SIM = 0.82` it is treated as definitely different. Otherwise call the `SemanticEquivalenceJudge` (`LlmEquivalenceJudge` in production), an 8-second LLM call capped at 256 output tokens that returns `Equivalent`, `NotEquivalent`, or `Uncertain`. Timeouts and uncertainty collapse to "do not merge".
4. On `Equivalent`, `absorb_candidate_into_row` replaces the canonical `content` (newer wins, so refinements like "Kolkata" → "Madhyam, Kolkata" land on the same row), appends the prior content to `metadata.merged_phrasings[]` (ring-buffered to `MAX_MERGED_PHRASINGS = 12`), increments `support_count`, and raises confidence to at most `0.99`.
5. Otherwise the function returns `AbsorbOutcome::Insert { embedding }` and the caller writes a fresh row with the precomputed vector.

Opaque high-entropy tokens and AgentArk redaction markers are stripped from `merged_phrasings` (`sanitize_merged_phrasing_value_for_storage`) so secrets cannot enter the audit trail.

## Prompt-time selection

`src/core/prompt_memory.rs` defines a deliberately minimal struct:

```rust
pub struct PromptMemory {
    pub memory_type: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub relevance_score: f32,
    pub importance: f32,
    pub final_score: f32,
}
```

Selection is owned by the consumers. The task router scores stored memories against the current task (`src/core/task_router.rs:1761`, `select_memories_for_task`) and packs them into delegated specialist payloads; specialists and the swarm coordinator (`src/core/swarm/specialist.rs:102`, `src/core/swarm/coordinator.rs:175`) accept `&[PromptMemory]`; the model runtime (`src/core/agent/model_runtime.rs:267`) folds them into the system prompt. Ranking is the scalar `final_score` blended from `relevance_score` and `importance`. Embeddings are used at write time (dedup) and at queue routing — not as a live retrieval index at prompt assembly.

## Memory operations

A `memory_capture_event` opens, one or more `memory_operations` are emitted (`insert`, `update`, `merge`, `deprecate`), each operation either creates a new `experience_item` or absorbs into an existing one, and an evidence-link row is written per application. From there a row can be:

- Updated by a later capture the dedup judge calls equivalent (newer content wins, prior phrasing preserved in `merged_phrasings`).
- Consolidated into another row by an explicit `queue_memory_merged` ledger event.
- Deprecated by a status change to `deprecated`, surfaced as "Archived" in the UI.
- Restored by a `ledger_event_rolled_back` action from the History tab.

Validity is tracked via `valid_from`, `expires_at`, and `review_at` on the operation row. There is no automatic age-out cron in the read code paths; expiry is honoured at prompt-assembly read time and stale rows surface through the cleanup endpoint for review.

## Privacy and encryption

`src/storage/encrypted.rs` documents the contract: content fields (fact text, message content, KV values) are encrypted with AES-256-GCM via `EncryptedStorage` wrapping the base `Storage` and a shared `KeyManager`; non-content fields (timestamps, IDs, filterable metadata) stay in plaintext so SeaORM queries still work. Memory content rides this same wrapper, so it inherits AES-256-GCM at rest. Capture-time redaction happens before write: `crate::security::redact_secret_input` strips secrets from candidate text and from `merged_phrasings`, and the dedup module additionally drops opaque-token-shaped substrings via Shannon-entropy heuristics in `merged_memory_is_opaque_token_shape`.

## HTTP API

Endpoints registered in `src/channels/http.rs:2309-2356`, served from `memory_control.rs`:

| Method | Path | Handler | Purpose |
| --- | --- | --- | --- |
| GET | `/memory/stats` | `memory_stats` | Counts of facts, documents, preferences, user-data, knowledge. |
| GET | `/memory/facts` | `list_facts` | Paginated learned facts. |
| GET, POST | `/memory/preferences` | `list_user_preferences`, `upsert_user_preference` | Read/write a preference. |
| DELETE | `/memory/preferences/{key}` | `delete_user_preference` | Remove a preference. |
| GET, POST | `/memory/user-data` | `list_user_data_items`, `create_user_data_item` | Manage user-data items. |
| GET, POST | `/memory/knowledge` | `list_knowledge_items`, `create_knowledge_item` | Manage knowledge entries. |
| GET | `/arkmemory/summary` | `arkmemory_summary` | Aggregate counts plus capture-pipeline state. |
| GET | `/arkmemory/queue` | `arkmemory_queue` | Pending-review candidates. |
| POST | `/arkmemory/queue/{id}/approve` `\|` `/reject` | `arkmemory_approve_queue_item`, `arkmemory_reject_queue_item` | Apply or discard a candidate. |
| GET, POST | `/arkmemory/ledger` `\|` `/ledger/{id}/rollback` | `arkmemory_ledger`, `arkmemory_rollback_ledger_event` | History and restore. |
| GET, POST | `/arkmemory/health` `\|` `/health/{id}/apply` | `arkmemory_health`, `arkmemory_apply_health` | Capture-failure findings and review outcomes. |
| GET | `/arkmemory/sources/{memory_id}` | `arkmemory_sources` | Evidence-link rows joined with source previews. |
| GET, POST | `/arkmemory/cleanup` `\|` `/cleanup/apply` | `arkmemory_cleanup`, `arkmemory_apply_cleanup` | Stale-row review and bulk action. |

Legacy `/arkrecall/*` aliases of the same handlers are retained for old browser tabs.

## UI: Memory page

`MemoryPage.tsx` polls summary, queue, ledger, and health endpoints every `REFRESH_MS = 8000`. The header shows total stored items, pending review count, queued-for-consolidation count, and history count. A capture-timing tooltip explains the consolidation delay. An info alert appears when capture events are queued; a warning alert appears for failed captures and auto-opens the Memory Health accordion, which lists findings with severity chips, redacted source previews, and per-finding `Mark reviewed` / `Correct skip` / `False positive` buttons.

Three tabs:

- **Current Memory** delegates to `MemoryPage` for browse, search, edit, and delete of facts, preferences, user data, and knowledge.
- **Pending Review** is a table of candidates with confidence percentage, replay-gate status, and Apply/Reject buttons; Apply is disabled until the replay gate clears the item.
- **History** is an accordion list of events with type chips (`Added`, `Updated`, `Archived`, `Consolidated`, `Rollback`, `Rejected`), a `Restorable` / `Restored` indicator, and a contextual restore button that targets either the event itself or the merged-source memory behind a consolidation.

`approveQueueMutation`, `rejectQueueMutation`, `rollbackMutation`, and `applyHealthMutation` post through `api.client` and invalidate React Query keys for summary, queue, ledger, and health on success.

## Limits and tradeoffs

What Memory does not do today:

- No live embedding-based recall in the prompt path. Prompt assembly uses scalar `final_score` ranking on already-selected memories; embeddings are write-time dedup and queue support, not retrieval-time semantic search.
- No cross-user or cross-instance sharing. The store is local to the install.
- No automatic time-based eviction. `expires_at` and `review_at` are honoured on read and surfaced via the cleanup endpoint, but no background TTL job exists in the paths read here.
- The dedup judge is conservative on uncertainty: any timeout, parse failure, or `Uncertain` verdict keeps memories distinct. False-positive new rows are preferred over false-positive merges.
- Capture is a per-turn LLM call; it adds latency and is therefore deferred into the background drainer rather than blocking the user reply.
- Sentinel redaction runs on capture text and audit text, but free-form prose can still carry user-identifying detail by design.

## Code map

| Path | Purpose |
| --- | --- |
| `src/core/agent/memory.rs` | Agent-side capture: prompt build, payload parse, sensitivity classification, `upsert_learned_user_memory`, deferred drain. |
| `src/core/memory_dedup.rs` | Write-time dedup: embedding lookup, `LlmEquivalenceJudge`, `attempt_absorb_into_canonical`, `merged_phrasings` audit. |
| `src/core/prompt_memory.rs` | Flat `PromptMemory` struct used by the prompt builder, task router, and specialists. |
| `src/storage/entities/memory_capture_event.rs` | Durable capture-attempt entity. |
| `src/storage/entities/memory_operation.rs` | Lifecycle operation entity. |
| `src/storage/entities/memory_evidence_link.rs` | Provenance edge. |
| `src/storage/mod.rs` | `upsert_memory_capture_event`, `upsert_memory_operation`, `upsert_memory_evidence_link`, `nearest_active_experience_items_semantic_txn`. |
| `src/storage/encrypted.rs` | `EncryptedStorage` providing AES-256-GCM at rest for content fields. |
| `src/storage/migrations.rs` | DDL and indexes for the three lifecycle tables. |
| `src/channels/http/memory_control.rs` | HTTP handlers for `/memory/*` and `/arkmemory/*`. |
| `src/channels/http.rs` | Route registration. |
| `src/core/task_router.rs` | Per-task memory selection and delegation packing. |
| `src/core/agent/model_runtime.rs` | Folds `&[PromptMemory]` into the assembled system prompt. |
| `frontend/src/components/pages/MemoryPage.tsx` | Workspace page: summary header, health accordion, Current/Queue/History tabs. |
| `frontend/src/components/pages/MemoryPage.tsx` | Embedded "Current Memory" browse/edit surface. |
