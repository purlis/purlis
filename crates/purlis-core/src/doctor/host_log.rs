// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `session host log` row (#6687): `session-host.log` grown far past its cap.
//!
//! A session host that is a login item caps its own log (`crate::hostlog`), looking at its size
//! every minute. A log past twice its cap therefore means the rotation failed (the host could
//! not rename or copy in the log directory, or a host from before #6687 wrote it), and the row
//! says how large it is. Printed only when there is something to say, as `hook spool` is. A
//! stat of one file, never read; not in the preflight, which says nothing of the machine's logs.

use super::{Doctor, Row};
use crate::hostlog;

/// The row's name.
pub(super) const NAME: &str = "session host log";

/// The row for the log `d` reads, or `None` when it is within twice its cap, not there, or a
/// doctor a test names reads none.
pub(super) fn host_log(d: &Doctor) -> Option<Row> {
    if d.preflight {
        return None;
    }
    let log = d.host_log.as_ref()?;
    let meta = std::fs::symlink_metadata(log).ok()?;
    if !meta.is_file() || meta.len() <= 2 * hostlog::CAP {
        return None;
    }
    Some(Row::warn(
        NAME,
        format!(
            "{} is {}: the session host keeps it under {} with {} older files, so its rotation \
             has failed",
            super::short_path(log),
            super::sandbox::bytes_said(meta.len()),
            super::sandbox::bytes_said(hostlog::CAP),
            hostlog::KEPT
        ),
        "The log's last lines say why the host could not rotate it. Emptying the file is safe: \
         the host goes on writing into it.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::Status;

    fn doctor_reading(log: &std::path::Path) -> Doctor {
        let root = log.parent().unwrap();
        Doctor::at(root, root, false, false).reading_host_log(log)
    }

    #[test]
    fn a_log_far_past_its_cap_says_its_rotation_failed_and_how_large_it_is() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join(hostlog::NAME);
        let size = 3 * hostlog::CAP;
        std::fs::File::create(&log).unwrap().set_len(size).unwrap();

        let row = host_log(&doctor_reading(&log)).expect("a row");

        assert_eq!(row.name, NAME);
        assert_eq!(row.status, Status::Warn);
        assert!(row.detail.contains("3.1 MB"), "{}", row.detail);
        assert!(row.detail.contains(hostlog::NAME), "{}", row.detail);
        assert!(!row.hint.is_empty());
    }

    #[test]
    fn a_log_within_twice_its_cap_says_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join(hostlog::NAME);
        std::fs::File::create(&log)
            .unwrap()
            .set_len(2 * hostlog::CAP)
            .unwrap();

        assert_eq!(host_log(&doctor_reading(&log)), None);
    }

    #[test]
    fn no_log_says_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join(hostlog::NAME);

        assert_eq!(host_log(&doctor_reading(&log)), None);
    }

    #[test]
    fn a_doctor_a_test_names_never_reads_the_persons_own_log() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(host_log(&Doctor::at(dir.path(), dir.path(), false, false)), None);
    }

    #[test]
    fn the_preflight_says_nothing_of_it() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join(hostlog::NAME);
        std::fs::File::create(&log)
            .unwrap()
            .set_len(3 * hostlog::CAP)
            .unwrap();
        let preflight = Doctor::at(dir.path(), dir.path(), false, true).reading_host_log(&log);

        assert_eq!(host_log(&preflight), None);
    }
}
