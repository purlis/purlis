// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The start's one read of the key** (T1a-61; R-078, R-510, R-592). Skeleton: every answer is
//! wrong until the implementation lands.

use std::path::Path;

use ulid::Ulid;

use super::mark::{Binding, ModelKeyError};
use crate::eventlog::stops::{AskGroup, AskRaised, Stop};
use crate::seam::{Credentials, Files};
use crate::secrets::keyring::Secret;

/// What a start's one read found.
#[derive(Debug)]
pub enum Read {
    Key(Secret, Binding),
    Locked(Locked),
    Refused(ModelKeyError),
}

/// A start the keychain's lock stopped.
#[derive(Debug)]
pub struct Locked {
    entry: Ulid,
}

impl Locked {
    pub fn entry(&self) -> Ulid {
        self.entry
    }

    pub fn says(&self) -> &'static str {
        ""
    }

    pub fn keyed_by(&self) -> &'static str {
        ""
    }

    pub fn raised(&self) -> Stop {
        Stop::AskRaised(AskRaised {
            ask: AskGroup::Consent,
        })
    }

    pub fn unlock(
        self,
        _unlocks: &dyn Unlocks,
        _config_root: &Path,
        _files: &dyn Files,
        _credentials: &dyn Credentials,
    ) -> Unlocking {
        Unlocking::Cancelled(self)
    }
}

/// What the OS's unlock did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unlock {
    Unlocked,
    Cancelled,
    Failed,
}

/// The OS's unlock.
pub trait Unlocks {
    fn unlock(&self) -> Unlock;
}

/// What a start did after *Unlock*.
#[derive(Debug)]
pub enum Unlocking {
    Read(Secret, Binding),
    Cancelled(Locked),
    Failed(Locked),
    Refused(ModelKeyError),
}

/// The start's one read.
pub fn read_once(
    _config_root: &Path,
    _files: &dyn Files,
    _credentials: &dyn Credentials,
    entry: Ulid,
) -> Read {
    Read::Locked(Locked { entry })
}
