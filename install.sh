#!/usr/bin/env sh
# Installs the latest a11y-report release for Linux or macOS.
#   curl -fsSL https://raw.githubusercontent.com/clemPerrousset/a11y-report/main/install.sh | sh
# Options (environment variables):
#   A11Y_REPORT_VERSION=v0.1.0          install a specific version (default: latest)
#   A11Y_REPORT_INSTALL_DIR=~/.local/bin where to put the binary
set -eu

REPO="clemPerrousset/a11y-report"
BIN="a11y-report"
VERSION="${A11Y_REPORT_VERSION:-latest}"
INSTALL_DIR="${A11Y_REPORT_INSTALL_DIR:-$HOME/.local/bin}"

os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Linux) os_part="unknown-linux-gnu" ;;
  Darwin) os_part="apple-darwin" ;;
  *) echo "Unsupported OS: $os (on Windows use install.ps1)" >&2; exit 1 ;;
esac
case "$arch" in
  x86_64 | amd64) arch_part="x86_64" ;;
  arm64 | aarch64) arch_part="aarch64" ;;
  *) echo "Unsupported architecture: $arch" >&2; exit 1 ;;
esac
target="$arch_part-$os_part"
asset="$BIN-$target.tar.gz"

if [ "$VERSION" = "latest" ]; then
  url="https://github.com/$REPO/releases/latest/download/$asset"
else
  url="https://github.com/$REPO/releases/download/$VERSION/$asset"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
echo "Downloading $url"
curl -fsSL "$url" -o "$tmp/$asset"
tar -xzf "$tmp/$asset" -C "$tmp"
mkdir -p "$INSTALL_DIR"
install -m 755 "$tmp/$BIN-$target/$BIN" "$INSTALL_DIR/$BIN"
echo "Installed $INSTALL_DIR/$BIN"
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) echo "Add $INSTALL_DIR to your PATH, e.g.: echo 'export PATH=\"$INSTALL_DIR:\$PATH\"' >> ~/.profile" ;;
esac
"$INSTALL_DIR/$BIN" --version || true
