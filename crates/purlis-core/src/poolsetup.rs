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
//!   Allow is only looked at. One thing happens on a running runner: asking how a profile stands
//!   is its `purlis profile sign-in --check`, which fetches a harness version the profile pins, as
//!   any look at that profile does.
//! - **A Docker that isn't on this machine, or a runner that ran a chat without the sandbox, is
//!   asked nothing about its runners**, as for a sign-in. What Docker or a runner says is shown
//!   only as escaped text ([`crate::shown::short`]).
//! - **Only a person's choices are stored** ([`Choices`], `<config>/pool-setup.json`): the kind
//!   they chose and the steps they skipped. What is found always wins over a skip.
//!
//! What the facts mean is one pure function ([`of`]); what reads them is [`read`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::dockerpool::{Docker, Missing};
use crate::profilestore::sign_in::Standing;
use crate::runners::{self, Refused};

/// The file a person's choices are kept in, in this machine's folder.
const CHOICES_FILE: &str = "pool-setup.json";

/// The programs an agent's forge identity is declared for.
const FORGE_PROGRAMS: [&str; 2] = ["gh", "glab"];

/// Where a pool's chats run. Only what is built is here (Q46).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Docker,
}

impl Kind {
    /// **The steps of this kind, in order** (Q35).
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
        }
    }
}

/// One step of Pool setup, by the prototype's names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepId {
    Where,
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
}

/// The one thing to do on a step: its button, what the button does, and, where one exists, the
/// command that does the same.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Action {
    pub id: ActionId,
    pub label: String,
    pub does: String,
    pub command: Option<String>,
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
}

/// The whole answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Setup {
    pub kind: Kind,
    pub pool: Option<String>,
    /// The first step that isn't finished. `None` once all are.
    pub at: Option<StepId>,
    pub steps: Vec<Step>,
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
}

/// A pool's facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolFacts {
    pub name: String,
    pub runner: Runner,
    pub sign_ins: SignIns,
}

/// Everything read, before it means anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    pub engine: Engine,
    pub pool: Option<PoolFacts>,
    pub identity: Identity,
}

/// **A person's choices, the only thing stored** (tier: Machine, device-bound).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Choices {
    /// The kind chosen on Where, before there is a pool.
    pub kind: Option<Kind>,
    /// The steps skipped on each pool.
    pub skipped: BTreeMap<String, Vec<StepId>>,
}

/// What this machine calls itself in a sentence.
fn this_machine() -> &'static str {
    if cfg!(target_os = "macos") {
        "this Mac"
    } else {
        "this machine"
    }
}

fn action(id: ActionId, label: &str, does: impl Into<String>) -> Option<Action> {
    Some(Action {
        id,
        label: label.to_owned(),
        does: does.into(),
        command: None,
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

fn where_reading(facts: &Facts, choices: &Choices) -> Reading {
    match facts.engine {
        Engine::Missing => Reading::attention(
            "You need Docker first. It runs your chats in a container on this machine, apart \
             from your files.",
            action(
                ActionId::GetDocker,
                "Get Docker",
                "Opens Docker's download page in your browser. purlis doesn't install anything, \
                 and checks again by itself.",
            ),
        ),
        Engine::NotAnswering => Reading::attention(
            "Docker needs to be running. purlis found it, and it isn't answering. If it is \
             running, check that your user may use it and that DOCKER_HOST, if you set it, \
             names a socket on this machine.",
            action(
                ActionId::StartDocker,
                "Start Docker",
                "Start Docker the way you usually do. purlis checks again by itself.",
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
                "Use Docker",
                "Saves your choice. Nothing changes on this machine.",
            ),
        ),
    }
}

fn pool_reading(facts: &Facts) -> Reading {
    let Some(pool) = &facts.pool else {
        return Reading::to_do(
            "A pool is a named place your chats run. There isn't one yet.",
            action(
                ActionId::CreatePool,
                "Create the pool",
                "Makes one container, its runner, in Docker on this machine. Settings › Pools \
                 has the form.",
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
        Runner::None => make_again(format!("Pool {name} has no runner.")),
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
            "Allow it",
            "Allows the persona's tool on this machine. It never holds your own sign-in.",
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
    }
}

fn try_reading(facts: &Facts) -> Reading {
    let pool = facts.pool.as_ref().map_or("the pool", |pool| &pool.name);
    Reading::to_do(
        "Try it starts a real chat on the pool, so you know it works before you rely on it.",
        action(
            ActionId::StartTestChat,
            "Start a test chat",
            format!(
                "Starts a real chat on {pool}, and checks gh and git push with commands that \
                 change nothing."
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
        let reading = match id {
            StepId::Where => where_reading(facts, choices),
            StepId::Pool => pool_reading(facts),
            StepId::Signins => sign_ins_reading(facts),
            StepId::Identity => identity_reading(facts),
            StepId::Try => try_reading(facts),
        };
        let state = if reading.finished {
            StepState::Done
        } else if id.skippable() && skipped.contains(&id) && !reading.attention {
            StepState::Skipped
        } else if at.is_some() {
            StepState::Later
        } else {
            at = Some(id);
            if reading.attention {
                StepState::Attention
            } else {
                StepState::Current
            }
        };
        let says = !matches!(state, StepState::Later | StepState::Skipped);
        steps.push(Step {
            id,
            name: id.name(),
            state,
            line: reading.line,
            findings: if says { reading.findings } else { Vec::new() },
            next: if says { reading.next } else { None },
            skippable: id.skippable(),
        });
    }
    Setup {
        kind,
        pool,
        at,
        steps,
    }
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
}

/// **Reads pool `pool`'s setup, or a new Docker pool's where `pool` is `None`.** Reads everything
/// again, and changes nothing. Refused where there is no such Docker pool on this machine.
pub fn read(here: &Here<'_>, pool: Option<&str>) -> Result<Setup, Refused> {
    let choices = read_choices(here.config)?;
    let engine = match here.docker {
        None => Engine::Missing,
        Some(docker) if docker.answers().is_err() => Engine::NotAnswering,
        Some(_) if here.engine.is_err() => Engine::Elsewhere,
        Some(_) => Engine::Ready,
    };
    let pool = pool
        .map(|name| pool_facts(here, name, engine))
        .transpose()?;
    let facts = Facts {
        engine,
        pool,
        identity: identity_in(here.plane),
    };
    Ok(of(Kind::Docker, &facts, &choices))
}

fn pool_facts(here: &Here<'_>, name: &str, engine: Engine) -> Result<PoolFacts, Refused> {
    if !runners::pools(here.config)?
        .iter()
        .any(|pool| pool.name() == name)
    {
        return Err(Refused::Other(
            if runners::profiles_of(here.config, name).is_ok() {
                format!(
                    "pool {name} is on a cluster, and Pool setup reads only Docker pools so far"
                )
            } else {
                format!("there is no pool named {name} on this machine")
            },
        ));
    }
    let docker = here.docker.filter(|_| engine == Engine::Ready);
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
    // Only a Docker shown to be on this machine reaches here (`Engine::Ready`).
    let (Ok(engine), Ok(kept)) = (&here.engine, runners::runner_for(here.config, name)) else {
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
        match docker.standing_on_runner(&kept.made, engine, &profile) {
            Ok(standing) => {
                let standing = standing.map_or(Profile::NotKept, Profile::Standing);
                read.push((profile, standing));
            }
            Err(why) => return SignIns::CannotTell(crate::shown::short(&why)),
        }
    }
    SignIns::Read(read)
}

fn identity_in(plane: Option<&Path>) -> Identity {
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
    short_of_it.unwrap_or(Identity::None)
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
        skipped: held.skipped,
    })
}

/// The file's form. Its fields are spelled out and not flattened from [`Choices`]: flattening
/// reads a number as something else under serde_json's `arbitrary_precision`.
#[derive(Serialize, Deserialize)]
struct Held {
    version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kind: Option<Kind>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    skipped: BTreeMap<String, Vec<StepId>>,
}

fn write_choices(config: &Path, choices: Choices) -> Result<(), Refused> {
    let held = Held {
        version: 1,
        kind: choices.kind,
        skipped: choices.skipped,
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
    write_choices(config, choices)
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
    let mut choices = read_choices(config)?;
    let skipped = choices.skipped.entry(pool.to_owned()).or_default();
    if !skipped.contains(&step) {
        skipped.push(step);
    }
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
    if choices.skipped.remove(pool).is_some() {
        write_choices(config, choices)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
