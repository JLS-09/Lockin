#!/usr/bin/env bash
# Installs a .desktop file and the app icon for your user, so KDE Plasma shows
# the Lockin icon in the task bar while you run `pnpm tauri dev`.
#
# Why: on Wayland the window icon is ignored. KDE looks for a .desktop file
# named after the window's app id (io.github.jls_09.lockin, set in
# app/src-tauri/src/lib.rs) and shows that file's icon.
# A packaged install (later) puts these files in place itself.
#
# Usage: tools/install-desktop-entry.sh           install or update
#        tools/install-desktop-entry.sh --remove  remove again
set -euo pipefail
cd "$(dirname "$0")/.."

app_id=io.github.jls_09.lockin
data="${XDG_DATA_HOME:-$HOME/.local/share}"
desktop_file="$data/applications/$app_id.desktop"
icon_file="$data/icons/hicolor/scalable/apps/$app_id.svg"

if [[ "${1:-}" == "--remove" ]]; then
  rm -f "$desktop_file" "$icon_file"
  echo "Removed $desktop_file and $icon_file"
else
  install -Dm644 assets/logo/svg/lockin-icon-dark.svg "$icon_file"
  mkdir -p "$(dirname "$desktop_file")"
  # NoDisplay: it doesn't show up in the app menu, since Exec points at the
  # dev build, which needs `pnpm tauri dev` running to show anything.
  cat > "$desktop_file" <<EOF
[Desktop Entry]
Type=Application
Name=Lockin
Comment=Class tracker and study sessions
Exec=$PWD/target/debug/lockin-app
Icon=$app_id
StartupWMClass=$app_id
Categories=Education;
NoDisplay=true
EOF
  echo "Installed $desktop_file and $icon_file"
fi

# Let KDE pick up the change right away.
if command -v kbuildsycoca6 > /dev/null; then
  kbuildsycoca6 > /dev/null 2>&1 || true
fi
