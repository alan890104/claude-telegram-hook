use crate::config::Config;
use anyhow::Result;
use std::io::{self, Write};

/// Run the interactive setup wizard.
pub fn run() -> Result<()> {
    println!("=== Claude Telegram Bridge Setup ===\n");

    // Step 1: Bot Token
    println!("Step 1: Create a Telegram Bot");
    println!("  1. Search for @BotFather in Telegram");
    println!("  2. Send /newbot and follow the instructions");
    println!("  3. Copy the Bot Token\n");

    print!("Enter Bot Token: ");
    io::stdout().flush()?;
    let mut token = String::new();
    io::stdin().read_line(&mut token)?;
    let token = token.trim().to_string();

    if token.is_empty() {
        println!("Error: Token cannot be empty");
        return Ok(());
    }

    // Verify token with getMe
    let client = reqwest::blocking::Client::new();
    let url = format!("https://api.telegram.org/bot{}/getMe", token);
    let resp: serde_json::Value = client.get(&url).send()?.json()?;

    if resp.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        println!("  Error: Invalid token");
        return Ok(());
    }

    let bot_name = resp["result"]["username"]
        .as_str()
        .unwrap_or("unknown");
    println!("  Bot verified: @{}\n", bot_name);

    // Step 2: Chat ID
    println!("Step 2: Get your Chat ID");
    println!("  1. Open @{} in Telegram", bot_name);
    println!("  2. Send any message (e.g. hello) to the bot");
    print!("  Press Enter when done...");
    io::stdout().flush()?;
    let mut _buf = String::new();
    io::stdin().read_line(&mut _buf)?;

    let mut chat_id = String::new();
    let url = format!("https://api.telegram.org/bot{}/getUpdates", token);
    if let Ok(resp) = client.get(&url).send() {
        if let Ok(data) = resp.json::<serde_json::Value>() {
            if let Some(updates) = data["result"].as_array() {
                for update in updates.iter().rev() {
                    if let Some(id) = update["message"]["chat"]["id"].as_i64() {
                        chat_id = id.to_string();
                        let user_name = update["message"]["from"]["first_name"]
                            .as_str()
                            .unwrap_or("");
                        println!("  Found Chat ID: {} ({})\n", chat_id, user_name);
                        break;
                    }
                }
            }
        }
    }

    if chat_id.is_empty() {
        println!("  Could not detect Chat ID automatically");
        print!("  Please enter your Chat ID manually: ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut chat_id)?;
        chat_id = chat_id.trim().to_string();
        if chat_id.is_empty() {
            println!("  Error: Chat ID cannot be empty");
            return Ok(());
        }
        println!();
    }

    // Step 3: Timeout
    println!("Step 3: Set permission timeout (seconds)");
    println!("  When Claude needs permission, it waits for your response on Telegram");
    println!("  After timeout, it falls back to the local terminal prompt");
    print!("  Timeout in seconds [default: 300]: ");
    io::stdout().flush()?;
    let mut timeout_input = String::new();
    io::stdin().read_line(&mut timeout_input)?;
    let timeout: u64 = timeout_input.trim().parse().unwrap_or(300);
    println!("  Timeout: {} seconds\n", timeout);

    // Step 4: Save config
    let config = Config {
        bot_token: token.clone(),
        chat_id: chat_id.clone(),
        permission_timeout: timeout,
        disabled: false,
        daemon_port: 19876,
        stop_notify_after: 120,
    };
    config.save()?;
    println!("Config saved to {}", Config::config_path().display());

    // Step 5: Test message
    println!("\nSending test message...");
    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
    let resp: serde_json::Value = client
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": "Claude Telegram Bridge configured successfully!\n\nYou will receive notifications here when Claude Code needs your attention.",
        }))
        .send()?
        .json()?;

    if resp.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        println!("Test message sent — check Telegram!");
    } else {
        println!("Error: Failed to send test message");
    }

    // Step 6: Test buttons
    println!("\nTesting button functionality...");
    // Flush pending updates so we only see new callback_query
    let _ = client
        .get(&format!(
            "https://api.telegram.org/bot{}/getUpdates",
            token
        ))
        .query(&[("offset", "-1"), ("timeout", "0")])
        .send();

    let resp: serde_json::Value = client
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": "Button test\n\nPress either button to confirm it works:",
            "reply_markup": {
                "inline_keyboard": [[
                    {"text": "Allow", "callback_data": "test_allow"},
                    {"text": "Deny", "callback_data": "test_deny"},
                ]]
            },
        }))
        .send()?
        .json()?;

    if resp.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        println!("Button test sent — press a button in Telegram...");

        // Poll for callback_query (up to 30 seconds)
        let updates_url = format!("https://api.telegram.org/bot{}/getUpdates", token);
        let mut confirmed = false;
        let start = std::time::Instant::now();

        while start.elapsed() < std::time::Duration::from_secs(30) {
            if let Ok(resp) = client
                .get(&updates_url)
                .query(&[("timeout", "5"), ("allowed_updates", "[\"callback_query\"]")])
                .send()
            {
                if let Ok(data) = resp.json::<serde_json::Value>() {
                    if let Some(updates) = data["result"].as_array() {
                        for update in updates {
                            let cb_data = update["callback_query"]["data"].as_str().unwrap_or("");
                            if cb_data == "test_allow" || cb_data == "test_deny" {
                                // Answer the callback so the spinner stops
                                let cb_id = update["callback_query"]["id"].as_str().unwrap_or("");
                                let _ = client
                                    .post(&format!(
                                        "https://api.telegram.org/bot{}/answerCallbackQuery",
                                        token
                                    ))
                                    .json(&serde_json::json!({
                                        "callback_query_id": cb_id,
                                        "text": "Button works!"
                                    }))
                                    .send();

                                // Confirm the update so it doesn't repeat
                                let update_id = update["update_id"].as_i64().unwrap_or(0);
                                let _ = client
                                    .get(&updates_url)
                                    .query(&[("offset", (update_id + 1).to_string()), ("timeout", "0".to_string())])
                                    .send();

                                let which = if cb_data == "test_allow" { "Allow" } else { "Deny" };
                                println!("  Received \"{}\" — buttons work!", which);
                                confirmed = true;
                                break;
                            }
                        }
                    }
                }
            }
            if confirmed {
                break;
            }
        }

        if !confirmed {
            println!("  No button press received (timed out after 30s)");
            println!("  Buttons may still work — you can test later with the daemon");
        }
    } else {
        println!("Error: Button test failed");
    }

    println!("\n{}", "=".repeat(40));
    println!("Setup complete!");
    println!();
    println!("Next step:");
    println!("  Run: claude-telegram-bridge install");
    println!();
    println!("Management:");
    println!("  - Config file: {}", Config::config_path().display());
    println!("  - Pause: set \"disabled\" to true in the config file");
    println!("  - Adjust timeout: edit \"permission_timeout\" in the config file");

    Ok(())
}
