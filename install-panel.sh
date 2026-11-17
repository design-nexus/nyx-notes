#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
share=${XDG_DATA_HOME:-$HOME/.local/share}
config=${XDG_CONFIG_HOME:-$HOME/.config}
if [[ ${NYX_SKIP_BUILD:-0} != 1 ]]; then
 cargo build --release --locked --manifest-path "$root/Cargo.toml" -p nyx-notes-panel
fi
binary=${CARGO_TARGET_DIR:-$root/target}/release/nyx-notes-panel
install -Dm755 "$binary" "$HOME/.local/bin/nyx-notes-panel.new"
mv -f "$HOME/.local/bin/nyx-notes-panel.new" "$HOME/.local/bin/nyx-notes-panel"
mkdir -p "$share/nyx-notes-panel/qml"
cp -a "$root/qml/panel/." "$share/nyx-notes-panel/qml/"
install -Dm644 "$root/data/panel/io.github.design_nexus.nyx.NotesPanel.desktop" "$share/applications/io.github.design_nexus.nyx.NotesPanel.desktop"
install -Dm644 "$root/data/icons/io.github.design_nexus.nyx.Notes.svg" "$share/icons/hicolor/scalable/apps/io.github.design_nexus.nyx.Notes.svg"
# Shared Nyx runtime discovers this optional component; no NyxOS-only service required.
mkdir -p "$share/nyx-shell/components"
ln -sfn "$share/nyx-notes-panel/qml" "$share/nyx-shell/components/nyx-notes-panel"
# Omarchy loads its own host adapter; retain every existing plugin's files.
if [[ -d ${OMARCHY_PATH:-/usr/share/omarchy}/shell || -f $config/omarchy/shell.json ]]; then
 mkdir -p "$config/omarchy/plugins/design-nexus.notes"
 cp -a "$root/qml/panel/." "$config/omarchy/plugins/design-nexus.notes/"
fi
command -v update-desktop-database >/dev/null && update-desktop-database "$share/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -f -t -q "$share/icons/hicolor" || true
printf 'Installed Notes Quick Panel without opening or restarting it.\n'
