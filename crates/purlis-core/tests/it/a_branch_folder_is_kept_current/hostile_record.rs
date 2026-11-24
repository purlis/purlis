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
