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
        said.contains("update-refs") && said.contains("not something git"),
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

/// What the record may not hold, whatever it is called: a name git acts on (#7594, rounds 3 and 4).
#[test]
fn a_lower_case_pseudo_ref_a_merge_mode_link_and_a_name_git_acts_on_are_refused() {
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
    let file = w.plane.join("victim.txt");

    // A lower-case `orig_head`: a case-insensitive disk opens it as `ORIG_HEAD`.
    for name in ["orig_head", "Rebase_Head", "merge_head", "Auto_Merge"] {
        let at = record.join(name);
        let kept = std::fs::read(&at).ok();
        std::fs::write(&at, "ref: refs/heads/victim\n").unwrap();
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
        match kept {
            Some(kept) => std::fs::write(&at, kept).unwrap(),
            None => std::fs::remove_file(&at).unwrap(),
        }
    }

    // `MERGE_MODE`, linked: git writes it in place.
    std::fs::write(&file, "the person's own file\n").unwrap();
    std::os::unix::fs::symlink(&file, record.join("MERGE_MODE")).unwrap();
    let (code, said) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("a link"), "{said}");
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "the person's own file\n"
    );
    std::fs::remove_file(record.join("MERGE_MODE")).unwrap();

    // A name git acts on: refused, as a file or a folder.
    for (name, folder, said_of) in [
        ("config.worktree", false, "per-worktree git config"),
        ("Config.Worktree", false, "per-worktree git config"),
        ("sequencer", true, "did not start"),
        ("Rebase-Apply", true, "did not start"),
    ] {
        let at = record.join(name);
        if folder {
            std::fs::create_dir(&at).unwrap();
        } else {
            std::fs::write(&at, "[core]\n\thooksPath = /tmp\n").unwrap();
        }
        for step in ["update", "push"] {
            let (code, said) = if step == "update" {
                update(&w, "p1", &start(None), &Reach::default())
            } else {
                push(&w, "p1")
            };
            assert_eq!(code, 1, "{name} {step}: {said}");
            assert!(said.contains(said_of), "{name} {step}: {said}");
        }
        if folder {
            std::fs::remove_dir(&at).unwrap();
        } else {
            std::fs::remove_file(&at).unwrap();
        }
    }
    // A non-empty `refs/`: git reads per-worktree refs.
    std::fs::create_dir_all(record.join("refs/bisect")).unwrap();
    std::fs::write(record.join("refs/bisect/bad"), "0\n").unwrap();
    let (code, said) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("per-worktree ref"), "{said}");
    std::fs::remove_dir_all(record.join("refs")).unwrap();
    let (code, said) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 0, "{said}");
}

#[test]
fn a_refs_to_delete_in_the_rebase_state_is_refused_on_continue_and_on_abort_and_the_branch_stays() {
    purlis_core::unsteered!();
    let (w, piece, state) = stopped();
    resolve(&piece);
    git(&w.clone, &["branch", "victim"]);
    let victim = out(&w.clone, &["rev-parse", "refs/heads/victim"]);

    for name in ["refs-to-delete", "Refs-To-Delete", "autostash", "strategy"] {
        let at = state.join(name);
        std::fs::write(&at, "refs/heads/victim\n").unwrap();
        for step in [UpdateStep::Continue, UpdateStep::Abort] {
            let (code, said) = update(&w, "p1", &step, &Reach::default());
            assert_eq!(code, 1, "{name}: {said}");
            assert!(said.contains("not something git"), "{name}: {said}");
            assert!(state.is_dir(), "{name}: the rebase went on or back");
            assert_eq!(
                out(&w.clone, &["rev-parse", "refs/heads/victim"]),
                victim,
                "{name}: the branch went"
            );
        }
        std::fs::remove_file(&at).unwrap();
    }
    let (code, said) = update(&w, "p1", &UpdateStep::Continue, &Reach::default());
    assert_eq!(code, 0, "{said}");
}

/// Forgets the app's note of each piece's branch, as a piece cut before the app kept one has none.
fn forget_branch_notes(w: &World) {
    fn walk(dir: &Path) {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|name| name == "purlis-branch") {
                std::fs::remove_file(&path).unwrap();
            } else if path.is_dir() && !path.ends_with(".git") {
                walk(&path);
            }
        }
    }
    walk(&w.plane);
}

#[test]
fn a_piece_cut_before_the_note_is_noted_only_when_head_is_its_own_name() {
    purlis_core::unsteered!();
    let w = world();
    for (piece, branch) in [("p1", None), ("p2", Some("fix/7594-p2-token"))] {
        let (code, cut) = match branch {
            Some(branch) => add_on(&w, piece, branch),
            None => add(&w, piece, None),
        };
        assert_eq!(code, 0, "{cut}");
    }
    w.they_commit("README.md", "theirs\n");
    forget_branch_notes(&w);
    assert!(app_notes(&w).is_empty(), "{:?}", app_notes(&w));

    // The attack: p1's HEAD, written by the chat, names the branch another piece has.
    let record = w.clone.join(".git/worktrees/p1");
    let other = out(&w.clone, &["rev-parse", "refs/heads/fix/7594-p2-token"]);
    std::fs::write(record.join("HEAD"), "ref: refs/heads/fix/7594-p2-token\n").unwrap();
    for _ in 0..2 {
        let (code, said) = update(&w, "p1", &start(None), &Reach::default());
        assert_eq!(code, 1, "{said}");
        assert!(said.contains("no note"), "{said}");
    }
    let (code, said) = push(&w, "p1");
    assert_eq!(code, 1, "{said}");
    assert!(app_notes(&w).is_empty(), "noted: {:?}", app_notes(&w));
    assert_eq!(
        out(&w.clone, &["rev-parse", "refs/heads/fix/7594-p2-token"]),
        other
    );

    // A piece cut on a `--branch` name has nothing that proves it: refused, not guessed.
    let (code, said) = update(&w, "p2", &start(None), &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("no note"), "{said}");
    assert!(app_notes(&w).is_empty(), "noted: {:?}", app_notes(&w));

    // Put back on its own name, it is the one shape that is noted.
    std::fs::write(record.join("HEAD"), "ref: refs/heads/p1\n").unwrap();
    let (code, said) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert!(
        app_notes(&w).contains(&"purlis-branch".to_owned()),
        "{:?}",
        app_notes(&w)
    );
}

#[test]
fn a_cut_whose_branch_note_cannot_be_written_fails() {
    purlis_core::unsteered!();
    let w = world();
    let (code, cut) = add(&w, "p0", None);
    assert_eq!(code, 0, "{cut}");
    // Where the notes go is a file, so none can be made under it.
    fn find(dir: &Path) -> Option<PathBuf> {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path
                .file_name()
                .is_some_and(|name| name == "worktree-updates")
            {
                return Some(path);
            } else if path.is_dir()
                && !path.ends_with(".git")
                && let Some(found) = find(&path)
            {
                return Some(found);
            }
        }
        None
    }
    let notes = find(&w.plane).expect("the first cut noted its branch");
    std::fs::remove_dir_all(&notes).unwrap();
    std::fs::write(&notes, "in the way").unwrap();
    let (code, said) = add(&w, "p1", None);
    assert_ne!(code, 0, "{said}");
    assert!(said.contains("note"), "{said}");
}

/// What git and the person's tools leave in a record that git does not act on: the review's survey
/// of 198 records on one machine found these (#7594, round 4). Each is ignored, and the
/// attacks of rounds 1 to 3 beside them are still refused.
#[test]
fn the_names_git_and_other_tools_leave_in_a_record_are_ignored_and_the_attacks_beside_them_are_not()
{
    purlis_core::unsteered!();
    let w = world();
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    write(&piece, "shared.txt", "a\nmine\nc\n");
    git(&piece, &["commit", "-q", "-am", "mine"]);
    w.they_commit("README.md", "theirs\n");
    let record = w.clone.join(".git/worktrees/p1");

    for folder in [
        "glab-cli",
        "modules",
        "refs/bisect",
        "fsmonitor--daemon.d",
        "Info",
    ] {
        std::fs::create_dir_all(record.join(folder)).unwrap();
    }
    for file in [
        "gc.log",
        "purlis-updated",
        "sharedindex.d28f321c554dd2a81e9b71f04f6c6aa3cb159355",
        "fsmonitor--daemon",
    ] {
        std::fs::write(record.join(file), "x\n").unwrap();
    }
    std::fs::write(record.join("glab-cli/inner"), "not looked at\n").unwrap();
    let (code, said) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 0, "{said}");
    let (code, said) = push(&w, "p1");
    assert_eq!(code, 0, "{said}");

    // Still refused beside them: a link, by any name, and what git acts on.
    #[cfg(unix)]
    {
        let victim = w.plane.join("victim.txt");
        std::fs::write(&victim, "the person's own file\n").unwrap();
        std::os::unix::fs::symlink(&victim, record.join("gc.log.1")).unwrap();
        let (code, said) = update(&w, "p1", &start(None), &Reach::default());
        assert_eq!(code, 1, "{said}");
        assert!(said.contains("a link"), "{said}");
        std::fs::remove_file(record.join("gc.log.1")).unwrap();
    }
    std::fs::write(record.join("ORIG_HEAD"), "ref: refs/heads/main\n").unwrap();
    let (code, said) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("symbolic ref"), "{said}");
}

/// M1: a plan whose ids are abbreviated is not one the update wrote, since git resolves an
/// abbreviation to a ref of that name first.
#[test]
fn a_pick_of_an_abbreviated_id_that_a_ref_of_the_same_name_shadows_is_not_continued() {
    purlis_core::unsteered!();
    let (w, piece, state) = stopped();
    resolve(&piece);
    let todo = state.join("git-rebase-todo");
    let planned = String::from_utf8(std::fs::read(&todo).unwrap()).unwrap();
    let full = planned
        .lines()
        .find_map(|line| line.strip_prefix("pick "))
        .and_then(|rest| rest.split_whitespace().next())
        .expect("a pick")
        .to_owned();
    let short = &full[..8];
    // A branch called as the abbreviation, on a commit nobody planned.
    let elsewhere = out(&w.theirs, &["rev-parse", "HEAD"]);
    git(&w.clone, &["branch", short, &elsewhere]);
    std::fs::write(&todo, planned.replacen(&full, short, 1)).unwrap();
    let (code, said) = update(&w, "p1", &UpdateStep::Continue, &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("does not plan"), "{said}");
    assert!(todo.is_file(), "the rebase went on");
    std::fs::write(&todo, planned).unwrap();
    let (code, said) = update(&w, "p1", &UpdateStep::Continue, &Reach::default());
    assert_eq!(code, 0, "{said}");
}

/// `purlis worktree adopt`: a piece cut on a `--branch` name before the note was kept is
/// recorded by the person, and not where another folder has the branch.
#[test]
fn adopt_records_a_branch_piece_and_refuses_a_branch_held_elsewhere() {
    purlis_core::unsteered!();
    let w = world();
    let (code, cut) = add_on(&w, "p1", "fix/7594-p1-token");
    assert_eq!(code, 0, "{cut}");
    w.they_commit("README.md", "theirs\n");
    forget_branch_notes(&w);

    // No note, and the folder's name is not its branch: the app's git refuses, naming the fix.
    let (code, refusal) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 1, "{refusal}");
    assert!(refusal.contains("purlis worktree adopt"), "{refusal}");

    let adopt = |piece: &str| {
        let mut lines = Vec::new();
        let code = brokered(|| {
            purlis_core::piececmd::adopt(&w.plane, WS, REPO, piece, &mut |l| lines.push(l))
        });
        (code, said(&lines))
    };
    let (code, adopted) = adopt("p1");
    assert_eq!(code, 0, "{adopted}");
    assert!(adopted.contains("fix/7594-p1-token"), "{adopted}");
    assert!(app_notes(&w).contains(&"purlis-branch".to_owned()));
    let (code, ran) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 0, "{ran}");

    // p2's folder is put on the branch p1's folder holds: not adopted.
    let (code, cut) = add_on(&w, "p2", "fix/7594-p2-token");
    assert_eq!(code, 0, "{cut}");
    forget_branch_notes(&w);
    git(
        &w.piece("p2"),
        &[
            "checkout",
            "-q",
            "--ignore-other-worktrees",
            "fix/7594-p1-token",
        ],
    );
    let (code, refused) = adopt("p2");
    assert_eq!(code, 1, "{refused}");
    assert!(refused.contains("another worktree"), "{refused}");
}
