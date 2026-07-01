#!/usr/bin/env bash
# Claw10 OS updater (Linux, macOS, WSL)
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/crediblemark-official/claw10/master/update.sh | sh
#
# This script downloads and runs the latest install.sh, preserving your
# existing ~/.claw10 configuration and data.

set -e

REPO="crediblemark-official/claw10"
INSTALL_SCRIPT="https://raw.githubusercontent.com/$REPO/master/install.sh"

echo "Claw10 OS Updater"
echo "===================="
echo ""

# Detect current version if binary exists
INSTALL_DIR="${CLAW10_INSTALL_DIR:-$HOME/.local/bin}"
BINARY="$INSTALL_DIR/claw10"
if [ -f "$BINARY" ]; then
    CURRENT_VERSION=$($BINARY --version 2>/dev/null || $BINARY version 2>/dev/null || echo "unknown")
    echo "Current version: $CURRENT_VERSION"
else
    echo "Claw10 is not currently installed."
    echo "Run the installer instead:"
    echo "  curl -fsSL $INSTALL_SCRIPT | sh"
    exit 1
fi

echo "Checking for updates..."
echo ""

# Download and run the latest installer
if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$INSTALL_SCRIPT" | sh
elif command -v wget >/dev/null 2>&1; then
    wget -qO- "$INSTALL_SCRIPT" | sh
else
    echo "Error: curl or wget is required to download the installer."
    exit 1
fi

echo ""
if [ -f "$BINARY" ]; then
    NEW_VERSION=$($BINARY --version 2>/dev/null || $BINARY version 2>/dev/null || echo "unknown")
    echo "Updated to: $NEW_VERSION"
fi
echo "Your config and data in ~/.claw10 have been preserved."
