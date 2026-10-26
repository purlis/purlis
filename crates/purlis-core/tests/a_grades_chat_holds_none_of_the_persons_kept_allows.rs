// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A grade's chat keeps none of the person's own Allows (SK-16, the operator's ruling of
//! 2026-10-26 on the review of #3229): a socket, a folder and a port kept for this project on
//! this machine reach every ordinary chat's compiled sandbox, and none of them reaches the one a
//! grade starts its chats in ([`purlis_core::skills::grades::ready_to_grade`]).

use std::fs;
use std::path::{Path, PathBuf};

use purlis_core::harness::Harness;
use purlis_core::profiles;
use purlis_core::sandbox::{self, Applied, Form, local};
use purlis_core::skills::grades;
use purlis_core::start::{self, Start};

/// The socket kept: outside every folder a chat may write, and not there, which keeps a grant
/// (an engine that is not running has no socket).
const SOCKET: &str = "/purlis-test-nowhere/kept.sock";

/// The port kept: a service on this Mac every chat of the project may connect to.
const PORT: u16 = 5432;

/// A project whose chats run in the full sandbox, on a Claude Code profile `work` whose program
/// is where no chat writes and whose own settings folder is empty.
struct Project {
    dir: tempfile::TempDir,
    /// Claude Code's settings folder for the profile: empty, so no settings turn its prompts off.
    _settings: tempfile::TempDir,
    _outside: stand_in::NoChatWrites,
}

impl Project {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("charter.toml"), "[sandbox]\nmode = \"on\"\n").unwrap();
        fs::create_dir_all(root.join("personas/steward")).unwrap();
        fs::write(root.join("personas/steward/persona.md"), "# steward\n").unwrap();
        let outside = stand_in::NoChatWrites::new();
        let claude = stand_in::claude_code(outside.path(), "claude", "#!/bin/sh\nexit 0\n");
        let settings = tempfile::tempdir().unwrap();
        fs::write(
            root.join(profiles::LOCAL_FILE),
            format!(
                "[harness.work]\nkind = \"claude\"\ncommand = [{:?}]\nenv = {{ CLAUDE_CONFIG_DIR = {:?} }}\n",
                claude.display().to_string(),
                settings.path().display().to_string()
            ),
        )
        .unwrap();
        let project = Self {
            dir,
            _settings: settings,
            _outside: outside,
        };
        let set = profiles::current(project.root());
        let profile = set.get("work").expect("the profile is declared");
        purlis_core::profiletrust::record_launched(
            project.root(),
            "work",
            &purlis_core::profiletrust::fingerprint(profile),
        )
        .unwrap();
        project
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    /// The folder kept: one in the project, which a chat may be granted.
    fn folder(&self) -> PathBuf {
        self.root()
            .canonicalize()
            .expect("the project's real path")
            .join("workspaces/beta/out")
    }

    fn start(&self) -> Start {
        Start {
            profile: Some("work".to_owned()),
            persona: None,
            name: "grade.1".to_owned(),
            cwd: Some(self.root().to_path_buf()),
            resume: None,
            show_footer: false,
            resuming: None,
            without_sandbox: None,
            held: None,
            grants: Default::default(),
            made: sandbox::Made::Window,
        }
    }
}

/// What a compiled sandbox holds of the three kept Allows: the socket, the folder, the port,
/// each by every place it could reach the chat — what a run for it is held to, and the
/// `--settings` Claude Code is handed.
fn held(applied: &Applied, folder: &Path) -> (bool, bool, bool) {
    let confines = applied.confines();
    let settings = match applied.form() {
        Form::ClaudeCode(settings) => settings.sandbox.to_string(),
        other => panic!("a grade's chat runs on Claude Code, not {other:?}"),
    };
    let socket =
        confines.widened.sockets.contains(&PathBuf::from(SOCKET)) || settings.contains(SOCKET);
    let folder = confines.writable.iter().any(|one| one == folder)
        || applied.writable().iter().any(|one| one == folder)
        || settings.contains(&folder.display().to_string());
    let port = confines.widened.ports.contains(&PORT)
        || applied
            .plan()
            .is_some_and(|plan| plan.given.contains(&PORT));
    (socket, folder, port)
}

#[test]
fn a_grades_chat_holds_none_of_the_socket_folder_and_port_an_ordinary_chat_is_given() {
    purlis_core::unsteered!();
    let project = Project::new();
    let root = project.root();
    let folder = project.folder();
    fs::create_dir_all(&folder).unwrap();
    local::grant_socket(root, Path::new(SOCKET)).expect("the socket is kept");
    local::grant_write(root, &folder).expect("the folder is kept");
    local::grant_port(root, PORT).expect("the port is kept");

    // Not vacuous: an ordinary chat here is given all three.
    let ordinary = match start::ready(&project.start(), root) {
        Ok(ready) => ready,
        // No skip where a machine has no sandbox backend: CI's runners have one, and a test
        // that passes by not running proves nothing.
        Err(refused) => panic!("an ordinary chat starts: {refused}"),
    };
    assert_eq!(ordinary.harness, Some(Harness::ClaudeCode));
    let applied = ordinary.sandbox.as_ref().expect("sandboxed");
    assert_eq!(applied.level(), sandbox::Level::Full);
    assert_eq!(
        held(applied, &folder),
        (true, true, true),
        "an ordinary chat is given the kept socket, folder and port"
    );

    // A grade's chat starts, in the full sandbox, and is given none of them.
    let graded = grades::ready_to_grade(&project.start(), root).expect("a grade's chat starts");
    let applied = graded.sandbox.as_ref().expect("sandboxed");
    assert_eq!(applied.level(), sandbox::Level::Full);
    assert_eq!(
        held(applied, &folder),
        (false, false, false),
        "a grade's chat holds none of the person's kept Allows"
    );
    assert_eq!(grades::chat_refused(&graded), Ok(sandbox::Level::Full));

    // Held only while the grade's chat is decided: the next ordinary chat is given them again.
    let after = start::ready(&project.start(), root).expect("an ordinary chat starts");
    assert_eq!(
        held(after.sandbox.as_ref().expect("sandboxed"), &folder),
        (true, true, true)
    );
}
