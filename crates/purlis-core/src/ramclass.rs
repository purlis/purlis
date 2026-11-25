// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A device's RAM class and its hot-chat target** (ADR 0082 §3, SC-4f #7635).
//!
//! The class is the largest of 8, 16, 32 and 64 GB that is not more than the device's physical
//! memory (a 12 GB machine is in the 8 GB class); a machine under 8 GB has no class and targets
//! one hot chat. The target is §3's formula, at most half of the class's memory for purlis and
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

/// A device's RAM class: what it reaches, and so how many hot chats it targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RamClass(Option<u64>);

impl RamClass {
    /// The class of a device with `bytes` of physical memory.
    pub fn of(bytes: u64) -> Self {
        let gigabytes = bytes / GIB;
        Self(CLASSES.into_iter().rfind(|class| *class <= gigabytes))
    }

    /// The class of a device whose memory was read as `total`; an unread memory is no class,
    /// which targets one chat, the way a machine under 8 GB does.
    pub fn of_total(total: Option<u64>) -> Self {
        total.map_or(Self(None), Self::of)
    }

    /// This machine's class, read from the operating system.
    pub fn read() -> Self {
        Self::of_total(crate::memorypressure::total())
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
