// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::sandbox::local::OwnConfigHome;
use crate::secrets::answers;
use crate::secrets::seal;
use std::path::{Path, PathBuf};

/// A machine of the test's own: its config home, a keyring with a key, and a folder for planes.
fn machine() -> (OwnConfigHome, tempfile::TempDir) {
    let home = OwnConfigHome::new();
    seal::forget_kept_for_this_test();
    seal::set_for_this_test(Some(vec![7; 32]));
    seal::set_foreign_for_this_test(false);
    seal::set_locked_for_this_test(false);
    seal::set_no_keyring_for_this_test(false);
    (home, tempfile::tempdir().expect("a folder for planes"))
}

/// A plane at `name` under `folder`: a git folder and the project file that marks it.
fn plane(folder: &Path, name: &str) -> PathBuf {
    let root = std::fs::canonicalize(folder).unwrap().join(name);
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join("purlis.toml"), "").unwrap();
    root
}

/// A folder that is no plane's, beside the planes.
fn outside(folder: &Path) -> PathBuf {
    let dir = std::fs::canonicalize(folder).unwrap().join("elsewhere");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn at(cwd: &Path) -> Whereabouts<'_> {
    Whereabouts {
        cwd,
        targets: &[],
        repos: &[],
        ancestry: &[],
    }
}

/// The five plane-less scenarios stay so: nothing of the process is in a plane, and it runs the
/// shipped default under `strict` in its own process.
#[test]
fn a_process_outside_every_anchored_plane_runs_the_shipped_strict_locally() {
    let (_home, planes) = machine();
    let root = plane(planes.path(), "p");
    answers::register(&root).expect("registered");
    let elsewhere = outside(planes.path());
    assert_eq!(locate(&at(&elsewhere)), Locality::PlaneLess);
    let said = judge(&at(&elsewhere), &said_request());
    assert_eq!(
        said,
        Judged::Locally(crate::policy::decide::decide_locally(&said_request()))
    );
}

fn said_request() -> crate::policy::decide::Request<'static> {
    crate::policy::decide::Request {
        chat: "1",
        of: crate::policy::levels::Chat {
            workspace: None,
            persona: None,
        },
        action: "exec",
        resource: ("Command", "ls"),
        context: serde_json::json!({}),
    }
}

/// `git -C <plane>` run from a folder outside every plane: the working directory is outside, the
/// target is not, and so the process is never judged as plane-less.
#[test]
fn git_c_into_an_anchored_plane_is_never_judged_as_plane_less() {
    let (_home, planes) = machine();
    let root = plane(planes.path(), "p");
    answers::register(&root).expect("registered");
    let elsewhere = outside(planes.path());
    let whereabouts = Whereabouts {
        cwd: &elsewhere,
        targets: std::slice::from_ref(&root),
        repos: &[],
        ancestry: &[],
    };
    assert_eq!(
        locate(&whereabouts),
        Locality::InPlane {
            root: root.clone(),
            anchored: true
        }
    );
    // A target reached through a link is the plane's by its real path.
    #[cfg(unix)]
    {
        let link = elsewhere.join("link");
        std::os::unix::fs::symlink(&root, &link).unwrap();
        let through = Whereabouts {
            cwd: &elsewhere,
            targets: std::slice::from_ref(&link),
            repos: &[],
            ancestry: &[],
        };
        assert!(matches!(locate(&through), Locality::InPlane { .. }));
    }
}

/// A process that sits outside, started by one that works in an anchored plane: its ancestry says
/// it is the plane's.
#[test]
fn a_process_whose_ancestry_is_in_an_anchored_plane_is_never_judged_as_plane_less() {
    let (_home, planes) = machine();
    let root = plane(planes.path(), "p");
    answers::register(&root).expect("registered");
    let elsewhere = outside(planes.path());
    let ancestry = [elsewhere.clone(), root.join("sub")];
    let whereabouts = Whereabouts {
        cwd: &elsewhere,
        targets: &[],
        repos: &[],
        ancestry: &ancestry,
    };
    assert!(matches!(
        locate(&whereabouts),
        Locality::InPlane { anchored: true, .. }
    ));
}

/// No keyring: the registry cannot be read, so the planes found on disk are what is asked, and a
/// process inside one is never plane-less.
#[test]
fn without_a_keyring_a_process_in_a_plane_found_on_disk_is_never_plane_less() {
    let (_home, planes) = machine();
    let root = plane(planes.path(), "p");
    seal::set_no_keyring_for_this_test(true);
    let inside = root.join("sub");
    std::fs::create_dir_all(&inside).unwrap();
    assert_eq!(
        locate(&at(&inside)),
        Locality::InPlane {
            root: root.clone(),
            anchored: false
        }
    );
    // The same through `git -C`, and through the ancestry.
    let elsewhere = outside(planes.path());
    let target = Whereabouts {
        cwd: &elsewhere,
        targets: std::slice::from_ref(&inside),
        repos: &[],
        ancestry: &[],
    };
    assert!(matches!(locate(&target), Locality::InPlane { .. }));
    let ancestry = [inside];
    let parent = Whereabouts {
        cwd: &elsewhere,
        targets: &[],
        repos: &[],
        ancestry: &ancestry,
    };
    assert!(matches!(locate(&parent), Locality::InPlane { .. }));
    // And a folder outside every plane stays plane-less.
    assert_eq!(locate(&at(&elsewhere)), Locality::PlaneLess);
}

/// Inside a plane found on disk with no keyring, the shipped default decides and the seal gap is
/// declared in what the decision says.
#[test]
fn without_a_keyring_inside_a_found_plane_the_shipped_default_applies_with_the_gap_declared() {
    let (_home, planes) = machine();
    let root = plane(planes.path(), "p");
    seal::set_no_keyring_for_this_test(true);
    let judged = judge(&at(&root), &said_request());
    let Judged::FoundPlane { decision, gap } = judged else {
        panic!("a found plane is judged by the shipped default: {judged:?}");
    };
    assert_eq!(
        decision,
        crate::policy::decide::decide_locally(&said_request())
    );
    assert_eq!(gap, answers::NO_KEYRING_NOTICE);
}

/// A plane anchored on a machine with a keyring is the host's to judge: the process must ask it,
/// and is refused where it cannot.
#[test]
fn a_process_in_an_anchored_plane_is_the_hosts_to_ask() {
    let (_home, planes) = machine();
    let root = plane(planes.path(), "p");
    answers::register(&root).expect("registered");
    let judged = judge(&at(&root), &said_request());
    assert_eq!(judged, Judged::AskTheHost { root });
}

/// What a process that is not the app (no seal key, so an unreadable registry) is shown.
fn outside_the_app() {
    seal::set_for_this_test(None);
}

/// F1: outside the app the registry reads as unreadable, and an anchored plane is still anchored,
/// by the app's own list: the refusal is reachable where the hook runs.
#[test]
fn outside_the_app_an_anchored_plane_is_still_anchored_by_the_apps_list() {
    let (_home, planes) = machine();
    let root = plane(planes.path(), "p");
    answers::register(&root).expect("registered by the app");
    outside_the_app();
    assert!(
        answers::plane_containing(&root).is_none(),
        "the registry is unreadable here: this is what the hook sees"
    );
    let inside = root.join("sub");
    std::fs::create_dir_all(&inside).unwrap();
    assert_eq!(
        locate(&at(&inside)),
        Locality::InPlane {
            root: root.clone(),
            anchored: true
        }
    );
    assert_eq!(
        judge(&at(&inside), &said_request()),
        Judged::AskTheHost { root }
    );
}

/// F3: a linked worktree cut outside the plane is found through its main tree's plane, and that
/// root is looked up in the anchored list: anchored, not "found on disk alone".
#[test]
fn a_linked_worktree_outside_an_anchored_plane_is_the_planes_and_anchored() {
    let (_home, planes) = machine();
    let root = plane(planes.path(), "p");
    answers::register(&root).expect("registered");
    outside_the_app();
    // What `git worktree add` leaves: a `.git` file in the worktree, the main tree's git folder
    // holding `worktrees/<name>`.
    let worktree = outside(planes.path()).join("wt");
    std::fs::create_dir_all(&worktree).unwrap();
    let admin = root.join(".git").join("worktrees").join("wt");
    std::fs::create_dir_all(&admin).unwrap();
    std::fs::write(admin.join("commondir"), "../..\n").unwrap();
    std::fs::write(
        worktree.join(".git"),
        format!("gitdir: {}\n", admin.display()),
    )
    .unwrap();
    assert_eq!(
        locate(&at(&worktree)),
        Locality::InPlane {
            root: root.clone(),
            anchored: true
        }
    );
}

/// A repo the command names is an anchored plane's by that plane's remote, from a folder that is
/// no plane's at all.
#[test]
fn a_repo_the_command_names_is_the_anchored_planes_by_its_remote() {
    let (_home, planes) = machine();
    let root = plane(planes.path(), "p");
    std::fs::write(
        root.join(".git").join("config"),
        "[remote \"origin\"]\n\turl = https://github.com/Acme/Web.git\n",
    )
    .unwrap();
    answers::register(&root).expect("registered");
    outside_the_app();
    let elsewhere = outside(planes.path());
    let named = ["acme/web".to_owned()];
    let whereabouts = Whereabouts {
        cwd: &elsewhere,
        targets: &[],
        repos: &named,
        ancestry: &[],
    };
    assert_eq!(
        locate(&whereabouts),
        Locality::InPlane {
            root,
            anchored: true
        }
    );
    let other = ["acme/other".to_owned()];
    let whereabouts = Whereabouts {
        repos: &other,
        ..whereabouts
    };
    assert_eq!(locate(&whereabouts), Locality::PlaneLess);
}

/// With no `-C` and no `-R`, the working directory's own remote names the repo: a clone of the
/// same repo outside the plane is the plane's too.
#[test]
fn the_working_directorys_remote_is_the_anchored_planes_when_it_is_the_same_repo() {
    let (_home, planes) = machine();
    let root = plane(planes.path(), "p");
    let config = "[remote \"origin\"]\n\turl = https://github.com/acme/web.git\n";
    std::fs::write(root.join(".git").join("config"), config).unwrap();
    answers::register(&root).expect("registered");
    outside_the_app();
    let clone = outside(planes.path()).join("clone");
    std::fs::create_dir_all(clone.join(".git")).unwrap();
    std::fs::write(clone.join(".git").join("config"), config).unwrap();
    assert!(matches!(
        locate(&at(&clone)),
        Locality::InPlane { anchored: true, .. }
    ));
}

fn no_env(_: &str) -> Option<String> {
    None
}

fn reach(command: &str) -> Result<Reach, Unreadable> {
    reach_of(command, Path::new("/work/elsewhere"), &no_env)
}

/// `git -C <folder>` names the folder the command acts on, relative to the working directory, and
/// git composes more than one.
#[test]
fn the_folders_of_a_git_command_are_those_of_its_own_global_options() {
    let cwd = Path::new("/work/elsewhere");
    assert_eq!(
        reach("git -C /plane status").unwrap().targets,
        [PathBuf::from("/plane")]
    );
    assert_eq!(
        reach("git -C ../plane status").unwrap().targets,
        [cwd.join("../plane")]
    );
    assert!(reach("git status").unwrap().targets.is_empty());
    // Composed, as git reads them: the second is against the first.
    assert_eq!(
        reach("git -C a -C b status").unwrap().targets,
        [cwd.join("a"), cwd.join("a").join("b")]
    );
    // Quoted, the folder is still the folder: the word is not cut at a space.
    assert_eq!(
        reach("git -C \"my plane\" status").unwrap().targets,
        [cwd.join("my plane")]
    );
}

/// F2, each evasion: the old reader took the last `-C` of the whole line.
#[test]
fn a_dash_c_after_the_command_or_in_another_tool_is_not_gits() {
    // A later `-C` of another tool does not replace git's.
    assert_eq!(
        reach("git -C /plane status && make -C /elsewhere")
            .unwrap()
            .targets,
        [PathBuf::from("/plane")]
    );
    // Nor is a `-C` after the subcommand (`git commit -C <commit>` reuses a message).
    assert!(reach("git commit -C HEAD").unwrap().targets.is_empty());
    // Each segment is read: a `-C` in the first of two commands is not lost to the second.
    assert_eq!(
        reach("git -C /plane status; git -C /other log")
            .unwrap()
            .targets,
        [PathBuf::from("/plane"), PathBuf::from("/other")]
    );
}

#[test]
fn a_cd_is_a_target_and_moves_the_folder_later_words_are_against() {
    let cwd = Path::new("/work/elsewhere");
    assert_eq!(
        reach("cd /plane && git status").unwrap().targets,
        [PathBuf::from("/plane")]
    );
    assert_eq!(
        reach("cd a && cd b && git status").unwrap().targets,
        [cwd.join("a"), cwd.join("a").join("b")]
    );
    assert_eq!(
        reach("cd sub && git -C inner status").unwrap().targets,
        [cwd.join("sub"), cwd.join("sub").join("inner")]
    );
}

#[test]
fn git_dir_and_work_tree_name_a_folder_too() {
    assert_eq!(
        reach("git --git-dir=/plane/.git status").unwrap().targets,
        [PathBuf::from("/plane/.git")]
    );
    assert_eq!(
        reach("git --work-tree /plane status").unwrap().targets,
        [PathBuf::from("/plane")]
    );
    assert_eq!(
        reach("GIT_DIR=/plane/.git git status").unwrap().targets,
        [PathBuf::from("/plane/.git")]
    );
    let env = |name: &str| (name == "GIT_WORK_TREE").then(|| "/plane".to_owned());
    let from_env = reach_of("git status", Path::new("/w"), &env).unwrap();
    assert_eq!(from_env.targets, [PathBuf::from("/plane")]);
}

/// Anything the shell would expand is read as unreadable, never as outside.
#[test]
fn a_folder_or_repo_the_shell_would_expand_is_unreadable() {
    for command in [
        "git -C $PLANE status",
        "git -C ~/plane status",
        "git -C \"$(pwd)/..\" status",
        "git -C /pl* status",
        "git -C `pwd` status",
        "cd ~/plane && git status",
        "cd && git status",
        "cd - && git status",
        "gh pr view 5 -R $REPO",
        "gh pr view 5 --repo=${REPO}",
        "GH_REPO=$R gh pr view 5",
        "git --git-dir=$D status",
    ] {
        assert!(reach(command).is_err(), "{command} must not be read");
    }
    // A command that will not tokenize is not read either.
    assert!(reach("git -C 'unclosed status").is_err());
}

/// The forge command's own target: `-R`, `--repo`, `GH_REPO` (in the line or the environment),
/// and the request's address.
#[test]
fn the_repo_a_forge_command_names_is_read_from_its_own_words() {
    assert_eq!(
        reach("gh pr view 5 -R Acme/Web").unwrap().repos,
        ["acme/web"]
    );
    assert_eq!(
        reach("gh pr view 5 --repo acme/web").unwrap().repos,
        ["acme/web"]
    );
    assert_eq!(
        reach("gh pr view 5 --repo=github.com/acme/web")
            .unwrap()
            .repos,
        ["acme/web"]
    );
    assert_eq!(
        reach("GH_REPO=acme/web gh pr view 5").unwrap().repos,
        ["acme/web"]
    );
    let env = |name: &str| (name == "GH_REPO").then(|| "acme/web".to_owned());
    let named = reach_of("gh pr view 5", Path::new("/w"), &env).unwrap();
    assert_eq!(named.repos, ["acme/web"]);
    assert_eq!(
        reach("gh pr view https://github.com/acme/web/pull/5")
            .unwrap()
            .repos,
        ["acme/web"]
    );
    assert_eq!(
        reach("glab mr view https://gitlab.com/grp/sub/api/-/merge_requests/7")
            .unwrap()
            .repos,
        ["grp/sub/api"]
    );
    // Another tool's `-R` names no repo of a forge's.
    assert!(reach("rsync -R a b").unwrap().repos.is_empty());
}

/// R2-4: the floor follows `sh -c`, `eval`, substitutions and wrappers to a merge; this reader does
/// not, so where the merge is not a top-level command of its own, nothing is said of where it goes.
#[test]
fn a_merge_that_is_not_a_top_level_command_is_unreadable() {
    for command in [
        "sh -c 'cd /anchored && gh pr merge 5'",
        "bash -c 'gh pr merge 5 -R o/planerepo'",
        "zsh -c \"gh pr merge 5\"",
        "eval 'gh pr merge 5 -R o/planerepo'",
        "echo 5 | xargs gh pr merge",
        "env -S 'gh pr merge 5'",
        "bash <<< 'gh pr merge 5'",
        "bash <<EOF\ngh pr merge 5\nEOF",
        "echo $(gh pr merge 5)",
        "echo `gh pr merge 5`",
        "bash ./merge-it.sh",
        "source ./merge-it.sh",
    ] {
        assert!(reach(command).is_err(), "{command} must not be read");
    }
    // The same merges, spelled out as a command of their own, are read.
    assert!(reach("gh pr merge 5").is_ok());
    assert!(reach("cd /anchored && gh pr merge 5 -R o/planerepo").is_ok());
    assert!(reach("env GH_REPO=o/r gh pr merge 5").is_ok());
}

/// R2-3: `-R` is read as pflag reads it, and its value as a remote's address would be.
#[test]
fn a_repo_flag_is_read_in_every_spelling_pflag_has() {
    for flag in [
        "-R o/r",
        "-Ro/r",
        "-R=o/r",
        "-dR o/r",
        "-dRo/r",
        "-dR=o/r",
        "--repo o/r",
        "--repo=o/r",
        "-R O/R",
        "-R github.com/o/r",
        "-R git@github.com:o/r.git",
        "-R https://github.com/o/r.git",
        "-R ssh://git@github.com/o/r",
        "--repo=https://github.com/o/r/pull/5",
    ] {
        let command = format!("gh pr merge 5 {flag}");
        assert_eq!(
            reach(&command)
                .unwrap_or_else(|why| panic!("{command}: {why}"))
                .repos,
            ["o/r"],
            "{command}"
        );
    }
    // A value another flag takes is not a repo, even where it looks like the flag.
    assert!(reach("gh pr merge 5 -b -Rx").unwrap().repos.is_empty());
}

#[test]
fn a_forge_command_whose_repo_cannot_be_named_is_unreadable() {
    for command in [
        "gh pr merge 5 -R justonename",
        "gh pr merge 5 -R",
        "gh pr merge 5 --repo=",
        "gh pr merge 5 -dR $REPO",
        "GH_REPO=nothing gh pr merge 5",
        "gh api graphql -f query=@merge.graphql",
        "gh api user",
        "gh api repos/{owner}/{repo}/pulls/5/merge -X PUT",
        "glab api projects/12/merge_requests/5/merge -X PUT",
    ] {
        assert!(reach(command).is_err(), "{command} must not be read");
    }
}

#[test]
fn a_gh_api_call_is_read_where_its_repo_path_is_plain() {
    assert_eq!(
        reach("gh api -X PUT repos/Acme/Web/pulls/5/merge")
            .unwrap()
            .repos,
        ["acme/web"]
    );
    assert_eq!(
        reach("gh api /repos/acme/web/pulls/5/merge --method PUT")
            .unwrap()
            .repos,
        ["acme/web"]
    );
}

/// R2-2: every `GH_REPO` the command and the process hold is a repo it reaches, never the last.
#[test]
fn every_gh_repo_seen_is_a_repo_the_command_reaches() {
    let env = |name: &str| (name == "GH_REPO").then(|| "env/one".to_owned());
    let read = reach_of(
        "GH_REPO=a/b; GH_REPO=c/d gh pr merge 5",
        Path::new("/w"),
        &env,
    )
    .unwrap();
    let mut repos = read.repos;
    repos.sort();
    assert_eq!(repos, ["a/b", "c/d", "env/one"]);
}

/// R2-5: a wrapper's own chdir, an exported variable and `CDPATH` all move where a command acts.
#[test]
fn a_wrapper_s_chdir_and_an_exported_variable_are_what_the_command_reaches() {
    assert_eq!(
        reach("env -C /plane gh pr merge 5").unwrap().targets,
        [PathBuf::from("/plane")]
    );
    assert_eq!(
        reach("sudo -D /plane gh pr merge 5").unwrap().targets,
        [PathBuf::from("/plane")]
    );
    assert_eq!(
        reach("export GH_REPO=a/b && gh pr merge 5").unwrap().repos,
        ["a/b"]
    );
    assert_eq!(
        reach("declare -x GIT_DIR=/plane/.git; gh pr merge 5")
            .unwrap()
            .targets,
        [PathBuf::from("/plane/.git")]
    );
    assert_eq!(
        reach("export GIT_WORK_TREE=/plane; gh pr merge 5")
            .unwrap()
            .targets,
        [PathBuf::from("/plane")]
    );
    assert!(reach("export GIT_DIR=$X; gh pr merge 5").is_err());
    // `cd` is read against the folder it runs in, except where CDPATH could send it elsewhere.
    assert!(reach("CDPATH=/anchored cd plane && gh pr merge 5").is_err());
    assert!(reach("export CDPATH=/anchored; cd plane; gh pr merge 5").is_err());
    let env = |name: &str| (name == "CDPATH").then(|| "/anchored".to_owned());
    assert!(reach_of("cd plane && gh pr merge 5", Path::new("/w"), &env).is_err());
    assert!(reach_of("gh pr merge 5", Path::new("/w"), &env).is_ok());
}

/// R2-1: a repo is a plane's by the remotes of every repo the plane owns, not its root's alone.
#[test]
fn a_repo_a_planes_workspace_owns_is_the_planes_by_that_repo_s_remote() {
    let (_home, planes) = machine();
    let root = plane(planes.path(), "p");
    let remote = |url: &str| format!("[remote \"origin\"]\n\turl = {url}\n");
    std::fs::write(
        root.join(".git").join("config"),
        remote("https://github.com/acme/purlis-plane.git"),
    )
    .unwrap();
    let repo = root.join("workspaces/ws/purlis");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::write(
        repo.join(".git").join("config"),
        remote("git@github.com:acme/purlis.git"),
    )
    .unwrap();
    let guest = root.join("workspaces/ws/repos/other");
    std::fs::create_dir_all(guest.join(".git")).unwrap();
    std::fs::write(
        guest.join(".git").join("config"),
        remote("https://gitlab.com/grp/other.git"),
    )
    .unwrap();
    answers::register(&root).expect("registered");
    outside_the_app();
    let elsewhere = outside(planes.path());
    for named in ["acme/purlis", "acme/purlis-plane", "grp/other"] {
        let repos = [named.to_owned()];
        let whereabouts = Whereabouts {
            repos: &repos,
            ..at(&elsewhere)
        };
        assert_eq!(
            locate(&whereabouts),
            Locality::InPlane {
                root: root.clone(),
                anchored: true
            },
            "{named}"
        );
    }
    let repos = ["acme/unrelated".to_owned()];
    let whereabouts = Whereabouts {
        repos: &repos,
        ..at(&elsewhere)
    };
    assert_eq!(locate(&whereabouts), Locality::PlaneLess);
}

/// R2-5: the remotes of every folder a command names are read, not the working directory's alone.
#[test]
fn the_remote_of_a_folder_the_command_names_is_the_anchored_planes_too() {
    let (_home, planes) = machine();
    let root = plane(planes.path(), "p");
    let config = "[remote \"origin\"]\n\turl = https://github.com/acme/web.git\n";
    std::fs::write(root.join(".git").join("config"), config).unwrap();
    answers::register(&root).expect("registered");
    outside_the_app();
    let clone = outside(planes.path()).join("clone");
    std::fs::create_dir_all(clone.join(".git")).unwrap();
    std::fs::write(clone.join(".git").join("config"), config).unwrap();
    let elsewhere = outside(planes.path());
    let named = [clone];
    let whereabouts = Whereabouts {
        targets: &named,
        ..at(&elsewhere)
    };
    assert!(matches!(
        locate(&whereabouts),
        Locality::InPlane { anchored: true, .. }
    ));
}

/// A merge asked by a process the app did not start is decided by the shipped default under
/// `strict`, in unattended terms: there is no window to ask, and the policy's answer is its own.
#[test]
fn a_process_that_is_no_chats_asks_with_the_context_a_host_would_make_for_it() {
    let request = process_request("merge", ("Branch", "acme/web#1"));
    assert_eq!(request.context["attended"], false);
    assert_eq!(request.context["via"], "hook");
    assert_eq!(request.context["sandbox"], "off");
    // It fits the schema: a decision names a rule and is not a schema error.
    let said = crate::policy::decide::decide_locally(&request);
    assert!(!said.rules_said().is_empty(), "{said:?}");
}
