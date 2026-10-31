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
use crate::skills::{PinnedBy, Running, for_use_skill};

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
            routed: skills.routed.as_deref(),
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

    let record = skills.routed.expect("a record").record.clone();
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

#[test]
fn a_skill_pinned_since_the_chat_s_start_is_listed_and_the_rest_of_the_answer_stays() {
    let (tmp, bundle) = a_plane_of(3);
    let root = tmp.path();
    workspace_skills(root, serde_json::json!({ "router": "relevance" }));
    let fake = Fake::hiding(&["skill-01", "skill-02"]);
    let skills = started(root, &bundle, None, &as_asker(&fake));

    workspace_skills(
        root,
        serde_json::json!({ "router": "relevance", "pinned": ["skill-01"] }),
    );

    let hidden: Vec<String> = hidden_in_use_skill(&running(root, &bundle, None, &skills))
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(
        hidden,
        ["skill-02"],
        "a pin is never hidden, and the router's other hide holds"
    );
    assert_eq!(fake.asked().len(), 1, "the router is not asked again");
}

/// The skills of the chat `started` read again at its reattach, routed as `kept` says.
fn reattached(root: &Path, persona: Option<&str>, kept: Kept) -> ChatSkills {
    ChatSkills::read(
        true,
        root,
        Some(Harness::ClaudeCode),
        Some(&root.join("workspaces/smart-ide")),
        persona,
    )
    .routed_as(kept, Some(Harness::ClaudeCode))
}

#[test]
fn a_reattached_chat_routes_as_its_start_did_and_its_router_is_not_asked_again_3578() {
    let (tmp, bundle) = a_plane_of(3);
    let root = tmp.path();
    workspace_skills(root, serde_json::json!({ "router": "relevance" }));
    let first = Fake::hiding(&["skill-02"]);
    let skills = started(root, &bundle, None, &as_asker(&first));

    // Kept in the reopen record, as text, and read back at the reattach.
    let kept = Kept::of(skills.routed.as_deref());
    let text = serde_json::to_string(&kept).unwrap();
    let kept: Kept = serde_json::from_str(&text).unwrap();
    let again = reattached(root, None, kept);

    assert_eq!(again.routed, skills.routed, "the start's record and answer");
    assert_eq!(
        hidden_in_use_skill(&running(root, &bundle, None, &again)),
        [(
            "skill-02".to_owned(),
            "hidden by the router relevance: not relevant here".to_owned()
        )]
    );
    let stamp = |start: &ChatSkills| crate::skills::stamp_now(&running(root, &bundle, None, start));
    assert_eq!(stamp(&again), stamp(&skills), "the restart stamp agrees");
    assert_eq!(
        first.asked().len(),
        1,
        "the router is asked at the start alone"
    );
}

#[test]
fn a_chat_that_started_on_the_built_in_router_reattaches_on_it_whatever_was_chosen_since_3578() {
    let (tmp, bundle) = a_plane_of(3);
    let root = tmp.path();
    let fake = Fake::hiding(&["skill-02"]);
    let skills = started(root, &bundle, None, &as_asker(&fake));
    assert_eq!(skills.routed, None);
    let kept = Kept::of(skills.routed.as_deref());

    workspace_skills(root, serde_json::json!({ "router": "relevance" }));
    let again = reattached(root, None, kept);

    assert_eq!(again.routed, None);
    assert!(hidden_in_use_skill(&running(root, &bundle, None, &again)).is_empty());
    assert!(fake.asked().is_empty());
}

#[test]
fn a_kept_answer_hides_only_what_a_router_may_hide_3578() {
    // A kept answer is read back as data: one that names a pin, a built-in skill or a skill
    // outside the set hides none of them, and the rest of it holds.
    let (tmp, bundle) = a_plane_of(3);
    let root = tmp.path();
    workspace_skills(
        root,
        serde_json::json!({ "router": "relevance", "pinned": ["skill-00"] }),
    );
    let line = || purlis_skills::Line::new("not relevant here").unwrap();
    let kept = Kept::Asked(Routed {
        record: Record {
            routing: ulid::Ulid::generate().to_string(),
            router: "relevance".to_owned(),
            ms: 3,
            fell_back: None,
            error: None,
            hidden: Vec::new(),
            prompt: false,
            pointed: Vec::new(),
        },
        answer: Some(
            ["skill-00", "handoff", "elsewhere", "skill-01"]
                .into_iter()
                .map(|name| (name.to_owned(), line()))
                .collect(),
        ),
        left_out: Vec::new(),
        reads_prompts: false,
    });

    let again = reattached(root, None, kept);

    let hidden: Vec<String> = hidden_in_use_skill(&running(root, &bundle, None, &again))
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(hidden, ["skill-01"]);
}

#[test]
fn nothing_kept_is_applied_where_the_plane_s_skills_do_not_reach_the_reattached_chat_3578() {
    let (tmp, _bundle) = a_plane_of(3);
    let root = tmp.path();
    workspace_skills(root, serde_json::json!({ "router": "relevance" }));
    let kept = Kept::Asked(Routed {
        record: Record {
            routing: ulid::Ulid::generate().to_string(),
            router: "relevance".to_owned(),
            ms: 3,
            fell_back: None,
            error: None,
            hidden: Vec::new(),
            prompt: false,
            pointed: Vec::new(),
        },
        answer: Some(Vec::new()),
        left_out: Vec::new(),
        reads_prompts: false,
    });
    let off = ChatSkills::read(
        false,
        root,
        Some(Harness::ClaudeCode),
        Some(&root.join("workspaces/smart-ide")),
        None,
    )
    .routed_as(kept.clone(), Some(Harness::ClaudeCode));
    assert_eq!(off.routed, None);
    // Nor on a program purlis knows as no harness.
    let unknown = ChatSkills::read(
        true,
        root,
        None,
        Some(&root.join("workspaces/smart-ide")),
        None,
    )
    .routed_as(kept, None);
    assert_eq!(unknown.routed, None);
}

/// The arm of a router's grade (SK-15e) holding `skills` of the plane at `root`, routed by
/// `router` or by the built-in router, and the skills of its chat as its start reads them.
fn arm(root: &Path, bundle: &Path, router: Option<&str>, asker: &Arc<dyn AskRouter>) -> ChatSkills {
    let level = crate::skills::verbs::Level::project();
    let skills: Vec<_> = ["skill-00", "skill-01", "skill-02"]
        .into_iter()
        .map(|name| crate::skills::verbs::drafts::snapshot(root, &level, name).unwrap())
        .collect();
    let laid =
        crate::skills::verbs::drafts::lay_out_library_arm(root, &skills, router, &[]).unwrap();
    ChatSkills::read(
        true,
        root,
        Some(Harness::ClaudeCode),
        Some(&laid.piece),
        None,
    )
    .routed_by(Some(bundle), Some(Harness::ClaudeCode), Some(asker))
}

#[test]
fn a_router_s_grade_s_arm_is_routed_by_the_router_it_grades_3233() {
    let (tmp, bundle) = a_plane_of(3);
    let root = tmp.path();
    // Whatever the plane chooses, the arm is routed by the router under grade.
    workspace_skills(root, serde_json::json!({ "router": "another" }));
    let fake = Fake::hiding(&["skill-02"]);

    let graded = arm(root, &bundle, Some("relevance"), &as_asker(&fake));

    assert!(graded.tried);
    assert_eq!(graded.arm_router.as_deref(), Some("relevance"));
    let routed = graded.routed.as_ref().expect("asked");
    assert_eq!(routed.record.router, "relevance");
    assert_eq!(routed.record.hidden.len(), 1);
    assert_eq!(fake.asked().len(), 1);
    assert_eq!(fake.asked()[0].0, "relevance");
    // The router is asked about the library the arm holds, and nothing of the chat.
    let asked = &fake.asked()[0].1;
    let names: Vec<&str> = asked["skills"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|one| one["name"].as_str())
        .collect();
    assert_eq!(names, ["skill-00", "skill-01", "skill-02"]);
}

#[test]
fn the_built_in_router_s_arm_asks_nothing_3233() {
    let (tmp, bundle) = a_plane_of(3);
    let root = tmp.path();
    let fake = Fake::hiding(&["skill-02"]);

    let baseline = arm(root, &bundle, None, &as_asker(&fake));

    assert!(baseline.tried);
    assert_eq!(baseline.arm_router, None);
    assert_eq!(baseline.routed, None);
    assert!(fake.asked().is_empty());
}

/// The workspace `smart-ide`'s `workspace.json`, as JSON.
fn workspace_json(root: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(root.join("workspaces/smart-ide/workspace.json"))
        .unwrap_or_else(|_| "{}".to_owned());
    serde_json::from_str(&text).unwrap()
}

#[test]
fn a_workspace_s_router_is_chosen_and_its_built_in_router_is_the_key_taken_out_3692() {
    let (tmp, _bundle) = a_plane_of(1);
    let root = tmp.path();
    let ws = PinnedBy::Workspace("smart-ide".to_owned());
    let relevance = Chosen::Extension("relevance".to_owned());

    assert_eq!(choose(root, &ws, Some(&relevance)), Ok(true));
    assert_eq!(chosen(root, Some("smart-ide"), None), Ok(relevance.clone()));
    assert_eq!(written(root, &ws).as_deref(), Some("relevance"));
    assert_eq!(
        choose(root, &ws, Some(&relevance)),
        Ok(false),
        "chosen already, nothing is written"
    );

    assert_eq!(choose(root, &ws, Some(&Chosen::BuiltIn)), Ok(true));
    assert_eq!(chosen(root, Some("smart-ide"), None), Ok(Chosen::BuiltIn));
    assert_eq!(written(root, &ws), None);
    assert!(
        workspace_json(root)["settings"]["skills"]
            .get("router")
            .is_none(),
        "{}",
        workspace_json(root)
    );
    assert_eq!(
        choose(root, &ws, None),
        Ok(false),
        "the built-in router already"
    );
}

#[test]
fn a_workspace_s_router_that_is_no_string_is_replaced_by_the_choice_3692() {
    let (tmp, _bundle) = a_plane_of(1);
    let root = tmp.path();
    let ws = PinnedBy::Workspace("smart-ide".to_owned());
    workspace_skills(root, serde_json::json!({ "router": 7 }));

    assert_eq!(written(root, &ws), None);
    assert_eq!(choose(root, &ws, Some(&Chosen::BuiltIn)), Ok(true));
    assert_eq!(chosen(root, Some("smart-ide"), None), Ok(Chosen::BuiltIn));
}

#[test]
fn a_persona_s_built_in_router_is_written_and_none_hands_its_chats_to_the_workspace_s_3692() {
    let (tmp, _bundle) = a_plane_of(1);
    let root = tmp.path();
    workspace_skills(root, serde_json::json!({ "router": "relevance" }));
    let reviewer = PinnedBy::Persona("reviewer".to_owned());

    assert_eq!(choose(root, &reviewer, Some(&Chosen::BuiltIn)), Ok(true));
    assert_eq!(written(root, &reviewer).as_deref(), Some(BUILT_IN));
    assert_eq!(
        chosen(root, Some("smart-ide"), Some("reviewer")),
        Ok(Chosen::BuiltIn),
        "the persona's built-in router stands over its workspace's"
    );
    let text = std::fs::read_to_string(root.join("personas/reviewer/persona.md")).unwrap();
    assert!(text.contains("skill-router: built-in\n"), "{text}");
    assert!(
        text.contains("role: Reviews.\n"),
        "the rest of it is kept: {text}"
    );

    let recent = Chosen::Extension("recent-first".to_owned());
    assert_eq!(choose(root, &reviewer, Some(&recent)), Ok(true));
    assert_eq!(chosen(root, None, Some("reviewer")), Ok(recent.clone()));
    assert_eq!(choose(root, &reviewer, Some(&recent)), Ok(false));

    assert_eq!(choose(root, &reviewer, None), Ok(true));
    assert_eq!(written(root, &reviewer), None);
    assert_eq!(
        chosen(root, Some("smart-ide"), Some("reviewer")),
        Ok(Chosen::Extension("relevance".to_owned())),
        "its chats are routed by their workspace's router again"
    );
    let text = std::fs::read_to_string(root.join("personas/reviewer/persona.md")).unwrap();
    assert!(!text.contains("skill-router"), "{text}");
}

#[test]
fn choosing_a_router_is_refused_for_what_the_project_does_not_have_or_no_id_3692() {
    let (tmp, _bundle) = a_plane_of(1);
    let root = tmp.path();
    let why = choose(
        root,
        &PinnedBy::Workspace("nowhere".to_owned()),
        Some(&Chosen::BuiltIn),
    )
    .expect_err("no such workspace");
    assert!(why.contains("no workspace \"nowhere\""), "{why}");

    let why = choose(
        root,
        &PinnedBy::Workspace("smart-ide".to_owned()),
        Some(&Chosen::Extension("../escape".to_owned())),
    )
    .expect_err("no extension's id");
    assert!(why.contains("is not an extension's id"), "{why}");
    assert!(!root.join("workspaces/smart-ide/workspace.json").exists());

    for router in [Some(&Chosen::BuiltIn), None] {
        let why = choose(root, &PinnedBy::Persona("nobody".to_owned()), router)
            .expect_err("no such persona");
        assert!(
            why.contains("no persona") && why.contains("nobody"),
            "{why}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// On each prompt, a router that reads prompts (SK-15c, #3231; ADR 0102 §3)
// ---------------------------------------------------------------------------------------

/// `fake`, as a router whose approval said it reads prompts.
struct Reading(Arc<Fake>);

impl AskRouter for Reading {
    fn ask(
        &self,
        extension: &str,
        ask: &Ask<'_>,
    ) -> Result<serde_json::Map<String, serde_json::Value>, String> {
        self.0.ask(extension, ask)
    }

    fn reads_prompts(&self, _: &str) -> bool {
        true
    }
}

fn reading(fake: &Arc<Fake>) -> Arc<dyn AskRouter> {
    Arc::new(Reading(Arc::clone(fake)))
}

fn left_out() -> Vec<(String, String)> {
    vec![
        ("deploy".to_owned(), "Deploys the service.".to_owned()),
        ("release".to_owned(), "Cuts a release.".to_owned()),
    ]
}

fn relevance() -> Result<Chosen, String> {
    Ok(Chosen::Extension("relevance".to_owned()))
}

#[test]
fn a_router_s_pick_reaches_the_prompt_as_a_pointer_in_purlis_s_own_words() {
    let fake = Fake::answering(serde_json::json!({ "pointed": ["release"] }));

    let prompted = on_prompt(
        relevance(),
        &left_out(),
        "cut the 1.2 release",
        &reading(&fake),
        ROUTER_DEADLINE,
    )
    .expect("asked");

    let pointer = prompted.pointer.expect("a pointer");
    assert!(
        pointer.contains("- `release`: Cuts a release."),
        "{pointer}"
    );
    assert!(pointer.contains("skill_load"), "{pointer}");
    assert!(!pointer.contains("deploy"), "{pointer}");
    assert_eq!(prompted.record.fell_back, None);
    assert!(prompted.record.prompt);
    assert_eq!(prompted.record.pointed, ["release"]);
    let body = serde_json::to_string(&prompted.record).unwrap();
    assert!(
        !body.contains("1.2"),
        "the record holds nothing of the prompt: {body}"
    );

    let asked = fake.asked();
    assert_eq!(asked.len(), 1);
    let line = &asked[0].1;
    assert_eq!(line["ask"], "prompt");
    assert_eq!(line["prompt"], "cut the 1.2 release");
    assert_eq!(line["skills"][1]["line"], "Cuts a release.");
    let mut keys: Vec<&String> = line.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(
        keys,
        ["ask", "prompt", "skills"],
        "nothing else of the chat"
    );
}

#[test]
fn a_slow_router_adds_no_pointer_and_marks_the_record() {
    let slow = Arc::new(Fake {
        wait: Duration::from_secs(2),
        answer: Ok(serde_json::json!({ "pointed": ["release"] })),
        asked: Mutex::default(),
    });

    let began = Instant::now();
    let prompted = on_prompt(
        relevance(),
        &left_out(),
        "go",
        &reading(&slow),
        ROUTER_DEADLINE,
    )
    .expect("asked");
    let took = began.elapsed();

    assert!(
        took < ROUTER_DEADLINE + Duration::from_millis(400),
        "{took:?}"
    );
    assert_eq!(prompted.pointer, None);
    assert_eq!(prompted.record.fell_back, Some(FellBack::Slow));
    assert!(prompted.record.prompt);
}

#[test]
fn a_router_pointing_at_a_skill_the_list_has_is_not_taken() {
    let fake = Fake::answering(serde_json::json!({ "pointed": ["handoff"] }));

    let prompted = on_prompt(
        relevance(),
        &left_out(),
        "go",
        &reading(&fake),
        ROUTER_DEADLINE,
    )
    .expect("asked");

    assert_eq!(prompted.pointer, None);
    assert_eq!(prompted.record.fell_back, Some(FellBack::Wrong));
}

#[test]
fn nothing_is_asked_where_the_list_leaves_nothing_out_or_the_router_is_the_built_in_one() {
    let fake = Fake::answering(serde_json::json!({ "pointed": ["release"] }));
    assert_eq!(
        on_prompt(relevance(), &[], "go", &reading(&fake), ROUTER_DEADLINE),
        None
    );
    assert_eq!(
        on_prompt(
            Ok(Chosen::BuiltIn),
            &left_out(),
            "go",
            &reading(&fake),
            ROUTER_DEADLINE
        ),
        None
    );
    assert!(fake.asked().is_empty());
}

#[test]
fn every_harness_purlis_ships_has_a_prompt_hook_for_a_router_s_pointer() {
    for harness in Harness::ALL {
        assert!(harness.prompt_hook(), "{harness:?}");
    }
}

#[test]
fn a_workspace_s_built_in_router_written_out_is_already_the_built_in_router_3692() {
    let (tmp, _bundle) = a_plane_of(1);
    let root = tmp.path();
    workspace_skills(root, serde_json::json!({ "router": "built-in" }));
    let ws = PinnedBy::Workspace("smart-ide".to_owned());
    assert_eq!(choose(root, &ws, Some(&Chosen::BuiltIn)), Ok(false));
    assert_eq!(choose(root, &ws, None), Ok(false));
}

#[test]
fn a_start_routed_by_a_router_that_reads_prompts_keeps_what_its_list_leaves_out() {
    let (tmp, bundle) = a_plane_of(3);
    let root = tmp.path();
    workspace_skills(root, serde_json::json!({ "router": "relevance" }));
    let fake = Fake::hiding(&["skill-02"]);

    let read = |asker: &Arc<dyn AskRouter>| started(root, &bundle, None, asker);
    let not_reading = read(&as_asker(&fake)).routed.expect("routed");
    assert!(
        !not_reading.points(),
        "a router that reads no prompts is never handed one"
    );
    let reads = read(&reading(&fake)).routed.expect("routed");
    assert!(reads.points());
    assert_eq!(
        reads.left_out,
        [("skill-02".to_owned(), "Does thing 2.".to_owned())]
    );

    // On a prompt: asked about exactly those, and its pick reaches the pointer.
    let pick = Fake::answering(serde_json::json!({ "pointed": ["skill-02"] }));
    let prompted = reads
        .on_prompt("do thing two", &reading(&pick))
        .expect("asked");
    assert_eq!(
        prompted
            .pointer
            .as_deref()
            .map(|text| text.contains("- `skill-02`: Does thing 2.")),
        Some(true)
    );
    let asked = pick.asked();
    assert_eq!(asked[0].1["skills"].as_array().map(Vec::len), Some(1));
}

#[test]
fn a_router_slow_at_the_chat_s_start_still_routes_its_prompts() {
    let (tmp, bundle) = a_plane_of(MOST + 2);
    let root = tmp.path();
    workspace_skills(root, serde_json::json!({ "router": "relevance" }));
    let slow = Arc::new(Fake {
        wait: Duration::from_secs(2),
        answer: Ok(serde_json::json!({ "hidden": [] })),
        asked: Mutex::default(),
    });

    let routed = started(root, &bundle, None, &reading(&slow))
        .routed
        .expect("routed");

    assert_eq!(routed.record.fell_back, Some(FellBack::Slow));
    assert!(
        routed.points(),
        "what the built-in router left out is pointed at"
    );
    assert_eq!(routed.left_out.len(), MOST + 2);
}

// ---------------------------------------------------------------------------------------
// The person's switch, and what a record may hold (ADR 0102 as amended 2026-10-31)
// ---------------------------------------------------------------------------------------

/// A router answering as `fake`, whose switch is `on`.
struct Switched {
    fake: Arc<Fake>,
    on: std::sync::atomic::AtomicBool,
}

impl AskRouter for Switched {
    fn ask(
        &self,
        extension: &str,
        ask: &Ask<'_>,
    ) -> Result<serde_json::Map<String, serde_json::Value>, String> {
        self.fake.ask(extension, ask)
    }

    fn reads_prompts(&self, _: &str) -> bool {
        self.on.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[test]
fn a_router_turned_off_reads_no_further_prompt_of_a_chat_already_open() {
    let (tmp, bundle) = a_plane_of(3);
    let root = tmp.path();
    workspace_skills(root, serde_json::json!({ "router": "relevance" }));
    let start = Fake::hiding(&["skill-02"]);
    let routed = started(root, &bundle, None, &reading(&start))
        .routed
        .expect("routed");
    let fake = Fake::answering(serde_json::json!({ "pointed": ["skill-02"] }));
    let switched = Arc::new(Switched {
        fake: Arc::clone(&fake),
        on: true.into(),
    });
    let asker: Arc<dyn AskRouter> = switched.clone();

    assert!(routed.on_prompt("first", &asker).is_some());
    assert_eq!(fake.asked().len(), 1);

    switched
        .on
        .store(false, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(routed.on_prompt("second", &asker), None, "no record either");
    assert_eq!(fake.asked().len(), 1, "the router saw no second prompt");
}

#[test]
fn a_prompt_s_record_holds_none_of_what_the_router_said_back() {
    // A router could echo the prompt in an error or in an answer purlis does not take. The
    // record's reason is purlis's own, so neither reaches the event log.
    const PROMPT: &str = "the-private-words";
    let left = left_out();
    for answer in [
        serde_json::json!({ "pointed": [PROMPT] }),
        serde_json::json!({ "pointed": PROMPT }),
        serde_json::json!({ "said": PROMPT }),
    ] {
        let prompted = on_prompt(
            Ok(Chosen::Extension("relevance".to_owned())),
            &left,
            PROMPT,
            &reading(&Fake::answering(answer)),
            ROUTER_DEADLINE,
        )
        .expect("asked");
        assert_eq!(prompted.record.fell_back, Some(FellBack::Wrong));
        let written = serde_json::to_string(&prompted.record).unwrap();
        assert!(!written.contains(PROMPT), "{written}");
    }
    let failing = on_prompt(
        Ok(Chosen::Extension("relevance".to_owned())),
        &left,
        PROMPT,
        &(Arc::new(Echoes) as Arc<dyn AskRouter>),
        ROUTER_DEADLINE,
    )
    .expect("asked");
    assert_eq!(failing.record.fell_back, Some(FellBack::Failed));
    let written = serde_json::to_string(&failing.record).unwrap();
    assert!(!written.contains(PROMPT), "{written}");
}

/// A router whose error quotes the prompt it was handed.
struct Echoes;

impl AskRouter for Echoes {
    fn ask(
        &self,
        _: &str,
        ask: &Ask<'_>,
    ) -> Result<serde_json::Map<String, serde_json::Value>, String> {
        Err(serde_json::to_string(ask).unwrap_or_default())
    }

    fn reads_prompts(&self, _: &str) -> bool {
        true
    }
}

#[test]
fn a_reattached_chat_keeps_what_its_list_leaves_out_for_its_prompts() {
    let (tmp, bundle) = a_plane_of(3);
    let root = tmp.path();
    workspace_skills(root, serde_json::json!({ "router": "relevance" }));
    let fake = Fake::hiding(&["skill-02"]);
    let routed = started(root, &bundle, None, &reading(&fake))
        .routed
        .expect("routed");
    let kept = Kept::of(Some(&routed));
    let text = serde_json::to_string(&kept).unwrap();
    let back: Kept = serde_json::from_str(&text).expect("reads back");
    let back = back.routed().expect("asked");
    assert!(back.points());
    assert_eq!(back.left_out, routed.left_out);

    // An index line may run past a router's reason's 120 characters, and the record still reads.
    let Kept::Asked(mut long) = Kept::of(Some(&routed)) else {
        panic!("asked");
    };
    long.left_out[0].1 = "x".repeat(121) + "…";
    let text = serde_json::to_string(&Kept::Asked(long)).unwrap();
    assert!(serde_json::from_str::<Kept>(&text).is_ok(), "{text}");
    // A line of two lines does not.
    let bad = text.replace(&("x".repeat(121)), "a\\nb");
    assert!(serde_json::from_str::<Kept>(&bad).is_err(), "{bad}");

    // A router that may not read prompts keeps none of it.
    let plain = started(root, &bundle, None, &as_asker(&fake))
        .routed
        .expect("routed");
    assert!(plain.left_out.is_empty());
}

// ---------------------------------------------------------------------------------------
// A router's grade, with its pointer (SK-15k, #3619)
// ---------------------------------------------------------------------------------------

#[test]
fn a_router_s_arm_that_reads_prompts_points_its_trigger_test_s_prompt_as_a_real_chat_s_3619() {
    let (tmp, bundle) = a_plane_of(3);
    let root = tmp.path();
    let fake = Fake::hiding(&["skill-02"]);

    let graded = arm(root, &bundle, Some("relevance"), &reading(&fake));
    let routed = graded.routed.as_ref().expect("asked");
    assert!(routed.points(), "its chat's hook is armed");

    let pick = Fake::answering(serde_json::json!({ "pointed": ["skill-02"] }));
    let prompted = routed
        .on_prompt("a trigger test's prompt", &reading(&pick))
        .expect("asked");
    assert!(
        prompted
            .pointer
            .is_some_and(|pointer| pointer.contains("- `skill-02`:"))
    );

    // Off for this router, its arm is graded on its start's answer alone.
    let off = arm(root, &bundle, Some("relevance"), &as_asker(&fake));
    assert!(!off.routed.as_ref().expect("asked").points());

    // The built-in router's arm asks nothing, whatever the router under grade reads.
    let baseline = arm(root, &bundle, None, &reading(&fake));
    assert!(baseline.routed.is_none());
}
