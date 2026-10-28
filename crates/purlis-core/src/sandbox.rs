// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The sandbox a chat runs in (ADR 0067): one harness-agnostic policy, compiled per harness.
//!
//! [`Plane`] reads a plane's `charter.toml` once; [`Said`] is what it says about the sandbox;
//! [`Denied`] resolves the denial classes to this machine; [`Compiled`] is the neutral answer
//! every harness's compiler reads; and [`for_start`] is the one place a chat is sandboxed or
//! refused. Each harness's compiler is its adapter's
//! ([`crate::harness::HarnessAdapter::sandbox_compiler`], asked through [`compiler`]).

use std::fmt;
use std::path::{Path, PathBuf};

use crate::harness::Harness;

/// The table in `charter.toml` that holds the policy.
pub const TABLE: &str = "sandbox";

/// The file the policy is read from. Only the committed file: turning the sandbox on is a
/// restriction a plane may carry (ADR 0035), and nothing else says anything about it.
/// Named as messages name it; the file read is [`crate::names::manifest`].
pub const FILE: &str = crate::names::PLANE_MANIFEST.reads[0];

/// A named set of hosts a sandboxed chat may reach (ADR 0067 §3). What each holds is
/// [`hosts`]; SD-4 and SD-31 add to the set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    ModelProviders,
    Forge,
    Toolchains,
}

impl Preset {
    /// Every preset, in the order a sentence lists them.
    pub const ALL: [Preset; 3] = [Preset::ModelProviders, Preset::Forge, Preset::Toolchains];

    /// What a new plane starts with (ruling V21, 5).
    pub const DEFAULT: [Preset; 3] = Self::ALL;

    /// The word `charter.toml` names it by.
    pub fn word(self) -> &'static str {
        match self {
            Self::ModelProviders => "model-providers",
            Self::Forge => "forge",
            Self::Toolchains => "toolchains",
        }
    }

    /// The name the window shows it by (`CONTEXT.md`, **Internet access**, #1340).
    pub fn title(self) -> &'static str {
        match self {
            Self::ModelProviders => "AI providers",
            Self::Forge => "Code hosting",
            Self::Toolchains => "Package registries",
        }
    }

    /// Whether a chat may write the project's own package caches while it is on (D-1337-6):
    /// what [`Widened::of`] grants, and what Settings › Sandbox says, from this one answer.
    pub fn widens_caches(self) -> bool {
        matches!(self, Self::Toolchains)
    }

    /// The hosts it lists itself, before a project's own forges ([`hosts`]): the one place a
    /// preset's hosts are written.
    pub fn own_hosts(self) -> &'static [&'static str] {
        match self {
            Self::ModelProviders => &[
                "api.anthropic.com",
                "claude.ai",
                "platform.claude.com",
                "api.openai.com",
                "auth.openai.com",
                "chatgpt.com",
                "opencode.ai",
                "models.dev",
                "openrouter.ai",
            ],
            Self::Forge => &[
                "github.com",
                "api.github.com",
                "codeload.github.com",
                "*.githubusercontent.com",
                "ghcr.io",
                "gitlab.com",
                "registry.gitlab.com",
            ],
            Self::Toolchains => &[
                "registry.npmjs.org",
                "registry.yarnpkg.com",
                "pypi.org",
                "files.pythonhosted.org",
                "crates.io",
                "index.crates.io",
                "static.crates.io",
                "static.rust-lang.org",
                "proxy.golang.org",
                "sum.golang.org",
                "rubygems.org",
                "repo.maven.apache.org",
                "repo1.maven.org",
            ],
        }
    }

    fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|preset| preset.word() == word)
    }

    /// Every preset's word, as a refusal lists them. The one place that list is written.
    fn listed() -> String {
        Self::ALL.map(Self::word).join(", ")
    }
}

/// A plane's sandbox policy, where it has turned the sandbox on.
///
/// Only Internet access is the plane's to choose: the presets, and the project's own hosts
/// ([`hosts::Host`], #1341). What a chat may write (its own directory, and the harness's own temp
/// directory) and what it is always denied ([`Class`]) are not in the file at all, so no file
/// can widen them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    pub egress: Vec<Preset>,
    /// The project's own hosts: `[sandbox] hosts` in the committed file.
    pub hosts: Vec<hosts::Host>,
    /// `certificate-checks`: whether a chat may ask the system's certificate check, which Go
    /// programs such as `gh` need to verify a host (D-1337-7). **On unless the file says
    /// `false`** (the operator's ruling R1 of 2026-10-13, amending ADR 0093): the key only
    /// ever narrows. An administrator's `socket-grants` lock turns it off besides
    /// ([`CertificateChecks`]).
    pub certificate_checks: bool,
    /// Each persona's own grants: `[sandbox.personas.<name>]` in the committed file (#1362).
    pub personas: std::collections::BTreeMap<String, persona::Grants>,
}

impl Policy {
    /// The sandbox turned on and nothing else said: the default presets ([`Preset::DEFAULT`]),
    /// no host of the project's own, certificate checks on, no persona's grants.
    pub fn defaults() -> Self {
        Self {
            egress: Preset::DEFAULT.to_vec(),
            hosts: Vec::new(),
            certificate_checks: true,
            personas: std::collections::BTreeMap::new(),
        }
    }
}

/// The key that turns the system's certificate check off for a project's chats: `false` is the
/// one value that changes anything (R1, 2026-10-13).
pub const CERTIFICATE_CHECKS: &str = "certificate-checks";

/// **Whether a sandboxed chat of a project has the system's certificate check, and why not**
/// (the operator's ruling R1 of 2026-10-13, amending ADR 0093). On by default, on every
/// harness; off only where the project's committed file says `certificate-checks = false` or
/// an administrator's `socket-grants` lock stands. One answer for what a start compiles
/// ([`Widened::of`], [`Widened::and_granted`]), what Settings says, and what a block's Notice
/// says, so none of them can tell another story.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertificateChecks {
    /// A chat has it from its next start.
    On,
    /// The project's committed file turns it off for everyone in the project.
    OffByProject,
    /// An administrator's policy turns it off on this machine: the sentence names who set it.
    OffByPolicy(String),
}

impl CertificateChecks {
    /// What `policy`, a project's sandbox in force, comes to under `locks`. Policy is said
    /// first: it holds whatever the project's file says.
    pub fn of(policy: &Policy, locks: &policy::Locks) -> Self {
        if locks.forbids_socket_grants() {
            return Self::OffByPolicy(format!(
                "Policy turns certificate checks off on this machine. {}",
                locks.locked_by()
            ));
        }
        if policy.certificate_checks {
            Self::On
        } else {
            Self::OffByProject
        }
    }

    /// [`Self::of`] for the project at `root` as its file stands; none where its chats are not
    /// sandboxed.
    pub fn at(root: &Path) -> Option<Self> {
        let locks = policy::Locks::of(root);
        Plane::read(root)
            .in_force(&locks)
            .map(|policy| Self::of(&policy, &locks))
    }

    pub fn is_on(&self) -> bool {
        matches!(self, Self::On)
    }
}

/// The `[sandbox]` block that turns the sandbox on with the default egress, as charter writes it
/// into a new project's `charter.toml` and into an existing one whose operator took the offer
/// (ADR 0067 §1, ruling V21 1 and 5). It ends with a newline.
pub fn on_block() -> String {
    let egress: Vec<String> = Preset::DEFAULT
        .iter()
        .map(|preset| format!("\"{}\"", preset.word()))
        .collect();
    format!(
        "[{TABLE}]\nmode = \"on\"\negress = [{}]\n",
        egress.join(", ")
    )
}

/// A plane's `charter.toml`, read and parsed once for everything the sandbox asks of it: the
/// policy, the refusals and the forge hosts.
#[derive(Debug, Clone, Default)]
pub struct Plane {
    top: Option<toml::Table>,
    /// The file is there and is not TOML charter can read, so what it says about the sandbox
    /// is unknown ([`NotStarted::PlaneUnreadable`]).
    unreadable: bool,
    /// There is no file at all, so what the project says about the sandbox is unknown too
    /// ([`NotStarted::PlaneMissing`]): a project's own manifest that has gone never reads as
    /// "the sandbox is off" (D-1410e).
    missing: bool,
}

impl Plane {
    /// The plane at `root`. No file says nothing. A file that cannot be read, or is not TOML,
    /// is [`Self::unreadable`]: it may say `[sandbox]`, so it never reads as saying nothing.
    pub fn read(root: &Path) -> Self {
        match read_plane_file(&crate::names::manifest(root)) {
            Ok(text) => Self::of(text.as_deref()),
            Err(()) => Self {
                top: None,
                unreadable: true,
                missing: false,
            },
        }
    }

    /// `text`, the whole of a `charter.toml`, or `None` where there is no file.
    pub fn of(text: Option<&str>) -> Self {
        let top = text.map(|text| text.parse::<toml::Table>().ok());
        Self {
            unreadable: matches!(top, Some(None)),
            missing: top.is_none(),
            top: top.flatten(),
        }
    }

    /// Whether the file is there and charter cannot read it as TOML.
    pub fn unreadable(&self) -> bool {
        self.unreadable
    }

    /// Whether there is no file at all.
    pub fn missing(&self) -> bool {
        self.missing
    }

    /// What it says about the sandbox.
    pub fn said(&self) -> Said {
        Said::of(self.top.as_ref())
    }

    /// What its `[sandbox] browser-sites` holds ([`browser_sites`], #2443): read as a project's
    /// own hosts are, and never among them.
    pub fn browser_sites(&self) -> Result<hosts::Listed, hosts::NotAList> {
        hosts::read(
            self.top
                .as_ref()
                .and_then(|top| top.get(TABLE))
                .and_then(|table| table.get(browser_sites::KEY)),
        )
    }

    /// What it says of the team's say over each person's level ([`team`], #2071).
    pub fn team(&self) -> team::Team {
        team::Team::of_top(self.top.as_ref())
    }

    /// The hosts of its `[[forge]]` blocks, without a port: a sandbox allows a host. A host that
    /// is not one ([`crate::forge::host_ok`]) is never added.
    fn forge_hosts(&self) -> Vec<String> {
        forge_hosts_of(self.top.as_ref())
            .into_iter()
            .filter_map(|(_, host)| host.ok())
            .map(|host| host.to_string())
            .collect()
    }

    /// **The sandbox policy in force for chats here under `locks`**: the project's own where it
    /// turned the sandbox on, and none where it has not — unless an administrator's policy
    /// requires the sandbox ([`policy::Locks::forbids_opt_out`], D-1423-1). Then a project
    /// with no `[sandbox]`, or one that has not turned it on, runs every chat sandboxed as if
    /// it had: what its `[sandbox]` says with the mode on, so the default presets where it
    /// names none ([`Policy::defaults`]). `locks` hold that policy as they hold any other when
    /// it is compiled ([`Compiled::granted`]): the presets it fixes, the hosts it allows.
    ///
    /// The one place "do chats here run sandboxed" is answered, for a start, Settings and every
    /// Notice.
    pub fn in_force(&self, locks: &policy::Locks) -> Option<Policy> {
        self.said().in_force(locks)
    }

    /// The hosts that reach chats here because this file says so, under `locks`: its own
    /// `[sandbox] hosts`, then, while the `forge` preset is on, its `[[forge]]` hosts — what the
    /// one-time Notice of a change names (#1341). A persona's own hosts (#1362) are not among
    /// them: they reach nothing until the person on each machine allows them, which a Notice of
    /// their own asks ([`persona::shown`], D-1362-7).
    ///
    /// **Only what a chat reaches** (#1423): a host an administrator's policy locks out, and a
    /// forge's while policy turns the `forge` preset off, is not named, as [`Compiled::granted`]
    /// grants neither.
    pub fn granted_hosts(&self, locks: &policy::Locks) -> Vec<String> {
        let Some(policy) = self.in_force(locks) else {
            return Vec::new();
        };
        let reaches = |host: &hosts::Host, level: hosts::Level| {
            locks
                .refuses(&hosts::Granted {
                    host: host.clone(),
                    level,
                })
                .is_none()
        };
        let mut out: Vec<String> = policy
            .hosts
            .iter()
            .filter(|host| reaches(host, hosts::Level::Project))
            .map(ToString::to_string)
            .collect();
        if locks.presets(&policy.egress).contains(&Preset::Forge) {
            for host in self.forge_hosts() {
                let reached = hosts::Host::parse(&host)
                    .is_ok_and(|host| reaches(&host, hosts::Level::Project));
                if reached && !out.contains(&host) {
                    out.push(host);
                }
            }
        }
        out
    }
}

/// The most of a `charter.toml` charter reads when a chat starts. Far more than any plane's,
/// and a bound on what a start can be made to read.
pub const PLANE_FILE_MAX: usize = 1024 * 1024;

/// The `charter.toml` at `path`: `None` where there is none, its text where it is a regular
/// file of at most [`PLANE_FILE_MAX`] bytes of UTF-8, and `Err` for anything else.
///
/// **Not followed, and never blocked on.** A link, dangling or not, a directory, a FIFO, a
/// socket and a device are each refused: the name is checked without following it, then opened
/// without following it and without waiting (`O_NOFOLLOW | O_NONBLOCK`), and the open file is
/// checked again before it is read, so nothing swapped in between is read either.
pub(crate) fn read_plane_file(path: &Path) -> Result<Option<String>, ()> {
    use std::io::Read;
    match std::fs::symlink_metadata(path) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(()),
        Ok(meta) if !meta.file_type().is_file() => return Err(()),
        Ok(_) => {}
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|_| ())?;
    if !file.metadata().map_err(|_| ())?.file_type().is_file() {
        return Err(());
    }
    let mut bytes = Vec::new();
    let limit = u64::try_from(PLANE_FILE_MAX).unwrap_or(u64::MAX) + 1;
    file.take(limit).read_to_end(&mut bytes).map_err(|_| ())?;
    if bytes.len() > PLANE_FILE_MAX {
        return Err(());
    }
    String::from_utf8(bytes).map(Some).map_err(|_| ())
}

/// One thing a plane's `[sandbox]` says that charter does not honour as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// `sandbox` is not a table.
    NotATable,
    /// A key the schema does not have.
    UnknownKey(String),
    /// `mode = "off"` where the project's `policy` is `"never"`, which forbids it (#2071).
    ModeOff,
    /// A `mode` that is not `"on"` or `"off"`.
    ModeUnknown,
    /// `egress` that is not a list.
    EgressNotAList,
    /// A word in `egress` that is not a preset, as written.
    EgressUnknown(String),
    /// `hosts` that is not a list.
    HostsNotAList,
    /// An entry of `hosts` that is not a host ([`hosts::Host::parse`]): as written, and why.
    Host(String, String),
    /// `browser-sites` that is not a list (#2443).
    BrowserSitesNotAList,
    /// An entry of `browser-sites` that is not a host: as written, and why.
    BrowserSite(String, String),
    /// `certificate-checks` that is not `true` or `false`.
    CertificateChecksNotABool,
    /// A `policy` that is not `"never"` or `"local"`: read as `"never"`.
    PolicyUnknown,
    /// Something in `personas` that grants nothing ([`persona::read`]), as one sentence with
    /// its key.
    Persona(crate::settings::Refusal),
    /// A `[[forge]]` host the sandbox does not let chats reach while the `forge` preset is on
    /// ([`forge_hosts_of`]): as written, and why. Said, not a change: the forge still works
    /// outside the sandbox.
    ForgeHost(String, String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let on = "so the sandbox is on";
        match self {
            Self::NotATable => write!(
                f,
                "{TABLE} in {FILE} is not a table — [{TABLE}] holds mode, egress, hosts and \
                 {CERTIFICATE_CHECKS}; {on}"
            ),
            Self::UnknownKey(key) => write!(
                f,
                "{TABLE}.{key} in {FILE} is not a key purlis reads — [{TABLE}] holds mode, \
                 egress, hosts and {CERTIFICATE_CHECKS}"
            ),
            Self::ModeOff => write!(
                f,
                "{TABLE}.mode in {FILE} cannot be \"off\": the project's {TABLE}.{} is \
                 \"never\", so nobody turns the sandbox off; {on}",
                team::KEY
            ),
            Self::ModeUnknown => write!(
                f,
                "{TABLE}.mode in {FILE} is not \"on\" — the one value a project may give it; {on}"
            ),
            Self::EgressNotAList => write!(
                f,
                "{TABLE}.egress in {FILE} is not a list of presets — one or more of: {}; the \
                 default is used",
                Preset::listed()
            ),
            Self::EgressUnknown(word) => write!(
                f,
                "{TABLE}.egress in {FILE} names {word}, which is not a preset — one of: {}",
                Preset::listed()
            ),
            Self::HostsNotAList => write!(
                f,
                "{TABLE}.hosts in {FILE} is not a list of hosts, so no host of the project's is \
                 allowed — write hosts = [\"api.example.com\", \"10.0.0.5:6443\"]"
            ),
            Self::Host(written, why) => write!(
                f,
                "{TABLE}.hosts in {FILE} names {written}, which no chat is let reach: {why}"
            ),
            Self::BrowserSitesNotAList => write!(
                f,
                "{TABLE}.{} in {FILE} is not a list of sites, so no browser site of the \
                 project's is allowed — write {} = [\"news.example.org\"]",
                browser_sites::KEY,
                browser_sites::KEY
            ),
            Self::BrowserSite(written, why) => write!(
                f,
                "{TABLE}.{} in {FILE} names {written}, which no browser is let reach: {why}",
                browser_sites::KEY
            ),
            Self::CertificateChecksNotABool => write!(
                f,
                "{TABLE}.{CERTIFICATE_CHECKS} in {FILE} is not true or false; certificate checks \
                 are off until it is"
            ),
            Self::PolicyUnknown => write!(
                f,
                "{TABLE}.{} in {FILE} is not \"never\" or \"local\", so it is read as \
                 \"never\": nobody turns the sandbox down until it says one",
                team::KEY
            ),
            Self::Persona(said) => f.write_str(&said.why),
            Self::ForgeHost(written, why) => write!(
                f,
                "forge.host in {FILE} names {written}, which the sandbox does not let chats \
                 reach: {why}"
            ),
        }
    }
}

impl Refusal {
    /// **The key it is about** (#1292), one step per table or key, so the Settings tab links it
    /// to the setting that mends it. A `[[forge]]` host's is the array as a whole.
    pub fn key(&self) -> Vec<String> {
        let at = |key: &str| vec![TABLE.to_owned(), key.to_owned()];
        match self {
            Self::NotATable => vec![TABLE.to_owned()],
            Self::UnknownKey(key) => at(key),
            Self::ModeOff | Self::ModeUnknown => at("mode"),
            Self::EgressNotAList | Self::EgressUnknown(_) => at("egress"),
            Self::HostsNotAList | Self::Host(..) => at(hosts::KEY),
            Self::BrowserSitesNotAList | Self::BrowserSite(..) => at(browser_sites::KEY),
            Self::CertificateChecksNotABool => at(CERTIFICATE_CHECKS),
            Self::PolicyUnknown => at(team::KEY),
            Self::Persona(said) => said.key.clone().unwrap_or_else(|| at(persona::KEY)),
            Self::ForgeHost(..) => vec!["forge".to_owned()],
        }
    }
}

/// Each `[[forge]]` host of `top` that is a host at all ([`crate::forge::host_ok`]), as written,
/// with what the sandbox makes of it without its port: taken as a project's own hosts are
/// (#1341), so never this machine, a link-local or metadata address, or a name ending in a
/// number. A refused one is never let through ([`Plane::forge_hosts`]) and is said
/// ([`Refusal::ForgeHost`], #1405).
fn forge_hosts_of(top: Option<&toml::Table>) -> Vec<(String, Result<hosts::Host, String>)> {
    let Some(forges) = top
        .and_then(|top| top.get("forge"))
        .and_then(toml::Value::as_array)
    else {
        return Vec::new();
    };
    forges
        .iter()
        .filter_map(|forge| forge.get("host")?.as_str())
        .filter(|host| crate::forge::host_ok(host))
        .map(|host| {
            let bare = host.split(':').next().unwrap_or(host);
            (host.to_owned(), hosts::Host::parse(bare))
        })
        .collect()
}

/// What a plane's `charter.toml` says about the sandbox: the policy where it is on, and each
/// thing it said that charter does not honour as written.
///
/// **Absent is not "off".** A plane that has no `[sandbox]`, or no `mode` in it, runs its chats
/// as it did before the sandbox existed, until the operator takes the one-time offer (ruling
/// V21, 1). **Anything else that is not `"on"` is read as `"on"`**, with a refusal: a committed
/// file can never loosen what a chat is confined to, and a typo must not do it by accident.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Said {
    pub policy: Option<Policy>,
    pub refused: Vec<Refusal>,
    /// What its `[sandbox]` says where it has **not** turned the sandbox on (a table with no
    /// `mode`): what its chats run under where an administrator's policy requires the sandbox
    /// ([`Plane::in_force`], D-1423-1). `None` where the sandbox is on, and where there is no
    /// `[sandbox]` at all.
    pub unset: Option<Policy>,
}

impl Said {
    /// The sandbox policy in force under `locks` ([`Plane::in_force`]): its own where it turned
    /// the sandbox on, or what it says with the mode on where policy requires the sandbox.
    pub fn in_force(self, locks: &policy::Locks) -> Option<Policy> {
        self.policy.or_else(|| {
            locks
                .forbids_opt_out()
                .then(|| self.unset.unwrap_or_else(Policy::defaults))
        })
    }

    /// What `top`, a parsed `charter.toml`, says.
    pub fn of(top: Option<&toml::Table>) -> Self {
        Self::of_known(top, None)
    }

    /// [`Self::of`], with `known`, the personas the project defines: a grant under
    /// `[sandbox.personas.<name>]` for any other is refused ([`persona::read`], #1407).
    pub fn of_known(top: Option<&toml::Table>, known: Option<&[String]>) -> Self {
        let Some(table) = top.and_then(|top| top.get(TABLE)) else {
            return Self::default();
        };
        let Some(table) = table.as_table() else {
            return Self {
                policy: Some(Policy::defaults()),
                refused: vec![Refusal::NotATable],
                unset: None,
            };
        };
        let mut refused = Vec::new();
        // The team's say (#2071 PR C): `policy`, read first, as `mode = "off"` is refused under
        // `never`. A policy that cannot be read is `never`, the stricter.
        let policy = team::Policy::of_value(table.get(team::KEY)).unwrap_or_else(|stricter| {
            refused.push(Refusal::PolicyUnknown);
            stricter
        });
        let on = match table.get("mode") {
            None => false,
            Some(toml::Value::String(word)) if word == "on" => true,
            // The team's off ([`team`]): it reaches each person on their Accept, and the
            // project's policy is still read for a person who keeps the full sandbox.
            Some(toml::Value::String(word)) if word == "off" => {
                if policy == team::Policy::Never {
                    refused.push(Refusal::ModeOff);
                }
                true
            }
            Some(_) => {
                refused.push(Refusal::ModeUnknown);
                true
            }
        };
        for key in table.keys() {
            if key != "mode"
                && key != "egress"
                && key != hosts::KEY
                && key != browser_sites::KEY
                && key != CERTIFICATE_CHECKS
                && key != persona::KEY
                && key != services::EXPECTS
                && key != team::KEY
            {
                refused.push(Refusal::UnknownKey(key.clone()));
            }
        }
        let egress = match table.get("egress") {
            None => Preset::DEFAULT.to_vec(),
            Some(toml::Value::Array(words)) => {
                let mut egress = Vec::new();
                for word in words {
                    match word.as_str().and_then(Preset::of_word) {
                        Some(preset) if !egress.contains(&preset) => egress.push(preset),
                        Some(_) => {}
                        None => refused.push(Refusal::EgressUnknown(word.to_string())),
                    }
                }
                egress
            }
            Some(_) => {
                refused.push(Refusal::EgressNotAList);
                Preset::DEFAULT.to_vec()
            }
        };
        let hosts = match hosts::read(table.get(hosts::KEY)) {
            Ok(hosts::Listed {
                hosts,
                refused: not,
            }) => {
                refused.extend(
                    not.into_iter()
                        .map(|(written, why)| Refusal::Host(written, why)),
                );
                hosts
            }
            Err(hosts::NotAList) => {
                refused.push(Refusal::HostsNotAList);
                Vec::new()
            }
        };
        // The project's browser sites (#2443): never a host of the chats' own, so only said here.
        match hosts::read(table.get(browser_sites::KEY)) {
            Ok(listed) => refused.extend(
                listed
                    .refused
                    .into_iter()
                    .map(|(written, why)| Refusal::BrowserSite(written, why)),
            ),
            Err(hosts::NotAList) => refused.push(Refusal::BrowserSitesNotAList),
        }
        // On unless the file says `false` (R1): what cannot be read is refused and read as off,
        // the narrow value.
        let certificate_checks = match table.get(CERTIFICATE_CHECKS) {
            None => true,
            Some(toml::Value::Boolean(on)) => *on,
            Some(_) => {
                refused.push(Refusal::CertificateChecksNotABool);
                false
            }
        };
        let (personas, not) = persona::keyed(table.get(persona::KEY), FILE, known);
        refused.extend(not.into_iter().map(Refusal::Persona));
        // The forge preset lets a project's `[[forge]]` hosts through, but not one the sandbox
        // refuses: said here, while the sandbox and that preset are on (#1405).
        if on && egress.contains(&Preset::Forge) {
            refused.extend(
                forge_hosts_of(top)
                    .into_iter()
                    .filter_map(|(written, host)| Some(Refusal::ForgeHost(written, host.err()?))),
            );
        }
        let written = Policy {
            egress,
            hosts,
            certificate_checks,
            personas,
        };
        let (policy, unset) = if on {
            (Some(written), None)
        } else {
            (None, Some(written))
        };
        Self {
            policy,
            refused,
            unset,
        }
    }
}

/// Everything in `text`'s `[sandbox]` that charter would not honour as written, as `file`
/// holds it — for the Settings tab's save, which refuses to write it. The committed file's
/// whole table, and this machine's `charter.local.toml`, which holds the person's own hosts and
/// nothing else of the sandbox ([`hosts`], #1341).
pub fn refusals(text: &str, file: &str) -> Vec<String> {
    whys(keyed_of(text, file, None))
}

/// [`refusals`] for the project at `root`, which also refuses a grant for a persona it does not
/// define ([`Said::of_known`], #1407): what the Settings tab's save asks.
pub fn refusals_at(root: &Path, text: &str, file: &str) -> Vec<String> {
    whys(keyed_at(root, text, file))
}

/// [`refusals_at`], each with the key it is about (#1292).
pub fn keyed_at(root: &Path, text: &str, file: &str) -> Vec<crate::settings::Refusal> {
    keyed_of(text, file, Some(&crate::personaverbs::names(root)))
}

fn whys(keyed: Vec<crate::settings::Refusal>) -> Vec<String> {
    keyed.into_iter().map(|one| one.why).collect()
}

fn keyed_of(text: &str, file: &str, known: Option<&[String]>) -> Vec<crate::settings::Refusal> {
    if file == crate::profiles::LOCAL_FILE {
        return local_refusals(text);
    }
    if file != FILE {
        return Vec::new();
    }
    let top = text.parse::<toml::Table>().ok();
    Said::of_known(top.as_ref(), known)
        .refused
        .iter()
        .map(|one| crate::settings::Refusal {
            why: one.to_string(),
            key: Some(one.key()),
        })
        .collect()
}

/// What a chat's sandbox always denies (ADR 0067 §5). Classes, not a list of paths: each is
/// turned into rules by [`Denied::of`], and no plane, persona or preset can remove one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Class {
    /// Chats never read a vault directly: every provider's storage and local session.
    Vaults,
    /// Charter's integrity state, which `charterd` alone writes.
    Integrity,
    /// What a person holds that a chat must not: the approvals and scopes behind them.
    HumanPowers,
    /// On a runner, `charterd`'s install files and the git internals it operates on.
    RunnerInternals,
    /// What a program run later, outside any sandbox, loads code or settings from: git's
    /// config and hooks, shell startup files, and each harness's and editor's project config
    /// ([`PLANTED`], ruling V73b), never written by a chat wherever it may write; and the
    /// project's manifests where they could change a chat's sandbox ([`manifests_held`]).
    LaterCode,
}

impl Class {
    pub const ALL: [Class; 5] = [
        Class::Vaults,
        Class::Integrity,
        Class::HumanPowers,
        Class::RunnerInternals,
        Class::LaterCode,
    ];

    /// The word a refusal and the audit name the class by.
    pub fn word(self) -> &'static str {
        match self {
            Self::Vaults => "vaults",
            Self::Integrity => "integrity",
            Self::HumanPowers => "human-powers",
            Self::RunnerInternals => "runner-internals",
            Self::LaterCode => "later-code",
        }
    }
}

/// How much of a [`Planted`] name a chat is denied writing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// The name itself: it is never created, renamed, moved or removed. What is below it is
    /// written as usual. A directory moved into its place would bring its contents with it.
    Itself,
    /// The name and everything below it.
    AndBelow,
}

/// One name, at any depth below a directory a chat may write, that the [`Class::LaterCode`]
/// class denies writing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Planted {
    /// The name, as `/`-separated parts, relative to the directory it is found in. A `**` part
    /// stands for any number of directories, as in a submodule's git directory.
    pub path: &'static str,
    pub reach: Reach,
}

impl Planted {
    const fn itself(path: &'static str) -> Self {
        Self {
            path,
            reach: Reach::Itself,
        }
    }

    const fn and_below(path: &'static str) -> Self {
        Self {
            path,
            reach: Reach::AndBelow,
        }
    }
}

/// What a program run later, outside any sandbox, loads code or settings from (ADR 0067 §5,
/// class 5, ruling V73b): git's config and hooks, in every clone and worktree; shell startup
/// files; and the project config of every harness and editor. A chat writing one of these could
/// have code run outside its sandbox the next time a person, an editor or another chat opens
/// the directory. The project's manifests are held only where they could change a chat's
/// sandbox ([`MANIFESTS`]).
pub const PLANTED: [Planted; 77] = [
    // A `.git` moved into place brings its own config and hooks.
    Planted::itself(".git"),
    Planted::and_below(".git/config"),
    Planted::and_below(".git/config.worktree"),
    Planted::and_below(".git/hooks"),
    Planted::and_below(".git/worktrees"),
    // What moves git's config, objects or attributes somewhere else, so a chat cannot point a
    // repository the app runs git in at a config of its own (#1335, D-1335-7). A linked
    // worktree's pointer files live under `.git/worktrees`, above, and its `.git` file is
    // `.git` itself.
    Planted::and_below(".git/commondir"),
    Planted::and_below(".git/objects/info/alternates"),
    Planted::and_below(".git/info/attributes"),
    // Each submodule's git directory, nested at any depth; and the directory that holds them,
    // so it is never moved into place whole.
    Planted::itself(".git/modules"),
    Planted::and_below(".git/modules/**/config"),
    Planted::and_below(".git/modules/**/config.worktree"),
    Planted::and_below(".git/modules/**/hooks"),
    Planted::and_below(".git/modules/**/commondir"),
    Planted::and_below(".git/modules/**/objects/info/alternates"),
    Planted::and_below(".git/modules/**/info/attributes"),
    // Hook managers' directories, which a `core.hooksPath` commonly names (ruling V73d).
    Planted::and_below(".husky"),
    Planted::and_below(".githooks"),
    Planted::itself(".claude"),
    Planted::and_below(".claude/settings.json"),
    Planted::and_below(".claude/settings.local.json"),
    Planted::and_below(".claude/commands"),
    Planted::and_below(".claude/agents"),
    // A skill runs its `!` commands when invoked and registers its frontmatter hooks (#1057).
    Planted::and_below(".claude/skills"),
    Planted::and_below(".mcp.json"),
    Planted::and_below("opencode.json"),
    Planted::and_below("opencode.jsonc"),
    Planted::and_below(".opencode"),
    // opencode's interface config, whose `plugin` key loads code (#1057).
    Planted::and_below("tui.json"),
    Planted::and_below("tui.jsonc"),
    Planted::and_below(".codex"),
    // The skills and plugins Codex reads from a project, which can start code outside any
    // sandbox (#1057).
    Planted::and_below(".agents"),
    Planted::and_below(".vscode"),
    Planted::and_below(".idea"),
    Planted::and_below(".envrc"),
    Planted::and_below(".profile"),
    Planted::and_below(".bashrc"),
    Planted::and_below(".bash_profile"),
    Planted::and_below(".bash_login"),
    Planted::and_below(".zshrc"),
    Planted::and_below(".zshenv"),
    Planted::and_below(".zprofile"),
    Planted::and_below(".zlogin"),
    Planted::and_below(".zlogout"),
    Planted::and_below(".bash_logout"),
    Planted::and_below(".config/fish"),
    // What runs later at the person's own level (#2071): at the floor every folder is written,
    // the home among them. Git's own config, which purlis's own git reads outside any sandbox;
    // ssh's; what macOS starts at login; each harness's user-level config and hooks; and each
    // editor's user settings and tasks.
    Planted::and_below(".gitconfig"),
    Planted::and_below(".config/git"),
    // ssh's own: what it runs, and who it lets in. Its `known_hosts` stays the chat's to add to,
    // as ssh does on a first connection; the folder itself is never moved aside.
    Planted::itself(".ssh"),
    Planted::and_below(".ssh/config"),
    Planted::and_below(".ssh/config.d"),
    Planted::and_below(".ssh/authorized_keys"),
    Planted::and_below(".ssh/authorized_keys2"),
    Planted::and_below(".ssh/rc"),
    Planted::and_below(".ssh/environment"),
    Planted::and_below("Library/LaunchAgents"),
    Planted::and_below(".claude.json"),
    Planted::and_below(".claude/hooks"),
    Planted::and_below(".claude/plugins"),
    Planted::and_below(".config/opencode"),
    Planted::and_below("Library/Application Support/Code/User"),
    Planted::and_below("Library/Application Support/Cursor/User"),
    Planted::and_below("Library/Application Support/JetBrains"),
    Planted::and_below(".cursor"),
    Planted::and_below(".config/zed"),
    // What a Linux session starts or loads at login, and each editor's user settings there
    // (#1040): the same classes as macOS's above.
    Planted::and_below(".config/systemd/user"),
    Planted::and_below(".local/share/systemd/user"),
    Planted::and_below(".config/autostart"),
    Planted::and_below(".config/environment.d"),
    Planted::and_below(".config/plasma-workspace/env"),
    Planted::and_below(".local/share/applications"),
    Planted::and_below(".pam_environment"),
    Planted::and_below(".xprofile"),
    Planted::and_below(".xsessionrc"),
    Planted::and_below(".xinitrc"),
    Planted::and_below(".config/Code/User"),
    Planted::and_below(".config/Cursor/User"),
    Planted::and_below(".config/JetBrains"),
];

/// The project's manifests, under both names (RN-2a): `purlis.toml`, which turns the sandbox
/// on, and `purlis.local.toml`, what the machine adds to the project and purlis reads at every
/// later start (the variables passed to chats, plugins and extensions).
///
/// Not [`PLANTED`]: a manifest is held only where it could change a chat's sandbox
/// ([`manifests_held`]), and elsewhere, in a clone's fixtures or a temp folder, it is an
/// ordinary file (ADR 0067 §5 as amended on 2026-10-06).
pub const MANIFESTS: [&str; 4] = [
    crate::names::PLANE_MANIFEST.write,
    crate::names::PLANE_MANIFEST.reads[0],
    crate::names::LOCAL_SETTINGS.write,
    crate::names::LOCAL_SETTINGS.reads[0],
];

/// The manifests a chat of the project at `root`, working in `cwd`, is denied writing (ADR 0067
/// §5 as amended on 2026-10-06): every [`MANIFESTS`] name at the project root and in each
/// folder from the chat's own up to the root, both ends included. Those are the files purlis
/// reads when it starts a chat there. With no `cwd`, or one outside the project, only the
/// root's.
///
/// Each as the folder is spelled and as the kernel names the folder, the name itself never
/// resolved: a manifest that is a link (a `purlis.local.toml` kept in a dotfiles repository) is
/// held by its own path, so it is neither removed nor replaced, as well as by its target.
///
/// **A manifest lower down changes no chat's sandbox.** A chat's project is the root its sandbox
/// was compiled for ([`Applied::root`]), never the nearest manifest to its folder, and every
/// reader of the project's settings reads them at that root. A marker the walk finds inside
/// another project's `workspaces/` re-roots to that project (`plane::outermost`), so a manifest
/// a chat writes in a clone's fixtures names no project of its own.
pub fn manifests_held(root: &Path, cwd: Option<&Path>) -> Vec<Denial> {
    let real_root = real(root);
    let mut folders = vec![root.to_path_buf()];
    if let Some(cwd) = cwd {
        let mut chain = Vec::new();
        for folder in cwd.ancestors() {
            chain.push(folder.to_path_buf());
            if real(folder) == real_root {
                folders.extend(chain);
                break;
            }
        }
    }
    let mut out: Vec<Denial> = Vec::new();
    for folder in folders {
        for spelled in [real(&folder), folder] {
            for name in MANIFESTS {
                let denial = Denial {
                    class: Class::LaterCode,
                    path: spelled.join(name),
                    access: Access::Write,
                    named: None,
                    reach: Reach::AndBelow,
                    any_case: false,
                };
                if !out.contains(&denial) {
                    out.push(denial);
                }
            }
        }
    }
    out
}

/// **The skills of the project at `root` a chat is denied writing** (SK-9, #2772; ADR 0099 as
/// amended 2026-10-22): every plane level's folder of skills, shared and local, at the project's
/// root, in each workspace and in each persona's folder, whether or not it is there yet, and in
/// any workspace or persona folder made later. A skill runs its `!` commands when it is used and
/// reaches other chats, as `.claude/skills` does ([`PLANTED`]), so a chat's write of one becomes
/// a draft skill the person tries and accepts ([`crate::skills::verbs::drafts`]).
///
/// The folders there now are held by their paths, and every workspace's and persona's, there now
/// or made later, by a pattern too, a path with a [`ANY_NAME`] part, which Claude Code reads as a glob and purlis's own
/// wrap as a regular expression ([`seatbelt::denial_rules`]). Where a backend cannot hold a
/// pattern, the levels a chat could have made reach no chat while it runs
/// ([`crate::skills::levels`]). A clone is never named `skills` or `skills.local`
/// (`repocmd::clone::destination`), so no pattern holds a clone's root.
pub fn skills_held(root: &Path) -> Vec<Denial> {
    let levels = [purlis_skills::PROJECT_DIR, purlis_skills::LOCAL_DIR];
    let mut owners: Vec<PathBuf> = vec![root.to_path_buf()];
    for ws in crate::workspaces::Plane::open(root)
        .workspaces()
        .unwrap_or_default()
    {
        owners.push(root.join("workspaces").join(ws));
    }
    if let Ok(personas) = std::fs::read_dir(root.join("personas")) {
        let mut found: Vec<PathBuf> = personas
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .map(|entry| entry.path())
            .collect();
        found.sort();
        owners.extend(found);
    }
    // Any level, made now or later, by its pattern too.
    owners.push(root.join("workspaces").join(ANY_NAME));
    owners.push(root.join("personas").join(ANY_NAME));
    owners
        .iter()
        .flat_map(|owner| levels.iter().map(move |level| owner.join(level)))
        .map(|path| Denial {
            class: Class::LaterCode,
            path,
            access: Access::Write,
            named: None,
            reach: Reach::AndBelow,
            any_case: false,
        })
        .collect()
}

/// **A part of a denied path that stands for any one name** (SK-9): `workspaces/*/skills` holds
/// the skills of every workspace, made now or later. Only [`skills_held`] writes one.
pub const ANY_NAME: &str = "*";

/// A workspace's own manifest, `workspaces/<workspace>/workspace.json` (#2046): what the app
/// reads of the workspace (its repos, its settings, what its sends to a runner leave out). A
/// chat's sandbox never writes it, as it never writes the project's manifests; the app writes
/// it for the chat's asks that need it (ADR 0067 §2), and the person changes the rest.
pub const WORKSPACE_MANIFEST: &str = crate::settings::workspace::FILE;

/// The folder the project's workspaces are in.
pub const WORKSPACES: &str = "workspaces";

/// **Every workspace's manifest of the project at `root`** a chat is denied writing (#2046):
/// [`WORKSPACE_MANIFEST`] in each folder of `workspaces/`, as the folder is spelled and as the
/// kernel names it, under every spelling of the name a case-folding disk takes for it.
pub fn workspace_manifests_held(root: &Path) -> Vec<Denial> {
    let mut out: Vec<Denial> = Vec::new();
    for spelled in [real(root), root.to_path_buf()] {
        let Ok(entries) = std::fs::read_dir(spelled.join(WORKSPACES)) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let denial = Denial {
                class: Class::LaterCode,
                path: entry.path().join(WORKSPACE_MANIFEST),
                access: Access::Write,
                named: None,
                reach: Reach::AndBelow,
                any_case: true,
            };
            if !out.contains(&denial) {
                out.push(denial);
            }
        }
    }
    out
}

/// Whether this process runs in a chat the app started under a sandbox
/// ([`crate::hookwire::SANDBOXED_ENV`] is `1`): a tool hook of that chat inherits it.
pub fn chat_is_sandboxed() -> bool {
    chat_is_sandboxed_in(&crate::envvar::var)
}

/// [`chat_is_sandboxed`], asking `env`.
pub fn chat_is_sandboxed_in(env: &dyn Fn(&str) -> Option<String>) -> bool {
    env(crate::hookwire::SANDBOXED_ENV).as_deref() == Some("1")
}

/// What started this process: a command the chat ran, or the harness itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Started {
    /// A command the chat ran, in its shell or tool: what a chat's sandbox confines first.
    ByTheChat,
    /// The harness itself, for one of its hooks or as its MCP server. Inside the chat's sandbox
    /// only where the harness's adapter says its sandbox holds what it starts
    /// ([`crate::harness::adapter::HarnessAdapter::sandbox_holds_what_it_starts`]).
    ByTheHarness,
}

/// Set once, at the start of a hook or the MCP server ([`started_by_the_harness`]).
static STARTED_BY_THE_HARNESS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Says that this process is one the harness itself started, a hook or its MCP server: the
/// entry of `purlis hook` and `purlis mcp` calls it before anything is written (#1421).
pub fn started_by_the_harness() {
    STARTED_BY_THE_HARNESS.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// [`Started`], for this process.
pub fn started() -> Started {
    if STARTED_BY_THE_HARNESS.load(std::sync::atomic::Ordering::Relaxed) {
        Started::ByTheHarness
    } else {
        Started::ByTheChat
    }
}

/// Whether a write this process makes is held to its chat's sandbox, so a refusal of it may be
/// told as the sandbox's (#1345, #1421). [`chat_is_sandboxed`] alone says only that the chat
/// has one: a hook inherits that from its chat, and Claude Code's sandbox confines the commands
/// its Bash tool runs and not its hooks, so a hook there that meets `EPERM` met something else.
pub fn writes_are_sandboxed() -> bool {
    writes_are_sandboxed_in(&crate::envvar::var, started())
}

/// [`writes_are_sandboxed`], asking `env`, for a process `started` so. A harness this binary
/// does not know, or none named, is not taken to hold what it starts: a refusal is then told
/// without blaming a sandbox, which is the side that is never untrue.
pub fn writes_are_sandboxed_in(env: &dyn Fn(&str) -> Option<String>, started: Started) -> bool {
    if !chat_is_sandboxed_in(env) {
        return false;
    }
    match started {
        Started::ByTheChat => true,
        Started::ByTheHarness => crate::hookwire::chat_harness(env)
            .as_deref()
            .and_then(Harness::of_registry_or_kind)
            .is_some_and(|harness| harness.adapter().sandbox_holds_what_it_starts()),
    }
}

/// What a denied path is denied for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Neither read nor written.
    ReadWrite,
    /// Read, but never written.
    Write,
}

/// One path a chat is denied, and the class it is denied for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Denial {
    pub class: Class,
    pub path: PathBuf,
    pub access: Access,
    /// The file and the word in it that named the path, for one a config named
    /// ([`Class::LaterCode`]); `None` for one charter names itself.
    pub named: Option<Named>,
    /// How much of `path` is held: it and everything below it, or the entry alone, which is
    /// never created, renamed, moved or removed while what is below it is written (#2752).
    pub reach: Reach,
    /// Whether it is held under every spelling of its name a case-folding disk takes for it,
    /// as well as its own: a folder that is not there yet is made under the name as typed
    /// (#2752).
    pub any_case: bool,
}

/// Where a config named a denied path: the file, and the word in it as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    pub file: PathBuf,
    pub word: String,
}

/// A class that is held by a service rather than a path: the compiler of each harness either
/// denies the service or cannot, and a harness that cannot is refused (ADR 0067 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    /// The operating system's credential store, where a `keyring` vault keeps its secrets
    /// (ADR 0047). Reached through a system service, never through a file a rule can name.
    CredentialStore,
}

impl Service {
    /// The class it is denied for.
    pub fn class(self) -> Class {
        match self {
            Self::CredentialStore => Class::Vaults,
        }
    }
}

/// The operating system a chat runs on, which decides the backend and what it can express.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    MacOs,
    Linux,
    Windows,
    Other,
}

impl Os {
    /// Whether purlis has a sandbox backend on this system at all. Where it has none (Windows),
    /// a chat in a project that turns the sandbox on starts unsandboxed and says why
    /// ([`By::NoBackend`]), so nothing about a sandbox is checked for it.
    pub fn has_backend(self) -> bool {
        self != Self::Windows
    }

    /// The word a run's `sandbox` attribute names this system's backend by (ADR 0066, #868):
    /// Seatbelt on macOS; on Linux, Claude Code's own bubblewrap, or `landlock` for purlis's own
    /// wrap of a harness it holds whole (Codex, opencode; #1040). `None` where purlis has none,
    /// or does not know which.
    pub fn backend_word(self, harness: Option<Harness>) -> Option<&'static str> {
        match self {
            Self::MacOs => Some("seatbelt"),
            Self::Linux if harness.is_some_and(|it| self.purlis_wraps(it)) => Some("landlock"),
            Self::Linux => Some("bubblewrap"),
            Self::Windows | Self::Other => None,
        }
    }

    /// Whether a chat of `harness` here starts inside purlis's own Linux wrap (#1040): a harness
    /// purlis holds whole (Codex, opencode), on Linux.
    pub fn purlis_wraps(self, harness: Harness) -> bool {
        self == Self::Linux && harness.adapter().sandbox_holds_what_it_starts()
    }

    /// This machine's.
    pub fn this() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Other
        }
    }
}

/// What of this machine the denial classes are resolved against.
#[derive(Debug, Clone)]
pub struct Machine {
    /// The environment charter runs in: `$CHARTER_HOME`, `$CHARTER_CONFIG_HOME`,
    /// `$XDG_CONFIG_HOME` and `$OP_CONFIG_DIR` move what is denied with them.
    pub env: crate::secrets::Env,
    pub home: Option<PathBuf>,
    pub os: Os,
}

impl Machine {
    /// This machine, as charter's own process sees it.
    pub fn this() -> Self {
        Self {
            env: crate::secrets::Env::from_process(),
            home: dirs::home_dir(),
            os: Os::this(),
        }
    }
}

/// Everything a chat in one plane is denied, resolved to this machine.
///
/// Each class is resolved to the state charter keeps for it on this machine, and a change that
/// adds state to a class adds it here, with its class's test naming it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Denied {
    pub paths: Vec<Denial>,
    pub services: Vec<Service>,
    /// A config whose commands purlis stopped reading ([`planted::Unread`]), so what they run
    /// is not known and the chat is refused ([`NotStarted::Unread`]).
    pub unread: Option<Named>,
    /// Why what purlis keeps from a chat could not be told whole ([`kept_from_reading`]): the
    /// paths may be short of a vault's storage, so no sandbox is compiled from them and the
    /// chat is refused ([`NotStarted::KeptUntold`]). `None` when the list is whole.
    pub untold: Option<Untold>,
}

impl Denied {
    /// **These denials, and what waits to be told to a chat on its next turn**
    /// (`<state>/handbacks/`, D-T59-19): the reports of the chats it dispatched, and purlis's
    /// own word on a dispatch the person was asked about (#1436, #1437). Neither read nor
    /// written, under every spelling of the state folder.
    ///
    /// **Only for a harness whose hooks run outside its sandbox**
    /// ([`crate::harness::adapter::HarnessAdapter::sandbox_holds_what_it_starts`] says no).
    /// The app leaves those files and purlis's hooks take them, reading and then removing
    /// each. Where the sandbox confines the harness's tools and not its hooks, nothing inside
    /// it needs the folder, and a chat that could write there could put a line in purlis's
    /// voice into another chat's turn. Where the sandbox is a wrap around the whole harness,
    /// its hooks run inside it, and denying the folder would cut every report off: there it
    /// stays as it was, until delivery moves onto the hook socket (#1457), and what stands is
    /// the check each file gets as it is read ([`crate::handback`]).
    #[must_use]
    pub fn and_what_waits_for_a_chat(mut self, root: &Path) -> Self {
        for state in crate::names::STATE_DIR.spellings() {
            self.paths.push(Denial {
                class: Class::Integrity,
                path: root.join(state).join(crate::handback::DIR_NAME),
                access: Access::ReadWrite,
                named: None,
                reach: Reach::AndBelow,
                any_case: false,
            });
        }
        self
    }

    /// The denials for a chat in the plane at `root`, on `machine`.
    pub fn of(root: &Path, machine: &Machine) -> Self {
        let KeptList {
            mut paths,
            keyring,
            untold,
            ..
        } = kept_by_purlis(root, machine);

        // …and this machine's policy (#1343): what an administrator locks is never a chat's to
        // write, wherever the system lets it be written.
        paths.push(Denial {
            class: Class::HumanPowers,
            path: real(&policy::machine_folder()),
            access: Access::Write,
            named: None,
            reach: Reach::AndBelow,
            any_case: false,
        });

        // 5. Later code: the project root's manifests, under every name (#1336); those between
        // the root and the chat's folder are added where the folder is known
        // ([`Applied::form_in`]).
        paths.extend(manifests_held(root, None));
        // …and every workspace's manifest (#2046): the app reads a workspace's repos and
        // settings from it, so only the app and the person write it.
        paths.extend(workspace_manifests_held(root));
        // …and the project's harness declarations (#1458): the app starts a declared harness's
        // program from them, outside any sandbox, once the person approved what they say.
        paths.push(Denial {
            class: Class::LaterCode,
            path: root.join(crate::harness_declaration::DIR),
            access: Access::Write,
            named: None,
            reach: Reach::AndBelow,
            any_case: false,
        });
        // …and the project's skills, wherever they reach chats (SK-9,
        // #2772): a chat's write of one is a draft the person accepts, never the file itself.
        if crate::skills::reach_chats_here(root) {
            paths.extend(skills_held(root));
        }
        // …and where Docker Desktop, OrbStack and Rancher Desktop link `docker` and its
        // credential helpers in the home folder (#2752), which purlis runs outside any sandbox
        // for a Docker pool ([`crate::dockerpool::Docker::on_this_machine`]): each `bin` folder
        // and what is in it, and the folder that holds it as an entry alone, so it is never
        // moved aside for one that brings a `docker` of its own (the person's ruling of
        // 2026-10-20). Under every spelling a case-folding disk takes, since a folder that is
        // not there yet is made under the name as typed. Paths of the home alone: a project's
        // own `.docker` stays the chat's, and so does what is beside each `bin`.
        if let Some(home) = &machine.home {
            // …and where each installer keeps its engine's socket in the home (D-2752-7),
            // which purlis points each run of `docker` at only where no chat can put another
            // socket in its place ([`crate::dockerpool::settings::trusted`]).
            for held in crate::dockerpool::settings::HELD_IN_HOME {
                paths.push(Denial {
                    class: Class::LaterCode,
                    path: home.join(held),
                    access: Access::Write,
                    named: None,
                    reach: Reach::AndBelow,
                    any_case: true,
                });
            }
            for folder in crate::dockerpool::DOCKER_HOME_FOLDERS {
                let bin = home.join(folder);
                let holder = bin.parent().map(Path::to_path_buf);
                for (path, reach) in [(Some(bin), Reach::AndBelow), (holder, Reach::Itself)] {
                    let Some(path) = path else { continue };
                    paths.push(Denial {
                        class: Class::LaterCode,
                        path,
                        access: Access::Write,
                        named: None,
                        reach,
                        any_case: true,
                    });
                }
            }
        }
        // …and where a Podman machine's helper and a rootless engine keep their socket, in the
        // temporary and the runtime folder (D-2752-7).
        let named = |name: &str| {
            machine
                .env
                .get(name)
                .map(PathBuf::from)
                .filter(|folder| folder.is_absolute())
        };
        let in_tmp = named("TMPDIR")
            .into_iter()
            .map(|tmp| tmp.join(crate::dockerpool::settings::HELD_IN_TMP));
        let in_runtime = named("XDG_RUNTIME_DIR").into_iter().flat_map(|runtime| {
            crate::dockerpool::settings::HELD_IN_RUNTIME
                .iter()
                .map(move |at| runtime.join(at))
        });
        for path in in_tmp.chain(in_runtime) {
            paths.push(Denial {
                class: Class::LaterCode,
                path,
                access: Access::Write,
                named: None,
                reach: Reach::AndBelow,
                any_case: machine.os == Os::MacOs,
            });
        }
        // …and Podman's own folders (#2680, the person's ruling of 2026-10-20): its settings,
        // which name the programs it runs and the engine it talks to, and its storage and run
        // state, which are a rootless Podman's engine itself. A chat writes none of them, under
        // every spelling on macOS's case-folding disk.
        for (folder, reach) in podman_folders(machine) {
            paths.push(Denial {
                class: Class::LaterCode,
                path: folder,
                access: Access::Write,
                named: None,
                reach,
                any_case: machine.os == Os::MacOs,
            });
        }
        // …and what a protected config points at elsewhere, resolved now (V73d).
        let xdg_config = machine
            .env
            .get("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|dir| dir.is_absolute())
            .or_else(|| machine.home.as_ref().map(|home| home.join(".config")));
        // Where else a chat could write, so a value split off a later letter there is still
        // named (D-T56-1): every folder you list as one chats may be granted, which with the
        // project holds every grant (D-1342-10), and the project's cache home (#1337), whether
        // or not this chat is given it. Counting too many only names more, which fails closed.
        let writable: Vec<PathBuf> = local::grantable_folders(root)
            .into_iter()
            .chain(caches::root_of(machine, root))
            .collect();
        let (resolved, unread) = match planted::resolved(
            root,
            machine.home.as_deref(),
            xdg_config.as_deref(),
            &writable,
        ) {
            Ok(resolved) => (resolved, None),
            Err(unread) => (Vec::new(), Some(unread)),
        };
        for it in resolved {
            paths.push(Denial {
                class: Class::LaterCode,
                path: it.path,
                access: Access::Write,
                named: Some(Named {
                    file: it.file,
                    word: it.word,
                }),
                reach: Reach::AndBelow,
                any_case: false,
            });
        }

        Self {
            paths,
            services: if keyring {
                vec![Service::CredentialStore]
            } else {
                Vec::new()
            },
            unread,
            untold,
        }
    }
}

/// **What purlis keeps for itself, of a chat's denials** (classes 1 to 3: vaults, integrity and
/// human powers), resolved for the plane at `root` on `machine`, and whether a vault there
/// keeps its secrets in the credential store.
///
/// One list, read twice: [`Denied::of`] hands it to each harness's sandbox, and
/// [`kept_from_reading`] hands the part no chat reads to the window's own file reads, so the
/// two cannot drift apart.
///
/// **Whole, or said not to be** ([`KeptList::untold`]). Every entry of the vault registry, in
/// the committed half and in this machine's, is judged ([`judged`]): a vault of a kind purlis
/// knows resolves to where it keeps its secrets, which is in the list; an entry that names no
/// kind, or a kind this purlis does not know, has every location it names in the project in
/// the list, and Doctor is told ([`KeptList::notes`]). The list is not whole where the registry
/// does not read, where its `vaults` is not an object, where a vault of a known kind says where
/// its file is in a way purlis cannot use, or where a vault's file has other names and the
/// project holds more than purlis looks through for them. Neither reader goes on with a list
/// that is not whole: a chat does not start, and the window's file reads list nothing.
///
/// **A registry that is not there is whole**: a project with no vault registry, or no state
/// folder yet, registers no vault, and keeps what every project keeps. So is one whose
/// `plain-file` vault has no file on this machine yet: there is nothing of it here to keep.
fn kept_by_purlis(root: &Path, machine: &Machine) -> KeptList {
    let ctx = crate::secrets::Ctx::new(root, machine.env.clone());
    let mut paths = Vec::new();
    let mut deny = |class, path: PathBuf, access| {
        paths.push(Denial {
            class,
            path,
            access,
            named: None,
            reach: Reach::AndBelow,
            any_case: false,
        })
    };

    // 1. Vaults: every provider's storage and local session, wherever it is configured.
    deny(Class::Vaults, ctx.vaults_dir(), Access::ReadWrite);
    deny(
        Class::Vaults,
        crate::secrets::fingerprint::key_path(&ctx),
        Access::ReadWrite,
    );
    // …and under EVERY spelling of the state folder, there or not, as its other records are
    // below (RN-2a): which folder holds the vaults is decided by which are there.
    // The folder in use is one of the spellings unless `$PURLIS_HOME` moved it: named once.
    let in_use = [
        ctx.vaults_dir(),
        crate::secrets::fingerprint::key_path(&ctx),
    ];
    for state in crate::names::STATE_DIR.spellings() {
        for kept in ["vaults", crate::secrets::fingerprint::KEY_FILE] {
            let path = root.join(state).join(kept);
            if !in_use.contains(&path) {
                deny(Class::Vaults, path, Access::ReadWrite);
            }
        }
    }
    // Each half is read by itself, so every entry of either is judged, under any name.
    use crate::secrets::registry;
    let mut keyring = false;
    let mut unresolved: Option<Untold> = None;
    let mut notes: Vec<KeptNote> = Vec::new();
    let unread = |file: PathBuf| Untold {
        why: format!("the vault registry {} could not be read", file.display()),
        registry: Some(file),
    };
    let halves = match registry::load_shared(&ctx) {
        Err(_) => Err(unread(ctx.shared_registry())),
        Ok(shared) => match registry::load_local(&ctx) {
            Err(_) => Err(unread(ctx.local_registry())),
            Ok(local) => Ok((shared, local)),
        },
    };
    match halves {
        Err(why) => {
            keyring = true;
            unresolved = Some(why);
        }
        Ok((shared, local)) => {
            let files = [ctx.shared_registry(), ctx.local_registry()];
            // An entry under a name purlis refuses, judged where it is written.
            let mut entries: Vec<(String, serde_json::Map<String, serde_json::Value>, &PathBuf)> =
                Vec::new();
            for (half, file) in [&shared, &local].into_iter().zip(&files) {
                match half.get("vaults") {
                    None => {}
                    Some(serde_json::Value::Object(named)) => {
                        for (name, entry) in named {
                            if let (false, Some(entry)) =
                                (registry::name_ok(name), entry.as_object())
                            {
                                entries.push((name.clone(), entry.clone(), file));
                            }
                        }
                    }
                    Some(_) => {
                        unresolved.get_or_insert_with(|| Untold {
                            why: format!(
                                "'vaults' in the vault registry {} is not a list of vaults",
                                file.display()
                            ),
                            registry: Some(file.clone()),
                        });
                    }
                }
            }
            // And each vault of the two halves together, said to be in the half that says
            // where its file is.
            let doc = registry::merged(&shared, &local);
            for (name, entry) in registry::vaults(&doc) {
                let says_where = |half: &serde_json::Map<String, serde_json::Value>| {
                    half.get("vaults")
                        .and_then(|vaults| vaults.get(&name))
                        .is_some_and(|entry| entry.get("config").is_some())
                };
                let file = if says_where(&local) || !scope_has(&shared, &name) {
                    &files[1]
                } else {
                    &files[0]
                };
                if let Some(entry) = entry.as_object() {
                    entries.push((name, entry.clone(), file));
                }
            }
            // In one order, whatever order the files were written in.
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            for (name, entry, file) in entries {
                let name = crate::personas::one_line(&name);
                match judged(&ctx, &entry) {
                    Judged::Kept(places) => {
                        for place in places {
                            deny(Class::Vaults, place, Access::ReadWrite);
                        }
                    }
                    Judged::CredentialStore => keyring = true,
                    Judged::Elsewhere => {}
                    Judged::NotSetUp => notes.push(KeptNote::NotSetUp(name)),
                    Judged::LeftOver(places) => {
                        for place in places {
                            deny(Class::Vaults, place, Access::ReadWrite);
                        }
                        notes.push(KeptNote::LeftOver(name));
                    }
                    Judged::UnknownKind(kind, places) => {
                        for place in places {
                            deny(Class::Vaults, place, Access::ReadWrite);
                        }
                        notes.push(KeptNote::UnknownKind { name, kind });
                    }
                    Judged::Unusable => {
                        unresolved.get_or_insert_with(|| Untold {
                            why: format!(
                                "the vault '{name}' in the vault registry {} does not say where \
                                 its file is",
                                file.display()
                            ),
                            registry: Some(file.clone()),
                        });
                    }
                }
            }
        }
    }
    if let Some(home) = &machine.home {
        let op = machine
            .env
            .get("OP_CONFIG_DIR")
            .filter(|dir| !dir.is_empty())
            .map_or_else(|| home.join(".config/op"), PathBuf::from);
        deny(Class::Vaults, op, Access::ReadWrite);
        deny(Class::Vaults, home.join(".op"), Access::ReadWrite);
        deny(Class::Vaults, home.join(".vault-token"), Access::ReadWrite);
    }
    // The registry that names the vaults: readable, never rewritten.
    deny(Class::Vaults, ctx.shared_registry(), Access::Write);
    deny(Class::Vaults, ctx.local_registry(), Access::Write);

    // 2. Integrity: charter's own records, which only charter writes.
    // Under EVERY spelling of the state folder (RN-2a): a chat that could write
    // `.purlis/app` would be writing records charter may read next.
    for state in crate::names::STATE_DIR.spellings() {
        let app = root.join(state).join("app");
        deny(Class::Integrity, app.clone(), Access::Write);
        // Every chat's theme drafts (#2332): the app writes a chat's draft, in the folder of the
        // chat whose token asked, and no chat writes any. Read, never written.
        deny(
            Class::Integrity,
            root.join(state).join(crate::theme::drafts::DRAFTS),
            Access::Write,
        );
        // The skills a delete kept on this machine, and the notes of deletes that run (SK-7,
        // SK-35): only purlis writes either. A note a chat planted would be finished as a delete
        // the next time the studio opens.
        for kept in crate::skills::verbs::KEPT_BY_DELETES {
            deny(Class::Integrity, root.join(state).join(kept), Access::Write);
        }
        // Every chat's hook spool and the keys that check it, neither read nor written: the
        // hooks that write a spool run outside the sandbox their tools run in (ADR 0068 §6
        // as amended by V63), so nothing inside it needs them.
        deny(
            Class::Integrity,
            crate::hookwire::spool::dir_for(&app.join("hooks.sock")),
            Access::ReadWrite,
        );
        // Every report a chat is handed (#1436): written by the app alone, and read by the
        // hook of the chat it waits for, which may run inside the sandbox (Codex, opencode), so
        // read, never written. Claude Code's hooks run outside it, and its chats are kept from
        // reading it too (`Denied::and_what_waits_for_a_chat`).
        deny(
            Class::Integrity,
            root.join(state).join(crate::handback::DIR_NAME),
            Access::Write,
        );
        // Every dispatch's record, neither read nor written (#1452, D-1452-11): a brief or a
        // report written for one persona is not for a chat running as another. A chat gets
        // its own dispatch's report on the delivery path and its own list from the app's
        // answer, never from the file.
        deny(
            Class::Integrity,
            app.join(crate::dispatchrecord::DIR_NAME),
            Access::ReadWrite,
        );
        // Every chat's browser timeline (#2017), neither read nor written: a screenshot one
        // persona's chat was given is not for a chat running as another. A chat was handed
        // its own screenshots as its tools answered.
        deny(
            Class::Integrity,
            root.join(state).join(crate::browsertimeline::DIR),
            Access::ReadWrite,
        );
        // The person's approvals of a profile's command and of a project's harness
        // declaration (#1458): what lets purlis start that program, written by the app when
        // the person approves in the window and by nothing a chat runs. Read, never written.
        // And the person's approvals of a persona's credentialed MCP servers, written by
        // `purlis persona approve-mcp` in the person's terminal, never a chat's.
        for record in [
            crate::profiletrust::RECORD,
            crate::harness_declaration::APPROVED,
            crate::personaverbs::mcp::APPROVED_FILE,
        ] {
            deny(
                Class::Integrity,
                root.join(state).join(record),
                Access::Write,
            );
        }
        // Every chat's settings file (SK-9): written by purlis as the chat starts, and read by
        // its harness as the settings it runs with. Read, never written.
        deny(
            Class::Integrity,
            root.join(state).join(crate::hoststart::SETTINGS_DIR),
            Access::Write,
        );
    }
    // The MCP approvals are read where `$PURLIS_HOME` puts the state folder, when it does.
    if !crate::names::STATE_DIR
        .spellings()
        .any(|state| root.join(state) == ctx.state)
    {
        deny(
            Class::Integrity,
            crate::personaverbs::mcp::approvals_path(&ctx.state),
            Access::Write,
        );
        // …and the theme drafts (#2332), which the app writes there.
        deny(
            Class::Integrity,
            ctx.state.join(crate::theme::drafts::DRAFTS),
            Access::Write,
        );
        // …and what skill deletes keep there (SK-7, SK-35).
        for kept in crate::skills::verbs::KEPT_BY_DELETES {
            deny(Class::Integrity, ctx.state.join(kept), Access::Write);
        }
        // …and every chat's settings file, which a start writes there.
        deny(
            Class::Integrity,
            ctx.state.join(crate::hoststart::SETTINGS_DIR),
            Access::Write,
        );
    }
    // …and a state folder the project does not have as a folder is not the chat's to make
    // (D-RN2a-7): which folder holds charter's state is decided by which are there, so making
    // one is a move of that state. One that is there is left to the rows above.
    for state in crate::names::STATE_DIR.spellings() {
        let folder = root.join(state);
        // Not a directory of its own: absent, a file, or a link (which a chat could point
        // anywhere) — none is the chat's to turn into a state folder.
        if !std::fs::symlink_metadata(&folder).is_ok_and(|meta| meta.is_dir()) {
            deny(Class::Integrity, folder, Access::Write);
        }
    }

    // 3. Human powers: the approvals a person gave on this machine.
    let config_root = crate::machine::rooted(
        machine.env.get(crate::machine::HOME_VAR).map(Into::into),
        machine.env.get("XDG_CONFIG_HOME").map(Into::into),
        machine.home.clone(),
    );
    // Under EVERY spelling of the config home, there or not (RN-5): which one is the home
    // is decided by which is there, so a chat that could make `purlis/` beside `charter/`
    // would be writing the approvals charter reads next.
    if let Some(config_root) = config_root {
        for home in crate::machine::dirs_spelled(&config_root) {
            deny(Class::HumanPowers, home.clone(), Access::Write);
            // The human client scopes' credentials (FD-27, V16a): a chat neither reads nor
            // writes anything there, so it holds no person's credential. The host's socket
            // is in the runtime directory (below); a connect to it is network to a sandbox,
            // and each compiler allows no unix socket but the hook socket. The host also
            // refuses those scopes to a chat's processes. Every spelling of the folder.
            for daemon in crate::names::DAEMON_DIR.spellings() {
                deny(Class::HumanPowers, home.join(daemon), Access::ReadWrite);
            }
            // A runner's one peer (ADR 0078 §8, #2044): chats touch none of the link's
            // records, reading included.
            deny(
                Class::HumanPowers,
                home.join(crate::machine::PEERS_FILE),
                Access::ReadWrite,
            );
            // The runners this desktop pinned (ADR 0078 §8, #2525): denied to chats.
            deny(
                Class::HumanPowers,
                home.join(crate::machine::RUNNERS_FILE),
                Access::ReadWrite,
            );
            // When each of those runners ran (#3492): denied as they are.
            deny(
                Class::HumanPowers,
                home.join(crate::machine::RUNNER_HOURS_FILE),
                Access::ReadWrite,
            );
            // What the human's forge sign-in fetched: the native transport's ETag store
            // holds raw answers, private repos' among them, so a chat neither reads nor
            // writes it (ADR 0070 §3 and §4).
            deny(
                Class::HumanPowers,
                home.join(crate::forge::etag::DIR),
                Access::ReadWrite,
            );
        }
    }
    // This machine's profiles (ADR 0095, #2034): the folders purlis made, each holding a
    // profile's sign-in, settings and conversations. A chat neither reads nor writes them; their
    // records are in the config home above.
    if let Some(data) = Homes::charter_data(machine) {
        deny(
            Class::HumanPowers,
            data.join(crate::profilestore::DIR),
            Access::ReadWrite,
        );
        // The data folders of the browsers the session host runs (#2012): each holds its
        // engine's profile, the cookies and storage of every site the person opens in it.
        deny(
            Class::HumanPowers,
            data.join(crate::datahome::BROWSERS),
            Access::ReadWrite,
        );
        // The checked copies of Docker's settings purlis makes for each run of `docker`
        // (#2752): they hold the person's registry sign-ins, so a chat neither reads nor
        // writes them.
        deny(
            Class::HumanPowers,
            data.join(crate::dockerpool::settings::DIR),
            Access::ReadWrite,
        );
        // The browser logins this device keeps sealed (#2021, the person's ruling of
        // 2026-10-18): a chat neither reads nor writes any of them.
        deny(
            Class::HumanPowers,
            data.join(crate::browserlogins::DIR),
            Access::ReadWrite,
        );
        // The browser engines purlis downloads (#2835): the AppArmor profile the `.deb`
        // installs lets a program at the pinned engine's path make user namespaces, so a chat
        // writes nothing there, in the data home in use or under the default data folder's
        // either name, which the profile names too.
        let mut engines = vec![crate::browsers::engines_in(&data)];
        if let Some(home) = &machine.home {
            let share = home.join(".local/share");
            for name in ["purlis", "charter"] {
                let folder = crate::browsers::engines_in(&share.join(name));
                if !engines.contains(&folder) {
                    engines.push(folder);
                }
            }
        }
        for folder in engines {
            deny(Class::HumanPowers, folder, Access::Write);
        }
    }
    // The session host's socket and its lock, in the folder it makes this user's alone under
    // the runtime or temporary directory (#1997, `hostsocket`): a chat neither reads nor
    // writes anything there.
    #[cfg(unix)]
    {
        let runtime = machine
            .env
            .get("XDG_RUNTIME_DIR")
            .map(std::ffi::OsString::from);
        if let Some(folder) = crate::hostsocket::socket_for(runtime.as_deref()).parent() {
            deny(Class::HumanPowers, folder.to_path_buf(), Access::ReadWrite);
        }
        // The folders chats' servers listen in (#2019): no chat writes there, so none takes
        // another's socket, nor the folder a door carries to. A chat's own socket is given back
        // after every class, by the rules that let it serve.
        deny(
            Class::Integrity,
            door::root_in(runtime.as_deref()),
            Access::Write,
        );
    }
    // A vault's file is kept under the other names purlis finds for it in the project.
    let mut vaults: Vec<PathBuf> = Vec::new();
    for denial in &paths {
        if denial.class == Class::Vaults && denial.access == Access::ReadWrite {
            for place in [real(&denial.path), denial.path.clone()] {
                if !vaults.contains(&place) {
                    vaults.push(place);
                }
            }
        }
    }
    match second_names(root, &vaults) {
        Ok(found) => {
            paths.extend(found.names.into_iter().map(|path| Denial {
                class: Class::Vaults,
                path,
                access: Access::ReadWrite,
                named: None,
                reach: Reach::AndBelow,
                any_case: false,
            }));
            notes.extend(found.not_looked_in.into_iter().map(KeptNote::NotLookedIn));
        }
        Err(why) => {
            unresolved.get_or_insert(why);
        }
    }
    // A list that is not whole is never held by the store alone.
    keyring |= unresolved.is_some();
    KeptList {
        paths,
        keyring,
        untold: unresolved,
        notes,
    }
}

/// Whether `half` of the registry has an entry named `name`.
fn scope_has(half: &serde_json::Map<String, serde_json::Value>, name: &str) -> bool {
    half.get("vaults")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|vaults| vaults.contains_key(name))
}

/// **Podman's own folders on `machine`** (#2680): its settings (`containers/` in
/// `$XDG_CONFIG_HOME` and `~/.config`), its storage (`containers/` in `$XDG_DATA_HOME` and
/// `~/.local/share`) and its run state (`containers/` and `libpod/` in `$XDG_RUNTIME_DIR`, or,
/// where that names no folder, `/tmp/podman-run-<uid>`, `/tmp/storage-run-<uid>` and
/// `/tmp/containers-user-<uid>`), each with everything below it. A variable that names no
/// absolute folder names nothing.
fn podman_folders(machine: &Machine) -> Vec<(PathBuf, Reach)> {
    let named = |name: &str| {
        machine
            .env
            .get(name)
            .map(PathBuf::from)
            .filter(|folder| folder.is_absolute())
    };
    let mut folders: Vec<PathBuf> = Vec::new();
    let homes = machine
        .home
        .iter()
        .flat_map(|home| [home.join(".config"), home.join(".local/share")]);
    for base in named("XDG_CONFIG_HOME")
        .into_iter()
        .chain(named("XDG_DATA_HOME"))
        .chain(homes)
    {
        folders.push(base.join("containers"));
    }
    match named("XDG_RUNTIME_DIR") {
        Some(runtime) => {
            folders.push(runtime.join("containers"));
            folders.push(runtime.join("libpod"));
        }
        // Where none is named, as in an ssh session or one without systemd, Podman keeps its
        // run state in `/tmp` by the person's user id (#2896).
        None => {
            let uid = rustix::process::getuid().as_raw();
            for kind in ["podman-run", "storage-run", "containers-user"] {
                folders.push(PathBuf::from(format!("/tmp/{kind}-{uid}")));
            }
        }
    }
    let mut out: Vec<(PathBuf, Reach)> = Vec::new();
    for folder in folders {
        if !out.iter().any(|(one, _)| *one == folder) {
            out.push((folder, Reach::AndBelow));
        }
    }
    out
}

/// What [`kept_by_purlis`] answers.
struct KeptList {
    paths: Vec<Denial>,
    /// Whether a vault here keeps its secrets in the credential store.
    keyring: bool,
    untold: Option<Untold>,
    notes: Vec<KeptNote>,
}

/// **Why the list of what purlis keeps from a chat is not whole.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Untold {
    /// Why, as a clause of a sentence, naming the file concerned.
    pub why: String,
    /// The half of the vault registry to put right, where that is the repair: the one file
    /// the window still opens while nothing else of the project is read.
    pub registry: Option<PathBuf>,
}

impl fmt::Display for Untold {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.why)
    }
}

/// **What Doctor says of a list that is whole**: an entry of the registry that keeps nothing
/// from starting and that a person would still want to know of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeptNote {
    /// An entry that names no kind of vault: what is left on this machine of a vault the
    /// project no longer registers.
    LeftOver(String),
    /// A vault of a kind this purlis does not know: one a newer purlis registered.
    UnknownKind { name: String, kind: String },
    /// A `plain-file` vault with no file on this machine yet.
    NotSetUp(String),
    /// A folder purlis could not look in for another name of a vault's file.
    NotLookedIn(PathBuf),
}

impl fmt::Display for KeptNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LeftOver(name) => write!(
                f,
                "'{name}' is left over on this machine: it names no kind of vault"
            ),
            Self::UnknownKind { name, kind } => write!(
                f,
                "'{name}' is a vault of a kind this purlis does not know ({kind}): update purlis"
            ),
            Self::NotSetUp(name) => write!(
                f,
                "'{name}' is not set up on this machine: it does not say where its file is"
            ),
            Self::NotLookedIn(folder) => write!(
                f,
                "purlis could not look in {} for another name of a vault's file",
                folder.display()
            ),
        }
    }
}

/// **What purlis says of its list for the project at `root`**, for Doctor: why it is not
/// whole, if it is not, and what it noted while telling it.
pub fn kept_said(root: &Path, machine: &Machine) -> (Option<Untold>, Vec<KeptNote>) {
    let list = kept_by_purlis(root, machine);
    (list.untold, list.notes)
}

/// **Whether chats in the project at `root` run sandboxed**, or may: its own file turned the
/// sandbox on, an administrator's policy requires it, or the file that would say is gone or
/// does not read, which never reads as "off".
pub fn in_force_at(root: &Path) -> bool {
    let plane = Plane::read(root);
    plane.unreadable() || plane.missing() || plane.in_force(&policy::Locks::of(root)).is_some()
}

/// What one entry of the vault registry means for the list.
enum Judged {
    /// A vault that keeps its secrets in these places.
    Kept(Vec<PathBuf>),
    /// A vault that keeps its secrets in the credential store.
    CredentialStore,
    /// A vault of a kind purlis knows that keeps nothing on this machine's disk.
    Elsewhere,
    /// A `plain-file` vault with no file said yet.
    NotSetUp,
    /// An entry that names no kind, with every file it names.
    LeftOver(Vec<PathBuf>),
    /// An entry of a kind this purlis does not know, with every file it names.
    UnknownKind(String, Vec<PathBuf>),
    /// A vault of a known kind whose file is said in a way purlis cannot use.
    Unusable,
}

/// `entry`, one entry of the vault registry in the project at `root`, judged.
///
/// A `plain-file` vault keeps the file it names. One that names none yet has no file on this
/// machine. One whose `config` is not an object, or whose `file` is not a path, cannot be
/// placed, and the list is not whole. **An entry keeps the files it names, whatever kind it
/// says**: one of no kind, or of a kind this purlis does not know, keeps every `file` or `path`
/// it gives, of its own or in its `config`, placed as a `plain-file` vault's file is.
fn judged(ctx: &crate::secrets::Ctx, entry: &serde_json::Map<String, serde_json::Value>) -> Judged {
    use serde_json::Value;
    // Where a file said as `said` is: as it is written, and with every `..` step taken.
    let placed = |said: &str| -> Vec<PathBuf> {
        let written = ctx.vault_file_path(said);
        let stepped = planted::lexical(&written);
        if stepped == written {
            vec![written]
        } else {
            vec![stepped, written]
        }
    };
    let named = || -> Vec<PathBuf> {
        let config = entry.get("config").and_then(Value::as_object);
        let mut places = Vec::new();
        for part in [Some(entry), config].into_iter().flatten() {
            for key in ["file", "path"] {
                let Some(said) = part.get(key).and_then(Value::as_str) else {
                    continue;
                };
                if said.is_empty() {
                    continue;
                }
                for place in placed(said) {
                    if !places.contains(&place) {
                        places.push(place);
                    }
                }
            }
        }
        places
    };
    match entry.get("provider") {
        None | Some(Value::Null) => Judged::LeftOver(named()),
        Some(Value::String(kind)) if kind == "plain-file" => match entry.get("config") {
            None | Some(Value::Null) => Judged::NotSetUp,
            Some(Value::Object(config)) => match config.get("file") {
                None | Some(Value::Null) => Judged::NotSetUp,
                Some(Value::String(file)) if !file.is_empty() => Judged::Kept(placed(file)),
                Some(_) => Judged::Unusable,
            },
            Some(_) => Judged::Unusable,
        },
        Some(Value::String(kind)) if kind == "keyring" => Judged::CredentialStore,
        Some(Value::String(kind))
            if crate::secrets::registry::PROVIDERS.contains(&kind.as_str()) =>
        {
            Judged::Elsewhere
        }
        Some(kind) => Judged::UnknownKind(
            crate::personas::one_line(&match kind {
                Value::String(kind) => kind.clone(),
                other => other.to_string(),
            }),
            named(),
        ),
    }
}

/// The most entries of the vaults' own places [`second_names`] looks at.
const MOST_KEPT_LOOKED_AT: usize = 20_000;

/// The most entries of a project [`second_names`] looks through for a file's other names.
const MOST_LOOKED_THROUGH: usize = 200_000;

/// Folders [`second_names`] does not look in, at any depth: git's own, and what builds and
/// package managers fill, which no review of a project lists.
const NOT_LOOKED_IN: [&str; 3] = [".git", "target", "node_modules"];

/// How long an answer of [`second_names`] is kept for the same files with the same number of
/// names, so one start and the window's reads beside it look through the project once.
const NAMES_KEPT_FOR: std::time::Duration = std::time::Duration::from_secs(60);

/// What [`second_names`] found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct OtherNames {
    /// The other names found, sorted.
    names: Vec<PathBuf>,
    /// Folders that could not be listed, and so were not looked in: said in Doctor.
    not_looked_in: Vec<PathBuf>,
}

/// A file in a kept place that has more than one name: where it is, what it is, and how many
/// names it has.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Several {
    path: PathBuf,
    id: (u64, u64),
    names: u64,
}

/// One kept answer of [`second_names`]: for which project and which files, when, and what.
type NamesKept = (
    PathBuf,
    Vec<Several>,
    std::time::Instant,
    Result<OtherNames, Untold>,
);

/// **The other names purlis finds, in the project at `root`, of every file at or under `kept`**
/// (each kept place as written or as the kernel names it): a file with more than one name is
/// the same file under each, so each name found is kept as the file is. The project is looked
/// through only where a kept file has more than one name, which a vault's file as purlis
/// writes it never has; and once for the same files with the same number of names
/// ([`NAMES_KEPT_FOR`]), so a start and the window's reads do not each look again.
///
/// Refused, naming the file, where the project holds more entries than are looked through. A
/// folder that cannot be listed is not looked in, and is answered so Doctor says so. No link
/// is followed, and [`NOT_LOOKED_IN`] is not looked in.
fn second_names(root: &Path, kept: &[PathBuf]) -> Result<OtherNames, Untold> {
    static KEPT: std::sync::Mutex<Vec<NamesKept>> = std::sync::Mutex::new(Vec::new());
    let several = several_names(kept, MOST_KEPT_LOOKED_AT)?;
    if several.is_empty() {
        return Ok(OtherNames::default());
    }
    let now = std::time::Instant::now();
    let mut held = KEPT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    held.retain(|(_, _, when, _)| now.duration_since(*when) < NAMES_KEPT_FOR);
    if let Some((_, _, _, answer)) = held
        .iter()
        .find(|(of, files, _, _)| of == root && *files == several)
    {
        return answer.clone();
    }
    let answer = names_in(root, kept, &several, MOST_LOOKED_THROUGH);
    held.retain(|(of, _, _, _)| of != root);
    held.push((root.to_path_buf(), several, now, answer.clone()));
    answer
}

/// The files at or under `kept` that have more than one name, looking at no more than `most`
/// entries. A kept folder that cannot be listed is kept whole by its own name, and whoever
/// cannot list it is not one to say what is in it.
#[cfg(unix)]
fn several_names(kept: &[PathBuf], most: usize) -> Result<Vec<Several>, Untold> {
    use std::os::unix::fs::MetadataExt as _;
    let mut several: Vec<Several> = Vec::new();
    let mut left = most;
    let mut places: Vec<PathBuf> = kept.to_vec();
    while let Some(place) = places.pop() {
        let Ok(meta) = std::fs::symlink_metadata(&place) else {
            continue;
        };
        if meta.is_dir() {
            let Ok(listed) = std::fs::read_dir(&place) else {
                continue;
            };
            for entry in listed.flatten() {
                if left == 0 {
                    return Err(Untold {
                        why: format!(
                            "{} holds more files than purlis looks at for a vault's file with \
                             more than one name",
                            place.display()
                        ),
                        registry: None,
                    });
                }
                left -= 1;
                places.push(entry.path());
            }
        } else if meta.is_file() && meta.nlink() > 1 {
            let id = (meta.dev(), meta.ino());
            if !several.iter().any(|one| one.id == id) {
                several.push(Several {
                    path: place,
                    id,
                    names: meta.nlink(),
                });
            }
        }
    }
    several.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(several)
}

/// The names in the project at `root`, outside `kept`, of the files `several`, looking through
/// no more than `most` entries.
#[cfg(unix)]
fn names_in(
    root: &Path,
    kept: &[PathBuf],
    several: &[Several],
    most: usize,
) -> Result<OtherNames, Untold> {
    use std::os::unix::fs::{DirEntryExt as _, MetadataExt as _};
    let mut found = OtherNames::default();
    let mut left = most;
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let Ok(listed) = std::fs::read_dir(&folder) else {
            found.not_looked_in.push(folder);
            continue;
        };
        for entry in listed.flatten() {
            if left == 0 {
                return Err(Untold {
                    why: format!(
                        "the vault file {} has more than one name, and this project holds more \
                         files than purlis looks through for the others",
                        several[0].path.display()
                    ),
                    registry: None,
                });
            }
            left -= 1;
            let path = entry.path();
            if kept.iter().any(|it| path.starts_with(it)) {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if !NOT_LOOKED_IN
                    .iter()
                    .any(|name| entry.file_name() == std::ffi::OsStr::new(name))
                {
                    folders.push(path);
                }
            } else if kind.is_file()
                && several.iter().any(|one| one.id.1 == entry.ino())
                && entry
                    .metadata()
                    .is_ok_and(|meta| several.iter().any(|one| one.id == (meta.dev(), meta.ino())))
            {
                found.names.push(path);
            }
        }
    }
    found.names.sort();
    found.not_looked_in.sort();
    Ok(found)
}

/// Where a file has one name, nothing more is kept.
#[cfg(not(unix))]
fn several_names(_kept: &[PathBuf], _most: usize) -> Result<Vec<Several>, Untold> {
    Ok(Vec::new())
}

/// Where a file has one name, nothing more is kept.
#[cfg(not(unix))]
fn names_in(
    _root: &Path,
    _kept: &[PathBuf],
    _several: &[Several],
    _most: usize,
) -> Result<OtherNames, Untold> {
    Ok(OtherNames::default())
}

/// **Every path no chat in the plane at `root` may read**, each as written and as the kernel
/// names it: the vaults and their keys, purlis's own records a chat is kept from (its hook
/// spool, every dispatch's record, what waits to be told to a chat), and the person's
/// credentials on this machine. The read half of [`Denied::of`], with what
/// [`Denied::and_what_waits_for_a_chat`] adds for every harness.
///
/// What the window's own file reads leave out too (`crate::files`, the plane root's files):
/// the window's exclusion list and the sandbox's read denials are one list.
///
/// **Refused, with why, when the list cannot be told whole** ([`Denied::untold`]): the vault
/// registry does not read, or a vault in it says where its file is in a way purlis cannot
/// use. A reader that would list files by a list that may be short lists nothing instead, as a
/// chat does not start under one.
pub fn kept_from_reading(root: &Path, machine: &Machine) -> Result<Vec<PathBuf>, Untold> {
    let KeptList { paths, untold, .. } = kept_by_purlis(root, machine);
    if let Some(why) = untold {
        return Err(why);
    }
    let denied = Denied {
        paths,
        ..Denied::default()
    }
    .and_what_waits_for_a_chat(root);
    let mut out: Vec<PathBuf> = Vec::new();
    for denial in denied.paths {
        if denial.access != Access::ReadWrite {
            continue;
        }
        for path in [real(&denial.path), denial.path] {
            if !out.contains(&path) {
                out.push(path);
            }
        }
    }
    Ok(out)
}

/// `path` as the kernel names it: its longest part that exists, with its links resolved, and
/// the rest as written. A sandbox matches the path the kernel resolved, so a rule written on a
/// link's name (`/tmp`, `/var`, a linked `~/.config`) would match nothing.
pub(crate) fn real(path: &Path) -> PathBuf {
    let mut existing = path.to_path_buf();
    let mut rest = Vec::new();
    loop {
        if let Ok(found) = existing.canonicalize() {
            let mut out = found;
            out.extend(rest.into_iter().rev());
            return out;
        }
        match (existing.file_name().map(ToOwned::to_owned), existing.pop()) {
            (Some(name), true) => rest.push(name),
            _ => return path.to_path_buf(),
        }
    }
}

/// The directories strictly between `root` and `path`, nearest `root` first: each one a chat
/// working in `root` could otherwise move away with `path` inside it, and replace with one of
/// its own, which no rule on `path` itself holds.
pub fn ancestors_within(path: &Path, root: &Path) -> Vec<PathBuf> {
    let Ok(below) = path.strip_prefix(root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut at = root.to_path_buf();
    let parts: Vec<_> = below.components().collect();
    for part in parts.iter().take(parts.len().saturating_sub(1)) {
        at.push(part);
        out.push(at.clone());
    }
    out
}

/// The hosts `presets` let a chat reach in `plane`: each preset's own, and for
/// [`Preset::Forge`] the hosts of the plane's `[[forge]]` blocks — the forges its logins are
/// checked against (ADR 0055).
///
/// A first cut, and SD-4 owns what each preset holds; what a preset does not list is refused
/// rather than let through.
///
/// **A forge's host is the project's choice, not the preset's** (D-1343-10): it is held to
/// `locks` as a host of the project's own is, so a policy's `hosts` reaches it. A preset's fixed
/// list is the `presets` lock's, which the caller asks first.
pub fn hosts(presets: &[Preset], plane: &Plane, locks: &policy::Locks) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |host: &str| {
        if !out.iter().any(|it| it == host) {
            out.push(host.to_owned());
        }
    };
    for preset in presets {
        let listed = preset.own_hosts();
        for host in listed {
            add(host);
        }
        if *preset == Preset::Forge {
            for host in plane.forge_hosts() {
                let held = hosts::Host::parse(&host).map_or(true, |host| {
                    locks
                        .refuses(&hosts::Granted {
                            host,
                            level: hosts::Level::Project,
                        })
                        .is_some()
                });
                if !held {
                    add(&host);
                }
            }
        }
    }
    out
}

/// What every chat of a project reaches and writes on this machine besides its presets, its own
/// folder and its temp folder (#1340): counted as [`Compiled::granted`] grants them, for the
/// sentence Settings › Sandbox opens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Besides {
    /// The project's own hosts it is granted: each it takes, none a policy locks out, none an
    /// address of this machine.
    pub project_hosts: usize,
    /// Your own hosts on this machine it is granted besides those: confirmed here, and only
    /// while the local file is one git ignores ([`hosts::personal`]).
    pub your_hosts: usize,
    /// The folders you let every chat of the project write on this machine that still judge
    /// as grantable ([`local::granted_writes`], [`grant::still_grantable`]).
    pub folders: usize,
}

/// [`Besides`] for the project at `root`, read as `plane`, on `machine`: nothing where no
/// sandbox is in force ([`Plane::in_force`]).
pub fn besides(root: &Path, plane: &Plane, machine: &Machine) -> Besides {
    let locks = policy::Locks::of(root);
    let Some(policy) = plane.in_force(&locks) else {
        return Besides::default();
    };
    Besides {
        folders: granted_folders(root, machine, &grant::Grants::default(), &locks).len(),
        ..counted_hosts(&granted_hosts(&policy, root, &[], &[], &locks))
    }
}

/// The hosts every level grants a chat in the project at `root`: the project's, this
/// machine's with `chat`'s own, then `persona`'s, each held to `locks` and none an address of
/// this machine. The one place a chat's start and Settings' count read them ([`besides`]).
fn granted_hosts(
    policy: &Policy,
    root: &Path,
    persona: &[hosts::Host],
    chat: &[hosts::Host],
    locks: &policy::Locks,
) -> Vec<hosts::Granted> {
    // What an Allow kept, for the person or for the project, only at a scope policy leaves
    // open (#1927); a line someone wrote stays its file's.
    let made = local::made(root);
    let project = hosts_in_scope(&policy.hosts, grant::Level::Project, locks, &made);
    let mut personal = hosts_in_scope(&hosts::personal(root), grant::Level::You, locks, &made);
    for host in chat {
        if !personal.contains(host) {
            personal.push(host.clone());
        }
    }
    hosts::off_this_machine(
        hosts::in_force(&project, &personal, persona, locks),
        &hosts::own_addresses(),
    )
}

/// **The ports of this Mac's loopback a chat in the project at `root` may listen on** (#2019):
/// `chat`'s, then yours for this project on this machine, each at a scope `locks` leave open
/// and with the layer that granted it. [`Widened::and_listens`] judges each again.
fn granted_listens(
    root: &Path,
    chat: &grant::Grants,
    locks: &policy::Locks,
) -> Vec<(u16, reach::By)> {
    let mut out: Vec<(u16, reach::By)> = chat
        .listens
        .iter()
        .filter(|port| locks.keeps_scope(chat.level_of(&grant::What::Listen(**port))))
        .map(|port| (*port, reach::By::Chat))
        .collect();
    if locks.keeps_scope(grant::Level::You) {
        out.extend(
            local::granted_listens(root)
                .into_iter()
                .map(|port| (port, reach::By::You)),
        );
    }
    out
}

/// The folders a chat in the project at `root` may write besides its own: `chat`'s and the
/// ones you let every chat write here, each judged again ([`grant::still_grantable`]); none
/// where `locks` forbid write grants (#1343). The one place a chat's start and Settings' count
/// read them ([`besides`]).
fn granted_folders(
    root: &Path,
    machine: &Machine,
    chat: &grant::Grants,
    locks: &policy::Locks,
) -> Vec<PathBuf> {
    if locks.forbids_write_grants() {
        return Vec::new();
    }
    // Each at a scope policy leaves open (#1927): this chat's at the one it holds it at, and
    // yours for this project on this machine.
    let mut writes: Vec<PathBuf> = chat
        .writes
        .iter()
        .filter(|folder| locks.keeps_scope(chat.level_of(&grant::What::Write((*folder).clone()))))
        .cloned()
        .collect();
    if locks.keeps_scope(grant::Level::You) {
        writes.extend(local::granted_writes(root));
    }
    grant::still_grantable(&writes, &grant::Ground::of(root, root, machine).place())
}

/// One grant this machine keeps that an administrator's policy holds: it reaches no chat
/// while the policy stands, and is neither forgotten nor taken off the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldGrant {
    pub what: grant::What,
    /// The level it was kept at: this project on this machine, or every project on it.
    pub level: grant::Level,
    /// The policy's sentence, with who set it ([`policy::Locks::holds_kept`]).
    pub why: String,
}

/// **What this machine keeps for the project at `root` that policy holds now** (#1927): each
/// folder, socket and service you allowed for this project, then each socket and service you
/// allowed for every project, that [`policy::Locks::holds_kept`] says reaches no chat, then
/// each host an Allow kept for you or for the project at a scope left out ([`host_held`]),
/// with why, and each socket or service of the system's name lookup kept here
/// ([`services::name_lookup_refused`]). What Doctor counts; Settings' Allowed list marks each row by the same judgement. The
/// certificate check's service is never listed: one an older build kept is no grant (R1).
pub fn held_by_policy(root: &Path) -> Vec<HeldGrant> {
    use grant::{Level, What};
    let locks = policy::Locks::of(root);
    let service = |name: String| (name != services::TRUSTD).then_some(What::Service(name));
    let yours = local::granted_writes(root)
        .into_iter()
        .map(What::Write)
        .chain(local::granted_sockets(root).into_iter().map(What::Socket))
        .chain(
            local::granted_services(root)
                .into_iter()
                .filter_map(service),
        )
        .chain(local::granted_ports(root).into_iter().map(What::Port))
        .chain(local::granted_listens(root).into_iter().map(What::Listen))
        .map(|what| (what, Level::You));
    let everywhere = local::sockets_everywhere()
        .into_iter()
        .map(What::Socket)
        .chain(local::services_everywhere().into_iter().filter_map(service))
        .map(|what| (what, Level::Machine));
    let mut out: Vec<HeldGrant> = yours
        .chain(everywhere)
        .filter_map(|(what, level)| {
            // The system's name lookup reaches no chat whatever was kept for it (#2657).
            services::name_lookup_refused(&what)
                .map(str::to_owned)
                .or_else(|| locks.holds_kept(&what, level))
                .map(|why| HeldGrant { what, level, why })
        })
        .collect();
    // And each host an Allow kept, for you or for the project, at a scope policy leaves out.
    let project = Plane::read(root)
        .in_force(&locks)
        .map(|policy| policy.hosts)
        .unwrap_or_default();
    for (listed, level) in [
        (hosts::personal(root), Level::You),
        (project, Level::Project),
    ] {
        for host in listed {
            if let Some(why) = host_held(root, &host, level, &locks) {
                out.push(HeldGrant {
                    what: What::Host(host),
                    level,
                    why,
                });
            }
        }
    }
    out
}

/// Whether an Allow on this machine kept `host` at `level`, by this machine's record of what
/// an Allow kept (`made`, [`local::made`]): what tells an Allow's host from a line someone
/// wrote in the file.
fn allow_kept(host: &hosts::Host, level: grant::Level, made: &[local::Made]) -> bool {
    let target = host.to_string();
    made.iter()
        .any(|one| one.what == "host" && one.level == level.word() && one.target == target)
}

/// **The hosts of `listed`, kept at `level`, that reach a start under `locks`' scopes**
/// (#1927): every one where the policy leaves that scope open, and otherwise only the ones
/// no Allow kept. A host an Allow kept for the person, or for the project, is judged against
/// the scopes at every start like anything else an Allow kept; a line someone wrote in a file
/// they edit by hand is the file's, and its own locks (`personal-hosts`, `hosts`,
/// `never-hosts`) hold it. `made` is this machine's record of what an Allow kept: a file no
/// chat writes and no repository brings, so a host is told as an Allow's only on the machine
/// whose person allowed it.
pub(crate) fn hosts_in_scope(
    listed: &[hosts::Host],
    level: grant::Level,
    locks: &policy::Locks,
    made: &[local::Made],
) -> Vec<hosts::Host> {
    listed
        .iter()
        .filter(|host| locks.keeps_scope(level) || !allow_kept(host, level, made))
        .cloned()
        .collect()
}

/// **Why `host`, listed at `level` for the project at `root`, reaches no chat under `locks`'
/// scopes**, where an Allow kept it at a scope they leave out ([`hosts_in_scope`]); none for
/// a line someone wrote, and where the scope is open. What Settings' Allowed list says of it.
pub fn host_held(
    root: &Path,
    host: &hosts::Host,
    level: grant::Level,
    locks: &policy::Locks,
) -> Option<String> {
    allow_kept(host, level, &local::made(root))
        .then(|| locks.scope_held(level))
        .flatten()
}

/// **The sockets and services this machine keeps for the project at `root` at a scope
/// `locks` leave open** (#1927): yours for this project, then yours for every project. One
/// kept at a scope a policy's `allow-scopes` leaves out is not among them, so it reaches no
/// start and is not read as allowed. The one place a start and the question about what a
/// project expects read them.
pub(crate) fn kept_in_scope(root: &Path, locks: &policy::Locks) -> (Vec<PathBuf>, Vec<String>) {
    let (mut sockets, mut named) = (Vec::new(), Vec::new());
    if locks.keeps_scope(grant::Level::You) {
        sockets.extend(local::granted_sockets(root));
        named.extend(local::granted_services(root));
    }
    if locks.keeps_scope(grant::Level::Machine) {
        sockets.extend(local::sockets_everywhere());
        named.extend(local::services_everywhere());
    }
    (sockets, named)
}

/// The project's hosts and yours among `granted`.
fn counted_hosts(granted: &[hosts::Granted]) -> Besides {
    let at = |level: hosts::Level| granted.iter().filter(|one| one.level == level).count();
    Besides {
        project_hosts: at(hosts::Level::Project),
        your_hosts: at(hosts::Level::You),
        folders: 0,
    }
}

/// **How much a chat's sandbox holds** (#2071, the operator's ruling of 2026-10-14).
///
/// - [`Level::Full`]: the sandbox a project turns on (`[sandbox] mode = "on"`): the hosts it
///   lists, the folders a chat writes, the Allow asks, and every denial class.
/// - [`Level::Floor`]: the default for a project that has not turned it on. No network limits,
///   no folder limits beyond what it holds, and no host is asked about, so a chat works much as
///   its harness does unsandboxed; but
///   the denial classes stay ([`Class`]): no vault and none of purlis's own keys is read, and
///   nothing a later program runs outside any protection is written. A brokered `secret exec`
///   still runs only the persona's declared tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    #[default]
    Full,
    Floor,
}

impl Level {
    /// What a chat's tab and its start call the level.
    pub fn title(self) -> &'static str {
        match self {
            Self::Full => "full sandbox",
            Self::Floor => "floor",
        }
    }
}

/// **Whether the floor also keeps a chat from writing every folder on the person's `PATH`**
/// (#2071, the operator's ruling of 2026-10-14): a chat cannot plant a program the person runs
/// later. Besides it, the floor always holds the harness's own program and each declared tool's
/// ([`Applied::hold_programs`]). A write it refuses is a sandbox block like any other, whose
/// Allow lets that chat write the folder ([`Compiled::floor`] leaves a granted folder out).
pub const FLOOR_HOLDS_PATH: bool = true;

/// **The folders on `machine`'s `PATH` the floor keeps a chat from writing** ([`FLOOR_HOLDS_PATH`]):
/// each folder a program is searched in ([`crate::programs::search_dirs_from`]), as the kernel
/// names it, and the package each link in it leads to ([`package_root`]): a Homebrew keg, an npm
/// global package, a pipx venv, a toolchain. None that holds the project at `root` or is inside
/// it (the chat writes its project), none that is the home folder or above it, and none inside a
/// folder a person let the chat write (`granted`).
pub fn floor_held(root: &Path, machine: &Machine, granted: &[PathBuf]) -> Vec<PathBuf> {
    if !FLOOR_HOLDS_PATH {
        return Vec::new();
    }
    let root = real(root);
    let home = machine.home.as_deref().map(real);
    machine_held(machine)
        .iter()
        .filter(|dir| {
            let around_root = root.starts_with(dir) || dir.starts_with(&root);
            let above_home = home.as_deref().is_some_and(|home| home.starts_with(dir));
            let granted_here = granted.iter().any(|folder| dir.starts_with(folder));
            !(around_root || above_home || granted_here)
        })
        .cloned()
        .collect()
}

/// How long [`machine_held`] keeps what it found: a burst of compiles (every open chat's, as the
/// window asks whether any runs on an older sandbox) reads the `PATH` folders once, and a
/// package installed since is held from the next start a few seconds on.
const MACHINE_HELD_KEPT: std::time::Duration = std::time::Duration::from_secs(5);

/// What [`floor_held`] holds on `machine` before a project or a grant is asked about: the package
/// prefixes, the person's Python site folders, the `PATH` folders and where their links lead.
/// Reading them follows a few thousand links, so it is kept for [`MACHINE_HELD_KEPT`] per `PATH`
/// and home folder.
fn machine_held(machine: &Machine) -> std::sync::Arc<Vec<PathBuf>> {
    use std::sync::{Arc, Mutex, OnceLock};
    type Kept = (
        Option<String>,
        Option<PathBuf>,
        std::time::Instant,
        Arc<Vec<PathBuf>>,
    );
    static KEPT: OnceLock<Mutex<Option<Kept>>> = OnceLock::new();
    let path = machine.env.get("PATH");
    // This crate's own tests change a `PATH` folder between two reads of it.
    if cfg!(test) {
        return Arc::new(machine_held_now(path.as_deref(), machine.home.as_deref()));
    }
    let kept = KEPT.get_or_init(|| Mutex::new(None));
    let fresh = |(was_path, was_home, at, _): &Kept| {
        *was_path == path && *was_home == machine.home && at.elapsed() < MACHINE_HELD_KEPT
    };
    if let Some(found) = kept.lock().ok().and_then(|kept| {
        kept.as_ref()
            .filter(|one| fresh(one))
            .map(|one| Arc::clone(&one.3))
    }) {
        return found;
    }
    let found = Arc::new(machine_held_now(path.as_deref(), machine.home.as_deref()));
    if let Ok(mut kept) = kept.lock() {
        *kept = Some((
            path,
            machine.home.clone(),
            std::time::Instant::now(),
            Arc::clone(&found),
        ));
    }
    found
}

/// [`machine_held`], read now.
fn machine_held_now(path: Option<&str>, home: Option<&Path>) -> Vec<PathBuf> {
    let mut held: Vec<PathBuf> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut hold = |dir: PathBuf| {
        if dir.parent().is_some() && seen.insert(dir.clone()) {
            held.push(dir);
        }
    };
    // A package manager's whole prefix, where it is there: what its programs load at every start
    // (a keg's libraries, `opt` links, Python's `site-packages`) as well as the programs.
    for prefix in PREFIXES_HELD {
        let prefix = Path::new(prefix);
        if prefix.is_dir() {
            hold(real(prefix));
        }
    }
    // The person's own Python site folders, whose `.pth` files run at every interpreter start.
    if let Some(home) = home {
        hold(real(&home.join("Library/Python")));
        // Whole, so a Python folder made there after the chat started is held too.
        hold(real(&home.join(".local/lib")));
    }
    let path = path.map(std::ffi::OsString::from);
    for dir in crate::programs::search_dirs_from(path.as_deref(), home) {
        hold(real(&dir));
        // What each link in it leads to, a few thousand at most: a `PATH` folder is mostly
        // links into the packages its manager installed.
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten().take(LINKS_LOOKED_AT_MOST) {
            let is_link = entry.file_type().is_ok_and(|kind| kind.is_symlink());
            if is_link {
                let target = real(&entry.path());
                if target != entry.path() {
                    hold(package_root(&target));
                    // npm's global packages, all of them: what one loads, another can be.
                    if let Some(global) = node_modules_of(&target) {
                        hold(global);
                    }
                }
            }
        }
    }
    held
}

/// The package managers' prefixes the floor holds whole where they are there ([`floor_held`]):
/// Homebrew's on Apple silicon, and its folders under `/usr/local` on Intel.
const PREFIXES_HELD: [&str; 5] = [
    "/opt/homebrew",
    "/usr/local/Cellar",
    "/usr/local/opt",
    "/usr/local/lib",
    "/usr/local/Homebrew",
];

/// The most entries of one `PATH` folder [`floor_held`] follows.
const LINKS_LOOKED_AT_MOST: usize = 4096;

/// The `node_modules` folder `file` is in, where it is in one: npm's global packages for a
/// program a `PATH` link leads to.
fn node_modules_of(file: &Path) -> Option<PathBuf> {
    file.ancestors()
        .find(|dir| dir.file_name().is_some_and(|name| name == "node_modules"))
        .map(Path::to_path_buf)
}

/// What purlis arms a chat with, by where each lies ([`Applied::hold_kit`]): its own `binary`,
/// which the chat's hooks run, its `plugin` and its `git_hooks` folder.
#[derive(Debug, Clone, Copy, Default)]
pub struct ArmedWith<'a> {
    pub binary: Option<&'a Path>,
    pub plugin: Option<&'a Path>,
    pub git_hooks: Option<&'a Path>,
    /// Where a chat's folders of links to the plane's skills are laid out (ADR 0063 as amended
    /// 2026-10-20): a plugin folder too, held as the git hooks are.
    pub skill_sets: Option<&'a Path>,
}

/// **The whole install `program` belongs to**, as the kernel names it: the app bundle it is
/// inside (the nearest folder named `*.app`, its plugin and every other file it ships among
/// it), else its [`package_root`].
pub fn install_root(program: &Path) -> PathBuf {
    let program = real(program);
    program
        .ancestors()
        .skip(1)
        .find(|folder| folder.extension().is_some_and(|ext| ext == "app"))
        .map_or_else(|| package_root(&program), Path::to_path_buf)
}

/// **The package a program file belongs to**, as far as its path says (#2071): the whole of a
/// Homebrew keg (`Cellar/<formula>`), an npm package (`node_modules/<name>`, or
/// `node_modules/@scope/<name>`), a pipx venv (`venvs/<name>`) or a toolchain
/// (`toolchains/<name>`); else the folder the file is in.
pub fn package_root(file: &Path) -> PathBuf {
    let parts: Vec<_> = file.components().collect();
    for (at, part) in parts.iter().enumerate().rev() {
        let word = part.as_os_str().to_string_lossy();
        let keep = match word.as_ref() {
            "Cellar" | "venvs" | "toolchains" => 2,
            "node_modules" => {
                let scoped = parts
                    .get(at + 1)
                    .is_some_and(|next| next.as_os_str().to_string_lossy().starts_with('@'));
                if scoped { 3 } else { 2 }
            }
            _ => continue,
        };
        if at + keep < parts.len() {
            return parts[..at + keep].iter().collect();
        }
    }
    file.parent()
        .map_or_else(|| file.to_path_buf(), Path::to_path_buf)
}

/// The policy for one chat, resolved to this machine and ready for a harness's compiler: the
/// neutral answer every compiler reads, and the only one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compiled {
    /// How much it holds ([`Level`]): every compiler reads it.
    pub level: Level,
    /// At the floor, the folders a chat may not write that no denial class names: the person's
    /// `PATH` and the harness's own program ([`floor_held`], [`Applied::hold_programs`]). A
    /// person's Allow lifts one. Empty for the full sandbox, which lists what a chat writes.
    pub held: Vec<PathBuf>,
    pub denied: Denied,
    pub hosts: Vec<String>,
    /// The same hosts, each with the layer that lists it (#1664): what purlis's egress proxy
    /// decides by ([`reach`]). [`Self::hosts`] is what a harness's own form names.
    pub reach: reach::Reach,
    /// The folders a person let this chat write besides its own and the temp folders (#1342):
    /// its own grants and yours ([`grant`]), each judged again as it is compiled. Every compiler
    /// puts its denials after them, so a class still wins inside one.
    pub writable: Vec<PathBuf>,
    pub os: Os,
    /// Where a harness keeps its own files on this machine, for a compiler that has to let the
    /// whole harness write them ([`opencode`]).
    pub homes: Homes,
    /// What the presets widen past the hosts.
    pub widened: Widened,
}

/// What a project's sandbox widens past the hosts it lets a chat reach (spec #1330's coupled
/// widenings, #1337).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Widened {
    /// The project's own package caches, with [`Preset::Toolchains`] only (D-1337-6): a cache
    /// home of its sandboxed chats', never the person's caches ([`caches`]).
    pub caches: Option<caches::CacheHome>,
    /// Whether a chat may ask the system's certificate check (D-1337-7): on macOS, Go programs
    /// such as `gh` verify a host's certificate by asking the `trustd` service, and without it
    /// every one of them fails TLS. **On by default** (R1, 2026-10-13, amending ADR 0093),
    /// with a known residual: the service fetches the addresses a certificate names, past the
    /// egress proxy. Off where the project's `certificate-checks` is `false` or policy locks
    /// it ([`CertificateChecks`]); no grant of a person's turns it back on.
    pub trust: bool,
    /// The local sockets a person let this chat connect to (D-sock-1): its own grants and
    /// yours on this machine, never the committed file's ([`services`]). Each as the kernel
    /// names it.
    pub sockets: Vec<PathBuf>,
    /// The system services a person let this chat look up by name besides the certificate
    /// check, which is [`Self::trust`] and never a grant (D-sock-1, R1).
    pub services: Vec<String>,
    /// The services on this Mac a person let this chat connect to, each by its one loopback
    /// port (qw384): its own grants and yours on this machine, never purlis's own
    /// ([`localport::still_ports`]).
    pub ports: Vec<u16>,
    /// The ports of this Mac's loopback a person let this chat listen on (#2019): its own
    /// grants and yours on this machine, never purlis's own, and none something on this Mac
    /// listened on as the chat was compiled. Each is in [`Self::ports`] too, so the chat's
    /// browser reaches the door as a named local port.
    pub listens: Vec<u16>,
    /// Those of [`Self::listens`] that are in [`Self::ports`] for a listen grant alone, and no
    /// port grant: reached only where a door opens on them, and withdrawn where none does.
    pub listen_only: Vec<u16>,
    /// The ports a person let this chat listen on that something on this Mac already listened
    /// on as it was compiled: neither served nor reached, and told so.
    pub unserved: Vec<u16>,
}

impl Widened {
    /// What `policy` widens for a chat in the project at `root` on `machine`, denied `denied`.
    pub fn of(policy: &Policy, machine: &Machine, root: &Path, denied: &Denied) -> Self {
        Self {
            caches: policy
                .egress
                .iter()
                .any(|preset| preset.widens_caches())
                .then(|| caches::home_of(machine, root, denied))
                .flatten(),
            // On unless the project's file says `false` (R1): nothing a person grants, and
            // nothing a committed file can widen, since `true` is what an absent key means.
            // Policy has its say in [`Self::and_granted`].
            trust: policy.certificate_checks,
            sockets: Vec::new(),
            services: Vec::new(),
            ports: Vec::new(),
            listens: Vec::new(),
            listen_only: Vec::new(),
            unserved: Vec::new(),
        }
    }

    /// These widenings with the ports of this Mac's loopback at `listens` to listen on besides
    /// (#2019), each one a port to connect to as well, for the door on it: none where `locks`
    /// forbid ports, never a port purlis itself listens on, and none something on this Mac
    /// listens on now ([`door::in_use`]), which is neither served nor reached
    /// ([`Self::unserved`]).
    #[must_use]
    pub fn and_listens(mut self, listens: &[u16], locks: &policy::Locks) -> Self {
        if locks.port_grants_refused().is_some() {
            return self;
        }
        for port in localport::still_ports(listens) {
            if self.listens.contains(&port) || self.unserved.contains(&port) {
                continue;
            }
            if door::in_use(port) {
                self.unserved.push(port);
                continue;
            }
            self.listens.push(port);
            if !self.ports.contains(&port) {
                self.ports.push(port);
                self.listen_only.push(port);
            }
        }
        self
    }

    /// These widenings without the ports given for a listen grant alone (#2019): what a chat no
    /// door ever opens for is given, so a listen grant never leaves it a plain connect.
    #[must_use]
    pub fn unserving(mut self) -> Self {
        self.ports.retain(|port| !self.listen_only.contains(port));
        self.listens.clear();
        self.listen_only.clear();
        self
    }

    /// These widenings with the services on this Mac at `ports` besides (qw384): none where
    /// `locks` forbid them, and never a port purlis itself listens on.
    #[must_use]
    pub fn and_ports(mut self, ports: &[u16], locks: &policy::Locks) -> Self {
        if locks.port_grants_refused().is_some() {
            return self;
        }
        for port in localport::still_ports(ports) {
            if !self.ports.contains(&port) {
                self.ports.push(port);
            }
        }
        self
    }

    /// These widenings with `sockets` and `services` granted besides (D-sock-1): each judged
    /// again ([`services::still_sockets`], [`services::still_services`]), and none where
    /// `locks` forbid them. **The certificate check's service is never taken from a grant**
    /// (R1): the project's setting and policy alone decide [`Self::trust`], so a grant an
    /// older build kept for it changes nothing either way.
    pub fn and_granted(
        mut self,
        sockets: &[PathBuf],
        services: &[String],
        ground: &services::SocketGround,
        locks: &policy::Locks,
    ) -> Self {
        if locks.forbids_socket_grants() {
            // Policy takes the certificate check away too, the project's own included.
            self.trust = false;
            return self;
        }
        for socket in services::still_sockets(sockets, ground) {
            if !self.sockets.contains(&socket) {
                self.sockets.push(socket);
            }
        }
        for service in services::still_services(services) {
            if service != services::TRUSTD && !self.services.contains(&service) {
                self.services.push(service);
            }
        }
        self
    }
}

/// The directories a harness keeps its own files under, by the XDG base directory rules: each
/// variable where it is set and absolute, its default under the home directory otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Homes {
    pub home: Option<PathBuf>,
    /// `$XDG_DATA_HOME`, or `~/.local/share`.
    pub data: Option<PathBuf>,
    /// `$XDG_STATE_HOME`, or `~/.local/state`.
    pub state: Option<PathBuf>,
    /// `$XDG_CONFIG_HOME`, or `~/.config`.
    pub config: Option<PathBuf>,
    /// `$XDG_CACHE_HOME`, or `~/.cache`.
    pub cache: Option<PathBuf>,
    /// The operator's own Codex home: `$CODEX_HOME`, or `~/.codex`. A wrapped chat neither reads
    /// nor writes it (D-88q).
    pub codex: Option<PathBuf>,
    /// The Codex home this project's sandboxed chats had of their own before they ran on a
    /// profile for sandboxed chats only (D-88q, amended 2026-10-15): under charter's data home
    /// ([`crate::datahome`]), one per project, its conversations brought to that profile once.
    /// Set by [`Compiled::of`], which knows the project.
    pub codex_project: Option<PathBuf>,
    /// The folders of the profiles purlis made on this machine (`<data>/profiles`, #2034):
    /// denied to every chat, but a wrapped chat's own profile's folder.
    pub profiles: Option<PathBuf>,
}

impl Homes {
    /// `machine`'s.
    pub fn of(machine: &Machine) -> Self {
        let xdg = |var: &str, default: &str| {
            machine
                .env
                .get(var)
                .map(PathBuf::from)
                .filter(|dir| dir.is_absolute())
                .or_else(|| machine.home.as_ref().map(|home| home.join(default)))
        };
        Self {
            home: machine.home.clone(),
            data: xdg("XDG_DATA_HOME", ".local/share"),
            state: xdg("XDG_STATE_HOME", ".local/state"),
            config: xdg("XDG_CONFIG_HOME", ".config"),
            cache: xdg("XDG_CACHE_HOME", ".cache"),
            codex: xdg("CODEX_HOME", ".codex"),
            codex_project: None,
            profiles: Self::charter_data(machine).map(|data| data.join(crate::profilestore::DIR)),
        }
    }

    /// charter's data home on `machine` ([`crate::datahome`]'s ladder, read from `machine`): the
    /// variable, else `$XDG_DATA_HOME/charter`, else the system's data directory under its home.
    pub(crate) fn charter_data(machine: &Machine) -> Option<PathBuf> {
        let system = machine.home.as_ref().map(|home| match machine.os {
            Os::MacOs => home.join("Library/Application Support"),
            Os::Linux | Os::Windows | Os::Other => home.join(".local/share"),
        });
        // The same ladder as the store's ([`crate::datahome::root_in`]), so a relative variable
        // names the same folder for both, and the folder rename-local moved it to, once it has
        // (RN-5).
        crate::datahome::root_with(&|name: &str| machine.env.get(name), system)
    }

    /// The Codex home of the project at `root`'s sandboxed chats on `machine` (D-88q): a folder
    /// named for the project as the kernel names it, under charter's data home.
    pub fn codex_project(machine: &Machine, root: &Path) -> Option<PathBuf> {
        Some(
            Self::charter_data(machine)?
                .join("codex-homes")
                .join(Self::project_key(root)),
        )
    }

    /// The name of the project at `root`'s folders under purlis's data home: a digest of the
    /// project as the kernel names it.
    pub(crate) fn project_key(root: &Path) -> String {
        use sha2::Digest;
        let real = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let digest = sha2::Sha256::digest(real.as_os_str().as_encoded_bytes());
        digest
            .iter()
            .take(16)
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

impl Compiled {
    /// **The floor** ([`Level::Floor`]) for a chat in the project at `root` on `machine`: every
    /// denial class, and nothing else held. No hosts are listed, since nothing limits the
    /// network; the folders a chat writes are not listed, since nothing limits them. The local
    /// sockets and system services stay as a person granted them (the operator's ruling Q1 on
    /// #2071): a socket can reach what the floor denies. The certificate check is on.
    pub fn floor(root: &Path, machine: &Machine, chat: &grant::Grants) -> Self {
        let locks = policy::Locks::of(root);
        let mut denied = Denied::of(root, machine);
        // The keychain's files, by path: the floor allows the lookups the full sandbox's profile
        // denies by default, and the items' own access rule holds the rest (V90a, Q5).
        if let Some(keychains) = seatbelt::keychains(machine.home.as_deref())
            && !denied.paths.contains(&keychains)
        {
            denied.paths.push(keychains);
        }
        let in_scope = |what: grant::What| locks.keeps_scope(chat.level_of(&what));
        let mut sockets: Vec<PathBuf> = chat
            .sockets
            .iter()
            .filter(|socket| in_scope(grant::What::Socket((*socket).clone())))
            .cloned()
            .collect();
        let mut services: Vec<String> = chat
            .services
            .iter()
            .filter(|service| in_scope(grant::What::Service((*service).clone())))
            .cloned()
            .collect();
        let (kept_sockets, kept_services) = kept_in_scope(root, &locks);
        sockets.extend(kept_sockets);
        services.extend(kept_services);
        let writable = granted_folders(root, machine, chat, &locks);
        // Services on this Mac (qw384), as the full sandbox grants them.
        let mut ports: Vec<(u16, reach::By)> = chat
            .ports
            .iter()
            .filter(|port| in_scope(grant::What::Port(**port)))
            .map(|port| (*port, reach::By::Chat))
            .collect();
        if locks.keeps_scope(grant::Level::You) {
            ports.extend(
                local::granted_ports(root)
                    .into_iter()
                    .map(|port| (port, reach::By::You)),
            );
        }
        // The certificate check as the project's file says it (R1), and as policy leaves it
        // ([`Widened::and_granted`]).
        let widened = Widened {
            trust: Plane::read(root)
                .said()
                .unset
                .unwrap_or_else(Policy::defaults)
                .certificate_checks,
            ..Widened::default()
        }
        .and_granted(
            &sockets,
            &services,
            &services::SocketGround::of(root, machine, &chat.writes),
            &locks,
        )
        .and_ports(
            &ports.iter().map(|(port, _)| *port).collect::<Vec<_>>(),
            &locks,
        );
        let listens = granted_listens(root, chat, &locks);
        let widened = widened.and_listens(
            &listens.iter().map(|(port, _)| *port).collect::<Vec<_>>(),
            &locks,
        );
        ports.extend(listens);
        let mut layered: Vec<(String, reach::By)> = Vec::new();
        for (port, by) in &ports {
            if widened.ports.contains(port) {
                layered.extend(
                    localport::reach_entries(*port)
                        .into_iter()
                        .map(|entry| (entry, *by)),
                );
            }
        }
        Self {
            level: Level::Floor,
            held: floor_held(root, machine, &writable),
            widened,
            denied,
            hosts: Vec::new(),
            // Every host, through the chat's own proxy, which keeps this machine's own
            // addresses out of reach as at the full sandbox but a service a person allowed, and
            // asks about none (#2071).
            reach: reach::Reach::anything()
                .and(layered)
                .never(locks.never_hosts().to_vec()),
            writable,
            os: machine.os,
            homes: Homes {
                codex_project: Homes::codex_project(machine, root),
                ..Homes::of(machine)
            },
        }
    }

    /// Whether every class a service holds here is held for `harness`'s chat on this system.
    ///
    /// **The credential store is held by the store itself on macOS** (ruling V90a): charter
    /// writes every keyring item so that only charter's own app reads it without the person's
    /// confirmation (`secrets::keyring::OsStore`), whatever the chat's sandbox lets it
    /// reach. Charter's own wrap denies the service as well (V90b, [`seatbelt`]). Elsewhere the
    /// store keeps no rule of its own (the Secret Service answers any process of the session),
    /// so a chat is refused with the ways out (V90c) unless purlis's own wrap holds it: on Linux
    /// the wrap reaches no socket of the session bus (#1040). Claude Code's own sandbox has
    /// not been measured keeping a chat off it, so a Claude Code chat there is still refused.
    fn holds_every_service(&self, harness: Harness) -> Result<(), Uncompilable> {
        // purlis's own wrap on Linux (#1040) reaches no unix socket but the chat's hook socket
        // and what a person allowed it, so never the session bus the Secret Service answers on,
        // and none of the kernel's key store.
        let wrapped_on_linux = self.os.purlis_wraps(harness);
        let unheld = self.denied.services.iter().find(|service| match service {
            Service::CredentialStore => self.os != Os::MacOs && !wrapped_on_linux,
        });
        match unheld {
            Some(service) => Err(Uncompilable {
                harness,
                unheld: Unheld::Service(*service, self.os),
            }),
            None => Ok(()),
        }
    }

    /// `policy`, for a chat in `plane` at `root` running as `persona`, on `machine`: its
    /// presets' hosts, then the hosts in force at every level ([`hosts::in_force`]): the
    /// project's, this machine's, then the persona's (#1362), those only as the person here
    /// allowed them ([`persona::in_force_here`]). A chat on another persona, or on none, gets
    /// no persona's hosts.
    pub fn of(
        policy: &Policy,
        plane: &Plane,
        root: &Path,
        machine: &Machine,
        persona: Option<&str>,
    ) -> Self {
        Self::granted(
            policy,
            plane,
            root,
            machine,
            persona,
            &grant::Grants::default(),
        )
    }

    /// [`Self::of`], with what a person let this one chat do besides (#1342): `chat`'s hosts,
    /// after the project's and this machine's and held to the same locks; and the folders it and
    /// you may write, each judged again here ([`grant::still_grantable`]), so a grant a denial
    /// class has since come to cover grants nothing.
    pub fn granted(
        policy: &Policy,
        plane: &Plane,
        root: &Path,
        machine: &Machine,
        persona: Option<&str>,
        chat: &grant::Grants,
    ) -> Self {
        // An administrator's policy (#1343): the strictest value wins, so a preset it does not
        // allow is off here, whatever the project's file says.
        let locks = policy::Locks::of(root);
        let policy = &Policy {
            egress: locks.presets(&policy.egress),
            ..policy.clone()
        };
        let mut reached = hosts(&policy.egress, plane, &locks);
        let mut layered: Vec<(String, reach::By)> = reached
            .iter()
            .map(|host| (host.clone(), reach::By::Open))
            .collect();
        // A persona's hosts only as the person on this machine allowed them (D-1362-7).
        let personas = persona::in_force_here(root, &policy.personas, persona, &locks);
        let yours = hosts::personal(root);
        // A host this chat holds is an Allow's alone, so it is held to the scopes policy
        // leaves open (#1927); your own hosts and the project's have their own locks.
        let held_hosts: &[hosts::Host] = if locks.keeps_scope(grant::Level::Chat) {
            &chat.hosts
        } else {
            &[]
        };
        for one in granted_hosts(policy, root, &personas, held_hosts, &locks) {
            let spelled = one.host.to_string();
            let by = match one.level {
                hosts::Level::Project => reach::By::Open,
                hosts::Level::Persona => reach::By::Persona,
                hosts::Level::You if !yours.contains(&one.host) => reach::By::Chat,
                hosts::Level::You => reach::By::You,
            };
            layered.push((spelled.clone(), by));
            if !reached.contains(&spelled) {
                reached.push(spelled);
            }
        }
        let denied = Denied::of(root, machine);
        let writable = granted_folders(root, machine, chat, &locks);
        // Sockets and services: this chat's, then yours for this project on this machine
        // (D-sock-2), then yours for every project on it (R2), never a committed file's. Each
        // is judged again below for this project, whichever level kept it, **and against the
        // policy as it stands now** (#1927): one kept at a scope policy leaves out, before the
        // policy arrived or since, contributes nothing to this start.
        let in_scope = |what: grant::What| locks.keeps_scope(chat.level_of(&what));
        let mut sockets: Vec<PathBuf> = chat
            .sockets
            .iter()
            .filter(|socket| in_scope(grant::What::Socket((*socket).clone())))
            .cloned()
            .collect();
        let mut named: Vec<String> = chat
            .services
            .iter()
            .filter(|service| in_scope(grant::What::Service((*service).clone())))
            .cloned()
            .collect();
        let (kept_sockets, kept_services) = kept_in_scope(root, &locks);
        sockets.extend(kept_sockets);
        named.extend(kept_services);
        // Services on this Mac (qw384): this chat's, then yours for this project on this
        // machine, at the scopes policy leaves open.
        let mut ports: Vec<(u16, reach::By)> = chat
            .ports
            .iter()
            .filter(|port| in_scope(grant::What::Port(**port)))
            .map(|port| (*port, reach::By::Chat))
            .collect();
        if locks.keeps_scope(grant::Level::You) {
            ports.extend(
                local::granted_ports(root)
                    .into_iter()
                    .map(|port| (port, reach::By::You)),
            );
        }
        let widened = Widened::of(policy, machine, root, &denied)
            .and_granted(
                &sockets,
                &named,
                &services::SocketGround::of(root, machine, &chat.writes),
                &locks,
            )
            .and_ports(
                &ports.iter().map(|(port, _)| *port).collect::<Vec<_>>(),
                &locks,
            );
        // Ports to listen on (#2019), at the scopes policy leaves open: each one a named local
        // port of the chat's reach too, so its browser opens the server it started.
        let listens = granted_listens(root, chat, &locks);
        let widened = widened.and_listens(
            &listens.iter().map(|(port, _)| *port).collect::<Vec<_>>(),
            &locks,
        );
        ports.extend(listens);
        // The chat's proxy carries a client that does use it to exactly those ports of this
        // machine, at the layer that granted each.
        for (port, by) in &ports {
            if widened.ports.contains(port) {
                layered.extend(
                    localport::reach_entries(*port)
                        .into_iter()
                        .map(|entry| (entry, *by)),
                );
            }
        }
        Self {
            level: Level::Full,
            held: Vec::new(),
            widened,
            denied,
            hosts: reached,
            reach: reach::Reach::of(layered).never(locks.never_hosts().to_vec()),
            writable,
            os: machine.os,
            homes: Homes {
                codex_project: Homes::codex_project(machine, root),
                ..Homes::of(machine)
            },
        }
    }
}

/// What a harness's compiler cannot hold on this machine, so the chat does not start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Uncompilable {
    pub harness: Harness,
    pub unheld: Unheld,
}

/// What a compiler could not hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unheld {
    /// A class held by a service that neither the store nor the harness's sandbox holds on
    /// this system.
    Service(Service, Os),
    /// The harness has no sandbox of its own, and charter cannot wrap it on this system yet,
    /// so no class can be held.
    Wrap(Os),
}

impl Uncompilable {
    /// The one class that could not be held, where it was one class.
    pub fn class(self) -> Option<Class> {
        match self.unheld {
            Unheld::Service(service, _) => Some(service.class()),
            Unheld::Wrap(_) => None,
        }
    }
}

/// The sandbox a chat starts under, compiled for its harness by that harness's compiler.
///
/// Made only by [`for_start`], for the harness it was asked about, and it says which
/// ([`Self::harness`]): the one place a session opens refuses one handed to a chat of another
/// harness, so a form compiled for one harness never reaches another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    harness: Harness,
    /// How much it holds ([`Level`]).
    level: Level,
    /// Boxed: a wrap carries its grants and widenings, and [`Decided`] carries this beside a
    /// variant of a few words.
    form: Box<Form>,
    /// The plane it was compiled for, which the chat's folder must be inside.
    root: PathBuf,
    /// The paths it denies, which must not cover the chat's folder ([`covering`]).
    denied: Vec<Denial>,
    /// The project's package caches it lets a chat write ([`Widened::caches`]).
    caches: Option<Box<caches::CacheHome>>,
    /// What a command run on the chat's behalf is held to. Boxed: it is a list of paths, and
    /// [`Decided`] carries this beside a variant of a few words.
    confines: Box<Confines>,
    /// The hosts it lets the chat reach, each with its layer: what its proxy decides by
    /// (#1664). Boxed for [`Decided`]'s reason.
    reach: Box<reach::Reach>,
    /// Whether the chat's network goes through purlis's proxy (#1665): always for a harness
    /// purlis wraps, and for Claude Code once its program answered a version that takes the
    /// proxy's ports ([`Self::answered`]).
    through_proxy: bool,
    /// What [`Self::answered`] found to say once about an older Claude Code, and the version it
    /// names, marked said once a tab shows it ([`older_shown`]).
    older: Option<(String, ClaudeCodeVersion)>,
    /// The persona's tools for this chat (#1407), written as its line is decided.
    tools: Option<Box<tools::Resolved>>,
    /// purlis's own program the wrap starts the chat under on Linux, as it was checked and held
    /// ([`program::checked_wrap`], #1040); none until then, and the line is refused.
    wrap: Option<PathBuf>,
}

/// The first Claude Code whose sandbox takes a proxy's ports from `--settings` (#1665):
/// `sandbox.network.httpProxyPort` and `socksProxyPort`.
pub const CLAUDE_CODE_TAKES_PROXY_PORTS: (u32, u32, u32) = (2, 1, 285);

/// A Claude Code version, as `--version` answers it: (0, 0, 0) for an answer with none.
pub type ClaudeCodeVersion = (u32, u32, u32);

/// The Claude Code versions an older one was already said for, in this process: said once,
/// counted from a tab showing it ([`older_shown`]).
static OLDER_SAID: std::sync::Mutex<Vec<ClaudeCodeVersion>> = std::sync::Mutex::new(Vec::new());

/// Whether the sentence about an older Claude Code `version` was shown on a tab in this
/// process ([`older_shown`]).
pub fn older_was_shown(version: ClaudeCodeVersion) -> bool {
    OLDER_SAID
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains(&version)
}

/// **The sentence about an older Claude Code `version` was shown on a tab** (#1699): no other
/// chat in this process says it again. Marked only once shown, so a chat that never reached
/// its tab does not use up the one telling.
pub fn older_shown(version: ClaudeCodeVersion) {
    let mut already = OLDER_SAID.lock().unwrap_or_else(|e| e.into_inner());
    if !already.contains(&version) {
        already.push(version);
    }
}

impl Applied {
    /// What a Claude Code chat's program answered `--version` ([`program::checked_answering`],
    /// #1665): from [`CLAUDE_CODE_TAKES_PROXY_PORTS`] on, its network goes through purlis's
    /// proxy, its pair of ports named in its `--settings`. An older one, or an answer with no
    /// version in it, keeps Claude Code's own proxy and the allowed domains, as before, and the
    /// first such chat on that version in this process carries the sentence that says so
    /// ([`Self::older_notice`]). A harness purlis wraps goes through the proxy whatever it
    /// answers.
    pub fn answered(&mut self, answer: &str) {
        if self.harness != Harness::ClaudeCode {
            return;
        }
        let version = program::claude_code_version(answer);
        self.through_proxy = version.is_some_and(|it| it >= CLAUDE_CODE_TAKES_PROXY_PORTS);
        self.older = None;
        if self.through_proxy {
            return;
        }
        let key = version.unwrap_or_default();
        if older_was_shown(key) {
            return;
        }
        let dotted = |(major, minor, patch): (u32, u32, u32)| format!("{major}.{minor}.{patch}");
        let newer = dotted(CLAUDE_CODE_TAKES_PROXY_PORTS);
        let this = version.map_or_else(
            || "This Claude Code is".to_owned(),
            |version| format!("Claude Code {} is", dotted(version)),
        );
        self.older = Some((
            format!(
                "{this} older than {newer}, so its chats reach the network through Claude \
                 Code's own proxy and the same allowed hosts as before, and their connections \
                 are not in purlis's network record. Update Claude Code to {newer} or later to \
                 see them there."
            ),
            key,
        ));
    }

    /// The sentence a chat on a Claude Code older than [`CLAUDE_CODE_TAKES_PROXY_PORTS`] says as
    /// it starts, once per version in this process ([`Self::answered`]); `None` for every other.
    /// With the version it names, for [`older_shown`] once a tab shows it.
    pub fn older_notice(&self) -> Option<(&str, ClaudeCodeVersion)> {
        self.older
            .as_ref()
            .map(|(said, version)| (said.as_str(), *version))
    }

    /// Whether the chat's network goes through purlis's proxy (#1665): what
    /// [`Self::confine_telling`] starts one for.
    pub fn through_purlis_proxy(&self) -> bool {
        self.through_proxy
    }

    /// The harness it was compiled for.
    pub fn harness(&self) -> Harness {
        self.harness
    }

    /// How much it holds ([`Level`]).
    pub fn level(&self) -> Level {
        self.level
    }

    /// The checked program the wrap starts this chat under ([`Self::wrap_with`]).
    pub fn wrap_program(&self) -> Option<&Path> {
        self.wrap.as_deref()
    }

    /// Starts this chat's wrap with `program`, as [`program::checked_wrap`] checked it, held at
    /// the floor as the harness's program is.
    pub fn wrap_with(&mut self, program: PathBuf) {
        self.hold_programs(&[program.display().to_string()]);
        self.confines.wrap = Some(program.clone());
        self.wrap = Some(program);
    }

    /// **On Linux, purlis's own program checked and held for this chat** (#1040, #2342), as
    /// [`program::checked_wrap`] checks it for `chat`: the wrap a chat purlis holds whole starts
    /// under, and the one a brokered `secret exec` for any sandboxed chat runs its command in
    /// ([`Confines::wrap`]). A chat purlis wraps does not start without it; a chat whose
    /// harness holds itself starts, and a run for it is refused. Nothing elsewhere.
    pub fn hold_wrap(&mut self, chat: program::Chat<'_>) -> Result<(), NotStarted> {
        if Os::this() != Os::Linux {
            return Ok(());
        }
        match program::checked_wrap(&self.root, chat) {
            Ok(wrap) => {
                self.wrap_with(wrap);
                Ok(())
            }
            Err(refused) if self.wrapped_by_purlis_here() => Err(refused),
            Err(_) => Ok(()),
        }
    }

    /// **What purlis arms this chat with, never written by it** ([`ArmedWith`]): purlis's own
    /// program its hooks run, its plugin and its git hooks, which later chats run too.
    ///
    /// - **Refused where the chat writes**, at every level, the program and the plugin, as
    ///   [`program::checked_wrap`] refuses purlis's own program: the project, the chat's folder
    ///   `cwd`, what its sandbox lets it write (`env`'s temp folders among them at the full
    ///   sandbox) and what a person let it write, each asked as written and as the kernel names
    ///   it.
    /// - **Held at the floor**: the program and the whole install it belongs to
    ///   ([`install_root`]: an app bundle, or its [`package_root`]), the plugin and the git
    ///   hooks, as written and as the kernel names them, also inside a folder a person let the
    ///   chat write. None that holds the project or is inside it, which the chat writes, and none
    ///   that holds the home folder. The full sandbox lists what a chat writes, so nothing is
    ///   added there.
    pub fn hold_kit(
        &mut self,
        kit: ArmedWith<'_>,
        cwd: &Path,
        env: &[(String, String)],
    ) -> Result<(), NotStarted> {
        let mut places = vec![self.root.clone(), cwd.to_path_buf()];
        places.extend(self.writable());
        places.extend(self.confines.writable.iter().cloned());
        if self.level == Level::Full {
            places.extend(program::temp_roots(env));
        }
        let places: Vec<(PathBuf, PathBuf)> = places
            .into_iter()
            .map(|dir| {
                let real_dir = dir.canonicalize().unwrap_or_else(|_| dir.clone());
                (dir, real_dir)
            })
            .collect();
        for refused in kit.binary.into_iter().chain(kit.plugin) {
            let resolved = real(refused);
            if let Some(path) = [refused, resolved.as_path()]
                .into_iter()
                .find(|path| program::writable(path, &places))
            {
                return Err(NotStarted::KitWritable(path.to_path_buf()));
            }
        }
        if self.level != Level::Floor {
            return Ok(());
        }
        let root = real(&self.root);
        let home = dirs::home_dir().map(|home| real(&home));
        let mut more: Vec<PathBuf> = Vec::new();
        let install = kit.binary.map(install_root);
        for path in kit
            .binary
            .into_iter()
            .chain(install.as_deref())
            .chain(kit.plugin)
            .chain(kit.git_hooks)
            .chain(kit.skill_sets)
            .flat_map(|path| [path.to_path_buf(), real(path)])
        {
            let around_root = root.starts_with(&path) || path.starts_with(&root);
            let above_home = home.as_deref().is_some_and(|home| home.starts_with(&path));
            if !around_root && !above_home && path.parent().is_some() && !more.contains(&path) {
                more.push(path);
            }
        }
        self.hold_past_grants(more);
        Ok(())
    }

    /// Whether this chat's line starts with purlis's own program: a harness purlis holds
    /// whole, on Linux (#1040).
    pub fn wrapped_by_purlis_here(&self) -> bool {
        Os::this().purlis_wraps(self.harness())
    }

    /// **At the floor, the programs `words` start, never written by the chat** (#2071, the
    /// operator's ruling Q4): the first word as written (a link on the `PATH` among them) and the
    /// folder of the real file it names, held as [`Compiled::held`] holds the `PATH`. The full
    /// sandbox lists what a chat writes, and [`program::checked`] refuses a program there, so it
    /// is left as it is. A folder that holds the project, or is inside it, is not held: the chat
    /// writes its project, and the program check refuses a program there.
    pub fn hold_programs(&mut self, words: &[String]) {
        if self.level != Level::Floor {
            return;
        }
        let root = real(&self.root);
        let mut more: Vec<PathBuf> = Vec::new();
        for word in words.iter().take(1) {
            let written = PathBuf::from(word);
            if !written.is_absolute() {
                continue;
            }
            // The whole package it belongs to, a harness's install root (#2071, review item 5).
            let folder = Some(package_root(&real(&written)));
            for path in std::iter::once(written).chain(folder) {
                let around_root = root.starts_with(&path) || path.starts_with(&root);
                if !around_root && path.parent().is_some() && !more.contains(&path) {
                    more.push(path);
                }
            }
        }
        self.hold(more);
    }

    /// `more` held at the floor besides what was compiled: in the form, and for a run for the chat.
    /// One inside a folder a person let the chat write is left to that grant.
    fn hold(&mut self, more: Vec<PathBuf>) {
        let more: Vec<PathBuf> = more
            .into_iter()
            .filter(|path| {
                !self
                    .confines
                    .writable
                    .iter()
                    .any(|folder| path.starts_with(folder))
            })
            .collect();
        self.hold_past_grants(more);
    }

    /// [`Self::hold`], inside a folder a person let the chat write too: a grant lifts a denial
    /// class's names no more than it lifts these.
    fn hold_past_grants(&mut self, more: Vec<PathBuf>) {
        let more: Vec<PathBuf> = more
            .into_iter()
            .filter(|path| !self.confines.held.contains(path))
            .collect();
        if more.is_empty() {
            return;
        }
        self.confines.held.extend(more.iter().cloned());
        match &mut *self.form {
            Form::ClaudeCode(settings) => *settings = settings.holding(&more),
            Form::Codex(codex::Wrap { held, .. }) | Form::Opencode(opencode::Wrap { held, .. }) => {
                held.extend(more);
            }
        }
    }

    /// **This sandbox for a chat on the profile whose folders are `folders`** (#2037, ADR 0095
    /// §5): the profile's sign-in files ([`signin::credential_files`]) denied to the agent's
    /// commands, reading and writing, with the human-powers class, where the harness runs its
    /// commands apart from itself. Claude Code does: its sandbox holds the commands its Bash tool
    /// runs, and a deny rule its Read and Edit tools, while its own process reads its sign-in.
    /// Codex and opencode run whole inside purlis's wrap with their commands, so what the harness
    /// reads its commands can read too, and their declarations say so (`[sign_in]
    /// kept_from_commands`). A wrapped Codex is also started in its profile's folder
    /// ([`codex::Wrap::on_profile`]).
    pub fn on_profile(&mut self, folders: &signin::ProfileFolders) {
        // A wrapped Codex runs in its profile's folder: one purlis made for sandboxed chats
        // only (#2037, ruled 2026-10-15), never the person's own Codex home.
        if let Form::Codex(wrap) = &mut *self.form
            && let (Some(name), Some(folder)) = (&folders.sandbox_only, &folders.config)
        {
            wrap.on_profile(name, folder);
        }
        let Form::ClaudeCode(settings) = &mut *self.form else {
            return;
        };
        let more: Vec<Denial> = signin::credential_files(self.harness, folders)
            .into_iter()
            .map(|path| Denial {
                class: Class::HumanPowers,
                path,
                access: Access::ReadWrite,
                named: None,
                reach: Reach::AndBelow,
                any_case: false,
            })
            .filter(|denial| !self.denied.contains(denial))
            .collect();
        *settings = settings.denying(&more);
        self.confines.denied.extend(more.iter().cloned());
        self.denied.extend(more);
    }

    /// The plane it was compiled for.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// What a command purlis runs for this chat is held to ([`Confines`]).
    pub fn confines(&self) -> &Confines {
        &self.confines
    }

    /// The folders this sandbox lets a chat write besides its own and the temp directories:
    /// opencode's own data, or what a Codex turn writes in Codex's home, under charter's wrap;
    /// and, for every harness, what the project's package caches let a chat write.
    pub fn writable(&self) -> Vec<PathBuf> {
        let mut out = match &*self.form {
            Form::Opencode(wrap) => wrap.data.iter().cloned().collect(),
            Form::Codex(wrap) => wrap.writable(),
            Form::ClaudeCode(_) => Vec::new(),
        };
        out.extend(self.caches.iter().flat_map(|caches| caches.writable()));
        out
    }

    /// The form as compiled for the project, before the manifests of a chat's own folders are
    /// added: what a chat is handed is [`Self::form_in`] its folder.
    pub fn form(&self) -> &Form {
        &self.form
    }

    /// The chat's whole line under this sandbox, from `words` ([`Words`]), where it opens,
    /// `at`; or the one sentence saying why it may not start.
    ///
    /// **Fail closed** (ADR 0067). What is checked here holds for every harness: a folder to
    /// open in, never one reached through a link, and none the sandbox denies at or above it;
    /// and the project's package caches. Where the sandbox goes on the line, and which of the
    /// chat's own words refuse it, is each harness's own: its adapter's
    /// [`crate::harness::HarnessAdapter::sandboxed_line`].
    pub fn line(&self, words: Words, at: &At<'_>) -> Result<Line, String> {
        // Ruling of 2026-10-03, every harness: a folder reached through a link is not the
        // folder the rules name, so no sandboxed chat starts in one.
        // Every refusal here is said under the policy in force (#1423): where it forbids the
        // opt-out, none names it, and each ends with the policy and who set it.
        let locks = policy::Locks::of(&self.root);
        let locked = locks.opt_out_refused();
        // The floor confines a chat to no folder (#2071): a chat with none of its own is read
        // at the project's root, and one outside the project or reached through a link is not
        // refused for it. What a folder's denials hold, below, holds at every level.
        let floor = self.level() == Level::Floor;
        let cwd = match at.cwd {
            Some(cwd) => cwd,
            None if floor => self.root.as_path(),
            None => return Err(FolderRefusal::Missing.said(&locks)),
        };
        if !floor && let Some(why) = folder_refusal(&self.root, cwd) {
            return Err(why.said(&locks));
        }
        // #1327: never a chat whose own folder, or one above it, it may not write — by what
        // was compiled, or by a manifest of its folders that is a link to one of them (#1336).
        let ground: Vec<PathBuf> = cwd.ancestors().map(Path::to_path_buf).collect();
        let chain = self.chain(Some(cwd));
        if let Some(refused) = covering(&self.denied, &ground).or_else(|| covering(&chain, &ground))
        {
            return Err(refused.placed_in(&self.root).said(&locks));
        }
        // An adapter's own refusal, and the caches' below, ends with the policy too, and leads
        // with it (#1431).
        let under = |why: String| under_policy(&locks, why);
        let at = &At {
            no_opt_out: locked.is_some(),
            cwd: Some(cwd),
            ..*at
        };
        // A chat whose network goes through purlis's proxy is never started without it (#1665):
        // a Claude Code chat's line without its ports would leave it on its own proxy, unseen.
        // (A wrapped harness's adapter refuses that itself.)
        if self.harness == Harness::ClaudeCode && self.through_proxy && at.confinement.is_none() {
            return Err(under(format!(
                "{LEAD}, and purlis's network proxy was not started for this Claude Code chat, \
                 so nothing was started."
            )));
        }
        let mut line = self
            .harness
            .adapter()
            .sandboxed_line(&self.with(chain), words, at)
            .map_err(under)?;
        // ssh, and git over ssh, through the chat's SOCKS port (#1667): each harness wrapped
        // whole ([`Self::ssh_route`]).
        if let Some(route) = self.ssh_route(at.confinement) {
            let gained = route.env();
            line.env
                .retain(|(key, _)| !gained.iter().any(|(set, _)| set == key));
            line.env.extend(gained);
        }
        // The project's package caches (D-1337-6): made here, outside the sandbox, with any link
        // a chat planted in them taken out; refused, naming the path, only where one stays.
        if let Some(caches) = &self.caches {
            caches.prepare().map_err(under)?;
            line.env
                .retain(|(key, _)| !caches.env.iter().any(|(set, _)| set == key));
            line.env.extend(caches.env.iter().cloned());
        }
        // The persona's tools (#1407): their shims first on the chat's `PATH`.
        if let Some(resolved) = &self.tools {
            let gained = tools::armed(resolved, self.harness);
            line.env
                .retain(|(key, _)| !gained.iter().any(|(set, _)| set == key));
            line.env.extend(gained);
        }
        // ssh in a Claude Code chat (#1667): its route, written where no chat writes, goes
        // first on each command's `PATH` through the one file Claude Code reads before each
        // command, never on the harness's own `PATH`. The persona's tools go before it there.
        if let Some(route) = self.claude_ssh_route(at.confinement) {
            let tools_first = line
                .env
                .iter()
                .find(|(key, _)| key == tools::FIRST_ON_PATH)
                .map(|(_, dir)| PathBuf::from(dir));
            match route.write_claude_env(tools_first.as_deref()) {
                Ok(file) => {
                    line.env.retain(|(key, _)| key != tools::CLAUDE_ENV_FILE);
                    line.env.push((
                        tools::CLAUDE_ENV_FILE.to_owned(),
                        file.display().to_string(),
                    ));
                }
                Err(err) => {
                    tracing::warn!("purlis: a chat's ssh route was not handed on ({err})");
                }
            }
        }
        Ok(line)
    }

    /// What that harness is handed for a chat in `cwd`: [`Self::form`], with the manifests in
    /// each folder from `cwd` up to the project root denied too ([`manifests_held`], #1336).
    /// The one form a chat is armed with and opened under, so both name the same folders.
    pub fn form_in(&self, cwd: Option<&Path>) -> Form {
        self.with(self.chain(cwd))
    }

    /// The manifests a chat in `cwd` is denied that the compiled form does not already deny.
    fn chain(&self, cwd: Option<&Path>) -> Vec<Denial> {
        manifests_held(&self.root, cwd)
            .into_iter()
            .filter(|denial| !self.denied.contains(denial))
            .collect()
    }

    /// The compiled form with `more` denied too.
    fn with(&self, more: Vec<Denial>) -> Form {
        let mut form = (*self.form).clone();
        match &mut form {
            Form::ClaudeCode(settings) => *settings = settings.denying(&more),
            Form::Codex(codex::Wrap { denied, .. })
            | Form::Opencode(opencode::Wrap { denied, .. }) => denied.extend(more),
        }
        form
    }

    /// What has to run for as long as a chat under this sandbox does, started now: charter's
    /// egress proxy and the chat's own temp directory, for a harness charter wraps ([`Form::
    /// Opencode`], [`Form::Codex`]) and for a Claude Code that goes through purlis's proxy
    /// ([`Self::through_purlis_proxy`], #1665); `None` for an older Claude Code, whose own
    /// proxy holds the policy.
    pub fn confine(&self) -> std::io::Result<Option<Confinement>> {
        self.confine_keeping(egress::Refusals::default())
    }

    /// [`Self::confine`], its proxy keeping what it refuses in `refusals`, which may tell the
    /// app each host as it is refused (#1663): the wrapped harness's own road for a block, by
    /// the proxy's word, never the chat's.
    pub fn confine_keeping(
        &self,
        refusals: egress::Refusals,
    ) -> std::io::Result<Option<Confinement>> {
        self.confine_telling(refusals, None)
    }

    /// **The ssh route a chat of this harness is handed** (#1667), from `confinement`: only
    /// where the sandbox wraps the whole harness (Codex, opencode), so everything that reads the
    /// route's `PATH` and `GIT_SSH_COMMAND` runs inside it. The route sits in the chat's own temp
    /// directory, which a wrapped chat may write; a Claude Code chat's harness runs outside its
    /// sandbox, with its hooks and its servers, so a file there that a chat could have written is
    /// never on that harness's `PATH` and never its git's ssh. Claude Code's own sandbox sets
    /// `GIT_SSH_COMMAND` for each command it runs, through the same SOCKS port, and a bare `ssh`
    /// there takes a route written where no chat writes ([`Self::claude_ssh_route`]).
    pub fn ssh_route<'c>(
        &self,
        confinement: Option<&'c Confinement>,
    ) -> Option<&'c tunnel::SshRoute> {
        if self.harness == Harness::ClaudeCode {
            return None;
        }
        confinement.and_then(Confinement::ssh_route)
    }

    /// **A Claude Code chat's ssh route** (#1667), from `confinement`: only one written in this
    /// project's routes folder ([`tunnel::routes_folder`]), where no chat writes, and handed to
    /// the commands its sandbox runs alone ([`Self::line`]).
    pub fn claude_ssh_route<'c>(
        &self,
        confinement: Option<&'c Confinement>,
    ) -> Option<&'c tunnel::SshRoute> {
        if self.harness != Harness::ClaudeCode {
            return None;
        }
        confinement
            .and_then(Confinement::ssh_route)
            .filter(|route| route.is_within(&tunnel::routes_folder(&self.root)))
    }

    /// [`Self::confine_keeping`], its proxy also telling `reached` each connection it carried
    /// (#1664), coalesced: what the app keeps every connection in the network record by. The
    /// proxy decides by the layers this was compiled with ([`reach`]).
    pub fn confine_telling(
        &self,
        refusals: egress::Refusals,
        reached: Option<egress::Reached>,
    ) -> std::io::Result<Option<Confinement>> {
        self.confine_asking(refusals, reached, None)
    }

    /// [`Self::confine_telling`], its proxy holding a connection to a host nothing lists on
    /// `asks` while the person is asked (#1666); none refuses it at once. It starts
    /// [`Self::plan`] here, as the session host starts it there (#1999).
    pub fn confine_asking(
        &self,
        refusals: egress::Refusals,
        reached: Option<egress::Reached>,
        asks: Option<std::sync::Arc<asks::Asks>>,
    ) -> std::io::Result<Option<Confinement>> {
        self.plan()
            .map(|plan| {
                plan.start(plan::Telling {
                    refusals,
                    reached,
                    asks,
                })
            })
            .transpose()
    }

    /// **What this chat's confinement is started from** (#1999), wherever it starts: the layers
    /// this was compiled with ([`reach`]), its ports, and what runs beside its proxy. `None`
    /// where nothing runs beside the chat: an older Claude Code keeps its own proxy (#1665).
    pub fn plan(&self) -> Option<plan::Plan> {
        self.through_proxy.then(|| self.planned())
    }

    /// [`Self::plan`], whether or not anything runs beside the chat.
    fn planned(&self) -> plan::Plan {
        let claude = matches!(*self.form, Form::ClaudeCode(_));
        plan::Plan {
            reach: self.reach.written(),
            tunnel_ports: egress::TUNNEL_PORTS.to_vec(),
            plain_ports: egress::PLAIN_PORTS.to_vec(),
            // A host listed without a port on every port for Claude Code, as its own proxy
            // carried it (#1665).
            any_port: claude,
            // Claude Code's harness runs outside its sandbox and keeps its own temp folder, so
            // its chat needs the proxy alone (#1699), and its ssh route where no chat writes
            // (#1667).
            beside: if claude {
                plan::Beside::ProxyOnly {
                    routes: plan::routes(&self.root),
                }
            } else {
                plan::Beside::Wrapped
            },
            given: self.confines.widened.ports.clone(),
        }
    }

    /// What this chat's proxy serves ([`Self::confine_telling`]): the layers this was compiled
    /// with ([`reach`]), and for Claude Code a host listed without a port on every port, as
    /// Claude Code's own proxy carried it (#1665), so a chat reaches what it reached before.
    pub fn serving(
        &self,
        refusals: egress::Refusals,
        reached: Option<egress::Reached>,
    ) -> egress::Serving {
        self.planned().serving(
            (*self.reach).clone(),
            plan::Telling {
                refusals,
                reached,
                asks: None,
            },
        )
    }
}

/// A chat's words, before the sandbox decides its line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Words {
    /// The program the chat runs: the harness, or the profile's wrapper of it.
    pub program: String,
    /// The rest of the profile's own command.
    pub command: Vec<String>,
    /// What arms the harness for this chat.
    pub armed: Vec<String>,
    /// Charter's own words, which end the line.
    pub charters: Vec<String>,
}

/// What only the place a session opens knows about it.
#[derive(Debug, Clone, Copy, Default)]
pub struct At<'a> {
    /// The directory the chat runs in.
    pub cwd: Option<&'a Path>,
    /// The socket its hooks report on, where the app listens on one.
    pub hook_socket: Option<&'a Path>,
    /// What [`Applied::confine`] started for it.
    pub confinement: Option<&'a Confinement>,
    /// Whether an administrator's policy forbids starting a chat without the sandbox
    /// ([`policy::Locks::forbids_opt_out`]): a refusal an adapter writes then names no opt-out
    /// (#1423). [`Applied::line`] sets it from the policy in force, and ends every refusal with
    /// the policy and who set it; a caller leaves it `false`.
    pub no_opt_out: bool,
    /// purlis's own program, which a wrapped chat's line starts with on Linux (#1040).
    pub purlis: Option<&'a Path>,
}

/// A chat's line under its sandbox: the program that runs, its arguments, and what its
/// environment gains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

/// What runs beside a chat charter wraps, for as long as it lives: the loopback proxy its
/// traffic leaves through ([`egress`]) and a temp directory of its own, which a Claude Code
/// chat's is without ([`Self::proxy_only`]). Both end when this is dropped.
///
/// **Here, or held elsewhere** (#1999): the proxy runs in this process, or in the session host,
/// which started it from this chat's [`plan::Plan`] and holds it for as long as the chat runs
/// ([`Self::held_elsewhere`]). Either way the chat is named the same ports and folders, and the
/// window answers its board of live asks the same way ([`Board`]).
#[derive(Debug)]
pub struct Confinement {
    proxy: Proxied,
    tmp: Option<Folder>,
    /// Its proxy's live asks (#1666), where it holds connections while the person is asked.
    asks: Option<std::sync::Arc<dyn Board>>,
    /// Its ssh route through its SOCKS port (#1667), in its temp directory; none where it
    /// could not be written, and ssh then reaches nothing, as before.
    ssh: Option<tunnel::SshRoute>,
    /// The folder a Claude Code chat's route was written in, where no chat writes
    /// ([`Self::proxy_only_routed`]): removed as this ends.
    routed: Option<Folder>,
    /// A folder of the chat's own made for what it keeps ([`Self::keep_own`]) where it has
    /// neither of the two above: removed as this ends.
    kept: Option<tempfile::TempDir>,
    /// Where the chat's servers listen and the doors to them (#2019), once
    /// [`Self::serve`] opened them: held in the window's process wherever the proxy runs.
    serving: Option<Serving>,
}

/// **Where a chat's servers listen, and the doors to them** (#2019, [`door`]).
#[derive(Debug)]
struct Serving {
    /// The folder purlis made for this chat's sockets alone, `0700`.
    folder: PathBuf,
    /// A door for each port the chat serves on.
    doors: Vec<door::Door>,
}

/// Where a chat's proxy runs.
#[derive(Debug)]
enum Proxied {
    /// Boxed: a proxy holds its listeners' threads, and the other variant only two ports.
    Here(Box<egress::Proxy>),
    Elsewhere(Elsewhere),
}

/// A proxy the session host holds (#1999): its ports, purlis's own in this process too while it
/// is held on this machine, and what reaches it.
#[derive(Debug)]
struct Elsewhere {
    http: u16,
    socks: u16,
    held: Box<dyn HeldElsewhere>,
    left_out: Vec<u16>,
    host_name: Option<String>,
    /// Whether it listens on this machine, whose ports it holds as purlis's own: a runner's
    /// listens on the runner's (#2446).
    on_this_machine: bool,
}

impl Drop for Elsewhere {
    fn drop(&mut self) {
        if self.on_this_machine {
            localport::release(self.http);
            localport::release(self.socks);
        }
    }
}

/// A folder beside a chat: made here and removed as it ends, or made by whoever holds its proxy.
#[derive(Debug)]
enum Folder {
    Own(tempfile::TempDir),
    Theirs(PathBuf),
}

impl Folder {
    fn path(&self) -> &Path {
        match self {
            Self::Own(dir) => dir.path(),
            Self::Theirs(path) => path,
        }
    }
}

/// **A chat's board of live asks, as the window answers it** (#1666): [`asks::Asks`] beside a
/// proxy in this process, or the board the session host holds beside its proxy (#1999). Every
/// answer comes from the window; nothing a chat sends reaches a board.
pub trait Board: Send + Sync + std::fmt::Debug {
    /// Whether the person is being asked about `host` ([`asks::Asks::asks_about`]).
    fn asks_about(&self, host: &hosts::Host) -> bool;
    /// The person allowed `host` at `by` ([`asks::Asks::allow`]): the hosts held connections
    /// gave up on before it, which the chat is told to retry.
    fn allow(&self, host: &hosts::Host, by: reach::By) -> Vec<String>;
    /// The person kept `host` blocked ([`asks::Asks::keep_blocked`]).
    fn keep_blocked(&self, host: &hosts::Host);
    /// An Allow of `host` at `by` was removed ([`asks::Asks::forget`]).
    fn forget(&self, host: &hosts::Host, by: reach::By);
    /// The hosts allowed live, each with its scope ([`asks::Asks::allowed_live`]).
    fn allowed_live(&self) -> Vec<(String, reach::By)>;
    /// The person allowed the service on `port` of the machine its proxy runs on, left out of
    /// the chat's plan, for this chat ([`asks::Asks::allow_left_out`], #2510): the targets held
    /// connections gave up on before it, which the chat is told to retry; or why the host that
    /// holds the proxy did not take it.
    fn allow_left_out(&self, port: u16) -> Result<Vec<String>, String>;
    /// The person kept that service blocked ([`asks::Asks::keep_left_out_blocked`], #2510).
    fn keep_left_out_blocked(&self, port: u16);
}

impl Board for asks::Asks {
    fn asks_about(&self, host: &hosts::Host) -> bool {
        asks::Asks::asks_about(self, host)
    }

    fn allow(&self, host: &hosts::Host, by: reach::By) -> Vec<String> {
        asks::Asks::allow(self, host, by)
    }

    fn keep_blocked(&self, host: &hosts::Host) {
        asks::Asks::keep_blocked(self, host);
    }

    fn forget(&self, host: &hosts::Host, by: reach::By) {
        asks::Asks::forget(self, host, by);
    }

    fn allowed_live(&self) -> Vec<(String, reach::By)> {
        asks::Asks::allowed_live(self)
    }

    fn allow_left_out(&self, port: u16) -> Result<Vec<String>, String> {
        Ok(asks::Asks::allow_left_out(self, port))
    }

    fn keep_left_out_blocked(&self, port: u16) {
        asks::Asks::keep_left_out_blocked(self, port);
    }
}

/// **What holds a chat's proxy in another process** (#1999): the session host. Dropping it lets
/// the holder end the proxy.
pub trait HeldElsewhere: Send + Sync + std::fmt::Debug {
    /// What the holder calls this confinement.
    fn id(&self) -> &str;
    /// `port` of this machine is carried no longer ([`Confinement::withdraw_port`]), or why the
    /// holder could not be told. A holder that could not be told ends the proxy where it can,
    /// so a port whose grant was removed is never carried on.
    fn withdraw_port(&self, port: u16) -> Result<(), String>;
    /// Its board of live asks, where its proxy asks live.
    fn board(&self) -> Option<std::sync::Arc<dyn Board>>;
}

/// `folder` and the folder it is in, each made `0700` by this process where it is not there,
/// and refused where either is not a folder of this user's that no one else may write (#2019):
/// no other user reaches a socket in it, or puts one there.
fn own_folder(folder: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    let me = rustix::process::getuid().as_raw();
    let parent = folder
        .parent()
        .ok_or_else(|| std::io::Error::other("a server's folder has no folder above it"))?;
    for at in [parent, folder] {
        match std::fs::DirBuilder::new().mode(0o700).create(at) {
            Err(err) if err.kind() != std::io::ErrorKind::AlreadyExists => return Err(err),
            _ => {}
        }
        let meta = std::fs::symlink_metadata(at)?;
        if !meta.is_dir() || meta.uid() != me {
            return Err(std::io::Error::other(format!(
                "{} is not a folder of this user's",
                at.display()
            )));
        }
        if meta.permissions().mode() & 0o077 != 0 {
            std::fs::set_permissions(at, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    Ok(())
}

/// Clears `path` for a server's socket: what was left there, a socket a server that ended left
/// or anything else that is not a folder, is taken away (the folder is purlis's, and only the
/// chat's own socket is ever made in it). A folder there is refused, and said.
fn cleared(path: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => Err(std::io::Error::other(format!(
            "a folder is where its socket goes, {}",
            path.display()
        ))),
        Ok(_) => std::fs::remove_file(path),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

/// The doors to `ports`, each to its socket in `folder` ([`Confinement::serve`]), and the ports
/// something on this Mac listens on already, which get none.
fn doors_to(folder: &Path, ports: &[u16]) -> std::io::Result<(Vec<door::Door>, Vec<u16>)> {
    own_folder(folder)?;
    let mut doors = Vec::new();
    let mut unserved = Vec::new();
    for port in ports {
        let socket = localport::serve_socket(folder, *port);
        cleared(&socket)?;
        match door::Door::open(*port, &socket) {
            Ok(door) => doors.push(door),
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => unserved.push(*port),
            Err(err) => return Err(err),
        }
    }
    Ok((doors, unserved))
}

/// **Where a confinement is**: its proxy's two ports, and the folders beside it. What the
/// session host says of one it started (#1999).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    pub http: u16,
    pub socks: u16,
    /// The chat's own temp folder.
    pub tmp: Option<PathBuf>,
    /// The folder a Claude Code chat's ssh route was written in.
    pub routed: Option<PathBuf>,
    /// The ssh route's folder.
    pub ssh: Option<PathBuf>,
    /// The services the chat was planned to reach on the host's machine for it alone, which
    /// that host started it without (#2510).
    pub left_out: Vec<u16>,
    /// The name of the host's machine, where it left a service out (#2510).
    pub host_name: Option<String>,
}

impl Placed {
    /// Where a confinement the session host started is, as the session protocol says it.
    pub fn of_wire(confined: &purlis_session_protocol::session::Confined) -> Placed {
        Placed {
            http: confined.http,
            socks: confined.socks,
            tmp: confined.tmp.as_ref().map(PathBuf::from),
            routed: confined.routed.as_ref().map(PathBuf::from),
            ssh: confined.ssh.as_ref().map(PathBuf::from),
            left_out: confined.left_out.clone(),
            // Said in the window: held to a name's own characters, whatever the host sent.
            host_name: confined
                .host_name
                .as_deref()
                .map(crate::dispatch::host_of)
                .filter(|name| name != "unknown"),
        }
    }
}

impl Confinement {
    /// A proxy carrying `hosts`, and a new temp directory.
    pub fn start(hosts: Vec<String>) -> std::io::Result<Self> {
        Self::start_keeping(hosts, egress::Refusals::default())
    }

    /// [`Self::start`], with its proxy keeping what it refuses in `refusals`.
    pub fn start_keeping(hosts: Vec<String>, refusals: egress::Refusals) -> std::io::Result<Self> {
        Self::serving(egress::Serving {
            refusals,
            ..egress::Serving::of(reach::Reach::open(hosts))
        })
    }

    /// A proxy serving `serving` on a pair of ports of its own (#1664), and a new temp directory.
    pub fn serving(serving: egress::Serving) -> std::io::Result<Self> {
        let asks = serving.asks.clone();
        let proxy = egress::Proxy::serving(serving)?;
        let tmp = tempfile::Builder::new().prefix("charter-chat-").tempdir()?;
        let ssh = tunnel::SshRoute::write(tmp.path(), proxy.socks_port())
            .inspect_err(|err| {
                tracing::warn!("purlis: a chat's ssh route was not written ({err})");
            })
            .ok();
        Ok(Self {
            proxy: Proxied::Here(Box::new(proxy)),
            tmp: Some(Folder::Own(tmp)),
            asks: asks.map(|asks| asks as std::sync::Arc<dyn Board>),
            ssh,
            routed: None,
            kept: None,
            serving: None,
        })
    }

    /// A proxy serving `serving`, and nothing beside it (#1699): for a Claude Code chat, whose
    /// harness keeps its own temp folder and whose git sets its own ssh through the proxy's
    /// SOCKS port, so a temp directory and an ssh route would never be used.
    pub fn proxy_only(serving: egress::Serving) -> std::io::Result<Self> {
        let asks = serving.asks.clone();
        Ok(Self {
            proxy: Proxied::Here(Box::new(egress::Proxy::serving(serving)?)),
            tmp: None,
            asks: asks.map(|asks| asks as std::sync::Arc<dyn Board>),
            ssh: None,
            routed: None,
            kept: None,
            serving: None,
        })
    }

    /// [`Self::proxy_only`], with an ssh route through its SOCKS port written in a folder of its
    /// own inside `within` ([`tunnel::routed_in`]), a folder no chat writes, removed as this
    /// ends (#1667). Where it cannot be written, ssh reaches nothing, as before, and it is
    /// logged.
    pub fn proxy_only_routed(serving: egress::Serving, within: &Path) -> std::io::Result<Self> {
        let mut confinement = Self::proxy_only(serving)?;
        match tunnel::routed_in(within, confinement.socks_port()) {
            Ok((dir, route)) => {
                confinement.ssh = Some(route);
                confinement.routed = Some(Folder::Own(dir));
            }
            Err(err) => tracing::warn!("purlis: a chat's ssh route was not written ({err})"),
        }
        Ok(confinement)
    }

    /// **A confinement the session host started and holds** (#1999), at `placed`, reached
    /// through `held`: the chat is named its ports and folders as it would be named one started
    /// here. Its two ports are purlis's own in this process too while it is held
    /// ([`localport`]), so no chat here is allowed either; refused where a chat here was given
    /// one of them.
    pub fn held_elsewhere(placed: Placed, held: Box<dyn HeldElsewhere>) -> std::io::Result<Self> {
        if !localport::hold_elsewhere(&[placed.http, placed.socks]) {
            return Err(std::io::Error::other(
                "the session host's proxy for this chat listens on a port a chat here was given",
            ));
        }
        Ok(Self::held_at(placed, held, true))
    }

    /// **A confinement a session host on another machine started and holds** (#2446): a
    /// runner's. Its proxy listens on that machine's ports, so none of them is judged against,
    /// or taken as, a port of this machine ([`localport`]); otherwise it is held as
    /// [`Self::held_elsewhere`] holds one.
    pub fn held_on_another_machine(placed: Placed, held: Box<dyn HeldElsewhere>) -> Self {
        Self::held_at(placed, held, false)
    }

    fn held_at(placed: Placed, held: Box<dyn HeldElsewhere>, on_this_machine: bool) -> Self {
        let asks = held.board();
        Self {
            proxy: Proxied::Elsewhere(Elsewhere {
                http: placed.http,
                socks: placed.socks,
                held,
                left_out: placed.left_out,
                host_name: placed.host_name,
                on_this_machine,
            }),
            tmp: placed.tmp.map(Folder::Theirs),
            asks,
            ssh: placed.ssh.map(tunnel::SshRoute::at),
            routed: placed.routed.map(Folder::Theirs),
            kept: None,
            serving: None,
        }
    }

    /// **Where this is** ([`Placed`]): what the session host says of one it started.
    pub fn placed(&self) -> Placed {
        Placed {
            http: self.proxy_port(),
            socks: self.socks_port(),
            tmp: self.tmp().map(Path::to_path_buf),
            routed: self.routed.as_ref().map(|dir| dir.path().to_path_buf()),
            ssh: self.ssh.as_ref().map(|route| route.dir().to_path_buf()),
            left_out: self.left_out().to_vec(),
            host_name: self.runner().map(str::to_owned),
        }
    }

    /// The name of the machine the host holding its proxy runs on, where that host left a
    /// service out ([`Self::left_out`], #2510).
    pub fn runner(&self) -> Option<&str> {
        match &self.proxy {
            Proxied::Here(_) => None,
            Proxied::Elsewhere(elsewhere) => elsewhere.host_name.as_deref(),
        }
    }

    /// **The services the chat was planned to reach that the host holding its proxy started it
    /// without** (#2510): those the person allowed for this chat alone on this machine, which a
    /// runner's own sandbox does not give. None for a proxy that runs here.
    pub fn left_out(&self) -> &[u16] {
        match &self.proxy {
            Proxied::Here(_) => &[],
            Proxied::Elsewhere(elsewhere) => &elsewhere.left_out,
        }
    }

    /// **What the window says of the services left out** ([`Self::left_out`]), for the chat's
    /// tab: what the runner did not give, then what comes of it, by whether its proxy asks
    /// live. `None` where none was.
    pub fn said_of_what_was_left_out(&self) -> Option<String> {
        let (last, rest) = self.left_out().split_last()?;
        let ports = match rest {
            [] => format!("the service on port {last}"),
            _ => format!(
                "the services on ports {} and {last}",
                rest.iter()
                    .map(u16::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        let runner = match self.runner() {
            Some(name) => format!("The runner {name}"),
            None => "The runner this chat runs on".to_owned(),
        };
        let it = if rest.is_empty() { "it" } else { "them" };
        let then = if self.asks.is_some() {
            format!("You are asked when the chat reaches for {it} there.")
        } else {
            format!("The chat cannot reach {it} there.")
        };
        Some(format!(
            "{runner} does not give this chat {ports} that you allowed for it alone. {then}"
        ))
    }

    /// What holds this one in another process calls it ([`HeldElsewhere::id`]); `None` for one
    /// that runs here.
    pub fn held_as(&self) -> Option<&str> {
        match &self.proxy {
            Proxied::Here(_) => None,
            Proxied::Elsewhere(elsewhere) => Some(elsewhere.held.id()),
        }
    }

    /// Its ssh route through its SOCKS port (#1667), where it was written.
    pub fn ssh_route(&self) -> Option<&tunnel::SshRoute> {
        self.ssh.as_ref()
    }

    /// The loopback port of its proxy's HTTP side.
    pub fn proxy_port(&self) -> u16 {
        match &self.proxy {
            Proxied::Here(proxy) => proxy.port(),
            Proxied::Elsewhere(elsewhere) => elsewhere.http,
        }
    }

    /// The loopback port of its proxy's SOCKS5 side.
    pub fn socks_port(&self) -> u16 {
        match &self.proxy {
            Proxied::Here(proxy) => proxy.socks_port(),
            Proxied::Elsewhere(elsewhere) => elsewhere.socks,
        }
    }

    /// Both its proxy's ports, the only ones its wrap lets the chat connect to: HTTP, then SOCKS5.
    pub fn proxy_ports(&self) -> [u16; 2] {
        [self.proxy_port(), self.socks_port()]
    }

    /// The URL its proxy is named by in the chat's environment.
    pub fn proxy_url(&self) -> String {
        egress::url_of(self.proxy_port())
    }

    /// Its proxy's live asks (#1666), which the window's answers reach the chat by.
    pub fn asks(&self) -> Option<&std::sync::Arc<dyn Board>> {
        self.asks.as_ref()
    }

    /// The chat's own temp directory, which a proxy-only one has none of.
    pub fn tmp(&self) -> Option<&Path> {
        self.tmp.as_ref().map(Folder::path)
    }

    /// **Opens a door for each port `widened` lets the chat listen on, to its servers in
    /// `folder`** (#2019): the folder purlis keeps for this chat's sockets, made `0700` where it
    /// is not there, and whatever was left where a socket goes taken away ([`cleared`]). A port
    /// something on this Mac listens on already gets no door ([`door::in_use`]): answered back,
    /// so the chat is started without serving there and told why. **Every port given for a
    /// listen grant alone that gets no door is withdrawn from the chat's proxy**, whatever kept
    /// it shut, so the grant never leaves it a plain connect there. Once per confinement; a
    /// second call opens nothing more.
    pub fn serve(&mut self, folder: &Path, widened: &Widened) -> std::io::Result<Vec<u16>> {
        if self.serving.is_some() || widened.listens.is_empty() {
            return Ok(Vec::new());
        }
        let opened = doors_to(folder, &widened.listens);
        let open: Vec<u16> = opened
            .as_ref()
            .map(|(doors, _)| doors.iter().map(door::Door::port).collect())
            .unwrap_or_default();
        for port in widened
            .listen_only
            .iter()
            .filter(|port| !open.contains(port))
        {
            if let Err(why) = self.withdraw_port(*port) {
                tracing::warn!("purlis: port {port} could not be withdrawn ({why})");
            }
        }
        let (doors, unserved) = opened?;
        self.serving = Some(Serving {
            folder: folder.to_path_buf(),
            doors,
        });
        Ok(unserved)
    }

    /// **Where this chat's servers listen, and the ports their doors are open on** (#2019):
    /// what its profile lets it serve on. None before [`Self::serve`], or where it serves on
    /// none.
    pub fn served(&self) -> Option<(&Path, Vec<u16>)> {
        let serving = self.serving.as_ref()?;
        let ports: Vec<u16> = serving.doors.iter().map(door::Door::port).collect();
        (!ports.is_empty()).then_some((serving.folder.as_path(), ports))
    }

    /// **`text` kept as `name` in this chat's own folder** (qw384b), for this chat alone and
    /// for as long as it runs: its temp directory, or else the folder its ssh route was
    /// written in, where no chat writes, or else a folder made for it. Readable by its owner
    /// alone. Where the file is.
    pub fn keep_own(&mut self, name: &str, text: &str) -> std::io::Result<PathBuf> {
        let dir = match (&self.tmp, &self.routed, &self.kept) {
            (Some(own), _, _) | (None, Some(own), _) => own.path().to_path_buf(),
            (None, None, Some(own)) => own.path().to_path_buf(),
            (None, None, None) => {
                let made = tempfile::Builder::new().prefix("purlis-chat-").tempdir()?;
                let path = made.path().to_path_buf();
                self.kept = Some(made);
                path
            }
        };
        let path = dir.join(name);
        // Made new each time, never through whatever stands at the path: a file kept before is
        // taken away first (a link itself, never what it points at), and the open neither
        // follows a link nor takes a file that is there already (#2097 review).
        match std::fs::remove_file(&path) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => return Err(err),
            _ => {}
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        std::io::Write::write_all(&mut options.open(&path)?, text.as_bytes())?;
        Ok(path)
    }

    /// **`port` of this machine is carried by its proxy no longer** (qw384b): its grant was
    /// removed while the chat runs.
    /// One held elsewhere whose holder could not be told says why ([`HeldElsewhere::withdraw_port`]).
    pub fn withdraw_port(&self, port: u16) -> Result<(), String> {
        match &self.proxy {
            Proxied::Here(proxy) => {
                proxy.withdraw_port(port);
                Ok(())
            }
            Proxied::Elsewhere(elsewhere) => elsewhere.held.withdraw_port(port),
        }
    }

    /// **`port` of this machine, withdrawn, is carried by its proxy again as it was given**
    /// (#2625): what a runner does for a chat whose port it withdrew because no window was left,
    /// once its window takes it back. Only a proxy of this process's own gives one back.
    pub fn give_back_port(&self, port: u16) -> Result<(), String> {
        match &self.proxy {
            Proxied::Here(proxy) => {
                proxy.give_back_port(port);
                Ok(())
            }
            Proxied::Elsewhere(_) => Err(format!(
                "port {port} is given back only by the session host that runs the chat's proxy"
            )),
        }
    }
}

/// Each harness's own form of the policy, one variant per compiler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Form {
    ClaudeCode(claude::Settings),
    Codex(codex::Wrap),
    Opencode(opencode::Wrap),
}

impl Form {
    /// Every path a wrap charter applies denies, with what its compiler added to the neutral
    /// list ([`Compiled::denied`]); `None` for a harness whose own sandbox holds the policy.
    pub fn denied(&self) -> Option<&[Denial]> {
        match self {
            Self::Codex(wrap) => Some(&wrap.denied),
            Self::Opencode(wrap) => Some(&wrap.denied),
            Self::ClaudeCode(_) => None,
        }
    }

    /// **No workspace's folder of `planes` is made, moved or removed by the chat** (#2046),
    /// where the harness's sandbox can hold a folder's entry alone (Seatbelt). Claude Code's
    /// holds what is below a name too, so it holds each workspace's manifest by its path and
    /// name ([`workspace_manifests_held`]) and nothing more.
    pub fn hold_workspace_folders(&mut self, planes: Vec<PathBuf>) {
        match self {
            Self::Codex(wrap) => wrap.planes = planes,
            Self::Opencode(wrap) => wrap.planes = planes,
            Self::ClaudeCode(_) => {}
        }
    }
}

/// `compiled`, by `compile`, for a chat in the plane at `root` on `machine`, with every path
/// it denies, or why it does not start: [`covering`]'s refusal among the reasons, held to the
/// list a wrap applies as well as the neutral one.
fn compile_checked(
    compile: Compiler,
    compiled: &Compiled,
    root: &Path,
    machine: &Machine,
) -> Result<(Form, Vec<Denial>), NotStarted> {
    if let Some(unread) = &compiled.denied.unread {
        return Err(NotStarted::Unread(unread.clone()));
    }
    // Before any compiler is asked: no harness's sandbox is made from a list that may be
    // short of what purlis keeps from a chat.
    if let Some(why) = &compiled.denied.untold {
        return Err(NotStarted::KeptUntold(why.clone()));
    }
    let ground = ground(root, machine.home.as_deref());
    if let Some(refused) = covering(&compiled.denied.paths, &ground) {
        return Err(refused.placed_in(root));
    }
    let mut form = compile(compiled).map_err(NotStarted::Uncompilable)?;
    form.hold_workspace_folders(workspace_folder_planes(root, compiled.level));
    // Held again only where the compiler added denials of its own (#2570): a harness whose own
    // sandbox holds the policy (Claude Code) denies exactly the neutral list, which the check
    // above has just held to this ground.
    let Some(added) = form.denied() else {
        return Ok((form, compiled.denied.paths.clone()));
    };
    let denied = added.to_vec();
    if let Some(refused) = covering(&denied, &ground) {
        return Err(refused.placed_in(root));
    }
    Ok((form, denied))
}

/// **The plane roots whose workspace folders a chat of the plane at `root` never makes, moves or
/// removes** (#2046): its own, and at the floor, where it writes every folder, every plane this
/// machine remembers (`machine.json`). A project this machine does not know keeps its
/// workspaces' manifests held by name alone ([`seatbelt::workspace_manifests_regex`]).
fn workspace_folder_planes(root: &Path, level: Level) -> Vec<PathBuf> {
    let mut planes = vec![root.to_path_buf()];
    // A store outside a fenced build's fence is no store of its own: nothing more is known.
    let store = crate::machine::config_root_unheld().filter(|config_root| {
        !cfg!(feature = "fenced") || crate::fence::inside(config_root, &crate::fence::fence())
    });
    if level == Level::Floor
        && let Some(config_root) = store
    {
        let store = crate::machine::read(&config_root).store;
        let known = store
            .recents
            .iter()
            .map(|recent| recent.plane.clone())
            .chain(
                store
                    .windows
                    .iter()
                    .flat_map(|window| window.planes.clone()),
            );
        for plane in known {
            if !planes.contains(&plane) {
                planes.push(plane);
            }
        }
    }
    planes
}

/// A harness's compiler.
pub type Compiler = fn(&Compiled) -> Result<Form, Uncompilable>;

/// The compiler for `harness`, or none where charter has not written one yet: its adapter's
/// ([`crate::harness::HarnessAdapter::sandbox_compiler`]). A harness gains a sandbox by its
/// adapter gaining a compiler and a [`Form`] variant.
pub fn compiler(harness: Harness) -> Option<Compiler> {
    harness.adapter().sandbox_compiler()
}

/// Why no chat on `harness` starts sandboxed on `os`, whatever the project: charter has no
/// compiler for it, holds its sandbox back, or cannot wrap it on this system. `None` where one
/// can, as far as the harness and the system decide; a project can still refuse it for its own
/// reasons, such as a keyring vault. What a harness's card says about the sandbox.
pub fn never_on(harness: Harness, os: Os) -> Option<String> {
    never_with(compiler(harness), harness.adapter().sandbox_held_back(), os)
}

/// [`never_on`] for a harness with `compile` as its compiler, held back for the issue
/// `held_back` names. The sentence is the window's, so it never carries the issue (#1422).
fn never_with(compile: Option<Compiler>, held_back: Option<u32>, os: Os) -> Option<String> {
    let Some(compile) = compile else {
        return Some("purlis has no sandbox compiler for it yet".to_owned());
    };
    if held_back.is_some() {
        return Some("purlis cannot keep its chats inside the sandbox yet".to_owned());
    }
    let nothing = Compiled {
        level: Level::Full,
        held: Vec::new(),
        denied: Denied::default(),
        hosts: Vec::new(),
        reach: reach::Reach::default(),
        writable: Vec::new(),
        os,
        homes: Homes::default(),
        widened: Widened::default(),
    };
    match compile(&nothing) {
        Err(Uncompilable {
            unheld: Unheld::Wrap(_),
            ..
        }) => Some("purlis can wrap it on macOS and Linux only, so far".to_owned()),
        _ => None,
    }
}

/// **The lead of a refusal to start a sandboxed chat** where the project runs its chats
/// sandboxed: every such refusal opens with it, and [`under_policy`] says the policy instead
/// where it is the policy that requires the sandbox.
pub const LEAD: &str = "this project runs every chat sandboxed";

/// **The lead where an administrator's policy requires the sandbox** (#1431, D-1423-1): the
/// project's own setting is then not why the chat had to run sandboxed.
pub const POLICY_LEAD: &str = "policy requires the sandbox for every chat on this machine";

/// `why` with its [`LEAD`] said as [`POLICY_LEAD`] where `locks` require the sandbox; any other
/// sentence as it is.
fn led(locks: &policy::Locks, why: String) -> String {
    match why.strip_prefix(LEAD) {
        Some(rest) if locks.forbids_opt_out() => format!("{POLICY_LEAD}{rest}"),
        _ => why,
    }
}

/// **A refusal to start a sandboxed chat, `why`, as the person reads it under `locks`** (#1423,
/// #1431): where an administrator's policy requires the sandbox, it leads with the policy rather
/// than the project ([`POLICY_LEAD`]) and ends with the policy and who set it. Where it does not,
/// `why` as it is.
pub fn under_policy(locks: &policy::Locks, why: String) -> String {
    match locks.opt_out_refused() {
        Some(policy) => format!("{} {policy}", led(locks, why)),
        None => why,
    }
}

/// Why a sandboxed chat is not started when `path`, of its project's package caches, is a link
/// purlis could not take out, as found `when` it made the folders.
pub fn caches_linked(path: &Path, when: &str) -> String {
    format!(
        "this project runs every chat sandboxed, and {} of the project's package caches is a \
         link purlis could not take out ({when} making them), so nothing was started. Remove \
         that link and start the chat again.",
        path.display()
    )
}

/// Why a sandboxed chat is not started in the folder it was given (ruling of 2026-10-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderRefusal {
    /// The chat has no folder of its own.
    Missing,
    /// Its folder, or one between it and the project, is a link or not a real folder.
    Linked,
    /// Its folder is not inside the project.
    Outside,
}

impl FolderRefusal {
    /// What happened and what to change, with no full stop: [`Self::said`] ends it.
    fn happened(self) -> &'static str {
        match self {
            Self::Missing => {
                "this project runs every chat sandboxed, and the chat has no folder of its own \
                 to be confined to, so nothing was started. Forget this chat and start a new one \
                 in a folder of the project"
            }
            Self::Linked => {
                "this project runs every chat sandboxed, and the chat's folder, or a folder \
                 between it and the project, is a link or not a real folder, so nothing was \
                 started. Move the chat's folder into the project as a real folder, or forget \
                 this chat and start a new one"
            }
            Self::Outside => {
                "this project runs every chat sandboxed, and the chat's folder is not inside \
                 the project, so nothing was started. Move the chat's folder into the project, \
                 or forget this chat and start a new one"
            }
        }
    }

    /// **The refusal as the person reads it under `locks`** (#1423): it ends with the person's
    /// own way out, "Start without the sandbox", or, where an administrator's policy forbids
    /// that, with the policy and who set it instead. Both are built from the same first part,
    /// so no rewording can leave the opt-out named under a lock.
    pub fn said(self, locks: &policy::Locks) -> String {
        let happened = self.happened();
        if locks.forbids_opt_out() {
            under_policy(locks, format!("{happened}."))
        } else {
            format!("{happened}; Start without the sandbox is yours to pick when you start it.")
        }
    }
}

/// Why a chat in the folder `cwd` of the plane at `root` may not start sandboxed, or `None`
/// (ruling of 2026-10-03): every folder from the plane's own, as the kernel names it, down to
/// the chat's must be a real directory and not a link, so a folder another chat swapped for a
/// link never takes the rules of the folder it replaced to somewhere else.
pub fn folder_refusal(root: &Path, cwd: &Path) -> Option<FolderRefusal> {
    let Ok(real_root) = root.canonicalize() else {
        return Some(FolderRefusal::Linked);
    };
    // Where the folder meets the plane: the outermost of its ancestors that is the plane as the
    // kernel names it, so a link above the plane (`/tmp`, a linked parent) is allowed on either
    // side, and one inside it is walked below and refused.
    let Some(meets) = cwd
        .ancestors()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .find(|dir| dir.canonicalize().is_ok_and(|real| real == real_root))
    else {
        return Some(FolderRefusal::Outside);
    };
    let Ok(below) = cwd.strip_prefix(meets) else {
        return Some(FolderRefusal::Outside);
    };
    let below = below.to_path_buf();
    let mut at = real_root.clone();
    let check = |at: &Path| match std::fs::symlink_metadata(at) {
        Ok(meta) => meta.file_type().is_dir() && !meta.file_type().is_symlink(),
        Err(_) => false,
    };
    if !check(&at) {
        return Some(FolderRefusal::Linked);
    }
    for part in below.components() {
        match part {
            std::path::Component::Normal(name) => at.push(name),
            std::path::Component::CurDir => continue,
            _ => return Some(FolderRefusal::Linked),
        }
        if !check(&at) {
            return Some(FolderRefusal::Linked);
        }
    }
    // The walk and the kernel agree on where the folder is.
    match cwd.canonicalize() {
        Ok(real) if real == at => None,
        _ => Some(FolderRefusal::Linked),
    }
}

/// The ground a chat in the plane at `root` stands on, whatever its folder: `/` and every
/// folder down to the plane's own, and the home directory.
fn ground(root: &Path, home: Option<&Path>) -> Vec<PathBuf> {
    root.ancestors()
        .map(Path::to_path_buf)
        .chain(home.map(Path::to_path_buf))
        .collect()
}

/// The refusal for the first of `denied` that covers a folder of `ground` — is it, or is above
/// it — by any name either has ([`spellings`]), compared case-folded where the volume the folder
/// is on folds case ([`folds_case`]); `None` where none does.
///
/// **Fail closed** (#1327). A rule that covers the ground a chat stands on (`/`, the home
/// directory, the project, or the chat's own folder or one above it) leaves it read-only
/// everywhere it works. That is never what a class means, so the chat is refused, naming what
/// named the path, rather than started read-only or started with the rule left out.
pub fn covering(denied: &[Denial], ground: &[PathBuf]) -> Option<NotStarted> {
    let ground: Vec<(PathBuf, bool)> = ground
        .iter()
        .flat_map(|folder| {
            let folds = folds_case(folder);
            spellings(folder).into_iter().map(move |name| (name, folds))
        })
        .collect();
    // An entry held alone holds nothing below it (#2752), so no ground is under it.
    denied
        .iter()
        .filter(|denial| denial.reach == Reach::AndBelow)
        .find_map(|denial| {
            let covers = spellings(&denial.path).iter().any(|name| {
                ground.iter().any(|(folder, folds)| {
                    folder.starts_with(name) || (*folds && folded(folder).starts_with(folded(name)))
                })
            });
            covers.then(|| NotStarted::CoversItsGround {
                path: denial.path.clone(),
                class: denial.class,
                named: denial.named.clone(),
                within: None,
            })
        })
}

/// Each spelling of `path` a sandbox or a tool may compare: as written, with `..` taken off,
/// as the kernel names either, and each of those on the data volume's other side
/// ([`firmlink_twin`]).
fn spellings(path: &Path) -> Vec<PathBuf> {
    let lexical = planted::lexical(path);
    let mut out = vec![path.to_path_buf(), real(path), real(&lexical), lexical];
    let twins: Vec<PathBuf> = out.iter().filter_map(|it| firmlink_twin(it)).collect();
    out.extend(twins);
    out
}

/// `path` with every part lower-cased: what a volume that folds case compares.
fn folded(path: &Path) -> PathBuf {
    PathBuf::from(path.to_string_lossy().to_lowercase())
}

/// Whether the volume `path` is on folds case (#1356): whether the deepest folder of it that
/// exists, with a letter in its name, is found again under that name with its case swapped.
/// A path that does not exist yet and differs from one that does only in case is that folder
/// there, which resolving it ([`real`]) cannot tell.
///
/// Asked of `path` as the kernel names it, and of the swapped name itself, not what it links
/// to (#1418): a link on the way could put the folder on another volume, and a link beside it
/// whose name differs only in case would answer as the folder.
fn folds_case(path: &Path) -> bool {
    let path = real(path);
    path.ancestors()
        .filter(|folder| folder.exists())
        .find_map(|folder| {
            let name = folder.file_name()?.to_str()?;
            let swapped: String = name
                .chars()
                .flat_map(|c| {
                    if c.is_lowercase() {
                        c.to_uppercase().collect::<Vec<_>>()
                    } else {
                        c.to_lowercase().collect()
                    }
                })
                .collect();
            (swapped != name).then(|| (folder, folder.with_file_name(swapped)))
        })
        .is_some_and(|(folder, swapped)| {
            let id = crate::pypath::file_identity(folder);
            id.is_some() && id == entry_identity(&swapped)
        })
}

/// `(st_dev, st_ino)` of the entry `path` itself, a link not followed.
fn entry_identity(path: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some((meta.dev(), meta.ino()))
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        None
    }
}

/// Where macOS keeps what its firmlinks reach: `/Users` is also `/System/Volumes/Data/Users`.
const DATA_VOLUME: &str = "/System/Volumes/Data";

/// The firmlinks macOS 26.2 lists in `/usr/share/firmlinks`, for a machine whose list cannot
/// be read.
const KNOWN_FIRMLINKS: [&str; 18] = [
    "/AppleInternal",
    "/Applications",
    "/Library",
    "/System/Library/Caches",
    "/System/Library/Assets",
    "/System/Library/PreinstalledAssets",
    "/System/Library/AssetsV2",
    "/System/Library/PreinstalledAssetsV2",
    "/System/Library/CoreServices/CoreTypes.bundle/Contents/Library",
    "/System/Library/Speech",
    "/Users",
    "/Volumes",
    "/cores",
    "/opt",
    "/private",
    "/usr/local",
    "/usr/libexec/cups",
    "/usr/share/snmp",
];

/// The folders this machine reaches by a firmlink: none but on macOS, where they are those
/// `/usr/share/firmlinks` lists and those [`KNOWN_FIRMLINKS`] holds, together.
///
/// **Fail closed** (#1356). A list that cannot be read, or reads empty, is logged and leaves
/// the known ones, never none: without them a rule or a ground loses its other name.
fn firmlinks() -> &'static [PathBuf] {
    static FIRMLINKS: std::sync::OnceLock<Vec<PathBuf>> = std::sync::OnceLock::new();
    FIRMLINKS.get_or_init(|| {
        if !cfg!(target_os = "macos") {
            return Vec::new();
        }
        let text = std::fs::read_to_string("/usr/share/firmlinks").ok();
        if text.as_deref().is_none_or(|text| text.trim().is_empty()) {
            tracing::warn!(
                "the system's firmlink list could not be read; the sandbox uses the one purlis knows"
            );
        }
        firmlinks_from(text.as_deref())
    })
}

/// The firmlinks `text`, a `/usr/share/firmlinks`, lists, with [`KNOWN_FIRMLINKS`]; the known
/// ones alone where there is no text.
pub(crate) fn firmlinks_from(text: Option<&str>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = KNOWN_FIRMLINKS.iter().map(PathBuf::from).collect();
    let listed = text
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split('\t').next())
        .map(PathBuf::from)
        .filter(|it| it.is_absolute() && it.parent().is_some());
    for link in listed {
        if !out.contains(&link) {
            out.push(link);
        }
    }
    out
}

/// `path` as the kernel names it ([`real`]), and under its other name across a firmlink
/// ([`firmlink_twin`]) where it has one: each a rule written for the kernel is held to.
///
/// The name off the data volume comes first, so either spelling gives one order.
pub(crate) fn kernel_names(path: &Path) -> Vec<PathBuf> {
    both_firmlink_names(real(path))
}

/// `path` itself as the kernel names it, its last part never followed: its folder as the kernel
/// names it ([`real`]) with its own name, and under its other name across a firmlink
/// ([`firmlink_twin`]) where it has one. A rule on a link held to the link, never to what it
/// points at (#2617).
pub(crate) fn kernel_names_itself(path: &Path) -> Vec<PathBuf> {
    let itself = match (path.parent(), path.file_name()) {
        (Some(folder), Some(name)) => real(folder).join(name),
        _ => real(path),
    };
    both_firmlink_names(itself)
}

/// `path` as it is spelled, and under its other name across a firmlink ([`firmlink_twin`])
/// where it has one, the name off the data volume first.
pub(crate) fn both_firmlink_names(path: PathBuf) -> Vec<PathBuf> {
    match firmlink_twin(&path) {
        Some(twin) if path.starts_with(DATA_VOLUME) => vec![twin, path],
        Some(twin) => vec![path, twin],
        None => vec![path],
    }
}

/// `path`'s other name across a firmlink (#1356): `/Users/x` for `/System/Volumes/Data/Users/x`
/// and the other way, or `None` where no firmlink reaches it. Resolving either name
/// ([`real`]) leaves it as it is spelled, so a rule or a ground held to one would miss the
/// other.
pub(crate) fn firmlink_twin(path: &Path) -> Option<PathBuf> {
    let data = Path::new(DATA_VOLUME);
    let linked = |it: &Path| firmlinks().iter().any(|link| it.starts_with(link));
    match path.strip_prefix(data) {
        Ok(below) => {
            let plain = Path::new("/").join(below);
            linked(&plain).then_some(plain)
        }
        Err(_) => linked(path)
            .then(|| path.strip_prefix("/").ok().map(|below| data.join(below)))
            .flatten(),
    }
}

/// Where a config that named a refused path sits in the project: its repo, as a path from
/// the project, and its workspace (#1356).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Within {
    pub repo: Option<PathBuf>,
    pub workspace: Option<String>,
}

impl Within {
    /// Where `file` sits in the project at `root`: the nearest folder above it, below the
    /// project, that holds a `.git`, and its workspace; `None` where it is in neither.
    fn of(file: &Path, root: &Path) -> Option<Self> {
        let dir = file.parent()?;
        let repo = dir
            .ancestors()
            .take_while(|folder| *folder != root && folder.starts_with(root))
            .find(|folder| folder.join(".git").exists())
            .and_then(|folder| folder.strip_prefix(root).ok())
            .map(Path::to_path_buf);
        let workspace = crate::active::workspace_of_tree(root, dir);
        (repo.is_some() || workspace.is_some()).then_some(Self { repo, workspace })
    }
}

impl fmt::Display for Within {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.repo, &self.workspace) {
            (Some(repo), Some(workspace)) => {
                write!(f, "in the repo {} of workspace {workspace}", repo.display())
            }
            (Some(repo), None) => write!(f, "in the repo {}", repo.display()),
            (None, Some(workspace)) => write!(f, "in workspace {workspace}"),
            (None, None) => Ok(()),
        }
    }
}

/// Whether a plane manifest above `start`, under either name (`charter.toml` or
/// `purlis.toml`), is there and is not a regular file — a link, a FIFO, a socket, a device or a
/// directory — which the plane's own walk does not take for a plane at all. A sandboxed start
/// refuses it ([`NotStarted::PlaneUnreadable`]) rather than reading "no plane" and starting the
/// chat unsandboxed.
pub fn marker_unreadable(start: &Path) -> bool {
    let here = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    here.ancestors().any(|dir| {
        crate::names::PLANE_MANIFEST.spellings().any(|name| {
            std::fs::symlink_metadata(dir.join(name)).is_ok_and(|meta| !meta.file_type().is_file())
        })
    })
}

/// The indefinite article a sentence puts before `word`.
fn article(word: &str) -> &'static str {
    if word.starts_with(['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    }
}

/// The harnesses charter can sandbox on this machine other than `but`, as a sentence names
/// them: one a chat could be started on instead. A harness held back or one this system cannot
/// wrap is never offered.
fn sandboxed_harnesses_but(but: Option<Harness>) -> String {
    let titles: Vec<&str> = Harness::ALL
        .into_iter()
        .filter(|harness| never_on(*harness, Os::this()).is_none() && Some(*harness) != but)
        .map(Harness::title)
        .collect();
    match titles.split_last() {
        Some((last, [])) => (*last).to_owned(),
        Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
        None => String::new(),
    }
}

/// Why a chat did not start in a sandboxed plane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotStarted {
    /// Charter has no compiler for the harness yet.
    NoCompiler(Harness),
    /// This machine cannot apply a sandbox.
    NoBackend(backend::Missing),
    /// The harness's compiler cannot hold a class here.
    Uncompilable(Uncompilable),
    /// The profile's program, as written or as the kernel names it, lives where the chat can
    /// write, so a sandboxed chat could have changed it (ruling V87g).
    ProgramWritable(PathBuf),
    /// purlis's own program, which wraps a chat on Linux, was replaced on disk while it ran
    /// (#1040): the running one is no longer the one a new chat would start.
    WrapReplaced,
    /// What purlis arms a chat with, its own program or its plugin, as written or as the kernel
    /// names it, lies where the chat can write ([`Applied::hold_kit`]).
    KitWritable(PathBuf),
    /// The profile's program is a relative path, which would be found against a folder the
    /// chat can write.
    ProgramRelative,
    /// A word of the profile's command, or the value of a `--flag=value` word, names a file
    /// where the chat can write: a program outside handed it would run what a chat wrote.
    WordWritable(String),
    /// A word of the profile's command is longer than [`program::WORD_MAX`], which the check
    /// does not read (D-88k).
    WordTooLong,
    /// The profile's program does not answer as the harness its sandbox was compiled for, so
    /// the sandbox may bind nothing (ruling V87g).
    NotTheHarness(Harness, String, Option<String>),
    /// The profile's program did not answer `--version` in time ([`program::PATIENCE`]): a
    /// first run of a program the system has not seen can be slow, so the person is asked to
    /// start the chat again rather than told it is not the harness.
    ProbeTimedOut(Harness),
    /// Charter has a compiler for the harness, and holds it back: its own sandbox cannot be
    /// kept to what the compiler says while it runs, until the issue named lands.
    HeldBack(Harness, u32),
    /// The plane's `charter.toml` cannot be read, so whether it turns the sandbox on is
    /// unknown, and the chat is not started rather than started unsandboxed.
    PlaneUnreadable,
    /// The open project's own manifest is not there, so whether it runs chats sandboxed is
    /// unknown, and the chat is not started rather than started unsandboxed (D-1410e).
    PlaneMissing,
    /// A path the sandbox would deny covers the ground the chat stands on ([`covering`]), so
    /// it would start unable to write where it works (#1327).
    CoversItsGround {
        path: PathBuf,
        class: Class,
        named: Option<Named>,
        /// Where the config that named it sits, in a repo or a workspace.
        within: Option<Within>,
    },
    /// A config's commands change folder or name scripts more often than purlis follows
    /// ([`planted::MOST_MOVES`], [`planted::MOST_NAMED`]), so what they run is not known.
    Unread(Named),
    /// What purlis keeps from a chat could not be told whole ([`Denied::untold`]), and why:
    /// a sandbox compiled from a list that may be short would keep less than purlis means to,
    /// so none is compiled and the chat is not started.
    KeptUntold(Untold),
    /// A person asked to start the chat without the sandbox, and an administrator's policy
    /// forbids it (#1343): why, naming the policy and who set it.
    OptOutLocked(String),
    /// An administrator's policy requires the sandbox, and purlis has no sandbox backend on
    /// this system (D-1423-1): the chat is not started, and never started unconfined.
    /// [`Self::said`] names the policy and who set it.
    RequiredWithoutBackend(Os),
}

impl NotStarted {
    /// This refusal with where the config that named its path sits in the project at `root`
    /// ([`Within`]), for one [`covering`] gave.
    fn placed_in(self, root: &Path) -> Self {
        match self {
            Self::CoversItsGround {
                path,
                class,
                named,
                within: None,
            } => {
                let within = named
                    .as_ref()
                    .and_then(|named| Within::of(&named.file, root));
                Self::CoversItsGround {
                    path,
                    class,
                    named,
                    within,
                }
            }
            other => other,
        }
    }
}

impl fmt::Display for NotStarted {
    /// The refusal where a person may still start the chat without the sandbox. Where an
    /// administrator's policy forbids that, say it with [`NotStarted::said`] instead.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.sentence(true))
    }
}

impl NotStarted {
    /// **The refusal as the person reads it under `locks`** (#1423): where an administrator's
    /// policy forbids starting a chat without the sandbox, no sentence sends the person to
    /// that opt-out, and each ends with the policy and who set it instead
    /// ([`policy::Locks::opt_out_refused`]). Where it does not, the refusal as it always read.
    pub fn said(&self, locks: &policy::Locks) -> String {
        match (self, locks.opt_out_refused()) {
            // It names the policy itself.
            (Self::OptOutLocked(_), _) | (_, None) => self.to_string(),
            (_, Some(_)) => under_policy(locks, self.sentence(false)),
        }
    }

    /// **The refusal beside a line that already names the policy** ([`Ahead::Refused`], which
    /// the new-chat picker draws over what locks the opt-out): [`Self::said`] without the
    /// policy's own sentence.
    fn said_beside(&self, locks: &policy::Locks) -> String {
        led(locks, self.sentence(!locks.forbids_opt_out()))
    }

    /// The refusal in one or more sentences. `opt_out` is whether a person may start this chat
    /// without the sandbox: where they may not, no way out names it.
    fn sentence(&self, opt_out: bool) -> String {
        let lead = LEAD;
        match self {
            Self::NoCompiler(harness) => format!(
                "{lead}, and purlis cannot sandbox {} {} chat yet, so it was not started. \
                 Start this chat on a {} profile.",
                article(harness.title()),
                harness.title(),
                sandboxed_harnesses_but(Some(*harness))
            ),
            Self::WrapReplaced => format!(
                "{lead}, and purlis's own program was replaced while it ran, so it cannot \
                 start this chat sandboxed. Restart purlis, then start the chat again."
            ),
            Self::ProgramWritable(path) => format!(
                "{lead}, and the program lives where this chat can write: {}, so it was not \
                 started sandboxed. Keep the program outside the project and outside what a chat \
                 may write.",
                path.display()
            ),
            Self::KitWritable(path) => format!(
                "{lead}, and purlis's own files lie where this chat can write: {}, so it \
                 was not started sandboxed. Install purlis outside the project and outside \
                 what a chat may write, then start the chat again.",
                path.display()
            ),
            Self::ProgramRelative => format!(
                "{lead}, and this profile's program is a relative path, which would be found in \
                 a folder the chat can write, so it was not started sandboxed. Name the program \
                 by its full path."
            ),
            Self::WordTooLong => format!(
                "{lead}, and a word of this profile's command is longer than 4 KiB, which purlis \
                 does not check, so it was not started sandboxed. Keep what it says in a file \
                 outside the project and name that file instead."
            ),
            Self::WordWritable(word) => format!(
                "{lead}, and this profile's command names {word}, which lies where this chat \
                 can write, so it was not started sandboxed. Keep every file the command names \
                 outside the project and outside what a chat may write."
            ),
            Self::ProbeTimedOut(harness) => format!(
                "{lead}, and this profile's program did not answer whether it is {} within {} \
                 seconds, asked twice, so it was not started. A first run of a program can be \
                 slow: start the chat again.",
                harness.title(),
                (program::PATIENCE * 3).as_secs()
            ),
            Self::NotTheHarness(harness, program, said) => format!(
                "{lead}, and this profile's program {} does not answer as {}, whose sandbox it \
                 was given{}, so it was not started sandboxed. Point the profile at {} itself, \
                 or at a wrapper that hands `--version` on to it.",
                crate::shown::short(program),
                harness.title(),
                said.as_deref().map_or_else(
                    || ": it gave no answer purlis could read".to_owned(),
                    |said| format!(": asked `--version`, it answered \"{said}\"")
                ),
                harness.title()
            ),
            // The issue it is held back for is the adapter's to name, not the window's (#1422).
            Self::HeldBack(harness, _) => format!(
                "purlis cannot keep {} {} chat inside its sandbox yet, so in this \
                 project {}",
                article(harness.title()),
                harness.title(),
                if opt_out {
                    "a new one starts only without the sandbox, from the new-chat picker."
                } else {
                    "none starts."
                }
            ),
            Self::PlaneUnreadable => format!(
                "{FILE} in this project cannot be read as TOML, so purlis cannot tell whether it \
                 runs chats sandboxed, and nothing was started. Fix {FILE} and start the chat \
                 again."
            ),
            Self::PlaneMissing => format!(
                "This project's {FILE} is missing, so purlis cannot tell whether it runs chats \
                 sandboxed, and nothing was started. Restore {FILE}, or reopen the project."
            ),
            Self::CoversItsGround {
                path,
                named: Some(named),
                within,
                ..
            } => format!(
                "{lead}, and `{}` in {}{} reads as a script purlis keeps this chat from \
                 changing, which would leave it unable to write {}, so nothing was started. \
                 Change that word in {}{}",
                named.word,
                named.file.display(),
                within
                    .as_ref()
                    .map_or_else(String::new, |within| format!(" ({within})")),
                path.display(),
                named.file.display(),
                if opt_out {
                    ", or start this chat without the sandbox from the new-chat picker."
                } else {
                    "."
                }
            ),
            Self::CoversItsGround {
                path,
                class,
                named: None,
                ..
            } => format!(
                "{lead}, and its {} rules would keep the chat from writing {}, so nothing was \
                 started.{}",
                class.word(),
                path.display(),
                if opt_out {
                    " Start this chat without the sandbox from the new-chat picker."
                } else {
                    ""
                }
            ),
            Self::Unread(named) => format!(
                "{lead}, and a command in {} changes folder or names scripts more often than \
                 purlis follows, from `{}` on, so purlis cannot tell what it runs, and nothing \
                 was started. Move that command into a script of its own{}",
                named.file.display(),
                named.word,
                if opt_out {
                    ", or start this chat without the sandbox from the new-chat picker."
                } else {
                    "."
                }
            ),
            Self::KeptUntold(untold) => format!(
                "{lead}, and {untold}, so purlis cannot tell what to keep from this chat, and \
                 nothing was started. {} and start the chat again.",
                match untold.registry {
                    Some(_) => {
                        "Doctor says what is wrong: put that file right, or remove the vault \
                         in its view,"
                    }
                    None => "Doctor says which file: leave it one name,",
                }
            ),
            Self::OptOutLocked(why) => format!("{why} Nothing was started."),
            // The Windows backend is M46, #565.
            Self::RequiredWithoutBackend(os) => format!(
                "purlis has no sandbox backend on {} yet, and policy requires the sandbox for \
                 every chat on this machine, so nothing was started.",
                match os {
                    Os::Windows => "Windows",
                    Os::MacOs | Os::Linux | Os::Other => "this system",
                }
            ),
            Self::NoBackend(missing @ backend::Missing::Landlock) => format!(
                "{lead}, and this machine cannot apply the sandbox: {missing}. Nothing was \
                 started. {}",
                backend::landlock_way_on(opt_out)
            ),
            Self::NoBackend(missing) => format!(
                "{lead}, and this machine cannot apply the sandbox: {missing}. Nothing was \
                 started."
            ),
            Self::Uncompilable(it) => match it.unheld {
                // The window's sentence names no issue (#1422).
                Unheld::Wrap(os) => format!(
                    "{lead}, and purlis runs {} inside a sandbox of its own, which it can apply \
                     on macOS and Linux but not yet on {}, so it was not started. Start this \
                     chat on a {} profile.",
                    it.harness.title(),
                    match os {
                        Os::Linux => "Linux",
                        Os::Windows => "Windows",
                        Os::MacOs | Os::Other => "this system",
                    },
                    sandboxed_harnesses_but(Some(it.harness))
                ),
                // Ruling V90c: never a dead end. The new-chat picker offers the opt-out beside
                // it; a resumed or relaunched chat has no opt-out, so moving the secrets is its
                // way on.
                Unheld::Service(Service::CredentialStore, os) => format!(
                    "{lead}, and {} purlis cannot keep {} {} \
                     chat away from the system keyring, where this project's keyring vaults \
                     keep their secrets, so nothing was started. {} a plain-file or 1Password \
                     vault, which the sandbox can keep from a chat.{}",
                    match os {
                        Os::Linux => "on Linux",
                        Os::MacOs | Os::Windows | Os::Other => "on this system",
                    },
                    article(it.harness.title()),
                    it.harness.title(),
                    if opt_out {
                        "Start this chat without the sandbox from the new-chat picker, or move \
                         those secrets to"
                    } else {
                        "Move those secrets to"
                    },
                    if opt_out {
                        " For a resumed or relaunched chat, moving them is the way on."
                    } else {
                        ""
                    }
                ),
            },
        }
    }
}

/// **Whether writing `after` over `before` changes a sandbox key** — the rule every brokered
/// write keeps (ADR 0067 §1 and §2 as amended; #1333): a write a chat asks `purlisd` to make is
/// refused when it would. `before` is the file as it stands (`None`: not there). Any change to
/// `[sandbox]` counts, in either settings file: its mode, its presets, its hosts. A text that is
/// not TOML counts too, since what it says about the sandbox is unknown — and so does an absent
/// file made where none was, if it holds `[sandbox]`.
pub fn changes_a_sandbox_key(before: Option<&str>, after: &str) -> bool {
    let table = |text: &str| -> Result<Option<toml::Value>, ()> {
        let top = text.parse::<toml::Table>().map_err(|_| ())?;
        Ok(top.get(TABLE).cloned())
    };
    let was = match before.map(table) {
        None => Ok(None),
        Some(was) => was,
    };
    match (was, table(after)) {
        (Ok(was), Ok(now)) => was != now,
        _ => true,
    }
}

/// What this machine's `charter.local.toml` says in `[sandbox]` that purlis does not take: any
/// key but `hosts`, and each entry of it that is not a host, each with its key (#1292).
fn local_refusals(text: &str) -> Vec<crate::settings::Refusal> {
    use crate::settings::Refusal as Keyed;
    let local = crate::profiles::LOCAL_FILE;
    let Ok(top) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    let Some(table) = top.get(TABLE) else {
        return Vec::new();
    };
    let Some(table) = table.as_table() else {
        return vec![Keyed::at(
            format!(
                "{TABLE} in {local} is not a table — this machine's [{TABLE}] holds hosts, and \
                 only hosts"
            ),
            &[TABLE],
        )];
    };
    let mut out: Vec<Keyed> = table
        .keys()
        .filter(|key| ![hosts::KEY, personal::LEVEL, personal::PERSONAS].contains(&key.as_str()))
        .map(|key| {
            Keyed::at(
                format!(
                    "{TABLE}.{key} in {local} is not read — this machine's [{TABLE}] holds \
                     hosts and your own level ({}, {}), and nothing else. The project's mode \
                     and presets are the project's, in {FILE}.",
                    personal::LEVEL,
                    personal::PERSONAS
                ),
                &[TABLE, key.as_str()],
            )
        })
        .collect();
    // Your own level for this project (#2071): a value that is not one is refused.
    out.extend(personal::read(table, local).1.into_iter().map(|refused| {
        let key: Vec<&str> = refused.key.iter().map(String::as_str).collect();
        Keyed::at(refused.why, &key)
    }));
    match hosts::read(table.get(hosts::KEY)) {
        Ok(hosts::Listed { refused, .. }) => {
            out.extend(refused.into_iter().map(|(written, why)| {
                Keyed::at(
                    format!(
                        "{TABLE}.hosts in {local} names {written}, which no chat is let reach: \
                         {why}"
                    ),
                    &[TABLE, hosts::KEY],
                )
            }))
        }
        Err(hosts::NotAList) => out.push(Keyed::at(
            format!(
                "{TABLE}.hosts in {local} is not a list of hosts, so none of yours is allowed — \
                 write hosts = [\"api.example.com\", \"10.0.0.5:6443\"]"
            ),
            &[TABLE, hosts::KEY],
        )),
    }
    out
}

/// What a chat of `harness` in the plane at `root` starts under on `machine`: `None` where no
/// sandbox is in force (the plane has not turned it on, and no policy requires it:
/// [`Plane::in_force`]), the compiled sandbox where one is — or why the chat does not start.
/// `has` answers whether a program the backend needs is installed ([`backend::installed`]).
///
/// **Fail closed** (ADR 0067 §1). A harness charter has no compiler for, a machine the policy
/// cannot be applied on, and a class the harness cannot hold here each refuse the chat; none
/// of them starts it unsandboxed.
pub fn for_start(
    harness: Harness,
    root: &Path,
    machine: &Machine,
    has: &dyn Fn(&str) -> bool,
) -> Result<Option<Applied>, NotStarted> {
    for_start_granted(harness, root, machine, has, None, &grant::Grants::default())
}

/// [`for_start`] for a chat running as `persona`, which adds the persona's own grants (#1362).
pub fn for_start_as(
    harness: Harness,
    root: &Path,
    machine: &Machine,
    has: &dyn Fn(&str) -> bool,
    persona: Option<&str>,
) -> Result<Option<Applied>, NotStarted> {
    for_start_granted(
        harness,
        root,
        machine,
        has,
        persona,
        &grant::Grants::default(),
    )
}

/// [`for_start_as`], for a chat a person let do `chat` besides (#1342): compiled in by
/// [`Compiled::granted`].
pub fn for_start_granted(
    harness: Harness,
    root: &Path,
    machine: &Machine,
    has: &dyn Fn(&str) -> bool,
    persona: Option<&str>,
    chat: &grant::Grants,
) -> Result<Option<Applied>, NotStarted> {
    for_start_at(harness, root, machine, has, persona, chat, None)
}

/// Whether a person's own `floor` starts the floor in a project at the full sandbox: not where
/// the project's `policy = "never"` or an administrator's policy forbids them a looser sandbox
/// than the project's, as the team's bound says ([`team::allowed_levels_under`]).
/// What a start does, and what Settings says of their level, both ask this.
pub fn floor_below_full_allowed(root: &Path, locks: &policy::Locks) -> bool {
    team::allowed_levels_under(root, locks).allows(personal::Choice::Floor)
}

/// [`for_start_granted`] at `at`, a level a person's own setting chose ([`decide_granted`]),
/// or `None` for the project's own: **the full sandbox** where the project turned it on, or with
/// the defaults its file says where it has not; **the floor** where the project has not turned
/// it on, or where it has and the person chose the floor and no policy forbids them a looser
/// sandbox than the project's ([`policy::Locks::opt_out_refused`]).
fn for_start_at(
    harness: Harness,
    root: &Path,
    machine: &Machine,
    has: &dyn Fn(&str) -> bool,
    persona: Option<&str>,
    chat: &grant::Grants,
    at: Option<Level>,
) -> Result<Option<Applied>, NotStarted> {
    // One spelling of the plane for every harness: the kernel's.
    let real_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let root = real_root.as_path();
    let plane = Plane::read(root);
    // Fail closed: a file that may say `[sandbox]` and cannot be read never reads as "not set".
    if plane.unreadable() {
        return Err(NotStarted::PlaneUnreadable);
    }
    // The project's own, or the one an administrator's policy requires (D-1423-1); else the
    // floor (#2071). A person's level moves it either way (#2071, ADR 0096 §8).
    let locks = policy::Locks::of(root);
    let policy = match (plane.in_force(&locks), at) {
        (Some(_), Some(Level::Floor)) if floor_below_full_allowed(root, &locks) => None,
        (Some(policy), _) => Some(policy),
        (None, Some(Level::Full)) => Some(plane.said().unset.unwrap_or_else(Policy::defaults)),
        (None, _) => None,
    };
    let Some(policy) = policy else {
        return floor_for_start(harness, root, &plane, machine, has, persona, chat).map(Some);
    };
    let Some(compile) = compiler(harness) else {
        return Err(NotStarted::NoCompiler(harness));
    };
    // Ruling V87f: the one place a harness with a compiler is held back.
    if let Some(issue) = harness.adapter().sandbox_held_back() {
        return Err(NotStarted::HeldBack(harness, issue));
    }
    if let Some(missing) = backend::missing_for(machine.os, machine.os.purlis_wraps(harness), has) {
        return Err(NotStarted::NoBackend(missing));
    }
    let compiled = Compiled::granted(&policy, &plane, root, machine, persona, chat);
    let mut applied = applied_of(harness, compile, &compiled, root, machine)?;
    // The persona's tools (#1407), only as the person on this machine allowed them with its
    // hosts (D-PE-ruling-1), each program found where no chat writes. What is armed is also
    // what a brokered `secret exec` for this chat may run (D-PE-ruling-2).
    if let Some(name) = persona {
        // Every folder the chat may write is skipped in the search (#1828): its grants, and
        // what the sandbox widens to, the cache homes among them, as a brokered run of the chat
        // drops them ([`crate::secrets::brokered::chat_writes`]).
        let mut writable = compiled.writable.clone();
        writable.extend(applied.writable());
        if let Some(resolved) = tools::for_chat(
            root,
            &policy.personas,
            name,
            &policy::Locks::of(root),
            &crate::programs::search_dirs(),
            &writable,
        ) {
            applied.confines.tools = resolved.ready.clone();
            applied.tools = Some(Box::new(resolved));
        }
    }
    Ok(Some(applied))
}

/// **The floor for a chat of `harness`** in a project that has not turned the sandbox on
/// (#2071): [`Compiled::floor`] compiled by the harness's own compiler, with the persona's
/// tools armed as the full sandbox arms them, so a brokered `secret exec` still runs only those
/// ([`tools::declared`]); or why it cannot be applied here, which [`decide_granted`] turns into
/// a start without it ([`Decided::Floorless`], the operator's ruling Q2).
fn floor_for_start(
    harness: Harness,
    root: &Path,
    plane: &Plane,
    machine: &Machine,
    has: &dyn Fn(&str) -> bool,
    persona: Option<&str>,
    chat: &grant::Grants,
) -> Result<Applied, NotStarted> {
    let Some(compile) = compiler(harness) else {
        return Err(NotStarted::NoCompiler(harness));
    };
    if let Some(issue) = harness.adapter().sandbox_held_back() {
        return Err(NotStarted::HeldBack(harness, issue));
    }
    if let Some(missing) = backend::missing_for(machine.os, machine.os.purlis_wraps(harness), has) {
        return Err(NotStarted::NoBackend(missing));
    }
    let compiled = Compiled::floor(root, machine, chat);
    let mut applied = applied_of(harness, compile, &compiled, root, machine)?;
    if let Some(name) = persona {
        // The personas the project declares, and their tools (#1407): as the file says them with
        // the sandbox turned on, or not, so a chat at the floor in either runs the same tools.
        let personas = floor_personas(plane);
        let mut writable = compiled.writable.clone();
        writable.extend(applied.writable());
        if let Some(resolved) = tools::for_chat(
            root,
            &personas,
            name,
            &policy::Locks::of(root),
            &crate::programs::search_dirs(),
            &writable,
        ) {
            let programs: Vec<PathBuf> = resolved
                .ready
                .iter()
                .map(|ready| ready.program.clone())
                .collect();
            applied.confines.tools = resolved.ready.clone();
            applied.tools = Some(Box::new(resolved));
            // Each tool's program is one the person runs too: never written by the chat (Q4).
            for program in programs {
                applied.hold_programs(&[program.display().to_string()]);
            }
        }
    }
    Ok(applied)
}

/// **The personas the project at the floor declares** (#1407, #2071): its own `[sandbox]`, as
/// the file says it whether or not it turned the full sandbox on, else the defaults. A chat at
/// the floor in a project at the full sandbox (your level) arms the same tools the full one does.
pub(crate) fn floor_personas(plane: &Plane) -> std::collections::BTreeMap<String, persona::Grants> {
    let said = plane.said();
    said.policy
        .or(said.unset)
        .unwrap_or_else(Policy::defaults)
        .personas
}

/// `compiled`, compiled by `compile` for a chat of `harness` in the plane at `root` on
/// `machine`, with what a command run for that chat is held to ([`Confines`]).
pub(crate) fn applied_of(
    harness: Harness,
    compile: Compiler,
    compiled: &Compiled,
    root: &Path,
    machine: &Machine,
) -> Result<Applied, NotStarted> {
    // What waits for a chat's next turn is denied where the harness's hooks, which deliver
    // it, run outside the sandbox ([`Denied::and_what_waits_for_a_chat`]).
    let with_what_waits;
    let compiled = if harness.adapter().sandbox_holds_what_it_starts() {
        compiled
    } else {
        with_what_waits = Compiled {
            denied: compiled.denied.clone().and_what_waits_for_a_chat(root),
            ..compiled.clone()
        };
        &with_what_waits
    };
    // No door opens for a Claude Code chat (#2019): a listen grant gives it nothing, neither in
    // its own sandbox's settings nor through its proxy.
    let unserving;
    let compiled = if harness == Harness::ClaudeCode && !compiled.widened.listen_only.is_empty() {
        let reach = compiled.reach.clone();
        for port in &compiled.widened.listen_only {
            reach.withdraw(*port);
        }
        unserving = Compiled {
            widened: compiled.widened.clone().unserving(),
            reach,
            ..compiled.clone()
        };
        &unserving
    } else {
        compiled
    };
    let (form, denied) = compile_checked(compile, compiled, root, machine)?;
    let mut held = denied.clone();
    if let Some(keychains) = seatbelt::keychains(machine.home.as_deref())
        && !held.contains(&keychains)
    {
        held.push(keychains);
    }
    Ok(Applied {
        harness,
        level: compiled.level,
        form: Box::new(form),
        root: root.to_path_buf(),
        denied,
        caches: compiled.widened.caches.clone().map(Box::new),
        confines: Box::new(Confines {
            level: compiled.level,
            held: compiled.held.clone(),
            denied: held,
            hosts: compiled.hosts.clone(),
            reach: compiled.reach.clone(),
            widened: compiled.widened.clone(),
            writable: compiled.writable.clone(),
            tools: Vec::new(),
            wrap: None,
        }),
        reach: Box::new(compiled.reach.clone()),
        // Claude Code waits for its program's answer ([`Applied::answered`]).
        through_proxy: harness != Harness::ClaudeCode,
        older: None,
        tools: None,
        wrap: None,
    })
}

/// What a command purlis runs on a sandboxed chat's behalf is held to (#1407): exactly what the
/// chat's own sandbox was compiled to deny, as its harness's compiler listed it (a Codex chat's
/// own Codex home among them, D-88q) with the credential store's files, and the hosts it may
/// reach. Recorded when the chat starts, so a change to the project's policy afterwards does
/// not change what a run for that chat may do. What the chat's sandbox widens (#1337) is
/// recorded too, so the run gets the chat's certificate check and cache grants.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Confines {
    /// How much the chat's own sandbox holds ([`Level`]): at the floor a run for it reaches the
    /// network as the chat does, with no proxy and no host list.
    pub level: Level,
    /// At the floor, the folders the chat may not write that no class names
    /// ([`Compiled::held`]): held for a run too.
    pub held: Vec<PathBuf>,
    pub denied: Vec<Denial>,
    pub hosts: Vec<String>,
    /// The layer that lists each of [`Self::hosts`] (#1708): Open, the persona's, yours or this
    /// chat's, as the chat's own proxy decides by, so a run's connections are recorded by the
    /// same word as the chat's ([`Self::decides`]).
    pub reach: reach::Reach,
    /// What the chat's sandbox widens (#1337): its cache grants and certificate check, so a
    /// run for the chat gets them too.
    pub widened: Widened,
    /// The folders a person let the chat write besides its own ([`Compiled::writable`], #1342):
    /// its own grants and the every-chat ones. Recorded so a start compiled later can be told
    /// from this one by them (#1428); a run for the chat is not widened by it.
    pub writable: Vec<PathBuf>,
    /// The persona's tools armed for the chat as it started (D-PE-ruling-1): each allowed on
    /// this machine, with the program found for it where no chat writes. The only commands a
    /// brokered `secret exec` runs for the chat (D-PE-ruling-2, [`tools::declared`]).
    pub tools: Vec<tools::Ready>,
    /// On Linux, purlis's own program as the chat's start checked it and held it
    /// ([`Applied::wrap_with`], #1040): what a run for the chat is wrapped with (#2342). `None`
    /// elsewhere, and where the start found none it could hold; a run for such a chat is
    /// refused.
    pub wrap: Option<PathBuf>,
}

impl Confines {
    /// **What a run for the chat decides a connection by** (#1708): the chat's own layers, so
    /// a host listed for the chat is carried as the chat's proxy carries it (Open, persona,
    /// yours, this chat's) and recorded by that word. A host in [`Self::hosts`] no layer was
    /// kept for is an Open host, as every listed host was before the layers were kept.
    pub fn decides(&self) -> reach::Reach {
        let layered = self.reach.hosts();
        self.reach.clone().and(
            self.hosts
                .iter()
                .filter(|host| !layered.contains(host))
                .map(|host| (host.clone(), reach::By::Open)),
        )
    }

    /// `host`, allowed live at scope `by` since the chat started (#1666): listed for a run
    /// started now, at that scope.
    pub fn allow_live(&mut self, host: &str, by: reach::By) {
        if !self.hosts.iter().any(|one| one == host) {
            self.hosts.push(host.to_owned());
        }
        self.reach = std::mem::take(&mut self.reach).and([(host.to_owned(), by)]);
    }
}

/// `policy` for a chat of `harness` in `plane` at `root`, with no `charter.toml` written and no
/// backend asked: for a test of what a compiler makes of a policy.
#[cfg(test)]
pub(crate) fn applied_for(
    harness: Harness,
    policy: &Policy,
    plane: &Plane,
    root: &Path,
    machine: &Machine,
) -> Result<Applied, NotStarted> {
    let compile = compiler(harness).expect("a harness with a compiler");
    let compiled = Compiled::of(policy, plane, root, machine, None);
    applied_of(harness, compile, &compiled, root, machine)
        .map(|applied| on_a_profile(applied, machine))
}

/// `applied` as a start applies it to a chat on a profile: a Codex chat on a profile for
/// sandboxed chats only, whose folder is Codex's home (#2037); every other harness as compiled.
#[cfg(test)]
pub(crate) fn on_a_profile(mut applied: Applied, machine: &Machine) -> Applied {
    // Its home as a start checks it on disk; these tests' machines are paths only.
    if let Form::Codex(wrap) = &mut *applied.form {
        wrap.home = Homes::of(machine)
            .profiles
            .map(|profiles| profiles.join("codex-sandbox/config"));
    }
    applied
}

/// [`for_start`] without asking this machine for a backend, for a test of a compiler alone.
#[cfg(test)]
pub(crate) fn compiled_anyway(
    harness: Harness,
    root: &Path,
    machine: &Machine,
) -> Result<Applied, NotStarted> {
    let plane = Plane::read(root);
    let policy = plane
        .said()
        .policy
        .expect("a plane that turned the sandbox on");
    let compile = compiler(harness).ok_or(NotStarted::NoCompiler(harness))?;
    let compiled = Compiled::of(&policy, &plane, root, machine, None);
    applied_of(harness, compile, &compiled, root, machine)
        .map(|applied| on_a_profile(applied, machine))
}

/// A person's choice, in the window, to start one chat without the sandbox (ADR 0067 §7,
/// ruling V78 a): the new-chat picker's "Start without the sandbox".
///
/// **Only the window makes one** — the app's start command, on the human `local-ui` scope.
/// No CLI word, no file, no profile and no chat can: [`crate::start::Start::without_sandbox`]
/// is `None` everywhere else, and nothing is recorded that a later start reads as one, so it is
/// never inherited by a new chat, a resumed chat, a relaunch, a workspace or the project.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptOut {
    /// What the person typed as the reason, if anything: an operator note for the audit.
    pub reason: Option<String>,
}

impl OptOut {
    /// The longest reason kept, in characters.
    pub const MOST_REASON_CHARS: usize = 200;

    /// The reason as the audit keeps it: one line, its whitespace runs made one space, clipped
    /// to [`Self::MOST_REASON_CHARS`] with its ellipsis, and none when it is blank.
    fn kept_reason(&self) -> Option<String> {
        let words: Vec<&str> = self.reason.as_deref()?.split_whitespace().collect();
        (!words.is_empty())
            .then(|| crate::shown::one_line(&words.join(" "), Self::MOST_REASON_CHARS - 1))
    }
}

/// Who started a chat in a sandboxed project without the sandbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum By {
    /// A person, in the window, for this chat ([`OptOut`]).
    Person,
    /// A person, by their own level for this project or the chat's persona, as they set or
    /// confirmed it in Settings on this machine (#2071, ADR 0096; [`your_level`]), at a start
    /// made [`Via`] this way.
    YourLevel(Via),
    /// Charter itself, because this operating system has no sandbox backend yet (Windows,
    /// M46, #565; ruling V21 3). The audit names charter as the actor, never the operator (ruling V78 b).
    NoBackend(Os),
}

/// A chat in a sandboxed project that starts without the sandbox: who chose it and why — what
/// its `trust.sandbox.off` event records ([`crate::eventlog::Recorder::trust`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lifted {
    pub by: By,
    /// The person's reason, one line, where one was typed.
    pub reason: Option<String>,
}

impl Lifted {
    /// What is lifted: every class (ADR 0067 §5). An unsandboxed chat holds none of them.
    pub const CLASSES: [Class; 5] = Class::ALL;

    /// What the chat's tab says as it starts, so the opt-out is visible for its whole life.
    pub fn notice(&self) -> String {
        match self.by {
            By::Person => "This chat runs without the sandbox: you turned it off for this chat \
                           only. A new or resumed chat does not inherit it."
                .to_owned(),
            By::YourLevel(_) => "No sandbox — your setting. This chat runs without the sandbox \
                              because your level for this project, on this machine, is no \
                              sandbox (Settings › Sandbox › Your level). Every chat it covers \
                              starts this way until you change it."
                .to_owned(),
            // The Windows backend is M46, #565.
            By::NoBackend(os) => format!(
                "This chat runs without the sandbox: purlis has no sandbox backend on {} yet, \
                 so every chat here starts without it until one exists.",
                match os {
                    Os::Windows => "Windows",
                    Os::MacOs | Os::Linux | Os::Other => "this system",
                }
            ),
        }
    }
}

/// A change to one chat's sandbox, as its trust event records it (ADR 0067 §7; ADR 0075 §4's
/// `trust.sandbox.off` and `trust.sandbox.on`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// The chat starts without the sandbox.
    Off(Lifted),
    /// A chat whose last run was unsandboxed starts this run sandboxed: an opt-out lasts one
    /// run and is never inherited, so it is charter that puts it back on.
    On,
}

impl Change {
    /// The event's kind.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Off(_) => "trust.sandbox.off",
            Self::On => "trust.sandbox.on",
        }
    }

    /// The event's body: who, the chat's harness and persona, and the classes lifted or put
    /// back. Which chat, which run, when and on which machine are the envelope's (ADR 0066).
    ///
    /// **The person is named by their scope, never by a login** (ADR 0075 §3): the actor is
    /// `operator` on `local-ui`, and the audit's pseudonym replaces it when AU-18 lands.
    pub fn body(&self, harness: Option<Harness>, persona: Option<&str>) -> serde_json::Value {
        let classes: Vec<&str> = Lifted::CLASSES.iter().map(|class| class.word()).collect();
        let harness = harness.map(Harness::name);
        match self {
            Self::Off(Lifted {
                by: By::Person,
                reason,
            }) => serde_json::json!({
                "actor_kind": "human",
                "actor": "operator",
                "scope": "local-ui",
                "harness": harness,
                "persona": persona,
                "reason": reason,
                "lifted": classes,
            }),
            // The person's own setting, confirmed on this machine, at a start made `via`: the
            // actor is whoever made the start, never the person for a start a chat asked for.
            Self::Off(Lifted {
                by: By::YourLevel(via),
                reason,
            }) => {
                let (actor_kind, actor, scope) = via.actor();
                serde_json::json!({
                    "actor_kind": actor_kind,
                    "actor": actor,
                    "scope": scope,
                    "by": "your-level",
                    "made": via.word(),
                    "harness": harness,
                    "persona": persona,
                    "reason": reason,
                    "lifted": classes,
                })
            }
            Self::Off(Lifted {
                by: By::NoBackend(os),
                reason,
            }) => serde_json::json!({
                "actor_kind": "host",
                "actor": "purlis (no backend on this OS)",
                "os": match os {
                    Os::Windows => "windows",
                    Os::MacOs => "macos",
                    Os::Linux => "linux",
                    Os::Other => "other",
                },
                "harness": harness,
                "persona": persona,
                "reason": reason,
                "lifted": classes,
            }),
            Self::On => serde_json::json!({
                "actor_kind": "host",
                "actor": "charter",
                "harness": harness,
                "persona": persona,
                "restored": classes,
            }),
        }
    }
}

/// What a chat in a sandboxed project starts under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decided {
    /// The sandbox, compiled for its harness: the full sandbox or the floor ([`Applied::level`]).
    Sandboxed(Applied),
    /// No sandbox, and why ([`Lifted`]): audited as `trust.sandbox.off`.
    Unsandboxed(Lifted),
    /// No sandbox in a project that has not turned it on, because the floor cannot be applied
    /// here (#2071, the operator's ruling Q2): the sentence its tab says. The chat starts as it
    /// did before the floor; nothing was lifted, so nothing is audited.
    Floorless(String),
}

/// What a chat's tab says where the floor could not be applied (#2071): the reason of `why`, a
/// refusal of [`NotStarted`]. Only its reason: its lead ("this project runs every chat
/// sandboxed") and what it goes on to say (that nothing was started, and how to start without
/// the sandbox) are not so for a chat that started.
pub fn floorless_said(why: &NotStarted) -> String {
    let said = why.to_string();
    let reason = said
        .strip_prefix(LEAD)
        .and_then(|rest| rest.strip_prefix(", and "))
        .unwrap_or(&said);
    let reason = reason
        .split_once(", so ")
        .map_or(reason, |(reason, _)| reason);
    let reason = reason
        .split_once(". Nothing was started")
        .map_or(reason, |(reason, _)| reason)
        .trim_end_matches('.');
    format!(
        "This chat runs with no sandbox: purlis could not apply the floor here, because {reason}."
    )
}

/// What a chat of `harness` in the project at `root` starts under on `machine`, where `opt_out`
/// is a person's choice to start it without the sandbox: `None` where no sandbox is in force
/// (the project has not turned it on and no policy requires it, [`Plane::in_force`]: there is
/// nothing to lift, so nothing to audit), else sandboxed or unsandboxed — or why the chat does
/// not start.
///
/// **Fail closed, with two exits that are each audited** (ADR 0067 §1 and §7):
/// - a person's opt-out, which starts the chat without the sandbox even where it could not be
///   applied — the opt-out inside the refusal;
/// - Windows, where no backend exists yet and every chat starts at the opt-out with charter as
///   the actor (ruling V21 3). Any other system with no backend still refuses.
///
/// **An administrator's policy that forbids the opt-out closes both** (ADR 0067 §4 as amended,
/// D-1423-1): the opt-out is refused ([`NotStarted::OptOutLocked`]), and so is a start on a
/// system with no backend ([`NotStarted::RequiredWithoutBackend`]). Neither starts unconfined.
pub fn decide(
    harness: Harness,
    root: &Path,
    machine: &Machine,
    has: &dyn Fn(&str) -> bool,
    opt_out: Option<&OptOut>,
    persona: Option<&str>,
) -> Result<Option<Decided>, NotStarted> {
    decide_granted(
        harness,
        root,
        machine,
        has,
        &Person {
            opt_out,
            level_as: persona,
            made: &Made::Window,
        },
        persona,
        &grant::Grants::default(),
    )
}

/// **How a start was made** (#2071): what a person's own level is read through at it, and what
/// its audit names. Set by whoever builds the start, from the app's own record and never from
/// a request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Made {
    /// A start that does not say how it was made. **A person's `off` is never honoured at it**:
    /// a builder that forgot to say starts the chat sandboxed, never unsandboxed.
    #[default]
    Unnamed,
    /// The person, in the window: the new-chat picker, a curation action, a finished task
    /// reopened.
    Window,
    /// The person, resuming a session record from the Sessions panel.
    Resume,
    /// The app, starting a chat again from its record: a relaunch, a restart, a start in the
    /// place of one that could not resume. A chat another chat asked for is [`Self::Asked`]
    /// at every later start too.
    Relaunch,
    /// Another chat asked for it.
    Asked {
        how: Asking,
        /// **The chats above it, nearest first**, the chat the person started last: each one's
        /// persona, `None` for one on none (the project's level), from the app's record and
        /// never from a request. `None`, or an empty chain, where the record cannot say, and
        /// then a person's `off` is not honoured.
        above: Option<Vec<Option<String>>>,
    },
}

/// How another chat asked for a chat (#1434).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asking {
    Dispatch,
    Handoff,
}

/// [`Made`], as a chat's audit names it: a start a person's level lifted the sandbox at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Window,
    Resume,
    Relaunch,
    Dispatch,
    Handoff,
}

impl Via {
    /// How the start was made, as the audit's `made` says it.
    pub fn word(self) -> &'static str {
        match self {
            Self::Window => "local-ui",
            Self::Resume => "resume",
            Self::Relaunch => "relaunch",
            Self::Dispatch => "dispatch",
            Self::Handoff => "handoff",
        }
    }

    /// Who made the start, as the audit names an actor (ADR 0075 §3): the kind, the actor and
    /// its scope.
    fn actor(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Window | Self::Resume => ("human", "operator", "local-ui"),
            Self::Relaunch => ("host", "purlis", "relaunch"),
            Self::Dispatch | Self::Handoff => ("agent", "a chat", self.word()),
        }
    }
}

impl Made {
    /// How the audit names it; `None` for [`Self::Unnamed`], which never lifts anything.
    fn via(&self) -> Option<Via> {
        match self {
            Self::Unnamed => None,
            Self::Window => Some(Via::Window),
            Self::Resume => Some(Via::Resume),
            Self::Relaunch => Some(Via::Relaunch),
            Self::Asked {
                how: Asking::Dispatch,
                ..
            } => Some(Via::Dispatch),
            Self::Asked {
                how: Asking::Handoff,
                ..
            } => Some(Via::Handoff),
        }
    }
}

/// What a person brings to one start: their opt-out for it, the persona their level is read
/// for (the chat's own, `None` for a chat that names none: the project's), and how the start
/// was made.
#[derive(Debug, Clone, Copy)]
pub struct Person<'a> {
    pub opt_out: Option<&'a OptOut>,
    pub level_as: Option<&'a str>,
    pub made: &'a Made,
}

/// **The level a person's own setting starts a chat at** (#2071, ADR 0096): their level for
/// the project at `root`, or for `persona`, as they set or confirmed it on this machine
/// ([`personal::in_force`], never the file's word alone), within what policy lets a person
/// choose ([`bounded`]), and for a start another chat asked for, **the strictest level across
/// the chain of chats that asked** — the level each of them runs at — so no chat reaches past
/// its own by asking for another, however many hops away. A start that does not say how it was
/// made, or whose chain the record cannot name whole, never takes `off`.
pub fn your_level(
    root: &Path,
    persona: Option<&str>,
    locks: &policy::Locks,
    made: &Made,
) -> Option<personal::Choice> {
    let in_force = personal::in_force(root);
    // What the team's policy and this machine's let a person choose here, and the level of a
    // chat whose person chose none: the project's own — the full sandbox where it turned it on,
    // the floor where it has not, no sandbox once they accepted the team's off (#2071 PR C).
    // Every level below is read through it, the chain's included.
    let bound = team::allowed_levels_under(root, locks);
    let held = |level| bounded(level, &bound);
    let own = held(in_force.for_persona(persona));
    let not_off = |level: Option<personal::Choice>| level.filter(|l| *l != personal::Choice::Off);
    // Not set reads as the project's own level, which is what a chat with no level starts at.
    let unset = bound.default;
    // A start that does not say how it was made, or whose chain purlis cannot read whole, is
    // never read as a shorter chain: never `off`, and never below the project's own level.
    let unread = || not_off(own).map(|level| level.min(personal::base(root)));
    match made {
        Made::Unnamed | Made::Asked { above: None, .. } => unread(),
        Made::Asked {
            above: Some(above), ..
        } if above.is_empty() => unread(),
        Made::Asked {
            above: Some(above), ..
        } => {
            // A chat on no persona is kept in the chain under the default persona's name, so
            // an entry naming it is held to the project's level too.
            let default = crate::start::persona_for_a_new_chat(root);
            let project = held(in_force.for_persona(None)).unwrap_or(unset);
            let theirs = above
                .iter()
                .map(|one| {
                    let level = held(in_force.for_persona(one.as_deref())).unwrap_or(unset);
                    if one.is_some() && *one == default {
                        level.min(project)
                    } else {
                        level
                    }
                })
                .min()
                .unwrap_or(personal::Choice::On);
            Some(own.unwrap_or(unset).min(theirs))
        }
        Made::Window | Made::Resume | Made::Relaunch => own,
    }
}

/// **`level`, within `bound`**, the one place a start reads a person's level through (#2071,
/// PR C): their level, or the team's default where they chose none, never below what the
/// team's policy and this machine's let a person choose ([`team::allowed_levels`]): under
/// `policy = "never"`, or an administrator's policy that forbids the opt-out, the full sandbox.
pub fn bounded(level: Option<personal::Choice>, bound: &team::Bound) -> Option<personal::Choice> {
    Some(bound.clamp(level))
}

/// `person`'s own level at this start, read once ([`your_level`]): for the chat's own persona
/// ([`Person::level_as`]), never for whose grants it holds. `None` for a start that does not
/// say how it was made, which takes the project's own level.
fn yours(
    root: &Path,
    person: &Person<'_>,
    locks: &policy::Locks,
) -> Option<(personal::Choice, Via)> {
    let via = person.made.via()?;
    Some((your_level(root, person.level_as, locks, person.made)?, via))
}

/// What it lifts: every class where it is `off`, named by how the start was made.
fn yours_lifted(yours: Option<(personal::Choice, Via)>) -> Option<Lifted> {
    match yours? {
        (personal::Choice::Off, via) => Some(Lifted {
            by: By::YourLevel(via),
            reason: None,
        }),
        _ => None,
    }
}

/// [`decide`], for a chat a person let do `chat` besides (#1342).
pub fn decide_granted(
    harness: Harness,
    root: &Path,
    machine: &Machine,
    has: &dyn Fn(&str) -> bool,
    person: &Person<'_>,
    persona: Option<&str>,
    chat: &grant::Grants,
) -> Result<Option<Decided>, NotStarted> {
    let opt_out = person.opt_out;
    // A file that cannot be read may say `[sandbox]`, so it never reads as "not set", and nor
    // does one that has gone (D-1410e): each falls through to a refusal below, which the
    // opt-out sits inside.
    let plane = Plane::read(root);
    let locks = policy::Locks::of(root);
    if !plane.unreadable() && !plane.missing() && plane.in_force(&locks).is_none() {
        // The floor (#2071): a person may still start the chat without it, audited as the
        // opt-out is. Where it cannot be applied the chat starts as before, and says so.
        if let Some(opt_out) = opt_out {
            return Ok(Some(Decided::Unsandboxed(Lifted {
                by: By::Person,
                reason: opt_out.kept_reason(),
            })));
        }
        // Your own level, through the one seam as in a project at the full sandbox.
        let yours = yours(root, person, &locks);
        if let Some(lifted) = yours_lifted(yours) {
            return Ok(Some(Decided::Unsandboxed(lifted)));
        }
        let at = yours_at(yours);
        // **Your `on` is the full sandbox or no start** (#2071): chosen stricter than the floor,
        // it never ends as a start with no sandbox where the full one cannot be applied here. It
        // is refused with why, as in a project at the full sandbox, where the person may opt out.
        if at == Some(Level::Full) {
            return for_start_at(harness, root, machine, has, persona, chat, at)
                .map(|applied| applied.map(Decided::Sandboxed));
        }
        if !machine.os.has_backend() {
            return Ok(Some(Decided::Floorless(
                "This chat runs with no sandbox: purlis could not apply the floor here, because \
                 purlis has no sandbox backend on this system."
                    .to_owned(),
            )));
        }
        return Ok(Some(
            match for_start_at(harness, root, machine, has, persona, chat, at) {
                Ok(Some(applied)) => Decided::Sandboxed(applied),
                // The floor is always compiled where no policy is in force, so this is not met;
                // said all the same rather than started with nothing said.
                Ok(None) => Decided::Floorless(
                    "This chat runs with no sandbox: purlis did not compile the floor for it."
                        .to_owned(),
                ),
                Err(not) => Decided::Floorless(floorless_said(&not)),
            },
        ));
    }
    if let Some(opt_out) = opt_out {
        // An administrator's policy may forbid it (#1343, ADR 0067 §7): refused, saying who.
        if let Some(why) = locks.opt_out_refused() {
            return Err(NotStarted::OptOutLocked(why));
        }
        return Ok(Some(Decided::Unsandboxed(Lifted {
            by: By::Person,
            reason: opt_out.kept_reason(),
        })));
    }
    // Your own level (#2071): a confirmed `off` starts the chat as the opt-out does; `on` and
    // `floor` start it at that level below ([`yours_at`]).
    let yours = yours(root, person, &locks);
    if let Some(lifted) = yours_lifted(yours) {
        return Ok(Some(Decided::Unsandboxed(lifted)));
    }
    if !machine.os.has_backend() {
        // A policy that requires the sandbox is never met by starting without one (D-1423-1,
        // which replaces D-1343-9): refused, and never started unconfined.
        if locks.forbids_opt_out() {
            return Err(NotStarted::RequiredWithoutBackend(machine.os));
        }
        return Ok(Some(Decided::Unsandboxed(Lifted {
            by: By::NoBackend(Os::Windows),
            reason: None,
        })));
    }
    if plane.missing() {
        return Err(NotStarted::PlaneMissing);
    }
    for_start_at(harness, root, machine, has, persona, chat, yours_at(yours))
        .map(|applied| applied.map(Decided::Sandboxed))
}

/// The level `person`'s own setting starts the chat at, where it chose one: `on` the full
/// sandbox, `floor` the floor ([`your_level`]); `off` is [`yours_lifted`]'s.
fn yours_at(yours: Option<(personal::Choice, Via)>) -> Option<Level> {
    match yours?.0 {
        personal::Choice::On => Some(Level::Full),
        personal::Choice::Floor => Some(Level::Floor),
        personal::Choice::Off => None,
    }
}

/// What the new-chat picker says about the sandbox for a chat of one harness, before anything
/// starts (ruling V78 a): the reason is on screen beside "Start without the sandbox".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ahead {
    /// The project has not turned the sandbox on: the picker says nothing.
    Off,
    /// The chat starts sandboxed, unless the person opts out.
    Sandboxed,
    /// The chat starts without the sandbox whatever is picked (Windows), and why.
    Unsandboxed(Lifted),
    /// The sandbox cannot be applied, so the chat starts only without it: the refusal, and
    /// SD-30's install command where installing something would fix it (ruling V78 c).
    Refused {
        why: String,
        install: Option<String>,
    },
}

/// [`Ahead`] for a chat of `harness` in the project at `root` on `machine`, with `has` saying
/// what is installed and `os_release` the text of `/etc/os-release` (empty off Linux).
///
/// Every refusal the start would give is shown here first, beside "Start without the sandbox":
/// a harness charter holds back (Codex, ruling V87f), and — where the chat would be sandboxed —
/// what `check` says of the program under the sandbox compiled for it (ruling V87g): the start's
/// own [`program::checked`], asked by [`crate::start::sandbox_ahead`] with the start's words,
/// folder and environment. A profile nobody approved is never run, so its caller passes a
/// `check` that asks nothing.
pub fn ahead(
    harness: Harness,
    root: &Path,
    machine: &Machine,
    has: &dyn Fn(&str) -> bool,
    os_release: &str,
    check: &dyn Fn(&Applied) -> Result<(), NotStarted>,
) -> Ahead {
    ahead_decided(
        decide(harness, root, machine, has, None, None),
        root,
        os_release,
        check,
    )
}

/// [`ahead`], from what [`decide`] answered for the harness: the compile, asked once and shared
/// by every profile of that harness one picker lists (#2179), since it reads nothing of the
/// profile. `check` is then asked of each profile's own program.
pub fn ahead_decided(
    decided: Result<Option<Decided>, NotStarted>,
    root: &Path,
    os_release: &str,
    check: &dyn Fn(&Applied) -> Result<(), NotStarted>,
) -> Ahead {
    // Under a policy that forbids the opt-out no sentence sends the person to it (#1423); the
    // picker says what locks it on a line of its own.
    let locks = policy::Locks::of(root);
    let refused = |refused: NotStarted| Ahead::Refused {
        why: refused.said_beside(&locks),
        install: match &refused {
            NotStarted::NoBackend(missing) => backend::install_command(missing, os_release),
            NotStarted::NoCompiler(_)
            | NotStarted::HeldBack(..)
            | NotStarted::ProgramWritable(_)
            | NotStarted::WrapReplaced
            | NotStarted::KitWritable(_)
            | NotStarted::ProgramRelative
            | NotStarted::WordWritable(_)
            | NotStarted::WordTooLong
            | NotStarted::NotTheHarness(..)
            | NotStarted::ProbeTimedOut(_)
            | NotStarted::Uncompilable(_)
            | NotStarted::CoversItsGround { .. }
            | NotStarted::Unread(_)
            | NotStarted::KeptUntold(_)
            | NotStarted::PlaneUnreadable
            | NotStarted::PlaneMissing
            | NotStarted::OptOutLocked(_)
            | NotStarted::RequiredWithoutBackend(_) => None,
        },
    };
    match decided {
        Ok(None) => Ahead::Off,
        Ok(Some(Decided::Sandboxed(applied))) => match check(&applied) {
            Ok(()) => Ahead::Sandboxed,
            Err(not) => refused(not),
        },
        Ok(Some(Decided::Unsandboxed(lifted))) => Ahead::Unsandboxed(lifted),
        // Where the floor cannot be applied the chat starts as before, with nothing to choose.
        Ok(Some(Decided::Floorless(_))) => Ahead::Off,
        Err(not) => refused(not),
    }
}

/// What one start of a chat means for the sandbox's audit and its count: the trust event to
/// write, if any, and how to count the chat, if it is new.
///
/// - `lifted`: it starts without the sandbox in a project that has it on — `trust.sandbox.off`.
/// - `sandboxed`: it starts under the sandbox; where its last run was unsandboxed
///   (`was_unsandboxed`), the sandbox is back on — `trust.sandbox.on`.
/// - `new_chat`: a chat that was not open before, counted once towards the opt-out rate. A
///   relaunch or a start in a chat's place is another run of the same chat, never counted.
pub fn at_start(
    lifted: Option<&Lifted>,
    sandboxed: bool,
    was_unsandboxed: bool,
    new_chat: bool,
) -> (Option<Change>, Option<local::Started>) {
    let counted = |started| new_chat.then_some(started);
    match lifted {
        Some(lifted) => (
            Some(Change::Off(lifted.clone())),
            counted(match lifted.by {
                // Your level is a person's opt-out too, counted as one.
                By::Person | By::YourLevel(_) => local::Started::OptedOut,
                By::NoBackend(_) => local::Started::NoBackend,
            }),
        ),
        None if sandboxed => (
            was_unsandboxed.then_some(Change::On),
            counted(local::Started::Sandboxed),
        ),
        None => (None, None),
    }
}

pub mod asks;
pub mod backend;
pub mod browser_listing;
pub mod browser_reach;
pub mod browser_sites;
pub mod caches;
pub mod claude;
pub mod codex;
pub mod confined;
pub mod door;
pub mod egress;
pub mod grant;
pub mod held;
pub mod hosted;
pub mod hosts;
pub mod internal;
pub mod kubeconfig;
pub mod local;
pub mod localport;
pub mod nat64;
pub mod opencode;
pub mod persona;
pub mod personal;
pub mod plan;
pub mod planted;
pub mod policy;
pub mod program;
pub mod ran;
pub mod reach;
pub mod seatbelt;
pub mod services;
pub mod signin;
pub mod team;
pub mod tools;
pub mod tunnel;
pub mod wrapper;

#[cfg(test)]
mod claude_tests;
#[cfg(test)]
mod codex_tests;
#[cfg(test)]
mod floor_tests;
#[cfg(test)]
mod kept_tests;
#[cfg(test)]
mod opencode_tests;
#[cfg(test)]
mod persona_tests;
#[cfg(test)]
mod reach_tests;
#[cfg(test)]
mod signin_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tunnel_tests;
