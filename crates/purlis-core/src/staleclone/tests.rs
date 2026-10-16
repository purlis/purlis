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
            pushes_kept: Vec::new(),
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
fn a_push_url_naming_the_old_place_moves_with_origin() {
    // However it is spelled, a push URL of the old place goes with origin: pushes then go
    // where origin now fetches from.
    for pushes in [
        &["https://github.com/acme/widget.git"][..],
        &["git@github.com:acme/widget.git"][..],
        &[
            "https://github.com/Acme/widget",
            "ssh://git@github.com/acme/widget.git",
        ][..],
    ] {
        let seen = moved_with_pushes(pushes);
        assert_eq!(
            judged(&seen),
            [Stale::RemoteMoved {
                origin: "https://github.com/acme/widget.git".into(),
                now: "https://github.com/purlis/widget.git".into(),
                pushes_kept: Vec::new(),
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
fn a_push_url_naming_another_place_is_left_and_said() {
    for pushes in [
        &["https://github.com/someone/widget.git"][..],
        // One of them elsewhere: every one is left, and every one is said.
        &[
            "https://github.com/acme/widget.git",
            "https://gitlab.example/acme/widget.git",
        ][..],
        // One purlis cannot read as a place is not the old place.
        &["/srv/mirror/widget.git"][..],
    ] {
        let seen = moved_with_pushes(pushes);
        assert_eq!(
            judged(&seen),
            [Stale::RemoteMoved {
                origin: "https://github.com/acme/widget.git".into(),
                now: "https://github.com/purlis/widget.git".into(),
                pushes_kept: pushes.iter().map(|&url| url.to_owned()).collect(),
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
