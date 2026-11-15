// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `session host log` row (#6687): `session-host.log` grown far past its cap while a
//! session host serves.
//!
//! A session host that is a login item trims its own log (`crate::hostlog`), looking at its
//! size every minute. A log past twice its cap while a host serves therefore means the trimming
//! has stopped working (the host could not copy or rename in the log directory), and the row
//! says how large it is and how to empty it. With no host serving, a large log is one a host
//! from before #6687 left, which nothing writes to any more: the row says nothing of it.
//! Printed only when there is something to say, as `hook spool` is. A stat of one file, never
//! read, and a connect to the host's socket; not in the preflight, which says nothing of the
//! machine's logs.

use std::path::PathBuf;

use super::{Doctor, Row};
use crate::hostlog;

/// The row's name.
pub(super) const NAME: &str = "session host log";

/// The log the row looks at, and whether a session host serves now.
#[derive(Debug, Clone)]
pub(crate) struct HostLog {
    pub(crate) log: PathBuf,
    pub(crate) serving: fn() -> bool,
}

impl HostLog {
    /// This user's log, and their session host's socket.
    pub(crate) fn here() -> Option<Self> {
        Some(Self {
            log: hostlog::path()?,
            serving: a_host_serves_here,
        })
    }
}

/// Whether this user's session host answers on its socket now.
fn a_host_serves_here() -> bool {
    #[cfg(unix)]
    {
        std::os::unix::net::UnixStream::connect(crate::hostsocket::here()).is_ok()
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// The row for the log `d` reads, or `None` when it is within twice its cap, not there, no
/// session host serves, or a doctor a test names reads none.
pub(super) fn host_log(d: &Doctor) -> Option<Row> {
    if d.preflight {
        return None;
    }
    let HostLog { log, serving } = d.host_log.as_ref()?;
    let meta = std::fs::symlink_metadata(log).ok()?;
    if !meta.is_file() || meta.len() <= 2 * hostlog::CAP || !serving() {
        return None;
    }
    let shown = super::short_path(log);
    Some(Row::warn(
        NAME,
        format!(
            "{shown} is {}; purlis keeps it under {}, so trimming it has stopped working",
            super::sandbox::bytes_said(meta.len()),
            hostlog::CAP_SAID,
        ),
        format!(
            "Its last lines say why. To empty it, run `: > {shown}`: the session host goes on \
             writing into it. Deleting `{shown}.1` to `.{}` frees the older copies.",
            hostlog::KEPT
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::Status;

    fn doctor_reading(log: &std::path::Path, serving: fn() -> bool) -> Doctor {
        let root = log.parent().unwrap();
        Doctor::at(root, root, false, false).reading_host_log(log, serving)
    }

    fn log_of(dir: &tempfile::TempDir, size: u64) -> PathBuf {
        let log = dir.path().join(hostlog::NAME);
        std::fs::File::create(&log).unwrap().set_len(size).unwrap();
        log
    }

    #[test]
    fn a_log_far_past_its_cap_while_a_host_serves_says_how_large_it_is_and_how_to_empty_it() {
        let dir = tempfile::tempdir().unwrap();
        let log = log_of(&dir, 3 * hostlog::CAP);

        let row = host_log(&doctor_reading(&log, || true)).expect("a row");

        assert_eq!(row.name, NAME);
        assert_eq!(row.status, Status::Warn);
        assert!(row.detail.contains("is 3.0 MB"), "{}", row.detail);
        assert!(row.detail.contains("under 1 MB"), "{}", row.detail);
        assert!(row.detail.contains(hostlog::NAME), "{}", row.detail);
        assert!(!row.detail.contains("rotat"), "{}", row.detail);
        assert!(row.hint.contains("`: > "), "{}", row.hint);
        assert!(row.hint.contains(".1` to `.3`"), "{}", row.hint);
    }

    #[test]
    fn a_large_log_with_no_host_serving_says_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let log = log_of(&dir, 3 * hostlog::CAP);

        assert_eq!(host_log(&doctor_reading(&log, || false)), None);
    }

    #[test]
    fn a_log_within_twice_its_cap_says_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let log = log_of(&dir, 2 * hostlog::CAP);

        assert_eq!(host_log(&doctor_reading(&log, || true)), None);
    }

    #[test]
    fn no_log_says_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join(hostlog::NAME);

        assert_eq!(host_log(&doctor_reading(&log, || true)), None);
    }

    #[test]
    fn a_doctor_a_test_names_never_reads_the_persons_own_log() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            host_log(&Doctor::at(dir.path(), dir.path(), false, false)),
            None
        );
    }

    #[test]
    fn the_preflight_says_nothing_of_it() {
        let dir = tempfile::tempdir().unwrap();
        let log = log_of(&dir, 3 * hostlog::CAP);
        let preflight =
            Doctor::at(dir.path(), dir.path(), false, true).reading_host_log(&log, || true);

        assert_eq!(host_log(&preflight), None);
    }
}
