use std::time::Duration;

use anyhow::Result;
use tokio::sync::watch;
use tokio::time;

use crate::api::ChzzkClient;
use crate::config::Config;
use crate::notification;
use crate::state::{AppState, ChannelState};

/// Run the headless live-check daemon (intended for systemd).
/// If `state_tx` is provided, daemon pushes state updates through it
/// so that the in-process tray reacts instantly without polling.
pub async fn run(state_tx: Option<watch::Sender<AppState>>) -> Result<()> {
    tracing::info!("Starting Kucheat daemon…");

    let config = Config::load()?;

    if config.channels.is_empty() {
        tracing::warn!("No channels configured. Add channels with: kucheat add <channel_id>");
    }

    let client = ChzzkClient::new(&config.api)?;

    tracing::info!(
        "Monitoring {} channel(s) every {}s",
        config.channels.len(),
        config.settings.check_interval_secs,
    );

    let interval = Duration::from_secs(config.settings.check_interval_secs);
    let mut ticker = time::interval(interval);

    // Short-interval poll for config changes so that `kucheat add/remove`
    // triggers an immediate check instead of waiting for the next tick.
    let mut config_poll = time::interval(Duration::from_secs(2));
    config_poll.tick().await; // consume immediate first tick

    let mut last_channel_ids: Vec<String> =
        config.channels.iter().map(|c| c.id.clone()).collect();

    // Keep a running state across cycles so we never lose track of
    // channels the tray is already displaying.
    let mut state = AppState::load().unwrap_or_default();

    loop {
        // Wait for the normal check interval **or** a config file change.
        tokio::select! {
            _ = ticker.tick() => {}
            _ = config_poll.tick() => {
                let new_ids: Vec<String> = match Config::load() {
                    Ok(c) => c.channels.iter().map(|ch| ch.id.clone()).collect(),
                    Err(_) => continue,
                };
                if new_ids == last_channel_ids {
                    continue; // no change, keep waiting
                }
                tracing::info!("Config changed, running immediate channel check…");
                ticker.reset(); // avoid a redundant check right after
            }
        }

        // Reload config each cycle so the user can add/remove channels
        // without restarting the daemon.
        let config = match Config::load() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to reload config: {e}");
                continue;
            }
        };

        last_channel_ids = config.channels.iter().map(|c| c.id.clone()).collect();

        for channel in &config.channels {
            match client.check_channel_live(&channel.id).await {
                Ok(live) => {
                    let was_live = state
                        .channels
                        .get(&channel.id)
                        .is_some_and(|s| s.is_live);

                    // Notifications on state transitions.
                    if live.is_live && !was_live {
                        tracing::info!("🔴 {} is now LIVE!", live.channel_name);
                        if let Err(e) =
                            notification::send_live_notification(&channel.id, &live).await
                        {
                            tracing::error!("Failed to send live notification: {e}");
                        }
                    } else if !live.is_live && was_live {
                        tracing::info!("⚫ {} went offline", live.channel_name);
                        if config.settings.notify_offline {
                            if let Err(e) =
                                notification::send_offline_notification(&live.channel_name).await
                            {
                                tracing::error!("Failed to send offline notification: {e}");
                            }
                        }
                    }

                    let prev = state.channels.get(&channel.id);
                    state.channels.insert(
                        channel.id.clone(),
                        ChannelState::from_live_status(&live, prev),
                    );

                    // Push incremental update so tray reflects changes
                    // immediately after each channel check.
                    if let Some(ref tx) = state_tx {
                        let _ = tx.send(state.clone());
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to check channel {} ({}): {e}",
                        channel.name,
                        channel.id,
                    );
                }
            }

            // Small delay between requests to avoid rate-limiting.
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        // Persist to disk.
        if let Err(e) = state.save() {
            tracing::error!("Failed to persist state: {e}");
        }
    }
}
