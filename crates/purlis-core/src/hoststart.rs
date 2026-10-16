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
//! proxy asks nobody, since no window hears it. A person's opt-out is written in the host's event
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
}

impl Kit {
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
        }
    }
}

/// The folder of the app's data folder the window writes purlis's git hooks in at its launch.
pub const GIT_HOOKS: &str = "git-hooks";

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
    let ready = told_first(ready, start.first_message.as_deref())?;
    let persona = persona_of(&ready.env, root);
    let sandbox = ready.sandbox.as_ref();
    // What the start decided about the sandbox, written before anything runs (ADR 0067 §7).
    let (trust, counted) =
        crate::sandbox::at_start(ready.unsandboxed.as_ref(), sandbox.is_some(), false, true);
    if let Some(change) = &trust {
        recorded(change, start, ready.harness, persona.as_deref(), beside)?;
    }
    let armed = armed(
        root,
        kit,
        ready.harness,
        Some(&cwd),
        &ready.plugins,
        sandbox,
        persona.as_deref(),
    )?;
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
    let this_program = std::env::current_exe().ok();
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
                    // This program, `purlis serve` or the app as its session host: both answer
                    // the hidden `_sandbox` a wrapped chat's line starts with on Linux (#1040).
                    purlis: this_program.as_deref(),
                },
            )?;
            (line.program, line.args, line.env)
        }
        None => (ready.program.clone(), ready.command_line(hooks), Vec::new()),
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

/// What arms a chat's harness: the arguments its adapter adds, and the variables.
pub type Armed = (Vec<String>, Vec<(String, String)>);

/// **What arms a chat's harness** for this session alone: the arguments and the variables its
/// adapter adds, as the window arms it. **Fail closed**: a sandbox compiled for another harness,
/// or one the harness cannot be armed to carry here, refuses the chat.
pub fn armed(
    root: &Path,
    kit: &Kit,
    harness: Option<Harness>,
    cwd: Option<&Path>,
    plugins: &crate::harness_plugin::Chosen,
    sandbox: Option<&crate::sandbox::Applied>,
    persona: Option<&str>,
) -> Result<Armed, String> {
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
    let armed = crate::harness::Kit {
        binary,
        plugin: kit.plugin.as_deref(),
        persona: persona.map(|persona| crate::harness::As { root, persona }),
    };
    match harness.state_hooks(armed, cwd, plugins, sandbox) {
        StateHooks::ThisSessionOnly { args, env, .. } => Ok((args, env)),
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
