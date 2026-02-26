use crate::daemon::state::AppState;
use crate::types::Decision;
use std::sync::Arc;
use std::time::Duration;
use teloxide::dispatching::UpdateFilterExt;
use teloxide::error_handlers::LoggingErrorHandler;
use teloxide::prelude::*;
use teloxide::types::{AllowedUpdate, CallbackQuery, ChatId, MessageId};
use teloxide::update_listeners::Polling;
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

    // Only poll for callback queries, drop stale updates from before daemon start
    let polling = Polling::builder(bot.clone())
        .allowed_updates(vec![AllowedUpdate::CallbackQuery])
        .timeout(Duration::from_secs(30))
        .drop_pending_updates()
        .build();

    Dispatcher::builder(bot, handler)
        .build()
        .dispatch_with_listener(
            polling,
            LoggingErrorHandler::with_custom_text("Telegram polling error"),
        )
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
            // Answer the callback query (stops the loading spinner in Telegram)
            let answer_text = if decision == Decision::Allow {
                "Allowed"
            } else {
                "Denied"
            };
            let _ = bot
                .answer_callback_query(&query.id)
                .text(answer_text)
                .await;

            // Send decision to the waiting HTTP handler IMMEDIATELY
            // (don't block Claude Code on the delete_message API call)
            let _ = req.sender.send(decision);
            info!(request_id, ?decision, "Permission resolved");

            // Update tray
            state.notify_tray_pending().await;

            // Delete the permission message in the background
            let chat = ChatId(state.config.chat_id.parse::<i64>().unwrap_or(0));
            let msg_id = req.telegram_msg_id;
            tokio::spawn(async move {
                let _ = bot.delete_message(chat, MessageId(msg_id)).await;
            });
        }
        None => {
            // Stale or expired button
            let _ = bot
                .answer_callback_query(&query.id)
                .text("This button has expired")
                .await;
        }
    }
}
