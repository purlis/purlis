// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use purlis_core::dockerpool::Image;
use purlis_core::runners::{Pool, Set};

use super::*;

fn of(root: &Path, config: Option<&Path>) -> Option<ProjectNotHeld> {
    now(config)
        .into_iter()
        .find(|one| Path::new(&one.root) == root)
}

#[test]
fn a_project_not_held_is_kept_with_the_runners_words_until_it_is_held_again() {
    let _one = rechecking();
    let root = PathBuf::from("/projects/notheld-test/alpha");
    not_held(&root, "that folder is not there");
    assert_eq!(
        of(&root, None),
        Some(ProjectNotHeld {
            root: root.display().to_string(),
            name: "alpha".into(),
            why: "that folder is not there".into(),
            pools: Vec::new(),
        })
    );
    not_held(&root, "the runner is stopped");
    assert_eq!(of(&root, None).unwrap().why, "the runner is stopped");

    held(&root);
    assert_eq!(of(&root, None), None);
}

#[test]
fn the_runners_words_are_kept_as_plain_text_and_cut_short() {
    let _one = rechecking();
    let root = PathBuf::from("/projects/notheld-test/words");
    not_held(&root, "line one\nline\u{202e} two\u{7}");
    assert_eq!(of(&root, None).unwrap().why, "line one line two");

    not_held(&root, &"x".repeat(MOST_SAID * 2));
    let why = of(&root, None).unwrap().why;
    assert_eq!(why.chars().count(), MOST_SAID);
    assert!(why.ends_with('…'), "{why}");

    not_held(&root, "cannot reach ssh://someone:hidden@build.example");
    assert_eq!(
        of(&root, None).unwrap().why,
        "cannot reach ssh://…@build.example"
    );
    held(&root);
}

/// A project's mark names the pool its chats run on, and goes once it runs on none: its pool
/// removed, or the project forgotten.
#[test]
fn a_mark_names_its_pool_and_goes_once_the_project_runs_on_none() {
    let _one = rechecking();
    let config = tempfile::tempdir().unwrap();
    let root = config.path().join("beta");
    std::fs::create_dir_all(&root).unwrap();
    let image = Image::pinned(&format!(
        "ghcr.io/purlis/purlis-runner@sha256:{}",
        "0".repeat(64)
    ))
    .unwrap();
    purlis_core::runners::record_pool(
        config.path(),
        &Pool::new("builds", image, Set::default()).unwrap(),
    )
    .unwrap();
    purlis_core::machine::update(config.path(), |store| {
        store.remember(&root, 1);
        store
            .set_chats_run_on(&root, "alpha", Some("builds"))
            .unwrap();
    })
    .unwrap();
    not_held(&root, "that folder is not there");

    recheck(config.path());
    assert_eq!(of(&root, Some(config.path())).unwrap().pools, ["builds"]);

    // Its pool goes: so does the mark.
    purlis_core::runners::forget_pool(config.path(), "builds").unwrap();
    recheck(config.path());
    assert_eq!(of(&root, Some(config.path())), None);
}

/// A project's pool counts for its mark only while a workspace of it follows that pool (#4591).
#[test]
fn a_projects_pool_marks_it_only_while_a_workspace_follows_that_pool() {
    let _one = rechecking();
    let config = tempfile::tempdir().unwrap();
    let root = config.path().join("gamma");
    let folder = |workspace: &str| {
        std::fs::create_dir_all(root.join("workspaces").join(workspace)).unwrap();
    };
    folder("alpha");
    let image = Image::pinned(&format!(
        "ghcr.io/purlis/purlis-runner@sha256:{}",
        "0".repeat(64)
    ))
    .unwrap();
    purlis_core::runners::record_pool(
        config.path(),
        &Pool::new("builds", image, Set::default()).unwrap(),
    )
    .unwrap();
    purlis_core::machine::update(config.path(), |store| {
        store.remember(&root, 1);
        store
            .set_project_chats_run_on(&root, Some("builds"))
            .unwrap();
        store.set_chats_run_here(&root, "alpha").unwrap();
    })
    .unwrap();
    not_held(&root, "that folder is not there");

    // Its one workspace runs here: nothing follows the project's pool.
    recheck(config.path());
    assert_eq!(of(&root, Some(config.path())), None);

    // A workspace that follows it marks the project again.
    folder("beta");
    not_held(&root, "that folder is not there");
    recheck(config.path());
    assert_eq!(of(&root, Some(config.path())).unwrap().pools, ["builds"]);
    held(&root);
}
