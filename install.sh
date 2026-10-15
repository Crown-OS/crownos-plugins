#!/bin/sh
# Release build, installed under /usr. Run as root.
set -eu
cargo build --release -p crownplugind -p crownplugin -p crownplugin-sysassets
install -Dm755 target/release/crownplugind /usr/bin/crownplugind
install -Dm755 target/release/crownplugin /usr/bin/crownplugin
install -Dm755 target/release/crownplugin-sysassets /usr/lib/crownos/crownplugin-sysassets
install -Dm644 data/crownplugind.service /usr/lib/systemd/user/crownplugind.service
install -Dm644 data/crownplugind.socket /usr/lib/systemd/user/crownplugind.socket
install -Dm644 data/org.crownos.plugins.sysassets.policy /usr/share/polkit-1/actions/org.crownos.plugins.sysassets.policy
install -d /usr/share/crownos/themes /usr/share/crownos/plugins /usr/share/crownos/theme-assets
echo "now: systemctl --user enable --now crownplugind.socket"
