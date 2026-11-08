// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A correction records a miss** (#5738, spec #5714): from a chat's hook lines, a `skill.missed`
//! under its run and a miss in the plane's skill log, with the skill, harness, persona and
//! signal, and never the prompt.

use super::*;
use crate::state::Event::{Stop, UserPromptSubmit};
use std::time::Instant;

const PROMPT: &str = "/purlis:purlis-secrets put the deploy token in the vault";

/// Chat 3 begun on `harness` as the persona `steward`.
fn begun(host: &mut Recorder, plane: &Path, harness: &str) {
    host.begin(
        plane,
        3,
        RunOf {
            chat: CHAT,
            run: "01K6E8ZK6V4Q9T0N3M2B1C5D70",
        },
        Began::Start,
        &Attributes {
            persona: Some("steward".to_owned()),
            harness: Some(harness.to_owned()),
            ..Attributes::default()
        },
    )
    .unwrap();
}

/// Claude Code saying the prompt the person sent started built-in `skill`.
fn expansion(skill: &str) -> crate::hookwire::ToolCall {
    let mut expansion = tool_call(3, "userpromptexpansion", crate::hookwire::Decision::None);
    expansion.tool = None;
    expansion.call = None;
    expansion.skill = Some(Box::new(crate::hookwire::SkillStarted {
        skill: skill.to_owned(),
        run: crate::reopen::mint(),
        started_by: purlis_skills::runs::StartedBy::UseSkill,
        workspace: None,
        level: Some(purlis_skills::runs::Level {
            origin: crate::skills::Origin::BuiltIn,
            local: false,
        }),
    }));
    expansion
}

#[test]
fn a_built_in_the_person_starts_after_a_turn_without_it_is_a_skill_missed_and_a_row_with_no_prompt()
{
    let dir = tempfile::tempdir().unwrap();
    let plane = a_plane();
    let mut host = recorder(dir.path());
    begun(&mut host, plane.path(), "claude");
    host.report(plane.path(), &sent("set up the release"), Followed::No)
        .unwrap();
    host.report(plane.path(), &report(3, Stop), Followed::No)
        .unwrap();
    host.report(plane.path(), &sent(PROMPT), Followed::No)
        .unwrap();
    host.tool(
        plane.path(),
        &expansion("purlis:purlis-secrets"),
        Instant::now(),
    )
    .unwrap();

    let events = read(dir.path()).unwrap();
    let missed: Vec<_> = events
        .iter()
        .filter(|event| event.kind == "skill.missed")
        .collect();
    let [missed] = &missed[..] else {
        panic!("one skill.missed: {events:#?}");
    };
    assert_eq!(missed.body["missed"], "purlis-secrets");
    assert_eq!(missed.body["signal"], "correction");
    assert_eq!(missed.run.as_deref(), Some("01K6E8ZK6V4Q9T0N3M2B1C5D70"));

    let [miss] = &crate::skilluse::misses(plane.path())[..] else {
        panic!("one miss logged");
    };
    assert_eq!(miss.skill, "purlis-secrets");
    assert_eq!(miss.harness.as_deref(), Some("claude"));
    assert_eq!(miss.persona.as_deref(), Some("steward"));
    assert_eq!(miss.signal, crate::skilluse::Signal::Correction);
    assert!(
        crate::skilluse::runs(plane.path()).is_empty(),
        "a slash command typed by hand opens no run"
    );

    let log = std::fs::read_dir(plane.path().join("personas/_skills"))
        .unwrap()
        .map(|file| std::fs::read_to_string(file.unwrap().path()).unwrap())
        .collect::<String>();
    let events = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
    assert!(events.contains("skill.missed"));
    for word in ["deploy", "token", "vault"] {
        assert!(!log.contains(word), "the prompt in the skill log: {log}");
        assert!(!events.contains(word), "the prompt in the event log");
    }
}

#[test]
fn a_chats_first_prompt_and_a_skill_its_turn_before_started_record_no_miss() {
    let dir = tempfile::tempdir().unwrap();
    let plane = a_plane();
    let mut host = recorder(dir.path());
    begun(&mut host, plane.path(), "claude");
    host.report(plane.path(), &sent(PROMPT), Followed::No)
        .unwrap();
    host.tool(
        plane.path(),
        &expansion("purlis:purlis-secrets"),
        Instant::now(),
    )
    .unwrap();
    host.report(plane.path(), &report(3, Stop), Followed::No)
        .unwrap();

    host.report(plane.path(), &report(3, UserPromptSubmit), Followed::No)
        .unwrap();
    let mut agent = starting(3, "posttooluse-skill", "toolu_a", "purlis:purlis-secrets");
    agent.skill.as_mut().unwrap().level = expansion("x").skill.unwrap().level;
    host.tool(plane.path(), &agent, Instant::now()).unwrap();
    host.report(plane.path(), &report(3, Stop), Followed::No)
        .unwrap();
    host.report(plane.path(), &sent(PROMPT), Followed::No)
        .unwrap();
    host.tool(
        plane.path(),
        &expansion("purlis:purlis-secrets"),
        Instant::now(),
    )
    .unwrap();

    let events = read(dir.path()).unwrap();
    assert!(
        events.iter().all(|event| event.kind != "skill.missed"),
        "{events:#?}"
    );
    assert!(crate::skilluse::misses(plane.path()).is_empty());
}

#[test]
fn a_built_in_use_skill_typed_into_codex_or_opencode_after_a_turn_without_it_is_a_miss() {
    for harness in ["codex", "opencode"] {
        let dir = tempfile::tempdir().unwrap();
        let plane = a_plane();
        let mut host = recorder(dir.path());
        begun(&mut host, plane.path(), harness);
        host.report(plane.path(), &sent("set up the release"), Followed::No)
            .unwrap();
        host.report(plane.path(), &report(3, Stop), Followed::No)
            .unwrap();
        let mut typed = typed_skill(
            "purlis-handoff",
            "$purlis-handoff",
            crate::skillspan::Confirmed::ByPrompt,
        );
        typed.level = Some(purlis_skills::runs::Level {
            origin: crate::skills::Origin::BuiltIn,
            local: false,
        });
        host.apply(super::super::Entry::SkillTyped {
            plane: plane.path().to_path_buf(),
            number: 3,
            typed,
        })
        .unwrap();
        host.report(plane.path(), &sent("$purlis-handoff"), Followed::No)
            .unwrap();

        let [miss] = &crate::skilluse::misses(plane.path())[..] else {
            panic!("{harness}: one miss");
        };
        assert_eq!(miss.skill, "purlis-handoff", "{harness}");
        assert_eq!(miss.harness.as_deref(), Some(harness));
        assert_eq!(crate::skilluse::runs(plane.path()).len(), 1, "{harness}");
    }
}
