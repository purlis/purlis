// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Whether a process group is still running once its leader has gone, and the wait for it to
//! go: what ending a terminal's program and everything it started asks (`session`), and what
//! the core's own process groups ask too (`purlis_core::worktree`).

/// Whether the group `leader` leads still has a process running in it, once `leader` itself
/// has exited and is waiting to be reaped. `None` where that cannot be read.
///
/// Linux counts the exited leader as a member when the group is signalled, so the null signal
/// cannot tell: each process's own record is read instead (`/proc/<pid>/stat`: after the
/// name come the state, the parent and the group).
#[cfg(target_os = "linux")]
pub fn group_still_runs(leader: rustix::process::Pid) -> Option<bool> {
    group_runs_as_listed(std::path::Path::new("/proc"), leader.as_raw_nonzero().get())
}

/// [`group_still_runs`], reading the processes listed under `proc`.
#[cfg(any(target_os = "linux", test))]
fn group_runs_as_listed(proc: &std::path::Path, group: i32) -> Option<bool> {
    let listed = std::fs::read_dir(proc).ok()?;
    let group = group.to_string();
    Some(listed.flatten().any(|entry| {
        let name = entry.file_name();
        if !name
            .to_str()
            .is_some_and(|name| name.bytes().all(|b| b.is_ascii_digit()))
        {
            return false;
        }
        // A process that ended between the listing and the read is not running.
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            return false;
        };
        in_group_and_running(&stat, &group)
    }))
}

/// Whether one `/proc/<pid>/stat` line is a process of `group` that has not exited. The name,
/// in parentheses, may itself hold spaces and parentheses, so the fields are read after the
/// last `)`: state, parent, group.
#[cfg(any(target_os = "linux", test))]
fn in_group_and_running(stat: &str, group: &str) -> bool {
    let Some((_, rest)) = stat.rsplit_once(')') else {
        return false;
    };
    let mut fields = rest.split_ascii_whitespace();
    let (state, _parent, its) = (fields.next(), fields.next(), fields.next());
    its == Some(group) && !matches!(state, Some("Z" | "X") | None)
}

/// On the BSDs and macOS the null signal passes over a process that has exited (measured on
/// macOS 26.2: a group holding only its unreaped leader answers `EPERM`, one with a running
/// member answers yes), so it is the question itself, and always has an answer.
#[cfg(all(unix, not(target_os = "linux")))]
pub fn group_still_runs(leader: rustix::process::Pid) -> Option<bool> {
    Some(rustix::process::test_kill_process_group(leader).is_ok())
}

/// How long a group that cannot be read is given, in all, once its leader has gone.
///
/// Not the whole grace: with nothing to say whether anyone is left, waiting it out would be
/// two seconds on every stop for a group that is nearly always empty. A tenth of a second is
/// many times what a process needs to act on a signal, and what is left after it is killed.
#[cfg(unix)]
pub const UNSEEN_GRACE: std::time::Duration = std::time::Duration::from_millis(100);

/// Waits until `done` says so or `until` has come, and says whether it was `done`.
///
/// It looks at once, then again after ten milliseconds, twenty, forty, and so on up to a
/// hundred and sixty: what is waited for is nearly always there within the first look or two,
/// and each look at a process group on Linux reads every process's record.
pub fn wait_until(until: std::time::Instant, mut done: impl FnMut() -> bool) -> bool {
    let mut pause = std::time::Duration::from_millis(10);
    loop {
        if done() {
            return true;
        }
        let left = until.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return false;
        }
        std::thread::sleep(pause.min(left));
        pause = (pause * 2).min(std::time::Duration::from_millis(160));
    }
}

/// Waits until nothing of the group `leader` led is still running, or `until`. `leader` has
/// exited and is not reaped. Where the group cannot be read, [`UNSEEN_GRACE`] in all.
#[cfg(unix)]
pub fn wait_for_group_to_go(leader: rustix::process::Pid, until: std::time::Instant) {
    let began = std::time::Instant::now();
    wait_until(until, || match group_still_runs(leader) {
        Some(runs) => !runs,
        None => began.elapsed() >= UNSEEN_GRACE,
    });
}

#[cfg(test)]
mod tests;
