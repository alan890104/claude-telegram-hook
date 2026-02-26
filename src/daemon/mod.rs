mod server;
pub mod state;
mod telegram;
pub mod tray;

use crate::config::Config;
use state::{AppState, TrayUpdate};
use std::sync::Arc;
use teloxide::payloads::EditMessageTextSetters;
use teloxide::prelude::Requester;
use teloxide::Bot;
use tokio::net::TcpListener;
use tracing::{error, info};

/// Run the daemon: starts axum server, teloxide polling, and timeout reaper.
/// The tray event loop must be run separately on the main thread.
pub async fn run(
    config: Config,
    tray_tx: std::sync::mpsc::Sender<TrayUpdate>,
    tray_shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) -> anyhow::Result<()> {
    let bot = Bot::new(&config.bot_token);
    let port = config.daemon_port;

    // Create a second shutdown channel for the HTTP /shutdown endpoint
    let (http_shutdown_tx, http_shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let state = AppState::new(config.clone(), bot, tray_tx, http_shutdown_tx);

    // Start axum HTTP server
    let app = server::router(state.clone());
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    info!("HTTP server listening on {}", addr);

    let server_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            error!("HTTP server error: {}", e);
        }
    });

    // Start teloxide long polling
    let poll_state = state.clone();
    let polling_handle = tokio::spawn(async move {
        telegram::start_polling(poll_state).await;
    });

    // Start timeout reaper
    let reaper_state = state.clone();
    let reaper_handle = tokio::spawn(async move {
        timeout_reaper(reaper_state).await;
    });

    // Wait for shutdown signal from either tray menu or HTTP endpoint
    tokio::select! {
        _ = tray_shutdown_rx => {
            info!("Shutdown signal received from tray menu");
        }
        _ = http_shutdown_rx => {
            info!("Shutdown signal received from HTTP endpoint");
        }
    }

    // Abort background tasks
    server_handle.abort();
    polling_handle.abort();
    reaper_handle.abort();

    Ok(())
}

/// Periodically scans pending requests and times out expired ones.
async fn timeout_reaper(state: Arc<AppState>) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
    loop {
        interval.tick().await;

        let timeout_secs = state.config.permission_timeout;
        let now = chrono::Utc::now();
        let mut expired = Vec::new();

        {
            let pending = state.pending.read().await;
            for (id, req) in pending.iter() {
                let elapsed = (now - req.created_at).num_seconds() as u64;
                if elapsed >= timeout_secs {
                    expired.push(id.clone());
                }
            }
        }

        if expired.is_empty() {
            continue;
        }

        let mut pending = state.pending.write().await;
        for id in &expired {
            if let Some(req) = pending.remove(id) {
                // Send timeout decision
                let _ = req.sender.send(crate::types::Decision::Timeout);

                // Edit Telegram message
                let chat_id = teloxide::types::ChatId(
                    state.config.chat_id.parse::<i64>().unwrap_or(0),
                );
                let new_text = format!(
                    "{}\n\n⏰ <b>Timed out — respond in terminal</b>",
                    req.original_text
                );
                let _ = state
                    .bot
                    .edit_message_text(
                        chat_id,
                        teloxide::types::MessageId(req.telegram_msg_id),
                        new_text,
                    )
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await;

                info!(request_id = %id, "Request timed out by reaper");
            }
        }
        drop(pending);
        state.notify_tray_pending().await;
    }
}
