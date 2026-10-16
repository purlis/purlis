// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The run journal** (#2007, ADR 0068 §3): what a session host left running when it ended
//! without ending it, for the next host to settle.
//!
//! A host that is killed outright ends nothing on its way out: its terminals close, and the
//! kernel hangs up each program, but what each program's end leaves (its sub-agents' runs, its
//! hook spool, what is kept in the event log) is not kept, and a process that ignored the hang-up
//! runs on with nobody holding it. So the host writes down each program as it starts it, and
//! again as it ends, and the next host to serve this user reads what is left
//! ([`Journal::take_over`]).
//!
//! **One file, [`JOURNAL`], in the host's own folder**: the folder its credentials and kill
//! switch are kept in, which every chat's sandbox denies reading and writing. One JSON object a
//! line, appended and flushed to disk before the start is answered or the end told:
//!
//! - `{"started":{"program":…,"project":…,"project_path":…,"number":…,"run":…,"process":…,"process_started":…}}`
//!   as a program starts: its id in the host, the project it is in (by id and where it is on
//!   this machine), its number there, the run it begins where the window named one, its process
//!   and when that process started, as the kernel says it (what tells it from a later process
//!   given the same number);
//! - `{"ended":{"program":…}}` as it ends.
//!
//! A program with a `started` line and no `ended` one was left running. A host that ends as it
//! should, having ended every program, empties the file; the next host empties it once it has
//! read it. A line that cannot be read, a torn last one among them, is passed over: the journal
//! only ever adds to what is settled, so a line lost is a program the next host does not settle,
//! never one it settles wrongly.
//!
//! The file is this user's alone (`0600`), never written or read through a link, and read only
//! when it is a regular file this user owns (`purlis_same_user::read_private_file`).

use std::fs::File;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// The journal's file in the host's own folder.
pub const JOURNAL: &str = "journal.jsonl";

/// A program the journal says was started, as it was written down.
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
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Line {
    Started(Started),
    Ended { program: String },
}

/// The journal of one host: where it is, and the file it appends to once it is open.
pub(crate) struct Journal {
    path: PathBuf,
    file: Mutex<Option<File>>,
}

impl Journal {
    /// The journal kept in `folder`, the host's own. Nothing is read or opened until it is used.
    pub(crate) fn in_folder(folder: &Path) -> Journal {
        Journal {
            path: folder.join(JOURNAL),
            file: Mutex::new(None),
        }
    }

    /// **What the last host left running**, read and then forgotten: every program with a
    /// `started` line and no `ended` one, in the order they started. From here on the journal
    /// is this host's, and empty. Called once, before the host serves, under its lock, so no
    /// other host is writing it.
    pub(crate) fn take_over(&self) -> Vec<Started> {
        let left = match purlis_same_user::read_private_file(&self.path) {
            Ok(text) => left_running(&text),
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(why) => {
                crate::say(&format!(
                    "the run journal at {} could not be read ({why}); what the last session \
                     host left running is not settled",
                    self.path.display()
                ));
                Vec::new()
            }
        };
        let mut file = self.file();
        *file = None;
        if let Err(why) = self.open(true).map(|opened| *file = Some(opened)) {
            crate::say(&format!(
                "the run journal at {} could not be started again ({why})",
                self.path.display()
            ));
        }
        left
    }

    /// Writes down that `started` started.
    pub(crate) fn started(&self, started: Started) {
        self.write(&Line::Started(started));
    }

    /// Writes down that `program` ended.
    pub(crate) fn ended(&self, program: &str) {
        self.write(&Line::Ended {
            program: program.to_owned(),
        });
    }

    /// The host ended every program it ran, as it should: nothing is left for the next one.
    pub(crate) fn empty(&self) {
        let mut file = self.file();
        *file = None;
        match self.open(true) {
            Ok(opened) => *file = Some(opened),
            Err(why) => crate::say(&format!(
                "the run journal at {} could not be emptied ({why})",
                self.path.display()
            )),
        }
    }

    fn write(&self, line: &Line) {
        let Ok(mut said) = serde_json::to_vec(line) else {
            return;
        };
        said.push(b'\n');
        let mut file = self.file();
        if file.is_none() {
            match self.open(false) {
                Ok(opened) => *file = Some(opened),
                Err(why) => {
                    crate::say(&format!(
                        "the run journal at {} could not be opened ({why}); a crash of this \
                         session host would leave what it runs unsettled",
                        self.path.display()
                    ));
                    return;
                }
            }
        }
        let Some(open) = file.as_mut() else { return };
        if let Err(why) = open.write_all(&said).and_then(|()| open.sync_data()) {
            crate::say(&format!(
                "the run journal at {} could not be written ({why})",
                self.path.display()
            ));
        }
    }

    /// The journal's file, for appending, `0600` and never through a link: emptied first where
    /// `fresh`.
    fn open(&self, fresh: bool) -> std::io::Result<File> {
        File::options()
            .create(true)
            .append(!fresh)
            .write(true)
            .truncate(fresh)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
            .open(&self.path)
    }

    fn file(&self) -> MutexGuard<'_, Option<File>> {
        self.file
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Every program `text` says started and does not say ended, in the order they started. A line
/// that cannot be read is passed over.
fn left_running(text: &str) -> Vec<Started> {
    let mut left: Vec<Started> = Vec::new();
    for line in text.lines() {
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
        }
    }

    #[test]
    fn a_program_written_down_as_started_and_not_ended_is_left_running() {
        let folder = tempfile::tempdir().unwrap();
        let journal = Journal::in_folder(folder.path());
        journal.started(started("a", 1));
        journal.started(started("b", 2));
        journal.started(started("c", 3));
        journal.ended("b");

        let next = Journal::in_folder(folder.path());

        assert_eq!(next.take_over(), [started("a", 1), started("c", 3)]);
    }

    #[test]
    fn what_was_left_is_taken_over_once() {
        let folder = tempfile::tempdir().unwrap();
        Journal::in_folder(folder.path()).started(started("a", 1));
        assert_eq!(Journal::in_folder(folder.path()).take_over().len(), 1);

        assert!(Journal::in_folder(folder.path()).take_over().is_empty());
    }

    #[test]
    fn a_host_that_empties_its_journal_leaves_nothing_for_the_next() {
        let folder = tempfile::tempdir().unwrap();
        let journal = Journal::in_folder(folder.path());
        journal.started(started("a", 1));

        journal.empty();

        assert!(Journal::in_folder(folder.path()).take_over().is_empty());
    }

    #[test]
    fn the_host_that_took_over_writes_its_own_programs_after() {
        let folder = tempfile::tempdir().unwrap();
        Journal::in_folder(folder.path()).started(started("old", 1));
        let journal = Journal::in_folder(folder.path());
        journal.take_over();

        journal.started(started("new", 2));

        assert_eq!(
            Journal::in_folder(folder.path()).take_over(),
            [started("new", 2)]
        );
    }

    #[test]
    fn a_torn_or_unreadable_line_is_passed_over_and_the_rest_are_read() {
        let folder = tempfile::tempdir().unwrap();
        let whole = serde_json::to_string(&Line::Started(started("a", 1))).unwrap();
        let path = folder.path().join(JOURNAL);
        std::fs::write(
            &path,
            format!("not json\n{whole}\n{{\"started\":{{\"program\":\"b\""),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600))
            .unwrap();

        assert_eq!(
            Journal::in_folder(folder.path()).take_over(),
            [started("a", 1)]
        );
    }

    #[test]
    fn the_journal_is_this_users_alone() {
        use std::os::unix::fs::PermissionsExt;
        let folder = tempfile::tempdir().unwrap();
        Journal::in_folder(folder.path()).started(started("a", 1));

        let mode = std::fs::metadata(folder.path().join(JOURNAL))
            .unwrap()
            .permissions()
            .mode();

        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn a_journal_that_is_a_link_is_neither_read_nor_written_through() {
        let folder = tempfile::tempdir().unwrap();
        let elsewhere = folder.path().join("elsewhere");
        let whole = serde_json::to_string(&Line::Started(started("a", 1))).unwrap();
        std::fs::write(&elsewhere, format!("{whole}\n")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, folder.path().join(JOURNAL)).unwrap();

        let journal = Journal::in_folder(folder.path());
        assert!(journal.take_over().is_empty(), "read through a link");
        journal.started(started("b", 2));

        assert_eq!(
            std::fs::read_to_string(&elsewhere).unwrap(),
            format!("{whole}\n"),
            "written through a link"
        );
    }
}
