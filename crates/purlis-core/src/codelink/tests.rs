// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Code over the link against real repositories: a desktop's clone, and a runner's repository
//! made for it. Each bundle is moved as a file, as the link's code stream carries its bytes.

use std::path::{Path, PathBuf};

use super::*;

fn git(dir: &Path, args: &[&str]) -> String {
    let done = crate::testgit::run(dir, args);
    assert!(done.ok(), "git {args:?} failed: {done:?}");
    done.out.trim().to_string()
}

/// A commit in `dir` with `file` holding `text`, by a test's own identity, never signed.
fn commit(dir: &Path, file: &str, text: &str) -> String {
    std::fs::write(dir.join(file), text).unwrap();
    git(dir, &["add", file]);
    git(
        dir,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            text,
        ],
    );
    git(dir, &["rev-parse", "HEAD"])
}

struct World {
    _dir: tempfile::TempDir,
    top: PathBuf,
    /// The desktop's clone.
    desktop: PathBuf,
    /// The runner's held project, where its repositories are made.
    project: PathBuf,
}

impl World {
    /// A desktop clone on `fix`, cut from `main`, one commit each.
    fn new() -> World {
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path().canonicalize().unwrap();
        let desktop = top.join("desktop/workspaces/app/web");
        std::fs::create_dir_all(&desktop).unwrap();
        git(&desktop, &["init", "-q", "-b", "main", "."]);
        commit(&desktop, "README", "one\n");
        git(&desktop, &["checkout", "-q", "-b", "fix"]);
        commit(&desktop, "fix.txt", "fixed\n");
        let project = top.join("runner/projects/01J9PRJCT00000000000000000");
        std::fs::create_dir_all(&project).unwrap();
        World {
            _dir: dir,
            top,
            desktop,
            project,
        }
    }

    fn runner_repo(&self) -> PathBuf {
        repo_in(&self.project, "workspaces/app/web").unwrap()
    }

    fn scratch(&self) -> PathBuf {
        let scratch = self.top.join("scratch");
        std::fs::create_dir_all(&scratch).unwrap();
        scratch
    }

    fn bundle_path(&self, name: &str) -> PathBuf {
        self.scratch().join(name)
    }

    /// The desktop sends `fix`, with its work tree's uncommitted work, to the runner.
    fn push_fix(&self, have: &[String]) -> Vec<Ref> {
        let into = self.bundle_path("fix.bundle");
        let sent = bundle_branch(
            &self.desktop,
            Some((self.desktop.as_path(), &[][..])),
            "fix",
            have,
            &into,
            &self.scratch(),
        )
        .unwrap();
        let bundle = (sent.bundled == Bundled::Wrote).then_some(into.as_path());
        receive(&self.runner_repo(), bundle, &sent.refs, None).unwrap()
    }
}

fn named<'a>(refs: &'a [Ref], name: &str) -> Option<&'a Ref> {
    refs.iter().find(|one| one.name == name)
}

#[test]
fn a_branch_and_its_uncommitted_work_reach_the_runner_under_the_refs_named() {
    let world = World::new();
    let fix = git(&world.desktop, &["rev-parse", "fix"]);
    // Uncommitted: a changed file, a staged one and a new one git does not track yet.
    std::fs::write(world.desktop.join("fix.txt"), "fixed better\n").unwrap();
    std::fs::write(world.desktop.join("README"), "two\n").unwrap();
    git(&world.desktop, &["add", "README"]);
    std::fs::write(world.desktop.join("new.txt"), "brand new\n").unwrap();
    let status_before = git(&world.desktop, &["status", "--porcelain"]);

    let held = world.push_fix(&[]);

    assert_eq!(
        named(&held, "refs/heads/fix").map(|r| r.oid.as_str()),
        Some(fix.as_str())
    );
    let snapshot = named(&held, "refs/purlis/snapshots/fix").expect("the snapshot is held");
    let runner = world.runner_repo();
    let shown = |path: &str| git(&runner, &["show", &format!("{}:{path}", snapshot.oid)]);
    assert_eq!(shown("fix.txt"), "fixed better");
    assert_eq!(shown("README"), "two");
    assert_eq!(shown("new.txt"), "brand new");
    assert_eq!(
        git(&runner, &["rev-parse", &format!("{}^", snapshot.oid)]),
        fix,
        "the snapshot sits on the branch it is of"
    );
    // The desktop's branch, index and work tree are as they were, and it keeps no snapshot ref.
    assert_eq!(git(&world.desktop, &["rev-parse", "fix"]), fix);
    assert_eq!(
        git(&world.desktop, &["status", "--porcelain"]),
        status_before
    );
    assert_eq!(
        git(&world.desktop, &["for-each-ref", "refs/purlis/"]),
        "",
        "the snapshot's ref is kept only while its bundle is made"
    );
    // Nothing but what was named: no `main`, which the bundle never carried as a ref.
    assert!(named(&held, "refs/heads/main").is_none(), "{held:?}");
}

#[test]
fn a_second_push_sends_only_what_the_runner_lacks_and_none_when_it_lacks_nothing() {
    let world = World::new();
    let first = world.push_fix(&[]);
    let have: Vec<String> = first.iter().map(|one| one.oid.clone()).collect();

    let newer = commit(&world.desktop, "fix.txt", "fixed again\n");
    let held = world.push_fix(&have);
    assert_eq!(named(&held, "refs/heads/fix").unwrap().oid, newer);

    // Nothing new to send: the bundle is empty, and the ref is moved from what the runner holds.
    let have: Vec<String> = held.iter().map(|one| one.oid.clone()).collect();
    let into = world.bundle_path("none.bundle");
    let sent = bundle_branch(&world.desktop, None, "fix", &have, &into, &world.scratch()).unwrap();
    assert_eq!(sent.bundled, Bundled::Nothing);
    assert!(!into.exists());
    let other = Ref {
        name: "refs/heads/also".into(),
        oid: newer.clone(),
    };
    let held = receive(&world.runner_repo(), None, &[other], None).unwrap();
    assert_eq!(named(&held, "refs/heads/also").unwrap().oid, newer);
}

#[test]
fn a_bundle_that_carries_a_ref_it_did_not_name_is_refused_and_nothing_moves() {
    let world = World::new();
    let into = world.bundle_path("both.bundle");
    assert_eq!(
        bundle(
            &world.desktop,
            &["refs/heads/fix", "refs/heads/main"],
            &[],
            &into
        )
        .unwrap(),
        Bundled::Wrote
    );
    let fix = Ref {
        name: "refs/heads/fix".into(),
        oid: git(&world.desktop, &["rev-parse", "fix"]),
    };
    let refused = receive(&world.runner_repo(), Some(&into), &[fix], None).unwrap_err();
    assert!(
        matches!(&refused, Refused::NotAllowed(why) if why.contains("refs/heads/main")),
        "{refused:?}"
    );
    assert_eq!(held_refs(&world.runner_repo()).unwrap(), Vec::new());
}

#[test]
fn a_bundle_naming_a_ref_at_another_commit_than_the_bundle_carries_is_refused() {
    let world = World::new();
    let into = world.bundle_path("fix.bundle");
    bundle(&world.desktop, &["refs/heads/fix"], &[], &into).unwrap();
    let wrong = Ref {
        name: "refs/heads/fix".into(),
        oid: git(&world.desktop, &["rev-parse", "main"]),
    };
    assert!(matches!(
        receive(&world.runner_repo(), Some(&into), &[wrong], None),
        Err(Refused::NotAllowed(_))
    ));
    assert_eq!(held_refs(&world.runner_repo()).unwrap(), Vec::new());
}

#[test]
fn only_branches_and_their_snapshots_are_carried_under_names_git_takes() {
    for fine in [
        "refs/heads/fix",
        "refs/heads/team/fix-2",
        "refs/purlis/snapshots/fix",
    ] {
        assert_eq!(carried(fine), Ok(()), "{fine}");
    }
    for refused in [
        "HEAD",
        "refs/heads/",
        "refs/tags/v1",
        "refs/remotes/origin/main",
        "refs/purlis/other/fix",
        "refs/heads/../config",
        "refs/heads/a..b",
        "refs/heads/fix.lock",
        "refs/heads/.hidden",
        "refs/heads/a b",
        "refs/heads/a~1",
        "refs/heads/a:b",
        "refs/heads/a//b",
        "refs/heads/a/",
        "refs/heads/@{-1}",
        "refs/heads/purlis/held",
        "refs/heads/a\u{7f}",
    ] {
        assert!(
            matches!(carried(refused), Err(Refused::NotAllowed(_))),
            "{refused}"
        );
    }
}

#[test]
fn a_ref_whose_commit_is_not_a_full_name_is_refused() {
    let world = World::new();
    let short = Ref {
        name: "refs/heads/fix".into(),
        oid: "HEAD".into(),
    };
    assert!(matches!(
        receive(&world.runner_repo(), None, &[short], None),
        Err(Refused::NotAllowed(_))
    ));
}

#[test]
fn a_branch_checked_out_on_the_runner_is_never_moved_under_it() {
    let world = World::new();
    world.push_fix(&[]);
    let runner = world.runner_repo();
    let tree = world.project.join("workspaces/app/.worktrees/web/fix");
    git(
        &runner,
        &["worktree", "add", "-q", tree.to_str().unwrap(), "fix"],
    );
    let at_first = git(&runner, &["rev-parse", "fix"]);
    // Sent again, with a newer commit, by bundle and by a commit the runner holds.
    let have: Vec<String> = held_refs(&runner)
        .unwrap()
        .into_iter()
        .map(|one| one.oid)
        .collect();
    commit(&world.desktop, "fix.txt", "newer here\n");
    world.push_fix(&have);
    let main = Ref {
        name: "refs/heads/fix".into(),
        oid: git(&world.desktop, &["rev-parse", "main"]),
    };
    receive(&runner, None, &[main], None).unwrap();
    assert_eq!(
        git(&runner, &["rev-parse", "fix"]),
        at_first,
        "the runner chat's branch is its own"
    );
}

#[test]
fn no_hook_of_the_runners_repository_runs_as_it_takes_a_bundle() {
    let world = World::new();
    world.push_fix(&[]);
    let runner = world.runner_repo();
    let mark = world.top.join("hook-ran");
    let hooks = world.top.join("planted-hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    for hook in [
        "reference-transaction",
        "post-checkout",
        "post-merge",
        "pre-auto-gc",
    ] {
        let script = hooks.join(hook);
        std::fs::write(&script, format!("#!/bin/sh\ntouch '{}'\n", mark.display())).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    git(
        &runner,
        &["config", "core.hooksPath", hooks.to_str().unwrap()],
    );
    let held = held_refs(&runner).unwrap();
    let have: Vec<String> = held.iter().map(|one| one.oid.clone()).collect();
    commit(&world.desktop, "fix.txt", "more\n");
    world.push_fix(&have);
    assert!(!mark.exists(), "a hook of the runner's repository ran");
}

#[test]
fn the_desktop_takes_a_runners_branches_back_under_its_pool_and_touches_nothing_of_its_own() {
    let world = World::new();
    world.push_fix(&[]);
    let runner = world.runner_repo();
    // The runner's chat commits on `fix`, in a work tree of its own there.
    let tree = world.project.join("workspaces/app/.worktrees/web/fix");
    git(
        &runner,
        &["worktree", "add", "-q", tree.to_str().unwrap(), "fix"],
    );
    let on_the_runner = commit(&tree, "fix.txt", "fixed on the runner\n");

    // The desktop: a hook planted where its fetch would run one, and work of its own.
    let mark = world.top.join("hook-ran");
    let hooks = world.top.join("desktop-hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let script = hooks.join("reference-transaction");
    std::fs::write(&script, format!("#!/bin/sh\ntouch '{}'\n", mark.display())).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    git(
        &world.desktop,
        &["config", "core.hooksPath", hooks.to_str().unwrap()],
    );
    std::fs::write(world.desktop.join("fix.txt"), "the person's own work\n").unwrap();
    let fix_here = git(&world.desktop, &["rev-parse", "fix"]);
    let status_here = git(&world.desktop, &["status", "--porcelain"]);

    let have = vec![fix_here.clone()];
    let into = world.bundle_path("back.bundle");
    let (heads, bundled) = give(&runner, &have, &into).unwrap();
    assert_eq!(bundled, Bundled::Wrote);
    let kept = take_back(&world.desktop, "builds", Some(&into), &heads).unwrap();

    assert_eq!(
        named(&kept, "refs/remotes/builds/fix").map(|r| r.oid.as_str()),
        Some(on_the_runner.as_str())
    );
    assert_eq!(git(&world.desktop, &["rev-parse", "fix"]), fix_here);
    assert_eq!(git(&world.desktop, &["status", "--porcelain"]), status_here);
    assert_eq!(
        std::fs::read_to_string(world.desktop.join("fix.txt")).unwrap(),
        "the person's own work\n"
    );
    assert!(!mark.exists(), "a hook of the desktop's repository ran");

    // A branch the runner no longer has goes from the pool's refs, and only from there.
    let kept = take_back(&world.desktop, "builds", None, &[]).unwrap();
    assert_eq!(kept, Vec::new());
    assert_eq!(git(&world.desktop, &["rev-parse", "fix"]), fix_here);
}

#[test]
fn a_bundle_from_the_runner_that_carries_a_ref_it_did_not_name_is_refused() {
    let world = World::new();
    let into = world.bundle_path("sneaky.bundle");
    bundle(
        &world.desktop,
        &["refs/heads/fix", "refs/heads/main"],
        &[],
        &into,
    )
    .unwrap();
    let fix = Ref {
        name: "refs/heads/fix".into(),
        oid: git(&world.desktop, &["rev-parse", "fix"]),
    };
    assert!(matches!(
        take_back(&world.desktop, "builds", Some(&into), &[fix]),
        Err(Refused::NotAllowed(_))
    ));
    assert_eq!(git(&world.desktop, &["for-each-ref", "refs/remotes/"]), "");
}

#[test]
fn a_runner_naming_anything_but_a_branch_is_refused_by_the_desktop() {
    let world = World::new();
    let main = git(&world.desktop, &["rev-parse", "main"]);
    for name in [
        "refs/purlis/snapshots/fix",
        "refs/tags/v1",
        "HEAD",
        "refs/heads/../x",
    ] {
        let one = Ref {
            name: name.into(),
            oid: main.clone(),
        };
        assert!(
            matches!(
                take_back(&world.desktop, "builds", None, &[one]),
                Err(Refused::NotAllowed(_))
            ),
            "{name}"
        );
    }
    for pool in ["", "../x", "a b", "a..b", ".hidden"] {
        assert!(
            matches!(
                take_back(&world.desktop, pool, None, &[]),
                Err(Refused::NotAllowed(_))
            ),
            "{pool:?}"
        );
    }
    assert_eq!(git(&world.desktop, &["for-each-ref", "refs/remotes/"]), "");
}

#[test]
fn a_repositorys_folder_is_the_projects_own_or_a_workspaces_repo_reached_through_no_link() {
    let world = World::new();
    assert_eq!(repo_in(&world.project, "").unwrap(), world.project);
    assert_eq!(
        repo_in(&world.project, "workspaces/app/web").unwrap(),
        world.project.join("workspaces/app/web")
    );
    for refused in [
        "..",
        "../other",
        "/etc",
        "workspaces",
        "workspaces/app",
        "workspaces/app/web/.git",
        "workspaces/../web",
        "workspaces/app/..",
        "personas/steward/web",
        "workspaces/.hidden/web",
    ] {
        assert!(
            matches!(
                repo_in(&world.project, refused),
                Err(Refused::NotAllowed(_))
            ),
            "{refused}"
        );
    }
    // A link a chat put in the project's folders sends nothing elsewhere.
    let elsewhere = world.top.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::create_dir_all(world.project.join("workspaces")).unwrap();
    std::os::unix::fs::symlink(&elsewhere, world.project.join("workspaces/linked")).unwrap();
    assert!(matches!(
        repo_in(&world.project, "workspaces/linked/web"),
        Err(Refused::NotAllowed(_))
    ));
}

/// **Where each clone a runner holds came from** (1.54, #3985): its own `origin`, as its git
/// keeps it, only where that is a forge's HTTPS or SSH address with no sign-in in it. Never a
/// local path, another transport or a token, nothing an included file names, and nothing for a
/// name that is not a clone of the workspace.
#[test]
fn a_runner_names_each_clones_origin_only_where_it_is_a_forges_address_with_no_sign_in() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().canonicalize().unwrap().join("project");
    let workspace = project.join("workspaces/app");
    let origins = [
        ("api", Some("https://github.com/owner/api.git")),
        ("web", Some("git@github.com:owner/web.git")),
        ("local", Some("/srv/git/local.git")),
        ("file", Some("file:///srv/git/file.git")),
        ("ext", Some("ext::sh -c touch% /tmp/ran")),
        (
            "token",
            Some(concat!(
                "https://",
                "a-token",
                "@",
                "github.com/owner/token.git"
            )),
        ),
        ("plain", Some("http://example.com/owner/plain.git")),
        ("none", None),
        ("included", None),
    ];
    for (repo, origin) in origins {
        let clone = workspace.join(repo);
        std::fs::create_dir_all(&clone).unwrap();
        git(&clone, &["init", "-q", "-b", "main"]);
        commit(&clone, "README", repo);
        if let Some(url) = origin {
            git(&clone, &["config", "remote.origin.url", url]);
        }
    }
    let included = workspace.join("included");
    std::fs::write(
        dir.path().join("origin.inc"),
        "[remote \"origin\"]\n\turl = https://github.com/owner/included.git\n",
    )
    .unwrap();
    git(
        &included,
        &[
            "config",
            "include.path",
            &dir.path().join("origin.inc").display().to_string(),
        ],
    );

    let mut asked: Vec<String> = origins.iter().map(|(repo, _)| (*repo).to_owned()).collect();
    asked.push("../outside".to_owned());
    asked.push("memory".to_owned());
    assert_eq!(
        origins_in(&project, "app", &asked),
        std::collections::BTreeMap::from([
            (
                "api".to_owned(),
                "https://github.com/owner/api.git".to_owned()
            ),
            ("web".to_owned(), "git@github.com:owner/web.git".to_owned()),
        ])
    );
    assert!(origins_in(&project, "..", &asked).is_empty());
}

/// **A clone made fresh from a runner's work is given the runner's `origin`** (1.54, #3985),
/// held to the rules a URL pasted here is held to: a forge's HTTPS or SSH address with no
/// sign-in in it. Anything else, a pool's branches already kept under `origin`, or a remote
/// already there, leaves the clone with none, in a sentence.
#[test]
fn a_fresh_clone_is_given_the_runners_origin_only_where_a_pasted_url_would_be_taken() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("api");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    commit(&repo, "README", "api");

    for refused in [
        "ext::sh -c touch% /tmp/ran",
        "file:///srv/git/api.git",
        "/srv/git/api.git",
        "./api",
        "http://example.com/owner/api.git",
        concat!("https://", "a-token", "@", "github.com/owner/api.git"),
        concat!("https://", "user:pass", "@", "github.com/owner/api.git"),
        "-oProxyCommand=sh:owner/api",
        "ssh://-oProxyCommand=sh/owner/api",
        concat!("ssh://", "-oProxyCommand=sh", "@", "github.com/owner/api"),
        "https://github.com/owner/api.git\n[core]\n\tfsmonitor = sh",
        "",
    ] {
        let given = give_origin(&repo, "builds", refused);
        assert!(given.is_err(), "{refused:?}: {given:?}");
        assert_eq!(git(&repo, &["remote"]), "", "{refused:?}");
    }
    let long = format!("https://github.com/owner/{}.git", "a".repeat(4096));
    assert!(give_origin(&repo, "builds", &long).is_err());

    assert!(
        give_origin(&repo, "Origin", "https://github.com/owner/api.git").is_err(),
        "a pool named origin keeps its branches under refs/remotes/origin/"
    );
    assert_eq!(git(&repo, &["remote"]), "");

    give_origin(&repo, "builds", "git@github.com:owner/api.git").expect("given");
    assert_eq!(
        git(&repo, &["remote", "get-url", "origin"]),
        "git@github.com:owner/api.git"
    );
    assert!(
        give_origin(&repo, "builds", "https://github.com/other/api.git").is_err(),
        "a remote already there is left as it is"
    );
    assert_eq!(
        git(&repo, &["remote", "get-url", "origin"]),
        "git@github.com:owner/api.git"
    );
}

/// **The clones a runner holds in a workspace** (1.51, #3811): each repository's folder in it,
/// by its name, and nothing reached through a link, nothing that is not a repository, no name a
/// repository's folder may not have and none the workspace keeps for its own.
#[test]
fn a_runner_names_the_clones_it_holds_in_a_workspace_and_none_through_a_link() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().canonicalize().unwrap().join("project");
    let workspace = project.join("workspaces/app");
    assert_eq!(clones_in(&project, "app").unwrap(), Vec::<String>::new());
    for repo in [
        "web", "api", "notes", ".hidden", "memory", "Todos", "unborn", "detached",
    ] {
        std::fs::create_dir_all(workspace.join(repo)).unwrap();
    }
    // A repository at a name the workspace keeps for its own is never a clone, nor one whose
    // work a move cannot give: on no branch, or with no commit yet.
    for repo in [
        "web", "api", ".hidden", "memory", "Todos", "unborn", "detached",
    ] {
        git(&workspace.join(repo), &["init", "-q", "-b", "main"]);
    }
    for repo in ["web", "api", ".hidden", "memory", "Todos", "detached"] {
        commit(&workspace.join(repo), "README", repo);
    }
    git(&workspace.join("detached"), &["checkout", "-q", "--detach"]);
    std::fs::write(workspace.join("README"), "a file\n").unwrap();
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir_all(elsewhere.join(".git")).unwrap();
    std::os::unix::fs::symlink(&elsewhere, workspace.join("linked")).unwrap();

    assert_eq!(clones_in(&project, "app").unwrap(), ["api", "web"]);
    for refused in ["..", "", "app/web", ".hidden"] {
        assert!(
            matches!(clones_in(&project, refused), Err(Refused::NotAllowed(_))),
            "{refused:?}"
        );
    }

    // A workspace reached through a link is not listed.
    std::fs::create_dir_all(elsewhere.join("web")).unwrap();
    git(&elsewhere.join("web"), &["init", "-q", "-b", "main"]);
    std::os::unix::fs::symlink(&elsewhere, project.join("workspaces/other")).unwrap();
    assert!(matches!(
        clones_in(&project, "other"),
        Err(Refused::NotAllowed(_))
    ));
}

#[test]
fn no_filter_the_work_tree_names_runs_on_its_snapshot() {
    let world = World::new();
    let mark = world.top.join("filter-ran");
    std::fs::write(world.desktop.join(".gitattributes"), "*.txt filter=mark\n").unwrap();
    git(
        &world.desktop,
        &[
            "config",
            "filter.mark.clean",
            &format!("sh -c 'touch {}; cat'", mark.display()),
        ],
    );
    std::fs::write(world.desktop.join("fix.txt"), "dirty\n").unwrap();
    let snapshot = snapshot(&world.desktop, &world.scratch(), &[])
        .unwrap()
        .expect("dirty");
    assert!(!mark.exists(), "a clean filter ran on the snapshot");
    assert_eq!(
        git(&world.desktop, &["show", &format!("{snapshot}:fix.txt")]),
        "dirty"
    );
}

#[test]
fn a_clean_work_tree_has_no_snapshot() {
    let world = World::new();
    assert_eq!(
        snapshot(&world.desktop, &world.scratch(), &[]).unwrap(),
        None
    );
}

#[test]
fn a_fresh_repository_is_checked_out_at_the_branch_with_its_uncommitted_work_and_never_again() {
    let world = World::new();
    std::fs::write(world.desktop.join("fix.txt"), "fixed better\n").unwrap();
    std::fs::write(world.desktop.join("new.txt"), "brand new\n").unwrap();
    std::fs::remove_file(world.desktop.join("README")).unwrap();
    let fix = git(&world.desktop, &["rev-parse", "fix"]);
    let into = world.bundle_path("fix.bundle");
    let sent = bundle_branch(
        &world.desktop,
        Some((world.desktop.as_path(), &[][..])),
        "fix",
        &[],
        &into,
        &world.scratch(),
    )
    .unwrap();
    let runner = world.runner_repo();
    receive(&runner, Some(&into), &sent.refs, Some("refs/heads/fix")).unwrap();

    assert_eq!(git(&runner, &["rev-parse", "HEAD"]), fix);
    assert_eq!(
        git(&runner, &["symbolic-ref", "-q", "HEAD"]),
        "refs/heads/fix"
    );
    let read = |file: &str| std::fs::read_to_string(runner.join(file)).ok();
    assert_eq!(read("fix.txt").as_deref(), Some("fixed better\n"));
    assert_eq!(read("new.txt").as_deref(), Some("brand new\n"));
    assert_eq!(read("README"), None);
    // Read untrimmed: a line's first column says whether the change is staged.
    let status = crate::testgit::run(&runner, &["status", "--porcelain"]).out;
    assert!(status.contains(" M fix.txt"), "{status}");
    assert!(status.contains("?? new.txt"), "{status}");
    assert!(status.contains(" D README"), "{status}");

    // A runner chat's own work there is never checked out over.
    std::fs::write(runner.join("fix.txt"), "the runner chat's work\n").unwrap();
    let newer = commit(&world.desktop, "other.txt", "later\n");
    let have: Vec<String> = sent.refs.iter().map(|one| one.oid.clone()).collect();
    let into = world.bundle_path("later.bundle");
    let later = bundle_branch(&world.desktop, None, "fix", &have, &into, &world.scratch()).unwrap();
    let bundle = (later.bundled == Bundled::Wrote).then_some(into.as_path());
    let held = receive(&runner, bundle, &later.refs, Some("refs/heads/fix")).unwrap();
    assert_eq!(
        named(&held, "refs/heads/fix").unwrap().oid,
        fix,
        "the branch the runner has checked out is its own, and {newer} was not taken under it"
    );
    assert_eq!(read("fix.txt").as_deref(), Some("the runner chat's work\n"));
    assert_eq!(git(&runner, &["rev-parse", "HEAD"]), fix);
}

#[test]
fn a_checkout_naming_a_ref_the_transfer_does_not_carry_is_refused_and_nothing_moves() {
    let world = World::new();
    let into = world.bundle_path("fix.bundle");
    let sent = bundle_branch(&world.desktop, None, "fix", &[], &into, &world.scratch()).unwrap();
    for checkout in ["refs/heads/main", "refs/purlis/snapshots/fix", "HEAD"] {
        assert!(
            matches!(
                receive(
                    &world.runner_repo(),
                    Some(&into),
                    &sent.refs,
                    Some(checkout)
                ),
                Err(Refused::NotAllowed(_))
            ),
            "{checkout}"
        );
    }
    assert_eq!(
        held_refs(&world.runner_repo()).unwrap_or_default(),
        Vec::new()
    );
}

#[test]
fn a_snapshot_of_a_project_carries_none_of_its_state_or_local_settings() {
    let world = World::new();
    let left = plane_leaves_out();
    assert!(left.iter().any(|name| name == ".purlis"), "{left:?}");
    std::fs::create_dir_all(world.desktop.join(".purlis/vaults")).unwrap();
    std::fs::write(
        world.desktop.join(".purlis/vaults/forge.json"),
        "{\"token\":\"x\"}",
    )
    .unwrap();
    std::fs::write(world.desktop.join("purlis.local.toml"), "[sandbox]\n").unwrap();
    // Nothing but what is left out: no snapshot at all.
    assert_eq!(
        snapshot(&world.desktop, &world.scratch(), &left).unwrap(),
        None
    );

    std::fs::write(world.desktop.join("fix.txt"), "a memory\n").unwrap();
    let snapshot = snapshot(&world.desktop, &world.scratch(), &left)
        .unwrap()
        .expect("the change is carried");
    let listed = git(&world.desktop, &["ls-tree", "-r", "--name-only", &snapshot]);
    assert!(listed.contains("fix.txt"), "{listed}");
    assert!(!listed.contains(".purlis"), "{listed}");
    assert!(!listed.contains("purlis.local.toml"), "{listed}");
}

/// The window's two ends over the link, as the link would carry them: `take_code` into the
/// runner's repository, and `give_code` back from it.
fn sent_to(runner: &Path) -> impl FnOnce(Sending<'_>) -> Result<Vec<Ref>, String> + '_ {
    move |sending| {
        receive(
            runner,
            sending.bundle,
            &sending.refs,
            Some(&sending.checkout),
        )
        .map_err(|why| why.to_string())
    }
}

#[test]
fn a_clone_is_sent_on_its_branch_with_its_work_and_its_runner_commits_come_back() {
    let world = World::new();
    std::fs::write(world.desktop.join("fix.txt"), "half done\n").unwrap();
    let runner = world.runner_repo();

    let (held, left_out) =
        send(&world.desktop, &[], &[], &world.scratch(), sent_to(&runner)).unwrap();
    assert_eq!(left_out, Vec::<String>::new());
    assert!(named(&held, "refs/heads/fix").is_some());
    assert_eq!(
        std::fs::read_to_string(runner.join("fix.txt")).unwrap(),
        "half done\n",
        "the runner's work tree has the uncommitted work"
    );

    // The runner's chat commits; the desktop fetches it back.
    let on_the_runner = commit(&runner, "fix.txt", "done on the runner\n");
    let scratch = world.scratch();
    let kept = fetch_back(&world.desktop, "builds", &scratch, |have, into| {
        give(&runner, &have, into)
            .map(|(heads, bundled)| (heads, bundled == Bundled::Wrote))
            .map_err(|why| why.to_string())
    })
    .unwrap();
    assert!(
        kept.iter()
            .any(|one| one.name == "refs/remotes/builds/fix" && one.oid == on_the_runner),
        "{kept:?}"
    );
    assert_eq!(
        std::fs::read_to_string(world.desktop.join("fix.txt")).unwrap(),
        "half done\n",
        "the desktop's own work is untouched"
    );
    // What Changes shows of it: the runner's branch, one commit ahead of the branch here.
    assert_eq!(
        on_a_runner(&world.desktop, "builds"),
        vec![OnARunner {
            branch: "fix".into(),
            oid: on_the_runner,
            ahead: 1,
        }]
    );
    assert_eq!(on_a_runner(&world.desktop, "other"), Vec::new());
}

#[test]
fn a_folder_that_is_not_a_repository_sends_and_fetches_nothing_and_one_on_no_branch_says_so() {
    let world = World::new();
    let plain = world.top.join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    assert_eq!(
        send(&plain, &[], &[], &world.scratch(), |_| panic!(
            "nothing is sent"
        ))
        .unwrap(),
        (Vec::new(), Vec::new())
    );
    assert_eq!(
        fetch_back(&plain, "builds", &world.scratch(), |_, _| panic!(
            "nothing is fetched"
        ))
        .unwrap(),
        Vec::new()
    );
    let main = git(&world.desktop, &["rev-parse", "main"]);
    git(&world.desktop, &["checkout", "-q", "--detach", &main]);
    assert!(matches!(
        send(&world.desktop, &[], &[], &world.scratch(), |_| panic!(
            "nothing is sent"
        )),
        Err(Refused::NotAllowed(_))
    ));
}

#[test]
fn a_project_that_ignores_its_state_folder_is_still_snapshotted() {
    let world = World::new();
    std::fs::write(world.desktop.join(".gitignore"), "/.purlis/\n").unwrap();
    git(&world.desktop, &["add", ".gitignore"]);
    git(
        &world.desktop,
        &[
            "-c",
            "user.name=T",
            "-c",
            "user.email=t@example.invalid",
            "commit",
            "-q",
            "-m",
            "ignore",
        ],
    );
    std::fs::create_dir_all(world.desktop.join(".purlis")).unwrap();
    std::fs::write(world.desktop.join(".purlis/this-machine.json"), "{}").unwrap();
    std::fs::write(world.desktop.join("fix.txt"), "a memory\n").unwrap();

    let snapshot = snapshot(&world.desktop, &world.scratch(), &plane_leaves_out())
        .expect("staged, whatever the project ignores")
        .expect("the change is carried");
    let listed = git(&world.desktop, &["ls-tree", "-r", "--name-only", &snapshot]);
    assert!(listed.contains("fix.txt"), "{listed}");
    assert!(!listed.contains(".purlis"), "{listed}");
}

#[test]
fn a_pool_named_as_a_remote_of_the_repository_is_refused_and_its_refs_are_left_alone() {
    let world = World::new();
    git(
        &world.desktop,
        &[
            "config",
            "remote.builds.url",
            "https://example.invalid/acme/web.git",
        ],
    );
    git(
        &world.desktop,
        &["update-ref", "refs/remotes/builds/main", "refs/heads/main"],
    );
    let main = git(&world.desktop, &["rev-parse", "main"]);
    let refused = take_back(&world.desktop, "builds", None, &[]).unwrap_err();
    assert!(
        matches!(&refused, Refused::NotAllowed(why) if why.contains("remote named builds")),
        "{refused:?}"
    );
    assert_eq!(
        git(&world.desktop, &["rev-parse", "refs/remotes/builds/main"]),
        main
    );
}

#[test]
fn a_bundle_carrying_a_malformed_object_is_refused_and_nothing_moves() {
    let world = World::new();
    let empty = git(
        &world.desktop,
        &["hash-object", "-w", "-t", "tree", "/dev/null"],
    );
    let bad = world.top.join("bad-commit");
    // No committer, and an author with no address: what fsck refuses.
    std::fs::write(
        &bad,
        format!("tree {empty}\nauthor nobody\n\na bad commit\n"),
    )
    .unwrap();
    let oid = git(
        &world.desktop,
        &[
            "hash-object",
            "-t",
            "commit",
            "-w",
            "--literally",
            bad.to_str().unwrap(),
        ],
    );
    git(&world.desktop, &["update-ref", "refs/heads/bad", &oid]);
    let into = world.bundle_path("bad.bundle");
    bundle(&world.desktop, &["refs/heads/bad"], &[], &into).unwrap();
    let refused = receive(
        &world.runner_repo(),
        Some(&into),
        &[Ref {
            name: "refs/heads/bad".into(),
            oid,
        }],
        None,
    );
    assert!(matches!(refused, Err(Refused::Git(_))), "{refused:?}");
    assert_eq!(held_refs(&world.runner_repo()).unwrap(), Vec::new());
}

#[test]
fn a_fetch_made_as_a_bundle_is_read_reaches_no_transport_but_a_file() {
    let world = World::new();
    for url in [
        "https://example.invalid/acme/web.git",
        "ssh://example.invalid/acme/web.git",
        "git://example.invalid/acme/web.git",
    ] {
        let mut args: Vec<&str> = FETCHES_WITH.to_vec();
        args.extend([
            "fetch",
            "--quiet",
            url,
            "refs/heads/main:refs/heads/elsewhere",
        ]);
        // Through purlis's network entry, as a fetch that names another repository is run: git
        // itself refuses every transport but the file one a bundle is read through.
        let ran = git::run_network(&world.desktop, None, &args).unwrap();
        assert!(!ran.ok(), "{url} was reached: {ran:?}");
        assert!(ran.err.contains("not allowed"), "{url}: {}", ran.err);
    }
}

#[test]
fn a_snapshot_carries_no_credential_file_and_no_file_with_another_link() {
    let world = World::new();
    std::fs::write(world.desktop.join(".env"), "TOKEN=x\n").unwrap();
    std::fs::write(world.desktop.join(".env.local"), "TOKEN=y\n").unwrap();
    std::fs::write(world.desktop.join("id_ed25519"), "-----BEGIN-----\n").unwrap();
    std::fs::write(world.desktop.join("server.pem"), "-----BEGIN-----\n").unwrap();
    let outside = world.top.join("outside.txt");
    std::fs::write(&outside, "not the tree's\n").unwrap();
    std::fs::hard_link(&outside, world.desktop.join("linked.txt")).unwrap();
    std::fs::write(world.desktop.join(".env.example"), "TOKEN=\n").unwrap();
    std::fs::write(world.desktop.join("fix.txt"), "the work\n").unwrap();

    let snapshot = snapshot(&world.desktop, &world.scratch(), &[])
        .unwrap()
        .expect("the work is carried");
    let listed: Vec<String> = git(&world.desktop, &["ls-tree", "-r", "--name-only", &snapshot])
        .lines()
        .map(str::to_owned)
        .collect();
    for left in [
        ".env",
        ".env.local",
        "id_ed25519",
        "server.pem",
        "linked.txt",
    ] {
        assert!(
            !listed.iter().any(|one| one == left),
            "{left} went: {listed:?}"
        );
    }
    assert!(listed.iter().any(|one| one == ".env.example"), "{listed:?}");
    assert_eq!(
        git(&world.desktop, &["show", &format!("{snapshot}:fix.txt")]),
        "the work"
    );
}

#[test]
fn a_committed_credential_file_changed_here_goes_as_it_is_committed() {
    let world = World::new();
    commit(&world.desktop, ".env", "TOKEN=committed\n");
    std::fs::write(world.desktop.join(".env"), "TOKEN=changed\n").unwrap();
    std::fs::write(world.desktop.join("fix.txt"), "the work\n").unwrap();
    let snapshot = snapshot(&world.desktop, &world.scratch(), &[])
        .unwrap()
        .expect("the work is carried");
    assert_eq!(
        git(&world.desktop, &["show", &format!("{snapshot}:.env")]),
        "TOKEN=committed"
    );
}

#[test]
fn a_hard_link_whose_name_spells_pathspec_magic_is_left_out_all_the_same() {
    let world = World::new();
    let outside = world.top.join("outside.txt");
    std::fs::write(&outside, "not the tree's\n").unwrap();
    std::fs::hard_link(&outside, world.desktop.join(":linked")).unwrap();
    std::fs::write(world.desktop.join("fix.txt"), "the work\n").unwrap();

    let mut left = Vec::new();
    let snapshot = snapshot_leaving(&world.desktop, &world.scratch(), &[], &mut left)
        .unwrap()
        .expect("the work is carried");
    let listed = git(&world.desktop, &["ls-tree", "-r", "--name-only", &snapshot]);
    assert!(!listed.lines().any(|one| one == ":linked"), "{listed}");
    assert_eq!(left, vec![":linked".to_owned()]);
}

#[test]
fn a_snapshot_leaves_out_what_a_save_to_the_forge_refuses_and_says_which() {
    let world = World::new();
    // A token by its forge's own prefix, spelled in two halves so no scanner here reads one.
    let token = format!("ghp_{}", "A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8");
    std::fs::write(
        world.desktop.join(".npmrc"),
        "//registry.example.invalid/:_authToken=abc123\n",
    )
    .unwrap();
    std::fs::write(world.desktop.join("notes.txt"), format!("use {token}\n")).unwrap();
    std::fs::write(world.desktop.join("plain.txt"), "nothing secret\n").unwrap();

    let mut left = Vec::new();
    let snapshot = snapshot_leaving(&world.desktop, &world.scratch(), &[], &mut left)
        .unwrap()
        .expect("the work is carried");
    let listed = git(&world.desktop, &["ls-tree", "-r", "--name-only", &snapshot]);
    for gone in [".npmrc", "notes.txt"] {
        assert!(!listed.lines().any(|one| one == gone), "{gone}: {listed}");
    }
    assert!(listed.lines().any(|one| one == "plain.txt"), "{listed}");
    left.sort();
    assert_eq!(left, vec![".npmrc".to_owned(), "notes.txt".to_owned()]);
}

#[test]
fn a_remote_that_differs_from_the_pool_in_case_alone_is_refused_too() {
    let world = World::new();
    git(
        &world.desktop,
        &[
            "config",
            "remote.Builds.url",
            "https://example.invalid/acme/web.git",
        ],
    );
    let refused = take_back(&world.desktop, "builds", None, &[]).unwrap_err();
    assert!(
        matches!(&refused, Refused::NotAllowed(why) if why.contains("remote named builds")),
        "{refused:?}"
    );
}

#[test]
fn a_file_that_vanished_from_disk_after_it_was_staged_is_left_out_of_the_send() {
    let world = World::new();
    std::fs::write(world.desktop.join("gone.txt"), "here a moment\n").unwrap();
    std::fs::write(world.desktop.join("fix.txt"), "the work\n").unwrap();
    let index = world.scratch().join("vanished.index");
    for args in [
        &["read-tree", "HEAD"][..],
        &["add", "--", "gone.txt", "fix.txt"][..],
    ] {
        let ran = git::run_with_index(&world.desktop, &index, args).unwrap();
        assert!(ran.ok(), "{args:?}: {ran:?}");
    }
    // Staged, then gone before it is looked at: what it was can no longer be told.
    std::fs::remove_file(world.desktop.join("gone.txt")).unwrap();

    let left = keep_secrets_out(&world.desktop, &index).unwrap();

    assert_eq!(left, vec!["gone.txt".to_owned()]);
    let staged = git::run_with_index(&world.desktop, &index, &["ls-files"]).unwrap();
    assert!(
        !staged.out.lines().any(|one| one == "gone.txt"),
        "{}",
        staged.out
    );
    assert!(
        staged.out.lines().any(|one| one == "fix.txt"),
        "{}",
        staged.out
    );
}

#[test]
fn a_name_left_out_is_matched_without_regard_to_case_where_the_repository_ignores_case() {
    let world = World::new();
    git(&world.desktop, &["config", "core.ignorecase", "true"]);
    std::fs::create_dir_all(world.desktop.join("secrets")).unwrap();
    std::fs::write(world.desktop.join("secrets/plan.txt"), "mine alone\n").unwrap();
    std::fs::write(world.desktop.join("fix.txt"), "the work\n").unwrap();

    let snapshot = snapshot(&world.desktop, &world.scratch(), &["Secrets".to_owned()])
        .unwrap()
        .expect("the work is carried");
    let listed = git(&world.desktop, &["ls-tree", "-r", "--name-only", &snapshot]);
    assert!(!listed.contains("secrets/"), "{listed}");
    assert!(listed.lines().any(|one| one == "fix.txt"), "{listed}");
}

#[test]
fn a_tracked_file_the_repository_ignores_is_still_left_out_by_name() {
    let world = World::new();
    std::fs::create_dir_all(world.desktop.join("config")).unwrap();
    commit(&world.desktop, "config/local.yml", "committed: yes\n");
    commit(&world.desktop, ".gitignore", "config/local.yml\n");
    std::fs::write(
        world.desktop.join("config/local.yml"),
        "changed here: yes\n",
    )
    .unwrap();

    let snapshot = snapshot(
        &world.desktop,
        &world.scratch(),
        &["config/local.yml".to_owned()],
    )
    .unwrap();
    // Its change stays here: the snapshot carries nothing else, so there is none.
    assert_eq!(snapshot, None);
}
