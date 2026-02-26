use anyhow::{Context, Result};
use std::path::PathBuf;

/// Run the install subcommand: installs launchd/systemd service and merges Claude Code settings.
pub fn run() -> Result<()> {
    let binary_path = std::env::current_exe()
        .context("Failed to determine current executable path")?;

    println!("=== Claude Telegram Bridge Service Install ===\n");

    // Detect platform
    let platform = std::env::consts::OS;
    match platform {
        "macos" => install_launchd(&binary_path)?,
        "linux" => install_systemd(&binary_path)?,
        _ => {
            println!("Error: Unsupported platform: {}", platform);
            println!("Please start manually: {} daemon", binary_path.display());
            return Ok(());
        }
    }

    // Merge into Claude Code settings.json
    merge_claude_settings(&binary_path)?;

    println!("\n{}", "=".repeat(40));
    println!("Install complete!");
    println!();
    println!("Background service started and Claude Code hooks configured.");

    Ok(())
}

fn install_launchd(binary_path: &PathBuf) -> Result<()> {
    let home = dirs::home_dir().context("Cannot determine home directory")?;
    let plist_dir = home.join("Library/LaunchAgents");
    std::fs::create_dir_all(&plist_dir)?;

    let plist_path = plist_dir.join("com.claude-telegram-bridge.plist");
    let log_dir = home.join("Library/Logs");
    std::fs::create_dir_all(&log_dir)?;

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.claude-telegram-bridge</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{}/claude-telegram-bridge.log</string>
    <key>StandardErrorPath</key>
    <string>{}/claude-telegram-bridge.log</string>
</dict>
</plist>"#,
        binary_path.display(),
        log_dir.display(),
        log_dir.display(),
    );

    std::fs::write(&plist_path, &plist_content)
        .with_context(|| format!("Failed to write {}", plist_path.display()))?;

    println!("LaunchAgent plist written: {}", plist_path.display());

    // Unload existing (ignore errors if not loaded)
    let _ = std::process::Command::new("launchctl")
        .args(["unload", &plist_path.to_string_lossy()])
        .output();

    // Load the service
    let output = std::process::Command::new("launchctl")
        .args(["load", &plist_path.to_string_lossy()])
        .output()
        .context("Failed to run launchctl load")?;

    if output.status.success() {
        println!("LaunchAgent loaded and started");
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("Warning: launchctl load reported: {}", stderr.trim());
    }

    Ok(())
}

fn install_systemd(binary_path: &PathBuf) -> Result<()> {
    let home = dirs::home_dir().context("Cannot determine home directory")?;
    let unit_dir = home.join(".config/systemd/user");
    std::fs::create_dir_all(&unit_dir)?;

    let unit_path = unit_dir.join("claude-telegram-bridge.service");

    let unit_content = format!(
        r#"[Unit]
Description=Claude Telegram Bridge
After=network-online.target

[Service]
ExecStart={} daemon
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
"#,
        binary_path.display(),
    );

    std::fs::write(&unit_path, &unit_content)
        .with_context(|| format!("Failed to write {}", unit_path.display()))?;

    println!("systemd unit written: {}", unit_path.display());

    // Enable and start
    let output = std::process::Command::new("systemctl")
        .args(["--user", "enable", "--now", "claude-telegram-bridge"])
        .output()
        .context("Failed to run systemctl")?;

    if output.status.success() {
        println!("systemd service enabled and started");
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("Warning: systemctl reported: {}", stderr.trim());
    }

    Ok(())
}

fn merge_claude_settings(binary_path: &PathBuf) -> Result<()> {
    let home = dirs::home_dir().context("Cannot determine home directory")?;
    let settings_path = home.join(".claude/settings.json");

    // Read existing settings
    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)
            .with_context(|| format!("Failed to read {}", settings_path.display()))?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let bin = binary_path.to_string_lossy();

    // Build hook entries
    let hook_entry = |subcommand: &str| -> serde_json::Value {
        serde_json::json!([{
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": format!("{} hook {}", bin, subcommand),
            }]
        }])
    };

    // Merge hooks
    let hooks = settings
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert(serde_json::json!({}));

    let hooks_obj = hooks.as_object_mut().unwrap();
    hooks_obj.insert(
        "PermissionRequest".to_string(),
        hook_entry("permission"),
    );
    hooks_obj.insert(
        "Notification".to_string(),
        hook_entry("notification"),
    );
    hooks_obj.insert("Stop".to_string(), hook_entry("stop"));

    // Write back
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(&settings)?;
    std::fs::write(&settings_path, content)
        .with_context(|| format!("Failed to write {}", settings_path.display()))?;

    println!("Claude Code settings.json updated: {}", settings_path.display());

    Ok(())
}
