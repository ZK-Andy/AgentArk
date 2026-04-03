//! Multi-turn chat flows for onboarding integrations (connect + secret setup).
//!
//! This is intentionally lightweight:
//! - Start when the user asks to connect a known integration.
//! - Ask the user to provide required secrets via chat-safe commands.
//! - When secrets are saved, run a connectivity check and enable on success.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretRequirementKind {
    All,
    Any,
}

#[derive(Debug, Clone, Copy)]
pub struct SecretRequirement {
    pub kind: SecretRequirementKind,
    pub keys: &'static [&'static str],
}

#[derive(Debug, Clone, Copy)]
pub struct IntegrationConnectSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub triggers: &'static [&'static str],
    pub required: SecretRequirement,
    pub optional: &'static [&'static str],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingIntegrationConnect {
    pub integration_id: String,
    pub started_at: DateTime<Utc>,
}

pub const CONNECT_FLOW_TTL_SECS: i64 = 20 * 60;

fn normalize_phrase(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_was_space = true;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_space = false;
        } else if !last_was_space {
            out.push(' ');
            last_was_space = true;
        }
    }
    out.trim().to_string()
}

fn contains_normalized_phrase(haystack: &str, phrase: &str) -> bool {
    let haystack = format!(" {} ", normalize_phrase(haystack));
    let needle = format!(" {} ", normalize_phrase(phrase));
    !needle.trim().is_empty() && haystack.contains(&needle)
}

fn message_contains_any_phrase(message: &str, phrases: &[&str]) -> bool {
    phrases
        .iter()
        .any(|phrase| contains_normalized_phrase(message, phrase))
}

fn contains_normalized_token(haystack: &str, token: &str) -> bool {
    let haystack_tokens = normalize_phrase(haystack)
        .split_whitespace()
        .map(|word| word.to_string())
        .collect::<std::collections::BTreeSet<_>>();
    haystack_tokens.contains(&token.trim().to_ascii_lowercase())
}

fn message_contains_any_token(message: &str, tokens: &[&str]) -> bool {
    tokens
        .iter()
        .any(|token| contains_normalized_token(message, token))
}

fn connect_trigger_matches(message: &str, trigger: &str) -> bool {
    let normalized = normalize_phrase(trigger);
    if normalized.split_whitespace().count() <= 1 {
        return contains_normalized_token(message, trigger);
    }
    contains_normalized_phrase(message, trigger)
}

static SPECS: &[IntegrationConnectSpec] = &[
    IntegrationConnectSpec {
        id: "github",
        name: "GitHub",
        triggers: &["github"],
        required: SecretRequirement {
            kind: SecretRequirementKind::All,
            keys: &["GITHUB_TOKEN"],
        },
        optional: &[],
    },
    IntegrationConnectSpec {
        id: "notion",
        name: "Notion",
        triggers: &["notion"],
        required: SecretRequirement {
            kind: SecretRequirementKind::All,
            keys: &["NOTION_TOKEN"],
        },
        optional: &[],
    },
    IntegrationConnectSpec {
        id: "twitter",
        name: "X (Twitter)",
        triggers: &["twitter", "x api", "x.com"],
        required: SecretRequirement {
            kind: SecretRequirementKind::All,
            keys: &["TWITTER_BEARER_TOKEN"],
        },
        optional: &[],
    },
    IntegrationConnectSpec {
        id: "onepassword",
        name: "1Password Connect",
        triggers: &["1password", "onepassword", "1 password"],
        required: SecretRequirement {
            kind: SecretRequirementKind::All,
            keys: &["ONEPASSWORD_TOKEN"],
        },
        optional: &["ONEPASSWORD_HOST"],
    },
    IntegrationConnectSpec {
        id: "google_places",
        name: "Google Places",
        triggers: &["google places", "places"],
        required: SecretRequirement {
            kind: SecretRequirementKind::All,
            keys: &["GOOGLE_PLACES_API_KEY"],
        },
        optional: &[],
    },
    IntegrationConnectSpec {
        id: "twilio",
        name: "Twilio",
        triggers: &["twilio"],
        required: SecretRequirement {
            kind: SecretRequirementKind::All,
            keys: &[
                "TWILIO_ACCOUNT_SID",
                "TWILIO_AUTH_TOKEN",
                "TWILIO_FROM_NUMBER",
            ],
        },
        optional: &[],
    },
    IntegrationConnectSpec {
        id: "ordering",
        name: "Ordering",
        triggers: &["ordering", "shopify"],
        required: SecretRequirement {
            kind: SecretRequirementKind::All,
            keys: &["ORDERING_CONFIG_JSON"],
        },
        optional: &[
            "SHOPIFY_ACCESS_TOKEN",
            "SHOPIFY_STORE_URL",
            "ORDERING_WEBHOOK_URL",
        ],
    },
    IntegrationConnectSpec {
        id: "garmin",
        name: "Garmin",
        triggers: &["garmin"],
        required: SecretRequirement {
            kind: SecretRequirementKind::All,
            keys: &["GARMIN_TOKEN"],
        },
        optional: &["GARMIN_API_BASE"],
    },
    IntegrationConnectSpec {
        id: "whoop",
        name: "WHOOP",
        triggers: &["whoop"],
        required: SecretRequirement {
            kind: SecretRequirementKind::All,
            keys: &["WHOOP_TOKEN"],
        },
        optional: &[],
    },
    IntegrationConnectSpec {
        id: "ga4",
        name: "Google Analytics 4 (GA4)",
        triggers: &["ga4", "google analytics"],
        required: SecretRequirement {
            kind: SecretRequirementKind::All,
            keys: &["GA4_ACCESS_TOKEN"],
        },
        optional: &["GA4_PROPERTY_ID"],
    },
    IntegrationConnectSpec {
        id: "gsc",
        name: "Google Search Console (GSC)",
        triggers: &["gsc", "search console"],
        required: SecretRequirement {
            kind: SecretRequirementKind::All,
            keys: &["GSC_ACCESS_TOKEN"],
        },
        optional: &["GSC_SITE_URL"],
    },
    IntegrationConnectSpec {
        id: "social_analytics",
        name: "Social Analytics",
        triggers: &["social analytics", "social_analytics", "social"],
        required: SecretRequirement {
            kind: SecretRequirementKind::Any,
            keys: &[
                "SOCIAL_TWITTER_BEARER_TOKEN",
                "SOCIAL_GA4_ACCESS_TOKEN",
                "SOCIAL_GA4_PROPERTY_ID",
            ],
        },
        optional: &[],
    },
    IntegrationConnectSpec {
        id: "moltbook",
        name: "Moltbook",
        triggers: &["moltbook"],
        required: SecretRequirement {
            kind: SecretRequirementKind::All,
            keys: &["MOLTBOOK_API_KEY"],
        },
        optional: &[],
    },
];

pub fn all_specs() -> &'static [IntegrationConnectSpec] {
    SPECS
}

pub fn spec_by_id(id: &str) -> Option<&'static IntegrationConnectSpec> {
    SPECS.iter().find(|s| s.id == id)
}

fn looks_like_connect_intent(message_lc: &str) -> bool {
    message_contains_any_phrase(
        message_lc,
        &[
            "set up",
            "setup",
            "configure",
            "add integration",
            "enable integration",
            "grant access",
            "give access",
            "request access",
            "need access",
            "sign in",
            "log in",
            "wire up",
            "hook up",
        ],
    ) || message_contains_any_token(
        message_lc,
        &[
            "connect",
            "link",
            "integrate",
            "authorize",
            "authenticate",
            "token",
            "credentials",
            "secret",
            "sync",
            "pair",
        ],
    ) || message_contains_any_phrase(message_lc, &["api key"])
}

pub fn detect_connect_integration(message: &str) -> Option<&'static IntegrationConnectSpec> {
    let lc = message.trim().to_ascii_lowercase();
    if lc.is_empty() {
        return None;
    }
    if !looks_like_connect_intent(&lc) {
        return None;
    }
    SPECS.iter().find(|spec| {
        spec.triggers
            .iter()
            .any(|t| connect_trigger_matches(&lc, t))
    })
}

pub fn is_cancel_message(message: &str) -> bool {
    let lc = message.trim().to_ascii_lowercase();
    let normalized = normalize_phrase(&lc);
    normalized == "cancel"
        || lc.trim() == "/cancel"
        || contains_normalized_phrase(&lc, "cancel setup")
        || contains_normalized_phrase(&lc, "never mind")
        || contains_normalized_phrase(&lc, "nevermind")
        || contains_normalized_phrase(&lc, "stop setup")
        || contains_normalized_phrase(&lc, "abort setup")
}

pub fn connect_instructions(spec: &IntegrationConnectSpec) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Integration setup: {} (`{}`)\n\n",
        spec.name, spec.id
    ));
    out.push_str("Send secrets using one of these safe commands:\n");
    out.push_str("- Telegram/WhatsApp: `/setsecret KEY=VALUE`\n");
    out.push_str("- Web chat: `set secret KEY=VALUE`\n\n");

    match spec.required.kind {
        SecretRequirementKind::All => {
            out.push_str("Required:\n");
            for k in spec.required.keys {
                out.push_str(&format!("- `{}`\n", k));
            }
        }
        SecretRequirementKind::Any => {
            out.push_str("Provide at least one of:\n");
            for k in spec.required.keys {
                out.push_str(&format!("- `{}`\n", k));
            }
        }
    }

    if !spec.optional.is_empty() {
        out.push_str("\nOptional:\n");
        for k in spec.optional {
            out.push_str(&format!("- `{}`\n", k));
        }
    }

    out.push_str("\nAfter you set the secret(s), I will run a connection test and enable it if successful.\n");
    out.push_str("To cancel: `cancel setup`.\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_connect_requests_with_normalized_paraphrases() {
        let spec = detect_connect_integration("Please set-up GitHub for me").expect("github spec");
        assert_eq!(spec.id, "github");

        let spec = detect_connect_integration("I need GitHub access").expect("github spec");
        assert_eq!(spec.id, "github");
    }

    #[test]
    fn matches_hyphenated_integration_names() {
        let spec =
            detect_connect_integration("Connect Google-Places for me").expect("google places");
        assert_eq!(spec.id, "google_places");
    }

    #[test]
    fn does_not_treat_plain_mentions_as_connect_requests() {
        assert!(detect_connect_integration("Summarize GitHub issues for me").is_none());
    }

    #[test]
    fn does_not_match_connectivity_or_other_non_intents() {
        assert!(detect_connect_integration("This is a connectivity report for GitHub").is_none());
        assert!(detect_connect_integration("GitHub docs update").is_none());
    }

    #[test]
    fn matches_single_word_triggers_via_token_boundaries() {
        let spec = detect_connect_integration("Please connect GitHub").expect("github spec");
        assert_eq!(spec.id, "github");
    }
}
