use crate::config::Config;
use crate::types::{Decision, HookRequest, HookResponse, HookType};
use std::io::Read;
use uuid::Uuid;

/// Run the hook thin client.
/// Reads JSON from stdin, POSTs to daemon, writes response to stdout.
pub fn run(hook_type: HookType) {
    // Load config to get daemon port
    let config = match Config::load() {
        Some(c) => c,
        None => {
            // No config — silently exit (graceful degradation)
            std::process::exit(0);
        }
    };

    if config.disabled {
        std::process::exit(0);
    }

    // Read stdin
    let mut input = String::new();
    let payload: serde_json::Value = match std::io::stdin().read_to_string(&mut input) {
        Ok(_) => serde_json::from_str(&input).unwrap_or(serde_json::json!({})),
        Err(_) => serde_json::json!({}),
    };

    let request_id = Uuid::new_v4().to_string()[..8].to_string();

    let hook_req = HookRequest {
        request_id: request_id.clone(),
        hook_type,
        payload,
    };

    let endpoint = match hook_type {
        HookType::Permission => "permission",
        HookType::Notification => "notification",
        HookType::Stop => "stop",
    };

    let url = format!("http://127.0.0.1:{}/hook/{}", config.daemon_port, endpoint);

    // Use blocking reqwest for the thin client
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(config.permission_timeout + 30))
        .build()
        .unwrap_or_else(|_| {
            // Can't build client — silently exit
            std::process::exit(0);
        });

    let response = match client.post(&url).json(&hook_req).send() {
        Ok(r) => r,
        Err(_) => {
            // Daemon not running — silently exit (graceful degradation)
            std::process::exit(0);
        }
    };

    match hook_type {
        HookType::Permission => {
            let hook_resp: HookResponse = match response.json() {
                Ok(r) => r,
                Err(_) => std::process::exit(0),
            };

            match hook_resp.decision {
                Decision::Allow => {
                    let output = serde_json::json!({
                        "hookSpecificOutput": {
                            "hookEventName": "PermissionRequest",
                            "decision": { "behavior": "allow" },
                        }
                    });
                    println!("{}", serde_json::to_string(&output).unwrap());
                }
                Decision::Deny => {
                    let output = serde_json::json!({
                        "hookSpecificOutput": {
                            "hookEventName": "PermissionRequest",
                            "decision": {
                                "behavior": "deny",
                                "message": "使用者透過 Telegram 拒絕了此操作",
                            },
                        }
                    });
                    println!("{}", serde_json::to_string(&output).unwrap());
                }
                Decision::Timeout => {
                    eprintln!("Permission request timed out — denied by default");
                    std::process::exit(2);
                }
            }
        }
        HookType::Notification | HookType::Stop => {
            // Fire-and-forget: just exit 0
        }
    }
}
