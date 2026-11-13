// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::*;
use crate::eventlog::{ArgsKey, Attributes, Began, Log, Recorder, RunOf};
use crate::harness::Harness;
use crate::sandbox::{Asking, Decided, OptOut, Os, Person, decide_granted, grant, personal};

const DEVICE: &str = "01J9ZZDEVICE0000000000000";
const CHAT: &str = "01J9ZZCHAT000000000000000A";
const RUN: &str = "01J9ZZRUN0000000000000000A";

/// A project at the full sandbox, with no cache folders to find, so its sandbox compiles here.
const FULL: &str = "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n";

/// A project whose team requires the sandbox: its `policy = "never"`.
const NEVER: &str =
    "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\npolicy = \"never\"\n";

/// An administrator's policy on this machine that forbids starting without the sandbox.
const NO_OPT_OUT: &str = r#"{"owner": "Platform team", "sandbox": {"opt-out": false}}"#;

/// A project with `manifest`, committed: the commit's id.
fn project(dir: &Path, manifest: &str) -> String {
    std::fs::write(dir.join("charter.toml"), format!("schema = 1\n{manifest}")).unwrap();
    // This machine's own file, which git never carries, as a project made by purlis ignores it.
    std::fs::write(
        dir.join(".gitignore"),
        format!("{}\n", crate::profiles::LOCAL_FILE),
    )
    .unwrap();
    git(dir, &["init", "-q"]);
    git(dir, &["add", "charter.toml", ".gitignore"]);
    git(dir, &["commit", "-q", "-m", "the project"]);
    git(dir, &["rev-parse", "HEAD"]).out.trim().to_owned()
}

fn git(dir: &Path, args: &[&str]) -> crate::worktree::git::Run {
    let ran = crate::testgit::run(dir, args);
    assert!(ran.ok(), "{args:?}: {ran:?}");
    ran
}

/// The person's own levels on this machine: the slots they set or confirmed in Settings, each
/// `(persona, level)`, and what the local file says, where it is not just those.
struct Levels {
    file: &'static str,
    confirmed: &'static [(Option<&'static str>, Choice)],
}

const NO_LEVELS: Levels = Levels {
    file: "",
    confirmed: &[],
};

fn levels_in(dir: &Path, levels: &Levels) {
    let write = |file: &str| std::fs::write(dir.join(crate::profiles::LOCAL_FILE), file).unwrap();
    // The file as Settings writes it for the confirmed slots, then their Confirm.
    let mut file = String::new();
    let mut personas = String::new();
    for (persona, level) in levels.confirmed {
        match persona {
            None => file.push_str(&format!("[sandbox]\nlevel = \"{}\"\n", level.word())),
            Some(name) => personas.push_str(&format!("{name} = \"{}\"\n", level.word())),
        }
    }
    if !personas.is_empty() {
        file.push_str(&format!("[sandbox.personas]\n{personas}"));
    }
    write(&file);
    for (persona, level) in levels.confirmed {
        personal::set_by_you(dir, *persona, Some(*level)).expect("confirmed");
    }
    // What the file says since, where the case has it say something else.
    if !levels.file.is_empty() {
        write(levels.file);
    }
}

/// One way a chat starts, and what its start event must record of its sandbox.
struct Case {
    name: &'static str,
    manifest: &'static str,
    levels: Levels,
    persona: Option<&'static str>,
    made: Made,
    locks: Option<&'static str>,
    opt_out: bool,
    os: Os,
    level: &'static str,
    tier: &'static str,
}

impl Case {
    const fn window(
        name: &'static str,
        manifest: &'static str,
        level: &'static str,
        tier: &'static str,
    ) -> Case {
        Case {
            name,
            manifest,
            levels: NO_LEVELS,
            persona: None,
            made: Made::Window,
            locks: None,
            opt_out: false,
            os: Os::MacOs,
            level,
            tier,
        }
    }
}

/// What the start event of `case` records of its protection, as the start resolved it, and the
/// commit its project is at.
fn recorded(case: &Case) -> (serde_json::Value, String) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let commit = project(root, case.manifest);
    levels_in(root, &case.levels);
    let machine = crate::sandbox::Machine {
        env: crate::secrets::Env::of(&[]),
        // Not under `/home`: on macOS that is an autofs mount, where every name that is not there
        // costs a trip to the automounter, and a start resolves hundreds of them (T1-46).
        home: Some(std::path::PathBuf::from("/nonexistent-home/op")),
        os: case.os,
    };
    policy::set_for_this_test(match case.locks {
        Some(text) => policy::Locks::parse(text, Path::new("policy.json")),
        None => policy::Locks::none(),
    });
    let opt_out = OptOut { reason: None };
    let person = Person {
        opt_out: case.opt_out.then_some(&opt_out),
        level_as: case.persona,
        made: &case.made,
    };
    let decided = decide_granted(
        Harness::ClaudeCode,
        root,
        &machine,
        &|_| true,
        &person,
        case.persona,
        &grant::Grants::default(),
    );
    let (sandbox, lifted) = match &decided {
        Ok(Some(Decided::Sandboxed(applied))) => (Some(applied), None),
        Ok(Some(Decided::Unsandboxed(lifted))) => (None, Some(lifted)),
        Ok(Some(Decided::Floorless(_))) => (None, None),
        other => panic!("{}: {other:?}", case.name),
    };
    let protection = at_start(root, case.persona, &case.made, sandbox, lifted);
    policy::set_for_this_test(policy::Locks::none());
    (started_with(root, protection), commit)
}

/// The `protection` of the one `run.started` a start under `protection` writes.
fn started_with(root: &Path, protection: InForce) -> serde_json::Value {
    let attributes = Attributes {
        protection: Some(protection),
        ..Attributes::default()
    };
    let log = tempfile::tempdir().unwrap();
    let mut host = Recorder::new(
        Log::open(log.path(), DEVICE).unwrap(),
        ArgsKey::open(log.path()).unwrap(),
    );
    host.begin(
        root,
        1,
        RunOf {
            chat: CHAT,
            run: RUN,
        },
        Began::Start,
        &attributes,
    )
    .unwrap();
    let events = crate::eventlog::read(log.path()).unwrap();
    let started: Vec<_> = events.iter().filter(|e| e.kind == "run.started").collect();
    assert_eq!(started.len(), 1, "one start event, {events:#?}");
    started[0].body["protection"].clone()
}

/// What the start event of each case records of its protection, its working `charter.toml`
/// unchanged and its answers in force present and empty in phase 1.
fn each_records(cases: &[Case]) {
    for case in cases {
        let (protection, commit) = recorded(case);
        assert_eq!(
            protection,
            serde_json::json!({
                "layers": [{"layer": "sandbox", "level": case.level, "tier": case.tier}],
                "charter_commit": commit,
                "charter_dirty": false,
                "answers_in_force": [],
            }),
            "{}",
            case.name
        );
    }
}

fn asked(above: Option<Vec<Option<String>>>) -> Made {
    Made::Asked {
        how: Asking::Dispatch,
        above,
    }
}

/// No tier's level: the default floor, and the project's own where its file turns it on.
/// R-227, Q238.
#[test]
fn a_sandbox_no_tier_set_is_recorded_as_purlis_s_default_or_the_project_s_file() {
    each_records(&[
        Case::window("the default floor", "", "floor", "purlis"),
        Case::window("the project's file", FULL, "on", "project"),
    ]);
}

/// The person's own level, set and confirmed in Settings.
/// R-227, Q238.
#[test]
fn a_level_the_person_set_in_settings_is_recorded_as_theirs() {
    each_records(&[
        Case {
            levels: Levels {
                file: "",
                confirmed: &[(None, Choice::On)],
            },
            ..Case::window("your on at the floor", "", "on", "person")
        },
        Case {
            levels: Levels {
                file: "",
                confirmed: &[(None, Choice::Floor)],
            },
            ..Case::window("your floor at the full sandbox", FULL, "floor", "person")
        },
        Case {
            levels: Levels {
                file: "",
                confirmed: &[(None, Choice::Off)],
            },
            ..Case::window("your off at the floor", "", "off", "person")
        },
        Case {
            levels: Levels {
                file: "",
                confirmed: &[(None, Choice::Off)],
            },
            ..Case::window("your off at the full sandbox", FULL, "off", "person")
        },
    ]);
}

/// A persona's row, confirmed, and one only the file says.
/// R-227, Q238.
#[test]
fn a_persona_row_is_recorded_as_the_person_s_only_where_they_confirmed_it() {
    each_records(&[
        Case {
            levels: Levels {
                file: "",
                confirmed: &[(Some("steward"), Choice::On)],
            },
            persona: Some("steward"),
            ..Case::window("a confirmed persona row", "", "on", "person")
        },
        Case {
            levels: Levels {
                file: "[sandbox.personas]\nsteward = \"on\"\n",
                confirmed: &[],
            },
            persona: Some("steward"),
            ..Case::window("a file-only persona row", "", "on", "unconfirmed")
        },
    ]);
}

/// The project's level only the file says (FileOnly), never confirmed.
/// R-227, Q238.
#[test]
fn a_level_only_the_local_file_says_is_recorded_as_unconfirmed() {
    each_records(&[Case {
        levels: Levels {
            file: "[sandbox]\nlevel = \"on\"\n",
            confirmed: &[],
        },
        ..Case::window("a file-only level", "", "on", "unconfirmed")
    }]);
}

/// A lower level waiting for Confirm: what is in force instead.
/// R-227, Q238.
#[test]
fn a_level_waiting_for_confirm_records_whoever_s_level_is_in_force_instead() {
    each_records(&[
        Case {
            levels: Levels {
                file: "[sandbox]\nlevel = \"off\"\n",
                confirmed: &[],
            },
            ..Case::window(
                "waits, nothing in force, at the floor",
                "",
                "floor",
                "purlis",
            )
        },
        Case {
            levels: Levels {
                file: "[sandbox]\nlevel = \"off\"\n",
                confirmed: &[],
            },
            ..Case::window(
                "waits, nothing in force, at the full sandbox",
                FULL,
                "on",
                "project",
            )
        },
        Case {
            levels: Levels {
                file: "[sandbox]\nlevel = \"floor\"\n",
                confirmed: &[(None, Choice::On)],
            },
            ..Case::window("waits, your on in force", "", "on", "person")
        },
    ]);
}

/// A policy: an administrator's, the team's `policy = "never"`, and both.
/// R-227, Q238.
#[test]
fn a_sandbox_a_policy_holds_is_recorded_as_the_administrator_s_or_the_project_s() {
    each_records(&[
        Case {
            locks: Some(NO_OPT_OUT),
            ..Case::window("an administrator's policy", "", "on", "policy")
        },
        Case::window("the team's never", NEVER, "on", "project"),
        Case {
            locks: Some(NO_OPT_OUT),
            ..Case::window(
                "an administrator's policy and the team's never",
                NEVER,
                "on",
                "policy",
            )
        },
    ]);
}

/// The person, for this chat only.
/// R-227, Q238.
#[test]
fn a_sandbox_lifted_for_one_chat_is_recorded_as_the_chat_s() {
    each_records(&[
        Case {
            opt_out: true,
            ..Case::window("an opt-out for this chat", FULL, "off", "chat")
        },
        Case {
            opt_out: true,
            ..Case::window("an opt-out of the floor", "", "off", "chat")
        },
    ]);
}

/// No sandbox this system can apply: none set it.
/// R-227, Q238.
#[test]
fn no_sandbox_this_system_can_apply_is_recorded_as_purlis_s() {
    each_records(&[
        Case {
            os: Os::Windows,
            ..Case::window("no backend", FULL, "off", "purlis")
        },
        Case {
            os: Os::Windows,
            ..Case::window("floorless", "", "off", "purlis")
        },
    ]);
}

/// Another chat asked for it.
/// R-227, Q238.
#[test]
fn a_chat_another_asked_for_is_recorded_as_the_person_s_only_where_it_takes_their_level() {
    each_records(&[
        Case {
            levels: Levels {
                file: "",
                confirmed: &[(None, Choice::Off)],
            },
            made: asked(None),
            ..Case::window("asked, the chain unread", "", "floor", "purlis")
        },
        Case {
            levels: Levels {
                file: "",
                confirmed: &[(None, Choice::Floor), (Some("steward"), Choice::Off)],
            },
            persona: Some("steward"),
            made: asked(Some(vec![None])),
            ..Case::window("asked by a stricter chat", "", "floor", "person")
        },
        Case {
            levels: Levels {
                file: "",
                confirmed: &[(None, Choice::On)],
            },
            made: asked(Some(vec![None])),
            ..Case::window("asked, at the level it would take", "", "on", "person")
        },
    ]);
}

/// A start that names no way it was made (a peer's, on the host): no level of the
/// person's is read, whatever it is.
/// R-227, Q238.
#[test]
fn a_start_that_names_no_way_it_was_made_is_never_recorded_as_the_person_s() {
    each_records(&[
        Case {
            levels: Levels {
                file: "",
                confirmed: &[(None, Choice::Floor)],
            },
            made: Made::Unnamed,
            ..Case::window(
                "unnamed, your level equal to the floor",
                "",
                "floor",
                "purlis",
            )
        },
        Case {
            levels: Levels {
                file: "",
                confirmed: &[(None, Choice::On)],
            },
            made: Made::Unnamed,
            ..Case::window(
                "unnamed, your on at the full sandbox",
                FULL,
                "on",
                "project",
            )
        },
        Case {
            levels: Levels {
                file: "",
                confirmed: &[(None, Choice::Floor)],
            },
            made: Made::Unnamed,
            ..Case::window(
                "unnamed, your floor at the full sandbox",
                FULL,
                "on",
                "project",
            )
        },
    ]);
}

/// R-227, Q238: a start whose `charter.toml` differs from the commit that last had it ran under
/// text no commit has, and says so.
#[test]
fn a_start_under_a_changed_charter_toml_records_its_commit_as_dirty() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let commit = project(root, FULL);
    std::fs::write(root.join("charter.toml"), format!("schema = 1\n{NEVER}")).unwrap();

    let protection = at_start(root, None, &Made::Window, None, None);

    assert_eq!(protection.charter_commit, commit);
    assert!(protection.charter_dirty, "{protection:?}");
}

/// R-227, Q238: `none` is a project with no commit of `charter.toml` (no repository, or one
/// that never committed it); a repository git could not read is `unknown`, never `none`.
#[test]
fn a_charter_commit_is_none_only_where_no_commit_has_it_and_unknown_where_git_failed() {
    let bare = tempfile::tempdir().unwrap();
    std::fs::write(bare.path().join("charter.toml"), "schema = 1\n").unwrap();
    let untracked = tempfile::tempdir().unwrap();
    project(untracked.path(), "");
    std::fs::write(untracked.path().join("other.toml"), "").unwrap();
    git(untracked.path(), &["rm", "-q", "--cached", "charter.toml"]);
    git(untracked.path(), &["commit", "-q", "-m", "not tracked"]);
    let broken = tempfile::tempdir().unwrap();
    std::fs::write(broken.path().join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::write(broken.path().join(".git"), "gitdir: /nowhere/at/all\n").unwrap();

    let commit = |root: &Path| at_start(root, None, &Made::Window, None, None);

    assert_eq!(commit(bare.path()).charter_commit, crate::eventlog::NONE);
    assert_eq!(
        commit(untracked.path()).charter_commit,
        crate::eventlog::NONE
    );
    let failed = commit(broken.path());
    assert_eq!(failed.charter_commit, crate::eventlog::UNKNOWN);
    assert!(!failed.charter_dirty);
}

/// R-227, Q238: a `run.started` a newer host wrote, with a tier or an answer this version does
/// not know, is still read: its tier as unknown, its answers as they are, and a `protection`
/// in a shape this version cannot read at all as not known, never the whole entry refused.
#[test]
fn a_protection_this_version_does_not_know_never_drops_its_start() {
    let newer = serde_json::json!({
        "persona": "steward",
        "protection": {
            "layers": [{"layer": "sandbox", "level": "on", "tier": "project_requires"}],
            "charter_commit": "none",
            "charter_dirty": false,
            "answers_in_force": [{"recommendation": "egress", "accepted": "2026-11-13"}],
        },
    });
    let read: Attributes = serde_json::from_value(newer).expect("read");
    let protection = read.protection.expect("its protection");
    assert_eq!(protection.layers[0].tier, Tier::Unknown);
    assert_eq!(
        protection.answers_in_force,
        [serde_json::json!({"recommendation": "egress", "accepted": "2026-11-13"})]
    );

    let reshaped = serde_json::json!({"persona": "steward", "protection": {"layers": 3}});
    let read: Attributes = serde_json::from_value(reshaped).expect("read");
    assert_eq!(read.persona.as_deref(), Some("steward"));
    assert_eq!(read.protection, None);
}
