// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The kill switch's state on disk (OV-1, ADR 0071): whether this machine's agents are
//! stopped, and the journal of every stop, re-arm and tamper.
//!
//! **Two files in charter's config directory** ([`crate::machine::dir`]), beside
//! `machine.json` and not inside it:
//!
//! - `halted`, an **empty** file whose existence says every agent is stopped. Empty because who
//!   stopped them and when is the journal's to say, and a file that says nothing but "I am here"
//!   has nothing in it to parse or to get wrong;
//! - `kill-switch.jsonl`, one [`Entry`] per line.
//!
//! **Either one says stopped** ([`stopped_on_disk`]): the marker, or a journal whose last
//! entry is a stop — **under any spelling of the config home** (RN-5): an older build's
//! `charter stop --all` writes `charter/halted` after rename-local moved the folder to
//! `purlis/`, and that stop holds all the same. So removing the marker by hand does not re-arm a machine; its journal still
//! ends on the stop. Only [`rearm`], which the app's window alone calls, writes the entry that
//! lets agents start again. `charter stop --all` stops and never re-arms.
//!
//! **The residual, stated rather than hidden.** Both files belong to the operator's user, and
//! any process running as that user — an agent among them — can edit both. A running app
//! notices a marker taken away and puts it back (the app's `killswitch` module), and journals
//! that as a [`Event::Tamper`]; it cannot stop a process that rewrites the journal as well.
//! What closes that is in ADR 0071, and is not in this module's power.

use std::io;
use std::path::{Path, PathBuf};

/// The marker's name inside [`crate::machine::dir`].
pub const FILE: &str = "halted";

/// The journal's name inside [`crate::machine::dir`].
pub const JOURNAL: &str = "kill-switch.jsonl";

/// How many lines the journal keeps, newest last. A thousand stops is years of them.
pub const JOURNAL_LINES: usize = 1000;

/// What happened to the switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    /// Every agent was stopped.
    Stop,
    /// The operator let agents start again, from the window.
    Rearm,
    /// The marker went away while agents were stopped, and the app put it back.
    Tamper,
}

/// Who did it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Actor {
    /// The operator, in the app's window.
    Window,
    /// `charter stop --all`.
    Cli,
    /// A runner's desktop, over its link (ADR 0071 as amended by ADR 0078, #2051): any scope of
    /// the desktop may send a stop, and only its window a re-arm.
    Link,
    /// The app itself, on noticing a tamper.
    App,
}

/// One line of the journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Entry {
    /// Epoch seconds.
    pub at: u64,
    pub event: Event,
    pub by: Actor,
}

/// A stop that could not be kept on disk. Whatever asked has still stopped what it can, and
/// has to say this loudly: the next launch, and every other process, may not know.
#[derive(Debug)]
pub struct NotKept {
    /// Why the marker could not be written.
    pub why: io::Error,
    /// Whether the journal line was written all the same, which alone keeps the stop.
    pub journaled: bool,
}

impl std::fmt::Display for NotKept {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.journaled {
            write!(
                f,
                "the stop marker could not be written ({}); the journal keeps the stop",
                self.why
            )
        } else {
            write!(
                f,
                "neither the stop marker nor the journal could be written ({}), so no other \
                 process and no later launch knows every agent was stopped",
                self.why
            )
        }
    }
}

/// Where the marker is, under `config_root`.
pub fn marker(config_root: &Path) -> PathBuf {
    crate::machine::dir(config_root).join(FILE)
}

/// Where the journal is, under `config_root`.
pub fn journal_path(config_root: &Path) -> PathBuf {
    crate::machine::dir(config_root).join(JOURNAL)
}

/// charter's directory under `config_root`, made (0700) if it is not there yet: what a watch
/// on the switch watches, which has to exist before anything is written into it.
pub fn directory(config_root: &Path) -> io::Result<PathBuf> {
    let dir = crate::machine::dir(config_root);
    crate::profiletrust::private_dir(&dir)?;
    Ok(dir)
}

/// Whether a file named `name` in charter's directory is one of the switch's own — the marker,
/// the journal, or a temporary one being renamed over either of them.
pub fn concerns(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    name.contains(FILE) || name.contains(JOURNAL)
}

/// Whether anything is at the marker's path, a link included: a reader that could be talked
/// out of a stop by the shape of what is there would be a switch that fails open.
pub fn marker_present(config_root: &Path) -> bool {
    crate::machine::dirs_spelled(config_root)
        .iter()
        .any(|dir| dir.join(FILE).symlink_metadata().is_ok())
}

/// Whether the journal in the config home folder `dir` ends on anything but a re-arm.
fn stopped_by_journal_in(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join(JOURNAL))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<Entry>(line).ok())
        .next_back()
        .is_some_and(|entry| entry.event != Event::Rearm)
}

/// Whether the files say every agent is stopped: the marker is there, or the journal's last
/// entry is a stop that nothing re-armed.
pub fn stopped_on_disk(config_root: &Path) -> bool {
    marker_present(config_root)
        || crate::machine::dirs_spelled(config_root)
            .iter()
            .any(|dir| stopped_by_journal_in(dir))
}

/// **The files of a config home, as the session host reads them** (#2190): what the window's
/// binary and `purlis serve` give their host, so it hears `purlis stop --all` with no window
/// there. The host reads them only; a re-arm is the window's.
#[cfg(unix)]
pub struct OnDisk(pub std::path::PathBuf);

#[cfg(unix)]
impl purlis_host::StopsElsewhere for OnDisk {
    fn last_stop(&self) -> Option<String> {
        if !stopped_on_disk(&self.0) {
            return None;
        }
        // Each stop the command line writes is a journal line of its own, so the newest line
        // names it; a stop the journal does not hold is the marker's.
        Some(match last(&self.0) {
            Some(entry) => format!("{}:{:?}:{:?}", entry.at, entry.event, entry.by),
            None => "marker".to_owned(),
        })
    }

    /// **The runner's desktop stopped or re-armed it over its link** (#2051): its journal says so,
    /// `by: link`, and the event log records it with the desktop's device id (ADR 0075 as
    /// amended).
    fn link_switched(&self, rearm: bool, device: &str) -> Option<serde_json::Value> {
        let at = now();
        let (event, written) = if rearm {
            (Event::Rearm, rearm_by(&self.0, Actor::Link, at).is_ok())
        } else {
            // Kept in the host's own folder whatever happens here: the journal is the record.
            (Event::Stop, stop(&self.0, Actor::Link, at).is_ok())
        };
        if !written {
            tracing::warn!("purlis: the kill switch's journal did not take a link's line");
        }
        let line = Entry {
            at,
            event,
            by: Actor::Link,
        };
        serde_json::to_value(crate::eventlog::Entry::KillSwitchWrote {
            line,
            device: Some(device.to_owned()),
        })
        .ok()
    }
}

/// Stops every agent on this machine until [`rearm`]: the marker, then one journal line.
///
/// The journal line is written even when the marker cannot be, because it alone keeps the stop
/// for [`stopped_on_disk`]. The error says which of the two was kept.
pub fn stop(config_root: &Path, by: Actor, at: u64) -> Result<(), NotKept> {
    let marked = write_marker(config_root);
    let journaled = append(
        config_root,
        Entry {
            at,
            event: Event::Stop,
            by,
        },
    )
    .is_ok();
    match marked {
        Ok(()) => Ok(()),
        Err(why) => Err(NotKept { why, journaled }),
    }
}

/// Lets agents start again. **The window's alone** (ADR 0071): nothing on the command line
/// calls it, because any agent can run a command.
///
/// The marker goes first and the journal line after. A journal line that cannot be written puts
/// the marker back and fails, so the files never say re-armed without the journal saying so.
pub fn rearm(config_root: &Path, at: u64) -> io::Result<()> {
    rearm_by(config_root, Actor::Window, at)
}

/// [`rearm`], by `by`: the window, or a runner's desktop's window over its link (ADR 0071 as
/// amended, V29b), which is how a runner, with no window of its own, re-arms.
pub fn rearm_by(config_root: &Path, by: Actor, at: u64) -> io::Result<()> {
    // Every spelling's marker (RN-5): a stop under either name holds until the window re-arms.
    for dir in crate::machine::dirs_spelled(config_root) {
        match std::fs::remove_file(dir.join(FILE)) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
    }
    let entry = Entry {
        at,
        event: Event::Rearm,
        by,
    };
    if let Err(why) = append(config_root, entry) {
        let _ = write_marker(config_root);
        return Err(why);
    }
    // A journal under the other spelling that ends on a stop is re-armed in its own folder.
    let here = crate::machine::dir(config_root);
    for dir in crate::machine::dirs_spelled(config_root) {
        if dir != here && stopped_by_journal_in(&dir) {
            append_in(&dir, entry)?;
        }
    }
    Ok(())
}

/// Puts back a marker that went away while agents were stopped, and journals the tamper.
///
/// The journal line is written even when the marker cannot be put back: a tamper is recorded
/// whatever else fails (#1375), and the journal alone keeps the stop. The error says which of
/// the two was kept.
pub fn restore(config_root: &Path, at: u64) -> Result<(), NotKept> {
    let marked = write_marker(config_root);
    let journaled = append(
        config_root,
        Entry {
            at,
            event: Event::Tamper,
            by: Actor::App,
        },
    );
    match (marked, journaled) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(why), journaled) => Err(NotKept {
            why,
            journaled: journaled.is_ok(),
        }),
        (Ok(()), Err(why)) => Err(NotKept {
            why,
            journaled: false,
        }),
    }
}

/// The journal, oldest first. A line that does not read as an [`Entry`] is left out.
pub fn journal(config_root: &Path) -> Vec<Entry> {
    std::fs::read_to_string(journal_path(config_root))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// The journal's newest entry.
pub fn last(config_root: &Path) -> Option<Entry> {
    journal(config_root).pop()
}

/// Seconds since the epoch, as the journal stamps them.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

fn write_marker(config_root: &Path) -> io::Result<()> {
    let dir = crate::machine::dir(config_root);
    crate::profiletrust::private_dir(&dir)?;
    crate::rewrite::replace(
        &dir,
        &marker(config_root),
        b"",
        crate::rewrite::Mode::Private,
    )
}

/// Adds `entry` to the journal, keeping the newest [`JOURNAL_LINES`], the way the save journal
/// is kept (`planegit::journal_append`): read, then renamed over whole.
fn append(config_root: &Path, entry: Entry) -> io::Result<()> {
    append_in(&crate::machine::dir(config_root), entry)
}

/// [`append`], to the journal in the config home folder `dir`.
fn append_in(dir: &Path, entry: Entry) -> io::Result<()> {
    let path = dir.join(JOURNAL);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let line = serde_json::to_string(&entry).map_err(io::Error::other)?;
    lines.push(&line);
    let keep = &lines[lines.len().saturating_sub(JOURNAL_LINES)..];
    let mut out = keep.join("\n");
    out.push('\n');
    crate::profiletrust::private_dir(dir)?;
    crate::rewrite::replace(dir, &path, out.as_bytes(), crate::rewrite::Mode::Private)
}

#[cfg(test)]
mod tests;
