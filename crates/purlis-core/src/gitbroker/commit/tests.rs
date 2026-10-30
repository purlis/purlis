// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn paths(of: &[&str]) -> Vec<String> {
    of.iter().map(|path| (*path).to_owned()).collect()
}

fn change(was: &str, mode: &str, status: char, path: &str) -> Staged {
    Staged {
        was: was.to_owned(),
        mode: mode.to_owned(),
        status,
        path: path.to_owned(),
    }
}

#[test]
fn a_path_is_a_name_inside_the_chats_folder_or_it_is_refused() {
    for fine in [
        vec!["a.txt"],
        vec!["src/lib.rs", "./docs/a b.md"],
        // Named like options: names, because git reads paths as data.
        vec!["-n", "--amend", "--", ":(top)x", "*.rs"],
        vec![".gitignore", ".gitattributes", "git", "a.git"],
    ] {
        assert_eq!(paths_ok(&paths(&fine)), Ok(()), "{fine:?}");
    }
    for (refused, why) in [
        (vec![], "no path"),
        (vec![""], "empty"),
        (vec!["/etc/passwd"], "not inside"),
        (vec!["../other/a"], "not inside"),
        (vec!["src/../../a"], "not inside"),
        (vec![".git/config"], "`.git`"),
        (vec!["sub/.GIT/hooks/pre-commit"], "`.git`"),
        (vec!["sub/.git"], "`.git`"),
        (vec!["a\nb"], "control character"),
        (vec!["a\u{1b}[2J"], "control character"),
    ] {
        let said = paths_ok(&paths(&refused)).expect_err("refused");
        assert!(said.contains(why), "{refused:?}: {said}");
    }
    let long = "a/".repeat(MOST_PATH);
    assert!(paths_ok(&[long]).unwrap_err().contains("longer than"));
    let many = vec!["a".to_owned(); MOST_PATHS + 1];
    assert!(paths_ok(&many).unwrap_err().contains("more than"));
    assert_eq!(paths_ok(&vec!["a".to_owned(); MOST_PATHS]), Ok(()));
}

#[test]
fn a_message_is_text_of_a_size() {
    assert_eq!(message_ok("fix: one\n\nwhy"), Ok(()));
    assert!(message_ok(" \n\t").unwrap_err().contains("empty"));
    assert!(message_ok("a\0b").unwrap_err().contains("NUL"));
    assert!(message_ok(&"m".repeat(MOST_MESSAGE + 1)).is_err());
    assert_eq!(message_ok(&"m".repeat(MOST_MESSAGE)), Ok(()));
}

#[cfg(unix)]
#[test]
fn nothing_is_staged_through_a_link_and_a_link_itself_may_be_named() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let cwd = dir.path().join("folder");
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(cwd.join("real")).expect("a folder");
    std::fs::create_dir_all(&outside).expect("outside");
    std::fs::write(outside.join("secret"), "s").expect("a file");
    std::os::unix::fs::symlink(&outside, cwd.join("out")).expect("a link to a folder");
    std::os::unix::fs::symlink(outside.join("secret"), cwd.join("real/link"))
        .expect("a link to a file");

    assert_eq!(through_no_link(&cwd, "real/a.txt"), Ok(()));
    assert_eq!(through_no_link(&cwd, "not/there/yet"), Ok(()));
    // The link is the last part: git stores it as a link, and reads nothing through it.
    assert_eq!(through_no_link(&cwd, "real/link"), Ok(()));
    assert_eq!(through_no_link(&cwd, "out"), Ok(()));
    for through in ["out/secret", "./out/deeper/x", "real/link/x"] {
        let said = through_no_link(&cwd, through).expect_err("refused");
        assert!(said.contains("through a link"), "{through}: {said}");
    }
}

#[test]
fn what_git_lists_as_staged_is_read_exactly() {
    let raw = b":100644 100644 aaaa bbbb M\0src/a.rs\0\
:000000 120000 0000 cccc A\0a link\0\
:000000 160000 0000 dddd A\0nested\0\
:100644 000000 eeee 0000 D\0gone.txt\0";
    assert_eq!(
        parse_raw(raw),
        Some(vec![
            change("100644", "100644", 'M', "src/a.rs"),
            change("000000", "120000", 'A', "a link"),
            change("000000", "160000", 'A', "nested"),
            change("100644", "000000", 'D', "gone.txt"),
        ])
    );
    assert_eq!(parse_raw(b""), Some(Vec::new()));
    // Anything else is not read as "nothing staged".
    assert_eq!(parse_raw(b"100644 100644 a b M\0x\0"), None);
    assert_eq!(parse_raw(b":100644 100644 a b M\0"), None);
    assert_eq!(parse_raw(b":100644\0x\0"), None);
}

#[test]
fn a_commit_that_would_record_another_repository_is_refused() {
    let fine = [
        change("100644", "100644", 'M', "src/a.rs"),
        change("000000", "120000", 'A', "a-link"),
        change("000000", "100644", 'A', ".gitattributes"),
        change("000000", "100644", 'A', "docs/gitmodules.md"),
    ];
    assert_eq!(records_another_repository(&fine), None);
    for refused in [
        change("000000", "160000", 'A', "nested"),
        change("160000", "000000", 'D', "was-a-submodule"),
        change("160000", "100644", 'T', "now-a-file"),
        change("000000", "100644", 'A', ".gitmodules"),
        change("100644", "100644", 'M', "vendor/.GitModules"),
    ] {
        assert!(
            records_another_repository(std::slice::from_ref(&refused)).is_some(),
            "{refused:?}"
        );
    }
}

#[test]
fn a_name_added_beside_its_twin_in_another_case_is_refused() {
    let tracked = paths(&["README.md", "src/Lib.rs", "src/lib.rs", "docs/a.md"]);
    let adds = |path: &str| [change("000000", "100644", 'A', path)];
    assert!(twin_among(&tracked, &adds("src/lib.rs")).is_some());
    assert!(twin_among(&tracked, &adds("src/Lib.rs")).is_some());
    // A pair already there is not this commit's, and a plain addition is fine.
    assert_eq!(twin_among(&tracked, &adds("docs/a.md")), None);
    assert_eq!(
        twin_among(&tracked, &[change("100644", "100644", 'M', "src/lib.rs")]),
        None
    );
}

fn asker(cwd: Option<&Path>) -> Asker {
    Asker {
        chat: 4,
        cwd: cwd.map(Path::to_path_buf),
        persona: Some("steward".to_owned()),
        harnessed: true,
        unsandboxed: false,
        config: None,
    }
}

fn ask() -> CommitAsk {
    CommitAsk {
        chat: 4,
        message: "one".to_owned(),
        stage: Stage::Tracked,
        folder: None,
    }
}

#[test]
fn a_chat_that_is_not_sandboxed_is_told_to_use_git_and_nothing_is_run() {
    // No manifest turns the sandbox on here, so every chat is one that commits by itself.
    let dir = tempfile::tempdir().expect("a temp dir");
    let root = dir.path();
    std::fs::create_dir_all(root.join("workspaces/alpha")).expect("a workspace");
    let identity = git::Isolated::default();
    for who in [
        asker(Some(root)),
        Asker {
            unsandboxed: true,
            ..asker(Some(root))
        },
        Asker {
            harnessed: false,
            ..asker(Some(root))
        },
    ] {
        assert_eq!(
            answer(root, &who, &ask(), &identity),
            Answer::No {
                why: PLAIN_GIT.to_owned()
            }
        );
    }
}

#[test]
fn the_folder_is_the_apps_record_and_one_that_is_no_branch_folder_is_refused() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let root = std::fs::canonicalize(dir.path()).expect("resolves");
    let workspace = root.join("workspaces/alpha");
    std::fs::create_dir_all(workspace.join(".worktrees/thing/piece")).expect("folders");
    // No record of where the chat stands.
    assert!(
        Here::of(&root, None)
            .unwrap_err()
            .contains("no record of where this chat stands")
    );
    // The project's top, a workspace, and a folder shaped like a branch folder with no link.
    for cwd in [
        root.clone(),
        workspace.clone(),
        workspace.join(".worktrees/thing/piece"),
    ] {
        let said = Here::of(&root, Some(&cwd)).unwrap_err();
        assert!(
            said.contains("does not stand in a branch folder"),
            "{}: {said}",
            cwd.display()
        );
    }
}

/// A project with one branch folder, `.worktrees/thing/piece`, in workspace `alpha`, laid out
/// by names alone: no git, and no link in it.
fn folders() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().expect("a temp dir");
    let root = std::fs::canonicalize(dir.path()).expect("resolves");
    let workspace = root.join("workspaces/alpha");
    std::fs::create_dir_all(workspace.join(".worktrees/thing/piece/src")).expect("folders");
    std::fs::create_dir_all(workspace.join(".worktrees/thing/other")).expect("folders");
    (dir, root, workspace)
}

#[test]
fn a_chat_above_branch_folders_commits_in_the_one_below_it_its_paths_name() {
    // #1476: a chat started in a workspace that holds the clone and its branch folders.
    let (_dir, root, workspace) = folders();
    let piece = workspace.join(".worktrees/thing/piece");
    let (folder, stage) = standing(
        &root,
        &workspace,
        &staging(&[
            ".worktrees/thing/piece/src/a.rs",
            "./.worktrees/thing/piece",
        ]),
    )
    .expect("a branch folder");
    assert_eq!(folder, piece);
    assert_eq!(stage, staging(&["src/a.rs", "."]));
    // From the project's top, and from the folder that holds a repo's branch folders.
    for (cwd, path) in [
        (
            root.clone(),
            "workspaces/alpha/.worktrees/thing/piece/src/a.rs",
        ),
        (workspace.join(".worktrees/thing"), "piece/src/a.rs"),
    ] {
        let (folder, stage) = standing(&root, &cwd, &staging(&[path])).expect("found");
        assert_eq!((folder, stage), (piece.clone(), staging(&["src/a.rs"])));
    }
    // A chat in the branch folder commits where it stands, as the ask names it.
    let (folder, stage) = standing(&root, &piece.join("src"), &staging(&["a.rs"])).expect("in it");
    assert_eq!((folder, stage), (piece.join("src"), staging(&["a.rs"])));
    assert_eq!(
        standing(&root, &piece, &Stage::Tracked).expect("in it"),
        (piece.clone(), Stage::Tracked)
    );
}

#[test]
fn from_above_a_path_in_no_branch_folder_two_folders_and_all_are_refused() {
    let (_dir, root, workspace) = folders();
    let said = |stage: Stage| standing(&root, &workspace, &stage).unwrap_err();
    for stage in [
        staging(&["README.md"]),
        staging(&[".worktrees/thing"]),
        staging(&["thing/src/a.rs"]),
    ] {
        let why = said(stage.clone());
        assert!(
            why.contains("not inside a branch folder"),
            "{stage:?}: {why}"
        );
    }
    let why = said(staging(&[
        ".worktrees/thing/piece/a",
        ".worktrees/thing/other/b",
    ]));
    assert!(why.contains("more than one branch folder"), "{why}");
    let why = said(Stage::Tracked);
    assert!(why.contains("does not stand in a branch folder"), "{why}");
    assert!(why.contains("--all"), "{why}");
    // Never a folder above the chat's own.
    let below = workspace.join(".worktrees/thing/piece/src");
    assert!(standing(&root, &workspace.join("elsewhere"), &staging(&["a"])).is_err());
    assert!(branch_folder_of(&root.join("workspaces"), &below, "a").is_none());
}

#[cfg(unix)]
#[test]
fn from_above_a_branch_folder_reached_through_a_link_is_refused() {
    let (_dir, root, workspace) = folders();
    std::os::unix::fs::symlink(
        workspace.join(".worktrees/thing/piece"),
        workspace.join(".worktrees/thing/linked"),
    )
    .expect("a link");
    let why = standing(
        &root,
        &workspace,
        &staging(&[".worktrees/thing/linked/src/a.rs"]),
    )
    .unwrap_err();
    assert!(why.contains("through a link"), "{why}");
    // And the whole ask is refused before that, by the path's own check.
    assert!(through_no_link(&workspace, ".worktrees/thing/linked/src/a.rs").is_err());
}

#[test]
fn a_chat_is_held_to_a_number_of_commits_in_a_while() {
    // #1476: sizes were bounded, and how often was not.
    let rate = crate::brokered::Rate::new(2, std::time::Duration::from_secs(60));
    let now = std::time::Instant::now();
    assert!(pace_of(&rate, 4, now).is_ok());
    assert!(pace_of(&rate, 4, now).is_ok());
    let why = pace_of(&rate, 4, now).unwrap_err();
    assert!(why.contains("2 commits in the last 1 minutes"), "{why}");
    // Another chat is counted apart, and the window passes.
    assert!(pace_of(&rate, 5, now).is_ok());
    assert!(pace_of(&rate, 4, now + std::time::Duration::from_secs(61)).is_ok());
    const _: () = assert!(COMMITS_A_WHILE >= 10);
}

#[test]
fn a_message_carrying_purlis_s_own_provenance_is_refused() {
    // #1476: purlis writes these; one a chat wrote would claim what purlis did not check.
    for message in [
        "one\n\nPurlis-Chat: 01ABC",
        "one\n\ncharter-persona: steward",
        "one\n\nCo-authored-by: x <x@example.invalid>\nPURLIS-CHANGE: c",
    ] {
        let why = trailers_ok(message).unwrap_err();
        assert!(why.contains("leave the line out"), "{message:?}: {why}");
    }
    for message in [
        "one\n\nCo-authored-by: x <x@example.invalid>",
        "one\n\nAssisted-by: claude-code",
        "mentions Purlis-Chat: in a sentence",
    ] {
        assert_eq!(trailers_ok(message), Ok(()), "{message:?}");
    }
}

#[test]
fn the_commit_itself_is_told_to_start_no_editor_signer_or_upkeep() {
    // Each is off another way as well (the message on standard input, the refused signing
    // keys); these hold on the commit's own command line whatever the repository's
    // configuration says (#1476).
    for word in [
        "--no-edit",
        "--no-gpg-sign",
        "--file=-",
        "gc.auto=0",
        "maintenance.auto=false",
    ] {
        assert!(COMMIT.contains(&word), "{word}");
    }
}

#[test]
fn the_refusal_of_other_spellings_names_what_is_never_done() {
    for word in ["amend", "reset", "rebase", "merge", "push"] {
        assert!(COMMITS_ONLY.contains(word), "{word}");
    }
}

// ---------------------------------------------------------------------------------------
// The git half, against real git
// ---------------------------------------------------------------------------------------

/// A repository and a linked worktree of it, each keeping its git data in a folder beside it,
/// so the fixture can be built where a `.git` folder cannot be written. The checks that read
/// the folder's link ([`Here::of`]) have their own tests; this is what git then does.
struct Tree {
    _dir: tempfile::TempDir,
    root: PathBuf,
    here: Here,
}

fn tree() -> Tree {
    let dir = tempfile::tempdir().expect("a temp dir");
    let root = std::fs::canonicalize(dir.path()).expect("resolves");
    let clone = root.join("clone");
    let store = root.join("clone.gitdir");
    let top = root.join("piece");
    std::fs::create_dir_all(&clone).expect("a folder");
    let kept = format!("--separate-git-dir={}", store.display());
    let ok = |run: git::Run| assert!(run.ok(), "{run:?}");
    ok(crate::testgit::run(
        &clone,
        &["init", "-q", "-b", "main", &kept, "."],
    ));
    std::fs::write(clone.join("README.md"), "one\n").expect("a file");
    ok(crate::testgit::run(&clone, &["add", "README.md"]));
    ok(crate::testgit::run(
        &clone,
        &[
            "-c",
            "user.name=Setup",
            "-c",
            "user.email=setup@example.invalid",
            "commit",
            "-q",
            "-m",
            "one",
        ],
    ));
    ok(crate::testgit::run(
        &clone,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "piece",
            &top.display().to_string(),
        ],
    ));
    let here = Here {
        cwd: top.clone(),
        git_dir: store.join("worktrees/piece"),
        common: store,
        clone,
        top,
    };
    Tree {
        _dir: dir,
        root,
        here,
    }
}

impl Tree {
    fn asked_from(&self, cwd: &Path, stage: Stage) -> Result<Vec<Say>, String> {
        self.telling(cwd, stage).map_err(|refused| refused.why)
    }

    /// [`Self::asked_from`], with the needs-you line of a refusal.
    fn telling(&self, cwd: &Path, stage: Stage) -> Result<Vec<Say>, Refused> {
        self.saying(cwd, stage, "piece: one change\n\nWhy it was made.")
    }

    /// [`Self::telling`], asked with `message`.
    fn saying(&self, cwd: &Path, stage: Stage, message: &str) -> Result<Vec<Say>, Refused> {
        let here = Here {
            cwd: cwd.to_path_buf(),
            top: self.here.top.clone(),
            git_dir: self.here.git_dir.clone(),
            common: self.here.common.clone(),
            clone: self.here.clone.clone(),
        };
        let ask = CommitAsk {
            chat: 4,
            message: message.to_owned(),
            stage,
            folder: None,
        };
        let identity = git::Isolated::identity(Some("The Person"), Some("person@example.invalid"));
        let plain = git::Isolated::default().pinned(&here.top, &here.git_dir);
        git::isolated(&plain, || made(&self.root, 4, &ask, &here, &identity))
    }

    fn asked(&self, stage: Stage) -> Result<Vec<Say>, String> {
        self.asked_from(&self.here.top, stage)
    }

    fn git(&self, args: &[&str]) -> String {
        let run = crate::testgit::run(&self.here.top, args);
        assert!(run.ok(), "{args:?}: {run:?}");
        run.out.trim_end().to_owned()
    }

    fn nothing_staged(&self) -> bool {
        self.git(&["diff", "--cached", "--name-only"]).is_empty()
    }
}

fn staging(of: &[&str]) -> Stage {
    Stage::Paths(paths(of))
}

#[test]
fn the_commit_lands_on_the_folders_branch_with_the_named_files_and_the_persons_identity() {
    let t = tree();
    let before = t.git(&["rev-parse", "HEAD"]);
    let top = &t.here.top;
    std::fs::create_dir_all(top.join("src")).unwrap();
    std::fs::write(top.join("src/new.rs"), "fn new() {}\n").unwrap();
    std::fs::write(top.join("README.md"), "one\ntwo\n").unwrap();
    std::fs::write(top.join("left-out.txt"), "not named\n").unwrap();

    let lines = t
        .asked(staging(&["src/new.rs", "README.md"]))
        .expect("committed");

    let id = t.git(&["rev-parse", "HEAD"]);
    assert_eq!(
        lines,
        [Say::Done(
            format!("committed {id} on piece: 2 files").into()
        )]
    );
    assert_eq!(t.git(&["rev-parse", "HEAD^"]), before);
    assert_eq!(t.git(&["rev-parse", "refs/heads/piece"]), id);
    assert_eq!(t.git(&["rev-parse", "refs/heads/main"]), before);
    assert_eq!(
        t.git(&["show", "--format=", "--name-only", "HEAD"]),
        "README.md\nsrc/new.rs"
    );
    assert_eq!(t.git(&["status", "--porcelain"]), "?? left-out.txt");
    assert_eq!(
        t.git(&["log", "-1", "--format=%an <%ae>|%cn <%ce>"]),
        "The Person <person@example.invalid>|The Person <person@example.invalid>"
    );
    // The record holds no chat 4 here, so the message is as written: no trailer is invented.
    assert_eq!(
        t.git(&["log", "-1", "--format=%B"]),
        "piece: one change\n\nWhy it was made."
    );
}

#[test]
fn tracked_changes_are_staged_from_the_top_and_paths_from_where_the_chat_stands() {
    let t = tree();
    let top = &t.here.top;
    let below = top.join("deep");
    std::fs::create_dir_all(&below).unwrap();
    std::fs::write(top.join("README.md"), "changed\n").unwrap();
    std::fs::write(below.join("a.txt"), "a\n").unwrap();

    t.asked_from(&below, Stage::Tracked).expect("committed");
    assert_eq!(
        t.git(&["show", "--format=", "--name-only", "HEAD"]),
        "README.md"
    );
    let nothing = t.asked(Stage::Tracked).expect_err("nothing changed");
    assert!(nothing.contains("nothing to commit"), "{nothing}");

    t.asked_from(&below, staging(&["a.txt"]))
        .expect("committed");
    assert_eq!(
        t.git(&["show", "--format=", "--name-only", "HEAD"]),
        "deep/a.txt"
    );
    assert_eq!(t.git(&["status", "--porcelain"]), "");
}

#[cfg(unix)]
#[test]
fn a_hook_of_the_repository_does_not_run_and_each_one_there_is_named() {
    use std::os::unix::fs::PermissionsExt as _;
    let t = tree();
    let ran = t.root.join("the-hook-ran");
    let hooks = t.here.common.join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    for name in ["pre-commit", "commit-msg", "post-commit"] {
        let hook = hooks.join(name);
        std::fs::write(
            &hook,
            format!("#!/bin/sh\ntouch '{}'\nexit 1\n", ran.display()),
        )
        .unwrap();
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    // One that git would not run is not spoken of.
    std::fs::write(hooks.join("prepare-commit-msg"), "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(
        hooks.join("prepare-commit-msg"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    std::fs::write(t.here.top.join("README.md"), "changed\n").unwrap();

    let lines = t.asked(Stage::Tracked).expect("committed");

    assert!(!ran.exists(), "a hook of the repository ran");
    assert_eq!(
        lines[1..],
        ["pre-commit", "commit-msg", "post-commit"].map(|name| Say::Warn(
            format!(
                "this repository's {name} hook was not run: for a sandboxed chat, purlis commits \
             with hooks off"
            )
            .into()
        ))
    );
}

#[cfg(unix)]
#[test]
fn a_link_is_committed_as_a_link_and_what_it_points_at_is_never_read() {
    let t = tree();
    let top = &t.here.top;
    let outside = t.root.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let secret = outside.join("secret.txt");
    std::fs::write(&secret, "what the chat may not read out\n").unwrap();
    std::os::unix::fs::symlink(&secret, top.join("to-a-file")).unwrap();
    std::os::unix::fs::symlink(&outside, top.join("to-a-folder")).unwrap();

    t.asked(staging(&["to-a-file", "to-a-folder"]))
        .expect("committed");

    for name in ["to-a-file", "to-a-folder"] {
        let entry = t.git(&["ls-tree", "HEAD", name]);
        assert!(entry.starts_with("120000 blob "), "{entry}");
    }
    assert_eq!(
        t.git(&["cat-file", "-p", "HEAD:to-a-file"]),
        secret.display().to_string()
    );
    assert!(!t.git(&["show", "HEAD"]).contains("may not read out"));
    // And git itself stages nothing beyond one, were the path check not there.
    let beyond = stage(&t.here, &staging(&["to-a-folder/secret.txt"])).expect_err("refused");
    assert!(beyond.contains("git did not stage that"), "{beyond}");
    assert!(t.nothing_staged());
}

#[test]
fn a_file_named_like_an_option_or_a_pattern_is_that_file() {
    let t = tree();
    let top = &t.here.top;
    let names = ["-n", "--amend", "--all", ":(top)README.md", "*.md"];
    for name in names {
        std::fs::write(top.join(name), "a name\n").unwrap();
    }
    // `*.md` is that file, not every Markdown file: README.md is changed and not committed.
    std::fs::write(top.join("README.md"), "changed\n").unwrap();

    t.asked(staging(&names)).expect("committed");

    let mut files: Vec<String> = t
        .git(&["show", "--format=", "--name-only", "HEAD"])
        .lines()
        .map(str::to_owned)
        .collect();
    files.sort();
    assert_eq!(files, ["*.md", "--all", "--amend", "-n", ":(top)README.md"]);
    assert_eq!(t.git(&["status", "--porcelain"]), " M README.md");
}

#[test]
fn a_repository_inside_the_folder_and_a_submodule_declaration_are_unstaged_and_refused() {
    let t = tree();
    let top = &t.here.top;
    let nested = top.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    let kept = format!(
        "--separate-git-dir={}",
        t.root.join("nested.gitdir").display()
    );
    for args in [
        vec!["init", "-q", "-b", "main", kept.as_str(), "."],
        vec![
            "-c",
            "user.name=Setup",
            "-c",
            "user.email=setup@example.invalid",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "one",
        ],
    ] {
        let run = crate::testgit::run(&nested, &args);
        assert!(run.ok(), "{run:?}");
    }
    std::fs::write(
        top.join(".gitmodules"),
        "[submodule \"x\"]\n\tpath = x\n\turl = https://example.invalid/x\n",
    )
    .unwrap();
    let before = t.git(&["rev-parse", "HEAD"]);

    let gitlink = t.asked(staging(&["nested"])).expect_err("refused");
    assert!(gitlink.contains("repository of its own"), "{gitlink}");
    assert!(t.nothing_staged());

    let modules = t.asked(staging(&[".gitmodules"])).expect_err("refused");
    assert!(modules.contains("declares submodules"), "{modules}");
    assert!(t.nothing_staged());
    assert_eq!(t.git(&["rev-parse", "HEAD"]), before);
}

#[test]
fn what_looks_like_a_secret_is_refused_by_the_scan_and_unstaged() {
    let t = tree();
    let key = ["-----BEGIN RSA PRIV", "ATE KEY-----"].concat();
    std::fs::write(t.here.top.join("id"), format!("{key}\nMIIB\n")).unwrap();
    let before = t.git(&["rev-parse", "HEAD"]);

    let refused = t
        .telling(&t.here.top, staging(&["id"]))
        .expect_err("refused");

    assert!(refused.why.contains("commit refused"), "{}", refused.why);
    // The app raises a needs-you item with the masked line a chat's own pre-commit sends
    // (#1476), and never the value.
    let queue = refused.scan.expect("a needs-you line");
    assert!(queue.starts_with("commit refused"), "{queue}");
    assert!(refused.why.starts_with(&queue), "{}", refused.why);
    assert!(!queue.contains("MIIB"), "{queue}");
    assert_eq!(t.git(&["rev-parse", "HEAD"]), before);
    assert!(t.nothing_staged());
    // A refusal that is not the scan's raises none.
    let other = t
        .telling(&t.here.top, staging(&["missing"]))
        .expect_err("refused");
    assert_eq!(other.scan, None, "{}", other.why);
}

/// #1133: the message a chat asks the app to commit with is read as a chat's own `commit-msg`
/// reads it, and a credential there is refused with the same needs-you line.
#[test]
fn a_credential_in_the_message_is_refused_by_the_scan_and_nothing_stays_staged() {
    let t = tree();
    std::fs::write(t.here.top.join("README.md"), "two\n").unwrap();
    let before = t.git(&["rev-parse", "HEAD"]);
    let key = ["ghp", "_0123456789abcdefABCDEF"].concat();

    let refused = t
        .saying(
            &t.here.top,
            staging(&["README.md"]),
            &format!("piece: two\n\npush with {key}"),
        )
        .expect_err("refused");

    let queue = refused.scan.expect("a needs-you line");
    assert!(queue.contains("the commit message:3"), "{queue}");
    assert!(
        !queue.contains(&key) && !refused.why.contains(&key),
        "{}",
        refused.why
    );
    assert_eq!(t.git(&["rev-parse", "HEAD"]), before);
    assert!(t.nothing_staged());

    // Nothing cuts a brokered message at git's scissors line, so what is below it is read.
    let below = t
        .saying(
            &t.here.top,
            staging(&["README.md"]),
            &format!("piece: two\n# ------------------------ >8 ------------------------\n{key}"),
        )
        .expect_err("refused");
    let queue = below.scan.expect("a needs-you line");
    assert!(queue.contains("the commit message:3"), "{queue}");
    assert_eq!(t.git(&["rev-parse", "HEAD"]), before);
    assert!(t.nothing_staged());
}

#[test]
fn the_repositorys_identity_wins_and_one_that_signs_or_is_mid_something_is_refused() {
    let t = tree();
    let top = &t.here.top;
    let config = t.here.common.join("config");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str("[user]\n\tname = The Repo\n\temail = repo@example.invalid\n");
    std::fs::write(&config, &text).unwrap();
    std::fs::write(top.join("README.md"), "changed\n").unwrap();
    t.asked(Stage::Tracked).expect("committed");
    assert_eq!(
        t.git(&["log", "-1", "--format=%an <%ae>"]),
        "The Repo <repo@example.invalid>"
    );

    // Staged by other hands: left alone.
    std::fs::write(top.join("staged.txt"), "theirs\n").unwrap();
    t.git(&["add", "staged.txt"]);
    let staged = t.asked(Stage::Tracked).expect_err("refused");
    assert!(staged.contains("purlis did not stage"), "{staged}");
    assert_eq!(t.git(&["diff", "--cached", "--name-only"]), "staged.txt");
    t.git(&["reset", "-q"]);

    std::fs::write(t.here.git_dir.join("MERGE_HEAD"), "0\n").unwrap();
    let merging = t.asked(Stage::Tracked).expect_err("refused");
    assert!(merging.contains("in the middle of a merge"), "{merging}");
    std::fs::remove_file(t.here.git_dir.join("MERGE_HEAD")).unwrap();

    text.push_str("[commit]\n\tgpgsign = true\n");
    std::fs::write(&config, &text).unwrap();
    std::fs::write(top.join("README.md"), "changed again\n").unwrap();
    let signed = t.asked(Stage::Tracked).expect_err("refused");
    assert!(signed.contains("signs its commits"), "{signed}");
    assert!(t.nothing_staged());

    // A repository that names a signer means its commits to be signed: refused, saying
    // why and what to do (#1476). A signer set for the person alone is never read here.
    let unsigned = text.replace("gpgsign = true", "gpgsign = false");
    std::fs::write(
        &config,
        format!("{unsigned}[gpg \"ssh\"]\n\tprogram = /opt/signer\n"),
    )
    .unwrap();
    let signer = t.asked(Stage::Tracked).expect_err("refused");
    assert!(
        signer.contains("names a signer (`gpg.ssh.program`)")
            && signer.contains("from your own terminal"),
        "{signer}"
    );
    assert!(t.nothing_staged());

    // A worktree's own config and borrowed objects are not read for a chat.
    std::fs::write(&config, &unsigned).unwrap();
    std::fs::write(t.here.git_dir.join("config.worktree"), "").unwrap();
    let own = t.asked(Stage::Tracked).expect_err("refused");
    assert!(
        own.contains("a configuration of this worktree's own"),
        "{own}"
    );
    std::fs::remove_file(t.here.git_dir.join("config.worktree")).unwrap();
    std::fs::create_dir_all(t.here.common.join("objects/info")).unwrap();
    std::fs::write(
        t.here.common.join("objects/info/alternates"),
        "/elsewhere\n",
    )
    .unwrap();
    let borrowed = t.asked(Stage::Tracked).expect_err("refused");
    assert!(borrowed.contains("objects borrowed"), "{borrowed}");
}

#[test]
fn a_file_marked_for_a_content_filter_is_unstaged_and_refused() {
    // The person's filters live in the config this commit does not read, so the file would
    // be stored as it is on disk where their own git would store the filtered form.
    let t = tree();
    let top = &t.here.top;
    std::fs::write(
        top.join(".gitattributes"),
        "*.bin filter=lfs diff=lfs merge=lfs -text\n",
    )
    .unwrap();
    std::fs::write(top.join("big.bin"), "the real bytes\n").unwrap();
    std::fs::write(top.join("notes.txt"), "plain\n").unwrap();
    let before = t.git(&["rev-parse", "HEAD"]);

    let refused = t
        .asked(staging(&["big.bin", "notes.txt", ".gitattributes"]))
        .expect_err("refused");
    assert!(
        refused.contains("\"big.bin\" is marked for a content filter"),
        "{refused}"
    );
    assert!(refused.contains("`filter=lfs`"), "{refused}");
    assert_eq!(t.git(&["rev-parse", "HEAD"]), before);
    assert!(t.nothing_staged());

    // What no filter is asked for is committed, the attributes file with it.
    t.asked(staging(&["notes.txt", ".gitattributes"]))
        .expect("committed");
    assert_eq!(t.git(&["status", "--porcelain"]), "?? big.bin");
}

#[test]
fn what_git_answers_about_filters_is_read_exactly() {
    assert_eq!(filter_named(b""), None);
    assert_eq!(
        filter_named(b"a.txt\0filter\0unspecified\0b\0filter\0unset\0"),
        None
    );
    assert_eq!(
        filter_named(b"a.txt\0filter\0unspecified\0deep/b.bin\0filter\0lfs\0c\0filter\0x\0"),
        Some(("deep/b.bin".to_owned(), "lfs".to_owned()))
    );
    assert_eq!(
        filter_named(b"a\0filter\0set\0"),
        Some(("a".to_owned(), "set".to_owned()))
    );
}

#[test]
fn a_chat_standing_below_the_top_is_held_to_the_checked_git_directory() {
    // A `.git` file left in the folder the chat stands in, naming another repository: every
    // call is given the git directory that was checked, so it is never read.
    let t = tree();
    let top = &t.here.top;
    let below = top.join("deep");
    std::fs::create_dir_all(&below).unwrap();
    let decoy = t.root.join("decoy.gitdir");
    let kept = format!("--separate-git-dir={}", decoy.display());
    let made_there = t.root.join("decoy");
    std::fs::create_dir_all(&made_there).unwrap();
    let run = crate::testgit::run(&made_there, &["init", "-q", "-b", "main", &kept, "."]);
    assert!(run.ok(), "{run:?}");
    std::fs::write(below.join(".git"), format!("gitdir: {}\n", decoy.display())).unwrap();
    std::fs::write(below.join("a.txt"), "a\n").unwrap();
    let decoy_before = std::fs::read_dir(decoy.join("objects")).unwrap().count();

    // Asked which repository a call there uses, git answers the checked one.
    let plain = git::Isolated::default().pinned(top, &t.here.git_dir);
    let used = git::isolated(&plain, || {
        git::run(&below, &["rev-parse", "--absolute-git-dir"], git::READ).unwrap()
    });
    assert_eq!(
        std::fs::canonicalize(used.line()).unwrap(),
        std::fs::canonicalize(&t.here.git_dir).unwrap(),
        "{used:?}"
    );
    // And a commit asked for from there never stages in the other one. git takes a folder
    // with a `.git` of its own for a repository inside this one and stages nothing from it,
    // so the ask is refused; what matters here is where nothing was written.
    let before = t.git(&["rev-parse", "HEAD"]);
    t.asked_from(&below, staging(&["a.txt"]))
        .expect_err("refused");
    assert_eq!(t.git(&["rev-parse", "HEAD"]), before);
    assert!(t.nothing_staged());
    assert!(
        !decoy.join("index").exists(),
        "the other repository was staged in"
    );
    assert_eq!(
        std::fs::read_dir(decoy.join("objects")).unwrap().count(),
        decoy_before
    );
}

#[test]
fn what_an_interrupted_commit_staged_is_put_back_and_the_next_ask_carries_on() {
    let t = tree();
    let top = &t.here.top;
    std::fs::write(top.join("README.md"), "changed\n").unwrap();
    std::fs::write(top.join("new.txt"), "new\n").unwrap();
    // An app that stopped after staging: the note names the tree it staged.
    t.git(&["add", "new.txt"]);
    let tree = t.git(&["write-tree"]);
    std::fs::write(t.here.git_dir.join(NOTE), &tree).unwrap();

    let lines = t.asked(Stage::Tracked).expect("committed");

    assert!(
        matches!(&lines[1], Say::Info(said) if said.contains("was interrupted")),
        "{lines:?}"
    );
    assert_eq!(
        t.git(&["show", "--format=", "--name-only", "HEAD"]),
        "README.md"
    );
    assert_eq!(t.git(&["status", "--porcelain"]), "?? new.txt");
    assert!(!t.here.git_dir.join(NOTE).exists());

    // One that stopped while staging left an empty note: whatever is staged is its own.
    std::fs::write(top.join("README.md"), "changed again\n").unwrap();
    t.git(&["add", "new.txt"]);
    std::fs::write(t.here.git_dir.join(NOTE), "").unwrap();
    t.asked(Stage::Tracked).expect("committed");
    assert_eq!(t.git(&["status", "--porcelain"]), "?? new.txt");

    // A note that names another tree: what is staged now is someone's, and stays.
    t.git(&["add", "new.txt"]);
    std::fs::write(
        t.here.git_dir.join(NOTE),
        "0000000000000000000000000000000000000000",
    )
    .unwrap();
    let theirs = t.asked(Stage::Tracked).expect_err("refused");
    assert!(theirs.contains("purlis did not stage"), "{theirs}");
    assert_eq!(t.git(&["diff", "--cached", "--name-only"]), "new.txt");
}

#[test]
fn two_asks_for_one_folder_commit_one_after_the_other() {
    let t = std::sync::Arc::new(tree());
    for name in ["a.txt", "b.txt"] {
        std::fs::write(t.here.top.join(name), "x\n").unwrap();
    }
    // The lock is the one `commit` takes; held here, the other thread's ask waits for it.
    let folder = lock_of(&t.here.git_dir);
    let asks: Vec<_> = ["a.txt", "b.txt"]
        .into_iter()
        .map(|name| {
            let (t, folder) = (t.clone(), folder.clone());
            std::thread::spawn(move || {
                let _held = folder.lock().unwrap();
                t.asked(staging(&[name])).map(|_| ())
            })
        })
        .collect();
    for ask in asks {
        ask.join().unwrap().expect("committed");
    }
    assert_eq!(t.git(&["status", "--porcelain"]), "");
    assert_eq!(t.git(&["rev-list", "--count", "HEAD"]), "3");
    assert!(std::sync::Arc::ptr_eq(&folder, &lock_of(&t.here.git_dir)));
    assert!(ran_out("finish staging", STAGING).contains("within 5 minutes"));
}

/// What a test does to a folder once it is staged.
type Swap = Box<dyn Fn(&Path)>;

thread_local! {
    /// What a test does to the folder once the ask's paths are staged, before the commit
    /// reads what was staged: a file swapped in that gap.
    static ONCE_STAGED: std::cell::RefCell<Option<Swap>> =
        const { std::cell::RefCell::new(None) };
}

/// Runs what the test set in [`ONCE_STAGED`], if it set anything.
pub(super) fn once_staged(top: &Path) {
    ONCE_STAGED.with(|then| {
        if let Some(then) = then.borrow().as_ref() {
            then(top);
        }
    });
}

/// `run`, with `then` done to the folder once the ask's paths are staged.
fn swapping<T>(then: impl Fn(&Path) + 'static, run: impl FnOnce() -> T) -> T {
    struct Clear;
    impl Drop for Clear {
        fn drop(&mut self) {
            ONCE_STAGED.with(|then| then.borrow_mut().take());
        }
    }
    ONCE_STAGED.with(|held| *held.borrow_mut() = Some(Box::new(then)));
    let _clear = Clear;
    run()
}

impl Tree {
    /// Whether the repository's object store holds `sha`.
    fn holds(&self, sha: &str) -> bool {
        crate::testgit::run(&self.here.top, &["cat-file", "-e", sha]).ok()
    }
}

#[cfg(unix)]
#[test]
fn a_named_file_with_a_second_name_is_refused_before_anything_is_staged() {
    let t = tree();
    let top = &t.here.top;
    std::fs::write(top.join("linked.txt"), "bytes with a second name\n").unwrap();
    std::fs::hard_link(top.join("linked.txt"), t.root.join("second-name")).unwrap();
    std::fs::write(top.join("plain.txt"), "one name\n").unwrap();
    let blob = t.git(&["hash-object", "linked.txt"]);
    let before = t.git(&["rev-parse", "HEAD"]);

    let refused = t
        .asked(staging(&["plain.txt", "linked.txt"]))
        .expect_err("refused");

    assert!(refused.contains("linked.txt"), "{refused}");
    assert!(refused.contains("has a second name"), "{refused}");
    assert!(refused.contains("Nothing was staged"), "{refused}");
    assert_eq!(t.git(&["rev-parse", "HEAD"]), before);
    assert!(t.nothing_staged());
    assert!(!t.holds(&blob), "the file's bytes reached the object store");
}

#[cfg(unix)]
#[test]
fn a_file_swapped_after_it_was_staged_is_refused_unstaged_and_its_blob_taken_out() {
    let t = tree();
    let top = &t.here.top;
    std::fs::write(top.join("swapped.txt"), "the bytes that were staged\n").unwrap();
    let blob = t.git(&["hash-object", "swapped.txt"]);
    let before = t.git(&["rev-parse", "HEAD"]);
    let aside = t.root.join("aside");

    let refused = swapping(
        move |top| {
            std::fs::rename(top.join("swapped.txt"), &aside).unwrap();
            std::fs::write(top.join("swapped.txt"), "other bytes\n").unwrap();
        },
        || t.asked(staging(&["swapped.txt"])),
    )
    .expect_err("refused");

    assert!(
        refused.contains("changed on disk after it was staged"),
        "{refused}"
    );
    assert_eq!(t.git(&["rev-parse", "HEAD"]), before);
    assert!(t.nothing_staged());
    assert!(
        !t.holds(&blob),
        "the staged bytes are still in the object store"
    );
    assert_eq!(
        std::fs::read_to_string(top.join("swapped.txt")).unwrap(),
        "other bytes\n"
    );
}

fn named(workspace: &str, repo: &str, piece: &str) -> FolderName {
    FolderName {
        workspace: workspace.to_owned(),
        repo: repo.to_owned(),
        piece: piece.to_owned(),
    }
}

#[test]
fn a_folder_the_command_ran_in_takes_its_paths_from_its_top_wherever_the_chat_stands() {
    // #3927: a chat in its workspace ran the command in a branch folder below it.
    let (_dir, root, workspace) = folders();
    let piece = workspace.join(".worktrees/thing/piece");
    let ran_in = named("alpha", "thing", "piece");
    for cwd in [
        root.clone(),
        workspace.clone(),
        workspace.join(".worktrees/thing"),
        piece.clone(),
        piece.join("src"),
    ] {
        let (folder, stage) =
            named_folder(&root, &cwd, &ran_in, &staging(&["src/a.rs"])).expect("placed");
        assert_eq!(folder, piece, "{}", cwd.display());
        assert_eq!(stage, staging(&["src/a.rs"]));
        let (_, stage) = named_folder(&root, &cwd, &ran_in, &Stage::Tracked).expect("placed");
        assert_eq!(stage, Stage::Tracked);
    }
}

#[test]
fn a_folder_the_command_ran_in_beside_the_chats_own_or_not_there_is_refused() {
    let (_dir, root, workspace) = folders();
    let stage = staging(&["src/a.rs"]);
    // A chat in another branch folder, or in another workspace.
    std::fs::create_dir_all(root.join("workspaces/beta")).expect("a workspace");
    for cwd in [
        workspace.join(".worktrees/thing/other"),
        root.join("workspaces/beta"),
    ] {
        let why = named_folder(&root, &cwd, &named("alpha", "thing", "piece"), &stage).unwrap_err();
        assert!(why.contains("thing · piece"), "{why}");
        assert!(why.contains("nothing was committed"), "{why}");
    }
    // A folder that is not there, and names that cannot name one.
    for bad in [
        named("alpha", "thing", "gone"),
        named("alpha", "..", "piece"),
        named("../x", "thing", "piece"),
    ] {
        assert!(
            named_folder(&root, &workspace, &bad, &stage).is_err(),
            "{bad:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_folder_the_command_ran_in_reached_through_a_link_is_refused() {
    let (_dir, root, workspace) = folders();
    std::os::unix::fs::symlink(
        workspace.join(".worktrees/thing/piece"),
        workspace.join(".worktrees/thing/linked"),
    )
    .expect("a link");
    let why = named_folder(
        &root,
        &workspace,
        &named("alpha", "thing", "linked"),
        &staging(&["src/a.rs"]),
    )
    .unwrap_err();
    assert!(why.contains("through a link"), "{why}");
    // And a path through a link inside it is refused as from where the chat stands.
    std::os::unix::fs::symlink(&workspace, workspace.join(".worktrees/thing/piece/out"))
        .expect("a link");
    let why = named_folder(
        &root,
        &workspace,
        &named("alpha", "thing", "piece"),
        &staging(&["out/a.rs"]),
    )
    .unwrap_err();
    assert!(why.contains("through a link"), "{why}");
}
