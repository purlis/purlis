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
//!
//! **What a delete would have to remove is asked too** ([`deletion_held_within`]). Every
//! sandboxed chat is denied writing the later-code names wherever it may write
//! ([`super::PLANTED`]: a clone's git config, a branch folder's `.git`, a harness's project
//! config), and a delete that meets one stops there, half done. A folder holding one this
//! process may not write is one the sandbox would stop the delete in, whichever project it is
//! in.
//!
//! **And what a checkout would have to write** ([`checkout_held`], qw346). git checks a tree
//! out file by file, writing each it may and skipping each it may not, so a move of the
//! project's tree to another commit that changes a file this process may not write ends half
//! done: most files as the other commit has them, the rest and HEAD as they were.

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

/// Whether this process may not write a file below `target` that deleting `target` would have
/// to remove: the sign that a chat's sandbox would stop that delete partway (#1408).
pub fn deletion_held_within(target: &Path) -> bool {
    held_below(target, &later_code_files())
}

/// The later-code names a chat is denied writing wherever it may write ([`super::PLANTED`])
/// that name one file to ask.
fn later_code_files() -> Vec<&'static str> {
    super::PLANTED
        .iter()
        .map(|planted| planted.path)
        .filter(|path| !path.contains("**"))
        .collect()
}

/// [`deletion_held_within`], asking the files named `names` in each folder of the walk
/// ([`super::planted::directories`]: bounded, following no link). Only a plain file is asked:
/// a link, a folder or anything else of the name is passed over, and so is a `target` that is
/// itself a link, which a delete refuses on its own.
fn held_below(target: &Path, names: &[&str]) -> bool {
    if !std::fs::symlink_metadata(target).is_ok_and(|meta| meta.is_dir()) {
        return false;
    }
    super::planted::directories(target).iter().any(|folder| {
        names.iter().any(|name| {
            let path = folder.join(name);
            std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_file())
                && held_opening(&path, NOT_THROUGH_A_LINK)
        })
    })
}

/// Whether this process runs under a chat's sandbox in the project at `root`: the app says so
/// in its environment ([`super::chat_is_sandboxed`]), or the sandbox itself holds the project's
/// manifests ([`writes_held_at`]).
pub fn in_a_sandbox(root: &Path) -> bool {
    #[cfg(test)]
    if STOOD_IN.with(|names| names.borrow().is_some()) {
        return true;
    }
    super::chat_is_sandboxed() || writes_held_at(root)
}

/// The files among `changed`, each relative to the project at `root`, that a checkout would
/// have to write and this process may not: the sign that a chat's sandbox would stop a move of
/// the tree partway (qw346). Empty outside a sandbox ([`in_a_sandbox`]).
///
/// A file that is there is asked of the kernel, as [`held`] asks, never through a link. One
/// that is not there yet cannot be asked without making it, so it is judged by its name: a
/// later-code name, or a manifest in the root folder ([`crate::rootupdate::path_is_denied`]).
pub fn checkout_held(root: &Path, changed: &[String]) -> Vec<String> {
    #[cfg(test)]
    if let Some(names) = STOOD_IN.with(|names| names.borrow().clone()) {
        return changed
            .iter()
            .filter(|path| names.contains(path))
            .cloned()
            .collect();
    }
    if !in_a_sandbox(root) {
        return Vec::new();
    }
    changed
        .iter()
        .filter(|rel| {
            let path = root.join(rel);
            let there = std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_file());
            (there && held_opening(&path, NOT_THROUGH_A_LINK))
                || crate::rootupdate::path_is_denied(rel, root)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
thread_local! {
    /// The files a test says a sandbox denies this thread, standing in for one: a test cannot
    /// put itself under a sandbox, and one run inside a chat's is denied the real names.
    static STOOD_IN: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

/// For a test: this thread is under a sandbox that denies it writing `names`, until the answer
/// is dropped.
#[cfg(test)]
pub(crate) fn stand_in(names: &[&str]) -> StoodIn {
    STOOD_IN.with(|held| {
        *held.borrow_mut() = Some(names.iter().map(|name| (*name).to_string()).collect());
    });
    StoodIn
}

/// [`stand_in`]'s sandbox, lifted when dropped.
#[cfg(test)]
pub(crate) struct StoodIn;

#[cfg(test)]
impl Drop for StoodIn {
    fn drop(&mut self) {
        STOOD_IN.with(|held| *held.borrow_mut() = None);
    }
}

/// Whether this process may not open `path`, an existing file, for writing, where its
/// permissions would let it.
pub fn held(path: &Path) -> bool {
    held_opening(path, 0)
}

/// Opened in the walk below a delete: a name swapped for a link since it was looked at is not
/// followed (`ELOOP`, held).
#[cfg(unix)]
const NOT_THROUGH_A_LINK: i32 = libc::O_NOFOLLOW;
#[cfg(not(unix))]
const NOT_THROUGH_A_LINK: i32 = 0;

/// [`held`], opening with the further flags `extra`.
fn held_opening(path: &Path, extra: i32) -> bool {
    let mut options = std::fs::OpenOptions::new();
    options.append(true);
    // Never waits, and never takes a terminal for this process's own: a name swapped for a
    // pipe or a device since it was looked at is refused at once (`ENXIO`), and that doubt is
    // held.
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::custom_flags(
        &mut options,
        libc::O_NONBLOCK | libc::O_NOCTTY | extra,
    );
    #[cfg(not(unix))]
    let _ = extra;
    let opened = options.open(path).map(drop);
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

    /// Names a test may write, standing for the later-code names (the walk is theirs).
    const STAND_INS: [&str; 2] = ["held.toml", "conf/held.toml"];

    #[test]
    fn a_tree_whose_files_this_process_may_write_is_not_held_and_is_left_as_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let tree = dir.path().join("beta");
        std::fs::create_dir_all(tree.join("clone/conf")).unwrap();
        let file = tree.join("clone/conf/held.toml");
        std::fs::write(&file, "[core]\n").unwrap();
        let before = std::fs::metadata(&file).unwrap().modified().unwrap();

        assert!(!held_below(&tree, &STAND_INS));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "[core]\n");
        assert_eq!(
            std::fs::metadata(&file).unwrap().modified().unwrap(),
            before
        );
        // Nothing there to delete: nothing held.
        assert!(!held_below(&dir.path().join("gone"), &STAND_INS));
    }

    /// A delete the sandbox would stop partway: a file below it the kernel refuses writing
    /// whatever its permissions (macOS's user-immutable flag, standing for a sandbox's refusal).
    #[cfg(target_os = "macos")]
    #[test]
    fn a_tree_holding_a_file_the_kernel_refuses_is_held_at_any_depth_the_walk_reaches() {
        let dir = tempfile::tempdir().unwrap();
        let tree = dir.path().join("beta");
        for (at, name) in [("", "held.toml"), ("clone", "conf/held.toml")] {
            let folder = tree.join(at);
            std::fs::create_dir_all(folder.join("conf")).unwrap();
            let file = folder.join(name);
            std::fs::write(&file, "").unwrap();
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
            let was = held_below(&tree, &STAND_INS);
            flags("nouchg");
            std::fs::remove_file(&file).unwrap();
            assert!(was, "{name} in '{at}' was not held");
        }
    }

    /// A link is never followed: what it points at is not what the delete removes.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_link_in_the_tree_or_to_it_is_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        let file = elsewhere.join("held.toml");
        std::fs::write(&file, "").unwrap();
        let tree = dir.path().join("beta");
        std::fs::create_dir_all(tree.join("clone")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, tree.join("linked")).unwrap();
        std::os::unix::fs::symlink(&file, tree.join("clone/held.toml")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, dir.path().join("a-link")).unwrap();
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
        let in_tree = held_below(&tree, &STAND_INS);
        let to_it = held_below(&dir.path().join("a-link"), &STAND_INS);
        flags("nouchg");
        assert!(!in_tree, "a link in the tree was followed");
        assert!(!to_it, "a link to the tree was followed");
    }

    /// A name swapped for a link after the walk looked at it is not opened through the link:
    /// that doubt is held, and the file it points at is left as it was.
    #[cfg(unix)]
    #[test]
    fn the_walk_opens_no_file_through_a_link() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("elsewhere.toml");
        std::fs::write(&file, "[core]\n").unwrap();
        let link = dir.path().join("held.toml");
        std::os::unix::fs::symlink(&file, &link).unwrap();

        assert!(held_opening(&link, NOT_THROUGH_A_LINK));
        assert!(
            !held(&link),
            "a manifest reached through a link is still asked"
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "[core]\n");
    }

    #[test]
    fn outside_a_sandbox_no_checkout_is_held_and_a_stood_in_one_holds_only_what_it_names() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "").unwrap();
        let changed = vec!["a.md".to_string(), "held.toml".to_string()];
        // No manifest here to ask, and no app said this is a chat.
        if !crate::sandbox::chat_is_sandboxed() {
            assert!(!in_a_sandbox(dir.path()));
            assert!(checkout_held(dir.path(), &changed).is_empty());
        }
        {
            let _sandbox = stand_in(&["held.toml"]);
            assert!(in_a_sandbox(dir.path()));
            assert_eq!(checkout_held(dir.path(), &changed), ["held.toml"]);
        }
        assert!(STOOD_IN.with(|names| names.borrow().is_none()));
    }

    /// A file the kernel refuses writing whatever its permissions stands for a sandbox's
    /// refusal: a checkout that changes it is held at it, and at a name the sandbox denies
    /// that is not there yet, and at nothing else.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_checkout_is_held_at_the_files_the_kernel_refuses_and_the_names_not_there_yet() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // A name a test may write stands for the manifest: what puts this process "in a
        // sandbox" is asked through `writes_held_at`, which a test cannot make true, so the
        // files are asked directly.
        let file = root.join("settings.toml");
        std::fs::write(&file, "").unwrap();
        std::fs::write(root.join("a.md"), "").unwrap();
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
        let there = std::fs::symlink_metadata(&file).is_ok_and(|meta| meta.is_file());
        let was = there && held_opening(&file, NOT_THROUGH_A_LINK);
        let free = held_opening(&root.join("a.md"), NOT_THROUGH_A_LINK);
        flags("nouchg");
        assert!(was, "a file the kernel refuses writing was not held");
        assert!(!free, "a file this process may write was held");
        // Not there yet: judged by its name.
        assert!(crate::rootupdate::path_is_denied(
            ".claude/skills/x/SKILL.md",
            root
        ));
        assert!(!crate::rootupdate::path_is_denied("docs/new.md", root));
    }

    #[test]
    fn the_later_code_names_asked_below_a_delete_are_the_planted_ones() {
        let names = later_code_files();
        for name in [".git", ".git/config", ".claude/settings.json", ".mcp.json"] {
            assert!(names.contains(&name), "{name}: {names:?}");
        }
        // A pattern names no one file to ask.
        assert!(names.iter().all(|name| !name.contains("**")), "{names:?}");
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
