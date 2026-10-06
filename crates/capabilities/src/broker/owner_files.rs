//! The owner's own files (Phase 21, ADR-093): the file view, the editor, and Save.
//!
//! - **Only what Plenipo knows:** the organization folder (ADR-205), each active project's folder,
//!   and its objectives' working copies that are still there. A file is named by its top folder
//!   (`org:folder`, `project:<ID>`, or `copy:<ID>`) and its path inside, checked with Guard's path
//!   checker against the folder's real path (`..`, links, junctions, device names, letter case),
//!   so a name can never lead outside. Git's own folder is never listed, read, or written.
//! - **One writer at a time** (ADR-016): a folder a worker is writing in now (its working copy,
//!   a project folder it writes in directly, or its own folder) opens read-only, naming the
//!   worker, through whichever top folder it is reached. A save checks for a writer and writes
//!   while holding the lock a worker's step takes to open, so a step can never start in the
//!   middle of a save.
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
use plenipo_guard::places::{same_place, within};
use plenipo_guard::{Capability, Level};
use plenipo_ledger::{FolderKind, Project, WorkspaceState};
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
/// run, a Mac's and Linux's (Phase 23), and the scripts and installers people send.
const RUNS: &[&str] = &[
    "action",
    "ade",
    "adp",
    "ahk",
    "ahk2",
    "app",
    "appcontent-ms",
    "appimage",
    "appinstaller",
    "applescript",
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
    "bin",
    "cab",
    "cer",
    "chm",
    "cmd",
    "cnt",
    "com",
    "command",
    "cpl",
    "crt",
    "csh",
    "deb",
    "der",
    "deskthemepack",
    "desktop",
    "diagcab",
    "dll",
    "dmg",
    "docm",
    "dotm",
    "drv",
    "exe",
    "fileloc",
    "fish",
    "flatpakref",
    "fxp",
    "gadget",
    "groovy",
    "grp",
    "hlp",
    "hpj",
    "hta",
    "htc",
    "img",
    "inetloc",
    "inf",
    "ins",
    "iqy",
    "iso",
    "isp",
    "its",
    "jar",
    "jl",
    "jnlp",
    "js",
    "jse",
    "kext",
    "ksh",
    "library-ms",
    "lnk",
    "lua",
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
    "mpkg",
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
    "nu",
    "ocx",
    "one",
    "ops",
    "osd",
    "pcd",
    "php",
    "pif",
    "pkg",
    "pl",
    "plg",
    "potm",
    "ppam",
    "ppsm",
    "pptm",
    "prefpane",
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
    "r",
    "rb",
    "rbw",
    "rdp",
    "reg",
    "rpm",
    "run",
    "scf",
    "scpt",
    "scptd",
    "scr",
    "sct",
    "search-ms",
    "searchconnector-ms",
    "settingcontent-ms",
    "sh",
    "shb",
    "shs",
    "sldm",
    "slk",
    "snap",
    "sys",
    "tcl",
    "tcsh",
    "terminal",
    "theme",
    "themepack",
    "tk",
    "tool",
    "udl",
    "url",
    "vb",
    "vbe",
    "vbp",
    "vbs",
    "vhd",
    "vhdx",
    "vsix",
    "vsmacros",
    "vsw",
    "webloc",
    "webpnp",
    "website",
    "workflow",
    "ws",
    "wsb",
    "wsc",
    "wsf",
    "wsh",
    "xbap",
    "xla",
    "xlam",
    "xll",
    "xlm",
    "xlsm",
    "xltm",
    "xnk",
    "zsh",
];

/// File name endings of web pages and drawings that a browser opens from the disk with their
/// scripts running. Plenipo shows them, but never opens them in another program (P-DESK-2).
const WEB_PAGES: &[&str] = &[
    "htm", "html", "mht", "mhtml", "shtml", "svg", "svgz", "xht", "xhtml",
];

/// `name` ends in one of `endings` (after a dot; trailing dots and spaces, which Windows drops,
/// do not count).
fn ends_in(name: &str, endings: &[&str]) -> bool {
    let lower = name.trim_end_matches(['.', ' ']).to_ascii_lowercase();
    lower
        .rsplit_once('.')
        .is_some_and(|(_, ext)| endings.contains(&ext))
}

/// A file that runs when opened (a program or a script), by the end of its name.
pub fn runs(name: &str) -> bool {
    ends_in(name, RUNS)
}

/// A web page or a drawing whose scripts would run in the owner's browser, by the end of its
/// name.
fn web_page(name: &str) -> bool {
    ends_in(name, WEB_PAGES)
}

/// A file that runs when opened: a program or a script by the end of its name, or, on a Mac and
/// Linux, any file marked as a program, which runs there whatever its name (Phase 23).
/// What the file view shows of one entry (the reviewer's S1 on #224): a file kept only online is
/// described from the folder listing alone (`listed`), because following it to what it leads to
/// opens it, and opening one marked to download when opened (`RECALL_ON_OPEN`) downloads it. A
/// cloud file is the file itself, never a link, so nothing is lost. Anything else is followed
/// (`follow`), so a link shows what it leads to.
fn shown_metadata<M>(
    listed: Option<M>,
    online_only: bool,
    follow: impl FnOnce() -> Option<M>,
) -> Option<M> {
    if online_only {
        listed
    } else {
        follow()
    }
}

pub fn runs_at(path: &Path, name: &str) -> bool {
    runs(name) || marked_to_run(path)
}

#[cfg(unix)]
fn marked_to_run(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Windows runs a file by the end of its name only.
#[cfg(not(unix))]
fn marked_to_run(_path: &Path) -> bool {
    false
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
    /// The organization folder (ADR-205): there is one, so its name is always `org:folder`.
    Organization,
}

/// The organization folder's name as a file view's top folder.
pub(crate) const ORG_ROOT: &str = "org:folder";

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
        "org" if root == ORG_ROOT => Ok(RootRef::Organization),
        _ => Err(bad()),
    }
}

/// A file view's top folder, found in the Ledger.
struct KnownRoot {
    id: String,
    /// The project it belongs to (`None`: the organization folder).
    project: Option<Project>,
    /// How it is named in a sentence: "the folder of Website", "the organization folder".
    shown: String,
    kind: FileRootKind,
    folder: PathBuf,
    workspace_id: Option<String>,
    branch: Option<String>,
}

/// How a folder of the organization's is marked in the file view (ADR-205).
fn place_label(kind: FolderKind, title: Option<&str>) -> String {
    match kind {
        FolderKind::Organization => "Organization folder".into(),
        FolderKind::Department => "Department".into(),
        FolderKind::Project => "Project".into(),
        FolderKind::DepartmentFiles | FolderKind::ProjectFiles => "Finished files".into(),
        FolderKind::ScratchPads => "Scratch pads".into(),
        FolderKind::ScratchPad => match title {
            Some(t) => format!("{t}'s scratch pad"),
            None => "Scratch pad".into(),
        },
    }
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
    let written = (|| {
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
        Ok(())
    })();
    if let Err(e) = written {
        // The new copy could not be written whole (the disk is full, say): the file stays as it
        // was, never half written.
        let _ = std::fs::remove_file(&temp);
        return Err(e);
    }
    match std::fs::rename(&temp, path) {
        Ok(()) => Ok(()),
        // Only the swap was refused (another program holds the file open on Windows): the new
        // copy is whole, so the file is written in place from it.
        Err(_) => {
            let result = std::fs::read(&temp).and_then(|whole| std::fs::write(path, whole));
            let _ = std::fs::remove_file(&temp);
            result
        }
    }
}

impl Broker {
    /// The folders Plenipo knows: each active project's folder, and its working copies that are
    /// still there, with the worker writing in each now.
    pub fn file_roots(&self) -> Result<FileRoots> {
        let writers = self.folder_writers();
        let mut roots = Vec::new();
        // The organization folder first (ADR-205), as it really is on the disk.
        let organization = self.ledger().organization_folder()?.map(|f| {
            let path = PathBuf::from(&f.path);
            FileRoot {
                id: ORG_ROOT.into(),
                project_id: String::new(),
                project_name: String::new(),
                kind: FileRootKind::OrganizationFolder,
                label: self.organization_name(),
                exists: path.is_dir()
                    && !std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()),
                path: f.path,
                writer: None,
                inside_organization: None,
            }
        });
        let org_real = organization
            .as_ref()
            .filter(|o| o.exists)
            .and_then(|o| dunce::canonicalize(&o.path).ok());
        let mut projects = self.ledger().list_projects()?;
        projects.retain(|p| p.status == "active" && p.deleted_at.is_none());
        projects.sort_by_key(|p| p.name.to_lowercase());
        for p in projects {
            if let Some(path) = p.local_path.clone() {
                let id = format!("project:{}", p.id);
                let inside = org_real.as_ref().is_some_and(|org| {
                    dunce::canonicalize(&path).is_ok_and(|real| within(&real, org))
                });
                roots.push(FileRoot {
                    writer: writers.get(&id).cloned(),
                    exists: Path::new(&path).is_dir(),
                    id,
                    project_id: p.id.clone(),
                    project_name: p.name.clone(),
                    kind: FileRootKind::ProjectFolder,
                    label: "Project folder".into(),
                    path,
                    inside_organization: inside.then_some(true),
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
                    inside_organization: None,
                });
            }
        }
        Ok(FileRoots {
            roots,
            desktop_in_use: self.desktop_in_use(),
            organization,
        })
    }

    /// The organization's own folders (ADR-205), where each really is, with its mark. A folder
    /// that isn't there now is left out.
    fn organization_places(&self) -> Vec<(PathBuf, FolderPlace)> {
        let Ok(folders) = self.ledger().folders() else {
            return Vec::new();
        };
        if folders.is_empty() {
            return Vec::new();
        }
        let titles: std::collections::HashMap<String, String> = self
            .ledger()
            .org_records()
            .map(|r| r.positions.into_iter().map(|p| (p.id, p.title)).collect())
            .unwrap_or_default();
        folders
            .into_iter()
            .filter_map(|f| {
                let real = dunce::canonicalize(&f.path).ok()?;
                let title = f.ref_id.as_ref().and_then(|id| titles.get(id));
                Some((
                    real,
                    FolderPlace {
                        kind: f.kind,
                        label: place_label(f.kind, title.map(String::as_str)),
                    },
                ))
            })
            .collect()
    }

    /// Every folder the organization has recorded (ADR-205), where each really is, or would be
    /// when it is missing (the part of its path that exists, followed).
    fn recorded_folder_paths(&self) -> Vec<PathBuf> {
        self.ledger()
            .folders()
            .map(|folders| {
                folders
                    .into_iter()
                    .map(|f| plenipo_guard::places::real_or_written(Path::new(&f.path)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The organization's name, as its Ledger keeps it.
    fn organization_name(&self) -> String {
        self.ledger()
            .setting("organization")
            .ok()
            .flatten()
            .and_then(|v| v["name"].as_str().map(str::to_owned))
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| "Organization".into())
    }

    /// Where a task's worker kept its files (ADR-201): the folder the task's newest grant opened
    /// in, from Plenipo's own record of it. `None` when the worker had no folder (no file tools,
    /// or work that never started).
    pub fn work_folder(&self, task_id: &str) -> Result<Option<WorkFolder>> {
        let events = self.ledger().events_for_task(task_id)?;
        let Some(opened) = events.iter().rev().find(|e| {
            e.event_type == "guard.grant_opened"
                && e.payload["folder"].as_str().is_some_and(|f| !f.is_empty())
        }) else {
            return Ok(None);
        };
        let path = opened.payload["folder"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let plenipo_files = self
            .inner
            .config
            .files_dir
            .as_ref()
            .is_some_and(|base| Path::new(&path).starts_with(base));
        let text = |v: &serde_json::Value| v.as_str().filter(|s| !s.is_empty()).map(str::to_owned);
        Ok(Some(WorkFolder {
            exists: Path::new(&path).is_dir(),
            project: text(&opened.payload["project"]),
            branch: text(&opened.payload["workspace"]["branch"]),
            plenipo_files,
            path,
        }))
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

    /// The worker writing in a folder that holds `abs` — through whichever known folder the file
    /// is reached (a project folder inside another, ADR-093 §13; a project's or an agent's own
    /// folder inside the organization folder, ADR-205) — if one is. Every step that may change
    /// files counts, in its working copy, its project folder, or its own folder.
    fn writer_over(&self, abs: &Path) -> Option<FolderWriter> {
        let s = self.state();
        let mut grants: Vec<&Grant> = s.grants.values().filter(|g| writes_in(g)).collect();
        grants.sort_by_key(|g| g.opened_at);
        grants
            .into_iter()
            .find(|g| {
                g.workspace
                    .as_ref()
                    // Part by part (the reviewer's N1 on #224): a long path's `\\?\` form or
                    // another letter case can't miss its writer.
                    .is_some_and(|w| within(abs, w.root()))
            })
            .map(writer_of)
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
                    shown: format!("the folder of {}", project.name),
                    kind: FileRootKind::ProjectFolder,
                    folder: PathBuf::from(folder),
                    workspace_id: None,
                    branch: None,
                    project: Some(project),
                })
            }
            RootRef::Organization => {
                let folder = self.ledger().organization_folder()?.ok_or_else(|| {
                    BrokerError::Invalid("This organization has no organization folder yet.".into())
                })?;
                let folder = PathBuf::from(folder.path);
                // One that became a junction or link leads somewhere else (ADR-205).
                if std::fs::symlink_metadata(&folder).is_ok_and(|m| m.file_type().is_symlink()) {
                    return Err(BrokerError::Invalid(format!(
                        "The organization folder ({}) is now a shortcut to another place, so \
                         Plenipo doesn't open it.",
                        folder.display()
                    )));
                }
                Ok(KnownRoot {
                    id: ORG_ROOT.into(),
                    shown: "the organization folder".into(),
                    kind: FileRootKind::OrganizationFolder,
                    folder,
                    workspace_id: None,
                    branch: None,
                    project: None,
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
                    shown: format!("the working copy of {}", project.name),
                    kind: FileRootKind::WorkingCopy,
                    folder: w.folder(),
                    workspace_id: Some(w.id.clone()),
                    branch: Some(w.branch.clone()),
                    project: Some(project),
                })
            }
        }
    }

    /// Open a known top folder and check a path inside it.
    fn resolve_owner(&self, root: &KnownRoot, path: &str) -> Result<(Folder, Resolved)> {
        let folder = Folder::open(&root.folder.display().to_string()).map_err(|_| {
            let shown = &root.shown;
            let mut sentence = shown.chars();
            let capital = sentence
                .next()
                .map(|c| c.to_uppercase().chain(sentence).collect::<String>())
                .unwrap_or_default();
            BrokerError::Invalid(format!(
                "{capital} is not there any more ({}).",
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
        let places = self.organization_places();
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
            // Read from the listing itself first: looking never downloads a file kept only online.
            let listed = entry.metadata().ok();
            let online_only = listed
                .as_ref()
                .is_some_and(plenipo_guard::places::online_only);
            // A link is shown as what it leads to (a broken one as a file).
            let meta = shown_metadata(listed, online_only, || std::fs::metadata(entry.path()).ok());
            let folder = meta.as_ref().is_some_and(std::fs::Metadata::is_dir);
            let place = folder
                .then(|| {
                    let at = entry.path();
                    places
                        .iter()
                        .find(|(path, _)| same_place(path, &at))
                        .map(|(_, place)| place.clone())
                })
                .flatten();
            out.push(FolderEntry {
                size: meta
                    .as_ref()
                    .filter(|m| m.is_file())
                    .map(std::fs::Metadata::len),
                modified: meta.as_ref().and_then(modified_ms),
                blocked: blocked_by(&blocked, &rel).is_some(),
                runs: !folder && runs_at(&entry.path(), &name),
                name,
                path: rel,
                folder,
                place,
                online_only: online_only.then_some(true),
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
        let runs = runs_at(&resolved.abs, &name);
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
            self.writer_over(&resolved.abs)
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
        // Never a file where one of the organization's folders belongs, even while that folder
        // is missing: Plenipo makes it again there (the reviewer's N2 on #224).
        if self
            .recorded_folder_paths()
            .iter()
            .any(|f| same_place(f, &resolved.abs))
        {
            return Err(BrokerError::Invalid(format!(
                "{} is where one of the organization's folders belongs. Choose another name.",
                resolved.rel
            )));
        }
        // Held while checking for a writer and writing: a worker's step takes it to open, so a
        // step never starts in the middle of this save (ADR-093 §13).
        let _no_step_opens = lock(&self.inner.making);
        if self.desktop_in_use() {
            return Err(BrokerError::Invalid(
                "A worker is using the screen, mouse, and keyboard. Take over first, then save."
                    .into(),
            ));
        }
        if let Some(writer) = self.writer_over(&resolved.abs) {
            return Err(BrokerError::Invalid(format!(
                "{} is writing in this {} now. Save once it is done, or stop the worker.",
                writer.worker,
                match known.kind {
                    FileRootKind::WorkingCopy => "working copy",
                    FileRootKind::ProjectFolder => "project folder",
                    FileRootKind::OrganizationFolder => "folder",
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
                FileRootKind::OrganizationFolder => "organizationFolder",
            },
            "path": self.redact(&resolved.rel),
            "bytes": bytes.len(),
            "added": added,
            "removed": removed,
        });
        if let Some(project) = &known.project {
            payload["projectId"] = project.id.as_str().into();
            payload["project"] = project.name.as_str().into();
        }
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
        if to_open && runs_at(&resolved.abs, &name) {
            return Err(BrokerError::Invalid(
                "Plenipo never starts programs or scripts. Open it in Plenipo, or show it in its \
                 folder."
                    .into(),
            ));
        }
        if to_open && web_page(&name) {
            return Err(BrokerError::Invalid(
                "A web page or an SVG drawing opens in Plenipo only: in your browser, its own \
                 scripts would run. Open it in Plenipo, or show it in its folder."
                    .into(),
            ));
        }
        // Windows' File Explorer reads a comma as the end of a name: such a file opens in
        // Plenipo only, so another program is never handed half a name.
        if to_open && resolved.abs.to_string_lossy().contains(',') {
            return Err(BrokerError::Invalid(
                "A file whose name or folder has a comma opens in Plenipo only.".into(),
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
        let mut out: Vec<ChangingFile> = self
            .watch()
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
            .collect();
        // A file in a project folder inside the organization folder (ADR-205) is marked where
        // the organization folder shows it too.
        let org = self
            .ledger()
            .organization_folder()
            .ok()
            .flatten()
            .and_then(|f| dunce::canonicalize(f.path).ok());
        if let Some(org) = org {
            let mut inside: std::collections::HashMap<String, Option<String>> =
                std::collections::HashMap::new();
            let mut more = Vec::new();
            for c in &out {
                let prefix = inside.entry(c.root.clone()).or_insert_with(|| {
                    let known = self.known_root(&c.root).ok()?;
                    if known.kind != FileRootKind::ProjectFolder {
                        return None;
                    }
                    let real = dunce::canonicalize(&known.folder).ok()?;
                    let rel = real.strip_prefix(&org).ok()?;
                    Some(
                        rel.components()
                            .map(|p| p.as_os_str().to_string_lossy().into_owned())
                            .collect::<Vec<_>>()
                            .join("/"),
                    )
                });
                if let Some(prefix) = prefix {
                    more.push(ChangingFile {
                        root: ORG_ROOT.into(),
                        path: if prefix.is_empty() {
                            c.path.clone()
                        } else {
                            format!("{prefix}/{}", c.path)
                        },
                        worker: c.worker.clone(),
                        position_id: c.position_id.clone(),
                    });
                }
            }
            out.extend(more);
        }
        out
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

    /// The reviewer's S1 on #224: a file kept only online, even one OneDrive downloads when it
    /// is opened (`RECALL_ON_OPEN`), is listed from the folder listing's own marks and never
    /// opened; anything else is followed, so a link shows what it leads to.
    #[test]
    fn a_file_kept_only_online_is_never_opened_to_list_it() {
        let opened = std::cell::Cell::new(false);
        let follow = || {
            opened.set(true);
            Some("what the link leads to")
        };
        assert_eq!(
            shown_metadata(Some("the listing's"), true, follow),
            Some("the listing's")
        );
        assert!(!opened.get(), "a file kept only online was opened");
        assert_eq!(
            shown_metadata(Some("the listing's"), false, follow),
            Some("what the link leads to")
        );
        assert!(opened.get());
        // OneDrive's two "download when used" marks, and Windows' offline mark.
        for mark in [0x0040_0000, 0x0004_0000, 0x0000_1000] {
            assert!(
                plenipo_guard::places::online_only_attributes(mark),
                "{mark:#x}"
            );
        }
        assert!(!plenipo_guard::places::online_only_attributes(0x10));
    }

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
            "notes.one",
            "update.appinstaller",
            "root.cer",
            "extension.vsix",
            "query.iqy",
            "Open Me.command",
            "Installer.pkg",
            "shortcut.webloc",
            "Tool.AppImage",
            "app.desktop",
            "setup.run",
            // P-DESK-2: interpreters a double-click starts, when they are installed.
            "evil.ahk",
            "evil.AHK2",
            "evil.RB",
            "evil.rbw",
            "evil.tcl",
            "evil.tk",
            "evil.r",
            "evil.jl",
            "evil.nu",
            "evil.groovy",
        ] {
            assert!(runs(name), "{name} runs");
        }
        for name in [
            "README.md",
            "notes.md",
            "report.pdf",
            "logo.png",
            "notes.txt",
            "exe",
            "Makefile",
            "page.html",
        ] {
            assert!(!runs(name), "{name} does not run");
        }
        // Kept sorted and without repeats, so a missing one is easy to see.
        for list in [RUNS, WEB_PAGES] {
            assert!(list.windows(2).all(|w| w[0] < w[1]), "{list:?}");
        }
        for name in [
            "index.html",
            "Page.HTM",
            "saved.mht",
            "logo.svg",
            "a.xhtml",
            "x.html.",
        ] {
            assert!(web_page(name), "{name}");
        }
        assert!(!web_page("notes.md") && !web_page("html"));
    }

    /// Phase 23: on a Mac and Linux a file marked as a program runs whatever its name, so it is
    /// never opened in another program either.
    #[cfg(unix)]
    #[test]
    fn a_file_marked_as_a_program_runs_on_a_mac_and_linux() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("deploy");
        std::fs::write(&script, "#!/bin/sh\necho hi\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!runs_at(&script, "deploy"), "not marked: a plain file");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(runs_at(&script, "deploy"), "marked as a program");
        // A folder is not a program, whatever its marks.
        assert!(!runs_at(dir.path(), "folder"));
        assert!(runs_at(&dir.path().join("missing.sh"), "missing.sh"));
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
        let listed = p.broker.list_folder(&p.root, "").unwrap();
        let runs_of = |name: &str| listed.entries.iter().find(|e| e.name == name).unwrap().runs;
        assert!(runs_of("tool.exe") && runs_of("setup.ps1"));
        assert!(!runs_of("logo.png") && !runs_of("src"));
        assert!(p
            .broker
            .owner_file_path(&p.root, "setup.ps1", true)
            .is_err());
        assert!(
            p.broker.owner_file_path(&p.root, "tool.exe", false).is_ok(),
            "shown in its folder"
        );
        // A comma ends a name for Windows' File Explorer: such a file opens in Plenipo only.
        std::fs::write(p.folder.join("a,b.txt"), "comma\n").unwrap();
        assert!(p.broker.read_file(&p.root, "a,b.txt").is_ok());
        assert!(p.broker.owner_file_path(&p.root, "a,b.txt", true).is_err());
        assert!(p.broker.owner_file_path(&p.root, "a,b.txt", false).is_ok());
        let picture = p.broker.read_file(&p.root, "logo.png").unwrap();
        assert!(
            matches!(&picture.content, FileContent::Picture { mime, .. } if mime == "image/png")
        );
        // P-DESK-2: a web page or an SVG drawing is shown in Plenipo and in its folder, but never
        // opened in the owner's browser, where its scripts would run.
        std::fs::write(p.folder.join("page.html"), "<script>1</script>\n").unwrap();
        std::fs::write(p.folder.join("logo.svg"), "<svg/>\n").unwrap();
        for name in ["page.html", "logo.svg"] {
            let page = p.broker.read_file(&p.root, name).unwrap();
            assert!(!page.runs, "{name}");
            let why = p.broker.owner_file_path(&p.root, name, true).unwrap_err();
            assert!(why.to_string().contains("opens in Plenipo only"), "{why}");
            assert!(p.broker.owner_file_path(&p.root, name, false).is_ok());
        }
    }

    /// The organization folder (ADR-205): a department with its head, and a project inside it,
    /// made by Plenipo's keeper; the harness's own project stays elsewhere on this PC.
    fn with_organization_folder(p: &Project) -> PathBuf {
        let root = p._dir.path().join("Acme");
        crate::org_folder::create(&p.ledger, &root, "Acme", "owner").unwrap();
        let templates = plenipo_workforce::templates::role_templates();
        p.ledger.ensure_roles(&templates, "owner").unwrap();
        let manager = p
            .ledger
            .org_records()
            .unwrap()
            .roles
            .into_iter()
            .find(|r| r.name == "Manager")
            .unwrap()
            .id;
        p.ledger
            .create_department_with_head(
                "Development",
                "",
                &plenipo_ledger::NewPosition {
                    title: "Development Manager".into(),
                    role_id: manager,
                    ..plenipo_ledger::NewPosition::default()
                },
                "owner",
            )
            .unwrap();
        let kept = crate::org_folder::keep(&p.ledger, &[]);
        assert!(kept.problems.is_empty(), "{kept:?}");
        root
    }

    #[tokio::test]
    async fn the_organization_folder_is_the_first_folder_with_its_folders_marked() {
        let p = project();
        assert!(p.broker.file_roots().unwrap().organization.is_none());
        assert!(p.broker.list_folder(ORG_ROOT, "").is_err(), "no folder yet");
        let root = with_organization_folder(&p);
        // A project whose folder is inside the organization folder is marked so.
        let inside = root.join("Development").join("Inside");
        std::fs::create_dir_all(&inside).unwrap();
        p.ledger
            .create_project(
                "Inside",
                Some(&inside.display().to_string()),
                None,
                None,
                "test",
            )
            .unwrap();
        let roots = p.broker.file_roots().unwrap();
        let org = roots.organization.clone().unwrap();
        assert_eq!(org.id, ORG_ROOT);
        assert_eq!(org.kind, FileRootKind::OrganizationFolder);
        assert!(org.exists);
        let by_name = |name: &str| roots.roots.iter().find(|r| r.project_name == name).unwrap();
        assert_eq!(by_name("Inside").inside_organization, Some(true));
        assert_eq!(by_name("Website").inside_organization, None);
        // Its folders are marked with what they are for.
        let top = p.broker.list_folder(ORG_ROOT, "").unwrap();
        let mark = |l: &FolderListing, name: &str| {
            l.entries
                .iter()
                .find(|e| e.name == name)
                .unwrap_or_else(|| panic!("{name} in {l:?}"))
                .place
                .clone()
                .map(|p| p.label)
        };
        assert_eq!(mark(&top, "Development").as_deref(), Some("Department"));
        assert_eq!(mark(&top, "Read me.md"), None);
        let dept = p.broker.list_folder(ORG_ROOT, "Development").unwrap();
        assert_eq!(mark(&dept, "Files").as_deref(), Some("Finished files"));
        assert_eq!(mark(&dept, "Scratch pads").as_deref(), Some("Scratch pads"));
        let pads = p
            .broker
            .list_folder(ORG_ROOT, "Development/Scratch pads")
            .unwrap();
        assert_eq!(
            mark(&pads, "Development Manager").as_deref(),
            Some("Development Manager's scratch pad")
        );
        assert!(pads.entries.iter().all(|e| e.online_only.is_none()));
    }

    #[tokio::test]
    async fn the_owner_reads_and_saves_in_the_organization_folder_and_nowhere_outside() {
        let p = project();
        let root = with_organization_folder(&p);
        let pad = "Development/Scratch pads/Development Manager";
        let pad_folder = pad.split('/').fold(root.clone(), |at, part| at.join(part));
        std::fs::write(pad_folder.join("notes.md"), "plan\n").unwrap();
        let notes = format!("{pad}/notes.md");
        let view = p.broker.read_file(ORG_ROOT, &notes).unwrap();
        assert!(view.read_only.is_none());
        let saved = p
            .broker
            .save_file(
                ORG_ROOT,
                &notes,
                "plan, done\n",
                false,
                LineEnding::Lf,
                None,
            )
            .unwrap();
        assert!(matches!(saved, SaveOutcome::Saved { .. }));
        let event = p
            .ledger
            .recent_events(20)
            .unwrap()
            .into_iter()
            .find(|e| e.event_type == "file.saved")
            .unwrap();
        assert_eq!(event.payload["place"], "organizationFolder");
        assert_eq!(event.payload["root"], ORG_ROOT);
        assert!(event.payload.get("projectId").is_none());
        // Never outside it, never git's own folder, and only the one organization folder.
        std::fs::create_dir_all(root.join(".git")).unwrap();
        for path in ["../website/README.md", ".git/config", "Development/../../x"] {
            assert!(p.broker.read_file(ORG_ROOT, path).is_err(), "{path}");
        }
        for bad in ["org:other", "org:", "org:folder/x"] {
            assert!(p.broker.list_folder(bad, "").is_err(), "{bad}");
        }
    }

    /// The reviewer's N2 on #224: the owner can't save a file where one of the organization's
    /// folders belongs, even while that folder is missing (Plenipo makes it again there).
    #[tokio::test]
    async fn a_file_is_never_saved_where_an_organization_folder_belongs() {
        let p = project();
        let root = with_organization_folder(&p);
        let files = root.join("Development").join("Files");
        std::fs::remove_dir_all(&files).unwrap();
        let save = |path: &str| {
            p.broker
                .save_file(ORG_ROOT, path, "mine\n", false, LineEnding::Lf, None)
        };
        let why = save("Development/Files").unwrap_err().to_string();
        assert!(
            why.contains("where one of the organization's folders belongs"),
            "{why}"
        );
        // Another letter case is the same place where the system ignores it.
        if cfg!(any(windows, target_os = "macos")) {
            assert!(save("development/FILES").is_err());
        }
        assert!(!files.exists());
        // A file of its own name beside it is fine.
        assert!(save("Development/Files.md").is_ok());
    }

    /// The reviewer's O2 for the owner's own view: an organization folder that became a junction
    /// or link is not opened, and its files aren't read through it.
    #[tokio::test]
    async fn an_organization_folder_that_became_a_link_is_not_opened() {
        let p = project();
        let root = with_organization_folder(&p);
        let moved = p._dir.path().join("moved");
        std::fs::rename(&root, &moved).unwrap();
        #[cfg(windows)]
        {
            let status = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(&root)
                .arg(&moved)
                .stdout(std::process::Stdio::null())
                .status()
                .unwrap();
            assert!(status.success());
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&moved, &root).unwrap();
        let org = p.broker.file_roots().unwrap().organization.unwrap();
        assert!(!org.exists);
        let why = p.broker.list_folder(ORG_ROOT, "").unwrap_err().to_string();
        assert!(why.contains("shortcut"), "{why}");
        assert!(p.broker.read_file(ORG_ROOT, "Read me.md").is_err());
    }
}
