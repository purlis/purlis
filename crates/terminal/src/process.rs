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

/// **What a program started that left its process group** (#5749): each process descended
/// from the program that is in a group other than the program's own. Ending the program's
/// group does not reach these, and Claude Code puts every shell its model runs in a session of
/// its own, so a background shell is one: read while the program still runs, and ended with
/// it, it does not outlive the chat as an orphan nobody sees.
///
/// Read once, before the first signal, because a process whose parent has gone is handed to
/// the system's first process and is nobody's descendant after. Signalled within the end's
/// grace of being read, while the system would have to hand out every other process id before
/// one of these came round again.
#[cfg(unix)]
#[derive(Debug, Default)]
pub struct Strays(Vec<Stray>);

/// One of [`Strays`]: its process, and whether it leads a group of its own, which is then
/// ended with it.
#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stray {
    pid: u32,
    leads: bool,
}

#[cfg(unix)]
impl Strays {
    /// What `leader`, which leads its process group and still runs, has started outside it.
    /// None where the processes cannot be listed.
    pub fn of(leader: u32) -> Self {
        Self(strays_in(&listed(), leader))
    }

    /// Whether there is none.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Hangs up on each, and on the group of each that leads one.
    pub fn hang_up(&self) {
        self.signal(rustix::process::Signal::HUP);
    }

    /// Kills each, and the group of each that leads one.
    pub fn kill(&self) {
        self.signal(rustix::process::Signal::KILL);
    }

    fn signal(&self, signal: rustix::process::Signal) {
        use rustix::process::{Pid, kill_process, kill_process_group};
        for stray in &self.0 {
            let Some(pid) = i32::try_from(stray.pid)
                .ok()
                .filter(|raw| *raw > 1)
                .and_then(Pid::from_raw)
            else {
                continue;
            };
            if stray.leads {
                let _ = kill_process_group(pid, signal);
            }
            let _ = kill_process(pid, signal);
        }
    }
}

/// One process as the system lists it: its id, its parent's and its group's.
#[cfg(any(unix, test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Listed {
    pid: u32,
    parent: u32,
    group: u32,
}

/// The descendants of `leader` in `table` that are not in `leader`'s group, by process id.
#[cfg(any(unix, test))]
fn strays_in(table: &[Listed], leader: u32) -> Vec<Stray> {
    let mut descendants = std::collections::BTreeMap::new();
    let mut parents = vec![leader];
    while let Some(parent) = parents.pop() {
        for child in table
            .iter()
            .filter(|row| row.parent == parent && row.pid != leader)
        {
            if descendants.insert(child.pid, *child).is_none() {
                parents.push(child.pid);
            }
        }
    }
    descendants
        .into_values()
        .filter(|row| row.group != leader)
        .map(|row| Stray {
            pid: row.pid,
            leads: row.group == row.pid,
        })
        .collect()
}

/// Every process running, from each one's own record (`/proc/<pid>/stat`).
#[cfg(target_os = "linux")]
fn listed() -> Vec<Listed> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let pid = entry.file_name().to_str()?.parse().ok()?;
            let stat = std::fs::read_to_string(entry.path().join("stat")).ok()?;
            listed_in_stat(pid, &stat)
        })
        .collect()
}

/// Process `pid` as its `/proc/<pid>/stat` line says, unless it has exited. The fields are read
/// after the name's last `)`: state, parent, group.
#[cfg(any(target_os = "linux", test))]
fn listed_in_stat(pid: u32, stat: &str) -> Option<Listed> {
    let (_, rest) = stat.rsplit_once(')')?;
    let mut fields = rest.split_ascii_whitespace();
    let state = fields.next()?;
    if matches!(state, "Z" | "X") {
        return None;
    }
    Some(Listed {
        pid,
        parent: fields.next()?.parse().ok()?,
        group: fields.next()?.parse().ok()?,
    })
}

/// Every process, as `ps` lists it. macOS has no `/proc`, and its own listing is a call only
/// `unsafe` reaches; `ps` is run once for each program ended, never in a loop.
#[cfg(all(unix, not(target_os = "linux")))]
fn listed() -> Vec<Listed> {
    let mut ps = std::process::Command::new("/bin/ps");
    ps.args(["-A", "-o", "pid=", "-o", "ppid=", "-o", "pgid="]);
    crate::forklock::output(&mut ps)
        .ok()
        .filter(|out| out.status.success())
        .map(|out| listed_by_ps(&String::from_utf8_lossy(&out.stdout)))
        .unwrap_or_default()
}

/// The rows of `ps -o pid= -o ppid= -o pgid=`; a line that is not three numbers is passed over.
#[cfg(any(all(unix, not(target_os = "linux")), test))]
fn listed_by_ps(text: &str) -> Vec<Listed> {
    text.lines()
        .filter_map(|line| {
            let mut numbers = line.split_ascii_whitespace().map(str::parse);
            let (Some(Ok(pid)), Some(Ok(parent)), Some(Ok(group)), None) = (
                numbers.next(),
                numbers.next(),
                numbers.next(),
                numbers.next(),
            ) else {
                return None;
            };
            Some(Listed { pid, parent, group })
        })
        .collect()
}

#[cfg(test)]
mod tests;
