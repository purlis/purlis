// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The host record** (#2007, ADR 0068 §3): the session host's own note of the programs it has
//! started and not yet ended, so that what a host left running when it was killed outright is
//! settled by the next one. It is the host's state alone, which nothing else writes or reads,
//! and not ADR 0076's run journal, which holds each run's state for the window.
//!
//! A host that is killed outright ends nothing on its way out: its terminals close, and the
//! kernel hangs up each program, but what each program's end leaves (its sub-agents' runs, its
//! hook spool, what is kept in the event log) is not kept, and a process that ignored the hang-up
//! runs on with nobody holding it. So the host notes each program as it starts it, and again as
//! it ends, and the next host to serve this user reads what is left
//! ([`HostRecord::take_over`]).
//!
//! **One file, [`HOST_RECORD`], in the host's own folder**: the folder its credentials and kill
//! switch are kept in, which every chat's sandbox denies reading and writing. One JSON object a
//! line, appended and flushed to disk before the start is answered or the end told:
//!
//! - `{"started":{"program":…,"project":…,"project_path":…,"number":…,"run":…,"process":…,"process_started":…,"boot":…}}`
//!   as a program starts: its id in the host, the project it is in (by id and where it is on
//!   this machine), its number there, the run it begins where the window named one, its process,
//!   when that process started as the kernel says it, and which boot of the machine that was
//!   (what together tell it from a later process given the same number);
//! - `{"ended":{"program":…}}` as it ends.
//!
//! A program with a `started` line and no `ended` one was left running. A host that ends as it
//! should, having ended every program, empties the file; the next host empties it once it has
//! read it. **It stays small**: once it is past [`REWRITTEN_PAST`] bytes it is written again
//! whole with only the programs still running, beside itself and renamed over it, so a write
//! that fails leaves it as it was; a file past [`READ_AT_MOST`] is not read. A line that cannot
//! be read, or is not text, a torn last one among them, is passed over alone: a line lost is a
//! program the next host does not settle, never one it settles wrongly.
//!
//! The file is this user's alone (`0600`), never written or read through a link, and read only
//! when it is a regular file this user owns that nobody else may read or write.

use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// The host record's file in the host's own folder.
pub const HOST_RECORD: &str = "hosts.jsonl";

/// How large the file grows before it is written again with only the programs still running.
pub const REWRITTEN_PAST: u64 = 256 * 1024;

/// The most of the file the next host reads: one larger is not one this host wrote.
pub const READ_AT_MOST: u64 = 4 * 1024 * 1024;

/// A program the host record says was started, as it was noted.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Started {
    /// Its id in the host.
    pub program: String,
    /// The project it ran in, by its stable id.
    pub project: String,
    /// Where that project is on this machine.
    pub project_path: PathBuf,
    /// Its number in the project.
    pub number: u32,
    /// The run it began, where the window named one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// Its process, where it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<u32>,
    /// When that process started, as the kernel says it, where it says
    /// (`purlis_same_user::Parents::started`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_started: Option<String>,
    /// The boot of the machine it started in, where the system names one ([`this_boot`]): a
    /// start counted from boot means nothing after the next one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boot: Option<String>,
    /// How its run is sandboxed, as the host applied it (#2636): none in a line an older host
    /// wrote.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<purlis_session_protocol::session::RunSandbox>,
}

/// **This boot of the machine**, where the system names it: Linux's `boot_id`, new at every
/// boot. `None` elsewhere: macOS says when a process started as a time of day, which no reboot
/// makes again.
pub fn this_boot() -> Option<String> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .ok()
            .map(|id| id.trim().to_owned())
            .filter(|id| !id.is_empty())
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        None
    }
}

/// Whether a start noted in `noted` is one of this boot, `now`: the same boot named, or no boot
/// named on a system that names none. Every doubt is another boot.
pub fn of_this_boot(noted: Option<&str>, now: Option<&str>) -> bool {
    noted == now
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Line {
    Started(Started),
    Ended { program: String },
}

/// The host record of one host: where it is, the file it appends to once it is open, and the
/// programs it has noted running.
pub(crate) struct HostRecord {
    path: PathBuf,
    /// How large the file grows before it is written again ([`REWRITTEN_PAST`]).
    rewritten_past: u64,
    /// A test's: each writing again fails, as a full disk fails it.
    #[cfg(test)]
    fails_to_write_again: bool,
    kept: Mutex<Kept>,
}

#[derive(Default)]
struct Kept {
    file: Option<File>,
    /// What the file holds, in bytes, as this host wrote it.
    bytes: u64,
    /// The programs noted started and not yet ended, in the order they started.
    running: Vec<Started>,
}

impl HostRecord {
    /// The host record kept in `folder`, the host's own. Nothing is read or opened until it is
    /// used.
    pub(crate) fn in_folder(folder: &Path) -> HostRecord {
        HostRecord {
            path: folder.join(HOST_RECORD),
            rewritten_past: REWRITTEN_PAST,
            #[cfg(test)]
            fails_to_write_again: false,
            kept: Mutex::default(),
        }
    }

    /// **What the last host left running**, read and then forgotten: every program with a
    /// `started` line and no `ended` one, in the order they started. From here on the record is
    /// this host's, and empty. Called once, before the host serves, under its lock, so no other
    /// host is writing it.
    pub(crate) fn take_over(&self) -> Vec<Started> {
        let left = match read_private(&self.path) {
            Ok(lines) => left_running(&lines),
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(why) => {
                crate::say(&format!(
                    "the host record at {} could not be read ({why}); what the last session \
                     host left running is not settled",
                    self.path.display()
                ));
                Vec::new()
            }
        };
        self.empty();
        left
    }

    /// Notes that `started` started.
    pub(crate) fn started(&self, started: Started) {
        let mut kept = self.kept();
        kept.running.retain(|one| one.program != started.program);
        kept.running.push(started.clone());
        self.write(&mut kept, &Line::Started(started));
    }

    /// Notes that `program` ended.
    pub(crate) fn ended(&self, program: &str) {
        let mut kept = self.kept();
        kept.running.retain(|one| one.program != program);
        self.write(
            &mut kept,
            &Line::Ended {
                program: program.to_owned(),
            },
        );
    }

    /// The host ended every program it ran, as it should: nothing is left for the next one.
    pub(crate) fn empty(&self) {
        let mut kept = self.kept();
        kept.running.clear();
        self.start_again(&mut kept, &[]);
    }

    fn write(&self, kept: &mut Kept, line: &Line) {
        if kept.bytes > self.rewritten_past {
            let running: Vec<Started> = kept.running.clone();
            if self.start_again(kept, &running) {
                // The line is in what was written again.
                return;
            }
        }
        let Some(said) = said(line) else { return };
        if kept.file.is_none() {
            match open(&self.path, false) {
                Ok(opened) => {
                    kept.bytes = opened.metadata().map(|meta| meta.len()).unwrap_or(0);
                    kept.file = Some(opened);
                }
                Err(why) => {
                    crate::say(&format!(
                        "the host record at {} could not be opened ({why}); a crash of this \
                         session host would leave what it runs unsettled",
                        self.path.display()
                    ));
                    return;
                }
            }
        }
        let Some(open) = kept.file.as_mut() else {
            return;
        };
        match open.write_all(&said).and_then(|()| open.sync_data()) {
            Ok(()) => kept.bytes += said.len() as u64,
            Err(why) => crate::say(&format!(
                "the host record at {} could not be written ({why})",
                self.path.display()
            )),
        }
    }

    /// The file written again from nothing, with `running` noted started, and kept open to
    /// append to. Written whole beside it first and renamed over it once on disk
    /// (`purlis_same_user::PrivateDirectory::replace_file`), so a write that fails leaves the
    /// file as it was, and it is appended to as before.
    fn start_again(&self, kept: &mut Kept, running: &[Started]) -> bool {
        let mut whole = Vec::new();
        for started in running {
            if let Some(said) = said(&Line::Started(started.clone())) {
                whole.extend_from_slice(&said);
            }
        }
        let written = self
            .write_whole(&whole)
            .and_then(|()| open(&self.path, false));
        match written {
            Ok(opened) => {
                kept.bytes = whole.len() as u64;
                kept.file = Some(opened);
                true
            }
            Err(why) => {
                crate::say(&format!(
                    "the host record at {} could not be written again ({why}); it is kept as \
                     it was",
                    self.path.display()
                ));
                false
            }
        }
    }

    /// `whole` as the file, replacing it in one rename.
    fn write_whole(&self, whole: &[u8]) -> std::io::Result<()> {
        #[cfg(test)]
        if self.fails_to_write_again {
            return Err(std::io::Error::other("the disk is full"));
        }
        let (Some(folder), Some(name)) = (
            self.path.parent(),
            self.path.file_name().and_then(|name| name.to_str()),
        ) else {
            return Err(std::io::Error::other("the host record has no folder"));
        };
        purlis_same_user::PrivateDirectory::open(folder)?.replace_file(name, whole)
    }

    fn kept(&self) -> MutexGuard<'_, Kept> {
        self.kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// One line, as it is written.
fn said(line: &Line) -> Option<Vec<u8>> {
    let mut said = serde_json::to_vec(line).ok()?;
    said.push(b'\n');
    Some(said)
}

/// The file, for appending, `0600` and never through a link: emptied first where `fresh`.
fn open(path: &Path, fresh: bool) -> std::io::Result<File> {
    File::options()
        .create(true)
        .append(!fresh)
        .write(true)
        .truncate(fresh)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(path)
}

/// Each line of `path` that is text, read only when it is a regular file, not a link, owned by
/// this user, that nobody else may read or write, and no longer than [`READ_AT_MOST`]. A line
/// that is not text, a torn last one among them, is passed over alone.
fn read_private(path: &Path) -> std::io::Result<Vec<String>> {
    let refused = |why: &str| std::io::Error::other(format!("{} {why}", path.display()));
    let mut opened = File::options()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC | nix::libc::O_NONBLOCK)
        .open(path)?;
    let found = opened.metadata()?;
    if !found.is_file() {
        return Err(refused("is not a regular file"));
    }
    if found.uid() != purlis_same_user::Uid::effective().as_raw() {
        return Err(refused("is not this user's"));
    }
    if found.mode() & 0o077 != 0 {
        return Err(refused("may be read or written by others"));
    }
    let mut bytes = Vec::new();
    (&mut opened)
        .take(READ_AT_MOST + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > READ_AT_MOST {
        return Err(refused("is larger than a host record is"));
    }
    Ok(bytes
        .split(|byte| *byte == b'\n')
        .filter_map(|line| String::from_utf8(line.to_vec()).ok())
        .collect())
}

/// Every program `lines` say started and do not say ended, in the order they started. A line
/// that cannot be read is passed over.
fn left_running(lines: &[String]) -> Vec<Started> {
    let mut left: Vec<Started> = Vec::new();
    for line in lines {
        match serde_json::from_str::<Line>(line) {
            Ok(Line::Started(started)) => {
                left.retain(|one| one.program != started.program);
                left.push(started);
            }
            Ok(Line::Ended { program }) => left.retain(|one| one.program != program),
            Err(_) => {}
        }
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started(program: &str, number: u32) -> Started {
        Started {
            program: program.to_owned(),
            project: "01J9PRJCT00000000000000000".to_owned(),
            project_path: PathBuf::from("/home/me/project"),
            number,
            run: Some(format!("run-{number}")),
            process: Some(4000 + number),
            process_started: Some("12345".to_owned()),
            boot: Some("a boot".to_owned()),
            sandbox: None,
        }
    }

    #[test]
    fn a_program_noted_as_started_and_not_ended_is_left_running() {
        let folder = tempfile::tempdir().unwrap();
        let record = HostRecord::in_folder(folder.path());
        record.started(started("a", 1));
        record.started(started("b", 2));
        record.started(started("c", 3));
        record.ended("b");

        let next = HostRecord::in_folder(folder.path());

        assert_eq!(next.take_over(), [started("a", 1), started("c", 3)]);
    }

    #[test]
    fn what_was_left_is_taken_over_once() {
        let folder = tempfile::tempdir().unwrap();
        HostRecord::in_folder(folder.path()).started(started("a", 1));
        assert_eq!(HostRecord::in_folder(folder.path()).take_over().len(), 1);

        assert!(HostRecord::in_folder(folder.path()).take_over().is_empty());
    }

    #[test]
    fn a_host_that_empties_its_record_leaves_nothing_for_the_next() {
        let folder = tempfile::tempdir().unwrap();
        let record = HostRecord::in_folder(folder.path());
        record.started(started("a", 1));

        record.empty();

        assert!(HostRecord::in_folder(folder.path()).take_over().is_empty());
    }

    #[test]
    fn the_host_that_took_over_notes_its_own_programs_after() {
        let folder = tempfile::tempdir().unwrap();
        HostRecord::in_folder(folder.path()).started(started("old", 1));
        let record = HostRecord::in_folder(folder.path());
        record.take_over();

        record.started(started("new", 2));

        assert_eq!(
            HostRecord::in_folder(folder.path()).take_over(),
            [started("new", 2)]
        );
    }

    #[test]
    fn a_long_lived_host_keeps_its_record_small_and_whole() {
        let folder = tempfile::tempdir().unwrap();
        let record = HostRecord {
            rewritten_past: 4096,
            ..HostRecord::in_folder(folder.path())
        };
        record.started(started("kept", 0));
        for n in 1..200 {
            let program = format!("p{n}");
            record.started(started(&program, n));
            record.ended(&program);
        }
        record.started(started("last", 200));

        let size = std::fs::metadata(folder.path().join(HOST_RECORD))
            .unwrap()
            .len();
        assert!(size <= 2 * 4096, "{size} bytes");
        assert_eq!(
            HostRecord::in_folder(folder.path()).take_over(),
            [started("kept", 0), started("last", 200)]
        );
    }

    #[test]
    fn a_torn_or_unreadable_line_is_passed_over_and_the_rest_are_read() {
        let folder = tempfile::tempdir().unwrap();
        let whole = serde_json::to_string(&Line::Started(started("a", 1))).unwrap();
        let path = folder.path().join(HOST_RECORD);
        std::fs::write(
            &path,
            format!("not json\n{whole}\n{{\"started\":{{\"program\":\"b\""),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600))
            .unwrap();

        assert_eq!(
            HostRecord::in_folder(folder.path()).take_over(),
            [started("a", 1)]
        );
    }

    #[test]
    fn a_line_that_is_not_text_is_passed_over_alone() {
        let folder = tempfile::tempdir().unwrap();
        let a = serde_json::to_string(&Line::Started(started("a", 1))).unwrap();
        let b = serde_json::to_string(&Line::Started(started("b", 2))).unwrap();
        let path = folder.path().join(HOST_RECORD);
        let mut bytes = format!("{a}\n").into_bytes();
        bytes.extend_from_slice(b"{\"started\":{\"program\":\"\xff\xfe\"}}\n");
        bytes.extend_from_slice(format!("{b}\n").as_bytes());
        bytes.extend_from_slice(&[0xe2, 0x82]);
        std::fs::write(&path, bytes).unwrap();
        std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600))
            .unwrap();

        assert_eq!(
            HostRecord::in_folder(folder.path()).take_over(),
            [started("a", 1), started("b", 2)]
        );
    }

    #[test]
    fn a_writing_again_that_fails_leaves_the_record_as_it_was() {
        let folder = tempfile::tempdir().unwrap();
        let record = HostRecord {
            rewritten_past: 1,
            fails_to_write_again: true,
            ..HostRecord::in_folder(folder.path())
        };
        record.started(started("a", 1));
        record.started(started("b", 2));

        record.ended("a");

        assert_eq!(
            HostRecord::in_folder(folder.path()).take_over(),
            [started("b", 2)],
            "what it had, and what was appended after"
        );
    }

    #[test]
    fn a_record_others_may_read_is_not_read() {
        let folder = tempfile::tempdir().unwrap();
        HostRecord::in_folder(folder.path()).started(started("a", 1));
        let path = folder.path().join(HOST_RECORD);
        std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o644))
            .unwrap();

        assert!(HostRecord::in_folder(folder.path()).take_over().is_empty());
    }

    #[test]
    fn the_record_is_this_users_alone() {
        use std::os::unix::fs::PermissionsExt;
        let folder = tempfile::tempdir().unwrap();
        HostRecord::in_folder(folder.path()).started(started("a", 1));

        let mode = std::fs::metadata(folder.path().join(HOST_RECORD))
            .unwrap()
            .permissions()
            .mode();

        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn a_record_that_is_a_link_is_neither_read_nor_written_through() {
        let folder = tempfile::tempdir().unwrap();
        let elsewhere = folder.path().join("elsewhere");
        let whole = serde_json::to_string(&Line::Started(started("a", 1))).unwrap();
        std::fs::write(&elsewhere, format!("{whole}\n")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, folder.path().join(HOST_RECORD)).unwrap();

        let record = HostRecord::in_folder(folder.path());
        assert!(record.take_over().is_empty(), "read through a link");
        record.started(started("b", 2));

        assert_eq!(
            std::fs::read_to_string(&elsewhere).unwrap(),
            format!("{whole}\n"),
            "written through a link"
        );
    }

    #[test]
    fn a_start_is_of_this_boot_only_where_the_same_boot_is_named() {
        assert!(of_this_boot(Some("b1"), Some("b1")));
        assert!(of_this_boot(None, None), "a system that names no boot");
        assert!(!of_this_boot(Some("b1"), Some("b2")), "another boot");
        assert!(!of_this_boot(None, Some("b2")), "noted with no boot");
        assert!(!of_this_boot(Some("b1"), None));
    }
}
