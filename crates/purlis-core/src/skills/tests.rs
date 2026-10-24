// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use super::*;
use crate::machine::Flags;

const ON: Flags = Flags { skill_studio: true };

#[test]
fn the_skill_studio_flag_is_off_until_this_machine_turns_it_on() {
    let unset = |_: &str| None;
    assert!(!studio_on(&Flags::default(), &unset));
    assert!(studio_on(&ON, &unset));
}

#[test]
fn turning_the_skill_studio_on_is_kept_in_this_machines_store_and_off_again() {
    let config = tempfile::tempdir().unwrap();

    set_studio_on(config.path(), true).unwrap();
    assert!(crate::machine::read(config.path()).store.flags.skill_studio);
    set_studio_on(config.path(), false).unwrap();
    assert!(!crate::machine::read(config.path()).store.flags.skill_studio);
}

#[test]
fn the_variable_turns_the_skill_studio_flag_on_or_off_over_what_the_machine_says() {
    let set =
        |value: &'static str| move |name: &str| (name == STUDIO_ENV).then(|| value.to_owned());
    for flags in [Flags::default(), ON] {
        assert!(studio_on(&flags, &set("1")), "{flags:?}");
        assert!(!studio_on(&flags, &set("0")), "{flags:?}");
        // A value it does not know leaves the machine to decide.
        assert_eq!(
            studio_on(&flags, &set("yes")),
            flags.skill_studio,
            "{flags:?}"
        );
    }
    // Under its old name too, as every product variable is read.
    let old = |name: &str| (name == "CHARTER_SKILL_STUDIO").then(|| "1".to_owned());
    assert!(studio_on(&Flags::default(), &old));
}

#[test]
fn a_chat_is_handed_every_built_in_skill_by_its_harness_route() {
    let plugin = crate::harness::testing::bundled_plugin();
    let kit = Kit {
        skills: None,
        binary: Path::new("/bin/purlis"),
        plugin: Some(&plugin),
        persona: None,
    };
    for harness in [Harness::ClaudeCode, Harness::Codex, Harness::Opencode] {
        let set = for_chat(kit, harness);

        assert!(
            set.effective.iter().any(|e| e.skill.name == "secrets"),
            "{harness:?}"
        );
        assert!(
            set.effective
                .iter()
                .all(|e| e.route == harness.skills() && e.origin == Origin::BuiltIn),
            "{harness:?}"
        );
        assert!(set.left_out.is_empty(), "{:?}", set.left_out);
        assert_eq!(set.built_in_dir(), Some(plugin.join("skills").as_path()));
    }
}

#[test]
fn only_a_chat_started_with_the_variable_is_briefed_on_its_skills() {
    let root = Path::new("/plane");
    let unset = |_: &str| None;
    assert_eq!(briefed(root, None, None, &unset), None);
    let empty = |name: &str| (name == LISTED_ENV).then(String::new);
    assert_eq!(briefed(root, None, None, &empty), None);

    let skills = crate::harness::testing::bundled_plugin().join("skills");
    let dir = skills.display().to_string();
    let set = move |name: &str| (name == LISTED_ENV).then(|| dir.clone());
    let text = briefed(root, None, None, &set).expect("listed");

    assert!(text.contains("`safe-remove`"), "{text}");
    assert!(text.contains(&skills.join("safe-remove/SKILL.md").display().to_string()));
}

#[test]
fn a_briefing_lists_what_the_chats_set_holds_and_not_a_folder_it_leaves_out() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("skills");
    for (folder, text) in [
        ("kept", "---\nname: kept\ndescription: Is kept.\n---\n"),
        ("undescribed", "---\nname: undescribed\n---\n"),
    ] {
        std::fs::create_dir_all(dir.join(folder)).unwrap();
        std::fs::write(dir.join(folder).join("SKILL.md"), text).unwrap();
    }
    let named = dir.display().to_string();
    let env = move |name: &str| (name == LISTED_ENV).then(|| named.clone());

    let text = briefed(tmp.path(), None, None, &env).expect("listed");

    assert!(text.contains("- `kept` — Is kept."), "{text}");
    assert!(!text.contains("undescribed"), "{text}");
}

/// A plane for the grant and Share orchestration (#2767): its `charter.toml`, a workspace
/// `smart-ide`, and a project skill `deploy`. No git repository: a local skill in it is read.
fn granting_plane() -> tempfile::TempDir {
    let plane = tempfile::tempdir().unwrap();
    std::fs::write(plane.path().join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::create_dir_all(plane.path().join("workspaces/smart-ide")).unwrap();
    let deploy = plane.path().join("skills/deploy");
    std::fs::create_dir_all(&deploy).unwrap();
    std::fs::write(
        deploy.join("SKILL.md"),
        "---\nname: deploy\ndescription: Ships it.\n---\n",
    )
    .unwrap();
    plane
}

fn gets(plane: &Path, workspace: Option<&str>) -> Vec<String> {
    in_the_studio(None, plane, workspace, None)
        .effective
        .into_iter()
        .map(|e| e.skill.name)
        .collect()
}

#[test]
fn a_grant_reaches_the_workspace_it_names_and_the_studio_shows_it() {
    let plane = granting_plane();
    assert!(gets(plane.path(), Some("smart-ide")).is_empty());

    let folder = grant(
        plane.path(),
        "deploy",
        &Grant::Only(vec!["smart-ide".to_owned()]),
    )
    .unwrap();

    assert_eq!(folder, plane.path().join("skills/deploy"));
    assert_eq!(gets(plane.path(), Some("smart-ide")), ["deploy"]);
}

#[test]
fn a_grant_to_a_workspace_the_plane_does_not_have_is_refused_and_writes_nothing() {
    let plane = granting_plane();

    let refused = grant(
        plane.path(),
        "deploy",
        &Grant::Only(vec!["smart-id".into()]),
    )
    .unwrap_err();

    assert!(refused.contains("no workspace \"smart-id\""), "{refused}");
    assert!(!plane.path().join("skills/deploy.toml").exists());
}

#[test]
fn a_workspace_turns_a_granted_skill_off_and_back_on_in_its_own_settings() {
    let plane = granting_plane();
    grant(plane.path(), "deploy", &Grant::All).unwrap();

    assert_eq!(switch(plane.path(), "smart-ide", "deploy", false), Ok(true));
    assert!(gets(plane.path(), Some("smart-ide")).is_empty());
    let manifest =
        std::fs::read_to_string(plane.path().join("workspaces/smart-ide/workspace.json")).unwrap();
    assert!(manifest.contains("\"off\""), "{manifest}");
    assert_eq!(
        switch(plane.path(), "smart-ide", "deploy", false),
        Ok(false),
        "already off: nothing is written"
    );

    assert_eq!(switch(plane.path(), "smart-ide", "deploy", true), Ok(true));
    assert_eq!(gets(plane.path(), Some("smart-ide")), ["deploy"]);
    assert!(turned_off(plane.path(), "smart-ide").is_empty());
}

#[test]
fn a_workspace_cannot_turn_on_a_skill_the_project_did_not_grant_it_and_is_told_why() {
    let plane = granting_plane();

    let refused = switch(plane.path(), "smart-ide", "deploy", true).unwrap_err();

    assert_eq!(
        refused,
        "deploy is not granted to smart-ide, so it cannot be turned on there: a workspace only \
         turns off what the project granted it. Its grant is root: a grant to smart-ide, or to \
         all, would reach it"
    );
    assert!(
        !plane
            .path()
            .join("workspaces/smart-ide/workspace.json")
            .exists()
    );
}

#[test]
fn a_local_skill_in_a_plane_that_is_no_git_repository_is_read_and_shared() {
    let plane = granting_plane();
    let scratch = plane.path().join("skills.local/scratch");
    std::fs::create_dir_all(&scratch).unwrap();
    std::fs::write(
        scratch.join("SKILL.md"),
        "---\nname: scratch\ndescription: Tries it.\n---\n",
    )
    .unwrap();
    let set = in_the_studio(None, plane.path(), None, None);
    assert!(
        set.effective
            .iter()
            .any(|e| e.skill.name == "scratch" && e.local),
        "{set:?}"
    );

    let moved = share_here(plane.path(), &Whose::Project, "scratch").unwrap();

    assert_eq!(moved, plane.path().join("skills/scratch"));
    let set = in_the_studio(None, plane.path(), None, None);
    assert!(
        set.effective
            .iter()
            .any(|e| e.skill.name == "scratch" && !e.local)
    );
}

#[test]
fn a_local_skill_is_read_only_while_git_ignores_it() {
    // The live/local rule of ADR 0048, for `skills.local/` (#2767): one git would commit is left
    // out with git's answer and the fix; ignored, it is in the set.
    let plane = granting_plane();
    let init = crate::testgit::run(plane.path(), &["init", "-q"]);
    assert_eq!(init.code, Some(0), "git init: {}", init.err);
    let scratch = plane.path().join("skills.local/scratch");
    std::fs::create_dir_all(&scratch).unwrap();
    std::fs::write(
        scratch.join("SKILL.md"),
        "---\nname: scratch\ndescription: Tries it.\n---\n",
    )
    .unwrap();

    let set = in_the_studio(None, plane.path(), None, None);
    let left = set
        .left_out
        .iter()
        .find(|l| l.name == "scratch")
        .expect("left out");
    assert_eq!(
        left.reason.to_string(),
        "a local skill is used only while it stays on this machine, and git would commit it: \
         purlis reinit keeps it here"
    );

    std::fs::write(plane.path().join(".gitignore"), "/skills.local/\n").unwrap();
    let set = in_the_studio(None, plane.path(), None, None);
    assert!(
        set.effective
            .iter()
            .any(|e| e.skill.name == "scratch" && e.local),
        "{set:?}"
    );
}

#[test]
fn what_a_workspace_turned_off_is_read_from_its_settings_and_a_list_of_no_names_turns_nothing_off()
{
    let plane = granting_plane();
    let manifest = plane.path().join("workspaces/smart-ide/workspace.json");
    std::fs::write(
        &manifest,
        r#"{"settings": {"skills": {"off": ["deploy", "lint"]}}}"#,
    )
    .unwrap();
    assert_eq!(turned_off(plane.path(), "smart-ide"), ["deploy", "lint"]);

    std::fs::write(&manifest, r#"{"settings": {"skills": {"off": "deploy"}}}"#).unwrap();
    assert!(turned_off(plane.path(), "smart-ide").is_empty());
    std::fs::write(&manifest, "not json").unwrap();
    assert!(turned_off(plane.path(), "smart-ide").is_empty());
    assert!(turned_off(plane.path(), "no-such-workspace").is_empty());
}

/// A plane with one project skill, a bundle with one built-in skill, and the folder the sets
/// go in, all in `tmp`: what a host on this machine arms a chat from.
fn a_host_on(tmp: &Path) -> (PathBuf, crate::hoststart::Kit) {
    let root = tmp.canonicalize().unwrap();
    let write_skill = |dir: &Path, name: &str| {
        std::fs::create_dir_all(dir.join(name)).unwrap();
        std::fs::write(
            dir.join(name).join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Does {name}.\n---\n\nbody\n"),
        )
        .unwrap();
    };
    let plugin = root.join("plugin");
    write_skill(&plugin.join("skills"), "secrets");
    let plane = root.join("plane");
    write_skill(&plane.join("skills"), "deploy");
    let kit = crate::hoststart::Kit {
        binary: Some(PathBuf::from("/bin/purlis")),
        plugin: Some(plugin),
        git_hooks: None,
        skill_sets: Some(root.join("data").join(crate::hoststart::SKILL_SETS)),
    };
    (plane, kit)
}

/// Makes `folder` look as if it was last laid out two hours ago, past [`TIDY_GRACE`].
fn last_laid_out_long_ago(folder: &Path) {
    let then = std::time::SystemTime::now() - 2 * TIDY_GRACE;
    std::fs::File::open(folder)
        .unwrap()
        .set_modified(then)
        .unwrap();
}

/// Waits until `done`, for ten seconds at most.
fn within_ten_seconds(mut done: impl FnMut() -> bool) -> bool {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < until {
        if done() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    done()
}

#[cfg(unix)]
#[test]
fn a_skills_folder_a_running_chat_uses_is_kept_and_removed_once_the_chat_has_ended() {
    crate::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let (plane, kit) = a_host_on(tmp.path());
    let sets = kit.skill_sets.clone().unwrap();
    // The host arms a Claude Code chat as the window would, with the Skill studio on.
    let (args, env) = crate::hoststart::armed_as(
        true,
        &crate::hoststart::Arming {
            root: &plane,
            kit: &kit,
            harness: Some(Harness::ClaudeCode),
            cwd: Some(&plane),
            plugins: &crate::harness_plugin::Chosen::new(),
            sandbox: None,
            persona: None,
            env: &[],
        },
    )
    .unwrap();
    let folder = args
        .windows(2)
        .filter(|pair| pair[0] == "--plugin-dir")
        .map(|pair| PathBuf::from(&pair[1]))
        .find(|dir| dir.starts_with(&sets))
        .expect("the chat is handed a folder of links to the project's skills");
    // Its terminal, as every chat's is composed: a program that runs until it is ended,
    // handed the chat's line.
    let mut line = vec!["-c".to_owned(), "sleep 60".to_owned(), "chat".to_owned()];
    line.extend(args);
    let spec = crate::chatspec::spec(
        Vec::new(),
        &crate::chatspec::Terminal {
            program: "/bin/sh",
            args: &line,
            cwd: plane.to_str(),
            size: crate::engine::Size {
                columns: 80,
                rows: 24,
            },
            starting: crate::chatenv::Starting {
                harness: Some(Harness::ClaudeCode),
                operator: &[],
                strip: &[],
                set: &env,
                git_hooks: None,
                declared: &[],
            },
            number: 1,
            reporting: None,
        },
    );
    assert!(
        !spec.env.iter().any(|(name, _)| name
            .to_string_lossy()
            .ends_with(SET_ENV.trim_start_matches("PURLIS_"))),
        "the program is never handed the folder's variable: {:?}",
        spec.env
    );
    let chat = crate::session::Session::spawn(
        spec,
        Box::new(crate::engine::AlacrittyEngine::new(
            crate::engine::Size {
                columns: 80,
                rows: 24,
            },
            100,
        )),
    )
    .unwrap();
    last_laid_out_long_ago(&folder);

    let tidied = tidy(&sets).unwrap();
    assert!(tidied.removed.is_empty(), "{tidied:?}");
    assert!(folder.join("skills/deploy/SKILL.md").is_file());

    drop(chat);
    let digest = folder.file_name().unwrap().to_str().unwrap().to_owned();
    let notes = sets.join(HELD).join(&digest);
    assert!(
        within_ten_seconds(|| std::fs::read_dir(&notes).is_ok_and(|mut n| n.next().is_none())),
        "the chat's note goes once it has ended"
    );
    let tidied = tidy(&sets).unwrap();
    assert_eq!(tidied.removed, vec![digest]);
    assert!(!folder.exists());
    assert!(!notes.exists());
    assert!(
        plane.join("skills/deploy/SKILL.md").is_file(),
        "the skill it linked to is the plane's, and stays"
    );
}

/// A folder of `sets` named as a set's folder is named, holding a link to `target`.
fn a_set_folder(sets: &Path, digest: &str, target: &Path) -> PathBuf {
    let folder = sets.join(digest);
    std::fs::create_dir_all(folder.join("skills")).unwrap();
    std::os::unix::fs::symlink(target, folder.join("skills/deploy")).unwrap();
    folder
}

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[cfg(unix)]
#[test]
fn a_skills_folder_no_chat_uses_is_removed_once_its_grace_has_passed_and_never_what_it_links_to() {
    let tmp = tempfile::tempdir().unwrap();
    let sets = tmp.path().join("skill-sets");
    let skill = tmp.path().join("plane/skills/deploy");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(skill.join("SKILL.md"), "keep me").unwrap();
    let old = a_set_folder(&sets, A, &skill);
    last_laid_out_long_ago(&old);
    let young = a_set_folder(&sets, B, &skill);
    // A note of a program that has gone: a process id no process has.
    std::fs::create_dir_all(sets.join(HELD).join(A)).unwrap();
    std::fs::write(sets.join(HELD).join(A).join(u32::MAX.to_string()), "").unwrap();

    let tidied = tidy(&sets).unwrap();

    assert_eq!(
        tidied,
        Tidied {
            removed: vec![A.to_owned()],
            ..Tidied::default()
        }
    );
    assert!(!old.exists());
    assert!(!sets.join(HELD).join(A).exists(), "its notes go with it");
    assert!(young.exists(), "one laid out within its grace is kept");
    assert_eq!(
        std::fs::read_to_string(skill.join("SKILL.md")).unwrap(),
        "keep me"
    );
}

#[cfg(unix)]
#[test]
fn a_tidy_removes_only_the_folders_purlis_names_and_never_follows_a_link_out_of_the_sets() {
    let tmp = tempfile::tempdir().unwrap();
    let sets = tmp.path().join("skill-sets");
    std::fs::create_dir_all(&sets).unwrap();
    // Outside the sets: a folder, as a link named like a set's folder points to it.
    let outside = tmp.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("precious"), "").unwrap();
    std::os::unix::fs::symlink(&outside, sets.join(A)).unwrap();
    // A link named like a set's folder's notes, out of the sets too.
    std::os::unix::fs::symlink(&outside, sets.join(HELD)).unwrap();
    // Inside: folders purlis did not name, and a name that climbs.
    for name in ["notes", "ABCDEF", "..", ".hidden"] {
        let _ = std::fs::create_dir_all(sets.join(name));
    }
    let theirs = sets.join("notes");
    last_laid_out_long_ago(&theirs);

    let tidied = tidy(&sets).unwrap();

    assert_eq!(tidied, Tidied::default());
    assert!(outside.join("precious").exists());
    assert!(sets.join(A).is_symlink());
    assert!(theirs.is_dir());
    assert!(tmp.path().join("skill-sets").is_dir());
}

#[cfg(unix)]
#[test]
fn a_lay_out_that_never_finished_is_tidied_and_one_found_again_is_kept() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, kit) = a_host_on(tmp.path());
    let sets = kit.skill_sets.clone().unwrap();
    let folder = crate::hoststart::plane_skills_laid_out(
        true,
        &plane,
        None,
        None,
        &kit,
        Harness::ClaudeCode,
    )
    .unwrap();
    let leftover = sets.join(format!(".{A}.x1Y2z3"));
    std::fs::create_dir_all(&leftover).unwrap();
    last_laid_out_long_ago(&leftover);
    last_laid_out_long_ago(&folder);

    // Found again by the next chat's start: used now, so kept.
    let again = crate::hoststart::plane_skills_laid_out(
        true,
        &plane,
        None,
        None,
        &kit,
        Harness::ClaudeCode,
    )
    .unwrap();
    assert_eq!(again, folder);
    let tidied = tidy(&sets).unwrap();

    assert!(tidied.removed.is_empty(), "{tidied:?}");
    assert!(folder.is_dir());
    assert!(!leftover.exists());
}

#[test]
fn a_tidy_of_sets_that_are_not_there_does_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(
        tidy(&tmp.path().join("skill-sets")).unwrap(),
        Tidied::default()
    );
}

#[test]
fn a_hold_is_only_for_a_sets_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let sets = tmp.path().join("skill-sets");
    std::fs::create_dir_all(sets.join(A)).unwrap();
    std::fs::create_dir_all(tmp.path().join("elsewhere").join(B)).unwrap();

    assert_eq!(hold_of(&sets.join(A)), Some(sets.join(HELD).join(A)));
    assert_eq!(hold_of(&tmp.path().join("elsewhere").join(B)), None);
    assert_eq!(hold_of(&sets.join(B)), None, "not there");
    assert_eq!(hold_of(&sets.join("..")), None);
    let mut env = vec![
        ("CHARTER_SKILL_SET".into(), sets.join(A).into_os_string()),
        ("PURLIS_SKILL_SET".into(), sets.join(A).into_os_string()),
        ("KEPT".into(), "1".into()),
    ];
    assert_eq!(take_hold(&mut env), Some(sets.join(HELD).join(A)));
    assert_eq!(env, vec![("KEPT".into(), "1".into())]);
}

/// A folder outside the sets, with files and a folder of files in it: what a link in the sets
/// may point a tidy at. Answers every path in it, to tell nothing was removed or written there.
fn an_outside_folder(tmp: &Path) -> (PathBuf, Vec<PathBuf>) {
    let outside = tmp.join("home");
    std::fs::create_dir_all(outside.join("Documents")).unwrap();
    std::fs::write(outside.join("notes.txt"), "mine").unwrap();
    std::fs::write(outside.join("Documents/thesis.txt"), "mine").unwrap();
    std::fs::write(outside.join("Documents/4242"), "mine").unwrap();
    let all = everything_in(&outside);
    (outside, all)
}

fn everything_in(dir: &Path) -> Vec<PathBuf> {
    let mut all = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(next) = todo.pop() {
        for entry in std::fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            if std::fs::symlink_metadata(&path).unwrap().is_dir() {
                todo.push(path.clone());
            }
            all.push(path);
        }
    }
    all.sort();
    all
}

#[cfg(unix)]
#[test]
fn a_tidy_reads_and_removes_nothing_through_a_link_in_place_of_the_notes() {
    let tmp = tempfile::tempdir().unwrap();
    let (outside, before) = an_outside_folder(tmp.path());
    let skill = tmp.path().join("plane/skills/deploy");
    std::fs::create_dir_all(&skill).unwrap();

    // `.held` itself, a link out of the sets.
    let sets = tmp.path().join("one/skill-sets");
    let old = a_set_folder(&sets, A, &skill);
    last_laid_out_long_ago(&old);
    std::os::unix::fs::symlink(&outside, sets.join(HELD)).unwrap();
    let tidied = tidy(&sets).unwrap();
    assert!(
        tidied.removed.is_empty(),
        "notes it cannot read may name a running chat"
    );
    assert_eq!(everything_in(&outside), before);
    assert!(sets.join(HELD).is_symlink());

    // A folder of notes, `.held/<digest>`, a link out of the sets: it is not read, so its set's
    // folder is kept, and a digest-named link beside it is not followed.
    let sets = tmp.path().join("two/skill-sets");
    let old = a_set_folder(&sets, A, &skill);
    last_laid_out_long_ago(&old);
    std::fs::create_dir_all(sets.join(HELD)).unwrap();
    std::os::unix::fs::symlink(outside.join("Documents"), sets.join(HELD).join(A)).unwrap();
    std::os::unix::fs::symlink(&outside, sets.join(HELD).join(B)).unwrap();
    let tidied = tidy(&sets).unwrap();
    assert!(tidied.removed.is_empty(), "{tidied:?}");
    assert!(old.is_dir());
    assert_eq!(everything_in(&outside), before);
    assert!(sets.join(HELD).join(A).is_symlink());
    assert!(sets.join(HELD).join(B).is_symlink());
    assert!(skill.is_dir());
}

#[cfg(unix)]
#[test]
fn a_tidy_of_sets_that_are_a_link_does_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("real");
    let old = a_set_folder(&real, A, tmp.path());
    last_laid_out_long_ago(&old);
    let sets = tmp.path().join("skill-sets");
    std::os::unix::fs::symlink(&real, &sets).unwrap();

    assert_eq!(tidy(&sets).unwrap(), Tidied::default());
    assert!(old.is_dir());
}

#[cfg(unix)]
#[test]
fn a_tidy_never_waits_on_a_chat_being_started() {
    let tmp = tempfile::tempdir().unwrap();
    let sets = tmp.path().join("skill-sets");
    let old = a_set_folder(&sets, A, tmp.path());
    last_laid_out_long_ago(&old);
    let _starting = lock_shared_in(&at::folder(&sets).unwrap()).unwrap();

    let tidied = tidy(&sets).unwrap();

    assert!(tidied.busy);
    assert!(tidied.removed.is_empty());
    assert!(old.is_dir());
}

#[cfg(unix)]
#[test]
fn a_link_put_in_place_of_a_folder_of_notes_after_it_was_opened_is_never_followed() {
    let tmp = tempfile::tempdir().unwrap();
    let (outside, before) = an_outside_folder(tmp.path());
    let notes = tmp.path().join("skill-sets").join(HELD).join(A);
    std::fs::create_dir_all(&notes).unwrap();
    // A program that has gone, and a file that is not purlis's.
    std::fs::write(notes.join(u32::MAX.to_string()), "").unwrap();
    std::fs::write(notes.join("keep-me"), "").unwrap();
    let opened = at::folder(&notes).unwrap();
    // Between the check and the reading: the folder moved aside, and a link to a folder that
    // holds a file named like a note put in its place.
    let aside = notes.with_file_name("aside");
    std::fs::rename(&notes, &aside).unwrap();
    std::os::unix::fs::symlink(outside.join("Documents"), &notes).unwrap();

    assert!(still_held(&opened) == Notes::None);

    assert_eq!(everything_in(&outside), before);
    assert!(
        !aside.join(u32::MAX.to_string()).exists(),
        "the gone program's note goes"
    );
    assert!(
        aside.join("keep-me").exists(),
        "what is not named by a process id stays"
    );
}

#[cfg(unix)]
#[test]
fn a_set_folder_that_became_a_link_after_it_was_checked_is_not_removed_through_it() {
    let tmp = tempfile::tempdir().unwrap();
    let (outside, before) = an_outside_folder(tmp.path());
    let sets = tmp.path().join("skill-sets");
    let folder = a_set_folder(&sets, A, tmp.path());
    let sets_dir = at::folder(&sets).unwrap();
    // Checked, then swapped for a link before it is removed.
    std::fs::rename(&folder, sets.join("aside")).unwrap();
    std::os::unix::fs::symlink(&outside, &folder).unwrap();
    assert!(at::remove_tree_in(&sets_dir, A).is_err());
    assert_eq!(everything_in(&outside), before);

    // A folder inside a set's folder swapped for a link: only the link goes.
    let folder = a_set_folder(&sets, B, tmp.path());
    std::fs::remove_file(folder.join("skills/deploy")).unwrap();
    std::fs::remove_dir(folder.join("skills")).unwrap();
    std::os::unix::fs::symlink(&outside, folder.join("skills")).unwrap();
    at::remove_tree_in(&sets_dir, B).unwrap();
    assert!(!folder.exists());
    assert_eq!(everything_in(&outside), before);
}

#[cfg(unix)]
#[test]
fn a_tree_too_deep_to_be_a_set_folder_is_left_in_place_and_said() {
    let tmp = tempfile::tempdir().unwrap();
    let sets = tmp.path().join("skill-sets");
    let mut deepest = sets.join(A);
    for _ in 0..at::DEEPEST + 4 {
        deepest = deepest.join("d");
    }
    std::fs::create_dir_all(&deepest).unwrap();
    std::fs::write(deepest.join("bottom"), "").unwrap();
    last_laid_out_long_ago(&sets.join(A));

    let tidied = tidy(&sets).unwrap();

    assert!(tidied.removed.is_empty(), "{tidied:?}");
    assert_eq!(tidied.failed.len(), 1, "{tidied:?}");
    assert_eq!(tidied.failed[0].0, A);
    assert!(tidied.failed[0].1.contains("64 folders deep"), "{tidied:?}");
    assert!(
        deepest.join("bottom").is_file(),
        "what is below the limit stays"
    );
}

#[cfg(unix)]
#[test]
fn a_lay_out_into_sets_that_are_a_link_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let (plane, mut kit) = a_host_on(tmp.path());
    let outside = tmp.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let sets = tmp.path().join("linked").join(crate::hoststart::SKILL_SETS);
    std::fs::create_dir_all(sets.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&outside, &sets).unwrap();
    kit.skill_sets = Some(sets);

    assert!(
        crate::hoststart::plane_skills_laid_out(
            true,
            &plane,
            None,
            None,
            &kit,
            Harness::ClaudeCode
        )
        .is_none()
    );
    assert_eq!(
        std::fs::read_dir(&outside).unwrap().count(),
        0,
        "nothing written there"
    );
}

/// A plane for Use skill… (SK-12): a workspace `smart-ide` granted a project skill `review` that
/// takes arguments, a project skill `release` that is manual-only and granted nowhere, and a
/// workspace skill `lint`; and a bundle with the built-in `handoff`.
fn use_skill_plane() -> (tempfile::TempDir, PathBuf) {
    let tmp = granting_plane();
    let write = |dir: PathBuf, text: &str| {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), text).unwrap();
    };
    let root = tmp.path();
    write(
        root.join("skills/review"),
        "---\nname: review\ndescription: Reviews a PR.\narguments: pr focus\n---\n",
    );
    std::fs::write(root.join("skills/review.toml"), "grant = \"all\"\n").unwrap();
    write(
        root.join("skills/release"),
        "---\nname: release\ndescription: Cuts a release.\ndisable-model-invocation: true\n---\n",
    );
    write(
        root.join("workspaces/smart-ide/skills/lint"),
        "---\nname: lint\ndescription: Lints.\n---\n",
    );
    let bundle = root.join("bundle");
    write(
        bundle.join("handoff"),
        "---\nname: handoff\ndescription: Hands off.\n---\n",
    );
    (tmp, bundle)
}

fn running<'a>(root: &'a Path, bundle: &'a Path, workspace: Option<&'a str>) -> Running<'a> {
    Running {
        plane: root,
        workspace,
        persona: None,
        harness: Harness::ClaudeCode,
        built_in: Some(bundle),
        plane_skills: true,
    }
}

fn offered_names(skills: &ChatSkills) -> Vec<&str> {
    skills
        .offered
        .iter()
        .map(|one| one.effective.skill.name.as_str())
        .collect()
}

#[test]
fn use_skill_offers_exactly_the_chats_set_and_a_manual_only_one_is_free_on_every_harness() {
    let (tmp, bundle) = use_skill_plane();
    let root = tmp.path();

    let at_the_root = for_use_skill(&running(root, &bundle, None));
    assert_eq!(
        offered_names(&at_the_root),
        ["deploy", "handoff", "release", "review"]
    );
    let release = &at_the_root.offered[2];
    assert!(release.declared.manual);
    assert!(!release.takes_arguments);
    assert!(at_the_root.offered[3].takes_arguments, "review names two");
    assert_eq!(release.known_as, "skills:release");
    assert_eq!(at_the_root.offered[1].known_as, "purlis:handoff");
    assert_eq!(
        at_the_root.spent.skills, 3,
        "Claude Code leaves release out"
    );
    assert_eq!(at_the_root.spent.manual, 1);
    assert_eq!(
        at_the_root.spent.chars,
        "Ships it.".len() + "Hands off.".len() + "Reviews a PR.".len()
    );
    assert_eq!(at_the_root.spent.warns(), None);

    // opencode and Codex are handed the translation (SK-8, #2771): it is free there too.
    for harness in [Harness::Opencode, Harness::Codex] {
        let elsewhere = for_use_skill(&Running {
            harness,
            ..running(root, &bundle, None)
        });
        assert_eq!(elsewhere.spent.skills, 3, "{harness:?} leaves release out");
        assert_eq!(elsewhere.spent.manual, 1);
    }

    // In the workspace: its own skill and the granted one, never one it was not granted.
    let in_smart_ide = for_use_skill(&running(root, &bundle, Some("smart-ide")));
    assert_eq!(offered_names(&in_smart_ide), ["handoff", "lint", "review"]);

    // With the flag off, a chat's set is the built-in skills alone, as at its start.
    let flag_off = for_use_skill(&Running {
        plane_skills: false,
        ..running(root, &bundle, None)
    });
    assert_eq!(offered_names(&flag_off), ["handoff"]);
}

#[test]
fn use_skill_types_a_skill_of_the_set_in_its_harnesss_syntax_and_holds_the_run_it_starts() {
    let (tmp, bundle) = use_skill_plane();
    let root = tmp.path();
    let values = vec!["42".to_owned(), "the tests".to_owned()];
    for (harness, text, known_as, confirmed) in [
        (
            Harness::ClaudeCode,
            "/skills:review 42 \"the tests\"",
            "skills:review",
            crate::skillspan::Confirmed::ByName,
        ),
        (
            Harness::Opencode,
            "/review 42 \"the tests\"",
            "review",
            crate::skillspan::Confirmed::ByPrompt,
        ),
        (
            Harness::Codex,
            "$review pr: 42; focus: the tests",
            "review",
            crate::skillspan::Confirmed::ByPrompt,
        ),
    ] {
        let chat = for_use_skill(&Running {
            harness,
            ..running(root, &bundle, Some("smart-ide"))
        });
        let typing = chat.typed("review", &values).unwrap();
        assert_eq!(typing.text, text, "{harness:?}");
        assert_eq!(typing.held.skill, known_as, "{harness:?}");
        assert_eq!(typing.held.confirmed, confirmed, "{harness:?}");
        assert_eq!(typing.held.workspace.as_deref(), Some("smart-ide"));
        assert_eq!(
            typing.held.level.map(|level| level.origin),
            Some(Origin::Project)
        );
        // What a prompt holding the text is marked with.
        let sent = crate::skillspan::PromptMarks::of(&format!("{text} and more"));
        assert!(sent.holds(typing.held.text), "{harness:?}");
        let refused = chat.typed("deploy", &[]).unwrap_err();
        assert!(
            refused.contains("not one of this chat's skills"),
            "{refused}"
        );
    }
}

#[test]
fn a_set_at_four_fifths_of_its_harnesss_budget_warns() {
    let (tmp, bundle) = use_skill_plane();
    let root = tmp.path();
    // 3 skills listed already; 21 more make 24, four fifths of 30.
    for n in 0..21 {
        let dir = root.join(format!("skills/s{n:02}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: s{n:02}\ndescription: d\n---\n"),
        )
        .unwrap();
    }
    let chat = for_use_skill(&running(root, &bundle, None));
    assert_eq!(chat.spent.skills, 24);
    assert_eq!(chat.spent.warns(), Some(Over::Skills));
}

#[test]
fn the_studio_checks_every_harness_the_projects_profiles_run_and_claude_code_for_none() {
    use crate::harness::Harness;
    let kinds = |words: &[&str]| -> Vec<String> { words.iter().map(|w| (*w).to_owned()).collect() };
    assert_eq!(
        harnesses_of(&kinds(&["opencode", "claude", "codex", "claude"])),
        [Harness::ClaudeCode, Harness::Codex, Harness::Opencode]
    );
    assert_eq!(harnesses_of(&kinds(&["codex"])), [Harness::Codex]);
    assert_eq!(harnesses_of(&kinds(&["sh"])), [Harness::ClaudeCode]);
    assert_eq!(harnesses_of(&[]), [Harness::ClaudeCode]);
}

#[test]
fn the_studio_lists_each_harnesss_own_skills_beside_the_projects_and_marks_a_clash() {
    // SK-6 (#2769): Codex's own `review` and the project's `review` both load, so the pair is a
    // clash; a skill the project turned off for Codex is listed left out, with whose it is.
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(plane.join("skills/review")).unwrap();
    std::fs::write(
        plane.join("skills/review/SKILL.md"),
        "---\nname: review\ndescription: Reviews.\n---\n",
    )
    .unwrap();
    std::fs::write(
        plane.join("charter.toml"),
        "[harness_skills.codex]\nnotes = false\n",
    )
    .unwrap();
    let codex = tmp.path().join("codex-home");
    for name in ["review", "notes"] {
        std::fs::create_dir_all(codex.join("skills").join(name)).unwrap();
        std::fs::write(
            codex.join("skills").join(name).join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Does {name}.\n---\n"),
        )
        .unwrap();
    }
    let chat = [("CODEX_HOME".to_owned(), codex.display().to_string())];
    let env = crate::harness_plugin::Env {
        chat: &chat,
        home: Some(tmp.path().join("home")),
        process: false,
    };

    let set = in_the_studio(None, &plane, None, None);
    let harnesses = harnesses_in_the_studio(&set, &plane, None, &env);

    let titles: Vec<&str> = harnesses.iter().map(|h| h.group.adapter.title()).collect();
    assert_eq!(titles, ["Claude Code", "opencode", "Codex"]);
    let on_codex = &harnesses[2];
    assert_eq!(
        on_codex.set.clashes,
        [Clash {
            name: "review".to_owned(),
            plane: At {
                origin: Origin::Project,
                local: false,
            },
            outside: Origin::Harness,
            owner: Owner::You,
            outcome: Outcome::BothLoad,
        }]
    );
    let rows: Vec<(String, Option<String>)> = outside_rows(on_codex, &plane)
        .into_iter()
        .map(|row| (row.name, row.left_out))
        .collect();
    assert_eq!(
        rows,
        [
            (
                "notes".to_owned(),
                Some("turned off for this project's chats".to_owned())
            ),
            ("review".to_owned(), None),
        ]
    );
    assert!(
        harnesses[0].set.clashes.is_empty(),
        "Claude Code has none of its own here"
    );
}

#[test]
fn a_briefed_chat_in_a_clone_is_told_the_repos_copy_of_a_name_and_not_the_projects() {
    // SK-6 (#2769), decision 51: Codex reads the clone's `.agents/skills` itself, and purlis
    // builds the briefing, so the narrower repo copy is the one the chat gets.
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    let clone = plane.join("workspaces/alpha/api");
    std::fs::create_dir_all(clone.join(".git")).unwrap();
    for (dir, name) in [
        (plane.join("skills"), "review"),
        (plane.join("skills"), "deploy"),
        (clone.join(".agents/skills"), "review"),
    ] {
        std::fs::create_dir_all(dir.join(name)).unwrap();
        std::fs::write(
            dir.join(name).join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Does {name}.\n---\n"),
        )
        .unwrap();
    }
    for name in ["review", "deploy"] {
        std::fs::write(
            plane.join(format!("skills/{name}.toml")),
            "grant = \"all\"\n",
        )
        .unwrap();
    }
    let built_in = tmp.path().join("bundle/skills");
    std::fs::create_dir_all(&built_in).unwrap();
    let dir = built_in.display().to_string();
    // The set's folder the host laid out, as a Codex chat is handed it (SK-8).
    let handed = tmp.path().join("sets/handed");
    for name in ["deploy", "review"] {
        std::fs::create_dir_all(handed.join("skills").join(name)).unwrap();
        std::fs::write(
            handed.join("skills").join(name).join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Does {name}.\n---\n"),
        )
        .unwrap();
    }
    let handed = handed.display().to_string();
    let env = move |name: &str| match name {
        LISTED_ENV => Some(dir.clone()),
        PLANE_LISTED_ENV => Some(handed.clone()),
        _ => None,
    };

    let text = briefed(&plane, Some("alpha"), Some(&clone), &env).expect("listed");

    assert!(text.contains("`deploy`"), "{text}");
    assert!(
        !text.contains("`review`"),
        "the repo's copy is Codex's own: {text}"
    );
    let elsewhere = briefed(&plane, Some("alpha"), None, &env).expect("listed");
    assert!(
        elsewhere.contains("`review`"),
        "a chat in no clone: {elsewhere}"
    );
}

#[test]
fn a_repo_skill_named_as_a_built_in_one_never_replaces_it_on_codex_or_opencode() {
    // SK-6 (#2769): a clone's `secrets` must not take purlis's own away from a chat.
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    let clone = plane.join("workspaces/alpha/api");
    std::fs::create_dir_all(clone.join(".git")).unwrap();
    let built_in = tmp.path().join("bundle/skills");
    for (dir, says) in [
        (built_in.clone(), "purlis's"),
        (clone.join(".agents/skills"), "repo's"),
    ] {
        std::fs::create_dir_all(dir.join("secrets")).unwrap();
        std::fs::write(
            dir.join("secrets/SKILL.md"),
            format!("---\nname: secrets\ndescription: The {says} secrets.\n---\n"),
        )
        .unwrap();
    }
    let repo = repo_skills_of_chat(Harness::Codex, &plane, "alpha", &clone);
    assert_eq!(repo.len(), 1, "{repo:?}");
    for route in [Route::Briefing, Route::Config] {
        let set = ask(
            Some(&built_in),
            Some(PlaneSkills {
                plane: &plane,
                workspace: Some("alpha"),
                linked: None,
                repo: &repo,
            }),
            None,
            route,
            None,
        );
        assert_eq!(set.built_in_dir(), Some(built_in.as_path()), "{route:?}");
        let handed: Vec<(&str, Origin)> = set
            .effective
            .iter()
            .map(|e| (e.skill.name.as_str(), e.origin))
            .collect();
        assert_eq!(handed, [("secrets", Origin::BuiltIn)], "{route:?}");
        assert_eq!(
            set.left_out[0].reason.to_string(),
            "hidden by purlis's built-in skill of this name",
            "{route:?}"
        );
    }
}

/// SK-9 (#2772, decision 50): a Try it's scratch chat is handed every one of its profile's own
/// harness skills as turned off, read from the config folder the profile points its harness at
/// (`CLAUDE_CONFIG_DIR`), not purlis's own; a chat elsewhere on the same profile keeps them.
#[test]
fn a_try_chat_turns_off_the_skills_of_its_profiles_own_config_folder() {
    crate::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let (plane, kit) = a_host_on(tmp.path());
    std::fs::write(plane.join("charter.toml"), "schema = 1\n").unwrap();
    let file = verbs::drafts::draftable(&plane, &plane.join("skills/triage/SKILL.md")).unwrap();
    let id = verbs::drafts::take(
        &plane,
        7,
        &file,
        &crate::proposed::Change::Whole {
            content: "---\nname: triage\ndescription: Sorts.\n---\n\nbody\n".to_owned(),
        },
    )
    .unwrap();
    let tried = verbs::drafts::try_out(&plane, &id).unwrap();
    let config = tmp.path().canonicalize().unwrap().join("profile-claude");
    std::fs::create_dir_all(config.join("skills/profiles-own")).unwrap();
    std::fs::write(
        config.join("skills/profiles-own/SKILL.md"),
        "---\nname: profiles-own\ndescription: The profile's own.\n---\n\nbody\n",
    )
    .unwrap();
    let env = vec![("CLAUDE_CONFIG_DIR".to_owned(), config.display().to_string())];
    let armed_in = |cwd: &Path| {
        crate::hoststart::armed_as(
            true,
            &crate::hoststart::Arming {
                root: &plane,
                kit: &kit,
                harness: Some(Harness::ClaudeCode),
                cwd: Some(cwd),
                plugins: &crate::harness_plugin::Chosen::new(),
                sandbox: None,
                persona: None,
                env: &env,
            },
        )
        .unwrap()
        .0
        .join(" ")
    };
    let in_the_piece = armed_in(&tried.piece);
    assert!(in_the_piece.contains("skillOverrides"), "{in_the_piece}");
    assert!(in_the_piece.contains("profiles-own"), "{in_the_piece}");
    assert!(!armed_in(&plane).contains("profiles-own"));
}

#[test]
fn a_harness_skill_a_persona_names_is_badged_and_marked_where_it_widens_the_chat() {
    let tmp = tempfile::tempdir().unwrap();
    let folder = tmp.path().join("harness-skills/wide");
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("SKILL.md");
    std::fs::write(
        &path,
        "---\nname: wide\ndescription: Widens a chat on purpose.\nallowed-tools: Bash\n---\n",
    )
    .unwrap();
    let outside = [OutsideSkill {
        skill: Skill {
            name: "wide".to_owned(),
            description: "Widens a chat on purpose.".to_owned(),
            path: path.clone(),
        },
        root: path.parent().unwrap().to_path_buf(),
        origin: Origin::Harness,
        id: "wide".to_owned(),
        owner: Owner::You,
        switch: Switch::Alone,
        off: false,
    }];
    let set = SkillSet {
        from_the_harness: vec![FromHarness {
            reference: Reference::Named("wide".to_owned()),
            checked: true,
        }],
        ..SkillSet::default()
    };
    let names = [Reference::Named("wide".to_owned())];
    let mut rows = studio_rows(&set, tmp.path(), &names);
    badge_named(&mut rows, &[(Harness::ClaudeCode, &outside)], tmp.path());
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].compat.len(), 1);
    assert!(rows[0].compat[0].compat.widens(), "{:?}", rows[0].compat);
    assert!(
        rows[0]
            .lint
            .iter()
            .any(|line| line.starts_with("Widens the chat")),
        "{:?}",
        rows[0].lint
    );
}

// ---- links (SK-36) --------------------------------------------------------------------------

/// A plane whose project skill `deploy` has a file beside its `SKILL.md`, and a folder outside it
/// holding a `deploy` of its own: the set is read, then `swap` puts a link in place of a folder.
#[cfg(unix)]
fn swapped_after_the_set_was_read(swap: impl Fn(&Path, &Path)) -> (SkillSet, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().unwrap().keep();
    let plane = tmp.join("plane");
    let outside = tmp.join("outside");
    for (dir, said) in [(&plane, "Ship it."), (&outside, "Not this one.")] {
        let skill = dir.join("skills/deploy");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(
            skill.join("SKILL.md"),
            format!("---\nname: deploy\ndescription: {said}\n---\n"),
        )
        .unwrap();
        std::fs::write(skill.join("notes.md"), said).unwrap();
    }
    let set = of_a_chat(None, &plane, None, None, Route::Plugin);
    assert_eq!(set.effective.len(), 1, "{set:?}");
    swap(&plane, &outside);
    (set, plane, tmp)
}

#[cfg(unix)]
#[test]
fn a_lay_out_hands_nothing_through_a_folder_swapped_for_a_link_after_the_set_was_read() {
    type Swap = fn(&Path, &Path);
    let swaps: [(&str, Swap); 2] = [
        ("the skill's folder", |plane, outside| {
            let at = plane.join("skills/deploy");
            std::fs::rename(&at, plane.join("was-deploy")).unwrap();
            std::os::unix::fs::symlink(outside.join("skills/deploy"), &at).unwrap();
        }),
        ("the level's folder", |plane, outside| {
            let at = plane.join("skills");
            std::fs::rename(&at, plane.join("was-skills")).unwrap();
            std::os::unix::fs::symlink(outside.join("skills"), &at).unwrap();
        }),
    ];
    for (what, swap) in swaps {
        let (set, _plane, tmp) = swapped_after_the_set_was_read(swap);
        let honours = Harness::ClaudeCode.skill_frontmatter();
        let sets = tmp.join("sets");
        std::fs::create_dir_all(&sets).unwrap();

        assert_eq!(lay_out(&set, &sets, &honours).unwrap(), None, "{what}");
        let said = not_here_of(&set, &honours);
        assert_eq!(said, [Refusal::FolderLink], "{what}");
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}

/// Why each plane skill of `set` is handed to no chat, as a chat's tab is told.
#[cfg(unix)]
fn not_here_of(set: &SkillSet, honours: &Honours) -> Vec<Refusal> {
    set.from_the_plane()
        .filter_map(|effective| handout::of(effective, honours).err())
        .collect()
}
