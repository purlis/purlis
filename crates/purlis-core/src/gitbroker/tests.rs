// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn asker(cwd: Option<&Path>) -> Asker {
    Asker {
        chat: 4,
        cwd: cwd.map(Path::to_path_buf),
        persona: Some("steward".to_owned()),
        harnessed: true,
        unsandboxed: false,
        config: None,
        id: None,
        live: None,
    }
}

/// A project directory with two workspaces, and no manifest: what [`bound`] reads is the tree.
fn tree() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    for ws in ["alpha", "beta"] {
        std::fs::create_dir_all(root.join("workspaces").join(ws).join("widget")).unwrap();
    }
    std::fs::create_dir_all(root.join("docs")).unwrap();
    (dir, root)
}

#[test]
fn a_chat_at_the_project_root_may_have_any_workspace_written() {
    let (_dir, root) = tree();
    assert_eq!(bound(&root, Some(&root), "alpha"), Ok(()));
    assert_eq!(bound(&root, Some(&root), "beta"), Ok(()));
}

#[test]
fn a_chat_in_a_workspace_may_have_only_that_workspace_written() {
    let (_dir, root) = tree();
    let in_alpha = root.join("workspaces/alpha/widget");
    assert_eq!(bound(&root, Some(&in_alpha), "alpha"), Ok(()));
    let refused = bound(&root, Some(&in_alpha), "beta").expect_err("another workspace");
    assert!(refused.contains("works in workspace 'alpha'"), "{refused}");
    assert!(refused.contains("'beta'"), "{refused}");
}

#[test]
fn a_chat_elsewhere_or_nowhere_has_no_workspace_written() {
    let (_dir, root) = tree();
    let outside = tempfile::tempdir().unwrap();
    for cwd in [root.join("docs"), outside.path().to_path_buf()] {
        let refused = bound(&root, Some(&cwd), "alpha").expect_err("outside the workspaces");
        assert!(
            refused.contains("outside the project's workspaces"),
            "{refused}"
        );
    }
    assert!(bound(&root, None, "alpha").is_err());
}

#[test]
fn a_refused_workspace_is_answered_no_and_nothing_runs() {
    let (_dir, root) = tree();
    let answer = answer(
        &root,
        &asker(Some(&root.join("workspaces/alpha"))),
        &GitAsk {
            chat: 4,
            workspace: "beta".to_owned(),
            alive: false,
            work: GitWork::WorktreeAdd {
                repo: "widget".to_owned(),
                piece: "p1".to_owned(),
                branch: None,
            },
        },
        &git::Isolated::default(),
        chrono::Utc::now(),
    );
    assert!(matches!(answer, Answer::No { .. }), "{answer:?}");
    assert!(!root.join("workspaces/beta/pieces").exists());
}

/// #3931: the Bash guard of a chat that runs git itself asks before plain git writes a branch
/// folder. The first live chat takes the folder, a second is refused in the brokered refusal's
/// words, and a shell tab (the person's own terminal) is never held to it.
#[test]
fn plain_git_s_claim_is_taken_by_the_first_live_chat_and_refused_to_a_second() {
    use crate::hookwire::ClaimAct;
    let (_dir, root) = tree();
    let live = vec![
        crate::piececlaim::Live {
            id: "01A".to_owned(),
            chat: Some(4),
            back: true,
        },
        crate::piececlaim::Live {
            id: "01B".to_owned(),
            chat: Some(9),
            back: true,
        },
    ];
    let chat = |number: u32, id: &str, harnessed: bool| Asker {
        chat: number,
        id: Some(id.to_owned()),
        live: Some(live.clone()),
        harnessed,
        unsandboxed: true,
        // Anywhere: the claim runs no git, so where the chat stands is not asked.
        cwd: None,
        ..asker(None)
    };
    let ask = |act| GitAsk {
        chat: 0,
        workspace: "alpha".to_owned(),
        work: GitWork::WorktreeClaim {
            repo: "widget".to_owned(),
            piece: "p1".to_owned(),
            act,
        },
    };
    let asked = |asker: &Asker, act| {
        answer(
            &root,
            asker,
            &ask(act),
            &git::Isolated::default(),
            chrono::Utc::now(),
        )
    };
    let may = Answer::Said {
        lines: Vec::new(),
        code: 0,
    };
    assert_eq!(asked(&chat(4, "01A", true), ClaimAct::Commit), may);
    assert_eq!(asked(&chat(4, "01A", true), ClaimAct::Push), may);
    // Held by another live chat: the one answer the guard refuses on.
    let Answer::Said { lines, code } = asked(&chat(9, "01B", true), ClaimAct::Push) else {
        panic!("a second live chat's push was let through");
    };
    assert_eq!(code, crate::hookwire::CLAIM_HELD);
    let [Say::Fail(why)] = lines.as_slice() else {
        panic!("{lines:?}");
    };
    assert!(
        why.contains("widget's branch folder 'p1' is claimed by chat 4"),
        "{why}"
    );
    assert!(why.contains("will not push in it"), "{why}");
    assert!(why.contains("purlis worktree done"), "{why}");
    // A shell tab is the person's own: never refused, and it takes nothing.
    assert_eq!(asked(&chat(9, "01B", false), ClaimAct::Commit), may);
    assert!(matches!(
        asked(&chat(9, "01B", true), ClaimAct::Commit),
        Answer::Said {
            code: crate::hookwire::CLAIM_HELD,
            ..
        }
    ));
    // The holder still writes.
    assert_eq!(asked(&chat(4, "01A", true), ClaimAct::Update), may);
    // A name that cannot be a branch folder is refused before anything is read.
    let odd = GitAsk {
        chat: 0,
        workspace: "../alpha".to_owned(),
        work: GitWork::WorktreeClaim {
            repo: "widget".to_owned(),
            piece: "p1".to_owned(),
            act: ClaimAct::Commit,
        },
    };
    let odd = answer(
        &root,
        &chat(4, "01A", true),
        &odd,
        &git::Isolated::default(),
        chrono::Utc::now(),
    );
    assert!(matches!(odd, Answer::No { .. }), "{odd:?}");
}

/// The review of #4129: only a folder another LIVE chat holds is `CLAIM_HELD`. A holder that
/// has not come back since a restart is a refusal the guard lets through, so it is a `No`.
#[test]
fn a_holder_not_back_since_a_restart_is_answered_no_and_not_held() {
    use crate::hookwire::ClaimAct;
    let (_dir, root) = tree();
    let chat = |number: u32, id: &str, holder_back: bool| Asker {
        chat: number,
        id: Some(id.to_owned()),
        live: Some(vec![
            crate::piececlaim::Live {
                id: "01A".to_owned(),
                chat: Some(4),
                back: holder_back,
            },
            crate::piececlaim::Live {
                id: "01B".to_owned(),
                chat: Some(9),
                back: true,
            },
        ]),
        unsandboxed: true,
        ..asker(None)
    };
    let asked = |asker: &Asker| {
        answer(
            &root,
            asker,
            &GitAsk {
                chat: 0,
                workspace: "alpha".to_owned(),
                work: GitWork::WorktreeClaim {
                    repo: "widget".to_owned(),
                    piece: "p1".to_owned(),
                    act: ClaimAct::Push,
                },
            },
            &git::Isolated::default(),
            chrono::Utc::now(),
        )
    };
    // Chat 4 takes the folder while it runs.
    assert!(matches!(
        asked(&chat(4, "01A", true)),
        Answer::Said { code: 0, .. }
    ));
    // Restarted, and not back yet: chat 9 is told no, not that a live chat holds it.
    let answer = asked(&chat(9, "01B", false));
    let Answer::No { why } = &answer else {
        panic!("{answer:?}");
    };
    assert!(why.contains("has not come back"), "{why}");
}

#[test]
fn a_shell_tab_or_a_chat_started_without_the_sandbox_has_no_host_limit() {
    // Neither reads the manifest: a shell is the operator's own, and the opt-out is a person's.
    let (_dir, root) = tree();
    let shell = Asker {
        harnessed: false,
        ..asker(None)
    };
    assert_eq!(hosts(&root, &shell), Ok(None));
    let lifted = Asker {
        unsandboxed: true,
        ..asker(None)
    };
    assert_eq!(hosts(&root, &lifted), Ok(None));
}

#[test]
fn a_sandboxed_chat_reaches_only_its_egress_and_a_project_without_the_sandbox_none_is_set() {
    let (_dir, root) = tree();
    let manifest = crate::names::manifest(&root);
    std::fs::write(
        &manifest,
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\n",
    )
    .unwrap();
    let hosts_of = hosts(&root, &asker(None)).unwrap().expect("a limit");
    assert!(hosts_of.iter().any(|h| h == "api.anthropic.com"));
    assert!(!hosts_of.iter().any(|h| h == "github.com"), "{hosts_of:?}");

    std::fs::write(&manifest, "").unwrap();
    assert_eq!(hosts(&root, &asker(None)), Ok(None));
}

#[test]
fn lines_that_fit_are_kept_whole_and_ones_that_do_not_are_cut_and_counted() {
    let few = vec![
        Say::Info("a".to_owned().into()),
        Say::Done("b".to_owned().into()),
    ];
    assert_eq!(fitted(few.clone(), SAID_AT_MOST), few);

    let many: Vec<Say> = (0..200)
        .map(|n| Say::Fail(format!("{n}:{}", "x".repeat(5000)).into()))
        .collect();
    let kept = fitted(many, SAID_AT_MOST);
    let json = serde_json::to_vec(&kept).unwrap();
    assert!(json.len() <= SAID_AT_MOST, "{} bytes", json.len());
    let Some(Say::Warn(note)) = kept.last() else {
        panic!("no note of what was cut: {:?}", kept.last());
    };
    let cut: usize = note.split(' ').next().unwrap().parse().unwrap();
    assert_eq!(kept.len() - 1 + cut, 200, "{note}");
    assert!(matches!(&kept[0], Say::Fail(text) if text.starts_with("0:") && text.ends_with(" …")));
}

#[test]
fn every_kind_of_key_that_names_a_program_is_one_and_ordinary_keys_are_not() {
    for key in [
        "filter.lfs.smudge",
        "filter.evil.process",
        "diff.pdf.textconv",
        "diff.x.command",
        "diff.external",
        "merge.ours.driver",
        "core.sshcommand",
        "core.gitproxy",
        "core.alternaterefscommand",
        "include.path",
        "includeif.gitdir:/x/.path",
        "credential.helper",
        "credential.https://github.com.helper",
        "remote.origin.uploadpack",
        // What a commit or a later git could start (#1476): a trailer's command, and a hook a
        // configuration defines rather than the hooks folder.
        "trailer.sign.cmd",
        "trailer.sign.command",
        "hook.lint.command",
        "hook.lint.event",
        // What a gc a commit starts runs, and the helper a remote's transport starts (#1853).
        "gc.recentobjectshook",
        "remote.origin.vcs",
    ] {
        assert!(names_a_program(key), "{key}");
    }
    for key in [
        "core.bare",
        "remote.origin.url",
        "user.name",
        "diff.renames",
        "merge.conflictstyle",
        "credential.username",
        "branch.main.merge",
        // Settings of those that name no program, and an editor a commit given its message
        // never starts. A signer is the brokered commit's alone to refuse: a clone or a
        // worktree add starts none.
        "gpg.program",
        "gpg.ssh.program",
        "gpg.format",
        "gpg.ssh.allowedsignersfile",
        "user.signingkey",
        "trailer.sign.key",
        "trailer.sign.where",
        "trailer.ifexists",
        "core.editor",
        "commit.template",
    ] {
        assert!(!names_a_program(key), "{key}");
    }
}

#[test]
fn a_signer_is_named_by_its_keys_and_by_no_other_signing_setting() {
    for key in [
        "gpg.program",
        "gpg.ssh.program",
        "gpg.x509.program",
        "gpg.openpgp.program",
        "gpg.ssh.defaultkeycommand",
        "GPG.SSH.Program",
    ] {
        assert!(names_a_signer(key), "{key}");
    }
    for key in [
        "gpg.format",
        "gpg.ssh.allowedsignersfile",
        "gpg.a.b.program",
        "gpg..program",
        "user.signingkey",
        "commit.gpgsign",
        "core.editor",
    ] {
        assert!(!names_a_signer(key), "{key}");
    }
}

/// A repo at `workspaces/alpha/widget` whose `.gitattributes` asks for content filter `evil`
/// on every file, and the path a run of the filter would leave behind.
fn a_repo_asking_for_a_filter(root: &Path) -> (PathBuf, PathBuf) {
    let repo = root.join("workspaces/alpha/widget");
    crate::testgit::run(&repo, &["init", "-q", "-b", "main", "."]);
    commit_asking_for_a_filter(&repo);
    (repo, root.join("filter-ran"))
}

/// A commit in `repo` whose `.gitattributes` asks for content filter `evil` on every file.
fn commit_asking_for_a_filter(repo: &Path) {
    std::fs::write(repo.join(".gitattributes"), "* filter=evil\n").unwrap();
    std::fs::write(repo.join("a.txt"), "a\n").unwrap();
    crate::testgit::run(repo, &["add", "-A"]);
    crate::testgit::run(repo, &["commit", "-q", "-m", "one"]);
}

/// `config` with content filter `evil` defined to leave `ran` behind when it runs. Quoted, so
/// git does not read the `;` as the start of a comment and cut the command short.
fn with_the_filter(config: &Path, ran: &Path) {
    let mut text = std::fs::read_to_string(config).unwrap_or_default();
    text.push_str(&format!(
        "[filter \"evil\"]\n\tsmudge = \"sh -c 'touch {}; cat'\"\n\tclean = cat\n",
        ran.display()
    ));
    std::fs::write(config, text).unwrap();
}

/// What the app answers chat 4, at the project's top, asking for piece `p1` of `repo`.
fn a_worktree_of(root: &Path, repo: &str) -> Answer {
    answer(
        root,
        &asker(Some(root)),
        &GitAsk {
            chat: 4,
            workspace: "alpha".to_owned(),
            alive: false,
            work: GitWork::WorktreeAdd {
                repo: repo.to_owned(),
                piece: "p1".to_owned(),
                branch: None,
            },
        },
        &git::Isolated::default(),
        chrono::Utc::now(),
    )
}

#[test]
fn a_bare_repository_a_chat_made_in_its_workspace_is_never_the_one_git_runs_in() {
    // D-1335-8: `init --bare` writes `config` with no `.git` above it, so no denied path
    // covers it, and discovery would take the directory as the repository.
    let (_dir, root) = tree();
    let ran = root.join("filter-ran");
    let src = root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    crate::testgit::run(
        &src,
        &[
            "init",
            "-q",
            "-b",
            "main",
            &format!("--separate-git-dir={}", root.join("src.gitdir").display()),
            ".",
        ],
    );
    commit_asking_for_a_filter(&src);
    let evil = root.join("workspaces/alpha/evil");
    crate::testgit::run(
        &root,
        &[
            "clone",
            "-q",
            "--bare",
            &src.display().to_string(),
            &evil.display().to_string(),
        ],
    );
    with_the_filter(&evil.join("config"), &ran);

    let answer = a_worktree_of(&root, "evil");

    assert!(matches!(answer, Answer::No { .. }), "{answer:?}");
    assert!(!ran.exists(), "the chat's filter ran outside its sandbox");
    assert!(!root.join("workspaces/alpha/.worktrees/evil").exists());
}

#[test]
fn a_directory_with_no_git_of_its_own_is_never_taken_for_the_project_s_repository() {
    // D-1335-8: discovery would climb from a plain directory to the project's own repository,
    // whose config the check never read.
    let (_dir, root) = tree();
    let ran = root.join("filter-ran");
    crate::testgit::run(&root, &["init", "-q", "-b", "main", "."]);
    commit_asking_for_a_filter(&root);
    with_the_filter(&root.join(".git/config"), &ran);
    std::fs::create_dir_all(root.join("workspaces/alpha/plain")).unwrap();

    let answer = a_worktree_of(&root, "plain");

    assert!(matches!(answer, Answer::No { .. }), "{answer:?}");
    assert!(!ran.exists(), "the project's filter ran for the chat");
    assert!(!root.join("workspaces/alpha/.worktrees/plain").exists());
}

#[test]
fn a_commondir_pointed_at_a_config_the_chat_wrote_is_refused_and_its_filter_never_runs() {
    // F1 of #1335's review: a chat that could write `.git/commondir` points git at a common
    // directory of its own, whose config defines the filter the committed `.gitattributes`
    // asks for. The app must neither run it nor cut the worktree.
    let (_dir, root) = tree();
    let (repo, ran) = a_repo_asking_for_a_filter(&root);
    let theirs = root.join("theirs.git");
    crate::testgit::run(
        &root,
        &[
            "clone",
            "-q",
            "--bare",
            &repo.display().to_string(),
            &theirs.display().to_string(),
        ],
    );
    let mut config = std::fs::read_to_string(theirs.join("config")).unwrap();
    config.push_str(&format!(
        "[filter \"evil\"]\n\tsmudge = \"sh -c 'touch {}; cat'\"\n\tclean = cat\n",
        ran.display()
    ));
    std::fs::write(theirs.join("config"), config).unwrap();
    std::fs::write(
        repo.join(".git/commondir"),
        format!("{}\n", theirs.display()),
    )
    .unwrap();
    // The redirect is live: git now reads the filter from the chat's config.
    assert!(
        crate::testgit::run(&repo, &["config", "--get", "filter.evil.smudge"])
            .out
            .contains("touch"),
        "the commondir did not redirect the config"
    );

    let answer = answer(
        &root,
        &asker(Some(&root)),
        &GitAsk {
            chat: 4,
            workspace: "alpha".to_owned(),
            alive: false,
            work: GitWork::WorktreeAdd {
                repo: "widget".to_owned(),
                piece: "p1".to_owned(),
                branch: None,
            },
        },
        &git::Isolated::default(),
        chrono::Utc::now(),
    );

    let Answer::No { why } = answer else {
        panic!("not refused: {answer:?}");
    };
    assert!(why.contains("`filter.evil.smudge`"), "{why}");
    assert!(!ran.exists(), "the chat's filter ran outside its sandbox");
    assert!(!root.join("workspaces/alpha/.worktrees/widget/p1").exists());
}

#[test]
fn a_repos_own_include_or_credential_helper_is_refused_by_name() {
    let (_dir, root) = tree();
    let (repo, _) = a_repo_asking_for_a_filter(&root);
    for (key, value) in [
        ("include.path", "/elsewhere/config"),
        ("credential.helper", "!sh -c 'echo x'"),
    ] {
        crate::testgit::run(&repo, &["config", "--local", key, value]);
        let refused = git::isolated(&git::Isolated::default(), || runs_a_program(&repo))
            .map(|_| ())
            .expect_err("refused");
        assert!(refused.contains(&format!("`{key}`")), "{refused}");
        crate::testgit::run(&repo, &["config", "--local", "--unset", key]);
    }
    // The helper purlis's own git policy writes into every clone it makes is not refused.
    crate::testgit::run(
        &repo,
        &[
            "config",
            "--local",
            "credential.helper",
            "!gh auth git-credential",
        ],
    );
    assert_eq!(
        git::isolated(&git::Isolated::default(), || runs_a_program(&repo)),
        Ok(std::fs::canonicalize(repo.join(".git")).unwrap())
    );
}

// ---- the checked clone is held to its identity (#1415) ------------------------------------

/// A clone whose `.git` is renamed away while a worktree is cut from it, and another put in
/// its place: the add says so and fails, whatever git answered.
#[test]
fn a_worktree_add_whose_clone_was_replaced_while_it_ran_fails_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let clone = std::fs::canonicalize(dir.path()).unwrap().join("widget");
    let git_dir = clone.join(".git");
    std::fs::create_dir_all(&git_dir).unwrap();
    let found = Checked {
        git_dir: git_dir.clone(),
        id: git::DirId::of(&git_dir),
    };
    let mut lines = Vec::new();
    let code = cut_pinned(
        &git::Isolated::default(),
        &clone,
        &found,
        &mut |line| lines.push(line),
        |_| {
            std::fs::rename(&git_dir, clone.join("checked.git")).unwrap();
            std::fs::create_dir(&git_dir).unwrap();
            0
        },
    );
    assert_eq!(code, 1);
    assert!(
        lines
            .iter()
            .any(|line| matches!(line, Say::Fail(text) if text.contains("was replaced"))),
        "{lines:?}"
    );
    // Untouched, the add's own answer stands.
    let mut quiet = Vec::new();
    let found = Checked {
        git_dir: clone.join(".git"),
        id: git::DirId::of(&clone.join(".git")),
    };
    let code = cut_pinned(
        &git::Isolated::default(),
        &clone,
        &found,
        &mut |line| quiet.push(line),
        |_| 0,
    );
    assert_eq!((code, quiet.len()), (0, 0));
}

/// Work the app does in a checked clone is refused before git runs once the clone's `.git`
/// is another directory than the one checked.
#[test]
fn a_clone_replaced_after_its_check_runs_no_git() {
    let (_dir, root) = tree();
    let (repo, _) = a_repo_asking_for_a_filter(&root);
    let answered = in_a_checked_clone(&root, "alpha", "widget", &git::Isolated::default(), || {
        std::fs::rename(repo.join(".git"), root.join("checked.git")).unwrap();
        std::fs::create_dir(repo.join(".git")).unwrap();
        git::run(&repo, &["rev-parse", "--absolute-git-dir"], git::READ).expect("answered")
    })
    .expect("checked");
    assert!(!answered.ok(), "{answered:?}");
    assert!(answered.err.contains("was replaced"), "{answered:?}");
}

#[test]
fn a_branch_folders_own_git_is_run_for_a_chat_in_it_or_above_it_and_no_other() {
    // qw387: `purlis worktree update` and `push` name a folder; the chat must stand in it, or
    // above branch folders, as the brokered commit holds it.
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let pieces = root.join("workspaces/alpha/.worktrees/widget");
    for piece in ["p1", "p2"] {
        std::fs::create_dir_all(pieces.join(piece)).unwrap();
    }
    let ws = root.join("workspaces/alpha");
    assert_eq!(at_or_above(&root, Some(&ws), "widget", "p1"), Ok(()));
    assert_eq!(at_or_above(&root, Some(&root), "widget", "p1"), Ok(()));
    assert_eq!(
        at_or_above(&root, Some(&pieces.join("p1")), "widget", "p1"),
        Ok(())
    );
    let refused = at_or_above(&root, Some(&pieces.join("p2")), "widget", "p1").unwrap_err();
    assert!(refused.contains("widget · p2"), "{refused}");
    assert!(at_or_above(&root, None, "widget", "p1").is_err());
}

#[test]
fn the_root_is_updated_only_for_the_chat_whose_place_it_is() {
    // #3831: a chat in a workspace has its own folders updated, never the root.
    let (_dir, root) = tree();
    assert_eq!(at_the_root(&root, Some(&root)), Ok(()));
    for elsewhere in [
        root.join("workspaces/alpha"),
        root.join("workspaces/alpha/widget"),
        root.join("docs"),
        root.join("gone"),
    ] {
        assert!(
            at_the_root(&root, Some(&elsewhere)).is_err(),
            "{}",
            elsewhere.display()
        );
    }
    assert!(at_the_root(&root, None).is_err());
}

#[cfg(unix)]
#[test]
fn a_clone_whose_git_folder_is_a_link_is_never_the_one_git_runs_in() {
    // #3831: where git keeps a clone is a real folder, never a link a chat could point.
    let (_dir, root) = tree();
    let elsewhere = root.join("elsewhere.git");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let clone = root.join("workspaces/alpha/widget");
    std::os::unix::fs::symlink(&elsewhere, clone.join(".git")).unwrap();
    let Err(refused) = checked_repo(&clone) else {
        panic!("a clone whose `.git` is a link was checked");
    };
    assert!(
        refused.contains("not a clone with its own `.git`"),
        "{refused}"
    );
}

#[test]
fn a_clones_own_checkout_is_updated_for_a_chat_in_it_or_above_it_and_no_other() {
    // #3831: `purlis worktree update` standing in a clone names the clone; the chat must stand
    // in it, in its workspace's folder or at the project root, never in a branch folder or
    // beside the clone.
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let ws = root.join("workspaces/alpha");
    for folder in ["widget/src", "gadget", "notes", ".worktrees/widget/p1"] {
        std::fs::create_dir_all(ws.join(folder)).unwrap();
    }
    for here in [&root, &ws, &ws.join("widget"), &ws.join("widget/src")] {
        assert_eq!(
            at_or_above_clone(&root, Some(here), "alpha", "widget"),
            Ok(()),
            "{}",
            here.display()
        );
    }
    let beside = at_or_above_clone(&root, Some(&ws.join("gadget")), "alpha", "widget").unwrap_err();
    assert!(beside.contains("gadget"), "{beside}");
    assert!(at_or_above_clone(&root, Some(&ws.join("notes")), "alpha", "widget").is_err());
    let piece = ws.join(".worktrees/widget/p1");
    let refused = at_or_above_clone(&root, Some(&piece), "alpha", "widget").unwrap_err();
    assert!(refused.contains("widget · p1"), "{refused}");
    assert!(at_or_above_clone(&root, None, "alpha", "widget").is_err());
    // Fails closed on its own: outside the workspace, in another one, or a folder that is
    // not there.
    std::fs::create_dir_all(root.join("workspaces/beta/widget")).unwrap();
    std::fs::create_dir_all(root.join("docs")).unwrap();
    for elsewhere in [
        root.join("workspaces/beta/widget"),
        root.join("docs"),
        root.join("gone"),
        std::env::temp_dir(),
    ] {
        assert!(
            at_or_above_clone(&root, Some(&elsewhere), "alpha", "widget").is_err(),
            "{}",
            elsewhere.display()
        );
    }
    assert!(at_or_above_clone(&root, Some(&ws), "nowhere", "widget").is_err());
}

#[test]
fn a_branch_folder_is_removed_for_a_chat_above_it_and_never_one_in_it() {
    // #3879: from the workspace or the project's top; never from inside the folder itself,
    // which would go from under the chat, nor from another branch folder.
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let pieces = root.join("workspaces/alpha/.worktrees/widget");
    for piece in ["p1", "p2"] {
        std::fs::create_dir_all(pieces.join(piece).join("src")).unwrap();
    }
    let ws = root.join("workspaces/alpha");
    assert_eq!(above(&root, Some(&ws), "widget", "p1"), Ok(()));
    assert_eq!(above(&root, Some(&root), "widget", "p1"), Ok(()));
    for inside in [pieces.join("p1"), pieces.join("p1/src")] {
        let refused = above(&root, Some(&inside), "widget", "p1").unwrap_err();
        assert!(
            refused.contains("would take the folder this chat works in"),
            "{refused}"
        );
    }
    let refused = above(&root, Some(&pieces.join("p2")), "widget", "p1").unwrap_err();
    assert!(refused.contains("widget · p2"), "{refused}");
    assert!(above(&root, None, "widget", "p1").is_err());
    // A place that cannot be read is refused, not taken for one above the folder.
    let gone = above(&root, Some(&ws.join("gone")), "widget", "p1").unwrap_err();
    assert!(gone.contains("could not place"), "{gone}");
}

#[test]
fn a_removal_for_a_chat_in_another_workspace_or_in_the_folder_is_answered_no() {
    let (_dir, root) = tree();
    std::fs::create_dir_all(root.join("workspaces/alpha/.worktrees/widget/p1")).unwrap();
    let remove = |cwd: PathBuf, workspace: &str| {
        answer(
            &root,
            &asker(Some(&cwd)),
            &GitAsk {
                chat: 4,
                workspace: workspace.to_owned(),
                alive: false,
                work: GitWork::WorktreeRemove {
                    repo: "widget".to_owned(),
                    piece: "p1".to_owned(),
                    delete_branch: false,
                },
            },
            &git::Isolated::default(),
            chrono::Utc::now(),
        )
    };
    for (cwd, workspace) in [
        (root.join("workspaces/beta"), "alpha"),
        (root.join("workspaces/alpha/.worktrees/widget/p1"), "alpha"),
    ] {
        let said = remove(cwd.clone(), workspace);
        assert!(
            matches!(said, Answer::No { .. }),
            "{}: {said:?}",
            cwd.display()
        );
    }
    assert!(root.join("workspaces/alpha/.worktrees/widget/p1").is_dir());
}
