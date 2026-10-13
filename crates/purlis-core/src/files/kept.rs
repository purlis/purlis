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
//! **A branch's folder leaves out the same files** (ADR 0038 as amended): a workspace's clone
//! is inside the project, so a vault's file may be registered in one and a kept file may have a
//! name in one. [`Kept::of`] answers every place with the one list, and only the plane root
//! with the rule for clones. Where the list cannot be told whole, a branch's files are not
//! read either, in a project that runs its chats sandboxed; where it runs none sandboxed there
//! is no such list in force, and a branch reads as it always did. At the plane root the one
//! file that still opens is the half of the vault registry to put right.

use std::path::{Component, Path, PathBuf};

use super::{Branch, Refused};

/// The most entries of the kept folders [`Kept::holds_a_name_of`] looks at for a file with
/// several names. Past it the file is refused: purlis could not tell, so it does not open.
const WALKED: usize = 20_000;

/// What a read of one place leaves out: what purlis keeps to itself, in every place, and at
/// the plane root everything of a workspace's clone too.
#[derive(Debug)]
pub(super) struct Kept {
    /// Whether this guard keeps anything: every place of a project's.
    on: bool,
    /// Whether this is the plane root's guard, which alone holds the rule for clones.
    root: bool,
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
    unread: Option<crate::sandbox::Untold>,
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
    /// The guard of `branch` in the project at `plane`, its folder resolved to `base`, on
    /// this machine.
    pub(super) fn of(plane: &Path, base: &Path, branch: Branch<'_>) -> Self {
        let machine = crate::sandbox::Machine::this();
        if branch.is_plane_root() {
            Self::at_the_root(base, &machine)
        } else {
            Self::in_a_branch(plane, base, &machine)
        }
    }

    /// The guard of a branch's folder, resolved to `base`, in the project at `plane` on
    /// `machine`: what the project keeps to itself, wherever in the folder it is.
    pub(super) fn in_a_branch(
        plane: &Path,
        base: &Path,
        machine: &crate::sandbox::Machine,
    ) -> Self {
        let plane = std::fs::canonicalize(plane).unwrap_or_else(|_| plane.to_path_buf());
        let (private, ids, mut unread) = Self::private_in(&plane, machine);
        // The list is what a chat is kept from. Where the project runs no chat sandboxed
        // there is no such list in force, so a branch whose list cannot be told is read as a
        // branch always was; the plane root keeps its own rule.
        if unread.is_some() && !crate::sandbox::in_force_at(&plane) {
            unread = None;
        }
        Self {
            on: true,
            root: false,
            base: base.to_path_buf(),
            private,
            ids,
            workspaces: Vec::new(),
            unread,
            unlisted: None,
        }
    }

    /// What purlis keeps to itself in the project at `plane` on `machine`, with the identity
    /// of each that is there — or why that could not be told whole.
    fn private_in(
        plane: &Path,
        machine: &crate::sandbox::Machine,
    ) -> (Vec<PathBuf>, Vec<Id>, Option<crate::sandbox::Untold>) {
        let (private, unread) = match crate::sandbox::kept_from_reading(plane, machine) {
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
        (private, ids, unread)
    }

    /// The plane root's guard, the project's folder resolved to `base`, on `machine`.
    pub(super) fn at_the_root(base: &Path, machine: &crate::sandbox::Machine) -> Self {
        let (private, ids, unread) = Self::private_in(base, machine);
        let (workspaces, unlisted) = match crate::workspaces::Plane::open(base).workspaces() {
            Ok(workspaces) => (workspaces, None),
            Err(why) => (Vec::new(), Some(why.to_string())),
        };
        Self {
            on: true,
            root: true,
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
        if !self.root {
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
            // But the one file to put right, so the person can read what is wrong with it.
            if self.is_the_registry_to_put_right(relative) {
                return Ok(());
            }
            return Err(Refused::Unreadable {
                what: if self.root {
                    "the project's own files"
                } else {
                    "this folder's files"
                }
                .to_string(),
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

    /// Whether `relative` is the half of the vault registry that keeps the list from being
    /// told whole, at the plane root: the one file that still opens there, so what is wrong
    /// with it can be read in the window.
    fn is_the_registry_to_put_right(&self, relative: &Path) -> bool {
        self.root
            && self
                .unread
                .as_ref()
                .and_then(|untold| untold.registry.as_ref())
                .is_some_and(|registry| *registry == self.base.join(relative))
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
        // The registry to put right opens as the one file it is: under no other name of it.
        if self.unread.is_some() && names_of(&meta) > 1 {
            return true;
        }
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

    /// `offered`, a place's offered list, without what the place does not read: nothing
    /// purlis keeps to itself and, at the plane root, nothing of a workspace's clone is
    /// offered, so neither opens and neither is found.
    ///
    /// **Nothing is offered by a list that may be short**: where what purlis keeps could not be
    /// told whole the list holds at most the registry to put right, and where the workspaces could not be listed
    /// nothing under `workspaces/` stays on it, whoever asks and whatever was refused before.
    pub(super) fn offered(&self, offered: &mut Vec<String>) {
        if !self.on {
            return;
        }
        if self.unread.is_some() {
            offered.retain(|path| self.is_the_registry_to_put_right(Path::new(path)));
            return;
        }
        offered.retain(|path| {
            let relative = Path::new(path);
            let unlisted = self.unlisted.is_some()
                && relative
                    .components()
                    .next()
                    .is_some_and(|first| folded(first) == "workspaces");
            // A branch's list is judged by spelling, as [`Self::listed_private`] says why.
            let private = if self.root {
                self.private(relative)
            } else {
                self.listed_private(relative)
            };
            !unlisted && !private && self.clone_of(relative).is_none()
        });
    }

    /// Whether `relative`, one path of a list of the place's files, is or is under something
    /// purlis keeps to itself, **by its spelling alone**: a list may hold every file of a
    /// branch, and the disk is not asked about each. What a path IS is asked where a file is
    /// opened ([`Self::refuse`], [`Self::holds_a_name_of`]).
    pub(super) fn listed_private(&self, relative: &Path) -> bool {
        self.on && self.spelled_private(&self.base.join(relative))
    }

    /// Refused where nothing of the place is read: the list of what purlis keeps could not be
    /// told whole.
    pub(super) fn whole(&self) -> Result<(), Refused> {
        self.refuse(Path::new(""), "")
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

        let said = format!(
            "purlis could not read 'the project's own files': the vault registry {} could not \
             be read, so purlis cannot tell which files are a vault's. Doctor says what is \
             wrong with it",
            base.join("vaults.json").display()
        );
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
    fn the_registry_to_put_right_is_the_one_file_that_still_opens_at_the_plane_root() {
        let (_dir, base, _) = project();
        std::fs::write(base.join("vaults.json"), "{ not json").unwrap();
        let home = base.join("home");
        let kept = Kept::at_the_root(&base, &machine(&home));

        assert!(kept.refuse(Path::new("vaults.json"), "vaults.json").is_ok());
        // Under no other spelling, and nothing beside it.
        for path in ["Vaults.json", "docs/vaults.json", "vaults.json/x", ""] {
            assert!(kept.refuse(Path::new(path), path).is_err(), "{path}");
        }
        let mut offered = vec![
            "README.md".to_string(),
            "vaults.json".to_string(),
            "docs/vaults.json".to_string(),
        ];
        kept.offered(&mut offered);
        assert_eq!(offered, ["vaults.json".to_string()]);

        // A branch's folder has no such file: nothing of it opens.
        let clone = base.join("workspaces/alpha/svc");
        let kept = Kept::in_a_branch(&base, &clone, &machine(&home));
        assert!(
            kept.refuse(Path::new("vaults.json"), "vaults.json")
                .is_err()
        );
    }

    #[test]
    fn the_registry_to_put_right_does_not_open_where_it_is_another_name_of_a_file() {
        let (_dir, base, _) = project();
        // The key, under the registry's name: it does not read as a registry.
        std::fs::hard_link(
            base.join(state()).join("fingerprint.key"),
            base.join("vaults.json"),
        )
        .unwrap();
        let kept = Kept::at_the_root(&base, &machine(&base.join("home")));
        assert!(kept.refuse(Path::new("README.md"), "README.md").is_err());
        let opened = std::fs::File::open(base.join("vaults.json")).unwrap();
        assert!(kept.holds_a_name_of(&opened));

        // A registry of one name that does not read still opens.
        std::fs::remove_file(base.join("vaults.json")).unwrap();
        std::fs::write(base.join("vaults.json"), "{ not json").unwrap();
        let kept = Kept::at_the_root(&base, &machine(&base.join("home")));
        let opened = std::fs::File::open(base.join("vaults.json")).unwrap();
        assert!(!kept.holds_a_name_of(&opened));
    }

    #[test]
    fn a_branch_is_read_as_ever_where_the_project_runs_no_chat_sandboxed() {
        let (_dir, base, _) = project();
        std::fs::write(base.join("vaults.json"), "{ not json").unwrap();
        let clone = base.join("workspaces/alpha/svc");
        let home = base.join("home");
        let read = |manifest: Option<&str>| {
            match manifest {
                Some(text) => std::fs::write(base.join("charter.toml"), text).unwrap(),
                None => {
                    let _ = std::fs::remove_file(base.join("charter.toml"));
                }
            }
            let kept = Kept::in_a_branch(&base, &clone, &machine(&home));
            (
                kept.refuse(Path::new("main.rs"), "main.rs").is_ok(),
                kept.whole().is_ok(),
            )
        };
        // The sandbox is not on: there is no list in force, and the branch reads.
        assert_eq!(read(Some("schema = 1\n")), (true, true));
        // On, or not known to be off: the branch is not read by a list that is not whole.
        assert_eq!(read(Some("[sandbox]\nmode = \"on\"\n")), (false, false));
        assert_eq!(read(Some("not toml [")), (false, false));
        assert_eq!(read(None), (false, false));
        // The plane root keeps its own rule, whatever the sandbox is.
        std::fs::write(base.join("charter.toml"), "schema = 1\n").unwrap();
        let root = Kept::at_the_root(&base, &machine(&home));
        assert!(root.refuse(Path::new("README.md"), "README.md").is_err());
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
    fn nothing_is_offered_where_the_vault_registry_does_not_read() {
        let (_dir, base, _) = project();
        std::fs::write(base.join("vaults.json"), "{ not json").unwrap();
        let home = base.join("home");
        for kept in [
            Kept::at_the_root(&base, &machine(&home)),
            Kept::in_a_branch(&base, &base.join("workspaces/alpha/svc"), &machine(&home)),
        ] {
            let mut offered = vec![
                "README.md".to_string(),
                "secrets/dev.json".to_string(),
                "workspaces/alpha/workspace.md".to_string(),
            ];
            kept.offered(&mut offered);
            assert_eq!(offered, Vec::<String>::new());
        }
    }

    #[test]
    fn nothing_under_workspaces_is_offered_where_the_workspaces_cannot_be_listed() {
        let (_dir, base, _) = project();
        std::fs::remove_dir_all(base.join("workspaces")).unwrap();
        std::fs::write(base.join("workspaces"), "").unwrap();
        let kept = Kept::at_the_root(&base, &machine(&base.join("home")));

        let mut offered = vec![
            "README.md".to_string(),
            "workspaces/alpha/svc/main.rs".to_string(),
            "Workspaces/alpha/workspace.md".to_string(),
            format!("{}/sessions/one.md", state()),
        ];
        kept.offered(&mut offered);
        assert_eq!(
            offered,
            [
                "README.md".to_string(),
                format!("{}/sessions/one.md", state())
            ]
        );
    }

    /// A project whose `plain-file` vault keeps its file in the clone `workspaces/alpha/svc`,
    /// and the guard of that clone's folder.
    fn clone_with_a_vault_file() -> (tempfile::TempDir, PathBuf, PathBuf, Kept) {
        let (dir, base, _) = project();
        let clone = base.join("workspaces/alpha/svc");
        std::fs::create_dir_all(clone.join("secrets")).unwrap();
        std::fs::write(clone.join("secrets/dev.json"), "{}").unwrap();
        std::fs::write(
            base.join("vaults.json"),
            r#"{"vaults": {"dev": {"provider": "plain-file",
                "config": {"file": "workspaces/alpha/svc/secrets/dev.json"}}}}"#,
        )
        .unwrap();
        let kept = Kept::in_a_branch(&base, &clone, &machine(&base.join("home")));
        (dir, base, clone, kept)
    }

    #[test]
    fn a_branchs_folder_leaves_out_what_the_project_keeps_and_reads_the_rest() {
        let (_dir, _base, _clone, kept) = clone_with_a_vault_file();
        assert!(kept.on());
        assert!(kept.private(Path::new("secrets/dev.json")));
        assert!(kept.private(Path::new("Secrets/DEV.json")));
        assert_eq!(
            kept.refuse(Path::new("secrets/dev.json"), "secrets/dev.json")
                .unwrap_err()
                .to_string(),
            "'secrets/dev.json' is one of the files purlis keeps to itself (vaults, keys and \
             its own records), so it is not listed or opened here"
        );
        assert!(kept.private_entry(Path::new("secrets"), "dev.json".as_ref()));
        // The rest of the clone reads as ever, and the rule for clones is the plane root's.
        for path in ["", "main.rs", "secrets", "workspaces/alpha/svc/main.rs"] {
            assert!(kept.refuse(Path::new(path), path).is_ok(), "{path}");
        }
        assert_eq!(
            kept.clone_of(Path::new("workspaces/alpha/svc/main.rs")),
            None
        );
        let mut offered = vec!["main.rs".to_string(), "secrets/dev.json".to_string()];
        kept.offered(&mut offered);
        assert_eq!(offered, ["main.rs".to_string()]);
    }

    #[test]
    fn a_list_of_a_branchs_changes_leaves_out_a_kept_file_at_either_end_of_a_move() {
        let (_dir, _base, _clone, kept) = clone_with_a_vault_file();
        let left_out = crate::files::compare::kept_from_a_list;
        assert!(left_out(&kept, "secrets/dev.json", None));
        assert!(left_out(&kept, "SECRETS/dev.json", None));
        assert!(left_out(
            &kept,
            "notes/moved.json",
            Some("secrets/dev.json")
        ));
        assert!(left_out(&kept, "secrets/dev.json", Some("notes/old.json")));
        assert!(!left_out(&kept, "main.rs", None));
        assert!(!left_out(&kept, "main.rs", Some("old.rs")));
        // A list is read where the kept list is whole, and not where it is not.
        assert!(kept.whole().is_ok());
    }

    #[test]
    fn no_list_of_a_branchs_changes_is_read_where_the_vault_registry_does_not_read() {
        let (_dir, base, _) = project();
        std::fs::write(base.join("vaults.json"), "{ not json").unwrap();
        let clone = base.join("workspaces/alpha/svc");
        let kept = Kept::in_a_branch(&base, &clone, &machine(&base.join("home")));
        assert!(kept.whole().is_err());
    }

    #[test]
    fn a_second_name_in_a_branchs_folder_of_a_kept_file_does_not_open_there() {
        let (_dir, base, clone, kept) = clone_with_a_vault_file();
        // Made after the guard: found by what the file is.
        let beside = clone.join("harmless.json");
        std::fs::hard_link(base.join(state()).join("vaults/forge.json"), &beside).unwrap();
        std::fs::hard_link(clone.join("main.rs"), clone.join("main.again")).unwrap();
        let opened = |path: &Path| std::fs::File::open(path).unwrap();
        assert!(kept.holds_a_name_of(&opened(&beside)));
        assert!(!kept.holds_a_name_of(&opened(&clone.join("main.again"))));

        // There when the guard is made: kept by its name too.
        let kept = Kept::in_a_branch(&base, &clone, &machine(&base.join("home")));
        assert!(kept.private(Path::new("harmless.json")));
    }

    #[test]
    fn a_branchs_files_are_not_read_where_the_vault_registry_does_not_read() {
        let (_dir, base, _) = project();
        std::fs::write(base.join("vaults.json"), "{ not json").unwrap();
        let clone = base.join("workspaces/alpha/svc");
        let kept = Kept::in_a_branch(&base, &clone, &machine(&base.join("home")));
        for path in ["", "main.rs"] {
            assert_eq!(
                kept.refuse(Path::new(path), path).unwrap_err().to_string(),
                format!(
                    "purlis could not read 'this folder's files': the vault registry {} could \
                     not be read, so purlis cannot tell which files are a vault's. Doctor says \
                     what is wrong with it",
                    base.join("vaults.json").display()
                ),
                "{path}"
            );
        }
    }
}
