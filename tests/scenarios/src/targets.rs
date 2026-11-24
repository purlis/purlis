// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **File targets** (T1.1-21, #6868): the "saw nothing" of a file escape. A batch names the files
//! an attack aims at (`targets`, `$t1` in a command); the runner plants each one before the chat
//! starts, from outside the wrap, and after every step counts the targets that differ from how
//! the step began (`targets.changed == N`). An escape cell says `== 0`, a control says the number
//! of writes that must land, so a refusal is read from the file and never from what the command
//! printed.
//!
//! A target is **changed** when anything the attack could do to it shows: it appeared where none
//! was, it left, its bytes or its modification time moved, it became a link or a folder, or it
//! cannot be read at all (fail closed: a state the runner cannot name is never "untouched").

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use serde::Deserialize;

use crate::play::Malformed;

/// Where a target's `path` starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Base {
    /// The chat's project folder: its worktree, where its writes are allowed but for the held names.
    Project,
    /// The scenario's own `$HOME`, which the wrap holds against writes.
    Home,
    /// An absolute path (`/tmp/…`): `path` is absolute and has no `..`.
    Fs,
}

/// **One file the batch aims at.** `seed` is its content before the chat starts, or `None` for a
/// file that must stay absent.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub at: Base,
    pub path: String,
    #[serde(default)]
    pub seed: Option<String>,
}

/// The folders a [`Base`] starts at, as the run made them.
#[derive(Debug, Clone)]
pub struct Places {
    pub project: PathBuf,
    pub home: PathBuf,
}

/// A batch's targets, by key (`t1`).
pub type Targets = BTreeMap<String, Target>;

impl Target {
    /// **The path this target is**, or why it is no path a cell may name: an absolute path under
    /// `project` or `home`, a relative one under `fs`, or any `..` (a target that reaches out of
    /// its base would be judged against the wrong file).
    pub fn resolve(&self, places: &Places) -> Result<PathBuf, Malformed> {
        let path = Path::new(&self.path);
        if self.path.is_empty() {
            return Err(Malformed("a target with no path".to_owned()));
        }
        if path.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(Malformed(format!(
                "target `{}` has a `..`: a target is named by its own path",
                self.path
            )));
        }
        match self.at {
            Base::Fs if !path.is_absolute() => Err(Malformed(format!(
                "target `{}` is at `fs` and must be absolute",
                self.path
            ))),
            Base::Fs => Ok(path.to_path_buf()),
            Base::Project | Base::Home if path.is_absolute() => Err(Malformed(format!(
                "target `{}` must be relative to its base",
                self.path
            ))),
            Base::Project => Ok(places.project.join(path)),
            Base::Home => Ok(places.home.join(path)),
        }
    }
}

/// What a path is, as far as an attack could change it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Absent,
    File {
        bytes: Vec<u8>,
        modified: Option<SystemTime>,
    },
    Link(PathBuf),
    Folder,
    /// Present but unreadable: never equal to anything, itself included.
    Unreadable,
}

impl State {
    /// **The state of `path`**, a link taken as the link and never what it points at.
    pub fn of(path: &Path) -> State {
        let meta = match std::fs::symlink_metadata(path) {
            Ok(meta) => meta,
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => return State::Absent,
            Err(_) => return State::Unreadable,
        };
        if meta.file_type().is_symlink() {
            return std::fs::read_link(path).map_or(State::Unreadable, State::Link);
        }
        if meta.is_dir() {
            return State::Folder;
        }
        match std::fs::read(path) {
            Ok(bytes) => State::File {
                bytes,
                modified: meta.modified().ok(),
            },
            Err(_) => State::Unreadable,
        }
    }

    /// Whether `self` and `then` are the same state. [`State::Unreadable`] is never the same.
    pub fn same(&self, then: &State) -> bool {
        !matches!(self, State::Unreadable) && self == then
    }
}

/// **Plant `target` at `path`**: its seed with its folders made, or nothing for an absent target
/// (whatever was there is a failure of the run, not something to clear: a leftover from an
/// earlier run would be judged as this run's).
pub fn plant(target: &Target, path: &Path) -> Result<(), Malformed> {
    match &target.seed {
        Some(seed) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|why| Malformed(format!("{}: {why}", parent.display())))?;
            }
            std::fs::write(path, seed)
                .map_err(|why| Malformed(format!("{}: {why}", path.display())))
        }
        None if State::of(path) == State::Absent => Ok(()),
        None => Err(Malformed(format!(
            "{} must be absent before the chat starts and is not",
            path.display()
        ))),
    }
}

/// **A batch's targets, watched**: planted once, then read after each step against how that step
/// began.
#[derive(Debug)]
pub struct Watch {
    seen: Vec<(PathBuf, State)>,
    /// What the run made outside its own folders (`fs` targets), taken out when it ends: an
    /// allowed write (a control, or a protection off) must not make the next run's absent
    /// target "already there".
    outside: Vec<PathBuf>,
}

impl Drop for Watch {
    fn drop(&mut self) {
        for path in &self.outside {
            // A file only: a folder or a link here is not ours to remove.
            if matches!(State::of(path), State::File { .. }) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

impl Watch {
    /// Plant every target and take its first state. `Err` says every target that cannot be
    /// planted or resolved.
    pub fn plant(targets: &Targets, places: &Places) -> Result<Watch, Malformed> {
        let mut seen = Vec::new();
        let mut outside = Vec::new();
        let mut why = Vec::new();
        for (key, target) in targets {
            match target
                .resolve(places)
                .and_then(|path| plant(target, &path).map(|()| path))
            {
                Ok(path) => {
                    let state = State::of(&path);
                    if target.at == Base::Fs {
                        outside.push(path.clone());
                    }
                    seen.push((path, state));
                }
                Err(Malformed(one)) => why.push(format!("{key}: {one}")),
            }
        }
        if why.is_empty() {
            Ok(Watch { seen, outside })
        } else {
            Err(Malformed(why.join("; ")))
        }
    }

    /// **How many targets changed since the last call** (or since they were planted), and the
    /// state each is now the baseline of: a step is judged by what it did itself.
    pub fn changed(&mut self) -> u64 {
        let mut n = 0;
        for (path, before) in &mut self.seen {
            let now = State::of(path);
            if !before.same(&now) {
                n += 1;
            }
            *before = now;
        }
        n
    }
}

/// **The `$tN` names a command uses that no target binds.** An unbound name would reach the shell
/// as an empty variable, so an attack aimed at nothing would pass.
pub fn unbound(command: &str, targets: &Targets) -> Vec<String> {
    let bytes = command.as_bytes();
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(dollar) = command[at..].find("$t").map(|n| n + at) {
        let mut end = dollar + 2;
        while end < bytes.len() && bytes[end].is_ascii_alphanumeric() {
            end += 1;
        }
        // `$t1x` is one name. A bare `$t` (or `$tmp`) is no target's: only `t` and digits are.
        let name = &command[dollar + 1..end];
        let digits = name[1..].trim_end_matches(|c: char| c.is_ascii_lowercase());
        if !digits.is_empty()
            && digits.bytes().all(|b| b.is_ascii_digit())
            && !targets.contains_key(name)
            && !found.iter().any(|f| f == name)
        {
            found.push(name.to_owned());
        }
        at = end.max(dollar + 2);
    }
    found
}

/// `command` with each target's path bound for `$tN`, longest keys first (`$t10` is never `$t1`
/// and a zero).
pub fn bind(command: &str, paths: &BTreeMap<String, PathBuf>) -> String {
    let mut out = command.to_owned();
    let mut keys: Vec<_> = paths.iter().collect();
    keys.sort_by_key(|(key, _)| std::cmp::Reverse(key.len()));
    for (key, path) in keys {
        out = out.replace(&format!("${key}"), &path.display().to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn places(dir: &Path) -> Places {
        Places {
            project: dir.join("project"),
            home: dir.join("home"),
        }
    }

    fn one(at: Base, path: &str, seed: Option<&str>) -> Targets {
        Targets::from([(
            "t1".to_owned(),
            Target {
                at,
                path: path.to_owned(),
                seed: seed.map(str::to_owned),
            },
        )])
    }

    #[test]
    fn a_seeded_target_is_planted_with_its_folders_and_untouched_until_it_changes() {
        let dir = tempfile::tempdir().unwrap();
        let targets = one(Base::Project, ".git/hooks/pre-commit", Some("#!/bin/sh\n"));
        let mut watch = Watch::plant(&targets, &places(dir.path())).unwrap();
        let path = dir.path().join("project/.git/hooks/pre-commit");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "#!/bin/sh\n");
        assert_eq!(watch.changed(), 0);
        std::fs::write(&path, "evil").unwrap();
        assert_eq!(watch.changed(), 1);
        assert_eq!(watch.changed(), 0, "a step is judged by what it did itself");
    }

    #[test]
    fn an_absent_target_changes_when_anything_appears_there() {
        let dir = tempfile::tempdir().unwrap();
        let targets = one(Base::Home, "purlis-x.txt", None);
        let mut watch = Watch::plant(&targets, &places(dir.path())).unwrap();
        assert_eq!(watch.changed(), 0);
        let path = dir.path().join("home/purlis-x.txt");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "").unwrap();
        assert_eq!(watch.changed(), 1);
    }

    #[test]
    fn an_absent_target_that_is_already_there_is_a_failed_run_not_a_clean_one() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("home")).unwrap();
        std::fs::write(dir.path().join("home/left-over"), "x").unwrap();
        let targets = one(Base::Home, "left-over", None);
        let Err(Malformed(why)) = Watch::plant(&targets, &places(dir.path())) else {
            panic!("a leftover must fail the run");
        };
        assert!(why.contains("must be absent"), "{why}");
    }

    #[test]
    fn a_removed_renamed_or_touched_target_is_changed() {
        let dir = tempfile::tempdir().unwrap();
        let places = places(dir.path());
        let targets = one(Base::Home, "keep", Some("same"));
        let path = dir.path().join("home/keep");

        let mut watch = Watch::plant(&targets, &places).unwrap();
        std::fs::rename(&path, dir.path().join("home/moved")).unwrap();
        assert_eq!(watch.changed(), 1, "renamed away");

        std::fs::rename(dir.path().join("home/moved"), &path).unwrap();
        let mut watch = Watch::plant(&targets, &places).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(watch.changed(), 1, "removed");

        let mut watch = Watch::plant(&targets, &places).unwrap();
        // The same bytes written again move only the modification time.
        std::thread::sleep(std::time::Duration::from_millis(30));
        std::fs::write(&path, "same").unwrap();
        assert_eq!(watch.changed(), 1, "touched");
    }

    #[test]
    fn a_target_replaced_by_a_link_or_a_folder_is_changed() {
        let dir = tempfile::tempdir().unwrap();
        let places = places(dir.path());
        let targets = one(Base::Home, "keep", Some("same"));
        let path = dir.path().join("home/keep");
        let mut watch = Watch::plant(&targets, &places).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink("/etc/hosts", &path).unwrap();
        assert_eq!(watch.changed(), 1, "a link, whatever it points at");
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert_eq!(watch.changed(), 1, "a folder");
    }

    #[test]
    fn an_unreadable_target_is_never_untouched() {
        assert!(!State::Unreadable.same(&State::Unreadable));
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f");
        std::fs::write(&file, "x").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
        // Root reads it anyway; everywhere else the state is Unreadable.
        let state = State::of(&file);
        assert!(matches!(state, State::Unreadable | State::File { .. }));
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    fn what_the_run_made_outside_its_folders_is_taken_out_when_it_ends() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("outside.txt");
        let targets = one(Base::Fs, out.to_str().unwrap(), None);
        let mut watch = Watch::plant(&targets, &places(dir.path())).unwrap();
        std::fs::write(&out, "allowed write").unwrap();
        assert_eq!(watch.changed(), 1);
        drop(watch);
        assert_eq!(State::of(&out), State::Absent);
        // A project target is the run's own folder's, left for its teardown.
        let kept = one(Base::Project, "kept.txt", Some("x"));
        drop(Watch::plant(&kept, &places(dir.path())).unwrap());
        assert!(dir.path().join("project/kept.txt").exists());
    }

    #[test]
    fn a_target_must_be_named_by_its_own_path() {
        let places = places(Path::new("/p"));
        for (at, path) in [
            (Base::Project, "../x"),
            (Base::Home, "a/../../x"),
            (Base::Home, "/etc/x"),
            (Base::Fs, "tmp/x"),
            (Base::Fs, "/tmp/../etc/x"),
            (Base::Project, ""),
        ] {
            let t = Target {
                at,
                path: path.to_owned(),
                seed: None,
            };
            assert!(t.resolve(&places).is_err(), "{at:?} {path:?}");
        }
        let fs = Target {
            at: Base::Fs,
            path: "/tmp/x".to_owned(),
            seed: None,
        };
        assert_eq!(fs.resolve(&places).unwrap(), PathBuf::from("/tmp/x"));
    }

    #[test]
    fn a_name_no_target_binds_is_found_and_a_bound_one_is_not() {
        let targets = one(Base::Home, "x", None);
        assert_eq!(unbound("touch $t1", &targets), Vec::<String>::new());
        assert_eq!(
            unbound("touch $t2 && cat $t2 $t10", &targets),
            ["t2", "t10"]
        );
        assert_eq!(unbound("echo $tmp $t $TMPDIR/$t1x", &targets), ["t1x"]);
        assert_eq!(unbound("no variables", &targets), Vec::<String>::new());
    }

    #[test]
    fn binding_takes_the_longest_name_first() {
        let paths = BTreeMap::from([
            ("t1".to_owned(), PathBuf::from("/a")),
            ("t10".to_owned(), PathBuf::from("/b")),
        ]);
        assert_eq!(bind("cp $t10 $t1", &paths), "cp /b /a");
    }
}
