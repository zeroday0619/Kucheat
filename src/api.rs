use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::config::ApiConfig;

const OFFICIAL_BASE_URL: &str = "https://openapi.chzzk.naver.com";
const UNOFFICIAL_BASE_URL: &str = "https://api.chzzk.naver.com";

const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

// ---------------------------------------------------------------------------
// API response wrappers
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiResponse<T> {
    code: i32,
    message: Option<String>,
    content: Option<T>,
}

/// Official API wraps channel list in `{ data: [...] }`.
#[derive(Debug, Deserialize)]
struct PagedData<T> {
    data: Vec<T>,
}

// ---------------------------------------------------------------------------
// Channel info (unofficial API — includes openLive)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelInfo {
    #[allow(dead_code)]
    pub channel_id: String,
    pub channel_name: String,
    #[allow(dead_code)]
    pub channel_image_url: Option<String>,
    #[serde(default)]
    pub open_live: bool,
}

// ---------------------------------------------------------------------------
// Official API channel info (GET /open/v1/channels?channelIds=…)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OfficialChannelInfo {
    channel_id: String,
    channel_name: String,
    channel_image_url: Option<String>,
}

// ---------------------------------------------------------------------------
// Live detail (unofficial API)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveDetail {
    pub live_title: Option<String>,
    #[serde(default)]
    pub concurrent_user_count: i64,
    pub live_category: Option<String>,
    pub live_category_value: Option<String>,
}

// ---------------------------------------------------------------------------
// Aggregated live status (our abstraction)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LiveStatus {
    pub is_live: bool,
    pub channel_name: String,
    pub live_title: Option<String>,
    pub category: Option<String>,
    pub viewer_count: Option<i64>,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

pub struct ChzzkClient {
    client: Client,
    use_official: bool,
    client_id: String,
    client_secret: String,
}

impl ChzzkClient {
    pub fn new(config: &ApiConfig) -> Result<Self> {
        let use_official =
            !config.client_id.is_empty() && !config.client_secret.is_empty();

        if use_official {
            tracing::info!("Official CHZZK credentials configured");
        } else {
            tracing::info!("No client credentials — using unofficial API only");
        }

        let client = Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            use_official,
            client_id: config.client_id.clone(),
            client_secret: config.client_secret.clone(),
        })
    }

    // -----------------------------------------------------------------------
    // Generic fetch helper — eliminates repeated HTTP+parse+error boilerplate
    // -----------------------------------------------------------------------

    async fn fetch_json<T: DeserializeOwned>(
        &self,
        url: &str,
        auth: bool,
    ) -> Result<T> {
        let mut req = self.client.get(url);
        if auth {
            req = req
                .header("Client-Id", &self.client_id)
                .header("Client-Secret", &self.client_secret);
        }

        let body = req
            .send()
            .await
            .context("HTTP request failed")?
            .text()
            .await
            .context("Failed to read response body")?;

        let api: ApiResponse<T> =
            serde_json::from_str(&body).with_context(|| {
                format!(
                    "Failed to parse API response. Body: {}",
                    &body[..body.len().min(500)]
                )
            })?;

        if api.code != 200 {
            bail!(
                "API error ({}): {}",
                api.code,
                api.message.unwrap_or_default()
            );
        }

        api.content.context("Empty content in API response")
    }

    // -----------------------------------------------------------------------
    // Channel info
    // -----------------------------------------------------------------------

    /// Get basic channel info. Tries official API first (if configured),
    /// then falls back to unofficial.
    pub async fn get_channel_info(
        &self,
        channel_id: &str,
    ) -> Result<ChannelInfo> {
        if self.use_official {
            match self.get_channel_info_official(channel_id).await {
                Ok(info) => return Ok(info),
                Err(e) => {
                    tracing::warn!(
                        "Official channel info failed, falling back: {e}"
                    );
                }
            }
        }
        self.get_channel_info_unofficial(channel_id).await
    }

    /// Official: `GET /open/v1/channels?channelIds=<id>`
    async fn get_channel_info_official(
        &self,
        channel_id: &str,
    ) -> Result<ChannelInfo> {
        let url = format!(
            "{OFFICIAL_BASE_URL}/open/v1/channels?channelIds={channel_id}"
        );

        let paged: PagedData<OfficialChannelInfo> =
            self.fetch_json(&url, true).await?;

        let ch = paged
            .data
            .into_iter()
            .next()
            .context("No channel found in official API response")?;

        Ok(ChannelInfo {
            channel_id: ch.channel_id,
            channel_name: ch.channel_name,
            channel_image_url: ch.channel_image_url,
            open_live: false, // official API does not provide this
        })
    }

    /// Unofficial: `GET /service/v1/channels/<id>`
    async fn get_channel_info_unofficial(
        &self,
        channel_id: &str,
    ) -> Result<ChannelInfo> {
        let url =
            format!("{UNOFFICIAL_BASE_URL}/service/v1/channels/{channel_id}");
        self.fetch_json(&url, false).await
    }

    // -----------------------------------------------------------------------
    // Live detail (always unofficial)
    // -----------------------------------------------------------------------

    async fn get_live_detail(
        &self,
        channel_id: &str,
    ) -> Result<LiveDetail> {
        let url = format!(
            "{UNOFFICIAL_BASE_URL}/service/v3/channels/{channel_id}/live-detail"
        );
        self.fetch_json(&url, false).await
    }

    // -----------------------------------------------------------------------
    // High-level live check
    // -----------------------------------------------------------------------

    /// Check whether a channel is currently live and gather details.
    /// Always uses the **unofficial** API because only it has `openLive`.
    pub async fn check_channel_live(
        &self,
        channel_id: &str,
    ) -> Result<LiveStatus> {
        let ch = self.get_channel_info_unofficial(channel_id).await?;

        let mut status = LiveStatus {
            is_live: ch.open_live,
            channel_name: ch.channel_name,
            live_title: None,
            category: None,
            viewer_count: None,
        };

        if ch.open_live {
            match self.get_live_detail(channel_id).await {
                Ok(detail) => {
                    status.live_title = detail.live_title;
                    status.category = detail
                        .live_category_value
                        .or(detail.live_category);
                    status.viewer_count =
                        Some(detail.concurrent_user_count);
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to get live detail for {channel_id}: {e}"
                    );
                }
            }
        }

        Ok(status)
    }
}
