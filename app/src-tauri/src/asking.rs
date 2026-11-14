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
    /// **Each task question still with its asking chat** (#4564, ADR 0107): in progress, so
    /// its task's row alone says it, and nothing here is counted by the ✋ or listed in the
    /// Inbox, which read [`Asking::asks`].
    pub with_asking_chat: Vec<WithAskingChat>,
    /// **The asks Snooze all put away** (#5647, ADR 0119): each held by a chat that holds no
    /// ask it did not have when it was snoozed. Listed apart from [`Asking::asks`], so neither
    /// the ✋ nor Asks counts them; the Chats view, which reads the board, still says the chat
    /// waits. Only [`counted`] puts any here.
    #[serde(default)]
    pub snoozed: Vec<Shown>,
    /// **When the first chat of [`Asking::snoozed`] comes back by itself** (#6018, ADR 0121
    /// decision 14), in seconds since 1970: snoozed for 1 hour or until tomorrow. The window
    /// reads the asks again then, so they are counted and listed again; none where every
    /// snoozed chat waits only for a new ask.
    #[serde(default)]
    pub snoozed_until: Option<u32>,
    /// **The asks a cool-off holds quietly** (ADR 0121 decision 7, #6010): each the same request
    /// its chat heard Don't allow to twice in a row lately. Listed apart, as snoozed asks are, so
    /// neither the ✋ nor a notification counts them, until the cool-off ends or the person
    /// ends it. Only [`counted`] puts any here.
    #[serde(default)]
    pub cooling: Vec<Shown>,
    /// When the first cool-off ends by itself, in seconds since 1970: the window reads the asks
    /// again then.
    #[serde(default)]
    pub cooling_until: Option<u32>,
    /// **The window-only asks no chat waits on** (T1-42, R-329, Q181): each a chat's whose turn
    /// has ended, drawn on that chat's finish row with its control. Listed apart from
    /// [`Asking::asks`], so neither the ✋, Asks nor a notification counts them: a Blocking ask
    /// is one a chat waits on. Only [`every_ask`] puts any here ([`finish_rows_apart`]).
    #[serde(default)]
    pub on_finish_rows: Vec<Shown>,
}

/// A task question its asking chat has, as the task's row says it: `asking steward 2 · 1m`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct WithAskingChat {
    /// The task that asked.
    pub session: u32,
    /// The chat it asks, by name as its own row names it.
    pub asker: String,
    /// When it was asked, in seconds since 1970.
    pub since: u32,
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
    /// **Until when it waits before it ends by itself** (ADR 0121 decision 5, #6006), in seconds
    /// since 1970: an **Expiring ask**, listed first with its countdown. Present only where its
    /// source ends it at a time purlis knows: a permission hook's deadline, a held connection's
    /// hold, a run grant's or a Secret ask's command hold, a login's or a fill's wait, a command's
    /// confirmation's two minutes (the table in ADR 0121's amendment of 2026-11-10).
    #[specta(optional)]
    pub expires: Option<u32>,
    /// **How many open chats wait on its chat's report** (ADR 0121 decision 5, #6006), directly
    /// or up a dispatched chain (`purlis_core::dispatchwaits`): its row says "blocks N chats".
    /// Absent where none does.
    #[specta(optional)]
    pub blocks: Option<u32>,
    /// **When its wait ended with nobody's answer** (ADR 0121 decision 15, #6007), in seconds
    /// since 1970: a **timed-out ask**, still listed and still answerable, whose row says
    /// "Timed out. Answer, and the chat is told to try again." Present only where an answer
    /// still reaches the chat: a permission hook's ask, whose answer waits for the chat's same
    /// request; a held connection that gave up, whose Allow the chat is told to retry; a run
    /// grant and a Secret ask, which the core holds open past their command. Never with
    /// [`Self::expires`].
    #[specta(optional)]
    pub timed_out: Option<u32>,
    /// **What a yes to it risks** (ADR 0121 decision 8, #6011): a push to a protected branch,
    /// a host that looks like production, a write outside the project, read from the project's
    /// own facts (`purlis_core::askrisk`). Its row says it, it is never grouped and never
    /// offered as a rule, and its Allow asks once more. Absent where it risks none of these.
    #[specta(optional)]
    pub risk: Option<RiskShown>,
    /// **What an edit would write** (#1146): each change, drawn before its Allow, or why it is
    /// not shown and no Allow is offered. Absent for any other ask.
    #[specta(optional)]
    pub writes: Option<crate::askwrites::WritesShown>,
    /// **The lines a permission's action runs over** (#3502), where its line puts several on
    /// one: their count, and each as [`Shown::says`] shows it. Absent for an action on one line.
    #[specta(optional)]
    pub lines: Option<LinesShown>,
    /// **What its chat's turn ended on** (#4550): on a chat waiting on the person's reply, its
    /// last message, masked and bounded as a report's text is, over the lines it has. Data,
    /// never markup. Absent where the app heard none, and on every other ask.
    #[specta(optional)]
    pub last_message: Option<String>,
    /// **The site a login would be filled on** (#4595): drawn large and editable. Absent for any
    /// other ask.
    #[specta(optional)]
    pub login: Option<LoginShown>,
    /// **The few messages its chat's turns ended on before its last** (#4563), oldest first,
    /// kept as [`Shown::last_message`] is: on a chat waiting on the person's reply, where it said
    /// any, so a question a later turn pushed aside can be read back. Absent otherwise.
    #[specta(optional)]
    pub earlier_messages: Option<Vec<String>>,
    /// **The key a Secret ask asks for** (#5068, ADR 0109): drawn with a masked field the value
    /// is typed in. Absent for any other ask.
    #[specta(optional)]
    pub secret: Option<SecretShown>,
    /// **The program a run asks the person to allow** (ADR 0112, #5239): the app's facts, kept
    /// apart from the chat's command. Absent for any other ask.
    #[specta(optional)]
    pub run: Option<RunShown>,
    /// **The secret a fill would put into a page, and where** (#5115): drawn with its site
    /// large. Absent for any other ask.
    #[specta(optional)]
    pub fill: Option<FillShown>,
    /// **What its request is called, and the rule it offers** (ADR 0121 decision 7, #6010):
    /// present where its request can be named, so its answer is noted against it. Absent on an
    /// ask that is never offered a rule nor cooled off.
    #[specta(optional)]
    pub rule: Option<RuleShown>,
}

/// **An ask's request, and the rule it offers** (#6010).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RuleShown {
    /// The request's name: two asks with the same are the same request.
    pub request: String,
    /// The rule offered, where the same Allow was given it twice in a week.
    #[specta(optional)]
    pub offer: Option<RuleOffered>,
}

/// **A rule an ask offers** (#6010): the Allow given each time, how often, and its wider grants
/// that a rule may be made of, the one suggested first.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RuleOffered {
    /// The id of the Allow given each time.
    pub answered: String,
    /// How many times, in the last week.
    pub times: u32,
    /// The option ids of the grants a rule may be made of, the suggested one first: the
    /// narrowest that would have prevented every ask, else the widest there is.
    pub options: Vec<String>,
}

/// **An ask's risk, as its row says it** (#6011): a mirror of `purlis_core::askrisk::Risk`,
/// which the core has no window types for.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RiskShown {
    pub class: RiskClass,
    /// What the row says of it: "Pushes to main, a protected branch".
    pub why: String,
    /// What its Allow asks before it acts: "Allow a push to main?".
    pub confirm: String,
}

/// **What a merge row shows of its request** (T1-40; R-120, R-147, Q248), every one of them read
/// from the forge with the app's own login, never from the chat
/// (`purlis_core::mergehold::Facts`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct MergeShown {
    /// The request's title, written out inertly: its author may be the chat, so the row draws it
    /// as text the chat may have written (Q248).
    pub title: String,
    /// Who opened it, by the forge's login, written out inertly.
    pub author: String,
    /// The branch it merges into: the ask's key's.
    pub base: String,
    /// The checks at its head, as one of the five words: `PASSED`, `FAILED`, `RUNNING`, `NOT
    /// RUN` or `UNKNOWN`.
    pub checks: String,
    /// When the forge was read for these, in seconds since 1970: "Checks PASSED as of 14:02".
    pub read_at: u32,
    /// Whether a ruleset protects the base, as purlis read it.
    pub ruleset: RulesetShown,
    /// What the person has seen of its head (T1-41, R-122): viewed in purlis, approved on the
    /// forge by someone else, or neither. A merge takes two presses whichever it is (Q268).
    pub look: LookShown,
    /// Where it is approved: the login of the person who approved this head (Q277), said
    /// inertly, as text after purlis's own words and never as a control.
    pub approved_by: Option<String>,
    /// Where it was not viewed: how many commits its head is ahead of the one the person
    /// viewed, where purlis knows it (R-123): "2 commits since you looked".
    pub since_viewed: Option<u32>,
    /// Whether purlis recorded the asking chat pushing its head (R-124). A head it did not is
    /// flagged.
    pub pushed_here: bool,
    /// The digest of what these facts show (`purlis_core::mergehold::Facts::digest`): Merge
    /// hands it back, and merges only while the ask shows the same.
    pub digest: String,
}

/// **What the person has seen of a merge's head** (T1-41, R-122), as its row says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum LookShown {
    /// Viewed in purlis, at this head.
    Viewed,
    /// Approved at this head on the forge, by a person who is neither its author nor an app or
    /// a bot.
    Approved,
    /// Neither.
    NotViewed,
}

/// **Whether a ruleset protects a merge's base** (R-147), as its row shows it and its Merge hands
/// back (R-150).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum RulesetShown {
    /// A ruleset protects it.
    Protects,
    /// None does.
    None,
    /// The forge did not say.
    Unknown,
}

impl From<purlis_core::mergehold::Seen> for RulesetShown {
    fn from(seen: purlis_core::mergehold::Seen) -> Self {
        use purlis_core::mergehold::Seen;
        match seen {
            Seen::Protects => Self::Protects,
            Seen::None => Self::None,
            Seen::Unknown => Self::Unknown,
        }
    }
}

impl From<RulesetShown> for purlis_core::mergehold::Seen {
    fn from(shown: RulesetShown) -> Self {
        match shown {
            RulesetShown::Protects => Self::Protects,
            RulesetShown::None => Self::None,
            RulesetShown::Unknown => Self::Unknown,
        }
    }
}

impl MergeShown {
    /// What a merge row shows of `facts`, read for a request into `base`, with what the person
    /// has seen of its head (`look`) and whether its chat pushed it.
    pub fn of(
        facts: &purlis_core::mergehold::Facts,
        base: &str,
        look: purlis_core::mergehold::Look,
        pushed_here: bool,
    ) -> Self {
        use purlis_core::mergehold::Look;
        let (look, approved_by, since_viewed) = match look {
            Look::Viewed => (LookShown::Viewed, None, None),
            Look::Approved { by } => (LookShown::Approved, Some(said_inertly(&by)), None),
            Look::NotViewed { since } => (LookShown::NotViewed, None, since),
        };
        Self {
            title: said_inertly(&facts.title),
            author: said_inertly(&facts.author),
            base: base.to_owned(),
            checks: facts.checks.ci.word().to_owned(),
            read_at: u32::try_from(facts.read_at).unwrap_or(u32::MAX),
            ruleset: facts.ruleset.seen().into(),
            look,
            approved_by,
            since_viewed,
            pushed_here,
            digest: facts.digest(),
        }
    }
}

/// Which risk an ask carries ([`RiskShown`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum RiskClass {
    ProtectedPush,
    ProductionHost,
    OutsideProject,
    /// A delete its command or tool plainly says (#6178).
    Delete,
    /// A credential handed from a purlis vault to a chat's program, a page or a runner (#6178).
    VaultUse,
    /// A merge a chat's hook held, which the app performs on the person's press (Q239).
    Merge,
}

impl From<purlis_core::askrisk::Risk> for RiskShown {
    fn from(risk: purlis_core::askrisk::Risk) -> Self {
        use purlis_core::askrisk::Class;
        Self {
            class: match risk.class {
                Class::ProtectedPush => RiskClass::ProtectedPush,
                Class::ProductionHost => RiskClass::ProductionHost,
                Class::OutsideProject => RiskClass::OutsideProject,
                Class::Delete => RiskClass::Delete,
                Class::VaultUse => RiskClass::VaultUse,
                Class::Merge => RiskClass::Merge,
            },
            why: risk.why,
            confirm: risk.confirm,
        }
    }
}

/// **What a yes to the permission `ask` risks** (#6011), against the project's `facts`. A push
/// that names no branch is read by where `pushes` says its folder's branch lands (#6178), and an
/// edit the harness's own protocol names a delete (ACP's `delete` kind) is one.
pub fn risk_of_action(
    ask: &purlis_core::harness::model::Ask,
    facts: &purlis_core::askrisk::Facts,
    pushes: purlis_core::askrisk::PushesIn<'_>,
) -> Option<RiskShown> {
    use purlis_core::harness::model::Risk;
    match &ask.action {
        Action::Command { line } => facts.of_command_in(line, pushes),
        Action::Edit { path } if ask.risk == Risk::Delete => facts
            .of_write(path)
            .or_else(|| Some(purlis_core::askrisk::delete_of(path))),
        Action::Edit { path } => facts.of_write(path),
        Action::Tool { input, .. } => facts.of_tool(input),
        Action::Unsaid | Action::Elicit { .. } | Action::Question { .. } => None,
    }
    .map(RiskShown::from)
}

/// **The vault use an Allow on it would make** (#6178): `what` handed from `vault` to `to`.
fn vault_use(what: &str, vault: Option<&str>, to: &str) -> Option<RiskShown> {
    Some(purlis_core::askrisk::vault_use(what, vault, to).into())
}

/// **An ask to allow a program, as its row draws it** (ADR 0112, #5239). Every field is the app's
/// own record of the run but [`Self::command`], which is the chat's words made inert: spelled so
/// no character in them changes what they seem to say, and drawn as the chat's. Names only: no
/// value is ever here.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RunShown {
    pub persona: String,
    pub vault: String,
    /// The program's full path, as the app resolved it where no chat writes.
    pub program: String,
    /// Each hand-over: the variable, the key, and whether it is a file or the value itself.
    pub values: Vec<RunValueShown>,
    /// The hosts this chat's sandbox lets the run reach.
    pub hosts: Vec<String>,
    /// The chat's sandbox holds the run to no host.
    pub every_host: bool,
    /// T-4's sentence, where the program is a general-purpose one.
    #[specta(optional)]
    pub caution: Option<String>,
    /// The vault is not one the persona may use: an Allow opens it for that grant's scope alone.
    pub not_its_vault: bool,
    /// The chat's command, its words made inert.
    pub command: String,
    /// The folder it starts in.
    pub cwd: String,
    /// What Always allow would write, where it is offered (#5245).
    #[specta(optional)]
    pub always: Option<AlwaysShown>,
    /// **The keys the run lacks**, one value field each (#5242).
    pub fills: Vec<RunFillShown>,
    /// **What the notice asks of the program** (#5242): `program` for an Allow; `vault` where it
    /// has none and the vault's line has its own Allow; `fill` for its value fields alone.
    pub offer: String,
    /// Why it offers no Allow for the program, where policy is why: the app's own words.
    #[specta(optional)]
    pub no_allow: Option<String>,
    /// **Asked ahead of its run** (#5244): no command yet; only Allow for this chat is offered,
    /// and the first run fixes how the values are handed.
    pub ahead: bool,
    /// **What *Remember this need for the persona* would write** (#5080, #5240), where a field's
    /// key is one the persona does not declare and it may: the file, and a line per such field.
    /// No `vault:` line: a run names its vault in its command.
    #[specta(optional)]
    pub remember: Option<SecretNeedShown>,
}

/// **A key a run lacks, as its notice's value field** (#5242). Names only.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RunFillShown {
    pub key: String,
    /// What the persona's definition says it is for, where it declares it.
    #[specta(optional)]
    pub declared: Option<String>,
    /// The persona declares no Secret need for it: Remember would add one (#5080).
    pub need: bool,
    /// The entry a link read, which is gone, where the key is such a link.
    #[specta(optional)]
    pub gone: Option<SecretEntryShown>,
}

/// **What Always allow writes** ([`RunShown`], #5245): the exact line, and whether it hands a value
/// by a variable as the command did, where a file is safer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct AlwaysShown {
    pub line: String,
    pub by_env: bool,
    /// **Something else of the persona's list waits on the person's Allow** (#5080): Always
    /// allow approves this line alone, and the persona's usual Notice stays up for the rest.
    pub others_wait: bool,
}

/// One value a run asks to hand its program ([`RunShown`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RunValueShown {
    pub var: String,
    pub key: String,
    /// In a private file, whose path is in the variable; otherwise in the variable itself.
    pub in_a_file: bool,
}

/// **A Secret ask, as its row draws it** (#5068, ADR 0109): the persona, the vault and the key
/// its chats miss, and how many wait on it. Names only: no value is ever here.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SecretShown {
    pub persona: String,
    pub vault: String,
    pub key: String,
    /// The declared tool whose run missed it, where it is one.
    #[specta(optional)]
    pub tool: Option<String>,
    /// **What the chat says it is for** (#5072), in its own words: drawn as the chat's, never
    /// as purlis's. Absent for a broker miss.
    #[specta(optional)]
    pub purpose: Option<String>,
    /// Whether the persona declares the key as a Secret need: one it does not is drawn with a
    /// quiet "not declared by this persona" tag (#5072).
    pub declared: bool,
    /// **What the persona's committed definition says the key is for** (#5076), where it
    /// declares it: the maintainers' words, quoted as theirs.
    #[specta(optional)]
    pub declared_purpose: Option<String>,
    /// How many chats wait on it.
    pub waiting: u32,
    /// **Why the run that missed it is offered no Allow** (ADR 0112, #5243): the app's own words.
    #[specta(optional)]
    pub no_allow: Option<String>,
    /// **The chat says its value doesn't work** (#5075): the chat's own words, drawn as the
    /// chat's. Absent on an ask for a key the vault doesn't hold.
    #[specta(optional)]
    pub replace: Option<String>,
    /// The entry the key reads, where it is a Linked entry asked to be replaced (#5075): a new
    /// value goes there.
    #[specta(optional)]
    pub reads: Option<SecretEntryShown>,
    /// The entry a broken link read, which is gone (#5075).
    #[specta(optional)]
    pub gone: Option<SecretEntryShown>,
    /// **What *Remember this need for the persona* would write** (#5240, spec #5235), where it
    /// would write anything and may.
    #[specta(optional)]
    pub remember: Option<SecretNeedShown>,
}

/// **The line a Secret ask's Remember writes** (#5240): shown whole before it is ticked for.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SecretNeedShown {
    /// The file it goes in, relative to the project.
    pub file: String,
    /// The persona does not declare the key, so a `secret-need.<key>` line goes in.
    pub need: bool,
    /// The chat's words for it, for the person to edit: empty where it said none.
    pub purpose: String,
    /// The vault a `vault:` line names, where the persona names none.
    #[specta(optional)]
    pub vault: Option<String>,
}

/// An entry of a vault named in a Secret ask (#5075): the person's to read, never a chat's.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SecretEntryShown {
    pub vault: String,
    pub key: String,
}

/// **A secret fill, as its notice draws it** (#5115): the key's name and its vault, the exact
/// site, the page and the field's label; never a value, which purlis hasn't read.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct FillShown {
    pub key: String,
    pub vault: String,
    /// The site the field's own frame is on, as purlis read it: its host, with a port where it
    /// has one, and the scheme where it isn't https (`http://localhost:3000`).
    pub site: String,
    /// The page the field is on, where the browser said: an address.
    #[specta(optional)]
    pub page: Option<String>,
    /// What the page labels the field: the page's words, never purlis's.
    pub label: String,
    /// The chat's browser, by its name to the chat (`b1`).
    pub browser: String,
    /// Plain warnings, said above the choices.
    pub warnings: Vec<String>,
    /// **A card's or payment value's key** (#5116, SF-4): Allow once and Keep blocked alone.
    pub card: bool,
    /// **A multi-line text box or an editable area** (#5120, SF-5): asked each time, with Allow
    /// once and Keep blocked alone.
    pub multi_line: bool,
    /// The line Always allow writes, shown before it's pressed (#5116).
    #[specta(optional)]
    pub line: Option<String>,
    /// Where the line goes: `[sandbox.personas.<persona>] tools`.
    #[specta(optional)]
    pub under: Option<String>,
    /// Why Always allow isn't offered, where it isn't and the key is no card's.
    #[specta(optional)]
    pub no_always: Option<String>,
    /// Always also allows the vault for the persona.
    pub vault_too: bool,
    /// **A 2FA code the seed under `key` makes, not the value itself** (#5118): the notice says
    /// so, and that the seed reaches no page.
    pub code: bool,
}

/// **A suggested login, as its notice draws it** (#4595). The vault and key names, and never a
/// value: purlis hasn't read one.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct LoginShown {
    /// The domain of the page it opens, with a port where it has one: `shop.example.com`.
    pub domain: String,
    /// The page's path on it.
    pub path: String,
    /// **The host the password is typed on** (#4995), with a port where it has one: the login
    /// form's, which the page may have sent the browser to. The domain where they're one.
    pub form: String,
    /// **The part of the domain's host its owner registered** (the person's ruling of
    /// 2026-11-07), by the public suffix list: the end of its host, which the notice marks.
    pub registrable: String,
    /// The same of the form's host.
    pub form_registrable: String,
    pub vault: String,
    #[specta(optional)]
    pub user_key: Option<String>,
    pub key: String,
    /// Plain warnings, said above the choices.
    pub warnings: Vec<String>,
    /// The line Always writes, shown before it's pressed, and where.
    pub line: String,
    pub under: String,
    /// Why Always isn't offered, where it isn't.
    #[specta(optional)]
    pub no_always: Option<String>,
    /// The persona's chats get the vault too with Always.
    pub vault_too: bool,
    /// **The page the declared login opens now, which this replaces** (#4996).
    #[specta(optional)]
    pub replaces: Option<String>,
    /// The host its password is typed on now, with a port where it has one.
    #[specta(optional)]
    pub replaces_form: Option<String>,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
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
    /// A sandboxed Codex chat, or on Linux a sandboxed Claude Code chat, refused for want of a
    /// sandbox-only profile (#2037): answered by making one and running the harness's own
    /// sign-in in it.
    SandboxOnly,
    /// A project that still holds the old `[harness_plugins]` keys purlis no longer reads
    /// (#3467): said once, answered by Remove, which proposes the edit, or Dismiss.
    OldHarnessPlugins,
    /// A chat's first browser call, asking what its browser may reach (#2013).
    Browser,
    /// **The lanes a forge refused for unsigned commits** (#5633, ADR 0116 §3): one Ask for all
    /// of them, said once, counted once.
    Signing,
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
    /// A browser login a chat suggested, which its persona doesn't declare (#4595): an Allow
    /// lets purlis fill it on the one origin the person confirmed.
    BrowserLogin,
    /// A key a chat's brokered run found missing from its persona's vault (#5068, ADR 0109):
    /// answered by a value typed in the window, or Not now.
    Secret,
    /// A brokered run of a program its persona does not declare (ADR 0112, #5239): an Allow is a
    /// Run grant, and Don't allow is remembered for that chat, program and vault.
    RunGrant,
    /// A secret a chat asks purlis to fill into a field of its browser (#5115): an Allow lets
    /// purlis put that one key into that field, on that one origin, once.
    SecretFill,
    /// A vault a chat was refused because its persona is not tagged for it (#1430, #5160): an
    /// Allow lets that persona use it in this project on this machine.
    Vault,
    /// **What the project expects and this machine has not allowed** (#6000, qw354): its
    /// sandboxed chats are refused it until the person allows it on this machine.
    SandboxExpects,
    /// **A persona whose hosts or tools wait for an Allow on this machine** (ADR 0121, amended
    /// 2026-11-12): its chats reach none of its hosts and run none of its tools until the
    /// person allows them as shown. One per persona.
    PersonaAllow,
    /// **A `purlis` command's confirmation in the window** (#6000): a pool sign-in, a profile
    /// taken off a pool, a signing check, each waiting on the person's yes.
    Confirmation,
    /// **A chat paused at its budget** (#6015, ADR 0121 decision 20): a dispatch limit the
    /// person set, reached. Continue, Raise the budget, or Stop.
    Budget,
    /// **A chat paused as looping** (#6015, ADR 0121 decision 20): the same command failing
    /// again and again, or its asks refused again and again. It's fine, continue, or Stop.
    Looping,
    /// **A merge a chat's hook held** (R-114, R-115): the chat never merges. Merge is the
    /// app's own forge call, on the person's press, and Don't merge merges nothing.
    MergeHold,
    /// **A refusal whose only way out is Trust this clone** (R-328, R-330): a GitHub App token
    /// that couldn't be minted for a clone purlis has no record of. Trust this clone records it,
    /// as the clone's own Notice does, and the chat's next mint goes through.
    TrustClone,
    /// **The clones purlis has no record of** (R-336, Q184): one ask for all of them, no chat's,
    /// never one per clone. Trust all records each, as its Notice's Trust this clone does.
    TrustClones,
    /// **Pool setup's cluster is ready** (#4496): the admin the person asked has given what
    /// the install couldn't, and Pool setup goes on. Said once, about no chat.
    PoolSetup,
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
    /// Make one sends `make_sandbox_only_profile` for `harness` (the kind's word, `codex` or
    /// `claude`), then opens a shell tab where the core types the harness's own sign-in for the
    /// profile it made (`type_profile_sign_in`). Dismiss sends `dismiss_sandbox_only`.
    SandboxOnly { harness: String },
    /// `remove_old_harness_plugins` for Remove, which holds purlis's proposed change for each
    /// file and opens the first; `dismiss_old_harness_plugins` for Dismiss (#3467).
    OldHarnessPlugins,
    /// Set up signing opens the setup dialog in the window, whose last step is Sign and retry
    /// all; Sign and retry all sends `signing_sign_and_retry` for every waiting lane; Dismiss
    /// sends `signing_forget` for them all (#5633).
    Signing,
    /// Open Pool setup sends `pool_setup_stop_waiting` and opens the project's Pool setup tab;
    /// Dismiss sends `pool_setup_stop_waiting` alone (#4496).
    PoolSetup,
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
    /// `answer_browser_login` at the press an option names, on held login `id`, for the proposal
    /// whose digest is `shown`, with the domain the person typed (#4595).
    BrowserLogin { id: u32, shown: String },
    /// `answer_secret_ask` on Secret ask `id` (#5068): Fill with the value typed in the ask's
    /// masked field, Not now with none.
    Secret { id: u32 },
    /// `answer_run_grant` on the ask `id`, at the press an option names (ADR 0112).
    RunGrant { id: u32 },
    /// `answer_secret_fill` at the press an option names, on held fill `id`, for the notice
    /// whose digest is `shown` (#5115).
    SecretFill { id: u32, shown: String },
    /// The refused vault Notice's own commands, for chat [`Shown::session`]'s refusal of
    /// `vault` (#5160): `allow_refused_vault` for Allow, the one grant its Notice makes, and
    /// `keep_vault_blocked` for Keep blocked, with the person's reason.
    Vault { vault: String },
    /// `allow_project_expects` for Allow on this machine and `decline_project_expects` for Not
    /// now, each sent `items` as its Notice sends them (#6000).
    SandboxExpects {
        items: Vec<crate::sandboxing::ExpectedItem>,
    },
    /// `allow_persona_hosts` for Allow, sent `digest`, the list the row showed, as its Notice
    /// sends it; and `persona_allow_not_now` for Not now, which puts it away while the project
    /// is open and the list stands (ADR 0121, amended 2026-11-12).
    PersonaAllow { persona: String, digest: String },
    /// `answer_pool_sign_in` for confirmation `id`, yes or no, as its dialog answers (#6000).
    Confirmation { id: u32 },
    /// `answer_pause` for the chat it names, with the option chosen (#6015); Raise the budget is
    /// the window's own, opening Settings where the limit is.
    Paused,
    /// `merge_held_request` for Merge and `dont_merge_held_request` for Don't merge, each for
    /// request `number` of `repo` at `head`, the key the ask was raised at (R-115). `facts` are
    /// what the row shows of it, as the forge last gave them (T1-40): Merge hands back the
    /// ruleset state and the digest of them, which it is bound to (R-150, T1-41), and
    /// `reread_held_merge` reads them again before Merge asks once more (Q248).
    /// `view_held_merge` shows its changes and marks its head viewed (R-302).
    MergeHold {
        repo: String,
        number: u32,
        head: String,
        facts: MergeShown,
    },
    /// `vouch_clone` for Trust this clone, on the clone at `folder` (relative to the project)
    /// at `place`, the address its remote named when the refusal was raised (R-330): the clone
    /// Notice's own command, which clears the ask.
    TrustClone { folder: String, place: String },
    /// `vouch_clone` for Trust all, on each of `clones` at the one address its remote names: the
    /// clone Notice's own command, and each clone it trusts leaves the ask (R-336).
    TrustClones {
        clones: Vec<crate::clonevouch::CloneToVouch>,
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
    /// **Who a grant is for, where a rule may be made of it** (ADR 0121 decision 7, #6010):
    /// this chat, this project or this machine. Absent on an answer that is no grant, and on
    /// one whose rule Rules I made could not undo (a harness's saved rule, which the window never
    /// writes: #6177).
    #[specta(optional)]
    pub scope: Option<RuleScope>,
}

/// **Who a rule is for** (ADR 0121 decision 7, #6010), narrowest first.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    specta::Type,
)]
#[serde(rename_all = "kebab-case")]
pub enum RuleScope {
    Chat,
    Project,
    Machine,
}

impl From<RuleScope> for purlis_core::askrules::Scope {
    fn from(scope: RuleScope) -> Self {
        match scope {
            RuleScope::Chat => Self::Chat,
            RuleScope::Project => Self::Project,
            RuleScope::Machine => Self::Machine,
        }
    }
}

impl From<purlis_core::askrules::Scope> for RuleScope {
    fn from(scope: purlis_core::askrules::Scope) -> Self {
        use purlis_core::askrules::Scope;
        match scope {
            Scope::Chat => Self::Chat,
            Scope::Project => Self::Project,
            Scope::Machine => Self::Machine,
        }
    }
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
        // A hook holds no task question: `every_ask` lists those.
        with_asking_chat: Vec::new(),
        snoozed: Vec::new(),
        snoozed_until: None,
        cooling: Vec::new(),
        cooling_until: None,
        on_finish_rows: Vec::new(),
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
                scope: None,
                id: option.id,
                label: option.label,
            })
            .collect(),
        source: AskSource::Permission,
        chain: Vec::new(),
        answer: AnswerPath::Hook,
        since: None,
        held_until: None,
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes,
        lines,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
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
/// `raised` is told of each ask held before the window is. The app hears give-ups too
/// ([`permitting_told_or_given_up`]); this is the tests' way in.
#[cfg(test)]
pub fn permitting(
    plane: PlaneId,
    hooks: Arc<HookAsks>,
    telling: Telling,
    raised: purlis_core::hookwire::permission::Raising,
) -> Permitting {
    permitting_told_or_given_up(plane, hooks, telling, raised, Arc::new(|_| {}))
}

/// What hears a permission hook's ask: `raised` told of each ask held before the window is
/// (R-229), and each ask `hooks` will not hold told to `gave_up` (R-204): its chat then waits
/// in its terminal ([`Waiting::in_its_terminal`]).
pub fn permitting_told_or_given_up(
    plane: PlaneId,
    hooks: Arc<HookAsks>,
    telling: Telling,
    raised: purlis_core::hookwire::permission::Raising,
    gave_up: purlis_core::hookwire::permission::GivingUp,
) -> Permitting {
    let told = Arc::clone(&hooks);
    purlis_core::hookwire::permission::held_told_or_given_up(
        hooks,
        Arc::new(move || tell(&plane, &told, &telling)),
        raised,
        gave_up,
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

/// What [`asks_waiting`]'s reads are named off the window's thread (#5582).
pub(crate) const READING_ASKS: &str = "reading the asks waiting";

/// **Every ask a project has waiting now** (#1690), from every source ([`every_ask`]): what
/// the window lists and the title bar counts. Read again whenever a source may have moved:
/// [`EVENT`], a chat moving, a dispatch held, a block heard, an answer.
///
/// The window's alone: what waits on the person, dispatches and refused hosts included, is
/// never a link's to read, as `dispatch_grants_needed` is not.
///
/// Read off the window's thread (#5582): it reads the project's files (its locks, sign-ins and
/// removals, and each waiting persona's tools), and the window asks it on every burst of moves.
/// Two reads may answer out of order, and the window draws only the newest (`asks.ts`).
#[tauri::command]
#[specta::specta]
pub async fn asks_waiting(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Asking, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window(READING_ASKS, move || Ok(counted(&held))).await
}

/// **What the ✋ counts of project `held`** (ADR 0121 decisions 2 and 3): its Blocking asks,
/// from every source ([`every_ask`]), with what Snooze all put away apart ([`snoozed_apart`]).
/// The window lists it, and the system notifications (`asknotify`) are sent for it alone, so
/// what notifies and what counts are one answer. A snoozed chat neither counts nor notifies
/// until it is back (ADR 0121 decision 14, #6018).
pub fn counted(held: &crate::planes::Held) -> Asking {
    // Snooze all's record is this machine's (#5647): where the app keeps no store, as in a
    // test of its own, nothing is snoozed and no data home is looked for.
    let store = held
        .config()
        .and_then(|_| purlis_core::inboxupdates::Store::here());
    let now = crate::sandboxing::now_secs();
    let asking = snoozed_apart(every_ask(held), store.as_ref(), held.root(), now);
    // The answers and rules are this machine's too (#6010): none where it keeps no store.
    match held
        .config()
        .and_then(|_| purlis_core::askrules::Store::here())
    {
        Some(rules) => with_rules(
            asking,
            &rules.read(),
            &purlis_core::askrules::project_of(held.root()),
            now,
        ),
        None => asking,
    }
}

/// **Snooze all's asks, put apart** (#5647): every snoozed chat that holds an ask it did not
/// have when snoozed, or whose time has come by `now` (#6018), is let go of first, so it is back
/// with all its asks; what each chat still snoozed holds moves from [`Asking::asks`] to
/// [`Asking::snoozed`], and the first time one comes back by itself is
/// [`Asking::snoozed_until`]. Where this machine keeps no data home, nothing is snoozed; where
/// the record cannot be written, it is read as it is, less any chat whose time has come.
pub fn snoozed_apart(
    mut asking: Asking,
    store: Option<&purlis_core::inboxupdates::Store>,
    root: &std::path::Path,
    now: u64,
) -> Asking {
    let Some(store) = store else {
        return asking;
    };
    let mut held = purlis_core::inboxupdates::Held::new();
    for ask in &asking.asks {
        held.entry(ask.session).or_default().push(ask.ask.clone());
    }
    let away = store.release(root, &held, now).unwrap_or_else(|_| {
        let mut kept = store.snoozed(root);
        kept.retain(|_, one| one.until.is_none_or(|until| now < until));
        kept
    });
    if away.is_empty() {
        return asking;
    }
    let (snoozed, asks) = asking
        .asks
        .into_iter()
        .partition(|ask| away.contains_key(&ask.session));
    asking.asks = asks;
    asking.snoozed = snoozed;
    asking.snoozed_until = purlis_core::inboxupdates::first_back(&away)
        .map(|until| u32::try_from(until).unwrap_or(u32::MAX));
    asking
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

/// **Who a sandbox grant at `level` is for, as a rule** (#6010): this chat, this project (on
/// this machine, or committed for everyone), or every project here. Once is no rule.
fn level_scope(level: crate::sandboxing::GrantLevel) -> Option<RuleScope> {
    use crate::sandboxing::GrantLevel;
    match level {
        GrantLevel::Once => None,
        GrantLevel::Chat => Some(RuleScope::Chat),
        GrantLevel::You | GrantLevel::Project => Some(RuleScope::Project),
        GrantLevel::Machine => Some(RuleScope::Machine),
    }
}

/// **Whether `ask` is high-risk** (ADR 0121 decision 8, #6011): one is never offered a rule. It
/// is so where its row carries a risk (#6178), the same predicate as `asksSame.ts`'s `highRisk`
/// (#6177).
pub fn high_risk(ask: &Shown) -> bool {
    ask.risk.is_some()
}

/// **What `ask`'s request is called** (#6010), where it can be named: two asks with the same name
/// are the same request, as identical requests are one row of Asks (`asksSame.ts`, #6009). A
/// sandbox's host, folder or service by its target; a permission by its line (an edit's change
/// and a command over several lines are never named); a run of a program by its program, vault,
/// persona, folder and command, which ADR 0112 binds a grant to.
pub fn request_of(ask: &Shown) -> Option<String> {
    let asked = match (&ask.source, &ask.answer) {
        (
            AskSource::SandboxHost | AskSource::SandboxWrite | AskSource::SandboxLocal,
            AnswerPath::SandboxBlock { shown },
        ) => shown.target.clone(),
        (AskSource::Permission, AnswerPath::Hook)
            if ask.writes.is_none() && ask.lines.is_none() =>
        {
            ask.says.clone()
        }
        (AskSource::RunGrant, _) => {
            let run = ask
                .run
                .as_ref()
                .filter(|run| run.offer == "program" && run.fills.is_empty())?;
            serde_json::to_string(&[
                &run.program,
                &run.vault,
                &run.persona,
                &run.cwd,
                &run.command,
            ])
            .ok()?
        }
        _ => return None,
    };
    serde_json::to_string(&[serde_json::to_value(ask.source).ok()?, asked.into()]).ok()
}

/// **The rule `ask` offers** under `offer` (#6010): its grants wider than the Allow given each
/// time that a rule may be made of, the suggested one first: the narrowest that would have
/// prevented every ask, else the widest there is. None where it has no such grant, or is
/// high-risk.
fn offered(ask: &Shown, offer: purlis_core::askrules::Offer) -> Option<RuleOffered> {
    if high_risk(ask) {
        return None;
    }
    let given = ask
        .options
        .iter()
        .find(|one| one.id == offer.answered)?
        .scope;
    let wider: Vec<(&Offered, RuleScope)> = ask
        .options
        .iter()
        .filter(|one| one.allows)
        .filter_map(|one| Some((one, one.scope?)))
        .filter(|(_, scope)| given.is_none_or(|given| *scope > given))
        .collect();
    let needs = RuleScope::from(offer.needs);
    let suggested = wider
        .iter()
        .filter(|(_, scope)| *scope >= needs)
        .min_by_key(|(_, scope)| *scope)
        .or_else(|| wider.iter().max_by_key(|(_, scope)| *scope))?
        .0;
    Some(RuleOffered {
        answered: offer.answered,
        times: offer.times,
        options: std::iter::once(suggested)
            .chain(
                wider
                    .iter()
                    .map(|(one, _)| *one)
                    .filter(|one| one.id != suggested.id),
            )
            .map(|one| one.id.clone())
            .collect(),
    })
}

/// **Asks less** (#6010): each ask of `asking` named by its request, with the rule it offers
/// under `record` at `now`; and the asks a cool-off holds put apart, in [`Asking::cooling`].
pub fn with_rules(
    mut asking: Asking,
    record: &purlis_core::askrules::Record,
    project: &str,
    now: u64,
) -> Asking {
    use purlis_core::askrules::{self, Asked};
    let mut kept = Vec::new();
    for mut ask in std::mem::take(&mut asking.asks) {
        let Some(request) = request_of(&ask) else {
            kept.push(ask);
            continue;
        };
        let asked = Asked {
            project: project.to_owned(),
            session: ask.session,
            request: request.clone(),
        };
        let offer = askrules::offer(record, &asked, now).and_then(|offer| offered(&ask, offer));
        ask.rule = Some(RuleShown { request, offer });
        match askrules::cooling(record, &asked, now) {
            Some(until) => {
                let until = u32::try_from(until).unwrap_or(u32::MAX);
                asking.cooling_until = Some(asking.cooling_until.map_or(until, |at| at.min(until)));
                asking.cooling.push(ask);
            }
            None => kept.push(ask),
        }
    }
    asking.asks = kept;
    asking
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
        // A sandbox's grant is on Settings' Granted list, where Rules I made undoes it.
        scope: if sandbox { level_scope(*level) } else { None },
    })
}

fn refuses(id: &str, label: &str) -> Offered {
    Offered {
        id: id.to_owned(),
        label: label.to_owned(),
        allows: false,
        preselected: None,
        scope: None,
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
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
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
            expires: None,
            blocks: None,
            timed_out: None,
            risk: None,
            writes: None,
            lines: None,
            last_message: None,
            login: None,
            fill: None,
            earlier_messages: None,
            rule: None,
            secret: None,
            run: None,
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
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
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
    // A held folder is answered on its own Notice alone, which has room for what allowing it
    // means, and only ever for this chat: the ask is listed so the Notice is drawn, and nothing
    // here sends its answer.
    let held = purlis_core::sandbox::names_what_may_be_lifted(std::path::Path::new(&block.target));
    let levels: Vec<GrantLevel> = [GrantLevel::Chat, GrantLevel::You]
        .into_iter()
        .filter(|level| !held || *level == GrantLevel::Chat)
        .filter(|level| locks.refuses_grant(&folder, (*level).into()).is_none())
        .collect();
    let says = format!("The sandbox refused a write in {}", block.target);
    if held {
        return Some(Shown {
            session,
            ask: block_key(session, block),
            says: said_inertly(&says),
            options: allows(&levels, true)
                .chain([refuses(KEEP, KEEP_SAID)])
                .collect(),
            source: AskSource::SandboxWrite,
            chain: Vec::new(),
            answer: AnswerPath::InItsPane,
            since: None,
            held_until: None,
            expires: None,
            blocks: None,
            timed_out: None,
            risk: None,
            writes: None,
            lines: None,
            last_message: None,
            login: None,
            fill: None,
            earlier_messages: None,
            rule: None,
            secret: None,
            run: None,
        });
    }
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
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
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
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
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
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
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
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
    })
}

/// **The harness-terminal adapter**: chat `session` is stopped on a prompt it asked mid-turn,
/// shown in its own terminal, and purlis holds nothing it could answer it by. Named, so it
/// does not wait unseen, with the kind of prompt its harness's nudge said (#1691); the person
/// answers it in the chat.
///
/// **A form purlis's own server asks through says its question** (#4710): `asking` is what the
/// server said it asks the person, held with the ask, masked. A form is the one prompt it asks
/// through, so another prompt keeps its kind. Its words are `says`, on one line, and in full in
/// `last_message`, as a question's are, so the strip, the Inbox card and the quiz show the ask as
/// asked; no later turn replaces them.
pub fn terminal(
    session: u32,
    prompt: purlis_core::harness::model::Prompt,
    asking: Option<String>,
) -> Shown {
    let asked = asking.filter(|_| prompt == purlis_core::harness::model::Prompt::Form);
    let says = match &asked {
        Some(question) => purlis_core::shown::one_line(
            &question.split_whitespace().collect::<Vec<_>>().join(" "),
            purlis_core::shown::NO_CLIP,
        ),
        None => prompt.says().to_owned(),
    };
    Shown {
        session,
        ask: format!("terminal:{session}"),
        says,
        options: Vec::new(),
        source: AskSource::Terminal,
        chain: Vec::new(),
        answer: AnswerPath::InItsPane,
        since: None,
        held_until: None,
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: asked,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
    }
}

/// **The question adapter**: chat `session` is in the needs-you queue with no prompt open,
/// its turn over and the next move the person's. Answered by replying in the chat. `says` is
/// its kind; what its chat said is `last_message`, where the app heard any (#4550).
pub fn question(session: u32, last_message: Option<String>, earlier: Vec<String>) -> Shown {
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
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message,
        login: None,
        fill: None,
        earlier_messages: (!earlier.is_empty()).then_some(earlier),
        rule: None,
        secret: None,
        run: None,
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
        says: purlis_core::shown::prose(&masked(&reached.text)),
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
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
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
                scope: None,
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
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
    }
}

/// The option id of a held merge's Merge.
pub const MERGE: &str = "merge";
/// The option id of a held merge's Don't merge.
pub const DONT_MERGE: &str = "dont-merge";

/// The key of chat `session`'s merge ask for `key`'s request at its head (Q242): a moved head
/// is another ask, so nothing the window held for the row at the old head carries over.
fn merge_key(session: u32, key: &purlis_core::mergehold::MergeKey) -> String {
    format!(
        "merge:{session}:{}#{}@{}",
        key.repo.as_str(),
        key.number,
        key.head.as_str()
    )
}

/// **The merge hold adapter** (R-114): a merge chat `waits.session`'s hook held, as one ask.
/// It offers Merge, which the app performs itself, and Don't merge (R-303's word). Every word is
/// purlis's; the repo, the number and the head are the key the app raised it at. Never offered
/// a rule: a merge is never remembered (R-117).
///
/// **High-risk** (Q239): never grouped, passed by keyboard triage, and its Merge asks once more
/// before it acts ([`purlis_core::askrisk::merge_of`]).
pub fn merge_hold(waits: &purlis_core::mergehold::Waits) -> Shown {
    let key = &waits.key;
    Shown {
        session: waits.session,
        ask: merge_key(waits.session, key),
        says: said_inertly(&format!(
            "Merge request {} of {} at {}? There is no undo.",
            key.number,
            key.repo.as_str(),
            &key.head.as_str()[..12]
        )),
        options: vec![
            Offered {
                id: MERGE.to_owned(),
                label: "Merge".to_owned(),
                allows: true,
                preselected: None,
                scope: None,
            },
            refuses(DONT_MERGE, "Don't merge"),
        ],
        source: AskSource::MergeHold,
        chain: Vec::new(),
        answer: AnswerPath::MergeHold {
            repo: key.repo.as_str().to_owned(),
            number: u32::try_from(key.number).unwrap_or(u32::MAX),
            head: key.head.as_str().to_owned(),
            facts: MergeShown::of(
                &waits.facts,
                key.base.as_str(),
                waits.look.clone(),
                waits.pushed_here,
            ),
        },
        // Since when is the registry's to say, as for every ask (`Waiting::since`).
        since: None,
        held_until: None,
        expires: None,
        blocks: None,
        timed_out: None,
        risk: Some(
            purlis_core::askrisk::merge_of(
                key.repo.as_str(),
                key.number,
                key.head.as_str(),
                key.base.as_str(),
            )
            .into(),
        ),
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
    }
}

/// The option id of a window-only refusal's one control: the press that clears it.
pub const PRESS: &str = "press";

/// The key of chat `session`'s ask for a window-only refusal whose way out is `control`.
fn window_only_key(session: u32, control: &purlis_core::windowonly::Control) -> String {
    match control {
        purlis_core::windowonly::Control::TrustClone { folder, place } => {
            format!("trust-clone:{session}:{folder}|{place}")
        }
    }
}

/// **The window-only refusal adapter** (R-328): a refusal whose only way out is a press in the
/// window, as one ask with that control on its row. Every word is purlis's; the folder and the
/// address are the ones the refusal was raised at (R-330).
pub fn window_only(refused: &purlis_core::windowonly::Refusal) -> Shown {
    let control = &refused.control;
    let answer = match control {
        purlis_core::windowonly::Control::TrustClone { folder, place } => AnswerPath::TrustClone {
            folder: folder.clone(),
            place: place.clone(),
        },
    };
    Shown {
        session: refused.session,
        ask: window_only_key(refused.session, control),
        says: said_inertly(&control.says()),
        options: vec![Offered {
            id: PRESS.to_owned(),
            label: control.label().to_owned(),
            allows: true,
            preselected: None,
            scope: None,
        }],
        source: AskSource::TrustClone,
        chain: Vec::new(),
        answer,
        // Since when is the registry's to say, as for every ask (`Waiting::since`).
        since: None,
        held_until: None,
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
    }
}

/// The option id of Trust these clones' Trust all.
pub const TRUST_ALL: &str = "trust-all";

/// The option id of Trust these clones' Choose… (R-337): the window draws a box for each clone
/// under the row, and trusts only those the person ticks, each with the clone Notice's own press.
pub const CHOOSE: &str = "choose";

/// The key of the project's Trust these clones: one per project, however many clones it lists.
const TRUST_CLONES_KEY: &str = "trust-clones";

/// **The adapter for the clones purlis has no record of** (R-336, R-337, Q184): the project's one
/// ask for all of them, no chat's, with Trust all and Choose…, each clone with its remote and when
/// it was made. Every word is purlis's; each folder and address is one the core read from the
/// clone, said inertly.
pub fn trust_these_clones(ask: &purlis_core::windowonly::TrustTheseClones) -> Shown {
    let named: Vec<String> = ask
        .clones
        .iter()
        .map(|clone| format!("{} ({})", clone.folder, clone.remote_said()))
        .collect();
    let them = if ask.clones.len() == 1 { "it" } else { "them" };
    Shown {
        session: 0,
        ask: TRUST_CLONES_KEY.to_owned(),
        says: said_inertly(&format!(
            "{}: purlis did not clone {}. Trust {them} only if you made {them} yourself.",
            purlis_core::windowonly::TRUST_THESE_CLONES,
            named.join(", ")
        )),
        options: vec![
            Offered {
                id: TRUST_ALL.to_owned(),
                label: "Trust all".to_owned(),
                allows: true,
                preselected: None,
                scope: None,
            },
            // It leads to a trust, so it waits for its row to settle as Trust all does.
            Offered {
                id: CHOOSE.to_owned(),
                label: "Choose…".to_owned(),
                allows: true,
                preselected: None,
                scope: None,
            },
        ],
        source: AskSource::TrustClones,
        chain: vec![said_inertly("purlis")],
        answer: AnswerPath::TrustClones {
            clones: ask
                .clones
                .iter()
                .cloned()
                .map(crate::clonevouch::CloneToVouch::of)
                .collect(),
        },
        // Since when is the registry's to say, as for every ask (`Waiting::since`).
        since: None,
        held_until: None,
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
    }
}

/// **Whether a chat waits on its window-only ask, as the board has it** (T1-42, R-329): in its
/// turn, or stopped on a prompt it asked mid-turn, it does; once its turn has ended on the person
/// it does not, and its ask sits on its finish row. A chat the board has no state for is taken to
/// wait, so its ask is never kept from Asks for want of a hook.
pub fn waits_on_its_ask(glance: &crate::host::Glance) -> bool {
    glance.state != purlis_core::state::State::Waiting
        || glance.in_its_turn()
        || glance.waits_on_its_prompt()
}

/// **`asks` split into the Blocking and those for a finish row** (T1-42, R-329, Q181): a
/// window-only ask (Trust this clone) whose chat does not wait on it, by `waits`, moves to the
/// second list; every other ask stays, in its order.
pub fn finish_rows_apart(
    asks: Vec<Shown>,
    waits: &dyn Fn(u32) -> bool,
) -> (Vec<Shown>, Vec<Shown>) {
    asks.into_iter()
        .partition(|ask| ask.source != AskSource::TrustClone || waits(ask.session))
}

/// **The project's Trust these clones, less every clone a chat's own Trust this clone asks for**
/// (Q257): one ask per clone. `None` where no clone is left to ask for.
fn not_asked_by_a_chat(
    unrecorded: Option<&purlis_core::windowonly::TrustTheseClones>,
    window_only: &[purlis_core::windowonly::Refusal],
) -> Option<purlis_core::windowonly::TrustTheseClones> {
    let mut ask = unrecorded?.clone();
    ask.clones.retain(|clone| {
        !window_only.iter().any(|one| {
            let purlis_core::windowonly::Control::TrustClone { folder, .. } = &one.control;
            *folder == clone.folder
        })
    });
    (!ask.clones.is_empty()).then_some(ask)
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
        scope: None,
    };
    // The declared reach's answer for this chat alone is marked, in place.
    let marked = |reach: Browser, said: &str| match (persona, declared) {
        (Some(persona), Some(declared)) if declared == reach => Offered {
            preselected: Some(true),
            scope: None,
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
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
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
                scope: None,
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
                scope: None,
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
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
    }
}

/// The key of the secret fill ask numbered `id` (#5115).
fn secret_fill_key(id: u32) -> String {
    format!("secret-fill:{id}")
}

/// **The secret fill adapter** (#5115, #5116): one notice naming the key, its vault and the exact
/// site, with the sandbox's own answers. Allow once and Allow for this chat, each unless policy
/// leaves its scope out; Always allow where its line can be written; Keep blocked. A card's key
/// is offered Allow once and Keep blocked alone (SF-4), and so is a multi-line text box or an
/// editable area, which asks each time (SF-5, #5120).
pub fn secret_fill(
    wait: &crate::secretfills::FillWait,
    locks: &purlis_core::sandbox::policy::Locks,
) -> Shown {
    use purlis_core::sandbox::grant::Level;
    let allow = |level: Level, label: &str| Offered {
        id: level.word().to_owned(),
        label: label.to_owned(),
        allows: true,
        preselected: None,
        scope: None,
    };
    let mut options = Vec::new();
    if locks.scope_removed(Level::Once).is_none() {
        options.push(allow(Level::Once, "Allow once"));
    }
    if !wait.once_only() {
        if locks.scope_removed(Level::Chat).is_none() {
            options.push(allow(Level::Chat, "Allow for this chat"));
        }
        if wait.no_always.is_none() && wait.line.is_some() {
            options.push(allow(Level::You, "Always allow"));
        }
    }
    options.push(refuses(KEEP, KEEP_SAID));
    let site = wait
        .origin
        .strip_prefix("https://")
        .unwrap_or(&wait.origin)
        .to_owned();
    Shown {
        session: wait.session,
        ask: secret_fill_key(wait.id),
        says: said_inertly(&if wait.code {
            format!(
                "Wants to put a 2FA code from {} in vault {} into a field on {site}",
                wait.key, wait.vault
            )
        } else {
            format!(
                "Wants to put {} from vault {} into a field on {site}",
                wait.key, wait.vault
            )
        }),
        options,
        source: AskSource::SecretFill,
        chain: Vec::new(),
        answer: AnswerPath::SecretFill {
            id: wait.id,
            shown: wait.digest(),
        },
        since: None,
        held_until: None,
        expires: None,
        blocks: None,
        timed_out: None,
        // **A vault use** (#6178): its Allow puts a vault's secret into a page.
        risk: vault_use(&wait.key, Some(&wait.vault), &format!("a field on {site}")),
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
        fill: Some(FillShown {
            key: wait.key.clone(),
            vault: wait.vault.clone(),
            site,
            page: wait.page.clone(),
            label: purlis_core::shown::short(&wait.label),
            browser: wait.browser.clone(),
            warnings: wait.warnings.clone(),
            // A field the page marks for payment is said as a card's (the decision of 2026-11-06).
            card: wait.card || wait.payment,
            multi_line: wait.multi_line,
            line: (!wait.once_only()).then(|| wait.line.clone()).flatten(),
            under: wait
                .persona
                .as_ref()
                .map(|persona| format!("[sandbox.personas.{persona}] tools")),
            no_always: (!wait.once_only())
                .then(|| wait.no_always.clone())
                .flatten(),
            vault_too: wait.vault_too,
            code: wait.code,
        }),
    }
}

/// The key of the login ask numbered `id` (#4595).
fn browser_login_key(id: u32) -> String {
    format!("browser-login:{id}")
}

/// **The browser login adapter** (#4595): one notice naming what a chat suggests, with the
/// sandbox's own answers. Allow once and Allow for this chat, each unless policy leaves its scope
/// out; Always allow; Keep blocked.
pub fn browser_login(
    wait: &crate::loginasks::LoginWait,
    locks: &purlis_core::sandbox::policy::Locks,
) -> Shown {
    use purlis_core::sandbox::grant::Level;
    let proposal = &wait.proposal;
    let allow = |level: Level, label: &str| Offered {
        id: level.word().to_owned(),
        label: label.to_owned(),
        allows: true,
        preselected: None,
        scope: None,
    };
    let mut options = Vec::new();
    if locks.scope_removed(Level::Once).is_none() {
        options.push(allow(Level::Once, "Allow once"));
    }
    if locks.scope_removed(Level::Chat).is_none() {
        options.push(allow(Level::Chat, "Allow for this chat"));
    }
    // A replacement of a declared login's site says what Always and Keep do to it (#4996).
    let replaces = proposal.replaces.is_some();
    if wait.no_always.is_none() {
        options.push(allow(
            Level::You,
            if replaces {
                "Always replace"
            } else {
                "Always allow"
            },
        ));
    }
    options.push(refuses(
        KEEP,
        if replaces {
            "Keep the recorded site"
        } else {
            KEEP_SAID
        },
    ));
    let domain = proposal
        .origin
        .strip_prefix("https://")
        .unwrap_or(&proposal.origin)
        .to_owned();
    Shown {
        session: wait.session,
        ask: browser_login_key(wait.id),
        says: said_inertly(&if replaces {
            format!(
                "Wants login {} of {} to sign in on another site",
                proposal.login, proposal.persona
            )
        } else {
            format!(
                "Wants to log its browser in as {}, from vault {}",
                proposal.persona, proposal.vault
            )
        }),
        options,
        source: AskSource::BrowserLogin,
        chain: Vec::new(),
        answer: AnswerPath::BrowserLogin {
            id: wait.id,
            shown: proposal.digest(),
        },
        since: None,
        held_until: None,
        expires: None,
        blocks: None,
        timed_out: None,
        // **A vault use** (#6178): its Allow fills a vault's login on the site.
        risk: vault_use(
            &format!("login {}", proposal.login),
            Some(&proposal.vault),
            &domain,
        ),
        writes: None,
        lines: None,
        last_message: None,
        fill: None,
        login: Some(LoginShown {
            path: proposal.page[proposal.origin.len()..].to_owned(),
            form: proposal
                .form
                .strip_prefix("https://")
                .unwrap_or(&proposal.form)
                .to_owned(),
            registrable: purlis_core::loginask::registrable(purlis_core::loginask::host_of(
                &proposal.origin,
            ))
            .to_owned(),
            form_registrable: purlis_core::loginask::registrable(purlis_core::loginask::host_of(
                &proposal.form,
            ))
            .to_owned(),
            domain,
            vault: proposal.vault.clone(),
            user_key: proposal.user_key.clone(),
            key: proposal.key.clone(),
            warnings: proposal.warnings.iter().map(|one| one.said()).collect(),
            line: proposal.line(),
            under: format!("[sandbox.personas.{}] tools", proposal.persona),
            no_always: wait.no_always.clone(),
            vault_too: wait.vault_too,
            replaces: proposal.replaces.as_ref().map(|old| old.page.clone()),
            replaces_form: proposal.replaces.as_ref().map(|old| {
                old.form
                    .strip_prefix("https://")
                    .unwrap_or(&old.form)
                    .to_owned()
            }),
        }),
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
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
                scope: None,
            },
            refuses(KEEP, "Keep off"),
        ],
        source: AskSource::PoolLogin,
        chain: Vec::new(),
        answer: AnswerPath::PoolLogin,
        since: None,
        held_until: None,
        expires: None,
        blocks: None,
        timed_out: None,
        // **A vault use** (#6178): its Allow fills the login on the pool's runner.
        risk: vault_use(
            &format!("{persona}'s {login}"),
            None,
            &format!("the runner of pool {pool}"),
        ),
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
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
                scope: None,
            },
            refuses(KEEP, "Keep off"),
        ],
        source: AskSource::PoolTool,
        chain: Vec::new(),
        answer: AnswerPath::PoolTool { run: waits.run },
        since: None,
        held_until: None,
        expires: None,
        blocks: None,
        timed_out: None,
        // **A vault use** (#6178): its Allow sends the tool its values on the pool's runner.
        risk: vault_use(
            &format!("{persona}'s values"),
            Some(vault),
            &format!("{tool} on pool {pool}"),
        ),
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
    }
}

/// The id of Allow on a pool tool Ask (#2047).
pub const POOL_TOOL_ALLOW: &str = "allow";

/// The key of chat `session`'s refusal of `vault`, unique in its project (#5160).
fn vault_key(session: u32, vault: &str) -> String {
    format!("vault:{session}:{vault}")
}

/// The id of Allow on a refused vault's ask (#5160): `allow_refused_vault`, the one grant its
/// Notice makes, so no level is chosen.
pub const VAULT_ALLOW: &str = "allow";

/// **The refused vault adapter** (#1430, #5160): a vault chat `session` was refused because the
/// persona it runs as is not tagged for it, held until the person answers. Allow lets that
/// persona use the vault in this project on this machine, exactly as its Notice's Allow does,
/// and is not offered where an administrator's policy forbids it; then Keep blocked. The names
/// are the app's record and the registry's, and are data.
pub fn vault_refused(one: &crate::vaultroute::VaultRefused) -> Shown {
    let (persona, vault) = (&one.persona, &one.vault);
    let tagged = one.tagged_for.as_deref().map_or_else(
        || "tagged for no persona".to_owned(),
        |them| format!("tagged for {them}"),
    );
    // What its Notice says beside its answers, since the row has these words alone. What Allow
    // grants comes first after the reason: a line is cut at its width, and that must survive.
    // Where policy took Allow away, the dispatch the chat was told of is the way forward.
    let says = [
        Some(format!(
            "Runs as {persona}, and vault {vault} is {tagged}, so purlis did not open it"
        )),
        Some(one.locked.as_deref().map_or_else(
            || {
                format!(
                    "Allow lets every chat opened as {persona} use it in this project on this \
                     machine"
                )
            },
            |why| why.trim_end_matches('.').to_owned(),
        )),
        one.login
            .as_deref()
            .map(|login| format!("It's for the browser login {login}")),
        one.locked
            .as_ref()
            .and(one.dispatch_to.as_deref())
            .map(|them| format!("purlis told the chat how to dispatch to {them}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(". ");
    let allow = one.locked.is_none().then(|| Offered {
        id: VAULT_ALLOW.to_owned(),
        label: format!("Allow {} to use this vault", said_inertly(persona)),
        allows: true,
        preselected: None,
        scope: None,
    });
    Shown {
        session: one.session,
        ask: vault_key(one.session, vault),
        says: said_inertly(&says),
        options: allow
            .into_iter()
            .chain([refuses(KEEP, KEEP_SAID)])
            .collect(),
        source: AskSource::Vault,
        chain: Vec::new(),
        answer: AnswerPath::Vault {
            vault: vault.clone(),
        },
        since: None,
        held_until: None,
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        fill: None,
        run: None,
    }
}

/// The key of Secret ask `id` (#5068).
fn secret_key(id: u32) -> String {
    format!("secret:{id}")
}

/// The id of Fill on a Secret ask (#5068).
pub const SECRET_FILL: &str = "fill";

/// **The Secret ask adapter** (#5068, #5071, ADR 0109): a chat's brokered run found a key
/// missing from its persona's vault, and the command waits on the person. One ask per vault and
/// key, however many chats wait on it; it is drawn under the first to ask. Fill stores the value
/// typed in its masked field in the vault and lets every waiting command run; Not now tells them
/// it was declined. Every word is purlis's but the names, which are the registry's and are data.
/// `may_remember`: policy leaves the persona's definition for the person to write (#5240).
pub fn secret_ask(open: &purlis_core::secrets::ask::Open, may_remember: bool) -> Shown {
    let (key, vault, persona) = (&open.key, &open.vault, &open.persona);
    // A profile's credential a start misses names no tool, and no chat has started (#5074).
    let used_by = match open.tool.as_deref() {
        Some(tool) => format!(" for {tool}"),
        None if open.chats.is_empty() && open.starts > 0 => " to start a chat".to_owned(),
        None => String::new(),
    };
    let others = match open.waiting() {
        0 | 1 => String::new(),
        n => format!(". {n} chats wait on it"),
    };
    let undeclared = if open.declared.is_some() {
        ""
    } else {
        " (not declared by this persona)"
    };
    // What the persona's definition says it is for, quoted as the definition's (#5076).
    let declared_for = open
        .declared
        .as_deref()
        .filter(|purpose| !purpose.is_empty())
        .map_or_else(String::new, |purpose| {
            format!(". {persona} declares it for: “{purpose}”")
        });
    // The chat's own words, quoted as its and last: a long one is what the row clips, never
    // purlis's sentence. Whole in `SecretShown::purpose`.
    let what_for = open.purpose.as_deref().map_or_else(String::new, |purpose| {
        format!(". The chat says it is for: “{purpose}”")
    });
    use purlis_core::secrets::ask::Reason;
    let says = match &open.reason {
        // A chat on a runner that cannot receive a value was told it was declined (#5077).
        None => {
            if open.chats.is_empty() && open.starts == 0 && !open.cannot_receive.is_empty() {
                format!(
                    "Needs {key}{undeclared} from {persona}'s vault {vault}{used_by}. This chat's runner \
                 can't receive secrets: Fill stores it in that vault for chats on this \
                 machine{declared_for}{what_for}"
                )
            } else {
                let unreached = if open.cannot_receive.is_empty() {
                    ""
                } else {
                    ". A runner chat that asked can't receive it"
                };
                format!(
                    "Needs {key}{undeclared} from {persona}'s vault {vault}{used_by}. Fill stores it in \
                 that vault on this machine; the chat never sees \
                 it{others}{unreached}{declared_for}{what_for}"
                )
            }
        }
        // The chat's words are data: the line says whose they are (#5075).
        Some(reason @ Reason::Replace { reads, .. }) => {
            let goes = reads.as_ref().map_or_else(
                || "Fill replaces it in that vault on this machine".to_owned(),
                |to| {
                    format!(
                        "{key} reads {} in vault {}: a new value goes there, or pick another \
                         entry",
                        to.key, to.vault
                    )
                },
            );
            format!(
                "Replace {key} in {persona}'s vault {vault}{used_by}: {}. {goes}; the chat \
                 never sees it{others}{declared_for}{what_for}",
                reason.said(key)
            )
        }
        Some(reason @ Reason::BrokenLink { .. }) => format!(
            "Mend {key} in {persona}'s vault {vault}{used_by}: {}. Fill gives it a value, or \
             pick another entry; the chat never sees it{others}{declared_for}{what_for}",
            reason.said(key)
        ),
    };
    let entry = |to: &purlis_core::secrets::links::Target| SecretEntryShown {
        vault: to.vault.clone(),
        key: to.key.clone(),
    };
    let (replace, reads, gone) = match &open.reason {
        None => (None, None, None),
        Some(Reason::Replace { said, reads }) => {
            (Some(said.clone()), reads.as_ref().map(entry), None)
        }
        Some(Reason::BrokenLink { was }) => (None, None, Some(entry(was))),
    };
    let need = open.declared.is_none();
    let remember =
        (may_remember && !persona.is_empty() && (need || open.names_no_vault)).then(|| {
            SecretNeedShown {
                file: crate::proposals::definition_file(persona),
                need,
                purpose: open.purpose.clone().unwrap_or_default(),
                vault: open.names_no_vault.then(|| vault.clone()),
            }
        });
    Shown {
        session: open
            .chats
            .first()
            .or(open.cannot_receive.first())
            .copied()
            .unwrap_or_default(),
        ask: secret_key(open.id),
        says: said_inertly(&says),
        options: vec![
            Offered {
                id: SECRET_FILL.to_owned(),
                label: "Fill".to_owned(),
                allows: true,
                preselected: None,
                scope: None,
            },
            refuses(KEEP, "Not now"),
        ],
        source: AskSource::Secret,
        chain: Vec::new(),
        answer: AnswerPath::Secret { id: open.id },
        since: None,
        held_until: None,
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        earlier_messages: None,
        rule: None,
        fill: None,
        secret: Some(SecretShown {
            persona: persona.clone(),
            vault: vault.clone(),
            key: key.clone(),
            tool: open.tool.clone(),
            purpose: open.purpose.clone(),
            declared: open.declared.is_some(),
            declared_purpose: open.declared.clone(),
            waiting: u32::try_from(open.waiting()).unwrap_or(u32::MAX),
            no_allow: open.no_allow.clone(),
            replace,
            reads,
            gone,
            remember,
        }),
        run: None,
    }
}

/// The key of the ask to allow a program `id` (ADR 0112).
fn run_grant_key(id: u32) -> String {
    format!("run-grant:{id}")
}

/// The id of Allow once on a Run grant's ask (ADR 0112).
pub const RUN_ALLOW_ONCE: &str = "allow-once";
/// The Allow of an ask to allow a program that holds for the chat (#5241).
pub const RUN_ALLOW_CHAT: &str = "allow-chat";
/// The Allow of an ask to allow a program that declares it as the persona's tool (#5245).
pub const RUN_ALLOW_ALWAYS: &str = "allow-always";
/// The vault's own Allow on a run's notice with no program Allow (#5242).
pub const RUN_ALLOW_VAULT: &str = "allow-vault";
/// Fill on a run's notice of value fields alone (#5242).
pub const RUN_FILL: &str = "fill";

/// **The Run grant adapter** (ADR 0112, #5239): a chat's brokered run names a program its persona
/// does not declare. Every word of the line is purlis's, and every name the app's; the chat's
/// command is never in it, and is drawn apart ([`RunShown::command`]). Allow once comes first, the
/// narrowest and the one a keyboard reaches first.
pub fn run_grant(
    open: &purlis_core::secrets::ask::run::OpenRun,
    locks: &purlis_core::sandbox::policy::Locks,
    may_remember: bool,
    others_wait: &dyn Fn(&str) -> bool,
) -> Shown {
    use purlis_core::secrets::ask::run::Hosts;
    let bound = &open.bound;
    let values = |specs: &[String], in_a_file: bool| -> Vec<RunValueShown> {
        specs
            .iter()
            .filter_map(|spec| spec.split_once('='))
            .map(|(var, key)| RunValueShown {
                var: var.to_owned(),
                key: key.to_owned(),
                in_a_file,
            })
            .collect()
    };
    let (hosts, every_host) = match &open.hosts {
        Hosts::Listed(hosts) => (hosts.clone(), false),
        Hosts::Every => (Vec::new(), true),
    };
    use purlis_core::sandbox::grant::Level;
    // Wider than one command only for a program reviewed for any arguments (ADR 0112).
    let reviewed = bound.reviewed_for_any_arguments();
    // Only for a persona, and only what would be written whole.
    let always = (!open.persona.is_empty() && reviewed)
        .then(|| purlis_core::settings::always_allow::line_of(bound))
        .flatten()
        .filter(|_| locks.keeps_scope(Level::Project))
        .map(|line| AlwaysShown {
            line: line.text,
            by_env: line.by_env,
            others_wait: others_wait(&open.persona),
        });
    let locked = locks
        .refuses_run_grant(&open.persona, &bound.program_name())
        .is_some()
        || (open.not_its_vault && locks.forbids_vault_grants());
    use purlis_core::secrets::ask::run::Offer;
    let lacks = open
        .fills
        .iter()
        .map(|fill| fill.key.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let says = match &open.offer {
        Offer::Program if open.ahead => format!(
            "{} asks for {lacks} from vault {} to run {} with, in this chat's own sandbox. What \
             you fill is stored there whatever you allow",
            open.persona,
            bound.vault,
            bound.program.display(),
        ),
        Offer::Program if lacks.is_empty() => format!(
            "{} wants to run {} with {} from vault {}, in this chat's own sandbox",
            open.persona,
            bound.program.display(),
            open.keys().join(", "),
            bound.vault,
        ),
        Offer::Program => format!(
            "{} wants to run {} with {} from vault {}, in this chat's own sandbox. The vault lacks \
             {lacks}: what you fill is stored there whatever you allow",
            open.persona,
            bound.program.display(),
            open.keys().join(", "),
            bound.vault,
        ),
        Offer::Vault { .. } => format!(
            "{} runs {} with {} from vault {}, which it may not use, and the vault lacks {lacks}. \
             Allow lets every chat opened as {} use it in this project on this machine",
            open.persona,
            bound.program.display(),
            open.keys().join(", "),
            bound.vault,
            open.persona,
        ),
        Offer::Fill { .. } => format!(
            "{} runs {} with {} from vault {}, which lacks {lacks}. Fill stores it in that vault on \
             this machine; the chat never sees it",
            open.persona,
            bound.program.display(),
            open.keys().join(", "),
            bound.vault,
        ),
    };
    let options: Vec<Offered> = match &open.offer {
        // Ahead of its run (#5244): Allow for this chat alone, with no command to allow once.
        Offer::Program if open.ahead => program_options(locks, reviewed, false, locked)
            .into_iter()
            .filter(|one| one.id != RUN_ALLOW_ONCE)
            .collect(),
        Offer::Program => program_options(locks, reviewed, always.is_some(), locked),
        Offer::Vault { .. } => (!locks.forbids_vault_grants())
            .then(|| Offered {
                id: RUN_ALLOW_VAULT.to_owned(),
                label: format!("Allow {} to use this vault", said_inertly(&open.persona)),
                allows: true,
                preselected: None,
                scope: None,
            })
            .into_iter()
            .chain([refuses(KEEP, KEEP_SAID)])
            .collect(),
        Offer::Fill { .. } => vec![
            Offered {
                id: RUN_FILL.to_owned(),
                label: "Fill".to_owned(),
                allows: true,
                preselected: None,
                scope: None,
            },
            refuses(KEEP, "Not now"),
        ],
    };
    let fills = open
        .fills
        .iter()
        .map(|fill| RunFillShown {
            key: fill.key.clone(),
            declared: fill.declared.clone().filter(|purpose| !purpose.is_empty()),
            need: fill.declared.is_none(),
            gone: match &fill.reason {
                Some(purlis_core::secrets::ask::Reason::BrokenLink { was }) => {
                    Some(SecretEntryShown {
                        vault: was.vault.clone(),
                        key: was.key.clone(),
                    })
                }
                _ => None,
            },
        })
        .collect();
    Shown {
        session: bound.chat,
        ask: run_grant_key(open.id),
        says: said_inertly(&says),
        options,
        source: AskSource::RunGrant,
        chain: Vec::new(),
        answer: AnswerPath::RunGrant { id: open.id },
        since: None,
        held_until: None,
        expires: None,
        blocks: None,
        timed_out: None,
        // **A vault use** (#6178): a program Allow hands the vault's values to the chat's run.
        // A Fill stores a value, and a vault's own Allow tags it: neither hands one over.
        risk: (open.offer == Offer::Program)
            .then(|| {
                vault_use(
                    &open.keys().join(", "),
                    Some(&bound.vault),
                    &bound.program.display().to_string(),
                )
            })
            .flatten(),
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        earlier_messages: None,
        rule: None,
        fill: None,
        secret: None,
        run: Some(RunShown {
            persona: open.persona.clone(),
            vault: bound.vault.clone(),
            program: bound.program.display().to_string(),
            values: values(&bound.file, true)
                .into_iter()
                .chain(values(&bound.env, false))
                .collect(),
            hosts,
            every_host,
            caution: open.caution.clone(),
            not_its_vault: open.not_its_vault,
            command: open.command.clone(),
            cwd: purlis_core::shown::ascii_word(&bound.cwd.to_string_lossy()),
            always: always.filter(|_| open.offer == Offer::Program && !open.ahead),
            fills,
            offer: match &open.offer {
                Offer::Program => "program",
                Offer::Vault { .. } => "vault",
                Offer::Fill { .. } => "fill",
            }
            .to_owned(),
            no_allow: open.offer.no_allow().map(str::to_owned),
            ahead: open.ahead,
            remember: (may_remember
                && !open.persona.is_empty()
                && open.fills.iter().any(|fill| fill.declared.is_none()))
            .then(|| SecretNeedShown {
                file: crate::proposals::definition_file(&open.persona),
                need: true,
                purpose: String::new(),
                vault: None,
            }),
        }),
    }
}

/// **A run's notice's program Allows** (ADR 0112): Allow once first, the narrowest and the one a
/// keyboard reaches first; wider only for a reviewed program; and Don't allow.
fn program_options(
    locks: &purlis_core::sandbox::policy::Locks,
    reviewed: bool,
    always: bool,
    locked: bool,
) -> Vec<Offered> {
    use purlis_core::sandbox::grant::Level;
    [
        (RUN_ALLOW_ONCE, "Allow once", Level::Once, true),
        (RUN_ALLOW_CHAT, "Allow for this chat", Level::Chat, reviewed),
        (RUN_ALLOW_ALWAYS, "Always allow", Level::Project, always),
    ]
    .into_iter()
    // Policy's `allow-scopes` (#1666) holds for these as for every Allow.
    // And a lock that came since it was raised leaves only Don't allow.
    .filter(|(_, _, level, offered)| *offered && locks.keeps_scope(*level) && !locked)
    .map(|(id, label, _, _)| Offered {
        id: id.to_owned(),
        label: label.to_owned(),
        allows: true,
        preselected: None,
        // Allow for this chat is a Run grant on Settings' Granted list, which Rules I made
        // undoes. Always declares the program a tool of the persona's for this project, whose
        // line Rules I made takes out again (#6177).
        scope: match id {
            RUN_ALLOW_CHAT => Some(RuleScope::Chat),
            RUN_ALLOW_ALWAYS => Some(RuleScope::Project),
            _ => None,
        },
    })
    .chain(std::iter::once(refuses(KEEP, "Don't allow")))
    .collect()
}

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
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
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
        scope: None,
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
/// **Dismiss** always. Never "always apply" (PC-13). One purlis holds is drawn as purlis's, with
/// the chat and the ask behind it by `name_of`, never as a chat's (#5240).
pub fn proposed_change(
    one: &crate::proposals::Proposal,
    now: &crate::proposals::Now,
    name_of: &dyn Fn(u32) -> Option<String>,
) -> Shown {
    use crate::proposals::Now;
    // A proposed pin (#5963): Review… opens Pin to workspace, which alone pins it; never Apply.
    if one.pin.is_some() {
        let mut options = Vec::new();
        if matches!(now, Now::Made { .. }) {
            options.push(refuses(crate::pinproposals::REVIEW, "Review…"));
        }
        options.push(refuses(DISMISS, "Dismiss"));
        return Shown {
            session: one.session,
            ask: proposal_key(one.id),
            says: said_inertly(&crate::pinproposals::says(one, now)),
            options,
            source: AskSource::ProposedChange,
            chain: Vec::new(),
            answer: AnswerPath::ProposedChange {
                id: one.id,
                path: one.path.clone(),
            },
            since: None,
            held_until: None,
            writes: None,
            lines: None,
            last_message: None,
            login: None,
            fill: None,
            earlier_messages: None,
            secret: None,
            run: None,
            expires: None,
            blocks: None,
            timed_out: None,
            risk: None,
            rule: None,
        };
    }
    let path = &one.path;
    let mut opens = false;
    let (says, writes, applies) = match now {
        Now::Made { before, after } => {
            let writes = crate::askwrites::proposed(path, before, after);
            let whole = matches!(writes, crate::askwrites::WritesShown::Whole { .. });
            opens = crate::askwrites::proposed_too_long(before, after);
            let says = if one.skills.is_some() {
                format!("Proposes a change to skills, which only you apply: {path}")
            } else {
                format!("Proposes a change to {path}, which only you apply")
            };
            (says, Some(writes), whole)
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
            scope: None,
        });
    }
    if opens {
        options.push(refuses(OPEN, "Open"));
    }
    options.push(refuses(DISMISS, "Dismiss"));
    let mut chain = Vec::new();
    let mut says = says;
    if one.session == crate::proposals::BY_PURLIS {
        chain.push("purlis".to_owned());
        if let Some(behind) = &one.behind {
            chain.extend(behind.chat.map(|chat| {
                name_of(chat).map_or_else(
                    || format!("chat {chat}"),
                    |name| purlis_core::dispatchgrant::inert(&name),
                )
            }));
            says = format!(
                "{says}. You ticked Remember for {} on Fill, and purlis could not write it then",
                behind.key
            );
        }
    }
    Shown {
        session: one.session,
        ask: proposal_key(one.id),
        says: said_inertly(&says),
        options,
        source: AskSource::ProposedChange,
        chain,
        answer: AnswerPath::ProposedChange {
            id: one.id,
            path: one.path.clone(),
        },
        since: None,
        held_until: None,
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
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
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
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
            scope: None,
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
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
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
            scope: None,
        }]
    } else {
        plans
            .iter()
            .map(|plan| Offered {
                id: format!("{SIGN_IN}:{}", plan.provider),
                label: format!("Sign in to {}", plan.title),
                allows: true,
                preselected: None,
                scope: None,
            })
            .collect()
    };
    options.push(refuses(DISMISS, "Dismiss"));
    Shown {
        session: 0,
        ask: format!("sign-in:{name}"),
        says: said_inertly(&format!(
            "{name} is not signed in on {}: a chat on it does not start until it is. Sign in \
             runs {}'s own sign-in in a shell",
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
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
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
            scope: None,
        }]
    } else {
        plans
            .iter()
            .map(|plan| Offered {
                id: format!("{SIGN_IN}:{}", plan.provider),
                label: format!("Sign in to {} on pool {pool}", plan.title),
                allows: true,
                preselected: None,
                scope: None,
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
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
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

/// Every project `planes` holds is told its asks may have moved: a sandbox-only profile was made
/// outside its Ask (#2037), which clears any project's wait for one of its harness.
pub fn every_projects_asks_may_have_moved(planes: &crate::planes::Planes) {
    for plane in planes.open_now() {
        if let Ok(held) = planes.held(&plane) {
            held.chats().asks_may_have_moved();
        }
    }
}

/// The option id of Make one on the Ask for a sandbox-only profile (#2037).
pub const MAKE_SANDBOX_ONLY: &str = "make";

/// The key of the Ask for a sandbox-only profile of `harness`: `sandbox-only:codex`.
pub fn sandbox_only_key(harness: purlis_core::harness::Harness) -> String {
    format!("sandbox-only:{}", harness.name())
}

/// **The sandbox-only adapter** (#2037, the operator's ruling of 2026-10-15; Claude Code on
/// Linux, ADR 0067 as amended 2026-11-10): a project where a sandboxed chat was refused for want
/// of a sandbox-only profile of its harness, as one Ask about no chat. It says, in plain words
/// and within a summary's width, what that profile is and the three steps to one, and offers
/// Make one, which makes the profile and opens the harness's own sign-in in it, and Dismiss.
pub fn sandbox_only(wait: &purlis_core::profilestore::sandbox_only::Wait) -> Shown {
    use purlis_core::harness::Harness;
    use purlis_core::profilestore::sandbox_only::{ask, called};
    let called = called(wait.harness);
    let here = if wait.harness == Harness::ClaudeCode {
        "on Linux "
    } else {
        ""
    };
    Shown {
        session: 0,
        ask: sandbox_only_key(wait.harness),
        says: said_inertly(&format!(
            "{ask}: {here}a sandboxed chat keeps its own sign-in, apart from yours. Make one, \
             sign in in the shell it opens, then start a new chat on it.",
            ask = ask(wait.harness)
        )),
        options: vec![
            Offered {
                id: MAKE_SANDBOX_ONLY.to_owned(),
                label: "Make one".to_owned(),
                allows: true,
                preselected: None,
                scope: None,
            },
            refuses(DISMISS, "Dismiss"),
        ],
        source: AskSource::SandboxOnly,
        chain: vec![said_inertly(called)],
        answer: AnswerPath::SandboxOnly {
            harness: wait.harness.name().to_owned(),
        },
        since: None,
        held_until: None,
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
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
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
    }
}

/// The option ids of the Ask about lanes waiting on signing (#5633). Where lanes wait on more
/// than one forge, setup is `set-up:<kind>:<host>`, one for each (#5790).
pub const SET_UP_SIGNING: &str = "set-up";
pub const SIGN_AND_RETRY_ALL: &str = "retry-all";

/// **The lanes waiting on signing, as one Ask** (#5633): every branch folder a forge refused
/// for unsigned commits, listed by name, about no chat. Before signing is set up it offers Set
/// up signing; once on, Sign and retry all N; and Dismiss, which drops them all. Lanes on more
/// than one forge get Set up signing on each forge, in both states (#5790). Once every one
/// was signed and sent it says so, with Dismiss. A lane that failed again stays listed, with its
/// reason, and a lane whose folder has gone is dropped. Nothing where nothing waits.
pub fn signing_waiting(root: &std::path::Path, on: impl FnOnce() -> bool) -> Option<Shown> {
    let waiting = purlis_core::signingsetup::retry::waiting_here(root);
    if waiting.is_empty() {
        return None;
    }
    // Whether this Mac has a key is asked only where a lane waits: the keychain and the machine
    // store are no business of a project with nothing waiting on signing.
    let on = on();
    let still: Vec<_> = waiting.iter().filter(|one| !one.sent).collect();
    let counted = |n: usize| {
        if n == 1 {
            "1 lane".to_owned()
        } else {
            format!("{n} lanes")
        }
    };
    let (says, options) = if still.is_empty() {
        (
            format!("{} signed and sent ✓", counted(waiting.len())),
            vec![refuses(DISMISS, "Dismiss")],
        )
    } else {
        let mut repos: Vec<&str> = still.iter().map(|one| one.path.as_str()).collect();
        repos.sort_unstable();
        repos.dedup();
        let forges: Vec<&str> = {
            let mut forges: Vec<&str> = still.iter().map(|one| one.kind.display()).collect();
            forges.sort_unstable();
            forges.dedup();
            forges
        };
        let lines: Vec<String> = still
            .iter()
            .map(|one| match &one.why {
                Some(why) => format!("- {} · {} ({}): {why}", one.repo, one.piece, one.branch),
                None => format!("- {} · {} ({})", one.repo, one.piece, one.branch),
            })
            .collect();
        let head = format!(
            "{} can't merge: {} only accepts signed commits{}",
            counted(still.len()),
            repos.join(", "),
            if forges.len() == 1 && repos.len() > 1 {
                format!(" on {}", forges[0])
            } else {
                String::new()
            },
        );
        let retry_all = refuses(
            SIGN_AND_RETRY_ALL,
            &if still.len() == 1 {
                "Sign and retry".to_owned()
            } else {
                format!("Sign and retry all {}", still.len())
            },
        );
        // Each forge the lanes wait on, by kind and host (#5790): where there is more than one,
        // each is set up on its own, before signing is on and after it, since the key may be
        // on one forge's account and not yet on the other's.
        let mut on_forges: Vec<(&str, &str, &str)> = still
            .iter()
            .map(|one| (one.kind.display(), one.kind.word(), one.host.as_str()))
            .collect();
        on_forges.sort_unstable();
        on_forges.dedup();
        let mut options = Vec::new();
        if on_forges.len() > 1 {
            if on {
                options.push(retry_all);
            }
            for (shown, word, host) in &on_forges {
                let same_kind = on_forges.iter().filter(|(_, w, _)| w == word).count();
                let label = if same_kind > 1 {
                    format!("Set up signing on {shown} ({host})")
                } else {
                    format!("Set up signing on {shown}")
                };
                options.push(refuses(&format!("{SET_UP_SIGNING}:{word}:{host}"), &label));
            }
        } else if on {
            options.push(retry_all);
        } else {
            options.push(refuses(SET_UP_SIGNING, "Set up signing"));
        }
        options.push(refuses(DISMISS, "Dismiss"));
        (format!("{head}\n{}", lines.join("\n")), options)
    };
    Some(Shown {
        session: 0,
        ask: "signing-waiting".to_owned(),
        says: said_inertly(&says),
        options,
        source: AskSource::Signing,
        chain: vec![said_inertly("purlis")],
        answer: AnswerPath::Signing,
        since: None,
        held_until: None,
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
    })
}

/// The option id of Open Pool setup on the Ask that a cluster is ready (#4496).
pub const OPEN_POOL_SETUP: &str = "open";

/// **Pool setup's cluster is ready, as one Ask** (#4496, story 45): the person sent the words to
/// their admin, and the background recheck found the cluster no longer stops the setup. Listed
/// in the project the setup was asked from (in every one where none was named), about no chat,
/// until it is answered: Open Pool setup, or Dismiss. Nothing where nothing is ready.
#[cfg(unix)]
pub fn pool_setup_ready(root: &std::path::Path, config: &std::path::Path) -> Option<Shown> {
    let choices = purlis_core::poolsetup::read_choices(config).ok()?;
    let asked = purlis_core::poolsetup::admin::ready(&choices)?;
    if let Some(project) = &asked.project {
        let same = project == root
            || matches!(
                (std::fs::canonicalize(project), std::fs::canonicalize(root)),
                (Ok(one), Ok(other)) if one == other
            );
        if !same {
            return None;
        }
    }
    Some(Shown {
        session: 0,
        ask: "pool-setup-ready".to_owned(),
        says: said_inertly(&format!(
            "Your cluster is ready for Pool setup: what your admin was asked for is there, on \
             context {}. Pool setup goes on where you stopped.",
            asked.context
        )),
        options: vec![
            refuses(OPEN_POOL_SETUP, "Open Pool setup"),
            refuses(DISMISS, "Dismiss"),
        ],
        source: AskSource::PoolSetup,
        chain: vec![said_inertly("purlis")],
        answer: AnswerPath::PoolSetup,
        since: None,
        held_until: None,
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
    })
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
    /// The programs chats' brokered runs ask the person to allow (ADR 0112).
    pub run_grants: Vec<purlis_core::secrets::ask::run::OpenRun>,
    /// The permission asks its chats' hooks hold open (HP-6).
    pub permissions: Vec<Raised>,
    /// The needs-you queue: the chats the board says need the person.
    pub queue: Vec<u32>,
    /// The prompt a chat is stopped on in its terminal, asked mid-turn, where it is
    /// (`Glance::prompt`, #1691).
    pub at_its_prompt: &'a dyn Fn(u32) -> Option<purlis_core::harness::model::Prompt>,
    /// Whether a chat waits in its terminal on a prompt its permission hook gave up on (R-204,
    /// `purlis_core::state::Board::waits_in_its_terminal`): listed as its own ask, even beside
    /// the asks its hook still holds, since none of those answers that prompt.
    pub in_its_terminal: &'a dyn Fn(u32) -> bool,
    /// The dispatches held for the person, as their Notices are told them.
    pub dispatches: Vec<crate::dispatchgrants::DispatchPending>,
    /// The blocks each chat is held on now.
    pub blocks: Vec<(u32, crate::taskblocks::HeldBlock)>,
    /// The workspaces a chat asked the person to remove, with what each holds (qw356).
    pub removals: Vec<(crate::removalasks::Waits, purlis_core::removalask::Holds)>,
    /// The merges its chats' hooks held, each waiting on the person's press (R-114).
    pub merges: Vec<purlis_core::mergehold::Waits>,
    /// The refusals whose only way out is a press in the window, each an ask (R-328).
    pub window_only: Vec<purlis_core::windowonly::Refusal>,
    /// The project's one ask for its clones purlis has no record of, where it has one (R-336).
    pub unrecorded: Option<purlis_core::windowonly::TrustTheseClones>,
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
    /// The harnesses whose sandboxed chats here wait for a sandbox-only profile (#2037).
    pub sandbox_only: Vec<purlis_core::profilestore::sandbox_only::Wait>,
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
    /// The logins chats suggested that wait on the person (#4595).
    pub browser_logins: Vec<crate::loginasks::LoginWait>,
    /// The Secret asks chats' brokered runs wait on (#5068).
    pub secrets: Vec<purlis_core::secrets::ask::Open>,
    /// Whether Remember is offered for this persona (#5240, [`crate::proposals::need_offerable`]).
    pub may_remember: &'a dyn Fn(&str) -> bool,
    /// Whether anything else of this persona's tools waits on the person's Allow, where Always
    /// allow is offered (#5080, [`purlis_core::settings::always_allow::others_wait`]).
    pub others_wait: &'a dyn Fn(&str) -> bool,
    /// The secret fills chats asked for that wait on the person (#5115).
    pub secret_fills: Vec<crate::secretfills::FillWait>,
    /// The vaults chats were refused for their persona, as their Notice shows them (#5160).
    pub vaults: Vec<crate::vaultroute::VaultRefused>,
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
    /// What a chat's turn last ended on, as kept (#4550): what a question shows.
    pub last_message: &'a dyn Fn(u32) -> Option<String>,
    /// What its turns ended on before that, oldest first (#4563).
    pub earlier_messages: &'a dyn Fn(u32) -> Vec<String>,
    /// What purlis's MCP server said a chat asks the person in its harness's form (#4710).
    pub asking: &'a dyn Fn(u32) -> Option<String>,
    /// Until when a chat's proxy holds a connection to a host, where it holds one now, asking
    /// the person (#1709), given when the ask began.
    pub held: &'a HoldEnds<'a>,
    /// When an ask began to wait, by its key, where its source knows (#1700).
    pub since: &'a dyn Fn(&str) -> Option<u32>,
    /// Until when an ask waits before its source ends it, by its key, where purlis knows
    /// (#6006). A held connection's hold is [`Self::held`]'s.
    pub expires: &'a dyn Fn(&str) -> Option<u32>,
    /// How many open chats wait on chat `session`'s report, where any does (#6006).
    pub chats_waiting: &'a dyn Fn(u32) -> Option<u32>,
    /// When an ask's wait ended with nobody's answer, by its key, where it is still answerable
    /// (#6007).
    pub timed_out: &'a dyn Fn(&str) -> Option<u32>,
    /// Whether the person is Quiet, so a held connection is held open past its minute and is no
    /// Expiring ask (ADR 0121 decision 13, #6007).
    pub held_open: bool,
    /// The name of the runner a chat runs on, where its runner left a service out of its plan
    /// (#2510).
    pub runner_of: &'a dyn Fn(u32) -> Option<String>,
    /// **What the project knows that an ask's risk is read against** (#6011).
    pub risk: &'a purlis_core::askrisk::Facts,
    /// **Where a push that names no branch lands** (#6178): for chat `session`, from the folder
    /// a `git -C` names in its own (`""` for none), as git there says.
    pub pushes: &'a dyn Fn(u32, &str) -> Option<purlis_core::askrisk::Pushes>,
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
        .filter_map(|raised| {
            let session = raised.chat.parse().unwrap_or(0);
            let pushes = |folder: &str| (waiting.pushes)(session, folder);
            let risk = risk_of_action(&raised.ask, waiting.risk, &pushes);
            permission(raised).map(|shown| Shown { risk, ..shown })
        })
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
    // A chat whose merge was held waits on the person's press, never its own (R-114).
    asks.extend(waiting.merges.iter().map(merge_hold));
    // A refusal whose only way out is the window's press is an ask with that control (R-328).
    asks.extend(waiting.window_only.iter().map(window_only));
    // The clones purlis has no record of are one ask, no chat's (R-336), and a clone a chat's
    // own Trust this clone asks for is not asked for twice (Q257).
    asks.extend(
        not_asked_by_a_chat(waiting.unrecorded.as_ref(), &waiting.window_only)
            .iter()
            .map(trust_these_clones),
    );
    // A chat that proposed a change waits on the person's answer, as one that asked them to
    // remove a workspace does (#1975).
    asks.extend(
        waiting
            .proposals
            .iter()
            .map(|(one, now)| proposed_change(one, now, waiting.name_of)),
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
    // A login a chat suggested (#4595).
    asks.extend(
        waiting
            .browser_logins
            .iter()
            .map(|wait| browser_login(wait, waiting.locks)),
    );
    // A secret a chat asks purlis to fill (#5115).
    asks.extend(
        waiting
            .secret_fills
            .iter()
            .map(|wait| secret_fill(wait, waiting.locks)),
    );
    // A chat whose brokered run names a program its persona does not declare (ADR 0112).
    asks.extend(waiting.run_grants.iter().map(|open| {
        run_grant(
            open,
            waiting.locks,
            (waiting.may_remember)(&open.persona),
            waiting.others_wait,
        )
    }));
    // A runner chat whose declared tool waits on the person's Allow for its pool (#2047).
    asks.extend(waiting.pool_tools.iter().map(pool_tool));
    // A chat whose brokered run missed a key waits on the person to fill it (#5068).
    asks.extend(
        waiting
            .secrets
            .iter()
            .map(|open| secret_ask(open, (waiting.may_remember)(&open.persona))),
    );
    // A permission ask that timed out says nothing of the chat's wait (#6007): its harness's own
    // prompt still waits in the pane, and only the pane answers it, so it is listed too.
    // A window-only refusal is a refusal, not the chat's wait (R-328, R-329): like a block, it
    // stands beside whatever the chat asks of its own, which stays an ask when it moves to the
    // chat's finish row.
    let said: std::collections::HashSet<u32> = asks
        .iter()
        .filter(|ask| ask.source != AskSource::TrustClone)
        .filter(|ask| {
            ask.source != AskSource::Permission || (waiting.timed_out)(&ask.ask).is_none()
        })
        .map(|ask| ask.session)
        .collect();
    // About no chat: a profile that waits to be signed in (#2035).
    let signing = asks.len();
    asks.extend(waiting.sign_ins.iter().map(sign_in));
    asks.extend(waiting.pool_sign_ins.iter().map(pool_sign_in));
    asks.extend(waiting.sandbox_only.iter().map(sandbox_only));
    asks.extend(
        waiting
            .queue
            .iter()
            .filter(|session| {
                (!said.contains(session) || (waiting.in_its_terminal)(**session))
                    && !(waiting.only_updates)(**session)
            })
            .map(|&session| match (waiting.at_its_prompt)(session) {
                Some(prompt) => terminal(session, prompt, (waiting.asking)(session)),
                None => question(
                    session,
                    (waiting.last_message)(session),
                    (waiting.earlier_messages)(session),
                ),
            }),
    );
    asks.extend(waiting.blocks.iter().filter_map(|(session, block)| {
        // A runner's service, named with its runner (#2510).
        let mut ask = sandbox_runner(*session, block, (waiting.runner_of)(*session).as_deref())
            .or_else(|| sandbox_block(*session, block, waiting.locks, waiting.declared))?;
        let since = (waiting.since)(&ask.ask);
        // **A host that looks like production, a folder outside the project** (#6011).
        ask.risk = match block.what {
            crate::sandboxing::GrantWhat::Host => waiting.risk.of_host(&block.target),
            crate::sandboxing::GrantWhat::Write => waiting.risk.of_write(&block.target),
            _ => None,
        }
        .map(RiskShown::from);
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
    // A vault a chat was refused (#5160): its command was refused, not held, so like a block
    // it stands beside whatever the chat asks of its own.
    asks.extend(waiting.vaults.iter().map(|one| {
        let mut ask = vault_refused(one);
        ask.since = (waiting.since)(&ask.ask);
        ask
    }));
    for (at, ask) in asks.iter_mut().enumerate() {
        // A profile's Ask is about no chat: its chain is the profile, as its adapter said.
        let profiles = signing
            ..signing
                + waiting.sign_ins.len()
                + waiting.pool_sign_ins.len()
                + waiting.sandbox_only.len();
        // Nor is a change purlis holds, whose adapter names purlis and what is behind it.
        if !profiles.contains(&at) && ask.chain.is_empty() {
            ask.chain = chain(ask.session, waiting.name_of, waiting.asker_of);
        }
        ask.since = (waiting.since)(&ask.ask);
        // **The facts Asks orders by** (ADR 0121 decision 5, #6006): when it ends by itself,
        // and how many chats wait on its chat. An ask of no chat's blocks none.
        // A held connection held open while the person is Quiet ends at no time of its own
        // (#6007): its hold is still said, so the window reads it again.
        ask.expires = match ask.held_until {
            Some(_) if waiting.held_open => None,
            Some(until) => Some(until),
            None => (waiting.expires)(&ask.ask),
        };
        // **Timed out, and answerable still** (ADR 0121 decision 15, #6007): never while it
        // is Expiring, as a connection held again is.
        ask.timed_out = if ask.expires.is_none() {
            (waiting.timed_out)(&ask.ask)
        } else {
            None
        };
        ask.blocks = if ask.session == 0 {
            None
        } else {
            (waiting.chats_waiting)(ask.session)
        };
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
/// and no ask's notification is sent (a task that failed is told by Should know, from its
/// record: `asknotify::Rules::ended`, #6005). A chat whose own turn
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

/// **Which open chat waits on which open chat's report** (#6006): each chat's own record of the
/// chat that dispatched it, where that chat is open and its report is still due.
fn waits_on(chats: &crate::chats::Chats) -> Vec<purlis_core::dispatchwaits::WaitsOn> {
    let open: std::collections::HashSet<u32> =
        chats.open_now().iter().map(|one| one.session).collect();
    open.iter()
        .filter_map(|&task| {
            let from = chats.recorded_chat(task)?.from?;
            (from.report == purlis_core::reopen::Owed::Due && open.contains(&from.chat)).then_some(
                purlis_core::dispatchwaits::WaitsOn {
                    task,
                    asker: from.chat,
                },
            )
        })
        .collect()
}

/// Every ask project `held` has waiting now, as the window lists them.
pub fn every_ask(held: &crate::planes::Held) -> Asking {
    // A backstop for the host's board (the review of #4905): told when a vault value is
    // remembered already, so this is nothing unless a value came since and no one was told.
    crate::dispatched::said_masked_kept(held);
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
    let mut permissions = held.hooks().asks().pending_since(now);
    // **When each ask's wait ended unanswered, by its key** (ADR 0121 decision 15, #6007),
    // where an answer still reaches the chat: a hook ask that lapsed, a run grant and a Secret
    // ask past their command's hold. A held connection that timed out is a block, not an ask
    // (R-290, Q154).
    let mut timed_out: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let ago = |at: Instant| now_secs.saturating_sub(now.saturating_duration_since(at).as_secs());
    for lapsed in held.hooks().asks().lapsed(now) {
        timed_out.insert(lapsed.raised.id.to_string(), ago(lapsed.ended));
        permissions.push((lapsed.raised, lapsed.at));
    }
    let held_blocks = chats.blocks().every_since();
    let waiting_dispatches = store.every_waiting();
    // When each ask began, by its key (#1700): a hook ask's raise, read back from how long ago
    // it was; a dispatch's hold; a block's first hearing.
    let mut since: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    // **When each ask ends by itself, by its key** (ADR 0121 decision 5 and its amendment of
    // 2026-11-10, #6006): a hook ask at its deadline, below the harness's hook timeout; a run
    // grant and a Secret ask as their command's hold runs out; a login and a fill as purlis's
    // own wait does. A held connection's hold is said by [`Shown::held_until`].
    let mut expires: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for (raised, at) in &permissions {
        let elapsed = now.saturating_duration_since(*at);
        since.insert(
            raised.id.to_string(),
            now_secs.saturating_sub(elapsed.as_secs()),
        );
        if let Some(deadline) = raised.ask.deadline.duration() {
            let left = deadline.saturating_sub(elapsed).as_secs();
            expires.insert(raised.id.to_string(), now_secs.saturating_add(left));
        }
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
    let merges = held.merge_holds().every();
    for one in &merges {
        since.insert(merge_key(one.session, &one.key), one.since);
    }
    let window_only = purlis_core::windowonly::every(root);
    for one in &window_only {
        since.insert(window_only_key(one.session, &one.control), one.since);
    }
    // A clone a chat's own Trust this clone asks for is not asked for again (Q257).
    let unrecorded = not_asked_by_a_chat(
        purlis_core::windowonly::clones_ask(root).as_ref(),
        &window_only,
    );
    if let Some(ask) = &unrecorded {
        since.insert(TRUST_CLONES_KEY.to_owned(), ask.since);
    }
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
    for one in &sandbox_only {
        since.insert(sandbox_only_key(one.harness), one.since);
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
    // A login's and a fill's wait, and a brokered command's hold on its chat's harness.
    let waits_for_the_person = purlis_core::loginask::THE_PERSON_IS_WAITED_FOR.as_secs();
    let command_holds =
        |chat: u32| purlis_core::secrets::ask::holds_for(chats.harness(chat)).as_secs();
    let browser_logins = held.login_grants().waiting();
    for one in &browser_logins {
        since.insert(browser_login_key(one.id), one.since);
        expires.insert(
            browser_login_key(one.id),
            one.since.saturating_add(waits_for_the_person),
        );
    }
    let secret_fills = held.secret_fills().waiting();
    for one in &secret_fills {
        since.insert(secret_fill_key(one.id), one.since);
        expires.insert(
            secret_fill_key(one.id),
            one.since.saturating_add(waits_for_the_person),
        );
    }
    let pool_tools = held.pool_tools().waiting();
    for one in &pool_tools {
        since.insert(pool_tool_key(one.session, one.run), one.since);
    }
    let run_grants = held.secret_asks().open_runs();
    for one in &run_grants {
        since.insert(run_grant_key(one.id), one.since);
        let ends = one.since.saturating_add(command_holds(one.bound.chat));
        expires.insert(run_grant_key(one.id), ends);
        // Its command gave up, and the core holds it open still (#6007).
        if ends <= now_secs {
            timed_out.insert(run_grant_key(one.id), ends);
        }
    }
    let secrets = held.secret_asks().open();
    let may_remember = |persona: &str| crate::proposals::need_offerable(held.root(), persona);
    // Once per persona however many of its runs wait: it reads the project's file.
    let waits_of = std::cell::RefCell::new(std::collections::HashMap::<String, bool>::new());
    let others_wait = |persona: &str| {
        *waits_of
            .borrow_mut()
            .entry(persona.to_owned())
            .or_insert_with(|| {
                purlis_core::settings::always_allow::others_wait(held.root(), persona)
            })
    };
    for one in &secrets {
        since.insert(secret_key(one.id), one.since);
        if let Some(chat) = one.chats.first() {
            let ends = one.since.saturating_add(command_holds(*chat));
            expires.insert(secret_key(one.id), ends);
            if ends <= now_secs {
                timed_out.insert(secret_key(one.id), ends);
            }
        }
    }
    let vaults: Vec<_> = held
        .vault_refusals()
        .every()
        .into_iter()
        .map(|(session, not, first)| {
            since.insert(vault_key(session, &not.vault), first);
            crate::vaultroute::shown(plane, session, not)
        })
        .collect();
    let declared = purlis_core::sandbox::services::declared(root);
    let tasks = held.tasks();
    let chats_waiting = purlis_core::dispatchwaits::blocked_by(&waits_on(chats));
    let risk = purlis_core::askrisk::Facts::read(root);
    let waiting = Waiting {
        run_grants,
        browsers,
        needs_you,
        browser_sites,
        pool_logins,
        pool_tools,
        browser_logins,
        secrets,
        may_remember: &may_remember,
        others_wait: &others_wait,
        secret_fills,
        vaults,
        sandbox_only,
        sign_ins,
        pool_sign_ins,
        removals,
        merges,
        window_only,
        unrecorded,
        proposals,
        runner_writes,
        runner_dispatches,
        permissions: permissions.into_iter().map(|(raised, _)| raised).collect(),
        queue,
        at_its_prompt: &|session| board.glance(session).prompt,
        in_its_terminal: &|session| held.hooks().board().waits_in_its_terminal(session),
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
                waiting: hooks.turn_ended_on_the_person(session),
                at_its_prompt: hooks.waits_on_its_prompt(session),
            })
        },
        last_message: &|session| tasks.ledger().last_said(session).map(str::to_owned),
        asking: &|session| tasks.ledger().asking(session).map(str::to_owned),
        earlier_messages: &|session| {
            tasks
                .ledger()
                .said_before(session)
                .into_iter()
                .map(str::to_owned)
                .collect()
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
        expires: &|ask| {
            expires
                .get(ask)
                .filter(|at| **at > now_secs)
                .map(|at| u32::try_from(*at).unwrap_or(u32::MAX))
        },
        chats_waiting: &|session| chats_waiting.get(&session).copied(),
        timed_out: &|ask| {
            timed_out
                .get(ask)
                .map(|at| u32::try_from(*at).unwrap_or(u32::MAX))
        },
        held_open: crate::asknotify::holds_asks_open(),
        runner_of: &|session| chats.runner_of(session),
        risk: &risk,
        pushes: &|session, folder| {
            purlis_core::askrisk::pushes_from(&chats.folder_of(session)?.join(folder))
        },
    };
    let task_questions: Vec<Shown> = crate::dispatched::reached_the_person(held)
        .into_iter()
        .map(|(reached, question)| {
            let chain = chain(reached.task, waiting.name_of, waiting.asker_of);
            task_question(reached, question.asked, chain)
        })
        .collect();
    let with_asking_chat = crate::dispatched::with_asking_chat(held)
        .into_iter()
        .filter(|one| !chats.kept_from_the_window(one.task))
        .map(|one| WithAskingChat {
            session: one.task,
            asker: (waiting.name_of)(one.asker).unwrap_or_default(),
            since: u32::try_from(one.since).unwrap_or(u32::MAX),
        })
        .collect();
    let asking = Asking {
        plane: plane.clone(),
        with_asking_chat,
        snoozed: Vec::new(),
        snoozed_until: None,
        cooling: Vec::new(),
        cooling_until: None,
        on_finish_rows: Vec::new(),
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
            // The lanes a forge refused for unsigned commits (#5633): one Ask for them all.
            .chain(signing_waiting(root, signing_on))
            // Pool setup's cluster, ready once the admin the person asked has given it (#4496).
            .chain(pool_setup_ready_here(root))
            // What the project expects and this machine has not allowed (#6000): a consent.
            .chain(consents::sandbox_expects(
                purlis_core::sandbox::services::awaiting(
                    root,
                    &purlis_core::sandbox::Machine::this(),
                )
                .map(Into::into)
                .as_ref(),
            ))
            // A persona whose hosts or tools wait for an Allow here: one ask each (ADR 0121,
            // amended 2026-11-12), read as Settings and its Notice read them.
            .chain({
                let (personas, hosts_locked) = crate::sandboxing::persona_hosts_here(root);
                consents::persona_allows(&personas, hosts_locked, held.persona_later(), &risk)
            })
            // A command's confirmation in the window, filed under this project (#6000).
            .chain(
                crate::poolsignin_confirm::waiting_in(plane)
                    .map(|(asked, until)| consents::confirmation(&asked, until)),
            )
            // A chat paused at its budget or as looping (#6015).
            .chain(
                held.pauses()
                    .now()
                    .into_iter()
                    .filter(|(session, _)| !chats.kept_from_the_window(*session))
                    .map(|(session, paused)| {
                        let chain = chain(session, waiting.name_of, waiting.asker_of);
                        let blocks = (waiting.chats_waiting)(session);
                        pauses::paused(session, &paused, chain, blocks)
                    }),
            )
            .collect(),
    };
    // A window-only ask no chat waits on sits on its chat's finish row, not in Asks (R-329).
    let (asks, on_finish_rows) = finish_rows_apart(asking.asks, &|session| {
        waits_on_its_ask(&board.glance(session))
    });
    Asking {
        asks,
        on_finish_rows,
        ..asking
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
    let Ok(block) = crate::taskblocks::shown_one(blocks, session, &shown) else {
        return Ok(false);
    };
    // An Allow racing this that let the block go first is the one answer (Q262 amended):
    // this writes nothing.
    if !blocks.answered(session, &block) {
        return Ok(false);
    }
    // R-229: the person's Keep blocked, in the chat's Chat log.
    crate::chatlog::answered_in_the_window(
        held.hooks(),
        session,
        purlis_core::eventlog::stops::AskGroup::Sandbox,
        false,
    );
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
    Ok(true)
}

mod consents;
pub use consents::Later as PersonaLater;

/// **Not now on a persona's Allow** (ADR 0121, amended 2026-11-12), on its row in Asks or on its
/// Notice: put away while project `plane` is open and `persona`'s list is the one shown,
/// `digest`. Nothing is kept, and nothing is granted; a changed list asks again. The window's
/// alone, as the Allow is.
#[tauri::command]
#[specta::specta]
pub async fn persona_allow_not_now(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    persona: String,
    digest: String,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    held.persona_later().say(&persona, &digest);
    Ok(())
}
pub mod pauses;

#[cfg(test)]
mod tests;

/// [`pool_setup_ready`], on this machine's config home, where it has one.
fn pool_setup_ready_here(root: &std::path::Path) -> Option<Shown> {
    #[cfg(unix)]
    {
        // Read only where a config home is there, and never past a test build's fence: every
        // project's Asks are listed through here, so a fenced run reads one outside it as none.
        let config = purlis_core::machine::config_root_read_in(&purlis_core::envvar::var)?;
        pool_setup_ready(root, &config)
    }
    #[cfg(not(unix))]
    {
        let _ = root;
        None
    }
}

/// Whether this Mac has a signing key, asked of the keychain's attributes only, never its value.
fn signing_on() -> bool {
    let Some(config) = purlis_core::machine::config_root() else {
        return false;
    };
    let keyring = purlis_core::commitsign::this_builds_keyring(&config);
    purlis_core::signingsetup::key_is_there(&config, keyring.as_ref()).unwrap_or(false)
}

#[cfg(test)]
mod rules_tests;
