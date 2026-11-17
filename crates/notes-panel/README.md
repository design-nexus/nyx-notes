# Notes Quick Panel

Quickshell implementation of the original selected Notes browser mockup. `nyx-notes-panel` opens/toggles the panel. `--stdio` is the private interactive UI companion, not a public automation tool or a new synchronization service.

The companion is owned by Notes and uses the public `notes-vault` backend for conditional saves, Markdown fidelity, history, recovery and the existing per-vault search index. It reads the same active vault and pin preferences as Notes. It creates no separate notes database or sync mechanism. The existing proposal-gated external Notes Control API is unchanged.

Pinned/recent notes, full-text search, quick creation, source editing, autosave, recoverable drafts, opening in full Notes, and keyboard shortcuts are provided. Existing note titles identify their filenames and are read-only in the compact editor; renaming stays in the full app. New notes use no-clobber creation with collision suffixes. Large files and symlink writes are rejected. A dirty full-app recovery draft prevents panel editing until reviewed in Notes.

Saves write a recovery draft before etag/digest-checked replacement, and reuse Notes history retention preferences. Panel recovery drafts live under `nyx-notes/quick-panel/recovery`, outside the vault, so they cannot overwrite the full app's own recovery draft. External edits produce a conflict with the panel draft retained; Retry and Save a copy are explicit actions. Restored drafts require review. A save reply acknowledges the exact submitted text, so typing during an in-flight save is not overwritten.

`./install-panel.sh` at the repository root installs the binary, launcher, icon, Omarchy adapter (`design-nexus.notes`), and Nyx Shell component without opening/restarting Notes. Both hosts share `qml/panel/NotesPanel.qml`. Theme tokens follow the live system snapshot using the public Nyx Ui/Theme modules recorded in `qml/panel/nyx/vendor.json`.

Ctrl+N captures; Ctrl+F opens search after a successful save; Escape returns to browsing or dismisses. Tab/Enter operate the shared focusable rows and buttons. Opening full Notes is always an explicit user action. No note content is placed in command arguments, logs, notifications, or shell commands.

Validation includes a scratch-vault private-RPC round trip (pins/search, Unicode capture, autosave, external-edit conflict, recovery, path boundaries), a focused conflict test and offscreen QML render. Live popup focus/placement and physical typing remain for a user-session check; installation does not launch the apps.
