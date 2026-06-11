//! The mining pass: bounded, quiet-hours-only orchestration that turns the
//! usage window into persisted, value-vetted opportunities.
//!
//! Bounds (resource rule): single-flight CAS, one pass per cooldown window,
//! ≤500 runs loaded, miners run sequentially and synchronously, exactly ONE
//! LLM verdict call per pass (covering every fresh draft), drafts capped per
//! pass. The caller is responsible for idle gating (the tick) or explicit
//! user invocation.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::core::model::llm::LlmClient;
use crate::storage::Storage;

use super::miners::default_miners;
use super::{opportunity_id, topic_similarity, OpportunityDraft};

const MINING_COOLDOWN_KEY: &str = "evolve_opportunity_mining_last_pass_v1";
const MINING_COOLDOWN_SECS: i64 = 6 * 60 * 60;
const MINING_WINDOW_RUNS: u64 = 500;
const MINING_DRAFT_CAP: usize = 8;
const MINING_DEDUPE_HORIZON: u64 = 200;
/// Rejected verdicts get re-judged after this many days (interests evolve;
/// a negative verdict must never pin a segment dead forever).
const REJECTED_REJUDGE_DAYS: i64 = 14;
/// Topics this similar to an existing live opportunity are duplicates.
const DUPLICATE_TOPIC_SIMILARITY: f64 = 0.6;
const VERDICT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
/// Style-profile KV key written by self_tune — this engine is its first
/// production consumer.
const SELF_TUNE_STYLE_PROFILE_KEY: &str = "self_tune:style_profile";

static MINING_PASS_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

struct MiningPassGuard;

impl Drop for MiningPassGuard {
    fn drop(&mut self) {
        MINING_PASS_IN_FLIGHT.store(false, Ordering::Release);
    }
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct MiningPassSummary {
    pub ran: bool,
    pub skipped_reason: Option<String>,
    pub drafts_mined: usize,
    pub fresh_judged: usize,
    pub surfaced: usize,
    pub rejected: usize,
}

/// Run one bounded mining pass if the cooldown allows. `force` bypasses the
/// cooldown for explicit user invocation (never the single-flight guard).
pub async fn maybe_run_opportunity_mining_pass(
    storage: &Storage,
    llm: &LlmClient,
    force: bool,
) -> MiningPassSummary {
    let mut summary = MiningPassSummary::default();
    if MINING_PASS_IN_FLIGHT
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        summary.skipped_reason = Some("mining pass already in flight".to_string());
        return summary;
    }
    let _guard = MiningPassGuard;

    if !force && !mining_cooldown_elapsed(storage).await {
        summary.skipped_reason = Some("mining cooldown active".to_string());
        return summary;
    }

    let runs = match storage
        .list_recent_experience_runs_for_canary_eval(MINING_WINDOW_RUNS)
        .await
    {
        Ok(runs) => runs,
        Err(error) => {
            tracing::warn!(error = %error, "opportunity mining could not load usage window");
            summary.skipped_reason = Some("usage window unavailable".to_string());
            return summary;
        }
    };
    if runs.is_empty() {
        summary.skipped_reason = Some("no honest-labeled usage yet".to_string());
        record_mining_pass(storage).await;
        return summary;
    }

    let style_profile = storage
        .get(SELF_TUNE_STYLE_PROFILE_KEY)
        .await
        .ok()
        .flatten()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());

    let operation_items = storage
        .list_active_experience_items_any_scope(&["lesson", "procedure"], 160)
        .await
        .unwrap_or_default();
    let router_learning_candidates = storage
        .list_learning_candidates_with_options(None, false, 64)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|candidate| {
            candidate.candidate_type == crate::core::self_evolve::ROUTER_LEARNING_CANDIDATE_TYPE
        })
        .collect::<Vec<_>>();
    let semantic_units = {
        let from = window_from_timestamp(&runs);
        match storage
            .list_semantic_work_units_between(&from, &chrono::Utc::now().to_rfc3339(), 240)
            .await
        {
            Ok(units) => units,
            Err(error) => {
                tracing::warn!(error = %error, "opportunity mining could not load semantic work units");
                Vec::new()
            }
        }
    };
    let window = super::window::UsageWindow::from_context(
        runs,
        style_profile,
        operation_items,
        semantic_units,
        router_learning_candidates,
    );

    // Refresh the usage eval store from the same window (bounded; pure
    // derivation, no LLM): served/corrected exchanges become the candidate
    // evaluation distribution.
    let derived_cases = super::eval_sets::cases_from_runs(&window.runs);
    if !derived_cases.is_empty() {
        if let Err(error) =
            super::eval_sets::persist_usage_eval_cases(storage, &derived_cases).await
        {
            tracing::warn!(error = %error, "failed to persist usage eval rows");
        }
        let mut eval_store = super::eval_sets::load_usage_eval_store(storage).await;
        if super::eval_sets::merge_usage_cases(&mut eval_store, derived_cases) {
            super::eval_sets::save_usage_eval_store(storage, &eval_store).await;
        }
    }

    let mut drafts: Vec<OpportunityDraft> = Vec::new();
    for miner in default_miners() {
        drafts.extend(miner.mine(&window));
    }
    summary.drafts_mined = drafts.len();

    let existing = storage
        .list_recent_evolve_opportunities(MINING_DEDUPE_HORIZON)
        .await
        .unwrap_or_default();
    let now = chrono::Utc::now();
    let fresh: Vec<OpportunityDraft> = drafts
        .into_iter()
        .filter(|draft| draft_is_fresh(draft, &existing, now))
        .take(MINING_DRAFT_CAP)
        .collect();
    if fresh.is_empty() {
        record_mining_pass(storage).await;
        return summary;
    }

    // ONE intent-based value-verdict call covers the whole batch.
    let verdicts = judge_drafts(llm, &fresh, window.style_profile.as_ref()).await;
    summary.fresh_judged = fresh.len();
    let Some(verdicts) = verdicts else {
        // Fail-open: nothing persisted, next pass retries; the failure is
        // visible in logs and the pass cooldown still applies.
        record_mining_pass(storage).await;
        return summary;
    };

    for draft in &fresh {
        let id = opportunity_id(draft.miner_key, &draft.segment_key, &draft.target_surface);
        let verdict = verdicts.get(&id);
        let (status, title, description, segment_label, reason) = match verdict {
            Some(verdict) if verdict.useful => (
                "surfaced",
                verdict.title.clone(),
                verdict.description.clone(),
                if verdict.segment_label.trim().is_empty() {
                    draft.segment_label.clone()
                } else {
                    verdict.segment_label.clone()
                },
                verdict.reason.clone(),
            ),
            Some(verdict) => (
                "rejected",
                String::new(),
                String::new(),
                draft.segment_label.clone(),
                verdict.reason.clone(),
            ),
            // Missing from the verdict response: leave unpersisted for retry.
            None => continue,
        };
        let now_iso = chrono::Utc::now().to_rfc3339();
        let model = crate::storage::entities::evolve_opportunity::Model {
            id: id.clone(),
            miner_key: draft.miner_key.to_string(),
            status: status.to_string(),
            title,
            description,
            segment_label,
            segment_key: draft.segment_key.clone(),
            target_surface: draft.target_surface.clone(),
            evidence_json: serde_json::to_value(&draft.evidence)
                .unwrap_or_else(|_| serde_json::json!({})),
            expected_benefit_json: serde_json::to_value(&draft.expected_benefit)
                .unwrap_or_else(|_| serde_json::json!({})),
            risk_json: serde_json::json!({ "summary": draft.risk }),
            holdout_run_ids_json: serde_json::json!(draft.holdout_run_ids),
            verdict_json: serde_json::json!({
                "useful": status == "surfaced",
                "reason": reason,
                "judged_at": now_iso,
                "topic_text": draft.topic_text,
            }),
            ledger_json: serde_json::json!({}),
            gepa_job_id: None,
            decided_at: None,
            created_at: now_iso.clone(),
            updated_at: now_iso,
        };
        match storage.upsert_evolve_opportunity(&model).await {
            Ok(()) => {
                if status == "surfaced" {
                    let mut eval_rows = super::eval_sets::opportunity_case_rows_from_runs(
                        &id,
                        &draft.evidence.example_run_ids,
                        &draft.holdout_run_ids,
                        &window.runs,
                    );
                    if draft.miner_key == "operation_contract_repair" {
                        eval_rows.extend(super::eval_sets::operation_safety_floor_rows(&id));
                    }
                    if !eval_rows.is_empty() {
                        if let Err(error) = storage.upsert_evolve_eval_cases(&eval_rows).await {
                            tracing::warn!(
                                error = %error,
                                opportunity_id = %id,
                                "failed to persist opportunity eval cases"
                            );
                        }
                    }
                    summary.surfaced += 1;
                } else {
                    summary.rejected += 1;
                }
            }
            Err(error) => {
                tracing::warn!(error = %error, opportunity_id = %id, "failed to persist opportunity");
            }
        }
    }

    summary.ran = true;
    record_mining_pass(storage).await;
    summary
}

fn window_from_timestamp(runs: &[crate::storage::entities::experience_run::Model]) -> String {
    runs.iter()
        .map(|run| run.created_at.as_str())
        .filter(|value| !value.trim().is_empty())
        .min()
        .map(str::to_string)
        .unwrap_or_else(|| (chrono::Utc::now() - chrono::Duration::days(30)).to_rfc3339())
}

fn draft_is_fresh(
    draft: &OpportunityDraft,
    existing: &[crate::storage::entities::evolve_opportunity::Model],
    now: chrono::DateTime<chrono::Utc>,
) -> bool {
    let id = opportunity_id(draft.miner_key, &draft.segment_key, &draft.target_surface);
    for row in existing {
        if row.id == id {
            // Re-judge old rejections so evolving interests aren't pinned
            // dead; every other status means the row is alive or decided.
            if row.status == "rejected" {
                let rejudge_due = chrono::DateTime::parse_from_rfc3339(&row.updated_at)
                    .map(|updated| {
                        (now - updated.with_timezone(&chrono::Utc)).num_days()
                            >= REJECTED_REJUDGE_DAYS
                    })
                    .unwrap_or(true);
                return rejudge_due;
            }
            return false;
        }
        // Near-duplicate topic under a different id (cross-miner overlap):
        // keep the existing live row, drop the draft.
        if row.status != "rejected" && row.status != "dismissed" {
            let row_topic = row
                .verdict_json
                .get("topic_text")
                .and_then(|topic| topic.as_str())
                .unwrap_or(&row.segment_label);
            if topic_similarity(&draft.topic_text, row_topic) >= DUPLICATE_TOPIC_SIMILARITY {
                return false;
            }
        }
    }
    true
}

async fn mining_cooldown_elapsed(storage: &Storage) -> bool {
    match storage.get(MINING_COOLDOWN_KEY).await {
        Ok(Some(bytes)) => String::from_utf8(bytes)
            .ok()
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value.trim()).ok())
            .map(|last| {
                (chrono::Utc::now() - last.with_timezone(&chrono::Utc)).num_seconds()
                    >= MINING_COOLDOWN_SECS
            })
            .unwrap_or(true),
        _ => true,
    }
}

async fn record_mining_pass(storage: &Storage) {
    let now = chrono::Utc::now().to_rfc3339();
    if let Err(error) = storage.set(MINING_COOLDOWN_KEY, now.as_bytes()).await {
        tracing::warn!(error = %error, "failed to record mining pass timestamp");
    }
}

struct DraftVerdict {
    useful: bool,
    title: String,
    description: String,
    segment_label: String,
    reason: String,
}

fn verdict_system_prompt() -> &'static str {
    "You judge whether mined optimization opportunities are genuinely valuable to \
THIS user, based on evidence from their own usage. Work from underlying intent \
and meaning, never surface phrasing. An opportunity is useful only when a person \
with this activity would genuinely want it acted on: the evidence is material \
(enough samples, a real excess over their own baseline), the expected benefit \
would be felt (fewer tokens or seconds or corrections in something they actually \
do), and acting on it is low-regret. An update nobody asked for about something \
nobody is deciding is not useful. For each useful item write a short human title \
(plain language, names the user's activity, no internal jargon or ids), a one- \
or-two-sentence description that says the concrete benefit, and a short \
segment_label describing the activity in the user's own terms. Use the style \
profile, when given, only to phrase things the way this user works. Return only \
JSON: {\"verdicts\":[{\"id\":\"...\",\"useful\":true|false,\"title\":\"...\",\
\"description\":\"...\",\"segment_label\":\"...\",\"reason\":\"...\"}]}"
}

async fn judge_drafts(
    llm: &LlmClient,
    drafts: &[OpportunityDraft],
    style_profile: Option<&serde_json::Value>,
) -> Option<std::collections::BTreeMap<String, DraftVerdict>> {
    let items = drafts
        .iter()
        .map(|draft| {
            let id = opportunity_id(draft.miner_key, &draft.segment_key, &draft.target_surface);
            serde_json::json!({
                "id": id,
                "dimension": draft.miner_key,
                "segment_activity": draft.topic_text,
                "evidence": draft.evidence,
                "expected_benefit": draft.expected_benefit,
                "target_surface": draft.target_surface,
            })
        })
        .collect::<Vec<_>>();
    let mut user_message = format!(
        "Mined opportunities from this user's recent usage:\n{}",
        serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".to_string())
    );
    if let Some(profile) = style_profile {
        user_message.push_str(&format!(
            "\n\nUser style profile (phrasing context only):\n{}",
            serde_json::to_string(profile).unwrap_or_default()
        ));
    }
    match tokio::time::timeout(
        VERDICT_TIMEOUT,
        llm.chat_with_system(verdict_system_prompt(), &user_message),
    )
    .await
    {
        Ok(Ok(response)) => parse_verdicts(&response.content),
        Ok(Err(error)) => {
            tracing::warn!(error = %error, "opportunity value verdict failed");
            None
        }
        Err(_) => {
            tracing::warn!("opportunity value verdict timed out");
            None
        }
    }
}

fn parse_verdicts(text: &str) -> Option<std::collections::BTreeMap<String, DraftVerdict>> {
    let trimmed = text.trim();
    let value = serde_json::from_str::<serde_json::Value>(trimmed)
        .ok()
        .or_else(|| {
            trimmed
                .find('{')
                .zip(trimmed.rfind('}'))
                .and_then(|(start, end)| {
                    serde_json::from_str::<serde_json::Value>(&trimmed[start..=end]).ok()
                })
        })?;
    let verdicts = value.get("verdicts")?.as_array()?;
    let mut map = std::collections::BTreeMap::new();
    for verdict in verdicts {
        let Some(id) = verdict.get("id").and_then(|id| id.as_str()) else {
            continue;
        };
        let text_field = |key: &str| {
            verdict
                .get(key)
                .and_then(|field| field.as_str())
                .map(str::trim)
                .unwrap_or_default()
                .to_string()
        };
        map.insert(
            id.trim().to_string(),
            DraftVerdict {
                useful: verdict
                    .get("useful")
                    .and_then(|useful| useful.as_bool())
                    .unwrap_or(false),
                title: text_field("title"),
                description: text_field("description"),
                segment_label: text_field("segment_label"),
                reason: text_field("reason"),
            },
        );
    }
    Some(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_parser_reads_batch_json_with_prose_tolerance() {
        let parsed = parse_verdicts(
            r#"Here you go: {"verdicts":[
                {"id":"evolve-opp-abc","useful":true,"title":"Trim long data answers",
                 "description":"Saves ~1.2k tokens per analysis turn.",
                 "segment_label":"your CSV analysis sessions","reason":"material excess"},
                {"id":"evolve-opp-def","useful":false,"reason":"too few samples"}
            ]}"#,
        )
        .unwrap();
        assert_eq!(parsed.len(), 2);
        assert!(parsed["evolve-opp-abc"].useful);
        assert_eq!(
            parsed["evolve-opp-abc"].segment_label,
            "your CSV analysis sessions"
        );
        assert!(!parsed["evolve-opp-def"].useful);

        assert!(parse_verdicts("no json").is_none());
    }

    #[test]
    fn rejected_rows_block_remining_until_rejudge_window_elapses() {
        let draft = OpportunityDraft {
            miner_key: "token_hotspot",
            segment_key: "segment-x".to_string(),
            segment_label: "segment-x".to_string(),
            target_surface: "prompt_bundle:primary_response".to_string(),
            evidence: Default::default(),
            expected_benefit: Default::default(),
            risk: String::new(),
            holdout_run_ids: Vec::new(),
            topic_text: "segment-x analysis work".to_string(),
        };
        let id = opportunity_id(
            "token_hotspot",
            "segment-x",
            "prompt_bundle:primary_response",
        );
        let now = chrono::Utc::now();
        let mut row = crate::storage::entities::evolve_opportunity::Model {
            id,
            miner_key: "token_hotspot".to_string(),
            status: "rejected".to_string(),
            title: String::new(),
            description: String::new(),
            segment_label: "segment-x".to_string(),
            segment_key: "segment-x".to_string(),
            target_surface: "prompt_bundle:primary_response".to_string(),
            evidence_json: serde_json::json!({}),
            expected_benefit_json: serde_json::json!({}),
            risk_json: serde_json::json!({}),
            holdout_run_ids_json: serde_json::json!([]),
            verdict_json: serde_json::json!({}),
            ledger_json: serde_json::json!({}),
            gepa_job_id: None,
            decided_at: None,
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
        };
        // Freshly rejected: blocked.
        assert!(!draft_is_fresh(&draft, std::slice::from_ref(&row), now));
        // Past the re-judge window: eligible again.
        row.updated_at = (now - chrono::Duration::days(REJECTED_REJUDGE_DAYS + 1)).to_rfc3339();
        assert!(draft_is_fresh(&draft, std::slice::from_ref(&row), now));
    }

    #[test]
    fn near_duplicate_topics_under_live_rows_are_deduped() {
        let draft = OpportunityDraft {
            miner_key: "latency_hotspot",
            segment_key: "other-key".to_string(),
            segment_label: "other".to_string(),
            target_surface: "prompt_bundle:primary_response".to_string(),
            evidence: Default::default(),
            expected_benefit: Default::default(),
            risk: String::new(),
            holdout_run_ids: Vec::new(),
            topic_text: "quarterly sales csv analysis with pandas".to_string(),
        };
        let now = chrono::Utc::now();
        let row = crate::storage::entities::evolve_opportunity::Model {
            id: "evolve-opp-existing".to_string(),
            miner_key: "token_hotspot".to_string(),
            status: "surfaced".to_string(),
            title: "Trim analysis answers".to_string(),
            description: String::new(),
            segment_label: "csv work".to_string(),
            segment_key: "csv-work".to_string(),
            target_surface: "prompt_bundle:primary_response".to_string(),
            evidence_json: serde_json::json!({}),
            expected_benefit_json: serde_json::json!({}),
            risk_json: serde_json::json!({}),
            holdout_run_ids_json: serde_json::json!([]),
            verdict_json: serde_json::json!({
                "topic_text": "pandas csv quarterly sales analysis sessions"
            }),
            ledger_json: serde_json::json!({}),
            gepa_job_id: None,
            decided_at: None,
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
        };
        assert!(!draft_is_fresh(&draft, std::slice::from_ref(&row), now));
    }
}
