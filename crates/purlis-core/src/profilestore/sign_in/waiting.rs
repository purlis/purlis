// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which sign-ins a runner's logins wait for** (#4044): how the host on a runner knows a
//! browser's answer belongs to a login purlis is holding, before it connects to anything.
//!
//! The runner's held login writes one record for the `state` its page printed ([`Waiting`]), in
//! the app's state in the data home, which no chat's sandbox reads or writes (so none can lock
//! one). Named by the state's digest, so the folder says nothing of it. The record is locked for as long as its
//! holder runs, so one a holder left behind when it was killed counts for nothing. A record is
//! taken once ([`take`]): one answer goes to one login.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use sha2::Digest;

/// The folder of the app's state these records are in.
const DIR: &str = "sign-ins-waiting";

/// **Where this machine keeps its records**: none where it has no usable data home.
pub fn kept_here() -> Option<PathBuf> {
    let data = crate::datahome::root()?;
    crate::datahome::refusal(&data)
        .is_none()
        .then(|| in_data_home(&data))
}

pub(crate) fn in_data_home(data: &Path) -> PathBuf {
    data.join(crate::appstate::DIR).join(DIR)
}

/// **A login waiting for the browser's answer with one `state`**: the record is there, and locked,
/// until this is dropped.
#[derive(Debug)]
pub struct Waiting {
    path: PathBuf,
    _held: File,
}

impl Waiting {
    /// Records `state` in `dir`, made if it is not there.
    pub fn record(dir: &Path, state: &str) -> io::Result<Self> {
        private_folder(dir)?;
        let path = record_of(dir, state);
        let held = File::create(&path)?;
        held.try_lock()?;
        Ok(Self { path, _held: held })
    }
}

impl Drop for Waiting {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// **Whether a login is waiting with `state`, which takes its record**: `false` with no record, one
/// already taken, or one its holder left behind.
pub fn take(dir: &Path, state: &str) -> bool {
    let path = record_of(dir, state);
    let Ok(record) = File::open(&path) else {
        return false;
    };
    let held = matches!(record.try_lock(), Err(std::fs::TryLockError::WouldBlock));
    // Removed whoever held it: of two asking, one removes it.
    std::fs::remove_file(&path).is_ok() && held
}

/// `dir`, made private as the app's state is.
fn private_folder(dir: &Path) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)
}

fn record_of(dir: &Path, state: &str) -> PathBuf {
    let digest = sha2::Sha256::digest(state.as_bytes());
    dir.join(
        digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATE: &str = "Zm9yLXRoaXMtc2lnbi1pbi1hbG9uZQ";

    #[test]
    fn a_login_waiting_is_taken_once() {
        let dir = tempfile::tempdir().unwrap();
        let _waiting = Waiting::record(dir.path(), STATE).unwrap();

        assert!(!take(dir.path(), "another-sign-ins-state"));
        assert!(take(dir.path(), STATE));
        assert!(!take(dir.path(), STATE));
    }

    #[test]
    fn a_login_that_ended_is_not_waiting() {
        let dir = tempfile::tempdir().unwrap();
        drop(Waiting::record(dir.path(), STATE).unwrap());

        assert!(!take(dir.path(), STATE));
    }

    /// A holder that was killed left its record, and no lock on it.
    #[test]
    fn a_record_its_holder_left_behind_counts_for_nothing_and_goes() {
        let dir = tempfile::tempdir().unwrap();
        File::create(record_of(dir.path(), STATE)).unwrap();

        assert!(!take(dir.path(), STATE));
        assert!(!record_of(dir.path(), STATE).exists());
    }

    #[test]
    fn nothing_of_the_state_is_in_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let _waiting = Waiting::record(dir.path(), STATE).unwrap();

        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names.len(), 1);
        assert!(!names[0].contains(STATE));
    }
}
