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
//!   Notice, no ask); one quiet would-have-blocked event in the event log names the rule, the
//!   family, the chat and the host, and nothing else ([`record`]; ADR 0125's enforcement log,
//!   `app/enforcement/<chat>.jsonl`, is retired).
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
//! - It is **resolved at a chat's start** ([`at_start`]) and answered to the chat's hooks by the
//!   app over the hook socket ([`answered_by_the_app`]). `$PURLIS_POLICY`, which once carried it,
//!   is retired: nothing reads it.
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

use std::collections::BTreeMap;
use std::path::Path;

/// The table of the committed settings file that holds these keys.
pub const TABLE: &str = "policy";

/// The key naming the template.
pub const TEMPLATE_KEY: &str = "template";

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
    /// The record itself: its keys are limits only, and a policy can't turn it off or down to a
    /// log (Q313.4, Q316.8). Any mode but `enforce` reads as `enforce`.
    Audit,
}

impl Family {
    /// Every family, in the order the docs list them.
    pub const ALL: [Family; 26] = [
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
        Family::Audit,
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
            Family::Audit => "audit",
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
    /// is a level, not a mode, `audit` takes neither `off` nor `log` ([`Family::turns_off`]), and
    /// `ports` is the kernel's refusal of a local socket or a port of this machine, which tells
    /// nothing of a connection it let through, so it has no line to write (the Q308 follow-up,
    /// item 5): `log` there is enforced, and said.
    pub fn logs(self) -> bool {
        !matches!(
            self,
            Family::Sandbox
                | Family::Vaults
                | Family::Ports
                | Family::CredentialFiles
                | Family::Audit
        )
    }

    /// Whether the family takes `off`: every one but `audit`, which a policy can't turn off, the
    /// record being what the rest is judged by (Q313.4; M75 slice 10).
    pub fn turns_off(self) -> bool {
        self != Family::Audit
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
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
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
        // `audit` is the same in every template: a policy can't turn the record off or down.
        if family == Family::Audit {
            return Mode::Enforce;
        }
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
            return mode_of_level_under(self.sandbox_level(), self);
        }
        if mode == Mode::Log && !family.logs() {
            Mode::Enforce
        } else {
            mode
        }
    }

    /// **The sandbox level the template sets** (Q308a, Q316, M75-11): `[policy] sandbox` sets it
    /// apart from the template, and `strict`, which leaves the level to that key, is the floor
    /// where it names none (**proposed**; Q318 point 9).
    pub fn sandbox_level(self) -> SandboxLevel {
        match self {
            Template::Open => SandboxLevel::Off,
            Template::Watch | Template::Hold | Template::Strict => SandboxLevel::Floor,
            Template::Team => SandboxLevel::Full,
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

    /// The level's word, as the accepted record and the window spell it.
    pub fn word(self) -> &'static str {
        match self {
            SandboxLevel::Off => "off",
            SandboxLevel::Floor => "floor",
            SandboxLevel::Full => "full",
        }
    }

    /// **The level `[policy] sandbox` names** (M75-11): `off`, `floor` or `full`, and nothing
    /// else. `log` and `enforce` are modes of a family, not levels of the sandbox, so neither is
    /// read ([`Self::parse`] reads `enforce` only for an acceptance kept before the level).
    pub fn of_policy_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|level| level.word() == word)
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

/// **What the hooks do with the `read` and `write` forbids at `level`, under `template`**
/// (M75-11; Q311 point 8, Q320b R2-2): `full` and `floor` enforce them, and `watch` runs at the
/// floor. At the level `off` they follow the template's general mode: `off` under `open`, so
/// `open` stays "everything off", and `log` under any other template that sets the level `off`.
pub fn mode_of_level_under(level: SandboxLevel, template: Template) -> Mode {
    match (level, template) {
        (SandboxLevel::Off, Template::Open) => Mode::Off,
        (SandboxLevel::Off, _) => Mode::Log,
        _ => Mode::Enforce,
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
    /// **What `[policy] sandbox` itself says** (M75-11): the level the key names, which replaces
    /// `[sandbox] mode` and wins over it where a file has both. Apart from [`Self::sandbox`] so
    /// your own file's `[sandbox] mode` is never read as a tightening of the project's level.
    policy_sandbox: Option<SandboxLevel>,
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
}

impl Settings {
    /// What `text`, a settings file, says; the default where it is `None`, holds no `[policy]`,
    /// or is not TOML (that file's own reader says why).
    pub fn from_text(text: Option<&str>) -> Self {
        let mut said = Self::read_text(text);
        said.denies = crate::dispatchgrant::Denied::of_text(text);
        // **`[sandbox] policy = "never"` is a lock on the level** (M75-11): nobody runs below the
        // full sandbox, whatever `[policy] sandbox` or the template says, as the levels reader
        // raises it. The local file's own `never` raises only what is read here, never the
        // project's ([`Self::read`] takes the local file's `policy_sandbox` and template alone).
        if text.is_some_and(|text| {
            crate::sandbox::team::Team::of_text(text).policy == crate::sandbox::team::Policy::Never
        }) {
            said.sandbox = Some(SandboxLevel::Full);
        }
        // **Any value purlis cannot read means `strict`**, with the sentence for the Notice (the
        // review of #6992, S6): a file that is not TOML, a key that is no family, a mode or a
        // template that is none.
        if !said.refused.is_empty() {
            said.template = Template::Strict;
            said.template_said = true;
            said.overrides.clear();
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
                ..Self::default()
            };
        };
        let sandbox = SandboxLevel::of_sandbox_mode(&top);
        let Some(table) = top.get(TABLE).cloned() else {
            return Self {
                sandbox,
                ..Self::default()
            };
        };
        let mut said = Self {
            sandbox,
            ..Self::default()
        };
        let Some(table) = table.as_table() else {
            said.template = Template::Strict;
            said.template_said = true;
            said.refused.push(format!(
                "[{TABLE}] is not a table, so every family is enforced"
            ));
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
            // `[policy] sandbox` is the family's one switch (M75-11): a level, never a mode.
            match value.as_str().and_then(SandboxLevel::of_policy_word) {
                Some(level) => {
                    self.sandbox = Some(level);
                    self.policy_sandbox = Some(level);
                }
                None => {
                    self.sandbox = Some(SandboxLevel::Full);
                    self.policy_sandbox = Some(SandboxLevel::Full);
                    self.refused.push(format!(
                        "{TABLE}.sandbox takes off, floor or full (a level, never log or \
                         enforce), so the sandbox is full"
                    ));
                }
            }
            return;
        }
        match value.as_str().and_then(Mode::parse) {
            Some(mode) if !family.turns_off() && mode != Mode::Enforce => {
                self.overrides.insert(family, Mode::Enforce);
                self.refused.push(format!(
                    "{TABLE}.{key} takes enforce only, so {} is enforced",
                    mode.word()
                ));
            }
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
        let names: Vec<&'static str> = crate::names::PLANE_MANIFEST.spellings().collect();
        let mut project = match committed_text(root, &names) {
            Committed::Text { text, .. } => Self::from_text(Some(&text)),
            Committed::Absent => Self::from_text(None),
            // ADR 0125's reader keeps its own working-file read where the plane is no
            // repository at all, until M75 slice 13 retires it; `committed_text` has none.
            Committed::NoHead { repository: false } => {
                // A file that is there and does not read (a folder, a file this process may not
                // read) is not "no file" (the review of #7113): strict.
                let path = Which::Shared.path(root);
                match std::fs::metadata(&path) {
                    Err(none) if none.kind() == std::io::ErrorKind::NotFound => {
                        Self::from_text(None)
                    }
                    _ => match std::fs::read_to_string(&path) {
                        Ok(text) => Self::from_text(Some(&text)),
                        Err(_) => Self {
                            template: Template::Strict,
                            template_said: true,
                            overrides: BTreeMap::new(),
                            sandbox: None,
                            policy_sandbox: None,
                            refused: vec![
                                "purlis could not read the project's settings file, so its \
                                 [policy] is enforced"
                                    .to_owned(),
                            ],
                            unread: true,
                            taken: Vec::new(),
                            denies: crate::dispatchgrant::Denied::all(),
                        },
                    },
                }
            }
            // A `HEAD` git could not read in time, or none yet, is not a file to fall back to
            // (the re-review of #6992): strict, with the sentence for the Notice.
            Committed::NoHead { repository: true } | Committed::Unread { .. } => Self {
                template: Template::Strict,
                template_said: true,
                overrides: BTreeMap::new(),
                // No level of its own: what this person accepted holds, never a looser one.
                sandbox: None,
                policy_sandbox: None,
                refused: vec![
                    "git could not read the project's last commit, so its [policy] is enforced"
                        .to_owned(),
                ],
                unread: true,
                taken: Vec::new(),
                denies: crate::dispatchgrant::Denied::all(),
            },
        };
        let local = Self::from_text(layer_text(root, Which::Local).text());
        // The local file can add a deny, never take one away.
        project.denies = project.denies.with(&local.denies);
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
        // The local file's template and its `[policy] sandbox` may raise the level, never lower
        // it; its `[sandbox]` is the person's own level, read where a start reads it, and not
        // the project's.
        if let Some(mine) = local
            .policy_sandbox
            .into_iter()
            .chain(local.template_said.then(|| local.template.sandbox_level()))
            .max()
            && mine > project.level()
        {
            project.sandbox = Some(mine);
        }
        project.refused.extend(local.refused);
        project
    }

    /// **The sandbox level the settings set** (Q308a, Q316, M75-11): `[policy] sandbox` where the
    /// file names one, else the legacy `[sandbox] mode`, else the template's (the floor under
    /// `strict`).
    pub fn level(&self) -> SandboxLevel {
        self.sandbox
            .unwrap_or_else(|| self.template.sandbox_level())
    }

    /// How the settings run `family`, before any lock or acceptance.
    pub fn mode(&self, family: Family) -> Mode {
        if family == Family::Sandbox {
            return mode_of_level_under(self.level(), self.template);
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

/// **A file of the plane root as it stands at `HEAD`**, what [`committed_text`] reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Committed {
    /// The file as committed, under the name it has there.
    Text {
        /// The name it is committed under.
        file: &'static str,
        /// What it holds.
        text: String,
    },
    /// `HEAD` holds no such file: the project says nothing in it.
    Absent,
    /// There is no `HEAD` to read: the plane is no git repository (`repository` false), or one
    /// with no commit yet. Nothing is committed, and the working file is never read in its place
    /// (M75 spec §5; Q320a N3, Q320b R2-1).
    NoHead {
        /// Whether the plane is a git repository at all.
        repository: bool,
    },
    /// git could not read `HEAD`, or a file at it, or not in time, or what it read does not hash
    /// to its name.
    Unread {
        /// The file that did not read, where it was one file and not the commit.
        file: Option<&'static str>,
        /// The sentence for the Notice.
        why: String,
    },
}

/// **The plane root's file, under the first of `names` it is committed by, as it stands at
/// `HEAD`**: [`committed_files`] for one file.
pub fn committed_text(root: &Path, names: &[&'static str]) -> Committed {
    committed_files(root, &[names])
        .pop()
        .expect("one answer for each file asked")
}

/// **Files of the plane root as they stand at one commit, `HEAD`'s** (Q309 point 1; M75 spec §5),
/// one answer for each of `files`, each the first of its names that commit holds: a change not
/// committed yet is not the project's, and neither is a branch folder's copy. `HEAD` is read once,
/// so every file is of the same commit.
///
/// The read is of history as written ([`crate::worktree::git::run_as_written`]): no replace ref,
/// graft or repository config key changes what it reads (review round 5, SEC5-SF1; Q320c R5-1).
/// It is also **verified**: the commit, its tree and every file are hashed here and compared with
/// the names git gives them, and each file is looked up in the tree as hashed, so an object
/// rewritten in place under its old name, or one that is gone, is [`Committed::Unread`], never
/// another text or [`Committed::Absent`]. There is no working-file fallback (review round 5, FB5
/// R4-4): where there is no `HEAD`, it says so, and a `.git` git does not read as a repository
/// is unread, never "no repository" (spec §5: the working file is never read as committed).
pub fn committed_files(root: &Path, files: &[&[&'static str]]) -> Vec<Committed> {
    use crate::worktree::git::run_as_written;
    let every = |answer: Committed| vec![answer; files.len()];
    let head = match run_as_written(
        root,
        &["rev-parse", "--verify", "--quiet", "HEAD^{commit}"],
        GIT_WAIT,
    ) {
        Ok(run) if run.ok() => run.line().to_owned(),
        Ok(run) if run.code == Some(128) && run.err.contains("not a git repository") => {
            // A `.git` that is there but does not read (a `HEAD` that names nothing, say) is a
            // repository git could not read, never a plane with none.
            if root.join(".git").symlink_metadata().is_ok() {
                return every(Committed::Unread {
                    file: None,
                    why: "git does not read the plane's .git as a repository".to_owned(),
                });
            }
            return every(Committed::NoHead { repository: false });
        }
        // `--verify --quiet` exits 1 with nothing said where there is no commit yet.
        Ok(run) if run.code == Some(1) && run.err.trim().is_empty() => {
            return every(Committed::NoHead { repository: true });
        }
        _ => {
            return every(Committed::Unread {
                file: None,
                why: "git could not read the project's last commit".to_owned(),
            });
        }
    };
    committed_files_at(root, &head, files)
}

/// [`committed_files`] at the commit `head` the caller already resolved, so a caller that reads
/// more from the same commit never meets a `HEAD` that moved in between.
pub fn committed_files_at(root: &Path, head: &str, files: &[&[&'static str]]) -> Vec<Committed> {
    match at_commit(root, head, files) {
        Ok(answers) => answers,
        Err(why) => vec![Committed::Unread { file: None, why }; files.len()],
    }
}

/// The object formats git names objects in.
#[derive(Debug, Clone, Copy)]
enum ObjectFormat {
    Sha1,
    Sha256,
}

impl ObjectFormat {
    /// The format an object name is in, by its length.
    fn of(name: &str) -> Option<Self> {
        if !name.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        match name.len() {
            40 => Some(Self::Sha1),
            64 => Some(Self::Sha256),
            _ => None,
        }
    }

    /// The length of a name in a tree, as bytes.
    fn raw_len(self) -> usize {
        match self {
            Self::Sha1 => 20,
            Self::Sha256 => 32,
        }
    }

    /// The name git gives an object of `kind` holding `content`.
    fn name(self, kind: &str, content: &[u8]) -> String {
        use sha2::Digest as _;
        let header = format!("{kind} {}\0", content.len());
        let digest: Vec<u8> = match self {
            Self::Sha1 => {
                let mut hash = sha1::Sha1::new();
                hash.update(header.as_bytes());
                hash.update(content);
                hash.finalize().to_vec()
            }
            Self::Sha256 => {
                let mut hash = sha2::Sha256::new();
                hash.update(header.as_bytes());
                hash.update(content);
                hash.finalize().to_vec()
            }
        };
        data_encoding::HEXLOWER.encode(&digest)
    }
}

/// One object of `cat-file --batch`'s answer.
struct Object<'a> {
    name: &'a str,
    kind: &'a str,
    content: &'a [u8],
}

impl Object<'_> {
    /// Whether the object is a `kind` and its content hashes to its name.
    fn holds(&self, format: ObjectFormat, kind: &str) -> bool {
        self.kind == kind && format.name(kind, self.content) == self.name.to_ascii_lowercase()
    }
}

/// The next object of a `cat-file --batch` answer: `Some(None)` where git said it is missing, and
/// `None` where the answer is not one git gives.
fn next_object<'a>(answer: &mut &'a [u8]) -> Option<Option<Object<'a>>> {
    let end = answer.iter().position(|b| *b == b'\n')?;
    let header = std::str::from_utf8(&answer[..end]).ok()?;
    let rest = &answer[end + 1..];
    if header.ends_with(" missing") {
        *answer = rest;
        return Some(None);
    }
    let mut words = header.split(' ');
    let (name, kind, size) = (words.next()?, words.next()?, words.next()?);
    if words.next().is_some() {
        return None;
    }
    let size: usize = size.parse().ok()?;
    let content = rest.get(..size)?;
    if rest.get(size) != Some(&b'\n') {
        return None;
    }
    *answer = &rest[size + 1..];
    Some(Some(Object {
        name,
        kind,
        content,
    }))
}

/// A tree's entries by name: each one's mode and object name.
type Entries<'a> = BTreeMap<&'a [u8], (&'a [u8], String)>;

/// A tree's entries. `None` where it is no tree.
fn tree_entries(mut tree: &[u8], format: ObjectFormat) -> Option<Entries<'_>> {
    let mut entries = BTreeMap::new();
    while !tree.is_empty() {
        let space = tree.iter().position(|b| *b == b' ')?;
        let mode = &tree[..space];
        let rest = &tree[space + 1..];
        let nul = rest.iter().position(|b| *b == 0)?;
        let raw = rest.get(nul + 1..nul + 1 + format.raw_len())?;
        entries.insert(&rest[..nul], (mode, data_encoding::HEXLOWER.encode(raw)));
        tree = &rest[nul + 1 + format.raw_len()..];
    }
    Some(entries)
}

/// [`committed_files`] at the commit `head`, every object read in one `cat-file --batch` and
/// verified here: the error is the sentence for a Notice where the commit or its tree does not
/// read as written.
fn at_commit(root: &Path, head: &str, files: &[&[&'static str]]) -> Result<Vec<Committed>, String> {
    let bad = |what: &str| format!("git could not read the project's last commit: {what}");
    let format = ObjectFormat::of(head).ok_or_else(|| bad("HEAD names no object"))?;
    let names: Vec<&'static str> = files.iter().flat_map(|f| f.iter().copied()).collect();
    let mut input = format!("{head}\n{head}^{{tree}}\n");
    for name in &names {
        input.push_str(&format!("{head}:{name}\n"));
    }
    let run = crate::worktree::git::run_as_written_with_input(
        root,
        &["cat-file", "--batch"],
        input.into_bytes(),
        GIT_WAIT,
    )
    .map_err(|_| bad("git did not run"))?;
    if run.code != Some(0) {
        return Err(bad("git could not read its objects"));
    }
    let mut answer = run.out.as_slice();
    let mut next = || next_object(&mut answer).ok_or_else(|| bad("git's answer did not read"));
    let commit = next()?.ok_or_else(|| bad("its commit is missing"))?;
    if commit.name != head || !commit.holds(format, "commit") {
        return Err(bad("its commit does not hash to its name"));
    }
    let tree = next()?.ok_or_else(|| bad("its tree is missing"))?;
    let named = commit
        .content
        .strip_prefix(b"tree ")
        .and_then(|rest| rest.split(|b| *b == b'\n').next());
    if named != Some(tree.name.as_bytes()) || !tree.holds(format, "tree") {
        return Err(bad(
            "its tree does not hash to the name its commit gives it",
        ));
    }
    let entries =
        tree_entries(tree.content, format).ok_or_else(|| bad("its tree does not read"))?;
    // Each name's answer: its text, none at that commit, or why it did not read.
    let mut read: BTreeMap<&'static str, Result<Option<String>, String>> = BTreeMap::new();
    for name in &names {
        let object = next()?;
        let unread = |what: &str| {
            Err(format!(
                "git could not read {name} at the last commit: {what}"
            ))
        };
        let said = match (entries.get(name.as_bytes()), object) {
            (None, None) => Ok(None),
            (None, Some(_)) => unread("git read a file its tree does not hold"),
            (Some(_), None) => unread("its tree names it, and the file is missing"),
            (Some((mode, oid)), Some(object)) => {
                if !matches!(*mode, b"100644" | b"100755") {
                    unread("it is not a file")
                } else if object.name != oid || !object.holds(format, "blob") {
                    unread("it does not hash to the name its tree gives it")
                } else {
                    Ok(Some(String::from_utf8_lossy(object.content).into_owned()))
                }
            }
        };
        read.insert(name, said);
    }
    Ok(files
        .iter()
        .map(|names| {
            for file in names.iter().copied() {
                match read.remove(file) {
                    Some(Ok(Some(text))) => return Committed::Text { file, text },
                    Some(Err(why)) => {
                        return Committed::Unread {
                            file: Some(file),
                            why,
                        };
                    }
                    _ => {}
                }
            }
            Committed::Absent
        })
        .collect())
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
    let text = format!("{}\n{}", resolved.spelled(), resolved.denies.spelled());
    crate::extension::hex(&sha2::Sha256::digest(text.as_bytes()))
}

/// The key an acceptance keeps the denies it accepted under: no family's key.
const DENIES_KEPT: &str = "dispatch.denies";

/// The key the template is spelled and kept under, at the level `off` only: no family's key.
const TEMPLATE_KEPT: &str = "template";

/// How every family runs for one chat: what [`at_start`] resolved, the sandbox's level, and the
/// dispatch denies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    modes: BTreeMap<Family, Mode>,
    /// The project's sandbox level ([`SandboxLevel`]); the `sandbox` family's mode follows it and
    /// the template ([`mode_of_level_under`]).
    level: SandboxLevel,
    /// The template the level was resolved under: at the level `off` it decides whether the
    /// `sandbox` family logs or is off ([`mode_of_level_under`]), so it travels with the level
    /// through [`Self::stricter`], [`Self::spelled`] and [`Self::parse`].
    template: Template,
    /// The dispatch denies (`[dispatch.denies]`, ticket T1.1-73).
    denies: crate::dispatchgrant::Denied,
}

impl Resolved {
    /// Every family at `mode`, the sandbox off where `mode` is and at the floor otherwise: what a
    /// process with no answer runs ([`Mode::Enforce`]), where the project's `[sandbox]` may still
    /// raise the level ([`sandbox_level`]); and no dispatch denied.
    pub fn all(mode: Mode) -> Self {
        let (level, template) = if mode == Mode::Off {
            (SandboxLevel::Off, Template::Open)
        } else {
            (SandboxLevel::Floor, Template::default())
        };
        Self::of_modes(
            Family::ALL
                .into_iter()
                .map(|family| (family, mode))
                .collect(),
            level,
            template,
            crate::dispatchgrant::Denied::default(),
        )
    }

    /// **`strict`**: every family enforced, and every dispatch across personas denied. What an
    /// acceptance that does not hold reads as.
    pub fn strict() -> Self {
        Self {
            denies: crate::dispatchgrant::Denied::all(),
            ..Self::all(Mode::Enforce)
        }
    }

    /// `modes` at `level` under `template`, the `sandbox` family's mode set by both
    /// ([`mode_of_level_under`]), with `denies`.
    fn of_modes(
        mut modes: BTreeMap<Family, Mode>,
        level: SandboxLevel,
        template: Template,
        denies: crate::dispatchgrant::Denied,
    ) -> Self {
        modes.insert(Family::Sandbox, mode_of_level_under(level, template));
        // The template matters only at the level `off`, so it is kept only there: two resolved
        // policies that run alike are equal, whatever template each came from.
        let template = if level == SandboxLevel::Off {
            template
        } else {
            Template::default()
        };
        Self {
            modes,
            level,
            template,
            denies,
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
        // What the hooks do with the `read` and `write` forbids follows the level and the
        // template (M75-11): `log` at the level `off` under any template but `open`.
        Self::of_modes(
            Family::ALL
                .into_iter()
                .map(|family| (family, under(settings, locks, family)))
                .collect(),
            level,
            settings.template,
            settings.denies.clone(),
        )
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

    /// The stricter of `self` and `other`, family by family, the stricter level, and every deny
    /// of either.
    pub fn stricter(&self, other: &Self) -> Self {
        Self::of_modes(
            Family::ALL
                .into_iter()
                .map(|family| (family, self.mode(family).max(other.mode(family))))
                .collect(),
            self.level.max(other.level),
            // Either side's template that is not `open` logs at the level `off`: the stricter.
            if self.template == Template::Open {
                other.template
            } else {
                self.template
            },
            self.denies.with(&other.denies),
        )
    }

    /// The families `self` runs looser than `accepted`, each with both modes: what a loosening
    /// press would accept. The sandbox is [`Self::level_looser_than`]'s.
    pub fn looser_than(&self, accepted: &Self) -> Vec<(Family, Mode, Mode)> {
        Family::ALL
            .into_iter()
            .filter(|family| *family != Family::Sandbox)
            .filter(|family| self.mode(*family) < accepted.mode(*family))
            .map(|family| (family, accepted.mode(family), self.mode(family)))
            .collect()
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

    /// As the accepted record and the app's answer to a hook carry it: `family=mode` pairs,
    /// comma-separated, the sandbox's by its level.
    /// The denies are not carried: only the app judges a dispatch.
    pub fn spelled(&self) -> String {
        let mut pairs: Vec<String> = self
            .modes
            .iter()
            .map(|(family, mode)| {
                let word = if *family == Family::Sandbox {
                    self.level.word()
                } else {
                    mode.word()
                };
                format!("{}={word}", family.key())
            })
            .collect();
        // The template matters only at the level `off`, so it is spelled only there and every
        // other spelling (and digest) is as it was.
        if self.level == SandboxLevel::Off {
            pairs.push(format!("{TEMPLATE_KEPT}={}", self.template.word()));
        }
        pairs.join(",")
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
        // A template that is not said is the default's, which logs at the level `off`: never
        // looser than a template that is said.
        let template = pairs
            .iter()
            .find(|(key, _)| *key == TEMPLATE_KEPT)
            .and_then(|(_, word)| Template::parse(word))
            .unwrap_or_default();
        Self::of_modes(
            Family::ALL
                .into_iter()
                .map(|family| (family, said.get(&family).copied().unwrap_or(Mode::Enforce)))
                .collect(),
            level,
            template,
            crate::dispatchgrant::Denied::default(),
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
        if self.level == SandboxLevel::Off {
            kept.insert(TEMPLATE_KEPT.to_owned(), self.template.word().to_owned());
        }
        if !self.denies.is_empty() {
            kept.insert(DENIES_KEPT.to_owned(), self.denies.spelled());
        }
        kept
    }

    fn of_kept(kept: &BTreeMap<String, String>) -> Self {
        let mut resolved = Self::parse(
            &kept
                .iter()
                .filter(|(key, _)| key.as_str() != DENIES_KEPT)
                .map(|(family, mode)| format!("{family}={mode}"))
                .collect::<Vec<_>>()
                .join(","),
        );
        if let Some(denies) = kept.get(DENIES_KEPT) {
            resolved.denies = crate::dispatchgrant::Denied::parse(denies);
        }
        resolved
    }

    /// This, at `level` where that is looser or stricter: what the team's answer to its off
    /// makes of an acceptance ([`accepted`]).
    fn at_level(self, level: SandboxLevel) -> Self {
        Self::of_modes(self.modes, level, self.template, self.denies)
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
    use crate::sandbox::local::PolicyAccepted;
    let kept = match crate::sandbox::local::policy_accepted_record(root) {
        PolicyAccepted::Held(kept) => Resolved::of_kept(&kept),
        PolicyAccepted::Never => {
            Resolved::of(&Settings::default(), &crate::sandbox::policy::Locks::none())
        }
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
fn resolved_in_the_app(root: &Path) -> Resolved {
    let accepted = accepted(root);
    let settings = Settings::read(root);
    let resolved =
        Resolved::of(&settings, &crate::sandbox::policy::Locks::of(root)).stricter(&accepted);
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
        return Err(std::io::Error::other(
            "the project's policy changed since it was shown, so nothing was accepted; look at \
             it again",
        ));
    }
    crate::sandbox::local::accept_policy(root, now.as_kept())?;
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
    /// What the project's `[policy]` holds that purlis could not read, each enforced.
    pub refused: Vec<String>,
    /// Whether an acceptance is kept here whose seal does not hold: it counts as none.
    pub unsealed: bool,
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
    let accepted = accepted(root);
    Standing {
        waiting: now.looser_than(&accepted),
        level: now.level_looser_than(&accepted),
        not_in_force: not_in_force_at(now.level()),
        lifted: now.lifts(&accepted),
        refused: settings.refused.clone(),
        unsealed: crate::sandbox::local::policy_unsealed(root),
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
///   can loosen anything.
pub fn in_force(root: &Path) -> Resolved {
    if in_the_app() {
        return resolved_in_the_app(root);
    }
    answered().unwrap_or_else(|| Resolved::all(Mode::Enforce))
}

/// **The sandbox level a chat starting in the project at `root` runs at** before a person's own
/// level (the follow-up's item 1): what drives [`crate::sandbox::team::allowed_levels`]'s
/// default, and so [`crate::sandbox::decide_granted`].
///
/// - **In the app**: [`in_force`]'s level, the stricter of what the project's committed file
///   sets (its `[sandbox] mode`, else its template's level) and what this person accepted.
/// - **Anywhere else**: never looser than what the project's file says as purlis read it before
///   Q308 (`file`, [`crate::sandbox::team::as_the_file_says`]), raised by the app's answer or
///   the app's answer, which can only tighten.
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
    if in_the_app() {
        resolved_in_the_app(root).denies
    } else {
        Settings::read(root).denies
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

/// One would-have-blocked line as a test reads it back: the rule, the family, the chat and the
/// host, and never a command, a path or a URL (Q308a). The app writes it into the event log as a
/// [`crate::eventlog::stops::WouldBlock`] under the chat (spec §10.3).
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

/// **One quiet line for chat `chat`'s would-have-blocked event**: `family`'s `rule` would have
/// refused, about `host` where there is one. Written by the app; any other process hands it to
/// the app ([`log_to`]), and keeps nothing where it cannot. Best effort: a line that cannot be
/// written changes no outcome. Never a Notice and never an ask.
pub fn record(root: &Path, chat: &str, family: Family, rule: &str, host: Option<&str>) {
    if !in_the_app() {
        if let Some(sink) = SINK.get() {
            sink(family, rule, host);
        }
        return;
    }
    write_line(root, chat, family, rule, host);
}

/// How the app writes a would-have-blocked line into the event log, under the chat it is of
/// ([`log_in_the_app`]).
pub type Writer = Box<dyn Fn(&Path, &str, &crate::eventlog::stops::WouldBlock) + Send + Sync>;

static WRITER: std::sync::OnceLock<Writer> = std::sync::OnceLock::new();

/// **Where the app writes its would-have-blocked lines** (spec §10.3): the event log's one writer,
/// which maps the chat's id to its number and run. Once. There is no store of its own: ADR 0125's
/// `app/enforcement/<chat>.jsonl` is retired.
pub fn log_in_the_app(writer: Writer) {
    let _ = WRITER.set(writer);
}

/// The app's write of one would-have-blocked line ([`record`]) into the event log.
pub fn write_line(root: &Path, chat: &str, family: Family, rule: &str, host: Option<&str>) {
    // The line as the event log would write it (S2): a rule id changed to a token, never dropped,
    // and a host that is no `host[:port]` left out, so what a test reads is what the log holds.
    let line = crate::eventlog::stops::WouldBlock {
        rules: vec![crate::eventlog::stops::token_of(rule)],
        level: None,
        family: family.key().to_owned(),
        host: host
            .filter(|host| crate::eventlog::stops::is_host(host))
            .map(|host| crate::shown::one_line(host, MOST_CHARS)),
    };
    #[cfg(any(test, feature = "test-support"))]
    CAPTURED.with(|held| {
        held.borrow_mut().push(Logged {
            at: jiff::Timestamp::now()
                .strftime("%Y-%m-%dT%H:%M:%SZ")
                .to_string(),
            rule: line.rules[0].clone(),
            family: line.family.clone(),
            chat: chat.to_owned(),
            host: line.host.clone(),
        });
    });
    #[cfg(not(test))]
    match WRITER.get() {
        Some(write) => write(root, chat, &line),
        None => tracing::warn!(
            "purlis: a would-have-blocked line for {} was not written: no event log is open",
            family.key()
        ),
    }
    #[cfg(test)]
    let _ = (root, &WRITER);
}

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    /// The lines this thread's test had written, standing in for the event log.
    static CAPTURED: std::cell::RefCell<Vec<Logged>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Chat `chat`'s would-have-blocked lines, oldest first, as this thread's test had them written:
/// what a test of a choke point reads in place of the event log.
#[cfg(any(test, feature = "test-support"))]
pub fn read_log(_root: &Path, chat: &str) -> Vec<Logged> {
    CAPTURED.with(|held| {
        held.borrow()
            .iter()
            .filter(|line| line.chat == chat)
            .cloned()
            .collect()
    })
}

/// The chat this process runs for, by the id the app started its harness with
/// ([`crate::hookwire::CHAT_ID_ENV`]): none for a harness the app did not start.
pub fn this_chat() -> Option<String> {
    crate::envvar::var(crate::hookwire::CHAT_ID_ENV).filter(|chat| !chat.is_empty())
}

#[cfg(test)]
mod tests;
