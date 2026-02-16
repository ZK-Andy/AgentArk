use std::collections::HashSet;

use crate::actions::ActionDef;

pub const DEFAULT_ACTION_INTENT_THRESHOLD: f32 = 0.45;

fn tokenize(text: &str) -> HashSet<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter_map(|w| {
            let w = w.trim();
            if w.len() < 3 || w.chars().all(|c| c.is_ascii_digit()) {
                None
            } else {
                Some(w.to_string())
            }
        })
        .collect()
}

fn action_tokens(action: &ActionDef) -> HashSet<String> {
    let mut tokens = HashSet::new();
    let name_words = action.name.replace('_', " ");
    tokens.extend(tokenize(&name_words));
    tokens.extend(tokenize(&action.description));
    for cap in &action.capabilities {
        tokens.extend(tokenize(cap));
    }
    tokens
}

pub fn action_intent_score(message: &str, action: &ActionDef) -> f32 {
    let msg_tokens = tokenize(message);
    if msg_tokens.is_empty() {
        return 0.0;
    }

    let action_tokens = action_tokens(action);
    if action_tokens.is_empty() {
        return 0.0;
    }

    let overlap = msg_tokens.intersection(&action_tokens).count() as f32;
    let coverage = overlap / msg_tokens.len() as f32;
    let precision = overlap / action_tokens.len() as f32;
    // Coverage dominates because action descriptions are often long.
    let mut score = (0.8 * coverage) + (0.2 * (precision * 6.0).min(1.0));

    let message_lc = message.to_lowercase();
    let exact_name = action.name.to_lowercase();
    if message_lc.contains(&exact_name) {
        score = score.max(0.95);
    } else {
        let spaced_name = exact_name.replace('_', " ");
        if message_lc.contains(&spaced_name) {
            score = score.max(0.90);
        }
    }

    score.clamp(0.0, 1.0)
}

pub fn has_action_intent(
    message: &str,
    actions: &[ActionDef],
    action_name: &str,
    threshold: f32,
) -> bool {
    actions
        .iter()
        .find(|a| a.name == action_name)
        .map(|a| action_intent_score(message, a) >= threshold)
        .unwrap_or(false)
}

pub fn has_action_intent_default(message: &str, actions: &[ActionDef], action_name: &str) -> bool {
    has_action_intent(
        message,
        actions,
        action_name,
        DEFAULT_ACTION_INTENT_THRESHOLD,
    )
}
