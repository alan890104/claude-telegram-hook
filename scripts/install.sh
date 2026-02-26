#!/bin/bash
set -euo pipefail

# Claude Telegram Bridge installer
# Usage: curl -fsSL https://raw.githubusercontent.com/alan890104/claude-telegram-hook/main/scripts/install.sh | bash

REPO="alan890104/claude-telegram-hook"
INSTALL_DIR="${HOME}/.local/bin"
BINARY_NAME="claude-telegram-bridge"

# Detect OS and architecture
detect_platform() {
    local os arch

    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Darwin) os="darwin" ;;
        Linux)  os="linux" ;;
        MINGW*|MSYS*|CYGWIN*) os="windows" ;;
        *)
            echo "❌ 不支援的作業系統: $os"
            exit 1
            ;;
    esac

    case "$arch" in
        x86_64|amd64)  arch="amd64" ;;
        aarch64|arm64) arch="arm64" ;;
        *)
            echo "❌ 不支援的架構: $arch"
            exit 1
            ;;
    esac

    local suffix=""
    if [ "$os" = "windows" ]; then
        suffix=".exe"
    fi

    echo "${BINARY_NAME}-${os}-${arch}${suffix}"
}

# Get latest release tag
get_latest_version() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | head -1 \
        | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/'
}

main() {
    echo "=== Claude Telegram Bridge 安裝程式 ==="
    echo

    # Detect platform
    local asset_name
    asset_name="$(detect_platform)"
    echo "平台: ${asset_name}"

    # Get latest version
    echo "正在取得最新版本..."
    local version
    version="$(get_latest_version)"
    if [ -z "$version" ]; then
        echo "❌ 無法取得最新版本，請確認 GitHub Release 已發布"
        exit 1
    fi
    echo "版本: ${version}"

    # Download
    local download_url="https://github.com/${REPO}/releases/download/${version}/${asset_name}"
    echo "正在下載 ${download_url}..."

    local tmp_file
    tmp_file="$(mktemp)"
    if ! curl -fsSL -o "$tmp_file" "$download_url"; then
        echo "❌ 下載失敗"
        rm -f "$tmp_file"
        exit 1
    fi

    # Install
    mkdir -p "$INSTALL_DIR"
    mv "$tmp_file" "${INSTALL_DIR}/${BINARY_NAME}"
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

    echo
    echo "✅ 已安裝到 ${INSTALL_DIR}/${BINARY_NAME}"

    # Check PATH
    if ! echo "$PATH" | tr ':' '\n' | grep -q "^${INSTALL_DIR}$"; then
        echo
        echo "⚠️  ${INSTALL_DIR} 不在你的 PATH 中"
        echo "   請加入以下設定到你的 shell 設定檔（~/.zshrc 或 ~/.bashrc）："
        echo
        echo "   export PATH=\"${INSTALL_DIR}:\$PATH\""
        echo
    fi

    echo
    echo "下一步："
    echo "  1. ${BINARY_NAME} setup     # 設定 Telegram Bot"
    echo "  2. ${BINARY_NAME} install   # 安裝背景服務"
}

main
