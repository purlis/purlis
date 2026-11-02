// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A browser login the person confirms from one notice** (#4595, the person's rulings of
//! 2026-11-01, which amend PC-1 and PC-5 of #1975 for this one case): the window's half of
//! [`purlis_core::loginask`].
//!
//! A chat's `browser_login` on a login its persona doesn't declare raises one notice, and its call
//! waits for the answer ([`before_the_login`]). The notice is the sandbox's: Allow once, Allow for
//! this chat, Always allow, Keep blocked.
//!
//! - **What's filled is what the person pressed on.** A press names the proposal and carries its
//!   digest ([`Proposal::digest`]), and nothing a chat sends changes a proposal once it's shown:
//!   one waits for each chat, and another call is refused.
//! - **A typed site is confirmed as it will be filled** ([`answer`]): read as a chat's suggestion
//!   is. Where it isn't what was typed, or a warning now applies, the notice is drawn again and
//!   takes another press.
//! - **Once and this chat write nothing** ([`Grants`]): they're held here, for that chat and its
//!   persona, keep no sealed login, and go with the chat. Each fills in a new browser of its own,
//!   so nothing the chat did in another browser is there when the password goes in.
//! - **Always writes the one line and allows it here**, only where the persona's list was already
//!   seen ([`purlis_core::sandbox::persona::list_seen_in`]), so its Allow carries nothing nobody
//!   was shown. `sandbox.proposed-changes = "off"` holds it, and the line is shown first.
//! - **A chat on a runner still needs its opt-in** (ADR 0089): its call can't wait across the
//!   link, so it's told once the person answers, and the opt-in is asked on its next call.
//!
//! In memory only: a relaunch starts with none.

use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError, mpsc};

use purlis_core::hookwire::{Answer, BrowseAsk, Browsing};
use purlis_core::loginask::{self, Proposal, Suggested};
use purlis_core::sandbox::tools::Tool;
use purlis_core::sandbox::{self, policy::Locks};
use purlis_core::secrets::brokered;
use purlis_core::secrets::{Ctx, Env};

use crate::planes::{Held, PlaneId, Planes};

/// **A login's notice, as it waits**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginWait {
    /// Its number in the project: what its answer names.
    pub id: u32,
    pub session: u32,
    pub proposal: Proposal,
    pub since: u64,
    /// The persona needs its vault allowed too: what Always then also does.
    pub vault_too: bool,
    /// Why Always isn't offered, where it isn't.
    pub no_always: Option<String>,
    /// The pool whose runner the chat is on: it isn't waiting, and is told.
    pub pool: Option<String>,
}

/// What a chat's waiting call hears.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    /// Allowed: its login is held for it.
    Allowed,
    /// Kept blocked, with the person's reason.
    Kept(Option<String>),
    /// The chat closed, or it's no longer asked.
    Gone,
}

/// A login the person allowed for a chat: nothing of it is on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Grant {
    session: u32,
    persona: String,
    tool: Tool,
    /// Allow once: it goes with the read of its values.
    once: bool,
}

#[derive(Debug, Default)]
struct Inner {
    next: u32,
    waits: Vec<(LoginWait, Option<mpsc::Sender<Heard>>)>,
    grants: Vec<Grant>,
    /// What the person kept blocked, by chat, login and origin: not asked of them again.
    kept: Vec<(u32, String, String)>,
    /// Every login allowed from a notice since purlis started, used up or closed too: where a key
    /// went counts for a warning whatever became of the grant.
    filled: Vec<Tool>,
}

/// How many Keep blocked one chat is given before it's asked about no more logins, so a chat
/// can't change a name or a subdomain until someone presses yes.
const KEPT_AT_MOST: usize = 2;

/// **The logins a project's chats wait on the person for, and what they allowed.**
#[derive(Debug, Default)]
pub struct Grants {
    inner: Mutex<Inner>,
    /// Held by a press from its read of what waits to its answer: the Inbox and a pane draw one
    /// notice, and two presses never both pass.
    answering: Mutex<()>,
}

impl Grants {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// **Holds `wait`**, numbering it, or refuses it where its chat has one waiting: a proposal
    /// is never replaced under the person's eyes. The receiver hears the answer.
    pub fn raise(&self, mut wait: LoginWait) -> Result<(u32, mpsc::Receiver<Heard>), String> {
        let mut inner = self.lock();
        if inner
            .waits
            .iter()
            .any(|(one, _)| one.session == wait.session)
        {
            return Err(
                "the person is already being asked about a browser login for this chat: wait for \
                 their answer before suggesting another"
                    .to_owned(),
            );
        }
        inner.next += 1;
        wait.id = inner.next;
        let id = wait.id;
        let (tell, heard) = mpsc::channel();
        inner.waits.push((wait, Some(tell)));
        Ok((id, heard))
    }

    /// Every login that waits, oldest first.
    pub fn waiting(&self) -> Vec<LoginWait> {
        self.lock()
            .waits
            .iter()
            .map(|(one, _)| one.clone())
            .collect()
    }

    /// The login of chat `session` numbered `id`, while it waits.
    fn wait(&self, session: u32, id: u32) -> Option<LoginWait> {
        self.lock()
            .waits
            .iter()
            .find(|(one, _)| one.session == session && one.id == id)
            .map(|(one, _)| one.clone())
    }

    /// Replaces the proposal of a login that waits, as the person typed it or as the project
    /// reads now. Only the app's own answer calls this: no chat's call reaches it.
    fn redraw(&self, id: u32, proposal: Proposal) {
        if let Some((one, _)) = self.lock().waits.iter_mut().find(|(one, _)| one.id == id) {
            one.proposal = proposal;
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

    /// Chat `session`'s login `login` kept blocked on `origin`.
    fn keep(&self, session: u32, login: &str, origin: &str) {
        self.lock()
            .kept
            .push((session, login.to_owned(), origin.to_owned()));
    }

    /// Whether the person kept this blocked for this chat, or kept enough blocked that it's
    /// asked about no more.
    fn kept(&self, session: u32, login: &str, origin: &str) -> bool {
        let inner = self.lock();
        let mine = || inner.kept.iter().filter(|(chat, _, _)| *chat == session);
        mine().count() >= KEPT_AT_MOST || mine().any(|(_, name, on)| name == login && on == origin)
    }

    /// Holds `tool` for chat `session`, as `persona`.
    fn grant(&self, session: u32, persona: &str, tool: Tool, once: bool) {
        let mut inner = self.lock();
        inner
            .grants
            .retain(|one| !(one.session == session && one.tool.name == tool.name));
        inner.filled.push(tool.clone());
        inner.grants.push(Grant {
            session,
            persona: persona.to_owned(),
            tool,
            once,
        });
    }

    /// The login `name` the person allowed for chat `session` running as `persona`.
    pub fn granted(&self, session: u32, persona: Option<&str>, name: &str) -> Option<Tool> {
        self.lock()
            .grants
            .iter()
            .find(|one| {
                one.session == session
                    && persona == Some(one.persona.as_str())
                    && one.tool.name == name
            })
            .map(|one| one.tool.clone())
    }

    /// **The login `name` to read now**: as [`Self::granted`], and Allow once goes with it, in
    /// one step, so two calls never both fill it and a read that fails leaves nothing behind.
    pub fn take(&self, session: u32, persona: Option<&str>, name: &str) -> Option<Tool> {
        let mut inner = self.lock();
        let at = inner.grants.iter().position(|one| {
            one.session == session && persona == Some(one.persona.as_str()) && one.tool.name == name
        })?;
        if inner.grants[at].once {
            Some(inner.grants.remove(at).tool)
        } else {
            Some(inner.grants[at].tool.clone())
        }
    }

    /// Every login allowed from a notice, for what a warning counts as declared.
    fn held(&self) -> Vec<Tool> {
        self.lock().filled.clone()
    }

    /// Chat `session` closed: what it waited on and was allowed go with it, so nothing reaches a
    /// later chat dealt the same number, and its call stops waiting.
    pub fn forget(&self, session: u32) {
        let mut inner = self.lock();
        inner.grants.retain(|one| one.session != session);
        inner.kept.retain(|(chat, _, _)| *chat != session);
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

/// What a chat is told while its login waits on the person, on a runner.
pub fn asked_on_a_runner_said(login: &str, pool: &str) -> String {
    format!(
        "this browser runs on pool {pool}'s runner, so purlis asked the person about login \
         {login} and filled nothing: you are told once they answer; call browser_login again \
         then, and go on with what needs no login meanwhile"
    )
}

/// What a chat is told where the person kept a login blocked.
pub fn kept_said(login: &str, reason: Option<&str>) -> String {
    let why = reason.map_or_else(String::new, |why| {
        format!(": {}", purlis_core::shown::short(why))
    });
    format!(
        "the person kept {login} blocked, so nothing was filled{why}. Don't suggest it again; \
         tell them what needs the login, or go on without it"
    )
}

/// What a chat is told where nobody answered in time.
pub const NOT_ANSWERED: &str = "nobody answered about this login in time, so nothing was filled \
                                and the notice went. Tell the person what needs the login, or \
                                call browser_login again when they're back";

/// **What a chat whose login is undeclared waits on, before its browser hears anything**: `None`
/// where the normal path answers it: a login its persona declares or was allowed, a chat with no
/// persona, one its host composed, a call that suggests nothing, or a browser nobody allowed yet,
/// whose Ask comes first. Otherwise it answers with why not, or having waited. Allowed answers
/// `None`, and the normal path fills what's now held.
pub fn before_the_login(
    held: &Held,
    browse: &BrowseAsk,
    pool: Option<&str>,
    now: u64,
) -> Option<Answer> {
    let Browsing::LogIn { login, suggested } = &browse.asked else {
        return None;
    };
    let suggested = suggested.as_ref()?;
    let session = browse.chat;
    let always = crate::chatbrowsers::as_persona_of(held, session).always;
    if !held.browsers().may_browse(session, always.is_some()) {
        return None;
    }
    if held.chats().sandboxed_by_its_host(session) {
        return None;
    }
    let (asker, unconfined) = held.chats().login_asker(held.root(), session)?;
    let persona = asker
        .persona
        .clone()
        .filter(|persona| !persona.is_empty())?;
    if pool.is_some() && unconfined {
        return None;
    }
    let no = |why: String| Some(Answer::No { why });
    let grants = held.login_grants();
    if grants.granted(session, Some(&persona), login).is_some() {
        return None;
    }
    let (declared, _) = sandbox::tools::declared_and_allowed_here(held.root(), &persona);
    if declared.iter().any(|tool| tool.name == *login) {
        return None;
    }
    if let Some(pool) = pool {
        // Only a runner shown to be yours is ever asked about, as for a declared login.
        let config = held.chats().pools_in()?;
        let reached = crate::hooks::reached_over_the_link();
        if let Err(why) =
            purlis_core::poollogins::Bound::over_the_link(&config, pool, reached.as_deref())
        {
            return no(format!("no login {login} was filled on pool {pool}: {why}"));
        }
    }
    let wait = match judged(held.root(), &persona, login, suggested, &grants.held()) {
        Ok(judged) if grants.kept(session, login, &judged.proposal.origin) => {
            return no(kept_said(login, None));
        }
        Ok(judged) => LoginWait {
            id: 0,
            session,
            since: now,
            pool: pool.map(str::to_owned),
            proposal: judged.proposal,
            vault_too: judged.vault_too,
            no_always: judged.no_always,
        },
        Err(why) => return no(why),
    };
    let (id, heard) = match grants.raise(wait) {
        Ok(raised) => raised,
        Err(why) => return no(why),
    };
    tracing::info!("purlis: chat {session}'s browser login asks the person about its site");
    held.chats().asks_may_have_moved();
    if let Some(pool) = pool {
        return no(asked_on_a_runner_said(login, pool));
    }
    match heard.recv_timeout(loginask::THE_PERSON_IS_WAITED_FOR) {
        Ok(Heard::Allowed) => None,
        Ok(Heard::Kept(reason)) => no(kept_said(login, reason.as_deref())),
        Ok(Heard::Gone) | Err(mpsc::RecvTimeoutError::Disconnected) => no(NOT_ANSWERED.to_owned()),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // A press in that very moment has ended it, and its answer is waiting here.
            if !grants.end(id, Heard::Gone)
                && let Ok(Heard::Allowed) = heard.try_recv()
            {
                return None;
            }
            held.chats().asks_may_have_moved();
            no(NOT_ANSWERED.to_owned())
        }
    }
}

/// A suggestion as the project reads it now.
#[derive(Debug, PartialEq, Eq)]
struct Judged {
    proposal: Proposal,
    vault_too: bool,
    no_always: Option<String>,
}

/// **Whether a suggestion is one the person is asked about**, and what they're offered: no tool
/// where policy forbids it, a vault the project has, and one the persona may use or that policy
/// lets the person allow it.
fn judged(
    root: &Path,
    persona: &str,
    login: &str,
    suggested: &Suggested,
    held: &[Tool],
) -> Result<Judged, String> {
    let locks = Locks::of(root);
    if let Some(why) = locks.refuses_tool(persona, login) {
        return Err(why);
    }
    let proposal = loginask::proposal(root, persona, login, suggested, held)?;
    finish(root, &locks, proposal)
}

/// What the person is offered on `proposal`, under `locks`.
fn finish(root: &Path, locks: &Locks, proposal: Proposal) -> Result<Judged, String> {
    let ctx = Ctx::new(root, Env::from_process());
    let doc = purlis_core::secrets::registry::load_registry(&ctx)
        .map_err(|why| why.message.to_string())?;
    purlis_core::secrets::registry::vault_in(&doc, &proposal.vault)
        .map_err(|why| why.message.to_string())?;
    let vault_too = brokered::not_tagged(&ctx, Some(&proposal.persona), &proposal.vault).is_some();
    let mut proposal = proposal;
    if vault_too {
        proposal.warnings.push(loginask::Warning::NotItsVault {
            vault: proposal.vault.clone(),
            persona: proposal.persona.clone(),
        });
    }
    let refused = locks.vault_grants_refused();
    if let (true, Some(why)) = (vault_too, refused) {
        return Err(why);
    }
    let no_always = locks
        .proposed_changes_refused()
        .or_else(|| locks.scope_removed(sandbox::grant::Level::You))
        .or_else(|| {
            let file =
                std::fs::read_to_string(purlis_core::names::manifest(root)).unwrap_or_default();
            if !loginask::has_a_sandbox(&file) {
                return Some(
                    "Always allow isn't offered in a project whose settings have no [sandbox]"
                        .to_owned(),
                );
            }
            (!sandbox::persona::list_seen_in(root, &file, &proposal.persona, locks)).then(|| {
                format!(
                    "Always allow isn't offered while {}'s hosts and tools wait for your Allow",
                    proposal.persona
                )
            })
        });
    Ok(Judged {
        proposal,
        vault_too,
        no_always,
    })
}

/// A press on a login's notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum Press {
    Once,
    Chat,
    Always,
    Keep,
}

/// What a press answered.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Pressed {
    /// The answer stands: the sentence it leaves.
    Done { said: String },
    /// The notice was drawn again and takes another press: what changed.
    DrawnAgain { said: String },
}

/// Writes the audit of a press, and answers whether it was written.
pub type Audit<'a> = &'a dyn Fn(Option<u32>, &sandbox::grant::Audited<'_>) -> Result<(), String>;

/// What the audit calls a login allowed from its notice.
const AUDITED: &str = "browser-login";

/// A press, as the window sends it.
#[derive(Debug, Clone, Copy)]
pub struct Asked<'a> {
    pub session: u32,
    pub id: u32,
    pub press: Press,
    /// The digest of the proposal the notice drew.
    pub shown: &'a str,
    /// The domain the person typed, where they changed it.
    pub typed: Option<&'a str>,
    pub reason: Option<&'a str>,
}

/// **The person's press** on the login a notice drew. Fails closed: a press on anything but what's
/// waiting now changes nothing.
pub fn answer(
    root: &Path,
    grants: &Grants,
    asked: &Asked<'_>,
    audit: Audit<'_>,
) -> Result<Pressed, String> {
    const NO_MORE: &str = "That ask has gone, and nothing was allowed.";
    let _one_press = grants
        .answering
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let Asked {
        session,
        id,
        press,
        shown,
        typed,
        reason,
    } = *asked;
    let wait = grants.wait(session, id).ok_or(NO_MORE)?;
    if wait.proposal.digest() != shown {
        return Err("That ask changed, so nothing was allowed. Read it again.".to_owned());
    }
    if press == Press::Keep {
        grants.keep(session, &wait.proposal.login, &wait.proposal.origin);
        grants.end(id, Heard::Kept(reason.map(str::to_owned)));
        return Ok(Pressed::Done {
            said: "Kept blocked. Nothing was filled.".to_owned(),
        });
    }
    // A field left empty names no site: it never falls back to the one the chat suggested.
    let moved = match typed.map(str::trim) {
        Some("") => return Err("Type the site to fill this on, or Keep blocked.".to_owned()),
        Some(typed) => Some((typed, wait.proposal.at(root, typed, &grants.held())?)),
        None => None,
    };
    let (proposal, spelled) = match moved {
        Some((typed, moved)) if moved.page != wait.proposal.page => {
            let bare = moved.origin.strip_prefix("https://");
            let spelled = bare == Some(typed) || moved.page.strip_prefix("https://") == Some(typed);
            (moved, spelled)
        }
        _ => (wait.proposal.clone(), true),
    };
    // Judged again as the project reads now: a line or a mark added since is a new notice.
    let locks = Locks::of(root);
    if let Some(why) = locks.refuses_tool(&proposal.persona, &proposal.login) {
        return Err(why);
    }
    let again = Suggested {
        site: proposal.page.clone(),
        vault: proposal.vault.clone(),
        user_key: proposal.user_key.clone(),
        key: proposal.key.clone(),
    };
    let fresh = loginask::proposal(
        root,
        &proposal.persona,
        &proposal.login,
        &again,
        &grants.held(),
    )?;
    let judged = finish(root, &locks, fresh)?;
    let changed = judged.proposal.digest() != wait.proposal.digest();
    // Always writes a line, and the line is shown before it's written.
    if changed && (!spelled || !judged.proposal.warnings.is_empty() || press == Press::Always) {
        let said = format!(
            "purlis would fill this on {}. Check it, then choose again.",
            judged.proposal.origin
        );
        grants.redraw(id, judged.proposal);
        return Ok(Pressed::DrawnAgain { said });
    }
    if press == Press::Always
        && let Some(why) = judged.no_always.clone()
    {
        return Err(why);
    }
    let proposal = judged.proposal;
    let tool = proposal.tool()?;
    let (level, once) = match press {
        Press::Once => (sandbox::grant::Level::Once, true),
        Press::Chat => (sandbox::grant::Level::Chat, false),
        Press::Always | Press::Keep => (sandbox::grant::Level::You, false),
    };
    // Policy's list of scopes holds here as well as on the notice.
    if let Some(why) = locks.scope_removed(level) {
        return Err(why);
    }
    let target = tool.whole();
    audit(
        Some(session),
        &sandbox::grant::Audited {
            granted: true,
            what: AUDITED,
            target: &target,
            level,
        },
    )?;
    let said = if press == Press::Always {
        written(root, &locks, &proposal, judged.vault_too, session, audit)?;
        format!(
            "Allowed. {} is in the project's settings, and allowed on this machine.",
            proposal.login
        )
    } else {
        format!(
            "Allowed. Nothing was written. It fills on {}.",
            proposal.origin
        )
    };
    grants.grant(session, &proposal.persona, tool, once);
    grants.end(id, Heard::Allowed);
    Ok(Pressed::Done { said })
}

/// **Always**: the file with the one line, written only over what was read, then the persona's
/// list allowed as what was written says it, and its vault where it needs it.
fn written(
    root: &Path,
    locks: &Locks,
    proposal: &Proposal,
    vault_too: bool,
    session: u32,
    audit: Audit<'_>,
) -> Result<(), String> {
    let file = purlis_core::names::manifest(root);
    let rel = file
        .strip_prefix(root)
        .map_err(|_| NOT_WRITTEN.to_owned())?;
    let before = match std::fs::read(&file) {
        Ok(bytes) => Some(bytes),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return Err(NOT_WRITTEN.to_owned()),
    };
    let text = match &before {
        Some(bytes) => std::str::from_utf8(bytes).map_err(|_| NOT_WRITTEN.to_owned())?,
        None => "",
    };
    // Judged again from the very bytes the write is pinned to.
    if !sandbox::persona::list_seen_in(root, text, &proposal.persona, locks) {
        return Err(NOT_WRITTEN.to_owned());
    }
    let after = loginask::with_line(text, proposal)?;
    let digest = sandbox::persona::digest_in(root, &after, &proposal.persona, locks)
        .ok_or_else(|| NOT_WRITTEN.to_owned())?;
    let base = purlis_core::proposed::base_of(before.as_deref());
    purlis_core::proposed::write_pinned(root, rel, base.as_deref(), &after)
        .map_err(|_| NOT_WRITTEN.to_owned())?;
    sandbox::local::allow_persona_hosts(root, &proposal.persona, &digest)
        .map_err(|why| format!("purlis wrote the line, and could not allow it here: {why}"))?;
    if vault_too {
        let grant = sandbox::local::VaultGrant {
            vault: proposal.vault.clone(),
            persona: proposal.persona.clone(),
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
        sandbox::local::grant_vault(root, &grant.vault, &grant.persona).map_err(|why| {
            format!("purlis wrote the line, and could not allow the vault: {why}")
        })?;
    }
    Ok(())
}

const NOT_WRITTEN: &str = "The project's settings changed or can't be written, so nothing was \
                           written or allowed. Read the ask again.";

/// **The person's press on a login's notice** (#4595). The window's alone (`ui::WINDOW_ONLY`).
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn answer_browser_login(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    id: u32,
    press: Press,
    shown: String,
    typed: Option<String>,
    reason: Option<String>,
) -> Result<Pressed, String> {
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    crate::off_the_window("answering a browser login", move || {
        pressed(
            &held,
            network.as_ref(),
            &Asked {
                session,
                id,
                press,
                shown: &shown,
                typed: typed.as_deref(),
                reason: reason.as_deref(),
            },
        )
    })
    .await
}

/// [`answer_browser_login`]'s work, with what a chat on a runner is told of it.
fn pressed(
    held: &Held,
    network: Option<&purlis_core::sandboxblock::record::Record>,
    asked: &Asked<'_>,
) -> Result<Pressed, String> {
    use purlis_core::askoutcome::{About, Answer as Said, Outcome, Scope};
    let wait = held.login_grants().wait(asked.session, asked.id);
    let answer = answer(
        held.root(),
        held.login_grants(),
        asked,
        &held.audit_then_record(network),
    );
    held.chats().asks_may_have_moved();
    if let (Ok(Pressed::Done { .. }), Some(wait)) = (&answer, wait)
        && wait.pool.is_some()
    {
        let about = About::BrowserLogin {
            login: wait.proposal.login.clone(),
        };
        held.chats().tell_outcome(
            asked.session,
            if asked.press == Press::Keep {
                crate::askoutcome::kept(about, asked.reason)
            } else {
                Outcome {
                    about,
                    answer: Said::Allowed {
                        scope: match asked.press {
                            Press::Once => Scope::Once,
                            Press::Always => Scope::You,
                            Press::Chat | Press::Keep => Scope::Chat,
                        },
                    },
                }
            },
        );
    }
    answer
}

#[cfg(test)]
#[path = "loginasks/tests.rs"]
mod tests;
