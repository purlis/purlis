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

    /// What purlis ships, as stand-ins: a `purlis` that does nothing, and an empty plugin.
    fn kit(&self) -> Kit {
        let plugin = self.bin.path().join("plugin");
        fs::create_dir_all(&plugin).unwrap();
        Kit {
            binary: Some(stand_in::program(
                self.bin.path(),
                "purlis",
                "#!/bin/sh\nexit 0\n",
            )),
            plugin: Some(plugin),
            git_hooks: None,
        }
    }

    fn program(&self) -> String {
        self.bin.path().join("claude").display().to_string()
    }
}

/// What the host was asked to do beside the start.
#[derive(Default)]
struct Host {
    confined: RefCell<Vec<ConfinePlan>>,
    recorded: RefCell<Vec<serde_json::Value>>,
    refuses_to_record: bool,
}

impl Beside for Host {
    fn confine(&self, plan: &ConfinePlan) -> Result<Confined, String> {
        self.confined.borrow_mut().push(plan.clone());
        Ok(Confined {
            confinement: "01J9CNFNMNT0000000000000000".into(),
            http: 50811,
            socks: 50812,
            tmp: None,
            routed: None,
            ssh: None,
        })
    }

    fn record(&self, entry: serde_json::Value) -> Result<(), String> {
        if self.refuses_to_record {
            return Err("the event log is not open".into());
        }
        self.recorded.borrow_mut().push(entry);
        Ok(())
    }
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

    let launch = composed(&project, &asking(|_| {}), &host).expect("composed");

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
    assert_eq!(recorded.len(), 1, "{recorded:?}");
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

/// Where this machine sandboxes a chat, the host starts its confinement, as the program's own,
/// and the chat is wrapped in it.
#[cfg(target_os = "macos")]
#[test]
fn a_sandboxed_start_runs_in_the_confinement_the_host_started_for_it() {
    purlis_core::unsteered!();
    let project = Project::on("claude");
    let host = Host::default();

    let launch = composed(&project, &asking(|_| {}), &host).expect("composed");

    assert_eq!(host.confined.borrow().len(), 1, "one confinement");
    assert!(!host.confined.borrow()[0].listed.is_empty(), "its reach");
    assert_eq!(
        launch.confinement.as_deref(),
        Some("01J9CNFNMNT0000000000000000")
    );
    assert_eq!(
        set(&launch, purlis_core::hookwire::SANDBOXED_ENV).as_deref(),
        Some("1")
    );
}
