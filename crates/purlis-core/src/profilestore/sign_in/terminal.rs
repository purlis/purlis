// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A driven sign-in on a pool's runner, followed from a terminal** (#4046, spec #4023):
//! `purlis profile sign-in <profile> --pool <pool>` takes the same steps as the window's strip
//! ([`super::driven::Event`]), and says them as lines. It says a page before it opens it, a device
//! code before its page, and reads a code from its input, line by line, when one is asked for.
//! Anything but signed in is an `Err` that says what to do, which the command exits non-zero on.

use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use super::driven::{Driving, Event};

/// How long a step is waited for before the terminal's input is looked at.
const LOOK_EVERY: Duration = Duration::from_millis(100);

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
    let mut asked = false;
    loop {
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
                    return Err(format!(
                        "'{profile}' did not sign in on pool {pool}: {why}. Run this again to \
                         try once more."
                    ));
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
            Ok(code) => match driving.paste(&code) {
                Ok(()) => asked = false,
                Err(why) => terminal.say(&format!("{why}. Paste it again, then press Return:")),
            },
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
