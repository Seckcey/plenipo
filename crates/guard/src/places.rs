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

use std::path::{Component, Path, PathBuf};

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

/// Whether two paths are the same place, ignoring letter case where the system does (Windows and
/// a Mac's usual disk).
fn same(a: &Path, b: &Path) -> bool {
    if cfg!(any(windows, target_os = "macos")) {
        a.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())
    } else {
        a == b
    }
}

/// `path` is `place` or inside it.
fn within(path: &Path, place: &Path) -> bool {
    path.ancestors().any(|a| same(a, place))
}

/// Where a place really is: links followed for the part that exists, the rest added as written.
/// Places that don't exist are kept as written.
fn real(path: &Path) -> PathBuf {
    let mut existing = path.to_path_buf();
    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if let Ok(found) = dunce::canonicalize(&existing) {
            let mut out = found;
            for name in rest.iter().rev() {
                out.push(name);
            }
            return out;
        }
        match (
            existing.file_name().map(|n| n.to_os_string()),
            existing.parent(),
        ) {
            (Some(name), Some(parent)) => {
                rest.push(name);
                existing = parent.to_path_buf();
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// The first part of `path` (an absolute path) that is a junction or a link, from the top down,
/// leaving out the folders above (or at) any of `trusted`, which are the system's. `None` when
/// there is none, or when the parts that exist end before one.
pub fn link_on_the_way(path: &Path, trusted: &[PathBuf]) -> Option<PathBuf> {
    let mut parts: Vec<&Path> = path.ancestors().collect();
    parts.reverse();
    for part in parts {
        if trusted.iter().any(|t| within(t, part)) {
            continue;
        }
        match std::fs::symlink_metadata(part) {
            Ok(meta) if meta.file_type().is_symlink() => return Some(part.to_path_buf()),
            Ok(_) => {}
            Err(_) => return None,
        }
    }
    None
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
/// drive's or a network share's top folder; and a path with a junction or link on the way.
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
    if shown.starts_with("\\\\?\\")
        || shown.starts_with("\\\\.\\")
        || shown.starts_with("//?/")
        || shown.starts_with("//./")
    {
        return Err("Write the folder's path the usual way, such as D:\\Work\\Acme.".into());
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
    if let Some(link) = link_on_the_way(p, &places.trusted()) {
        return Err(format!(
            "{} is a shortcut to another place (a junction or a link). Choose the folder it \
             points to, or another one.",
            link.display()
        ));
    }
    let real_path = real(p);
    if is_top(&real_path) {
        return Err(
            "That's the top of a drive. Choose a folder inside it, such as a folder in Documents."
                .into(),
        );
    }
    let real_place = |place: &Path| real(place);
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
        if same(&real_path, &real_profile)
            || same(p, profile)
            || users.as_deref().is_some_and(|u| same(&real_path, u))
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
            if same(at, root) {
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
            base.join("WINDOWS").join("System32").display().to_string(),
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
