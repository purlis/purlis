// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::process::Command;

/// git for a fixture: never the developer's global config, so never their signer.
fn git(dir: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(dir)
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.invalid");
    let out = purlis_core::forklock::output(&mut command).expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A plane with one commit, its own identity, and no remote.
fn plane(toml: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("charter.toml"), toml).unwrap();
    std::fs::write(root.join(".gitignore"), ".charter/\n").unwrap();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.name", "t"]);
    git(root, &["config", "user.email", "t@example.invalid"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "one"]);
    dir
}

#[test]
fn a_file_written_into_the_plane_is_what_the_next_save_takes() {
    let dir = plane("");
    std::fs::write(dir.path().join("note.md"), "n").unwrap();

    let got = saving_of(dir.path());

    assert_eq!(got.stage, "changed");
    assert_eq!(got.changed, ["note.md"]);
    assert_eq!((got.mode, got.mode_from.as_str()), (None, "default"));
}

#[test]
fn the_save_button_commits_with_the_message_typed_and_the_journal_says_so() {
    let dir = plane("[plane]\nmode = \"commit\"\n");
    std::fs::write(dir.path().join("note.md"), "n").unwrap();

    let said = save(dir.path(), Some("  from the button  ")).expect("saved");

    assert!(
        said.iter().any(|l| l.contains("from the button")),
        "{said:?}"
    );
    let got = saving_of(dir.path());
    assert_eq!(got.changed, Vec::<String>::new());
    assert_eq!(
        (got.mode.as_deref(), got.mode_from.as_str()),
        (Some("commit"), "charter.toml")
    );
    let last = &got.journal[0];
    assert_eq!(
        (last.trigger.as_str(), last.outcome.as_str(), last.files),
        ("manual", "committed", 1)
    );
}

#[test]
fn a_second_save_while_one_runs_is_refused_in_words_and_the_first_is_untouched() {
    let dir = plane("");
    std::fs::write(dir.path().join("note.md"), "n").unwrap();
    let running = planegit::Claim::of(dir.path()).expect("the first save");

    let err = save(dir.path(), None).expect_err("a second save");

    assert_eq!(err, planegit::ALREADY_SAVING);
    assert_eq!(
        saving_of(dir.path()).changed,
        ["note.md"],
        "the second save committed"
    );
    drop(running);
    assert!(
        save(dir.path(), None).is_ok(),
        "the next save, once the first is done"
    );
}

#[test]
fn the_answer_to_how_a_project_is_saved_is_written_as_its_mode_keeping_the_rest_of_the_file() {
    let dir = plane("# the team's settings\n[memory]\nshare = \"local\"\n");
    assert_eq!(saving_of(dir.path()).mode, None, "not asked yet");

    let got = choose_mode(dir.path(), "commit").expect("chosen");

    assert_eq!(
        (got.mode.as_deref(), got.mode_from.as_str()),
        (Some("commit"), "charter.toml")
    );
    let text = std::fs::read_to_string(dir.path().join("charter.toml")).unwrap();
    assert!(text.starts_with("# the team's settings\n"), "{text}");
    assert!(text.contains("[plane]\nmode = \"commit\"\n"), "{text}");
}

#[test]
fn the_standing_answered_after_choosing_a_mode_is_the_new_modes() {
    // FD-11: the standing is shared, and choosing writes `charter.toml` — so what is
    // answered is the plane with that change in it, not the "committed" read before.
    let dir = plane("");
    git(
        dir.path(),
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/plane.git",
        ],
    );
    assert_eq!(saving_of(dir.path()).stage, "committed");

    let got = choose_mode(dir.path(), "commit").expect("chosen");

    assert_eq!(
        (got.stage.as_str(), got.changed.as_slice()),
        ("changed", ["charter.toml".to_owned()].as_slice())
    );
}

#[test]
fn a_mode_charter_does_not_know_is_refused_and_nothing_is_written() {
    let dir = plane("");
    let err = choose_mode(dir.path(), "yolo").expect_err("refused");
    assert_eq!(
        err,
        "yolo is not a mode — one of off, commit, push, pr, pr-merge"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("charter.toml")).unwrap(),
        ""
    );
}

#[test]
fn what_came_in_and_was_not_pulled_is_counted() {
    let dir = plane("");
    assert_eq!(
        saving_of(dir.path()).behind,
        None,
        "nothing to count against"
    );
}

/// A plane saying `toml`, with a workspace `alpha` holding a clone `widget` of one commit.
fn plane_with_a_repo(toml: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = plane(toml);
    let clone = dir.path().join("workspaces/alpha/widget");
    std::fs::create_dir_all(&clone).unwrap();
    git(&clone, &["init", "-q", "-b", "main"]);
    git(&clone, &["config", "user.name", "t"]);
    git(&clone, &["config", "user.email", "t@example.invalid"]);
    std::fs::write(clone.join("README.md"), "one").unwrap();
    git(&clone, &["add", "-A"]);
    git(&clone, &["commit", "-q", "-m", "one"]);
    (dir, clone)
}

#[test]
fn each_repo_in_the_workspace_is_a_row_with_its_mode_and_stage() {
    let (dir, clone) = plane_with_a_repo("[repos.widget]\nmode = \"commit\"\n");
    std::fs::write(clone.join("a.md"), "a").unwrap();

    let rows = repos_saving(dir.path(), "alpha").expect("the workspace reads");

    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(
        (row.name.as_str(), row.mode.as_str(), row.mode_from.as_str()),
        ("widget", "commit", "charter.toml")
    );
    assert_eq!((row.stage.as_str(), row.changed), ("changed", 1));
    assert_eq!(row.branch.as_deref(), Some("main"));
    assert!(!row.autosave, "a repo's auto-save is off by default");
}

#[test]
fn a_pr_mode_row_says_where_its_pull_request_goes_and_whether_it_needs_a_branch_of_its_own() {
    // On `main`, the PR's base: the save would cut `charter/alpha/…` rather than push main.
    let (dir, clone) = plane_with_a_repo("[repos.widget]\nmode = \"pr\"\nbranch = \"main\"\n");
    let row = &repos_saving(dir.path(), "alpha").unwrap()[0];
    assert_eq!(
        (row.target.as_deref(), row.own_branch),
        (Some("main"), true)
    );

    // On a feature branch, the save pushes that branch and opens the PR from it.
    git(&clone, &["checkout", "-q", "-b", "feature/x"]);
    let row = &repos_saving(dir.path(), "alpha").unwrap()[0];
    assert_eq!(
        (row.target.as_deref(), row.own_branch),
        (Some("main"), false)
    );
}

#[test]
fn a_project_and_a_repo_whose_origin_is_on_gitlab_name_their_request_a_merge_request() {
    let (dir, clone) =
        plane_with_a_repo("[plane]\nmode = \"pr\"\n\n[repos.widget]\nmode = \"pr\"\n");
    assert_eq!(saving_of(dir.path()).request, "pull request");
    assert_eq!(
        repos_saving(dir.path(), "alpha").unwrap()[0].request,
        "pull request"
    );

    for at in [dir.path(), clone.as_path()] {
        git(
            at,
            &["remote", "add", "origin", "git@gitlab.com:acme/widget.git"],
        );
    }
    assert_eq!(saving_of(dir.path()).request, "merge request");
    assert_eq!(
        repos_saving(dir.path(), "alpha").unwrap()[0].request,
        "merge request"
    );
}

#[test]
fn a_request_is_named_as_its_forge_names_it_and_says_what_ends_a_wait() {
    use purlis_core::forge::pr::Ready;
    let stalled = planegit::Merging::of(&Ok(Ready::Stalled("no check has started".into())));
    let github = SaveMerge::of(
        stalled.clone(),
        Some("https://github.com/acme/api/pull/12"),
        "pull request",
    );
    assert_eq!(
        (github.number.as_deref(), github.needs.as_deref()),
        (Some("#12"), Some("person"))
    );
    let gitlab = SaveMerge::of(
        stalled.clone(),
        Some("https://gitlab.com/acme/api/-/merge_requests/7/"),
        "merge request",
    );
    assert_eq!(gitlab.number.as_deref(), Some("!7"));
    // A page that ends in no number names none, and a wait that ends by itself needs nothing.
    for page in [None, Some("https://github.com/acme/api/pull/x1"), Some("")] {
        assert_eq!(
            SaveMerge::of(stalled.clone(), page, "pull request").number,
            None
        );
    }
    let running = planegit::Merging::of(&Ok(Ready::Waiting("its checks run".into())));
    assert_eq!(SaveMerge::of(running, None, "pull request").needs, None);
}

#[test]
fn a_row_that_opens_no_pull_request_names_no_target() {
    let (dir, _clone) = plane_with_a_repo("[repos.widget]\nmode = \"push\"\n");
    let row = &repos_saving(dir.path(), "alpha").unwrap()[0];
    assert_eq!((row.target.as_deref(), row.own_branch), (None, false));
}

#[test]
fn a_repo_charter_never_saves_says_off_rather_than_a_stage() {
    let (dir, clone) = plane_with_a_repo("[repos.widget]\nmode = \"off\"\n");
    std::fs::write(clone.join("a.md"), "a").unwrap();

    assert_eq!(repos_saving(dir.path(), "alpha").unwrap()[0].stage, "off");
}

#[test]
fn a_repo_save_waits_for_a_chat_mid_turn_and_names_it() {
    let (dir, clone) = plane_with_a_repo("[repos.widget]\nmode = \"commit\"\n");
    std::fs::write(clone.join("a.md"), "a").unwrap();

    let err = save_repo_in(dir.path(), "alpha", "widget", None, &|| {
        vec!["alpha.2".to_owned()]
    })
    .expect_err("held back");

    assert_eq!(
        err,
        "Not saved: alpha.2 is mid-turn in alpha. A repo is saved between turns — save \
         again when the turn ends."
    );
    assert_eq!(
        repos_saving(dir.path(), "alpha").unwrap()[0].stage,
        "changed"
    );

    let said = save_repo_in(
        dir.path(),
        "alpha",
        "widget",
        Some(" from the row "),
        &Vec::new,
    )
    .expect("saved once the turn ended");
    assert!(said.iter().any(|l| l.contains("from the row")), "{said:?}");
    assert_eq!(repos_saving(dir.path(), "alpha").unwrap()[0].stage, "saved");
    let last = &saving_of(dir.path()).journal[0];
    assert_eq!(
        (last.target.as_str(), last.outcome.as_str()),
        ("repo:alpha/widget", "committed")
    );
}

#[test]
fn a_repo_the_workspace_does_not_hold_is_refused_by_name() {
    let (dir, _clone) = plane_with_a_repo("");
    assert_eq!(
        save_repo_in(dir.path(), "alpha", "gadget", None, &Vec::new).unwrap_err(),
        "alpha holds no clone called gadget"
    );
}

#[test]
fn a_save_refused_answers_the_refusal_in_the_cores_words() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();

    let err = save(dir.path(), None).expect_err("not a repository");

    assert!(err.contains("is not a git repository"), "{err}");
}

#[test]
fn a_conflicts_step_is_one_of_three_and_none_runs_where_no_rebase_is_stopped() {
    let dir = tempfile::tempdir().expect("a directory");

    let unknown = settle(dir.path(), "merge", &[]).expect_err("not a step");
    assert!(
        unknown.contains("start, continue, skip or abort"),
        "{unknown}"
    );
    for step in ["continue", "skip", "abort"] {
        let got = settle(dir.path(), step, &[]).expect("answered");
        assert_eq!(got.outcome, "refused", "{step}");
        assert!(
            got.said.contains("No rebase is stopped"),
            "{step}: {}",
            got.said
        );
    }
}
