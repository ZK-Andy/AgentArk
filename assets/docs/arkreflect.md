# Reflect

Reflect is AgentArk's local retrospective surface. It reads the work the agent and the user already produced — chats, ArkOrbit canvases, memories, watchers, deployed apps, sentinel proposals, pulse events, evolution lineage, usage rows — and turns a selected day, week, or month into a clustered, browsable recap without asking the user to journal anything.

## Overview

Reflect solves the post-hoc visibility problem: you used the system, and now you want to know what you actually did. It does not capture new signal at write time — every other AgentArk subsystem already writes its own row. Instead, Reflect periodically scans those rows inside a bounded time window, derives one normalized `semantic_work_unit` per source row, embeds and clusters those units, and serves the result from cache so the page and the API never block on heavy work. A daily digest can wake the user once a day if the day was actually meaningful, and is otherwise silent.

## Concepts: work unit, cluster, panorama

- **Work unit (`semantic_work_unit`).** A single derived row that represents one piece of recently-touched work, regardless of which subsystem produced it. Reflect reads upstream entities (conversations, orbits, experience items, procedural patterns, apps, tasks/goals, watchers, sentinel observations and proposals, Pulse events, evolution lineage, LLM usage) and projects each one into a uniform `(source_kind, source_id, title, summary, content_preview, occurred_at, message_count, metadata, embedding)` shape with a stable `id` derived from `stable_unit_id(source_kind, source_id)` (`src/channels/http/reflect_control.rs:560`).
- **Cluster.** A spherical-k-means group of work units, computed from L2-normalized embeddings using cosine distance (`src/channels/http/reflect_control.rs:2898`). Each cluster carries a representative unit, a centroid embedding, a source-mix histogram, and an optional `related_history` block linking back to similar units from earlier or later periods.
- **Panorama.** The visual layer in the UI. The Pattern Observatory tab renders clusters as a fixed-orbit graph of nodes (one per cluster, with up to two satellite nodes per cluster pulled from `related_history`), and the rest of the page renders telemetry, evidence, and replay views around the same cached data.

## Data model: semantic_work_unit

Defined in `src/storage/entities/semantic_work_unit.rs`. The table is `semantic_work_units`.

| Field | Type | Meaning |
| :--- | :--- | :--- |
| `id` | `String` (PK, not auto) | Stable hash of `source_kind:source_id`, prefixed `reflect-`. Same upstream row always projects to the same unit id. |
| `source_kind` | `String` | One of `conversation`, `orbit_chat`, `experience_item`, `procedural_pattern`, `app`, `goal`, `watcher`, `sentinel`, `arkpulse`, `arkevolve`, `llm_usage`. |
| `source_id` | `String` | Identifier inside the upstream subsystem (conversation id, orbit transcript id, app slug, etc.). |
| `conversation_id` | `Option<String>` | Set when the unit is anchored to a chat or orbit transcript; used by the chat-side dedup deletes. |
| `project_id` | `Option<String>` | Set when the upstream row carries a project scope. |
| `channel` | `String` | Channel that produced the row (e.g. `web`, `slack`). |
| `title` | `String` | Short label rendered in cards and the panorama. |
| `summary` | `String` | One-paragraph plain summary. |
| `content_preview` | `String` | Truncated excerpt (≤ `REFLECT_PREVIEW_CHARS = 260`). |
| `text_hash` | `String` | SHA-256 of the embedding text; lets the refresh job skip re-embedding identical content. |
| `occurred_at` | `String` (RFC3339) | Time the underlying work happened; the only time column used by range queries. |
| `period_start` | `Option<String>` | Optional period anchor (used by lineage and usage rows). |
| `period_end` | `Option<String>` | Optional period anchor. |
| `message_count` | `i32` | How many messages or events fed this unit. |
| `metadata` | `JsonBinary` | Source-specific structured payload (lineage info, deploy outcome, watcher status, etc.). |
| `created_at` | `String` (RFC3339) | First time this unit was upserted. |
| `updated_at` | `String` (RFC3339) | Last refresh; drives staleness. |
| `embedding` | `Option<PgVector>` | L2-normalizable embedding of `embedding_text`. Optional so cache rows can exist before the embedder catches up. |

Indexes (declared in `src/storage/migrations.rs:887`):

- `idx_semantic_work_units_source` — unique on `(source_kind, source_id)`. Guarantees the projection is one-to-one.
- `idx_semantic_work_units_occurred` — on `occurred_at` for range queries.
- `idx_semantic_work_units_channel` — on `(channel, occurred_at)` for channel-scoped lookups.
- `idx_semantic_work_units_embedding_hnsw` — pgvector HNSW with `vector_cosine_ops` `WHERE embedding IS NOT NULL`. Used only by `nearest_semantic_work_units_outside_window` for cross-period related-history lookups, not for in-period clustering.

Free-form text columns (`title`, `summary`, `content_preview`, `metadata`) are decrypted at read time by `Storage::decrypt_semantic_work_unit` (`src/storage/mod.rs`).

## Source coverage

The bounded scan in `refresh_reflect_units` (`src/channels/http/reflect_control.rs:2468`) reads each input stream and bridges it to a `ReflectCandidateUnit`:

| Source | Bridge | Per-window cap |
| :--- | :--- | :--- |
| Main chat | `conversation_candidates` from `Storage::list_conversations_updated_between` + `get_messages_between` | 120 conversations, 80 messages each |
| ArkOrbit transcripts | `orbit_candidates` from `ArkOrbitService::list_orbits` + transcript reads | 80 orbits, 16 transcripts each |
| Memory captures | `experience_item_candidate` from `list_experience_items_between` | 200 |
| Procedural patterns | `procedural_pattern_candidate` from `list_procedural_patterns_between` | 160 |
| Deployed apps | `app_candidate` from `AppRegistry::list` (filtered by window) | n/a (registry size) |
| Goals / tasks | `goal_candidate` from `list_tasks_updated_between` | 220 |
| Watchers (live) | `watcher_candidate` from `Storage::list_watchers` | 160 |
| Watchers (supervisor states) | `supervisor_watcher_candidate` from `list_automation_supervisor_states` | 160 |
| Sentinel observations | `sentinel_observation_candidate` from `sentinel_panel::load_observations` | 120 |
| Sentinel proposals | `sentinel_proposal_candidate` from `sentinel_panel::load_proposals` | 120 |
| Pulse events | `arkpulse_candidate` from `list_arkpulse_events_between` | 160 |
| Evolve lineage | `lineage_candidate` over `routing_policy`, `prompt_bundle`, `specialist_prompt_bundle`, `prompt_fragment_bundle` lineage files | 160 rows per file |
| LLM usage | `usage_candidates` from `list_llm_usage_between` | 4000 rows, then aggregated |

The total cached unit count read back into the page is capped at `REFLECT_MAX_UNITS = 700` per query.

## Derivation pipeline

Each scan iteration follows the same shape, in `refresh_reflect_units`:

1. **Collect** rows from the upstream entity for the `[from, to)` window (each call wrapped in `REFLECT_DB_TIMEOUT = 12s`).
2. **Project** them to `ReflectCandidateUnit` via the source-specific bridge (e.g. `conversation_candidate` at line 1417, `orbit_candidate` at 1509). The projection synthesizes `embedding_text` (≤ `REFLECT_EMBED_TEXT_CHARS = 16_000`), `content_preview`, and metadata.
3. **Embed** in `embedding_for_candidate` (line 2289). The function hashes the embedding text with `Sha256`, looks up an existing unit with the same id, and reuses its embedding if `text_hash` matches; otherwise it calls the active `EmbeddingClient` with a `REFLECT_EMBED_TIMEOUT = 20s` budget. If embedding fails or no embedder is configured, the unit is upserted with `embedding = None` and clustering will fall back to activity mode.
4. **Upsert** via `upsert_candidate` -> `Storage::upsert_semantic_work_unit`, which `ON CONFLICT (id) DO UPDATE` over every column (`src/storage/mod.rs:6502`).
5. **Cluster** at read time, not refresh time. `build_clusters` (line 2959) keeps only units whose embedding normalizes to the dominant dimension, picks `k = min(REFLECT_MAX_CLUSTERS=8, sqrt(n)).ceil()` farthest-first seeds via `choose_seed_vectors`, runs `REFLECT_KMEANS_ROUNDS = 8` rounds of cosine-distance assignment + centroid recomputation, and picks the unit with minimum total in-cluster distance as the representative.
6. **Enrich** clusters with cross-period history: `enrich_clusters_with_related_history` calls `Storage::nearest_semantic_work_units_outside_window` against the HNSW index, accepts matches with cosine distance ≤ `REFLECT_RELATED_HISTORY_MAX_DISTANCE = 0.32`, and attaches up to 8 hits.
7. **Suggest follow-ups** via `build_suggested_followups`, capped at 5 with diversification across `recovery_advice`, `latest_developments`, `continue_theme`.

## Caching strategy

The page reads cache first. `ark_reflect_endpoint` (line 4180) calls `Storage::list_semantic_work_units_between` directly; if the result is empty and no refresh is running, it spawns a `cache_miss` refresh and returns whatever it has. Clients with `refresh=true` in the query string also trigger a `query` refresh in parallel.

Staleness is judged by `cache_status_for_units` (line 3654):

- `latest_unit_at = max(updated_at)` across the returned set.
- `stale = (now - latest_unit_at) > REFLECT_STALE_AFTER_SECS` (one hour).
- Mode resolves to `empty`, `refreshing`, `stale`, or `ready` and is included in the response.

Refresh is single-flight. `spawn_reflect_refresh` (line 3446) checks the in-process `REFLECT_REFRESH_IN_FLIGHT` atomic and acquires a cluster-wide KV lease (`arkreflect_refresh_lease_v1`, TTL 180s) before running, so concurrent processes do not duplicate work. Successful refreshes also delete units older than `REFLECT_CACHE_RETENTION_DAYS = 400` days at the start of the run.

A background idle loop, `reflect_idle_loop` (line 4135), wakes every `REFLECT_IDLE_INTERVAL = 10 min`, checks `reflect_server_is_idle`, and runs the daily digest pass plus a monthly refresh over the last `REFLECT_IDLE_LOOKBACK_DAYS = 35` days. It is launched once per process by `spawn_reflect_idle_loop`.

## Day / week / month windowing

`ReflectPeriod::from_query` (line 91) accepts `daily|day`, `weekly|week`, `monthly|month`. Defaults to weekly. Default windows are simple deltas from `now` — 1, 7, 31 days — but explicit `from` / `to` query parameters override them. The validator rejects `from >= to`.

There is no pre-aggregated rollup table. The window is a SQL `OccurredAt >= from AND OccurredAt < to` over the cached `semantic_work_units` rows; clustering, source counts, the timeline ribbon, and the daily digest all derive from that same fetch. The only persisted aggregates are `ReflectRefreshStatus` and `ReflectDailyDigestStatus`, both stored in KV (`arkreflect_refresh_lease_v1`, `arkreflect_daily_digest_status_v1`).

For the daily digest, the window is computed in the user's profile timezone via `reflect_daily_window_for_date` (line 490), with midnight handled by `reflect_local_midnight_utc`.

## Daily Digest

The digest is opt-in. The flag lives at `ARKREFLECT_DAILY_DIGEST_ENABLED_KEY = "arkreflect_daily_digest_enabled_v1"` (`src/channels/http/autonomy_support.rs:13`) and is surfaced in `SettingsPageFull`.

`maybe_prepare_daily_digest` (line 3944) is the gate. Order of checks:

1. **Enabled?** If not, write a `disabled` status and return.
2. **Target date.** `reflect_digest_target_date` returns today's local date if `local_hour >= REFLECT_DAILY_DIGEST_NOT_BEFORE_LOCAL_HOUR = 20`, otherwise yesterday. The digest is therefore aimed at the just-finished day and only fires after 20:00 local.
3. **Already sent?** If the previous status for the same `target_date` is `sent`, return.
4. **Lease.** Acquire `arkreflect_daily_digest_lease_v1` (TTL 180s) under the owner string `arkreflect-digest:{pid}:{uuid}`. Fails fast if another process owns it.
5. **Cache freshness.** Fetch units; if the window is empty or the cache is `stale`, mark status `preparing`, spawn a `daily_digest` refresh, and return without notifying.
6. **Meaningful?** `reflect_activity_is_meaningful` (line 656) requires any of: at least one background-source unit (memory, procedures, apps, goals, watchers, sentinel, arkpulse, arkevolve), ≥ 2 meaningful units excluding usage, ≥ 2 clusters, or ≥ 4 conversational messages. If none holds, the status flips to `skipped_quiet` and nothing is sent.
7. **Summarize.** `generate_daily_digest_summary` calls the active LLM with a 35s timeout and falls back to `fallback_daily_digest_summary` (a deterministic source-line list) on failure.
8. **Deliver.** In-app notification via `emit_notification_with_status("Reflect Daily Digest", ...)` plus the user's preferred push channel via `notify_preferred_channel_reported`. `delivery_attempts` are recorded in the status row.

The digest does not fire when `enabled = false`, before 20:00 local, on cache miss / stale, when activity is not meaningful, when another process holds the lease, or when the day has already been sent.

## HTTP API

Both routes are registered in `src/channels/http.rs:2233`. The handlers are in `src/channels/http/reflect_control.rs`.

| Method | Path | Purpose |
| :--- | :--- | :--- |
| `GET` | `/reflect` | `ark_reflect_endpoint`. Returns the full `ReflectResponse`: period, window, source counts, baseline counts, embedding status, refresh status, cache status, daily-digest status, suggested follow-ups, clusters, and unclustered units. Cache-first; spawns a `cache_miss` refresh if the window is empty. |
| `GET` | `/reflect?period=weekly&from=...&to=...&refresh=true` | Same handler. `refresh=true` triggers a non-blocking `query` refresh in parallel with the read. |
| `POST` | `/reflect/refresh` | `ark_reflect_refresh_endpoint`. Manually requests a `manual` refresh for the same period/window parameters. Returns `202 Accepted` if the refresh started or was already running, `409 Conflict` if it could not be queued. |

Query parameters accepted on both: `period` (`daily`/`weekly`/`monthly`), `from` and `to` (RFC3339), `refresh` (`1|true|yes`). `from >= to` returns 400.

The daily-digest enable flag is not on `/reflect`; it is a settings field, `arkreflect_daily_digest_enabled`, read and written through `/api/settings` (`src/channels/http/api_types.rs:443`, `:695`).

## UI: Reflect page (Panorama)

`frontend/src/components/pages/ReflectPage.tsx` polls `/reflect` every 120s when `autoRefresh` is on. The page is a `WorkspacePageShell` with a `WorkspacePageHeader` (eyebrow `Reflect`, title `Your work, clustered into a clear recap`), a period toggle (`Day` / `Week` / `Month`), an anchor date picker, and a `Refresh` button bound to `POST /reflect/refresh`.

The body is a tabbed story view. Tabs are `Reflection Studio`, `Pattern Observatory`, `Achievement Canvas`, `Dream Board`, and `Weekly Replay`; the latter four hide when there is nothing to show.

- **Reflection Studio.** A 3 / 6 / 3 grid (collapses on narrow screens). Left column: an `Activity mix` panel showing the top 5 source kinds with `tacticalAccent` color bars. Center column: `What we did`, `What you achieved`, `What went good`, optional `What went wrong`, `Observation`, `Dream`, and `Evidence` cards. Right column: `Today status`, `Follow-ups`, and `Grouping` (embedding coverage percentage). Source counts come from `response.source_counts`; `Today status` text is rendered by `digestStatusTitle` / `digestStatusDetail` against the persisted daily-digest status.
- **Pattern Observatory.** A `ReactECharts` graph with the recently restyled tactical sensor look — JetBrains Mono labels, a fixed orbit layout (`x = cos(θ) * 240`, `y = sin(θ) * 150`), per-source symbol shapes (`tacticalSymbol`: hexagon for chat/orbit, diamond for watcher/sentinel/arkpulse, triangle for memory/procedures, square for app/goal/arkevolve), a desaturated phosphor palette derived from `tacticalAccent`, an HUD reticle drawn as four short axis ticks plus two concentric circles via `graphic.elements`, and corner brackets `◢ PANORAMA · NN TRACES` / `◣ FOCUS·MAP`. To the right of the graph, the `Observed patterns` table lists the top 5 clusters as 3-column rows (icon, name + related-history hint, two-digit ordinal), with deduped labels via `buildClusterLabelMap` so colliding names get a `: hint` suffix.
- **Timeline ribbon.** Built by `activityOption` (line 1141). It bins all in-window units into `TIMELINE_BUCKETS` ( 24 / 28 / 36 for day/week/month), draws each bucket as a 2px bar, highlights the peak bucket with a brighter border, and labels only the first and last x-axis ticks. Surfaced inside `Weekly Replay` next to up to 6 chronologically earliest units rendered as scene cards.
- **Achievement Canvas / Dream Board.** Compact stat cards driven by `achievementCards` and `dreamCards`, with text generated from cluster counts, recovery follow-ups, the lead cluster, and `relatedHistoryText`.
- **Empty state.** When the cache is empty, a single panel explains that "No reflected work units are cached for this range yet" and a `Collecting` / `Waiting for activity` chip flips while a refresh is in flight.

The page never blocks on clustering: the `embedding_status.mode` field can be `semantic` (real k-means) or `activity` (fallback list of single-unit clusters when embeddings are missing or the worker semaphore is busy), and the UI reads either the same way.

## Privacy

Reflect is local-only. The scan reads encrypted upstream rows through `Storage`, which decrypts in-process, derives one `semantic_work_unit` row, encrypts the text columns again at write time, and stores the embedding alongside in pgvector. The HNSW related-history query and the k-means clustering both run in-process. No reflection data leaves the machine. The optional Daily Digest summary is generated by whichever LLM the user has configured locally and is delivered through the user's notification channels; if no channel is configured, the digest is prepared but not sent and the failure is recorded in `delivery_attempts`.

## Limits and tradeoffs

- **Cluster quality is bounded by the embedding model.** If the configured embedder is unavailable or slow, units are persisted with `embedding = None` and the page falls back to `activity` mode — single-unit clusters ordered by recency. The page will show this as `Grouping … %` below 100%.
- **Bounded source scan.** Per-window caps (120 conversations, 200 memories, 220 tasks, 160 watchers, 160 pulse events, etc.) keep refresh time predictable but mean very high-volume days can lose long-tail rows. The scan does not paginate within a window.
- **HNSW related-history is Postgres-only.** `nearest_semantic_work_units_outside_window` returns empty on non-Postgres backends; SQLite developers will see `unavailable` history blocks.
- **Daily Digest quiet-window heuristic.** `reflect_activity_is_meaningful` is intentionally permissive but can suppress legitimate days that consist of only a single short chat (< 4 messages, no background events). The digest also will not fire before 20:00 local even on a day full of activity, because it is structured as an end-of-day summary.
- **One-cluster-worker semaphore.** `REFLECT_CLUSTER_SEMAPHORE` (size 1) plus `REFLECT_CLUSTER_QUEUE_TIMEOUT = 250ms` and `REFLECT_CLUSTER_TIMEOUT = 4s` mean a slow cluster pass forces concurrent requests onto the activity fallback rather than queueing them.
- **No live event stream.** Unlike chat, ArkOrbit, or Pulse, Reflect has no WebSocket or SSE channel. The page is a periodic poll over a cache and is, by design, latent.

## Code map

| Path | Purpose |
| :--- | :--- |
| `src/channels/http/reflect_control.rs` | HTTP handlers, candidate bridges, refresh job, k-means, related-history enrichment, daily digest pipeline, idle loop. |
| `src/channels/http/autonomy_support.rs` | KV key constants for the daily-digest enable flag. |
| `src/channels/http/settings_control.rs` | Read/write of `arkreflect_daily_digest_enabled` through `/api/settings`. |
| `src/channels/http.rs` (lines 2233–2234) | Route registration for `/reflect` and `/reflect/refresh`. |
| `src/storage/entities/semantic_work_unit.rs` | The `Model` and `ActiveModel` for the cached row. |
| `src/storage/mod.rs` (`upsert_semantic_work_unit`, `list_semantic_work_units_between`, `nearest_semantic_work_units_outside_window`, `delete_semantic_work_units_*`) | Storage layer for Reflect rows, plus the HNSW-backed cross-window similarity query. |
| `src/storage/migrations.rs` (lines 887–940) | Pgvector HNSW index plus `idx_semantic_work_units_source/occurred/channel`. |
| `src/core/arkorbit/orbit_agent.rs` | One of the upstream sources; produces transcripts that `orbit_candidates` projects. |
| `src/sentinel.rs`, `src/channels/http/sentinel_panel.rs` | Source of `sentinel` observations and proposals consumed by `sentinel_observation_candidate` / `sentinel_proposal_candidate`. |
| `frontend/src/components/pages/ReflectPage.tsx` | The Panorama UI: tabs, sensor graph, timeline ribbon, telemetry cards, daily-digest status. |
| `frontend/src/api/client.ts` | `api.rawGet("/reflect")` / `api.rawPost("/reflect/refresh")` glue. |
| `frontend/src/styles.css` | `.arkreflect-pill`, `.arkreflect-panorama`, and the phosphor color tokens used by the sensor view. |

For sibling subsystems referenced from this page see `arkmemory.md`, `arkorbit.md`, `arkpulse.md`, `arksentinel.md`, and `arkevolve.md` in the same folder.
