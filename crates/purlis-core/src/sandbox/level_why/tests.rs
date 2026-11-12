// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::sandbox::{Asking, personal, team};

const FULL_PROJECT: &str = "[sandbox]\nmode = \"on\"\n";
/// [`FULL_PROJECT`] with no cache folders to find, for a test that compiles its sandbox.
const FULL_NO_CACHES: &str =
    "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n";

const TEAM_OFF: &str = "[sandbox]\nmode = \"off\"\negress = [\"model-providers\", \"forge\"]\n";

/// A project with `manifest`, and `local` as this machine's own file.
fn project(manifest: &str, local: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a project");
    std::fs::write(
        dir.path().join("charter.toml"),
        format!("schema = 1\n{manifest}"),
    )
    .expect("charter.toml");
    std::fs::write(dir.path().join(crate::profiles::LOCAL_FILE), local).expect("local file");
    dir
}

/// [`TEAM_OFF`] as a commit brought it, which is the only off a person is asked to accept.
fn commit_the_team_s_off(root: &Path) {
    let made = crate::testgit::run(root, &["init", "-q"]);
    assert!(made.ok(), "{made:?}");
    crate::testgit::run(root, &["add", "charter.toml"]);
    let ran = crate::testgit::run(
        root,
        &[
            "-c",
            "user.name=A teammate",
            "-c",
            "user.email=someone@example.invalid",
            "commit",
            "-q",
            "-m",
            "the team's off",
        ],
    );
    assert!(ran.ok(), "{ran:?}");
}

fn next(root: &Path, persona: Option<&str>, made: &Made) -> NextStart {
    next_start(root, persona, &policy::Locks::of(root), made).expect("a manifest it reads")
}

/// The level, and why, of a chat the person starts in the window.
fn in_the_window(root: &Path, persona: Option<&str>) -> NextStart {
    next(root, persona, &Made::Window)
}

fn asked_by(above: &[Option<&str>]) -> Made {
    Made::Asked {
        how: Asking::Dispatch,
        above: Some(above.iter().map(|one| one.map(str::to_owned)).collect()),
    }
}

/// A local file that sets `level` for the project, confirmed by the person.
fn confirmed(manifest: &str, level: Choice) -> tempfile::TempDir {
    let dir = project(
        manifest,
        &format!("[sandbox]\nlevel = \"{}\"\n", level.word()),
    );
    personal::set_by_you(dir.path(), None, Some(level)).expect("confirmed");
    dir
}

#[test]
fn a_level_you_set_and_confirmed_is_the_level_you_chose() {
    let dir = confirmed("", Choice::Off);
    assert_eq!(
        in_the_window(dir.path(), None),
        NextStart {
            level: Choice::Off,
            why: Why::Chosen
        }
    );
}

#[test]
fn with_no_level_set_a_chat_takes_the_project_s_own() {
    let dir = project(FULL_PROJECT, "");
    assert_eq!(
        in_the_window(dir.path(), None),
        NextStart {
            level: Choice::On,
            why: Why::NoneChosen
        }
    );
    let dir = project("", "");
    assert_eq!(
        in_the_window(dir.path(), None),
        NextStart {
            level: Choice::Floor,
            why: Why::NoneChosen
        }
    );
}

#[test]
fn a_stricter_level_the_file_alone_says_is_never_called_the_one_you_chose() {
    // A sandboxed chat can write the file, so what it wrote is not credited to the person.
    let dir = project("", "[sandbox]\nlevel = \"on\"\n");
    assert_eq!(
        in_the_window(dir.path(), None),
        NextStart {
            level: Choice::On,
            why: Why::FileOnly
        }
    );
    assert!(!Why::FileOnly.said().contains("you chose"));
    assert!(Why::FileOnly.said().contains("you did not set in Settings"));
}

#[test]
fn a_level_waiting_for_confirm_is_never_called_yours_and_says_a_chat_can_write_it() {
    let said = Why::WaitsForConfirm(Choice::Off).said();
    assert!(!said.contains("Your level of"), "{said}");
    assert!(said.contains("A chat can write that file"), "{said}");
    assert!(said.contains("If you wrote it, press Confirm"), "{said}");
}

#[test]
fn a_manifest_a_start_is_refused_over_has_no_answer() {
    let dir = project("", "");
    std::fs::write(dir.path().join("charter.toml"), "[sandbox\n").expect("a typo");
    let locks = policy::Locks::of(dir.path());
    assert_eq!(next_start(dir.path(), None, &locks, &Made::Window), None);
    std::fs::remove_file(dir.path().join("charter.toml")).expect("gone");
    assert_eq!(next_start(dir.path(), None, &locks, &Made::Window), None);
}

#[test]
fn a_chain_the_record_cannot_read_after_a_team_s_off_you_accepted_keeps_the_sandbox() {
    let dir = project(TEAM_OFF, "");
    commit_the_team_s_off(dir.path());
    team::answer(dir.path(), true).expect("accepted");
    assert_eq!(in_the_window(dir.path(), None).level, Choice::Off);
    assert_eq!(
        next(dir.path(), None, &asked_by(&[])),
        NextStart {
            level: Choice::On,
            why: Why::ChainUnread
        }
    );
}

#[test]
fn a_persona_s_row_over_a_confirmed_off_is_named() {
    // The shape of #5207: `off` for the project, and a stricter row for the chat's persona.
    let dir = project(
        "",
        "[sandbox]\nlevel = \"off\"\n[sandbox.personas]\nsteward = \"floor\"\n",
    );
    personal::set_by_you(dir.path(), None, Some(Choice::Off)).expect("confirmed");
    assert_eq!(
        in_the_window(dir.path(), Some("steward")),
        NextStart {
            level: Choice::Floor,
            why: Why::Persona("steward".to_owned())
        }
    );
    // A chat on another persona, or none, takes the project's.
    assert_eq!(in_the_window(dir.path(), Some("qa")).why, Why::Chosen);
    assert_eq!(in_the_window(dir.path(), None).level, Choice::Off);
}

#[test]
fn an_off_the_file_alone_says_waits_for_confirm() {
    let dir = project("", "[sandbox]\nlevel = \"off\"\n");
    assert_eq!(
        in_the_window(dir.path(), None),
        NextStart {
            level: Choice::Floor,
            why: Why::WaitsForConfirm(Choice::Off)
        }
    );
    // And a persona's row the file alone says, over a project's level that is confirmed.
    let dir = project(
        "",
        "[sandbox]\nlevel = \"floor\"\n[sandbox.personas]\nsteward = \"off\"\n",
    );
    personal::set_by_you(dir.path(), None, Some(Choice::Floor)).expect("confirmed");
    assert_eq!(
        in_the_window(dir.path(), Some("steward")).why,
        Why::WaitsForConfirm(Choice::Off)
    );
}

#[test]
fn a_team_policy_holds_it_whatever_you_chose_and_says_so() {
    // Confirmed `off` before the team turned its policy on: what the policy holds is the level.
    let dir = confirmed("", Choice::Off);
    std::fs::write(
        dir.path().join("charter.toml"),
        "schema = 1\n[sandbox]\nmode = \"on\"\npolicy = \"never\"\n",
    )
    .expect("charter.toml");
    assert_eq!(
        in_the_window(dir.path(), None),
        NextStart {
            level: Choice::On,
            why: Why::Policy(team::never_said())
        }
    );
    // An `off` the file alone says is the policy's too: no Confirm would lift it.
    let dir = project(
        "[sandbox]\nmode = \"on\"\npolicy = \"never\"\n",
        "[sandbox]\nlevel = \"off\"\n",
    );
    assert_eq!(
        in_the_window(dir.path(), None).why,
        Why::Policy(team::never_said())
    );
}

#[test]
fn an_administrator_s_lock_holds_it_and_names_who() {
    let dir = confirmed("", Choice::Off);
    let locks = policy::Locks::parse(
        r#"{"sandbox": {"opt-out": false}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    assert!(locks.forbids_opt_out());
    let got = next_start(dir.path(), None, &locks, &Made::Window).expect("an answer");
    assert_eq!(got.level, Choice::On);
    let Why::Policy(said) = &got.why else {
        panic!("not the policy's: {got:?}");
    };
    assert_eq!(Some(said.clone()), locks.opt_out_refused());
}

#[test]
fn a_chat_asked_for_by_a_stricter_chat_says_that_and_a_chain_at_your_level_does_not() {
    let dir = project(
        "",
        "[sandbox]\nlevel = \"off\"\n[sandbox.personas]\nsteward = \"floor\"\n",
    );
    personal::set_by_you(dir.path(), None, Some(Choice::Off)).expect("confirmed");
    personal::set_by_you(dir.path(), Some("steward"), Some(Choice::Floor)).expect("confirmed");
    assert_eq!(
        next(dir.path(), None, &asked_by(&[Some("steward")])),
        NextStart {
            level: Choice::Floor,
            why: Why::AskedByStricter
        }
    );
    assert_eq!(
        next(dir.path(), None, &asked_by(&[Some("qa")])),
        NextStart {
            level: Choice::Off,
            why: Why::Chosen
        }
    );
}

#[test]
fn a_chain_the_record_cannot_read_whole_says_that() {
    let dir = confirmed("", Choice::Off);
    for made in [
        Made::Asked {
            how: Asking::Handoff,
            above: None,
        },
        asked_by(&[]),
    ] {
        assert_eq!(
            next(dir.path(), None, &made),
            NextStart {
                level: Choice::Floor,
                why: Why::ChainUnread
            }
        );
    }
}

#[test]
fn a_peer_s_start_on_the_host_says_your_level_is_not_read_there() {
    let dir = confirmed("", Choice::Off);
    assert_eq!(
        next(dir.path(), None, &Made::Unnamed),
        NextStart {
            level: Choice::Floor,
            why: Why::PeerStart
        }
    );
    // Stricter too: such a start takes the project's own and never your level, as `decide` does.
    let dir = confirmed("", Choice::On);
    assert_eq!(
        next(dir.path(), None, &Made::Unnamed),
        NextStart {
            level: Choice::Floor,
            why: Why::PeerStart
        }
    );
}

#[test]
fn the_answer_is_the_level_decide_starts_the_chat_at() {
    use crate::harness::Harness;
    use crate::sandbox::{Decided, Level, Person, decide_granted, grant};
    let machine = crate::sandbox::Machine {
        env: crate::secrets::Env::of(&[]),
        // Not under `/home`: on macOS that is an autofs mount, where every name that is not there
        // costs a trip to the automounter, and a start resolves hundreds of them (T1-46).
        home: Some(std::path::PathBuf::from("/nonexistent-home/op")),
        os: crate::sandbox::Os::MacOs,
    };
    let made_ways = [
        Made::Window,
        Made::Relaunch,
        Made::Unnamed,
        asked_by(&[Some("steward")]),
        asked_by(&[]),
    ];
    // Each file a start reads differently: the local file, and whether its level is confirmed.
    let cases = [
        ("", "", false),
        (FULL_NO_CACHES, "", false),
        ("", "[sandbox]\nlevel = \"on\"\n", true),
        (FULL_NO_CACHES, "[sandbox]\nlevel = \"floor\"\n", true),
        ("", "[sandbox]\nlevel = \"off\"\n", true),
        (FULL_NO_CACHES, "[sandbox]\nlevel = \"off\"\n", true),
        (FULL_NO_CACHES, "[sandbox]\nlevel = \"off\"\n", false),
        // A team's off, accepted and not.
        (TEAM_OFF, "", true),
        (TEAM_OFF, "", false),
    ];
    for (manifest, local, confirm) in cases {
        let dir = project(manifest, local);
        let root = dir.path();
        if confirm && let Some(level) = personal::of(root).level {
            personal::set_by_you(root, None, Some(level)).expect("confirmed");
        }
        if manifest == TEAM_OFF {
            commit_the_team_s_off(root);
            team::answer(root, confirm).expect("answered");
        }
        for made in &made_ways {
            let person = Person {
                opt_out: None,
                level_as: None,
                made,
            };
            let started = decide_granted(
                Harness::ClaudeCode,
                root,
                &machine,
                &|_| true,
                &person,
                None,
                &grant::Grants::default(),
            );
            let started = match started {
                Ok(Some(Decided::Sandboxed(applied))) => match applied.level() {
                    Level::Full => Choice::On,
                    Level::Floor => Choice::Floor,
                },
                Ok(Some(Decided::Unsandboxed(_))) => Choice::Off,
                other => panic!("{manifest:?} {local:?} {made:?}: {other:?}"),
            };
            assert_eq!(
                next(root, None, made).level,
                started,
                "{manifest:?} {local:?} confirmed: {confirm} {made:?}"
            );
        }
    }
}

#[test]
fn each_reason_says_where_to_change_it_or_whose_it_is() {
    assert_eq!(
        Why::Chosen.said(),
        "This is the level you chose for this project. Change it in Settings › Project › \
         Sandbox › Your level."
    );
    for why in [
        Why::Chosen,
        Why::FileOnly,
        Why::NoneChosen,
        Why::Persona("steward".to_owned()),
        Why::WaitsForConfirm(Choice::Off),
        Why::AskedByStricter,
    ] {
        assert!(why.said().contains(WHERE), "{why:?}: {}", why.said());
    }
    assert!(
        Why::Persona("steward".to_owned())
            .said()
            .contains("As steward")
    );
    assert!(Why::WaitsForConfirm(Choice::Off).said().contains("Confirm"));
    assert!(
        Why::WaitsForConfirm(Choice::Off)
            .said()
            .contains("no sandbox")
    );
    assert_eq!(
        Why::Policy("Policy says.".to_owned()).said(),
        "Policy says."
    );
    assert!(Why::ChainUnread.said().contains("chain"));
    assert!(Why::PeerStart.said().contains("host"));
}

#[test]
fn a_chat_at_its_next_level_says_only_why_and_one_behind_it_says_a_restart_applies_it() {
    let answer = NextStart {
        level: Choice::Off,
        why: Why::Chosen,
    };
    assert_eq!(answer.said(Choice::Off), Why::Chosen.said());
    assert_eq!(
        answer.said(Choice::Floor),
        format!(
            "This chat started with the floor, and its next start has no sandbox. Restart chat \
             applies it. {}",
            Why::Chosen.said()
        )
    );
    assert!(
        NextStart {
            level: Choice::On,
            why: Why::NoneChosen,
        }
        .said(Choice::Off)
        .starts_with("This chat started with no sandbox, and its next start has the full sandbox.")
    );
}
