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
        laid_out: None,
        binary: Path::new("/bin/purlis"),
        plugin: Some(&plugin),
        persona: None,
        graded: false,
    };
    for (harness, secrets) in [
        (Harness::ClaudeCode, "secrets"),
        (Harness::Codex, "secrets"),
        // opencode knows it by a name no other skill it reads takes.
        (Harness::Opencode, "purlis-secrets"),
    ] {
        let set = for_chat(kit, harness);

        assert!(
            set.effective.iter().any(|e| e.skill.name == secrets),
            "{harness:?}"
        );
        if harness == Harness::Opencode {
            assert!(
                !set.effective.iter().any(|e| e.skill.name == "secrets"),
                "no bare `secrets` from purlis"
            );
        }
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

    let moved = share_here(plane.path(), &Whose::Project, "scratch")
        .unwrap()
        .to;

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
        chat_settings: None,
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
    let (args, env) = crate::hoststart::armed(&crate::hoststart::Arming {
        root: &plane,
        kit: &kit,
        harness: Some(Harness::ClaudeCode),
        cwd: Some(&plane),
        plugins: &crate::harness_plugin::Chosen::new(),
        sandbox: None,
        persona: None,
        env: &[],
        graded: false,
        skills: &Box::new(crate::hoststart::ChatSkills::read(
            true,
            &plane,
            Some(Harness::ClaudeCode),
            Some(&plane),
            None,
        )),
    })
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
            prompt_key: None,
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

/// An event log that counts the ends it was told of, and keeps nothing.
#[cfg(unix)]
#[derive(Clone, Default)]
struct CountsEnds(std::sync::Arc<std::sync::atomic::AtomicUsize>);

#[cfg(unix)]
impl purlis_host::Record for CountsEnds {
    fn record(&self, _entry: serde_json::Value) -> Result<(), String> {
        Ok(())
    }

    fn ended(&self, _ended: &purlis_host::Ended<'_>) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(unix)]
#[test]
fn a_host_with_no_window_tidies_after_each_program_ends_and_keeps_a_folder_a_running_one_uses() {
    use purlis_host::Record as _;
    let tmp = tempfile::tempdir().unwrap();
    let sets = tmp.path().join("skill-sets");
    let skill = tmp.path().join("plane/skills/deploy");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(skill.join("SKILL.md"), "keep me").unwrap();
    let unused = a_set_folder(&sets, A, &skill);
    last_laid_out_long_ago(&unused);
    // Used by a program that runs: this test's own process, noted after it started.
    let used = a_set_folder(&sets, B, &skill);
    last_laid_out_long_ago(&used);
    std::fs::create_dir_all(sets.join(HELD).join(B)).unwrap();
    std::fs::write(
        sets.join(HELD).join(B).join(std::process::id().to_string()),
        "",
    )
    .unwrap();
    let ends = CountsEnds::default();
    let host_log =
        TidiesAfterEachEnd::of(ends.clone(), Some(sets.clone()), std::time::Duration::ZERO);

    host_log.ended(&purlis_host::Ended {
        project_path: tmp.path(),
        number: 1,
        exit: purlis_session_protocol::session::Exit::Code(0),
        stopped: None,
        watched: false,
    });

    assert_eq!(
        ends.0.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the end is the log's own still"
    );
    assert!(
        within_ten_seconds(|| !unused.exists()),
        "a folder no running program uses goes"
    );
    assert!(
        used.join("skills/deploy").is_symlink(),
        "one a running program uses is kept"
    );
    assert_eq!(
        std::fs::read_to_string(skill.join("SKILL.md")).unwrap(),
        "keep me"
    );
}

#[cfg(unix)]
#[test]
fn a_host_with_no_sets_folder_only_keeps_the_end() {
    use purlis_host::Record as _;
    let tmp = tempfile::tempdir().unwrap();
    let ends = CountsEnds::default();
    let host_log = TidiesAfterEachEnd::of(ends.clone(), None, std::time::Duration::ZERO);
    host_log.ended(&purlis_host::Ended {
        project_path: tmp.path(),
        number: 1,
        exit: purlis_session_protocol::session::Exit::Code(0),
        stopped: None,
        watched: true,
    });
    assert_eq!(ends.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(
        host_log.log_end().is_err(),
        "what the log does not keep, it still does not"
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
        start: None,
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
    // opencode offers, lists and types the built-in skill by the name it knows it by.
    let opencode = for_use_skill(&Running {
        harness: Harness::Opencode,
        ..running(root, &bundle, None)
    });
    assert_eq!(
        offered_names(&opencode),
        ["deploy", "purlis-handoff", "release", "review"]
    );
    assert_eq!(opencode.offered[1].known_as, "purlis-handoff");
    let salt = crate::skillspan::PromptSalt::of_key("01JB00000000000000000000C4").unwrap();
    assert_eq!(
        opencode.typed("purlis-handoff", &[], &salt).unwrap().text,
        "/purlis-handoff"
    );
    assert!(
        opencode.typed("handoff", &[], &salt).is_err(),
        "no bare `handoff`"
    );

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
        let salt = crate::skillspan::PromptSalt::of_key("01JB00000000000000000000C4").unwrap();
        let typing = chat.typed("review", &values, &salt).unwrap();
        assert_eq!(typing.text, text, "{harness:?}");
        let Start::Held(held) = typing.start else {
            panic!("a listed skill's run is held")
        };
        assert_eq!(held.skill, known_as, "{harness:?}");
        assert_eq!(held.confirmed, confirmed, "{harness:?}");
        assert_eq!(held.workspace.as_deref(), Some("smart-ide"));
        assert_eq!(held.level.map(|level| level.origin), Some(Origin::Project));
        // What a prompt holding the text is marked with.
        let sent = crate::skillspan::PromptMarks::of(&format!("{text} and more"), &salt);
        assert!(sent.holds(held.text), "{harness:?}");
        let refused = chat.typed("deploy", &[], &salt).unwrap_err();
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
    let harnesses = harnesses_in_the_studio(
        &set,
        &|route| in_the_studio_on(None, &plane, None, None, route),
        &plane,
        None,
        &[],
        &[],
        &env,
        &Known::of(&plane, None),
        Reading::Whole,
    );

    let titles: Vec<&str> = harnesses.iter().map(|h| h.harness.title()).collect();
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
    let rows: Vec<(String, Option<String>)> =
        outside_rows(on_codex, &plane, &Known::of(&plane, None), Reading::Whole)
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
fn a_repo_skill_named_as_a_built_in_one_never_replaces_it_on_codex_and_is_another_on_opencode() {
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
    let set_on = |route| {
        ask(
            Some(&built_in),
            Some(PlaneSkills {
                plane: &plane,
                workspace: Some("alpha"),
                repo: &repo,
                routed: None,
            }),
            None,
            route,
            None,
            None,
        )
    };
    let handed = |set: &SkillSet| -> Vec<(String, Origin)> {
        set.effective
            .iter()
            .map(|e| (e.skill.name.clone(), e.origin))
            .collect()
    };

    let codex = set_on(Route::Briefing);
    assert_eq!(codex.built_in_dir(), Some(built_in.as_path()));
    assert_eq!(
        handed(&codex),
        [
            ("secrets".to_owned(), Origin::BuiltIn),
            ("secrets".to_owned(), Origin::Repo),
        ],
        "Codex reads the repo's copy itself, beside purlis's (SK-37)"
    );
    assert_eq!(codex.from_the_plane().count(), 0);
    assert_eq!(codex.beside_a_built_in().count(), 1);

    // opencode knows purlis's as `purlis-secrets`, so the clone's `.agents/skills/secrets` is
    // another skill beside it, by a name of its own.
    let opencode = set_on(Route::Config);
    assert_eq!(
        handed(&opencode),
        [
            ("purlis-secrets".to_owned(), Origin::BuiltIn),
            ("secrets".to_owned(), Origin::Repo),
        ]
    );
    assert!(opencode.left_out.is_empty(), "{:?}", opencode.left_out);
}

/// A plane whose workspace `alpha` has the clone `api`, which brings its own `secrets` and
/// `purlis-secrets` in `.agents/skills`, a folder Codex and opencode read, beside a bundle with
/// purlis's built-in `secrets` (SK-37): Codex knows purlis's by the bare name, and opencode as
/// `purlis-secrets`. Its temporary folder, the plane, the clone, and the bundle's skills.
fn a_clone_with_a_secrets_skill_of_its_own() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(&plane).unwrap();
    std::fs::write(plane.join("charter.toml"), "[plane]\n").unwrap();
    let clone = plane.join("workspaces/alpha/api");
    std::fs::create_dir_all(clone.join(".git")).unwrap();
    let built_in = tmp.path().join("bundle/skills");
    for (dir, name, says) in [
        (built_in.clone(), "secrets", "purlis's"),
        (clone.join(".agents/skills"), "secrets", "repo's"),
        (clone.join(".agents/skills"), "purlis-secrets", "repo's"),
    ] {
        std::fs::create_dir_all(dir.join(name)).unwrap();
        std::fs::write(
            dir.join(name).join("SKILL.md"),
            format!(
                "---\nname: {name}\ndescription: Reads the {says} secrets from a vault.\n---\n"
            ),
        )
        .unwrap();
    }
    (tmp, plane, clone, built_in)
}

#[test]
fn a_chat_in_a_clone_with_a_skill_named_like_a_built_in_one_is_told_at_its_start() {
    // SK-37 (#3027): Codex loads the clone's `secrets` beside purlis's, and opencode the clone's
    // `purlis-secrets` beside purlis's of that name, and the model may pick the repo's. purlis
    // cannot stop that, so the chat's tab says so.
    let (tmp, plane, clone, built_in) = a_clone_with_a_secrets_skill_of_its_own();
    let env = crate::harness_plugin::Env {
        chat: &[],
        home: Some(tmp.path().join("home")),
        process: false,
    };
    let said = |harness: Harness, cwd: &Path| {
        not_here(
            &plane,
            Some("alpha"),
            None,
            Some(&built_in),
            harness,
            &env,
            Some(cwd),
        )
        .map(|not| not.beside_a_built_in)
        .unwrap_or_default()
    };

    assert_eq!(said(Harness::Codex, &clone), ["secrets"]);
    assert_eq!(said(Harness::Opencode, &clone), ["purlis-secrets"]);
    assert_eq!(
        said(Harness::ClaudeCode, &clone),
        Vec::<String>::new(),
        "Claude Code reads a repo's .claude/skills, and this clone has none there"
    );
    assert_eq!(
        said(Harness::Codex, &plane.join("workspaces/alpha")),
        Vec::<String>::new(),
        "a chat in no clone"
    );
}

#[test]
fn on_claude_code_a_repos_claude_skills_secrets_is_said_beside_purliss_own() {
    // SK-37 (#3027): Claude Code reads the clone's `.claude/skills/secrets` itself, beside the
    // `purlis:secrets` purlis hands it, so its chat's tab and studio say so as Codex's do.
    let (tmp, plane, clone, built_in) = a_clone_with_a_secrets_skill_of_its_own();
    let dir = clone.join(".claude/skills/secrets");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        "---\nname: secrets\ndescription: Reads the repo's secrets for Claude Code.\n---\n",
    )
    .unwrap();
    let env = crate::harness_plugin::Env {
        chat: &[],
        home: Some(tmp.path().join("home")),
        process: false,
    };

    let not = not_here(
        &plane,
        Some("alpha"),
        None,
        Some(&built_in),
        Harness::ClaudeCode,
        &env,
        Some(&clone),
    )
    .expect("said");
    assert_eq!(not.beside_a_built_in, ["secrets"]);

    let list = studio_list(Some(&built_in), &plane, Some("alpha"), None, &env);
    let claude = list
        .harnesses
        .iter()
        .find(|one| one.harness == Harness::ClaudeCode)
        .expect("Claude Code listed");
    let row = claude
        .rows
        .iter()
        .find(|row| row.name == "secrets" && row.origin == Origin::Repo)
        .unwrap_or_else(|| panic!("{:?}", claude.rows));
    assert_eq!(row.left_out, None);
    assert_eq!(
        claude
            .set
            .beside_a_built_in()
            .map(|clash| (clash.name.as_str(), clash.outside))
            .collect::<Vec<_>>(),
        [("secrets", Origin::Repo)]
    );
}

#[test]
fn the_studio_marks_a_repo_skill_named_as_each_program_knows_a_built_in_one() {
    // SK-37 (#3027): the repo's row is no longer said hidden, as the program loads it; its lint
    // says the name is a built-in one's, and the pair is a clash the built-in's row is marked by.
    // Codex knows purlis's `secrets` by that name, and opencode as `purlis-secrets`.
    let (tmp, plane, _clone, built_in) = a_clone_with_a_secrets_skill_of_its_own();
    let env = crate::harness_plugin::Env {
        chat: &[],
        home: Some(tmp.path().join("home")),
        process: false,
    };

    let list = studio_list(Some(&built_in), &plane, Some("alpha"), None, &env);

    for (harness, taken, other, outcome) in [
        (
            Harness::Codex,
            "secrets",
            "purlis-secrets",
            Outcome::BothLoad,
        ),
        (
            Harness::Opencode,
            "purlis-secrets",
            "secrets",
            Outcome::EitherOne,
        ),
    ] {
        let one = list
            .harnesses
            .iter()
            .find(|one| one.harness == harness)
            .unwrap_or_else(|| panic!("{harness:?} listed"));
        let row = |name: &str| {
            one.rows
                .iter()
                .find(|row| row.name == name && row.origin == Origin::Repo)
                .unwrap_or_else(|| panic!("{harness:?}: {:?}", one.rows))
        };
        assert_eq!(row(taken).left_out, None, "{harness:?}");
        assert_eq!(row(other).left_out, None, "{harness:?}");
        assert_eq!(row(other).lint, Vec::<String>::new(), "{harness:?}");
        assert_eq!(
            row(taken).lint,
            [
                "it has the name of one of purlis's built-in skills, so a chat gets purlis's too \
                 and may use either: rename it"
            ],
            "{harness:?}"
        );
        let clashes: Vec<(&str, Origin, Outcome)> = one
            .set
            .beside_a_built_in()
            .map(|clash| (clash.name.as_str(), clash.outside, clash.outcome))
            .collect();
        assert_eq!(clashes, [(taken, Origin::Repo, outcome)], "{harness:?}");
    }
    assert!(
        list.rows
            .iter()
            .any(|row| row.name == "secrets" && row.origin == Origin::BuiltIn),
        "the built-in's own row is there to be marked: {:?}",
        list.rows
    );
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
        crate::hoststart::armed(&crate::hoststart::Arming {
            root: &plane,
            kit: &kit,
            harness: Some(Harness::ClaudeCode),
            cwd: Some(cwd),
            plugins: &crate::harness_plugin::Chosen::new(),
            sandbox: None,
            persona: None,
            env: &env,
            graded: false,
            skills: &Box::new(crate::hoststart::ChatSkills::read(
                true,
                &plane,
                Some(Harness::ClaudeCode),
                Some(cwd),
                None,
            )),
        })
        .unwrap()
        .0
        .join(" ")
    };
    let in_the_piece = armed_in(&tried.piece);
    assert!(in_the_piece.contains("skillOverrides"), "{in_the_piece}");
    assert!(in_the_piece.contains("profiles-own"), "{in_the_piece}");
    assert!(!armed_in(&plane).contains("profiles-own"));
}

/// SK-16 (the review of #3229): whether a chat is held to a grade's MCP limits is the start's
/// word alone, and a start whose word and folder disagree is refused rather than started armed
/// as an ordinary chat.
#[test]
fn a_grades_limits_follow_the_start_and_a_mismatch_with_its_folder_refuses() {
    crate::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let (plane, kit) = a_host_on(tmp.path());
    std::fs::write(plane.join("charter.toml"), "schema = 1\n").unwrap();
    let copy = verbs::drafts::Snapshot {
        name: "deploy".to_owned(),
        files: [(
            "SKILL.md".to_owned(),
            "---\nname: deploy\ndescription: Ships.\n---\n\nbody\n".to_owned(),
        )]
        .into_iter()
        .collect(),
    };
    let arm = verbs::drafts::lay_out_arm(&plane, Some(&copy)).unwrap();
    let armed = |cwd: &Path, graded: bool, harness: Harness| {
        crate::hoststart::armed(&crate::hoststart::Arming {
            root: &plane,
            kit: &kit,
            harness: Some(harness),
            cwd: Some(cwd),
            plugins: &crate::harness_plugin::Chosen::new(),
            sandbox: None,
            persona: None,
            env: &[],
            graded,
            skills: &Box::new(crate::hoststart::ChatSkills::read(
                true,
                &plane,
                Some(harness),
                Some(cwd),
                None,
            )),
        })
        .map(|(args, _)| args.join(" "))
    };
    let graded = armed(&arm.piece, true, Harness::ClaudeCode).unwrap();
    assert!(graded.contains("--strict-mcp-config"), "{graded}");
    assert!(graded.contains(crate::chattools::GRADED_ARG), "{graded}");
    assert!(
        armed(&arm.piece, false, Harness::ClaudeCode).is_err(),
        "an ordinary start in a grade's folder is refused"
    );
    assert!(
        armed(&plane, true, Harness::ClaudeCode).is_err(),
        "a grade's start outside a grade's folder is refused"
    );
    assert!(
        armed(&arm.piece, true, Harness::Codex).is_err(),
        "a grade's start on a program it cannot be held on is refused"
    );
    let ordinary = armed(&plane, false, Harness::ClaudeCode).unwrap();
    assert!(!ordinary.contains("--strict-mcp-config"), "{ordinary}");
}

/// #3461: a start `armed` refuses reads none of the chat's skill folders, its clone's included:
/// the read waits until every fail-closed check has passed, and a start that passes reads once.
#[test]
fn a_start_armed_refuses_reads_no_skills_and_one_it_arms_reads_them_once() {
    crate::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let (plane, kit) = a_host_on(tmp.path());
    std::fs::write(plane.join("charter.toml"), "schema = 1\n").unwrap();
    let reads = std::cell::Cell::new(0);
    let armed = |graded: bool| {
        let skills = std::cell::LazyCell::new(|| {
            reads.set(reads.get() + 1);
            crate::hoststart::ChatSkills::read(
                true,
                &plane,
                Some(Harness::ClaudeCode),
                Some(&plane),
                None,
            )
        });
        crate::hoststart::armed(&crate::hoststart::Arming {
            root: &plane,
            kit: &kit,
            harness: Some(Harness::ClaudeCode),
            cwd: Some(&plane),
            plugins: &crate::harness_plugin::Chosen::new(),
            sandbox: None,
            persona: None,
            env: &[],
            graded,
            skills: &skills,
        })
        .map(|_| ())
    };
    assert!(
        armed(true).is_err(),
        "a grade's start outside a grade's folder"
    );
    assert_eq!(reads.get(), 0, "a refused start read the chat's skills");
    armed(false).unwrap();
    assert_eq!(
        reads.get(),
        1,
        "a start that is armed reads its skills once"
    );
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
        unread: None,
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
    badge_named(
        &mut rows,
        &[(Harness::ClaudeCode, &outside)],
        &Known::of(tmp.path(), None),
    );
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

#[test]
fn the_studio_list_of_a_workspace_chat_holds_its_rows_and_each_harnesss_own_with_their_rows() {
    // SK-34 (#3008): the one list the window and `purlis skill list` draw. A workspace chat gets
    // its workspace's skill; Codex finds one of the person's own and one in the workspace's
    // clone, each row with whose it is and its badge on Codex.
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    let ship = plane.join("workspaces/alpha/skills/ship");
    std::fs::create_dir_all(&ship).unwrap();
    std::fs::write(
        ship.join("SKILL.md"),
        "---\nname: ship\ndescription: Ships.\n---\n",
    )
    .unwrap();
    std::fs::write(plane.join("charter.toml"), "[plane]\n").unwrap();
    let clone = plane.join("workspaces/alpha/app");
    std::fs::create_dir_all(clone.join(".git")).unwrap();
    std::fs::create_dir_all(clone.join(".agents/skills/tool")).unwrap();
    std::fs::write(
        clone.join(".agents/skills/tool/SKILL.md"),
        "---\nname: tool\ndescription: The repo's tool.\n---\n",
    )
    .unwrap();
    let codex = tmp.path().join("codex-home");
    std::fs::create_dir_all(codex.join("skills/mine")).unwrap();
    std::fs::write(
        codex.join("skills/mine/SKILL.md"),
        "---\nname: mine\ndescription: Mine alone.\n---\n",
    )
    .unwrap();
    let chat = [("CODEX_HOME".to_owned(), codex.display().to_string())];
    let env = crate::harness_plugin::Env {
        chat: &chat,
        home: Some(tmp.path().join("home")),
        process: false,
    };

    let list = studio_list(None, &plane, Some("alpha"), None, &env);

    let plane_rows: Vec<(&str, Origin, bool)> = list
        .rows
        .iter()
        .map(|row| (row.name.as_str(), row.origin, row.left_out.is_none()))
        .collect();
    assert_eq!(plane_rows, [("ship", Origin::Workspace, true)]);
    let on_codex = list
        .harnesses
        .iter()
        .find(|one| one.harness == Harness::Codex)
        .expect("Codex listed");
    let rows: Vec<(&str, Origin, Option<Owner>, bool)> = on_codex
        .rows
        .iter()
        .map(|row| {
            (
                row.name.as_str(),
                row.origin,
                row.outside.as_ref().map(|o| o.owner.clone()),
                row.compat.is_empty(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("mine", Origin::Harness, Some(Owner::You), false),
            (
                "tool",
                Origin::Repo,
                Some(Owner::Repo("app".to_owned())),
                false
            ),
        ]
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

#[cfg(unix)]
#[test]
fn the_lint_reads_no_file_of_a_skill_that_is_a_link_or_has_a_second_name() {
    // SK-44 (#3133): the lint reads a skill's other files through its held folder, so a script
    // that is a link, or a hard link, to one outside the skill is never read for the programs it
    // calls. A plain script beside them still is.
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    let review = plane.join("skills/review");
    std::fs::create_dir_all(&review).unwrap();
    std::fs::write(plane.join("charter.toml"), "[plane]\n").unwrap();
    let text = "---\nname: review\ndescription: Reviews a change.\n---\n\nRun the scripts.\n";
    std::fs::write(review.join("SKILL.md"), text).unwrap();
    std::fs::write(
        review.join("plain.sh"),
        "#!/bin/sh\npurlis-sk44-plain --x\n",
    )
    .unwrap();
    let outside = tmp.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(
        outside.join("far.sh"),
        "#!/bin/sh\npurlis-sk44-linked --x\n",
    )
    .unwrap();
    std::fs::write(outside.join("twin.sh"), "#!/bin/sh\npurlis-sk44-twin --x\n").unwrap();
    std::os::unix::fs::symlink(outside.join("far.sh"), review.join("linked.sh")).unwrap();
    std::fs::hard_link(outside.join("twin.sh"), review.join("twin.sh")).unwrap();

    let said: Vec<String> = lint(&plane, &verbs::Level::project(), "review", text)
        .iter()
        .map(ToString::to_string)
        .collect();

    assert!(
        said.iter().any(|line| line.contains("purlis-sk44-plain")),
        "{said:?}"
    );
    assert!(
        !said
            .iter()
            .any(|line| line.contains("purlis-sk44-linked") || line.contains("purlis-sk44-twin")),
        "{said:?}"
    );
}

#[test]
fn a_repo_skills_folder_purlis_could_not_list_leaves_the_plane_skills_out_of_a_chat_in_the_clone() {
    // SK-44 (#3133): a chat can write a clone's skills folder past the most names purlis reads.
    // Its row is never a skill of the chat's, and since any plane skill may have a repo copy
    // there that the chat's harness would get instead, none is laid out for it.
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    let ship = plane.join("workspaces/alpha/skills/ship");
    std::fs::create_dir_all(&ship).unwrap();
    std::fs::write(
        ship.join("SKILL.md"),
        "---\nname: ship\ndescription: Ships.\n---\n",
    )
    .unwrap();
    std::fs::write(plane.join("charter.toml"), "[plane]\n").unwrap();
    let clone = plane.join("workspaces/alpha/app");
    std::fs::create_dir_all(clone.join(".git")).unwrap();
    let dir = clone.join(".agents/skills");
    std::fs::create_dir_all(&dir).unwrap();
    let sets = tmp.path().join("sets");
    let lay_out = |repo: &[OutsideSkill]| {
        lay_out_for_chat(
            Some(crate::skills::PlaneSkills {
                plane: &plane,
                workspace: Some("alpha"),
                repo,
                routed: None,
            }),
            None,
            None,
            &sets,
            Harness::Codex,
        )
    };

    let before = repo_skills_of_chat(Harness::Codex, &plane, "alpha", &clone);
    assert_eq!(before, []);
    assert!(
        lay_out(&before).is_some(),
        "the workspace's skill is handed"
    );

    for n in 0..=2000 {
        std::fs::write(dir.join(format!("f{n}")), "").unwrap();
    }
    let after = repo_skills_of_chat(Harness::Codex, &plane, "alpha", &clone);

    assert_eq!(after.len(), 1);
    assert_eq!(after[0].unread, Some(purlis_skills::Unread::Crowded));
    assert_eq!(lay_out(&after), None, "no plane skill is handed");
}

#[cfg(unix)]
#[test]
fn a_skill_of_the_persons_own_that_is_a_hard_link_is_badged_and_never_refused() {
    // SK-44 (#3133): the Nix store, `cp -al` and `rsync --link-dest` leave the person's own
    // skills as hard links. purlis reads them for its badges and lint as it lists them, so
    // their row says what each harness does with them and carries no refusal.
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(&plane).unwrap();
    std::fs::write(plane.join("charter.toml"), "[plane]\n").unwrap();
    let store = tmp.path().join("store/nix");
    std::fs::create_dir_all(&store).unwrap();
    std::fs::write(
        store.join("SKILL.md"),
        "---\nname: nix\ndescription: Comes from the store, linked in by the person.\n---\n",
    )
    .unwrap();
    let config = tmp.path().join("claude-config");
    std::fs::create_dir_all(config.join("skills/nix")).unwrap();
    std::fs::hard_link(store.join("SKILL.md"), config.join("skills/nix/SKILL.md")).unwrap();
    let chat = [("CLAUDE_CONFIG_DIR".to_owned(), config.display().to_string())];
    let env = crate::harness_plugin::Env {
        chat: &chat,
        home: Some(tmp.path().join("home")),
        process: false,
    };

    let list = studio_list(None, &plane, None, None, &env);

    let row = list
        .harnesses
        .iter()
        .find(|one| one.harness == Harness::ClaudeCode)
        .and_then(|one| one.rows.iter().find(|row| row.name == "nix"))
        .expect("nix listed");
    assert_eq!(row.compat.len(), 1, "{:?}", row.compat);
    assert!(!row.compat[0].compat.widens(), "{:?}", row.compat);
    assert_eq!(row.lint, Vec::<String>::new());
}

/// A plane whose workspace `alpha` has the skills `review` and `ship`, and the clones `api`,
/// which brings its own `review` for Codex, and `web`, whose skills folder holds more names than
/// purlis reads (SK-53). Its temporary folder, the plane, and the two clones.
fn a_workspace_whose_repos_leave_skills_out() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(&plane).unwrap();
    std::fs::write(plane.join("charter.toml"), "[plane]\n").unwrap();
    let api = plane.join("workspaces/alpha/api");
    let web = plane.join("workspaces/alpha/web");
    for clone in [&api, &web] {
        std::fs::create_dir_all(clone.join(".git")).unwrap();
        std::fs::create_dir_all(clone.join(".agents/skills")).unwrap();
    }
    for (dir, name) in [
        (plane.join("workspaces/alpha/skills"), "review"),
        (plane.join("workspaces/alpha/skills"), "ship"),
        (api.join(".agents/skills"), "review"),
    ] {
        std::fs::create_dir_all(dir.join(name)).unwrap();
        std::fs::write(
            dir.join(name).join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Does {name}.\n---\n"),
        )
        .unwrap();
    }
    for n in 0..=2000 {
        std::fs::write(web.join(".agents/skills").join(format!("f{n}")), "").unwrap();
    }
    (tmp, plane, api, web)
}

/// What the studio and `purlis skill list` say of a skill `api` leaves out (#3234: by name).
const HIDDEN_BY_THE_REPO: &str = "hidden by the skill of this name in the repo api";
const WEB_UNREAD: &str = "purlis could not read the skills folder of web whole, so a copy of \
                          this skill may be there, and a chat in that repo would get that one";
/// What the tab of a chat in that repo says of the same (#3234), after "because of the repo it
/// works in".
const API_HAS_ITS_OWN: &str = "the repo api has its own copy, which loads in its place";
const WEB_UNREAD_TO_ITS_CHAT: &str = "purlis could not read the skills folder of web whole, so \
                                      it is left out in case the repo has its own";

#[test]
fn a_chat_in_a_clone_is_told_which_skills_its_repo_leaves_out_and_why() {
    // SK-53 (#3213), decision 41: never a silent drop. A Codex chat in `api` gets the repo's
    // `review` in place of the workspace's (decision 51); one in `web` gets none of the
    // workspace's, since purlis could not read that clone's skills folder whole (SK-44).
    let (tmp, plane, api, web) = a_workspace_whose_repos_leave_skills_out();
    let env = crate::harness_plugin::Env {
        chat: &[],
        home: Some(tmp.path().join("home")),
        process: false,
    };
    let said = |harness: Harness, cwd: &Path| {
        not_here(&plane, Some("alpha"), None, None, harness, &env, Some(cwd))
            .map(|not| {
                not.by_a_repo
                    .into_iter()
                    .map(|one| (one.name, one.origin, one.why.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };

    assert_eq!(
        said(Harness::Codex, &api),
        [(
            "review".to_owned(),
            Origin::Workspace,
            API_HAS_ITS_OWN.to_owned()
        )]
    );
    let mut in_web = said(Harness::Codex, &web);
    in_web.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        in_web,
        [
            (
                "review".to_owned(),
                Origin::Workspace,
                WEB_UNREAD_TO_ITS_CHAT.to_owned()
            ),
            (
                "ship".to_owned(),
                Origin::Workspace,
                WEB_UNREAD_TO_ITS_CHAT.to_owned()
            ),
        ]
    );
    assert_eq!(
        said(Harness::ClaudeCode, &web),
        [],
        "Claude Code's chats get the workspace's skills as their own plugin"
    );
    assert_eq!(
        said(Harness::Codex, &plane.join("workspaces/alpha")),
        [],
        "a chat in no clone"
    );
}

#[test]
fn a_chats_tab_says_what_the_repo_skills_it_was_armed_with_leave_out_and_reads_the_clone_no_more() {
    // #3234: the host reads a chat's scope and its clone's repo skills once, at its start, and
    // what arms it and what its tab says are both worked out from that one read.
    let (_tmp, plane, api, _web) = a_workspace_whose_repos_leave_skills_out();
    let kit = crate::hoststart::Kit {
        binary: None,
        plugin: None,
        git_hooks: None,
        skill_sets: None,
        chat_settings: None,
    };
    let read =
        crate::hoststart::ChatSkills::read(true, &plane, Some(Harness::Codex), Some(&api), None);
    let said = |skills: &crate::hoststart::ChatSkills| {
        crate::hoststart::skills_not_here(skills, &kit, Some(Harness::Codex), &[])
            .map(|not| {
                not.by_a_repo
                    .into_iter()
                    .map(|one| (one.name, one.why.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };

    assert_eq!(read.workspace.as_deref(), Some("alpha"));
    assert!(
        read.repo.iter().any(|one| one.id == "review"),
        "{:?}",
        read.repo
    );
    assert_eq!(
        said(&read),
        [("review".to_owned(), API_HAS_ITS_OWN.to_owned())]
    );
    let handed_none = crate::hoststart::ChatSkills {
        repo: Vec::new(),
        ..read.clone()
    };
    assert_eq!(said(&handed_none), [], "the tab reads no clone of its own");
    let off =
        crate::hoststart::ChatSkills::read(false, &plane, Some(Harness::Codex), Some(&api), None);
    assert!(
        off.repo.is_empty(),
        "nothing is read where the plane's skills reach no chat"
    );
    assert_eq!(said(&off), []);
}

#[test]
fn the_studio_lists_under_each_harness_the_skills_a_repo_leaves_out_of_its_chats() {
    // SK-53 (#3213): the plane's rows are a set worked out with no repo skills, so what a repo
    // takes from a Codex or opencode chat is listed under that harness, with why. `api`'s copy
    // took `review` first, so `web` is why `ship` alone is left out.
    let (tmp, plane, _api, _web) = a_workspace_whose_repos_leave_skills_out();
    let env = crate::harness_plugin::Env {
        chat: &[],
        home: Some(tmp.path().join("home")),
        process: false,
    };

    let list = studio_list(None, &plane, Some("alpha"), None, &env);

    let left_out = |harness: Harness| {
        let mut rows: Vec<(String, Origin, Option<String>)> = list
            .harnesses
            .iter()
            .find(|one| one.harness == harness)
            .unwrap_or_else(|| panic!("{harness:?} listed"))
            .repo_left_out
            .iter()
            .map(|row| (row.name.clone(), row.origin, row.left_out.clone()))
            .collect();
        rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(&b.2)));
        rows
    };
    for harness in [Harness::Codex, Harness::Opencode] {
        assert_eq!(
            left_out(harness),
            [
                (
                    "review".to_owned(),
                    Origin::Workspace,
                    Some(HIDDEN_BY_THE_REPO.to_owned())
                ),
                (
                    "ship".to_owned(),
                    Origin::Workspace,
                    Some(WEB_UNREAD.to_owned())
                ),
            ],
            "{harness:?}"
        );
    }
    assert_eq!(left_out(Harness::ClaudeCode), []);
    assert!(
        list.rows
            .iter()
            .filter(|row| row.name == "ship")
            .all(|row| row.left_out.is_none()),
        "the plane's own rows are a chat's in no clone"
    );
}

/// What Codex's own sidecar says of a skill it finds itself, keeping it out of its model's
/// choice (SK-39): the fixture `agents/openai.yaml`.
const CODEX_MANUAL_ONLY: &str =
    "interface:\n  display_name: Deploy\npolicy:\n  allow_implicit_invocation: false\n";

/// What a badge says of a skill its harness runs only when asked for.
const RUNS_ONLY_WHEN_ASKED: &str = "Runs only when asked for by name.";

/// The skill `name` in `dir`, with Codex's sidecar saying `sidecar` beside it where given.
fn a_skill_with_a_sidecar(dir: &Path, name: &str, sidecar: Option<&str>) {
    let folder = dir.join(name);
    std::fs::create_dir_all(folder.join("agents")).unwrap();
    std::fs::write(
        folder.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Does {name}.\n---\n"),
    )
    .unwrap();
    if let Some(sidecar) = sidecar {
        std::fs::write(folder.join("agents/openai.yaml"), sidecar).unwrap();
    }
}

/// The badge lines `harness` gives the row `name` among `rows`.
fn said_on(rows: &[StudioRow], name: &str, harness: Harness) -> Vec<String> {
    rows.iter()
        .find(|row| row.name == name)
        .unwrap_or_else(|| panic!("{name} listed"))
        .compat
        .iter()
        .find(|badge| badge.harness == harness)
        .unwrap_or_else(|| panic!("{name} badged on {harness:?}"))
        .compat
        .lines()
}

#[test]
fn a_codex_skill_its_sidecar_keeps_out_of_the_models_choice_says_so_on_its_badge() {
    // SK-39 (#3050): Codex keeps a skill it finds itself, the person's own or a repo's, out of
    // its catalogue by `agents/openai.yaml`, and its badge says it runs only when asked for. A
    // skill purlis hands is briefed, never discovered, so its sidecar is never read; nor does
    // another harness read Codex's file.
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(&plane).unwrap();
    std::fs::write(plane.join("charter.toml"), "[plane]\n").unwrap();
    let api = plane.join("workspaces/alpha/api");
    std::fs::create_dir_all(api.join(".git")).unwrap();
    let repo = api.join(".agents/skills");
    a_skill_with_a_sidecar(&repo, "deploy", Some(CODEX_MANUAL_ONLY));
    a_skill_with_a_sidecar(
        &repo,
        "lint",
        Some("policy:\n  allow_implicit_invocation: true\n"),
    );
    a_skill_with_a_sidecar(&repo, "plain", None);
    let codex_home = tmp.path().join("codex-home");
    a_skill_with_a_sidecar(&codex_home.join("skills"), "mine", Some(CODEX_MANUAL_ONLY));
    a_skill_with_a_sidecar(
        &plane.join("workspaces/alpha/skills"),
        "ship",
        Some(CODEX_MANUAL_ONLY),
    );
    let chat = [("CODEX_HOME".to_owned(), codex_home.display().to_string())];
    let env = crate::harness_plugin::Env {
        chat: &chat,
        home: Some(tmp.path().join("home")),
        process: false,
    };

    let list = studio_list(None, &plane, Some("alpha"), None, &env);

    let rows_of = |harness: Harness| {
        list.harnesses
            .iter()
            .find(|one| one.harness == harness)
            .map(|one| one.rows.clone())
            .unwrap_or_default()
    };
    let codex = rows_of(Harness::Codex);
    for name in ["deploy", "mine"] {
        assert_eq!(
            said_on(&codex, name, Harness::Codex),
            ["Everything it asks for applies.", RUNS_ONLY_WHEN_ASKED],
            "{name}"
        );
    }
    for name in ["lint", "plain"] {
        assert_eq!(
            said_on(&codex, name, Harness::Codex),
            ["Everything it asks for applies."],
            "{name}"
        );
    }
    let opencode = rows_of(Harness::Opencode);
    assert_eq!(
        said_on(&opencode, "deploy", Harness::Opencode),
        ["Everything it asks for applies."],
        "opencode reads no such file"
    );
    for harness in Harness::ALL {
        assert!(
            !said_on(&list.rows, "ship", harness).contains(&RUNS_ONLY_WHEN_ASKED.to_owned()),
            "{harness:?}: a skill purlis hands is never read by its sidecar"
        );
    }
}

#[test]
fn a_codex_sidecar_purlis_cannot_read_plainly_leaves_its_skill_listed() {
    // SK-39: failing closed, a sidecar that is a link, or that Codex's own reader might not read
    // as manual-only, says nothing, as Codex then lists the skill.
    let tmp = tempfile::tempdir().unwrap();
    let skills = tmp.path().join("skills");
    let elsewhere = tmp.path().join("elsewhere.yaml");
    std::fs::write(&elsewhere, CODEX_MANUAL_ONLY).unwrap();
    a_skill_with_a_sidecar(&skills, "linked", None);
    std::os::unix::fs::symlink(&elsewhere, skills.join("linked/agents/openai.yaml")).unwrap();
    for (name, sidecar) in [
        (
            "anchored",
            "policy: &p\n  allow_implicit_invocation: false\nagain: *p\n",
        ),
        (
            "twice",
            "policy:\n  allow_implicit_invocation: false\n  allow_implicit_invocation: true\n",
        ),
        (
            "quoted",
            "policy:\n  allow_implicit_invocation: \"false\"\n",
        ),
        ("not-yaml", "policy: [\n"),
    ] {
        a_skill_with_a_sidecar(&skills, name, Some(sidecar));
    }
    a_skill_with_a_sidecar(&skills, "plain", Some(CODEX_MANUAL_ONLY));
    let said = |name: &str| {
        let path = skills.join(name).join("SKILL.md");
        badges((&skills, &path), Origin::Repo, &[Harness::Codex])[0]
            .compat
            .lines()
    };
    assert_eq!(
        said("plain"),
        ["Everything it asks for applies.", RUNS_ONLY_WHEN_ASKED],
        "the same file, read plainly, says it"
    );
    for name in ["linked", "anchored", "twice", "quoted", "not-yaml"] {
        assert_eq!(said(name), ["Everything it asks for applies."], "{name}");
    }
}

#[test]
fn a_run_of_purlis_secrets_is_the_built_in_skill_s_on_opencode_and_a_project_skill_s_elsewhere() {
    // On opencode purlis's `secrets` goes by `purlis-secrets` and a plane skill of that name is
    // left out, so the run is purlis's; on Claude Code and Codex a plane skill may be named so,
    // and a run of it is that skill's, never the built-in one's.
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    let built_in = tmp.path().join("bundle/skills");
    let write = |dir: PathBuf, name: &str| {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Does {name}.\n---\n"),
        )
        .unwrap();
    };
    write(built_in.join("secrets"), "secrets");
    write(plane.join("skills/purlis-secrets"), "purlis-secrets");
    let level = |harness: Harness, skill: &str| {
        whose_with(Some(&built_in), &plane, None, None, Some(harness), skill)
            .level
            .map(|level| level.origin)
    };

    assert_eq!(
        level(Harness::Opencode, "purlis-secrets"),
        Some(Origin::BuiltIn)
    );
    for harness in [Harness::ClaudeCode, Harness::Codex] {
        assert_eq!(
            level(harness, "purlis-secrets"),
            Some(Origin::Project),
            "{harness:?}"
        );
        assert_eq!(
            level(harness, "secrets"),
            Some(Origin::BuiltIn),
            "{harness:?}"
        );
    }

    // A clone's own `purlis-secrets` on Claude Code or Codex, with no plane skill of that name,
    // is no copy purlis knows: never counted as the built-in one.
    std::fs::remove_dir_all(plane.join("skills/purlis-secrets")).unwrap();
    for harness in [Harness::ClaudeCode, Harness::Codex] {
        assert_eq!(level(harness, "purlis-secrets"), None, "{harness:?}");
    }
    assert_eq!(
        level(Harness::Opencode, "purlis-secrets"),
        Some(Origin::BuiltIn)
    );
}

/// #3462: every persona's own rows come from one read of its own levels, so a local skill of
/// the project or a workspace is never asked about, and each persona's local skill once.
#[test]
fn persona_rows_asks_of_each_personas_own_local_skills_once_and_of_no_others() {
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(plane.join("workspaces/alpha")).unwrap();
    let write = |dir: &Path, name: &str| {
        std::fs::create_dir_all(dir.join(name)).unwrap();
        std::fs::write(
            dir.join(name).join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Does {name}.\n---\n"),
        )
        .unwrap();
    };
    write(&plane.join("skills.local"), "scratch");
    write(&plane.join("workspaces/alpha/skills.local"), "notes");
    for persona in ["ops", "reviewer"] {
        let at = plane.join("personas").join(persona);
        std::fs::create_dir_all(&at).unwrap();
        std::fs::write(at.join("persona.md"), "---\n---\n").unwrap();
        write(&at.join("skills"), &format!("{persona}-shared"));
        write(&at.join("skills.local"), &format!("{persona}-local"));
    }
    let asked = std::sync::Mutex::new(Vec::<PathBuf>::new());
    let kept = |folder: &Path| {
        asked.lock().unwrap().push(folder.to_path_buf());
        Ok(())
    };

    let rows = persona_rows_with(None, &plane, &kept);

    let mut asked = asked.into_inner().unwrap();
    asked.sort();
    assert_eq!(
        asked,
        [
            plane.join("personas/ops/skills.local/ops-local"),
            plane.join("personas/reviewer/skills.local/reviewer-local"),
        ]
    );
    let read: Vec<(&str, Vec<(&str, bool)>)> = rows
        .iter()
        .map(|one| {
            (
                one.persona.as_str(),
                one.rows
                    .iter()
                    .map(|row| (row.name.as_str(), row.local))
                    .collect(),
            )
        })
        .collect();
    assert_eq!(
        read,
        [
            ("ops", vec![("ops-local", true), ("ops-shared", false)]),
            (
                "reviewer",
                vec![("reviewer-local", true), ("reviewer-shared", false)]
            ),
        ]
    );
}

/// #3462: the clones a chat works in are the ones its folder is in, which the Skills view
/// narrows its repo skills to: none for a chat at its workspace's own folder.
#[test]
fn a_chat_works_in_the_clone_its_folder_is_in_and_in_none_at_its_workspaces_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    for repo in ["api", "web"] {
        std::fs::create_dir_all(plane.join("workspaces/alpha").join(repo).join(".git")).unwrap();
    }
    std::fs::create_dir_all(plane.join("workspaces/alpha/api/src")).unwrap();
    let names = |cwd: &Path| -> Vec<String> {
        clones_of_chat(&plane, "alpha", cwd)
            .into_iter()
            .map(|repo| repo.name)
            .collect()
    };
    assert_eq!(names(&plane.join("workspaces/alpha/api/src")), ["api"]);
    assert_eq!(names(&plane.join("workspaces/alpha/web")), ["web"]);
    assert!(names(&plane.join("workspaces/alpha")).is_empty());
}

/// A branch folder of clone `repo` of workspace `alpha`, as git leaves a linked worktree: its
/// `.git` is a file.
fn branch_folder(plane: &Path, repo: &str, piece: &str) -> std::path::PathBuf {
    let folder = plane
        .join("workspaces/alpha/.worktrees")
        .join(repo)
        .join(piece);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join(".git"), "gitdir: elsewhere\n").unwrap();
    folder
}

fn a_repo_skill(checkout: &Path, name: &str) {
    let dir = checkout.join(".agents/skills").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {name}.\n---\n"),
    )
    .unwrap();
}

/// #4238: a chat in a branch folder works in the clone the folder was cut from, and its harness
/// reads the folder's checkout: a skill only that branch holds counts, and one only the clone's
/// own branch holds does not.
#[test]
fn a_chat_in_a_branch_folder_counts_the_repo_skills_of_that_folder_under_its_clones_name() {
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    let clone = plane.join("workspaces/alpha/api");
    std::fs::create_dir_all(clone.join(".git")).unwrap();
    std::fs::create_dir_all(plane.join("workspaces/alpha/web/.git")).unwrap();
    a_repo_skill(&clone, "on-main");
    a_repo_skill(&clone, "on-both");
    let folder = branch_folder(&plane, "api", "fix");
    a_repo_skill(&folder, "on-the-branch");
    a_repo_skill(&folder, "on-both");
    std::fs::create_dir_all(folder.join("src")).unwrap();

    for cwd in [folder.clone(), folder.join("src")] {
        let mine = checkouts_of_chat(&plane, "alpha", &cwd);
        assert_eq!(
            mine.iter()
                .map(|repo| repo.name.as_str())
                .collect::<Vec<_>>(),
            ["api"]
        );
        let mut read: Vec<(String, std::path::PathBuf, Owner)> =
            repo_skills_of_chat(Harness::Codex, &plane, "alpha", &cwd)
                .into_iter()
                .map(|one| {
                    let at = one.skill.path.canonicalize().unwrap();
                    (one.id, at, one.owner)
                })
                .collect();
        read.sort_by(|a, b| a.0.cmp(&b.0));
        let at = folder.canonicalize().unwrap();
        let owner = Owner::Repo("api".to_owned());
        assert_eq!(
            read,
            [
                (
                    "on-both".to_owned(),
                    at.join(".agents/skills/on-both/SKILL.md"),
                    owner.clone()
                ),
                (
                    "on-the-branch".to_owned(),
                    at.join(".agents/skills/on-the-branch/SKILL.md"),
                    owner.clone()
                ),
            ]
        );
    }
    // What a start reads for its budget and its set is the same.
    let started =
        crate::hoststart::ChatSkills::read(true, &plane, Some(Harness::Codex), Some(&folder), None);
    let mut ids: Vec<&str> = started.repo.iter().map(|one| one.id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(ids, ["on-both", "on-the-branch"]);
    // The Skills view's rows are read from the clone's folder, so they do not speak for this chat.
    assert!(clones_of_chat(&plane, "alpha", &folder).is_empty());
    // The clone's own chat still reads the clone.
    let own: Vec<String> = repo_skills_of_chat(Harness::Codex, &plane, "alpha", &clone)
        .into_iter()
        .map(|one| one.id)
        .collect();
    assert!(own.contains(&"on-main".to_owned()), "{own:?}");
}

/// #4238: only a checkout purlis's layout names is a branch folder. Each of these leaves the chat
/// in no clone, as it was.
#[test]
fn a_chat_is_in_no_clone_in_what_is_not_a_branch_folder_of_one() {
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(plane.join("workspaces/alpha/api/.git")).unwrap();
    let names = |cwd: &Path| -> Vec<String> {
        checkouts_of_chat(&plane, "alpha", cwd)
            .into_iter()
            .map(|repo| repo.name)
            .collect()
    };
    // The folders above a branch folder.
    let folder = branch_folder(&plane, "api", "fix");
    assert_eq!(names(&folder), ["api"]);
    assert!(names(&plane.join("workspaces/alpha/.worktrees")).is_empty());
    assert!(names(&plane.join("workspaces/alpha/.worktrees/api")).is_empty());
    // A branch folder of a clone this workspace does not hold.
    assert!(names(&branch_folder(&plane, "gone", "fix")).is_empty());
    // A folder that is no checkout: nothing marks it a repo's top to its harness.
    let plain = plane.join("workspaces/alpha/.worktrees/api/copied");
    std::fs::create_dir_all(&plain).unwrap();
    assert!(names(&plain).is_empty());
    // A folder with a `.git` directory is a clone of its own, not one cut from `api`.
    let nested = plane.join("workspaces/alpha/.worktrees/api/nested");
    std::fs::create_dir_all(nested.join(".git")).unwrap();
    assert!(names(&nested).is_empty());
}

/// #4238: a link where a branch folder would be is not one: the chat's folder resolves to where
/// the link leads, and purlis reads no skills through it.
#[cfg(unix)]
#[test]
fn a_link_in_the_branch_folders_layout_puts_a_chat_in_no_clone() {
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(plane.join("workspaces/alpha/api/.git")).unwrap();
    let away = tmp.path().join("away");
    std::fs::create_dir_all(&away).unwrap();
    std::fs::write(away.join(".git"), "gitdir: elsewhere\n").unwrap();
    a_repo_skill(&away, "from-away");

    let parent = plane.join("workspaces/alpha/.worktrees/api");
    std::fs::create_dir_all(&parent).unwrap();
    let at_the_piece = parent.join("linked");
    std::os::unix::fs::symlink(&away, &at_the_piece).unwrap();
    assert!(checkouts_of_chat(&plane, "alpha", &at_the_piece).is_empty());

    // The same, one level up: `.worktrees` itself leads elsewhere.
    let other = tmp.path().join("other");
    std::fs::create_dir_all(plane.join("workspaces/beta/api/.git")).unwrap();
    std::fs::create_dir_all(other.join("api/fix")).unwrap();
    std::fs::write(other.join("api/fix/.git"), "gitdir: elsewhere\n").unwrap();
    std::os::unix::fs::symlink(&other, plane.join("workspaces/beta/.worktrees")).unwrap();
    let through = plane.join("workspaces/beta/.worktrees/api/fix");
    assert!(checkouts_of_chat(&plane, "beta", &through).is_empty());
}

/// The harnesses purlis ships and `goose`, a harness a project declares, taking MCP servers or
/// not.
fn with_goose(mcp: bool) -> crate::harness_declaration::Declarations {
    let goose = format!(
        "name = \"goose\"\ntitle = \"Goose\"\nprogram = \"goose\"\n\n[session]\n\n\
         [terminal]\nready_to_type = \"raw-and-quiet\"\n\n[levels]\n\n[capabilities]\n\
         mcp_servers = \"{}\"\n",
        if mcp { "yes" } else { "no: it takes none" }
    );
    let mut declared = crate::harness_declaration::builtins().to_vec();
    declared.push(
        crate::harness_declaration::parse(
            &goose,
            crate::harness_declaration::Origin::Project,
            "harnesses/goose.toml",
        )
        .expect("a declaration"),
    );
    crate::harness_declaration::Declarations {
        declared,
        refused: Vec::new(),
    }
}

#[test]
fn a_harness_with_neither_a_skill_format_nor_mcp_is_told_skills_are_not_supported() {
    let declarations = with_goose(false);
    assert_eq!(
        route_of(Some("goose"), &declarations),
        None,
        "it gets no skills"
    );
    let said = not_supported(&declarations);
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!(
        (said[0].name.as_str(), said[0].title.as_str()),
        ("goose", "Goose")
    );
    assert_eq!(
        said[0].said,
        "Skills are not supported on Goose: it has no skill format purlis can hand skills by, \
         and its declaration does not say it takes MCP servers, so its chats get no skills."
    );
    assert!(!said[0].said.contains('\n'), "one line");
}

#[test]
fn a_harness_purlis_ships_or_one_that_takes_mcp_is_never_said_unsupported() {
    assert!(not_supported(&with_goose(true)).is_empty());
    for shipped in crate::harness_declaration::builtins() {
        assert_eq!(not_supported_on(shipped), None, "{}", shipped.name);
    }
}

/// Spec #3679 S12: the names compiled in as purlis's own are its bundle's skills, each folder
/// named as its skill, so a name stays reserved where no bundle is found beside the binary.
#[test]
fn the_shipped_names_are_the_bundles_skills() {
    let bundle = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/src-tauri/plugin/skills");
    let skills = crate::skills::read(&bundle);
    let names: Vec<&str> = skills.iter().map(|skill| skill.name.as_str()).collect();
    assert_eq!(names, crate::skills::SHIPPED);
    for skill in &skills {
        assert_eq!(
            skill.path.parent().and_then(Path::file_name),
            Some(std::ffi::OsStr::new(&skill.name)),
            "{skill:?}"
        );
    }
}

/// #3686: Make my own copy copies a built-in skill's `SKILL.md` alone, so every built-in skill
/// is that one file. One that ships more files makes the copy carry them first.
#[test]
fn each_built_in_skill_is_its_skill_md_alone() {
    let bundle = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/src-tauri/plugin/skills");
    for skill in crate::skills::read(&bundle) {
        let folder = skill.path.parent().unwrap();
        let held: Vec<String> = std::fs::read_dir(folder)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(held, ["SKILL.md"], "{}", skill.name);
    }
}

/// #3777: the description Make my own copy proposes for a copy, each built-in skill's with the
/// note `copyDescriptionOf` (`app/src/SkillCopy.tsx`) adds, is one the core takes, so the copy
/// is made in one click whichever built-in it is of.
#[test]
fn each_built_in_skills_proposed_copy_description_keeps_the_rules() {
    let bundle = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/src-tauri/plugin/skills");
    let skills = crate::skills::read(&bundle);
    assert!(!skills.is_empty());
    for skill in skills {
        let proposed = format!("{} (your copy of {})", skill.description.trim(), skill.name);
        assert_eq!(
            purlis_skills::change::description_problem(&proposed),
            None,
            "{}",
            skill.name
        );
        assert!(purlis_skills::fence::holds(&proposed), "{}", skill.name);
    }
}

#[test]
fn a_built_in_skills_name_is_reserved_with_or_without_a_bundle() {
    let bundle = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(bundle.path().join("bundled")).unwrap();
    std::fs::write(
        bundle.path().join("bundled/SKILL.md"),
        "---\nname: bundled\ndescription: Comes with purlis.\n---\n",
    )
    .unwrap();
    let with = Some(bundle.path());
    for name in [
        "secrets",
        "browser",
        "skill-index",
        "purlis-secrets",
        "purlis-skill-index",
    ] {
        assert!(crate::skills::name_reserved(None, name), "{name}");
        assert!(crate::skills::name_reserved(with, name), "{name}");
    }
    assert!(crate::skills::name_reserved(with, "bundled"));
    assert!(crate::skills::name_reserved(with, "purlis-bundled"));
    assert!(!crate::skills::name_reserved(None, "bundled"));
    for name in [
        "deploy",
        "purlis",
        "purlis-deploy",
        "my-secrets",
        "secrets-two",
    ] {
        assert!(!crate::skills::name_reserved(with, name), "{name}");
    }
}

#[test]
fn the_studio_lints_the_planes_own_skills_a_chat_gets_once_per_list() {
    // #3780: Needs you's problems in a skill hold the plane's own too. The prototype's deploy,
    // whose step names a script it never shipped; a clean skill beside it says nothing.
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(plane.join("skills/deploy/scripts")).unwrap();
    std::fs::create_dir_all(plane.join("skills/review")).unwrap();
    std::fs::write(plane.join("charter.toml"), "[plane]\n").unwrap();
    std::fs::write(
        plane.join("skills/deploy/SKILL.md"),
        "---\nname: deploy\ndescription: Deploys the app to staging, then to production.\n---\n\n\
         1. Run scripts/build.sh.\n2. Check it.\n3. Tag it with scripts/tag.sh.\n4. Then scripts/kept.sh.\n",
    )
    .unwrap();
    std::fs::write(plane.join("skills/deploy/scripts/build.sh"), "#!/bin/sh\n").unwrap();
    // A link the listing does not follow is there all the same.
    #[cfg(unix)]
    std::os::unix::fs::symlink("build.sh", plane.join("skills/deploy/scripts/kept.sh")).unwrap();
    #[cfg(not(unix))]
    std::fs::write(plane.join("skills/deploy/scripts/kept.sh"), "").unwrap();
    std::fs::write(
        plane.join("skills/review/SKILL.md"),
        "---\nname: review\ndescription: Reviews a change before it is merged.\n---\n",
    )
    .unwrap();
    let env = crate::harness_plugin::Env {
        chat: &[],
        home: Some(tmp.path().join("home")),
        process: false,
    };

    let list = studio_list(None, &plane, None, None, &env);

    let lint_of = |name: &str| {
        list.rows
            .iter()
            .find(|row| row.name == name && row.origin == Origin::Project)
            .map(|row| row.lint.clone())
            .unwrap_or_else(|| panic!("{name} listed: {:?}", list.rows))
    };
    assert_eq!(
        lint_of("deploy"),
        [
            "it names scripts/tag.sh, which its folder doesn't have: a chat told to run it finds none"
        ]
    );
    assert_eq!(lint_of("review"), Vec::<String>::new());
}

#[test]
fn a_list_read_for_needs_you_has_every_row_lint_and_clash_of_the_whole_and_no_badge() {
    // #3781: the persona view's count reads only what Needs you's items are worked out from.
    let (tmp, plane, _clone, built_in) = a_clone_with_a_secrets_skill_of_its_own();
    std::fs::create_dir_all(plane.join("workspaces/alpha/skills/deploy")).unwrap();
    std::fs::write(
        plane.join("workspaces/alpha/skills/deploy/SKILL.md"),
        "---\nname: deploy\ndescription: Deploys.\n---\n",
    )
    .unwrap();
    let env = crate::harness_plugin::Env {
        chat: &[],
        home: Some(tmp.path().join("home")),
        process: false,
    };
    type Seen = (
        String,
        Origin,
        bool,
        Option<String>,
        Option<String>,
        Vec<String>,
    );
    let seen = |rows: &[StudioRow]| -> Vec<Seen> {
        rows.iter()
            .map(|row| {
                (
                    row.name.clone(),
                    row.origin,
                    row.local,
                    row.left_out.clone(),
                    row.accept.clone(),
                    row.lint.clone(),
                )
            })
            .collect()
    };
    let read = |reading| {
        studio_list_for(
            reading,
            Some(&built_in),
            &plane,
            Some("alpha"),
            None,
            &env,
            &[],
        )
    };

    let whole = read(Reading::Whole);
    let needs = read(Reading::Needs);

    assert_eq!(seen(&needs.rows), seen(&whole.rows));
    assert!(
        whole.rows.iter().any(|row| !row.lint.is_empty()),
        "deploy's short description is linted: {:?}",
        seen(&whole.rows)
    );
    for (one, other) in needs.harnesses.iter().zip(&whole.harnesses) {
        assert_eq!(seen(&one.rows), seen(&other.rows), "{:?}", one.harness);
        assert_eq!(one.set.clashes, other.set.clashes, "{:?}", one.harness);
    }
    assert!(
        whole
            .harnesses
            .iter()
            .any(|one| !one.set.clashes.is_empty()),
        "the repo's secrets clashes"
    );
    assert!(needs.rows.iter().all(|row| row.compat.is_empty()));
    assert!(
        needs
            .harnesses
            .iter()
            .flat_map(|one| &one.rows)
            .all(|row| row.compat.is_empty())
    );
}

/// A lint that answers many skills at once, recording each question: one finding per skill.
#[derive(Default)]
struct AtOnce {
    asked: std::cell::RefCell<Vec<Vec<String>>>,
}

impl purlis_skills::SkillLint for AtOnce {
    fn findings(&self, skill: &purlis_skills::SkillFiles<'_>) -> Vec<Finding> {
        self.findings_of_each(std::slice::from_ref(skill))
            .pop()
            .unwrap_or_default()
    }

    fn findings_of_each(&self, skills: &[purlis_skills::SkillFiles<'_>]) -> Vec<Vec<Finding>> {
        self.asked
            .borrow_mut()
            .push(skills.iter().map(|it| it.name.to_owned()).collect());
        skills
            .iter()
            .map(|skill| {
                vec![Finding {
                    severity: purlis_skills::Severity::Warns,
                    lint: purlis_skills::Lint::Said {
                        by: "House style".to_owned(),
                        says: format!("{} was asked with the others", skill.name),
                    },
                }]
            })
            .collect()
    }
}

#[test]
fn a_studio_list_hands_each_extension_lint_every_plane_skill_at_once_4195() {
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    for name in ["deploy", "review"] {
        std::fs::create_dir_all(plane.join("skills").join(name)).unwrap();
        std::fs::write(
            plane.join("skills").join(name).join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Does the {name} work when asked to.\n---\n"),
        )
        .unwrap();
    }
    std::fs::write(plane.join("skills/deploy/notes.md"), "Some notes.\n").unwrap();
    std::fs::write(plane.join("charter.toml"), "[plane]\n").unwrap();
    let env = crate::harness_plugin::Env {
        chat: &[],
        home: Some(tmp.path().join("home")),
        process: false,
    };
    let lint = AtOnce::default();

    let list = studio_list_for(Reading::Whole, None, &plane, None, None, &env, &[&lint]);

    let mut asked = lint.asked.borrow().clone();
    assert_eq!(asked.len(), 1, "one question for the list: {asked:?}");
    asked[0].sort();
    assert_eq!(asked[0], ["deploy", "review"]);
    for name in ["deploy", "review"] {
        let row = list
            .rows
            .iter()
            .find(|row| row.name == name)
            .expect("its row");
        assert!(
            row.lint
                .iter()
                .any(|line| line.contains(&format!("{name} was asked with the others"))),
            "{name}: {:?}",
            row.lint
        );
    }
}

#[test]
fn each_harness_s_own_catalogue_is_measured_beside_purlis_s_budget_3324() {
    // #3324 (SK-39's follow-up): the skills a harness finds itself in the person's own folders
    // are never in purlis's budget, so each harness in the studio measures them as a figure of
    // their own: what it lists, its manual-only ones apart, and one the project turned off not
    // at all. Each harness reads manual-only its own way: Claude Code by the frontmatter key,
    // Codex by its sidecar alone.
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(&plane).unwrap();
    std::fs::write(
        plane.join("charter.toml"),
        "[harness_skills.codex]\nnotes = false\n",
    )
    .unwrap();
    let manual_key = |dir: &Path, name: &str| {
        std::fs::create_dir_all(dir.join(name)).unwrap();
        std::fs::write(
            dir.join(name).join("SKILL.md"),
            format!(
                "---\nname: {name}\ndescription: Does {name}.\ndisable-model-invocation: true\n---\n"
            ),
        )
        .unwrap();
    };
    let claude = tmp.path().join("claude-config/skills");
    a_skill_with_a_sidecar(&claude, "review", None);
    manual_key(&claude, "quiet");
    let codex = tmp.path().join("codex-home/skills");
    a_skill_with_a_sidecar(&codex, "review", None);
    a_skill_with_a_sidecar(&codex, "mine", Some(CODEX_MANUAL_ONLY));
    a_skill_with_a_sidecar(&codex, "notes", None);
    // Codex does not read the frontmatter key: it lists this one.
    manual_key(&codex, "quiet");
    let chat = [
        (
            "CLAUDE_CONFIG_DIR".to_owned(),
            tmp.path().join("claude-config").display().to_string(),
        ),
        (
            "CODEX_HOME".to_owned(),
            tmp.path().join("codex-home").display().to_string(),
        ),
    ];
    let env = crate::harness_plugin::Env {
        chat: &chat,
        home: Some(tmp.path().join("home")),
        process: false,
    };

    let list = studio_list(None, &plane, None, None, &env);

    let own = |harness: Harness| {
        list.harnesses
            .iter()
            .find(|one| one.harness == harness)
            .and_then(|one| one.own)
            .unwrap_or_else(|| panic!("{harness:?} measured"))
    };
    let described = "Does review.".chars().count();
    assert_eq!(
        own(Harness::ClaudeCode),
        OwnCatalogue {
            skills: 1,
            chars: described,
            manual: 1,
        },
        "Claude Code: review listed, quiet manual-only by its key"
    );
    // Codex lists each skill's path beside its description, and purlis measures it so.
    let path = |name: &str| codex.join(name).join("SKILL.md").display().to_string();
    assert_eq!(
        own(Harness::Codex),
        OwnCatalogue {
            skills: 2,
            chars: described
                + path("review").chars().count()
                + "Does quiet.".chars().count()
                + path("quiet").chars().count(),
            manual: 1,
        },
        "Codex: review and quiet listed, mine manual-only by its sidecar, notes turned off"
    );
    // A read for a count of needs measures nothing.
    let needs = studio_list_for(Reading::Needs, None, &plane, None, None, &env, &[]);
    assert!(needs.harnesses.iter().all(|one| one.own.is_none()));
}

#[test]
fn a_harness_s_own_catalogue_is_said_beside_the_budget_and_never_counted_3324() {
    let own = |skills, chars, manual| OwnCatalogue {
        skills,
        chars,
        manual,
    };
    assert_eq!(
        own(2, 1_310, 1).said("Codex").as_deref(),
        Some("Codex's own, not counted: 2 skills, 1,310 characters, 1 manual-only skill")
    );
    assert_eq!(
        own(1, 12, 0).said("Claude Code").as_deref(),
        Some("Claude Code's own, not counted: 1 skill, 12 characters")
    );
    assert_eq!(
        own(0, 0, 3).said("Codex").as_deref(),
        Some("Codex's own, not counted: 3 manual-only skills"),
        "nothing listed is not said as 0 skills"
    );
    assert_eq!(own(0, 0, 0).said("Codex"), None, "none of its own");
    assert_eq!(
        own(1, 1_234_567, 0).said("Codex").as_deref(),
        Some("Codex's own, not counted: 1 skill, 1,234,567 characters")
    );
}

#[test]
fn purlis_s_own_plugin_is_no_part_of_a_harness_s_own_catalogue_3324() {
    // Its skills are purlis's built-in ones, or a plugin it pins off in every chat.
    let claude = &crate::harness_plugin::CLAUDE_CODE;
    for name in ["charter", "purlis"] {
        assert!(
            crate::harness_plugin::purlis_s_own_named(claude, name),
            "{name}"
        );
    }
    assert!(!crate::harness_plugin::purlis_s_own_named(claude, "figma"));
}

mod read_budget;
