// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A persona's credentialed MCP server that fails as its chat starts is tried once more**
//! (qw381, ruling T-3 of 2026-10-14).
//!
//! Claude Code starts such a server itself, over stdio, as `purlis secret exec <vault> … --
//! <server…>` ([`crate::personaverbs::chatstart::servers`]); that `purlis` asks the app to run
//! the server ([`super::brokered`]). The harness sees the server close before it answered
//! `initialize` whenever that run ends early, and reports the server as failed for the life of
//! the chat. So the `purlis` the harness started is where the second try lives:
//!
//! - **The start is marked.** purlis puts [`SERVER_ENV`] in the server's environment, naming
//!   it, when it wraps it. A `secret exec` without it is any other command and is never run
//!   twice.
//! - **One more try, only before the server spoke.** A run that was refused, or that ended
//!   before the server wrote a byte on stdout, is asked for again after [`RETRY_AFTER`]. What
//!   the harness had written to the server's stdin by then is kept ([`KEPT_AT_MOST`]) and
//!   handed to the second run first, so the second server reads the `initialize` the first
//!   one took. A server that has spoken is the harness's from then on.
//! - **The last try is said to be the last** ([`McpStart::last`]). When it fails too, the app
//!   tells the person on the chat's tab and the chat at its next turn, with the server's own
//!   error ([`Watch`], [`error_shown`]): its last words, bounded, with every vault value this
//!   process read and every credential-shaped stretch masked.

use std::io::Write;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use super::brokered::Frame;

/// The variable purlis sets in a persona's credentialed MCP server's environment, naming the
/// server, when it wraps the server's start in `secret exec`.
pub const SERVER_ENV: &str = "PURLIS_MCP_SERVER";

/// How long the second try waits after the first failed: long enough for a chat that is still
/// starting to be one the app has open, and well inside a harness's wait for a server.
pub const RETRY_AFTER: Duration = Duration::from_secs(2);

/// The most stdin kept for a second try. A server's first message is a few hundred bytes; past
/// this, nothing is kept and there is no second try.
pub const KEPT_AT_MOST: usize = 256 * 1024;

/// The most characters of a server's own error shown to the person and told to the chat.
pub const ERROR_SHOWN_AT_MOST: usize = 400;

/// The most of a failed run's stderr held while it runs: its last bytes.
const STDERR_HELD_AT_MOST: usize = 8 * 1024;

/// What purlis says on the server's stderr between the two tries, in the harness's log.
pub const AGAIN: &str = "the MCP server did not start; purlis is trying it once more";

/// A persona's MCP server's start, as its `secret exec` line carries it to the app.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct McpStart {
    /// The server's name, as the persona's `mcp.json` declares it.
    pub server: String,
    /// Whether this is the last try: a failure now is the person's to hear of.
    pub last: bool,
}

/// The server named in `env` ([`SERVER_ENV`]), where it is a name purlis accepts for one.
pub fn server_in(env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    env(SERVER_ENV).filter(|name| crate::personaverbs::mcp::name_ok(name))
}

/// Whether `persona`'s definition in the project at `root` declares an MCP server named
/// `server`: the one thing a failure the app hears of may name.
pub fn declared_by(root: &std::path::Path, persona: &str, server: &str) -> bool {
    crate::personaverbs::mcp::name_ok(server)
        && crate::personaverbs::mcp::declared(root, persona)
            .0
            .contains_key(server)
}

// ----------------------------------------------------------------------------------------
// the asker's stdin, kept for a second try

/// The asker's stdin, read once on a thread of its own and sent on to whichever run is the
/// current one; until the server speaks, what was read is kept for a second run.
pub(crate) struct Feed {
    state: Mutex<Fed>,
}

struct Fed {
    /// Not read yet: the stream, taken by the reading thread at the first run's start.
    stdin: Option<Box<dyn std::io::Read + Send>>,
    /// Whether there is a stdin at all.
    has: bool,
    /// What was read and is kept for another run, while `keeping`.
    kept: Vec<u8>,
    keeping: bool,
    /// Whether a second run can be handed all that the first was: nothing kept was let go.
    whole: bool,
    /// The stdin has ended.
    closed: bool,
    /// The current run's connection.
    to: Option<Box<dyn Write + Send>>,
}

impl Feed {
    /// `stdin`, kept for a second run when `keeping`.
    pub(crate) fn new(stdin: Option<Box<dyn std::io::Read + Send>>, keeping: bool) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(Fed {
                has: stdin.is_some(),
                stdin,
                kept: Vec::new(),
                keeping,
                whole: true,
                closed: false,
                to: None,
            }),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Fed> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether there is a stdin to send.
    pub(crate) fn has_stdin(&self) -> bool {
        self.lock().has
    }

    /// Whether a run after this one can be handed the whole stdin so far.
    pub(crate) fn replayable(&self) -> bool {
        let fed = self.lock();
        fed.keeping && fed.whole
    }

    /// The server spoke: nothing more is kept, and what was is let go.
    pub(crate) fn spoke(&self) {
        let mut fed = self.lock();
        fed.keeping = false;
        fed.kept = Vec::new();
    }

    /// The run on `to` has started and wants the stdin: what was kept first, then the rest as
    /// it comes. The first call starts the thread that reads it.
    pub(crate) fn attach(self: &Arc<Self>, mut to: Box<dyn Write + Send>) {
        let mut fed = self.lock();
        if !fed.has {
            return;
        }
        // In the pieces it was read in at most, so no line is longer than one the app reads.
        let mut ok = fed
            .kept
            .chunks(16 * 1024)
            .all(|piece| send(&mut *to, &input_of(piece)));
        if ok && fed.closed {
            ok = send(&mut *to, &super::brokered::Input::StdinClosed);
        }
        fed.to = ok.then_some(to);
        if let Some(stdin) = fed.stdin.take() {
            let feed = Arc::clone(self);
            let _ = std::thread::Builder::new()
                .name("purlis-secret-exec-stdin".into())
                .spawn(move || feed.read(stdin));
        }
    }

    fn read(&self, mut stdin: Box<dyn std::io::Read + Send>) {
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            let n = match stdin.read(&mut buf) {
                Ok(0) | Err(_) => 0,
                Ok(n) => n,
            };
            let mut fed = self.lock();
            if n == 0 {
                fed.closed = true;
                if let Some(to) = fed.to.as_mut()
                    && !send(&mut **to, &super::brokered::Input::StdinClosed)
                {
                    fed.to = None;
                }
                return;
            }
            let chunk = &buf[..n];
            if fed.keeping {
                if fed.kept.len() + n <= KEPT_AT_MOST {
                    fed.kept.extend_from_slice(chunk);
                } else {
                    fed.whole = false;
                    fed.kept = Vec::new();
                }
            }
            if let Some(to) = fed.to.as_mut()
                && !send(&mut **to, &input_of(chunk))
            {
                fed.to = None;
            }
        }
    }
}

fn input_of(bytes: &[u8]) -> super::brokered::Input {
    use base64::Engine as _;
    super::brokered::Input::Stdin(base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn send(to: &mut dyn Write, input: &super::brokered::Input) -> bool {
    let Ok(mut line) = serde_json::to_vec(input) else {
        return false;
    };
    line.push(b'\n');
    to.write_all(&line).and_then(|()| to.flush()).is_ok()
}

// ----------------------------------------------------------------------------------------
// the app: what the last try's run came to

/// What a run's frames came to, as [`Watch`] saw them go by.
#[derive(Debug, Default)]
pub struct Seen {
    /// The server wrote on stdout.
    spoke: bool,
    /// The last of its stderr, as sent: masked by the run.
    stderr: Vec<u8>,
    refused: Option<String>,
    exit: Option<i32>,
}

impl Seen {
    /// **Why the server did not start**, in its own words where it said any, shown
    /// ([`error_shown`]); `None` where it spoke, or where the run has not ended.
    pub fn failure(&self) -> Option<String> {
        if self.spoke {
            return None;
        }
        if let Some(why) = &self.refused {
            return Some(error_shown(why));
        }
        let code = self.exit?;
        let said = error_shown(&String::from_utf8_lossy(&self.stderr));
        Some(if said.is_empty() {
            format!("it ended with status {code} before it answered")
        } else {
            said
        })
    }
}

/// The app's end of a run's connection, watched: every frame passes on as written, and what
/// they came to is kept in [`Seen`].
pub struct Watch {
    to: Box<dyn Write + Send>,
    line: Vec<u8>,
    seen: Arc<Mutex<Seen>>,
}

impl Watch {
    /// `to`, watched, and where what it saw is read once the run is over.
    pub fn new(to: Box<dyn Write + Send>) -> (Self, Arc<Mutex<Seen>>) {
        let seen = Arc::new(Mutex::new(Seen::default()));
        (
            Self {
                to,
                line: Vec::new(),
                seen: Arc::clone(&seen),
            },
            seen,
        )
    }

    fn heard(&mut self, frame: Frame) {
        let mut seen = self.seen.lock().unwrap_or_else(PoisonError::into_inner);
        match frame {
            Frame::Stdout(_) => seen.spoke = true,
            Frame::Stderr(bytes) => {
                use base64::Engine as _;
                if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(bytes) {
                    seen.stderr.extend_from_slice(&bytes);
                    let over = seen.stderr.len().saturating_sub(STDERR_HELD_AT_MOST);
                    seen.stderr.drain(..over);
                }
            }
            Frame::Refused { why, code } => {
                seen.refused = Some(why);
                seen.exit = Some(code);
            }
            Frame::Exit(code) => seen.exit = Some(code),
            Frame::Started | Frame::Note(_) => {}
        }
    }
}

impl Write for Watch {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.to.write(buf)?;
        self.line.extend_from_slice(&buf[..n]);
        while let Some(at) = self.line.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.line.drain(..=at).collect();
            if let Ok(frame) = serde_json::from_slice::<Frame>(&line) {
                self.heard(frame);
            }
        }
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.to.flush()
    }
}

/// **A server's own error as the person and the chat are shown it**: its lines joined on one,
/// the last [`ERROR_SHOWN_AT_MOST`] characters of them, every vault value this process read
/// ([`super::seen::found_in`]) and every credential-shaped stretch
/// ([`crate::secretshape::leak_spans`]) masked, and every character with no glyph escaped.
pub fn error_shown(raw: &str) -> String {
    let joined = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" | ");
    // Escaped before it is cut, so a text shown once is the same shown again.
    let escaped = crate::shown::one_line(&masked(&joined), crate::shown::NO_CLIP);
    let n = escaped.chars().count();
    if n <= ERROR_SHOWN_AT_MOST {
        return escaped;
    }
    // One character less, for the mark.
    let kept: String = escaped.chars().skip(n + 1 - ERROR_SHOWN_AT_MOST).collect();
    format!("…{kept}")
}

/// `text` with each stretch the masks find replaced by `***`.
fn masked(text: &str) -> String {
    let mut spans: Vec<std::ops::Range<usize>> = super::seen::found_in(text);
    spans.extend(
        crate::secretshape::leak_spans(text)
            .into_iter()
            .map(|(span, _)| span),
    );
    // Byte ranges of `text`, each widened to the characters it touches so nothing of a value is
    // left, then joined where they meet.
    let mut joined: Vec<std::ops::Range<usize>> = Vec::new();
    let mut widened: Vec<std::ops::Range<usize>> = spans
        .into_iter()
        .filter(|span| span.start < span.end && span.start < text.len())
        .map(|span| {
            let start = (0..=span.start)
                .rev()
                .find(|i| text.is_char_boundary(*i))
                .unwrap_or(0);
            let end = (span.end.min(text.len())..=text.len())
                .find(|i| text.is_char_boundary(*i))
                .unwrap_or(text.len());
            start..end
        })
        .collect();
    widened.sort_by_key(|span| span.start);
    for span in widened {
        match joined.last_mut() {
            Some(last) if span.start <= last.end => last.end = last.end.max(span.end),
            _ => joined.push(span),
        }
    }
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for span in joined {
        out.push_str(&text[at..span.start]);
        out.push_str("***");
        at = span.end;
    }
    out.push_str(&text[at..]);
    out
}

#[cfg(all(test, unix))]
#[path = "mcpstart_tests.rs"]
mod tests;
