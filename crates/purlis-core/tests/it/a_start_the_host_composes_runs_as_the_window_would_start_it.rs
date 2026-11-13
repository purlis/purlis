// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A start with no launch, composed in the session host (#2171): by persona, profile,
//! workspace, resume, first message and the person's opt-out, from the project's own files and
//! this machine's profiles, through the same start the window's picker runs. What the host does
//! beside it (its confinement, its event log) is a stand-in here.

use std::cell::RefCell;
use std::fs;
use std::path::Path;

use purlis_core::hoststart::{self, Beside, Kit, Starting};
use purlis_core::profiles;
use purlis_core::sandbox::reach::{By, Decision};
use purlis_session_protocol::session::{
    self, ConfinePlan, Confined, Launch, ProgramKind, StartProgram,
};

/// A project with the persona `steward`, a workspace `alpha`, and the profile `work` of `kind`
/// running a stand-in Claude Code beside it, never in it.
struct Project {
    dir: tempfile::TempDir,
    bin: tempfile::TempDir,
}

impl Project {
    fn on(kind: &str) -> Project {
        let project = Project {
            dir: tempfile::tempdir().unwrap(),
            bin: tempfile::tempdir().unwrap(),
        };
        let root = project.root();
        fs::write(root.join("charter.toml"), "").unwrap();
        fs::create_dir_all(root.join("personas/steward")).unwrap();
        fs::write(root.join("personas/steward/persona.md"), "# steward\n").unwrap();
        fs::create_dir_all(root.join("workspaces/alpha")).unwrap();
        let program = stand_in::claude_code(project.bin.path(), "claude", "#!/bin/sh\n");
        fs::write(
            root.join(profiles::LOCAL_FILE),
            format!(
                "[harness.work]\nkind = {kind:?}\ncommand = [{:?}]\n",
                program.display().to_string()
            ),
        )
        .unwrap();
        let declared = profiles::current(root);
        purlis_core::profiletrust::record_launched(
            root,
            "work",
            &purlis_core::profiletrust::fingerprint(declared.get("work").unwrap()),
        )
        .unwrap();
        project
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    /// What purlis ships, as stand-ins: a `purlis` that does nothing, a plugin holding only
    /// opencode's file, and a folder for chats' settings files.
    fn kit(&self) -> Kit {
        let plugin = self.bin.path().join("plugin");
        let shim = purlis_core::opencode::shim_in(&plugin);
        fs::create_dir_all(shim.parent().unwrap()).unwrap();
        fs::write(&shim, "// opencode's purlis plugin, a stand-in\n").unwrap();
        Kit {
            binary: Some(stand_in::program(
                self.bin.path(),
                "purlis",
                "#!/bin/sh\nexit 0\n",
            )),
            plugin: Some(plugin),
            git_hooks: None,
            skill_sets: None,
            // Where the app keeps a chat's settings too long for its command line (SK-9): a
            // Claude Code chat's on macOS, which denies each held name under every spelling,
            // passes that length on a runner.
            chat_settings: Some(self.bin.path().join("chat-settings")),
        }
    }

    fn program(&self) -> String {
        self.bin.path().join("claude").display().to_string()
    }
}

/// What the host was asked to do beside the start.
#[derive(Default)]
struct Host {
    /// The chat's own temp folder beside its proxy.
    tmp: Option<tempfile::TempDir>,
    confined: RefCell<Vec<ConfinePlan>>,
    recorded: RefCell<Vec<serde_json::Value>>,
    refuses_to_record: bool,
    /// What the start said it armed the chat with (#4218).
    armed: RefCell<Option<Vec<session::ArmedSkill>>>,
    /// What the start said the chat started without here (#2977).
    not_there: RefCell<Option<session::SkillsNotThere>>,
}

impl Beside for Host {
    fn confine(&self, plan: &ConfinePlan) -> Result<Confined, String> {
        self.confined.borrow_mut().push(plan.clone());
        Ok(Confined {
            confinement: "01J9CNFNMNT0000000000000000".into(),
            http: 50811,
            socks: 50812,
            tmp: self
                .tmp
                .as_ref()
                .map(|tmp| tmp.path().display().to_string()),
            routed: None,
            ssh: None,
            left_out: Vec::new(),
            host_name: None,
        })
    }

    fn record(&self, entry: serde_json::Value) -> Result<(), String> {
        if self.refuses_to_record {
            return Err("the event log is not open".into());
        }
        self.recorded.borrow_mut().push(entry);
        Ok(())
    }

    fn armed(&self, skills: Vec<session::ArmedSkill>) {
        *self.armed.borrow_mut() = Some(skills);
    }

    fn not_there(&self, not: session::SkillsNotThere) {
        *self.not_there.borrow_mut() = Some(not);
    }
}

/// **Whether purlis wraps a sandboxed Claude Code chat here** (Linux): there it runs inside
/// purlis's wrap, on a sandbox-only profile, which the project's `work` is not (ADR 0067 as
/// amended 2026-11-10).
fn purlis_wraps_claude_code() -> bool {
    purlis_core::sandbox::Machine::this().purlis_wraps(purlis_core::harness::Harness::ClaudeCode)
}

/// For a test of something else: out of the sandbox where purlis wraps a Claude Code chat
/// ([`purlis_wraps_claude_code`]). Elsewhere it starts sandboxed as it did.
fn out_of_the_sandbox_where_purlis_wraps_claude_code(start: &mut StartProgram) {
    start.without_sandbox = purlis_wraps_claude_code();
}

fn asking(change: impl FnOnce(&mut StartProgram)) -> StartProgram {
    let mut start = StartProgram {
        program: Some("01J9CHAT000000000000000000".into()),
        run: Some("01J9RUN0000000000000000000".into()),
        persona: Some("steward".into()),
        profile: Some("work".into()),
        workspace: Some("alpha".into()),
        ..StartProgram::of("01J9PRJCT00000000000000000", ProgramKind::Harness)
    };
    change(&mut start);
    start
}

fn composed(project: &Project, start: &StartProgram, host: &Host) -> Result<Launch, String> {
    hoststart::compose(
        &Starting {
            root: project.root(),
            number: 3,
            start,
            reporting: Some(&session::Reporting {
                socket: "/the/project/hooks.sock".into(),
                token: "token-3".into(),
            }),
            started_by: "terminal",
            prompt_key: None,
        },
        &project.kit(),
        host,
    )
}

fn set(launch: &Launch, name: &str) -> Option<String> {
    launch
        .set
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.clone())
}

/// The words the chat's program is started with, wherever its sandbox puts them on its line.
fn words(launch: &Launch) -> Vec<String> {
    std::iter::once(launch.program.clone())
        .chain(launch.args.iter().cloned())
        .collect()
}

#[test]
fn a_start_with_no_launch_runs_its_profiles_harness_in_its_workspace_as_its_persona() {
    purlis_core::unsteered!();
    let project = Project::on("claude");
    let host = Host::default();

    let launch = composed(
        &project,
        &asking(out_of_the_sandbox_where_purlis_wraps_claude_code),
        &host,
    )
    .expect("composed");

    let bin = fs::canonicalize(project.bin.path()).unwrap();
    assert!(
        words(&launch)
            .iter()
            .any(|word| { word == &project.program() || Path::new(word) == bin.join("claude") }),
        "{:?}",
        words(&launch)
    );
    let alpha = fs::canonicalize(project.root().join("workspaces/alpha")).unwrap();
    assert_eq!(
        launch
            .cwd
            .as_deref()
            .map(|cwd| fs::canonicalize(cwd).unwrap()),
        Some(alpha)
    );
    assert_eq!(launch.harness.as_deref(), Some("claude"));
    assert_eq!(set(&launch, "PURLIS_PERSONA").as_deref(), Some("steward"));
    assert_eq!(
        set(&launch, purlis_core::hookwire::CHAT_ID_ENV).as_deref(),
        Some("01J9CHAT000000000000000000")
    );
    assert_eq!(launch.reporting, None, "the host says where it reports");
}

/// #3369: the prompt key the host minted for a start it composes is in the chat's environment,
/// for its hooks, and nowhere on its line.
#[test]
fn a_start_the_host_composes_carries_the_prompt_key_it_was_given_in_its_environment_alone() {
    purlis_core::unsteered!();
    let project = Project::on("claude");
    let host = Host::default();
    let start = asking(out_of_the_sandbox_where_purlis_wraps_claude_code);
    let key = session::PromptKey::mint().expect("a key");

    let spec = hoststart::spec(
        &Starting {
            root: project.root(),
            number: 3,
            start: &start,
            reporting: None,
            started_by: "terminal",
            prompt_key: Some(&key),
        },
        purlis_core::engine::Size::MIN,
        &project.kit(),
        &host,
    )
    .expect("composed");

    let in_env: Vec<_> = spec
        .env
        .iter()
        .filter(|(name, _)| *name == purlis_core::hookwire::PROMPT_KEY_ENV)
        .map(|(_, value)| value.clone())
        .collect();
    assert_eq!(in_env, vec![std::ffi::OsString::from(key.expose())]);
    assert!(
        !spec
            .args
            .iter()
            .any(|word| word.to_string_lossy().contains(key.expose())),
        "the key is on the line"
    );
}

#[test]
fn a_profile_this_machine_does_not_declare_is_refused_and_nothing_is_started_beside_it() {
    purlis_core::unsteered!();
    let project = Project::on("claude");
    let host = Host::default();

    let refused = composed(
        &project,
        &asking(|start| start.profile = Some("someone-elses".into())),
        &host,
    )
    .unwrap_err();

    assert!(refused.contains("someone-elses"), "{refused}");
    assert!(host.confined.borrow().is_empty());
    assert!(host.recorded.borrow().is_empty());
}

#[test]
fn a_start_that_names_no_profile_is_refused() {
    purlis_core::unsteered!();
    let project = Project::on("claude");

    let refused = composed(
        &project,
        &asking(|start| start.profile = None),
        &Host::default(),
    )
    .unwrap_err();

    assert!(refused.contains("profile"), "{refused}");
}

#[test]
fn a_persona_the_project_does_not_have_is_refused() {
    purlis_core::unsteered!();
    let project = Project::on("claude");

    let refused = composed(
        &project,
        &asking(|start| start.persona = Some("intruder".into())),
        &Host::default(),
    )
    .unwrap_err();

    assert!(refused.contains("intruder"), "{refused}");
}

#[test]
fn a_workspace_that_is_not_a_folder_of_the_project_is_refused() {
    purlis_core::unsteered!();
    let project = Project::on("claude");
    let outside = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), project.root().join("workspaces/linked")).unwrap();

    for workspace in ["../..", "linked", "missing"] {
        let refused = composed(
            &project,
            &asking(|start| start.workspace = Some(workspace.into())),
            &Host::default(),
        );

        assert!(refused.is_err(), "{workspace} was composed: {refused:?}");
    }
}

#[test]
fn a_shell_is_never_composed_here() {
    purlis_core::unsteered!();
    let project = Project::on("claude");

    let refused = composed(
        &project,
        &asking(|start| start.kind = ProgramKind::Shell),
        &Host::default(),
    )
    .unwrap_err();

    assert!(refused.contains("shell"), "{refused}");
}

#[test]
fn a_resumed_start_carries_on_its_conversation_and_a_first_message_ends_its_line() {
    purlis_core::unsteered!();
    let project = Project::on("claude");
    let conversation = "0199a2f4-1c2b-7f00-8000-000000000007";

    let launch = composed(
        &project,
        &asking(|start| {
            start.resume = Some(conversation.into());
            start.first_message = Some("carry on with the plan".into());
            // Out of the sandbox, so its line is the profile's own.
            start.without_sandbox = true;
        }),
        &Host::default(),
    )
    .expect("composed");

    let words = words(&launch);
    assert!(
        words.iter().any(|word| word.contains(conversation)),
        "{words:?}"
    );
    assert_eq!(
        words.last().map(String::as_str),
        Some("carry on with the plan")
    );
}

#[test]
fn a_first_message_the_harness_would_read_as_its_own_flag_is_refused() {
    purlis_core::unsteered!();
    let project = Project::on("claude");

    for told in ["--dangerously-skip-permissions", "login", ""] {
        let refused = composed(
            &project,
            // Out of the sandbox, so nothing but the message decides it.
            &asking(|start| {
                start.first_message = Some(told.into());
                start.without_sandbox = true;
            }),
            &Host::default(),
        );

        assert!(refused.is_err(), "{told:?} was composed: {refused:?}");
    }
}

#[test]
fn a_conversation_that_is_not_one_is_refused() {
    purlis_core::unsteered!();
    let project = Project::on("claude");

    let refused = composed(
        &project,
        &asking(|start| start.resume = Some("--dangerously-skip-permissions".into())),
        &Host::default(),
    );

    assert!(refused.is_err(), "{refused:?}");
}

#[test]
fn an_opt_out_is_written_under_its_run_before_it_is_composed() {
    purlis_core::unsteered!();
    let project = Project::on("claude");
    let host = Host::default();

    let launch = composed(
        &project,
        &asking(|start| start.without_sandbox = true),
        &host,
    )
    .expect("composed");

    assert_eq!(launch.confinement, None, "it runs without the sandbox");
    assert!(host.confined.borrow().is_empty());
    let recorded = host.recorded.borrow();
    assert_eq!(
        recorded.len(),
        2,
        "its trust event, then its run: {recorded:?}"
    );
    assert_eq!(recorded[0]["entry"], "trust");
    assert_eq!(recorded[0]["kind"], "trust.sandbox.off");
    assert_eq!(recorded[0]["chat"], "01J9CHAT000000000000000000");
    assert_eq!(recorded[0]["run"], "01J9RUN0000000000000000000");
}

#[test]
fn an_opt_out_that_cannot_be_written_is_not_started() {
    purlis_core::unsteered!();
    let project = Project::on("claude");
    let host = Host {
        refuses_to_record: true,
        ..Host::default()
    };

    let refused = composed(
        &project,
        &asking(|start| start.without_sandbox = true),
        &host,
    )
    .unwrap_err();

    assert!(refused.contains("always recorded"), "{refused}");
}

#[test]
fn an_opt_out_with_no_run_to_write_it_under_is_not_started() {
    purlis_core::unsteered!();
    let project = Project::on("claude");

    let refused = composed(
        &project,
        &asking(|start| {
            start.without_sandbox = true;
            start.run = None;
        }),
        &Host::default(),
    )
    .unwrap_err();

    assert!(refused.contains("always recorded"), "{refused}");
}

#[test]
fn a_composed_starts_run_begins_under_its_ids_and_says_who_started_it() {
    purlis_core::unsteered!();
    let project = Project::on("claude");
    let host = Host::default();

    composed(
        &project,
        // Out of the sandbox, so nothing but the start decides it.
        &asking(|start| start.without_sandbox = true),
        &host,
    )
    .expect("composed");

    let recorded = host.recorded.borrow();
    let begun = recorded
        .iter()
        .find(|entry| entry["entry"] == "begin")
        .unwrap_or_else(|| panic!("no run began: {recorded:?}"));
    assert_eq!(begun["chat"], "01J9CHAT000000000000000000");
    assert_eq!(begun["run"], "01J9RUN0000000000000000000");
    assert_eq!(begun["number"], 3);
    assert_eq!(begun["cause"], "start");
    assert_eq!(begun["attributes"]["started_by"], "terminal");
    assert_eq!(begun["attributes"]["persona"], "steward");
    assert_eq!(begun["attributes"]["profile"], "work");
    // R-227: what the host resolved, the person's opt-out for this chat only, and the commit of a
    // `charter.toml` no commit has.
    assert_eq!(
        begun["attributes"]["protection"],
        serde_json::json!({
            "layers": [{"layer": "sandbox", "level": "off", "tier": "chat"}],
            "charter_commit": "none",
            "charter_dirty": false,
            "answers_in_force": [],
        })
    );
}

/// An opencode chat's sandbox, compiled for a Mac whatever this machine is: what a composed start
/// is given to wrap, with no program run to get there.
fn a_sandboxed_opencode_start(project: &Project) -> purlis_core::start::Ready {
    a_sandboxed_start_on_a_mac(project, purlis_core::harness::Harness::Opencode)
}

/// A `harness` chat's sandbox, compiled for a Mac whatever this machine is, as
/// [`a_sandboxed_opencode_start`] compiles opencode's.
fn a_sandboxed_start_on_a_mac(
    project: &Project,
    harness: purlis_core::harness::Harness,
) -> purlis_core::start::Ready {
    let applied = purlis_core::sandbox::for_start(
        harness,
        project.root(),
        &purlis_core::sandbox::Machine {
            env: purlis_core::secrets::Env::of(&[]),
            home: None,
            os: purlis_core::sandbox::Os::MacOs,
        },
        &|_| true,
    )
    .expect("it compiles")
    .expect("a sandbox at the floor");
    purlis_core::start::Ready {
        program: project.program(),
        command: Vec::new(),
        args: Vec::new(),
        env: vec![("PURLIS_PERSONA".to_owned(), "steward".to_owned())],
        cwd: Some(project.root().to_path_buf()),
        harness: Some(harness),
        declared: None,
        session: None,
        how: purlis_core::reopen::Reopened::Fresh(
            purlis_core::reopen::Fresh::NoResumeForThisProgram,
        ),
        plugins: Default::default(),
        sandbox: Some(applied),
        unsandboxed: None,
        floorless: None,
        notices: Vec::new(),
        agents_md: Vec::new(),
        sign_in: None,
        first_message: None,
    }
}

fn composed_from(
    project: &Project,
    ready: purlis_core::start::Ready,
    host: &dyn Beside,
) -> Result<Launch, String> {
    composed_with(project, ready, &project.kit(), host)
}

/// On every machine: a sandboxed start the host composes runs in the confinement the host
/// started for it, named in its launch, and is wrapped in its sandbox.
#[test]
fn a_sandboxed_composed_start_runs_in_the_confinement_the_host_started_for_it() {
    purlis_core::unsteered!();
    let project = Project::on("codex");
    let host = Host {
        tmp: Some(tempfile::tempdir().unwrap()),
        ..Host::default()
    };

    let launch =
        composed_from(&project, a_sandboxed_opencode_start(&project), &host).expect("composed");

    assert_eq!(host.confined.borrow().len(), 1, "one confinement");
    assert_eq!(
        launch.confinement.as_deref(),
        Some("01J9CNFNMNT0000000000000000")
    );
    assert_ne!(
        launch.program,
        project.program(),
        "it runs inside its sandbox"
    );
    assert_eq!(
        set(&launch, purlis_core::hookwire::SANDBOXED_ENV).as_deref(),
        Some("1")
    );
}

/// R-227, Q238: a start the host composes reads no level of the person's, so its start event
/// never says the person set it, even where their confirmed level is the one it runs at: the
/// default floor is purlis's.
#[test]
fn a_composed_start_records_its_sandbox_as_purlis_s_floor_never_the_person_s() {
    purlis_core::unsteered!();
    let project = Project::on("codex");
    // Your floor, as Settings writes it and confirms it.
    let local = project.root().join(profiles::LOCAL_FILE);
    let declared = fs::read_to_string(&local).unwrap();
    fs::write(&local, format!("{declared}[sandbox]\nlevel = \"floor\"\n")).unwrap();
    purlis_core::sandbox::personal::set_by_you(
        project.root(),
        None,
        Some(purlis_core::sandbox::personal::Choice::Floor),
    )
    .expect("your floor, confirmed");
    let host = Host {
        tmp: Some(tempfile::tempdir().unwrap()),
        ..Host::default()
    };

    composed_from(&project, a_sandboxed_opencode_start(&project), &host).expect("composed");

    let recorded = host.recorded.borrow();
    let begun = recorded
        .iter()
        .find(|entry| entry["entry"] == "begin")
        .unwrap_or_else(|| panic!("no run began: {recorded:?}"));
    assert_eq!(
        begun["attributes"]["protection"]["layers"],
        serde_json::json!([{"layer": "sandbox", "level": "floor", "tier": "purlis"}])
    );
}

/// The id [`Host`] gives the confinement it starts.
const ITS_CONFINEMENT: &str = "01J9CNFNMNT0000000000000000";

/// A host whose confinement for the chat allows live what `now` holds, read as it is asked.
struct LiveHost {
    host: Host,
    now: std::sync::Arc<std::sync::Mutex<session::ReachNow>>,
    /// Each confinement it was asked to read.
    read: RefCell<Vec<String>>,
}

impl Beside for LiveHost {
    fn confine(&self, plan: &ConfinePlan) -> Result<Confined, String> {
        self.host.confine(plan)
    }

    fn record(&self, entry: serde_json::Value) -> Result<(), String> {
        self.host.record(entry)
    }

    fn reach_of(&self, confinement: &str) -> Option<purlis_core::hostedchats::Live> {
        self.read.borrow_mut().push(confinement.to_owned());
        let now = std::sync::Arc::clone(&self.now);
        Some(std::sync::Arc::new(move || now.lock().unwrap().clone()))
    }
}

/// What a run of chat 3's tool here decides a connection to `host` on 443 by.
fn its_tool_decides(project: &Project, host: &str) -> Decision {
    purlis_core::hostedchats::asker(project.root(), 3)
        .expect("the host recorded the chat it composed")
        .confines
        .expect("held to its sandbox")
        .decides()
        .decide(host, 443, &[443], &[])
}

/// #3407: a sandboxed chat the host composed runs its tool held to its confinement as it is now:
/// a host the person allowed for the chat since it started is reached by the tool's next run as
/// the chat's Allow, and one taken back again is not. The floor carries every host as Open, so
/// what the Allow changes is the scope the run decides it by.
#[test]
fn a_composed_chats_tool_run_reaches_the_hosts_its_confinement_allows_live() {
    purlis_core::unsteered!();
    let project = Project::on("codex");
    let host = LiveHost {
        host: Host {
            tmp: Some(tempfile::tempdir().unwrap()),
            ..Host::default()
        },
        now: std::sync::Arc::default(),
        read: RefCell::default(),
    };

    composed_from(&project, a_sandboxed_opencode_start(&project), &host).expect("composed");

    assert_eq!(
        *host.read.borrow(),
        [ITS_CONFINEMENT],
        "its own confinement"
    );
    let host_name = "pypi.example.com";
    assert_eq!(its_tool_decides(&project, host_name), Decision::Open);
    host.now.lock().unwrap().allowed_live = vec![session::Listed {
        host: host_name.into(),
        layer: session::Layer::Chat,
    }];
    assert_eq!(
        its_tool_decides(&project, host_name),
        Decision::Allowed(By::Chat)
    );
    host.now.lock().unwrap().allowed_live.clear();
    assert_eq!(its_tool_decides(&project, host_name), Decision::Open);
}

/// What a host that could not start a chat's confinement does.
struct NoConfinement;

impl Beside for NoConfinement {
    fn confine(&self, _plan: &ConfinePlan) -> Result<Confined, String> {
        Err("its confinement was not started".into())
    }

    fn record(&self, _entry: serde_json::Value) -> Result<(), String> {
        Ok(())
    }
}

/// On every machine: a sandboxed start whose confinement the host could not start is not
/// composed at all, never composed to run without its proxy.
#[test]
fn a_sandboxed_composed_start_without_its_confinement_is_not_started() {
    purlis_core::unsteered!();
    let project = Project::on("codex");

    let refused = composed_from(
        &project,
        a_sandboxed_opencode_start(&project),
        &NoConfinement,
    )
    .unwrap_err();

    assert!(
        refused.contains("its confinement was not started"),
        "{refused}"
    );
}

/// Where this machine sandboxes a chat, the host starts its confinement, as the program's own,
/// with the floor's reach, and the chat is wrapped in it.
#[cfg(target_os = "macos")]
#[test]
fn a_sandboxed_start_runs_in_the_confinement_the_host_started_for_it() {
    purlis_core::unsteered!();
    let project = Project::on("claude");
    let host = Host::default();

    let launch = composed(&project, &asking(|_| {}), &host).expect("composed");

    assert_eq!(host.confined.borrow().len(), 1, "one confinement");
    // Its reach: a project that has not turned the sandbox on runs at the floor (#2081), whose
    // proxy carries every host and lists none.
    assert!(host.confined.borrow()[0].anything, "its reach");
    assert!(
        host.confined.borrow()[0].listed.is_empty(),
        "the floor lists no host"
    );
    assert_eq!(
        launch.confinement.as_deref(),
        Some("01J9CNFNMNT0000000000000000")
    );
    assert_eq!(
        set(&launch, purlis_core::hookwire::SANDBOXED_ENV).as_deref(),
        Some("1")
    );
}

/// On every machine: at the floor, the plugin and the git hooks a composed chat is armed with
/// are never written by it: its sandbox, on its line, holds both.
#[test]
fn a_composed_chat_at_the_floor_never_writes_the_plugin_or_git_hooks_it_is_armed_with() {
    purlis_core::unsteered!();
    let project = Project::on("codex");
    let data = tempfile::tempdir().unwrap();
    let git_hooks = data.path().canonicalize().unwrap().join("git-hooks");
    fs::create_dir_all(&git_hooks).unwrap();
    let kit = Kit {
        git_hooks: Some(purlis_core::githooks::GitHooks::at(&git_hooks)),
        ..project.kit()
    };
    let plugin = kit.plugin.clone().unwrap().canonicalize().unwrap();
    let host = Host {
        tmp: Some(tempfile::tempdir().unwrap()),
        ..Host::default()
    };

    let launch = composed_with(&project, a_sandboxed_opencode_start(&project), &kit, &host)
        .expect("composed");

    let line = words(&launch).join(" ");
    for folder in [&plugin, &git_hooks] {
        let rule = format!(
            "(deny file-write* file-link (subpath \"{}\"))",
            folder.display()
        );
        assert!(line.contains(&rule), "{rule} not in {line}");
    }
}

/// On every machine: a composed chat whose plugin lies where it would write is refused, and
/// the refusal says what to do. Nothing is started beside it.
#[test]
fn a_composed_chat_whose_plugin_it_could_write_is_refused() {
    purlis_core::unsteered!();
    let project = Project::on("codex");
    let plugin = project.root().join("tools/plugin");
    fs::create_dir_all(&plugin).unwrap();
    let kit = Kit {
        plugin: Some(plugin),
        ..project.kit()
    };
    let host = Host::default();

    let refused =
        composed_with(&project, a_sandboxed_opencode_start(&project), &kit, &host).unwrap_err();

    assert!(
        refused.contains("Install purlis outside the project and outside what a chat may write"),
        "{refused}"
    );
    assert!(
        host.confined.borrow().is_empty(),
        "nothing started beside it"
    );
}

fn composed_with(
    project: &Project,
    ready: purlis_core::start::Ready,
    kit: &Kit,
    host: &dyn Beside,
) -> Result<Launch, String> {
    let start = asking(|_| {});
    hoststart::compose_ready(
        &Starting {
            root: project.root(),
            number: 3,
            start: &start,
            reporting: None,
            started_by: "terminal",
            prompt_key: None,
        },
        ready,
        kit,
        host,
    )
}

/// #2636: the line a sandboxed Claude Code start is composed with is read as Claude Code's own
/// sandbox, by the line alone, with no confinement counted.
#[test]
fn a_composed_claude_code_line_is_read_as_running_in_its_own_sandbox() {
    purlis_core::unsteered!();
    // Claude Code's own sandbox runs a sandboxed Claude Code chat on macOS. On Linux every one
    // runs inside purlis's wrap, on a sandbox-only profile ([`purlis_wraps_claude_code`]).
    if purlis_wraps_claude_code() {
        return;
    }
    let project = Project::on("claude");
    let host = Host::default();

    let launch = composed(&project, &asking(|_| {}), &host).expect("composed");

    let os = purlis_core::sandbox::Os::this();
    let args: Vec<std::ffi::OsString> = launch.args.iter().map(Into::into).collect();
    assert_eq!(
        purlis_core::sandbox::ran::applied_to(launch.program.as_ref(), &args, false, os),
        session::RunSandbox::By(os.backend_word().expect("a backend here").to_owned()),
        "{:?}",
        words(&launch)
    );
}

/// A skill folder `name` in `dir`, as a plane holds one.
fn skill_in(dir: &Path, name: &str) {
    fs::create_dir_all(dir.join(name)).unwrap();
    fs::write(
        dir.join(name).join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Does {name}.\n---\n\nbody\n"),
    )
    .unwrap();
}

/// **SK-15h, #4218: a chat the host composes tells its host the skills it armed it with**, by
/// name and level alone, and only where its desktop's word lets the plane's skills reach it.
#[test]
fn a_composed_chat_tells_its_host_the_names_and_levels_of_the_skills_it_armed_it_with() {
    purlis_core::unsteered!();
    let project = Project::on("claude");
    skill_in(&project.root().join("skills"), "deploy");
    let armed = |on: bool| {
        let host = Host::default();
        let start = asking(|start| {
            start.plane_skills = Some(on);
            start.without_sandbox = true;
            start.workspace = None;
        });
        hoststart::compose(
            &Starting {
                root: project.root(),
                number: 3,
                start: &start,
                reporting: None,
                started_by: "remote-link",
                prompt_key: None,
            },
            &project.kit(),
            &host,
        )
        .expect("composed");
        host.armed.take().expect("its start said what it armed")
    };

    assert_eq!(
        armed(true),
        [session::ArmedSkill {
            name: "deploy".into(),
            level: "project".into(),
        }]
    );
    assert_eq!(armed(false), [], "the plane's skills do not reach it");
}

/// **SK-5d, #2977: a chat the host composes tells its host the skills its persona names that
/// the host's machine does not have**, by the same reading of its harness's skills a chat on the
/// desktop gets (`skills_at`), and only where its desktop's word lets the plane's skills reach
/// it. A persona whose skills are all there says nothing.
#[test]
fn a_composed_chat_tells_its_host_the_skills_its_persona_names_that_are_not_there() {
    purlis_core::unsteered!();
    let project = Project::on("claude");
    let not_there = |on: bool| {
        let host = Host::default();
        let start = asking(|start| {
            start.plane_skills = Some(on);
            start.without_sandbox = true;
            start.workspace = None;
        });
        hoststart::compose(
            &Starting {
                root: project.root(),
                number: 3,
                start: &start,
                reporting: None,
                started_by: "remote-link",
                prompt_key: None,
            },
            &project.kit(),
            &host,
        )
        .expect("composed, without what is not there");
        host.not_there.take()
    };
    assert_eq!(not_there(true), None, "its persona names no skill");

    fs::write(
        project.root().join("personas/steward/persona.md"),
        "---\nrole: Ops\nskills: nowhere-plugin:never-installed\n---\n# steward\n",
    )
    .unwrap();

    assert_eq!(
        not_there(true),
        Some(session::SkillsNotThere {
            missing: vec!["nowhere-plugin:never-installed".into()],
            ..Default::default()
        })
    );
    assert_eq!(not_there(false), None, "the plane's skills do not reach it");
}

/// **#6078: a composed chat tells its host every skill it starts without there, as a chat on
/// this machine says them**: another workspace's skill its persona names that the project does
/// not have (SK-5b), and a plane skill that could not be loaded there, with its level and why
/// (SK-8), besides the installed ones it lacks.
#[test]
fn a_composed_chat_tells_its_host_the_skills_that_could_not_be_loaded_and_another_workspaces_it_lacks()
 {
    purlis_core::unsteered!();
    let project = Project::on("claude");
    fs::write(
        project.root().join("personas/steward/persona.md"),
        "---\nrole: Ops\nskills: workspaces/ghost:x\n---\n# steward\n",
    )
    .unwrap();
    let half_made = project.root().join("skills/half-made");
    fs::create_dir_all(&half_made).unwrap();
    fs::write(half_made.join("SKILL.md"), "no frontmatter here\n").unwrap();
    let host = Host::default();
    let start = asking(|start| {
        start.plane_skills = Some(true);
        start.without_sandbox = true;
        start.workspace = None;
    });
    hoststart::compose(
        &Starting {
            root: project.root(),
            number: 3,
            start: &start,
            reporting: None,
            started_by: "remote-link",
            prompt_key: None,
        },
        &project.kit(),
        &host,
    )
    .expect("composed, without what is not there");

    let said = host.not_there.take().expect("it says what it lacks");
    assert_eq!(said.no_workspace, ["workspaces/ghost:x"]);
    assert!(said.not_in_its_workspace.is_empty(), "{said:?}");
    let [broken] = said.broken.as_slice() else {
        panic!("one skill could not be loaded: {said:?}");
    };
    assert_eq!(
        (broken.name.as_str(), broken.level.as_str(), broken.local),
        ("half-made", "project", false)
    );
    assert!(!broken.why.trim().is_empty(), "it says why");
}

/// The listing limit a composed Claude Code chat is handed in its `--settings`, read from the
/// file those settings go through where they are too long for a word of its line.
fn listing_limit_of(launch: &Launch) -> usize {
    let args = words(launch);
    let at = args
        .iter()
        .position(|word| word == "--settings")
        .expect("Claude Code is handed its settings");
    let word = &args[at + 1];
    let text = if word.starts_with('{') {
        word.clone()
    } else {
        fs::read_to_string(word).expect("its settings file")
    };
    let settings: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    settings["env"]["SLASH_COMMAND_TOOL_CHAR_BUDGET"]
        .as_str()
        .expect("a listing limit")
        .parse()
        .expect("a number")
}

/// **#5842: a chat the plane's skills do not reach still gets a limit that holds its own
/// catalogue.** The person's own skills share Claude Code's listing with purlis's built-in ones
/// whether or not the plane's are handed, so the limit counts them either way.
#[test]
fn a_chat_the_planes_skills_do_not_reach_is_handed_a_limit_that_holds_the_persons_own_skills() {
    purlis_core::unsteered!();
    let project = Project::on("claude");
    // The person's own Claude Code skills, in the config folder the profile points it at: 30
    // descriptions, each past what Claude Code lists of one, so past its 40,000 floor together.
    let config = tempfile::tempdir().unwrap();
    for at in 0..30 {
        let folder = config.path().join("skills").join(format!("mine-{at:02}"));
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("SKILL.md"),
            format!(
                "---\nname: mine-{at:02}\ndescription: {}\n---\n\nbody\n",
                "d".repeat(2_000)
            ),
        )
        .unwrap();
    }
    let root = project.root();
    fs::write(
        root.join(profiles::LOCAL_FILE),
        format!(
            "[harness.work]\nkind = \"claude\"\ncommand = [{:?}]\nenv = {{ CLAUDE_CONFIG_DIR = {:?} }}\n",
            project.program(),
            config.path().display().to_string()
        ),
    )
    .unwrap();
    let declared = profiles::current(root);
    purlis_core::profiletrust::record_launched(
        root,
        "work",
        &purlis_core::profiletrust::fingerprint(declared.get("work").unwrap()),
    )
    .unwrap();
    let limit = |on: bool| {
        let start = asking(|start| {
            start.plane_skills = Some(on);
            start.without_sandbox = true;
        });
        listing_limit_of(&composed(&project, &start, &Host::default()).expect("composed"))
    };

    // Each description as Claude Code lists it, cut at 1,536, with its name and its line.
    let own = 30 * (1_536 + "mine-00".len() + 23);
    let off = limit(false);
    assert!(
        off > own + 6_848,
        "the limit {off} holds the person's {own} and Claude Code's own entries"
    );
    assert_eq!(
        off,
        limit(true),
        "the plane holds no skills, so reaching it changes nothing"
    );
}

/// **#3097 (SK-41): a sandboxed Claude Code start notes the levels there as it is composed**, so
/// a workspace made while it runs hands no chat its skills, on every system, whatever its own
/// sandbox makes of the pattern that holds a later level (ADR 0099 as amended 2026-11-09). A
/// workspace there at its start still reaches chats.
#[test]
fn a_sandboxed_claude_code_start_leaves_a_workspace_made_while_it_runs_out_of_every_chat() {
    purlis_core::unsteered!();
    let project = Project::on("claude");
    let root = project.root();
    // The person has accepted the folders there (the Skill studio's Needs you), as on any
    // project purlis kept state for before.
    if let Some((_, folders)) = purlis_core::skills::levels::to_confirm(root) {
        purlis_core::skills::levels::confirm(root, &folders).expect("accepted");
    }
    let start = asking(|start| {
        start.number = Some(3);
        start.plane_skills = Some(true);
    });

    // Its desktop's word on the plane's skills is read from a link's start (#3084).
    let starting = Starting {
        root,
        number: 3,
        start: &start,
        reporting: None,
        started_by: "remote-link",
        prompt_key: None,
    };
    // Where purlis wraps a sandboxed Claude Code chat, it runs only on a sandbox-only profile
    // ([`purlis_wraps_claude_code`]), so the start composed there is one compiled for a Mac.
    let (launch, os) = if purlis_wraps_claude_code() {
        let ready = a_sandboxed_start_on_a_mac(&project, purlis_core::harness::Harness::ClaudeCode);
        (
            hoststart::compose_ready(&starting, ready, &project.kit(), &Host::default()),
            purlis_core::sandbox::Os::MacOs,
        )
    } else {
        (
            hoststart::compose(&starting, &project.kit(), &Host::default()),
            purlis_core::sandbox::Os::this(),
        )
    };
    let launch = launch.expect("composed");

    let args: Vec<std::ffi::OsString> = launch.args.iter().map(Into::into).collect();
    assert!(
        matches!(
            purlis_core::sandbox::ran::applied_to(launch.program.as_ref(), &args, false, os),
            session::RunSandbox::By(_)
        ),
        "a sandboxed start: {:?}",
        words(&launch)
    );
    fs::create_dir_all(root.join("workspaces/later")).unwrap();
    let made = purlis_core::skills::levels::made_since(root);
    let why = made(&root.join("workspaces/later")).expect("made while the chat runs");
    assert!(why.contains("chat 3"), "{why}");
    assert_eq!(
        made(&root.join("workspaces/alpha")),
        None,
        "there at its start"
    );
}
