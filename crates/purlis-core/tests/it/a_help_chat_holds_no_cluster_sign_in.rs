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
    /// Whether `work` is this machine's sandbox-only profile, which starts no chat unsandboxed.
    sandbox_only: bool,
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
            sandbox_only: !declared_here,
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

    // Its first start, with the record on disk as at every later one: the record's own folder
    // is part of what a start compiles.
    let first = help::ready_to_help(&project.start(), root).expect("a help chat starts");
    let first = first
        .sandbox
        .as_ref()
        .expect("sandboxed")
        .confines()
        .clone();

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

/// The help chat's record, as the app writes it: what [`help::ready_again`] starts again.
fn help_chat(root: &Path) -> purlis_core::reopen::Chat {
    purlis_core::reopen::Chat {
        program: "claude".to_owned(),
        name: "help.1".to_owned(),
        cwd: Some(root.to_path_buf()),
        profile: Some("work".to_owned()),
        number: Some(1),
        held_from_clusters: true,
        ..Default::default()
    }
}

/// **A help chat is never started again without the sandbox** (#4497, story 60): the person's
/// Start without the sandbox, which any other chat may take, is refused for it, and says why.
#[test]
fn a_help_chat_is_not_started_again_without_the_sandbox() {
    purlis_core::unsteered!();
    let Some(project) = Project::new(
        "a_help_chat_holds_no_cluster_sign_in::a_help_chat_is_not_started_again_without_the_sandbox",
    ) else {
        return;
    };
    let root = project.root();
    let unsandboxed = || Start {
        without_sandbox: Some(sandbox::OptOut {
            reason: Some("started again without the sandbox".to_owned()),
        }),
        ..project.start()
    };

    // Not vacuous: an ordinary chat here starts again without the sandbox when asked to, where
    // the profile allows it at all (a sandbox-only profile, on Linux, refuses every chat that).
    let ordinary = purlis_core::reopen::Chat {
        held_from_clusters: false,
        ..help_chat(root)
    };
    if !project.sandbox_only {
        let ready = help::ready_again(&ordinary, &unsandboxed(), root).expect("it starts again");
        assert!(ready.sandbox.is_none(), "an ordinary chat runs as asked");
    }

    let refused = help::ready_again(&help_chat(root), &unsandboxed(), root)
        .expect_err("a help chat is not started without the sandbox");
    assert_eq!(refused, help::ONLY_SANDBOXED);
    assert!(
        refused.contains("Get help runs only in the sandbox"),
        "{refused}"
    );

    // With the sandbox it starts again, held.
    let again = help::ready_again(&help_chat(root), &project.start(), root).expect("it starts");
    assert_eq!(
        held(again.sandbox.as_ref().expect("sandboxed")),
        (false, true)
    );
}

/// **A help chat is not started again where the project now runs chats without the sandbox**
/// (#4497, story 60), as its first start is refused there: here, the person's own level is off.
#[test]
fn a_help_chat_is_not_started_again_in_a_project_without_the_sandbox() {
    purlis_core::unsteered!();
    let Some(project) = Project::new(
        "a_help_chat_holds_no_cluster_sign_in::a_help_chat_is_not_started_again_in_a_project_without_the_sandbox",
    ) else {
        return;
    };
    let root = project.root();
    // The person's own level for this project, set in Settings on this machine: off.
    let local = root.join(profiles::LOCAL_FILE);
    let text = fs::read_to_string(&local).unwrap_or_default();
    fs::write(&local, format!("[sandbox]\nlevel = \"off\"\n\n{text}")).unwrap();
    sandbox::personal::set_by_you(root, None, Some(sandbox::personal::Choice::Off))
        .expect("your level");

    let refused = help::ready_again(&help_chat(root), &project.start(), root)
        .expect_err("a help chat is not started without the sandbox");
    if project.sandbox_only {
        // A sandbox-only profile (Linux) starts no chat unsandboxed, help chat or not: it
        // refuses first, in its own words.
        return;
    }
    assert_eq!(refused, help::PROJECT_UNSANDBOXED);

    // Not vacuous: an ordinary chat of this project starts again, and runs unsandboxed.
    let ordinary = purlis_core::reopen::Chat {
        held_from_clusters: false,
        ..help_chat(root)
    };
    let ready = help::ready_again(&ordinary, &project.start(), root).expect("it starts again");
    assert!(
        ready.sandbox.is_none(),
        "the project runs its chats unsandboxed"
    );
}

/// The session record the help chat writes, as `purlis session record` writes it from inside
/// it: every fact in its frontmatter from the app's record of the chat
/// ([`purlis_core::sessionrecord::chat_facts`]). Its plane-relative path.
fn help_chats_session_record(root: &Path) -> String {
    let chat = purlis_core::reopen::Chat {
        identity: purlis_core::reopen::Identity {
            id: Some("01K6E8ZK6V4Q9T0N3M2B1C5D7F".to_owned()),
            ..Default::default()
        },
        ..help_chat(root)
    };
    let record = purlis_core::reopen::Record {
        chats: vec![chat],
        ..Default::default()
    };
    purlis_core::reopen::write(root, &record).expect("the record is written");
    let facts = purlis_core::sessionrecord::Facts {
        place: purlis_core::active::Place::PlaneRoot,
        at: chrono::NaiveDate::from_ymd_opt(2026, 11, 14)
            .unwrap()
            .and_hms_opt(4, 0, 0)
            .unwrap(),
        chat: Some(purlis_core::sessionrecord::chat_facts(root, 1)),
        persona: None,
        pieces: Vec::new(),
    };
    let shown = purlis_core::sessionrecord::record(
        root,
        &purlis_core::sessionrecord::New {
            title: "Why the Pool step failed",
            body: "## Goal\n\ng\n\n## Done\n\nd\n\n## Decisions\n\nx\n\n## Open\n\no\n\n## \
                   How to resume\n\nr\n",
            facts: &facts,
        },
    )
    .expect("the session record is written")
    .shown;
    // Once the chat is closed, the app's record of it is gone: only the session record is left.
    purlis_core::reopen::write(root, &purlis_core::reopen::Record::default())
        .expect("the record is written");
    shown
}

/// **Resuming a help chat's session record starts it as a restart does** (#4497, story 60): held
/// from every cluster, and recorded as a help chat, so its own later starts are held too.
#[test]
fn a_help_chat_resumed_from_its_session_record_is_held_as_at_its_first_start() {
    purlis_core::unsteered!();
    let Some(project) = Project::new(
        "a_help_chat_holds_no_cluster_sign_in::a_help_chat_resumed_from_its_session_record_is_held_as_at_its_first_start",
    ) else {
        return;
    };
    let root = project.root();
    let shown = help_chats_session_record(root);

    let resumed = purlis_core::sessionresume::ready(root, &shown, "help.2", false)
        .expect("the help chat resumes");
    let applied = resumed.ready.sandbox.as_ref().expect("sandboxed");
    assert_eq!(applied.level(), sandbox::Level::Full);
    assert_eq!(held(applied), (false, true), "the help chat resumed");
    assert!(
        resumed.chat("help.2").held_from_clusters,
        "the resumed chat is recorded as a help chat"
    );
}

/// **A help chat is not resumed where the project now runs chats without the sandbox**, as its
/// first start and a restart are refused there.
#[test]
fn a_help_chat_is_not_resumed_in_a_project_without_the_sandbox() {
    purlis_core::unsteered!();
    let Some(project) = Project::new(
        "a_help_chat_holds_no_cluster_sign_in::a_help_chat_is_not_resumed_in_a_project_without_the_sandbox",
    ) else {
        return;
    };
    let root = project.root();
    let shown = help_chats_session_record(root);
    let local = root.join(profiles::LOCAL_FILE);
    let text = fs::read_to_string(&local).unwrap_or_default();
    fs::write(&local, format!("[sandbox]\nlevel = \"off\"\n\n{text}")).unwrap();
    sandbox::personal::set_by_you(root, None, Some(sandbox::personal::Choice::Off))
        .expect("your level");

    let refused = purlis_core::sessionresume::ready(root, &shown, "help.2", false)
        .expect_err("a help chat is not resumed without the sandbox");
    if project.sandbox_only {
        // A sandbox-only profile (Linux) starts no chat unsandboxed: it refuses in its own words.
        return;
    }
    assert_eq!(refused, help::PROJECT_UNSANDBOXED);
}
