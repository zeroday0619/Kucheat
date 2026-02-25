use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub api: ApiConfig,
    pub settings: Settings,
    #[serde(default)]
    pub channels: Vec<ChannelConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiConfig {
    /// CHZZK Open API Client ID (leave empty to use unofficial API)
    #[serde(default)]
    pub client_id: String,
    /// CHZZK Open API Client Secret
    #[serde(default)]
    pub client_secret: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
    /// Interval between live checks in seconds
    #[serde(default = "default_check_interval")]
    pub check_interval_secs: u64,
    /// Whether to send notification when a stream goes offline
    #[serde(default)]
    pub notify_offline: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChannelConfig {
    pub id: String,
    pub name: String,
}

fn default_check_interval() -> u64 {
    60
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api: ApiConfig {
                client_id: String::new(),
                client_secret: String::new(),
            },
            settings: Settings {
                check_interval_secs: 60,
                notify_offline: false,
            },
            channels: Vec::new(),
        }
    }
}

impl Config {
    pub fn config_dir() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("Failed to find config directory")?
            .join("kucheat");
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let content =
                fs::read_to_string(&path).context("Failed to read config file")?;
            toml::from_str(&content).context("Failed to parse config file")
        } else {
            let config = Self::default();
            config.save()?;
            tracing::info!("Created default config at {:?}", path);
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        let content =
            toml::to_string_pretty(self).context("Failed to serialize config")?;
        fs::write(&path, content).context("Failed to write config file")?;
        Ok(())
    }

    pub fn add_channel(&mut self, id: &str, name: &str) {
        self.channels.retain(|ch| ch.id != id);
        self.channels.push(ChannelConfig {
            id: id.to_string(),
            name: name.to_string(),
        });
    }

    pub fn remove_channel(&mut self, id: &str) -> bool {
        let before = self.channels.len();
        self.channels.retain(|ch| ch.id != id);
        self.channels.len() < before
    }
}
