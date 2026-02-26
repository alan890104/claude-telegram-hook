use crate::daemon::state::{AppState, PendingRequest};
use crate::types::{Decision, HookRequest, HookResponse};
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use html_escape::encode_text;
use serde_json::{json, Value};
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardMarkup, MessageId, ParseMode, ReplyMarkup,
};
use tokio::sync::oneshot;
use tracing::{error, info};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/hook/permission", post(handle_permission))
        .route("/hook/notification", post(handle_notification))
        .route("/hook/stop", post(handle_stop))
        .route("/health", get(handle_health))
        .with_state(state)
}

async fn handle_health(State(state): State<Arc<AppState>>) -> Json<Value> {
    let pending = state.pending.read().await.len();
    Json(json!({
        "status": "ok",
        "pending": pending,
    }))
}

async fn handle_permission(
    State(state): State<Arc<AppState>>,
    Json(req): Json<HookRequest>,
) -> Result<Json<HookResponse>, StatusCode> {
    let payload = &req.payload;
    let tool_name = payload
        .get("tool_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");
    let tool_input = payload.get("tool_input").cloned().unwrap_or(json!({}));

    // Build HTML-formatted message
    let mut lines = vec![
        "🔐 <b>Permission Required</b>".to_string(),
        String::new(),
        format!("Tool: <b>{}</b>", encode_text(tool_name)),
    ];

    match tool_name {
        "Bash" => {
            let cmd = tool_input
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let cmd = if cmd.len() > 500 { &cmd[..500] } else { cmd };
            lines.push(format!("Command: <code>{}</code>", encode_text(cmd)));
            if tool_input
                .get("command")
                .and_then(|v| v.as_str())
                .map_or(false, |c| c.len() > 500)
            {
                lines.push("...(truncated)".to_string());
            }
        }
        "Edit" | "Write" | "MultiEdit" => {
            let file_path = tool_input
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            lines.push(format!("File: <code>{}</code>", encode_text(file_path)));
        }
        "Task" => {
            let desc = tool_input
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            lines.push(format!("Description: {}", encode_text(desc)));
        }
        _ => {
            let detail = serde_json::to_string(&tool_input).unwrap_or_default();
            let detail = if detail.len() > 300 {
                format!("{}...", &detail[..300])
            } else {
                detail
            };
            lines.push(format!("Input: <code>{}</code>", encode_text(&detail)));
        }
    }

    let text = lines.join("\n");

    // Build inline keyboard
    let keyboard = InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("Allow", format!("allow:{}", req.request_id)),
        InlineKeyboardButton::callback("Deny", format!("deny:{}", req.request_id)),
    ]]);

    let chat_id = ChatId(
        state
            .config
            .chat_id
            .parse::<i64>()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    );

    // Send Telegram message
    let msg = state
        .bot
        .send_message(chat_id, &text)
        .parse_mode(ParseMode::Html)
        .reply_markup(ReplyMarkup::InlineKeyboard(keyboard))
        .await
        .map_err(|e| {
            error!("Failed to send Telegram message: {}", e);
            StatusCode::BAD_GATEWAY
        })?;

    let msg_id = msg.id.0;

    // Create oneshot channel for response
    let (tx, rx) = oneshot::channel::<Decision>();

    // Store pending request
    {
        let mut pending = state.pending.write().await;
        pending.insert(
            req.request_id.clone(),
            PendingRequest {
                telegram_msg_id: msg_id,
                original_text: text.clone(),
                sender: tx,
                created_at: Utc::now(),
            },
        );
    }
    state.notify_tray_pending().await;

    info!(request_id = %req.request_id, "Permission request sent to Telegram");

    // Wait for response or timeout
    let timeout_secs = state.config.permission_timeout;
    let decision = tokio::select! {
        result = rx => {
            match result {
                Ok(d) => d,
                Err(_) => Decision::Timeout,
            }
        }
        _ = tokio::time::sleep(std::time::Duration::from_secs(timeout_secs)) => {
            // Timeout: remove from pending and edit message
            let removed = {
                let mut pending = state.pending.write().await;
                pending.remove(&req.request_id)
            };
            if let Some(req_data) = removed {
                let new_text = format!("{}\n\n⏰ <b>Timed out — respond in terminal</b>", req_data.original_text);
                let _ = state.bot
                    .edit_message_text(chat_id, MessageId(req_data.telegram_msg_id), new_text)
                    .parse_mode(ParseMode::Html)
                    .await;
            }
            state.notify_tray_pending().await;
            Decision::Timeout
        }
    };

    Ok(Json(HookResponse {
        request_id: req.request_id,
        decision,
    }))
}

async fn handle_notification(
    State(state): State<Arc<AppState>>,
    Json(req): Json<HookRequest>,
) -> StatusCode {
    let payload = &req.payload;

    let ntype = payload
        .get("notification_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let message = payload
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Claude Code");

    let emoji = match ntype {
        "permission_prompt" => "🔔",
        "elicitation_dialog" => "❓",
        "idle_prompt" => "💤",
        "auth_success" => "🔑",
        _ => "📢",
    };

    let safe_title = encode_text(title);
    let msg_text = if message.len() > 1000 {
        &message[..1000]
    } else {
        message
    };
    let safe_message = encode_text(msg_text);

    let mut text = format!("{} <b>{}</b>\n\n{}", emoji, safe_title, safe_message);

    if ntype == "elicitation_dialog" {
        text.push_str("\n\n💡 <i>Please respond in the terminal</i>");
    }

    let chat_id = match state.config.chat_id.parse::<i64>() {
        Ok(id) => ChatId(id),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };

    match state
        .bot
        .send_message(chat_id, &text)
        .parse_mode(ParseMode::Html)
        .await
    {
        Ok(_) => StatusCode::OK,
        Err(e) => {
            error!("Failed to send notification: {}", e);
            StatusCode::BAD_GATEWAY
        }
    }
}

async fn handle_stop(
    State(state): State<Arc<AppState>>,
    Json(req): Json<HookRequest>,
) -> StatusCode {
    let payload = &req.payload;

    let last_msg = payload
        .get("last_assistant_message")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let text = if !last_msg.is_empty() {
        let truncated = if last_msg.len() > 500 {
            format!("{}...", &last_msg[..500])
        } else {
            last_msg.to_string()
        };
        format!(
            "✅ <b>Claude Code task complete</b>\n\n{}",
            encode_text(&truncated)
        )
    } else {
        "✅ <b>Claude Code task complete</b>".to_string()
    };

    let chat_id = match state.config.chat_id.parse::<i64>() {
        Ok(id) => ChatId(id),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };

    match state
        .bot
        .send_message(chat_id, &text)
        .parse_mode(ParseMode::Html)
        .await
    {
        Ok(_) => StatusCode::OK,
        Err(e) => {
            error!("Failed to send stop notification: {}", e);
            StatusCode::BAD_GATEWAY
        }
    }
}
