# Notes integration contracts v1

All integrations are optional. Notes retains ordinary Markdown and JSON Canvas files, remains offline-capable, and starts with AI, semantic indexing, suite reads/writes, and Glimpse export disabled. Nothing uploads notes to a cloud provider automatically.

## Suite control

On its existing application bus/object path, Notes implements `io.github.design_nexus.nyx.Control`: `Describe() -> s` returns tool descriptions; `Call(s name, s arguments) -> s` returns JSON `{ok: ...}` or `{error: ...}`. These are the shared Nyx signatures, not a new service.

- `notes.vault_status`: permission/availability flags, without exposing note contents.
- `notes.vault_search {query}`: background FTS results from the active vault.
- `notes.vault_read {note}`: text, BOM flag, source version, and an untrusted-data marker; bounded to 128 KiB.
- `notes.vault_open {note}`: local note/heading navigation.
- `notes.vault_create {title,text}`: propose a new note.
- `notes.vault_append {note,text}`: propose an append.
- `notes.vault_replace {note,text,version}`: propose a full replacement against the read version.
- `notes.clip {title,url,text}`: propose supplied webpage text with preserved source URL/capture metadata; no automatic web fetch.
- `notes.vault_proposal_status {proposal_id}`: check a pending proposal.

Read and proposed-edit permissions are separate. Write requests return `{proposal_id,status:"awaiting_review",applied:false}` immediately. The user then accepts or rejects a native source/diff review. Poll status before claiming a change was saved. Conflicts retain editor drafts and require normal resolution; stale replacements/reviews are refused. Root/canonical-path checks, symlink read-only handling, `.obsidian` protection, and conditional saves remain in force. No delete/bulk-destructive tool is exposed.

The Assistant `nyx-apps` ability consumes this surface. Its policy marks vault reads/searches as private/untrusted and edit proposals as editing operations. Existing Assistant note-memory tools retain their separate names; vault tools use `notes.vault_*`.

## Local AI and semantic indexing

Notes uses Nyx AI Core (`nyx-aid`), the models this computer shares between apps, through `nyx-ai-client` on its private socket; the Assistant doesn't need to be running. Generation is a plain chat request with no tools, speech, web retrieval or conversation history. Notes supplies only the requested passage, or a bounded set of retrieved excerpts for vault questions. Generated edits always require a visible diff and preserve undo/conditional saves.

Local AI preferences choose the writing model (a chat model) and the embedding model from those AI Core has; each defaults to AI Core's model for the role (`chat`, then `assistant`; `embedding`). A model is never swapped for another: when the chosen one is gone, Notes says so. Nothing is downloaded or sent off the computer. The semantic index records which model file and vector settings made it, read from AI Core without loading the model; another embedding model, or settings that change its vectors, rebuild it.

The `notes-ai` provider trait separates transport from indexing. `sqlite-vec` supplies native vector lookup; model-profile-specific, chunked SQLite caches live outside the vault and are rebuildable. Reconciliation checks source fingerprints and only re-embeds changed files. Keyword/vector ranks are combined with reciprocal rank fusion. Cached related-note lookup works without starting a model. Clear-cache and force-rebuild actions are available. No index is generated until semantic indexing is enabled.

Build `nyx-notes` with `--no-default-features` to omit the AI/vector dependency. Native writing, knowledge views, and non-AI integration controls remain available.

## Glimpse

The bundled `nyx-notes-glimpse` plugin uses the existing pinned Glimpse Plugin API v1 handshake. It is installed as a package; enable it in Glimpse's plugin manager and separately enable Notes' local export.

Only the explicitly enabled export under `$XDG_DATA_HOME/nyx-notes/glimpse/launcher.sqlite` is mounted read-only in the plugin sandbox. The export has version-1 `launcher_*` tables and no account credentials or recovery/history files. Disabling the export clears its note/heading rows and vault registry. The plugin has no network or filesystem-write grant.

Queries:

- `note rust`: keyword/title search.
- `note new Project Ideas`: create an ordinary Markdown note in an exported vault.
- `note append Daily Notes`: open a matching note at its end for writing.
- `note heading rust`: heading search/navigation.

Activation tokens are JSON `LauncherAction` v1 (`api_version`, `op`, `vault`, optional `path`, `title`, `offset`, `append`). The Glimpse host validates the limited operation, current plugin grant, and argument shape, then invokes only `nyx-notes --launcher-action TOKEN` with literal arguments. Notes verifies its export permission/vault registry and canonical boundaries again. Neither side interprets token text as shell code. Older Glimpse installations fail gracefully rather than executing unsupported actions.

## Capture, Browser, Files, and Settings

Capture insertion uses its public `capture.search`, `capture.export`, and `capture.ocr_text` control tools. Exported screenshots contain Capture's flattened annotations/redactions; recordings and OCR remain ordinary attachments/text. No Capture database/internal crate is imported. An unavailable Capture process produces a normal error; it is not auto-started.

Web clipping is available through a native dialog and the `notes.clip` tool, preserving source URLs and timestamps. A standalone Nyx Browser interface was not available in the inspected independent repositories, so no private browser internals are assumed. Any browser/suite adapter can supply selected text/article content through this documented surface.

Files integration uses the standard Markdown MIME launcher entry and ordinary file arguments. Appearance uses the pinned shared Nyx preferences/theme components. No mandatory Files, Browser, Drive, or Settings process is introduced.

## Installation and verification boundaries

Notes, its Glimpse package, and the compatible Assistant/Glimpse consumer updates are installed locally. No application or service is started/restarted by the update. Already running processes retain their old code until their normal restart/session transition; clients handle a missing Nyx AI Core or companion as an unavailable capability.

Verification uses local socket fixtures, deterministic embedding vectors, pure URL/action validation, native invisible GTK checks, and compilation including Assistant's CUDA build. No model was downloaded and no inference was run. Model quality, real server throughput, and live Capture/browser sessions have not been claimed as tested.
