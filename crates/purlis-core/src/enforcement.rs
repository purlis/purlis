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
//! - A hook takes the stricter of what its chat started with, what the plane root's file says
//!   now, and what this person accepted ([`for_this_chat`]): a tightening applies at once, a
//!   loosening never mid-chat, and nothing a chat puts in its own environment loosens anything.
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
}

impl Family {
    /// Every family, in the order the docs list them.
    pub const ALL: [Family; 15] = [
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
        }
    }

    /// The family whose key is `key`, or none.
    pub fn of_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|family| family.key() == key)
    }

    /// Whether the family takes `log`: `vaults` takes off or enforce only (Q308a), and the
    /// sandbox is a level, not a mode.
    pub fn logs(self) -> bool {
        !matches!(self, Family::Sandbox | Family::Vaults)
    }

    /// **Whether `[policy]` switches the family yet.** Not where its wiring waits on the second
    /// review of how it fits with the grants and remembered answers it holds (instruction of
    /// 2026-11-16): those stay enforced, as before Q308, whatever `[policy]` says.
    pub fn switchable(self) -> bool {
        !matches!(
            self,
            Family::Sandbox | Family::Dispatch | Family::Vaults | Family::Asks | Family::Browser
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
            return if self.sandbox_level() == SandboxLevel::Off {
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

    /// The sandbox level the template sets where the project's `[sandbox]` says none (Q308a).
    pub fn sandbox_level(self) -> SandboxLevel {
        match self {
            Template::Open => SandboxLevel::Off,
            Template::Watch | Template::Hold => SandboxLevel::Floor,
            Template::Team | Template::Strict => SandboxLevel::Full,
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
        let Some(table) = text
            .and_then(|text| text.parse::<toml::Table>().ok())
            .and_then(|top| top.get(TABLE).cloned())
        else {
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
                        "{TABLE}.guards is not a table of git, merge, release, save and \
                         credential, and is not read"
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

    /// **The plane at `root`'s `[policy]`**: its committed file's, made stricter where the local
    /// file is stricter, and never looser for anything the local file says (Q308a). `root` is
    /// the plane root; a branch folder's own file is never asked.
    pub fn read(root: &Path) -> Self {
        use crate::settings::{Which, layer_text};
        let mut project = Self::from_text(layer_text(root, Which::Shared).text());
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
/// before anything was kept (the upgrade announces it once).
pub fn accepted(root: &Path) -> Resolved {
    crate::sandbox::local::policy_accepted(root)
        .map(|kept| Resolved::of_kept(&kept))
        .unwrap_or_else(|| {
            Resolved::of(&Settings::default(), &crate::sandbox::policy::Locks::none())
        })
}

/// **How a chat starting in the project at `root` runs** (Q308a): the stricter of what the
/// project says now and what this person last accepted, and kept as accepted, so a tightening
/// holds until the person presses to loosen it. Best effort: an acceptance that cannot be kept
/// changes nothing this start resolves.
pub fn at_start(root: &Path) -> Resolved {
    let resolved = committed(root).stricter(&accepted(root));
    if crate::sandbox::local::policy_accepted(root).map(|kept| Resolved::of_kept(&kept))
        != Some(resolved.clone())
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
/// it is looser. Never a chat's: no sandboxed chat writes the file it is kept in.
pub fn press(root: &Path) -> std::io::Result<()> {
    crate::sandbox::local::accept_policy(root, committed(root).as_kept())
}

/// **How `family` runs for the chat this process serves**, from a hook: the stricter of what its
/// chat started with ([`ENV`], read through `env`), what the plane at `root` says now, and what
/// this person accepted. A tightening applies at once; a loosening waits for a press and the
/// next start. [`ENV`] can only ever tighten: a git hook runs under the environment the chat's
/// own command gave it, so it is never trusted to loosen.
pub fn for_this_chat(root: &Path, env: &dyn Fn(&str) -> Option<String>) -> Resolved {
    let now = committed(root).stricter(&accepted(root));
    match env(ENV).filter(|said| !said.is_empty()) {
        Some(started) => Resolved::parse(&started).stricter(&now),
        None => now,
    }
}

/// [`for_this_chat`], with the process's own environment, for one family.
pub fn mode(root: &Path, family: Family) -> Mode {
    for_this_chat(root, &crate::envvar::var).mode(family)
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
            if let Some(chat) = chat {
                record(root, chat, family, rule, host);
            }
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

/// **Append one quiet line to chat `chat`'s enforcement log**: `family`'s `rule` would have
/// refused, about `host` where there is one. Best effort: a log that cannot be written changes no
/// outcome, and is said once to the process's own log. Never a Notice and never an ask.
pub fn record(root: &Path, chat: &str, family: Family, rule: &str, host: Option<&str>) {
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
