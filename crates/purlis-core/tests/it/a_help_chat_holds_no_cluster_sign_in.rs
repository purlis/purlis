// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Pool setup's Get help chat can't change a pool** (#4497, story 60): where an ordinary chat
//! of the project reaches the cluster's API server the project lists, and may read the person's
//! kubeconfigs, the help chat ([`purlis_core::poolsetup::help::ready_to_help`]) is granted no
//! host and is denied `~/.kube`, so it holds no sign-in to a cluster and no way to one.

use std::fs;
use std::path::Path;

use crate::support::{self, ClaudeProfile};

use purlis_core::harness::Harness;
use purlis_core::poolsetup::help;
use purlis_core::profiles;
use purlis_core::sandbox::{self, Applied};
use purlis_core::start::{self, Start};

/// The cluster's API server, as the project lists it for its chats.
const API: &str = "k8s.team.example";

/// A project whose chats run in the full sandbox and may reach [`API`], on a Claude Code profile
/// `work` whose program is where no chat writes ([`support::sandboxed_claude_profile`]).
struct Project {
    dir: tempfile::TempDir,
    /// Claude Code's settings folder for the project's profile: empty, so no settings turn its
    /// prompts off.
    _settings: Option<tempfile::TempDir>,
    _outside: stand_in::NoChatWrites,
}

impl Project {
    /// The project for `test`, or `None` where a child ran it.
    fn new(test: &str) -> Option<Self> {
        let declared_here = match support::sandboxed_claude_profile(test) {
            ClaudeProfile::Ran => return None,
            ClaudeProfile::Project => true,
            ClaudeProfile::SandboxOnly(_) => false,
        };
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(
            root.join("charter.toml"),
            format!("[sandbox]\nmode = \"on\"\nhosts = [\"{API}\"]\n"),
        )
        .unwrap();
        fs::create_dir_all(root.join("personas/steward")).unwrap();
        fs::write(root.join("personas/steward/persona.md"), "# steward\n").unwrap();
        let outside = stand_in::NoChatWrites::new();
        let settings = declared_here.then(|| {
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
            let set = profiles::current(root);
            let profile = set.get("work").expect("the profile is declared");
            purlis_core::profiletrust::record_launched(
                root,
                "work",
                &purlis_core::profiletrust::fingerprint(profile),
            )
            .unwrap();
            settings
        });
        Some(Self {
            dir,
            _settings: settings,
            _outside: outside,
        })
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn start(&self) -> Start {
        Start {
            profile: Some("work".to_owned()),
            persona: None,
            name: "help.1".to_owned(),
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

/// Whether `applied` lets the chat reach [`API`], and whether it denies the person's `~/.kube`.
fn held(applied: &Applied) -> (bool, bool) {
    let confines = applied.confines();
    let reaches = confines.hosts.iter().any(|host| host == API)
        || confines.decides().hosts().iter().any(|host| host == API);
    let kube = dirs::home_dir().expect("a home").join(".kube");
    let denied = confines.denied.iter().any(|denial| denial.path == kube);
    (reaches, denied)
}

#[test]
fn a_help_chat_reaches_no_cluster_and_reads_no_kubeconfig() {
    purlis_core::unsteered!();
    let Some(project) = Project::new(
        "a_help_chat_holds_no_cluster_sign_in::a_help_chat_reaches_no_cluster_and_reads_no_kubeconfig",
    ) else {
        return;
    };
    let root = project.root();

    // Not vacuous: an ordinary chat here reaches the cluster and is not denied the kubeconfig.
    let ordinary = match start::ready(&project.start(), root) {
        Ok(ready) => ready,
        // No skip where a machine has no sandbox backend: CI's runners have one.
        Err(refused) => panic!("an ordinary chat starts: {refused}"),
    };
    assert_eq!(ordinary.harness, Some(Harness::ClaudeCode));
    let applied = ordinary.sandbox.as_ref().expect("sandboxed");
    assert_eq!(applied.level(), sandbox::Level::Full);
    assert_eq!(held(applied), (true, false), "an ordinary chat");

    // The help chat starts in the full sandbox, reaching no cluster and reading no kubeconfig.
    let helping = help::ready_to_help(&project.start(), root).expect("a help chat starts");
    let applied = helping.sandbox.as_ref().expect("sandboxed");
    assert_eq!(applied.level(), sandbox::Level::Full);
    assert_eq!(held(applied), (false, true), "the help chat");

    // Held only while the help chat is decided.
    let after = start::ready(&project.start(), root).expect("an ordinary chat starts");
    assert_eq!(
        held(after.sandbox.as_ref().expect("sandboxed")),
        (true, false)
    );
}

/// **A help chat is held at every start after its first** (#4497, story 60): its record says it
/// is one ([`purlis_core::reopen::Chat::held_from_clusters`]), that survives the record's round
/// trip, and a start of the chat out of its record ([`help::ready_again`], what a relaunch, a
/// restart, Again and a move all start it with) compiles the very sandbox its first start did.
#[test]
fn a_help_chat_started_again_from_its_record_is_held_as_at_its_first_start() {
    purlis_core::unsteered!();
    let Some(project) = Project::new(
        "a_help_chat_holds_no_cluster_sign_in::a_help_chat_started_again_from_its_record_is_held_as_at_its_first_start",
    ) else {
        return;
    };
    let root = project.root();
    let first = help::ready_to_help(&project.start(), root).expect("a help chat starts");
    let first = first
        .sandbox
        .as_ref()
        .expect("sandboxed")
        .confines()
        .clone();

    // Recorded as the app records it, and read back as a relaunch reads it.
    let record = purlis_core::reopen::Record {
        chats: vec![
            purlis_core::reopen::Chat {
                program: "claude".to_owned(),
                name: "help.1".to_owned(),
                cwd: Some(root.to_path_buf()),
                profile: Some("work".to_owned()),
                number: Some(1),
                held_from_clusters: true,
                ..Default::default()
            },
            purlis_core::reopen::Chat {
                program: "claude".to_owned(),
                name: "ordinary.2".to_owned(),
                cwd: Some(root.to_path_buf()),
                profile: Some("work".to_owned()),
                number: Some(2),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    purlis_core::reopen::write(root, &record).expect("the record is written");
    let back = purlis_core::reopen::read_or_refusal(root)
        .expect("the record reads")
        .chats;
    assert!(back[0].held_from_clusters, "the help chat's record says so");
    assert!(!back[1].held_from_clusters, "no other chat's does");

    let again = help::ready_again(&back[0], &project.start(), root).expect("it starts again");
    let applied = again.sandbox.as_ref().expect("sandboxed");
    assert_eq!(applied.level(), sandbox::Level::Full);
    assert_eq!(held(applied), (false, true), "the help chat started again");
    assert_eq!(
        applied.confines(),
        &first,
        "the sandbox its first start compiled: no cluster host, every kubeconfig denied"
    );

    // Not vacuous: an ordinary chat started again from the same record reaches the cluster.
    let ordinary = help::ready_again(&back[1], &project.start(), root).expect("it starts again");
    assert_eq!(
        held(ordinary.sandbox.as_ref().expect("sandboxed")),
        (true, false),
        "an ordinary chat started again"
    );
}
