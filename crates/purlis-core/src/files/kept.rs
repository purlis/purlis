// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the plane root's files leave out** (issue 1916; rulings G3, F1 and F2 of 2026-10-13).
//!
//! The plane root is the one place whose folder holds more than a repo's files: purlis's own
//! state folder is in it, and so is every workspace's clone. Two things are decided here, and
//! nowhere else, for every read of that place — the tree, a file's open, a path placed or
//! named, a folder watched, a link followed:
//!
//! - **What purlis keeps to itself is neither listed nor opened** (F1): the vaults and their
//!   keys, and the records of purlis's own that no chat reads. The window's exclusion list and
//!   the sandbox's read denials are one list ([`crate::sandbox::kept_from_reading`]), so the
//!   window shows a person nothing a chat is kept from. Where that list cannot be told whole —
//!   the vault registry does not read — nothing of the plane root is listed or opened.
//! - **A workspace's clone is one row** (F2): its files are that workspace's, read on its own
//!   tab, and nothing at or below the clone is a folder or a file of the plane root.
//!
//! **A path is judged by how it is spelled and by what it is.** By spelling first, without
//! regard to case or to how its letters are composed, and before the disk is asked, so a
//! refusal never says whether something is there. Then by identity: a folder or file that IS
//! one of the kept ones, however its name is written on a file system that folds names
//! together. A file with more than one name is refused when one of its names is in a kept
//! folder.
//!
//! Every other branch reads as it did: [`Kept::of`] answers a guard that keeps nothing.

use std::path::{Component, Path, PathBuf};

use super::{Branch, Refused};

/// The most entries of the kept folders [`Kept::holds_a_name_of`] looks at for a file with
/// several names. Past it the file is refused: purlis could not tell, so it does not open.
const WALKED: usize = 20_000;

/// What a read of one place leaves out: nothing, but at the plane root.
#[derive(Debug)]
pub(super) struct Kept {
    /// Whether this is the plane root's guard. Every other place keeps nothing.
    on: bool,
    /// The place's folder, resolved.
    base: PathBuf,
    /// What purlis keeps to itself: absolute paths, as written and as the kernel names them.
    private: Vec<PathBuf>,
    /// The identity of each of those that is there.
    ids: Vec<Id>,
    /// The project's workspaces, by name.
    workspaces: Vec<String>,
    /// Why nothing of the place is read at all: the list of what purlis keeps could not be
    /// told whole. `None` when it could.
    unread: Option<String>,
    /// Why nothing under `workspaces/` is read: the project's workspaces could not be listed,
    /// so a clone cannot be told from a folder of the project's own.
    unlisted: Option<String>,
}

/// A file or folder as the file system knows it, whatever it is called.
#[cfg(unix)]
type Id = (u64, u64);
#[cfg(not(unix))]
type Id = ();

#[cfg(unix)]
fn id_of(meta: &std::fs::Metadata) -> Option<Id> {
    use std::os::unix::fs::MetadataExt as _;
    Some((meta.dev(), meta.ino()))
}

#[cfg(not(unix))]
fn id_of(_meta: &std::fs::Metadata) -> Option<Id> {
    None
}

/// How many names a file has.
#[cfg(unix)]
fn names_of(meta: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt as _;
    meta.nlink()
}

#[cfg(not(unix))]
fn names_of(_meta: &std::fs::Metadata) -> u64 {
    1
}

/// One step of a path as it is compared: composed, and in one case, so `.PURLIS` and a name
/// whose letters are composed differently are the folder they reach on a file system that
/// folds them together. Folding where the file system does not only keeps more.
fn folded(step: Component<'_>) -> String {
    use unicode_normalization::UnicodeNormalization as _;
    step.as_os_str()
        .to_string_lossy()
        .nfc()
        .collect::<String>()
        .to_lowercase()
}

/// Whether `path` is `root` or under it, by [`folded`] spelling.
fn under(path: &Path, root: &Path) -> bool {
    let mut steps = path.components();
    root.components()
        .all(|want| steps.next().is_some_and(|got| folded(got) == folded(want)))
}

impl Kept {
    /// The guard of `branch`, whose folder is resolved to `base`, on this machine.
    pub(super) fn of(base: &Path, branch: Branch<'_>) -> Self {
        if branch.is_plane_root() {
            Self::at_the_root(base, &crate::sandbox::Machine::this())
        } else {
            Self::nothing(base)
        }
    }

    /// A guard that keeps nothing: every branch but the plane root.
    pub(super) fn nothing(base: &Path) -> Self {
        Self {
            on: false,
            base: base.to_path_buf(),
            private: Vec::new(),
            ids: Vec::new(),
            workspaces: Vec::new(),
            unread: None,
            unlisted: None,
        }
    }

    /// The plane root's guard, the project's folder resolved to `base`, on `machine`.
    pub(super) fn at_the_root(base: &Path, machine: &crate::sandbox::Machine) -> Self {
        let (private, unread) = match crate::sandbox::kept_from_reading(base, machine) {
            Ok(private) => (private, None),
            Err(why) => (Vec::new(), Some(why)),
        };
        let mut ids = Vec::new();
        for path in &private {
            // The entry itself, and what it leads to where it is a link.
            for meta in [std::fs::symlink_metadata(path), std::fs::metadata(path)] {
                if let Some(id) = meta.ok().as_ref().and_then(id_of)
                    && !ids.contains(&id)
                {
                    ids.push(id);
                }
            }
        }
        let (workspaces, unlisted) = match crate::workspaces::Plane::open(base).workspaces() {
            Ok(workspaces) => (workspaces, None),
            Err(why) => (Vec::new(), Some(why.to_string())),
        };
        Self {
            on: true,
            base: base.to_path_buf(),
            private,
            ids,
            workspaces,
            unread,
            unlisted,
        }
    }

    /// Whether this guard keeps anything at all.
    pub(super) fn on(&self) -> bool {
        self.on
    }

    /// Whether `absolute` is, or is under, something purlis keeps to itself, by its spelling.
    fn spelled_private(&self, absolute: &Path) -> bool {
        self.private.iter().any(|kept| under(absolute, kept))
    }

    /// Whether what is at `absolute` now IS one of the kept folders or files, or a link that
    /// leads to one.
    fn is_private(&self, absolute: &Path) -> bool {
        !self.ids.is_empty()
            && [
                std::fs::symlink_metadata(absolute),
                std::fs::metadata(absolute),
            ]
            .iter()
            .filter_map(|meta| meta.as_ref().ok().and_then(id_of))
            .any(|id| self.ids.contains(&id))
    }

    /// Whether `relative`, a path inside the place, is or is under something purlis keeps to
    /// itself: by its spelling, and by what each folder on the way to it is.
    pub(super) fn private(&self, relative: &Path) -> bool {
        if !self.on {
            return false;
        }
        let absolute = self.base.join(relative);
        if self.spelled_private(&absolute) {
            return true;
        }
        let mut at = self.base.clone();
        relative.components().any(|step| {
            at.push(step);
            self.is_private(&at)
        })
    }

    /// [`Self::private`] for an entry `name` of the folder `relative`, which was already
    /// judged: only the entry itself is looked at.
    pub(super) fn private_entry(&self, relative: &Path, name: &std::ffi::OsStr) -> bool {
        if !self.on {
            return false;
        }
        let absolute = self.base.join(relative).join(name);
        self.spelled_private(&absolute) || self.is_private(&absolute)
    }

    /// The workspace whose clone `relative` is, or is inside: `workspaces/<ws>/<repo>` and
    /// everything below it, where `<ws>` is one of the project's workspaces and `<repo>` holds
    /// a `.git` of its own. purlis's own folders under a workspace start with a dot, and none
    /// is a repo.
    pub(super) fn clone_of(&self, relative: &Path) -> Option<&str> {
        if !self.on {
            return None;
        }
        let mut steps = relative.components();
        let (Some(first), Some(ws), Some(repo)) = (steps.next(), steps.next(), steps.next()) else {
            return None;
        };
        if folded(first) != "workspaces" {
            return None;
        }
        let ws = ws.as_os_str().to_str()?;
        let repo = repo.as_os_str().to_str()?;
        if repo.starts_with('.') {
            return None;
        }
        let named = self.workspaces.iter().find(|one| {
            folded(Component::Normal(one.as_ref())) == folded(Component::Normal(ws.as_ref()))
        })?;
        let clone = self.base.join("workspaces").join(ws).join(repo);
        std::fs::symlink_metadata(clone.join(".git"))
            .is_ok()
            .then_some(named.as_str())
    }

    /// `relative`, said as `said`, refused when the plane root does not read it: something
    /// purlis keeps to itself, or anything of a workspace's clone.
    pub(super) fn refuse(&self, relative: &Path, said: &str) -> Result<(), Refused> {
        // The list that says what to leave out could not be told whole: nothing is read by a
        // list that may be short, the place's own folder included.
        if let Some(why) = &self.unread {
            return Err(Refused::Unreadable {
                what: "the project's own files".to_string(),
                why: format!(
                    "{why}, so purlis cannot tell which files are a vault's. Doctor says \
                     what is wrong with it"
                ),
            });
        }
        // Nor is anything under `workspaces/` read while the workspaces cannot be listed: a
        // workspace's clone could not be told from a folder of the project's own.
        if let Some(why) = &self.unlisted
            && relative
                .components()
                .next()
                .is_some_and(|first| folded(first) == "workspaces")
        {
            return Err(Refused::Unreadable {
                what: said.to_string(),
                why: format!("purlis could not list the project's workspaces ({why})"),
            });
        }
        if self.private(relative) {
            return Err(Refused::KeptPrivate(said.to_string()));
        }
        if let Some(ws) = self.clone_of(relative) {
            return Err(Refused::OfAWorkspace {
                path: said.to_string(),
                ws: ws.to_string(),
            });
        }
        Ok(())
    }

    /// [`Self::refuse`] for a path the disk resolved, as a link's target is: outside the
    /// place it is not this guard's to judge, and the caller refuses it.
    pub(super) fn refuse_resolved(&self, resolved: &Path, said: &str) -> Result<(), Refused> {
        if !self.on {
            return Ok(());
        }
        if self.spelled_private(resolved) {
            return Err(Refused::KeptPrivate(said.to_string()));
        }
        match resolved.strip_prefix(&self.base) {
            Ok(relative) => self.refuse(relative, said),
            Err(_) => Ok(()),
        }
    }

    /// Whether the open `file` is one of the kept files under another name: it IS a kept
    /// file, or it has several names and one of them is in a kept folder. A file with several
    /// names that purlis could not look for, or that the kept folders hold too many entries to
    /// look through, is counted as one: it does not open.
    pub(super) fn holds_a_name_of(&self, file: &std::fs::File) -> bool {
        if !self.on {
            return false;
        }
        let Ok(meta) = file.metadata() else {
            return true;
        };
        let Some(id) = id_of(&meta) else {
            return false;
        };
        if self.ids.contains(&id) {
            return true;
        }
        if names_of(&meta) < 2 {
            return false;
        }
        let mut left = WALKED;
        let mut folders: Vec<PathBuf> = self.private.clone();
        while let Some(folder) = folders.pop() {
            // Never through a link: what a kept folder links to is not what it holds.
            let Ok(listed) = std::fs::read_dir(&folder) else {
                continue;
            };
            for entry in listed.flatten() {
                if left == 0 {
                    return true;
                }
                left -= 1;
                let Ok(meta) = std::fs::symlink_metadata(entry.path()) else {
                    continue;
                };
                if meta.is_dir() {
                    folders.push(entry.path());
                } else if id_of(&meta) == Some(id) {
                    return true;
                }
            }
        }
        false
    }

    /// `offered`, a place's offered list, without what the plane root does not read: nothing
    /// purlis keeps to itself and nothing of a workspace's clone is offered, so neither opens
    /// and neither is found.
    pub(super) fn offered(&self, offered: &mut Vec<String>) {
        if !self.on {
            return;
        }
        offered.retain(|path| {
            let relative = Path::new(path);
            !self.private(relative) && self.clone_of(relative).is_none()
        });
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::sandbox::{Machine, Os};

    fn machine(home: &Path) -> Machine {
        Machine {
            env: crate::secrets::Env::of(&[]),
            home: Some(home.to_path_buf()),
            os: Os::this(),
        }
    }

    /// A project with a state folder holding a vault, a key, a spool, a dispatch record and
    /// a waiting report, a workspace `alpha` with a clone `svc`, and a plain file.
    fn project() -> (tempfile::TempDir, PathBuf, Kept) {
        let dir = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(dir.path()).unwrap();
        let state = base.join(crate::names::STATE_DIR.spellings().next().unwrap());
        for folder in [
            "vaults",
            "app/spool",
            "app/dispatches",
            "handbacks/chat-1",
            "sessions",
        ] {
            std::fs::create_dir_all(state.join(folder)).unwrap();
        }
        std::fs::write(state.join("vaults/forge.json"), "{}").unwrap();
        std::fs::write(state.join("fingerprint.key"), "k").unwrap();
        std::fs::write(state.join("sessions/one.md"), "a record").unwrap();
        std::fs::create_dir_all(base.join("workspaces/alpha/svc/.git")).unwrap();
        std::fs::create_dir_all(base.join("workspaces/alpha/.worktrees")).unwrap();
        std::fs::create_dir_all(base.join("workspaces/alpha/memory")).unwrap();
        std::fs::write(base.join("workspaces/alpha/svc/main.rs"), "").unwrap();
        std::fs::write(base.join("README.md"), "read me").unwrap();
        let home = base.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let kept = Kept::at_the_root(&base, &machine(&home));
        (dir, base, kept)
    }

    fn state() -> &'static str {
        crate::names::STATE_DIR.spellings().next().unwrap()
    }

    #[test]
    fn everything_a_chat_is_kept_from_reading_in_the_project_is_private_here() {
        let (_dir, _base, kept) = project();
        for path in [
            "vaults",
            "vaults/forge.json",
            "vaults/not-there.json",
            "fingerprint.key",
            "app/spool",
            "app/spool/1/turn",
            "app/dispatches/7.json",
            "handbacks",
            "handbacks/chat-1/report.md",
        ] {
            let relative = Path::new(state()).join(path);
            assert!(kept.private(&relative), "{relative:?} is listed");
        }
        // What a chat may read, the window may: the state folder itself and its records.
        for path in ["", "sessions", "sessions/one.md", "app"] {
            let relative = Path::new(state()).join(path);
            assert!(!kept.private(&relative), "{relative:?} is kept");
        }
        assert!(!kept.private(Path::new("README.md")));
    }

    #[test]
    fn the_vaults_and_the_key_are_private_under_every_spelling_of_the_state_folder() {
        let (_dir, _base, kept) = project();
        for state in crate::names::STATE_DIR.spellings() {
            for path in ["vaults/any.json", "fingerprint.key", "handbacks/x"] {
                let relative = Path::new(state).join(path);
                assert!(kept.private(&relative), "{relative:?} is listed");
            }
        }
    }

    #[test]
    fn a_kept_path_is_private_however_its_letters_are_cased_or_composed() {
        let (_dir, _base, kept) = project();
        let shouted = Path::new(&state().to_uppercase()).join("VAULTS/Forge.JSON");
        assert!(kept.private(&shouted), "{shouted:?}");
        let key = Path::new(state()).join("Fingerprint.Key");
        assert!(kept.private(&key));
    }

    #[test]
    fn a_folder_that_is_a_kept_one_is_private_under_any_name() {
        let (_dir, base, kept) = project();
        // What a case-folding file system does for `.PURLIS/VAULTS`, done by a rename the
        // spelling cannot know: the folder is the vaults', wherever it is found.
        let vaults = base.join(state()).join("vaults");
        let moved = base.join("notes");
        std::fs::rename(&vaults, &moved).unwrap();
        assert!(kept.private(Path::new("notes")));
        assert!(kept.private(Path::new("notes/forge.json")));
        assert!(kept.private_entry(Path::new(""), "notes".as_ref()));
    }

    #[test]
    fn a_file_with_a_second_name_in_a_kept_folder_holds_a_name_of_it() {
        let (_dir, base, kept) = project();
        let secret = base.join(state()).join("vaults/forge.json");
        let beside = base.join("harmless.json");
        std::fs::hard_link(&secret, &beside).unwrap();
        let key = base.join("key-copy");
        std::fs::hard_link(base.join(state()).join("fingerprint.key"), &key).unwrap();
        // Two names, neither kept.
        std::fs::hard_link(base.join("README.md"), base.join("README.again")).unwrap();

        let opened = |path: &Path| std::fs::File::open(path).unwrap();
        assert!(kept.holds_a_name_of(&opened(&beside)));
        assert!(kept.holds_a_name_of(&opened(&key)));
        assert!(!kept.holds_a_name_of(&opened(&base.join("README.again"))));
        assert!(!kept.holds_a_name_of(&opened(&base.join(state()).join("sessions/one.md"))));
    }

    #[test]
    fn a_workspaces_clone_and_everything_in_it_is_that_workspaces() {
        let (_dir, _base, kept) = project();
        for path in [
            "workspaces/alpha/svc",
            "workspaces/alpha/svc/main.rs",
            "workspaces/alpha/svc/.git/config",
        ] {
            assert_eq!(kept.clone_of(Path::new(path)), Some("alpha"), "{path}");
        }
        // The workspace's own folder and purlis's folders in it are the plane's.
        for path in [
            "workspaces",
            "workspaces/alpha",
            "workspaces/alpha/memory",
            "workspaces/alpha/.worktrees",
            "workspaces/beta/svc",
            "README.md",
        ] {
            assert_eq!(kept.clone_of(Path::new(path)), None, "{path}");
        }
    }

    #[test]
    fn each_refusal_is_said_in_a_sentence_of_purlis() {
        let (_dir, _base, kept) = project();
        let vault = Path::new(state()).join("vaults/forge.json");
        assert_eq!(
            kept.refuse(&vault, "the-path").unwrap_err().to_string(),
            "'the-path' is one of the files purlis keeps to itself (vaults, keys and its own \
             records), so it is not listed or opened here"
        );
        assert_eq!(
            kept.refuse(Path::new("workspaces/alpha/svc/main.rs"), "the-path")
                .unwrap_err()
                .to_string(),
            "'the-path' is in a repo of workspace 'alpha', whose files are on that workspace's tab"
        );
        assert!(kept.refuse(Path::new("README.md"), "README.md").is_ok());
    }

    #[test]
    fn nothing_kept_or_of_a_clone_stays_on_an_offered_list() {
        let (_dir, _base, kept) = project();
        let mut offered = vec![
            "README.md".to_string(),
            format!("{}/fingerprint.key", state()),
            format!("{}/sessions/one.md", state()),
            format!("{}/vaults/forge.json", state()),
            "workspaces/alpha/svc/".to_string(),
            "workspaces/alpha/svc/main.rs".to_string(),
            "workspaces/alpha/workspace.md".to_string(),
        ];
        kept.offered(&mut offered);
        assert_eq!(
            offered,
            [
                "README.md".to_string(),
                format!("{}/sessions/one.md", state()),
                "workspaces/alpha/workspace.md".to_string(),
            ]
        );
    }

    #[test]
    fn nothing_is_read_where_the_vault_registry_does_not_read() {
        let (_dir, base, _) = project();
        // A vault whose file is among the project's own, and a registry that stops reading.
        std::fs::create_dir_all(base.join("secrets")).unwrap();
        std::fs::write(base.join("secrets/dev.json"), "{}").unwrap();
        std::fs::write(base.join("vaults.json"), "{ not json").unwrap();
        let kept = Kept::at_the_root(&base, &machine(&base.join("home")));

        let said = "purlis could not read 'the project's own files': the vault registry could \
                    not be read, so purlis cannot tell which files are a vault's. Doctor says \
                    what is wrong with it";
        // The place's own folder, a file of its own, and the vault's file: none is read.
        for path in ["", "README.md", "secrets/dev.json", "docs"] {
            assert_eq!(
                kept.refuse(Path::new(path), path).unwrap_err().to_string(),
                said,
                "{path}"
            );
        }
    }

    #[test]
    fn nothing_under_workspaces_is_read_where_the_workspaces_cannot_be_listed() {
        let (_dir, base, _) = project();
        // `workspaces` as a file: it cannot be listed, so no clone can be told.
        std::fs::remove_dir_all(base.join("workspaces")).unwrap();
        std::fs::write(base.join("workspaces"), "").unwrap();
        let kept = Kept::at_the_root(&base, &machine(&base.join("home")));

        let refused = kept
            .refuse(Path::new("workspaces/alpha/svc"), "workspaces/alpha/svc")
            .unwrap_err()
            .to_string();
        assert!(
            refused.starts_with(
                "purlis could not read 'workspaces/alpha/svc': purlis could not list the \
                 project's workspaces ("
            ),
            "{refused}"
        );
        assert!(kept.refuse(Path::new("workspaces"), "workspaces").is_err());
        // The rest of the project is read as ever.
        assert!(kept.refuse(Path::new("README.md"), "README.md").is_ok());
    }

    #[test]
    fn a_guard_of_any_other_branch_keeps_nothing() {
        let (_dir, base, _) = project();
        let kept = Kept::of(&base, Branch::repo("alpha", "svc"));
        let vault = Path::new(state()).join("vaults/forge.json");
        assert!(!kept.private(&vault));
        assert_eq!(
            kept.clone_of(Path::new("workspaces/alpha/svc/main.rs")),
            None
        );
        let mut offered = vec![format!("{}/vaults/forge.json", state())];
        kept.offered(&mut offered);
        assert_eq!(offered.len(), 1);
    }
}
