// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The asks registry, as the window reads it** (#1690, spec #1688): everything that waits on
//! the person, one list per project, and the title bar's ✋ counts it.
//!
//! The registry is `purlis_core::harness::asks`: the asks a chat's harness raised (HP-5),
//! first answer wins, and only a human scope answers. This is its window side. It holds no
//! store of its own (I-8): each project's list is **derived, each time it is read**, from the
//! sources that wait today, one adapter each, so an ask leaves the list the moment its source
//! stops waiting, wherever it was answered.
//!
//! - **A permission prompt** a chat or a task holds on its hook ([`permission`]): answered on
//!   that hook, by [`answer_ask`].
//! - **A dispatch grant** the person has not answered ([`dispatch`]): answered by the dispatch
//!   Notice's own commands (`allow_dispatch`, `keep_dispatch_blocked`, `never_dispatch`).
//! - **A sandbox block ask**, a host a chat's sandbox refused or a folder it refused a write in
//!   ([`sandbox_block`]): answered by the block Notice's own `allow_sandbox_block`, and its Keep
//!   blocked: `keep_sandbox_block` for a host, [`forget_sandbox_block`] for a folder. **A live
//!   ask** (#1666), a connection the chat's proxy holds while the person is asked, is one of
//!   these: it is held for the chat as the proxy asks (`chats::asked_by_the_proxy`, #1709), and
//!   its line says the connection waits.
//! - **A prompt in a harness's own terminal** that purlis holds nothing for ([`terminal`]),
//!   and **a chat waiting on the person's reply** ([`question`]): named, with no choices; the
//!   person goes to the chat. A terminal prompt says which prompt it is, as the harness's
//!   nudge named it (#1691); where a harness's hook can answer it, it is a permission ask.
//!
//! **Each ask names the existing path that answers it** ([`AnswerPath`]): the window calls that
//! path's own command, checked and audited as it is from its Notice, so the registry opens no
//! route of its own. Every command a path names is the window's alone
//! (`purlis_session_protocol::ui::WINDOW_ONLY`): no chat, and no client but the window, can
//! answer an ask through it, its own least of all (V16).
//!
//! **What an ask says is data.** A chat's names, a brief, a host: each is a string the window
//! draws as text, never as markup or a control, and the choices an ask offers are purlis's
//! fixed words, or for a permission the harness's own option labels.
//!
//! # A chat's permission prompt (HP-6)
//!
//! A Claude Code chat's `PermissionRequest` hook hands its ask to this project's hook channel
//! and waits (`purlis_core::hookwire::permission`). The ask is held in the project's
//! [`HookAsks`], the window is told the asks it now has ([`EVENT`]), and the operator's choice
//! goes back on that hook ([`answer_ask`]), so the harness carries it out without its pane
//! having focus. Unanswered, the hook decides nothing and the pane asks as it always did.
//!
//! **The window answers, and nothing else does.** [`answer_ask`] is a Tauri command the
//! window invokes, admitted as `local-ui`; it is never served on the link to `charterd`
//! (`purlis_session_protocol::ui::WINDOW_ONLY`), and the session protocol refuses its own
//! `answer`. On the hook channel the host reads asks and writes back only the window's choice,
//! and the hook believes a reply only from a listener that is its own ancestor
//! (`purlis_same_user::admit_host`). An answer names its project, chat and ask, and lands only
//! on that chat's ask, once.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use purlis_core::harness::asks::{Admitted, Answerer, Raised};
use purlis_core::harness::hooked::HookAsks;
use purlis_core::harness::model::{Action, ChoiceKind};
use purlis_core::hookwire::Permitting;

use crate::planes::{PlaneId, Planes};

/// The event the window is told a project's asks on: [`Asking`].
pub const EVENT: &str = "asks-changed";

/// Every ask a project holds open, as the window lists them: the whole list each time, so the
/// window never assembles it from events it might have missed one of.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct Asking {
    pub plane: PlaneId,
    pub asks: Vec<Shown>,
}

/// One ask, as the registry lists it and its row draws it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct Shown {
    /// The chat that asked.
    pub session: u32,
    /// The ask's key, unique in its project: for a permission its id, which its answer names.
    pub ask: String,
    /// What it asks, in one line, every credential shape masked. Data, never markup.
    pub says: String,
    /// The answers it offers, in the source's order and words. Empty where it is answered
    /// only in its chat ([`AnswerPath::InItsPane`]).
    pub options: Vec<Offered>,
    /// Which source is waiting.
    pub source: AskSource,
    /// Who is asking, by name, the session first and the chat that asked last: `steward 12`,
    /// `#3046 drill`, `log watch` (I-9). From the app's own record of who asked whom, never
    /// from what a chat says of itself; the names are the chats' and are data.
    pub chain: Vec<String>,
    /// The existing path that answers it.
    pub answer: AnswerPath,
    /// When it began to wait, in seconds since 1970, where its source knows (#1700): what
    /// orders the longest waiting first. Null for a chat's own wait, a reply or a prompt in its
    /// terminal.
    #[specta(optional)]
    pub since: Option<u32>,
    /// **Until when its chat's proxy holds the connection it asks about** (#1709), in seconds
    /// since 1970: present only while a connection waits on the answer, so a window that sees it
    /// gone says plainly that the hold ran out. The window reads the list again at that time.
    #[specta(optional)]
    pub held_until: Option<u32>,
    /// **What an edit would write** (#1146): each change, drawn before its Allow, or why it is
    /// not shown and no Allow is offered. Absent for any other ask.
    #[specta(optional)]
    pub writes: Option<crate::askwrites::WritesShown>,
    /// **The lines a permission's action runs over** (#3502), where its line puts several on
    /// one: their count, and each as [`Shown::says`] shows it. Absent for an action on one line.
    #[specta(optional)]
    pub lines: Option<LinesShown>,
}

/// The lines an action runs over, as a row draws them under its line (#3502). An action whose
/// line holds several is never offered Allow (`hooked::shown_in_full`): its line is not what
/// would run.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct LinesShown {
    /// How many lines it runs over.
    pub count: u32,
    /// **Each line, as [`Shown::says`] has it**: joined by a space, they are that line,
    /// character for character, masked and cut as it is. Empty where they could not be shown
    /// that way; the row then says only the count.
    pub shown: Vec<String>,
}

/// Which source an ask waits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum AskSource {
    /// A permission prompt a chat or a task holds on its hook (HP-6).
    Permission,
    /// A dispatch to another persona that no grant covers (#1437).
    Dispatch,
    /// A host a chat's sandbox refused, which an Allow can name (#1342).
    SandboxHost,
    /// A folder a chat's sandbox refused a write in, which an Allow can name (#1700).
    SandboxWrite,
    /// A local socket or a system service a chat's sandbox refused, which an Allow can name
    /// (D-sock-1).
    SandboxLocal,
    /// A workspace a chat asked the person to remove, which only their press removes
    /// (qw356).
    WorkspaceRemoval,
    /// A change a chat proposed to purlis's own configuration, which only the person applies
    /// (#1975).
    ProposedChange,
    /// A change to the project a chat on a pool's runner asked for, which only the person
    /// applies (#2046).
    RunnerWrite,
    /// An open or a dispatch a chat on a pool's runner asked for, which starts only on the
    /// person's Approve.
    RunnerDispatch,
    /// A prompt shown in the harness's own terminal that purlis holds nothing for.
    Terminal,
    /// A chat whose turn ended with the next move the person's.
    Question,
    /// **A task's question that has reached the person** (ADR 0107): in the task's own words.
    TaskQuestion,
    /// A profile a chat could not start on, because its harness says it is not signed in on
    /// this device (#2035): answered by running the harness's own sign-in in a shell tab.
    SignIn,
    /// A sandboxed Codex chat refused for want of a sandbox-only profile (#2037): answered by
    /// making one and running Codex's own sign-in in it.
    SandboxOnly,
    /// A project that still holds the old `[harness_plugins]` keys purlis no longer reads
    /// (#3467): said once, answered by Remove, which proposes the edit, or Dismiss.
    OldHarnessPlugins,
    /// A chat's first browser call, asking what its browser may reach (#2013).
    Browser,
    /// A chat's browser whose page needs the person (#2015): the chat asked them to take it
    /// over, through `ask_operator` pointed at it.
    BrowserNeedsYou,
    /// Sites new to the project a chat's browser held back what it would open, send or type
    /// on (#2018): an Allow is a browser site grant, for the browser alone.
    BrowserSite,
    /// A runner chat's browser login, which waits on the person's opt-in for its persona on its
    /// pool (#2050): an Allow lets that persona's logins be filled on the pool's runner.
    PoolLogin,
    /// One run of a runner chat's declared tool, which waits on the person (#2047): an Allow
    /// lets this desktop send that tool its vault values on the pool's runner for that run alone.
    PoolTool,
}

/// **The existing, checked path an ask is answered by** (#1690): the command the window
/// already answers that source with, and what that command is sent. The registry adds no route.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "kebab-case", tag = "via")]
pub enum AnswerPath {
    /// `answer_ask`, back on the chat's own permission hook, by [`Shown::ask`].
    Hook,
    /// The dispatch Notice's own commands, for held dispatch `id`: `allow_dispatch` at the
    /// level an option names, with `shown`, the digest the question was told; then
    /// `keep_dispatch_blocked` and `never_dispatch`.
    Dispatch { id: u32, shown: String },
    /// The block Notice's own `allow_sandbox_block` at the level an option names, bound to the
    /// block shown; and for Keep blocked `keep_sandbox_block` on a host, [`forget_sandbox_block`]
    /// on a folder.
    SandboxBlock {
        shown: crate::taskblocks::BlockShown,
    },
    /// `keep_asked_workspace` for Keep; Remove… sends nothing, and opens the window's own
    /// Delete workspace on `workspace`, where the person's press removes (qw356). `workspace`
    /// is one this project lists; `memories` and `todos` are what it holds now, by the app's
    /// own count.
    WorkspaceRemoval {
        workspace: String,
        memories: u32,
        todos: u32,
    },
    /// `apply_proposed_change` for Apply and `dismiss_proposed_change` for Dismiss, on held
    /// proposal `id` (#1975); Open opens its own tab, titled by `path`, and sends nothing.
    ProposedChange { id: u32, path: String },
    /// `apply_runner_write` for Apply and `dismiss_runner_write` for Dismiss, on held runner
    /// write `id` (#2046); Open opens its own tab, which shows it whole, and sends nothing.
    RunnerWrite { id: u32 },
    /// `refuse_runner_dispatch` for Refuse, on held runner dispatch `id`; Open opens its own
    /// tab, which shows it whole with Approve (`approve_runner_dispatch`), and sends nothing.
    RunnerDispatch { id: u32 },
    /// Sign in sends nothing: it opens a shell tab where the core types the harness's own
    /// sign-in for `profile` (`type_profile_sign_in`), naming the provider an option names, and
    /// the Ask waits until the harness says it is signed in. Dismiss sends `dismiss_sign_in`.
    /// With `pool`, it is signed in on that pool's runner (#2049): the line typed is the
    /// runner's own sign-in, and both commands name the pool.
    SignIn {
        profile: String,
        pool: Option<String>,
    },
    /// Make one sends `make_sandbox_only_profile`, then opens a shell tab where the core types
    /// Codex's own sign-in for the profile it made (`type_profile_sign_in`). Dismiss sends
    /// `dismiss_sandbox_only`.
    SandboxOnly,
    /// `remove_old_harness_plugins` for Remove, which holds purlis's proposed change for each
    /// file and opens the first; `dismiss_old_harness_plugins` for Dismiss (#3467).
    OldHarnessPlugins,
    /// `allow_chat_browser`, with the open web for `open-web`, and `keep_chat_browser_off` for
    /// Keep off, with the person's reason (#2013).
    Browser,
    /// Take over sends `take_over_browser` for browser `program`, the session host's lease,
    /// then opens its view, so the person's first click or key reaches the page (#2015). The
    /// Ask ends on their Done (`hand_back_browser`), or with the browser.
    TakeOver { program: String },
    /// `allow_browser_sites` at the level an option names, and `keep_browser_sites_blocked` for
    /// Keep blocked with the person's reason, each for `sites`, the sites the ask showed (#2018).
    BrowserSites { sites: Vec<String> },
    /// `answer_pool_login`, allowing for Allow and not for Keep off with the person's reason,
    /// for the chat that asked (#2050).
    PoolLogin,
    /// `answer_pool_tool`, allowing for Allow this run and not for Keep off, for run `run` of
    /// the chat that asked (#2047).
    PoolTool { run: u32 },
    /// `answer_task_question`, through the form that reads the question from the core: the
    /// task's own words, with `why` it reached the person. `note` is the asking chat's own
    /// line, `from` being that chat, and is never the task's words.
    TaskQuestion {
        why: ReachedWhy,
        from: String,
        note: Option<String>,
    },
    /// Nothing the window can send: the person answers in the chat.
    InItsPane,
}

/// Why a task's question reached the person ([`AnswerPath::TaskQuestion`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum ReachedWhy {
    /// Its asking chat passed it on.
    PassedOn,
    /// Its asking chat had its turn and did nothing with it in time.
    NoReply,
    /// Its asking chat closed or crashed.
    AskerGone,
}

impl From<purlis_core::dispatchtalk::Reached> for ReachedWhy {
    fn from(why: purlis_core::dispatchtalk::Reached) -> Self {
        use purlis_core::dispatchtalk::Reached;
        match why {
            Reached::PassedOn => Self::PassedOn,
            Reached::NoReply => Self::NoReply,
            Reached::AskerGone => Self::AskerGone,
        }
    }
}

/// One answer an ask offers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct Offered {
    pub id: String,
    pub label: String,
    /// Whether choosing it lets the call run.
    pub allows: bool,
    /// **The answer marked as the one the source suggests** (#2241): a persona's declared
    /// browser. Marked in its place, never moved, and never an answer: only the person's press
    /// answers. Absent (rather than `false`) wherever nothing is marked.
    #[specta(optional)]
    pub preselected: Option<bool>,
}

/// Told a project's asks each time they change.
pub type Teller = Arc<dyn Fn(Asking) + Send + Sync + 'static>;

/// Where a project's [`Teller`] goes once there is one: a slot, filled after the hook channel
/// is listening, as the project's other listeners are (`hooks::Hooks`).
pub type Telling = Arc<Mutex<Option<Teller>>>;

/// The asks `hooks` holds now, as the window lists them for `plane`.
pub fn asking(plane: &PlaneId, hooks: &HookAsks) -> Asking {
    Asking {
        plane: plane.clone(),
        asks: hooks
            .pending(Instant::now())
            .into_iter()
            .filter_map(permission)
            .collect(),
    }
}

/// **The permission adapter**: a held hook ask as its row draws it, or none for one whose chat
/// is no chat of this project's.
pub fn permission(raised: Raised) -> Option<Shown> {
    let session = raised.chat.parse().ok()?;
    let ask = raised.ask;
    let writes = crate::askwrites::of(&ask);
    let said = ask.summary.as_str();
    let says = if said.is_empty() {
        match &ask.action {
            Action::Command { line } => format!("Run {line}"),
            Action::Edit { path } => format!("Change {path}"),
            Action::Tool { name, .. } => format!("Use {name}"),
            Action::Elicit { .. } => "Asks you for values".to_owned(),
            Action::Question { .. } => "Asks you a question".to_owned(),
            Action::Unsaid => "Asks for your permission".to_owned(),
        }
    } else {
        said.to_owned()
    };
    let says = said_inertly(&says);
    let lines = lines_of(&ask, &says);
    Some(Shown {
        session,
        ask: raised.id.to_string(),
        says,
        options: ask
            .options
            .into_iter()
            .map(|option| Offered {
                allows: option.kind == ChoiceKind::Allow,
                preselected: None,
                id: option.id,
                label: option.label,
            })
            .collect(),
        source: AskSource::Permission,
        chain: Vec::new(),
        answer: AnswerPath::Hook,
        since: None,
        held_until: None,
        writes,
        lines,
    })
}

/// **The lines `ask`'s command runs over, as the row shows `says`**, its line (#3502): `None`
/// for any other action, a command on one line, or one whose summary is not made from it. Each
/// line is kept only where, joined by a space, they are `says`.
fn lines_of(ask: &purlis_core::harness::model::Ask, says: &str) -> Option<LinesShown> {
    let Action::Command { line } = &ask.action else {
        return None;
    };
    // Most commands are on one line: no summary is made again for them.
    if !line.contains('\n') {
        return None;
    }
    let said = ask.action.said();
    if ask.summary != purlis_core::harness::model::Summary::of(&said) {
        return None;
    }
    let lines =
        purlis_core::harness::model::Summary::lines_of(&purlis_core::dispatchgrant::inert(&said))?;
    Some(LinesShown {
        count: u32::try_from(lines.count).unwrap_or(u32::MAX),
        shown: if lines.shown.join(" ") == says {
            lines.shown
        } else {
            Vec::new()
        },
    })
}

/// Tells the window `plane`'s asks, where something listens.
pub fn tell(plane: &PlaneId, hooks: &HookAsks, telling: &Telling) {
    let teller = telling
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    if let Some(teller) = teller {
        teller(asking(plane, hooks));
    }
}

/// What hears a permission hook's ask on `plane`'s channel: held in `hooks` until it is
/// answered, its hook goes away, or its deadline passes, and the window told each change.
pub fn permitting(plane: PlaneId, hooks: Arc<HookAsks>, telling: Telling) -> Permitting {
    let told = Arc::clone(&hooks);
    purlis_core::hookwire::permission::held_in(
        hooks,
        Arc::new(move || tell(&plane, &told, &telling)),
    )
}

/// The permission asks a project holds open now: what the window lists before any [`EVENT`]
/// arrives.
///
/// A link is served it too, so it sees each ask waiting, but never an edit's change: the UI
/// RPC serves an edit's `writes` as not shown (`purlis_session_protocol::ui::WINDOW_ONLY_FIELDS`),
/// since only the window draws it and gives its Allow.
#[tauri::command]
#[specta::specta]
pub fn pending_asks(planes: tauri::State<'_, Planes>, plane: PlaneId) -> Result<Asking, String> {
    Ok(planes.held(&plane)?.asks())
}

/// **Every ask a project has waiting now** (#1690), from every source ([`every_ask`]): what
/// the window lists and the title bar counts. Read again whenever a source may have moved:
/// [`EVENT`], a chat moving, a dispatch held, a block heard, an answer.
///
/// The window's alone: what waits on the person, dispatches and refused hosts included, is
/// never a link's to read, as `dispatch_grants_needed` is not.
#[tauri::command]
#[specta::specta]
pub fn asks_waiting(planes: tauri::State<'_, Planes>, plane: PlaneId) -> Result<Asking, String> {
    Ok(every_ask(&*planes.held(&plane)?))
}

/// Answers chat `session`'s ask `ask` with its option `option`, as the operator in this window.
///
/// The answer goes back on the channel the ask came on, the chat's own permission hook, and
/// the harness carries it out; nothing is typed into the chat. The first answer wins: one that
/// comes too late, twice, or for another chat's ask is refused with a sentence saying why.
#[tauri::command]
#[specta::specta]
pub fn answer_ask(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    ask: String,
    option: String,
) -> Result<(), String> {
    planes.held(&plane)?.answer_ask(session, &ask, &option)
}

/// The window, as an answerer: a Tauri command is invoked only by the app's own window, which
/// is the `local-ui` scope (ADR 0068 §5).
pub fn the_window() -> Answerer {
    Answerer::admitted(Admitted::LocalUi).expect("the window is a human scope")
}

/// **The words an Allow says at each level** (#1700): written once, here. Every Notice and the
/// Inbox draw the label an ask's option carries ([`Offered::label`]) and spell none of their own.
/// A sandbox ask's this-chat Allow says "only", since its main button is the wider one (#1666,
/// N-3); a dispatch's lists this chat first.
fn allow_said(level: crate::sandboxing::GrantLevel, sandbox: bool) -> &'static str {
    use crate::sandboxing::GrantLevel;
    match level {
        GrantLevel::Once => "Allow once",
        GrantLevel::Chat if sandbox => "Allow only for this chat",
        GrantLevel::Chat => "Allow for this chat",
        GrantLevel::You => "Allow for me on this machine",
        GrantLevel::Machine => "Allow on this machine, for every project",
        GrantLevel::Project => "Allow for everyone in this project",
    }
}

/// What Keep blocked says, for a dispatch and a sandbox ask alike.
const KEEP_SAID: &str = "Keep blocked";
/// What a dispatch's Never for this pair says.
const NEVER_SAID: &str = "Never for this pair";

/// The option id an Allow at `level` is sent as: the level's own word, as the Notice sends it.
pub fn level_id(level: crate::sandboxing::GrantLevel) -> &'static str {
    use crate::sandboxing::GrantLevel;
    match level {
        GrantLevel::Once => "once",
        GrantLevel::Chat => "chat",
        GrantLevel::You => "you",
        GrantLevel::Machine => "machine",
        GrantLevel::Project => "project",
    }
}

/// The option id of Keep blocked, for a dispatch and a sandbox host alike.
pub const KEEP: &str = "keep";
/// The option id of a dispatch's Never for this pair (#1503).
pub const NEVER: &str = "never";

fn allows(
    levels: &[crate::sandboxing::GrantLevel],
    sandbox: bool,
) -> impl Iterator<Item = Offered> + '_ {
    levels.iter().map(move |level| Offered {
        id: level_id(*level).to_owned(),
        label: allow_said(*level, sandbox).to_owned(),
        allows: true,
        preselected: None,
    })
}

fn refuses(id: &str, label: &str) -> Offered {
    Offered {
        id: id.to_owned(),
        label: label.to_owned(),
        allows: false,
        preselected: None,
    }
}

/// **The dispatch grant adapter** (#1437): a dispatch held for the person, as its Notice was
/// told it. It offers the Notice's answers with no box ticked: Allow at each level the question
/// offers, Keep blocked, and Never for this pair where the asking chat runs as a persona. A
/// dispatch policy locked offers nothing here: its Notice only says so.
pub fn dispatch(told: &crate::dispatchgrants::DispatchPending) -> Shown {
    let says = if told.mode == purlis_core::reopen::Mode::Handoff.word() {
        format!("Wants to hand its work off to {}", told.target)
    } else {
        format!("Wants to hand a task to {}", told.target)
    };
    let says = said_inertly(&says);
    let options = if told.locked.is_some() {
        Vec::new()
    } else {
        allows(&told.levels, false)
            .chain([refuses(KEEP, KEEP_SAID)])
            .chain(told.asking.is_some().then(|| refuses(NEVER, NEVER_SAID)))
            .collect()
    };
    Shown {
        session: told.session,
        ask: format!("dispatch:{}", told.id),
        says,
        answer: if options.is_empty() {
            AnswerPath::InItsPane
        } else {
            AnswerPath::Dispatch {
                id: told.id,
                shown: told.shown.clone(),
            }
        },
        options,
        source: AskSource::Dispatch,
        chain: Vec::new(),
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    }
}

/// **The sandbox host adapter** (#1342, #1538): a host chat `session`'s sandbox refused, held
/// now. Allow is offered at each level `locks`, an administrator's policy, leaves open for that
/// host, as its Notice offers it, and Keep blocked. A host the block did not name is typed in
/// its Notice, so it is answered there ([`AnswerPath::InItsPane`]). A folder's block is
/// [`sandbox_write`]'s.
pub fn sandbox_host(
    session: u32,
    block: &crate::taskblocks::HeldBlock,
    locks: &purlis_core::sandbox::policy::Locks,
) -> Option<Shown> {
    use crate::sandboxing::{GrantLevel, GrantWhat};
    use purlis_core::sandbox::grant::{self, What};
    if block.what != GrantWhat::Host {
        return None;
    }
    let named = (!block.target.is_empty())
        .then(|| grant::host(&block.target).ok())
        .flatten();
    let Some(host) = named else {
        // The host is typed in its Notice, so nothing here can send an answer; the Notice draws
        // these words for its Allow (#1700), at the levels policy leaves open there.
        return Some(Shown {
            session,
            ask: block_key(session, block),
            says: "The sandbox refused a host it did not name".to_owned(),
            options: allows(
                &[GrantLevel::You, GrantLevel::Chat, GrantLevel::Project],
                true,
            )
            .chain([refuses(KEEP, KEEP_SAID)])
            .collect(),
            source: AskSource::SandboxHost,
            chain: Vec::new(),
            answer: AnswerPath::InItsPane,
            since: None,
            held_until: None,
            writes: None,
            lines: None,
        });
    };
    // In its Notice's order (#1666, N-3): this project on this machine is the main button.
    let levels: Vec<GrantLevel> = [GrantLevel::You, GrantLevel::Chat, GrantLevel::Project]
        .into_iter()
        .filter(|level| {
            locks
                .refuses_grant(&What::Host(host.clone()), (*level).into())
                .is_none()
        })
        .collect();
    let says = format!("The sandbox refused {}", block.target);
    Some(Shown {
        session,
        ask: block_key(session, block),
        says: said_inertly(&says),
        options: allows(&levels, true)
            .chain([refuses(KEEP, KEEP_SAID)])
            .collect(),
        source: AskSource::SandboxHost,
        chain: Vec::new(),
        answer: AnswerPath::SandboxBlock {
            shown: crate::taskblocks::BlockShown {
                operation: block.operation.clone(),
                kind: block.kind.clone(),
                what: GrantWhat::Host,
                target: block.target.clone(),
            },
        },
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    })
}

/// **What an ask says, drawn safely** (#1688, I-1): one line, every credential shape masked, and
/// every character that draws as nothing (one that turns the text around, or hides what
/// follows) written out as its escape, so a folder's or a chat's name cannot make the line read
/// as something else, in the Inbox or on a pane.
fn said_inertly(says: &str) -> String {
    purlis_core::harness::model::Summary::of(&purlis_core::dispatchgrant::inert(says))
        .as_str()
        .to_owned()
}

/// The key of a block ask, unique in its project: the chat, the block's operation and kind, and
/// what it names (empty for a host it did not name).
fn block_key(session: u32, block: &crate::taskblocks::HeldBlock) -> String {
    format!(
        "block:{session}:{}:{}:{}",
        block.operation, block.kind, block.target
    )
}

/// **The folder-write adapter** (#1700, D-1700-1): a folder chat `session`'s sandbox refused a
/// write in, held now, is an ask as a host is, since its Notice offers Allow and Keep blocked
/// too. Allow is offered for this chat and for the person, as its Notice offers it (a folder is
/// never everyone's), at each level `locks` leaves open; then Keep blocked.
pub fn sandbox_write(
    session: u32,
    block: &crate::taskblocks::HeldBlock,
    locks: &purlis_core::sandbox::policy::Locks,
) -> Option<Shown> {
    use crate::sandboxing::{GrantLevel, GrantWhat};
    use purlis_core::sandbox::grant::What;
    if block.what != GrantWhat::Write || block.target.is_empty() {
        return None;
    }
    let folder = What::Write(std::path::PathBuf::from(&block.target));
    let levels: Vec<GrantLevel> = [GrantLevel::Chat, GrantLevel::You]
        .into_iter()
        .filter(|level| locks.refuses_grant(&folder, (*level).into()).is_none())
        .collect();
    let says = format!("The sandbox refused a write in {}", block.target);
    Some(Shown {
        session,
        ask: block_key(session, block),
        says: said_inertly(&says),
        options: allows(&levels, true)
            .chain([refuses(KEEP, KEEP_SAID)])
            .collect(),
        source: AskSource::SandboxWrite,
        chain: Vec::new(),
        answer: AnswerPath::SandboxBlock {
            shown: crate::taskblocks::BlockShown {
                operation: block.operation.clone(),
                kind: block.kind.clone(),
                what: GrantWhat::Write,
                target: block.target.clone(),
            },
        },
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    })
}

/// **The sandbox block adapter**: a block chat `session` is held on, as a host ask
/// ([`sandbox_host`]) or a folder-write ask ([`sandbox_write`]). `declared` is what the
/// project's committed file says it expects, which a socket's or a service's ask names.
pub fn sandbox_block(
    session: u32,
    block: &crate::taskblocks::HeldBlock,
    locks: &purlis_core::sandbox::policy::Locks,
    declared: &purlis_core::sandbox::services::Declared,
) -> Option<Shown> {
    sandbox_host(session, block, locks)
        .or_else(|| sandbox_write(session, block, locks))
        .or_else(|| sandbox_local(session, block, locks, declared))
        .or_else(|| sandbox_runner(session, block, None))
        .or_else(|| sandbox_port(session, block, locks))
}

/// **The runner's service adapter** (#2510, ruled 2026-10-18): a service on the runner chat
/// `session` runs on, which that runner left out of its plan and asks about, held now, is an
/// ask as a host is. Allow is offered once and for this chat alone, never wider: it lasts while
/// the chat runs there and keeps nothing on this machine. It names the runner, `runner`, where
/// the window knows its name. Then Keep blocked.
pub fn sandbox_runner(
    session: u32,
    block: &crate::taskblocks::HeldBlock,
    runner: Option<&str>,
) -> Option<Shown> {
    use crate::sandboxing::{GrantLevel, GrantWhat};
    if block.kind != purlis_core::sandboxblock::Kind::RunnerService.word() {
        return None;
    }
    let port: u16 = block.target.parse().ok()?;
    // Short enough to be said whole: what allowing it means at length is on its Notice.
    let says = format!(
        "{} Allowing it lets this chat alone reach it there, and nothing on this Mac.",
        runner.map_or_else(
            || format!("The runner this chat runs on refused its service at port {port}."),
            |name| format!("The runner {name} refused its service at port {port}."),
        ),
    );
    Some(Shown {
        session,
        ask: block_key(session, block),
        says: said_inertly(&says),
        options: allows(&[GrantLevel::Once, GrantLevel::Chat], true)
            .chain([refuses(KEEP, KEEP_SAID)])
            .collect(),
        source: AskSource::SandboxLocal,
        chain: Vec::new(),
        answer: AnswerPath::SandboxBlock {
            shown: crate::taskblocks::BlockShown {
                operation: block.operation.clone(),
                kind: block.kind.clone(),
                what: GrantWhat::Port,
                target: block.target.clone(),
            },
        },
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    })
}

/// **The service-on-this-Mac adapter** (qw384): a port of this machine chat `session`'s
/// sandbox refused, to connect to or to listen on (#2019), held now, is an ask as a host is. Allow is offered for this chat and for
/// the person (never once, never everyone's: it is this machine's and lasts past a run), at
/// each level `locks` leaves open; then Keep blocked. A port the block did not name is typed in
/// its Notice, so it is answered there ([`AnswerPath::InItsPane`]).
pub fn sandbox_port(
    session: u32,
    block: &crate::taskblocks::HeldBlock,
    locks: &purlis_core::sandbox::policy::Locks,
) -> Option<Shown> {
    use crate::sandboxing::{GrantLevel, GrantWhat};
    // A port to listen on (#2019) is asked about as a port to connect to is, at the same two
    // scopes, in its own words.
    let listen = match block.what {
        GrantWhat::Port => false,
        GrantWhat::Listen => true,
        _ => return None,
    };
    let order = [GrantLevel::Chat, GrantLevel::You];
    let named: Option<u16> = block.target.parse().ok();
    let levels: Vec<GrantLevel> = order
        .into_iter()
        .filter(|level| match named {
            Some(port) => block
                .what
                .of_port(port)
                .is_some_and(|what| locks.refuses_grant(&what, (*level).into()).is_none()),
            None => {
                locks.port_grants_refused().is_none()
                    && locks.scope_removed((*level).into()).is_none()
            }
        })
        .collect();
    let says = match (named, listen) {
        (Some(port), false) => format!(
            "The sandbox refused the service on this Mac at port {port}. Allowing it {}.",
            purlis_core::sandbox::localport::RISK
        ),
        (None, false) => {
            "The sandbox refused a service on this Mac at a port it did not name".to_owned()
        }
        (Some(port), true) => format!(
            "This chat wants to run a server on port {port}. Allowing it {}.",
            purlis_core::sandbox::localport::listen_risk(port)
        ),
        (None, true) => "This chat wants to run a server on a port it did not name".to_owned(),
    };
    Some(Shown {
        session,
        ask: block_key(session, block),
        says: said_inertly(&says),
        options: allows(&levels, true)
            .chain([refuses(KEEP, KEEP_SAID)])
            .collect(),
        source: AskSource::SandboxLocal,
        chain: Vec::new(),
        answer: match named {
            Some(_) => AnswerPath::SandboxBlock {
                shown: crate::taskblocks::BlockShown {
                    operation: block.operation.clone(),
                    kind: block.kind.clone(),
                    what: block.what,
                    target: block.target.clone(),
                },
            },
            None => AnswerPath::InItsPane,
        },
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    })
}

/// **The socket and service adapter** (D-sock-1): a local socket or a system service chat
/// `session`'s sandbox refused, held now, is an ask as a host is. Allow is offered once, for
/// this chat and for the person (never everyone's: it is this machine's), at each level `locks`
/// leaves open; then Keep blocked. What it says names it whole and what allowing it means, led
/// by "This project expects it" where `declared` names it, as the block's own Notice says
/// (ADR 0093, #1888). The certificate check is never asked about here: it is no grant (R1,
/// 2026-10-13).
pub fn sandbox_local(
    session: u32,
    block: &crate::taskblocks::HeldBlock,
    locks: &purlis_core::sandbox::policy::Locks,
    declared: &purlis_core::sandbox::services::Declared,
) -> Option<Shown> {
    use crate::sandboxing::{GrantLevel, GrantWhat};
    use purlis_core::sandbox::grant::What;
    use purlis_core::sandbox::services;
    if block.target.is_empty() {
        return None;
    }
    let (what, named) = match block.what {
        GrantWhat::Socket => (
            What::Socket(std::path::PathBuf::from(&block.target)),
            "the local socket",
        ),
        GrantWhat::Service => (What::Service(block.target.clone()), "the system service"),
        _ => return None,
    };
    // The narrowest answer first: a container engine's socket is about the same as no sandbox,
    // and a service purlis cannot tell could be anything. Then **every project on this
    // machine** (R2, 2026-10-13), the standing answer the Notice draws beside it, so one press
    // is the last for a thing every project of the person's uses; only this chat, and this
    // project alone, are the other scopes.
    let order = [
        GrantLevel::Once,
        GrantLevel::Machine,
        GrantLevel::Chat,
        GrantLevel::You,
    ];
    let levels: Vec<GrantLevel> = order
        .into_iter()
        .filter(|level| locks.refuses_grant(&what, (*level).into()).is_none())
        .collect();
    let says = format!(
        "The sandbox refused {named} {}. {}",
        block.target,
        services::said_of(&what, declared)
    );
    Some(Shown {
        session,
        ask: block_key(session, block),
        says: said_inertly(&says),
        options: allows(&levels, true)
            .chain([refuses(KEEP, KEEP_SAID)])
            .collect(),
        source: AskSource::SandboxLocal,
        chain: Vec::new(),
        answer: AnswerPath::SandboxBlock {
            shown: crate::taskblocks::BlockShown {
                operation: block.operation.clone(),
                kind: block.kind.clone(),
                what: block.what,
                target: block.target.clone(),
            },
        },
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    })
}

/// **The harness-terminal adapter**: chat `session` is stopped on a prompt it asked mid-turn,
/// shown in its own terminal, and purlis holds nothing it could answer it by. Named, so it
/// does not wait unseen, with the kind of prompt its harness's nudge said (#1691); the person
/// answers it in the chat.
pub fn terminal(session: u32, prompt: purlis_core::harness::model::Prompt) -> Shown {
    Shown {
        session,
        ask: format!("terminal:{session}"),
        says: prompt.says().to_owned(),
        options: Vec::new(),
        source: AskSource::Terminal,
        chain: Vec::new(),
        answer: AnswerPath::InItsPane,
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    }
}

/// **The question adapter**: chat `session` is in the needs-you queue with no prompt open,
/// its turn over and the next move the person's. Answered by replying in the chat.
pub fn question(session: u32) -> Shown {
    Shown {
        session,
        ask: format!("question:{session}"),
        says: "Waiting on your reply".to_owned(),
        options: Vec::new(),
        source: AskSource::Question,
        chain: Vec::new(),
        answer: AnswerPath::InItsPane,
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    }
}

/// **The task question adapter** (ADR 0107): a question that has reached the person, one item
/// for the ✋ and the Inbox, in the task's own words, masked as any ask's line is. Its
/// `chain` says which tasks it came through.
pub fn task_question(
    reached: purlis_core::dispatchtalk::ReachedYou,
    from: String,
    chain: Vec<String>,
) -> Shown {
    use purlis_core::dispatchrecord::masked;
    Shown {
        session: reached.task,
        ask: format!("task-question:{}:{}", reached.task, reached.number),
        says: purlis_core::shown::one_line(&masked(&reached.text), purlis_core::shown::NO_CLIP),
        options: Vec::new(),
        source: AskSource::TaskQuestion,
        chain,
        answer: AnswerPath::TaskQuestion {
            why: reached.why.into(),
            from,
            note: reached.note,
        },
        since: u32::try_from(reached.at).ok(),
        held_until: None,
        writes: None,
        lines: None,
    }
}

/// The option id of Remove… on a workspace's removal (qw356): the window's
/// `OPENS_DELETE_WORKSPACE`.
pub const REMOVE: &str = "remove";
/// What Remove… and Keep say on a workspace's removal: written once, here. Remove… opens a
/// dialog, so it is spelled as what opens one is.
const REMOVE_SAID: &str = "Remove…";
const KEEP_IT_SAID: &str = "Keep";

/// The key of a workspace removal's ask, unique in its project: the chat and the workspace.
fn removal_key(session: u32, workspace: &str) -> String {
    format!("removal:{session}:{workspace}")
}

/// **The workspace removal adapter** (qw356): a workspace a chat asked the person to remove,
/// held now. It offers Remove…, which opens the window's own Delete workspace and removes
/// nothing itself, and Keep. Every word is
/// purlis's; the workspace's name is the project's own listing's, drawn as data, and what it
/// holds is counted by the app.
pub fn workspace_removal(
    waits: &crate::removalasks::Waits,
    holds: purlis_core::removalask::Holds,
) -> Shown {
    let count = |many: usize| u32::try_from(many).unwrap_or(u32::MAX);
    Shown {
        session: waits.session,
        ask: removal_key(waits.session, &waits.workspace),
        // What a removal deletes is said wherever the question is: the Inbox draws this line
        // beside Remove, with no Notice around it.
        says: said_inertly(&format!(
            "Removing workspace {} deletes its memory, its todos and its clones",
            waits.workspace
        )),
        options: vec![
            Offered {
                id: REMOVE.to_owned(),
                label: REMOVE_SAID.to_owned(),
                allows: true,
                preselected: None,
            },
            refuses(KEEP, KEEP_IT_SAID),
        ],
        source: AskSource::WorkspaceRemoval,
        chain: Vec::new(),
        answer: AnswerPath::WorkspaceRemoval {
            workspace: waits.workspace.clone(),
            memories: count(holds.memories),
            todos: count(holds.todos),
        },
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    }
}

/// The key of chat `session`'s browser Ask (#2013): one per chat.
fn browser_key(session: u32) -> String {
    format!("browser:{session}")
}

/// **The browser adapter** (#2013): a chat's first browser call, which asks the person what its
/// browser may reach. Every word is purlis's. **This project's sites** is the hosts the chat's
/// sandbox lists and the services on this machine the person allowed (for a chat started
/// without the sandbox, those a sandboxed chat of its persona lists, and the line says so,
/// #2242); **Also the open web**
/// adds every public site; **Keep off** refuses its every browser call.
///
/// **Always for this persona** (#2241) is offered beside each, for a chat that runs as a
/// persona, unless an administrator's policy leaves out `you`, the scope it is kept at
/// (`locks`). **The answers never move**: This project's sites, Also the open web, each Always
/// after them, Keep off last, whatever the persona declares, so a key always presses the same
/// answer. **A persona's declared browser** marks its for-this-chat answer only, by its label
/// ("<persona> uses this") and as `preselected`, its Always beside it unmarked, and the line says
/// what the persona declares: the person still answers (ADR 0073).
pub fn browser_posture(
    waits: &crate::chatbrowsers::Waits,
    locks: &purlis_core::sandbox::policy::Locks,
) -> Shown {
    use purlis_core::sandbox::persona::Browser;
    let persona = waits.as_persona.persona.as_deref();
    let always_for = persona.filter(|_| {
        locks
            .scope_removed(purlis_core::sandbox::grant::Level::You)
            .is_none()
    });
    let declared = waits.as_persona.declared;
    // A chat started without the sandbox (#2242): its browser is held to what a sandboxed chat
    // of its persona lists, though the chat itself is not.
    let mut says = if waits.unsandboxed {
        format!(
            "Wants a browser. This chat has no sandbox, but its browser does: this project's \
             sites are the hosts {} lists and the services on this machine you allowed",
            persona.map_or_else(
                || "a sandboxed chat on no persona".to_owned(),
                |p| format!("a sandboxed {p} chat")
            )
        )
    } else {
        "Wants a browser: what may it reach? This project's sites are the hosts its sandbox \
         lists and the services on this machine you allowed"
            .to_owned()
    };
    if let (Some(persona), Some(declared)) = (persona, declared) {
        says.push_str(&format!(
            ". Its persona {persona} declares {}",
            match declared {
                Browser::Project => "this project's sites",
                Browser::OpenWeb => "the open web too",
            }
        ));
    }
    let offer = |id: &str, label: String| Offered {
        id: id.to_owned(),
        label,
        allows: true,
        preselected: None,
    };
    // The declared reach's answer for this chat alone is marked, in place.
    let marked = |reach: Browser, said: &str| match (persona, declared) {
        (Some(persona), Some(declared)) if declared == reach => Offered {
            preselected: Some(true),
            ..offer(
                match reach {
                    Browser::Project => BROWSER_PROJECT,
                    Browser::OpenWeb => BROWSER_OPEN_WEB,
                },
                format!("{said} ({persona} uses this)"),
            )
        },
        _ => offer(
            match reach {
                Browser::Project => BROWSER_PROJECT,
                Browser::OpenWeb => BROWSER_OPEN_WEB,
            },
            said.to_owned(),
        ),
    };
    let mut options = vec![
        marked(Browser::Project, "This project's sites"),
        marked(Browser::OpenWeb, "Also the open web"),
    ];
    if let Some(persona) = always_for {
        options.push(offer(
            BROWSER_PROJECT_ALWAYS,
            format!("This project's sites, always for {persona}"),
        ));
        options.push(offer(
            BROWSER_OPEN_WEB_ALWAYS,
            format!("Also the open web, always for {persona}"),
        ));
    }
    options.push(refuses(KEEP, "Keep off"));
    Shown {
        session: waits.session,
        ask: browser_key(waits.session),
        says: said_inertly(&says),
        options,
        source: AskSource::Browser,
        chain: Vec::new(),
        answer: AnswerPath::Browser,
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    }
}

/// The key of the Ask about browser `program` (#2015): one per browser.
fn needs_you_key(program: &str) -> String {
    format!("browser-needs-you:{program}")
}

/// The id of Take over on a browser's Ask (#2015).
pub const TAKE_OVER: &str = "take-over";

/// The id of Open its view on a browser's Ask the person has taken over (#2015): it sends
/// nothing, and opens the view again.
pub const OPEN_VIEW: &str = "open-view";

/// **The browser-needs-you adapter** (#2015): a chat asked the person to take its browser over.
/// The words are the agent's, said as the chat's after purlis's own; the one answer is Take
/// over, and Done in the browser's view ends it. Once the person drives it, the row says so,
/// and its one answer opens the view again.
pub fn browser_needs_you(waiting: &crate::browserneedsyou::Waiting) -> Shown {
    // Once the person drives it, the row says so: Done in its view hands it back, and the one
    // answer opens that view again, sending nothing.
    let (says, offered) = if waiting.taken {
        (
            format!(
                "You are driving its browser {}: Done in its view hands it back. The chat said: \
                 {}",
                waiting.browser, waiting.question
            ),
            Offered {
                id: OPEN_VIEW.to_owned(),
                label: "Open its view".to_owned(),
                allows: false,
                preselected: None,
            },
        )
    } else {
        (
            format!(
                "Needs you in its browser {}. The chat says: {}",
                waiting.browser, waiting.question
            ),
            Offered {
                id: TAKE_OVER.to_owned(),
                label: "Take over".to_owned(),
                allows: true,
                preselected: None,
            },
        )
    };
    Shown {
        session: waiting.session,
        ask: needs_you_key(&waiting.program),
        says: said_inertly(&says),
        options: vec![offered],
        source: AskSource::BrowserNeedsYou,
        chain: Vec::new(),
        answer: AnswerPath::TakeOver {
            program: waiting.program.clone(),
        },
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    }
}

/// The key of chat `session`'s pool login Ask (#2050): one per chat.
fn pool_login_key(session: u32) -> String {
    format!("pool-login:{session}")
}

/// The id of Allow on a pool login Ask (#2050).
pub const POOL_LOGIN_ALLOW: &str = "allow";

/// **The pool login adapter** (#2050, spec #2041 stories 22 and 23): a runner chat asked purlis
/// to fill one of its persona's browser logins in its browser, which runs on the pool's runner.
/// It names the login, the pool and the engine the pool runs on. Allow on pool X lets that login
/// of that persona be filled there while the app runs and the pool and its runner stay as they
/// are now; Keep off refuses it there until they change. Every word is purlis's, the line says
/// the runner keeps none of the values, and a changed pool's Ask says it changed.
pub fn pool_login(waits: &crate::chatbrowsers::PoolLoginWaits) -> Shown {
    let pool = &waits.bound.pool;
    let (login, persona, engine) = (&waits.login, &waits.persona, &waits.bound.engine);
    // A cluster's words come last, so a long context cuts nothing of what Allow does.
    let says = if waits.bound.on_a_cluster() {
        format!(
            "{}Wants its browser logged in with {login} on pool {pool}. Allow on pool {pool} \
             fills {persona}'s {login} there from your vault; the runner is told to keep none. \
             It runs on {engine}",
            if waits.changed {
                "Pool {pool} changed since you answered. ".replace("{pool}", pool)
            } else {
                String::new()
            }
        )
    } else if waits.changed {
        format!(
            "Pool {pool} changed since you answered. Wants its browser logged in with {login} \
             there, on {engine}. Allow on pool {pool} fills it from your vault; the runner keeps \
             none"
        )
    } else {
        format!(
            "Wants its browser logged in with {login} on pool {pool}, which runs on {engine}. \
             Allow on pool {pool} fills {persona}'s {login} there from your vault; the runner \
             keeps none"
        )
    };
    Shown {
        session: waits.session,
        ask: pool_login_key(waits.session),
        says: said_inertly(&says),
        options: vec![
            Offered {
                id: POOL_LOGIN_ALLOW.to_owned(),
                label: format!("Allow on pool {pool}"),
                allows: true,
                preselected: None,
            },
            refuses(KEEP, "Keep off"),
        ],
        source: AskSource::PoolLogin,
        chain: Vec::new(),
        answer: AnswerPath::PoolLogin,
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    }
}

/// The key of run `run` of chat `session`'s tool on its pool (#2047): one Ask per run.
fn pool_tool_key(session: u32, run: u32) -> String {
    format!("pool-tool:{session}:{run}")
}

/// **The pool tool adapter** (#2047, spec #2041 story 21; the person's ruling of 2026-10-26:
/// every run asks): a runner chat ran one of its persona's declared tools, which needs its vault
/// values on the pool's runner, and the command waits for the answer. It names the tool, the
/// vault, the pool and the engine the pool runs on. Allow this run sends the tool its values for
/// that run alone; Keep off refuses that run. Every word is purlis's, and the line says the runner
/// keeps none.
pub fn pool_tool(waits: &crate::pooltools::PoolToolWaits) -> Shown {
    let pool = &waits.bound.pool;
    let (tool, persona, vault, engine) = (
        &waits.tool,
        &waits.persona,
        &waits.vault,
        &waits.bound.engine,
    );
    let away = if waits.waited {
        "Asked while you were away. "
    } else {
        ""
    };
    // A cluster's words come last, so a long context cuts nothing of what Allow does.
    let says = if waits.bound.on_a_cluster() {
        format!(
            "{away}Wants to run {tool} on pool {pool} with {persona}'s values from vault {vault}. \
             Allow this run sends them from your vault for this run alone; the runner is told \
             to keep none. It runs on {engine}"
        )
    } else {
        format!(
            "{away}Wants to run {tool} on pool {pool}, which runs on {engine}, with {persona}'s \
             values from vault {vault}. Allow this run sends them from your vault for this run \
             alone; the runner keeps none"
        )
    };
    Shown {
        session: waits.session,
        ask: pool_tool_key(waits.session, waits.run),
        says: said_inertly(&says),
        options: vec![
            Offered {
                id: POOL_TOOL_ALLOW.to_owned(),
                label: "Allow this run".to_owned(),
                allows: true,
                preselected: None,
            },
            refuses(KEEP, "Keep off"),
        ],
        source: AskSource::PoolTool,
        chain: Vec::new(),
        answer: AnswerPath::PoolTool { run: waits.run },
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    }
}

/// The id of Allow on a pool tool Ask (#2047).
pub const POOL_TOOL_ALLOW: &str = "allow";

/// The key of chat `session`'s browser sites Ask (#2018): one per chat, every site it waits on.
fn browser_sites_key(session: u32) -> String {
    format!("browser-sites:{session}")
}

/// **The browser sites adapter** (#2018): one ask per chat, naming the sites new to the project
/// its browser held back what it would open, send or type on, oldest first, each whole. Allow
/// for this chat; Allow always here, on this machine; Allow for everyone in this project
/// (#2443) where `everyone`, that is where the project's sandbox is in force, the one place a
/// committed browser site reaches a browser; each unless an administrator's policy leaves its
/// scope out; Keep blocked. Every word
/// is purlis's, and the line says an Allow is the browser's alone (the chat's own commands still
/// need their own Allow for a host) and what it does (#2364): the browser reaches the site from
/// its next act, and nothing restarts.
///
/// **It answers only what its line names whole** ([`sites_named`], #2364): the rest wait for the
/// next ask. A site too long to be named whole is offered Keep blocked alone, at every scope.
pub fn browser_sites(
    waits: &crate::chatbrowsers::SitesWait,
    locks: &purlis_core::sandbox::policy::Locks,
    everyone: bool,
) -> Shown {
    let all: Vec<String> = waits.sites.iter().map(ToString::to_string).collect();
    let (sites, says, options) = browser_site_answers(&all, locks, true, everyone);
    Shown {
        session: waits.session,
        ask: browser_sites_key(waits.session),
        says: said_inertly(&says),
        options,
        source: AskSource::BrowserSite,
        chain: Vec::new(),
        answer: AnswerPath::BrowserSites { sites },
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    }
}

/// **What a browser's sites ask names and offers** (#2018, #2176): the sites of `all` it names
/// whole ([`sites_named`]), its line, and its answers. Allow for this chat where it is a chat's
/// (`for_a_chat`); Always allow in this project, on this machine; Allow for everyone in this
/// project where `everyone` (#2443); each unless an administrator's policy leaves its scope out;
/// Keep blocked last. A site too long to be named whole is offered Keep blocked alone.
pub fn browser_site_answers(
    all: &[String],
    locks: &purlis_core::sandbox::policy::Locks,
    for_a_chat: bool,
    everyone: bool,
) -> (Vec<String>, String, Vec<Offered>) {
    use purlis_core::sandbox::grant::Level;
    let mut options = Vec::new();
    let allow = |level: Level, label: &str| Offered {
        id: level.word().to_owned(),
        label: label.to_owned(),
        allows: true,
        preselected: None,
    };
    let (sites, says) = match sites_named(all) {
        Some(named) => {
            if for_a_chat && locks.scope_removed(Level::Chat).is_none() {
                options.push(allow(Level::Chat, "Allow for this chat"));
            }
            if locks.scope_removed(Level::You).is_none() {
                options.push(allow(
                    Level::You,
                    "Always allow in this project, on this machine",
                ));
            }
            // Committed for everyone in the project (#2443): still the browsers' alone.
            if everyone && locks.scope_removed(Level::Project).is_none() {
                options.push(allow(Level::Project, "Allow for everyone in this project"));
            }
            named
        }
        // Not even the oldest fits whole: nothing is allowed that the person cannot read.
        None => {
            let oldest: Vec<String> = all.iter().take(1).cloned().collect();
            let says = sites_said(&oldest, all.len() - oldest.len());
            (oldest, says)
        }
    };
    options.push(refuses(KEEP, KEEP_SAID));
    (sites, says, options)
}

/// **The sites of `all`, oldest first, that a browser sites ask names whole** (#2364), and its
/// line: as many as fit, with how many more wait after them; where the oldest alone fits only
/// without that count, the oldest alone. `None` where not even the oldest is named whole: an
/// Allow of it is never offered, and never taken ([`crate::chatbrowsers::sites_allowed`]).
pub fn sites_named(all: &[String]) -> Option<(Vec<String>, String)> {
    let whole = |says: String| (said_inertly(&says) == says).then_some(says);
    (1..=all.len())
        .rev()
        .find_map(|count| {
            whole(sites_said(&all[..count], all.len() - count)).map(|says| (count, says))
        })
        .or_else(|| whole(sites_said(&all[..all.len().min(1)], 0)).map(|says| (1, says)))
        .filter(|(count, _)| *count <= all.len())
        .map(|(count, says)| (all[..count].to_vec(), says))
}

/// The line of a browser sites ask that names `sites`, with `more` waiting after them.
fn sites_said(sites: &[String], more: usize) -> String {
    let named = match sites {
        [] => String::new(),
        [one] => one.clone(),
        [first @ .., last] => format!("{} and {last}", first.join(", ")),
    };
    let after = if more == 0 {
        String::new()
    } else {
        format!(", and {more} more")
    };
    format!(
        "Its browser wants to reach {named}{after}. An Allow is its browser's alone, from its \
         next act; nothing restarts."
    )
}

/// The id of This project's sites, always for the chat's persona (#2241).
pub const BROWSER_PROJECT_ALWAYS: &str = "project-always";
/// The id of Also the open web, always for the chat's persona (#2241).
pub const BROWSER_OPEN_WEB_ALWAYS: &str = "open-web-always";

/// The id of This project's sites on a chat's browser Ask (#2013).
pub const BROWSER_PROJECT: &str = "project";
/// The id of Also the open web on a chat's browser Ask (#2013).
pub const BROWSER_OPEN_WEB: &str = "open-web";

/// The id of Apply on a proposed change (#1975).
pub const APPLY: &str = "apply";
/// The id of Dismiss on a proposed change.
pub const DISMISS: &str = "dismiss";
/// The id of Open on a proposed change too long for a row (PC-7): it opens the change's own
/// tab, and answers nothing.
pub const OPEN: &str = "open";
/// The id of Refuse on a runner chat's held open or dispatch.
pub const REFUSE: &str = "refuse";

/// The key of chat `session`'s proposal `id`.
fn proposal_key(id: u32) -> String {
    format!("proposal:{id}")
}

/// **The proposed change adapter** (#1975): a change a chat proposed to purlis's own
/// configuration, held now, with the file as the app reads it and as the change makes it.
/// **Apply** is offered only where the change is shown whole and still applies (PC-7, PC-8);
/// **Dismiss** always. Never "always apply" (PC-13).
pub fn proposed_change(one: &crate::proposals::Proposal, now: &crate::proposals::Now) -> Shown {
    use crate::proposals::Now;
    let path = &one.path;
    let mut opens = false;
    let (says, writes, applies) = match now {
        Now::Made { before, after } => {
            let writes = crate::askwrites::proposed(path, before, after);
            let whole = matches!(writes, crate::askwrites::WritesShown::Whole { .. });
            opens = crate::askwrites::proposed_too_long(before, after);
            (
                format!("Proposes a change to {path}, which only you apply"),
                Some(writes),
                whole,
            )
        }
        Now::Stale => (
            format!(
                "Proposed a change to {path}, which changed since: it can no longer be applied"
            ),
            None,
            false,
        ),
        Now::Unprepared => (
            format!("Proposed a change to {path}, which can no longer be made"),
            None,
            false,
        ),
    };
    let mut options = Vec::new();
    if applies {
        options.push(Offered {
            id: APPLY.to_owned(),
            label: "Apply".to_owned(),
            allows: true,
            preselected: None,
        });
    }
    if opens {
        options.push(refuses(OPEN, "Open"));
    }
    options.push(refuses(DISMISS, "Dismiss"));
    Shown {
        session: one.session,
        ask: proposal_key(one.id),
        says: said_inertly(&says),
        options,
        source: AskSource::ProposedChange,
        chain: Vec::new(),
        answer: AnswerPath::ProposedChange {
            id: one.id,
            path: one.path.clone(),
        },
        since: None,
        held_until: None,
        writes,
        lines: None,
    }
}

/// The key of held runner dispatch `id`.
fn runner_dispatch_key(id: u32) -> String {
    format!("runner-dispatch:{id}")
}

/// **The runner dispatch adapter**: an open or a dispatch a chat on a pool's runner asked for,
/// said in words, held until the person answers. Its row cuts the brief, so **Open** shows it
/// whole in its own tab, where Approve is pressed; **Refuse** starts nothing. Never "always
/// approve".
pub fn runner_dispatch(one: &crate::runnerdispatches::HeldDispatch) -> Shown {
    Shown {
        session: one.session,
        ask: runner_dispatch_key(one.id),
        says: said_inertly(&one.said()),
        options: vec![refuses(OPEN, "Open"), refuses(REFUSE, "Refuse")],
        source: AskSource::RunnerDispatch,
        chain: Vec::new(),
        answer: AnswerPath::RunnerDispatch { id: one.id },
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    }
}

/// The key of held runner write `id`.
fn runner_write_key(id: u32) -> String {
    format!("runner-write:{id}")
}

/// What a held runner write's row says.
fn runner_write_says(one: &crate::runnerwrites::HeldWrite) -> String {
    format!(
        "Asks, from pool {}, to change this project: {}",
        one.pool, one.said
    )
}

/// **Whether held runner write `one`'s row shows it whole** (#1146's rule): its line, as the
/// row draws it, is the line said, with no text cut, collapsed, masked or escaped; so what the
/// person reads there is what Apply writes.
pub fn runner_write_shown_whole(one: &crate::runnerwrites::HeldWrite) -> bool {
    let says = runner_write_says(one);
    !purlis_core::runnerwrite::cut(&one.write) && said_inertly(&says) == says
}

/// **The runner write adapter** (#2046): a change to the project a chat on a pool's runner
/// asked for, said in words, held until the person answers. **Apply** is offered only where the
/// row shows it whole ([`runner_write_shown_whole`]); otherwise **Open** shows it whole in its
/// own tab, where Apply is pressed. **Dismiss** makes nothing. Never "always apply".
pub fn runner_write(one: &crate::runnerwrites::HeldWrite) -> Shown {
    let mut options = Vec::new();
    if runner_write_shown_whole(one) {
        options.push(Offered {
            id: APPLY.to_owned(),
            label: "Apply".to_owned(),
            allows: true,
            preselected: None,
        });
    } else {
        options.push(refuses(OPEN, "Open"));
    }
    options.push(refuses(DISMISS, "Dismiss"));
    Shown {
        session: one.session,
        ask: runner_write_key(one.id),
        says: said_inertly(&runner_write_says(one)),
        options,
        source: AskSource::RunnerWrite,
        chain: Vec::new(),
        answer: AnswerPath::RunnerWrite { id: one.id },
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    }
}

/// The option id of Sign in on a profile's Ask (#2035), and its prefix where a provider is
/// named: `sign-in:<provider>`.
pub const SIGN_IN: &str = "sign-in";

/// **The sign-in adapter** (#2035, ADR 0095 §3): a profile a start was refused on because its
/// harness says it is not signed in here, as one Ask per profile, about no chat: its chain is
/// the profile. It offers Sign in, once per plan where purlis names the provider (opencode's
/// ChatGPT and GitHub Copilot, never a Claude plan), and Dismiss.
pub fn sign_in(wait: &purlis_core::profilestore::sign_in::Wait) -> Shown {
    use purlis_core::profilestore::sign_in;
    let name = &wait.profile.name;
    let plans = sign_in::providers(&wait.profile.kind);
    let mut options: Vec<Offered> = if plans.is_empty() {
        vec![Offered {
            id: SIGN_IN.to_owned(),
            label: "Sign in".to_owned(),
            allows: true,
            preselected: None,
        }]
    } else {
        plans
            .iter()
            .map(|plan| Offered {
                id: format!("{SIGN_IN}:{}", plan.provider),
                label: format!("Sign in to {}", plan.title),
                allows: true,
                preselected: None,
            })
            .collect()
    };
    options.push(refuses(DISMISS, "Dismiss"));
    Shown {
        session: 0,
        ask: format!("sign-in:{name}"),
        says: said_inertly(&format!(
            "{name} is not signed in on {}: a chat on it does not start until it is. Sign in \
             runs {}'s own sign-in in a shell tab",
            purlis_core::profilestore::this_device(),
            wait.harness
        )),
        options,
        source: AskSource::SignIn,
        chain: vec![said_inertly(&format!("Profile {name}"))],
        answer: AnswerPath::SignIn {
            profile: name.clone(),
            pool: None,
        },
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    }
}

/// The key a profile's Ask to sign it in on pool `pool` is kept under (#2049).
fn pool_sign_in_key(profile: &str, pool: &str) -> String {
    format!("sign-in:{profile}:on:{pool}")
}

/// **A profile a start on a pool was refused on, as its Ask** (#2049): *Profile <profile> on
/// pool <pool>*, about no chat. Its Sign in on pool <pool> opens a shell tab where the core types the runner's own
/// sign-in for it, which the person drives there; the Ask waits until the runner says it is
/// signed in there. Nothing of this machine's sign-in goes to the runner.
pub fn pool_sign_in(wait: &purlis_core::profilestore::onrunner::PoolWait) -> Shown {
    use purlis_core::profilestore::sign_in;
    let (name, pool) = (&wait.profile, &wait.pool);
    let plans = sign_in::providers(&wait.kind);
    let mut options: Vec<Offered> = if plans.is_empty() {
        vec![Offered {
            id: SIGN_IN.to_owned(),
            label: format!("Sign in on pool {pool}"),
            allows: true,
            preselected: None,
        }]
    } else {
        plans
            .iter()
            .map(|plan| Offered {
                id: format!("{SIGN_IN}:{}", plan.provider),
                label: format!("Sign in to {} on pool {pool}", plan.title),
                allows: true,
                preselected: None,
            })
            .collect()
    };
    options.push(refuses(DISMISS, "Dismiss"));
    Shown {
        session: 0,
        ask: pool_sign_in_key(name, pool),
        says: said_inertly(&format!(
            "{name} is not signed in on pool {pool}. Sign in runs {}'s own sign-in on its \
             runner; nothing of this machine's sign-in goes there",
            wait.harness
        )),
        options,
        source: AskSource::SignIn,
        chain: vec![said_inertly(&format!("Profile {name} on pool {pool}"))],
        answer: AnswerPath::SignIn {
            profile: name.clone(),
            pool: Some(pool.clone()),
        },
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    }
}

/// **A start in the project `held` was refused, for `why`** (#2037, #2035): a refusal may have
/// left a wait the window lists as an Ask (*Codex needs a sandbox-only profile here*, *Sign in
/// …*), so the window is told its asks may have moved and reads them again at once. `why`, as
/// it was.
pub fn start_refused(held: &crate::planes::Held, why: String) -> String {
    held.chats().asks_may_have_moved();
    why
}

/// Every project `planes` holds is told its asks may have moved: a sandbox-only Codex profile
/// was made outside its Ask (#2037), which clears any project's wait for one.
pub fn every_projects_asks_may_have_moved(planes: &crate::planes::Planes) {
    for plane in planes.open_now() {
        if let Ok(held) = planes.held(&plane) {
            held.chats().asks_may_have_moved();
        }
    }
}

/// The option id of Make one on the Ask for a sandbox-only Codex profile (#2037).
pub const MAKE_SANDBOX_ONLY: &str = "make";

/// **The sandbox-only adapter** (#2037, the operator's ruling of 2026-10-15): a project where a
/// sandboxed Codex chat was refused for want of a sandbox-only profile, as one Ask about no
/// chat. It offers Make one, which makes the profile and opens Codex's own sign-in in it, and
/// Dismiss.
pub fn sandbox_only(_wait: &purlis_core::profilestore::sandbox_only::Wait) -> Shown {
    use purlis_core::profilestore::sandbox_only::ASK;
    Shown {
        session: 0,
        ask: "sandbox-only:codex".to_owned(),
        says: said_inertly(&format!(
            "{ASK}: a sandboxed Codex chat runs only on a profile for sandboxed chats only, so \
             your own Codex is never written by one. Make one makes it and runs Codex's own \
             sign-in in a shell tab"
        )),
        options: vec![
            Offered {
                id: MAKE_SANDBOX_ONLY.to_owned(),
                label: "Make one".to_owned(),
                allows: true,
                preselected: None,
            },
            refuses(DISMISS, "Dismiss"),
        ],
        source: AskSource::SandboxOnly,
        chain: vec![said_inertly("Codex")],
        answer: AnswerPath::SandboxOnly,
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    }
}

/// The option id of Remove on the Ask about the old `[harness_plugins]` keys (#3467).
pub const REMOVE_OLD_PLUGINS: &str = "remove";

/// **The old harness plugin keys' adapter** (#3467): a project that still holds
/// `[harness_plugins]`, which nothing reads now, as one Ask about no chat, said once. It offers
/// Remove where a file holding the keys can be made without them (the project's own two, or a
/// workspace's manifest, #3486), which proposes the edit for the person to apply, and Dismiss.
pub fn old_harness_plugins(old: &crate::oldplugins::OldHarnessPlugins) -> Shown {
    let mut options = Vec::new();
    if old.removable {
        options.push(refuses(REMOVE_OLD_PLUGINS, "Remove"));
    }
    options.push(refuses(DISMISS, "Dismiss"));
    Shown {
        session: 0,
        ask: "old-harness-plugins".to_owned(),
        says: said_inertly(&old.said),
        options,
        source: AskSource::OldHarnessPlugins,
        chain: vec![said_inertly("purlis")],
        answer: AnswerPath::OldHarnessPlugins,
        since: None,
        held_until: None,
        writes: None,
        lines: None,
    }
}

/// How far up who-asked-whom a chain is followed: far past any depth a dispatch is allowed.
const MOST_CHAIN: usize = 64;

/// **Who is asking, as a chain of names** (I-9): the session first, chat `session` last, by
/// `asker_of`, the app's own record of who asked whom, and `name_of`. A link that loops ends it.
pub fn chain(
    session: u32,
    name_of: &dyn Fn(u32) -> Option<String>,
    asker_of: &dyn Fn(u32) -> Option<u32>,
) -> Vec<String> {
    let named = |chat: u32| {
        name_of(chat).map_or_else(
            || format!("chat {chat}"),
            |name| purlis_core::dispatchgrant::inert(&name),
        )
    };
    let mut names = vec![named(session)];
    let mut seen = vec![session];
    let mut at = session;
    for _ in 0..MOST_CHAIN {
        match asker_of(at) {
            Some(asker) if !seen.contains(&asker) => {
                seen.push(asker);
                names.push(named(asker));
                at = asker;
            }
            _ => break,
        }
    }
    names.reverse();
    names
}

/// Until when chat `session`'s proxy holds a connection to `host`, given when its ask began
/// (seconds since 1970), where it holds one now (#1709).
pub type HoldEnds<'a> = dyn Fn(u32, &str, Option<u32>) -> Option<u32> + 'a;

/// **What waits in one project, read from each source as it stands** (#1690): the inputs of
/// [`derive`], so the registry's rules are tested without a running project.
pub struct Waiting<'a> {
    /// The permission asks its chats' hooks hold open (HP-6).
    pub permissions: Vec<Raised>,
    /// The needs-you queue: the chats the board says need the person.
    pub queue: Vec<u32>,
    /// The prompt a chat is stopped on in its terminal, asked mid-turn, where it is
    /// (`Glance::prompt`, #1691).
    pub at_its_prompt: &'a dyn Fn(u32) -> Option<purlis_core::harness::model::Prompt>,
    /// The dispatches held for the person, as their Notices are told them.
    pub dispatches: Vec<crate::dispatchgrants::DispatchPending>,
    /// The blocks each chat is held on now.
    pub blocks: Vec<(u32, crate::taskblocks::HeldBlock)>,
    /// The workspaces a chat asked the person to remove, with what each holds (qw356).
    pub removals: Vec<(crate::removalasks::Waits, purlis_core::removalask::Holds)>,
    /// The changes a chat proposed to purlis's own configuration, each as it is now (#1975).
    pub proposals: Vec<(crate::proposals::Proposal, crate::proposals::Now)>,
    /// The changes to the project chats on a pool's runner asked for (#2046).
    pub runner_writes: Vec<crate::runnerwrites::HeldWrite>,
    /// The opens and dispatches chats on a pool's runner asked for.
    pub runner_dispatches: Vec<crate::runnerdispatches::HeldDispatch>,
    /// The profiles a start was refused on because they are not signed in here (#2035).
    pub sign_ins: Vec<purlis_core::profilestore::sign_in::Wait>,
    /// The profiles a start on a pool was refused on because they are not signed in on its
    /// runner (#2049).
    pub pool_sign_ins: Vec<purlis_core::profilestore::onrunner::PoolWait>,
    /// Whether a sandboxed Codex chat here waits for a sandbox-only profile (#2037).
    pub sandbox_only: Option<purlis_core::profilestore::sandbox_only::Wait>,
    /// The chats whose first browser call asks what their browser may reach (#2013).
    pub browsers: Vec<crate::chatbrowsers::Waits>,
    /// The browsers whose chat asked the person to take them over (#2015).
    pub needs_you: Vec<crate::browserneedsyou::Waiting>,
    /// The chats whose browser's sites new to the project wait on the person (#2018).
    pub browser_sites: Vec<crate::chatbrowsers::SitesWait>,
    /// The runner chats whose browser login waits on the person's opt-in for its pool (#2050).
    pub pool_logins: Vec<crate::chatbrowsers::PoolLoginWaits>,
    /// The runs of runner chats' declared tools that wait on the person (#2047).
    pub pool_tools: Vec<crate::pooltools::PoolToolWaits>,
    /// The administrator's policy, which the levels an Allow is offered at answer to.
    pub locks: &'a purlis_core::sandbox::policy::Locks,
    /// Whether a browser site for everyone in the project reaches a browser here: only where the
    /// project's sandbox is in force (#2443). A browser sites ask offers it only then.
    pub everyone: bool,
    /// What the project's committed file declares it expects (ADR 0093): a socket's or a
    /// service's ask says so. It grants nothing.
    pub declared: &'a purlis_core::sandbox::services::Declared,
    /// The name a chat is shown under, where it is open.
    pub name_of: &'a dyn Fn(u32) -> Option<String>,
    /// The chat that asked for a chat as a task, by the app's own record.
    pub asker_of: &'a dyn Fn(u32) -> Option<u32>,
    /// Whether a chat is in the queue only for what is an update ([`only_updates`]): each is
    /// an update in the Inbox (#1693, #1694), and nothing of it waits on the person.
    pub only_updates: &'a dyn Fn(u32) -> bool,
    /// Until when a chat's proxy holds a connection to a host, where it holds one now, asking
    /// the person (#1709), given when the ask began.
    pub held: &'a HoldEnds<'a>,
    /// When an ask began to wait, by its key, where its source knows (#1700).
    pub since: &'a dyn Fn(&str) -> Option<u32>,
    /// The name of the runner a chat runs on, where its runner left a service out of its plan
    /// (#2510).
    pub runner_of: &'a dyn Fn(u32) -> Option<String>,
}

/// **The asks of one project, one per thing that waits** (#1690): each source's adapter, then
/// each ask's chain. A chat in the needs-you queue is no ask of its own where a structured ask
/// of its says why it waits: its held permission prompt is that prompt, and a held dispatch is
/// what its turn waits on. A chat there only for what is an update asks nothing (#1693, #1694):
/// a task that came to nothing, a report with nowhere to go, a commit refused are each an update
/// of the Inbox's. The rest of the queue is a prompt in its terminal or a question.
pub fn derive(waiting: &Waiting<'_>) -> Vec<Shown> {
    let mut asks: Vec<Shown> = waiting
        .permissions
        .iter()
        .cloned()
        .filter_map(permission)
        .collect();
    asks.extend(waiting.dispatches.iter().map(dispatch));
    // Before the queue is read: a chat that asked the person to remove a workspace waits on
    // that answer, so its wait is that ask and no question of its own beside it (qw356).
    asks.extend(
        waiting
            .removals
            .iter()
            .map(|(waits, holds)| workspace_removal(waits, *holds)),
    );
    // A chat that proposed a change waits on the person's answer, as one that asked them to
    // remove a workspace does (#1975).
    asks.extend(
        waiting
            .proposals
            .iter()
            .map(|(one, now)| proposed_change(one, now)),
    );
    // And one on a runner that asked to change the project waits on the person too (#2046).
    asks.extend(waiting.runner_writes.iter().map(runner_write));
    // And one there that asked to start a chat here.
    asks.extend(waiting.runner_dispatches.iter().map(runner_dispatch));
    // A chat whose first browser call asks the person waits on that answer too (#2013).
    asks.extend(
        waiting
            .browsers
            .iter()
            .map(|one| browser_posture(one, waiting.locks)),
    );
    // And one whose browser needs the person waits on them to take it over (#2015).
    asks.extend(waiting.needs_you.iter().map(browser_needs_you));
    // A chat whose browser's sites new to the project wait on the person (#2018).
    asks.extend(
        waiting
            .browser_sites
            .iter()
            .map(|one| browser_sites(one, waiting.locks, waiting.everyone)),
    );
    // A runner chat whose browser login waits on the person's opt-in for its pool (#2050).
    asks.extend(waiting.pool_logins.iter().map(pool_login));
    // A runner chat whose declared tool waits on the person's Allow for its pool (#2047).
    asks.extend(waiting.pool_tools.iter().map(pool_tool));
    let said: std::collections::HashSet<u32> = asks.iter().map(|ask| ask.session).collect();
    // About no chat: a profile that waits to be signed in (#2035).
    let signing = asks.len();
    asks.extend(waiting.sign_ins.iter().map(sign_in));
    asks.extend(waiting.pool_sign_ins.iter().map(pool_sign_in));
    asks.extend(waiting.sandbox_only.iter().map(sandbox_only));
    asks.extend(
        waiting
            .queue
            .iter()
            .filter(|session| !said.contains(session) && !(waiting.only_updates)(**session))
            .map(|&session| match (waiting.at_its_prompt)(session) {
                Some(prompt) => terminal(session, prompt),
                None => question(session),
            }),
    );
    asks.extend(waiting.blocks.iter().filter_map(|(session, block)| {
        // A runner's service, named with its runner (#2510).
        let mut ask = sandbox_runner(*session, block, (waiting.runner_of)(*session).as_deref())
            .or_else(|| sandbox_block(*session, block, waiting.locks, waiting.declared))?;
        let since = (waiting.since)(&ask.ask);
        // A live ask (#1709): the same ask, under the same key, saying its connection waits,
        // and until when.
        if block.what == crate::sandboxing::GrantWhat::Host
            && !block.target.is_empty()
            && let Some(until) = (waiting.held)(*session, &block.target, since)
        {
            ask.says = said_inertly(&format!(
                "A connection to {} waits on your answer",
                block.target
            ));
            ask.held_until = Some(until);
        }
        Some(ask)
    }));
    for (at, ask) in asks.iter_mut().enumerate() {
        // A profile's Ask is about no chat: its chain is the profile, as its adapter said.
        let profiles = signing
            ..signing
                + waiting.sign_ins.len()
                + waiting.pool_sign_ins.len()
                + usize::from(waiting.sandbox_only.is_some());
        if !profiles.contains(&at) {
            ask.chain = chain(ask.session, waiting.name_of, waiting.asker_of);
        }
        ask.since = (waiting.since)(&ask.ask);
    }
    asks
}

/// What one chat in the needs-you queue is there for, as [`only_updates`] reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InQueue {
    /// A task it asked for came to nothing, and the person has not looked (#1491).
    pub failed: bool,
    /// The board says it needs the person for a reason besides those failures.
    pub for_itself: bool,
    /// The app's reasons for it ([`reasons_besides_failures`]): a report with nowhere to go,
    /// a commit refused.
    pub reasons: usize,
    /// Its turn ended on the person: its state is waiting.
    pub waiting: bool,
    /// It is stopped on a prompt in its terminal.
    pub at_its_prompt: bool,
}

/// **The registry's own split: a chat in the queue for updates alone asks nothing** (#1693,
/// #1694, I-1). A task of its that came to nothing, a report with nowhere to go and a commit
/// refused are each an update of the Inbox's, so the registry does not list the chat for them,
/// and no notification is sent (`asknotify` reads the registry alone). A chat whose own turn
/// ended on the person, or that stopped on a prompt in its terminal, still asks, whatever else
/// it is in the queue for, so an update never silences what the chat itself waits on.
pub fn only_updates(chat: InQueue) -> bool {
    (chat.failed && !chat.for_itself) || (chat.reasons > 0 && !chat.waiting && !chat.at_its_prompt)
}

/// How soon the window looks again at a hold already past its minute (#1709).
pub const LOOK_AGAIN: u64 = 5;

/// **When a live ask's hold ends** (#1709): the proxy holds a connection
/// [`purlis_core::sandbox::asks::HOLD`] from when it asked, which is when its block was first
/// heard (`since`). A block heard before its connection was held is still held past that, so
/// the window is told to look again shortly, and is told anew then while it is held.
pub fn hold_ends(since: Option<u64>, now: u64) -> u64 {
    let hold = purlis_core::sandbox::asks::HOLD.as_secs();
    match since {
        Some(at) if at.saturating_add(hold) > now => at.saturating_add(hold),
        Some(_) => now.saturating_add(LOOK_AGAIN),
        None => now.saturating_add(hold),
    }
}

/// **The app's reasons that make a chat's wait an update**: a report with nowhere to go and a
/// refused commit. Not a task of its that came to nothing (#1693): a chat in the queue only for
/// those asks nothing and is no ask at all ([`derive`]), and one that also waits on the person
/// for itself still asks, so a failure left unlooked-at never silences its notification.
pub fn reasons_besides_failures(needs: Option<&[crate::hooks::Need]>, refusals: usize) -> usize {
    needs.map_or(0, |needs| {
        needs
            .iter()
            .filter(|need| !matches!(need, crate::hooks::Need::TaskFailed { .. }))
            .count()
    }) + refusals
}

/// The chat that asked for chat `session` as a task, as `chats` records it while it is open.
fn asker_in(chats: &crate::chats::Chats, session: u32) -> Option<u32> {
    let from = chats.recorded_chat(session)?.from?;
    (from.mode == purlis_core::reopen::Mode::Task).then_some(from.chat)
}

/// Every ask project `held` has waiting now, as the window lists them.
pub fn every_ask(held: &crate::planes::Held) -> Asking {
    let plane = held.plane_id();
    let root = held.root();
    let locks = purlis_core::sandbox::policy::Locks::of(root);
    let board = held.board();
    let chats = held.chats();
    // The queue is the board's, whichever chat it is read through.
    let queue = chats
        .open_now()
        .first()
        .map(|any| board.now(any.session).queue)
        .unwrap_or_default();
    let store = held.dispatch_grants();
    let now = Instant::now();
    let now_secs = crate::sandboxing::now_secs();
    let permissions = held.hooks().asks().pending_since(now);
    let held_blocks = chats.blocks().every_since();
    let waiting_dispatches = store.every_waiting();
    // When each ask began, by its key (#1700): a hook ask's raise, read back from how long ago
    // it was; a dispatch's hold; a block's first hearing.
    let mut since: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for (raised, at) in &permissions {
        let ago = now.saturating_duration_since(*at).as_secs();
        since.insert(raised.id.to_string(), now_secs.saturating_sub(ago));
    }
    for one in &waiting_dispatches {
        since.insert(format!("dispatch:{}", one.id), one.at);
    }
    for (session, block, first) in &held_blocks {
        if let Some(first) = first {
            since.insert(block_key(*session, block), *first);
        }
    }
    let removals: Vec<_> = crate::removalasks::waiting(held)
        .into_iter()
        .map(|waits| {
            since.insert(removal_key(waits.session, &waits.workspace), waits.since);
            let holds = purlis_core::removalask::holds(root, &waits.workspace);
            (waits, holds)
        })
        .collect();
    let proposals: Vec<_> = crate::proposals::waiting(held)
        .into_iter()
        .inspect(|(one, _)| {
            since.insert(proposal_key(one.id), one.since);
        })
        .collect();
    let runner_writes = held.runner_writes().every();
    for one in &runner_writes {
        since.insert(runner_write_key(one.id), one.since);
    }
    let runner_dispatches = held.runner_dispatches().every();
    for one in &runner_dispatches {
        since.insert(runner_dispatch_key(one.id), one.since);
    }
    let sign_ins = purlis_core::profilestore::sign_in::waiting(root);
    for one in &sign_ins {
        since.insert(format!("sign-in:{}", one.profile.name), one.since);
    }
    let pool_sign_ins = purlis_core::profilestore::onrunner::waiting_on_pools(root);
    for one in &pool_sign_ins {
        since.insert(pool_sign_in_key(&one.profile, &one.pool), one.since);
    }
    let sandbox_only = purlis_core::profilestore::sandbox_only::waiting(
        root,
        &purlis_core::profilestore::listed_here(),
    );
    if let Some(one) = &sandbox_only {
        since.insert("sandbox-only:codex".to_owned(), one.since);
    }
    let browsers = held.browsers().waiting();
    for one in &browsers {
        since.insert(browser_key(one.session), one.since);
    }
    // An Ask whose browser is gone is not listed: it ends here, with its Update (#2015).
    held.needs_you().settle(held.browsers(), now_secs);
    let needs_you = held.needs_you().waiting();
    for one in &needs_you {
        since.insert(needs_you_key(&one.program), one.since);
    }
    let browser_sites = held.browsers().sites_waiting();
    for one in &browser_sites {
        since.insert(browser_sites_key(one.session), one.since);
    }
    let pool_logins = held.browsers().pool_logins_waiting();
    for one in &pool_logins {
        since.insert(pool_login_key(one.session), one.since);
    }
    let pool_tools = held.pool_tools().waiting();
    for one in &pool_tools {
        since.insert(pool_tool_key(one.session, one.run), one.since);
    }
    let declared = purlis_core::sandbox::services::declared(root);
    let waiting = Waiting {
        browsers,
        needs_you,
        browser_sites,
        pool_logins,
        pool_tools,
        sandbox_only,
        sign_ins,
        pool_sign_ins,
        removals,
        proposals,
        runner_writes,
        runner_dispatches,
        permissions: permissions.into_iter().map(|(raised, _)| raised).collect(),
        queue,
        at_its_prompt: &|session| board.glance(session).prompt,
        dispatches: waiting_dispatches
            .iter()
            .map(|one| store.told(plane, root, &locks, one))
            .collect(),
        blocks: held_blocks
            .into_iter()
            .map(|(session, block, _)| (session, block))
            .collect(),
        locks: &locks,
        everyone: crate::chatbrowsers::everyone_counts(root, &locks),
        declared: &declared,
        name_of: &|session| chats.shown_name(session),
        asker_of: &|session| asker_in(chats, session),
        only_updates: &|session| {
            let now = board.now(session);
            let reasons = reasons_besides_failures(now.needs.as_deref(), now.refusals.len());
            let hooks = held.hooks().board();
            only_updates(InQueue {
                failed: !hooks.failed_tasks(session).is_empty(),
                for_itself: hooks.needs_you_for_itself(session),
                reasons,
                waiting: hooks.state(session) == purlis_core::state::State::Waiting,
                at_its_prompt: hooks.waits_on_its_prompt(session),
            })
        },
        held: &|session, target, began| {
            chats.asking(session, Some(target)).then(|| {
                let ends = hold_ends(began.map(u64::from), now_secs);
                u32::try_from(ends).unwrap_or(u32::MAX)
            })
        },
        since: &|ask| {
            since
                .get(ask)
                .map(|at| u32::try_from(*at).unwrap_or(u32::MAX))
        },
        runner_of: &|session| chats.runner_of(session),
    };
    let task_questions: Vec<Shown> = crate::dispatched::reached_the_person(held)
        .into_iter()
        .map(|(reached, question)| {
            let chain = chain(reached.task, waiting.name_of, waiting.asker_of);
            task_question(reached, question.asked, chain)
        })
        .collect();
    Asking {
        plane: plane.clone(),
        // A grade's chat is drawn nowhere, and asks nobody (SK-16): the grade ends it at its
        // first ask, with that end, and none of its asks is listed.
        asks: derive(&waiting)
            .into_iter()
            .chain(task_questions)
            .filter(|ask| !chats.kept_from_the_window(ask.session))
            // The old `[harness_plugins]` keys: Needs you holds their Ask while the Skill studio
            // is on (#3689), so it is in the Inbox only while the studio is off.
            .chain(
                (!purlis_core::skills::studio_on_here())
                    .then(|| {
                        crate::oldplugins::notice_of(
                            root,
                            &held.proposals().paths_of(crate::proposals::BY_PURLIS),
                        )
                    })
                    .flatten()
                    .map(|old| old_harness_plugins(&old)),
            )
            .collect(),
    }
}

/// **Keep blocked on a sandbox host ask** (#1690): chat `session` is no longer held on the
/// block shown, so the registry stops listing it. It grants nothing and changes no sandbox; it
/// is the Notice's Keep blocked, told to the app, which before kept the block until the chat
/// ended. Answers whether the block was still held.
#[tauri::command]
#[specta::specta]
pub fn forget_sandbox_block(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    shown: crate::taskblocks::BlockShown,
) -> Result<bool, String> {
    let held = planes.held(&plane)?;
    let blocks = held.chats().blocks();
    Ok(
        match crate::taskblocks::shown_one(blocks, session, &shown) {
            Ok(block) => {
                blocks.answered(session, &block);
                // The person's no, kept where the Inbox's look back reads it (#1700).
                if block.what == crate::sandboxing::GrantWhat::Host {
                    crate::network::record_kept_blocked(
                        planes.network(),
                        held.root(),
                        held.chats(),
                        session,
                        &block.target,
                    );
                }
                true
            }
            Err(_) => false,
        },
    )
}

#[cfg(test)]
mod tests;
