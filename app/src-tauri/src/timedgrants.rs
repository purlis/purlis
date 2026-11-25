// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The timed grant and the Allowed list, for the window and the host** (M75 slice 21, #7030;
//! spec §8.8; Q312).
//!
//! - [`ask_offered`] says what the person is offered at a chat's ask: the scopes, the timed
//!   grant's lengths, and, at a deny, the one way out ("Propose a project change…").
//! - [`give_timed_grant`] is **the person's press**: "Allow for this chat, for…". The host decides
//!   the ask itself and grants only what it decides ([`crate::decisions::timed_grants`]), so a
//!   window cannot name a rule or a length the policy does not offer. It is the window's alone
//!   (`session_protocol::ui::WINDOW_ONLY`), never a link's: a chat's request is not the person's.
//! - [`allowed_answers`] and [`remove_allowed_answer`] are the Allowed list's data: the sealed
//!   grants and timed grants, with reason, countdown and Remove. Removing only narrows, so it
//!   takes no press, but it is still the window's.
//! - A timed grant **ends** four ways, and each writes one quiet line to the Journal (the host's
//!   event log, kind `trust.timed_grant.ended`) with its own `ended` word: [`sweep`] at its time
//!   (`time`), [`chat_closed`] at its chat's close (`chat closed`), a decision that finds its rule
//!   gone from the policy (`rule removed`, [`crate::decisions::decision`]) and Remove on the
//!   Allowed list (`removed`, [`remove_allowed_answer`]). All four go through [`end`].
//!
//! **The line is written before the entry leaves the store**, so a line that cannot be written
//! loses nothing: the entry stays, serves nothing, and is ended again at the next look. One at a
//! time in this process, and an entry whose line is written but whose removal failed is
//! remembered ([`end`]), so the line is written **once per end while the app runs**; a crash
//! between the line and the removal writes it once more when the entry is found again. That is
//! at-least-once across a crash, and never none.

use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use purlis_core::policy::grants::{self, Span};
use purlis_core::secrets::answers::{self, Entry, Standing};

use crate::hooks::Hooks;
use crate::planes::{Held, PlaneId};

/// **The sealed store, as the end of a timed grant reads and changes it**: the one seam, so a test
/// holds entries without the keyring the real store needs.
trait Store {
    /// The answers held now.
    fn entries(&self) -> Vec<Entry>;
    /// Takes `entries` out. It only narrows, so it takes no press.
    fn take_out(&self, entries: &[Entry]) -> io::Result<()>;
}

/// The sealed store of the project at `.0`.
struct Sealed<'a>(&'a Path);

impl Store for Sealed<'_> {
    fn entries(&self) -> Vec<Entry> {
        answers::standing(self.0).entries
    }

    fn take_out(&self, entries: &[Entry]) -> io::Result<()> {
        answers::commit(self.0, |draft| {
            for entry in entries {
                draft.remove_narrowing(answers::GRANT, &entry.text);
            }
        })
        .map(|_| ())
    }
}

/// The texts of the entries whose Journal line is written and that the store has not yet let go
/// of, held across the whole of [`end_in`] so one end is one line.
static WRITTEN: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// **Ends `ended`: each one's Journal line, then its entry out of the store.** An entry the store
/// no longer holds was ended by someone else and is passed over. A line that cannot be written
/// keeps its entry (nothing is lost, and it is tried again), and so does a removal that fails,
/// whose line is not written twice. `Err` says what was left.
fn end_in(
    store: &dyn Store,
    root: &Path,
    hooks: &Hooks,
    number: Option<u32>,
    ended: &[grants::Ended],
) -> Result<(), String> {
    if ended.is_empty() {
        return Ok(());
    }
    let mut guard = WRITTEN.lock().unwrap_or_else(PoisonError::into_inner);
    let written = guard.get_or_insert_with(HashSet::new);
    let held = store.entries();
    let mut leaving: Vec<Entry> = Vec::new();
    let mut left: Vec<String> = Vec::new();
    for one in ended {
        if !held.contains(&one.entry) {
            continue;
        }
        if !written.contains(&one.entry.text) {
            if let Err(why) = hooks.record_timed_grant_ended(root, number, one) {
                left.push(format!("the Journal line was not written ({why})"));
                continue;
            }
            written.insert(one.entry.text.clone());
        }
        leaving.push(one.entry.clone());
    }
    if !leaving.is_empty() {
        match store.take_out(&leaving) {
            Ok(()) => {
                for entry in &leaving {
                    written.remove(&entry.text);
                }
            }
            Err(why) => left.push(format!("the store kept it ({why})")),
        }
    }
    if left.is_empty() {
        Ok(())
    } else {
        Err(left.join("; "))
    }
}

/// **Takes the ended timed grants out of the sealed store of the project at `root`, each after its
/// Journal line** ([`end_in`]). A store that cannot be changed (the deny state, no keyring) keeps
/// the entry, which serves nothing past its end all the same, and is tried again.
pub fn end(
    root: &Path,
    hooks: &Hooks,
    number: Option<u32>,
    ended: &[grants::Ended],
) -> Result<(), String> {
    end_in(&Sealed(root), root, hooks, number, ended)
}

/// [`end`], the left-over only logged: where nobody is waiting on the answer.
pub fn end_quietly(root: &Path, hooks: &Hooks, number: Option<u32>, ended: &[grants::Ended]) {
    if let Err(why) = end(root, hooks, number, ended) {
        tracing::warn!("purlis: a timed grant was not fully ended: {why}");
    }
}

fn sweep_in(store: &dyn Store, root: &Path, hooks: &Hooks, entries: &[Entry], now: u64) {
    let ended = grants::ended(entries, None, now);
    if let Err(why) = end_in(store, root, hooks, None, &ended) {
        tracing::warn!("purlis: a timed grant was not fully ended: {why}");
    }
}

/// **Ends the timed grants of `standing` that have run out** by `now`, as a decision found them.
pub fn sweep(root: &Path, hooks: &Hooks, standing: &Standing, now: u64) {
    sweep_in(&Sealed(root), root, hooks, &standing.entries, now);
}

fn chat_closed_in(store: &dyn Store, root: &Path, hooks: &Hooks, number: u32, id: Option<&str>) {
    let ended = grants::ended(&store.entries(), id, grants::clock_now());
    if let Err(why) = end_in(store, root, hooks, Some(number), &ended) {
        tracing::warn!("purlis: a timed grant was not fully ended: {why}");
    }
}

/// **Ends chat `id`'s timed grants, and any that have run out, with the chat's close**: called
/// where no session of the chat is left open (a restart keeps the chat, so its grants).
pub fn chat_closed(root: &Path, hooks: &Hooks, number: u32, id: Option<&str>) {
    chat_closed_in(&Sealed(root), root, hooks, number, id);
}

/// What the person is offered at an ask, as the window draws it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct OfferShown {
    /// The grant scopes, narrowest first, in the words `@keep` uses.
    pub scopes: Vec<String>,
    /// The timed grant's lengths (`15m`, `1h`), shortest first; none where it is not offered.
    pub timed: Vec<String>,
    /// Whether Don't allow is offered.
    pub dont_allow: bool,
    /// The scopes a Never can be given at.
    pub never: Vec<String>,
    /// What a deny offers instead of the rest: the button's words.
    pub way_out: Option<String>,
}

fn offer_shown(offer: &grants::Offer) -> OfferShown {
    let words = |scopes: &[grants::Scope]| scopes.iter().map(|s| s.word().to_owned()).collect();
    OfferShown {
        scopes: words(&offer.scopes),
        timed: offer.timed.iter().map(|s| s.word().to_owned()).collect(),
        dont_allow: offer.dont_allow,
        never: words(&offer.never),
        way_out: offer.way_out.map(|way| way.label().to_owned()),
    }
}

/// **What chat `chat`'s ask offers the person**, decided by the host now. The window's alone.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ask_offered(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
    chat: u32,
    action: String,
    kind: String,
    resource: String,
) -> Result<OfferShown, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading what an ask offers", move || {
        crate::decisions::offered(&held, chat, &action, (&kind, &resource))
            .map(|offer| offer_shown(&offer))
            .ok_or_else(|| format!("chat {chat} is not one this app has open."))
    })
    .await
}

/// What a timed grant given says.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct TimedGiven {
    /// The rules it answers (one grant each).
    pub rules: Vec<String>,
    /// When it ends, in seconds since 1970.
    pub until: u32,
}

/// **The person allows chat `chat`'s ask for a while**, with a reason: one timed grant for each
/// rule that decides it, sealed on this machine. Made here and nowhere else: the press is the
/// person's in their window. Refused, with what to do, at a deny, at what is not an ask, with no
/// reason and for a length the policy does not offer.
fn give(
    held: &crate::planes::Held,
    press: &answers::Press,
    (chat, action, kind, resource): (u32, &str, &str, &str),
    (length, reason): (&str, &str),
) -> Result<TimedGiven, String> {
    let span = Span::from_word(length).ok_or_else(|| {
        format!("{length:?} is not a length of timed grant: choose 15 minutes or 1 hour.")
    })?;
    let made = crate::decisions::timed_grants(held, chat, action, (kind, resource), (span, reason))
        .ok_or_else(|| format!("chat {chat} is not one this app has open."))?
        .map_err(|refused| refused.said())?;
    let chat_id = made.first().and_then(|grant| grant.chat.clone());
    // Kept only while the chat is still open, under the lock a close takes, so a chat that closed
    // since the ask was decided is given nothing that would outlive it ([`Chats::commit_grant`]).
    held.chats()
        .while_open(chat_id.as_deref().unwrap_or_default(), || {
            answers::commit(held.root(), |draft| {
                for grant in &made {
                    draft.add(press, answers::GRANT, &grant.text());
                }
            })
        })
        .ok_or_else(|| format!("chat {chat} closed before it was allowed, so nothing was kept."))?
        .map_err(|why| format!("The grant was not kept, so nothing is allowed: {why}"))?;
    Ok(TimedGiven {
        rules: made.iter().map(|grant| grant.rule.clone()).collect(),
        until: made
            .first()
            .and_then(|grant| grant.timed.as_ref())
            .map_or(0, |timed| u32::try_from(timed.until()).unwrap_or(u32::MAX)),
    })
}

/// **"Allow for this chat, for…"**: [`give`], on the person's press. The window's alone.
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn give_timed_grant(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
    chat: u32,
    action: String,
    kind: String,
    resource: String,
    length: String,
    reason: String,
) -> Result<TimedGiven, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("giving a timed grant", move || {
        #[allow(
            clippy::disallowed_methods,
            reason = "the window's command layer is the one place that makes the person's press"
        )]
        let press = answers::Press::of_the_persons_press();
        give(
            &held,
            &press,
            (chat, &action, &kind, &resource),
            (&length, &reason),
        )
    })
    .await
}

/// One row of the Allowed list: a grant or a timed grant.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct AllowedAnswer {
    /// What Remove names it by.
    pub id: String,
    /// `once`, `timed`, `this chat`, `this workspace` or `everywhere`.
    pub scope: String,
    /// The rule it covers.
    pub rule: String,
    /// The action.
    pub action: String,
    /// What it covers.
    pub resource: String,
    /// The chat's ULID, for Once, timed and This chat.
    pub chat: Option<String>,
    /// The workspace, for This workspace.
    pub workspace: Option<String>,
    /// The reason, for a timed grant.
    pub reason: Option<String>,
    /// When a timed grant ends, in seconds since 1970.
    pub until: Option<u32>,
    /// How many seconds it has left as this list was read.
    pub left: Option<u32>,
    /// The chat's name, where it is open now: what a timed row names its chat by.
    pub chat_name: Option<String>,
    /// Whether it serves now. A timed grant stops serving when a cap tightens (`@timed`, a lock)
    /// or its rule no longer asks: it stays on the list, said so, until it ends or is removed.
    pub serving: bool,
}

fn shown(row: grants::Row, chat_name: Option<String>, serving: bool) -> AllowedAnswer {
    let secs = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
    AllowedAnswer {
        id: row.id,
        scope: row.scope.word().to_owned(),
        rule: row.rule,
        action: row.action,
        resource: row.resource,
        chat: row.chat,
        workspace: row.workspace,
        reason: row.reason,
        until: row.until.map(secs),
        left: row.left.map(secs),
        chat_name,
        serving,
    }
}

/// Whether a row serves now: a timed grant only while its rule still asks for this and the length
/// is one the caps still offer. Where its chat is not open there is no ask to judge it by, and it
/// is read as serving, as a chat's restart keeps it.
fn serving(held: &Held, row: &grants::Row, session: Option<u32>) -> bool {
    let (Some(span), Some(session)) = (row.span, session) else {
        return true;
    };
    crate::decisions::unanswered(
        held,
        session,
        &row.action,
        (&row.resource_kind, &row.resource),
    )
    .is_some_and(|decision| {
        decision.after_mode == purlis_core::policy::decide::Outcome::Ask
            && decision.deciding.iter().any(|rule| rule.id == row.rule)
            && grants::offer(&decision).timed.contains(&span)
    })
}

/// The Allowed list's rows for the project held, ending what has run out first.
fn list(held: &Held) -> Vec<AllowedAnswer> {
    let standing = answers::standing(held.root());
    let now = grants::clock_now();
    sweep(held.root(), held.hooks(), &standing, now);
    grants::rows(&standing.entries, now)
        .into_iter()
        .map(|row| {
            let session = row
                .chat
                .as_deref()
                .and_then(|id| held.chats().session_of_id(id));
            let name = session.and_then(|session| held.chats().shown_name(session));
            let serves = serving(held, &row, session);
            shown(row, name, serves)
        })
        .collect()
}

/// **The grants and timed grants the project's chats hold**, for the Allowed list. The window's
/// alone.
#[tauri::command]
#[specta::specta]
pub(crate) async fn allowed_answers(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
) -> Result<Vec<AllowedAnswer>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the Allowed list", move || Ok(list(&held))).await
}

/// Takes the grant `id` out of the store. A grant's removal only narrows what a chat may do, so it
/// needs no press. A timed grant is ended as `removed`, with its Journal line before it goes
/// ([`end_in`]); one of another scope leaves no line.
fn remove_in(store: &dyn Store, root: &Path, hooks: &Hooks, id: &str) -> Result<(), String> {
    let hit: Vec<Entry> = store
        .entries()
        .into_iter()
        .filter(|entry| entry.kind == answers::GRANT && grants::row_id(entry) == id)
        .collect();
    if hit.is_empty() {
        return Err("That is not on the list any more.".to_owned());
    }
    let timed: Vec<grants::Ended> = hit
        .iter()
        .filter_map(|entry| grants::Ended::of(entry, grants::Ending::Removed))
        .collect();
    end_in(store, root, hooks, None, &timed).map_err(|why| format!("It was not removed: {why}"))?;
    let rest: Vec<Entry> = hit
        .into_iter()
        .filter(|entry| !timed.iter().any(|one| one.entry == *entry))
        .collect();
    if !rest.is_empty() {
        store
            .take_out(&rest)
            .map_err(|why| format!("It was not removed: {why}"))?;
    }
    Ok(())
}

/// **Remove**, on the Allowed list. The window's alone.
#[tauri::command]
#[specta::specta]
pub(crate) async fn remove_allowed_answer(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
    id: String,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("removing an allowed answer", move || {
        remove_in(&Sealed(held.root()), held.root(), held.hooks(), &id)
    })
    .await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use purlis_core::eventlog::{self, ArgsKey, Log, Recorder};
    use purlis_core::policy::grants::{ENDED_KIND, Ending};
    use purlis_session_protocol::ui;

    use super::*;
    use crate::hooks::Events;

    const GIVEN: u64 = 1_800_000_000;
    const REASON: &str = "the private reason";

    /// The sealed store without its keyring: entries held in memory, and a removal that can be made
    /// to fail. What `end`, `sweep`, `chat_closed` and Remove read and change, through [`Store`].
    struct Kept {
        entries: Mutex<Vec<Entry>>,
        fails: std::sync::atomic::AtomicBool,
    }

    impl Kept {
        fn with(entries: &[Entry]) -> Self {
            Self {
                entries: Mutex::new(entries.to_vec()),
                fails: false.into(),
            }
        }

        fn kept(&self) -> Vec<Entry> {
            self.entries.lock().unwrap().clone()
        }

        fn refuse_removal(&self, fails: bool) {
            self.fails.store(fails, std::sync::atomic::Ordering::SeqCst);
        }
    }

    impl Store for Kept {
        fn entries(&self) -> Vec<Entry> {
            self.kept()
        }

        fn take_out(&self, entries: &[Entry]) -> io::Result<()> {
            if self.fails.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(io::Error::other("the store would not change"));
            }
            self.entries
                .lock()
                .unwrap()
                .retain(|held| !entries.contains(held));
            Ok(())
        }
    }

    /// A timed grant of `chat`, given at `GIVEN` for fifteen minutes, as the sealed store keeps it.
    /// Each test has a chat of its own: the lines written are remembered by entry, per process.
    fn timed_entry(chat: &str) -> Entry {
        let line = serde_json::json!({
            "v": 1, "kind": "ask-grant", "scope": "timed", "rule": "processes-ask-exec",
            "action": "exec", "resource_kind": "Command", "resource": "npm", "narrowing": {},
            "chat": chat, "span": "15m", "given": GIVEN, "reason": REASON,
        });
        Entry::new(answers::GRANT, &line.to_string())
    }

    /// A grant that is not a timed one: it ends with no Journal line.
    fn chat_entry(chat: &str) -> Entry {
        let line = serde_json::json!({
            "v": 1, "kind": "ask-grant", "scope": "chat", "rule": "processes-ask-exec",
            "action": "exec", "resource_kind": "Command", "resource": "npm", "narrowing": {},
            "chat": chat,
        });
        Entry::new(answers::GRANT, &line.to_string())
    }

    /// A `Hooks` whose event log is not open: every Journal line is refused.
    fn hooks_without_a_log(dir: &Path) -> Hooks {
        let at = crate::hooks::Where {
            within: dir.to_path_buf(),
            socket: dir.join("app").join("hooks.sock"),
        };
        let plane: PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");
        // Relayed binds no socket, which the Journal line does not need.
        Hooks::relayed(plane, &at, Arc::new(|_| {}), Arc::new(|_| {}))
    }

    /// A `Hooks` whose event log is in `dir`'s `events` folder.
    fn hooks_logging_into(dir: &Path) -> (Hooks, std::path::PathBuf) {
        let hooks = hooks_without_a_log(dir);
        let logs = dir.join("events");
        hooks.record_into(Events::here(Recorder::new(
            Log::open(&logs, "DEVICE").expect("a log"),
            ArgsKey::open(&logs).expect("a key"),
        )));
        (hooks, logs)
    }

    fn journal(logs: &Path) -> Vec<(String, Option<String>, serde_json::Value)> {
        eventlog::read(logs)
            .expect("the log reads")
            .into_iter()
            .filter(|event| event.kind == ENDED_KIND)
            .map(|event| (event.kind, event.chat, event.body))
            .collect()
    }

    #[test]
    fn a_grant_that_ran_out_is_one_quiet_journal_line_without_its_reason() {
        let dir = tempfile::tempdir().expect("a folder");
        let (hooks, logs) = hooks_logging_into(dir.path());
        let entry = timed_entry("01M85WM43DT07ANNAQ6NQEJTX1");
        let store = Kept::with(&[entry]);

        sweep_in(&store, dir.path(), &hooks, &store.kept(), GIVEN + 15 * 60);

        let lines = journal(&logs);
        assert_eq!(lines.len(), 1, "one line: {lines:?}");
        assert_eq!(lines[0].2["ended"], "time");
        assert_eq!(lines[0].2["rule"], "processes-ask-exec");
        assert!(
            !lines[0].2.to_string().contains(REASON),
            "the reason stays in the sealed store"
        );
        assert_eq!(store.kept(), [], "the entry is out of the store");
    }

    #[test]
    fn a_sweep_before_the_end_ends_nothing() {
        let dir = tempfile::tempdir().expect("a folder");
        let (hooks, logs) = hooks_logging_into(dir.path());
        let store = Kept::with(&[timed_entry("01M85WM43DT07ANNAQ6NQEJTX2")]);

        sweep_in(&store, dir.path(), &hooks, &store.kept(), GIVEN + 60);

        assert_eq!(journal(&logs), []);
        assert_eq!(store.kept().len(), 1);
    }

    #[test]
    fn a_chat_s_close_ends_its_grants_alone_under_that_chat() {
        let dir = tempfile::tempdir().expect("a folder");
        let (hooks, logs) = hooks_logging_into(dir.path());
        let (closing, other) = ("01M85WM43DT07ANNAQ6NQEJTX3", "01M85WM43DT07ANNAQ6NQEJTX4");
        // A clock that reads before the grants end: the close is what ends them.
        let store = Kept::with(&[
            timed_entry(closing),
            timed_entry(other),
            chat_entry(closing),
        ]);
        let ended = grants::ended(&store.kept(), Some(closing), GIVEN + 60);
        assert_eq!(ended.len(), 1);
        assert_eq!(ended[0].how, Ending::ChatClosed);

        end_in(&store, dir.path(), &hooks, Some(7), &ended).expect("ended");

        let lines = journal(&logs);
        assert_eq!(lines.len(), 1, "one line: {lines:?}");
        assert_eq!(lines[0].2["ended"], "chat closed");
        assert_eq!(lines[0].2["chat"], closing);
        assert_eq!(
            store.kept(),
            [timed_entry(other), chat_entry(closing)],
            "only the closing chat's timed grant went"
        );
    }

    #[test]
    fn a_chat_s_close_also_ends_a_grant_whose_time_has_run_out() {
        // The close reads the clock itself: a store holding a grant of a long-gone chat has it
        // ended by time, and one line is written.
        let dir = tempfile::tempdir().expect("a folder");
        let (hooks, logs) = hooks_logging_into(dir.path());
        let chat = "01M85WM43DT07ANNAQ6NQEJTX5";
        let store = Kept::with(&[timed_entry(chat)]);

        chat_closed_in(&store, dir.path(), &hooks, 7, Some(chat));

        assert_eq!(journal(&logs).len(), 1);
        assert_eq!(store.kept(), []);
    }

    #[test]
    fn two_ends_of_one_grant_write_one_line_between_them() {
        let dir = tempfile::tempdir().expect("a folder");
        let (hooks, logs) = hooks_logging_into(dir.path());
        let store = Kept::with(&[timed_entry("01M85WM43DT07ANNAQ6NQEJTX6")]);
        let found = grants::ended(&store.kept(), None, GIVEN + 15 * 60);

        end_in(&store, dir.path(), &hooks, None, &found).expect("the first");
        end_in(&store, dir.path(), &hooks, None, &found).expect("the second");

        assert_eq!(journal(&logs).len(), 1);
    }

    #[test]
    fn a_line_that_cannot_be_written_keeps_the_entry_and_is_written_when_it_can() {
        let dir = tempfile::tempdir().expect("a folder");
        let chat = "01M85WM43DT07ANNAQ6NQEJTX7";
        let store = Kept::with(&[timed_entry(chat)]);
        let found = grants::ended(&store.kept(), None, GIVEN + 15 * 60);

        let closed = hooks_without_a_log(dir.path());
        let refused = end_in(&store, dir.path(), &closed, None, &found);
        assert!(refused.is_err(), "{refused:?}");
        assert_eq!(store.kept().len(), 1, "the entry stays: no line was lost");

        let (hooks, logs) = hooks_logging_into(dir.path());
        end_in(&store, dir.path(), &hooks, None, &found).expect("ended at the next look");
        assert_eq!(journal(&logs).len(), 1);
        assert_eq!(store.kept(), []);
    }

    #[test]
    fn a_removal_that_fails_keeps_the_entry_and_does_not_write_its_line_twice() {
        let dir = tempfile::tempdir().expect("a folder");
        let (hooks, logs) = hooks_logging_into(dir.path());
        let store = Kept::with(&[timed_entry("01M85WM43DT07ANNAQ6NQEJTX8")]);
        let found = grants::ended(&store.kept(), None, GIVEN + 15 * 60);
        store.refuse_removal(true);

        assert!(end_in(&store, dir.path(), &hooks, None, &found).is_err());
        assert!(end_in(&store, dir.path(), &hooks, None, &found).is_err());
        assert_eq!(store.kept().len(), 1);
        assert_eq!(
            journal(&logs).len(),
            1,
            "one line however often it is tried"
        );

        store.refuse_removal(false);
        end_in(&store, dir.path(), &hooks, None, &found).expect("out at last");
        assert_eq!(journal(&logs).len(), 1);
        assert_eq!(store.kept(), []);
    }

    #[test]
    fn a_grant_lost_with_its_rule_is_one_line_saying_so() {
        let dir = tempfile::tempdir().expect("a folder");
        let (hooks, logs) = hooks_logging_into(dir.path());
        let entry = timed_entry("01M85WM43DT07ANNAQ6NQEJTX9");
        let store = Kept::with(std::slice::from_ref(&entry));
        let lost = [grants::Ended::of(&entry, Ending::RuleRemoved).expect("a timed grant")];

        end_in(&store, dir.path(), &hooks, None, &lost).expect("ended");

        let lines = journal(&logs);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0].2["ended"], "rule removed");
        assert_eq!(store.kept(), []);
    }

    #[test]
    fn remove_ends_a_timed_grant_as_removed_and_leaves_no_line_for_another_scope() {
        let dir = tempfile::tempdir().expect("a folder");
        let (hooks, logs) = hooks_logging_into(dir.path());
        let chat = "01M85WM43DT07ANNAQ6NQEJTXA";
        let (timed, plain) = (timed_entry(chat), chat_entry(chat));
        let store = Kept::with(&[timed.clone(), plain.clone()]);

        remove_in(&store, dir.path(), &hooks, &grants::row_id(&timed)).expect("removed");
        let lines = journal(&logs);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0].2["ended"], "removed");
        assert_eq!(store.kept(), std::slice::from_ref(&plain));

        remove_in(&store, dir.path(), &hooks, &grants::row_id(&plain)).expect("removed");
        assert_eq!(
            journal(&logs).len(),
            1,
            "a grant that is not timed writes no line"
        );
        assert_eq!(store.kept(), []);

        let gone = remove_in(&store, dir.path(), &hooks, &grants::row_id(&plain));
        assert_eq!(gone, Err("That is not on the list any more.".to_owned()));
    }

    #[test]
    fn remove_refuses_and_keeps_a_timed_grant_whose_line_cannot_be_written() {
        let dir = tempfile::tempdir().expect("a folder");
        let hooks = hooks_without_a_log(dir.path());
        let entry = timed_entry("01M85WM43DT07ANNAQ6NQEJTXB");
        let store = Kept::with(std::slice::from_ref(&entry));

        let refused = remove_in(&store, dir.path(), &hooks, &grants::row_id(&entry));

        assert!(
            refused
                .as_ref()
                .is_err_and(|said| said.starts_with("It was not removed")),
            "{refused:?}"
        );
        assert_eq!(store.kept(), std::slice::from_ref(&entry));
    }

    #[test]
    fn a_sweep_and_a_close_of_a_store_holding_nothing_write_no_line() {
        // The sealed store needs the app's keyring, which a test has not: it reads as holding
        // nothing, so neither end has an entry to take out and neither writes a line.
        let dir = tempfile::tempdir().expect("a folder");
        let (hooks, logs) = hooks_logging_into(dir.path());

        sweep(dir.path(), &hooks, &answers::standing(dir.path()), GIVEN);
        chat_closed(dir.path(), &hooks, 7, Some("01M85WM43DT07ANNAQ6NQEJTXC"));

        assert_eq!(journal(&logs), []);
    }

    #[test]
    fn the_four_commands_are_the_window_s_alone_and_a_link_is_refused_each() {
        let commands = include_str!("ipc_commands.rs");
        for name in [
            "ask_offered",
            "give_timed_grant",
            "allowed_answers",
            "remove_allowed_answer",
        ] {
            assert!(ui::window_only(name), "{name} is the window's alone");
            assert!(!ui::served_on_a_link(name), "{name} is refused to a link");
            assert!(
                commands.contains(&format!("timedgrants::{name}")),
                "{name} is registered"
            );
        }
        // Called over a link, each is refused before its handler is asked:
        // `the_four_timed_grant_commands_are_refused_over_a_link` in `purlis-session-protocol`.
    }
}
