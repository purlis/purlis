// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Proposed changes the person applies** (#1975, the operator's rulings PC-1..PC-14 of
//! 2026-10-14): the window's half of [`purlis_core::proposed`].
//!
//! A chat's edit of purlis's own configuration reaches the app as a proposal
//! (`purlis_core::hookwire::Ask::Propose`): the file, the base the change was worked out
//! against, and the change (the tool's own edits, or a whole new content). Here it is held for
//! the person in the asks registry (`crate::asking`), drawn on the chat's tab and in the Inbox:
//! the change shown whole, **Apply** and **Dismiss**.
//!
//! - **What is shown and what is written are the app's own** (PC-3): the file is read here, only
//!   as the one regular file its name says, the change is made to it here, and Apply writes
//!   exactly that, as the person, only while the file is still what the proposal was worked out
//!   against ([`purlis_core::proposed::write_pinned`]). Nothing the chat sent says what the file
//!   holds.
//! - **Stale** (PC-8): where the file changed since, Apply refuses, the proposal offers Dismiss
//!   alone, and the chat is told to read it again and propose again. Never merged.
//! - **One a file a chat** (PC-9): a newer proposal of a chat's replaces its older one for the
//!   same file; two chats' proposals are both held, and applying one makes the other stale.
//! - **The chat hears fixed words** (PC-10): applied, dismissed or stale, on its next turn
//!   (`purlis_core::dispatchtalk::Talk::proposal_word`). Its proposals end with it.
//! - **Recorded** (PC-4): every Apply is audited before the file is written.
//! - **Apply allows nothing** (PC-5) and there is **no "always apply"** (PC-13); policy's
//!   `sandbox.proposed-changes = "off"` holds none (PC-14).
//! - **Shown whole, or not applied here** (PC-7): a change too long for a row says so and
//!   offers Open, its own comparison tab with Apply, where the lines that changed are marked
//!   and the rest folded (#2121), and Dismiss.
//!
//! In memory only: a relaunch starts with none, and the chat's next edit proposes again.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use purlis_core::dispatchtalk::ProposalWord;
use purlis_core::hookwire::{Answer, ProposeAsk, ProposeLimitAsk};
use purlis_core::poollimits::LimitId;
use purlis_core::proposed::{self, Change, NotMade};

use crate::planes::{Held, PlaneId, Planes};

/// The most proposals one chat has waiting at once: one more is not taken.
pub const AT_MOST_PER_CHAT: usize = 8;

/// **The session of a change purlis itself proposes** (#3467): no chat's, so no chat is told
/// of it, and no chat's ask can name it (a chat's must be one this app has open). Its file is
/// judged by [`proposed::proposable_by_purlis`].
pub const BY_PURLIS: u32 = 0;

/// One proposal that waits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    /// Its number in the project, for the window's answer.
    pub id: u32,
    pub session: u32,
    /// The file, relative to the project, as [`proposed::proposable`] took it.
    pub path: String,
    pub base: Option<String>,
    pub change: Change,
    /// When it began to wait, in seconds since 1970.
    pub since: u64,
    /// The file changed since it was proposed: Dismiss alone (PC-8).
    pub stale: bool,
    /// **A value for a pool's limit, in place of a change to a file** (#4284): [`Self::path`] is
    /// then its name in words, and [`Self::change`] is not read.
    pub limit: Option<LimitChange>,
}

/// **A value proposed for a pool's limit** (#4284, the person's ruling of 2026-11-01 on PC-1).
/// Only that value can change: Apply saves it through
/// [`purlis_core::runners::set_pool_limit`], and no part of the file travels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitChange {
    /// This machine's config home, as the app holds it, where the pool is kept.
    pub config: PathBuf,
    pub pool: String,
    pub id: LimitId,
    pub value: Option<u64>,
    /// What the person had set when it was proposed: another value at Apply makes it stale.
    pub was: Option<u64>,
    /// The file held a value this limit can't be: shown, since Apply replaces it (#4381).
    pub file_refused: bool,
}

/// The proposals of a project's chats, until the person answers.
#[derive(Debug, Default)]
pub struct Proposals {
    held: Mutex<(u32, Vec<Proposal>)>,
    /// Held by an Apply from its read of the file to its rename ([`apply_in`]).
    applying: Mutex<()>,
}

impl Proposals {
    fn lock(&self) -> std::sync::MutexGuard<'_, (u32, Vec<Proposal>)> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Holds chat `session`'s proposal to `path`: in place of its older one for that file
    /// (PC-9), or none where the chat has [`AT_MOST_PER_CHAT`] waiting already.
    pub fn note(
        &self,
        session: u32,
        path: &str,
        base: Option<String>,
        change: Change,
        now: u64,
    ) -> Option<u32> {
        self.hold(session, path, base, change, None, now)
    }

    /// Holds chat `session`'s value for a pool's limit, under the rules of [`Self::note`].
    pub fn note_limit(
        &self,
        session: u32,
        path: &str,
        limit: LimitChange,
        now: u64,
    ) -> Option<u32> {
        let nothing = Change::Whole {
            content: String::new(),
        };
        self.hold(session, path, None, nothing, Some(limit), now)
    }

    fn hold(
        &self,
        session: u32,
        path: &str,
        base: Option<String>,
        change: Change,
        limit: Option<LimitChange>,
        now: u64,
    ) -> Option<u32> {
        let mut held = self.lock();
        let (next, list) = &mut *held;
        list.retain(|one| !(one.session == session && one.path == path));
        if list.iter().filter(|one| one.session == session).count() >= AT_MOST_PER_CHAT {
            return None;
        }
        *next = next.wrapping_add(1).max(1);
        let id = *next;
        list.push(Proposal {
            id,
            session,
            path: path.to_owned(),
            base,
            change,
            since: now,
            stale: false,
            limit,
        });
        Some(id)
    }

    /// The files of every proposal of `session`'s that waits, read under the lock alone.
    pub fn paths_of(&self, session: u32) -> Vec<String> {
        self.lock()
            .1
            .iter()
            .filter(|one| one.session == session)
            .map(|one| one.path.clone())
            .collect()
    }

    /// The project's Apply lock ([`apply_in`]).
    pub fn applying(&self) -> &Mutex<()> {
        &self.applying
    }

    /// Every proposal that waits, oldest first.
    pub fn every(&self) -> Vec<Proposal> {
        self.lock().1.clone()
    }

    /// Proposal `id`, where it waits.
    pub fn get(&self, id: u32) -> Option<Proposal> {
        self.lock().1.iter().find(|one| one.id == id).cloned()
    }

    /// Takes proposal `id` out: it waits no more.
    pub fn take(&self, id: u32) -> Option<Proposal> {
        let mut held = self.lock();
        let at = held.1.iter().position(|one| one.id == id)?;
        Some(held.1.remove(at))
    }

    /// Marks proposal `id` stale. Answers whether it was not already.
    pub fn stale(&self, id: u32) -> bool {
        let mut held = self.lock();
        match held.1.iter_mut().find(|one| one.id == id) {
            Some(one) if !one.stale => {
                one.stale = true;
                true
            }
            _ => false,
        }
    }

    /// Every other proposal to `path` is stale now (PC-9): the chats newly told.
    pub fn others_stale(&self, path: &str) -> Vec<u32> {
        let mut held = self.lock();
        let mut told = Vec::new();
        for one in held
            .1
            .iter_mut()
            .filter(|one| one.path == path && !one.stale)
        {
            one.stale = true;
            told.push(one.session);
        }
        told
    }

    /// Chat `session` has closed: its proposals go with it (PC-10).
    pub fn forget(&self, session: u32) {
        self.lock().1.retain(|one| one.session != session);
    }
}

/// What the chat is told once its proposal is held: the person has it, and it waits.
pub const HELD_SAID: &str = "shown to the person as a proposed change: only they apply it, in \
                             the window. Wait for their answer.";

/// What a chat is told where the file changed since it read it.
pub const CHANGED_SINCE: &str = "the file changed since this chat read it, so nothing was \
                                 proposed: read it again, and propose the change again.";

/// **Chat `ask.chat` proposed a change** to one of purlis's own configuration files: held for
/// the person, or why not. **Nothing is written here, whatever is asked.**
///
/// The chat the line names is the one whose token it carries, which the listener checks before
/// this is asked; each chat is held to the rate its brokered writes are, so a chat cannot fill
/// the person's Inbox by proposing.
pub fn asked(held: &Held, ask: &ProposeAsk) -> Answer {
    let no = |why: String| Answer::No { why };
    if !held
        .chats()
        .open_now()
        .iter()
        .any(|open| open.session == ask.chat)
    {
        return no(format!("chat {} is not one this app has open", ask.chat));
    }
    if let Err(why) = held.brokered().allow(ask.chat, std::time::Instant::now()) {
        return no(why);
    }
    // A plane skill's file is kept as a draft skill, in the Skill studio alone (SK-9, #2772).
    if let Some(answer) = crate::skilldrafts::drafted(held, ask) {
        return answer;
    }
    match asked_in(
        held.root(),
        held.proposals(),
        ask,
        crate::sandboxing::now_secs(),
    ) {
        Ok(()) => {
            held.chats().asks_may_have_moved();
            Answer::Said {
                lines: vec![purlis_core::repocmd::Say::Done(HELD_SAID.into())],
                code: 0,
            }
        }
        Err(why) => no(why),
    }
}

/// [`asked`]'s judgement of `ask` in the project at `root`, held in `proposals` at `now`.
pub fn asked_in(
    root: &Path,
    proposals: &Proposals,
    ask: &ProposeAsk,
    now: u64,
) -> Result<(), String> {
    if let Some(why) = purlis_core::sandbox::policy::Locks::of(root).proposed_changes_refused() {
        return Err(why);
    }
    // No chat proposes as purlis itself, whose changes are judged by a wider rule.
    if ask.chat == BY_PURLIS {
        return Err(format!("chat {BY_PURLIS} is no chat that proposes"));
    }
    if ask.change.carried() > proposed::MOST_BYTES {
        return Err(proposed::UNPREPARED.to_owned());
    }
    let rel = proposed::proposable(root, &root.join(&ask.path))?;
    // Made here, against the file as this app reads it, before it is held: one that does not
    // apply is never shown.
    match proposed::prepare(root, &rel, Some(ask.base.as_deref()), &ask.change) {
        Ok(_) => {}
        Err(NotMade::Stale) => return Err(CHANGED_SINCE.to_owned()),
        Err(NotMade::Unprepared) => return Err(proposed::UNPREPARED.to_owned()),
    }
    let path = rel.display().to_string();
    proposals
        .note(ask.chat, &path, ask.base.clone(), ask.change.clone(), now)
        .map(|_| ())
        .ok_or_else(|| {
            format!(
                "this chat has {AT_MOST_PER_CHAT} proposed changes waiting already: wait for the \
                 person's answers before proposing another"
            )
        })
}

/// **Chat `ask.chat` proposed a value for a pool's limit**: held for the person, or why not.
/// Held to what [`asked`] holds a chat to. **Nothing is written here, whatever is asked.**
pub fn asked_limit(held: &Held, ask: &ProposeLimitAsk) -> Answer {
    let no = |why: String| Answer::No { why };
    if !held
        .chats()
        .open_now()
        .iter()
        .any(|open| open.session == ask.chat)
    {
        return no(format!("chat {} is not one this app has open", ask.chat));
    }
    if let Err(why) = held.brokered().allow(ask.chat, std::time::Instant::now()) {
        return no(why);
    }
    let Some(config) = held.config() else {
        return no("There is no config home on this machine to keep a pool in.".to_owned());
    };
    match limit_asked_in(
        held.root(),
        config,
        held.proposals(),
        ask,
        crate::sandboxing::now_secs(),
    ) {
        Ok(()) => {
            held.chats().asks_may_have_moved();
            Answer::Said {
                lines: vec![purlis_core::repocmd::Say::Done(HELD_SAID.into())],
                code: 0,
            }
        }
        Err(why) => no(why),
    }
}

/// [`asked_limit`]'s judgement of `ask`, for the project at `root` and the machine at `config`.
pub fn limit_asked_in(
    root: &Path,
    config: &Path,
    proposals: &Proposals,
    ask: &ProposeLimitAsk,
    now: u64,
) -> Result<(), String> {
    // PC-14: a project where proposals are off holds none of these either.
    if let Some(why) = purlis_core::sandbox::policy::Locks::of(root).proposed_changes_refused() {
        return Err(why);
    }
    if ask.chat == BY_PURLIS {
        return Err(format!("chat {BY_PURLIS} is no chat that proposes"));
    }
    let limit = purlis_core::poollimits::of_key(&ask.key).ok_or_else(|| {
        format!(
            "a pool has no limit named {}",
            purlis_core::shown::short(&ask.key)
        )
    })?;
    let held = purlis_core::runners::limit_may_be_set(config, &ask.pool, limit.id, ask.value)
        .map_err(|why| why.to_string())?;
    let was = held.chosen.get(limit.id);
    let file_refused = held.refused.iter().any(|one| one.limit == limit.id);
    if was == ask.value && !file_refused {
        return Err(format!("{} is that already", limit.plain));
    }
    let path = format!("pool {}: {}", ask.pool, limit.plain);
    proposals
        .note_limit(
            ask.chat,
            &path,
            LimitChange {
                config: config.to_path_buf(),
                pool: ask.pool.clone(),
                id: limit.id,
                value: ask.value,
                was,
                file_refused,
            },
            now,
        )
        .map(|_| ())
        .ok_or_else(|| {
            format!(
                "this chat has {AT_MOST_PER_CHAT} proposed changes waiting already: wait for the \
                 person's answers before proposing another"
            )
        })
}

/// A limit's value as its proposal shows it, in words: one line.
fn limit_line(change: &LimitChange, value: Option<u64>) -> String {
    let limit = change.id.limit();
    let said = match value {
        Some(value) => limit.unit.said(value),
        None => format!("{} (purlis's default)", limit.unit.said(limit.default)),
    };
    format!("{}: {said}\n", limit.plain)
}

/// [`Now`] of a limit's proposal, as its machine holds the pool: judged again.
pub fn limit_now(change: &LimitChange) -> Now {
    match purlis_core::runners::limit_may_be_set(
        &change.config,
        &change.pool,
        change.id,
        change.value,
    ) {
        Ok(held) if held.chosen.get(change.id) != change.was => Now::Stale,
        Ok(_) if change.file_refused => Now::Made {
            before: format!(
                "{}: a value in the pool's file that can't be used\n",
                change.id.limit().plain
            ),
            after: limit_line(change, change.value),
        },
        Ok(_) => Now::Made {
            before: limit_line(change, change.was),
            after: limit_line(change, change.value),
        },
        Err(_) => Now::Unprepared,
    }
}

/// What a proposal is now, as its row and its Notice draw it: the change made to the file as
/// it is, or why it cannot be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Now {
    /// The file before and after: shown whole where it fits a row.
    Made { before: String, after: String },
    /// The file changed since, or the proposal was marked so.
    Stale,
    /// It cannot be made at all any more.
    Unprepared,
}

/// Proposal `one` made to its file as it is now, in the project at `root`.
pub fn now_of(root: &Path, one: &Proposal) -> Now {
    if one.stale {
        return Now::Stale;
    }
    if let Some(change) = &one.limit {
        return limit_now(change);
    }
    match proposed::prepare(
        root,
        Path::new(&one.path),
        Some(one.base.as_deref()),
        &one.change,
    ) {
        Ok(made) => Now::Made {
            before: made.before.unwrap_or_default(),
            after: made.after,
        },
        Err(NotMade::Stale) => Now::Stale,
        Err(NotMade::Unprepared) => Now::Unprepared,
    }
}

/// Every proposal of project `held` that waits, each with what it is now. One found stale here
/// is marked so, and its chat told.
pub fn waiting(held: &Held) -> Vec<(Proposal, Now)> {
    let root = held.root();
    held.proposals()
        .every()
        .into_iter()
        .map(|one| {
            let now = now_of(root, &one);
            if now == Now::Stale && held.proposals().stale(one.id) {
                tell(held, one.session, stale_word(&one));
            }
            (one, now)
        })
        .collect()
}

/// What a chat is told of `one` gone stale.
fn stale_word(one: &Proposal) -> ProposalWord {
    let path = one.path.clone();
    if one.limit.is_some() {
        ProposalWord::LimitStale { path }
    } else {
        ProposalWord::Stale { path }
    }
}

/// Tells chat `session` `word` on its next turn.
fn tell(held: &Held, session: u32, word: ProposalWord) {
    if session == BY_PURLIS {
        return;
    }
    held.tasks().ledger().talk.proposal_word(session, word);
}

/// **A proposed change whole, as its own tab draws it** (PC-7): the file before and after, and
/// whether Apply is offered there, or why it is not shown.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ProposedWhole {
    pub path: String,
    /// The chat that proposed it.
    pub session: u32,
    /// The file as the app reads it now, and as the change makes it: drawn whole. Empty where
    /// it is not shown ([`Self::why`]).
    pub before: String,
    pub after: String,
    /// git's hunks between the two sides, as drawn: the lines the merge view marks (#2121).
    pub hunks: Vec<crate::piecefiles::GitHunk>,
    /// Whether Apply is offered: shown whole, and it still applies.
    pub applies: bool,
    /// Why it is not shown, or not applied, in a sentence.
    pub why: Option<String>,
}

/// **Proposal `id` whole, for its own tab** (PC-7). Window-only: what a configuration file
/// holds is the person's to read.
#[tauri::command]
#[specta::specta]
pub async fn proposed_change(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    id: u32,
) -> Result<ProposedWhole, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading a proposed change", move || {
        let one = held.proposals().get(id).ok_or(WAITS_NO_MORE)?;
        Ok(whole_of(&one, &now_of(held.root(), &one)))
    })
    .await
}

/// `one` as its tab draws it, made as `now` says.
pub fn whole_of(one: &Proposal, now: &Now) -> ProposedWhole {
    let empty = |why: &str| ProposedWhole {
        path: one.path.clone(),
        session: one.session,
        before: String::new(),
        after: String::new(),
        hunks: Vec::new(),
        applies: false,
        why: Some(why.to_owned()),
    };
    match now {
        Now::Stale => empty(STALE_SAID),
        Now::Unprepared => empty(proposed::UNPREPARED),
        Now::Made { before, after } => {
            let shown = crate::askwrites::proposed(&one.path, before, after);
            let whole = matches!(shown, crate::askwrites::WritesShown::Whole { .. });
            if whole || crate::askwrites::proposed_too_long(before, after) {
                let before = purlis_core::harness::asked::edit::drawn(before);
                let after = purlis_core::harness::asked::edit::drawn(after);
                let hunks = lines_changed(&before, &after);
                ProposedWhole {
                    path: one.path.clone(),
                    session: one.session,
                    before,
                    after,
                    hunks,
                    applies: true,
                    why: None,
                }
            } else {
                let crate::askwrites::WritesShown::NotShown { why } = shown else {
                    unreachable!("a change not shown whole is said not shown")
                };
                empty(&why)
            }
        }
    }
}

/// The lines that changed between `before` and `after` as the tab draws them: git's hunks, by
/// the one line diff every comparison uses (#2121), so the merge view marks those lines and
/// folds the unchanged stretches between them.
pub(crate) fn lines_changed(before: &str, after: &str) -> Vec<crate::piecefiles::GitHunk> {
    purlis_core::files::line_hunks(before, after)
        .into_iter()
        .map(crate::piecefiles::GitHunk::from)
        .collect()
}

/// Why an answer found nothing to answer: answered elsewhere, or its chat closed.
const WAITS_NO_MORE: &str = "That proposed change waits no more: it was answered already, or \
                             its chat closed. Nothing was written.";

/// **Apply** on a proposed change (PC-3): written exactly as the window showed it, as the
/// person, only while the file is still what it was proposed against; audited first (PC-4).
/// Window-only.
#[tauri::command]
#[specta::specta]
pub async fn apply_proposed_change(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    id: u32,
    from: From,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("applying a proposed change", move || {
        applied(&held, id, from)
    })
    .await
}

/// **Where Apply was pressed** (PC-7): a row (the Inbox's, a tab's Notice), which shows a change
/// whole only up to its length, or the change's own tab, which shows it whole however long.
/// What Apply writes is judged again by the rules of where it was shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum From {
    Row,
    Tab,
}

/// [`apply_proposed_change`]'s work, for project `held`.
pub fn applied(held: &Held, id: u32, from: From) -> Result<String, String> {
    let one = held.proposals().get(id).ok_or(WAITS_NO_MORE)?;
    let root = held.root();
    let persona = held
        .chats()
        .recorded_chat(one.session)
        .and_then(|chat| chat.persona);
    let audit =
        |applied: &proposed::Applied<'_>| held.hooks().record_proposed(root, one.session, applied);
    let stale_said = if one.limit.is_some() {
        LIMIT_STALE_SAID
    } else {
        STALE_SAID
    };
    match apply_in(
        root,
        &one,
        persona.as_deref(),
        &audit,
        held.proposals().applying(),
        from,
    ) {
        Ok(said) => {
            held.proposals().take(id);
            crate::oldplugins::purlis_change_applied(root, &one);
            for session in held.proposals().others_stale(&one.path) {
                tell(held, session, stale_word(&one));
            }
            let word = if one.limit.is_some() {
                ProposalWord::LimitApplied {
                    path: one.path.clone(),
                }
            } else {
                ProposalWord::Applied {
                    path: one.path.clone(),
                }
            };
            tell(held, one.session, word);
            held.chats().asks_may_have_moved();
            Ok(said)
        }
        Err(Refusal::Stale) => {
            if held.proposals().stale(id) {
                tell(held, one.session, stale_word(&one));
            }
            held.chats().asks_may_have_moved();
            Err(stale_said.to_owned())
        }
        Err(Refusal::Not(why)) => Err(why),
    }
}

/// What Apply says of a proposal whose file changed since (PC-8).
pub const STALE_SAID: &str = "The file changed since this change was proposed, so nothing was \
                              written. Dismiss it; the chat is told to read the file again.";

/// What Apply says of a limit's proposal whose value changed since.
pub const LIMIT_STALE_SAID: &str = "This limit changed since the value was proposed, so nothing \
                                    was saved. Dismiss it; the chat is told it changed.";

/// Why Apply did not write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    Stale,
    Not(String),
}

/// What Apply says of a change that is not shown whole: never applied, whatever asked.
pub const NOT_SHOWN_SAID: &str = "This change is not shown whole, so nothing was written.";

/// **Applies `one` in the project at `root`**: judged again, shown whole, audited, written, and
/// the outcome audited. `lock` is the project's: held from the read of the file to its
/// rename, so two Applies against the same file as it was cannot both pass the check; the
/// second finds the file changed and is stale.
pub fn apply_in(
    root: &Path,
    one: &Proposal,
    persona: Option<&str>,
    audit: &dyn Fn(&proposed::Applied<'_>) -> Result<(), String>,
    lock: &Mutex<()>,
    from: From,
) -> Result<String, Refusal> {
    if one.stale {
        return Err(Refusal::Stale);
    }
    if let Some(why) = purlis_core::sandbox::policy::Locks::of(root).proposed_changes_refused() {
        return Err(Refusal::Not(why));
    }
    if let Some(change) = &one.limit {
        return apply_limit(one, change, persona, audit, lock);
    }
    let judged = if one.session == BY_PURLIS {
        proposed::proposable_by_purlis
    } else {
        proposed::proposable
    };
    let rel: PathBuf = judged(root, &root.join(&one.path)).map_err(Refusal::Not)?;
    let _held = lock.lock().unwrap_or_else(PoisonError::into_inner);
    let made = match proposed::prepare(root, &rel, Some(one.base.as_deref()), &one.change) {
        Ok(made) => made,
        Err(NotMade::Stale) => return Err(Refusal::Stale),
        Err(NotMade::Unprepared) => return Err(Refusal::Not(proposed::UNPREPARED.to_owned())),
    };
    // Only what the window shows whole is applied (PC-7, S3): the same judgement its row makes,
    // made again here, so no caller writes a change never shown.
    let shown =
        crate::askwrites::proposed(&one.path, made.before.as_deref().unwrap_or(""), &made.after);
    let before = made.before.as_deref().unwrap_or("");
    let whole = matches!(shown, crate::askwrites::WritesShown::Whole { .. })
        || (from == From::Tab && crate::askwrites::proposed_too_long(before, &made.after));
    if !whole {
        return Err(Refusal::Not(NOT_SHOWN_SAID.to_owned()));
    }
    let written = proposed::base_of(Some(made.after.as_bytes())).unwrap_or_default();
    let record = |outcome, why: Option<&str>| {
        audit(&proposed::Applied {
            persona,
            path: &one.path,
            written: &written,
            outcome,
            why,
        })
    };
    record(proposed::Outcome::Intent, None).map_err(Refusal::Not)?;
    // purlis's own change to a workspace's manifest goes through the manifest's writer (#3486).
    // Where its writer said why it refused, that is what the person is told.
    let (result, said) = match if one.session == BY_PURLIS {
        proposed::write_by_purlis(
            root,
            &rel,
            one.base.as_deref(),
            made.before.as_deref(),
            &made.after,
        )
    } else {
        proposed::write_pinned(root, &rel, one.base.as_deref(), &made.after).map_err(|m| (m, None))
    } {
        Ok(()) => (Ok(()), None),
        Err((made, said)) => (Err(made), said),
    };
    let couldnt = said.unwrap_or_else(|| {
        format!(
            "purlis could not write {}, so nothing was written.",
            purlis_core::shown::short(&one.path)
        )
    });
    let (outcome, why) = match &result {
        Ok(()) => (proposed::Outcome::Applied, None),
        Err(NotMade::Stale) => (proposed::Outcome::Stale, None),
        Err(NotMade::Unprepared) => (proposed::Outcome::Failed, Some(couldnt.as_str())),
    };
    // What happened is recorded whatever it was; a record that fails now is said in the log,
    // since the file is as it is either way.
    if let Err(why) = record(outcome, why) {
        tracing::warn!("purlis: a proposed change's outcome was not recorded ({why})");
    }
    match result {
        Ok(()) => Ok(format!(
            "Applied to {}. It rides your next save.",
            purlis_core::shown::short(&one.path)
        )),
        Err(NotMade::Stale) => Err(Refusal::Stale),
        Err(NotMade::Unprepared) => Err(Refusal::Not(couldnt)),
    }
}

/// **Saves a limit's proposal**: judged again, audited and saved
/// through [`purlis_core::runners::set_pool_limit`], so only that value changes.
pub(crate) fn apply_limit(
    one: &Proposal,
    change: &LimitChange,
    persona: Option<&str>,
    audit: &dyn Fn(&proposed::Applied<'_>) -> Result<(), String>,
    lock: &Mutex<()>,
) -> Result<String, Refusal> {
    let _held = lock.lock().unwrap_or_else(PoisonError::into_inner);
    let after = match limit_now(change) {
        Now::Made { after, .. } => after,
        Now::Stale => return Err(Refusal::Stale),
        Now::Unprepared => return Err(Refusal::Not(proposed::UNPREPARED.to_owned())),
    };
    let written = proposed::base_of(Some(after.as_bytes())).unwrap_or_default();
    let record = |outcome, why: Option<&str>| {
        audit(&proposed::Applied {
            persona,
            path: &one.path,
            written: &written,
            outcome,
            why,
        })
    };
    record(proposed::Outcome::Intent, None).map_err(Refusal::Not)?;
    // Compared again under the file's lock: the file is the machine's, `lock` the project's.
    let saved = purlis_core::runners::set_pool_limit_if(
        &change.config,
        &change.pool,
        change.id,
        change.value,
        change.was,
    )
    .map_err(|why| why.to_string());
    let (outcome, why) = match &saved {
        Ok(true) => (proposed::Outcome::Applied, None),
        Ok(false) => (proposed::Outcome::Stale, None),
        Err(why) => (proposed::Outcome::Failed, Some(why.as_str())),
    };
    if let Err(why) = record(outcome, why) {
        tracing::warn!("purlis: a proposed limit's outcome was not recorded ({why})");
    }
    if !saved.map_err(Refusal::Not)? {
        return Err(Refusal::Stale);
    }
    Ok(format!(
        "Saved to pool {}. {}",
        change.pool,
        change.id.limit().takes_effect.said()
    ))
}

/// **Dismiss** on a proposed change: nothing is written, and the chat is told on its next turn.
/// Window-only.
#[tauri::command]
#[specta::specta]
pub async fn dismiss_proposed_change(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    id: u32,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("dismissing a proposed change", move || dismissed(&held, id)).await
}

/// [`dismiss_proposed_change`]'s work, for project `held`.
pub fn dismissed(held: &Held, id: u32) -> Result<(), String> {
    let one = held.proposals().take(id).ok_or(WAITS_NO_MORE)?;
    crate::oldplugins::purlis_change_dismissed(held.root(), &one);
    // A stale one was told stale already; one put away fresh is the person's no.
    if !one.stale {
        tell(
            held,
            one.session,
            ProposalWord::Dismissed { path: one.path },
        );
    }
    held.chats().asks_may_have_moved();
    Ok(())
}

#[cfg(test)]
mod tests;
