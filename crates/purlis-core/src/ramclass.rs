// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A device's RAM class and its hot-chat target** (ADR 0082 §3, SC-4f #7635).
//!
//! The class is the largest of 8, 16, 32 and 64 GB that the device's installed physical memory
//! reaches (a 12 GB machine is in the 8 GB class); a machine under 8 GB has no class and targets
//! one hot chat. **An operating system reports a little less than is installed** (Linux's
//! `MemTotal` leaves out what the kernel keeps), so a reported total of at least 94% of a class
//! is that class: 15.5 GB is a 16 GB machine and 7.6 GB an 8 GB one, while 14 GB is still 8. The target is §3's formula, at most half of the class's memory for purlis and
//! its harnesses, capped at the top class's 50:
//!
//! ```text
//! hot target = max(1, floor((class / 2 - base) / (harness + scrollback)))
//! ```
//!
//! It is a budget, never a cap: nothing here refuses, queues or hibernates a chat. It reads the
//! memory ([`crate::memorypressure::total`]) and stores nothing (ADR 0082: the class is read from
//! the operating system and not stored). GB are binary (2^30 bytes), as an operating system
//! reports a machine's memory.

/// The classes, largest last, in GB.
const CLASSES: [u64; 4] = [8, 16, 32, 64];

/// The most hot chats any class targets: the top class's number (ADR 0082 §3, D-0082a).
const CEILING: u32 = 50;

/// purlis's own base, in tenths of a MB: 2 GB.
const BASE: u64 = 2 * 1024 * 10;

/// A hot chat's cost, in tenths of a MB: a harness's 385 MB and 20.2 MB of scrollback.
const PER_CHAT: u64 = (385 * 10) + 202;

const GIB: u64 = 1024 * 1024 * 1024;

/// The share of a class, in percent, a reported total must reach to be that class.
const REACHES: u128 = 94;

/// A device's RAM class: what it reaches, and so how many hot chats it targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RamClass(Option<u64>);

impl RamClass {
    /// The class of a device whose operating system reports `bytes` of physical memory: the
    /// largest class it reaches at least 94% of.
    pub fn of(bytes: u64) -> Self {
        let reaches = |class: &u64| u128::from(bytes) * 100 >= u128::from(class * GIB) * REACHES;
        Self(CLASSES.into_iter().rfind(reaches))
    }

    /// This machine's class, read from the operating system; `None` where its memory could not
    /// be read, which is no class at all rather than one under 8 GB.
    pub fn read() -> Option<Self> {
        crate::memorypressure::total().map(Self::of)
    }

    /// A class named in GB, as a test names one (`None`: under 8 GB), rounded down to the class
    /// that GB reaches.
    pub fn of_gigabytes(gigabytes: Option<u64>) -> Self {
        gigabytes.map_or(Self(None), |gigabytes| Self::of(gigabytes * GIB))
    }

    /// The class's memory in GB: 8, 16, 32 or 64; `None` under 8 GB.
    pub fn gigabytes(self) -> Option<u64> {
        self.0
    }

    /// How many hot chats this device targets.
    pub fn hot_target(self) -> u32 {
        let Some(class) = self.0 else { return 1 };
        let half = class * 1024 * 10 / 2;
        let fits = half.saturating_sub(BASE) / PER_CHAT;
        u32::try_from(fits).map_or(CEILING, |fits| fits.clamp(1, CEILING))
    }
}

#[cfg(test)]
mod tests;
