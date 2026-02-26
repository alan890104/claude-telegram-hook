use serde::{Deserialize, Serialize};

/// Hook type corresponding to Claude Code hook events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookType {
    Permission,
    Notification,
    Stop,
}

/// Request sent from hook thin client to daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookRequest {
    pub request_id: String,
    pub hook_type: HookType,
    pub payload: serde_json::Value,
}

/// Decision made by the user via Telegram.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Allow,
    Deny,
    Timeout,
}

/// Response sent from daemon back to hook thin client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResponse {
    pub request_id: String,
    pub decision: Decision,
}
