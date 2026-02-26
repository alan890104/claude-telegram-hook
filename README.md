<div align="center">

# Claude Telegram Bridge

**Approve Claude Code from your phone.**

[![GitHub Release](https://img.shields.io/github/v/release/alan890104/claude-telegram-hook?style=flat-square&logo=github&color=blue)](https://github.com/alan890104/claude-telegram-hook/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-green?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey?style=flat-square)]()
[![Telegram Bot API](https://img.shields.io/badge/Telegram-Bot%20API-26A5E4?style=flat-square&logo=telegram)](https://core.telegram.org/bots/api)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-dea584?style=flat-square&logo=rust)](https://www.rust-lang.org)

**[English](README.md)** | [繁體中文](docs/README.zh-TW.md) | [简体中文](docs/README.zh-CN.md) | [日本語](docs/README.ja.md) | [한국어](docs/README.ko.md) | [Русский](docs/README.ru.md)

</div>

---

When Claude Code needs permission to run a tool — execute a shell command, write a file, anything — you get a Telegram message with **Allow / Deny** buttons. Tap from your couch, a coffee shop, or another room. No need to stay at the terminal.

You also get notified when Claude asks a question or finishes a task.

## Install

**macOS / Linux:**

```bash
curl -fsSL https://raw.githubusercontent.com/alan890104/claude-telegram-hook/main/scripts/install.sh | bash
```

**Manual download:** grab the binary for your platform from [Releases](https://github.com/alan890104/claude-telegram-hook/releases).

| Platform | File |
|---|---|
| macOS (Apple Silicon) | `claude-telegram-bridge-darwin-arm64` |
| macOS (Intel) | `claude-telegram-bridge-darwin-amd64` |
| Linux x86_64 | `claude-telegram-bridge-linux-amd64` |
| Linux ARM64 | `claude-telegram-bridge-linux-arm64` |
| Windows x86_64 | `claude-telegram-bridge-windows-amd64.exe` |

<details>
<summary>Build from source</summary>

```bash
cargo build --release
cp target/release/claude-telegram-bridge ~/.local/bin/
```
</details>

## Getting Started

**1. Setup** — create a Telegram bot and link it:

```bash
claude-telegram-bridge setup
```

The wizard handles everything: bot creation via [@BotFather](https://t.me/BotFather), chat ID detection, permission timeout, and a test message.

**2. Install the service** — register the background daemon and configure Claude Code:

```bash
claude-telegram-bridge install
```

Done. Open Claude Code and it just works.

## How It Works

```
You (Telegram)          Daemon                    Claude Code
     │                    │                          │
     │              ┌─────┴──────┐                   │
     │              │ HTTP Server │◄── hook thin ────┤ needs permission
     │              │ :19876      │    client POST    │
     │              └─────┬──────┘                   │
     │                    │                          │
     │◄── sends message ──┤                          │
     │   [Allow] [Deny]   │                          │
     │                    │                          │
     ├── taps Allow ─────►│                          │
     │                    ├── returns decision ─────►│ proceeds
     │                    │                          │
```

A single daemon process owns the Telegram connection. Each Claude Code session talks to the daemon over localhost HTTP. Button presses are routed to the correct session via unique request IDs.

**Why a daemon?** The old approach spawned a new process per hook. Multiple Claude Code sessions would fight over Telegram's `getUpdates`, causing buttons to break. One daemon, one connection, zero conflicts.

## Configuration

`~/.claude/hooks/telegram_config.json`

```json
{
  "bot_token": "123456:ABC-DEF...",
  "chat_id": "987654321",
  "permission_timeout": 300,
  "disabled": false,
  "daemon_port": 19876
}
```

| Field | Default | Description |
|---|---|---|
| `bot_token` | — | Telegram Bot API token |
| `chat_id` | — | Your Telegram chat ID |
| `permission_timeout` | `300` | Seconds before auto-deny |
| `disabled` | `false` | Pause without uninstalling |
| `daemon_port` | `19876` | Localhost port for hook ↔ daemon |

Environment variable fallback: `TELEGRAM_BOT_TOKEN`, `TELEGRAM_CHAT_ID`

## Key Behaviors

| Scenario | What happens |
|---|---|
| You tap **Allow** | Claude Code proceeds |
| You tap **Deny** | Claude Code is told the user refused |
| You don't respond (timeout) | Permission **denied** — safe default |
| Daemon not running | Hook exits silently, Claude falls back to terminal prompt |
| Stale button pressed | Telegram shows "expired" — no effect |
| Multiple sessions | Each gets its own buttons, no interference |

## System Tray

- **Green** — running
- **Orange** — pending requests
- Menu: status, pending count, open config, quit

## Troubleshooting

```bash
# Check daemon health
curl http://127.0.0.1:19876/health

# Run with debug logging
RUST_LOG=debug claude-telegram-bridge daemon

# macOS: restart service
launchctl unload ~/Library/LaunchAgents/com.claude-telegram-bridge.plist
launchctl load ~/Library/LaunchAgents/com.claude-telegram-bridge.plist
tail -f ~/Library/Logs/claude-telegram-bridge.log

# Linux: restart service
systemctl --user restart claude-telegram-bridge
journalctl --user -u claude-telegram-bridge -f
```

## Security

- Hook traffic stays on `127.0.0.1` — never exposed to the network
- Chat ID verified on every callback
- UUID request IDs prevent stale button replay
- All Telegram text is HTML-escaped

## License

MIT
