// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What every clone the person watches shares**: Get a project ([`crate::getproject`], #1815)
//! and a repo cloned into a workspace ([`crate::repocmd::clone`], #4079).
//!
//! - **How far it has got**: git's `--progress` lines read as a [`Stage`] and a share
//!   ([`read_progress`]), and those mapped onto one scale that never moves backwards ([`Climb`]).
//! - **Why it failed**: git's own words read as a [`Reading`], which each caller says in its own
//!   sentences ([`read_words`]).
//! - **Reading git to its end** ([`follow`]): progress as it comes, the person's stop heard at
//!   once, and, where the caller gives one, a stall limit in place of a total deadline: a large
//!   repo on a slow line takes as long as it takes, and a git that says nothing is stopped.

use std::io::Read;
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::worktree::git;

/// How long a clone may say nothing before it is stopped (#4079): git rewrites its progress line
/// about once a second while anything arrives, so this long of silence is a dead clone.
pub const STALL: Duration = Duration::from_secs(5 * 60);

/// The `-c` settings that let curl end a dead transfer itself: less than a byte a second for
/// [`STALL`]. So low because a forge preparing a large pack sends only a keep-alive every few
/// seconds, which is slow and is not dead.
pub fn low_speed_settings() -> [String; 2] {
    [
        "http.lowSpeedLimit=1".to_owned(),
        format!("http.lowSpeedTime={}", STALL.as_secs()),
    ]
}

/// The `-c` setting of a clone's second try: the known cure for curl's HTTP/2 stream errors on a
/// large pack.
pub const SECOND_TRY: &str = "http.version=HTTP/1.1";

/// How often a running clone is checked for the person's stop and for a stall.
const LISTEN: Duration = Duration::from_millis(50);

/// The most lines of git's own words kept to say why a clone failed.
const KEPT_LINES: usize = 40;

/// Which part of a clone is running, in the person's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// Reaching the forge, and the forge getting the project ready.
    Connecting,
    /// The project's history coming down.
    Downloading,
    /// That history being unpacked.
    Unpacking,
    /// The project's files written into the folder.
    WritingFiles,
    /// Its Git LFS files fetched, after the rest (#4102).
    LargeFiles,
}

impl Stage {
    /// What the window says while this stage runs.
    pub fn words(self) -> &'static str {
        match self {
            Self::Connecting => "Connecting",
            Self::Downloading => "Downloading",
            Self::Unpacking => "Unpacking",
            Self::WritingFiles => "Writing files",
            Self::LargeFiles => "Downloading large files",
        }
    }

    /// Where this stage runs on the one scale of a clone: its first percent and its last.
    fn span(self) -> (u8, u8) {
        match self {
            Self::Connecting => (0, 5),
            Self::Downloading => (5, 80),
            Self::Unpacking => (80, 90),
            Self::WritingFiles => (90, 95),
            Self::LargeFiles => (95, 100),
        }
    }
}

/// How far a clone has got: its stage, and the share of that stage done when git says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub stage: Stage,
    pub percent: Option<u8>,
}

/// One line of git's `--progress` output as [`Progress`], or `None` for a line that is not
/// progress (and may say why a clone failed).
pub fn read_progress(line: &str) -> Option<Progress> {
    let line = line.trim();
    // The forge's own preparation comes as `remote: …`, and is part of connecting.
    let said = line.strip_prefix("remote: ").unwrap_or(line);
    let (what, rest) = said.split_once(':')?;
    let stage = match what {
        "Enumerating objects" | "Counting objects" | "Compressing objects" => Stage::Connecting,
        "Receiving objects" => Stage::Downloading,
        "Resolving deltas" => Stage::Unpacking,
        "Updating files" | "Checking out files" => Stage::WritingFiles,
        // git-lfs's own line, as `git lfs pull` draws it.
        "Downloading LFS objects" => Stage::LargeFiles,
        _ => return None,
    };
    let percent = rest.split_once('%').and_then(|(number, _)| {
        number
            .trim()
            .parse::<u8>()
            .ok()
            .filter(|number| *number <= 100)
    });
    Some(Progress { stage, percent })
}

/// **One percent for a whole clone, never moving backwards** (#4079). git starts again at 0 for
/// each stage, so each stage has its place on one scale ([`Stage::span`]). What it has shown
/// stands: a line further back, a stage counted twice or a second try only hold it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Climb {
    stage: Stage,
    percent: u8,
}

impl Default for Climb {
    fn default() -> Self {
        Self {
            stage: Stage::Connecting,
            percent: 0,
        }
    }
}

impl Climb {
    /// The stage to show, and its percent of the whole clone.
    pub fn now(&self) -> (Stage, u8) {
        (self.stage, self.percent)
    }

    /// Takes in what git just said: where the clone now stands when that moved it, else `None`.
    pub fn heard(&mut self, now: Progress) -> Option<(Stage, u8)> {
        let (from, to) = now.stage.span();
        let share = u16::from(now.percent.unwrap_or(0));
        let reached = from + u8::try_from(u16::from(to - from) * share / 100).unwrap_or(to - from);
        let stage = self.stage.max(now.stage);
        let percent = self.percent.max(reached);
        if stage == self.stage && percent == self.percent {
            return None;
        }
        self.stage = stage;
        self.percent = percent;
        Some(self.now())
    }
}

/// What git's own words say about why a clone failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reading {
    /// The forge did not let this sign-in in.
    NoAccess,
    /// The forge has no project there (or hides a private one from this sign-in).
    Missing,
    /// The forge could not be reached.
    Unreachable,
    /// curl ended a transfer that had gone dead (`http.lowSpeedTime`).
    Stalled,
    /// Git LFS fetched nothing for the stall limit (#4102). Never read from git's words: said of
    /// a stall in [`Stage::LargeFiles`].
    StalledOnLargeFiles,
    /// Anything else: git's last line.
    Other(String),
}

/// git's words read for why a clone failed.
///
/// They are matched lower-cased; `LC_ALL=C` keeps them in English. The no-access words come
/// first: a forge that says "not found" to a sign-in it refuses says so as well.
pub fn read_words(said: &[String]) -> Reading {
    let all = said.join("\n").to_lowercase();
    let any = |words: &[&str]| words.iter().any(|word| all.contains(word));
    if any(&[
        "permission denied",
        "authentication failed",
        "could not read username",
        "could not read password",
        "terminal prompts disabled",
        "host key verification failed",
        "access denied",
        "returned error: 401",
        "returned error: 403",
    ]) {
        Reading::NoAccess
    } else if any(&[
        "repository not found",
        "could not be found",
        "does not appear to be a git repository",
        "returned error: 404",
        "does not exist",
    ]) || all.contains("repository '") && all.contains("' not found")
    {
        Reading::Missing
    } else if any(&["operation too slow"]) {
        Reading::Stalled
    } else if any(&[
        "could not resolve host",
        "could not resolve hostname",
        "connection refused",
        "connection timed out",
        "operation timed out",
        "network is unreachable",
        "failed to connect",
        "no route to host",
    ]) {
        Reading::Unreachable
    } else {
        let last = said
            .iter()
            .rev()
            .map(|line| line.trim())
            .find(|line| !line.is_empty())
            .map(|line| line.strip_prefix("fatal: ").unwrap_or(line))
            .unwrap_or("it stopped without saying why");
        Reading::Other(last.to_owned())
    }
}

/// Whether git's words are a network failure a second try may cure (#4079): the transfer broke
/// part way. What [`read_words`] reads as anything but [`Reading::Other`] would fail the same
/// way again, a transfer that went dead included.
pub fn is_transient(said: &[String]) -> bool {
    let all = said.join("\n").to_lowercase();
    matches!(read_words(said), Reading::Other(_))
        && ["early eof", "rpc failed", "remote end hung up"]
            .iter()
            .any(|word| all.contains(word))
}

/// How a clone [`follow`]ed to its end went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Followed {
    /// git ended by itself, well or not, with the words it said that were not progress.
    Ended { ok: bool, said: Vec<String> },
    /// The person called it off, and git was stopped.
    CalledOff,
    /// git said nothing for the stall limit, and was stopped. Its words so far.
    Stalled { said: Vec<String> },
}

/// Reads a running clone to its end: its progress to `progress`, its other words kept to say
/// why it failed, and the child stopped the moment `called_off` is raised, or once it has said
/// nothing for `stall`. With no `stall` it may take for ever, and only the person ends it. Each
/// line git says beats the pulse this thread holds ([`crate::hookwire::alive`]), so a chat that
/// waits on the clone waits as long as git speaks.
///
/// An error only where the child could no longer be asked how it is.
pub fn follow(
    mut child: Child,
    called_off: &AtomicBool,
    stall: Option<Duration>,
    progress: &mut dyn FnMut(Progress),
) -> std::io::Result<Followed> {
    let (lines, heard) = mpsc::channel::<String>();
    // git says its progress on stderr, and `git lfs pull` its own on stdout (#4162).
    if let Some(stdout) = child.stdout.take() {
        hear_lines(stdout, lines.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        hear_lines(stderr, lines);
    }
    let mut said: Vec<String> = Vec::new();
    let mut last: Option<Progress> = None;
    let mut spoke = Instant::now();
    let silent = |since: Instant| stall.is_some_and(|limit| since.elapsed() >= limit);
    loop {
        if called_off.load(Ordering::SeqCst) {
            git::stop(&mut child);
            return Ok(Followed::CalledOff);
        }
        if silent(spoke) {
            git::stop(&mut child);
            return Ok(Followed::Stalled { said });
        }
        match heard.recv_timeout(LISTEN) {
            Ok(line) => {
                spoke = Instant::now();
                // A chat waiting on this clone is told it is alive (#4080).
                crate::hookwire::alive::beat();
                match read_progress(&line) {
                    Some(now) if last != Some(now) => {
                        last = Some(now);
                        progress(now);
                    }
                    Some(_) => {}
                    None => {
                        said.push(line);
                        if said.len() > KEPT_LINES {
                            said.remove(0);
                        }
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let status = loop {
        if called_off.load(Ordering::SeqCst) {
            git::stop(&mut child);
            return Ok(Followed::CalledOff);
        }
        if silent(spoke) {
            git::stop(&mut child);
            return Ok(Followed::Stalled { said });
        }
        match child.try_wait()? {
            Some(status) => break status,
            None => std::thread::sleep(LISTEN),
        }
    };
    Ok(Followed::Ended {
        ok: status.success(),
        said,
    })
}

/// Hands on each line `from` says, on a thread of its own, until it ends. A progress line is
/// rewritten in place with `\r`, so either ends a line.
fn hear_lines(from: impl std::io::Read + Send + 'static, lines: mpsc::Sender<String>) {
    std::thread::spawn(move || {
        let mut line = Vec::new();
        for byte in std::io::BufReader::new(from).bytes() {
            let Ok(byte) = byte else { break };
            if byte != b'\r' && byte != b'\n' {
                line.push(byte);
                continue;
            }
            if !line.is_empty() {
                if lines
                    .send(String::from_utf8_lossy(&line).into_owned())
                    .is_err()
                {
                    return;
                }
                line.clear();
            }
        }
        if !line.is_empty() {
            let _ = lines.send(String::from_utf8_lossy(&line).into_owned());
        }
    });
}

#[cfg(test)]
mod tests;
