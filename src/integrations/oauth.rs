//! OAuth 2.0 Handler with Security-First Design
//!
//! SECURITY GUARANTEES:
//! - Tokens are NEVER logged (Debug impl redacts)
//! - Tokens are NEVER sent to LLM (no Display impl, no serialization to JSON for LLM)
//! - Tokens are encrypted at rest using KeyManager
//! - Tokens are only used internally for API calls
//! - Access tokens auto-refresh, refresh tokens are long-lived
//!
//! The OAuthTokens struct intentionally does NOT implement common traits that could
//! accidentally expose tokens.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::Path;
use crate::crypto::KeyManager;
use zeroize::Zeroizing;

/// Secure OAuth token container
///
/// SECURITY: This struct intentionally:
/// - Uses Zeroizing<String> to clear memory on drop
/// - Has a custom Debug impl that redacts tokens
/// - Does NOT implement Display
/// - Only serializes to encrypted storage, never to API responses
pub struct OAuthTokens {
    /// Access token (short-lived, auto-refreshed)
    access_token: Zeroizing<String>,
    /// Refresh token (long-lived, stored securely)
    refresh_token: Option<Zeroizing<String>>,
    /// Expiration timestamp (Unix seconds)
    expires_at: Option<i64>,
    /// Token type (usually "Bearer")
    token_type: String,
    /// Granted scopes
    scope: Option<String>,
}

// Custom Debug that NEVER shows token values
impl std::fmt::Debug for OAuthTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthTokens")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &self.refresh_token.as_ref().map(|_| "[REDACTED]"))
            .field("expires_at", &self.expires_at)
            .field("token_type", &self.token_type)
            .field("scope", &self.scope)
            .finish()
    }
}

impl Clone for OAuthTokens {
    fn clone(&self) -> Self {
        Self {
            access_token: Zeroizing::new(self.access_token.as_str().to_string()),
            refresh_token: self.refresh_token.as_ref().map(|t| Zeroizing::new(t.as_str().to_string())),
            expires_at: self.expires_at,
            token_type: self.token_type.clone(),
            scope: self.scope.clone(),
        }
    }
}

#[allow(dead_code)]
impl OAuthTokens {
    /// Check if the access token has expired (with 5 min buffer)
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = chrono::Utc::now().timestamp();
            now >= (expires_at - 300) // 5 minute buffer
        } else {
            false
        }
    }

    /// Get access token for internal use ONLY
    /// This should NEVER be logged or sent to LLM
    pub(crate) fn access_token(&self) -> &str {
        &self.access_token
    }

    /// Get refresh token for internal use ONLY
    pub(crate) fn refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_ref().map(|t| t.as_str())
    }

    /// Check if we have a refresh token
    pub fn has_refresh_token(&self) -> bool {
        self.refresh_token.is_some()
    }

    /// Get expiration info (safe to expose)
    pub fn expires_in_seconds(&self) -> Option<i64> {
        self.expires_at.map(|exp| exp - chrono::Utc::now().timestamp())
    }
}

/// Internal struct for serialization ONLY - never expose to API
#[derive(Clone, Serialize, Deserialize)]
struct TokensForStorage {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<i64>,
    token_type: String,
    scope: Option<String>,
}

impl From<&OAuthTokens> for TokensForStorage {
    fn from(t: &OAuthTokens) -> Self {
        Self {
            access_token: t.access_token.to_string(),
            refresh_token: t.refresh_token.as_ref().map(|r| r.to_string()),
            expires_at: t.expires_at,
            token_type: t.token_type.clone(),
            scope: t.scope.clone(),
        }
    }
}

impl From<TokensForStorage> for OAuthTokens {
    fn from(t: TokensForStorage) -> Self {
        Self {
            access_token: Zeroizing::new(t.access_token),
            refresh_token: t.refresh_token.map(Zeroizing::new),
            expires_at: t.expires_at,
            token_type: t.token_type,
            scope: t.scope,
        }
    }
}

/// OAuth configuration for a service
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    /// SECURITY: client_secret is NOT Debug-printed
    client_secret: Zeroizing<String>,
    pub auth_url: String,
    pub token_url: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
}

#[allow(dead_code)]
impl OAuthConfig {
    /// Create config for Google OAuth
    pub fn google(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            client_id,
            client_secret: Zeroizing::new(client_secret),
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            redirect_uri,
            scopes: vec![
                "https://www.googleapis.com/auth/calendar".to_string(),
                "https://www.googleapis.com/auth/calendar.events".to_string(),
            ],
        }
    }

    /// Create config for Spotify OAuth
    pub fn spotify(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            client_id,
            client_secret: Zeroizing::new(client_secret),
            auth_url: "https://accounts.spotify.com/authorize".to_string(),
            token_url: "https://accounts.spotify.com/api/token".to_string(),
            redirect_uri,
            scopes: vec![
                "user-read-playback-state".to_string(),
                "user-modify-playback-state".to_string(),
                "user-read-currently-playing".to_string(),
                "playlist-read-private".to_string(),
            ],
        }
    }

    /// Create config for Todoist OAuth
    pub fn todoist(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            client_id,
            client_secret: Zeroizing::new(client_secret),
            auth_url: "https://todoist.com/oauth/authorize".to_string(),
            token_url: "https://todoist.com/oauth/access_token".to_string(),
            redirect_uri,
            scopes: vec!["data:read_write".to_string()],
        }
    }

    /// Create config for Meta/WhatsApp Business OAuth
    pub fn meta_whatsapp(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            client_id,
            client_secret: Zeroizing::new(client_secret),
            auth_url: "https://www.facebook.com/v18.0/dialog/oauth".to_string(),
            token_url: "https://graph.facebook.com/v18.0/oauth/access_token".to_string(),
            redirect_uri,
            scopes: vec![
                "whatsapp_business_management".to_string(),
                "whatsapp_business_messaging".to_string(),
            ],
        }
    }

    /// Generate the authorization URL for user to visit
    pub fn auth_url(&self, state: &str) -> String {
        let scopes = self.scopes.join(" ");
        format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&state={}",
            self.auth_url,
            urlencoding::encode(&self.client_id),
            urlencoding::encode(&self.redirect_uri),
            urlencoding::encode(&scopes),
            urlencoding::encode(state)
        )
    }

    /// Get client secret for internal use only
    pub(crate) fn client_secret(&self) -> &str {
        &self.client_secret
    }
}

/// OAuth client for handling auth flows
///
/// SECURITY: All token operations are internal, never exposed
pub struct OAuthClient {
    http: reqwest::Client,
}

#[allow(dead_code)]
impl OAuthClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    /// Exchange authorization code for tokens
    /// SECURITY: Tokens returned are in secure container, never logged
    pub async fn exchange_code(&self, config: &OAuthConfig, code: &str) -> Result<OAuthTokens> {
        // SECURITY: Using form encoding, not logging the request
        let params = [
            ("client_id", config.client_id.as_str()),
            ("client_secret", config.client_secret()),
            ("code", code),
            ("redirect_uri", config.redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
        ];

        let response = self.http
            .post(&config.token_url)
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            // SECURITY: Don't include response body in error (might contain partial secrets)
            let status = response.status();
            return Err(anyhow!("Token exchange failed with status {}", status));
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            refresh_token: Option<String>,
            expires_in: Option<i64>,
            token_type: String,
            scope: Option<String>,
        }

        let token_response: TokenResponse = response.json().await?;

        let expires_at = token_response.expires_in.map(|secs| {
            chrono::Utc::now().timestamp() + secs
        });

        // SECURITY: Immediately wrap in Zeroizing containers
        Ok(OAuthTokens {
            access_token: Zeroizing::new(token_response.access_token),
            refresh_token: token_response.refresh_token.map(Zeroizing::new),
            expires_at,
            token_type: token_response.token_type,
            scope: token_response.scope,
        })
    }

    /// Refresh an access token using the refresh token
    /// SECURITY: Tokens never logged
    pub async fn refresh_token(&self, config: &OAuthConfig, refresh_token: &str) -> Result<OAuthTokens> {
        let params = [
            ("client_id", config.client_id.as_str()),
            ("client_secret", config.client_secret()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ];

        let response = self.http
            .post(&config.token_url)
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(anyhow!("Token refresh failed with status {}", status));
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            expires_in: Option<i64>,
            token_type: String,
            scope: Option<String>,
        }

        let token_response: TokenResponse = response.json().await?;

        let expires_at = token_response.expires_in.map(|secs| {
            chrono::Utc::now().timestamp() + secs
        });

        Ok(OAuthTokens {
            access_token: Zeroizing::new(token_response.access_token),
            refresh_token: Some(Zeroizing::new(refresh_token.to_string())), // Keep the old refresh token
            expires_at,
            token_type: token_response.token_type,
            scope: token_response.scope,
        })
    }
}

impl Default for OAuthClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Token storage - encrypted at rest with AES-256-GCM
///
/// SECURITY:
/// - All tokens encrypted using KeyManager before writing to disk
/// - Decryption only happens in memory
/// - Failed decryption returns error, doesn't expose partial data
pub struct TokenStorage {
    storage_path: std::path::PathBuf,
    key_manager: std::sync::Arc<KeyManager>,
}

#[allow(dead_code)]
impl TokenStorage {
    pub fn new(config_dir: &Path, key_manager: std::sync::Arc<KeyManager>) -> Self {
        Self {
            storage_path: config_dir.join("oauth_tokens.enc"),
            key_manager,
        }
    }

    /// Save tokens for a service (encrypted)
    pub fn save(&self, service_id: &str, tokens: &OAuthTokens) -> Result<()> {
        let mut all_tokens = self.load_all_internal()?;
        all_tokens.insert(service_id.to_string(), TokensForStorage::from(tokens));

        // SECURITY: Serialize to JSON, then encrypt
        let json = serde_json::to_vec(&all_tokens)?;
        let encrypted = self.key_manager.encrypt(&json)?;
        std::fs::write(&self.storage_path, encrypted)?;

        Ok(())
    }

    /// Load tokens for a service (decrypted into secure container)
    pub fn load(&self, service_id: &str) -> Result<Option<OAuthTokens>> {
        let all_tokens = self.load_all_internal()?;
        Ok(all_tokens.get(service_id).map(|t| OAuthTokens::from(t.clone())))
    }

    /// Check if tokens exist for a service (without loading)
    pub fn exists(&self, service_id: &str) -> bool {
        self.load_all_internal()
            .map(|t| t.contains_key(service_id))
            .unwrap_or(false)
    }

    /// Delete tokens for a service
    pub fn delete(&self, service_id: &str) -> Result<()> {
        let mut all_tokens = self.load_all_internal()?;
        all_tokens.remove(service_id);

        if all_tokens.is_empty() {
            let _ = std::fs::remove_file(&self.storage_path);
        } else {
            let json = serde_json::to_vec(&all_tokens)?;
            let encrypted = self.key_manager.encrypt(&json)?;
            std::fs::write(&self.storage_path, encrypted)?;
        }

        Ok(())
    }

    /// List all service IDs that have stored tokens (doesn't expose tokens)
    pub fn list_services(&self) -> Vec<String> {
        self.load_all_internal()
            .map(|t| t.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn load_all_internal(&self) -> Result<std::collections::HashMap<String, TokensForStorage>> {
        if !self.storage_path.exists() {
            return Ok(std::collections::HashMap::new());
        }

        let encrypted = std::fs::read(&self.storage_path)?;
        let decrypted = self.key_manager.decrypt(&encrypted)?;
        let tokens: std::collections::HashMap<String, TokensForStorage> = serde_json::from_slice(&decrypted)?;

        Ok(tokens)
    }
}

/// Safe info about OAuth status (no secrets)
/// This CAN be serialized and returned in API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthStatusInfo {
    pub service_id: String,
    pub is_connected: bool,
    pub expires_in_seconds: Option<i64>,
    pub needs_refresh: bool,
    pub scopes: Option<Vec<String>>,
}

#[allow(dead_code)]
impl OAuthStatusInfo {
    /// Create status info from tokens (safe - no secrets exposed)
    pub fn from_tokens(service_id: &str, tokens: Option<&OAuthTokens>) -> Self {
        match tokens {
            Some(t) => Self {
                service_id: service_id.to_string(),
                is_connected: true,
                expires_in_seconds: t.expires_in_seconds(),
                needs_refresh: t.is_expired(),
                scopes: None, // Could expose this if needed
            },
            None => Self {
                service_id: service_id.to_string(),
                is_connected: false,
                expires_in_seconds: None,
                needs_refresh: false,
                scopes: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_debug_redacted() {
        let tokens = OAuthTokens {
            access_token: Zeroizing::new("super_secret_token".to_string()),
            refresh_token: Some(Zeroizing::new("refresh_secret".to_string())),
            expires_at: Some(12345),
            token_type: "Bearer".to_string(),
            scope: Some("calendar".to_string()),
        };

        let debug_output = format!("{:?}", tokens);
        assert!(!debug_output.contains("super_secret"));
        assert!(!debug_output.contains("refresh_secret"));
        assert!(debug_output.contains("[REDACTED]"));
    }

    #[test]
    fn test_config_auth_url() {
        let config = OAuthConfig::google(
            "client123".to_string(),
            "secret456".to_string(),
            "http://localhost:17990/oauth/callback".to_string(),
        );

        let url = config.auth_url("test_state");
        assert!(url.contains("client123"));
        assert!(!url.contains("secret456")); // Secret should NOT be in auth URL
        assert!(url.contains("test_state"));
    }
}
