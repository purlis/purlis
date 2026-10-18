// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A session host with no window, `purlis serve` on a runner (#2447), arms each chat it composes
//! with what is installed beside its own binary: purlis's plugin, from the place a package or the
//! runner image puts it, and purlis's git hooks, which it writes in its data folder itself as the
//! window writes them at its launch. So a sandboxed Claude Code chat it composes starts, armed as
//! one the window's own host composes.

#![cfg(unix)]

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use purlis_core::hoststart::{self, Beside, Kit, Starting};
use purlis_core::profiles;
use purlis_session_protocol::session::{
    self, ConfinePlan, Confined, Launch, ProgramKind, StartProgram,
};

/// Where a package or the runner image installs purlis: `bin/purlis`, and its plugin in
/// `lib/purlis/plugin`.
struct Installed {
    prefix: tempfile::TempDir,
}

impl Installed {
    fn with_its_plugin() -> Installed {
        let installed = Installed::without_its_plugin();
        let plugin = installed.plugin();
        fs::create_dir_all(plugin.join(".claude-plugin")).unwrap();
        fs::write(
            plugin.join(".claude-plugin/plugin.json"),
            r#"{"name":"purlis"}"#,
        )
        .unwrap();
        installed
    }

    fn without_its_plugin() -> Installed {
        let prefix = tempfile::tempdir().unwrap();
        fs::create_dir_all(prefix.path().join("bin")).unwrap();
        stand_in::program(&prefix.path().join("bin"), "purlis", "#!/bin/sh\nexit 0\n");
        Installed { prefix }
    }

    fn binary(&self) -> PathBuf {
        self.prefix.path().join("bin/purlis")
    }

    fn plugin(&self) -> PathBuf {
        self.prefix.path().join("lib/purlis/plugin")
    }
}

/// A project with the persona `steward` and the profile `work` running a stand-in Claude Code.
struct Project {
    dir: tempfile::TempDir,
    bin: tempfile::TempDir,
}

impl Project {
    fn on_claude_code() -> Project {
        let project = Project {
            dir: tempfile::tempdir().unwrap(),
            bin: tempfile::tempdir().unwrap(),
        };
        let root = project.dir.path();
        fs::write(root.join("charter.toml"), "").unwrap();
        fs::create_dir_all(root.join("personas/steward")).unwrap();
        fs::write(root.join("personas/steward/persona.md"), "# steward\n").unwrap();
        let program = stand_in::claude_code(project.bin.path(), "claude", "#!/bin/sh\n");
        fs::write(
            root.join(profiles::LOCAL_FILE),
            format!(
                "[harness.work]\nkind = \"claude\"\ncommand = [{:?}]\n",
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

    /// Its chat's start, sandboxed on a Mac whatever this machine is, so no program is run.
    fn sandboxed_start(&self) -> purlis_core::start::Ready {
        let harness = purlis_core::harness::Harness::ClaudeCode;
        let applied = purlis_core::sandbox::for_start(
            harness,
            self.root(),
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
            program: self.bin.path().join("claude").display().to_string(),
            command: Vec::new(),
            args: Vec::new(),
            env: vec![("PURLIS_PERSONA".to_owned(), "steward".to_owned())],
            cwd: Some(self.root().to_path_buf()),
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
}

/// A host that starts every confinement it is asked for and records nothing.
#[derive(Default)]
struct Host {
    tmp: Option<tempfile::TempDir>,
    confined: RefCell<u32>,
}

impl Beside for Host {
    fn confine(&self, _plan: &ConfinePlan) -> Result<Confined, String> {
        *self.confined.borrow_mut() += 1;
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

    fn record(&self, _entry: serde_json::Value) -> Result<(), String> {
        Ok(())
    }
}

fn composed(project: &Project, kit: &Kit, host: &Host) -> Result<Launch, String> {
    let start = StartProgram {
        program: Some("01J9CHAT000000000000000000".into()),
        run: Some("01J9RUN0000000000000000000".into()),
        persona: Some("steward".into()),
        profile: Some("work".into()),
        ..StartProgram::of("01J9PRJCT00000000000000000", ProgramKind::Harness)
    };
    hoststart::compose_ready(
        &Starting {
            root: project.root(),
            number: 3,
            start: &start,
            reporting: Some(&session::Reporting {
                socket: "/the/project/hooks.sock".into(),
                token: "token-3".into(),
            }),
            started_by: "local-ui",
        },
        project.sandboxed_start(),
        kit,
        host,
    )
}

#[test]
fn the_kit_has_the_plugin_installed_beside_the_binary_and_git_hooks_that_run_it() {
    purlis_core::unsteered!();
    let installed = Installed::with_its_plugin();
    let data = tempfile::tempdir().unwrap();

    let kit = Kit::installed_beside(installed.binary(), data.path());

    assert_eq!(kit.binary.as_deref(), Some(installed.binary().as_path()));
    assert_eq!(
        kit.plugin
            .as_deref()
            .map(Path::canonicalize)
            .transpose()
            .unwrap(),
        Some(installed.plugin().canonicalize().unwrap())
    );
    let hooks = kit.git_hooks.expect("its git hooks");
    assert!(
        hooks.dir().starts_with(data.path()),
        "{} is in its data folder",
        hooks.dir().display()
    );
    let pre_commit = fs::read_to_string(hooks.dir().join("pre-commit")).expect("a pre-commit");
    assert!(
        pre_commit.contains(&installed.binary().display().to_string()),
        "the hook runs this purlis:\n{pre_commit}"
    );
}

#[test]
fn a_sandboxed_claude_code_chat_starts_armed_with_the_plugin_beside_the_binary() {
    purlis_core::unsteered!();
    let installed = Installed::with_its_plugin();
    let data = tempfile::tempdir().unwrap();
    let kit = Kit::installed_beside(installed.binary(), data.path());
    let project = Project::on_claude_code();
    let host = Host {
        tmp: Some(tempfile::tempdir().unwrap()),
        ..Host::default()
    };

    let launch = composed(&project, &kit, &host).expect("it starts");

    assert!(
        launch
            .set
            .iter()
            .any(|(key, value)| key == purlis_core::hookwire::SANDBOXED_ENV && value == "1"),
        "it is sandboxed: {:?}",
        launch.set
    );
    let line = std::iter::once(launch.program.clone())
        .chain(launch.args.iter().cloned())
        .collect::<Vec<_>>();
    let plugin_dir = line
        .iter()
        .position(|word| word == "--plugin-dir")
        .unwrap_or_else(|| panic!("no plugin on its line: {line:?}"));
    assert_eq!(
        Path::new(&line[plugin_dir + 1]).canonicalize().unwrap(),
        installed.plugin().canonicalize().unwrap()
    );
    let hooks_path = launch.git_hooks.clone().expect("its git hooks");
    assert!(
        Path::new(&hooks_path).starts_with(data.path()),
        "its commits go through purlis's git hooks, at {hooks_path}"
    );
}

#[test]
fn with_no_plugin_beside_the_binary_a_sandboxed_claude_code_chat_is_still_refused() {
    purlis_core::unsteered!();
    let installed = Installed::without_its_plugin();
    let data = tempfile::tempdir().unwrap();
    let kit = Kit::installed_beside(installed.binary(), data.path());
    let project = Project::on_claude_code();
    let host = Host {
        tmp: Some(tempfile::tempdir().unwrap()),
        ..Host::default()
    };

    let refused = composed(&project, &kit, &host).unwrap_err();

    assert!(refused.contains("plugin"), "{refused}");
}
