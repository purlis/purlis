// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #5253: `purlis worktree update --merge`, which brings a branch folder's base in without
//! rewriting a commit, for a sandboxed chat that may not run `git merge` there itself. The app
//! runs it outside that chat's sandbox, so nothing the chat could have written may start.

use super::*;

fn merge(base: Option<&str>) -> UpdateStep {
    UpdateStep::Merge {
        base: base.map(str::to_owned),
        take_base_baselines: false,
    }
}

const TAKING_BASELINES: UpdateStep = UpdateStep::Merge {
    base: None,
    take_base_baselines: true,
};

fn parents(dir: &Path) -> Vec<String> {
    out(dir, &["log", "-1", "--format=%P"])
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// `path` in `dir`, its folders made, holding `bytes`.
fn put(dir: &Path, path: &str, bytes: &[u8]) {
    let at = dir.join(path);
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(at, bytes).unwrap();
}

/// A commit of each of `files`, in `dir`, on what it has checked out.
fn commit_files(dir: &Path, files: &[(&str, &[u8])]) {
    for (path, bytes) in files {
        put(dir, path, bytes);
        git(dir, &["add", "--", path]);
    }
    git(dir, &["commit", "-q", "-m", "files"]);
}

/// Someone else's commit of each of `files` on the forge's `main`.
fn they_commit_files(w: &World, files: &[(&str, &[u8])]) {
    git(&w.theirs, &["pull", "-q", "--ff-only", "origin", "main"]);
    commit_files(&w.theirs, files);
    git(&w.theirs, &["push", "-q", "origin", "HEAD:main"]);
}

/// [`they_commit_files`], with the clone told, so a piece cut after this starts with them.
fn they_had_already(w: &World, files: &[(&str, &[u8])]) {
    they_commit_files(w, files);
    git(&w.clone, &["pull", "-q", "--ff-only", "origin", "main"]);
}

const PICTURE: &str = "app/src/__screenshots__/Card.stories.tsx/card-dark-chromium-linux.png";
const PICTURE_TWO: &str = "app/src/__screenshots__/Card.stories.tsx/card-dark-webkit-linux.png";
/// Bytes git reads as binary, as a PNG's are, so a conflict in them has no markers.
const OURS: &[u8] = b"\x89PNG\r\n\x1a\n\0ours";
const BASE: &[u8] = b"\x89PNG\r\n\x1a\n\0base";
const WAS: &[u8] = b"\x89PNG\r\n\x1a\n\0was";

/// A pushed piece and a forge's `main` that each redrew the two pictures they had in common.
fn a_piece_whose_pictures_conflict(w: &World, name: &str) -> PathBuf {
    they_had_already(w, &[(PICTURE, WAS), (PICTURE_TWO, WAS)]);
    let piece = pushed_piece(w, name);
    commit_files(&piece, &[(PICTURE, OURS), (PICTURE_TWO, OURS)]);
    let (code, said) = push(w, name);
    assert_eq!(code, 0, "{said}");
    they_commit_files(w, &[(PICTURE, BASE), (PICTURE_TWO, BASE)]);
    piece
}

#[test]
fn a_merge_brings_the_base_in_rewrites_no_commit_and_a_plain_push_follows() {
    purlis_core::unsteered!();
    let w = world();
    let piece = pushed_piece(&w, "p1");
    let mine = out(&piece, &["rev-parse", "HEAD"]);
    w.they_commit("theirs.txt", "theirs\n");
    let main = out(&w.theirs, &["rev-parse", "HEAD"]);

    let (code, said) = update(&w, "p1", &merge(None), &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("purlis worktree push"), "{said}");
    assert_eq!(parents(&piece), [mine.clone(), main]);
    assert_eq!(
        out(&piece, &["branch", "--show-current"]),
        "p1",
        "the folder stays on its branch"
    );
    assert!(piece.join("theirs.txt").exists());

    let (code, said) = push(&w, "p1");
    assert_eq!(code, 0, "{said}");
    assert_eq!(forge_has(&w, "p1"), out(&piece, &["rev-parse", "HEAD"]));
    assert_eq!(
        out(&w.forge, &["merge-base", &mine, "refs/heads/p1"]),
        mine,
        "the commit already pushed is still in the branch"
    );

    // Nothing left to bring in.
    let (code, said) = update(&w, "p1", &merge(None), &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("already"), "{said}");
}

#[test]
fn a_merge_conflict_stops_naming_the_files_and_continue_or_abort_finishes_it() {
    purlis_core::unsteered!();
    let w = world();
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    commit_in(&piece, "shared.txt", "a\nmine\nc\n");
    let mine = out(&piece, &["rev-parse", "HEAD"]);
    w.they_commit("shared.txt", "a\ntheirs\nc\n");
    let main = out(&w.theirs, &["rev-parse", "HEAD"]);

    let (code, said) = update(&w, "p1", &merge(None), &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("conflict in shared.txt"), "{said}");
    assert!(said.contains("--continue"), "{said}");
    assert!(said.contains("--abort"), "{said}");

    // Nothing else moves while it is stopped.
    let (code, said) = push(&w, "p1");
    assert_eq!(code, 1, "{said}");
    let (code, said) = update(&w, "p1", &merge(None), &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("stopped"), "{said}");

    // A marker left in is refused, and the merge stays stopped.
    let (code, said) = update(&w, "p1", &UpdateStep::Continue, &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("conflict marker"), "{said}");
    assert_eq!(out(&piece, &["rev-parse", "HEAD"]), mine);

    // Abort puts the branch back, and its file.
    let (code, said) = update(&w, "p1", &UpdateStep::Abort, &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert_eq!(out(&piece, &["rev-parse", "HEAD"]), mine);
    assert_eq!(
        std::fs::read_to_string(piece.join("shared.txt")).unwrap(),
        "a\nmine\nc\n"
    );

    // Again, resolved this time.
    let (code, _) = update(&w, "p1", &merge(None), &Reach::default());
    assert_eq!(code, 1);
    write(&piece, "shared.txt", "a\nboth\nc\n");
    let (code, said) = update(&w, "p1", &UpdateStep::Continue, &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert_eq!(parents(&piece), [mine, main]);
    assert_eq!(out(&piece, &["show", "HEAD:shared.txt"]), "a\nboth\nc");
    assert_eq!(
        out(&piece, &["log", "-1", "--format=%s"]),
        "Merge main into p1"
    );

    // Nothing is stopped now, and it is pushed as any other commit.
    let (code, said) = update(&w, "p1", &UpdateStep::Abort, &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("no rebase is stopped"), "{said}");
    let (code, said) = push(&w, "p1");
    assert_eq!(code, 0, "{said}");
}

#[test]
fn a_merge_taking_the_bases_baselines_finishes_with_no_stop_and_is_pushed() {
    purlis_core::unsteered!();
    // The #5154 round: main redrew the pictures a pull request redrew too.
    let w = world();
    let piece = a_piece_whose_pictures_conflict(&w, "p1");
    let mine = out(&piece, &["rev-parse", "HEAD"]);
    let main = out(&w.theirs, &["rev-parse", "HEAD"]);

    let (code, said) = update(&w, "p1", &TAKING_BASELINES, &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert_eq!(parents(&piece), [mine, main]);
    for picture in [PICTURE, PICTURE_TWO] {
        assert_eq!(std::fs::read(piece.join(picture)).unwrap(), BASE);
    }
    assert_eq!(
        out(&piece, &["status", "--porcelain", "--untracked-files=no"]),
        ""
    );

    let (code, said) = push(&w, "p1");
    assert_eq!(code, 0, "{said}");
    assert_eq!(forge_has(&w, "p1"), out(&piece, &["rev-parse", "HEAD"]));
}

#[test]
fn a_merge_not_asked_to_take_baselines_stops_on_them_and_says_how_to_take_them() {
    purlis_core::unsteered!();
    let w = world();
    let piece = a_piece_whose_pictures_conflict(&w, "p1");

    let (code, said) = update(&w, "p1", &merge(None), &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains(PICTURE), "{said}");
    assert!(said.contains("--take-base-baselines"), "{said}");
    assert_eq!(std::fs::read(piece.join(PICTURE)).unwrap(), OURS);
}

#[test]
fn only_a_conflicted_png_under_a_screenshots_folder_takes_the_bases_copy() {
    purlis_core::unsteered!();
    // Taking the base's copy of anything else would lose someone's work without a word.
    const ELSEWHERE: &str = "app/src/assets/logo.png";
    const NOT_A_PICTURE: &str = "app/src/__screenshots__/notes.bin";
    let w = world();
    they_had_already(
        &w,
        &[(PICTURE, WAS), (ELSEWHERE, WAS), (NOT_A_PICTURE, WAS)],
    );
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    commit_files(
        &piece,
        &[
            (PICTURE, OURS),
            (ELSEWHERE, OURS),
            (NOT_A_PICTURE, OURS),
            ("shared.txt", b"a\nmine\nc\n"),
        ],
    );
    they_commit_files(
        &w,
        &[
            (PICTURE, BASE),
            (ELSEWHERE, BASE),
            (NOT_A_PICTURE, BASE),
            ("shared.txt", b"a\ntheirs\nc\n"),
        ],
    );

    let (code, said) = update(&w, "p1", &TAKING_BASELINES, &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains(ELSEWHERE), "{said}");
    assert!(said.contains(NOT_A_PICTURE), "{said}");
    assert!(said.contains("shared.txt"), "{said}");
    assert!(
        !said.contains("card-dark"),
        "the picture is settled: {said}"
    );
    assert_eq!(std::fs::read(piece.join(PICTURE)).unwrap(), BASE);
    assert_eq!(std::fs::read(piece.join(ELSEWHERE)).unwrap(), OURS);

    // The rest resolved by hand, and the picture stays as it was taken.
    write(&piece, "shared.txt", "a\nboth\nc\n");
    let (code, said) = update(&w, "p1", &UpdateStep::Continue, &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert_eq!(out(&piece, &["show", "HEAD:shared.txt"]), "a\nboth\nc");
    let at = format!("HEAD:{PICTURE}");
    assert_eq!(
        out(&piece, &["rev-parse", &at]),
        out(&w.theirs, &["rev-parse", &at])
    );
}

#[test]
fn a_baseline_the_base_deleted_or_the_branch_deleted_takes_the_bases_side() {
    purlis_core::unsteered!();
    let w = world();
    they_had_already(&w, &[(PICTURE, WAS), (PICTURE_TWO, WAS)]);
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    // The branch redrew the first and removed the second; main removed the first and redrew
    // the second.
    put(&piece, PICTURE, OURS);
    git(&piece, &["add", "--", PICTURE]);
    git(&piece, &["rm", "-q", "--", PICTURE_TWO]);
    git(&piece, &["commit", "-q", "-m", "mine"]);
    git(&w.theirs, &["pull", "-q", "--ff-only", "origin", "main"]);
    git(&w.theirs, &["rm", "-q", "--", PICTURE]);
    put(&w.theirs, PICTURE_TWO, BASE);
    git(&w.theirs, &["add", "--", PICTURE_TWO]);
    git(&w.theirs, &["commit", "-q", "-m", "theirs"]);
    git(&w.theirs, &["push", "-q", "origin", "HEAD:main"]);

    let (code, said) = update(&w, "p1", &TAKING_BASELINES, &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert!(!piece.join(PICTURE).exists(), "{said}");
    assert_eq!(std::fs::read(piece.join(PICTURE_TWO)).unwrap(), BASE);
    assert_eq!(parents(&piece).len(), 2);
}

/// Everything a clone's config, its hooks folder and its attributes could name for git to run,
/// each leaving a mark in `<dir>/ran` under its own name if it does. Planted last, because the
/// test's own git obeys it. Which keys are held is the runner's business: this is what must not
/// start, and what a key that merely stops git is noticed by.
fn plant_programs(w: &World, name: &str, dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let ran = dir.join("ran");
    std::fs::create_dir_all(&ran).unwrap();
    let mark = |what: &str| format!("touch {}/{what}", ran.display());
    let hooks = dir.join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let git_dir = PathBuf::from(out(&w.clone, &["rev-parse", "--absolute-git-dir"]));
    std::fs::create_dir_all(git_dir.join("hooks")).unwrap();
    std::fs::create_dir_all(git_dir.join("info")).unwrap();
    for hook in [
        "pre-merge-commit",
        "prepare-commit-msg",
        "commit-msg",
        "post-commit",
        "post-merge",
        "post-checkout",
        "post-index-change",
        "reference-transaction",
        "pre-commit",
        "pre-auto-gc",
    ] {
        for (place, label) in [(hooks.clone(), "path"), (git_dir.join("hooks"), "clone")] {
            let script = place.join(hook);
            let body = format!("#!/bin/sh\n{}\n", mark(&format!("hook-{label}-{hook}")));
            std::fs::write(&script, body).unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    std::fs::write(
        git_dir.join("info").join("attributes"),
        "shared.txt merge=planted diff=planted\n\
         incoming.txt filter=planted\n\
         *.png filter=planted diff=planted\n",
    )
    .unwrap();
    let options = format!("branch.{name}.mergeOptions");
    let keys: [(&str, String); 30] = [
        ("core.hooksPath", hooks.display().to_string()),
        ("core.fsmonitor", mark("fsmonitor")),
        ("core.editor", mark("editor")),
        ("sequence.editor", mark("sequence-editor")),
        ("core.pager", mark("pager")),
        ("core.sshCommand", mark("ssh")),
        ("core.alternateRefsCommand", mark("alternate-refs")),
        ("merge.planted.driver", mark("merge-driver")),
        ("merge.default", "planted".to_owned()),
        ("merge.tool", "planted".to_owned()),
        ("mergetool.planted.cmd", mark("merge-tool")),
        ("filter.planted.clean", mark("filter-clean")),
        ("filter.planted.smudge", mark("filter-smudge")),
        ("filter.planted.process", mark("filter-process")),
        ("filter.planted.required", "true".to_owned()),
        ("diff.external", mark("diff-external")),
        ("diff.planted.command", mark("diff-command")),
        ("diff.planted.textconv", mark("diff-textconv")),
        ("gpg.program", mark("gpg")),
        ("gpg.ssh.program", mark("gpg-ssh")),
        ("commit.gpgSign", "true".to_owned()),
        ("merge.verifySignatures", "true".to_owned()),
        ("trailer.planted.command", mark("trailer")),
        ("pull.twohead", "planted".to_owned()),
        ("merge.autoStash", "true".to_owned()),
        ("merge.ff", "only".to_owned()),
        ("rerere.enabled", "true".to_owned()),
        (options.as_str(), "-s planted --no-commit".to_owned()),
        ("hook.planted.command", mark("hook-by-config")),
        ("hook.planted.event", "post-merge".to_owned()),
    ];
    for (key, value) in keys {
        git(&w.clone, &["config", key, &value]);
    }
    ran
}

fn nothing_ran(ran: &Path, after: &str) {
    let marks: Vec<String> = std::fs::read_dir(ran)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(marks.is_empty(), "after {after}, these ran: {marks:?}");
}

#[test]
fn a_merge_the_app_runs_starts_no_program_the_repository_names() {
    purlis_core::unsteered!();
    // At each of its steps: no hook, driver, filter, editor, signer or strategy.
    let w = world();
    let elsewhere = tempfile::tempdir().unwrap();
    they_had_already(&w, &[(PICTURE, WAS)]);
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    commit_files(&piece, &[(PICTURE, OURS), ("shared.txt", b"a\nmine\nc\n")]);
    let mine = out(&piece, &["rev-parse", "HEAD"]);
    they_commit_files(
        &w,
        &[
            (PICTURE, BASE),
            ("shared.txt", b"a\ntheirs\nc\n"),
            ("incoming.txt", b"new\n"),
        ],
    );
    let main = out(&w.theirs, &["rev-parse", "HEAD"]);
    let ran = plant_programs(&w, "p1", elsewhere.path());

    // Start, take the picture's copy, and stop on the text file.
    let (code, said) = update(&w, "p1", &TAKING_BASELINES, &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("conflict in shared.txt"), "{said}");
    nothing_ran(&ran, "the merge starting");
    assert_eq!(std::fs::read(piece.join(PICTURE)).unwrap(), BASE);

    // Back.
    let (code, said) = update(&w, "p1", &UpdateStep::Abort, &Reach::default());
    assert_eq!(code, 0, "{said}");
    nothing_ran(&ran, "the merge aborted");
    assert_eq!(out(&piece, &["rev-parse", "HEAD"]), mine);

    // And through to its commit.
    let (code, said) = update(&w, "p1", &TAKING_BASELINES, &Reach::default());
    assert_eq!(code, 1, "{said}");
    write(&piece, "shared.txt", "a\nboth\nc\n");
    let (code, said) = update(&w, "p1", &UpdateStep::Continue, &Reach::default());
    assert_eq!(code, 0, "{said}");
    nothing_ran(&ran, "the merge committed");
    assert_eq!(parents(&piece), [mine, main]);
}

#[test]
fn what_is_planted_does_run_for_a_git_nothing_holds() {
    purlis_core::unsteered!();
    // The control: "nothing ran" above means something only if these would have.
    let w = world();
    let elsewhere = tempfile::tempdir().unwrap();
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    commit_in(&piece, "shared.txt", "a\nmine\nc\n");
    w.they_commit("shared.txt", "a\ntheirs\nc\n");
    git(&piece, &["fetch", "-q", "origin"]);
    let ran = plant_programs(&w, "p1", elsewhere.path());

    // Only what would stop this merge outright is lifted, so the rest is reached.
    let mut plain = support::unsigned();
    plain
        .args(["-c", "user.name=t", "-c", "user.email=t@e.invalid"])
        .args(["-c", "branch.p1.mergeOptions=", "-c", "pull.twohead=ort"])
        .args(["-c", "merge.ff=true", "-c", "merge.verifySignatures=false"])
        .args(["-c", "commit.gpgSign=false", "-C"])
        .arg(&piece)
        .args(["merge", "--no-edit", "origin/main"])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null");
    purlis_core::forklock::output(&mut plain).expect("git runs");

    let marks: Vec<String> = std::fs::read_dir(&ran)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    for wanted in ["merge-driver", "hook-path-post-merge"] {
        assert!(marks.iter().any(|mark| mark == wanted), "{marks:?}");
    }
}

#[test]
fn a_merge_given_no_conflict_also_starts_no_program_and_finishes() {
    purlis_core::unsteered!();
    let w = world();
    let elsewhere = tempfile::tempdir().unwrap();
    let piece = pushed_piece(&w, "p1");
    let mine = out(&piece, &["rev-parse", "HEAD"]);
    w.they_commit("incoming.txt", "new\n");
    let main = out(&w.theirs, &["rev-parse", "HEAD"]);
    let ran = plant_programs(&w, "p1", elsewhere.path());

    let (code, said) = update(&w, "p1", &merge(None), &Reach::default());
    assert_eq!(code, 0, "{said}");
    nothing_ran(&ran, "a clean merge");
    assert_eq!(parents(&piece), [mine, main]);
    assert_eq!(
        std::fs::read_to_string(piece.join("incoming.txt")).unwrap(),
        "new\n",
        "written as the base has it, through no filter"
    );
}

#[test]
fn a_merge_never_moves_a_file_of_the_base_into_a_folder_the_branch_renamed() {
    purlis_core::unsteered!();
    // #1948: git's own rename detection can write a path neither commit's difference names,
    // which the judging of a sandboxed chat's merge could not have read. So it is off.
    let w = world();
    they_had_already(
        &w,
        &[
            ("old/1.txt", b"1\n"),
            ("old/2.txt", b"2\n"),
            ("old/3.txt", b"3\n"),
        ],
    );
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    git(&piece, &["mv", "old", "new"]);
    git(&piece, &["commit", "-q", "-m", "renamed"]);
    they_commit_files(&w, &[("old/4.txt", b"4\n")]);

    let (code, said) = update(&w, "p1", &merge(None), &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert!(piece.join("old/4.txt").exists(), "{said}");
    assert!(!piece.join("new/4.txt").exists(), "{said}");
}

#[test]
fn a_merge_for_a_sandboxed_chat_that_would_write_a_denied_file_does_not_start() {
    purlis_core::unsteered!();
    let w = world();
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    commit_in(&piece, "mine.txt", "mine\n");
    std::fs::create_dir_all(w.theirs.join(".vscode")).unwrap();
    w.they_commit(".vscode/tasks.json", "{}\n");
    let before = out(&piece, &["rev-parse", "HEAD"]);
    let chat = w.plane.join("workspaces").join(WS);
    let reach = Reach {
        hosts: None,
        sandboxed_at: Some(&chat),
        for_a_chat: true,
    };

    let (code, said) = update(&w, "p1", &merge(None), &reach);
    assert_eq!(code, 1, "{said}");
    assert!(said.contains(".vscode/tasks.json"), "{said}");
    assert!(said.contains("merge"), "{said}");
    assert_eq!(out(&piece, &["rev-parse", "HEAD"]), before);
    assert!(!piece.join(".vscode").exists());
}

/// Whether git has a merge stopped in `dir`.
fn merging(dir: &Path) -> bool {
    dir.join(out(dir, &["rev-parse", "--git-path", "MERGE_HEAD"]))
        .exists()
}

/// A sandboxed chat's stopped merge, in a piece that tracks a file the sandbox denies, and the
/// reach it runs with.
fn a_stopped_merge_beside_a_denied_file(w: &World, name: &str) -> (PathBuf, PathBuf) {
    they_had_already(w, &[(".vscode/tasks.json", b"{}\n")]);
    let piece = a_piece_whose_pictures_conflict(w, name);
    let (code, said) = update(w, name, &merge(None), &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(merging(&piece));
    (piece, w.plane.join("workspaces").join(WS))
}

#[test]
fn an_abort_for_a_sandboxed_chat_that_would_write_a_denied_file_does_not_run() {
    purlis_core::unsteered!();
    let w = world();
    let (piece, chat) = a_stopped_merge_beside_a_denied_file(&w, "p1");
    put(&piece, ".vscode/tasks.json", b"staged\n");
    git(&piece, &["add", "--", ".vscode/tasks.json"]);
    let reach = Reach {
        hosts: None,
        sandboxed_at: Some(&chat),
        for_a_chat: true,
    };

    let (code, said) = update(&w, "p1", &UpdateStep::Abort, &reach);

    assert_eq!(code, 1, "{said}");
    assert!(said.contains(".vscode/tasks.json"), "{said}");
    assert_eq!(
        std::fs::read(piece.join(".vscode/tasks.json")).unwrap(),
        b"staged\n"
    );
    assert!(merging(&piece));
}

#[test]
fn an_abort_for_a_sandboxed_chat_that_writes_only_what_it_may_goes_back() {
    purlis_core::unsteered!();
    let w = world();
    let (piece, chat) = a_stopped_merge_beside_a_denied_file(&w, "p1");
    let head = out(&piece, &["rev-parse", "HEAD"]);
    let reach = Reach {
        hosts: None,
        sandboxed_at: Some(&chat),
        for_a_chat: true,
    };

    let (code, said) = update(&w, "p1", &UpdateStep::Abort, &reach);

    assert_eq!(code, 0, "{said}");
    assert_eq!(out(&piece, &["rev-parse", "HEAD"]), head);
    assert!(!merging(&piece));
}

#[test]
fn a_merge_refuses_uncommitted_work_and_a_file_it_would_write_over() {
    purlis_core::unsteered!();
    let w = world();
    let piece = pushed_piece(&w, "p1");
    let before = out(&piece, &["rev-parse", "HEAD"]);
    w.they_commit("incoming.txt", "new\n");

    write(&piece, "mine.txt", "changed, not committed\n");
    let (code, said) = update(&w, "p1", &merge(None), &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("uncommitted changes"), "{said}");
    git(&piece, &["checkout", "-q", "--", "mine.txt"]);

    write(&piece, "incoming.txt", "someone's own\n");
    let (code, said) = update(&w, "p1", &merge(None), &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("incoming.txt"), "{said}");
    assert_eq!(
        std::fs::read_to_string(piece.join("incoming.txt")).unwrap(),
        "someone's own\n"
    );
    assert_eq!(out(&piece, &["rev-parse", "HEAD"]), before);
}

#[test]
fn a_merge_of_a_named_base_records_it_as_a_rebase_does() {
    purlis_core::unsteered!();
    let w = world();
    git(&w.theirs, &["checkout", "-q", "-b", "release"]);
    commit_in(&w.theirs, "release.txt", "r\n");
    git(&w.theirs, &["push", "-q", "origin", "HEAD:release"]);
    git(&w.theirs, &["checkout", "-q", "main"]);
    git(&w.clone, &["fetch", "-q", "origin"]);
    git(&w.clone, &["branch", "-q", "release", "origin/release"]);
    let piece = pushed_piece(&w, "p1");

    let (code, said) = update(&w, "p1", &merge(Some("release")), &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert!(piece.join("release.txt").exists());
    assert_eq!(
        out(&w.clone, &["config", "--get", "branch.p1.charterBase"]),
        "release"
    );
}

#[test]
fn a_merge_the_person_started_is_theirs_to_continue_or_abort() {
    purlis_core::unsteered!();
    let w = world();
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    commit_in(&piece, "shared.txt", "a\nmine\nc\n");
    w.they_commit("shared.txt", "a\ntheirs\nc\n");
    git(&piece, &["fetch", "-q", "origin"]);
    let mut person = support::unsigned();
    person
        .args(["-c", "user.name=t", "-c", "user.email=t@e.invalid", "-C"])
        .arg(&piece)
        .args(["merge", "--no-edit", "origin/main"])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null");
    let theirs = purlis_core::forklock::output(&mut person).expect("git runs");
    assert!(!theirs.status.success(), "the person's merge stops");
    let git_dir = PathBuf::from(out(&piece, &["rev-parse", "--absolute-git-dir"]));

    for step in [UpdateStep::Abort, UpdateStep::Continue] {
        let (code, said) = update(&w, "p1", &step, &Reach::default());
        assert_eq!(code, 1, "{said}");
        assert!(said.contains("not one purlis started"), "{said}");
        assert!(git_dir.join("MERGE_HEAD").exists(), "{step:?} touched it");
    }
}

#[test]
fn a_merge_is_not_for_a_clone() {
    purlis_core::unsteered!();
    let w = world();
    let (code, said) = update_clone(&w, &merge(None), &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("is for a branch folder"), "{said}");
}

#[test]
fn a_merge_after_an_earlier_update_rewrote_the_branch_still_pushes_over_its_copy() {
    purlis_core::unsteered!();
    // #3895 stands: a rewrite the app made, then a merge on top, still goes over what it saw.
    let w = world();
    let piece = pushed_piece(&w, "p1");
    w.they_commit("first.txt", "1\n");
    let (code, said) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 0, "{said}");
    w.they_commit("second.txt", "2\n");
    let (code, said) = update(&w, "p1", &merge(None), &Reach::default());
    assert_eq!(code, 0, "{said}");
    let (code, said) = push(&w, "p1");
    assert_eq!(code, 0, "{said}");
    assert_eq!(forge_has(&w, "p1"), out(&piece, &["rev-parse", "HEAD"]));
}
