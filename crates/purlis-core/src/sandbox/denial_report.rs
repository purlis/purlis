// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The macOS denial-report receiver** (T1.1-52, Q294, Q295): every refusal Seatbelt reports for
//! a chat's processes becomes the same [`Refusal`] the Linux supervisor tells, so a file refusal
//! is a verified block on both systems. What the spike found is in
//! `docs/spikes/seatbelt-denial-reports.md`; this is what it asks to be built on.
//!
//! **How.** Each chat's profile has a [`Tag`] of its own on every deny rule ([`tag_denials`]),
//! and one `log stream` per chat reads the kernel's reports that end in it ([`Receiver`]). The
//! stream is started before the chat's first process and read until [`Receiver::finish`], which
//! keeps reading for a [`LATE_FOLDS`] after the chat ends, because a fold can arrive that late.
//!
//! **What a report is trusted for.** Only a line the kernel wrote (`kernel`, sender `Sandbox`),
//! whose second line is exactly the chat's tag, of the form
//! `Sandbox: <process>(<pid>) deny(<n>) <operation> <target>`, for a pid that is the chat's
//! process or one of its descendants. Any local process can write a log line that ends in the
//! tag, so the tag alone is never the proof ([`Reader`]).
//!
//! **Every refusal is told** (Q299): none is dropped for a window or a limit. Seatbelt folds
//! identical refusals into `N duplicate report(s) for …` lines; each of the N is told too. A
//! report that never came is *unknown*, never *allowed*: a new process's first few refusals are
//! sometimes not reported at all, so a count is at least the refusals reported, not a sum.
//!
//! **Fails closed for a standard user**: macOS lets only an administrator read the log, so for
//! anyone else there is no receiver ([`Unavailable::NotAdmin`]) and nothing is verified.

use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::sandboxblock::supervised::{Refusal, Tell};

/// The program that streams the log.
pub const LOG: &str = "/usr/bin/log";

/// How long the receiver keeps reading after the chat's last process ends: a folded report was
/// measured arriving about 25 s after its process ended.
pub const LATE_FOLDS: Duration = Duration::from_secs(30);

/// How long the stream may take to say it is reading before the receiver gives up on it.
const READY: Duration = Duration::from_secs(10);

/// The operations a refusal is told as, in Seatbelt's own word for each. A report of another
/// operation is counted ([`Totals::unrecognised`]) and not told, as it is no [`Refusal`] the
/// sorter knows.
const OPERATIONS: &[&str] = &[
    "file-read-data",
    "file-read-metadata",
    "file-read-xattr",
    "file-read-acl",
    "file-write-create",
    "file-write-data",
    "file-write-unlink",
    "file-write-flags",
    "file-write-mode",
    "file-write-owner",
    "file-write-times",
    "file-write-xattr",
    "file-write-acl",
    "file-write-setugid",
    "file-write-mount",
    "file-write-unmount",
    "file-link",
    "file-ioctl",
    "network-outbound",
    "network-bind",
];

/// A chat's own tag: what its profile's deny rules say, and what its stream is filtered on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag(String);

impl Tag {
    /// A new tag, random, so that no other chat's reports end in it.
    pub fn new() -> std::io::Result<Tag> {
        let mut bytes = [0u8; 8];
        getrandom::fill(&mut bytes).map_err(std::io::Error::other)?;
        Ok(Tag::from_bytes(bytes))
    }

    fn from_bytes(bytes: [u8; 8]) -> Tag {
        let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        Tag(format!("purlis-denial-{hex}"))
    }

    /// The tag as written in a profile and read in a report.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The predicate `log stream` filters on: the kernel's Sandbox lines that end in the tag.
    pub fn predicate(&self) -> String {
        format!(
            "process == \"kernel\" AND sender == \"Sandbox\" AND eventMessage ENDSWITH \"{}\"",
            self.0
        )
    }
}

/// **`profile` with `tag` as the message of every deny rule in it**, so that each refusal the
/// kernel reports ends in it. A rule `(deny file-write* (subpath "/x"))` becomes
/// `(deny file-write* (with message "<tag>") (subpath "/x"))`: the modifier follows the
/// operations and comes before their filters. Strings, regular expressions and comments are
/// stepped over, so a parenthesis in a path never moves it. A profile whose parentheses do not
/// balance is returned as it is, for the system to refuse.
pub fn tag_denials(profile: &str, tag: &Tag) -> String {
    let modifier = format!(" (with message \"{}\")", tag.as_str());
    let bytes = profile.as_bytes();
    let mut out = String::with_capacity(profile.len() + 64);
    let mut depth = 0usize;
    let mut at = 0;
    let mut copied = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'"' => at = past_string(bytes, at),
            b';' => {
                at = bytes[at..]
                    .iter()
                    .position(|&byte| byte == b'\n')
                    .map_or(bytes.len(), |end| at + end);
            }
            b'(' => {
                depth += 1;
                at += 1;
                if depth == 1 && profile[at..].starts_with("deny") {
                    let after = at + "deny".len();
                    if bytes
                        .get(after)
                        .is_some_and(|byte| byte.is_ascii_whitespace())
                    {
                        let end = after_operations(profile, after);
                        out.push_str(&profile[copied..end]);
                        out.push_str(&modifier);
                        copied = end;
                        at = end;
                    }
                }
            }
            b')' => {
                depth = depth.saturating_sub(1);
                at += 1;
            }
            _ => at += 1,
        }
    }
    if depth != 0 {
        return profile.to_owned();
    }
    out.push_str(&profile[copied..]);
    out
}

/// The index after the closing quote of the string that opens at `start`.
fn past_string(bytes: &[u8], start: usize) -> usize {
    let mut at = start + 1;
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => at += 2,
            b'"' => return at + 1,
            _ => at += 1,
        }
    }
    bytes.len()
}

/// The index after the run of bare operation words that follows `deny`, before the first filter
/// or the closing parenthesis.
fn after_operations(profile: &str, from: usize) -> usize {
    let bytes = profile.as_bytes();
    let mut at = from;
    let mut end = from;
    while at < bytes.len() {
        match bytes[at] {
            byte if byte.is_ascii_whitespace() => at += 1,
            b'(' | b')' | b'"' | b'#' | b';' => break,
            _ => {
                while at < bytes.len()
                    && !bytes[at].is_ascii_whitespace()
                    && !matches!(bytes[at], b'(' | b')' | b'"')
                {
                    at += 1;
                }
                end = at;
            }
        }
    }
    end
}

/// One refusal Seatbelt reported, as the kernel's line says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The name of the process that asked.
    pub process: String,
    /// Its pid.
    pub pid: u32,
    /// The operation, in Seatbelt's word.
    pub operation: String,
    /// What it was refused on.
    pub target: String,
    /// How many refusals the line stands for: 1, or the N of an `N duplicate report(s)` line.
    pub count: u32,
}

/// Reads the physical lines `log stream --style compact` prints into [`Report`]s. A report is a
/// header line and, on the next line, the tag; a header without it is dropped.
pub struct Reader {
    tag: Tag,
    held: Option<Report>,
}

impl Reader {
    /// A reader of the reports that end in `tag`.
    pub fn new(tag: Tag) -> Reader {
        Reader { tag, held: None }
    }

    /// Takes the next line of the stream; the report it completes, if it does.
    pub fn feed(&mut self, line: &str) -> Option<Report> {
        let line = line.trim_end_matches(['\r', '\n']);
        if is_header(line) {
            self.held = parse_header(line);
            return None;
        }
        let held = self.held.take()?;
        (line.trim() == self.tag.as_str()).then_some(held)
    }
}

/// Whether `line` starts with a timestamp, as every record's first line does.
fn is_header(line: &str) -> bool {
    let bytes = line.as_bytes();
    bytes.len() > 19
        && bytes[..19].iter().enumerate().all(|(at, &byte)| match at {
            4 | 7 => byte == b'-',
            10 => byte == b' ',
            13 | 16 => byte == b':',
            _ => byte.is_ascii_digit(),
        })
}

/// Takes the first word of `text`, and the rest with its leading blanks gone.
fn word(text: &str) -> Option<(&str, &str)> {
    let (first, rest) = text.split_once(char::is_whitespace)?;
    Some((first, rest.trim_start()))
}

/// The report a header line says, or None where the kernel's `Sandbox` did not write it or it is
/// not in the form of a refusal.
fn parse_header(line: &str) -> Option<Report> {
    let (_date, rest) = word(line)?;
    let (_time, rest) = word(rest)?;
    let (_level, rest) = word(rest)?;
    let (writer, rest) = word(rest)?;
    let (sender, message) = word(rest)?;
    if !writer.starts_with("kernel[") || sender != "(Sandbox)" {
        return None;
    }
    let (count, body) = match message.split_once(" duplicate report") {
        Some((n, tail)) if !n.is_empty() && n.bytes().all(|byte| byte.is_ascii_digit()) => {
            let tail = tail.strip_prefix('s').unwrap_or(tail);
            (n.parse().ok()?, tail.strip_prefix(" for ")?)
        }
        _ => (1, message),
    };
    let body = body.strip_prefix("Sandbox: ")?;
    let (who, rest) = body.split_once(" deny(")?;
    let (_n, rest) = rest.split_once(") ")?;
    let (operation, target) = rest.split_once(' ')?;
    let (process, pid) = who.strip_suffix(')')?.rsplit_once('(')?;
    Some(Report {
        process: process.to_owned(),
        pid: pid.parse().ok()?,
        operation: operation.to_owned(),
        target: target.to_owned(),
        count,
    })
}

/// Why there is no receiver for a chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unavailable {
    /// The person is a standard user, and macOS lets only an administrator read the log.
    NotAdmin,
    /// The stream did not start, or did not say it was reading.
    NotStarted(String),
}

impl std::fmt::Display for Unavailable {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unavailable::NotAdmin => out.write_str(
                "Refusals are not reported on this account (not an administrator). The sandbox \
                 still blocks what it should, but macOS lets only an administrator read its \
                 reports, so purlis cannot confirm which files this chat was refused, and none \
                 is recorded as confirmed. Sign in with an administrator account to have them \
                 confirmed.",
            ),
            Unavailable::NotStarted(why) => write!(
                out,
                "purlis could not start reading this Mac's sandbox reports, so it cannot \
                 confirm which files this chat was refused: {why}"
            ),
        }
    }
}

impl std::error::Error for Unavailable {}

/// Whether the groups `id -Gn` names include macOS's administrators.
pub fn is_admin_in(groups: &str) -> bool {
    groups.split_whitespace().any(|group| group == "admin")
}

/// Whether the person running this process is an administrator.
pub fn is_admin() -> bool {
    crate::forklock::output(Command::new("/usr/bin/id").arg("-Gn"))
        .is_ok_and(|out| is_admin_in(&String::from_utf8_lossy(&out.stdout)))
}

/// What a receiver did, when it ended.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Totals {
    /// Refusals told: a report's lines and the N of its folded lines.
    pub told: u64,
    /// Reports of the chat's tag for a pid that is not the chat's: not told.
    pub unattributed: u64,
    /// Reports of an operation a [`Refusal`] has no word for: not told.
    pub unrecognised: u64,
}

/// One `log stream` for one chat, telling each refusal reported for its processes.
pub struct Receiver {
    child: Child,
    reader: Option<JoinHandle<Totals>>,
}

impl Receiver {
    /// **Starts reading** the reports that end in `tag`, before the chat's first process starts.
    /// `in_chat` says whether a pid is the chat's process or a descendant of it; a pid it once
    /// said yes to stays the chat's, so a fold that arrives after its process ended is told.
    /// `tell` gets each refusal, on the receiver's own thread.
    ///
    /// Fails closed ([`Unavailable`]) for a standard user, and where the stream does not say it
    /// is reading.
    pub fn start(
        tag: &Tag,
        in_chat: impl Fn(u32) -> bool + Send + 'static,
        tell: Tell,
    ) -> Result<Receiver, Unavailable> {
        if !is_admin() {
            return Err(Unavailable::NotAdmin);
        }
        Receiver::start_with(Path::new(LOG), tag, in_chat, tell)
    }

    /// [`Receiver::start`] with `program` as `log` and no check of the user.
    pub fn start_with(
        program: &Path,
        tag: &Tag,
        in_chat: impl Fn(u32) -> bool + Send + 'static,
        tell: Tell,
    ) -> Result<Receiver, Unavailable> {
        let mut command = Command::new(program);
        command
            .args([
                "stream",
                "--style",
                "compact",
                "--predicate",
                &tag.predicate(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = crate::forklock::spawn(&mut command)
            .map_err(|why| Unavailable::NotStarted(why.to_string()))?;
        let stdout = child.stdout.take().ok_or_else(missing)?;
        let stderr = child.stderr.take().ok_or_else(missing)?;
        let (ready, heard) = channel();
        let said = Arc::new(Mutex::new(String::new()));
        let error_ready = ready.clone();
        let error_said = Arc::clone(&said);
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = BufReader::new(stderr).take(8192).read_to_string(&mut text);
            if let Ok(mut said) = error_said.lock() {
                *said = text;
            }
            let _ = error_ready.send(false);
        });
        let reader = {
            let mut lines = Reader::new(tag.clone());
            let mut known = HashSet::new();
            std::thread::spawn(move || {
                let mut totals = Totals::default();
                let mut first = true;
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    if std::mem::take(&mut first) {
                        let _ = ready.send(true);
                    }
                    if let Some(report) = lines.feed(&line) {
                        tell_report(&report, &in_chat, &mut known, &tell, &mut totals);
                    }
                }
                totals
            })
        };
        let mut receiver = Receiver {
            child,
            reader: Some(reader),
        };
        match heard.recv_timeout(READY) {
            Ok(true) => Ok(receiver),
            Ok(false) | Err(RecvTimeoutError::Disconnected) => {
                receiver.stop();
                let why = said
                    .lock()
                    .map(|text| text.trim().to_owned())
                    .unwrap_or_default();
                Err(Unavailable::NotStarted(if why.is_empty() {
                    "the log stream ended at once".to_owned()
                } else {
                    why
                }))
            }
            Err(RecvTimeoutError::Timeout) => {
                receiver.stop();
                Err(Unavailable::NotStarted(
                    "the log stream did not start reading".to_owned(),
                ))
            }
        }
    }

    /// **Ends the receiver** after `grace` more of reading (the chat has ended: [`LATE_FOLDS`]
    /// lets a late fold arrive), and says what it did.
    pub fn finish(mut self, grace: Duration) -> Totals {
        std::thread::sleep(grace);
        self.stop()
    }

    fn stop(&mut self) -> Totals {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.reader
            .take()
            .and_then(|reader| reader.join().ok())
            .unwrap_or_default()
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        self.stop();
    }
}

fn missing() -> Unavailable {
    Unavailable::NotStarted("the log stream gave no output".to_owned())
}

/// Tells a report's refusals, `count` of them, if its pid is the chat's and its operation known.
fn tell_report(
    report: &Report,
    in_chat: &impl Fn(u32) -> bool,
    known: &mut HashSet<u32>,
    tell: &Tell,
    totals: &mut Totals,
) {
    if !known.contains(&report.pid) {
        if !in_chat(report.pid) {
            totals.unattributed += u64::from(report.count);
            return;
        }
        known.insert(report.pid);
    }
    let Some(operation) = OPERATIONS
        .iter()
        .find(|word| **word == report.operation)
        .copied()
    else {
        totals.unrecognised += u64::from(report.count);
        return;
    };
    let refusal = Refusal {
        operation,
        target: report.target.clone(),
        process: Some(report.process.clone()),
    };
    for _ in 0..report.count {
        tell(&refusal);
        totals.told += 1;
    }
}

#[cfg(test)]
mod tests;
