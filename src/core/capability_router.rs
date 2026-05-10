use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    sync::Arc,
};

#[cfg(test)]
use std::cell::RefCell;
use crate::actions::ActionDef;
use dashmap::DashMap;
use once_cell::sync::Lazy;

const INTENT_NGRAM_WIDTH: usize = 3;
const ACTION_PROFILE_CACHE_LIMIT: usize = 512;
const TEXT_PROFILE_CACHE_LIMIT: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ProfileKey {
    len: usize,
    hash: u64,
}

#[derive(Debug)]
struct IntentTextProfile {
    source_text: Arc<str>,
    tokens: HashSet<String>,
    token_ngrams: HashMap<String, HashSet<String>>,
    text_ngrams: HashSet<String>,
}

#[derive(Debug)]
struct ActionIntentProfile {
    #[cfg(test)]
    action_name: Arc<str>,
    #[cfg(test)]
    action_version: Arc<str>,
    descriptor_text: Arc<str>,
    tokens: HashSet<String>,
    token_ngrams: HashMap<String, HashSet<String>>,
    text_ngrams: HashSet<String>,
}

#[cfg(test)]
#[derive(Debug)]
struct ActionIntentProfileScope {
    profiles: HashMap<String, Arc<ActionIntentProfile>>,
}

static TEXT_PROFILE_CACHE: Lazy<DashMap<ProfileKey, Arc<IntentTextProfile>>> =
    Lazy::new(DashMap::new);
static ACTION_PROFILE_CACHE: Lazy<DashMap<ProfileKey, Arc<ActionIntentProfile>>> =
    Lazy::new(DashMap::new);

#[cfg(test)]
thread_local! {
    static ACTION_PROFILE_SCOPE: RefCell<Vec<Arc<ActionIntentProfileScope>>> =
        const { RefCell::new(Vec::new()) };
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub struct RankedCapabilityAction {
    pub action: ActionDef,
    pub score: f32,
    pub second_score: f32,
}

fn normalized_text(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { ' ' })
        .collect()
}

fn profile_key(value: &str) -> ProfileKey {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.len().hash(&mut hasher);
    value.hash(&mut hasher);
    ProfileKey {
        len: value.len(),
        hash: hasher.finish(),
    }
}

fn clear_cache_if_needed<K, V>(cache: &DashMap<K, V>, limit: usize)
where
    K: Eq + Hash,
{
    if cache.len() >= limit {
        cache.clear();
    }
}

fn tokenize(value: &str) -> HashSet<String> {
    normalized_text(value)
        .split_whitespace()
        .filter(|token| token.len() >= 2)
        .map(ToString::to_string)
        .collect()
}

fn char_ngrams(value: &str, width: usize) -> HashSet<String> {
    let compact = normalized_text(value)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if compact.is_empty() {
        return HashSet::new();
    }
    let chars = compact.chars().collect::<Vec<_>>();
    if chars.len() <= width {
        return [compact].into_iter().collect();
    }
    (0..=chars.len().saturating_sub(width))
        .map(|index| chars[index..index + width].iter().collect::<String>())
        .collect()
}

fn jaccard_similarity(left: &HashSet<String>, right: &HashSet<String>) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let overlap = left.intersection(right).count() as f32;
    let union = left.union(right).count() as f32;
    if union <= 0.0 {
        0.0
    } else {
        overlap / union
    }
}

fn token_similarity(left: &str, right: &str) -> f32 {
    if left == right {
        return 1.0;
    }
    let min_len = left.len().min(right.len()) as f32;
    let max_len = left.len().max(right.len()) as f32;
    if max_len <= 0.0 {
        return 0.0;
    }
    if left.starts_with(right) || right.starts_with(left) {
        return (0.75 + (min_len / max_len) * 0.25).clamp(0.0, 1.0);
    }
    let left_ngrams = char_ngrams(left, INTENT_NGRAM_WIDTH);
    let right_ngrams = char_ngrams(right, INTENT_NGRAM_WIDTH);
    jaccard_similarity(&left_ngrams, &right_ngrams)
}

fn token_similarity_from_profiles(
    left: &str,
    right: &str,
    left_token_ngrams: &HashMap<String, HashSet<String>>,
    right_token_ngrams: &HashMap<String, HashSet<String>>,
) -> f32 {
    if left == right {
        return 1.0;
    }
    let min_len = left.len().min(right.len()) as f32;
    let max_len = left.len().max(right.len()) as f32;
    if max_len <= 0.0 {
        return 0.0;
    }
    if left.starts_with(right) || right.starts_with(left) {
        return (0.75 + (min_len / max_len) * 0.25).clamp(0.0, 1.0);
    }
    match (left_token_ngrams.get(left), right_token_ngrams.get(right)) {
        (Some(left_ngrams), Some(right_ngrams)) => jaccard_similarity(left_ngrams, right_ngrams),
        _ => token_similarity(left, right),
    }
}

fn soft_token_overlap_from_profiles(
    left: &HashSet<String>,
    right: &HashSet<String>,
    left_token_ngrams: &HashMap<String, HashSet<String>>,
    right_token_ngrams: &HashMap<String, HashSet<String>>,
) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    left.iter()
        .map(|left_token| {
            right
                .iter()
                .map(|right_token| {
                    token_similarity_from_profiles(
                        left_token,
                        right_token,
                        left_token_ngrams,
                        right_token_ngrams,
                    )
                })
                .fold(0.0f32, f32::max)
        })
        .sum::<f32>()
        / left.len() as f32
}

fn schema_tokens(value: &serde_json::Value, out: &mut HashSet<String>) {
    match value {
        serde_json::Value::String(text) => {
            out.extend(tokenize(text));
        }
        serde_json::Value::Array(items) => {
            for item in items {
                schema_tokens(item, out);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                out.extend(tokenize(key));
                schema_tokens(value, out);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
}

fn planner_metadata_tokens(action: &ActionDef) -> HashSet<String> {
    let metadata = action.planner_metadata();
    tokenize(&serde_json::to_string(&metadata).unwrap_or_else(|_| format!("{:?}", metadata)))
}

fn action_descriptor_text(action: &ActionDef) -> String {
    format!(
        "{} {} {} {} {}",
        action.name,
        action.description,
        action.capabilities.join(" "),
        serde_json::to_string(&action.input_schema).unwrap_or_default(),
        serde_json::to_string(&action.planner_metadata()).unwrap_or_default(),
    )
}

fn token_ngram_map(tokens: &HashSet<String>) -> HashMap<String, HashSet<String>> {
    tokens
        .iter()
        .map(|token| (token.clone(), char_ngrams(token, INTENT_NGRAM_WIDTH)))
        .collect()
}

fn build_text_profile(message: &str) -> Arc<IntentTextProfile> {
    let tokens = tokenize(message);
    Arc::new(IntentTextProfile {
        source_text: Arc::<str>::from(message.to_string()),
        token_ngrams: token_ngram_map(&tokens),
        text_ngrams: char_ngrams(message, INTENT_NGRAM_WIDTH),
        tokens,
    })
}

fn cached_text_profile(message: &str) -> Arc<IntentTextProfile> {
    let key = profile_key(message);
    if let Some(profile) = TEXT_PROFILE_CACHE.get(&key) {
        if profile.source_text.as_ref() == message {
            return Arc::clone(profile.value());
        }
    }
    let profile = build_text_profile(message);
    clear_cache_if_needed(&TEXT_PROFILE_CACHE, TEXT_PROFILE_CACHE_LIMIT);
    TEXT_PROFILE_CACHE.insert(key, Arc::clone(&profile));
    profile
}

fn build_action_profile(action: &ActionDef, descriptor_text: String) -> Arc<ActionIntentProfile> {
    let mut tokens = tokenize(&descriptor_text);
    schema_tokens(&action.input_schema, &mut tokens);
    tokens.extend(planner_metadata_tokens(action));
    let text_ngrams = char_ngrams(&descriptor_text, INTENT_NGRAM_WIDTH);
    Arc::new(ActionIntentProfile {
        #[cfg(test)]
        action_name: Arc::<str>::from(action.name.clone()),
        #[cfg(test)]
        action_version: Arc::<str>::from(action.version.clone()),
        descriptor_text: Arc::<str>::from(descriptor_text),
        token_ngrams: token_ngram_map(&tokens),
        text_ngrams,
        tokens,
    })
}

#[cfg(test)]
impl ActionIntentProfileScope {
    fn new(actions: &[ActionDef]) -> Self {
        let profiles = actions
            .iter()
            .map(|action| {
                (
                    action.name.clone(),
                    build_action_profile(action, action_descriptor_text(action)),
                )
            })
            .collect();
        Self { profiles }
    }
}

#[cfg(test)]
struct ActionIntentProfileScopeGuard;

#[cfg(test)]
impl Drop for ActionIntentProfileScopeGuard {
    fn drop(&mut self) {
        ACTION_PROFILE_SCOPE.with(|scope| {
            scope.borrow_mut().pop();
        });
    }
}

#[cfg(test)]
pub(crate) fn with_action_intent_profiles<R>(actions: &[ActionDef], run: impl FnOnce() -> R) -> R {
    let scope = Arc::new(ActionIntentProfileScope::new(actions));
    ACTION_PROFILE_SCOPE.with(|cell| {
        cell.borrow_mut().push(scope);
    });
    let _guard = ActionIntentProfileScopeGuard;
    run()
}

#[cfg(test)]
fn scoped_action_profile(action: &ActionDef) -> Option<Arc<ActionIntentProfile>> {
    ACTION_PROFILE_SCOPE.with(|cell| {
        let scopes = cell.borrow();
        let profile = scopes
            .last()
            .and_then(|scope| scope.profiles.get(&action.name))
            .cloned()?;
        (profile.action_name.as_ref() == action.name
            && profile.action_version.as_ref() == action.version)
            .then_some(profile)
    })
}

#[cfg(not(test))]
fn scoped_action_profile(_action: &ActionDef) -> Option<Arc<ActionIntentProfile>> {
    None
}

fn cached_action_profile(action: &ActionDef) -> Arc<ActionIntentProfile> {
    if let Some(profile) = scoped_action_profile(action) {
        return profile;
    }
    let descriptor_text = action_descriptor_text(action);
    let key = profile_key(&descriptor_text);
    if let Some(profile) = ACTION_PROFILE_CACHE.get(&key) {
        if profile.descriptor_text.as_ref() == descriptor_text.as_str() {
            return Arc::clone(profile.value());
        }
    }
    let profile = build_action_profile(action, descriptor_text);
    clear_cache_if_needed(&ACTION_PROFILE_CACHE, ACTION_PROFILE_CACHE_LIMIT);
    ACTION_PROFILE_CACHE.insert(key, Arc::clone(&profile));
    profile
}

fn score_profiles(
    request_profile: &IntentTextProfile,
    action_profile: &ActionIntentProfile,
    include_reasons: bool,
) -> (f32, Vec<String>) {
    let request_tokens = &request_profile.tokens;
    let action_tokens = &action_profile.tokens;
    let overlap_count = request_tokens.intersection(action_tokens).count() as f32;
    let request_coverage = if request_tokens.is_empty() {
        0.0
    } else {
        overlap_count / request_tokens.len() as f32
    };
    let action_coverage = if action_tokens.is_empty() {
        0.0
    } else {
        overlap_count / action_tokens.len() as f32
    };
    let fuzzy_request_coverage = soft_token_overlap_from_profiles(
        request_tokens,
        action_tokens,
        &request_profile.token_ngrams,
        &action_profile.token_ngrams,
    );
    let fuzzy_action_coverage = soft_token_overlap_from_profiles(
        action_tokens,
        request_tokens,
        &action_profile.token_ngrams,
        &request_profile.token_ngrams,
    );
    let trigram_similarity =
        jaccard_similarity(&request_profile.text_ngrams, &action_profile.text_ngrams);

    let exact_score = (request_coverage + action_coverage) / 2.0;
    let fuzzy_score = (fuzzy_request_coverage + fuzzy_action_coverage) / 2.0;
    let score = (exact_score * 0.4 + fuzzy_score * 0.4 + trigram_similarity * 0.2).clamp(0.0, 1.0);

    if !include_reasons {
        return (score, Vec::new());
    }

    let mut reasons = Vec::new();
    if overlap_count > 0.0 {
        reasons.push(format!("catalog metadata overlap {:.0}", overlap_count));
    }
    if fuzzy_request_coverage >= 0.18 || fuzzy_action_coverage >= 0.18 {
        reasons.push(format!(
            "fuzzy intent overlap {:.2}",
            ((fuzzy_request_coverage + fuzzy_action_coverage) / 2.0)
        ));
    }
    if trigram_similarity >= 0.12 {
        reasons.push(format!("phrase similarity {:.2}", trigram_similarity));
    }

    (score, reasons)
}

pub fn score_action_intent(message: &str, action: &ActionDef) -> f32 {
    let request_profile = cached_text_profile(message);
    let action_profile = cached_action_profile(action);
    score_profiles(&request_profile, &action_profile, false).0
}

#[cfg(test)]
pub fn score_action_intent_with_reasons(message: &str, action: &ActionDef) -> (f32, Vec<String>) {
    let request_profile = cached_text_profile(message);
    let action_profile = cached_action_profile(action);
    score_profiles(&request_profile, &action_profile, true)
}

#[cfg(test)]
pub fn ranked_action_candidates(
    message: &str,
    all_actions: &[ActionDef],
    boosted_action_names: &HashSet<String>,
) -> Vec<RankedCapabilityAction> {
    let mut ranked = all_actions
        .iter()
        .map(|action| {
            let score = score_action_intent(message, action);
            RankedCapabilityAction {
                action: action.clone(),
                score,
                second_score: 0.0,
            }
        })
        .collect::<Vec<_>>();

    ranked.sort_by(|left, right| {
        let left_boosted = boosted_action_names.contains(&left.action.name);
        let right_boosted = boosted_action_names.contains(&right.action.name);
        right_boosted
            .cmp(&left_boosted)
            .then_with(|| {
                right
                    .score
                    .partial_cmp(&left.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| left.action.name.cmp(&right.action.name))
    });

    let scores = ranked.iter().map(|item| item.score).collect::<Vec<_>>();
    for (index, item) in ranked.iter_mut().enumerate() {
        item.second_score = scores
            .iter()
            .enumerate()
            .find_map(|(candidate_index, score)| (candidate_index != index).then_some(*score))
            .unwrap_or(0.0);
    }
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(name: &str, description: &str, capabilities: &[&str]) -> ActionDef {
        ActionDef {
            name: name.to_string(),
            description: description.to_string(),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Topic or query" }
                }
            }),
            capabilities: capabilities.iter().map(|value| value.to_string()).collect(),
            sandbox_mode: None,
            source: crate::actions::ActionSource::System,
            file_path: None,
            authorization: Default::default(),
        }
    }

    #[test]
    fn scorer_ranks_unknown_custom_action_from_live_metadata() {
        let actions = vec![
            action("generic_tool", "General utility", &[]),
            action(
                "custom_action_alpha",
                "Handles alpha beta gamma requests",
                &["alpha_capability"],
            ),
        ];

        let ranked =
            ranked_action_candidates("Please handle alpha beta gamma", &actions, &HashSet::new());

        assert_eq!(ranked[0].action.name, "custom_action_alpha");
    }

    #[test]
    fn scorer_handles_typos_and_paraphrase_without_exact_token_overlap() {
        let actions = vec![
            action(
                "watch",
                "Monitor a source repeatedly and alert on changes",
                &["watcher"],
            ),
            action(
                "app_deploy",
                "Build, deploy, and expose an application",
                &["app_hosting"],
            ),
        ];

        let ranked = ranked_action_candidates(
            "montior this every 10 sec and tell me when something changes",
            &actions,
            &HashSet::new(),
        );

        assert_eq!(ranked[0].action.name, "watch");
    }

    #[test]
    fn scoped_action_profiles_preserve_public_score() {
        let actions = vec![
            action("generic_tool", "General utility", &[]),
            action(
                "custom_action_alpha",
                "Handles alpha beta gamma requests",
                &["alpha_capability"],
            ),
        ];
        let message = "Please handle alpha beta gamma";
        let outside = score_action_intent(message, &actions[1]);
        let inside =
            with_action_intent_profiles(&actions, || score_action_intent(message, &actions[1]));
        let reasoned = score_action_intent_with_reasons(message, &actions[1]).0;

        assert!((outside - inside).abs() < f32::EPSILON);
        assert!((inside - reasoned).abs() < f32::EPSILON);
    }
}
