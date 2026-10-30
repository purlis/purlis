// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A harness's own login, driven with no terminal** (spec #4023, #4042): purlis runs it with
//! its input and output pipes, reads what it prints, and says each step as an [`Event`] to
//! whoever drives it (the window's sign-in strip): open this page here, ask for the code it
//! shows, the code was refused, checking, signed in, could not follow, failed. The code the
//! person pastes goes back to the login as one line ([`Driven::paste`]).
//!
//! **"Signed in" is the harness's own status check's** ([`Driven::checked`]), never a guess
//! from what it printed: a login that ends well is checked first.
//!
//! **Nothing is guessed.** A page is opened only on the harness vendor's own https hosts; what
//! purlis does not recognise within [`PAGE_WITHIN`] is [`Event::CouldNotFollow`], with what the
//! login printed, and the person signs in in a terminal instead.
//!
//! **No credential is read or kept.** What the login prints is read for its page, its prompt and
//! its words of refusal; the code pasted passes through to it and is kept nowhere. Each
//! harness's words are here, beside [`super::FLOWS`]: Claude Code's alone so far (2.1.314's
//! `auth login`, which with its input a pipe prints its page and `Paste code here if prompted >`
//! and reads `code#state` lines). [`drive`] runs a [`Login`] and feeds it to a [`Driven`].

use std::io::{Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::{Login, Standing};

/// How long a login has to show its page before purlis says it cannot follow it: long enough
/// for a runner to fetch the harness first (#4007).
pub const PAGE_WITHIN: Duration = Duration::from_secs(180);

/// The longest a sign-in runs before it is ended.
pub const LONGEST: Duration = Duration::from_secs(15 * 60);

/// The most of a pasted code passed on.
const MOST_CODE: usize = 2048;

/// How many of the last lines a login printed are kept, to show when purlis cannot follow it.
const KEPT_LINES: usize = 24;

/// One step of a driven sign-in, as its driver says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Open this page in the browser on this device.
    Open { url: String },
    /// Ask the person for the code the page shows.
    AskCode,
    /// The harness refused the code, in its own words; another may be pasted.
    Refused { said: String },
    /// The login ended well: its harness's status check is asked.
    Checking,
    /// The status check says it is signed in.
    SignedIn,
    /// purlis did not recognise what the login asks: what it printed, for a terminal instead.
    CouldNotFollow { said: String },
    /// It did not sign in, and why.
    Failed { why: String },
    /// The person cancelled it, and the login was ended.
    Cancelled,
}

impl Event {
    /// Whether nothing follows it.
    pub fn is_last(&self) -> bool {
        matches!(
            self,
            Self::SignedIn | Self::CouldNotFollow { .. } | Self::Failed { .. } | Self::Cancelled
        )
    }
}

/// What a harness's login prints, and where its pages may be.
struct Words {
    kind: &'static str,
    /// What comes right before its page's address on a line.
    page_after: &'static [&'static str],
    /// The prompt for the code, which may end with no line break.
    asks_code: &'static str,
    /// The starts of the lines it refuses a code with.
    refuses: &'static [&'static str],
    /// The start of the line it fails with, before its reason.
    fails: &'static str,
    /// The hosts its pages are on, and their subdomains.
    hosts: &'static [&'static str],
}

/// Claude Code 2.1.314's `auth login`, by its own code: `If the browser didn't open, visit:`
/// or `Sign in at:` before the page, `Paste code here` for the code, its refusals of a pasted
/// code, and `Login failed:`. Its pages are on claude.ai, claude.com and anthropic.com.
const CLAUDE: Words = Words {
    kind: "claude",
    page_after: &["visit: ", "Sign in at: "],
    asks_code: "Paste code here",
    refuses: &["Invalid code", "That code is not from this sign-in"],
    fails: "Login failed: ",
    hosts: &["claude.ai", "claude.com", "anthropic.com"],
};

/// Where a driven login is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum At {
    Starting,
    Opened,
    Asking,
    Pasted,
    Over,
}

/// **The state of one driven login**: fed what it prints ([`Self::read`]), the person's code
/// ([`Self::paste`]), how it ended ([`Self::ended`]) and its status check ([`Self::checked`]),
/// it says each step once.
pub struct Driven {
    words: &'static Words,
    at: At,
    started: Instant,
    /// What was printed since the last line break.
    partial: String,
    /// The last lines printed, without their escapes.
    lines: Vec<String>,
    /// The reason it said it failed with, where it did.
    failed: Option<String>,
}

impl Driven {
    /// A login of harness `kind`, started at `now`; `None` for one purlis does not follow yet.
    pub fn of(kind: &str, now: Instant) -> Option<Self> {
        [&CLAUDE]
            .into_iter()
            .find(|words| words.kind == kind)
            .map(|words| Self {
                words,
                at: At::Starting,
                started: now,
                partial: String::new(),
                lines: Vec::new(),
                failed: None,
            })
    }

    /// **What the login printed next**, in any piece: the steps it says.
    pub fn read(&mut self, text: &str) -> Vec<Event> {
        let mut said = Vec::new();
        self.partial.push_str(text);
        while let Some(end) = self.partial.find('\n') {
            let line: String = self.partial.drain(..=end).collect();
            let line = plain(&line);
            self.line(&line, &mut said);
            if !line.is_empty() {
                self.lines.push(line);
                if self.lines.len() > KEPT_LINES {
                    self.lines.remove(0);
                }
            }
        }
        // A prompt ends with no line break.
        if self.at == At::Opened && plain(&self.partial).contains(self.words.asks_code) {
            self.partial.clear();
            self.at = At::Asking;
            said.push(Event::AskCode);
        }
        said
    }

    fn line(&mut self, line: &str, said: &mut Vec<Event>) {
        if self.at == At::Over {
            return;
        }
        if let Some(why) = line.strip_prefix(self.words.fails) {
            self.failed = Some(why.trim().to_owned());
            return;
        }
        match self.at {
            At::Starting => {
                if let Some(address) = self.page_on(line) {
                    match self.page(address) {
                        Some(url) => {
                            self.at = At::Opened;
                            said.push(Event::Open { url });
                        }
                        None => {
                            self.at = At::Over;
                            said.push(Event::CouldNotFollow {
                                said: self.said_with(line),
                            });
                        }
                    }
                }
            }
            At::Opened if line.contains(self.words.asks_code) => {
                self.at = At::Asking;
                said.push(Event::AskCode);
            }
            At::Pasted if self.words.refuses.iter().any(|r| line.starts_with(r)) => {
                self.at = At::Asking;
                said.push(Event::Refused {
                    said: line.to_owned(),
                });
            }
            _ => {}
        }
    }

    /// The address after one of the words before a page, on `line`.
    fn page_on<'a>(&self, line: &'a str) -> Option<&'a str> {
        self.words.page_after.iter().find_map(|before| {
            let (_, after) = line.split_once(before)?;
            after.split_whitespace().next()
        })
    }

    /// `address`, where it is an https page on one of the harness vendor's hosts, spelled
    /// plainly: a lowercase host with no user or port before it, and only the characters an
    /// address is written in, so no browser reads it as another.
    fn page(&self, address: &str) -> Option<String> {
        let rest = address.strip_prefix("https://")?;
        let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let host = &rest[..end];
        let plain_host = !host.is_empty()
            && host
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-');
        let theirs = self
            .words
            .hosts
            .iter()
            .any(|own| host == *own || host.ends_with(&format!(".{own}")));
        let written = address
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~:/?#[]@!$&()*+,;=%".contains(&b));
        (plain_host && theirs && written).then(|| address.to_owned())
    }

    /// **The line the person's code is passed on as**: the code, trimmed, and a line break.
    /// Refused while the login does not ask for one, and for anything but one word of
    /// printable characters.
    pub fn paste(&mut self, code: &str) -> Result<String, String> {
        if self.at != At::Asking {
            return Err("the sign-in is not asking for a code now".to_owned());
        }
        let code = code.trim();
        if code.is_empty()
            || code.len() > MOST_CODE
            || code.chars().any(|c| c.is_control() || c.is_whitespace())
        {
            return Err("that is not a code: paste the one the page shows, on its own".to_owned());
        }
        self.at = At::Pasted;
        Ok(format!("{code}\n"))
    }

    /// **How the login ended**: its exit status, `None` where it had none. Ending well is
    /// [`Event::Checking`]; anything else failed, in the login's words where it said why.
    pub fn ended(&mut self, exit: Option<i32>) -> Vec<Event> {
        if self.at == At::Over {
            return Vec::new();
        }
        let rest = std::mem::take(&mut self.partial);
        let mut said = self.read(&format!("{rest}\n"));
        if said.iter().any(Event::is_last) {
            return said;
        }
        if exit == Some(0) {
            said.push(Event::Checking);
            return said;
        }
        self.at = At::Over;
        let why = self
            .failed
            .clone()
            .or_else(|| self.lines.last().cloned())
            .unwrap_or_else(|| match exit {
                Some(code) => format!("the sign-in ended with exit status {code}"),
                None => "the sign-in was ended".to_owned(),
            });
        said.push(Event::Failed { why });
        said
    }

    /// **What the harness's own status check said** after the login ended well.
    pub fn checked(&mut self, standing: Standing) -> Event {
        self.at = At::Over;
        match standing {
            Standing::SignedIn => Event::SignedIn,
            Standing::SignedOut => Event::Failed {
                why: "the sign-in ended, and its harness says it is still not signed in".to_owned(),
            },
            Standing::CannotTell => Event::Failed {
                why: "the sign-in ended, and its harness could not say whether it is signed in"
                    .to_owned(),
            },
        }
    }

    /// **Whether the login waited too long at `now`**: one that shows no page within
    /// [`PAGE_WITHIN`] cannot be followed. Said once.
    pub fn waited(&mut self, now: Instant) -> Option<Event> {
        if self.at != At::Starting || now.saturating_duration_since(self.started) < PAGE_WITHIN {
            return None;
        }
        self.at = At::Over;
        Some(Event::CouldNotFollow {
            said: self.said_with(&plain(&self.partial)),
        })
    }

    /// The last lines the login printed, with `more` after them.
    fn said_with(&self, more: &str) -> String {
        let mut all = self.lines.clone();
        if !more.is_empty() && all.last().map(String::as_str) != Some(more) {
            all.push(more.to_owned());
        }
        all.join("\n")
    }
}

/// `text` without its terminal escapes (a link's address is kept in its text, as a terminal
/// that shows no links prints it), its control characters, or the spaces around it.
fn plain(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => match chars.next() {
                // OSC: up to BEL or ESC \.
                Some(']') => {
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' || (c == '\u{1b}' && chars.next_if_eq(&'\\').is_some()) {
                            break;
                        }
                    }
                }
                // CSI: up to its final byte.
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                _ => {}
            },
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.trim().to_owned()
}

/// **A driven login, running**: its steps, read with [`Self::next`], and the person's code
/// and cancel going back to it.
pub struct Driving {
    events: Receiver<Event>,
    input: Mutex<Option<ChildStdin>>,
    driven: Arc<Mutex<Driven>>,
    cancelled: Arc<AtomicBool>,
}

/// **Runs `login` and drives it** as a login of harness `kind`, asking `check` for its status
/// once it ends well. Refused for a harness purlis does not follow, and a login that cannot
/// be run.
pub fn drive(
    login: &Login,
    kind: &str,
    check: impl FnOnce() -> Standing + Send + 'static,
) -> Result<Driving, String> {
    let driven = Driven::of(kind, Instant::now())
        .ok_or_else(|| format!("purlis does not follow {kind}'s sign-in without a terminal yet"))?;
    let mut command = Command::new(&login.program);
    command
        .args(&login.args)
        .envs(login.env.iter().map(|(n, v)| (n, v)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for name in &login.cleared {
        command.env_remove(name);
    }
    let mut child = crate::forklock::spawn(&mut command)
        .map_err(|e| format!("{} could not be run: {e}", login.program))?;
    let input = child.stdin.take();
    let (tell, events) = std::sync::mpsc::channel();
    let driven = Arc::new(Mutex::new(driven));
    let readers: Vec<_> = [
        child
            .stdout
            .take()
            .map(|out| Box::new(out) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|err| Box::new(err) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    .map(|from| reading(from, Arc::clone(&driven), tell.clone()))
    .collect();
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let (driven, cancelled) = (Arc::clone(&driven), Arc::clone(&cancelled));
        std::thread::spawn(move || watching(child, readers, &driven, &cancelled, &tell, check));
    }
    Ok(Driving {
        events,
        input: Mutex::new(input),
        driven,
        cancelled,
    })
}

/// Reads what the login prints on `from`, feeding it to `driven` and telling its steps.
fn reading(
    mut from: Box<dyn Read + Send>,
    driven: Arc<Mutex<Driven>>,
    tell: Sender<Event>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        while let Ok(read) = from.read(&mut buffer) {
            if read == 0 {
                return;
            }
            let text = String::from_utf8_lossy(&buffer[..read]);
            let said = driven
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .read(&text);
            for event in said {
                let _ = tell.send(event);
            }
        }
    })
}

/// Watches the login until it ends, is cancelled, cannot be followed or runs too long; then
/// tells how it ended, its status checked where it ended well.
fn watching(
    mut child: Child,
    readers: Vec<std::thread::JoinHandle<()>>,
    driven: &Mutex<Driven>,
    cancelled: &AtomicBool,
    tell: &Sender<Event>,
    check: impl FnOnce() -> Standing,
) {
    let started = Instant::now();
    let ended = loop {
        if cancelled.load(Ordering::SeqCst) {
            end(&mut child);
            let _ = tell.send(Event::Cancelled);
            return;
        }
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) => {}
            Err(_) => break None,
        }
        let late = driven
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .waited(Instant::now());
        if let Some(event) = late {
            end(&mut child);
            let _ = tell.send(event);
            return;
        }
        if started.elapsed() > LONGEST {
            end(&mut child);
            let _ = tell.send(Event::Failed {
                why: "the sign-in took too long, so purlis ended it".to_owned(),
            });
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    // All it printed is read before how it ended is said.
    for reader in readers {
        let _ = reader.join();
    }
    let said = driven
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .ended(ended);
    let checking = said.last() == Some(&Event::Checking);
    for event in said {
        let _ = tell.send(event);
    }
    if checking {
        let standing = check();
        let event = driven
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .checked(standing);
        let _ = tell.send(event);
    }
}

fn end(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

impl Driving {
    /// **The next step**, waiting at most `within` for it; `None` while there is none yet.
    /// After the last step, a driver that stopped telling says it failed.
    pub fn next(&self, within: Duration) -> Option<Event> {
        match self.events.recv_timeout(within) {
            Ok(event) => Some(event),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => Some(Event::Failed {
                why: "the sign-in stopped answering".to_owned(),
            }),
        }
    }

    /// **Passes the person's code to the login** ([`Driven::paste`]).
    pub fn paste(&self, code: &str) -> Result<(), String> {
        let line = self
            .driven
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .paste(code)?;
        let mut input = self.input.lock().unwrap_or_else(PoisonError::into_inner);
        let input = input
            .as_mut()
            .ok_or_else(|| "the sign-in takes no more input".to_owned())?;
        input
            .write_all(line.as_bytes())
            .and_then(|()| input.flush())
            .map_err(|e| format!("the code could not be passed to the sign-in: {e}"))
    }

    /// **Ends the login**: [`Event::Cancelled`] follows.
    pub fn cancel(&self) {
        self.input
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests;
