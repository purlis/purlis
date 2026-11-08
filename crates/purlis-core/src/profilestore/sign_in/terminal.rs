// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A driven sign-in on a pool's runner, followed from a terminal** (#4046, spec #4023):
//! `purlis profile sign-in <profile> --pool <pool>` takes the same steps as the window's strip
//! ([`super::driven::Event`]), and says them as lines. It says a page before it opens it, a device
//! code before its page, and reads a code from its input, line by line, when one is asked for.
//! Anything but signed in is an `Err` that says what to do, which the command exits non-zero on.

use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use super::Login;
use super::driven::{Driving, Event};

/// **Runs `login` in this terminal, for the person to drive**, its input this terminal's own, and
/// answers how it exited: `purlis profile sign-in --pool --in-terminal`, the line Open in terminal
/// types ([`super::pool_line`]). Where it is held to a pin ([`super::Pin`]), nothing it printed is
/// shown before its pin held on the first thing it printed, so nothing it asks for is typed to a
/// login that did not; where it does not, the login is ended and this answers why.
pub fn in_this_terminal(login: &Login) -> Result<Option<i32>, String> {
    in_this_terminal_to(login, &mut std::io::stdout())
}

/// [`in_this_terminal`], what a pinned login prints written to `out`.
pub fn in_this_terminal_to(login: &Login, out: &mut dyn Write) -> Result<Option<i32>, String> {
    let mut command = std::process::Command::new(&login.program);
    command
        .args(&login.args)
        .envs(login.env.iter().map(|(n, v)| (n, v)));
    for name in &login.cleared {
        command.env_remove(name);
    }
    let could_not_run = |e: std::io::Error| format!("{} could not be run: {e}", login.program);
    let Some(pin) = &login.pin else {
        let status = crate::forklock::spawn(&mut command)
            .and_then(|mut child| child.wait())
            .map_err(could_not_run)?;
        return Ok(status.code());
    };
    command.stdout(std::process::Stdio::piped());
    let mut child = crate::forklock::spawn(&mut command).map_err(could_not_run)?;
    let end = |child: &mut std::process::Child| {
        let _ = child.kill();
        let _ = child.wait();
    };
    let Some(mut printed) = child.stdout.take() else {
        end(&mut child);
        return Err("the sign-in's output could not be read".to_owned());
    };
    let mut buffer = [0u8; 4096];
    let mut heard = false;
    loop {
        let read = match printed.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        if !heard {
            heard = true;
            if let Err(why) = pin.holds() {
                end(&mut child);
                return Err(why);
            }
        }
        if out
            .write_all(&buffer[..read])
            .and_then(|()| out.flush())
            .is_err()
        {
            end(&mut child);
            return Err("what the sign-in printed could not be shown here".to_owned());
        }
    }
    child
        .wait()
        .map(|status| status.code())
        .map_err(|e| format!("the sign-in's end could not be read: {e}"))
}

/// How long a step is waited for before the terminal's input is looked at.
const LOOK_EVERY: Duration = Duration::from_millis(100);

/// How often a sign-in followed here asks whether what it was opened on still holds.
const STILL_EVERY: Duration = Duration::from_secs(1);

/// **A sign-in that failed, in a terminal's words**: one that ran out of its pool's limit names the
/// value to raise it to and where (#4397), as the window's form does.
fn failed_said(profile: &str, pool: &str, why: &str, longest: Duration) -> String {
    if why != super::driven::TOOK_TOO_LONG {
        return format!(
            "'{profile}' did not sign in on pool {pool}: {why}. Run this again to try once more."
        );
    }
    let limit = crate::poollimits::LimitId::SignInWait.limit();
    let hit = limit.waited(longest.as_secs().div_ceil(60), None);
    let said = hit.said();
    // At the most it can be, its words already say what to do instead.
    if hit.suggested.is_none() {
        return format!("'{profile}' did not sign in on pool {pool}: {why}: {said}.");
    }
    format!(
        "'{profile}' did not sign in on pool {pool}: {why}: {said}. Change it on the pool's page \
         in Settings › Pools, then run this again."
    )
}

/// What a terminal does for a sign-in it follows.
pub trait Terminal {
    /// Says a line to the person.
    fn say(&mut self, line: &str);
    /// Opens `url`, already said, in the browser on this device, if it can.
    fn open(&mut self, url: &str);
}

/// **Follows `driving`, profile `profile`'s sign-in on pool `pool`**, to its last step: `Ok` once
/// it is signed in. `lines` are what the person types: an input that ends while a code is asked
/// for cancels the sign-in.
pub fn follow(
    driving: &Driving,
    lines: &Receiver<String>,
    terminal: &mut dyn Terminal,
    profile: &str,
    pool: &str,
) -> Result<(), String> {
    follow_while(driving, lines, terminal, profile, pool, &|| Ok(()))
}

/// [`follow`], with `still` asked every [`STILL_EVERY`] and before each code goes on (#3819,
/// #4164): where it refuses, the sign-in is cancelled, a code goes nowhere, and this answers its
/// refusal. A window ends its sign-ins as a mark is taken back; this command is another process,
/// and a login approved on the vendor's page takes no code, so it asks as it goes.
pub fn follow_while(
    driving: &Driving,
    lines: &Receiver<String>,
    terminal: &mut dyn Terminal,
    profile: &str,
    pool: &str,
    still: &dyn Fn() -> Result<(), String>,
) -> Result<(), String> {
    let mut asked = false;
    let mut last_asked: Option<std::time::Instant> = None;
    loop {
        if last_asked.is_none_or(|at| at.elapsed() >= STILL_EVERY) {
            last_asked = Some(std::time::Instant::now());
            if let Err(why) = still() {
                driving.cancel();
                return Err(why);
            }
        }
        if let Some(event) = driving.next(LOOK_EVERY) {
            match event {
                Event::Open { url } => {
                    terminal.say(&format!("Sign in on this page: {url}"));
                    terminal.open(&url);
                }
                Event::DeviceCode { url, code } => {
                    terminal.say(&format!("Enter this code on the page: {code}"));
                    terminal.say(&format!("The page: {url}"));
                    terminal.open(&url);
                    terminal.say(
                        "GitHub should ask only to read your profile, for opencode's Copilot \
                         sign-in. If it names another app, or asks for your repositories or \
                         organisations, cancel there and here (Ctrl-C).",
                    );
                    terminal.say("Waiting for GitHub…");
                }
                // Said only where the browser's answer is carried to the runner (#4044), which
                // this does not do: `drive_with` refuses such a login with no carry.
                Event::Approve { .. } | Event::PortTaken { .. } => {
                    driving.cancel();
                    return Err(format!(
                        "purlis cannot carry your browser's answer to the runner from a terminal, \
                         so '{profile}' was not signed in on pool {pool}."
                    ));
                }
                Event::AskCode => {
                    terminal.say("Paste the code the page shows, then press Return:");
                    asked = true;
                }
                Event::Refused { said } => {
                    terminal.say(&format!("The sign-in refused that code: {said}"));
                    terminal.say("Paste the code again, then press Return:");
                    asked = true;
                }
                Event::Checking => terminal.say("Checking it is signed in on the runner…"),
                // Said only by a login in the window's terminal (#4043), which this never follows.
                Event::Printed { .. } => {}
                Event::SignedIn => {
                    terminal.say(&format!("'{profile}' is signed in on pool {pool}"));
                    return Ok(());
                }
                Event::CouldNotFollow { said } => {
                    return Err(format!(
                        "purlis could not follow this sign-in, so it ended it. Sign in with \
                         Open in terminal on '{profile}''s line in Settings › Pools, where the \
                         harness asks you itself. It said:\n{said}"
                    ));
                }
                Event::Failed { why } => {
                    return Err(failed_said(profile, pool, &why, driving.longest()));
                }
                Event::Cancelled => {
                    return Err(format!(
                        "the sign-in of '{profile}' on pool {pool} was cancelled; nothing \
                         changed there."
                    ));
                }
            }
        }
        if !asked {
            continue;
        }
        match lines.try_recv() {
            Ok(code) => {
                if let Err(why) = still() {
                    driving.cancel();
                    return Err(why);
                }
                match driving.paste(&code) {
                    Ok(()) => asked = false,
                    Err(why) => {
                        terminal.say(&format!("{why}. Paste it again, then press Return:"));
                    }
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                driving.cancel();
                return Err(format!(
                    "this command's input ended with no code, so '{profile}' was not signed in \
                     on pool {pool}. Run it again in a terminal, or pipe it the code."
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests;
