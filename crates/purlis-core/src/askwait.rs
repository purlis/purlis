// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A call that waits on the person, and notices its asker hang up.** A browser login's notice,
//! a secret fill's and the Secret ask a fill raises all wait here: for an answer, at most as
//! long as they may, looking every [`LOOKS_AGAIN`] whether the call that waits is still there.

use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// **How a wait on the person ended** ([`waited`]).
#[derive(Debug, PartialEq, Eq)]
pub enum Ended<H> {
    /// The answer.
    Heard(H),
    /// What answers it went away, and its wait with it.
    Closed,
    /// Nobody answered in time.
    NotAnswered,
    /// The call's asker hung up.
    AskerGone,
}

/// How often a waiting call looks whether its asker hung up.
pub const LOOKS_AGAIN: Duration = Duration::from_secs(1);

/// **The answer `heard` brings**, waited for `at_most`, looking every [`LOOKS_AGAIN`] whether
/// the call's asker is `gone`: past the time, or with nobody waiting any more, `settle` ends the
/// wait. It answers the answer that came in that very moment, where one did, and that answer
/// wins.
pub fn waited<H>(
    heard: &mpsc::Receiver<H>,
    at_most: Duration,
    gone: &dyn Fn() -> bool,
    settle: &dyn Fn() -> Option<H>,
) -> Ended<H> {
    let until = Instant::now() + at_most;
    loop {
        let left = until.saturating_duration_since(Instant::now());
        match heard.recv_timeout(left.min(LOOKS_AGAIN)) {
            Ok(one) => return Ended::Heard(one),
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ended::Closed,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let hung_up = gone();
                if !hung_up && Instant::now() < until {
                    continue;
                }
                if let Some(one) = settle() {
                    return Ended::Heard(one);
                }
                return if hung_up {
                    Ended::AskerGone
                } else {
                    Ended::NotAnswered
                };
            }
        }
    }
}

/// **Where the person answers what a command of the project at `root` waits on** (#6008, ADR
/// 0121 decision 16): the Asks view, behind the ✋, in the window that has that project open,
/// named as its tab is (the last segment of its folder, `app/src/projectName.ts`).
pub fn answered_in(root: &Path) -> String {
    let name = root.file_name().map_or_else(
        || root.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    format!(
        "Asks (\u{270b}) in the purlis window, project {}",
        crate::shown::short(&name)
    )
}

/// **What a command says as it starts to wait on the person** for at most `within`, in the
/// project at `root` (#6008, ADR 0121 decision 16). Every such command says it in these words.
pub fn waiting_said(root: &Path, within: Duration) -> String {
    let secs = within.as_secs();
    let how_long = if secs >= 120 && secs % 60 == 0 {
        format!("{} minutes", secs / 60)
    } else {
        format!("{secs} seconds")
    };
    format!(
        "waiting for the person: {}, for up to {how_long}",
        answered_in(root)
    )
}

/// **What a command that stopped waiting says of an ask that stays open** (#6008, ADR 0121
/// decision 15): the person may still answer it where it waited.
pub fn still_answerable_said(root: &Path) -> String {
    format!("the ask is still answerable in {}", answered_in(root))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wait_names_the_asks_view_the_project_and_how_long() {
        let root = Path::new("/home/one/smart-ide");
        assert_eq!(
            waiting_said(root, Duration::from_secs(90)),
            "waiting for the person: Asks (\u{270b}) in the purlis window, project smart-ide, \
             for up to 90 seconds"
        );
        assert_eq!(
            waiting_said(root, Duration::from_secs(15 * 60)),
            "waiting for the person: Asks (\u{270b}) in the purlis window, project smart-ide, \
             for up to 15 minutes"
        );
        assert_eq!(
            still_answerable_said(root),
            "the ask is still answerable in Asks (\u{270b}) in the purlis window, project \
             smart-ide"
        );
    }

    #[test]
    fn a_project_at_the_root_of_the_disk_is_named_whole() {
        assert_eq!(
            answered_in(Path::new("/")),
            "Asks (\u{270b}) in the purlis window, project /"
        );
    }
}
