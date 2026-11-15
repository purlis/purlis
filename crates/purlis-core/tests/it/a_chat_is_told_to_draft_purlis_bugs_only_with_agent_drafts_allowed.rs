// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat drafts purlis's bugs it meets, through the `purlis-report` skill, and is told so at
//! its start only while the person allows Agent drafts** (#5698, spec #5683, Q4/Q21, Q7/Q19
//! layers 2 and 3).
//!
//! Layer 1 catches what purlis can see itself. The cases only a chat can judge (the window
//! showing something false, the person saying it is broken, an agent working around purlis, a
//! purlis skill that should have started) need the skill, and a chat reaches for a skill only
//! when something in its context points at it. So the briefing names the six cases and the
//! skill, on every harness, in one line that is there only with the person's yes to Agent
//! drafts: without it a draft is refused, and the line would send the chat to a refusal.

use std::path::{Path, PathBuf};

use purlis_core::harness::{Harness, Kit, StateHooks};
use purlis_core::report::Consent;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn bundle() -> PathBuf {
    repo().join("app/src-tauri/plugin")
}

fn report_skill() -> String {
    let path = bundle().join("skills/purlis-report/SKILL.md");
    std::fs::read_to_string(&path).unwrap_or_else(|why| panic!("{}: {why}", path.display()))
}

/// `text` as one line of words: a page wraps its lines.
fn words(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn kit(plugin: &Path) -> Kit<'_> {
    Kit {
        binary: Path::new("/bin/purlis"),
        plugin: Some(plugin),
        persona: None,
        skills: None,
        own: &[],
        graded: false,
        laid_out: None,
        judging: false,
        writes_its_piece: false,
        runs_a_shell: false,
    }
}

/// What `harness`'s adapter starts a chat with: its arguments and its environment.
fn armed(harness: Harness, plugin: &Path, cwd: &Path) -> (Vec<String>, Vec<(String, String)>) {
    match harness.state_hooks(
        kit(plugin),
        Some(cwd),
        &purlis_core::harness_plugin::Chosen::new(),
        None,
    ) {
        StateHooks::ThisSessionOnly { args, env, .. } => (args, env),
        StateHooks::None => panic!("{harness:?} is armed"),
    }
}

/// A plane with one workspace, and a config home apart from it.
struct Here {
    _tmp: tempfile::TempDir,
    plane: PathBuf,
    config: PathBuf,
}

fn here() -> Here {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let plane = root.join("plane");
    std::fs::create_dir_all(plane.join("workspaces/smart-ide")).unwrap();
    std::fs::write(plane.join("charter.toml"), "schema = 1\n").unwrap();
    let config = root.join("config");
    std::fs::create_dir_all(&config).unwrap();
    Here {
        _tmp: tmp,
        plane,
        config,
    }
}

fn answered(here: &Here, agent_drafts: Option<bool>) {
    Consent {
        agent_drafts,
        asked: true,
        ..Consent::default()
    }
    .write(&here.config)
    .unwrap();
}

/// What a chat `harness`'s adapter started in the app is told at its start: the briefing every
/// harness's `sessionstart` hook prints, read with the adapter's environment, the app's hook
/// socket and this machine's config home.
fn told(here: &Here, harness: Harness) -> Vec<String> {
    let cwd = here.plane.join("workspaces/smart-ide");
    let (_, mut env) = armed(harness, &bundle(), &cwd);
    env.push((
        purlis_core::hookwire::SOCKET_ENV.to_owned(),
        "/run/purlis/hooks.sock".to_owned(),
    ));
    env.push((
        purlis_core::machine::HOME_VAR.to_owned(),
        here.config.display().to_string(),
    ));
    let lookup = move |name: &str| {
        env.iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    };
    let payload = serde_json::json!({"source": "startup"});
    purlis_core::briefing::parts(
        &purlis_core::briefing::Ask {
            root: &here.plane,
            cwd: &cwd,
            payload: &payload,
            env: &lookup,
            now: chrono::Utc::now(),
            attendance: purlis_core::dispatchunattended::Attendance::Attended,
        },
        None,
    )
}

/// The part that tells a chat about reports: it names the skill and the command. (Codex's
/// listing of its built-in skills names the skill too, as it names every one.)
fn the_report_line(parts: &[String]) -> Option<&String> {
    parts
        .iter()
        .find(|part| part.contains("`purlis-report`") && part.contains("`purlis report`"))
}

#[test]
fn with_agent_drafts_allowed_every_harness_is_told_the_six_cases_and_the_skill() {
    purlis_core::unsteered!();
    let here = here();
    answered(&here, Some(true));
    for harness in Harness::ALL {
        let parts = told(&here, harness);
        let line = the_report_line(&parts)
            .unwrap_or_else(|| panic!("{harness:?} is not told about purlis-report: {parts:#?}"));
        assert_eq!(
            line,
            purlis_core::briefing::REPORT_NOTE,
            "{harness:?}: one line, the same on every harness"
        );
    }
    let line = words(purlis_core::briefing::REPORT_NOTE);
    for case in [
        "fails",
        "blocks its own work",
        "window shows something false",
        "broken or slow",
        "working around purlis",
        "should have started",
    ] {
        assert!(
            line.contains(case),
            "the line does not name {case:?}: {line}"
        );
    }
    assert!(line.contains("`purlis report`"), "{line}");
}

#[test]
fn without_a_yes_to_agent_drafts_no_harness_is_told() {
    purlis_core::unsteered!();
    let here = here();
    // Unanswered, refused, and no file at all: each is no.
    for agent_drafts in [None, Some(false)] {
        answered(&here, agent_drafts);
        for harness in Harness::ALL {
            let parts = told(&here, harness);
            assert_eq!(
                the_report_line(&parts),
                None,
                "{harness:?}, {agent_drafts:?}"
            );
        }
    }
    std::fs::remove_file(
        here.config
            .join(purlis_core::machine::DIR)
            .join("reports.json"),
    )
    .unwrap();
    for harness in Harness::ALL {
        assert_eq!(the_report_line(&told(&here, harness)), None, "{harness:?}");
    }
}

#[test]
fn a_harness_started_outside_the_app_is_not_told_even_with_agent_drafts_allowed() {
    purlis_core::unsteered!();
    // Only the app holds a chat's draft for the person; a terminal's `purlis report` is the
    // person's own, shown before anything is filed.
    let here = here();
    answered(&here, Some(true));
    let config = here.config.display().to_string();
    let lookup = move |name: &str| (name == purlis_core::machine::HOME_VAR).then(|| config.clone());
    let payload = serde_json::json!({"source": "startup"});
    let parts = purlis_core::briefing::parts(
        &purlis_core::briefing::Ask {
            root: &here.plane,
            cwd: &here.plane.join("workspaces/smart-ide"),
            payload: &payload,
            env: &lookup,
            now: chrono::Utc::now(),
            attendance: purlis_core::dispatchunattended::Attendance::Attended,
        },
        None,
    );
    assert_eq!(the_report_line(&parts), None, "{parts:#?}");
}

#[test]
fn the_report_skill_is_a_built_in_on_every_harness() {
    purlis_core::unsteered!();
    let here = here();
    let cwd = here.plane.join("workspaces/smart-ide");
    let skills = bundle().join("skills");
    let shipped = purlis_core::skills::built_in_names(Some(&skills));
    assert!(
        shipped.iter().any(|name| name == "purlis-report"),
        "{shipped:?}"
    );
    assert!(purlis_core::skills::SHIPPED.contains(&"purlis-report"));

    // Claude Code loads the bundle as a plugin, opencode is handed its skills folder, and
    // Codex is told each built-in in its briefing.
    let (claude, _) = armed(Harness::ClaudeCode, &bundle(), &cwd);
    assert!(
        claude
            .windows(2)
            .any(|pair| pair[0] == "--plugin-dir" && Path::new(&pair[1]) == bundle()),
        "{claude:?}"
    );
    let (_, opencode) = armed(Harness::Opencode, &bundle(), &cwd);
    let config = opencode
        .iter()
        .find(|(key, _)| key == purlis_core::opencode::CONFIG_ENV)
        .map(|(_, value)| value.clone())
        .expect("opencode's config");
    assert!(config.contains(&skills.display().to_string()), "{config}");
    let (_, codex) = armed(Harness::Codex, &bundle(), &cwd);
    let lookup = |name: &str| {
        codex
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    };
    let listed = purlis_core::skills::briefed(&here.plane, Some("smart-ide"), Some(&cwd), &lookup)
        .expect("Codex is told its skills");
    assert!(listed.contains("`purlis-report`"), "{listed}");
}

#[test]
fn the_report_skill_drafts_and_never_files() {
    purlis_core::unsteered!();
    let text = report_skill();
    let (front, body) = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .expect("frontmatter");
    let description = front
        .lines()
        .find_map(|line| line.strip_prefix("description: "))
        .expect("a description");
    assert!(description.contains("purlis"), "{description}");
    assert!(description.chars().count() <= 1024, "Codex cuts at 1,024");
    let body = words(body);
    // It drafts through the command, which lands in the person's Inbox, and never files.
    assert!(
        body.contains("purlis report bug"),
        "drafts through `purlis report`"
    );
    assert!(
        !body.contains("--yes"),
        "a chat never files, so it is never shown how"
    );
    assert!(
        !body.contains("gh issue create"),
        "never filed around purlis"
    );
    // What is not a purlis bug, and what to do when unsure.
    for not_purlis in ["your project", "harness's own", "the person's own mistake"] {
        assert!(
            body.contains(not_purlis),
            "does not rule out {not_purlis:?}"
        );
    }
    assert!(body.contains("When unsure, draft"), "draft when unsure");
}

#[test]
fn a_case_six_draft_names_the_skill_the_harness_and_the_words_that_should_have_started_it() {
    purlis_core::unsteered!();
    let body = words(&report_skill());
    let case_six = body
        .split("### 6.")
        .nth(1)
        .and_then(|rest| rest.split("###").next())
        .expect("case 6 has a section of its own");
    for field in ["Skill:", "Harness:", "Words:"] {
        assert!(
            case_six.contains(field),
            "case 6's draft has no {field:?}: {case_six}"
        );
    }
}
