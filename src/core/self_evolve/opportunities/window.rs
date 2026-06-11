//! The usage window: a bounded, read-only view over the install's recent
//! honest-labeled experience runs, grouped into content-derived segments.
//! Segment keys come from the runs' own intent keys (content-derived at
//! write time) — no fixed taxonomy.

use crate::storage::entities::{
    experience_item::Model as ExperienceItem, experience_run::Model as ExperienceRun,
    learning_candidate::Model as LearningCandidate, semantic_work_unit::Model as SemanticWorkUnit,
};

/// Per-segment aggregates over the window. All statistics are relative to
/// this install's own distribution — miners compare against window medians,
/// never absolute domain thresholds.
#[derive(Debug, Clone)]
pub(crate) struct SegmentStats {
    pub key: String,
    pub label: String,
    pub sample_count: usize,
    pub corrected_count: usize,
    pub avg_tokens: Option<f64>,
    pub p95_wall_ms: Option<i64>,
    pub avg_cost_microusd: Option<f64>,
    /// Newest-first run ids, capped.
    pub example_run_ids: Vec<String>,
    /// Newest-first redacted request texts, capped — topic material only.
    pub example_requests: Vec<String>,
}

impl SegmentStats {
    pub fn corrected_rate(&self) -> f64 {
        if self.sample_count == 0 {
            0.0
        } else {
            self.corrected_count as f64 / self.sample_count as f64
        }
    }
}

pub(crate) struct UsageWindow {
    pub runs: Vec<ExperienceRun>,
    pub operation_items: Vec<ExperienceItem>,
    pub router_learning_candidates: Vec<LearningCandidate>,
    /// The self_tune style profile (real per-user pattern data) as verdict
    /// context — its first production consumer.
    pub style_profile: Option<serde_json::Value>,
    segments: Vec<SegmentStats>,
    /// Median tokens/turn across all runs with token evidence.
    pub median_tokens: Option<f64>,
    /// Median wall ms across all runs with timing evidence.
    pub median_wall_ms: Option<i64>,
    /// Overall corrected rate across resolved runs.
    pub overall_corrected_rate: f64,
}

const SEGMENT_EXAMPLE_CAP: usize = 8;

impl UsageWindow {
    #[cfg(test)]
    pub fn from_runs(runs: Vec<ExperienceRun>, style_profile: Option<serde_json::Value>) -> Self {
        Self::from_context(runs, style_profile, Vec::new(), Vec::new(), Vec::new())
    }

    pub fn from_context(
        runs: Vec<ExperienceRun>,
        style_profile: Option<serde_json::Value>,
        operation_items: Vec<ExperienceItem>,
        semantic_units: Vec<SemanticWorkUnit>,
        router_learning_candidates: Vec<LearningCandidate>,
    ) -> Self {
        let mut token_values = Vec::new();
        let mut wall_values = Vec::new();
        let mut resolved = 0usize;
        let mut corrected = 0usize;
        let semantic_by_conversation = semantic_units
            .iter()
            .filter_map(|unit| {
                unit.conversation_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|conversation_id| (conversation_id.to_string(), unit))
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut by_segment =
            std::collections::BTreeMap::<String, (String, Vec<&ExperienceRun>)>::new();
        for run in &runs {
            let (key, label) = segment_descriptor_for_run(run, &semantic_by_conversation);
            by_segment
                .entry(key)
                .or_insert_with(|| (label, Vec::new()))
                .1
                .push(run);
            if let (Some(tokens_in), Some(tokens_out)) = (run.tokens_in, run.tokens_out) {
                token_values.push((tokens_in + tokens_out) as f64);
            }
            if let Some(wall_ms) = run.wall_ms {
                wall_values.push(wall_ms);
            }
            if run.success_state != "provisional" {
                resolved += 1;
                if run.correction_state == "corrected" {
                    corrected += 1;
                }
            }
        }
        let median_tokens = median_f64(&mut token_values);
        let median_wall_ms = median_i64(&mut wall_values);
        let overall_corrected_rate = if resolved == 0 {
            0.0
        } else {
            corrected as f64 / resolved as f64
        };

        let segments = by_segment
            .into_iter()
            .map(|(key, (label, segment_runs))| {
                let mut tokens = Vec::new();
                let mut walls = Vec::new();
                let mut costs = Vec::new();
                let mut corrected_count = 0usize;
                let mut example_run_ids = Vec::new();
                let mut example_requests = Vec::new();
                for run in &segment_runs {
                    if let (Some(tokens_in), Some(tokens_out)) = (run.tokens_in, run.tokens_out) {
                        tokens.push((tokens_in + tokens_out) as f64);
                    }
                    if let Some(wall_ms) = run.wall_ms {
                        walls.push(wall_ms);
                    }
                    if let Some(cost) = run.est_cost_microusd {
                        costs.push(cost as f64);
                    }
                    if run.correction_state == "corrected" {
                        corrected_count += 1;
                    }
                    if example_run_ids.len() < SEGMENT_EXAMPLE_CAP {
                        example_run_ids.push(run.id.clone());
                        if let Some(request) = run
                            .request_text
                            .as_deref()
                            .map(str::trim)
                            .filter(|request| !request.is_empty())
                        {
                            example_requests.push(request.to_string());
                        }
                    }
                }
                let sample_count = segment_runs.len();
                SegmentStats {
                    key,
                    label,
                    sample_count,
                    corrected_count,
                    avg_tokens: mean(&tokens),
                    p95_wall_ms: percentile_i64(&mut walls, 0.95),
                    avg_cost_microusd: mean(&costs),
                    example_run_ids,
                    example_requests,
                }
            })
            .collect();

        Self {
            runs,
            operation_items,
            router_learning_candidates,
            style_profile,
            segments,
            median_tokens,
            median_wall_ms,
            overall_corrected_rate,
        }
    }

    pub fn segments(&self) -> &[SegmentStats] {
        &self.segments
    }
}

fn segment_descriptor_for_run(
    run: &ExperienceRun,
    semantic_by_conversation: &std::collections::BTreeMap<String, &SemanticWorkUnit>,
) -> (String, String) {
    if let Some(event) =
        super::contract_events::contract_events_from_metadata(&run.metadata).first()
    {
        let identity = super::contract_events::contract_event_identity(event);
        return (
            format!("contract:{}", super::stable_content_hash(&identity)),
            event.operation_descriptor.clone(),
        );
    }
    if let Some(unit) = run
        .conversation_id
        .as_deref()
        .and_then(|conversation_id| semantic_by_conversation.get(conversation_id.trim()))
    {
        let key_material = if !unit.text_hash.trim().is_empty() {
            unit.text_hash.trim()
        } else {
            unit.id.trim()
        };
        let label = [unit.title.trim(), unit.summary.trim()]
            .into_iter()
            .find(|value| !value.is_empty())
            .unwrap_or("semantic work segment")
            .to_string();
        return (
            format!("semantic:{}", super::stable_content_hash(key_material)),
            label,
        );
    }
    let intent = run.intent_key.trim();
    if intent.is_empty() {
        ("intent:chat".to_string(), "chat".to_string())
    } else {
        (intent.to_string(), intent.to_string())
    }
}

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

fn median_f64(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|left, right| left.total_cmp(right));
    Some(values[values.len() / 2])
}

fn median_i64(values: &mut [i64]) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    Some(values[values.len() / 2])
}

fn percentile_i64(values: &mut [i64], percentile: f64) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let rank = ((values.len() as f64 - 1.0) * percentile.clamp(0.0, 1.0)).round() as usize;
    values.get(rank).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(
        id: &str,
        intent: &str,
        tokens: Option<(i64, i64)>,
        wall_ms: Option<i64>,
        corrected: bool,
    ) -> ExperienceRun {
        ExperienceRun {
            id: id.to_string(),
            execution_run_id: None,
            trace_id: None,
            conversation_id: None,
            project_id: None,
            channel: "web".to_string(),
            scope: "chat".to_string(),
            intent_key: intent.to_string(),
            task_type: Some("chat".to_string()),
            request_text: Some(format!("request for {intent}")),
            tool_sequence_digest: None,
            tool_sequence_json: serde_json::json!([]),
            strategy_version: None,
            policy_version: None,
            prompt_version: None,
            model_slot: None,
            tokens_in: tokens.map(|(input, _)| input),
            tokens_out: tokens.map(|(_, output)| output),
            wall_ms,
            est_cost_microusd: None,
            success_state: if corrected { "failed" } else { "accepted" }.to_string(),
            correction_state: if corrected { "corrected" } else { "none" }.to_string(),
            outcome_summary: None,
            failure_reason: None,
            metadata: serde_json::json!({}),
            consolidated: false,
            accepted_at: None,
            corrected_at: None,
            heuristic_reflected: false,
            heuristic_reflection_status: None,
            heuristic_reflection_attempted_at: None,
            heuristic_reflection_completed_at: None,
            heuristic_lesson_id: None,
            heuristic_reflection_error: None,
            created_at: "2026-06-10T00:00:00Z".to_string(),
            updated_at: "2026-06-10T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn window_groups_by_intent_and_computes_install_relative_baselines() {
        let runs = vec![
            run("a", "intent-a", Some((900, 100)), Some(2_000), false),
            run("b", "intent-a", Some((1100, 100)), Some(2_400), true),
            run("c", "intent-b", Some((90, 10)), Some(800), false),
        ];
        let window = UsageWindow::from_runs(runs, None);

        assert_eq!(window.segments().len(), 2);
        let segment_a = window
            .segments()
            .iter()
            .find(|segment| segment.key == "intent-a")
            .unwrap();
        assert_eq!(segment_a.sample_count, 2);
        assert_eq!(segment_a.label, "intent-a");
        assert_eq!(segment_a.corrected_count, 1);
        assert!(segment_a.avg_tokens.unwrap() > 1000.0);
        assert!(window.median_tokens.is_some());
        assert!(window.overall_corrected_rate > 0.0);
    }
}
