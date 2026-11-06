// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::hookwire::{Answer, GitAsk, GitWork, alive};
use crate::pieces::Who;
use crate::testgit;
use crate::worktree::git::Isolated;

/// A plane with workspace `alpha` holding a clone `api` on `main`, one commit in.
fn a_plane() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let plane = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::write(plane.join("charter.toml"), "schema = 1\n").unwrap();
    let clone = plane.join("workspaces/alpha/api");
    std::fs::create_dir_all(&clone).unwrap();
    for args in [
        &["init", "-q", "-b", "main", "."][..],
        &["config", "user.name", "Tester"],
        &["config", "user.email", "t@e.invalid"],
    ] {
        assert!(testgit::run(&clone, args).ok());
    }
    std::fs::write(clone.join("README.md"), "one\n").unwrap();
    assert!(testgit::run(&clone, &["add", "README.md"]).ok());
    assert!(testgit::run(&clone, &["commit", "-q", "-m", "one"]).ok());
    (dir, plane)
}

/// A plain folder `name` of `api`, cut from the clone's HEAD as the terminal cuts one.
fn plain(plane: &Path, name: &str) {
    let code = crate::piececmd::add(
        plane,
        "alpha",
        "api",
        name,
        None,
        Some(crate::piececmd::HEAD),
        None,
        None,
        &Who::here(None, None, None),
        chrono::Utc::now(),
        &mut |_| {},
    );
    assert_eq!(code, 0, "{name} was not cut");
}

/// The prune ask a chat standing in the workspace sends: a dry run, its asker reading
/// [`Answer::Alive`] lines where `alive`.
fn the_ask(alive: bool) -> GitAsk {
    GitAsk {
        chat: 4,
        workspace: "alpha".to_owned(),
        alive,
        work: GitWork::WorktreePrune {
            repo: None,
            dry_run: true,
            keep_branches: false,
        },
    }
}

fn the_asker(plane: &Path) -> crate::gitbroker::Asker {
    crate::gitbroker::Asker {
        chat: 4,
        cwd: Some(plane.join("workspaces/alpha")),
        persona: None,
        harnessed: true,
        unsandboxed: false,
        roots_git_held: false,
        unreadable: Vec::new(),
        config: None,
        id: None,
        live: Some(Vec::new()),
    }
}

#[test]
fn a_chats_prune_of_many_folders_says_it_is_alive_as_it_goes() {
    // #5439: a chat's `prune --dry-run` on about 290 folders outlasted the asker's one wait
    // and came back with os error 35 and no list. The asker's wait bounds a silence
    // ([`crate::hookwire::Asking::ask`]); the prune says it is alive as it reads each folder,
    // so the longest silence the asker sees is about one folder's reading, never the whole prune's.
    crate::unsteered!();
    let (_dir, plane) = a_plane();
    // More folders than the prune reads at once, so the whole is several readings long.
    let folders = 24;
    for n in 0..folders {
        plain(&plane, &format!("lane-{n}"));
    }
    let said: Arc<Mutex<Vec<Instant>>> = Arc::default();
    let lines = Arc::clone(&said);
    let pulse = alive::Pulse::saying(move || {
        lines.lock().unwrap().push(Instant::now());
        true
    });
    let begun = Instant::now();
    let answer = alive::within(Some(&pulse), || {
        crate::gitbroker::answer(
            &plane,
            &the_asker(&plane),
            &the_ask(true),
            &Isolated::identity(Some("Tester"), Some("t@e.invalid")),
            chrono::Utc::now(),
        )
    });
    let ended = Instant::now();
    let Answer::Said { lines, code: 0 } = answer else {
        panic!("{answer:?}");
    };
    let spoken: Vec<String> = lines.iter().map(ToString::to_string).collect();
    assert!(
        spoken
            .iter()
            .any(|line| line.contains("Would remove 24 finished folders")),
        "{spoken:?}"
    );
    let mut at = vec![begun];
    at.extend(said.lock().unwrap().iter().copied());
    at.push(ended);
    let longest = at.windows(2).map(|w| w[1] - w[0]).max().unwrap();
    let whole = ended - begun;
    assert!(
        longest < whole / 2,
        "the asker heard nothing for {longest:?} of a {whole:?} prune of {folders} folders"
    );
}

#[test]
fn the_links_of_every_folder_are_read_saying_the_work_is_alive() {
    // #5439, on review: after every folder is judged, a `git status` of each is read for its
    // links, which on about 290 folders was minutes with nothing said.
    crate::unsteered!();
    let (_dir, plane) = a_plane();
    for n in 0..4 {
        plain(&plane, &format!("lane-{n}"));
    }
    let said: Arc<Mutex<Vec<Instant>>> = Arc::default();
    let lines = Arc::clone(&said);
    let pulse = alive::Pulse::saying(move || {
        lines.lock().unwrap().push(Instant::now());
        true
    });
    let links = alive::within(Some(&pulse), || crate::finished::links(&plane, "alpha"));
    assert!(links.is_empty(), "{links:?}");
    assert!(
        !said.lock().unwrap().is_empty(),
        "nothing was said while the links were read"
    );
}
