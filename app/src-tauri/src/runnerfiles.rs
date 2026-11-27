// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A pool's runner, as a pane's file link reads its chat's files** (#6387, #6949): the adapter
//! of the core's neutral [`purlis_core::panelinks::RunnerFiles`] for every kind of runner purlis
//! pairs ([`reading_files_needs`]: a Docker pool's, a cluster pool's own, and a chat's fresh runner on
//! a cluster pool), over the session protocol's `read_file` on the window's link to that runner, asked
//! only of one that keeps its own folders and sign-ins from the window (1.97). The path the chat printed is sent as data; the runner resolves and reads it. Nothing
//! here runs `docker exec` or builds a command line.
//!
//! - A hover only looks ([`ReadsOnRunner::look`]), for at most [`LOOK_WITHIN`]: a runner that is
//!   slow to answer leaves the text no link, and never holds the pane up.
//! - A tab reads the file in pieces of one version ([`ReadsOnRunner::read`]): a file that
//!   changed between them is read again from its start, once. The runner reads each piece
//!   alone (#7078), so the pieces joined say whether the file is text.
//! - A file outside the chat's workspace is read only as the file it was when the person
//!   answered the ask for it: one put in its place since, or a link put there, is not read.

use std::sync::Arc;
use std::time::Duration;

use purlis_session_protocol::session::{
    self, Answer, Command, FileReach, FileRead, MOST_FILE_PIECE, MOST_FILE_READ,
};
use purlis_session_protocol::version::Version;

use crate::hostedsessions::HostLink;

/// The longest a hover's look at a runner's file is waited for, inside the pane's own bound.
pub const LOOK_WITHIN: Duration = Duration::from_millis(1500);

/// The longest one piece of a tab's read is waited for.
pub const PIECE_WITHIN: Duration = Duration::from_secs(10);

/// **What reading a chat's files on the runner `named` reaches needs of its link** (#6387,
/// #6949): the session protocol version it must speak, or `None` where the window reads no file
/// there. `named` is as a link to a pool's runner names it: `<pool>`, or `<pool>/<runner>` for a
/// Docker pool's draining runner and a chat's fresh runner.
///
/// Every kind needs [`session::KNOWS_KEPT_FILES`] (1.97, #7064): a runner before it would read
/// its own folders and sign-ins for the window, so it is never asked and its chat's paths stay
/// no link.
///
/// Files are read on every kind of runner purlis pairs, and only as a chat's own link reaches
/// it: a Docker pool's runner as `runners::reachable` says (its pool kept with a runner of its
/// image, or that very runner kept draining), and a cluster pool's own runner or a chat's fresh
/// runner there as `runners::cluster_runner_for` says (its pool kept with its cluster's pin, and
/// the runner kept under that very name).
///
/// Every kind runs the same host, so one adapter ([`OnLink`] over `read_file`) asks them all,
/// and on every kind the path is held to the chat's workspace there, a file outside it asks
/// first, and one opened on that answer is read only while it is the file it was
/// ([`ReadsOnRunner::read`]).
pub(crate) fn reading_files_needs(config: &std::path::Path, named: &str) -> Option<Version> {
    let reached = purlis_core::runners::reachable(config, named).is_ok()
        || purlis_core::runners::cluster_runner_for(config, named).is_ok_and(|kept| kept.is_some());
    reached.then_some(session::KNOWS_KEPT_FILES)
}

/// **One answer of a runner to `read_file`**, as the window asks it: on a link, for one chat.
pub(crate) trait AsksRunner: Send + Sync {
    fn ask(
        &self,
        path: &str,
        reach: FileReach,
        at: Option<u64>,
        within: Duration,
    ) -> Result<FileRead, String>;
}

/// The link to a runner and the chat's program on it.
pub(crate) struct OnLink {
    pub link: Arc<HostLink>,
    pub program: String,
}

impl AsksRunner for OnLink {
    fn ask(
        &self,
        path: &str,
        reach: FileReach,
        at: Option<u64>,
        within: Duration,
    ) -> Result<FileRead, String> {
        let command = Command::ReadFile {
            program: self.program.clone(),
            path: path.to_owned(),
            reach,
            at,
        };
        match self.link.call_refused_within(command, within) {
            Ok(Answer::File(read)) => Ok(*read),
            Ok(_) => Err("the runner did not answer with the file".to_owned()),
            Err(refused) => Err(refused.why),
        }
    }
}

/// **What a runner's file is, read for its tab**.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ReadThere {
    /// Its text.
    Text(String),
    /// Not text, `size` bytes long.
    Binary(u64),
    /// Past [`MOST_FILE_READ`], `size` bytes long.
    TooLarge(u64),
}

/// **A chat's files on its runner**, read through `asks`, named as the chat's tab names it.
pub(crate) struct ReadsOnRunner {
    runner: String,
    asks: Box<dyn AsksRunner>,
}

impl ReadsOnRunner {
    pub(crate) fn new(runner: String, asks: Box<dyn AsksRunner>) -> Self {
        Self { runner, asks }
    }

    /// The runner's name, as the chat's tab says it.
    pub(crate) fn runner(&self) -> &str {
        &self.runner
    }

    /// **`path`, read for its tab**: held to the chat's workspace on the runner, unless the
    /// person answered an ask for it when it was the file `opened` names
    /// (`onrunner::identity_of`), and then read only while it still is that file at that path.
    /// The sentence the tab shows where it can't be.
    pub(crate) fn read(&self, path: &str, opened: Option<&str>) -> Result<ReadThere, String> {
        let mut tried = 0;
        loop {
            tried += 1;
            match self.read_once(path, opened)? {
                Some(read) => return Ok(read),
                None if tried < 2 => {}
                None => {
                    return Err(format!(
                        "The file kept changing on runner {} while purlis read it. Open it again in a moment.",
                        self.runner
                    ));
                }
            }
        }
    }

    /// One read from the start: `None` where the file changed between its pieces.
    fn read_once(&self, path: &str, opened: Option<&str>) -> Result<Option<ReadThere>, String> {
        let reach = match opened {
            Some(_) => FileReach::Anywhere,
            None => FileReach::Workspace,
        };
        let mut text = Vec::new();
        let mut first: Option<(u64, String)> = None;
        loop {
            let at = text.len() as u64;
            let read = self.asks.ask(path, reach, Some(at), PIECE_WITHIN)?;
            let (size, version, piece) = match read {
                FileRead::Plain {
                    path: found,
                    version,
                    ..
                } if opened.is_some_and(|opened| {
                    found != path
                        || purlis_core::panelinks::onrunner::identity_of(&version) != Some(opened)
                }) =>
                {
                    return Err(format!(
                        "This file changed on runner {} since you opened it. Open it from its link in the chat again.",
                        self.runner
                    ));
                }
                FileRead::Plain {
                    size,
                    version,
                    piece: Some(piece),
                    ..
                } => (size, version, piece),
                FileRead::Plain { piece: None, .. } => {
                    return Err(self.said("did not send the file"));
                }
                FileRead::Binary { size, .. } => return Ok(Some(ReadThere::Binary(size))),
                FileRead::TooLarge { size, .. } => return Ok(Some(ReadThere::TooLarge(size))),
                FileRead::Kept { .. } => {
                    return Err(format!("{}.", purlis_core::panelinks::RUNNER_KEPT));
                }
                FileRead::Unsure { .. } => {
                    return Err(format!("{}.", purlis_core::panelinks::RUNNER_UNSURE));
                }
                FileRead::NotThere | FileRead::NotPlain { .. } => {
                    return Err(self.said("no longer has this file"));
                }
                FileRead::Outside { .. } => {
                    return Err(format!(
                        "This file is now outside the chat's workspace on runner {}. Open it from its link in the chat again.",
                        self.runner
                    ));
                }
            };
            match &first {
                None => first = Some((size, version)),
                Some(was) if *was != (size, version) => return Ok(None),
                Some(_) => {}
            }
            if size > MOST_FILE_READ
                || piece.at != at
                || piece.bytes.len() as u64 > MOST_FILE_PIECE
                || at + piece.bytes.len() as u64 > size
            {
                return Err(self.said("sent the file in pieces that do not fit"));
            }
            if piece.bytes.is_empty() && at < size {
                return Err(self.said("sent the file in pieces that do not fit"));
            }
            text.extend_from_slice(&piece.bytes);
            if text.len() as u64 >= size {
                break;
            }
        }
        // Each piece is judged as text on the runner but for a character cut at its edges
        // (#7078): whole, the file is text or it is not.
        let size = text.len() as u64;
        Ok(Some(
            String::from_utf8(text).map_or(ReadThere::Binary(size), ReadThere::Text),
        ))
    }

    /// A sentence that says what the runner did.
    fn said(&self, did: &str) -> String {
        format!(
            "Runner {} {did}. Open it from its link in the chat again.",
            self.runner
        )
    }
}

/// What a runner chat's file tab says when the window cannot reach its runner now.
pub(crate) fn not_reachable(runner: &str) -> String {
    format!(
        "purlis can't reach runner {runner} for this chat now, so it can't show the file. Open it from its link in the chat again."
    )
}

/// A runner's file as the light editor draws it.
pub(crate) fn shown(read: ReadThere) -> crate::piecefiles::PieceFile {
    use crate::piecefiles::PieceFile;
    let bytes = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
    match read {
        ReadThere::Text(text) => PieceFile::Text { text },
        ReadThere::Binary(size) => PieceFile::Binary { bytes: bytes(size) },
        ReadThere::TooLarge(size) => PieceFile::TooLarge { bytes: bytes(size) },
    }
}

impl purlis_core::panelinks::RunnerFiles for ReadsOnRunner {
    fn runner(&self) -> &str {
        &self.runner
    }

    fn look(&self, path: &str) -> Option<FileRead> {
        self.asks
            .ask(path, FileReach::Anywhere, None, LOOK_WITHIN)
            .ok()
    }
}

#[cfg(test)]
mod tests;
