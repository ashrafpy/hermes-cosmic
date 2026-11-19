#!/usr/bin/env bash
set -euo pipefail

APP_ID="io.github.hermes.CosmicAppletHermes"
BIN_NAME="cosmic-applet-hermes"

BIN_DIR="${HOME}/.local/bin"
APPS_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/scalable/apps"

if pgrep -x cosmic-applet-h >/dev/null 2>&1; then
    echo "==> Stopping running $BIN_NAME instance..."
    pkill -x cosmic-applet-h || true
fi

rm -f "${BIN_DIR}/${BIN_NAME}"
rm -f "${APPS_DIR}/${APP_ID}.desktop"
rm -f "${ICON_DIR}/${APP_ID}-symbolic.svg"
rm -f "${ICON_DIR}/${APP_ID}-active-symbolic.svg"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APPS_DIR" >/dev/null 2>&1 || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "${HOME}/.local/share/icons/hicolor" >/dev/null 2>&1 || true
fi

echo "Uninstalled $BIN_NAME from ~/.local."
