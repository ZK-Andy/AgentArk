//! Semantic outcome judge: decides whether the previous assistant answer in a
//! conversation actually served the user, using the user's next message as the
//! only evidence. Pure intent judgment — no keyword or phrasing rules. This is
//! what makes experience-run success labels honest instead of the blanket
//! timeout auto-accept.

use crate::core::model::llm::LlmClient;

const OUTCOME_JUDGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutcomeVerdict {
    /// The user built on, accepted, or moved forward from the answer.
    Served,
    /// The user corrected, contradicted, re-asked, or abandoned the answer.
    Corrected,
    /// Not enough signal either way (topic change, greeting, unrelated ask).
    Unclear,
}

fn outcome_judge_system_prompt() -> &'static str {
    "You judge whether an assistant's previous answer actually served the user, \
using only the user's next message as evidence. Work from underlying intent and \
meaning, not surface wording: differences in phrasing, language, tone, typos, \
politeness, or formatting must not change the verdict. Return corrected when the \
next message semantically corrects or contradicts the answer, re-asks the same \
need, says the answer was wrong or unhelpful, or restarts the same task another \
way. Return served when the next message builds on the answer, uses its results, \
acknowledges it and advances, or asks a natural follow-up that presumes the \
answer was good. Return unclear when the next message is unrelated, a topic \
change, or too thin to tell. When torn between corrected and unclear, prefer \
unclear — a false correction poisons learning more than a missed one. Return \
only JSON: {\"verdict\":\"served|corrected|unclear\",\"reason\":\"one short sentence\"}"
}

pub(crate) fn parse_outcome_verdict(text: &str) -> (OutcomeVerdict, Option<String>) {
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
        });
    let Some(value) = value else {
        return (OutcomeVerdict::Unclear, None);
    };
    let reason = value
        .get("reason")
        .and_then(|reason| reason.as_str())
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .map(str::to_string);
    let verdict = match value
        .get("verdict")
        .and_then(|verdict| verdict.as_str())
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("served") => OutcomeVerdict::Served,
        Some("corrected") => OutcomeVerdict::Corrected,
        _ => OutcomeVerdict::Unclear,
    };
    (verdict, reason)
}

/// Judge the prior answer. Fail-open: any error or timeout yields Unclear so
/// the run stays provisional and the existing timeout auto-accept applies.
pub(crate) async fn judge_prior_answer(
    llm: &LlmClient,
    prior_request: &str,
    prior_answer_summary: &str,
    next_user_message: &str,
) -> (OutcomeVerdict, Option<String>) {
    let user_message = format!(
        "Previous user request:\n{}\n\nAssistant's answer (summary):\n{}\n\nUser's next message:\n{}",
        prior_request, prior_answer_summary, next_user_message
    );
    match tokio::time::timeout(
        OUTCOME_JUDGE_TIMEOUT,
        llm.chat_with_system(outcome_judge_system_prompt(), &user_message),
    )
    .await
    {
        Ok(Ok(response)) => parse_outcome_verdict(&response.content),
        Ok(Err(error)) => {
            tracing::warn!(error = %error, "outcome judge call failed; leaving run provisional");
            (OutcomeVerdict::Unclear, None)
        }
        Err(_) => {
            tracing::warn!("outcome judge timed out; leaving run provisional");
            (OutcomeVerdict::Unclear, None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clean_verdicts() {
        let (verdict, reason) =
            parse_outcome_verdict(r#"{"verdict":"served","reason":"user built on it"}"#);
        assert_eq!(verdict, OutcomeVerdict::Served);
        assert_eq!(reason.as_deref(), Some("user built on it"));

        let (verdict, _) =
            parse_outcome_verdict(r#"{"verdict":"corrected","reason":"re-asked the same need"}"#);
        assert_eq!(verdict, OutcomeVerdict::Corrected);
    }

    #[test]
    fn parses_verdict_embedded_in_prose_and_falls_back_to_unclear() {
        let (verdict, _) = parse_outcome_verdict(
            "Sure, here is the JSON: {\"verdict\":\"SERVED\",\"reason\":\"ok\"} hope that helps",
        );
        assert_eq!(verdict, OutcomeVerdict::Served);

        let (verdict, reason) = parse_outcome_verdict("no json at all");
        assert_eq!(verdict, OutcomeVerdict::Unclear);
        assert_eq!(reason, None);

        let (verdict, _) = parse_outcome_verdict(r#"{"verdict":"banana"}"#);
        assert_eq!(verdict, OutcomeVerdict::Unclear);

        let (verdict, _) = parse_outcome_verdict(r#"{"reason":"missing verdict"}"#);
        assert_eq!(verdict, OutcomeVerdict::Unclear);
    }
}
