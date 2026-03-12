use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub anthropic_api_key: Option<String>,
    #[serde(default = "default_model")]
    pub model: String,
}

fn default_model() -> String {
    "claude-sonnet-4-20250514".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            anthropic_api_key: None,
            model: default_model(),
        }
    }
}

pub struct ConfigManager {
    path: PathBuf,
}

impl ConfigManager {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join("config.json"),
        }
    }

    pub fn load(&self) -> AppConfig {
        if self.path.exists() {
            let data = fs::read_to_string(&self.path).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            AppConfig::default()
        }
    }

    pub fn save(&self, config: &AppConfig) -> Result<(), String> {
        let json = serde_json::to_string_pretty(config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        fs::write(&self.path, json)
            .map_err(|e| format!("Failed to write config: {}", e))?;
        Ok(())
    }
}
