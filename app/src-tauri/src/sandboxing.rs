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

/// [`SandboxAhead`] for a chat on `profile` in the project at `root`, on this machine: every
/// refusal the start would give, shown before anything starts — a harness charter holds back
/// (Codex, ruling V87f), and a program the sandbox will not bind (ruling V87g), which is asked
/// of the profile's own resolved words.
///
/// **Only a profile the start would run has its program run.** The V87g check asks the
/// program its `--version`, which runs it, outside any sandbox. So it is asked only past the
/// start's own gate ([`purlis_core::start::may_check_program`], which is
/// `wiring::refusal`): a startable kind, a `charter.local.toml` git would not carry, an
/// approved command. Any other row says the program is checked once the profile may start.
pub fn ahead_here(
    profile: &purlis_core::profiles::Profile,
    root: &std::path::Path,
) -> Option<SandboxAhead> {
    ahead_with(
        profile,
        root,
        purlis_core::start::may_check_program(profile, root),
    )
}

/// [`ahead_here`], told whether the start's gate lets the program be checked: the start's own
/// check ([`purlis_core::start::sandbox_ahead`]), which asks the same gate itself and runs
/// the program only past it. `approved` only phrases the row.
fn ahead_with(
    profile: &purlis_core::profiles::Profile,
    root: &std::path::Path,
    approved: bool,
) -> Option<SandboxAhead> {
    let row = SandboxAhead::of(purlis_core::start::sandbox_ahead(
        profile,
        root,
        &sandbox::Machine::this(),
        &backend::installed,
        &os_release(),
        None,
    )?)?;
    let row = SandboxAhead {
        locked: sandbox::policy::Locks::of(root).opt_out_refused(),
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
            digest: one.digest,
        })
        .collect()
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

/// The project's sandbox, for the offer notice and Settings.
#[tauri::command]
#[specta::specta]
pub fn sandbox_state(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<SandboxState, String> {
    Ok(state_of(planes.held(&plane)?.root()))
}

/// The person's answer to the one-time offer: `turn_on` writes `[sandbox] mode = "on"` into
/// the project's `charter.toml`; either answer is the last time it is asked on this machine.
#[tauri::command]
#[specta::specta]
pub fn answer_sandbox_offer(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    turn_on: bool,
) -> Result<SandboxState, String> {
    let held = planes.held(&plane)?;
    answer(held.root(), turn_on)
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
#[tauri::command]
#[specta::specta]
pub fn persona_grants_held(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<Option<GrantsHeld>, String> {
    let held = planes.held(&plane)?;
    let locked = persona_hosts_locked(held.root());
    Ok(held
        .chats()
        .grants_held(session)
        .map(|(from, persona)| GrantsHeld {
            waits_here: persona_hosts_wait(held.root(), persona.as_deref()),
            from,
            persona,
            locked,
        }))
}

/// The person allowed chat `session` its own persona's grants from its tab's Notice (#1362):
/// they apply from its next start, since a chat's sandbox is compiled as it starts.
#[tauri::command]
#[specta::specta]
pub fn allow_persona_grants(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<bool, String> {
    let held = planes.held(&plane)?;
    own_grants(
        session,
        held.chats()
            .grants_held(session)
            .map(|(_, persona)| persona),
        &held.audit_then_record(planes.network()),
        || held.chats().allow_own_grants(session),
    )
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
pub fn acknowledge_project_hosts(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    shown: Vec<String>,
) -> Result<SandboxState, String> {
    let held = planes.held(&plane)?;
    acknowledge(held.root(), &shown)
}

/// The person read the Notice of the project's Internet access presets as it showed them,
/// `shown` (#1385): it is not shown again until they change from that.
#[tauri::command]
#[specta::specta]
pub fn acknowledge_project_presets(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    shown: Vec<String>,
) -> Result<SandboxState, String> {
    let held = planes.held(&plane)?;
    acknowledge_presets(held.root(), &shown)
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
/// file declares, of what the question showed (`shown`, each as the file writes it). Each is
/// judged as any grant is, audited, and kept in this machine's own file, never the project's:
/// the declaration itself grants nothing. One that cannot be allowed here now is left out and
/// said. A chat takes it from its next start. The window's alone (`WINDOW_ONLY`): no link and
/// no chat makes it, so a chat can neither ask for it nor answer it.
#[tauri::command]
#[specta::specta]
pub fn allow_project_expects(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    shown: Vec<String>,
) -> Result<SandboxState, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    allow_expects(
        &root,
        held.chats(),
        &shown,
        &held.audit_then_record(planes.network()),
        now_secs(),
    )?;
    Ok(state_of(&root))
}

/// [`allow_project_expects`] in the project at `root`: only what is declared **and** was
/// shown **and** still waits is kept, for every chat of the project on this machine
/// ([`kept_for`], for no chat). Refused whole where policy forbids it. The answer is then
/// recorded for what was shown, so the question is not asked again for what could not be
/// allowed, and is asked for an item declared since the question was drawn.
fn allow_expects(
    root: &std::path::Path,
    chats: &crate::chats::Chats,
    shown: &[String],
    audit: Audit<'_>,
    at: u64,
) -> Result<(), String> {
    use sandbox::grant::Level;
    let Some(awaiting) = sandbox::services::awaiting(root, &sandbox::Machine::this()) else {
        return Err("Nothing this project expects waits to be allowed on this machine.".to_owned());
    };
    if let Some(locked) = &awaiting.locked {
        return Err(locked.clone());
    }
    let locks = sandbox::policy::Locks::of(root);
    let mut kept = 0;
    for item in awaiting
        .items
        .iter()
        .filter(|item| item.refused.is_none() && shown.contains(&item.written))
    {
        if let Some(why) = locks.refuses_grant(&item.what, Level::You) {
            return Err(why);
        }
        kept_for(root, chats, None, (&item.what, Level::You), audit, at)?;
        kept += 1;
    }
    if kept == 0 {
        return Err(
            "Nothing shown can be allowed on this machine now, so nothing was allowed.".to_owned(),
        );
    }
    let answered: Vec<String> = awaiting
        .shown()
        .into_iter()
        .filter(|one| shown.contains(one))
        .collect();
    sandbox::local::acknowledge_expects(root, &answered).map_err(|err| err.to_string())
}

/// **Not now**, on the Notice of what a project expects (ADR 0093): remembered on this
/// machine for the declaration as it was shown, `shown`, so it is asked again only when the
/// project declares something new. Nothing is allowed, and Settings › Sandbox still offers
/// it. The window's alone (`WINDOW_ONLY`): a chat cannot answer the question for the person.
#[tauri::command]
#[specta::specta]
pub fn decline_project_expects(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    shown: Vec<String>,
) -> Result<SandboxState, String> {
    let held = planes.held(&plane)?;
    decline_expects(held.root(), &shown)
}

/// [`decline_project_expects`] in the project at `root`: only what the project declares now
/// is recorded, whatever the window sent.
fn decline_expects(root: &std::path::Path, shown: &[String]) -> Result<SandboxState, String> {
    let declared: Vec<String> = sandbox::services::awaiting(root, &sandbox::Machine::this())
        .map(|awaiting| awaiting.shown())
        .unwrap_or_default()
        .into_iter()
        .filter(|one| shown.contains(one))
        .collect();
    sandbox::local::acknowledge_expects(root, &declared).map_err(|err| err.to_string())?;
    Ok(state_of(root))
}

/// **Allow** on a persona's hosts' Notice (#1362, D-1362-7): the person lets chats running as
/// `persona` reach its committed hosts on this machine, exactly as the Notice showed them,
/// `digest`. A list that changed since it was shown is refused, and nothing is kept. The window's
/// alone (`WINDOW_ONLY`): no link and no chat makes it.
#[tauri::command]
#[specta::specta]
pub fn allow_persona_hosts(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    persona: String,
    digest: String,
) -> Result<SandboxState, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    allow_persona(
        &root,
        &persona,
        &digest,
        &held.audit_then_record(planes.network()),
        now_secs(),
    )?;
    Ok(state_of(&root))
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
    let hosts: Vec<String> = shown
        .hosts
        .iter()
        .map(ToString::to_string)
        .chain(shown.tools.iter().map(sandbox::tools::Tool::whole))
        .collect();
    audit(
        None,
        &sandbox::grant::Audited {
            granted: true,
            what: sandbox::local::PERSONA_HOSTS,
            target: &format!(
                "{persona}: {}{}",
                hosts.join(", "),
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
}

impl GrantWhat {
    fn word(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Write => "write",
            Self::Vault => sandbox::local::VAULT,
            Self::PersonaHosts => sandbox::local::PERSONA_HOSTS,
            Self::Socket => "socket",
            Self::Service => "service",
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
) -> Result<Allowed, String> {
    let folder = chats.folder_of(session);
    let (what, level) = judged(root, machine, folder.as_deref(), session, asked)?;
    let kept = kept(root, chats, session, (&what, level), audit, at)?;
    Ok(Allowed {
        said: allowed_said(session, level, &kept),
        live: kept.live.contains(&session),
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
) -> Result<(sandbox::grant::What, sandbox::grant::Level), String> {
    use sandbox::grant::{self, Level, What};
    let level = Level::from(level);
    if level == Level::Project && matches!(what, GrantWhat::Socket | GrantWhat::Service) {
        return Err(
            "purlis will not allow a local socket or a system service for everyone in the \
             project: it is this machine's, and a committed file that granted one could grant \
             itself to every chat. Allow it for this chat, or for every chat of this project on \
             this machine."
                .to_owned(),
        );
    }
    if level == Level::Once && matches!(what, GrantWhat::Host | GrantWhat::Write) {
        return Err(
            "purlis allows a host or a folder for this chat, or for longer, not once.".to_owned(),
        );
    }
    if level == Level::Project && what == GrantWhat::Write {
        return Err(
            "purlis will not allow a folder for everyone in the project: a folder is a path on \
             this machine. Allow it for this chat, or for every chat of this project on this \
             machine."
                .to_owned(),
        );
    }
    let what = match what {
        GrantWhat::Vault => {
            return Err(
                "purlis allows a vault from the notice its refusal raises, not from a block's."
                    .to_owned(),
            );
        }
        GrantWhat::PersonaHosts => {
            return Err(
                "purlis allows a persona's hosts from the notice that shows them, not from a \
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
        GrantWhat::Socket => What::Socket(
            sandbox::services::socket(
                target,
                &sandbox::services::SocketGround::of(root, machine, &[]),
            )
            .map_err(|why| why.to_string())?,
        ),
        GrantWhat::Service => {
            let name = sandbox::services::service(target).map_err(|why| why.to_string())?;
            // Never a grant (R1): the project's setting and policy alone decide it.
            if name == sandbox::services::TRUSTD {
                return Err(
                    "Certificate checks are a setting of the project's, on unless it turns them \
                     off, so there is nothing to allow: see Settings › Sandbox."
                        .to_owned(),
                );
            }
            What::Service(name)
        }
    };
    // Policy (#1343): the strictest wins, for a host at the level it would be kept at, and for
    // any folder at all.
    if let Some(why) = sandbox::policy::Locks::of(root).refuses_grant(&what, level) {
        return Err(why);
    }
    Ok((what, level))
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

/// **Keeps a grant [`judged`] made for chat `session`**: audited first, then kept where
/// `level` keeps it, and the chat owed a restart on its conversation unless its proxy took it
/// live (#1666). An audit that cannot be written keeps nothing.
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
    if session.is_none() && matches!(level, Level::Once | Level::Chat) {
        return Err(
            "purlis allows a host for one chat from that chat's own Notice, not from Blocked \
             lately."
                .to_owned(),
        );
    }
    // Every project on this machine is a socket's or a service's alone (R2), whoever asks.
    if level == Level::Machine && !what.machine_wide() {
        return Err(
            "purlis allows only a local socket or a system service for every project on this \
             machine: a host and a folder are a project's."
                .to_owned(),
        );
    }
    let already = match (what, level, session) {
        (What::Host(host), Level::You | Level::Project, _) => grant::allowed_already(root, host)
            .is_some_and(|held| held == level || held == Level::Project),
        (_, Level::Once | Level::Chat, Some(session)) => chats.holds_for(session, what),
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
    let told = grant::told(what, level);
    match (what, level, session) {
        (_, Level::Once, Some(session)) => chats.hold_grant_once(session, what.clone(), at)?,
        (_, Level::Chat, Some(session)) => chats.hold_grant(session, what.clone(), at)?,
        (_, Level::Once | Level::Chat, None) => {}
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
        What::Write(_) | What::Socket(_) | What::Service(_) => crate::chats::Live::default(),
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

/// What an Allow on chat `session`'s Notice says, once [`kept`] came to `kept` at `level`.
pub(crate) fn allowed_said(session: u32, level: sandbox::grant::Level, kept: &Kept) -> String {
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
#[tauri::command]
#[specta::specta]
pub fn allow_sandbox_block(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    shown: crate::taskblocks::BlockShown,
    level: GrantLevel,
) -> Result<Allowed, String> {
    let held = planes.held(&plane)?;
    let block = crate::taskblocks::shown_one(held.chats().blocks(), session, &shown)?;
    let (what, target) = (shown.what, shown.target);
    let root = held.root().to_path_buf();
    let allowed = allow(
        &root,
        &sandbox::Machine::this(),
        held.chats(),
        session,
        (what, &target, level),
        &held.audit_then_record(planes.network()),
        now_secs(),
    )?;
    // Answered: neither this Notice nor a question for several tasks answers it again.
    held.chats().blocks().answered(session, &block);
    Ok(allowed)
}

/// **Keep blocked** on a block's Notice (#1666, taking over #1411's line): `shown` is the block
/// the Notice showed. Refused unless the chat is held on that block now (#1538). Nothing is
/// granted. For a host, what the chat's proxy holds on it is refused now, and the chat is told,
/// in purlis's fixed words, not to try it again unless the person asks.
#[tauri::command]
#[specta::specta]
pub fn keep_sandbox_block(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    shown: crate::taskblocks::BlockShown,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    let block = keep_block(held.chats(), session, &shown)?;
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

/// [`keep_sandbox_block`] on `chats`: the block it answered.
pub(crate) fn keep_block(
    chats: &crate::chats::Chats,
    session: u32,
    shown: &crate::taskblocks::BlockShown,
) -> Result<crate::taskblocks::HeldBlock, String> {
    let block = crate::taskblocks::shown_one(chats.blocks(), session, shown)?;
    chats.blocks().answered(session, &block);
    if block.what == GrantWhat::Host
        && let Ok(host) = sandbox::grant::host(&block.target)
    {
        chats.keep_blocked_live(session, &host);
    }
    Ok(block)
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

/// The separator of a grant's id: no host or folder purlis grants holds it.
const SEP: char = '\u{1f}';

/// **Every grant in force for the project at `root`** (#1348): each open chat's (a closed
/// chat's ended with it, D-1348-1), yours (hosts and folders), and the project's own hosts.
fn grants_of(root: &std::path::Path, chats: &crate::chats::Chats) -> Vec<SandboxGrant> {
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
        let hosts: Vec<String> = now
            .map(|one| {
                one.hosts
                    .iter()
                    .map(ToString::to_string)
                    .chain(one.tools.iter().map(sandbox::tools::Tool::whole))
                    .collect()
            })
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
            persona: Some(kept.persona),
            ..row(
                GrantWhat::PersonaHosts,
                if hosts.is_empty() {
                    "no host or tool now".to_owned()
                } else {
                    hosts.join(", ")
                },
                Level::You,
                None,
            )
        });
    }
    // What is kept and policy now drops is said, never listed as granted (#1423): a host at
    // the level it is kept at, and every folder where policy forbids write grants.
    for one in &mut out {
        one.locked = match one.what {
            GrantWhat::Host => sandbox::hosts::Host::parse(&one.target)
                .ok()
                .and_then(|host| {
                    locks.refuses(&sandbox::hosts::Granted {
                        host,
                        level: sandbox::grant::Level::from(one.level).hosts_level(),
                    })
                }),
            GrantWhat::Write => locks.write_grants_refused(),
            // A vault's row was given its own lock as it was listed (#1430).
            GrantWhat::Vault => one.locked.take(),
            // Only hosts a chat would reach are listed for a persona (#1362).
            GrantWhat::PersonaHosts => None,
            GrantWhat::Socket | GrantWhat::Service => locks.socket_grants_refused(),
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
    let grant = match GrantWhat::of_word(what).ok_or_else(gone)? {
        // A vault's grant is named with its persona, and a persona's hosts by it, above.
        GrantWhat::Vault | GrantWhat::PersonaHosts => return Err(gone()),
        GrantWhat::Host => What::Host(sandbox::hosts::Host::parse(target).map_err(|_| gone())?),
        GrantWhat::Write => What::Write(std::path::PathBuf::from(target)),
        GrantWhat::Socket => What::Socket(std::path::PathBuf::from(target)),
        GrantWhat::Service => What::Service(target.to_owned()),
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
    match (&grant, chat) {
        (_, Some(chat)) => {
            chats.revoke(chat, &grant);
            if let What::Host(host) = &grant {
                chats.forget_live(Some(chat), host, level);
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
    }
    if let Err(why) = sandbox::local::forget_made(root, what, target, level.word()) {
        tracing::warn!("purlis: a revoked grant's record was left behind ({why})");
    }
    Ok(())
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
        Ok(grants_of(held.root(), held.chats()))
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
    // On a blocking thread, as `sandbox_grants` is: the list it answers asks git (#1543).
    crate::off_the_window("revoking a sandbox grant", move || {
        let root = held.root();
        revoke(
            root,
            held.chats(),
            &id,
            &held.audit_then_record(network.as_ref()),
        )?;
        Ok(grants_of(root, held.chats()))
    })
    .await
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
pub fn grantable_folders(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<GrantableFolders, String> {
    let held = planes.held(&plane)?;
    Ok(listed_and_pruned(held.root()))
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
pub fn list_grantable_folder(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    folder: String,
) -> Result<GrantableListed, String> {
    let held = planes.held(&plane)?;
    list_folder(held.root(), &sandbox::Machine::this(), &folder)
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
pub fn unlist_grantable_folder(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    folder: String,
) -> Result<Vec<String>, String> {
    let held = planes.held(&plane)?;
    sandbox::local::unlist_grantable(held.root(), std::path::Path::new(&folder))
        .map_err(|why| format!("purlis could not take {folder} off the list: {why}"))?;
    Ok(listed_folders(held.root()))
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
mod tests {
    use super::*;

    // ---- the policy watch (#1431) ------------------------------------------------------------

    #[test]
    fn the_policy_watch_tells_a_change_once_and_a_rewrite_to_the_same_locks_never() {
        use sandbox::policy::Locks;
        let file = std::path::Path::new("/etc/purlis/policy.json");
        let required = || Locks::parse(r#"{"owner": "IT", "sandbox": {"opt-out": false}}"#, file);
        let mut watch = PolicyWatch::from(Locks::none());
        assert!(!watch.moved(Locks::none()), "no policy, then none");
        assert!(watch.moved(required()), "a policy landed");
        assert!(!watch.moved(required()), "the same locks, read again");
        assert!(
            watch.moved(Locks::parse(
                r#"{"owner": "Ops", "sandbox": {"opt-out": false}}"#,
                file
            )),
            "who set it is part of what the Notice says"
        );
        assert!(watch.moved(Locks::none()), "the policy went");
    }

    // ---- the live ask (#1666) ----------------------------------------------------------------

    #[test]
    fn an_allow_says_whether_the_command_carries_on_or_the_chat_restarts() {
        use sandbox::grant::Level;
        let live = Kept {
            live: vec![4, 6],
            retry: vec![6],
            already: false,
        };
        assert_eq!(
            allowed_said(4, Level::You, &live),
            "Allowed for me on this machine. The command that asked carries on now; nothing \
             restarts."
        );
        // Taken live by other chats only: this one still restarts.
        assert!(allowed_said(5, Level::You, &live).contains("restarts on the same conversation"));
        // Allowed already, and this chat is not on the proxy: said so, and it restarts to take
        // it, where pressing Allow used to be refused and the Notice stayed up.
        // A held connection that had given up: the chat is told to run it again.
        assert!(
            allowed_said(6, Level::You, &live).contains("told to run it again"),
            "{}",
            allowed_said(6, Level::You, &live)
        );
        // Allowed already, and taken live: said so, and the command carries on.
        let both = Kept {
            live: vec![4],
            retry: Vec::new(),
            already: true,
        };
        assert!(allowed_said(4, Level::You, &both).starts_with("It was allowed already"));
        let already = Kept {
            live: Vec::new(),
            retry: Vec::new(),
            already: true,
        };
        let said = allowed_said(4, Level::Project, &already);
        assert!(
            said.starts_with("It was allowed already, for everyone in this project."),
            "{said}"
        );
        assert!(said.contains("restarts on the same conversation"), "{said}");
    }

    #[test]
    fn keep_blocked_answers_the_block_it_showed_and_tells_the_chat_in_fixed_words() {
        let chats = crate::chats::Chats::new();
        let told: std::sync::Arc<
            std::sync::Mutex<Vec<(u32, purlis_core::dispatchtalk::NetworkWord)>>,
        > = std::sync::Arc::default();
        let keep = std::sync::Arc::clone(&told);
        chats.tell_network_words_to(std::sync::Arc::new(move |session, word| {
            keep.lock().unwrap().push((session, word));
        }));
        let shown = crate::taskblocks::BlockShown {
            operation: "connect".to_owned(),
            kind: "host".to_owned(),
            what: GrantWhat::Host,
            target: "paste.example:8443".to_owned(),
        };
        // Not held on it: refused whole, and the chat is told nothing.
        assert!(keep_block(&chats, 4, &shown).is_err());
        assert!(told.lock().unwrap().is_empty());
        chats.blocks().heard(
            4,
            crate::taskblocks::HeldBlock {
                operation: "connect".to_owned(),
                kind: "host".to_owned(),
                what: GrantWhat::Host,
                target: "paste.example:8443".to_owned(),
            },
        );
        keep_block(&chats, 4, &shown).expect("kept blocked");
        assert_eq!(
            *told.lock().unwrap(),
            vec![(
                4,
                purlis_core::dispatchtalk::NetworkWord::KeptBlocked {
                    hosts: vec!["paste.example:8443".to_owned()],
                },
            )]
        );
        // Answered: a second press finds nothing held.
        assert!(keep_block(&chats, 4, &shown).is_err());
    }

    // ---- an administrator's policy (#1343), through the test build's seam ----

    #[test]
    fn a_persona_s_hosts_locked_by_policy_say_who_locked_them_and_no_policy_reads_none() {
        use sandbox::policy::{Locks, set_for_this_test};
        let root = std::path::Path::new("/home/dev/plane");
        // A test build never reads this machine's file: no policy until a test sets one.
        assert_eq!(persona_hosts_locked(root), None);
        assert_eq!(state_on_policy(root), None);
        set_for_this_test(Locks::parse(
            r#"{"owner": "IT", "sandbox": {"persona-hosts": false}}"#,
            std::path::Path::new("/etc/purlis/policy.json"),
        ));
        let said = persona_hosts_locked(root).expect("locked");
        set_for_this_test(Locks::none());
        assert_eq!(
            said,
            "Policy forbids a persona's own hosts. Locked by policy, set by IT in \
             /etc/purlis/policy.json."
        );
    }

    /// This thread's policy until it is dropped, a panic included: no later test on the thread
    /// is left under it.
    struct Under;

    impl Under {
        fn policy(json: &str) -> Self {
            sandbox::policy::set_for_this_test(sandbox::policy::Locks::parse(
                json,
                std::path::Path::new("/etc/purlis/policy.json"),
            ));
            Self
        }
    }

    impl Drop for Under {
        fn drop(&mut self) {
            sandbox::policy::set_for_this_test(sandbox::policy::Locks::none());
        }
    }

    /// D-1423-1: a policy that forbids the opt-out requires the sandbox, so a project that has
    /// not turned it on reads as on, is offered nothing, and says who requires it.
    #[test]
    fn a_project_with_no_sandbox_reads_as_on_and_required_where_policy_forbids_the_opt_out() {
        let project = tempfile::tempdir().expect("a project");
        std::fs::write(project.path().join("charter.toml"), "schema = 1\n").expect("toml");
        let free = state_of(project.path());
        assert!(!free.on && free.offer, "{free:?}");
        {
            let _under = Under::policy(r#"{"owner": "IT", "sandbox": {"opt-out": false}}"#);
            let state = state_of(project.path());
            assert!(state.on, "{state:?}");
            assert!(!state.offer, "what is on already is not offered");
            assert_eq!(
                state.policy.expect("a policy").required.as_deref(),
                Some("On, required by policy, set by IT in /etc/purlis/policy.json.")
            );
        }
        // A policy that locks something else requires nothing.
        let _under = Under::policy(r#"{"owner": "IT", "sandbox": {"write-grants": false}}"#);
        let state = state_of(project.path());
        assert!(!state.on, "{state:?}");
        assert_eq!(state.policy.expect("a policy").required, None);
    }

    /// #1423: what is kept on this machine and policy now drops is marked, never listed as
    /// granted: a folder under a write-grants lock.
    #[test]
    fn a_kept_grant_policy_now_drops_is_marked_locked_on_the_granted_list() {
        let project = tempfile::tempdir().expect("a project");
        let folder = project.path().join("out");
        sandbox::local::grant_write(project.path(), &folder).expect("kept");
        let chats = crate::chats::Chats::new();
        let free = grants_of(project.path(), &chats);
        assert_eq!(free.len(), 1, "{free:?}");
        assert_eq!(free[0].locked, None);
        let _under = Under::policy(r#"{"owner": "IT", "sandbox": {"write-grants": false}}"#);
        let held = grants_of(project.path(), &chats);
        assert_eq!(held[0].what, GrantWhat::Write);
        assert_eq!(
            held[0].locked.as_deref(),
            Some(
                "Policy forbids allowing a chat to write a folder. Locked by policy, set by IT \
                 in /etc/purlis/policy.json."
            )
        );
    }

    /// What Settings is told of the policy for the project at `root`.
    fn state_on_policy(root: &std::path::Path) -> Option<SandboxPolicy> {
        SandboxPolicy::of(&sandbox::policy::Locks::of(root))
    }

    // ---- a block's Allow, and the Granted list (#1342, #1348) ----

    fn no_audit() -> impl Fn(Option<u32>, &sandbox::grant::Audited<'_>) -> Result<(), String> {
        |_, _| Err("the event log is not open".to_owned())
    }

    fn machine() -> sandbox::Machine {
        sandbox::Machine {
            env: purlis_core::secrets::Env::of(&[]),
            home: Some(std::path::PathBuf::from("/nohome/dev")),
            os: sandbox::Os::MacOs,
        }
    }

    /// #1681: a chat taking its own persona's grants is audited, as the chat's persona's hosts
    /// for that chat, before anything is taken; an audit that fails takes nothing, and a chat
    /// holding no other's is neither audited nor taken.
    #[test]
    fn a_chat_taking_its_own_persona_s_grants_is_audited_first() {
        let seen = std::sync::Mutex::new(Vec::new());
        let audit = |chat: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
            seen.lock().unwrap().push((
                chat,
                audited.granted,
                audited.what.to_owned(),
                audited.target.to_owned(),
                audited.level.word(),
            ));
            Ok(())
        };
        let taken = std::cell::Cell::new(0);
        let take = || {
            taken.set(taken.get() + 1);
            true
        };
        assert_eq!(
            own_grants(8, Some(Some("devops".to_owned())), &audit, take),
            Ok(true)
        );
        assert_eq!(
            *seen.lock().unwrap(),
            [(
                Some(8),
                true,
                "persona-hosts".to_owned(),
                "devops".to_owned(),
                "chat"
            )]
        );
        assert_eq!(own_grants(9, None, &audit, take), Ok(false));
        assert!(own_grants(8, Some(None), &no_audit(), take).is_err());
        assert_eq!(taken.get(), 1, "taken only once audited");
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

    #[test]
    fn nothing_is_allowed_that_was_not_audited_or_that_a_grant_cannot_name() {
        let project = tempfile::tempdir().expect("a project");
        let chats = crate::chats::Chats::new();
        let refused = |what, target: &str, level| {
            allow(
                project.path(),
                &machine(),
                &chats,
                3,
                (what, target, level),
                &no_audit(),
                100,
            )
            .expect_err("refused")
        };
        assert!(
            refused(GrantWhat::Write, "/tmp/x", GrantLevel::Project)
                .contains("everyone in the project")
        );
        assert!(
            refused(GrantWhat::Host, "169.254.169.254", GrantLevel::You).contains("link-local")
        );
        // A write is judged against the chat's own folder, so a chat that is not open gets none.
        assert!(refused(GrantWhat::Write, "/tmp/x", GrantLevel::Chat).contains("not open"));
        // An unaudited grant is no grant: nothing is kept at any level.
        assert!(refused(GrantWhat::Host, "api.example.com", GrantLevel::You).contains("event log"));
        assert!(sandbox::hosts::personal(project.path()).is_empty());
        assert!(grants_of(project.path(), &chats).is_empty());
    }

    /// ADR 0093 (D-sock-2, D-sock-3): a socket or a service is this machine's, so never allowed
    /// for everyone in the project; once is a socket's or a service's alone; and each is judged
    /// again, never taken on the window's word.
    #[test]
    fn a_socket_or_a_service_is_never_the_projects_and_once_is_theirs_alone() {
        let project = tempfile::tempdir().expect("a project");
        let chats = crate::chats::Chats::new();
        let refused = |what, target: &str, level| {
            allow(
                project.path(),
                &machine(),
                &chats,
                3,
                (what, target, level),
                &no_audit(),
                100,
            )
            .expect_err("refused")
        };
        for what in [GrantWhat::Socket, GrantWhat::Service] {
            assert!(
                refused(what, "/var/run/docker.sock", GrantLevel::Project)
                    .contains("committed file that granted one"),
                "{what:?}"
            );
        }
        assert!(refused(GrantWhat::Host, "api.example.com", GrantLevel::Once).contains("not once"));
        assert!(refused(GrantWhat::Write, "/tmp/x", GrantLevel::Once).contains("not once"));
        // Not a socket on this machine now: nothing to allow.
        assert!(
            refused(GrantWhat::Socket, "/nowhere/docker.sock", GrantLevel::You)
                .contains("not a local socket")
        );
        assert!(
            refused(
                GrantWhat::Service,
                "com.apple.SecurityServer",
                GrantLevel::You
            )
            .contains("vault")
        );
        // An unaudited grant is no grant.
        assert!(
            refused(GrantWhat::Service, "com.example.helper", GrantLevel::You)
                .contains("event log")
        );
        assert!(sandbox::local::granted_services(project.path()).is_empty());
    }

    /// R1 (2026-10-13): Settings says what a chat's next start has. On by default with
    /// nothing allowed by anyone; off where the project's file says `false`; and nothing is
    /// said where the sandbox is off.
    #[test]
    fn settings_says_certificate_checks_are_on_unless_the_project_turns_them_off() {
        let said = |text: &str| {
            let project = tempfile::tempdir().expect("a project");
            std::fs::write(project.path().join("charter.toml"), text).expect("toml");
            state_on(project.path(), sandbox::Os::MacOs).certificate_check
        };
        let on = Some(CertificateCheck {
            on: true,
            locked: None,
            widens: true,
        });
        assert_eq!(said("[sandbox]\nmode = \"on\"\n"), on);
        assert_eq!(
            said("[sandbox]\nmode = \"on\"\ncertificate-checks = true\n"),
            on
        );
        assert_eq!(
            said("[sandbox]\nmode = \"on\"\ncertificate-checks = false\n"),
            Some(CertificateCheck {
                on: false,
                locked: None,
                widens: true,
            })
        );
        assert_eq!(said(""), None);
        // Off a Mac the setting widens nothing, and Settings can say so.
        let project = tempfile::tempdir().expect("a project");
        std::fs::write(
            project.path().join("charter.toml"),
            "[sandbox]\nmode = \"on\"\n",
        )
        .expect("toml");
        assert_eq!(
            state_on(project.path(), sandbox::Os::Linux).certificate_check,
            Some(CertificateCheck {
                on: true,
                locked: None,
                widens: false,
            })
        );
    }

    /// R1: an administrator's `socket-grants` lock turns certificate checks off whatever the
    /// project says, and Settings says who set it.
    #[test]
    fn settings_says_policy_turned_certificate_checks_off_and_who() {
        let locks = sandbox::policy::Locks::parse(
            r#"{"owner": "IT", "sandbox": {"socket-grants": false}}"#,
            std::path::Path::new("/etc/purlis/policy.json"),
        );
        let said = CertificateCheck::of(
            sandbox::CertificateChecks::of(&sandbox::Policy::defaults(), &locks),
            sandbox::Os::MacOs,
        );
        assert!(!said.on);
        let why = said.locked.expect("said");
        assert!(why.contains("Policy turns certificate checks off"), "{why}");
        assert!(why.contains("IT"), "{why}");
    }

    /// R1: the certificate check is no grant, so nothing allows it by name: not a block's
    /// Allow at any level, and one an older build kept is not listed as allowed.
    #[test]
    fn the_certificate_check_is_never_granted_by_name_and_an_old_grant_is_not_listed() {
        use sandbox::services::TRUSTD;
        let project = tempfile::tempdir().expect("a project");
        let root = project.path();
        std::fs::write(root.join("charter.toml"), "[sandbox]\nmode = \"on\"\n").expect("toml");
        for level in [GrantLevel::Once, GrantLevel::Chat, GrantLevel::You] {
            let refused = judged(
                root,
                &sandbox::Machine::this(),
                Some(root),
                1,
                (GrantWhat::Service, TRUSTD, level),
            )
            .expect_err("refused");
            assert!(refused.contains("Settings"), "{refused}");
        }
        sandbox::local::grant_service(root, TRUSTD).expect("an older build's");
        let chats = crate::chats::Chats::new();
        assert!(
            !grants_of(root, &chats)
                .iter()
                .any(|one| one.target == TRUSTD),
            "it grants nothing, so it is not listed"
        );
    }

    /// ADR 0093, ruling 2: what a project declares it expects is asked about up front and
    /// grants nothing by itself. Each state: declared and not allowed (asked, with each item's
    /// own words); "Not now" (remembered here, still offered in Settings); a new declared item
    /// (asked again); Allow (audited, kept in this machine's file for this project, and what
    /// cannot be allowed here is left out and said); and policy's lock (nothing is allowed,
    /// and who set it is said).
    /// A path a repository declared may be a link to another socket. The window is told the
    /// socket an Allow would keep, as the kernel names it, beside the path as written.
    #[test]
    fn a_declared_socket_is_named_as_the_grant_would_name_it() {
        use sandbox::grant::What;
        use sandbox::services::{Awaited, Awaiting};
        let said: ProjectExpects = Awaiting {
            items: vec![
                Awaited {
                    what: What::Socket("/Users/op/.docker/run/docker.sock".into()),
                    written: "/opt/tools/harmless.sock".to_owned(),
                    said: "said".to_owned(),
                    refused: None,
                },
                Awaited {
                    what: What::Service("com.example.helper".to_owned()),
                    written: "com.example.helper".to_owned(),
                    said: "said".to_owned(),
                    refused: None,
                },
            ],
            locked: None,
            asked: true,
        }
        .into();
        assert_eq!(
            said.items
                .iter()
                .map(|item| (item.named.as_str(), item.target.as_str()))
                .collect::<Vec<_>>(),
            [
                (
                    "/Users/op/.docker/run/docker.sock",
                    "/opt/tools/harmless.sock"
                ),
                ("com.example.helper", "com.example.helper"),
            ]
        );
    }

    #[test]
    fn what_a_project_expects_is_asked_once_allowed_by_the_person_and_never_by_the_file() {
        let project = tempfile::tempdir().expect("a project");
        let root = project.path();
        let expecting = |expects: &str| {
            std::fs::write(
                root.join("charter.toml"),
                format!("[sandbox]\nmode = \"on\"\nexpects = [{expects}]\n"),
            )
            .expect("toml");
        };
        let chats = crate::chats::Chats::new();
        let seen = std::sync::Mutex::new(Vec::new());
        let audit = |chat: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
            seen.lock().unwrap().push((
                chat,
                audited.granted,
                audited.what.to_owned(),
                audited.target.to_owned(),
                audited.level.word(),
            ));
            Ok(())
        };
        std::fs::write(root.join("charter.toml"), "[sandbox]\nmode = \"on\"\n").expect("toml");
        assert_eq!(state_of(root).expects, None, "nothing declared");

        expecting("\"com.example.helper\", \"/purlis-test-nowhere/docker.sock\"");
        let said = state_of(root).expects.expect("asked");
        assert!(said.asked);
        assert_eq!(said.locked, None);
        assert_eq!(
            said.items
                .iter()
                .map(|item| (item.what, item.target.as_str(), item.refused.is_some()))
                .collect::<Vec<_>>(),
            [
                (GrantWhat::Socket, "/purlis-test-nowhere/docker.sock", true),
                (GrantWhat::Service, "com.example.helper", false),
            ]
        );
        assert!(said.items[0].says.contains("about the same as no sandbox"));
        // The declaration allowed nothing.
        assert!(sandbox::local::granted_services(root).is_empty());

        // Not now: remembered, still awaited, and nothing is allowed or audited.
        let shown: Vec<String> = said.items.iter().map(|item| item.target.clone()).collect();
        let declined = decline_expects(root, &shown)
            .expect("kept")
            .expects
            .expect("awaited");
        assert!(!declined.asked);
        assert_eq!(declined.items, said.items);
        assert!(seen.lock().unwrap().is_empty());
        assert!(sandbox::local::granted_services(root).is_empty());

        // The declaration changes: asked again.
        expecting(
            "\"com.example.helper\", \"/purlis-test-nowhere/docker.sock\", \"com.example.other\"",
        );
        assert!(state_of(root).expects.expect("asked").asked);

        // An unaudited Allow is no Allow.
        let all = [
            "com.example.helper".to_owned(),
            "/purlis-test-nowhere/docker.sock".to_owned(),
            "com.example.other".to_owned(),
        ];
        assert!(
            allow_expects(root, &chats, &all, &no_audit(), 100)
                .expect_err("refused")
                .contains("event log")
        );
        assert!(sandbox::local::granted_services(root).is_empty());
        // Allow, for what was shown: only that, only what the file declares, and never what
        // cannot be allowed here. Something the window names that the file does not is nothing.
        let shown = [
            "com.example.helper".to_owned(),
            "/purlis-test-nowhere/docker.sock".to_owned(),
            "com.example.never-declared".to_owned(),
        ];
        allow_expects(root, &chats, &shown, &audit, 100).expect("allowed");
        assert_eq!(
            sandbox::local::granted_services(root),
            ["com.example.helper"]
        );
        assert!(sandbox::local::granted_sockets(root).is_empty());
        assert_eq!(
            *seen.lock().unwrap(),
            [(
                None,
                true,
                "service".to_owned(),
                "com.example.helper".to_owned(),
                "you"
            )]
        );
        // What is left still awaits: the socket that is not there, answered, and the item
        // that was not shown, which is still to be asked about. An answer covers what the
        // person saw and nothing declared since.
        let left = state_of(root).expects.expect("awaited");
        assert_eq!(left.items.len(), 2);
        assert!(left.asked, "an item nobody was shown is not answered");
        assert_eq!(
            sandbox::local::expects_seen(root),
            ["/purlis-test-nowhere/docker.sock", "com.example.helper"]
        );
        // Nothing shown that can be allowed: said, and nothing kept.
        assert!(
            allow_expects(
                root,
                &chats,
                &["/purlis-test-nowhere/docker.sock".to_owned()],
                &audit,
                101
            )
            .expect_err("refused")
            .contains("nothing was allowed")
        );

        // Policy's lock: nothing is allowed, and the state says who set it.
        sandbox::policy::set_for_this_test(sandbox::policy::Locks::parse(
            r#"{"owner": "IT", "sandbox": {"socket-grants": false}}"#,
            std::path::Path::new("/etc/purlis/policy.json"),
        ));
        let locked = state_of(root).expects.expect("awaited");
        let refused = allow_expects(root, &chats, &["com.example.other".to_owned()], &audit, 102);
        sandbox::policy::set_for_this_test(sandbox::policy::Locks::none());
        let why = locked.locked.expect("locked");
        assert!(
            why.contains("Policy forbids") && why.contains("IT"),
            "{why}"
        );
        assert_eq!(refused.expect_err("refused"), why);
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

    /// R2 (2026-10-13): a socket or a service allowed "on this machine, for every project" is
    /// judged as any grant is, audited once, kept in this machine's one file (never the
    /// project's, nor under its clone's key), listed in every project's Allowed list with
    /// Remove, and taken out of them all by one Remove. A host or a folder is never kept there.
    #[test]
    fn a_socket_or_a_service_is_allowed_for_every_project_listed_in_each_and_removed_once() {
        use sandbox::grant::{Level, What};
        let everywhere = tempfile::tempdir().expect("a config home");
        sandbox::local::everywhere_for_this_test(Some(
            everywhere.path().join(sandbox::local::EVERY_PROJECT),
        ));
        let project = tempfile::tempdir().expect("a project");
        let other = tempfile::tempdir().expect("another project");
        for root in [project.path(), other.path()] {
            std::fs::write(root.join("charter.toml"), "[sandbox]\nmode = \"on\"\n").expect("toml");
        }
        let root = project.path();
        let chats = crate::chats::Chats::new();
        let seen = std::sync::Mutex::new(Vec::new());
        let audit = |chat: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
            seen.lock().unwrap().push((
                chat,
                audited.granted,
                audited.what.to_owned(),
                audited.target.to_owned(),
                audited.level.word(),
            ));
            Ok(())
        };
        // Judged by the core at this level: a service by its whole name, never the
        // certificate check's, and never a host or a folder.
        let judge = |what, target: &str| {
            judged(
                root,
                &sandbox::Machine::this(),
                Some(root),
                1,
                (what, target, GrantLevel::Machine),
            )
        };
        let (service, level) = judge(GrantWhat::Service, "com.example.helper").expect("judged");
        assert_eq!(level, Level::Machine);
        assert!(
            judge(GrantWhat::Host, "api.example.com")
                .expect_err("refused")
                .contains("only a local socket or a system service")
        );
        assert!(judge(GrantWhat::Service, sandbox::services::TRUSTD).is_err());
        // An unaudited Allow is no Allow.
        assert!(
            kept_for(root, &chats, None, (&service, level), &no_audit(), 100)
                .expect_err("refused")
                .contains("event log")
        );
        assert!(sandbox::local::services_everywhere().is_empty());

        let socket = What::Socket("/purlis-test-nowhere/docker.sock".into());
        for what in [&service, &socket] {
            let kept = kept_for(root, &chats, None, (what, level), &audit, 100).expect("kept");
            assert!(!kept.already);
        }
        // Allowed twice is kept once, and audited once.
        assert!(
            kept_for(root, &chats, None, (&socket, level), &audit, 101)
                .expect("allowed already")
                .already
        );
        assert_eq!(
            *seen.lock().unwrap(),
            [
                (
                    None,
                    true,
                    "service".to_owned(),
                    "com.example.helper".to_owned(),
                    "machine"
                ),
                (
                    None,
                    true,
                    "socket".to_owned(),
                    "/purlis-test-nowhere/docker.sock".to_owned(),
                    "machine"
                ),
            ]
        );
        // This machine's one file: nothing in the project, nothing under its clone's key.
        assert!(sandbox::local::granted_sockets(root).is_empty());
        assert!(sandbox::local::granted_services(root).is_empty());
        assert!(sandbox::local::everywhere_path().starts_with(everywhere.path()));
        // A host or a folder is refused there whoever asks.
        let host = What::Host(sandbox::grant::host("api.example.com").expect("a host"));
        assert!(kept_for(root, &chats, None, (&host, level), &audit, 102).is_err());

        // Listed in each project, at that level, with when it was allowed.
        let listed_in =
            |root: &std::path::Path| -> Vec<(GrantWhat, String, GrantLevel, Option<u32>)> {
                grants_of(root, &chats)
                    .into_iter()
                    .filter(|one| one.level == GrantLevel::Machine)
                    .map(|one| (one.what, one.target, one.level, one.at))
                    .collect()
            };
        for root in [project.path(), other.path()] {
            assert_eq!(
                listed_in(root),
                [
                    (
                        GrantWhat::Socket,
                        "/purlis-test-nowhere/docker.sock".to_owned(),
                        GrantLevel::Machine,
                        Some(100)
                    ),
                    (
                        GrantWhat::Service,
                        "com.example.helper".to_owned(),
                        GrantLevel::Machine,
                        Some(100)
                    ),
                ]
            );
        }
        // Remove, from the other project's list: gone from both, audited, with its record.
        let id = grants_of(other.path(), &chats)
            .into_iter()
            .find(|one| one.level == GrantLevel::Machine && one.what == GrantWhat::Socket)
            .expect("listed")
            .id;
        revoke(other.path(), &chats, &id, &audit).expect("removed");
        assert!(sandbox::local::sockets_everywhere().is_empty());
        assert_eq!(listed_in(root).len(), 1);
        assert_eq!(sandbox::local::made_everywhere().len(), 1);
        let last = seen.lock().unwrap().last().cloned().expect("audited");
        assert_eq!((last.1, last.4), (false, "machine"));
        // Removed twice, it is no longer there.
        assert!(revoke(other.path(), &chats, &id, &audit).is_err());
        sandbox::local::everywhere_for_this_test(None);
    }

    #[test]
    fn a_vault_you_allowed_a_persona_is_listed_and_revoke_takes_it_out_audited_once() {
        let project = tempfile::tempdir().expect("a project");
        let root = project.path();
        let chats = crate::chats::Chats::new();
        sandbox::local::grant_vault(root, "devops", "steward").expect("granted");
        sandbox::local::record_made(
            root,
            sandbox::local::Made {
                what: "vault".to_owned(),
                target: "devops for steward".to_owned(),
                level: "you".to_owned(),
                at: 42,
                chat: Some("steward 1".to_owned()),
            },
        )
        .expect("recorded");
        let listed = grants_of(root, &chats);
        assert_eq!(
            listed,
            [SandboxGrant {
                id: "you\u{1f}vault\u{1f}devops\u{1f}steward".to_owned(),
                what: GrantWhat::Vault,
                target: "devops".to_owned(),
                persona: Some("steward".to_owned()),
                level: GrantLevel::You,
                by: None,
                at: Some(42),
                chat: Some("steward 1".to_owned()),
                locked: None,
                waiting: None,
                for_no_persona: false,
                expected: false,
            }]
        );
        // A block's Allow never grants one: only the refused vault's own Notice does.
        let from_a_block = allow(
            root,
            &machine(),
            &chats,
            3,
            (GrantWhat::Vault, "devops", GrantLevel::You),
            &no_audit(),
            100,
        )
        .expect_err("refused");
        assert!(
            from_a_block.contains("notice its refusal raises"),
            "{from_a_block}"
        );
        // Not audited, not revoked.
        assert!(revoke(root, &chats, &listed[0].id, &no_audit()).is_err());
        assert_eq!(grants_of(root, &chats).len(), 1);

        let heard = std::sync::Mutex::new(Vec::new());
        let audit = |number: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
            heard.lock().unwrap().push((
                number,
                audited.kind(),
                audited.what.to_owned(),
                audited.target.to_owned(),
            ));
            Ok(())
        };
        revoke(root, &chats, &listed[0].id, &audit).expect("revoked");
        assert!(grants_of(root, &chats).is_empty());
        assert!(sandbox::local::granted_vaults(root).is_empty());
        assert!(sandbox::local::made(root).is_empty());
        let again = revoke(root, &chats, &listed[0].id, &audit).expect_err("not there");
        assert!(again.contains("no longer there"), "{again}");
        assert_eq!(
            *heard.lock().unwrap(),
            [(
                None,
                "trust.sandbox.revoke",
                "vault".to_owned(),
                "devops for steward".to_owned()
            )],
            "one audit, for the one revoke that happened"
        );
    }

    /// #1362, D-1362-7: a persona's committed hosts reach nothing here until the person allows
    /// them as shown; the Allow is audited, listed credited to the persona, and revocable.
    #[test]
    fn a_persona_s_hosts_are_allowed_as_shown_listed_for_the_persona_and_revoked() {
        let project = tempfile::tempdir().expect("a project");
        let root = project.path();
        std::fs::write(
            root.join("charter.toml"),
            "schema = 1\n\n[sandbox]\nmode = \"on\"\n\n[sandbox.personas.devops]\n\
             hosts = [\"10.0.0.5:6443\", \"*.internal.example\"]\n",
        )
        .expect("toml");
        let chats = crate::chats::Chats::new();
        let state = state_of(root);
        assert_eq!(state.persona_hosts.len(), 1);
        let shown = state.persona_hosts[0].clone();
        assert!(!shown.allowed);
        assert!(grants_of(root, &chats).is_empty());

        // A digest that is not the list's as it stands allows nothing.
        let stale = allow_persona(root, "devops", "not-what-stands", &no_audit(), 7)
            .expect_err("changed since");
        assert!(stale.contains("changed after they were shown"), "{stale}");
        // An Allow that is not audited is no Allow.
        assert!(allow_persona(root, "devops", &shown.digest, &no_audit(), 7).is_err());
        assert!(!state_of(root).persona_hosts[0].allowed);

        let heard = std::sync::Mutex::new(Vec::new());
        let audit = |number: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
            heard.lock().unwrap().push((
                number,
                audited.kind(),
                audited.what.to_owned(),
                audited.target.to_owned(),
            ));
            Ok(())
        };
        allow_persona(root, "devops", &shown.digest, &audit, 42).expect("allowed");
        assert!(state_of(root).persona_hosts[0].allowed);
        let listed = grants_of(root, &chats);
        assert_eq!(
            listed,
            [SandboxGrant {
                id: "you\u{1f}persona-hosts\u{1f}devops".to_owned(),
                what: GrantWhat::PersonaHosts,
                target: "10.0.0.5:6443, *.internal.example".to_owned(),
                persona: Some("devops".to_owned()),
                level: GrantLevel::You,
                by: None,
                at: Some(42),
                chat: None,
                locked: None,
                waiting: None,
                for_no_persona: false,
                expected: false,
            }]
        );
        // A teammate changes the list: the Allow grants nothing, and is listed as waiting.
        std::fs::write(
            root.join("charter.toml"),
            "schema = 1\n\n[sandbox]\nmode = \"on\"\n\n[sandbox.personas.devops]\n\
             hosts = [\"10.0.0.5:6443\"]\n",
        )
        .expect("toml");
        let now = state_of(root).persona_hosts[0].clone();
        assert!(!now.allowed);
        assert!(now.waiting);
        let waiting = grants_of(root, &chats);
        assert_eq!(waiting.len(), 1);
        assert_eq!(waiting[0].id, listed[0].id);
        assert!(
            waiting[0]
                .waiting
                .as_deref()
                .is_some_and(|why| why.starts_with("waiting: the list changed")),
            "{waiting:?}"
        );
        // Put back as it was allowed: it stays waiting until it is allowed anew (D-1362-13).
        std::fs::write(
            root.join("charter.toml"),
            "schema = 1\n\n[sandbox]\nmode = \"on\"\n\n[sandbox.personas.devops]\n\
             hosts = [\"10.0.0.5:6443\", \"*.internal.example\"]\n",
        )
        .expect("toml");
        assert!(state_of(root).persona_hosts[0].waiting);
        assert!(grants_of(root, &chats)[0].waiting.is_some());
        revoke(root, &chats, &listed[0].id, &audit).expect("revoked");
        assert!(grants_of(root, &chats).is_empty());
        assert!(!state_of(root).persona_hosts[0].allowed);
        assert!(!state_of(root).persona_hosts[0].waiting);
        assert_eq!(
            *heard.lock().unwrap(),
            [
                (
                    None,
                    "trust.sandbox.grant",
                    "persona-hosts".to_owned(),
                    "devops: 10.0.0.5:6443, *.internal.example".to_owned()
                ),
                (
                    None,
                    "trust.sandbox.revoke",
                    "persona-hosts".to_owned(),
                    "devops".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn a_vault_grant_policy_forbids_is_listed_locked() {
        use sandbox::policy::{Locks, set_for_this_test};
        let project = tempfile::tempdir().expect("a project");
        let root = project.path();
        sandbox::local::grant_vault(root, "devops", "steward").expect("granted");
        set_for_this_test(Locks::parse(
            r#"{"owner": "IT", "sandbox": {"vault-grants": false}}"#,
            std::path::Path::new("/etc/purlis/policy.json"),
        ));
        let listed = grants_of(root, &crate::chats::Chats::new());
        set_for_this_test(Locks::none());
        assert_eq!(
            listed[0].locked.as_deref(),
            Some(
                "Policy forbids allowing a persona a vault it is not tagged for. Locked by \
                 policy, set by IT in /etc/purlis/policy.json."
            )
        );
    }

    /// #1431: an entry policy locks out is kept, and a person may still take it away: Revoke
    /// works on it while the policy stands, audited as any other.
    #[test]
    fn a_grant_policy_locks_out_is_still_revoked() {
        let project = tempfile::tempdir().expect("a project");
        let root = project.path();
        let chats = crate::chats::Chats::new();
        sandbox::local::grant_write(root, std::path::Path::new("/tmp/tool-cache")).expect("kept");
        sandbox::local::grant_vault(root, "devops", "steward").expect("kept");
        let _under = Under::policy(
            r#"{"owner": "IT", "sandbox": {"write-grants": false, "vault-grants": false}}"#,
        );
        let listed = grants_of(root, &chats);
        assert_eq!(listed.len(), 2, "{listed:?}");
        assert!(listed.iter().all(|one| one.locked.is_some()), "{listed:?}");
        let heard = std::sync::Mutex::new(0);
        let audit = |_: Option<u32>, _: &sandbox::grant::Audited<'_>| {
            *heard.lock().unwrap() += 1;
            Ok(())
        };
        for one in &listed {
            revoke(root, &chats, &one.id, &audit).expect("revoked");
        }
        assert!(grants_of(root, &chats).is_empty());
        assert_eq!(*heard.lock().unwrap(), 2);
    }

    #[test]
    fn a_folder_you_were_granted_is_listed_and_revoke_takes_it_out_audited_once() {
        let project = tempfile::tempdir().expect("a project");
        let root = project.path();
        let chats = crate::chats::Chats::new();
        let folder = std::path::PathBuf::from("/tmp/tool-cache");
        sandbox::local::grant_write(root, &folder).expect("granted");
        sandbox::local::record_made(
            root,
            sandbox::local::Made {
                what: "write".to_owned(),
                target: "/tmp/tool-cache".to_owned(),
                level: "you".to_owned(),
                at: 42,
                chat: None,
            },
        )
        .expect("recorded");
        let listed = grants_of(root, &chats);
        assert_eq!(
            listed,
            [SandboxGrant {
                id: "you\u{1f}write\u{1f}/tmp/tool-cache".to_owned(),
                what: GrantWhat::Write,
                target: "/tmp/tool-cache".to_owned(),
                persona: None,
                level: GrantLevel::You,
                by: None,
                at: Some(42),
                chat: None,
                locked: None,
                waiting: None,
                for_no_persona: false,
                expected: false,
            }]
        );
        // Not audited, not revoked.
        assert!(revoke(root, &chats, &listed[0].id, &no_audit()).is_err());
        assert_eq!(grants_of(root, &chats).len(), 1);

        let heard = std::sync::Mutex::new(Vec::new());
        let audit = |number: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
            heard
                .lock()
                .unwrap()
                .push((number, audited.kind(), audited.target.to_owned()));
            Ok(())
        };
        revoke(root, &chats, &listed[0].id, &audit).expect("revoked");
        assert!(grants_of(root, &chats).is_empty());
        assert!(sandbox::local::granted_writes(root).is_empty());
        // Revoked twice: the second is refused before anything is audited.
        let again = revoke(root, &chats, &listed[0].id, &audit).expect_err("not there");
        assert!(again.contains("no longer there"), "{again}");
        assert!(revoke(root, &chats, "nonsense", &audit).is_err());
        assert_eq!(
            *heard.lock().unwrap(),
            [(None, "trust.sandbox.revoke", "/tmp/tool-cache".to_owned())],
            "one audit, for the one revoke that happened"
        );
    }

    #[test]
    fn a_folder_you_list_widens_the_allowlist_and_nothing_a_class_or_path_holds() {
        let project = tempfile::tempdir().expect("a project");
        let root = project.path();
        assert_eq!(
            list_folder(root, &machine(), "/noopt/tools"),
            Ok(GrantableListed {
                folders: vec!["/noopt/tools".to_owned()],
                holds: Vec::new(),
            })
        );
        for refused in ["/", "/nohome/dev", "relative"] {
            assert!(list_folder(root, &machine(), refused).is_err(), "{refused}");
        }
        // Listed, and Settings is told what it holds that later code loads (review R3).
        let agents =
            list_folder(root, &machine(), "/nohome/dev/Library/LaunchAgents").expect("listed");
        assert!(
            agents
                .holds
                .iter()
                .any(|one| one.ends_with("Library/LaunchAgents")),
            "{agents:?}"
        );
        // One that holds a harness's own is refused outright.
        assert!(list_folder(root, &machine(), "/nohome/dev/Library").is_err());
        assert_eq!(
            listed_folders(root),
            ["/noopt/tools", "/nohome/dev/Library/LaunchAgents"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_listed_folder_that_now_resolves_elsewhere_is_dropped_and_said() {
        // Review R8: listed, then swapped for a link by whatever could write where it was.
        let project = tempfile::tempdir().expect("a project");
        let root = project.path().canonicalize().expect("real");
        let listed = root.join("tools");
        let out = root.join("elsewhere");
        std::fs::create_dir_all(&listed).expect("made");
        std::fs::create_dir_all(&out).expect("made");
        sandbox::local::list_grantable(&root, &listed).expect("listed");
        assert_eq!(listed_and_pruned(&root).dropped, Vec::<String>::new());
        std::fs::remove_dir(&listed).expect("removed");
        std::os::unix::fs::symlink(&out, &listed).expect("linked");
        let now = listed_and_pruned(&root);
        assert_eq!(now.dropped, [listed.display().to_string()]);
        assert!(now.folders.is_empty());
    }

    #[test]
    fn a_block_report_is_drafted_from_the_fixed_words_alone() {
        let report = sandbox_block_report(
            "write".to_owned(),
            "project-files".to_owned(),
            Some("claude".to_owned()),
        )
        .unwrap();
        assert_eq!(report.repository, purlis_core::report::UPSTREAM);
        assert_eq!(
            report.title,
            "Sandbox blocked purlis's own write (project-files)"
        );
        assert!(
            report.body.contains("- **harness:** claude"),
            "{}",
            report.body
        );
        // A harness word purlis does not start is not one, and says nothing of itself.
        let other = sandbox_block_report(
            "write".to_owned(),
            "project-files".to_owned(),
            Some("/home/dev/my-harness".to_owned()),
        )
        .unwrap();
        assert!(!other.body.contains("/home/dev"), "{}", other.body);
        assert!(
            other.body.contains("- **harness:** not known"),
            "{}",
            other.body
        );
    }

    #[test]
    fn a_block_report_refuses_words_the_sandbox_does_not_report() {
        for (operation, kind) in [
            ("write", "/Users/dev/plane/workspaces/a/sessions"),
            ("rm -rf /", "project-files"),
            ("", ""),
        ] {
            assert!(
                sandbox_block_report(operation.to_owned(), kind.to_owned(), None).is_err(),
                "{operation} {kind}"
            );
        }
    }

    #[test]
    fn only_the_draft_that_was_shown_is_filed() {
        let shown = sandbox_block_report("connect".to_owned(), "host".to_owned(), None).unwrap();
        assert!(shown_draft("connect", "host", None, &shown.digest).is_ok());
        let refused = shown_draft("connect", "host", Some("claude"), &shown.digest).unwrap_err();
        assert!(refused.contains("nothing was sent"), "{refused}");
        assert!(shown_draft("write", "host", None, &shown.digest).is_err());
    }

    #[test]
    fn a_project_without_the_sandbox_shows_the_picker_nothing() {
        assert_eq!(SandboxAhead::of(Ahead::Off), None);
    }

    #[test]
    fn the_picker_is_told_a_refusal_with_its_install_command() {
        assert_eq!(
            SandboxAhead::of(Ahead::Refused {
                why: "socat is not installed".to_owned(),
                install: Some("sudo apt install socat".to_owned()),
            }),
            Some(SandboxAhead {
                state: "refused".to_owned(),
                said: "socat is not installed".to_owned(),
                install: Some("sudo apt install socat".to_owned()),
                locked: None,
            })
        );
    }

    #[test]
    fn on_windows_the_picker_is_told_the_chat_starts_without_it_and_why() {
        let lifted = sandbox::Lifted {
            by: sandbox::By::NoBackend(sandbox::Os::Windows),
            reason: None,
        };
        let row = SandboxAhead::of(Ahead::Unsandboxed(lifted.clone())).expect("a row");
        assert_eq!(row.state, "unsandboxed");
        assert_eq!(row.said, lifted.notice());
    }

    /// Ruling V78 c: the line is the compiled-in command for what is missing, typed and never
    /// run, so it carries no newline.
    #[test]
    fn the_install_line_is_charters_own_command_with_no_return_at_the_end() {
        let missing = backend::missing(sandbox::Os::Linux, &|_| false);
        assert_eq!(
            install_line(missing.as_ref(), "ID=fedora\n").as_deref(),
            Ok("sudo dnf install bubblewrap socat")
        );
        assert!(
            install_line(None, "ID=fedora\n").is_err(),
            "nothing missing"
        );
        assert!(
            install_line(missing.as_ref(), "ID=nixos\n").is_err(),
            "a distribution purlis does not know"
        );
    }

    /// #1340: Settings › Sandbox draws the core's presets, each with the hosts the core lets a
    /// chat in this project reach, the project's forges among code hosting's.
    #[test]
    fn settings_is_handed_every_preset_with_the_hosts_the_core_lists() {
        let plane = sandbox::Plane::of(Some(
            "[sandbox]\nmode = \"on\"\n\n[[forge]]\nkind = \"gitlab\"\nhost = \"git.example.org\"\n",
        ));
        let shown = presets_of(&plane, &sandbox::policy::Locks::none());
        assert_eq!(
            shown
                .iter()
                .map(|one| (one.word.as_str(), one.title.as_str()))
                .collect::<Vec<_>>(),
            [
                ("model-providers", "AI providers"),
                ("forge", "Code hosting"),
                ("toolchains", "Package registries"),
            ]
        );
        for (one, preset) in shown.iter().zip(sandbox::Preset::ALL) {
            assert_eq!(
                one.hosts,
                sandbox::hosts(&[preset], &plane, &sandbox::policy::Locks::none())
            );
        }
        assert!(shown[1].hosts.contains(&"git.example.org".to_owned()));
        assert!(!shown[0].hosts.contains(&"git.example.org".to_owned()));
        // #1422: the core says which preset widens the package caches.
        assert_eq!(
            shown
                .iter()
                .map(|one| one.widens_caches)
                .collect::<Vec<_>>(),
            [false, false, true]
        );
    }

    /// #1340, #1362: a persona's own hosts, as the core read them, and whether the person
    /// allowed them here (D-1362-7).
    #[test]
    fn settings_is_handed_each_personas_own_hosts() {
        let root = tempfile::tempdir().expect("a project");
        let plane = sandbox::Plane::of(Some(
            "[sandbox]\nmode = \"on\"\n\n[sandbox.personas.devops]\nhosts = [\"10.0.0.5:6443\"]\n",
        ));
        let none = sandbox::policy::Locks::none();
        let digest = sandbox::persona::digest(
            &[sandbox::hosts::Host::parse("10.0.0.5:6443").expect("a host")],
            false,
        );
        let devops = |allowed: bool| PersonaHosts {
            persona: "devops".to_owned(),
            hosts: vec!["10.0.0.5:6443".to_owned()],
            reached: vec!["10.0.0.5:6443".to_owned()],
            tools: Vec::new(),
            digest: digest.clone(),
            allowed,
            default: false,
            waiting: false,
        };
        assert_eq!(
            persona_hosts_of(root.path(), &plane, &none),
            [devops(false)]
        );
        sandbox::local::allow_persona_hosts(root.path(), "devops", &digest).expect("kept");
        assert_eq!(persona_hosts_of(root.path(), &plane, &none), [devops(true)]);
        assert!(persona_hosts_of(root.path(), &sandbox::Plane::of(None), &none).is_empty());
    }

    /// #1411 C3 (D-1410e): a project whose own manifest has gone, or cannot be read as TOML,
    /// refuses every chat, so Settings says so as its own state, never as "off", and offers
    /// nothing that would write a manifest in its place.
    #[test]
    fn a_project_whose_manifest_is_gone_or_unreadable_says_chats_are_refused_never_off() {
        let gone = tempfile::tempdir().expect("a project with no manifest");
        let state = state_of(gone.path());
        let said = state.manifest.expect("the gone manifest is said");
        assert!(
            said.contains("is missing") && said.contains("refused until"),
            "{said}"
        );
        assert!(
            !state.offer,
            "no offer writes a manifest in place of the gone one"
        );

        let broken = tempfile::tempdir().expect("a project");
        std::fs::write(broken.path().join("charter.toml"), "[sandbox\nmode = ").expect("toml");
        let state = state_of(broken.path());
        let said = state.manifest.expect("the unreadable manifest is said");
        assert!(
            said.contains("cannot be read") && said.contains("refused until"),
            "{said}"
        );
        assert!(!state.offer);

        let fine = tempfile::tempdir().expect("a project");
        std::fs::write(fine.path().join("charter.toml"), "schema = 1\n").expect("toml");
        assert_eq!(state_of(fine.path()).manifest, None);
    }

    /// D-PE-ruling-1: a persona's tools are shown with its hosts and wait for the same Allow; a
    /// persona with tools alone is asked about too.
    #[test]
    fn a_personas_tools_are_shown_and_allowed_with_its_hosts() {
        let root = tempfile::tempdir().expect("a project");
        let plane = sandbox::Plane::of(Some(
            "[sandbox]\nmode = \"on\"\n\n[sandbox.personas.devops]\ntools = { kubectl = { vault \
             = \"devops\", as = \"KUBECONFIG file\", key = \"kubeconfig\" } }\n",
        ));
        let none = sandbox::policy::Locks::none();
        let shown = persona_hosts_of(root.path(), &plane, &none);
        assert_eq!(shown.len(), 1);
        assert_eq!(
            shown[0].tools,
            ["kubectl with kubeconfig from vault devops as KUBECONFIG file"]
        );
        assert!(shown[0].reached.is_empty());
        assert!(!shown[0].allowed);
        sandbox::local::allow_persona_hosts(root.path(), "devops", &shown[0].digest).expect("kept");
        assert!(persona_hosts_of(root.path(), &plane, &none)[0].allowed);
    }

    #[test]
    fn taking_the_offer_turns_the_project_on_and_it_is_not_offered_again() {
        let project = tempfile::tempdir().expect("a project");
        std::fs::write(project.path().join("charter.toml"), "schema = 1\n").expect("toml");
        assert_eq!(
            state_of(project.path()),
            SandboxState {
                on: false,
                offer: true,
                said: None,
                never: never_here(sandbox::Os::this()),
                hosts_changed: None,
                presets_changed: None,
                presets: presets_of(
                    &sandbox::Plane::read(project.path()),
                    &sandbox::policy::Locks::none(),
                ),
                persona_hosts: Vec::new(),
                besides: SandboxBesides::default(),
                policy: None,
                manifest: None,
                certificate_check: None,
                expects: None,
            }
        );

        let after = answer(project.path(), true).expect("answered");

        assert_eq!(
            after,
            SandboxState {
                on: true,
                offer: false,
                said: Some(
                    "no chat has started under this project's sandbox on this machine yet"
                        .to_owned()
                ),
                never: never_here(sandbox::Os::this()),
                hosts_changed: None,
                presets_changed: None,
                presets: presets_of(
                    &sandbox::Plane::read(project.path()),
                    &sandbox::policy::Locks::none(),
                ),
                persona_hosts: Vec::new(),
                besides: SandboxBesides::default(),
                policy: None,
                manifest: None,
                certificate_check: Some(CertificateCheck {
                    on: true,
                    locked: None,
                    widens: sandbox::Os::this() == sandbox::Os::MacOs,
                }),
                expects: None,
            }
        );
    }

    #[test]
    fn keeping_it_off_leaves_the_project_as_it_was_and_asks_no_more() {
        let project = tempfile::tempdir().expect("a project");
        std::fs::write(project.path().join("charter.toml"), "schema = 1\n").expect("toml");

        let after = answer(project.path(), false).expect("answered");

        assert_eq!(
            after,
            SandboxState {
                on: false,
                offer: false,
                said: None,
                never: never_here(sandbox::Os::this()),
                hosts_changed: None,
                presets_changed: None,
                presets: presets_of(
                    &sandbox::Plane::read(project.path()),
                    &sandbox::policy::Locks::none(),
                ),
                persona_hosts: Vec::new(),
                besides: SandboxBesides::default(),
                policy: None,
                manifest: None,
                certificate_check: None,
                expects: None,
            }
        );
        assert_eq!(
            std::fs::read_to_string(project.path().join("charter.toml")).expect("toml"),
            "schema = 1\n"
        );
    }

    /// Must-fix (verifier r7): the picker never runs a profile's program before the start's
    /// gate lets it start (approved, among the rest). Past the gate, its program is checked,
    /// which runs it. The program answers as something other than Claude Code, so whether it
    /// ran shows in the row: the probe runs inside charter's wrap (D-88k), where it can leave
    /// no other trace.
    #[cfg(unix)]
    #[test]
    fn the_picker_runs_no_program_nobody_approved() {
        let project = tempfile::tempdir().expect("a project");
        // A folder no chat writes: the start refuses a program, or any file its command
        // names, in a temp folder (ruling V87g, D-88g).
        let outside = stand_in::NoChatWrites::new();
        std::fs::write(
            project.path().join("charter.toml"),
            "[sandbox]\nmode = \"on\"\n",
        )
        .expect("charter.toml");
        // Run as `/bin/sh <script>`.
        let script = outside.path().join("claude");
        std::fs::write(&script, "echo 'not claude'\n").expect("the script");
        std::fs::write(
            project.path().join(purlis_core::profiles::LOCAL_FILE),
            format!(
                "[harness.work]\nkind = \"claude\"\ncommand = [\"/bin/sh\", {:?}]\n",
                script.display().to_string()
            ),
        )
        .expect("a profile");
        let set = purlis_core::profiles::current(project.path());
        let profile = set.get("work").expect("declared");

        let row = ahead_here(profile, project.path()).expect("a row");

        // Where this machine can apply the sandbox at all (not a CI runner without bubblewrap):
        // the row says the check waits for the approval, so the program was not run; once
        // approved, it is checked, and its answer refuses it.
        if row.state == "sandboxed" {
            assert_eq!(
                row.said, CHECKED_ONCE_APPROVED,
                "an unapproved program was run"
            );
            purlis_core::profiletrust::record_launched(
                project.path(),
                "work",
                &purlis_core::profiletrust::fingerprint(profile),
            )
            .expect("approved");
            let approved = ahead_here(profile, project.path()).expect("a row");
            assert_eq!(approved.state, "refused", "an approved program is checked");
            assert!(
                approved.said.contains("does not answer as Claude Code"),
                "{approved:?}"
            );
        }
    }

    /// #1341: a teammate is told once of the project's hosts, and not again once it was read.
    #[test]
    fn the_project_s_hosts_are_told_once_and_not_again_once_read() {
        let project = tempfile::tempdir().expect("a project");
        std::fs::write(
            project.path().join("charter.toml"),
            "[sandbox]\nmode = \"on\"\nhosts = [\"10.100.39.145:6443\"]\n",
        )
        .expect("toml");
        let told = state_of(project.path()).hosts_changed.expect("told");
        assert_eq!(told.added, ["10.100.39.145:6443"]);
        assert_eq!(told.removed, Vec::<String>::new());

        let after = acknowledge(project.path(), &told.now).expect("read");

        assert_eq!(after.hosts_changed, None);
    }

    /// #1385: a teammate is told once of a preset that widens, and not again once it was read.
    #[test]
    fn a_widening_preset_is_told_once_and_not_again_once_read() {
        let project = tempfile::tempdir().expect("a project");
        std::fs::write(
            project.path().join("charter.toml"),
            "[sandbox]\nmode = \"on\"\n",
        )
        .expect("toml");
        // A default project tells nothing: certificate checks are among the defaults (R1).
        assert_eq!(state_of(project.path()).presets_changed, None);
        // This machine last saw the project before they were: told once, with the widening.
        sandbox::local::acknowledge_presets(
            project.path(),
            &["model-providers", "forge", "toolchains"].map(str::to_owned),
        )
        .expect("seen");
        let told = state_of(project.path()).presets_changed.expect("told");
        assert_eq!(told.added, ["Certificate checks"]);
        assert_eq!(told.widens.len(), 1);

        let after = acknowledge_presets(project.path(), &told.now).expect("read");

        assert_eq!(after.presets_changed, None);
    }

    /// Fold-in (round 11): the offer never reads as covering a harness that is never sandboxed
    /// on this system, whatever the project.
    #[test]
    fn the_offer_names_each_harness_never_sandboxed_on_this_system() {
        // #1123: charter wraps Codex as it wraps opencode, on macOS so far.
        assert!(never_here(sandbox::Os::MacOs).is_empty());
        assert_eq!(
            never_here(sandbox::Os::Linux),
            [
                "Codex: purlis can wrap it on macOS only, so far",
                "opencode: purlis can wrap it on macOS only, so far",
            ]
        );
    }
}
