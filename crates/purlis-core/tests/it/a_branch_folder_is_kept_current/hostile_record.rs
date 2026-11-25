// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #7553: where a chat can write the worktree's git record (`.git/worktrees/<piece>/`), the
//! app's own git is not steered by it: a link planted in it is refused, and a rebase the app
//! stopped is continued only as the app left it.

use super::*;

/// A piece with two commits of its own, the first of which conflicts with a commit on the
/// forge, and the update that stopped on it: the rebase's plan then holds the second.
fn stopped() -> (World, PathBuf, PathBuf) {
    let w = world();
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    write(&piece, "shared.txt", "a\nmine\nc\n");
    git(&piece, &["commit", "-q", "-am", "mine"]);
    write(&piece, "second.txt", "two\n");
    git(&piece, &["add", "second.txt"]);
    git(&piece, &["commit", "-q", "-m", "second"]);
    w.they_commit("shared.txt", "a\ntheirs\nc\n");
    let (code, said) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 1, "{said}");
    let state = w.clone.join(".git/worktrees/p1/rebase-merge");
    assert!(state.join("git-rebase-todo").is_file());
    (w, piece, state)
}

fn resolve(piece: &Path) {
    write(piece, "shared.txt", "a\nboth\nc\n");
}

#[test]
fn a_plan_changed_after_the_update_stopped_is_not_continued() {
    purlis_core::unsteered!();
    let (w, piece, state) = stopped();
    resolve(&piece);
    let todo = state.join("git-rebase-todo");
    let planned = std::fs::read(&todo).unwrap();
    let marker = w.plane.join("pwned");
    let elsewhere = out(&w.theirs, &["rev-parse", "HEAD"]);

    for (what, tampered) in [
        ("an exec line", format!("exec touch {}\n", marker.display())),
        (
            "a pick of a commit nobody planned",
            format!("pick {elsewhere}\n"),
        ),
        ("a pick, dropped", String::new()),
    ] {
        let mut bytes = planned.clone();
        if what.contains("dropped") {
            bytes = bytes
                .split(|b| *b == b'\n')
                .filter(|line| !line.starts_with(b"pick"))
                .flat_map(|line| [line, b"\n"].concat())
                .collect();
        } else {
            bytes.extend_from_slice(tampered.as_bytes());
        }
        std::fs::write(&todo, bytes).unwrap();
        let (code, said) = update(&w, "p1", &UpdateStep::Continue, &Reach::default());
        assert_eq!(code, 1, "{what}: {said}");
        assert!(said.contains("does not plan"), "{what}: {said}");
        assert!(!marker.exists(), "{what}: the exec line ran");
        assert!(todo.is_file(), "{what}: the rebase went on");
    }
    std::fs::write(&todo, &planned).unwrap();

    // `update-refs` would move a branch the update never meant to.
    let refs = state.join("update-refs");
    std::fs::write(&refs, "refs/heads/main\n").unwrap();
    let (code, said) = update(&w, "p1", &UpdateStep::Continue, &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(
        said.contains("update-refs") && said.contains("does not plan"),
        "{said}"
    );
    assert!(state.is_dir(), "the rebase went on");
    std::fs::remove_file(&refs).unwrap();

    // As the app left it, it goes on.
    let (code, said) = update(&w, "p1", &UpdateStep::Continue, &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert_eq!(out(&piece, &["log", "-1", "--format=%s"]), "second");
}

#[cfg(unix)]
#[test]
fn a_link_planted_in_the_worktrees_record_refuses_every_step_and_is_never_written_through() {
    purlis_core::unsteered!();
    let w = world();
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    write(&piece, "shared.txt", "a\nmine\nc\n");
    git(&piece, &["commit", "-q", "-am", "mine"]);
    w.they_commit("README.md", "theirs\n");
    let record = w.clone.join(".git/worktrees/p1");
    let victim = w.plane.join("victim.txt");

    for name in [
        "COMMIT_EDITMSG",
        "logs/HEAD",
        "FETCH_HEAD",
        "index",
        "ORIG_HEAD",
    ] {
        std::fs::write(&victim, "the person's own file\n").unwrap();
        let at = record.join(name);
        let kept = std::fs::read(&at).ok();
        let _ = std::fs::remove_file(&at);
        std::os::unix::fs::symlink(&victim, &at).unwrap();

        let (code, said) = update(&w, "p1", &start(None), &Reach::default());
        assert_eq!(code, 1, "{name}: {said}");
        assert!(said.contains("a link"), "{name}: {said}");
        assert_eq!(
            std::fs::read_to_string(&victim).unwrap(),
            "the person's own file\n",
            "{name}: written through"
        );
        let (code, said) = push(&w, "p1");
        assert_eq!(code, 1, "{name}: {said}");
        assert_eq!(
            std::fs::read_to_string(&victim).unwrap(),
            "the person's own file\n",
            "{name}: written through by a push"
        );

        std::fs::remove_file(&at).unwrap();
        if let Some(kept) = kept {
            std::fs::write(&at, kept).unwrap();
        }
    }
    let (code, said) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 0, "{said}");
}

/// [`add`], on a branch of its own that is not the folder's name (`--branch`).
fn add_on(w: &World, piece: &str, branch: &str) -> (u8, String) {
    let mut lines = Vec::new();
    let code = brokered(|| {
        purlis_core::piececmd::add(
            &w.plane,
            WS,
            REPO,
            piece,
            Some(branch),
            None,
            None,
            None,
            &Who::here(None, Some("3".to_owned()), None),
            jiff::Timestamp::now(),
            &mut |line| lines.push(line),
        )
    });
    (code, said(&lines))
}

/// Every file of the app's notes for the plane's branch folders (`app/worktree-updates/*/`).
fn app_notes(w: &World) -> Vec<String> {
    fn walk(dir: &Path, found: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path
                .file_name()
                .is_some_and(|name| name == "worktree-updates")
            {
                for notes in std::fs::read_dir(&path).into_iter().flatten().flatten() {
                    for file in std::fs::read_dir(notes.path())
                        .into_iter()
                        .flatten()
                        .flatten()
                    {
                        found.push(file.file_name().to_string_lossy().into_owned());
                    }
                }
            } else if path.is_dir() && !path.ends_with(".git") {
                walk(&path, found);
            }
        }
    }
    let mut found = Vec::new();
    walk(&w.plane, &mut found);
    found.sort();
    found
}

#[test]
fn a_pseudo_ref_holding_a_symbolic_ref_refuses_update_and_push_and_the_branch_it_names_does_not_move()
 {
    purlis_core::unsteered!();
    let w = world();
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    write(&piece, "shared.txt", "a\nmine\nc\n");
    git(&piece, &["commit", "-q", "-am", "mine"]);
    git(&w.clone, &["branch", "victim"]);
    let victim = out(&w.clone, &["rev-parse", "refs/heads/victim"]);
    w.they_commit("README.md", "theirs\n");
    let record = w.clone.join(".git/worktrees/p1");

    for name in ["ORIG_HEAD", "REBASE_HEAD", "AUTO_MERGE", "MERGE_HEAD"] {
        std::fs::write(record.join(name), "ref: refs/heads/victim\n").unwrap();
        let (code, said) = update(&w, "p1", &start(None), &Reach::default());
        assert_eq!(code, 1, "{name}: {said}");
        assert!(said.contains("symbolic ref"), "{name}: {said}");
        let (code, said) = push(&w, "p1");
        assert_eq!(code, 1, "{name}: {said}");
        assert_eq!(
            out(&w.clone, &["rev-parse", "refs/heads/victim"]),
            victim,
            "{name}"
        );
        std::fs::remove_file(record.join(name)).unwrap();
    }
    let (code, said) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 0, "{said}");
}

#[test]
fn a_head_repointed_to_another_branch_refuses_update_and_push_and_that_branch_does_not_move() {
    purlis_core::unsteered!();
    let w = world();
    // Cut on a branch that is not the folder's name, so the folder's name is no help.
    let (code, cut) = add_on(&w, "p1", "fix/6494-p1-token");
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    write(&piece, "shared.txt", "a\nmine\nc\n");
    git(&piece, &["commit", "-q", "-am", "mine"]);
    w.they_commit("README.md", "theirs\n");

    // As cut: updates and pushes, though the branch and the folder differ.
    let (code, said) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 0, "{said}");
    let (code, said) = push(&w, "p1");
    assert_eq!(code, 0, "{said}");

    // Repointed to the folder's own name, or to another branch: neither is the cut's branch.
    let record = w.clone.join(".git/worktrees/p1");
    git(&w.clone, &["branch", "p1", "main"]);
    for other in ["p1", "main"] {
        let before = out(&w.clone, &["rev-parse", &format!("refs/heads/{other}")]);
        std::fs::write(record.join("HEAD"), format!("ref: refs/heads/{other}\n")).unwrap();
        w.they_commit("README.md", &format!("theirs {other}\n"));
        let (code, said) = update(&w, "p1", &start(None), &Reach::default());
        assert_eq!(code, 1, "{other}: {said}");
        assert!(said.contains("fix/6494-p1-token"), "{other}: {said}");
        let (code, said) = push(&w, "p1");
        assert_eq!(code, 1, "{other}: {said}");
        assert!(said.contains("fix/6494-p1-token"), "{other}: {said}");
        assert_eq!(
            out(&w.clone, &["rev-parse", &format!("refs/heads/{other}")]),
            before,
            "{other} moved"
        );
    }
}

#[test]
fn a_continue_or_an_abort_with_a_tampered_rebase_record_is_refused_and_no_other_branch_moves() {
    purlis_core::unsteered!();
    let (w, piece, state) = stopped();
    resolve(&piece);
    git(&w.clone, &["branch", "victim"]);
    let victim = out(&w.clone, &["rev-parse", "refs/heads/victim"]);
    let elsewhere = out(&w.theirs, &["rev-parse", "HEAD~1"]);
    let record = w.clone.join(".git/worktrees/p1");

    for (what, file, tampered) in [
        ("a head-name", "head-name", "refs/heads/victim\n".to_owned()),
        ("an orig-head", "orig-head", format!("{elsewhere}\n")),
        ("an onto", "onto", format!("{elsewhere}\n")),
        ("an amend", "amend", format!("{elsewhere}\n")),
        ("an author-script", "author-script", "exec x\n".to_owned()),
        ("a longer plan", "end", "9\n".to_owned()),
        ("a msgnum", "msgnum", "2\n".to_owned()),
    ] {
        let at = state.join(file);
        let kept = std::fs::read(&at).ok();
        std::fs::write(&at, tampered).unwrap();
        // `--abort` reads no plan: its count is the one thing only `--continue` is held to.
        let steps: &[UpdateStep] = if matches!(file, "end" | "msgnum") {
            &[UpdateStep::Continue]
        } else {
            &[UpdateStep::Continue, UpdateStep::Abort]
        };
        for step in steps {
            let (code, said) = update(&w, "p1", step, &Reach::default());
            assert_eq!(code, 1, "{what}: {said}");
            assert!(
                said.contains("does not plan") || said.contains("not one purlis started"),
                "{what}: {said}"
            );
            assert!(state.is_dir(), "{what}: the rebase went on or back");
            assert_eq!(
                out(&w.clone, &["rev-parse", "refs/heads/victim"]),
                victim,
                "{what}"
            );
        }
        match kept {
            Some(kept) => std::fs::write(&at, kept).unwrap(),
            None => std::fs::remove_file(&at).unwrap(),
        }
    }

    // What is read when git runs is looked at then: a pseudo-ref planted as a symbolic one, or a
    // HEAD that names a branch, is refused too.
    for (name, text) in [
        ("ORIG_HEAD", "ref: refs/heads/victim\n"),
        ("HEAD", "ref: refs/heads/victim\n"),
    ] {
        let at = record.join(name);
        let kept = std::fs::read(&at).ok();
        std::fs::write(&at, text).unwrap();
        for step in [UpdateStep::Continue, UpdateStep::Abort] {
            let (code, said) = update(&w, "p1", &step, &Reach::default());
            assert_eq!(code, 1, "{name}: {said}");
            assert!(state.is_dir(), "{name}: the rebase went on or back");
            assert_eq!(
                out(&w.clone, &["rev-parse", "refs/heads/victim"]),
                victim,
                "{name}"
            );
        }
        match kept {
            Some(kept) => std::fs::write(&at, kept).unwrap(),
            None => std::fs::remove_file(&at).unwrap(),
        }
    }

    // As the app left it, it goes on.
    let (code, said) = update(&w, "p1", &UpdateStep::Continue, &Reach::default());
    assert_eq!(code, 0, "{said}");
}

#[test]
fn the_plan_a_rebase_goes_on_with_is_the_apps_own_and_its_notes_go_when_it_finishes_or_is_put_back()
{
    purlis_core::unsteered!();
    // Finished.
    let (w, piece, _state) = stopped();
    assert!(
        app_notes(&w).contains(&"purlis-update-range".to_owned()),
        "{:?}",
        app_notes(&w)
    );
    resolve(&piece);
    let (code, said) = update(&w, "p1", &UpdateStep::Continue, &Reach::default());
    assert_eq!(code, 0, "{said}");
    let left = app_notes(&w);
    assert!(
        !left.contains(&"purlis-update-range".to_owned())
            && !left.contains(&"purlis-update-todo".to_owned()),
        "{left:?}"
    );

    // Put back.
    let (w, piece, _state) = stopped();
    let before = out(&piece, &["rev-parse", "refs/heads/p1"]);
    let (code, said) = update(&w, "p1", &UpdateStep::Abort, &Reach::default());
    assert_eq!(code, 0, "{said}");
    let left = app_notes(&w);
    assert!(
        !left.contains(&"purlis-update-range".to_owned())
            && !left.contains(&"purlis-update-todo".to_owned()),
        "{left:?}"
    );
    assert_eq!(out(&piece, &["rev-parse", "refs/heads/p1"]), before);
}
