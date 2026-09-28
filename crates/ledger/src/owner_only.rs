//! Files and folders readable by the owner's account only. On Unix (Linux is used for
//! development and CI) the Ledger's folder, its database, its backups, its exports, and the
//! app's data folder around them are made `0700` (folders) and `0600` (files), so another account
//! on the same computer sees nothing of the owner's activity history. On Windows there is nothing
//! to set: the app's data folder under `%LOCALAPPDATA%` is private to the account already through
//! its access control list, and files have no mode bits.
//!
//! A folder made private protects everything in it, so a file's own mode is a second wall: the
//! callers set it where they can and go on when they cannot (a disk without Unix modes).

use std::path::Path;

/// Make `dir` readable, writable, and listable by the owner's account only (nothing to do when it
/// is already).
pub fn folder(dir: &Path) -> std::io::Result<()> {
    set_mode(dir, 0o700)
}

/// Make `file` readable and writable by the owner's account only (nothing to do when it is
/// already).
pub fn file(file: &Path) -> std::io::Result<()> {
    set_mode(file, 0o600)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let now = std::fs::metadata(path)?.permissions().mode() & 0o777;
    if now & 0o077 == 0 {
        return Ok(());
    }
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> std::io::Result<()> {
    // Windows: the account's own app-data folder is private already (its access control list
    // names the account, the system, and the administrators), and files carry no mode bits.
    Ok(())
}
