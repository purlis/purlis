// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which of purlis's refusals run in a project** (rulings Q308 and Q308a, the first slice of
//! the project policy, map #6969, [ADR 0125]).
//!
//! purlis's refusals fall into fifteen **families** ([`Family`]). A project's committed settings
//! file says, under `[policy]`, how each runs ([`Mode`]):
//!
//! - **`off`**: the check does not run.
//! - **`log`**: the check runs and allows. Nothing is shown to the chat or the person (no
//!   Notice, no ask); one quiet line in the chat's enforcement log names the rule, the family,
//!   the chat and the host, and nothing else ([`record`]).
//! - **`enforce`**: the refusal, as purlis has always made it.
//!
//! ```toml
//! [policy]
//! template = "watch"          # open, watch, hold, team or strict; watch when absent
//! network = "enforce"         # any family, to override the template
//! guards.merge = "enforce"    # the guards.* families are dotted keys
//! ```
//!
//! # Who can loosen it
//!
//! **A chat can never switch its own enforcement off** (Q307 Q4, Q308a):
//!
//! - The setting is read from the **plane root's committed file** alone. The local file can only
//!   tighten it ([`Settings::read`]).
//! - It is **resolved once, at a chat's start** ([`at_start`]), and handed to the chat's hooks in
//!   [`ENV`], which the app sets after anything a profile set. The app's forge route is held to the
//!   same rule for `guards.merge` ([`crate::forge::mergestance`]): it reads the policy live, for
//!   the chat (never looser than its dispatch chain) and never looser than the chat started, so a
//!   tightening reaches an open chat at once and a loosening press with its next start.
//! - What a start resolves is never looser than what **this person last accepted**
//!   ([`crate::sandbox::local::policy_accepted`], in their config home, which no sandboxed chat
//!   writes). A tightening is accepted as the start reads it; a loosening waits for the person's
//!   press in the window ([`press`]). Before the first start the accepted is the shipped
//!   default, `watch`, which the upgrade announces once.
//! - **Only the app resolves it** ([`in_force`]): a hook, a git hook or `purlis secret exec`
//!   asks the app over the hook socket, and runs `strict` where the app does not answer.
//!   Nothing a chat puts in its own environment, and no folder it names, loosens anything.
//!
//! **A value purlis cannot read never loosens anything**: an unknown template is `strict`, an
//! unknown mode is `enforce` for its family, and each is said ([`Settings::refused`]) for the
//! window's Notice.
//!
//! **An administrator's lock wins** ([`crate::sandbox::policy::Locks::enforcement_floor`]): a
//! locked family runs at least at what the lock says.
//!
//! # Families not yet switchable
//!
//! [`Family::switchable`] is false for the families whose wiring waits on a later ruling: they
//! stay enforced whatever `[policy]` says, as before Q308.
//!
//! [ADR 0125]: ../../../docs/adr/0125-blocks-are-off-by-default-and-opt-in-per-family.md

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The table of the committed settings file that holds these keys.
pub const TABLE: &str = "policy";

/// The key naming the template.
pub const TEMPLATE_KEY: &str = "template";
/// The `[policy]` key for lineage, a parent chat taking up its ended child's cut branch through
/// the forge route: `"allow"` (the default, nothing set) or `"deny"`, the strict value.
pub const LINEAGE_KEY: &str = "lineage";

/// The variable a chat's hooks read what their chat started with from ([`Resolved::spelled`]).
/// Set by the app at the launch, after anything a profile set, and never inherited.
pub const ENV: &str = "PURLIS_POLICY";

/// **Where this machine's acceptance of a project's `[policy]` lives, as a refusal names it**
/// (ADR 0130 §7, §8): acceptance is per machine, so a chat started on another machine (a runner's
/// or a pool's, #7722) is told which machine's window accepts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptedOn<'a> {
    /// The machine this chat runs on.
    ThisMachine,
    /// Another machine, by the name the person knows it by.
    Machine(&'a str),
}

impl AcceptedOn<'_> {
    /// "on this machine", or "on <machine>".
    fn on(self) -> String {
        match self {
            Self::ThisMachine => "on this machine".to_owned(),
            Self::Machine(name) => format!("on {name}"),
        }
    }

    /// The window that accepts it: purlis's own, on that machine.
    fn window(self) -> String {
        match self {
            Self::ThisMachine => "purlis's window".to_owned(),
            Self::Machine(name) => format!("purlis's window on {name}"),
        }
    }
}

/// **Why a refusal an unaccepted policy causes was made, and the way out** (ADR 0130 §7): the
/// policy is not accepted where it lives (`on`), and only that window accepts it. Never "set the
/// family to `log`": loosening the family the policy holds changes nothing until it is accepted.
/// A clause to follow "because "; the caller says what was refused.
pub fn unaccepted_said(on: AcceptedOn<'_>) -> String {
    format!(
        "this project's policy isn't accepted {}. A person accepts it in {}; a chat cannot.",
        on.on(),
        on.window()
    )
}

/// One family of purlis's refusals (Q308, from the #6970 inventory).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family {
    /// The OS wrap around a chat. Set by the template through the existing `[sandbox]` mode and
    /// level, never by a key of `[policy]` (Q308a).
    Sandbox,
    /// The egress proxy's refusal of a host.
    Network,
    /// Local sockets, listening and connect ports.
    Ports,
    /// The wrap's refusal of a program (`launchctl`, `osascript`, `open`, cron, Apple Events).
    Processes,
    /// The guards on git: the plane root's branch, reset and update, a branch folder's git, the
    /// project's lock files.
    GuardsGit,
    /// The merge hold: a chat's merge is refused and handed to the person.
    GuardsMerge,
    /// The release floor: an unattended chat may not publish.
    GuardsRelease,
    /// What a chat commits that is not a credential: personal data, a commit that skips the
    /// scan, the scan's own allowlist and runs-under files.
    GuardsSave,
    /// One credential per forge, and the token scan on commit and push.
    GuardsCredential,
    /// A host, folder or socket ask (Q308a).
    Asks,
    /// Who may dispatch to whom, and the guards on a dispatch's or a handoff's call.
    Dispatch,
    /// A chat's reach to a vault: reads across personas, a vault's Allow. Off or enforce only.
    Vaults,
    /// The browser's reach to a site.
    Browser,
    /// Installing a skill against the project's skill policy.
    Skills,
    /// A vault's value kept out of a chat: masking, the leak guard, the vault-file Read guard.
    Secrets,
    /// The person's own credential files at the floor (ADR 0067 §5 class 6, ruling Q325): an SSH
    /// private key and each forge and cloud credential file, unread by a chat at the floor only
    /// where this is enforced. The full sandbox holds them whatever it says. Off or enforce only.
    CredentialFiles,
    /// The guards on a forge: its verbs and its login (Q308b).
    GuardsForge,
    /// The shape of a call: the tool gate's caps, prose, handoff and plumbing arms, and the
    /// backstops on the consent rules' spelling (Q308b).
    GuardsShape,
    /// The plane itself: the plane root's git, brokered writes, proposals, removal (Q308b).
    GuardsPlane,
    /// The harness's permission mode and attendance (Q308b).
    HarnessMode,
    /// Persona tools, disallowed tools, MCP servers and pre-allows (Q308b).
    Tools,
    /// Profiles, harness declarations and extensions a project trusts (Q308b).
    Trust,
    /// Runners and pools (Q308b).
    Runners,
    /// The window-only presses and reports (Q308b).
    Consents,
    /// Depth, concurrency, rates, timeouts and budgets: under `log` a limit is counted, not
    /// applied (Q308b).
    Limits,
}

impl Family {
    /// Every family, in the order the docs list them.
    pub const ALL: [Family; 25] = [
        Family::Sandbox,
        Family::Network,
        Family::Ports,
        Family::Processes,
        Family::GuardsGit,
        Family::GuardsMerge,
        Family::GuardsRelease,
        Family::GuardsSave,
        Family::GuardsCredential,
        Family::Asks,
        Family::Dispatch,
        Family::Vaults,
        Family::Browser,
        Family::Skills,
        Family::Secrets,
        Family::CredentialFiles,
        Family::GuardsForge,
        Family::GuardsShape,
        Family::GuardsPlane,
        Family::HarnessMode,
        Family::Tools,
        Family::Trust,
        Family::Runners,
        Family::Consents,
        Family::Limits,
    ];

    /// The family's key, as `[policy]` and the log spell it.
    pub fn key(self) -> &'static str {
        match self {
            Family::Sandbox => "sandbox",
            Family::Network => "network",
            Family::Ports => "ports",
            Family::Processes => "processes",
            Family::GuardsGit => "guards.git",
            Family::GuardsMerge => "guards.merge",
            Family::GuardsRelease => "guards.release",
            Family::GuardsSave => "guards.save",
            Family::GuardsCredential => "guards.credential",
            Family::Asks => "asks",
            Family::Dispatch => "dispatch",
            Family::Vaults => "vaults",
            Family::Browser => "browser",
            Family::Skills => "skills",
            Family::Secrets => "secrets",
            Family::CredentialFiles => "secrets.credential_files",
            Family::GuardsForge => "guards.forge",
            Family::GuardsShape => "guards.shape",
            Family::GuardsPlane => "guards.plane",
            Family::HarnessMode => "mode",
            Family::Tools => "tools",
            Family::Trust => "trust",
            Family::Runners => "runners",
            Family::Consents => "consents",
            Family::Limits => "limits",
        }
    }

    /// The family whose key is `key`, or none.
    pub fn of_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|family| family.key() == key)
    }

    /// Whether Q308b added the family: `team` and `strict` enforce it, `watch` and `hold` log it,
    /// `open` turns it off. `trust` is the exception (Q329): every template but `open` enforces
    /// it.
    pub fn added_by_q308b(self) -> bool {
        matches!(
            self,
            Family::GuardsForge
                | Family::GuardsShape
                | Family::GuardsPlane
                | Family::HarnessMode
                | Family::Tools
                | Family::Trust
                | Family::Runners
                | Family::Consents
                | Family::Limits
        )
    }

    /// Whether the family takes `log`: `vaults` takes off or enforce only (Q308a), the sandbox
    /// is a level, not a mode, and `ports` is the kernel's refusal of a local socket or a port of
    /// this machine, which tells nothing of a connection it let through, so it has no line to
    /// write (the Q308 follow-up, item 5): `log` there is enforced, and said.
    pub fn logs(self) -> bool {
        !matches!(
            self,
            Family::Sandbox | Family::Vaults | Family::Ports | Family::CredentialFiles
        )
    }

    /// **Whether `[policy]` switches the family yet.** Not where its wiring waits on a later
    /// ruling; those stay enforced, as before Q308, whatever `[policy]` says:
    ///
    /// - `browser`: it holds grants or remembered answers (browser reach) whose fit with
    ///   `[policy]` a later slice settles. The sandbox's level, `network` and `ports` switch since
    ///   the follow-up's items 1 and 5 (ruling Q328: `ports` off or enforce). `dispatch` switches
    ///   whole (ticket T1.1-73): a policy ask or deny goes ahead under `off` and `log`, and a lock
    ///   and the person's nevers hold in every mode. `asks` switches its simple half (instruction
    ///   of 2026-11-16): a host or folder ask is answered Allow (a socket only under `off`), and
    ///   Keep blocked holds in every mode. `vaults` switches off or enforce only, at a vault's
    ///   check against its persona and the in-chat refusal of a keyring read (item 4 of the
    ///   follow-up); the declared-tool gate of a brokered run is the `tools` family's;
    /// - `processes`: the wrap's refusal of programs that would run outside it (`launchctl`,
    ///   `osascript`, `open`, cron, Apple Events) is part of the sandbox, kept whenever a wrap is
    ///   applied (the review of #6992); what the family may lift beyond that is the follow-up's;
    /// - `skills`: every skills refusal a chat meets is a hand-over to the person (a draft, a
    ///   proposal) or the org's `[skill_policy]` on an install only the window makes, so which
    ///   of them the family switches needs a ruling.
    ///
    /// The nine families Q308b added switch at the choke points
    /// `docs/policy-inventory-families.md` names; a row of theirs that no choke point asks yet
    /// stays enforced, as before Q308.
    pub fn switchable(self) -> bool {
        matches!(
            self,
            Family::Sandbox
                | Family::Network
                | Family::Ports
                | Family::Dispatch
                | Family::Vaults
                | Family::Asks
                | Family::GuardsGit
                | Family::GuardsMerge
                | Family::GuardsRelease
                | Family::GuardsSave
                | Family::GuardsCredential
                | Family::Secrets
                | Family::CredentialFiles
        ) || self.added_by_q308b()
    }
}

/// How a family's refusals run. Ordered from the loosest: the stricter of two is the greater.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mode {
    /// The check does not run.
    Off,
    /// The check runs and allows, and one quiet line records what it would have refused.
    Log,
    /// The check refuses, as purlis always has.
    Enforce,
}

impl Mode {
    /// The mode's word.
    pub fn word(self) -> &'static str {
        match self {
            Mode::Off => "off",
            Mode::Log => "log",
            Mode::Enforce => "enforce",
        }
    }

    /// The mode whose word is `word`, or none.
    pub fn parse(word: &str) -> Option<Self> {
        [Mode::Off, Mode::Log, Mode::Enforce]
            .into_iter()
            .find(|mode| mode.word() == word)
    }
}

/// A project's starting point, which single families override.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Template {
    /// Everything off.
    Open,
    /// Everything logged; `secrets`, `vaults`, `guards.credential` and `trust` enforced. The
    /// default.
    #[default]
    Watch,
    /// `watch`, and `guards.merge` enforced.
    Hold,
    /// The merge, release, save and credential guards, `secrets`, `vaults`, `trust` and the
    /// families Q308b added enforced; the rest logged.
    Team,
    /// Everything enforced: purlis as it was before Q308.
    Strict,
}

impl Template {
    /// Every template.
    pub const ALL: [Template; 5] = [
        Template::Open,
        Template::Watch,
        Template::Hold,
        Template::Team,
        Template::Strict,
    ];

    /// The template's word.
    pub fn word(self) -> &'static str {
        match self {
            Template::Open => "open",
            Template::Watch => "watch",
            Template::Hold => "hold",
            Template::Team => "team",
            Template::Strict => "strict",
        }
    }

    /// The template whose word is `word`, or none.
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|template| template.word() == word)
    }

    /// How the template runs `family`. The sandbox's entry is only whether a wrap is applied at
    /// all; its level is [`Template::sandbox_level`]'s.
    pub fn mode(self, family: Family) -> Mode {
        // The person's own credential files at the floor (Q325): `team` and `strict` hold them,
        // and the floor templates leave the person's tools working, as before Q321.
        if family == Family::CredentialFiles {
            return match self {
                Template::Team | Template::Strict => Mode::Enforce,
                Template::Open | Template::Watch | Template::Hold => Mode::Off,
            };
        }
        let secret = matches!(
            family,
            Family::Secrets | Family::Vaults | Family::GuardsCredential
        );
        let mode = match self {
            Template::Open => Mode::Off,
            Template::Strict => Mode::Enforce,
            Template::Watch if secret => Mode::Enforce,
            // **An unapproved profile or harness declaration still needs the person's approval**
            // (Q329): only `open`, or the person's own `trust = "off"`, lets one run unapproved.
            Template::Watch | Template::Hold if family == Family::Trust => Mode::Enforce,
            Template::Hold if secret || family == Family::GuardsMerge => Mode::Enforce,
            Template::Team
                if secret
                    || family.added_by_q308b()
                    || matches!(
                        family,
                        Family::GuardsMerge | Family::GuardsRelease | Family::GuardsSave
                    ) =>
            {
                Mode::Enforce
            }
            Template::Watch | Template::Hold | Template::Team => Mode::Log,
        };
        if family == Family::Sandbox {
            return mode_of_level(self.sandbox_level().unwrap_or(SandboxLevel::Floor));
        }
        if mode == Mode::Log && !family.logs() {
            Mode::Enforce
        } else {
            mode
        }
    }

    /// **The sandbox level the template sets** (Q308a, Q316), or `None` for `strict`, under
    /// which the project's `[sandbox]` decides it as it always has: the floor, unless
    /// `[sandbox] mode = "on"`.
    pub fn sandbox_level(self) -> Option<SandboxLevel> {
        match self {
            Template::Open => Some(SandboxLevel::Off),
            Template::Watch | Template::Hold => Some(SandboxLevel::Floor),
            Template::Team => Some(SandboxLevel::Full),
            Template::Strict => None,
        }
    }
}

/// The sandbox family's three values (Q308a). Ordered from the loosest: the stricter of two is
/// the greater.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SandboxLevel {
    /// No wrap.
    Off,
    /// Today's floor: only purlis's own secret stores are unreadable.
    Floor,
    /// The full sandbox.
    Full,
}

impl SandboxLevel {
    /// Every level, loosest first.
    pub const ALL: [SandboxLevel; 3] = [SandboxLevel::Off, SandboxLevel::Floor, SandboxLevel::Full];

    /// The level's word, as the accepted record, [`ENV`] and the window spell it.
    pub fn word(self) -> &'static str {
        match self {
            SandboxLevel::Off => "off",
            SandboxLevel::Floor => "floor",
            SandboxLevel::Full => "full",
        }
    }

    /// The level whose word is `word`, or none. `enforce`, which an acceptance kept before the
    /// level was wired spelled the sandbox with, is the floor: the least a wrap held then.
    pub fn parse(word: &str) -> Option<Self> {
        if word == Mode::Enforce.word() {
            return Some(SandboxLevel::Floor);
        }
        Self::ALL.into_iter().find(|level| level.word() == word)
    }

    /// **What `[sandbox] mode` in `top`, a parsed settings file, says of the level**, where it
    /// says one: the family's override (Q308a, the follow-up's item 1). `"on"` is the full
    /// sandbox; `"off"` is no sandbox where the team's policy is `local` (the team's off) and
    /// the full sandbox under `never`, which refuses it; any other value is read as `"on"`, as
    /// [`crate::sandbox::Said`] reads it. `None` where the file names no mode, and the template
    /// decides.
    pub fn of_sandbox_mode(top: &toml::Table) -> Option<Self> {
        let table = top.get(crate::sandbox::TABLE)?.as_table()?;
        let mode = table.get("mode")?;
        let team = crate::sandbox::team::Team::of_top(Some(top));
        Some(match mode.as_str() {
            Some("off") if team.off => SandboxLevel::Off,
            _ => SandboxLevel::Full,
        })
    }
}

/// The sandbox family's mode at `level`: off where there is no wrap, enforced wherever one is
/// applied.
pub fn mode_of_level(level: SandboxLevel) -> Mode {
    if level == SandboxLevel::Off {
        Mode::Off
    } else {
        Mode::Enforce
    }
}

/// What a project's `[policy]` says.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Settings {
    /// The template, [`Template::Watch`] where none is said.
    pub template: Template,
    /// Whether the file named a template at all.
    pub template_said: bool,
    /// The families the project sets apart from its template.
    pub overrides: BTreeMap<Family, Mode>,
    /// **The sandbox level the file's `[sandbox] mode` sets apart from the template**, where it
    /// names one ([`SandboxLevel::of_sandbox_mode`]): the `sandbox` family's override. Never a
    /// key of `[policy]`, so there are never two switches (Q308a).
    pub sandbox: Option<SandboxLevel>,
    /// What the table holds that purlis could not read, one sentence each, for the window's
    /// Notice. Each was taken in the strict direction.
    pub refused: Vec<String>,
    /// **Whether git could not read the committed file** ([`committed_text`]): the project is
    /// read as `strict` meanwhile, and that is never kept as what the person accepted, since it
    /// is no tightening the project made (#7073).
    pub unread: bool,
    /// What the table says that purlis read, but enforces in place of what it says, one
    /// sentence each: `log` for a family that takes off or enforce only. Said in the Notice with
    /// [`Self::refused`], but it makes only its own family stricter, never the whole policy.
    taken: Vec<String>,
    /// **The project's dispatch denies** (`[dispatch.denies]`, ticket T1.1-73, and the review
    /// of #7113): read with `[policy]`, from the same committed text, so a deny is accepted,
    /// and lifted, the way a family is. A file that does not read denies every dispatch across
    /// personas ([`crate::dispatchgrant::Denied::all`]).
    pub denies: crate::dispatchgrant::Denied,
    /// **Whether the project denies lineage** (`[policy] lineage = "deny"`, [`LINEAGE_KEY`]): a
    /// parent chat taking up its ended child's cut branch through the forge route. An allowance
    /// whose strict value is the denial, so **`false` is what a file that says nothing means**,
    /// and a value purlis cannot read, a file that is not TOML and a `HEAD` git could not read all
    /// deny it ([`Self::from_text`], [`Self::read`]).
    pub lineage_denied: bool,
    /// **The branches the project's committed `[risk] protected_branches` names** (names, or
    /// patterns with `*`): what the forge route refuses every chat push to. Read from the same
    /// committed text as the rest, so a list edited in the working tree and not committed is no
    /// list; an unread file adds none, which [`Resolved`]'s accepted floor then keeps.
    pub protected: Vec<String>,
}

impl Settings {
    /// **Whether the committed `[policy]` itself lets merges through the forge route** (the
    /// person's ruling of 2026-11-22): it names `guards.merge` by key, or a template other than
    /// the default `watch`. A project with no `[policy]`, or on `watch` with no `guards.merge`,
    /// runs the guard as `log` for the hook by the template's default, and the route holds its
    /// merges. Not enough alone: [`Resolved::merges_opted_in`] needs the person's press too.
    pub fn merges_opted_in(&self) -> bool {
        // A file that did not read, or holds a value that did not, is `strict` and opts in to nothing.
        !self.unread
            && self.refused.is_empty()
            && (self.overrides.contains_key(&Family::GuardsMerge)
                || (self.template_said && self.template != Template::Watch))
    }

    /// What `text`, a settings file, says; the default where it is `None`, holds no `[policy]`,
    /// or is not TOML (that file's own reader says why).
    pub fn from_text(text: Option<&str>) -> Self {
        let mut said = Self::read_text(text);
        said.denies = crate::dispatchgrant::Denied::of_text(text);
        // **Any value purlis cannot read means `strict`**, with the sentence for the Notice (the
        // review of #6992, S6): a file that is not TOML, a key that is no family, a mode or a
        // template that is none.
        if !said.refused.is_empty() {
            said.template = Template::Strict;
            said.template_said = true;
            said.overrides.clear();
            said.lineage_denied = true;
        }
        // A `log` a family does not take is that family enforced, and said (the review of
        // #7105): read, so never the whole policy strict.
        let taken = std::mem::take(&mut said.taken);
        said.refused.extend(taken);
        said
    }

    fn read_text(text: Option<&str>) -> Self {
        let Some(text) = text else {
            return Self::default();
        };
        let Ok(top) = text.parse::<toml::Table>() else {
            return Self {
                refused: vec![
                    "the project's settings file is not TOML, so every family is enforced"
                        .to_owned(),
                ],
                lineage_denied: true,
                ..Self::default()
            };
        };
        let sandbox = SandboxLevel::of_sandbox_mode(&top);
        let protected = crate::askrisk::protected_in(&top);
        let Some(table) = top.get(TABLE).cloned() else {
            return Self {
                sandbox,
                protected,
                ..Self::default()
            };
        };
        let mut said = Self {
            sandbox,
            protected,
            ..Self::default()
        };
        let Some(table) = table.as_table() else {
            said.template = Template::Strict;
            said.template_said = true;
            said.refused.push(format!(
                "[{TABLE}] is not a table, so every family is enforced"
            ));
            said.lineage_denied = true;
            return said;
        };
        for (key, value) in table {
            if key == TEMPLATE_KEY {
                said.template_said = true;
                match value.as_str().and_then(Template::parse) {
                    Some(template) => said.template = template,
                    None => {
                        said.template = Template::Strict;
                        said.refused.push(format!(
                            "{TABLE}.{TEMPLATE_KEY} is not open, watch, hold, team or strict, so \
                             every family it does not override is enforced"
                        ));
                    }
                }
            } else if key == LINEAGE_KEY {
                // An allowance: `deny` is the tightening, and anything else is read as it.
                match value.as_str() {
                    Some("allow") => {}
                    Some("deny") => said.lineage_denied = true,
                    _ => {
                        said.lineage_denied = true;
                        said.refused.push(format!(
                            "{TABLE}.{LINEAGE_KEY} is not allow or deny, so lineage is denied"
                        ));
                    }
                }
            } else if key == "secrets" && value.is_table() {
                // `secrets.credential_files = "enforce"` makes `secrets` a table (Q325): each of
                // its keys is a family, and the `secrets` family stays the template's. A project
                // that sets both writes the quoted key `"secrets.credential_files"` beside
                // `secrets = …`, since TOML gives one key one value.
                if let Some(keys) = value.as_table() {
                    for (sub, value) in keys {
                        said.family(&format!("secrets.{sub}"), value);
                    }
                }
            } else if key == "guards" {
                match value.as_table() {
                    Some(guards) => {
                        for (guard, value) in guards {
                            said.family(&format!("guards.{guard}"), value);
                        }
                    }
                    None => said.refused.push(format!(
                        "{TABLE}.guards is not a table of git, merge, release, save, credential, \
                         forge, shape and plane, and is not read"
                    )),
                }
            } else {
                said.family(key, value);
            }
        }
        said
    }

    /// One family's override, `key = value`.
    fn family(&mut self, key: &str, value: &toml::Value) {
        let Some(family) = Family::of_key(key) else {
            self.refused.push(format!(
                "{TABLE}.{key} is not a family purlis knows, and is not read"
            ));
            return;
        };
        if family == Family::Sandbox {
            self.refused.push(format!(
                "{TABLE}.sandbox is not read: the template sets the sandbox, and [sandbox] mode \
                 sets it apart from the template"
            ));
            return;
        }
        match value.as_str().and_then(Mode::parse) {
            Some(Mode::Log) if !family.logs() => {
                self.overrides.insert(family, Mode::Enforce);
                self.taken.push(format!(
                    "{TABLE}.{key} takes off or enforce, so log is enforced"
                ));
            }
            Some(mode) => {
                self.overrides.insert(family, mode);
            }
            None => {
                self.overrides.insert(family, Mode::Enforce);
                self.refused.push(format!(
                    "{TABLE}.{key} is not off, log or enforce, so it is enforced"
                ));
            }
        }
    }

    /// **The plane at `root`'s `[policy]`**: its committed file's as it stands at the plane
    /// root's `HEAD` ([`committed_text`], Q309 point 1), made stricter where the local file is
    /// stricter, and never looser for anything the local file says (Q308a). `root` is the plane
    /// root; a branch folder's own file is never asked.
    pub fn read(root: &Path) -> Self {
        use crate::settings::{Which, layer_text};
        let mut project = match committed_text(root) {
            Ok(text) => Self::from_text(text.as_deref()),
            // A `HEAD` git could not read in time is not a file to fall back to (the
            // re-review of #6992): strict, with the sentence for the Notice.
            Err(why) => Self {
                template: Template::Strict,
                template_said: true,
                overrides: BTreeMap::new(),
                // No level of its own: what this person accepted holds, never a looser one.
                sandbox: None,
                refused: vec![why],
                unread: true,
                taken: Vec::new(),
                denies: crate::dispatchgrant::Denied::all(),
                lineage_denied: true,
                protected: Vec::new(),
            },
        };
        let local = Self::from_text(layer_text(root, Which::Local).text());
        // The local file can add a deny, never take one away.
        project.denies = project.denies.with(&local.denies);
        // And a denial of lineage; never one taken away. The protected branches are the committed
        // file's alone, as `[risk]` is.
        project.lineage_denied |= local.lineage_denied;
        project.protected.sort();
        project.protected.dedup();
        for family in Family::ALL {
            // Only what the local file says: a family it is silent on is the project's.
            let mine = local
                .overrides
                .get(&family)
                .copied()
                .or_else(|| local.template_said.then(|| local.template.mode(family)));
            if let Some(mine) = mine
                && mine > project.mode(family)
            {
                project.overrides.insert(family, mine);
            }
        }
        // The local file's template may raise the level, never lower it; its `[sandbox]` is
        // the person's own level, read where a start reads it, and not the project's.
        if let Some(mine) = local
            .template_said
            .then(|| local.template.sandbox_level())
            .flatten()
            && mine > project.level()
        {
            project.sandbox = Some(mine);
        }
        project.refused.extend(local.refused);
        project
    }

    /// **The sandbox level the settings set** (Q308a, Q316): `[sandbox] mode` where the file names
    /// one, else the template's; under `strict`, whose template leaves it to `[sandbox]`, the
    /// floor where `[sandbox]` names no mode, as before Q308.
    pub fn level(&self) -> SandboxLevel {
        self.sandbox
            .unwrap_or_else(|| self.template.sandbox_level().unwrap_or(SandboxLevel::Floor))
    }

    /// How the settings run `family`, before any lock or acceptance.
    pub fn mode(&self, family: Family) -> Mode {
        if family == Family::Sandbox {
            return mode_of_level(self.level());
        }
        let own = self
            .overrides
            .get(&family)
            .copied()
            .unwrap_or_else(|| self.template.mode(family));
        // **A3 and A3b moved from `guards.git` to `guards.plane`** (Q308b): a project that
        // names `guards.git` and not `guards.plane` keeps them at least as strict as its
        // `guards.git` says, so the move loosens nothing it set (#7104).
        if family == Family::GuardsPlane
            && !self.overrides.contains_key(&family)
            && let Some(git) = self.overrides.get(&Family::GuardsGit)
        {
            return own.max(*git);
        }
        own
    }
}

/// How long reading the committed file at `HEAD` may take.
const GIT_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// **The plane root's committed settings file as it stands at `HEAD`** (Q309 point 1): a
/// change not committed yet is not the project's. Where the plane is no git repository, the
/// file as it stands is all there is. `Ok(None)` where there is no such file; an error, which
/// reads as `strict`, where the plane is a repository whose `HEAD` git could not read or did not
/// read in time: never the working tree's file then (the re-review of #6992).
pub fn committed_text(root: &Path) -> Result<Option<String>, String> {
    use crate::settings::Which;
    let unread =
        || "git could not read the project's last commit, so its [policy] is enforced".to_owned();
    match crate::worktree::git::run(
        root,
        &["rev-parse", "--verify", "--quiet", "HEAD"],
        GIT_WAIT,
    ) {
        Ok(run) if run.ok() => {}
        // git said the plane is no repository at all, and it has no `.git` git could have
        // failed to read (#7073): its file is all there is. A `.git` that is there and does not
        // read, a corrupted `HEAD` say, is strict.
        Ok(run)
            if run.code == Some(128)
                && run.err.contains("not a git repository")
                && root.join(".git").symlink_metadata().is_err() =>
        {
            // A file that is there and does not read (a folder, a file this process may not
            // read) is not "no file" (the review of #7113): strict.
            let path = Which::Shared.path(root);
            return match std::fs::metadata(&path) {
                Err(none) if none.kind() == std::io::ErrorKind::NotFound => Ok(None),
                _ => std::fs::read_to_string(&path).map(Some).map_err(|_| {
                    "purlis could not read the project's settings file, so its [policy] is \
                     enforced"
                        .to_owned()
                }),
            };
        }
        // Timed out, failed, or no commit yet: never the working tree's file.
        _ => return Err(unread()),
    }
    let file = Which::Shared.file_at(root);
    match crate::worktree::git::run(root, &["show", &format!("HEAD:./{file}")], GIT_WAIT) {
        Ok(run) if run.ok() => Ok(Some(run.out)),
        // No such file at `HEAD`, as git says it: the project says nothing. git exits 128 for a
        // blob it could not read too, which is strict (#7073).
        Ok(run) if run.code == Some(128) && no_such_path(&run.err) => Ok(None),
        _ => Err(unread()),
    }
}

/// **Whether git's `show <rev>:<path>` said there is no such path at `rev`**: the path is in no
/// commit (`does not exist in`), or only in the working tree (`exists on disk, but not in`).
/// git runs with `LC_ALL=C`, so its words are these.
fn no_such_path(err: &str) -> bool {
    err.contains("does not exist in") || err.contains("exists on disk, but not in")
}

/// **The digest of what the press accepts** (the review of #6992, S2, and its re-review): of the
/// modes the project resolves to here, from the committed file at `HEAD`, the local file and this
/// machine's locks, so a press refuses anything that changed since the window showed it.
pub fn digest_of(root: &Path) -> String {
    digest(&committed(root))
}

/// The digest of `resolved`, as [`digest_of`] names it.
fn digest(resolved: &Resolved) -> String {
    use sha2::Digest;
    // The denies are part of what is accepted, so of what is digested (the review of #7113).
    let text = format!(
        "{}\n{}\nmerges={}\nlineage={}\nprotected={}",
        resolved.spelled(),
        resolved.denies.spelled(),
        resolved.merges,
        resolved.lineage,
        resolved.protected_spelled()
    );
    crate::extension::hex(&sha2::Sha256::digest(text.as_bytes()))
}

/// The key an acceptance keeps the denies it accepted under: no family's key.
const DENIES_KEPT: &str = "dispatch.denies";

/// The key an acceptance keeps a denial of lineage under ([`Resolved::lineage`]), kept only where
/// lineage was denied: an acceptance with none (from before the key existed) reads as allowed,
/// the default.
const LINEAGE_KEPT: &str = "lineage";

/// The key an acceptance keeps the protected branches it accepted under
/// ([`Resolved::protected_branches`]), one per line (a branch name has no control character).
const PROTECTED_KEPT: &str = "risk.protected_branches";

/// The key an acceptance keeps the merge opt-in under ([`Resolved::merges_opted_in`]), kept only
/// where it was accepted: an acceptance from before it existed has none, which reads as not opted in.
const MERGES_KEPT: &str = "guards.merge.opted_in";

/// How every family runs for one chat: what [`at_start`] resolved, the sandbox's level, and the
/// dispatch denies.
#[derive(Debug, Clone)]
pub struct Resolved {
    modes: BTreeMap<Family, Mode>,
    /// The project's sandbox level ([`SandboxLevel`]); the `sandbox` family's mode follows it
    /// ([`mode_of_level`]).
    level: SandboxLevel,
    /// The dispatch denies (`[dispatch.denies]`, ticket T1.1-73).
    denies: crate::dispatchgrant::Denied,
    /// **Whether the project opted in to merges through the forge route** ([`Settings::merges_opted_in`]).
    /// Part of what a person accepts: turning it on is a loosening that waits for the press,
    /// though `guards.merge` itself may read `log` either way (the default `watch` runs it as
    /// `log`). Taking it away is a tightening, at once. An acceptance kept before this existed
    /// reads as not opted in.
    merges: bool,
    /// **Whether a parent may take up its ended child's cut branch through the forge route**
    /// (`[policy] lineage`, [`Settings::lineage_denied`]): an allowance, on by default, whose
    /// strict value is the denial. Denying it is a tightening that holds at once and is kept;
    /// allowing it again is a loosening that waits for the press. Unlike a family, it is not
    /// carried to a hook ([`Self::spelled`]): only the app's route asks it.
    lineage: bool,
    /// **The protected branches the project's committed file named** ([`Settings::protected`]),
    /// as this person accepted them: never fewer ([`Self::stricter`]), and a name the project
    /// drops waits for the press ([`Self::protection_lifted`]).
    protected: BTreeSet<String>,
    /// **The families enforced only because this machine's acceptance is Broken or Waiting**
    /// (ADR 0130 §7): enforced here, where the project's committed file under this machine's
    /// locks would not enforce them. What a refusal names as its cause ([`Self::unaccepted`]).
    /// A cause, not a mode: it never changes how a family runs, and equality ignores it.
    unaccepted: BTreeSet<Family>,
    /// **The dispatch denies held only because the acceptance is Broken or Waiting**: in
    /// [`Self::denies`], and not in what the committed file denies.
    unaccepted_denies: crate::dispatchgrant::Denied,
}

/// Equal where they run the same: the cause of an enforced family ([`Resolved::unaccepted`]) is
/// what a refusal says, not how anything runs, so a policy kept as accepted compares as it runs.
impl PartialEq for Resolved {
    fn eq(&self, other: &Self) -> bool {
        self.modes == other.modes
            && self.level == other.level
            && self.denies == other.denies
            && self.merges == other.merges
            && self.lineage == other.lineage
            && self.protected == other.protected
    }
}

impl Eq for Resolved {}

impl Resolved {
    /// Every family at `mode`, the sandbox off where `mode` is and at the floor otherwise: what a
    /// process with no answer runs ([`Mode::Enforce`]), where the project's `[sandbox]` may still
    /// raise the level ([`sandbox_level`]); and no dispatch denied.
    pub fn all(mode: Mode) -> Self {
        let level = if mode == Mode::Off {
            SandboxLevel::Off
        } else {
            SandboxLevel::Floor
        };
        Self::of_modes(
            Family::ALL
                .into_iter()
                .map(|family| (family, mode))
                .collect(),
            level,
            crate::dispatchgrant::Denied::default(),
            false,
        )
    }

    /// **`strict`**: every family enforced, and every dispatch across personas denied. What an
    /// acceptance that does not hold reads as.
    pub fn strict() -> Self {
        Self {
            denies: crate::dispatchgrant::Denied::all(),
            lineage: false,
            ..Self::all(Mode::Enforce)
        }
    }

    /// `modes` at `level`, the `sandbox` family's mode set by the level, with `denies`.
    fn of_modes(
        mut modes: BTreeMap<Family, Mode>,
        level: SandboxLevel,
        denies: crate::dispatchgrant::Denied,
        merges: bool,
    ) -> Self {
        modes.insert(Family::Sandbox, mode_of_level(level));
        Self {
            modes,
            level,
            denies,
            merges,
            lineage: true,
            protected: BTreeSet::new(),
            unaccepted: BTreeSet::new(),
            unaccepted_denies: crate::dispatchgrant::Denied::default(),
        }
    }

    /// `settings` under `locks`: each family as the settings say, raised to its lock, and
    /// enforced where it is not switchable yet; the level as the settings set it, never below
    /// the floor where a lock holds the sandbox.
    pub fn of(settings: &Settings, locks: &crate::sandbox::policy::Locks) -> Self {
        let mut level = settings.level();
        if locks.enforcement_floor(Family::Sandbox).is_some() {
            level = level.max(SandboxLevel::Floor);
        }
        let modes: BTreeMap<Family, Mode> = Family::ALL
            .into_iter()
            .map(|family| (family, under(settings, locks, family)))
            .collect();
        // The opt-in counts only where it loosens the route: a `guards.merge` that resolves to
        // `enforce` (a `hold`, `team` or `strict` template, a lock, or the key itself) holds
        // merges in the hook and the route alike, so there is nothing to wait for.
        let merges = settings.merges_opted_in()
            && modes
                .get(&Family::GuardsMerge)
                .is_some_and(|mode| *mode < Mode::Enforce);
        let mut resolved = Self::of_modes(modes, level, settings.denies.clone(), merges);
        resolved.lineage = !settings.lineage_denied && !locks.forbids_lineage();
        resolved.protected = settings.protected.iter().cloned().collect();
        resolved
    }

    /// **How a chat runs, with `chain` the strictest policy along the chain that dispatched it**
    /// ([`crate::reopen::HandedFrom::policy`]): the stricter of the two, family by family, so a
    /// dispatched chat is never looser than its asker (Q308a, Q308b). `None` for a chat nobody
    /// dispatched, which runs as `self`.
    #[must_use]
    pub fn for_chat(self, chain: Option<&str>) -> Self {
        match chain {
            Some(chain) => self.stricter(&Self::parse(chain)),
            None => self,
        }
    }

    /// How `family` runs.
    pub fn mode(&self, family: Family) -> Mode {
        self.modes.get(&family).copied().unwrap_or(Mode::Enforce)
    }

    /// **The sandbox level resolved** (Q308a): what the project sets, before a person's own level
    /// and the team's `never` ([`crate::sandbox::team::allowed_levels`]).
    pub fn level(&self) -> SandboxLevel {
        self.level
    }

    /// **The dispatch denies resolved** (`[dispatch.denies]`, ticket T1.1-73): what the
    /// project's committed file says and what this person accepted, whichever denies more.
    pub fn denies(&self) -> &crate::dispatchgrant::Denied {
        &self.denies
    }

    /// **Whether merges through the forge route are opted in and accepted**: the project's
    /// committed `[policy]` names `guards.merge` or a template other than `watch`
    /// ([`Settings::merges_opted_in`]), and the person pressed Accept for it. In [`in_force`] it
    /// is both at once.
    pub fn merges_opted_in(&self) -> bool {
        self.merges
    }

    /// The stricter of `self` and `other`, family by family, the stricter level, and every deny
    /// of either.
    pub fn stricter(&self, other: &Self) -> Self {
        let mut stricter = Self::of_modes(
            Family::ALL
                .into_iter()
                .map(|family| (family, self.mode(family).max(other.mode(family))))
                .collect(),
            self.level.max(other.level),
            self.denies.with(&other.denies),
            self.merges && other.merges,
        );
        // An allowance holds only where both allow it; a protection where either has it.
        stricter.lineage = self.lineage && other.lineage;
        stricter.protected = self.protected.union(&other.protected).cloned().collect();
        // **A cause is said only where acceptance is the reason for this chat** (ADR 0130 §7): a
        // family one side enforces with no cause (the start's [`ENV`], the chain's policy, which
        // carry none) is enforced on its own merits, and the press would not lift its refusal.
        let own_merits = |side: &Self, family: Family| {
            side.mode(family) == Mode::Enforce && !side.unaccepted(family)
        };
        stricter.unaccepted = self
            .unaccepted
            .union(&other.unaccepted)
            .copied()
            .filter(|family| !own_merits(self, *family) && !own_merits(other, *family))
            .collect();
        stricter.unaccepted_denies = self.unaccepted_denies.with(&other.unaccepted_denies);
        stricter
    }

    /// **The policy in force where `accepted` is what this machine accepted and `committed` what
    /// the project's file says under its locks**: their stricter, with each family it enforces
    /// that `committed` alone would not marked as held by the unaccepted policy (ADR 0130 §7).
    /// Under a held acceptance with nothing waiting, `committed` is never looser, and nothing
    /// is marked.
    fn accepting(committed: &Self, accepted: &Self) -> Self {
        let mut resolved = committed.stricter(accepted);
        resolved.unaccepted = Family::ALL
            .into_iter()
            .filter(|family| {
                resolved.mode(*family) == Mode::Enforce && committed.mode(*family) < Mode::Enforce
            })
            .collect();
        resolved.unaccepted_denies = crate::dispatchgrant::Denied::parse(
            &resolved.denies.without(&committed.denies).join("|"),
        );
        resolved
    }

    /// **Whether `family` is enforced only because this machine's acceptance of the project's
    /// policy is Broken or Waiting** (ADR 0130 §7): its refusal names the unaccepted policy, and
    /// never tells the person to set the family to `log`. False for a family enforced on its own
    /// merits: by the committed file, its template, or an administrator's lock.
    pub fn unaccepted(&self, family: Family) -> bool {
        self.unaccepted.contains(&family)
    }

    /// The keys of [`Self::unaccepted`]'s families, as the app's Policy answer carries them
    /// ([`crate::hookwire::Answer::Policy`]).
    pub fn unaccepted_keys(&self) -> Vec<String> {
        self.unaccepted
            .iter()
            .map(|family| family.key().to_owned())
            .collect()
    }

    /// **This, with the families `keys` names marked as held by the unaccepted policy**: what a
    /// hook reads off the app's Policy answer. A key it does not know, or a family this does not
    /// enforce, marks nothing: a cause changes the words of a refusal, never whether it is made.
    #[must_use]
    pub fn with_unaccepted(mut self, keys: &[String]) -> Self {
        self.unaccepted = keys
            .iter()
            .filter_map(|key| Family::of_key(key))
            .filter(|family| self.mode(*family) == Mode::Enforce)
            .collect();
        self
    }

    /// **The dispatch denies held only because the acceptance is Broken or Waiting**: what the
    /// committed file does not deny ([`crate::dispatchgrant::Deny::Unaccepted`]).
    pub fn unaccepted_denies(&self) -> &crate::dispatchgrant::Denied {
        &self.unaccepted_denies
    }

    /// **Whether lineage is allowed**: the project does not deny it, no administrator's lock holds
    /// it off, and what this person accepted does not deny it.
    pub fn lineage(&self) -> bool {
        self.lineage
    }

    /// Whether `self` allows lineage where `accepted` denies it: what a loosening press would
    /// accept.
    pub fn lineage_looser_than(&self, accepted: &Self) -> bool {
        self.lineage && !accepted.lineage
    }

    /// **The protected branches**, names or patterns with `*`: what the committed file names and
    /// this person accepted, never fewer than either.
    pub fn protected_branches(&self) -> Vec<String> {
        self.protected.iter().cloned().collect()
    }

    /// **Whether `self` is looser than `accepted` anywhere**: a family's mode, the merge
    /// opt-in, the sandbox's level, a deny lifted, lineage allowed or a protected branch lifted.
    /// What a loosening press would accept; a tightening alone is none.
    pub fn loosens(&self, accepted: &Self) -> bool {
        !self.looser_than(accepted).is_empty()
            || self.level_looser_than(accepted).is_some()
            || !self.lifts(accepted).is_empty()
            || self.lineage_looser_than(accepted)
            || !self.protection_lifted(accepted).is_empty()
    }

    /// The protected branches `accepted` holds and `self` does not: what a press would lift.
    pub fn protection_lifted(&self, accepted: &Self) -> Vec<String> {
        accepted
            .protected
            .difference(&self.protected)
            .cloned()
            .collect()
    }

    fn protected_spelled(&self) -> String {
        self.protected
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The families `self` runs looser than `accepted`, each with both modes: what a loosening
    /// press would accept. The sandbox is [`Self::level_looser_than`]'s.
    pub fn looser_than(&self, accepted: &Self) -> Vec<(Family, Mode, Mode)> {
        let mut looser: Vec<(Family, Mode, Mode)> = Family::ALL
            .into_iter()
            .filter(|family| *family != Family::Sandbox)
            .filter(|family| self.mode(*family) < accepted.mode(*family))
            .map(|family| (family, accepted.mode(family), self.mode(family)))
            .collect();
        // The merge opt-in is a loosening of the route, which holds every merge until it is
        // accepted, even where the hook's mode reads the same (`watch` is `log`). Its row has
        // equal modes (the hook's, accepted and now): no real loosening has, so the window
        // says it as "merges through the forge route" (`policypress`).
        if self.merges
            && !accepted.merges
            && !looser
                .iter()
                .any(|(family, _, _)| *family == Family::GuardsMerge)
        {
            let mode = self.mode(Family::GuardsMerge);
            looser.push((Family::GuardsMerge, mode, mode));
        }
        looser
    }

    /// The accepted level and this one, where this one is looser: what a loosening press would
    /// accept of the sandbox.
    pub fn level_looser_than(&self, accepted: &Self) -> Option<(SandboxLevel, SandboxLevel)> {
        (self.level < accepted.level).then_some((accepted.level, self.level))
    }

    /// The denies `accepted` holds and `self` does not, each as it is spelled: what a press
    /// would lift (the review of #7113).
    pub fn lifts(&self, accepted: &Self) -> Vec<String> {
        accepted.denies.without(&self.denies)
    }

    /// As [`ENV`] carries it: `family=mode` pairs, comma-separated, the sandbox's by its level.
    /// The denies are not carried: only the app judges a dispatch.
    pub fn spelled(&self) -> String {
        self.modes
            .iter()
            .map(|(family, mode)| {
                let word = if *family == Family::Sandbox {
                    self.level.word()
                } else {
                    mode.word()
                };
                format!("{}={word}", family.key())
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    /// What [`Self::spelled`] spelled. A pair it cannot read, or a family it does not name, is
    /// enforced, and a sandbox level it cannot read is the full sandbox.
    pub fn parse(text: &str) -> Self {
        let pairs: Vec<(&str, &str)> = text
            .split(',')
            .filter_map(|pair| pair.split_once('='))
            .collect();
        let said: BTreeMap<Family, Mode> = pairs
            .iter()
            .filter_map(|(family, mode)| Some((Family::of_key(family)?, Mode::parse(mode)?)))
            .collect();
        let level = pairs
            .iter()
            .find(|(family, _)| *family == Family::Sandbox.key())
            .and_then(|(_, word)| SandboxLevel::parse(word))
            .unwrap_or(SandboxLevel::Full);
        Self::of_modes(
            Family::ALL
                .into_iter()
                .map(|family| (family, said.get(&family).copied().unwrap_or(Mode::Enforce)))
                .collect(),
            level,
            crate::dispatchgrant::Denied::default(),
            // The spelling says nothing of the opt-in, so it never takes it away from what it
            // is joined with ([`Self::stricter`]); [`Self::of_kept`] reads it from the record.
            // Safe only because the route always joins this with a committed-and-accepted
            // `Resolved` (`for_chat`, `in_force`): a `true` that stood alone would open merges.
            true,
        )
    }

    /// As the acceptance keeps it: each family's mode, and the denies under [`DENIES_KEPT`]
    /// where there are any, so an acceptance with none is kept as before.
    fn as_kept(&self) -> BTreeMap<String, String> {
        let mut kept: BTreeMap<String, String> = self
            .modes
            .keys()
            .map(|family| {
                let word = if *family == Family::Sandbox {
                    self.level.word()
                } else {
                    self.mode(*family).word()
                };
                (family.key().to_owned(), word.to_owned())
            })
            .collect();
        if !self.denies.is_empty() {
            kept.insert(DENIES_KEPT.to_owned(), self.denies.spelled());
        }
        if self.merges {
            kept.insert(MERGES_KEPT.to_owned(), "true".to_owned());
        }
        if !self.lineage {
            kept.insert(LINEAGE_KEPT.to_owned(), "denied".to_owned());
        }
        if !self.protected.is_empty() {
            kept.insert(PROTECTED_KEPT.to_owned(), self.protected_spelled());
        }
        kept
    }

    fn of_kept(kept: &BTreeMap<String, String>) -> Self {
        let mut resolved = Self::parse(
            &kept
                .iter()
                .filter(|(key, _)| {
                    ![DENIES_KEPT, MERGES_KEPT, LINEAGE_KEPT, PROTECTED_KEPT]
                        .contains(&key.as_str())
                })
                .map(|(family, mode)| format!("{family}={mode}"))
                .collect::<Vec<_>>()
                .join(","),
        );
        if let Some(denies) = kept.get(DENIES_KEPT) {
            resolved.denies = crate::dispatchgrant::Denied::parse(denies);
        }
        // Fail closed: only an acceptance that says so is opted in.
        resolved.merges = kept.get(MERGES_KEPT).is_some_and(|said| said == "true");
        // Any word under the key denies: only its absence allows.
        resolved.lineage = !kept.contains_key(LINEAGE_KEPT);
        resolved.protected = kept
            .get(PROTECTED_KEPT)
            .map(|spelled| {
                spelled
                    .lines()
                    .filter(|line| !line.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        resolved
    }

    #[cfg(test)]
    pub(crate) fn as_kept_for_test(&self) -> BTreeMap<String, String> {
        self.as_kept()
    }

    #[cfg(test)]
    pub(crate) fn of_kept_for_test(kept: &BTreeMap<String, String>) -> Self {
        Self::of_kept(kept)
    }

    /// This, at `level` where that is looser or stricter: what the team's answer to its off
    /// makes of an acceptance ([`accepted`]).
    fn at_level(mut self, level: SandboxLevel) -> Self {
        self.level = level;
        self.modes.insert(Family::Sandbox, mode_of_level(level));
        self
    }
}

/// `family` as `settings` run it under `locks`: enforced where it is not switchable yet, and
/// never below what a lock holds it at (Q308a: a lock wins).
pub fn under(settings: &Settings, locks: &crate::sandbox::policy::Locks, family: Family) -> Mode {
    if !family.switchable() {
        return Mode::Enforce;
    }
    let project = settings.mode(family);
    match locks.enforcement_floor(family) {
        Some(floor) => project.max(floor),
        None => project,
    }
}

/// **What the project at `root` says now**, under this machine's locks, before any acceptance.
pub fn committed(root: &Path) -> Resolved {
    Resolved::of(
        &Settings::read(root),
        &crate::sandbox::policy::Locks::of(root),
    )
}

/// [`committed`] for settings text, as a test names it.
#[cfg(test)]
pub(crate) fn committed_for_test(text: &str) -> Resolved {
    Resolved::of(
        &Settings::from_text(Some(text)),
        &crate::sandbox::policy::Locks::none(),
    )
}

/// The policy in force where the project says `committed` and this person `accepted` what they
/// did, each family the acceptance alone enforces marked as unaccepted, as a test names it.
#[cfg(test)]
pub(crate) fn in_force_for_test(committed: &Resolved, accepted: &Resolved) -> Resolved {
    Resolved::accepting(committed, accepted)
}

/// **What this person last accepted** for the project at `root`: the shipped default, `watch`,
/// where nothing was ever kept (the upgrade announces it once); **`strict`** where one was kept
/// and what is here now is not it (the review of #6992): removed, edited, an older copy put back,
/// or a process that cannot verify it.
///
/// **The team's off is the sandbox family's press** (the follow-up's item 1): the person's answer
/// to a committed `[sandbox] mode = "off"` ([`crate::sandbox::team::answer`]) is what they
/// accepted of the level while it is kept: Accept is no sandbox, only for an off the project
/// committed and still says, and Keep it on for me never below the floor. A broken acceptance is
/// never loosened by it, and runs the full sandbox.
pub fn accepted(root: &Path) -> Resolved {
    accepted_of(root, crate::sandbox::local::policy_accepted_record(root))
}

/// **What the shipped default accepts** ([`accepted`] where nothing was ever kept): `watch`,
/// with no lock of this machine's. What a committed policy is weighed against where nothing
/// was kept, and where what was kept is broken (ADR 0130 §3).
pub fn shipped() -> Resolved {
    Resolved::of(&Settings::default(), &crate::sandbox::policy::Locks::none())
}

/// [`accepted`] for the project at `root`, of the acceptance `record` read for it once.
fn accepted_of(root: &Path, record: crate::sandbox::local::PolicyAccepted) -> Resolved {
    use crate::sandbox::local::PolicyAccepted;
    let kept = match record {
        PolicyAccepted::Held(kept) => Resolved::of_kept(&kept),
        PolicyAccepted::Never => shipped(),
        // Every family enforced, and the full sandbox: never the floor, which would loosen a
        // full sandbox that was kept (the review of #7105).
        PolicyAccepted::Broken => return broken_accepted(),
    };
    let level = kept.level();
    // The answer is kept unsealed, so it lowers the sealed level only for an off the project
    // committed and still says, which this person accepted ([`crate::sandbox::team::team_off`]);
    // Keep it on for me only ever raises it (the review of #7105).
    use crate::sandbox::team::{TeamOff, team_off};
    if matches!(team_off(root), TeamOff::Accepted(_)) {
        return kept.at_level(SandboxLevel::Off);
    }
    match crate::sandbox::local::team_off_answer(root) {
        Some(answer) if !answer.accepted => kept.at_level(level.max(SandboxLevel::Floor)),
        _ => kept,
    }
}

/// **What a broken acceptance counts as** ([`accepted`]): `strict`, every family enforced and
/// every dispatch across personas denied ([`Resolved::strict`]), at the full sandbox.
pub fn broken_accepted() -> Resolved {
    Resolved::strict().at_level(SandboxLevel::Full)
}

/// **Whether what this person accepted was kept and is not here now** (the M75 review): removed,
/// edited, an older copy put back, or a `.git` moved so the clone looks new. Then every family is
/// enforced, every dispatch across personas is refused, and the window offers the press again.
/// Only the app can tell.
pub fn broken(root: &Path) -> bool {
    in_the_app()
        && crate::sandbox::local::policy_accepted_record(root)
            == crate::sandbox::local::PolicyAccepted::Broken
}

/// **How a chat starting in the project at `root` runs** (Q308a): [`in_force`], which in the app
/// keeps every tightening it sees as accepted.
pub fn at_start(root: &Path) -> Resolved {
    in_force(root)
}

/// **The app's view**: the stricter of what the project says now and what this person accepted,
/// and **kept as accepted wherever it is stricter**, so a tightening holds until the person
/// presses to loosen it, through a rolled-back `HEAD` or a broken file alike. Best effort: an
/// acceptance that cannot be kept changes nothing this answers.
///
/// Resolved once per plane for one answer ([`resolved_once`]), never longer.
fn resolved_in_the_app(root: &Path) -> Resolved {
    if let Some(kept) =
        RESOLVED_ONCE.with_borrow(|kept| kept.as_ref().and_then(|kept| kept.get(root).cloned()))
    {
        return kept;
    }
    let resolved = resolve_in_the_app(root);
    RESOLVED_ONCE.with_borrow_mut(|kept| {
        if let Some(kept) = kept {
            kept.insert(root.to_path_buf(), resolved.clone());
        }
    });
    resolved
}

/// Each project's policy as one answer resolved it in the app, by the plane; `None` outside one
/// ([`resolved_once`]).
type ResolvedOnce = Option<std::collections::HashMap<PathBuf, Resolved>>;

thread_local! {
    static RESOLVED_ONCE: std::cell::RefCell<ResolvedOnce> =
        const { std::cell::RefCell::new(None) };
}

/// **`work`, with each project's policy resolved once** (H20): every [`in_force`] (and every
/// other reader of [`resolved_in_the_app`]) in it gets what the first resolution found. A
/// resolution runs git twice, reads the locks and verifies the sealed acceptance; the new-chat
/// picker asked it once per persona, 44 s at 38 personas on a loaded Mac.
///
/// **For one answer, never longer**, as [`crate::personas::read_once`] (which opens it) is: the
/// project's committed policy and the person's acceptance change, so what was resolved is
/// dropped when `work` returns. The same strictness: a `HEAD` git could not read is strict for
/// the whole answer and kept for none, and a tightening is kept as accepted by the first
/// resolution, as it was before. A press in it is never answered from before the press
/// ([`press`]).
pub fn resolved_once<T>(work: impl FnOnce() -> T) -> T {
    /// Drops what was resolved when the outermost answer ends, however it ends.
    struct Answering(bool);
    impl Drop for Answering {
        fn drop(&mut self) {
            if self.0 {
                RESOLVED_ONCE.with_borrow_mut(|kept| *kept = None);
            }
        }
    }
    let _answering = Answering(RESOLVED_ONCE.with_borrow_mut(|kept| {
        let outermost = kept.is_none();
        if outermost {
            *kept = Some(std::collections::HashMap::new());
        }
        outermost
    }));
    work()
}

/// [`resolved_in_the_app`], resolved now.
fn resolve_in_the_app(root: &Path) -> Resolved {
    #[cfg(test)]
    RESOLUTIONS.with(|count| count.set(count.get() + 1));
    let accepted = accepted(root);
    let settings = Settings::read(root);
    let resolved = Resolved::accepting(
        &Resolved::of(&settings, &crate::sandbox::policy::Locks::of(root)),
        &accepted,
    );
    // A `HEAD` git could not read is strict for now and never kept (#7073): a git that timed out
    // once is no tightening the project made, and would need a press to undo.
    if resolved != accepted
        && !settings.unread
        && let Err(why) = crate::sandbox::local::accept_policy(root, resolved.as_kept())
    {
        tracing::warn!("purlis: the accepted policy was not kept ({why})");
    }
    resolved
}

/// **What the person would accept with a press** (Q308a): each family the project now runs
/// looser than they accepted, with the accepted mode and the project's. Empty where nothing
/// waits.
pub fn waiting(root: &Path) -> Vec<(Family, Mode, Mode)> {
    committed(root).looser_than(&accepted(root))
}

/// **The sandbox level the person would accept with a press**: the level they accepted and the
/// project's, where the project's is looser. None where nothing waits.
pub fn level_waiting(root: &Path) -> Option<(SandboxLevel, SandboxLevel)> {
    committed(root).level_looser_than(&accepted(root))
}

/// **The person's press, in the window**: they accept what the project says now, looser where
/// it is looser, sealed by the app. Never a chat's: no sandboxed chat writes the file it is kept
/// in, and only the app can seal it. One press, on the committed file as it stands; never a
/// re-seal of an acceptance something else wrote.
///
/// `HEAD` is read once: what is digested is what is accepted.
pub fn press(root: &Path, shown: &str) -> std::io::Result<()> {
    let now = committed(root);
    if digest(&now) != shown {
        return Err(changed_since_shown());
    }
    crate::sandbox::local::accept_policy(root, now.as_kept())?;
    RESOLVED_ONCE.with_borrow_mut(|kept| {
        if let Some(kept) = kept {
            kept.remove(root);
        }
    });
    // One press for the level (the follow-up's item 1): where it accepts the team's off, the
    // team's own ask is answered with it, so nobody is asked twice.
    if now.level() == SandboxLevel::Off
        && crate::sandbox::team::Team::of(root).off
        && let Ok(answer) = crate::sandbox::team::answering(root, true)
    {
        crate::sandbox::local::answer_team_off(root, answer)?;
    }
    Ok(())
}

/// **The person's press where the seal's key is another program's: make a new key and accept**
/// (#7714), the window's alone, on the plane it names. A view already stale asks nothing of
/// macOS. Then the old items are replaced and a new key made
/// ([`crate::sandbox::local::renew_seal_key`]), which macOS may ask the person to allow, and
/// **the digest is checked again after that prompt returns**: a file that changed while it was
/// open accepts nothing, with the new key kept. Then this plane's acceptance is sealed from what
/// the person was shown, as [`press`] seals it. Every other plane whose acceptance was sealed
/// with the old key reads as broken because of the new key ([`Standing::new_key`]), never as
/// never accepted. Where the key is the app's own, nothing is replaced and it is [`press`].
pub fn press_with_new_key(root: &Path, shown: &str) -> std::io::Result<()> {
    if digest(&committed(root)) != shown {
        return Err(changed_since_shown());
    }
    crate::sandbox::local::renew_seal_key()?;
    press(root, shown)
}

/// What a press says where the project's file changed since it was shown.
fn changed_since_shown() -> std::io::Error {
    std::io::Error::other(
        "the project's policy changed since it was shown, so nothing was accepted; look at it \
         again",
    )
}

/// **What the window says about the project's `[policy]` on this machine** (Q308a, Q308b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    /// Each family the project now runs looser than this person accepted: the family, the
    /// accepted mode and the project's. What the press would accept.
    pub waiting: Vec<(Family, Mode, Mode)>,
    /// The sandbox level this person accepted and the project's, where the project's is looser
    /// ([`level_waiting`]).
    pub level: Option<(SandboxLevel, SandboxLevel)>,
    /// **The families that mean nothing without a wrap** (Q308a): `network`, `ports` and
    /// `processes`, where the project's level is no sandbox, as it runs or as a press would make
    /// it run. Said, so a project never reads as enforcing them there.
    pub not_in_force: Vec<Family>,
    /// Each dispatch deny this person accepted that the project's committed file no longer
    /// holds, as [`crate::dispatchgrant::Denied::spelled`] spells one: still denied until the
    /// press, which would lift it (the review of #7113).
    pub lifted: Vec<String>,
    /// Whether the project now allows lineage where this person accepted a denial of it: what the
    /// press would lift.
    pub lineage_waiting: bool,
    /// Each protected branch this person accepted that the project's committed file no longer
    /// names: still protected until the press, which would lift it.
    pub protection_lifted: Vec<String>,
    /// What the project's `[policy]` holds that purlis could not read, each enforced.
    pub refused: Vec<String>,
    /// Whether an acceptance is kept here whose seal does not hold: it counts as none.
    pub unsealed: bool,
    /// **Whether this machine's acceptance is Broken** ([`crate::sandbox::local::PolicyAccepted`]),
    /// of the same read of the record as everything else here: then [`Self::runs`] is strict
    /// mode, and what waits is weighed against it.
    pub broken: bool,
    /// **Whether the committed policy is looser than the shipped `watch`** anywhere
    /// ([`Resolved::loosens`] against [`shipped`]), from the same read of `HEAD`: under Broken,
    /// what alone says the policy is Waiting too, with no read of the record that does not hold
    /// (ADR 0130 §3).
    pub loosens_the_shipped: bool,
    /// Whether the seal's key on this machine is another program's (#7714), whoever made it:
    /// what happens after an update macOS treats as a different app. Then nothing holds, and
    /// the press is [`press_with_new_key`].
    pub key_foreign: bool,
    /// Whether this plane's acceptance broke because purlis replaced its seal key on this
    /// machine since it was sealed (#7714), never because something edited it
    /// ([`crate::sandbox::local::broken_by_a_new_key`]): enforced as broken all the same.
    pub new_key: bool,
    /// Whether purlis vouches that this plane's nevers were anchored by a seal key it has
    /// replaced since (#7714, F3 of #7734's review; H2 of its round 3), and not sealed again:
    /// they read as unread until the person keeps the pairs the window shows
    /// ([`crate::dispatchnever::keep_shown_after_a_new_key`]). Accept never seals them.
    pub nevers_new_key: bool,
    /// The pairs that press keeps, and the digest it names (H1 of #7734's round 3): where
    /// `nevers_new_key` and the file reads.
    pub nevers_to_keep: Option<crate::dispatchnever::ToKeep>,
    /// Whether that list holds a name purlis cannot show faithfully (M1 of #7734's round 4):
    /// then no pair is offered to keep or to seal, and the window says why
    /// ([`crate::dispatchnever::holds_a_name_it_cannot_show`]).
    pub nevers_unshowable: bool,
    /// Whether this plane's nevers stay unread on a list of a replaced seal key that purlis
    /// cannot vouch for, and no keychain item can be named to delete (#7852): the window offers
    /// Seal these nevers ([`crate::dispatchnever::seal_shown_unvouched`]), which also clears the
    /// project's dispatch grants on this machine.
    pub nevers_unvouched: bool,
    /// The pairs Seal these nevers seals, and the digest it names: where `nevers_unvouched` and
    /// the file reads.
    pub nevers_to_seal: Option<crate::dispatchnever::ToKeep>,
    /// The digest of the committed text this was read from, which the press must name.
    pub digest: String,
    /// What the project's `[policy]` says, from the same read: its template and the families
    /// it sets apart.
    pub said: Settings,
    /// How every family runs here now: the stricter of what the project says and what this
    /// person accepted, each enforced where the acceptance's seal does not hold.
    pub runs: Resolved,
    /// Each family an administrator's lock holds enforced on this machine.
    pub locked: Vec<Family>,
}

impl Standing {
    /// **Whether anything waits for the press**: a family, the sandbox's level, a deny, lineage
    /// or a protected branch the project now runs looser than this person accepted.
    pub fn waits(&self) -> bool {
        !self.waiting.is_empty()
            || self.level.is_some()
            || !self.lifted.is_empty()
            || self.lineage_waiting
            || !self.protection_lifted.is_empty()
    }
}

/// The families that mean nothing at `level` ([`Standing::not_in_force`]): `network`, `ports`
/// and `processes` where it is no sandbox, and none otherwise.
pub fn not_in_force_at(level: SandboxLevel) -> Vec<Family> {
    if level == SandboxLevel::Off {
        vec![Family::Network, Family::Ports, Family::Processes]
    } else {
        Vec::new()
    }
}

/// [`Standing`] for the project at `root`.
pub fn standing(root: &Path) -> Standing {
    // One read of `HEAD`, so what waits, what is said and the digest the press must name are
    // all of the same file.
    let settings = Settings::read(root);
    let locks = crate::sandbox::policy::Locks::of(root);
    let now = Resolved::of(&settings, &locks);
    let record = crate::sandbox::local::policy_accepted_record(root);
    let broken = record == crate::sandbox::local::PolicyAccepted::Broken;
    let accepted = accepted_of(root, record);
    Standing {
        broken,
        loosens_the_shipped: now.loosens(&shipped()),
        waiting: now.looser_than(&accepted),
        level: now.level_looser_than(&accepted),
        not_in_force: not_in_force_at(now.level()),
        lifted: now.lifts(&accepted),
        lineage_waiting: now.lineage_looser_than(&accepted),
        protection_lifted: now.protection_lifted(&accepted),
        refused: settings.refused.clone(),
        // Of the same read of the record: `policy_unsealed` is whether it is Broken.
        unsealed: broken,
        key_foreign: crate::secrets::seal::key_is_foreign(),
        new_key: crate::sandbox::local::broken_by_a_new_key(root),
        nevers_new_key: crate::dispatchnever::vouched_for_a_new_key(root),
        nevers_to_keep: crate::dispatchnever::to_keep_after_a_new_key(root),
        nevers_unshowable: crate::dispatchnever::holds_a_name_it_cannot_show(root),
        nevers_unvouched: crate::dispatchnever::unvouched_for_a_new_key(root),
        nevers_to_seal: crate::dispatchnever::to_seal_unvouched(root),
        digest: digest(&now),
        runs: now.stricter(&accepted),
        locked: Family::ALL
            .into_iter()
            .filter(|family| locks.enforcement_floor(*family).is_some())
            .collect(),
        said: settings,
    }
}

#[cfg(test)]
thread_local! {
    /// A test's stand-in for "this process is purlis's app": true unless the test says not.
    static IN_THE_APP: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    /// A test's stand-in for what the app answered this process ([`answered_by_the_app`]).
    static ANSWERED: std::cell::RefCell<Option<Resolved>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
thread_local! {
    /// How many times this thread has resolved a project's policy in the app (git, the locks
    /// and the sealed acceptance), for a test of an answer that resolves it once.
    pub(crate) static RESOLUTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// **A test's process is not the app** (or is again) for this thread.
#[cfg(test)]
pub(crate) fn set_in_the_app_for_this_test(in_the_app: bool) {
    IN_THE_APP.with(|held| held.set(in_the_app));
    ANSWERED.with(|held| *held.borrow_mut() = None);
}

#[cfg(not(test))]
static ANSWERED: std::sync::Mutex<Option<Option<Resolved>>> = std::sync::Mutex::new(None);

/// How a process that is not the app asks the app for its chat's policy ([`ask_with`]).
pub type Asker = fn() -> Option<Resolved>;

static ASKER: std::sync::OnceLock<Asker> = std::sync::OnceLock::new();

/// **How this process asks the app for its chat's policy** (the CLI's, over the hook socket):
/// asked once, the first time a family's mode is needed. Where none is set, or the app does not
/// answer, every family is enforced.
pub fn ask_with(asker: Asker) {
    let _ = ASKER.set(asker);
}

/// Whether this process is purlis's app: the one process that can verify what the person
/// accepted ([`crate::secrets::seal`]).
pub fn in_the_app() -> bool {
    #[cfg(test)]
    {
        IN_THE_APP.with(std::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        crate::secrets::keyhold::is_the_app()
    }
}

/// **What the app answered this process about its chat's policy** ([`crate::hookwire::Ask::Policy`]),
/// kept for the process's life: a hook, a git hook or `purlis secret exec` asks once, first.
/// `None` where nobody answered, which leaves every family enforced.
pub fn answered_by_the_app(resolved: Option<Resolved>) {
    #[cfg(test)]
    ANSWERED.with(|held| *held.borrow_mut() = resolved);
    #[cfg(not(test))]
    if let Ok(mut held) = ANSWERED.lock() {
        *held = Some(resolved);
    }
}

#[cfg(all(not(test), feature = "test-support"))]
thread_local! {
    /// Another crate's test's stand-in for what the app answered, on its own thread alone.
    static ANSWERED_HERE: std::cell::RefCell<Option<Resolved>> =
        const { std::cell::RefCell::new(None) };
}

/// **What the app answered, for this thread of another crate's test alone** (#7153): so a test
/// of the app's own code can run a family `off` without changing what the tests beside it read.
/// `None` stands for no answer kept on this thread. Only tests turn `test-support` on.
#[cfg(all(not(test), feature = "test-support"))]
pub fn answered_on_this_thread(resolved: Option<Resolved>) {
    ANSWERED_HERE.with(|held| *held.borrow_mut() = resolved);
}

fn answered() -> Option<Resolved> {
    #[cfg(test)]
    {
        ANSWERED.with(|held| held.borrow().clone())
    }
    #[cfg(not(test))]
    {
        #[cfg(feature = "test-support")]
        if let Some(here) = ANSWERED_HERE.with(|held| held.borrow().clone()) {
            return Some(here);
        }
        let Ok(mut held) = ANSWERED.lock() else {
            return None;
        };
        held.get_or_insert_with(|| ASKER.get().and_then(|ask| ask()))
            .clone()
    }
}

/// **How every family runs here, for the project at `root`** (Q308a, and the review of #6992):
///
/// - **In the app**, which alone can verify what the person accepted: the stricter of what the
///   plane root's committed file says now and what this person accepted. A press reaches every
///   chat at once, through the app's answers.
/// - **Anywhere else** (a hook, a git hook, `purlis secret exec`): what the app answered this
///   process ([`answered_by_the_app`]), or **`strict`** where it did not answer. `root` is never
///   read there: a process a chat starts can name any folder as its plane, so nothing it names
///   can loosen anything. [`ENV`], the start's resolution, can only tighten.
pub fn in_force(root: &Path) -> Resolved {
    if in_the_app() {
        return resolved_in_the_app(root);
    }
    let asked = answered().unwrap_or_else(|| Resolved::all(Mode::Enforce));
    match crate::envvar::var(ENV).filter(|said| !said.is_empty()) {
        Some(started) => asked.stricter(&Resolved::parse(&started)),
        None => asked,
    }
}

/// **The sandbox level a chat starting in the project at `root` runs at** before a person's own
/// level (the follow-up's item 1): what drives [`crate::sandbox::team::allowed_levels`]'s
/// default, and so [`crate::sandbox::decide_granted`].
///
/// - **In the app**: [`in_force`]'s level, the stricter of what the project's committed file
///   sets (its `[sandbox] mode`, else its template's level) and what this person accepted.
/// - **Anywhere else**: never looser than what the project's file says as purlis read it before
///   Q308 (`file`, [`crate::sandbox::team::as_the_file_says`]), raised by the app's answer or
///   the start's [`ENV`], which can only tighten.
pub fn sandbox_level(root: &Path, file: impl FnOnce() -> SandboxLevel) -> SandboxLevel {
    let resolved = in_force(root).level();
    if in_the_app() {
        resolved
    } else {
        resolved.max(file())
    }
}

/// **The dispatch denies in force for the project at `root`** (`[dispatch.denies]`, ticket
/// T1.1-73, and the review of #7113), read with `[policy]`:
///
/// - **In the app**: the denies of [`in_force`], so from the committed file at `HEAD`, never the
///   working tree's, and never fewer than this person accepted. A deny the project takes away is
///   a loosening that waits for the press ([`press`]); one it adds holds at once. A file that
///   does not read, or a `HEAD` git cannot read, denies every dispatch across personas.
/// - **Anywhere else**: the committed file's at `HEAD`, as unreadable as strictly. No other
///   process judges a dispatch for a chat.
pub fn denies_in_force(root: &Path) -> crate::dispatchgrant::Denied {
    denies_in_force_and_unaccepted(root).0
}

/// [`denies_in_force`], and of those the denies held only because this machine's acceptance is
/// Broken or Waiting ([`Resolved::unaccepted_denies`]), from the same read. None anywhere but
/// in the app, which alone verifies the acceptance.
pub fn denies_in_force_and_unaccepted(
    root: &Path,
) -> (crate::dispatchgrant::Denied, crate::dispatchgrant::Denied) {
    if in_the_app() {
        let resolved = resolved_in_the_app(root);
        (resolved.denies, resolved.unaccepted_denies)
    } else {
        (
            Settings::read(root).denies,
            crate::dispatchgrant::Denied::default(),
        )
    }
}

/// **Whether a parent chat may take up its ended child's cut branch through the forge route, for
/// the project at `root`** (`[policy] lineage`, [`LINEAGE_KEY`]), an allowance whose strict value
/// is the denial:
///
/// - **In the app**: [`in_force`]'s, so from the committed file at `HEAD`, never the working
///   tree's, and never looser than this person accepted. A denial the project adds, or an
///   administrator's lock (`sandbox.lineage: false`), holds at once; allowing it again waits for
///   the press ([`press`]). A value that does not read, a file that is not TOML and a `HEAD` git
///   cannot read all deny it.
/// - **Anywhere else**: the committed file's at `HEAD` and the locks, as unreadable as strictly.
///   Only the app's route asks.
pub fn lineage_in_force(root: &Path) -> bool {
    if in_the_app() {
        resolved_in_the_app(root).lineage
    } else {
        !Settings::read(root).lineage_denied
            && !crate::sandbox::policy::Locks::of(root).forbids_lineage()
    }
}

/// **The protected branches in force for the project at `root`** (`[risk] protected_branches`,
/// names or patterns with `*`), read as `[policy]` is:
///
/// - **In the app**: the committed file's at `HEAD`, never the working tree's, **and never fewer
///   than this person accepted**, so a chat that edits the file without committing it, or breaks
///   it, loses nothing. A name the project drops waits for the press ([`press`]); one it adds
///   holds at once.
/// - **Anywhere else**: the committed file's at `HEAD`, with no accepted copy to floor it.
pub fn protected_branches_in_force(root: &Path) -> Vec<String> {
    if in_the_app() {
        resolved_in_the_app(root).protected_branches()
    } else {
        Settings::read(root).protected
    }
}

/// [`in_force`], for one family.
pub fn mode(root: &Path, family: Family) -> Mode {
    in_force(root).mode(family)
}

/// **What a choke point does with a refusal of `family`** under `mode`: `Some(refusal)` where it
/// refuses, `None` where it allows. A logged one is recorded first, for chat `chat` in the
/// project at `root`, by its `rule` and `host` alone.
pub fn judged<T>(
    root: &Path,
    chat: Option<&str>,
    family: Family,
    mode: Mode,
    rule: &str,
    host: Option<&str>,
    refusal: T,
) -> Option<T> {
    match mode {
        Mode::Enforce => Some(refusal),
        Mode::Log => {
            record(root, chat.unwrap_or(UNKNOWN_CHAT), family, rule, host);
            None
        }
        Mode::Off => None,
    }
}

/// The folder of the enforcement logs, in the app's own state, which every wrapped chat is
/// denied writing.
pub fn log_dir(root: &Path) -> PathBuf {
    crate::names::state(root).join("app").join("enforcement")
}

/// Chat `chat`'s enforcement log (its ULID, ADR 0066), or `None` where `chat` cannot name one
/// file there.
pub fn log_file(root: &Path, chat: &str) -> Option<PathBuf> {
    crate::contain::mintable(chat)
        .is_ok()
        .then(|| log_dir(root).join(format!("{chat}.jsonl")))
}

/// One line of a chat's enforcement log: the rule, the family, the chat and the host, and never
/// a command, a path or a URL (Q308a).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Logged {
    /// When, UTC, RFC 3339.
    pub at: String,
    /// The id of the rule that would have refused: a guard's arm, a scan's rule.
    pub rule: String,
    /// The family's key.
    pub family: String,
    /// The chat's id.
    pub chat: String,
    /// The host it was about, where the rule is about one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

/// The most of a rule id or a host a log line keeps, in characters.
pub const MOST_CHARS: usize = 200;

/// What a log line names as its chat where the process was not told which (Q308 review, S3).
pub const UNKNOWN_CHAT: &str = "unknown";

/// The most lines one chat's log keeps (the re-review of #6992, S3): past it, a line is dropped.
pub const MOST_LINES: usize = 10_000;

/// The rule id of a host ask answered under `asks` = off or log.
pub const RULE_HOST_ASK: &str = "host-ask";
/// The rule id of a folder or socket ask answered under `asks` = off or log (the Q308
/// follow-up, item 3).
pub const RULE_SANDBOX_ASK: &str = "sandbox-ask";
/// The rule id of a host nothing lists carried under `network` = off or log (the Q308
/// follow-up, item 5).
pub const RULE_NETWORK_UNLISTED: &str = "network-unlisted-host";
/// The rule id of the Read guard on a vault file.
pub const RULE_VAULT_READ: &str = "A-read";
/// The rule id of a vault value masked in a chat.
pub const RULE_MASK: &str = "mask";
/// The rule id of the merge hold.
pub const RULE_MERGE_HOLD: &str = "merge-hold";
/// The rule id of the guard on a branch folder another chat holds.
pub const RULE_BRANCH_FOLDER: &str = "branch-folder";
/// The rule id of `forge_milestones`' bound to the chat's own workspace (`guards.forge`).
pub const RULE_FORGE_WORKSPACE: &str = "forge.milestones.workspace";
/// The rule ids of a forge route refusal (#7779; ADR 0128, amended 2026-11-27, §10), by its kind
/// and act: a request purlis doesn't carry, a floor act (also no action), and a known act the
/// policy doesn't allow. Each is counted under "Would have blocked".
pub const RULE_FORGE_REFUSALS: &[&str] = &[
    RULE_FORGE_NO_ACTION,
    RULE_FORGE_ADMINISTRATION,
    RULE_FORGE_RULE,
    RULE_FORGE_APPROVAL,
    RULE_FORGE_MERGE,
    RULE_FORGE_RELEASE_OR_TAG,
    RULE_FORGE_FORK,
];
/// A forge request purlis doesn't carry.
pub const RULE_FORGE_NO_ACTION: &str = "forge.no-action";
/// A floor act: the repo's administration.
pub const RULE_FORGE_ADMINISTRATION: &str = "forge.no-action.administration";
/// A floor act: a rule over the repo (a ruleset or a branch protection).
pub const RULE_FORGE_RULE: &str = "forge.no-action.rule";
/// A floor act: an approval or a review.
pub const RULE_FORGE_APPROVAL: &str = "forge.no-action.approval";
/// A known act the policy doesn't allow: a merge.
pub const RULE_FORGE_MERGE: &str = "forge.not-allowed.merge";
/// A known act the policy doesn't allow: a release or a tag.
pub const RULE_FORGE_RELEASE_OR_TAG: &str = "forge.not-allowed.release-or-tag";
/// A known act the policy doesn't allow: a fork.
pub const RULE_FORGE_FORK: &str = "forge.not-allowed.fork";
/// The rule id of a dispatched chat on a profile that switches the prompts off (`mode`).
pub const RULE_ASKS_NOBODY: &str = "dispatch.prompts-off";
/// The rule id of a persona's deny-list its harness cannot carry (`tools`).
pub const RULE_DENY_LIST: &str = "persona.disallowed-tools";
/// The rule id of a profile's command nobody approved here (`trust`).
pub const RULE_PROFILE_APPROVAL: &str = "profile.approval";
/// The rule id of a harness declaration nobody approved here (`trust`).
pub const RULE_DECLARATION_APPROVAL: &str = "declaration.approval";
/// The rule id of a runner chat's dispatch held for the person (`runners`).
pub const RULE_RUNNER_DISPATCH: &str = "runner.dispatch";
/// The rule id of a runner chat's write held for the person (`runners`).
pub const RULE_RUNNER_WRITE: &str = "runner.write";
/// The rule id of a window-only refusal raised as an ask (`consents`).
pub const RULE_WINDOW_ONLY_ASK: &str = "window-only.ask";
/// The rule id of the brokered commit's rate (`limits`).
pub const RULE_COMMIT_RATE: &str = "commit.rate";
/// The prefix of a dispatch limit's rule id, before the limit's word (`limits`).
pub const RULE_DISPATCH_LIMIT: &str = "dispatch.";

/// **Whether `rule` is one of purlis's rules of `family`** (the re-review of #6992, S3): an arm
/// of the Bash guard, a rule of the commit and push scan, or one of the rules named above. What
/// the app takes from a line a chat's process hands over; any other is refused.
pub fn known_rule(family: Family, rule: &str) -> bool {
    if let Some(arm) = crate::toolgate::Arm::of_id(rule) {
        return arm.family() == Some(family)
            || (arm == crate::toolgate::Arm::HookSkip && family == Family::GuardsCredential);
    }
    let named = match rule {
        RULE_HOST_ASK | RULE_SANDBOX_ASK => Some(Family::Asks),
        RULE_NETWORK_UNLISTED => Some(Family::Network),
        RULE_VAULT_READ | RULE_MASK => Some(Family::Secrets),
        RULE_MERGE_HOLD => Some(Family::GuardsMerge),
        RULE_BRANCH_FOLDER => Some(Family::GuardsGit),
        RULE_FORGE_WORKSPACE => Some(Family::GuardsForge),
        _ if RULE_FORGE_REFUSALS.contains(&rule) => Some(Family::GuardsForge),
        RULE_ASKS_NOBODY => Some(Family::HarnessMode),
        RULE_DENY_LIST => Some(Family::Tools),
        RULE_PROFILE_APPROVAL | RULE_DECLARATION_APPROVAL => Some(Family::Trust),
        RULE_RUNNER_DISPATCH | RULE_RUNNER_WRITE => Some(Family::Runners),
        RULE_WINDOW_ONLY_ASK => Some(Family::Consents),
        RULE_COMMIT_RATE => Some(Family::Limits),
        _ => rule
            .strip_prefix(RULE_DISPATCH_LIMIT)
            .and_then(crate::dispatchlimits::Limit::of_word)
            .map(|_| Family::Limits),
    };
    if let Some(named) = named {
        return named == family;
    }
    crate::secretshape::leak_rules()
        .iter()
        .any(|(id, _)| *id == rule)
        && crate::diffscan::family_of(rule) == family
}

/// **Whether `host` is a `host:port`**: a name or an address, and a port.
pub fn is_host_and_port(host: &str) -> bool {
    let Some((name, port)) = host.rsplit_once(':') else {
        return false;
    };
    let name = name
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .unwrap_or(name);
    !name.is_empty()
        && name.len() <= 253
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':'))
        && port.parse::<u16>().is_ok_and(|port| port != 0)
}

/// Where a process that is not the app sends its log lines: the app, over the hook socket
/// ([`crate::hookwire::Ask::PolicyLog`]), since a process inside the wrap cannot write the log.
pub type Sink = Box<dyn Fn(Family, &str, Option<&str>) + Send + Sync>;

static SINK: std::sync::OnceLock<Sink> = std::sync::OnceLock::new();

/// **Send this process's log lines to `sink`** (the CLI's hand-over to the app). Once.
pub fn log_to(sink: Sink) {
    let _ = SINK.set(sink);
}

/// **One quiet line for chat `chat`'s enforcement log**: `family`'s `rule` would have refused,
/// about `host` where there is one. Written by the app; any other process hands it to the app
/// ([`log_to`]), and keeps nothing where it cannot. Best effort: a line that cannot be written
/// changes no outcome. Never a Notice and never an ask.
pub fn record(root: &Path, chat: &str, family: Family, rule: &str, host: Option<&str>) {
    if !in_the_app() {
        if let Some(sink) = SINK.get() {
            sink(family, rule, host);
        }
        return;
    }
    write_line(root, chat, family, rule, host);
}

/// The app's write of one log line ([`record`]).
pub fn write_line(root: &Path, chat: &str, family: Family, rule: &str, host: Option<&str>) {
    let Some(path) = log_file(root, chat) else {
        return;
    };
    let line = Logged {
        at: jiff::Timestamp::now()
            .strftime("%Y-%m-%dT%H:%M:%SZ")
            .to_string(),
        rule: crate::shown::one_line(rule, MOST_CHARS),
        family: family.key().to_owned(),
        chat: chat.to_owned(),
        host: host.map(|host| crate::shown::one_line(host, MOST_CHARS)),
    };
    if let Err(why) = append(root, &path, &line) {
        tracing::warn!(
            "purlis: an enforcement log line for {} was not written ({why})",
            family.key()
        );
    }
}

fn append(root: &Path, path: &Path, line: &Logged) -> std::io::Result<()> {
    use std::io::Write;
    let dir = path.parent().unwrap_or(root);
    crate::contain::no_link_on_the_way(root, dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(dir)?;
    crate::contain::no_link_on_the_way(root, path)?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true).append(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = crate::contain::nofollow(&mut options).open(path)?;
    // One writer at a time, so the count kept is the file's.
    let mut counts = COUNTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let now = Counted::of(&file.metadata()?, 0);
    let lines = match counts.get(path) {
        Some(kept) if kept.same_file(&now) => kept.lines,
        _ => lines_in(&mut file)?,
    };
    if lines >= MOST_LINES {
        counts.insert(path.to_path_buf(), Counted { lines, ..now });
        return Ok(());
    }
    let mut text = serde_json::to_string(line).map_err(std::io::Error::other)?;
    text.push('\n');
    file.write_all(text.as_bytes())?;
    counts.insert(
        path.to_path_buf(),
        Counted::of(&file.metadata()?, lines + 1),
    );
    Ok(())
}

/// **How many lines each log held after the app's last write to it** (#7073), so a line is not
/// a read of the whole file: counted again only where the file is not as the app left it
/// (another file in its place, or a length the app did not write).
static COUNTS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<PathBuf, Counted>>> =
    std::sync::LazyLock::new(Default::default);

/// One log as the app last left it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Counted {
    /// Which file it is: its device and inode where the platform has them.
    file: (u64, u64),
    /// Its length in bytes.
    len: u64,
    /// The lines it holds.
    lines: usize,
}

impl Counted {
    fn of(meta: &std::fs::Metadata, lines: usize) -> Self {
        #[cfg(unix)]
        let file = {
            use std::os::unix::fs::MetadataExt;
            (meta.dev(), meta.ino())
        };
        #[cfg(not(unix))]
        let file = (0, 0);
        Self {
            file,
            len: meta.len(),
            lines,
        }
    }

    /// Whether `now` is this very file at the length the app left it.
    fn same_file(&self, now: &Self) -> bool {
        self.file == now.file && self.len == now.len
    }
}

#[cfg(test)]
thread_local! {
    /// How many times this thread read a log whole to count its lines.
    static COUNTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// **How many times this thread read a log whole** to count its lines.
#[cfg(test)]
fn counted_whole() -> usize {
    COUNTED.with(std::cell::Cell::get)
}

/// How many lines `file` holds, read from its start.
fn lines_in(file: &mut std::fs::File) -> std::io::Result<usize> {
    use std::io::{BufRead, Seek};
    #[cfg(test)]
    COUNTED.with(|counted| counted.set(counted.get() + 1));
    file.seek(std::io::SeekFrom::Start(0))?;
    let mut lines = 0;
    let mut reader = std::io::BufReader::new(&*file);
    let mut buf = Vec::new();
    while reader.read_until(b'\n', &mut buf)? > 0 {
        lines += 1;
        buf.clear();
    }
    Ok(lines)
}

/// Chat `chat`'s enforcement log, oldest first: what the window's count and a test read. Lines
/// that do not read are left out.
pub fn read_log(root: &Path, chat: &str) -> Vec<Logged> {
    let Some(path) = log_file(root, chat) else {
        return Vec::new();
    };
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// The chat this process runs for, by the id the app started its harness with
/// ([`crate::hookwire::CHAT_ID_ENV`]): none for a harness the app did not start.
pub fn this_chat() -> Option<String> {
    crate::envvar::var(crate::hookwire::CHAT_ID_ENV).filter(|chat| !chat.is_empty())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod unaccepted_tests;
