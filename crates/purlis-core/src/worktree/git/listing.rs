// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The config listing, kept per repository until a file it was read from changes** (train 80).
//!
//! [`super::programs_held`] lists a repository's configuration (`git config --list`) before
//! every call that can reach a program or cross a network, which is one more git per call. The
//! listing is kept here, for the folder and the exact way it was asked, and handed back only
//! while nothing it was read from has changed. A kept listing that outlived a change would let a
//! program the configuration now names run unheld, so "nothing changed" is checked on every use,
//! never assumed:
//!
//! - **What is checked**: each file the listing says it read (`--show-origin`); each file git
//!   would have read had it been there (the repository's `config` and `config.worktree`, your
//!   global files and the ones the environment names); and each folder's `.git` from the call's
//!   folder up to the repository's, which is what decides which repository git finds.
//! - **How**: by device, inode, size, modification time and change time. A writer can set a
//!   file's modification time back; nobody can set its change time.
//! - **Never kept**: a listing with an include or a conditional include in it (what it reads can
//!   change with no file changing, or come from a file that does not exist yet), one read from
//!   anything but a file or the command line, one asked on a thread an isolation pins, one whose
//!   files changed while it was read, and anything off unix.
//!
//! Whatever cannot be checked is listed again. The race the listing already has, a configuration
//! rewritten between the listing and the call, is the same with the kept one and no wider.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

/// The most listings kept. Past it, every one is dropped and the next calls list again.
const MOST_KEPT: usize = 256;

/// How long before the listing started a file must have last changed for the listing to be
/// kept: a filesystem whose times are coarse can give a change during the listing a time just
/// before it began.
const SETTLED: std::time::Duration = std::time::Duration::from_secs(2);

/// Everything the listing depends on besides the files it reads: the folder, the options before
/// the verb, the `-c` settings, the variables passed through, the checked link and `HOME`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(super) struct Asked {
    dir: PathBuf,
    before_verb: Vec<String>,
    config: Vec<String>,
    pass: Vec<(String, OsString)>,
    link: Option<(PathBuf, PathBuf)>,
    home: Option<OsString>,
}

impl Asked {
    /// What a listing asked this way depends on, or none where it is never kept: options before
    /// the verb other than `-c <setting>` pairs (one may name another folder or repository).
    pub(super) fn new(
        dir: &Path,
        before_verb: &[&str],
        config: &[String],
        pass: &[(String, OsString)],
        link: Option<&(PathBuf, PathBuf)>,
    ) -> Option<Asked> {
        let only_settings = before_verb.len().is_multiple_of(2)
            && before_verb.chunks(2).all(|pair| pair[0] == "-c");
        if !only_settings || !dir.is_absolute() {
            return None;
        }
        Some(Asked {
            dir: dir.to_path_buf(),
            before_verb: before_verb.iter().map(|word| (*word).to_owned()).collect(),
            config: config.to_vec(),
            pass: pass.to_vec(),
            link: link.cloned(),
            home: std::env::var_os("HOME"),
        })
    }
}

/// What a file was when the listing was kept.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Stamp {
    Absent,
    /// The `.git` folder git finds the repository by: which folder it is, and not what is in
    /// it, which every write of the index changes. Its configuration is stamped on its own.
    Folder {
        dev: u64,
        ino: u64,
    },
    Present {
        dev: u64,
        ino: u64,
        size: u64,
        modified: (i64, i64),
        changed: (i64, i64),
    },
}

/// A kept listing and what each file it depends on was.
struct Kept {
    stamps: Vec<(PathBuf, Stamp)>,
    listing: Vec<u8>,
}

static KEPT: Mutex<Option<HashMap<Asked, Kept>>> = Mutex::new(None);

/// The listing kept for `asked`, if every file it depends on is as it was.
pub(super) fn kept(asked: &Asked) -> Option<Vec<u8>> {
    let held = KEPT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let kept = held.as_ref()?.get(asked)?;
    kept.stamps
        .iter()
        .all(|(path, was)| restamp(path, was).as_ref() == Some(was))
        .then(|| kept.listing.clone())
}

/// Keep `listing` (`git config --list --show-origin --show-scope -z`, as git gave it) for
/// `asked`, unless it is one that is never kept ([module docs](self)). `started` is when the
/// listing's git was started.
pub(super) fn keep(asked: Asked, listing: &[u8], started: SystemTime) {
    let (Some(stamps), Some(listing)) = (
        stamps_for(&asked, listing, started),
        without_origins(listing),
    ) else {
        return;
    };
    let mut held = KEPT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let all = held.get_or_insert_with(HashMap::new);
    if all.len() >= MOST_KEPT && !all.contains_key(&asked) {
        all.clear();
    }
    all.insert(asked, Kept { stamps, listing });
}

/// The files a listing depends on, each stamped, or none where the listing is not to be kept.
fn stamps_for(asked: &Asked, listing: &[u8], started: SystemTime) -> Option<Vec<(PathBuf, Stamp)>> {
    let read = Read::of(listing, &asked.dir)?;
    let mut paths: Vec<PathBuf> = read.files.clone();
    // The repository's own files, there or not.
    let common = read.repository?;
    paths.push(common.join("config"));
    paths.push(common.join("config.worktree"));
    if let Some((_, git_dir)) = &asked.link {
        paths.push(git_dir.join("config.worktree"));
        paths.push(git_dir.join("commondir"));
    }
    // Your own files, there or not.
    let env = |name: &str| {
        asked
            .pass
            .iter()
            .find(|(passed, _)| passed == name)
            .map(|(_, value)| PathBuf::from(value))
    };
    if let Some(home) = &asked.home {
        let home = PathBuf::from(home);
        paths.push(home.join(".gitconfig"));
        paths.push(
            env("XDG_CONFIG_HOME")
                .unwrap_or_else(|| home.join(".config"))
                .join("git")
                .join("config"),
        );
    }
    paths.extend(env("GIT_CONFIG_GLOBAL"));
    paths.extend(env("GIT_CONFIG_SYSTEM"));
    paths.sort();
    paths.dedup();
    let settled = started.checked_sub(SETTLED)?;
    let mut stamps = Vec::with_capacity(paths.len());
    // Which repository git finds from the folder: each `.git` on the way up, absent, to the
    // first one there, a folder. A link names its git directory to git, so nothing is found.
    if asked.link.is_none() {
        for folder in asked.dir.ancestors() {
            let dot_git = folder.join(".git");
            match stamp(&dot_git)? {
                Stamp::Absent => stamps.push((dot_git, Stamp::Absent)),
                _ => {
                    // A `.git` file with no checked link is not one this keeps track of.
                    let found = folder_stamp(&dot_git)?;
                    stamps.push((dot_git, found));
                    break;
                }
            }
        }
    }
    for path in paths {
        let stamp = stamp(&path)?;
        if let Stamp::Present { changed, .. } = &stamp
            && !before(*changed, settled)
        {
            return None;
        }
        stamps.push((path, stamp));
    }
    Some(stamps)
}

/// Whether a file time, as (seconds, nanoseconds) since the epoch, is before `then`.
fn before((secs, nanos): (i64, i64), then: SystemTime) -> bool {
    let Ok(then) = then.duration_since(SystemTime::UNIX_EPOCH) else {
        return false;
    };
    let then = (
        i64::try_from(then.as_secs()).unwrap_or(i64::MAX),
        i64::from(then.subsec_nanos()),
    );
    (secs, nanos) < then
}

/// What a listing says it read: the files, and the repository's own folder (where its `local`
/// scope's file is).
#[derive(Debug, PartialEq, Eq)]
struct Read {
    files: Vec<PathBuf>,
    repository: Option<PathBuf>,
}

impl Read {
    /// None where the listing is never kept: an include in it, an origin that is not a file or
    /// the command line, or a listing that does not read as `scope\0origin\0key\nvalue\0`.
    fn of(listing: &[u8], dir: &Path) -> Option<Read> {
        let listing = std::str::from_utf8(listing).ok()?;
        let mut files = Vec::new();
        let mut repository = None;
        let mut fields = listing.split('\0');
        while let Some(scope) = fields.next() {
            if scope.is_empty() {
                // The listing ends with a NUL: what follows the last one is nothing.
                fields.next().is_none().then_some(())?;
                break;
            }
            let origin = fields.next()?;
            let entry = fields.next()?;
            let key = entry.split_once('\n').map_or(entry, |(key, _)| key);
            let section = key.split_once('.').map_or(key, |(section, _)| section);
            if section == "include" || section == "includeif" {
                return None;
            }
            if origin == "command line:" {
                continue;
            }
            let path = Path::new(origin.strip_prefix("file:")?);
            let path = dir.join(path);
            if scope == "local" {
                repository = Some(path.parent()?.to_path_buf());
            }
            if !files.contains(&path) {
                files.push(path);
            }
        }
        Some(Read { files, repository })
    }
}

/// A listing asked with `--show-origin` as the listing without it: `scope\0key\nvalue\0` for
/// each entry, which is what every reader of it takes. None where it does not read as
/// `scope\0origin\0entry\0` throughout.
pub(super) fn without_origins(listing: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(listing.len());
    let mut fields = listing.split(|byte| *byte == 0);
    while let Some(scope) = fields.next() {
        if scope.is_empty() {
            fields.next().is_none().then_some(())?;
            break;
        }
        let _origin = fields.next()?;
        let entry = fields.next().filter(|entry| !entry.is_empty())?;
        out.extend_from_slice(scope);
        out.push(0);
        out.extend_from_slice(entry);
        out.push(0);
    }
    Some(out)
}

/// `path` stated, following links, or none where it cannot be told.
#[cfg(unix)]
fn stamp(path: &Path) -> Option<Stamp> {
    use std::os::unix::fs::MetadataExt;
    match std::fs::metadata(path) {
        Ok(meta) => Some(Stamp::Present {
            dev: meta.dev(),
            ino: meta.ino(),
            size: meta.size(),
            modified: (meta.mtime(), meta.mtime_nsec()),
            changed: (meta.ctime(), meta.ctime_nsec()),
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(Stamp::Absent),
        Err(_) => None,
    }
}

/// `path` stated as the `.git` folder git finds, without following a link, or none where it is
/// not a folder.
#[cfg(unix)]
fn folder_stamp(path: &Path) -> Option<Stamp> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(path).ok()?;
    meta.is_dir().then(|| Stamp::Folder {
        dev: meta.dev(),
        ino: meta.ino(),
    })
}

/// `path` stated again the way it was when kept.
fn restamp(path: &Path, was: &Stamp) -> Option<Stamp> {
    match was {
        Stamp::Folder { .. } => folder_stamp(path),
        _ => stamp(path),
    }
}

#[cfg(not(unix))]
fn folder_stamp(_path: &Path) -> Option<Stamp> {
    None
}

/// Off unix nothing is kept.
#[cfg(not(unix))]
fn stamp(_path: &Path) -> Option<Stamp> {
    None
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::Duration;

    /// A repository's folders on disk with no git in them: `<case>/home`, `<case>/work` with
    /// its `.git` folder, and the repository's `config`, which sets `filter.x.clean`. Nothing here starts git; what is checked is
    /// only whether a kept listing is handed back.
    struct Case {
        _dir: tempfile::TempDir,
        case: PathBuf,
        work: PathBuf,
        config: PathBuf,
    }

    fn case() -> Case {
        let dir = tempfile::tempdir().unwrap();
        let case = std::fs::canonicalize(dir.path()).unwrap();
        let work = case.join("work");
        std::fs::create_dir_all(work.join(".git")).unwrap();
        std::fs::create_dir_all(case.join("home")).unwrap();
        // The repository's own files where the listing says they are: beside the work tree,
        // as for a git directory kept apart, which nothing here tells from `.git/config`.
        std::fs::create_dir_all(case.join("store")).unwrap();
        let config = case.join("store").join("config");
        std::fs::write(&config, "[filter \"x\"]\n\tclean = cat\n").unwrap();
        Case {
            _dir: dir,
            case,
            work,
            config,
        }
    }

    impl Case {
        fn asked(&self) -> Asked {
            Asked {
                dir: self.work.clone(),
                before_verb: Vec::new(),
                config: Vec::new(),
                pass: Vec::new(),
                link: None,
                home: Some(self.case.join("home").into_os_string()),
            }
        }

        /// What `git config --list --show-origin --show-scope -z` says of this repository.
        fn listing(&self) -> Vec<u8> {
            format!(
                "local\0file:{}\0filter.x.clean\ncat\0command\0command line:\0core.hookspath\n\0",
                self.config.display()
            )
            .into_bytes()
        }

        /// The listing kept as if its git had started a while after every file was written.
        fn keep(&self) {
            keep(
                self.asked(),
                &self.listing(),
                SystemTime::now() + Duration::from_secs(60),
            );
        }
    }

    #[test]
    fn a_kept_listing_is_handed_back_without_its_origins_while_nothing_changed() {
        let case = case();
        case.keep();
        assert_eq!(
            kept(&case.asked()),
            Some(b"local\0filter.x.clean\ncat\0command\0core.hookspath\n\0".to_vec())
        );
    }

    #[test]
    fn a_config_rewritten_in_place_with_its_old_size_and_time_is_listed_again() {
        let case = case();
        case.keep();
        let was = std::fs::metadata(&case.config).unwrap().modified().unwrap();
        // The same size, and the modification time put back: only the change time tells.
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(&case.config, "[filter \"x\"]\n\tclean = ./p\n").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&case.config)
            .unwrap()
            .set_modified(was)
            .unwrap();
        assert_eq!(kept(&case.asked()), None);
    }

    #[test]
    fn a_file_git_would_read_that_appears_after_the_listing_is_listed_again() {
        for path in [
            |c: &Case| c.case.join("store").join("config.worktree"),
            |c: &Case| c.case.join("home").join(".gitconfig"),
            |c: &Case| c.case.join("home").join(".config"),
        ] {
            let case = case();
            case.keep();
            assert!(kept(&case.asked()).is_some());
            let path = path(&case);
            if path.ends_with(".config") {
                std::fs::create_dir_all(path.join("git")).unwrap();
                std::fs::write(path.join("git").join("config"), "").unwrap();
            } else {
                std::fs::write(&path, "").unwrap();
            }
            assert_eq!(kept(&case.asked()), None, "{}", path.display());
        }
    }

    #[test]
    fn a_write_inside_the_git_folder_that_is_not_its_config_keeps_the_listing() {
        let case = case();
        case.keep();
        std::fs::write(case.work.join(".git").join("index"), "an index").unwrap();
        assert!(kept(&case.asked()).is_some());
    }

    #[test]
    fn a_git_folder_swapped_for_another_is_listed_again() {
        let case = case();
        case.keep();
        let dot_git = case.work.join(".git");
        std::fs::rename(&dot_git, case.case.join("old")).unwrap();
        std::fs::create_dir_all(&dot_git).unwrap();
        assert_eq!(kept(&case.asked()), None);
    }

    #[test]
    fn a_repository_found_nearer_the_folder_is_listed_again() {
        let case = case();
        let nearer = case.work.join("sub");
        std::fs::create_dir_all(&nearer).unwrap();
        let asked = Asked {
            dir: nearer.clone(),
            ..case.asked()
        };
        keep(
            asked.clone(),
            &case.listing(),
            SystemTime::now() + Duration::from_secs(60),
        );
        assert!(kept(&asked).is_some());
        std::fs::create_dir_all(nearer.join(".git")).unwrap();
        assert_eq!(kept(&asked), None);
    }

    #[test]
    fn a_listing_whose_files_changed_while_it_was_read_is_not_kept() {
        let case = case();
        // The file was written just now, after the listing "started" a minute ago.
        keep(
            case.asked(),
            &case.listing(),
            SystemTime::now() - Duration::from_secs(60),
        );
        assert_eq!(kept(&case.asked()), None);
    }

    #[test]
    fn a_listing_with_an_include_or_an_origin_that_is_no_file_is_not_kept() {
        let case = case();
        let config = case.config.display().to_string();
        for listing in [
            format!("local\0file:{config}\0include.path\nmore\0"),
            format!("local\0file:{config}\0includeif.onbranch:main.path\nmore\0"),
            format!("local\0file:{config}\0a.b\nc\0local\0blob:abc\0d.e\nf\0"),
            "global\0file:/g\0a.b\nc\0".to_owned(),
            "local\0file".to_owned(),
        ] {
            keep(
                case.asked(),
                listing.as_bytes(),
                SystemTime::now() + Duration::from_secs(60),
            );
            assert_eq!(kept(&case.asked()), None, "{listing:?}");
        }
    }

    #[test]
    fn only_a_listing_asked_with_settings_alone_before_the_verb_is_kept() {
        let case = case();
        let ask = |before: &[&str]| Asked::new(&case.work, before, &[], &[], None);
        assert!(ask(&[]).is_some());
        assert!(ask(&["-c", "a.b=c"]).is_some());
        assert!(ask(&["-C", "/elsewhere"]).is_none());
        assert!(ask(&["--git-dir=/elsewhere"]).is_none());
        assert!(ask(&["-c"]).is_none());
        assert!(Asked::new(Path::new("work"), &[], &[], &[], None).is_none());
    }

    #[test]
    fn the_listing_without_origins_is_what_its_readers_take() {
        assert_eq!(
            without_origins(b"local\0file:/r/config\0a.b\nc\0global\0file:/g\0d.e\nf\0"),
            Some(b"local\0a.b\nc\0global\0d.e\nf\0".to_vec())
        );
        assert_eq!(without_origins(b""), Some(Vec::new()));
        assert_eq!(without_origins(b"local\0file:/r/config\0"), None);
    }
}
