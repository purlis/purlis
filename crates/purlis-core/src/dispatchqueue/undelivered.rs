// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a chat left queued when it ended without the app delivering it** (#3958), kept until
//! the person can read it.
//!
//! When a project opens without a chat that had messages queued, the spool clears its queue as
//! it drops its keys ([`crate::hookwire::spool::forget_all_but`]). First, [`set_aside`] writes
//! what of the queue still checks under those keys to one [`Report`] under [`DIR`], beside the
//! queues and under the same integrity denial. The app then places each message on the dispatch
//! record it was between ([`to_place`], [`placed`]); what no record can keep it keeps as one
//! notice a sender ([`keep_as_notice`], [`notices`]) until the person puts it away
//! ([`put_away`]).
//!
//! # What is kept of a message's text
//!
//! What a dispatch record would keep, and no more ([`crate::dispatchrecord::said_by`], #1520,
//! #1872): a text that holds a secret keeps its line and not its words ([`Message::secret`]),
//! and every other text is kept masked, then masked again whenever it is shown
//! ([`Message::shown`]), so a value learned to be a secret after it was kept is masked too.
//!
//! # Where it is read
//!
//! Only in a spool the chat sandbox's integrity denial covers
//! ([`crate::hookwire::spool::covered`]), as a queue is only written there: a report beside a
//! socket that fell back to a temporary folder is none, and the reader says why in the log.
//!
//! # Once each
//!
//! A message is set aside once: a second look at a queue an app that stopped left behind leaves
//! out every message a report already holds, by id. The app places a message on a record once
//! (the record's line names its id), and keeps it in one notice.

use std::io;
use std::path::{Path, PathBuf};

use super::{Found, Queued, VERSION};

/// The folder of the messages folder the reports are kept in.
pub const DIR: &str = "undelivered";

/// The most of a report's file that is read: [`super::MOST_QUEUED`] messages of at most 4,096
/// bytes' text, with room.
const A_REPORT_IS_READ_UP_TO: u64 = 4 * 1024 * 1024;

/// How a report's file ends once the app found no dispatch record for its messages.
const NOTICE: &str = ".notice.json";

/// **What chat `sender` left queued** when it ended: its messages, read from their own files
/// while its key still checked them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Report {
    v: u32,
    /// The chat that queued them, by the number it had then.
    pub sender: u32,
    /// Its name, where the app found one for it ([`keep_as_notice`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// When its queue was cleared, in seconds since the epoch.
    pub at: i64,
    /// What it queued, oldest first.
    pub messages: Vec<Message>,
}

/// One message of a [`Report`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Message {
    /// Its id, which its sender was told.
    pub id: String,
    /// When it was queued, in seconds since the epoch.
    pub at: i64,
    /// What it was, its text masked, or empty where [`Self::secret`].
    #[serde(flatten)]
    pub what: Queued,
    /// Its text held a secret, so its words are not kept.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub secret: bool,
}

impl Message {
    /// What chat `chat` queued as `what`, as a report keeps it: its text masked, or left out
    /// where it holds a secret.
    fn kept(id: String, at: i64, mut what: Queued) -> Self {
        let mut secret = false;
        if let Some(text) = what.text_mut() {
            if crate::dispatchrecord::holds_a_secret(text) {
                secret = true;
                text.clear();
            } else {
                *text = crate::dispatchrecord::masked(text);
            }
        }
        Self {
            id,
            at,
            what,
            secret,
        }
    }

    /// **Its words as the person reads them**, on one line, masked again now; or that they held
    /// a secret and are not kept. `None` for an acknowledgement alone.
    pub fn shown(&self) -> Option<String> {
        if self.secret {
            return Some("(its text held a secret, so it is not kept)".to_owned());
        }
        let text = self.what.text().filter(|text| !text.trim().is_empty())?;
        let words = text.split_whitespace().collect::<Vec<_>>().join(" ");
        Some(format!(
            "\"{}\"",
            crate::personas::one_line(&crate::dispatchrecord::masked(&words))
        ))
    }

    /// One line of it, as a notice or a record says it: what it was, and what it said.
    pub fn line(&self) -> String {
        let named = self.what.named();
        match (self.shown(), self.what.acknowledges()) {
            (Some(shown), _) => format!("{named}: {shown}"),
            (None, Some(number)) => format!("{named} acknowledging instruction {number}"),
            (None, None) => named,
        }
    }
}

impl Report {
    /// **What the person is told of it**, as the notice of messages no dispatch could keep
    /// says it: the sender, how many, and each one's line.
    pub fn notice(&self) -> String {
        let who = self.name.as_ref().map_or_else(
            || format!("chat {}", self.sender),
            |name| format!("{} (chat {})", crate::personas::one_line(name), self.sender),
        );
        let count = self.messages.len();
        let (messages, were) = if count == 1 {
            ("message", "was")
        } else {
            ("messages", "were")
        };
        let lines: Vec<String> = self.messages.iter().map(Message::line).collect();
        format!(
            "{who} did not come back after purlis restarted, and {count} {messages} it queued \
             while purlis did not answer {were} never delivered, with no running dispatch left \
             to keep them on: {}",
            lines.join("; ")
        )
    }
}

/// **Sets aside what chat `chat` left queued in the spool directory `spool`**, as its keys are
/// about to be dropped: every message that checks under them, is above what the app delivered,
/// and no report holds already, written to one report. Answers how many were set aside.
pub(crate) fn set_aside(spool: &Path, chat: u32, at: i64) -> io::Result<usize> {
    let folder = spool.join(super::DIR).join(DIR);
    let held: std::collections::HashSet<String> = reports_in(spool, Which::ToPlace)
        .into_iter()
        .chain(reports_in(spool, Which::Notices))
        .flat_map(|(_, report)| report.messages)
        .map(|message| message.id)
        .collect();
    let messages: Vec<Message> = super::waiting_in(spool, chat)
        .into_iter()
        .filter_map(|found| match found {
            Found::Message(message) if !held.contains(&message.id) => {
                Some(Message::kept(message.id, message.at, message.what))
            }
            Found::Message(_) | Found::Unchecked(_) | Found::Delivered(_) => None,
        })
        .collect();
    if messages.is_empty() {
        return Ok(0);
    }
    let count = messages.len();
    crate::secrets::make_private_dir(&folder)?;
    crate::contain::no_link_on_the_way(spool, &folder)?;
    let report = Report {
        v: VERSION,
        sender: chat,
        name: None,
        at,
        messages,
    };
    let name = format!("{chat}.{}.json", uuid::Uuid::new_v4().simple());
    whole(
        &folder.join(name),
        &serde_json::to_vec(&report).map_err(io::Error::other)?,
    )
    .map(|()| count)
}

/// **Writes `bytes` to `path` whole**: into a `.part` beside it, synced, then renamed over it,
/// so a reader never sees half of one. A part that could not be written is removed.
fn whole(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write as _;
    let part = path.with_extension("part");
    let done = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&part)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&part, path)
    })();
    if done.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    done
}

/// Which of the reports a reader asks for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Which {
    /// Those the app has not placed yet.
    ToPlace,
    /// Those kept as a notice for the person.
    Notices,
}

/// **The reports the app has still to place** beside `socket`, each with its file, for
/// [`placed`] once it is.
pub fn to_place(socket: &Path) -> Vec<(PathBuf, Report)> {
    reports_in(&super::spool_dir(socket), Which::ToPlace)
}

/// The reports under [`DIR`] in the spool directory `spool` that `which` asks for, oldest
/// first. None in a spool the integrity denial does not cover, nor any file that does not read.
fn reports_in(spool: &Path, which: Which) -> Vec<(PathBuf, Report)> {
    use std::io::Read;
    let folder = spool.join(super::DIR).join(DIR);
    let Ok(entries) = std::fs::read_dir(&folder) else {
        return Vec::new();
    };
    if !super::covered(spool) {
        tracing::warn!(
            "purlis: {} is not in a project's state folder, which a sandboxed chat is denied, so \
             no report of undelivered messages is read there",
            folder.display()
        );
        return Vec::new();
    }
    if crate::contain::no_link_on_the_way(spool, &folder).is_err() {
        return Vec::new();
    }
    let mut reports: Vec<(PathBuf, Report)> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_name().to_str().is_some_and(|name| {
                name.ends_with(".json") && name.ends_with(NOTICE) == (which == Which::Notices)
            }) && entry.file_type().is_ok_and(|kind| kind.is_file())
        })
        .filter_map(|entry| {
            let mut bytes = Vec::new();
            std::fs::File::open(entry.path())
                .ok()?
                .take(A_REPORT_IS_READ_UP_TO + 1)
                .read_to_end(&mut bytes)
                .ok()?;
            if bytes.len() as u64 > A_REPORT_IS_READ_UP_TO {
                return None;
            }
            let report: Report = serde_json::from_slice(&bytes).ok()?;
            (report.v == VERSION).then(|| (entry.path(), report))
        })
        .collect();
    reports.sort_by_key(|(path, report)| (report.at, path.clone()));
    reports
}

/// The report at `path` has been placed, every message of it: it is removed.
pub fn placed(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// **Keeps `messages` of the report at `path` as one notice for the person**: those the app
/// found no running dispatch record for, less any a notice holds already, with the name it
/// found for the sender. The report then goes. A notice that could not be written leaves the
/// report, for the next open to place again.
pub fn keep_as_notice(path: &Path, report: &Report, messages: Vec<Message>, name: Option<String>) {
    let Some(stem) = path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".json"))
    else {
        return;
    };
    let spool = path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .unwrap_or(path);
    let shown: std::collections::HashSet<String> = reports_in(spool, Which::Notices)
        .into_iter()
        .flat_map(|(_, notice)| notice.messages)
        .map(|message| message.id)
        .collect();
    let messages: Vec<Message> = messages
        .into_iter()
        .filter(|message| !shown.contains(&message.id))
        .collect();
    if messages.is_empty() {
        placed(path);
        return;
    }
    let notice = Report {
        v: VERSION,
        sender: report.sender,
        name,
        at: report.at,
        messages,
    };
    let to = path.with_file_name(format!("{stem}{NOTICE}"));
    let written = serde_json::to_vec(&notice)
        .map_err(io::Error::other)
        .and_then(|bytes| whole(&to, &bytes));
    match written {
        Ok(()) => placed(path),
        Err(why) => tracing::warn!(
            "purlis: what chat {} left queued is kept for the next open to report ({why})",
            report.sender
        ),
    }
}

/// **The notices the person is told of** ([`keep_as_notice`]) beside `socket`: each by the name
/// [`put_away`] takes.
pub fn notices(socket: &Path) -> Vec<(String, Report)> {
    reports_in(&super::spool_dir(socket), Which::Notices)
        .into_iter()
        .filter_map(|(path, report)| Some((path.file_name()?.to_str()?.to_owned(), report)))
        .collect()
}

/// **The person put notice `name` away** ([`notices`]): it is removed. A name that is not one of
/// those notices' removes nothing.
pub fn put_away(socket: &Path, name: &str) -> io::Result<()> {
    if !notices(socket).iter().any(|(one, _)| one == name) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "there is no such notice of undelivered messages",
        ));
    }
    std::fs::remove_file(super::queues_for(socket).join(DIR).join(name))
}

#[cfg(all(test, unix))]
mod tests;
