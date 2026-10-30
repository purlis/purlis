// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::skillstudio::{ClashOutcome, StudioClash, studio};

/// An environment in which no harness has anything of its own.
fn no_harness() -> purlis_core::harness_plugin::Env<'static> {
    purlis_core::harness_plugin::Env {
        chat: &[],
        home: None,
        process: false,
    }
}

fn skill(dir: &std::path::Path, folder: &str, text: &str) {
    std::fs::create_dir_all(dir.join(folder)).unwrap();
    std::fs::write(dir.join(folder).join("SKILL.md"), text).unwrap();
}

/// A studio of one project skill, `deploy`, read as the window reads it.
fn one_skill() -> (tempfile::TempDir, Studio) {
    let tmp = tempfile::tempdir().unwrap();
    skill(
        &tmp.path().join("skills"),
        "deploy",
        "---\nname: deploy\ndescription: Deploys the app to staging, then to production.\n---\n",
    );
    let studio = studio(tmp.path(), None, true, None, None, &no_harness());
    (tmp, studio)
}

fn kinds(needs: &[NeedsYou]) -> Vec<(NeedKind, &str)> {
    needs
        .iter()
        .map(|one| (one.kind, one.title.as_str()))
        .collect()
}

#[test]
fn a_studio_with_nothing_to_decide_needs_nothing() {
    let (_tmp, studio) = one_skill();

    assert_eq!(studio.needs, []);
}

#[test]
fn a_folder_a_chat_made_is_a_change_to_review_that_accept_commits() {
    let (_tmp, mut studio) = one_skill();
    let deploy = &mut studio.skills[0];
    deploy.on = false;
    deploy.left_out = Some("a chat made it while it ran".to_owned());
    deploy.accept = Some("skills/deploy".to_owned());

    let needs = of(&studio);

    let [one] = needs.as_slice() else {
        panic!("one item: {needs:?}");
    };
    assert_eq!(one.kind, NeedKind::Review);
    assert_eq!(one.title, "deploy");
    assert_eq!(one.tag.as_deref(), Some("project"));
    assert_eq!(
        one.reason,
        "A chat made it while it ran. Chats get it once you accept it."
    );
    assert_eq!(
        one.fix,
        NeedFix::Accept {
            folder: "skills/deploy".to_owned()
        }
    );
    assert_eq!(one.fix_label, "Accept it");
}

#[test]
fn a_clash_is_one_item_per_copy_of_the_planes_whichever_harnesses_load_the_other() {
    let (_tmp, mut studio) = one_skill();
    let clash = |harness: &str| StudioClash {
        name: "deploy".to_owned(),
        level: Level::Project,
        local: false,
        harness: harness.to_owned(),
        outside: Level::Repo,
        owner: Owner::Repo {
            name: "svc".to_owned(),
        },
        outcome: ClashOutcome::BothLoad,
        said: "the repo svc has a skill of this name too, and a chat there gets both".to_owned(),
    };
    studio.clashes = vec![clash("Claude Code"), clash("Codex")];

    let needs = of(&studio);

    let [one] = needs.as_slice() else {
        panic!("one item: {needs:?}");
    };
    assert_eq!(one.kind, NeedKind::Clash);
    assert_eq!(
        one.reason,
        "The repo svc has a skill of this name too, and a chat there gets both."
    );
    assert_eq!(
        one.fix,
        NeedFix::Rename {
            name: "deploy".to_owned(),
            level: Level::Project,
            local: false,
        }
    );
    assert_eq!(one.fix_label, "Rename it…");
}

#[test]
fn a_repo_skill_of_a_built_in_skills_name_is_fixed_by_the_other_copys_switch() {
    // The built-in skill is guarded (S12): it is neither renamed nor turned off.
    let (_tmp, mut studio) = one_skill();
    studio.clashes = vec![StudioClash {
        name: "secrets".to_owned(),
        level: Level::BuiltIn,
        local: false,
        harness: "Claude Code".to_owned(),
        outside: Level::Repo,
        owner: Owner::Repo {
            name: "svc".to_owned(),
        },
        outcome: ClashOutcome::EitherOne,
        said: "the repo svc has a skill of this name too".to_owned(),
    }];

    let needs = of(&studio);

    assert_eq!(needs[0].fix, NeedFix::Harness);
    assert_eq!(needs[0].tag.as_deref(), Some("built-in"));
    assert!(
        needs[0].reason.contains("can't be renamed or turned off"),
        "{}",
        needs[0].reason
    );
    assert_eq!(needs[0].fix_label, "Show in Harness skills");
}

#[test]
fn a_lint_of_a_harness_skill_its_switch_can_leave_out_is_a_problem_in_a_skill() {
    let (_tmp, mut studio) = one_skill();
    let mut mine = studio.skills[0].clone();
    mine.name = "notes".to_owned();
    mine.level = Level::Installed;
    mine.owner = Some(Owner::You);
    mine.switch = Some(SkillSwitch::Alone);
    mine.lint = vec!["its description: it is 4 characters".to_owned()];
    let mut plugin = mine.clone();
    plugin.name = "figma:frame".to_owned();
    plugin.owner = Some(Owner::Plugin {
        name: "figma".to_owned(),
    });
    plugin.switch = Some(SkillSwitch::Not {
        why: "it comes with the plugin".to_owned(),
    });
    studio.harnesses = vec![crate::skillstudio::StudioHarness {
        harness: crate::skillstudio::HarnessKind::Claude,
        title: "Claude Code".to_owned(),
        skills: vec![mine, plugin],
        repo_left_out: Vec::new(),
    }];

    let needs = of(&studio);

    // The plugin's is read-only, so there is nothing to do about it here.
    assert_eq!(kinds(&needs), [(NeedKind::Problem, "notes")]);
    assert_eq!(needs[0].reason, "Its description: it is 4 characters.");
    assert_eq!(needs[0].tag.as_deref(), Some("yours"));
    assert_eq!(needs[0].fix, NeedFix::Harness);
}

#[test]
fn a_lint_of_the_planes_own_skill_is_a_problem_in_a_skill_that_opens_its_text() {
    // #3780: the prototype's deploy, whose step names a script it never shipped.
    let tmp = tempfile::tempdir().unwrap();
    skill(
        &tmp.path().join("skills"),
        "deploy",
        "---\nname: deploy\ndescription: Deploys the app to staging, then to production.\n---\n\n\
         1. Build it.\n2. Check it.\n3. Tag it with scripts/tag.sh.\n",
    );

    let studio = studio(tmp.path(), None, true, None, None, &no_harness());

    let [one] = studio.needs.as_slice() else {
        panic!("one item: {:?}", studio.needs);
    };
    assert_eq!(one.kind, NeedKind::Problem);
    assert_eq!(one.id, "problem:project:deploy");
    assert_eq!(one.title, "deploy");
    assert_eq!(one.tag.as_deref(), Some("project"));
    assert_eq!(
        one.reason,
        "It names scripts/tag.sh, which its folder doesn't have: a chat told to run it finds none."
    );
    assert_eq!(
        one.fix,
        NeedFix::OpenText {
            name: "deploy".to_owned(),
            level: Level::Project,
            local: false,
        }
    );
    assert_eq!(one.fix_label, "Open its text");
    assert_eq!(one.fix_does, "Opens deploy › Text in Edit.");
}

#[test]
fn a_lint_of_a_skill_no_chat_gets_or_a_built_in_one_is_no_problem_here() {
    let (_tmp, mut studio) = one_skill();
    studio.skills[0].lint = vec!["its description: it is 4 characters".to_owned()];
    studio.skills[0].on = false;
    let mut built_in = studio.skills[0].clone();
    built_in.on = true;
    built_in.level = Level::BuiltIn;
    studio.skills.push(built_in);

    assert_eq!(of(&studio), []);
}

#[test]
fn pins_no_chat_reads_offer_to_take_out_the_one_that_names_no_skill() {
    let (_tmp, mut studio) = one_skill();
    let by = SkillPinner::Workspace {
        name: "ops".to_owned(),
    };
    studio.pins_refused = Some(crate::skillchange::SkillPinsRefused {
        by: by.clone(),
        why: "\"Not A Name\" is not a skill's name".to_owned(),
        bad: Some("Not A Name".to_owned()),
        file: "workspaces/ops/workspace.json".to_owned(),
    });

    let needs = of(&studio);

    assert_eq!(kinds(&needs), [(NeedKind::Pins, "workspace ops's pins")]);
    assert_eq!(
        needs[0].fix,
        NeedFix::Unpin {
            by,
            name: "Not A Name".to_owned()
        }
    );

    studio.pins_refused.as_mut().unwrap().bad = None;
    assert_eq!(of(&studio)[0].fix, NeedFix::Library);
}

#[test]
fn skills_only_some_programs_read_are_a_tidy_up_that_moves_them() {
    let tmp = tempfile::tempdir().unwrap();
    skill(
        &tmp.path().join(".claude/skills"),
        "deploy",
        "---\nname: deploy\ndescription: Deploys.\n---\n",
    );
    skill(
        &tmp.path().join("skills"),
        "notify",
        "---\nname: notify\ndescription: Tells the team what shipped.\n---\n",
    );

    let studio = studio(tmp.path(), None, true, None, None, &no_harness());

    assert_eq!(
        kinds(&studio.needs),
        [(NeedKind::TidyUp, "1 skill only some programs read")]
    );
    assert_eq!(
        studio.needs[0].fix,
        NeedFix::MoveSkills {
            fix: "project-skills".to_owned()
        }
    );
    assert_eq!(studio.needs[0].fix_label, "Move it");
}

#[test]
fn the_old_plugin_keys_are_a_tidy_up_after_every_other_kind() {
    let (_tmp, mut studio) = one_skill();
    studio.pins_refused = Some(crate::skillchange::SkillPinsRefused {
        by: SkillPinner::Persona {
            name: "ops".to_owned(),
        },
        why: "the list holds more than 64 pins".to_owned(),
        bad: None,
        file: "personas/ops/persona.md".to_owned(),
    });
    let old = crate::oldplugins::OldHarnessPlugins {
        said: "purlis no longer turns other harnesses' plugins on or off.".to_owned(),
        files: vec!["charter.toml".to_owned()],
        removable: true,
    };

    let needs = with_old_keys(of(&studio), Some(&old));

    assert_eq!(
        kinds(&needs),
        [
            (NeedKind::Pins, "persona ops's pins"),
            (NeedKind::TidyUp, "Old plugin keys")
        ]
    );
    assert_eq!(needs[1].fix, NeedFix::RemoveOldKeys);
    assert_eq!(needs[1].tag.as_deref(), Some("charter.toml"));
    assert_eq!(with_old_keys(Vec::new(), None), []);

    // Keys no file can be proposed without are left, which asks no more.
    let kept = old_keys(&crate::oldplugins::OldHarnessPlugins {
        removable: false,
        ..old
    });
    assert_eq!(kept.fix, NeedFix::LeaveOldKeys);
    assert_eq!(kept.fix_label, "Leave them");
}

#[test]
fn every_item_of_a_studio_has_an_id_of_its_own() {
    let (_tmp, mut studio) = one_skill();
    studio.skills[0].accept = Some("skills/deploy".to_owned());
    studio.clashes = vec![StudioClash {
        name: "deploy".to_owned(),
        level: Level::Project,
        local: true,
        harness: "Claude Code".to_owned(),
        outside: Level::Installed,
        owner: Owner::You,
        outcome: ClashOutcome::BothLoad,
        said: "x".to_owned(),
    }];

    let needs = of(&studio);

    let ids: std::collections::HashSet<&str> = needs.iter().map(|one| one.id.as_str()).collect();
    assert_eq!(ids.len(), needs.len(), "{needs:?}");
}

#[test]
fn a_summary_lines_count_reads_less_and_counts_what_the_page_holds() {
    // #3781: the persona view's Skills line counts Needs you without the whole studio, and
    // counts the same items as the page: here a problem in a skill and a tidy-up.
    let tmp = tempfile::tempdir().unwrap();
    skill(
        &tmp.path().join("skills"),
        "deploy",
        "---\nname: deploy\ndescription: Deploys.\n---\n",
    );
    skill(
        &tmp.path().join(".claude/skills"),
        "notify",
        "---\nname: notify\ndescription: Tells the team what shipped.\n---\n",
    );
    let mut page = studio(tmp.path(), None, true, None, None, &no_harness());
    crate::skillstudio::with_the_projects_needs(&mut page, tmp.path(), &[]);

    let counted = crate::skillstudio::needing(tmp.path(), None, &[], None, None, &no_harness());

    assert_eq!(
        kinds(&page.needs),
        [
            (NeedKind::Problem, "deploy"),
            (NeedKind::TidyUp, "1 skill only some programs read")
        ]
    );
    assert_eq!(counted, page.needs.len());
}
