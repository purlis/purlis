// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The sandbox level and the escape denies** (M75 slice 11, #7020; spec §3.5).
//!
//! `[policy] sandbox = off|floor|full` is the `sandbox` family's one switch: it sets
//! `context.sandbox` for a chat ([`super::levels::Loaded::sandbox_level_for`]), and what the hooks
//! do with the `read` and `write` forbids follows from it
//! ([`crate::enforcement::mode_of_level_under`]).
//!
//! **The escape denies belong to the level, not to a family's mode.** A program a chat starts
//! through launchd, Launch Services, cron or `at` runs outside every wrap, and so does a call to
//! the session bus or a mach service that starts one. They are a step of their own, applied
//! whenever the level is `floor` or `full`: [`in_force`] takes the level and nothing else, so no
//! family's mode (`processes = "off"`, the `watch` template's `log`) lifts one, and `watch` never
//! logs them. A rule that names one is refused at load ([`named_by`]).
//!
//! With the level `off` there is no wrap, so `network`, `ports` and `processes`, which a wrap
//! enforces, are not in force ([`not_in_force`]).

use crate::enforcement::SandboxLevel;

/// The programs the floor never lets a chat start: each hands a job to launchd, cron or `at`, or
/// sends an Apple event, so what it starts is outside the wrap.
pub const PROGRAMS: [&str; 6] = ["launchctl", "osascript", "open", "crontab", "at", "batch"];

/// The mach services the floor never lets a chat look up: Launch Services and Apple events.
pub const MACH_SERVICES: [&str; 2] = [
    "com.apple.coreservices.launchservicesd",
    "com.apple.coreservices.appleevents",
];

/// The session bus, as a `Socket` a rule could name.
pub const SESSION_BUS: [&str; 2] = ["session-bus", "dbus-session"];

/// The families a wrap enforces, which are not in force where there is none.
pub const NEEDS_A_WRAP: [&str; 3] = ["network", "ports", "processes"];

/// **Whether the escape denies apply at `level`**: `floor` and `full`, whatever any family's mode
/// says.
pub fn in_force(level: SandboxLevel) -> bool {
    level >= SandboxLevel::Floor
}

/// **The families that are not in force at `level`**: `network`, `ports` and `processes` where
/// there is no wrap (Q308a), none otherwise.
pub fn not_in_force(level: SandboxLevel) -> &'static [&'static str] {
    if level == SandboxLevel::Off {
        &NEEDS_A_WRAP
    } else {
        &[]
    }
}

/// **The escape deny a rule's `action` on a `kind` of resource named `id` would name**, if it
/// names one: `exec` on a `Command` that is one of [`PROGRAMS`] (by its name, or the last part of
/// its path), `dial` on a `Socket` that is the session bus or one of [`MACH_SERVICES`].
pub fn named_by(action: &str, kind: &str, id: &str) -> Option<&'static str> {
    match (action, kind) {
        ("exec", "Command") => {
            let name = id.rsplit('/').next().unwrap_or(id);
            PROGRAMS.into_iter().find(|program| *program == name)
        }
        ("dial", "Socket") => SESSION_BUS
            .into_iter()
            .chain(MACH_SERVICES)
            .find(|socket| *socket == id),
        _ => None,
    }
}
