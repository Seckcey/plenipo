//! The owner's own files (Phase 21, ADR-093): the file view, the editor, and Save.
//!
//! - **Only what Plenipo knows:** each active project's folder, and its objectives' working copies
//!   that are still there. A file is named by its top folder (`project:<ID>` or `copy:<ID>`) and
//!   its path inside, checked with Guard's path checker against the folder's real path (`..`,
//!   links, junctions, device names, letter case), so a name can never lead outside. Git's own
//!   folder is never listed, read, or written.
//! - **One writer at a time** (ADR-016): a working copy (or a project folder a worker writes in
//!   directly) that a worker is writing now opens read-only, naming the worker. A save checks
//!   for a writer and writes while holding the lock a worker's step takes to open, so a step can
//!   never start in the middle of a save.
//! - **A worker at the keyboard:** while a worker uses the screen, mouse, and keyboard, nothing
//!   is saved and blocked files are not shown, until the owner takes over (what a worker types
//!   must never reach a file through the owner's editor).
//! - **Recorded:** each save is in the Activity trail as the owner's (`file.saved`: where, the
//!   file's path, its size, and the lines added and removed; never its contents).
//! - **Programs and scripts** are never opened in another program: Plenipo never starts them.
//!
//! Workers are not affected: their file access still goes through Guard (ADR-013).

use std::collections::HashSet;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use plenipo_guard::paths::{blocked_by, Resolved, Workspace as Folder};
use plenipo_guard::{Capability, Level};
use plenipo_ledger::{Project, WorkspaceState};
use serde_json::json;
use sha2::{Digest as _, Sha256};

use super::{lock, Broker, Grant};
use crate::dto::*;
use crate::error::{BrokerError, Result};

/// The most text the editor opens, and the largest picture shown.
pub const MAX_TEXT_BYTES: u64 = 5 * 1024 * 1024;
pub const MAX_PICTURE_BYTES: u64 = 20 * 1024 * 1024;
/// The most entries a folder listing shows.
pub const MAX_ENTRIES: usize = 2_000;
/// Who does all of this.
const OWNER: &str = "owner";
/// Git's own folder.
const GIT_DIR: &str = ".git";

/// File name endings of programs and scripts: files that run when opened. Plenipo never starts
/// them, and never opens them in another program (ADR-091 §7). Windows' own list of files that
/// run, and the scripts and installers people send.
const RUNS: &[&str] = &[
    "ade",
    "adp",
    "app",
    "appcontent-ms",
    "application",
    "appref-ms",
    "appx",
    "appxbundle",
    "asp",
    "aspx",
    "asx",
    "bas",
    "bash",
    "bat",
    "bgi",
    "cab",
    "chm",
    "cmd",
    "cnt",
    "com",
    "command",
    "cpl",
    "csh",
    "diagcab",
    "dll",
    "docm",
    "dotm",
    "drv",
    "exe",
    "fxp",
    "gadget",
    "grp",
    "hlp",
    "hpj",
    "hta",
    "htc",
    "img",
    "inf",
    "ins",
    "iso",
    "isp",
    "its",
    "jar",
    "jnlp",
    "js",
    "jse",
    "ksh",
    "library-ms",
    "lnk",
    "mad",
    "maf",
    "mag",
    "mam",
    "maq",
    "mar",
    "mas",
    "mat",
    "mau",
    "mav",
    "maw",
    "mcf",
    "mda",
    "mdb",
    "mde",
    "mdt",
    "mdw",
    "mdz",
    "msc",
    "msh",
    "msh1",
    "msh1xml",
    "msh2",
    "msh2xml",
    "mshxml",
    "msi",
    "msix",
    "msixbundle",
    "msp",
    "mst",
    "msu",
    "ocx",
    "ops",
    "osd",
    "pcd",
    "pif",
    "pl",
    "plg",
    "potm",
    "ppam",
    "ppsm",
    "pptm",
    "prf",
    "prg",
    "printerexport",
    "ps1",
    "ps1xml",
    "ps2",
    "ps2xml",
    "psc1",
    "psc2",
    "psd1",
    "psdm1",
    "psm1",
    "pssc",
    "py",
    "pyc",
    "pyo",
    "pyw",
    "pyz",
    "pyzw",
    "rdp",
    "reg",
    "scf",
    "scr",
    "sct",
    "search-ms",
    "settingcontent-ms",
    "sh",
    "shb",
    "shs",
    "sldm",
    "sys",
    "theme",
    "url",
    "vb",
    "vbe",
    "vbp",
    "vbs",
    "vhd",
    "vhdx",
    "vsmacros",
    "vsw",
    "webpnp",
    "website",
    "ws",
    "wsb",
    "wsc",
    "wsf",
    "wsh",
    "xbap",
    "xlam",
    "xll",
    "xlsm",
    "xltm",
    "xnk",
    "zsh",
];

/// A file that runs when opened (a program or a script), by the end of its name.
pub fn runs(name: &str) -> bool {
    let lower = name.trim_end_matches(['.', ' ']).to_ascii_lowercase();
    lower
        .rsplit_once('.')
        .is_some_and(|(_, ext)| RUNS.contains(&ext))
}

/// A picture Plenipo shows, and its type.
fn picture_type(name: &str) -> Option<&'static str> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        _ => return None,
    })
}

/// A file that is not shown, in words.
fn what_it_is(name: &str, runs: bool) -> String {
    if runs {
        return "A program or a script. Plenipo never starts it.".into();
    }
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "pdf" => "A PDF document.".into(),
        "doc" | "docx" | "odt" | "rtf" => "A word-processing document.".into(),
        "xls" | "xlsx" | "ods" => "A spreadsheet.".into(),
        "ppt" | "pptx" | "odp" => "A presentation.".into(),
        "zip" | "7z" | "rar" | "tar" | "gz" | "tgz" => "A compressed folder.".into(),
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => "A sound file.".into(),
        "mp4" | "mov" | "avi" | "mkv" | "webm" => "A video.".into(),
        "ttf" | "otf" | "woff" | "woff2" => "A font.".into(),
        "psd" | "ai" | "indd" | "tif" | "tiff" | "heic" | "raw" => {
            "A picture Plenipo cannot show.".into()
        }
        _ => "Not a text file Plenipo can show.".into(),
    }
}

/// What a file view's top folder is.
enum RootRef<'a> {
    Project(&'a str),
    Copy(&'a str),
}

fn parse_root(root: &str) -> Result<RootRef<'_>> {
    let bad = || BrokerError::Invalid("That is not a folder Plenipo knows.".into());
    let (kind, id) = root.split_once(':').ok_or_else(bad)?;
    if id.is_empty() || id.len() > 128 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(bad());
    }
    match kind {
        "project" => Ok(RootRef::Project(id)),
        "copy" => Ok(RootRef::Copy(id)),
        _ => Err(bad()),
    }
}

/// A file view's top folder, found in the Ledger.
struct KnownRoot {
    id: String,
    project: Project,
    kind: FileRootKind,
    folder: PathBuf,
    workspace_id: Option<String>,
    branch: Option<String>,
}

/// Where a grant's files are: its working copy, or its project's folder when it works there
/// directly. `None` when it has no folder.
pub(super) fn root_of(g: &Grant) -> Option<String> {
    if let Some(place) = &g.place {
        return Some(format!("copy:{}", place.workspace_id));
    }
    match (&g.workspace, &g.scope.project) {
        (Some(_), Some(p)) => Some(format!("project:{}", p.id)),
        _ => None,
    }
}

/// The grant holds its folder as the one worker changing files there (ADR-016 §3): the writer
/// of its working copy, or, in a project folder, any step that may change files.
fn writes_in(g: &Grant) -> bool {
    if g.revoked {
        return false;
    }
    match &g.place {
        Some(place) => place.writer,
        None => {
            g.workspace.is_some()
                && [Capability::FilesystemWrite, Capability::GitWrite]
                    .iter()
                    .any(|c| g.levels.get(c).copied().unwrap_or_default() != Level::Blocked)
        }
    }
}

fn writer_of(g: &Grant) -> FolderWriter {
    FolderWriter {
        worker: g.worker.clone(),
        position_id: g.position_id.clone(),
        session_id: g.session_id.clone(),
        task_id: g.task_id.clone(),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn modified_ms(meta: &std::fs::Metadata) -> Option<u64> {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|d| u64::try_from(d.as_millis()).ok())
}

/// The path inside is git's own folder, or inside it.
fn in_git_dir(rel: &str) -> bool {
    rel.split('/')
        .any(|part| part.eq_ignore_ascii_case(GIT_DIR))
}

/// How a text's lines end (the first line ending decides; none: this PC's own).
fn line_ending_of(text: &str) -> LineEnding {
    match text.find('\n') {
        Some(i) if i > 0 && text.as_bytes()[i - 1] == b'\r' => LineEnding::Crlf,
        Some(_) => LineEnding::Lf,
        None if cfg!(windows) => LineEnding::Crlf,
        None => LineEnding::Lf,
    }
}

/// Text as it is kept on disk: the file's own line endings and byte-order mark.
fn to_disk(text: &str, bom: bool, ending: LineEnding) -> Vec<u8> {
    let normal = text.replace("\r\n", "\n");
    let body = match ending {
        LineEnding::Lf => normal,
        LineEnding::Crlf => normal.replace('\n', "\r\n"),
    };
    let mut out = Vec::with_capacity(body.len() + 3);
    if bom {
        out.extend_from_slice(b"\xEF\xBB\xBF");
    }
    out.extend_from_slice(body.as_bytes());
    out
}

/// Lines added and removed between two texts (`(0, 0)` if they could not be worked out quickly).
fn counts(before: &str, after: &str) -> (u32, u32) {
    let diff = similar::TextDiff::configure()
        .timeout(std::time::Duration::from_millis(300))
        .diff_lines(before, after);
    let (mut added, mut removed) = (0u32, 0u32);
    for op in diff.ops() {
        let (tag, old, new) = op.as_tag_tuple();
        let n = |r: std::ops::Range<usize>| u32::try_from(r.len()).unwrap_or(u32::MAX);
        match tag {
            similar::DiffTag::Equal => {}
            similar::DiffTag::Insert => added = added.saturating_add(n(new)),
            similar::DiffTag::Delete => removed = removed.saturating_add(n(old)),
            similar::DiffTag::Replace => {
                added = added.saturating_add(n(new));
                removed = removed.saturating_add(n(old));
            }
        }
    }
    (added, removed)
}

/// Write a file through a new copy beside it, swapped in, so a crash never leaves half a file.
/// When the swap is refused (the file is open in another program on Windows), it is written in
/// place instead.
fn write_whole(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::other("the file has no folder"))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = dir.join(format!(
        ".{name}.plenipo-{}.tmp",
        uuid::Uuid::new_v4().simple()
    ));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        if let Ok(meta) = std::fs::metadata(path) {
            let _ = std::fs::set_permissions(&temp, meta.permissions());
        }
        std::fs::rename(&temp, path)
    })();
    match result {
        Ok(()) => Ok(()),
        Err(_) => {
            let _ = std::fs::remove_file(&temp);
            std::fs::write(path, bytes)
        }
    }
}

impl Broker {
    /// The folders Plenipo knows: each active project's folder, and its working copies that are
    /// still there, with the worker writing in each now.
    pub fn file_roots(&self) -> Result<FileRoots> {
        let writers = self.folder_writers();
        let mut roots = Vec::new();
        let mut projects = self.ledger().list_projects()?;
        projects.retain(|p| p.status == "active" && p.deleted_at.is_none());
        projects.sort_by_key(|p| p.name.to_lowercase());
        for p in projects {
            if let Some(path) = p.local_path.clone() {
                let id = format!("project:{}", p.id);
                roots.push(FileRoot {
                    writer: writers.get(&id).cloned(),
                    exists: Path::new(&path).is_dir(),
                    id,
                    project_id: p.id.clone(),
                    project_name: p.name.clone(),
                    kind: FileRootKind::ProjectFolder,
                    label: "Project folder".into(),
                    path,
                });
            }
            for w in self.ledger().project_workspaces(&p.id, 200)? {
                if w.state != WorkspaceState::Active {
                    continue;
                }
                let id = format!("copy:{}", w.id);
                let folder = w.folder();
                roots.push(FileRoot {
                    writer: writers.get(&id).cloned(),
                    exists: folder.is_dir(),
                    id,
                    project_id: p.id.clone(),
                    project_name: p.name.clone(),
                    kind: FileRootKind::WorkingCopy,
                    label: w.branch.clone(),
                    path: folder.display().to_string(),
                });
            }
        }
        Ok(FileRoots {
            roots,
            desktop_in_use: self.desktop_in_use(),
        })
    }

    /// Who is writing where now, by top folder.
    fn folder_writers(&self) -> std::collections::HashMap<String, FolderWriter> {
        let s = self.state();
        let mut out = std::collections::HashMap::new();
        let mut grants: Vec<&Grant> = s.grants.values().filter(|g| writes_in(g)).collect();
        grants.sort_by_key(|g| g.opened_at);
        for g in grants {
            if let Some(root) = root_of(g) {
                out.entry(root).or_insert_with(|| writer_of(g));
            }
        }
        out
    }

    /// A worker is using the screen, mouse, and keyboard now (the owner has not taken over).
    fn desktop_in_use(&self) -> bool {
        self.inner.control.status().desktop_active()
    }

    fn known_root(&self, root: &str) -> Result<KnownRoot> {
        let unknown = || BrokerError::Invalid("That is not a folder Plenipo knows.".into());
        let active = |p: &Project| p.status == "active" && p.deleted_at.is_none();
        match parse_root(root)? {
            RootRef::Project(id) => {
                let project = self
                    .ledger()
                    .project(id)?
                    .filter(active)
                    .ok_or_else(unknown)?;
                let folder = project.local_path.clone().ok_or_else(|| {
                    BrokerError::Invalid(format!("The {} project has no folder.", project.name))
                })?;
                Ok(KnownRoot {
                    id: root.to_owned(),
                    kind: FileRootKind::ProjectFolder,
                    folder: PathBuf::from(folder),
                    workspace_id: None,
                    branch: None,
                    project,
                })
            }
            RootRef::Copy(id) => {
                let w = self
                    .ledger()
                    .workspace(id)?
                    .filter(|w| w.state == WorkspaceState::Active)
                    .ok_or_else(unknown)?;
                let project = self
                    .ledger()
                    .project(&w.project_id)?
                    .filter(active)
                    .ok_or_else(unknown)?;
                Ok(KnownRoot {
                    id: root.to_owned(),
                    kind: FileRootKind::WorkingCopy,
                    folder: w.folder(),
                    workspace_id: Some(w.id.clone()),
                    branch: Some(w.branch.clone()),
                    project,
                })
            }
        }
    }

    /// Open a known top folder and check a path inside it.
    fn resolve_owner(&self, root: &KnownRoot, path: &str) -> Result<(Folder, Resolved)> {
        let folder = Folder::open(&root.folder.display().to_string()).map_err(|_| {
            BrokerError::Invalid(format!(
                "The folder of {} is not there any more ({}).",
                root.project.name,
                root.folder.display()
            ))
        })?;
        if path.len() > 4096 || path.contains('\0') {
            return Err(BrokerError::Invalid("That is not a file name.".into()));
        }
        let resolved = folder.resolve(path).map_err(|_| {
            BrokerError::Invalid(format!(
                "{path} is outside the folders Plenipo knows, so it cannot be opened here."
            ))
        })?;
        if resolved.in_git_dir() || in_git_dir(&resolved.rel) {
            return Err(BrokerError::Invalid(
                "Git's own files are not opened or changed in Plenipo.".into(),
            ));
        }
        Ok((folder, resolved))
    }

    fn blocked_files(&self) -> Result<Vec<String>> {
        Ok(self.inner.guard.config()?.blocked_files)
    }

    /// One folder's files and folders (folders first), up to [`MAX_ENTRIES`].
    pub fn list_folder(&self, root: &str, path: &str) -> Result<FolderListing> {
        let known = self.known_root(root)?;
        let (_, resolved) = self.resolve_owner(&known, path)?;
        let blocked = self.blocked_files()?;
        let entries = std::fs::read_dir(&resolved.abs).map_err(|e| {
            BrokerError::Invalid(format!("Plenipo could not open that folder ({e})."))
        })?;
        let mut out: Vec<FolderEntry> = Vec::new();
        for entry in entries.filter_map(std::result::Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.eq_ignore_ascii_case(GIT_DIR) {
                continue;
            }
            let rel = if resolved.rel.is_empty() {
                name.clone()
            } else {
                format!("{}/{name}", resolved.rel)
            };
            // A link is shown as what it leads to (a broken one as a file).
            let meta = std::fs::metadata(entry.path()).ok();
            let folder = meta.as_ref().is_some_and(std::fs::Metadata::is_dir);
            out.push(FolderEntry {
                size: meta
                    .as_ref()
                    .filter(|m| m.is_file())
                    .map(std::fs::Metadata::len),
                modified: meta.as_ref().and_then(modified_ms),
                blocked: blocked_by(&blocked, &rel).is_some(),
                name,
                path: rel,
                folder,
            });
        }
        out.sort_by(|a, b| {
            b.folder
                .cmp(&a.folder)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                .then_with(|| a.name.cmp(&b.name))
        });
        let more = out.len().saturating_sub(MAX_ENTRIES);
        out.truncate(MAX_ENTRIES);
        Ok(FolderListing {
            root: known.id,
            path: resolved.rel,
            entries: out,
            more: u32::try_from(more).unwrap_or(u32::MAX),
        })
    }

    /// Open a file in Plenipo: text, a picture, or what it is.
    pub fn read_file(&self, root: &str, path: &str) -> Result<FileView> {
        let known = self.known_root(root)?;
        let (_, resolved) = self.resolve_owner(&known, path)?;
        let meta = std::fs::metadata(&resolved.abs).map_err(|_| {
            BrokerError::Invalid(format!("{} is not there any more.", resolved.rel))
        })?;
        if meta.is_dir() {
            return Err(BrokerError::Invalid(format!(
                "{} is a folder.",
                resolved.rel
            )));
        }
        let name = resolved
            .abs
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let blocked = blocked_by(&self.blocked_files()?, &resolved.rel).is_some();
        let desktop = self.desktop_in_use();
        if blocked && desktop {
            return Err(BrokerError::Invalid(
                "Hidden while a worker uses the screen, mouse, and keyboard. Take over to see it."
                    .into(),
            ));
        }
        let runs = runs(&name);
        let size = meta.len();
        let (content, hash) = if let Some(mime) = picture_type(&name) {
            if size > MAX_PICTURE_BYTES {
                (
                    FileContent::Other {
                        what: format!("A large picture ({}).", words_size(size)),
                    },
                    None,
                )
            } else {
                let bytes = std::fs::read(&resolved.abs).map_err(read_error)?;
                let hash = sha256_hex(&bytes);
                (
                    FileContent::Picture {
                        mime: mime.into(),
                        data: base64::engine::general_purpose::STANDARD.encode(&bytes),
                    },
                    Some(hash),
                )
            }
        } else if size > MAX_TEXT_BYTES {
            (
                FileContent::Other {
                    what: format!(
                        "A large file ({}). Plenipo opens text files up to 5 MB.",
                        words_size(size)
                    ),
                },
                None,
            )
        } else {
            let bytes = std::fs::read(&resolved.abs).map_err(read_error)?;
            let hash = sha256_hex(&bytes);
            let (body, bom) = match bytes.strip_prefix(b"\xEF\xBB\xBF") {
                Some(rest) => (rest, true),
                None => (bytes.as_slice(), false),
            };
            match std::str::from_utf8(body) {
                Ok(text) if !text.contains('\0') => (
                    FileContent::Text {
                        line_ending: line_ending_of(text),
                        text: text.replace("\r\n", "\n"),
                        bom,
                    },
                    Some(hash),
                ),
                _ => (
                    FileContent::Other {
                        what: what_it_is(&name, runs),
                    },
                    Some(hash),
                ),
            }
        };
        let read_only = if matches!(content, FileContent::Text { .. }) {
            self.folder_writers()
                .remove(&known.id)
                .map(|writer| ReadOnlyWhy::Writer { writer })
                .or(desktop.then_some(ReadOnlyWhy::Desktop))
                .or(meta.permissions().readonly().then_some(ReadOnlyWhy::Disk))
        } else {
            None
        };
        Ok(FileView {
            root: known.id,
            path: resolved.rel,
            name,
            size,
            modified: modified_ms(&meta),
            hash,
            content,
            runs,
            blocked,
            read_only,
        })
    }

    /// Save a text file the owner edited. `base`: the fingerprint of the file as it was opened
    /// (`None`: Save anyway, over whatever is there now).
    pub fn save_file(
        &self,
        root: &str,
        path: &str,
        text: &str,
        bom: bool,
        ending: LineEnding,
        base: Option<&str>,
    ) -> Result<SaveOutcome> {
        let bytes = to_disk(text, bom, ending);
        if bytes.len() as u64 > MAX_TEXT_BYTES {
            return Err(BrokerError::Invalid(
                "Plenipo saves text files up to 5 MB.".into(),
            ));
        }
        let known = self.known_root(root)?;
        let (_, resolved) = self.resolve_owner(&known, path)?;
        // Held while checking for a writer and writing: a worker's step takes it to open, so a
        // step never starts in the middle of this save (ADR-093 §13).
        let _no_step_opens = lock(&self.inner.making);
        if self.desktop_in_use() {
            return Err(BrokerError::Invalid(
                "A worker is using the screen, mouse, and keyboard. Take over first, then save."
                    .into(),
            ));
        }
        if let Some(writer) = self.folder_writers().remove(&known.id) {
            return Err(BrokerError::Invalid(format!(
                "{} is writing in this {} now. Save once it is done, or stop the worker.",
                writer.worker,
                match known.kind {
                    FileRootKind::WorkingCopy => "working copy",
                    FileRootKind::ProjectFolder => "project folder",
                }
            )));
        }
        let current = std::fs::read(&resolved.abs).ok();
        if let Ok(meta) = std::fs::metadata(&resolved.abs) {
            if meta.is_dir() {
                return Err(BrokerError::Invalid(format!(
                    "{} is a folder.",
                    resolved.rel
                )));
            }
            if meta.permissions().readonly() {
                return Err(BrokerError::Invalid(format!(
                    "{} is marked read-only on the disk.",
                    resolved.rel
                )));
            }
        }
        if let Some(base) = base {
            if current.as_deref().map(sha256_hex).as_deref() != Some(base) {
                return Ok(SaveOutcome::ChangedOnDisk);
            }
        }
        write_whole(&resolved.abs, &bytes).map_err(|e| {
            BrokerError::Invalid(format!("Plenipo could not save {} ({e}).", resolved.rel))
        })?;
        let before = current
            .as_deref()
            .map(|b| String::from_utf8_lossy(b).replace("\r\n", "\n"))
            .unwrap_or_default();
        let (added, removed) = counts(&before, &text.replace("\r\n", "\n"));
        let meta = std::fs::metadata(&resolved.abs).ok();
        let mut payload = json!({
            "root": known.id,
            "place": match known.kind {
                FileRootKind::ProjectFolder => "projectFolder",
                FileRootKind::WorkingCopy => "workingCopy",
            },
            "projectId": known.project.id,
            "project": known.project.name,
            "path": self.redact(&resolved.rel),
            "bytes": bytes.len(),
            "added": added,
            "removed": removed,
        });
        if let (Some(id), Some(branch)) = (&known.workspace_id, &known.branch) {
            payload["workspaceId"] = id.as_str().into();
            payload["branch"] = branch.as_str().into();
        }
        self.event(None, OWNER, "file.saved", payload);
        Ok(SaveOutcome::Saved {
            hash: sha256_hex(&bytes),
            size: bytes.len() as u64,
            modified: meta.as_ref().and_then(modified_ms),
            added,
            removed,
        })
    }

    /// Where a known file is on this PC, to open it in another program or show it in its folder.
    /// Refused for a program or a script when `to_open` (Plenipo never starts them).
    pub fn owner_file_path(&self, root: &str, path: &str, to_open: bool) -> Result<PathBuf> {
        let known = self.known_root(root)?;
        let (_, resolved) = self.resolve_owner(&known, path)?;
        if !resolved.exists {
            return Err(BrokerError::Invalid(format!(
                "{} is not there any more.",
                resolved.rel
            )));
        }
        let name = resolved
            .abs
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if to_open && runs(&name) {
            return Err(BrokerError::Invalid(
                "Plenipo never starts programs or scripts. Open it in Plenipo, or show it in its \
                 folder."
                    .into(),
            ));
        }
        if to_open && resolved.abs.is_dir() {
            return Err(BrokerError::Invalid(format!(
                "{} is a folder.",
                resolved.rel
            )));
        }
        if to_open
            && self.desktop_in_use()
            && blocked_by(&self.blocked_files()?, &resolved.rel).is_some()
        {
            return Err(BrokerError::Invalid(
                "Hidden while a worker uses the screen, mouse, and keyboard. Take over first."
                    .into(),
            ));
        }
        Ok(resolved.abs)
    }

    /// The files the workers of the steps open now are changing (the file view marks them).
    pub fn changing_files(&self) -> Vec<ChangingFile> {
        let sessions: HashSet<String> = self
            .state()
            .grants
            .values()
            .filter(|g| !g.revoked)
            .map(|g| g.session_id.clone())
            .collect();
        self.watch()
            .changing(&sessions)
            .into_iter()
            .filter_map(|c| {
                Some(ChangingFile {
                    root: c.root?,
                    path: c.path,
                    worker: c.worker,
                    position_id: c.position_id,
                })
            })
            .collect()
    }
}

fn read_error(e: std::io::Error) -> BrokerError {
    BrokerError::Invalid(format!("Plenipo could not read that file ({e})."))
}

fn words_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{} KB", bytes.div_ceil(1024))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_and_scripts_are_known_by_their_names() {
        for name in [
            "setup.exe",
            "Run.BAT",
            "deploy.ps1",
            "install.msi",
            "link.lnk",
            "macro.docm",
            "tool.sh",
            "tricky.exe.",
            "tricky.exe ",
        ] {
            assert!(runs(name), "{name} runs");
        }
        for name in [
            "README.md",
            "report.pdf",
            "logo.png",
            "notes.txt",
            "exe",
            "Makefile",
        ] {
            assert!(!runs(name), "{name} does not run");
        }
    }

    #[test]
    fn roots_are_named_by_kind_and_an_id() {
        assert!(matches!(
            parse_root("project:abc-123"),
            Ok(RootRef::Project("abc-123"))
        ));
        assert!(matches!(parse_root("copy:w1"), Ok(RootRef::Copy("w1"))));
        for bad in [
            "",
            "project:",
            "folder:x",
            "copy:../x",
            "project:a/b",
            "copy:a b",
        ] {
            assert!(parse_root(bad).is_err(), "{bad} is refused");
        }
    }

    #[test]
    fn text_keeps_its_own_line_endings_and_mark() {
        assert_eq!(line_ending_of("a\r\nb"), LineEnding::Crlf);
        assert_eq!(line_ending_of("a\nb"), LineEnding::Lf);
        assert_eq!(to_disk("a\nb\n", false, LineEnding::Crlf), b"a\r\nb\r\n");
        assert_eq!(to_disk("a\r\nb", false, LineEnding::Lf), b"a\nb");
        assert_eq!(to_disk("a", true, LineEnding::Lf), b"\xEF\xBB\xBFa");
    }

    #[test]
    fn git_s_own_folder_is_recognised_at_any_depth_and_case() {
        assert!(in_git_dir(".git"));
        assert!(in_git_dir("sub/.GIT/config"));
        assert!(!in_git_dir(".github/workflows/ci.yml"));
        assert!(!in_git_dir("src/git.rs"));
    }

    #[test]
    fn a_whole_file_is_swapped_in() {
        let dir = tempfile::tempdir().expect("a folder");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "old").expect("written");
        write_whole(&file, b"new").expect("saved");
        assert_eq!(std::fs::read(&file).expect("read"), b"new");
        // No copy is left behind.
        let left: Vec<_> = std::fs::read_dir(dir.path())
            .expect("listed")
            .filter_map(std::result::Result::ok)
            .collect();
        assert_eq!(left.len(), 1);
    }

    #[test]
    fn counts_lines_added_and_removed() {
        assert_eq!(counts("a\nb\nc\n", "a\nB\nc\nd\n"), (2, 1));
        assert_eq!(counts("", "x\n"), (1, 0));
    }

    // ---- The owner's files, through a broker on a project folder ------------------------------

    use std::sync::Arc;

    use plenipo_ledger::Ledger;
    use plenipo_runtime::{
        EventSink, ExecutablePolicy, ProfileRegistry, RuntimeEvent, Supervisor, SupervisorConfig,
    };

    use crate::control::ControlKind;
    use crate::{BrokerConfig, MemorySecretStore};

    struct NoOutput;
    impl EventSink for NoOutput {
        fn emit(&self, _: RuntimeEvent) {}
    }

    struct Project {
        broker: Broker,
        ledger: Arc<Ledger>,
        root: String,
        folder: PathBuf,
        _dir: tempfile::TempDir,
    }

    /// A broker, and a project whose folder holds README.md, src/app.txt, a blocked `.env`, and
    /// git's own folder.
    fn project() -> Project {
        let dir = tempfile::tempdir().expect("a folder");
        let folder = dir.path().join("website");
        std::fs::create_dir_all(folder.join("src")).unwrap();
        std::fs::create_dir_all(folder.join(".git")).unwrap();
        std::fs::write(folder.join("README.md"), "# Website\r\n\r\nHello.\r\n").unwrap();
        std::fs::write(folder.join("src").join("app.txt"), "a\nb\n").unwrap();
        std::fs::write(folder.join(".env"), "KEY=secret\n").unwrap();
        std::fs::write(folder.join(".git").join("config"), "[core]\n").unwrap();
        let ledger = Arc::new(Ledger::open_in_memory().unwrap());
        let guard = plenipo_guard::Guard::new(Arc::clone(&ledger));
        let sup = Supervisor::new(
            SupervisorConfig::default(),
            ExecutablePolicy::default(),
            ProfileRegistry::default(),
            Arc::new(plenipo_liaison::store::LedgerExecutionStore(Arc::clone(
                &ledger,
            ))),
            Arc::new(NoOutput),
            vec![],
        );
        let config = BrokerConfig::new(PathBuf::from("relay"), dir.path().join("tickets"));
        let broker = Broker::new(guard, sup, Arc::new(MemorySecretStore::default()), config);
        let p = ledger
            .create_project(
                "Website",
                Some(&folder.display().to_string()),
                None,
                None,
                "test",
            )
            .unwrap();
        Project {
            broker,
            ledger,
            root: format!("project:{}", p.id),
            folder,
            _dir: dir,
        }
    }

    #[tokio::test]
    async fn the_owner_sees_only_the_folders_plenipo_knows_and_never_gits_own() {
        let p = project();
        let roots = p.broker.file_roots().unwrap();
        assert_eq!(roots.roots.len(), 1);
        assert_eq!(roots.roots[0].id, p.root);
        assert_eq!(roots.roots[0].kind, FileRootKind::ProjectFolder);
        assert!(roots.roots[0].exists && roots.roots[0].writer.is_none());
        let top = p.broker.list_folder(&p.root, "").unwrap();
        let names: Vec<&str> = top.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["src", ".env", "README.md"],
            "folders first; no .git"
        );
        assert!(
            top.entries
                .iter()
                .find(|e| e.name == ".env")
                .unwrap()
                .blocked
        );
        assert!(
            !top.entries
                .iter()
                .find(|e| e.name == "README.md")
                .unwrap()
                .blocked
        );
        for path in [
            "../outside.txt",
            "/etc/passwd",
            ".git/config",
            ".GIT",
            "src/../../x",
        ] {
            assert!(
                p.broker.read_file(&p.root, path).is_err(),
                "{path} is refused"
            );
            assert!(p
                .broker
                .save_file(&p.root, path, "x", false, LineEnding::Lf, None)
                .is_err());
        }
        assert!(p.broker.read_file("project:nope", "README.md").is_err());
        assert!(p.broker.read_file("folder:x", "README.md").is_err());
        #[cfg(unix)]
        {
            let outside = p._dir.path().join("secret.txt");
            std::fs::write(&outside, "not yours to read here").unwrap();
            std::os::unix::fs::symlink(&outside, p.folder.join("link.txt")).unwrap();
            assert!(
                p.broker.read_file(&p.root, "link.txt").is_err(),
                "a link out is refused"
            );
        }
    }

    #[tokio::test]
    async fn a_save_keeps_the_files_own_endings_and_is_recorded_as_the_owners() {
        let p = project();
        let view = p.broker.read_file(&p.root, "README.md").unwrap();
        let FileContent::Text {
            text,
            bom,
            line_ending,
        } = &view.content
        else {
            panic!("text: {view:?}");
        };
        assert_eq!(text, "# Website\n\nHello.\n");
        assert_eq!(*line_ending, LineEnding::Crlf);
        assert!(!bom);
        assert!(view.read_only.is_none() && !view.runs && !view.blocked);
        let saved = p
            .broker
            .save_file(
                &p.root,
                "README.md",
                "# Website\n\nHello, world.\n",
                *bom,
                *line_ending,
                view.hash.as_deref(),
            )
            .unwrap();
        let SaveOutcome::Saved { added, removed, .. } = saved else {
            panic!("saved: {saved:?}");
        };
        assert_eq!((added, removed), (1, 1));
        assert_eq!(
            std::fs::read_to_string(p.folder.join("README.md")).unwrap(),
            "# Website\r\n\r\nHello, world.\r\n"
        );
        // In the Activity trail as the owner's, with no contents.
        let events = p.ledger.recent_events(20).unwrap();
        let e = events
            .iter()
            .find(|e| e.event_type == "file.saved")
            .expect("recorded");
        assert_eq!(e.source, "owner");
        assert_eq!(e.payload["path"], "README.md");
        assert_eq!(e.payload["added"], 1);
        assert!(!e.payload.to_string().contains("Hello"), "{}", e.payload);
    }

    #[tokio::test]
    async fn a_file_changed_on_disk_is_not_saved_over_unless_the_owner_says_so() {
        let p = project();
        let view = p.broker.read_file(&p.root, "src/app.txt").unwrap();
        std::fs::write(p.folder.join("src").join("app.txt"), "changed by someone\n").unwrap();
        let outcome = p
            .broker
            .save_file(
                &p.root,
                "src/app.txt",
                "mine\n",
                false,
                LineEnding::Lf,
                view.hash.as_deref(),
            )
            .unwrap();
        assert_eq!(outcome, SaveOutcome::ChangedOnDisk);
        assert_eq!(
            std::fs::read_to_string(p.folder.join("src").join("app.txt")).unwrap(),
            "changed by someone\n"
        );
        // Save anyway.
        let outcome = p
            .broker
            .save_file(
                &p.root,
                "src/app.txt",
                "mine\n",
                false,
                LineEnding::Lf,
                None,
            )
            .unwrap();
        assert!(matches!(outcome, SaveOutcome::Saved { .. }));
        assert_eq!(
            std::fs::read_to_string(p.folder.join("src").join("app.txt")).unwrap(),
            "mine\n"
        );
    }

    #[tokio::test]
    async fn while_a_worker_uses_the_keyboard_nothing_is_saved_and_blocked_files_are_hidden() {
        let p = project();
        assert!(
            p.broker.read_file(&p.root, ".env").is_ok(),
            "blocked files are the owner's"
        );
        p.broker
            .inner
            .control
            .begin(ControlKind::Desktop, "g1", "t1", "Operator", None);
        assert!(p.broker.file_roots().unwrap().desktop_in_use);
        assert!(p.broker.read_file(&p.root, ".env").is_err(), "hidden now");
        let view = p.broker.read_file(&p.root, "README.md").unwrap();
        assert_eq!(view.read_only, Some(ReadOnlyWhy::Desktop));
        let refused = p
            .broker
            .save_file(
                &p.root,
                "README.md",
                "typed by a worker",
                false,
                LineEnding::Lf,
                None,
            )
            .unwrap_err();
        assert!(refused.to_string().contains("Take over"), "{refused}");
        assert!(p.broker.owner_file_path(&p.root, ".env", true).is_err());
        // The owner takes over: it is theirs again.
        p.broker
            .inner
            .control
            .take_over(&crate::control::session_id(ControlKind::Desktop, "g1"));
        assert!(p.broker.read_file(&p.root, ".env").is_ok());
    }

    #[tokio::test]
    async fn programs_are_never_opened_elsewhere_and_other_files_are_described() {
        let p = project();
        std::fs::write(p.folder.join("setup.ps1"), "Write-Host hi\n").unwrap();
        std::fs::write(p.folder.join("tool.exe"), b"MZ\x00\x00binary").unwrap();
        std::fs::write(p.folder.join("logo.png"), b"\x89PNG\r\n").unwrap();
        let script = p.broker.read_file(&p.root, "setup.ps1").unwrap();
        assert!(script.runs && matches!(script.content, FileContent::Text { .. }));
        let program = p.broker.read_file(&p.root, "tool.exe").unwrap();
        assert!(program.runs);
        assert!(
            matches!(&program.content, FileContent::Other { what } if what.contains("never starts"))
        );
        assert!(p.broker.owner_file_path(&p.root, "tool.exe", true).is_err());
        assert!(p
            .broker
            .owner_file_path(&p.root, "setup.ps1", true)
            .is_err());
        assert!(
            p.broker.owner_file_path(&p.root, "tool.exe", false).is_ok(),
            "shown in its folder"
        );
        let picture = p.broker.read_file(&p.root, "logo.png").unwrap();
        assert!(
            matches!(&picture.content, FileContent::Picture { mime, .. } if mime == "image/png")
        );
    }
}
