//! The owner's folders on this PC (ADR-205): the organization folder, and every folder Plenipo
//! records inside it. Which places can never be one (Plenipo's own data folder, the system's
//! folders, a profile's or a drive's top folder, a Startup folder); a junction or link on the way
//! (refused: only a folder that really is where it says can be one); a name that is safe as one
//! folder's name on every system; and whether a sync service keeps the folder online, and if
//! OneDrive is told to keep it on this device.
//!
//! On Windows, Rust's `FileType::is_symlink` is true for a reparse point of the "name surrogate"
//! kind: junctions and symbolic links, which point somewhere else. OneDrive's cloud files
//! (`IO_REPARSE_TAG_CLOUD_*`) are reparse points too, but they are the file itself, not a pointer,
//! so they are not refused (the Coordinator's answer of 2026-10-05; ADR-214 lets them through on
//! purpose too).

use std::path::{Component, Path, PathBuf, Prefix};

/// The longest folder path Plenipo uses as one of the organization's folders.
pub const MAX_PLACE_CHARS: usize = 1000;

/// A name that is safe as one folder's name on every system: no slashes, colons, or other marks
/// a file name cannot hold, no dots or spaces at either end (so never `.` or `..`, and never a
/// hidden folder), no Windows device name, and at most 60 characters. `fallback` when nothing is
/// left.
pub fn folder_name(name: &str, fallback: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                ' '
            } else {
                c
            }
        })
        .collect();
    let mut out = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    out = out.chars().take(60).collect::<String>();
    let out = out.trim_matches(|c: char| c == '.' || c == ' ').to_owned();
    const DEVICES: &[&str] = &[
        "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
        "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
    ];
    let stem = out.split('.').next().unwrap_or("").to_ascii_lowercase();
    if out.is_empty() {
        fallback.to_owned()
    } else if DEVICES.contains(&stem.as_str()) {
        format!("_{out}")
    } else {
        out
    }
}

/// The system's own places on this PC, which can never be (or hold) an organization's folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SystemPlaces {
    /// Plenipo's own data folder (the Ledger, backups, working copies): never, nor inside it.
    pub data: Option<PathBuf>,
    /// The system's folders: Windows, Program Files, ProgramData (on a Mac or Linux, `/etc`,
    /// `/usr`, and the like): never, nor inside them.
    pub system: Vec<PathBuf>,
    /// Startup folders, whose programs start with the PC: never, nor inside them.
    pub startup: Vec<PathBuf>,
    /// The owner's own top folder (`C:\Users\<you>`, or the home folder): never itself, but any
    /// folder inside it. Junctions and links above it are the system's, not looked at.
    pub profile: Option<PathBuf>,
    /// The owner's Documents folder, wherever the system keeps it (OneDrive can move it): a
    /// junction or link at or above it is the system's, not looked at.
    pub documents: Option<PathBuf>,
}

impl SystemPlaces {
    /// This PC's places, from its environment, with Plenipo's data folder and the Documents
    /// folder as the system reports them.
    pub fn from_env(data: Option<PathBuf>, documents: Option<PathBuf>) -> Self {
        let env = |k: &str| {
            std::env::var_os(k)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        let mut system: Vec<PathBuf> = [
            "SystemRoot",
            "windir",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "ProgramW6432",
            "ProgramData",
            "ALLUSERSPROFILE",
        ]
        .iter()
        .filter_map(|k| env(k))
        .collect();
        if !cfg!(windows) {
            system.extend(
                [
                    "/bin",
                    "/boot",
                    "/dev",
                    "/etc",
                    "/lib",
                    "/opt",
                    "/proc",
                    "/sbin",
                    "/sys",
                    "/usr",
                    "/var",
                    "/private",
                    "/System",
                    "/Library",
                    "/Applications",
                ]
                .map(PathBuf::from),
            );
        }
        let profile = env("USERPROFILE").or_else(|| env("HOME"));
        let mut startup = Vec::new();
        let start_menu = |base: PathBuf| {
            base.join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs")
                .join("Startup")
        };
        if let Some(app_data) = env("APPDATA") {
            startup.push(start_menu(app_data));
        }
        if let Some(program_data) = env("ProgramData") {
            startup.push(start_menu(program_data));
        }
        if let Some(home) = &profile {
            if !cfg!(windows) {
                startup.push(home.join(".config").join("autostart"));
                startup.push(home.join("Library").join("LaunchAgents"));
            }
        }
        Self {
            data,
            system,
            startup,
            profile,
            documents,
        }
    }

    /// The system's own folders, at and above which a junction or link is not looked at.
    pub fn trusted(&self) -> Vec<PathBuf> {
        self.profile
            .iter()
            .chain(self.documents.iter())
            .cloned()
            .collect()
    }
}

/// The parts of a path to compare, one by one (the reviewer's S2 on #221): its drive or share,
/// whether it starts at the top, and each name. `\\?\C:\` is the same drive as `C:\`, and
/// `\\?\UNC\server\share` the same share as `\\server\share`; `/` and `\` both separate names;
/// and names are in small letters where the system ignores letter case (Windows, and a Mac's
/// usual disk). Never compared as text, which a long path's `\\?\` or a `/` would fool.
fn parts(path: &Path) -> Vec<String> {
    let fold = |s: &std::ffi::OsStr| {
        let s = s.to_string_lossy();
        if cfg!(any(windows, target_os = "macos")) {
            s.to_lowercase()
        } else {
            s.into_owned()
        }
    };
    path.components()
        .filter_map(|c| match c {
            Component::Prefix(prefix) => Some(match prefix.kind() {
                Prefix::Disk(d) | Prefix::VerbatimDisk(d) => {
                    format!("disk:{}", char::from(d).to_ascii_lowercase())
                }
                Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
                    format!("share:{}\\{}", fold(server), fold(share))
                }
                Prefix::Verbatim(x) => format!("verbatim:{}", fold(x)),
                Prefix::DeviceNS(x) => format!("device:{}", fold(x)),
            }),
            Component::RootDir => Some("\\".into()),
            Component::CurDir => None,
            Component::ParentDir => Some("..".into()),
            Component::Normal(n) => Some(fold(n)),
        })
        .collect()
}

/// Whether two paths are the same place, compared part by part (see [`parts`]).
pub fn same_place(a: &Path, b: &Path) -> bool {
    parts(a) == parts(b)
}

/// `path` is `place` or inside it, compared part by part (see [`parts`]).
pub fn within(path: &Path, place: &Path) -> bool {
    let (path, place) = (parts(path), parts(place));
    path.len() >= place.len() && path[..place.len()] == place[..]
}

/// A network share's path as Windows' tools write it, `\\server\share\…`, instead of the
/// `\\?\UNC\server\share\…` form a full path is given in, which File Explorer may not open (the
/// reviewer's N1 on #221). Left as it is when it is too long for the usual form.
pub fn usual_form(path: PathBuf) -> PathBuf {
    match path.to_str().and_then(|s| s.strip_prefix(r"\\?\UNC\")) {
        Some(rest) if rest.len() + 2 < 260 => PathBuf::from(format!(r"\\{rest}")),
        _ => path,
    }
}

/// Where a place really is.
struct Real {
    /// Links followed for the part that exists, the rest added as written.
    path: PathBuf,
    /// The deepest part that exists, as it really is.
    found: PathBuf,
}

/// Where a place really is; `None` when no part of it exists at all (a drive this PC hasn't, or
/// a network place it can't reach).
fn real(path: &Path) -> Option<Real> {
    let mut existing = path.to_path_buf();
    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if let Ok(found) = dunce::canonicalize(&existing) {
            let found = usual_form(found);
            let mut out = found.clone();
            for name in rest.iter().rev() {
                out.push(name);
            }
            return Some(Real { path: out, found });
        }
        match (
            existing.file_name().map(|n| n.to_os_string()),
            existing.parent(),
        ) {
            (Some(name), Some(parent)) => {
                rest.push(name);
                existing = parent.to_path_buf();
            }
            _ => return None,
        }
    }
}

/// A place as it really is, or as written when no part of it exists.
fn real_or_written(path: &Path) -> PathBuf {
    real(path).map_or_else(|| path.to_path_buf(), |r| r.path)
}

/// What stands in the way of a path, from the top down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnTheWay {
    /// A junction or a link: a shortcut to another place.
    Link(PathBuf),
    /// A folder Plenipo isn't allowed to look at, so it can't tell (the reviewer's N3 on #221).
    Unreadable(PathBuf),
}

impl OnTheWay {
    /// The part in the way.
    pub fn path(&self) -> &Path {
        match self {
            Self::Link(p) | Self::Unreadable(p) => p,
        }
    }

    /// Why nothing is made there, in plain words.
    pub fn words(&self) -> String {
        match self {
            Self::Link(p) => format!(
                "{} is a shortcut to another place (a junction or a link)",
                p.display()
            ),
            Self::Unreadable(p) => format!(
                "Plenipo isn't allowed to look inside {}, so it can't tell where it leads",
                p.display()
            ),
        }
    }
}

/// The first part of `path` (an absolute path) that is a junction or a link, or that Plenipo
/// isn't allowed to look at, from the top down, leaving out the folders above (or at) any of
/// `trusted`, which are the system's. `None` when there is none, or when the parts that exist end
/// before one.
pub fn on_the_way(path: &Path, trusted: &[PathBuf]) -> Option<OnTheWay> {
    let mut parts: Vec<&Path> = path.ancestors().collect();
    parts.reverse();
    for part in parts {
        if trusted.iter().any(|t| within(t, part)) {
            continue;
        }
        match std::fs::symlink_metadata(part) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Some(OnTheWay::Link(part.to_path_buf()))
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                return Some(OnTheWay::Unreadable(part.to_path_buf()))
            }
            // Not there yet (or a network place that doesn't answer): nothing more to look at.
            Err(_) => return None,
        }
    }
    None
}

/// The first part of `path` in the way (see [`on_the_way`]): a junction, a link, or a folder
/// Plenipo isn't allowed to look at. Either way, nothing is made through it.
pub fn link_on_the_way(path: &Path, trusted: &[PathBuf]) -> Option<PathBuf> {
    on_the_way(path, trusted).map(|w| w.path().to_path_buf())
}

/// What a network path's server and share say about it (the reviewer's S1 on #221): one of this
/// PC's own names (`\\localhost\C$`, `\\127.0.0.1\…`, `\\<this PC's name>\…`, WSL's
/// `\\wsl.localhost\…`) leads back to this PC's own drives, past every check on them; and a
/// drive's hidden share (`C$`, `ADMIN$`) on any PC is a whole drive.
fn network_problem(server: &str, share: &str) -> Option<String> {
    let server = server
        .split('@')
        .next()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let this_pc = matches!(
        server.as_str(),
        "localhost"
            | "::1"
            | "[::1]"
            | "0.0.0.0"
            | "0--1.ipv6-literal.net"
            | "--1.ipv6-literal.net"
            | "wsl$"
            | "wsl.localhost"
    ) || server.starts_with("127.")
        || server.ends_with(".localhost")
        || ["COMPUTERNAME", "HOSTNAME"]
            .iter()
            .filter_map(|k| std::env::var(k).ok())
            .map(|n| n.trim().to_ascii_lowercase())
            .filter(|n| !n.is_empty())
            .any(|n| server == n || server.starts_with(&format!("{n}.")));
    if this_pc {
        return Some(
            "That's this PC's own drive through a network name. Choose it by its drive letter, \
             such as C:\\Work\\Acme."
                .into(),
        );
    }
    let share = share.to_ascii_lowercase();
    let hidden_drive = share == "admin$"
        || (share.len() == 2 && share.ends_with('$') && share.as_bytes()[0].is_ascii_alphabetic());
    hidden_drive.then(|| {
        "That's a whole drive's hidden network share. Choose a shared folder, or a folder on this \
         PC by its drive letter, such as C:\\Work\\Acme."
            .into()
    })
}

/// [`network_problem`] for a path written as `\\server\share\…` (with `\` or `/`).
fn written_network_problem(shown: &str) -> Option<String> {
    let norm = shown.replace('/', "\\");
    let rest = norm.strip_prefix("\\\\")?;
    let mut names = rest.split('\\');
    let server = names.next().unwrap_or("");
    let share = names.next().unwrap_or("");
    network_problem(server, share)
}

/// [`network_problem`] for a path as the system reads it.
fn path_network_problem(path: &Path) -> Option<String> {
    match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
                network_problem(&server.to_string_lossy(), &share.to_string_lossy())
            }
            _ => None,
        },
        _ => None,
    }
}

/// A path on a network share (`\\server\share\…`).
fn is_network(path: &Path) -> bool {
    matches!(
        path.components().next(),
        Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::UNC(..) | Prefix::VerbatimUNC(..))
    )
}

/// A drive's, a network share's, or the file system's top folder.
fn is_top(path: &Path) -> bool {
    !path.components().any(|c| matches!(c, Component::Normal(_)))
        || matches!(
            (path.components().next(), path.components().count()),
            (Some(Component::Prefix(p)), 2) if matches!(
                p.kind(),
                std::path::Prefix::UNC(..) | std::path::Prefix::VerbatimUNC(..)
            )
        )
}

/// One name of a folder the owner chose (below its drive): an ordinary name.
fn name_problem(name: &str) -> Option<String> {
    if name.ends_with('.') || name.ends_with(' ') {
        return Some(format!(
            "\"{name}\" ends with a dot or a space, which Windows quietly drops"
        ));
    }
    if name
        .chars()
        .any(|c| c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
    {
        return Some(format!(
            "\"{name}\" has a mark that can't be in a folder's name"
        ));
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim()
        .to_ascii_lowercase();
    if [
        "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
        "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9", "conin$",
        "conout$",
    ]
    .contains(&stem.as_str())
    {
        return Some(format!("\"{name}\" is a name Windows keeps for a device"));
    }
    None
}

/// Why `path` can't be one of the organization's folders, in plain words (ADR-205 §2.1, the
/// reviewer's O6 and O2), or where it really is when it can. Refused: a path that isn't full, has
/// `.` or `..` in it, or has an odd name; Plenipo's own data folder and anything inside it; the
/// system's folders and anything inside them; a Startup folder; the owner's own top folder; a
/// drive's or a network share's top folder; a path with a junction or link on the way, or a
/// folder Plenipo isn't allowed to look at; this PC's own drives through a network name, and a
/// whole drive's hidden share (the reviewer's S1 on #221); a file; and a drive this PC hasn't.
/// Every comparison is part by part, never as text (S2).
pub fn place_problem(path: &str, places: &SystemPlaces) -> Result<PathBuf, String> {
    let shown = path.trim();
    if shown.is_empty() {
        return Err("Choose a folder.".into());
    }
    if shown.chars().count() > MAX_PLACE_CHARS {
        return Err(format!(
            "That folder's path is longer than {MAX_PLACE_CHARS} characters."
        ));
    }
    if shown.chars().any(char::is_control) {
        return Err("That folder's path has a character a path can't hold.".into());
    }
    // `\\?\`, `\\.\`, and their mixes with `/`: never the usual way to write a folder's path.
    let norm = shown.replace('/', "\\");
    if norm.starts_with("\\\\?") || norm.starts_with("\\\\.") {
        return Err("Write the folder's path the usual way, such as D:\\Work\\Acme.".into());
    }
    // Before anything is looked at: a network name that is this PC, or a whole drive's share.
    if let Some(why) = written_network_problem(shown) {
        return Err(why);
    }
    let p = Path::new(shown);
    if !p.is_absolute() {
        return Err(format!(
            "{shown} isn't a full path. Choose a folder, or write its whole path, such as \
             D:\\Work\\Acme."
        ));
    }
    for c in p.components() {
        match c {
            Component::CurDir | Component::ParentDir => {
                return Err("Write the folder's path without . or .. in it.".into())
            }
            Component::Normal(n) => {
                if let Some(why) = name_problem(&n.to_string_lossy()) {
                    return Err(format!("That folder can't be used: {why}."));
                }
            }
            Component::Prefix(_) | Component::RootDir => {}
        }
    }
    if is_top(p) {
        return Err(
            "That's the top of a drive. Choose a folder inside it, such as a folder in Documents."
                .into(),
        );
    }
    match on_the_way(p, &places.trusted()) {
        Some(OnTheWay::Link(link)) => {
            return Err(format!(
                "{} is a shortcut to another place (a junction or a link). Choose the folder it \
                 points to, or another one.",
                link.display()
            ))
        }
        Some(unreadable @ OnTheWay::Unreadable(_)) => {
            return Err(format!("{}. Choose another folder.", unreadable.words()))
        }
        None => {}
    }
    let real_path = match real(p) {
        Some(found) => {
            if !found.found.is_dir() {
                return Err(format!(
                    "{} is a file, not a folder. Choose a folder.",
                    found.found.display()
                ));
            }
            found.path
        }
        // A network place that doesn't answer now is kept as written: it is checked again when
        // the folder is made. A drive this PC hasn't is refused.
        None if is_network(p) => p.to_path_buf(),
        None => {
            return Err(format!(
                "Plenipo can't find {} on this PC. Choose a folder on one of its drives.",
                p.components().next().map_or_else(String::new, |c| c
                    .as_os_str()
                    .to_string_lossy()
                    .into_owned())
            ))
        }
    };
    if let Some(why) = path_network_problem(&real_path) {
        return Err(why);
    }
    if is_top(&real_path) {
        return Err(
            "That's the top of a drive. Choose a folder inside it, such as a folder in Documents."
                .into(),
        );
    }
    let real_place = |place: &Path| real_or_written(place);
    if let Some(data) = &places.data {
        if within(&real_path, &real_place(data)) || within(p, data) {
            return Err(
                "That's Plenipo's own data folder, where it keeps its records. Choose a folder \
                 of your own, such as one in Documents."
                    .into(),
            );
        }
    }
    for system in &places.system {
        if within(&real_path, &real_place(system)) || within(p, system) {
            return Err(format!(
                "{} is one of the system's own folders. Choose a folder of your own, such as one \
                 in Documents.",
                system.display()
            ));
        }
    }
    for startup in &places.startup {
        if within(&real_path, &real_place(startup)) || within(p, startup) {
            return Err(
                "That's a Startup folder, whose programs start with your PC. Choose another \
                 folder."
                    .into(),
            );
        }
    }
    if let Some(profile) = &places.profile {
        let real_profile = real_place(profile);
        let users = real_profile.parent().map(Path::to_path_buf);
        if same_place(&real_path, &real_profile)
            || same_place(p, profile)
            || users.as_deref().is_some_and(|u| same_place(&real_path, u))
        {
            return Err(
                "That's the top of your user folders. Choose a folder inside it, such as one in \
                 Documents."
                    .into(),
            );
        }
    }
    Ok(real_path)
}

/// Who syncs a folder online, when Plenipo can tell (the screens' own word is
/// `plenipo_core::FolderSync`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncedBy {
    /// OneDrive: Plenipo can see whether it is told to keep the folder on this device.
    OneDrive,
    /// Another sync service (Dropbox, Google Drive, iCloud): Plenipo can't see that.
    Other,
}

/// The folders OneDrive syncs on this PC, from its environment (`OneDrive`, `OneDriveCommercial`,
/// `OneDriveConsumer`).
pub fn onedrive_roots() -> Vec<PathBuf> {
    ["OneDrive", "OneDriveCommercial", "OneDriveConsumer"]
        .iter()
        .filter_map(std::env::var_os)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// The OneDrive folder `path` is in, of `onedrive`.
pub fn onedrive_root_of(path: &Path, onedrive: &[PathBuf]) -> Option<PathBuf> {
    onedrive.iter().find(|root| within(path, root)).cloned()
}

/// Whether a sync service keeps `path` online: OneDrive when it is inside one of `onedrive`
/// (the folders OneDrive syncs), another service when a folder on the way is a sync service's
/// usual one (Dropbox, Google Drive, iCloud Drive, a Mac's CloudStorage).
pub fn synced_by(path: &Path, onedrive: &[PathBuf]) -> Option<SyncedBy> {
    if onedrive.iter().any(|root| within(path, root)) {
        return Some(SyncedBy::OneDrive);
    }
    const OTHERS: &[&str] = &[
        "Dropbox",
        "Google Drive",
        "My Drive",
        "iCloudDrive",
        "iCloud Drive",
        "CloudStorage",
        "Mobile Documents",
    ];
    let other = path.components().any(|c| match c {
        Component::Normal(n) => {
            let n = n.to_string_lossy();
            OTHERS.iter().any(|o| n.eq_ignore_ascii_case(o))
        }
        _ => false,
    });
    other.then_some(SyncedBy::Other)
}

/// Windows' mark on a file or folder OneDrive is told to keep on this device ("Always keep on
/// this device").
pub const FILE_ATTRIBUTE_PINNED: u32 = 0x0008_0000;
/// Windows' mark on one OneDrive is told to keep online only ("Free up space").
pub const FILE_ATTRIBUTE_UNPINNED: u32 = 0x0010_0000;

/// Whether OneDrive is told to keep `path` on this device: the nearest folder at or above it
/// (as far up as the OneDrive folder `root`) marked to always keep on this device, or to free up
/// space, decides. `None` where Plenipo can't tell (not on Windows).
pub fn kept_on_this_device(path: &Path, root: &Path) -> Option<bool> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        for at in path.ancestors() {
            if let Ok(meta) = std::fs::metadata(at) {
                let attributes = meta.file_attributes();
                if attributes & FILE_ATTRIBUTE_PINNED != 0 {
                    return Some(true);
                }
                if attributes & FILE_ATTRIBUTE_UNPINNED != 0 {
                    return Some(false);
                }
            }
            if same_place(at, root) {
                break;
            }
        }
        Some(false)
    }
    #[cfg(not(windows))]
    {
        let _ = (path, root);
        None
    }
}

/// Windows' marks on a file kept only online (OneDrive's "Files On-Demand", and other cloud
/// services' placeholders): reading it makes the service download it first.
const ONLINE_ONLY: u32 = 0x0040_0000 // FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS
    | 0x0004_0000 // FILE_ATTRIBUTE_RECALL_ON_OPEN
    | 0x0000_1000; // FILE_ATTRIBUTE_OFFLINE

/// Whether a file with Windows' `attributes` is kept only online (ADR-205 §3).
pub fn online_only_attributes(attributes: u32) -> bool {
    attributes & ONLINE_ONLY != 0
}

/// Whether the file whose metadata is `meta` is kept only online: reading it would download it.
/// Looking at its marks never does. Always `false` outside Windows, where Plenipo can't tell.
pub fn online_only(meta: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        online_only_attributes(meta.file_attributes())
    }
    #[cfg(not(windows))]
    {
        let _ = meta;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Files kept only online carry one of three marks; a file on this computer, pinned or not,
    /// carries none of them.
    #[test]
    fn files_kept_only_online_are_known_by_their_marks() {
        for online in [0x0040_0000, 0x0004_0000, 0x0000_1000, 0x0040_0020] {
            assert!(online_only_attributes(online), "{online:#x}");
        }
        for here in [
            0,
            0x20,
            FILE_ATTRIBUTE_PINNED,
            FILE_ATTRIBUTE_UNPINNED | 0x20,
        ] {
            assert!(!online_only_attributes(here), "{here:#x}");
        }
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "here").unwrap();
        assert!(!online_only(&std::fs::metadata(&file).unwrap()));
    }

    fn real_temp() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let real = dunce::canonicalize(dir.path()).unwrap();
        (dir, real)
    }

    fn places_in(base: &Path) -> SystemPlaces {
        SystemPlaces {
            data: Some(base.join("data")),
            system: vec![base.join("Windows"), base.join("Program Files")],
            startup: vec![base.join("Startup")],
            profile: Some(base.join("Users").join("you")),
            documents: Some(base.join("Users").join("you").join("Documents")),
        }
    }

    #[test]
    fn folder_names_are_one_ordinary_name() {
        assert_eq!(folder_name("Acme Co", "Organization"), "Acme Co");
        assert_eq!(folder_name("a/b\\c:d", "x"), "a b c d");
        assert_eq!(folder_name("..", "Project"), "Project");
        assert_eq!(folder_name(".git", "Project"), "git");
        assert_eq!(folder_name("name.", "x"), "name");
        assert_eq!(folder_name("  ", "Agent"), "Agent");
        assert_eq!(folder_name("CON", "x"), "_CON");
        assert_eq!(folder_name("nul.txt", "x"), "_nul.txt");
        assert_eq!(folder_name(&"y".repeat(200), "x").chars().count(), 60);
        assert_eq!(folder_name("tab\there", "x"), "tab here");
    }

    #[test]
    fn ordinary_folders_of_the_owner_are_fine() {
        let (_d, base) = real_temp();
        let places = places_in(&base);
        let docs = base
            .join("Users")
            .join("you")
            .join("Documents")
            .join("Plenipo");
        std::fs::create_dir_all(&docs).unwrap();
        let got = place_problem(&docs.join("Acme Co").display().to_string(), &places).unwrap();
        assert_eq!(got, docs.join("Acme Co"));
        let work = base.join("Work");
        assert!(place_problem(&work.display().to_string(), &places).is_ok());
    }

    /// The reviewer's O6: Plenipo's data folder, the system's folders, Startup, the profile's
    /// and a drive's top folder, and paths that aren't full or have `.` or `..`, are refused.
    #[test]
    fn the_systems_places_and_odd_paths_are_refused() {
        let (_d, base) = real_temp();
        let places = places_in(&base);
        let refused = [
            base.join("data").display().to_string(),
            base.join("data")
                .join("organizations")
                .join("x")
                .display()
                .to_string(),
            base.join("Windows").display().to_string(),
            // Another letter case is the same folder where the system ignores case.
            base.join(if cfg!(any(windows, target_os = "macos")) {
                "WINDOWS"
            } else {
                "Windows"
            })
            .join("System32")
            .display()
            .to_string(),
            base.join("Program Files").join("App").display().to_string(),
            base.join("Startup").display().to_string(),
            base.join("Startup").join("x").display().to_string(),
            base.join("Users").join("you").display().to_string(),
            base.join("Users").display().to_string(),
            "relative\\folder".into(),
            String::new(),
            format!(
                "{}{}..{}x",
                base.display(),
                std::path::MAIN_SEPARATOR,
                std::path::MAIN_SEPARATOR
            ),
            format!("{}{}name.", base.display(), std::path::MAIN_SEPARATOR),
            format!("{}{}con", base.display(), std::path::MAIN_SEPARATOR),
            format!("{}{}a\nb", base.display(), std::path::MAIN_SEPARATOR),
            "\\\\?\\C:\\x".into(),
            "x".repeat(MAX_PLACE_CHARS + 1),
        ];
        for bad in refused {
            assert!(place_problem(&bad, &places).is_err(), "{bad:?}");
        }
        let top = if cfg!(windows) { "C:\\" } else { "/" };
        assert!(place_problem(top, &places).is_err());
        #[cfg(windows)]
        assert!(place_problem("\\\\server\\share", &places).is_err());
        #[cfg(windows)]
        assert!(place_problem("\\\\server\\share\\Acme", &places).is_ok());
    }

    /// The reviewer's O2: a junction (Windows) or a link anywhere below the profile's own folder
    /// is refused, whether it leads outside or back inside.
    #[test]
    fn a_junction_or_link_on_the_way_is_refused() {
        let (_d, base) = real_temp();
        let places = places_in(&base);
        let home = base.join("Users").join("you");
        let docs = home.join("Documents");
        let elsewhere = base.join("elsewhere");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();
        let out = docs.join("out");
        let inside = docs.join("inside");
        let real_inner = docs.join("real");
        std::fs::create_dir_all(&real_inner).unwrap();
        make_link(&elsewhere, &out);
        make_link(&real_inner, &inside);
        for linked in [out.join("Acme"), out.clone(), inside.join("Acme")] {
            let why = place_problem(&linked.display().to_string(), &places).unwrap_err();
            assert!(why.contains("shortcut"), "{linked:?}: {why}");
        }
        assert!(link_on_the_way(&out.join("x"), std::slice::from_ref(&home)).is_some());
        assert_eq!(link_on_the_way(&real_inner.join("x"), &[home]), None);
        // The folders above the profile's own are the system's: a link there isn't looked at.
        let via = base.join("via");
        make_link(&base.join("Users"), &via);
        assert_eq!(
            link_on_the_way(&via.join("you").join("Documents"), &[via.join("you")]),
            None
        );
    }

    /// The reviewer's S1 on #221: this PC's own drives through a network name (`\\localhost\C$`,
    /// `\\127.0.0.1\…`, this PC's name, WSL's) are refused before anything is looked at, and so
    /// is a whole drive's hidden share on any PC; a shared folder on another PC is fine.
    #[test]
    fn this_pcs_own_drives_through_a_network_name_are_refused() {
        let (_d, base) = real_temp();
        let places = places_in(&base);
        let this_pc = "That's this PC's own drive through a network name";
        let mut refused = vec![
            (r"\\localhost\C$\Work\Acme".to_owned(), this_pc),
            ("//127.0.0.1/C$/Work/Acme".to_owned(), this_pc),
            (r"\\127.1\share\Acme".to_owned(), this_pc),
            (r"\\[::1]\share\Acme".to_owned(), this_pc),
            (r"\\LOCALHOST@SSL@443\DavWWWRoot\Acme".to_owned(), this_pc),
            (r"\\wsl.localhost\Ubuntu\mnt\c\Work".to_owned(), this_pc),
            (r"\\wsl$\Ubuntu\mnt\c\Work".to_owned(), this_pc),
            (r"\\server\C$\Work\Acme".to_owned(), "hidden network share"),
            (r"\\server\admin$\Acme".to_owned(), "hidden network share"),
        ];
        if let Ok(name) = std::env::var("COMPUTERNAME") {
            refused.push((format!(r"\\{name}\share\Acme"), this_pc));
            refused.push((
                format!(r"\\{}.corp.example\share\Acme", name.to_lowercase()),
                this_pc,
            ));
        }
        // Plenipo's data folder and a Startup folder, reached through this PC's network names.
        if cfg!(windows) {
            let shown = base.display().to_string();
            let (drive, rest) = shown.split_at(2);
            let drive = drive.trim_end_matches(':');
            refused.push((format!(r"\\localhost\{drive}${rest}\data"), this_pc));
            refused.push((format!(r"\\127.0.0.1\{drive}${rest}\Startup\Acme"), this_pc));
        }
        for (bad, why) in refused {
            let got = place_problem(&bad, &places).unwrap_err();
            assert!(got.contains(why), "{bad}: {got}");
        }
        #[cfg(windows)]
        assert!(place_problem(r"\\server\share\Acme", &places).is_ok());
    }

    /// The reviewer's S2 on #221: places are compared part by part, so a long path (which comes
    /// back as `\\?\C:\…`), `/` instead of `\`, and other letter case can't slip past.
    #[test]
    fn places_are_compared_part_by_part() {
        let (_d, base) = real_temp();
        let places = places_in(&base);
        // An existing folder inside the data folder, its path well over 260 characters.
        let mut deep = base.join("data");
        while deep.as_os_str().len() < 464 {
            deep.push("a-rather-long-folder-name");
        }
        std::fs::create_dir_all(&deep).unwrap();
        let slashed = deep.display().to_string().replace('\\', "/");
        let why = place_problem(&slashed, &places).unwrap_err();
        assert!(why.contains("Plenipo's own data folder"), "{why}");
        let why = place_problem(&deep.display().to_string(), &places).unwrap_err();
        assert!(why.contains("Plenipo's own data folder"), "{why}");
        if cfg!(windows) {
            let mixed = format!(r"\\?/{}", deep.display());
            assert!(place_problem(&mixed, &places).is_err());
            assert!(same_place(
                Path::new(r"\\?\C:\Work\Acme"),
                Path::new("c:/work/ACME")
            ));
            assert!(within(
                Path::new(r"\\?\UNC\server\share\Acme\Files"),
                Path::new(r"\\SERVER\share\acme")
            ));
            assert!(!within(Path::new(r"C:\Workshop"), Path::new(r"C:\Work")));
            assert!(!same_place(Path::new(r"D:\Work"), Path::new(r"C:\Work")));
        }
    }

    /// The reviewer's N1 on #221: a share is kept as `\\server\share\…`, which File Explorer
    /// opens.
    #[test]
    fn a_share_is_kept_the_usual_way() {
        assert_eq!(
            usual_form(PathBuf::from(r"\\?\UNC\server\share\Acme")),
            PathBuf::from(r"\\server\share\Acme")
        );
        let long = format!(r"\\?\UNC\server\share\{}", "a".repeat(300));
        assert_eq!(usual_form(PathBuf::from(&long)), PathBuf::from(&long));
        assert_eq!(
            usual_form(PathBuf::from(r"C:\Work")),
            PathBuf::from(r"C:\Work")
        );
    }

    /// The reviewer's N3 on #221: a file, and a drive this PC hasn't, are refused in plain words.
    #[test]
    fn a_file_or_a_missing_drive_is_refused_in_plain_words() {
        let (_d, base) = real_temp();
        let places = places_in(&base);
        let file = base.join("notes.txt");
        std::fs::write(&file, "mine").unwrap();
        for bad in [file.clone(), file.join("Acme")] {
            let why = place_problem(&bad.display().to_string(), &places).unwrap_err();
            assert!(why.contains("is a file, not a folder"), "{bad:?}: {why}");
        }
        let unused = ('D'..='Z')
            .rev()
            .find(|l| !Path::new(&format!("{l}:\\")).exists());
        if let (true, Some(letter)) = (cfg!(windows), unused) {
            let why = place_problem(&format!(r"{letter}:\Work\Acme"), &places).unwrap_err();
            assert!(why.contains("can't find"), "{why}");
        }
    }

    #[cfg(windows)]
    fn make_link(target: &Path, link: &Path) {
        // A junction needs no special rights, unlike a symbolic link.
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        assert!(std::fs::symlink_metadata(link)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[cfg(unix)]
    fn make_link(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    #[test]
    fn who_syncs_a_folder() {
        let (_d, base) = real_temp();
        let onedrive = vec![base.join("OneDrive - GG IT")];
        assert_eq!(
            synced_by(&base.join("OneDrive - GG IT").join("Documents"), &onedrive),
            Some(SyncedBy::OneDrive)
        );
        assert_eq!(
            synced_by(&base.join("ONEDRIVE - gg it").join("x"), &onedrive),
            if cfg!(any(windows, target_os = "macos")) {
                Some(SyncedBy::OneDrive)
            } else {
                None
            }
        );
        assert_eq!(
            synced_by(&base.join("Dropbox").join("Acme"), &onedrive),
            Some(SyncedBy::Other)
        );
        assert_eq!(synced_by(&base.join("Documents"), &onedrive), None);
    }

    /// Plenipo reads OneDrive's "Always keep on this device" mark from the folder or the nearest
    /// folder above it; outside Windows it can't tell.
    #[test]
    fn kept_on_this_device_reads_the_nearest_mark() {
        let (_d, base) = real_temp();
        let folder = base.join("Documents").join("Plenipo").join("Acme");
        std::fs::create_dir_all(&folder).unwrap();
        let got = kept_on_this_device(&folder, &base);
        if cfg!(windows) {
            // A folder outside OneDrive carries no mark.
            assert_eq!(got, Some(false));
        } else {
            assert_eq!(got, None);
        }
    }
}
