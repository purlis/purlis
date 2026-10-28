// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's router (SK-15b, #3230; ADR 0102 §5, §6): which router its workspace or persona
//! chose, what an extension router is asked and within what time, what the chat lists where it
//! answers and where it does not, and the routing record of each.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::*;
use crate::harness::Harness;
use crate::hoststart::ChatSkills;
use crate::skills::{Running, for_use_skill};

/// The most skills a chat lists on every harness (ADR 0102 as amended 2026-10-21).
const MOST: usize = 30;

fn write(dir: &Path, text: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), text).unwrap();
}

/// A plane with a workspace `smart-ide`, a persona `reviewer`, one built-in skill and `count`
/// project skills granted to every workspace, `skill-00` to `skill-<count - 1>`.
fn a_plane_of(count: usize) -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::create_dir_all(root.join("workspaces/smart-ide")).unwrap();
    persona(root, "");
    for at in 0..count {
        let name = format!("skill-{at:02}");
        write(
            &root.join("skills").join(&name),
            &format!("---\nname: {name}\ndescription: Does thing {at}.\n---\n\nbody\n"),
        );
        std::fs::write(
            root.join("skills").join(format!("{name}.toml")),
            "grant = \"all\"\n",
        )
        .unwrap();
    }
    let bundle = root.join("bundle");
    write(
        &bundle.join("handoff"),
        "---\nname: handoff\ndescription: Hands off.\n---\n",
    );
    (tmp, bundle)
}

/// The persona `reviewer`, with `more` lines in its frontmatter.
fn persona(root: &Path, more: &str) {
    std::fs::create_dir_all(root.join("personas/reviewer")).unwrap();
    std::fs::write(
        root.join("personas/reviewer/persona.md"),
        format!("---\nname: reviewer\nrole: Reviews.\n{more}---\n\nReviews.\n"),
    )
    .unwrap();
}

/// The workspace `smart-ide`'s `settings.skills`.
fn workspace_skills(root: &Path, skills: serde_json::Value) {
    std::fs::write(
        root.join("workspaces/smart-ide/workspace.json"),
        serde_json::json!({ "settings": { "skills": skills } }).to_string(),
    )
    .unwrap();
}

/// A router that answers what `answer` says, after `wait`, and keeps every question it is asked.
struct Fake {
    wait: Duration,
    answer: Result<serde_json::Value, String>,
    asked: Mutex<Vec<(String, serde_json::Value)>>,
}

impl Fake {
    fn answering(answer: serde_json::Value) -> Arc<Self> {
        Arc::new(Self {
            wait: Duration::ZERO,
            answer: Ok(answer),
            asked: Mutex::default(),
        })
    }

    fn hiding(names: &[&str]) -> Arc<Self> {
        Self::answering(serde_json::json!({
            "hidden": names
                .iter()
                .map(|name| serde_json::json!({ "skill": name, "why": "not relevant here" }))
                .collect::<Vec<_>>(),
        }))
    }

    fn asked(&self) -> Vec<(String, serde_json::Value)> {
        self.asked.lock().unwrap().clone()
    }
}

impl AskRouter for Fake {
    fn ask(
        &self,
        extension: &str,
        ask: &Ask<'_>,
    ) -> Result<serde_json::Map<String, serde_json::Value>, String> {
        self.asked
            .lock()
            .unwrap()
            .push((extension.to_owned(), ask.line()));
        std::thread::sleep(self.wait);
        self.answer
            .clone()
            .map(|answer| answer.as_object().cloned().unwrap_or_default())
    }
}

fn as_asker(fake: &Arc<Fake>) -> Arc<dyn AskRouter> {
    Arc::clone(fake) as Arc<dyn AskRouter>
}

/// The skills of a Claude Code chat in `smart-ide` as `persona`, read and routed at its start
/// with `asker`.
fn started(
    root: &Path,
    bundle: &Path,
    persona: Option<&str>,
    asker: &Arc<dyn AskRouter>,
) -> ChatSkills {
    ChatSkills::read(
        true,
        root,
        Some(Harness::ClaudeCode),
        Some(&root.join("workspaces/smart-ide")),
        persona,
    )
    .routed_by(Some(bundle), Some(Harness::ClaudeCode), Some(asker))
}

fn running<'a>(
    root: &'a Path,
    bundle: &'a Path,
    persona: Option<&'a str>,
    start: &'a ChatSkills,
) -> Running<'a> {
    Running {
        plane: root,
        workspace: Some("smart-ide"),
        persona,
        harness: Harness::ClaudeCode,
        built_in: Some(bundle),
        plane_skills: true,
        start: Some(start),
    }
}

/// What Use skill… says each skill of the chat is hidden by, by name, for the hidden ones.
fn hidden_in_use_skill(chat: &Running<'_>) -> Vec<(String, String)> {
    for_use_skill(chat)
        .offered
        .iter()
        .filter_map(|one| {
            one.hidden
                .as_ref()
                .map(|hidden| (one.effective.skill.name.clone(), hidden.to_string()))
        })
        .collect()
}

#[test]
fn the_default_router_is_the_built_in_one_and_nothing_is_asked() {
    let (tmp, bundle) = a_plane_of(3);
    let root = tmp.path();
    assert_eq!(chosen(root, Some("smart-ide"), None), Ok(Chosen::BuiltIn));

    let fake = Fake::hiding(&["skill-00"]);
    let skills = started(root, &bundle, None, &as_asker(&fake));

    assert_eq!(skills.routed, None);
    assert!(fake.asked().is_empty(), "the built-in router asks no one");
}

#[test]
fn a_workspace_chooses_a_router_and_a_persona_s_choice_wins() {
    let (tmp, _bundle) = a_plane_of(1);
    let root = tmp.path();
    workspace_skills(root, serde_json::json!({ "router": "relevance" }));
    assert_eq!(
        chosen(root, Some("smart-ide"), None),
        Ok(Chosen::Extension("relevance".to_owned()))
    );
    assert_eq!(
        chosen(root, Some("smart-ide"), Some("reviewer")),
        Ok(Chosen::Extension("relevance".to_owned())),
        "a persona that chooses none takes its workspace's"
    );

    persona(root, "skill-router: built-in\n");
    assert_eq!(
        chosen(root, Some("smart-ide"), Some("reviewer")),
        Ok(Chosen::BuiltIn)
    );
    persona(root, "skill-router: recent-first\n");
    assert_eq!(
        chosen(root, Some("smart-ide"), Some("reviewer")),
        Ok(Chosen::Extension("recent-first".to_owned()))
    );
    assert_eq!(
        chosen(root, None, Some("reviewer")),
        Ok(Chosen::Extension("recent-first".to_owned())),
        "in any workspace, and at the plane's root"
    );
}

#[test]
fn a_choice_that_names_no_router_is_the_built_in_router_with_why() {
    let (tmp, bundle) = a_plane_of(2);
    let root = tmp.path();
    workspace_skills(root, serde_json::json!({ "router": "../escape" }));

    let why = chosen(root, Some("smart-ide"), None).expect_err("no router's name");
    assert!(why.contains("the workspace smart-ide"), "{why}");

    let fake = Fake::hiding(&["skill-00"]);
    let skills = started(root, &bundle, None, &as_asker(&fake));
    let record = &skills.routed.as_ref().expect("a record").record;
    assert_eq!(record.fell_back, Some(FellBack::Unchosen));
    assert!(fake.asked().is_empty());

    workspace_skills(root, serde_json::json!({ "router": 7 }));
    assert!(chosen(root, Some("smart-ide"), None).is_err());
}

#[test]
fn a_router_is_asked_its_candidates_and_the_budget_and_nothing_of_the_chat() {
    let (tmp, bundle) = a_plane_of(3);
    let root = tmp.path();
    workspace_skills(
        root,
        serde_json::json!({ "router": "relevance", "pinned": ["skill-01"] }),
    );
    let fake = Fake::hiding(&[]);

    started(root, &bundle, None, &as_asker(&fake));

    let asked = fake.asked();
    assert_eq!(asked.len(), 1, "asked once, at the chat's start");
    let (extension, line) = &asked[0];
    assert_eq!(extension, "relevance");
    assert_eq!(line["ask"], "route");
    let names: Vec<(&str, bool)> = line["skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| {
            (
                one["name"].as_str().unwrap(),
                one["pinned"].as_bool().unwrap(),
            )
        })
        .collect();
    // The built-in skill is no candidate: no router hides it.
    assert_eq!(
        names,
        [("skill-00", false), ("skill-01", true), ("skill-02", false)]
    );
    assert_eq!(line["besides"]["skills"], 1);
    assert_eq!(
        line["budget"]["skills"],
        Harness::ClaudeCode.skill_budget().skills
    );
    let mut keys: Vec<&String> = line.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(
        keys,
        ["ask", "besides", "budget", "skills"],
        "nothing else of the chat"
    );
}

#[test]
fn what_a_router_hides_is_hidden_wherever_the_chat_s_set_is_routed_with_its_line() {
    let (tmp, bundle) = a_plane_of(3);
    let root = tmp.path();
    workspace_skills(root, serde_json::json!({ "router": "relevance" }));
    let fake = Fake::hiding(&["skill-02"]);

    let skills = started(root, &bundle, None, &as_asker(&fake));

    let routed = skills.routed.as_ref().expect("routed by the extension");
    assert_eq!(routed.record.fell_back, None);
    assert_eq!(routed.record.router, "relevance");
    assert_eq!(
        routed.record.hidden,
        [RecordHidden {
            skill: "skill-02".to_owned(),
            why: "not relevant here".to_owned(),
        }]
    );
    assert!(
        routed
            .said()
            .starts_with("Routed by the router relevance in "),
        "{}",
        routed.said()
    );
    // Use skill… routes the chat's set again from its start, and so says the router's line.
    assert_eq!(
        hidden_in_use_skill(&running(root, &bundle, None, &skills)),
        [(
            "skill-02".to_owned(),
            "hidden by the router relevance: not relevant here".to_owned()
        )]
    );
    // And the folder its harness is handed leaves it out, with the index in its place.
    let folder = crate::skills::lay_out_for_chat(
        Some(crate::skills::PlaneSkills {
            plane: root,
            workspace: Some("smart-ide"),
            repo: &[],
            routed: skills.routed.as_ref(),
        }),
        None,
        Some(&bundle),
        &root.join("sets"),
        Harness::ClaudeCode,
    )
    .expect("a folder");
    let handed = folder.join(crate::skills::DIR_IN_BUNDLE);
    assert!(!handed.join("skill-02").exists());
    assert!(handed.join("skill-01").exists());
    assert!(handed.join(crate::skills::INDEX).exists());
}

#[test]
fn a_slow_router_falls_back_on_the_built_in_router_after_its_deadline() {
    let (tmp, bundle) = a_plane_of(MOST + 2);
    let root = tmp.path();
    workspace_skills(root, serde_json::json!({ "router": "relevance" }));
    let slow = Arc::new(Fake {
        wait: Duration::from_secs(2),
        answer: Ok(serde_json::json!({ "hidden": [] })),
        asked: Mutex::default(),
    });

    let began = Instant::now();
    let skills = started(root, &bundle, None, &as_asker(&slow));
    let took = began.elapsed();

    assert!(
        took < ROUTER_DEADLINE + Duration::from_millis(400),
        "the chat's start waited {took:?} for a router that takes two seconds"
    );
    let routed = skills.routed.as_ref().expect("a record");
    assert_eq!(routed.record.fell_back, Some(FellBack::Slow));
    assert!(routed.record.ms >= 90, "{}", routed.record.ms);
    assert!(
        routed.said().contains("did not answer within 100 ms"),
        "{}",
        routed.said()
    );
    // What the built-in router hides past the budget, in the record and in Use skill… alike.
    assert_eq!(routed.record.hidden.len(), MOST + 2);
    assert!(
        routed
            .record
            .hidden
            .iter()
            .all(|one| one.why == "over-budget")
    );
    let chat = running(root, &bundle, None, &skills);
    let hidden = hidden_in_use_skill(&chat);
    assert_eq!(hidden.len(), MOST + 2);
    assert!(
        hidden[0].1.starts_with("hidden by the built-in router"),
        "{hidden:?}"
    );
}

#[test]
fn a_failing_router_falls_back_with_its_error_in_the_record() {
    let (tmp, bundle) = a_plane_of(3);
    let root = tmp.path();
    workspace_skills(root, serde_json::json!({ "router": "relevance" }));
    let failing = Arc::new(Fake {
        wait: Duration::ZERO,
        answer: Err("'relevance' is not approved on this machine".to_owned()),
        asked: Mutex::default(),
    });

    let skills = started(root, &bundle, None, &as_asker(&failing));

    let record = &skills.routed.as_ref().expect("a record").record;
    assert_eq!(record.fell_back, Some(FellBack::Failed));
    assert_eq!(
        record.error.as_deref(),
        Some("'relevance' is not approved on this machine")
    );
    assert!(
        record.hidden.is_empty(),
        "the built-in router hides nothing below the budget"
    );
    assert!(hidden_in_use_skill(&running(root, &bundle, None, &skills)).is_empty());
}

#[test]
fn a_router_that_hides_a_pin_or_names_a_skill_outside_the_set_falls_back() {
    let (tmp, bundle) = a_plane_of(3);
    let root = tmp.path();
    workspace_skills(
        root,
        serde_json::json!({ "router": "relevance", "pinned": ["skill-01"] }),
    );

    for (hides, says) in [
        ("skill-01", "this chat pins"),
        ("not-a-skill-here", "not a skill of this chat it may hide"),
        ("handoff", "not a skill of this chat it may hide"),
    ] {
        let fake = Fake::hiding(&[hides]);
        let skills = started(root, &bundle, None, &as_asker(&fake));
        let record = &skills.routed.as_ref().expect("a record").record;
        assert_eq!(record.fell_back, Some(FellBack::Wrong), "{hides}");
        let error = record.error.as_deref().unwrap_or_default();
        assert!(error.contains(says), "{hides}: {error}");
        assert!(
            hidden_in_use_skill(&running(root, &bundle, None, &skills)).is_empty(),
            "{hides}: nothing hidden"
        );
    }
    // An answer that is no routing at all falls back the same way.
    let fake = Fake::answering(serde_json::json!({ "shown": ["skill-00"] }));
    let skills = started(root, &bundle, None, &as_asker(&fake));
    assert_eq!(
        skills.routed.expect("a record").record.fell_back,
        Some(FellBack::Wrong)
    );
}

#[test]
fn a_start_that_asks_no_router_falls_back_and_says_so() {
    let (tmp, bundle) = a_plane_of(2);
    let root = tmp.path();
    workspace_skills(root, serde_json::json!({ "router": "relevance" }));

    let skills = ChatSkills::read(
        true,
        root,
        Some(Harness::ClaudeCode),
        Some(&root.join("workspaces/smart-ide")),
        None,
    )
    .routed_by(Some(&bundle), Some(Harness::ClaudeCode), None);

    let record = skills.routed.expect("a record").record;
    assert_eq!(record.fell_back, Some(FellBack::Failed));
    assert!(
        record.error.unwrap_or_default().contains("asks no router"),
        "a host with no executor says why"
    );
}

#[test]
fn nothing_is_asked_where_the_plane_s_skills_do_not_reach_the_chat() {
    let (tmp, bundle) = a_plane_of(2);
    let root = tmp.path();
    workspace_skills(root, serde_json::json!({ "router": "relevance" }));
    let fake = Fake::hiding(&["skill-00"]);

    let skills = ChatSkills::read(
        false,
        root,
        Some(Harness::ClaudeCode),
        Some(&root.join("workspaces/smart-ide")),
        None,
    )
    .routed_by(
        Some(&bundle),
        Some(Harness::ClaudeCode),
        Some(&as_asker(&fake)),
    );

    assert_eq!(skills.routed, None);
    assert!(fake.asked().is_empty());
}

#[test]
fn a_record_holds_skill_names_and_lines_and_nothing_of_a_description() {
    let (tmp, bundle) = a_plane_of(3);
    let root = tmp.path();
    workspace_skills(root, serde_json::json!({ "router": "relevance" }));
    let fake = Fake::hiding(&["skill-00"]);

    let skills = started(root, &bundle, None, &as_asker(&fake));

    let body = serde_json::to_string(&skills.routed.expect("a record").record).unwrap();
    assert!(!body.contains("Does thing"), "{body}");
    let mut keys: Vec<String> =
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&body)
            .unwrap()
            .keys()
            .cloned()
            .collect();
    keys.sort();
    assert_eq!(
        keys,
        ["error", "fell_back", "hidden", "ms", "router", "routing"]
    );
}
