// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The built-in router (SK-15, #2778; ADR 0102 §2, §4, §7), on sets worked out from planes in a
//! temporary directory: everything below the budget, the pins and the index above it, and a
//! reason for every skill it hides.

use std::path::Path;

use super::*;
use crate::{Chat, HarnessSkills, Machine, PROJECT_DIR, Reason, Route, SkillSet, skill_set};

fn skill(dir: &Path, name: &str, description: &str, more: &str) {
    std::fs::create_dir_all(dir.join(name)).unwrap();
    std::fs::write(
        dir.join(name).join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n{more}---\n\nbody\n"),
    )
    .unwrap();
}

/// The set of a chat at the root of the plane at `plane` on `route`, with the built-in skills in
/// `built_in`.
fn set_on(plane: &Path, built_in: &Path, route: Route) -> SkillSet {
    skill_set(
        Some(plane),
        &Machine {
            built_in: Some(built_in),
            kept_local: Some(&|_| Ok(())),
            harness: None,
            made_since: None,
        },
        &Chat {
            workspace: None,
            persona: None,
            names: &[],
            turned_off: &[],
            harness: HarnessSkills { route },
        },
    )
}

fn set_at_the_root(plane: &Path, built_in: &Path) -> SkillSet {
    set_on(plane, built_in, Route::Plugin)
}

/// A budget of `skills` skills and plenty of characters.
fn skills(skills: usize) -> Budget {
    Budget {
        skills,
        chars: 100_000,
        per_description: None,
        lists_paths: false,
        per_line: None,
    }
}

fn names<'a>(set: impl IntoIterator<Item = &'a crate::Effective>) -> Vec<&'a str> {
    set.into_iter().map(|e| e.skill.name.as_str()).collect()
}

/// A plane with one built-in skill and the project skills `deploy`, `lint` and `release`.
fn three(tmp: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let built_in = tmp.join("bundle");
    let plane = tmp.join("plane");
    skill(&built_in, "handoff", "Hands off.", "");
    for name in ["deploy", "lint", "release"] {
        skill(&plane.join(PROJECT_DIR), name, &format!("Does {name}."), "");
    }
    (plane, built_in)
}

#[test]
fn below_the_budget_every_skill_is_listed_and_there_is_no_index() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());

    let set = set_at_the_root(&plane, &built_in).routed(skills(4), &[]);

    let routing = set.routing.as_ref().expect("routed");
    assert_eq!(routing.router, Router::BuiltIn);
    assert!(routing.hidden.is_empty());
    assert_eq!(
        names(set.listed()),
        ["deploy", "handoff", "lint", "release"]
    );
    assert_eq!(set.index(), None);
}

/// A skill from outside the plane named `name`, of `origin`, whose `owner`'s it is.
fn from_outside(name: &str, origin: crate::Origin, owner: crate::Owner) -> crate::OutsideSkill {
    crate::OutsideSkill {
        skill: crate::Skill {
            name: name.to_owned(),
            description: format!("Does {name}."),
            path: std::path::PathBuf::from(format!("/elsewhere/{name}/SKILL.md")),
        },
        root: std::path::PathBuf::from(format!("/elsewhere/{name}")),
        origin,
        id: name.to_owned(),
        owner,
        switch: crate::Switch::Alone,
        off: false,
        unread: None,
    }
}

#[test]
fn a_harness_s_own_skills_count_against_no_budget_and_a_repo_s_do_4192() {
    // ADR 0102 as amended 2026-10-31: the harness's own catalogue counts against nothing, so a
    // set routed with it beside routes as a chat's start, which never holds it (#4192). A clone's
    // repo skills are what purlis hands with the set, and count.
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    let own = (0..5).map(|at| {
        from_outside(
            &format!("mine-{at}"),
            crate::Origin::Harness,
            crate::Owner::You,
        )
    });

    let set = set_at_the_root(&plane, &built_in)
        .with_outside(Route::Plugin, own.clone())
        .routed(skills(4), &[]);

    assert!(set.routing.as_ref().expect("routed").hidden.is_empty());
    assert_eq!(set.index(), None);

    let repo = (0..2).map(|at| {
        from_outside(
            &format!("theirs-{at}"),
            crate::Origin::Repo,
            crate::Owner::Repo("api".to_owned()),
        )
    });
    let set = set_at_the_root(&plane, &built_in)
        .with_outside(Route::Plugin, own.chain(repo))
        .routed(skills(4), &[]);

    let hidden: Vec<&str> = set
        .routing
        .as_ref()
        .expect("routed")
        .hidden
        .iter()
        .map(|one| one.name.as_str())
        .collect();
    assert_eq!(
        hidden,
        ["deploy", "lint", "release"],
        "the repo's two put the plane's past the budget"
    );
}

#[test]
fn above_the_budget_the_plane_skills_are_hidden_behind_the_index_and_stay_in_the_set() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());

    let set = set_at_the_root(&plane, &built_in).routed(skills(3), &[]);

    // The built-in skill is never hidden: it comes with purlis's own plugin on every route.
    assert_eq!(names(set.listed()), ["handoff"]);
    assert_eq!(names(set.from_the_plane()), Vec::<&str>::new());
    // Hidden, never left out: the chat still has each one, for `skill_find` and `skill_load`.
    assert_eq!(
        names(&set.effective),
        ["deploy", "handoff", "lint", "release"]
    );
    assert!(set.left_out.is_empty());
    let hidden = set.hidden_by_router("deploy").expect("hidden");
    assert_eq!(hidden.why, HiddenWhy::OverBudget);
    assert_eq!(
        hidden.to_string(),
        "hidden by the built-in router: over this chat's skill budget, so it is in the index skill"
    );
    let index = set.index().expect("an index");
    assert!(index.text.starts_with(&format!("---\nname: {INDEX}\n")));
    for name in ["deploy", "lint", "release"] {
        assert!(
            index.text.contains(&format!("- `{name}`: Does {name}.")),
            "{name} in {}",
            index.text
        );
    }
    assert!(!index.text.contains("`handoff`"));
    assert!(index.text.contains("skill_load"));
}

#[test]
fn over_the_budget_by_characters_alone_hides_too() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    let budget = Budget {
        skills: 30,
        chars: 20,
        per_description: None,
        lists_paths: false,
        per_line: None,
    };

    let set = set_at_the_root(&plane, &built_in).routed(budget, &[]);

    assert_eq!(names(set.listed()), ["handoff"]);
}

#[test]
fn a_pinned_skill_is_never_hidden_and_counts_first() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());

    let set = set_at_the_root(&plane, &built_in).routed(skills(3), &["lint".to_owned()]);

    assert_eq!(names(set.listed()), ["handoff", "lint"]);
    let routing = set.routing.as_ref().expect("routed");
    assert_eq!(routing.pinned, ["lint"]);
    assert_eq!(routing.still_over, None);
    assert!(set.is_pinned("lint"));
    assert!(!set.index().expect("an index").text.contains("`lint`"));
}

#[test]
fn pins_over_the_budget_are_all_listed_and_say_so() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    let pins = ["deploy".to_owned(), "lint".to_owned(), "release".to_owned()];

    let set = set_at_the_root(&plane, &built_in).routed(skills(2), &pins);

    assert_eq!(
        names(set.listed()),
        ["deploy", "handoff", "lint", "release"]
    );
    assert_eq!(
        set.routing.as_ref().expect("routed").still_over,
        Some(StillOver::Pins)
    );
    assert_eq!(set.index(), None, "nothing is hidden, so no index");
}

#[test]
fn a_pin_naming_no_skill_of_the_set_adds_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());

    let set = set_at_the_root(&plane, &built_in).routed(skills(3), &["ghost".to_owned()]);

    assert!(set.routing.as_ref().expect("routed").pinned.is_empty());
    assert!(set.effective.iter().all(|e| e.skill.name != "ghost"));
}

#[test]
fn a_manual_only_skill_is_never_hidden_since_it_costs_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    skill(
        &plane.join(PROJECT_DIR),
        "by-hand",
        "Only when asked.",
        "disable-model-invocation: true\n",
    );

    let set = set_at_the_root(&plane, &built_in).routed(skills(3), &[]);

    assert_eq!(names(set.listed()), ["by-hand", "handoff"]);
    assert!(!set.index().expect("an index").text.contains("by-hand"));
}

#[test]
fn a_deprecated_skill_is_indexed_after_the_rest_with_its_replacement() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    std::fs::write(
        plane.join(PROJECT_DIR).join("deploy.toml"),
        "lifecycle = \"deprecated\"\nreplacement = \"release\"\n",
    )
    .unwrap();
    let set = set_at_the_root(&plane, &built_in);
    assert!(
        set.effective
            .iter()
            .any(|e| e.skill.name == "deploy" && e.deprecated.is_some()),
        "the plane marks deploy deprecated"
    );

    let index = set.routed(skills(3), &[]).index().expect("an index").text;

    let deploy = index.find("- `deploy`").expect("deploy");
    assert!(index.find("- `release`").expect("release") < deploy);
    assert!(index[deploy..].contains("deprecated: use `release` instead"));
}

#[test]
fn a_description_is_one_line_in_the_index_however_it_is_written() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    let long = "word ".repeat(60);
    skill(
        &plane.join(PROJECT_DIR),
        "wordy",
        &format!("First\t  second {long}"),
        "",
    );

    let index = set_at_the_root(&plane, &built_in)
        .routed(skills(3), &[])
        .index()
        .expect("an index")
        .text;

    let line = index
        .lines()
        .find(|line| line.starts_with("- `wordy`"))
        .expect("wordy");
    assert!(line.starts_with("- `wordy`: First second word"), "{line}");
    assert!(line.ends_with('…'), "{line}");
    assert!(line.chars().count() <= "- `wordy`: ".len() + MOST_LINE_CHARS + 1);
}

#[test]
fn a_chat_reached_only_through_purlis_tools_lists_nothing_so_nothing_is_hidden() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());

    let set = set_on(&plane, &built_in, Route::Mcp).routed(skills(1), &[]);

    assert!(set.routing.as_ref().expect("routed").hidden.is_empty());
    assert_eq!(set.index(), None);
}

#[test]
fn a_plane_skill_named_as_the_index_never_loads() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    skill(&plane.join(PROJECT_DIR), INDEX, "Mine.", "");

    let set = set_at_the_root(&plane, &built_in);

    assert!(set.effective.iter().all(|e| e.skill.name != INDEX));
    let left = set
        .left_out
        .iter()
        .find(|l| l.name == INDEX)
        .expect("left out");
    assert_eq!(left.reason, Reason::IndexName);
    assert_eq!(
        left.reason.to_string(),
        "purlis's index skill has this name, so this skill never loads: rename it"
    );
}

#[test]
fn the_index_costs_one_skill_and_its_description_of_what_is_left_listed() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    // Room for the built-in skill and one pin, and no more: with the index it is over.
    let set = set_at_the_root(&plane, &built_in).routed(skills(2), &["lint".to_owned()]);

    let routing = set.routing.as_ref().expect("routed");
    assert_eq!(
        routing.still_over,
        Some(StillOver::Pins),
        "handoff, lint and the index are three"
    );
    assert_eq!(names(set.listed()), ["handoff", "lint"]);
    assert!(set.index().is_some());
}

#[test]
fn what_no_pin_decides_over_the_budget_is_said_apart_from_the_pins() {
    // No pins at all, and more built-in skills than the budget holds.
    let tmp = tempfile::tempdir().unwrap();
    let built_in = tmp.path().join("bundle");
    let plane = tmp.path().join("plane");
    for name in ["handoff", "secrets", "theme"] {
        skill(&built_in, name, "Purlis's own.", "");
    }
    skill(&plane.join(PROJECT_DIR), "deploy", "Deploys.", "");

    let set = set_at_the_root(&plane, &built_in).routed(skills(2), &[]);

    let routing = set.routing.as_ref().expect("routed");
    assert_eq!(routing.still_over, Some(StillOver::NotHideable));
    assert!(routing.pinned.is_empty());
    assert_eq!(names(set.listed()), ["handoff", "secrets", "theme"]);
}

#[test]
fn the_index_shows_no_character_a_reader_would_not_see() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    // A description with a right-to-left override, a zero width space and a tag character.
    skill(
        &plane.join(PROJECT_DIR),
        "sneaky",
        "Safe\u{202E}txet\u{200B}word\u{E0041}end.",
        "",
    );
    // A name with them is no skill's name by the format's grammar.
    skill(&plane.join(PROJECT_DIR), "odd", "Odd one.", "");
    std::fs::write(
        plane.join(PROJECT_DIR).join("odd").join("SKILL.md"),
        "---\nname: odd\u{202E}\u{200B}\u{E0041}x\ndescription: Odd one.\n---\n\nbody\n",
    )
    .unwrap();

    let index = set_at_the_root(&plane, &built_in)
        .routed(skills(3), &[])
        .index()
        .expect("an index")
        .text;

    for hidden in ['\u{202E}', '\u{200B}', '\u{E0041}'] {
        assert!(!index.contains(hidden), "{hidden:?} in {index}");
    }
    assert!(
        index.contains("- `sneaky`: Safe⟦U+202E⟧txet⟦U+200B⟧word⟦U+E0041⟧end."),
        "{index}"
    );
    assert!(!index.contains("odd"), "{index}");
    assert!(
        index.contains("1 more skill has a name that is not a skill name"),
        "{index}"
    );
}

#[test]
fn a_repo_or_harness_skill_named_as_the_index_is_hidden_by_it() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    let folder = tmp.path().join("clone/skills-of-the-repo").join(INDEX);
    skill(folder.parent().unwrap(), INDEX, "The repo's.", "");
    let outside = |origin, owner| crate::OutsideSkill {
        skill: crate::Skill {
            name: INDEX.to_owned(),
            description: "The repo's.".to_owned(),
            path: folder.join("SKILL.md"),
        },
        root: tmp.path().join("clone"),
        origin,
        id: INDEX.to_owned(),
        owner,
        switch: crate::Switch::Alone,
        off: false,
        unread: None,
    };

    for (origin, owner) in [
        (crate::Origin::Repo, crate::Owner::Repo("app".to_owned())),
        (crate::Origin::Harness, crate::Owner::You),
    ] {
        let set = set_at_the_root(&plane, &built_in)
            .with_outside(Route::Plugin, [outside(origin, owner.clone())]);

        assert!(set.effective.iter().all(|e| e.skill.name != INDEX));
        let left = set
            .left_out
            .iter()
            .find(|l| l.name == INDEX)
            .expect("left out");
        assert_eq!(left.reason, Reason::HiddenByIndex, "{origin:?}");
        assert_eq!(
            left.reason.to_string(),
            "hidden by purlis's index skill of this name"
        );
        let briefed = set_on(&plane, &built_in, Route::Briefing)
            .with_outside(Route::Briefing, [outside(origin, owner.clone())]);
        assert!(
            briefed
                .left_out
                .iter()
                .any(|l| l.reason == Reason::HiddenByIndex)
        );

        // Where purlis's folder is one the harness discovers as its own, purlis's index is
        // `purlis-skill-index`: the copy is another skill beside it, by a name of its own.
        let discovered = set_on(&plane, &built_in, Route::Config)
            .with_outside(Route::Config, [outside(origin, owner)])
            .routed(skills(3), &[]);
        assert!(discovered.effective.iter().any(|e| e.skill.name == INDEX));
        assert!(discovered.left_out.is_empty(), "{:?}", discovered.left_out);
        assert_eq!(
            discovered.index().expect("an index").name,
            "purlis-skill-index"
        );
    }
}

#[test]
fn on_the_config_route_the_index_goes_by_a_name_of_its_own() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());

    let index = set_on(&plane, &built_in, Route::Config)
        .routed(skills(3), &[])
        .index()
        .expect("an index");
    assert_eq!(index.name, "purlis-skill-index");
    assert!(
        index.text.starts_with("---\nname: purlis-skill-index\n"),
        "its name is its folder's: {}",
        index.text
    );
    for route in [Route::Plugin, Route::Briefing] {
        let index = set_on(&plane, &built_in, route)
            .routed(skills(3), &[])
            .index()
            .expect("an index");
        assert_eq!(index.name, INDEX, "{route:?} is as it was");
    }

    // A project skill of that name is kept from it; a repo's is a clash.
    skill(&plane.join(PROJECT_DIR), "purlis-skill-index", "Mine.", "");
    let theirs = crate::OutsideSkill {
        skill: crate::Skill {
            name: "purlis-skill-index".to_owned(),
            description: "The repo's.".to_owned(),
            path: tmp.path().join("clone/purlis-skill-index/SKILL.md"),
        },
        root: tmp.path().join("clone"),
        origin: crate::Origin::Repo,
        id: "purlis-skill-index".to_owned(),
        owner: crate::Owner::Repo("app".to_owned()),
        switch: crate::Switch::Alone,
        off: false,
        unread: None,
    };
    let set = set_on(&plane, &built_in, Route::Config).with_outside(Route::Config, [theirs]);
    // The project's copy is left out; the repo's loads beside purlis's index, under one name.
    let reasons: Vec<(crate::Origin, &Reason)> = set
        .left_out
        .iter()
        .filter(|l| l.name == "purlis-skill-index")
        .map(|l| (l.origin, &l.reason))
        .collect();
    assert_eq!(
        reasons,
        [(crate::Origin::Project, &Reason::BuiltInsOwnName)]
    );
    assert!(
        set.effective
            .iter()
            .any(|e| e.skill.name == "purlis-skill-index" && e.origin == crate::Origin::Repo)
    );
    assert_eq!(
        set.clashes
            .iter()
            .map(|c| (c.name.as_str(), c.outcome))
            .collect::<Vec<_>>(),
        [("purlis-skill-index", crate::Outcome::EitherOne)]
    );
    // Elsewhere the name is the project's to use.
    let plugin = set_on(&plane, &built_in, Route::Plugin);
    assert!(
        plugin
            .effective
            .iter()
            .any(|e| e.skill.name == "purlis-skill-index")
    );
}

// ---------------------------------------------------------------------------------------
// An extension router's answer (SK-15b, #3230; ADR 0102 §1, §5, §7)
// ---------------------------------------------------------------------------------------

fn line(said: &str) -> Line {
    Line::new(said).expect("a line")
}

fn by_the_router() -> Router {
    Router::Extension("relevance".to_owned())
}

#[test]
fn a_router_s_candidates_are_the_skills_it_may_hide_with_the_pins_marked() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());

    let asked = set_at_the_root(&plane, &built_in).candidates(skills(3), &["lint".to_owned()]);

    // The built-in skill is never one: no router hides it.
    let names: Vec<(&str, bool)> = asked
        .skills
        .iter()
        .map(|one| (one.name.as_str(), one.pinned))
        .collect();
    assert_eq!(
        names,
        [("deploy", false), ("lint", true), ("release", false)]
    );
    assert_eq!(asked.skills[0].description, "Does deploy.");
    // What it cannot hide is spent first: the built-in skill.
    assert_eq!(asked.besides.skills, 1);
    assert_eq!(asked.budget.skills, 3);
}

#[test]
fn a_router_s_answer_hides_what_it_names_each_with_its_own_line() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());

    let set = set_at_the_root(&plane, &built_in)
        .routed_by(
            by_the_router(),
            skills(10),
            &[],
            &[("release".to_owned(), line("not relevant to this workspace"))],
        )
        .expect("an answer it takes");

    assert_eq!(names(set.listed()), ["deploy", "handoff", "lint"]);
    let hidden = set.hidden_by_router("release").expect("hidden");
    assert_eq!(hidden.router, by_the_router());
    assert_eq!(
        hidden.why,
        HiddenWhy::ByRouter(line("not relevant to this workspace"))
    );
    assert_eq!(
        hidden.to_string(),
        "hidden by the router relevance: not relevant to this workspace"
    );
    // Hidden, never left out, and named in the index, which says why they are not listed.
    assert_eq!(set.effective.len(), 4);
    let index = set.index().expect("an index").text;
    assert!(index.contains("- `release`"), "{index}");
    assert!(
        index.contains("This chat's router left these skills out of its list."),
        "{index}"
    );
    assert!(
        !index.contains("more skills than its list holds"),
        "{index}"
    );
}

#[test]
fn a_router_that_hides_nothing_shows_every_skill_and_no_index() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());

    let set = set_at_the_root(&plane, &built_in)
        .routed_by(by_the_router(), skills(10), &[], &[])
        .expect("an answer it takes");

    assert_eq!(set.listed().count(), 4);
    assert_eq!(set.index(), None);
    assert_eq!(set.routing.expect("routed").router, by_the_router());
}

#[test]
fn a_router_never_hides_a_pin() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());

    let refused = set_at_the_root(&plane, &built_in)
        .routed_by(
            by_the_router(),
            skills(10),
            &["lint".to_owned()],
            &[("lint".to_owned(), line("rarely used"))],
        )
        .expect_err("a pin is never hidden");

    assert!(
        refused.contains("lint") && refused.contains("pin"),
        "{refused}"
    );
}

#[test]
fn a_router_never_names_a_skill_the_set_leaves_out_or_one_no_router_hides() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    let set = set_at_the_root(&plane, &built_in);

    // Not in the set at all: a router only ever hides what the chat already has.
    let outside = set
        .clone()
        .routed_by(
            by_the_router(),
            skills(10),
            &[],
            &[("secret-skill".to_owned(), line("hidden"))],
        )
        .expect_err("outside the set");
    assert!(outside.contains("secret-skill"), "{outside}");
    // A built-in skill comes whole with purlis's plugin: no router hides it.
    let built = set
        .routed_by(
            by_the_router(),
            skills(10),
            &[],
            &[("handoff".to_owned(), line("hidden"))],
        )
        .expect_err("a built-in skill");
    assert!(built.contains("handoff"), "{built}");
}

#[test]
fn a_router_that_names_one_skill_twice_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());

    let refused = set_at_the_root(&plane, &built_in)
        .routed_by(
            by_the_router(),
            skills(10),
            &[],
            &[
                ("deploy".to_owned(), line("one")),
                ("deploy".to_owned(), line("two")),
            ],
        )
        .expect_err("twice");

    assert!(refused.contains("twice"), "{refused}");
}

#[test]
fn a_router_that_leaves_the_chat_over_its_budget_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());

    // Three skills fit; handoff and three plane skills do not, even with one of them hidden
    // behind the index.
    let refused = set_at_the_root(&plane, &built_in)
        .routed_by(
            by_the_router(),
            skills(3),
            &[],
            &[("deploy".to_owned(), line("rarely used"))],
        )
        .expect_err("still over");

    assert!(refused.contains("budget"), "{refused}");
}

#[test]
fn a_router_that_hides_all_it_may_and_is_still_over_by_the_pins_is_taken_and_says_so() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    let pins = ["deploy".to_owned(), "lint".to_owned()];

    let set = set_at_the_root(&plane, &built_in)
        .routed_by(
            by_the_router(),
            skills(3),
            &pins,
            &[("release".to_owned(), line("rarely used"))],
        )
        .expect("it hid all it may");

    assert_eq!(
        set.routing.expect("routed").still_over,
        Some(StillOver::Pins)
    );
}

#[test]
fn a_router_s_line_is_one_short_line_a_reader_sees_whole() {
    assert_eq!(line("  not relevant  ").as_str(), "not relevant");
    assert!(Line::new("").is_err());
    assert!(Line::new("   ").is_err());
    assert!(Line::new("two\nlines").is_err());
    assert!(Line::new("hidden\u{202e}gnihton").is_err());
    assert!(Line::new("zero\u{200b}width").is_err());
    assert!(Line::new(&"x".repeat(Line::MOST_CHARS + 1)).is_err());
    assert!(Line::new(&"é".repeat(Line::MOST_CHARS)).is_ok());
}

#[test]
fn routers_are_said_by_name() {
    assert_eq!(Router::BuiltIn.shown(), "the built-in router");
    assert_eq!(by_the_router().shown(), "the router relevance");
}

// ---------------------------------------------------------------------------------------
// The index read back, for a router that reads prompts (SK-15c, #3231)
// ---------------------------------------------------------------------------------------

#[test]
fn an_index_is_read_back_as_the_skills_it_names_each_with_its_line() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    let set = set_at_the_root(&plane, &built_in).routed(skills(3), &[]);
    let index = set.index().expect("an index");

    assert_eq!(
        Index::named_in(&index.text),
        [
            ("deploy".to_owned(), "Does deploy.".to_owned()),
            ("lint".to_owned(), "Does lint.".to_owned()),
            ("release".to_owned(), "Does release.".to_owned()),
        ]
    );
}

#[test]
fn an_index_read_back_takes_only_its_own_entries() {
    let text = "---\nname: skill-index\ndescription: d\n---\n\n# This chat's other skills\n\n\
                - `deploy`: Deploys.\n\
                - `Not A Name`: never one.\n\
                - not an entry\n\
                -`lint`: no space.\n\
                - `release`: Releases. (deprecated: use `ship` instead)\n";
    assert_eq!(
        Index::named_in(text),
        [
            ("deploy".to_owned(), "Deploys.".to_owned()),
            (
                "release".to_owned(),
                "Releases. (deprecated: use `ship` instead)".to_owned()
            ),
        ]
    );
}

#[test]
fn a_routed_set_spends_what_it_still_lists_and_pins_may_take_it_past_the_budget() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, built_in) = three(tmp.path());
    let set = set_at_the_root(&plane, &built_in);
    let budget = Budget {
        skills: 1,
        chars: 100_000,
        per_description: None,
        lists_paths: false,
        per_line: Some(10),
    };
    let unrouted = set.spends(budget);
    let hid_all = set.clone().routed(budget, &[]);
    let index = budget.line(INDEX, INDEX_DESCRIPTION.chars().count());
    assert!(hid_all.spends(budget) < unrouted + index);
    let name = hid_all.effective[0].skill.name.clone();
    let pinned = set.routed(budget, std::slice::from_ref(&name));
    assert!(pinned.spends(budget) > hid_all.spends(budget));
}
