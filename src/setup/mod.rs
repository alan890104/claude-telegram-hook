use crate::config::Config;
use anyhow::Result;
use std::io::{self, Write};

/// Run the interactive setup wizard.
pub fn run() -> Result<()> {
    println!("=== Claude Code Telegram Hook 設定 ===\n");

    // Step 1: Bot Token
    println!("步驟 1: 建立 Telegram Bot");
    println!("  1. 在 Telegram 搜尋 @BotFather");
    println!("  2. 發送 /newbot 並按指示操作");
    println!("  3. 複製 Bot Token\n");

    print!("請輸入 Bot Token: ");
    io::stdout().flush()?;
    let mut token = String::new();
    io::stdin().read_line(&mut token)?;
    let token = token.trim().to_string();

    if token.is_empty() {
        println!("❌ Token 不可為空");
        return Ok(());
    }

    // Verify token with getMe
    let client = reqwest::blocking::Client::new();
    let url = format!("https://api.telegram.org/bot{}/getMe", token);
    let resp: serde_json::Value = client.get(&url).send()?.json()?;

    if resp.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        println!("  ❌ Token 無效");
        return Ok(());
    }

    let bot_name = resp["result"]["username"]
        .as_str()
        .unwrap_or("unknown");
    println!("  ✅ Bot 驗證成功: @{}\n", bot_name);

    // Step 2: Chat ID
    println!("步驟 2: 取得 Chat ID");
    println!("  1. 在 Telegram 開啟 @{}", bot_name);
    println!("  2. 發送任意訊息（例如 hello）給 Bot");
    print!("  完成後按 Enter 繼續...");
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
                        println!("  ✅ 找到 Chat ID: {} ({})\n", chat_id, user_name);
                        break;
                    }
                }
            }
        }
    }

    if chat_id.is_empty() {
        println!("  ⚠️  無法自動偵測 Chat ID");
        print!("  請手動輸入 Chat ID: ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut chat_id)?;
        chat_id = chat_id.trim().to_string();
        if chat_id.is_empty() {
            println!("  ❌ Chat ID 不可為空");
            return Ok(());
        }
        println!();
    }

    // Step 3: Timeout
    println!("步驟 3: 設定權限等待逾時（秒）");
    println!("  當 Claude 需要權限時，會在 Telegram 等待您回應");
    println!("  逾時後會回到終端機的本地提示");
    print!("  逾時秒數 [預設 300]: ");
    io::stdout().flush()?;
    let mut timeout_input = String::new();
    io::stdin().read_line(&mut timeout_input)?;
    let timeout: u64 = timeout_input.trim().parse().unwrap_or(300);
    println!("  ✅ 逾時: {} 秒\n", timeout);

    // Step 4: Save config
    let config = Config {
        bot_token: token.clone(),
        chat_id: chat_id.clone(),
        permission_timeout: timeout,
        disabled: false,
        daemon_port: 19876,
    };
    config.save()?;
    println!("✅ 設定已儲存到 {}", Config::config_path().display());

    // Step 5: Test message
    println!("\n正在發送測試訊息...");
    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
    let resp: serde_json::Value = client
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": "🎉 Claude Code Telegram Hook 設定成功！\n\n當 Claude Code 需要您的操作時，會透過這裡通知您。",
        }))
        .send()?
        .json()?;

    if resp.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        println!("✅ 測試訊息已發送，請檢查 Telegram！");
    } else {
        println!("❌ 測試訊息發送失敗");
    }

    // Step 6: Test buttons
    println!("\n正在測試按鈕功能...");
    let resp: serde_json::Value = client
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": "🧪 按鈕測試\n\n請按任一按鈕確認功能正常：",
            "reply_markup": {
                "inline_keyboard": [[
                    {"text": "✅ 允許", "callback_data": "test_allow"},
                    {"text": "❌ 拒絕", "callback_data": "test_deny"},
                ]]
            },
        }))
        .send()?
        .json()?;

    if resp.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        println!("✅ 按鈕測試訊息已發送！");
    } else {
        println!("❌ 按鈕測試失敗");
    }

    println!("\n{}", "=".repeat(40));
    println!("🎉 設定完成！");
    println!();
    println!("下一步：");
    println!("  執行 claude-telegram-bridge install 來安裝背景服務");
    println!();
    println!("管理：");
    println!("  - 設定檔: {}", Config::config_path().display());
    println!("  - 暫時停用: 將設定檔中 \"disabled\" 改為 true");
    println!("  - 調整逾時: 修改設定檔中 \"permission_timeout\"");

    Ok(())
}
