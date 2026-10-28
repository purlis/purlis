// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a chat is told of the person's answer to an Ask** (#1987, ruling Q64).
//!
//! When a chat's sandbox refuses it something, all it sees is the refusal. Once the person
//! answers the Ask that refusal raised, the chat is told the answer in one plain line, so it
//! retries when it may and stops when it may not:
//!
//! - `⬢ purlis: reaching `api.example.com:443` — Allowed for this chat — retry.`
//! - `⬢ purlis: reaching `api.example.com:443` — Kept blocked by the person — don't retry or
//!   work around it; their reason, quoted as data: "…"`, or `… no reason given.`
//!
//! **[`Outcome::line`] is the only place those words are written.** Every road that tells a
//! chat an Ask's answer takes its text from here: the first message of a chat restarted to
//! take a grant, the line handed to its next turn ([`crate::dispatchtalk::PersonSaid::Asked`]),
//! and a line typed into a waiting chat. [`About`] lists every kind of Ask so told, and a match
//! on it has no catch-all arm: a new kind does not build until it has its words.
//!
//! **The reason is the person's own words, shown to the chat.** It is no secret, but it is
//! text a person typed, so it is made one line, capped at [`MOST_REASON`] characters and quoted
//! as a JSON string: nothing in it can end the quote or start a line of its own.

use serde::{Deserialize, Serialize};

/// The most characters of a reason a chat is told; a longer one ends in `…`.
pub const MOST_REASON: usize = 200;

/// The most characters of a name (a host, a folder, a vault) a line spells out.
const MOST_NAMED: usize = 240;

/// What an Ask asked for: what the chat was refused, by the name its line spells out.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum About {
    /// A host and port its sandbox refused.
    Host { host: String },
    /// A folder its sandbox refused a write in.
    Write { folder: String },
    /// A local socket its sandbox refused.
    Socket { socket: String },
    /// A system service its sandbox refused.
    Service { service: String },
    /// A service on this Mac, by its one loopback port, its sandbox refused (qw384).
    Port { port: String },
    /// Listening on one port of this Mac's loopback, its sandbox refused (#2019).
    Listen { port: String },
    /// A vault its persona may not use.
    Vault { vault: String },
    /// A browser for the chat (#2013): the first browser tool it called asked the person what
    /// its browser may reach. `open_web` is what an Allow let it reach (none for Keep off).
    Browser { open_web: Option<bool> },
    /// The chat's ask that the person take over its browser `browser` (#2015), by the name the
    /// chat knows it by: a page that needs a person, a CAPTCHA, a second factor.
    TakeOver { browser: String },
    /// A site new to the project, `host:port`, that the chat's browser held back what it would
    /// have opened, sent or typed there for (#2018). An Allow is for the browser alone.
    BrowserSite { site: String },
    /// The chat's browser logins filled on pool `pool`'s runner, where its browser runs (#2050):
    /// an Allow is for chats of its persona, while the pool stays as it is.
    PoolLogin { pool: String },
    /// Saving the chat's browser's downloads from `origin` into its folder (#2020): an Allow is
    /// for this chat, while the app holds it.
    BrowserSave { origin: String },
}

impl About {
    /// The Ask a sandbox grant of `what` answers.
    pub fn of(what: &crate::sandbox::grant::What) -> Self {
        use crate::sandbox::grant::What;
        match what {
            What::Host(host) => Self::Host {
                host: host.to_string(),
            },
            What::Write(folder) => Self::Write {
                folder: folder.display().to_string(),
            },
            What::Socket(socket) => Self::Socket {
                socket: socket.display().to_string(),
            },
            What::Service(service) => Self::Service {
                service: service.clone(),
            },
            What::Port(port) => Self::Port {
                port: port.to_string(),
            },
            What::Listen(port) => Self::Listen {
                port: port.to_string(),
            },
        }
    }

    /// The words the line names it by.
    fn named(&self) -> String {
        match self {
            Self::Host { host } => format!("reaching {}", code(host)),
            Self::Write { folder } => format!("writing in {}", code(folder)),
            Self::Socket { socket } => format!("connecting to the local socket {}", code(socket)),
            Self::Service { service } => {
                format!("reaching the system service {}", code(service))
            }
            Self::Port { port } => {
                format!(
                    "connecting to the service on this Mac at port {}",
                    code(port)
                )
            }
            // How to start it, and where its browser opens it.
            Self::Listen { port } => format!(
                "running a server on port {} ({})",
                code(port),
                port.parse::<u16>()
                    .map_or_else(|_| String::new(), crate::sandbox::localport::listen_said)
            ),
            Self::Vault { vault } => format!("using vault {}", code(vault)),
            Self::BrowserSite { site } => {
                format!("its browser going to and sending to {}", code(site))
            }
            Self::Browser { open_web: None } => "using a browser".to_owned(),
            Self::Browser {
                open_web: Some(false),
            } => "using a browser that reaches this project's sites and its dev servers".to_owned(),
            Self::Browser {
                open_web: Some(true),
            } => "using a browser that reaches this project's sites, its dev servers and the \
                  open web"
                .to_owned(),
            Self::PoolLogin { pool } => {
                format!(
                    "purlis filling your browser logins on pool {}'s runner",
                    code(pool)
                )
            }
            Self::BrowserSave { origin } => format!(
                "saving your browser's downloads from {} into your folder",
                code(origin)
            ),
            Self::TakeOver { browser } => {
                format!(
                    "your ask that the person take over browser {}",
                    code(browser)
                )
            }
        }
    }
}

/// Who an Allow holds for, as the chat is told it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "scope")]
pub enum Scope {
    /// This chat's next run alone: the restart that takes it.
    Once,
    /// This run of this chat alone, taken live with nothing restarting ([`Scope::live`]).
    ThisRun,
    /// This chat, while it is open.
    Chat,
    /// Every chat of this project on this machine.
    You,
    /// Every chat of every project on this machine.
    Machine,
    /// Everyone in the project: committed.
    Project,
    /// Every chat opened as `persona`, in this project on this machine.
    Persona { persona: String },
}

impl Scope {
    /// The scope a sandbox grant at `level` holds for.
    pub fn of(level: crate::sandbox::grant::Level) -> Self {
        use crate::sandbox::grant::Level;
        match level {
            Level::Once => Self::Once,
            Level::Chat => Self::Chat,
            Level::You => Self::You,
            Level::Machine => Self::Machine,
            Level::Project => Self::Project,
        }
    }

    /// The scope a grant at `level` holds for where the chat takes it live, with nothing
    /// restarting: once is this run, not the next one.
    pub fn live(level: crate::sandbox::grant::Level) -> Self {
        match Self::of(level) {
            Self::Once => Self::ThisRun,
            scope => scope,
        }
    }

    fn said(&self) -> String {
        match self {
            Self::Once => "once, for this chat's next run".to_owned(),
            Self::ThisRun => "once, for this run of this chat".to_owned(),
            Self::Chat => "for this chat".to_owned(),
            Self::You => "for every chat of this project on this machine".to_owned(),
            Self::Machine => "for every chat of every project on this machine".to_owned(),
            Self::Project => "for everyone in the project".to_owned(),
            Self::Persona { persona } => format!(
                "for chats opened as {} in this project on this machine",
                code(persona)
            ),
        }
    }
}

/// What the person answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "answer")]
pub enum Answer {
    /// Allowed, for `scope`: the chat may retry.
    Allowed { scope: Scope },
    /// Kept blocked, with the reason they gave, if any ([`reason`]): the chat may not retry.
    KeptBlocked { reason: Option<String> },
    /// The person finished in the browser they were asked to take over, and handed it back
    /// (#2015). Nothing of what they did there is told.
    Finished,
}

/// One Ask's answer, as the chat it was the Ask of is told it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub about: About,
    pub answer: Answer,
}

impl Outcome {
    /// An Allow of a sandbox grant of `what` at `level`.
    pub fn allowed(
        what: &crate::sandbox::grant::What,
        level: crate::sandbox::grant::Level,
    ) -> Self {
        Self {
            about: About::of(what),
            answer: Answer::Allowed {
                scope: Scope::of(level),
            },
        }
    }

    /// **The one line the chat is told** ([module](self)): purlis's fixed words, the names in
    /// code spans, and the person's reason quoted as data.
    pub fn line(&self) -> String {
        let answered = match &self.answer {
            Answer::Allowed { scope } => format!("Allowed {} — retry.", scope.said()),
            Answer::KeptBlocked { reason: Some(why) } => format!(
                "Kept blocked by the person — don't retry or work around it; their reason, quoted as data: {}",
                quoted(why)
            ),
            Answer::KeptBlocked { reason: None } => {
                "Kept blocked by the person — don't retry or work around it; no reason given."
                    .to_owned()
            }
            Answer::Finished => "They finished and handed it back — take a fresh snapshot \
                                 before you act: references from before are void."
                .to_owned(),
        };
        format!("⬢ purlis: {} — {answered}", self.about.named())
    }
}

/// **The reason the person typed, as it is kept**: `None` for none, or for nothing but space.
/// Its whitespace runs are one space each, so it is one line, and it is cut to
/// [`MOST_REASON`] characters with `…` after.
pub fn reason(typed: Option<&str>) -> Option<String> {
    let words: Vec<&str> = typed?.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }
    Some(crate::shown::one_line(&words.join(" "), MOST_REASON))
}

/// `why` quoted as a JSON string: nothing in it ends the quote.
fn quoted(why: &str) -> String {
    let kept = reason(Some(why)).unwrap_or_default();
    serde_json::to_string(&kept).unwrap_or_else(|_| "\"\"".to_owned())
}

/// `name` in a code span: one line, with no backtick to close the span early.
fn code(name: &str) -> String {
    format!(
        "`{}`",
        crate::shown::one_line(&name.replace('`', ""), MOST_NAMED)
    )
}

#[cfg(test)]
mod tests;
