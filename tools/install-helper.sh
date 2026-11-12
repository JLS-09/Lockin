#!/usr/bin/env bash
# One-time setup for blocking websites. Only root may change /etc/hosts, so the
# daemon asks lockin-helper to do it, through polkit. This script installs:
#
#   /usr/local/lib/lockin/lockin-helper                       the helper (root-owned)
#   /usr/share/polkit-1/actions/io.github.jls_09.lockin.policy the polkit action
#   /etc/polkit-1/rules.d/50-lockin.rules                     lets you run it without a password
#
# Run it as yourself (it uses sudo where needed), and again after pulling changes.
#
# Usage: tools/install-helper.sh           build and install (or update)
#        tools/install-helper.sh --remove  unblock everything and remove it again
set -euo pipefail
cd "$(dirname "$0")/.."

helper=/usr/local/lib/lockin/lockin-helper
action=/usr/share/polkit-1/actions/io.github.jls_09.lockin.policy
rule=/etc/polkit-1/rules.d/50-lockin.rules

if [[ $EUID -eq 0 ]]; then
  echo "Run this as yourself, not with sudo: the polkit rule is made for your user." >&2
  exit 1
fi
user="$(id -un)"
if [[ ! "$user" =~ ^[a-z_][a-z0-9_-]*$ ]]; then
  echo "Unexpected user name: $user" >&2
  exit 1
fi

if [[ "${1:-}" == "--remove" ]]; then
  # Take Lockin's websites out of /etc/hosts first, so nothing stays blocked.
  if [[ -x "$helper" ]]; then sudo "$helper" unblock || true; fi
  sudo rm -f "$helper" "$action" "$rule"
  sudo rmdir /usr/local/lib/lockin 2> /dev/null || true
  echo "Removed lockin-helper and its polkit files. Websites are unblocked."
  exit 0
fi

cargo build --release -p lockin-helper

sudo install -D -m 755 -o root -g root target/release/lockin-helper "$helper"
sudo install -D -m 644 -o root -g root tools/polkit/io.github.jls_09.lockin.policy "$action"
sed "s/@USER@/$user/g" tools/polkit/50-lockin.rules | sudo tee "$rule" > /dev/null
sudo chmod 644 "$rule"

# Check: this must work without asking for a password.
if pkexec "$helper" show > /dev/null; then
  echo "Installed. Websites on your block profiles are blocked while you study."
else
  echo "Installed, but pkexec couldn't run the helper. Is polkit running? (systemctl status polkit)" >&2
  exit 1
fi
