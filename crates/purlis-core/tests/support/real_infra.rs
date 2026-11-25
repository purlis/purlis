// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A real-infrastructure test whose fixture is missing** (#7188, ruling 4): in CI it fails, on
//! a laptop it skips with a line nobody can miss.
//!
//! A test that needs a rig (an sshd, a dind, a systemd machine) finds it in the environment its
//! `up.sh` set. Without the rig, returning early and passing is a green run that ran nothing, and
//! a rig that went missing in CI looked exactly like a rig that was never there. The rigs' jobs
//! set `PURLIS_PARITY`, and with it a missing fixture panics; without it the test prints
//! `SKIP real-infra: ...` to stderr and returns `None`.

use std::fmt::Write as _;

/// The variable the rigs' jobs set (`.github/workflows/ci.yml`, `parity` and `remote-parity`).
pub const IN_CI: &str = "PURLIS_PARITY";

/// What a missing fixture comes to.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict<T> {
    /// The fixture is there: run the test.
    Run(T),
    /// A laptop without the rig: skip, saying so with this line.
    Skip(String),
    /// A job that sets up the rig, and the rig is not there: fail with this message.
    Fail(String),
}

/// The line a skipped test prints. It starts `SKIP real-infra` so one grep finds every one.
pub fn skip_line(what: &str, set_by: &str) -> String {
    let mut line = String::new();
    let _ = write!(
        line,
        "SKIP real-infra: {what} was not run, no fixture is set (start it with {set_by}; \
         {IN_CI}=1 makes this a failure)"
    );
    line
}

/// The verdict, given whether this is a rig's job. Pure, so it is tested without the environment.
pub fn judge<T>(in_ci: bool, what: &str, set_by: &str, found: Option<T>) -> Verdict<T> {
    match found {
        Some(fixture) => Verdict::Run(fixture),
        None if in_ci => Verdict::Fail(format!(
            "{what}: the fixture is missing and {IN_CI} is set, so this test fails instead of \
             passing unrun (start it with {set_by})"
        )),
        None => Verdict::Skip(skip_line(what, set_by)),
    }
}

/// **The one gate every real-infrastructure test passes through**: the fixture it found, or
/// `None` after a loud SKIP line, or a panic where `PURLIS_PARITY` is set.
pub fn fixture_or_skip<T>(what: &str, set_by: &str, found: Option<T>) -> Option<T> {
    match judge(
        std::env::var_os(IN_CI).is_some_and(|v| !v.is_empty() && v != "0"),
        what,
        set_by,
        found,
    ) {
        Verdict::Run(fixture) => Some(fixture),
        Verdict::Skip(line) => {
            let _ =
                std::io::Write::write_all(&mut std::io::stderr(), format!("{line}\n").as_bytes());
            None
        }
        Verdict::Fail(why) => panic!("{why}"),
    }
}
