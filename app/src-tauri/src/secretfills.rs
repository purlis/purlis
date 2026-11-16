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
//! - **The sandbox's four answers** (#5115, #5116, SF-6). Allow once reads the value and has it
//!   filled into that one field, on that one origin, once; nothing is kept or written. Allow for
//!   this chat holds that key on that origin for the chat's life, in memory. Always allow writes
//!   one secret-fill line ([`purlis_core::sandbox::tools::fill`]) in the persona's tools, shown
//!   on the notice before the press, and allows it here, as a login's Always does
//!   ([`crate::loginasks::written`]); `sandbox.proposed-changes = "off"` holds it back.
//! - **A card's key is offered Allow once alone** (SF-4): a secret-fill line of any persona's
//!   with `card = true` marks it, on every origin, and no grant of it fills without a notice.
//! - **A field the page marks for payment is offered Allow once alone** (the steward's decision of
//!   2026-11-06): an `autocomplete` token that begins `cc-` asks each time, as a card's key does,
//!   whatever its key is marked.
//! - **A grant fills no field whose form sends it by GET** (the person's ruling of 2026-11-06):
//!   in the page's address, which the site's logs and the browser's history keep. Under Allow for
//!   this chat or Always, such a fill is refused; asked about, Allow once fills it.
//! - **A key the vault doesn't hold goes to the Secret ask** (#5054, ADR 0109) once the person
//!   allowed the fill: the call waits on that too, and fills what they add. Where the call ends,
//!   or stops waiting, before they add it, their Allow stands for the chat's retry of exactly
//!   that notice, for one call (#5242, ADR 0067 as amended 2026-11-12).
//! - **What's filled is what the person pressed on.** A press names the notice and carries its
//!   digest ([`FillWait::digest`]); one waits for each chat, and another call is refused.
//! - **Keep blocked fills nothing and tells the chat**, and the persona's chats aren't asked about
//!   that key on that site again until purlis restarts; after [`KEPT_AT_MOST`] they're asked about
//!   nothing more (the person's ruling of 2026-11-07). A chat with no persona keeps its own.
//! - **The engine judges again at the moment of filling**: the field's own frame must still be
//!   on the approved origin, and the field one line. The window's read is only what it shows.
//!
//! - **A form on the filled page may be sent only where the key is approved** (#5230, SF-15): the
//!   other origins a line or an Allow for this chat approve it on go with the fill, and the browser
//!   refuses any other, saying how to approve it there.
//! - **A browser on a pool's runner** (#5122, SF-12) is asked about the same way, in this
//!   window, and the notice warns that the value goes to that runner. The fill and its hold run
//!   there, through the runner's link, as a login's do.
//!
//! In memory only: a relaunch starts with none, and a chat's close takes its waits and grants
//! with it.

use std::sync::{Mutex, MutexGuard, PoisonError, mpsc};

use purlis_core::sandbox::policy::Locks;
use purlis_core::secrets::brokered;
use purlis_core::secrets::{Ctx, Env};
use purlis_session_protocol::browsing::{FieldKind, FieldSaid, Secret};
use sha2::{Digest, Sha256};

use crate::loginasks::{Ended, Press, Pressed};
use crate::planes::{Held, PlaneId, Planes};

/// **A secret fill's notice, as it waits**: names, an origin and the page's words, never a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillWait {
    /// Its number in the project: what its answer names.
    pub id: u32,
    pub session: u32,
    /// The chat's id (ADR 0066) as it raised this: what an Allow for this chat is bound to,
    /// beside its number.
    pub chat: Option<String>,
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
    /// A 2FA code the seed under `key` makes, not the value itself (#5118).
    pub code: bool,
    pub since: u64,
    /// The persona whose grants the chat fills under: whose tools Always writes in. A narrowed
    /// chat's is the persona it was narrowed to ([`crate::chats::Chats::secret_grants_of`]).
    pub persona: Option<String>,
    /// **A card's or payment value's key** (SF-4): Allow once and Keep blocked alone.
    pub card: bool,
    /// **A field the page marks a card's or payment one** (`autocomplete="cc-…"`, the steward's
    /// decision of 2026-11-06), or whose read doesn't say: asked each time, with Allow once and
    /// Keep blocked alone, whatever the key is marked.
    pub payment: bool,
    /// **A multi-line text box or an editable area** (#5120, SF-5): asked each time, whatever
    /// was allowed, with Allow once and Keep blocked alone.
    pub multi_line: bool,
    /// The line Always would write, as written: `<name> = { vault = …, key = …, fill = … }`.
    pub line: Option<String>,
    /// Why Always isn't offered, where it isn't.
    pub no_always: Option<String>,
    /// The persona needs its vault allowed too: what Always then also does.
    pub vault_too: bool,
    /// **The persona whose vaults the fill reads**: the one the chat was started as, narrowed or
    /// not, as the read judges a link (#5831). Its key is judged as this persona again as the
    /// notice is pressed.
    pub reads_as: Option<String>,
}

impl FillWait {
    /// Whether only Allow once and Keep blocked are offered: a card's key, or a field that asks
    /// each time.
    pub fn once_only(&self) -> bool {
        self.card || self.payment || self.multi_line || !held_open(&self.origin)
    }

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
        // What the choices are and what Always writes are part of what's pressed on.
        for part in [
            self.persona.as_deref().unwrap_or_default(),
            self.line.as_deref().unwrap_or_default(),
            self.no_always.as_deref().unwrap_or_default(),
        ] {
            hash.update(part.len().to_le_bytes());
            hash.update(part.as_bytes());
        }
        hash.update([
            u8::from(self.card),
            u8::from(self.payment),
            u8::from(self.vault_too),
            u8::from(self.multi_line),
            // A press on the notice of a value never allows a code, nor one on a code a value.
            u8::from(self.code),
        ]);
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

/// A key on an origin for one chat: by chat's number and id, vault, key and origin.
type ForChat = (u32, Option<String>, String, String, String);

/// **A chat, by its number and its id** (ADR 0066): what it was allowed answers it alone, never
/// a later chat dealt the same number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chat<'a> {
    pub session: u32,
    pub id: Option<&'a str>,
}

impl Chat<'_> {
    /// Whether `(number, id)` is this chat.
    fn is(&self, (number, id): (&u32, &Option<String>)) -> bool {
        *number == self.session && id.as_deref() == self.id
    }
}

/// **Whose Keep blocked it is** (the person's ruling of 2026-11-07): a persona's, for all its
/// chats until purlis restarts, so a new chat starts no fresh tries. A chat that runs as no
/// persona keeps its own, which goes when it closes.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Whose {
    Persona(String),
    Chat(u32),
}

impl Whose {
    fn of(session: u32, persona: Option<&str>) -> Self {
        match persona {
            Some(persona) if !persona.is_empty() => Self::Persona(persona.to_owned()),
            _ => Self::Chat(session),
        }
    }
}

#[derive(Debug, Default)]
struct Inner {
    next: u32,
    waits: Vec<(FillWait, Option<mpsc::Sender<Heard>>)>,
    /// What the person kept blocked, by whose it is, vault, key and origin.
    kept: Vec<(Whose, String, String, String)>,
    /// What the person allowed for the chat (#5116): in memory, for the chat's life.
    granted: Vec<ForChat>,
    /// The 2FA codes the person allowed for the chat (#5118): a grant of its own, so neither a
    /// value's grant nor a code's ever stands for the other.
    codes_granted: Vec<ForChat>,
    /// **The person's Allow on a notice whose fill never went in** (#5242): its call ended, or
    /// stopped waiting, while the key it fills was asked for. Kept for that chat's retry of
    /// exactly that notice (its digest), on the same runner or none, and taken by the one call
    /// that uses it.
    allowed_unfilled: Vec<AllowedUnfilled>,
}

/// An Allow whose fill never went in: by chat's number and id, the pool its browser runs on, and
/// the digest of the notice the person pressed on.
type AllowedUnfilled = (u32, Option<String>, Option<String>, String);

/// How many Keep blocked one persona is given before its chats are asked about no more fills until
/// purlis restarts (a chat with no persona, that chat), so a chat can't change a key's name or a
/// field, or a new chat start again, until someone presses yes.
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

    /// Draws a fill that waits anew, as the project reads now. Only the app's own answer calls
    /// this: no chat's call reaches it.
    fn redraw(&self, wait: FillWait) {
        if let Some((one, _)) = self
            .lock()
            .waits
            .iter_mut()
            .find(|(one, _)| one.id == wait.id)
        {
            *one = wait;
        }
    }

    /// Ends the wait numbered `id` with `heard`: answers whether it waited.
    fn end(&self, id: u32, heard: Heard) -> bool {
        Self::ended(&mut self.lock(), id, heard)
    }

    /// [`Self::end`], under a lock already held.
    fn ended(inner: &mut Inner, id: u32, heard: Heard) -> bool {
        let Some(at) = inner.waits.iter().position(|(one, _)| one.id == id) else {
            return false;
        };
        let (_, tell) = inner.waits.remove(at);
        if let Some(tell) = tell {
            let _ = tell.send(heard);
        }
        true
    }

    /// Whether the person kept this blocked for chat `session` running as `persona`, or kept
    /// enough blocked that it's asked about no more ([`Whose`]).
    fn kept(
        &self,
        session: u32,
        persona: Option<&str>,
        vault: &str,
        key: &str,
        origin: &str,
    ) -> bool {
        let whose = Whose::of(session, persona);
        let inner = self.lock();
        let mine = || inner.kept.iter().filter(|(one, ..)| *one == whose);
        mine().count() >= KEPT_AT_MOST
            || mine().any(|(_, v, k, o)| v == vault && k == key && o == origin)
    }

    /// Whether the person allowed this key on this origin for this chat (#5116).
    fn granted(&self, chat: Chat<'_>, vault: &str, key: &str, origin: &str) -> bool {
        self.lock()
            .granted
            .iter()
            .any(|(n, id, v, k, o)| chat.is((n, id)) && v == vault && k == key && o == origin)
    }

    /// Whether the person allowed codes of this key on this origin for this chat (#5118).
    fn code_granted(&self, chat: Chat<'_>, vault: &str, key: &str, origin: &str) -> bool {
        self.lock()
            .codes_granted
            .iter()
            .any(|(n, id, v, k, o)| chat.is((n, id)) && v == vault && k == key && o == origin)
    }

    /// The origins the person allowed this key on for this chat (#5230).
    fn granted_origins(&self, chat: Chat<'_>, vault: &str, key: &str) -> Vec<String> {
        self.lock()
            .granted
            .iter()
            .filter(|(n, id, v, k, _)| chat.is((n, id)) && v == vault && k == key)
            .map(|(.., origin)| origin.clone())
            .collect()
    }

    /// **Keeps the person's Allow on `digest`'s notice** for chat `chat`'s retry of that very fill
    /// (#5242): the value was not there yet, so nothing went in.
    fn keep_unfilled(&self, chat: Chat<'_>, pool: Option<&str>, digest: String) {
        let mut inner = self.lock();
        let one = (
            chat.session,
            chat.id.map(str::to_owned),
            pool.map(str::to_owned),
            digest,
        );
        if !inner.allowed_unfilled.contains(&one) {
            inner.allowed_unfilled.push(one);
        }
    }

    /// **Takes the Allow kept for chat `chat`'s fill of exactly `digest`'s notice** (#5242), if one
    /// is: whether it was. Taken here, so only one call ever uses it.
    fn take_unfilled(&self, chat: Chat<'_>, pool: Option<&str>, digest: &str) -> bool {
        let mut inner = self.lock();
        let Some(at) = inner
            .allowed_unfilled
            .iter()
            .position(|(n, id, on, shown)| {
                chat.is((n, id)) && on.as_deref() == pool && shown == digest
            })
        else {
            return false;
        };
        inner.allowed_unfilled.remove(at);
        true
    }

    /// Chat `session` closed: what it waited on and was allowed go with it, so nothing reaches a
    /// later chat dealt the same number, and its call stops waiting.
    pub fn forget(&self, session: u32) {
        // Never between a press's read of what waits and its answer.
        let _no_press = self
            .answering
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let mut inner = self.lock();
        inner
            .kept
            .retain(|(whose, ..)| *whose != Whose::Chat(session));
        inner.granted.retain(|(chat, ..)| *chat != session);
        inner.codes_granted.retain(|(chat, ..)| *chat != session);
        inner.allowed_unfilled.retain(|(chat, ..)| *chat != session);
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
    /// That browser's program in the session host.
    pub program: String,
    pub target: String,
    pub vault: String,
    pub key: String,
    /// A 2FA code the seed under `key` makes, not the value itself (#5118).
    pub code: bool,
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

/// What [`judged`] found of a fill it lets through.
struct Judged {
    /// The field's web origin.
    origin: String,
    /// What the person is warned of on its notice.
    warnings: Vec<String>,
    /// The persona whose vaults it read as: the one the chat was started as, which its notice
    /// reads as too, so the two cannot drift apart (#5910).
    reads_as: Option<String>,
}

/// **Whether `asked` may be put to the person, and what the notice warns of**: the field's frame
/// has a web origin, the field takes typing, and the project lets the key be offered.
fn judged(held: &Held, session: u32, asked: &FillAsked) -> Result<Judged, String> {
    let target = &asked.target;
    let Some(origin) = asked.field.origin.clone() else {
        return Err(format!(
            "{target} is in a document with no web origin, so nothing was filled"
        ));
    };
    if asked.field.kind == FieldKind::Other {
        return Err(format!(
            "{target} is not a field that takes typing, so nothing was filled: only a one-line \
             field (password, text, email, number, tel, url), a multi-line text box or an \
             editable area takes a secret"
        ));
    }
    // Whose vaults it reads: the persona it was started as, narrowed or not (#2599, #2691).
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
    Ok(Judged {
        origin,
        warnings,
        reads_as: persona,
    })
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
/// it: or why nothing is filled, in words the chat reads. A line Always wrote, or the person's
/// Allow for this chat, fills without a notice, never a card's key (SF-4). The call waits here
/// for the answer, at most [`purlis_core::loginask::THE_PERSON_IS_WAITED_FOR`], under no lock of
/// the browsers', and then on a Secret ask where the vault holds no such key. Where the call's
/// asker is `gone` while the person is asked, the notice goes and nothing is filled.
pub fn asked(
    held: &Held,
    session: u32,
    asked: &FillAsked,
    now: u64,
    gone: &dyn Fn() -> bool,
) -> Result<Allowed, String> {
    let started = std::time::Instant::now();
    let Judged {
        origin,
        warnings,
        reads_as,
    } = judged(held, session, asked)?;
    // SF-5: a multi-line text box or an editable area asks each time, whatever was allowed.
    let multi_line = asked.field.kind != FieldKind::OneLine;
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
    // Whose grants it fills under, and whose tools Always writes in: a narrowed chat's are the
    // persona it was narrowed to (the decision of 2026-11-06).
    let persona = held.chats().secret_grants_of(session);
    let id = held.chats().id_of(session);
    let chat = Chat {
        session,
        id: id.as_deref(),
    };
    if fills.kept(
        session,
        persona.as_deref(),
        &asked.vault,
        &asked.key,
        &origin,
    ) {
        return Err(kept_said(&asked.key, &origin, None));
    }
    // A browser on a runner is asked about each time: a grant here holds this machine's
    // browsers (#5122).
    let asks_the_person =
        runner.is_some() || asks(held.root(), fills, chat, persona.as_deref(), asked, &origin);
    // A grant fills no field whose form sends it in the page's address (the person's ruling of
    // 2026-11-06).
    if !asks_the_person && asked.field.by_get == Some(true) {
        return Err(by_get_said(&asked.target, &origin));
    }
    // The digest of the notice the person allowed, where they were asked: what a fill that is not
    // filled keeps their Allow for (#5242).
    let mut allowed_on = None;
    if asks_the_person {
        let wait = offered(
            held.root(),
            FillWait {
                id: 0,
                session,
                chat: id.clone(),
                browser: asked.browser.clone(),
                vault: asked.vault.clone(),
                key: asked.key.clone(),
                origin: origin.clone(),
                page: asked.page.clone(),
                label: asked.field.label.clone(),
                warnings,
                code: asked.code,
                since: now,
                persona,
                card: false,
                payment: asked.field.payment != Some(false),
                multi_line,
                line: None,
                no_always: None,
                vault_too: false,
                reads_as,
            },
        );
        let digest = wait.digest();
        let pool = asked.pool.as_deref();
        // **An Allow the person gave this very notice, whose fill never went in** (#5242): the
        // retry takes it, as policy still lets Allow once be, rather than asking again. It was
        // judged as the project reads now, above.
        let carried = fills.take_unfilled(chat, pool, &digest)
            && Locks::of(held.root())
                .scope_removed(purlis_core::sandbox::grant::Level::Once)
                .is_none();
        if !carried {
            match heard(held, fills, wait, gone)? {
                Heard::Allowed => {
                    // Judged again as the project reads now: policy may have changed while it
                    // waited.
                    judged(held, session, asked)?;
                }
                Heard::Kept(reason) => {
                    return Err(kept_said(&asked.key, &origin, reason.as_deref()));
                }
                Heard::Gone => return Err(NOT_ANSWERED.to_owned()),
            }
        }
        allowed_on = Some(digest);
    }
    let value = match read(held, session, asked, &origin, started, gone) {
        Ok(value) => value,
        Err(Unread { why, waited_out }) => {
            // The person said yes to this fill, and its key was not there in time: their yes
            // stands for the retry of exactly this fill, and nothing more.
            if waited_out && let Some(digest) = allowed_on {
                fills.keep_unfilled(chat, asked.pool.as_deref(), digest);
            }
            return Err(why);
        }
    };
    // Again once it is read: the pool or its runner may have changed meanwhile.
    if let Some((read, bound)) = &runner
        && read().as_ref() != Ok(bound)
    {
        return Err(
            "the pool or its runner changed as the secret was read, so nothing was filled"
                .to_owned(),
        );
    }
    let persona = persona_of_fill(held, session);
    let also = approved_elsewhere(held.root(), fills, chat, persona.as_deref(), asked, &origin);
    Ok(Allowed {
        value,
        multi_line,
        also,
        under_a_grant: !asks_the_person,
    })
}

/// **What a chat is told where a grant would fill a field whose form sends it by GET** (the
/// person's ruling of 2026-11-06): in the page's address, which the site's logs and the browser's
/// history keep.
fn by_get_said(target: &str, origin: &str) -> String {
    format!(
        "{target} on {origin} is in a form that sends what it holds in the page's address (by \
         GET), so purlis fills nothing there under the person's standing Allow: tell them what \
         needs it"
    )
}

/// The persona whose fill grants chat `session` reads ([`crate::chats::Chats::secret_grants_of`]).
fn persona_of_fill(held: &Held, session: u32) -> Option<String> {
    held.chats().secret_grants_of(session)
}

/// **The other origins this key is approved for** (#5230, SF-15): a line of the persona's, or the
/// person's Allow for this chat, each as [`granted`] reads one. What the page's form may be sent
/// to while it holds the value.
fn approved_elsewhere(
    root: &std::path::Path,
    fills: &Fills,
    chat: Chat<'_>,
    persona: Option<&str>,
    asked: &FillAsked,
    origin: &str,
) -> Vec<String> {
    let ctx = Ctx::new(root, Env::from_process());
    let mut named =
        purlis_core::secretfill::origins_of_lines(&ctx, persona, &asked.vault, &asked.key);
    named.extend(fills.granted_origins(chat, &asked.vault, &asked.key));
    named.sort();
    named.dedup();
    named.retain(|one| one != origin && granted(root, fills, chat, persona, asked, one));
    named
}

/// **What a fill the person allowed is handed to the browser with** (#5115, #5120): the value,
/// and whether the person answered a notice for this very fill that named a multi-line text box
/// or an editable area, which alone lets one take it.
pub struct Allowed {
    pub value: Secret,
    pub multi_line: bool,
    /// The other origins the key is approved for (#5230, SF-15).
    pub also: Vec<String>,
    /// **It goes in under a grant, with no notice for this fill**: the engine judges the field
    /// again as it writes (the rulings of 2026-11-06).
    pub under_a_grant: bool,
}

/// **Whether the person is asked about this fill**: a multi-line text box or an editable area
/// each time, whatever was allowed (SF-5, #5120); a field the page marks a card's or payment one
/// (the steward's decision of 2026-11-06); a field whose read doesn't say either of those (a
/// host that doesn't read them); any other field unless it is [`granted`].
fn asks(
    root: &std::path::Path,
    fills: &Fills,
    chat: Chat<'_>,
    persona: Option<&str>,
    asked: &FillAsked,
    origin: &str,
) -> bool {
    asked.field.kind != FieldKind::OneLine
        || asked.field.payment != Some(false)
        || asked.field.by_get.is_none()
        || !granted(root, fills, chat, persona, asked, origin)
}

/// **Whether a fill goes in without a notice** (#5116): a line Always wrote, allowed here, or the
/// person's Allow for this chat, each while policy still lets that scope be, and never a card's
/// key. A 2FA code goes in only under a grant made for codes (#5118), and a value never does.
fn granted(
    root: &std::path::Path,
    fills: &Fills,
    chat: Chat<'_>,
    persona: Option<&str>,
    asked: &FillAsked,
    origin: &str,
) -> bool {
    use purlis_core::sandbox::grant::Level;
    let ctx = Ctx::new(root, Env::from_process());
    let lines = purlis_core::secretfill::lines(&ctx, persona, &asked.vault, &asked.key, origin);
    if lines.card || !held_open(origin) {
        return false;
    }
    let locks = Locks::of(root);
    if asked.code {
        return fills.code_granted(chat, &asked.vault, &asked.key, origin)
            && locks.scope_removed(Level::Chat).is_none();
    }
    (lines.always && locks.scope_removed(Level::You).is_none())
        || (fills.granted(chat, &asked.vault, &asked.key, origin)
            && locks.scope_removed(Level::Chat).is_none())
}

/// **Whether a grant may hold `origin` open** (#5116): an `https` site alone. A site on this
/// machine (`http://localhost` and the like) is served by whatever listens on its port, which
/// may be a program the chat itself started, so each fill there is asked about.
fn held_open(origin: &str) -> bool {
    origin.starts_with("https://")
}

/// Why Always isn't offered to a chat that runs as no persona: it has no tools to write in.
const NO_PERSONA: &str = "Always allow isn't offered for a chat that runs as no persona";

/// Why Always isn't offered for a 2FA code: the line it writes allows a key's value, and says
/// nothing of a code.
const NO_ALWAYS_FOR_A_CODE: &str = "Always allow isn't offered for a 2FA code";

/// **What the person is offered on `wait`**: a card's key Allow once alone (SF-4); a 2FA code
/// never Always (#5118); otherwise Always too, with the line it writes, where the project lets
/// one be written for the persona.
fn offered(root: &std::path::Path, mut wait: FillWait) -> FillWait {
    let lines = purlis_core::secretfill::lines(
        &Ctx::new(root, Env::from_process()),
        wait.persona.as_deref(),
        &wait.vault,
        &wait.key,
        &wait.origin,
    );
    wait.card = lines.card;
    if wait.once_only() {
        return wait;
    }
    if wait.code {
        wait.no_always = Some(NO_ALWAYS_FOR_A_CODE.to_owned());
        return wait;
    }
    let Some(persona) = wait.persona.clone() else {
        wait.no_always = Some(NO_PERSONA.to_owned());
        return wait;
    };
    let locks = Locks::of(root);
    let text = std::fs::read_to_string(purlis_core::names::manifest(root)).unwrap_or_default();
    wait.line =
        purlis_core::secretfill::line(&text, &persona, &wait.vault, &wait.key, &wait.origin)
            .map(|(name, table)| format!("{name} = {table}"));
    wait.no_always = crate::loginasks::always_held(root, &locks, &persona).or_else(|| {
        wait.line.is_none().then(|| {
            format!(
                "Always allow isn't offered for {}, which makes no line's name",
                wait.key
            )
        })
    });
    let ctx = Ctx::new(root, Env::from_process());
    wait.vault_too = brokered::not_tagged(&ctx, Some(&persona), &wait.vault).is_some();
    wait
}

/// **`wait` raised, and the person's answer**: refused where the chat has a notice up already.
/// Where the call's asker is `gone`, the notice goes with it and nothing is filled.
fn heard(
    held: &Held,
    fills: &Fills,
    wait: FillWait,
    gone: &dyn Fn() -> bool,
) -> Result<Heard, String> {
    let session = wait.session;
    let (id, heard) = fills.raise(wait)?;
    tracing::info!("purlis: chat {session}'s secret fill asks the person");
    held.chats().asks_may_have_moved();
    // Under the presses' own hold, so a press is either wholly before this or finds the wait
    // gone: a press in that very moment has ended it, and its answer waits here.
    let settle = || {
        let _no_press = fills
            .answering
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if fills.end(id, Heard::Gone) {
            None
        } else {
            heard.try_recv().ok()
        }
    };
    let at_most = purlis_core::loginask::THE_PERSON_IS_WAITED_FOR;
    Ok(
        match crate::loginasks::waited(&heard, at_most, gone, &settle) {
            Ended::Heard(heard) => heard,
            Ended::Closed => Heard::Gone,
            Ended::NotAnswered => {
                held.chats().asks_may_have_moved();
                Heard::Gone
            }
            Ended::AskerGone => {
                tracing::info!(
                    "purlis: chat {session}'s secret fill call ended, so its notice went"
                );
                held.chats().asks_may_have_moved();
                return Err(crate::loginasks::ASKER_GONE.to_owned());
            }
        },
    )
}

/// **The value to fill, read for the chat now**; where the vault holds no such key, asked of the
/// person on a Secret ask (#5054, ADR 0109), and read again once they added it.
/// It waits on that ask for what is left of the time the person is waited for on a fill, and at
/// least as long as a brokered run waits on one. The fill is judged again once the key is added.
/// Where the call's asker is `gone` meanwhile, it stops waiting, and the ask goes from the
/// person's window unless another chat waits on it too.
fn read(
    held: &Held,
    session: u32,
    asked: &FillAsked,
    origin: &str,
    started: std::time::Instant,
    gone: &dyn Fn() -> bool,
) -> Result<Secret, Unread> {
    not_taken_over(held.browsers(), asked)?;
    let (asker, _) = held
        .chats()
        .login_asker(held.root(), session)
        .ok_or_else(|| {
            "this chat is no longer open in purlis's window, so nothing was filled".to_owned()
        })?;
    let read = || read_now(&asker, asked);
    if let Some(value) = read()? {
        return Ok(value);
    }
    let need = purlis_core::secrets::ask::Need {
        chat: Some(session),
        persona: asker.persona.clone().unwrap_or_default(),
        vault: asked.vault.clone(),
        key: asked.key.clone(),
        // What the person reads the key is for.
        tool: Some(if asked.code {
            format!("a 2FA code on {origin}, made from the seed this key holds")
        } else {
            format!("a secret fill on {origin}")
        }),
        // The chat names no purpose for a fill: the notice it answered named the key and site.
        purpose: None,
        declared: purlis_core::secrets::needs::declared(
            held.root(),
            asker.persona.as_deref().unwrap_or_default(),
            &asked.key,
        )
        .map(|need| need.purpose),
        // A fill names its vault.
        names_no_vault: false,
        // Asked for a key the vault doesn't hold (#5075's reasons are for keys it has).
        reason: None,
        no_allow: None,
    };
    let within = purlis_core::loginask::THE_PERSON_IS_WAITED_FOR
        .saturating_sub(started.elapsed())
        .max(purlis_core::secrets::ask::holds_for(asker.harness));
    let moved = || held.chats().asks_may_have_moved();
    match held
        .secret_asks()
        .hold_while(need, within, gone, &mut |_: &str| moved())
    {
        // Only the very vault and key the person allowed for this site go in: a value added
        // under other names is that key's, and a fill of it is a fill of its own.
        purlis_core::secrets::ask::Held::Filled { vault, key }
            if vault != asked.vault || key != asked.key =>
        {
            Err(format!(
                "the person added {vault}/{key} rather than {}/{}, so nothing was filled: call \
                 browser_fill_secret again naming {key} of vault {vault}",
                asked.vault, asked.key
            )
            .into())
        }
        purlis_core::secrets::ask::Held::Filled { .. } => {
            // Judged again as the project reads now: it may have changed while the key was added.
            judged(held, session, asked)?;
            not_taken_over(held.browsers(), asked)?;
            Ok(read()?.ok_or_else(|| {
                format!(
                    "vault '{}' still holds no {}, so nothing was filled",
                    asked.vault, asked.key
                )
            })?)
        }
        purlis_core::secrets::ask::Held::CallerGone { .. } => {
            tracing::info!("purlis: chat {session}'s secret fill call ended as its key was asked");
            moved();
            Err(Unread::waited_out(crate::loginasks::ASKER_GONE.to_owned()))
        }
        purlis_core::secrets::ask::Held::StillOpen { line } => {
            Err(Unread::waited_out(format!("{line}: nothing was filled")))
        }
        purlis_core::secrets::ask::Held::Declined { line }
        | purlis_core::secrets::ask::Held::Refused { line } => {
            Err(format!("{line}: nothing was filled").into())
        }
    }
}

/// **Why [`read`] has no value**, in words the chat reads, and whether only its wait for the key
/// ran out (#5242): its call ended, or stopped waiting, before the person added it. Only then
/// does the person's Allow on the fill's notice stand for its retry.
struct Unread {
    why: String,
    waited_out: bool,
}

impl Unread {
    fn waited_out(why: String) -> Self {
        Self {
            why,
            waited_out: true,
        }
    }
}

impl From<String> for Unread {
    fn from(why: String) -> Self {
        Self {
            why,
            waited_out: false,
        }
    }
}

/// **What `asked` fills, read for the chat now**: the value, or the code its seed makes (#5118),
/// worked out here as it is filled, so the seed stays. `None` where the vault holds no such key.
fn read_now(asker: &brokered::Asker, asked: &FillAsked) -> Result<Option<Secret>, String> {
    if asked.code {
        brokered::fill_code(asker, &asked.vault, &asked.key)
    } else {
        brokered::fill_value(asker, &asked.vault, &asked.key)
    }
}

/// **No value is read for a browser the person has taken over** (#5119, SF-10), which they may
/// have done while they were asked.
fn not_taken_over(
    browsers: &crate::chatbrowsers::ChatBrowsers,
    asked: &FillAsked,
) -> Result<(), String> {
    if browsers.driver_of(&asked.program) == Some(crate::chatbrowsers::BrowserDriver::Person) {
        return Err(crate::chatbrowsers::taken_over_said(&asked.browser));
    }
    Ok(())
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
/// waiting now changes nothing, a card's key takes Allow once and Keep blocked alone (SF-4), and
/// Always writes only the line the notice showed, under the rules a login's Always is held to.
pub fn answer(
    root: &std::path::Path,
    fills: &Fills,
    asked: &Asked<'_>,
    audit: Audit<'_>,
) -> Result<Pressed, String> {
    use purlis_core::sandbox::grant::Level;
    const NO_MORE: &str = "That ask has gone, and nothing was allowed.";
    const CHANGED: &str = "That ask changed, so nothing was allowed. Read it again.";
    const ONCE_ONLY: &str = "Only Allow once or Keep blocked can be pressed on this ask: it is a \
                             card's key, or a field that asks each time.";
    let _one_press = fills
        .answering
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let wait = fills.wait(asked.session, asked.id).ok_or(NO_MORE)?;
    if wait.digest() != asked.shown {
        return Err(CHANGED.to_owned());
    }
    let level = match asked.press {
        Press::Keep => {
            // Counted only with the wait it ends, in one step: a Keep that lost to the time
            // running out is never counted.
            let mut inner = fills.lock();
            if !inner.waits.iter().any(|(one, _)| one.id == wait.id) {
                return Err(NO_MORE.to_owned());
            }
            inner.kept.push((
                Whose::of(wait.session, wait.persona.as_deref()),
                wait.vault.clone(),
                wait.key.clone(),
                wait.origin.clone(),
            ));
            Fills::ended(
                &mut inner,
                wait.id,
                Heard::Kept(asked.reason.map(str::to_owned)),
            );
            drop(inner);
            return Ok(Pressed::Done {
                said: "Kept blocked. Nothing was filled.".to_owned(),
            });
        }
        Press::Once => Level::Once,
        Press::Chat | Press::Always if wait.once_only() => return Err(ONCE_ONLY.to_owned()),
        Press::Chat => Level::Chat,
        Press::Always => Level::You,
    };
    // Judged as the read judges it (#5831): a key that became a link into a vault the persona may
    // not use while the notice waited is allowed nothing, writes no line and grants no vault. The
    // notice stays up, as for every press refused, and shows the person why.
    let ctx = Ctx::new(root, Env::from_process());
    let reads_as = wait.reads_as.as_deref().unwrap_or_default();
    if let Some(why) = brokered::read_out_of_reach(&ctx, reads_as, &wait.vault, &[&wait.key]) {
        return Err(crate::loginasks::refused_at_the_press_said(&why));
    }
    let locks = Locks::of(root);
    if let Some(why) = locks.scope_removed(level) {
        return Err(why);
    }
    // Judged again as the project reads now: a card's mark or a line added since is a new notice.
    let now = offered(
        root,
        FillWait {
            card: false,
            multi_line: wait.multi_line,
            line: None,
            no_always: None,
            vault_too: false,
            ..wait.clone()
        },
    );
    // Once and this chat are drawn again only where what they are changed: a card's mark. Always
    // writes the line, so anything of it changed is a new notice.
    let moved = if asked.press == Press::Always {
        now.digest() != wait.digest()
    } else {
        now.card != wait.card
    };
    if moved {
        let said = format!(
            "The project's settings for {} changed. Check the ask, then choose again.",
            wait.key
        );
        fills.redraw(now);
        return Ok(Pressed::DrawnAgain { said });
    }
    if asked.press == Press::Always
        && let Some(why) = wait.no_always.clone()
    {
        return Err(why);
    }
    // Policy that took vault grants away since the notice was drawn holds Always's back too.
    if asked.press == Press::Always
        && wait.vault_too
        && let Some(why) = locks.vault_grants_refused()
    {
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
    let what = if wait.code {
        format!("A 2FA code from {}", wait.key)
    } else {
        wait.key.clone()
    };
    let said = match asked.press {
        Press::Always => {
            let persona = wait.persona.as_deref().ok_or(NO_MORE)?;
            let text =
                std::fs::read_to_string(purlis_core::names::manifest(root)).unwrap_or_default();
            let (name, table) =
                purlis_core::secretfill::line(&text, persona, &wait.vault, &wait.key, &wait.origin)
                    .ok_or(CHANGED)?;
            // Only the very line the notice showed is written.
            if wait.line.as_deref() != Some(format!("{name} = {table}").as_str()) {
                return Err(CHANGED.to_owned());
            }
            crate::loginasks::written(
                root,
                &locks,
                &crate::loginasks::Line {
                    persona,
                    vault: &wait.vault,
                    vault_too: wait.vault_too,
                    with: &|text| {
                        purlis_core::loginask::with_tool_line(text, persona, &name, &table)
                    },
                },
                wait.session,
                audit,
            )?;
            format!(
                "Allowed. {} goes into fields on {} from now on: the line is in the project's \
                 settings, and allowed on this machine.",
                wait.key, wait.origin
            )
        }
        Press::Chat => {
            format!(
                "Allowed for this chat. {what} goes into fields on {} in this chat without asking \
                 again; nothing was written.",
                wait.origin
            )
        }
        Press::Once | Press::Keep => format!(
            "Allowed once. {what} goes into that one field on {}; nothing was written.",
            wait.origin
        ),
    };
    // Held for the chat before its call hears the answer, under the presses' hold, so a chat
    // that closed in between holds nothing ([`Fills::forget`] waits for this press).
    if asked.press == Press::Chat {
        let mut inner = fills.lock();
        if inner.waits.iter().any(|(one, _)| one.id == wait.id) {
            let grant = (
                wait.session,
                wait.chat.clone(),
                wait.vault.clone(),
                wait.key.clone(),
                wait.origin.clone(),
            );
            if wait.code {
                inner.codes_granted.push(grant);
            } else {
                inner.granted.push(grant);
            }
        }
    }
    if !fills.end(wait.id, Heard::Allowed) {
        return Err(NO_MORE.to_owned());
    }
    Ok(Pressed::Done { said })
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
