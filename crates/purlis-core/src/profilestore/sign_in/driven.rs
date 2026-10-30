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
//! **Nothing is guessed.** A page is opened only where it is one of the harness's own sign-in pages; what
//! purlis does not recognise within [`PAGE_WITHIN`] is [`Event::CouldNotFollow`], with what the
//! login printed, and the person signs in in a terminal instead.
//!
//! **No credential is read or kept.** What the login prints is read for its page, its prompt and
//! its words of refusal; the code pasted passes through to it and is kept nowhere. Each
//! harness's words are here, beside its sign-in's own words (`FLOWS`): Claude Code's alone so far (2.1.314's
//! `auth login`, which with its input a pipe prints its page and `Paste code here if prompted >`
//! and reads `code#state` lines). [`drive`] runs a [`Login`] and feeds it to a [`Driven`].
//!
//! **Where purlis cannot follow, the person drives** (#4043): [`in_a_terminal`] runs the login in
//! a terminal of the runner's and says what it prints as [`Event::Printed`], read for nothing and
//! kept nowhere, and what the person types goes back to it ([`Driving::typed`]). Its status is
//! asked when it ends, however it ended.

use std::io::{Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError};
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

/// How long what a login printed is still read once it has ended: something it started may hold
/// its output open.
const DRAIN_WITHIN: Duration = Duration::from_secs(5);

/// How many of the last lines a login printed are kept, to show when purlis cannot follow it.
const KEPT_LINES: usize = 24;

/// The most of one line kept, in characters: a longer one is cut there.
const MOST_LINE: usize = 1024;

/// The most a login prints with no line break before it is read as a line.
const MOST_PARTIAL: usize = 16 * 1024;

/// The most of what a login printed that is said back, in characters: its last.
const MOST_SAID: usize = 4096;

/// How long a login that showed its page has to ask for the code, and one that was given a code
/// has to answer it, before purlis says it cannot follow it: past these, it asks something else.
const NEXT_WITHIN: Duration = Duration::from_secs(60);

/// The most typed into a terminal at once: a paste.
const MOST_TYPED: usize = 64 * 1024;

/// How many steps wait to be asked for before what the login prints is read no further: a login
/// that prints faster than the window draws is held back, never kept without end.
const MOST_WAITING: usize = 64;

/// How long the last steps wait to be asked for before no one is taken to be asking any more.
const UNHEARD_WITHIN: Duration = Duration::from_secs(60);

/// **Where a login's steps are told**: held back while they wait to be asked for, and let go
/// once no one asks any more, so no thread waits on a window that went away.
#[derive(Clone)]
struct Telling {
    tell: SyncSender<Event>,
    /// Set once the last steps went unasked for [`UNHEARD_WITHIN`].
    unheard: Arc<AtomicBool>,
}

impl Telling {
    /// Tells `event`, waiting while the steps before it are not yet asked for.
    fn say(&self, event: Event) {
        self.say_within(event, None);
    }

    /// [`Self::say`], giving up after `within` and saying no one asks any more.
    fn say_within(&self, mut event: Event, within: Option<Duration>) {
        let until = within.map(|within| Instant::now() + within);
        loop {
            match self.tell.try_send(event) {
                Ok(()) | Err(TrySendError::Disconnected(_)) => return,
                Err(TrySendError::Full(back)) => event = back,
            }
            if self.unheard.load(Ordering::SeqCst) {
                return;
            }
            if until.is_some_and(|until| Instant::now() >= until) {
                self.unheard.store(true, Ordering::SeqCst);
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// One of the last steps: told within [`UNHEARD_WITHIN`], or no one is asking.
    fn say_last(&self, event: Event) {
        self.say_within(event, Some(UNHEARD_WITHIN));
    }
}

/// One step of a driven sign-in, as its driver says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Open this page in the browser on this device.
    Open { url: String },
    /// Ask the person for the code the page shows.
    AskCode,
    /// What a login in a terminal printed, escapes and all ([`in_a_terminal`]).
    Printed { text: String },
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
    /// Its sign-in pages: each a host and a path, exactly.
    pages: &'static [(&'static str, &'static str)],
}

/// Claude Code 2.1.314's `auth login`, by its own code: `If the browser didn't open, visit:`
/// or `Sign in at:` before the page, `Paste code here` for the code, its refusals of a pasted
/// code, and `Login failed:`. Its sign-in pages are its own code's three `authorize` pages.
const CLAUDE: Words = Words {
    kind: "claude",
    page_after: &["visit: ", "Sign in at: "],
    asks_code: "Paste code here",
    refuses: &["Invalid code", "That code is not from this sign-in"],
    fails: "Login failed: ",
    pages: &[
        ("claude.ai", "/oauth/authorize"),
        ("claude.com", "/cai/oauth/authorize"),
        ("platform.claude.com", "/oauth/authorize"),
    ],
};

/// The words of a login in a terminal, which are read for nothing.
const NO_WORDS: Words = Words {
    kind: "",
    page_after: &[],
    asks_code: "",
    refuses: &[],
    fails: "",
    pages: &[],
};

/// Which of a login's outputs a piece was printed on: each is read in lines of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Out,
    Err,
}

/// Where a driven login is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum At {
    Starting,
    Opened,
    Asking,
    Pasted,
    /// In a terminal the person drives: nothing it prints is read.
    Terminal,
    Over,
}

/// **The state of one driven login**: fed what it prints ([`Self::read`]), the person's code
/// ([`Self::paste`]), how it ended ([`Self::ended`]) and its status check ([`Self::checked`]),
/// it says each step once.
pub struct Driven {
    words: &'static Words,
    at: At,
    /// When it came to where it is.
    since: Instant,
    /// What was printed on each output since its last line break.
    partial: [String; 2],
    /// The last lines printed, without their escapes.
    lines: Vec<String>,
    /// The reason it said it failed with, where it did.
    failed: Option<String>,
}

impl Driven {
    /// A login of harness `kind`, started at `now`; `None` for one purlis does not follow (Codex and opencode: #4044, #4045).
    pub fn of(kind: &str, now: Instant) -> Option<Self> {
        [&CLAUDE]
            .into_iter()
            .find(|words| words.kind == kind)
            .map(|words| Self::at(words, At::Starting, now))
    }

    /// A login in a terminal the person drives, of any harness, started at `now`.
    pub fn in_a_terminal(now: Instant) -> Self {
        Self::at(&NO_WORDS, At::Terminal, now)
    }

    fn at(words: &'static Words, at: At, now: Instant) -> Self {
        Self {
            words,
            at,
            since: now,
            partial: [String::new(), String::new()],
            lines: Vec::new(),
            failed: None,
        }
    }

    /// Comes to `at`, from now.
    fn came_to(&mut self, at: At) {
        self.at = at;
        self.since = Instant::now();
    }

    /// **What the login printed next on its output**, in any piece: the steps it says.
    pub fn read(&mut self, text: &str) -> Vec<Event> {
        self.read_from(Stream::Out, text)
    }

    /// [`Self::read`], of what it printed on `stream`. Once it is over, nothing more is kept.
    pub fn read_from(&mut self, stream: Stream, text: &str) -> Vec<Event> {
        if self.at == At::Terminal {
            return vec![Event::Printed {
                text: text.to_owned(),
            }];
        }
        let mut said = Vec::new();
        let at = stream as usize;
        for (n, piece) in text.split('\n').enumerate() {
            if self.at == At::Over {
                return said;
            }
            if n > 0 {
                let line = std::mem::take(&mut self.partial[at]);
                self.take(&line, &mut said);
            }
            self.partial[at].push_str(piece);
            if self.partial[at].len() > MOST_PARTIAL {
                let line = std::mem::take(&mut self.partial[at]);
                self.take(&line, &mut said);
            }
        }
        // A prompt ends with no line break.
        if self.at == At::Opened && plain(&self.partial[at]).contains(self.words.asks_code) {
            self.partial[at].clear();
            self.came_to(At::Asking);
            said.push(Event::AskCode);
        }
        said
    }

    /// One whole line the login printed: read for its step, and kept, cut to [`MOST_LINE`].
    fn take(&mut self, line: &str, said: &mut Vec<Event>) {
        let line = plain(line);
        self.line(&line, said);
        if !line.is_empty() {
            self.lines.push(cut(&line, MOST_LINE));
            if self.lines.len() > KEPT_LINES {
                self.lines.remove(0);
            }
        }
    }

    fn line(&mut self, line: &str, said: &mut Vec<Event>) {
        if self.at == At::Over {
            return;
        }
        if let Some(why) = line.strip_prefix(self.words.fails) {
            self.failed = Some(cut(why.trim(), MOST_LINE));
            return;
        }
        match self.at {
            At::Starting => {
                if let Some(address) = self.page_on(line) {
                    match self.page(address) {
                        Some(url) => {
                            self.came_to(At::Opened);
                            said.push(Event::Open { url });
                        }
                        None => {
                            self.at = At::Over;
                            said.push(Event::CouldNotFollow {
                                said: self.said_with(&cut(line, MOST_LINE)),
                            });
                        }
                    }
                }
            }
            At::Opened if line.contains(self.words.asks_code) => {
                self.came_to(At::Asking);
                said.push(Event::AskCode);
            }
            At::Pasted if self.words.refuses.iter().any(|r| line.starts_with(r)) => {
                self.came_to(At::Asking);
                said.push(Event::Refused {
                    said: cut(line, MOST_LINE),
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

    /// `address`, where it is one of the harness's own sign-in pages ([`Words::pages`]), spelled
    /// plainly: https, its host and path exactly, no user or port, and only the characters an
    /// address is written in, so no browser reads it as another.
    fn page(&self, address: &str) -> Option<String> {
        let rest = address.strip_prefix("https://")?;
        let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let (host, after) = rest.split_at(end);
        let path = &after[..after.find(['?', '#']).unwrap_or(after.len())];
        let theirs = self
            .words
            .pages
            .iter()
            .any(|(own, at)| host == *own && path == *at);
        let written = address
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~:/?#[]@!$&()*+,;=%".contains(&b));
        (theirs && written).then(|| address.to_owned())
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
        self.came_to(At::Pasted);
        Ok(format!("{code}\n"))
    }

    /// **How the login ended**: its exit status, `None` where it had none. Ending well is
    /// [`Event::Checking`]; anything else failed, in the login's words where it said why.
    pub fn ended(&mut self, exit: Option<i32>) -> Vec<Event> {
        if self.at == At::Over {
            return Vec::new();
        }
        // Whatever the person did in it, its harness says how it ended.
        if self.at == At::Terminal {
            return vec![Event::Checking];
        }
        let mut said = self.read_from(Stream::Err, "\n");
        said.extend(self.read_from(Stream::Out, "\n"));
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
            .or_else(|| {
                self.lines
                    .last()
                    .map(|line| format!("the runner said: {line}"))
            })
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
            // A fetch the status check started reads as a check that could not say (#4008).
            Standing::CannotTell | Standing::Fetching => Event::Failed {
                why: "the sign-in ended, and its harness could not say whether it is signed in"
                    .to_owned(),
            },
        }
    }

    /// **Whether the login waited too long at `now`**: one that shows no page within
    /// [`PAGE_WITHIN`], asks for no code after it or does not answer a code within
    /// [`NEXT_WITHIN`] cannot be followed. Said once. The person has their time at the page.
    pub fn waited(&mut self, now: Instant) -> Option<Event> {
        let within = match self.at {
            At::Starting => PAGE_WITHIN,
            At::Opened | At::Pasted => NEXT_WITHIN,
            _ => return None,
        };
        if now.saturating_duration_since(self.since) < within {
            return None;
        }
        self.at = At::Over;
        let printed = format!("{}{}", self.partial[0], self.partial[1]);
        Some(Event::CouldNotFollow {
            said: self.said_with(&cut(&plain(&printed), MOST_LINE)),
        })
    }

    /// Whether what is typed goes to it: a login in a terminal, still running.
    fn takes_typing(&self) -> bool {
        self.at == At::Terminal
    }

    /// Whether nothing more is read of it.
    pub fn is_over(&self) -> bool {
        self.at == At::Over
    }

    /// The last lines the login printed, with `more` after them.
    fn said_with(&self, more: &str) -> String {
        let mut all = self.lines.clone();
        if !more.is_empty() && all.last().map(String::as_str) != Some(more) {
            all.push(more.to_owned());
        }
        let all = all.join("\n");
        let skip = all.chars().count().saturating_sub(MOST_SAID);
        all.chars().skip(skip).collect()
    }
}

/// `line`, cut to its first `most` characters.
fn cut(line: &str, most: usize) -> String {
    match line.char_indices().nth(most) {
        Some((at, _)) => format!("{}…", &line[..at]),
        None => line.to_owned(),
    }
}

/// Whether `c` changes how the words around it read without showing itself: a direction mark,
/// an embedding or isolate, or a zero-width character.
fn unseen(c: char) -> bool {
    matches!(c, '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2069}' | '\u{feff}')
}

/// `text` without its terminal escapes (a link's address is kept in its text, as a terminal
/// that shows no links prints it), its control characters, the characters no one sees
/// ([`unseen`]), or the spaces around it.
fn plain(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => match chars.next() {
                // OSC, DCS, SOS, PM and APC: up to BEL or ESC \.
                Some(']' | 'P' | 'X' | '^' | '_') => {
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
            c if c.is_control() || unseen(c) => {}
            c => out.push(c),
        }
    }
    out.trim().to_owned()
}

/// **A driven login, running**: its steps, read with [`Self::next`], and the person's code
/// and cancel going back to it.
pub struct Driving {
    /// Held, so one asks for the next step at a time.
    events: Mutex<Receiver<Event>>,
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
    let driven = Driven::of(kind, Instant::now()).ok_or_else(|| {
        format!(
            "purlis follows {kind}'s sign-in only in a terminal: sign it in with Open in terminal"
        )
    })?;
    run(login, driven, check)
}

/// **Runs `login` in the runner's terminal it sets up itself, for the person to drive**
/// (#4043): what it prints is said as [`Event::Printed`], what is typed goes to it
/// ([`Driving::typed`]), and `check` is asked once it ends, however it ended.
pub fn in_a_terminal(
    login: &Login,
    check: impl FnOnce() -> Standing + Send + 'static,
) -> Result<Driving, String> {
    run(login, Driven::in_a_terminal(Instant::now()), check)
}

fn run(
    login: &Login,
    driven: Driven,
    check: impl FnOnce() -> Standing + Send + 'static,
) -> Result<Driving, String> {
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
    let (tell, events) = std::sync::mpsc::sync_channel(MOST_WAITING);
    let tell = Telling {
        tell,
        unheard: Arc::new(AtomicBool::new(false)),
    };
    let driven = Arc::new(Mutex::new(driven));
    let readers: Vec<_> = [
        (
            Stream::Out,
            child
                .stdout
                .take()
                .map(|out| Box::new(out) as Box<dyn Read + Send>),
        ),
        (
            Stream::Err,
            child
                .stderr
                .take()
                .map(|err| Box::new(err) as Box<dyn Read + Send>),
        ),
    ]
    .into_iter()
    .filter_map(|(stream, from)| Some(reading(stream, from?, Arc::clone(&driven), tell.clone())))
    .collect();
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let (driven, cancelled) = (Arc::clone(&driven), Arc::clone(&cancelled));
        std::thread::spawn(move || watching(child, readers, &driven, &cancelled, &tell, check));
    }
    Ok(Driving {
        events: Mutex::new(events),
        input: Mutex::new(input),
        driven,
        cancelled,
    })
}

/// Reads what the login prints on `from`, its output `stream`, feeding it to `driven` and
/// telling its steps. A character cut between two reads is read whole.
fn reading(
    stream: Stream,
    mut from: Box<dyn Read + Send>,
    driven: Arc<Mutex<Driven>>,
    tell: Telling,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        let mut carried: Vec<u8> = Vec::new();
        while let Ok(read) = from.read(&mut buffer) {
            if read == 0 {
                return;
            }
            carried.extend_from_slice(&buffer[..read]);
            // What ends in the middle of a character waits for the rest of it.
            let whole = match std::str::from_utf8(&carried) {
                Ok(_) => carried.len(),
                Err(e) if e.error_len().is_none() => e.valid_up_to(),
                Err(_) => carried.len(),
            };
            let text = String::from_utf8_lossy(&carried[..whole]).into_owned();
            carried.drain(..whole);
            let said = driven
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .read_from(stream, &text);
            for event in said {
                tell.say(event);
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
    tell: &Telling,
    check: impl FnOnce() -> Standing,
) {
    let started = Instant::now();
    let ended = loop {
        if cancelled.load(Ordering::SeqCst) {
            end(&mut child);
            tell.say_last(Event::Cancelled);
            return;
        }
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) => {}
            Err(_) => {
                end(&mut child);
                break None;
            }
        }
        let (late, over) = {
            let mut driven = driven.lock().unwrap_or_else(PoisonError::into_inner);
            (driven.waited(Instant::now()), driven.is_over())
        };
        if let Some(event) = late {
            end(&mut child);
            tell.say_last(event);
            return;
        }
        // It was said to be over while it ran (a page purlis does not open): nothing more is
        // asked of it.
        if over {
            end(&mut child);
            return;
        }
        if started.elapsed() > LONGEST {
            end(&mut child);
            tell.say_last(Event::Failed {
                why: "the sign-in took too long, so purlis ended it".to_owned(),
            });
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    // All it printed is read before how it ended is said, for as long as that is worth waiting.
    let until = Instant::now() + DRAIN_WITHIN;
    while Instant::now() < until && !readers.iter().all(std::thread::JoinHandle::is_finished) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let said = driven
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .ended(ended);
    let checking = said.last() == Some(&Event::Checking);
    for event in said {
        tell.say_last(event);
    }
    if checking {
        let standing = check();
        let event = driven
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .checked(standing);
        tell.say_last(event);
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
        let events = self.events.lock().unwrap_or_else(PoisonError::into_inner);
        match events.recv_timeout(within) {
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

    /// **Passes what the person typed to a login in a terminal**, as it is.
    pub fn typed(&self, text: &str) -> Result<(), String> {
        let takes = self
            .driven
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .takes_typing();
        if !takes {
            return Err("the sign-in is not in a terminal now".to_owned());
        }
        if text.len() > MOST_TYPED {
            return Err("that is too much to type into a sign-in at once".to_owned());
        }
        let mut input = self.input.lock().unwrap_or_else(PoisonError::into_inner);
        let input = input
            .as_mut()
            .ok_or_else(|| "the sign-in takes no more input".to_owned())?;
        input
            .write_all(text.as_bytes())
            .and_then(|()| input.flush())
            .map_err(|e| format!("what was typed could not be passed to the sign-in: {e}"))
    }

    /// **Ends the process that drives the login**: [`Event::Cancelled`] follows. A code being
    /// passed on meanwhile fails as it ends. Where that process only reaches the login, as
    /// `docker exec` does, the login ends with its input, on the runner ([`super::held`]).
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

/// A sign-in no one drives any more is ended.
impl Drop for Driving {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests;
