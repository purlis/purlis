// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `hook spool` row (#983): a `keys.json` this build does not read.
//!
//! **Why a person needs to be told.** A version of the spool's `keys.json` this build does not
//! read stops every drain and every new key for the project ([`crate::hookwire::spool`]): a
//! hook line the app did not take while it ran waits there, and a chat started now spools under
//! no key. Before this row that was said only in the app's log. A newer purlis writes such a
//! file, so it comes from going back a version, and the fix is going forward again.
//!
//! **Read, never drained.** The row reads the file's version and nothing else of it: no line is
//! drained, no key is added and nothing is written. Only a file that is a plain file of a
//! bounded size is read, never through a link. A spool a chat's sandbox denies to the chat is
//! not read from inside one, and says nothing there. Printed only when there is something to
//! say, as `vault files` is.

use std::path::Path;

use super::{Doctor, Row};
use crate::hookwire::spool;

/// The row's name.
pub(super) const NAME: &str = "hook spool";

/// The versions of `keys.json` this build reads: what the spool's own reader accepts, held to
/// it by `the_versions_said_here_are_the_ones_the_spool_reads`.
const READ: [u64; 2] = [1, 2];

/// Larger than any `keys.json` the host writes (one key per open chat), and small enough that a
/// doctor never holds a file it has no use for.
const BOUND: u64 = 1 << 20;

/// The row for the project `d` answers for, or `None` when there is nothing to say.
pub(super) fn hook_spool(d: &Doctor) -> Option<Row> {
    if !d.has_plane {
        return None;
    }
    row_for(&d.root)
}

/// The row for the project at `root`: each spelling of its state folder's `app/spool/`.
pub(super) fn row_for(root: &Path) -> Option<Row> {
    crate::names::STATE_DIR
        .spellings()
        .find_map(|state| row_of(&root.join(state).join("app").join(spool::DIR)))
}

/// The row for the spool `dir`, or `None` where its `keys.json` is one this build reads, is not
/// there, or is not this process's to read.
fn row_of(dir: &Path) -> Option<Row> {
    let path = dir.join(spool::KEYS);
    let meta = std::fs::symlink_metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > BOUND {
        return None;
    }
    let bytes = crate::contain::read_no_link(dir, &path).ok()?;
    #[derive(serde::Deserialize)]
    struct Versioned {
        v: serde_json::Value,
    }
    // A file that does not read as JSON with a version is read as no keys at all by the spool,
    // which goes on draining; it is not this row's to say.
    let Versioned { v } = serde_json::from_slice(&bytes).ok()?;
    if v.as_u64().is_some_and(|n| READ.contains(&n)) {
        return None;
    }
    Some(Row::warn(
        NAME,
        format!(
            "{} is version {}, which this purlis does not read: no hook line spooled here is \
             drained, and no chat started now spools under a key, until a purlis that reads it \
             runs here",
            super::one_line(&path.display().to_string(), super::PATH_DISPLAY_LIMIT),
            super::one_line(&v.to_string(), super::DISPLAY_LIMIT)
        ),
        "A newer purlis wrote it. Update purlis to that version or later: the file is left as \
         it is, and the lines it checks wait for that build. Nothing a chat wrote is lost while \
         they wait.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::Status;
    use crate::hookwire::ChatTokens;

    /// A project whose spool holds `keys` as its `keys.json`.
    fn spool_with(keys: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("a directory");
        let spool = dir.path().join(".purlis/app").join(spool::DIR);
        std::fs::create_dir_all(&spool).expect("the spool");
        std::fs::write(spool.join(spool::KEYS), keys).expect("the keys");
        (dir, spool)
    }

    #[test]
    fn a_keys_file_this_build_does_not_read_is_said_where_a_person_looks() {
        let (dir, spool) = spool_with(r#"{"v":3,"keys":[]}"#);
        let before = std::fs::read(spool.join(spool::KEYS)).unwrap();

        let row = row_for(dir.path()).expect("a row");

        assert_eq!(row.name, "hook spool");
        assert_eq!(row.status, Status::Warn);
        assert_eq!(
            row.detail,
            format!(
                "{} is version 3, which this purlis does not read: no hook line spooled here is \
                 drained, and no chat started now spools under a key, until a purlis that reads \
                 it runs here",
                spool.join(spool::KEYS).display()
            )
        );
        assert!(
            row.hint.starts_with("A newer purlis wrote it."),
            "{}",
            row.hint
        );
        // Read, never written.
        assert_eq!(std::fs::read(spool.join(spool::KEYS)).unwrap(), before);
    }

    #[test]
    fn a_project_with_no_spool_or_one_this_build_reads_has_no_row() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(row_for(dir.path()), None);
        for keys in [r#"{"v":1,"keys":[]}"#, r#"{"v":2,"keys":[]}"#, "{not json"] {
            let (dir, _spool) = spool_with(keys);
            assert_eq!(row_for(dir.path()), None, "{keys}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_keys_file_reached_through_a_link_is_not_read() {
        let (dir, spool) = spool_with("");
        let elsewhere = tempfile::tempdir().unwrap();
        let target = elsewhere.path().join("keys.json");
        std::fs::write(&target, r#"{"v":3,"keys":[]}"#).unwrap();
        std::fs::remove_file(spool.join(spool::KEYS)).unwrap();
        std::os::unix::fs::symlink(&target, spool.join(spool::KEYS)).unwrap();
        assert_eq!(row_for(dir.path()), None);
    }

    #[test]
    fn the_versions_said_here_are_the_ones_the_spool_reads() {
        // Differential: for each version, the row warns exactly where the spool's own reader
        // refuses the file, which is where a key cannot be added.
        for version in ["0", "1", "2", "3", "3.0", "\"2\"", "null", "4294967298"] {
            let (dir, spool) = spool_with(&format!(r#"{{"v":{version},"keys":[]}}"#));
            let token = ChatTokens::spooling_into(spool.clone())
                .issue_to_this_process(5)
                .ok();
            // Issuing writes the key, so the file is set back before the spool is asked.
            std::fs::write(
                spool.join(spool::KEYS),
                format!(r#"{{"v":{version},"keys":[]}}"#),
            )
            .unwrap();
            let refused = match token {
                Some(token) => spool::remember(&spool, 6, &token).is_err(),
                None => true,
            };
            assert_eq!(row_for(dir.path()).is_some(), refused, "version {version}");
        }
    }
}
