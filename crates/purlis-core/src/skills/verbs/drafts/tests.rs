// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam for draft skills (SK-9, #2772): a chat's write kept as a draft on a temporary plane,
//! listed with its diff, accepted as one commit of exactly its files, rejected, and tried in a
//! throwaway piece that leaves nothing behind.

use std::path::Path;

use super::*;
use crate::repocmd::Say;
use crate::testgit::run;

/// A plane that is a git repository with a save mode that commits and never pushes, with a
/// workspace `ops` and a project skill `deploy`, in one commit.
fn plane() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[plane]\nmode = \"commit\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
    std::fs::write(root.join("workspaces/ops/.keep"), "").unwrap();
    skill_at(root, "skills/deploy", "deploy", "Ships it.");
    std::fs::write(
        root.join(".gitignore"),
        ".charter/\n.purlis/\n/skills.local/\n/workspaces/*/*\n",
    )
    .unwrap();
    run(root, &["init", "-q", "-b", "main", "."]);
    run(root, &["config", "user.name", "Fixture"]);
    run(root, &["config", "user.email", "fixture@example.invalid"]);
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "one"]);
    tmp
}

fn skill_text(name: &str, description: &str) -> String {
    format!("---\nname: {name}\ndescription: {description}\n---\n\nbody\n")
}

fn skill_at(root: &Path, folder: &str, name: &str, description: &str) {
    std::fs::create_dir_all(root.join(folder)).unwrap();
    std::fs::write(
        root.join(folder).join("SKILL.md"),
        skill_text(name, description),
    )
    .unwrap();
}

fn whole(text: &str) -> Change {
    Change::Whole {
        content: text.to_owned(),
    }
}

/// Keeps `change` to the plane file `rel` as chat 7's draft.
fn drafted(root: &Path, rel: &str, change: &Change) -> String {
    let file = draftable(root, &root.join(rel)).expect("a skill's file");
    take(root, 7, &file, change).unwrap()
}

fn accepted(root: &Path, id: &str) -> (Result<PathBuf, String>, Vec<Say>) {
    let mut lines = Vec::new();
    let mut say = |line: Say| lines.push(line);
    let answer = accept(
        &mut Ctx {
            plane: root,
            built_in: None,
            by: super::super::By::Window,
            say: &mut say,
        },
        id,
    );
    (answer, lines)
}

fn head_of(root: &Path) -> String {
    run(root, &["rev-parse", "HEAD"]).line().to_owned()
}

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

/// What git says is not committed, beside the state folder.
fn uncommitted(root: &Path) -> String {
    run(root, &["status", "--porcelain", "--untracked-files=all"])
        .out
        .clone()
}

#[test]
fn only_a_file_below_a_plane_levels_skill_is_draftable() {
    let tmp = plane();
    let root = tmp.path();
    let is = |rel: &str| draftable(root, &root.join(rel));
    let file = is("skills/triage/SKILL.md").unwrap();
    assert_eq!(file.level, Level::project());
    assert_eq!(
        (file.folder.as_str(), file.inner.as_str()),
        ("triage", "SKILL.md")
    );
    assert_eq!(
        is("workspaces/ops/skills.local/x/scripts/run.sh").map(|f| f.level),
        Some(Level::workspace("ops").local(true))
    );
    for not in [
        "skills/SKILL.md",
        "skills/.hidden/SKILL.md",
        "skills/x/.envrc",
        "skills/x/hooks/pre",
        "skills/x/package.json",
        "workspaces/nope/skills/x/SKILL.md",
        "personas/p/skills/x/SKILL.md",
        "charter.toml",
        "notes/skills/x/SKILL.md",
    ] {
        assert_eq!(is(not), None, "{not}");
    }
    assert_eq!(
        draftable(root, Path::new("/elsewhere/skills/x/SKILL.md")),
        None
    );
}

#[test]
fn a_personas_skill_is_drafted_and_accepted_as_one_commit_of_its_files() {
    // SK-32 (#3048): a persona with a folder of its own; a one-file persona keeps no skills.
    let tmp = plane();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("personas/reviewer")).unwrap();
    std::fs::write(
        root.join("personas/reviewer/persona.md"),
        "---\nname: reviewer\n---\n\nReviews.\n",
    )
    .unwrap();
    std::fs::write(root.join("personas/flat.md"), "---\nname: flat\n---\n").unwrap();
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "personas"]);

    assert_eq!(
        draftable(
            root,
            &root.join("personas/reviewer/skills.local/x/SKILL.md")
        )
        .map(|f| f.level),
        Some(Level::persona("reviewer").local(true))
    );
    assert_eq!(
        draftable(root, &root.join("personas/flat/skills/x/SKILL.md")),
        None
    );
    assert_eq!(
        draftable(root, &root.join("personas/reviewer/persona.md")),
        None
    );

    let id = drafted(
        root,
        "personas/reviewer/skills/lint/SKILL.md",
        &whole(&skill_text("lint", "Lints the change under review.")),
    );
    let (answer, _) = accepted(root, &id);
    answer.unwrap();
    let (subject, paths) = last_commit(root);
    assert!(subject.ends_with("skill new lint"), "{subject}");
    assert_eq!(paths, ["personas/reviewer/skills/lint/SKILL.md"]);
    assert!(asking(&Level::persona("reviewer")).contains("personas/reviewer/skills/<name>"));
}

#[cfg(unix)]
#[test]
fn a_skill_folder_that_is_a_link_is_never_draftable() {
    let tmp = plane();
    let root = tmp.path();
    let away = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(away.path(), root.join("skills/linked")).unwrap();
    assert_eq!(draftable(root, &root.join("skills/linked/SKILL.md")), None);
}

#[test]
fn a_chats_write_is_a_draft_that_never_reaches_the_project_or_git() {
    let tmp = plane();
    let root = tmp.path();
    let before = head_of(root);
    let id = drafted(
        root,
        "skills/triage/SKILL.md",
        &whole(&skill_text("triage", "Sorts issues.")),
    );

    assert!(!root.join("skills/triage").exists());
    assert_eq!(uncommitted(root), "");
    assert_eq!(head_of(root), before);
    let drafts = list(root);
    assert_eq!(drafts.len(), 1);
    let draft = &drafts[0];
    assert_eq!(
        (draft.id.as_str(), draft.name.as_str()),
        (id.as_str(), "triage")
    );
    assert_eq!((draft.new, draft.chat), (true, Some(7)));
    assert_eq!(draft.files[0].before, None);
    assert!(draft.files[0].after.contains("Sorts issues."));
    // Never a skill any chat gets.
    let gets: Vec<String> = crate::skills::in_the_studio(None, root, None, None)
        .effective
        .into_iter()
        .map(|e| e.skill.name)
        .collect();
    assert!(!gets.contains(&"triage".to_owned()), "{gets:?}");
}

#[test]
fn a_second_write_adds_to_the_same_draft_and_an_edit_is_made_to_the_drafts_copy() {
    let tmp = plane();
    let root = tmp.path();
    let first = drafted(
        root,
        "skills/deploy/SKILL.md",
        &whole(&skill_text("deploy", "Ships it carefully.")),
    );
    let edit = Change::Edits {
        edits: vec![crate::proposed::Edit {
            old: "carefully".to_owned(),
            new: "twice".to_owned(),
            every: false,
        }],
    };
    let second = drafted(root, "skills/deploy/SKILL.md", &edit);
    let third = drafted(root, "skills/deploy/scripts/check.sh", &whole("echo ok\n"));
    assert_eq!(first, second);
    assert_eq!(second, third);
    let draft = one(root, &first).unwrap();
    assert!(!draft.new);
    let paths: Vec<&str> = draft.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["SKILL.md", "scripts/check.sh"]);
    assert!(draft.files[0].after.contains("Ships it twice."));
    assert!(
        draft.files[0]
            .before
            .as_deref()
            .unwrap()
            .contains("Ships it.")
    );
    assert!(!draft.stale());
}

#[test]
fn accept_is_one_commit_of_exactly_the_drafts_files_and_the_draft_is_gone() {
    let tmp = plane();
    let root = tmp.path();
    // Somebody's unrelated work, which the accept may not carry.
    std::fs::write(root.join("notes.md"), "mine\n").unwrap();
    let id = drafted(
        root,
        "skills/triage/SKILL.md",
        &whole(&skill_text("triage", "Sorts issues.")),
    );
    drafted(root, "skills/triage/scripts/sort.sh", &whole("sort\n"));
    let before = head_of(root);

    let (answer, _) = accepted(root, &id);
    answer.unwrap();
    assert_eq!(
        run(root, &["rev-list", "--count", &format!("{before}..HEAD")]).line(),
        "1"
    );
    let (subject, paths) = last_commit(root);
    assert!(subject.ends_with("skill new triage"), "{subject}");
    assert_eq!(
        paths,
        ["skills/triage/SKILL.md", "skills/triage/scripts/sort.sh"]
    );
    assert!(uncommitted(root).contains("notes.md"));
    assert!(list(root).is_empty());
    assert_eq!(one(root, &id).unwrap_err(), NO_SUCH);
}

#[test]
fn accepting_a_change_to_a_skill_commits_the_files_it_changed_and_nothing_else() {
    let tmp = plane();
    let root = tmp.path();
    std::fs::write(root.join("skills/deploy/notes.md"), "mine\n").unwrap();
    let id = drafted(
        root,
        "skills/deploy/SKILL.md",
        &whole(&skill_text("deploy", "Ships it twice.")),
    );
    drafted(root, "skills/deploy/check.sh", &whole("check\n"));
    let (answer, lines) = accepted(root, &id);
    answer.unwrap();
    let (subject, paths) = last_commit(root);
    assert!(
        subject.ends_with("skill edit deploy"),
        "{subject} {lines:?}"
    );
    assert_eq!(
        paths,
        ["skills/deploy/SKILL.md", "skills/deploy/check.sh"],
        "{lines:?}"
    );
    assert!(uncommitted(root).contains("skills/deploy/notes.md"));
}

#[test]
fn accept_refuses_a_draft_of_a_file_the_project_changed_since_and_writes_nothing() {
    let tmp = plane();
    let root = tmp.path();
    let id = drafted(
        root,
        "skills/deploy/SKILL.md",
        &whole(&skill_text("deploy", "Ships it twice.")),
    );
    std::fs::write(
        root.join("skills/deploy/SKILL.md"),
        skill_text("deploy", "Ships it by hand."),
    )
    .unwrap();
    assert!(one(root, &id).unwrap().stale());
    let (answer, _) = accepted(root, &id);
    assert!(answer.unwrap_err().contains("changed in the project"));
    assert!(
        std::fs::read_to_string(root.join("skills/deploy/SKILL.md"))
            .unwrap()
            .contains("by hand")
    );
    assert_eq!(list(root).len(), 1);
}

#[test]
fn accept_refuses_instructions_the_lint_finds_a_problem_in() {
    let tmp = plane();
    let root = tmp.path();
    let id = drafted(root, "skills/triage/SKILL.md", &whole("no frontmatter\n"));
    let (answer, _) = accepted(root, &id);
    assert!(answer.unwrap_err().starts_with("nothing was accepted"));
    assert!(!root.join("skills/triage").exists());

    let scripts_only = drafted(root, "skills/other/run.sh", &whole("x\n"));
    let (answer, _) = accepted(root, &scripts_only);
    assert!(answer.unwrap_err().contains("has no instructions"));
}

#[test]
fn a_local_skills_draft_is_written_and_never_committed() {
    let tmp = plane();
    let root = tmp.path();
    let before = head_of(root);
    let id = drafted(
        root,
        "skills.local/mine/SKILL.md",
        &whole(&skill_text("mine", "Mine alone.")),
    );
    accepted(root, &id).0.unwrap();
    assert!(root.join("skills.local/mine/SKILL.md").is_file());
    assert_eq!(head_of(root), before);
}

#[test]
fn reject_deletes_the_draft_and_nothing_else() {
    let tmp = plane();
    let root = tmp.path();
    let id = drafted(
        root,
        "skills/deploy/SKILL.md",
        &whole(&skill_text("deploy", "Ships it twice.")),
    );
    reject(root, &id).unwrap();
    assert!(list(root).is_empty());
    assert!(
        std::fs::read_to_string(root.join("skills/deploy/SKILL.md"))
            .unwrap()
            .contains("Ships it.")
    );
    assert_eq!(uncommitted(root), "");
    assert_eq!(reject(root, &id).unwrap_err(), NO_SUCH);
    assert_eq!(reject(root, "../../etc").unwrap_err(), NO_SUCH);
}

#[test]
fn try_it_hands_a_chat_in_its_piece_the_draft_alone_and_ending_it_leaves_nothing() {
    let tmp = plane();
    let root = tmp.path();
    let id = drafted(
        root,
        "skills/deploy/SKILL.md",
        &whole(&skill_text("deploy", "Ships it twice.")),
    );
    drafted(root, "skills/deploy/notes.md", &whole("drafted\n"));
    std::fs::write(root.join("skills/deploy/kept.md"), "the project's\n").unwrap();
    skill_at(root, "skills/release", "release", "Cuts it.");

    let tried_out = try_out(root, &id).unwrap();
    assert!(tried_out.piece.is_dir());
    assert!(
        tried_out
            .piece
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(PIECE_PREFIX)
    );
    let skills_from = tried(root, &tried_out.piece).expect("the piece is a try's");
    // The skill as the draft makes it, the project's other files of it with it, and no other.
    let set = crate::skills::in_the_studio(None, &skills_from, None, None);
    let names: Vec<&str> = set
        .effective
        .iter()
        .map(|e| e.skill.name.as_str())
        .collect();
    assert_eq!(names, ["deploy"]);
    assert!(set.effective[0].skill.description.contains("twice"));
    let laid = skills_from.join("skills/deploy");
    assert_eq!(
        std::fs::read_to_string(laid.join("notes.md")).unwrap(),
        "drafted\n"
    );
    assert_eq!(
        std::fs::read_to_string(laid.join("kept.md")).unwrap(),
        "the project's\n"
    );
    // Any other folder is no try's.
    assert_eq!(tried(root, root), None);
    assert_eq!(tried(root, &root.join("workspaces/ops")), None);

    std::fs::write(tried_out.piece.join("made.txt"), "by the chat\n").unwrap();
    // Inside the project, and nothing git or a save takes.
    assert!(
        !uncommitted(root).contains(".throwaway"),
        "{}",
        uncommitted(root)
    );
    assert!(end_try_at(root, &tried_out.piece).unwrap());
    assert!(!tried_out.piece.exists());
    assert!(!skills_from.exists());
    assert_eq!(
        std::fs::read_dir(root.join("workspaces/.throwaway"))
            .unwrap()
            .count(),
        0
    );
    // The draft stays, for Accept or Reject.
    assert_eq!(list(root).len(), 1);
    assert!(!end_try_at(root, &tried_out.piece).unwrap());
}

#[cfg(unix)]
#[test]
fn ending_a_try_never_follows_a_link_put_in_its_piece() {
    let tmp = plane();
    let root = tmp.path();
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(elsewhere.path().join("keep.txt"), "keep\n").unwrap();
    let id = drafted(
        root,
        "skills/triage/SKILL.md",
        &whole(&skill_text("triage", "Sorts issues.")),
    );
    let tried_out = try_out(root, &id).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), tried_out.piece.join("link")).unwrap();
    end_try(root, &tried_out.id).unwrap();
    assert!(!tried_out.piece.exists());
    assert!(elsewhere.path().join("keep.txt").is_file());
}

#[test]
fn a_try_left_behind_is_swept_once_old_and_one_a_chat_stands_in_is_kept() {
    let tmp = plane();
    let root = tmp.path();
    let id = drafted(
        root,
        "skills/triage/SKILL.md",
        &whole(&skill_text("triage", "Sorts issues.")),
    );
    let kept = try_out(root, &id).unwrap();
    let left = try_out(root, &id).unwrap();
    let state = crate::plane::state_dir(root);
    // Just laid out: its chat may not be on the app's record yet.
    sweep_tries(root, &[]);
    assert!(left.piece.is_dir());
    let old = std::time::SystemTime::now() - SWEEP_GRACE * 2;
    for one in [&kept, &left] {
        std::fs::File::open(state.join(TRIES_DIR).join(&one.id))
            .unwrap()
            .set_modified(old)
            .unwrap();
    }
    // A throwaway folder whose try is gone, as a crash between the two removals leaves it.
    let orphan = root.join("workspaces/.throwaway/purlis-try-fedcba9876543210");
    std::fs::create_dir_all(orphan.join("inside")).unwrap();
    sweep_tries(root, std::slice::from_ref(&kept.piece));
    assert!(kept.piece.is_dir(), "a chat stands in it");
    assert!(!left.piece.exists());
    assert!(!state.join(TRIES_DIR).join(&left.id).exists());
    assert!(!orphan.exists());
    let ids: Vec<String> = tries(root).into_iter().map(|one| one.id).collect();
    assert_eq!(ids, std::slice::from_ref(&kept.id));
}

#[test]
fn only_a_throwaway_folder_is_a_piece_and_only_its_chat_takes_the_tries_skills() {
    let tmp = plane();
    let root = tmp.path();
    let id = drafted(
        root,
        "skills/triage/SKILL.md",
        &whole(&skill_text("triage", "Sorts issues.")),
    );
    let tried_out = try_out(root, &id).unwrap();
    assert!(a_piece(&tried_out.piece));
    for not in [
        root.join("workspaces/ops"),
        root.join("workspaces/.throwaway/purlis-try-x"),
        root.join("elsewhere/.throwaway/purlis-try-0123456789abcdef"),
    ] {
        assert!(!a_piece(&not), "{}", not.display());
    }
    let scope = scope_of(root, Some(&tried_out.piece));
    assert!(scope.tried);
    assert_eq!(scope.root, tried(root, &tried_out.piece).unwrap());
    let elsewhere = scope_of(root, Some(&root.join("workspaces/ops")));
    assert_eq!(
        elsewhere,
        Scope {
            root: root.to_path_buf(),
            tried: false
        }
    );
}

/// Decision 50: a Try it's scratch chat has the built-in skills and the draft alone, on every
/// harness: each one's installed plugins and the person's own skills for it are turned off, and
/// each adapter is handed the names.
#[test]
fn a_try_turns_off_every_harness_skill_and_plugin_on_every_harness() {
    use crate::harness::Harness;
    let home = tempfile::tempdir().unwrap();
    let at = |rel: &str| home.path().join(rel);
    std::fs::create_dir_all(at("claude/plugins")).unwrap();
    std::fs::write(
        at("claude/plugins/installed_plugins.json"),
        r#"{"version": 2, "plugins": {"figma@market": [{"scope": "user", "installPath": "/x", "version": "1"}]}}"#,
    )
    .unwrap();
    skill_at(home.path(), "claude/skills/mine", "mine", "Mine.");
    skill_at(home.path(), "codex/skills/codexmine", "codexmine", "Mine.");
    skill_at(home.path(), ".claude/skills/ocmine", "ocmine", "Mine.");
    let vars: Vec<(String, String)> = [("CLAUDE_CONFIG_DIR", "claude"), ("CODEX_HOME", "codex")]
        .iter()
        .map(|(name, rel)| ((*name).to_owned(), at(rel).display().to_string()))
        .collect();
    let env = crate::harness_plugin::Env {
        chat: &vars,
        home: Some(home.path().to_path_buf()),
        process: false,
    };
    let kit = crate::harness::Kit {
        binary: Path::new("/opt/purlis/purlis"),
        plugin: None,
        persona: None,
        skills: None,
        graded: false,
        laid_out: None,
    };
    for (harness, own) in [
        (Harness::ClaudeCode, "mine"),
        (Harness::Codex, "codexmine"),
        (Harness::Opencode, "ocmine"),
    ] {
        let chosen = alone_on(harness, &env);
        assert!(chosen.skills_off.contains(own), "{harness:?}: {chosen:?}");
        assert!(
            chosen.plugins.values().all(|on| !on),
            "{harness:?}: {chosen:?}"
        );
        assert!(
            crate::skills::skills_off_for(kit, harness, &chosen).contains(&own.to_owned()),
            "{harness:?} is handed {own} as turned off"
        );
    }
    assert_eq!(
        alone_on(Harness::ClaudeCode, &env)
            .plugins
            .get("figma@market"),
        Some(&false)
    );
}

/// SK-16 (#2779): a grade's arm with the skill hands its chat the copy as the project has it,
/// without the fixtures it is graded by; the arm without it hands the chat no project skill.
#[test]
fn a_grades_arms_hand_a_chat_the_skill_without_its_fixtures_or_no_skill_at_all() {
    let tmp = plane();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("skills/deploy/evals")).unwrap();
    std::fs::write(root.join("skills/deploy/evals/triggers.json"), "[]").unwrap();
    std::fs::write(root.join("skills/deploy/notes.md"), "kept\n").unwrap();
    skill_at(root, "skills/release", "release", "Cuts it.");

    let copy = snapshot(root, &Level::project(), "deploy").unwrap();
    assert!(
        copy.files.keys().all(|path| !path.starts_with("evals/")),
        "{:?}",
        copy.files
    );
    // An edit after the grade read it changes nothing it lays out.
    std::fs::write(root.join("skills/deploy/notes.md"), "edited mid-grade\n").unwrap();
    let with = lay_out_arm(root, Some(&copy)).unwrap();
    assert!(graded_piece(root, &with.piece));
    let skills_from = tried(root, &with.piece).expect("the piece is a try's");
    let set = crate::skills::in_the_studio(None, &skills_from, None, None);
    let names: Vec<&str> = set
        .effective
        .iter()
        .map(|e| e.skill.name.as_str())
        .collect();
    assert_eq!(names, ["deploy"]);
    let laid = skills_from.join("skills/deploy");
    assert_eq!(
        std::fs::read_to_string(laid.join("notes.md")).unwrap(),
        "kept\n"
    );
    assert!(
        !laid.join("evals").exists(),
        "the fixtures stay out of the chat's reach"
    );

    let without = lay_out_arm(root, None).unwrap();
    assert!(graded_piece(root, &without.piece));
    let none_from = tried(root, &without.piece).expect("the piece is a try's");
    let set = crate::skills::in_the_studio(None, &none_from, None, None);
    assert!(set.effective.is_empty(), "{:?}", set.effective);

    assert!(end_try_at(root, &with.piece).unwrap());
    assert!(end_try_at(root, &without.piece).unwrap());
    assert!(!skills_from.exists() && !none_from.exists());
}

/// #3245: Use skill… and the restart's stamp route a Try it's scratch chat as its arming does:
/// the draft alone from the plane, never the project's other skills.
#[test]
fn use_skill_offers_a_try_its_draft_alone_as_its_arming_hands_it() {
    let tmp = plane();
    let root = tmp.path();
    let id = drafted(
        root,
        "skills/deploy/SKILL.md",
        &whole(&skill_text("deploy", "Ships it twice.")),
    );
    skill_at(root, "skills/release", "release", "Cuts it.");
    let tried_out = try_out(root, &id).unwrap();
    let harness = crate::harness::Harness::ClaudeCode;
    // What each chat's start read, once (#3234).
    let in_the_try =
        crate::hoststart::ChatSkills::read(true, root, Some(harness), Some(&tried_out.piece), None);
    let at_the_root =
        crate::hoststart::ChatSkills::read(true, root, Some(harness), Some(root), None);
    let chat = |start| crate::skills::Running {
        plane: root,
        workspace: None,
        persona: None,
        harness,
        built_in: None,
        plane_skills: true,
        start: Some(start),
    };
    let offered = |start| -> Vec<String> {
        crate::skills::for_use_skill(&chat(start))
            .offered
            .into_iter()
            .map(|one| one.effective.skill.name)
            .collect()
    };

    assert!(in_the_try.tried);
    assert_eq!(offered(&in_the_try), ["deploy"]);
    assert!(offered(&at_the_root).contains(&"release".to_owned()));
    assert_ne!(
        crate::skills::stamp_of(&chat(&in_the_try)),
        crate::skills::stamp_of(&chat(&at_the_root)),
    );
}
