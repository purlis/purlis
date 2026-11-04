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
//! - **A key the vault doesn't hold goes to the Secret ask** (#5054, ADR 0109) once the person
//!   allowed the fill: the call waits on that too, and fills what they add.
//! - **What's filled is what the person pressed on.** A press names the notice and carries its
//!   digest ([`FillWait::digest`]); one waits for each chat, and another call is refused.
//! - **Keep blocked fills nothing and tells the chat**, and the chat isn't asked about that key
//!   on that site again; after [`KEPT_AT_MOST`] it's asked about nothing more.
//! - **The engine judges again at the moment of filling**: the field's own frame must still be
//!   on the approved origin, and the field one line. The window's read is only what it shows.
//!
//! In memory only: a relaunch starts with none, and a chat's close takes its waits with it.

use std::sync::{Mutex, MutexGuard, PoisonError, mpsc};

use purlis_core::sandbox::policy::Locks;
use purlis_core::secrets::brokered;
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
    /// The persona the chat was started as: whose tools Always writes in.
    pub persona: Option<String>,
    /// **A card's or payment value's key** (SF-4): Allow once and Keep blocked alone.
    pub card: bool,
    /// **A multi-line text box or an editable area** (#5120, SF-5): asked each time, whatever
    /// was allowed, with Allow once and Keep blocked alone.
    pub multi_line: bool,
    /// The line Always would write, as written: `<name> = { vault = …, key = …, fill = … }`.
    pub line: Option<String>,
    /// Why Always isn't offered, where it isn't.
    pub no_always: Option<String>,
    /// The persona needs its vault allowed too: what Always then also does.
    pub vault_too: bool,
}

impl FillWait {
    /// Whether only Allow once and Keep blocked are offered: a card's key, or a field that asks
    /// each time.
    pub fn once_only(&self) -> bool {
        self.card || self.multi_line
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
            u8::from(self.vault_too),
            u8::from(self.multi_line),
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

/// A key on an origin for one chat: by chat, vault, key and origin.
type ForChat = (u32, String, String, String);

#[derive(Debug, Default)]
struct Inner {
    next: u32,
    waits: Vec<(FillWait, Option<mpsc::Sender<Heard>>)>,
    /// What the person kept blocked.
    kept: Vec<ForChat>,
    /// What the person allowed for the chat (#5116): in memory, for the chat's life.
    granted: Vec<ForChat>,
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

    /// Whether the person allowed this key on this origin for this chat (#5116).
    fn granted(&self, session: u32, vault: &str, key: &str, origin: &str) -> bool {
        self.lock()
            .granted
            .iter()
            .any(|(chat, v, k, o)| *chat == session && v == vault && k == key && o == origin)
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
        inner.kept.retain(|(chat, ..)| *chat != session);
        inner.granted.retain(|(chat, ..)| *chat != session);
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
/// has a web origin, the field takes typing, and the project lets the key be offered.
fn judged(held: &Held, session: u32, asked: &FillAsked) -> Result<(String, Vec<String>), String> {
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
    Ok((origin, warnings))
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
/// the browsers', and then on a Secret ask where the vault holds no such key.
pub fn asked(held: &Held, session: u32, asked: &FillAsked, now: u64) -> Result<Allowed, String> {
    let (origin, warnings) = judged(held, session, asked)?;
    // SF-5: a multi-line text box or an editable area asks each time, whatever was allowed.
    let multi_line = asked.field.kind != FieldKind::OneLine;
    let fills = held.secret_fills();
    if fills.kept(session, &asked.vault, &asked.key, &origin) {
        return Err(kept_said(&asked.key, &origin, None));
    }
    let persona = held
        .chats()
        .persona_of(session)
        .and_then(|persona| persona.started_as);
    if asks(
        held.root(),
        fills,
        session,
        persona.as_deref(),
        asked,
        &origin,
    ) {
        let wait = offered(
            held.root(),
            FillWait {
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
                persona,
                card: false,
                multi_line,
                line: None,
                no_always: None,
                vault_too: false,
            },
        );
        match heard(held, fills, wait)? {
            Heard::Allowed => {
                // Judged again as the project reads now: policy may have changed while it
                // waited.
                judged(held, session, asked)?;
            }
            Heard::Kept(reason) => return Err(kept_said(&asked.key, &origin, reason.as_deref())),
            Heard::Gone => return Err(NOT_ANSWERED.to_owned()),
        }
    }
    Ok(Allowed {
        value: read(held, session, asked)?,
        multi_line,
    })
}

/// **What a fill the person allowed is handed to the browser with** (#5115, #5120): the value,
/// and whether the person answered a notice for this very fill that named a multi-line text box
/// or an editable area, which alone lets one take it.
pub struct Allowed {
    pub value: Secret,
    pub multi_line: bool,
}

/// **Whether the person is asked about this fill**: a multi-line text box or an editable area
/// each time, whatever was allowed (SF-5, #5120); any other field unless it is [`granted`].
fn asks(
    root: &std::path::Path,
    fills: &Fills,
    session: u32,
    persona: Option<&str>,
    asked: &FillAsked,
    origin: &str,
) -> bool {
    asked.field.kind != FieldKind::OneLine || !granted(root, fills, session, persona, asked, origin)
}

/// **Whether a fill goes in without a notice** (#5116): a line Always wrote, allowed here, or the
/// person's Allow for this chat, each while policy still lets that scope be, and never a card's
/// key.
fn granted(
    root: &std::path::Path,
    fills: &Fills,
    session: u32,
    persona: Option<&str>,
    asked: &FillAsked,
    origin: &str,
) -> bool {
    use purlis_core::sandbox::grant::Level;
    let lines = purlis_core::secretfill::lines(root, persona, &asked.vault, &asked.key, origin);
    if lines.card {
        return false;
    }
    let locks = Locks::of(root);
    (lines.always && locks.scope_removed(Level::You).is_none())
        || (fills.granted(session, &asked.vault, &asked.key, origin)
            && locks.scope_removed(Level::Chat).is_none())
}

/// Why Always isn't offered to a chat that runs as no persona: it has no tools to write in.
const NO_PERSONA: &str = "Always allow isn't offered for a chat that runs as no persona";

/// **What the person is offered on `wait`**: a card's key Allow once alone (SF-4); otherwise
/// Always too, with the line it writes, where the project lets one be written for the persona.
fn offered(root: &std::path::Path, mut wait: FillWait) -> FillWait {
    let lines = purlis_core::secretfill::lines(
        root,
        wait.persona.as_deref(),
        &wait.vault,
        &wait.key,
        &wait.origin,
    );
    wait.card = lines.card;
    if wait.once_only() {
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
fn heard(held: &Held, fills: &Fills, wait: FillWait) -> Result<Heard, String> {
    let session = wait.session;
    let (id, heard) = fills.raise(wait)?;
    tracing::info!("purlis: chat {session}'s secret fill asks the person");
    held.chats().asks_may_have_moved();
    Ok(
        match heard.recv_timeout(purlis_core::loginask::THE_PERSON_IS_WAITED_FOR) {
            Ok(heard) => heard,
            Err(mpsc::RecvTimeoutError::Disconnected) => Heard::Gone,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Under the presses' own hold, so a press is either wholly before this or finds
                // the wait gone: a press in that very moment has ended it, and its answer waits
                // here.
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
        },
    )
}

/// **The value to fill, read for the chat now**; where the vault holds no such key, asked of the
/// person on a Secret ask (#5054, ADR 0109), and read again once they added it.
fn read(held: &Held, session: u32, asked: &FillAsked) -> Result<Secret, String> {
    let (asker, _) = held
        .chats()
        .login_asker(held.root(), session)
        .ok_or_else(|| {
            "this chat is no longer open in purlis's window, so nothing was filled".to_owned()
        })?;
    let read = || brokered::fill_value(&asker, &asked.vault, &asked.key);
    if let Some(value) = read()? {
        return Ok(value);
    }
    let need = purlis_core::secrets::ask::Need {
        chat: session,
        persona: asker.persona.clone().unwrap_or_default(),
        vault: asked.vault.clone(),
        key: asked.key.clone(),
        tool: None,
    };
    let within = purlis_core::secrets::ask::holds_for(asker.harness);
    let moved = || held.chats().asks_may_have_moved();
    match held
        .secret_asks()
        .hold(need, within, &mut |_: &str| moved())
    {
        purlis_core::secrets::ask::Held::Filled => read()?.ok_or_else(|| {
            format!(
                "vault '{}' still holds no {}, so nothing was filled",
                asked.vault, asked.key
            )
        }),
        purlis_core::secrets::ask::Held::Declined { line }
        | purlis_core::secrets::ask::Held::StillOpen { line } => {
            Err(format!("{line}: nothing was filled"))
        }
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
            fills.lock().kept.push((
                wait.session,
                wait.vault.clone(),
                wait.key.clone(),
                wait.origin.clone(),
            ));
            fills.end(wait.id, Heard::Kept(asked.reason.map(str::to_owned)));
            return Ok(Pressed::Done {
                said: "Kept blocked. Nothing was filled.".to_owned(),
            });
        }
        Press::Once => Level::Once,
        Press::Chat | Press::Always if wait.once_only() => return Err(ONCE_ONLY.to_owned()),
        Press::Chat => Level::Chat,
        Press::Always => Level::You,
    };
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
    if now.digest() != wait.digest() {
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
                "Allowed for this chat. {} goes into fields on {} in this chat without asking \
                 again; nothing was written.",
                wait.key, wait.origin
            )
        }
        Press::Once | Press::Keep => format!(
            "Allowed once. {} goes into that one field on {}; nothing was written.",
            wait.key, wait.origin
        ),
    };
    // Held for the chat before its call hears the answer, under the presses' hold, so a chat
    // that closed in between holds nothing ([`Fills::forget`] waits for this press).
    if asked.press == Press::Chat {
        let mut inner = fills.lock();
        if inner.waits.iter().any(|(one, _)| one.id == wait.id) {
            inner.granted.push((
                wait.session,
                wait.vault.clone(),
                wait.key.clone(),
                wait.origin.clone(),
            ));
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
