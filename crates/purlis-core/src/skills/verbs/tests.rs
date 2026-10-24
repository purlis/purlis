// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam for making and changing skills (SK-7, #2770): each verb on a temporary plane, read
//! back through the skill set the studio shows, and, on a plane that is a git repository, as one
//! commit of exactly its own paths.

use std::path::Path;

use super::*;
use crate::testgit::run;
use purlis_skills::change::{BuiltInTemplates, Template, TemplateId};

/// A plane with a workspace `ops`, a persona `steward` that names `deploy`, and a project skill
/// `deploy` that another project skill `release` names. No git repository: a change is made on
/// disk and left for the next save.
fn plane() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
    std::fs::create_dir_all(root.join("personas/steward")).unwrap();
    std::fs::write(
        root.join("personas/steward/persona.md"),
        "---\nname: steward\nskills: deploy, purlis:handoff\n---\n\nRun skills:deploy daily.\n",
    )
    .unwrap();
    skill(root, "skills/deploy", "deploy", "Ships it.");
    skill(
        root,
        "skills/release",
        "release",
        "Cuts a release, then skills:deploy.",
    );
    tmp
}

fn skill(root: &Path, folder: &str, name: &str, description: &str) {
    std::fs::create_dir_all(root.join(folder)).unwrap();
    std::fs::write(
        root.join(folder).join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n\nbody\n"),
    )
    .unwrap();
}

/// What a chat at the plane root (`None`) or in a workspace gets, by name.
fn gets(root: &Path, workspace: Option<&str>) -> Vec<String> {
    crate::skills::in_the_studio(None, root, workspace, None)
        .effective
        .into_iter()
        .map(|e| e.skill.name)
        .collect()
}

/// Runs `change` on the plane at `root`, with the built-in skills in `built_in`, and answers what
/// it answered and every line it said.
fn on<T>(
    root: &Path,
    built_in: Option<&Path>,
    change: impl FnOnce(&mut Ctx<'_>) -> T,
) -> (T, Vec<Say>) {
    let mut lines = Vec::new();
    let mut say = |line: Say| lines.push(line);
    let answer = change(&mut Ctx {
        plane: root,
        built_in,
        by: By::Command,
        say: &mut say,
    });
    (answer, lines)
}

/// [`on`], with no built-in skills and its lines not kept.
fn did<T>(root: &Path, change: impl FnOnce(&mut Ctx<'_>) -> T) -> T {
    on(root, None, change).0
}

fn said(lines: &[Say], what: impl Fn(&Say) -> bool) -> bool {
    lines.iter().any(what)
}

#[test]
fn a_new_skill_from_a_template_lands_at_the_level_it_was_made_at() {
    let tmp = plane();
    let root = tmp.path();

    let folder = did(root, |cx| {
        create(
            cx,
            &Level::workspace("ops"),
            "triage",
            "Sorts new issues.",
            &BuiltInTemplates,
            &TemplateId::BuiltIn(Template::Steps),
        )
    })
    .unwrap();

    assert_eq!(folder, root.join("workspaces/ops/skills/triage"));
    assert!(gets(root, Some("ops")).contains(&"triage".to_owned()));
    assert!(!gets(root, None).contains(&"triage".to_owned()));
    let text = std::fs::read_to_string(folder.join("SKILL.md")).unwrap();
    assert!(text.contains("## Steps"), "{text}");
}

#[test]
fn a_new_skill_is_refused_for_a_bad_name_a_taken_name_or_a_built_in_name() {
    let tmp = plane();
    let root = tmp.path();
    let built_in = tempfile::tempdir().unwrap();
    skill(built_in.path(), "handoff", "handoff", "Hands off.");
    let new = |name: &str| {
        on(root, Some(built_in.path()), |cx| {
            create(
                cx,
                &Level::project(),
                name,
                "Does it.",
                &BuiltInTemplates,
                &TemplateId::BuiltIn(Template::Blank),
            )
        })
        .0
    };

    assert!(
        new("Bad Name")
            .unwrap_err()
            .contains("cannot be a skill's name")
    );
    assert!(
        new("deploy")
            .unwrap_err()
            .contains("already have one named deploy")
    );
    assert!(new("handoff").unwrap_err().contains("built-in skill"));
    assert!(!root.join("skills/handoff").exists());
    let workspace_gone = did(root, |cx| {
        create(
            cx,
            &Level::workspace("nope"),
            "x",
            "Does x.",
            &BuiltInTemplates,
            &TemplateId::BuiltIn(Template::Blank),
        )
    });
    assert!(
        workspace_gone
            .unwrap_err()
            .contains("no workspace \"nope\"")
    );
}

#[test]
fn an_edit_is_saved_only_while_the_file_is_what_the_editor_opened_and_the_lint_is_clean() {
    let tmp = plane();
    let root = tmp.path();
    let (_, was) = text_of(root, &Level::project(), "deploy").unwrap();
    let now = was.replace("body", "Ship it carefully.");

    let broken = now.replace("name: deploy", "name: shipit");
    let refused = did(root, |cx| {
        edit(cx, &Level::project(), "deploy", &was, &broken)
    });
    assert!(refused.unwrap_err().contains("Rename"));

    did(root, |cx| edit(cx, &Level::project(), "deploy", &was, &now)).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("skills/deploy/SKILL.md")).unwrap(),
        now
    );

    // The editor's text is stale now: its save is refused, and the file stays.
    let other = was.replace("body", "other");
    let stale = did(root, |cx| {
        edit(cx, &Level::project(), "deploy", &was, &other)
    });
    assert!(stale.unwrap_err().contains("changed since it was opened"));
}

#[test]
fn a_rename_follows_into_every_persona_skill_and_turn_off_that_names_it() {
    let tmp = plane();
    let root = tmp.path();
    crate::skills::grant(root, "deploy", &Grant::All).unwrap();
    crate::skills::switch(root, "ops", "deploy", false).unwrap();
    std::fs::write(
        root.join("skills/deploy.toml"),
        "grant = \"all\"\nnote = \"kept\"\n",
    )
    .unwrap();

    let (folder, lines) = on(root, None, |cx| {
        rename(cx, &Level::project(), "deploy", "ship")
    });

    assert_eq!(folder.unwrap(), root.join("skills/ship"));
    assert!(!root.join("skills/deploy").exists());
    assert_eq!(
        std::fs::read_to_string(root.join("skills/ship.toml")).unwrap(),
        "grant = \"all\"\nnote = \"kept\"\n"
    );
    assert!(gets(root, None).contains(&"ship".to_owned()));
    assert_eq!(
        std::fs::read_to_string(root.join("personas/steward/persona.md")).unwrap(),
        "---\nname: steward\nskills: ship, purlis:handoff\n---\n\nRun skills:ship daily.\n"
    );
    assert!(
        std::fs::read_to_string(root.join("skills/release/SKILL.md"))
            .unwrap()
            .contains("then skills:ship.")
    );
    assert_eq!(crate::skills::turned_off(root, "ops"), ["ship"]);
    // Still turned off in ops under its new name.
    assert!(!gets(root, Some("ops")).contains(&"ship".to_owned()));
    assert!(said(
        &lines,
        |line| matches!(line, Say::Info(text) if text.starts_with("Not saved"))
    ));
}

#[test]
fn a_rename_leaves_the_references_alone_while_another_skill_keeps_the_old_name() {
    let tmp = plane();
    let root = tmp.path();
    skill(root, "workspaces/ops/skills/deploy", "deploy", "Ships ops.");

    let (renamed, lines) = on(root, None, |cx| {
        rename(cx, &Level::workspace("ops"), "deploy", "ship-ops")
    });

    renamed.unwrap();
    assert!(
        std::fs::read_to_string(root.join("personas/steward/persona.md"))
            .unwrap()
            .contains("skills: deploy,")
    );
    assert!(said(
        &lines,
        |line| matches!(line, Say::Warn(text) if text.contains("still named deploy"))
    ));
}

#[test]
fn a_move_into_the_project_asks_for_a_grant_and_a_move_out_leaves_the_grant_and_turn_offs_behind() {
    let tmp = plane();
    let root = tmp.path();
    skill(
        root,
        "workspaces/ops/skills/triage",
        "triage",
        "Sorts issues.",
    );
    assert!(needs_grant(&Level::workspace("ops"), &Level::project()));
    assert!(!needs_grant(&Level::project(), &Level::workspace("ops")));

    let refused = did(root, |cx| {
        move_to(
            cx,
            &Level::workspace("ops"),
            "triage",
            &Level::project(),
            None,
        )
    });
    assert_eq!(refused.unwrap_err(), GRANT_NEEDED);
    assert!(root.join("workspaces/ops/skills/triage").exists());

    let grant = Grant::Only(vec!["ops".to_owned()]);
    let moved = did(root, |cx| {
        move_to(
            cx,
            &Level::workspace("ops"),
            "triage",
            &Level::project(),
            Some(&grant),
        )
    })
    .unwrap();
    assert_eq!(moved, root.join("skills/triage"));
    assert_eq!(grant_of(&moved), Ok(grant));
    assert!(gets(root, Some("ops")).contains(&"triage".to_owned()));
    crate::skills::switch(root, "ops", "triage", false).unwrap();

    let back = did(root, |cx| {
        move_to(
            cx,
            &Level::project(),
            "triage",
            &Level::workspace("ops").local(true),
            None,
        )
    })
    .unwrap();
    assert_eq!(back, root.join("workspaces/ops/skills.local/triage"));
    assert!(!beside(&back).exists(), "a workspace skill keeps no grant");
    assert!(
        crate::skills::turned_off(root, "ops").is_empty(),
        "nothing turns it off now"
    );
}

#[test]
fn a_move_never_lands_on_a_folder_already_there() {
    let tmp = plane();
    let root = tmp.path();
    skill(root, "workspaces/ops/skills/deploy", "deploy", "Ships ops.");

    let refused = did(root, |cx| {
        move_to(
            cx,
            &Level::workspace("ops"),
            "deploy",
            &Level::project(),
            Some(&Grant::All),
        )
    });

    assert!(
        refused
            .unwrap_err()
            .contains("already have one named deploy")
    );
    assert!(root.join("workspaces/ops/skills/deploy/SKILL.md").exists());
}

#[cfg(unix)]
#[test]
fn a_move_into_a_level_whose_folder_is_a_link_is_refused() {
    let tmp = plane();
    let root = tmp.path();
    let elsewhere = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), root.join("workspaces/ops/skills")).unwrap();

    let refused = did(root, |cx| {
        move_to(
            cx,
            &Level::project(),
            "deploy",
            &Level::workspace("ops"),
            None,
        )
    });

    assert!(refused.is_err());
    assert!(root.join("skills/deploy/SKILL.md").exists());
    assert_eq!(std::fs::read_dir(elsewhere.path()).unwrap().count(), 0);
}

#[test]
fn delete_says_who_uses_it_and_a_skill_git_does_not_track_comes_back_on_restore() {
    let tmp = plane();
    let root = tmp.path();
    crate::skills::grant(root, "deploy", &Grant::All).unwrap();

    let users = users(root, &Level::project(), "deploy").unwrap();
    assert_eq!(
        users,
        Users {
            root: true,
            workspaces: vec!["ops".to_owned()],
            personas: vec!["steward".to_owned()],
            skills: vec!["release".to_owned()],
        }
    );

    let kept = did(root, |cx| delete(cx, &Level::project(), "deploy")).unwrap();
    assert!(!kept.in_git());
    assert!(!root.join("skills/deploy").exists());
    assert!(!gets(root, None).contains(&"deploy".to_owned()));

    let listed = deleted(root);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(
        (listed[0].name.as_str(), &listed[0].level),
        ("deploy", &Level::project())
    );

    did(root, |cx| restore(cx, &listed[0])).unwrap();
    assert!(gets(root, None).contains(&"deploy".to_owned()));
    assert_eq!(grant_of(&root.join("skills/deploy")), Ok(Grant::All));
    assert!(deleted(root).is_empty());
    assert_eq!(
        std::fs::read_dir(crate::plane::state_dir(root).join(DELETED_DIR))
            .unwrap()
            .count(),
        0,
        "the kept folder is taken away once its skill went back"
    );
    let again = did(root, |cx| restore(cx, &listed[0]));
    assert!(again.unwrap_err().contains("cannot be restored"));
}

#[test]
fn a_deleted_skill_whose_name_is_taken_again_is_not_offered_back() {
    let tmp = plane();
    let root = tmp.path();
    did(root, |cx| delete(cx, &Level::project(), "deploy")).unwrap();
    skill(root, "skills/deploy-two", "deploy", "Ships it again.");

    assert!(deleted(root).is_empty());
}

/// A plane that is a git repository with an identity and a save mode that commits and never
/// pushes, holding [`plane`]'s files in one commit.
fn committed_plane() -> tempfile::TempDir {
    committed_plane_saving(Some("commit"))
}

/// [`committed_plane`], saving by `mode`, or told no way of saving.
fn committed_plane_saving(mode: Option<&str>) -> tempfile::TempDir {
    let tmp = plane();
    let root = tmp.path();
    let manifest = match mode {
        Some(mode) => format!("schema = 1\n\n[plane]\nmode = \"{mode}\"\n"),
        None => "schema = 1\n".to_owned(),
    };
    std::fs::write(root.join("charter.toml"), manifest).unwrap();
    std::fs::write(root.join(".gitignore"), ".charter/\n.purlis/\n").unwrap();
    run(root, &["init", "-q", "-b", "main", "."]);
    run(root, &["config", "user.name", "Fixture"]);
    run(root, &["config", "user.email", "fixture@example.invalid"]);
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "one"]);
    tmp
}

fn head_of(root: &Path) -> String {
    run(root, &["rev-parse", "HEAD"]).line().to_owned()
}

/// The paths the last commit changed.
fn last_commit(root: &Path) -> (String, Vec<String>) {
    let subject = run(root, &["log", "-1", "--format=%s"]).line().to_owned();
    let mut paths: Vec<String> = run(
        root,
        &["show", "--no-renames", "--name-only", "--format=", "HEAD"],
    )
    .out
    .lines()
    .filter(|line| !line.is_empty())
    .map(str::to_owned)
    .collect();
    paths.sort();
    (subject, paths)
}

#[test]
fn each_change_is_one_commit_of_its_own_paths_and_nothing_else() {
    let tmp = committed_plane();
    let root = tmp.path();
    // Somebody's unrelated work, which no skill change may carry.
    std::fs::write(root.join("notes.md"), "mine\n").unwrap();

    let before = head_of(root);
    did(root, |cx| {
        create(
            cx,
            &Level::project(),
            "triage",
            "Sorts issues.",
            &BuiltInTemplates,
            &TemplateId::BuiltIn(Template::Blank),
        )
    })
    .unwrap();
    assert_ne!(head_of(root), before);
    let (subject, paths) = last_commit(root);
    assert!(subject.ends_with("skill new triage"), "{subject}");
    assert_eq!(paths, ["skills/triage/SKILL.md"]);

    did(root, |cx| rename(cx, &Level::project(), "deploy", "ship")).unwrap();
    let (subject, paths) = last_commit(root);
    assert!(subject.ends_with("skill rename deploy ship"), "{subject}");
    assert_eq!(
        paths,
        [
            "personas/steward/persona.md",
            "skills/deploy/SKILL.md",
            "skills/release/SKILL.md",
            "skills/ship/SKILL.md",
        ]
    );

    did(root, |cx| grant(cx, "ship", &Grant::All)).unwrap();
    assert_eq!(last_commit(root).1, ["skills/ship.toml"]);

    // The unrelated file is still the person's, uncommitted.
    let status = run(root, &["status", "--porcelain", "--", "notes.md"]);
    assert_eq!(status.line(), "?? notes.md");
}

#[test]
fn an_edit_commits_its_skill_md_alone_and_says_what_else_it_left() {
    let tmp = committed_plane();
    let root = tmp.path();
    std::fs::write(root.join("skills/release/notes.md"), "draft\n").unwrap();
    let (_, was) = text_of(root, &Level::project(), "release").unwrap();
    let now = was.replace("body", "Cut it.");

    let (edited, lines) = on(root, None, |cx| {
        edit(cx, &Level::project(), "release", &was, &now)
    });

    edited.unwrap();
    assert_eq!(last_commit(root).1, ["skills/release/SKILL.md"]);
    assert!(said(
        &lines,
        |line| matches!(line, Say::Warn(text) if text.contains("1 other file"))
    ));
    let status = run(
        root,
        &["status", "--porcelain", "--", "skills/release/notes.md"],
    );
    assert_eq!(status.line(), "?? skills/release/notes.md");
}

#[test]
fn a_committed_skill_is_deleted_by_git_and_restored_from_the_commit_that_removed_it() {
    let tmp = committed_plane();
    let root = tmp.path();

    let kept = did(root, |cx| delete(cx, &Level::project(), "release")).unwrap();
    assert!(kept.in_git(), "{kept:?}");
    assert!(!root.join("skills/release").exists());
    assert_eq!(last_commit(root).1, ["skills/release/SKILL.md"]);

    let listed = deleted(root);
    let one = listed
        .iter()
        .find(|one| one.name == "release")
        .unwrap_or_else(|| panic!("{listed:?}"));
    assert_eq!(one.kept(), &kept);

    did(root, |cx| restore(cx, one)).unwrap();
    assert!(root.join("skills/release/SKILL.md").exists());
    let (subject, paths) = last_commit(root);
    assert!(subject.ends_with("skill restore release"), "{subject}");
    assert_eq!(paths, ["skills/release/SKILL.md"]);
    assert!(deleted(root).iter().all(|one| one.name != "release"));
}

#[test]
fn a_committed_skill_whose_commit_cannot_land_is_kept_on_this_machine() {
    // A project told no way of saving, and one that holds somebody's staged work: neither
    // commits, so the skill is kept here, and comes back from here.
    for staged_too in [false, true] {
        let tmp = committed_plane_saving(if staged_too { Some("commit") } else { None });
        let root = tmp.path();
        if staged_too {
            std::fs::write(root.join("notes.md"), "mine\n").unwrap();
            run(root, &["add", "notes.md"]);
        }
        let before = head_of(root);

        let kept = did(root, |cx| delete(cx, &Level::project(), "release")).unwrap();

        assert!(!kept.in_git(), "staged too: {staged_too}");
        assert_eq!(head_of(root), before, "staged too: {staged_too}");
        let listed = deleted(root);
        let one = listed.iter().find(|one| one.name == "release").unwrap();
        assert_eq!(one.kept(), &kept);
        did(root, |cx| restore(cx, one)).unwrap();
        assert!(
            root.join("skills/release/SKILL.md").exists(),
            "staged too: {staged_too}"
        );
    }
}

#[test]
fn a_committed_skill_with_files_git_does_not_have_is_not_deleted() {
    let tmp = committed_plane();
    let root = tmp.path();
    std::fs::write(root.join("skills/release/notes.md"), "new\n").unwrap();

    let refused = did(root, |cx| delete(cx, &Level::project(), "release"));

    assert!(refused.unwrap_err().contains("not committed yet"));
    assert!(root.join("skills/release/SKILL.md").exists());
}

#[test]
fn a_save_of_some_paths_is_refused_while_something_else_is_staged() {
    let tmp = committed_plane();
    let root = tmp.path();
    std::fs::write(root.join("notes.md"), "mine\n").unwrap();
    run(root, &["add", "notes.md"]);
    let before = head_of(root);

    let (made, lines) = on(root, None, |cx| {
        create(
            cx,
            &Level::project(),
            "triage",
            "Sorts issues.",
            &BuiltInTemplates,
            &TemplateId::BuiltIn(Template::Blank),
        )
    });

    made.unwrap();
    assert_eq!(head_of(root), before);
    assert!(said(
        &lines,
        |line| matches!(line, Say::Info(text) if text == crate::planegit::OTHERS_STAGED)
    ));
}

#[test]
fn a_folder_whose_name_git_would_read_as_a_pattern_is_named_letter_for_letter() {
    let tmp = committed_plane();
    let root = tmp.path();
    skill(root, "skills/[ab]", "pattern", "Has a bracketed folder.");
    skill(root, "skills/a", "plain-a", "Is matched by [ab].");
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "two"]);
    let (_, was) = text_of(root, &Level::project(), "pattern").unwrap();
    let now = was.replace("body", "Changed.");
    std::fs::write(root.join("skills/a/SKILL.md"), "changed elsewhere\n").unwrap();

    did(root, |cx| {
        edit(cx, &Level::project(), "pattern", &was, &now)
    })
    .unwrap();

    assert_eq!(last_commit(root).1, ["skills/[ab]/SKILL.md"]);
    let status = run(root, &["status", "--porcelain", "--", "skills/a/SKILL.md"]);
    assert_eq!(status.line(), " M skills/a/SKILL.md");
}

#[test]
fn a_delete_whose_commit_is_refused_after_the_removal_puts_the_skill_back_and_names_no_commit() {
    // The save signs, and the signer refuses: no commit is made once the files are removed.
    let tmp = committed_plane();
    let root = tmp.path();
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[plane]\nmode = \"commit\"\nsign = true\n",
    )
    .unwrap();
    run(root, &["add", "charter.toml"]);
    run(root, &["commit", "-q", "-m", "signs"]);
    run(root, &["config", "gpg.program", "false"]);
    let before = head_of(root);

    let refused = did(root, |cx| delete(cx, &Level::project(), "release"));

    assert!(refused.unwrap_err().contains("put back"));
    assert_eq!(head_of(root), before);
    assert!(root.join("skills/release/SKILL.md").exists());
    assert_eq!(
        run(root, &["status", "--porcelain"]).line(),
        "",
        "nothing is left staged or removed"
    );
    assert!(deleted(root).iter().all(|one| one.name != "release"));
}

#[test]
fn files_git_ignores_left_in_a_deleted_skills_folder_do_not_hide_it_from_restore() {
    let tmp = committed_plane();
    let root = tmp.path();
    std::fs::write(root.join(".gitignore"), ".charter/\n.purlis/\n*.log\n").unwrap();
    run(root, &["add", ".gitignore"]);
    run(root, &["commit", "-q", "-m", "ignore logs"]);
    std::fs::write(root.join("skills/release/run.log"), "output\n").unwrap();

    let kept = did(root, |cx| delete(cx, &Level::project(), "release")).unwrap();
    assert!(kept.in_git());
    assert!(
        root.join("skills/release/run.log").exists(),
        "git leaves what it ignores"
    );

    let listed = deleted(root);
    let one = listed
        .iter()
        .find(|one| one.name == "release")
        .unwrap_or_else(|| panic!("hidden by an ignored file: {listed:?}"));
    did(root, |cx| restore(cx, one)).unwrap();
    assert!(root.join("skills/release/SKILL.md").exists());
}
