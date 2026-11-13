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
//!   persona, keep no sealed login, and go with the chat. Each fills in the browser the chat names
//!   or uses (#5121, the person's ruling SF-7 on #5096, which ends the new browser of its own):
//!   what the chat did there before doesn't matter, as the value is kept out of all it reads.
//! - **Always writes the one line and allows it here**, only where the persona's list was already
//!   seen ([`purlis_core::sandbox::persona::list_seen_in`]), so its Allow carries nothing nobody
//!   was shown. `sandbox.proposed-changes = "off"` holds it, and the line is shown first.
//! - **A chat on a runner still needs its opt-in** (ADR 0089): its call can't wait across the
//!   link, so it's told once the person answers, and the opt-in is asked on its next call.
//!
//! In memory only: a relaunch starts with none.

use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError, mpsc};
use std::time::Duration;

use purlis_core::hookwire::{Answer, BrowseAsk, Browsing};
use purlis_core::loginask::{self, FindsForm, Proposal, Suggested};
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
    /// The chat's id (ADR 0066) as it raised this: what its grant is bound to, beside its number.
    pub chat: Option<String>,
    pub proposal: Proposal,
    pub since: u64,
    /// The persona needs its vault allowed too: what Always then also does.
    pub vault_too: bool,
    /// Why Always isn't offered, where it isn't.
    pub no_always: Option<String>,
    /// The pool whose runner the chat is on: it isn't waiting, and is told.
    pub pool: Option<String>,
    /// **The persona whose vaults the login reads**: the one the chat was started as, narrowed or
    /// not, as the read judges a link (#5831); `proposal.persona` is whose tools Always writes in.
    pub reads_as: String,
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

/// **What the chat reads where its notice is refused for a key out of reach** (#5831): the read's
/// own words, unchanged.
fn as_the_chat_reads_it(why: &str) -> String {
    why.to_owned()
}

/// **What the person reads where a press is refused for a key out of reach** (#5831), on the
/// notice, which stays up as it does for every press refused: nothing was allowed, and the read's
/// own words, which name the key and the chat's vault alone, told to the person as "you"
/// (docs/ui-copy.md) where the chat's words say "the person".
pub(crate) fn refused_at_the_press_said(why: &str) -> String {
    let why = why.replace("The person can", "You can");
    let stop = if why.ends_with('.') { "" } else { "." };
    format!("Nothing was allowed: {why}{stop}")
}

/// A login the person allowed for a chat: nothing of it is on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Grant {
    session: u32,
    /// The id of the chat it was allowed for (ADR 0066): a number dealt again never reads it.
    chat: Option<String>,
    persona: String,
    tool: Tool,
    /// Allow once: it goes with the read of its values.
    once: bool,
}

impl Grant {
    /// Whether it's login `name`, allowed for chat `session` whose id is `chat`, as `persona`.
    fn is_for(&self, session: u32, chat: Option<&str>, persona: Option<&str>, name: &str) -> bool {
        self.session == session
            && self.chat.as_deref() == chat
            && persona == Some(self.persona.as_str())
            && self.tool.name == name
    }
}

#[derive(Debug, Default)]
struct Inner {
    next: u32,
    waits: Vec<(LoginWait, Option<mpsc::Sender<Heard>>)>,
    grants: Vec<Grant>,
    /// **Each Allow once a read took, until its login answers** (#5251): spent then, or the chat's
    /// again where the login was refused before it ran.
    taken: Vec<Grant>,
    /// **What the person kept blocked, by persona, login and origin**: not asked of them again
    /// until purlis restarts, whichever of the persona's chats asks (the person's ruling of
    /// 2026-11-07), so a new chat starts no fresh tries.
    kept: Vec<(String, String, String)>,
    /// Every login allowed from a notice since purlis started, used up or closed too: where a key
    /// went counts for a warning whatever became of the grant.
    filled: Vec<Tool>,
    /// The same, by the persona each was allowed for (#5121).
    from_notices: Vec<(String, Tool)>,
}

/// What a chat is told while the person is being asked about another of its logins.
const ALREADY_ASKED: &str = "the person is already being asked about a browser login for this \
                             chat: wait for their answer before suggesting another";

/// How many Keep blocked one persona is given before its chats are asked about no more logins
/// until purlis restarts, so a chat can't change a name or a subdomain, or a new chat start
/// again, until someone presses yes.
const KEPT_AT_MOST: usize = 2;

/// **The logins a project's chats wait on the person for, and what they allowed.**
#[derive(Debug, Default)]
pub struct Grants {
    inner: Mutex<Inner>,
    /// Held by a press from its read of what waits to its answer: the Inbox and a pane draw one
    /// notice, and two presses never both pass.
    answering: Mutex<()>,
    /// The chats a page is being followed to its form for now (#4995): one at a time each, so
    /// calls side by side never open a browser apiece.
    following: Mutex<Vec<u32>>,
}

/// Chat `session`'s one page being followed to its form, let go as it's dropped.
pub struct Following<'a> {
    grants: &'a Grants,
    session: u32,
}

impl Drop for Following<'_> {
    fn drop(&mut self) {
        self.grants
            .following
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|one| *one != self.session);
    }
}

impl Grants {
    /// **Chat `session` follows a page to its form now**, or `None` where it already does.
    pub fn follow(&self, session: u32) -> Option<Following<'_>> {
        let mut following = self
            .following
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if following.contains(&session) {
            return None;
        }
        following.push(session);
        Some(Following {
            grants: self,
            session,
        })
    }

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
            return Err(ALREADY_ASKED.to_owned());
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

    /// Draws a login that waits anew, whole, as the person typed it or as the project reads now.
    /// Only the app's own answer calls this: no chat's call reaches it.
    fn redraw(&self, id: u32, judged: Judged) {
        if let Some((one, _)) = self.lock().waits.iter_mut().find(|(one, _)| one.id == id) {
            one.proposal = judged.proposal;
            one.vault_too = judged.vault_too;
            one.no_always = judged.no_always;
        }
    }

    /// **Allows the wait numbered `id`, in one step**: the login held for its chat and its call
    /// told, or nothing where it waits no more, so nothing lands after its chat has gone.
    fn allow(&self, id: u32, persona: &str, tool: Tool, once: bool) -> bool {
        let mut inner = self.lock();
        let Some(at) = inner.waits.iter().position(|(one, _)| one.id == id) else {
            return false;
        };
        let (wait, tell) = inner.waits.remove(at);
        inner
            .grants
            .retain(|one| !(one.session == wait.session && one.tool.name == tool.name));
        inner.filled.push(tool.clone());
        inner.from_notices.push((persona.to_owned(), tool.clone()));
        inner.grants.push(Grant {
            session: wait.session,
            chat: wait.chat.clone(),
            persona: persona.to_owned(),
            tool,
            once,
        });
        if let Some(tell) = tell {
            let _ = tell.send(Heard::Allowed);
        }
        true
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

    /// `persona`'s login `login` kept blocked on `origin`.
    #[cfg(test)]
    fn keep(&self, persona: &str, login: &str, origin: &str) {
        self.lock()
            .kept
            .push((persona.to_owned(), login.to_owned(), origin.to_owned()));
    }

    /// **The wait numbered `id` kept blocked, in one step**: counted for its persona and its call
    /// told, or nothing where it waits no more, so a Keep that lost to the time running out or
    /// the call ending is never counted.
    fn keep_wait(&self, id: u32, reason: Option<String>) -> bool {
        let mut inner = self.lock();
        let Some((wait, _)) = inner.waits.iter().find(|(one, _)| one.id == id) else {
            return false;
        };
        let proposal = &wait.proposal;
        let kept = (
            proposal.persona.clone(),
            proposal.login.clone(),
            proposal.origin.clone(),
        );
        inner.kept.push(kept);
        Self::ended(&mut inner, id, Heard::Kept(reason))
    }

    /// Whether the person kept this blocked for `persona`, or kept enough of its logins blocked
    /// that its chats are asked about no more, until purlis restarts.
    fn kept(&self, persona: &str, login: &str, origin: &str) -> bool {
        let inner = self.lock();
        let its = || inner.kept.iter().filter(|(whose, _, _)| whose == persona);
        its().count() >= KEPT_AT_MOST || its().any(|(_, name, on)| name == login && on == origin)
    }

    /// The login `name` the person allowed for chat `session`, whose id is `chat`, running as
    /// `persona`.
    pub fn granted(
        &self,
        session: u32,
        chat: Option<&str>,
        persona: Option<&str>,
        name: &str,
    ) -> Option<Tool> {
        self.lock()
            .grants
            .iter()
            .find(|one| one.is_for(session, chat, persona, name))
            .map(|one| one.tool.clone())
    }

    /// **The login `name` to read now**: as [`Self::granted`], and Allow once goes with it, in
    /// one step, so two calls never both fill it and a read that fails leaves nothing behind.
    pub fn take(
        &self,
        session: u32,
        chat: Option<&str>,
        persona: Option<&str>,
        name: &str,
    ) -> Option<Tool> {
        let mut inner = self.lock();
        let at = inner
            .grants
            .iter()
            .position(|one| one.is_for(session, chat, persona, name))?;
        if inner.grants[at].once {
            let taken = inner.grants.remove(at);
            let tool = taken.tool.clone();
            inner
                .taken
                .retain(|one| !one.is_for(session, chat, persona, name));
            inner.taken.push(taken);
            Some(tool)
        } else {
            Some(inner.grants[at].tool.clone())
        }
    }

    /// **Chat `session`'s login `name` answered** (#5251): an Allow once [`Self::take`] took for it
    /// is the chat's again where the login was refused before it ran (`ran` false), and is spent
    /// otherwise. Given back only where nothing of that name was allowed since.
    pub fn answered(
        &self,
        session: u32,
        chat: Option<&str>,
        persona: Option<&str>,
        name: &str,
        ran: bool,
    ) {
        let mut inner = self.lock();
        let at = inner
            .taken
            .iter()
            .position(|one| one.is_for(session, chat, persona, name));
        let Some(at) = at else {
            return;
        };
        let taken = inner.taken.remove(at);
        let allowed_since = inner
            .grants
            .iter()
            .any(|one| one.is_for(session, chat, persona, name));
        if !ran && !allowed_since {
            inner.grants.push(taken);
        }
    }

    /// Every login allowed from a notice, for what a warning counts as declared.
    fn held(&self) -> Vec<Tool> {
        self.lock().filled.clone()
    }

    /// Whether `persona`'s login `name` on page `address` was allowed from a notice since purlis
    /// started, Always too (#5121). Another's login of that name and page is its own.
    pub fn allowed_from_a_notice(&self, persona: Option<&str>, name: &str, address: &str) -> bool {
        self.lock().from_notices.iter().any(|(whose, tool)| {
            persona == Some(whose.as_str())
                && tool.name == name
                && tool
                    .login
                    .as_ref()
                    .is_some_and(|login| login.url == address)
        })
    }

    /// Chat `session` closed: what it waited on and was allowed go with it, so nothing reaches a
    /// later chat dealt the same number, and its call stops waiting.
    pub fn forget(&self, session: u32) {
        let mut inner = self.lock();
        inner.grants.retain(|one| one.session != session);
        inner.taken.retain(|one| one.session != session);
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

/// What a chat is told where the person kept a declared login's site as it is (#4996).
pub fn kept_where_it_is_said(login: &str, page: &str, reason: Option<&str>) -> String {
    let why = reason.map_or_else(String::new, |why| {
        format!(": {}", purlis_core::shown::short(why))
    });
    format!(
        "the person kept login {login} on {page}, so nothing changed and nothing was filled{why}. \
         Call browser_login without site to log in there, or tell them what needs another site"
    )
}

/// **Login `login` as `persona`'s list in the project's file declares it now**, allowed here or
/// not: the site a call naming another would replace (#4996).
fn declared_now(root: &Path, persona: &str, login: &str) -> Option<Tool> {
    let (declared, _) = sandbox::tools::declared_and_allowed_here(root, persona);
    declared.into_iter().find(|tool| tool.name == login)
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
///
/// `gone` says whether the call's asker hung up: its wait then ends, and its notice goes with it.
pub fn before_the_login(
    held: &Held,
    browse: &BrowseAsk,
    pool: Option<&str>,
    now: u64,
    gone: &dyn Fn() -> bool,
) -> Option<Answer> {
    let finds = |page: &str| follows(held, browse.chat, pool, page);
    asked_about_the_login(held, browse, pool, now, gone, &finds)
}

/// [`before_the_login`], each page the call names followed to its login form by `finds`.
fn asked_about_the_login(
    held: &Held,
    browse: &BrowseAsk,
    pool: Option<&str>,
    now: u64,
    gone: &dyn Fn() -> bool,
    finds: FindsForm<'_>,
) -> Option<Answer> {
    let Browsing::LogIn {
        login, suggested, ..
    } = &browse.asked
    else {
        return None;
    };
    let suggested = suggested.as_ref()?;
    let session = browse.chat;
    let always = crate::chatbrowsers::as_persona_of(held, session).always;
    // A browser kept off is told so by the normal path, and nothing is followed for it.
    if held.browsers().is_kept_off(session)
        || !held.browsers().may_browse(session, always.is_some())
    {
        return None;
    }
    if held.chats().sandboxed_by_its_host(session) {
        return None;
    }
    let (asker, unconfined) = held.chats().login_asker(held.root(), session)?;
    // Whose vaults it reads, as the read judges a link: the persona it was started as (#5831).
    let reads_as = asker.persona.unwrap_or_default();
    // Whose login grants it reads, and whose tools Always writes in: a narrowed chat's are the
    // persona it was narrowed to (the decision of 2026-11-06).
    let persona = held
        .chats()
        .secret_grants_of(session)
        .filter(|persona| !persona.is_empty())?;
    if pool.is_some() && unconfined {
        return None;
    }
    let no = |why: String| Some(Answer::No { why });
    let grants = held.login_grants();
    let chat = held.chats().id_of(session);
    if grants
        .granted(session, chat.as_deref(), Some(&persona), login)
        .is_some()
    {
        return None;
    }
    // A declared login a call names another site for asks to replace that site (#4996).
    let declared = declared_now(held.root(), &persona, login);
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
    // Nothing is opened for a login the person kept blocked here, or while one is being asked
    // about for this chat: following a page to its form opens a browser (#4995).
    // Kept on a declared login is its recorded site kept, not the login blocked (#4996).
    let kept = || match declared.as_ref().and_then(|tool| tool.login.as_ref()) {
        Some(recorded) => kept_where_it_is_said(login, &recorded.url, None),
        None => kept_said(login, None),
    };
    if let Ok((origin, _)) = loginask::site_of(&suggested.site)
        && grants.kept(&persona, login, &origin)
    {
        return no(kept());
    }
    if grants.waiting().iter().any(|one| one.session == session) {
        return no(ALREADY_ASKED.to_owned());
    }
    let Some(_following) = grants.follow(session) else {
        return no(ALREADY_ASKED.to_owned());
    };
    let wait = match judged(
        held.root(),
        Whose {
            grants: &persona,
            reads_as: &reads_as,
        },
        login,
        suggested,
        finds,
        declared.as_ref(),
        &grants.held(),
    ) {
        // The site the call names is the one declared: the declared login fills it.
        Ok(None) => return None,
        Ok(Some(judged)) if grants.kept(&persona, login, &judged.proposal.origin) => {
            return no(kept());
        }
        Ok(Some(judged)) => LoginWait {
            id: 0,
            session,
            chat,
            since: now,
            pool: pool.map(str::to_owned),
            proposal: judged.proposal,
            vault_too: judged.vault_too,
            no_always: judged.no_always,
            reads_as,
        },
        Err(why) => return no(why),
    };
    let replaces = wait.proposal.replaces.clone();
    let (id, heard) = match grants.raise(wait) {
        Ok(raised) => raised,
        Err(why) => return no(why),
    };
    tracing::info!("purlis: chat {session}'s browser login asks the person about its site");
    held.chats().asks_may_have_moved();
    if let Some(pool) = pool {
        let_go_unanswered(
            held,
            session,
            login,
            (id, heard),
            loginask::THE_PERSON_IS_WAITED_FOR,
        );
        return no(asked_on_a_runner_said(login, pool));
    }
    let waited =
        waited_for_the_person(grants, id, &heard, loginask::THE_PERSON_IS_WAITED_FOR, gone);
    match waited {
        Ended::Heard(Heard::Allowed) => None,
        Ended::Heard(Heard::Kept(reason)) => no(match &replaces {
            Some(old) => kept_where_it_is_said(login, &old.page, reason.as_deref()),
            None => kept_said(login, reason.as_deref()),
        }),
        Ended::Heard(Heard::Gone) | Ended::Closed => no(NOT_ANSWERED.to_owned()),
        Ended::NotAnswered => {
            held.chats().asks_may_have_moved();
            no(NOT_ANSWERED.to_owned())
        }
        Ended::AskerGone => {
            tracing::info!("purlis: chat {session}'s browser login call ended, so its notice went");
            held.chats().asks_may_have_moved();
            no(ASKER_GONE.to_owned())
        }
    }
}

/// What a call is answered where its asker hung up while its notice waited: nobody reads it.
/// A secret fill's call is answered the same ([`crate::secretfills`]).
pub(crate) const ASKER_GONE: &str = "the call ended while the person was asked, so its notice \
                                     went and nothing was filled";

/// A login's wait and a secret fill's both wait on [`waited`], as the Secret ask a fill raises
/// does.
pub(crate) use purlis_core::askwait::{Ended, waited};

/// **The answer to the login notice numbered `id`** ([`waited`]).
fn waited_for_the_person(
    grants: &Grants,
    id: u32,
    heard: &mpsc::Receiver<Heard>,
    at_most: Duration,
    gone: &dyn Fn() -> bool,
) -> Ended<Heard> {
    // A press in that very moment has ended it, and its answer is waiting here.
    let settle = || {
        if grants.end(id, Heard::Gone) {
            None
        } else {
            heard.try_recv().ok()
        }
    };
    waited(heard, at_most, gone, &settle)
}

/// **A runner chat's notice lets go after as long as a waiting call waits** (the person's ruling
/// of 2026-11-07): nothing waits on it, so where nobody answered by then it goes, and the chat is
/// told so on its next turn. A press or the chat's close ends the watch. `at_most` is that
/// time.
fn let_go_unanswered(
    held: &Held,
    session: u32,
    login: &str,
    (id, heard): (u32, mpsc::Receiver<Heard>),
    at_most: Duration,
) {
    let held = held.weak();
    let login = login.to_owned();
    let watched = std::thread::Builder::new()
        .name(format!("login notice {id}"))
        .spawn(move || {
            let went = went_unanswered(&heard, at_most, || {
                held.upgrade()
                    .filter(|held| held.login_grants().end(id, Heard::Gone))
            });
            if let Some(held) = went {
                held.chats().asks_may_have_moved();
                held.chats().tell_outcome(
                    session,
                    purlis_core::askoutcome::Outcome {
                        about: purlis_core::askoutcome::About::BrowserLogin { login },
                        answer: purlis_core::askoutcome::Answer::NotAnswered,
                    },
                );
            }
        });
    if let Err(why) = watched {
        tracing::warn!("purlis: login notice {id} has no time limit: {why}");
    }
}

/// **What a notice nobody waits on went with**: `None` where it was answered or its chat closed
/// within `at_most`; otherwise what `ends` answers, which ends it unless a press just did.
fn went_unanswered<T>(
    heard: &mpsc::Receiver<Heard>,
    at_most: Duration,
    ends: impl FnOnce() -> Option<T>,
) -> Option<T> {
    match heard.recv_timeout(at_most) {
        Err(mpsc::RecvTimeoutError::Timeout) => ends(),
        Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => None,
    }
}

/// **Where `page` sends chat `session`'s browser to sign in** (#4995), followed in a browser of
/// purlis's own that reaches what the chat's reaches, on the runner of `pool` where it runs.
fn follows(held: &Held, session: u32, pool: Option<&str>, page: &str) -> Result<String, String> {
    let always = crate::chatbrowsers::as_persona_of(held, session).always;
    held.browsers().find_login_form(
        held.browser_driver(),
        held.root(),
        pool,
        session,
        crate::chatbrowsers::reach_of(held, session),
        always,
        page,
    )
}

/// **Why a login allowed from its notice may not be filled now**, where policy changed since the
/// press: a persona's tools forbidden, or a vault its persona isn't tagged for that policy no
/// longer lets the person allow.
pub fn refused_now(root: &Path, persona: &str, tool: &Tool) -> Option<String> {
    let locks = Locks::of(root);
    locks.refuses_tool(persona, &tool.name).or_else(|| {
        let ctx = Ctx::new(root, Env::from_process());
        brokered::not_tagged(&ctx, Some(persona), &tool.vault)?;
        locks.vault_grants_refused()
    })
}

/// A suggestion as the project reads it now.
#[derive(Debug, PartialEq, Eq)]
struct Judged {
    proposal: Proposal,
    vault_too: bool,
    no_always: Option<String>,
}

/// **Whose a login is** (#5831): whose tools Always writes in, a narrowed chat's the persona it was
/// narrowed to, and whose vaults the chat reads, the persona it was started as, as the read judges
/// a link.
#[derive(Debug, Clone, Copy)]
struct Whose<'a> {
    grants: &'a str,
    reads_as: &'a str,
}

/// **Whether a suggestion is one the person is asked about**, and what they're offered: no tool
/// where policy forbids it, a key no link takes out of the chat's reach, a vault the project has,
/// and one the persona may use or that policy lets the person allow it.
///
/// Where `persona` declares `login` already (#4996), a suggestion of another site replaces its
/// site, and one of the very site it has is no ask at all: `None`.
fn judged(
    root: &Path,
    whose: Whose<'_>,
    login: &str,
    suggested: &Suggested,
    finds: FindsForm<'_>,
    declared: Option<&Tool>,
    held: &[Tool],
) -> Result<Option<Judged>, String> {
    let Whose {
        grants: persona,
        reads_as,
    } = whose;
    let locks = Locks::of(root);
    if let Some(why) = locks.refuses_tool(persona, login) {
        return Err(why);
    }
    // What needs no browser is judged before a page is followed to its form (#4995).
    let ctx = Ctx::new(root, Env::from_process());
    let doc = purlis_core::secrets::registry::load_registry(&ctx)
        .map_err(|why| why.message.to_string())?;
    purlis_core::secrets::registry::vault_in(&doc, &suggested.vault)
        .map_err(|why| why.message.to_string())?;
    if let Some(why) = out_of_reach(
        &ctx,
        reads_as,
        &suggested.vault,
        suggested.user_key.as_deref(),
        &suggested.key,
    ) {
        return Err(why);
    }
    let page = loginask::site_of(&suggested.site).map(|(_, page)| page);
    let declared_page = declared
        .and_then(|declared| declared.login.as_ref())
        .filter(|declared| page.as_ref() == Ok(&declared.url));
    if let Some(declared) = declared {
        let same_keys = declared.vault == suggested.vault
            && declared.login_keys()
                == Some((suggested.user_key.as_deref(), suggested.key.as_str()));
        if !same_keys {
            return Err(loginask::only_its_site_said(login));
        }
        // A line that records its form names its site whole: its page is enough to tell.
        if declared_page.is_some_and(|declared| declared.form.is_some()) {
            return Ok(None);
        }
    }
    let proposal = match loginask::proposal(root, persona, login, suggested, finds, held) {
        Ok(proposal) => proposal,
        // A declared login called on its own page fills as declared where the page can't be
        // followed: its fill holds the password to its origin all the same.
        Err(_) if declared_page.is_some() => return Ok(None),
        Err(why) => return Err(why),
    };
    let proposal = match declared {
        None => proposal,
        Some(declared) if proposal.is_where(declared) => return Ok(None),
        Some(declared) => proposal.replacing(root, declared, held)?,
    };
    offered_on_the_notice(root, &locks, proposal, reads_as, as_the_chat_reads_it).map(Some)
}

/// **Why `persona`, whose vaults the chat reads, may read neither the login's user key nor its
/// password** (#5831): one is a link into a vault it may not use, which the read would refuse once
/// the person had allowed it ([`brokered::read_out_of_reach`], #5795). Judged before the notice is
/// raised and as it is pressed, so no notice asks for, nor Always writes or grants a vault for, a
/// login refused.
fn out_of_reach(
    ctx: &Ctx,
    persona: &str,
    vault: &str,
    user_key: Option<&str>,
    key: &str,
) -> Option<String> {
    let keys: Vec<&str> = user_key.into_iter().chain([key]).collect();
    brokered::read_out_of_reach(ctx, persona, vault, &keys)
}

/// What the person is offered on `proposal`, under `locks`, for a chat whose vaults are
/// `reads_as`'s; a key out of reach refused in the words `out_of_reach_said` makes of the read's.
fn offered_on_the_notice(
    root: &Path,
    locks: &Locks,
    proposal: Proposal,
    reads_as: &str,
    out_of_reach_said: fn(&str) -> String,
) -> Result<Judged, String> {
    let ctx = Ctx::new(root, Env::from_process());
    let doc = purlis_core::secrets::registry::load_registry(&ctx)
        .map_err(|why| why.message.to_string())?;
    purlis_core::secrets::registry::vault_in(&doc, &proposal.vault)
        .map_err(|why| why.message.to_string())?;
    // Judged again as it is drawn or pressed: a link made while the notice waits is seen, and said
    // as its reader is told it, the chat as it is raised and the person as they press.
    if let Some(why) = out_of_reach(
        &ctx,
        reads_as,
        &proposal.vault,
        proposal.user_key.as_deref(),
        &proposal.key,
    ) {
        return Err(out_of_reach_said(&why));
    }
    let vault_too = brokered::not_tagged(&ctx, Some(&proposal.persona), &proposal.vault).is_some();
    let mut proposal = proposal;
    proposal
        .warnings
        .extend(loginask::not_in_vault(&ctx, &proposal));
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
    let no_always = always_held(root, locks, &proposal.persona);
    Ok(Judged {
        proposal,
        vault_too,
        no_always,
    })
}

/// **Why Always allow isn't offered for a line in `persona`'s tools**, where it isn't (#4595,
/// #5116): policy holds proposed changes or the scope back, the project's settings have no
/// `[sandbox]`, or the persona's list waits for the person's Allow, which a line's Allow would
/// carry with it unseen.
pub fn always_held(root: &Path, locks: &Locks, persona: &str) -> Option<String> {
    locks
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
            (!sandbox::persona::list_seen_in(root, &file, persona, locks)).then(|| {
                format!(
                    "Always allow isn't offered while {persona}'s hosts and tools wait for your \
                     Allow"
                )
            })
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

/// **Why a press on `wait` is refused, where a key of its login is a link into a vault the chat
/// may not use** (#5831), judged as the read judges it: [`refused_at_the_press_said`], the press's
/// error. Nothing is allowed, audited or written, and the notice stays up, as for every press
/// refused, so the person reads why on it and Keep blocked still ends it.
fn refused_out_of_reach(root: &Path, wait: &LoginWait) -> Result<(), String> {
    let ctx = Ctx::new(root, Env::from_process());
    let drawn = &wait.proposal;
    let Some(why) = out_of_reach(
        &ctx,
        &wait.reads_as,
        &drawn.vault,
        drawn.user_key.as_deref(),
        &drawn.key,
    ) else {
        return Ok(());
    };
    Err(refused_at_the_press_said(&why))
}

/// **The person's press** on the login a notice drew. Fails closed: a press on anything but what's
/// waiting now changes nothing. A site the person typed is followed to its form with `finds`
/// before anything is allowed on it (#4995).
pub fn answer(
    root: &Path,
    grants: &Grants,
    asked: &Asked<'_>,
    finds: FindsForm<'_>,
    audit: Audit<'_>,
) -> Result<Pressed, String> {
    const NO_MORE: &str = "That ask has gone, and nothing was allowed.";
    const CHANGED: &str = "That ask changed, so nothing was allowed. Read it again.";
    let Asked {
        session,
        id,
        press,
        shown,
        typed,
        reason,
    } = *asked;
    // A typed site is followed to its form before the press is held, so no other notice's press
    // waits on a browser (#4995); what waits is read again once it is.
    let followed = match (press, typed.map(str::trim)) {
        (Press::Keep, _) | (_, None | Some("")) => None,
        (_, Some(typed)) => {
            let wait = grants.wait(session, id).ok_or(NO_MORE)?;
            if wait.proposal.digest() != shown {
                return Err(CHANGED.to_owned());
            }
            // Nothing is followed in a browser for a login refused (#5831).
            refused_out_of_reach(root, &wait)?;
            Some(wait.proposal.at(root, typed, finds, &grants.held())?)
        }
    };
    let _one_press = grants
        .answering
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let wait = grants.wait(session, id).ok_or(NO_MORE)?;
    if wait.proposal.digest() != shown {
        return Err(CHANGED.to_owned());
    }
    if press == Press::Keep {
        if !grants.keep_wait(id, reason.map(str::to_owned)) {
            return Err(NO_MORE.to_owned());
        }
        let said = if wait.proposal.replaces.is_some() {
            "Kept the recorded site. Nothing changed or was filled."
        } else {
            "Kept blocked. Nothing was filled."
        };
        return Ok(Pressed::Done {
            said: said.to_owned(),
        });
    }
    // Judged as the read judges it (#5831): a key that became a link into a vault the persona may
    // not use while the notice waited is allowed nothing.
    refused_out_of_reach(root, &wait)?;
    // A field left empty names no site: it never falls back to the one the chat suggested.
    let moved = match (typed.map(str::trim), followed) {
        (Some(""), _) => {
            return Err("Type the site to fill this on, or Keep blocked.".to_owned());
        }
        (Some(typed), Some(followed)) => Some((typed, followed)),
        _ => None,
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
    // The form stays where it was found for this page: what the notice showed.
    let form = proposal.form.clone();
    let fresh = loginask::proposal(
        root,
        &proposal.persona,
        &proposal.login,
        &again,
        &|_| Ok(form.clone()),
        &grants.held(),
    )?;
    // A replacement is judged against the login as it's declared now (#4996): a line changed
    // since is a new notice, and one that has this site already leaves nothing to replace.
    let fresh = match &proposal.replaces {
        None => fresh,
        Some(_) => {
            let declared = declared_now(root, &proposal.persona, &proposal.login).ok_or(
                "That login is no longer in the project's settings, so nothing was changed.",
            )?;
            if fresh.is_where(&declared) {
                return Err("That login has this site already, so nothing was changed.".to_owned());
            }
            fresh.replacing(root, &declared, &grants.held())?
        }
    };
    // A link made since the judge above is refused by `offered_on_the_notice`, in the person's
    // words as above.
    let judged = offered_on_the_notice(
        root,
        &locks,
        fresh,
        &wait.reads_as,
        refused_at_the_press_said,
    )?;
    let changed = judged.proposal.digest() != wait.proposal.digest();
    // Always writes a line, and the line is shown before it's written. A warning the notice
    // didn't show is shown before a press counts; one it showed was read.
    let unseen = judged
        .proposal
        .warnings
        .iter()
        .any(|warning| !warning.seen_in(&wait.proposal.warnings));
    if changed && (!spelled || unseen || press == Press::Always) {
        let said = format!(
            "purlis would open {} and type the password on {}. Check it, then choose again.",
            judged.proposal.page, judged.proposal.form
        );
        grants.redraw(id, judged);
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
        written(
            root,
            &locks,
            &Line {
                persona: &proposal.persona,
                vault: &proposal.vault,
                vault_too: judged.vault_too,
                with: &|text| loginask::with_line(text, &proposal),
            },
            session,
            audit,
        )?;
        if proposal.replaces.is_some() {
            format!(
                "Replaced. {} opens {} in the project's settings now, and is allowed on this \
                 machine.",
                proposal.login, proposal.page
            )
        } else {
            format!(
                "Allowed. {} is in the project's settings, and allowed on this machine.",
                proposal.login
            )
        }
    } else {
        format!(
            "Allowed. Nothing was written. The password is typed on {} alone.",
            proposal.form
        )
    };
    if !grants.allow(id, &proposal.persona, tool, once) {
        return Err(NO_MORE.to_owned());
    }
    Ok(Pressed::Done { said })
}

/// **A line Always writes in a persona's tools** (#4595, #5116): whose, the vault it reads, and
/// how the project's file reads with it.
pub struct Line<'a> {
    pub persona: &'a str,
    pub vault: &'a str,
    /// The persona needs the vault allowed too.
    pub vault_too: bool,
    /// The project's file with the line, from the file as read.
    pub with: &'a dyn Fn(&str) -> Result<String, String>,
}

/// **Always**: the file with the one line, written only over what was read, then the persona's
/// list allowed as what was written says it, and its vault where it needs it.
pub fn written(
    root: &Path,
    locks: &Locks,
    line: &Line<'_>,
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
    if !sandbox::persona::list_seen_in(root, text, line.persona, locks) {
        return Err(NOT_WRITTEN.to_owned());
    }
    let after = (line.with)(text)?;
    let digest = sandbox::persona::digest_in(root, &after, line.persona, locks)
        .ok_or_else(|| NOT_WRITTEN.to_owned())?;
    let base = purlis_core::proposed::base_of(before.as_deref());
    purlis_core::proposed::write_pinned(root, rel, base.as_deref(), &after)
        .map_err(|_| NOT_WRITTEN.to_owned())?;
    sandbox::local::allow_persona_hosts(root, line.persona, &digest)
        .map_err(|why| format!("purlis wrote the line, and could not allow it here: {why}"))?;
    if line.vault_too {
        let grant = sandbox::local::VaultGrant {
            vault: line.vault.to_owned(),
            persona: line.persona.to_owned(),
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
    let pool = wait.as_ref().and_then(|wait| wait.pool.clone());
    let finds = |page: &str| follows(held, asked.session, pool.as_deref(), page);
    let answer = answer(
        held.root(),
        held.login_grants(),
        asked,
        &finds,
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
