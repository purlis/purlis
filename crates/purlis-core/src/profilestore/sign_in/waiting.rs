// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which sign-ins a runner's logins wait for** (#4044): how the host on a runner knows a
//! browser's answer belongs to a login purlis is holding, before it connects to anything.
//!
//! The runner's held login writes one record for the `state` its page printed ([`Waiting`]), in
//! a folder of the data home no chat's sandbox reads or writes (so none can lock
//! one). Named by the state's digest, so the folder says nothing of it. The record is locked for as long as its
//! holder runs, so one a holder left behind when it was killed counts for nothing. A record is
//! taken once ([`take`]): one answer goes to one login. It says which of the loopback's addresses
//! the login was found listening on when its page was printed, and the login's process group, so
//! the answer goes to a socket of that group ([`super::callback`]).

use std::fs::File;
use std::io::{self, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use sha2::Digest;

/// The folder of the data home these records are in.
const DIR: &str = "sign-ins-waiting";

/// **Where this machine keeps its records**: none where it has no usable data home.
pub fn kept_here() -> Option<PathBuf> {
    let data = crate::datahome::root()?;
    crate::datahome::refusal(&data)
        .is_none()
        .then(|| in_data_home(&data))
}

pub(crate) fn in_data_home(data: &Path) -> PathBuf {
    data.join(DIR)
}

/// **A login waiting for the browser's answer with one `state`**: the record is there, and locked,
/// until this is dropped.
#[derive(Debug)]
pub struct Waiting {
    path: PathBuf,
    _held: File,
}

impl Waiting {
    /// Records `state` in `dir`, made if it is not there, of a login not found listening.
    pub fn record(dir: &Path, state: &str) -> io::Result<Self> {
        Self::record_at(dir, state, None)
    }

    /// [`Self::record`], of a login found listening on `at`.
    pub fn record_at(dir: &Path, state: &str, at: Option<IpAddr>) -> io::Result<Self> {
        Self::record_of_group(dir, state, at, None)
    }

    /// [`Self::record_at`], of a login in the process group `group`.
    pub fn record_of_group(
        dir: &Path,
        state: &str,
        at: Option<IpAddr>,
        group: Option<u32>,
    ) -> io::Result<Self> {
        private_folder(dir)?;
        let path = record_of(dir, state);
        let mut held = File::create(&path)?;
        held.try_lock()?;
        let at = at.map(|at| at.to_string()).unwrap_or_default();
        let group = group.map(|group| group.to_string()).unwrap_or_default();
        write!(held, "{at}\n{group}")?;
        Ok(Self { path, _held: held })
    }
}

impl Drop for Waiting {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// A record taken: the login's loopback address, where it was found listening, and its process
/// group, where it was recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Taken {
    pub at: Option<IpAddr>,
    pub group: Option<u32>,
}

/// **Whether a login is waiting with `state`, which takes its record**: `None` with no record, one
/// already taken, or one its holder left behind.
pub fn take(dir: &Path, state: &str) -> Option<Taken> {
    let path = record_of(dir, state);
    let mut record = File::open(&path).ok()?;
    let held = matches!(record.try_lock(), Err(std::fs::TryLockError::WouldBlock));
    let text = io::read_to_string(&mut record).unwrap_or_default();
    let (at, group) = text.split_once('\n').unwrap_or((&text, ""));
    let at = at.parse::<IpAddr>().ok().filter(IpAddr::is_loopback);
    let group = group.parse().ok();
    // Removed whoever held it: of two asking, one removes it.
    (std::fs::remove_file(&path).is_ok() && held).then_some(Taken { at, group })
}

/// `dir`, made private.
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

        assert!(take(dir.path(), "another-sign-ins-state").is_none());
        assert_eq!(
            take(dir.path(), STATE),
            Some(Taken {
                at: None,
                group: None
            })
        );
        assert!(take(dir.path(), STATE).is_none());
    }

    #[test]
    fn a_record_says_its_logins_process_group() {
        let dir = tempfile::tempdir().unwrap();
        let _waiting = Waiting::record_of_group(dir.path(), STATE, None, Some(77)).unwrap();
        assert_eq!(
            take(dir.path(), STATE),
            Some(Taken {
                at: None,
                group: Some(77)
            })
        );
    }

    #[test]
    fn a_login_that_ended_is_not_waiting() {
        let dir = tempfile::tempdir().unwrap();
        drop(Waiting::record(dir.path(), STATE).unwrap());

        assert!(take(dir.path(), STATE).is_none());
    }

    #[test]
    fn a_record_says_where_its_login_was_found_listening_on_the_loopback_alone() {
        let dir = tempfile::tempdir().unwrap();
        let at = IpAddr::from(std::net::Ipv6Addr::LOCALHOST);
        let _waiting = Waiting::record_at(dir.path(), STATE, Some(at)).unwrap();
        assert_eq!(
            take(dir.path(), STATE),
            Some(Taken {
                at: Some(at),
                group: None
            })
        );

        let other = IpAddr::from([192, 0, 2, 1]);
        let _waiting = Waiting::record_at(dir.path(), STATE, Some(other)).unwrap();
        assert_eq!(
            take(dir.path(), STATE),
            Some(Taken {
                at: None,
                group: None
            })
        );
    }

    /// A holder that was killed left its record, and no lock on it.
    #[test]
    fn a_record_its_holder_left_behind_counts_for_nothing_and_goes() {
        let dir = tempfile::tempdir().unwrap();
        File::create(record_of(dir.path(), STATE)).unwrap();

        assert!(take(dir.path(), STATE).is_none());
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
