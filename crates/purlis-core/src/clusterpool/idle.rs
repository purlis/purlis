// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How a cluster runner tells its operator it is idle** (#3980). The operator has no `exec` and
//! reads only a pod's status, so the runner's host ends itself with a known termination message,
//! which the cluster shows in that status. Its container is then started again in place until
//! the operator suspends its sandbox, so a host that finds [`MARK`] does not serve for a while:
//! nothing starts on a runner about to be suspended. An operator that never comes leaves a runner
//! that serves again after [`PARKED_FOR`], and that the operator no longer takes as idle.

use std::path::Path;
use std::time::{Duration, SystemTime};

/// The environment variable the operator gives a runner its pool's `idleAfter` in. An image from
/// before #3980 does not read it, and is never idle, which is the safe side.
pub const AFTER_VAR: &str = "PURLIS_IDLE_AFTER";

/// The termination message that says it. `operator/src/lifecycle.rs` reads the same words.
pub const MESSAGE: &str = "purlis-runner-idle";

/// Where the cluster takes a container's termination message from.
pub const TERMINATION_LOG: &str = "/dev/termination-log";

/// What says the last start ended idle: in the pod's memory folder, so it survives a restart of
/// the container and goes with the pod.
pub const MARK: &str = "/run/purlis-link/idle-said";

/// How long a host that finds [`MARK`] waits before it serves again. The operator looks at a
/// runner every minute, and takes its word for four (`operator/src/lifecycle.rs`).
pub const PARKED_FOR: Duration = Duration::from_secs(5 * 60);

/// `spelled`, as the operator gives it (a whole number of `s`, `m` or `h`), as a duration. Anything
/// else is no time: such a runner is never idle.
pub fn after(spelled: &str) -> Option<Duration> {
    let split = spelled.len().checked_sub(1)?;
    if !spelled.is_char_boundary(split) {
        return None;
    }
    let (count, unit) = spelled.split_at(split);
    let unit = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        _ => return None,
    };
    let count: u64 = count.parse().ok()?;
    let seconds = count.checked_mul(unit).filter(|seconds| *seconds > 0)?;
    Some(Duration::from_secs(seconds))
}

/// Says the runner is idle: [`MESSAGE`] for the cluster to show, and `mark` for the next start.
pub fn say(log: &Path, mark: &Path) -> std::io::Result<()> {
    std::fs::write(mark, MESSAGE)?;
    std::fs::write(log, MESSAGE)
}

/// How much longer a host started now waits before it serves, where `mark` says the last start
/// ended idle; none where it does not, or the wait has passed.
pub fn parked_for(mark: &Path, now: SystemTime) -> Option<Duration> {
    let said = std::fs::metadata(mark)
        .and_then(|held| held.modified())
        .ok()?;
    let waited = now.duration_since(said).unwrap_or_default();
    PARKED_FOR
        .checked_sub(waited)
        .filter(|left| !left.is_zero())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_reads_as_the_operator_spells_it_and_anything_else_is_none() {
        assert_eq!(after("30m"), Some(Duration::from_secs(1800)));
        assert_eq!(after("90s"), Some(Duration::from_secs(90)));
        assert_eq!(after("2h"), Some(Duration::from_secs(7200)));
        for not in [
            "",
            "m",
            "0m",
            "1d",
            "-1m",
            "1.5h",
            "é",
            "99999999999999999999h",
        ] {
            assert_eq!(after(not), None, "{not}");
        }
    }

    #[test]
    fn a_host_waits_only_while_the_mark_is_recent() {
        let folder = tempfile::tempdir().unwrap();
        let (log, mark) = (folder.path().join("log"), folder.path().join("mark"));
        let now = SystemTime::now();
        assert_eq!(parked_for(&mark, now), None);
        say(&log, &mark).unwrap();
        assert_eq!(std::fs::read_to_string(&log).unwrap(), MESSAGE);
        assert!(parked_for(&mark, SystemTime::now()).is_some());
        assert_eq!(
            parked_for(&mark, now + PARKED_FOR + Duration::from_secs(5)),
            None
        );
    }

    #[test]
    fn the_operator_reads_the_same_words() {
        assert_eq!(MESSAGE, "purlis-runner-idle");
    }
}
