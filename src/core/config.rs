//! Agent configuration with encryption for sensitive data
//!
//! Non-sensitive config is stored in config.toml (readable)
//! Sensitive data (API keys, tokens) is stored encrypted in secrets.enc

use super::llm::LlmProvider;
use crate::crypto::KeyManager;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

/// Main agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    #[serde(default = "default_personality")]
    pub personality: String,
    /// Primary LLM provider
    pub llm: LlmProvider,
    /// Fallback LLM provider (used if primary fails)
    #[serde(default)]
    pub llm_fallback: Option<LlmProvider>,
    #[serde(default)]
    pub telegram: Option<TelegramConfig>,
    #[serde(default)]
    pub sandbox: SandboxConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub auto_approve: Vec<String>,
    /// Media generation settings
    #[serde(default)]
    pub media_gen: MediaGenConfig,
}

fn default_personality() -> String {
    "friendly".to_string()
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            name: "CogniArk".to_string(),
            personality: default_personality(),
            llm: LlmProvider::default(),
            llm_fallback: None,
            telegram: None,
            sandbox: SandboxConfig::default(),
            memory: MemoryConfig::default(),
            auto_approve: vec![],
            media_gen: MediaGenConfig::default(),
        }
    }
}

/// Media generation configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MediaGenConfig {
    /// Default provider for image generation
    #[serde(default)]
    pub default_image_provider: Option<String>,
    /// Fallback provider for image generation
    #[serde(default)]
    pub fallback_image_provider: Option<String>,
    /// Default provider for video generation
    #[serde(default)]
    pub default_video_provider: Option<String>,
    /// Fallback provider for video generation
    #[serde(default)]
    pub fallback_video_provider: Option<String>,
    /// API keys for media providers (stored encrypted in secrets.enc)
    /// Keys: replicate, stability_ai, fal, together, openai, google, runway, luma
    #[serde(default)]
    pub provider_api_keys: std::collections::HashMap<String, String>,
}

/// Encrypted secrets storage
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Secrets {
    /// Primary LLM API key
    pub llm_api_key: Option<String>,
    /// Fallback LLM API key
    pub llm_fallback_api_key: Option<String>,
    /// Telegram bot token
    pub telegram_bot_token: Option<String>,
    /// Media generation provider API keys (encrypted)
    #[serde(default)]
    pub media_provider_keys: std::collections::HashMap<String, String>,
    /// Custom secrets (for future extensibility)
    #[serde(default)]
    pub custom: std::collections::HashMap<String, String>,
}

/// Secure configuration manager
/// Handles encryption/decryption of sensitive data
pub struct SecureConfigManager {
    key_manager: Arc<KeyManager>,
    config_dir: std::path::PathBuf,
}

impl SecureConfigManager {
    /// Create a new secure config manager
    pub fn new(config_dir: &Path) -> Result<Self> {
        let keyfile = config_dir.join(".keyfile");
        let key_manager = Arc::new(KeyManager::load_or_create(&keyfile)?);

        Ok(Self {
            key_manager,
            config_dir: config_dir.to_path_buf(),
        })
    }

    /// Create with a custom key manager (for testing or custom key derivation)
    #[allow(dead_code)]
    pub fn with_key_manager(config_dir: &Path, key_manager: Arc<KeyManager>) -> Self {
        Self {
            key_manager,
            config_dir: config_dir.to_path_buf(),
        }
    }

    /// Load configuration with decrypted secrets
    pub fn load(&self) -> Result<AgentConfig> {
        let config_path = self.config_dir.join("config.toml");
        let secrets_path = self.config_dir.join("secrets.enc");

        // Load base config
        let mut config = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            toml::from_str(&content)?
        } else {
            let config = AgentConfig::default();
            self.save_config_only(&config)?;
            config
        };

        // Load and decrypt secrets
        if secrets_path.exists() {
            let secrets = self.load_secrets()?;
            // Inject secrets into config
            self.inject_secrets(&mut config, &secrets);
        } else {
            // Migrate from old plain config if secrets exist there
            self.migrate_from_plain_config(&mut config)?;
        }

        Ok(config)
    }

    /// Save configuration with encrypted secrets
    pub fn save(&self, config: &AgentConfig) -> Result<()> {
        // Extract secrets from config
        let secrets = self.extract_secrets(config);

        // Create sanitized config (without secrets)
        let sanitized = self.sanitize_config(config);

        // Save non-sensitive config as TOML
        self.save_config_only(&sanitized)?;

        // Encrypt and save secrets
        self.save_secrets(&secrets)?;

        Ok(())
    }

    /// Save only the config.toml (without encryption)
    fn save_config_only(&self, config: &AgentConfig) -> Result<()> {
        let config_path = self.config_dir.join("config.toml");
        let content = toml::to_string_pretty(config)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }

    /// Save encrypted secrets
    pub(crate) fn save_secrets(&self, secrets: &Secrets) -> Result<()> {
        let secrets_path = self.config_dir.join("secrets.enc");
        let json = serde_json::to_vec(secrets)?;
        let encrypted = self.key_manager.encrypt(&json)?;
        std::fs::write(secrets_path, encrypted)?;
        Ok(())
    }

    fn load_secrets(&self) -> Result<Secrets> {
        let secrets_path = self.config_dir.join("secrets.enc");
        if !secrets_path.exists() {
            return Ok(Secrets::default());
        }
        let encrypted_data = std::fs::read(&secrets_path)?;
        let decrypted = self.key_manager.decrypt(&encrypted_data)?;
        let secrets: Secrets = serde_json::from_slice(&decrypted)?;
        Ok(secrets)
    }

    /// Read a custom secret string by key
    pub fn get_custom_secret(&self, key: &str) -> Result<Option<String>> {
        let secrets = self.load_secrets()?;
        Ok(secrets.custom.get(key).cloned())
    }

    /// Write or remove a custom secret string by key
    pub fn set_custom_secret(&self, key: &str, value: Option<String>) -> Result<()> {
        let mut secrets = self.load_secrets()?;
        match value {
            Some(v) => {
                secrets.custom.insert(key.to_string(), v);
            }
            None => {
                secrets.custom.remove(key);
            }
        }
        self.save_secrets(&secrets)
    }

    /// Extract secrets from config
    fn extract_secrets(&self, config: &AgentConfig) -> Secrets {
        let mut secrets = Secrets::default();

        // Extract primary LLM API key
        match &config.llm {
            LlmProvider::Anthropic { api_key, .. } if !api_key.is_empty() && api_key != "[ENCRYPTED]" => {
                secrets.llm_api_key = Some(api_key.clone());
            }
            LlmProvider::OpenAI { api_key, .. } if !api_key.is_empty() && api_key != "[ENCRYPTED]" => {
                secrets.llm_api_key = Some(api_key.clone());
            }
            _ => {}
        }

        // Extract fallback LLM API key
        if let Some(fallback) = &config.llm_fallback {
            match fallback {
                LlmProvider::Anthropic { api_key, .. } if !api_key.is_empty() && api_key != "[ENCRYPTED]" => {
                    secrets.llm_fallback_api_key = Some(api_key.clone());
                }
                LlmProvider::OpenAI { api_key, .. } if !api_key.is_empty() && api_key != "[ENCRYPTED]" => {
                    secrets.llm_fallback_api_key = Some(api_key.clone());
                }
                _ => {}
            }
        }

        // Extract Telegram token
        if let Some(tg) = &config.telegram {
            if !tg.bot_token.is_empty() && tg.bot_token != "[ENCRYPTED]" {
                secrets.telegram_bot_token = Some(tg.bot_token.clone());
            }
        }

        // Extract media provider API keys
        for (provider, key) in &config.media_gen.provider_api_keys {
            if !key.is_empty() && key != "[ENCRYPTED]" {
                secrets.media_provider_keys.insert(provider.clone(), key.clone());
            }
        }

        secrets
    }

    /// Create sanitized config with placeholder secrets
    fn sanitize_config(&self, config: &AgentConfig) -> AgentConfig {
        let mut sanitized = config.clone();

        // Replace primary API key with placeholder
        match &mut sanitized.llm {
            LlmProvider::Anthropic { api_key, .. } => {
                if !api_key.is_empty() {
                    *api_key = "[ENCRYPTED]".to_string();
                }
            }
            LlmProvider::OpenAI { api_key, .. } => {
                if !api_key.is_empty() {
                    *api_key = "[ENCRYPTED]".to_string();
                }
            }
            _ => {}
        }

        // Replace fallback API key with placeholder
        if let Some(fallback) = &mut sanitized.llm_fallback {
            match fallback {
                LlmProvider::Anthropic { api_key, .. } => {
                    if !api_key.is_empty() {
                        *api_key = "[ENCRYPTED]".to_string();
                    }
                }
                LlmProvider::OpenAI { api_key, .. } => {
                    if !api_key.is_empty() {
                        *api_key = "[ENCRYPTED]".to_string();
                    }
                }
                _ => {}
            }
        }

        // Replace Telegram token with placeholder
        if let Some(tg) = &mut sanitized.telegram {
            if !tg.bot_token.is_empty() {
                tg.bot_token = "[ENCRYPTED]".to_string();
            }
        }

        // Replace media provider API keys with placeholder
        for (_, key) in sanitized.media_gen.provider_api_keys.iter_mut() {
            if !key.is_empty() {
                *key = "[ENCRYPTED]".to_string();
            }
        }

        sanitized
    }

    /// Inject decrypted secrets into config
    fn inject_secrets(&self, config: &mut AgentConfig, secrets: &Secrets) {
        // Inject primary LLM API key
        if let Some(api_key) = &secrets.llm_api_key {
            match &mut config.llm {
                LlmProvider::Anthropic { api_key: key, .. } => {
                    *key = api_key.clone();
                }
                LlmProvider::OpenAI { api_key: key, .. } => {
                    *key = api_key.clone();
                }
                _ => {}
            }
        }

        // Inject fallback LLM API key
        if let Some(api_key) = &secrets.llm_fallback_api_key {
            if let Some(fallback) = &mut config.llm_fallback {
                match fallback {
                    LlmProvider::Anthropic { api_key: key, .. } => {
                        *key = api_key.clone();
                    }
                    LlmProvider::OpenAI { api_key: key, .. } => {
                        *key = api_key.clone();
                    }
                    _ => {}
                }
            }
        }

        // Inject Telegram token
        if let Some(token) = &secrets.telegram_bot_token {
            if let Some(tg) = &mut config.telegram {
                tg.bot_token = token.clone();
            }
        }

        // Inject media provider API keys
        for (provider, key) in &secrets.media_provider_keys {
            config.media_gen.provider_api_keys.insert(provider.clone(), key.clone());
        }
    }

    /// Migrate secrets from old plain config to encrypted storage
    fn migrate_from_plain_config(&self, config: &mut AgentConfig) -> Result<()> {
        let secrets = self.extract_secrets(config);

        // Only migrate if there are actual secrets
        let has_secrets = secrets.llm_api_key.as_ref().map(|k| !k.is_empty() && k != "[ENCRYPTED]").unwrap_or(false)
            || secrets.llm_fallback_api_key.as_ref().map(|k| !k.is_empty() && k != "[ENCRYPTED]").unwrap_or(false)
            || secrets.telegram_bot_token.as_ref().map(|t| !t.is_empty() && t != "[ENCRYPTED]").unwrap_or(false)
            || !secrets.media_provider_keys.is_empty();

        if has_secrets {
            tracing::info!("Migrating secrets to encrypted storage...");
            self.save_secrets(&secrets)?;

            // Save sanitized config
            let sanitized = self.sanitize_config(config);
            self.save_config_only(&sanitized)?;

            tracing::info!("Secrets migration complete");
        }

        Ok(())
    }

    /// Get the key manager (for encrypting other data)
    #[allow(dead_code)]
    pub fn key_manager(&self) -> Arc<KeyManager> {
        self.key_manager.clone()
    }
}

impl AgentConfig {
    /// Load config (legacy method - use SecureConfigManager for encrypted secrets)
    pub fn load(config_dir: &Path) -> Result<Self> {
        let secrets_path = config_dir.join("secrets.enc");

        // If encrypted secrets exist, we MUST be able to decrypt them
        // Falling back to plain config would lose the API keys
        if secrets_path.exists() {
            let manager = SecureConfigManager::new(config_dir)
                .map_err(|e| anyhow!("Encryption initialization failed and secrets.enc exists - cannot decrypt secrets: {}", e))?;
            return manager.load();
        }

        // No encrypted secrets yet - try secure manager, fall back to plain for first run
        match SecureConfigManager::new(config_dir) {
            Ok(manager) => manager.load(),
            Err(e) => {
                tracing::warn!("Failed to initialize encryption (first run?), loading plain config: {}", e);
                Self::load_plain(config_dir)
            }
        }
    }

    /// Load plain config without encryption (fallback)
    fn load_plain(config_dir: &Path) -> Result<Self> {
        let config_path = config_dir.join("config.toml");

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: AgentConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            let config = AgentConfig::default();
            config.save_plain(config_dir)?;
            Ok(config)
        }
    }

    /// Save config (legacy method - use SecureConfigManager for encrypted secrets)
    pub fn save(&self, config_dir: &Path) -> Result<()> {
        // Always use encryption for saving - don't silently fall back to plain
        let manager = SecureConfigManager::new(config_dir)
            .map_err(|e| anyhow!("Failed to initialize encryption for saving config: {}", e))?;
        manager.save(self)
    }

    /// Save plain config without encryption (fallback)
    fn save_plain(&self, config_dir: &Path) -> Result<()> {
        let config_path = config_dir.join("config.toml");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    #[serde(default)]
    pub bot_token: String,
    #[serde(default)]
    pub allowed_users: Vec<i64>,
    #[serde(default = "default_dm_policy")]
    pub dm_policy: String,
}

fn default_dm_policy() -> String {
    "pairing".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    #[serde(default = "default_sandbox_mode")]
    pub default_mode: String,
    #[serde(default = "default_docker_image")]
    pub docker_image: String,
    #[serde(default = "default_true")]
    pub enable_rollback: bool,
    pub snapshot_dir: Option<String>,
}

fn default_sandbox_mode() -> String {
    "wasm".to_string()
}

fn default_docker_image() -> String {
    "cogniark-sandbox:latest".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            default_mode: default_sandbox_mode(),
            docker_image: default_docker_image(),
            enable_rollback: true,
            snapshot_dir: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_max_episodes")]
    pub max_episodes: usize,
    #[serde(default = "default_consolidation_interval")]
    pub consolidation_interval_hours: u64,
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
}

fn default_max_episodes() -> usize {
    10000
}

fn default_consolidation_interval() -> u64 {
    24
}

fn default_embedding_model() -> String {
    "BAAI/bge-small-en-v1.5".to_string()
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_episodes: default_max_episodes(),
            consolidation_interval_hours: default_consolidation_interval(),
            embedding_model: default_embedding_model(),
        }
    }
}
