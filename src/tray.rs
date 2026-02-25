use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use anyhow::Result;
use ksni::TrayMethods;
use tokio::sync::watch;

use crate::config::{ChannelConfig, Config};
use crate::state::{AppState, ChannelState};

/// Source of truth for tray state updates.
pub enum StateSource {
    /// Watch channel driven by the daemon (instant updates).
    Watch(watch::Receiver<AppState>),
    /// Read from disk (standalone tray mode).
    File,
}

// ---------------------------------------------------------------------------
// Tray data — holds everything the menu needs to render
// ---------------------------------------------------------------------------

struct TrayData {
    channels: Vec<ChannelConfig>,
    state: AppState,
}

// ---------------------------------------------------------------------------
// Embedded icon from assets/icons/kucheat.png
// ---------------------------------------------------------------------------

static ICON_PNG: &[u8] = include_bytes!("../assets/icons/kucheat.png");

/// Decode the embedded PNG once and cache the ARGB pixmap.
fn make_icon_pixmap() -> Vec<ksni::Icon> {
    use std::sync::LazyLock;

    static PIXMAP: LazyLock<Vec<ksni::Icon>> = LazyLock::new(|| {
        let img =
            image::load_from_memory_with_format(ICON_PNG, image::ImageFormat::Png)
                .expect("embedded icon.png is invalid")
                .into_rgba8();

        let width = img.width() as i32;
        let height = img.height() as i32;

        let mut argb = Vec::with_capacity((width * height * 4) as usize);
        for pixel in img.pixels() {
            let [r, g, b, a] = pixel.0;
            argb.extend_from_slice(&[a, r, g, b]);
        }

        vec![ksni::Icon { width, height, data: argb }]
    });

    PIXMAP.clone()
}

// ---------------------------------------------------------------------------
// Menu builder helpers — reduce ksni boilerplate
// ---------------------------------------------------------------------------

fn label_item<T: ksni::Tray>(text: String) -> ksni::MenuItem<T> {
    ksni::MenuItem::Standard(ksni::menu::StandardItem {
        label: text,
        enabled: false,
        ..Default::default()
    })
}

fn link_item<T: ksni::Tray>(label: String, url: String) -> ksni::MenuItem<T> {
    ksni::MenuItem::Standard(ksni::menu::StandardItem {
        label,
        activate: Box::new(move |_| {
            let _ = open::that(&url);
        }),
        ..Default::default()
    })
}

fn chzzk_url(path: &str, channel_id: &str) -> String {
    format!("https://chzzk.naver.com/{path}/{channel_id}")
}

// ---------------------------------------------------------------------------
// ksni Tray implementation (0.3 API)
// ---------------------------------------------------------------------------

struct KucheatTray {
    data: Arc<StdMutex<TrayData>>,
}

impl ksni::Tray for KucheatTray {
    fn id(&self) -> String {
        "kucheat".to_string()
    }

    fn title(&self) -> String {
        "Kucheat – CHZZK 알림".to_string()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::Communications
    }

    fn status(&self) -> ksni::Status {
        ksni::Status::Active
    }

    fn icon_name(&self) -> String {
        String::new()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        make_icon_pixmap()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let data = self.data.lock().unwrap();
        let live_count = data
            .channels
            .iter()
            .filter(|ch| {
                data.state
                    .channels
                    .get(&ch.id)
                    .is_some_and(|s| s.is_live)
            })
            .count();

        let description = if live_count > 0 {
            format!("{live_count}개 채널 라이브 중")
        } else {
            "라이브 없음".to_string()
        };

        ksni::ToolTip {
            icon_name: String::new(),
            icon_pixmap: Vec::new(),
            title: "Kucheat".to_string(),
            description,
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        let data = self.data.lock().unwrap();
        let mut items: Vec<ksni::MenuItem<Self>> = Vec::new();

        if data.channels.is_empty() {
            items.push(label_item("채널 없음 (kucheat add <id>)".into()));
        } else {
            let (live, offline): (Vec<_>, Vec<_>) =
                data.channels.iter().partition(|ch| {
                    data.state
                        .channels
                        .get(&ch.id)
                        .is_some_and(|s| s.is_live)
                });

            // ── Live channels ──────────────────────────────────────
            if !live.is_empty() {
                items.push(label_item(format!("━━ 라이브 ({}) ━━", live.len())));

                for ch in &live {
                    let cs = data.state.channels.get(&ch.id);
                    items.push(self.build_live_submenu(ch, cs));
                }
            }

            // ── Offline channels ───────────────────────────────────
            if !offline.is_empty() {
                items.push(label_item(format!(
                    "━━ 오프라인 ({}) ━━",
                    offline.len()
                )));

                for ch in &offline {
                    items.push(self.build_offline_submenu(ch));
                }
            }
        }

        items.push(ksni::MenuItem::Separator);
        items.push(ksni::MenuItem::Standard(ksni::menu::StandardItem {
            label: "종료".to_string(),
            activate: Box::new(|_| std::process::exit(0)),
            ..Default::default()
        }));

        items
    }

    fn watcher_offline(&self, reason: ksni::OfflineReason) -> bool {
        tracing::warn!("StatusNotifierWatcher offline: {reason:?}");
        true
    }
}

impl KucheatTray {
    fn build_live_submenu(
        &self,
        ch: &ChannelConfig,
        cs: Option<&ChannelState>,
    ) -> ksni::MenuItem<Self> {
        let name = cs.map_or(&ch.name as &str, |s| &s.channel_name);
        let mut sub: Vec<ksni::MenuItem<Self>> = Vec::new();

        if let Some(title) = cs.and_then(|s| s.live_title.as_deref()) {
            sub.push(label_item(format!("📺 {title}")));
        }
        if let Some(count) = cs.and_then(|s| s.viewer_count) {
            sub.push(label_item(format!("👤 시청자 {count}명")));
        }

        sub.push(ksni::MenuItem::Separator);
        sub.push(link_item(
            "🔗 라이브 보기".into(),
            chzzk_url("live", &ch.id),
        ));
        sub.push(link_item(
            "🔗 채널 페이지 열기".into(),
            chzzk_url("", &ch.id),
        ));

        ksni::MenuItem::SubMenu(ksni::menu::SubMenu {
            label: format!("🔴 {name}"),
            submenu: sub,
            ..Default::default()
        })
    }

    fn build_offline_submenu(&self, ch: &ChannelConfig) -> ksni::MenuItem<Self> {
        ksni::MenuItem::SubMenu(ksni::menu::SubMenu {
            label: format!("⚫ {}", ch.name),
            submenu: vec![link_item(
                "🔗 채널 페이지 열기".into(),
                chzzk_url("", &ch.id),
            )],
            ..Default::default()
        })
    }
}

// ---------------------------------------------------------------------------
// Entry-point
// ---------------------------------------------------------------------------

pub async fn run(source: StateSource) -> Result<()> {
    tracing::info!("Starting Kucheat tray app…");

    let config = Config::load()?;

    let tray_data = Arc::new(StdMutex::new(TrayData {
        channels: config.channels.clone(),
        state: AppState::default(),
    }));

    let handle = KucheatTray {
        data: Arc::clone(&tray_data),
    }
    .assume_sni_available(true)
    .spawn()
    .await
    .map_err(|e| anyhow::anyhow!("Failed to spawn tray: {e}"))?;

    tracing::info!("Tray icon registered, entering state-sync loop…");

    match source {
        // ── Daemon in same process: react to watch channel instantly ──
        StateSource::Watch(mut rx) => {
            loop {
                // Block until the daemon pushes a new state.
                if rx.changed().await.is_err() {
                    tracing::warn!("Daemon state channel closed, stopping tray sync");
                    break;
                }

                let app_state = rx.borrow_and_update().clone();
                let config = Config::load().unwrap_or_default();

                {
                    let mut td = tray_data.lock().unwrap();
                    td.channels = config.channels;
                    td.state = app_state;
                }

                handle.update(|_| {}).await;
            }
        }

        // ── Standalone tray: poll disk every few seconds ─────────────
        StateSource::File => {
            const SYNC_INTERVAL: Duration = Duration::from_secs(3);

            loop {
                let config = Config::load().unwrap_or_default();
                let app_state = AppState::load().unwrap_or_default();

                {
                    let mut td = tray_data.lock().unwrap();
                    td.channels = config.channels;
                    td.state = app_state;
                }

                handle.update(|_| {}).await;
                tokio::time::sleep(SYNC_INTERVAL).await;
            }
        }
    }

    Ok(())
}
