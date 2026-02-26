use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub bot_token: String,
    #[serde(default)]
    pub chat_id: String,
    #[serde(default = "default_timeout")]
    pub permission_timeout: u64,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default = "default_port")]
    pub daemon_port: u16,
}

fn default_timeout() -> u64 {
    300
}

fn default_port() -> u16 {
    19876
}

impl Config {
    pub fn config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".claude")
            .join("hooks")
            .join("telegram_config.json")
    }

    /// Load config from file with env var fallback.
    /// Returns None if credentials are missing.
    pub fn load() -> Option<Self> {
        let path = Self::config_path();
        let mut config: Config = match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Config::default(),
        };

        // Env var fallback
        if config.bot_token.is_empty() {
            if let Ok(token) = std::env::var("TELEGRAM_BOT_TOKEN") {
                config.bot_token = token;
            }
        }
        if config.chat_id.is_empty() {
            if let Ok(chat_id) = std::env::var("TELEGRAM_CHAT_ID") {
                config.chat_id = chat_id;
            }
        }

        if config.bot_token.is_empty() || config.chat_id.is_empty() {
            return None;
        }

        Some(config)
    }

    /// Save config to the standard path.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write config: {}", path.display()))?;
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bot_token: String::new(),
            chat_id: String::new(),
            permission_timeout: default_timeout(),
            disabled: false,
            daemon_port: default_port(),
        }
    }
}
