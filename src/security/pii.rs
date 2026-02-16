//! PII detection and redaction for logs and outputs
//!
//! Provides fast regex-based detection of personally identifiable information
//! including emails, phone numbers, SSNs, credit cards, and IP addresses.
//! Redaction is non-reversible — original data is permanently removed.

use once_cell::sync::Lazy;
use regex::Regex;

static EMAIL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b").unwrap());

static PHONE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?:\+?\d{1,3}[-.\s]?)?\(?\d{2,4}\)?[-.\s]?\d{3,4}[-.\s]?\d{4}").unwrap()
});

static SSN_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap());

static CREDIT_CARD_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b\d{4}[\s-]?\d{4}[\s-]?\d{4}[\s-]?\d{4}\b").unwrap());

static IPV4_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b").unwrap()
});

/// PII redactor with configurable pattern toggles
pub struct PiiRedactor {
    pub redact_emails: bool,
    pub redact_phones: bool,
    pub redact_ssn: bool,
    pub redact_credit_cards: bool,
    pub redact_ips: bool,
}

impl Default for PiiRedactor {
    fn default() -> Self {
        Self::new()
    }
}

impl PiiRedactor {
    /// Create a new PII redactor with all detection enabled
    pub fn new() -> Self {
        Self {
            redact_emails: true,
            redact_phones: true,
            redact_ssn: true,
            redact_credit_cards: true,
            redact_ips: true,
        }
    }

    /// Redact PII from text. Non-reversible.
    pub fn redact(&self, text: &str) -> String {
        let mut result = text.to_string();

        if self.redact_ssn {
            result = SSN_RE.replace_all(&result, "[SSN]").to_string();
        }
        if self.redact_credit_cards {
            result = CREDIT_CARD_RE.replace_all(&result, "[CARD]").to_string();
        }
        if self.redact_emails {
            result = EMAIL_RE.replace_all(&result, "[EMAIL]").to_string();
        }
        if self.redact_phones {
            result = PHONE_RE.replace_all(&result, "[PHONE]").to_string();
        }
        if self.redact_ips {
            result = IPV4_RE.replace_all(&result, "[IP]").to_string();
        }

        result
    }
}

/// Convenience function for quick redaction with all patterns enabled
pub fn redact_pii(text: &str) -> String {
    PiiRedactor::new().redact(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_redaction() {
        assert_eq!(
            redact_pii("Contact john.doe@example.com for details"),
            "Contact [EMAIL] for details"
        );
    }

    #[test]
    fn test_phone_redaction() {
        assert_eq!(redact_pii("Call 555-123-4567"), "Call [PHONE]");
        assert!(redact_pii("+1 (555) 987-6543").contains("[PHONE]"));
    }

    #[test]
    fn test_ssn_redaction() {
        assert_eq!(redact_pii("SSN: 123-45-6789"), "SSN: [SSN]");
    }

    #[test]
    fn test_credit_card_redaction() {
        assert_eq!(redact_pii("Card: 4111 1111 1111 1111"), "Card: [CARD]");
        assert!(redact_pii("4111-1111-1111-1111").contains("[CARD]"));
    }

    #[test]
    fn test_ip_redaction() {
        assert_eq!(redact_pii("Server at 192.168.1.100"), "Server at [IP]");
    }

    #[test]
    fn test_multiple_pii() {
        let input = "Email: user@test.com, Phone: 555-123-4567, IP: 10.0.0.1";
        let result = redact_pii(input);
        assert!(result.contains("[EMAIL]"));
        assert!(result.contains("[PHONE]"));
        assert!(result.contains("[IP]"));
        assert!(!result.contains("user@test.com"));
        assert!(!result.contains("555-123"));
        assert!(!result.contains("10.0.0.1"));
    }

    #[test]
    fn test_no_false_positives_on_clean_text() {
        let input = "Hello, how are you today?";
        assert_eq!(redact_pii(input), input);
    }

    #[test]
    fn test_selective_redaction() {
        let mut redactor = PiiRedactor::new();
        redactor.redact_emails = false;
        let result = redactor.redact("Email: user@test.com, SSN: 123-45-6789");
        assert!(result.contains("user@test.com"));
        assert!(result.contains("[SSN]"));
    }
}
