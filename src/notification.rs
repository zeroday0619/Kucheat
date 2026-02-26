use std::path::PathBuf;
use std::sync::LazyLock;

use anyhow::Result;
use notify_rust::{Notification, Urgency};

use crate::api::LiveStatus;

static ICON_PNG: &[u8] = include_bytes!("../assets/icons/kucheat.png");

/// Resolve the notification icon path.
/// Prefers the XDG-installed icon; falls back to writing the embedded PNG
/// to a temp file so that the notification daemon can read it.
static NOTIFICATION_ICON: LazyLock<String> = LazyLock::new(|| {
    // 1) Check the standard XDG icon location
    if let Some(data_dir) = dirs::data_dir() {
        let installed = data_dir.join("icons/hicolor/512x512/apps/kucheat.png");
        if installed.exists() {
            return installed.to_string_lossy().into_owned();
        }
    }

    // 2) Fallback: write embedded PNG to a temp file
    let tmp_path: PathBuf = std::env::temp_dir().join("kucheat-notification-icon.png");
    if std::fs::write(&tmp_path, ICON_PNG).is_ok() {
        return tmp_path.to_string_lossy().into_owned();
    }

    // 3) Last resort
    "video-display".to_string()
});

/// Send a desktop notification when a channel goes live.
/// The notification is shown immediately and control returns to the caller.
/// Action button handling (opening URLs) runs in a background task so it
/// does not block the daemon loop.
pub async fn send_live_notification(channel_id: &str, status: &LiveStatus) -> Result<()> {
    tracing::debug!(
        channel_id = channel_id,
        channel_name = %status.channel_name,
        title = ?status.live_title,
        category = ?status.category,
        viewers = ?status.viewer_count,
        "Sending live notification"
    );
    let summary = format!("🔴 {} 방송 시작!", status.channel_name);

    let mut body_parts: Vec<String> = Vec::new();
    if let Some(ref title) = status.live_title {
        body_parts.push(title.clone());
    }
    if let Some(ref category) = status.category {
        body_parts.push(format!("카테고리: {}", category));
    }
    if let Some(count) = status.viewer_count {
        body_parts.push(format!("시청자: {}명", count));
    }

    let body = body_parts.join("\n");
    let live_url = format!("https://chzzk.naver.com/live/{}", channel_id);

    // Show the notification (quick) and get the handle.
    let handle = tokio::task::spawn_blocking(move || {
        Notification::new()
            .summary(&summary)
            .body(&body)
            .icon(&NOTIFICATION_ICON)
            .urgency(Urgency::Normal)
            .timeout(10_000)
            .action("live", "라이브 보기")
            .action("channel", "채널 페이지")
            .show()
    })
    .await??;

    // Process action clicks in a detached background task so
    // the daemon is never blocked waiting for user interaction.
    tokio::task::spawn_blocking(move || {
        handle.wait_for_action(|action| match action {
            "live" => {
                let _ = open::that(&live_url);
            }
            "channel" => {
                let channel_url = live_url.replace("/live/", "/");
                let _ = open::that(channel_url);
            }
            _ => {}
        });
    });

    Ok(())
}

/// Send a desktop notification when a channel goes offline.
pub async fn send_offline_notification(channel_name: &str) -> Result<()> {
    tracing::debug!(channel_name = channel_name, "Sending offline notification");
    let name = channel_name.to_string();

    tokio::task::spawn_blocking(move || {
        Notification::new()
            .summary(&format!("⚫ {} 방송 종료", name))
            .icon(&NOTIFICATION_ICON)
            .urgency(Urgency::Low)
            .timeout(5_000)
            .show()
    })
    .await??;

    Ok(())
}
