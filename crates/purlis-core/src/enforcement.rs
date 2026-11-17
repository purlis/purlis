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
//!   [`ENV`], which the app sets after anything a profile set.
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
use std::path::{Path, PathBuf};

/// The table of the committed settings file that holds these keys.
pub const TABLE: &str = "policy";

/// The key naming the template.
pub const TEMPLATE_KEY: &str = "template";

/// The variable a chat's hooks read what their chat started with from ([`Resolved::spelled`]).
/// Set by the app at the launch, after anything a profile set, and never inherited.
pub const ENV: &str = "PURLIS_POLICY";

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
    pub const ALL: [Family; 24] = [
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
    /// `open` turns it off.
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

    /// Whether the family takes `log`: `vaults` takes off or enforce only (Q308a), and the
    /// sandbox is a level, not a mode.
    pub fn logs(self) -> bool {
        !matches!(self, Family::Sandbox | Family::Vaults)
    }

    /// **Whether `[policy]` switches the family yet.** Not where its wiring waits on a later
    /// ruling; those stay enforced, as before Q308, whatever `[policy]` says:
    ///
    /// - `vaults` and `browser`, and the sandbox's level: each holds grants or remembered
    ///   answers (sandbox grants, browser reach, the team's sandbox accept) whose fit with
    ///   `[policy]` the follow-up settles. `dispatch` and `asks` switch their simple half now
    ///   (instruction of 2026-11-16): a pair nothing covers starts with no grant, and a host ask
    ///   is answered Allow; a lock and the person's nevers and Keep blocked hold in every mode;
    /// - `network` and `ports`: the egress proxy's refusal is its live ask, and a port or socket
    ///   is a sandbox grant, both held with the above;
    /// - `processes`: the wrap's refusal of programs that would run outside it (`launchctl`,
    ///   `osascript`, `open`, cron, Apple Events) is part of the sandbox, kept whenever a wrap is
    ///   applied (the review of #6992); what the family may lift beyond that is the follow-up's;
    /// - `skills`: every skills refusal a chat meets is a hand-over to the person (a draft, a
    ///   proposal) or the org's `[skill_policy]` on an install only the window makes, so which
    ///   of them the family switches needs a ruling;
    /// - the families Q308b added (`guards.forge`, `guards.shape`, `guards.plane`, `mode`,
    ///   `tools`, `trust`, `runners`, `consents`, `limits`): keys with their template defaults
    ///   now, wired with the inventory's full mapping next.
    pub fn switchable(self) -> bool {
        matches!(
            self,
            Family::Dispatch
                | Family::Asks
                | Family::GuardsGit
                | Family::GuardsMerge
                | Family::GuardsRelease
                | Family::GuardsSave
                | Family::GuardsCredential
                | Family::Secrets
        )
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
    /// Everything logged; `secrets`, `vaults` and `guards.credential` enforced. The default.
    #[default]
    Watch,
    /// `watch`, and `guards.merge` enforced.
    Hold,
    /// The merge, release, save and credential guards, `secrets` and `vaults` enforced; the rest
    /// logged.
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
        let secret = matches!(
            family,
            Family::Secrets | Family::Vaults | Family::GuardsCredential
        );
        let mode = match self {
            Template::Open => Mode::Off,
            Template::Strict => Mode::Enforce,
            Template::Watch if secret => Mode::Enforce,
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
            return if self.sandbox_level() == Some(SandboxLevel::Off) {
                Mode::Off
            } else {
                Mode::Enforce
            };
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

/// The sandbox family's three values (Q308a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxLevel {
    /// No wrap.
    Off,
    /// Today's floor: only purlis's own secret stores are unreadable.
    Floor,
    /// The full sandbox.
    Full,
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
    /// What the table holds that purlis could not read, one sentence each, for the window's
    /// Notice. Each was taken in the strict direction.
    pub refused: Vec<String>,
}

impl Settings {
    /// What `text`, a settings file, says; the default where it is `None`, holds no `[policy]`,
    /// or is not TOML (that file's own reader says why).
    pub fn from_text(text: Option<&str>) -> Self {
        let mut said = Self::read_text(text);
        // **Any value purlis cannot read means `strict`**, with the sentence for the Notice (the
        // review of #6992, S6): a file that is not TOML, a key that is no family, a mode or a
        // template that is none.
        if !said.refused.is_empty() {
            said.template = Template::Strict;
            said.template_said = true;
            said.overrides.clear();
        }
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
        let Some(table) = top.get(TABLE).cloned() else {
            return Self::default();
        };
        let mut said = Self::default();
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
                self.refused.push(format!(
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
        let mut project = Self::from_text(committed_text(root).as_deref());
        let local = Self::from_text(layer_text(root, Which::Local).text());
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
        project.refused.extend(local.refused);
        project
    }

    /// How the settings run `family`, before any lock or acceptance.
    pub fn mode(&self, family: Family) -> Mode {
        self.overrides
            .get(&family)
            .copied()
            .unwrap_or_else(|| self.template.mode(family))
    }
}

/// How long reading the committed file at `HEAD` may take.
const GIT_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// **The plane root's committed settings file as it stands at `HEAD`** (Q309 point 1): a
/// change not committed yet is not the project's. Where the plane is no git repository, the
/// file as it stands is all there is. `None` where there is no such file.
pub fn committed_text(root: &Path) -> Option<String> {
    use crate::settings::{Which, layer_text};
    let git = |args: &[&str]| {
        crate::worktree::git::run(root, args, GIT_WAIT)
            .ok()
            .filter(crate::worktree::git::Run::ok)
    };
    if git(&["rev-parse", "--verify", "--quiet", "HEAD"]).is_none() {
        return layer_text(root, Which::Shared).text().map(str::to_owned);
    }
    let file = Which::Shared.file_at(root);
    git(&["show", &format!("HEAD:./{file}")]).map(|run| run.out)
}

/// **The digest of what the press accepts** (the review of #6992, S2): of the committed text the
/// window showed, so a press refuses a file that changed since.
pub fn digest_of(root: &Path) -> String {
    use sha2::Digest;
    let text = committed_text(root).unwrap_or_default();
    crate::extension::hex(&sha2::Sha256::digest(text.as_bytes()))
}

/// How every family runs for one chat: what [`at_start`] resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved(BTreeMap<Family, Mode>);

impl Resolved {
    /// Every family at `mode`.
    pub fn all(mode: Mode) -> Self {
        Self(
            Family::ALL
                .into_iter()
                .map(|family| (family, mode))
                .collect(),
        )
    }

    /// `settings` under `locks`: each family as the settings say, raised to its lock, and
    /// enforced where it is not switchable yet.
    pub fn of(settings: &Settings, locks: &crate::sandbox::policy::Locks) -> Self {
        Self(
            Family::ALL
                .into_iter()
                .map(|family| (family, under(settings, locks, family)))
                .collect(),
        )
    }

    /// How `family` runs.
    pub fn mode(&self, family: Family) -> Mode {
        self.0.get(&family).copied().unwrap_or(Mode::Enforce)
    }

    /// The stricter of `self` and `other`, family by family.
    pub fn stricter(&self, other: &Self) -> Self {
        Self(
            Family::ALL
                .into_iter()
                .map(|family| (family, self.mode(family).max(other.mode(family))))
                .collect(),
        )
    }

    /// The families `self` runs looser than `accepted`, each with both modes: what a loosening
    /// press would accept.
    pub fn looser_than(&self, accepted: &Self) -> Vec<(Family, Mode, Mode)> {
        Family::ALL
            .into_iter()
            .filter(|family| self.mode(*family) < accepted.mode(*family))
            .map(|family| (family, accepted.mode(family), self.mode(family)))
            .collect()
    }

    /// As [`ENV`] carries it: `family=mode` pairs, comma-separated.
    pub fn spelled(&self) -> String {
        self.0
            .iter()
            .map(|(family, mode)| format!("{}={}", family.key(), mode.word()))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// What [`Self::spelled`] spelled. A pair it cannot read, or a family it does not name, is
    /// enforced.
    pub fn parse(text: &str) -> Self {
        let said: BTreeMap<Family, Mode> = text
            .split(',')
            .filter_map(|pair| pair.split_once('='))
            .filter_map(|(family, mode)| Some((Family::of_key(family)?, Mode::parse(mode)?)))
            .collect();
        Self(
            Family::ALL
                .into_iter()
                .map(|family| (family, said.get(&family).copied().unwrap_or(Mode::Enforce)))
                .collect(),
        )
    }

    fn as_kept(&self) -> BTreeMap<String, String> {
        self.0
            .iter()
            .map(|(family, mode)| (family.key().to_owned(), mode.word().to_owned()))
            .collect()
    }

    fn of_kept(kept: &BTreeMap<String, String>) -> Self {
        Self::parse(
            &kept
                .iter()
                .map(|(family, mode)| format!("{family}={mode}"))
                .collect::<Vec<_>>()
                .join(","),
        )
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
pub fn accepted(root: &Path) -> Resolved {
    use crate::sandbox::local::PolicyAccepted;
    match crate::sandbox::local::policy_accepted_record(root) {
        PolicyAccepted::Held(kept) => Resolved::of_kept(&kept),
        PolicyAccepted::Never => {
            Resolved::of(&Settings::default(), &crate::sandbox::policy::Locks::none())
        }
        PolicyAccepted::Broken => Resolved::all(Mode::Enforce),
    }
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
    let resolved = committed(root).stricter(&accepted);
    if resolved != accepted
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

/// **The person's press, in the window**: they accept what the project says now, looser where
/// it is looser, sealed by the app. Never a chat's: no sandboxed chat writes the file it is kept
/// in, and only the app can seal it. One press, on the committed file as it stands; never a
/// re-seal of an acceptance something else wrote.
pub fn press(root: &Path, shown: &str) -> std::io::Result<()> {
    if digest_of(root) != shown {
        return Err(std::io::Error::other(
            "the project's policy changed since it was shown, so nothing was accepted; look at \
             it again",
        ));
    }
    crate::sandbox::local::accept_policy(root, committed(root).as_kept())
}

/// **What the window says about the project's `[policy]` on this machine** (Q308a, Q308b).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Standing {
    /// Each family the project now runs looser than this person accepted: the family, the
    /// accepted mode and the project's. What the press would accept.
    pub waiting: Vec<(Family, Mode, Mode)>,
    /// What the project's `[policy]` holds that purlis could not read, each enforced.
    pub refused: Vec<String>,
    /// Whether an acceptance is kept here whose seal does not hold: it counts as none.
    pub unsealed: bool,
    /// The digest of the committed text this was read from, which the press must name.
    pub digest: String,
}

/// [`Standing`] for the project at `root`.
pub fn standing(root: &Path) -> Standing {
    Standing {
        waiting: waiting(root),
        refused: Settings::read(root).refused,
        unsealed: crate::sandbox::local::policy_unsealed(root),
        digest: digest_of(root),
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

fn answered() -> Option<Resolved> {
    #[cfg(test)]
    {
        ANSWERED.with(|held| held.borrow().clone())
    }
    #[cfg(not(test))]
    {
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
const MOST_CHARS: usize = 200;

/// What a log line names as its chat where the process was not told which (Q308 review, S3).
pub const UNKNOWN_CHAT: &str = "unknown";

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
        at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
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
    options.append(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = crate::contain::nofollow(&mut options).open(path)?;
    let mut text = serde_json::to_string(line).map_err(std::io::Error::other)?;
    text.push('\n');
    file.write_all(text.as_bytes())
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
