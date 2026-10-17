// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_folder_is_walked_only_while_its_longest_entry_still_fits_the_path_limit() {
    // A 20-byte root, `/` and a folder: the folder, `/`, a 255-byte name and the zero.
    let fits = 1024 - 20 - 1 - (1 + 255 + 1);
    assert!(!too_deep(20, fits, 1024));
    assert!(too_deep(20, fits + 1, 1024));
    assert!(!too_deep(20, 0, 1024));
    // Nothing wraps past the limit, however long a path is.
    assert!(too_deep(usize::MAX, usize::MAX, usize::MAX - 1));
}

/// A chain of folders under `at/top`, each named by 200 bytes, longer in all than
/// `longer_than` bytes, with a file at its bottom. Built by renames, so no call is handed a
/// path longer than a few hundred bytes.
fn deep_chain(at: &Path, longer_than: usize) {
    let top = at.join("top");
    std::fs::create_dir(&top).unwrap();
    std::fs::write(top.join("bottom.txt"), "at the bottom\n").unwrap();
    let (mut length, mut n) = (0, 0);
    while length <= longer_than {
        let wrapper = at.join("wrapper");
        std::fs::create_dir(&wrapper).unwrap();
        let name = format!("{n:03}{}", "d".repeat(197));
        std::fs::rename(&top, wrapper.join(&name)).unwrap();
        std::fs::rename(&wrapper, &top).unwrap();
        length += name.len() + 1;
        n += 1;
    }
}

/// #1130: the walk for untracked files used to end at the first folder too long to name,
/// and the whole status with it. In a repository with a separate git directory, as a
/// branch's folder is.
#[test]
fn a_folder_chain_past_the_path_limit_is_one_untracked_entry_and_the_walk_goes_on() {
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let work = base.join("work");
    let git_dir = base.join("work.git");
    let made = crate::testgit::run(
        &base,
        &[
            "init",
            "-q",
            "-b",
            "main",
            "--separate-git-dir",
            &git_dir.display().to_string(),
            &work.display().to_string(),
        ],
    );
    assert!(made.ok(), "{}", made.err);
    std::fs::write(work.join("tracked.txt"), "one\n").unwrap();
    assert!(crate::testgit::run(&work, &["add", "tracked.txt"]).ok());
    assert!(
        crate::testgit::run(
            &work,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.invalid",
                "commit",
                "-q",
                "-m",
                "one",
            ]
        )
        .ok()
    );
    // Past Linux's 4,096 bytes, and so past macOS's 1,024 too.
    deep_chain(&work, 4096 + work.as_os_str().len());
    // Walked after `top`: the walk has to come back from the chain to find it.
    std::fs::write(work.join("zz-new.txt"), "new\n").unwrap();
    std::fs::write(work.join("tracked.txt"), "two\n").unwrap();

    let repo = gix::open(&work).unwrap();
    let (mut loose, mut untracked, mut gone) = Default::default();
    uncommitted(&repo, &mut loose, &mut untracked, &mut gone)
        .expect("the status degrades rather than fails");

    assert_eq!(loose, BTreeSet::from(["tracked.txt".to_string()]));
    assert!(gone.is_empty(), "{gone:?}");
    assert!(untracked.contains("zz-new.txt"), "{untracked:?}");
    let in_chain: Vec<&String> = untracked
        .iter()
        .filter(|path| path.starts_with("top/"))
        .collect();
    assert_eq!(in_chain.len(), 1, "{untracked:?}");
    assert!(!in_chain[0].ends_with("bottom.txt"), "{untracked:?}");
    // The folder named is one the walk could still have read, and the first it could not
    // enter: its parent fits the limit with room for a name, and it does not.
    let root_len = work.as_os_str().len();
    assert!(too_deep(root_len, in_chain[0].len(), PATH_MOST));
    let parent = in_chain[0].rsplit_once('/').unwrap().0;
    assert!(!too_deep(root_len, parent.len(), PATH_MOST));
    assert_eq!(untracked.len(), 2, "{untracked:?}");
}

/// #1130 (train 20's review): an untracked folder the walk cannot open — here one with no
/// permissions at all — costs only itself. It is one untracked entry, as a too-deep folder
/// is, and the untracked files after it are still found. Skipped as root, whom a mode-000
/// folder does not stop.
#[cfg(unix)]
#[test]
fn an_untracked_folder_that_cannot_be_opened_is_one_entry_and_the_walk_goes_on() {
    use std::os::unix::fs::PermissionsExt as _;
    if rustix::process::geteuid().is_root() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let work = base.join("work");
    let made = crate::testgit::run(
        &base,
        &["init", "-q", "-b", "main", &work.display().to_string()],
    );
    assert!(made.ok(), "{}", made.err);
    std::fs::write(work.join("tracked.txt"), "one\n").unwrap();
    assert!(crate::testgit::run(&work, &["add", "tracked.txt"]).ok());
    assert!(
        crate::testgit::run(
            &work,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.invalid",
                "commit",
                "-q",
                "-m",
                "one",
            ]
        )
        .ok()
    );
    let shut = work.join("shut");
    std::fs::create_dir(&shut).unwrap();
    std::fs::write(shut.join("inside.txt"), "unseen\n").unwrap();
    // Walked after `shut`: the walk has to go on past it to find it.
    std::fs::write(work.join("zz-new.txt"), "new\n").unwrap();
    std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o000)).unwrap();

    let repo = gix::open(&work).unwrap();
    let (mut loose, mut untracked, mut gone) = Default::default();
    let read = uncommitted(&repo, &mut loose, &mut untracked, &mut gone);
    // Opened again before anything is asserted, so the temp folder can be removed.
    std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o755)).unwrap();

    read.expect("the status degrades rather than fails");
    assert!(loose.is_empty(), "{loose:?}");
    assert!(gone.is_empty(), "{gone:?}");
    assert_eq!(
        untracked,
        BTreeSet::from(["shut".to_string(), "zz-new.txt".to_string()])
    );
}

/// #1130 (the race left by train 20's review): an untracked folder that goes between the
/// walk's check that it opens and gix's own read of it ended the walk, and the status with
/// it. The walk is begun again, and the files after it are still found.
#[test]
fn an_untracked_folder_that_goes_mid_walk_costs_only_itself() {
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let work = base.join("work");
    let made = crate::testgit::run(
        &base,
        &[
            "init",
            "-q",
            "-b",
            "main",
            "--separate-git-dir",
            &base.join("work.git").display().to_string(),
            &work.display().to_string(),
        ],
    );
    assert!(made.ok(), "{}", made.err);
    let gone = work.join("gone");
    let fill = || {
        std::fs::create_dir_all(gone.join("deeper")).unwrap();
        std::fs::write(gone.join("deeper/inside.txt"), "soon gone\n").unwrap();
    };
    fill();
    // Walked after `gone`: the walk has to go on past it to find it.
    std::fs::write(work.join("zz-new.txt"), "new\n").unwrap();
    let repo = gix::open(&work).unwrap();
    // Removed once, the first time the walk is let into it.
    let removing_once = || {
        let gone = gone.clone();
        let mut removed = false;
        move |folder: &Path| {
            if folder == gone && !removed {
                std::fs::remove_dir_all(folder).unwrap();
                removed = true;
            }
        }
    };

    // One walk alone fails on it: the race is real.
    assert!(untracked_walk(&repo, &mut removing_once()).is_err());
    fill();

    let found = untracked_files_entering(&repo, &mut removing_once())
        .expect("the walk is begun again rather than failing");

    assert_eq!(found, BTreeSet::from(["zz-new.txt".to_string()]));
}

/// The guard holds back only untracked folders. A tracked folder close enough to the limit
/// that a longest name would not fit is still walked, so a short untracked file in it is
/// still found: holding the folder back would have hidden it, with no mark in its place.
#[test]
fn a_tracked_folder_near_the_path_limit_is_still_walked_for_untracked_files() {
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let work = base.join("work");
    let git_dir = base.join("work.git");
    let made = crate::testgit::run(
        &base,
        &[
            "init",
            "-q",
            "-b",
            "main",
            "--separate-git-dir",
            &git_dir.display().to_string(),
            &work.display().to_string(),
        ],
    );
    assert!(made.ok(), "{}", made.err);
    let root_len = work.as_os_str().len();
    // A folder path `rela` bytes long: past the guard's line, and still far inside the
    // limit for a short name.
    let rela = PATH_MOST - root_len - 200;
    let mut folder = String::new();
    let mut n = 0;
    while folder.len() < rela {
        if !folder.is_empty() {
            folder.push('/');
        }
        let room = (rela - folder.len()).min(200);
        folder.push_str(&format!(
            "{n:03}{}",
            "d".repeat(room.saturating_sub(3).max(1))
        ));
        n += 1;
    }
    assert!(too_deep(root_len, folder.len(), PATH_MOST));
    std::fs::create_dir_all(work.join(&folder)).unwrap();
    std::fs::write(work.join(&folder).join("kept.txt"), "kept\n").unwrap();
    let tracked = format!("{folder}/kept.txt");
    assert!(crate::testgit::run(&work, &["add", "--", &tracked]).ok());
    assert!(
        crate::testgit::run(
            &work,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.invalid",
                "commit",
                "-q",
                "-m",
                "deep",
            ]
        )
        .ok()
    );
    std::fs::write(work.join(&folder).join("new.txt"), "new\n").unwrap();

    let repo = gix::open(&work).unwrap();
    let (mut loose, mut untracked, mut gone) = Default::default();
    uncommitted(&repo, &mut loose, &mut untracked, &mut gone).expect("read");

    assert!(loose.is_empty(), "{loose:?}");
    assert!(gone.is_empty(), "{gone:?}");
    assert_eq!(untracked, BTreeSet::from([format!("{folder}/new.txt")]));
}

#[test]
fn a_line_that_gains_its_line_end_is_a_changed_line_as_git_shows_it() {
    let (hunks, added, removed) = line_diff(b"a\nb", b"a\nb\nc\n");
    assert_eq!((added, removed), (2, 1));
    assert_eq!(
        hunks,
        [Hunk {
            old_start: 2,
            old_lines: 1,
            new_start: 2,
            new_lines: 2
        }]
    );
}

/// git's own answer, `git diff --no-index -U0` with its default `diff.indentHeuristic`: the
/// repeated block is placed after the first `b`, not after the second `a` as Myers alone
/// places it (`-c diff.indentHeuristic=false` answers `@@ -5,0 +6,3 @@`).
#[test]
fn an_ambiguous_hunk_sits_where_gits_indent_heuristic_puts_it() {
    let (hunks, ..) = line_diff(b"1\n2\na\n\nb\n3\n4\n", b"1\n2\na\n\nb\na\n\nb\n3\n4\n");
    assert_eq!(
        hunks,
        [Hunk {
            old_start: 4,
            old_lines: 0,
            new_start: 5,
            new_lines: 3
        }]
    );
}

#[test]
fn a_pure_insertion_names_the_line_it_comes_after() {
    let (hunks, ..) = line_diff(b"a\nb\n", b"top\na\nb\n");
    assert_eq!(
        hunks,
        [Hunk {
            old_start: 0,
            old_lines: 0,
            new_start: 1,
            new_lines: 1
        }]
    );
}

/// A repository at `base/work` with its git directory beside it, as a branch's folder is.
fn work_repo(base: &Path) -> PathBuf {
    let work = base.join("work");
    let made = crate::testgit::run(
        base,
        &[
            "init",
            "-q",
            "-b",
            "main",
            "--separate-git-dir",
            &base.join("work.git").display().to_string(),
            &work.display().to_string(),
        ],
    );
    assert!(made.ok(), "{}", made.err);
    work
}

fn git_ok(dir: &Path, args: &[&str]) -> String {
    let mut all = vec![
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@example.invalid",
        "-c",
        "commit.gpgsign=false",
    ];
    all.extend_from_slice(args);
    let ran = crate::testgit::run(dir, &all);
    assert!(ran.ok(), "git {args:?}: {}", ran.err);
    ran.out.trim().to_string()
}

fn opened(work: &Path) -> gix::Repository {
    gix::open_opts(work, gix::open::Options::isolated()).unwrap()
}

/// The attribute files on the way to `paths` in the repository at `work`, read through its
/// folder held from `base`.
fn attributes_at(base: &Path, work: &Path, paths: &[&str]) -> HeldAttributes {
    let held = hold(base, work).expect("the folder is held");
    HeldAttributes::read(&mut opened(work), Some(&held), paths.iter().copied())
}

/// Train 80 XG: the attribute files on the way to a path are read through the folder held
/// open, following no link. A `.gitattributes` that is a link, or one in a folder swapped
/// for a link, says nothing, and the one in a plain folder beside them still says what it
/// says: for binary, and for line ends.
#[cfg(unix)]
#[test]
fn attribute_files_are_read_through_the_held_folder_following_no_link() {
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let work = work_repo(&base);
    let outside = base.join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join(".gitattributes"), "* -diff text eol=crlf\n").unwrap();
    std::os::unix::fs::symlink(outside.join(".gitattributes"), work.join(".gitattributes"))
        .unwrap();
    std::os::unix::fs::symlink(&outside, work.join("linked")).unwrap();
    std::fs::create_dir(work.join("plain")).unwrap();
    std::fs::write(
        work.join("plain/.gitattributes"),
        "*.txt -diff\n*.crlf text eol=crlf\n",
    )
    .unwrap();
    let paths = ["top.txt", "linked/a.txt", "plain/a.txt", "plain/a.crlf"];
    let attributes = attributes_at(&base, &work, &paths);
    let mut said = DiffAttribute::of(&attributes);
    let text = (Side::Bytes(b"one\n".to_vec()), Kind::File);
    assert!(
        !said.binary(&text, "top.txt"),
        "a linked .gitattributes was read"
    );
    assert!(
        !said.binary(&text, "linked/a.txt"),
        "a linked folder was read"
    );
    assert!(said.binary(&text, "plain/a.txt"));
    let mut to_git = ToGit::of(&attributes).expect("a pipeline");
    assert_eq!(to_git.clean("top.txt", b"one\r\n".to_vec()), b"one\r\n");
    assert_eq!(
        to_git.clean("linked/a.txt", b"one\r\n".to_vec()),
        b"one\r\n"
    );
    assert_eq!(to_git.clean("plain/a.crlf", b"one\r\n".to_vec()), b"one\n");
}

/// A superproject at `base/work` whose `.gitmodules` (in its index and HEAD) names the
/// submodule `name` at `sub`, and whose index points `sub` at `one`.
fn superproject(base: &Path, name: &str, one: &str) -> PathBuf {
    let work = work_repo(base);
    std::fs::write(
        work.join(".gitmodules"),
        format!("[submodule \"{name}\"]\n\tpath = sub\n\turl = ./elsewhere\n"),
    )
    .unwrap();
    git_ok(&work, &["add", ".gitmodules"]);
    git_ok(
        &work,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{one},sub"),
        ],
    );
    git_ok(&work, &["commit", "-q", "-m", "a submodule"]);
    std::fs::create_dir_all(work.join("sub")).unwrap();
    work
}

fn write_at(at: &Path, text: &str) {
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(at, text).unwrap();
}

fn id(hex: &str) -> gix::ObjectId {
    gix::ObjectId::from_hex(hex.as_bytes()).unwrap()
}

/// #704: the commit a submodule has checked out is read from its git folder under the
/// superproject's (`modules/<name>`), else from its own `.git` folder: HEAD, then its loose
/// ref, then `packed-refs`, each as a plain file.
#[test]
fn a_submodules_checkout_is_read_from_its_git_folder_as_plain_files() {
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let (one, two, three) = ("1".repeat(40), "2".repeat(40), "3".repeat(40));
    let work = superproject(&base, "sub", &one);
    let repo = opened(&work);
    let held = hold(&base, &work).unwrap();
    let own = work.join("sub/.git");
    assert_eq!(
        checked_out(&repo, &held, "sub"),
        None,
        "nothing checked out"
    );

    write_at(&own.join("HEAD"), "ref: refs/heads/main\n");
    write_at(&own.join("refs/heads/main"), &format!("{two}\n"));
    assert_eq!(checked_out(&repo, &held, "sub"), Some(id(&two)));

    std::fs::remove_file(own.join("refs/heads/main")).unwrap();
    write_at(
        &own.join("packed-refs"),
        &format!(
            "# pack-refs with: peeled fully-peeled sorted\n{one} refs/heads/other\n{three} refs/heads/main\n^{one}\n"
        ),
    );
    assert_eq!(checked_out(&repo, &held, "sub"), Some(id(&three)));

    write_at(&own.join("HEAD"), &format!("{two}\n"));
    assert_eq!(checked_out(&repo, &held, "sub"), Some(id(&two)), "detached");

    // The git folder under the superproject's is read first, as git keeps a submodule's.
    let modules = base.join("work.git/modules/sub");
    write_at(&modules.join("HEAD"), &format!("{three}\n"));
    std::fs::remove_dir_all(&own).unwrap();
    write_at(&own, "gitdir: ../../work.git/modules/sub\n");
    assert_eq!(checked_out(&repo, &held, "sub"), Some(id(&three)));
}

/// #704: a submodule whose folder checked out another commit than the index points at is
/// listed as changed and not committed, its working tree's side the commit it checked out,
/// as `git add -A` would stage it; the explorer's markers still leave it out.
#[test]
fn a_submodule_checked_out_at_another_commit_is_a_change_not_committed() {
    let (one, two) = ("1".repeat(40), "2".repeat(40));
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let work = superproject(&base, "sub", &one);
    write_at(&work.join("sub/.git/HEAD"), &format!("{two}\n"));
    let repo = opened(&work);
    let held = hold(&base, &work).unwrap();
    let since = repo.head_commit().unwrap().id;

    let listed = changed_with(&repo, Some(since), At::WorkingTree, Some(&held), true).unwrap();
    assert_eq!(
        listed
            .changes
            .iter()
            .map(|c| (c.path.as_str(), c.mark, c.uncommitted))
            .collect::<Vec<_>>(),
        [("sub", Mark::Changed, true)]
    );
    let head = listed.repo.find_tree(listed.head_tree).unwrap();
    let (side, kind) = side_at(&listed.repo, Some(&head), "sub", COUNTED);
    assert_eq!(kind, Kind::Submodule);
    assert_eq!(
        side.bytes(),
        Some(format!("Subproject commit {two}\n").as_bytes())
    );
    assert!(
        changed(&repo, Some(since), At::WorkingTree, Some(&held))
            .unwrap()
            .changes
            .is_empty()
    );
}

/// #704's confinement rule: a submodule's `.git` file is never followed wherever its
/// `gitdir:` line points, nor a `.git` folder or a submodule folder that is a link, nor a
/// name that walks out of `modules/`, nor a ref that walks out of the git folder.
#[cfg(unix)]
#[test]
fn a_submodules_checkout_is_never_read_through_a_link_a_gitdir_line_or_a_walk_up() {
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let (one, two) = ("1".repeat(40), "2".repeat(40));
    let outside = base.join("outside");
    write_at(&outside.join("HEAD"), &format!("{two}\n"));
    write_at(&outside.join("refs/heads/main"), &format!("{two}\n"));

    let work = superproject(&base, "sub", &one);
    let repo = opened(&work);
    let held = hold(&base, &work).unwrap();
    let own = work.join("sub/.git");
    write_at(&own, &format!("gitdir: {}\n", outside.display()));
    assert_eq!(
        checked_out(&repo, &held, "sub"),
        None,
        "a gitdir: line was followed"
    );

    std::fs::remove_file(&own).unwrap();
    std::os::unix::fs::symlink(&outside, &own).unwrap();
    assert_eq!(
        checked_out(&repo, &held, "sub"),
        None,
        "a linked .git was followed"
    );

    std::fs::remove_file(&own).unwrap();
    std::fs::remove_dir(work.join("sub")).unwrap();
    let elsewhere = base.join("elsewhere");
    write_at(&elsewhere.join(".git/HEAD"), &format!("{two}\n"));
    std::os::unix::fs::symlink(&elsewhere, work.join("sub")).unwrap();
    assert_eq!(
        checked_out(&repo, &held, "sub"),
        None,
        "a linked folder was followed"
    );

    std::fs::remove_file(work.join("sub")).unwrap();
    std::fs::create_dir(work.join("sub")).unwrap();
    write_at(
        &own.join("HEAD"),
        "ref: refs/../../../outside/refs/heads/main\n",
    );
    assert_eq!(checked_out(&repo, &held, "sub"), None, "a ref walked up");

    // A name that walks out of `modules/` is never read, whatever is there.
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let work = superproject(&base, "../planted", &one);
    write_at(&base.join("work.git/planted/HEAD"), &format!("{two}\n"));
    let repo = opened(&work);
    let held = hold(&base, &work).unwrap();
    assert_eq!(checked_out(&repo, &held, "sub"), None, "a name walked up");
}

/// #704: git draws a type change as the old kind out and the new one in, two hunks, and
/// counts every line of both, even a line the two sides share.
#[test]
fn a_type_change_is_two_hunks_and_both_whole_sides() {
    let (hunks, added, removed) = lines_between(
        b"target.txt",
        Kind::File,
        b"target.txt",
        Kind::Link,
        Whitespace::Shown,
    );
    assert_eq!((added, removed), (1, 1));
    assert_eq!(
        hunks,
        [
            Hunk {
                old_start: 1,
                old_lines: 1,
                new_start: 0,
                new_lines: 0
            },
            Hunk {
                old_start: 0,
                old_lines: 0,
                new_start: 1,
                new_lines: 1
            }
        ]
    );
    // A side with no lines has no hunk; the same kind is a line diff as before.
    let (hunks, ..) = lines_between(b"", Kind::File, b"a\n", Kind::Link, Whitespace::Shown);
    assert_eq!(hunks.len(), 1);
    assert_eq!(
        lines_between(
            b"a\nb\n",
            Kind::File,
            b"a\nc\n",
            Kind::File,
            Whitespace::Shown
        ),
        line_diff(b"a\nb\n", b"a\nc\n")
    );
}

/// #704: the `diff` attribute decides binary before the bytes, read from the working tree;
/// a driver's name leaves it to the bytes.
#[test]
fn the_diff_attribute_decides_binary_before_the_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let work = work_repo(&base);
    std::fs::write(
        work.join(".gitattributes"),
        "unset.txt -diff\nmacro.txt binary\nnamed.txt diff=false\nforced.dat diff\n",
    )
    .unwrap();
    let attributes = attributes_at(&base, &work, &["unset.txt", "macro.txt"]);
    let mut said = DiffAttribute::of(&attributes);
    let text = (Side::Bytes(b"one\n".to_vec()), Kind::File);
    let nul = (Side::Bytes(b"a\0b\n".to_vec()), Kind::File);
    assert!(said.binary(&text, "unset.txt"));
    assert!(said.binary(&text, "macro.txt"));
    assert!(!said.binary(&text, "named.txt"));
    assert!(!said.binary(&nul, "forced.dat"));
    assert!(said.binary(&nul, "sniffed.dat"));
    assert!(!said.binary(&text, "plain.txt"));
    // A submodule's pointer never is, whatever is said of its path.
    let sub = (
        pointer(gix::ObjectId::empty_tree(gix::hash::Kind::Sha1)),
        Kind::Submodule,
    );
    assert!(!said.binary(&sub, "unset.txt"));
}

/// #704: a named driver's `diff.<driver>.binary` decides binary as git reads it: `true`
/// is binary whatever the bytes, `false` is text whatever the bytes, and a driver the config
/// does not say it of, or says it of in no boolean, leaves it to the bytes.
#[test]
fn a_named_drivers_binary_setting_decides_binary_before_the_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let work = work_repo(&base);
    git_ok(&work, &["config", "diff.blob.binary", "true"]);
    git_ok(&work, &["config", "diff.Prose.binary", "false"]);
    git_ok(&work, &["config", "diff.odd.binary", "maybe"]);
    std::fs::write(
        work.join(".gitattributes"),
        "*.blob diff=blob\n*.prose diff=Prose\n*.lower diff=prose\n*.odd diff=odd\n\
             *.none diff=none\n",
    )
    .unwrap();
    let attributes = attributes_at(&base, &work, &["a.blob"]);
    let mut said = DiffAttribute::of(&attributes);
    let text = (Side::Bytes(b"one\n".to_vec()), Kind::File);
    let nul = (Side::Bytes(b"a\0b\n".to_vec()), Kind::File);
    assert!(said.binary(&text, "a.blob"));
    assert!(!said.binary(&nul, "a.prose"));
    // A driver's name is matched as git matches it, case and all.
    assert!(said.binary(&nul, "a.lower"));
    assert!(!said.binary(&text, "a.lower"));
    assert!(said.binary(&nul, "a.odd"));
    assert!(!said.binary(&text, "a.odd"));
    assert!(said.binary(&nul, "a.none"));
    assert!(!said.binary(&text, "a.none"));
}

/// #704: a working-tree file is compared as `git add` would store it, its line ends
/// converted as the attributes say; one that says nothing is left as it is.
#[test]
fn a_working_tree_file_is_cleaned_as_git_add_stores_it() {
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let work = work_repo(&base);
    std::fs::write(work.join(".gitattributes"), "*.txt text eol=crlf\n").unwrap();
    let attributes = attributes_at(&base, &work, &["a.txt"]);
    let mut to_git = ToGit::of(&attributes).expect("a pipeline");
    assert_eq!(
        to_git.clean("a.txt", b"one\r\ntwo\r\n".to_vec()),
        b"one\ntwo\n"
    );
    assert_eq!(to_git.clean("a.bin", b"one\r\n".to_vec()), b"one\r\n");
}

/// #704: a filter the config names is never run: the reader keeps no driver.
#[test]
fn a_filter_driver_the_repo_names_is_never_run() {
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let work = work_repo(&base);
    let ran = base.join("ran");
    git_ok(
        &work,
        &[
            "config",
            "filter.planted.clean",
            &format!("touch {}", ran.display()),
        ],
    );
    std::fs::write(work.join(".gitattributes"), "* filter=planted\n").unwrap();
    let attributes = attributes_at(&base, &work, &["a.txt"]);
    let mut to_git = ToGit::of(&attributes).expect("a pipeline");
    assert_eq!(to_git.clean("a.txt", b"one\n".to_vec()), b"one\n");
    assert!(!ran.exists());
}

/// #704: a submodule's pointer is listed only when a comparison asks for it, never for the
/// explorer's markers.
#[test]
fn a_submodules_pointer_is_listed_only_when_asked_for() {
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let work = work_repo(&base);
    std::fs::write(work.join("a.txt"), "a\n").unwrap();
    git_ok(&work, &["add", "a.txt"]);
    git_ok(&work, &["commit", "-q", "-m", "one"]);
    let one = git_ok(&work, &["rev-parse", "HEAD"]);
    git_ok(
        &work,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{one},sub"),
        ],
    );
    git_ok(&work, &["commit", "-q", "-m", "sub"]);
    let two = git_ok(&work, &["rev-parse", "HEAD"]);
    let repo = opened(&work);
    let id = |hex: &str| gix::ObjectId::from_hex(hex.as_bytes()).unwrap();
    let tree = repo
        .find_commit(id(&two))
        .unwrap()
        .tree_id()
        .unwrap()
        .detach();
    let none = BTreeSet::new();
    let (asked, refused) = between_trees(&repo, Some(id(&one)), tree, &none, true).unwrap();
    assert!(refused.is_empty(), "{refused:?}");
    assert_eq!(
        asked
            .iter()
            .map(|c| (c.path.as_str(), c.mark))
            .collect::<Vec<_>>(),
        [("sub", Mark::Added)]
    );
    assert!(
        between_trees(&repo, Some(id(&one)), tree, &none, false)
            .unwrap()
            .0
            .is_empty()
    );
    let head = repo.find_tree(tree).unwrap();
    let (side, kind) = side_at(&repo, Some(&head), "sub", COUNTED);
    assert_eq!(kind, Kind::Submodule);
    assert_eq!(
        side.bytes(),
        Some(format!("Subproject commit {one}\n").as_bytes())
    );
}

/// A tree written into the repository at `work` as it is given, whatever its names say:
/// `entries` are `(mode, id, name)`, a mode of `040000` naming a tree. git's own `add`
/// refuses such names, a commit made elsewhere need not.
fn mktree_at(work: &Path, entries: &[(&str, &str, &str)]) -> String {
    use gix::objs::tree::{Entry, EntryKind};
    let repo = opened(work);
    let mut tree = gix::objs::Tree::empty();
    for (mode, hex, name) in entries {
        let kind = if *mode == "040000" {
            EntryKind::Tree
        } else {
            EntryKind::Blob
        };
        tree.entries.push(Entry {
            mode: kind.into(),
            filename: (*name).into(),
            oid: id(hex),
        });
    }
    tree.entries.sort();
    repo.write_object(&tree).unwrap().to_string()
}

/// #704: a path a commit holds under git's own folder's name, in any case, or under a name
/// that is not a plain path, is named as refused with its mark, never left out of the list
/// without a word, and nothing at it is read. A rename into or out of such a path is the
/// plain end's own change, and the other end is refused.
#[test]
fn a_path_through_gits_own_folder_is_refused_by_name_never_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let work = work_repo(&base);
    let blob = |text: &str| opened(&work).write_blob(text).unwrap().to_string();
    let (one, two, planted) = (blob("one\n"), blob("two\n"), blob("planted\n"));
    let (into, out_of) = (blob(&long_lines("into")), blob(&long_lines("out of")));
    let x = mktree_at(&work, &[("100644", &planted, "x")]);
    let into_tree = mktree_at(&work, &[("100644", &into, "moved.txt")]);
    let out_of_tree = mktree_at(&work, &[("100644", &out_of, "long.txt")]);
    let src = mktree_at(&work, &[("040000", &out_of_tree, ".git")]);
    let before = mktree_at(
        &work,
        &[
            ("100644", &into, "moved.txt"),
            ("100644", &one, "ok.txt"),
            ("040000", &src, "src"),
        ],
    );
    let vendor = mktree_at(&work, &[("040000", &x, ".GIT")]);
    let deep = mktree_at(&work, &[("040000", &into_tree, ".git")]);
    let after = mktree_at(
        &work,
        &[
            ("100644", &planted, "a\\b.txt"),
            ("040000", &deep, "deep"),
            ("100644", &two, "ok.txt"),
            ("100644", &out_of, "out.txt"),
            ("040000", &vendor, "vendor"),
        ],
    );
    let since = git_ok(&work, &["commit-tree", &before, "-m", "before"]);
    let repo = opened(&work);

    let (changes, refused) =
        between_trees(&repo, Some(id(&since)), id(&after), &BTreeSet::new(), true).unwrap();

    assert_eq!(
        changes
            .iter()
            .map(|c| (c.path.as_str(), c.mark, c.from.as_deref()))
            .collect::<Vec<_>>(),
        [
            ("moved.txt", Mark::Deleted, None),
            ("ok.txt", Mark::Changed, None),
            ("out.txt", Mark::Added, None),
        ]
    );
    use super::super::status::RefusedFor::{GitsOwnFolder, NotAPlainPath};
    assert_eq!(
        refused
            .iter()
            .map(|r| (r.path.as_str(), r.mark, r.why, r.uncommitted))
            .collect::<Vec<_>>(),
        [
            ("a\\b.txt", Mark::Added, NotAPlainPath, false),
            ("deep/.git/moved.txt", Mark::Added, GitsOwnFolder, false),
            ("src/.git/long.txt", Mark::Deleted, GitsOwnFolder, false),
            ("vendor/.GIT/x", Mark::Added, GitsOwnFolder, false),
        ]
    );
}

/// #704, the explorer's markers: a file the working tree holds under a name that is not a
/// plain path is named as refused, not committed, and is never read or staged.
#[cfg(unix)]
#[test]
fn an_untracked_path_that_is_not_plain_is_refused_by_name_in_the_markers() {
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let work = work_repo(&base);
    std::fs::write(work.join("ok.txt"), "one\n").unwrap();
    git_ok(&work, &["add", "ok.txt"]);
    git_ok(&work, &["commit", "-q", "-m", "one"]);
    std::fs::write(work.join("odd\\name.txt"), "never read\n").unwrap();
    std::fs::write(work.join("new.txt"), "new\n").unwrap();
    let repo = opened(&work);
    let held = hold(&base, &work).unwrap();
    let since = repo.head_commit().unwrap().id;

    for listed in [
        changed(&repo, Some(since), At::WorkingTree, Some(&held)).unwrap(),
        changed_with(&repo, Some(since), At::WorkingTree, Some(&held), true).unwrap(),
    ] {
        assert_eq!(
            listed
                .changes
                .iter()
                .map(|c| c.path.as_str())
                .collect::<Vec<_>>(),
            ["new.txt"]
        );
        assert_eq!(
            listed.refused,
            [RefusedChange {
                path: "odd\\name.txt".into(),
                mark: Mark::Added,
                why: super::super::status::RefusedFor::NotAPlainPath,
                uncommitted: true,
            }]
        );
        let head = listed.repo.find_tree(listed.head_tree).unwrap();
        assert!(
            head.lookup_entry_by_path("odd\\name.txt")
                .unwrap()
                .is_none(),
            "a refused path was staged"
        );
    }
}

fn long_lines(seed: &str) -> String {
    (0..40)
        .map(|n| format!("{seed} line {n} with enough words to be similar\n"))
        .collect()
}

/// git's own `-U0` hunks of `old` against `new`, and the lines they add and remove, with
/// `flags` (`-w`): `git diff --no-index`, which needs no repository, reading no config.
fn gits_hunks(old: &[u8], new: &[u8], flags: &[&str]) -> (Vec<Hunk>, u64, u64) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("old"), old).unwrap();
    std::fs::write(dir.path().join("new"), new).unwrap();
    let mut args = vec!["diff", "--no-index", "--no-ext-diff", "--no-color", "-U0"];
    args.extend_from_slice(flags);
    args.extend(["old", "new"]);
    let ran = crate::testgit::run_unconfigured(dir.path(), &args);
    assert!(ran.code == Some(0) || ran.code == Some(1), "{}", ran.err);
    let hunks: Vec<Hunk> = ran
        .out
        .lines()
        .filter_map(|line| line.strip_prefix("@@ -"))
        .map(|rest| {
            let numbers: Vec<&str> = rest.split(" @@").next().unwrap().split(' ').collect();
            let side = |s: &str| -> (u32, u32) {
                let s = s.trim_start_matches(['-', '+']);
                match s.split_once(',') {
                    Some((start, lines)) => (start.parse().unwrap(), lines.parse().unwrap()),
                    None => (s.parse().unwrap(), 1),
                }
            };
            let (old_start, old_lines) = side(numbers[0]);
            let (new_start, new_lines) = side(numbers[1]);
            Hunk {
                old_start,
                old_lines,
                new_start,
                new_lines,
            }
        })
        .collect();
    let added = hunks.iter().map(|h| u64::from(h.new_lines)).sum();
    let removed = hunks.iter().map(|h| u64::from(h.old_lines)).sum();
    (hunks, added, removed)
}

/// Hand-picked pairs for `-w`: each kind of whitespace git skips and the two it does not
/// (`\v`, `\f`), a last line gaining its line end, CRLF against LF, a file whose only change
/// is whitespace, blank lines, and real changes among whitespace ones.
const WHITESPACE_PAIRS: &[(&[u8], &[u8])] = &[
        (b"a b\nc\n", b"ab\nc  \n"),
        (b"\tif x {\n\t\ty\n\t}\n", b"    if x {\n        y\n    }\n"),
        (b"a\r\nb\r\nc\r\n", b"a\nb\nc\nd\n"),
        (b"a\nb", b"a\nb\n"),
        (b"a\nb", b"a\nb\nc"),
        (b"a\x0bb\nx\n", b"ab\nx\n"),
        (b"a\x0cb\nx\n", b"ab\nx\n"),
        (b"one\n\ntwo\n", b"one\ntwo\n"),
        (b"one\ntwo\n", b"one\n   \ntwo\n"),
        (b"  keep\nold\n  keep\n", b"keep\nnew\nkeep  \n"),
        (b"", b"  \n\t\n"),
        (b"x y z\n", b""),
        (
            b"fn a() {\n    one();\n}\n\nfn b() {\n    two();\n}\n",
            b"fn a() {\n  one();\n}\n\nfn c() {\n  three();\n}\n\nfn b() {\n  two();\n}\n",
        ),
        // Blocks alike, re-indented with tabs, one inserted among them: the integration
        // fixture's `src/lib.rs`, where the heuristic places the inserted block.
        (
            b"fn one() {\n    let x = 1;\n    let y = 2;\n    x + y\n}\n\nfn two() {\n    let x = 1;\n    let y = 2;\n    x + y\n}\n\nfn three() {\n    let x = 1;\n    let y = 2;\n    x + y\n}\n\n",
            b"fn one() {\n\tlet x = 1;\n\tlet y = 2;\n\tx + y\n}\n\nfn two() {\n\tlet x = 1;\n\tlet y = 2;\n\tx + y\n}\n\nfn inserted() {\n\tlet x = 1;\n\tlet y = 2;\n\tx + y\n}\n\nfn three() {\n\tlet x = 1;\n\tlet y = 2;\n\tx + y\n}\n\n",
        ),
    ];

/// R3 (#704): on the hand-picked pairs, hiding whitespace gives `git diff -w`'s hunks and
/// counts exactly.
#[test]
fn hidden_whitespace_is_gits_dash_w_on_the_hand_picked_pairs() {
    for (old, new) in WHITESPACE_PAIRS {
        assert_eq!(
            line_diff_as(old, new, Whitespace::Hidden),
            gits_hunks(old, new, &["-w"]),
            "-w: {:?} against {:?}",
            String::from_utf8_lossy(old),
            String::from_utf8_lossy(new),
        );
    }
}

/// A change of whitespace only, of any kind git skips, is no change once it is hidden, and
/// a change when it is shown.
#[test]
fn a_whitespace_only_change_has_no_hunk_once_whitespace_is_hidden() {
    let (old, new): (&[u8], &[u8]) = (b"fn a() {\r\n\tone( x );\r\n}", b"fn a() {\n  one(x);\n}\n");
    assert_eq!(line_diff_as(old, new, Whitespace::Hidden), (vec![], 0, 0));
    assert_eq!(line_diff_as(old, new, Whitespace::Shown).1, 3);
}

/// A file's lines, each with its line end, as the engine splits them.
fn lines_of(text: &[u8]) -> Vec<&[u8]> {
    text.split_inclusive(|byte| *byte == b'\n').collect()
}

/// Whether `hunks` are a diff of `old` and `new` in which every line they keep is the
/// same line on both sides once whitespace is taken out: in order, inside both files, and
/// leaving the same number of lines unchanged on each side.
fn keeps_only_lines_alike_without_whitespace(old: &[u8], new: &[u8], hunks: &[Hunk]) -> bool {
    let stripped = |line: &[u8]| -> Vec<u8> {
        line.iter()
            .copied()
            .filter(|b| !hidden_by_dash_w(*b))
            .collect()
    };
    let (old, new) = (lines_of(old), lines_of(new));
    // Each hunk as 0-based starts: `-U0` names the line before an empty side.
    let (mut i, mut j) = (0usize, 0usize);
    for hunk in hunks {
        let start = |at: u32, lines: u32| (at - u32::from(lines > 0)) as usize;
        let (oi, nj) = (
            start(hunk.old_start, hunk.old_lines),
            start(hunk.new_start, hunk.new_lines),
        );
        if oi < i || nj < j || oi - i != nj - j {
            return false;
        }
        while i < oi {
            if stripped(old[i]) != stripped(new[j]) {
                return false;
            }
            i += 1;
            j += 1;
        }
        i += hunk.old_lines as usize;
        j += hunk.new_lines as usize;
    }
    if i > old.len() || j > new.len() || old.len() - i != new.len() - j {
        return false;
    }
    old[i..]
        .iter()
        .zip(&new[j..])
        .all(|(a, b)| stripped(a) == stripped(b))
}

/// R3 (#704), on a few hundred generated pairs whose lines differ mostly in whitespace:
/// hiding whitespace counts what `git diff -w` counts, and every line it keeps is the same
/// line on both sides once whitespace is out. Where two equally short diffs exist, its hunks
/// may sit elsewhere than git's (ADR 0084's 2026-10-13 note), so only the counts and the
/// lines kept are held to git here, not each hunk's place.
#[test]
fn hidden_whitespace_counts_as_gits_dash_w_and_keeps_only_lines_alike() {
    // A fixed sequence, so a failure is the same failure on every run.
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = |below: u64| {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) % below
    };
    let words: [&[u8]; 6] = [b"a", b"b", b"(", b")", b"", b"c d"];
    let pads: [&[u8]; 5] = [b"", b" ", b"\t", b"   ", b" \t "];
    let mut file = |lines: u64| {
        let mut out = Vec::new();
        for _ in 0..lines {
            out.extend_from_slice(pads[next(5) as usize]);
            out.extend_from_slice(words[next(6) as usize]);
            out.extend_from_slice(pads[next(5) as usize]);
            out.extend_from_slice(if next(6) == 0 { b"\r\n" } else { b"\n" });
        }
        out
    };
    let mut pairs = Vec::new();
    for round in 0..300u64 {
        let most = if round % 10 == 0 { 60 } else { 14 };
        let (a, b) = (file(round % most), file((round * 7 + 3) % most));
        pairs.push((a, b));
    }
    for (old, new) in &pairs {
        let (hunks, added, removed) = line_diff_as(old, new, Whitespace::Hidden);
        let (_, git_added, git_removed) = gits_hunks(old, new, &["-w"]);
        let shown = || {
            format!(
                "{:?} against {:?}",
                String::from_utf8_lossy(old),
                String::from_utf8_lossy(new)
            )
        };
        assert_eq!((added, removed), (git_added, git_removed), "{}", shown());
        assert!(
            keeps_only_lines_alike_without_whitespace(old, new, &hunks),
            "{hunks:?}: {}",
            shown()
        );
        let summed = |side: fn(&Hunk) -> u32| hunks.iter().map(|h| u64::from(side(h))).sum();
        assert_eq!(
            (summed(|h| h.new_lines), summed(|h| h.old_lines)),
            (added, removed),
            "{}",
            shown()
        );
    }
}

/// Whitespace shown is the plain line diff: `git diff` with no flag, on the hand-picked
/// pairs.
#[test]
fn shown_whitespace_is_gits_plain_diff() {
    for (old, new) in WHITESPACE_PAIRS {
        assert_eq!(
            line_diff_as(old, new, Whitespace::Shown),
            gits_hunks(old, new, &[]),
        );
    }
}

/// A type change hides no whitespace: both whole sides, as git draws it.
#[test]
fn a_type_change_with_whitespace_hidden_is_still_both_whole_sides() {
    assert_eq!(
        lines_between(b"a \n", Kind::File, b"a\n", Kind::Link, Whitespace::Hidden).0,
        lines_between(b"a \n", Kind::File, b"a\n", Kind::Link, Whitespace::Shown).0,
    );
}

#[test]
fn members_come_after_the_members_they_need_and_otherwise_in_the_records_order() {
    use crate::change::Member;
    let member = |repo: &str, needs: &[&str]| Member {
        repo: repo.to_string(),
        branch: "change/x".to_string(),
        needs: needs.iter().map(|n| n.to_string()).collect(),
    };
    let members = [
        member("app", &["api", "schema"]),
        member("api", &["schema"]),
        member("docs", &[]),
        member("schema", &[]),
    ];
    let order: Vec<&str> = needs_order(&members)
        .iter()
        .map(|m| m.repo.as_str())
        .collect();
    assert_eq!(order, ["docs", "schema", "api", "app"]);
}
