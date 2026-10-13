// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Whether this process runs under a chat's sandbox, read from the sandbox itself (#1408).
//!
//! Every chat the app starts under the sandbox is denied writing the project's manifests
//! ([`super::manifests_held`], ADR 0067 §5), and no chat is started where the manifest is not
//! there ([`super::NotStarted::PlaneMissing`]). So a process that cannot open an existing
//! manifest for writing, where its permissions would let it, is held by a sandbox. Nothing is
//! written: the manifest is opened to append, never created or cut, and closed at once.
//!
//! The manifests at the project's root and in every folder above it are asked, so a project
//! inside another project's tree is held by the outer one's too.

use std::io;
use std::path::{Path, PathBuf};

/// Whether this process may not write a project manifest at `root` or in a folder above it,
/// where its permissions would let it: the sign that a chat's sandbox holds it. `false` where
/// there is none to ask, and on a system purlis has no sandbox backend for.
pub fn writes_held_at(root: &Path) -> bool {
    manifests_at_and_above(root)
        .iter()
        .any(|manifest| held(manifest))
}

/// The manifests there are now at `root` and in each folder above it, as the folder is spelled
/// and as the kernel names it. A name whose kind cannot be read is listed too, so the doubt is
/// asked rather than passed over.
pub fn manifests_at_and_above(root: &Path) -> Vec<PathBuf> {
    files_at_and_above(root, &super::MANIFESTS)
}

/// [`manifests_at_and_above`], for the files named `names`.
fn files_at_and_above(root: &Path, names: &[&str]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for spelled in [root.to_path_buf(), super::real(root)] {
        for folder in spelled.ancestors() {
            for name in names {
                let path = folder.join(name);
                let there = match std::fs::metadata(&path) {
                    Ok(meta) => meta.is_file(),
                    Err(e) => !matches!(
                        e.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                    ),
                };
                if there && !out.contains(&path) {
                    out.push(path);
                }
            }
        }
    }
    out
}

/// Whether this process may not open `path`, an existing file, for writing, where its
/// permissions would let it.
pub fn held(path: &Path) -> bool {
    let opened = std::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .map(drop);
    verdict(opened, || permits_writing(path))
}

/// Whether the file's permissions let this process write it, as the kernel checks them.
#[cfg(unix)]
fn permits_writing(path: &Path) -> bool {
    rustix::fs::access(path, rustix::fs::Access::WRITE_OK).is_ok()
}

/// No backend here (ruling V21, 3): a refusal is only the permissions'.
#[cfg(not(unix))]
fn permits_writing(_path: &Path) -> bool {
    false
}

/// What opening a file for writing says of a sandbox: refused by something other than the
/// file's permissions (`permits`) is held. **Every doubt is held**: an error that is neither
/// the file gone nor the permissions' own is never read as free to write.
///
/// - `EPERM`: Seatbelt's refusal, and an immutable file's.
/// - `EROFS`: a path bubblewrap mounts read-only.
/// - `EACCES` where the permissions allow writing: Landlock's refusal.
pub(crate) fn verdict(opened: io::Result<()>, permits: impl FnOnce() -> bool) -> bool {
    let Err(e) = opened else {
        return false;
    };
    if matches!(
        e.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    ) {
        return false;
    }
    if is_eacces(&e) {
        return permits();
    }
    true
}

#[cfg(unix)]
fn is_eacces(e: &io::Error) -> bool {
    e.raw_os_error() == Some(libc::EACCES)
}

#[cfg(not(unix))]
fn is_eacces(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::PermissionDenied
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(code: i32) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(code))
    }

    #[test]
    fn a_file_opened_for_writing_is_not_held() {
        assert!(!verdict(Ok(()), || panic!("the permissions were asked")));
    }

    #[cfg(unix)]
    #[test]
    fn a_refusal_the_sandbox_makes_is_held() {
        // Seatbelt's, and bubblewrap's read-only mount.
        assert!(verdict(os(libc::EPERM), || true));
        assert!(verdict(os(libc::EROFS), || true));
        // Landlock's: the permissions allow it, and the kernel still refuses.
        assert!(verdict(os(libc::EACCES), || true));
    }

    #[cfg(unix)]
    #[test]
    fn a_file_whose_permissions_refuse_writing_is_not_held() {
        // A person's terminal with a read-only manifest is not taken for a chat.
        assert!(!verdict(os(libc::EACCES), || false));
    }

    #[cfg(unix)]
    #[test]
    fn a_file_gone_since_it_was_listed_is_not_held_and_any_other_doubt_is() {
        assert!(!verdict(os(libc::ENOENT), || true));
        assert!(!verdict(os(libc::ENOTDIR), || true));
        assert!(verdict(os(libc::EIO), || true));
        assert!(verdict(os(libc::ELOOP), || true));
    }

    #[test]
    fn a_file_this_process_may_write_is_not_held_and_is_left_as_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.toml");
        std::fs::write(&file, "[sandbox]\n").unwrap();
        let before = std::fs::metadata(&file).unwrap().modified().unwrap();

        assert!(!held(&file));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "[sandbox]\n");
        assert_eq!(
            std::fs::metadata(&file).unwrap().modified().unwrap(),
            before
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_file_only_its_permissions_keep_from_writing_is_not_held() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.toml");
        std::fs::write(&file, "").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o444)).unwrap();
        // Root writes any file whatever its permissions, so there is nothing to tell apart.
        if rustix::process::geteuid().is_root() {
            return;
        }
        assert!(!held(&file));
    }

    /// A file the kernel refuses writing with `EPERM` whatever its permissions, as a sandbox
    /// refuses it: macOS's user-immutable flag, which the file's owner sets and clears.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_file_the_kernel_refuses_whatever_its_permissions_is_held() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.toml");
        std::fs::write(&file, "[sandbox]\n").unwrap();
        let flags = |flag: &str| {
            let done = crate::forklock::status(
                std::process::Command::new("/usr/bin/chflags")
                    .arg(flag)
                    .arg(&file),
            )
            .unwrap();
            assert!(done.success(), "chflags {flag}");
        };
        flags("uchg");
        let was = held(&file);
        flags("nouchg");
        assert!(was, "a file the kernel refuses writing was not held");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "[sandbox]\n");
    }

    #[test]
    fn the_files_at_and_above_the_root_are_asked_and_nothing_else() {
        // A name a test may write stands for the manifests' (this walk is theirs).
        let names = ["settings.toml"];
        let dir = tempfile::tempdir().unwrap();
        let outer = dir.path().join("outer");
        let inner = outer.join("workspaces/alpha/clone");
        std::fs::create_dir_all(inner.join("workspaces/beta")).unwrap();
        assert!(files_at_and_above(&inner, &names).is_empty());

        // Below the root, a folder of that name, and a file of another name: none asked.
        std::fs::write(inner.join("workspaces/beta/settings.toml"), "").unwrap();
        std::fs::create_dir_all(outer.join("workspaces/settings.toml")).unwrap();
        std::fs::write(inner.join("other.toml"), "").unwrap();
        assert!(files_at_and_above(&inner, &names).is_empty());

        // At the root and in a folder above it.
        std::fs::write(inner.join("settings.toml"), "").unwrap();
        std::fs::write(outer.join("settings.toml"), "").unwrap();
        let found: Vec<PathBuf> = files_at_and_above(&inner, &names)
            .into_iter()
            .map(|path| super::super::real(&path))
            .collect();
        let real = |path: PathBuf| super::super::real(&path);
        assert!(
            found.contains(&real(inner.join("settings.toml"))),
            "{found:?}"
        );
        assert!(
            found.contains(&real(outer.join("settings.toml"))),
            "{found:?}"
        );
        assert!(!writes_held_at(&inner));
    }
}
