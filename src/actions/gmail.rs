//! Gmail integration (scan and reply)

use anyhow::{anyhow, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const TOKEN_FILE: &str = "gmail.json";
const GMAIL_SECRET_KEY: &str = "gmail_tokens";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GMAIL_API_BASE: &str = "https://gmail.googleapis.com/gmail/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GmailTokens {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
    #[serde(default)]
    refresh_token: Option<String>,
}

fn token_path(config_dir: &Path) -> PathBuf {
    config_dir.join(TOKEN_FILE)
}

fn get_oauth_client_with_config(config_dir: &Path) -> Result<(String, String)> {
    // Try env vars first
    if let (Ok(id), Ok(secret)) = (std::env::var("GMAIL_CLIENT_ID"), std::env::var("GMAIL_CLIENT_SECRET")) {
        return Ok((id, secret));
    }
    // Fall back to secure config
    let manager = crate::core::config::SecureConfigManager::new(config_dir)?;
    if let Some(json_str) = manager.get_custom_secret("gmail_oauth_config")? {
        let v: serde_json::Value = serde_json::from_str(&json_str)?;
        let client_id = v.get("client_id").and_then(|c| c.as_str()).map(String::from)
            .ok_or_else(|| anyhow!("Missing client_id in gmail config"))?;
        let client_secret = v.get("client_secret").and_then(|c| c.as_str()).map(String::from)
            .ok_or_else(|| anyhow!("Missing client_secret in gmail config"))?;
        return Ok((client_id, client_secret));
    }
    Err(anyhow!("Gmail OAuth credentials not configured. Go to Settings > Gmail to add them."))
}

async fn load_tokens(config_dir: &Path) -> Result<GmailTokens> {
    let manager = crate::core::config::SecureConfigManager::new(config_dir)?;
    if let Some(payload) = manager.get_custom_secret(GMAIL_SECRET_KEY)? {
        let tokens: GmailTokens = serde_json::from_str(&payload)?;
        return Ok(tokens);
    }

    // Migration from legacy plaintext file if it exists
    let legacy_path = token_path(config_dir);
    if legacy_path.exists() {
        let content = tokio::fs::read_to_string(&legacy_path).await?;
        let tokens: GmailTokens = serde_json::from_str(&content)?;
        let payload = serde_json::to_string(&tokens)?;
        let _ = manager.set_custom_secret(GMAIL_SECRET_KEY, Some(payload));
        let _ = tokio::fs::remove_file(&legacy_path).await;
        return Ok(tokens);
    }

    Err(anyhow!("Gmail tokens not found"))
}

async fn save_tokens(config_dir: &Path, tokens: &GmailTokens) -> Result<()> {
    let manager = crate::core::config::SecureConfigManager::new(config_dir)?;
    let payload = serde_json::to_string(tokens)?;
    manager.set_custom_secret(GMAIL_SECRET_KEY, Some(payload))?;
    let legacy_path = token_path(config_dir);
    if legacy_path.exists() {
        let _ = tokio::fs::remove_file(&legacy_path).await;
    }
    Ok(())
}

pub(crate) async fn ensure_access_token(config_dir: &Path) -> Result<String> {
    let mut tokens = load_tokens(config_dir).await?;
    let now = chrono::Utc::now().timestamp();

    if tokens.expires_at > now + 60 {
        return Ok(tokens.access_token);
    }

    let (client_id, client_secret) = get_oauth_client_with_config(config_dir)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let params = [
        ("client_id", client_id.as_str()),
        ("client_secret", client_secret.as_str()),
        ("refresh_token", tokens.refresh_token.as_str()),
        ("grant_type", "refresh_token"),
    ];

    let resp = client.post(TOKEN_URL).form(&params).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("Failed to refresh token: {}", resp.status()));
    }

    let token_resp: TokenResponse = resp.json().await?;
    tokens.access_token = token_resp.access_token;
    tokens.expires_at = now + token_resp.expires_in;
    if let Some(refresh) = token_resp.refresh_token {
        tokens.refresh_token = refresh;
    }

    save_tokens(config_dir, &tokens).await?;
    Ok(tokens.access_token)
}

pub(crate) async fn gmail_profile_email(config_dir: &Path) -> Result<String> {
    let access_token = ensure_access_token(config_dir).await?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    let resp = client
        .get(format!("{}/users/me/profile", GMAIL_API_BASE))
        .bearer_auth(access_token)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(anyhow!("Gmail profile failed: {}", resp.status()));
    }
    #[derive(Debug, Deserialize)]
    struct ProfileResp {
        #[serde(default)]
        email_address: String,
    }
    let profile: ProfileResp = resp.json().await?;
    Ok(profile.email_address)
}

#[derive(Debug, Deserialize)]
pub struct GmailScanArgs {
    pub query: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub max_results: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct GmailReplyArgs {
    pub to: String,
    pub subject: String,
    pub body: String,
    pub thread_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GmailListResponse {
    messages: Option<Vec<GmailMessageRef>>,
}

#[derive(Debug, Deserialize)]
struct GmailMessageRef {
    id: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct GmailMessage {
    id: String,
    #[serde(default)]
    thread_id: String,
    #[serde(default)]
    payload: GmailPayload,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct GmailFullMessage {
    id: String,
    #[serde(default)]
    thread_id: String,
    #[serde(default, rename = "labelIds")]
    label_ids: Vec<String>,
    #[serde(default)]
    payload: GmailPayload,
}

#[derive(Debug, Deserialize, Default)]
struct GmailPayload {
    #[serde(default)]
    headers: Vec<GmailHeader>,
}

#[derive(Debug, Deserialize)]
struct GmailHeader {
    name: String,
    value: String,
}

fn header_value(headers: &[GmailHeader], name: &str) -> String {
    headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(name))
        .map(|h| h.value.clone())
        .unwrap_or_default()
}

pub async fn gmail_scan(config_dir: &Path, args: &serde_json::Value) -> Result<String> {
    let args: GmailScanArgs = serde_json::from_value(args.clone())
        .map_err(|e| anyhow!("Invalid Gmail scan args: {}", e))?;

    let access_token = ensure_access_token(config_dir).await?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()?;

    let labels = if args.labels.is_empty() {
        vec!["INBOX".to_string()]
    } else {
        args.labels
    };

    let mut url = reqwest::Url::parse(&format!("{}/users/me/messages", GMAIL_API_BASE))?;
    {
        let mut qp = url.query_pairs_mut();
        qp.append_pair("maxResults", &args.max_results.unwrap_or(10).to_string());
        for label in &labels {
            qp.append_pair("labelIds", label);
        }
        if let Some(q) = &args.query {
            qp.append_pair("q", q);
        }
    }

    let list_resp = client
        .get(url)
        .bearer_auth(&access_token)
        .send()
        .await?;
    if !list_resp.status().is_success() {
        return Err(anyhow!("Gmail list failed: {}", list_resp.status()));
    }

    let list: GmailListResponse = list_resp.json().await?;
    let Some(messages) = list.messages else {
        return Ok("No messages found.".to_string());
    };

    let mut summaries = Vec::new();
    for msg in messages {
        let msg_url = format!(
            "{}/users/me/messages/{}?format=metadata&metadataHeaders=Subject&metadataHeaders=From&metadataHeaders=Date",
            GMAIL_API_BASE, msg.id
        );
        let msg_resp = client.get(msg_url).bearer_auth(&access_token).send().await?;
        if !msg_resp.status().is_success() {
            continue;
        }
        let message: GmailFullMessage = msg_resp.json().await?;
        let subject = header_value(&message.payload.headers, "Subject");
        let from = header_value(&message.payload.headers, "From");
        let date = header_value(&message.payload.headers, "Date");
        let labels = message.label_ids.join(", ");
        summaries.push(format!(
            "- From: {}\n  Subject: {}\n  Date: {}\n  Labels: {}\n  Id: {}",
            from, subject, date, labels, message.id
        ));
    }

    Ok(summaries.join("\n\n"))
}

pub async fn gmail_reply(config_dir: &Path, args: &serde_json::Value) -> Result<String> {
    let args: GmailReplyArgs = serde_json::from_value(args.clone())
        .map_err(|e| anyhow!("Invalid Gmail reply args: {}", e))?;

    let access_token = ensure_access_token(config_dir).await?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()?;

    let raw = format!(
        "To: {}\r\nSubject: {}\r\nContent-Type: text/plain; charset=\"UTF-8\"\r\n\r\n{}",
        args.to, args.subject, args.body
    );

    let raw_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw.as_bytes());
    let mut body = serde_json::json!({
        "raw": raw_b64
    });
    if let Some(thread_id) = &args.thread_id {
        body["threadId"] = serde_json::Value::String(thread_id.clone());
    }

    let resp = client
        .post(format!("{}/users/me/messages/send", GMAIL_API_BASE))
        .bearer_auth(&access_token)
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(anyhow!("Gmail send failed: {}", resp.status()));
    }

    Ok("Reply sent successfully.".to_string())
}
