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
        .route("/shutdown", post(handle_shutdown))
        .with_state(state)
}

/// Extract a short session label from the payload (last component of cwd).
fn session_label(payload: &Value) -> String {
    let cwd = payload
        .get("cwd")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if cwd.is_empty() {
        return String::new();
    }
    let dir_name = std::path::Path::new(cwd)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(cwd);
    format!("📂 <code>{}</code>\n", encode_text(dir_name))
}

/// Truncate a string to max_len, appending "..." if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}

/// Format the permission message based on tool type.
fn format_permission_message(payload: &Value) -> String {
    let tool_name = payload
        .get("tool_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");
    let tool_input = payload.get("tool_input").cloned().unwrap_or(json!({}));
    let session = session_label(payload);

    let mut lines = vec![
        "🔐 <b>Permission Required</b>".to_string(),
        String::new(),
        format!("{}Tool: <b>{}</b>", session, encode_text(tool_name)),
    ];

    match tool_name {
        "Bash" => {
            let cmd = tool_input
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let desc = tool_input
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !desc.is_empty() {
                lines.push(format!("{}", encode_text(&truncate(desc, 200))));
            }
            lines.push(format!(
                "\n<pre>{}</pre>",
                encode_text(&truncate(cmd, 800))
            ));
        }
        "Edit" | "Write" | "MultiEdit" => {
            let file_path = tool_input
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            lines.push(format!("File: <code>{}</code>", encode_text(file_path)));

            if let Some(old) = tool_input.get("old_string").and_then(|v| v.as_str()) {
                if !old.is_empty() {
                    lines.push(format!(
                        "\n<pre>{}</pre>",
                        encode_text(&truncate(old, 300))
                    ));
                    lines.push("↓".to_string());
                }
            }
            if let Some(new) = tool_input.get("new_string").and_then(|v| v.as_str()) {
                if !new.is_empty() {
                    lines.push(format!(
                        "<pre>{}</pre>",
                        encode_text(&truncate(new, 300))
                    ));
                }
            }
        }
        "Task" => {
            let desc = tool_input
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let prompt = tool_input
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !desc.is_empty() {
                lines.push(format!("{}", encode_text(&truncate(desc, 300))));
            }
            if !prompt.is_empty() {
                lines.push(format!(
                    "\n<pre>{}</pre>",
                    encode_text(&truncate(prompt, 500))
                ));
            }
        }
        "AskUserQuestion" => {
            lines.clear();
            lines.push("❓ <b>Claude has a question</b>".to_string());
            lines.push(String::new());
            lines.push(session.clone());

            if let Some(questions) = tool_input.get("questions").and_then(|v| v.as_array()) {
                for (qi, q) in questions.iter().enumerate() {
                    let question_text = q
                        .get("question")
                        .and_then(|v| v.as_str())
                        .unwrap_or("(no question text)");
                    if questions.len() > 1 {
                        lines.push(format!(
                            "<b>Q{}: {}</b>",
                            qi + 1,
                            encode_text(question_text)
                        ));
                    } else {
                        lines.push(format!("<b>{}</b>", encode_text(question_text)));
                    }

                    if let Some(options) = q.get("options").and_then(|v| v.as_array()) {
                        for (oi, opt) in options.iter().enumerate() {
                            let label = opt
                                .get("label")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?");
                            let desc = opt
                                .get("description")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            if !desc.is_empty() {
                                lines.push(format!(
                                    "  {}. <b>{}</b> — {}",
                                    oi + 1,
                                    encode_text(label),
                                    encode_text(&truncate(desc, 150))
                                ));
                            } else {
                                lines.push(format!(
                                    "  {}. <b>{}</b>",
                                    oi + 1,
                                    encode_text(label)
                                ));
                            }
                        }
                    }
                    lines.push(String::new());
                }
            }
            lines.push("💡 <i>Respond in the terminal</i>".to_string());
        }
        "Read" | "Glob" | "Grep" => {
            // Typically auto-approved, but if not:
            if let Some(path) = tool_input.get("file_path").and_then(|v| v.as_str()) {
                lines.push(format!("Path: <code>{}</code>", encode_text(path)));
            }
            if let Some(pattern) = tool_input.get("pattern").and_then(|v| v.as_str()) {
                lines.push(format!("Pattern: <code>{}</code>", encode_text(pattern)));
            }
        }
        "WebFetch" | "WebSearch" => {
            if let Some(url) = tool_input.get("url").and_then(|v| v.as_str()) {
                lines.push(format!("URL: {}", encode_text(&truncate(url, 200))));
            }
            if let Some(query) = tool_input.get("query").and_then(|v| v.as_str()) {
                lines.push(format!("Query: {}", encode_text(&truncate(query, 200))));
            }
        }
        _ => {
            // Generic fallback — show key-value pairs instead of raw JSON
            if let Some(obj) = tool_input.as_object() {
                for (key, val) in obj.iter().take(5) {
                    let val_str = match val {
                        Value::String(s) => truncate(s, 200),
                        _ => truncate(&val.to_string(), 200),
                    };
                    lines.push(format!(
                        "{}: <code>{}</code>",
                        encode_text(key),
                        encode_text(&val_str)
                    ));
                }
            } else {
                let detail = serde_json::to_string(&tool_input).unwrap_or_default();
                lines.push(format!(
                    "Input: <code>{}</code>",
                    encode_text(&truncate(&detail, 300))
                ));
            }
        }
    }

    lines.join("\n")
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
    // Track that this session had a permission request
    if let Some(sid) = req.payload.get("session_id").and_then(|v| v.as_str()) {
        state.record_permission(sid).await;
    }

    let text = format_permission_message(&req.payload);

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

    let session = session_label(payload);

    let ntype = payload
        .get("notification_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Skip noisy notifications that aren't actionable from Telegram
    if ntype == "idle_prompt" {
        return StatusCode::OK;
    }

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
        "auth_success" => "🔑",
        _ => "📢",
    };

    let safe_title = encode_text(title);
    let msg_truncated = truncate(message, 1000);
    let safe_message = encode_text(&msg_truncated);

    let mut text = format!(
        "{} <b>{}</b>\n\n{}{}",
        emoji, safe_title, session, safe_message
    );

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
    let session_id = payload
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Only notify if this session had permission requests (i.e. real tool work).
    // Pure chat (no tool calls needing approval) gets silently skipped.
    if !session_id.is_empty() {
        let count = state.take_permission_count(session_id).await;
        if count == 0 {
            info!(session_id, "Skipping stop notification (no permission requests)");
            return StatusCode::OK;
        }
    }

    let session = session_label(payload);

    let last_msg = payload
        .get("last_assistant_message")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let text = if !last_msg.is_empty() {
        format!(
            "✅ <b>Task complete</b>\n\n{}{}",
            session,
            encode_text(&truncate(last_msg, 500))
        )
    } else {
        format!("✅ <b>Task complete</b>\n\n{}", session)
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

async fn handle_shutdown(State(state): State<Arc<AppState>>) -> Json<Value> {
    info!("Shutdown requested via HTTP");
    let mut tx = state.shutdown_tx.lock().await;
    if let Some(sender) = tx.take() {
        let _ = sender.send(());
    }
    Json(json!({"status": "shutting down"}))
}
