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
    /// Tracks permission request count per session since last stop,
    /// used to decide whether a "task complete" notification is worth sending.
    pub session_permission_count: RwLock<HashMap<String, u32>>,
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
            session_permission_count: RwLock::new(HashMap::new()),
        })
    }

    /// Increment the permission request count for a session.
    pub async fn record_permission(&self, session_id: &str) {
        let mut map = self.session_permission_count.write().await;
        *map.entry(session_id.to_string()).or_insert(0) += 1;
    }

    /// Take (and reset) the permission count for a session. Returns 0 if none.
    pub async fn take_permission_count(&self, session_id: &str) -> u32 {
        let mut map = self.session_permission_count.write().await;
        map.remove(session_id).unwrap_or(0)
    }

    /// Notify the tray of current pending count.
    pub async fn notify_tray_pending(&self) {
        let count = self.pending.read().await.len();
        let _ = self.tray_tx.send(TrayUpdate::PendingCount(count));
    }
}
