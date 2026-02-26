<div align="center">

# Claude Telegram Bridge

**스마트폰으로 Claude Code 제어.**

[![GitHub Release](https://img.shields.io/github/v/release/alan890104/claude-telegram-hook?style=flat-square&logo=github&color=blue)](https://github.com/alan890104/claude-telegram-hook/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-green?style=flat-square)](../LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey?style=flat-square)]()
[![Telegram Bot API](https://img.shields.io/badge/Telegram-Bot%20API-26A5E4?style=flat-square&logo=telegram)](https://core.telegram.org/bots/api)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-dea584?style=flat-square&logo=rust)](https://www.rust-lang.org)

[English](../README.md) | [繁體中文](README.zh-TW.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | **[한국어](README.ko.md)** | [Русский](README.ru.md)

</div>

---

Claude Code가 도구 실행 권한을 요청할 때 — 셸 명령어 실행, 파일 쓰기 등 — **허용 / 거부** 버튼이 있는 Telegram 메시지를 받습니다. 소파에서든, 카페에서든, 다른 방에서든 탭 한 번이면 됩니다. 터미널 앞에 있을 필요 없습니다.

Claude가 질문하거나 작업을 완료했을 때도 알림을 받습니다.

## 설치

**macOS / Linux:**

```bash
curl -fsSL https://raw.githubusercontent.com/alan890104/claude-telegram-hook/main/scripts/install.sh | bash
```

**수동 다운로드:** [Releases](https://github.com/alan890104/claude-telegram-hook/releases)에서 플랫폼에 맞는 바이너리를 다운로드하세요.

| 플랫폼 | 파일 |
|---|---|
| macOS (Apple Silicon) | `claude-telegram-bridge-darwin-arm64` |
| macOS (Intel) | `claude-telegram-bridge-darwin-amd64` |
| Linux x86_64 | `claude-telegram-bridge-linux-amd64` |
| Linux ARM64 | `claude-telegram-bridge-linux-arm64` |
| Windows x86_64 | `claude-telegram-bridge-windows-amd64.exe` |

<details>
<summary>소스에서 빌드</summary>

```bash
cargo build --release
cp target/release/claude-telegram-bridge ~/.local/bin/
```
</details>

## 시작하기

**1. 설정** — Telegram 봇을 만들고 연결:

```bash
claude-telegram-bridge setup
```

마법사가 모든 것을 처리합니다: [@BotFather](https://t.me/BotFather)로 봇 생성, chat ID 감지, 타임아웃 설정, 테스트 메시지 전송.

**2. 서비스 설치** — 백그라운드 데몬을 등록하고 Claude Code를 설정:

```bash
claude-telegram-bridge install
```

끝. Claude Code를 열면 바로 사용할 수 있습니다.

## 작동 방식

```
당신 (Telegram)           Daemon                    Claude Code
     │                      │                          │
     │                ┌─────┴──────┐                   │
     │                │ HTTP Server │◄── hook client ──┤ 권한 필요
     │                │ :19876      │    POST 요청      │
     │                └─────┬──────┘                   │
     │                      │                          │
     │◄── 메시지 전송 ──────┤                          │
     │   [허용] [거부]       │                          │
     │                      │                          │
     ├── 허용 탭 ───────────►│                          │
     │                      ├── 결정 반환 ─────────────►│ 처리 계속
     │                      │                          │
```

단일 데몬 프로세스가 Telegram 연결을 독점합니다. 각 Claude Code 세션은 localhost HTTP로 데몬과 통신합니다. 버튼 클릭은 고유한 요청 ID로 올바른 세션에 라우팅됩니다.

**왜 데몬인가?** 이전 방식은 hook 호출마다 새 프로세스를 생성했습니다. 여러 Claude Code 세션이 Telegram의 `getUpdates`를 서로 빼앗아 버튼이 작동하지 않았습니다. 데몬 하나, 연결 하나, 충돌 제로.

## 설정 파일

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

| 필드 | 기본값 | 설명 |
|---|---|---|
| `bot_token` | — | Telegram Bot API 토큰 |
| `chat_id` | — | 본인의 Telegram chat ID |
| `permission_timeout` | `300` | 자동 거부까지 대기 초 |
| `disabled` | `false` | 제거 없이 일시 정지 |
| `daemon_port` | `19876` | Hook ↔ 데몬 통신용 로컬 포트 |

환경 변수 대체: `TELEGRAM_BOT_TOKEN`, `TELEGRAM_CHAT_ID`

## 동작 일람

| 시나리오 | 결과 |
|---|---|
| **허용** 탭 | Claude Code 계속 진행 |
| **거부** 탭 | Claude Code에 사용자가 거부했다고 전달 |
| 응답 없음 (타임아웃) | 권한 **거부** — 안전한 기본값 |
| 데몬 미실행 | Hook이 조용히 종료, 터미널 프롬프트로 대체 |
| 만료된 버튼 클릭 | Telegram이 "만료됨" 표시 — 영향 없음 |
| 다중 세션 | 각각 독립 버튼, 간섭 없음 |

## 시스템 트레이

- **초록색** — 정상 작동
- **주황색** — 대기 중인 요청 있음
- 메뉴: 상태, 대기 수, 설정 파일 열기, 종료

## 문제 해결

```bash
# 데몬 상태 확인
curl http://127.0.0.1:19876/health

# 디버그 로그로 실행
RUST_LOG=debug claude-telegram-bridge daemon

# macOS: 서비스 재시작
launchctl unload ~/Library/LaunchAgents/com.claude-telegram-bridge.plist
launchctl load ~/Library/LaunchAgents/com.claude-telegram-bridge.plist
tail -f ~/Library/Logs/claude-telegram-bridge.log

# Linux: 서비스 재시작
systemctl --user restart claude-telegram-bridge
journalctl --user -u claude-telegram-bridge -f
```

## 보안

- Hook 트래픽은 `127.0.0.1`만 사용 — 네트워크에 노출되지 않음
- 모든 콜백에서 Chat ID 검증
- UUID 요청 ID로 만료된 버튼 재사용 방지
- 모든 Telegram 텍스트는 HTML 이스케이프 처리

## 라이선스

MIT
