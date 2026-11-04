// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A secret a chat asks purlis to fill, which the person approves on one notice** (#5115, spec
//! #5100, the person's rulings SF-1, SF-2 and SF-13 on #5096): the window's half of
//! [`purlis_core::secretfill`].
//!
//! A chat's `browser_fill_secret` names a field from its last snapshot, a vault and a key. The
//! browser reads the field first ([`purlis_session_protocol::browsing::Act::DescribeField`]):
//! the origin of the document of its own frame, as the engine records it, its label and its
//! kind. The person is then asked on a notice in the login notice's family, which names the key,
//! its vault, the exact site, large, the page and the field's label, never a value, and the
//! chat's call waits for the answer ([`asked`]).
//!
//! - **This slice offers Allow once and Keep blocked** (#5115). Allow once reads the value and
//!   has it filled into that one field, on that one origin, once; nothing is kept or written.
//! - **What's filled is what the person pressed on.** A press names the notice and carries its
//!   digest ([`FillWait::digest`]); one waits for each chat, and another call is refused.
//! - **Keep blocked fills nothing and tells the chat**, and the chat isn't asked about that key
//!   on that site again; after [`KEPT_AT_MOST`] it's asked about nothing more.
//! - **The engine judges again at the moment of filling**: the field's own frame must still be
//!   on the approved origin, and the field one line. The window's read is only what it shows.
//!
//! - **A browser on a pool's runner** (#5122, SF-12) is asked about the same way, in this
//!   window, and the notice warns that the value goes to that runner. The fill and its hold run
//!   there, through the runner's link, as a login's do.
//!
//! In memory only: a relaunch starts with none, and a chat's close takes its waits with it.

use std::sync::{Mutex, MutexGuard, PoisonError, mpsc};

use purlis_core::secrets::{Ctx, Env};
use purlis_session_protocol::browsing::{FieldKind, FieldSaid, Secret};
use sha2::{Digest, Sha256};

use crate::loginasks::{Press, Pressed};
use crate::planes::{Held, PlaneId, Planes};

/// **A secret fill's notice, as it waits**: names, an origin and the page's words, never a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillWait {
    /// Its number in the project: what its answer names.
    pub id: u32,
    pub session: u32,
    /// The chat's browser, by the name the chat knows it by (`b1`).
    pub browser: String,
    pub vault: String,
    pub key: String,
    /// The origin of the document of the field's own frame, as the engine recorded it.
    pub origin: String,
    /// The address of the page the field is on, where it is known.
    pub page: Option<String>,
    /// What the page labels the field: the page's words.
    pub label: String,
    /// What the person is warned of: a vault the chat's persona isn't tagged for, a field in a
    /// frame of another site than its page's.
    pub warnings: Vec<String>,
    pub since: u64,
}

impl FillWait {
    /// **The digest of what the notice draws**: a press names it, so a notice drawn of anything
    /// else is never what's answered.
    pub fn digest(&self) -> String {
        let mut hash = Sha256::new();
        for part in [
            self.browser.as_str(),
            &self.vault,
            &self.key,
            &self.origin,
            self.page.as_deref().unwrap_or_default(),
            &self.label,
        ] {
            hash.update(part.len().to_le_bytes());
            hash.update(part.as_bytes());
        }
        for warning in &self.warnings {
            hash.update(warning.len().to_le_bytes());
            hash.update(warning.as_bytes());
        }
        hash.finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

/// What a chat's waiting call hears.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    /// Allowed once.
    Allowed,
    /// Kept blocked, with the person's reason.
    Kept(Option<String>),
    /// The chat closed, or it's no longer asked.
    Gone,
}

/// What the person kept blocked: by chat, vault, key and origin.
type KeptOne = (u32, String, String, String);

#[derive(Debug, Default)]
struct Inner {
    next: u32,
    waits: Vec<(FillWait, Option<mpsc::Sender<Heard>>)>,
    kept: Vec<KeptOne>,
}

/// How many Keep blocked one chat is given before it's asked about no more fills, so a chat can't
/// change a key's name or a field until someone presses yes.
pub const KEPT_AT_MOST: usize = 2;

/// What a chat is told while the person is being asked about another of its fills.
const ALREADY_ASKED: &str = "the person is already being asked about a secret fill for this \
                             chat: wait for their answer before asking for another";

/// What a chat is told where nobody answered in time.
pub const NOT_ANSWERED: &str = "nobody answered about this secret in time, so nothing was \
                                filled and the notice went. Tell the person what needs it, or \
                                call browser_fill_secret again when they're back";

/// **The secret fills a project's chats wait on the person for.**
#[derive(Debug, Default)]
pub struct Fills {
    inner: Mutex<Inner>,
    /// Held by a press from its read of what waits to its answer: two presses never both pass.
    answering: Mutex<()>,
}

impl Fills {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// **Holds `wait`**, numbering it, or refuses it where its chat has one waiting: a notice is
    /// never replaced under the person's eyes. The receiver hears the answer.
    pub fn raise(&self, mut wait: FillWait) -> Result<(u32, mpsc::Receiver<Heard>), String> {
        let mut inner = self.lock();
        if inner
            .waits
            .iter()
            .any(|(one, _)| one.session == wait.session)
        {
            return Err(ALREADY_ASKED.to_owned());
        }
        inner.next += 1;
        wait.id = inner.next;
        let id = wait.id;
        let (tell, heard) = mpsc::channel();
        inner.waits.push((wait, Some(tell)));
        Ok((id, heard))
    }

    /// Every fill that waits, oldest first.
    pub fn waiting(&self) -> Vec<FillWait> {
        self.lock()
            .waits
            .iter()
            .map(|(one, _)| one.clone())
            .collect()
    }

    /// The fill of chat `session` numbered `id`, while it waits.
    fn wait(&self, session: u32, id: u32) -> Option<FillWait> {
        self.lock()
            .waits
            .iter()
            .find(|(one, _)| one.session == session && one.id == id)
            .map(|(one, _)| one.clone())
    }

    /// Ends the wait numbered `id` with `heard`: answers whether it waited.
    fn end(&self, id: u32, heard: Heard) -> bool {
        let mut inner = self.lock();
        let Some(at) = inner.waits.iter().position(|(one, _)| one.id == id) else {
            return false;
        };
        let (_, tell) = inner.waits.remove(at);
        if let Some(tell) = tell {
            let _ = tell.send(heard);
        }
        true
    }

    /// Whether the person kept this blocked for this chat, or kept enough blocked that it's
    /// asked about no more.
    fn kept(&self, session: u32, vault: &str, key: &str, origin: &str) -> bool {
        let inner = self.lock();
        let mine = || inner.kept.iter().filter(|(chat, ..)| *chat == session);
        mine().count() >= KEPT_AT_MOST
            || mine().any(|(_, v, k, o)| v == vault && k == key && o == origin)
    }

    /// Chat `session` closed: what it waited on goes with it, so nothing reaches a later chat
    /// dealt the same number, and its call stops waiting.
    pub fn forget(&self, session: u32) {
        // Never between a press's read of what waits and its answer.
        let _no_press = self
            .answering
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let mut inner = self.lock();
        inner.kept.retain(|(chat, ..)| *chat != session);
        inner.waits.retain(|(one, tell)| {
            if one.session != session {
                return true;
            }
            if let Some(tell) = tell {
                let _ = tell.send(Heard::Gone);
            }
            false
        });
    }
}

/// **What a chat's fill asks**, once its browser read the field: names and the engine's read of
/// the field, never a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillAsked {
    /// The chat's browser, by the name the chat knows it by.
    pub browser: String,
    pub target: String,
    pub vault: String,
    pub key: String,
    pub field: FieldSaid,
    /// The address of the page the field is on, where the browser said.
    pub page: Option<String>,
    /// The pool whose runner the browser runs on, where it runs on one (#5122).
    pub pool: Option<String>,
}

/// What a chat is told where the person kept a fill blocked.
pub fn kept_said(key: &str, origin: &str, reason: Option<&str>) -> String {
    let why = reason.map_or_else(String::new, |why| {
        format!(": {}", purlis_core::shown::short(why))
    });
    format!(
        "the person kept {key} blocked on {origin}, so nothing was filled{why}. Don't ask for it \
         again; tell them what needs it, or go on without it"
    )
}

/// **Whether `asked` may be put to the person, and what the notice warns of**: the field's frame
/// has a web origin, the field is one line, and the project lets the key be offered.
fn judged(held: &Held, session: u32, asked: &FillAsked) -> Result<(String, Vec<String>), String> {
    let target = &asked.target;
    let Some(origin) = asked.field.origin.clone() else {
        return Err(format!(
            "{target} is in a document with no web origin, so nothing was filled"
        ));
    };
    if asked.field.kind != FieldKind::OneLine {
        return Err(format!(
            "{target} is not a one-line field, so nothing was filled: only a one-line field \
             (password, text, email, number, tel, url) takes a secret"
        ));
    }
    let persona = held
        .chats()
        .persona_of(session)
        .and_then(|persona| persona.started_as);
    let ctx = Ctx::new(held.root(), Env::from_process());
    let mut warnings: Vec<String> =
        purlis_core::secretfill::judged(&ctx, persona.as_deref(), &asked.vault, &asked.key)?
            .into_iter()
            .collect();
    warnings.extend(frame_warning(&origin, asked.page.as_deref()));
    warnings.extend(asked.pool.as_deref().map(runner_warning));
    Ok((origin, warnings))
}

/// **The runner of `pool` a chat's fill may go to** (#5122; SF-12, ADR 0089): only one shown to be
/// the person's own, as a login's is (`purlis_core::poollogins::Bound::over_the_link`), for a
/// chat in its sandbox there, on a pool no chat ran on without one; or why not, in words the chat
/// reads. `config` is this machine's pools, `reached` the runner the chat's link reached.
fn runner_bound(
    unconfined: bool,
    config: Option<&std::path::Path>,
    pool: &str,
    reached: Option<&str>,
) -> Result<purlis_core::poollogins::Bound, String> {
    if unconfined {
        return Err(format!(
            "this chat runs on pool {pool}'s runner without the sandbox, so purlis fills no \
             secret there: start it in its sandbox"
        ));
    }
    let config = config.ok_or_else(|| {
        format!("purlis keeps no pools on this machine, so nothing was filled on pool {pool}")
    })?;
    if purlis_core::runnertaint::pool_ran_unsandboxed(config, pool) {
        return Err(purlis_core::runnertaint::refused_on(pool));
    }
    purlis_core::poollogins::Bound::over_the_link(config, pool, reached)
        .map_err(|why| format!("nothing was filled on pool {pool}: {why}"))
}

/// **What the person is warned of where the browser runs on a pool's runner** (#5122, SF-12):
/// the value is read on this machine and goes over the runner's link to that machine.
fn runner_warning(pool: &str) -> String {
    format!(
        "This browser runs on pool {pool}'s runner, so the secret goes to that machine. Allow \
         only if you trust that runner with it."
    )
}

/// **What the person is warned of where the field's frame is on another site than its page**
/// (#5115): a page may hold a part of another site's, and the secret goes to that part's site.
fn frame_warning(origin: &str, page: Option<&str>) -> Option<String> {
    let page = page.and_then(purlis_session_protocol::browsing::origin_of)?;
    (page != origin).then(|| {
        format!(
            "The field is in a part of the page from {origin}, inside a page from {page}. Allow \
             only if you know {origin} as the place this secret goes."
        )
    })
}

/// **Chat `session`'s fill `asked`, put to the person**, and the value to fill once they allowed
/// it: or why nothing is filled, in words the chat reads. The call waits here for the answer, at
/// most [`purlis_core::loginask::THE_PERSON_IS_WAITED_FOR`], under no lock of the browsers'.
pub fn asked(held: &Held, session: u32, asked: &FillAsked, now: u64) -> Result<Secret, String> {
    let (origin, warnings) = judged(held, session, asked)?;
    // On a runner, only one shown to be the person's own, before anyone is asked (#5122).
    let runner = match asked.pool.as_deref() {
        Some(pool) => {
            let unconfined = held
                .chats()
                .login_asker(held.root(), session)
                .is_none_or(|(_, unconfined)| unconfined);
            let config = held.chats().pools_in();
            let reached = crate::hooks::reached_over_the_link();
            let read =
                move || runner_bound(unconfined, config.as_deref(), pool, reached.as_deref());
            let bound = read()?;
            Some((read, bound))
        }
        None => None,
    };
    let fills = held.secret_fills();
    if fills.kept(session, &asked.vault, &asked.key, &origin) {
        return Err(kept_said(&asked.key, &origin, None));
    }
    let wait = FillWait {
        id: 0,
        session,
        browser: asked.browser.clone(),
        vault: asked.vault.clone(),
        key: asked.key.clone(),
        origin: origin.clone(),
        page: asked.page.clone(),
        label: asked.field.label.clone(),
        warnings,
        since: now,
    };
    let (id, heard) = fills.raise(wait)?;
    tracing::info!("purlis: chat {session}'s secret fill asks the person");
    held.chats().asks_may_have_moved();
    let heard = match heard.recv_timeout(purlis_core::loginask::THE_PERSON_IS_WAITED_FOR) {
        Ok(heard) => heard,
        Err(mpsc::RecvTimeoutError::Disconnected) => Heard::Gone,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // Under the presses' own hold, so a press is either wholly before this or finds the
            // wait gone: a press in that very moment has ended it, and its answer waits here.
            let _no_press = fills
                .answering
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let pressed = if fills.end(id, Heard::Gone) {
                Heard::Gone
            } else {
                heard.try_recv().unwrap_or(Heard::Gone)
            };
            held.chats().asks_may_have_moved();
            pressed
        }
    };
    match heard {
        Heard::Allowed => {
            // Judged again as the project reads now: policy may have changed while it waited.
            judged(held, session, asked)?;
            let (asker, _) = held
                .chats()
                .login_asker(held.root(), session)
                .ok_or_else(|| {
                    "this chat is no longer open in purlis's window, so nothing was filled"
                        .to_owned()
                })?;
            let value =
                purlis_core::secrets::brokered::fill_value(&asker, &asked.vault, &asked.key)?;
            // Again once it is read: the pool or its runner may have changed meanwhile.
            if let Some((read, bound)) = &runner
                && read().as_ref() != Ok(bound)
            {
                return Err(
                    "the pool or its runner changed as the secret was read, so nothing was filled"
                        .to_owned(),
                );
            }
            Ok(value)
        }
        Heard::Kept(reason) => Err(kept_said(&asked.key, &origin, reason.as_deref())),
        Heard::Gone => Err(NOT_ANSWERED.to_owned()),
    }
}

/// Writes the audit of a press, and answers whether it was written.
pub type Audit<'a> =
    &'a dyn Fn(Option<u32>, &purlis_core::sandbox::grant::Audited<'_>) -> Result<(), String>;

/// What the audit calls a secret fill the person allowed.
const AUDITED: &str = "secret-fill";

/// A press, as the window sends it.
#[derive(Debug, Clone, Copy)]
pub struct Asked<'a> {
    pub session: u32,
    pub id: u32,
    pub press: Press,
    /// The digest of the notice the person pressed on.
    pub shown: &'a str,
    pub reason: Option<&'a str>,
}

/// **The person's press** on a fill's notice. Fails closed: a press on anything but what's
/// waiting now changes nothing, and only Allow once and Keep blocked are offered in this slice.
pub fn answer(
    root: &std::path::Path,
    fills: &Fills,
    asked: &Asked<'_>,
    audit: Audit<'_>,
) -> Result<Pressed, String> {
    const NO_MORE: &str = "That ask has gone, and nothing was allowed.";
    const CHANGED: &str = "That ask changed, so nothing was allowed. Read it again.";
    let _one_press = fills
        .answering
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let wait = fills.wait(asked.session, asked.id).ok_or(NO_MORE)?;
    if wait.digest() != asked.shown {
        return Err(CHANGED.to_owned());
    }
    match asked.press {
        Press::Keep => {
            fills.lock().kept.push((
                wait.session,
                wait.vault.clone(),
                wait.key.clone(),
                wait.origin.clone(),
            ));
            fills.end(wait.id, Heard::Kept(asked.reason.map(str::to_owned)));
            Ok(Pressed::Done {
                said: "Kept blocked. Nothing was filled.".to_owned(),
            })
        }
        Press::Once => {
            let level = purlis_core::sandbox::grant::Level::Once;
            let locks = purlis_core::sandbox::policy::Locks::of(root);
            if let Some(why) = locks.scope_removed(level) {
                return Err(why);
            }
            let target = format!("{}/{} -> {}", wait.vault, wait.key, wait.origin);
            audit(
                Some(wait.session),
                &purlis_core::sandbox::grant::Audited {
                    granted: true,
                    what: AUDITED,
                    target: &target,
                    level,
                },
            )?;
            if !fills.end(wait.id, Heard::Allowed) {
                return Err(NO_MORE.to_owned());
            }
            Ok(Pressed::Done {
                said: format!(
                    "Allowed once. {} goes into that one field on {}; nothing was written.",
                    wait.key, wait.origin
                ),
            })
        }
        Press::Chat | Press::Always => {
            Err("Only Allow once or Keep blocked can be pressed on this ask.".to_owned())
        }
    }
}

/// **The person's press on a secret fill's notice** (#5115). The window's alone
/// (`ui::WINDOW_ONLY`).
#[tauri::command]
#[specta::specta]
pub async fn answer_secret_fill(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    id: u32,
    press: Press,
    shown: String,
    reason: Option<String>,
) -> Result<Pressed, String> {
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    crate::off_the_window("answering a secret fill", move || {
        let answer = answer(
            held.root(),
            held.secret_fills(),
            &Asked {
                session,
                id,
                press,
                shown: &shown,
                reason: reason.as_deref(),
            },
            &held.audit_then_record(network.as_ref()),
        );
        held.chats().asks_may_have_moved();
        answer
    })
    .await
}

#[cfg(test)]
#[path = "secretfills/tests.rs"]
mod tests;
