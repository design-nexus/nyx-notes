//! Notes-owned interactive panel companion. Uses the same vault APIs and index;
//! it is not exposed as an automation tool or a second sync/indexing service.
use anyhow::{Context, Result, bail, ensure};
use notes_vault as vault;
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, Read, Write},
    path::{Path, PathBuf},
};
const MAX: usize = 4 * 1024 * 1024;
fn xdg(key: &str, fallback: &str) -> PathBuf {
    std::env::var_os(key)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap()).join(fallback))
}
struct Panel {
    root: PathBuf,
    db: PathBuf,
    state: PathBuf,
    prefs: toml::Value,
    doc: Option<vault::Loaded>,
}
impl Panel {
    fn connect() -> Result<Self> {
        let config = xdg("XDG_CONFIG_HOME", ".config").join("nyx-notes/settings.toml");
        let prefs: toml::Value = toml::from_str(
            &std::fs::read_to_string(config).context("Open Notes and choose a vault first")?,
        )?;
        let root = PathBuf::from(
            prefs
                .get("last_vault")
                .and_then(|v| v.as_str())
                .context("Open Notes and choose a vault first")?,
        )
        .canonicalize()?;
        let cache = xdg("XDG_CACHE_HOME", ".cache").join("nyx-notes");
        let id = vault::draft_path(&cache, &root)
            .file_stem()
            .unwrap()
            .to_owned();
        let db = cache.join(id).with_extension("sqlite");
        let state = xdg("XDG_STATE_HOME", ".local/state").join("nyx-notes");
        Ok(Self {
            root,
            db,
            state,
            prefs,
            doc: None,
        })
    }
    fn recovery(&self) -> PathBuf {
        self.state.join("quick-panel")
    }
    fn checked(&self, s: &str) -> Result<PathBuf> {
        let p = Path::new(s);
        ensure!(
            !p.components()
                .any(|c| matches!(c, std::path::Component::ParentDir)
                    || c.as_os_str().to_string_lossy().starts_with('.')),
            "Hidden files and parent paths are not panel notes"
        );
        let p = if p.is_absolute() {
            p.to_owned()
        } else {
            self.root.join(p)
        };
        let canonical = if p.exists() {
            p.canonicalize()?
        } else {
            let parent = p.parent().context("Missing parent")?.canonicalize()?;
            let candidate = parent.join(p.file_name().context("Missing note name")?);
            ensure!(
                vault::read_draft(&self.recovery(), &candidate)?.is_some(),
                "Note not found"
            );
            candidate
        };
        ensure!(
            canonical.starts_with(&self.root) && vault::is_markdown(&canonical),
            "Choose a Markdown note inside the active vault"
        );
        ensure!(
            std::fs::metadata(&canonical).map(|m| m.len()).unwrap_or(0) <= MAX as u64,
            "Note is too large for the quick panel; open it in Notes"
        );
        ensure!(
            !std::fs::symlink_metadata(&p).is_ok_and(|m| m.file_type().is_symlink()),
            "Symlink notes are read-only"
        );
        Ok(canonical)
    }
    fn list(&self, query: &str) -> Result<Value> {
        if query.is_empty() || !self.db.exists() {
            vault::reconcile_index(&self.db, &self.root)?;
        }
        let pins = self
            .prefs
            .get("pinned")
            .and_then(|v| v.get(self.root.to_string_lossy().as_ref()))
            .and_then(|v| v.as_array());
        let mut hits = if query.is_empty() {
            vault::note_list(&self.db, &self.root)?
                .into_iter()
                .map(|n| (n.path, n.snippet, n.modified))
                .collect::<Vec<_>>()
        } else {
            vault::search(&self.db, query)?
                .into_iter()
                .take(100)
                .map(|n| {
                    let m = std::fs::metadata(&n.path)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                        .map_or(0, |d| d.as_secs() as i64);
                    (n.path, n.snippet, m)
                })
                .collect()
        };
        if query.is_empty() {
            hits.sort_by_key(|(path, _, _)| {
                !pins.is_some_and(|ps| {
                    ps.iter().any(|v| {
                        v.as_str() == path.strip_prefix(&self.root).ok().and_then(|p| p.to_str())
                    })
                })
            });
            hits.truncate(100);
        }
        let notes=hits.into_iter().filter_map(|(path,snippet,modified)|{let rel=path.strip_prefix(&self.root).ok()?;if !path.exists(){return None}Some(json!({"path":rel,"title":path.file_stem().unwrap_or_default().to_string_lossy(),"preview":snippet,"modified":modified,"pinned":pins.is_some_and(|p|p.iter().any(|v|v.as_str()==rel.to_str()))}))}).collect::<Vec<_>>();
        let drafts = vault::recovery_paths(&self.recovery())?
            .into_iter()
            .filter_map(|p| p.strip_prefix(&self.root).ok().map(|p| p.to_path_buf()))
            .collect::<Vec<_>>();
        Ok(
            json!({"vault_id":self.root,"vault":self.root.file_name().unwrap_or_default().to_string_lossy(),"notes":notes,"drafts":drafts}),
        )
    }
    fn document(&self, text: &str, recovered: bool) -> Value {
        let d = self.doc.as_ref().unwrap();
        json!({"path":d.path.strip_prefix(&self.root).unwrap_or(&d.path),"title":d.path.file_stem().unwrap_or_default().to_string_lossy(),"text":text,"read_only":d.read_only,"recovered":recovered})
    }
    fn call(&mut self, v: &Value) -> Result<Value> {
        let op = v["op"].as_str().unwrap_or("");
        match op {
            "list" => self.list(v["query"].as_str().unwrap_or("")),
            "read" => {
                let p = self.checked(v["path"].as_str().context("Missing path")?)?;
                ensure!(
                    vault::read_draft(&self.state, &p)?.is_none(),
                    "This note has unsaved work in Notes. Open Notes to review it first."
                );
                let (d, draft) = vault::open_with_recovery(&self.recovery(), &p)?;
                let text = draft
                    .as_ref()
                    .map(|d| d.text.clone())
                    .unwrap_or_else(|| d.text.clone());
                self.doc = Some(d);
                if let Some(draft) = &draft {
                    let d = self.doc.as_mut().unwrap();
                    d.version.etag = draft.base_etag.clone();
                    d.version.digest = blake3::Hash::from_hex(&draft.base_digest)?;
                }
                Ok(self.document(&text, draft.is_some()))
            }
            "create" => {
                let title = v["title"].as_str().unwrap_or("Untitled").trim();
                ensure!(
                    !title.is_empty() && title.len() <= 200,
                    "Use a short visible title"
                );
                let text = v["text"].as_str().unwrap_or("");
                ensure!(text.len() <= MAX, "Note too large");
                let mut path = None;
                for i in 0..100 {
                    let name = if i == 0 {
                        title.to_string()
                    } else {
                        format!("{title} ({i})")
                    };
                    let dest = self.root.join(if vault::is_markdown(Path::new(&name)) {
                        name.clone()
                    } else {
                        format!("{name}.md")
                    });
                    ensure!(
                        !name.contains(['/', '\\', '\0', '\n', '\r']) && !name.starts_with('.'),
                        "Use a title without path separators"
                    );
                    if dest.exists() {
                        continue;
                    }
                    vault::write_draft(
                        &self.recovery(),
                        &vault::Draft {
                            path: dest.clone(),
                            text: text.into(),
                            bom: false,
                            base_etag: String::new(),
                            base_digest: blake3::hash(b"_").to_hex().to_string(),
                        },
                    )?;
                    match vault::save_copy(&dest, text, false) {
                        Ok(()) => {
                            path = Some(dest);
                            break;
                        }
                        Err(e)
                            if e.downcast_ref::<std::io::Error>()
                                .is_some_and(|e| e.kind() == std::io::ErrorKind::AlreadyExists) =>
                        {
                            continue;
                        }
                        Err(e) => return Err(e),
                    }
                }
                let p = path.context("Could not choose a unique note title")?;
                self.doc = Some(vault::load(&p)?);
                vault::remove_draft(&self.recovery(), &p)?;
                let _ = vault::update_index(&self.db, &p);
                Ok(self.document(text, false))
            }
            "save" | "draft" => {
                let text = v["text"].as_str().context("Missing text")?;
                ensure!(text.len() <= MAX, "Note too large");
                let recovery = self.recovery();
                let d = self.doc.as_mut().context("Choose a note before editing")?;
                ensure!(
                    v["path"].as_str()
                        == d.path
                            .strip_prefix(&self.root)
                            .ok()
                            .and_then(|p| p.to_str()),
                    "The active note changed; draft not applied"
                );
                ensure!(!d.read_only, "This note is read-only");
                vault::write_draft(
                    &recovery,
                    &vault::Draft {
                        path: d.path.clone(),
                        text: text.into(),
                        bom: d.bom,
                        base_etag: d.version.etag.clone(),
                        base_digest: d.version.digest.to_hex().to_string(),
                    },
                )?;
                if op == "draft" {
                    return Ok(json!({"draft_saved":true}));
                }
                let old = vault::load(&d.path)?;
                vault::history::capture(
                    &self.state,
                    &d.path,
                    &old.text,
                    old.bom,
                    self.prefs
                        .get("history_keep")
                        .and_then(|v| v.as_integer())
                        .unwrap_or(200)
                        .clamp(0, 10000) as u32,
                    self.prefs
                        .get("history_days")
                        .and_then(|v| v.as_integer())
                        .unwrap_or(90)
                        .clamp(0, 3650) as u32,
                )?;
                d.version = vault::save(&d.path, text, d.bom, &d.version)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                d.text = text.into();
                vault::remove_draft(&recovery, &d.path)?;
                let _ = vault::update_index(&self.db, &d.path);
                Ok(json!({"saved":true,"path":v["path"]}))
            }
            "open" => {
                let mut command = std::process::Command::new("nyx-notes");
                command.arg("--vault").arg(&self.root);
                if let Some(path) = v["path"].as_str().filter(|p| !p.is_empty()) {
                    command.arg(self.checked(path)?);
                }
                command.spawn()?;
                Ok(json!({"opened":true}))
            }
            _ => bail!("Unsupported panel operation"),
        }
    }
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if !args.iter().any(|a| a == "--stdio") {
        let share = xdg("XDG_DATA_HOME", ".local/share");
        if toggle_existing(&share) {
            return;
        }
        let status = std::process::Command::new("qs")
            .arg("-p")
            .arg(share.join("nyx-notes-panel/qml/Standalone.qml"))
            .status();
        if !status.is_ok_and(|s| s.success()) {
            std::process::exit(1)
        }
        return;
    }
    let mut panel = None;
    let mut stdin = io::stdin().lock();
    loop {
        let mut data = Vec::new();
        if stdin
            .by_ref()
            .take((MAX * 2 + 1) as u64)
            .read_until(b'\n', &mut data)
            .unwrap_or(0)
            == 0
        {
            break;
        }
        if data.len() > MAX * 2 {
            break;
        }
        let Ok(v) = serde_json::from_slice::<Value>(&data) else {
            continue;
        };
        if v["op"] == "open" && panel.is_none() && v["path"].as_str().unwrap_or("").is_empty() {
            let result = std::process::Command::new("nyx-notes").spawn();
            println!(
                "{}",
                match result {
                    Ok(_) => json!({"id":v["id"],"result":{"opened":true}}),
                    Err(e) => json!({"id":v["id"],"error":e.to_string()}),
                }
            );
            let _ = io::stdout().flush();
            continue;
        }
        if v["op"] == "list" {
            panel = None;
        }
        if panel.is_none() {
            match Panel::connect() {
                Ok(p) => panel = Some(p),
                Err(e) => {
                    println!("{}", json!({"id":v["id"],"error":e.to_string()}));
                    let _ = io::stdout().flush();
                    continue;
                }
            }
        }
        let result = panel.as_mut().unwrap().call(&v);
        println!(
            "{}",
            match result {
                Ok(r) => json!({"id":v["id"],"result":r}),
                Err(e) => json!({"id":v["id"],"error":e.to_string()}),
            }
        );
        let _ = io::stdout().flush();
    }
}
fn toggle_existing(data: &std::path::Path) -> bool {
    let quiet = |program: &str, args: Vec<String>| {
        std::process::Command::new(program)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    };
    if std::env::var_os("OMARCHY_PATH").is_some()
        && quiet(
            "omarchy-shell",
            vec![
                "shell".into(),
                "toggle".into(),
                "design-nexus.notes".into(),
                "{}".into(),
            ],
        )
    {
        return true;
    }
    for qml in [
        data.join("nyx-shell/qml/shell.qml"),
        data.join("nyx-notes-panel/qml/Standalone.qml"),
    ] {
        if qml.exists()
            && quiet(
                "qs",
                vec![
                    "-p".into(),
                    qml.to_string_lossy().into_owned(),
                    "ipc".into(),
                    "call".into(),
                    "nyx-notes-panel".into(),
                    "toggle".into(),
                ],
            )
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conflict_retains_draft_and_rejects_escape() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("vault");
        std::fs::create_dir(&root).unwrap();
        let p = root.join("note.md");
        vault::save_copy(&p, "original", false).unwrap();
        let mut panel = Panel {
            root: root.clone(),
            db: temp.path().join("index.sqlite"),
            state: temp.path().join("state"),
            prefs: toml::Value::Table(Default::default()),
            doc: None,
        };
        panel.call(&json!({"op":"read","path":"note.md"})).unwrap();
        std::fs::write(&p, "external").unwrap();
        assert!(
            panel
                .call(&json!({"op":"save","path":"note.md","text":"mine"}))
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "external");
        assert_eq!(
            vault::read_draft(&panel.recovery(), &p)
                .unwrap()
                .unwrap()
                .text,
            "mine"
        );
        assert!(panel.checked("../outside.md").is_err());
        assert!(panel.checked(".obsidian/settings.md").is_err());
    }
}
