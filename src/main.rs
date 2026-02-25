mod api;
mod config;
mod daemon;
mod notification;
mod state;
mod tray;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "kucheat")]
#[command(version, about = "CHZZK 라이브 알림 시스템 트레이 앱")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 라이브 체크 데몬 실행 (systemd용)
    Daemon,
    /// 시스템 트레이 앱 실행
    Tray,
    /// 모니터링할 채널 추가
    Add {
        /// 채널 ID
        channel_id: String,
        /// 채널 이름 (미지정 시 API에서 자동 조회)
        #[arg(short, long)]
        name: Option<String>,
    },
    /// 모니터링 채널 제거
    Remove {
        /// 채널 ID
        channel_id: String,
    },
    /// 모니터링 중인 채널 목록
    List,
    /// 현재 라이브 상태 조회
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("kucheat=info".parse()?),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        // ── daemon ─────────────────────────────────────────────────
        Commands::Daemon => {
            let (state_tx, state_rx) =
                tokio::sync::watch::channel(state::AppState::default());

            tokio::spawn(async move {
                let source = tray::StateSource::Watch(state_rx);
                if let Err(e) = tray::run(source).await {
                    tracing::error!("Tray error: {e}");
                }
            });

            daemon::run(Some(state_tx)).await?
        }

        // ── tray ───────────────────────────────────────────────────
        Commands::Tray => tray::run(tray::StateSource::File).await?,

        // ── add channel ────────────────────────────────────────────
        Commands::Add { channel_id, name } => {
            let mut config = config::Config::load()?;

            let display_name = match name {
                Some(n) => n,
                None => {
                    let client = api::ChzzkClient::new(&config.api)?;
                    client
                        .get_channel_info(&channel_id)
                        .await
                        .map(|info| info.channel_name)
                        .unwrap_or_else(|_| channel_id.clone())
                }
            };

            config.add_channel(&channel_id, &display_name);
            config.save()?;
            println!("✅ 채널 추가: {display_name} ({channel_id})");
        }

        // ── remove channel ─────────────────────────────────────────
        Commands::Remove { channel_id } => {
            let mut config = config::Config::load()?;
            if config.remove_channel(&channel_id) {
                config.save()?;
                println!("🗑️  채널 제거: {channel_id}");
            } else {
                println!("⚠️  해당 채널을 찾을 수 없습니다: {channel_id}");
            }
        }

        // ── list ───────────────────────────────────────────────────
        Commands::List => {
            let config = config::Config::load()?;
            if config.channels.is_empty() {
                println!("등록된 채널이 없습니다.");
            } else {
                println!("모니터링 채널 목록:");
                for ch in &config.channels {
                    println!("  • {} ({})", ch.name, ch.id);
                }
            }
        }

        // ── status ─────────────────────────────────────────────────
        Commands::Status => {
            let config = config::Config::load()?;
            if config.channels.is_empty() {
                println!("등록된 채널이 없습니다.");
                return Ok(());
            }

            let mut app_state = state::AppState::load().unwrap_or_default();

            // Fetch live state for any channel missing from persisted state.
            let missing: Vec<_> = config
                .channels
                .iter()
                .filter(|ch| !app_state.channels.contains_key(&ch.id))
                .cloned()
                .collect();

            if !missing.is_empty() {
                let client = api::ChzzkClient::new(&config.api)?;
                for ch in &missing {
                    match client.check_channel_live(&ch.id).await {
                        Ok(live) => {
                            app_state.channels.insert(
                                ch.id.clone(),
                                state::ChannelState::from_live_status(&live, None),
                            );
                        }
                        Err(e) => {
                            tracing::error!("Failed to check {}: {e}", ch.id);
                        }
                    }
                }
                let _ = app_state.save();
            }

            println!("채널 상태:");
            for ch in &config.channels {
                let (icon, extra) = match app_state.channels.get(&ch.id) {
                    Some(s) if s.is_live => {
                        let title = s
                            .live_title
                            .as_deref()
                            .map(|t| format!(" 「{t}」"))
                            .unwrap_or_default();
                        ("🔴 LIVE", title)
                    }
                    Some(_) => ("⚫ Offline", String::new()),
                    None => ("❓ Unknown", String::new()),
                };
                println!("  {icon} {} ({}){extra}", ch.name, ch.id);
            }
        }
    }

    Ok(())
}
