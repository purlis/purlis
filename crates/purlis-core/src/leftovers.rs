// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Temp files an older charter left behind when it was killed mid-write (#440).
//!
//! Before #432 and #434 each whole-file writer named its own temp file beside its target:
//! `reopen.json.writing`, `machine.json.<pid>.<tag>.writing` and so on. Every writer now goes
//! through [`crate::rewrite::replace`], whose temps are `.purlis-generated.*.tmp`, so nothing
//! that runs today ever writes the old names again — and a temp an older charter was killed
//! in front of is never renamed away or cleaned up by anyone.
//!
//! **Swept when the app opens a plane**, which is when every writer that used the old names
//! (the reopen record, the launched-profiles record, the machine store, the extension record,
//! the window layout) is about to run again. Only a name that is exactly one of the old
//! shapes, a plain file (never a link), and older than [`STALE_AFTER`] is removed: a name
//! charter did not make is not charter's to remove, and a temp an older charter still running
//! beside this one is writing right now is younger than that.
//!
//! The gate on the way to the directory is a `stat`, and the removal is by path, so a
//! directory swapped for a link in between is ADR 0028's shared window; what could be lost
//! there is a file with exactly one of these names, older than the threshold.

use std::path::Path;
use std::time::{Duration, SystemTime};

/// How old a leftover must be before it is removed. A whole-file write takes milliseconds;
/// this is long enough that an older charter still running beside this one is never cut off
/// mid-write.
pub const STALE_AFTER: Duration = Duration::from_secs(10 * 60);

/// The old names that carried a pid and a per-call tag: `<stem>.<pid>.<tag>.<ending>`.
const TAGGED: [(&str, &str); 4] = [
    ("machine.json", "writing"),
    ("extensions.json", "writing"),
    ("layout.json", "writing"),
    ("harness-profiles-launched.json", "tmp"),
];

/// The one old name that carried neither: the reopen record's.
const FIXED: &str = "reopen.json.writing";

/// Whether `name` is exactly a temp file name an older charter used.
pub fn is_old_temp(name: &str) -> bool {
    if name == FIXED {
        return true;
    }
    TAGGED.iter().any(|(stem, ending)| {
        let Some(middle) = name
            .strip_prefix(stem)
            .and_then(|rest| rest.strip_prefix('.'))
            .and_then(|rest| rest.strip_suffix(ending))
            .and_then(|rest| rest.strip_suffix('.'))
        else {
            return false;
        };
        let Some((pid, tag)) = middle.split_once('.') else {
            return false;
        };
        // The pid in decimal, and `workspaces::scratch_tag`'s twelve lowercase hex digits.
        !pid.is_empty()
            && pid.bytes().all(|b| b.is_ascii_digit())
            && tag.len() == 12
            && tag.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    })
}

/// Remove the leftovers in `dir` older than [`STALE_AFTER`] at `now`; how many went.
///
/// `dir` is reached from `root` with no link on the way, or nothing is swept. A leftover is
/// asked about without following a link (`DirEntry::file_type` is the entry's own), and a
/// link is left alone: it is not a temp charter made.
pub fn sweep(root: &Path, dir: &Path, now: SystemTime) -> usize {
    if crate::contain::no_link_on_the_way(root, dir).is_err() {
        return 0;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        if !is_old_temp(&entry.file_name().to_string_lossy()) {
            continue;
        }
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        let old_enough = entry
            .metadata()
            .and_then(|found| found.modified())
            .is_ok_and(|at| now.duration_since(at).is_ok_and(|age| age >= STALE_AFTER));
        if old_enough && std::fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// The old sandbox-blocks store in the state folder's `app/` (#1338): nothing reads or writes it
/// since the network record took its place (#1662).
const OLD_BLOCKS: &str = "sandbox-blocks.json";

/// The plane's leftovers: `.charter/` (the launched-profiles record's) and `.charter/app/`
/// (the reopen record's), and the old sandbox-blocks store there (#1681), on the same terms:
/// a plain file older than [`STALE_AFTER`], with no link on the way.
pub fn sweep_plane(root: &Path) -> usize {
    let state = crate::names::state(root);
    let now = SystemTime::now();
    let app = state.join("app");
    sweep(root, &state, now) + sweep(root, &app, now) + sweep_old_blocks(root, &app, now)
}

/// [`OLD_BLOCKS`] in `app`, removed on [`sweep`]'s terms and logged: 1 where it went.
fn sweep_old_blocks(root: &Path, app: &Path, now: SystemTime) -> usize {
    if crate::contain::no_link_on_the_way(root, app).is_err() {
        return 0;
    }
    let old = app.join(OLD_BLOCKS);
    let Ok(found) = std::fs::symlink_metadata(&old) else {
        return 0;
    };
    let old_enough = found
        .modified()
        .is_ok_and(|at| now.duration_since(at).is_ok_and(|age| age >= STALE_AFTER));
    if !found.is_file() || !old_enough || std::fs::remove_file(&old).is_err() {
        return 0;
    }
    tracing::info!(
        "purlis: removed {}, the old sandbox-blocks store nothing reads since the network \
         record took its place",
        old.display()
    );
    1
}

/// The machine's leftovers, in the config home's charter directory: the store's, the
/// extension record's and the window layout's.
pub fn sweep_config(config_root: &Path) -> usize {
    sweep(
        config_root,
        &crate::machine::dir(config_root),
        SystemTime::now(),
    )
}

// ---- the homes of projects that are gone (#1364) ---------------------------------------------

/// How long a project's homes under purlis's data home must have gone unused, besides the
/// project being gone, before [`sweep_homes`] removes them: past any chat that could still be
/// running in a folder taken away under it.
pub const HOME_IDLE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// The note purlis writes in a project's cache home at every chat start that uses it
/// ([`crate::sandbox::caches::CacheHome::prepare`]): the project's path, as the system resolved
/// it when the home was named for it. A chat never writes the cache home's own folder.
pub const HOME_NOTE: &str = ".purlis-project";

/// The most of [`HOME_NOTE`] read: one path.
const HOME_NOTE_MAX: u64 = 4096;

/// The folders under purlis's data home that hold one home per project, each named for the
/// project ([`crate::sandbox::Homes::project_key`]).
const PROJECT_HOMES: [&str; 2] = ["cache-homes", "codex-homes"];

/// What a home being removed is renamed to first, beside it, so a removal cut short leaves
/// nothing that reads as a project's home, and the next sweep finishes it.
const REMOVING: &str = ".removing-";

/// **Removes the cache and Codex homes of projects that are gone** (#1364), under `data_home`,
/// so they do not cost disk for ever; answers each home removed. A project's homes go only
/// when every one of these holds, and are otherwise left as they are:
///
/// - its cache home holds [`HOME_NOTE`], a plain file reached through no link, naming an
///   absolute path whose digest is the home's own name: the note says which project the home
///   is, and nothing written there can name another;
/// - that path is gone, and the folder it was in is still there: a project on a disk that is
///   not mounted is not gone;
/// - neither home, nor anything directly in it, was changed in [`HOME_IDLE`] before `now`: no
///   chat started on it, or wrote in it, for that long.
///
/// A home with no note is left: which project it is cannot be told. Nothing is followed
/// through a link, and a home that is a link is never removed.
pub fn sweep_homes(data_home: &Path, now: SystemTime) -> Vec<std::path::PathBuf> {
    let mut removed = Vec::new();
    for kind in PROJECT_HOMES {
        finish_removals(&data_home.join(kind));
    }
    let caches = data_home.join(PROJECT_HOMES[0]);
    if !is_plain_dir(data_home) || !is_plain_dir(&caches) {
        return removed;
    }
    let Ok(entries) = std::fs::read_dir(&caches) else {
        return removed;
    };
    for entry in entries.flatten() {
        let key = entry.file_name().to_string_lossy().into_owned();
        if !is_project_key(&key) || !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let Some(project) = project_of(&entry.path(), &key) else {
            continue;
        };
        if !gone(&project) {
            continue;
        }
        let homes: Vec<std::path::PathBuf> = PROJECT_HOMES
            .iter()
            .map(|kind| data_home.join(kind).join(&key))
            .filter(|home| std::fs::symlink_metadata(home).is_ok())
            .collect();
        if homes
            .iter()
            .any(|home| !is_plain_dir(home) || used_since(home, now))
        {
            continue;
        }
        for home in homes {
            if remove_home(&home) {
                removed.push(home);
            }
        }
    }
    removed
}

/// Whether `name` is a name [`crate::sandbox::Homes::project_key`] gives: 32 lowercase hex
/// digits.
fn is_project_key(name: &str) -> bool {
    name.len() == 32 && name.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Whether `path` is a folder, itself and not through a link.
fn is_plain_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir())
}

/// The project the cache home at `home`, named `key`, says it is ([`HOME_NOTE`]): only an
/// absolute path whose digest is `key`.
fn project_of(home: &Path, key: &str) -> Option<std::path::PathBuf> {
    let note = home.join(HOME_NOTE);
    let meta = std::fs::symlink_metadata(&note).ok()?;
    if !meta.is_file() || meta.len() > HOME_NOTE_MAX {
        return None;
    }
    let text = std::fs::read_to_string(&note).ok()?;
    let project = std::path::PathBuf::from(text.trim_end_matches('\n'));
    (project.is_absolute() && crate::sandbox::Homes::project_key(&project) == key)
        .then_some(project)
}

/// Whether the project at `project` is gone: nothing is at its path, and the folder it was in
/// is there, so its disk is mounted.
fn gone(project: &Path) -> bool {
    let nothing_there = matches!(
        std::fs::symlink_metadata(project),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound
    );
    nothing_there
        && project
            .parent()
            .is_some_and(|parent| is_plain_dir(parent) && !holds_mounts(parent))
}

/// Whether `folder` is one disks are mounted in (train 77 review): a project that was a whole
/// disk, `/Volumes/Backup` unplugged, leaves nothing at its path while the folder it was in is
/// still there, so the project is not known to be gone.
fn holds_mounts(folder: &Path) -> bool {
    let at = |path: &str| folder == Path::new(path);
    let under = |path: &str| folder.parent() == Some(Path::new(path));
    at("/Volumes") || at("/mnt") || at("/media") || under("/media") || under("/run/media")
}

/// Whether `home`, or anything directly in it, was changed less than [`HOME_IDLE`] before
/// `now`. A time that cannot be read counts as now.
fn used_since(home: &Path, now: SystemTime) -> bool {
    let recent = |meta: std::io::Result<std::fs::Metadata>| {
        meta.and_then(|meta| meta.modified()).map_or(true, |at| {
            now.duration_since(at).map_or(true, |age| age < HOME_IDLE)
        })
    };
    if recent(std::fs::symlink_metadata(home)) {
        return true;
    }
    let Ok(entries) = std::fs::read_dir(home) else {
        return true;
    };
    entries
        .map(|entry| entry.map(|entry| entry.path()))
        .any(|entry| entry.map_or(true, |path| recent(std::fs::symlink_metadata(path))))
}

/// Renames `home` aside ([`REMOVING`]), then removes it whole, never through a link.
fn remove_home(home: &Path) -> bool {
    let (Some(parent), Some(name)) = (home.parent(), home.file_name()) else {
        return false;
    };
    let aside = parent.join(format!("{REMOVING}{}", name.to_string_lossy()));
    if std::fs::rename(home, &aside).is_err() {
        return false;
    }
    let _ = std::fs::remove_dir_all(&aside);
    true
}

/// Removes what a sweep cut short renamed aside in `homes` ([`REMOVING`]).
fn finish_removals(homes: &Path) {
    if !is_plain_dir(homes) {
        return;
    }
    let Ok(entries) = std::fs::read_dir(homes) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let left = name.strip_prefix(REMOVING).is_some_and(is_project_key);
        if left && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file at `path`, last modified `age` ago.
    fn left(path: &Path, age: Duration) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"half a record").unwrap();
        let file = std::fs::File::options().write(true).open(path).unwrap();
        file.set_modified(SystemTime::now() - age).unwrap();
    }

    const OLD: Duration = Duration::from_secs(60 * 60);

    #[test]
    fn the_names_an_older_charter_used_are_known_and_nothing_else_is() {
        for name in [
            "reopen.json.writing",
            "machine.json.4242.0123456789ab.writing",
            "extensions.json.1.abcdefabcdef.writing",
            "layout.json.77.000000000000.writing",
            "harness-profiles-launched.json.9.0a0b0c0d0e0f.tmp",
        ] {
            assert!(is_old_temp(name), "{name}");
        }
        for name in [
            "reopen.json",
            "notes.writing",
            "reopen.json.writing.bak",
            "machine.json.writing",
            "machine.json.4242.writing",
            "machine.json.x42.0123456789ab.writing",
            "machine.json.4242.0123456789AB.writing",
            "machine.json.4242.0123456789a.writing",
            "machine.json.4242.0123456789ab.tmp",
            "layout.json.77.000000000000.writing.x",
            "harness-profiles-launched.json.9.0a0b0c0d0e0f.writing",
            ".charter-generated.reopen.json.1.0123456789ab.tmp",
        ] {
            assert!(!is_old_temp(name), "{name}");
        }
    }

    #[test]
    fn a_planes_sweep_removes_the_temps_an_older_charter_left_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let reopen = root.join(".charter/app/reopen.json.writing");
        let launched = root.join(".charter/harness-profiles-launched.json.12.0123456789ab.tmp");
        let record = root.join(".charter/app/reopen.json");
        let theirs = root.join(".charter/app/notes.writing");
        let fresh = root.join(".charter/harness-profiles-launched.json.13.0123456789ab.tmp");
        left(&reopen, OLD);
        left(&launched, OLD);
        left(&record, OLD);
        left(&theirs, OLD);
        left(&fresh, Duration::ZERO);

        assert_eq!(sweep_plane(root), 2);

        assert!(!reopen.exists() && !launched.exists());
        assert!(record.exists() && theirs.exists() && fresh.exists());
    }

    /// #1681: the old sandbox-blocks store, which nothing reads or writes since #1662, is taken
    /// away once the project opens; a link of that name, or a file another purlis may be
    /// writing this minute, is left alone.
    #[test]
    fn the_old_sandbox_blocks_store_is_taken_away_when_the_project_opens() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let state = crate::names::state(root).join("app");
        let old = state.join("sandbox-blocks.json");
        left(&old, OLD);
        assert_eq!(sweep_plane(root), 1);
        assert!(!old.exists());
        assert_eq!(sweep_plane(root), 0, "once");

        left(&old, Duration::from_secs(5));
        assert_eq!(sweep_plane(root), 0, "another purlis may be writing it");
        std::fs::remove_file(&old).unwrap();

        #[cfg(unix)]
        {
            let elsewhere = root.join("kept.json");
            left(&elsewhere, OLD);
            std::os::unix::fs::symlink(&elsewhere, &old).unwrap();
            assert_eq!(sweep_plane(root), 0);
            assert!(elsewhere.exists() && old.symlink_metadata().is_ok());
        }
    }

    #[test]
    fn a_temp_younger_than_the_threshold_is_left_for_the_charter_writing_it() {
        let dir = tempfile::tempdir().unwrap();
        let reopen = dir.path().join(".charter/app/reopen.json.writing");
        left(&reopen, Duration::from_secs(5));
        assert_eq!(sweep_plane(dir.path()), 0);
        assert!(reopen.exists());
    }

    #[test]
    fn the_config_homes_leftovers_are_swept_too() {
        let dir = tempfile::tempdir().unwrap();
        let home = crate::machine::dir(dir.path());
        let names = [
            "machine.json.4242.0123456789ab.writing",
            "extensions.json.4242.0123456789ab.writing",
            "layout.json.4242.0123456789ab.writing",
        ];
        for name in names {
            left(&home.join(name), OLD);
        }
        left(&home.join("machine.json"), OLD);

        assert_eq!(sweep_config(dir.path()), 3);

        assert!(names.iter().all(|name| !home.join(name).exists()));
        assert!(home.join("machine.json").exists());
    }

    /// A leftover's name that is a link is left alone, and so is what it points at; a
    /// `.charter/` that is a link is not swept at all.
    #[cfg(unix)]
    #[test]
    fn a_link_is_never_swept_and_never_swept_through() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("plane");
        let elsewhere = tempfile::tempdir().unwrap();
        let theirs = elsewhere.path().join("theirs");
        left(&theirs, OLD);
        let at = root.join(".charter/app/reopen.json.writing");
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&theirs, &at).unwrap();

        assert_eq!(sweep_plane(&root), 0);
        assert!(at.is_symlink() && theirs.exists());

        std::fs::remove_dir_all(root.join(".charter")).unwrap();
        left(&elsewhere.path().join("app/reopen.json.writing"), OLD);
        std::os::unix::fs::symlink(elsewhere.path(), root.join(".charter")).unwrap();
        assert_eq!(sweep_plane(&root), 0);
        assert!(elsewhere.path().join("app/reopen.json.writing").exists());
    }

    // ---- the homes of projects that are gone (#1364) -----------------------------------------

    /// A data home with one project's cache and Codex homes, the project at `project`, and the
    /// note naming it.
    fn homes_of(data: &Path, project: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
        let key = crate::sandbox::Homes::project_key(project);
        let cache = data.join("cache-homes").join(&key);
        let codex = data.join("codex-homes").join(&key);
        std::fs::create_dir_all(cache.join("cargo/registry")).unwrap();
        std::fs::create_dir_all(codex.join("sessions")).unwrap();
        std::fs::write(cache.join(HOME_NOTE), project.display().to_string()).unwrap();
        (cache, codex)
    }

    /// Well past [`HOME_IDLE`] after everything here was written.
    fn later() -> SystemTime {
        SystemTime::now() + HOME_IDLE + Duration::from_secs(24 * 60 * 60)
    }

    fn a_base() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        (dir, base)
    }

    #[test]
    fn the_homes_of_a_project_that_is_gone_are_removed_once_unused_long_enough() {
        let (_dir, base) = a_base();
        let data = base.join("data");
        std::fs::create_dir_all(base.join("projects")).unwrap();
        let project = base.join("projects/gone");
        let (cache, codex) = homes_of(&data, &project);

        assert!(
            sweep_homes(&data, SystemTime::now()).is_empty(),
            "used just now"
        );
        assert!(cache.is_dir() && codex.is_dir());

        let removed = sweep_homes(&data, later());

        assert_eq!(removed, [cache.clone(), codex.clone()]);
        assert!(!cache.exists() && !codex.exists());
        assert_eq!(
            std::fs::read_dir(data.join("cache-homes")).unwrap().count(),
            0,
            "nothing is left aside"
        );
    }

    #[test]
    fn a_project_that_is_there_or_on_a_disk_not_mounted_keeps_its_homes() {
        let (_dir, base) = a_base();
        let data = base.join("data");
        let here = base.join("here");
        std::fs::create_dir_all(&here).unwrap();
        let unmounted = base.join("Volumes/Gone/project");
        let homes = [homes_of(&data, &here), homes_of(&data, &unmounted)];

        assert!(sweep_homes(&data, later()).is_empty());
        for (cache, codex) in homes {
            assert!(cache.is_dir() && codex.is_dir());
        }
    }

    /// Train 77 review: a project that was a whole disk, now unplugged, is not gone.
    #[test]
    fn a_project_that_was_a_whole_disk_is_not_known_to_be_gone() {
        for project in [
            "/Volumes/purlis-test-unplugged",
            "/mnt/purlis-test-unplugged",
            "/media/someone/purlis-test-unplugged",
            "/run/media/someone/purlis-test-unplugged",
        ] {
            assert!(!gone(Path::new(project)), "{project}");
        }
        let (_dir, base) = a_base();
        assert!(gone(&base.join("deleted")));
    }

    #[cfg(unix)]
    #[test]
    fn a_home_whose_note_is_missing_a_link_or_names_another_project_is_kept() {
        let (_dir, base) = a_base();
        let data = base.join("data");
        std::fs::create_dir_all(base.join("projects")).unwrap();
        // No note: which project it is cannot be told.
        let (bare, _) = homes_of(&data, &base.join("projects/a"));
        std::fs::remove_file(bare.join(HOME_NOTE)).unwrap();
        // A note naming a gone project that is not the one the home is named for.
        let (other, _) = homes_of(&data, &base.join("projects/b"));
        std::fs::write(
            other.join(HOME_NOTE),
            base.join("projects/c").display().to_string(),
        )
        .unwrap();
        // A note that is a link to a file naming the right project.
        let (linked, _) = homes_of(&data, &base.join("projects/d"));
        let outside = base.join("note");
        std::fs::write(&outside, base.join("projects/d").display().to_string()).unwrap();
        std::fs::remove_file(linked.join(HOME_NOTE)).unwrap();
        std::os::unix::fs::symlink(&outside, linked.join(HOME_NOTE)).unwrap();

        assert!(sweep_homes(&data, later()).is_empty());
        assert!(bare.is_dir() && other.is_dir() && linked.is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn a_home_that_is_a_link_is_never_removed_or_removed_through() {
        let (_dir, base) = a_base();
        let data = base.join("data");
        std::fs::create_dir_all(base.join("projects")).unwrap();
        let project = base.join("projects/gone");
        let (cache, codex) = homes_of(&data, &project);
        let elsewhere = base.join("elsewhere");
        std::fs::rename(&codex, &elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &codex).unwrap();

        assert!(sweep_homes(&data, later()).is_empty());
        assert!(cache.is_dir() && elsewhere.join("sessions").is_dir());
    }

    #[test]
    fn a_removal_cut_short_is_finished_by_the_next_sweep() {
        let (_dir, base) = a_base();
        let data = base.join("data");
        let aside = data.join(format!("cache-homes/{REMOVING}{}", "0".repeat(32)));
        std::fs::create_dir_all(aside.join("npm")).unwrap();
        let theirs = data.join("cache-homes/.removing-notes");
        std::fs::create_dir_all(&theirs).unwrap();

        sweep_homes(&data, SystemTime::now());

        assert!(!aside.exists());
        assert!(
            theirs.is_dir(),
            "a name purlis did not give is not purlis's to remove"
        );
    }
}
