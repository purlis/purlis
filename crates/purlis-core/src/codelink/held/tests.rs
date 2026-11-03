// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a runner holds, against real repositories, and the proof that a desktop has it (#4569).
//! Each case a review found a task's work could be missed in is here, and each ends with the
//! runner kept: [`not_home`] says what the desktop lacks.

use std::path::{Path, PathBuf};

use super::*;

const PROJECT: &str = "01J9PRJCT00000000000000000";

fn git(dir: &Path, args: &[&str]) -> String {
    let done = crate::testgit::run(dir, args);
    assert!(done.ok(), "git {args:?} failed: {done:?}");
    done.out.trim().to_string()
}

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

fn init(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main", "."]);
}

fn path(dir: &Path) -> &str {
    dir.to_str().unwrap()
}

/// A runner keeping one project, and a desktop's copy of that project.
struct World {
    _dir: tempfile::TempDir,
    /// Where the runner keeps its projects.
    projects: PathBuf,
    /// The runner's copy of the project.
    runner: PathBuf,
    /// The desktop's copy of the project.
    desktop: PathBuf,
}

impl World {
    /// The project's own repository on both ends, the desktop's a clone of the runner's, and a
    /// clone `workspaces/app/web` the same way: everything the runner holds is home.
    fn new() -> World {
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path().canonicalize().unwrap();
        let projects = top.join("runner/projects");
        let runner = projects.join(PROJECT);
        init(&runner);
        std::fs::write(runner.join(".gitignore"), "workspaces/\n").unwrap();
        commit(&runner, ".gitignore", "workspaces/\n");
        let web = runner.join("workspaces/app/web");
        init(&web);
        commit(&web, "index.html", "web\n");
        let desktop = top.join("desktop");
        git(&top, &["clone", "-q", path(&runner), "desktop"]);
        let app = desktop.join("workspaces/app");
        std::fs::create_dir_all(&app).unwrap();
        git(&app, &["clone", "-q", path(&web), "web"]);
        World {
            _dir: dir,
            projects,
            runner,
            desktop,
        }
    }

    fn web(&self) -> PathBuf {
        self.runner.join("workspaces/app/web")
    }

    fn not_home(&self) -> Vec<String> {
        let (held, unread) = repos_held(&self.projects);
        not_home(&self.desktop, PROJECT, &held, &unread)
    }
}

fn says(missing: &[String], words: &str) -> bool {
    missing.iter().any(|one| one.contains(words))
}

#[test]
fn a_runner_whose_every_commit_is_home_is_proved_home() {
    let world = World::new();

    assert_eq!(world.not_home(), Vec::<String>::new());
}

#[test]
fn a_runner_that_keeps_no_projects_holds_nothing() {
    let dir = tempfile::tempdir().unwrap();

    let (held, unread) = repos_held(&dir.path().join("none"));

    assert!(held.is_empty() && unread.is_empty());
}

#[test]
fn a_commit_on_a_branch_the_desktop_never_fetched_keeps_the_runner() {
    let world = World::new();
    commit(&world.web(), "fix.txt", "fixed on the runner\n");

    let missing = world.not_home();

    assert!(
        says(
            &missing,
            "refs/heads/main of the repository workspaces/app/web"
        ),
        "{missing:?}"
    );
}

#[test]
fn a_commit_on_no_branch_keeps_the_runner() {
    let world = World::new();
    git(&world.web(), &["checkout", "-q", "--detach"]);
    commit(&world.web(), "detached.txt", "on no branch\n");

    let missing = world.not_home();

    assert!(
        says(&missing, "HEAD of the repository workspaces/app/web"),
        "{missing:?}"
    );
}

#[test]
fn a_clone_stopped_mid_rebase_is_listed_and_keeps_the_runner() {
    let world = World::new();
    let web = world.web();
    git(&web, &["checkout", "-q", "-b", "fix"]);
    commit(&web, "index.html", "the fix\n");
    git(&web, &["checkout", "-q", "main"]);
    commit(&web, "index.html", "main moved\n");
    git(&web, &["checkout", "-q", "fix"]);
    let stopped = crate::testgit::run(
        &web,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "rebase",
            "main",
        ],
    );
    assert!(
        !stopped.ok(),
        "the rebase was meant to stop on its conflict"
    );

    let (held, unread) = repos_held(&world.projects);
    let missing = not_home(&world.desktop, PROJECT, &held, &unread);

    let listed = held
        .iter()
        .find(|one| one.path == "workspaces/app/web")
        .expect("the clone mid-rebase is listed");
    assert!(listed.refs.iter().any(|one| one.name == "refs/heads/fix"));
    assert!(
        says(
            &missing,
            "refs/heads/fix of the repository workspaces/app/web"
        ),
        "{missing:?}"
    );
}

#[test]
fn a_repository_nested_in_a_clone_keeps_the_runner() {
    let world = World::new();
    let nested = world.web().join("vendor/lib");
    init(&nested);
    commit(&nested, "lib.rs", "nested\n");

    let missing = world.not_home();

    assert!(
        says(
            &missing,
            "the repository workspaces/app/web/vendor/lib on the runner has no repository here"
        ),
        "{missing:?}"
    );
    // Its folder is the nested repository's own, not uncommitted work of the clone around it.
    assert!(
        !says(&missing, "workspaces/app/web on the runner holds changes"),
        "{missing:?}"
    );
}

#[test]
fn a_work_tree_whose_git_folder_is_elsewhere_is_listed() {
    let world = World::new();
    git(
        &world.web(),
        &["worktree", "add", "-q", "-b", "side", "side"],
    );
    commit(
        &world.web().join("side"),
        "side.txt",
        "a submodule's or a worktree's\n",
    );

    let missing = world.not_home();

    assert!(says(&missing, "workspaces/app/web/side"), "{missing:?}");
}

#[test]
fn a_clone_named_like_a_folder_a_workspace_keeps_keeps_the_runner() {
    let world = World::new();
    let named = world.runner.join("workspaces/app/memory");
    init(&named);
    commit(&named, "notes.md", "made on the runner\n");

    let missing = world.not_home();

    assert!(says(&missing, "workspaces/app/memory"), "{missing:?}");
}

#[test]
fn a_bare_repository_keeps_the_runner() {
    let world = World::new();
    let bare = world.runner.join("workspaces/app/mirror.git");
    git(
        &world.runner,
        &["clone", "-q", "--bare", path(&world.web()), path(&bare)],
    );
    commit(&world.web(), "more.txt", "in the bare one too\n");
    git(&world.web(), &["push", "-q", path(&bare), "main"]);

    let missing = world.not_home();

    assert!(says(&missing, "workspaces/app/mirror.git"), "{missing:?}");
}

#[test]
fn a_project_kept_under_another_id_keeps_the_runner() {
    // A project moved on the desktop, its id read again: the runner holds the old one too.
    let world = World::new();
    let old = world.projects.join("01J9PRJCT0000000000000OLD0");
    init(&old);
    commit(&old, "work.txt", "the task's work under the old id\n");

    let missing = world.not_home();

    assert!(says(&missing, "kept under another id"), "{missing:?}");
}

#[test]
fn a_stash_keeps_the_runner() {
    let world = World::new();
    std::fs::write(world.web().join("index.html"), "stashed\n").unwrap();
    git(
        &world.web(),
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "stash",
            "-q",
        ],
    );

    let missing = world.not_home();

    assert!(says(&missing, "refs/stash"), "{missing:?}");
}

#[test]
fn uncommitted_work_keeps_the_runner() {
    let world = World::new();
    std::fs::write(world.web().join("new.txt"), "not committed\n").unwrap();

    let missing = world.not_home();

    assert!(
        says(
            &missing,
            "the repository workspaces/app/web on the runner holds changes no commit has, which \
             differ here: new.txt"
        ),
        "{missing:?}"
    );
}

#[test]
fn the_desktops_own_uncommitted_work_sent_to_the_runner_is_no_loss() {
    // Sent as the chat started, and left as it was beside a commit the desktop fetched.
    let world = World::new();
    let desktop_web = world.desktop.join("workspaces/app/web");
    std::fs::write(desktop_web.join("index.html"), "not committed here\n").unwrap();
    std::fs::write(world.web().join("index.html"), "not committed here\n").unwrap();
    std::fs::write(desktop_web.join("draft.txt"), "a draft\n").unwrap();
    std::fs::write(world.web().join("draft.txt"), "a draft\n").unwrap();

    assert_eq!(world.not_home(), Vec::<String>::new());
}

#[test]
fn a_file_the_task_changed_further_keeps_the_runner() {
    let world = World::new();
    let desktop_web = world.desktop.join("workspaces/app/web");
    std::fs::write(desktop_web.join("index.html"), "not committed here\n").unwrap();
    std::fs::write(world.web().join("index.html"), "and changed by the task\n").unwrap();

    let missing = world.not_home();

    assert!(says(&missing, "differ here: index.html"), "{missing:?}");
}

#[test]
fn a_file_the_task_removed_keeps_the_runner() {
    let world = World::new();
    std::fs::remove_file(world.web().join("index.html")).unwrap();

    let missing = world.not_home();

    assert!(says(&missing, "differ here: index.html"), "{missing:?}");
}

/// **A file in no repository keeps the runner** (#4569, the person's ruling of 2026-11-02): one a
/// task wrote in its workspace's folder, which the project's own repository ignores.
#[test]
fn a_file_in_a_workspaces_folder_outside_any_clone_keeps_the_runner() {
    let world = World::new();
    std::fs::write(
        world.runner.join("workspaces/app/report.md"),
        "the task's report\n",
    )
    .unwrap();

    let missing = world.not_home();

    assert!(
        says(&missing, "differ here: workspaces/app/report.md"),
        "{missing:?}"
    );
}

/// The same file, here as it is there, is no loss.
#[test]
fn a_file_outside_any_clone_that_is_here_too_is_no_loss() {
    let world = World::new();
    for root in [&world.runner, &world.desktop] {
        std::fs::write(root.join("workspaces/app/report.md"), "the task's report\n").unwrap();
    }

    assert_eq!(world.not_home(), Vec::<String>::new());
}

/// **What a clone ignores is left out, as git leaves it out** (the person's ruling of
/// 2026-11-02): its build output keeps no runner.
#[test]
fn what_a_clone_ignores_keeps_no_runner() {
    let world = World::new();
    let web = world.web();
    commit(&web, ".gitignore", "out/\n");
    git(
        &world.desktop.join("workspaces/app/web"),
        &["pull", "-q", "origin", "main"],
    );
    std::fs::create_dir_all(web.join("out")).unwrap();
    std::fs::write(web.join("out/bundle.js"), "built\n").unwrap();

    assert_eq!(world.not_home(), Vec::<String>::new());
}

#[test]
fn the_state_folder_of_the_project_is_not_uncommitted_work() {
    let world = World::new();
    let state = world
        .runner
        .join(crate::names::STATE_DIR.spellings().next().unwrap());
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(state.join("sessions.json"), "{}").unwrap();

    assert_eq!(world.not_home(), Vec::<String>::new());
}

#[test]
fn branches_fetched_from_elsewhere_are_not_the_runners_work() {
    let world = World::new();
    let top = world.projects.parent().unwrap().to_path_buf();
    git(&top, &["clone", "-q", path(&world.web()), "elsewhere"]);
    let elsewhere = top.join("elsewhere");
    commit(&elsewhere, "theirs.txt", "on the forge\n");
    git(
        &world.web(),
        &[
            "fetch",
            "-q",
            path(&elsewhere),
            "main:refs/remotes/origin/main",
        ],
    );

    assert_eq!(world.not_home(), Vec::<String>::new());
}

#[test]
fn two_clones_differing_in_case_alone_keep_the_runner() {
    let world = World::new();
    let other = world.runner.join("workspaces/app/Web");
    if other.exists() {
        // A file system that ignores case holds one folder for both: nothing to tell apart.
        return;
    }
    init(&other);
    commit(&other, "index.html", "the other clone\n");

    let missing = world.not_home();

    assert!(says(&missing, "workspaces/app/Web"), "{missing:?}");
}

#[test]
fn a_list_that_is_not_whole_keeps_the_runner() {
    let world = World::new();
    let (held, _) = repos_held(&world.projects);

    let missing = not_home(
        &world.desktop,
        PROJECT,
        &held,
        &["too many folders".to_owned()],
    );

    assert_eq!(missing, vec!["too many folders".to_owned()]);
}

#[test]
fn a_path_that_leaves_the_project_is_never_one_here() {
    let world = World::new();
    let held = vec![Held {
        project: PROJECT.to_owned(),
        path: "../elsewhere".to_owned(),
        refs: vec![Ref {
            name: "refs/heads/main".to_owned(),
            oid: "0".repeat(40),
        }],
        changed: Vec::new(),
    }];

    let missing = not_home(&world.desktop, PROJECT, &held, &[]);

    assert!(says(&missing, "can't be one here"), "{missing:?}");
}

/// **A file whose name is not text keeps the runner** (D-4569-22): it can't be found again on
/// either end, so it is never taken for one that matches.
#[cfg(target_os = "linux")]
#[test]
fn a_file_whose_name_is_not_text_keeps_the_runner() {
    use std::os::unix::ffi::OsStrExt;
    let world = World::new();
    let name = std::ffi::OsStr::from_bytes(b"made-\xff.txt");
    std::fs::write(world.web().join(name), "the task's\n").unwrap();

    let missing = world.not_home();

    assert!(says(&missing, "whose name is not text"), "{missing:?}");
}

/// **A runner that no longer holds the project's own repository keeps it** (D-4569-21): a task
/// that removed it could hold, outside every clone, files no list shows.
#[test]
fn a_runner_whose_project_lost_its_own_repository_keeps_the_runner() {
    let world = World::new();
    std::fs::remove_dir_all(world.runner.join(".git")).unwrap();
    std::fs::write(
        world.runner.join("workspaces/app/report.md"),
        "the task's\n",
    )
    .unwrap();

    let missing = world.not_home();

    assert!(
        says(&missing, "no longer holds the project's own repository"),
        "{missing:?}"
    );
}

/// **What a host keeps beside its projects holds no task's work** (#4569): a transfer's scratch,
/// which comes and goes, is not looked through.
#[test]
fn what_is_kept_beside_the_projects_is_not_looked_through() {
    let world = World::new();
    let scratch = world.projects.join(".tmpAbC123");
    std::fs::create_dir_all(&scratch).unwrap();
    std::fs::write(scratch.join("transfer.bundle"), "bytes\n").unwrap();

    assert_eq!(world.not_home(), Vec::<String>::new());
}

/// **A file git is told not to look at still counts** (#4781): `git status` does not show a
/// change to one marked unchanged, so the runner looks at what it holds itself.
#[test]
fn a_file_marked_as_unchanged_that_the_task_changed_keeps_the_runner() {
    let world = World::new();
    git(
        &world.web(),
        &["update-index", "--assume-unchanged", "index.html"],
    );
    std::fs::write(
        world.web().join("index.html"),
        "changed where git won't look\n",
    )
    .unwrap();

    let missing = world.not_home();

    assert!(says(&missing, "differ here: index.html"), "{missing:?}");
}

/// **A file left out of git's view of the work tree still counts** (#4781): one marked to skip
/// the work tree, as a sparse checkout marks it.
#[test]
fn a_file_marked_to_skip_the_work_tree_that_the_task_changed_keeps_the_runner() {
    let world = World::new();
    git(
        &world.web(),
        &["update-index", "--skip-worktree", "index.html"],
    );
    std::fs::write(
        world.web().join("index.html"),
        "changed outside git's view\n",
    )
    .unwrap();

    let missing = world.not_home();

    assert!(says(&missing, "differ here: index.html"), "{missing:?}");
}

/// **What a sparse checkout leaves out is no loss** (#4781): a file marked to skip the work tree
/// and not there holds nothing, and its commit is home.
#[test]
fn a_file_a_sparse_checkout_leaves_out_is_no_loss() {
    let world = World::new();
    git(
        &world.web(),
        &["update-index", "--skip-worktree", "index.html"],
    );
    std::fs::remove_file(world.web().join("index.html")).unwrap();

    assert_eq!(world.not_home(), Vec::<String>::new());
}

/// **A file marked as unchanged that is as it is here is no loss** (#4781).
#[test]
fn a_file_marked_as_unchanged_that_is_as_it_is_here_is_no_loss() {
    let world = World::new();
    git(
        &world.web(),
        &["update-index", "--assume-unchanged", "index.html"],
    );

    assert_eq!(world.not_home(), Vec::<String>::new());
}

/// **Every stash counts, not only the newest** (#4781): `refs/stash` names the last one, and
/// the ones under it are only in its log.
#[test]
fn a_stash_under_the_newest_keeps_the_runner() {
    let world = World::new();
    let stash = |text: &str| {
        std::fs::write(world.web().join("index.html"), text).unwrap();
        git(
            &world.web(),
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "stash",
                "-q",
            ],
        );
    };
    stash("stashed first\n");
    stash("stashed last\n");

    let missing = world.not_home();

    assert!(says(&missing, "refs/stash@{1}"), "{missing:?}");
}

/// A runner's home, with what was in it before its host first started a program, and when that
/// was.
#[cfg(unix)]
fn a_home() -> (tempfile::TempDir, PathBuf, std::time::SystemTime) {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().canonicalize().unwrap();
    std::fs::create_dir_all(home.join("from-the-image")).unwrap();
    std::fs::write(
        home.join("from-the-image/readme.txt"),
        "made with the image\n",
    )
    .unwrap();
    std::fs::create_dir_all(home.join(".config")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    let started = std::time::SystemTime::now();
    std::thread::sleep(std::time::Duration::from_millis(50));
    (dir, home, started)
}

/// **Work a task keeps outside its project's folder keeps the runner** (#4781): a file it wrote
/// in the runner's home is in no repository, so it can't come here, and the Notice names it.
#[cfg(unix)]
#[test]
fn a_file_a_task_wrote_in_the_runners_home_keeps_the_runner() {
    let (_dir, home, started) = a_home();
    std::fs::create_dir_all(home.join("notes")).unwrap();
    std::fs::write(home.join("notes/findings.md"), "what the task found\n").unwrap();

    let said = outside_the_projects(&home, &[], Some(started));

    assert!(
        says(&said, "~/notes/findings.md") && says(&said, "outside its project"),
        "{said:?}"
    );
}

/// **What the runner's home held before its host started is no task's work** (#4781), and nor
/// is what programs keep in its hidden folders: their settings, caches and records.
#[cfg(unix)]
#[test]
fn what_the_home_held_before_and_what_programs_keep_hidden_keeps_no_runner() {
    let (_dir, home, started) = a_home();
    std::fs::write(home.join(".config/tool.toml"), "a program's settings\n").unwrap();
    std::fs::write(home.join(".bash_history"), "ls\n").unwrap();

    assert_eq!(
        outside_the_projects(&home, &[], Some(started)),
        Vec::<String>::new()
    );
}

/// **The folders purlis keeps in the home are not looked through** (#4781): its projects are
/// listed repository by repository, wherever its data home is.
#[cfg(unix)]
#[test]
fn the_folders_purlis_keeps_are_not_looked_through_outside_the_projects() {
    let (_dir, home, started) = a_home();
    let data = home.join("purlis-data");
    std::fs::create_dir_all(data.join("projects/one")).unwrap();
    std::fs::write(data.join("projects/one/file.txt"), "a project's\n").unwrap();

    assert_eq!(
        outside_the_projects(&home, &[data], Some(started)),
        Vec::<String>::new()
    );
}

/// **A link a task made in the home counts, and is never followed** (#4781).
#[cfg(unix)]
#[test]
fn a_link_a_task_made_in_the_home_keeps_the_runner() {
    let (_dir, home, started) = a_home();
    std::os::unix::fs::symlink("/etc", home.join("etc-link")).unwrap();

    let said = outside_the_projects(&home, &[], Some(started));

    assert!(says(&said, "~/etc-link"), "{said:?}");
}

/// **Before any program has run, nothing outside the projects is a task's** (#4781): a start
/// is never told its runner holds work there.
#[cfg(unix)]
#[test]
fn before_any_program_has_run_nothing_outside_the_projects_counts() {
    let (_dir, home, _started) = a_home();
    std::fs::write(
        home.join("written-at-boot.txt"),
        "by the image's own start\n",
    )
    .unwrap();

    assert_eq!(outside_the_projects(&home, &[], None), Vec::<String>::new());
}

/// **What a tool keeps by its own default is no task's work, but what is beside it is** (#4781):
/// Go's module cache under its default `GOPATH`, and a pipe a program left.
#[cfg(unix)]
#[test]
fn a_tools_own_cache_and_a_pipe_keep_no_runner_but_work_beside_them_does() {
    let (_dir, home, started) = a_home();
    std::fs::create_dir_all(home.join("go/pkg/mod/example.com")).unwrap();
    std::fs::write(home.join("go/pkg/mod/example.com/lib.go"), "package lib\n").unwrap();
    let made =
        crate::forklock::status(std::process::Command::new("mkfifo").arg(home.join("agent.pipe")))
            .unwrap();
    assert!(made.success(), "a pipe is made");

    assert_eq!(
        outside_the_projects(&home, &[], Some(started)),
        Vec::<String>::new()
    );

    std::fs::create_dir_all(home.join("go/src/tool")).unwrap();
    std::fs::write(home.join("go/src/tool/main.go"), "package main\n").unwrap();
    let said = outside_the_projects(&home, &[], Some(started));
    assert!(says(&said, "~/go/src/tool/main.go"), "{said:?}");
}

/// **A link to a folder is never looked through** (#4781): only the link is named, never what
/// it points at.
#[cfg(unix)]
#[test]
fn a_link_to_a_folder_outside_the_home_is_never_looked_through() {
    let (_dir, home, started) = a_home();
    let outside = tempfile::tempdir().unwrap();
    let elsewhere = outside.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::write(elsewhere.join("secret.txt"), "not the home's\n").unwrap();
    let inside = home.join("from-the-image");
    std::fs::rename(&inside, home.join("moved")).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &inside).unwrap();

    let said = outside_the_projects(&home, &[], Some(started));

    assert!(says(&said, "~/from-the-image"), "{said:?}");
    assert!(!says(&said, "secret.txt"), "{said:?}");
}

/// **Many files are counted, and only a few named** (#4781): what the runner says stays short
/// however many a task left.
#[cfg(unix)]
#[test]
fn many_files_outside_the_projects_are_counted_and_a_few_named() {
    let (_dir, home, started) = a_home();
    std::fs::create_dir_all(home.join("out")).unwrap();
    for n in 0..40 {
        std::fs::write(home.join(format!("out/{n:02}.txt")), "made\n").unwrap();
    }

    let said = outside_the_projects(&home, &[], Some(started));

    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        says(
            &said,
            "~/out/00.txt, ~/out/01.txt, ~/out/02.txt, ~/out/03.txt, ~/out/04.txt and 35 more"
        ),
        "{said:?}"
    );
}

/// **A file marked as unchanged that the task removed keeps the runner** (#4781): `status`
/// does not see the removal, and the file is still here.
#[test]
fn a_file_marked_as_unchanged_that_the_task_removed_keeps_the_runner() {
    let world = World::new();
    git(
        &world.web(),
        &["update-index", "--assume-unchanged", "index.html"],
    );
    std::fs::remove_file(world.web().join("index.html")).unwrap();

    let missing = world.not_home();

    assert!(says(&missing, "differ here: index.html"), "{missing:?}");
}

/// A runner's whole file system, as its pod has it: its home a folder of the volume it is on
/// (`/var/lib/purlis/home`), and what was there before its host first started a program.
#[cfg(unix)]
fn a_runners_root() -> (tempfile::TempDir, PathBuf, PathBuf, std::time::SystemTime) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let home = root.join("var/lib/purlis/home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(root.join("usr/share/doc")).unwrap();
    std::fs::write(root.join("usr/share/doc/readme"), "from the image\n").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    let started = std::time::SystemTime::now();
    std::thread::sleep(std::time::Duration::from_millis(50));
    (dir, root, home, started)
}

#[cfg(unix)]
fn the_runners_user() -> u32 {
    rustix::process::geteuid().as_raw()
}

/// **A file a task saved outside the runner's home keeps the runner too** (#4933): at the
/// sandbox's floor a task writes wherever the runner's user may, such as the volume its home is
/// a folder of, and the Notice names it from `/`.
#[cfg(unix)]
#[test]
fn a_file_a_task_saved_outside_the_runners_home_keeps_the_runner() {
    let (_dir, root, home, started) = a_runners_root();
    std::fs::write(
        root.join("var/lib/purlis/findings.md"),
        "what the task found\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("opt/work")).unwrap();
    std::fs::write(root.join("opt/work/.env"), "A=1\n").unwrap();

    let said = outside_the_projects_and_beyond(
        &home,
        &[],
        Some(started),
        Some(Beyond {
            root: &root,
            owner: the_runners_user(),
        }),
    );

    assert!(
        says(&said, "/var/lib/purlis/findings.md") && says(&said, "/opt/work/.env"),
        "{said:?}"
    );
}

/// **What holds no task's work outside the home keeps no runner** (#4933): the kernel's views of
/// itself, temporary folders, what a program keeps hidden in a person's home, what was there
/// before, and the home itself, which is looked through by its own rules.
#[cfg(unix)]
#[test]
fn what_holds_no_tasks_work_outside_the_home_keeps_no_runner() {
    let (_dir, root, home, started) = a_runners_root();
    for temporary in [
        "tmp/x",
        "var/tmp/x",
        "run/purlis-link/x",
        "dev/shm/x",
        "proc/1/x",
        "sys/x",
    ] {
        let at = root.join(temporary);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, "temporary\n").unwrap();
    }
    std::fs::create_dir_all(root.join("home/purlis/.ssh")).unwrap();
    std::fs::write(root.join("home/purlis/.ssh/known_hosts"), "a host\n").unwrap();
    std::fs::create_dir_all(home.join(".config")).unwrap();
    std::fs::write(home.join(".config/tool.toml"), "a program's\n").unwrap();
    let beyond = Beyond {
        root: &root,
        owner: the_runners_user(),
    };

    assert_eq!(
        outside_the_projects_and_beyond(&home, &[], Some(started), Some(beyond)),
        Vec::<String>::new()
    );

    std::fs::write(root.join("home/purlis/report.md"), "a task's\n").unwrap();
    let said = outside_the_projects_and_beyond(&home, &[], Some(started), Some(beyond));
    assert!(says(&said, "/home/purlis/report.md"), "{said:?}");
    assert!(
        !says(&said, "~/"),
        "the home is named once, by its own rules: {said:?}"
    );
}

/// **A file another user owns is no task's** (#4933): only what the runner's user owns counts
/// outside its home.
#[cfg(unix)]
#[test]
fn a_file_outside_the_home_another_user_owns_keeps_no_runner() {
    use std::os::unix::fs::PermissionsExt;
    if the_runners_user() == 0 {
        return; // Every file is open to root.
    }
    let (_dir, root, home, started) = a_runners_root();
    std::fs::write(root.join("var/lib/purlis/findings.md"), "made\n").unwrap();
    // Another's, and shut to the runner's user, as the walk sees it.
    std::fs::set_permissions(
        root.join("var/lib/purlis/findings.md"),
        std::fs::Permissions::from_mode(0o444),
    )
    .unwrap();

    assert_eq!(
        outside_the_projects_and_beyond(
            &home,
            &[],
            Some(started),
            Some(Beyond {
                root: &root,
                owner: the_runners_user().wrapping_add(1),
            }),
        ),
        Vec::<String>::new()
    );
}

/// **A file another user owns that the runner's user may write counts as its own** (#5103):
/// a task may change it as well as one it owns, so a change keeps the runner.
#[cfg(unix)]
#[test]
fn a_file_outside_the_home_its_user_may_write_but_does_not_own_keeps_the_runner() {
    if the_runners_user() == 0 {
        return; // Every file is open to root.
    }
    let (_dir, root, home, started) = a_runners_root();
    // Another's, as the walk sees it, but one the runner's user may write.
    std::fs::write(root.join("var/lib/purlis/findings.md"), "made\n").unwrap();

    let said = outside_the_projects_and_beyond(
        &home,
        &[],
        Some(started),
        Some(Beyond {
            root: &root,
            owner: the_runners_user().wrapping_add(1),
        }),
    );

    assert!(says(&said, "/var/lib/purlis/findings.md"), "{said:?}");
}

/// **An unreadable folder outside the home is said unless it is another's, unchanged, that the
/// runner's user can neither write in nor pass through** (#4933): only such a one can hold
/// nothing a task made.
#[cfg(unix)]
#[test]
fn a_folder_the_runners_user_cannot_read_is_said_unless_it_could_hold_nothing_of_a_task() {
    use std::os::unix::fs::PermissionsExt;
    if the_runners_user() == 0 {
        return; // Every folder is open to root.
    }
    let (_dir, root, home, _) = a_runners_root();
    let shut = |folder: &str, mode: u32| {
        std::fs::create_dir_all(root.join(folder)).unwrap();
        std::fs::set_permissions(root.join(folder), std::fs::Permissions::from_mode(mode)).unwrap();
    };
    shut("var/locked", 0o000);
    shut("var/passage", 0o100);
    shut("var/dropbox", 0o300);
    std::thread::sleep(std::time::Duration::from_millis(50));
    let since = std::time::SystemTime::now();
    std::thread::sleep(std::time::Duration::from_millis(50));
    shut("var/shut-since", 0o000);
    // Another user's folders, as the walk sees them.
    let others = Beyond {
        root: &root,
        owner: the_runners_user().wrapping_add(1),
    };

    let said = outside_the_projects_and_beyond(&home, &[], Some(since), Some(others));
    // The user's own, however long shut.
    let mine = Beyond {
        owner: the_runners_user(),
        ..others
    };
    let said_mine = outside_the_projects_and_beyond(&home, &[], Some(since), Some(mine));
    for folder in ["var/locked", "var/passage", "var/dropbox", "var/shut-since"] {
        std::fs::set_permissions(root.join(folder), std::fs::Permissions::from_mode(0o700))
            .unwrap();
    }

    for folder in ["/var/dropbox", "/var/passage", "/var/shut-since"] {
        assert!(
            says(&said, &format!("{folder} on the runner could not be read")),
            "{folder}: {said:?}"
        );
    }
    assert!(!says(&said, "/var/locked"), "{said:?}");
    assert_eq!(said.len(), 3, "{said:?}");
    assert!(
        says(&said_mine, "/var/locked on the runner could not be read"),
        "{said_mine:?}"
    );
}

/// **A folder too deep is said, not looked into** (#4933): the folders a walk holds open stay
/// bounded however deep a task nests them.
#[cfg(unix)]
#[test]
fn a_folder_nested_too_deep_outside_the_projects_is_said_and_keeps_the_runner() {
    let (_dir, home, started) = a_home();
    let deep: PathBuf = (0..MOST_DEPTH + 2).map(|_| "d").collect();
    std::fs::create_dir_all(home.join(&deep)).unwrap();

    let said = outside_the_projects(&home, &[], Some(started));

    assert!(says(&said, "folders deep"), "{said:?}");
}

/// **The first program's start is marked once, and kept** (#4933): a later start, or the host
/// starting again, never moves it; before any, nothing is marked.
#[cfg(unix)]
#[test]
fn the_first_programs_start_is_marked_once_and_kept() {
    let dir = tempfile::tempdir().unwrap();
    let projects = dir.path().join("data/projects");

    assert_eq!(first_program_at(&projects), Ok(None));

    first_program_starting(&projects).unwrap();
    let first = first_program_at(&projects).unwrap().expect("marked");
    std::thread::sleep(std::time::Duration::from_millis(50));
    first_program_starting(&projects).unwrap();

    assert_eq!(first_program_at(&projects), Ok(Some(first)));
}

/// **A mark that is not the file purlis made says so, which keeps the runner** (#4933), and a
/// link in its place is never followed, nor a pipe waited on: the program starts, since the
/// runner is kept for it (#5103).
#[cfg(unix)]
#[test]
fn a_mark_that_is_not_the_file_purlis_made_keeps_the_runner_and_is_never_followed() {
    let dir = tempfile::tempdir().unwrap();
    let projects = dir.path().join("projects");
    std::fs::create_dir_all(projects.join(FIRST_PROGRAM)).unwrap();

    assert_eq!(first_program_starting(&projects), Ok(()));
    let unknown = first_program_at(&projects).unwrap_err();
    assert!(unknown.contains("could not be looked for"), "{unknown}");

    std::fs::remove_dir(projects.join(FIRST_PROGRAM)).unwrap();
    let elsewhere = dir.path().join("elsewhere");
    std::os::unix::fs::symlink(&elsewhere, projects.join(FIRST_PROGRAM)).unwrap();
    assert_eq!(first_program_starting(&projects), Ok(()));
    assert!(!elsewhere.exists(), "never through the link");
    assert!(first_program_at(&projects).is_err());

    // A pipe in its place is never waited on, nor while a task holds it open to read.
    std::fs::remove_file(projects.join(FIRST_PROGRAM)).unwrap();
    let made = crate::forklock::status(
        std::process::Command::new("mkfifo").arg(projects.join(FIRST_PROGRAM)),
    )
    .unwrap();
    assert!(made.success(), "a pipe is made");
    assert_eq!(first_program_starting(&projects), Ok(()));
    let _reading = rustix::fs::open(
        projects.join(FIRST_PROGRAM),
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .unwrap();
    assert_eq!(first_program_starting(&projects), Ok(()));
    let unknown = first_program_at(&projects).unwrap_err();
    assert!(unknown.contains("not a file purlis made"), "{unknown}");
}

/// A runner's home with purlis's data home in it, as purlis's own image has it
/// (`~/.local/share/purlis`), its projects folder made and marked, and when that was.
#[cfg(unix)]
fn a_home_with_its_data() -> (tempfile::TempDir, PathBuf, PathBuf, std::time::SystemTime) {
    let (dir, home, _) = a_home();
    let data = home.join(".local/share/purlis");
    std::fs::create_dir_all(data.join(crate::datahome::PROJECTS)).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    let started = std::time::SystemTime::now();
    std::thread::sleep(std::time::Duration::from_millis(50));
    first_program_starting(&data.join(crate::datahome::PROJECTS)).unwrap();
    (dir, home, data, started)
}

/// **A file a task saved in the data home outside any repository keeps the runner** (#5029):
/// beside the projects, loose in the projects folder, or in a project's folder that holds no
/// repository. No repository lists it.
#[cfg(unix)]
#[test]
fn a_file_a_task_saved_in_the_data_home_outside_any_repository_keeps_the_runner() {
    let (_dir, home, data, started) = a_home_with_its_data();
    let projects = data.join(crate::datahome::PROJECTS);
    std::fs::write(data.join("beside.md"), "a task's\n").unwrap();
    std::fs::write(projects.join("loose.txt"), "a task's\n").unwrap();
    std::fs::create_dir_all(projects.join(PROJECT).join("notes")).unwrap();
    std::fs::write(projects.join(PROJECT).join("notes/found.md"), "a task's\n").unwrap();
    std::fs::write(projects.join(".notes"), "a task's, hidden\n").unwrap();

    let said = outside_with(
        &home,
        Some(DataHome {
            data: &data,
            projects: &projects,
            listed: &[],
        }),
        std::slice::from_ref(&data),
        Some(started),
        None,
    );

    for file in [
        "~/.local/share/purlis/beside.md",
        "~/.local/share/purlis/projects/loose.txt",
        "~/.local/share/purlis/projects/.notes",
        &format!("~/.local/share/purlis/projects/{PROJECT}/notes/found.md"),
    ] {
        assert!(says(&said, file), "{file}: {said:?}");
    }
}

/// **What purlis keeps in its data home keeps no runner** (#5029): its own stores (a browser's
/// profile, the host's events), the mark of the first program, a transfer's scratch, and each
/// repository listed on its own.
#[cfg(unix)]
#[test]
fn what_purlis_keeps_in_its_data_home_keeps_no_runner() {
    let (_dir, home, data, started) = a_home_with_its_data();
    let projects = data.join(crate::datahome::PROJECTS);
    for (folder, file) in [
        ("browsers/chat-7/Default", "Cookies"),
        ("events/device", "events.jsonl"),
        ("app-state/key", "state.json"),
        ("cache-homes/key", "x"),
        ("projects/.code-random", "bundle"),
    ] {
        std::fs::create_dir_all(data.join(folder)).unwrap();
        std::fs::write(data.join(folder).join(file), "purlis's own\n").unwrap();
    }
    let repo = projects.join(PROJECT);
    std::fs::create_dir_all(&repo).unwrap();
    init(&repo);
    std::fs::write(repo.join("work.txt"), "in a repository\n").unwrap();
    let (held, unread) = repos_held(&projects);
    assert_eq!(unread, Vec::<String>::new());
    let listed: Vec<PathBuf> = held.iter().map(|one| projects.join(&one.project)).collect();
    assert_eq!(listed.len(), 1, "{held:?}");

    assert_eq!(
        outside_with(
            &home,
            Some(DataHome {
                data: &data,
                projects: &projects,
                listed: &listed,
            }),
            std::slice::from_ref(&data),
            Some(started),
            None
        ),
        Vec::<String>::new()
    );
}

/// **A folder whose `.git` git does not take for a repository of its own is said** (#5029): git
/// would read the next repository up in its place, and its files would be in no list.
#[test]
fn a_folder_with_a_git_folder_that_is_not_a_repository_keeps_the_runner() {
    let dir = tempfile::tempdir().unwrap();
    let projects = dir.path().join("projects");
    let project = projects.join(PROJECT);
    std::fs::create_dir_all(&project).unwrap();
    init(&project);
    std::fs::create_dir_all(project.join("notes/.git")).unwrap();
    std::fs::write(project.join("notes/found.md"), "a task's\n").unwrap();

    let (held, unread) = repos_held(&projects);

    assert!(
        says(&unread, "notes in project") && says(&unread, "repository of its own"),
        "{unread:?} {held:?}"
    );
}

/// **A home inside the data home is looked through by the home's rules, not the data home's**
/// (#5029): what programs keep hidden in it keeps no runner.
#[cfg(unix)]
#[test]
fn a_home_inside_the_data_home_is_looked_through_as_a_home() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().canonicalize().unwrap().join("var/lib/purlis");
    let home = data.join("home");
    let projects = data.join(crate::datahome::PROJECTS);
    std::fs::create_dir_all(home.join(".config")).unwrap();
    std::fs::create_dir_all(&projects).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    let started = std::time::SystemTime::now();
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(home.join(".config/tool.toml"), "a program's\n").unwrap();
    let look = || {
        outside_with(
            &home,
            Some(DataHome {
                data: &data,
                projects: &projects,
                listed: &[],
            }),
            std::slice::from_ref(&data),
            Some(started),
            None,
        )
    };

    assert_eq!(look(), Vec::<String>::new());

    std::fs::write(home.join("report.md"), "a task's\n").unwrap();
    let said = look();
    assert!(says(&said, "~/report.md"), "{said:?}");
    assert!(!says(&said, "/var/lib/purlis/home"), "named once: {said:?}");
}

/// **The stores the walk of the data home leaves out are those the plane format names there**
/// (#5029, #5052): a store added there without it would keep every fresh runner, and a name left
/// out that the plane format does not name would hide a task's work there.
#[test]
fn every_store_the_plane_format_names_in_the_data_home_is_left_out_of_its_walk() {
    let format = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/plane-format.md"),
    )
    .unwrap();
    let mut named: Vec<&str> = format
        .match_indices("<data>/")
        .map(|(at, found)| {
            let rest = &format[at + found.len()..];
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'))
                .unwrap_or(rest.len());
            &rest[..end]
        })
        .filter(|name| !name.is_empty())
        .collect();
    named.sort_unstable();
    named.dedup();
    assert!(named.contains(&"events"), "{named:?}");

    let missing: Vec<&&str> = named
        .iter()
        .filter(|name| **name != crate::datahome::PROJECTS && !PURLIS_KEEPS.contains(name))
        .collect();

    assert!(missing.is_empty(), "add to PURLIS_KEEPS: {missing:?}");
    let unnamed: Vec<&&str> = PURLIS_KEEPS
        .iter()
        .filter(|kept| !named.contains(kept))
        .collect();
    assert!(
        unnamed.is_empty(),
        "name in docs/plane-format.md, with its tier: {unnamed:?}"
    );
}

/// **Each folder purlis keeps for itself is left out wherever the environment puts it** (#5028):
/// its config, data, log and socket homes moved outside the runner's home keep no runner, and a
/// file of another program's beside them still does.
#[cfg(unix)]
#[test]
fn each_home_purlis_keeps_moved_outside_the_runners_home_keeps_no_runner() {
    let looked_through = |env: &[(&str, &str)], written: &[&str]| {
        let (_dir, root, home, started) = a_runners_root();
        for file in written {
            std::fs::create_dir_all(root.join(file).parent().unwrap()).unwrap();
            std::fs::write(root.join(file), "written\n").unwrap();
        }
        let var = |name: &str| {
            env.iter()
                .find(|(set, _)| *set == name)
                .map(|(_, rel)| root.join(rel).to_string_lossy().into_owned())
        };
        outside_the_projects_and_beyond(
            &home,
            &purlis_keeps_for_itself(&var, &home),
            Some(started),
            Some(Beyond {
                root: &root,
                owner: the_runners_user(),
            }),
        )
    };
    let by_xdg = [
        ("XDG_CONFIG_HOME", "srv/config"),
        ("XDG_DATA_HOME", "srv/data"),
        ("XDG_RUNTIME_DIR", "srv/run"),
        ("XDG_STATE_HOME", "srv/state"),
        ("XDG_CACHE_HOME", "srv/cache"),
    ];

    let said = looked_through(
        &by_xdg,
        &[
            "srv/config/purlis/machine.json",
            "srv/config/charter/old.json",
            "srv/data/purlis/events/device/events.jsonl",
            "srv/data/dev.purlis.app/logs/purlis.log",
            "srv/run/purlis/host.lock",
        ],
    );
    assert_eq!(said, Vec::<String>::new());

    let said = looked_through(
        &[("PURLIS_CONFIG_HOME", "opt/purlis-config")],
        &[
            "opt/purlis-config/purlis/machine.json",
            "opt/purlis-config/.purlis.lock",
        ],
    );
    assert_eq!(said, Vec::<String>::new());

    // A folder a variable names as it is could be one others share: it is never left out
    // whole, nor is a folder holding the home.
    let said = looked_through(
        &[
            ("PURLIS_LOG_DIR", "srv"),
            ("PURLIS_DATA_HOME", "srv"),
            ("XDG_RUNTIME_DIR", "var/lib"),
        ],
        &["srv/work/out.txt", "var/lib/purlis/findings.md"],
    );
    assert!(
        says(&said, "/srv/work/out.txt") && says(&said, "/var/lib/purlis/findings.md"),
        "{said:?}"
    );

    // purlis keeps nothing of its own in a state or cache home, nor beside its folders: another
    // program's file there is the runner's user's like any other.
    let said = looked_through(
        &by_xdg,
        &["srv/state/tool/history", "srv/config/tool/settings.toml"],
    );
    assert!(
        says(&said, "/srv/state/tool/history") && says(&said, "/srv/config/tool/settings.toml"),
        "{said:?}"
    );
}

/// Makes each of `folders` in `root` one the runner's user can't write in, and gives them back.
#[cfg(unix)]
struct Shut(Vec<PathBuf>);

#[cfg(unix)]
impl Shut {
    fn folders(root: &Path, folders: &[&str]) -> Shut {
        use std::os::unix::fs::PermissionsExt;
        let mut shut: Vec<PathBuf> = folders.iter().map(|rel| root.join(rel)).collect();
        // The deepest first, so each is still open to be shut.
        shut.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
        for path in &shut {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o555)).unwrap();
        }
        Shut(shut)
    }
}

#[cfg(unix)]
impl Drop for Shut {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        for path in self.0.iter().rev() {
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
        }
    }
}

/// An image whose own folders, shut to the runner's user, outnumber what one check looks
/// through, with the volume its home is on (`/var/lib/purlis`) the one place it may write.
#[cfg(unix)]
fn a_large_image(folders: usize) -> (tempfile::TempDir, PathBuf, PathBuf, Shut) {
    let (dir, root, home, _) = a_runners_root();
    let mut shut = vec![String::new(), "var".to_owned(), "var/lib".to_owned()];
    for n in 0..folders {
        let rel = format!("usr/lib/{n:03}");
        std::fs::create_dir_all(root.join(&rel)).unwrap();
        shut.push(rel);
    }
    shut.extend(["usr", "usr/lib", "usr/share", "usr/share/doc"].map(str::to_owned));
    let shut: Vec<&str> = shut.iter().map(String::as_str).collect();
    let shut = Shut::folders(&root, &shut);
    (dir, root, home, shut)
}

/// **A large image is looked through once, for where its user may write, and each check walks
/// only those places** (#5030): an image's own folders, shut to that user, are never what keeps
/// a fresh runner for its size.
#[cfg(unix)]
#[test]
fn a_large_images_own_folders_are_looked_through_once_and_keep_no_runner_for_their_size() {
    if the_runners_user() == 0 {
        return; // Every folder is open to root.
    }
    let (_dir, root, home, _shut) = a_large_image(40);
    let since = std::time::SystemTime::now();
    // As another user's image: what only ownership would open is shut.
    let beyond = Beyond {
        root: &root,
        owner: the_runners_user().wrapping_add(1),
    };

    let found = writable(beyond, std::slice::from_ref(&home), since);

    assert_eq!(found.places(), ["var/lib/purlis"], "{found:?}");
    assert_eq!(
        outside_within(&home, None, &[], Some(since), Some((beyond, &found)), 10),
        Vec::<String>::new(),
        "40 shut folders are past a bound of 10 a check, and keep nothing"
    );
}

/// **An image too large even to look through once is said** (#5030), which keeps the runner as
/// anything not looked through does.
#[cfg(unix)]
#[test]
fn an_image_too_large_to_look_through_once_is_said() {
    if the_runners_user() == 0 {
        return;
    }
    let (_dir, root, home, _shut) = a_large_image(40);
    let since = std::time::SystemTime::now();
    let beyond = Beyond {
        root: &root,
        owner: the_runners_user().wrapping_add(1),
    };

    let found = writable_within(beyond, std::slice::from_ref(&home), since, 10);
    let said = outside_with(&home, None, &[], Some(since), Some((beyond, &found)));

    assert!(says(&said, "more than 10 folders"), "{said:?}");
}

/// **A place its user owns is looked through whatever its mode** (#5030): the user could open
/// it again at any time.
#[cfg(unix)]
#[test]
fn a_folder_its_user_owns_is_a_place_it_may_write_however_shut() {
    if the_runners_user() == 0 {
        return;
    }
    let (_dir, root, home, _shut) = a_large_image(2);
    let since = std::time::SystemTime::now();

    let found = writable(
        Beyond {
            root: &root,
            owner: the_runners_user(),
        },
        std::slice::from_ref(&home),
        since,
    );

    assert_eq!(
        found.places(),
        [""],
        "the root is the user's own: {found:?}"
    );
}

/// **A file of its user's own outside every place it may write is looked at again at each
/// check** (#5030): the user may change it wherever it is, and a change keeps the runner.
#[cfg(unix)]
#[test]
fn a_file_its_user_owns_where_it_may_not_write_keeps_the_runner_once_changed() {
    if the_runners_user() == 0 {
        return;
    }
    let (_dir, root, home, _) = a_runners_root();
    std::fs::create_dir_all(root.join("etc/tool")).unwrap();
    std::fs::write(root.join("etc/tool/settings.conf"), "as the image had it\n").unwrap();
    let _shut = Shut::folders(
        &root,
        &[
            "",
            "etc",
            "etc/tool",
            "usr",
            "usr/share",
            "usr/share/doc",
            "var",
            "var/lib",
        ],
    );
    std::thread::sleep(std::time::Duration::from_millis(50));
    let since = std::time::SystemTime::now();
    std::thread::sleep(std::time::Duration::from_millis(50));
    let beyond = Beyond {
        root: &root,
        owner: the_runners_user(),
    };
    // As an image's folders, shut to its user, which the test's own user can only stand for.
    let found = writable_bounded(
        beyond,
        std::slice::from_ref(&home),
        since,
        MOST_IMAGE_FOLDERS,
        MOST_OWN_FILES,
        false,
    );
    assert_eq!(found.places(), ["var/lib/purlis"], "{found:?}");
    let look = || outside_with(&home, None, &[], Some(since), Some((beyond, &found)));

    assert_eq!(look(), Vec::<String>::new());

    std::fs::write(root.join("etc/tool/settings.conf"), "as the task left it\n").unwrap();
    let said = look();
    assert!(says(&said, "/etc/tool/settings.conf"), "{said:?}");
}

/// **Past the most files of its user's own it records, the look is said and not whole**
/// (#5030), so it keeps the runner and is made again at the next check.
#[cfg(unix)]
#[test]
fn too_many_files_of_its_users_own_where_it_may_not_write_are_said() {
    if the_runners_user() == 0 {
        return;
    }
    let (_dir, root, home, _) = a_runners_root();
    std::fs::create_dir_all(root.join("etc")).unwrap();
    for n in 0..3 {
        std::fs::write(root.join(format!("etc/{n}.conf")), "the image's\n").unwrap();
    }
    let _shut = Shut::folders(
        &root,
        &[
            "",
            "etc",
            "usr",
            "usr/share",
            "usr/share/doc",
            "var",
            "var/lib",
        ],
    );
    let since = std::time::SystemTime::now();
    let beyond = Beyond {
        root: &root,
        owner: the_runners_user(),
    };

    let found = writable_bounded(beyond, std::slice::from_ref(&home), since, 100, 2, false);

    assert!(!found.whole(), "{found:?}");
    let said = outside_with(&home, None, &[], Some(since), Some((beyond, &found)));
    assert!(says(&said, "more than 2 files"), "{said:?}");
    let whole = writable_bounded(beyond, std::slice::from_ref(&home), since, 100, 10, false);
    assert!(whole.whole(), "{whole:?}");
}

/// **A file another user owns that the runner's user may write, where it may not write in the
/// folder, is looked at again at each check** (#5103): a task may change it in
/// place, and a change keeps the runner. One it may not write is not.
#[cfg(unix)]
#[test]
fn a_file_its_user_may_write_where_it_may_not_write_the_folder_keeps_the_runner_once_changed() {
    use std::os::unix::fs::PermissionsExt;
    if the_runners_user() == 0 {
        return;
    }
    let (_dir, root, home, _) = a_runners_root();
    std::fs::create_dir_all(root.join("etc/tool")).unwrap();
    std::fs::write(root.join("etc/tool/shared.conf"), "as the image had it\n").unwrap();
    std::fs::write(root.join("etc/tool/shut.conf"), "as the image had it\n").unwrap();
    std::fs::set_permissions(
        root.join("etc/tool/shut.conf"),
        std::fs::Permissions::from_mode(0o444),
    )
    .unwrap();
    let _shut = Shut::folders(
        &root,
        &[
            "",
            "etc",
            "etc/tool",
            "usr",
            "usr/share",
            "usr/share/doc",
            "var",
            "var/lib",
        ],
    );
    std::thread::sleep(std::time::Duration::from_millis(50));
    let since = std::time::SystemTime::now();
    std::thread::sleep(std::time::Duration::from_millis(50));
    // As another user's image: the test's own files are another's to the walk.
    let beyond = Beyond {
        root: &root,
        owner: the_runners_user().wrapping_add(1),
    };
    let found = writable(beyond, std::slice::from_ref(&home), since);
    assert_eq!(found.places(), ["var/lib/purlis"], "{found:?}");
    let look = || outside_with(&home, None, &[], Some(since), Some((beyond, &found)));

    assert_eq!(look(), Vec::<String>::new());

    std::fs::write(root.join("etc/tool/shared.conf"), "as the task left it\n").unwrap();
    std::fs::set_permissions(
        root.join("etc/tool/shut.conf"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    std::fs::set_permissions(
        root.join("etc/tool/shut.conf"),
        std::fs::Permissions::from_mode(0o444),
    )
    .unwrap();
    let said = look();
    assert!(says(&said, "/etc/tool/shared.conf"), "{said:?}");
    assert!(!says(&said, "/etc/tool/shut.conf"), "{said:?}");
}

/// **A name the runner reports is shown with each character a reader would not see marked**
/// (#5103): a control, a line end or a bidirectional override in a file's name
/// is written as `⟦U+XXXX⟧` in the sentences the window shows.
#[cfg(unix)]
#[test]
fn a_name_holding_a_character_a_reader_would_not_see_is_shown_marked() {
    let (_dir, home, started) = a_home();
    std::fs::write(home.join("report\u{202e}dm.exe"), "a task's\n").unwrap();
    std::fs::create_dir_all(home.join("two\nlines\u{1b}[31m")).unwrap();
    std::fs::write(home.join("two\nlines\u{1b}[31m/x"), "a task's\n").unwrap();

    let said = outside_the_projects(&home, &[], Some(started));

    assert!(says(&said, "~/report⟦U+202E⟧dm.exe"), "{said:?}");
    assert!(says(&said, "~/two⟦U+000A⟧lines⟦U+001B⟧[31m/x"), "{said:?}");
    for sentence in &said {
        assert!(
            !purlis_unicode::hides(sentence, purlis_unicode::Breaks::Marked),
            "{sentence:?}"
        );
    }
}

/// **What the desktop says a runner holds is shown marked too** (#5103): a path,
/// a ref's name or a sentence a runner sent holding a character a reader would not see.
#[test]
fn what_a_runner_reports_is_shown_marked_on_the_desktop() {
    let world = World::new();
    let held = vec![
        Held {
            project: PROJECT.to_owned(),
            path: String::new(),
            refs: Vec::new(),
            changed: Vec::new(),
        },
        Held {
            project: PROJECT.to_owned(),
            path: "web\u{202e}gpj.exe".to_owned(),
            refs: vec![Ref {
                name: "refs/heads/a\u{1b}[2J".to_owned(),
                oid: "0".repeat(40),
            }],
            changed: Vec::new(),
        },
    ];
    let unread = vec!["a sentence the runner sent\nwith a second line\u{2066}".to_owned()];

    let missing = not_home(&world.desktop, PROJECT, &held, &unread);

    assert!(says(&missing, "web⟦U+202E⟧gpj.exe"), "{missing:?}");
    assert!(says(&missing, "sent⟦U+000A⟧with"), "{missing:?}");
    for sentence in &missing {
        assert!(
            !purlis_unicode::hides(sentence, purlis_unicode::Breaks::Marked),
            "{sentence:?}"
        );
    }
}

/// **A start is refused where no mark could be made and nothing stands in its place** (#4933):
/// a host started again would then know of no program, and look for nothing.
#[cfg(unix)]
#[test]
fn a_start_is_refused_where_no_mark_can_be_made() {
    use std::os::unix::fs::PermissionsExt;
    if the_runners_user() == 0 {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let projects = dir.path().join("projects");
    std::fs::create_dir_all(&projects).unwrap();
    std::fs::set_permissions(&projects, std::fs::Permissions::from_mode(0o555)).unwrap();

    let refused = first_program_starting(&projects);
    std::fs::set_permissions(&projects, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert!(refused.unwrap_err().contains("could not note"));
    assert_eq!(first_program_at(&projects), Ok(None));
}
