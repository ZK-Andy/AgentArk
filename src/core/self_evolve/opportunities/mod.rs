//! Generic per-user opportunity engine: mines the install's own usage
//! (honest-labeled experience runs) into optimization opportunities through
//! dimension-generic detectors. No use-case templates, no category enums, no
//! phrasing matches — segments are content-derived from the user's actual
//! activity and every surfaced opportunity passes an intent-based LLM value
//! verdict.

pub mod contract_events;
mod engine;
pub mod eval_sets;
mod miners;
mod window;

pub use engine::{maybe_run_opportunity_mining_pass, MiningPassSummary};
pub(crate) use window::UsageWindow;

/// Multidimensional expected benefit. None = no evidence for that dimension.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ExpectedBenefit {
    pub tokens_per_turn: Option<f64>,
    pub ms_per_turn: Option<f64>,
    pub corrected_rate_delta: Option<f64>,
    /// 0..1, derived from sample size relative to the window.
    pub confidence: f64,
}

/// Aggregated evidence behind a draft, persisted verbatim for the UI.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct OpportunityEvidence {
    pub sample_runs: usize,
    pub corrected_runs: usize,
    pub corrected_rate: f64,
    pub avg_tokens_per_turn: Option<f64>,
    pub p95_wall_ms: Option<i64>,
    pub avg_cost_microusd: Option<f64>,
    pub example_run_ids: Vec<String>,
    pub window_runs: usize,
}

/// A mined-but-not-yet-vetted opportunity emitted by a miner.
#[derive(Debug, Clone)]
pub struct OpportunityDraft {
    /// Internal detector id; never user-facing copy.
    pub miner_key: &'static str,
    /// Stable content-derived segment key (e.g. the runs' intent key).
    pub segment_key: String,
    /// Data-derived working label; the verdict pass writes the final one.
    pub segment_label: String,
    /// Optimization target surface.
    pub target_surface: String,
    pub evidence: OpportunityEvidence,
    pub expected_benefit: ExpectedBenefit,
    pub risk: String,
    pub holdout_run_ids: Vec<String>,
    /// Topic text used for semantic dedupe and as verdict context. Built from
    /// already-redacted run request texts.
    pub topic_text: String,
}

/// A usage-pattern detector. Implementations must be deterministic over the
/// window, bounded (no I/O, no allocation proportional to anything but the
/// window), and free of use-case assumptions: thresholds are relative to the
/// install's own distribution, never absolute topic/domain rules.
pub trait OpportunityMiner: Send + Sync {
    fn key(&self) -> &'static str;
    fn mine(&self, window: &UsageWindow) -> Vec<OpportunityDraft>;
}

/// Deterministic, process-independent content hash (FNV-1a, hex) for stable
/// opportunity identity. Not security-sensitive.
pub(crate) fn stable_content_hash(value: &str) -> String {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{:016x}", hash)
}

/// Stable opportunity id from the identity triple.
pub(crate) fn opportunity_id(miner_key: &str, segment_key: &str, target_surface: &str) -> String {
    format!(
        "evolve-opp-{}",
        stable_content_hash(&format!("{miner_key}\n{segment_key}\n{target_surface}"))
    )
}

/// Meaning-token jaccard for semantic dedupe of near-duplicate topics across
/// miners/passes. Tokens are content words; no curated stoplists beyond a
/// length floor.
pub(crate) fn topic_similarity(left: &str, right: &str) -> f64 {
    let tokenize = |value: &str| {
        value
            .to_lowercase()
            .chars()
            .map(|ch| if ch.is_alphanumeric() { ch } else { ' ' })
            .collect::<String>()
            .split_whitespace()
            .filter(|token| token.chars().count() >= 3)
            .map(str::to_string)
            .collect::<std::collections::HashSet<_>>()
    };
    let left_tokens = tokenize(left);
    let right_tokens = tokenize(right);
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0.0;
    }
    let overlap = left_tokens.intersection(&right_tokens).count() as f64;
    let union = left_tokens.union(&right_tokens).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        overlap / union
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opportunity_ids_are_stable_and_distinct() {
        let a = opportunity_id("token_hotspot", "intent:analyze-csv", "prompt_bundle");
        let b = opportunity_id("token_hotspot", "intent:analyze-csv", "prompt_bundle");
        let c = opportunity_id("latency_hotspot", "intent:analyze-csv", "prompt_bundle");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with("evolve-opp-"));
    }

    #[test]
    fn topic_similarity_tracks_meaning_overlap() {
        let high = topic_similarity(
            "analyze quarterly sales csv with pandas",
            "quarterly sales csv analyze with pandas again",
        );
        assert!(high > 0.7, "expected high similarity, got {high}");

        let low = topic_similarity(
            "analyze quarterly sales csv with pandas",
            "book flight tickets to tokyo",
        );
        assert!(low < 0.2, "expected low similarity, got {low}");
    }
}
