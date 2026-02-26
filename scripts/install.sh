#!/bin/bash
set -euo pipefail

# Claude Telegram Bridge installer
# Usage: curl -fsSL https://raw.githubusercontent.com/alan890104/claude-telegram-hook/main/scripts/install.sh | bash

REPO="alan890104/claude-telegram-hook"
INSTALL_DIR="${HOME}/.local/bin"
BINARY_NAME="claude-telegram-bridge"

detect_platform() {
    local os arch

    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Darwin) os="darwin" ;;
        Linux)  os="linux" ;;
        MINGW*|MSYS*|CYGWIN*)
            echo "Error: Windows is not supported by this script."
            echo "Please download the binary manually from:"
            echo "  https://github.com/${REPO}/releases"
            exit 1
            ;;
        *)
            echo "Error: Unsupported operating system: $os"
            exit 1
            ;;
    esac

    case "$arch" in
        x86_64|amd64)  arch="amd64" ;;
        aarch64|arm64) arch="arm64" ;;
        *)
            echo "Error: Unsupported architecture: $arch"
            exit 1
            ;;
    esac

    echo "${BINARY_NAME}-${os}-${arch}"
}

get_latest_version() {
    local response
    response="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null)" || {
        echo ""
        return
    }
    echo "$response" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/'
}

main() {
    echo "=== Claude Telegram Bridge Installer ==="
    echo

    local asset_name
    asset_name="$(detect_platform)"
    echo "Platform: ${asset_name}"

    echo "Fetching latest version..."
    local version
    version="$(get_latest_version)"
    if [ -z "$version" ]; then
        echo "Error: Could not fetch latest version."
        echo "Please check that a GitHub Release has been published at:"
        echo "  https://github.com/${REPO}/releases"
        exit 1
    fi
    echo "Version: ${version}"

    local download_url="https://github.com/${REPO}/releases/download/${version}/${asset_name}"
    echo "Downloading ${download_url}..."

    local tmp_file
    tmp_file="$(mktemp)"
    if ! curl -fsSL -o "$tmp_file" "$download_url"; then
        echo "Error: Download failed."
        echo "Please check the release page: https://github.com/${REPO}/releases"
        rm -f "$tmp_file"
        exit 1
    fi

    mkdir -p "$INSTALL_DIR"
    mv "$tmp_file" "${INSTALL_DIR}/${BINARY_NAME}"
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

    echo
    echo "Installed to ${INSTALL_DIR}/${BINARY_NAME}"

    if ! echo "$PATH" | tr ':' '\n' | grep -q "^${INSTALL_DIR}$"; then
        echo
        echo "Warning: ${INSTALL_DIR} is not in your PATH."
        echo "Add the following line to your shell config (~/.zshrc or ~/.bashrc):"
        echo
        echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
        echo
    fi

    echo
    echo "Next steps:"
    echo "  1. ${BINARY_NAME} setup     # Configure your Telegram Bot"
    echo "  2. ${BINARY_NAME} install   # Install the background service"
}

main
