// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Ending a git purlis started, with everything that git started.
//!
//! git does much of its work in children: a filter for each file it checks out, a credential
//! helper, `git-remote-https`, a signer, and for `pull` or `submodule update` another git that
//! writes the working tree. Signalling git alone left each of them running after purlis had
//! said the call was stopped or out of time: a `git merge` under a stopped `git pull` kept
//! writing the tree and holding `index.lock`.
//!
//! # Who leads the group
//!
//! **A group is ended by whoever is answerable for stopping it**, and that differs by process.
//!
//! - **The app's host**, and anything else that does not say otherwise: git leads a process
//!   group of its own ([`lead`]), and [`stop`] signals the group. The host has no terminal
//!   ([`crate::noterminal`]) and nobody ends it by its group, so its own deadline is the only
//!   thing that ever stops a git it started.
//! - **The `purlis` command** says otherwise ([`stay_in_the_callers_group`]): git stays in the
//!   group the command itself runs in, and [`stop`] signals git alone, as it always did. That
//!   group already has an owner who ends it whole: the terminal's Ctrl-C and hang-up reach the
//!   foreground group, a harness tears down a hook's group at the end of a turn, and the app
//!   kills a chat's, an agent's and a brokered command's group. A git moved out of it would
//!   outlive every one of those. Passing the signals on instead was weighed and not done:
//!   a kill cannot be passed on, and a handler for the terminating signals, once installed, is
//!   never the default again for the rest of the process, which the secret commands' own
//!   handling of those signals depends on.
//!
//! **What that leaves:** a `purlis` command's own deadline still ends git alone, so what
//! that git started can outlive it there.
//!
//! # A new group and the terminal
//!
//! A group that is not its terminal's foreground group is stopped by the kernel when it
//! reads the terminal, or changes its settings. The runner's git never does: its standard
//! input is nothing or a pipe, its output is piped, `GIT_TERMINAL_PROMPT=0` closes git's own
//! prompt, no askpass program is passed, and the person's own clone asks ssh never to prompt.
//! A program git starts could still open the terminal for itself, where there is one (a test
//! run from a shell; never the host). It is then stopped rather than left reading the
//! person's keys, and [`stop`] wakes the group as it asks it to end, since a stopped process
//! acts on nothing else.
//!
//! # Windows
//!
//! There is no process group in this sense: a job object is the equivalent, and is not made
//! here. Off unix, [`lead`] changes nothing and [`stop`] ends the child alone.

use std::process::{Child, Command, ExitStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Whether this process keeps the gits it starts in its own group ([`stay_in_the_callers_group`]).
static IN_THE_CALLERS_GROUP: AtomicBool = AtomicBool::new(false);

/// From here on, a git this process starts stays in the process group this process runs in,
/// and is stopped alone.
///
/// For a command somebody else ends by its group (the module's docs say who). Called once, as
/// the command starts, before it runs any git: a git started before the call leads a group
/// that a stop after it would no longer signal.
pub fn stay_in_the_callers_group() {
    IN_THE_CALLERS_GROUP.store(true, Ordering::SeqCst);
}

/// Whether a git this process starts leads a group of its own.
fn leads() -> bool {
    !IN_THE_CALLERS_GROUP.load(Ordering::SeqCst)
}

/// Whether a git this process starts leads a process group of its own, rather than staying
/// in this process's ([`stay_in_the_callers_group`]): for a program to say which it is.
pub fn each_git_leads_its_own_group() -> bool {
    leads()
}

/// Makes `command` the leader of a process group of its own, where this process ends what it
/// starts by its group. Every program [`stop`] is handed is started through this.
pub(crate) fn lead(command: &mut Command) {
    lead_if(command, leads());
}

fn lead_if(command: &mut Command, leads: bool) {
    #[cfg(unix)]
    if leads {
        std::os::unix::process::CommandExt::process_group(command, 0);
    }
    #[cfg(not(unix))]
    let _ = (command, leads);
}

/// How long what was asked to stop has to do it before it is killed.
const STOP_GRACE: Duration = Duration::from_secs(2);

/// Stop the child THIS call spawned, and what it started, by its own id — never by name:
/// other chats on this machine run git too.
///
/// **Asked first, killed only if it will not go** (charter-app#267). A git killed outright
/// cannot clean up: it leaves `index.lock` behind, and every git after it in that repository
/// refuses to start — the `rebase --abort` after a rebase ran out of time failed exactly so,
/// and left the plane mid-rebase. `SIGTERM` is the signal git's own lockfile cleanup is wired
/// to, so a git asked to stop removes its locks and exits; `SIGKILL` follows for whatever has
/// not gone within [`STOP_GRACE`].
///
/// **The whole group, where git leads one** ([`lead`]). The grace is the group's, not the
/// leader's: a git that `pull` started removes its own lock a moment after the `pull` above it
/// has gone, so the wait ends when nothing of the group is left, and the kill that follows
/// reaches only what would not go. The group is looked at less and less often as the wait
/// goes on, and one that cannot be read at all is given a tenth of a second once its leader
/// has gone ([`purlis_terminal::process::UNSEEN_GRACE`]), not the whole grace.
///
/// **Never a group whose leader was reaped.** A group's id is its leader's process id, and
/// the system may hand that id to a stranger once the leader has been waited for and nothing
/// else bears it. So a child that has already ended is waited for and nothing is signalled,
/// the wait for the leader looks without reaping, and the leader is reaped last. An id of 0
/// or 1 is never signalled: those are this process's own group and every process there is.
pub(crate) fn stop(child: &mut Child) {
    let _ = end(child, leads(), STOP_GRACE);
}

/// Who a signal is for.
#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Whom {
    /// Every process of the group this process leads.
    Group(rustix::process::Pid),
    /// This process alone.
    Alone(rustix::process::Pid),
}

/// Sends `signal`, and says whether there was anyone to send it to.
#[cfg(unix)]
fn send(whom: Whom, signal: rustix::process::Signal) -> bool {
    match whom {
        Whom::Group(leader) => rustix::process::kill_process_group(leader, signal).is_ok(),
        Whom::Alone(pid) => rustix::process::kill_process(pid, signal).is_ok(),
    }
}

/// [`stop`], for a child that leads its group or does not, saying how the child ended.
#[cfg(unix)]
fn end(child: &mut Child, leads: bool, grace: Duration) -> Option<ExitStatus> {
    end_sending(child, leads, grace, &mut send)
}

/// [`end`], with every signal it sends going through `send`: how a test counts them.
#[cfg(unix)]
fn end_sending(
    child: &mut Child,
    leads: bool,
    grace: Duration,
    send: &mut dyn FnMut(Whom, rustix::process::Signal) -> bool,
) -> Option<ExitStatus> {
    use rustix::process::{Pid, Signal};
    use std::time::Instant;

    // Ended already: reaped here, and nothing is signalled.
    if let Ok(Some(status)) = child.try_wait() {
        return Some(status);
    }
    let Some(pid) = i32::try_from(child.id())
        .ok()
        .filter(|raw| *raw > 1)
        .and_then(Pid::from_raw)
    else {
        let _ = child.kill();
        return child.wait().ok();
    };
    // A child that was not started as a leader has no group by its id, and the group signal
    // answers that there is none: it is then signalled alone.
    let mut all = |signal: Signal| {
        (leads && send(Whom::Group(pid), signal)) || send(Whom::Alone(pid), signal)
    };
    if all(Signal::TERM) {
        // A stopped process hears nothing until it is continued.
        all(Signal::CONT);
        let until = Instant::now() + grace;
        // The leader first, then what it left: looked for less and less often, since each
        // look at a group on Linux reads every process's record.
        if crate::process::wait_until(until, || exited(child)) && leads {
            crate::process::wait_for_group_to_go(pid, until);
        }
    }
    // The leader is not reaped yet, so the id is still its group's.
    if leads {
        send(Whom::Group(pid), Signal::KILL);
    }
    send(Whom::Alone(pid), Signal::KILL);
    child.wait().ok()
}

#[cfg(not(unix))]
fn end(child: &mut Child, _leads: bool, _grace: Duration) -> Option<ExitStatus> {
    let _ = child.kill();
    child.wait().ok()
}

/// Whether `child` has exited, leaving it unreaped: while it is, its id is not handed out
/// again, so the group signal cannot reach a stranger.
#[cfg(unix)]
fn exited(child: &Child) -> bool {
    use rustix::process::{Pid, WaitId, WaitIdOptions, waitid};
    let options = WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT;
    // An error (the child is gone already) counts as exited, so the loop never spins on it.
    !matches!(
        waitid(WaitId::Pid(Pid::from_child(child)), options),
        Ok(None)
    )
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::path::Path;
    use std::process::Stdio;
    use std::time::Instant;

    /// How long a test waits for something that happens at once on an idle machine.
    const SOON: Duration = Duration::from_secs(10);

    /// `script`, run by `/bin/sh` as the runner starts a git: leading its group or not.
    fn started(script: &str, leads: bool) -> Child {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        lead_if(&mut command, leads);
        crate::forklock::spawn(&mut command).expect("a shell starts")
    }

    fn group_of(child: &Child) -> rustix::process::Pid {
        rustix::process::Pid::from_child(child)
    }

    /// Whether nothing is left of the group `leader` led, waiting for it: what was killed is
    /// there until it is reaped.
    fn group_is_gone(leader: rustix::process::Pid) -> bool {
        let until = Instant::now() + SOON;
        while rustix::process::test_kill_process_group(leader).is_ok() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        rustix::process::test_kill_process_group(leader).is_err()
    }

    fn appears(path: &Path) -> bool {
        let until = Instant::now() + SOON;
        while !path.exists() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        path.exists()
    }

    #[test]
    fn stopping_a_child_ends_what_it_started() {
        let dir = tempfile::tempdir().expect("a directory");
        let said = dir.path().join("started");
        let mut child = started(
            &format!(
                "sleep 30 &\necho $! > '{0}.part'\nmv '{0}.part' '{0}'\nwait",
                said.display()
            ),
            true,
        );
        let _ends = stand_in::Ends::group(child.id());
        let group = group_of(&child);
        let background =
            stand_in::pid_named_in(&said, SOON).expect("the shell said what it started");

        stop(&mut child);

        assert!(group_is_gone(group), "the group outlived its stop");
        assert!(
            stand_in::is_gone(background, SOON),
            "what the child started was left running"
        );
    }

    #[test]
    fn what_is_asked_to_stop_tidies_up_first_and_ends_as_asked() {
        let dir = tempfile::tempdir().expect("a directory");
        let lock = dir.path().join("index.lock");
        let mut child = started(
            &format!(
                "trap 'rm -f \"{0}\"; exit 0' TERM\n: > '{0}'\nwhile :; do sleep 1; done",
                lock.display()
            ),
            true,
        );
        let _ends = stand_in::Ends::group(child.id());
        assert!(appears(&lock), "the shell never took its lock");

        let ended = end(&mut child, true, SOON).expect("it is waited for");

        assert_eq!(ended.code(), Some(0), "it was killed, not asked: {ended:?}");
        assert!(!lock.exists(), "the lock was left behind");
    }

    #[test]
    fn what_the_child_started_has_the_grace_too_after_the_child_itself_has_gone() {
        // `git pull` over `git merge`: the leader ends at once, and the git below it removes
        // its lock a moment later. Killed as the leader went, it would leave the lock.
        let dir = tempfile::tempdir().expect("a directory");
        let lock = dir.path().join("index.lock");
        let below = format!(
            "trap 'sleep 0.5; rm -f \"{0}\"; exit 0' TERM; : > '{0}'; while :; do sleep 1; done",
            lock.display()
        );
        // The script below goes as an argument, so its quotes are its own.
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "/bin/sh -c \"$1\" &\nwait", "sh", &below])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        lead_if(&mut command, true);
        let mut child = crate::forklock::spawn(&mut command).expect("a shell starts");
        let _ends = stand_in::Ends::group(child.id());
        let group = group_of(&child);
        assert!(appears(&lock), "the shell below never took its lock");

        let ended = end(&mut child, true, SOON).expect("it is waited for");

        assert_eq!(
            ended.signal(),
            Some(rustix::process::Signal::TERM.as_raw()),
            "the leader has no handler, so it ends by the signal: {ended:?}"
        );
        assert!(
            !lock.exists(),
            "what was below was killed before it tidied up"
        );
        assert!(group_is_gone(group), "the group outlived its stop");
    }

    #[test]
    fn what_will_not_go_is_killed_when_the_grace_is_over() {
        let dir = tempfile::tempdir().expect("a directory");
        let ready = dir.path().join("ready");
        let mut child = started(
            &stand_in::stubborn_for("TERM INT HUP", &format!(": > '{}'", ready.display()), 30),
            true,
        );
        let _ends = stand_in::Ends::group(child.id());
        let group = group_of(&child);
        assert!(appears(&ready), "the shell never started");

        let began = Instant::now();
        let ended = end(&mut child, true, Duration::from_millis(300)).expect("it is waited for");

        assert_eq!(
            ended.signal(),
            Some(rustix::process::Signal::KILL.as_raw()),
            "{ended:?}"
        );
        assert!(began.elapsed() >= Duration::from_millis(300), "no grace");
        assert!(group_is_gone(group), "the group outlived its stop");
    }

    #[test]
    fn a_group_the_kernel_has_stopped_still_hears_the_ask() {
        // What reading a terminal from a background group does to it. On Linux the ask then
        // waits behind the stop, and without the wake only the kill ends it (macOS wakes a
        // stopped process for a signal that ends it, so there this holds either way).
        let dir = tempfile::tempdir().expect("a directory");
        let ready = dir.path().join("ready");
        let mut child = started(
            &format!(": > '{}'\nkill -STOP $$\nsleep 30", ready.display()),
            true,
        );
        let _ends = stand_in::Ends::group(child.id());
        assert!(appears(&ready), "the shell never started");
        // The stop lands a moment after the marker.
        std::thread::sleep(Duration::from_millis(300));

        let ended = end(&mut child, true, SOON).expect("it is waited for");

        assert_eq!(
            ended.signal(),
            Some(rustix::process::Signal::TERM.as_raw()),
            "{ended:?}"
        );
    }

    #[test]
    fn in_the_callers_group_the_child_is_stopped_alone_and_the_group_is_not_signalled() {
        // As the `purlis` command runs git. The group here is this test's own: a signal to it
        // would end the test.
        let dir = tempfile::tempdir().expect("a directory");
        let said = dir.path().join("started");
        let mut child = started(
            &format!(
                "sleep 5 &\necho $! > '{0}.part'\nmv '{0}.part' '{0}'\nwait",
                said.display()
            ),
            false,
        );
        let background =
            stand_in::pid_named_in(&said, SOON).expect("the shell said what it started");
        assert_ne!(
            rustix::process::getpgid(Some(group_of(&child))).ok(),
            Some(group_of(&child)),
            "it led a group of its own"
        );

        let ended = end(&mut child, false, SOON).expect("it is waited for");

        assert_eq!(
            ended.signal(),
            Some(rustix::process::Signal::TERM.as_raw()),
            "{ended:?}"
        );
        let its = rustix::process::Pid::from_raw(background.cast_signed()).expect("a pid");
        let left = rustix::process::test_kill_process(its).is_ok();
        // The test's own to end: it started it, and holds its id.
        let _ = rustix::process::kill_process(its, rustix::process::Signal::KILL);
        assert!(left, "what the child started was signalled too");
    }

    #[test]
    fn a_child_that_has_already_ended_is_waited_for_and_nothing_is_signalled() {
        // Its id may be a stranger's the moment it is waited for, and a group's with it.
        let mut child = started("exit 3", true);
        let until = Instant::now() + SOON;
        while !exited(&child) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        let mut sent = Vec::new();
        let mut counting = |whom: Whom, signal: rustix::process::Signal| {
            sent.push((whom, signal));
            send(whom, signal)
        };

        let ended = end_sending(&mut child, true, SOON, &mut counting).expect("it is waited for");
        // And asked again, now that it has been waited for.
        let again = end_sending(&mut child, true, SOON, &mut counting).expect("its status is kept");

        assert_eq!((ended.code(), again.code()), (Some(3), Some(3)));
        assert_eq!(sent, [], "a signal was sent for a child that had ended");
    }

    #[test]
    fn a_running_child_is_asked_then_woken_then_killed_as_a_group_and_in_that_order() {
        // The count the test above holds at none is a count of something: every signal a
        // stop sends goes through the same seam.
        use rustix::process::Signal;
        let mut child = started("exec sleep 30", true);
        let _ends = stand_in::Ends::group(child.id());
        let group = group_of(&child);
        let mut sent = Vec::new();
        let mut counting = |whom: Whom, signal: rustix::process::Signal| {
            sent.push((whom, signal));
            send(whom, signal)
        };

        let ended = end_sending(&mut child, true, SOON, &mut counting).expect("it is waited for");

        assert_eq!(ended.signal(), Some(Signal::TERM.as_raw()), "{ended:?}");
        assert_eq!(
            sent,
            [
                (Whom::Group(group), Signal::TERM),
                (Whom::Group(group), Signal::CONT),
                (Whom::Group(group), Signal::KILL),
                (Whom::Alone(group), Signal::KILL),
            ]
        );
    }
}
