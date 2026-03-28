#!/usr/bin/env bash
set -e

REPO="vizier-lab/vizier"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

info() { echo "→ $1"; }
error() { echo "✗ $1" >&2; exit 1; }

# Detect platform
case "$(uname -s)" in
    Linux*) OS="linux";;
    Darwin*) OS="macos";;
    *) error "Unsupported OS: $(uname -s)";;
esac

case "$(uname -m)" in
    x86_64|amd64) ARCH="x86_64";;
    arm64|aarch64) ARCH="aarch64";;
    *) error "Unsupported architecture: $(uname -m)";;
esac

case "$OS" in
    linux) TARGET="${ARCH}-unknown-linux-gnu";;
    macos) TARGET="${ARCH}-apple-darwin";;
esac

# Get version
VERSION="${VERSION:-$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"tag_name": "v?([^"]+)".*/\1/')}"
[ -z "$VERSION" ] && error "Failed to get latest version"
VERSION="${VERSION#v}"

info "Installing Vizier v$VERSION for $TARGET"

# Create install dir
mkdir -p "$INSTALL_DIR"

# Download
cd "$(mktemp -d)"
curl -fsSL "https://github.com/$REPO/releases/download/v$VERSION/vizier-v$VERSION-$TARGET.tar.gz" -o vizier.tar.gz || error "Download failed"
tar -xzf vizier.tar.gz
mv "vizier-v$VERSION-$TARGET/vizier" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/vizier"

# Add to PATH
add_to_path() {
    local shell_rc=""
    case "$SHELL" in
        */zsh) shell_rc="$HOME/.zshrc";;
        */bash) shell_rc="$HOME/.bashrc";;
        */fish) shell_rc="$HOME/.config/fish/config.fish";;
    esac

    if [ -n "$shell_rc" ] && ! grep -q "$INSTALL_DIR" "$shell_rc" 2>/dev/null; then
        echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$shell_rc"
        info "Added $INSTALL_DIR to PATH in $shell_rc"
        info "Run: source $shell_rc"
    fi
}

# Check if in PATH
case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) add_to_path;;
esac

info "Vizier installed to $INSTALL_DIR/vizier"
"$INSTALL_DIR/vizier" --version 2>/dev/null || true
