// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A clone that is not a chat's**: the window's and the terminal's (#4079). A chat's stays on
//! `clone`'s own deadline for now, because `gitask` bounds its wait on it.
//!
//! - **No total deadline.** git is stopped once it has said nothing for [`cloning::STALL`], and
//!   curl ends a dead transfer ([`cloning::low_speed_settings`]). A large repo on a slow line
//!   takes as long as it takes, and the window's Cancel ends it sooner.
//! - **It lands whole, or not at all.** git clones into `.cloning-<name>` beside the destination:
//!   the same volume, already ignored by `/workspaces/*/*`, and skipped by [`crate::repos::clones`]
//!   as every dot-name is, so no partial repo ever shows and no sibling's `wire` can touch one.
//!   Only a finished clone is renamed to the destination. A failed or stopped one is removed.
//! - **The hidden folder is held while it runs** ([`Hidden`]), so a crash's leftover is told from
//!   a clone running now: a leftover is ours by name and replaced, a running one is left to the
//!   run that has it. The lock goes with the process, so a killed clone never wedges the next.
//! - **One second try**, on a transfer that broke part way ([`cloning::is_transient`]), over
//!   HTTP/1.1, with a fresh token check.
//! - **The words come from the failure** ([`spoken`]). What the window says names no forge CLI.
//!
//! **A folder an older version left**, with a `.git`, no HEAD that resolves, and nothing else but
//! purlis's own layer in it, is a clone that never finished ([`unfinished`]), and is replaced once
//! this one has landed. Anything else at the destination stays, and is never deleted.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::{Watch, explained};
use crate::cloning::{self, Climb, Followed, Reading, Stage};
use crate::contain;
use crate::forge::gitcredential;
use crate::forge::{self, Forge};
use crate::guest;
use crate::rewrite::Lock;
use crate::worktree::{confine, git};

/// A test's stand-in forge: `-c` settings for its git, the stall limit it waits out, and a script
/// to run in git's place, with the folder to clone into and this try's settings as `$1` and `$2`.
#[cfg(test)]
#[derive(Default)]
pub(super) struct StandIn {
    pub config: Vec<String>,
    pub stall: Option<Duration>,
    pub script: Option<String>,
}

/// One clone, where it comes from and where it goes.
pub(super) struct Route<'a> {
    pub root: &'a Path,
    pub ws: &'a str,
    pub ws_dir: &'a Path,
    pub name: &'a str,
    pub dest: &'a Path,
    pub url: &'a str,
    pub forge: &'a Forge,
    pub watch: Option<&'a Watch<'a>>,
}

/// What came of it.
pub(super) enum Landed {
    Cloned,
    Refused(String),
    Stopped,
    Failed { head: String, said: String },
}

/// How one try ended.
enum Tried {
    Done,
    Stopped,
    Failed(Vec<String>),
    Stalled,
    /// git could not be started, or could no longer be asked how it was: this sentence.
    Lost(String),
}

impl Route<'_> {
    fn stall(&self) -> Duration {
        #[cfg(test)]
        if let Some(stall) = self.watch.and_then(|watch| watch.stand_in.stall) {
            return stall;
        }
        cloning::STALL
    }

    /// `-c` settings for this try, after the one-credential rule.
    fn settings(&self, second: bool) -> Vec<String> {
        let mut config = Vec::from(cloning::low_speed_settings());
        if second {
            config.push(cloning::SECOND_TRY.to_owned());
        }
        #[cfg(test)]
        if let Some(watch) = self.watch {
            config.extend(watch.stand_in.config.iter().cloned());
        }
        config
    }

    /// The helper this try's git consults, asked afresh, so a token that ran out over a long
    /// clone is renewed before its second try. In the window, a host with no account falls back
    /// to the forge's CLI only where that is on this machine: otherwise no helper is named,
    /// rather than one git would fail to start and say so.
    fn helper(&self) -> Option<String> {
        if !gitcredential::in_the_window() {
            return Some(forge::helper_for(self.forge));
        }
        gitcredential::for_the_window(&self.forge.host).or_else(|| {
            forge::find_cli(self.forge.kind.cli()).map(|_| forge::cli_helper_for(self.forge))
        })
    }

    fn tell(&self, stage: Stage, percent: u8) {
        if let Some(watch) = self.watch {
            (watch.heard)(stage, percent);
        }
    }

    /// One `git clone` into `hidden`, followed to its end.
    fn attempt(&self, hidden: &Path, second: bool, climb: &mut Climb) -> Tried {
        let into = hidden.display().to_string();
        let args = [
            "clone",
            "--progress",
            "--no-recurse-submodules",
            "--",
            self.url,
            into.as_str(),
        ];
        let settings = self.settings(second);
        #[cfg(test)]
        let spawned = match self
            .watch
            .and_then(|watch| watch.stand_in.script.as_deref())
        {
            Some(script) => Ok(Ok(in_gits_place(script, &into, &settings.join(" ")))),
            None => git::spawn_network(self.ws_dir, self.helper().as_deref(), &settings, &args),
        };
        #[cfg(not(test))]
        let spawned = git::spawn_network(self.ws_dir, self.helper().as_deref(), &settings, &args);
        let child = match spawned {
            Ok(Ok(child)) => child,
            Ok(Err(why)) => return Tried::Failed(vec![why]),
            Err(unavailable) => return Tried::Lost(unavailable.to_string()),
        };
        let started = Instant::now();
        let never = AtomicBool::new(false);
        let called_off = self.watch.map_or(&never, |watch| watch.called_off);
        let followed = cloning::follow(child, called_off, Some(self.stall()), &mut |now| {
            if let Some((stage, percent)) = climb.heard(now) {
                self.tell(stage, percent);
            }
        });
        let ok = matches!(followed, Ok(Followed::Ended { ok: true, .. }));
        git::network_call_ended(self.ws_dir, &args, ok, started);
        match followed {
            Ok(Followed::Ended { ok: true, .. }) => Tried::Done,
            Ok(Followed::Ended { said, .. }) => Tried::Failed(said),
            Ok(Followed::CalledOff) => Tried::Stopped,
            Ok(Followed::Stalled { .. }) => Tried::Stalled,
            Err(err) => Tried::Lost(format!("purlis lost track of git while it cloned: {err}")),
        }
    }

    fn failed(&self, reading: Reading, said: &[String]) -> Landed {
        if reading == Reading::NoAccess
            && let Some(sign_in) = self
                .watch
                .and_then(|watch| watch.sign_in.lock().ok())
                .as_mut()
        {
            **sign_in = Some(self.forge.host.clone());
        }
        let (head, said) = spoken(
            self.name,
            self.forge,
            &reading,
            said,
            gitcredential::in_the_window(),
        );
        Landed::Failed { head, said }
    }
}

/// A test's [`StandIn::script`], started as git would be.
#[cfg(test)]
fn in_gits_place(script: &str, into: &str, settings: &str) -> std::process::Child {
    use std::process::{Command, Stdio};
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", script, "sh", into, settings])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    git::lead(&mut command);
    crate::forklock::spawn(&mut command).expect("sh runs")
}

/// Clones the route's repo, and lands it at its destination whole.
pub(super) fn land(route: &Route) -> Landed {
    let Some(mut hidden) = (match Hidden::claim(route) {
        Ok(hidden) => hidden,
        Err(why) => return Landed::Refused(why),
    }) else {
        return Landed::Refused(format!(
            "it is already being cloned into workspaces/{}, by another purlis on this machine",
            route.ws
        ));
    };
    let mut climb = Climb::default();
    let mut tried = route.attempt(&hidden.path, false, &mut climb);
    if matches!(&tried, Tried::Failed(said) if cloning::is_transient(said)) {
        hidden.empty();
        tried = route.attempt(&hidden.path, true, &mut climb);
    }
    match tried {
        Tried::Done => {}
        Tried::Stopped => {
            if let Some(watch) = route.watch {
                watch.stopped.store(true, Ordering::SeqCst);
            }
            return Landed::Stopped;
        }
        Tried::Failed(said) => return route.failed(cloning::read_words(&said), &said),
        Tried::Stalled => return route.failed(Reading::Stalled, &[]),
        Tried::Lost(why) => return route.failed(Reading::Other(why), &[]),
    }
    if let Err(why) = hidden.land(route.dest) {
        return route.failed(
            Reading::Other(format!(
                "purlis could not move the finished clone to {}: {why}",
                route.dest.display()
            )),
            &[],
        );
    }
    // git's last stages say nothing on a small repo, so the clone's end is said here.
    route.tell(Stage::WritingFiles, 100);
    Landed::Cloned
}

/// The folder a clone runs in, held by this run until it lands or is removed.
struct Hidden {
    path: PathBuf,
    landed: bool,
    _held: Lock,
}

impl Hidden {
    /// The route's `.cloning-<name>`, empty and held, or `None` where another run holds it. A
    /// link at the name is taken away, never followed. A holder that lands or goes between the
    /// look and the lock leaves another directory at the name, so it is looked at again.
    fn claim(route: &Route) -> Result<Option<Self>, String> {
        let path = route.ws_dir.join(format!(".cloning-{}", route.name));
        if std::fs::symlink_metadata(&path).is_ok_and(|found| found.file_type().is_symlink()) {
            std::fs::remove_file(&path).map_err(|why| why.to_string())?;
        }
        confine::within_workspace(route.root, route.ws, &path).map_err(|why| why.to_string())?;
        contain::writable(route.root, &path).map_err(|why| why.to_string())?;
        for _ in 0..3 {
            match std::fs::create_dir(&path) {
                Ok(()) => {}
                Err(why) if why.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(why) => return Err(why.to_string()),
            }
            let Some(held) = Lock::try_on(&path) else {
                return Ok(None);
            };
            if held_is_at(&held, &path) {
                let mut hidden = Self {
                    path,
                    landed: false,
                    _held: held,
                };
                hidden.empty();
                return Ok(Some(hidden));
            }
        }
        Err("its folder kept changing while purlis looked at it".to_owned())
    }

    /// Everything in the folder taken away, the folder itself kept: it is what is held.
    fn empty(&mut self) {
        let Ok(entries) = std::fs::read_dir(&self.path) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let _ = if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
        }
    }

    /// Renames the finished clone to `dest`, in place of an unfinished folder there. A folder
    /// that has become something else in the meantime stays, and this answers why not.
    fn land(&mut self, dest: &Path) -> std::io::Result<()> {
        if !std::fs::symlink_metadata(&self.path).is_ok_and(|found| found.is_dir()) {
            return Err(std::io::Error::other("the clone's folder is not a folder"));
        }
        if std::fs::symlink_metadata(dest).is_ok() {
            if !unfinished(dest) {
                return Err(std::io::Error::other("something else is there now"));
            }
            std::fs::remove_dir_all(dest)?;
        }
        std::fs::rename(&self.path, dest)?;
        self.landed = true;
        Ok(())
    }
}

impl Drop for Hidden {
    fn drop(&mut self) {
        if !self.landed {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

/// Whether the directory `held` locks is the one at `path` now.
fn held_is_at(held: &Lock, path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    let (Some(file), Ok(found)) = (held.held(), std::fs::symlink_metadata(path)) else {
        return held.held().is_none() && path.is_dir();
    };
    file.metadata()
        .is_ok_and(|locked| locked.dev() == found.dev() && locked.ino() == found.ino())
}

/// Whether what is at `dest` is a clone an older purlis left unfinished: a real `.git`, no HEAD
/// that resolves, and nothing else in it but the layer purlis wired in as it listed the folder as
/// a clone. Asked of `.git` itself, so a broken one is not answered by a repository above it.
pub(super) fn unfinished(dest: &Path) -> bool {
    let git_dir = std::fs::symlink_metadata(dest.join(".git"));
    if !git_dir.is_ok_and(|found| found.is_dir()) {
        return false;
    }
    let head = git::run(
        dest,
        &["--git-dir=.git", "rev-parse", "--verify", "--quiet", "HEAD"],
        git::READ,
    );
    if !head.is_ok_and(|run| !run.ok()) {
        return false;
    }
    let own = guest::hidden_by_purlis(dest);
    holds_only(dest, "", &own)
}

/// Whether `folder`, `rel` below the checkout, holds nothing but `own` and folders of it.
fn holds_only(folder: &Path, rel: &str, own: &std::collections::BTreeSet<String>) -> bool {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return false;
    };
    entries.flatten().all(|entry| {
        let name = entry.file_name().to_string_lossy().into_owned();
        if rel.is_empty() && name == ".git" {
            return true;
        }
        let path = format!("{rel}{name}");
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            holds_only(&entry.path(), &format!("{path}/"), own)
        } else {
            guest::is_purlis_own(own, &path)
        }
    })
}

/// A failed clone in the person's words: the sentence, and git's own lines to go under it. The
/// window gets the sentence alone, naming no forge CLI: `reading` says what git's words said,
/// and a line of git's may name the CLI's helper. The terminal keeps git's lines, and its way to
/// sign in is the CLI it runs with.
fn spoken(
    name: &str,
    forge: &Forge,
    reading: &Reading,
    said: &[String],
    window: bool,
) -> (String, String) {
    let host = &forge.host;
    let cli = forge.kind.cli();
    let head = match reading {
        Reading::NoAccess if window => {
            format!("{name}: Sign in to {host} to clone it, in Settings › You › Accounts.")
        }
        Reading::NoAccess => format!(
            "{name}: Sign in to {host} to clone it: `{cli} auth login`, or Settings › You › \
             Accounts in the window."
        ),
        Reading::Missing => format!(
            "{name}: {host} has no such repo. A private one this sign-in cannot reach looks \
             missing too."
        ),
        Reading::Unreachable => {
            format!("{name}: purlis could not reach {host}. Check your connection, then Retry.")
        }
        Reading::Stalled => format!(
            "{name}: the clone stalled: nothing came from {host} for {} minutes, so purlis \
             stopped it and kept nothing. Retry when your connection is back.",
            cloning::STALL.as_secs() / 60
        ),
        Reading::Other(last) if window => {
            let last = explained(last.clone());
            let shown: Vec<&str> = last.lines().filter(|line| !names_a_cli(line)).collect();
            if shown.is_empty() {
                format!("{name}: git could not clone it.")
            } else {
                format!("{name}: git could not clone it: {}", shown.join(" "))
            }
        }
        Reading::Other(last) => format!("{name}: git could not clone it: {last}"),
    };
    let said = if window {
        String::new()
    } else {
        explained(said.join("\n"))
    };
    (head, said)
}

/// Whether a line of git's names a forge's CLI, as its credential helper's failure does.
fn names_a_cli(line: &str) -> bool {
    line.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|word| word == "gh" || word == "glab")
}
