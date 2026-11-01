// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The chats the app has open: the sessions, plus what each one was started as.
//!
//! `Sessions` knows how to run a program in a terminal and nothing about why. This knows
//! why: which harness a session is, what conversation it is under, and which one is in
//! front — everything a quit has to write down and a launch has to put back.
//!
//! It is a layer of its own so that the record is written from what the app itself did, and
//! not from anything a harness said. Nothing here reads a session's output.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use purlis_core::engine::Size;
use purlis_core::eventlog::{Attributes, Began, RunOf};
use purlis_core::harness::{Harness, SessionId};
use purlis_core::hoststart::Armed;
use purlis_core::reopen::{Chat, Focus, Record, Reopened, View};

use crate::host::{Opening, SessionHost};
#[cfg(test)]
use crate::sessions::{Reporting, Sessions};

/// `words` with the session a new conversation is started under, `--session-id <was>`, named
/// `fresh` instead; whether they held one. A `--resume` is left as it is.
fn renamed_session(words: &mut [String], was: &str, fresh: &str) -> bool {
    let mut renamed = false;
    for at in 1..words.len() {
        if words[at - 1] == "--session-id" && words[at] == was {
            words[at] = fresh.to_owned();
            renamed = true;
        }
    }
    renamed
}

/// Every identity variable a vault of the project at `root` declares — both halves of each
/// `env` binding — so a chat is started without any of them (#271 review, U6). `root` is the
/// open project's, never one found above the chat's folder (#1410). A registry that cannot be
/// read yields none: the chat still loses every `OP_*` by prefix. Read on the thread that
/// starts the chat; it is a small JSON read, once per start.
///
/// **Skipped in a fenced build.** Reading a vault's registry resolves through the plane and, in
/// a fenced test build, that aborts the moment it names a plane outside the fixture fence
/// (`purlis_core::fence`, charter-app#129) — which a unit test's project routinely is. A test
/// build therefore strips only by the `OP_` prefix; the declared-name strip is exercised at the
/// session builder ([`crate::sessions`] tests pass `env_strip` directly) and in the core.
fn declared_identity_vars(root: &Path) -> Vec<String> {
    purlis_core::hoststart::declared_identity_vars(root)
}

/// What more of this machine's environment the operator lets a chat of the project at `root`
/// have: the `[chat_env] pass` of its `charter.local.toml`. Read from the open project, never
/// from a project found above the chat's folder (#1410).
fn operator_env_pass(root: &Path) -> Vec<String> {
    purlis_core::chatenv::read(root)
}

/// What the project at `root` decides about the sandbox of a `harness` chat on no profile in
/// the folder `cwd`, on `machine` (#1410).
///
/// In order: the project's own manifest is read first, and a project that leaves the sandbox
/// off decides nothing more, unless an administrator's policy requires it here (D-1423-1).
/// Where the sandbox is in force, or the manifest is missing or cannot be read, the chat's
/// folder is checked next ([`purlis_core::sandbox::folder_refusal`]): a
/// chat with no folder, one outside the project, or one reached through a link is refused and
/// never started any other way. Only then is the sandbox decided. A system with no backend
/// (Windows) never sandboxes a chat, so its folder is not checked there.
fn project_sandbox(
    harness: Harness,
    root: &Path,
    cwd: Option<&Path>,
    machine: &purlis_core::sandbox::Machine,
    has: &dyn Fn(&str) -> bool,
    person: &purlis_core::sandbox::Person<'_>,
    grants: &purlis_core::sandbox::grant::Grants,
) -> Result<Option<purlis_core::sandbox::Decided>, String> {
    let opt_out = person.opt_out;
    let plane = purlis_core::sandbox::Plane::read(root);
    // An administrator's policy may require the sandbox where the project has not turned it on
    // (D-1423-1), and may forbid the opt-out no refusal then names (#1423).
    let locks = purlis_core::sandbox::policy::Locks::of(root);
    let leaves_it_off = !plane.missing() && !plane.unreadable() && plane.in_force(&locks).is_none();
    // A person's opt-out (#1342), or their own level confirmed at `off` (#2071), starts the
    // chat without the sandbox, so its folder confines nothing and is not checked: each sits
    // inside every refusal, as in `decide`.
    let yours_off = purlis_core::sandbox::your_level(root, person.level_as, &locks, person.made)
        == Some(purlis_core::sandbox::personal::Choice::Off);
    if !leaves_it_off && opt_out.is_none() && !yours_off && machine.os.has_backend() {
        let Some(cwd) = cwd else {
            return Err(purlis_core::sandbox::FolderRefusal::Missing.said(&locks));
        };
        if let Some(why) = purlis_core::sandbox::folder_refusal(root, cwd) {
            return Err(why.said(&locks));
        }
    }
    // A chat on no profile adopts no persona, so no persona's grants (#1362); a person's grants
    // for this one chat (#1342) are its own.
    purlis_core::sandbox::decide_granted(harness, root, machine, has, person, None, grants)
        .map_err(|not| not.said(&locks))
}

/// The workspace of the project at `root` a chat started in `cwd` works in, read once as it
/// starts (#1333): `None` at the project root or outside it. Against the app's own project,
/// never a root found from `cwd`: a clone carrying a manifest of its own would read as a
/// project root, and the chat's writes as a root chat's.
pub(crate) fn workspace_at_start(
    root: Option<&std::path::Path>,
    cwd: Option<&std::path::Path>,
) -> Option<String> {
    purlis_core::active::workspace_of_tree(root?, cwd?)
}

/// One chat the app has open, as the UI and the quit warning see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Open {
    pub session: u32,
    pub name: String,
    pub cwd: Option<PathBuf>,
    /// The workspace its directory was in when it started, resolved once then (#1333): what a
    /// brokered write is made in, which no link made later can move.
    pub workspace: Option<String>,
    pub harness: Option<Harness>,
    /// The declaration it started on, where that is a project's or this machine's
    /// (`purlis_core::start::Ready::declared`): what its header's card is made from (#1134).
    pub declared: Option<purlis_core::harness_declaration::Declaration>,
    /// The harness profile it started on, and the persona it adopted — what the sidebar
    /// names a chat by, beside its harness.
    pub profile: Option<String>,
    pub persona: Option<String>,
    /// Whether it is the chat in front. At a launch this is the one that was in front when
    /// the app was quit, so the window comes back looking as it was left.
    pub in_front: bool,
    /// How it came to be open. A chat the operator just started is `Fresh`, the same as one
    /// that could not be resumed — the difference is only interesting at a relaunch, which
    /// is where the UI says it.
    pub how: Reopened,
    /// Whether the operator pinned it (ADR 0039). It rides the record, so a pinned
    /// chat comes back pinned; see [`purlis_core::reopen::Chat::pinned`].
    pub pinned: bool,
    /// The name the operator gave it, where they gave one (charter-app#254). It rides the
    /// record too; see [`purlis_core::reopen::Chat::label`].
    pub label: Option<String>,
    /// The chat a handoff opened it from, where one did (charter-app#258). It rides the
    /// record; see [`purlis_core::reopen::Chat::from`].
    pub from: Option<purlis_core::reopen::HandedFrom>,
    /// Whether the window draws it as a tab: [`purlis_core::reopen::Chat::has_tab`].
    pub tab: bool,
    /// The chat its tab shows in place of it, where the person switched the tab to one
    /// (#1486): [`purlis_core::reopen::Chat::shows`].
    pub shows: Option<u32>,
    /// The chat whose tab it has a pane in, where it is not its tab's own chat (#1489):
    /// [`purlis_core::reopen::Chat::beside`].
    pub beside: Option<u32>,
    /// How much its sandbox holds as it started (#2071): the full sandbox or the floor, and
    /// `None` for a chat that runs with no sandbox.
    pub sandbox: Option<purlis_core::sandbox::Level>,
    /// Why it runs with no sandbox where the floor could not be applied here (#2071, Q2): what
    /// its tab's sandbox mark says. `None` for any other chat.
    pub floorless: Option<String>,
    /// Why what it was doing could not be taken from the session host as it was reattached
    /// (#2006): its tab says so until the Notice is dismissed. `None` for any other chat.
    pub untaken: Option<String>,
}

/// Who an open chat is beyond this launch, and the directory it works in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatAt {
    /// Its ULID (ADR 0066), `None` only for a chat that has not been given one.
    pub id: Option<String>,
    pub cwd: Option<PathBuf>,
}

/// One chat open here, as Use skill… works its skills out ([`Chats::skills_of`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatForSkills {
    /// The workspace it works in, `None` at the plane root.
    pub workspace: Option<String>,
    /// The persona it was started as.
    pub persona: Option<String>,
    /// Its harness, `None` for a shell or a program purlis has no adapter for.
    pub harness: Option<Harness>,
    /// Where its plane skills came from, as its start read them (#3234): what its set is routed
    /// from again (#3245).
    pub start: purlis_core::hoststart::ChatSkills,
    /// How its router did on its latest prompt (#4297), held with `start` so the two are of one
    /// start.
    pub last_prompt: Option<purlis_core::skills::router::Record>,
    /// What its prompts are digested under (SK-38, #3369): its prompt key, which its hooks read
    /// too. None for a chat whose host gave it none.
    pub salt: Option<purlis_core::skillspan::PromptSalt>,
    /// The folder it was started in: which of its workspace's clones it works in (#3462).
    pub cwd: Option<PathBuf>,
    /// Whether the plane's skills reached it at its start ([`Running::plane_skills`], #3445).
    pub plane_skills: bool,
    /// Why its chats get no skills, where its harness is a declared one skills are not
    /// supported on (SK-30): [`purlis_core::skills::not_supported_on`].
    pub not_supported: Option<String>,
}

/// One chat open here that its start routed by a router extension ([`Chats::routings`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRouted {
    pub session: u32,
    pub workspace: Option<String>,
    pub persona: Option<String>,
    pub routed: std::sync::Arc<purlis_core::skills::router::Routed>,
    /// How its router did on its latest prompt, where it was asked on one (#4297).
    pub last_prompt: Option<purlis_core::skills::router::Record>,
}

/// One chat open here, as its theme drafts are found and named ([`Chats::drafting`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drafting {
    pub session: u32,
    /// Its ULID (ADR 0066): the name of its drafts folder.
    pub id: String,
    /// What the person calls it.
    pub name: String,
    /// The workspace it works in, `None` at the plane root.
    pub workspace: Option<String>,
}

/// The most chats one record may start at a launch. The product's scale is fifty (the
/// spec's limits table); this is only a backstop against a record nobody meant.
const MOST_AT_ONCE: usize = 200;

/// Where the record goes whenever what is open changes.
///
/// It is a callback rather than a file so that this module keeps knowing nothing about the
/// plane — and so a test can see exactly when a write would happen.
pub type Recorder = Box<dyn Fn(&Record) + Send + Sync>;

/// Told as each chat starts, BEFORE its program does: its number, the harness it runs, the
/// conversation charter chose for it, and whether it is a grade's chat, which the window draws
/// nowhere and the board never queues (SK-16, #3583). Everything the board needs to judge a
/// report about it.
pub type Starting = Box<dyn Fn(u32, Option<Harness>, Option<String>, bool) + Send + Sync>;

/// Told as each chat starts, BEFORE its program does, of the run the start begins: the chat's
/// number, its id, the run's id, why it began and what it is (ADR 0066, #868), and whether its
/// `run.started` is held until the start goes on. What the event log is told.
pub type Beginning = Box<dyn Fn(u32, RunOf<'_>, Began, &Attributes, RunSaid) + Send + Sync>;

/// Told a chat's number and its routing record as its run begins (SK-15b).
pub type Routed = Box<dyn Fn(u32, &purlis_core::skills::router::Record) + Send + Sync>;

/// When a run's `run.started` is written (#2250).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunSaid {
    /// As it begins.
    Now,
    /// Once its start goes on ([`Chats::when_a_held_start_is_settled`]): a sandboxed Claude Code
    /// chat whose project settings its start screens, which may be started once more at once.
    /// A start put aside says nothing of it, nor of the trust event its start holds with it
    /// (the sandbox coming back on, #2252), which is written just before it.
    Held(Option<purlis_core::eventlog::HeldTrust>),
}

/// What became of a start whose run was held ([`RunSaid::Held`]) (#2250, #2252).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeldStart {
    /// It goes on: its run is said.
    GoesOn,
    /// It was put aside at once: its run is never said, and what its program still sends is
    /// of no run.
    TakenBack,
}

/// What a start decided about a chat's sandbox (ADR 0067 §7): the trust event to write, under
/// the chat and the run the start is about to begin, BEFORE its program runs; or, once a new
/// chat has started, how it counts towards this machine's opt-out rate (ruling V78 d). Each
/// call carries one of the two.
pub struct Sandboxing<'a> {
    pub change: Option<&'a purlis_core::sandbox::Change>,
    /// The chat's id and the run it begins, for `change`.
    pub run: Option<RunOf<'a>>,
    pub counted: Option<purlis_core::sandbox::local::Started>,
    pub harness: Option<Harness>,
    pub persona: Option<&'a str>,
}

/// Told of [`Sandboxing`]. An `Err` is that the trust event was not written: a person's opt-out
/// is then not started, so no opt-out ever runs unaudited (ADR 0067 §7).
pub type Trusting = Box<dyn Fn(Sandboxing<'_>) -> Result<(), String> + Send + Sync>;

/// What a run a start begins is (ADR 0066, "A run is a stretch…", #868), from what the start
/// decided and never from anything the chat said: the persona it runs as, its harness, the
/// profile it started on, the sandbox backend it is under and the conversation it was given.
///
/// A chat that runs no harness (a shell) has [`purlis_core::eventlog::NONE`] for a harness and,
/// unless it names one, a profile; a harness chat whose record names no profile does not know
/// its profile, and says so. Its model source is its profile's sign-in kind, which the start
/// sets (ADR 0087 as amended, #2036); no model is chosen yet, so the provider and the model are
/// not known.
pub(crate) fn run_attributes(
    chat: &Chat,
    harness: Option<Harness>,
    persona: Option<&str>,
    sandboxed: bool,
    conversation: Option<&str>,
) -> Attributes {
    use purlis_core::eventlog::NONE;
    let none = || Some(NONE.to_owned());
    // The record holds a profile to a name purlis would mint, on the way in.
    let profile = chat.profile.clone().filter(|name| !name.is_empty());
    Attributes {
        persona: persona.map(str::to_owned).or_else(none),
        harness: harness.map_or_else(none, |h| Some(h.name().to_owned())),
        profile: profile.or_else(|| harness.is_none().then(|| NONE.to_owned())),
        model_source: None,
        provider: None,
        model: None,
        sandbox: if sandboxed {
            purlis_core::sandbox::Os::this()
                .backend_word(harness)
                .map(str::to_owned)
        } else {
            none()
        },
        conversation: conversation.map(str::to_owned),
        // The window starts each of its own chats (#2171).
        started_by: Some(
            purlis_session_protocol::auth::Scope::LocalUi
                .word()
                .to_owned(),
        ),
    }
}

/// Why a chat is being started, which is what decides why its run begins (ADR 0066).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Why {
    /// A chat that was not open before: its first run, `start`.
    New,
    /// A chat a relaunch put back from the record: `reopen` or `fresh`, by how it came back
    /// ([`Began::at_relaunch`]).
    Relaunch,
    /// A chat a relaunch put back that its record held no id for, so one was minted at this
    /// launch: `reopen`, whatever it came back as. ADR 0066's migration: "Its first run after
    /// the upgrade has `cause: reopen`."
    Upgrade,
    /// A chat started again in the place of one whose harness could not bring its
    /// conversation back (`lostOnResume`): the same chat, `fresh`.
    Again,
}

/// **How a start of `chat` is made** (#2071), from the app's own record of it: asked for by
/// another chat (a dispatch or a handoff, at its first start and every later one), else the
/// person's new chat in the window, else the app starting it again.
fn made_of(chat: &Chat, why: Why) -> purlis_core::sandbox::Made {
    use purlis_core::sandbox::{Asking, Made};
    if let Some(from) = chat.from.as_ref().filter(|from| !from.by_person) {
        return Made::Asked {
            how: match from.mode {
                purlis_core::reopen::Mode::Handoff => Asking::Handoff,
                purlis_core::reopen::Mode::Task => Asking::Dispatch,
            },
            // The whole chain above it, nearest first, as the record keeps it; a record that
            // cannot say keeps none.
            above: from.above.clone(),
        };
    }
    match why {
        Why::New => Made::Window,
        Why::Relaunch | Why::Upgrade | Why::Again => Made::Relaunch,
    }
}

/// **Whose level a start of `chat` reads** (#2071): its own persona for a chat on a profile,
/// and the project's (`None`) for one on no profile, which adopts no persona. The one answer
/// its start and the check of a running chat against its next start both take.
fn level_as_of(chat: &Chat) -> Option<&str> {
    chat.profile.as_ref().and(chat.persona.as_deref())
}

/// The most chats that were closed this launch whose ids are remembered, for
/// [`Chats::start_ready_instead_of`]. That asks about a chat the window closed a moment ago.
const LET_GO_HELD: usize = 256;

use purlis_core::reopen::mint as minted;

/// One chat the app has open: what it was started as, how it came back, and the harness it
/// actually runs.
///
/// The harness is KEPT rather than asked of the chat again, because asking means asking its
/// program's NAME, and a profile's command is commonly a wrapper. The board is told this
/// same value at the start, so the sidebar and the board cannot disagree about what a chat
/// is running — which they did: the board learned the profile's declared kind while the
/// sidebar read `claude-stand-in` and said "no harness".
#[derive(Debug)]
struct Running {
    chat: Chat,
    how: Reopened,
    harness: Option<Harness>,
    /// The declaration a project or this machine gives the chat's harness, as its start read
    /// it (`purlis_core::start::Ready::declared`, #967, #968): what its pane's Shift+Enter sends
    /// and its header's card is made from.
    declared: Option<purlis_core::harness_declaration::Declaration>,
    /// [`Open::workspace`], resolved as it started.
    workspace: Option<String>,
    /// What runs beside a chat charter wraps — its egress proxy and its own temp directory —
    /// for as long as the chat is open (ADR 0067 §2). Dropped with it.
    confinement: Option<purlis_core::sandbox::Confinement>,
    /// What its sandbox was compiled to as it started, which a command run on its behalf is
    /// held to (#1407); `None` for a chat started without one.
    confines: Option<purlis_core::sandbox::Confines>,
    /// What decided its sandbox as it started besides the project's settings (#1428): the
    /// persona grants a handoff left it holding, and what a person had granted this chat. A
    /// start compiled now with the same two differs from [`Self::confines`] only by settings.
    started_with: StartedWith,
    /// **How its run is sandboxed, by the record of how it started** (#2636): this window's own,
    /// for a run it started, or the session host's, for one it reattached. `None` where that
    /// record cannot be read, which is taken as sandboxed.
    ran: Option<purlis_session_protocol::session::RunSandbox>,
    /// **Whether its host composed it by its names** (#2446): a runner's chat, whose sandbox,
    /// tools and line the runner compiled there, so this window holds no record of what its
    /// sandbox compiled to, and a restart would not give it one (#2692).
    composed_by_its_host: bool,
    /// **Whose persona grants it runs with, as its start read them** (#2691): on a profile, the
    /// grants a handoff left it holding, else its own persona's, else the project's default
    /// persona at its start; on no profile, none ([`grants_persona_of`]). The persona its
    /// sandbox was compiled for.
    /// Fixed at the start, as its sandbox is: an Allow of its own grants or a new default
    /// changes it at the next start.
    grants_as: Option<String>,
    /// **The plane skills it started with** (#2770, ADR 0099 §5), as its harness would miss a
    /// change to them: what [`Chats::skills_changed`] compares with the skills now. `None` for a
    /// chat that runs no harness.
    skills: Option<purlis_core::skills::Stamp>,
    /// **Whether the plane's skills reached it at its start** (#3445): the Skill studio's flag
    /// as this window read it once as the chat started (`purlis_core::skills::reach_chats_here`),
    /// which its arming, its Notice, its stamp and Use skill… all take. A chat reattached from
    /// the session host (#2006) takes its first start's, from its record (#3466), or the flag
    /// as read at its reattach where a record written before that has none. A runner's chat is
    /// armed by its runner (#2446), so this is this machine's word for it.
    plane_skills: bool,
    /// **Where its plane skills came from, read once at its start** (#3234): what its restart's
    /// stamp and Use skill… route it from again, so neither reads its clone a second time and
    /// both see the set it was handed (#3245).
    skills_read: purlis_core::hoststart::ChatSkills,
    /// **How its router did on its latest prompt** (#4297, ADR 0102 §6): the routing record, which
    /// holds no word of the prompt, for the Routing page and its tab. Held only while it runs;
    /// the event log keeps every one.
    last_prompt: Option<purlis_core::skills::router::Record>,
    /// **A runner chat's workspace clones as its session started here** (#2053): each clone of
    /// the plane's workspaces and its `.git`'s identity, read as this window started it. Only
    /// these name its repos for a GitHub App's token this desktop mints for it. `None` for a
    /// chat of this machine, whose sandbox records its own, and for one reattached, whose start
    /// this window did not see.
    clones_at_start: Option<Vec<purlis_core::forge::apptoken::Started>>,
    /// **Its prompt key** (#3369): what its hooks key the digests of its prompts by, and Use
    /// skill… and a curation action the digests of what they type into it. Minted here for a
    /// harness chat this window composes; answered by the host for one the host composed or
    /// reattached. Held in memory alone: no record of the chat's keeps it. None for a shell, and
    /// for a chat whose host gave it none.
    prompt_key: Option<purlis_core::skillspan::PromptSalt>,
    /// **The persona it runs as, as its start named it**: its run's `persona` (ADR 0066), what a
    /// runner's skill run of it is logged by here (SK-20b, #3084). `None` on no persona.
    runs_as: Option<String>,
}

/// **Whether the plane's skills reach a chat starting now** (#3445, #3466): a chat
/// `reattaching` from the session host keeps the flag its first start read, `recorded`, where its
/// record has one; any other start reads the Skill studio's flag now (`read`).
fn plane_skills_at_start(
    reattaching: bool,
    recorded: Option<bool>,
    read: impl FnOnce() -> bool,
) -> bool {
    if reattaching {
        recorded.unwrap_or_else(read)
    } else {
        read()
    }
}

/// **What a chat starting now is told of a harness no skill reaches** (SK-30, #3585): the one
/// line [`purlis_core::skills::not_supported_on`] says of its `declared` harness, which has no
/// adapter (`harness` is `None`), so no SessionStart briefing, and takes no MCP servers, so no
/// MCP instructions either. Its tab is the one place left to say it. Said at the chat's first
/// start alone (`why` is [`Why::New`]): a relaunch or a start again is the same chat, told
/// already, and Use skill… and the studio keep saying it. And only where the plane's skills
/// would reach a chat here (`plane_skills`): with the Skill studio off no chat gets them, and
/// there is nothing to miss. Read from the declaration the start already holds: nothing new is
/// read on a chat's open.
fn not_supported_at_start(
    why: Why,
    plane_skills: bool,
    harness: Option<Harness>,
    declared: Option<&purlis_core::harness_declaration::Declaration>,
) -> Option<String> {
    if why != Why::New || !plane_skills || harness.is_some() {
        return None;
    }
    declared
        .and_then(purlis_core::skills::not_supported_on)
        .map(|one| one.said)
}

/// **The routing a chat starting now keeps from its first start** (SK-15f, #3578): a chat
/// `reattaching` from the session host routes its set as its record says its start did, where
/// the record says, and its router is not asked again. Only beside the Skill studio flag that
/// start took (`flag_recorded`, #3466): a routing kept without it would be judged by a flag read
/// now, which may not be the one it was routed under. Any other start keeps nothing: it is
/// routed now.
fn routing_kept_at_start(
    reattaching: bool,
    flag_recorded: bool,
    recorded: Option<&purlis_core::skills::router::Kept>,
) -> Option<purlis_core::skills::router::Kept> {
    recorded.filter(|_| reattaching && flag_recorded).cloned()
}

/// **Whether a chat starting now is armed with its skills by the runner it runs on** (SK-15g,
/// #3604): one the window starts there (`on_the_runner`, #2446), and one it reattaches
/// (`reattaching`, #2006) that `runs_on_a_runner` says it found on a host that composes its
/// harness chats. Such a chat's skills are the runner's: the window reads none of them here, and
/// asks no router of its own, at the reattach as at the start. Every other chat is armed here.
fn armed_by_its_runner(
    on_the_runner: bool,
    reattaching: bool,
    runs_on_a_runner: impl FnOnce() -> bool,
) -> bool {
    on_the_runner || (reattaching && runs_on_a_runner())
}

/// **The personas a chat is named by** (#2691), from one read of the app's record of its run
/// ([`Chats::persona_of`]), so every path names it from the same place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatPersona {
    /// The persona it was started as: **whose vaults it may use**, as every brokered read of a
    /// vault judges it (`secret exec`, a browser login's vault, #2599). Never a fallback: a
    /// chat on no persona uses no persona's vault.
    pub started_as: Option<String>,
    /// **Whose grants it runs with** ([`Running::grants_as`]): the persona its tools, hosts and
    /// browser sites are its by, what its browser reaches and what its Asks are about.
    pub grants: Option<String>,
}

impl Running {
    /// [`Chats::persona_of`], of this run.
    fn persona(&self) -> ChatPersona {
        ChatPersona {
            started_as: self.chat.persona.clone(),
            grants: self.grants_as.clone(),
        }
    }
}

/// The skills a chat started without (SK-5, #2768; SK-8, #2771, decision 41): the ones its
/// persona names, each as the persona's `skills:` list writes it, and the ones that could not be
/// loaded. What its tab's Notice says.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SkillsMissing {
    /// The persona the chat runs as, or `None` for a chat on no persona.
    pub persona: Option<String>,
    /// Installed skills this machine does not have for the chat's harness.
    pub missing: Vec<String>,
    /// Built-in skills purlis has none of: a typo, or a name from another version of purlis.
    pub no_built_in: Vec<String>,
    /// Skills the chat would have loaded that could not be loaded, each with why.
    pub broken: Vec<SkillBroken>,
    /// The project's skills the repo the chat works in leaves out, each with why (SK-53).
    pub by_a_repo: Vec<SkillBroken>,
    /// The repo's own skills named like one of purlis's built-in skills, which the chat's
    /// program loads beside purlis's (SK-37).
    pub beside_a_built_in: Vec<String>,
}

/// A skill a chat would have loaded that could not be loaded (SK-8): the core's `Broken`, for
/// the window. purlis-core keeps no UI typing crate, so its level is the studio's `Level`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SkillBroken {
    /// Its name, or its folder's where it names none.
    pub name: String,
    /// Its level, and whether it is a local skill.
    pub level: crate::skillstudio::Level,
    pub local: bool,
    /// Its level as a person reads it: `project`, `local workspace` (the core's `level_shown`).
    pub label: String,
    /// Why, in one line: the core's sentence.
    pub why: String,
}

impl From<purlis_core::skills::Broken> for SkillBroken {
    fn from(one: purlis_core::skills::Broken) -> Self {
        SkillBroken {
            label: purlis_core::skills::level_shown(one.origin, one.local),
            name: one.name,
            level: one.origin.into(),
            local: one.local,
            why: one.why.to_string(),
        }
    }
}

/// [`Running::started_with`].
#[derive(Debug, Clone, Default)]
struct StartedWith {
    held: Option<purlis_core::reopen::HeldGrants>,
    grants: purlis_core::sandbox::grant::Grants,
}

/// A chat a launch could not start, as the window lists it (NO-3): by its id, which Retry now
/// and Forget name it by, with its name and why.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct NotStarted {
    /// The chat's id (ADR 0066), stable across launches and never shared.
    pub id: String,
    /// What its tab was called. Two waiting chats can share one.
    pub name: String,
    /// Why it did not start.
    pub why: String,
    /// The approval its profile needs before it can start, where it needs one (#1246,
    /// D-1246-5): read when its start was last refused, so the window offers **Review and
    /// approve…** from this and never from the words of `why`. Null for a chat on no profile,
    /// a profile not declared, or one with nothing to approve.
    pub approval: Option<NeedsApproval>,
}

/// **A profile's command waiting on the operator's approval** (ADR 0022), as a waiting chat's
/// Notice offers it (#1246): which profile, and the exact line the approval is for.
///
/// What the picker's row says of the same profile (`ProfileRow`), in its words, so the window
/// draws the picker's own sentence and mark (`ProfileApproval.tsx`, ruling V69). `shown` is the
/// line `approve_profile` checks the click against: a file changed since this was read is
/// refused there, and nothing is recorded.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct NeedsApproval {
    /// The profile's name, as `approve_profile` takes it.
    pub profile: String,
    /// Its `kind`.
    pub kind: String,
    /// Where it was declared: `built-in`, `charter.local.toml` or the project's harnesses.
    pub source: String,
    /// `new` or `changed`, as the picker's row says it.
    pub approval: String,
    /// What it would run, as the picker shows it (`profiletrust::shown`), already contained.
    pub shown: String,
}

impl NeedsApproval {
    /// What `chat`'s profile needs approved at `root` before it can start, read through the
    /// launch's own read, as `approve_profile` reads it: a profile in a file the launch
    /// refuses has nothing a yes could buy.
    fn of(chat: &Chat, root: &std::path::Path) -> Option<Self> {
        Self::of_profile(chat.profile.as_deref()?, root)
    }

    /// What the profile `name` needs approved at `root` before a chat on it can start, read as
    /// [`NeedsApproval::of`] reads a chat's: for a start the window asked for that the core
    /// refused, such as the Skill studio's (#3942).
    pub(crate) fn of_profile(name: &str, root: &std::path::Path) -> Option<Self> {
        let (set, _check) = purlis_core::profiles::for_launch(root);
        let profile = set.get(name)?;
        let needs = purlis_core::profiletrust::approval_needed(root, profile)?;
        Some(Self {
            profile: profile.name.clone(),
            kind: profile.kind.clone(),
            source: profile.source.as_str().to_owned(),
            approval: needs.as_str().to_owned(),
            shown: purlis_core::profiletrust::shown(root, profile),
        })
    }
}

/// A chat a launch could not start, as it is held: the chat, why, and the approval its profile
/// needs ([`NeedsApproval`]).
#[derive(Debug)]
struct Waiting {
    chat: Chat,
    why: String,
    approval: Option<NeedsApproval>,
    /// What the launch would have told it as it started (#1513): a task still owing its
    /// report is told to carry on, and Retry now tells it the same.
    told: Option<&'static str>,
}

/// A chat a launch could not start, as [`Chats::waiting_to_start`] hands it over.
#[derive(Debug, Clone)]
pub(crate) struct WaitingChat {
    pub chat: Chat,
    pub why: String,
    pub approval: Option<NeedsApproval>,
    /// What it is told as it starts, kept while it is held aside (#1513).
    pub told: Option<&'static str>,
}

/// A chat a launch has registered and not started yet ([`Chats::begin_put_back`]).
#[derive(Debug, Clone)]
struct Coming {
    /// The chat as it was recorded, under the id and the number it will start with.
    chat: Chat,
    /// The number its tab is drawn under until it starts, which is the one it starts as.
    number: u32,
    why: Why,
    /// What the launch tells it as it starts (#1513).
    told: Option<&'static str>,
    /// Whether its start is under way, so no second one takes it.
    starting: bool,
    /// The workspace its folder is in, read once as it is registered: where the window files
    /// its tab until it has started and the project's own read lists it (ADR 0036).
    workspace: Option<String>,
}

thread_local! {
    /// Whether this thread is starting a launch's chats: their record is written once, when
    /// the last has been tried, and not once per chat ([`Chats::finish_put_back`]).
    static PUTTING_BACK_HERE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Every chat the app has open, and which of them is in front.
pub struct Chats {
    /// The open project's root, as the app holds it (#1410): the project of every chat
    /// started here, whatever folder the chat stands in.
    project: PathBuf,
    /// Whatever runs the sessions (FD-3). A trait object, so nothing here can reach past
    /// [`SessionHost`] to a pty: a chat layer that did would not compile against another host.
    sessions: Box<dyn SessionHost>,
    /// Told as each chat starts, before its program does.
    starting: Mutex<Option<Starting>>,
    /// Told as each chat put back is reattached in the session host, before anything else is
    /// told of it (#2006).
    reattached: Mutex<Option<Starting>>,
    /// Told of the run each start begins, before its program runs (ADR 0066).
    beginning: Mutex<Option<Beginning>>,
    /// What asks a chat's router extension at its start (SK-15b): none until the app hands one,
    /// so a chat whose workspace or persona chose one is routed by the built-in router.
    router: Mutex<Option<Arc<dyn purlis_core::skills::router::AskRouter>>>,
    /// Told each chat's routing record as its run begins, where its start asked a router
    /// extension (SK-15b): what the event log is told.
    routed: Mutex<Option<Routed>>,
    /// Told, by number, what became of a start whose run was held ([`RunSaid::Held`]).
    #[allow(clippy::type_complexity)]
    settled: Mutex<Option<Box<dyn Fn(u32, HeldStart) + Send + Sync>>>,
    /// Told what each start decided about the chat's sandbox (ADR 0067 §7).
    trusting: Mutex<Option<Trusting>>,
    /// What a start found to say on a chat's tab after the core's start had spoken: that its
    /// trust event could not be written. Taken by the window's start ([`Self::start_notes`]).
    /// Every open sandboxed Claude Code chat's project settings files, watched and screened
    /// again on each change (ADR 0067 as amended 2026-10-16, [`crate::projectsettings`]).
    project_settings: crate::projectsettings::Watch,
    /// The size each open chat's terminal was last given: what a restart the core drives itself
    /// starts it at ([`Self::restart_for_project_settings`]).
    sizes: Mutex<HashMap<u32, Size>>,
    /// The runs ended so that the same chat starts again at once (ADR 0067 as amended
    /// 2026-10-16): their program's end is no end of the chat, and is taken as none.
    replacing: Mutex<std::collections::HashSet<u32>>,
    late_notes: Mutex<HashMap<u32, Vec<String>>>,
    /// The chats told what their settings left out on the way to a runner, by session, so a chat
    /// started again says it once (#3867).
    left_out_told: Mutex<std::collections::HashSet<u32>>,
    /// The skills each chat's persona names that it started without (SK-5, #2768), by session:
    /// what its tab's Notice says until it is put away ([`Self::skills_missing`]).
    skills_missing: Mutex<HashMap<u32, SkillsMissing>>,
    /// Why each chat that runs with no sandbox where the floor could not be applied does
    /// (#2071, Q2), by session: what its tab's sandbox mark says ([`Open::floorless`]).
    floorless: Mutex<HashMap<u32, String>>,
    /// Why what each chat reattached was doing could not be taken from the session host, by
    /// session (#2006): [`Open::untaken`].
    untaken: Mutex<HashMap<u32, String>>,
    /// The older Claude Code a chat's late notes say it is on (#1699): marked said once the
    /// window takes them ([`Self::start_notes`]), not before.
    older_in_notes: Mutex<HashMap<u32, purlis_core::sandbox::ClaudeCodeVersion>>,
    /// This device's id, the origin device of every chat minted here; `None` where the machine
    /// store has none to give (ADR 0031), which a chat records as `unknown`.
    device: Option<String>,
    /// This machine's config home, where the pools this machine keeps and which workspaces run
    /// their chats on one are (#2679); `None` where nothing is kept there, a test's.
    pools_in: Option<PathBuf>,
    /// How a chat's profile is kept, and found signed in, on the pool's runner it starts on
    /// (#2049); `None` where nothing is kept there, a test's.
    profiles_on_pools: Option<Arc<dyn crate::poolprofiles::ProfilesOnPools>>,
    /// What each chat closed this launch was, by number, for a start in one's place: its ids,
    /// and where it stood in a lineage.
    let_go: Mutex<HashMap<u32, LetGo>>,
    /// Told when a chat that was announced turned out not to start.
    #[allow(clippy::type_complexity)]
    never_started: Mutex<Option<Box<dyn Fn(u32) + Send + Sync>>>,
    /// What the app ships that a chat is armed with: its own `charter` and its own plugin.
    shipped: crate::Shipped,
    open: Mutex<HashMap<u32, Running>>,
    /// The open chats whose program has ended, their tabs still open (#4202).
    ended: Mutex<std::collections::HashSet<u32>>,
    front: Mutex<Option<u32>>,
    /// Chats a launch could not start, and why. They are kept because the record has to
    /// keep them: a workspace directory that has moved, or a harness mid-reinstall, must
    /// not silently delete the chat on the next write.
    would_not_start: Mutex<Vec<Waiting>>,
    /// The chats a launch is still putting back, in the record's order: each registered
    /// before any starts, and taken out as it starts or is found not to
    /// ([`Self::begin_put_back`], `crate::restoring`). The window draws their tabs as
    /// restoring, and every record written meanwhile holds them as they were recorded.
    coming: Mutex<Vec<Coming>>,
    /// What lets a few of them start at once, across every project.
    gate: Arc<crate::restoring::Gate>,
    /// The ids of the chats launches registered, in the order their records held them: a
    /// chat that did not start keeps its place among the others that did not.
    recorded_order: Mutex<Vec<String>>,
    /// The tabs the window has open that hold a view rather than a chat, as it last said.
    ///
    /// **The window's to say and this layer's to write down**, and nothing else: a view has no
    /// session, so there is nothing here that could know one opened. They are held beside the
    /// chats only because the record is one file and is written whole, in one place, under
    /// [`Self::writing`] — a second writer of `reopen.json` would be two answers racing to disk.
    views: Mutex<Vec<View>>,
    /// The branch the window's sidebar is focused on — its cockpit (FM-5) — as it last said,
    /// or at a launch as the record had it. Held here for [`Self::views`]' reason.
    focus: Mutex<Option<Focus>>,
    /// The order the window's strip draws the chats in, by session, as it last said — or, at
    /// a launch, the order the record listed them in (ADR 0039, as amended by SI-6).
    ///
    /// **The window's to say, for the reason [`Self::views`] is**: the operator drags a tab
    /// there, and nothing here could know it moved. It is held here and not in the window
    /// because the record is written from here, and because a reloaded window asks this layer
    /// what is open ([`Self::open_now`]) and has to get the strip back in the order it left it.
    /// A chat it has not placed yet — one that has just started — comes after the ones it has
    /// ([`Self::in_order`]).
    order: Mutex<Vec<u32>>,
    record_it: Recorder,
    /// Held across building a record and handing it over, so two changes at once cannot
    /// write themselves out of order and leave the older one on disk.
    writing: Mutex<()>,
    /// Set while a record is being put back, so reading one does not write it again once
    /// for every chat in it — fifty chats would be fifty writes of the same file, at the
    /// one moment the app is being measured for cold start.
    /// Set when every chat is being ended, at quit or when the project is closed: from then on
    /// nothing writes the record ([`Self::write_it_down`]).
    ending: AtomicBool,
    /// The most chats one record may start: [`MOST_AT_ONCE`], but for a test of the bound,
    /// which would otherwise open that many terminals (#1139).
    most_at_once: usize,
    /// What a person let each chat do past its project's sandbox, from a block's Notice
    /// (#1342), by the chat's id: handed to that chat's starts alone, so a restart on its
    /// conversation keeps it and no other chat ever gets it. **In memory only**: a grant for
    /// one chat ends with the app.
    grants: Mutex<HashMap<String, Vec<ChatGrant>>>,
    /// How many times each chat's grants were taken back (#2454), by the chat's id: by a
    /// Revoke, or a close that ended them. A grant reserved before one is never kept. Locked
    /// after `grants` wherever both are.
    taken_back: Mutex<HashMap<String, u64>>,
    /// The copy of a kube context each chat was pointed at (qw384b), by the chat's id: written
    /// into that chat's own folder at each of its starts alone. **In memory only**.
    kubeconfigs: Mutex<HashMap<String, String>>,
    /// The sandbox block each open chat is held on now, as the app heard it (#1508): what an
    /// answer to several tasks is checked against. **In memory only**, and gone with the chat.
    blocks: Arc<crate::taskblocks::Blocks>,
    /// Who is told a project's asks may have moved where nothing else tells the window (#1709):
    /// a live ask held for its chat whose Notice the throttle held back. Empty until the
    /// project's hooks listen.
    asks_moved: Arc<Mutex<Option<AsksMoved>>>,
    /// What hears each host purlis's own proxy refused a chat it wraps (Codex, opencode,
    /// #1663), as that chat's block: the road a hook's block takes into the app
    /// ([`crate::hooks::Hooks::block_hearer`]). Empty until the project's hooks listen.
    refused: Arc<Mutex<Option<crate::hooks::Blocks>>>,
    /// What hears, by chat, how many hosts its proxy refused past the ones it names (#1353).
    /// Empty until the project's window listens.
    refused_more: Arc<Mutex<Option<MoreRefused>>>,
    /// What hears each connection purlis's own proxy carried for a chat it wraps (#1664),
    /// coalesced by the proxy: what keeps every connection in the network record. Empty until
    /// the project records its network.
    reached: Arc<Mutex<Option<ProxyReached>>>,
    /// Who keeps purlis's word on a chat's held connections for its next turn (#1666).
    /// Told each open chat whose reach the person narrowed while it runs (#2013).
    narrowed: Mutex<Option<Narrowed>>,
    network_words: Mutex<Option<NetworkWords>>,
    /// The session record the app last wrote for each open chat, project-relative (#1436): what
    /// a task's report names as its record. The app's own knowledge of what it wrote, so a
    /// report never names a path its chat chose. **In memory only**, and gone with the chat.
    records: Mutex<HashMap<u32, String>>,
    /// The brief each dispatch this app started a chat on was sent, as a digest, by the
    /// dispatch's id (#1609): what a start again with no conversation holds a record's brief to
    /// before it hands it ([`crate::rebrief`]). **In memory only.**
    briefs: crate::rebrief::Sent,
    /// **The one lock a dispatch is decided under** (#1436): held from reading where the asking
    /// chat stands until the new chat's slot is reserved, and across a report's "is one owed"
    /// and its "one was sent". Asks arrive a thread each, so without it two in flight would
    /// both read the counts the other is about to change. Never held across a chat's start.
    dispatching: Mutex<()>,
    /// The persona chats that are starting (#1436), by the number each was dealt, as the
    /// record each will have: decided, not yet open, and counted as if they were. Each is
    /// taken out when its start ends, whichever way ([`Reserved`]). **In memory only.**
    reserved: Mutex<HashMap<u32, Chat>>,
    /// The persona each chat that is starting runs as, from its announcement until it is open
    /// or its start fails (qw381): what a chat whose start outlasts the wait for its record is
    /// held to before purlis names one of its MCP servers. **In memory only.**
    starting_as: Mutex<HashMap<u32, Option<String>>>,
    /// The chats whose close stops every chat at work below them (#1443): the person's "Stop
    /// them", given as a Smart close began, kept for the close that ends it. **In memory only.**
    stops_below: Mutex<std::collections::HashSet<u32>>,
    /// What each chat is owed once its turn ends (#1342): a restart on its conversation, with
    /// each sentence it is to be told, in order, as its first message. Queued, so a second
    /// grant before the restart adds to the first rather than replacing it.
    owed: Mutex<HashMap<u32, Vec<String>>>,
    /// The chats being started again in their place right now (#1342): one restart at a time
    /// per chat, whichever asked for it.
    restarting: Mutex<std::collections::HashSet<u32>>,
    /// What each chat was last told of where it is working (#1450), by its number: who its
    /// siblings and its persona's other chats were then, so a later turn is told only what
    /// changed ([`Self::working`]). **Here, beside the app's record of the chat, and in
    /// memory only**: never a file a chat could write, and a chat started again is briefed
    /// afresh.
    told: Mutex<HashMap<u32, purlis_core::awareness::Told>>,
    /// The model each chat's own harness said its program runs on, by the chat's number
    /// (#1021): what the record's `model` says while that program runs ([`Self::heard_model`]).
    /// Apart from [`Self::open`] because a harness reports its start at its own exec, which can
    /// come before the chat is listed as open. Forgotten as each start is announced and as the
    /// chat closes, so a model never outlives the program it was reported for.
    models: Mutex<HashMap<u32, String>>,
    /// The folders a discard is taking away now, and the folders chats are starting in
    /// (#1472): no chat starts in a folder that is going, and none goes while one starts there.
    /// **In memory only.**
    going: crate::goingaway::Going,
    /// What decides the sandbox of a chat on no profile in a test, in place of
    /// [`Self::sandbox_off_profile`]: that asks this machine and checks the real program, and a
    /// test's stand-in harness lives where a chat can write, so it could never start sandboxed.
    #[cfg(test)]
    deciding: Option<Deciding>,
}

/// [`Chats::deciding`]: the chat, what it was granted, and a person's opt-out.
#[cfg(test)]
type Deciding = Box<
    dyn Fn(
            &Chat,
            &purlis_core::sandbox::grant::Grants,
            Option<&purlis_core::sandbox::OptOut>,
        ) -> Result<Decided, String>
        + Send
        + Sync,
>;

/// The chats of a project still running under an older sandbox (#1428), for the Notice after
/// a sandbox setting changes: those that run under a sandbox other than the one a start would
/// compile for them now. The window is sent this type itself (`chats_on_older_sandbox`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct OlderSandbox {
    /// The chats, by session, lowest first.
    pub chats: Vec<OnOlderSandbox>,
}

#[cfg(test)]
impl OlderSandbox {
    /// The chats, by session, lowest first.
    pub fn sessions(&self) -> Vec<u32> {
        self.chats.iter().map(|one| one.session).collect()
    }
}

/// One chat still running under an older sandbox ([`OlderSandbox`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct OnOlderSandbox {
    pub session: u32,
    /// What stands for what the project's settings decide of this chat's sandbox now
    /// ([`Settled::key`]): a dismissal of the Notice is kept for each chat by it, so the
    /// Notice is shown once for each change, and one chat restarting does not bring it back
    /// for another. It is kept on disk with a dismissal, so it is the same on every build of
    /// purlis.
    pub change: String,
    /// **Where this machine's policy is the cause** (#1431): the chat runs without the sandbox,
    /// and policy now requires it. "The sandbox is now required by this machine's policy, set
    /// by <who> in <file>.", in the policy's own words ([`purlis_core::sandbox::policy::Locks::led_by`]).
    /// None where the project's settings moved it.
    #[specta(optional)]
    pub policy: Option<String>,
}

/// **What a project's settings decide of a chat's sandbox** (#1428, D-1428-10): the hosts it
/// may reach, what the presets widen, and the folders every chat may write. The Notice after a
/// sandbox setting changes compares this alone. What a start finds by walking the project's
/// tree (a clone's hooks folder, a script a harness's config runs) is denied as it is found
/// and is not in it: an agent making a clone or running an install is not the project's
/// sandbox changing.
///
/// Each list is sorted, so the same settings written in another order are the same.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Settled {
    hosts: Vec<String>,
    /// What the project's package caches let a chat write, where a preset widens to them.
    caches: Vec<std::path::PathBuf>,
    /// Whether a chat may ask the system's certificate check.
    trust: bool,
    writable: Vec<std::path::PathBuf>,
    /// The local sockets a chat may connect to, and the system services it may look up by
    /// name (ADR 0093): this chat's and the person's, for this project or for every project
    /// on this machine. One allowed since a chat started reaches it only when it restarts.
    sockets: Vec<std::path::PathBuf>,
    services: Vec<String>,
    /// The services on this Mac a chat may connect to, each by its port (qw384).
    ports: Vec<u16>,
    /// The ports of this Mac a chat may listen on (#2019).
    listens: Vec<u16>,
}

/// **What of a running chat its sandbox is compiled from now** ([`Chats::on_older_sandbox`]):
/// two chats alike in all of it compile to the same sandbox in one ask (#2157).
#[derive(Debug, PartialEq, Eq)]
struct StartKey {
    harness: purlis_core::harness::Harness,
    /// The persona whose grants it takes.
    persona: Option<String>,
    /// The persona the person's own level is read for.
    level_as: Option<String>,
    made: purlis_core::sandbox::Made,
    grants: purlis_core::sandbox::grant::Grants,
}

/// What a start would compile now, for [`StartKey`]: the sandbox it settles to, none at all,
/// or no answer, where it could not be compiled.
#[derive(Debug, Clone)]
enum CompiledNow {
    Sandboxed(Settled),
    Unconfined,
    Refused,
}

impl Settled {
    fn of(confines: &purlis_core::sandbox::Confines) -> Self {
        let sorted = |mut list: Vec<std::path::PathBuf>| {
            list.sort();
            list.dedup();
            list
        };
        let mut hosts = confines.hosts.clone();
        hosts.sort();
        hosts.dedup();
        Self {
            hosts,
            caches: sorted(
                confines
                    .widened
                    .caches
                    .as_ref()
                    .map(purlis_core::sandbox::caches::CacheHome::writable)
                    .unwrap_or_default(),
            ),
            trust: confines.widened.trust,
            writable: sorted(confines.writable.clone()),
            sockets: sorted(confines.widened.sockets.clone()),
            services: {
                let mut services = confines.widened.services.clone();
                services.sort();
                services.dedup();
                services
            },
            // As granted, not as this Mac was at the compile (#2019): a port given for a listen
            // grant alone, and whether something listened on it then (the chat's own door, once
            // it serves), change no grant.
            ports: {
                let widened = &confines.widened;
                let mut ports: Vec<u16> = widened
                    .ports
                    .iter()
                    .copied()
                    .filter(|port| !widened.listen_only.contains(port))
                    .collect();
                ports.sort_unstable();
                ports.dedup();
                ports
            },
            listens: {
                let widened = &confines.widened;
                let mut listens = widened.listens.clone();
                listens.extend(&widened.unserved);
                listens.sort_unstable();
                listens.dedup();
                listens
            },
        }
    }

    /// What stands for `settled`, or for a start with no sandbox: a SHA-256 over one canonical
    /// text of it, so it does not move with the toolchain as a `Debug` text or the standard
    /// hasher may.
    fn key(settled: Option<&Self>) -> String {
        use sha2::Digest as _;
        // One line for each entry, under a heading, with a NUL after each: no entry can read
        // as another list's, or as two.
        let mut text = String::new();
        let mut list = |heading: &str, entries: &mut dyn Iterator<Item = String>| {
            text.push_str(heading);
            text.push('\0');
            for entry in entries {
                text.push_str(&entry);
                text.push('\0');
            }
        };
        match settled {
            None => list("no sandbox", &mut std::iter::empty()),
            Some(settled) => {
                let paths = |paths: &[std::path::PathBuf]| -> Vec<String> {
                    paths
                        .iter()
                        .map(|path| path.to_string_lossy().into_owned())
                        .collect()
                };
                list("hosts", &mut settled.hosts.iter().cloned());
                list("caches", &mut paths(&settled.caches).into_iter());
                list(
                    "certificate checks",
                    &mut std::iter::once(settled.trust.to_string()),
                );
                list("writable", &mut paths(&settled.writable).into_iter());
                list("sockets", &mut paths(&settled.sockets).into_iter());
                list("services", &mut settled.services.iter().cloned());
                // Only where there is one, so what stood for a sandbox with none stays as it
                // was before ports were allowed (qw384).
                if !settled.ports.is_empty() {
                    list("ports", &mut settled.ports.iter().map(u16::to_string));
                }
                // Likewise ports to listen on (#2019).
                if !settled.listens.is_empty() {
                    list("listens", &mut settled.listens.iter().map(u16::to_string));
                }
            }
        }
        let digest = sha2::Sha256::digest(text.as_bytes());
        digest[..8]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

/// `ready` with `told` as the chat's first message where there is one, **by the route its
/// harness takes one** (`purlis_core::handoff::first_message_argv`): last on the line, as a
/// handoff's brief is, since nothing may come after a positional prompt. A harness purlis has
/// not measured the first message of is started without it.
fn told_first(
    mut ready: purlis_core::start::Ready,
    told: Option<&str>,
) -> purlis_core::start::Ready {
    if let Some((told, first)) = told.and_then(|told| {
        ready
            .harness
            .and_then(|harness| purlis_core::handoff::first_message_argv(harness.name(), told))
            .map(|first| (told, first))
    }) {
        ready.args.extend(first);
        ready.first_message = Some(told.to_owned());
    }
    ready
}

/// Why chat `session` was not restarted when purlis holds no conversation to resume it by, and
/// what to do (#1428). True of a chat that has not said which conversation it is in yet, and of
/// one whose harness never says: the first way out is for the one, the second for both.
fn no_conversation(session: u32) -> String {
    format!(
        "purlis did not restart chat {session}: it has no conversation to resume. If it has \
         only just started, send it a message and restart it again. Start fresh on its tab's \
         menu starts it again without a conversation."
    )
}

/// What a chat that ran without the sandbox is told on its tab when a restart puts it back in
/// one (#1428): a person's opt-out is for one start, never inherited (ADR 0067 §7).
pub const SANDBOXED_AGAIN: &str = "This chat ran without the sandbox until this restart. It runs \
                                   in the sandbox now, because that choice lasts for one start.";

/// **A grant reserved for one chat and not kept yet** ([`Chats::reserve_grant`], #2454): it
/// is held by nothing but this, so no start, no Granted list, no live ask and no later Allow
/// sees it until [`Chats::commit_grant`] keeps it, once its audit is written. Dropped, it was
/// never kept.
#[derive(Debug)]
pub(crate) struct ReservedGrant {
    session: u32,
    /// The chat's id, which its grants are kept by.
    id: String,
    what: purlis_core::sandbox::grant::What,
    at: u64,
    /// The chat's name as its tab showed it.
    chat: String,
    once: bool,
    already: bool,
    /// How many times the chat's grants had been taken back when it was reserved
    /// ([`Chats::revoke`], a close that ends them): one taken back since keeps it from being
    /// kept.
    taken_back: u64,
}

impl ReservedGrant {
    /// Whether the chat held it already, at least as long, when it was reserved: then keeping
    /// it changes nothing, and nothing is audited.
    pub(crate) fn already(&self) -> bool {
        self.already
    }
}

/// One grant a person made for one chat (#1342), as Settings' Granted list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatGrant {
    pub what: purlis_core::sandbox::grant::What,
    /// When, in seconds since 1970.
    pub at: u64,
    /// The chat's name as its tab showed it then.
    pub chat: String,
    /// Allowed once (D-sock-3): for the run that takes it alone. It goes once the session it
    /// was allowed in ends, which a restart to take it ends after it starts.
    pub once: bool,
    /// The session it was allowed in.
    pub from: u32,
}

/// A chat's sandbox, as a start decided it: compiled for its harness with the real program it
/// then runs (ruling V87g), or lifted and why — never both, and neither where its project has
/// not turned the sandbox on.
type Decided = (
    Option<(purlis_core::sandbox::Applied, String)>,
    Option<purlis_core::sandbox::Lifted>,
    // Where the floor could not be applied, the sentence its tab says (#2071, Q2).
    Option<String>,
);

/// What a chat that was closed was, kept for a start in its place
/// ([`Chats::start_ready_instead_of`]).
#[derive(Debug, Clone)]
struct LetGo {
    identity: purlis_core::reopen::Identity,
    /// The chat that dispatched it, and where it stood in its lineage.
    from: Option<purlis_core::reopen::HandedFrom>,
    /// The task it was named for.
    label: Option<String>,
    /// Whether it is a finished task the person reopened (#1543).
    reopened: Option<purlis_core::reopen::ReopenedTask>,
}

impl LetGo {
    fn of(chat: &Chat) -> Self {
        Self {
            identity: chat.identity.clone(),
            from: chat.from.clone(),
            label: chat.label.clone(),
            reopened: chat.reopened,
        }
    }
}

impl Chats {
    /// Chats whose record is written by `record_it` every time what is open changes, in no
    /// project: what the tests use when the project is not what they are about.
    ///
    /// Quitting writes it too, but only a graceful quit reaches that: an app that is killed,
    /// or crashes, runs no exit handler. Writing as it goes means such an app comes back on
    /// the chats it had rather than on none.
    #[cfg(test)]
    pub fn recorded_by(record_it: Recorder) -> Self {
        Self::recorded_by_reporting_to(record_it, None)
    }

    /// The same, with sessions that report what their harness does to `reporting`'s socket.
    #[cfg(test)]
    pub fn recorded_by_reporting_to(record_it: Recorder, reporting: Option<Reporting>) -> Self {
        Self::on_host(
            record_it,
            Box::new(Sessions::reporting_to(reporting)),
            tests::no_project(),
        )
    }

    /// Chats of the project at `project`, whose record `record_it` writes, on `host` —
    /// whatever runs the sessions, which is [`crate::sessions::Sessions`] in the app.
    ///
    /// **`project` is the open project's own root, as the app holds it** (#1410): every chat
    /// started here takes its project, and so its sandbox, from it — never from a walk up
    /// from the chat's folder, which a link or a planted manifest can steer.
    pub fn on_host(record_it: Recorder, host: Box<dyn SessionHost>, project: PathBuf) -> Self {
        Self {
            project,
            sessions: host,
            starting: Mutex::new(None),
            reattached: Mutex::new(None),
            beginning: Mutex::new(None),
            router: Mutex::new(None),
            routed: Mutex::new(None),
            settled: Mutex::new(None),
            trusting: Mutex::new(None),
            late_notes: Mutex::new(HashMap::new()),
            left_out_told: Mutex::new(std::collections::HashSet::new()),
            skills_missing: Mutex::new(HashMap::new()),
            project_settings: crate::projectsettings::Watch::new(),
            sizes: Mutex::new(HashMap::new()),
            replacing: Mutex::new(std::collections::HashSet::new()),
            floorless: Mutex::new(HashMap::new()),
            untaken: Mutex::new(HashMap::new()),
            older_in_notes: Mutex::new(HashMap::new()),
            device: None,
            pools_in: None,
            profiles_on_pools: None,
            let_go: Mutex::new(HashMap::new()),
            never_started: Mutex::new(None),
            shipped: crate::Shipped::default(),
            open: Mutex::new(HashMap::new()),
            ended: Mutex::new(std::collections::HashSet::new()),
            front: Mutex::new(None),
            would_not_start: Mutex::new(Vec::new()),
            coming: Mutex::new(Vec::new()),
            gate: crate::restoring::Gate::shared(),
            recorded_order: Mutex::new(Vec::new()),
            views: Mutex::new(Vec::new()),
            focus: Mutex::new(None),
            order: Mutex::new(Vec::new()),
            record_it,
            writing: Mutex::new(()),
            ending: AtomicBool::new(false),
            most_at_once: MOST_AT_ONCE,
            grants: Mutex::new(HashMap::new()),
            taken_back: Mutex::new(HashMap::new()),
            kubeconfigs: Mutex::new(HashMap::new()),
            blocks: Arc::default(),
            asks_moved: Arc::new(Mutex::new(None)),
            refused: Arc::new(Mutex::new(None)),
            refused_more: Arc::new(Mutex::new(None)),
            reached: Arc::new(Mutex::new(None)),
            network_words: Mutex::new(None),
            narrowed: Mutex::new(None),
            records: Mutex::new(HashMap::new()),
            briefs: crate::rebrief::Sent::default(),
            dispatching: Mutex::new(()),
            reserved: Mutex::new(HashMap::new()),
            starting_as: Mutex::new(HashMap::new()),
            stops_below: Mutex::new(std::collections::HashSet::new()),
            owed: Mutex::new(HashMap::new()),
            restarting: Mutex::new(std::collections::HashSet::new()),
            told: Mutex::new(HashMap::new()),
            models: Mutex::new(HashMap::new()),
            going: crate::goingaway::Going::default(),
            #[cfg(test)]
            deciding: None,
        }
    }

    /// These chats, with `deciding` deciding the sandbox of each chat on no profile.
    #[cfg(test)]
    fn deciding_by(self, deciding: Deciding) -> Self {
        Self {
            deciding: Some(deciding),
            ..self
        }
    }

    /// These chats, of the project at `root` instead.
    #[cfg(test)]
    fn in_project(self, root: &Path) -> Self {
        Self {
            project: root.to_path_buf(),
            ..self
        }
    }

    /// These chats, starting at most `most` from a record rather than [`MOST_AT_ONCE`].
    #[cfg(test)]
    fn starting_at_most(self, most: usize) -> Self {
        Self {
            most_at_once: most,
            ..self
        }
    }

    /// Chats nothing records — what the tests use when the record is not what they are about.
    #[cfg(test)]
    pub fn new() -> Self {
        Self::recorded_by(Box::new(|_| {}))
    }

    /// Calls `tell` as each chat starts, BEFORE its program does, with everything the board
    /// needs in order to judge a report about it.
    pub fn when_one_starts(&self, tell: Starting) {
        *lock(&self.starting) = Some(tell);
    }

    /// Calls `tell` as each chat put back is reattached in the session host rather than
    /// started (#2006), before anything else is told of it, with what the board needs to judge
    /// a report about it where it does not know the chat yet.
    pub fn when_one_is_reattached(&self, tell: Starting) {
        *lock(&self.reattached) = Some(tell);
    }

    /// Calls `tell` as each chat starts, BEFORE its program does, with the run that start
    /// begins: the chat's id, which a relaunch keeps, a new run's id, and why (ADR 0066).
    pub fn when_a_run_begins(&self, tell: Beginning) {
        *lock(&self.beginning) = Some(tell);
    }

    /// Asks a chat's router extension at its start with `router` (SK-15b, ADR 0102 §5).
    pub fn asks_routers_with(&self, router: Arc<dyn purlis_core::skills::router::AskRouter>) {
        *lock(&self.router) = Some(router);
    }

    /// Calls `tell` with each chat's routing record as its run begins, where its start asked a
    /// router extension (SK-15b): after the run's own `run.started` is told; and again for a
    /// chat reattached, whose router its start asked again.
    pub fn when_a_chat_is_routed(&self, tell: Routed) {
        *lock(&self.routed) = Some(tell);
    }

    /// Calls `tell` with the number of a chat whose run began [`RunSaid::Held`] once its project
    /// settings were screened at its start: whether the start goes on or was put aside (#2250,
    /// #2252).
    pub fn when_a_held_start_is_settled(&self, tell: Box<dyn Fn(u32, HeldStart) + Send + Sync>) {
        *lock(&self.settled) = Some(tell);
    }

    /// Calls `tell` with what each start decided about the chat's sandbox: its trust event as
    /// its run begins, before its program runs, and how a new chat counts once it has started
    /// (ADR 0067 §7, ruling V78).
    pub fn when_the_sandbox_is_decided(&self, tell: Trusting) {
        *lock(&self.trusting) = Some(tell);
    }

    /// Whether `session` is a shell tab at `root`: a chat on no profile running no harness,
    /// in that directory. Where SD-30's install command may be typed (ruling V78 c), never an
    /// agent's pane.
    pub fn is_shell_at(&self, session: u32, root: &std::path::Path) -> bool {
        let same = |a: &std::path::Path, b: &std::path::Path| {
            a == b || matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
        };
        lock(&self.open).get(&session).is_some_and(|one| {
            one.chat.profile.is_none()
                && one.harness.is_none()
                && one.chat.cwd.as_deref().is_some_and(|cwd| same(cwd, root))
        })
    }

    /// **Whether `session` is a shell tab at `root` whose program still runs** (#4202): the tab
    /// of a shell that ran `exit` stays open, with nothing in it.
    pub fn runs_a_shell_at(&self, session: u32, root: &std::path::Path) -> bool {
        self.is_shell_at(session, root) && !lock(&self.ended).contains(&session)
    }

    /// The skills chat `session`'s persona names that it started without (SK-5, #2768), until
    /// the Notice that says them is put away.
    pub fn skills_missing(&self, session: u32) -> Option<SkillsMissing> {
        lock(&self.skills_missing).get(&session).cloned()
    }

    /// Puts chat `session`'s missing-skills Notice away, for this run of it.
    pub fn put_away_skills_missing(&self, session: u32) {
        lock(&self.skills_missing).remove(&session);
    }

    /// What chat `session`'s start found to say on its tab beyond the core's notices, once.
    /// An older Claude Code's sentence among them is said, from now on, for this process
    /// (#1699): marked here, as the tab shows it, so a chat that never reached its tab does not
    /// use the one telling up.
    pub fn start_notes(&self, session: u32) -> Vec<String> {
        if let Some(version) = lock(&self.older_in_notes).remove(&session) {
            purlis_core::sandbox::older_shown(version);
        }
        lock(&self.late_notes).remove(&session).unwrap_or_default()
    }

    /// Says which device this is: the origin device of every chat minted from now on.
    pub fn on_device(&mut self, device: Option<String>) {
        self.device = device;
    }

    /// Reads where each workspace runs its chats from this machine's config home `config`
    /// (#2679): a workspace whose chats run on a pool starts none here.
    pub fn keeping_pools_in(&mut self, config: Option<PathBuf>) {
        self.profiles_on_pools = config.clone().map(|config| {
            Arc::new(crate::poolprofiles::Linked::at(config))
                as Arc<dyn crate::poolprofiles::ProfilesOnPools>
        });
        self.pools_in = config;
    }

    /// **A runner's refusal of a signed-out profile, in its pool's words** (#4233): a start let
    /// through while the runner fetched its harness is refused by the runner as a machine of its
    /// own, whose words send the person to sign in on the wrong one. The row is asked again, which
    /// raises the pool's own wait; any other refusal is as it was said.
    fn in_the_pools_words(
        &self,
        why: String,
        profile: Option<&str>,
        workspace: Option<&str>,
    ) -> String {
        if !purlis_core::profilestore::sign_in::is_refusal(&why) {
            return why;
        }
        let (Some(on_pools), Some(profile), Some(pool)) = (
            &self.profiles_on_pools,
            profile,
            self.sessions.pool_for(workspace),
        ) else {
            return why;
        };
        match on_pools.ready(&self.project, &pool, profile) {
            Err(words) => words,
            Ok(_) => purlis_core::profilestore::onrunner::signed_out_on_the_runner(&pool, profile),
        }
    }

    /// Readies each chat's profile on the pool it starts on through `on_pools` (#2049).
    #[cfg(test)]
    pub fn readying_profiles_on_pools(
        &mut self,
        on_pools: Arc<dyn crate::poolprofiles::ProfilesOnPools>,
    ) {
        self.profiles_on_pools = Some(on_pools);
    }

    /// **Keeps `profile` on pool `pool`'s runner, signed in there** (#2049), as a start there
    /// does first: what a chat moving there asks before its conversation goes (#2048), and the
    /// sign-in it waits on where it is signed out there (#3477). `Ok` where this machine keeps no
    /// pool to ready it on.
    pub fn ready_profile_on(
        &self,
        pool: &str,
        profile: &str,
    ) -> Result<(), purlis_core::profilestore::onrunner::NotYet> {
        match &self.profiles_on_pools {
            Some(on_pools) => on_pools.ready_to_move(&self.project, pool, profile),
            None => Ok(()),
        }
    }

    /// This machine's config home, where the pools it keeps are, where it keeps any (#2050).
    pub fn pools_in(&self) -> Option<PathBuf> {
        self.pools_in.clone()
    }

    /// This device's id, as [`Self::on_device`] gave it, or `None` where the machine store has
    /// none. It names the work link log a chat's link goes in (ADR 0088 §3).
    pub fn device(&self) -> Option<&str> {
        self.device.as_deref()
    }

    /// Who the chat in `session` is and where it works, or `None` for a session charter does
    /// not have open.
    pub fn chat_at(&self, session: u32) -> Option<ChatAt> {
        let open = lock(&self.open);
        let one = open.get(&session)?;
        Some(ChatAt {
            id: one.chat.identity.id.clone(),
            cwd: one.chat.cwd.clone(),
        })
    }

    /// **The chats open here, each with its id, its name and the workspace it works in**, in
    /// the order of their numbers (#2332): whose theme drafts a workspace draws, and what the
    /// draft strip and the close of a chat name. A chat that has no id has no drafts.
    pub fn drafting(&self) -> Vec<Drafting> {
        let open = lock(&self.open);
        let mut out: Vec<Drafting> = open
            .iter()
            .filter_map(|(session, one)| {
                Some(Drafting {
                    session: *session,
                    id: one.chat.identity.id.clone()?,
                    name: one.chat.name.clone(),
                    workspace: one.workspace.clone(),
                })
            })
            .collect();
        out.sort_by_key(|one| one.session);
        out
    }

    /// For tests: the open chat `session` as one whose record holds no id.
    #[cfg(test)]
    pub fn forget_id_of(&self, session: u32) {
        if let Some(one) = lock(&self.open).get_mut(&session) {
            one.chat.identity.id = None;
        }
    }

    /// Calls `tell` when a chat that was announced never started after all.
    ///
    /// The announcement has to come before the program, so a program that then fails to start
    /// leaves the board holding a chat that does not exist. Nothing is misattributed — ids are
    /// never reused — but it is one entry per failed start for the life of the app.
    pub fn when_one_does_not_start(&self, tell: Box<dyn Fn(u32) + Send + Sync>) {
        *lock(&self.never_started) = Some(tell);
    }

    /// Refuses every chat start while `switch` is thrown (OV-1), at the sessions every start
    /// reaches.
    pub fn stopped_by(&mut self, switch: std::sync::Arc<crate::killswitch::KillSwitch>) {
        self.sessions.stopped_by(switch);
    }

    /// The sessions underneath, for everything that is about a terminal and not about a chat.
    pub fn sessions(&self) -> &dyn SessionHost {
        self.sessions.as_ref()
    }

    /// What the app ships that a chat is armed with: the `charter` binary a hook runs and the
    /// plugin a Claude Code chat loads, when the app knows where each is.
    ///
    /// Its own, because the app and both ship together: the `charter` on `PATH` may be an
    /// older install, or the Python charter, and a hook pointed at either would be answering a
    /// different program's idea of these events.
    pub fn arming_with(&mut self, shipped: crate::Shipped) {
        self.shipped = shipped;
    }

    /// The arguments and the environment that arm this harness on this session alone, if any.
    ///
    /// `cwd` is the chat's own directory, which decides whether charter may also fill Claude
    /// Code's status line for it (`purlis_core::footerclaim`): project settings are read from
    /// the session's own directory, so that is the directory the question is asked about.
    ///
    /// `plugins` is the harness's own plugins the project chose for this chat
    /// (`purlis_core::start::Ready::plugins`, charter-app#274); empty for a chat on no profile.
    ///
    /// `sandbox` is the sandbox the core compiled for this chat (ADR 0067), or none.
    ///
    /// `persona` is the persona the chat runs as, which a harness that can is handed the MCP
    /// servers and the denied tools of (#1451, `purlis_core::personaverbs::chatstart`).
    ///
    /// `graded` is whether it is a grade's chat (SK-16), as its record says: what holds it to
    /// purlis's server and read tools alone (`purlis_core::hoststart::Arming::graded`).
    ///
    /// `skills` is where its plane skills come from, read once at its start
    /// (`purlis_core::hoststart::ChatSkills::read`, #3234), as its tab's Notice reads them: read
    /// only once the arming's fail-closed checks have passed (#3461).
    ///
    /// **Fail closed.** A sandbox compiled for another harness, or one the harness is not armed
    /// to carry here (the app shipped without its plugin or its binary), refuses the chat rather
    /// than starting it without the sandbox.
    #[allow(clippy::too_many_arguments)]
    fn state_hooks(
        &self,
        harness: Option<Harness>,
        cwd: Option<&std::path::Path>,
        plugins: &purlis_core::harness_plugin::Chosen,
        sandbox: Option<&purlis_core::sandbox::Applied>,
        persona: Option<&str>,
        env: &[(String, String)],
        graded: bool,
        skills: &dyn std::ops::Deref<Target = purlis_core::hoststart::ChatSkills>,
    ) -> Result<Armed, String> {
        purlis_core::hoststart::armed(&purlis_core::hoststart::Arming {
            root: &self.project,
            kit: &self.kit(harness),
            harness,
            cwd,
            plugins,
            sandbox,
            persona,
            env,
            graded,
            skills,
        })
    }

    /// The sandbox a chat that is not on a profile starts under: the same decision
    /// `purlis_core::start::ready` makes for one that is (ADR 0067). A chat whose program is a
    /// harness, in a project that turned the sandbox on, is sandboxed or refused. A shell, or a
    /// chat in a project that leaves the sandbox off, is the operator's own and is left as it
    /// was. With the sandbox, the program the chat then runs: the real file the check asked
    /// about (ruling V87g), never the name it was recorded by.
    ///
    /// **The project is the open one's, [`Self::project`]** (#1410), as for a chat on a profile,
    /// never one found by walking up from the chat's folder; see [`project_sandbox`].
    ///
    /// `opt_out` is a person's choice to run it without the sandbox for this run, made in the
    /// window on a sandbox block's Notice (#1342), and none for every other start. A system with
    /// no backend (Windows) starts it unsandboxed, and says why, as it does a chat on a profile.
    /// `grants` are what a person let this one chat do besides.
    fn sandbox_off_profile(
        &self,
        chat: &Chat,
        grants: &purlis_core::sandbox::grant::Grants,
        opt_out: Option<&purlis_core::sandbox::OptOut>,
        made: &purlis_core::sandbox::Made,
    ) -> Result<Decided, String> {
        let Some(harness) = chat.harness() else {
            return Ok((None, None, None));
        };
        let root = &self.project;
        let decided = project_sandbox(
            harness,
            root,
            chat.cwd.as_deref(),
            &purlis_core::sandbox::Machine::this(),
            &purlis_core::sandbox::backend::installed,
            // A chat on no profile has no persona: its level is the project's.
            &purlis_core::sandbox::Person {
                opt_out,
                level_as: level_as_of(chat),
                made,
            },
            grants,
        )?;
        let (applied, lifted, floorless) = match decided {
            Some(purlis_core::sandbox::Decided::Sandboxed(applied)) => (Some(applied), None, None),
            Some(purlis_core::sandbox::Decided::Unsandboxed(lifted)) => (None, Some(lifted), None),
            // Where the floor cannot be applied the chat starts as before, and says why (#2071,
            // Q2).
            Some(purlis_core::sandbox::Decided::Floorless(why)) => (None, None, Some(why)),
            None => (None, None, None),
        };
        // Ruling V87g: the program is the harness the sandbox was compiled for, and not a file
        // a sandboxed chat could have changed. The real file it names is what the check asks;
        // a chat whose program then runs by another name would not be the one checked. A chat
        // that starts without the sandbox is bound to none.
        if let Some(applied) = &applied {
            let launch = chat.launch();
            let words = purlis_core::programs::resolve_argv(std::slice::from_ref(&launch.program))
                .map_err(|gone| format!("{} Nothing was started.", gone.said()))?;
            let cwd = chat.cwd.clone().unwrap_or_else(|| root.clone());
            let mut writable = applied.writable();
            // At the floor a program outside the project is held unwritten instead (#2071, Q4).
            if applied.level() == purlis_core::sandbox::Level::Full {
                writable.extend(purlis_core::sandbox::program::temp_roots(&[]));
            }
            let (checked, said) = purlis_core::sandbox::program::checked_answering(
                harness,
                &words,
                applied.root(),
                purlis_core::sandbox::program::Chat {
                    cwd: &cwd,
                    writable: &writable,
                    env: &[],
                },
                None,
            )
            .map_err(|refused| refused.to_string())?;
            // Claude Code through purlis's proxy from the version that takes its ports
            // (#1665); an older one keeps its own, and says so once as it opens.
            let mut applied = applied.clone();
            if let Some(said) = said {
                applied.answered(&said);
            }
            // At the floor, the program it starts is never written by the chat (#2071, Q4).
            applied.hold_programs(&checked);
            // And on Linux, purlis's own program the wrap starts it under, the same (#1040), which
            // a brokered `secret exec` for the chat runs its command in too (#2342).
            applied
                .hold_wrap(purlis_core::sandbox::program::Chat {
                    cwd: &cwd,
                    writable: &writable,
                    env: &[],
                })
                .map_err(|refused| refused.to_string())?;
            return Ok((Some((applied, checked[0].clone())), None, None));
        }
        Ok((None, lifted, floorless))
    }

    /// Starts a chat the core has already worked out the launch for — a chat on a profile.
    ///
    /// The harness, the arguments and the environment all come from `ready`, which resolved
    /// them from the profile's DECLARED kind. Nothing here asks the program's name what it
    /// is: a profile's command is commonly a wrapper, and the answer would be `None`.
    pub fn start_ready(
        &self,
        chat: &Chat,
        ready: &purlis_core::start::Ready,
        size: Size,
    ) -> Result<u32, String> {
        self.start_ready_as(chat, ready, size, Why::New)
    }

    /// [`Self::start_ready`], in the place of chat `instead_of`, whose harness could not bring
    /// its conversation back (`lostOnResume`): **the same chat**, under its id, in a run that
    /// begins `fresh` (ADR 0066). `instead_of` may already be closed, as the window closes its
    /// tab first. A chat this launch never had is started as a new one.
    pub fn start_ready_instead_of(
        &self,
        instead_of: u32,
        chat: &Chat,
        ready: &purlis_core::start::Ready,
        size: Size,
    ) -> Result<u32, String> {
        // Taken out of what is recorded here, whether or not the window's close has arrived
        // yet: the two are not ordered, and a record holding both would name two chats by one
        // id (#856 review F3). Its program is the close's to end.
        let taken = lock(&self.open).remove(&instead_of);
        if let Some(taken) = &taken {
            lock(&self.let_go).insert(instead_of, LetGo::of(&taken.chat));
        }
        let was = taken
            .map(|one| LetGo::of(&one.chat))
            .or_else(|| lock(&self.let_go).get(&instead_of).cloned());
        let Some(was) = was.filter(|was| was.identity.id.is_some()) else {
            return self.start_ready(chat, ready, size);
        };
        let again = Chat {
            identity: purlis_core::reopen::Identity {
                run: None,
                ..was.identity
            },
            // **It is the same chat, so it stands where it stood** (#1436): the chat that
            // dispatched it, what it owes that chat, how deep it is and which lineage it is
            // in, and the task it is called by. Without them a dispatched chat that lost its
            // conversation would come back as one the person started: at depth 0, in a lineage
            // of its own, owing nobody a report.
            from: was.from.or_else(|| chat.from.clone()),
            label: was.label.or_else(|| chat.label.clone()),
            // And a reopened task is still one, held at its next start as it was at this one.
            reopened: was.reopened.or(chat.reopened),
            ..chat.clone()
        };
        // **A dispatched chat is handed its brief again** (#1609): it starts with no
        // conversation, and the brief was the whole of what it was asked.
        let told = self.told_again(&again);
        let ready = told_first(ready.clone(), told.as_deref());
        self.start_ready_as(&again, &ready, size, Why::Again)
    }

    /// The dispatch of `record`, as the store wrote it, started its chat on `brief` (#1609):
    /// kept, as a digest of it and of what frames it, for a start of that chat again with no
    /// conversation ([`crate::rebrief`]).
    pub(crate) fn brief_was_sent(&self, record: &purlis_core::dispatchrecord::Record, brief: &str) {
        self.briefs.note(record, brief);
    }

    /// What `chat` is told as it starts again with no conversation (#1609): its brief, where a
    /// dispatch started it and the brief can be handed, else why not. `None` for a chat no
    /// dispatch started.
    fn told_again(&self, chat: &Chat) -> Option<String> {
        self.again_told(chat).map(|again| again.message)
    }

    /// What `chat`, put back by a launch or by Retry now, is told as it starts: what the launch
    /// says (`told`), or else, for a chat its record names no conversation for, what a fresh
    /// start of a dispatched chat is told ([`Self::told_again`], #1609): it starts with no
    /// conversation, so its brief would otherwise be gone.
    fn told_put_back(&self, chat: &Chat, told: Option<&str>) -> Option<String> {
        told.map(str::to_owned).or_else(|| {
            chat.resume
                .is_none()
                .then(|| self.told_again(chat))
                .flatten()
        })
    }

    /// [`Self::told_again`], with whether it hands the chat its brief.
    fn again_told(&self, chat: &Chat) -> Option<crate::rebrief::Again> {
        crate::rebrief::again(&self.project, chat, &self.briefs)
    }

    fn start_ready_as(
        &self,
        chat: &Chat,
        ready: &purlis_core::start::Ready,
        size: Size,
        why: Why,
    ) -> Result<u32, String> {
        let session = self.open_it(
            chat,
            ready.program.clone(),
            ready.command.clone(),
            ready.args.clone(),
            ready.env.clone(),
            ready.harness,
            ready.declared.as_ref(),
            ready.session.as_ref().map(ToString::to_string),
            ready.how.clone(),
            &ready.plugins,
            ready.sandbox.as_ref(),
            ready.unsandboxed.as_ref(),
            size,
            // A chat on a profile is an agent, never the operator's shell.
            false,
            why,
            ready.sign_in.as_ref(),
            ready.first_message.as_deref(),
        )?;
        self.floorless_at(session, ready.floorless.clone());
        Ok(session)
    }

    /// Says why chat `session` runs with no sandbox where the floor could not be applied
    /// (#2071, Q2), or that it does not: each start of a session says it again.
    fn floorless_at(&self, session: u32, why: Option<String>) {
        let mut floorless = lock(&self.floorless);
        match why {
            Some(why) => floorless.insert(session, why),
            None => floorless.remove(&session),
        };
    }

    /// [`Self::put_back`], kept by the name the tests know it by.
    #[cfg(test)]
    fn put_back_here(&self, record: &Record, size: Size) -> Vec<Open> {
        self.put_back(record, size)
    }

    /// Starts one chat out of the record, on its own profile where it had one.
    ///
    /// **The profile is looked up again**, never taken from the record: an edit to it takes
    /// effect at this launch rather than a stale copy running, and a profile that is gone
    /// means this chat is skipped BY NAME — another profile may be another account, where
    /// this chat's resume id does not exist and where its workspace's code was never meant
    /// to go. It stays in the record, so declaring the profile again brings it back.
    ///
    /// `told` is the chat's first message where there is one: a sentence purlis tells it
    /// (#1342), or the brief of a dispatched chat started again with no conversation (#1609),
    /// last on its line, where its harness takes one (`handoff::first_message_argv`). A chat on
    /// no profile is started without it.
    fn start_recorded_told(
        &self,
        chat: &Chat,
        size: Size,
        why: Why,
        told: Option<&str>,
        opt_out: Option<purlis_core::sandbox::OptOut>,
    ) -> Result<u32, String> {
        let started = self.start_recorded_checked(chat, size, why, told, opt_out);
        // **A chat the session host still runs is reattached, never refused** (D-2006-11): what
        // would refuse its start now (its profile gone or no longer allowed, its sandbox or its
        // line) refuses a start, and nothing starts here. It is taken back as its record has it.
        match started {
            Err(refused) if self.reattaches(chat, why) => {
                tracing::info!(
                    "purlis: chat {} is reattached as it runs; a start of it now would be \
                     refused ({refused})",
                    chat.name
                );
                self.reattach_as_recorded(chat, size, why)
            }
            started => started,
        }
    }

    /// Whether `chat`, put back for `why`, is one the session host still runs from before this
    /// window, and so is reattached (#2006).
    fn reattaches(&self, chat: &Chat, why: Why) -> bool {
        why == Why::Relaunch && self.left_running(chat)
    }

    /// Whether `chat` is still in the session host from before this window (#2006): asked of
    /// the host its workspace runs its chats on (#2046).
    pub(crate) fn left_running(&self, chat: &Chat) -> bool {
        let Some(number) = chat.number else {
            return false;
        };
        let workspace = workspace_at_start(Some(&self.project), chat.cwd.as_deref());
        self.sessions.left_running_in(number, workspace.as_deref())
    }

    /// Reattaches `chat` as its record has it, where what a start reads of it now refuses
    /// (D-2006-11): its program and its words as recorded, its harness by its program, no
    /// sandbox this window compiled, and the confinement it runs in.
    fn reattach_as_recorded(&self, chat: &Chat, size: Size, why: Why) -> Result<u32, String> {
        let how = match &chat.resume {
            Some(id) => Reopened::Resumed(id.clone()),
            None => Reopened::Fresh(purlis_core::reopen::Fresh::NoConversationRecorded),
        };
        self.open_it_as(
            chat,
            chat.program.clone(),
            Vec::new(),
            chat.args.clone(),
            Vec::new(),
            chat.harness(),
            None,
            chat.resume.as_ref().map(ToString::to_string),
            how,
            &Default::default(),
            None,
            None,
            size,
            false,
            why,
            None,
            None,
            true,
        )
        .map(|(session, _)| session)
    }

    /// [`Self::start_recorded_told`], refused where what its start reads now refuses.
    fn start_recorded_checked(
        &self,
        chat: &Chat,
        size: Size,
        why: Why,
        told: Option<&str>,
        opt_out: Option<purlis_core::sandbox::OptOut>,
    ) -> Result<u32, String> {
        let Some(profile) = chat.profile.clone() else {
            return self.start_as_opted(chat, size, false, why, opt_out.as_ref());
        };
        // **A chat a dispatch started is held, each time it is started again, to what the
        // dispatch was held to** (#1509): a profile the project lists for its persona, where
        // it lists any, and one whose command does not switch the harness's prompts off. Both
        // may have changed since it was dispatched, and nobody chose the change for this chat.
        purlis_core::dispatchprofiles::may_start_again(&self.project, chat)?;
        let ready = purlis_core::start::ready(
            &purlis_core::start::Start {
                profile: Some(profile),
                persona: chat.persona.clone(),
                name: chat.name.clone(),
                cwd: chat.cwd.clone(),
                resume: chat.resume.clone(),
                // The chat's own footer choice, brought back with it. It rides on the
                // environment, which is rebuilt at every start, so a relaunch that did not
                // carry it would silently blank a footer the operator had turned on.
                show_footer: chat.show_footer,
                resuming: None,
                without_sandbox: opt_out,
                // A handed-off chat keeps holding the asking chat's persona grants across a
                // relaunch until the person allows its own (#1362, D-1362-5).
                held: chat.held.clone(),
                grants: self.grants_of(chat),
                made: made_of(chat, why),
            },
            &self.project,
        )?;
        // The core's start knows no chat, so it says "nothing recorded"; this chat may know
        // better — a workspace rename left it without its conversation (charter#367).
        let ready = purlis_core::start::Ready {
            how: chat.told(ready.how.clone()),
            ..ready
        };
        let ready = told_first(ready, told);
        self.start_ready_as(chat, &ready, size, why)
    }

    /// What a person let `chat` do past its sandbox (#1342), by its id: none for a chat with no
    /// id, which no grant can have been made for.
    fn grants_of(&self, chat: &Chat) -> purlis_core::sandbox::grant::Grants {
        let mut grants = purlis_core::sandbox::grant::Grants::default();
        if let Some(id) = chat.identity.id.as_deref()
            && let Some(made) = lock(&self.grants).get(id)
        {
            for one in made {
                // Once is a scope of its own, which a start judges against policy (#1927).
                if one.once {
                    grants.add_once(&one.what);
                } else {
                    grants.add(&one.what);
                }
            }
        }
        grants
    }

    /// What a start of open chat `session` would be granted now ([`Self::grants_of`]): for a
    /// test of what a start or a restart composes into its sandbox.
    #[cfg(test)]
    pub(crate) fn grants_a_start_of(&self, session: u32) -> purlis_core::sandbox::grant::Grants {
        let chat = lock(&self.open)
            .get(&session)
            .map(|running| running.chat.clone());
        chat.map(|chat| self.grants_of(&chat)).unwrap_or_default()
    }

    /// **Lets chat `session` do `what`** (#1342), for as long as the app holds it, and owes it a
    /// restart on its conversation that tells it `told`. A chat that is not open, or one with no
    /// id, gets nothing, and is told why.
    #[cfg(test)]
    pub fn grant(
        &self,
        session: u32,
        what: purlis_core::sandbox::grant::What,
        at: u64,
        told: String,
    ) -> Result<(), String> {
        self.hold_grant(session, what, at)?;
        self.owe_restart(session, told);
        Ok(())
    }

    /// [`Self::grant`] without the restart: for a grant the chat's proxy takes live (#1666),
    /// held still so a later start of the chat keeps it and the Granted list shows it.
    #[cfg(test)]
    pub fn hold_grant(
        &self,
        session: u32,
        what: purlis_core::sandbox::grant::What,
        at: u64,
    ) -> Result<(), String> {
        self.commit_grant(self.reserve_grant(session, what, at, false)?)
    }

    /// [`Self::hold_grant`], once (D-sock-3): the restart that takes it starts with it, and it
    /// goes when the session it was allowed in ends, so no later start of the chat has it.
    #[cfg(test)]
    pub fn hold_grant_once(
        &self,
        session: u32,
        what: purlis_core::sandbox::grant::What,
        at: u64,
    ) -> Result<(), String> {
        self.commit_grant(self.reserve_grant(session, what, at, true)?)
    }

    /// **Reserves a grant of `what` for open chat `session`** (#2454), once where `once`: the
    /// first of two steps, made before its audit is written. It checks that the chat is open,
    /// so a chat that is not is refused before anything is audited; and it keeps nothing, so
    /// what the chat holds is unchanged until [`Self::commit_grant`].
    pub(crate) fn reserve_grant(
        &self,
        session: u32,
        what: purlis_core::sandbox::grant::What,
        at: u64,
        once: bool,
    ) -> Result<ReservedGrant, String> {
        let open = lock(&self.open);
        let running = open
            .get(&session)
            .ok_or_else(|| format!("chat {session} is not open, so nothing was allowed"))?;
        let id =
            running.chat.identity.id.clone().ok_or_else(|| {
                "that chat has no id yet, so nothing was allowed for it".to_owned()
            })?;
        let chat = running
            .chat
            .label
            .clone()
            .unwrap_or_else(|| running.chat.name.clone());
        // Held already where it lasts, or is once and once is asked: once made lasting is a new
        // grant, and audited.
        let grants = lock(&self.grants);
        let already = grants.get(&id).is_some_and(|made| {
            made.iter()
                .any(|one| one.what == what && (once || !one.once))
        });
        let taken_back = lock(&self.taken_back).get(&id).copied().unwrap_or(0);
        Ok(ReservedGrant {
            session,
            id,
            what,
            at,
            chat,
            once,
            already,
            taken_back,
        })
    }

    /// **Keeps a grant [`Self::reserve_grant`] reserved** (#2454), once its audit is written:
    /// from now on the chat's starts, the Granted list and a later Allow see it. Kept once only:
    /// a grant the chat holds already at least as long is left as it is, and one held once is
    /// made lasting where this one lasts.
    ///
    /// Refused, keeping nothing, where since it was reserved the chat closed, as its grants end
    /// with it (D-1348-1), or any grant of the chat was taken back: a Revoke made meanwhile
    /// stands. A grant for the chat is kept for any session of it still open, so a restart on its
    /// conversation keeps it; a grant once is for the session it was allowed in alone (D-sock-3).
    /// Checked under the locks a close and a Revoke take, so neither can come between.
    pub(crate) fn commit_grant(&self, reserved: ReservedGrant) -> Result<(), String> {
        let open = lock(&self.open);
        let still = if reserved.once {
            open.get(&reserved.session)
                .is_some_and(|running| running.chat.identity.id.as_deref() == Some(&reserved.id))
        } else {
            open.values()
                .any(|running| running.chat.identity.id.as_deref() == Some(&reserved.id))
        };
        if !still {
            return Err(format!(
                "chat {} closed before it was allowed, so nothing was kept",
                reserved.session
            ));
        }
        let mut grants = lock(&self.grants);
        let taken_back = lock(&self.taken_back)
            .get(&reserved.id)
            .copied()
            .unwrap_or(0);
        if taken_back != reserved.taken_back {
            return Err(format!(
                "a grant of chat {} was taken back while this was allowed, so nothing was kept",
                reserved.session
            ));
        }
        let made = grants.entry(reserved.id).or_default();
        match made.iter_mut().find(|one| one.what == reserved.what) {
            // Allowed for the chat after once: it lasts now.
            Some(one) if one.once && !reserved.once => one.once = false,
            Some(_) => {}
            None => made.push(ChatGrant {
                what: reserved.what,
                at: reserved.at,
                chat: reserved.chat,
                once: reserved.once,
                from: reserved.session,
            }),
        }
        Ok(())
    }

    /// Counts a taking back of chat `id`'s grants, with `grants` locked (#2454).
    fn took_back(&self, id: &str) {
        *lock(&self.taken_back).entry(id.to_owned()).or_default() += 1;
    }

    /// **Points chat `session`'s kubectl at a cluster on this Mac** (qw384b): `config` is the
    /// copy of one context made for it (`purlis_core::sandbox::kubeconfig::for_this_chat`),
    /// kept by the chat's id for as long as the app holds it, so a restart on its conversation
    /// keeps it and no other chat ever gets it. Owes the chat a restart that tells it `told`:
    /// each start writes the copy into the chat's own folder and names it by `KUBECONFIG`.
    /// **In memory only**, as a grant for one chat is.
    pub fn point_kubectl(&self, session: u32, config: String, told: String) -> Result<(), String> {
        let id = {
            let open = lock(&self.open);
            let running = open.get(&session).ok_or_else(|| {
                format!("chat {session} is not open, so kubectl was pointed at nothing")
            })?;
            running.chat.identity.id.clone().ok_or_else(|| {
                "that chat has no id yet, so kubectl was pointed at nothing".to_owned()
            })?
        };
        lock(&self.kubeconfigs).insert(id, config);
        self.owe_restart(session, told);
        Ok(())
    }

    /// The copy of a context the chat with id `id` was pointed at ([`Self::point_kubectl`]).
    pub fn kubeconfig_of(&self, id: &str) -> Option<String> {
        lock(&self.kubeconfigs).get(id).cloned()
    }

    /// **Withdraws `port` of this Mac from running chats' proxies** (qw384b): its grant was
    /// removed, and a chat's proxy carries it no longer, whatever name it is reached by. `chat`
    /// names the one chat it was removed for; none, every open chat but one that holds the port
    /// for itself. `chat` is the chat's id. The chats it was withdrawn from, by number; or, where
    /// a chat's proxy in the session host could not be told (#1999), what was said of each, once
    /// every chat was tried. Such a chat's proxy was ended rather than left carrying the port.
    /// Each chat it was withdrawn from is told narrowed ([`Self::tell_narrowed_to`]), whose
    /// browsers end, and a browser it opens after is given the port withdrawn (#2013).
    pub fn withdraw_port(&self, chat: Option<&str>, port: u16) -> Result<Vec<u32>, String> {
        // A chat that holds the port to connect to or to listen on (#2019) keeps it.
        let held = [
            purlis_core::sandbox::grant::What::Port(port),
            purlis_core::sandbox::grant::What::Listen(port),
        ];
        let holding: Vec<String> = lock(&self.grants)
            .iter()
            .filter(|(_, made)| made.iter().any(|one| held.contains(&one.what)))
            .map(|(id, _)| id.clone())
            .collect();
        let open = lock(&self.open);
        let mut failed: Vec<String> = Vec::new();
        let mut narrowed: Vec<u32> = Vec::new();
        let mut withdrawn: Vec<u32> = open
            .iter()
            .filter(|(_, running)| match chat {
                Some(one) => running.chat.identity.id.as_deref() == Some(one),
                None => !running
                    .chat
                    .identity
                    .id
                    .as_ref()
                    .is_some_and(|id| holding.contains(id)),
            })
            .filter_map(|(number, running)| {
                // What the chat is now is what a browser it opens after is given (#2013).
                if let Some(confines) = &running.confines {
                    confines.reach.withdraw(port);
                }
                narrowed.push(*number);
                match running.confinement.as_ref()?.withdraw_port(port) {
                    Ok(()) => Some(*number),
                    Err(why) => {
                        failed.push(format!("Chat {number}: {why}"));
                        None
                    }
                }
            })
            .collect();
        drop(open);
        withdrawn.sort_unstable();
        self.narrowed(&narrowed);
        if failed.is_empty() {
            Ok(withdrawn)
        } else {
            Err(failed.join(" "))
        }
    }

    /// Who is told, from now on, each chat whose project's Claude Code settings changed so that
    /// it no longer loads them (ADR 0067 as amended 2026-10-16): the project wires it to
    /// [`crate::planes::Held::contain_project_settings`] and to the window.
    pub fn tell_project_settings_to(&self, told: crate::projectsettings::Told) {
        self.project_settings.telling(told);
    }

    /// Chat `session` owed a restart because its project settings no longer pass: what a test
    /// that drives the restart itself owes it.
    #[cfg(test)]
    pub fn project_settings_changed(&self, session: u32) {
        self.owe_restart(
            session,
            purlis_core::harness::claude::project::RESCREENED.to_owned(),
        );
    }

    /// **Restarts chat `session` because its project settings no longer pass** (ADR 0067 as
    /// amended 2026-10-16), at the size its terminal was last given, whether or not a window
    /// is there to drive it. **Its run ends before the new one starts**, so two runs never share
    /// its conversation; that end is taken as no end of the chat ([`Self::replaced`]). Refused
    /// where it has no conversation to resume, with the run still going: the caller ends it.
    pub fn restart_for_project_settings(&self, session: u32) -> Result<u32, String> {
        self.claim(session)?;
        let mut told = vec![purlis_core::harness::claude::project::RESCREENED.to_owned()];
        told.extend(lock(&self.owed).remove(&session).unwrap_or_default());
        let again = match self.again_on_its_conversation(session) {
            Ok(again) if again.resume.is_some() => again,
            Ok(_) => {
                self.unclaim(session);
                return Err(no_conversation(session));
            }
            Err(why) => {
                self.unclaim(session);
                return Err(why);
            }
        };
        let size = lock(&self.sizes).get(&session).copied().unwrap_or(Size {
            columns: 80,
            rows: 24,
        });
        lock(&self.replacing).insert(session);
        if let Err(why) = self.sessions.close(session) {
            tracing::warn!("purlis: chat {session}'s run did not end before its restart ({why})");
        }
        let first = told.join("\n\n");
        match self.start_recorded_told(&again, size, Why::Relaunch, Some(&first), None) {
            Ok(started) => self.restarted(session, started, true),
            Err(why) => {
                self.unclaim(session);
                Err(why)
            }
        }
    }

    /// Marks run `session` as replaced at once, as a restart for changed project settings does.
    #[cfg(test)]
    pub fn replaced_in_a_test(&self, session: u32) {
        lock(&self.replacing).insert(session);
    }

    /// Whether the end of run `session`'s program is the end of a run replaced at once, taken
    /// as no end of the chat. Answered once.
    pub fn replaced(&self, session: u32) -> bool {
        lock(&self.replacing).remove(&session)
    }

    /// **Chat `session`, which could not be started again, kept aside with `why`** (ADR 0067 as
    /// amended 2026-10-16): among the chats that would not start, which the window lists with
    /// why and a Retry, whenever it opens. Before it is ended, while its record is known.
    pub fn keep_aside(&self, session: u32, why: &str) {
        if let Some(chat) = self.recorded_chat(session) {
            lock(&self.would_not_start).push(Waiting {
                chat,
                why: why.to_owned(),
                approval: None,
                told: None,
            });
        }
    }

    /// Resizes chat `session`'s terminal, and keeps the size for a restart the core drives.
    pub fn resize(&self, session: u32, size: Size) -> Result<(), String> {
        self.sessions.resize(session, size)?;
        if lock(&self.open).contains_key(&session) {
            lock(&self.sizes).insert(session, size);
        }
        Ok(())
    }

    /// The watch of this project's chats' Claude Code settings, for a test to change files at
    /// a moment of its choosing.
    #[cfg(test)]
    pub fn project_settings_watch(&self) -> &crate::projectsettings::Watch {
        &self.project_settings
    }

    /// Every watched chat's project settings, screened again now.
    #[cfg(test)]
    pub fn screen_project_settings_again(&self) {
        self.project_settings.screen_all();
    }

    /// The project settings of chats started from now are watched by `watch`.
    #[cfg(test)]
    pub fn watching_project_settings_with(&mut self, watch: crate::projectsettings::Watch) {
        self.project_settings = watch;
    }

    /// Owes chat `session` a restart on its conversation that tells it `told` (#1342): a grant
    /// of any level reaches a running chat that way.
    /// A chat that is not open is owed nothing: what was allowed reaches it at its next start.
    /// The ports open chat `session` serves on, behind their doors (#2019).
    pub fn served_ports(&self, session: u32) -> Vec<u16> {
        lock(&self.open)
            .get(&session)
            .and_then(|running| running.confinement.as_ref())
            .map(purlis_core::sandbox::Confinement::serving_ports)
            .unwrap_or_default()
    }

    /// Whether open chat `session`'s server dials in to its doors (#3316).
    pub fn dials_in(&self, session: u32) -> bool {
        lock(&self.open)
            .get(&session)
            .and_then(|running| running.confinement.as_ref())
            .is_some_and(purlis_core::sandbox::Confinement::dials_in)
    }

    /// **Parks `slot`, a connection open chat `ask.chat`'s server dialled in, at its door on
    /// `ask.serve`** (#3316): whether it is the first that door was given. Refused, the slot told
    /// why and closed, where the chat is not open here, has no door on that port, or holds the
    /// most slots it may ([`purlis_core::sandbox::Confinement::park`]); the reason is answered.
    #[cfg(unix)]
    pub fn park_slot(
        &self,
        ask: &purlis_core::hookwire::serve::ServeAsk,
        slot: purlis_core::hookwire::serve::Slot,
    ) -> Result<bool, &'static str> {
        let parked = match lock(&self.open)
            .get(&ask.chat)
            .and_then(|running| running.confinement.as_ref())
        {
            Some(confinement) => confinement.park(ask.serve, slot),
            None => Err((slot, purlis_core::sandbox::door::NO_DOOR)),
        };
        parked.map_err(|(slot, why)| {
            purlis_core::hookwire::serve::refused(slot, why);
            why
        })
    }

    /// **Each open chat serving on `port` is owed a restart** (#2019): its server's grant was
    /// taken back, and its profile was fixed as it started, so it restarts without it. Where no
    /// grant lets it listen there any more (`project_holds` says whether the project's own still
    /// does; a chat's own is read here), its door there closes at once, so nothing reaches its
    /// server on that port from now on (#3316). `chat` names the one chat it was taken back
    /// from, by id; none, every chat. The chats owed it, by number.
    pub fn owe_restart_serving(
        &self,
        chat: Option<&str>,
        port: u16,
        project_holds: bool,
    ) -> Vec<u32> {
        let serving: Vec<u32> = lock(&self.open)
            .iter()
            .filter(|(_, running)| {
                chat.is_none_or(|one| running.chat.identity.id.as_deref() == Some(one))
                    && running
                        .confinement
                        .as_ref()
                        .is_some_and(|confinement| confinement.serving_ports().contains(&port))
            })
            .map(|(number, _)| *number)
            .collect();
        // Its door closes at once where nothing lets it listen there any more (#3316): nothing
        // reaches its server there from now on, and a server that dials in has its parked
        // connections closed.
        if !project_holds {
            // The chats whose own grant still lets them listen there, read under the grants'
            // hold alone, so no two holds are ever taken together here.
            let held_by_their_own: Vec<String> = lock(&self.grants)
                .iter()
                .filter(|(_, made)| {
                    made.iter()
                        .any(|one| one.what == purlis_core::sandbox::grant::What::Listen(port))
                })
                .map(|(id, _)| id.clone())
                .collect();
            let mut open = lock(&self.open);
            for number in &serving {
                let Some(running) = open.get_mut(number) else {
                    continue;
                };
                let its_own = running
                    .chat
                    .identity
                    .id
                    .as_ref()
                    .is_some_and(|id| held_by_their_own.contains(id));
                if let Some(confinement) = running.confinement.as_mut().filter(|_| !its_own) {
                    confinement.stop_serving(port);
                }
            }
        }
        for number in &serving {
            self.owe_restart(
                *number,
                format!(
                    "⬢ purlis: running a server on port `{port}` — taken back by the person; \
                     this chat restarted without it."
                ),
            );
        }
        serving
    }

    pub fn owe_restart(&self, session: u32, told: String) {
        if !lock(&self.open).contains_key(&session) {
            return;
        }
        lock(&self.owed).entry(session).or_default().push(told);
    }

    /// **Owes chat `session` a restart the person asked for** (#1428): Restart chat on its
    /// tab's menu, or Restart them on the Notice after a sandbox setting changed. It tells the
    /// chat nothing, and anything it is owed besides is still told. The one restart
    /// ([`Self::restart`]) then takes it, once the chat's turn has ended. Refused for a chat
    /// that is not open.
    ///
    /// **Nothing while the chat is restarting**: that restart is the one asked for, and an ask
    /// queued behind it would restart the new run a second time, which nobody asked for.
    pub fn ask_restart(&self, session: u32) -> Result<(), String> {
        if !lock(&self.open).contains_key(&session) {
            return Err(format!(
                "purlis did not restart chat {session}: it is not open."
            ));
        }
        // Read on its own: a lock is never taken while another of these is held.
        let restarting = lock(&self.restarting).contains(&session);
        if !restarting {
            lock(&self.owed).entry(session).or_default();
        }
        Ok(())
    }

    /// Whether a session of the chat whose id is `id` is open: what its grants last as long
    /// as (D-1348-1).
    /// **Chat `session`'s identity** (ADR 0066), where it is open and its record has one.
    pub fn id_of(&self, session: u32) -> Option<String> {
        lock(&self.open)
            .get(&session)
            .and_then(|one| one.chat.identity.id.clone())
    }

    pub fn id_is_open(&self, id: &str) -> bool {
        lock(&self.open)
            .values()
            .any(|one| one.chat.identity.id.as_deref() == Some(id))
    }

    /// Chat `session`'s board of live asks (#1666), where its proxy holds a connection while
    /// the person is asked.
    pub fn board_of(&self, session: u32) -> Option<Arc<dyn purlis_core::sandbox::Board>> {
        lock(&self.open)
            .get(&session)?
            .confinement
            .as_ref()?
            .asks()
            .cloned()
    }

    /// Whether chat `session`'s proxy is asking the person about `target` (#1666): what the
    /// window's Notice says a connection waits on.
    pub fn asking(&self, session: u32, target: Option<&str>) -> bool {
        let Some(host) = target.and_then(|typed| purlis_core::sandbox::grant::host(typed).ok())
        else {
            return false;
        };
        self.board_of(session)
            .is_some_and(|board| board.asks_about(&host))
    }

    /// **The person allowed `host` at `level`, from the window** (#1666): every open chat whose
    /// proxy asks live and whom it reaches takes it now, with nothing restarting: chat
    /// `session` alone at a one-chat level (once, this chat), and no chat where none is named;
    /// every chat of the project at the others. Each chat a
    /// held connection of had given up on it is told, in purlis's fixed words, to retry.
    pub fn allow_live(
        &self,
        session: Option<u32>,
        host: &purlis_core::sandbox::hosts::Host,
        level: purlis_core::sandbox::grant::Level,
    ) -> Live {
        // A one-chat Allow reaches only its chat (#2629): with no chat named, none.
        let boards: Vec<(u32, Arc<dyn purlis_core::sandbox::Board>)> = {
            let open = lock(&self.open);
            open.iter()
                .filter(|(number, _)| !level.one_chat() || Some(**number) == session)
                .filter_map(|(number, running)| {
                    Some((*number, running.confinement.as_ref()?.asks()?.clone()))
                })
                .collect()
        };
        let mut live = Live::default();
        for (number, board) in boards {
            let retry = board.allow(host, level.into());
            if !retry.is_empty() {
                for target in retry {
                    self.tell_outcome(
                        number,
                        purlis_core::askoutcome::Outcome {
                            about: purlis_core::askoutcome::About::Host { host: target },
                            answer: purlis_core::askoutcome::Answer::Allowed {
                                scope: purlis_core::askoutcome::Scope::live(level),
                            },
                        },
                    );
                }
                live.told_to_retry.push(number);
            }
            live.reached.push(number);
        }
        live
    }

    /// The service on `port` left out of chat `session`'s plan by the runner it runs on
    /// (#2510): its confinement's board, where its proxy asks live and the port is one it left
    /// out. `None` for any other chat or port.
    fn left_out_board(
        &self,
        session: u32,
        port: u16,
    ) -> Option<Arc<dyn purlis_core::sandbox::Board>> {
        let open = lock(&self.open);
        let confinement = open.get(&session)?.confinement.as_ref()?;
        confinement
            .left_out()
            .contains(&port)
            .then(|| confinement.asks().cloned())
            .flatten()
    }

    /// Whether the runner chat `session` runs on asks about the service on `port`, which it left
    /// out of the chat's plan (#2510).
    pub fn runner_asks_about(&self, session: u32, port: u16) -> bool {
        self.left_out_board(session, port).is_some()
    }

    /// The name of the runner chat `session` runs on, where its runner left a service out of
    /// its plan (#2510).
    pub fn runner_of(&self, session: u32) -> Option<String> {
        lock(&self.open)
            .get(&session)?
            .confinement
            .as_ref()?
            .runner()
            .map(str::to_owned)
    }

    /// **The person allowed the service on `port` the runner chat `session` runs on asked
    /// about, from the window** (#2510, ruled 2026-10-18): that chat alone takes it now, on that
    /// runner, while its confinement runs. No other chat is reached, and nothing is kept on
    /// this machine. Whether it reached the chat, and whether a held connection of it had given
    /// up and the chat is told to retry; or why the runner did not take it.
    pub fn allow_left_out_live(&self, session: u32, port: u16) -> Result<Live, String> {
        let Some(board) = self.left_out_board(session, port) else {
            return Ok(Live::default());
        };
        let mut live = Live {
            reached: vec![session],
            told_to_retry: Vec::new(),
        };
        if !board.allow_left_out(port)?.is_empty() {
            self.tell_outcome(
                session,
                purlis_core::askoutcome::Outcome {
                    about: purlis_core::askoutcome::About::Port {
                        port: port.to_string(),
                    },
                    answer: purlis_core::askoutcome::Answer::Allowed {
                        scope: purlis_core::askoutcome::Scope::live(
                            purlis_core::sandbox::grant::Level::Chat,
                        ),
                    },
                },
            );
            live.told_to_retry.push(session);
        }
        Ok(live)
    }

    /// **The person kept the service on `port` the runner chat `session` runs on asked about
    /// blocked** (#2510): what its proxy holds on it is refused now, and every later connection
    /// to it, until an Allow.
    pub fn keep_left_out_blocked_live(&self, session: u32, port: u16) {
        if let Some(board) = self.left_out_board(session, port) {
            board.keep_left_out_blocked(port);
        }
    }

    /// **An Allow of `host` at `level` was removed**, from the window (#1666): what it allowed
    /// live reaches nothing again in the chats it reached: the chat whose id is `chat`, and at a
    /// one-chat level (once, this chat) no chat where none is named; every open chat at the
    /// others.
    pub fn forget_live(
        &self,
        chat: Option<&str>,
        host: &purlis_core::sandbox::hosts::Host,
        level: purlis_core::sandbox::grant::Level,
    ) {
        let (narrowed, boards): (Vec<u32>, Vec<Arc<dyn purlis_core::sandbox::Board>>) = {
            let open = lock(&self.open);
            let reached: Vec<(&u32, &Running)> = open
                .iter()
                .filter(|(_, running)| match chat {
                    Some(chat) => running.chat.identity.id.as_deref() == Some(chat),
                    // A one-chat Allow reaches only its chat (#2629): with no chat named, none.
                    None => !level.one_chat(),
                })
                .collect();
            (
                reached.iter().map(|(number, _)| **number).collect(),
                reached
                    .iter()
                    .filter_map(|(_, running)| running.confinement.as_ref()?.asks().cloned())
                    .collect(),
            )
        };
        for board in boards {
            board.forget(host, level.into());
        }
        // A browser of the chat was given what it allowed live: it ends (#2013).
        self.narrowed(&narrowed);
    }

    /// **The person kept `host` blocked for chat `session`, from the window** (#1666): what its
    /// proxy holds on it is refused now. Nothing is granted. The chat is told by whoever kept
    /// it blocked ([`Self::tell_outcome`], #1987).
    ///
    /// The later answer wins (#3407): an Allow of a host it covers, or one that covers it, kept
    /// for the chat ends too, so the Granted list and the chat's next start agree with its
    /// proxy. Each one is audited as taken back through `audit` first, as a Revoke is; one whose
    /// audit could not be written ends all the same, and its audit over-records. Where the
    /// chat's reach narrowed so, its browsers end, as they do when an Allow is removed (#2013).
    pub fn keep_blocked_live(
        &self,
        session: u32,
        host: &purlis_core::sandbox::hosts::Host,
        audit: crate::sandboxing::Audit<'_>,
    ) {
        let id = lock(&self.open)
            .get(&session)
            .and_then(|running| running.chat.identity.id.clone());
        let ended = id.is_some_and(|id| self.end_grants_of_host(session, &id, host, audit));
        let narrowed = self
            .board_of(session)
            .is_some_and(|board| board.keep_blocked(host));
        if ended || narrowed {
            self.narrowed(&[session]);
        }
    }

    /// Takes back chat `id`'s grants of a host `host` covers, or that covers it, each audited
    /// through `audit` first: whether any was taken back.
    fn end_grants_of_host(
        &self,
        session: u32,
        id: &str,
        host: &purlis_core::sandbox::hosts::Host,
        audit: crate::sandboxing::Audit<'_>,
    ) -> bool {
        let touches = |what: &purlis_core::sandbox::grant::What| match what {
            purlis_core::sandbox::grant::What::Host(granted) => {
                host.covers(granted) || granted.covers(host)
            }
            _ => false,
        };
        // Audited outside the hold: writing the audit reads the chats.
        let ending: Vec<ChatGrant> = lock(&self.grants)
            .get(id)
            .map(|made| {
                made.iter()
                    .filter(|one| touches(&one.what))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        for one in &ending {
            let target = one.what.target();
            let audited = purlis_core::sandbox::grant::Audited {
                granted: false,
                what: one.what.word(),
                target: &target,
                level: if one.once {
                    purlis_core::sandbox::grant::Level::Once
                } else {
                    purlis_core::sandbox::grant::Level::Chat
                },
            };
            if let Err(why) = audit(Some(session), &audited) {
                tracing::warn!(
                    "purlis: a chat's Allow ended by Keep blocked was not audited ({why})"
                );
            }
        }
        // Taken back, and counted so, under one hold, as a Revoke is: a grant reserved before
        // it cannot be kept after it.
        let mut grants = lock(&self.grants);
        let Some(made) = grants.get_mut(id) else {
            return false;
        };
        let before = made.len();
        made.retain(|one| !touches(&one.what));
        let ended = made.len() != before;
        if ended {
            self.took_back(id);
        }
        ended
    }

    /// **Hands chat `session`'s next turn the person's answer to an Ask of its** (#1666,
    /// #1987), in [`purlis_core::askoutcome::Outcome::line`]'s one line, on the road the
    /// person's words take: from the app's memory, never a file. Every answer the window gives
    /// a chat's Ask that does not restart it is told through here.
    pub fn tell_outcome(&self, session: u32, outcome: purlis_core::askoutcome::Outcome) {
        if let Some(tell) = lock(&self.network_words).clone() {
            tell(session, outcome);
        }
    }

    /// Who keeps purlis's word on a chat's Asks for its next turn (#1666, #1987).
    pub fn tell_network_words_to(&self, keep: NetworkWords) {
        *lock(&self.network_words) = Some(keep);
    }

    /// Who is told, from now on, each open chat whose reach the person narrowed while it runs
    /// (#2013): a port withdrawn from it, an Allow of it removed. Its browsers end, so none
    /// reaches what the chat no longer may.
    pub fn tell_narrowed_to(&self, told: Narrowed) {
        *lock(&self.narrowed) = Some(told);
    }

    fn narrowed(&self, sessions: &[u32]) {
        let Some(tell) = lock(&self.narrowed).clone() else {
            return;
        };
        for session in sessions {
            tell(*session);
        }
    }

    /// The sandbox blocks each open chat is held on now (#1508).
    pub fn blocks(&self) -> &crate::taskblocks::Blocks {
        &self.blocks
    }

    /// Who hears, from now on, each host purlis's own proxy refuses a chat it wraps (#1663).
    pub fn tell_refusals_to(&self, hear: crate::hooks::Blocks) {
        *lock(&self.refused) = Some(hear);
    }

    /// Who hears, from now on, how many hosts a chat's proxy refused past the ones it names,
    /// each time that grows (#1353).
    pub fn tell_more_refused_to(&self, hear: MoreRefused) {
        *lock(&self.refused_more) = Some(hear);
    }

    /// Who is told, from now on, that this project's asks may have moved because a chat's proxy
    /// asked live (#1709).
    pub fn tell_asks_moved_to(&self, moved: AsksMoved) {
        *lock(&self.asks_moved) = Some(moved);
    }

    /// The pool chat `session` runs on, where it runs on a pool's runner (#2046).
    pub fn pool_of(&self, session: u32) -> Option<String> {
        self.sessions.pool_of(session)
    }

    /// Whether Move here brings back a clone the runner chat `session` runs on made (#3811).
    pub fn clones_come_back(&self, session: u32) -> bool {
        self.sessions.clones_come_back(session)
    }

    /// This project's asks may have moved for a reason no event of its own says (qw356: a
    /// chat asked the person to remove a workspace, or that question was answered): the window
    /// is told its asks, so it reads the registry again.
    pub fn asks_may_have_moved(&self) {
        if let Some(moved) = lock(&self.asks_moved).clone() {
            moved();
        }
    }

    /// Who hears, from now on, each connection purlis's own proxy carries for a chat it wraps
    /// (#1664).
    pub fn tell_connections_to(&self, hear: ProxyReached) {
        *lock(&self.reached) = Some(hear);
    }

    /// The folder chat `session` was started in: what a write grant for it is judged against.
    pub fn folder_of(&self, session: u32) -> Option<std::path::PathBuf> {
        lock(&self.open).get(&session)?.chat.cwd.clone()
    }

    /// Every grant made for an open chat, by chat id (#1348).
    pub fn chat_grants(&self) -> Vec<(String, ChatGrant)> {
        let open_ids: std::collections::HashSet<String> = lock(&self.open)
            .values()
            .filter_map(|running| running.chat.identity.id.clone())
            .collect();
        let mut out: Vec<(String, ChatGrant)> = lock(&self.grants)
            .iter()
            .filter(|(id, _)| open_ids.contains(*id))
            .flat_map(|(id, made)| made.iter().map(|one| (id.clone(), one.clone())))
            .collect();
        out.sort_by_key(|(_, one)| one.at);
        out
    }

    /// Whether chat `id` holds a grant of `what`.
    /// Whether open chat `session` holds a grant of `what` already (#1666): an Allow of it for
    /// this chat again keeps nothing new.
    #[cfg(test)]
    pub fn holds_for(&self, session: u32, what: &purlis_core::sandbox::grant::What) -> bool {
        let id = lock(&self.open)
            .get(&session)
            .and_then(|running| running.chat.identity.id.clone());
        id.is_some_and(|id| self.holds(&id, what))
    }

    pub fn holds(&self, id: &str, what: &purlis_core::sandbox::grant::What) -> bool {
        lock(&self.grants)
            .get(id)
            .is_some_and(|made| made.iter().any(|one| one.what == *what))
    }

    /// Takes back chat `id`'s grant of `what` (#1348): its next start is compiled without it.
    /// Answers whether there was one.
    pub fn revoke(&self, id: &str, what: &purlis_core::sandbox::grant::What) -> bool {
        let mut grants = lock(&self.grants);
        let Some(made) = grants.get_mut(id) else {
            return false;
        };
        let before = made.len();
        made.retain(|one| one.what != *what);
        let revoked = made.len() != before;
        if revoked {
            self.took_back(id);
        }
        revoked
    }

    /// **Restarts chat `session` on its conversation**: the one restart (#1342, #1362, #1428).
    /// The same chat, under its id, its conversation resumed, in a new run. What is decided at
    /// a start is decided again: its sandbox is compiled from the project's settings as they
    /// are now, with what the person granted it and the persona grants it holds, and its first
    /// message is whatever it was owed to be told.
    ///
    /// Refused for a chat owed nothing: a restart is owed by a grant ([`Self::grant`],
    /// [`Self::owe_restart`]) or asked for by the person ([`Self::ask_restart`]), and by
    /// nothing a chat sends. The old one stays open until the new one has started, as
    /// [`Self::start_fresh_unless`]'s does; ending it is the caller's next step.
    ///
    /// **A person's opt-out is never inherited** (ADR 0067 §7): a chat that ran without the
    /// sandbox restarts in it, and its tab says so ([`SANDBOXED_AGAIN`]).
    pub fn restart(&self, session: u32, size: Size) -> Result<u32, String> {
        self.claim(session)?;
        // Taken under the lock, so a grant made while this restart runs queues for the next one
        // and is never erased by this one.
        let Some(told) = lock(&self.owed).remove(&session) else {
            self.unclaim(session);
            return Err(format!(
                "purlis did not restart chat {session}: it is owed no restart."
            ));
        };
        let started = self.again_on_its_conversation(session).and_then(|again| {
            if again.resume.is_none() {
                let reaches = if told.is_empty() {
                    ""
                } else {
                    " What was allowed reaches it when it next starts."
                };
                return Err(format!("{}{reaches}", no_conversation(session)));
            }
            let first = (!told.is_empty()).then(|| told.join("\n\n"));
            let started =
                self.start_recorded_told(&again, size, Why::Relaunch, first.as_deref(), None)?;
            if again.unsandboxed && self.confines_of(started).is_some() {
                lock(&self.late_notes)
                    .entry(started)
                    .or_default()
                    .push(SANDBOXED_AGAIN.to_owned());
            }
            Ok(started)
        });
        match started {
            Ok(started) => self.restarted(session, started, true),
            Err(why) => {
                // Owed still: put back ahead of anything queued since. A restart the person
                // asked for, with nothing to tell, is not: they were told why, and ask again.
                let mut owed = lock(&self.owed);
                let mut back = told;
                back.extend(owed.remove(&session).unwrap_or_default());
                if !back.is_empty() {
                    owed.insert(session, back);
                }
                drop(owed);
                self.unclaim(session);
                Err(why)
            }
        }
    }

    /// **Starts chat `session` again without the sandbox** (#1342): the person's own choice, from
    /// a block's Notice that purlis grants nothing for. It is the picker's opt-out (ADR 0067 §7),
    /// for this one chat and this one run: audited as `trust.sandbox.off` by the start, never
    /// recorded, so a later start of the chat is sandboxed again. Its conversation is resumed
    /// where it has one. It restarts now, mid-turn or not: the person pressed for it.
    pub fn restart_without_sandbox(&self, session: u32, size: Size) -> Result<u32, String> {
        self.claim(session)?;
        let opt_out = purlis_core::sandbox::OptOut {
            reason: Some(
                "started again without the sandbox from a sandbox block's Notice".to_owned(),
            ),
        };
        let started = self.again_on_its_conversation(session).and_then(|again| {
            self.start_recorded_told(&again, size, Why::Relaunch, None, Some(opt_out))
        });
        match started {
            // What was allowed and not yet taken is dropped: it means nothing to a chat run
            // without the sandbox, and would restart it sandboxed once its turn ended.
            Ok(started) => self.restarted(session, started, false),
            Err(why) => {
                self.unclaim(session);
                Err(why)
            }
        }
    }

    /// **Starts chat `session` again on its conversation, where its workspace's chats run
    /// now** (#2048): what a move ends with, once the chat's work and conversation are there.
    /// The new run is held apart until [`Self::took_over`] gives it the chat's place, or
    /// [`Self::let_go_of_start`] ends it, so the chats of a workspace that move together all
    /// start before any takes its place (#3453). The old run stays meanwhile. A person's opt-out
    /// is never carried: it starts sandboxed.
    pub fn start_again_held(&self, session: u32, size: Size) -> Result<u32, String> {
        self.claim(session)?;
        let started = self.again_on_its_conversation(session).and_then(|again| {
            if again.resume.is_none() {
                return Err(no_conversation(session));
            }
            self.start_recorded_told(&again, size, Why::Relaunch, None, None)
        });
        if started.is_err() {
            self.unclaim(session);
        }
        started
    }

    /// **`started`, held apart by [`Self::start_again_held`], takes chat `session`'s place**
    /// (#2048), as a restart's does; ending the old run is the caller's next step.
    pub fn took_over(&self, session: u32, started: u32) -> Result<u32, String> {
        self.restarted(session, started, true)
    }

    /// **`started`, held apart by [`Self::start_again_held`], ends**, and chat `session` runs on
    /// where it was (#3453): another chat of its move did not start.
    pub fn let_go_of_start(&self, session: u32, started: u32) {
        self.unclaim(session);
        if let Err(why) = self.close(started) {
            tracing::warn!("purlis: a start of a move that did not happen did not end ({why})");
        }
    }

    /// **Says `note` on chat `session`'s tab** with what its start found to say (#2048): what a
    /// move found that the person is to look at.
    pub fn note_on_its_tab(&self, session: u32, note: String) {
        lock(&self.late_notes)
            .entry(session)
            .or_default()
            .push(note);
    }

    /// Marks chat `session` as restarting, or refuses: one restart at a time, whichever asked.
    fn claim(&self, session: u32) -> Result<(), String> {
        if lock(&self.restarting).insert(session) {
            Ok(())
        } else {
            Err(format!(
                "purlis is already starting chat {session} again, so it was not started twice."
            ))
        }
    }

    fn unclaim(&self, session: u32) {
        lock(&self.restarting).remove(&session);
    }

    /// What follows a restart of chat `session` as `started`: if the chat was closed while it
    /// restarted, the new run is ended (nobody asked for it any more, and it would keep the
    /// chat's grants alive unseen); otherwise the new run takes its place, and what was queued
    /// for the old one meanwhile to tell it is owed to the new one where `keep_owed`, else
    /// dropped.
    fn restarted(&self, session: u32, started: u32, keep_owed: bool) -> Result<u32, String> {
        if !lock(&self.open).contains_key(&session) {
            lock(&self.owed).remove(&session);
            self.unclaim(session);
            if let Err(why) = self.close(started) {
                tracing::warn!("purlis: a restart of a closed chat did not end ({why})");
            }
            return Err(format!(
                "purlis ended the new run of chat {session}: the chat was closed while it \
                 started again."
            ));
        }
        // Taken out in a statement of its own: the guard of a lock taken in an `if let` lives
        // through its body, and putting the queue back under it waited on this thread's own
        // lock for good (issue 1428's review, S1; the hang was on main since issue 1342).
        let queued = lock(&self.owed).remove(&session).unwrap_or_default();
        // Only what has something to tell is carried: an ask alone, queued while this restart
        // ran, is answered by this restart.
        if keep_owed && !queued.is_empty() {
            lock(&self.owed).insert(started, queued);
        }
        self.took_the_place_of(session, started);
        self.unclaim(session);
        Ok(started)
    }

    /// Chat `session` as it would start again on its conversation: the same chat under its id,
    /// in a new run.
    fn again_on_its_conversation(&self, session: u32) -> Result<Chat, String> {
        let was = lock(&self.open)
            .get(&session)
            .map(|one| one.chat.clone())
            .ok_or_else(|| format!("purlis did not restart chat {session}: it is not open."))?;
        Ok(Chat {
            identity: purlis_core::reopen::Identity {
                run: None,
                ..was.identity.clone()
            },
            pid: None,
            number: None,
            ..was
        })
    }

    /// `started` takes the place of `session` in the strip's order, and the record says so.
    fn took_the_place_of(&self, session: u32, started: u32) {
        for placed in lock(&self.order).iter_mut() {
            if *placed == session {
                *placed = started;
            }
        }
        self.write_it_down();
    }

    /// **The chats running under a sandbox other than the one a start would compile for them
    /// now** (#1428), or none: what the Notice after a sandbox setting changes is about.
    ///
    /// Decided by comparing what each chat's sandbox was compiled to as it started
    /// ([`Running::confines`]) with what the same start compiles now, never by a settings file
    /// having been written: a change that compiles to the same sandbox leaves nothing behind.
    ///
    /// git is asked once about the project's local file for every chat compared (#2179).
    pub fn on_older_sandbox(&self) -> Option<OlderSandbox> {
        purlis_core::profiles::git_asked_once(|| {
            self.on_older_sandbox_on(
                &purlis_core::sandbox::Machine::this(),
                &purlis_core::sandbox::backend::installed,
            )
        })
    }

    /// [`Self::on_older_sandbox`], on `machine` with `has` saying what is installed.
    ///
    /// **Only what the project's settings decide is compared** ([`Settled`], D-1428-10), and
    /// each chat is compiled with the persona grants and the chat's own grants it started
    /// with: a grant for one chat, or a persona's hosts allowed on its tab, is not the
    /// project's sandbox changing, and has its own Notice and its own restart.
    ///
    /// Not counted: a chat a person started without the sandbox (it runs under no compiled
    /// sandbox, old or new, by their own choice for that start), **unless this machine's
    /// policy now requires the sandbox** (#1431), which no choice of theirs lifts; a shell,
    /// which is the person's own; a chat whose sandbox could not be compiled now (its restart
    /// would be refused, and say why); and a chat already owed a restart or restarting, which
    /// gets the sandbox the project has now without being asked again.
    ///
    /// **A chat running without the sandbox that policy now requires it for says so**
    /// (`OnOlderSandbox::policy`), with who set the policy. The policy is only read here.
    fn on_older_sandbox_on(
        &self,
        machine: &purlis_core::sandbox::Machine,
        has: &dyn Fn(&str) -> bool,
    ) -> Option<OlderSandbox> {
        use purlis_core::sandbox::Decided;
        // Each read out from under its lock, one at a time: compiling reads the project's
        // files, and no lock here is taken while another is held.
        let owed: std::collections::HashSet<u32> = lock(&self.owed).keys().copied().collect();
        let restarting = lock(&self.restarting).clone();
        let locks = purlis_core::sandbox::policy::Locks::of(&self.project);
        let required = locks.forbids_opt_out();
        let running: Vec<_> = lock(&self.open)
            .iter()
            .filter(|(session, one)| {
                (required || !one.chat.unsandboxed)
                    && !owed.contains(session)
                    && !restarting.contains(session)
            })
            .filter_map(|(session, one)| {
                Some((
                    *session,
                    one.chat.clone(),
                    one.harness?,
                    one.confines.as_ref().map(Settled::of),
                    one.started_with.clone(),
                ))
            })
            .collect();
        let mut chats = Vec::new();
        // **Each distinct start is compiled once per ask** (#2157): the window asks whenever a
        // chat opens or closes, and chats started alike (most of them) compile alike, so a
        // compile per chat made the asks grow with the square of the chats open.
        let mut compiled: Vec<(StartKey, CompiledNow)> = Vec::new();
        for (session, chat, harness, started, with) in running {
            let persona = grants_persona_of(
                &Chat {
                    held: with.held.clone(),
                    ..chat.clone()
                },
                &self.project,
            );
            // Everything the compile below reads of the chat, and nothing else.
            let key = StartKey {
                harness,
                persona,
                // As its own start reads it: its persona on a profile, and none (the
                // project's) on no profile, as `sandbox_off_profile` passes.
                level_as: level_as_of(&chat).map(str::to_owned),
                // What the chat's next start would be made as.
                made: made_of(&chat, Why::Relaunch),
                grants: with.grants.clone(),
            };
            let known = compiled
                .iter()
                .find(|(one, _)| *one == key)
                .map(|(_, now)| now.clone());
            let now = known.unwrap_or_else(|| {
                let now = match purlis_core::sandbox::decide_granted(
                    key.harness,
                    &self.project,
                    machine,
                    has,
                    &purlis_core::sandbox::Person {
                        opt_out: None,
                        level_as: key.level_as.as_deref(),
                        made: &key.made,
                    },
                    key.persona.as_deref(),
                    &key.grants,
                ) {
                    Ok(Some(Decided::Sandboxed(applied))) => {
                        CompiledNow::Sandboxed(Settled::of(applied.confines()))
                    }
                    Ok(Some(Decided::Unsandboxed(_) | Decided::Floorless(_)) | None) => {
                        CompiledNow::Unconfined
                    }
                    Err(_) => CompiledNow::Refused,
                };
                compiled.push((key, now.clone()));
                now
            });
            let now = match now {
                CompiledNow::Sandboxed(settled) => Some(settled),
                CompiledNow::Unconfined => None,
                // A chat whose sandbox could not be compiled now is not counted.
                CompiledNow::Refused => continue,
            };
            if now != started {
                // Policy is the cause where the chat runs unconfined and policy requires the
                // sandbox: no setting of the project's could leave it without one now.
                let policy = (required && started.is_none() && now.is_some())
                    .then(|| locks.led_by("The sandbox is now required by this machine's policy"));
                chats.push(OnOlderSandbox {
                    session,
                    change: Settled::key(now.as_ref()),
                    policy,
                });
            }
        }
        if chats.is_empty() {
            return None;
        }
        chats.sort_unstable_by_key(|one| one.session);
        Some(OlderSandbox { chats })
    }

    /// **The open chats whose skills changed since they started** (#2770, ADR 0099 §5), by
    /// session: the plane skills each would get now differ from those it started with, in a way
    /// its harness does not take in by itself ([`Harness::skills_reload`]). Its tab offers a
    /// restart on the same conversation, and its Skills view names what changed (#3694). Read
    /// from disk each time it is asked.
    pub fn skills_changed(&self) -> Vec<(u32, purlis_core::skills::SkillsChanged)> {
        // Asked once, for every chat: where the project's skills reach no chat, none changed.
        if !purlis_core::skills::reach_chats_here(&self.project) {
            return Vec::new();
        }
        self.skills_changed_while_on()
    }

    /// [`Self::skills_changed`] while the `skill-studio` flag is on.
    fn skills_changed_while_on(&self) -> Vec<(u32, purlis_core::skills::SkillsChanged)> {
        // Read out first: no lock is held while the plane's skill folders are read.
        type Started = (
            u32,
            Option<String>,
            Option<String>,
            Harness,
            purlis_core::hoststart::ChatSkills,
            purlis_core::skills::Stamp,
        );
        let started: Vec<Started> = lock(&self.open)
            .iter()
            .filter_map(|(session, running)| {
                Some((
                    *session,
                    running.workspace.clone(),
                    running.chat.persona.clone(),
                    running.harness?,
                    running.skills_read.clone(),
                    running.skills.clone()?,
                ))
            })
            .collect();
        let built_in = built_in_skills_here();
        let mut changed: Vec<(u32, purlis_core::skills::SkillsChanged)> = started
            .into_iter()
            .filter_map(|(session, workspace, persona, harness, start, then)| {
                let now = purlis_core::skills::stamp_of(&purlis_core::skills::Running {
                    plane: &self.project,
                    workspace: workspace.as_deref(),
                    persona: persona.as_deref(),
                    harness,
                    built_in: built_in.as_deref(),
                    plane_skills: true,
                    start: Some(&start),
                });
                (now != then).then(|| (session, then.changed_to(&now)))
            })
            .collect();
        changed.sort_unstable_by_key(|(session, _)| *session);
        changed
    }

    /// The chats owed a restart (#1342, #1428): what the window drives once each one's turn
    /// has ended, read again whenever the window is drawn anew.
    pub fn owed_restarts(&self) -> Vec<u32> {
        let mut owed: Vec<u32> = lock(&self.owed).keys().copied().collect();
        owed.sort_unstable();
        owed
    }

    /// **Chat `session`'s program ended** before the pause its restart waited for (#1986): it is
    /// restarted by nothing, and owed nothing. What was allowed reaches it at its next start.
    /// Not while a restart of it is under way: that one ends this program itself, and carries
    /// what it owes to the new run ([`Self::restarted`]).
    pub fn its_program_ended(&self, session: u32) {
        // Read on its own: a lock is never taken while another of these is held.
        let restarting = lock(&self.restarting).contains(&session);
        if !restarting {
            lock(&self.owed).remove(&session);
            // Read on its own too. A chat closed first has no tab left to say anything of.
            let open = lock(&self.open).contains_key(&session);
            if open {
                lock(&self.ended).insert(session);
            }
        }
        self.project_settings.forget(session);
        // The GitHub App tokens minted for it go with its program (#3808).
        purlis_core::forge::apptoken::chat_ended(&self.project, session);
    }

    /// Starts a chat, and remembers what it was started as.
    ///
    /// For a chat that is NOT on a profile — the operator's shell — where what runs is
    /// decided from the record alone.
    pub fn start(&self, chat: &Chat, size: Size) -> Result<u32, String> {
        self.start_as(chat, size, false, Why::New)
    }

    /// [`Self::start`], for the shell the operator opens from the window: the one start the
    /// kill switch lets through while agents are stopped (OV-1, ADR 0071).
    pub fn start_operator_shell(&self, chat: &Chat, size: Size) -> Result<u32, String> {
        self.start_as(chat, size, true, Why::New)
    }

    fn start_as(
        &self,
        chat: &Chat,
        size: Size,
        operator_shell: bool,
        why: Why,
    ) -> Result<u32, String> {
        self.start_as_opted(chat, size, operator_shell, why, None)
    }

    /// [`Self::start_as`], with a person's opt-out of the sandbox for this run where there is
    /// one (#1342's "Start without the sandbox" on a block's Notice).
    fn start_as_opted(
        &self,
        chat: &Chat,
        size: Size,
        operator_shell: bool,
        why: Why,
        opt_out: Option<&purlis_core::sandbox::OptOut>,
    ) -> Result<u32, String> {
        // Before anything is resolved or run, as for a chat on a profile.
        let grants = self.grants_of(chat);
        #[cfg(test)]
        let decided = match &self.deciding {
            Some(deciding) => deciding(chat, &grants, opt_out),
            None => self.sandbox_off_profile(chat, &grants, opt_out, &made_of(chat, why)),
        };
        #[cfg(not(test))]
        let decided = self.sandbox_off_profile(chat, &grants, opt_out, &made_of(chat, why));
        let (sandboxed, unsandboxed, floorless) = decided?;
        let mut launch = chat.launch();
        let sandbox = sandboxed.map(|(applied, program)| {
            launch.program = program;
            applied
        });
        // A shell tab's shims, and the start files that keep them first. Never recorded: they
        // are this build's, and worked out again at every start.
        let (args, mut env) = self.shell_start(chat, &launch.program, launch.args);
        // The project its hooks and guards act for: the open one, as a chat on a profile is
        // told (`purlis_core::start`), never one they would find above the chat's folder.
        if chat.harness().is_some() {
            env.push(("PURLIS_ROOT".to_owned(), self.project.display().to_string()));
        }
        self.open_it(
            chat,
            launch.program,
            // A chat on no profile runs its program by name, so there is no wrapper's
            // command to keep in front: its recorded words follow charter's, as they always
            // have, because they may end in a positional prompt.
            Vec::new(),
            args,
            env,
            chat.harness(),
            // A chat on no profile runs no harness a project declares.
            None,
            launch.session.as_ref().map(ToString::to_string),
            launch.how,
            // A chat on no profile has no project choice to carry: it runs as it always did,
            // with the pins alone.
            &purlis_core::harness_plugin::Chosen::new(),
            sandbox.as_ref(),
            unsandboxed.as_ref(),
            size,
            operator_shell,
            why,
            // A chat on no profile has no sign-in of a profile's.
            None,
            // Nor a first message.
            None,
        )
        // Where the floor could not be applied, its tab's sandbox mark says why (#2071, Q2).
        .inspect(|session| self.floorless_at(*session, floorless))
    }

    /// What a shell tab's shell is started with beyond `args`, the chat's own words: charter's
    /// shims first on its `PATH`, and the start files that keep them first (ADR 0062). A chat
    /// that is not a shell tab — one on a profile, or one running a harness — gets nothing,
    /// and nor does any chat in an app that has no shims.
    fn shell_start(
        &self,
        chat: &Chat,
        program: &str,
        args: Vec<String>,
    ) -> (Vec<String>, Vec<(String, String)>) {
        let Some(shims) = self.shipped.shims.as_ref() else {
            return (args, Vec::new());
        };
        // A shell tab is a chat on no profile running no harness — `open_session` with no
        // program, and the same chat put back from the record.
        if chat.profile.is_some() || chat.harness().is_some() {
            return (args, Vec::new());
        }
        // The `PATH` every chat gets, worked out by the one function that works it out, with
        // the shims put in front of it.
        let chat_env =
            purlis_core::start::with_chat_path(Vec::new(), self.shipped.binary.as_deref());
        let start = shims.shell_start(
            program,
            chat_env,
            std::env::var_os("PATH").as_deref(),
            std::env::var_os("ZDOTDIR").as_deref(),
        );
        let mut all = start.args;
        all.extend(args);
        (all, start.env)
    }

    /// What arms a chat's git with charter's hooks (SQ-16): `core.hooksPath` pointed at the
    /// app's hooks directory, for a chat running a harness, so every commit its agent makes —
    /// in a workspace repo, a piece, or a repository outside any plane — is scanned before it
    /// is made. Nothing for a shell tab, which is the operator's own, and nothing when the app
    /// could not write the hooks.
    pub(crate) fn git_hooks_for(
        &self,
        harness: Option<Harness>,
    ) -> Option<purlis_core::githooks::GitHooks> {
        harness.and(self.shipped.git_hooks.clone())
    }

    /// What this app arms a chat of `harness` with ([`purlis_core::hoststart::Kit`]): its own
    /// binary, its plugin, and its git hooks for a chat running a harness.
    fn kit(&self, harness: Option<Harness>) -> purlis_core::hoststart::Kit {
        purlis_core::hoststart::Kit {
            binary: self.shipped.binary.clone(),
            plugin: self.shipped.plugin.clone(),
            git_hooks: self.git_hooks_for(harness),
            skill_sets: purlis_core::hoststart::skill_sets_here(),
            chat_settings: purlis_core::hoststart::chat_settings_here(),
        }
    }

    /// The one place a session is opened and a chat is remembered.
    ///
    /// Everything that differs between a profile chat and a shell chat is decided by the
    /// caller and arrives here as arguments — above all the HARNESS, which for a profile
    /// comes from its declared kind and must not be asked of the program's name.
    ///
    /// **A sandboxed Claude Code chat whose project settings stop passing as it starts** (ADR
    /// 0067 as amended 2026-10-16): its first screen, once its program runs, says so. That run
    /// is ended before anything else is started, and the chat is started once more on the
    /// person's settings and purlis's: the number answered is the run that goes on, never one
    /// already ended, and only that run is in the event log and this machine's count (#2250).
    #[allow(clippy::too_many_arguments)]
    fn open_it(
        &self,
        chat: &Chat,
        program: String,
        command: Vec<String>,
        args: Vec<String>,
        env: Vec<(String, String)>,
        harness: Option<Harness>,
        declared: Option<&purlis_core::harness_declaration::Declaration>,
        conversation: Option<String>,
        how: purlis_core::reopen::Reopened,
        plugins: &purlis_core::harness_plugin::Chosen,
        sandbox: Option<&purlis_core::sandbox::Applied>,
        unsandboxed: Option<&purlis_core::sandbox::Lifted>,
        size: Size,
        operator_shell: bool,
        why: Why,
        sign_in: Option<&purlis_core::profilesignin::AtStart>,
        first_message: Option<&str>,
    ) -> Result<u32, String> {
        let again = (
            program.clone(),
            command.clone(),
            args.clone(),
            env.clone(),
            conversation.clone(),
            how.clone(),
        );
        let (session, failed) = self.open_it_as(
            chat,
            program,
            command,
            args,
            env,
            harness,
            declared,
            conversation,
            how,
            plugins,
            sandbox,
            unsandboxed,
            size,
            operator_shell,
            why,
            sign_in,
            first_message,
            false,
        )?;
        if !failed {
            return Ok(session);
        }
        // Ended first, so two runs never share its conversation; its end is no chat's end.
        lock(&self.replacing).insert(session);
        if let Err(ended) = self.close(session) {
            tracing::warn!("purlis: a chat's first run did not end cleanly ({ended})");
        }
        // Taken back as a start that did not happen: what was announced of it goes.
        lock(&self.late_notes).remove(&session);
        lock(&self.skills_missing).remove(&session);
        lock(&self.older_in_notes).remove(&session);
        if let Some(gone) = lock(&self.never_started).as_ref() {
            gone(session);
        }
        let (program, mut command, mut args, env, mut conversation, how) = again;
        // A new conversation is started under a new id: Claude Code refuses an id whose
        // transcript the first run may have begun (measured in 2.1.300: `--session-id` is
        // refused while `<id>.jsonl` exists), and that run was never prompted, so nothing of it
        // is lost. A conversation resumed is resumed again under its own id.
        if let Some(was) = conversation.clone() {
            let fresh = purlis_core::harness::SessionId::fresh().as_str().to_owned();
            if renamed_session(&mut args, &was, &fresh)
                | renamed_session(&mut command, &was, &fresh)
            {
                conversation = Some(fresh);
            }
        }
        self.open_it_as(
            chat,
            program,
            command,
            args,
            env,
            harness,
            declared,
            conversation,
            how,
            plugins,
            sandbox,
            unsandboxed,
            size,
            operator_shell,
            why,
            sign_in,
            first_message,
            true,
        )
        .map(|(session, _)| session)
    }

    /// [`Self::open_it`] once: the session, and whether its project settings failed the first
    /// screen after its program started. `limited` starts a sandboxed Claude Code chat on the
    /// person's settings and purlis's, unwatched, whatever its project's files hold.
    #[allow(clippy::too_many_arguments)]
    fn open_it_as(
        &self,
        chat: &Chat,
        program: String,
        command: Vec<String>,
        args: Vec<String>,
        env: Vec<(String, String)>,
        harness: Option<Harness>,
        declared: Option<&purlis_core::harness_declaration::Declaration>,
        conversation: Option<String>,
        how: purlis_core::reopen::Reopened,
        plugins: &purlis_core::harness_plugin::Chosen,
        sandbox: Option<&purlis_core::sandbox::Applied>,
        unsandboxed: Option<&purlis_core::sandbox::Lifted>,
        size: Size,
        operator_shell: bool,
        why: Why,
        sign_in: Option<&purlis_core::profilesignin::AtStart>,
        first_message: Option<&str>,
        limited: bool,
    ) -> Result<(u32, bool), String> {
        // A project being let go of starts nothing more (#1949): its close may be running on
        // another thread than this start.
        if self.ending.load(Ordering::SeqCst) {
            return Err(LETTING_GO.to_owned());
        }
        // Said before anything runs, and until the chat is listed as open below: a discard of
        // its folder in the meantime is refused, and a folder that is going is no place to
        // start one (#1472).
        let _starting = self.going.starting_in(chat.cwd.as_deref())?;
        // **A chat put back that the session host still runs, or ran, from before this window
        // is reattached, not started again** (#2006): the window that started it went without a
        // quit, and the host kept it. Everything the window keeps of a running chat is made as
        // for a start; what a start begins (a run, its trust event, a first message, the
        // credentials it is handed) is not, since nothing starts: it is the run it was.
        let reattaching = self.reattaches(chat, why);
        // **A workspace set to run its chats on a pool runs them on its runner** (#2679, #2046):
        // this machine's store says where its chats run, and the host routes each start there.
        // One that names a pool this machine does not keep starts nowhere, in words, and is
        // never run on this machine instead.
        let workspace = workspace_at_start(Some(&self.project), chat.cwd.as_deref());
        #[cfg(unix)]
        if !reattaching && let Some(config) = &self.pools_in {
            purlis_core::pools::runs_on(config, &self.project, workspace.as_deref())?;
        }
        // **A harness chat on a runner is started by its names** (#2446): the runner composes
        // it, with the sandbox, hooks and line it makes for itself, and writes its run and its
        // sandbox's audit in its own event log. The window composes none of it, and carries on
        // only the conversation the chat resumes: a new one is the runner's to choose.
        let on_the_runner = !reattaching
            && !operator_shell
            && harness.is_some()
            && self
                .sessions
                .composes_its_harness_chats_in(workspace.as_deref());
        // **Whose skills it has** (SK-15g, #3604): a runner's chat is armed by its runner, and
        // so is one this window reattaches from a runner, which its start armed there too. Its
        // skills are read here for neither, and its router is not asked again.
        let armed_by_its_runner = armed_by_its_runner(on_the_runner, reattaching, || {
            !operator_shell
                && harness.is_some()
                && chat
                    .number
                    .is_some_and(|number| self.sessions.composes_the_harness_chat(number))
        });
        // A grade's chat is armed here, with its limits, or not at all (SK-16): a runner
        // composes its own line, and would start it with none of them.
        if on_the_runner && chat.graded {
            return Err(
                "A grade's chat runs on this machine alone, and this project's chats are started \
                 by a runner, so nothing was started."
                    .to_owned(),
            );
        }
        // **Its profile is kept, and signed in, on the pool's runner before it starts there**
        // (#2049): signed in, it starts with nobody asked, a moved chat's too; signed out, it
        // starts nothing and waits as one Ask to sign it in on that pool. Settings over the pool's
        // limits start nothing either, and say what to raise (spec #4275).
        if let Some(profile) = chat.profile.as_deref().filter(|_| on_the_runner) {
            purlis_core::profilestore::onrunner::forget_left_out(profile);
        }
        if on_the_runner
            && let (Some(on_pools), Some(profile)) = (&self.profiles_on_pools, &chat.profile)
            && let Some(pool) = self.sessions.pool_for(workspace.as_deref())
        {
            on_pools.ready(&self.project, &pool, profile)?;
        }
        // What its settings left out on their way there, said once on its tab (#3848, #3867).
        let left_out = chat
            .profile
            .as_deref()
            .filter(|_| on_the_runner)
            .and_then(purlis_core::profilestore::onrunner::left_out_said);
        // The services of this machine the person allowed it alone, which the runner is told
        // of: it leaves out those its own sandbox does not give, and asks about each (#2590).
        // Never a port given for a grant to serve on alone: a runner opens no door (#2019).
        let alone: Vec<u16> = sandbox
            .filter(|_| on_the_runner)
            .and_then(|applied| {
                let listen_only = applied.confines().widened.listen_only.clone();
                applied.plan().map(|plan| {
                    plan.given_for_this_chat_alone()
                        .into_iter()
                        .filter(|port| !listen_only.contains(port))
                        .collect()
                })
            })
            .unwrap_or_default();
        // Whether this window's own reading of the chat's start compiled a sandbox for it, which
        // a runner's record of its run is held to below (#2692).
        let compiled_here = sandbox.is_some();
        let sandbox = sandbox.filter(|_| !on_the_runner);
        // What it is armed with, never written by it, and never where it writes. A chat
        // reattached is not started, so it is never refused here: it is compared with the
        // sandbox it would start in now, held as a start would hold it.
        let kit_held;
        let sandbox = match sandbox {
            Some(applied) => {
                let mut held = applied.clone();
                let cwd = chat.cwd.clone().unwrap_or_else(|| self.project.clone());
                let kit = self.kit(harness);
                match held.hold_kit(kit.armed_with(), &cwd, &env) {
                    Ok(()) => {
                        kit_held = held;
                        Some(&kit_held)
                    }
                    Err(_) if reattaching => Some(applied),
                    Err(refused) => return Err(refused.to_string()),
                }
            }
            None => None,
        };
        let conversation = match &how {
            _ if !on_the_runner => conversation,
            purlis_core::reopen::Reopened::Resumed(id) => Some(id.to_string()),
            _ => None,
        };
        // The persona it runs as is the one its environment names, which the core's start put
        // there (`purlis_core::start::ready`) and every hook in the chat resolves; a chat that
        // names none runs as the project's default.
        let runs_as = env
            .iter()
            .find(|(name, _)| name == purlis_core::active::PERSONA_ENV)
            .map(|(_, persona)| persona.clone())
            .or_else(|| purlis_core::start::persona_for_a_new_chat(&self.project));
        // A key or a cloud's credentials, read for this one start under the vault's own rules
        // for the persona the chat runs as, before anything is recorded or started: a vault
        // that does not open to it starts nothing, and leaves no event and no hook behind
        // (#2036). Set on the harness's program as it starts; the commands, hooks and tools it
        // starts inherit them.
        let handed =
            match sign_in.filter(|s| !reattaching && !on_the_runner && !s.credentials.is_empty()) {
                Some(sign_in) => purlis_core::profilesignin::read(
                    &purlis_core::secrets::Ctx::new(
                        &self.project,
                        purlis_core::secrets::Env::from_process(),
                    ),
                    runs_as.as_deref(),
                    &sign_in.credentials,
                )?,
                None => purlis_core::profilesignin::Handed::default(),
            };
        // Who the chat is, and the run this start begins (ADR 0066). **The id is minted once**
        // per clone and device (V43: a copy's was minted again before `put_back` got it),
        // when no record holds one, on this device, and a chat put back or started again keeps
        // the one it had, with its origin device. Every start begins a run of its own.
        let identity = match &chat.identity.id {
            // Reattached, it is the run it was (#2006).
            Some(_) if reattaching && chat.identity.run.is_some() => chat.identity.clone(),
            Some(_) => purlis_core::reopen::Identity {
                run: Some(minted()),
                ..chat.identity.clone()
            },
            None => purlis_core::reopen::Identity {
                id: Some(minted()),
                device: self.device.clone(),
                run: Some(minted()),
                resumed_from: chat.identity.resumed_from.clone(),
            },
        };
        let cause = match why {
            Why::New => Began::Start,
            Why::Relaunch => Began::at_relaunch(&how, harness.is_some()),
            Why::Upgrade => Began::Reopen,
            Why::Again => Began::Fresh,
        };
        // What this start means for the sandbox's audit and its count (ADR 0067 §7): off where
        // it starts without the sandbox, back on where its last run did and this one does not,
        // and one more new chat towards the opt-out rate.
        let (mut trust, counted) = if reattaching || on_the_runner {
            // Nothing starts here, so nothing changes for this machine's audit or its count.
            (None, None)
        } else {
            purlis_core::sandbox::at_start(
                unsandboxed,
                sandbox.is_some(),
                chat.unsandboxed,
                why == Why::New,
            )
        };
        let mut late: Vec<String> = Vec::new();
        // An older Claude Code keeps its own proxy, and its first chat says so (#1665), marked
        // said once its tab shows it (#1699).
        let older = sandbox
            .and_then(purlis_core::sandbox::Applied::older_notice)
            .map(|(said, version)| (said.to_owned(), version));
        let older_version = older.as_ref().map(|(_, version)| *version);
        late.extend(older.map(|(said, _)| said));
        // What the project expects and this machine has not allowed (ADR 0093): said in the
        // chat's pane from its start, never first by a tool that failed.
        late.extend(sandbox.and_then(|applied| {
            purlis_core::sandbox::services::start_note(
                &self.project,
                &purlis_core::sandbox::Machine::this(),
                &applied.confines().widened,
            )
        }));
        // **Whether the plane's skills reach it** (#3445): the Skill studio's flag, read once
        // here, so its arming, its Notice, its stamp and Use skill… take the same word; a chat
        // reattached keeps its start's, where its record has it (#3466).
        let plane_skills = plane_skills_at_start(reattaching, chat.plane_skills, || {
            purlis_core::skills::reach_chats_here(&self.project)
        });
        // A declared harness no skill reaches says so on the chat's tab, once (#3585).
        late.extend(not_supported_at_start(why, plane_skills, harness, declared));
        // Where its plane skills come from, and its clone's repo skills, read once (#3234): what
        // arms it below and what its tab's Notice says are both worked out from this. Read once
        // its arming has checked the start is not refused (#3461).
        // And routed by the router its persona or workspace chose, asked now where it is an
        // extension's (SK-15b); a chat reattached routes as its start did, from its record, and
        // its router is not asked again (#3578).
        let router = lock(&self.router).clone();
        let kept_routing = routing_kept_at_start(
            reattaching,
            chat.plane_skills.is_some(),
            chat.routing.as_ref(),
        );
        let routing_was_kept = kept_routing.is_some();
        let skills = std::cell::LazyCell::new(|| {
            if armed_by_its_runner {
                return purlis_core::hoststart::ChatSkills::default();
            }
            let read = purlis_core::hoststart::ChatSkills::read(
                plane_skills,
                &self.project,
                harness,
                chat.cwd.as_deref(),
                runs_as.as_deref(),
            );
            match kept_routing {
                Some(kept) => read.routed_as(kept, harness),
                None => read.routed_by(built_in_skills_here().as_deref(), harness, router.as_ref()),
            }
        });
        // A sandboxed Claude Code chat whose project's own settings it does not load says which
        // keys, in the window only (ADR 0067 as amended 2026-10-15). Its arming read the same
        // files and decided the same way.
        if sandbox.is_some() && harness == Some(Harness::ClaudeCode) {
            late.extend(purlis_core::harness::claude::project::screen(chat.cwd.as_deref()).note());
        }
        // The profile's own command first — a wrapper reads its own words before it hands the
        // rest on (M8.3) — then the state hooks, then charter's own words: a chat's recorded
        // arguments may end in a positional prompt that nothing may come after.
        // `purlis_core::start::Ready::command_line` is the one place that order is decided.
        // A sandboxed Claude Code chat that loads its project's own settings has them watched
        // from before it starts (ADR 0067 as amended 2026-10-16). Where they cannot be, it is
        // started without them.
        let settings_watched = match chat.cwd.as_deref() {
            Some(cwd)
                if !limited
                    && !reattaching
                    && sandbox.is_some()
                    && harness == Some(Harness::ClaudeCode)
                    && purlis_core::harness::claude::project::screen(Some(cwd))
                        .loads_the_project() =>
            {
                Some(
                    self.project_settings
                        .prepare(cwd)
                        .map(|()| cwd.to_path_buf()),
                )
            }
            _ => None,
        };
        // A start its screen may put aside says its run once it goes on (#2250), and the sandbox
        // coming back on with it, held with its run in the event log (#2252): a start put aside
        // leaves nothing under its run. An opt-out is never held.
        let said = match settings_watched {
            Some(Ok(_)) => RunSaid::Held(match trust {
                Some(purlis_core::sandbox::Change::On) => trust.take().map(|change| {
                    purlis_core::eventlog::HeldTrust::of(&change, harness, chat.persona.as_deref())
                }),
                _ => None,
            }),
            _ => RunSaid::Now,
        };
        // The trust event, written before anything runs. **An opt-out is never unaudited**
        // (ADR 0067 §7): a person's is not started when it cannot be written. A system with no
        // backend still starts — every chat there would otherwise be refused — and its tab
        // says the record is missing.
        if let Some(change) = &trust
            && let Err(why) = match (
                lock(&self.trusting).as_ref(),
                identity.id.as_deref(),
                identity.run.as_deref(),
            ) {
                (Some(trusting), Some(id), Some(run)) => trusting(Sandboxing {
                    change: Some(change),
                    run: Some(RunOf { chat: id, run }),
                    counted: None,
                    harness,
                    persona: chat.persona.as_deref(),
                }),
                _ => Err("purlis's event log is not open".to_owned()),
            }
        {
            use purlis_core::sandbox::{By, Change, Lifted};
            match change {
                // A person's choice, here or as their level, is always recorded.
                Change::Off(Lifted {
                    by: By::Person | By::YourLevel(_),
                    ..
                }) => {
                    return Err(format!(
                        "purlis could not record that this chat would run without the \
                         sandbox ({why}), so it was not started. An opt-out is always \
                         recorded; start it sandboxed, or try again once the event log is \
                         back."
                    ));
                }
                Change::Off(_) => late.push(format!(
                    "purlis could not record that this chat runs without the sandbox \
                     ({why}), so the event log has no record of it."
                )),
                // Back on: the chat is sandboxed, and the record that it is can be missing.
                Change::On => tracing::warn!(
                    "purlis: a chat's sandbox came back on and was not recorded ({why})"
                ),
            }
        }
        let (hooks, armed) = if on_the_runner {
            (Vec::new(), Vec::new())
        } else {
            self.state_hooks(
                harness,
                chat.cwd.as_deref(),
                plugins,
                sandbox,
                runs_as.as_deref(),
                &env,
                // A grade's chat, as its record says (SK-16): the one word its limits take.
                chat.graded,
                &skills,
            )?
        };
        // A skill its persona names that this machine does not have (SK-5, #2768), or one it
        // would have loaded that cannot be (SK-8, #2771): the chat starts without it, and its
        // tab's Notice says which, never a silent drop.
        let skills_missing = if reattaching || on_the_runner {
            None
        } else {
            purlis_core::hoststart::skills_not_here(&skills, &self.kit(harness), harness, &env).map(
                |not| SkillsMissing {
                    persona: runs_as.clone(),
                    missing: not.missing,
                    no_built_in: not.no_built_in,
                    broken: not.broken.into_iter().map(SkillBroken::from).collect(),
                    by_a_repo: not.by_a_repo.into_iter().map(SkillBroken::from).collect(),
                    beside_a_built_in: not.beside_a_built_in,
                },
            )
        };
        let hooks = match &settings_watched {
            Some(Err(why)) => {
                tracing::warn!("purlis: a chat's project settings are not watched ({why})");
                late.push(purlis_core::harness::claude::project::UNWATCHED.to_owned());
                purlis_core::harness::claude::project::limited(
                    hooks,
                    &purlis_core::harness::claude::project::screen(chat.cwd.as_deref()),
                )?
            }
            _ if limited && sandbox.is_some() && harness == Some(Harness::ClaudeCode) => {
                let screened = purlis_core::harness::claude::project::screen(chat.cwd.as_deref());
                late.extend(screened.note());
                purlis_core::harness::claude::project::limited(hooks, &screened)?
            }
            _ => hooks,
        };
        // What the run this start begins is, for its `run.started` (ADR 0066, #868), its model
        // source its profile's sign-in kind (ADR 0087 as amended, #2036).
        let mut attributes = run_attributes(
            chat,
            harness,
            runs_as.as_deref(),
            sandbox.is_some(),
            conversation.as_deref(),
        );
        attributes.model_source = sign_in.map(|s| s.kind.word().to_owned());
        // What a wrapped chat needs running beside it, started before it and kept for as long
        // as it is open: charter's egress proxy and its own temp directory (ADR 0067 §2).
        // Its proxy tells the app each host it refuses, by the proxy's word (#1663): the
        // chat's block, under the number the chat is given below.
        let whose = Arc::new(std::sync::atomic::AtomicU32::new(0));
        // Each chat its own pair of ports (#1664): a connection's chat is the proxy it came in
        // on, and what it carried is told under that chat, never under anything it said.
        let refusals = refused_by_the_proxy(Arc::clone(&self.refused), Arc::clone(&whose), harness);
        // What its proxy refused before the chat had its number is told once it has (#1683).
        let untold = refusals.clone();
        let who = ReachedAs {
            id: identity.id.clone(),
            persona: runs_as.clone(),
        };
        let reached =
            reached_by_the_proxy(Arc::clone(&self.reached), Arc::clone(&whose), who.clone());
        // The live ask (#1666): a connection to a host nothing lists is held while the person
        // is asked, unless an administrator's policy turns that off.
        let asks = (!purlis_core::sandbox::policy::Locks::of(&self.project).forbids_live_asks())
            .then(|| {
                asked_by_the_proxy(
                    Asking {
                        hear: Arc::clone(&self.refused),
                        reached: Arc::clone(&self.reached),
                        whose: Arc::clone(&whose),
                        who,
                        harness,
                        blocks: Arc::clone(&self.blocks),
                        moved: Arc::clone(&self.asks_moved),
                    },
                    self.project.clone(),
                )
            });
        // Started where the chat runs, from the plan its sandbox compiled to: in the window, or
        // in the session host, which holds it for as long as the chat runs (#1999). A chat
        // reattached runs in the one it ran in, heard by this window from now on (#2006).
        // **What a chat reattached runs with** (D-2006-11): the sandbox it would start in now, where
        // that is the plan its confinement in the host was started from; otherwise none, so its
        // sandbox reads as one that changed while no window was there, and it is offered a
        // restart on the sandbox of now as any chat a change left behind is (#1428).
        // **How its run is sandboxed** (#2636): by the host's record of a run reattached, read
        // before it is taken back; by this window's own of one it starts. A run the host has no
        // record of is taken as sandboxed, with no sandbox of now standing in for it, so it is
        // offered a restart (#1428); a run that started without the sandbox has none.
        let mut ran = match (reattaching, chat.number) {
            (true, Some(number)) => self.sessions.ran_in(number),
            // The runner composes it, with the sandbox it compiles there: its record is the
            // runner's, which its start answers, below (#2692).
            _ if on_the_runner => None,
            _ => Some(ran_as_started(sandbox, harness)),
        };
        let confines = match (reattaching, chat.number, &ran) {
            (true, Some(number), Some(purlis_session_protocol::session::RunSandbox::By(_))) => {
                let now = sandbox.and_then(purlis_core::sandbox::Applied::plan);
                let then = self.sessions.ran_with(number);
                (now == then)
                    .then(|| sandbox.map(|applied| applied.confines().clone()))
                    .flatten()
            }
            (true, _, _) => None,
            _ => sandbox.map(|applied| applied.confines().clone()),
        };
        let hearing = purlis_core::sandbox::plan::Hearing {
            refusals,
            more: Some(more_refused_by_the_proxy(
                Arc::clone(&self.refused_more),
                Arc::clone(&whose),
            )),
            reached: Some(reached),
            asks,
        };
        // On a runner, the runner confines it, and its proxy is heard as the one it names.
        let (hearing, composed_hearing) = if on_the_runner {
            (
                purlis_core::sandbox::plan::Hearing::default(),
                Some(hearing),
            )
        } else {
            (hearing, None)
        };
        let mut confinement = match sandbox.and_then(purlis_core::sandbox::Applied::plan) {
            _ if on_the_runner => None,
            _ if reattaching => chat
                .number
                .and_then(|number| self.sessions.confined_again(number, hearing))
                .transpose()?,
            Some(plan) => Some(self.sessions.confine(&plan, hearing).map_err(|err| {
                purlis_core::sandbox::under_policy(
                    &purlis_core::sandbox::policy::Locks::of(&self.project),
                    format!(
                        "{}, and purlis could not start what the sandbox needs beside \
                                 this chat ({err}), so nothing was started.",
                        purlis_core::sandbox::LEAD
                    ),
                )
            })?),
            None => None,
        };
        // The doors to the servers this chat may run (#2019), opened in this process wherever
        // its proxy runs, before its line is compiled: a socket for each, in its own folder. A
        // port it serves on none of is told, and withdrawn from its proxy where only its listen
        // grant gave it.
        let claude_code = sandbox
            .is_some_and(|applied| applied.harness() == purlis_core::harness::Harness::ClaudeCode);
        // A Claude Code chat's server dials in instead (#3316): it can make no socket of its
        // own, so its doors carry to connections it parks through its hook socket. macOS alone,
        // where its hook socket is reachable from its sandbox.
        if let (Some(confinement), Some(confines)) = (confinement.as_mut(), confines.as_ref())
            && claude_code
            && cfg!(target_os = "macos")
        {
            let widened = &confines.widened;
            let mut unserved = widened.unserved.clone();
            match confinement.dial(widened) {
                Ok(more) => unserved.extend(more),
                Err(why) => {
                    tracing::warn!("purlis: a chat's servers were not served ({why})");
                    late.extend(purlis_core::sandbox::door::unservable_said(
                        &widened.listens,
                        &why.to_string(),
                    ));
                }
            }
            late.extend(purlis_core::sandbox::door::unserved_said(&unserved));
        }
        if let (Some(confinement), Some(confines)) = (confinement.as_mut(), confines.as_ref())
            && !claude_code
        {
            let widened = &confines.widened;
            let mut unserved = widened.unserved.clone();
            if !widened.listens.is_empty() {
                let folder = identity
                    .id
                    .as_deref()
                    .and_then(purlis_core::sandbox::door::folder_for);
                let served = match folder {
                    Some(folder) => confinement
                        .serve(&folder, widened)
                        .map_err(|err| err.to_string()),
                    None => {
                        for port in &widened.listen_only {
                            let _ = confinement.withdraw_port(*port);
                        }
                        Err("it has no folder of its own to serve in".to_owned())
                    }
                };
                match served {
                    Ok(more) => unserved.extend(more),
                    Err(why) => {
                        tracing::warn!("purlis: a chat's servers were not served ({why})");
                        late.extend(purlis_core::sandbox::door::unservable_said(
                            &widened.listens,
                            &why,
                        ));
                    }
                }
            }
            late.extend(purlis_core::sandbox::door::unserved_said(&unserved));
        }
        // A runner starts the chat without the services you allowed for this chat alone on this
        // machine, and the tab says so, and whether the chat is asked about them there (#2510).
        late.extend(
            confinement
                .as_ref()
                .and_then(purlis_core::sandbox::Confinement::said_of_what_was_left_out),
        );
        // Under a sandbox, the sandbox decides the line: a flag of the harness's own in the
        // chat's own words can outrank it, so such a chat is refused here, where every chat
        // opens, and flags it rides on go last among the flags. A harness charter wraps runs as
        // the wrap's program, with its whole line after it (ADR 0067).
        let socket = self.sessions.reports_to();
        // **A task starts in its asking chat's mode, capped at auto** (#3787): the words its
        // harness's adapter adds for the level its record holds, among purlis's own. None at
        // the default, none where the profile's command names a mode of its own, none for a
        // grade's chat, and none on the runner, which arms its chats itself.
        let mut hooks = hooks;
        if let Some(harness) = harness.filter(|_| !on_the_runner) {
            hooks.extend(purlis_core::dispatchmode::task_words(
                harness,
                chat.task_level,
                &command,
                chat.graded,
            ));
        }
        // A chat reattached starts nothing: no line is built for it, so none refuses it (#2006).
        let (program, all, wrapped) = match sandbox.filter(|_| !reattaching) {
            Some(applied) => {
                let line = applied.line(
                    purlis_core::sandbox::Words {
                        program,
                        command,
                        armed: hooks,
                        charters: args,
                    },
                    &purlis_core::sandbox::At {
                        cwd: chat.cwd.as_deref(),
                        hook_socket: socket.as_deref(),
                        confinement: confinement.as_ref(),
                        no_opt_out: false,
                        // purlis's own program as the start checked it (#1040).
                        purlis: applied.wrap_program(),
                    },
                )?;
                (line.program, line.args, line.env)
            }
            None => (
                program,
                purlis_core::start::Ready::line(command, hooks, args),
                Vec::new(),
            ),
        };
        // Settings too long for one word of a command line go through a file of their own (SK-9).
        let all = match identity.id.as_deref() {
            Some(id) if !reattaching => purlis_core::hoststart::settings_in_a_file(
                self.kit(harness).chat_settings.as_deref(),
                &self.project,
                id,
                all,
            )?,
            _ => all,
        };
        let mut env = env;
        env.extend(armed);
        // What the wrap sets wins over a profile's or the arming's value of the same name: its
        // proxy, its temp directory. Sorting cannot decide it, since the session applies the
        // pairs in order and two of one name would leave the later one standing.
        env.retain(|(key, _)| !wrapped.iter().any(|(set, _)| set == key));
        env.extend(wrapped);
        // A cluster on this Mac the person pointed this chat's kubectl at (qw384b): its own
        // copy, in its own folder, named for it alone and ahead of any kubeconfig of theirs.
        if let (false, Some(config), Some(confinement)) = (
            reattaching,
            identity.id.as_deref().and_then(|id| self.kubeconfig_of(id)),
            confinement.as_mut(),
        ) {
            match confinement.keep_own(purlis_core::sandbox::kubeconfig::FILE, &config) {
                Ok(path) => {
                    env.retain(|(key, _)| key != purlis_core::sandbox::kubeconfig::ENV);
                    env.push((
                        purlis_core::sandbox::kubeconfig::ENV.to_owned(),
                        path.display().to_string(),
                    ));
                }
                Err(err) => tracing::warn!("purlis: a chat's kubeconfig was not written ({err})"),
            }
        }
        // Where its servers listen (#2019), for `purlis listen` to name each one's socket.
        if let Some((folder, _)) = confinement.as_ref().and_then(|it| it.served()) {
            env.retain(|(key, _)| key != purlis_core::sandbox::door::FOLDER_ENV);
            env.push((
                purlis_core::sandbox::door::FOLDER_ENV.to_owned(),
                folder.display().to_string(),
            ));
        }
        // Or the hook socket its servers dial in to (#3316), for `purlis listen`'s shim.
        if let Some(hook) = socket
            .as_deref()
            .filter(|_| confinement.as_ref().is_some_and(|it| it.dials_in()))
        {
            env.retain(|(key, _)| key != purlis_core::sandbox::door::DIAL_ENV);
            env.push((
                purlis_core::sandbox::door::DIAL_ENV.to_owned(),
                hook.display().to_string(),
            ));
        }
        what_its_hooks_read(
            &mut env,
            chat.cwd.as_deref(),
            sandbox.is_some(),
            identity.id.as_deref(),
        );
        env.sort();
        // The app's own `charter` first, then the directories charter searched for the
        // harness — so a hook the plane spells as the bare word `charter`, or a skill's
        // command, finds the one this app shipped, from a Finder launch too (charter-app#136).
        let env = purlis_core::start::with_chat_path(env, self.shipped.binary.as_deref());
        // ssh through the chat's SOCKS port (#1667): its route's `ssh` first on the chat's PATH,
        // for a harness its sandbox wraps whole.
        let env = match sandbox.and_then(|applied| applied.ssh_route(confinement.as_ref())) {
            Some(route) => route.first_on_path(env),
            None => env,
        };
        // What the announcement below said, so a start that fails can take it back.
        let announced = std::sync::atomic::AtomicU32::new(0);
        let announce = |session: u32| {
            announced.store(session, std::sync::atomic::Ordering::SeqCst);
            whose.store(session, std::sync::atomic::Ordering::SeqCst);
            lock(&self.starting_as).insert(session, chat.persona.clone());
            untold.tell_untold();
            // A chat reattached is where it was: its state, its model and its run are as the
            // host kept them (#2006).
            if reattaching {
                if let Some(reattached) = lock(&self.reattached).as_ref() {
                    reattached(session, harness, conversation.clone(), chat.graded);
                }
                return;
            }
            // A new program knows no model until its harness says (#1021).
            lock(&self.models).remove(&session);
            if let Some(starting) = lock(&self.starting).as_ref() {
                starting(session, harness, conversation.clone(), chat.graded);
            }
            // A run on a runner is written there, where it runs, never here.
            if let Some(beginning) = lock(&self.beginning).as_ref().filter(|_| !on_the_runner)
                && let (Some(chat), Some(run)) = (&identity.id, &identity.run)
            {
                beginning(
                    session,
                    RunOf { chat, run },
                    cause,
                    &attributes,
                    said.clone(),
                );
            }
        };
        // A person's own opt-out of a chat the runner composes (#2446).
        let runner_without_sandbox = unsandboxed.is_some_and(|lifted| {
            matches!(
                lifted.by,
                purlis_core::sandbox::By::Person | purlis_core::sandbox::By::YourLevel(_)
            )
        });
        // **Its prompt key** (#3369): the one its host kept, for a chat reattached; the one the
        // runner mints, for a chat it composes (answered below); a fresh one for a harness chat
        // this window composes. Never on its line and never in a record.
        let mut prompt_key = match (reattaching, chat.number) {
            (true, Some(number)) => self.sessions.prompt_key_left(number),
            _ if on_the_runner || harness.is_none() => None,
            _ => Some(purlis_core::skillspan::PromptSalt::mint().map_err(|why| {
                format!(
                    "purlis could not make this chat's prompt key ({why}), so it was not started"
                )
            })?),
        };
        // A runner chat's workspace clones as it starts (#2053), read before any lock is taken.
        let mut clones_at_start = (on_the_runner && !reattaching)
            .then(|| purlis_core::forge::apptoken::clones_now(&self.project));
        // Before the program is started, so its first mint covers them (#4331).
        #[cfg(unix)]
        self.puts_its_repos_on_its_claim(
            workspace_at_start(Some(&self.project), chat.cwd.as_deref()).as_deref(),
            chat.cwd.as_deref(),
            // A start its runner is sure to refuse adds nothing to the Claim.
            clones_at_start
                .as_deref()
                .filter(|_| !runner_without_sandbox),
        );
        let opened = match (reattaching, chat.number) {
            (true, Some(number)) => self.sessions.reattach(number, &announce),
            _ if on_the_runner => self
                .sessions
                .open_named(
                    chat.number,
                    &crate::host::Named {
                        chat: identity.id.clone(),
                        run: identity.run.clone(),
                        profile: chat.profile.clone(),
                        persona: runs_as.clone(),
                        workspace: workspace_at_start(Some(&self.project), chat.cwd.as_deref()),
                        resume: conversation.clone(),
                        first_message: first_message.map(str::to_owned),
                        // A person's own opt-out, which a runner takes from no link but its own
                        // window's: such a start is refused there, never run sandboxed silently.
                        without_sandbox: runner_without_sandbox,
                        // A chat held from its persona's grants is armed with none of them.
                        tools_allowed: runs_as.as_deref().filter(|_| chat.held.is_none()).and_then(
                            |persona| {
                                purlis_core::sandbox::tools::allowed_digest_here(
                                    &self.project,
                                    persona,
                                )
                            },
                        ),
                        // The Skill studio's flag this start read (#3445): a runner arms it by
                        // this, not by its own machine's (SK-20b, #3084).
                        plane_skills,
                        alone,
                        size,
                    },
                    composed_hearing.unwrap_or_default(),
                    &announce,
                )
                .map(|composed| {
                    // How the runner applied the sandbox to the run it composed, by the runner
                    // host's own record as its start answered it (#2692): none from a runner
                    // that keeps no such record, which is taken as sandboxed. Its word that the
                    // run is off is taken only where this window's own reading starts the chat
                    // without a sandbox too: any other is no record, so this window never
                    // allows a chat more on a runner's word alone.
                    ran = composed.ran.filter(|ran| {
                        *ran != purlis_session_protocol::session::RunSandbox::Off || !compiled_here
                    });
                    confinement = composed.confinement;
                    prompt_key = composed.prompt_key;
                    composed.number
                })
                .map_err(|why| {
                    self.in_the_pools_words(why, chat.profile.as_deref(), workspace.as_deref())
                }),
            _ => self.sessions.open(
                // The number this chat already answers to, where it has one. A chat put
                // back keeps the key its workspace pointer and session lock are under; a
                // chat the operator just started has none yet (charter-app#90).
                chat.number,
                // Every field spelled out on purpose, with no `..Opening::default()`: a new
                // field is decided here, where a start's real value comes from (#2453).
                &Opening {
                    program: Some(program),
                    args: all,
                    cwd: chat.cwd.as_ref().map(|cwd| cwd.display().to_string()),
                    size,
                    env,
                    // Every identity variable a vault of this chat's plane declares, so none
                    // reaches the chat even when it is not `OP_`-prefixed (#271 review, U6). Read
                    // from the plane the chat starts in; a chat outside a plane declares none.
                    env_strip: declared_identity_vars(&self.project),
                    harness,
                    // A declared harness's own namespace (#967), each variable judged as the
                    // session composes the chat's environment (#1013).
                    declared_env: declared.map(|d| d.env.clone()).unwrap_or_default(),
                    env_pass: operator_env_pass(&self.project),
                    operator_shell,
                    git_hooks: self.git_hooks_for(harness),
                    handed,
                    confinement: confinement
                        .as_ref()
                        .and_then(purlis_core::sandbox::Confinement::held_as)
                        .map(str::to_owned),
                    // The run this start begins, which its `run.started` is written under.
                    run: identity.run.clone(),
                    // A grade's chat ends with this window (#3274): only it watches the chat.
                    ends_with_its_window: chat.graded,
                    prompt_key: prompt_key.clone(),
                },
                &announce,
            ),
        };
        let session = opened.inspect_err(|_| {
            // The chat was announced and then did not start. Take it back, or the board
            // holds one entry per failed start for the life of the app.
            let announced = announced.load(std::sync::atomic::Ordering::SeqCst);
            if announced > 0 {
                lock(&self.starting_as).remove(&announced);
            }
            if announced > 0
                && let Some(gone) = lock(&self.never_started).as_ref()
            {
                gone(announced);
            }
        })?;
        // The key its host set in its environment, where the window launched it: a host that
        // answers none set none, and its prompts are marked under none (#3369).
        if !reattaching && !on_the_runner {
            prompt_key = self.sessions.prompt_key_started(session, prompt_key);
        }
        // Under the id it was actually given, not the one it was recorded with: a chat
        // started fresh is under an id the app just chose, and that is what has to be
        // written down for the next launch to resume it.
        // And no longer as a chat a workspace rename left without its conversation: it has
        // said so once, at this start, and is under a conversation of its own now.
        let under = Chat {
            resume: conversation
                .as_deref()
                .and_then(|id| purlis_core::harness::SessionId::new(id).ok()),
            renamed_from: None,
            // What this run started under, for the next run's audit and the session record —
            // never read as an opt-out by any start. A chat reattached runs as it started.
            unsandboxed: if reattaching {
                chat.unsandboxed
            } else {
                unsandboxed.is_some()
            },
            identity,
            // The Skill studio's flag this start took (#3466): what its record keeps, and what
            // a later reattach of it takes.
            plane_skills: Some(plane_skills),
            // How this start routed its skills (#3578): what its record keeps, and what a later
            // reattach of it routes by. A runner's chat is armed by its runner, so none (#3604).
            routing: (!armed_by_its_runner)
                .then(|| purlis_core::skills::router::Kept::of(skills.routed.as_deref())),
            ..chat.clone()
        };
        // What a start says on its tab was said at the start it reattached to (#2006); what
        // the host could not hand over of it is said instead.
        match self.sessions.state_not_reattached().filter(|_| reattaching) {
            Some(why) => lock(&self.untaken).insert(session, why),
            None => lock(&self.untaken).remove(&session),
        };
        if let Some(said) = left_out.filter(|_| lock(&self.left_out_told).insert(session)) {
            late.push(said);
        }
        if !late.is_empty() && !reattaching {
            lock(&self.late_notes).insert(session, late);
        }
        match skills_missing {
            Some(missing) => lock(&self.skills_missing).insert(session, missing),
            None if !reattaching => lock(&self.skills_missing).remove(&session),
            None => None,
        };
        if let Some(version) = older_version.filter(|_| !reattaching) {
            lock(&self.older_in_notes).insert(session, version);
        }
        let workspace = workspace_at_start(Some(&self.project), under.cwd.as_deref());
        // Read before any lock is taken: a stamp is a read of the plane's skill folders.
        let stamp = harness.map(|harness| {
            purlis_core::skills::stamp_now(&purlis_core::skills::Running {
                plane: &self.project,
                workspace: workspace.as_deref(),
                persona: under.persona.as_deref(),
                harness,
                built_in: built_in_skills_here().as_deref(),
                plane_skills,
                start: Some(&skills),
            })
        });
        // A start under this number has been told nothing yet (#1450).
        lock(&self.told).remove(&session);
        lock(&self.starting_as).remove(&session);
        lock(&self.ended).remove(&session);
        lock(&self.open).insert(
            session,
            Running {
                chat: under,
                how,
                harness,
                declared: declared.cloned(),
                workspace,
                confinement,
                confines,
                started_with: StartedWith {
                    held: chat.held.clone(),
                    grants: self.grants_of(chat),
                },
                ran,
                composed_by_its_host: on_the_runner,
                grants_as: grants_persona_of(chat, &self.project),
                skills: stamp,
                skills_read: (*skills).clone(),
                last_prompt: None,
                plane_skills,
                clones_at_start: clones_at_start.take(),
                prompt_key,
                runs_as: runs_as.clone(),
            },
        );
        // Its routing record, after its run began (SK-15b), where its router was asked now. A chat
        // reattached began no run here: one that kept its start's routing is in its run's record
        // already (#3578), and one whose record kept none had its router asked again, an answer
        // recorded under the run it is in.
        if !armed_by_its_runner
            && !routing_was_kept
            && let Some(routed) = skills.routed.as_ref()
            && let Some(tell) = lock(&self.routed).as_ref()
        {
            tell(session, &routed.record);
        }
        lock(&self.sizes).insert(session, size);
        // Screened once more now its program runs: a change since the screen its arming made is
        // answered to the start, which starts it again on the person's settings.
        let failed = match &settings_watched {
            Some(Ok(cwd)) => !self.project_settings.chat(session, cwd),
            _ => false,
        };
        // A start put aside here is no run and no chat (#2250): the one started in its place
        // is said and counted.
        if matches!(said, RunSaid::Held(_))
            && let Some(settled) = lock(&self.settled).as_ref()
        {
            settled(
                session,
                if failed {
                    HeldStart::TakenBack
                } else {
                    HeldStart::GoesOn
                },
            );
        }
        if !failed {
            // Counted once it has started, so a start that failed is not a chat. A count that
            // could not be kept costs the rate one chat, never the chat.
            if let (Some(counted), Some(trusting)) = (counted, lock(&self.trusting).as_ref())
                && let Err(why) = trusting(Sandboxing {
                    change: None,
                    run: None,
                    counted: Some(counted),
                    harness,
                    persona: chat.persona.as_deref(),
                })
            {
                tracing::warn!("purlis: a chat was not counted for the sandbox ({why})");
            }
        }
        self.write_it_down();
        // A close that began while this started, past the check above, has ended every chat
        // it found: this one is ended as well, so nothing runs in a project nobody holds.
        if self.ending.load(Ordering::SeqCst) {
            let _ = self.close(session);
            return Err(LETTING_GO.to_owned());
        }
        Ok((session, failed))
    }

    /// **Whether chat `session` is a grade's** (SK-16), which the window draws nowhere: its
    /// record's [`purlis_core::reopen::Chat::graded`].
    pub fn kept_from_the_window(&self, session: u32) -> bool {
        lock(&self.open)
            .get(&session)
            .is_some_and(|one| one.chat.graded)
    }

    /// Ends a chat. It is no longer one a quit would record.
    pub fn close(&self, session: u32) -> Result<(), String> {
        lock(&self.starting_as).remove(&session);
        lock(&self.untaken).remove(&session);
        lock(&self.ended).remove(&session);
        let gone = lock(&self.open).remove(&session);
        // A Try it's scratch chat takes its throwaway piece with it, where no other open chat
        // stands in it (SK-9): a restart starts before the chat it replaces ends.
        if let Some(cwd) = gone.as_ref().and_then(|gone| gone.chat.cwd.clone())
            && !lock(&self.open)
                .values()
                .any(|one| one.chat.cwd.as_deref() == Some(cwd.as_path()))
        {
            crate::skilldrafts::closed_in(&self.project, &cwd);
        }
        self.project_settings.forget(session);
        lock(&self.sizes).remove(&session);
        lock(&self.owed).remove(&session);
        lock(&self.models).remove(&session);
        lock(&self.told).remove(&session);
        lock(&self.records).remove(&session);
        self.blocks.ended(session);
        // The GitHub App tokens minted for it are revoked, in the background (#3808).
        purlis_core::forge::apptoken::chat_ended(&self.project, session);
        if let Some(gone) = gone {
            // A chat's grants end with it (D-1348-1): kept only while a session of that chat is
            // open, which a restart for a grant is, since it starts before the old one ends.
            if let Some(id) = gone.chat.identity.id.as_deref() {
                // What was allowed once in this session was for the run that took it: the
                // restart that took it has started by now, and no later start has it (D-sock-3).
                if let Some(made) = lock(&self.grants).get_mut(id) {
                    made.retain(|one| !(one.once && one.from == session));
                }
                let still = lock(&self.open)
                    .values()
                    .any(|one| one.chat.identity.id.as_deref() == Some(id));
                if !still {
                    let mut grants = lock(&self.grants);
                    grants.remove(id);
                    self.took_back(id);
                }
            }
            let mut let_go = lock(&self.let_go);
            if let_go.len() >= LET_GO_HELD {
                let_go.clear();
            }
            let_go.insert(session, LetGo::of(&gone.chat));
        }
        // A chat still to start keeps its place in front (#3114: one whose start returned into
        // a quit is ended, and is still to start under the same number).
        let still_coming = lock(&self.coming).iter().any(|one| one.number == session);
        let mut front = lock(&self.front);
        if *front == Some(session) && !still_coming {
            *front = None;
        }
        drop(front);
        let closed = self.sessions.close(session);
        self.write_it_down();
        // The folder of links to the plane's skills it used, once its program has ended, where
        // no other running chat uses it (SK-24).
        purlis_core::skills::tidy_here_later(purlis_core::skills::TIDY_AFTER_A_CLOSE);
        closed
    }

    /// Which chat is in front, or none.
    pub fn front(&self) -> Option<u32> {
        *lock(&self.front)
    }

    /// **The chats the person is looking at**: every chat on screen in the tab in front, in
    /// the order of their numbers, and none when no chat's tab is in front.
    ///
    /// A tab is its session's, so "in front" names the tab's own chat whatever its panes
    /// draw; a tab that opened on a view stands for the first chat beside it, and the window
    /// says every other chat there is beside that one (#1525). On screen with it is every chat that has a pane in that tab ([`Self::tab_shows`]
    /// says which, #1489), and each pane draws its own chat or the chat it was switched to
    /// (#1486). So this is: the chat in front, or the task its pane shows in place of it; and
    /// for every chat beside it, the same. A task in a tab of its own is in front itself.
    ///
    /// The one answer for both things that turn on it: a reported task is not ended under the
    /// person reading it (#1485), and no notification is sent about a chat already on screen.
    /// A chat hidden behind a task its pane shows is not looked at. Whether the window has the
    /// keyboard is not asked here.
    pub fn looked_at(&self) -> Vec<u32> {
        let Some(front) = self.front() else {
            return Vec::new();
        };
        let open = lock(&self.open);
        let mut seen: Vec<u32> = open
            .iter()
            .filter(|(number, one)| **number == front || one.chat.beside == Some(front))
            .map(|(number, one)| one.chat.shows.unwrap_or(*number))
            .collect();
        // In front and not open: nothing more is known of it, and it is what is in front.
        if !open.contains_key(&front) {
            seen.push(front);
        }
        seen.sort_unstable();
        seen.dedup();
        seen
    }

    /// Whether chat `session` is on screen in the tab in front ([`Self::looked_at`]).
    pub fn looks_at(&self, session: u32) -> bool {
        self.looked_at().contains(&session)
    }

    /// Pins or unpins one chat, and writes the record so the pin outlives the app.
    ///
    /// **A chat charter does not have open cannot be pinned**, and the answer says so rather
    /// than inventing an entry: a pin is an arrangement of what is there, and the record is
    /// the only thing that says a chat exists at all (ADR 0040). It follows that a
    /// pinned chat that does not come back at a launch takes its pin with it, which is the
    /// dangling-pin question answered by there being nowhere for one to dangle.
    ///
    /// Nothing is written when nothing changed, for the reason `bring_to_front` gives: the
    /// record is rewritten on every write, and a write is a fingerprint the machine store
    /// then has to vouch for.
    pub fn pin(&self, session: u32, pinned: bool) -> Result<(), String> {
        let mut open = lock(&self.open);
        let Some(one) = open.get_mut(&session) else {
            return Err(format!("purlis has no chat {session} open to pin."));
        };
        if one.chat.pinned == pinned {
            return Ok(());
        }
        one.chat.pinned = pinned;
        drop(open);
        self.write_it_down();
        Ok(())
    }

    /// The person opened chat `session`'s tab (#1447, #1489): a task chat, listed until now,
    /// has a tab from here on, across a reload and a relaunch, until it is sent back
    /// ([`Self::close_tab`]). Written only when it changes, for [`Self::pin`]'s reason, and
    /// only for a chat that had none.
    pub fn open_tab(&self, session: u32) -> Result<(), String> {
        let mut open = lock(&self.open);
        let Some(one) = open.get_mut(&session) else {
            return Err(format!("purlis has no chat {session} open to show."));
        };
        if one.chat.has_tab() {
            return Ok(());
        }
        one.chat.tab_opened = true;
        drop(open);
        self.write_it_down();
        Ok(())
    }

    /// **The person sent task chat `session`'s tab back to the Chats list** (#1488, V100-38):
    /// the other half of [`Self::open_tab`]. It has no tab from here on, across a reload and a
    /// relaunch, and **nothing of the task changes**: its program runs, it owes what it owed,
    /// and nobody is told anything. Only a task has this: every other chat is its tab, and
    /// taking that away is ending it, which this never does. Written only when it changes.
    pub fn close_tab(&self, session: u32) -> Result<(), String> {
        let mut open = lock(&self.open);
        let Some(one) = open.get_mut(&session) else {
            return Err(format!("purlis has no chat {session} open."));
        };
        let task = one
            .chat
            .from
            .as_ref()
            .is_some_and(|from| from.mode == purlis_core::reopen::Mode::Task);
        if !task {
            return Err(
                "Only a task goes back to the Chats list. This chat is not one: closing its \
                 tab ends it."
                    .to_owned(),
            );
        }
        if !one.chat.tab_opened {
            return Ok(());
        }
        one.chat.tab_opened = false;
        drop(open);
        self.write_it_down();
        Ok(())
    }

    /// **Where chat `session`'s pane is and what it shows** (#1486, #1489): chat `shown` in
    /// place of its own, or its own again with `None`; and the chat whose tab the pane is in,
    /// `beside`, or `None` for a chat that is its tab's own or has no pane. The record keeps
    /// both, so a reloaded window and the next launch put each pane back. Written only when
    /// one changes, for [`Self::pin`]'s reason.
    ///
    /// **What is kept is the window's word, and it says only where the person is looking**
    /// ([`Self::looked_at`]): the core starts and allows nothing by these numbers. A reported
    /// task's end, already due, is held while they say the person is reading it, and no
    /// notification is sent about a chat they say is on screen. The window shows a chat only
    /// where the chat named is open and below this one. A chat is never said to show itself,
    /// or to be beside itself.
    pub fn tab_shows(
        &self,
        session: u32,
        shown: Option<u32>,
        beside: Option<u32>,
    ) -> Result<(), String> {
        let shown = shown.filter(|other| *other != session);
        let beside = beside.filter(|other| *other != session);
        let mut open = lock(&self.open);
        let Some(one) = open.get_mut(&session) else {
            return Err(format!(
                "purlis has no chat {session} open to show another in."
            ));
        };
        if one.chat.shows == shown && one.chat.beside == beside {
            return Ok(());
        }
        one.chat.shows = shown;
        one.chat.beside = beside;
        drop(open);
        self.write_it_down();
        Ok(())
    }

    /// Gives a chat the name `raw`, or takes the one it was given off when `raw` is blank —
    /// and answers the name it now has (charter-app#254).
    ///
    /// **Charter's label and nothing else**: the harness keeps the name it was started with
    /// (`Chat::name`), so a rename never reaches a program that is running. The name is held to
    /// [`purlis_core::reopen::label`], and a refusal changes nothing and says why.
    ///
    /// Nothing is written when nothing changed, for [`Self::pin`]'s reason.
    pub fn rename(&self, session: u32, raw: &str) -> Result<Option<String>, String> {
        let label = purlis_core::reopen::label(raw)?;
        let mut open = lock(&self.open);
        let Some(one) = open.get_mut(&session) else {
            return Err(format!("purlis has no chat {session} open to rename."));
        };
        if one.chat.label == label {
            return Ok(label);
        }
        one.chat.label.clone_from(&label);
        drop(open);
        self.write_it_down();
        Ok(label)
    }

    /// The name `session` is shown under — the one it was given, or its default — or `None`
    /// for a chat charter does not have open (charter-app#258).
    pub fn shown_name(&self, session: u32) -> Option<String> {
        let open = lock(&self.open);
        let one = open.get(&session)?;
        Some(purlis_core::reopen::shown_name(
            &one.chat,
            one.harness.map(Harness::name),
        ))
    }

    /// The harness `session` was started as, or none for a shell or a chat charter does not
    /// have open — the same one [`Self::open_now`] answers, never one inferred from the
    /// program's name.
    pub fn harness(&self, session: u32) -> Option<Harness> {
        lock(&self.open).get(&session)?.harness
    }

    /// What chat `session`'s pane sends for Shift+Enter (SI-4): the `[terminal] newline` of the
    /// declaration it started on, a project's or this machine's (#967, #968), else its
    /// harness's newline ([`Harness::newline`]), and none for a shell, a declaration that names
    /// none, or a chat charter does not have open, which keep the terminal's own Enter.
    pub fn newline(&self, session: u32) -> Option<String> {
        let open = lock(&self.open);
        let one = open.get(&session)?;
        match (&one.declared, one.harness) {
            (Some(declared), _) => declared.terminal.newline.clone(),
            (None, Some(harness)) => Some(harness.newline().to_owned()),
            (None, None) => None,
        }
    }

    /// The handoff `session` was opened by, where one opened it.
    pub fn handed_from(&self, session: u32) -> Option<purlis_core::reopen::HandedFrom> {
        lock(&self.open).get(&session)?.chat.from.clone()
    }

    /// Records what `session` owes the chat that handed it off, and writes the record so it
    /// holds across a relaunch (charter-app#259). Nothing for a chat no handoff opened.
    pub fn owes(&self, session: u32, owed: purlis_core::reopen::Owed) {
        let mut open = lock(&self.open);
        let Some(from) = open
            .get_mut(&session)
            .and_then(|one| one.chat.from.as_mut())
        else {
            return;
        };
        if from.report == owed {
            return;
        }
        from.report = owed;
        drop(open);
        self.write_it_down();
    }

    /// **Where chat `session` is working** (#1450): who asked for it, its sibling tasks and the
    /// other chats running as its persona, drawn from this app's own record of every chat it
    /// has open in the project ([`purlis_core::awareness`]), with each one's state, and whether
    /// it waits on work of its own (#3878), as `state_of` has them on the board, and what each
    /// is stopped on for the person as `asking_of` has it (a task of `session`'s that waits on
    /// the person is said once, at its next turn). `tell` says whether the chat counts as told from now, which is kept
    /// here. `None` for a chat that is not open.
    ///
    /// Nothing but `session` comes from the chat that asks, and the answer has a chat's name,
    /// persona, workspace, state and start: never its arguments, where a brief travels.
    pub fn working(
        &self,
        session: u32,
        tell: purlis_core::awareness::Tell,
        state_of: &dyn Fn(u32) -> (purlis_core::state::State, bool),
        asking_of: &dyn Fn(u32) -> Option<purlis_core::awareness::Prompt>,
    ) -> Option<purlis_core::awareness::Working> {
        use purlis_core::awareness::{Asker, Known};
        // The board is asked once `open` is let go: nothing waits on both.
        let mut known: Vec<Known> = lock(&self.open)
            .iter()
            .map(|(number, one)| Known {
                chat: *number,
                name: purlis_core::reopen::shown_name(&one.chat, one.harness.map(Harness::name)),
                persona: one.chat.persona.clone(),
                workspace: one.workspace.clone().map_or(
                    purlis_core::active::Place::PlaneRoot,
                    purlis_core::active::Place::Workspace,
                ),
                state: purlis_core::state::State::Unknown,
                own_work: false,
                started: purlis_core::awareness::run_started(&one.chat.identity),
                // The lineage it is in, as a dispatch counts it (#1455).
                lineage: one
                    .chat
                    .from
                    .as_ref()
                    .map_or_else(|| one.chat.identity.id.clone(), |from| from.root.clone()),
                from: one.chat.from.as_ref().map(|from| Asker {
                    chat: from.chat,
                    name: from.name.clone(),
                    reported: from.report.reported(),
                    mode: from.mode.into(),
                    owes: from.report == purlis_core::reopen::Owed::Due,
                    report: from.report,
                    long: from.long,
                }),
                asking: None,
            })
            .collect();
        for one in &mut known {
            (one.state, one.own_work) = state_of(one.chat);
            one.asking = asking_of(one.chat);
        }
        // Only a chat that is open has anything kept of what it was told.
        known.iter().find(|one| one.chat == session)?;
        let mut told = lock(&self.told);
        purlis_core::awareness::answer(&known, session, tell, told.entry(session).or_default())
    }

    /// The report chat `session` still owes as a task, where it owes one (#1443): its lineage
    /// record, for the app to report in its place. `None` for any other chat, and for one
    /// that has reported or been reported for. A read: what it owed is marked answered by
    /// [`Self::owes`] once the report is kept, under the caller's hold of [`Self::deciding`].
    pub fn owed_task_report(&self, session: u32) -> Option<purlis_core::reopen::HandedFrom> {
        use purlis_core::reopen::{Mode, Owed};

        let open = lock(&self.open);
        let from = open.get(&session)?.chat.from.as_ref()?;
        (from.mode == Mode::Task && from.report == Owed::Due).then(|| from.clone())
    }

    /// The deciding lock, where nobody holds it now ([`Self::deciding`]).
    pub fn try_deciding(&self) -> Option<MutexGuard<'_, ()>> {
        match self.dispatching.try_lock() {
            Ok(held) => Some(held),
            Err(std::sync::TryLockError::Poisoned(held)) => Some(held.into_inner()),
            Err(std::sync::TryLockError::WouldBlock) => None,
        }
    }

    /// Remembers, or forgets, that chat `session`'s close stops every chat at work below it:
    /// the person's answer, given as its Smart close began, for the close that ends it.
    pub fn stop_below_on_close(&self, session: u32, stop: bool) {
        let mut stopping = lock(&self.stops_below);
        if stop {
            stopping.insert(session);
        } else {
            stopping.remove(&session);
        }
    }

    /// Whether chat `session`'s close stops the chats below it, taken: asked once, as it closes.
    pub fn takes_stop_below(&self, session: u32) -> bool {
        lock(&self.stops_below).remove(&session)
    }

    /// The persona chats chat `asker` has open as tasks, by number, lowest first, each with
    /// its lineage record (#1443): what that chat's list of its dispatches is made from, and
    /// what closing it asks about. **What is below `asker`**: a handoff's chat is not (#1492).
    pub fn tasks_of(&self, asker: u32) -> Vec<(u32, purlis_core::reopen::HandedFrom)> {
        let mut tasks: Vec<_> = lock(&self.open)
            .iter()
            .filter_map(|(number, one)| {
                task_of(asker, *number, &one.chat).map(|from| (*number, from.clone()))
            })
            .collect();
        tasks.sort_by_key(|(number, _)| *number);
        tasks
    }

    /// Whether the app is quitting or letting go of the project: the chats' programs are
    /// being ended by the app, and every chat is kept for the next launch.
    pub fn ending(&self) -> bool {
        self.ending.load(Ordering::SeqCst)
    }

    /// Chat `session`'s own record, as the app keeps it while it is open: what a handoff from it
    /// reads its grants from (#1362), never the request.
    pub fn recorded_chat(&self, session: u32) -> Option<Chat> {
        lock(&self.open).get(&session).map(|one| one.chat.clone())
    }

    /// **What a dispatch is decided over**: every chat this app has open and every one it is
    /// about to start, by its number for each, handed to `decide` with whether a number is a
    /// chat still starting (which counts as working: it is about to be).
    ///
    /// Read under this store's own locks, and asked under [`Self::deciding`], so the records a
    /// decision reads are the ones the slot it then reserves is added to.
    pub fn deciding_over<R>(
        &self,
        decide: impl FnOnce(&[(u32, &Chat)], &dyn Fn(u32) -> bool) -> R,
    ) -> R {
        let open = lock(&self.open);
        let reserved = lock(&self.reserved);
        let records: Vec<(u32, &Chat)> = open
            .iter()
            .map(|(number, one)| (*number, &one.chat))
            // One that has opened is in `open`, and is counted once.
            .chain(
                reserved
                    .iter()
                    .filter(|(number, _)| !open.contains_key(number))
                    .map(|(number, chat)| (*number, chat)),
            )
            .collect();
        decide(&records, &|number: u32| {
            !open.contains_key(&number) && reserved.contains_key(&number)
        })
    }

    /// Where chat `session` stands among the chats this app has open, for a dispatch to its
    /// own persona: what a test reads the counts by.
    #[cfg(test)]
    pub fn lineage(
        &self,
        session: u32,
        default: Option<&str>,
        working: &dyn Fn(u32) -> bool,
    ) -> purlis_core::dispatchdecision::Lineage {
        use purlis_core::dispatchdecision::{lineage_of, pair_of};
        self.deciding_over(|open, starting| {
            let pair = open
                .iter()
                .find(|(number, _)| *number == session)
                .map(|(_, chat)| pair_of(chat, None, default))
                .unwrap_or_default();
            lineage_of(
                session,
                open,
                default,
                &|n| starting(n) || working(n),
                &pair,
            )
        })
    }

    /// The lock a dispatch is decided under, and a report is taken under: see
    /// the `dispatching` field. Held for the few lines that read and then reserve or
    /// record, never across a start.
    pub fn deciding(&self) -> MutexGuard<'_, ()> {
        lock(&self.dispatching)
    }

    /// The persona chat `session` is starting as, while it is starting and not yet open
    /// (qw381): `Some(None)` for a chat on no persona, `None` for a chat that is not starting.
    pub fn starting_as(&self, session: u32) -> Option<Option<String>> {
        lock(&self.starting_as).get(&session).cloned()
    }

    /// Notes that chat `session` is starting as `persona`, as its announcement does: for a test
    /// that stands in for a start.
    #[cfg(test)]
    pub fn starting_as_for_test(&self, session: u32, persona: Option<&str>) {
        lock(&self.starting_as).insert(session, persona.map(str::to_owned));
    }

    /// Holds a slot for the persona chat that is about to start as `number`, recorded as
    /// `starting`: from now it counts toward its asking chat's tasks and its lineage, as the
    /// open chat it is about to be. Made under [`Self::deciding`], in the same hold as the
    /// decision that allowed it. The slot is let go when what this returns is dropped: by
    /// then the chat is open and counts for itself, or its start failed and nothing does.
    pub fn reserve(&self, number: u32, starting: Chat) -> Reserved<'_> {
        lock(&self.reserved).insert(number, starting);
        Reserved {
            chats: self,
            number,
        }
    }

    /// **Chat `session` was started again as `started`** (a restart for a grant, Restart now,
    /// Start fresh): everything that named it by its number names the new one. The tasks it
    /// dispatched are still its tasks, so they stay counted against it and listed under it,
    /// and their reports find it. The session record the app wrote for it is still its own.
    pub fn followed(&self, session: u32, started: u32) {
        if session == started {
            return;
        }
        let mut moved = false;
        // **And the lineage it is the first chat of is still one** (#1456): where the chat the
        // person started was started again under an id of its own (it had none, or the old one
        // was let go of), every chat that names its old id as the lineage's root names the new
        // one, so a chat in the middle that has closed never splits it. A chat in the middle of
        // a lineage is not its root, and its restart changes no root. Read from what it was,
        // open or just let go of, and from the chat now in its place.
        let (was_first, was_id) = {
            let open = lock(&self.open);
            match open.get(&session) {
                Some(one) => (one.chat.from.is_none(), one.chat.identity.id.clone()),
                None => {
                    drop(open);
                    lock(&self.let_go)
                        .get(&session)
                        .map_or((false, None), |was| {
                            (was.from.is_none(), was.identity.id.clone())
                        })
                }
            }
        };
        let now_id = lock(&self.open)
            .get(&started)
            .filter(|one| one.chat.from.is_none())
            .and_then(|one| one.chat.identity.id.clone());
        if was_first && now_id.is_some() && now_id != was_id {
            let mut open = lock(&self.open);
            let mut reserved = lock(&self.reserved);
            // Where it had no id, its lineage's records name no root: the chats below it are
            // found by who asked whom, and only those are given the new one.
            let mut below = std::collections::HashSet::from([session]);
            if was_id.is_none() {
                loop {
                    let more: Vec<u32> = open
                        .iter()
                        .map(|(number, one)| (*number, one.chat.from.as_ref()))
                        .chain(
                            reserved
                                .iter()
                                .map(|(number, chat)| (*number, chat.from.as_ref())),
                        )
                        .filter(|(number, from)| {
                            !below.contains(number)
                                && from.is_some_and(|from| below.contains(&from.chat))
                        })
                        .map(|(number, _)| number)
                        .collect();
                    if more.is_empty() {
                        break;
                    }
                    below.extend(more);
                }
            }
            let records = open
                .iter_mut()
                .map(|(number, one)| (*number, &mut one.chat))
                .chain(reserved.iter_mut().map(|(number, chat)| (*number, chat)));
            for (number, chat) in records {
                if number == session {
                    continue;
                }
                let Some(from) = chat.from.as_mut() else {
                    continue;
                };
                let of_it = match &was_id {
                    Some(was) => from.root.as_ref() == Some(was),
                    None => from.root.is_none() && below.contains(&number),
                };
                if of_it {
                    from.root.clone_from(&now_id);
                    moved = true;
                }
            }
        }
        for one in lock(&self.open).values_mut() {
            if let Some(from) = one.chat.from.as_mut()
                && from.chat == session
            {
                from.chat = started;
                moved = true;
            }
            // And a tab that showed it shows it still, under its new number (#1486).
            if one.chat.shows == Some(session) {
                one.chat.shows = Some(started);
                moved = true;
            }
            // And a chat with a pane in its tab is beside it still (#1489).
            if one.chat.beside == Some(session) {
                one.chat.beside = Some(started);
                moved = true;
            }
        }
        for starting in lock(&self.reserved).values_mut() {
            if let Some(from) = starting.from.as_mut()
                && from.chat == session
            {
                from.chat = started;
            }
        }
        let mut records = lock(&self.records);
        if let Some(record) = records.remove(&session) {
            records.insert(started, record);
        }
        drop(records);
        if moved {
            self.write_it_down();
        }
    }

    /// The app wrote chat `session`'s session record at `path`, project-relative.
    pub fn wrote_record(&self, session: u32, path: &str) {
        if lock(&self.open).contains_key(&session) {
            lock(&self.records).insert(session, path.to_owned());
        }
    }

    /// The session record the app last wrote for chat `session`, where it wrote one.
    pub fn last_record(&self, session: u32) -> Option<String> {
        lock(&self.records).get(&session).cloned()
    }

    /// Whose persona grants chat `session` holds instead of its own (#1362, D-1362-5/6): the
    /// chat that opened it by a handoff, by the name the person saw (none for a Resume), and the
    /// persona it was started as. `None` for a chat that holds its own, or one not open.
    pub fn grants_held(&self, session: u32) -> Option<(Option<String>, Option<String>)> {
        let open = lock(&self.open);
        let one = open.get(&session)?;
        one.chat.held.as_ref()?;
        Some((
            one.chat.from.as_ref().map(|from| from.name.clone()),
            one.chat.persona.clone(),
        ))
    }

    /// The person allowed chat `session` its own persona's grants (#1362): it no longer holds
    /// another's, from its next start, and the record says so. Answers whether it held any.
    pub fn allow_own_grants(&self, session: u32) -> bool {
        let mut open = lock(&self.open);
        let Some(one) = open.get_mut(&session) else {
            return false;
        };
        if one.chat.held.take().is_none() {
            return false;
        }
        drop(open);
        self.write_it_down();
        true
    }

    /// Chat `session`'s own harness has put it in conversation `id` — the first one a Codex
    /// or opencode chat names, or the one a Claude Code chat moved to on `/clear` (C6) — so
    /// that is the conversation the record resumes it by from now on (Q10).
    ///
    /// **The board has already judged it.** This is told only what `Board::reported` adopted
    /// or followed (`Hooks::when_it_follows`), so a nested harness's report never gets here.
    ///
    /// Written down only when the record would change, for [`Self::pin`]'s reason. Nothing for
    /// a chat that runs no harness: a shell tab is resumed by nothing, whatever ran in it.
    ///
    /// **A report that arrives before the chat is remembered here is not kept**, which no
    /// harness charter starts can produce: Claude Code's first report names the id charter
    /// chose, which moves nothing, and Codex and opencode name theirs inside the first turn,
    /// long after the start returned.
    ///
    /// `run` is the run the host began when the move was a `/clear` (ADR 0066's `clear`): the
    /// chat's current run from now on, and so the one the record names.
    pub fn follow_conversation(&self, session: u32, id: &str, run: Option<&str>) {
        let Ok(id) = SessionId::new(id) else { return };
        let mut open = lock(&self.open);
        let Some(one) = open.get_mut(&session) else {
            return;
        };
        if one.harness.is_none() {
            return;
        }
        let mut moved = false;
        if one.chat.resume.as_ref() != Some(&id) {
            one.chat.resume = Some(id);
            moved = true;
        }
        if let Some(run) = run
            && one.chat.identity.run.as_deref() != Some(run)
        {
            one.chat.identity.run = Some(run.to_owned());
            moved = true;
        }
        drop(open);
        if moved {
            self.write_it_down();
        }
    }

    /// What a chat's harness said, in `report`, about the model its program runs on (ADR 0087
    /// §6, #1021), kept for the record's `model` so a commit's `Assisted-by` can name it.
    ///
    /// **Only the chat's own harness** ([`its_own_model`]): `now` is the conversation the board
    /// holds for the chat after it took `report`, and a report the board did not take as the
    /// chat's own, a sub-agent's, or any event but a `SessionStart` is no model. Written down
    /// only when the model changed, for [`Self::pin`]'s reason.
    pub fn heard_model(&self, report: &purlis_core::hookwire::Report, now: Option<&str>) {
        let Some(model) = its_own_model(report, now) else {
            return;
        };
        let changed = lock(&self.models)
            .insert(report.chat, model.to_owned())
            .as_deref()
            != Some(model);
        if changed {
            self.write_it_down();
        }
    }

    /// **The model chat `session`'s harness said it runs on** (ADR 0087 §6, #1021), where it
    /// said one ([`Self::heard_model`]): what a grade records its chats ran on (SK-60, #3240).
    pub fn model_of(&self, session: u32) -> Option<String> {
        lock(&self.models).get(&session).cloned()
    }

    /// What the window says its view tabs are now. Written down when it differs from what was
    /// held, and not otherwise — for [`Self::pin`]'s reason: every write is a fingerprint the
    /// machine store then has to vouch for.
    pub fn hold_views(&self, views: Vec<View>) {
        let changed = std::mem::replace(&mut *lock(&self.views), views.clone()) != views;
        if changed {
            self.write_it_down();
        }
    }

    /// The view tabs the window last said it had — at a launch, the ones the record put back.
    pub fn views(&self) -> Vec<View> {
        lock(&self.views).clone()
    }

    /// What branch the window's sidebar is focused on now (FM-5), or `None` for the whole
    /// workspace. Written down when it differs from what was held, for [`Self::hold_views`]'
    /// reason.
    pub fn hold_focus(&self, focus: Option<Focus>) {
        let changed = std::mem::replace(&mut *lock(&self.focus), focus.clone()) != focus;
        if changed {
            self.write_it_down();
        }
    }

    /// The branch the window last said its sidebar was focused on — at a launch, the record's.
    pub fn focus(&self) -> Option<Focus> {
        lock(&self.focus).clone()
    }

    /// Follows a workspace rename in everything held here that names it (charter#367): a
    /// chat's directory, the workspace a handed-off chat came from, and each view tab's strip —
    /// and writes the record once if any of it moved.
    ///
    /// The core has already rewritten the record on disk; without this the next write would put
    /// the old name back, because this is what the record is written from.
    pub fn follow(&self, moved: &purlis_core::wscmd::rename::Move) {
        let mut changed = false;
        for one in lock(&self.open).values_mut() {
            changed |= moved.chat(&mut one.chat);
        }
        for one in lock(&self.would_not_start).iter_mut() {
            changed |= moved.chat(&mut one.chat);
        }
        for view in lock(&self.views).iter_mut() {
            changed |= moved.view(view);
        }
        if let Some(focus) = lock(&self.focus).as_mut() {
            changed |= moved.focus(focus);
        }
        if changed {
            self.write_it_down();
        }
    }

    /// What order the window's strip now draws the chats in, by session (SI-6).
    ///
    /// Written down when the order the record would list them in moves, and not otherwise —
    /// for [`Self::pin`]'s reason. Not when the list said differs from the one held: the window
    /// says it after every change to its tabs, and a chat it has just opened is already last.
    pub fn hold_order(&self, sessions: Vec<u32>) {
        let was = self.in_order();
        *lock(&self.order) = sessions;
        if self.in_order() != was {
            self.write_it_down();
        }
    }

    /// The running sessions in the strip's order: the ones the window placed, where it placed
    /// them, then any it has not placed yet in the order they were opened.
    ///
    /// **The one answer to "in what order"**, for both the record and a reloaded window, so the
    /// two cannot disagree about which tab comes first.
    fn in_order(&self) -> Vec<u32> {
        let running = self.sessions.running();
        let placed = lock(&self.order).clone();
        let mut ordered: Vec<u32> = placed
            .iter()
            .copied()
            .filter(|session| running.contains(session))
            .collect();
        ordered.extend(
            running
                .into_iter()
                .filter(|session| !placed.contains(session)),
        );
        ordered
    }

    /// [`Self::in_order`], with the chats a launch has still to start where the strip has
    /// them: the order a record is written in while they come back.
    fn in_order_with(&self, coming: &[Coming]) -> Vec<u32> {
        let running = self.sessions.running();
        let placed = lock(&self.order).clone();
        let is_coming = |session: &u32| coming.iter().any(|one| one.number == *session);
        let mut ordered: Vec<u32> = placed
            .iter()
            .copied()
            .filter(|session| running.contains(session) || is_coming(session))
            .collect();
        let unplaced = |session: &u32| !placed.contains(session);
        ordered.extend(running.iter().copied().filter(unplaced));
        ordered.extend(
            coming
                .iter()
                .map(|one| one.number)
                .filter(|session| unplaced(session) && !running.contains(session)),
        );
        ordered
    }

    /// Says which chat is in front, so the record knows which one to bring back in front.
    pub fn bring_to_front(&self, session: Option<u32>) {
        let changed = std::mem::replace(&mut *lock(&self.front), session) != session;
        if changed {
            self.write_it_down();
        }
    }

    /// **The profile chat `session` runs on** (#2021): the one its record names, else its
    /// harness's built-in, whose name an adopted profile keeps. None for a chat this app does
    /// not have open, or one that runs no harness.
    pub fn profile_of(&self, session: u32) -> Option<String> {
        let open = lock(&self.open);
        let running = open.get(&session)?;
        running
            .chat
            .profile
            .clone()
            .or_else(|| running.harness.map(|harness| harness.name().to_owned()))
    }

    /// **Where chat `session` works, as whom and on which harness** (SK-12): what its skill set
    /// is worked out from for Use skill…, read under one hold of the open chats. None for a chat
    /// this app does not have open.
    pub fn skills_of(&self, session: u32) -> Option<ChatForSkills> {
        let open = lock(&self.open);
        let running = open.get(&session)?;
        Some(ChatForSkills {
            workspace: running.workspace.clone(),
            persona: running.persona().started_as,
            harness: running.harness,
            start: running.skills_read.clone(),
            plane_skills: running.plane_skills,
            last_prompt: running.last_prompt.clone(),
            salt: running.prompt_key.clone(),
            cwd: running.chat.cwd.clone(),
            // As its start read its declaration: what a running chat started with (decision 15).
            not_supported: running
                .declared
                .as_ref()
                .filter(|_| running.harness.is_none())
                .and_then(purlis_core::skills::not_supported_on)
                .map(|one| one.said),
        })
    }

    /// **What chat `session`'s router points `prompt` at** (SK-15c, ADR 0102 §3): asked where its
    /// start chose a router that reads prompts and its list leaves skills out, within the
    /// router's 100 ms, of those skills alone; the routing record told as a start's is. The text
    /// its prompt hook adds to the turn, or `None`. The prompt is handed to that router and kept
    /// nowhere.
    pub fn pointed(&self, session: u32, prompt: &str) -> Option<String> {
        // Taken out of the lock before the router is asked, which may take its 100 ms. A chat a
        // runner's host composed is routed there, never by this machine's router.
        let routed = lock(&self.open)
            .get(&session)
            .filter(|running| !running.composed_by_its_host)?
            .skills_read
            .routed
            .clone()?;
        let router = lock(&self.router).clone()?;
        let Some(prompted) = routed.on_prompt(prompt, &router) else {
            self.keep_last_prompt(session, &routed, None);
            return None;
        };
        self.keep_last_prompt(session, &routed, Some(&prompted.record));
        if let Some(tell) = lock(&self.routed).as_ref() {
            tell(session, &prompted.record);
        }
        prompted.pointer
    }

    /// Keeps `record` as chat `session`'s latest prompt's routing, or none where its router was
    /// not asked, where the chat is still the start `routed` came from: the router may take its
    /// 100 ms while it is started again.
    fn keep_last_prompt(
        &self,
        session: u32,
        routed: &Arc<purlis_core::skills::router::Routed>,
        record: Option<&purlis_core::skills::router::Record>,
    ) {
        if let Some(running) = lock(&self.open).get_mut(&session)
            && running
                .skills_read
                .routed
                .as_ref()
                .is_some_and(|now| Arc::ptr_eq(now, routed))
        {
            running.last_prompt = record.cloned();
        }
    }

    /// **Every chat open here that its start routed by a router extension** (SK-15b), by number,
    /// with where it works and as whom: what the Skill studio shows of the router's timings.
    pub fn routings(&self) -> Vec<ChatRouted> {
        let open = lock(&self.open);
        let mut out: Vec<ChatRouted> = open
            .iter()
            .filter_map(|(session, running)| {
                Some(ChatRouted {
                    session: *session,
                    workspace: running.workspace.clone(),
                    persona: running.persona().started_as,
                    routed: running.skills_read.routed.clone()?,
                    last_prompt: running.last_prompt.clone(),
                })
            })
            .collect();
        out.sort_by_key(|one| one.session);
        out
    }

    /// **The personas chat `session` is named by** (#2691): the one it was started as, whose
    /// vaults it may use, and the one whose grants it runs with, as its start read them. The
    /// one place a browser login, a browser's reach and its Asks name a chat's persona. None
    /// for a chat this app does not have open.
    pub fn persona_of(&self, session: u32) -> Option<ChatPersona> {
        lock(&self.open).get(&session).map(Running::persona)
    }

    /// **Who asks for chat `session`'s browser login** (#2560), and whether it runs with no
    /// sandbox at all by the record of how its run started (#2636), read under one hold of the
    /// open chats so both
    /// come from the same run. Its persona is the one the chat was started as, as every brokered
    /// read of a vault takes it (`crate::vaults`), never a fallback. None for a chat this app
    /// does not have open.
    pub fn login_asker(
        &self,
        root: &std::path::Path,
        session: u32,
    ) -> Option<(purlis_core::secrets::brokered::Asker, bool)> {
        // Read before `open` is held: no lock here is taken under another.
        let live = self.board_of(session).map(|board| board.allowed_live());
        let open = lock(&self.open);
        let running = open.get(&session)?;
        let mut confines = running.confines.clone();
        if let (Some(confines), Some(live)) = (confines.as_mut(), live) {
            for (host, by) in live {
                confines.allow_live(&host, by);
            }
        }
        // By the record of how its run started (#2636), never by what this window holds for it
        // now: a run whose record cannot be read is taken as sandboxed.
        let unconfined = running.ran == Some(purlis_session_protocol::session::RunSandbox::Off);
        Some((
            purlis_core::secrets::brokered::Asker {
                root: root.to_path_buf(),
                env: purlis_core::secrets::Env::from_process(),
                chat: session,
                // Whose vaults it may use, as `secret exec` judges them (#2599, #2691).
                persona: running.persona().started_as,
                folder: running.chat.cwd.clone(),
                confines,
                harness: running.harness,
            },
            unconfined,
        ))
    }

    /// **Whether chat `session` is open and its run started without the sandbox, by the record
    /// of how it started** (#2242, #2692), as [`Self::login_asker`] reads it: for a path where a
    /// chat with no sandbox is allowed more. A run whose record cannot be read is not one, nor is
    /// a chat reattached under a sandbox that changed while no window was there (D-2006-11),
    /// which has no compiled sandbox recorded but is still held.
    pub fn started_unsandboxed(&self, session: u32) -> bool {
        lock(&self.open).get(&session).is_some_and(|running| {
            running.ran == Some(purlis_session_protocol::session::RunSandbox::Off)
        })
    }

    /// **Whether chat `session` is open and runs in the sandbox its start compiled, by both
    /// records** (#2692): the record of how its run started says it is in a sandbox, and this
    /// window holds what that sandbox was compiled to. For a path where a sandboxed chat is
    /// allowed more than one with no sandbox (dispatching to another persona, a handoff's
    /// crossing, a dispatch while you are away, the network record), so it fails closed there:
    /// a run whose record cannot be read, one recorded off, and one whose compile this window
    /// does not hold (a chat its host composed, or one reattached under a sandbox that changed,
    /// D-2006-11) each count as unsandboxed.
    pub fn runs_sandboxed(&self, session: u32) -> bool {
        lock(&self.open).get(&session).is_some_and(|running| {
            running.confines.is_some()
                && matches!(
                    running.ran,
                    Some(purlis_session_protocol::session::RunSandbox::By(_))
                )
        })
    }

    /// Chat `session` as though its start had compiled `confines` and its run were recorded as
    /// `ran`, for a test of a path that reads them (#2692).
    #[cfg(test)]
    pub(crate) fn runs_as_recorded(
        &self,
        session: u32,
        confines: Option<purlis_core::sandbox::Confines>,
        ran: Option<purlis_session_protocol::session::RunSandbox>,
    ) {
        if let Some(running) = lock(&self.open).get_mut(&session) {
            running.confines = confines;
            running.ran = ran;
        }
    }

    /// **Whether chat `session` is open and runs in a sandbox its host composed for it** (#2692):
    /// a runner's chat whose run the runner's record does not say is off. Its sandbox, and the
    /// tools it armed, are the runner's, so this window holds no record of them, and a restart
    /// would not give it one.
    pub fn sandboxed_by_its_host(&self, session: u32) -> bool {
        lock(&self.open).get(&session).is_some_and(|running| {
            running.composed_by_its_host
                && running.ran != Some(purlis_session_protocol::session::RunSandbox::Off)
        })
    }

    /// **Logs a skill run of a chat a pool's runner runs** (SK-20b, #3084): `run`, as the log of
    /// `pool`'s runner says it started or ended, logged in this project's skill log as a local
    /// run is, with this window's own word for where the chat ran: the persona it runs as, its
    /// harness and its workspace. Only for a chat open here that runs on that pool's runner, by
    /// its id; any other line names no chat of this window's, and nothing is logged.
    pub fn skill_run_heard(
        &self,
        pool: Option<&str>,
        run: &purlis_core::skilluse::relayed::Relayed,
    ) {
        let Some(pool) = pool else {
            return;
        };
        let ran_as = {
            let open = lock(&self.open);
            open.iter()
                .find(|(_, running)| running.chat.identity.id.as_deref() == Some(run.chat.as_str()))
                .map(|(number, running)| {
                    (
                        *number,
                        running.runs_as.clone(),
                        running.harness.map(Harness::name),
                        running.workspace.clone(),
                    )
                })
        };
        let Some((number, persona, harness, workspace)) = ran_as else {
            return;
        };
        if self.sessions.pool_of(number).as_deref() != Some(pool) {
            return;
        }
        // Under this machine's own device, as a run of its own is: the runner keeps its own file
        // under its device in its checkout, which this machine sends it.
        let device = self
            .device
            .clone()
            .unwrap_or_else(purlis_core::dispatch::host);
        purlis_core::skilluse::relayed::logged(
            &self.project,
            run,
            &purlis_core::skilluse::relayed::RanAs {
                persona: persona.as_deref(),
                harness,
                workspace: workspace.as_deref(),
            },
            &device,
        );
    }

    /// **Puts the repos of the workspace chat `session` is sent with on its runner's Claim** (#4207),
    /// and only for a pool that opted in: a chat on any other pool, or one this machine does not
    /// keep, sends nothing and reads no clone for it. **The chat's start waits for them, at most
    /// [`purlis_core::pools::REPOS_WAITED_FOR`]** (#4331), so its first mint covers them and is not
    /// kept without them. A cluster slower than that lets the chat start: a mint before they arrive
    /// goes to the person, or covers what the Claim named by then.
    #[cfg(unix)]
    fn puts_its_repos_on_its_claim(
        &self,
        workspace: Option<&str>,
        folder: Option<&std::path::Path>,
        clones: Option<&[purlis_core::forge::apptoken::Started]>,
    ) {
        let (Some(config), Some(clones)) = (&self.pools_in, clones) else {
            return;
        };
        let Ok(Some(pool)) = purlis_core::pools::runs_on(config, &self.project, workspace) else {
            return;
        };
        let (config, root, folder, clones) = (
            config.clone(),
            self.project.clone(),
            folder.map(std::path::Path::to_path_buf),
            clones.to_vec(),
        );
        let sent =
            purlis_core::pools::waited_at_most(purlis_core::pools::REPOS_WAITED_FOR, move || {
                let record = purlis_core::forge::apptoken::record_root();
                let repos = || {
                    purlis_core::forge::apptoken::repos_of_workspace(
                        &purlis_core::forge::apptoken::For {
                            root: &root,
                            // Not started yet: a workspace's repos are no chat's own.
                            chat: 0,
                            folder: folder.as_deref(),
                            here: None,
                            writes: &[],
                            started: Some(&clones),
                            on_a_runner: true,
                            config_root: record.as_deref(),
                            ends_seen: None,
                        },
                    )
                };
                if let Err(why) = purlis_core::pools::add_repos_to_claim(&config, &pool, repos) {
                    tracing::warn!(
                        "purlis: the repos of a chat on pool {pool} did not reach its claim: {why}"
                    );
                }
            });
        match sent {
            Ok(true) => {}
            Ok(false) => tracing::info!(
                "purlis: a chat started before its repos reached its claim: the cluster was slow"
            ),
            Err(why) => tracing::warn!("purlis: a chat's repos did not reach its claim: {why}"),
        }
    }

    /// **Runner chat `session`'s workspace clones as its session started here** (#2053): `None`
    /// for a chat not open, of this machine, or reattached.
    pub fn clones_at_start(
        &self,
        session: u32,
    ) -> Option<Vec<purlis_core::forge::apptoken::Started>> {
        lock(&self.open)
            .get(&session)
            .and_then(|running| running.clones_at_start.clone())
    }

    /// What chat `session`'s sandbox was compiled to as it started (#1407), while it is open:
    /// `None` for a chat that is not open or was started without a sandbox.
    pub fn confines_of(&self, session: u32) -> Option<purlis_core::sandbox::Confines> {
        lock(&self.open)
            .get(&session)
            .and_then(|running| running.confines.clone())
    }

    /// Whether chat `session` is open and its sandbox was compiled with the system's
    /// certificate check as it started (R1): a refusal of the check then has another cause,
    /// and a restart would change nothing.
    pub fn has_certificate_check(&self, session: u32) -> bool {
        self.confines_of(session)
            .is_some_and(|confines| confines.widened.trust)
    }

    /// What chat `session`'s sandbox was compiled to as it started ([`Self::confines_of`]), with
    /// the hosts the person allowed it live since (#1666): what a brokered `secret exec` run for
    /// it now is held to, so an Allow from a run's Notice reaches the chat's next run (#1667).
    pub fn confines_now(&self, session: u32) -> Option<purlis_core::sandbox::Confines> {
        let mut confines = self.confines_of(session)?;
        if let Some(board) = self.board_of(session) {
            for (host, by) in board.allowed_live() {
                confines.allow_live(&host, by);
            }
        }
        Some(confines)
    }

    /// What hears each connection a command run for chat `session` makes (#1667): a brokered
    /// `secret exec`'s, through its proxy and through a tunnel to a database host. Told as the
    /// chat's own proxy tells its connections, under the chat and the persona it started as.
    pub fn reached_for(
        &self,
        session: u32,
        persona: Option<String>,
    ) -> purlis_core::sandbox::egress::Reached {
        let id = lock(&self.open)
            .get(&session)
            .and_then(|running| running.chat.identity.id.clone());
        reached_by_the_proxy(
            Arc::clone(&self.reached),
            Arc::new(std::sync::atomic::AtomicU32::new(session)),
            ReachedAs { id, persona },
        )
    }

    /// Marks `folder` as going while a discard takes it away (#1472), until what this answers
    /// is dropped: no chat starts in it or below it until then. Refused while a chat is starting
    /// there; a chat that is already open there is the caller's to ask about, after this.
    pub(crate) fn taking_away(
        &self,
        folder: &std::path::Path,
    ) -> Result<crate::goingaway::GoingAway<'_>, String> {
        self.going.taking_away(folder)
    }

    /// What is open, in the strip's order.
    pub fn open_now(&self) -> Vec<Open> {
        // In the strip's order (`in_order`), so the window comes back with its tabs the way
        // they were left. Asked before `open` is held: it takes `order` itself.
        let ordered = self.in_order();
        // Read out before `open` is held, as `order` is: no lock here is taken under another.
        let floorless = lock(&self.floorless).clone();
        let untaken = lock(&self.untaken).clone();
        let open = lock(&self.open);
        let front = *lock(&self.front);
        ordered
            .into_iter()
            .filter_map(|session| {
                let running = open.get(&session)?;
                let chat = &running.chat;
                Some(Open {
                    session,
                    name: chat.name.clone(),
                    cwd: chat.cwd.clone(),
                    workspace: running.workspace.clone(),
                    // The harness this chat was STARTED as, not one inferred from its
                    // program's name — a profile's command is commonly a wrapper, and the
                    // sidebar used to answer "no harness" for one while the board knew the
                    // kind. One idea of what is running, or the two drift.
                    harness: running.harness,
                    declared: running.declared.clone(),
                    profile: chat.profile.clone(),
                    persona: chat.persona.clone(),
                    in_front: front == Some(session),
                    how: running.how.clone(),
                    pinned: chat.pinned,
                    label: chat.label.clone(),
                    from: chat.from.clone(),
                    tab: chat.has_tab(),
                    shows: chat.shows,
                    beside: chat.beside,
                    sandbox: running.confines.as_ref().map(|confines| confines.level),
                    floorless: floorless.get(&session).cloned(),
                    untaken: untaken.get(&session).cloned(),
                })
            })
            .collect()
    }

    /// The number the next chat started here would take (#3376): one past the highest this
    /// plane has dealt. Only a guess for what to show — the Name field's placeholder,
    /// `steward 10` — since a chat started meanwhile takes it first; a start is dealt its
    /// number by the sessions, never by this.
    ///
    /// **Under the session host it can be low** (#3510): the host's counter is the one that
    /// deals, and this window's mirror of it is restored from the record. A window that ended
    /// without writing its record, after the host had dealt numbers to chats now gone, leaves
    /// the mirror behind the host, so the placeholder names a number the start will pass. It is
    /// never high, and the name the chat takes is the host's. Asking the host would take a
    /// protocol command that peeks without dealing, for a placeholder.
    pub fn next_number(&self) -> u32 {
        self.sessions.dealt().saturating_add(1)
    }

    /// What was open, to write down.
    pub fn record(&self) -> Record {
        // The chats a launch has still to start are written as they were recorded, each in
        // its place: a record written before one starts still holds it.
        let coming = lock(&self.coming).clone();
        let ordered = self.in_order_with(&coming);
        // Asked before `open` is taken: a program being ended answers only once it is gone,
        // and nothing else may wait on `open` that long (R2-2).
        let pids: HashMap<u32, Option<u32>> = ordered
            .iter()
            .map(|&session| (session, self.sessions.process_id(session)))
            .collect();
        let models = lock(&self.models).clone();
        let open = lock(&self.open);
        let front = *lock(&self.front);
        // **One chat, once** (NO-3): a chat being retried is waiting and open at once until its
        // start returns, and one being started fresh is open twice, under its old session and
        // its new. The record holds each id once — as the newest session open under it.
        let mut newest: HashMap<&str, u32> = HashMap::new();
        for (&session, running) in open.iter() {
            if let Some(id) = running.chat.identity.id.as_deref() {
                let at = newest.entry(id).or_insert(session);
                *at = (*at).max(session);
            }
        }
        let superseded = |chat: &Chat, session: Option<u32>| {
            chat.identity
                .id
                .as_deref()
                .and_then(|id| newest.get(id))
                .is_some_and(|&at| session != Some(at))
        };
        // The ones that could not be started come first, in the order they were recorded,
        // so they keep their place and are tried again at the next launch.
        let mut chats: Vec<Chat> = lock(&self.would_not_start)
            .iter()
            .map(|one| &one.chat)
            .filter(|chat| !superseded(chat, None))
            .map(|chat| Chat {
                pid: None,
                model: None,
                // Not started this launch: the flag and the routing its record had are its last
                // start's, which a program the host still runs was started with (#3466, #3578),
                // and a start that is not a reattach reads neither.
                ..chat.clone()
            })
            .collect();
        // The ids written so far, as chats that could not be started. A chat of a launch is
        // listed there before it leaves the ones still to start (`start_what_is_coming`), and
        // those were read first here, so one found not to start meanwhile is in both.
        let not_started: std::collections::HashSet<String> = chats
            .iter()
            .filter_map(|chat| chat.identity.id.clone())
            .collect();
        // Then the running ones, in the strip's order — which is the order the next launch
        // puts them back in.
        chats.extend(ordered.into_iter().filter_map(|session| {
            let Some(running) = open.get(&session) else {
                let one = coming.iter().find(|one| one.number == session)?;
                // Open already, under this id: its start has just returned.
                if superseded(&one.chat, None) {
                    return None;
                }
                // Written above already, as one that did not start: **one chat, once**. Twice,
                // the next launch would read the second as another chat and start both.
                if one
                    .chat
                    .identity
                    .id
                    .as_ref()
                    .is_some_and(|id| not_started.contains(id))
                {
                    return None;
                }
                return Some(Chat {
                    active: front == Some(session),
                    pid: None,
                    model: None,
                    // Its start is under way, and the flag it takes is not known yet (#3466):
                    // a later reattach reads it again rather than take an earlier run's. So is
                    // how it routes its skills (#3578).
                    plane_skills: None,
                    routing: None,
                    ..one.chat.clone()
                });
            };
            let chat = &running.chat;
            if superseded(chat, Some(session)) {
                return None;
            }
            Some(Chat {
                active: front == Some(session),
                // The number it is actually running under, which is the key its workspace
                // pointer and session lock are written at. Taken from the session and not
                // from the chat, so the two can never come to say different things.
                number: Some(session),
                // The process it runs as now, which is what a commit is checked against (V82,
                // #1018) — never one a record put back carried from an earlier launch.
                pid: pids.get(&session).copied().flatten(),
                // What its harness said this program runs on (#1021), and nothing a record put
                // back carried from an earlier run.
                model: models.get(&session).cloned(),
                ..chat.clone()
            })
        }));
        Record {
            chats,
            views: lock(&self.views).clone(),
            focus: lock(&self.focus).clone(),
            // What the next launch must not deal again — charter-app#90. It is the high
            // water mark and not the count of what is open, so the numbers of chats that
            // were closed are spent too, and no new chat lands on a pointer one of them
            // left behind.
            dealt: self.sessions.dealt(),
            // An ordinary write, which is what makes the flag last one launch: the quit that
            // restarts charter for an update is the only writer that says otherwise (#251).
            relaunch_after_update: false,
            // The writer stamps the clone and device it writes from (V43, `Records::write`).
            clone_seat: None,
        }
    }

    /// [`Self::put_back_telling`], telling no chat anything: what the tests put a record back
    /// with. A launch puts one back through `restored::begin` and `restored::finish`.
    #[cfg(test)]
    pub fn put_back(&self, record: &Record, size: Size) -> Vec<Open> {
        self.put_back_telling(record, size, &|_| None)
    }

    /// Puts a record back: one session per chat it holds, resumed where it can be, with `told`
    /// as a chat's first message where it answers one: a sentence of purlis's, last on its line,
    /// as a restart tells one. A task still owing its report is told to carry on (#1513).
    ///
    /// A chat whose program cannot be started is left out and the rest still open — a
    /// relaunch that failed whole because one harness had been uninstalled would be worse
    /// than one that came back short.
    ///
    /// Whole, on the thread that asks: [`Self::begin_put_back`], then
    /// [`Self::finish_put_back`]. A launch does the two apart, the second behind the window.
    #[cfg(test)]
    pub fn put_back_telling(
        &self,
        record: &Record,
        size: Size,
        told: &dyn Fn(&Chat) -> Option<&'static str>,
    ) -> Vec<Open> {
        self.begin_put_back(record, told);
        self.finish_put_back(size)
    }

    /// The first half of putting a record back: **registers** every chat it holds and starts
    /// none (`crate::restoring`). From here each has the number it will start under, the
    /// window can draw its tab ([`Self::restoring_now`]), and every record written holds it
    /// as it was recorded, so a quit before its start loses nothing.
    /// [`Self::finish_put_back`] starts them.
    pub(crate) fn begin_put_back(
        &self,
        record: &Record,
        told: &dyn Fn(&Chat) -> Option<&'static str>,
    ) {
        // Before a single chat starts, so that a number the record spent on a chat it no
        // longer holds — one the operator closed before quitting — is not dealt again to a
        // chat that would then read its workspace pointer and take its lock
        // (charter-app#90). Past every number the record's own chats hold too: each keeps
        // its own, and a chat opened while they are still starting must not be dealt one.
        let kept = record.chats.iter().filter_map(|chat| chat.number).max();
        self.sessions
            .already_dealt(record.dealt.max(kept.unwrap_or(0)));
        // The view tabs start nothing, so they are simply held until the window asks for them
        // (`reopened_views`) — and written back out with everything else at the next change.
        *lock(&self.views) = record.views.clone();
        *lock(&self.focus) = record.focus.clone();
        // Every chat here starts a program. A record with thousands in it — a runaway, or a
        // file nobody meant — would start them all. The cap is far above the fifty the
        // product is for, so it never meets an operator; it is only ever a backstop. What it
        // leaves out stays recorded, like anything else that did not start.
        //
        // A chat the record holds no id for is given one here, before it is tried, so the
        // one that does not start is kept under it too and is not given another at every
        // launch it fails at (#856 review F2).
        //
        // **And so is a chat whose id another chat in the record already has** — a record
        // hand-edited, corrupt or left by an older bug. Two chats are two chats: the record writes
        // one per id, the newest open (`record`), which is right only for a retry or a fresh
        // start under way, and two running under one id would lose one at the next write.
        let mut seen = std::collections::HashSet::new();
        let chats: Vec<(Chat, Why)> = record
            .chats
            .iter()
            // A grade's chat is never started again: its grade ended with the run it was in
            // (SK-16). Its throwaway piece goes once no chat stands in it.
            .filter(|chat| !chat.graded)
            .map(|chat| match &chat.identity.id {
                Some(id) if seen.insert(id.clone()) => (chat.clone(), Why::Relaunch),
                _ => (
                    Chat {
                        identity: purlis_core::reopen::Identity {
                            id: Some(minted()),
                            device: self.device.clone(),
                            ..chat.identity.clone()
                        },
                        ..chat.clone()
                    },
                    Why::Upgrade,
                ),
            })
            .collect();
        let most = self.most_at_once;
        let (starting, too_many) = chats.split_at(chats.len().min(most));
        for (chat, _) in too_many {
            lock(&self.would_not_start).push(Waiting {
                chat: chat.clone(),
                why: format!("more than {most} chats were recorded"),
                // Never tried, so nothing was refused: Retry now reads it.
                approval: None,
                told: told(chat),
            });
        }
        // Each under the number it starts as: the one it was recorded with, which is the key
        // its workspace pointer and session lock are under, or one dealt now for a chat
        // recorded with none, or with one another chat of the record or a running chat has.
        let mut taken: std::collections::HashSet<u32> =
            self.sessions.running().into_iter().collect();
        let coming: Vec<Coming> = starting
            .iter()
            .map(|(chat, why)| {
                let number = match chat.number {
                    Some(number) if taken.insert(number) => number,
                    _ => {
                        let dealt = self.sessions.deal();
                        taken.insert(dealt);
                        dealt
                    }
                };
                Coming {
                    told: told(chat),
                    chat: Chat {
                        number: Some(number),
                        ..chat.clone()
                    },
                    number,
                    why: *why,
                    starting: false,
                    workspace: workspace_at_start(Some(&self.project), chat.cwd.as_deref()),
                }
            })
            .collect();
        // The strip comes back in the record's order, which is the order it was drawn in when
        // it was written — not the order of the numbers the chats kept (charter-app#90).
        let mut order = lock(&self.order);
        order.retain(|session| !coming.iter().any(|one| one.number == *session));
        order.extend(coming.iter().map(|one| one.number));
        drop(order);
        // And the chat that was in front is the one in front, from before it starts: what it
        // goes first for, and what a record written meanwhile says.
        if let Some(front) = coming.iter().find(|one| one.chat.active) {
            *lock(&self.front) = Some(front.number);
        }
        lock(&self.recorded_order)
            .extend(coming.iter().filter_map(|one| one.chat.identity.id.clone()));
        lock(&self.coming).extend(coming);
    }

    /// Every chat of this project the session host still runs from before this window that no
    /// chat put back reattached is closed (#2006): once a put back has settled.
    pub(crate) fn close_what_was_not_reattached(&self) {
        self.sessions.close_what_was_not_reattached();
    }

    /// **The browsers the session host still runs, or ran, in this project from before this
    /// window** (#2025), each once: what the project's put back takes back with their chats, or
    /// ends.
    pub(crate) fn browsers_left(&self) -> Vec<purlis_session_protocol::session::HeldProgram> {
        self.sessions.browsers_left()
    }

    /// The second half of putting a record back: **starts** what
    /// [`Self::begin_put_back`] registered, a few at a time, the chat in front first, and
    /// answers once every one has been tried, with the ones that started.
    ///
    /// A chat whose program cannot be started is kept as one that would not start and the
    /// rest still open — a relaunch that failed whole because one harness had been
    /// uninstalled would be worse than one that came back short.
    ///
    /// Each chat is told to the window as it starts or is found not to
    /// (`crate::restoring::tell`). **Call it off the window's thread**: it starts programs.
    pub(crate) fn finish_put_back(&self, size: Size) -> Vec<Open> {
        let wanted = lock(&self.coming)
            .iter()
            .filter(|one| !one.starting)
            .count();
        let opened: Mutex<Vec<u32>> = Mutex::new(Vec::new());
        // Twice the starts the gate lets through at once: a start that hangs holds its
        // thread, and the ones behind it need one to go on with.
        let hands = wanted.min(crate::restoring::at_once() * 2);
        if hands <= 1 {
            // One chat, or none: on this thread, in turn.
            self.start_what_is_coming(size, &opened);
        } else {
            std::thread::scope(|scope| {
                for _ in 0..hands {
                    scope.spawn(|| self.start_what_is_coming(size, &opened));
                }
            });
        }
        let opened = opened.into_inner().unwrap_or_else(PoisonError::into_inner);
        // Written ONCE, when every chat has been tried, and not once per chat (fifty writes of
        // one file as a launch settles). What came back holds what the file did not: a run
        // each, begun at this launch, and an id for a chat recorded before ids (ADR 0066). An
        // id only in memory is minted again after a crash (#856 review F1). The chats that did
        // not start are written too, first, as every write writes them.
        if wanted > 0 {
            self.write_it_down();
        }
        let open = self.open_now();
        open.into_iter()
            .filter(|one| opened.contains(&one.session))
            .collect()
    }

    /// One hand of [`Self::finish_put_back`]: takes a place at the gate, then the chat that
    /// matters most among the ones still to start, until none is left.
    fn start_what_is_coming(&self, size: Size, opened: &Mutex<Vec<u32>>) {
        PUTTING_BACK_HERE.set(true);
        loop {
            let place = self.gate.enter(&|| crate::restoring::Rank {
                behind: crate::restoring::is_behind(&self.project),
                hidden: !self.front_is_coming(),
            });
            // A quit that began while this waited starts nothing more: the chats still to
            // start are in the record the quit writes.
            if self.ending.load(Ordering::SeqCst) {
                break;
            }
            let Some(next) = self.take_coming() else {
                break;
            };
            let first = self.told_put_back(&next.chat, next.told);
            let started =
                self.start_recorded_told(&next.chat, size, next.why, first.as_deref(), None);
            let session = match started {
                Ok(session) => {
                    lock(opened).push(session);
                    // It started under another number than its tab was drawn under (the
                    // host refused that one): the strip and the front follow it.
                    if session != next.number {
                        for placed in lock(&self.order).iter_mut() {
                            if *placed == next.number {
                                *placed = session;
                            }
                        }
                        let mut front = lock(&self.front);
                        if *front == Some(next.number) {
                            *front = Some(session);
                        }
                    }
                    Some(session)
                }
                // Kept, not dropped: the next record has to hold it too, or a directory
                // that has moved deletes the chat for good.
                Err(why) => {
                    // Read before the lock is taken, as Retry does: it reads the profile's
                    // file and runs git, and the list must not wait on either.
                    let approval = NeedsApproval::of(&next.chat, &self.project);
                    self.did_not_come(Waiting {
                        approval,
                        chat: next.chat.clone(),
                        why,
                        told: next.told,
                    });
                    let mut front = lock(&self.front);
                    if *front == Some(next.number) {
                        *front = None;
                    }
                    None
                }
            };
            // Out of the ones still to start only now, when it is open or among the ones that
            // would not start: a record written at any moment holds it once.
            //
            // **Unless the quit has begun and it started** (#3114): it is ended below, so it would
            // be in neither list, and the quit's record, built at any moment from here, would
            // lose it for good. It stays one still to start, as the record put back had it.
            // Asked once, under the lock the record reads the ones still to start under: seen not
            // begun, the chat stays open until the quit's record is written, and is ended with
            // the rest.
            let (left, quitting) = {
                let mut coming = lock(&self.coming);
                let quitting = self.ending.load(Ordering::SeqCst);
                if !quitting || session.is_none() {
                    coming.retain(|one| one.number != next.number);
                }
                (coming.len(), quitting)
            };
            // A quit that began while it started ends it with the rest.
            if let Some(session) = session
                && quitting
            {
                // Back under the number it is still to start as, in its place on the strip and
                // in front where it was, before it is ended: a record built meanwhile finds it
                // there, and not at the strip's end or with no chat in front.
                if session != next.number {
                    for placed in lock(&self.order).iter_mut() {
                        if *placed == session {
                            *placed = next.number;
                        }
                    }
                    let mut front = lock(&self.front);
                    if *front == Some(session) {
                        *front = Some(next.number);
                    }
                }
                // As any chat is closed, so nothing of it is left held; nothing is written,
                // since the quit's record is the last.
                let _ = self.close(session);
            }
            crate::restoring::tell(crate::restoring::Moved {
                root: self.project.clone(),
                number: next.number,
                session,
                left,
            });
            // The place is kept until the window has been told: the tells of one project come
            // in the order its chats settled, and how many are left only goes down.
            drop(place);
        }
        PUTTING_BACK_HERE.set(false);
    }

    /// Whether the chat in front is among the ones a launch has still to start.
    fn front_is_coming(&self) -> bool {
        let front = *lock(&self.front);
        lock(&self.coming)
            .iter()
            .any(|one| !one.starting && Some(one.number) == front)
    }

    /// Takes the next chat to start: the one in front, where it is still to start, or else
    /// the first in the record's order. It stays listed, marked as starting, until it has.
    fn take_coming(&self) -> Option<Coming> {
        let front = *lock(&self.front);
        let mut coming = lock(&self.coming);
        let at = coming
            .iter()
            .position(|one| !one.starting && Some(one.number) == front)
            .or_else(|| coming.iter().position(|one| !one.starting))?;
        coming[at].starting = true;
        Some(coming[at].clone())
    }

    /// Lists a chat of the launch among the ones that would not start, **in the record's
    /// order**: ahead of any that came after it in the record and was found out first.
    fn did_not_come(&self, waiting: Waiting) {
        let recorded = lock(&self.recorded_order).clone();
        let place = |chat: &Chat| {
            let id = chat.identity.id.as_ref()?;
            recorded.iter().position(|one| one == id)
        };
        let mine = place(&waiting.chat);
        let mut would_not = lock(&self.would_not_start);
        let at = would_not
            .iter()
            .position(|one| mine.is_some() && place(&one.chat) > mine)
            .unwrap_or(would_not.len());
        would_not.insert(at, waiting);
    }

    /// The chats a launch has registered and not started yet, as the window draws their
    /// tabs: in the record's order, each under the number it will start as.
    ///
    /// What only a start settles is not said: its harness and how it came back are the
    /// started chat's, told when it starts.
    pub fn restoring_now(&self) -> Vec<Open> {
        let front = *lock(&self.front);
        lock(&self.coming)
            .iter()
            .map(|one| Open {
                session: one.number,
                name: one.chat.name.clone(),
                cwd: one.chat.cwd.clone(),
                workspace: one.workspace.clone(),
                harness: None,
                // The declaration it starts on is its start's to say, as its harness is.
                declared: None,
                profile: one.chat.profile.clone(),
                persona: one.chat.persona.clone(),
                in_front: front == Some(one.number),
                how: Reopened::Fresh(purlis_core::reopen::Fresh::NoConversationRecorded),
                pinned: one.chat.pinned,
                label: one.chat.label.clone(),
                from: one.chat.from.clone(),
                tab: one.chat.has_tab(),
                shows: one.chat.shows,
                beside: one.chat.beside,
                // How it is sandboxed is its start's to say.
                sandbox: None,
                floorless: None,
                untaken: None,
            })
            .collect()
    }

    /// Every chat the window draws a tab for: the open ones and the ones a launch has still
    /// to start, in the strip's order, each with whether it is still to start.
    ///
    /// The ones still to start are read first: a chat leaves them only once it is open or
    /// among the ones that would not start, so read this way round none is missed.
    pub fn drawn_now(&self) -> Vec<(Open, bool)> {
        let restoring = self.restoring_now();
        let coming = lock(&self.coming).clone();
        let order = self.in_order_with(&coming);
        let open = self.open_now();
        let mut drawn: Vec<(Open, bool)> = Vec::new();
        for session in order {
            if let Some(one) = open.iter().find(|one| one.session == session) {
                drawn.push((one.clone(), false));
            } else if let Some(one) = restoring.iter().find(|one| one.session == session) {
                drawn.push((one.clone(), true));
            }
        }
        // An open chat the order has not caught up with is still an open chat.
        for one in open {
            if !drawn.iter().any(|(seen, _)| seen.session == one.session) {
                drawn.push((one, false));
            }
        }
        drawn
    }

    /// Writes the record again because a chat's program ended on its own, so it no longer
    /// names that program's pid (V82, #1018). The chat stays, as its tab does.
    pub fn a_program_ended(&self) {
        self.write_it_down();
    }

    /// Hands `write` the last record, the one written at quit or when the project is closed:
    /// the chats are kept for the next launch, with no pid, because their programs are ended
    /// right after this (V82, #1018).
    ///
    /// **Nothing is written after it.** Writing stops BEFORE this record is built, and it is
    /// built and written under the lock every write takes, so a write already on its way (a
    /// program that ended on its own, an operator's click) lands before it or not at all, and
    /// never puts live pids back on disk after it (R2-1).
    pub fn write_last(&self, write: impl FnOnce(&Record)) {
        self.ending.store(true, Ordering::SeqCst);
        let _writing = lock(&self.writing);
        let mut record = self.record();
        for chat in &mut record.chats {
            chat.pid = None;
            chat.model = None;
        }
        write(&record);
    }

    /// Hands the record as it now is to whoever writes it.
    fn write_it_down(&self) {
        // Nothing is written once the quit has begun: its own write is the last, and a
        // program's end heard after it would write a record with no chats in it.
        // Nor by a thread putting a launch's chats back, which writes once when every one has
        // been tried. Any other change meanwhile is written, and holds the ones still to start.
        if PUTTING_BACK_HERE.get() || self.ending.load(Ordering::SeqCst) {
            return;
        }
        // The record is built and handed over under one lock, so that two changes landing
        // together cannot write themselves out of order and leave the older one on disk.
        let _writing = lock(&self.writing);
        // Asked again under the lock: the last record may have been written while this one
        // waited for it ([`Self::write_last`]).
        if self.ending.load(Ordering::SeqCst) {
            return;
        }
        (self.record_it)(&self.record());
    }

    /// The chats a launch could not start, by id, name and reason. The window says so.
    ///
    /// **By id**, because a name says less than it seems to: a split's chat takes its tab's
    /// name and tab numbers start again at every launch, so two waiting chats can share one,
    /// and a Forget meant for the second must never drop the first (NO-3 review). Every waiting
    /// chat has an id: [`Self::put_back_telling`] mints one before it tries a chat that had none.
    pub fn would_not_start(&self) -> Vec<NotStarted> {
        lock(&self.would_not_start)
            .iter()
            .map(|one| NotStarted {
                id: one.chat.identity.id.clone().unwrap_or_default(),
                name: one.chat.name.clone(),
                why: one.why.clone(),
                approval: one.approval.clone(),
            })
            .collect()
    }

    /// The chats a launch could not start as the record holds them, each with why and the
    /// approval its profile needs: what says whose task one was (#1497, `crate::unstarted`).
    /// The window's list is [`Self::would_not_start`].
    /// **The chats that have not come back since the launch** (#3900, #3901), by their ids
    /// (ADR 0066) and the numbers their tabs are drawn under: still restoring, or waiting to
    /// start again. No program of theirs runs here yet, and each may yet: whether one still
    /// works is not something this app can say of it.
    pub(crate) fn not_back(&self) -> Vec<(String, Option<u32>)> {
        let coming: Vec<(String, Option<u32>)> = lock(&self.coming)
            .iter()
            .filter_map(|one| Some((one.chat.identity.id.clone()?, Some(one.number))))
            .collect();
        // One waiting to start has the number its record kept, where it kept one.
        let waiting = lock(&self.would_not_start)
            .iter()
            .filter_map(|one| Some((one.chat.identity.id.clone()?, one.chat.number)))
            .collect::<Vec<_>>();
        coming.into_iter().chain(waiting).collect()
    }

    pub(crate) fn waiting_to_start(&self) -> Vec<WaitingChat> {
        lock(&self.would_not_start)
            .iter()
            .map(|one| WaitingChat {
                chat: one.chat.clone(),
                why: one.why.clone(),
                approval: one.approval.clone(),
                told: one.told,
            })
            .collect()
    }

    /// Takes the chat `id` out of the chats waiting to start, and out of the record, and
    /// answers it: what [`Self::forget`] drops, handed back so that whoever took it can put
    /// it back ([`Self::keep_waiting`]) if what it took it for did not happen.
    pub(crate) fn take_waiting(&self, id: &str) -> Option<WaitingChat> {
        let taken = {
            let mut waiting = lock(&self.would_not_start);
            let at = waiting
                .iter()
                .position(|one| one.chat.identity.id.as_deref() == Some(id))?;
            waiting.remove(at)
        };
        self.write_it_down();
        Some(WaitingChat {
            chat: taken.chat,
            why: taken.why,
            approval: taken.approval,
            told: taken.told,
        })
    }

    /// Holds `waiting` as a chat a launch could not start, as [`Self::put_back`] holds one:
    /// recorded, and tried again at the next launch.
    pub(crate) fn keep_waiting(&self, waiting: WaitingChat) {
        lock(&self.would_not_start).push(Waiting {
            chat: waiting.chat,
            why: waiting.why,
            approval: waiting.approval,
            told: waiting.told,
        });
        self.write_it_down();
    }

    /// **Retry now** on a chat a launch could not start (NO-3): starts it again the way the
    /// launch did ([`Self::start_recorded_told`], as a relaunch), and answers its session.
    ///
    /// **Never twice, never lost.** The chat stays in the waiting list while it starts, so a
    /// record written meanwhile still has it; [`Self::record`] leaves out a waiting chat whose id
    /// is open, so one written after the start has it once, as running. It leaves the list only
    /// once it has started, and if it fails again it stays with the new reason.
    pub fn retry(&self, id: &str, size: Size) -> Result<u32, String> {
        let (chat, told) = lock(&self.would_not_start)
            .iter()
            .find(|one| one.chat.identity.id.as_deref() == Some(id))
            .map(|one| (one.chat.clone(), one.told))
            .ok_or_else(|| format!("chat {id} is not waiting to start"))?;
        // Told what the launch would have told it: a task is told to carry on (#1513).
        let told = self.told_put_back(&chat, told);
        let started = self.start_recorded_told(&chat, size, Why::Relaunch, told.as_deref(), None);
        // Read again at every refusal, outside the lock: what the profile needs now, and not
        // what it needed at the launch (#1246).
        let approval = started
            .as_ref()
            .err()
            .and_then(|_| NeedsApproval::of(&chat, &self.project));
        {
            let mut waiting = lock(&self.would_not_start);
            match &started {
                Ok(_) => waiting.retain(|one| one.chat.identity.id.as_deref() != Some(id)),
                Err(why) => {
                    for one in waiting.iter_mut() {
                        if one.chat.identity.id.as_deref() == Some(id) {
                            one.why = why.clone();
                            one.approval = approval.clone();
                        }
                    }
                }
            }
        }
        self.write_it_down();
        started
    }

    /// The chat with id `id` that a launch could not start, as its record holds it (#1513):
    /// what a Forget of a task ends by.
    pub fn waiting_chat(&self, id: &str) -> Option<Chat> {
        lock(&self.would_not_start)
            .iter()
            .find(|one| one.chat.identity.id.as_deref() == Some(id))
            .map(|one| one.chat.clone())
    }

    /// The chat with id `id` that a launch could not start is told nothing as it starts again
    /// (#1546): its dispatch is no longer at work (`crate::restored::retrying`).
    pub(crate) fn tell_nothing(&self, id: &str) {
        for one in lock(&self.would_not_start).iter_mut() {
            if one.chat.identity.id.as_deref() == Some(id) {
                one.told = None;
            }
        }
    }

    /// Whether the chat numbered `number` is one a launch could not start, and still waits.
    pub fn waits_to_start(&self, number: u32) -> bool {
        lock(&self.would_not_start)
            .iter()
            .any(|one| one.chat.number == Some(number))
    }

    /// **Forget this chat** (NO-3): drops a chat a launch could not start from the record, by
    /// its id.
    ///
    /// The one way such a chat leaves it. It is kept on purpose otherwise, so that a directory
    /// that moved, or a harness mid-reinstall, does not delete it ([`Self::put_back_telling`]).
    pub fn forget(&self, id: &str) -> Result<(), String> {
        {
            let mut waiting = lock(&self.would_not_start);
            let at = waiting
                .iter()
                .position(|one| one.chat.identity.id.as_deref() == Some(id))
                .ok_or_else(|| format!("chat {id} is not waiting to start"))?;
            waiting.remove(at);
        }
        self.write_it_down();
        Ok(())
    }

    /// **Start fresh** (NO-3, the plane-updated mark): chat `session` started again, on the
    /// plane as it is now, as **the same chat** under its id in a run that begins `fresh`, with
    /// no conversation resumed (ADR 0066).
    ///
    /// Its profile is looked up again, as at a relaunch ([`Self::start_recorded_told`]), so an edit
    /// to it is what the new run starts on. **The old one stays open until the new one has
    /// started**, so a refused start leaves it running and recorded as it was; while both are
    /// open, [`Self::record`] writes the newer. Ending the old one is the caller's next step
    /// (`Held::start_chat_fresh`), which takes it off the board too.
    ///
    /// **Unless `refused` answers why not**, from what the chat would be told as it starts again
    /// ([`crate::rebrief::again`]; `None` for a chat no dispatch started): then nothing starts.
    /// What it is told is read once, so what was judged is what it is handed (#1609).
    pub(crate) fn start_fresh_unless(
        &self,
        session: u32,
        size: Size,
        refused: impl FnOnce(Option<&crate::rebrief::Again>) -> Option<String>,
    ) -> Result<u32, String> {
        let was = lock(&self.open)
            .get(&session)
            .map(|one| one.chat.clone())
            .ok_or_else(|| format!("chat {session} is not open"))?;
        let again = Chat {
            identity: purlis_core::reopen::Identity {
                run: None,
                ..was.identity.clone()
            },
            resume: None,
            pid: None,
            number: None,
            ..was
        };
        // **A dispatched chat is handed its brief again** (#1609), as its first message: the
        // brief was the whole of what it was asked, and this run has no conversation to hold it.
        let told = self.again_told(&again);
        if let Some(why) = refused(told.as_ref()) {
            return Err(why);
        }
        let told = told.map(|told| told.message);
        let started = self.start_recorded_told(&again, size, Why::Again, told.as_deref(), None)?;
        // **It keeps its place** (#1246): the window puts the new session in the old one's pane,
        // so the record puts it where the old one was in the strip's order. Unplaced, it would
        // go last, and the next launch would draw it at the end of the strip.
        for placed in lock(&self.order).iter_mut() {
            if *placed == session {
                *placed = started;
            }
        }
        self.write_it_down();
        Ok(started)
    }

    /// [`Self::start_fresh_unless`], refused nothing: for a test of the start itself.
    #[cfg(test)]
    pub fn start_fresh(&self, session: u32, size: Size) -> Result<u32, String> {
        self.start_fresh_unless(session, size, |_| None)
    }

    /// Records chat `session` as one this app started inside a sandbox: for a test of what a
    /// confined chat is answered, where no stand-in goes through a real sandboxed start.
    #[cfg(test)]
    pub fn recorded_as_confined(&self, session: u32) {
        if let Some(running) = lock(&self.open).get_mut(&session) {
            running.confines = Some(purlis_core::sandbox::Confines::default());
        }
    }

    /// How many chats are remembered, which is not the same as how many are running: this
    /// is what a chat that closed has to stop costing. Only the tests ask.
    #[cfg(test)]
    pub fn remembered(&self) -> usize {
        lock(&self.open).len()
    }

    /// Ends every chat, and does not return until their programs are gone.
    pub fn end_all(&self) {
        self.ending.store(true, Ordering::SeqCst);
        self.sessions.end_all();
        // Waited on a little, so a quit right after does not cut the revokes off (#3808).
        purlis_core::forge::apptoken::project_ended(
            &self.project,
            std::time::Duration::from_secs(2),
        );
        lock(&self.open).clear();
        lock(&self.would_not_start).clear();
        lock(&self.coming).clear();
        lock(&self.views).clear();
        *lock(&self.focus) = None;
        *lock(&self.front) = None;
    }
}

#[cfg(test)]
impl Default for Chats {
    fn default() -> Self {
        Self::new()
    }
}

/// What a start in a project being let go of is refused with (#1949).
pub(crate) const LETTING_GO: &str =
    "purlis is letting go of this project, so no chat starts in it now.";

/// A slot held for a persona chat while it starts ([`Chats::reserve`]). Dropping it lets the
/// slot go.
pub struct Reserved<'a> {
    chats: &'a Chats,
    number: u32,
}

impl Drop for Reserved<'_> {
    fn drop(&mut self) {
        lock(&self.chats.reserved).remove(&self.number);
    }
}

/// The model `report` names for its chat, where it is that chat's own harness speaking: a
/// `SessionStart` that names a model, from no sub-agent, about the conversation the board holds
/// for the chat now (`now`, read after the board took the report). A nested harness's report
/// names another conversation, or one the board refused (ADR 0024), and is no model.
fn its_own_model<'a>(
    report: &'a purlis_core::hookwire::Report,
    now: Option<&str>,
) -> Option<&'a str> {
    use purlis_core::hookwire::Conversation;
    if report.event != purlis_core::state::Event::SessionStart || report.agent.is_some() {
        return None;
    }
    let Conversation::Named(said) = &report.conversation else {
        return None;
    };
    if now != Some(said.as_str()) {
        return None;
    }
    report
        .detail
        .model
        .as_ref()
        .map(purlis_core::state::Model::as_str)
}

/// Told, by chat, how many hosts its proxy refused past the ones it names (#1353).
pub type MoreRefused = Arc<dyn Fn(u32, u32) + Send + Sync + 'static>;

/// **What purlis's own proxy beside a chat it wraps tells of the hosts it refused past the ones
/// it names** (#1353): the count, under the chat's number (`whose`), to whoever `hear` holds.
/// Nothing before the chat has its number: the count is told again as it grows, and the first
/// eight hosts, told as blocks, come first.
fn more_refused_by_the_proxy(
    hear: Arc<Mutex<Option<MoreRefused>>>,
    whose: Arc<std::sync::atomic::AtomicU32>,
) -> purlis_core::sandbox::egress::MoreTold {
    Arc::new(move |more: usize| {
        let chat = whose.load(std::sync::atomic::Ordering::SeqCst);
        let Some(told) = lock(&hear).clone() else {
            return;
        };
        if chat != 0 {
            told(chat, u32::try_from(more).unwrap_or(u32::MAX));
        }
    })
}

/// **What purlis's own proxy beside a chat it wraps tells of each host it refused** (#1663):
/// the chat's block, a connection to a host named whole as the proxy heard it (`host:port`),
/// told to whoever `hear` holds once the chat has its number (`whose`, 0 before). One heard
/// before either is kept, and told by `Refusals::tell_untold` once the chat has its number
/// (#1683). The proxy's word, never the chat's: its hook leaves such a host to it
/// (`purlis_core::sandboxblock::the_proxy_tells_hosts`). Told on a thread of its own, so the
/// refusal the client is waiting for is never held up by the app's keeping of it.
fn refused_by_the_proxy(
    hear: Arc<Mutex<Option<crate::hooks::Blocks>>>,
    whose: Arc<std::sync::atomic::AtomicU32>,
    harness: Option<Harness>,
) -> purlis_core::sandbox::egress::Refusals {
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let (hear_local, whose_local) = (Arc::clone(&hear), Arc::clone(&whose));
    purlis_core::sandbox::egress::Refusals::telling_once_heard(Arc::new(
        move |host: &str, port: u16| {
            let chat = whose.load(std::sync::atomic::Ordering::SeqCst);
            let Some(told) = lock(&hear).clone() else {
                return false;
            };
            if chat == 0 {
                return false;
            }
            let block = purlis_core::hookwire::SandboxBlocked {
                chat,
                sandbox_blocked: Block {
                    operation: Operation::Connect,
                    kind: Kind::Host,
                    ours: false,
                },
                harness: harness.map(|harness| harness.name().to_owned()),
                target: Some(purlis_core::sandbox::egress::host_and_port(host, port)),
                read_back: false,
                suggested: None,
            };
            let _ = std::thread::Builder::new()
                .name("purlis-refused".into())
                .spawn(move || told(block));
            true
        },
    ))
    .telling_local({
        let (hear, whose) = (Arc::clone(&hear_local), Arc::clone(&whose_local));
        Arc::new(move |host: &str, port: u16| {
            let chat = whose.load(std::sync::atomic::Ordering::SeqCst);
            let Some(told) = lock(&hear).clone() else {
                return;
            };
            if chat == 0 {
                return;
            }
            // This machine's loopback at a port is a service on this Mac (qw384): its Block
            // names the port, which its Notice offers to allow. Any other local address (a
            // link-local address, a metadata service) is never a host to allow (#1664), so its
            // Block names none, and its Notice offers what a local socket's does.
            // A name of this Mac is that port too (qw384b), from a fixed list, never looked up.
            let target = purlis_core::sandbox::egress::host_and_port(host, port);
            let _ = std::thread::Builder::new()
                .name("purlis-refused".into())
                .spawn(move || {
                    let local = purlis_core::sandbox::localport::named_port(&target);
                    let block = purlis_core::hookwire::SandboxBlocked {
                        chat,
                        sandbox_blocked: Block {
                            operation: Operation::Connect,
                            kind: if local.is_some() {
                                Kind::LocalService
                            } else {
                                Kind::LocalSocket
                            },
                            ours: false,
                        },
                        harness: harness.map(|harness| harness.name().to_owned()),
                        target: local.map(|port| port.to_string()),
                        read_back: false,
                        suggested: None,
                    };
                    told(block);
                });
        })
    })
}

/// What an Allow reached live ([`Chats::allow_live`], #1666).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Live {
    /// The open chats whose proxy took it at once, by number: none of them restarts for it.
    pub reached: Vec<u32>,
    /// Those of them a held connection had given up on before the Allow, each told to retry.
    pub told_to_retry: Vec<u32>,
}

/// What keeps purlis's word on a chat's held connections for its next turn (#1666).
pub type NetworkWords = Arc<dyn Fn(u32, purlis_core::askoutcome::Outcome) + Send + Sync + 'static>;

/// What is told each open chat whose reach the person narrowed while it runs (#2013).
pub type Narrowed = Arc<dyn Fn(u32) + Send + Sync + 'static>;

/// What is told a project's asks may have moved (#1709).
pub type AsksMoved = Arc<dyn Fn() + Send + Sync + 'static>;

/// Who a chat's board of live asks tells (#1666), and as whom.
struct Asking {
    hear: Arc<Mutex<Option<crate::hooks::Blocks>>>,
    reached: Arc<Mutex<Option<ProxyReached>>>,
    whose: Arc<std::sync::atomic::AtomicU32>,
    who: ReachedAs,
    harness: Option<Harness>,
    /// The blocks each chat of the project is held on: what the asks registry lists (#1709).
    blocks: Arc<crate::taskblocks::Blocks>,
    moved: Arc<Mutex<Option<AsksMoved>>>,
}

/// **A chat's board of live asks** (#1666): what purlis's own proxy holds a connection on
/// while the person is asked, for the chat whose port it came in on.
///
/// - **A host allowed already** in the project at `root` (yours, or everyone's), since the chat
///   started, goes on at once and is never asked about: the person said yes to it.
/// - **An ask** raises the chat's Block on the road a refusal takes, its connection held
///   meanwhile (the window reads that from [`Chats::asking`]), and is kept in the network
///   record as an ask. A task's Block shows on its session's tab and in the needs-you queue by
///   the road every task's Block takes (#1508). **Each ask is registered in the asks registry
///   (#1709)**: held for the chat here, before and whatever the block throttle does with its
///   Notice, so the registry lists it and the window is told; it is answered there by
///   `allow_sandbox_block` and `keep_sandbox_block`, checked against what is held (#1538), one
///   ask at a time.
/// - **A timeout** is kept in the network record. The Notice stays.
/// - **Nobody to ask** (the project's hooks are not listening yet, or the chat has no number
///   yet, the first instant of its start): no Notice could be raised, so the connection is
///   refused at once, as before purlis asked live, never held its minute for an ask nobody sees.
///
/// Every answer comes from the window ([`Chats::allow_live`], [`Chats::keep_blocked_live`]);
/// nothing a chat sends reaches the board.
fn asked_by_the_proxy(asking: Asking, root: PathBuf) -> purlis_core::sandbox::plan::Asking {
    use purlis_core::sandbox::asks::Heard;
    use purlis_core::sandboxblock::{Block, Operation};
    let Asking {
        hear,
        reached,
        whose,
        who,
        harness,
        blocks,
        moved,
    } = asking;
    let askable = {
        let (hear, whose) = (Arc::clone(&hear), Arc::clone(&whose));
        Arc::new(move || {
            whose.load(std::sync::atomic::Ordering::SeqCst) != 0 && lock(&hear).is_some()
        })
    };
    let tell = Arc::new(move |heard: Heard| {
        let chat = whose.load(std::sync::atomic::Ordering::SeqCst);
        if chat == 0 {
            return;
        }
        // Told on a thread of its own, so a held connection never waits on the app's keeping.
        let told = |what: Box<dyn FnOnce() + Send>| {
            let _ = std::thread::Builder::new()
                .name("purlis-asked".into())
                .spawn(what);
        };
        let record = |target: &str, word: &'static str| {
            if let Some(keep) = lock(&reached).clone() {
                let (who, target) = (who.clone(), target.to_owned());
                told(Box::new(move || keep(chat, &who, Some(&target), word, 1)));
            }
        };
        match heard {
            Heard::Asked(targets) => {
                // Registered first, by the app's own proxy's word, never a chat's: bounded as
                // every block held is (`taskblocks::Blocks`), in memory only.
                let registered = targets
                    .iter()
                    .filter(|target| register_live(&blocks, chat, target))
                    .count();
                if registered > 0
                    && let Some(moved) = lock(&moved).clone()
                {
                    told(Box::new(move || moved()));
                }
                for target in targets {
                    record(&target, purlis_core::sandboxblock::record::ASKED);
                    let Some(raise) = lock(&hear).clone() else {
                        continue;
                    };
                    let (kind, target) = asked_as(target);
                    let block = purlis_core::hookwire::SandboxBlocked {
                        chat,
                        sandbox_blocked: Block {
                            operation: Operation::Connect,
                            kind,
                            ours: false,
                        },
                        harness: harness.map(|harness| harness.name().to_owned()),
                        target: Some(target),
                        read_back: false,
                        suggested: None,
                    };
                    told(Box::new(move || raise(block)));
                }
            }
            Heard::TimedOut(target) => {
                // It waits on the person no longer: it is held, and goes as any block does.
                if let Some(block) = live_block(&target) {
                    blocks.no_longer_waits(chat, &block);
                }
                record(&target, purlis_core::sandboxblock::record::TIMED_OUT);
            }
        }
    });
    purlis_core::sandbox::plan::Asking {
        tell,
        askable,
        known: Arc::new(move |host| {
            purlis_core::sandbox::grant::allowed_already(&root, host)
                .map(purlis_core::sandbox::reach::By::from)
        }),
    }
}

/// Holds `target`, a host and port chat `chat`'s proxy asks about, as the block it is for the
/// chat (#1709), as a grant names it: what the asks registry lists. Held as waiting on the
/// person, so a burst of later asks does not push it out while its connection waits. Whether it
/// is one a grant can name.
fn register_live(blocks: &crate::taskblocks::Blocks, chat: u32, target: &str) -> bool {
    let Some(block) = live_block(target) else {
        return false;
    };
    blocks.heard_waiting(chat, block);
    true
}

/// **What a live ask about `target`, a host and port, is raised as**: a host, or, for a port
/// of the loopback interface, a service of the runner the chat runs on, by its port (#2510).
/// Only a runner's proxy asks about a loopback port, for one it left out of the chat's plan
/// (`purlis_core::sandbox::egress`'s `left_out_at`, the one way a proxy asks about one); never a
/// service on this Mac, so no answer to it grants anything on this machine.
fn asked_as(target: String) -> (purlis_core::sandboxblock::Kind, String) {
    use purlis_core::sandboxblock::Kind;
    match purlis_core::sandbox::localport::named_port(&target) {
        Some(port) => (Kind::RunnerService, port.to_string()),
        None => (Kind::Host, target),
    }
}

/// The block a live ask about `target`, a host and port, is held as ([`asked_as`]): none for
/// what a grant cannot name.
fn live_block(target: &str) -> Option<crate::taskblocks::HeldBlock> {
    use purlis_core::sandboxblock::{Kind, Operation};
    let (kind, target) = asked_as(target.to_owned());
    let what = if kind == Kind::RunnerService {
        crate::sandboxing::GrantWhat::Port
    } else {
        crate::sandboxing::GrantWhat::Host
    };
    Some(crate::taskblocks::HeldBlock {
        operation: Operation::Connect.word().to_owned(),
        kind: kind.word().to_owned(),
        what,
        target: crate::taskblocks::normalised(what, &target)?,
    })
}

/// What hears each connection purlis's own proxy carried for a chat (#1664): the chat's number,
/// who it is ([`ReachedAs`]), the host and port (none for the hosts past what the proxy's tally
/// tells apart), the layer that let it through, and how many connections.
pub type ProxyReached =
    Arc<dyn Fn(u32, &ReachedAs, Option<&str>, &'static str, u64) + Send + Sync + 'static>;

/// Who a chat is, as the network record names its connections: known when its proxy starts, so
/// the connections its proxy tells as the chat ends are still the chat's.
#[derive(Debug, Clone, Default)]
pub struct ReachedAs {
    pub id: Option<String>,
    pub persona: Option<String>,
}

/// **What purlis's own proxy beside a chat it wraps tells of each connection it carried**
/// (#1664), coalesced by the proxy (`purlis_core::sandbox::egress::Tally`): told to whoever
/// `hear` holds once the chat has its number (`whose`, 0 before), on a thread of its own so a
/// connection is never held up by the record's write.
fn reached_by_the_proxy(
    hear: Arc<Mutex<Option<ProxyReached>>>,
    whose: Arc<std::sync::atomic::AtomicU32>,
    who: ReachedAs,
) -> purlis_core::sandbox::egress::Reached {
    Arc::new(move |target: Option<&str>, by: &'static str, times: u64| {
        let chat = whose.load(std::sync::atomic::Ordering::SeqCst);
        let Some(told) = lock(&hear).clone() else {
            return;
        };
        if chat == 0 {
            return;
        }
        let (who, target) = (who.clone(), target.map(str::to_owned));
        let _ = std::thread::Builder::new()
            .name("purlis-reached".into())
            .spawn(move || told(chat, &who, target.as_deref(), by, times));
    })
}

fn lock<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What a chat's hooks read about it (#1338, #1345), put in its `env`: the folder it was started
/// in, whether a sandbox was actually applied to it, and its own id, which its status line keeps
/// its harness's figure of the session's cost under (#1457). Set where every chat opens, and
/// only there: whatever a profile or the arming said under either name is replaced.
fn what_its_hooks_read(
    env: &mut Vec<(String, String)>,
    cwd: Option<&Path>,
    sandboxed: bool,
    id: Option<&str>,
) {
    purlis_core::hoststart::what_its_hooks_read(
        env,
        &purlis_core::hoststart::Watched { cwd, sandboxed, id },
    );
}

/// **Whose persona grants `chat` runs with, as its sandbox is compiled for them** (#1362,
/// #2691): on a profile, the grants a handoff left it holding, else its own persona's, else the
/// project's default persona (`purlis_core::start::runs_with`); on no profile, none.
fn grants_persona_of(chat: &Chat, project: &std::path::Path) -> Option<String> {
    chat.profile
        .as_ref()
        .and_then(|_| purlis_core::start::runs_with(chat, project))
}

/// **How a run this window starts is sandboxed, by its own record** (#2636): in the sandbox
/// `sandbox` compiled, by this machine's backend for `harness`, or off where it starts without
/// one.
fn ran_as_started(
    sandbox: Option<&purlis_core::sandbox::Applied>,
    harness: Option<Harness>,
) -> purlis_session_protocol::session::RunSandbox {
    use purlis_session_protocol::session::RunSandbox;
    match sandbox {
        Some(_) => RunSandbox::By(
            purlis_core::sandbox::Os::this()
                .backend_word(harness)
                .unwrap_or("proxy")
                .to_owned(),
        ),
        None => RunSandbox::Off,
    }
}

/// **Whether chat `number`, recorded as `chat`, is a task of chat `asker`**, and its lineage
/// record where it is: dispatched by `asker` as a task, never `asker` itself, and never a
/// handoff's chat (#1492). One reading for [`Chats::tasks_of`] and for the duplicate-name
/// refusal, which reads the same records under the deciding lock (#3795).
pub(crate) fn task_of(
    asker: u32,
    number: u32,
    chat: &Chat,
) -> Option<&purlis_core::reopen::HandedFrom> {
    chat.from.as_ref().filter(|from| {
        from.chat == asker && number != asker && from.mode == purlis_core::reopen::Mode::Task
    })
}

/// The built-in skills of the bundle beside this app, which count against a chat's skill budget
/// in its restart stamp as they do where the chat is armed (SK-15).
fn built_in_skills_here() -> Option<std::path::PathBuf> {
    crate::bundled_plugin_here().and_then(|plugin| purlis_core::skills::in_bundle(&plugin))
}

#[cfg(test)]
pub(crate) mod tests;

#[cfg(test)]
#[path = "chats/tests/restoring.rs"]
mod restoring_tests;

#[cfg(test)]
#[path = "chats/tests/letting_go.rs"]
mod letting_go_tests;
