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
        chat_branches: ["chat-3".to_string(), "task-7".to_string()].into(),
        origin: Some("https://github.com/acme/widget.git".into()),
        moved_to: None,
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
fn a_clone_left_on_a_branch_purlis_cut_for_a_chat_is_stale() {
    assert_eq!(
        judged(&on("chat-3")),
        [Stale::OnAChatsBranch {
            branch: "chat-3".into(),
            default: "main".into(),
        }]
    );
}

#[test]
fn a_clone_on_a_branch_no_chat_was_cut_for_is_left_alone() {
    // The person's own branch in the shared clone is theirs to be on.
    assert_eq!(judged(&on("my-feature")), []);
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
            to: "https://github.com/purlis/widget.git".into()
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
