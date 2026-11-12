// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Pool setup: each step's state, read fresh** (spec #4469, #4478). Given a pool, or a kind before
//! there is a pool, this answers each step's state, what purlis found and what to do next. The
//! window and `purlis pool setup` both show this answer, so they can't disagree, and it holds the words.
//!
//! - **The steps of a kind are data** ([`Kind::steps`]): a later kind changes the list and not
//!   the shape. Only built kinds exist here.
//! - **Everything is read on each call** ([`read`]) from Docker, the pool's record, its runner and
//!   the plane, so work done by hand or elsewhere is picked up on the next read. Reading changes
//!   nothing of the person's: a runner that is asleep is not woken to ask it anything, and an
//!   Allow is only looked at. Asking a running runner how a profile stands fetches nothing: it
//!   answers from what is installed (#4507).
//! - **A Docker that isn't on this machine, or a runner that ran a chat without the sandbox, is
//!   asked nothing about its runners**, as for a sign-in. What Docker or a runner says is shown
//!   only as escaped text ([`crate::shown::short`]).
//! - **Only a person's choices are stored** ([`Choices`], `<config>/pool-setup.json`): the kind
//!   they chose and the steps they skipped. What is found always wins over a skip.
//!
//! What the facts mean is one pure function ([`of`]); what reads them is [`read`].
//!
//! **Each step also says what it does by hand** ([`Step::see`], #4481, Q30): the commands behind
//! its See what this does, from the same source as its button, so what a person copies and runs
//! is picked up by the next read. The Pool step's are [`asked::Asked::command`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::dockerpool::{Docker, Missing};
use crate::forge::agentidentity::HostFound;
use crate::profilestore::sign_in::Standing;
use crate::runners::{self, Refused};

pub mod admin;
pub mod asked;
pub mod cluster;
pub mod help;
pub mod connecting;
pub mod install;
pub mod signins;
pub mod trying;

/// The file a person's choices are kept in, in this machine's folder.
const CHOICES_FILE: &str = "pool-setup.json";

/// The programs an agent's forge identity is declared for.
const FORGE_PROGRAMS: [&str; 2] = ["gh", "glab"];

/// Where a pool's chats run. Only what is built is here (Q46).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Docker,
    /// **Docker on another machine, reached over SSH** (#6266, ADR 0123 §1).
    #[serde(rename = "remote-docker")]
    RemoteDocker,
    /// A team's Kubernetes cluster, through purlis's cluster operator (#4492).
    Cluster,
}

impl Kind {
    /// **The steps of this kind, in order** (Q35): a kind is its list, and a later kind (an SSH
    /// host, Docker on another machine) adds its list and the readings of any step of its own.
    #[must_use]
    pub fn steps(self) -> &'static [StepId] {
        match self {
            Kind::Docker => &[
                StepId::Where,
                StepId::Pool,
                StepId::Signins,
                StepId::Identity,
                StepId::Try,
            ],
            Kind::Cluster => &[
                StepId::Where,
                StepId::Check,
                StepId::Install,
                StepId::Pool,
                StepId::Signins,
                StepId::Identity,
                StepId::Try,
            ],
            // Only Connect is new: the others are the Docker kind's, reading the pool's own
            // engine (ADR 0123 §6).
            Kind::RemoteDocker => &[
                StepId::Where,
                StepId::Connect,
                StepId::Pool,
                StepId::Signins,
                StepId::Identity,
                StepId::Try,
            ],
        }
    }
}

/// One step of Pool setup, by the prototype's names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepId {
    Where,
    /// The machine, its key and its engine (#6266).
    Connect,
    /// A cluster's: doctor's findings, before anything changes (#4492).
    Check,
    /// A cluster's: upstream, definitions, signing key and operator, each on its own button (#4493).
    Install,
    Pool,
    Signins,
    Identity,
    Try,
}

impl StepId {
    /// Its name on screen.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            StepId::Where => "Where chats run",
            StepId::Connect => "Connect",
            StepId::Check => "Check the cluster",
            StepId::Install => "Install",
            StepId::Pool => "Pool",
            StepId::Signins => "Sign-ins",
            StepId::Identity => "Agent identity",
            StepId::Try => "Try it",
        }
    }

    /// **Whether a person may skip it**: later steps need Where and Pool, and Try it is the end.
    #[must_use]
    pub fn skippable(self) -> bool {
        matches!(self, StepId::Signins | StepId::Identity)
    }
}

/// A step's state: Q29's five and `later`, a step not reached yet, which can't be jumped to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StepState {
    Done,
    Current,
    Attention,
    Skipped,
    Waiting,
    Later,
}

impl StepState {
    /// The word `purlis pool` prints for it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            StepState::Done => "done",
            StepState::Current => "current",
            StepState::Attention => "attention",
            StepState::Skipped => "skipped",
            StepState::Waiting => "waiting",
            StepState::Later => "later",
        }
    }
}

/// What a step's next action is, for the window to bind a button to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActionId {
    GetDocker,
    StartDocker,
    UseDocker,
    /// Keeps the cluster, on the context chosen, as where chats run (#4492).
    UseCluster,
    /// kubectl isn't on this machine: its own install page.
    GetKubectl,
    /// The install's parts, each on its own button (#4493).
    InstallUpstream,
    InstallDefinitions,
    MakeSigningKey,
    InstallOperator,
    /// The operator's image, by its digest, is named under Options first.
    ChooseOperatorImage,
    /// Makes a Pool in the cluster, of purlis's runner image (#4494).
    CreateClusterPool,
    /// Claims this person's runner of a Pool in the cluster and pairs it here (#4494).
    AddClusterPool,
    /// A cluster pool is marked as the person's own on its row, in Settings › Pools (#4071).
    MarkAsOwn,
    /// Doctor shows the person can't install: the words for their cluster's admin (#4496).
    SendToAdmin,
    /// The person stops waiting for their admin (#4496).
    StopWaiting,
    /// A cluster pool's Pool names a key its pin never vouched for: the person checks the new
    /// one with their admin and pins it again (#4495).
    CheckKeyAgain,
    CreatePool,
    MakeRunnerAgain,
    UseOtherDocker,
    UseLocalDocker,
    RemovePool,
    CheckAgain,
    WakeRunner,
    ChooseProfiles,
    SignIn,
    OpenProject,
    SetUpIdentity,
    AllowTool,
    StartTestChat,
    /// Where: set up a pool on another machine (ADR 0123 §6).
    UseRemoteDocker,
    /// Connect: contact the machine, which only reads its key.
    ConnectMachine,
    /// Connect: pin the key shown, then check the engine there.
    TrustMachine,
    /// Connect: go back to the pick of the machine.
    ChangeMachine,
    /// Connect: plain `tcp://` was refused; the same host over SSH.
    UseSshInstead,
    /// A pin that no longer holds: show what the machine shows now, to trust again.
    CheckAndTrustAgain,
}

/// The one thing to do on a step: its button, what the button does, and, where one exists, the
/// command that does the same.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Action {
    pub id: ActionId,
    pub label: String,
    pub does: String,
    pub command: Option<String>,
    /// The official page it opens, where it opens one.
    pub url: Option<String>,
}

/// **A step by hand** (Q30): the exact commands, and what running them means.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct See {
    pub note: String,
    pub text: String,
}

/// How one check on Try it came out (Q32): never colour alone, so each has its word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Ok,
    Missing,
    Todo,
    Skip,
}

/// **One line of what Try it checks** (#4483): what, how it came out, and why where it didn't.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Check {
    /// What it checks: the window puts what Try it found itself in the identity's place.
    pub about: CheckOf,
    pub verdict: Verdict,
    pub title: String,
    pub more: Option<String>,
}

/// What a line of Try it checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckOf {
    /// One of doctor's lines, on Check the cluster (#4492).
    Cluster,
    Chat,
    Signins,
    Identity,
}

/// One step as it stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Step {
    pub id: StepId,
    pub name: &'static str,
    pub state: StepState,
    /// A short line under its name: what it settled on.
    pub line: Option<String>,
    pub findings: Vec<String>,
    pub next: Option<Action>,
    pub skippable: bool,
    /// See what this does, where the step has commands of its own.
    pub see: Option<See>,
    /// What it checks, for Try it: a skipped step reads "works, except for this" (Q32).
    pub checks: Vec<Check>,
    /// **A second way on**, beside `next` (ADR 0123 §6): on Where, the other kind.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other: Option<Action>,
    /// The machine, on Connect (#6266).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine: Option<connecting::Machine>,
    /// What a finding is, by its key, for the window to draw its card by (`pool-pin-changed`,
    /// `plain-tcp`, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<&'static str>,
    /// The choice purlis didn't recommend, for the person to take instead (story 4 of #4469).
    pub also: Option<Action>,
    /// Install's parts, each with its own button (#4493).
    pub parts: Vec<install::Part>,
    /// What undoes the step by hand, where only that does (uninstalling from a cluster, story 56).
    pub undo: Option<See>,
    /// The words for the person's cluster's admin, where doctor shows they can't install
    /// (#4496): Check the cluster's alone, and held free of secrets.
    pub admin: Option<String>,
}

/// The whole answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Setup {
    pub kind: Kind,
    pub pool: Option<String>,
    /// The first step that isn't finished. `None` once all are.
    pub at: Option<StepId>,
    pub steps: Vec<Step>,
    /// Pools this machine keeps that this setup isn't on, for a person to go on with by pressing.
    pub others: Vec<String>,
    /// The kubeconfig's contexts, the one recommended and the one chosen, for Where's Options.
    pub contexts: Contexts,
}

/// **The contexts a cluster can be reached by** (#4492): names only, never what they sign in with.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Contexts {
    pub names: Vec<String>,
    pub recommended: Option<String>,
    pub chosen: Option<String>,
    /// Whether the person joins their team's install (#4495): the window then asks for the key
    /// their admin gave them before anything is pinned.
    pub joining: bool,
    /// The cluster operator key a kept cluster pool pinned here, for teammates to check theirs
    /// against (#4495).
    pub pinned: Option<String>,
    /// The key its Pool names now, where the pin never vouched for it (#4495).
    pub named_now: Option<String>,
    /// The chosen cluster's Pools, by name, where its operator runs (#4494).
    pub pools: Vec<String>,
}

impl Setup {
    /// A step of this setup.
    #[must_use]
    pub fn step(&self, id: StepId) -> Option<&Step> {
        self.steps.iter().find(|step| step.id == id)
    }
}

/// How this machine's Docker answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Missing,
    NotAnswering,
    /// It answers, and it isn't shown to be on this machine, so its runners are asked nothing.
    Elsewhere,
    Ready,
}

/// How a pool's runner stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Runner {
    /// The pool keeps no runner.
    None,
    /// Docker lists no container of it: it was removed outside purlis.
    Gone,
    /// It was made on another Docker than this one reaches.
    OnAnotherEngine,
    /// Its only runner is finishing its chats, and starts no new one.
    OnlyDraining,
    /// Docker isn't answering, or isn't on this machine, so nothing was asked.
    NotAsked,
    /// Docker answered something else than a state of the runner.
    CannotTell(String),
    Asleep,
    Running,
}

/// How one chosen profile stands on the pool's runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// The runner keeps no profile of this name.
    NotKept,
    Standing(Standing),
}

/// The sign-ins of a pool, as far as they could be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignIns {
    /// The runner wasn't asked: it isn't running.
    NotAsked,
    /// A chat ran there without the sandbox, or may have, so it is neither asked nor believed.
    Tainted,
    /// The pool keeps the project's default profile, and no project is open to say which.
    NeedsProject,
    /// The runner couldn't say.
    CannotTell(String),
    /// Each profile chosen for the pool.
    Read(Vec<(String, Profile)>),
    /// A cluster pool the person hasn't marked as their own: no profile of theirs goes there
    /// (#4071), so nothing is asked of its runner.
    NotYours,
}

/// An agent's forge identity, as this plane's personas declare one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Identity {
    /// There is no project open to read personas from.
    NoProject,
    /// No persona declares a forge tool.
    None,
    /// A persona declares `tool`, and this machine hasn't allowed it.
    NotAllowed { persona: String, tool: String },
    /// The person allowed it, and the persona's list changed since.
    Changed { persona: String, tool: String },
    /// A persona declares `tool`, and a policy or the check of its program keeps it off here.
    KeptOff { persona: String, tool: String },
    /// A persona has `tool`, allowed here.
    Set { persona: String, tool: String },
    /// No persona declares a forge tool, and the project's repos are on `host`, which purlis
    /// can't set up by itself yet (story 36 of #4469): only github.com has one-click setup.
    /// `program` is the forge's CLI there, none where purlis knows no forge at `host`, and
    /// `persona` the project's first, for the line to name.
    Elsewhere {
        host: String,
        program: Option<String>,
        persona: Option<String>,
    },
}

/// A pool's facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolFacts {
    pub name: String,
    pub runner: Runner,
    pub sign_ins: SignIns,
    /// Where the pool is a cluster's, what this machine keeps of it (#4494).
    pub cluster: Option<cluster::KeptHere>,
}

/// Everything read, before it means anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    pub engine: Engine,
    /// Connect's facts, for a pool on another machine (#6266): `None` for this machine's.
    pub connect: Option<connecting::Connecting>,
    pub pool: Option<PoolFacts>,
    pub identity: Identity,
    /// Whether the project's clones have a remote on a forge: with none, gh and git push have
    /// nothing to be checked against (C on #4469).
    pub repos: bool,
    /// The cluster the person can reach, as far as it was read (#4492).
    pub cluster: cluster::ClusterFacts,
}

/// **A person's choices, the only thing stored** (tier: Machine, device-bound).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Choices {
    /// The kind chosen on Where, before there is a pool.
    pub kind: Option<Kind>,
    /// The kubeconfig context chosen on Where for a cluster (#4492): its name, nothing of what it
    /// signs in with.
    pub context: Option<String>,
    /// The cluster operator's image, by its digest, named under Install's Options (#4493).
    pub operator_image: Option<String>,
    /// The name typed on the Pool step, for the box to hold.
    pub name: Option<String>,
    /// That no pool had `name` when it was typed: one that has it now was made since, by hand.
    pub name_free: bool,
    /// The pool this setup made, which it goes on with. A name typed never picks a pool.
    pub pool: Option<String>,
    /// The steps skipped on each pool.
    pub skipped: BTreeMap<String, Vec<StepId>>,
    /// When a real chat was last seen to start on each pool, from Try it (#4483), in seconds
    /// since the epoch: the proof setup ends on, which nothing else could read back.
    pub tried: BTreeMap<String, u64>,
    /// That the person sent the words to their cluster's admin, and waits (#4496).
    pub admin: Option<admin::Asked>,
    /// The signing key this setup made, by its public half (#4495).
    pub made: Option<MadeKey>,
    /// **The machine Connect read, and the pin the person trusted there** (#6266), for a pool on
    /// another machine before it is made.
    pub connect: Option<Chosen>,
}

/// **What Connect keeps** (#6266, `connect` in `pool-setup.json`): the machine, as the person
/// gave it (with the socket found once the engine was checked), and the pin, only once the
/// person pressed Trust this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chosen {
    pub route: String,
    pub trusted: Option<crate::sshroute::Pin>,
}

/// What this machine calls itself in a sentence.
fn this_machine() -> &'static str {
    if cfg!(target_os = "macos") {
        "this Mac"
    } else {
        "this machine"
    }
}

/// Docker's own install page for this system.
fn docker_install_page() -> &'static str {
    if cfg!(target_os = "macos") {
        "https://docs.docker.com/desktop/setup/install/mac-install/"
    } else {
        "https://docs.docker.com/engine/install/"
    }
}

fn action(id: ActionId, label: &str, does: impl Into<String>) -> Option<Action> {
    Some(Action {
        id,
        label: label.to_owned(),
        does: does.into(),
        command: None,
        url: None,
    })
}

/// A step as its facts read, before where it falls in the list is known.
struct Reading {
    finished: bool,
    attention: bool,
    line: Option<String>,
    findings: Vec<String>,
    next: Option<Action>,
}

impl Reading {
    fn done(line: impl Into<String>) -> Reading {
        Reading {
            finished: true,
            attention: false,
            line: Some(line.into()),
            findings: Vec::new(),
            next: None,
        }
    }

    fn to_do(finding: impl Into<String>, next: Option<Action>) -> Reading {
        Reading {
            finished: false,
            attention: false,
            line: None,
            findings: vec![finding.into()],
            next,
        }
    }

    fn attention(finding: impl Into<String>, next: Option<Action>) -> Reading {
        Reading {
            attention: true,
            ..Reading::to_do(finding, next)
        }
    }
}

fn where_reading(kind: Kind, facts: &Facts, choices: &Choices) -> Reading {
    if kind == Kind::RemoteDocker {
        return remote_where_reading(facts, choices);
    }
    match facts.engine {
        Engine::Missing => Reading::attention(
            "You need Docker first. It runs your chats in a container on this machine, apart \
             from your files.",
            Some(Action {
                id: ActionId::GetDocker,
                label: "Open Docker's install page".to_owned(),
                does: "Opens Docker's own page in your browser. purlis doesn't install anything, \
                       and checks again by itself."
                    .to_owned(),
                command: None,
                url: Some(docker_install_page().to_owned()),
            }),
        ),
        Engine::NotAnswering => Reading::attention(
            "Docker needs to be running. purlis found it, and it isn't answering. If it is \
             running, check that your user may use it and that DOCKER_HOST, if you set it, \
             names a socket on this machine.",
            action(
                ActionId::StartDocker,
                "Open the Docker app",
                "Opens Docker where purlis can. Otherwise, start it the way you usually do. \
                 purlis checks again by itself.",
            ),
        ),
        Engine::Elsewhere => Reading::attention(
            "Docker here points at an engine purlis can't tell is on this machine, so purlis \
             makes no runner there and asks it nothing.",
            action(
                ActionId::UseLocalDocker,
                "Use this machine's Docker",
                "Switch your Docker context or DOCKER_HOST to this machine. purlis checks again \
                 by itself.",
            ),
        ),
        Engine::Ready if facts.pool.is_some() || choices.kind == Some(Kind::Docker) => {
            Reading::done(format!("Docker on {}", this_machine()))
        }
        Engine::Ready => Reading::to_do(
            "Docker is ready, so purlis recommends running chats there.",
            action(
                ActionId::UseDocker,
                "Continue with Docker",
                "Saves your choice. Nothing on this machine changes yet.",
            ),
        ),
    }
}

/// **Where, for a pool on another machine** (ADR 0123 §6): Docker's own command is all it needs
/// here, to reach the engine there.
fn remote_where_reading(facts: &Facts, choices: &Choices) -> Reading {
    if facts.engine == Engine::Missing {
        return Reading::attention(
            "purlis reaches Docker on another machine with Docker's own command on this one, and \
             finds none here. Install Docker, or only its command, then purlis checks again by \
             itself.",
            Some(Action {
                id: ActionId::GetDocker,
                label: "Open Docker's install page".to_owned(),
                does: "Opens Docker's own page in your browser. purlis doesn't install anything, \
                       and checks again by itself."
                    .to_owned(),
                command: None,
                url: Some(docker_install_page().to_owned()),
            }),
        );
    }
    if facts.pool.is_some() || choices.kind == Some(Kind::RemoteDocker) {
        return Reading::done("Docker on another machine");
    }
    Reading::to_do(
        "Chats run in a container on another machine with Docker, reached over SSH.",
        action(
            ActionId::UseRemoteDocker,
            "Set up Docker on another machine",
            "Saves your choice. Nothing on this machine or the other one changes yet.",
        ),
    )
}

/// **The second way on of a step**, beside its next (ADR 0123 §6): on Where, before there is a
/// pool, the other kind.
fn other_of(kind: Kind, id: StepId, facts: &Facts) -> Option<Action> {
    if id != StepId::Where || facts.pool.is_some() {
        return None;
    }
    match kind {
        Kind::Docker => action(
            ActionId::UseRemoteDocker,
            "Set up Docker on another machine",
            "Opens the guide for a pool on another machine with Docker, reached over SSH.",
        ),
        Kind::RemoteDocker => action(
            ActionId::UseDocker,
            "Use this machine's Docker",
            "Goes back to running chats in Docker on this machine.",
        ),
        // A cluster's Where goes back to Docker ([`cluster::also`]), and on from there.
        Kind::Cluster => None,
    }
}

/// The machine a pool's runner is made on, in a sentence.
fn made_on(facts: &Facts) -> String {
    facts
        .connect
        .as_ref()
        .and_then(|connect| connect.route.as_deref())
        .and_then(|route| crate::sshroute::Route::parse(route).ok())
        .map_or_else(
            || this_machine().to_owned(),
            |route| route.host().to_owned(),
        )
}

fn pool_reading(facts: &Facts) -> Reading {
    let Some(pool) = &facts.pool else {
        return Reading::to_do(
            "A pool is a named place your chats run. There isn't one yet.",
            action(
                ActionId::CreatePool,
                "Create the pool",
                format!(
                    "Makes one container, its runner, in Docker on {}.",
                    made_on(facts)
                ),
            ),
        );
    };
    let name = &pool.name;
    let make_again = |finding: String| {
        Reading::attention(
            finding,
            action(
                ActionId::MakeRunnerAgain,
                "Make the runner again",
                "Makes a new container for this pool in Docker, with the pool's image and limits.",
            ),
        )
    };
    match &pool.runner {
        // Adding a pool records it first, so this is usually a runner being made.
        Runner::None => Reading::attention(
            format!(
                "Pool {name} has no runner yet. If it is being made, this clears by itself. If \
                 not, remove the pool in Settings › Pools and add it again."
            ),
            action(
                ActionId::CheckAgain,
                "Check again",
                "Asks Docker again. It changes nothing.",
            ),
        ),
        Runner::Gone => make_again(format!(
            "The runner of pool {name} was removed outside purlis."
        )),
        Runner::OnAnotherEngine => Reading::attention(
            format!("The runner of pool {name} was made on another Docker than this one."),
            action(
                ActionId::UseOtherDocker,
                "Switch back",
                "Switch Docker back to the one the runner was made on. purlis checks again by \
                 itself.",
            ),
        ),
        Runner::OnlyDraining => Reading::attention(
            format!("The runner of pool {name} is finishing its chats and starts no new one."),
            action(
                ActionId::CheckAgain,
                "Check again",
                "Asks Docker again. It changes nothing.",
            ),
        ),
        Runner::CannotTell(why) => Reading::attention(
            format!("purlis couldn't read the runner of pool {name}: {why}"),
            action(
                ActionId::CheckAgain,
                "Check again",
                "Asks Docker again. It changes nothing.",
            ),
        ),
        Runner::NotAsked | Runner::Asleep | Runner::Running => Reading::done(name.clone()),
    }
}

fn sign_ins_reading(facts: &Facts) -> Reading {
    let Some(pool) = &facts.pool else {
        return Reading::to_do("", None);
    };
    let name = &pool.name;
    let profiles = match &pool.sign_ins {
        // Only a runner asleep is woken: one that couldn't be read is asked again.
        SignIns::NotAsked if pool.runner != Runner::Asleep => {
            return Reading::attention(
                format!("purlis couldn't ask the runner of pool {name} how its sign-ins stand."),
                action(
                    ActionId::CheckAgain,
                    "Check again",
                    "Asks the runner again. It changes nothing.",
                ),
            );
        }
        SignIns::NotAsked => {
            return Reading::to_do(
                format!(
                    "The runner of pool {name} isn't running, so purlis can't read its sign-ins."
                ),
                action(
                    ActionId::WakeRunner,
                    "Wake the runner",
                    "Starts the runner's container, and it goes back to sleep by itself.",
                ),
            );
        }
        SignIns::Tainted => {
            return Reading::attention(
                crate::runnertaint::signs_nothing_in_on(name),
                action(
                    ActionId::RemovePool,
                    "Remove the pool",
                    "Removes the pool and its runner. Adding it again makes a new runner.",
                ),
            );
        }
        SignIns::NeedsProject => {
            return Reading::to_do(
                "This pool keeps your project's default profile, and no project is open here.",
                open_project(),
            );
        }
        SignIns::CannotTell(why) => {
            return Reading::attention(
                format!("The runner of pool {name} couldn't say how its sign-ins stand: {why}."),
                action(
                    ActionId::CheckAgain,
                    "Check again",
                    "Asks the runner again. It changes nothing.",
                ),
            );
        }
        SignIns::Read(profiles) => profiles,
        SignIns::NotYours => {
            return Reading::to_do(
                format!(
                    "Pool {name} isn't marked as your own, so purlis keeps no profile of yours on \
                     its runner. Your cluster's admins, and whoever holds its nodes, can read \
                     what a runner holds. Mark it on its row once you accept that."
                ),
                action(
                    ActionId::MarkAsOwn,
                    "Open Settings › Pools",
                    "Opens your pools, where This is my own is on the pool's row. Only you can \
                     turn it on.",
                ),
            );
        }
    };
    if profiles.is_empty() {
        return Reading::to_do(
            "A profile holds your Claude, Codex or other account. This pool has none yet.",
            choose_profiles(),
        );
    }
    if let Some((profile, _)) = profiles.iter().find(|(_, p)| *p == Profile::NotKept) {
        return Reading::to_do(
            format!("The runner doesn't keep the profile {profile} yet."),
            choose_profiles(),
        );
    }
    if let Some((profile, _)) = profiles
        .iter()
        .find(|(_, p)| *p == Profile::Standing(Standing::SignedOut))
    {
        return Reading::to_do(
            format!("The profile {profile} isn't signed in on this pool."),
            Some(Action {
                id: ActionId::SignIn,
                label: format!("Sign in {profile}"),
                does: "Opens the sign-in page in your browser. The sign-in stays on the runner."
                    .to_owned(),
                command: Some(format!("purlis profile sign-in {profile} --pool {name}")),
                url: None,
            }),
        );
    }
    let mut reading = Reading::done(
        profiles
            .iter()
            .map(|(profile, _)| profile.as_str())
            .collect::<Vec<_>>()
            .join(", "),
    );
    reading.findings = profiles
        .iter()
        .filter_map(|(profile, standing)| match standing {
            Profile::Standing(Standing::Fetching) => Some(format!(
                "The runner is fetching the harness version {profile} pins, so it can't say yet \
                 whether it is signed in. A chat on it starts."
            )),
            Profile::Standing(Standing::SignedIn) => None,
            _ => Some(format!(
                "Its harness doesn't say whether {profile} is signed in. A chat on it starts."
            )),
        })
        .collect();
    reading
}

fn choose_profiles() -> Option<Action> {
    action(
        ActionId::ChooseProfiles,
        "Choose profiles",
        "Picks the accounts a chat on this pool can use. Settings › Pools has the list.",
    )
}

fn open_project() -> Option<Action> {
    action(
        ActionId::OpenProject,
        "Open a project",
        "Run this in your project's folder, or open it in the window.",
    )
}

fn identity_reading(facts: &Facts) -> Reading {
    let set_up = || {
        action(
            ActionId::SetUpIdentity,
            "Set up an identity",
            "Gives the agent its own account on your forge. Your own sign-in never reaches a \
             chat.",
        )
    };
    let allow = || {
        action(
            ActionId::AllowTool,
            "Allow it in Settings › Project",
            "Opens Settings › Project: allow the persona's tool there, under Sandbox, on \
             this machine. It never holds your own sign-in. This step is done once it's allowed.",
        )
    };
    match &facts.identity {
        Identity::NoProject => Reading::to_do(
            "purlis reads the agent's identity from a project, and none is open here.",
            open_project(),
        ),
        Identity::None => Reading::to_do(
            "The agent has no account of its own, so gh and git push won't work in a chat on \
             the pool.",
            set_up(),
        ),
        Identity::NotAllowed { persona, tool } => Reading::to_do(
            format!("The persona {persona} declares {tool}, and this machine hasn't allowed it."),
            allow(),
        ),
        Identity::Changed { persona, tool } => Reading::to_do(
            format!("The persona {persona} has {tool}, and its list changed since you allowed it."),
            allow(),
        ),
        Identity::KeptOff { persona, tool } => Reading::to_do(
            format!("The persona {persona} declares {tool}, and this machine keeps it off."),
            set_up(),
        ),
        Identity::Set { persona, tool } => Reading::done(format!("{tool} through {persona}")),
        Identity::Elsewhere {
            host,
            program: Some(_),
            ..
        } => Reading::to_do(
            format!(
                "purlis can't set up {host} by itself yet: one-click setup works for github.com. \
                 You can still do this by hand, and it works just as well. Make a token on {host} \
                 for the agent, never your own, keep it in a vault of its own and add the tool \
                 line below. Once this machine allows it, this step is done."
            ),
            check_again(),
        ),
        Identity::Elsewhere {
            host,
            program: None,
            ..
        } => Reading::to_do(
            format!(
                "Your repos are on {host}, and purlis knows no forge there, so it can't give the \
                 agent an identity on it. If {host} is a GitHub Enterprise Server or a GitLab, add \
                 it in Settings › Forges, then check again. Or skip this for now."
            ),
            check_again(),
        ),
    }
}

fn check_again() -> Option<Action> {
    action(
        ActionId::CheckAgain,
        "Check again",
        "Reads the project's settings again. It changes nothing.",
    )
}

/// The vault a token kept by hand for the agent goes in: one of its own, never a vault the
/// person's own forge credential may be in.
const BY_HAND_VAULT: &str = "agent-forge";

/// **The tool line a person adds by hand for a forge purlis can't set up by itself** (story 36 of
/// #4469), after the commands that make its vault, in the system keyring, and keep its token
/// there: what the agent identity step's See what this does shows. The vault is made new, so a
/// vault of that name already there refuses the command rather than mix the agent's token with
/// what it holds. gitlab.com needs no `host`; `host` is a host's name ([`crate::forge::host_ok`]),
/// and the persona's name is written as a TOML key.
fn by_hand(host: &str, program: &str, persona: Option<&str>) -> String {
    let var = if program == "glab" {
        "GITLAB_TOKEN"
    } else {
        "GH_ENTERPRISE_TOKEN"
    };
    let host = if host == "gitlab.com" {
        String::new()
    } else {
        format!(", host = \"{host}\"")
    };
    let persona = persona.map_or_else(
        || "<persona>".to_owned(),
        |name| toml_edit::Key::new(name).display_repr().into_owned(),
    );
    format!(
        "purlis vault add {BY_HAND_VAULT} --provider keyring\n\
         purlis secret set {BY_HAND_VAULT} token --stdin\n\n\
         [sandbox.personas.{persona}.tools]\n\
         {program} = {{ vault = \"{BY_HAND_VAULT}\", as = \"{var} env\", key = \"token\"{host} }}"
    )
}

fn try_reading(facts: &Facts, choices: &Choices) -> Reading {
    let pool = facts.pool.as_ref().map_or("the pool", |pool| &pool.name);
    if facts
        .pool
        .as_ref()
        .is_some_and(|pool| choices.tried.contains_key(&pool.name))
    {
        return Reading::done("A real chat started");
    }
    Reading::to_do(
        "Try it starts a real chat on the pool, so you know it works before you rely on it.",
        action(
            ActionId::StartTestChat,
            "Start a test chat",
            format!(
                "Starts a real chat on {pool}, and closes it once it has started. Nothing is \
                 typed into it."
            ),
        ),
    )
}

/// **What the facts and choices mean**: each step's state, in the order of the kind's steps.
///
/// The first step that isn't done or skipped is where the person is. A step after it that isn't
/// done is `later`, and says nothing yet. A step found done is done wherever it falls.
#[must_use]
pub fn of(kind: Kind, facts: &Facts, choices: &Choices) -> Setup {
    let pool = facts.pool.as_ref().map(|pool| pool.name.clone());
    let skipped = pool
        .as_ref()
        .and_then(|name| choices.skipped.get(name))
        .map_or(&[][..], Vec::as_slice);
    let mut at = None;
    let mut steps = Vec::with_capacity(kind.steps().len());
    for &id in kind.steps() {
        let waits = id == StepId::Check
            && kind == Kind::Cluster
            && admin::waiting_on(choices, facts.cluster.chosen.as_deref());
        let mut machine = None;
        let mut cause = None;
        let reading = match (kind, id) {
            (Kind::Cluster, StepId::Where) => cluster::where_reading(facts, choices),
            (_, StepId::Where) => where_reading(kind, facts, choices),
            (_, StepId::Connect) => {
                let (reading, shown, why) =
                    connecting::reading(&facts.connect.clone().unwrap_or_default());
                machine = Some(shown);
                cause = why;
                reading
            }
            (_, StepId::Check) => cluster::check_reading(facts),
            (_, StepId::Install) => cluster::install_reading(facts, choices),
            (Kind::Cluster, StepId::Pool) => cluster::pool_reading(facts),
            (_, StepId::Pool) => pool_reading(facts),
            (_, StepId::Signins) => sign_ins_reading(facts),
            (_, StepId::Identity) => identity_reading(facts),
            (_, StepId::Try) => try_reading(facts, choices),
        };
        let state = if reading.finished {
            StepState::Done
        } else if id.skippable() && skipped.contains(&id) && !reading.attention {
            StepState::Skipped
        } else if at.is_some() {
            StepState::Later
        } else {
            at = Some(id);
            if reading.attention && waits {
                StepState::Waiting
            } else if reading.attention {
                StepState::Attention
            } else {
                StepState::Current
            }
        };
        let says = !matches!(state, StepState::Later | StepState::Skipped);
        let words = (id == StepId::Check && says)
            .then(|| admin::text(facts, choices))
            .flatten();
        steps.push(Step {
            id,
            name: id.name(),
            state,
            line: if state == StepState::Waiting {
                Some("Waiting for your admin".to_owned())
            } else {
                reading.line
            },
            findings: if says { reading.findings } else { Vec::new() },
            next: if says { reading.next } else { None },
            skippable: id.skippable(),
            see: see_of(kind, id, facts, choices),
            checks: Vec::new(),
            other: if says {
                other_of(kind, id, facts)
            } else {
                None
            },
            machine,
            cause: if says { cause } else { None },
            parts: if id == StepId::Install && says {
                cluster::parts_of(facts, choices)
            } else {
                Vec::new()
            },
            undo: if id == StepId::Install {
                cluster::undo_of(facts)
            } else {
                None
            },
            also: if id == StepId::Where && state != StepState::Done && facts.pool.is_none() {
                cluster::also(kind, facts)
            } else if words.is_some() && state == StepState::Waiting {
                action(
                    ActionId::StopWaiting,
                    "Stop waiting",
                    "Forgets that you asked your admin. Nothing on the cluster changes.",
                )
            } else if words.is_some() {
                action(
                    ActionId::SendToAdmin,
                    "Send to your admin",
                    "Shows the words to send your cluster's admin: what's missing, what purlis \
                     needs and the commands. They hold no password, token, key or credential.",
                )
            } else {
                None
            },
            admin: words,
        });
    }
    if let (Some(name), Some(at)) = (
        pool.as_deref(),
        steps.iter().position(|step| step.id == StepId::Try),
    ) {
        let checks = try_checks(&steps, name, choices.tried.contains_key(name), facts.repos);
        steps[at].checks = checks;
    }
    if let Some(check) = steps.iter_mut().find(|step| step.id == StepId::Check)
        && check.state != StepState::Later
    {
        check.checks = cluster::checks_of(facts);
    }
    let kept = facts.pool.as_ref().and_then(|pool| pool.cluster.as_ref());
    Setup {
        kind,
        pool,
        at,
        steps,
        others: Vec::new(),
        contexts: Contexts {
            names: facts.cluster.contexts.clone(),
            recommended: facts.cluster.recommended().map(str::to_owned),
            chosen: facts.cluster.chosen.clone(),
            joining: facts.cluster.joining,
            pinned: kept.map(|kept| kept.pinned.clone()),
            named_now: kept.and_then(|kept| match &kept.now {
                crate::pools::PinNow::Changed { named } => Some(named.clone()),
                _ => None,
            }),
            pools: facts.cluster.pools.clone().unwrap_or_default(),
        },
    }
}

/// **What Try it checks on pool `pool`, by what the steps before it say** (#4483, Q32, C on
/// #4469): a step that was skipped is a check that wasn't made, said as such, so the end reads
/// "works, except for this" and never as done.
fn try_checks(steps: &[Step], pool: &str, tried: bool, repos: bool) -> Vec<Check> {
    let state = |id| steps.iter().find(|step| step.id == id);
    let check = |about, verdict, title: String, more: Option<&str>| Check {
        about,
        verdict,
        title,
        more: more.map(str::to_owned),
    };
    let mut checks = vec![if tried {
        check(
            CheckOf::Chat,
            Verdict::Ok,
            format!("A chat started on {pool}"),
            None,
        )
    } else {
        check(
            CheckOf::Chat,
            Verdict::Todo,
            format!("A chat starts on {pool}"),
            None,
        )
    }];
    checks.push(match state(StepId::Signins) {
        Some(step) if step.state == StepState::Done => check(
            CheckOf::Signins,
            Verdict::Ok,
            format!(
                "{} signed in",
                step.line.as_deref().unwrap_or("Your profiles are")
            ),
            None,
        ),
        Some(step) if step.state == StepState::Skipped => check(
            CheckOf::Signins,
            Verdict::Skip,
            "Sign-ins were skipped".to_owned(),
            Some("Chats on the pool start on the profiles its runner has of its own."),
        ),
        _ => check(
            CheckOf::Signins,
            Verdict::Todo,
            "Your profiles are signed in".to_owned(),
            None,
        ),
    });
    // How the agent identity step stands, and a step not done is never said as passed. What Try
    // it proves of the identity itself, the window says in this line's place as it ends.
    checks.push(match state(StepId::Identity) {
        Some(step) if step.state == StepState::Skipped => check(
            CheckOf::Identity,
            Verdict::Skip,
            "gh and git push weren't checked: agent identity was skipped".to_owned(),
            Some("In these chats, gh and git push say they have no identity."),
        ),
        // C on #4469: with no repo there is nothing to check them against, set up or not.
        _ if !repos => check(
            CheckOf::Identity,
            Verdict::Skip,
            "gh and git push weren't checked: this project has no repo yet".to_owned(),
            Some(
                "Once a clone in a workspace has a remote on a forge, try it again to check them.",
            ),
        ),
        Some(step) if step.state == StepState::Done => check(
            CheckOf::Identity,
            Verdict::Ok,
            format!(
                "The agent's identity is set: {}",
                step.line.as_deref().unwrap_or("allowed here")
            ),
            None,
        ),
        _ => check(
            CheckOf::Identity,
            Verdict::Todo,
            "The agent's identity is set".to_owned(),
            Some("The agent identity step sets it, for gh and git push in these chats."),
        ),
    });
    checks
}

/// **What `id` does by hand**, where it has commands of its own. The Pool step's, before there is
/// a pool, depend on what the person chose, so they are [`asked::Asked::command`]'s.
fn see_of(kind: Kind, id: StepId, facts: &Facts, choices: &Choices) -> Option<See> {
    if kind == Kind::Cluster
        && let Some(see) = cluster::see_of(id, facts, choices)
    {
        return Some(see);
    }
    let pool = facts.pool.as_ref();
    if let (
        StepId::Identity,
        Identity::Elsewhere {
            host,
            program: Some(program),
            persona,
        },
    ) = (id, &facts.identity)
    {
        return Some(See {
            note: "These make a vault of the agent's own and keep its token there, then the \
                   line goes in your project's settings. Never use your own token: a chat would \
                   act as you."
                .to_owned(),
            text: by_hand(host, program, persona.as_deref()),
        });
    }
    match (id, pool) {
        (StepId::Connect, _) => Some(connecting::by_hand(
            facts
                .connect
                .as_ref()
                .and_then(|connect| connect.route.as_deref()),
        )),
        (StepId::Where, _) if kind != Kind::Cluster => Some(See {
            note: "This is how purlis asks after Docker. It only reads.".to_owned(),
            text: "docker version".to_owned(),
        }),
        (StepId::Pool, Some(pool)) => Some(See {
            note: "This lists the pool's limits. It only reads.".to_owned(),
            text: format!("purlis pool limits {}", shell_quote(&pool.name)),
        }),
        // What Sign-ins' button runs, as the window spells it for what is ticked (#4718): with
        // nothing chosen yet, a stand-in for the profile to record.
        (StepId::Signins, Some(pool)) => {
            let SignIns::Read(profiles) = &pool.sign_ins else {
                return None;
            };
            let adding = if profiles.is_empty() {
                vec!["PROFILE".to_owned()]
            } else {
                Vec::new()
            };
            let signing_in: Vec<String> = profiles
                .iter()
                .filter(|(_, profile)| *profile != Profile::Standing(Standing::SignedIn))
                .map(|(profile, _)| profile.clone())
                .collect();
            signins::by_hand(&pool.name, &adding, &[], &signing_in, false)
        }
        _ => None,
    }
}

fn shell_quote(word: &str) -> String {
    crate::shellseg::shell_quote(word)
}

/// Where a read looks.
pub struct Here<'a> {
    pub config: &'a Path,
    /// The plane personas are read from, where one is open.
    pub plane: Option<&'a Path>,
    /// This machine's `docker`, where it has one.
    pub docker: Option<&'a Docker>,
    /// The socket of this machine's engine, or why it isn't on this machine.
    pub engine: Result<PathBuf, String>,
    /// This machine's kubectl, through which a cluster is reached with the person's own
    /// sign-in, or why there is none ([`cluster::kubectl_here`]).
    pub kubectl: Result<&'a crate::clusterpool::Kubectl, &'a str>,
}

/// **Reads pool `pool`'s setup, or a new Docker pool's where `pool` is `None`.** Reads everything
/// again, and changes nothing. Refused where there is no such Docker pool on this machine.
pub fn read(here: &Here<'_>, pool: Option<&str>) -> Result<Setup, Refused> {
    read_pressed(here, pool, None)
}

/// **What a press of Connect found**, laid over what is read (#6266): the key met by Connect to,
/// or what went wrong. Nothing of it is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pressed {
    Met(connecting::Met),
    Found(crate::sshroute::connect::Finding),
}

/// **[`read`], with what a press of Connect just found** laid over Connect's facts.
pub fn read_pressed(
    here: &Here<'_>,
    pool: Option<&str>,
    pressed: Option<Pressed>,
) -> Result<Setup, Refused> {
    let choices = read_choices(here.config)?;
    let kept = match pool {
        Some(name) => runners::pools(here.config)?
            .into_iter()
            .find(|one| one.name() == name),
        None => None,
    };
    // On another machine where the pool is, or a new pool's choice on Where says so; the other
    // kinds are told apart below, once the pool and the contexts are read ([`kind_of`]).
    let remote = match (&kept, pool) {
        (Some(kept), _) => kept.remote().is_some(),
        (None, None) => choices.kind == Some(Kind::RemoteDocker),
        (None, Some(_)) => false,
    };
    let kind = if remote {
        Kind::RemoteDocker
    } else {
        Kind::Docker
    };
    let (engine, docker, mut connect) = match (
        kind,
        kept.as_ref()
            .and_then(|kept| kept.remote().map(|remote| (kept, remote))),
    ) {
        // A pool on another machine: its own engine, through its forward, pin checked.
        (_, Some((kept, remote))) => {
            let connect = connecting::Connecting {
                route: Some(remote.route.spelled()),
                trusted: Some(remote.pin.clone()),
                ..connecting::Connecting::default()
            };
            match here.docker {
                None => (Engine::Missing, None, connect),
                Some(docker) => match crate::sshroute::pool::docker_of(docker, kept) {
                    Ok(through) => (Engine::Ready, Some(through), connect),
                    Err(finding) => (
                        Engine::NotAnswering,
                        None,
                        connecting::Connecting {
                            finding: Some(finding),
                            ..connect
                        },
                    ),
                },
            }
        }
        (Kind::RemoteDocker, None) => {
            let connect = connecting::Connecting {
                route: choices.connect.as_ref().map(|chosen| chosen.route.clone()),
                trusted: choices
                    .connect
                    .as_ref()
                    .and_then(|chosen| chosen.trusted.clone()),
                ..connecting::Connecting::default()
            };
            let engine = if here.docker.is_some() {
                Engine::Ready
            } else {
                Engine::Missing
            };
            (engine, None, connect)
        }
        (_, None) => (
            engine_of(here),
            here.docker.cloned(),
            connecting::Connecting::default(),
        ),
    };
    match pressed {
        Some(Pressed::Met(met)) => connect.met = Some(met),
        Some(Pressed::Found(finding)) => connect.finding = Some(finding),
        None => {}
    }
    let pool = pool
        .map(|name| pool_facts(here, name, engine, docker.as_ref()))
        .transpose()?;
    // Each clone's remotes, read once: a git process a clone.
    let hosts = here
        .plane
        .map(crate::forge::agentidentity::hosts_of_the_project)
        .unwrap_or_default();
    // Contexts are read from the kubeconfig alone; the cluster itself only on a cluster's setup.
    let contexts = cluster::read(
        here.kubectl.map_err(str::to_owned),
        choices.context.as_deref(),
        false,
    );
    let on_a_cluster = pool.as_ref().map(|pool| pool.cluster.is_some());
    let kind = if remote {
        kind
    } else {
        kind_of(&choices, engine, on_a_cluster, &contexts)
    };
    let cluster = if kind == Kind::Cluster {
        cluster::read(
            here.kubectl.map_err(str::to_owned),
            pool.as_ref()
                .and_then(|pool| pool.cluster.as_ref())
                .map(|kept| kept.context.as_str())
                .or(choices.context.as_deref()),
            true,
        )
    } else {
        contexts
    };
    let facts = Facts {
        engine,
        connect: remote.then_some(connect),
        pool,
        identity: identity_in(here.plane, &hosts),
        repos: !hosts.is_empty(),
        cluster,
    };
    Ok(of(kind, &facts, &choices))
}

/// **What a finding of Connect says**, in Pool setup's words: the same wherever it is shown.
#[must_use]
pub fn connect_said(finding: &crate::sshroute::connect::Finding) -> String {
    connecting::said(finding)
}

/// **The kind a setup is on** (#4492): a pool's own, else the one chosen on Where, else the one
/// purlis recommends: Docker where it is ready or no cluster is named, else the cluster.
fn kind_of(
    choices: &Choices,
    engine: Engine,
    on_a_cluster: Option<bool>,
    cluster: &cluster::ClusterFacts,
) -> Kind {
    match on_a_cluster {
        Some(true) => return Kind::Cluster,
        Some(false) => return Kind::Docker,
        None => {}
    }
    choices
        .kind
        .unwrap_or(if engine == Engine::Ready || cluster.contexts.is_empty() {
            Kind::Docker
        } else {
            Kind::Cluster
        })
}

fn engine_of(here: &Here<'_>) -> Engine {
    match here.docker {
        None => Engine::Missing,
        Some(docker) if docker.answers().is_err() => Engine::NotAnswering,
        Some(_) if here.engine.is_err() => Engine::Elsewhere,
        Some(_) => Engine::Ready,
    }
}

/// **How pool `name`'s runner and its sign-ins stand, read now** (#4483): what Try it picks its
/// profile by ([`trying::profile_to_try`]). It reads as [`read`] does, and changes nothing.
pub fn read_pool(here: &Here<'_>, name: &str) -> Result<PoolFacts, Refused> {
    let kept = runners::pools(here.config)?
        .into_iter()
        .find(|one| one.name() == name);
    if let (Some(kept), Some(docker)) = (kept.filter(|kept| kept.remote().is_some()), here.docker) {
        return match crate::sshroute::pool::docker_of(docker, &kept) {
            Ok(through) => pool_facts(here, name, Engine::Ready, Some(&through)),
            Err(_) => pool_facts(here, name, Engine::NotAnswering, None),
        };
    }
    pool_facts(here, name, engine_of(here), here.docker)
}

fn pool_facts(
    here: &Here<'_>,
    name: &str,
    engine: Engine,
    docker: Option<&Docker>,
) -> Result<PoolFacts, Refused> {
    if !runners::pools(here.config)?
        .iter()
        .any(|pool| pool.name() == name)
    {
        return match cluster::pool_kept(here, name)? {
            Some(facts) => Ok(facts),
            None => Err(Refused::Other(format!(
                "there is no pool named {name} on this machine"
            ))),
        };
    }
    let docker = docker.filter(|_| engine == Engine::Ready);
    let runner = runner_of(here.config, docker, name);
    let sign_ins = match (&runner, docker) {
        // Whether it is awake or not: waking it would help nothing.
        (Runner::Running | Runner::Asleep, Some(_))
            if crate::runnertaint::pool_ran_unsandboxed(here.config, name) =>
        {
            SignIns::Tainted
        }
        (Runner::Running, Some(docker)) => sign_ins_of(here, docker, name),
        _ => SignIns::NotAsked,
    };
    Ok(PoolFacts {
        name: name.to_owned(),
        runner,
        sign_ins,
        cluster: None,
    })
}

fn runner_of(config: &Path, docker: Option<&Docker>, name: &str) -> Runner {
    let kept = match runners::runner_for(config, name) {
        Ok(kept) => kept,
        Err(_) => {
            return match runners::read(config) {
                Ok(all) if all.iter().any(|kept| kept.pool() == name) => Runner::OnlyDraining,
                Ok(_) => Runner::None,
                Err(why) => Runner::CannotTell(crate::shown::short(&why.to_string())),
            };
        }
    };
    let Some(docker) = docker else {
        return Runner::NotAsked;
    };
    match docker.missing(&kept.made, kept.engine.as_deref()) {
        Ok(Some(Missing::Removed(_))) => return Runner::Gone,
        Ok(Some(Missing::OnAnotherEngine)) => return Runner::OnAnotherEngine,
        Ok(None) => {}
        Err(why) => return Runner::CannotTell(crate::shown::short(&why.0)),
    }
    match docker.running(&kept.made) {
        Ok(true) => Runner::Running,
        Ok(false) => Runner::Asleep,
        Err(why) => Runner::CannotTell(crate::shown::short(&why.0)),
    }
}

fn sign_ins_of(here: &Here<'_>, docker: &Docker, name: &str) -> SignIns {
    // Only a Docker shown to be on this machine, or a pool's own on another machine through its
    // pinned forward (#6266), reaches here (`Engine::Ready`).
    let engine = match docker.forwarded() {
        Some(socket) => Ok(socket.to_path_buf()),
        None => here.engine.clone(),
    };
    let (Ok(engine), Ok(kept)) = (&engine, runners::runner_for(here.config, name)) else {
        return SignIns::NotAsked;
    };
    let default = here
        .plane
        .and_then(|root| crate::profiles::current(root).default);
    let chose = match runners::profiles_of(here.config, name) {
        Ok(chose) => chose,
        Err(why) => return SignIns::CannotTell(crate::shown::short(&why.to_string())),
    };
    if chose.is_none() && here.plane.is_none() {
        return SignIns::NeedsProject;
    }
    let profiles = match runners::profiles_kept_of(here.config, name, default.as_deref()) {
        Ok(profiles) => profiles,
        Err(why) => return SignIns::CannotTell(crate::shown::short(&why.to_string())),
    };
    let mut read = Vec::with_capacity(profiles.len());
    for profile in profiles {
        match docker.standing_as_installed(&kept.made, engine, &profile) {
            Ok(standing) => {
                let standing = standing.map_or(Profile::NotKept, Profile::Standing);
                read.push((profile, standing));
            }
            Err(why) => return SignIns::CannotTell(crate::shown::short(&why)),
        }
    }
    SignIns::Read(read)
}

/// **The persona and forge tool the agent identity step reads as set**, where it does, in the
/// project at `root`: what Try it may prove, and only that (#4490). Reads no clone.
#[must_use]
pub fn identity_set_in(root: &Path) -> Option<(String, String)> {
    match identity_in(Some(root), &[]) {
        Identity::Set { persona, tool } => Some((persona, tool)),
        _ => None,
    }
}

fn identity_in(plane: Option<&Path>, hosts: &[HostFound]) -> Identity {
    use crate::sandbox::persona::{Standing as Allow, looked_at};
    let Some(root) = plane else {
        return Identity::NoProject;
    };
    // A project with no sandbox table has no persona's tools to read.
    let personas = looked_at(root).unwrap_or_default();
    let forge = |name: &str| FORGE_PROGRAMS.contains(&name);
    let mut short_of_it = None;
    for (shown, declared) in personas {
        let persona = shown.persona.clone();
        if let Some(tool) = shown.tools.iter().find(|tool| forge(&tool.name)) {
            let tool = tool.name.clone();
            match shown.standing {
                Allow::Allowed => return Identity::Set { persona, tool },
                Allow::NotAllowed => {
                    short_of_it.get_or_insert(Identity::NotAllowed { persona, tool });
                }
                Allow::Waiting => {
                    short_of_it.get_or_insert(Identity::Changed { persona, tool });
                }
            }
        } else if let Some(tool) = declared.iter().find(|tool| forge(&tool.name)) {
            short_of_it.get_or_insert(Identity::KeptOff {
                persona,
                tool: tool.name.clone(),
            });
        }
    }
    short_of_it.unwrap_or_else(|| elsewhere(root, hosts))
}

/// **No persona has a forge tool**: one-click setup where the project's repos are on github.com
/// or on no host at all, and plain words where they are only elsewhere (story 36 of #4469): the
/// line by hand on a forge purlis knows, first, else what to declare.
fn elsewhere(root: &Path, hosts: &[HostFound]) -> Identity {
    use crate::forge::{Kind as Forge, agentidentity};
    if hosts.iter().any(|found| found.host == agentidentity::HOST) {
        return Identity::None;
    }
    let Some(found) = hosts
        .iter()
        .find(|found| found.kind.is_some())
        .or_else(|| hosts.first())
    else {
        return Identity::None;
    };
    Identity::Elsewhere {
        host: found.host.clone(),
        program: found.kind.map(|kind| {
            match kind {
                Forge::GitHub => "gh",
                Forge::GitLab => "glab",
            }
            .to_owned()
        }),
        persona: crate::workspaces::Plane::open(root.to_path_buf())
            .personas()
            .ok()
            .and_then(|names| names.into_iter().next()),
    }
}

/// **How a pool stands in the chat picker** (#4484, D on #4469, refining Q47): offered, offered
/// marked "Setup not finished" with why, or not offered, with why, because a chat can't start
/// there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InThePicker {
    Offered,
    /// A chat starts there, and setup isn't finished: why, in the picker's words.
    NotFinished(String),
    /// A chat can't start there: why, in the picker's words.
    NotOffered(String),
}

/// What the picker says of a pool whose chats have no forge identity.
pub const NO_IDENTITY_IN_THE_PICKER: &str =
    "Setup not finished: chats start there, but gh and git push have no account to act as.";

/// What the picker says of a pool where a chat can't start.
pub const CANNOT_START_IN_THE_PICKER: &str =
    "Setup not finished: a chat can't start there yet, as it has no runner ready.";

/// **How each of `pools` stands in the chat picker of the project at `plane`**: read from what
/// this machine keeps, its Pool setup choices and the plane's personas (read once), and nothing
/// asked of Docker or a runner, since the picker opens often. A Docker pool with no runner that
/// starts chats isn't offered. One whose agent identity step was skipped there is offered as the
/// person left it. A pool that isn't a Docker pool kept here is offered as it is: Pool setup
/// reads only Docker pools so far.
pub fn in_the_picker(
    config: &Path,
    plane: Option<&Path>,
    pools: &[String],
) -> Vec<(String, InThePicker)> {
    let kept = runners::pools(config).unwrap_or_default();
    let skipped = read_choices(config)
        .map(|choices| choices.skipped)
        .unwrap_or_default();
    let mut identity = None;
    pools
        .iter()
        .map(|pool| {
            let standing = if !kept.iter().any(|one| one.name() == pool) {
                InThePicker::Offered
            } else if runners::runner_for(config, pool).is_err() {
                InThePicker::NotOffered(CANNOT_START_IN_THE_PICKER.to_owned())
            } else if skipped
                .get(pool)
                .is_some_and(|steps| steps.contains(&StepId::Identity))
            {
                InThePicker::Offered
            } else {
                // No clone's remotes are read: only whether the identity reads as set counts here.
                in_the_picker_by(identity.get_or_insert_with(|| identity_in(plane, &[])))
            };
            (pool.clone(), standing)
        })
        .collect()
}

/// [`in_the_picker`] for a pool whose runner starts chats, by the plane's forge identity.
fn in_the_picker_by(identity: &Identity) -> InThePicker {
    match identity {
        // No project open says nothing of its personas.
        Identity::Set { .. } | Identity::NoProject => InThePicker::Offered,
        _ => InThePicker::NotFinished(NO_IDENTITY_IN_THE_PICKER.to_owned()),
    }
}

/// **The choices kept on this machine.** None where nothing was chosen; refused where the file
/// can't be read or isn't what purlis wrote, so a choice is never silently lost.
pub fn read_choices(config: &Path) -> Result<Choices, Refused> {
    let path = crate::machine::dir(config).join(CHOICES_FILE);
    let text = match purlis_same_user::read_private_file(&path) {
        Ok(text) => text,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(Choices::default()),
        Err(why) => {
            return Err(Refused::Other(format!(
                "your Pool setup choices could not be read safely ({}): {why}",
                path.display()
            )));
        }
    };
    let unreadable = || {
        Refused::Other(format!(
            "your Pool setup choices aren't in the form purlis wrote ({}). Remove the file to \
             choose again.",
            path.display()
        ))
    };
    let held: Held = serde_json::from_str(&text).map_err(|_| unreadable())?;
    if held.version != 1 {
        return Err(unreadable());
    }
    Ok(Choices {
        kind: held.kind,
        context: held.context,
        operator_image: held.operator_image,
        name: held.name,
        name_free: held.name_free,
        pool: held.pool,
        skipped: held.skipped,
        tried: held.tried,
        admin: held.admin,
        made: held.made,
        connect: match held.connect {
            None => None,
            Some(connect) => Some(chosen_from(connect).ok_or_else(unreadable)?),
        },
    })
}

/// The `connect` kept, read: a route as it was given, and a whole pin where there is one.
fn chosen_from(held: HeldConnect) -> Option<Chosen> {
    let trusted = match held.trusted {
        None => None,
        Some(pin) => {
            let route = crate::sshroute::Route::parse(&held.route).ok()?;
            route.socket()?;
            Some(crate::sshroute::Pin {
                host_key: crate::sshroute::HostKey::parse(&pin.host_key).ok()?,
                engine: Some(pin.engine).filter(|id| runners::usable_noted(id))?,
            })
        }
    };
    (held.route.len() <= 512).then_some(Chosen {
        route: held.route,
        trusted,
    })
}

/// `connect`, as the file holds it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HeldConnect {
    route: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    trusted: Option<HeldPin>,
}

/// A pin, as the file holds it: as a pool's `pin` in `runners.json`.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HeldPin {
    #[serde(rename = "hostKey")]
    host_key: String,
    engine: String,
}

/// The file's form. Its fields are spelled out and not flattened from [`Choices`]: flattening
/// reads a number as something else under serde_json's `arbitrary_precision`.
#[derive(Serialize, Deserialize)]
struct Held {
    version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kind: Option<Kind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    operator_image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    name_free: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pool: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    skipped: BTreeMap<String, Vec<StepId>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    tried: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    admin: Option<admin::Asked>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    made: Option<MadeKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    connect: Option<HeldConnect>,
}

fn write_choices(config: &Path, choices: Choices) -> Result<(), Refused> {
    let held = Held {
        version: 1,
        kind: choices.kind,
        context: choices.context,
        operator_image: choices.operator_image,
        name: choices.name,
        name_free: choices.name_free,
        pool: choices.pool,
        skipped: choices.skipped,
        tried: choices.tried,
        admin: choices.admin,
        made: choices.made,
        connect: choices.connect.map(|chosen| HeldConnect {
            route: chosen.route,
            trusted: chosen.trusted.map(|pin| HeldPin {
                host_key: pin.host_key.spelled(),
                engine: pin.engine,
            }),
        }),
    };
    let text = serde_json::to_string_pretty(&held)
        .map_err(|why| Refused::Other(format!("your Pool setup choices: {why}")))?;
    purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config))
        .and_then(|folder| folder.replace_file(CHOICES_FILE, format!("{text}\n").as_bytes()))
        .map_err(|why| {
            Refused::Other(format!(
                "your Pool setup choices could not be written: {why}"
            ))
        })
}

/// **Keeps the kind chosen on Where.** It changes nothing else on this machine.
pub fn choose_kind(config: &Path, kind: Kind) -> Result<(), Refused> {
    let _locked = runners::lock(config);
    let mut choices = read_choices(config)?;
    choices.kind = Some(kind);
    // Another kind on Where forgets the machine Connect read.
    if kind != Kind::RemoteDocker {
        choices.connect = None;
    }
    write_choices(config, choices)
}

/// **Keeps the machine picked or typed on Connect** (#6266), or forgets it (`None`): never a pin,
/// which only [`trusted`] keeps. Refused for an address too long to be one.
pub fn choose_machine(config: &Path, route: Option<&str>) -> Result<(), Refused> {
    let route = route.map(str::trim).filter(|route| !route.is_empty());
    if route.is_some_and(|route| route.len() > 512) {
        return Err(Refused::Other(
            "that address is too long to be a machine's".to_owned(),
        ));
    }
    let _locked = runners::lock(config);
    let mut choices = read_choices(config)?;
    choices.kind = Some(Kind::RemoteDocker);
    choices.connect = route.map(|route| Chosen {
        route: route.to_owned(),
        trusted: None,
    });
    write_choices(config, choices)
}

/// **Forgets the machine Connect read** (#6266), once a pool is made on it.
pub fn forget_machine(config: &Path) -> Result<(), Refused> {
    let _locked = runners::lock(config);
    let mut choices = read_choices(config)?;
    if choices.connect.take().is_some() {
        write_choices(config, choices)?;
    }
    Ok(())
}

/// **Keeps what the person trusted on Connect** (#6266): the route with its socket, and the pin
/// the Pool step's pool is made with.
pub fn trusted(config: &Path, remote: &crate::runners::Remote) -> Result<(), Refused> {
    let _locked = runners::lock(config);
    let mut choices = read_choices(config)?;
    choices.kind = Some(Kind::RemoteDocker);
    choices.connect = Some(Chosen {
        route: remote.route.spelled(),
        trusted: Some(remote.pin.clone()),
    });
    write_choices(config, choices)
}

/// **Keeps that the person chose their team's cluster on Where, through kubeconfig context
/// `context`** (#4492): its name alone, nothing of what it signs in with. Refused for a name no
/// context could have.
pub fn choose_cluster(config: &Path, context: &str) -> Result<(), Refused> {
    if !crate::clusterpool::is_context(context) {
        return Err(Refused::Other(format!(
            "\"{}\" isn't a kubeconfig context's name",
            crate::shown::short(context)
        )));
    }
    let _locked = runners::lock(config);
    let mut choices = read_choices(config)?;
    choices.kind = Some(Kind::Cluster);
    choices.context = Some(context.to_owned());
    write_choices(config, choices)
}

/// **Applies part `part` of the install to the cluster chosen on Where** (#4493), with the
/// person's own sign-in through `kubectl`, on Install's own button. Refused where no cluster was
/// chosen, so nothing is applied to a context the person didn't pick.
pub fn install_part(
    config: &Path,
    kubectl: &crate::clusterpool::Kubectl,
    part: install::PartId,
) -> Result<(), Refused> {
    let choices = read_choices(config)?;
    let (Some(Kind::Cluster), Some(context)) = (choices.kind, choices.context.as_deref()) else {
        return Err(Refused::Other(
            "nothing was applied: choose your team's cluster on Where first".to_owned(),
        ));
    };
    let made = install::apply(
        &kubectl.clone().with_context(context),
        part,
        choices.operator_image.as_deref(),
    )?;
    // The signing key this setup made (#4495): its own Pools' key is then pinned without a paste.
    let Some(key) = made else {
        return Ok(());
    };
    let context = context.to_owned();
    let _locked = runners::lock(config);
    let mut choices = read_choices(config)?;
    choices.made = Some(MadeKey {
        context,
        key: key.spelled(),
    });
    write_choices(config, choices)
}

/// **The signing key this setup made** (#4495): on which context, and its public half, as
/// `ed25519:` and 64 hex digits. Only the public half: the key itself went to the cluster alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MadeKey {
    pub context: String,
    pub key: String,
}

/// **How the cluster operator key `named` is confirmed before it is pinned** (#4495): by the key
/// the person `pasted`, as their cluster's admin gave it, which must be the same; or, with
/// nothing pasted, only where this machine made the signing key in this setup, on `context`, and
/// `named` is its public half. Anything else pins nothing.
pub fn confirm_key(
    config: &Path,
    context: &str,
    named: &crate::clusterpool::OperatorKey,
    pasted: Option<&str>,
) -> Result<crate::clusterpool::Confirmed, Refused> {
    use crate::clusterpool::{Confirmed, OperatorKey};
    if let Some(pasted) = pasted.filter(|pasted| !pasted.trim().is_empty()) {
        let pasted =
            OperatorKey::from_pasted(pasted).map_err(|why| Refused::Other(why.to_string()))?;
        return if &pasted == named {
            Ok(Confirmed::Pasted)
        } else {
            Err(Refused::Other(
                "the key you pasted isn't the one the cluster's pool names, so nothing was \
                 pinned. Stop here and check with your cluster's admin: you may be reaching \
                 another cluster."
                    .to_owned(),
            ))
        };
    }
    if made_here(config, context, named) {
        return Ok(Confirmed::MadeHere);
    }
    Err(Refused::Other(
        "nothing was pinned: paste the key your cluster's admin gave you, and purlis pins the \
         pool's key only if it is the same."
            .to_owned(),
    ))
}

/// **Whether this machine made the signing key whose public half is `named`, in this setup, on
/// `context`** (#4495): then no paste is asked for.
#[must_use]
pub fn made_here(config: &Path, context: &str, named: &crate::clusterpool::OperatorKey) -> bool {
    read_choices(config)
        .ok()
        .and_then(|choices| choices.made)
        .is_some_and(|made| {
            made.context == context
                && crate::clusterpool::OperatorKey::from_spelled(&made.key)
                    .is_ok_and(|key| &key == named)
        })
}

/// **Makes the Pool `name` in the cluster chosen on Where** (#4494), of runner image `image` by
/// digest, with the person's own sign-in, on the Pool step's own button. Refused where no
/// cluster was chosen.
pub fn create_cluster_pool(
    config: &Path,
    kubectl: &crate::clusterpool::Kubectl,
    name: &str,
    image: &str,
) -> Result<(), Refused> {
    let choices = read_choices(config)?;
    let (Some(Kind::Cluster), Some(context)) = (choices.kind, choices.context.as_deref()) else {
        return Err(Refused::Other(
            "nothing was made: choose your team's cluster on Where first".to_owned(),
        ));
    };
    cluster::create_pool(&kubectl.clone().with_context(context), name, image)
}

/// **Keeps the cluster operator's image named under Install's Options** (#4493), or none. Refused
/// for an image not named by its digest, so the operator runs only what was checked.
pub fn choose_operator_image(config: &Path, image: Option<&str>) -> Result<(), Refused> {
    if let Some(image) = image
        && !install::is_by_digest(image)
    {
        return Err(Refused::Other(format!(
            "name the operator's image by its digest, {}@sha256: and 64 hex digits",
            install::OPERATOR_REPOSITORY
        )));
    }
    let _locked = runners::lock(config);
    let mut choices = read_choices(config)?;
    choices.operator_image = image.map(str::to_owned);
    write_choices(config, choices)
}

/// **Keeps the name given on the Pool step**, or none, so a person who comes back finds it.
///
/// Refused for a name no pool could have, so the file stays small and what it holds stays a name.
pub fn choose_name(config: &Path, name: Option<&str>) -> Result<(), Refused> {
    if let Some(name) = name {
        runners::usable_pool_name(name).map_err(Refused::Other)?;
    }
    let _locked = runners::lock(config);
    let mut choices = read_choices(config)?;
    let kept = runners::pools(config)?;
    choices.name_free = name.is_some_and(|name| !kept.iter().any(|pool| pool.name() == name));
    choices.name = name.map(str::to_owned);
    write_choices(config, choices)
}

/// The names of the pools this machine keeps, Docker's and clusters' (#4494).
fn kept_pools(config: &Path) -> Result<Vec<String>, Refused> {
    let mut names: Vec<String> = runners::pools(config)?
        .iter()
        .map(|pool| pool.name().to_owned())
        .collect();
    names.extend(
        runners::cluster_pools(config)?
            .iter()
            .map(|pool| pool.name().to_owned()),
    );
    Ok(names)
}

/// **Keeps that this setup goes on with pool `name`**, once it has made it. Refused for a pool
/// this machine doesn't keep.
pub fn choose_pool(config: &Path, name: &str) -> Result<(), Refused> {
    let _locked = runners::lock(config);
    if !kept_pools(config)?.iter().any(|one| one == name) {
        return Err(Refused::Other(format!(
            "there is no pool named {name} on this machine"
        )));
    }
    let mut choices = read_choices(config)?;
    choices.pool = Some(name.to_owned());
    choices.name = None;
    choices.name_free = false;
    // What Connect kept is the pool's now (#6266).
    choices.connect = None;
    write_choices(config, choices)
}

/// **Where the person stopped** ([`resumed`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resumed {
    pub setup: Setup,
    /// The name typed, for the box to hold.
    pub name: Option<String>,
    /// The pool was made by hand, from the name typed, and purlis noticed.
    pub noticed: bool,
    /// The name a new pool is recommended: one no pool here has.
    pub recommended: String,
}

/// **Where the person stopped**: the setup of the pool it made where that is still kept, else of a
/// pool made since under the name typed, by hand from See what this does, else a new pool's.
///
/// A name typed never picks a pool that was already there: only one that came after it does.
pub fn resumed(here: &Here<'_>) -> Result<Resumed, Refused> {
    resumed_pressed(here, None)
}

/// **[`resumed`], with what a press of Connect just found** laid over Connect's facts (#6266).
pub fn resumed_pressed(here: &Here<'_>, pressed: Option<Pressed>) -> Result<Resumed, Refused> {
    let choices = read_choices(here.config)?;
    let kept = kept_pools(here.config)?;
    let on = pool_of(&choices, &kept);
    let mut setup = read_pressed(here, on, pressed)?;
    let noticed = on.is_some() && on != choices.pool.as_deref();
    let recommended = recommended_name(&kept);
    setup.others = kept
        .into_iter()
        .filter(|name| setup.pool.as_ref() != Some(name))
        .collect();
    Ok(Resumed {
        setup,
        name: choices.name,
        noticed,
        recommended,
    })
}

/// **What Add a pool found** ([`start_adding`], #6256).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adding {
    /// Nothing unfinished: setup starts at its first step, nothing filled in.
    New,
    /// A new pool's setup was started and not finished: setup goes on with it, and says so.
    Continuing,
}

/// The pool a setup is on, from what was chosen: the one it made where that is still kept, else
/// one made since, by hand, under the name typed ([`resumed`]).
fn pool_of<'a>(choices: &'a Choices, kept: &[String]) -> Option<&'a str> {
    let is_kept = |name: &&str| kept.iter().any(|pool| pool == name);
    let chosen = choices.pool.as_deref().filter(is_kept);
    chosen.or_else(|| {
        choices
            .name
            .as_deref()
            .filter(|_| choices.name_free)
            .filter(is_kept)
    })
}

/// **Add a pool: a new pool's setup, unless one was started and not finished** (#6256). A setup
/// is finished once a real chat started on its pool (Try it's proof, [`tried`]): that pool's
/// setup is then reached from its own Open setup, never from Add. Read from what is kept alone,
/// so Docker being down never makes a finished setup look unfinished.
///
/// Starting new forgets what this setup chose, and nothing of any pool's: each pool keeps its
/// skips and its proof.
pub fn start_adding(config: &Path) -> Result<Adding, Refused> {
    let _locked = runners::lock(config);
    let choices = read_choices(config)?;
    let kept = kept_pools(config)?;
    let unfinished = match pool_of(&choices, &kept) {
        Some(pool) => !choices.tried.contains_key(pool),
        None => choices.name.is_some(),
    };
    if unfinished {
        return Ok(Adding::Continuing);
    }
    forget_the_new_one(config, choices)?;
    Ok(Adding::New)
}

/// **Start over**: forgets the unfinished setup, so the next read is a new pool's from its first
/// step. A pool it made stays, with what was chosen for it.
pub fn start_over(config: &Path) -> Result<(), Refused> {
    let _locked = runners::lock(config);
    let choices = read_choices(config)?;
    forget_the_new_one(config, choices)
}

/// Forgets the kind, the context, the name and the pool this setup chose, writing only where one
/// was. The operator's image stays: it names what the person checked, whatever setup is next.
fn forget_the_new_one(config: &Path, choices: Choices) -> Result<(), Refused> {
    if choices.kind.is_none()
        && choices.context.is_none()
        && choices.name.is_none()
        && choices.pool.is_none()
        && choices.made.is_none()
    {
        return Ok(());
    }
    write_choices(
        config,
        Choices {
            kind: None,
            context: None,
            name: None,
            name_free: false,
            pool: None,
            // A key made in this setup confirms nothing in the next one.
            made: None,
            ..choices
        },
    )
}

/// **The name a new pool is recommended** (#6256): `docker`, else the first `docker-<n>` no pool
/// here has.
fn recommended_name(kept: &[String]) -> String {
    let taken = |name: &str| kept.iter().any(|pool| pool == name);
    std::iter::once("docker".to_owned())
        .chain((2..).map(|n| format!("docker-{n}")))
        .find(|name| !taken(name))
        .unwrap_or_default()
}

/// **This machine's `docker`**, where it has one, found as a pool's is.
#[must_use]
pub fn docker_here() -> Option<Docker> {
    let path = crate::envvar::var("PATH");
    let home = crate::envvar::var_os("HOME").map(PathBuf::from);
    Docker::on_this_machine(path.as_deref(), home.as_deref()).ok()
}

/// **Keeps that the person skipped `step` on pool `pool`.** Refused for a step later ones need.
pub fn skip(config: &Path, pool: &str, step: StepId) -> Result<(), Refused> {
    if !step.skippable() {
        return Err(Refused::Other(format!(
            "{} can't be skipped: the steps after it need it",
            step.name()
        )));
    }
    let _locked = runners::lock(config);
    if !kept_pools(config)?.iter().any(|one| one == pool) {
        return Err(Refused::Other(format!(
            "there is no pool named {pool} on this machine"
        )));
    }
    let mut choices = read_choices(config)?;
    let skipped = choices.skipped.entry(pool.to_owned()).or_default();
    if !skipped.contains(&step) {
        skipped.push(step);
    }
    write_choices(config, choices)
}

/// **Keeps that a real chat was seen to start on pool `pool` at `at`** (#4483): what Try it ends
/// on. Refused for a pool this machine doesn't keep.
pub fn tried(config: &Path, pool: &str, at: u64) -> Result<(), Refused> {
    let _locked = runners::lock(config);
    if !kept_pools(config)?.iter().any(|one| one == pool) {
        return Err(Refused::Other(format!(
            "there is no pool named {pool} on this machine"
        )));
    }
    let mut choices = read_choices(config)?;
    choices.tried.insert(pool.to_owned(), at);
    write_choices(config, choices)
}

/// **Takes back a skip**, so the person can come back to the step.
pub fn unskip(config: &Path, pool: &str, step: StepId) -> Result<(), Refused> {
    let _locked = runners::lock(config);
    let mut choices = read_choices(config)?;
    if let Some(skipped) = choices.skipped.get_mut(pool) {
        skipped.retain(|one| *one != step);
        if skipped.is_empty() {
            choices.skipped.remove(pool);
        }
    }
    write_choices(config, choices)
}

/// **Forgets what was chosen for a pool that is gone**, under the runners' lock: a new pool of the
/// same name starts clean. Nothing is written where nothing was chosen for it.
pub(crate) fn forget_pool_locked(config: &Path, pool: &str) -> Result<(), Refused> {
    let mut choices = read_choices(config)?;
    let named = choices.pool.as_deref() == Some(pool);
    if named {
        choices.pool = None;
    }
    // A name that picked this pool by hand must not pick the next one of that name.
    let typed = choices.name.as_deref() == Some(pool);
    if typed {
        choices.name = None;
        choices.name_free = false;
    }
    let tried = choices.tried.remove(pool).is_some();
    if choices.skipped.remove(pool).is_some() || named || typed || tried {
        write_choices(config, choices)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "poolsetup/confirm_tests.rs"]
mod confirm_tests;
#[cfg(test)]
mod tests;
