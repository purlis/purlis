// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam for making and changing skills (SK-7, #2770): each verb on a temporary plane, read
//! back through the skill set the studio shows, and, on a plane that is a git repository, as one
//! commit of exactly its own paths.

use std::path::{Path, PathBuf};

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
            persona: None,
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

/// The file beside a deleted skill's folder that cannot be put away (a link in its place): the
/// folder comes back, nothing is kept, and the delete says it was not deleted (SK-48).
#[cfg(unix)]
#[test]
fn a_delete_whose_file_beside_the_folder_cannot_move_puts_the_folder_back_and_refuses() {
    let tmp = plane();
    let root = tmp.path();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("deploy.toml"), "grant = \"all\"\n").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("deploy.toml"),
        root.join("skills/deploy.toml"),
    )
    .unwrap();

    let refused = did(root, |cx| delete(cx, &Level::project(), "deploy")).unwrap_err();

    assert_eq!(
        refused,
        "deploy was not deleted: skills/deploy.toml could not go with it (it is not a plain \
         folder or file, so purlis does not move it), so nothing was moved"
    );
    assert!(root.join("skills/deploy/SKILL.md").is_file());
    assert!(root.join("skills/deploy.toml").is_symlink());
    assert!(deleted(root).is_empty());
    assert_eq!(
        std::fs::read_dir(crate::plane::state_dir(root).join(DELETED_DIR))
            .map(|kept| kept.count())
            .unwrap_or(0),
        0,
        "nothing is kept of a delete that did not happen"
    );
}

/// A delete whose file beside the folder cannot move and whose folder's way back is taken: the
/// folder is kept, the delete says so and where to restore it from, and the file left in the
/// project does not hide it from Restore (SK-48).
#[cfg(unix)]
#[test]
fn a_delete_stranded_in_its_kept_folder_says_so_and_is_restored_from_there() {
    let tmp = plane();
    let root = tmp.path();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("deploy.toml"), "grant = \"all\"\n").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("deploy.toml"),
        root.join("skills/deploy.toml"),
    )
    .unwrap();
    let way_back = root.join("skills/deploy");
    let hook = purlis_skills::moving::hook::set(move |moment, _, to| {
        if moment == purlis_skills::moving::hook::Moment::BeforeRename && to == way_back {
            std::fs::write(to, "in the way\n").unwrap();
        }
    });

    let stranded = did(root, |cx| delete(cx, &Level::project(), "deploy")).unwrap_err();
    drop(hook);

    assert!(
        stranded.starts_with("deploy was deleted only in part: its folder was moved to "),
        "{stranded}"
    );
    assert!(
        stranded.ends_with(
            "and skills/deploy.toml was not moved (it is not a plain folder or file, so purlis \
             does not move it). It is in this machine's deleted skills: restore it from there"
        ),
        "{stranded}"
    );
    std::fs::remove_file(root.join("skills/deploy")).unwrap();
    let listed = deleted(root);
    assert_eq!(listed.len(), 1, "{listed:?}");

    did(root, |cx| restore(cx, &listed[0])).unwrap();

    assert!(root.join("skills/deploy/SKILL.md").is_file());
    assert!(root.join("skills/deploy.toml").is_symlink());
    assert!(deleted(root).is_empty());
}

/// The kept file beside a deleted skill's folder that cannot come back (a link in its place):
/// the folder stays kept, the skill is still offered back, and the restore says it was not
/// restored, never a skill back without its grant (SK-48).
#[cfg(unix)]
#[test]
fn a_restore_whose_file_beside_the_folder_cannot_move_keeps_the_folder_and_refuses() {
    let tmp = plane();
    let root = tmp.path();
    crate::skills::grant(root, "deploy", &Grant::All).unwrap();
    did(root, |cx| delete(cx, &Level::project(), "deploy")).unwrap();
    let kept_dir = std::fs::read_dir(crate::plane::state_dir(root).join(DELETED_DIR))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let kept_toml = kept_dir.join("deploy.toml");
    let outside = tempfile::tempdir().unwrap();
    std::fs::rename(&kept_toml, outside.path().join("deploy.toml")).unwrap();
    std::os::unix::fs::symlink(outside.path().join("deploy.toml"), &kept_toml).unwrap();
    let listed = deleted(root);

    let refused = did(root, |cx| restore(cx, &listed[0])).unwrap_err();

    assert!(
        refused.starts_with("deploy was not restored: "),
        "{refused}"
    );
    assert!(!root.join("skills/deploy").exists());
    assert!(kept_dir.join("deploy/SKILL.md").is_file());
    assert_eq!(deleted(root), listed);
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

// A delete cut short (SK-35, #3013): the app dies between the removal and its commit.

/// Deletes the project skill `release` in the plane at `root`, cut short at `step` as a crash
/// would: nothing after it runs, and what the delete held open is let go.
fn delete_cut_short_at(root: &Path, step: cut::Step) {
    let _hook = cut::set(move |at| {
        if at == step {
            panic!("cut short at {at:?}");
        }
    });
    let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        did(root, |cx| delete(cx, &Level::project(), "release"))
    }));
    assert!(crashed.is_err(), "the delete was not cut short at {step:?}");
}

/// The notes of deletes by git that ran, and were not seen to end.
fn notes_left(root: &Path) -> Vec<String> {
    std::fs::read_dir(crate::plane::state_dir(root).join(DELETING_DIR))
        .map(|dir| {
            dir.filter_map(Result::ok)
                .map(|one| one.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_delete_cut_short_before_its_commit_is_finished_at_the_next_start() {
    for step in [cut::Step::Removed, cut::Step::Unstaged] {
        let tmp = committed_plane();
        let root = tmp.path();
        let before = head_of(root);
        delete_cut_short_at(root, step);
        // What the crash left: the skill is in neither the live list nor Restore.
        assert!(!root.join("skills/release").exists(), "{step:?}");
        assert!(
            !gets(root, None).contains(&"release".to_owned()),
            "{step:?}"
        );
        assert!(deleted(root).iter().all(|one| one.name != "release"));

        let ((), lines) = on(root, None, interrupted);

        assert_eq!(lines.len(), 1, "{step:?}: one line, {lines:?}");
        assert!(
            matches!(&lines[0], Say::Info(text) if text.contains("release") && text.contains("finished")),
            "{step:?}: {lines:?}"
        );
        assert_ne!(head_of(root), before, "{step:?}");
        let (subject, paths) = last_commit(root);
        assert!(subject.ends_with("skill rm release"), "{step:?}: {subject}");
        assert_eq!(paths, ["skills/release/SKILL.md"], "{step:?}");
        assert_eq!(run(root, &["status", "--porcelain"]).line(), "", "{step:?}");
        let listed = deleted(root);
        let one = listed
            .iter()
            .find(|one| one.name == "release")
            .unwrap_or_else(|| panic!("{step:?}: not offered back: {listed:?}"));
        assert!(one.kept().in_git(), "{step:?}");
        assert!(notes_left(root).is_empty(), "{step:?}");
        // Once is enough: the next start finds nothing.
        assert!(on(root, None, interrupted).1.is_empty(), "{step:?}");
    }
}

#[test]
fn a_delete_cut_short_whose_commit_cannot_land_now_is_put_back_and_unstaged() {
    let tmp = committed_plane();
    let root = tmp.path();
    let before = head_of(root);
    delete_cut_short_at(root, cut::Step::Removed);
    // Somebody's own work staged since: the delete's commit cannot land now.
    std::fs::write(root.join("notes.md"), "mine\n").unwrap();
    run(root, &["add", "notes.md"]);

    let ((), lines) = on(root, None, interrupted);

    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        matches!(&lines[0], Say::Info(text) if text.contains("release") && text.contains("put back")),
        "{lines:?}"
    );
    assert_eq!(head_of(root), before);
    assert!(root.join("skills/release/SKILL.md").exists());
    assert!(gets(root, None).contains(&"release".to_owned()));
    assert_eq!(
        run(root, &["status", "--porcelain"]).line(),
        "A  notes.md",
        "the skill's removal is never left staged, and the person's work is left as it was"
    );
    assert!(notes_left(root).is_empty());
}

#[test]
fn a_delete_cut_short_after_its_commit_says_nothing_and_takes_its_note_away() {
    let tmp = committed_plane();
    let root = tmp.path();
    delete_cut_short_at(root, cut::Step::Committed);
    let after = head_of(root);
    assert_eq!(notes_left(root).len(), 1, "the crash left its note");

    let ((), lines) = on(root, None, interrupted);

    assert!(lines.is_empty(), "{lines:?}");
    assert_eq!(head_of(root), after);
    assert!(notes_left(root).is_empty());
    assert!(deleted(root).iter().any(|one| one.name == "release"));
}

#[test]
fn a_delete_still_running_is_never_finished_by_another_start() {
    let tmp = committed_plane();
    let root = tmp.path().to_path_buf();
    let heard = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let _hook = cut::set({
        let (root, heard) = (root.clone(), heard.clone());
        move |at| {
            if at == cut::Step::Unstaged {
                // Another start, while this delete runs: the window's studio, say.
                heard.borrow_mut().extend(on(&root, None, interrupted).1);
            }
        }
    });

    let kept = did(&root, |cx| delete(cx, &Level::project(), "release")).unwrap();

    assert!(heard.borrow().is_empty(), "{:?}", heard.borrow());
    assert!(kept.in_git());
    assert_eq!(last_commit(&root).1, ["skills/release/SKILL.md"]);
    assert!(notes_left(&root).is_empty());
}

#[test]
fn a_delete_neither_committed_nor_put_back_keeps_its_note_and_the_next_start_puts_it_back() {
    // The save signs and the signer refuses, so no commit; and git's index is locked from the
    // removal on, so the files cannot be put back either.
    let tmp = committed_plane();
    let root = tmp.path().to_path_buf();
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[plane]\nmode = \"commit\"\nsign = true\n",
    )
    .unwrap();
    run(&root, &["add", "charter.toml"]);
    run(&root, &["commit", "-q", "-m", "signs"]);
    run(&root, &["config", "gpg.program", "false"]);
    let before = head_of(&root);
    let locked = root.join(".git/index.lock");
    let refused = {
        let _hook = cut::set({
            let locked = locked.clone();
            move |at| {
                if at == cut::Step::Unstaged {
                    std::fs::write(&locked, "").unwrap();
                }
            }
        });
        did(&root, |cx| delete(cx, &Level::project(), "release"))
    };

    let why = refused.unwrap_err();
    assert!(why.contains("tries again"), "{why}");
    assert!(!root.join("skills/release/SKILL.md").exists());
    assert_eq!(
        notes_left(&root).len(),
        1,
        "the note stays for the next start"
    );

    std::fs::remove_file(&locked).unwrap();
    let ((), lines) = on(&root, None, interrupted);

    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        matches!(&lines[0], Say::Info(text) if text.contains("put back")),
        "{lines:?}"
    );
    assert_eq!(head_of(&root), before);
    assert!(root.join("skills/release/SKILL.md").exists());
    assert_eq!(run(&root, &["status", "--porcelain"]).line(), "");
    assert!(notes_left(&root).is_empty());
}

#[test]
fn a_start_that_can_neither_finish_nor_put_back_warns_and_keeps_the_note() {
    let tmp = committed_plane();
    let root = tmp.path();
    delete_cut_short_at(root, cut::Step::Unstaged);
    // The save refuses from now on, and git's index is locked: nothing can land or come back.
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[plane]\nmode = \"commit\"\nsign = true\n",
    )
    .unwrap();
    run(root, &["config", "gpg.program", "false"]);
    std::fs::write(root.join(".git/index.lock"), "").unwrap();

    let ((), lines) = on(root, None, interrupted);

    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        matches!(&lines[0], Say::Warn(text) if text.contains("could not be finished or put back")),
        "{lines:?}"
    );
    assert_eq!(notes_left(root).len(), 1, "tried again next time");
}

#[test]
fn a_delete_cut_short_whose_skill_is_back_says_it_is_still_here_and_unstages_its_removal() {
    let tmp = committed_plane();
    let root = tmp.path();
    let before = head_of(root);
    delete_cut_short_at(root, cut::Step::Removed);
    // The files are back in the working tree, the removal still staged.
    run(
        root,
        &[
            "restore",
            "--source=HEAD",
            "--worktree",
            "--",
            "skills/release",
        ],
    );
    assert_eq!(
        run(root, &["status", "--porcelain", "--untracked-files=no"]).line(),
        "D  skills/release/SKILL.md"
    );

    let ((), lines) = on(root, None, interrupted);

    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        matches!(&lines[0], Say::Info(text) if text.contains("release") && text.contains("still here")),
        "{lines:?}"
    );
    assert_eq!(head_of(root), before);
    assert_eq!(
        run(root, &["status", "--porcelain"]).line(),
        "",
        "the index is as the last commit has it"
    );
    assert!(notes_left(root).is_empty());
}

#[test]
fn a_planted_or_broken_note_is_taken_away_and_nothing_else_is_done() {
    let tmp = committed_plane();
    let root = tmp.path();
    let before = head_of(root);
    let dir = crate::plane::state_dir(root).join(DELETING_DIR);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, text) in [
        ("1-up", "skills\n..\n"),
        ("2-nested", "skills\na/b\n"),
        ("3-more", "skills\nrelease\nskills/deploy\n"),
        ("4-nowhere", "elsewhere\nrelease\n"),
        ("5-empty", ""),
    ] {
        std::fs::write(dir.join(name), text).unwrap();
    }

    let ((), lines) = on(root, None, interrupted);

    assert!(lines.is_empty(), "{lines:?}");
    assert!(notes_left(root).is_empty());
    assert_eq!(head_of(root), before);
    assert_eq!(run(root, &["status", "--porcelain"]).line(), "");
    assert!(root.join("skills/release/SKILL.md").exists());
    assert!(root.join("skills/deploy/SKILL.md").exists());
}

#[test]
fn a_note_whose_writer_died_before_it_was_named_is_taken_away() {
    let tmp = committed_plane();
    let root = tmp.path();
    let dir = crate::plane::state_dir(root).join(DELETING_DIR);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(".1-release"), "skills\nrelease\n").unwrap();

    let ((), lines) = on(root, None, interrupted);

    assert!(lines.is_empty(), "{lines:?}");
    assert!(notes_left(root).is_empty());
    assert!(root.join("skills/release/SKILL.md").exists());
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

// Persona-owned skills (SK-32, #3003): every verb above at a persona's level.

/// What a chat as `persona` in `workspace` gets, by name.
fn gets_as(root: &Path, workspace: Option<&str>, persona: &str) -> Vec<String> {
    crate::skills::in_the_studio(None, root, workspace, Some(persona))
        .effective
        .into_iter()
        .map(|e| e.skill.name)
        .collect()
}

/// The `skills:` line of `persona`'s definition at `rel`.
fn skills_line(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel))
        .unwrap()
        .lines()
        .find(|line| line.starts_with("skills:"))
        .unwrap_or_default()
        .to_owned()
}

/// [`committed_plane`] with a persona `reviewer` that owns the skill `lint`, a one-file persona
/// `auditor` that names `lint`, and the project skill `release` naming `skills:lint`.
fn persona_plane() -> tempfile::TempDir {
    let tmp = committed_plane();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("personas/reviewer")).unwrap();
    std::fs::write(
        root.join("personas/reviewer/persona.md"),
        "---\nname: reviewer\n---\n\nReviews.\n",
    )
    .unwrap();
    skill(root, "personas/reviewer/skills/lint", "lint", "Lints it.");
    std::fs::write(
        root.join("personas/auditor.md"),
        "---\nname: auditor\nskills: lint\n---\n\nAudits.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("skills/release/SKILL.md"),
        "---\nname: release\ndescription: Cuts a release.\n---\n\nThen skills:lint.\n",
    )
    .unwrap();
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "personas"]);
    tmp
}

#[test]
fn a_personas_own_skill_is_made_edited_and_committed_at_its_level() {
    let tmp = persona_plane();
    let root = tmp.path();

    let folder = did(root, |cx| {
        create(
            cx,
            &Level::persona("reviewer"),
            "triage",
            "Sorts new issues.",
            &BuiltInTemplates,
            &TemplateId::BuiltIn(Template::Blank),
        )
    })
    .unwrap();
    assert_eq!(folder, root.join("personas/reviewer/skills/triage"));
    let (subject, paths) = last_commit(root);
    assert!(subject.ends_with("skill new triage"), "{subject}");
    assert_eq!(paths, ["personas/reviewer/skills/triage/SKILL.md"]);
    // It goes with the persona into every workspace, and reaches no one else.
    assert!(gets_as(root, Some("ops"), "reviewer").contains(&"triage".to_owned()));
    assert!(!gets(root, Some("ops")).contains(&"triage".to_owned()));
    assert!(!gets(root, None).contains(&"triage".to_owned()));

    let (_, was) = text_of(root, &Level::persona("reviewer"), "triage").unwrap();
    let now = was.replace("body", "Sort them.");
    did(root, |cx| {
        edit(cx, &Level::persona("reviewer"), "triage", &was, &now)
    })
    .unwrap();
    assert_eq!(
        last_commit(root).1,
        ["personas/reviewer/skills/triage/SKILL.md"]
    );

    assert!(
        Level::every(root).contains(&Level::persona("reviewer").local(true)),
        "a persona with a folder is a level the studio offers"
    );
    assert!(
        !Level::every(root)
            .iter()
            .any(|level| level.at.persona() == Some("auditor")),
        "a one-file persona keeps no skills"
    );
}

#[test]
fn a_persona_skill_is_refused_for_a_persona_there_is_not_or_one_made_as_one_file() {
    let tmp = persona_plane();
    let root = tmp.path();
    let new = |persona: &str| {
        did(root, |cx| {
            create(
                cx,
                &Level::persona(persona),
                "x",
                "Does x.",
                &BuiltInTemplates,
                &TemplateId::BuiltIn(Template::Blank),
            )
        })
    };

    assert!(new("nobody").unwrap_err().contains("no persona \"nobody\""));
    assert!(
        new("auditor")
            .unwrap_err()
            .contains("keeps no skills of its own")
    );
    assert!(!root.join("personas/auditor").exists());
    assert!(!root.join("personas/nobody").exists());
}

#[test]
fn renaming_a_personas_skill_follows_into_every_persona_list_and_skill_that_names_it() {
    let tmp = persona_plane();
    let root = tmp.path();

    let to = did(root, |cx| {
        rename(cx, &Level::persona("reviewer"), "lint", "check")
    })
    .unwrap();

    assert_eq!(to, root.join("personas/reviewer/skills/check"));
    assert_eq!(skills_line(root, "personas/auditor.md"), "skills: check");
    assert!(
        std::fs::read_to_string(root.join("skills/release/SKILL.md"))
            .unwrap()
            .contains("skills:check")
    );
    let (subject, paths) = last_commit(root);
    assert!(subject.ends_with("skill rename lint check"), "{subject}");
    assert_eq!(
        paths,
        [
            "personas/auditor.md",
            "personas/reviewer/skills/check/SKILL.md",
            "personas/reviewer/skills/lint/SKILL.md",
            "skills/release/SKILL.md",
        ]
    );
    assert!(gets_as(root, None, "reviewer").contains(&"check".to_owned()));
}

#[test]
fn promoting_a_personas_skill_asks_for_its_grant_and_the_persona_keeps_it_everywhere() {
    let tmp = persona_plane();
    let root = tmp.path();
    let (from, to) = (Level::persona("reviewer"), Level::project());
    assert!(needs_grant(&from, &to));
    assert!(!needs_grant(&to, &from));
    assert!(!needs_grant(&from, &Level::workspace("ops")));

    let refused = did(root, |cx| move_to(cx, &from, "lint", &to, None));
    assert_eq!(refused.unwrap_err(), GRANT_NEEDED);
    assert!(root.join("personas/reviewer/skills/lint/SKILL.md").exists());

    // Granted to the root alone: the persona's list is what brings it into a workspace.
    let moved = did(root, |cx| {
        move_to(cx, &from, "lint", &to, Some(&Grant::Root))
    })
    .unwrap();
    assert_eq!(moved, root.join("skills/lint"));
    assert_eq!(
        skills_line(root, "personas/reviewer/persona.md"),
        "skills: lint"
    );
    assert!(gets_as(root, Some("ops"), "reviewer").contains(&"lint".to_owned()));
    assert!(!gets(root, Some("ops")).contains(&"lint".to_owned()));
    let (subject, paths) = last_commit(root);
    assert!(subject.ends_with("skill mv lint project"), "{subject}");
    assert_eq!(
        paths,
        [
            "personas/reviewer/persona.md",
            "personas/reviewer/skills/lint/SKILL.md",
            "skills/lint/SKILL.md",
        ]
    );
}

#[test]
fn a_project_skill_moved_to_a_persona_leaves_its_list_its_grant_and_the_turn_offs_and_says_who_loses_it()
 {
    let tmp = persona_plane();
    let root = tmp.path();
    crate::skills::grant(root, "deploy", &Grant::All).unwrap();
    crate::skills::switch(root, "ops", "deploy", false).unwrap();
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "granted"]);

    // steward names deploy: it becomes steward's own, so steward's list no longer needs it.
    let (moved, lines) = on(root, None, |cx| {
        move_to(
            cx,
            &Level::project(),
            "deploy",
            &Level::persona("steward"),
            None,
        )
    });

    let moved = moved.unwrap();
    assert_eq!(moved, root.join("personas/steward/skills/deploy"));
    assert!(!beside(&moved).exists(), "a persona's skill keeps no grant");
    assert_eq!(
        skills_line(root, "personas/steward/persona.md"),
        "skills: purlis:handoff"
    );
    assert!(crate::skills::turned_off(root, "ops").is_empty());
    assert!(gets_as(root, Some("ops"), "steward").contains(&"deploy".to_owned()));
    assert!(!gets(root, None).contains(&"deploy".to_owned()));
    assert!(said(&lines, |line| matches!(
        line,
        Say::Warn(text) if text.contains("the project's chats get deploy now only as persona steward")
    )));
    let (_, paths) = last_commit(root);
    assert!(
        paths.contains(&"personas/steward/persona.md".to_owned())
            && paths.contains(&"personas/steward/skills/deploy/SKILL.md".to_owned())
            && paths.contains(&"workspaces/ops/workspace.json".to_owned()),
        "{paths:?}"
    );
}

#[test]
fn moving_a_skill_into_a_persona_names_the_other_personas_that_lose_it() {
    let tmp = persona_plane();
    let root = tmp.path();
    // auditor names lint; moving it from reviewer to steward leaves auditor and reviewer without.
    let (moved, lines) = on(root, None, |cx| {
        move_to(
            cx,
            &Level::persona("reviewer"),
            "lint",
            &Level::persona("steward"),
            None,
        )
    });

    moved.unwrap();
    assert!(said(&lines, |line| matches!(
        line,
        Say::Warn(text) if text.contains("persona reviewer no longer gets lint")
    )));
    assert!(said(&lines, |line| matches!(
        line,
        Say::Warn(text) if text.contains("1 persona names lint") && text.contains("auditor")
    )));
    assert!(gets_as(root, None, "steward").contains(&"lint".to_owned()));
    assert!(!gets_as(root, None, "reviewer").contains(&"lint".to_owned()));
}

#[test]
fn a_personas_skill_says_who_uses_it_and_is_deleted_and_restored_from_git() {
    let tmp = persona_plane();
    let root = tmp.path();

    let users = users(root, &Level::persona("reviewer"), "lint").unwrap();
    assert_eq!(
        users,
        Users {
            root: false,
            workspaces: Vec::new(),
            persona: Some("reviewer".to_owned()),
            personas: vec!["auditor".to_owned()],
            skills: vec!["release".to_owned()],
        }
    );

    let kept = did(root, |cx| delete(cx, &Level::persona("reviewer"), "lint")).unwrap();
    assert!(kept.in_git(), "{kept:?}");
    assert!(!root.join("personas/reviewer/skills/lint").exists());
    assert_eq!(
        last_commit(root).1,
        ["personas/reviewer/skills/lint/SKILL.md"]
    );
    let listed = deleted(root);
    let one = listed
        .iter()
        .find(|one| one.name == "lint")
        .unwrap_or_else(|| panic!("{listed:?}"));
    assert_eq!(one.level, Level::persona("reviewer"));

    did(root, |cx| restore(cx, one)).unwrap();
    assert!(gets_as(root, None, "reviewer").contains(&"lint".to_owned()));
    let (subject, paths) = last_commit(root);
    assert!(subject.ends_with("skill restore lint"), "{subject}");
    assert_eq!(paths, ["personas/reviewer/skills/lint/SKILL.md"]);
}

#[test]
fn a_personas_local_skill_is_kept_on_this_machine_when_deleted_and_shared_into_its_skills() {
    let tmp = persona_plane();
    let root = tmp.path();
    skill(
        root,
        "personas/reviewer/skills.local/scratch",
        "scratch",
        "Tries things.",
    );
    skill(
        root,
        "personas/reviewer/skills.local/notes",
        "notes",
        "Keeps notes.",
    );
    let local = Level::persona("reviewer").local(true);

    let kept = did(root, |cx| delete(cx, &local, "scratch")).unwrap();
    assert!(!kept.in_git());
    let listed = deleted(root);
    let one = listed.iter().find(|one| one.name == "scratch").unwrap();
    assert_eq!(one.level, local);
    did(root, |cx| restore(cx, one)).unwrap();
    assert!(
        root.join("personas/reviewer/skills.local/scratch/SKILL.md")
            .exists()
    );

    let shared = did(root, |cx| {
        share(cx, &Whose::Persona("reviewer".to_owned()), "notes")
    })
    .unwrap();
    assert_eq!(shared, root.join("personas/reviewer/skills/notes"));
    let (subject, paths) = last_commit(root);
    assert!(subject.ends_with("skill share notes"), "{subject}");
    assert_eq!(paths, ["personas/reviewer/skills/notes/SKILL.md"]);
}

#[test]
fn a_skill_moved_into_a_personas_local_skills_keeps_loading_and_is_never_committed() {
    // SK-32 (#3124): the plane's `.gitignore` keeps every level's `skills.local/` on this
    // machine, a persona's too, so its local skill is read, and no save takes it.
    let tmp = persona_plane();
    let root = tmp.path();
    let ignores = crate::scaffold::skills_local_ignores().join("\n");
    std::fs::write(
        root.join(".gitignore"),
        format!(".charter/\n.purlis/\n{ignores}\n"),
    )
    .unwrap();
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "ignores"]);

    let moved = did(root, |cx| {
        move_to(
            cx,
            &Level::project(),
            "deploy",
            &Level::persona("steward").local(true),
            None,
        )
    })
    .unwrap();

    assert_eq!(moved, root.join("personas/steward/skills.local/deploy"));
    assert!(gets_as(root, Some("ops"), "steward").contains(&"deploy".to_owned()));
    let (_, paths) = last_commit(root);
    assert!(
        paths.iter().all(|path| !path.contains("skills.local")),
        "{paths:?}"
    );
    let tree = run(root, &["ls-tree", "-r", "--name-only", "HEAD"]).out;
    assert!(!tree.contains("skills.local"), "{tree}");
}

/// #3443: one place decides which copy of a name a skill's tab shows first: the levels in
/// [`Level::every`]'s order, the project's before this machine's own, each with its folder.
#[test]
fn a_skills_copies_come_in_the_one_order_its_tab_shows_them() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    for folder in ["skills.local/deploy", "skills/deploy"] {
        std::fs::create_dir_all(root.join(folder)).unwrap();
        std::fs::write(
            root.join(folder).join("SKILL.md"),
            "---\nname: deploy\ndescription: Ships it.\n---\n",
        )
        .unwrap();
    }

    let copies: Vec<(Level, std::path::PathBuf)> = Level::copies_of(root, "deploy").collect();

    let levels: Vec<&Level> = copies.iter().map(|(level, _)| level).collect();
    assert_eq!(
        levels,
        [&Level::project(), &Level::project().local(true)],
        "the project's copy first"
    );
    assert!(copies[0].1.ends_with("skills/deploy"));
    assert!(Level::copies_of(root, "nothing").next().is_none());
}

/// The folder of this machine's kept deletes in the plane at `root`.
fn kept_deletes(root: &Path) -> PathBuf {
    crate::plane::state_dir(root).join(DELETED_DIR)
}

/// The names of the kept deletes in the plane at `root`, sorted.
fn kept_names(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(kept_deletes(root))
        .unwrap()
        .map(|one| one.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// Gives the one kept delete of `folder` the stamp `stamp`, as if it had been deleted then.
fn deleted_at(root: &Path, folder: &str, stamp: u128) -> String {
    let dir = kept_deletes(root);
    let was = kept_names(root)
        .into_iter()
        .find(|name| name.ends_with(&format!("-{folder}")))
        .unwrap();
    let now = format!("{stamp}-{folder}");
    std::fs::rename(dir.join(&was), dir.join(&now)).unwrap();
    now
}

/// Long enough ago that a kept delete is past its removal: a day more than [`EXPIRY_AFTER`].
fn long_ago() -> u128 {
    (std::time::SystemTime::now() - EXPIRY_AFTER - std::time::Duration::from_secs(DAY))
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

#[cfg(unix)]
#[test]
fn a_skill_kept_deleted_past_its_time_is_removed_and_a_newer_copy_of_it_is_kept() {
    let tmp = plane();
    let root = tmp.path();
    did(root, |cx| delete(cx, &Level::project(), "deploy")).unwrap();
    let old = deleted_at(root, "deploy", long_ago());
    skill(root, "skills/deploy", "deploy", "Ships it again.");
    did(root, |cx| delete(cx, &Level::project(), "deploy")).unwrap();
    did(root, |cx| delete(cx, &Level::project(), "release")).unwrap();
    let young = kept_names(root)
        .into_iter()
        .filter(|name| *name != old)
        .collect::<Vec<_>>();

    let expired = expire(root, std::time::SystemTime::now());

    assert_eq!(
        expired,
        Expired {
            removed: vec![old],
            ..Expired::default()
        }
    );
    assert_eq!(
        kept_names(root),
        young,
        "the newer copy and the other skill stay"
    );
    let listed = deleted(root);
    assert_eq!(listed.len(), 2, "{listed:?}");
    assert_eq!(
        listed[0].stays(std::time::SystemTime::now()),
        "kept on this machine for 30 more days"
    );
    assert!(
        root.join("skills").is_dir() && root.join("charter.toml").is_file(),
        "nothing outside the kept deletes is touched"
    );
}

#[cfg(unix)]
#[test]
fn an_expiry_follows_no_link_out_of_the_kept_deletes() {
    let tmp = plane();
    let root = tmp.path();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("precious"), "keep me").unwrap();
    did(root, |cx| delete(cx, &Level::project(), "deploy")).unwrap();
    let old = deleted_at(root, "deploy", long_ago());
    let kept = kept_deletes(root).join(&old);
    // Inside the deleted skill's folder: a link out, to a folder and to a file.
    std::os::unix::fs::symlink(outside.path(), kept.join("deploy/out")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("precious"),
        kept.join("deploy/precious"),
    )
    .unwrap();
    // Beside it, a link named as a kept delete is, past its time, to a folder outside.
    std::os::unix::fs::symlink(outside.path(), kept_deletes(root).join("1000-outside")).unwrap();

    let expired = expire(root, std::time::SystemTime::now());

    assert_eq!(expired.removed, vec![old], "{expired:?}");
    assert!(!kept.exists());
    assert!(kept_deletes(root).join("1000-outside").is_symlink());
    assert_eq!(
        std::fs::read_to_string(outside.path().join("precious")).unwrap(),
        "keep me"
    );
}

#[cfg(unix)]
#[test]
fn kept_deletes_that_are_a_link_are_never_expired() {
    let tmp = plane();
    let root = tmp.path();
    did(root, |cx| delete(cx, &Level::project(), "deploy")).unwrap();
    let old = deleted_at(root, "deploy", long_ago());
    // The kept deletes moved elsewhere, and a link left in their place.
    let elsewhere = tempfile::tempdir().unwrap();
    let moved = elsewhere.path().join("kept");
    std::fs::rename(kept_deletes(root), &moved).unwrap();
    std::os::unix::fs::symlink(&moved, kept_deletes(root)).unwrap();

    let expired = expire(root, std::time::SystemTime::now());

    assert_eq!(expired, Expired::default());
    assert!(moved.join(&old).join("deploy/SKILL.md").is_file());
}

#[cfg(unix)]
#[test]
fn a_kept_delete_holding_what_no_delete_puts_there_is_left_whole_and_said() {
    let tmp = plane();
    let root = tmp.path();
    did(root, |cx| delete(cx, &Level::project(), "deploy")).unwrap();
    let old = deleted_at(root, "deploy", long_ago());
    let kept = kept_deletes(root).join(&old);
    std::fs::write(kept.join("notes.txt"), "mine").unwrap();
    // Named as no delete names its folder: a `..` and a name with no stamp.
    for name in ["1000-..", "deploy", "1000-"] {
        std::fs::create_dir_all(kept_deletes(root).join(name)).unwrap();
    }

    let expired = expire(root, std::time::SystemTime::now());

    assert!(expired.removed.is_empty(), "{expired:?}");
    assert_eq!(expired.failed.len(), 1, "{expired:?}");
    assert_eq!(expired.failed[0].0, old);
    assert!(expired.failed[0].1.contains("notes.txt"));
    assert!(kept.join("deploy/SKILL.md").is_file());
    assert!(kept.join(KEPT_FROM).is_file());
    for name in ["1000-..", "deploy", "1000-"] {
        assert!(kept_deletes(root).join(name).is_dir());
    }
}

#[test]
fn a_kept_deletes_name_is_a_stamp_and_one_plain_folder_name() {
    assert_eq!(
        kept_name("1761000000000-deploy"),
        Some((1_761_000_000_000, "deploy"))
    );
    assert_eq!(kept_name("1-my-skill"), Some((1, "my-skill")));
    for not_one in [
        "1-..",
        "1-.",
        "1-",
        "-deploy",
        "x1-deploy",
        "1-a/b",
        "1-a\\b",
        "deploy",
    ] {
        assert_eq!(kept_name(not_one), None, "{not_one}");
    }
}

#[test]
fn a_deleted_skill_says_how_long_it_stays() {
    let here = |at: u64| Deleted {
        name: "deploy".to_owned(),
        level: Level::project(),
        folder: "deploy".to_owned(),
        kept: Kept(Source::Here(format!("{}-deploy", at * 1000))),
        at,
    };
    let when = |secs: u64| std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs);
    let at = 1_761_000_000;
    let kept_for = KEPT_HERE_FOR.as_secs();
    assert_eq!(
        here(at).stays(when(at)),
        "kept on this machine for 30 more days"
    );
    assert_eq!(
        here(at).stays(when(at + kept_for - DAY - 1)),
        "kept on this machine for 2 more days"
    );
    assert_eq!(
        here(at).stays(when(at + kept_for - 60)),
        "kept on this machine for 1 more day"
    );
    assert_eq!(
        here(at).stays(when(at + kept_for + 1)),
        "removed from this machine soon"
    );
    assert_eq!(
        here(at + 365 * DAY).stays(when(at)),
        "kept on this machine for 30 more days",
        "never more than its time, whatever clock stamped it"
    );
    let in_git = Deleted {
        kept: Kept(Source::Commit("abc".to_owned())),
        ..here(at)
    };
    assert_eq!(
        in_git.stays(when(at + 10 * kept_for)),
        "kept in the project's history"
    );
}

#[cfg(unix)]
#[test]
fn a_kept_delete_past_its_time_is_offered_back_no_more_and_removed_only_a_day_later() {
    let tmp = plane();
    let root = tmp.path();
    did(root, |cx| delete(cx, &Level::project(), "deploy")).unwrap();
    let ms = |at: std::time::SystemTime| {
        at.duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    };
    // Deleted 30 days and an hour ago: past its time, within its removal's day.
    let then = std::time::SystemTime::now() - KEPT_HERE_FOR - std::time::Duration::from_secs(3600);
    let kept = deleted_at(root, "deploy", ms(then));

    assert!(deleted(root).is_empty(), "past its time, no list offers it");
    assert_eq!(
        expire(root, std::time::SystemTime::now()),
        Expired::default()
    );
    assert!(
        kept_deletes(root)
            .join(&kept)
            .join("deploy/SKILL.md")
            .is_file()
    );

    let a_day_on = std::time::SystemTime::now() + std::time::Duration::from_secs(DAY);
    assert_eq!(expire(root, a_day_on).removed, vec![kept]);
}

#[cfg(unix)]
#[test]
fn a_kept_delete_holding_a_name_that_is_not_text_is_left_whole() {
    use std::os::unix::ffi::OsStrExt as _;
    let tmp = plane();
    let root = tmp.path();
    did(root, |cx| delete(cx, &Level::project(), "deploy")).unwrap();
    let old = deleted_at(root, "deploy", long_ago());
    let kept = kept_deletes(root).join(&old);
    let odd = kept.join(std::ffi::OsStr::from_bytes(b"\xff\xfe"));
    if std::fs::write(&odd, "mine").is_err() {
        // A file system that takes only text names (APFS refuses these): nothing to test.
        return;
    }

    let expired = expire(root, std::time::SystemTime::now());

    assert!(expired.removed.is_empty(), "{expired:?}");
    assert!(kept.join(KEPT_FROM).is_file());
    assert!(kept.join("deploy/SKILL.md").is_file());
    assert!(odd.exists());
}

/// Every level of [`plane`]: the project's, the workspace `ops`'s and the persona `steward`'s,
/// shared and local.
fn every_level() -> Vec<Level> {
    [
        Level::project(),
        Level::workspace("ops"),
        Level::persona("steward"),
    ]
    .into_iter()
    .flat_map(|level| [level.clone(), level.local(true)])
    .collect()
}

/// Where [`skill`] writes a skill `folder` at `level` of [`plane`].
fn at(level: &Level, folder: &str) -> String {
    format!("{}/{folder}", level.rel().unwrap())
}

/// The names that belong to a built-in skill (spec #3679 S12): one purlis ships, found with no
/// bundle beside the binary; one only `bundle` holds; the index skill's; and the name of its own
/// a route gives a built-in skill.
const RESERVED: [&str; 4] = ["secrets", "bundled", "skill-index", "purlis-browser"];

/// A bundle's `skills/` holding the built-in skill `bundled`.
fn bundle() -> tempfile::TempDir {
    let built_in = tempfile::tempdir().unwrap();
    skill(built_in.path(), "bundled", "bundled", "Comes with purlis.");
    built_in
}

#[test]
fn a_new_skill_with_a_built_in_skills_name_is_refused_at_every_level_in_the_agreed_words() {
    let tmp = plane();
    let root = tmp.path();
    let built_in = bundle();

    for level in every_level() {
        for name in RESERVED {
            let (refused, _) = on(root, Some(built_in.path()), |cx| {
                create(
                    cx,
                    &level,
                    name,
                    "Stands in.",
                    &BuiltInTemplates,
                    &TemplateId::BuiltIn(Template::Blank),
                )
            });
            assert_eq!(
                refused.unwrap_err(),
                crate::skills::NAME_RESERVED,
                "{level} {name}"
            );
            assert!(!root.join(at(&level, name)).exists(), "{level} {name}");
        }
    }
    assert_eq!(
        crate::skills::NAME_RESERVED,
        "That name belongs to a built-in skill; pick another, or make a copy."
    );
}

#[test]
fn a_rename_to_a_built_in_skills_name_is_refused_at_every_level_and_changes_nothing() {
    let tmp = plane();
    let root = tmp.path();
    let built_in = bundle();

    for level in every_level() {
        let folder = at(&level, "mine");
        skill(root, &folder, "mine", "Mine.");
        let was = std::fs::read_to_string(root.join(&folder).join("SKILL.md")).unwrap();
        for name in RESERVED {
            let (refused, _) = on(root, Some(built_in.path()), |cx| {
                rename(cx, &level, "mine", name)
            });
            assert_eq!(
                refused.unwrap_err(),
                crate::skills::NAME_RESERVED,
                "{level} {name}"
            );
            assert!(!root.join(at(&level, name)).exists(), "{level} {name}");
        }
        let now = std::fs::read_to_string(root.join(&folder).join("SKILL.md")).unwrap();
        assert_eq!(now, was, "{level}");
    }
}

#[test]
fn a_skill_named_as_a_built_in_one_is_not_moved_or_shared_to_any_level() {
    let tmp = plane();
    let root = tmp.path();
    // Written by hand, or by a purlis older than the built-in skill.
    let from = Level::workspace("ops");
    skill(root, &at(&from, "secrets"), "secrets", "Mine.");

    for to in every_level().into_iter().filter(|to| *to != from) {
        let refused = did(root, |cx| {
            move_to(cx, &from, "secrets", &to, Some(&Grant::All))
        });
        assert_eq!(refused.unwrap_err(), crate::skills::NAME_RESERVED, "{to}");
        assert!(!root.join(at(&to, "secrets")).exists(), "{to}");
    }
    assert!(root.join(at(&from, "secrets")).join("SKILL.md").is_file());

    let local = from.clone().local(true);
    skill(root, &at(&local, "theme"), "theme", "Mine.");
    let refused = did(root, |cx| share(cx, &local.at, "theme"));
    assert_eq!(refused.unwrap_err(), crate::skills::NAME_RESERVED);
    assert!(root.join(at(&local, "theme")).join("SKILL.md").is_file());
    assert!(!root.join(at(&from, "theme")).exists());
}

#[test]
fn a_deleted_skill_named_as_a_built_in_one_is_not_restored() {
    let tmp = plane();
    let root = tmp.path();
    skill(root, "skills/secrets", "secrets", "Mine.");
    did(root, |cx| delete(cx, &Level::project(), "secrets")).unwrap();
    let listed = deleted(root);
    assert_eq!(listed.len(), 1, "{listed:?}");

    let refused = did(root, |cx| restore(cx, &listed[0]));

    assert_eq!(
        refused.unwrap_err(),
        "secrets cannot be restored under its name: it belongs to a built-in skill now"
    );
    assert!(!root.join("skills/secrets").exists());
}

#[test]
fn a_skill_on_disk_named_as_a_built_in_one_is_a_clash_to_rename_at_every_level() {
    let tmp = plane();
    let root = tmp.path();
    let built_in = bundle();
    for level in every_level() {
        skill(root, &at(&level, "bundled"), "bundled", "Mine.");
    }
    // Its folder's name is not its name: the name is what clashes.
    skill(root, "skills/vault", "secrets", "Mine.");

    let clashes = crate::skills::named_as_a_built_in(root, Some(built_in.path()));

    let mut wanted: Vec<(String, Level, String)> = every_level()
        .into_iter()
        .map(|level| ("bundled".to_owned(), level.clone(), at(&level, "bundled")))
        .collect();
    wanted.insert(
        1,
        (
            "secrets".to_owned(),
            Level::project(),
            "skills/vault".to_owned(),
        ),
    );
    let found: Vec<(String, Level, String)> = clashes
        .into_iter()
        .map(|one| (one.name, one.level, one.folder))
        .collect();
    assert_eq!(found, wanted);
    // Without the bundle, the name only it holds is anyone's again; the one purlis ships is not.
    let bare: Vec<String> = crate::skills::named_as_a_built_in(root, None)
        .into_iter()
        .map(|one| one.name)
        .collect();
    assert_eq!(bare, ["secrets"]);
}

#[test]
fn a_deleted_skill_counts_the_days_left_to_bring_it_back_wherever_it_is_kept() {
    let here = |at: u64| Deleted {
        name: "deploy".to_owned(),
        level: Level::project(),
        folder: "deploy".to_owned(),
        kept: Kept(Source::Here(format!("{}-deploy", at * 1000))),
        at,
    };
    let when = |secs: u64| std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs);
    let at = 1_761_000_000;
    let kept_for = KEPT_HERE_FOR.as_secs();
    assert_eq!(here(at).days_left(when(at)), 30);
    assert_eq!(here(at).days_left(when(at + 3 * DAY)), 27);
    assert_eq!(here(at).days_left(when(at + kept_for - 60)), 1);
    assert_eq!(here(at).days_left(when(at + kept_for + 1)), 0);
    assert_eq!(
        here(at + 365 * DAY).days_left(when(at)),
        30,
        "never more than its time, whatever clock stamped it"
    );
    let in_git = Deleted {
        kept: Kept(Source::Commit("abc".to_owned())),
        ..here(at)
    };
    assert_eq!(in_git.days_left(when(at + 18 * DAY)), 12);
    assert_eq!(in_git.days_left(when(at + 2 * kept_for)), 0);
}

#[test]
fn a_skill_kept_deleted_on_this_machine_shows_its_last_text() {
    let tmp = plane();
    let root = tmp.path();
    did(root, |cx| delete(cx, &Level::project(), "deploy")).unwrap();
    let listed = deleted(root);
    let text = deleted_text(root, &listed[0]).unwrap();
    assert!(text.contains("description: Ships it."), "{text}");
}

#[test]
fn a_committed_skill_deleted_by_git_shows_its_last_text_from_the_commit() {
    let tmp = committed_plane();
    let root = tmp.path();
    did(root, |cx| delete(cx, &Level::project(), "release")).unwrap();
    let listed = deleted(root);
    let one = listed.iter().find(|one| one.name == "release").unwrap();
    let text = deleted_text(root, one).unwrap();
    assert!(text.contains("name: release"), "{text}");
}

#[test]
fn the_last_text_of_a_record_the_project_does_not_list_is_refused_unread() {
    let tmp = plane();
    let root = tmp.path();
    std::fs::write(root.join("secret.md"), "not a skill").unwrap();
    let made_up = Deleted {
        name: "deploy".to_owned(),
        level: Level::project(),
        folder: "../..".to_owned(),
        kept: Kept(Source::Commit("HEAD".to_owned())),
        at: 0,
    };
    let refused = deleted_text(root, &made_up).unwrap_err();
    assert!(refused.contains("not a deleted skill"), "{refused}");
}

/// What a chat in `workspace` gets, by name, with the built-in skills in `built_in`.
fn gets_with(root: &Path, built_in: &Path, workspace: Option<&str>) -> Vec<String> {
    crate::skills::in_the_studio(Some(built_in), root, workspace, None)
        .effective
        .into_iter()
        .map(|e| e.skill.name)
        .collect()
}

/// A copy's own description, as Make my own copy proposes one (#3777, `copyDescriptionOf`).
const MINE: &str = "Comes with purlis. (your copy of bundled)";

#[test]
fn my_own_copy_of_a_built_in_skill_lands_at_the_level_picked_under_its_new_name() {
    let tmp = plane();
    let root = tmp.path();
    let built_in = bundle();
    let was = std::fs::read_to_string(built_in.path().join("bundled/SKILL.md")).unwrap();

    let (folder, lines) = on(root, Some(built_in.path()), |cx| {
        copy_built_in(
            cx,
            "bundled",
            &Level::workspace("ops"),
            "my-bundled",
            "Comes with purlis, my steps.",
        )
    });

    let folder = folder.unwrap();
    assert_eq!(folder, root.join("workspaces/ops/skills/my-bundled"));
    let text = std::fs::read_to_string(folder.join("SKILL.md")).unwrap();
    // Its own name and its own description, every other byte the built-in's (#3777).
    assert_eq!(
        text,
        was.replace("name: bundled", "name: my-bundled").replace(
            "description: Comes with purlis.",
            "description: Comes with purlis, my steps."
        )
    );
    assert!(said(&lines, |line| matches!(line, Say::Done(done) if done
        .contains("copied from the built-in skill bundled"))));
    // The built-in skill is as it was, and a chat in the workspace gets both.
    assert_eq!(
        std::fs::read_to_string(built_in.path().join("bundled/SKILL.md")).unwrap(),
        was
    );
    let got = gets_with(root, built_in.path(), Some("ops"));
    assert_eq!(
        got.iter().filter(|name| *name == "bundled").count(),
        1,
        "{got:?}"
    );
    assert_eq!(
        got.iter().filter(|name| *name == "my-bundled").count(),
        1,
        "{got:?}"
    );
}

#[test]
fn my_own_copy_lands_at_every_level_and_a_project_copy_reaches_the_root() {
    let tmp = plane();
    let root = tmp.path();
    let built_in = bundle();

    for level in every_level() {
        let to = format!(
            "copy-{}",
            every_level().iter().position(|one| *one == level).unwrap()
        );
        let folder = did(root, |cx| {
            copy_built_in(cx, "bundled", &level, &to, MINE).map(|_| ())
        });
        // `did` has no built-in skills: nothing to copy from.
        assert_eq!(
            folder.unwrap_err(),
            "bundled is no built-in skill, so there is nothing to copy"
        );
        let (made, _) = on(root, Some(built_in.path()), |cx| {
            copy_built_in(cx, "bundled", &level, &to, MINE)
        });
        assert_eq!(made.unwrap(), root.join(at(&level, &to)), "{level}");
    }
    let got = gets_with(root, built_in.path(), None);
    assert!(got.contains(&"copy-0".to_owned()), "{got:?}");
    assert!(got.contains(&"bundled".to_owned()), "{got:?}");
}

#[test]
fn my_own_copy_under_a_built_in_skills_name_is_refused_in_the_agreed_words() {
    let tmp = plane();
    let root = tmp.path();
    let built_in = bundle();

    for level in every_level() {
        for name in RESERVED {
            let (refused, _) = on(root, Some(built_in.path()), |cx| {
                copy_built_in(cx, "bundled", &level, name, MINE)
            });
            assert_eq!(
                refused.unwrap_err(),
                crate::skills::NAME_RESERVED,
                "{level} {name}"
            );
            assert!(!root.join(at(&level, name)).exists(), "{level} {name}");
        }
    }
}

#[test]
fn my_own_copy_is_refused_for_a_skill_that_is_not_built_in_or_a_name_the_level_holds() {
    let tmp = plane();
    let root = tmp.path();
    let built_in = bundle();

    let (refused, _) = on(root, Some(built_in.path()), |cx| {
        copy_built_in(cx, "deploy", &Level::project(), "my-deploy", MINE)
    });
    assert_eq!(
        refused.unwrap_err(),
        "deploy is no built-in skill, so there is nothing to copy"
    );
    assert!(!root.join("skills/my-deploy").exists());

    let was = std::fs::read_to_string(root.join("skills/release/SKILL.md")).unwrap();
    let (refused, _) = on(root, Some(built_in.path()), |cx| {
        copy_built_in(cx, "bundled", &Level::project(), "release", MINE)
    });
    assert!(
        refused
            .unwrap_err()
            .contains("already have one named release"),
        "a name the level holds"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("skills/release/SKILL.md")).unwrap(),
        was
    );

    let (refused, _) = on(root, Some(built_in.path()), |cx| {
        copy_built_in(cx, "bundled", &Level::project(), "Not A Name", MINE)
    });
    assert!(refused.unwrap_err().contains("cannot be a skill's name"));
}

#[test]
fn my_own_copy_is_refused_the_built_ins_description_word_for_word_or_one_that_breaks_a_rule() {
    let tmp = plane();
    let root = tmp.path();
    let built_in = bundle();

    // The built-in's own words, spaces around them or not: a chat could not tell the two apart.
    for same in ["Comes with purlis.", "  Comes with purlis. "] {
        let (refused, _) = on(root, Some(built_in.path()), |cx| {
            copy_built_in(cx, "bundled", &Level::project(), "my-bundled", same)
        });
        assert_eq!(
            refused.unwrap_err(),
            "the copy's description is bundled's word for word, so a chat could not tell them \
             apart: say what your copy does differently"
        );
    }
    let (refused, _) = on(root, Some(built_in.path()), |cx| {
        copy_built_in(cx, "bundled", &Level::project(), "my-bundled", "")
    });
    assert!(
        refused
            .unwrap_err()
            .starts_with("the copy's description: it is empty"),
        "an empty description"
    );
    let (refused, _) = on(root, Some(built_in.path()), |cx| {
        copy_built_in(
            cx,
            "bundled",
            &Level::project(),
            "my-bundled",
            "Steps: mine",
        )
    });
    assert!(
        refused.unwrap_err().contains("\": \""),
        "a rule of the line format"
    );
    // A character the line format cannot hold is said as the description's, not the built-in's.
    for odd in ["Mine.\u{c}more", "Mine.\u{1f}"] {
        let (refused, _) = on(root, Some(built_in.path()), |cx| {
            copy_built_in(cx, "bundled", &Level::project(), "my-bundled", odd)
        });
        assert!(
            refused
                .unwrap_err()
                .starts_with("the copy's description holds a character"),
            "{odd:?}"
        );
    }
    assert!(!root.join("skills/my-bundled").exists());

    // One made trims its description, and the built-in keeps its own.
    let (made, _) = on(root, Some(built_in.path()), |cx| {
        copy_built_in(cx, "bundled", &Level::project(), "my-bundled", "  Mine.  ")
    });
    let text = std::fs::read_to_string(made.unwrap().join("SKILL.md")).unwrap();
    assert!(text.contains("\ndescription: Mine.\n"), "{text}");
    assert!(
        std::fs::read_to_string(built_in.path().join("bundled/SKILL.md"))
            .unwrap()
            .contains("\ndescription: Comes with purlis.\n")
    );
}
