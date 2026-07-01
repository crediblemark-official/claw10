#!/usr/bin/env bash
# Claw10 OS uninstaller (Linux, macOS, WSL)
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/crediblemark-official/claw10/master/uninstall.sh | sh

set -e

BINARY="claw10"
INSTALL_DIR="${CLAW10_INSTALL_DIR:-$HOME/.local/bin}"
CONFIG_DIR="$HOME/.claw10"

echo "Claw10 OS Uninstaller"
echo "========================"
echo ""
echo "This will remove:"
echo "  - Binary: $INSTALL_DIR/$BINARY"
echo "  - Config & data: $CONFIG_DIR"
echo "  - PATH entry from shell config (if added by installer)"
echo ""

# Non-interactive mode: set CLAW10_UNINSTALL_FORCE=1 to skip confirmation
if [ -z "$CLAW10_UNINSTALL_FORCE" ]; then
    printf "Are you sure you want to uninstall Claw10? [y/N] "
    # Jika stdin bukan tty (misalnya di-pipe dari curl), baca dari /dev/tty
    if [ -t 0 ]; then
        read -r response
    else
        read -r response < /dev/tty 2>/dev/null || response="n"
    fi
    case "$response" in
        [yY][eE][sS]|[yY]) ;;
        *) echo "Uninstall cancelled."; exit 0 ;;
    esac
fi

# Remove binary
if [ -f "$INSTALL_DIR/$BINARY" ]; then
    rm -f "$INSTALL_DIR/$BINARY"
    echo "Removed: $INSTALL_DIR/$BINARY"
else
    echo "Binary not found: $INSTALL_DIR/$BINARY"
fi

# Remove config and data directory
if [ -d "$CONFIG_DIR" ]; then
    rm -rf "$CONFIG_DIR"
    echo "Removed: $CONFIG_DIR"
else
    echo "Config directory not found: $CONFIG_DIR"
fi

# Remove PATH export from shell config files
SHELL_NAME=$(basename "$SHELL")
case "$SHELL_NAME" in
    bash)   RC_FILES=("$HOME/.bashrc") ;;
    zsh)    RC_FILES=("$HOME/.zshrc") ;;
    fish)   RC_FILES=("$HOME/.config/fish/config.fish") ;;
    *)      RC_FILES=("$HOME/.bashrc" "$HOME/.zshrc") ;;
esac

for rc in "${RC_FILES[@]}"; do
    if [ -f "$rc" ] && grep -q "export PATH=\"$INSTALL_DIR:\\$PATH\"" "$rc" 2>/dev/null; then
        sed -i.bak "/export PATH=\"$INSTALL_DIR:\\\$PATH\"/d" "$rc"
        rm -f "$rc.bak"
        echo "Updated: $rc"
    fi
done

echo ""
echo "Claw10 has been uninstalled."
echo "You may need to restart your terminal for PATH changes to take effect."
