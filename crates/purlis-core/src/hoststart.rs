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

/// **The folder of links a chat in `root` is handed the plane's skills by**, laid out by the host
/// before the chat is armed, where the Skill studio's flag is `on` and `kit` says where folders
/// go ([`crate::skills::lay_out_for_chat`]), each skill as `harness` reads it. `None` otherwise,
/// and for a set with no plane skill.
pub fn plane_skills_laid_out(
    on: bool,
    root: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    kit: &Kit,
    harness: Harness,
) -> Option<PathBuf> {
    laid_out_on(on, root, workspace, persona, kit, harness, &[])
}

/// [`plane_skills_laid_out`] for a chat on `harness`, with `repo` the repo skills of the clone
/// it works in (SK-6): a plane skill a narrower repo copy hides is not laid out.
fn laid_out_on(
    on: bool,
    root: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    kit: &Kit,
    harness: Harness,
    repo: &[crate::skills::OutsideSkill],
) -> Option<PathBuf> {
    if !on {
        return None;
    }
    let sets = kit.skill_sets.as_deref()?;
    let built_in = kit.plugin.as_deref().and_then(crate::skills::in_bundle);
    crate::skills::lay_out_for_chat(
        root,
        workspace,
        persona,
        built_in.as_deref(),
        sets,
        harness,
        repo,
    )
}

/// **The skills a chat starts without** (SK-5, #2768; SK-8, #2771), where the Skill studio's
/// flag is `on`: the chat in `root` started in `cwd` on `harness` as `persona` (or on none),
/// whose environment is `env` ([`crate::skills::not_here`]). What its tab's Notice says. `None`
/// otherwise, and where it has every one.
pub fn skills_not_here(
    on: bool,
    root: &Path,
    kit: &Kit,
    harness: Option<Harness>,
    cwd: Option<&Path>,
    persona: Option<&str>,
    env: &[(String, String)],
) -> Option<crate::skills::NotHere> {
    if !on {
        return None;
    }
    let harness = harness?;
    let workspace = cwd.and_then(|cwd| crate::active::workspace_of_tree(root, cwd));
    let built_in = kit.plugin.as_deref().and_then(crate::skills::in_bundle);
    crate::skills::not_here(
        root,
        workspace.as_deref(),
        persona,
        built_in.as_deref(),
        harness,
        &crate::harness_plugin::Env::of(env),
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

/// What the host does beside a start it composes: what [`compose`] asks of it.
pub trait Beside {
    /// The confinement `plan` plans, started by the host as the program's own.
    fn confine(&self, plan: &session::ConfinePlan) -> Result<session::Confined, String>;
    /// Writes `entry` in the host's event log, durably, or says why not.
    fn record(&self, entry: serde_json::Value) -> Result<(), String>;
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
        Some(launch) => crate::chatspec::from_launch_here(launch, starting.number, size),
        None => {
            let mut launch = compose(starting, kit, beside)?;
            launch.reporting = starting.reporting.cloned();
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
            grants: crate::sandbox::grant::Grants::default(),
            made: crate::sandbox::Made::Unnamed,
        },
        root,
    )?;
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
    // What it is armed with, never where it writes, and never written by it.
    if let Some(applied) = ready.sandbox.as_mut() {
        applied
            .hold_kit(kit.armed_with(), &cwd, &ready.env)
            .map_err(|refused| refused.to_string())?;
    }
    let persona = persona_of(&ready.env, root);
    let sandbox = ready.sandbox.as_ref();
    // What the start decided about the sandbox, written before anything runs (ADR 0067 §7).
    let (trust, counted) =
        crate::sandbox::at_start(ready.unsandboxed.as_ref(), sandbox.is_some(), false, true);
    if let Some(change) = &trust {
        recorded(change, start, ready.harness, persona.as_deref(), beside)?;
    }
    // A sandboxed chat could make a workspace or persona folder while it runs: the ones there
    // now are noted as not its own (SK-9), before it is armed.
    if sandbox.is_some()
        && crate::skills::reach_chats_here(root)
        && let Some(id) = start.program.as_deref()
    {
        let name = start
            .number
            .map_or_else(|| id.to_owned(), |number| number.to_string());
        if let Err(why) = crate::skills::levels::note(root, id, &name) {
            tracing::warn!("purlis: a sandboxed chat's start was not noted for its skills ({why})");
        }
    }
    let armed = armed(&Arming {
        root,
        kit,
        harness: ready.harness,
        cwd: Some(&cwd),
        plugins: &ready.plugins,
        sandbox,
        persona: persona.as_deref(),
        env: &ready.env,
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
    let confinement = match sandbox.and_then(crate::sandbox::Applied::plan) {
        Some(plan) => Some(confined(&plan, beside)?),
        None => None,
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
        Some(id) => settings_in_a_file(root, id, args)?,
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
    if let Err(why) = begun(
        starting,
        &ready,
        persona.as_deref(),
        sandbox.is_some(),
        beside,
    ) {
        tracing::warn!("purlis: a composed chat's run was not written ({why})");
    }
    if let Some(counted) = counted
        && let Err(why) = crate::sandbox::local::count(root, counted)
    {
        tracing::warn!("purlis: a composed chat was not counted ({why})");
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

/// **Writes the run `starting` begins** in the host's event log, as the window writes a run of
/// its own: under the start's chat and run, with what the start decided, and who started it.
/// A run that is not written is said in the host's log, as the window says one, and the chat
/// still starts.
fn begun(
    starting: &Starting<'_>,
    ready: &crate::start::Ready,
    persona: Option<&str>,
    sandboxed: bool,
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
        sandbox: if sandboxed {
            crate::sandbox::Os::this()
                .backend_word(ready.harness)
                .map(str::to_owned)
        } else {
            none()
        },
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

/// The longest `--settings` JSON a chat's command line carries as it is (SK-9, #2772): well under
/// the one word Linux starts a program with (`MAX_ARG_STRLEN`, 128 KiB). A longer one, such as a
/// Claude Code chat's with every skill level it is denied writing, goes through a file.
pub const SETTINGS_IN_A_WORD: usize = 32 * 1024;

/// The folder of this machine's state a chat's settings file is kept in, by the chat's id.
pub const SETTINGS_DIR: &str = "chat-settings";

/// **`args` with a `--settings` JSON longer than [`SETTINGS_IN_A_WORD`] put in a file**, the
/// chat `id`'s in this machine's state of the project at `root` (`chat-settings/<id>.json`,
/// private, written through folder handles with no link followed), and its path in the JSON's
/// place: Claude Code reads `--settings` as JSON or as a file. Words with a shorter one, or none,
/// are as they were. Refused for an id that is no chat's.
pub fn settings_in_a_file(
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
    let state = crate::plane::state_dir(root);
    std::fs::create_dir_all(&state).map_err(|why| why.to_string())?;
    crate::profiletrust::private_dir(&state).map_err(|why| why.to_string())?;
    let rel = std::path::PathBuf::from(SETTINGS_DIR).join(format!("{id}.json"));
    let was = crate::proposed::read_pinned(&state, &rel)
        .map_err(|_| "a chat's settings file is not a plain file, so it was not started")?;
    let base = was
        .as_deref()
        .and_then(|bytes| crate::proposed::base_of(Some(bytes)));
    crate::proposed::write_pinned_making(&state, &rel, base.as_deref(), &args[at], &|_, _| true)
        .map_err(|_| "a chat's settings could not be written to its file, so it was not started")?;
    args[at] = state.join(&rel).display().to_string();
    Ok(args)
}

/// **The settings in `path`, where it is a chat's settings file purlis wrote**
/// ([`settings_in_a_file`]): `<state>/chat-settings/<chat id>.json`, read without following a
/// link. `None` for any other path.
pub fn settings_from_its_file(path: &Path) -> Option<serde_json::Value> {
    let name = path.file_name()?.to_str()?;
    crate::reopen::a_ulid(name.strip_suffix(".json")?)?;
    let dir = path.parent()?;
    if dir.file_name()? != SETTINGS_DIR {
        return None;
    }
    let bytes =
        crate::proposed::read_pinned(dir.parent()?, &Path::new(SETTINGS_DIR).join(name)).ok()??;
    serde_json::from_slice(&bytes).ok()
}

/// **Removes every chat's settings file whose chat is not among `open`**, older than a couple of
/// minutes (the time between a start writing it and the chat being on the app's record): the
/// app's, each time it writes that record. Only names this module makes, through the folder's
/// handle.
pub fn sweep_settings(root: &Path, open: &[String]) {
    let dir = crate::plane::state_dir(root).join(SETTINGS_DIR);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let Some(id) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_suffix(".json"))
            .and_then(crate::reopen::a_ulid)
        else {
            continue;
        };
        let young = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|at| at.elapsed().ok())
            .is_some_and(|age| age < crate::skills::verbs::drafts::SWEEP_GRACE);
        if young || open.contains(&id) {
            continue;
        }
        #[cfg(unix)]
        if let Ok(at) = rustix::fs::openat(
            rustix::fs::CWD,
            &dir,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        ) {
            let _ = rustix::fs::unlinkat(
                &at,
                format!("{id}.json").as_str(),
                rustix::fs::AtFlags::empty(),
            );
        }
    }
}

/// What arms a chat's harness: the arguments its adapter adds, and the variables.
pub type Armed = (Vec<String>, Vec<(String, String)>);

/// **What arms a chat's harness** for this session alone: the arguments and the variables its
/// adapter adds, as the window arms it. **Fail closed**: a sandbox compiled for another harness,
/// or one the harness cannot be armed to carry here, refuses the chat.
pub fn armed(arming: &Arming<'_>) -> Result<Armed, String> {
    armed_as(crate::skills::reach_chats_here(arming.root), arming)
}

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
}

/// [`armed`], with the Skill studio's flag `studio` as given rather than as this machine and
/// this process say it.
pub(crate) fn armed_as(studio: bool, arming: &Arming<'_>) -> Result<Armed, String> {
    let Arming {
        root,
        kit,
        harness,
        cwd,
        plugins,
        sandbox,
        persona,
        env,
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
    let (Some(harness), Some(binary)) = (harness, kit.binary.as_deref()) else {
        return match sandbox {
            Some(_) => Err(not_carried(": purlis's own binary was not found.")),
            None => Ok((Vec::new(), Vec::new())),
        };
    };
    // The plane's skills, where they reach the project's chats: while the Skill studio's flag is
    // on, and once the project requires them (`skills::reach_chats`, #2784): their
    // folder of links laid out here, by the host, as the git hooks are, so the adapter only
    // turns the set into words (ADR 0063 as amended 2026-10-20).
    let workspace = cwd.and_then(|cwd| crate::active::workspace_of_tree(root, cwd));
    let on = studio;
    // A Try it's scratch chat (SK-9, #2772): its project skills are the one skill its try laid
    // out, at a root of its own, so it gets the built-in skills and the draft alone.
    let scope = crate::skills::verbs::drafts::scope_of(root, cwd.filter(|_| on));
    let skills_root = scope.root.as_path();
    let (workspace, skills_persona) = if scope.tried {
        (None, None)
    } else {
        (workspace, persona)
    };
    // And every harness skill and plugin off, so the draft is tried alone.
    let alone;
    let plugins = if scope.tried {
        alone =
            crate::skills::verbs::drafts::alone_on(harness, &crate::harness_plugin::Env::of(env));
        &alone
    } else {
        plugins
    };
    // And the repo skills of the clone it works in, read here too, so a narrower repo copy of a
    // plane skill is the one its harness gets where purlis builds what it is handed (SK-6).
    let repo = match (on, workspace.as_deref(), cwd) {
        (true, Some(ws), Some(cwd)) => crate::skills::repo_skills_of_chat(harness, root, ws, cwd),
        _ => Vec::new(),
    };
    let linked = laid_out_on(
        on,
        skills_root,
        workspace.as_deref(),
        skills_persona,
        kit,
        harness,
        &repo,
    );
    let skills = on.then_some(crate::skills::PlaneSkills {
        plane: skills_root,
        workspace: workspace.as_deref(),
        linked: linked.as_deref(),
        repo: &repo,
    });
    let armed = crate::harness::Kit {
        binary,
        plugin: kit.plugin.as_deref(),
        persona: persona.map(|persona| crate::harness::As { root, persona }),
        skills,
    };
    // Which folder the chat is handed, for its terminal to note while it runs (SK-24).
    let held = linked.as_ref().map(|folder| {
        (
            crate::skills::SET_ENV.to_owned(),
            folder.display().to_string(),
        )
    });
    match harness.state_hooks(armed, cwd, plugins, sandbox) {
        StateHooks::ThisSessionOnly { args, mut env, .. } => {
            env.extend(held);
            Ok((args, env))
        }
        StateHooks::None if sandbox.is_some() => Err(not_carried(
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

/// The confinement `plan` plans, started by the host, as the chat is named one.
fn confined(
    plan: &crate::sandbox::plan::Plan,
    beside: &dyn Beside,
) -> Result<crate::sandbox::Confinement, String> {
    let confined = beside.confine(&plan.wired())?;
    crate::sandbox::Confinement::held_elsewhere(
        crate::sandbox::Placed::of_wire(&confined),
        Box::new(HeldByThisHost(confined.confinement)),
    )
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
