// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::host::Glance;
use purlis_core::state::State;

#[test]
fn a_stale_repo_is_one_row_with_both_findings_and_no_address_carries_a_user() {
    let findings = staleclone::Findings {
        found: vec![staleclone::Found {
            workspace: "smart-ide".into(),
            repo: "widget".into(),
            stale: vec![
                Stale::OffDefault {
                    branch: "chat-3".into(),
                    default: "main".into(),
                },
                Stale::RemoteMoved {
                    origin: "https://git@github.com/acme/widget.git".into(),
                    now: "https://github.com/Purlis/widget.git".into(),
                    pushes: staleclone::Pushes::Kept(vec![
                        "https://bot:tok@gitlab.example/mirror/widget.git".into(),
                        "git@gitlab.example:mirror/widget.git".into(),
                    ]),
                },
            ],
        }],
        unread: vec!["could not read workspace 'beta' (gone).".into()],
    };
    assert_eq!(
        shown(findings),
        StaleClones {
            stale: vec![StaleClone {
                workspace: "smart-ide".into(),
                repo: "widget".into(),
                branch: Some(StaleBranch {
                    branch: "chat-3".into(),
                    default: "main".into(),
                }),
                remote: Some(StaleRemote {
                    origin: "github.com/acme/widget".into(),
                    // Whole and as set, so the window shows what the Fix sets.
                    now: "https://github.com/Purlis/widget.git".into(),
                    // The push addresses the Fix leaves, as places, each once: no user is drawn.
                    pushes_follow: false,
                    pushes_kept: vec!["gitlab.example/mirror/widget".into()],
                }),
            }],
            unread: vec!["could not read workspace 'beta' (gone).".into()],
        }
    );
}

#[test]
fn a_push_url_that_follows_origin_is_said_and_none_is_kept() {
    let findings = staleclone::Findings {
        found: vec![staleclone::Found {
            workspace: "smart-ide".into(),
            repo: "widget".into(),
            stale: vec![Stale::RemoteMoved {
                origin: "https://github.com/acme/widget.git".into(),
                now: "https://github.com/purlis/widget.git".into(),
                pushes: staleclone::Pushes::Follow,
            }],
        }],
        unread: Vec::new(),
    };
    let remote = shown(findings).stale[0]
        .remote
        .clone()
        .expect("a remote finding");
    assert!(remote.pushes_follow);
    assert_eq!(remote.pushes_kept, Vec::<String>::new());
}

fn glance(state: State, asking: bool) -> Glance {
    Glance {
        state,
        asking,
        turns: 1,
        prompt: None,
        own_work: false,
    }
}

#[test]
fn a_chat_is_mid_turn_while_it_runs_asks_or_has_not_been_heard_from() {
    assert!(mid_turn(&glance(State::Running, false)));
    assert!(mid_turn(&glance(State::Unknown, false)));
    // Waiting on a permission or a question is still inside its turn.
    assert!(mid_turn(&glance(State::Waiting, true)));
    // A turn that ended, or a chat that ended, is not.
    assert!(!mid_turn(&glance(State::Waiting, false)));
    assert!(!mid_turn(&glance(State::Done, false)));
    assert!(!mid_turn(&glance(State::Failed, false)));
}
