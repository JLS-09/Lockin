#!/usr/bin/env bash
# Builds the daemon and installs it as a systemd user service (lockin.service),
# so study sessions start on time and the feeds sync every hour, also when the
# app is closed. Run it again after pulling changes to update the daemon.
#
# Installs:  ~/.local/bin/lockin-daemon               (also handy as a command)
#            ~/.config/systemd/user/lockin.service
#
# Usage: tools/install-daemon.sh           build, install or update, and (re)start
#        tools/install-daemon.sh --remove  stop and remove again (your data stays)
#
# Logs:  journalctl --user -u lockin -f
set -euo pipefail
cd "$(dirname "$0")/.."

bin_dir="$HOME/.local/bin"
unit_dir="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
binary="$bin_dir/lockin-daemon"
unit="$unit_dir/lockin.service"

if [[ "${1:-}" == "--remove" ]]; then
  systemctl --user disable --now lockin.service 2> /dev/null || true
  rm -f "$unit" "$binary"
  systemctl --user daemon-reload
  echo "Removed lockin.service and $binary. Your data in ~/.local/share/lockin stays."
  exit 0
fi

cargo build --release -p lockin-daemon

# Stop a running daemon first, so the binary can be replaced.
systemctl --user stop lockin.service 2> /dev/null || true
install -Dm755 target/release/lockin-daemon "$binary"
install -Dm644 tools/lockin.service "$unit"

systemctl --user daemon-reload
systemctl --user enable --now lockin.service
echo
systemctl --user --no-pager status lockin.service | head -n 5
echo
echo "Installed. Logs: journalctl --user -u lockin -f"
if [[ ":$PATH:" != *":$bin_dir:"* ]]; then
  echo "Note: $bin_dir is not in your PATH, so run the CLI as $binary."
fi
