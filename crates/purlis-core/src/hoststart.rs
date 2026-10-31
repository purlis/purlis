// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A start the session host composes itself** (#2171), and the one adapter from a start to the
//! program it runs that both binaries' hosts use (#2174).
//!
//! A start the window composed comes with its `launch`, and is run by its words
//! ([`crate::chatspec::from_launch`]). A start with no launch names only its persona, its
//! profile, its workspace, the conversation it resumes, its first message and whether the person
//! opted it out of the sandbox. The host composes the launch the window would have, with the
//! code the window runs: [`crate::start::ready`] decides the profile, the persona and the
//! sandbox, the harness is armed as the window arms it, and its line and environment are made
//! as the window makes them. Then it is run by the same adapter.
//!
//! **Only what the project's own files and this machine allow.** A peer names things; every
//! one of them is looked up here and checked by the gates the window's start passes: a profile
//! this machine declares and the person approved, a persona the project has, a workspace folder
//! inside the project reached through no link. The sandbox is the project's and this machine's,
//! with no grant of the window's: a start made here is [`crate::sandbox::Made::Unnamed`], so a
//! person's own level of `off` is never taken at it.
//!
//! **Fail closed where the window would ask or hear.** A profile that is handed a key or a
//! cloud's credentials is refused: credentials are read where the window starts its program and
//! never pass in a launch. A confinement is started by the host as the program's own, and its
//! proxy asks nobody, unless the start named it to the window's link: then the window hears it
//! and answers its asks (#2407). A person's opt-out is written in the host's event
//! log under the start's own run before anything runs, or the start is refused. A first message
//! the harness takes on no line of its own is refused, never dropped.

use std::path::{Path, PathBuf};

use purlis_session_protocol::session::{self, Launch, ProgramKind, StartProgram};

use crate::engine::Size;
use crate::harness::{Harness, StateHooks};
use crate::session::Spec;

/// What purlis ships that a chat is armed with: the binary its hooks run, the plugin a Claude
/// Code chat loads, and the git hooks every harness chat commits through. Found by the binary
/// that hosts.
#[derive(Debug, Clone, Default)]
pub struct Kit {
    pub binary: Option<PathBuf>,
    pub plugin: Option<PathBuf>,
    pub git_hooks: Option<crate::githooks::GitHooks>,
    /// Where the folders of links a chat is handed the plane's skills by are laid out
    /// ([`crate::skills::lay_out`]): `<app data>/skill-sets`, where purlis has a data folder.
    pub skill_sets: Option<PathBuf>,
    /// Where a chat's settings too long for its command line are kept ([`settings_in_a_file`]):
    /// `<app data>/chat-settings`, where purlis has a data folder.
    pub chat_settings: Option<PathBuf>,
}

impl Kit {
    /// Where each part of this kit lies, as a sandbox holds them
    /// ([`crate::sandbox::Applied::hold_kit`]).
    pub fn armed_with(&self) -> crate::sandbox::ArmedWith<'_> {
        crate::sandbox::ArmedWith {
            binary: self.binary.as_deref(),
            plugin: self.plugin.as_deref(),
            git_hooks: self.git_hooks.as_ref().map(crate::githooks::GitHooks::dir),
            skill_sets: self.skill_sets.as_deref(),
        }
    }

    /// What a host on this machine arms a chat with: purlis's own `binary`, the bundled
    /// `plugin` where it was found, and the git hooks the window wrote in its data folder at
    /// its launch, where they are.
    pub fn found(binary: Option<PathBuf>, plugin: Option<PathBuf>) -> Kit {
        let git_hooks = dirs::data_dir()
            .map(|data| data.join(crate::names::BUNDLE_ID.write).join(GIT_HOOKS))
            .filter(|dir| dir.is_dir())
            .map(|dir| crate::githooks::GitHooks::at(&dir));
        Kit {
            binary,
            plugin,
            git_hooks,
            skill_sets: skill_sets_here(),
            chat_settings: chat_settings_here(),
        }
    }

    /// **What a host with no window arms a chat with** (`purlis serve`, #2447): its own
    /// `binary`, the plugin installed beside it ([`crate::plugin_install::bundle_beside`]: where
    /// a package or the runner image puts it), and purlis's git hooks, written now in `data`,
    /// its data folder, to run `binary`, as the window writes them at its launch. Where none is
    /// installed beside it, the kit has no plugin, and a sandboxed Claude Code chat is refused
    /// as it is anywhere the plugin is missing. Git hooks that cannot be written leave a chat's
    /// commits unscanned, and say so in the host's log.
    pub fn installed_beside(binary: PathBuf, data: &Path) -> Kit {
        let plugin = crate::plugin_install::bundle_beside(&binary);
        let dir = data.join(crate::names::BUNDLE_ID.write).join(GIT_HOOKS);
        let hooks = crate::githooks::GitHooks::at(&dir);
        let git_hooks = match hooks.write(&binary) {
            Ok(()) => Some(hooks),
            Err(why) if why.kind() == std::io::ErrorKind::Unsupported => None,
            Err(why) => {
                tracing::warn!(
                    "purlis: no git hooks at {} ({why}); a chat's commits will not be scanned \
                     for secrets",
                    dir.display()
                );
                None
            }
        };
        Kit {
            binary: Some(binary),
            plugin,
            git_hooks,
            skill_sets: Some(skill_sets_in(data)),
            chat_settings: Some(chat_settings_in(data)),
        }
    }

    /// [`Kit::installed_beside`] for this process: this binary, and this user's data folder.
    /// With no binary to name, it arms nothing, and a sandboxed chat is refused.
    pub fn installed_beside_this_binary() -> Kit {
        match (std::env::current_exe(), dirs::data_dir()) {
            (Ok(binary), Some(data)) => Kit::installed_beside(binary, &data),
            (binary, _) => Kit {
                binary: binary.ok(),
                plugin: None,
                git_hooks: None,
                skill_sets: None,
                chat_settings: None,
            },
        }
    }
}

/// The folder of the app's data folder the window writes purlis's git hooks in at its launch.
pub const GIT_HOOKS: &str = "git-hooks";

/// The folder of the app's data folder a chat's folder of links to the plane's skills is laid
/// out in ([`crate::skills::lay_out`]).
pub const SKILL_SETS: &str = "skill-sets";

/// [`SKILL_SETS`] in this user's app data folder, beside the git hooks the window writes there.
pub fn skill_sets_here() -> Option<PathBuf> {
    dirs::data_dir().map(|data| skill_sets_in(&data))
}

/// [`SKILL_SETS`] in the app data folder of the data home `data`: the one spelling of its path.
pub fn skill_sets_in(data: &Path) -> PathBuf {
    data.join(crate::names::BUNDLE_ID.write).join(SKILL_SETS)
}

/// The folder of the app's data folder chats' settings files are kept in, one folder per plane
/// by its clone key ([`settings_in_a_file`]).
pub const CHAT_SETTINGS: &str = "chat-settings";

/// [`CHAT_SETTINGS`] in this user's app data folder.
pub fn chat_settings_here() -> Option<PathBuf> {
    dirs::data_dir().map(|data| chat_settings_in(&data))
}

/// [`CHAT_SETTINGS`] in the app data folder of the data home `data`.
pub fn chat_settings_in(data: &Path) -> PathBuf {
    data.join(crate::names::BUNDLE_ID.write).join(CHAT_SETTINGS)
}

/// **The folder of links a chat in `root` is handed its set's folder by**, laid out by the host
/// before the chat is armed where `kit` says where folders go
/// ([`crate::skills::lay_out_for_chat`]), each skill as `harness` reads it: the plane's skills
/// where the Skill studio's flag is `on`, and on a route that names the built-in skills their own
/// (opencode's, `purlis-<name>`) the built-in ones, flag or none. `None` otherwise, and for a set
/// with none of them.
pub fn plane_skills_laid_out(
    on: bool,
    root: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    kit: &Kit,
    harness: Harness,
) -> Option<PathBuf> {
    let at = on.then_some(crate::skills::PlaneSkills {
        plane: root,
        workspace,
        repo: &[],
        routed: None,
    });
    laid_out_on(at, persona, kit, harness)
}

/// [`plane_skills_laid_out`] for a chat on `harness` whose plane skills come from `at`, with the
/// repo skills of the clone it works in (SK-6) and its router's answer (SK-15b): a plane skill a
/// narrower repo copy hides, or its router hid, is not laid out. `None` where they do not reach
/// it.
fn laid_out_on(
    at: Option<crate::skills::PlaneSkills<'_>>,
    persona: Option<&str>,
    kit: &Kit,
    harness: Harness,
) -> Option<PathBuf> {
    let own_names = harness.skills().names_built_ins_its_own();
    if at.is_none() && !own_names {
        return None;
    }
    let Some(sets) = kit.skill_sets.as_deref() else {
        if own_names {
            tracing::warn!(
                "purlis: this chat's skills have no folder to be laid out in; {}",
                crate::skills::without_the_folder(harness)
            );
        }
        return None;
    };
    let built_in = kit.plugin.as_deref().and_then(crate::skills::in_bundle);
    crate::skills::lay_out_for_chat(at, persona, built_in.as_deref(), sets, harness)
}

/// **Where a chat's plane skills come from, read once at its start** (#3234): what arms it
/// ([`Arming::skills`]) and what its tab's Notice says it starts without ([`skills_not_here`])
/// are both worked out from this one value, so the two never disagree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatSkills {
    /// Whether the plane's skills reach the chat: the Skill studio's flag, once the project
    /// requires them (`skills::reach_chats`, #2784).
    pub on: bool,
    /// The project the plane's skills are read from: the chat's, or a Try it's own scope (SK-9).
    pub root: PathBuf,
    /// A Try it's scratch chat: the draft alone, with no workspace's, persona's or harness's.
    pub tried: bool,
    /// The workspace the chat works in, and the persona whose skills it gets; neither for a
    /// Try it's chat.
    pub workspace: Option<String>,
    pub persona: Option<String>,
    /// The repo skills of the clone the chat works in (SK-6), as its harness reads them:
    /// nothing where the plane's skills do not reach it, or it works in no clone.
    pub repo: Vec<crate::skills::OutsideSkill>,
    /// What its router extension made of its set at its start (SK-15b, [`Self::routed_by`]):
    /// `None` for a chat routed by the built-in router, which is asked nothing.
    pub routed: Option<std::sync::Arc<crate::skills::router::Routed>>,
    /// The router a router's grade's arm is routed by (SK-15e,
    /// [`crate::skills::verbs::drafts::arm_router`]): what a Try it's scratch chat of that arm is
    /// routed by in place of a workspace's or persona's choice. `None` for every other chat.
    pub arm_router: Option<String>,
}

impl ChatSkills {
    /// **The chat in the project at `root`, started in `cwd` on `harness` as `persona`**, with
    /// the plane's skills reaching it where `on` says so: its scope, its workspace and the repo
    /// skills of its clone, each read here once.
    pub fn read(
        on: bool,
        root: &Path,
        harness: Option<Harness>,
        cwd: Option<&Path>,
        persona: Option<&str>,
    ) -> Self {
        // A Try it's scratch chat (SK-9, #2772): its project skills are the one skill its try
        // laid out, at a root of its own, so it gets the built-in skills and the draft alone.
        let scope = crate::skills::verbs::drafts::scope_of(root, cwd.filter(|_| on));
        let (workspace, persona) = if scope.tried {
            (None, None)
        } else {
            (
                cwd.and_then(|cwd| crate::active::workspace_of_tree(root, cwd)),
                persona.map(str::to_owned),
            )
        };
        // The repo skills of the clone it works in, so a narrower repo copy of a plane skill is
        // the one its harness gets where purlis builds what it is handed (SK-6).
        let repo = match (on, harness, workspace.as_deref(), cwd) {
            (true, Some(harness), Some(ws), Some(cwd)) => {
                crate::skills::repo_skills_of_chat(harness, root, ws, cwd)
            }
            _ => Vec::new(),
        };
        // A router's grade's arm is routed by the router it grades (SK-15e).
        let arm_router = cwd
            .filter(|_| scope.tried)
            .and_then(|cwd| crate::skills::verbs::drafts::arm_router(root, cwd));
        ChatSkills {
            on,
            root: scope.root,
            tried: scope.tried,
            workspace,
            persona,
            repo,
            routed: None,
            arm_router,
        }
    }

    /// **These skills, routed by the router the chat's persona or workspace chose** (SK-15b,
    /// ADR 0102 §5, §6): where it is an extension's, `asker` asks it once, now, within
    /// [`crate::extension::studio::ROUTER_DEADLINE`], which of the set's skills to hide, and the
    /// answer or the built-in router's fallback is kept as [`Self::routed`], what every later
    /// routing of the chat's set applies. Nothing is asked where the plane's skills do not reach
    /// the chat, for a Try it's scratch chat, or where the built-in router is the one chosen. A
    /// router's grade's arm is routed by the router it grades ([`Self::arm_router`]).
    #[must_use]
    pub fn routed_by(
        self,
        built_in: Option<&Path>,
        harness: Option<Harness>,
        asker: Option<&std::sync::Arc<dyn crate::skills::router::AskRouter>>,
    ) -> Self {
        let Some(harness) =
            harness.filter(|_| self.on && (!self.tried || self.arm_router.is_some()))
        else {
            return self;
        };
        let chosen = match &self.arm_router {
            Some(id) => Ok(crate::skills::router::Chosen::Extension(id.clone())),
            None => crate::skills::router::chosen(
                &self.root,
                self.workspace.as_deref(),
                self.persona.as_deref(),
            ),
        };
        if chosen == Ok(crate::skills::router::Chosen::BuiltIn) {
            return self;
        }
        let (set, pins) = crate::skills::unrouted(
            built_in,
            crate::skills::PlaneSkills {
                plane: &self.root,
                workspace: self.workspace.as_deref(),
                repo: &self.repo,
                routed: None,
            },
            self.persona.as_deref(),
            harness,
        );
        let routed = crate::skills::router::at_start(
            &set,
            chosen,
            harness.skill_budget(),
            &pins,
            asker,
            crate::extension::studio::ROUTER_DEADLINE,
        );
        Self {
            routed: routed.map(std::sync::Arc::new),
            ..self
        }
    }

    /// **These skills, routed as the chat's start routed them** (SK-15f, #3578): for a chat
    /// reattached from the session host, whose harness still holds the listing that start handed
    /// it. Its router is not asked again; a chat the plane's skills do not reach, a Try it's, or
    /// one on no harness purlis knows, keeps routing nothing, as [`Self::routed_by`] would.
    #[must_use]
    pub fn routed_as(self, kept: crate::skills::router::Kept, harness: Option<Harness>) -> Self {
        if !self.on || self.tried || harness.is_none() {
            return self;
        }
        Self {
            routed: kept.routed().map(std::sync::Arc::new),
            ..self
        }
    }
}

/// **The skills a chat starts without** (SK-5, #2768; SK-8, #2771), where the plane's skills
/// reach it: the chat whose skills are `skills`, on `harness` with the environment `env`
/// ([`crate::skills::not_here_beside`]), beside the very repo skills it was armed with (#3234).
/// What its tab's Notice says. `None` otherwise, and where it has every one.
pub fn skills_not_here(
    skills: &ChatSkills,
    kit: &Kit,
    harness: Option<Harness>,
    env: &[(String, String)],
) -> Option<crate::skills::NotHere> {
    if !skills.on {
        return None;
    }
    let harness = harness?;
    let built_in = kit.plugin.as_deref().and_then(crate::skills::in_bundle);
    crate::skills::not_here_beside(
        &skills.root,
        skills.workspace.as_deref(),
        skills.persona.as_deref(),
        built_in.as_deref(),
        harness,
        &crate::harness_plugin::Env::of(env),
        &skills.repo,
    )
}

/// The host's [`Beside`], as two calls: what starts a confinement and what writes the log.
impl<C, R> Beside for (C, R)
where
    C: Fn(&session::ConfinePlan) -> Result<session::Confined, String>,
    R: Fn(serde_json::Value) -> Result<(), String>,
{
    fn confine(&self, plan: &session::ConfinePlan) -> Result<session::Confined, String> {
        (self.0)(plan)
    }

    fn record(&self, entry: serde_json::Value) -> Result<(), String> {
        (self.1)(entry)
    }
}

/// The host's [`Beside`] as two calls and what asks a chat's router extension at its start
/// (SK-15b).
impl<C, R> Beside for (C, R, std::sync::Arc<dyn crate::skills::router::AskRouter>)
where
    C: Fn(&session::ConfinePlan) -> Result<session::Confined, String>,
    R: Fn(serde_json::Value) -> Result<(), String>,
{
    fn confine(&self, plan: &session::ConfinePlan) -> Result<session::Confined, String> {
        (self.0)(plan)
    }

    fn record(&self, entry: serde_json::Value) -> Result<(), String> {
        (self.1)(entry)
    }

    fn router(&self) -> Option<std::sync::Arc<dyn crate::skills::router::AskRouter>> {
        Some(std::sync::Arc::clone(&self.2))
    }
}

/// The host's [`Beside`] as two calls, what asks a chat's router extension at its start
/// (SK-15b), and what reads a confinement it started as it is now (#3407).
impl<C, R, L> Beside
    for (
        C,
        R,
        std::sync::Arc<dyn crate::skills::router::AskRouter>,
        L,
    )
where
    C: Fn(&session::ConfinePlan) -> Result<session::Confined, String>,
    R: Fn(serde_json::Value) -> Result<(), String>,
    L: Fn(&str) -> Option<crate::hostedchats::Live>,
{
    fn confine(&self, plan: &session::ConfinePlan) -> Result<session::Confined, String> {
        (self.0)(plan)
    }

    fn record(&self, entry: serde_json::Value) -> Result<(), String> {
        (self.1)(entry)
    }

    fn router(&self) -> Option<std::sync::Arc<dyn crate::skills::router::AskRouter>> {
        Some(std::sync::Arc::clone(&self.2))
    }

    fn reach_of(&self, confinement: &str) -> Option<crate::hostedchats::Live> {
        (self.3)(confinement)
    }
}

/// What the host does beside a start it composes: what [`compose`] asks of it.
pub trait Beside {
    /// The confinement `plan` plans, started by the host as the program's own.
    fn confine(&self, plan: &session::ConfinePlan) -> Result<session::Confined, String>;
    /// Writes `entry` in the host's event log, durably, or says why not.
    fn record(&self, entry: serde_json::Value) -> Result<(), String>;
    /// What asks a chat's router extension at its start (SK-15b): `None` where this host asks
    /// none, so a chat whose workspace or persona chose one is routed by the built-in router,
    /// and its routing record says why.
    fn router(&self) -> Option<std::sync::Arc<dyn crate::skills::router::AskRouter>> {
        None
    }
    /// What reads confinement `confinement`, which this host started for the chat being
    /// composed, as it is now (#3407): the hosts allowed live for the chat since it started,
    /// which a run of its tool here is held to too. `None` where this host reads none, and the
    /// chat's runs are held to what its sandbox was compiled to.
    fn reach_of(&self, confinement: &str) -> Option<crate::hostedchats::Live> {
        let _ = confinement;
        None
    }
}

/// A start, as the host hands it to be composed.
pub struct Starting<'a> {
    /// Where the project it is in is on this machine.
    pub root: &'a Path,
    /// Its number in the project.
    pub number: u32,
    /// The start, as the client sent it.
    pub start: &'a StartProgram,
    /// Where its hooks report, and its token, as its project's channel issued them.
    pub reporting: Option<&'a session::Reporting>,
    /// The chat's prompt key, as the host minted it for a start it composes (#3369).
    pub prompt_key: Option<&'a session::PromptKey>,
    /// The scope of the link the start came on, by its word: what its run says started it.
    pub started_by: &'a str,
}

/// **The program a start runs, as the binary that hosts composes it** (#2174): the window's
/// launch by its words, or, with none, the launch [`compose`] makes, each against the
/// environment this process was started with.
pub fn spec(
    starting: &Starting<'_>,
    size: Size,
    kit: &Kit,
    beside: &dyn Beside,
) -> Result<Spec, String> {
    match &starting.start.launch {
        Some(launch) => {
            // A launch by its words is in no sandbox of this host's making: a shell alone, then.
            if starting.start.kind != ProgramKind::Shell {
                refused_where_a_token_is_near(starting.start.kind)?;
            }
            crate::chatspec::from_launch_here(launch, starting.number, size)
        }
        None => {
            let mut launch = compose(starting, kit, beside)?;
            launch.reporting = starting.reporting.cloned();
            launch.prompt_key = starting.prompt_key.cloned();
            crate::chatspec::from_launch_here(&launch, starting.number, size)
        }
    }
}

/// **The launch the window would compose for `starting`**, which carries no launch of its own,
/// or why it is not started, in a sentence (module docs).
pub fn compose(starting: &Starting<'_>, kit: &Kit, beside: &dyn Beside) -> Result<Launch, String> {
    let (root, start) = (starting.root, starting.start);
    if start.kind != ProgramKind::Harness {
        return Err(
            "this session host composes a chat's harness; a shell is started from the window's \
             own launch"
                .into(),
        );
    }
    let Some(profile) = start.profile.clone() else {
        return Err(
            "a start this session host composes names the profile it runs on; none was named"
                .into(),
        );
    };
    let cwd = match &start.workspace {
        Some(workspace) => {
            crate::dispatchplace::workspace_folder(root, workspace)
                .map_err(|refused| refused.in_window())?
                .1
        }
        None => root.to_path_buf(),
    };
    let resume = start
        .resume
        .as_deref()
        .map(crate::harness::SessionId::new)
        .transpose()
        .map_err(|why| format!("the conversation to resume is not one: {why}"))?;
    let ready = crate::start::ready(
        &crate::start::Start {
            profile: Some(profile),
            persona: start.persona.clone(),
            name: format!("chat {}", starting.number),
            cwd: Some(cwd.clone()),
            resume,
            show_footer: false,
            resuming: None,
            without_sandbox: start.without_sandbox.then(crate::sandbox::OptOut::default),
            held: None,
            grants: crate::sandbox::grant::Grants {
                tools_allowed_by_desktop: what_its_desktop_allowed(starting),
                ..Default::default()
            },
            made: crate::sandbox::Made::Unnamed,
        },
        root,
    )?;
    if ready.sandbox.is_none() {
        refused_where_a_token_is_near(start.kind)?;
    }
    // Credentials are read where the window starts its program, and never pass in a launch.
    if ready
        .sign_in
        .as_ref()
        .is_some_and(|sign_in| !sign_in.credentials.is_empty())
    {
        return Err(NOT_HANDED.into());
    }
    compose_ready(starting, ready, kit, beside)
}

/// What a start with no sandbox is told on a runner told of the operator's mint port.
pub const NO_SANDBOX_WHERE_A_TOKEN_IS_NEAR: &str = "this runner's pool mints GitHub tokens in the \
     cluster, so a chat here runs only inside purlis's sandbox, and this one would not, so nothing \
     was started. Start it sandboxed.";

/// **Refuses a start outside the sandbox on a runner of a Pool naming an App's key, or told of a
/// mint port** (#4207, the person's ruling of 2026-10-31), a shell alone being allowed. What this runner keeps of the port and cannot
/// read refuses too.
#[cfg(unix)]
fn refused_where_a_token_is_near(kind: ProgramKind) -> Result<(), String> {
    if crate::envvar::var(crate::forge::mintport::MARK_VAR).is_some() {
        return no_sandbox_refused(kind, Ok(true));
    }
    // A fenced build keeps no store of this machine's, so it was told of nothing.
    let told = crate::forge::apptoken::record_root().map_or(Ok(false), |config| {
        crate::forge::mintport::told(&config).map(|told| told.is_some())
    });
    no_sandbox_refused(kind, told)
}

#[cfg(not(unix))]
fn refused_where_a_token_is_near(_kind: ProgramKind) -> Result<(), String> {
    Ok(())
}

/// [`refused_where_a_token_is_near`], from whether this runner was told of a port.
pub fn no_sandbox_refused(kind: ProgramKind, told: Result<bool, String>) -> Result<(), String> {
    match told {
        _ if kind == ProgramKind::Shell => Ok(()),
        Ok(false) => Ok(()),
        Ok(true) | Err(_) => Err(NO_SANDBOX_WHERE_A_TOKEN_IS_NEAR.to_owned()),
    }
}

/// **What a start from this runner's desktop says its person allowed of the chat's persona**
/// (#4071): a runner has nobody to allow it. Only a link's start says so: a start of this
/// machine's is held to this machine's Allow.
fn what_its_desktop_allowed(starting: &Starting<'_>) -> Option<String> {
    (starting.started_by == purlis_session_protocol::auth::Scope::RemoteLink.word())
        .then(|| starting.start.tools_allowed.clone())
        .flatten()
}

/// **Whether the plane's skills reach a chat this host composes** (SK-20b, #3084): the Skill
/// studio's flag as the desktop that starts it on this runner read it, where its start says
/// (1.62); a runner has no person to turn the preview on. A start that does not say, and one of
/// this machine's, takes this machine's own reading ([`crate::skills::reach_chats_here`]).
fn plane_skills_of(starting: &Starting<'_>) -> bool {
    its_desktop_said(starting).unwrap_or_else(|| crate::skills::reach_chats_here(starting.root))
}

/// The desktop's word on the Skill studio's flag, where a link's start says it (1.62, #3084).
fn its_desktop_said(starting: &Starting<'_>) -> Option<bool> {
    (starting.started_by == purlis_session_protocol::auth::Scope::RemoteLink.word())
        .then_some(starting.start.plane_skills)
        .flatten()
}

/// **The Skill studio's flag a chat this host composes is started with** (#3084): its desktop's
/// word, as the variable that decides the flag over this machine's store
/// ([`crate::skills::STUDIO_ENV`]), so what the chat runs here (purlis's MCP server, its hooks)
/// reads the word its arming took. Nothing where its desktop did not say.
fn studio_env_of(starting: &Starting<'_>) -> Option<(String, String)> {
    its_desktop_said(starting).map(|on| {
        (
            crate::skills::STUDIO_ENV.to_owned(),
            if on { "1" } else { "0" }.to_owned(),
        )
    })
}

/// [`compose`] from what [`crate::start::ready`] decided for it: the seam a test hands a sandbox
/// compiled for another machine through.
pub fn compose_ready(
    starting: &Starting<'_>,
    ready: crate::start::Ready,
    kit: &Kit,
    beside: &dyn Beside,
) -> Result<Launch, String> {
    let (root, start) = (starting.root, starting.start);
    let cwd = ready.cwd.clone().unwrap_or_else(|| root.to_path_buf());
    let mut ready = told_first(ready, start.first_message.as_deref())?;
    ready.env.extend(studio_env_of(starting));
    // What it is armed with, never where it writes, and never written by it.
    if let Some(applied) = ready.sandbox.as_mut() {
        applied
            .hold_kit(kit.armed_with(), &cwd, &ready.env)
            .map_err(|refused| refused.to_string())?;
    }
    let persona = persona_of(&ready.env, root);
    let sandbox = ready.sandbox.as_ref();
    // Whether the plane's skills reach it, read once: its desktop's word, where it says (SK-20b).
    let plane_skills = plane_skills_of(starting);
    // No app state, no record of the folders there at a sandboxed chat's start (#3718): such a
    // chat is not started, and nothing about its start is written.
    if sandbox.is_some()
        && plane_skills
        && start.program.is_some()
        && let Some(refused) = crate::skills::levels::refusal_for_a_sandboxed_start(root)
    {
        return Err(refused);
    }
    // A sandboxed chat could make a workspace or persona folder while it runs: the ones there
    // now are noted as not its own (SK-9), before it is armed, and before anything about its start is
    // written, so a start refused here records nothing.
    if sandbox.is_some()
        && plane_skills
        && let Some(id) = start.program.as_deref()
    {
        let name = start
            .number
            .map_or_else(|| id.to_owned(), |number| number.to_string());
        match crate::skills::levels::note_or_hold(root, id, &name) {
            Ok(None) => {}
            Ok(Some(why)) => tracing::warn!(
                "purlis: a sandboxed chat's start was not noted for its skills ({why}); every \
                 workspace and persona folder made since waits for the person's Accept"
            ),
            Err(refused) => return Err(refused),
        }
    }
    // What the start decided about the sandbox, written before anything runs (ADR 0067 §7).
    let (trust, counted) =
        crate::sandbox::at_start(ready.unsandboxed.as_ref(), sandbox.is_some(), false, true);
    if let Some(change) = &trust {
        recorded(change, start, ready.harness, persona.as_deref(), beside)?;
    }
    // Read once `armed` has checked the start is not refused (#3461), and routed by the router
    // its persona or workspace chose (SK-15b).
    let skills = std::cell::LazyCell::new(|| {
        let built_in = kit.plugin.as_deref().and_then(crate::skills::in_bundle);
        ChatSkills::read(
            plane_skills,
            root,
            ready.harness,
            Some(&cwd),
            persona.as_deref(),
        )
        .routed_by(built_in.as_deref(), ready.harness, beside.router().as_ref())
    });
    let armed = armed(&Arming {
        root,
        kit,
        harness: ready.harness,
        cwd: Some(&cwd),
        plugins: &ready.plugins,
        sandbox,
        persona: persona.as_deref(),
        env: &ready.env,
        // The host starts no grade's chat: a grade is the window's (SK-16).
        graded: false,
        skills: &skills,
    })?;
    let (hooks, armed_env) = armed;
    // No watcher of the project's own settings runs here, so a sandboxed Claude Code chat is
    // started on the person's settings and purlis's, as the window starts one it cannot watch.
    let hooks = if sandbox.is_some() && ready.harness == Some(Harness::ClaudeCode) {
        let screened = crate::harness::claude::project::screen(Some(&cwd));
        crate::harness::claude::project::limited(hooks, &screened)?
    } else {
        hooks
    };
    let (confinement, live) = match sandbox.and_then(crate::sandbox::Applied::plan) {
        Some(plan) => {
            let (confinement, id) = confined(&plan, beside)?;
            (Some(confinement), beside.reach_of(&id))
        }
        None => (None, None),
    };
    let socket = starting
        .reporting
        .map(|reporting| PathBuf::from(&reporting.socket));
    let (program, args, wrapped) = match sandbox {
        Some(applied) => {
            let line = applied.line(
                crate::sandbox::Words {
                    program: ready.program.clone(),
                    command: ready.command.clone(),
                    armed: hooks,
                    charters: ready.args.clone(),
                },
                &crate::sandbox::At {
                    cwd: Some(&cwd),
                    hook_socket: socket.as_deref(),
                    confinement: confinement.as_ref(),
                    no_opt_out: false,
                    // purlis's own program as the start checked it (#1040).
                    purlis: applied.wrap_program(),
                },
            )?;
            (line.program, line.args, line.env)
        }
        None => (ready.program.clone(), ready.command_line(hooks), Vec::new()),
    };
    // Settings too long for one word of a command line go through a file of their own (SK-9).
    let args = match start.program.as_deref() {
        Some(id) => settings_in_a_file(kit.chat_settings.as_deref(), root, id, args)?,
        None => args,
    };
    let env = finished_env(
        ready.env.clone(),
        armed_env,
        wrapped,
        &Watched {
            cwd: Some(&cwd),
            sandboxed: sandbox.is_some(),
            id: start.program.as_deref(),
        },
        kit.binary.as_deref(),
    );
    let env = match sandbox.and_then(|applied| applied.ssh_route(confinement.as_ref())) {
        Some(route) => route.first_on_path(env),
        None => env,
    };
    // The run this start begins, written before its program can send a line (ADR 0066), and the
    // new chat counted towards this machine's opt-out rate (ADR 0067 §7, ruling V78 d).
    if let Err(why) = begun(starting, &ready, persona.as_deref(), sandbox, beside) {
        tracing::warn!("purlis: a composed chat's run was not written ({why})");
    }
    // Its routing record, under the run just begun, where its arming asked a router extension
    // (SK-15b).
    if let Some(routed) =
        std::cell::LazyCell::get(&skills).and_then(|skills| skills.routed.as_ref())
        && let Err(why) = routed_recorded(starting, &routed.record, beside)
    {
        tracing::warn!("purlis: a composed chat's routing record was not written ({why})");
    }
    if let Some(counted) = counted
        && let Err(why) = crate::sandbox::local::count(root, counted)
    {
        tracing::warn!("purlis: a composed chat was not counted ({why})");
    }
    // What its sandbox armed here, for a `secret exec` its desktop sends the values of (#2047);
    // a chat started without one leaves no record of an earlier chat under its number.
    match sandbox {
        None => crate::hostedchats::forget(root, starting.number),
        Some(applied) => crate::hostedchats::record(
            root,
            starting.number,
            crate::hostedchats::Composed {
                persona: persona.clone(),
                folder: cwd.clone(),
                harness: ready.harness,
                confines: applied.confines().clone(),
                live: live.map(crate::hostedchats::Reached),
            },
        ),
    }
    Ok(Launch {
        program,
        args,
        cwd: Some(cwd.display().to_string()),
        set: env,
        strip: declared_identity_vars(root),
        pass: crate::chatenv::read(root),
        declared: ready
            .declared
            .as_ref()
            .map(|declared| declared.env.clone())
            .unwrap_or_default(),
        harness: ready.harness.map(|harness| harness.name().to_owned()),
        git_hooks: ready
            .harness
            .and(kit.git_hooks.as_ref())
            .map(|hooks| hooks.dir().display().to_string()),
        reporting: None,
        confinement: confinement
            .as_ref()
            .and_then(crate::sandbox::Confinement::held_as)
            .map(str::to_owned),
        // Given with its token, by the host that starts it ([`spec`]).
        prompt_key: None,
    })
}

/// Why a start on a profile that hands its program a key or a cloud's credentials is not
/// composed here.
pub const NOT_HANDED: &str = "this chat's profile hands its program a key or a cloud's \
                              credentials, which purlis reads only where the window starts a \
                              chat; start it from the window";

/// `ready` with `told` as its first message, by the route its harness takes one
/// ([`crate::handoff::first_message_argv`]), last on its line, and held to what a handoff's
/// first message is held to ([`crate::handoff::bad_message`]). A harness with no such route is
/// refused here, never started without it.
pub fn told_first(
    mut ready: crate::start::Ready,
    told: Option<&str>,
) -> Result<crate::start::Ready, String> {
    let Some(told) = told else { return Ok(ready) };
    // Refused as a handoff's is: a message the harness would read as its own flag or
    // subcommand, an empty one, or one no command line can carry.
    if let Some(bad) = crate::handoff::bad_message(told) {
        return Err(bad.say());
    }
    let words = ready
        .harness
        .and_then(|harness| crate::handoff::first_message_argv(harness.name(), told))
        .ok_or_else(|| {
            "this chat's harness takes no first message on its command line, so it was not \
             started with one; start it without, then type it"
                .to_owned()
        })?;
    ready.args.extend(words);
    Ok(ready)
}

/// The persona the chat runs as: the one its environment names, which the core's start put
/// there, or the project's default for a new chat.
fn persona_of(env: &[(String, String)], root: &Path) -> Option<String> {
    env.iter()
        .find(|(name, _)| name == crate::active::PERSONA_ENV)
        .map(|(_, persona)| persona.clone())
        .or_else(|| crate::start::persona_for_a_new_chat(root))
}

/// Writes chat `starting`'s routing `record` in the host's event log (SK-15b), after its run's
/// `run.started`.
fn routed_recorded(
    starting: &Starting<'_>,
    record: &crate::skills::router::Record,
    beside: &dyn Beside,
) -> Result<(), String> {
    let entry = crate::eventlog::Entry::SkillRouted {
        plane: starting.root.to_path_buf(),
        number: starting.number,
        record: record.clone(),
    };
    serde_json::to_value(entry)
        .map_err(|why| why.to_string())
        .and_then(|entry| beside.record(entry))
}

/// **Writes the run `starting` begins** in the host's event log, as the window writes a run of
/// its own: under the start's chat and run, with what the start decided, and who started it.
/// A run that is not written is said in the host's log, as the window says one, and the chat
/// still starts.
fn begun(
    starting: &Starting<'_>,
    ready: &crate::start::Ready,
    persona: Option<&str>,
    sandbox: Option<&crate::sandbox::Applied>,
    beside: &dyn Beside,
) -> Result<(), String> {
    use crate::eventlog::{Attributes, Began, Entry, NONE};
    let start = starting.start;
    let (Some(chat), Some(run)) = (start.program.as_deref(), start.run.as_deref()) else {
        return Err("the start names no chat and run for its run to begin under".to_owned());
    };
    let none = || Some(NONE.to_owned());
    let attributes = Attributes {
        persona: persona.map(str::to_owned).or_else(none),
        harness: ready
            .harness
            .map_or_else(none, |harness| Some(harness.name().to_owned())),
        profile: start.profile.clone(),
        model_source: ready
            .sign_in
            .as_ref()
            .map(|sign_in| sign_in.kind.word().to_owned()),
        provider: None,
        model: None,
        sandbox: sandbox.map_or_else(none, |applied| applied.backend_word().map(str::to_owned)),
        conversation: ready.session.as_ref().map(ToString::to_string),
        started_by: Some(starting.started_by.to_owned()),
    };
    let cause = if start.resume.is_some() {
        Began::Reopen
    } else {
        Began::Start
    };
    let entry = Entry::Begin {
        plane: starting.root.to_path_buf(),
        number: starting.number,
        chat: chat.to_owned(),
        run: run.to_owned(),
        cause,
        attributes,
        held: false,
        trust: None,
    };
    serde_json::to_value(entry)
        .map_err(|why| why.to_string())
        .and_then(|entry| beside.record(entry))
}

/// Writes the trust event `change` under the start's own chat and run, or refuses the start
/// where it is a person's opt-out that cannot be written (ADR 0067 §7).
fn recorded(
    change: &crate::sandbox::Change,
    start: &StartProgram,
    harness: Option<Harness>,
    persona: Option<&str>,
    beside: &dyn Beside,
) -> Result<(), String> {
    use crate::sandbox::{By, Change, Lifted};
    let written = match (start.program.as_deref(), start.run.as_deref()) {
        (Some(chat), Some(run)) => serde_json::to_value(crate::eventlog::Entry::Trust {
            chat: chat.to_owned(),
            run: run.to_owned(),
            kind: change.kind().to_owned(),
            body: change.body(harness, persona),
        })
        .map_err(|why| why.to_string())
        .and_then(|entry| beside.record(entry)),
        _ => Err("the start names no chat and run to write it under".to_owned()),
    };
    match (written, change) {
        (Ok(()), _) => Ok(()),
        (
            Err(why),
            Change::Off(Lifted {
                by: By::Person | By::YourLevel(_),
                ..
            }),
        ) => Err(format!(
            "purlis could not record that this chat would run without the sandbox ({why}), so \
             it was not started. An opt-out is always recorded."
        )),
        (Err(why), _) => {
            tracing::warn!("purlis: a composed chat's sandbox was not recorded ({why})");
            Ok(())
        }
    }
}

/// The longest `--settings` JSON a chat's command line carries as it is (SK-9, #2772): under the
/// one word Linux starts a program with (`MAX_ARG_STRLEN`, 128 KiB), with room for the rest. A
/// longer one, such as a Claude Code chat's in a project with many workspaces and personas, each
/// level of which it is denied writing, goes through a file.
pub const SETTINGS_IN_A_WORD: usize = 96 * 1024;

/// The folder of the plane's state chats' settings files were kept in by earlier builds
/// (`<state>/chat-settings`): never read now, swept ([`sweep_settings`]), and held as an
/// integrity path.
pub const SETTINGS_DIR: &str = "chat-settings";

/// **`args` with a `--settings` JSON longer than [`SETTINGS_IN_A_WORD`] put in a file**, the
/// chat `id`'s in `folder`, the app data folder's [`CHAT_SETTINGS`], under the clone key of the
/// project at `root` (`<app data>/chat-settings/<clone key>/<id>.<digest>.json`, private,
/// written through folder handles with no link followed), and its path in the JSON's place:
/// Claude Code reads `--settings` as JSON or as a file. The app data folder is outside every
/// plane, so the file sits apart from what a chat works in. Its name carries the digest of what
/// purlis wrote ([`settings_digest`]), so what it is read back as is only ever what was written
/// ([`settings_from_its_file`]). Words with a shorter one, or none, are as they were. Refused for
/// an id that is no chat's, and where purlis has no data folder.
pub fn settings_in_a_file(
    folder: Option<&Path>,
    root: &Path,
    id: &str,
    mut args: Vec<String>,
) -> Result<Vec<String>, String> {
    let Some(at) = args
        .iter()
        .position(|word| word == "--settings")
        .map(|flag| flag + 1)
        .filter(|&at| at < args.len())
    else {
        return Ok(args);
    };
    if args[at].len() <= SETTINGS_IN_A_WORD || !args[at].starts_with('{') {
        return Ok(args);
    }
    if crate::reopen::a_ulid(id).is_none() {
        return Err(
            "a chat with no id of its own was not started: its settings are too long \
                    for its command line"
                .to_owned(),
        );
    }
    let Some(folder) = folder else {
        return Err(
            "a chat whose settings are too long for its command line was not started: purlis \
             has no data folder to keep them in"
                .to_owned(),
        );
    };
    std::fs::create_dir_all(folder).map_err(|why| why.to_string())?;
    crate::profiletrust::private_dir(folder).map_err(|why| why.to_string())?;
    let key = crate::plane::CloneKey::of(root);
    let rel = std::path::PathBuf::from(key.as_str()).join(format!(
        "{id}.{}.json",
        settings_digest(args[at].as_bytes())
    ));
    let was = crate::proposed::read_pinned(folder, &rel)
        .map_err(|_| "a chat's settings file is not a plain file, so it was not started")?;
    let base = was
        .as_deref()
        .and_then(|bytes| crate::proposed::base_of(Some(bytes)));
    crate::proposed::write_pinned_making(folder, &rel, base.as_deref(), &args[at], &|_, _| true)
        .map_err(|_| "a chat's settings could not be written to its file, so it was not started")?;
    args[at] = folder.join(&rel).display().to_string();
    Ok(args)
}

/// The digest a chat's settings file is named by: the first 32 hex digits of the SHA-256 of what
/// it holds.
fn settings_digest(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    sha2::Sha256::digest(bytes)[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The chat id and the digest a chat's settings file `name` is named by, where it is one:
/// `<chat id>.<digest>.json`.
fn settings_file_name(name: &str) -> Option<(String, &str)> {
    let (id, digest) = name.strip_suffix(".json")?.split_once('.')?;
    let id = crate::reopen::a_ulid(id)?;
    (digest.len() == 32 && digest.bytes().all(|b| b.is_ascii_hexdigit())).then_some((id, digest))
}

/// The parts of `path` a chat's settings file purlis writes is named by: the [`CHAT_SETTINGS`]
/// folder, the plane's clone key, and the file's name with its digest.
fn settings_file_parts(path: &Path) -> Option<(&Path, &str, &str, &str)> {
    let name = path.file_name()?.to_str()?;
    let (_, digest) = settings_file_name(name)?;
    let plane = path.parent()?;
    let key = plane.file_name()?.to_str()?;
    crate::plane::CloneKey::parse(key)?;
    let folder = plane.parent()?;
    (folder.file_name()? == CHAT_SETTINGS).then_some((folder, key, name, digest))
}

/// **Whether `path` is named as a chat's settings file purlis writes**
/// (`<app data>/chat-settings/<clone key>/<chat id>.<digest>.json`), whatever it holds now.
pub fn names_a_settings_file(path: &Path) -> bool {
    settings_file_parts(path).is_some()
}

/// **The settings in `path`, where it is a chat's settings file purlis wrote**
/// ([`settings_in_a_file`]): `<app data>/chat-settings/<clone key>/<chat id>.<digest>.json`,
/// read without following a link, and only while what it holds is what its name's digest says
/// was written. `None` for any other path, a file in the plane's state among them, and for a
/// file that holds anything else.
pub fn settings_from_its_file(path: &Path) -> Option<serde_json::Value> {
    let (folder, key, name, digest) = settings_file_parts(path)?;
    let bytes = crate::proposed::read_pinned(folder, &Path::new(key).join(name)).ok()??;
    if settings_digest(&bytes) != digest {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

/// **Removes every settings file of the project at `root` whose chat is not among `open`**,
/// older than a couple of minutes (the time between a start writing it and the chat being on
/// the app's record), in `folder`, the app data folder's [`CHAT_SETTINGS`]; and every file an
/// earlier build kept in the plane's state ([`SETTINGS_DIR`]) past that time, whatever its chat,
/// as none is read now. The app's, each time it writes that record. Only names this module makes,
/// through each folder's handle.
pub fn sweep_settings(folder: Option<&Path>, root: &Path, open: &[String]) {
    if let Some(folder) = folder {
        let key = crate::plane::CloneKey::of(root);
        sweep_in(&folder.join(key.as_str()), |name| {
            settings_file_name(name).is_some_and(|(id, _)| !open.contains(&id))
        });
    }
    sweep_in(&crate::plane::state_dir(root).join(SETTINGS_DIR), |name| {
        settings_file_name(name).is_some()
            || name
                .strip_suffix(".json")
                .and_then(crate::reopen::a_ulid)
                .is_some()
    });
}

/// Removes each file of `dir` that `goes` takes by its name, once it is older than the sweep's
/// grace, through the folder's handle with no link followed.
fn sweep_in(dir: &Path, goes: impl Fn(&str) -> bool) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if !goes(name) {
            continue;
        }
        let young = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|at| at.elapsed().ok())
            .is_some_and(|age| age < crate::skills::verbs::drafts::SWEEP_GRACE);
        if young {
            continue;
        }
        #[cfg(unix)]
        if let Ok(at) = rustix::fs::openat(
            rustix::fs::CWD,
            dir,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        ) {
            let _ = rustix::fs::unlinkat(&at, name, rustix::fs::AtFlags::empty());
        }
    }
}

/// What arms a chat's harness: the arguments its adapter adds, and the variables.
pub type Armed = (Vec<String>, Vec<(String, String)>);

/// What [`armed`] arms a chat from, as one value.
#[derive(Clone, Copy)]
pub struct Arming<'a> {
    pub root: &'a Path,
    pub kit: &'a Kit,
    pub harness: Option<Harness>,
    pub cwd: Option<&'a Path>,
    pub plugins: &'a crate::harness_plugin::Chosen,
    pub sandbox: Option<&'a crate::sandbox::Applied>,
    pub persona: Option<&'a str>,
    /// The chat's own variables, as its profile sets them: where its harness keeps its own
    /// config (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`), which a Try it's turn-offs are read from.
    pub env: &'a [(String, String)],
    /// **Whether it is a grade's chat** (SK-16): the start's own word, the record's `graded`,
    /// which decides its MCP limits. A start that says so in a folder that is no grade's arm,
    /// or one in a grade's arm that does not, is refused: the two never disagree in a chat that
    /// starts (the review of #3229).
    pub graded: bool,
    /// Where its plane skills come from, read once at its start ([`ChatSkills::read`], #3234):
    /// what its tab's Notice is worked out from too. Read the first time it is looked at, which
    /// [`armed`] does only once the checks that need nothing of it have passed (#3461): a start
    /// they refuse (a sandbox for another harness, a grade's mismatch, no binary for a sandbox)
    /// reads none of the chat's skill folders, its clone's included. The one refusal after it,
    /// a sandboxed chat whose harness cannot carry the arming, needs the arming built from it. A
    /// [`std::cell::LazyCell`] over the read is what a start passes.
    pub skills: &'a dyn std::ops::Deref<Target = ChatSkills>,
}

/// **What arms a chat's harness** for this session alone: the arguments and the variables its
/// adapter adds, as the window arms it. **Fail closed**: a sandbox compiled for another harness,
/// or one the harness cannot be armed to carry here, refuses the chat.
pub fn armed(arming: &Arming<'_>) -> Result<Armed, String> {
    let Arming {
        root,
        kit,
        harness,
        cwd,
        plugins,
        sandbox,
        persona,
        env,
        graded,
        skills,
    } = *arming;
    let not_carried = |then: &str| {
        crate::sandbox::under_policy(
            &crate::sandbox::policy::Locks::of(root),
            format!(
                "{}, and this app cannot hand the sandbox to this chat's harness, so nothing \
                 was started{then}",
                crate::sandbox::LEAD
            ),
        )
    };
    if let Some(applied) = sandbox
        && harness != Some(applied.harness())
    {
        return Err(not_carried("."));
    }
    // A grade's chat (SK-16): its MCP servers and tools held to purlis's read tools, as its
    // start says, and only where its folder is a grade's arm and its harness can be held so.
    let in_an_arm = cwd.is_some_and(|cwd| crate::skills::verbs::drafts::graded_piece(root, cwd));
    if graded != in_an_arm {
        return Err(if graded {
            "A grade's chat starts only in the folder a grade laid out for it, so nothing was \
             started."
        } else {
            "This folder is a grade's, and only a grade's chat starts in it, so nothing was \
             started."
        }
        .to_owned());
    }
    if graded && (harness != Some(Harness::ClaudeCode) || kit.binary.is_none()) {
        return Err(
            "A grade's chat starts on Claude Code alone, as only there can purlis hold it to a \
             grade's limits, so nothing was started."
                .to_owned(),
        );
    }
    let (Some(harness), Some(binary)) = (harness, kit.binary.as_deref()) else {
        return match sandbox {
            Some(_) => Err(not_carried(": purlis's own binary was not found.")),
            None => Ok((Vec::new(), Vec::new())),
        };
    };
    // Every check that needs nothing of the chat's skills has passed: they are read from here
    // on (#3461), as the arming below is built from them.
    let skills: &ChatSkills = skills;
    // Read for this very chat: its persona, unless a Try it's, which has none (#3234).
    debug_assert!(
        skills.tried || skills.persona.as_deref() == persona,
        "a chat is armed with the skills read for it"
    );
    // The plane's skills, where they reach the project's chats: while the Skill studio's flag is
    // on, and once the project requires them (`skills::reach_chats`, #2784): their
    // folder of links laid out here, by the host, as the git hooks are, so the adapter only
    // turns the set into words (ADR 0063 as amended 2026-10-20). Its scope, workspace and repo
    // skills were read once at its start (`ChatSkills::read`).
    let on = skills.on;
    let skills_root = skills.root.as_path();
    let workspace = skills.workspace.as_deref();
    // A Try it's scratch chat: every harness skill and plugin off, so the draft is tried alone.
    let alone;
    let plugins = if skills.tried {
        alone =
            crate::skills::verbs::drafts::alone_on(harness, &crate::harness_plugin::Env::of(env));
        &alone
    } else {
        plugins
    };
    let points = skills
        .routed
        .as_deref()
        .is_some_and(crate::skills::router::Routed::points);
    let at = on.then_some(crate::skills::PlaneSkills {
        plane: skills_root,
        workspace,
        repo: &skills.repo,
        routed: skills.routed.as_deref(),
    });
    let linked = laid_out_on(at, skills.persona.as_deref(), kit, harness);
    let skills = at;
    let armed = crate::harness::Kit {
        binary,
        plugin: kit.plugin.as_deref(),
        persona: persona.map(|persona| crate::harness::As { root, persona }),
        skills,
        graded,
        laid_out: linked.as_deref(),
    };
    // Which folder the chat is handed, for its terminal to note while it runs (SK-24).
    let held = linked.as_ref().map(|folder| {
        (
            crate::skills::SET_ENV.to_owned(),
            folder.display().to_string(),
        )
    });
    // Whether its prompt hook hands its prompts to the app for its router (SK-15c): only where
    // a router that reads prompts was chosen for it, and its list leaves skills out.
    let prompt_router =
        points.then(|| (crate::skills::PROMPT_ROUTER_ENV.to_owned(), "1".to_owned()));
    match harness.state_hooks(armed, cwd, plugins, sandbox) {
        StateHooks::ThisSessionOnly { args, mut env, .. } => {
            env.extend(held);
            env.extend(prompt_router);
            Ok((args, env))
        }
        // A grade's chat is never started without its limits.
        StateHooks::None if sandbox.is_some() || graded => Err(not_carried(
            ": purlis's plugin, which carries it, was not found.",
        )),
        StateHooks::None => Ok((Vec::new(), Vec::new())),
    }
}

/// **A chat's environment, finished** as every chat's is where it opens: what arms it after
/// its own, what its sandbox's wrap sets over both, what its hooks read about it, sorted, and
/// the `PATH` that finds purlis's own `binary` first.
pub fn finished_env(
    mut env: Vec<(String, String)>,
    armed: Vec<(String, String)>,
    wrapped: Vec<(String, String)>,
    watched: &Watched<'_>,
    binary: Option<&Path>,
) -> Vec<(String, String)> {
    env.extend(armed);
    // What the wrap sets wins over a profile's or the arming's value of the same name.
    env.retain(|(key, _)| !wrapped.iter().any(|(set, _)| set == key));
    env.extend(wrapped);
    what_its_hooks_read(&mut env, watched);
    env.sort();
    crate::start::with_chat_path(env, binary)
}

/// What a chat's hooks are told about it where it opens.
#[derive(Debug, Clone, Copy, Default)]
pub struct Watched<'a> {
    /// The folder it was started in.
    pub cwd: Option<&'a Path>,
    /// Whether a sandbox was applied to it.
    pub sandboxed: bool,
    /// Its own id (ADR 0066).
    pub id: Option<&'a str>,
}

/// What a chat's hooks read about it (#1338, #1345, #1457), put in its `env`: the folder it was
/// started in, whether a sandbox was applied to it, and its own id. Whatever a profile or the
/// arming said under either name is replaced.
pub fn what_its_hooks_read(env: &mut Vec<(String, String)>, watched: &Watched<'_>) {
    let ours = [
        crate::hookwire::SANDBOXED_ENV,
        crate::sandboxblock::CHAT_DIR_ENV,
        crate::hookwire::CHAT_ID_ENV,
    ];
    env.retain(|(key, _)| !ours.iter().any(|one| crate::envvar::same(one, key)));
    if let Some(cwd) = watched.cwd {
        env.push((ours[1].to_owned(), cwd.display().to_string()));
    }
    if watched.sandboxed {
        env.push((ours[0].to_owned(), "1".to_owned()));
    }
    if let Some(id) = watched.id {
        env.push((ours[2].to_owned(), id.to_owned()));
    }
}

/// Every identity variable a vault of the project at `root` declares, both halves of each
/// binding, so a chat is started without any of them (#271 review, U6). A registry that cannot
/// be read yields none: the chat still loses every `OP_*` by prefix. Skipped in a fenced build,
/// whose vault reads would leave the fixture fence.
pub fn declared_identity_vars(root: &Path) -> Vec<String> {
    if crate::fence::FENCED {
        return Vec::new();
    }
    let ctx = crate::secrets::Ctx::new(root, crate::secrets::Env::from_process());
    let Ok(doc) = crate::secrets::registry::load_registry(&ctx) else {
        return Vec::new();
    };
    let mut names: Vec<String> = crate::secrets::registry::identity_vars(&doc)
        .into_iter()
        .flat_map(|(_, vars)| vars)
        .collect();
    names.sort();
    names.dedup();
    names
}

/// The confinement `plan` plans, started by the host, as the chat is named one, and the id the
/// host holds it by.
fn confined(
    plan: &crate::sandbox::plan::Plan,
    beside: &dyn Beside,
) -> Result<(crate::sandbox::Confinement, String), String> {
    let confined = beside.confine(&plan.wired())?;
    let id = confined.confinement.clone();
    crate::sandbox::Confinement::held_elsewhere(
        crate::sandbox::Placed::of_wire(&confined),
        Box::new(HeldByThisHost(confined.confinement)),
    )
    .map(|confinement| (confinement, id))
    .map_err(|why| why.to_string())
}

/// A confinement this host holds as its program's own: it ends with the program, so dropping
/// this ends nothing.
#[derive(Debug)]
struct HeldByThisHost(String);

impl crate::sandbox::HeldElsewhere for HeldByThisHost {
    fn id(&self) -> &str {
        &self.0
    }

    fn withdraw_port(&self, port: u16) -> Result<(), String> {
        Err(format!(
            "port {port} is withdrawn from a chat this session host composed only by its start"
        ))
    }

    fn board(&self) -> Option<std::sync::Arc<dyn crate::sandbox::Board>> {
        None
    }
}

#[cfg(test)]
mod tests;
