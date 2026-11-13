// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// A clone on its default branch, clean, with nothing only it holds and no move heard.
fn settled() -> Seen {
    Seen {
        workspace: "smart-ide".into(),
        repo: "widget".into(),
        branch: Some("main".into()),
        default: Some("main".into()),
        origin: Some("https://github.com/acme/widget.git".into()),
        moved_to: None,
        push_urls: Vec::new(),
        clean: Some(true),
        only_here: Some(0),
    }
}

fn on(branch: &str) -> Seen {
    Seen {
        branch: Some(branch.into()),
        ..settled()
    }
}

#[test]
fn a_clone_on_its_default_branch_is_not_stale() {
    assert_eq!(judged(&settled()), []);
}

#[test]
fn a_clone_left_on_another_branch_is_stale() {
    assert_eq!(
        judged(&on("chat-3")),
        [Stale::OffDefault {
            branch: "chat-3".into(),
            default: "main".into(),
        }]
    );
}

#[test]
fn any_branch_but_the_default_counts_whoever_checked_it_out() {
    // purlis cannot tell a chat's plain checkout from the person's: the rule is the branch.
    assert_eq!(
        judged(&on("ci/sccache-stats-notice")),
        [Stale::OffDefault {
            branch: "ci/sccache-stats-notice".into(),
            default: "main".into(),
        }]
    );
}

#[test]
fn a_clone_whose_default_branch_purlis_cannot_read_is_not_judged_on_its_branch() {
    // No `origin/HEAD`: there is no branch to name as the one it should be on.
    let seen = Seen {
        default: None,
        ..on("chat-3")
    };
    assert_eq!(judged(&seen), []);
}

#[test]
fn a_detached_clone_is_not_judged_on_its_branch() {
    let seen = Seen {
        branch: None,
        ..settled()
    };
    assert_eq!(judged(&seen), []);
}

#[test]
fn a_remote_the_forge_said_moved_is_stale() {
    let seen = Seen {
        moved_to: Some("https://github.com/purlis/widget.git".into()),
        ..settled()
    };
    assert_eq!(
        judged(&seen),
        [Stale::RemoteMoved {
            origin: "https://github.com/acme/widget.git".into(),
            now: "https://github.com/purlis/widget.git".into(),
            pushes: Pushes::Origin,
        }]
    );
}

#[test]
fn a_remote_already_at_the_new_address_is_not_stale_however_it_is_spelled() {
    for origin in [
        "https://github.com/purlis/widget.git",
        "https://github.com/Purlis/widget",
        "git@github.com:purlis/widget.git",
    ] {
        let seen = Seen {
            origin: Some(origin.into()),
            moved_to: Some("https://github.com/purlis/widget.git".into()),
            ..settled()
        };
        assert_eq!(judged(&seen), [], "{origin}");
    }
}

#[test]
fn a_move_to_another_host_or_a_url_purlis_would_not_hand_git_is_never_offered() {
    for now in [
        "https://elsewhere.example/purlis/widget.git",
        "http://github.com/purlis/widget.git",
        "https://git@github.com/purlis/widget.git",
        "https://github.com/purlis/wid get.git",
        "ssh://git@github.com/purlis/widget.git",
        "-c core.sshCommand=x",
        "",
        // Dot segments a client would resolve to another path than the one shown.
        "https://github.com/acme/widget/../../evil/x",
        "https://github.com/./purlis/widget.git",
        "https://github.com/purlis/../widget.git",
        // A query, a fragment, an escape: nothing but the path is taken.
        "https://github.com/purlis/widget.git?x=1",
        "https://github.com/purlis/widget.git#x",
        "https://github.com/purlis/%77idget.git",
        // Another port than origin's, or one that is not a number.
        "https://github.com:8443/purlis/widget.git",
        "https://github.com:/purlis/widget.git",
        "https://github.com:x/purlis/widget.git",
        // An owner alone, or an empty segment.
        "https://github.com/widget.git",
        "https://github.com/purlis//widget.git",
    ] {
        let seen = Seen {
            moved_to: Some(now.into()),
            ..settled()
        };
        assert_eq!(judged(&seen), [], "{now}");
    }
}

#[test]
fn a_clone_can_be_stale_both_ways_at_once() {
    let seen = Seen {
        moved_to: Some("https://github.com/purlis/widget.git".into()),
        ..on("task-7")
    };
    assert_eq!(judged(&seen).len(), 2);
}

#[test]
fn the_forges_redirect_is_read_off_a_fetch() {
    let said = "warning: redirecting to https://github.com/purlis/widget.git/\n\
                From https://github.com/purlis/widget\n";
    assert_eq!(
        redirected(said).as_deref(),
        Some("https://github.com/purlis/widget.git")
    );
    assert_eq!(redirected("From https://github.com/acme/widget\n"), None);
    // Only git's own line, at the start of a line: a remote's message is not git's warning.
    assert_eq!(
        redirected("remote: warning: redirecting to https://github.com/x/y.git/\n"),
        None
    );
}

// The fix ---------------------------------------------------------------------------------

#[test]
fn a_clean_clone_whose_branch_is_kept_elsewhere_is_switched_to_its_default() {
    assert_eq!(
        step(&on("chat-3"), Fix::Branch),
        Ok(Step::Switch { to: "main".into() })
    );
}

#[test]
fn the_branch_fix_refuses_a_tree_with_changes() {
    let seen = Seen {
        clean: Some(false),
        ..on("chat-3")
    };
    assert_eq!(step(&seen, Fix::Branch), Err(Blocked::Changes));
}

#[test]
fn the_branch_fix_refuses_a_tree_it_could_not_read() {
    let seen = Seen {
        clean: None,
        ..on("chat-3")
    };
    assert_eq!(step(&seen, Fix::Branch), Err(Blocked::Unread));
}

#[test]
fn the_branch_fix_refuses_a_branch_holding_commits_nothing_else_has() {
    let seen = Seen {
        only_here: Some(2),
        ..on("chat-3")
    };
    assert_eq!(step(&seen, Fix::Branch), Err(Blocked::OnlyHere(2)));
    let unknown = Seen {
        only_here: None,
        ..on("chat-3")
    };
    assert_eq!(step(&unknown, Fix::Branch), Err(Blocked::Unread));
}

#[test]
fn a_fix_for_what_is_no_longer_stale_does_nothing() {
    assert_eq!(step(&settled(), Fix::Branch), Err(Blocked::Settled));
    assert_eq!(step(&settled(), Fix::Remote), Err(Blocked::Settled));
}

#[test]
fn the_remote_fix_sets_the_address_the_forge_named() {
    let seen = Seen {
        moved_to: Some("https://github.com/purlis/widget.git".into()),
        // A dirty tree does not stop a change of address: no file is touched.
        clean: Some(false),
        ..settled()
    };
    assert_eq!(
        step(&seen, Fix::Remote),
        Ok(Step::SetUrl {
            to: "https://github.com/purlis/widget.git".into(),
            drop_push: false,
        })
    );
}

#[test]
fn every_refusal_says_what_blocks_it() {
    for blocked in [
        Blocked::Settled,
        Blocked::Changes,
        Blocked::Unread,
        Blocked::OnlyHere(1),
        Blocked::OnlyHere(3),
    ] {
        let said = blocked.to_string();
        assert!(said.ends_with('.'), "{said}");
        assert!(!said.is_empty());
    }
    assert!(Blocked::OnlyHere(1).to_string().contains("1 commit "));
    assert!(Blocked::OnlyHere(3).to_string().contains("3 commits "));
}

#[test]
fn the_address_set_is_rebuilt_from_its_checked_parts() {
    let origin = "https://github.com/acme/widget.git";
    for (said, set) in [
        (
            "https://github.com/purlis/widget.git/",
            "https://github.com/purlis/widget.git",
        ),
        (
            "https://GitHub.com/purlis/widget",
            "https://github.com/purlis/widget.git",
        ),
        (
            "https://github.com/group/sub/widget.git",
            "https://github.com/group/sub/widget.git",
        ),
    ] {
        assert_eq!(rebuilt(origin, said).as_deref(), Some(set), "{said}");
    }
    // origin's own port is kept, and required.
    let on_a_port = "https://git.example:8443/acme/widget.git";
    assert_eq!(
        rebuilt(on_a_port, "https://git.example:8443/purlis/widget.git").as_deref(),
        Some("https://git.example:8443/purlis/widget.git")
    );
    assert_eq!(
        rebuilt(on_a_port, "https://git.example/purlis/widget.git"),
        None
    );
    // A user on origin is not carried to the new address.
    assert_eq!(
        rebuilt(
            "https://git@github.com/acme/widget.git",
            "https://github.com/purlis/widget.git"
        )
        .as_deref(),
        Some("https://github.com/purlis/widget.git")
    );
    // An SSH origin moves to the forge's HTTPS address on its host.
    assert_eq!(
        rebuilt(
            "git@github.com:acme/widget.git",
            "https://github.com/purlis/widget.git"
        )
        .as_deref(),
        Some("https://github.com/purlis/widget.git")
    );
}

#[test]
fn what_is_kept_is_what_the_fix_sets() {
    let seen = Seen {
        moved_to: Some("https://GitHub.com/purlis/widget/".into()),
        ..settled()
    };
    assert_eq!(
        step(&seen, Fix::Remote),
        Ok(Step::SetUrl {
            to: "https://github.com/purlis/widget.git".into(),
            drop_push: false,
        })
    );
}

fn moved_with_pushes(pushes: &[&str]) -> Seen {
    Seen {
        moved_to: Some("https://github.com/purlis/widget.git".into()),
        push_urls: pushes.iter().map(|&url| url.to_owned()).collect(),
        ..settled()
    }
}

#[test]
fn a_push_url_of_the_old_address_moves_with_origin() {
    // origin's own address, however git would also read it: pushes then go where origin now
    // fetches from.
    for pushes in [
        &["https://github.com/acme/widget.git"][..],
        &[
            "https://GitHub.com/acme/widget/",
            "https://github.com/acme/widget",
        ][..],
    ] {
        let seen = moved_with_pushes(pushes);
        assert_eq!(
            judged(&seen),
            [Stale::RemoteMoved {
                origin: "https://github.com/acme/widget.git".into(),
                now: "https://github.com/purlis/widget.git".into(),
                pushes: Pushes::Follow,
            }],
            "{pushes:?}"
        );
        assert_eq!(
            step(&seen, Fix::Remote),
            Ok(Step::SetUrl {
                to: "https://github.com/purlis/widget.git".into(),
                drop_push: true,
            }),
            "{pushes:?}"
        );
    }
}

#[test]
fn a_push_url_of_an_ssh_origin_moves_with_it_in_either_ssh_spelling() {
    let seen = Seen {
        origin: Some("git@github.com:acme/widget.git".into()),
        ..moved_with_pushes(&["ssh://git@github.com/acme/widget.git"])
    };
    assert_eq!(
        step(&seen, Fix::Remote),
        Ok(Step::SetUrl {
            to: "https://github.com/purlis/widget.git".into(),
            drop_push: true,
        })
    );
}

#[test]
fn a_push_url_naming_another_place_or_reaching_it_another_way_is_left_and_said() {
    for pushes in [
        &["https://github.com/someone/widget.git"][..],
        // The same place over another transport: dropping it would change how pushes go.
        &["git@github.com:acme/widget.git"][..],
        // The same place as another user: dropping it would change who pushes.
        &["https://git@github.com/acme/widget.git"][..],
        // A path differing only by case is another repository on a forge that tells them apart.
        &["https://github.com/Acme/widget.git"][..],
        // One of them elsewhere: every one is left, and every one is said.
        &[
            "https://github.com/acme/widget.git",
            "https://gitlab.example/acme/widget.git",
        ][..],
        // One purlis cannot read as an address is not origin's.
        &["/srv/mirror/widget.git"][..],
    ] {
        let seen = moved_with_pushes(pushes);
        assert_eq!(
            judged(&seen),
            [Stale::RemoteMoved {
                origin: "https://github.com/acme/widget.git".into(),
                now: "https://github.com/purlis/widget.git".into(),
                pushes: Pushes::Kept(pushes.iter().map(|&url| url.to_owned()).collect()),
            }],
            "{pushes:?}"
        );
        assert_eq!(
            step(&seen, Fix::Remote),
            Ok(Step::SetUrl {
                to: "https://github.com/purlis/widget.git".into(),
                drop_push: false,
            }),
            "{pushes:?}"
        );
    }
}

#[test]
fn what_is_found_never_prints_an_address_whole() {
    let seen = moved_with_pushes(&["https://u:s3cret@mirror/acme/widget.git"]);
    let found = Findings {
        found: vec![Found {
            workspace: "smart-ide".into(),
            repo: "widget".into(),
            stale: judged(&seen),
        }],
        unread: Vec::new(),
    };
    for printed in [format!("{seen:?}"), format!("{found:?}")] {
        assert!(!printed.contains("s3cret"), "{printed}");
        assert!(printed.contains("mirror/acme/widget"), "{printed}");
    }
}

// staleClones reads the shared standing (H6c).

/// A plane whose workspace `alpha` holds clone `widget`, left on branch `topic` while its
/// remote's default is `main`: stale for its branch.
fn plane_with_a_clone_off_its_default()
-> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let (dir, plane) = a_plane();
    let clone = a_clone(&plane, "alpha", "widget");
    git_in(&clone, &["checkout", "-q", "-b", "topic"]);
    (dir, plane, clone)
}

fn a_plane() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let plane = dir.path().canonicalize().unwrap();
    std::fs::write(plane.join("charter.toml"), "schema = 1\n").unwrap();
    (dir, plane)
}

fn git_in(clone: &std::path::Path, args: &[&str]) {
    let done = crate::testgit::run(clone, args);
    assert!(done.ok(), "git {args:?} failed: {done:?}");
}

/// Clone `name` of workspace `ws`, on `main`, the remote's default.
fn a_clone(plane: &std::path::Path, ws: &str, name: &str) -> std::path::PathBuf {
    let clone = plane.join("workspaces").join(ws).join(name);
    std::fs::create_dir_all(&clone).unwrap();
    let git = |args: &[&str]| git_in(&clone, args);
    git(&["init", "-q", "-b", "main", "."]);
    // Its own identity: a CI runner has none of its own to commit with.
    git(&["config", "user.email", "t@e.invalid"]);
    git(&["config", "user.name", "t"]);
    std::fs::write(clone.join("README.md"), "one\n").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "one"]);
    git(&[
        "remote",
        "add",
        "origin",
        "https://github.com/acme/widget.git",
    ]);
    git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    git(&[
        "symbolic-ref",
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/main",
    ]);
    clone
}

fn statuses_in(clone: &std::path::Path) -> usize {
    crate::worktree::git::tally::asked(clone)
        .iter()
        .filter(|args| args.iter().any(|arg| arg == "status"))
        .count()
}

#[test]
fn a_clone_whose_standing_is_held_is_judged_with_no_status_of_its_own() {
    let (_dir, plane, clone) = plane_with_a_clone_off_its_default();
    let repo = repos::Repo {
        name: "widget".into(),
        path: clone.clone(),
    };
    crate::reposave::shared_standing(&plane, "alpha", &repo);
    let before = statuses_in(&clone);

    let got = found(&plane);

    assert_eq!(statuses_in(&clone), before, "a status of its own");
    assert!(got.unread.is_empty(), "{:?}", got.unread);
    assert_eq!(got.found.len(), 1, "{:?}", got.found);
    assert_eq!(
        (got.found[0].workspace.as_str(), got.found[0].repo.as_str()),
        ("alpha", "widget")
    );
    assert!(
        got.found[0]
            .stale
            .iter()
            .any(|stale| matches!(stale, Stale::OffDefault { .. })),
        "{:?}",
        got.found[0].stale
    );
}

#[test]
fn a_clone_with_changes_is_still_read_as_changed_from_the_shared_standing() {
    let (_dir, plane, clone) = plane_with_a_clone_off_its_default();
    std::fs::write(clone.join("README.md"), "edited\n").unwrap();
    let repo = repos::Repo {
        name: "widget".into(),
        path: clone.clone(),
    };

    let seen = read_shared(&plane, "alpha", &repo);

    assert_eq!(seen.branch.as_deref(), Some("topic"));
    assert_eq!(seen.clean, Some(false));
    assert_eq!(seen, read("alpha", &repo));
}

// A look at a clone that cannot have changed runs no git (felt speed round 3, H8).

fn gits_in(clone: &std::path::Path) -> usize {
    crate::worktree::git::tally::asked(clone).len()
}

fn stale_of(plane: &std::path::Path) -> Vec<Vec<Stale>> {
    found(plane)
        .found
        .into_iter()
        .map(|one| one.stale)
        .collect()
}

#[test]
fn a_second_look_at_an_unchanged_clone_runs_no_git() {
    let (_dir, plane) = a_plane();
    let clone = a_clone(&plane, "alpha", "widget");
    assert_eq!(stale_of(&plane), Vec::<Vec<Stale>>::new());
    let before = gits_in(&clone);

    assert_eq!(stale_of(&plane), Vec::<Vec<Stale>>::new());

    assert_eq!(
        gits_in(&clone),
        before,
        "{:?}",
        crate::worktree::git::tally::asked(&clone)
    );
}

#[test]
fn a_second_look_at_an_unchanged_stale_clone_runs_no_git_and_still_finds_it() {
    let (_dir, plane, clone) = plane_with_a_clone_off_its_default();
    assert_eq!(found(&plane).found.len(), 1);
    let before = gits_in(&clone);

    assert_eq!(found(&plane).found.len(), 1);

    assert_eq!(gits_in(&clone), before);
}

#[test]
fn a_switch_of_branch_is_seen_on_the_next_look() {
    let (_dir, plane) = a_plane();
    let clone = a_clone(&plane, "alpha", "widget");
    assert_eq!(stale_of(&plane), Vec::<Vec<Stale>>::new());

    git_in(&clone, &["checkout", "-q", "-b", "topic"]);

    assert_eq!(
        stale_of(&plane),
        [vec![Stale::OffDefault {
            branch: "topic".into(),
            default: "main".into(),
        }]]
    );
}

#[test]
fn a_branch_born_under_an_unchanged_head_is_seen_on_the_next_look() {
    // HEAD names `topic` before it has a commit: no branch to judge. The commit that makes it
    // writes the branch's ref and leaves HEAD as it was.
    let (_dir, plane) = a_plane();
    let clone = a_clone(&plane, "alpha", "widget");
    git_in(&clone, &["symbolic-ref", "HEAD", "refs/heads/topic"]);
    assert_eq!(stale_of(&plane), Vec::<Vec<Stale>>::new());

    git_in(
        &clone,
        &["update-ref", "refs/heads/topic", "refs/heads/main"],
    );

    assert_eq!(stale_of(&plane).len(), 1);
}

#[test]
fn a_new_default_branch_is_seen_on_the_next_look() {
    let (_dir, plane) = a_plane();
    let clone = a_clone(&plane, "alpha", "widget");
    git_in(&clone, &["update-ref", "refs/remotes/origin/trunk", "HEAD"]);
    assert_eq!(stale_of(&plane), Vec::<Vec<Stale>>::new());

    git_in(
        &clone,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/trunk",
        ],
    );

    assert_eq!(
        stale_of(&plane),
        [vec![Stale::OffDefault {
            branch: "main".into(),
            default: "trunk".into(),
        }]]
    );
}

#[test]
fn a_change_to_the_clones_config_is_seen_on_the_next_look() {
    let (_dir, plane) = a_plane();
    let clone = a_clone(&plane, "alpha", "widget");
    assert_eq!(stale_of(&plane), Vec::<Vec<Stale>>::new());

    git_in(
        &clone,
        &["config", MOVED_KEY, "https://github.com/purlis/widget.git"],
    );

    assert!(
        matches!(&stale_of(&plane)[..], [one] if matches!(one[..], [Stale::RemoteMoved { .. }])),
        "{:?}",
        stale_of(&plane)
    );
}

#[test]
fn a_config_edited_in_place_to_the_same_size_and_time_is_seen_on_the_next_look() {
    // No stamp of time or size: what the file says is what is compared.
    let (_dir, plane) = a_plane();
    let clone = a_clone(&plane, "alpha", "widget");
    git_in(
        &clone,
        &["config", MOVED_KEY, "https://github.com/purlis/widget.git"],
    );
    let moved_to = |stale: Vec<Vec<Stale>>| match &stale[..] {
        [one] => match &one[..] {
            [Stale::RemoteMoved { now, .. }] => now.clone(),
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    };
    assert_eq!(
        moved_to(stale_of(&plane)),
        "https://github.com/purlis/widget.git"
    );
    let config = clone.join(".git/config");
    let was = std::fs::metadata(&config).unwrap();

    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, text.replace("purlis/widget", "purlis/gadget")).unwrap();
    let file = std::fs::File::options().write(true).open(&config).unwrap();
    file.set_modified(was.modified().unwrap()).unwrap();
    drop(file);
    assert_eq!(std::fs::metadata(&config).unwrap().len(), was.len());

    assert_eq!(
        moved_to(stale_of(&plane)),
        "https://github.com/purlis/gadget.git"
    );
}

#[test]
fn a_clone_whose_config_includes_another_file_is_read_in_full_each_time() {
    let (_dir, plane) = a_plane();
    let clone = a_clone(&plane, "alpha", "widget");
    git_in(&clone, &["config", "include.path", "elsewhere.conf"]);
    assert_eq!(stale_of(&plane), Vec::<Vec<Stale>>::new());
    let before = gits_in(&clone);

    assert_eq!(stale_of(&plane), Vec::<Vec<Stale>>::new());

    assert!(gits_in(&clone) > before, "an included file is not stamped");
}
