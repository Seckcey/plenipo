//! Where a Mac or a Linux PC keeps the programs Plenipo runs (Phase 23, ADR-150).
//!
//! An app opened from the Mac's Dock, or from many Linux desktops' menus, starts with only the
//! system's own folders on its PATH (`/usr/bin:/bin:/usr/sbin:/sbin`), not the ones the owner's
//! terminal adds: Homebrew, npm's global folder, nvm, Volta, fnm, bun, and the owner's own
//! `~/.local/bin`. AI tools installed there would not be found, and tools installed with npm would
//! not start, since they start through `node`. So Plenipo adds the folders from this fixed list
//! that exist, after the ones it was given. It never takes them from a login shell's settings, so
//! Guard always knows where a program came from. Windows needs none of this.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// Add the known program folders that exist to Plenipo's own PATH. Call once, first thing in
/// `main`, before any thread starts (it changes the process's environment). Does nothing on
/// Windows.
pub fn extend_path() {
    if cfg!(windows) {
        return;
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let given = std::env::var_os("PATH");
    let extended = with_known_dirs(given.as_deref(), &known_dirs(home.as_deref()));
    if Some(&extended) != given.as_ref() {
        std::env::set_var("PATH", extended);
    }
}

/// `path` with each folder of `extra` that is not already on it added at the end, in order.
pub fn with_known_dirs(path: Option<&OsStr>, extra: &[PathBuf]) -> OsString {
    let mut dirs: Vec<PathBuf> = path
        .map(|p| std::env::split_paths(p).collect())
        .unwrap_or_default();
    for dir in extra {
        if !dirs.contains(dir) {
            dirs.push(dir.clone());
        }
    }
    std::env::join_paths(dirs).unwrap_or_else(|_| path.map(OsStr::to_owned).unwrap_or_default())
}

/// The known program folders that exist on this computer, for the owner whose home is `home`.
pub fn known_dirs(home: Option<&Path>) -> Vec<PathBuf> {
    candidates(home)
        .into_iter()
        .filter(|d| d.is_dir())
        .collect()
}

/// Every known program folder, whether it exists or not, in the order they are added.
fn candidates(home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = [
        // Homebrew on a Mac with Apple's chips; on an Intel Mac it uses /usr/local.
        "/opt/homebrew/bin",
        "/opt/homebrew/sbin",
        "/usr/local/bin",
        "/usr/local/sbin",
        // The system's own, in case PATH was empty.
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
        // Snap packages on Ubuntu.
        "/snap/bin",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    if let Some(home) = home {
        for dir in [
            ".local/bin",
            "bin",
            ".cargo/bin",
            ".volta/bin",
            ".bun/bin",
            ".npm-global/bin",
            ".local/share/fnm/aliases/default/bin",
            "Library/Application Support/fnm/aliases/default/bin",
        ] {
            dirs.push(home.join(dir));
        }
        if let Some(nvm) = nvm_default(home) {
            dirs.push(nvm);
        }
    }
    dirs
}

/// The `bin` folder of the Node.js version nvm uses by default: the one its `default` alias
/// names exactly, or the newest installed one it names (`22` → `v22.10.3`; `node` or `stable` →
/// the newest of all). `None` when nvm is not installed or the alias names none installed.
fn nvm_default(home: &Path) -> Option<PathBuf> {
    let nvm = home.join(".nvm");
    let alias = std::fs::read_to_string(nvm.join("alias").join("default")).ok()?;
    let alias = alias.trim().trim_start_matches('v');
    let any = matches!(alias, "node" | "stable");
    let versions = nvm.join("versions").join("node");
    std::fs::read_dir(&versions)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let number = name.strip_prefix('v')?;
            let fits = any || number == alias || number.starts_with(&format!("{alias}."));
            let parts: Vec<u64> = number
                .split('.')
                .map(str::parse)
                .collect::<Result<_, _>>()
                .ok()?;
            fits.then_some((parts, entry.path().join("bin")))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, bin)| bin)
        .filter(|bin| bin.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_folders_come_after_the_given_ones_and_only_once() {
        let a = PathBuf::from(if cfg!(windows) { r"C:\a" } else { "/a" });
        let b = PathBuf::from(if cfg!(windows) { r"C:\b" } else { "/b" });
        let given = std::env::join_paths([&a, &b]).unwrap();
        let joined = with_known_dirs(
            Some(&given),
            &[
                b.clone(),
                PathBuf::from(if cfg!(windows) { r"C:\c" } else { "/c" }),
            ],
        );
        let dirs: Vec<PathBuf> = std::env::split_paths(&joined).collect();
        assert_eq!(dirs.len(), 3);
        assert_eq!(dirs[0], a);
        assert_eq!(dirs[1], b);
        let only = with_known_dirs(None, std::slice::from_ref(&a));
        assert_eq!(std::env::split_paths(&only).collect::<Vec<_>>(), vec![a]);
    }

    #[test]
    fn only_folders_that_exist_are_added() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join(".volta").join("bin")).unwrap();
        let dirs = known_dirs(Some(home.path()));
        assert!(dirs.contains(&home.path().join(".volta").join("bin")));
        assert!(!dirs.contains(&home.path().join(".bun").join("bin")));
    }

    #[test]
    fn nvm_s_default_is_the_newest_version_it_names() {
        let home = tempfile::tempdir().unwrap();
        let nvm = home.path().join(".nvm");
        for version in ["v20.11.0", "v22.1.0", "v22.10.3", "v220.0.0"] {
            std::fs::create_dir_all(nvm.join("versions").join("node").join(version).join("bin"))
                .unwrap();
        }
        std::fs::create_dir_all(nvm.join("alias")).unwrap();
        let bin = |v: &str| nvm.join("versions").join("node").join(v).join("bin");
        for (alias, want) in [
            ("22\n", Some(bin("v22.10.3"))),
            ("v22.1.0", Some(bin("v22.1.0"))),
            ("20", Some(bin("v20.11.0"))),
            ("node", Some(bin("v220.0.0"))),
            ("18", None),
            ("lts/*", None),
        ] {
            std::fs::write(nvm.join("alias").join("default"), alias).unwrap();
            assert_eq!(nvm_default(home.path()), want, "{alias:?}");
        }
        assert_eq!(nvm_default(&home.path().join("nobody")), None);
    }
}
