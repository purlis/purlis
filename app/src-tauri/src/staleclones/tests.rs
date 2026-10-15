// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_stale_repo_is_one_row_with_both_findings_and_no_address_carries_a_user() {
    let found = vec![staleclone::Found {
        workspace: "smart-ide".into(),
        repo: "widget".into(),
        stale: vec![
            Stale::OnAChatsBranch {
                branch: "chat-3".into(),
                default: "main".into(),
            },
            Stale::RemoteMoved {
                origin: "https://git@github.com/acme/widget.git".into(),
                now: "https://github.com/purlis/widget.git".into(),
            },
        ],
    }];
    assert_eq!(
        shown(found),
        [StaleClone {
            workspace: "smart-ide".into(),
            repo: "widget".into(),
            branch: Some(StaleBranch {
                branch: "chat-3".into(),
                default: "main".into(),
            }),
            remote: Some(StaleRemote {
                origin: "github.com/acme/widget".into(),
                now: "github.com/purlis/widget".into(),
            }),
        }]
    );
}
