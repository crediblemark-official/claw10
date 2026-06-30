#!/usr/bin/env bash
# ClawHive OS — cross-platform installer (Linux, macOS, WSL, VPS)
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/crediblemark-official/clawhive/master/install.sh | sh
#   wget -qO- https://raw.githubusercontent.com/crediblemark-official/clawhive/master/install.sh | sh
#
# To use your own domain, replace the raw GitHub URL above and host this script there.

set -e

REPO="crediblemark-official/clawhive"
BINARY="clawhive"
INSTALL_DIR="${CLAWHIVE_INSTALL_DIR:-$HOME/.local/bin}"
CARGO_BUILD="${CLAWHIVE_CARGO_BUILD:-0}"

# Detect OS
OS=$(uname -s)
ARCH=$(uname -m)

case "$OS" in
    Linux*)     PLATFORM="linux";;
    Darwin*)    PLATFORM="macos";;
    MINGW*|MSYS*|CYGWIN*) PLATFORM="windows";;
    *)          PLATFORM="unknown";;
esac

case "$ARCH" in
    x86_64|amd64)   ARCH="x86_64";;
    aarch64|arm64)  ARCH="aarch64";;
    armv7*)         ARCH="armv7";;
    *)              ARCH="unknown";;
esac

if [ "$PLATFORM" = "windows" ]; then
    echo "Detected Windows. Please use the PowerShell installer instead:"
    echo '  irm https://raw.githubusercontent.com/crediblemark-official/clawhive/master/install.ps1 | iex'
    exit 1
fi

if [ "$ARCH" = "unknown" ]; then
    echo "Unsupported architecture: $(uname -m). Falling back to cargo build..."
    CARGO_BUILD=1
fi

echo "Installing ClawHive OS for $PLATFORM-$ARCH..."

# Create install directory
mkdir -p "$INSTALL_DIR"

# Prefer prebuilt binary from GitHub release, fallback to cargo build
if [ "$CARGO_BUILD" = "0" ]; then
    # Try to download latest release
    LATEST_URL="https://api.github.com/repos/$REPO/releases/latest"
    ASSET_NAME="${BINARY}-${PLATFORM}-${ARCH}.tar.gz"

    if command -v curl >/dev/null 2>&1; then
        DOWNLOAD_URL=$(curl -fsSL "$LATEST_URL" 2>/dev/null | grep -o '"browser_download_url": *"[^"]*' | grep "$ASSET_NAME" | head -n 1 | sed 's/.*"//') || true
    elif command -v wget >/dev/null 2>&1; then
        DOWNLOAD_URL=$(wget -qO- "$LATEST_URL" 2>/dev/null | grep -o '"browser_download_url": *"[^"]*' | grep "$ASSET_NAME" | head -n 1 | sed 's/.*"//') || true
    fi

    if [ -n "$DOWNLOAD_URL" ]; then
        TMP_DIR=$(mktemp -d)
        echo "Downloading $ASSET_NAME..."
        if command -v curl >/dev/null 2>&1; then
            curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/$ASSET_NAME"
        else
            wget -q "$DOWNLOAD_URL" -O "$TMP_DIR/$ASSET_NAME"
        fi
        tar -xzf "$TMP_DIR/$ASSET_NAME" -C "$TMP_DIR"
        install -m 755 "$TMP_DIR/$BINARY" "$INSTALL_DIR/$BINARY"
        rm -rf "$TMP_DIR"
    else
        echo "No prebuilt binary found. Falling back to cargo build (this may take a while)..."
        CARGO_BUILD=1
    fi
fi

if [ "$CARGO_BUILD" = "1" ]; then
    if ! command -v cargo >/dev/null 2>&1; then
        echo "Rust/Cargo is required but not installed. Install Rust first:"
        echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi

    TMP_DIR=$(mktemp -d)
    git clone --depth 1 "https://github.com/$REPO.git" "$TMP_DIR/clawhive" 2>/dev/null || {
        echo "Failed to clone repository. Make sure git is installed and internet is available."
        rm -rf "$TMP_DIR"
        exit 1
    }
    cargo build --release --manifest-path "$TMP_DIR/clawhive/Cargo.toml"
    install -m 755 "$TMP_DIR/clawhive/target/release/$BINARY" "$INSTALL_DIR/$BINARY"
    rm -rf "$TMP_DIR"
fi

# Add to PATH if not already present
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    SHELL_NAME=$(basename "$SHELL")
    case "$SHELL_NAME" in
        bash)   RC_FILE="$HOME/.bashrc";;
        zsh)    RC_FILE="$HOME/.zshrc";;
        fish)   RC_FILE="$HOME/.config/fish/config.fish";;
        *)      RC_FILE="";;
    esac

    if [ -n "$RC_FILE" ]; then
        mkdir -p "$(dirname "$RC_FILE")"
        echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$RC_FILE"
        echo "Added $INSTALL_DIR to PATH in $RC_FILE"
    else
        echo "Please add $INSTALL_DIR to your PATH manually."
    fi
fi

echo ""
echo "ClawHive OS installed to: $INSTALL_DIR/$BINARY"
echo "Run 'clawhive --help' to get started."
echo "Run 'clawhive setup' for initial configuration wizard."
