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
//! harness's words are here, beside its sign-in's own words (`FLOWS`): Claude Code's 2.1.314
//! `auth login`, which with its input a pipe prints its page and `Paste code here if prompted >`
//! and reads `code#state` lines; opencode's 1.18.33 Copilot `auth login`, a device code
//! ([`Event::DeviceCode`], #4045); and Codex's 0.163.0 `login` and opencode's 1.18.33 ChatGPT
//! `auth login`, which print their page and wait for the browser's answer on their own port
//! ([`Event::Approve`], #4044). [`drive`] runs a [`Login`] and feeds it to a [`Driven`].
//!
//! **A device code goes with its page.** Both are said together, once the login has printed both,
//! for whoever drives it to show the code before any browser opens, the page on the vendor's own
//! https host. The login then waits for the vendor by itself, with nothing passed back.
//!
//! **Where purlis cannot follow, the person drives** (#4043): [`in_a_terminal`] runs the login in
//! a terminal of the runner's and says what it prints as [`Event::Printed`], read for nothing and
//! kept nowhere, and what the person types goes back to it ([`Driving::typed`]). Its status is
//! asked when it ends, however it ended.
//!
//! **A login that waits for the browser has nothing to paste** (#4044): [`drive_with`] listens on
//! this Mac's port for that login's redirect from before its page is opened until it ends, and
//! carries the redirect to the runner ([`callback`]). A taken port is said
//! ([`Event::PortTaken`]), and the page is not opened.

use std::io::{Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::callback::{self, Carry};
use super::{Login, Standing};

/// How long a login has to show its page before purlis says it cannot follow it: long enough
/// for a runner to fetch the harness first (#4007).
pub const PAGE_WITHIN: Duration = Duration::from_secs(180);

/// The longest a sign-in runs before it is ended.
pub const LONGEST: Duration = Duration::from_secs(15 * 60);

/// How long a sign-in runs with no one asking for its next step before it is ended: a window
/// that went away. Its driver asks again at least this often.
#[cfg(not(test))]
const UNDRIVEN_AFTER: Duration = Duration::from_secs(60);
#[cfg(test)]
const UNDRIVEN_AFTER: Duration = Duration::from_secs(10);

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
    /// Show this code to enter on the page `url`, the vendor's own, before opening it.
    DeviceCode { url: String, code: String },
    /// Open this page, and the login waits for the browser's answer on `port`: the redirect
    /// with `state` is carried to it. Said once this device is listening for it.
    Approve {
        url: String,
        port: u16,
        state: String,
    },
    /// This device's `port` is taken, so the browser's answer cannot be carried: the page was
    /// not opened.
    PortTaken { port: u16 },
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
            Self::SignedIn
                | Self::PortTaken { .. }
                | Self::CouldNotFollow { .. }
                | Self::Failed { .. }
                | Self::Cancelled
        )
    }
}

/// What a harness's login prints, and where its pages may be.
struct Words {
    kind: &'static str,
    /// The plan it signs in to, for a harness that serves several.
    plan: Option<&'static str>,
    /// What comes right before its page's address on a line.
    page_after: &'static [&'static str],
    /// Whether its page's address may be a line of its own.
    page_alone: bool,
    /// The port it waits on for the browser's answer, for a login with nothing to paste, and
    /// the client its page must name.
    answered_on: Option<u16>,
    client: &'static str,
    /// The prompt for the code, which may end with no line break.
    asks_code: &'static str,
    /// The starts of the lines it refuses a code with.
    refuses: &'static [&'static str],
    /// The start of the line it fails with, before its reason.
    fails: &'static str,
    /// Its sign-in pages: each a host and a path, exactly.
    pages: &'static [(&'static str, &'static str)],
    /// What comes right before its device code on a line, for a login that shows one.
    code_after: &'static str,
    /// The shape its vendor's codes have, in groups of letters and digits: `[4, 4]` is
    /// `WDJB-MJHT`. No other code is shown, so a login cannot pass words off as one.
    code_groups: &'static [usize],
    /// The line of its one question that shows the choice purlis takes already chosen: it is
    /// answered with Return, once, and no other question is.
    chosen: &'static str,
    /// What it ends a line with once it signed in, for a login whose exit status does not say.
    signed_in: &'static str,
    /// What it ends a line with once it failed, where its exit status does not say.
    stopped: &'static str,
}

/// What a login that says none of the rest has: spread into each harness's own.
const NO_WORDS: Words = Words {
    kind: "",
    plan: None,
    page_after: &[],
    page_alone: false,
    answered_on: None,
    client: "",
    asks_code: "",
    refuses: &[],
    fails: "",
    pages: &[],
    code_after: "",
    code_groups: &[],
    chosen: "",
    signed_in: "",
    stopped: "",
};

/// Claude Code 2.1.314's `auth login`, by its own code: `If the browser didn't open, visit:`
/// or `Sign in at:` before the page, `Paste code here` for the code, its refusals of a pasted
/// code, and `Login failed:`. Its sign-in pages are its own code's three `authorize` pages.
const CLAUDE: Words = Words {
    kind: "claude",
    plan: None,
    page_after: &["visit: ", "Sign in at: "],
    asks_code: "Paste code here",
    refuses: &["Invalid code", "That code is not from this sign-in"],
    fails: "Login failed: ",
    pages: &[
        ("claude.ai", "/oauth/authorize"),
        ("claude.com", "/cai/oauth/authorize"),
        ("platform.claude.com", "/oauth/authorize"),
    ],
    ..NO_WORDS
};

/// opencode 1.18.33's Copilot `auth login`, as it ran with its input a pipe (its ending words are
/// its own code's, read from the binary: its spinner stops on `Login successful`, or on `Failed to
/// authorize`): `Select GitHub
/// deployment type`, taken as it stands on GitHub.com (GitHub Enterprise is signed in in a
/// terminal), then `Go to: ` before GitHub's device page and `Enter code: ` before the code. It
/// exits 0 whether GitHub accepted the code or not, so its spinner's last words say which.
const OPENCODE_COPILOT: Words = Words {
    kind: "opencode",
    plan: Some("github-copilot"),
    page_after: &["Go to: "],
    fails: "Error: ",
    pages: &[("github.com", "/login/device")],
    code_after: "Enter code: ",
    // GitHub's own: four and four, upper case, by its device flow's documentation.
    code_groups: &[4, 4],
    chosen: "\u{25cf} GitHub.com",
    signed_in: "Login successful",
    stopped: "Failed to authorize",
    ..NO_WORDS
};

/// Where Codex's and opencode's ChatGPT logins send the person.
const OPENAI_PAGES: &[(&str, &str)] = &[("auth.openai.com", "/oauth/authorize")];

/// The client both of them sign in as, by their own code.
const OPENAI_CLIENT: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

/// Codex 0.163.0's `login`, by its own code: its page on a line of its own after `navigate to
/// this URL to authenticate:`, then it waits on [`callback::PORT`], and `Error logging in:`.
const CODEX: Words = Words {
    kind: "codex",
    page_alone: true,
    answered_on: Some(callback::PORT),
    client: OPENAI_CLIENT,
    fails: "Error logging in: ",
    pages: OPENAI_PAGES,
    ..NO_WORDS
};

/// opencode 1.18.33's ChatGPT `auth login`, as its browser method runs: `Go to: ` before its
/// page, then it waits on [`callback::PORT`].
const OPENCODE_CHATGPT: Words = Words {
    kind: "opencode",
    plan: Some("openai"),
    page_after: &["Go to: "],
    answered_on: Some(callback::PORT),
    client: OPENAI_CLIENT,
    fails: "Error: ",
    pages: OPENAI_PAGES,
    ..NO_WORDS
};

/// The shortest and longest `state` a login's page is believed to have: a login's own, which is
/// long and random.
const LEAST_STATE: usize = 16;
const MOST_STATE: usize = 512;

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
    /// Its device code is shown, and it waits for the vendor by itself.
    Device,
    /// In a terminal the person drives: nothing it prints is read.
    Terminal,
    /// Its page is opened, and it waits for the browser's answer.
    Approving,
    /// The browser's answer reached it: it is left to end.
    Carried,
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
    /// Its device page, once printed, until its code is.
    device_page: Option<String>,
    /// Whether it said it signed in ([`Words::signed_in`]).
    said_signed_in: bool,
    /// Whether its one question ([`Words::chosen`]) was answered, and what to pass it now.
    answered: bool,
    reply: Option<&'static str>,
}

impl Driven {
    /// A login of harness `kind`, started at `now`; `None` for one purlis does not follow.
    pub fn of(kind: &str, now: Instant) -> Option<Self> {
        Self::of_plan(kind, None, now)
    }

    /// A login of harness `kind` to its `plan`, started at `now`; `None` for one purlis does not
    /// follow.
    pub fn of_plan(kind: &str, plan: Option<&str>, now: Instant) -> Option<Self> {
        [&CLAUDE, &OPENCODE_COPILOT, &CODEX, &OPENCODE_CHATGPT]
            .into_iter()
            .find(|words| words.kind == kind && words.plan == plan)
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
            device_page: None,
            said_signed_in: false,
            answered: false,
            reply: None,
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
        if self.at == At::Opened
            && !self.words.asks_code.is_empty()
            && plain(&self.partial[at]).contains(self.words.asks_code)
        {
            self.partial[at].clear();
            self.came_to(At::Asking);
            said.push(Event::AskCode);
        }
        said
    }

    /// **What to pass the login now, unasked**: the answer to its one question, once.
    pub fn reply(&mut self) -> Option<&'static str> {
        self.reply.take()
    }

    /// One whole line the login printed: read for its step, and kept, cut to [`MOST_LINE`].
    fn take(&mut self, line: &str, said: &mut Vec<Event>) {
        let line = last_drawn(line);
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
            self.failed = Some(format!("the runner said: {}", cut(why.trim(), MOST_LINE)));
            return;
        }
        let words = self.words;
        if !words.signed_in.is_empty() && line.ends_with(words.signed_in) {
            self.said_signed_in = true;
        }
        if !words.stopped.is_empty() && line.ends_with(words.stopped) {
            self.failed = Some(format!("the runner said: {}", words.stopped));
        }
        match self.at {
            At::Starting => {
                if !words.chosen.is_empty() && !self.answered && line.contains(words.chosen) {
                    self.answered = true;
                    // Its prompt takes Return as a terminal sends it: a line break is not read.
                    self.reply = Some("\r");
                } else if let Some(code) = self.code_on(line) {
                    let code = code.to_owned();
                    self.device_code(code, line, said);
                } else if let Some(address) = self.page_on(line) {
                    match self.page(address) {
                        // Its page as purlis spells it, from its own host and path alone: nothing the
                        // login wrote after them is opened.
                        Some(url) if !words.code_after.is_empty() => {
                            let page = url.split(['?', '#']).next().unwrap_or_default().to_owned();
                            self.device_page.get_or_insert(page);
                        }
                        Some(url) => match self.step_at(url) {
                            Some((at, step)) => {
                                self.came_to(at);
                                said.push(step);
                            }
                            None => self.lost(line, said),
                        },
                        None => self.lost(line, said),
                    }
                }
            }
            At::Opened
                if !self.words.asks_code.is_empty() && line.contains(self.words.asks_code) =>
            {
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

    /// What follows the words before a device code on `line`, once its page was printed: a code
    /// printed before its page shows nothing.
    fn code_on<'a>(&self, line: &'a str) -> Option<&'a str> {
        let before = self.words.code_after;
        if before.is_empty() || self.device_page.is_none() {
            return None;
        }
        line.split_once(before).map(|(_, code)| code.trim())
    }

    /// The device code the login printed, with its page, where it is one a window may show.
    fn device_code(&mut self, code: String, line: &str, said: &mut Vec<Event>) {
        match self.device_page.take() {
            Some(url) if shaped(&code, self.words.code_groups) => {
                self.came_to(At::Device);
                said.push(Event::DeviceCode { url, code });
            }
            _ => self.lost(line, said),
        }
    }

    /// What it printed on `line` cannot be followed.
    fn lost(&mut self, line: &str, said: &mut Vec<Event>) {
        self.at = At::Over;
        said.push(Event::CouldNotFollow {
            said: self.said_with(&cut(line, MOST_LINE)),
        });
    }

    /// Where the login is, and the step said, once its page `url` is shown: `None` for a login
    /// waiting for the browser whose page does not send it back as purlis can carry.
    fn step_at(&self, url: String) -> Option<(At, Event)> {
        let Some(port) = self.words.answered_on else {
            return Some((At::Opened, Event::Open { url }));
        };
        let state = state_on(&url, port, self.words.client)?;
        Some((At::Approving, Event::Approve { url, port, state }))
    }

    /// The address after one of the words before a page, on `line`.
    fn page_on<'a>(&self, line: &'a str) -> Option<&'a str> {
        let alone = |host: &&(&str, &str)| {
            line.strip_prefix("https://")
                .is_some_and(|rest| rest.starts_with(host.0))
        };
        if self.words.page_alone && self.words.pages.iter().any(|page| alone(&page)) {
            return Some(line);
        }
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

    /// **How the login ended**: its exit status, `None` where it had none, and its own words
    /// where its status does not say ([`Words::signed_in`]). Ending well is [`Event::Checking`];
    /// anything else failed, in the login's words where it said why.
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
        if exit == Some(0) && (self.words.signed_in.is_empty() || self.said_signed_in) {
            if self.at == At::Approving {
                self.came_to(At::Carried);
            }
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

    /// **How carrying the browser's answer went**: carried, the login is left to end; not, it
    /// failed, and why. Nothing is said once it is past waiting for one.
    pub fn carried(&mut self, how: Result<(), String>) -> Option<Event> {
        if self.at != At::Approving {
            return None;
        }
        match how {
            Ok(()) => {
                self.at = At::Carried;
                None
            }
            Err(why) => {
                self.at = At::Over;
                Some(Event::Failed { why })
            }
        }
    }

    /// **This device's port for the browser's answer is taken**: the login is ended, and nothing
    /// more is read of it.
    pub fn port_taken(&mut self, port: u16) -> Event {
        self.at = At::Over;
        Event::PortTaken { port }
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

/// **The `state` of a login's page `url`, where it sends the browser back as purlis can carry**:
/// to this machine on `port` and [`callback::PATH`] exactly (Codex by its address, opencode by its
/// name), asking for a code as its own
/// `client`, with one `state` a login made and written as it is, and every name in its query
/// written plainly, so no vendor reads a name purlis did not. Nothing else about a page is guessed
/// at.
fn state_on(url: &str, port: u16, client: &str) -> Option<String> {
    let (_, query) = url.split_once('?')?;
    let query = query.split('#').next()?;
    if query.split('&').any(|pair| {
        pair.split('=')
            .next()
            .is_some_and(|name| name.contains('%'))
    }) {
        return None;
    }
    let one = |name: &str| {
        let mut values = query
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .filter(|(key, _)| *key == name)
            .map(|(_, value)| value);
        let value = values.next()?;
        values.next().is_none().then_some(value)
    };
    let back = percent_encoding::percent_decode_str(one("redirect_uri")?)
        .decode_utf8()
        .ok()?;
    let state = one("state")?;
    let written = (LEAST_STATE..=MOST_STATE).contains(&state.len())
        && state
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b));
    (written
        && one("response_type")? == "code"
        && one("client_id")? == client
        && ["127.0.0.1", "localhost"]
            .iter()
            .any(|host| back == format!("http://{host}:{port}{}", callback::PATH)))
    .then(|| state.to_owned())
}

/// **Whether a login of harness `kind` to `plan` waits for the browser's answer on its own
/// port** (#4044): one that restarts in a terminal on the runner signs in by its device-code flow
/// there, since nothing carries the browser's answer to a terminal.
pub fn waits_for_the_browser(kind: &str, plan: Option<&str>) -> bool {
    Driven::of_plan(kind, plan, Instant::now())
        .is_some_and(|driven| driven.words.answered_on.is_some())
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

/// Whether `code` has the shape `groups` say: groups of upper-case letters and digits of those
/// lengths, joined by `-`.
fn shaped(code: &str, groups: &[usize]) -> bool {
    let parts: Vec<&str> = code.split('-').collect();
    parts.len() == groups.len()
        && parts.iter().zip(groups).all(|(part, &len)| {
            part.len() == len
                && part
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        })
}

/// What is left on `line` where a spinner redrew it with no line break: what it last drew, in
/// [`plain`] words.
fn last_drawn(line: &str) -> String {
    line.rsplit("\u{1b}[J")
        .map(plain)
        .find(|drawn| !drawn.is_empty())
        .unwrap_or_default()
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
    input: Input,
    driven: Arc<Mutex<Driven>>,
    cancelled: Arc<AtomicBool>,
    /// When its next step was last asked for.
    asked: Arc<Mutex<Instant>>,
}

/// **The forward of a login that waits for the browser**: opened once its page is shown, closed
/// when it ends, whichever comes first, and never opened after.
struct Forwarding {
    carry: Carry,
    /// The port listened on, where it is not the one the login's page names: a test's.
    listen_on: Option<u16>,
    forward: Mutex<Forwarded>,
}

enum Forwarded {
    Not,
    Open { _listening: callback::Forward },
    Ended,
}

impl Forwarding {
    /// Listens for the redirect with `state` on `port`, carrying it and saying how that went.
    /// Answers `false`, with nothing listening, once the login ended.
    fn open(
        &self,
        port: u16,
        state: &str,
        driven: &Arc<Mutex<Driven>>,
        tell: &Telling,
    ) -> std::io::Result<bool> {
        let mut forward = self.forward.lock().unwrap_or_else(PoisonError::into_inner);
        if matches!(*forward, Forwarded::Ended) {
            return Ok(false);
        }
        let (driven, tell) = (Arc::clone(driven), tell.clone());
        *forward = Forwarded::Open {
            _listening: callback::Forward::open(
                self.listen_on.unwrap_or(port),
                state,
                Arc::clone(&self.carry),
                move |how| {
                    let said = driven
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .carried(how);
                    if let Some(event) = said {
                        tell.say(event);
                    }
                },
            )?,
        };
        Ok(true)
    }

    /// Closes it: nothing listens once this returns, and nothing does from here on.
    fn close(&self) {
        let forward = std::mem::replace(
            &mut *self.forward.lock().unwrap_or_else(PoisonError::into_inner),
            Forwarded::Ended,
        );
        drop(forward);
    }
}

/// What is passed to a login, shared with what reads it, which answers its one question.
type Input = Arc<Mutex<Option<ChildStdin>>>;

/// **Runs `login` and drives it** as a login of harness `kind`, asking `check` for its status
/// once it ends well. Refused for a harness purlis does not follow, and a login that cannot
/// be run.
pub fn drive(
    login: &Login,
    kind: &str,
    check: impl FnOnce() -> Standing + Send + 'static,
) -> Result<Driving, String> {
    drive_with(login, kind, None, None, check)
}

/// [`drive`], of a login to `plan` where its harness serves several, with `carry` to take the
/// browser's answer to a login that waits for it: one with no way to is refused.
pub fn drive_with(
    login: &Login,
    kind: &str,
    plan: Option<&str>,
    carry: Option<Carry>,
    check: impl FnOnce() -> Standing + Send + 'static,
) -> Result<Driving, String> {
    let driven = Driven::of_plan(kind, plan, Instant::now())
        .filter(|driven| driven.words.answered_on.is_none() || carry.is_some())
        .ok_or_else(|| {
            format!(
                "purlis follows {kind}'s sign-in only in a terminal: sign it in with Open in \
                 terminal"
            )
        })?;
    let forwarding = carry.map(|carry| {
        Arc::new(Forwarding {
            carry,
            listen_on: None,
            forward: Mutex::new(Forwarded::Not),
        })
    });
    run(login, driven, forwarding, check)
}

/// **Runs `login` in the runner's terminal it sets up itself, for the person to drive**
/// (#4043): what it prints is said as [`Event::Printed`], what is typed goes to it
/// ([`Driving::typed`]), and `check` is asked once it ends, however it ended.
pub fn in_a_terminal(
    login: &Login,
    check: impl FnOnce() -> Standing + Send + 'static,
) -> Result<Driving, String> {
    run(login, Driven::in_a_terminal(Instant::now()), None, check)
}

fn run(
    login: &Login,
    driven: Driven,
    forwarding: Option<Arc<Forwarding>>,
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
    let input: Input = Arc::new(Mutex::new(child.stdin.take()));
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
    .filter_map(|(stream, from)| {
        Some(reading(
            stream,
            from?,
            Arc::clone(&driven),
            Arc::clone(&input),
            forwarding.clone(),
            tell.clone(),
        ))
    })
    .collect();
    let cancelled = Arc::new(AtomicBool::new(false));
    let asked = Arc::new(Mutex::new(Instant::now()));
    {
        let (driven, cancelled, asked) = (
            Arc::clone(&driven),
            Arc::clone(&cancelled),
            Arc::clone(&asked),
        );
        std::thread::spawn(move || {
            let outside = Outside {
                cancelled: &cancelled,
                asked: &asked,
            };
            watching(
                child,
                readers,
                &driven,
                &outside,
                forwarding.as_deref(),
                &tell,
                check,
            );
        });
    }
    Ok(Driving {
        events: Mutex::new(events),
        input,
        driven,
        cancelled,
        asked,
    })
}

/// Reads what the login prints on `from`, its output `stream`, feeding it to `driven` and
/// telling its steps. A character cut between two reads is read whole. The page of a login that
/// waits for the browser is told only once this device listens for its answer.
fn reading(
    stream: Stream,
    mut from: Box<dyn Read + Send>,
    driven: Arc<Mutex<Driven>>,
    input: Input,
    forwarding: Option<Arc<Forwarding>>,
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
            let (said, reply) = {
                let mut driven = driven.lock().unwrap_or_else(PoisonError::into_inner);
                (driven.read_from(stream, &text), driven.reply())
            };
            for event in said {
                let opened = match (&event, &forwarding) {
                    (Event::Approve { port, state, .. }, Some(forwarding)) => forwarding
                        .open(*port, state, &driven, &tell)
                        .map_err(|why| (*port, why)),
                    _ => Ok(true),
                };
                match opened {
                    Ok(true) => {
                        tell.say(event);
                    }
                    Ok(false) => {}
                    Err((port, why)) => {
                        let mut driven = driven.lock().unwrap_or_else(PoisonError::into_inner);
                        let said = if why.kind() == std::io::ErrorKind::AddrInUse {
                            Some(driven.port_taken(port))
                        } else {
                            driven.carried(Err(format!(
                                "this Mac could not listen for your browser's answer: {why}"
                            )))
                        };
                        drop(driven);
                        if let Some(event) = said {
                            tell.say(event);
                        }
                    }
                }
            }
            // A login that went away takes no answer, and says so as it ends.
            if let Some(reply) = reply
                && let Some(input) = input
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .as_mut()
            {
                let _ = input
                    .write_all(reply.as_bytes())
                    .and_then(|()| input.flush());
            }
        }
    })
}

/// **What ends a login from outside it**: Cancel, or no one asking for its next step for
/// [`UNDRIVEN_AFTER`], as of a window that went away.
struct Outside<'a> {
    cancelled: &'a AtomicBool,
    asked: &'a Mutex<Instant>,
}

impl Outside<'_> {
    /// Whether the login is to end now.
    fn ends_it(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
            || self
                .asked
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .elapsed()
                > UNDRIVEN_AFTER
    }
}

/// Watches the login until it ends, is cancelled, cannot be followed or runs too long; then
/// tells how it ended, its status checked where it ended well. However it ended, its forward is
/// closed before that is told, so a sign-in tried again finds the port free.
fn watching(
    mut child: Child,
    readers: Vec<std::thread::JoinHandle<()>>,
    driven: &Mutex<Driven>,
    outside: &Outside<'_>,
    forwarding: Option<&Forwarding>,
    tell: &Telling,
    check: impl FnOnce() -> Standing,
) {
    let close = || {
        if let Some(forwarding) = forwarding {
            forwarding.close();
        }
    };
    let started = Instant::now();
    let ended = loop {
        if outside.ends_it() {
            end(&mut child);
            close();
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
            close();
            tell.say_last(event);
            return;
        }
        // It was said to be over while it ran (a page purlis does not open): nothing more is
        // asked of it.
        if over {
            end(&mut child);
            close();
            return;
        }
        if started.elapsed() > LONGEST {
            end(&mut child);
            close();
            tell.say_last(Event::Failed {
                why: "the sign-in took too long, so purlis ended it".to_owned(),
            });
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    close();
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
    /// **The next step**, waiting at most `within` for it, which must be under
    /// [`UNDRIVEN_AFTER`]; `None` while there is none yet. After the last step, a driver that
    /// stopped telling says it failed.
    pub fn next(&self, within: Duration) -> Option<Event> {
        *self.asked.lock().unwrap_or_else(PoisonError::into_inner) = Instant::now();
        let events = self.events.lock().unwrap_or_else(PoisonError::into_inner);
        let event = events.recv_timeout(within);
        *self.asked.lock().unwrap_or_else(PoisonError::into_inner) = Instant::now();
        match event {
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
