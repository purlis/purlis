// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the window asks about a project's sandbox (ADR 0067; ruling V78): what the new-chat
//! picker says before a chat starts, the one-time offer to an existing project, this machine's
//! opt-out count, and SD-30's install action.
//!
//! **The window names nothing that runs.** The install action sends a session number and no
//! text: the line typed is built here from charter's own table and what this machine is
//! missing ([`purlis_core::sandbox::backend::install_command`]), and it is typed without a
//! newline, so the person reads it and presses Return. It needs `sudo` (ruling V78 c).

use purlis_core::sandbox::{self, Ahead, backend};

use crate::planes::{PlaneId, Planes};

/// What the picker says about the sandbox for one profile, before anything starts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SandboxAhead {
    /// `sandboxed`, `unsandboxed` (this system has no backend) or `refused`.
    pub state: String,
    /// The sentence the picker shows: why it is refused, or why it starts without the sandbox.
    /// Empty for `sandboxed`.
    pub said: String,
    /// SD-30's install command, shown before the person asks for it to be typed: where the
    /// refusal is a program this machine is missing and charter knows its distribution.
    pub install: Option<String>,
    /// Where policy forbids starting a chat without the sandbox (#1343): why, naming the policy
    /// and who set it. The picker then offers no opt-out.
    pub locked: Option<String>,
}

impl SandboxAhead {
    /// The row for `ahead`, or none where the project has not turned the sandbox on.
    pub fn of(ahead: Ahead) -> Option<Self> {
        let row = |state: &str, said: String, install| Self {
            state: state.to_owned(),
            said,
            install,
            locked: None,
        };
        match ahead {
            Ahead::Off => None,
            Ahead::Sandboxed => Some(row("sandboxed", String::new(), None)),
            Ahead::Unsandboxed(lifted) => Some(row("unsandboxed", lifted.notice(), None)),
            Ahead::Refused { why, install } => Some(row("refused", why, install)),
        }
    }
}

/// The text of this machine's `/etc/os-release`, empty where there is none.
fn os_release() -> String {
    std::fs::read_to_string("/etc/os-release")
        .or_else(|_| std::fs::read_to_string("/usr/lib/os-release"))
        .unwrap_or_default()
}

/// What the picker says beside a profile whose program it has not checked: nobody approved
/// it yet, so it is not run, not even to ask its `--version`.
pub const CHECKED_ONCE_APPROVED: &str = "purlis checks this profile's program before it \
                                         starts sandboxed, once the profile may start: approved, \
                                         and declared in a file git does not carry.";

/// [`SandboxAhead`] for a chat on each of `profiles` in the project at `root`, in their order,
/// on `machine` with `has` saying what is installed: every refusal the start would give, shown
/// before anything starts — a harness charter holds back (Codex, ruling V87f), and a program
/// the sandbox will not bind (ruling V87g), which is asked of the profile's own resolved words.
///
/// **Each harness's sandbox is compiled once** for the whole list
/// ([`purlis_core::start::AheadOnce`], #2179): the picker asks for every profile it lists each
/// time a New tab opens, and a compile per row made every open pay one per profile. Each
/// profile's program is still checked for itself.
///
/// **Only a profile the start would run has its program run.** The V87g check asks the
/// program its `--version`, which runs it, outside any sandbox. So it is asked only past the
/// start's own gate ([`purlis_core::start::may_check_program`], which is
/// `wiring::refusal`): a startable kind, a `charter.local.toml` git would not carry, an
/// approved command. Any other row says the program is checked once the profile may start.
pub fn ahead_of_each(
    profiles: &[purlis_core::profiles::Profile],
    root: &std::path::Path,
    machine: &sandbox::Machine,
    has: &dyn Fn(&str) -> bool,
) -> Vec<Option<SandboxAhead>> {
    let os_release = os_release();
    let asked = purlis_core::start::AheadOnce::new(root, machine, has, &os_release, None);
    profiles
        .iter()
        .map(|profile| {
            ahead_with(
                &asked,
                profile,
                root,
                purlis_core::start::may_check_program(profile, root),
            )
        })
        .collect()
}

/// One row of [`ahead_of_each`], told whether the start's gate lets the program be checked:
/// the start's own check ([`purlis_core::start::sandbox_ahead`]), asked through `asked`, which
/// asks the same gate itself and runs the program only past it. `approved` only phrases the
/// row.
fn ahead_with(
    asked: &purlis_core::start::AheadOnce<'_>,
    profile: &purlis_core::profiles::Profile,
    root: &std::path::Path,
    approved: bool,
) -> Option<SandboxAhead> {
    let row = SandboxAhead::of(asked.of(profile)?)?;
    let row = SandboxAhead {
        // Policy's lock first; a profile for sandboxed chats only is never started without it
        // either (#2037).
        locked: sandbox::policy::Locks::of(root)
            .opt_out_refused()
            .or_else(|| {
                purlis_core::profiles::sandbox_only(profile)
                    .then(|| purlis_core::start::SANDBOX_ONLY_LOCK.to_owned())
            }),
        ..row
    };
    Some(if !approved && row.state == "sandboxed" {
        SandboxAhead {
            said: CHECKED_ONCE_APPROVED.to_owned(),
            ..row
        }
    } else {
        row
    })
}

/// What the project view says about the sandbox.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SandboxState {
    /// Whether chats here run sandboxed: the project turned the sandbox on, or an
    /// administrator's policy requires it on this machine (`policy.required`, #1423).
    pub on: bool,
    /// Whether the one-time offer is due: an existing project that has not turned it on, and
    /// nobody on this machine has answered (ruling V21 1).
    pub offer: bool,
    /// This machine's opt-out count in one sentence (V12), where the project has it on. Local
    /// only, never sent (ruling V78 d).
    pub said: Option<String>,
    /// The harnesses whose chats never start sandboxed on this machine, whatever the project,
    /// each with why (`sandbox::never_on`): Codex and opencode off macOS. The offer
    /// says them, so "every new chat runs sandboxed" is never read as covering them.
    pub never: Vec<String>,
    /// How the project's own hosts changed since this machine last told the person (#1341):
    /// the one-time Notice each teammate sees. `null` when nothing did.
    pub hosts_changed: Option<HostsChanged>,
    /// How the project's browser sites for everyone in it changed since this machine last
    /// showed them (#2443): one each reaches no browser here until it was shown. `null` when
    /// nothing did.
    #[specta(optional)]
    pub browser_sites_changed: Option<BrowserSitesChanged>,
    /// How the project's Internet access presets changed since this machine last told the
    /// person (#1385): the one-time Notice each teammate sees, so a preset that widens never
    /// widens unseen. `null` when nothing did.
    pub presets_changed: Option<PresetsChanged>,
    /// Every preset a project may turn on, in the core's order, each with the hosts it lets a
    /// chat reach here (#1340): Settings › Sandbox draws them, and lists no host of its own.
    pub presets: Vec<SandboxPreset>,
    /// Each persona's own hosts, where the project has the sandbox on (#1362).
    pub persona_hosts: Vec<PersonaHosts>,
    /// What every chat here reaches and writes on this machine besides its presets (#1340), as
    /// the core grants it (`sandbox::besides`): none where the sandbox is off.
    pub besides: SandboxBesides,
    /// An administrator's policy on this machine (#1343): what it locks, and who set it. `null`
    /// where there is none.
    pub policy: Option<SandboxPolicy>,
    /// Where the project's own manifest has gone or cannot be read as TOML (#1411, D-1410e):
    /// one sentence saying chats here are refused until it is restored, which Settings shows in
    /// place of the mode. Such a project never reads as "off". `null` where the manifest reads.
    #[specta(optional)]
    pub manifest: Option<String>,
    /// Whether a chat here has the system's certificate check (R1, 2026-10-13), where chats
    /// here run sandboxed: what Settings' one switch says. `null` where the sandbox is off.
    #[specta(optional)]
    pub certificate_check: Option<CertificateCheck>,
    /// What the project's committed file declares it expects that this machine has not
    /// allowed (ADR 0093): asked about once, up front. `null` where nothing waits.
    #[specta(optional)]
    pub expects: Option<ProjectExpects>,
    /// **The project's sandbox off** (#2071 PR C, #2156): its committed `[sandbox] mode =
    /// "off"`, where it came from as far as purlis verified it, and how the person on this
    /// machine answered it. `null` where the project does not turn the sandbox off in a commit.
    #[specta(optional)]
    pub team_off: Option<TeamOffState>,
    /// Why nobody here runs below the full sandbox (#2071 PR C): the project's
    /// `policy = "never"`, or an administrator's policy, in one sentence. Settings shows it in
    /// place of the level switch. `null` where each person chooses.
    #[specta(optional)]
    pub level_held: Option<String>,
    /// **Your own level here** (#2071, ADR 0096), as it starts a chat you start in this
    /// project: `null` where you set none that counts, which leaves the project's own.
    #[specta(optional)]
    pub yours: Option<YourLevel>,
}

/// **The project's sandbox off, as this machine's person is asked it** (#2071, SO-8; #2156):
/// once, with Accept and Keep it on for me; Settings offers the other answer after. It names
/// the commit and its author only where purlis verified them on the upstream it fetched, and
/// otherwise says the off is local.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct TeamOffState {
    /// The commit that turned it off, whole, where purlis verified it on the upstream it
    /// fetched; `null` where the off is local.
    pub commit: Option<String>,
    /// Who made that commit, as git names them, where verified as `commit` is. Written by a
    /// repository: drawn inertly.
    pub author: Option<String>,
    /// The ask's sentence, in purlis's words.
    pub said: String,
    /// `null` until answered; `true` accepted (no sandbox by default), `false` kept on.
    pub answer: Option<bool>,
}

impl TeamOffState {
    /// The window's form of `team_off`, where there is an off to answer.
    pub fn of(team_off: sandbox::team::TeamOff) -> Option<Self> {
        use sandbox::team::{Arrival, TeamOff};
        let (arrival, answer) = match team_off {
            TeamOff::Asks(arrival) => (arrival, None),
            TeamOff::Accepted(arrival) => (arrival, Some(true)),
            TeamOff::KeptOn(arrival) => (arrival, Some(false)),
            TeamOff::No | TeamOff::NotCommitted => return None,
        };
        let said = arrival.said();
        let (commit, author) = match arrival {
            Arrival::Upstream(arrived) => (Some(arrived.commit), Some(arrived.author)),
            Arrival::Local => (None, None),
        };
        Some(Self {
            commit,
            author,
            said,
            answer,
        })
    }
}

/// Your own level for a project, as it starts a chat you start there (#2071).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct YourLevel {
    /// `on`, `floor` or `off`.
    pub level: String,
    /// Whether it is the level you set or confirmed in Settings on this machine, rather than one
    /// the file says that counts unconfirmed because it is stricter.
    pub confirmed: bool,
}

/// [`YourLevel`] for the project at `root`: read through the one seam a start reads it through
/// (`sandbox::your_level`), for a chat you start in the window on no persona.
fn yours_of(root: &std::path::Path, locks: &sandbox::policy::Locks) -> Option<YourLevel> {
    // Only a level of yours: where you chose none, the project's own is said, or the team's.
    sandbox::personal::in_force(root).level?;
    // As a start takes it, within the team's bound: a floor the project's `never` or a policy
    // keeps at the full sandbox is said as the full sandbox, which a full project says itself.
    let level = sandbox::your_level(root, None, locks, &sandbox::Made::Window)?;
    // The project's own level is the project's to say, however you came to it.
    if level == sandbox::personal::base(root) {
        return None;
    }
    let confirmed = sandbox::local::confirmed_levels(root).level == Some(level);
    Some(YourLevel {
        level: level.word().to_owned(),
        confirmed,
    })
}

/// **What a project declares it expects and this machine has not allowed** (ADR 0093, ruling
/// 2: a committed file declares and never grants). The window asks about it once where the
/// declaration is read, on opening the project, and Settings offers the same Allow after a
/// "Not now" ([`sandbox::services::Awaiting`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ProjectExpects {
    pub items: Vec<ExpectedItem>,
    /// Where an administrator's policy forbids allowing any of it: the sentence that says so
    /// and who set it. Then nothing is offered.
    pub locked: Option<String>,
    /// Whether the person is still to be asked: something is declared that they have neither
    /// allowed nor said "Not now" to on this machine.
    pub asked: bool,
}

/// One local socket or system service a project expects ([`sandbox::services::Awaited`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ExpectedItem {
    pub what: GrantWhat,
    /// The socket's path or the service's name, as the committed file writes it: what an
    /// answer is sent back by. Written by a repository, so the window draws it inertly.
    pub target: String,
    /// **What an Allow would keep, and what the window names first**: the socket as the kernel
    /// names it, links resolved, where it can be allowed. A path a repository wrote may be a
    /// link to another socket, and the person is shown the one they would allow. As `target`
    /// for a service, and for a socket that cannot be allowed here.
    pub named: String,
    /// What allowing it means, in purlis's own words.
    pub says: String,
    /// Why it cannot be allowed on this machine now, where it cannot.
    pub refused: Option<String>,
}

/// **One item of the question as the window drew it**: what the person saw when they pressed
/// Allow, and so all an Allow may keep. A path a repository declared can lead somewhere else
/// by the time of the press, and an item that could not be allowed when it was drawn can have
/// become one that can; neither is what the person answered.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, specta::Type)]
pub struct ExpectedShown {
    /// As the committed file writes it ([`ExpectedItem::target`]).
    pub target: String,
    /// What the window named as what an Allow would keep ([`ExpectedItem::named`]); `null`
    /// where it drew the item as one that cannot be allowed here now.
    pub named: Option<String>,
}

/// Whether an Allow may keep `item`, as it stands now, of what the window drew (`shown`): it
/// can be allowed, it was drawn as one that can, and under the name it would be kept by now.
fn stands_as_shown(item: &sandbox::services::Awaited, shown: &[ExpectedShown]) -> bool {
    item.refused.is_none()
        && shown.iter().any(|one| {
            one.target == item.written && one.named.as_deref() == Some(&item.what.target())
        })
}

impl From<sandbox::services::Awaiting> for ProjectExpects {
    fn from(said: sandbox::services::Awaiting) -> Self {
        Self {
            items: said
                .items
                .into_iter()
                .map(|item| ExpectedItem {
                    what: match item.what {
                        sandbox::grant::What::Socket(_) => GrantWhat::Socket,
                        _ => GrantWhat::Service,
                    },
                    named: item.what.target(),
                    target: item.written,
                    says: item.said,
                    refused: item.refused,
                })
                .collect(),
            locked: said.locked,
            asked: said.asked,
        }
    }
}

/// **Whether a chat here has the system's certificate check, and why not** (the operator's
/// ruling R1 of 2026-10-13, amending ADR 0093): what a chat's next start compiles in
/// ([`sandbox::CertificateChecks`]). On by default; off where the project's
/// `certificate-checks` is `false`, or an administrator's policy locks it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct CertificateCheck {
    /// A chat has it from its next start.
    pub on: bool,
    /// Where an administrator's policy turns it off: the sentence that says so and who set
    /// it. Then the switch is drawn locked. `null` where it is the project's to say.
    pub locked: Option<String>,
    /// Whether the setting lets a chat reach anything on this system: macOS only, where the
    /// certificate check is a system service (D-1337-7). Elsewhere a chat's tools check
    /// certificates themselves, whatever the switch says.
    pub widens: bool,
}

impl CertificateCheck {
    /// What `said` comes to on a machine running `os`.
    fn of(said: sandbox::CertificateChecks, os: sandbox::Os) -> Self {
        let mac = os == sandbox::Os::MacOs;
        match said {
            sandbox::CertificateChecks::On => Self {
                on: true,
                locked: None,
                widens: mac,
            },
            sandbox::CertificateChecks::OffByProject => Self {
                on: false,
                locked: None,
                widens: mac,
            },
            sandbox::CertificateChecks::OffByPolicy(why) => Self {
                on: false,
                locked: Some(why),
                widens: mac,
            },
        }
    }
}

/// An administrator's policy (`sandbox::policy::Locks`), as Settings shows what it locks: each
/// locked value says [`Self::locked_by`] and offers no control.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SandboxPolicy {
    /// "Locked by policy, set by <who> in <file>.": said beside every value it locks.
    pub locked_by: String,
    /// The presets a project may turn on, by word, where the policy fixes them.
    pub presets: Option<Vec<String>>,
    /// The hosts any level may add, where the policy fixes them.
    pub hosts: Option<Vec<String>>,
    /// Whether your own hosts on this machine are forbidden.
    pub personal_hosts: bool,
    /// Whether a persona's own hosts are forbidden.
    pub persona_hosts: bool,
    /// Whether starting a chat without the sandbox is forbidden.
    pub opt_out: bool,
    /// Where the policy requires the sandbox (#1423): "On, required by policy, set by <who> in
    /// <file>.", which Settings says of the mode, with no control. One that forbids the opt-out
    /// does, in every project on this machine. `null` where it does not.
    pub required: Option<String>,
    /// Whether every folder a grant would let a chat write is forbidden.
    pub write_grants: bool,
}

impl SandboxPolicy {
    /// What `locks` lock, or none where no policy is in force.
    pub fn of(locks: &sandbox::policy::Locks) -> Option<Self> {
        locks.any().then(|| Self {
            locked_by: locks.locked_by(),
            presets: locks.fixes_presets().then(|| {
                locks
                    .presets(&sandbox::Preset::ALL)
                    .into_iter()
                    .map(|preset| preset.word().to_owned())
                    .collect()
            }),
            hosts: locks
                .allowed_hosts()
                .map(|hosts| hosts.iter().map(ToString::to_string).collect()),
            personal_hosts: locks.forbids_personal_hosts(),
            persona_hosts: locks.forbids_persona_hosts(),
            opt_out: locks.forbids_opt_out(),
            required: locks.required_by(),
            write_grants: locks.forbids_write_grants(),
        })
    }
}

/// `sandbox::Besides`, counted for the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, specta::Type)]
pub struct SandboxBesides {
    /// The project's own hosts every chat here is granted.
    pub project_hosts: u32,
    /// Your own hosts every chat here is granted on this machine.
    pub your_hosts: u32,
    /// The folders you let every chat here write on this machine.
    pub folders: u32,
}

impl From<sandbox::Besides> for SandboxBesides {
    fn from(said: sandbox::Besides) -> Self {
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        Self {
            project_hosts: count(said.project_hosts),
            your_hosts: count(said.your_hosts),
            folders: count(said.folders),
        }
    }
}

/// One Internet access preset (`sandbox::Preset`), as Settings shows it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SandboxPreset {
    /// The word the committed file names it by: `model-providers`.
    pub word: String,
    /// Its name in the window: `AI providers`.
    pub title: String,
    /// The hosts it lets a chat in this project reach: its own, and for code hosting the
    /// project's forges' too.
    pub hosts: Vec<String>,
    /// Whether a chat may write the project's own package caches while it is on
    /// (`sandbox::Preset::widens_caches`, #1422): the window keeps no copy of which one does.
    pub widens_caches: bool,
}

/// The hosts one persona's chats reach besides the project's (`[sandbox.personas.<name>]`), and
/// the tools they run through a vault, and whether the person allowed them on this machine
/// (#1362, D-1362-7; tools under the same Allow, D-PE-ruling-1).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PersonaHosts {
    pub persona: String,
    /// Each as the project's file lists it.
    pub hosts: Vec<String>,
    /// Those a chat would reach, which an Allow allows: none an administrator's policy locks
    /// out. Empty where policy forbids persona hosts.
    pub reached: Vec<String>,
    /// Its tools a chat would get, which the same Allow allows, each whole as a person reads
    /// it (`kubectl with kubeconfig from vault devops as KUBECONFIG file`). Empty where policy
    /// forbids persona tools. Where this and `reached` are both empty, nothing is asked.
    pub tools: Vec<String>,
    /// What an Allow of exactly this list sends back (`allow_persona_hosts`).
    pub digest: String,
    /// Whether the person allowed exactly this list here. Until then no chat reaches them, and
    /// the project view's Notice asks.
    pub allowed: bool,
    /// Whether it is the project's default persona, so its hosts reach every chat that names
    /// no persona too: part of what an Allow is of (D-1362-12).
    pub default: bool,
    /// Whether an Allow was kept here and the list, or its default-ness, has changed since: it
    /// grants nothing, and the Notice asks anew (D-1362-13).
    pub waiting: bool,
    /// One sentence for each of its tools whose program is a general-purpose one (T-4,
    /// [`sandbox::tools::caution`]), said beside the Allow and never part of what it is bound
    /// to; `null` where none is.
    #[specta(optional)]
    pub cautions: Option<Vec<String>>,
    /// One sentence for each of its tools that runs nothing because the program it runs here,
    /// as found and as its links resolve, is one no tool may be. Such a tool is not among
    /// `tools`, and no Allow is of it; `null` where none is.
    #[specta(optional)]
    pub refused: Option<Vec<String>>,
}

/// Every preset as the project at `plane` would have it reach under `locks`: a forge's host a
/// policy does not allow is not listed (D-1343-10).
fn presets_of(plane: &sandbox::Plane, locks: &sandbox::policy::Locks) -> Vec<SandboxPreset> {
    sandbox::Preset::ALL
        .into_iter()
        .map(|preset| SandboxPreset {
            word: preset.word().to_owned(),
            title: preset.title().to_owned(),
            hosts: sandbox::hosts(&[preset], plane, locks),
            widens_caches: preset.widens_caches(),
        })
        .collect()
}

/// Each persona's own hosts in the project at `root`, read as `plane`, under `locks`, and
/// whether the person allowed them here ([`sandbox::persona::shown_in`]).
fn persona_hosts_of(
    root: &std::path::Path,
    plane: &sandbox::Plane,
    locks: &sandbox::policy::Locks,
) -> Vec<PersonaHosts> {
    sandbox::persona::shown_in(root, plane, locks)
        .into_iter()
        .map(|one| PersonaHosts {
            allowed: one.allowed(),
            persona: one.persona,
            hosts: one.listed.iter().map(ToString::to_string).collect(),
            reached: one.hosts.iter().map(ToString::to_string).collect(),
            tools: one.tools.iter().map(sandbox::tools::Tool::whole).collect(),
            default: one.default,
            waiting: one.standing == sandbox::persona::Standing::Waiting,
            cautions: cautions_of(&one.tools),
            refused: Some(one.refused).filter(|refused| !refused.is_empty()),
            digest: one.digest,
        })
        .collect()
}

/// [`sandbox::tools::cautions`] of `tools`, `None` where there is none.
fn cautions_of(tools: &[sandbox::tools::Tool]) -> Option<Vec<String>> {
    Some(sandbox::tools::cautions(tools)).filter(|said| !said.is_empty())
}

/// The project's hosts as they changed (`sandbox::local::HostsChange`): each spelled as the
/// sandbox writes it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct HostsChanged {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    /// The whole list now: what the Notice sends back once it is read.
    pub now: Vec<String>,
}

/// The project's browser sites as they changed (`sandbox::browser_sites::Change`, #2443): each
/// spelled as the sandbox writes a host.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct BrowserSitesChanged {
    /// Listed and never shown here: what the Notice sends back once it is read.
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

/// The project's presets as they changed (`sandbox::local::PresetsChange`): each named as the
/// window names it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PresetsChanged {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    /// One sentence for each preset turned on that widens what a chat may do past its hosts.
    pub widens: Vec<String>,
    /// The whole set now, by the committed file's words: what the Notice sends back once read.
    pub now: Vec<String>,
}

fn state_of(root: &std::path::Path) -> SandboxState {
    state_on(root, sandbox::Os::this())
}

/// [`state_of`] on `os`.
fn state_on(root: &std::path::Path, os: sandbox::Os) -> SandboxState {
    let plane = sandbox::Plane::read(root);
    let locks = sandbox::policy::Locks::of(root);
    // The project's own, or the one an administrator's policy requires (D-1423-1).
    let on = plane.in_force(&locks).is_some();
    let manifest = manifest_said(root, &plane);
    SandboxState {
        on,
        // Taking the offer writes the manifest, which would put a new one in place of the gone
        // or unreadable one: nothing is offered until it is restored.
        offer: manifest.is_none() && sandbox::local::offer_due(root),
        said: on.then(|| sandbox::local::tally(root).said()),
        never: never_here(os),
        hosts_changed: sandbox::local::hosts_changed(root).map(|change| HostsChanged {
            added: change.added,
            removed: change.removed,
            now: change.now,
        }),
        browser_sites_changed: sandbox::browser_sites::changed(root, &locks).map(|change| {
            BrowserSitesChanged {
                added: change.added,
                removed: change.removed,
            }
        }),
        presets_changed: sandbox::local::presets_changed(root).map(|change| PresetsChanged {
            added: change.added,
            removed: change.removed,
            widens: change.widens,
            now: change.now,
        }),
        presets: presets_of(&plane, &locks),
        persona_hosts: persona_hosts_of(root, &plane, &locks),
        besides: sandbox::besides(root, &plane, &sandbox::Machine::this()).into(),
        policy: SandboxPolicy::of(&locks),
        manifest,
        certificate_check: plane.in_force(&locks).map(|policy| {
            CertificateCheck::of(sandbox::CertificateChecks::of(&policy, &locks), os)
        }),
        expects: sandbox::services::awaiting(root, &sandbox::Machine::this()).map(Into::into),
        team_off: TeamOffState::of(sandbox::team::team_off(root)),
        level_held: sandbox::team::allowed_levels(root).why,
        yours: yours_of(root, &locks),
    }
}

/// What Settings says of the project at `root`, read as `plane`, whose own manifest is missing
/// or cannot be read (#1411): every chat there is refused (D-1410e). None where it reads.
fn manifest_said(root: &std::path::Path, plane: &sandbox::Plane) -> Option<String> {
    let file = purlis_core::names::manifest_name(root);
    let why = if plane.missing() {
        format!("This project's {file} is missing")
    } else if plane.unreadable() {
        format!("This project's {file} cannot be read as TOML")
    } else {
        return None;
    };
    Some(format!(
        "{why}, so purlis cannot tell whether chats here run sandboxed. Chats here are refused \
         until the manifest is restored."
    ))
}

/// Each harness that never starts sandboxed on `os`, as "<title>: <why>".
fn never_here(os: sandbox::Os) -> Vec<String> {
    purlis_core::harness::Harness::ALL
        .into_iter()
        .filter_map(|harness| {
            sandbox::never_on(harness, os).map(|why| format!("{}: {why}", harness.title()))
        })
        .collect()
}

/// What the commands that read the project's sandbox name their work, off the window's thread.
pub(crate) const READING_SANDBOX: &str = "reading the project's sandbox";

/// What the commands that change this machine's answers about the project's sandbox name their
/// work, off the window's thread.
const ANSWERING_SANDBOX: &str = "keeping your answer about the project's sandbox";

/// **The person's answer to the project's sandbox off** (#2071, SO-8; #2156): Accept
/// (`accept`) or Keep it on for me, for the off as it stands; refused where no committed off
/// stands. Kept in this machine's config home, where no chat writes, until purlis sees the off
/// leave; it takes effect at each chat's next start. The window's alone.
#[tauri::command]
#[specta::specta]
pub async fn answer_team_sandbox_off(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    accept: bool,
) -> Result<SandboxState, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window(ANSWERING_SANDBOX, move || {
        let answer = sandbox::team::answering(&root, accept)?;
        sandbox::local::answer_team_off(&root, answer)
            .map_err(|why| format!("purlis could not keep your answer on this machine: {why}"))?;
        Ok(state_of(&root))
    })
    .await
}

/// The project's sandbox, for the offer notice and Settings.
///
/// Off the window's thread (#1949): it reads the project's manifests and the administrator's
/// policy, and where the project has a local settings file it asks git whether that file is
/// ignored, so it can take as long as git does.
#[tauri::command]
#[specta::specta]
pub async fn sandbox_state(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<SandboxState, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window(READING_SANDBOX, move || Ok(state_of(&root))).await
}

/// The person's answer to the one-time offer: `turn_on` writes `[sandbox] mode = "on"` into
/// the project's `charter.toml`; either answer is the last time it is asked on this machine.
#[tauri::command]
#[specta::specta]
pub async fn answer_sandbox_offer(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    turn_on: bool,
) -> Result<SandboxState, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window(ANSWERING_SANDBOX, move || answer(&root, turn_on)).await
}

fn answer(root: &std::path::Path, turn_on: bool) -> Result<SandboxState, String> {
    let answer = if turn_on {
        sandbox::local::Answer::TurnOn
    } else {
        sandbox::local::Answer::KeepItOff
    };
    sandbox::local::answer(root, answer).map_err(|err| err.to_string())?;
    Ok(state_of(root))
}

/// A handed-off chat that holds the asking chat's persona grants instead of its own (#1362,
/// D-1362-5), as its tab's Notice says it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct GrantsHeld {
    /// The chat that opened it by a handoff, by the name the person saw; `null` for a chat a
    /// Resume of a session record started.
    pub from: Option<String>,
    /// The persona it was opened as, whose hosts it cannot reach yet.
    pub persona: Option<String>,
    /// Where an administrator's policy forbids a persona's own hosts (#1343): why, naming the
    /// policy and who set it. Allowing them would reach nothing, so the Notice offers no Allow.
    pub locked: Option<String>,
    /// Whether the persona's hosts also wait for the person's Allow on this machine (D-1362-7):
    /// lifting the hold alone then reaches none of them.
    pub waits_here: bool,
}

/// Whether `persona`'s hosts in the project at `root` wait for the person's Allow on this
/// machine: it lists hosts a chat would reach, or tools, and they are not allowed as they stand.
/// **In a test of something else: no sandbox for this project's chats on this machine**, as a
/// person sets it in Settings, which a `Planes` keeping an event log records
/// ([`keeping_events_for_tests`]) (`[sandbox] level = "off"` in the local file, confirmed in this
/// machine's record). Codex and opencode run inside purlis's own wrap on macOS and Linux
/// (#1040), at the floor too, and a test about typing, a handoff or a relaunch starts its
/// stand-in harness without it.
#[cfg(test)]
pub(crate) fn no_sandbox_here_for_tests(root: &std::path::Path) {
    use purlis_core::sandbox::personal::{self, Choice};
    let local = root.join(purlis_core::profiles::LOCAL_FILE);
    let text = std::fs::read_to_string(&local).unwrap_or_default();
    // First in the file, so what a test later appends to its last table stays in that table.
    if !text.contains("[sandbox]") {
        std::fs::write(&local, format!("[sandbox]\nlevel = \"off\"\n\n{text}"))
            .expect("the local file");
    }
    personal::set_by_you(root, None, Some(Choice::Off)).expect("your level");
}

/// **In a test: `planes`, keeping an event log at `at`**, so a chat started without the
/// sandbox is started once that is recorded (ADR 0067 §7).
#[cfg(test)]
pub(crate) fn keeping_events_for_tests(
    planes: crate::planes::Planes,
    at: &std::path::Path,
) -> crate::planes::Planes {
    use purlis_core::eventlog::{ArgsKey, Log, Recorder};
    let events = crate::hooks::Events::here(Recorder::new(
        Log::open(at, "DEVICE").expect("a log"),
        ArgsKey::open(at).expect("a key"),
    ));
    planes.recording_events(Some(events))
}

pub(crate) fn persona_hosts_wait(root: &std::path::Path, persona: Option<&str>) -> bool {
    let Some(persona) = persona else {
        return false;
    };
    sandbox::persona::shown(root)
        .into_iter()
        .any(|one| one.persona == persona && one.asks() && !one.allowed())
}

/// Why a persona's own hosts reach no chat in the project at `root`, where policy says so.
pub(crate) fn persona_hosts_locked(root: &std::path::Path) -> Option<String> {
    let locks = sandbox::policy::Locks::of(root);
    locks.forbids_persona_hosts().then(|| {
        format!(
            "Policy forbids a persona's own hosts. {}",
            locks.locked_by()
        )
    })
}

/// Whether chat `session` holds the asking chat's persona grants instead of its own, and whose
/// (#1362): `null` for a chat that holds its own.
///
/// Off the window's thread (#1949): every pane asks it, and it reads the administrator's policy
/// and every persona's hosts.
#[tauri::command]
#[specta::specta]
pub async fn persona_grants_held(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<Option<GrantsHeld>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window(READING_SANDBOX, move || {
        // The chat's hold first, under the chats' lock alone; the files after, under none.
        let Some((from, persona)) = held.chats().grants_held(session) else {
            return Ok(None);
        };
        Ok(Some(GrantsHeld {
            waits_here: persona_hosts_wait(held.root(), persona.as_deref()),
            locked: persona_hosts_locked(held.root()),
            from,
            persona,
        }))
    })
    .await
}

/// The person allowed chat `session` its own persona's grants from its tab's Notice (#1362):
/// they apply from its next start, since a chat's sandbox is compiled as it starts.
#[tauri::command]
#[specta::specta]
pub async fn allow_persona_grants(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<bool, String> {
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    crate::off_the_window(ANSWERING_SANDBOX, move || {
        own_grants(
            session,
            held.chats()
                .grants_held(session)
                .map(|(_, persona)| persona),
            &held.audit_then_record(network.as_ref()),
            || held.chats().allow_own_grants(session),
        )
    })
    .await
}

/// [`allow_persona_grants`] for chat `session`, which holds another's grants as `holding` says
/// (the persona it was started as; `None` where it holds none): audited, then recorded, as
/// every Allow is (#1681), and only then taken (`take`). An audit that cannot be written takes
/// nothing. In the record's own words: the chat's persona's hosts, for this chat.
fn own_grants(
    session: u32,
    holding: Option<Option<String>>,
    audit: Audit<'_>,
    take: impl FnOnce() -> bool,
) -> Result<bool, String> {
    let Some(persona) = holding else {
        return Ok(false);
    };
    audit(
        Some(session),
        &sandbox::grant::Audited {
            granted: true,
            what: sandbox::local::PERSONA_HOSTS,
            target: persona.as_deref().unwrap_or("no persona"),
            level: sandbox::grant::Level::Chat,
        },
    )?;
    Ok(take())
}

/// The person read the Notice of the project's hosts as it showed them, `shown` (#1341): it is
/// not shown again until they change from that.
#[tauri::command]
#[specta::specta]
pub async fn acknowledge_project_hosts(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    shown: Vec<String>,
) -> Result<SandboxState, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window(ANSWERING_SANDBOX, move || acknowledge(&root, &shown)).await
}

/// **The person read the Notice of the project's browser sites** as it showed them, `shown`, the
/// sites it named as added (#2443): each still listed reaches every chat's browser here from its
/// next act. The window's alone (`WINDOW_ONLY`).
#[tauri::command]
#[specta::specta]
pub async fn acknowledge_project_browser_sites(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    shown: Vec<String>,
) -> Result<SandboxState, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window(ANSWERING_SANDBOX, move || {
        acknowledge_browser_sites(&root, &shown)
    })
    .await
}

fn acknowledge_browser_sites(
    root: &std::path::Path,
    shown: &[String],
) -> Result<SandboxState, String> {
    sandbox::browser_sites::acknowledge(root, shown).map_err(|err| err.to_string())?;
    Ok(state_of(root))
}

/// The person read the Notice of the project's Internet access presets as it showed them,
/// `shown` (#1385): it is not shown again until they change from that.
#[tauri::command]
#[specta::specta]
pub async fn acknowledge_project_presets(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    shown: Vec<String>,
) -> Result<SandboxState, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window(ANSWERING_SANDBOX, move || {
        acknowledge_presets(&root, &shown)
    })
    .await
}

fn acknowledge_presets(root: &std::path::Path, shown: &[String]) -> Result<SandboxState, String> {
    sandbox::local::acknowledge_presets(root, shown).map_err(|err| err.to_string())?;
    Ok(state_of(root))
}

fn acknowledge(root: &std::path::Path, shown: &[String]) -> Result<SandboxState, String> {
    sandbox::local::acknowledge_hosts(root, shown).map_err(|err| err.to_string())?;
    Ok(state_of(root))
}

/// **Allow on this machine**, on the Notice of what a project expects or in Settings › Sandbox
/// (ADR 0093, ruling 2): the person lets every chat of this project reach what its committed
/// file declares, of what the question showed (`shown`, each as the window drew it). Each is
/// judged as any grant is, audited, and kept in this machine's own file, never the project's:
/// the declaration itself grants nothing. One that cannot be allowed here now is left out and
/// said, and so is one that no longer stands as it was drawn. A chat takes it from its next start. The window's alone (`WINDOW_ONLY`): no link and
/// no chat makes it, so a chat can neither ask for it nor answer it.
#[tauri::command]
#[specta::specta]
pub async fn allow_project_expects(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    shown: Vec<ExpectedShown>,
) -> Result<SandboxState, String> {
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    crate::off_the_window(ANSWERING_SANDBOX, move || {
        let root = held.root().to_path_buf();
        allow_expects(
            &root,
            held.chats(),
            &shown,
            &held.audit_then_record(network.as_ref()),
            now_secs(),
        )?;
        Ok(state_of(&root))
    })
    .await
}

/// [`allow_project_expects`] in the project at `root`: only what is declared **and** was
/// shown **and** still waits **and** stands now as it was drawn ([`stands_as_shown`]) is kept,
/// for every chat of the project on this machine ([`kept_for`], for no chat). Refused whole
/// where policy forbids it. The answer is then recorded for what was shown and still stands
/// as shown, so the question is not asked again for what could not be allowed, and is asked
/// for an item declared since the question was drawn and for one that changed under it.
fn allow_expects(
    root: &std::path::Path,
    chats: &crate::chats::Chats,
    shown: &[ExpectedShown],
    audit: Audit<'_>,
    at: u64,
) -> Result<(), String> {
    allow_awaited(
        root,
        chats,
        sandbox::services::awaiting(root, &sandbox::Machine::this()),
        shown,
        audit,
        at,
    )
}

/// What is said where what a project expects no longer stands as the question drew it.
const EXPECTS_CHANGED: &str = "What this project expects changed on this machine since it was \
                               shown, so that was not allowed. It is shown again as it stands \
                               now.";

/// [`allow_expects`] of `awaiting`, what waits now.
fn allow_awaited(
    root: &std::path::Path,
    chats: &crate::chats::Chats,
    awaiting: Option<sandbox::services::Awaiting>,
    shown: &[ExpectedShown],
    audit: Audit<'_>,
    at: u64,
) -> Result<(), String> {
    use sandbox::grant::Level;
    let Some(awaiting) = awaiting else {
        return Err("Nothing this project expects waits to be allowed on this machine.".to_owned());
    };
    if let Some(locked) = &awaiting.locked {
        return Err(locked.clone());
    }
    let locks = sandbox::policy::Locks::of(root);
    let mut kept = 0;
    // Drawn as one thing and another now: a link that leads elsewhere since, or an item
    // drawn as one that cannot be allowed which now can. Never kept, and never answered.
    let changed = |item: &sandbox::services::Awaited| {
        !stands_as_shown(item, shown)
            && shown.iter().any(|one| {
                one.target == item.written
                    && match &one.named {
                        Some(named) => item.refused.is_some() || *named != item.what.target(),
                        None => item.refused.is_none(),
                    }
            })
    };
    for item in awaiting
        .items
        .iter()
        .filter(|item| stands_as_shown(item, shown))
    {
        if let Some(why) = locks.refuses_grant(&item.what, Level::You) {
            return Err(why);
        }
        kept_for(root, chats, None, (&item.what, Level::You), audit, at)?;
        kept += 1;
    }
    if kept == 0 {
        return Err(if awaiting.items.iter().any(changed) {
            EXPECTS_CHANGED.to_owned()
        } else {
            "Nothing shown can be allowed on this machine now, so nothing was allowed.".to_owned()
        });
    }
    // Each under the name it has now (#1927), which is the name it was drawn under: one that
    // changed is left out above.
    let answered: Vec<(String, String)> = awaiting
        .items
        .iter()
        .filter(|item| !changed(item) && shown.iter().any(|one| one.target == item.written))
        .map(sandbox::services::Awaited::as_named)
        .collect();
    sandbox::local::acknowledge_expects(root, &answered).map_err(|err| err.to_string())
}

/// **Not now**, on the Notice of what a project expects (ADR 0093): remembered on this
/// machine for the declaration as it was shown, `shown`, each item with the name it was drawn
/// under, so it is asked again only when the project declares something new, or when a path
/// it declared comes to name another socket (#1927). Nothing is allowed, and Settings ›
/// Sandbox still offers it. The window's alone (`WINDOW_ONLY`): a chat cannot answer the
/// question for the person.
#[tauri::command]
#[specta::specta]
pub async fn decline_project_expects(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    shown: Vec<ExpectedShown>,
) -> Result<SandboxState, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window(ANSWERING_SANDBOX, move || decline_expects(&root, &shown)).await
}

/// The longest name an answer records: far past any path or service name.
const MOST_NAME_ANSWERED: usize = 4096;

/// [`decline_project_expects`] in the project at `root`: only what the project declares now
/// is recorded, whatever the window sent, and **by the name it was drawn under** (the path as
/// written where it was drawn as one that cannot be allowed). The answer is to what the
/// person saw: an item that names something else by now is not answered by it, and is asked
/// about again as it stands.
fn decline_expects(
    root: &std::path::Path,
    shown: &[ExpectedShown],
) -> Result<SandboxState, String> {
    let declared = sandbox::services::awaiting(root, &sandbox::Machine::this())
        .map(|awaiting| awaiting.shown())
        .unwrap_or_default();
    let answered: Vec<(String, String)> = declared
        .into_iter()
        .filter_map(|written| {
            let drawn = shown.iter().find(|one| one.target == written)?;
            let named = drawn.named.clone().unwrap_or_else(|| written.clone());
            (named.len() <= MOST_NAME_ANSWERED).then_some((written, named))
        })
        .collect();
    sandbox::local::acknowledge_expects(root, &answered).map_err(|err| err.to_string())?;
    Ok(state_of(root))
}

/// **Allow** on a persona's hosts' Notice (#1362, D-1362-7): the person lets chats running as
/// `persona` reach its committed hosts on this machine, exactly as the Notice showed them,
/// `digest`. A list that changed since it was shown is refused, and nothing is kept. The window's
/// alone (`WINDOW_ONLY`): no link and no chat makes it.
#[tauri::command]
#[specta::specta]
pub async fn allow_persona_hosts(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    persona: String,
    digest: String,
) -> Result<SandboxState, String> {
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    crate::off_the_window(ANSWERING_SANDBOX, move || {
        let root = held.root().to_path_buf();
        allow_persona(
            &root,
            &persona,
            &digest,
            &held.audit_then_record(network.as_ref()),
            now_secs(),
        )?;
        Ok(state_of(&root))
    })
    .await
}

/// **A persona's hosts and tools on one line**, as its Allow is audited and its Granted row
/// says it: the hosts by commas, then each tool's whole line after a semicolon, since a tool
/// handed several values names them with commas and "and" of its own (T-1). Empty where it has
/// neither.
fn hosts_and_tools(hosts: &[sandbox::hosts::Host], tools: &[sandbox::tools::Tool]) -> String {
    let hosts: Vec<String> = hosts.iter().map(ToString::to_string).collect();
    std::iter::once(hosts.join(", "))
        .filter(|hosts| !hosts.is_empty())
        .chain(tools.iter().map(sandbox::tools::Tool::whole))
        .collect::<Vec<_>>()
        .join("; ")
}

/// [`allow_persona_hosts`] in the project at `root`: checked against the list as it stands,
/// audited, then kept with the digest of what was shown, and recorded for the Granted list.
/// Chats already running take it at their next start.
fn allow_persona(
    root: &std::path::Path,
    persona: &str,
    digest: &str,
    audit: Audit<'_>,
    at: u64,
) -> Result<(), String> {
    let shown = sandbox::persona::shown_now(root, persona, digest)?;
    audit(
        None,
        &sandbox::grant::Audited {
            granted: true,
            what: sandbox::local::PERSONA_HOSTS,
            target: &format!(
                "{persona}: {}{}",
                hosts_and_tools(&shown.hosts, &shown.tools),
                if shown.default {
                    " (the default persona: every chat that names no persona too)"
                } else {
                    ""
                }
            ),
            level: sandbox::grant::Level::You,
        },
    )?;
    sandbox::local::allow_persona_hosts(root, persona, &shown.digest)
        .map_err(|why| format!("purlis could not keep it, so nothing was allowed: {why}"))?;
    if let Err(why) = sandbox::local::record_made(
        root,
        sandbox::local::Made {
            what: sandbox::local::PERSONA_HOSTS.to_owned(),
            target: persona.to_owned(),
            level: sandbox::grant::Level::You.word().to_owned(),
            at,
            chat: None,
        },
    ) {
        // The Allow stands, and is audited; only the Granted list's "when" is lost.
        tracing::warn!("purlis: a persona's hosts were allowed without when ({why})");
    }
    Ok(())
}

/// The line [`type_sandbox_install`] types: SD-30's command for what `missing` names on the
/// distribution `os_release` says this is, **with no newline** — the person runs it.
fn install_line(missing: Option<&backend::Missing>, os_release: &str) -> Result<String, String> {
    missing
        .and_then(|missing| backend::install_command(missing, os_release))
        .ok_or_else(|| {
            "purlis has nothing to install for the sandbox on this machine, so nothing was \
             typed."
                .to_owned()
        })
}

/// Types SD-30's install command into shell session `session` at the project root, and does
/// not run it (ruling V78 c): installing needs `sudo`, so the person reads it and presses
/// Return. The window sends no text; the line is built here, and it is typed only into a shell
/// tab at the project root.
#[tauri::command]
#[specta::specta]
pub fn type_sandbox_install(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    // Into the operator's own shell at the project root, which the window opened for it, and
    // never into an agent's pane or a shell elsewhere.
    if !held.chats().is_shell_at(session, held.root()) {
        return Err(
            "that tab is not a shell at the project root, so the install command was not \
             typed."
                .to_owned(),
        );
    }
    let missing = backend::missing(sandbox::Os::this(), &backend::installed);
    let line = install_line(missing.as_ref(), &os_release())?;
    held.operator_input(session, line.as_bytes())
}

/// **A Report of a sandbox block of purlis's own** (#1338), as the window shows it before
/// anything is sent: the scrubbed draft `purlis report bug` would file, and its digest.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct BlockReport {
    /// The repository it would be filed on.
    pub repository: String,
    pub title: String,
    pub body: String,
    /// What filing names, so only exactly this draft is filed.
    pub digest: String,
}

/// The draft for a block of purlis's own `operation` on `kind`, from the chat running `harness`.
///
/// **Made from the fixed words alone.** The window hands back the two words the Notice was told,
/// and a word that is not one of [`purlis_core::sandboxblock`]'s is refused, so no text from the
/// window, or from the chat that sent the block, can reach the draft.
fn block_draft(
    operation: &str,
    kind: &str,
    harness: Option<&str>,
) -> Result<purlis_core::report::Draft, String> {
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let block = Block {
        operation: Operation::of_word(operation)
            .ok_or_else(|| "That is not an operation the sandbox reports.".to_owned())?,
        kind: Kind::of_word(kind)
            .ok_or_else(|| "That is not a kind of path or host the sandbox reports.".to_owned())?,
        ours: true,
    };
    let harness = harness.and_then(purlis_core::harness::Harness::of_kind);
    // The harness's version as this app last heard it answer, looked up by its kind here and
    // never taken from the window (#1353); the draft takes it only in a version's shape.
    let version = harness.and_then(purlis_core::sandbox::program::version_heard);
    purlis_core::report::Draft::of_sandbox_block(&block, harness, version.as_deref())
        .map_err(|refused| refused.0)
}

/// The draft of a Report for a sandbox block of purlis's own (#1338). Nothing is sent: this only
/// drafts, here, with no network.
#[tauri::command]
#[specta::specta]
pub fn sandbox_block_report(
    operation: String,
    kind: String,
    harness: Option<String>,
) -> Result<BlockReport, String> {
    let draft = block_draft(&operation, &kind, harness.as_deref())?;
    Ok(BlockReport {
        repository: purlis_core::report::UPSTREAM.to_owned(),
        digest: draft.digest(),
        title: draft.title,
        body: draft.body,
    })
}

/// Files the Report the window showed (#1338), on the person's press and never otherwise: by the
/// app, as the person, with purlis's own sign-in to GitHub and never `gh` or a token from the
/// environment ([`purlis_core::report::file`], #806), not by anything inside a chat's sandbox.
/// Only the draft whose digest is `digest` is filed; one that changed since it was shown is
/// refused unsent. Answers the new issue's address, or why it was not filed with the link that
/// files it in a browser.
///
/// **The window's alone** (`WINDOW_ONLY`): it files as the person, so no link makes it.
#[tauri::command]
#[specta::specta]
pub async fn file_sandbox_block_report(
    operation: String,
    kind: String,
    harness: Option<String>,
    digest: String,
) -> Result<String, String> {
    let draft = shown_draft(&operation, &kind, harness.as_deref(), &digest)?;
    tauri::async_runtime::spawn_blocking(move || {
        let caller = purlis_core::forge::Caller::window();
        purlis_core::report::as_the_reporter(&caller)
            .and_then(|tracker| purlis_core::report::file(tracker.as_ref(), &caller, &draft))
            .map_err(|why| {
                let (url, whole) = draft.fallback_url();
                format!(
                    "It was not filed: {why}. Open this link to file it in a browser{}: {url}",
                    if whole {
                        ""
                    } else {
                        ", and paste the body from the draft"
                    }
                )
            })
    })
    .await
    .map_err(|err| format!("The report did not finish: {err}"))?
}

/// The draft for a block, only if its digest is the one the window showed.
fn shown_draft(
    operation: &str,
    kind: &str,
    harness: Option<&str>,
    digest: &str,
) -> Result<purlis_core::report::Draft, String> {
    let draft = block_draft(operation, kind, harness)?;
    if draft.digest() != digest {
        return Err(
            "The draft is not the one that was shown, so nothing was sent. Open Report again."
                .to_owned(),
        );
    }
    Ok(draft)
}

// ---- a block's Allow, and every grant (#1342, #1348) -----------------------------------------

/// What a grant names, as the window sends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum GrantWhat {
    /// A host to reach.
    Host,
    /// A folder to write, and everything in it.
    Write,
    /// A vault a persona's chats may use although the registry does not tag it for the
    /// persona (#1430). Granted from a refused vault's Notice (`crate::vaultroute`), never from
    /// a block's Allow.
    Vault,
    /// A persona's committed hosts, which you allowed on this machine (#1362). Allowed from
    /// their own Notice (`allow_persona_hosts`), never from a block's Allow.
    PersonaHosts,
    /// A local socket to connect to (D-sock-1): this machine's alone, never the project's.
    Socket,
    /// A system service to look up by name (D-sock-1): this machine's alone.
    Service,
    /// A service on this Mac, by its one loopback port (qw384): this machine's alone.
    Port,
    /// A port of this Mac's loopback to listen on (#2019): this machine's alone, and a named
    /// local port of the chat's browser.
    Listen,
    /// Always for a persona's browser (#2241): what every chat as the persona may have its
    /// browser reach in this project on this machine. Allowed from a chat's browser Ask
    /// (`allow_chat_browser`), never from a block's Allow.
    Browser,
    /// A browser site (#2018, ruled 2026-10-17): a site a chat's browser may open, send and type
    /// on, for the browser alone, never the chat's own commands. Allowed from the chat's browser
    /// sites Ask (`allow_browser_sites`), never from a block's Allow.
    BrowserSite,
}

impl GrantWhat {
    /// **The grant of `port` this kind names** (#2019): a service on this Mac to connect to, or
    /// a port on this Mac to listen on. None for any other kind.
    pub(crate) fn of_port(self, port: u16) -> Option<sandbox::grant::What> {
        match self {
            Self::Port => Some(sandbox::grant::What::Port(port)),
            Self::Listen => Some(sandbox::grant::What::Listen(port)),
            _ => None,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Write => "write",
            Self::Vault => sandbox::local::VAULT,
            Self::PersonaHosts => sandbox::local::PERSONA_HOSTS,
            Self::Socket => "socket",
            Self::Service => "service",
            Self::Port => "port",
            Self::Listen => "listen",
            Self::Browser => sandbox::local::BROWSER,
            Self::BrowserSite => sandbox::local::BROWSER_SITE,
        }
    }

    fn of_word(word: &str) -> Option<Self> {
        [
            Self::Host,
            Self::Write,
            Self::Vault,
            Self::PersonaHosts,
            Self::Socket,
            Self::Service,
            Self::Port,
            Self::Listen,
            Self::Browser,
            Self::BrowserSite,
        ]
        .into_iter()
        .find(|what| what.word() == word)
    }
}

/// Who a grant is for, as the window sends it ([`sandbox::grant::Level`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum GrantLevel {
    /// This chat's next run alone (D-sock-3): a socket or a service only.
    Once,
    /// This chat, while the app holds it.
    Chat,
    /// Every chat of this project on this machine.
    You,
    /// Every chat of every project on this machine (R2, 2026-10-13): a socket or a service
    /// only.
    Machine,
    /// Everyone in the project: committed. A host only.
    Project,
}

impl From<GrantLevel> for sandbox::grant::Level {
    fn from(level: GrantLevel) -> Self {
        match level {
            GrantLevel::Once => Self::Once,
            GrantLevel::Chat => Self::Chat,
            GrantLevel::You => Self::You,
            GrantLevel::Machine => Self::Machine,
            GrantLevel::Project => Self::Project,
        }
    }
}

impl From<sandbox::grant::Level> for GrantLevel {
    fn from(level: sandbox::grant::Level) -> Self {
        match level {
            sandbox::grant::Level::Once => Self::Once,
            sandbox::grant::Level::Chat => Self::Chat,
            sandbox::grant::Level::You => Self::You,
            sandbox::grant::Level::Machine => Self::Machine,
            sandbox::grant::Level::Project => Self::Project,
        }
    }
}

/// What allowing a block answered (#1342): the sentence the Notice says. The chat is then owed
/// a restart on its conversation, which the window asks for once its turn has ended, unless
/// [`Self::live`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct Allowed {
    pub said: String,
    /// The chat took it at once, through its proxy's live ask (#1666): the command that asked
    /// carries on, and nothing restarts, so the window owes the chat no restart.
    pub live: bool,
}

/// Seconds since 1970, now.
pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Writes the audit of a grant or revoke, and answers whether it was written.
pub(crate) type Audit<'a> =
    &'a dyn Fn(Option<u32>, &sandbox::grant::Audited<'_>) -> Result<(), String>;

/// The project's file a host is kept in at `level`: yours or the committed one.
fn hosts_file(level: sandbox::grant::Level) -> purlis_core::settings::Which {
    if level == sandbox::grant::Level::Project {
        purlis_core::settings::Which::Shared
    } else {
        purlis_core::settings::Which::Local
    }
}

/// **Allows what chat `session`'s sandbox blocked** (#1342), at `level`, in the project at
/// `root` on `machine`: the core judges `target` again (the window's word and the chat's count
/// for nothing), the audit is written, then the grant is kept where its level keeps it and the
/// chat is owed a restart on its conversation. Never a class 1–3 path, never a folder off the
/// allowlist (D-1342-10), never a folder for the project, never past a policy lock.
fn allow(
    root: &std::path::Path,
    machine: &sandbox::Machine,
    chats: &crate::chats::Chats,
    session: u32,
    asked: (GrantWhat, &str, GrantLevel),
    audit: Audit<'_>,
    at: u64,
) -> Result<Allowed, NotJudged> {
    let (_, level, kept) = judged_and_kept(root, machine, chats, Some(session), asked, audit, at)?;
    Ok(Allowed {
        said: allowed_said(Some(session), level, &kept),
        live: kept.live.contains(&session),
    })
}

/// **The one path an Allow of what a sandbox refused is granted by** (#1342, #1970): `asked`
/// is [`judged`] by the core (never on the window's word), then audited and kept by
/// [`kept_for`], for chat `session` or for no chat. A block's Notice ([`allow`]) and Settings'
/// port form ([`allow_port`]) both come through here, so neither can grant what the other
/// refuses.
pub(crate) fn judged_and_kept(
    root: &std::path::Path,
    machine: &sandbox::Machine,
    chats: &crate::chats::Chats,
    session: Option<u32>,
    asked: (GrantWhat, &str, GrantLevel),
    audit: Audit<'_>,
    at: u64,
) -> Result<(sandbox::grant::What, sandbox::grant::Level, Kept), NotJudged> {
    let folder = session.and_then(|session| chats.folder_of(session));
    let (what, level) = judged(
        root,
        machine,
        folder.as_deref(),
        session.unwrap_or(0),
        asked,
    )?;
    let kept = kept_for(root, chats, session, (&what, level), audit, at)?;
    Ok((what, level, kept))
}

/// **A service on this Mac added by its port on Settings' Network page** (#1970): `port` as
/// typed, for open chat `chat` alone ([`GrantLevel::Chat`]) or for every chat of the project at
/// `root` on this machine ([`GrantLevel::You`], where `chat` is not read). Granted through
/// [`judged_and_kept`] exactly as a block's Notice grants a port, so a range, every port,
/// purlis's own port, any other scope and a policy lock are refused in the Notice's own words,
/// and nothing refused is audited or kept. For one chat, the core also checks, before anything
/// is judged or audited, that it names a chat open in this project (`chats` is this project's
/// alone) that runs a harness, never a shell. That chat then restarts on its conversation once
/// its turn ends, as after its Notice; for this project on this machine, a chat that is running
/// takes it from its next start.
pub(crate) fn allow_port(
    root: &std::path::Path,
    machine: &sandbox::Machine,
    chats: &crate::chats::Chats,
    chat: Option<u32>,
    (port, level): (&str, GrantLevel),
    audit: Audit<'_>,
    at: u64,
) -> Result<Allowed, NotJudged> {
    let session = match (level, chat) {
        (GrantLevel::Chat, None) => {
            return refused(format!(
                "purlis did not allow port {}: no open chat was named.",
                port.trim()
            ));
        }
        (GrantLevel::Chat, Some(chat)) if chats.harness(chat).is_none() => {
            return refused(format!(
                "purlis did not allow port {}: chat {chat} is not a chat open in this project.",
                port.trim()
            ));
        }
        (GrantLevel::Chat, Some(chat)) => Some(chat),
        _ => None,
    };
    let (_, level, kept) = judged_and_kept(
        root,
        machine,
        chats,
        session,
        (GrantWhat::Port, port, level),
        audit,
        at,
    )?;
    Ok(Allowed {
        said: allowed_said(session, level, &kept),
        live: session.is_some_and(|session| kept.live.contains(&session)),
    })
}

/// **What allowing `target` for chat `session` would grant**, judged by the core and never
/// taken from the window's word: a host by the project's own hosts' rules, a folder against
/// the allowlist from `folder`, the folder the chat was started in (none for a chat that is not
/// open), and either past an administrator's policy (#1343). Nothing is kept or audited here.
pub(crate) fn judged(
    root: &std::path::Path,
    machine: &sandbox::Machine,
    folder: Option<&std::path::Path>,
    session: u32,
    (what, target, level): (GrantWhat, &str, GrantLevel),
) -> Result<(sandbox::grant::What, sandbox::grant::Level), NotJudged> {
    use sandbox::grant::{self, Level, What};
    let level = Level::from(level);
    if level == Level::Project && matches!(what, GrantWhat::Socket | GrantWhat::Service) {
        return refused(
            "purlis will not allow a local socket or a system service for everyone in the \
             project: it is this machine's, and a committed file that granted one could grant \
             itself to every chat. Allow it for this chat, or for every chat of this project on \
             this machine."
                .to_owned(),
        );
    }
    // A service on this Mac (qw384): for this chat, or for every chat of this project on this
    // machine. A port is this machine's, never the project's, and lasts past one run.
    if what == GrantWhat::Port && !matches!(level, Level::Chat | Level::You) {
        return refused(
            "purlis allows a service on this Mac for this chat, or for every chat of this \
             project on this machine."
                .to_owned(),
        );
    }
    // A port to listen on (#2019): the same two scopes, for the same reason.
    if what == GrantWhat::Listen && !matches!(level, Level::Chat | Level::You) {
        return refused(
            "purlis allows a server on a port for this chat, or for every chat of this project \
             on this machine."
                .to_owned(),
        );
    }
    if level == Level::Once && matches!(what, GrantWhat::Host | GrantWhat::Write) {
        return refused(
            "purlis allows a host or a folder for this chat, or for longer, not once.".to_owned(),
        );
    }
    if level == Level::Project && what == GrantWhat::Write {
        return refused(
            "purlis will not allow a folder for everyone in the project: a folder is a path on \
             this machine. Allow it for this chat, or for every chat of this project on this \
             machine."
                .to_owned(),
        );
    }
    let what = match what {
        GrantWhat::Vault => {
            return refused(
                "purlis allows a vault from the notice its refusal raises, not from a block's."
                    .to_owned(),
            );
        }
        GrantWhat::PersonaHosts => {
            return refused(
                "purlis allows a persona's hosts from the notice that shows them, not from a \
                 block's."
                    .to_owned(),
            );
        }
        GrantWhat::Browser => {
            return refused(
                "purlis allows a persona's browser from its chat's browser ask, not from a \
                 block's."
                    .to_owned(),
            );
        }
        GrantWhat::BrowserSite => {
            return refused(
                "purlis allows a browser site from its chat's browser sites ask, not from a \
                 block's."
                    .to_owned(),
            );
        }
        GrantWhat::Host => What::Host(grant::host(target).map_err(|why| why.to_string())?),
        GrantWhat::Write => {
            let folder = folder.ok_or_else(|| {
                format!("purlis did not allow anything for chat {session}: it is not open.")
            })?;
            let ground = grant::Ground::of(root, folder, machine);
            What::Write(grant::write(target, &ground.place()).map_err(|why| why.to_string())?)
        }
        // Only the socket the Notice drew (#1927): one whose path leads elsewhere by now is
        // never kept, whichever Notice or list the answer came from.
        GrantWhat::Socket => {
            let ground = sandbox::services::SocketGround::of(root, machine, &[]);
            let now = sandbox::services::socket(target, &ground).map_err(|why| why.to_string())?;
            What::Socket(socket_as_drawn(target, now)?)
        }
        GrantWhat::Service => {
            let name = sandbox::services::service(target).map_err(|why| why.to_string())?;
            // Never a grant (R1): the project's setting and policy alone decide it.
            if name == sandbox::services::TRUSTD {
                return refused(
                    "Certificate checks are a setting of the project's, on unless it turns them \
                     off, so there is nothing to allow: see Settings › Sandbox."
                        .to_owned(),
                );
            }
            What::Service(name)
        }
        GrantWhat::Port => {
            What::Port(sandbox::localport::port(target).map_err(|why| why.to_string())?)
        }
        // A port to serve on (#2019): from 1024, and none something on this Mac listens on.
        GrantWhat::Listen => {
            What::Listen(sandbox::localport::serve_port(target).map_err(|why| why.to_string())?)
        }
    };
    // Policy (#1343): the strictest wins, for a host at the level it would be kept at, and for
    // any folder at all.
    if let Some(why) = sandbox::policy::Locks::of(root).refuses_grant(&what, level) {
        return refused(why);
    }
    Ok((what, level))
}

/// What an Allow says where a socket is no longer the one its Notice drew (#1927). purlis's
/// own sentence, whole: it names no path.
pub(crate) const SOCKET_CHANGED: &str = "That socket is not the one this Notice showed any more: \
                                         its path leads somewhere else now, so nothing was \
                                         allowed. It is shown again as it stands now.";

/// **Why an Allow was not judged as one to keep**: its socket is no longer the one its Notice
/// drew ([`socket_as_drawn`]), which the caller answers by telling the block again, or the
/// core's refusal in its own words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NotJudged {
    SocketChanged,
    Refused(String),
}

impl std::fmt::Display for NotJudged {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SocketChanged => f.write_str(SOCKET_CHANGED),
            Self::Refused(why) => f.write_str(why),
        }
    }
}

impl From<String> for NotJudged {
    fn from(why: String) -> Self {
        Self::Refused(why)
    }
}

impl From<NotJudged> for String {
    fn from(not: NotJudged) -> Self {
        not.to_string()
    }
}

/// `why`, as a refusal of [`judged`]'s.
fn refused<T>(why: String) -> Result<T, NotJudged> {
    Err(NotJudged::Refused(why))
}

/// **The socket an Allow of `drawn` keeps**: `now`, what the drawn path is judged to name at
/// the press ([`sandbox::services::socket`]), only while that is still the name that was
/// drawn (#1927). A block's Notice draws its socket by the name the kernel gave it when the
/// Notice was told, and may be answered much later; a link in that path can lead to another
/// socket by then. So an Allow keeps the socket only as it was drawn, as the up-front Allow of
/// what a project expects does ([`stands_as_shown`]). One that changed is never kept under
/// the name the person read.
fn socket_as_drawn(drawn: &str, now: std::path::PathBuf) -> Result<std::path::PathBuf, NotJudged> {
    if now == std::path::Path::new(drawn) {
        Ok(now)
    } else {
        Err(NotJudged::SocketChanged)
    }
}

/// **Tells the window chat `session`'s block `was` again, as it stands now** (#1927), after
/// an Allow found its socket changed: worded, judged and offered by the function that tells
/// any block (`crate::hooks::blocked_on`), from the path that was drawn. The chat is held on
/// the block as told now and no longer on `was`, so the Notice the person reads next is the
/// only one an answer matches. None where `was` is not a block the sandbox reports.
pub(crate) fn shown_again(
    plane: &PlaneId,
    chats: &crate::chats::Chats,
    session: u32,
    was: &crate::taskblocks::HeldBlock,
) -> Option<crate::hooks::ChatBlocked> {
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let block = Block {
        operation: Operation::of_word(&was.operation)?,
        kind: Kind::of_word(&was.kind)?,
        ours: false,
    };
    // The chat's harness by the app's own record of what it started, never a word it sent.
    let told = crate::hooks::blocked_on(
        plane,
        session,
        &block,
        chats.harness(session),
        Some(&was.target),
        chats.has_certificate_check(session),
    );
    chats.blocks().answered(session, was);
    crate::taskblocks::heard(chats, &told);
    Some(told)
}

/// What keeping a grant came to (#1666).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Kept {
    /// The open chats that took it at once, through their proxy's live asks: a held connection
    /// of theirs goes on, and none of them is owed a restart for it.
    pub live: Vec<u32>,
    /// Those of them whose held connection had given up before the Allow: told to retry.
    pub retry: Vec<u32>,
    /// It was allowed already, at this scope or everyone's: nothing new was kept or audited.
    pub already: bool,
}

/// **Keeps a grant [`judged`] made for chat `session`**: audited, then kept where `level`
/// keeps it, and the chat owed a restart on its conversation unless its proxy took it live
/// (#1666). For the chat alone it is reserved first, so a chat that is not open is refused
/// before anything is audited, and kept only once audited (#2454). An audit that cannot be
/// written keeps nothing.
pub(crate) fn kept(
    root: &std::path::Path,
    chats: &crate::chats::Chats,
    session: u32,
    grant: (&sandbox::grant::What, sandbox::grant::Level),
    audit: Audit<'_>,
    at: u64,
) -> Result<Kept, String> {
    kept_for(root, chats, Some(session), grant, audit, at)
}

/// [`kept`] for chat `session`, or for no chat (#1662, Allow on Settings' Blocked lately): then
/// nothing is kept for one chat alone, and no chat is owed a restart, so each takes it from its
/// next start.
///
/// **A host is taken live** (#1666): every open chat it reaches whose proxy asks live takes it
/// at once ([`crate::chats::Chats::allow_live`]), and only a chat that does not is owed the
/// restart. **A host allowed already** at this scope, or for everyone, is not kept twice: no
/// write, no audit, and the answer says so (#1666's fold-in), where it used to be refused with
/// "already listed" and the Notice stayed up.
pub(crate) fn kept_for(
    root: &std::path::Path,
    chats: &crate::chats::Chats,
    session: Option<u32>,
    (what, level): (&sandbox::grant::What, sandbox::grant::Level),
    audit: Audit<'_>,
    at: u64,
) -> Result<Kept, String> {
    use sandbox::grant::{self, Level, What};
    let target = what.target();
    if session.is_none() && level.one_chat() {
        return Err(
            "purlis allows a host for one chat from that chat's own Notice, not from Blocked \
             lately."
                .to_owned(),
        );
    }
    // Never the system's name lookup (#2657), whichever path the grant came by.
    if let Some(why) = sandbox::services::name_lookup_refused(what) {
        return Err(why.to_owned());
    }
    // Every project on this machine is a socket's or a service's alone (R2), whoever asks.
    if level == Level::Machine && !what.machine_wide() {
        return Err(
            "purlis allows only a local socket or a system service for every project on this \
             machine: a host and a folder are a project's."
                .to_owned(),
        );
    }
    // **For one chat: reserved, audited, then kept** (#2454). Reserving checks the chat is
    // open, so a chat that is not is refused before anything is audited; it keeps nothing, so
    // no start, Granted list or live ask has the grant until its audit is written; and a grant
    // whose audit cannot be written is never kept. Once made lasting is a new grant, audited.
    let reserved = match (level, session) {
        (Level::Once | Level::Chat, Some(session)) => {
            Some(chats.reserve_grant(session, what.clone(), at, level == Level::Once)?)
        }
        _ => None,
    };
    let already = match (what, level, session) {
        (What::Host(host), Level::You | Level::Project, _) => grant::allowed_already(root, host)
            .is_some_and(|held| held == level || held == Level::Project),
        (_, Level::Once | Level::Chat, Some(_)) => reserved
            .as_ref()
            .is_some_and(crate::chats::ReservedGrant::already),
        (What::Socket(socket), Level::You, _) => {
            sandbox::local::granted_sockets(root).contains(socket)
        }
        (What::Service(service), Level::You, _) => {
            sandbox::local::granted_services(root).contains(service)
        }
        (What::Socket(socket), Level::Machine, _) => {
            sandbox::local::sockets_everywhere().contains(socket)
        }
        (What::Service(service), Level::Machine, _) => {
            sandbox::local::services_everywhere().contains(service)
        }
        (What::Port(port), Level::You, _) => sandbox::local::granted_ports(root).contains(port),
        (What::Listen(port), Level::You, _) => sandbox::local::granted_listens(root).contains(port),
        _ => false,
    };
    if !already {
        audit(
            session,
            &grant::Audited {
                granted: true,
                what: what.word(),
                target: &target,
                level,
            },
        )?;
    }
    // Kept once audited. Where the chat closed, or a grant of it was taken back, while the
    // audit was written, nothing is kept, and the audit is told the grant it records ended.
    if let Some(reserved) = reserved.filter(|reserved| !reserved.already())
        && let Err(why) = chats.commit_grant(reserved)
    {
        let ended = audit(
            session,
            &grant::Audited {
                granted: false,
                what: what.word(),
                target: &target,
                level,
            },
        );
        return Err(match ended {
            Ok(()) => why,
            Err(unsaid) => format!(
                "{why}. The audit records it allowed, and could not record that it was not \
                 kept ({unsaid}): nothing was allowed."
            ),
        });
    }
    let told = grant::told(what, level);
    // A Claude Code chat reaches a service on this Mac by name alone (qw384b): told which.
    let told = match (what, session.and_then(|session| chats.harness(session))) {
        (What::Port(port), Some(purlis_core::harness::Harness::ClaudeCode)) => {
            format!("{told} {}", sandbox::localport::reached_by_name(*port))
        }
        _ => told,
    };
    match (what, level, session) {
        // Kept above, once audited.
        (_, Level::Once | Level::Chat, _) => {}
        // For every project on this machine (R2): this machine's one file, with when it was
        // allowed, never a project's or a clone's.
        (What::Socket(socket), Level::Machine, _) => {
            sandbox::local::grant_socket_everywhere(socket, everywhere_made(what, already, at))
                .map_err(|why| format!("purlis could not keep {}: {why}", socket.display()))?;
        }
        (What::Service(service), Level::Machine, _) => {
            sandbox::local::grant_service_everywhere(service, everywhere_made(what, already, at))
                .map_err(|why| format!("purlis could not keep {service}: {why}"))?;
        }
        (What::Socket(socket), _, _) => {
            sandbox::local::grant_socket(root, socket)
                .map_err(|why| format!("purlis could not keep {}: {why}", socket.display()))?;
        }
        (What::Service(service), _, _) => {
            sandbox::local::grant_service(root, service)
                .map_err(|why| format!("purlis could not keep {service}: {why}"))?;
        }
        (What::Port(port), _, _) => {
            sandbox::local::grant_port(root, *port)
                .map_err(|why| format!("purlis could not keep port {port}: {why}"))?;
        }
        (What::Listen(port), _, _) => {
            sandbox::local::grant_listen(root, *port)
                .map_err(|why| format!("purlis could not keep port {port}: {why}"))?;
        }
        (What::Host(host), Level::You | Level::Project, _) => {
            if !already {
                purlis_core::settings::hosts::grant(root, hosts_file(level), host)?;
            }
        }
        // Refused above: every project on this machine is never a host's.
        (What::Host(_), Level::Machine, _) => {}
        (What::Write(folder), _, _) => {
            sandbox::local::grant_write(root, folder)
                .map_err(|why| format!("purlis could not keep {}: {why}", folder.display()))?;
        }
    }
    // Taken live where a chat's proxy asks live; owed a restart where it does not.
    let crate::chats::Live {
        reached: live,
        told_to_retry: retry,
    } = match what {
        What::Host(host) => chats.allow_live(session, host, level),
        What::Write(_) | What::Socket(_) | What::Service(_) | What::Port(_) | What::Listen(_) => {
            crate::chats::Live::default()
        }
    };
    if let Some(session) = session
        && !live.contains(&session)
    {
        chats.owe_restart(session, told);
    }
    if !matches!(level, Level::Once | Level::Chat | Level::Machine)
        && !already
        && let Err(why) = sandbox::local::record_made(
            root,
            sandbox::local::Made {
                what: what.word().to_owned(),
                target,
                level: level.word().to_owned(),
                at,
                chat: None,
            },
        )
    {
        // The grant stands, and is audited; only the Granted list's "when" is lost.
        tracing::warn!("purlis: a sandbox grant was kept without when it was made ({why})");
    }
    Ok(Kept {
        live,
        retry,
        already,
    })
}

/// The record of a grant for every project on this machine, where it is newly made.
fn everywhere_made(
    what: &sandbox::grant::What,
    already: bool,
    at: u64,
) -> Option<sandbox::local::Made> {
    (!already).then(|| sandbox::local::Made {
        what: what.word().to_owned(),
        target: what.target(),
        level: sandbox::grant::Level::Machine.word().to_owned(),
        at,
        chat: None,
    })
}

/// What an Allow says once [`kept`] came to `kept` at `level`: on chat `session`'s Notice, or
/// for no chat (#1970, Settings' port form), where a chat that is running takes it from its
/// next start.
pub(crate) fn allowed_said(
    session: Option<u32>,
    level: sandbox::grant::Level,
    kept: &Kept,
) -> String {
    let Some(session) = session else {
        return if kept.already {
            format!("It was allowed already, {}.", level.said())
        } else {
            format!(
                "Allowed {}. A chat that is running reaches it from its next start.",
                level.said()
            )
        };
    };
    let retry = kept.retry.contains(&session);
    match (kept.already, kept.live.contains(&session)) {
        (true, true) => format!(
            "It was allowed already, {}. {}",
            level.said(),
            if retry {
                "The command that asked had given up waiting, so the chat is told to run it \
                 again; nothing restarts."
            } else {
                "The command that asked carries on now; nothing restarts."
            }
        ),
        (false, true) if retry => format!(
            "Allowed {}. The command that asked had given up waiting, so the chat is told to \
             run it again; nothing restarts.",
            level.said()
        ),
        (false, true) => format!(
            "Allowed {}. The command that asked carries on now; nothing restarts.",
            level.said()
        ),
        (true, false) => format!(
            "It was allowed already, {}. This chat started before that, so it restarts on the \
             same conversation once its turn ends to take it.",
            level.said()
        ),
        (false, false) => format!(
            // No target in the sentence: it is the chat's choice, and the window shows it apart.
            "Allowed {}. The chat restarts on the same conversation once its turn ends, and is \
             told to retry.",
            level.said()
        ),
    }
}

/// **Allow** on a block's Notice (#1342): `shown` is the block and the host or folder the
/// Notice showed whole (or the host the person typed, where the block named none). Refused
/// whole unless the chat is held on that block now (#1538, [`crate::taskblocks::shown_one`]). The window then restarts the chat once its turn has ended
/// (`restart_chat`).
///
/// Off the window's thread (#1949): judging a grant reads the project's files and this
/// machine's, and an Allow is audited and kept on disk before it is answered.
#[tauri::command]
#[specta::specta]
pub async fn allow_sandbox_block(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    shown: crate::taskblocks::BlockShown,
    level: GrantLevel,
) -> Result<Allowed, String> {
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    let tell = planes.block_teller();
    crate::off_the_window(ANSWERING_SANDBOX, move || {
        let block = crate::taskblocks::shown_one(held.chats().blocks(), session, &shown)?;
        let root = held.root().to_path_buf();
        let allowed = match allow_held(
            &root,
            &sandbox::Machine::this(),
            held.chats(),
            (session, &block),
            level,
            &held.audit_then_record(network.as_ref()),
            now_secs(),
        ) {
            // Its socket is not the one the Notice drew any more (#1927): nothing was kept,
            // and the Notice is told the block again as it stands, so what is read next is what
            // an answer would keep.
            Err(NotJudged::SocketChanged) => {
                if let Some(told) = shown_again(&plane, held.chats(), session, &block) {
                    tell(told);
                }
                return Err(SOCKET_CHANGED.to_owned());
            }
            answered => answered?,
        };
        // Answered: neither this Notice nor a question for several tasks answers it again.
        held.chats().blocks().answered(session, &block);
        Ok(allowed)
    })
    .await
}

/// **Allows `block`, which chat `session` is held on, at `level`** (#1342), in the project at
/// `root`: a service of the runner the chat runs on for this chat alone, there, with nothing
/// kept on this machine ([`allow_on_its_runner`], #2510); anything else as [`allow`] grants it.
pub(crate) fn allow_held(
    root: &std::path::Path,
    machine: &sandbox::Machine,
    chats: &crate::chats::Chats,
    (session, block): (u32, &crate::taskblocks::HeldBlock),
    level: GrantLevel,
    audit: Audit<'_>,
    at: u64,
) -> Result<Allowed, NotJudged> {
    if block.kind == purlis_core::sandboxblock::Kind::RunnerService.word() {
        return Ok(allow_on_its_runner(
            chats,
            session,
            (&block.target, level),
            audit,
        )?);
    }
    allow(
        root,
        machine,
        chats,
        session,
        (block.what, &block.target, level),
        audit,
        at,
    )
}

/// **Allows the service on `port` of the runner chat `session` runs on, which that runner asked
/// about** (#2510, ruled 2026-10-18): once or for this chat, never wider, for that chat alone
/// while it runs there. The Allow is audited, and no grant is kept on this machine, so the
/// chat's next start plans it as before and the runner asks again. Refused, with nothing done,
/// at any other level, and where the chat's runner did not leave that port out.
pub(crate) fn allow_on_its_runner(
    chats: &crate::chats::Chats,
    session: u32,
    (port, level): (&str, GrantLevel),
    audit: Audit<'_>,
) -> Result<Allowed, String> {
    allowed_on_its_runner(
        session,
        (port, level),
        audit,
        &|port| chats.runner_asks_about(session, port),
        &|port| chats.allow_left_out_live(session, port),
    )
}

/// [`allow_on_its_runner`], where `asks` says whether the chat's runner asks about a port and
/// `reach` tells it the Allow. An Allow the runner did not take is audited as taken back, and
/// refused in a sentence, so the audit never holds an Allow that did not reach the runner.
pub(crate) fn allowed_on_its_runner(
    session: u32,
    (port, level): (&str, GrantLevel),
    audit: Audit<'_>,
    asks: &dyn Fn(u16) -> bool,
    reach: &dyn Fn(u16) -> Result<crate::chats::Live, String>,
) -> Result<Allowed, String> {
    let level = sandbox::grant::Level::from(level);
    if !matches!(
        level,
        sandbox::grant::Level::Once | sandbox::grant::Level::Chat
    ) {
        return Err(RUNNER_SERVICE_SCOPES.to_owned());
    }
    let port: u16 = port
        .trim()
        .parse()
        .map_err(|_| format!("purlis did not allow {port}: it is not a port."))?;
    if !asks(port) {
        return Err(format!(
            "purlis did not allow port {port}: the runner chat {session} runs on does not ask \
             about it now."
        ));
    }
    let target = port.to_string();
    audit(
        Some(session),
        &sandbox::grant::Audited {
            granted: true,
            what: "port",
            target: &target,
            level,
        },
    )?;
    let live = match reach(port) {
        Ok(live) => live,
        Err(why) => {
            let undone = audit(
                Some(session),
                &sandbox::grant::Audited {
                    granted: false,
                    what: "port",
                    target: &target,
                    level,
                },
            );
            return Err(match undone {
                Ok(()) => {
                    format!("purlis did not allow port {port}: the runner did not take it ({why}).")
                }
                Err(unsaid) => format!(
                    "purlis did not allow port {port}: the runner did not take it ({why}). The \
                     audit records it allowed, and could not record that it was not ({unsaid})."
                ),
            });
        }
    };
    let kept = Kept {
        retry: live.told_to_retry,
        live: live.reached,
        already: false,
    };
    Ok(Allowed {
        said: allowed_said(Some(session), level, &kept),
        live: true,
    })
}

/// Why a service of a runner is not allowed past one chat (#2510).
pub(crate) const RUNNER_SERVICE_SCOPES: &str = "purlis allows a service on the runner a chat \
                                                runs on once or for this chat alone, never for \
                                                more chats or on this Mac.";

/// **Allow port** on Settings' Network page (#1970): the service on this Mac at `port`, for open
/// chat `chat` alone or for every chat of this project on this machine, granted by
/// [`allow_port`] through the path a block's Notice takes. The window's alone (`WINDOW_ONLY`).
///
/// Off the window's thread (#1949): it is judged, audited and kept on disk before it answers.
#[tauri::command]
#[specta::specta]
pub async fn allow_sandbox_port(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    port: String,
    chat: Option<u32>,
    level: GrantLevel,
) -> Result<Allowed, String> {
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    crate::off_the_window(ANSWERING_SANDBOX, move || {
        allow_port(
            held.root(),
            &sandbox::Machine::this(),
            held.chats(),
            chat,
            (&port, level),
            &held.audit_then_record(network.as_ref()),
            now_secs(),
        )
        .map_err(String::from)
    })
    .await
}

/// **Keep blocked** on a block's Notice (#1666, taking over #1411's line): `shown` is the block
/// the Notice showed, `reason` what the person typed for the chat, if anything. Nothing is
/// done where the chat is not held on that block now (#1538): it was answered already.
/// Nothing is granted. For a host, what the chat's
/// proxy holds on it is refused now; for every kind, the chat is told in one line not to retry,
/// with the reason (#1987).
#[tauri::command]
#[specta::specta]
pub fn keep_sandbox_block(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    shown: crate::taskblocks::BlockShown,
    reason: Option<String>,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    // Answered already, by another press or another window: nothing is left to keep blocked,
    // and the chat was told by that answer (#1987).
    let audit = held.audit_then_record(planes.network());
    let Some(block) = keep_block(held.chats(), session, &shown, reason.as_deref(), &audit) else {
        return Ok(());
    };
    // The person's no, kept where the Inbox's look back reads it (#1700).
    if block.what == GrantWhat::Host {
        crate::network::record_kept_blocked(
            planes.network(),
            held.root(),
            held.chats(),
            session,
            &block.target,
        );
    }
    Ok(())
}

/// [`keep_sandbox_block`] on `chats`: the block it answered, or none where the chat is not held
/// on it any more, and then nothing is done and the chat is told nothing.
pub(crate) fn keep_block(
    chats: &crate::chats::Chats,
    session: u32,
    shown: &crate::taskblocks::BlockShown,
    reason: Option<&str>,
    audit: Audit<'_>,
) -> Option<crate::taskblocks::HeldBlock> {
    let block = crate::taskblocks::shown_one(chats.blocks(), session, shown).ok()?;
    chats.blocks().answered(session, &block);
    if block.what == GrantWhat::Host
        && let Ok(host) = sandbox::grant::host(&block.target)
    {
        chats.keep_blocked_live(session, &host, audit);
    }
    // A service of the runner the chat runs on (#2510): its hold ends at once.
    if block.kind == purlis_core::sandboxblock::Kind::RunnerService.word()
        && let Ok(port) = block.target.parse::<u16>()
    {
        chats.keep_left_out_blocked_live(session, port);
    }
    crate::askoutcome::kept_blocked(chats, session, (block.what, &shown.target), reason);
    Some(block)
}

/// The chats of this project owed a restart (#1342, #1428): to take a grant, or because the
/// person asked. For the window that drives it once each one's turn has ended.
#[tauri::command]
#[specta::specta]
pub fn owed_restarts(planes: tauri::State<'_, Planes>, plane: PlaneId) -> Result<Vec<u32>, String> {
    Ok(planes.held(&plane)?.chats().owed_restarts())
}

/// One grant, as Settings' Granted list shows it (#1348).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SandboxGrant {
    /// What Revoke is sent by.
    pub id: String,
    pub what: GrantWhat,
    /// The host, the folder, the vault's name, or a persona's hosts.
    pub target: String,
    /// For a vault, the persona whose chats may use it; for a persona's hosts, that persona.
    pub persona: Option<String>,
    pub level: GrantLevel,
    /// Who committed it, for one the project carries; null for one you granted, or one the
    /// project's file holds that is not committed yet.
    pub by: Option<String>,
    /// When, in seconds since 1970, where that is known.
    pub at: Option<u32>,
    /// The chat it was granted from, where it came from one.
    pub chat: Option<String>,
    /// Why an administrator's policy locks it out, where one does: it is kept, and reaches or
    /// writes nothing while the policy stands. The list marks it "Locked by policy".
    pub locked: Option<String>,
    /// For a persona's hosts you allowed whose list has changed since: why it grants nothing.
    /// It stays listed, with Revoke, and never returns to force unless allowed anew.
    pub waiting: Option<String>,
    /// For a persona's hosts: the persona is the project's default, so they reach every chat
    /// that names no persona too.
    pub for_no_persona: bool,
    /// For a socket or a service: the project's committed file declares it expects it
    /// (D-sock-4). Said beside the grant; the declaration itself grants nothing.
    pub expected: bool,
    /// For a persona's hosts and tools: one sentence for each of its tools whose program is a
    /// general-purpose one (T-4); `null` where none is.
    #[specta(optional)]
    pub cautions: Option<Vec<String>>,
}

/// Who last committed a line naming each of `hosts` in the project's committed file, and when:
/// the project's history, asked of git **once for them all** (#1464,
/// [`purlis_core::committedby`]), through the hardened runner. `None` for one git has nothing
/// to say of (not committed yet).
fn committed_by(
    root: &std::path::Path,
    hosts: &[String],
) -> Vec<Option<purlis_core::committedby::Committed>> {
    let quoted: Vec<String> = hosts.iter().map(|host| format!("\"{host}\"")).collect();
    let wanted: Vec<_> = quoted
        .iter()
        .map(|host| move |line: &str| line.contains(host.as_str()))
        .collect();
    purlis_core::committedby::last_touching(root, &wanted)
}

/// Whether this machine still keeps `port` for every chat of the project at `root`, to connect
/// to or to listen on (#2019): then removing one of the two withdraws it from no chat.
fn held_for_the_project(root: &std::path::Path, port: u16) -> bool {
    sandbox::local::granted_ports(root).contains(&port)
        || sandbox::local::granted_listens(root).contains(&port)
}

/// The separator of a grant's id: no host or folder purlis grants holds it.
const SEP: char = '\u{1f}';

/// **Every grant in force for the project at `root`** (#1348): each open chat's (a closed
/// chat's ended with it, D-1348-1), yours (hosts and folders), and the project's own hosts.
fn grants_of(
    root: &std::path::Path,
    chats: &crate::chats::Chats,
    browsers: &crate::chatbrowsers::ChatBrowsers,
) -> Vec<SandboxGrant> {
    use sandbox::grant::Level;
    let made = sandbox::local::made(root);
    let when = |what: GrantWhat, target: &str, level: Level| {
        made.iter()
            .find(|one| {
                one.what == what.word() && one.target == target && one.level == level.word()
            })
            .and_then(|one| u32::try_from(one.at).ok())
    };
    let row = |what: GrantWhat, target: String, level: Level, at| SandboxGrant {
        id: format!("{}{SEP}{}{SEP}{target}", level.word(), what.word()),
        what,
        target,
        persona: None,
        level: level.into(),
        by: None,
        at,
        chat: None,
        locked: None,
        waiting: None,
        for_no_persona: false,
        expected: false,
        cautions: None,
    };
    let mut out: Vec<SandboxGrant> = chats
        .chat_grants()
        .into_iter()
        .filter_map(|(id, one)| {
            let what = GrantWhat::of_word(one.what.word())?;
            let target = one.what.target();
            let level = if one.once { Level::Once } else { Level::Chat };
            Some(SandboxGrant {
                // Its level first, so Remove audits it as what it is: once, or this chat.
                id: format!("{}{SEP}{id}{SEP}{}{SEP}{target}", level.word(), what.word()),
                chat: Some(one.chat),
                ..row(what, target, level, u32::try_from(one.at).ok())
            })
        })
        .collect();
    for host in sandbox::hosts::personal(root) {
        let target = host.to_string();
        let at = when(GrantWhat::Host, &target, Level::You);
        out.push(row(GrantWhat::Host, target, Level::You, at));
    }
    for folder in sandbox::local::granted_writes(root) {
        let target = folder.display().to_string();
        let at = when(GrantWhat::Write, &target, Level::You);
        out.push(row(GrantWhat::Write, target, Level::You, at));
    }
    // The sockets and services you let every chat here reach (D-sock-2).
    for socket in sandbox::local::granted_sockets(root) {
        let target = socket.display().to_string();
        let at = when(GrantWhat::Socket, &target, Level::You);
        out.push(row(GrantWhat::Socket, target, Level::You, at));
    }
    // What you allowed for every project on this machine (R2): listed in each project, since
    // each project's chats have it, and Remove takes it from them all.
    let everywhere = sandbox::local::made_everywhere();
    let when_everywhere = |what: GrantWhat, target: &str| {
        everywhere
            .iter()
            .find(|one| one.what == what.word() && one.target == target)
            .and_then(|one| u32::try_from(one.at).ok())
    };
    for socket in sandbox::local::sockets_everywhere() {
        let target = socket.display().to_string();
        let at = when_everywhere(GrantWhat::Socket, &target);
        out.push(row(GrantWhat::Socket, target, Level::Machine, at));
    }
    for service in sandbox::local::services_everywhere()
        .into_iter()
        .filter(|service| service != sandbox::services::TRUSTD)
    {
        let at = when_everywhere(GrantWhat::Service, &service);
        out.push(row(GrantWhat::Service, service, Level::Machine, at));
    }
    // Never the certificate check's (R1): one an older build kept grants nothing.
    for service in sandbox::local::granted_services(root)
        .into_iter()
        .filter(|service| service != sandbox::services::TRUSTD)
    {
        let at = when(GrantWhat::Service, &service, Level::You);
        out.push(row(GrantWhat::Service, service, Level::You, at));
    }
    // The services on this Mac you let every chat here connect to (qw384).
    for port in sandbox::local::granted_ports(root) {
        let target = port.to_string();
        let at = when(GrantWhat::Port, &target, Level::You);
        out.push(row(GrantWhat::Port, target, Level::You, at));
    }
    // The ports of this Mac you let every chat here listen on (#2019).
    for port in sandbox::local::granted_listens(root) {
        let target = port.to_string();
        let at = when(GrantWhat::Listen, &target, Level::You);
        out.push(row(GrantWhat::Listen, target, Level::You, at));
    }
    let locks = sandbox::policy::Locks::of(root);
    if let Some(policy) = sandbox::Plane::read(root).in_force(&locks) {
        let hosts: Vec<String> = policy.hosts.iter().map(ToString::to_string).collect();
        for (target, committed) in hosts.iter().zip(committed_by(root, &hosts)) {
            let at = committed
                .as_ref()
                .and_then(|one| u32::try_from(one.at).ok())
                .or_else(|| when(GrantWhat::Host, target, Level::Project));
            out.push(SandboxGrant {
                by: committed.map(|one| one.by),
                ..row(GrantWhat::Host, target.clone(), Level::Project, at)
            });
        }
    }
    // The vaults you let a persona's chats use here (#1430): locked out, not gone, where policy
    // forbids them, so the list says why they no longer open.
    let vaults_locked = locks.vault_grants_refused();
    for grant in sandbox::local::granted_vaults(root) {
        let recorded = made
            .iter()
            .find(|one| one.what == sandbox::local::VAULT && one.target == grant.target());
        out.push(SandboxGrant {
            id: format!(
                "you{SEP}{}{SEP}{}{SEP}{}",
                sandbox::local::VAULT,
                grant.vault,
                grant.persona
            ),
            persona: Some(grant.persona),
            at: recorded.and_then(|one| u32::try_from(one.at).ok()),
            chat: recorded.and_then(|one| one.chat.clone()),
            locked: vaults_locked.clone(),
            ..row(GrantWhat::Vault, grant.vault, Level::You, None)
        });
    }
    // Each persona's hosts you allowed here (#1362), credited to the persona: one row for the
    // list, since an Allow is of the list as it was shown. One whose list changed since grants
    // nothing and says so, with Revoke (D-1362-13).
    let shown = sandbox::persona::shown_in(root, &sandbox::Plane::read(root), &locks);
    for kept in sandbox::local::allowed_persona_hosts(root) {
        let now = shown.iter().find(|one| one.persona == kept.persona);
        let in_force = now.is_some_and(sandbox::persona::Shown::allowed);
        let hosts = now
            .map(|one| hosts_and_tools(&one.hosts, &one.tools))
            .unwrap_or_default();
        out.push(SandboxGrant {
            id: format!(
                "you{SEP}{}{SEP}{}",
                sandbox::local::PERSONA_HOSTS,
                kept.persona
            ),
            at: when(GrantWhat::PersonaHosts, &kept.persona, Level::You),
            waiting: (!in_force).then(|| {
                "waiting: the list changed since you allowed it, so it reaches nothing until \
                 you allow it again"
                    .to_owned()
            }),
            for_no_persona: in_force && now.is_some_and(|one| one.default),
            cautions: now.and_then(|one| cautions_of(&one.tools)),
            persona: Some(kept.persona),
            ..row(
                GrantWhat::PersonaHosts,
                if hosts.is_empty() {
                    "no host or tool now".to_owned()
                } else {
                    hosts
                },
                Level::You,
                None,
            )
        });
    }
    // Always for each persona's browser you allowed here (#2241): kept at `you`, so held, and
    // said to be, where policy's `allow-scopes` leaves that out.
    for kept in sandbox::local::browsers_always(root) {
        out.push(SandboxGrant {
            id: format!("you{SEP}{}{SEP}{}", sandbox::local::BROWSER, kept.persona),
            at: when(GrantWhat::Browser, &kept.persona, Level::You),
            chat: made
                .iter()
                .find(|one| one.what == sandbox::local::BROWSER && one.target == kept.persona)
                .and_then(|one| one.chat.clone()),
            locked: locks.scope_removed(Level::You),
            persona: Some(kept.persona.clone()),
            ..row(
                GrantWhat::Browser,
                crate::chatbrowsers::always_reach(kept.open_web).to_owned(),
                Level::You,
                None,
            )
        });
    }
    // The browser sites (#2018): for one chat's browser, and for every chat's here. Each is the
    // browser's alone, listed apart from the hosts the chats' commands reach.
    for (session, site) in browsers.chat_sites() {
        let target = site.to_string();
        out.push(SandboxGrant {
            id: format!("site{SEP}{session}{SEP}{target}"),
            chat: chats.shown_name(session),
            ..row(GrantWhat::BrowserSite, target, Level::Chat, None)
        });
    }
    for site in sandbox::local::browser_sites(root) {
        let target = site.to_string();
        out.push(SandboxGrant {
            at: when(GrantWhat::BrowserSite, &target, Level::You),
            chat: made
                .iter()
                .find(|one| one.what == sandbox::local::BROWSER_SITE && one.target == target)
                .and_then(|one| one.chat.clone()),
            locked: sandbox::local::browser_site_held(&site, &locks),
            ..row(GrantWhat::BrowserSite, target, Level::You, None)
        });
    }
    // The project's browser sites for everyone in it (#2443): committed, the browsers' alone,
    // each waiting on this machine until the person here was shown it.
    let seen = sandbox::local::browser_sites_seen(root);
    let committed_sites: Vec<String> = sandbox::browser_sites::committed(root, &locks)
        .iter()
        .map(ToString::to_string)
        .collect();
    for (target, committed) in committed_sites
        .iter()
        .zip(committed_by(root, &committed_sites))
    {
        out.push(SandboxGrant {
            by: committed.as_ref().map(|one| one.by.clone()),
            waiting: (!seen.contains(target)).then(|| BROWSER_SITE_UNSEEN.to_owned()),
            ..row(
                GrantWhat::BrowserSite,
                target.clone(),
                Level::Project,
                committed.and_then(|one| u32::try_from(one.at).ok()),
            )
        });
    }
    // What is kept and policy now drops is said, never listed as granted (#1423): a host at
    // the level it is kept at, every folder where policy forbids write grants, and anything
    // kept at a scope policy's `allow-scopes` leaves out (#1927), by the judgement a start
    // makes of it (`Locks::holds_kept`): held by policy, kept on the record, with Remove.
    for one in &mut out {
        let level = sandbox::grant::Level::from(one.level);
        one.locked = match one.what {
            GrantWhat::Host => sandbox::hosts::Host::parse(&one.target)
                .ok()
                .and_then(|host| {
                    locks
                        .refuses(&sandbox::hosts::Granted {
                            host: host.clone(),
                            level: level.hosts_level(),
                        })
                        // One chat's, or an Allow's line at a scope policy leaves out.
                        .or_else(|| {
                            sandbox::host_held(root, &host, level, &locks).or_else(|| {
                                locks.holds_kept(&sandbox::grant::What::Host(host), level)
                            })
                        })
                }),
            GrantWhat::Write => locks.holds_kept(
                &sandbox::grant::What::Write(one.target.clone().into()),
                level,
            ),
            // A vault's row and a browser's were given their own lock as they were listed.
            GrantWhat::Vault | GrantWhat::Browser | GrantWhat::BrowserSite => one.locked.take(),
            // Only hosts a chat would reach are listed for a persona (#1362).
            GrantWhat::PersonaHosts => None,
            // A socket or a service of the system's name lookup is never granted (#2657): one
            // kept for it reaches no chat, and says why.
            GrantWhat::Socket
                if sandbox::services::is_name_lookup(std::path::Path::new(&one.target)) =>
            {
                Some(sandbox::services::NAME_LOOKUP_REFUSED.to_owned())
            }
            GrantWhat::Service if sandbox::services::is_name_lookup_service(&one.target) => {
                Some(sandbox::services::NAME_LOOKUP_REFUSED.to_owned())
            }
            GrantWhat::Socket => locks.holds_kept(
                &sandbox::grant::What::Socket(one.target.clone().into()),
                level,
            ),
            GrantWhat::Service => {
                locks.holds_kept(&sandbox::grant::What::Service(one.target.clone()), level)
            }
            GrantWhat::Port => one
                .target
                .parse()
                .ok()
                .and_then(|port| locks.holds_kept(&sandbox::grant::What::Port(port), level)),
            GrantWhat::Listen => one
                .target
                .parse()
                .ok()
                .and_then(|port| locks.holds_kept(&sandbox::grant::What::Listen(port), level)),
        };
    }
    // What the project declares it expects is said beside its grant (D-sock-4), never a grant.
    let declared = sandbox::services::declared(root);
    for one in &mut out {
        one.expected = match one.what {
            GrantWhat::Socket => {
                declared.expects(&sandbox::grant::What::Socket(one.target.clone().into()))
            }
            GrantWhat::Service => {
                declared.expects(&sandbox::grant::What::Service(one.target.clone()))
            }
            _ => false,
        };
    }
    out
}

/// **Revokes the grant called `id`** ([`grants_of`]) in the project at `root` (#1348): checked
/// to be there, audited, then taken out of every later start. A project host's revoke is a
/// change to the committed file, which teammates follow like any other.
fn revoke(
    root: &std::path::Path,
    chats: &crate::chats::Chats,
    id: &str,
    audit: Audit<'_>,
) -> Result<(), String> {
    use sandbox::grant::{Audited, Level, What};
    let gone = || "purlis did not remove it: it is no longer there.".to_owned();
    let parts: Vec<&str> = id.split(SEP).collect();
    if let ["you", sandbox::local::VAULT, vault, persona] = parts.as_slice() {
        return revoke_vault(root, vault, persona, audit);
    }
    if let ["you", sandbox::local::PERSONA_HOSTS, persona] = parts.as_slice() {
        return revoke_persona(root, persona, audit);
    }
    if let ["you", sandbox::local::BROWSER, persona] = parts.as_slice() {
        return revoke_browser(root, persona, audit);
    }
    let (level, chat, what, target) = match parts.as_slice() {
        ["chat", chat, what, target] => (Level::Chat, Some(*chat), *what, *target),
        ["once", chat, what, target] => (Level::Once, Some(*chat), *what, *target),
        [level, what, target] => (
            Level::of_word(level).ok_or_else(gone)?,
            None,
            *what,
            *target,
        ),
        _ => return Err(gone()),
    };
    // A chat is named for a one-chat grant alone, and every one-chat grant names its chat
    // (#2653): a grant for me, for every project or for everyone is removed where it is kept,
    // never as one chat's.
    if chat.is_some() != level.one_chat() {
        return Err(gone());
    }
    let grant = match GrantWhat::of_word(what).ok_or_else(gone)? {
        // A vault's grant is named with its persona, and a persona's hosts and browser by it,
        // above.
        GrantWhat::Vault
        | GrantWhat::PersonaHosts
        | GrantWhat::Browser
        | GrantWhat::BrowserSite => return Err(gone()),
        GrantWhat::Host => What::Host(sandbox::hosts::Host::parse(target).map_err(|_| gone())?),
        GrantWhat::Write => What::Write(std::path::PathBuf::from(target)),
        GrantWhat::Socket => What::Socket(std::path::PathBuf::from(target)),
        GrantWhat::Service => What::Service(target.to_owned()),
        GrantWhat::Port => What::Port(target.parse().map_err(|_| gone())?),
        GrantWhat::Listen => What::Listen(target.parse().map_err(|_| gone())?),
    };
    let there = match (&grant, level, chat) {
        (_, Level::Once | Level::Chat, Some(chat)) => chats.holds(chat, &grant),
        (What::Host(host), Level::You, _) => sandbox::hosts::personal(root).contains(host),
        (What::Host(host), Level::Project, _) => sandbox::Plane::read(root)
            .in_force(&sandbox::policy::Locks::of(root))
            .is_some_and(|policy| policy.hosts.contains(host)),
        (What::Write(folder), Level::You, _) => {
            sandbox::local::granted_writes(root).contains(folder)
        }
        (What::Socket(socket), Level::You, _) => {
            sandbox::local::granted_sockets(root).contains(socket)
        }
        (What::Service(service), Level::You, _) => {
            sandbox::local::granted_services(root).contains(service)
        }
        (What::Socket(socket), Level::Machine, _) => {
            sandbox::local::sockets_everywhere().contains(socket)
        }
        (What::Service(service), Level::Machine, _) => {
            sandbox::local::services_everywhere().contains(service)
        }
        (What::Port(port), Level::You, _) => sandbox::local::granted_ports(root).contains(port),
        (What::Listen(port), Level::You, _) => sandbox::local::granted_listens(root).contains(port),
        _ => false,
    };
    if !there {
        return Err(gone());
    }
    audit(
        None,
        &Audited {
            granted: false,
            what,
            target,
            level,
        },
    )?;
    // A chat whose proxy in the session host could not be told, said once the grant is gone
    // (#1999): its proxy was ended rather than left carrying the port.
    let mut not_withdrawn: Option<String> = None;
    match (&grant, chat) {
        (_, Some(chat)) => {
            chats.revoke(chat, &grant);
            if let What::Host(host) = &grant {
                chats.forget_live(Some(chat), host, level);
            }
            // Carried by its proxy no longer (qw384b), unless this machine's file still holds
            // it for every chat of the project.
            if let What::Port(port) | What::Listen(port) = &grant
                && !held_for_the_project(root, *port)
            {
                not_withdrawn = chats.withdraw_port(Some(chat), *port).err();
            }
            // Its server serves until the chat restarts, which it is owed now (#2019).
            if let What::Listen(port) = &grant {
                chats.owe_restart_serving(Some(chat), *port);
            }
        }
        (What::Host(host), None) => {
            purlis_core::settings::hosts::revoke(root, hosts_file(level), host)?;
            // What it allowed live goes with it (#1666).
            chats.forget_live(None, host, level);
        }
        (What::Write(folder), None) => {
            sandbox::local::revoke_write(root, folder)
                .map_err(|why| format!("purlis could not remove {}: {why}", folder.display()))?;
        }
        // For every project (R2): out of this machine's one file, with its record.
        (What::Socket(socket), None) if level == Level::Machine => {
            sandbox::local::revoke_socket_everywhere(socket)
                .map_err(|why| format!("purlis could not remove {}: {why}", socket.display()))?;
            return Ok(());
        }
        (What::Service(service), None) if level == Level::Machine => {
            sandbox::local::revoke_service_everywhere(service)
                .map_err(|why| format!("purlis could not remove {service}: {why}"))?;
            return Ok(());
        }
        (What::Socket(socket), None) => {
            sandbox::local::revoke_socket(root, socket)
                .map_err(|why| format!("purlis could not remove {}: {why}", socket.display()))?;
        }
        (What::Service(service), None) => {
            sandbox::local::revoke_service(root, service)
                .map_err(|why| format!("purlis could not remove {service}: {why}"))?;
        }
        (What::Port(port), None) => {
            sandbox::local::revoke_port(root, *port)
                .map_err(|why| format!("purlis could not remove port {port}: {why}"))?;
            // Carried by no open chat's proxy any more but one that holds it for itself
            // (qw384b), or while the project still listens on it (#2019).
            if !held_for_the_project(root, *port) {
                not_withdrawn = chats.withdraw_port(None, *port).err();
            }
        }
        (What::Listen(port), None) => {
            sandbox::local::revoke_listen(root, *port)
                .map_err(|why| format!("purlis could not remove port {port}: {why}"))?;
            // Its browsers and proxies reach it no more (#2019), as for a port to connect to,
            // and each chat serving on it restarts without it once its turn ends.
            if !held_for_the_project(root, *port) {
                not_withdrawn = chats.withdraw_port(None, *port).err();
            }
            // Served no more, whatever else still lets the chats connect there: each serving
            // on it restarts without it once its turn ends.
            chats.owe_restart_serving(None, *port);
        }
    }
    if let Err(why) = sandbox::local::forget_made(root, what, target, level.word()) {
        tracing::warn!("purlis: a revoked grant's record was left behind ({why})");
    }
    match not_withdrawn {
        Some(why) => Err(why),
        None => Ok(()),
    }
}

/// **Revokes `persona`'s use of `vault`** in the project at `root` (#1430): checked to be
/// there, audited, then taken off this machine's record. The next brokered run reads it, so a
/// chat of that persona is refused the vault again with nothing restarted.
fn revoke_vault(
    root: &std::path::Path,
    vault: &str,
    persona: &str,
    audit: Audit<'_>,
) -> Result<(), String> {
    let grant = sandbox::local::VaultGrant {
        vault: vault.to_owned(),
        persona: persona.to_owned(),
    };
    if !sandbox::local::granted_vaults(root).contains(&grant) {
        return Err("purlis did not remove it: it is no longer there.".to_owned());
    }
    audit(
        None,
        &sandbox::grant::Audited {
            granted: false,
            what: sandbox::local::VAULT,
            target: &grant.target(),
            level: sandbox::grant::Level::You,
        },
    )?;
    sandbox::local::revoke_vault(root, vault, persona)
        .map_err(|why| format!("purlis could not remove vault {vault} for {persona}: {why}"))
}

/// **Revokes your Allow of `persona`'s hosts** in the project at `root` (#1362): checked to be
/// there, audited, then taken off this machine's record. Chats as `persona` reach them no more
/// from their next start, and the Notice asks again.
fn revoke_persona(root: &std::path::Path, persona: &str, audit: Audit<'_>) -> Result<(), String> {
    let gone = || "purlis did not remove it: it is no longer there.".to_owned();
    if !sandbox::local::allowed_persona_hosts(root)
        .iter()
        .any(|kept| kept.persona == persona)
    {
        return Err(gone());
    }
    audit(
        None,
        &sandbox::grant::Audited {
            granted: false,
            what: sandbox::local::PERSONA_HOSTS,
            target: persona,
            level: sandbox::grant::Level::You,
        },
    )?;
    sandbox::local::revoke_persona_hosts(root, persona)
        .map_err(|why| format!("purlis could not remove {persona}'s hosts: {why}"))
        .map(|_| ())
}

/// **Removes your Always for `persona`'s browser** in the project at `root` (#2241): checked to
/// be there, audited, then taken off this machine's record. The caller ends the browsers of the
/// chats it allowed ([`crate::chatbrowsers::always_removed`]).
fn revoke_browser(root: &std::path::Path, persona: &str, audit: Audit<'_>) -> Result<(), String> {
    let Some(open_web) = sandbox::local::browser_always(root, persona) else {
        return Err("purlis did not remove it: it is no longer there.".to_owned());
    };
    audit(
        None,
        &sandbox::grant::Audited {
            granted: false,
            what: sandbox::local::BROWSER,
            target: &crate::chatbrowsers::always_said(persona, open_web),
            level: sandbox::grant::Level::You,
        },
    )?;
    sandbox::local::revoke_browser_always(root, persona)
        .map_err(|why| format!("purlis could not remove Always for {persona}'s browser: {why}"))
        .map(|_| ())
}

/// The persona whose browser Always the grant called `id` is ([`grants_of`]), where it is one.
fn browser_always_of(id: &str) -> Option<&str> {
    match id.split(SEP).collect::<Vec<_>>().as_slice() {
        ["you", sandbox::local::BROWSER, persona] => Some(persona),
        _ => None,
    }
}

/// Every grant in force here, for Settings' Granted list (#1348).
#[tauri::command]
#[specta::specta]
pub async fn sandbox_grants(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<SandboxGrant>, String> {
    let held = planes.held(&plane)?;
    // On a blocking thread: naming who committed the project's hosts asks git (#1543).
    crate::off_the_window("reading the sandbox grants", move || {
        Ok(grants_of(held.root(), held.chats(), held.browsers()))
    })
    .await
}

/// **Revoke** on Settings' Granted list (#1348): the grant called `id` is taken out of every
/// later start, and audited. Answers the list as it is now.
#[tauri::command]
#[specta::specta]
pub async fn revoke_sandbox_grant(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    id: String,
) -> Result<Vec<SandboxGrant>, String> {
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    let browsers_ended = std::sync::Arc::clone(planes.browsers_ended());
    // On a blocking thread, as `sandbox_grants` is: the list it answers asks git (#1543).
    crate::off_the_window("revoking a sandbox grant", move || {
        let root = held.root();
        let audit = held.audit_then_record(network.as_ref());
        match browser_site_of(&id) {
            // A browser site's (#2018): its browsers end, as a narrowing ends them.
            Some(granted) => {
                revoke_browser_site(&held, &plane, &granted, &audit, &*browsers_ended)?
            }
            None => revoke(root, held.chats(), &id, &audit)?,
        }
        // Removing Always for a persona's browser narrows the chats it allowed (#2241): their
        // browsers end now, D-2013-11.
        if let Some(persona) = browser_always_of(&id) {
            crate::chatbrowsers::always_removed(&held, &plane, persona, &*browsers_ended);
        }
        Ok(grants_of(root, held.chats(), held.browsers()))
    })
    .await
}

/// Why a project's browser site reaches no browser here yet (#2443).
const BROWSER_SITE_UNSEEN: &str = "It reaches no browser here until you read the Notice of the \
                                   project's browser sites.";

/// A browser site's grant, as its id in the Granted list names it ([`grants_of`]).
#[derive(Debug, Clone, PartialEq, Eq)]
enum SiteGranted {
    /// For chat `session`'s browser alone.
    Chat(u32, sandbox::browser_reach::Site),
    /// For every chat's browser here, on this machine.
    You(sandbox::browser_reach::Site),
    /// For every chat's browser of everyone in the project, committed (#2443).
    Project(sandbox::hosts::Host),
}

/// The browser site the grant called `id` is, where it is one: None for any other grant, or
/// for an id that names no site.
fn browser_site_of(id: &str) -> Option<SiteGranted> {
    let site = |written: &str| sandbox::browser_reach::Site::parse(written).ok();
    match id.split(SEP).collect::<Vec<_>>().as_slice() {
        ["site", session, written] => {
            Some(SiteGranted::Chat(session.parse().ok()?, site(written)?))
        }
        ["you", sandbox::local::BROWSER_SITE, written] => Some(SiteGranted::You(site(written)?)),
        ["project", sandbox::local::BROWSER_SITE, written] => Some(SiteGranted::Project(
            sandbox::hosts::Host::parse(written).ok()?,
        )),
        _ => None,
    }
}

/// **Removes a browser site** (#2018): checked to be there, audited, then taken away, and the
/// browsers it reached end, their tabs told so; each one's next call opens one on what is
/// allowed then.
fn revoke_browser_site(
    held: &crate::planes::Held,
    plane: &PlaneId,
    granted: &SiteGranted,
    audit: Audit<'_>,
    told: &dyn Fn(crate::chatbrowsers::BrowsersEnded),
) -> Result<(), String> {
    use sandbox::grant::Level;
    let gone = || "purlis did not remove it: it is no longer there.".to_owned();
    let (session, site, level) = match granted {
        SiteGranted::Chat(session, site) => (Some(*session), site, Level::Chat),
        SiteGranted::You(site) => (None, site, Level::You),
        SiteGranted::Project(host) => return revoke_project_site(held, plane, host, audit, told),
    };
    let there = match session {
        Some(session) => held
            .browsers()
            .chat_sites()
            .contains(&(session, site.clone())),
        None => sandbox::local::browser_sites(held.root()).contains(site),
    };
    if !there {
        return Err(gone());
    }
    audit(
        session,
        &sandbox::grant::Audited {
            granted: false,
            what: sandbox::local::BROWSER_SITE,
            target: &site.to_string(),
            level,
        },
    )?;
    let ended = match session {
        Some(session) => held
            .browsers()
            .chat_site_removed(held.browser_driver(), session, site)
            .map(|ended| vec![(session, ended)])
            .unwrap_or_default(),
        None => {
            sandbox::local::revoke_browser_site(held.root(), site)
                .map_err(|why| format!("purlis could not remove {site}: {why}"))?;
            held.browsers().sites_removed(held.browser_driver())
        }
    };
    for (session, crate::chatbrowsers::Ended { ended, unended }) in ended {
        let note = if unended > 0 {
            crate::chatbrowsers::NOT_ENDED_ON_ITS_TAB
        } else if ended > 0 {
            crate::chatbrowsers::ENDED_ON_ITS_TAB
        } else {
            continue;
        };
        told(crate::chatbrowsers::BrowsersEnded {
            plane: plane.clone(),
            session,
            note: note.to_owned(),
        });
    }
    Ok(())
}

/// **Removes a browser site for everyone in the project** (#2443): checked to be listed,
/// audited, then taken out of the committed file and forgotten as seen here, and every chat's
/// browsers here end, their tabs told so. Where the file is not written, the audit says the site
/// stands again.
fn revoke_project_site(
    held: &crate::planes::Held,
    plane: &PlaneId,
    host: &sandbox::hosts::Host,
    audit: Audit<'_>,
    told: &dyn Fn(crate::chatbrowsers::BrowsersEnded),
) -> Result<(), String> {
    let root = held.root();
    let listed = sandbox::Plane::read(root)
        .browser_sites()
        .is_ok_and(|listed| listed.hosts.contains(host));
    if !listed {
        return Err("purlis did not remove it: it is no longer there.".to_owned());
    }
    let audited = |granted: bool| {
        audit(
            None,
            &sandbox::grant::Audited {
                granted,
                what: sandbox::local::BROWSER_SITE,
                target: &host.to_string(),
                level: sandbox::grant::Level::Project,
            },
        )
    };
    audited(false)?;
    if let Err(why) = sandbox::browser_sites::revoke(root, host) {
        // Nothing was removed: the audit says it stands again, as the file still says.
        if !sandbox::Plane::read(root)
            .browser_sites()
            .is_ok_and(|listed| !listed.hosts.contains(host))
        {
            let _ = audited(true);
        }
        return Err(format!("purlis could not remove {host}: {why}"));
    }
    browsers_ended_told(
        plane,
        held.browsers().sites_removed(held.browser_driver()),
        told,
    );
    Ok(())
}

/// Tells each tab of `ended` that its chat's browsers ended, or could not be ended yet.
fn browsers_ended_told(
    plane: &PlaneId,
    ended: Vec<(u32, crate::chatbrowsers::Ended)>,
    told: &dyn Fn(crate::chatbrowsers::BrowsersEnded),
) {
    for (session, crate::chatbrowsers::Ended { ended, unended }) in ended {
        let note = if unended > 0 {
            crate::chatbrowsers::NOT_ENDED_ON_ITS_TAB
        } else if ended > 0 {
            crate::chatbrowsers::ENDED_ON_ITS_TAB
        } else {
            continue;
        };
        told(crate::chatbrowsers::BrowsersEnded {
            plane: plane.clone(),
            session,
            note: note.to_owned(),
        });
    }
}

/// The commands of Settings' Granted list that ask git, which answer off the window's thread
/// (#1543). The thread test asks each of them.
#[cfg(test)]
pub(crate) const READS_HISTORY: [&str; 2] = ["sandbox_grants", "revoke_sandbox_grant"];

/// The folders you listed as ones chats in this project may be granted (D-1342-10), each as it
/// was resolved; and each one dropped from the list now because it no longer resolves to itself
/// (it, or a folder above it, was swapped for a link since: R8), for Settings to say.
#[tauri::command]
#[specta::specta]
pub async fn grantable_folders(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<GrantableFolders, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    // Off the window's thread (#1949): each listed folder is resolved on the disk again.
    crate::off_the_window(READING_SANDBOX, move || Ok(listed_and_pruned(&root))).await
}

/// The folders chats may be granted, and those just dropped from the list.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct GrantableFolders {
    pub folders: Vec<String>,
    pub dropped: Vec<String>,
}

fn listed_and_pruned(root: &std::path::Path) -> GrantableFolders {
    let mut dropped = Vec::new();
    for listed in sandbox::local::grantable_folders(root) {
        if !sandbox::grant::still_itself(&listed) {
            match sandbox::local::unlist_grantable(root, &listed) {
                Ok(()) => dropped.push(listed.display().to_string()),
                Err(why) => tracing::warn!(
                    "purlis: a listed folder that resolves elsewhere was left listed ({why})"
                ),
            }
        }
    }
    GrantableFolders {
        folders: listed_folders(root),
        dropped,
    }
}

fn listed_folders(root: &std::path::Path) -> Vec<String> {
    sandbox::local::grantable_folders(root)
        .iter()
        .map(|folder| folder.display().to_string())
        .collect()
}

/// Lists `folder` as one chats here may be granted (D-1342-10), this machine only: every refusal
/// a write grant makes, but the allowlist it adds to. Answers the list as it is now, and what
/// the folder holds that later code is loaded from, for the warning Settings shows.
#[tauri::command]
#[specta::specta]
pub async fn list_grantable_folder(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    folder: String,
) -> Result<GrantableListed, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window(ANSWERING_SANDBOX, move || {
        list_folder(&root, &sandbox::Machine::this(), &folder)
    })
    .await
}

fn list_folder(
    root: &std::path::Path,
    machine: &sandbox::Machine,
    folder: &str,
) -> Result<GrantableListed, String> {
    // Policy (#1343): a folder listed here is one a chat may be granted, which it forbids.
    if let Some(why) = sandbox::policy::Locks::of(root).write_grants_refused() {
        return Err(why);
    }
    let ground = sandbox::grant::Ground::of(root, root, machine);
    let folder =
        sandbox::grant::grantable(folder, &ground.place()).map_err(|why| why.to_string())?;
    let holds = sandbox::grant::holds_later_code(&folder, &ground.place());
    sandbox::local::list_grantable(root, &folder)
        .map_err(|why| format!("purlis could not keep {}: {why}", folder.display()))?;
    Ok(GrantableListed {
        folders: listed_folders(root),
        holds,
    })
}

/// The folders chats may be granted once one was listed, and what the one listed holds that
/// later code is loaded from, outside any sandbox: what Settings warns of.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct GrantableListed {
    pub folders: Vec<String>,
    pub holds: Vec<String>,
}

/// Takes `folder` off the folders chats here may be granted. Answers the list as it is now.
#[tauri::command]
#[specta::specta]
pub async fn unlist_grantable_folder(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    folder: String,
) -> Result<Vec<String>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window(ANSWERING_SANDBOX, move || {
        sandbox::local::unlist_grantable(&root, std::path::Path::new(&folder))
            .map_err(|why| format!("purlis could not take {folder} off the list: {why}"))?;
        Ok(listed_folders(&root))
    })
    .await
}

/// **The event every window is sent when this machine's sandbox policy changes** (#1431): the
/// window asks again which chats run under an older sandbox, so a policy that now requires the
/// sandbox is told about the chats running without it, with no settings file or chat moving.
pub const POLICY_CHANGED: &str = "sandbox-policy-changed";

/// How often this machine's policy file is read again: soon enough that a policy landing is
/// told while the person is still at the window, and a read of one small file costs nothing.
const POLICY_READ_EVERY: std::time::Duration = std::time::Duration::from_secs(5);

/// **What this machine's policy was when it was last read** (#1431): what says whether a read
/// found it changed. A file rewritten to the same locks is no change.
pub struct PolicyWatch {
    last: sandbox::policy::Locks,
}

impl PolicyWatch {
    /// A watch that starts from `now`.
    pub fn from(now: sandbox::policy::Locks) -> Self {
        Self { last: now }
    }

    /// Whether `now` is another policy than the last one read, keeping it as the last.
    pub fn moved(&mut self, now: sandbox::policy::Locks) -> bool {
        if now == self.last {
            return false;
        }
        self.last = now;
        true
    }
}

/// **Reads this machine's policy file every [`POLICY_READ_EVERY`]**, and tells every window
/// [`POLICY_CHANGED`] when it reads to other locks (#1431). It only reads: the file is read as
/// every start reads it ([`sandbox::policy::Locks::of`]), and nothing is written. On a thread
/// of its own for the life of the app.
pub(crate) fn watch_policy(app: tauri::AppHandle) {
    use tauri::Emitter as _;
    let read = || sandbox::policy::Locks::of(std::path::Path::new("/"));
    let spawned = std::thread::Builder::new()
        .name("purlis-policy".into())
        .spawn(move || {
            let mut watch = PolicyWatch::from(read());
            loop {
                std::thread::sleep(POLICY_READ_EVERY);
                if watch.moved(read()) {
                    let _ = app.emit(POLICY_CHANGED, ());
                }
            }
        });
    if let Err(why) = spawned {
        tracing::warn!(
            "purlis: the watch on this machine's sandbox policy could not be started ({why}), \
             so a policy that changes is told when the window is next focused"
        );
    }
}

#[cfg(test)]
mod tests;
