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
    /// Tracks when each session_id was first seen, for filtering short sessions.
    pub sessions: RwLock<HashMap<String, DateTime<Utc>>>,
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
            sessions: RwLock::new(HashMap::new()),
        })
    }

    /// Record a session's first contact time. Returns the first-seen timestamp.
    pub async fn touch_session(&self, session_id: &str) -> DateTime<Utc> {
        let mut sessions = self.sessions.write().await;
        *sessions.entry(session_id.to_string()).or_insert_with(Utc::now)
    }

    /// Get how long a session has been active (in seconds). Returns 0 if unknown.
    pub async fn session_age_secs(&self, session_id: &str) -> u64 {
        let sessions = self.sessions.read().await;
        match sessions.get(session_id) {
            Some(first_seen) => (Utc::now() - *first_seen).num_seconds().max(0) as u64,
            None => 0,
        }
    }

    /// Remove a session from tracking.
    pub async fn remove_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
    }

    /// Notify the tray of current pending count.
    pub async fn notify_tray_pending(&self) {
        let count = self.pending.read().await.len();
        let _ = self.tray_tx.send(TrayUpdate::PendingCount(count));
    }
}
