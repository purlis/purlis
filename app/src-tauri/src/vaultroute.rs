// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A vault refused for a chat's persona offers a way forward** (#1430).
//!
//! A sandboxed chat may use only the vaults tagged for the persona the app started it as
//! (`purlis_core::secrets::brokered::authorise`, D-1407-1). A chat that asks for another is
//! refused, and that used to be where it ended. Here the refusal becomes a Notice on the chat's
//! tab with two ways out, each the person's press and never the chat's:
//!
//! - **Allow** lets that persona's chats use the vault in this project on this machine
//!   ([`allow`], D-1430-1). It is kept beside the project, in the file no sandboxed chat writes
//!   (`sandbox::local`), never in the vault registry, whose shared half is committed. It is
//!   audited before it is kept, listed in Settings › Sandbox › Granted, revocable there, and
//!   forbidden where an administrator's policy says so (`vault-grants`). The next brokered run
//!   reads it, so the chat does not restart.
//! - **Keep blocked** puts the Notice away for the rest of the chat ([`Refusals::keep_blocked`]).
//!
//! The third way, a chat opened as the vault's persona, is not a button here (D-1430-12): the
//! chat's own refusal names it, and a direct dispatch from the Notice is #1438's.
//!
//! **Nothing a chat sends reaches any of it.** The refusal is noted by the app as it refuses
//! ([`refused`]), from its own record of the chat and the registry; Allow takes the chat's
//! persona from that record again, takes only a vault this chat was in fact refused, and no
//! line on the hook channel is a grant.
//!
//! # Telling the chat, and when (D-1430-3, D-1430-11)
//!
//! Once a vault is allowed the chat is told what now runs: one line of purlis's
//! own fixed text ([`told_allowed`]), sent as the person's message, a bracketed paste and then
//! Enter. **This is a second gated send beside smart close's** (ADR 0064, an exception to
//! ADR 0061), and it is gated the same way, by the board and never the screen ([`when`]):
//!
//! - a chat that is waiting, and asking nothing, is sent it now;
//! - a chat mid-turn has it queued until its turn ends ([`reported`]), because a line typed
//!   into a turn lands in whatever the harness is doing;
//! - a chat that is asking the person something (a permission prompt, a question), or whose
//!   state purlis does not know, is sent nothing: it is handed the line's first part
//!   ([`allowed_outcome`], #1987) when its next turn starts. **An Enter never lands on a
//!   harness's prompt**: there it would confirm the highlighted choice.

use std::sync::{Arc, Mutex, PoisonError};

use purlis_core::sandbox;
use purlis_core::secrets::brokered::{self, NotTagged};
use purlis_core::secrets::{Ctx, Env};
use purlis_core::state::State;

use crate::planes::{PlaneId, Planes};

/// The event the window is sent when a chat was refused a vault its persona is not tagged for.
pub const REFUSED: &str = "chat-vault-refused";

/// A vault a chat was refused, as the Notice on its tab shows it: names only. No key, no
/// value, and nothing of the command the chat ran.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct VaultRefused {
    pub plane: PlaneId,
    pub session: u32,
    pub vault: String,
    /// The persona the app started the chat as.
    pub persona: String,
    /// The persona the vault registry tags the vault for, where it tags one: the registry's
    /// own text, on one line.
    pub tagged_for: Option<String>,
    /// That persona, where the project defines it: one the chat's refusal told it how to
    /// dispatch to.
    pub dispatch_to: Option<String>,
    /// Why a policy forbids Allow, where one does: the Notice then offers none.
    pub locked: Option<String>,
    /// The browser login the persona declares with this vault, where it does: the one case
    /// purlis knows a vault is asked for a login. A `secret` command names none (#4595).
    #[specta(optional)]
    pub login: Option<String>,
}

/// Told each newly refused vault. The event carries its project.
pub type Teller = Arc<dyn Fn(VaultRefused) + Send + Sync + 'static>;

/// The most refused vaults one chat holds a Notice for at once: the oldest goes first.
pub const AT_MOST_PER_CHAT: usize = 5;

/// The vaults each of a project's chats was refused, until the person answers. In memory only:
/// a relaunch starts with none, and the chat's next try raises it again.
#[derive(Debug, Default)]
pub struct Refusals {
    /// Each chat's refusal, with when it was first heard, in seconds since 1970: what orders
    /// its ask in the Inbox (#5160).
    held: Mutex<Vec<(u32, NotTagged, u64)>>,
    /// What each chat is told once its turn ends ([`reported`]): the lines of an Allow made
    /// while it was mid-turn, oldest first.
    queued: Mutex<Vec<(u32, String)>>,
    /// The vaults the person kept blocked for each chat: its next try raises no ask, so a chat
    /// can't wear them down. In memory, as the refusals are, and gone with the chat.
    kept: Mutex<Vec<(u32, String)>>,
}

impl Refusals {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<(u32, NotTagged, u64)>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn kept_lock(&self) -> std::sync::MutexGuard<'_, Vec<(u32, String)>> {
        self.kept.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// **Keep blocked**: puts chat `session`'s refusal of `vault` away, and its later tries at
    /// `vault` raise no ask for as long as the chat is open: not this one, and no Run grant's,
    /// whose Always allow would give the persona the vault.
    pub fn keep_blocked(&self, session: u32, vault: &str) {
        // Kept first, so a refusal heard meanwhile finds it, and is not left up.
        {
            let mut kept = self.kept_lock();
            if !kept
                .iter()
                .any(|(chat, one)| *chat == session && one == vault)
            {
                kept.push((session, vault.to_owned()));
            }
        }
        self.put_away(session, vault);
    }

    /// Whether the person kept `vault` blocked for chat `session`.
    pub fn kept_blocked(&self, session: u32, vault: &str) -> bool {
        self.kept_lock()
            .iter()
            .any(|(chat, one)| *chat == session && one == vault)
    }

    /// Notes that chat `session` was refused `not`, and answers whether that is news: one
    /// Notice per chat and vault, however often the chat tries again.
    pub fn note(&self, session: u32, not: NotTagged) -> bool {
        if self.kept_blocked(session, &not.vault) {
            return false;
        }
        let mut held = self.lock();
        if held
            .iter()
            .any(|(chat, one, _)| *chat == session && one.vault == not.vault)
        {
            return false;
        }
        held.push((session, not, now_secs()));
        let mine = held.iter().filter(|(chat, ..)| *chat == session).count();
        if mine > AT_MOST_PER_CHAT
            && let Some(oldest) = held.iter().position(|(chat, ..)| *chat == session)
        {
            held.remove(oldest);
        }
        true
    }

    /// What chat `session` was refused, oldest first.
    pub fn of(&self, session: u32) -> Vec<NotTagged> {
        self.lock()
            .iter()
            .filter(|(chat, ..)| *chat == session)
            .map(|(_, one, _)| one.clone())
            .collect()
    }

    /// **Every chat's refusals**, oldest first, each with when it was first heard: what the
    /// asks registry lists (#5160).
    pub fn every(&self) -> Vec<(u32, NotTagged, u64)> {
        self.lock().clone()
    }

    /// Whether chat `session` holds a refusal of `vault`.
    pub fn holds(&self, session: u32, vault: &str) -> bool {
        self.lock()
            .iter()
            .any(|(chat, one, _)| *chat == session && one.vault == vault)
    }

    /// Puts chat `session`'s refusal of `vault` away: answered, or kept blocked.
    pub fn put_away(&self, session: u32, vault: &str) {
        self.lock()
            .retain(|(chat, one, _)| !(*chat == session && one.vault == vault));
    }

    /// Puts away every chat's refusal of `vault` for `persona`: an Allow for that pair answered
    /// them all (qw385), so no other chat's Notice offers an Allow that is already made.
    /// Answers the chats whose refusal it put away.
    pub fn put_away_cured(&self, persona: &str, vault: &str) -> Vec<u32> {
        let mut cured = Vec::new();
        self.lock().retain(|(chat, one, _)| {
            let answered = one.vault == vault && one.persona == persona;
            if answered {
                cured.push(*chat);
            }
            !answered
        });
        cured
    }

    /// Chat `session` is closed: its refusals and anything queued for it go with it, so none
    /// outlives its chat or reaches a later chat dealt the same number.
    pub fn forget(&self, session: u32) {
        self.lock().retain(|(chat, ..)| *chat != session);
        self.kept_lock().retain(|(chat, _)| *chat != session);
        self.queue().retain(|(chat, _)| *chat != session);
    }

    fn queue(&self) -> std::sync::MutexGuard<'_, Vec<(u32, String)>> {
        self.queued.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The lines that wait for chat `session`'s turn to end, in order.
    #[cfg(test)]
    pub fn queued_lines_for(&self, session: u32) -> Vec<String> {
        self.queue()
            .iter()
            .filter(|(chat, _)| *chat == session)
            .map(|(_, line)| line.clone())
            .collect()
    }

    /// How many lines wait for chat `session`'s turn to end.
    #[cfg(test)]
    pub fn queued_for(&self, session: u32) -> usize {
        self.queue()
            .iter()
            .filter(|(chat, _)| *chat == session)
            .count()
    }
}

/// When a line of purlis's own is sent to a chat ([`when`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sending {
    /// Now: the chat is waiting for the person, and asking nothing.
    Now,
    /// When its turn ends: it is mid-turn.
    AtTurnsEnd,
    /// Never by purlis: the chat is asking the person something, has ended, or has reported
    /// no state. The person tells it.
    Never,
}

/// **When a chat in `state` is sent a line**, by the board's answer and never the screen's, as
/// smart close's prompt is (ADR 0064). `asking` is the board's: a permission prompt or a
/// question is open, where an Enter would answer it.
pub fn when(state: State, asking: bool) -> Sending {
    match state {
        _ if asking => Sending::Never,
        State::Waiting => Sending::Now,
        State::Running => Sending::AtTurnsEnd,
        State::Unknown | State::Done | State::Failed => Sending::Never,
    }
}

/// What became of the line an Allow tells the chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Told {
    Sent,
    Queued,
    /// Nothing was sent, and nothing will be: the person tells the chat.
    NotSent,
}

/// Tells chat `session` `text`: sent now through `send`, queued in `refusals` for its turn's
/// end, or not at all ([`when`]). A write that fails is not sent, and is not tried again.
///
/// **`glance` reads the board, and it is asked under the queue's lock**, which
/// [`reported_sending`] takes before it asks the board too, as smart close's `begin` and
/// `reported` do. So a turn that ends while this runs is never missed by both: either the
/// board already says waiting here and the line is sent now, or it says running, the line is
/// queued before the lock is let go, and the turn's end, waiting on that lock, finds it.
pub fn tell(
    refusals: &Refusals,
    session: u32,
    glance: impl FnOnce() -> (State, bool),
    text: String,
    send: impl FnOnce(u32, &str) -> Result<(), String>,
) -> Told {
    let mut queue = refusals.queue();
    let glance = glance();
    match when(glance.0, glance.1) {
        Sending::Now => match send(session, &text) {
            Ok(()) => Told::Sent,
            Err(_) => Told::NotSent,
        },
        Sending::AtTurnsEnd => {
            queue.push((session, text));
            Told::Queued
        }
        Sending::Never => Told::NotSent,
    }
}

/// **Tells `line` to chat `pressed` and to each chat in `cured`** (review of #1967): one Allow
/// answered every chat of the persona refused that vault, and each is told the same fixed line,
/// each as its own board allows ([`tell`]). Answers what became of the line to `pressed`, which
/// the Notice says.
pub fn tell_every(
    refusals: &Refusals,
    pressed: u32,
    cured: &[u32],
    glance: &dyn Fn(u32) -> (State, bool),
    line: &str,
    send: &dyn Fn(u32, &str) -> Result<(), String>,
) -> Told {
    let told = tell(refusals, pressed, || glance(pressed), line.to_owned(), send);
    for &other in cured.iter().filter(|other| **other != pressed) {
        tell(refusals, other, || glance(other), line.to_owned(), send);
    }
    told
}

/// A report reached the project for chat `session`: what was queued for its turn's end is
/// sent once the board (`glance`, asked under the queue's lock, as [`tell`] asks it) says it
/// is waiting and asking nothing. While it runs on, or asks the person something, the lines
/// stay queued; a chat that has ended has them dropped.
pub fn reported_sending(
    refusals: &Refusals,
    session: u32,
    glance: impl FnOnce() -> (State, bool),
    mut send: impl FnMut(u32, &str) -> Result<(), String>,
) {
    let mut queue = refusals.queue();
    if !queue.iter().any(|(chat, _)| *chat == session) {
        return;
    }
    match glance() {
        (State::Waiting, false) => {}
        (State::Done | State::Failed, _) => {
            queue.retain(|(chat, _)| *chat != session);
            return;
        }
        _ => return,
    }
    let mut mine = Vec::new();
    queue.retain(|(chat, text)| {
        if *chat == session {
            mine.push(text.clone());
            false
        } else {
            true
        }
    });
    // One paste and one Enter for all of them, so a second line never lands on the turn the
    // first began.
    let _ = send(session, &mine.join("\n"));
}

/// [`reported_sending`] for chat `session` of `held`, as its hooks report.
pub fn reported(held: &crate::planes::Held, session: u32) {
    reported_sending(
        held.vault_refusals(),
        session,
        || {
            let glance = held.board().glance(session);
            (glance.state, glance.asking)
        },
        |chat, text| held.type_in_turn(chat, sent_as(text).as_bytes()),
    );
}

/// The bytes a line is sent as: one bracketed paste, then Enter, in one write, as smart
/// close's prompt is. Nothing in the paste can submit or end it early
/// (`crate::curation::bracketed`).
pub fn sent_as(text: &str) -> String {
    format!("{}\r", crate::curation::bracketed(text))
}

/// `not` as the window shows it for chat `session` of `plane`, with what policy says of Allow
/// now.
pub fn shown(plane: &PlaneId, session: u32, not: NotTagged) -> VaultRefused {
    VaultRefused {
        login: brokered::login_with(plane.root(), &not.persona, &not.vault),
        locked: sandbox::policy::Locks::of(plane.root()).vault_grants_refused(),
        dispatch_to: brokered::dispatchable(&Ctx::new(plane.root(), Env::from_process()), &not),
        plane: plane.clone(),
        session,
        vault: not.vault,
        persona: not.persona,
        tagged_for: not
            .tagged_for
            .as_deref()
            .map(purlis_core::personas::one_line),
    }
}

/// What the app does as it refuses `asker` the vault it asked for: where the vault is
/// registered and not one the chat's persona may use, the refusal is noted and, the first
/// time, told to the window. Everything it notes is the app's record of the chat and the
/// registry's; the line gave only the vault's name, which must be one the registry holds.
///
/// Only for a chat whose run started sandboxed, by the record of how it started (#2692), never
/// by what the window holds for it now: `unsandboxed` is that record saying it started without
/// the sandbox ([`crate::chats::Chats::started_unsandboxed`]). Such a chat reads its vault
/// itself, and is not refused here. A run whose record cannot be read is told, as a sandboxed
/// one is: the Notice decides nothing, and its Allow is the person's.
pub fn refused(
    plane: &PlaneId,
    refusals: &Refusals,
    asker: &brokered::Asker,
    unsandboxed: bool,
    vault: &str,
    tell: &(dyn Fn(VaultRefused) + Send + Sync),
) {
    if unsandboxed {
        return;
    }
    let ctx = Ctx::new(&asker.root, asker.env.clone());
    let Some(not) = brokered::not_tagged(&ctx, asker.persona.as_deref(), vault) else {
        return;
    };
    if refusals.note(asker.chat, not.clone()) {
        tell(shown(plane, asker.chat, not));
    }
}

/// What a press on the Notice answered: the sentence it then says.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct VaultAnswered {
    pub said: String,
}

/// Seconds since 1970, now.
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Writes the audit of a grant, and answers whether it was written.
type Audit<'a> = &'a dyn Fn(Option<u32>, &sandbox::grant::Audited<'_>) -> Result<(), String>;

/// A chat as the app recorded it: the persona it was started as, and the name its tab shows.
pub struct Chat<'a> {
    pub session: u32,
    pub persona: Option<&'a str>,
    pub name: Option<String>,
}

/// **Lets `chat`'s persona use `vault` in the project at `root` on this machine** (#1430).
///
/// - The persona is the app's record of the chat, never the window's word.
/// - Only a vault this chat was refused, and that is still registered and still not the
///   persona's: Allow follows a refusal, and nothing else can ask for it.
/// - Never past a policy lock (`vault-grants`).
/// - Audited before it is kept: an unaudited grant is no grant.
///
/// It is kept in `sandbox::local`, which the next brokered run reads: nothing restarts.
///
/// **A press never loops** (qw385): where the chat's persona may use the vault already (its
/// tag, or an Allow made from another chat's Notice), the press answers [`Allowed::Already`]
/// and puts the Notice away, whatever this chat holds. A grant made puts away every chat's
/// refusal it answered ([`Refusals::put_away_cured`]).
fn allow(
    root: &std::path::Path,
    refusals: &Refusals,
    chat: &Chat<'_>,
    vault: &str,
    audit: Audit<'_>,
    at: u64,
) -> Result<Allowed, String> {
    let session = chat.session;
    if let Some(persona) = chat.persona.filter(|p| !p.is_empty())
        && brokered::authorise(&Ctx::new(root, Env::from_process()), Some(persona), vault).is_ok()
    {
        refusals.put_away(session, vault);
        return Ok(Allowed::Already(sandbox::local::VaultGrant {
            vault: vault.to_owned(),
            persona: persona.to_owned(),
        }));
    }
    if !refusals.holds(session, vault) {
        return Err(
            "purlis allowed nothing: the chat was not refused that vault. A vault is allowed \
             from the ask its refusal raises."
                .to_owned(),
        );
    }
    let ctx = Ctx::new(root, Env::from_process());
    let Some(not) = brokered::not_tagged(&ctx, chat.persona, vault) else {
        refusals.put_away(session, vault);
        return Err(
            "There is nothing to allow now: the vault is gone from this project, or the chat's \
             persona may use it already."
                .to_owned(),
        );
    };
    if let Some(why) = sandbox::policy::Locks::of(root).vault_grants_refused() {
        return Err(why);
    }
    let grant = sandbox::local::VaultGrant {
        vault: not.vault,
        persona: not.persona,
    };
    let target = grant.target();
    audit(
        Some(session),
        &sandbox::grant::Audited {
            granted: true,
            what: sandbox::local::VAULT,
            target: &target,
            level: sandbox::grant::Level::You,
        },
    )?;
    // Kept blocked for this chat while the audit waited on the session host (#5617).
    if refusals.kept_blocked(session, vault) {
        let taken_back = audit(
            Some(session),
            &sandbox::grant::Audited {
                granted: false,
                what: sandbox::local::VAULT,
                target: &target,
                level: sandbox::grant::Level::You,
            },
        );
        if let Err(unrecorded) = taken_back {
            tracing::warn!("purlis: a vault Keep blocked overtook was not audited ({unrecorded})");
        }
        return Err(format!(
            "You kept vault {vault} blocked for this chat, so nothing was allowed."
        ));
    }
    sandbox::local::grant_vault(root, &grant.vault, &grant.persona)
        .map_err(|why| format!("purlis could not keep it, so nothing was allowed: {why}"))?;
    if let Err(why) = sandbox::local::record_made(
        root,
        sandbox::local::Made {
            what: sandbox::local::VAULT.to_owned(),
            target,
            level: sandbox::grant::Level::You.word().to_owned(),
            at,
            chat: chat.name.clone(),
        },
    ) {
        // The grant stands, and is audited; only the Granted list's "when" is lost.
        tracing::warn!("purlis: a vault grant was kept without when it was made ({why})");
    }
    refusals.put_away(session, vault);
    let cured = refusals.put_away_cured(&grant.persona, &grant.vault);
    Ok(Allowed::Made { grant, cured })
}

/// What a press on Allow did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Allowed {
    /// The grant is made now, and the chat is to be told what it opens; so is each other chat
    /// in `cured`, whose refusal of the same vault for the same persona it answered.
    Made {
        grant: sandbox::local::VaultGrant,
        cured: Vec<u32>,
    },
    /// The persona may use the vault already: nothing was changed, and the chat is not told.
    Already(sandbox::local::VaultGrant),
}

/// What the chat is told once its persona is allowed a vault: the person's own message, saying
/// exactly what now runs (qw385), never "run it again" for a command an Allow does not open.
/// purlis's own fixed text around two names: the persona the app recorded and a vault name the
/// registry holds.
///
/// The Notice is raised only where the app refused the vault for the chat's persona (a
/// brokered `secret exec`, or `secret list <vault>`), which is what the Allow opens. Reading a
/// value in the chat stays refused, and the line says so.
///
/// `tooled` is whether the persona declares a tool with the vault
/// ([`brokered::declares_a_tool_with`]): only then does the line name `secret exec`.
pub fn told_allowed(grant: &sandbox::local::VaultGrant, tooled: bool) -> String {
    let (persona, vault) = (&grant.persona, &grant.vault);
    let opens = if !tooled {
        format!(
            "`purlis secret list {vault}` now lists its key names here. Reading a value \
             directly (`secret get`, `secret cp`) stays refused in a sandboxed chat."
        )
    } else if brokered::runs_a_chats_command_here() {
        format!(
            "`purlis secret list {vault}` now lists its key names here, and the tools {persona} \
             declares with it now run (`purlis secret exec {vault} … -- <tool>`). Reading a \
             value directly (`secret get`, `secret cp`) stays refused in a sandboxed chat."
        )
    } else {
        format!(
            "`purlis secret list {vault}` now lists its key names here. Reading a value \
             directly (`secret get`, `secret cp`) stays refused in a sandboxed chat, and a \
             command that uses one runs in a terminal outside the chat on this system."
        )
    };
    format!("{} {opens}", allowed_outcome(grant).line())
}

/// **The Ask an Allow of `grant` answered** (#1987): the line [`told_allowed`] starts with, and
/// what a chat that is not sent it now is handed at its next turn.
pub fn allowed_outcome(grant: &sandbox::local::VaultGrant) -> purlis_core::askoutcome::Outcome {
    use purlis_core::askoutcome::{About, Answer, Outcome, Scope};
    Outcome {
        about: About::Vault {
            vault: grant.vault.clone(),
        },
        answer: Answer::Allowed {
            scope: Scope::Persona {
                persona: grant.persona.clone(),
            },
        },
    }
}

/// What the Notice says once a vault is allowed, by what became of the line to the chat.
///
/// **"The chat", never "this chat"** (#1538): the Notice is drawn on a tab whose chat is not
/// on screen too (a task's, `NoticeOf`), where "this chat" would read as the one in front.
/// The Notice's own sentence says which chat it is; the answer under it names none.
pub fn allowed_said(grant: &sandbox::local::VaultGrant, told: Told) -> String {
    let then = match told {
        Told::Sent => "The chat is told what now runs.",
        Told::Queued => "The chat is told what now runs when its turn ends.",
        Told::NotSent => "The chat is told when it next starts a turn.",
    };
    format!(
        "Allowed. Chats opened as {} can use vault {} in this project on this machine. {then} \
         It does not restart.",
        grant.persona, grant.vault
    )
}

/// What the Notice says where the persona may use the vault already (qw385): nothing changed.
pub fn already_said(grant: &sandbox::local::VaultGrant) -> String {
    format!(
        "Already allowed. Chats opened as {} can use vault {} in this project on this machine. \
         Nothing was changed.",
        grant.persona, grant.vault
    )
}

/// The vaults chat `session` was refused and the person has not answered, oldest first.
#[tauri::command]
#[specta::specta]
pub fn vault_refusals(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<Vec<VaultRefused>, String> {
    let held = planes.held(&plane)?;
    Ok(held
        .vault_refusals()
        .of(session)
        .into_iter()
        .map(|not| shown(&plane, session, not))
        .collect())
}

/// **Allow** on a refused vault's Notice: lets the chat's persona use `vault` in this project
/// on this machine, audited, and tells the chat to run its command again, now or when its turn
/// ends, or leaves that to the person ([`tell`]). No restart.
#[tauri::command]
#[specta::specta]
pub async fn allow_refused_vault(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    vault: String,
) -> Result<VaultAnswered, String> {
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    // Off the window's thread (#1949): the grant is audited and kept on disk.
    crate::off_the_window("keeping your Allow", move || {
        let root = held.root().to_path_buf();
        let open = held
            .chats()
            .open_now()
            .into_iter()
            .find(|open| open.session == session)
            .ok_or_else(|| format!("purlis allowed nothing for chat {session}: it is not open."))?;
        let (grant, cured) = match allow(
            &root,
            held.vault_refusals(),
            &Chat {
                session,
                persona: open.persona.as_deref(),
                name: held.chats().shown_name(session),
            },
            &vault,
            &held.audit_then_record(network.as_ref()),
            now_secs(),
        )? {
            Allowed::Made { grant, cured } => (grant, cured),
            Allowed::Already(grant) => {
                return Ok(VaultAnswered {
                    said: already_said(&grant),
                });
            }
        };
        let line = told_allowed(
            &grant,
            brokered::declares_a_tool_with(&root, &grant.persona, &grant.vault),
        );
        let told = tell_every(
            held.vault_refusals(),
            session,
            &cured,
            &|chat| {
                let glance = held.board().glance(chat);
                (glance.state, glance.asking)
            },
            &line,
            &|chat, text| held.type_in_turn(chat, sent_as(text).as_bytes()),
        );
        // Not sent now and not queued: handed to its next turn instead, so it is told (#1987).
        if told == Told::NotSent {
            held.chats().tell_outcome(session, allowed_outcome(&grant));
        }
        Ok(VaultAnswered {
            said: allowed_said(&grant, told),
        })
    })
    .await
}

/// **Keep blocked** on a refused vault's Notice: puts it away, and grants nothing. The chat's
/// next turn is told in one line not to retry, with `reason`, what the person typed for it, if
/// anything (#1987). Its next try at the vault raises nothing, for as long as the chat is open.
///
/// **On a vault its persona already holds** (#5801), the chat's runs on it are not held, so
/// neither the chat nor the person is told it was kept blocked: the chat hears that its persona
/// may use the vault and not to use it unless the person says to, and the press answers what
/// the person reads beside it, which says where the vault is held. None otherwise.
#[tauri::command]
#[specta::specta]
pub async fn keep_vault_blocked(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    vault: String,
    reason: Option<String>,
) -> Result<Option<VaultAnswered>, String> {
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    // Off the window's thread (#5424): a Run grant's ask open on the vault is answered Don't
    // allow, and that answer is audited on disk.
    crate::off_the_window("keeping the vault blocked", move || {
        Ok(kept_vault_blocked_said(
            &held,
            network.as_ref(),
            session,
            &vault,
            reason.as_deref(),
        ))
    })
    .await
}

/// [`keep_vault_blocked`]'s work: whether the chat's persona holds `vault` is decided **once, before
/// the press** (review of #5934), from the chat as it runs now, so a chat that closes meanwhile is
/// still the chat it was, and every run the press answers hears the same as the chat. Answers
/// what the person reads beside the press, where the persona holds the vault.
pub(crate) fn kept_vault_blocked_said(
    held: &crate::planes::Held,
    network: Option<&purlis_core::sandboxblock::record::Record>,
    session: u32,
    vault: &str,
    reason: Option<&str>,
) -> Option<VaultAnswered> {
    let persona = held
        .chats()
        .open_now()
        .into_iter()
        .find(|open| open.session == session)
        .and_then(|open| open.persona);
    // The person's reason goes with the decision, so a run it answers tells it too (#5934).
    let kept =
        kept_blocked_but_held(held.root(), persona.as_deref(), vault).map(|kept| KeptButHeld {
            reason: purlis_core::askoutcome::reason(reason),
            ..kept
        });
    let back = keep_blocked_pressed(held, network, session, vault, kept.as_ref());
    if let Some(kept) = kept {
        return Some(told_kept_but_held(
            held,
            session,
            vault,
            kept,
            back.told.contains(&session),
        ));
    }
    crate::askoutcome::kept_blocked(
        held.chats(),
        session,
        (crate::sandboxing::GrantWhat::Vault, vault),
        reason,
    );
    None
}

/// **Audits each Run grant Keep blocked took back** (#5617), as one removed from the Allowed list
/// is: taken back, at its own level, naming its keys. It was taken back whether or not the audit
/// is written, so a failure is said in the log.
pub(crate) fn audit_kept_back(
    held: &crate::planes::Held,
    taken_back: &[purlis_core::secrets::ask::run::GivenRun],
) {
    for one in taken_back {
        let taken_back = purlis_core::secrets::ask::run::Audited::revoked(&one.bound, one.once);
        // Given ahead of its run, its keys are not in its command yet: named as its grant was.
        let taken_back = match &one.ahead_keys {
            Some(keys) => taken_back.handing(keys),
            None => taken_back,
        };
        let audited = held
            .hooks()
            .record_run_grant(held.root(), one.bound.chat, &taken_back);
        if let Err(unrecorded) = audited {
            tracing::warn!(
                "purlis: a Run grant Keep blocked took back was not audited ({unrecorded})"
            );
        }
    }
}

/// **The person pressed Keep blocked on `vault` for chat `session`** (#5617). Held at once: the
/// chat's later tries raise nothing and its grants on the vault are taken back, though a press on
/// another ask holds the answers, its audit perhaps waiting on a stalled session host. A press
/// that comes to the answers after this gives nothing on the vault ([`crate::secretasks`]). The
/// asks open on it are then answered with [`crate::planes::Held::run_answers`] held, so this never
/// lands between an Always allow's tool line and its answer; a grant a press under way gave
/// meanwhile is taken back then.
///
/// `held_by`, where the chat's persona holds the vault (#5890): each run it answers is told so, as
/// the chat is, never that it was kept blocked.
pub(crate) fn keep_blocked_pressed(
    held: &crate::planes::Held,
    network: Option<&purlis_core::sandboxblock::record::Record>,
    session: u32,
    vault: &str,
    held_by: Option<&KeptButHeld>,
) -> KeptBack {
    held.vault_refusals().keep_blocked(session, vault);
    // Its asks are answered below, under the answers' lock.
    let kept = held.secret_asks().keep_vault_blocked_taking(session, vault);
    audit_kept_back(held, &kept.taken_back);
    let _one = held.run_answers(session);
    vault_kept_blocked(held, network, session, vault, held_by)
}

/// What [`vault_kept_blocked`] did to the asks open on the vault.
#[derive(Debug, Default)]
pub(crate) struct KeptBack {
    /// The asks that stayed up, and why.
    pub stayed: Vec<(u32, String)>,
    /// The chats a run it answered was told that the persona holds the vault (#5890): told
    /// already, so the press tells them nothing more.
    pub told: Vec<u32>,
}

/// What Keep blocked does to what the app holds for chat `session` and `vault`: its later tries
/// raise nothing, and a Run grant's ask already open on the vault is answered Don't allow, since
/// its Allow would open what the person just kept blocked. The asks hold it too, so a run the
/// gate let past before it raises no ask after it (#5424). Carried out with
/// [`crate::planes::Held::run_answers`] held, by its caller (#5617). Answers which of those asks
/// stayed up, and why: one a Keep blocked answered in its turn, through a vault's own ask, is not
/// one of them.
///
/// **An Always allow pressed before it stays** (#5617, accepted): its tool line is committed for
/// the persona, and the vault given to it, by the person's own earlier press, for every chat and
/// every teammate. Keep blocked is for one chat on a vault its persona may not use, and has never
/// taken back what a persona holds; removing the tool, or the vault in the Allowed list, does
/// (ADR 0112, amended 2026-11-08, "Keep blocked leaves what a persona holds"). One pressed while
/// Keep blocked is carried out waits for it, and finds its ask answered before it writes anything.
pub(crate) fn vault_kept_blocked(
    held: &crate::planes::Held,
    network: Option<&purlis_core::sandboxblock::record::Record>,
    session: u32,
    vault: &str,
    held_by: Option<&KeptButHeld>,
) -> KeptBack {
    held.vault_refusals().keep_blocked(session, vault);
    let mut back = KeptBack::default();
    let kept = held.secret_asks().keep_vault_blocked_taking(session, vault);
    audit_kept_back(held, &kept.taken_back);
    for id in kept.open {
        let answered = crate::secretasks::run_grant_answered(
            held,
            network,
            id,
            crate::secretasks::RunGrantPress::Refuse,
            held_by,
        );
        let answered = answered.map(|told| back.told.extend(told));
        // Keep blocked through a vault's own ask keeps the vault blocked in its turn, and that
        // answers the rest: an ask closed by then did not stay up.
        let still_open = || {
            held.secret_asks()
                .open_runs()
                .iter()
                .any(|one| one.id == id)
        };
        if let Err(why) = answered
            && still_open()
        {
            tracing::warn!("purlis: a run grant's ask stayed up after Keep blocked: {why}");
            back.stayed.push((id, why));
        }
    }
    back
}

/// **Keep blocked on a vault the chat's persona already holds** (#5801): what the person and
/// the chat are told instead of "kept blocked".
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeptButHeld {
    pub persona: String,
    pub by: purlis_core::askoutcome::HeldBy,
    /// What the person reads beside the press.
    pub said: String,
    /// The reason the person typed for the press, if any ([`purlis_core::askoutcome::reason`]):
    /// told with the line by whichever telling reaches the chat (review of #5934).
    pub reason: Option<String>,
}

/// **Tells chat `session` that Keep blocked on `vault` holds nothing**, since its persona already
/// holds it (#5801), and answers what the person reads beside the press: the one telling of Keep
/// blocked on the refused vault's Notice and on a run's own notice alike (#5890). Its next turn
/// hears that its persona may use the vault and not to use it unless the person says to, with
/// [`KeptButHeld::reason`], what the person typed for it, if anything.
pub(crate) fn told_kept_but_held(
    held: &crate::planes::Held,
    session: u32,
    vault: &str,
    kept: KeptButHeld,
    told_already: bool,
) -> VaultAnswered {
    // A chat a run of it was told this, now or at its turn's end, is not told it twice.
    if !told_already {
        held.chats()
            .tell_outcome(session, kept_but_held_outcome(vault, &kept));
    }
    VaultAnswered { said: kept.said }
}

/// The one line a chat is told of Keep blocked on `vault`, which its persona holds (#5801).
pub(crate) fn kept_but_held_outcome(
    vault: &str,
    kept: &KeptButHeld,
) -> purlis_core::askoutcome::Outcome {
    purlis_core::askoutcome::Outcome {
        about: purlis_core::askoutcome::About::Vault {
            vault: vault.to_owned(),
        },
        answer: purlis_core::askoutcome::Answer::KeptBlockedButHeld {
            persona: kept.persona.clone(),
            by: kept.by,
            reason: kept.reason.clone(),
        },
    }
}

/// **Where Keep blocked holds nothing, since `persona` may already use `vault`** (#5801; ADR
/// 0112, amended 2026-11-08): its tag, or a row of the Allowed list an Always allow or a refused
/// vault's Allow made, by the broker's own rule ([`brokered::authorise`]). Says how, and how to
/// take it back. None where the persona may not use it, or the chat runs as none.
pub(crate) fn kept_blocked_but_held(
    root: &std::path::Path,
    persona: Option<&str>,
    vault: &str,
) -> Option<KeptButHeld> {
    use purlis_core::askoutcome::HeldBy;
    let persona = persona.filter(|p| !p.is_empty())?;
    brokered::authorise(&Ctx::new(root, Env::from_process()), Some(persona), vault).ok()?;
    // The broker's own rule: the tag first, then a row of the Allowed list where policy leaves
    // vault grants (#5876's review). Removing a row leaves a tag that still holds.
    let ctx = Ctx::new(root, Env::from_process());
    let tagged = purlis_core::secrets::registry::load_registry(&ctx)
        .ok()
        .and_then(|doc| purlis_core::secrets::registry::vault_in(&doc, vault).ok())
        .and_then(|registered| registered.persona)
        .is_some_and(|tagged_for| tagged_for == persona);
    let allowed = !sandbox::policy::Locks::of(root).forbids_vault_grants()
        && sandbox::local::granted_vaults(root)
            .iter()
            .any(|one| one.vault == vault && one.persona == persona);
    let by = if tagged {
        HeldBy::Tag
    } else {
        HeldBy::AllowedList
    };
    let mut said = format!(
        "You kept vault {vault} blocked for this chat, but {persona} can already use it, so Keep blocked does not stop this \
         chat's runs on it."
    );
    said.push_str(&match by {
        HeldBy::AllowedList => {
            " Remove it in Settings › Sandbox › Allowed to stop them.".to_owned()
        }
        HeldBy::Tag => {
            format!(" The project's vaults tag it for {persona}: change that tag to stop them.")
        }
    });
    if tagged && allowed {
        said.push_str(" It is in Settings › Sandbox › Allowed too.");
    }
    if brokered::declares_a_tool_with(root, persona, vault) {
        said.push_str(&format!(
            " {persona}'s tools name it too: remove that tool line to stop its runs."
        ));
    }
    Some(KeptButHeld {
        persona: persona.to_owned(),
        by,
        said,
        reason: None,
    })
}

/// **Puts a refused vault's Notice away, and tells the chat nothing** (#1987): Dispatch to
/// {persona}… on it, where the work goes to the vault's own persona. Nothing was kept blocked
/// by the person, so the chat is not told it was; it hears how the work went from that persona
/// or the person. Its next try at the vault raises the Notice again.
#[tauri::command]
#[specta::specta]
pub fn put_away_refused_vault(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    vault: String,
) -> Result<(), String> {
    planes
        .held(&plane)?
        .vault_refusals()
        .put_away(session, &vault);
    Ok(())
}

#[cfg(test)]
mod tests;
