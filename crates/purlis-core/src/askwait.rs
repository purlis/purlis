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
    waiting_for(&answered_in(root), within)
}

/// [`waiting_said`], for a command that runs in no project (#6000): the window lists what it
/// waits on under the project it has in front.
pub fn waiting_said_in_the_window(within: Duration) -> String {
    waiting_for("Asks (\u{270b}) in the purlis window", within)
}

fn waiting_for(place: &str, within: Duration) -> String {
    format!(
        "waiting for the person: {place}, for up to {}",
        how_long(within)
    )
}

/// **A wait in words**, as every waiting command says it: "90 seconds", "2 minutes", and to a
/// tenth of a second where it is not whole ("7.5 seconds").
pub(crate) fn how_long(wait: Duration) -> String {
    let secs = wait.as_secs();
    if wait.subsec_millis() != 0 {
        format!("{:.1} seconds", wait.as_secs_f64())
    } else if secs >= 120 && secs.is_multiple_of(60) {
        format!("{} minutes", secs / 60)
    } else {
        format!("{secs} seconds")
    }
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
            waiting_said_in_the_window(Duration::from_secs(120)),
            "waiting for the person: Asks (\u{270b}) in the purlis window, for up to 2 minutes"
        );
        assert_eq!(
            still_answerable_said(root),
            "the ask is still answerable in Asks (\u{270b}) in the purlis window, project \
             smart-ide"
        );
    }

    #[test]
    fn a_wait_is_said_in_seconds_and_in_minutes_from_two_minutes_on() {
        let said = |ms: u64| how_long(Duration::from_millis(ms));
        assert_eq!(said(60_000), "60 seconds");
        assert_eq!(said(90_000), "90 seconds");
        assert_eq!(said(120_000), "2 minutes");
        assert_eq!(said(180_000), "3 minutes");
        assert_eq!(said(150_000), "150 seconds");
        assert_eq!(said(7_500), "7.5 seconds");
        assert_eq!(said(900), "0.9 seconds");
    }

    #[test]
    fn a_project_at_the_root_of_the_disk_is_named_whole() {
        assert_eq!(
            answered_in(Path::new("/")),
            "Asks (\u{270b}) in the purlis window, project /"
        );
    }
}
