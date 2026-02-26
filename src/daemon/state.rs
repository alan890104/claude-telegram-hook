use crate::config::Config;
use crate::types::Decision;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock, Mutex};

/// Message sent to the tray thread to update UI.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TrayUpdate {
    PendingCount(usize),
    Error(String),
    ClearError,
}

/// A pending permission request awaiting user response.
pub struct PendingRequest {
    pub telegram_msg_id: i32,
    pub original_text: String,
    pub sender: oneshot::Sender<Decision>,
    pub created_at: DateTime<Utc>,
}

/// Shared application state across axum handlers and teloxide.
pub struct AppState {
    pub config: Config,
    pub pending: RwLock<HashMap<String, PendingRequest>>,
    pub bot: teloxide::Bot,
    pub tray_tx: std::sync::mpsc::Sender<TrayUpdate>,
    /// Oneshot sender to trigger graceful shutdown from HTTP endpoint.
    pub shutdown_tx: Mutex<Option<oneshot::Sender<()>>>,
}

impl AppState {
    pub fn new(
        config: Config,
        bot: teloxide::Bot,
        tray_tx: std::sync::mpsc::Sender<TrayUpdate>,
        shutdown_tx: oneshot::Sender<()>,
    ) -> Arc<Self> {
        Arc::new(Self {
            config,
            pending: RwLock::new(HashMap::new()),
            bot,
            tray_tx,
            shutdown_tx: Mutex::new(Some(shutdown_tx)),
        })
    }

    /// Notify the tray of current pending count.
    pub async fn notify_tray_pending(&self) {
        let count = self.pending.read().await.len();
        let _ = self.tray_tx.send(TrayUpdate::PendingCount(count));
    }
}
