// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Inbox's updates** (#1693, spec #1688, I-6, I-8, I-10): what happened that the person
//! may want to know and need not answer — a task that finished or failed, a doctor finding, a
//! chat that resumed, a sandbox change, a dispatch refused while nobody was there, a Smart close
//! that stopped, a report with nowhere to go, a commit refused, a report the person filed — kept **per machine for a day**
//! in purlis's data home and never in a project, so it survives a relaunch, is never committed
//! and is never sent.
//!
//! An update is not an ask: nothing waits on it. The window derives each one from the source
//! that already says it (the finished rows, the doctor's report, the chats a launch put back,
//! the sandbox's blocks, the away list, a Smart close's steps) and notes it here by a key of its
//! own, so the same update noted again is one update. What the person did with it is kept with
//! it: **read** (Mark all read) and **dismissed** (Dismiss, Dismiss all). A dismissed update is
//! kept, unseen, until its day is over, so its source saying it again does not bring it back.
//!
//! # Where and how
//!
//! `<data>/inbox/<project key>.json` (the key is the digest the sandbox's cache homes and the
//! network record are named by), one JSON array, rewritten whole under a lock on its folder,
//! 0600. A write lets go of what is older than [`KEPT_FOR_SECS`] and past [`AT_MOST_KEPT`].
//! A file this build cannot read, or one larger than [`MOST_BYTES`], reads as no updates and is
//! written over: an update is information, and losing a day of it loses nothing that waits.
//!
//! **What a chat named is data.** A chain names chats, and an update says what its source said
//! of them; each is held to a bound here ([`MOST_SAID`], [`MOST_NAME`], [`MOST_CHAIN`],
//! [`MOST_KEY`]), and the window draws it as text.

use std::io;
use std::path::{Path, PathBuf};

/// The updates' folder under purlis's data home.
pub const DIR: &str = "inbox";

/// How long an update is kept: a day (I-6).
pub const KEPT_FOR_SECS: u64 = 24 * 60 * 60;

/// The most updates one project keeps; the oldest go first.
pub const AT_MOST_KEPT: usize = 200;

/// The most characters an update says.
pub const MOST_SAID: usize = 400;

/// The most characters of one name in a chain.
pub const MOST_NAME: usize = 80;

/// The most names a chain keeps: the session first, the chat it is about last.
pub const MOST_CHAIN: usize = 8;

/// The most characters of a key.
pub const MOST_KEY: usize = 200;

/// The most bytes a project's file is read to: past every bound above, kept whole, with room.
pub const MOST_BYTES: u64 = 4 * 1024 * 1024;

/// What kind of thing happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A task finished: done, cancelled, or ended by the person.
    TaskDone,
    /// A task came to nothing: failed, blocked, ended without a report, or did not start.
    TaskFailed,
    /// The doctor found something.
    Doctor,
    /// A chat came back: resumed, or started fresh in its place.
    Resumed,
    /// A chat's sandbox refused something that is no ask.
    Sandbox,
    /// A dispatch was refused while nobody was at its chat (#1507).
    RefusedAway,
    /// A Smart close stopped without its record (SI-8f).
    SmartClose,
    /// A chat's report has nowhere to go: the chat that asked for it has gone (#1448, #1694).
    ReportUndelivered,
    /// purlis's `pre-commit` refused a commit a chat made (SQ-16, #1694).
    CommitRefused,
    /// purlis could not update the project's root for a chat, for a reason no chat can settle,
    /// so bringing the remote's commits in is the person's (qw348, #3877).
    RootUpdateRefused,
    /// A chat asked the person to take its browser over, and the browser ended first (#2015).
    BrowserAskEnded,
    /// A task ended with its question unanswered (#4556).
    QuestionClosed,
    /// A task's question could not be taken its route (#4557).
    QuestionFailed,
    /// A chat another chat started arrived in no pane, and was announced (#4759): a handoff,
    /// or a task that can't report asks.
    Arrived,
    /// The person filed a report a chat drafted (#5694): sent to purlis/purlis or to purlis's
    /// report service.
    ReportSent,
    /// A working chat may not be getting anywhere (#6014, ADR 0121 decision 19): drawn while it
    /// reads Stuck?, a Should-know item.
    Stuck,
    /// A chat's turn ended (T1-31, R-241): its finish row, which says the blocks still
    /// unresolved at its end (R-178), and is never a failure (R-180).
    TurnFinished,
    /// A chat's own run ended failed (T1-31, R-241).
    ChatFailed,
}

/// One update, as it is kept.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Update {
    /// What tells it from every other update of its project: its source's own name for it.
    pub key: String,
    pub kind: Kind,
    /// When it happened, by its source's own time where it has one (seconds since 1970).
    pub at: u64,
    /// The chat it is about, while that number means one.
    #[serde(default)]
    pub session: Option<u32>,
    /// Who it is about, the session first: names, as data.
    #[serde(default)]
    pub chain: Vec<String>,
    /// What happened, in one line: data, never markup.
    pub says: String,
    /// The person has read it (Mark all read).
    #[serde(default)]
    pub read: bool,
    /// The person put it away (Dismiss).
    #[serde(default)]
    pub dismissed: bool,
    /// The first-open baseline set it aside (#5648): it was already there the first time the
    /// project opened on this machine, so it is listed From the project, read.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub baselined: bool,
}

/// What the person did with updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settle {
    /// Mark read: drawn as read, and kept.
    Read,
    /// Dismiss: not drawn again.
    Dismissed,
}

/// One machine's updates, under a data home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    dir: PathBuf,
    /// The config home whose layout file says which projects this machine's window opened
    /// before ([`Self::with_layout`]); `None` where it is not asked.
    layout: Option<PathBuf>,
}

impl Store {
    /// The updates under the data home `data`.
    pub fn in_data(data: &Path) -> Self {
        Self {
            dir: data.join(DIR),
            layout: None,
        }
    }

    /// The same store, asking the layout file under the config home `config_root` whether this
    /// machine's window opened a project before ([`Self::first_open`]).
    #[must_use]
    pub fn with_layout(self, config_root: &Path) -> Self {
        Self {
            layout: Some(config_root.to_path_buf()),
            ..self
        }
    }

    /// The updates under this process's data home ([`crate::datahome::root`]), when there is
    /// one and it is not inside a project or a git work tree (ADR 0075 §6).
    pub fn here() -> Option<Self> {
        let data = crate::datahome::root()?;
        let store = crate::datahome::refusal(&data)
            .is_none()
            .then(|| Self::in_data(&data))?;
        Some(match crate::machine::config_root() {
            Some(config) => store.with_layout(&config),
            None => store,
        })
    }

    /// The file the project at `root` keeps its updates in.
    pub fn file(&self, root: &Path) -> PathBuf {
        self.dir
            .join(format!("{}.json", crate::sandbox::Homes::project_key(root)))
    }

    /// The project at `root`'s updates at `now`: kept, not dismissed, newest first.
    pub fn read(&self, root: &Path, now: u64) -> Vec<Update> {
        shown(&kept(self.everything(root), now))
    }

    /// Notes `noted` for the project at `root`: each whose key is not kept yet is added, unread;
    /// one already kept, dismissed or not, is left as it is. Answers [`Self::read`].
    ///
    /// **Under a fresh first-open baseline** ([`Self::first_open`]), one that happened before the
    /// project first opened here is noted read and baselined: it was already there.
    pub fn note(&self, root: &Path, noted: Vec<Update>, now: u64) -> io::Result<Vec<Update>> {
        let baseline = self.baseline(root).filter(|one| one.fresh);
        self.change(root, now, |all| {
            let mut changed = false;
            for one in noted {
                let mut one = bounded(one, now);
                if baseline.as_ref().is_some_and(|base| base.covers(&one)) {
                    one.read = true;
                    one.baselined = true;
                }
                if !within(one.at, now) || all.iter().any(|kept| kept.key == one.key) {
                    continue;
                }
                all.push(one);
                changed = true;
            }
            changed
        })
    }

    /// Settles the project at `root`'s updates named by `keys`, or every one where `keys` is
    /// `None`: read, or dismissed (and so read). Answers [`Self::read`].
    pub fn settle(
        &self,
        root: &Path,
        keys: Option<&[String]>,
        how: Settle,
        now: u64,
    ) -> io::Result<Vec<Update>> {
        self.change(root, now, |all| {
            let mut changed = false;
            for one in all.iter_mut().filter(|one| !one.dismissed) {
                if keys.is_some_and(|keys| !keys.contains(&one.key)) {
                    continue;
                }
                let was = (one.read, one.dismissed);
                one.read = true;
                one.dismissed |= how == Settle::Dismissed;
                changed |= was != (one.read, one.dismissed);
            }
            changed
        })
    }

    /// Every update the file holds, dismissed ones too; none where it cannot be read, or where
    /// it is larger than [`MOST_BYTES`]: no write of this store makes one so large.
    fn everything(&self, root: &Path) -> Vec<Update> {
        use std::io::Read as _;
        let Ok(file) = std::fs::File::open(self.file(root)) else {
            return Vec::new();
        };
        let mut text = Vec::new();
        if file.take(MOST_BYTES + 1).read_to_end(&mut text).is_err()
            || text.len() as u64 > MOST_BYTES
        {
            return Vec::new();
        }
        serde_json::from_slice(&text).unwrap_or_default()
    }

    /// Reads, lets `change` edit, and writes back what is kept, under a lock on the folder.
    /// Written only where something changed or something was let go of.
    fn change(
        &self,
        root: &Path,
        now: u64,
        change: impl FnOnce(&mut Vec<Update>) -> bool,
    ) -> io::Result<Vec<Update>> {
        crate::rewrite::create_dir_all(&self.dir)?;
        let _held = crate::rewrite::Lock::bounded(&self.dir)?;
        let file = self.file(root);
        let was = self.everything(root);
        let before = was.len();
        let mut all = kept(was, now);
        let let_go = all.len() != before;
        let changed = change(&mut all);
        all = kept(all, now);
        if changed || let_go {
            let text = serde_json::to_string(&all).map_err(io::Error::other)?;
            crate::rewrite::replace(
                &self.dir,
                &file,
                text.as_bytes(),
                crate::rewrite::Mode::Private,
            )?;
        }
        Ok(shown(&all))
    }
}

/// Whether something that happened at `then` is still kept at `now`.
fn within(then: u64, now: u64) -> bool {
    then.saturating_add(KEPT_FOR_SECS) > now
}

/// What of `all` is still kept at `now`, oldest first, at most [`AT_MOST_KEPT`].
fn kept(mut all: Vec<Update>, now: u64) -> Vec<Update> {
    all.retain(|one| within(one.at, now));
    all.sort_by_key(|one| one.at);
    let over = all.len().saturating_sub(AT_MOST_KEPT);
    all.drain(..over);
    all
}

/// What of `all` is drawn: not dismissed, newest first.
fn shown(all: &[Update]) -> Vec<Update> {
    let mut out: Vec<Update> = all.iter().filter(|one| !one.dismissed).cloned().collect();
    out.sort_by(|one, other| other.at.cmp(&one.at).then_with(|| one.key.cmp(&other.key)));
    out
}

/// `text` held to `most` characters.
fn cut(text: &str, most: usize) -> String {
    text.chars().take(most).collect()
}

/// `one` as it is kept: new, unread, its words held to their bounds, and no later than `now`.
fn bounded(one: Update, now: u64) -> Update {
    Update {
        key: cut(&one.key, MOST_KEY),
        kind: one.kind,
        at: one.at.min(now),
        session: one.session,
        chain: one
            .chain
            .iter()
            .take(MOST_CHAIN)
            .map(|name| cut(name, MOST_NAME))
            .collect(),
        says: cut(&one.says, MOST_SAID),
        read: false,
        dismissed: false,
        baselined: false,
    }
}

/// **Snooze all** (#5647, ADR 0119): each snoozed chat, by its session, with what its snooze
/// keeps ([`Snooze`]). A chat stays away until it holds an ask its record has not, or its
/// time comes ([`Store::release`]), whichever is first.
pub type Snoozed = std::collections::BTreeMap<u32, Snooze>;

/// The asks each chat holds now, by session and key: what Snooze all puts away, and what a
/// read of the asks compares a snoozed chat's record with.
pub type Held = std::collections::BTreeMap<u32, Vec<String>>;

/// **One snoozed chat's record**: the keys of the asks it held when it was snoozed, and when it
/// comes back by itself (#6018, ADR 0121 decision 14), in seconds since 1970; `None` where only
/// a new ask brings it back.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Snooze {
    pub asks: Vec<String>,
    pub until: Option<u64>,
}

/// A record written before #6018 keeps a chat's asks alone, and reads as snoozed with no time.
impl<'de> serde::Deserialize<'de> for Snooze {
    fn deserialize<D: serde::Deserializer<'de>>(read: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum Kept {
            Timed {
                asks: Vec<String>,
                until: Option<u64>,
            },
            Asks(Vec<String>),
        }
        Ok(match Kept::deserialize(read)? {
            Kept::Timed { asks, until } => Snooze { asks, until },
            Kept::Asks(asks) => Snooze { asks, until: None },
        })
    }
}

/// The most chats one project keeps snoozed; the lowest session numbers go first.
pub const AT_MOST_SNOOZED: usize = 200;

/// The most asks one snoozed chat's record keeps.
pub const MOST_SNOOZED_ASKS: usize = 100;

/// What the snooze record's file is called beside the updates' (`<project key>.snoozed.json`).
const SNOOZED: &str = "snoozed";

/// Whether chat `session`, holding the asks `asks` by key, is still away under `snoozed` at
/// `now`: it has a record whose time has not come, and every ask it holds is in it. The
/// prototype's rule (#5638), with #6018's time.
pub fn away(snoozed: &Snoozed, session: u32, asks: &[String], now: u64) -> bool {
    snoozed.get(&session).is_some_and(|had| {
        had.until.is_none_or(|until| now < until) && asks.iter().all(|ask| had.asks.contains(ask))
    })
}

/// **When the first snoozed chat of `snoozed` comes back by itself**, if any has a time: what
/// the window reads its asks again at, so they are counted and notified then.
pub fn first_back(snoozed: &Snoozed) -> Option<u64> {
    snoozed.values().filter_map(|one| one.until).min()
}

impl Store {
    /// The file the project at `root` keeps its snooze record in, beside its updates.
    pub fn snoozed_file(&self, root: &Path) -> PathBuf {
        self.beside(root, SNOOZED)
    }

    /// The project at `root`'s snoozed chats, as the record holds them; none where it cannot be
    /// read.
    pub fn snoozed(&self, root: &Path) -> Snoozed {
        self.read_beside(root, SNOOZED)
    }

    /// Snoozes each chat of `chats` with the asks it holds, until `until` at the latest where
    /// it is given, in place of any record it had. Answers [`Self::snoozed`].
    pub fn snooze(&self, root: &Path, chats: Held, until: Option<u64>) -> io::Result<Snoozed> {
        self.change_beside(root, SNOOZED, |kept: &mut Snoozed| {
            for (session, asks) in chats {
                let asks = asks
                    .iter()
                    .take(MOST_SNOOZED_ASKS)
                    .map(|ask| cut(ask, MOST_KEY))
                    .collect();
                kept.insert(session, Snooze { asks, until });
            }
            while kept.len() > AT_MOST_SNOOZED {
                kept.pop_first();
            }
            true
        })
    }

    /// Brings back the chats `sessions`, or every chat where it is `None`: Show them now, and
    /// Snooze all's Undo. Answers [`Self::snoozed`].
    pub fn wake(&self, root: &Path, sessions: Option<&[u32]>) -> io::Result<Snoozed> {
        self.change_beside(root, SNOOZED, |kept: &mut Snoozed| {
            let before = kept.len();
            kept.retain(|session, _| sessions.is_some_and(|woken| !woken.contains(session)));
            kept.len() != before
        })
    }

    /// Lets go of every snoozed chat that holds, in `held`, an ask it did not have when it was
    /// snoozed, and of every one whose time has come by `now`, named in `held` or not: it is
    /// back, with all its asks, and stays back. A chat `held` does not name, or one holding
    /// only asks it had, stays away until its time. Answers what stays away.
    pub fn release(&self, root: &Path, held: &Held, now: u64) -> io::Result<Snoozed> {
        let back = |kept: &Snoozed| {
            kept.iter()
                .filter(|(session, _)| {
                    let asks = held.get(session).map_or(&[][..], Vec::as_slice);
                    !away(kept, **session, asks, now)
                })
                .map(|(session, _)| *session)
                .collect::<Vec<_>>()
        };
        // Read first, unlocked: the window lists its asks often, and a chat comes back rarely.
        let kept = self.snoozed(root);
        if back(&kept).is_empty() {
            return Ok(kept);
        }
        self.change_beside(root, SNOOZED, |kept: &mut Snoozed| {
            let back = back(kept);
            kept.retain(|session, _| !back.contains(session));
            !back.is_empty()
        })
    }

    /// The file named `name` that the project at `root` keeps beside its updates:
    /// `<project key>.<name>.json`. Each per-project record of this store has one of its own,
    /// so the updates' day and bounds never touch it.
    fn beside(&self, root: &Path, name: &str) -> PathBuf {
        self.dir.join(format!(
            "{}.{name}.json",
            crate::sandbox::Homes::project_key(root)
        ))
    }

    /// The record `name` beside the project at `root`'s updates; its default where it is
    /// missing, cannot be read, or is larger than [`MOST_BYTES`].
    fn read_beside<T: serde::de::DeserializeOwned + Default>(&self, root: &Path, name: &str) -> T {
        use std::io::Read as _;
        let Ok(file) = std::fs::File::open(self.beside(root, name)) else {
            return T::default();
        };
        let mut text = Vec::new();
        if file.take(MOST_BYTES + 1).read_to_end(&mut text).is_err()
            || text.len() as u64 > MOST_BYTES
        {
            return T::default();
        }
        serde_json::from_slice(&text).unwrap_or_default()
    }

    /// Reads the record `name`, lets `change` edit it, and writes it back where it changed,
    /// under the same lock on the folder as the updates. Answers the record as kept.
    fn change_beside<T: serde::Serialize + serde::de::DeserializeOwned + Default>(
        &self,
        root: &Path,
        name: &str,
        change: impl FnOnce(&mut T) -> bool,
    ) -> io::Result<T> {
        crate::rewrite::create_dir_all(&self.dir)?;
        let _held = crate::rewrite::Lock::bounded(&self.dir)?;
        let mut record = self.read_beside(root, name);
        if change(&mut record) {
            let text = serde_json::to_string(&record).map_err(io::Error::other)?;
            crate::rewrite::replace(
                &self.dir,
                &self.beside(root, name),
                text.as_bytes(),
                crate::rewrite::Mode::Private,
            )?;
        }
        Ok(record)
    }
}

/// **The first-open baseline** (#5648, ADR 0119): the record, made the first time a project
/// opens on this machine, that what was already there counts as seen. Kept beside the updates
/// (`<project key>.baseline.json`), on this machine and never in the project. A project with a
/// record is never baselined again.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Baseline {
    /// When the project first opened on this machine, in seconds since 1970.
    pub at: u64,
    /// Whether that open set the past aside: false where this machine had a seen record for the
    /// project already (an existing user updating purlis), which loses no unread line.
    pub fresh: bool,
    /// The doctor's findings already there, by the causes the window dismisses them by, once it
    /// has read the first report after a fresh open ([`Store::set_aside_findings`]); `None`
    /// until then, so they are set aside once.
    #[serde(default)]
    pub findings: Option<Vec<String>>,
}

impl Baseline {
    /// Whether `one` was already there at this baseline: it happened before the first open.
    fn covers(&self, one: &Update) -> bool {
        one.at <= self.at
    }
}

/// The most findings a baseline keeps.
pub const MOST_FINDINGS: usize = 200;

/// What the baseline's file is called beside the updates' (`<project key>.baseline.json`).
const BASELINE: &str = "baseline";

impl Store {
    /// The file the project at `root` keeps its first-open baseline in, beside its updates.
    pub fn baseline_file(&self, root: &Path) -> PathBuf {
        self.beside(root, BASELINE)
    }

    /// The project at `root`'s first-open baseline; `None` before its first open here, or where
    /// the record cannot be read.
    pub fn baseline(&self, root: &Path) -> Option<Baseline> {
        self.read_beside(root, BASELINE)
    }

    /// **Whether this machine opened the project at `root` before** (story 4): any record it
    /// keeps of the project says so, and only a project with none is a first open. Its updates
    /// file (which stays, emptied, once its updates age out), a snooze record, anything of its
    /// sandbox told, or what the window's layout file keeps of it: an arrangement, a view's own
    /// or a dismissed Notice, which every person who used the project in a window has.
    fn opened_before(&self, root: &Path) -> bool {
        self.file(root).exists()
            || self.snoozed_file(root).exists()
            || crate::sandbox::local::told_any(root)
            || self.layout.as_deref().is_some_and(|config| {
                crate::windowprefs::keeps_project(config, &root.to_string_lossy())
            })
    }

    /// **The project at `root` opens** at `now`: its baseline, made where it has none. It is
    /// **fresh** only where this machine has no record of the project
    /// ([`Self::opened_before`]): then the teammate Notices' seen records are written as
    /// the project stands, so its hosts and Internet access presets are not told as new. Never
    /// a grant: a teammate's dispatch grant or browser site still waits on the person's answer.
    /// A project with a record keeps it as it is.
    pub fn first_open(&self, root: &Path, now: u64) -> io::Result<Baseline> {
        if let Some(kept) = self.baseline(root) {
            return Ok(kept);
        }
        let mut made = None;
        let kept = self.change_beside(root, BASELINE, |kept: &mut Option<Baseline>| {
            if kept.is_some() {
                return false;
            }
            let fresh = !self.opened_before(root);
            *kept = Some(Baseline {
                at: now,
                fresh,
                findings: None,
            });
            made = Some(fresh);
            true
        })?;
        if made == Some(true) {
            crate::sandbox::local::told_as_it_stands(root)?;
        }
        Ok(kept.unwrap_or(Baseline {
            at: now,
            fresh: false,
            findings: None,
        }))
    }

    /// **The doctor's first report after a fresh first open**: its findings, `keys`, were
    /// already there, and the window puts each away until it changes. Kept once: nothing is set
    /// aside where the baseline is not fresh or has its findings already. Answers
    /// [`Self::read`].
    pub fn set_aside_findings(
        &self,
        root: &Path,
        keys: &[String],
        now: u64,
    ) -> io::Result<Vec<Update>> {
        self.change_beside(root, BASELINE, |kept: &mut Option<Baseline>| {
            let Some(base) = kept
                .as_mut()
                .filter(|one| one.fresh && one.findings.is_none())
            else {
                return false;
            };
            let keys: Vec<String> = keys
                .iter()
                .take(MOST_FINDINGS)
                .map(|key| cut(key, MOST_KEY))
                .collect();
            base.findings = Some(keys);
            true
        })?;
        Ok(self.read(root, now))
    }
}

/// **What the person has seen** (#6002, ADR 0121 decisions 10 and 11): each ask, Notice and
/// update of the project the window says was seen, by a key the window makes of its own
/// (`ask:<key>`, `notice:<cause>`, `update:<key>`), with when it was first seen (seconds since
/// 1970). One not in it is **New**. Seen, not received: the window says it once a row has been
/// visible in an open view for about a second, or its chat has been on screen in a focused
/// window. Kept per machine, beside the updates; nothing else reads or writes it.
pub type Seen = std::collections::BTreeMap<String, u64>;

/// The most keys one project keeps seen; what was seen longest ago goes first. An ask, a Notice
/// or an update stands far fewer than this at once, so what goes is long answered or gone.
pub const AT_MOST_SEEN: usize = 1000;

/// What the seen record's file is called beside the updates' (`<project key>.seen.json`).
const SEEN: &str = "seen";

impl Store {
    /// The file the project at `root` keeps its seen record in, beside its updates.
    pub fn seen_file(&self, root: &Path) -> PathBuf {
        self.beside(root, SEEN)
    }

    /// The project at `root`'s seen record; nothing seen where it cannot be read.
    pub fn seen(&self, root: &Path) -> Seen {
        self.read_beside(root, SEEN)
    }

    /// Notes each of `keys` seen at `now`: one seen before keeps when it was first seen, and
    /// nothing is written where every one was. Answers [`Self::seen`].
    pub fn see(&self, root: &Path, keys: &[String], now: u64) -> io::Result<Seen> {
        let keys: Vec<String> = keys.iter().map(|key| cut(key, MOST_KEY)).collect();
        // Read first, unlocked: the window says what it sees often, and most of it was seen.
        let kept = self.seen(root);
        if keys.iter().all(|key| kept.contains_key(key)) {
            return Ok(kept);
        }
        self.change_beside(root, SEEN, |kept: &mut Seen| {
            let mut changed = false;
            for key in keys {
                if let std::collections::btree_map::Entry::Vacant(at) = kept.entry(key) {
                    at.insert(now);
                    changed = true;
                }
            }
            let over = kept.len().saturating_sub(AT_MOST_SEEN);
            if over > 0 {
                let mut oldest: Vec<(u64, String)> =
                    kept.iter().map(|(key, at)| (*at, key.clone())).collect();
                oldest.sort();
                for (_, key) in oldest.into_iter().take(over) {
                    kept.remove(&key);
                }
            }
            changed
        })
    }

    /// Lets go of every key seen that starts with `prefix` and is not in `standing`: an ask
    /// answered, or gone, is forgotten, so the same ask asked again is New. Nothing is written
    /// where every one still stands. Answers [`Self::seen`].
    pub fn forget(&self, root: &Path, prefix: &str, standing: &[String]) -> io::Result<Seen> {
        let gone = |kept: &Seen| {
            kept.keys()
                .filter(|key| key.starts_with(prefix) && !standing.contains(key))
                .cloned()
                .collect::<Vec<_>>()
        };
        let kept = self.seen(root);
        if gone(&kept).is_empty() {
            return Ok(kept);
        }
        self.change_beside(root, SEEN, |kept: &mut Seen| {
            let gone = gone(kept);
            for key in &gone {
                kept.remove(key);
            }
            !gone.is_empty()
        })
    }
}

#[cfg(test)]
#[path = "inboxupdates_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "inboxupdates_snoozed_tests.rs"]
mod snoozed_tests;

#[cfg(test)]
#[path = "inboxupdates_baseline_tests.rs"]
mod baseline_tests;

#[cfg(test)]
#[path = "inboxupdates_seen_tests.rs"]
mod seen_tests;
