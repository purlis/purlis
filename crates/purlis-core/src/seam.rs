// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The file-system and vault seam** (R-697, design §5.4): every operation the key proxy's
//! request path and a start's key read could make, behind one of three traits, so a test can
//! count them (KP4: one `stat` per policy file, no file read, no subprocess and no
//! credential-store call per request).
//!
//! - [`Files`]: a `stat` and a read of a path (the policy files, your keys' index, the
//!   profiles store).
//! - [`Spawner`]: starting a subprocess. None is allowed per request; the trait makes that
//!   countable.
//! - [`Credentials`]: a decorator over [`Store`], so every keychain read and write, and every
//!   vault resolve made through it, passes the seam.
//!
//! **The real implementations** ([`RealFiles`], [`RealSpawner`], [`RealCredentials`]) are a
//! direct call each: no allocation, lock or branch beyond the call itself, so KP4's time row
//! is measured on them. (A start takes the fork lock, as every start in purlis does; no
//! request starts one.)
//!
//! **The recording implementations** (`RecordingFiles`, `RecordingSpawner`,
//! `RecordingCredentials`) share one `Record`, which keeps each operation's kind and target —
//! a path, a program, or a store's service and entry id — and never a value. They exist only
//! with the `test-support` feature, which only `[dev-dependencies]` turn on, and an
//! implementation is always a constructor argument: nothing a release build reads (an
//! environment variable, a file, a setting) picks one
//! (`the_recording_seam_is_absent_from_a_release_build`).

use std::fs::Metadata;
use std::io;
use std::path::Path;
use std::process::{Child, Command};

use crate::secrets::VaultError;
use crate::secrets::keyring::{Held, Presence, Secret, Store};

/// A `stat` and a read of a path.
pub trait Files: Send + Sync {
    /// The path's metadata, following a link.
    fn stat(&self, path: &Path) -> io::Result<Metadata>;
    /// The whole file.
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
}

/// The file system itself.
#[derive(Debug, Clone, Copy, Default)]
pub struct RealFiles;

impl Files for RealFiles {
    fn stat(&self, path: &Path) -> io::Result<Metadata> {
        std::fs::metadata(path)
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }
}

/// Starting a subprocess.
pub trait Spawner: Send + Sync {
    /// `command`, started.
    fn spawn(&self, command: &mut Command) -> io::Result<Child>;
}

/// The operating system's own start, through [`crate::forklock::spawn`], the one gate every
/// program purlis starts goes through. Its lock is never on the request path: no request
/// starts a subprocess.
#[derive(Debug, Clone, Copy, Default)]
pub struct RealSpawner;

impl Spawner for RealSpawner {
    fn spawn(&self, command: &mut Command) -> io::Result<Child> {
        crate::forklock::spawn(command)
    }
}

/// A credential store as the seam sees it: a [`Store`] wrapped by one of the seam's decorators,
/// so a constructor that takes a `Credentials` is never handed a bare store whose calls go
/// uncounted.
pub trait Credentials: Store {}

/// The store it wraps, called directly.
pub struct RealCredentials(Box<dyn Store>);

impl RealCredentials {
    pub fn new(store: Box<dyn Store>) -> Self {
        Self(store)
    }
}

impl std::fmt::Debug for RealCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RealCredentials")
    }
}

impl Credentials for RealCredentials {}

impl Store for RealCredentials {
    fn get(&self, service: &str, account: &str) -> Result<Option<Secret>, VaultError> {
        self.0.get(service, account)
    }
    fn set(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        self.0.set(service, account, value)
    }
    fn delete(&self, service: &str, account: &str) -> Result<bool, VaultError> {
        self.0.delete(service, account)
    }
    fn exists(&self, service: &str, account: &str) -> Presence {
        self.0.exists(service, account)
    }
    fn holds(&self) -> bool {
        self.0.holds()
    }
    fn rehold(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        self.0.rehold(service, account, value)
    }
    fn make_fresh(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        self.0.make_fresh(service, account, value)
    }
    fn make_own(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        self.0.make_own(service, account, value)
    }
    fn forget(&self, service: &str, account: &str) {
        self.0.forget(service, account)
    }
}

#[cfg(any(test, feature = "test-support"))]
pub use recording::{
    Kind, Op, Record, RecordingCredentials, RecordingFiles, RecordingSpawner, Target,
};

#[cfg(any(test, feature = "test-support"))]
mod recording {
    use std::ffi::OsString;
    use std::fs::Metadata;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command};
    use std::sync::{Arc, Mutex};

    use super::{Credentials, Files, RealFiles, RealSpawner, Spawner};
    use crate::secrets::VaultError;
    use crate::secrets::keyring::{Held, Presence, Secret, Store};

    /// The kind of one operation, each counted apart.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Kind {
        /// A path's metadata.
        Stat,
        /// A file read whole.
        Read,
        /// A subprocess started.
        Spawn,
        /// A credential store asked for an entry, or whether it has one.
        CredentialRead,
        /// An entry written through the seam: what the window's command does.
        CredentialWrite,
        /// An entry deleted, or forgotten by this process.
        CredentialDelete,
        /// An entry written past the seam, as another program would
        /// ([`RecordingCredentials::write_outside`]).
        OutsideWrite,
    }

    /// Where an operation went: never what it read or wrote.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Target {
        Path(PathBuf),
        Program(OsString),
        /// A store's entry, by its service and account.
        Entry {
            service: String,
            account: String,
        },
    }

    /// One operation the seam saw.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Op {
        pub kind: Kind,
        pub target: Target,
    }

    /// What the recording implementations saw, in order; shared by every one built on it.
    #[derive(Debug, Clone, Default)]
    pub struct Record(Arc<Mutex<Vec<Op>>>);

    impl Record {
        fn push(&self, kind: Kind, target: Target) {
            self.0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(Op { kind, target });
        }

        fn entry(&self, kind: Kind, service: &str, account: &str) {
            self.push(
                kind,
                Target::Entry {
                    service: service.to_owned(),
                    account: account.to_owned(),
                },
            );
        }

        /// Every operation so far, in order.
        pub fn ops(&self) -> Vec<Op> {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
        }

        /// How many operations of `kind` so far.
        pub fn count(&self, kind: Kind) -> usize {
            self.0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .filter(|op| op.kind == kind)
                .count()
        }

        /// Forget what was seen, so a test counts from here.
        pub fn clear(&self) {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).clear();
        }
    }

    /// The file system, each `stat` and read recorded.
    #[derive(Debug, Clone)]
    pub struct RecordingFiles(Record);

    impl RecordingFiles {
        pub fn new(record: Record) -> Self {
            Self(record)
        }
    }

    impl Files for RecordingFiles {
        fn stat(&self, path: &Path) -> io::Result<Metadata> {
            self.0.push(Kind::Stat, Target::Path(path.to_path_buf()));
            RealFiles.stat(path)
        }

        fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
            self.0.push(Kind::Read, Target::Path(path.to_path_buf()));
            RealFiles.read(path)
        }
    }

    /// The operating system's start, each subprocess recorded by its program.
    #[derive(Debug, Clone)]
    pub struct RecordingSpawner(Record);

    impl RecordingSpawner {
        pub fn new(record: Record) -> Self {
            Self(record)
        }
    }

    impl Spawner for RecordingSpawner {
        fn spawn(&self, command: &mut Command) -> io::Result<Child> {
            self.0.push(
                Kind::Spawn,
                Target::Program(command.get_program().to_owned()),
            );
            RealSpawner.spawn(command)
        }
    }

    /// A store, each call recorded by its entry; and a write past the seam, as another
    /// program makes one (X7).
    pub struct RecordingCredentials {
        store: Box<dyn Store>,
        record: Record,
    }

    impl std::fmt::Debug for RecordingCredentials {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("RecordingCredentials")
        }
    }

    impl RecordingCredentials {
        pub fn new(store: Box<dyn Store>, record: Record) -> Self {
            Self { store, record }
        }

        /// Writes `value` into the entry straight into the wrapped store, never through the
        /// window's write command: what another program writing the keychain item looks like,
        /// on every OS against the fenced stub (D6640-T3 X7). Recorded as
        /// [`Kind::OutsideWrite`], never as a [`Kind::CredentialWrite`].
        pub fn write_outside(
            &self,
            service: &str,
            account: &str,
            value: &str,
        ) -> Result<Held, VaultError> {
            self.record.entry(Kind::OutsideWrite, service, account);
            self.store.set(service, account, value)
        }
    }

    impl Credentials for RecordingCredentials {}

    impl Store for RecordingCredentials {
        fn get(&self, service: &str, account: &str) -> Result<Option<Secret>, VaultError> {
            self.record.entry(Kind::CredentialRead, service, account);
            self.store.get(service, account)
        }
        fn set(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
            self.record.entry(Kind::CredentialWrite, service, account);
            self.store.set(service, account, value)
        }
        fn delete(&self, service: &str, account: &str) -> Result<bool, VaultError> {
            self.record.entry(Kind::CredentialDelete, service, account);
            self.store.delete(service, account)
        }
        fn exists(&self, service: &str, account: &str) -> Presence {
            self.record.entry(Kind::CredentialRead, service, account);
            self.store.exists(service, account)
        }
        fn holds(&self) -> bool {
            self.store.holds()
        }
        fn rehold(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
            self.record.entry(Kind::CredentialWrite, service, account);
            self.store.rehold(service, account, value)
        }
        fn make_fresh(
            &self,
            service: &str,
            account: &str,
            value: &str,
        ) -> Result<Held, VaultError> {
            self.record.entry(Kind::CredentialWrite, service, account);
            self.store.make_fresh(service, account, value)
        }
        fn make_own(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
            self.record.entry(Kind::CredentialWrite, service, account);
            self.store.make_own(service, account, value)
        }
        fn forget(&self, service: &str, account: &str) {
            self.record.entry(Kind::CredentialDelete, service, account);
            self.store.forget(service, account)
        }
    }
}
