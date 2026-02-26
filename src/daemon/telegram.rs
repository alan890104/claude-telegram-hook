use crate::daemon::state::AppState;
use crate::types::Decision;
use std::sync::Arc;
use teloxide::dispatching::UpdateFilterExt;
use teloxide::prelude::*;
use teloxide::types::{CallbackQuery, ChatId, MessageId, ParseMode};
use tracing::{info, warn};

/// Start the teloxide dispatcher for handling callback queries.
pub async fn start_polling(state: Arc<AppState>) {
    let bot = state.bot.clone();

    let handler = Update::filter_callback_query().endpoint(
        move |bot: Bot, q: CallbackQuery| {
            let state = state.clone();
            async move {
                handle_callback(bot, q, state).await;
                respond(())
            }
        },
    );

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

async fn handle_callback(bot: Bot, query: CallbackQuery, state: Arc<AppState>) {
    let data = match &query.data {
        Some(d) => d.clone(),
        None => return,
    };

    // Verify chat_id
    let chat_id = query
        .message
        .as_ref()
        .map(|m| m.chat().id.0.to_string());

    if let Some(ref cid) = chat_id {
        if cid != &state.config.chat_id {
            warn!("Callback from unauthorized chat: {}", cid);
            return;
        }
    }

    // Parse callback data: "allow:<request_id>" or "deny:<request_id>"
    let parts: Vec<&str> = data.splitn(2, ':').collect();
    if parts.len() != 2 {
        return;
    }

    let decision_str = parts[0];
    let request_id = parts[1];

    let decision = match decision_str {
        "allow" => Decision::Allow,
        "deny" => Decision::Deny,
        _ => return,
    };

    // Look up the pending request
    let pending = {
        let mut map = state.pending.write().await;
        map.remove(request_id)
    };

    match pending {
        Some(req) => {
            // Answer the callback query
            let answer_text = if decision == Decision::Allow {
                "已允許"
            } else {
                "已拒絕"
            };
            let _ = bot
                .answer_callback_query(&query.id)
                .text(answer_text)
                .await;

            // Edit original message with result badge
            let badge = if decision == Decision::Allow {
                "\n\n✅ <b>已允許</b>"
            } else {
                "\n\n❌ <b>已拒絕</b>"
            };
            let new_text = format!("{}{}", req.original_text, badge);

            let chat = ChatId(state.config.chat_id.parse::<i64>().unwrap_or(0));
            let _ = bot
                .edit_message_text(chat, MessageId(req.telegram_msg_id), new_text)
                .parse_mode(ParseMode::Html)
                .await;

            // Send decision to the waiting HTTP handler
            let _ = req.sender.send(decision);
            info!(request_id, ?decision, "Permission resolved");

            // Update tray
            state.notify_tray_pending().await;
        }
        None => {
            // Stale or expired button
            let _ = bot
                .answer_callback_query(&query.id)
                .text("此按鈕已過期")
                .await;
        }
    }
}
