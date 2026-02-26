use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::api::LiveStatus;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AppState {
    pub channels: HashMap<String, ChannelState>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChannelState {
    pub is_live: bool,
    pub channel_name: String,
    pub live_title: Option<String>,
    pub category: Option<String>,
    pub viewer_count: Option<i64>,
    pub last_checked: DateTime<Utc>,
    pub went_live_at: Option<DateTime<Utc>>,
}

impl ChannelState {
    /// Build a `ChannelState` from a `LiveStatus`, preserving `went_live_at`
    /// from the previous state when applicable.
    pub fn from_live_status(
        status: &LiveStatus,
        previous: Option<&ChannelState>,
    ) -> Self {
        let now = Utc::now();
        let was_live = previous.is_some_and(|s| s.is_live);

        Self {
            is_live: status.is_live,
            channel_name: status.channel_name.clone(),
            live_title: status.live_title.clone(),
            category: status.category.clone(),
            viewer_count: status.viewer_count,
            last_checked: now,
            went_live_at: if status.is_live && !was_live {
                Some(now)
            } else {
                previous.and_then(|s| s.went_live_at)
            },
        }
    }
}

impl AppState {
    fn state_dir() -> Result<PathBuf> {
        let dir = dirs::state_dir()
            .or_else(dirs::data_local_dir)
            .context("Failed to find state directory")?
            .join("kucheat");
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    fn state_path() -> Result<PathBuf> {
        Ok(Self::state_dir()?.join("state.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::state_path()?;
        tracing::debug!(path = ?path, "Loading app state");
        if path.exists() {
            let content =
                fs::read_to_string(&path).context("Failed to read state file")?;
            let state: Self = serde_json::from_str(&content).context("Failed to parse state file")?;
            tracing::debug!(channels = state.channels.len(), "App state loaded");
            Ok(state)
        } else {
            tracing::debug!("No state file found, using default");
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::state_path()?;
        tracing::debug!(path = ?path, channels = self.channels.len(), "Saving app state");
        let content =
            serde_json::to_string_pretty(self).context("Failed to serialize state")?;
        fs::write(&path, content).context("Failed to write state file")?;
        tracing::debug!("App state saved successfully");
        Ok(())
    }
}
