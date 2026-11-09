// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #5630: a forge that takes only signed commits refuses a chat's push in GitHub's words; the
//! refusal is told apart and kept as work waiting on signing, and Sign and retry signs the
//! branch's own commits, nothing already pushed, and sends it again.
//!
//! The forge here is a bare repository whose `pre-receive` hook refuses an unsigned commit with
//! GitHub's words for it, as a ruleset's *Require signed commits* does.

use super::*;
use purlis_core::commitsign::{self, Signer};
use purlis_core::signingsetup::retry;

/// The forge's hook: every new commit pushed must carry a signature, or the push is refused in
/// GitHub's words.
const TAKES_ONLY_SIGNED: &str = r#"#!/bin/sh
zero=0000000000000000000000000000000000000000
while read old new ref; do
  if [ "$old" = "$zero" ]; then range="$new --not --all"; else range="$old..$new"; fi
  for c in $(git rev-list $range); do
    if ! git cat-file commit "$c" | sed '/^$/q' | grep -q '^gpgsig'; then
      echo "error: GH013: Repository rule violations found for $ref." >&2
      echo "- Commits must have verified signatures." >&2
      exit 1
    fi
  done
done
"#;

fn takes_only_signed(w: &World) {
    let hooks = w.forge.join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let hook = hooks.join("pre-receive");
    std::fs::write(&hook, TAKES_ONLY_SIGNED).unwrap();
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn for_a_chat() -> Reach<'static> {
    Reach {
        for_a_chat: true,
        ..Reach::default()
    }
}

#[test]
fn a_push_refused_for_unsigned_commits_is_told_apart_and_sign_and_retry_sends_it_signed() {
    purlis_core::unsteered!();
    let w = world();
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    // One commit pushed before the forge took only signed ones: it is never written again.
    commit_in(&piece, "before.txt", "before\n");
    let (code, said_now) = push(&w, "p1");
    assert_eq!(code, 0, "{said_now}");
    let pushed_before = out(&piece, &["rev-parse", "HEAD"]);
    takes_only_signed(&w);
    commit_in(&piece, "one.txt", "one\n");
    commit_in(&piece, "two.txt", "two\n");

    let mut lines = Vec::new();
    let (code, unsigned) =
        brokered(|| pieceupdate::push_told(&w.at("p1"), &for_a_chat(), &mut |l| lines.push(l)));
    let told = said(&lines);
    assert_eq!(code, 1, "{told}");
    let refused = unsigned.expect("the refusal is told apart");
    assert_eq!(refused.branch, "p1");
    assert!(told.contains("only accepts signed commits here"), "{told}");
    assert!(told.contains("A chat can't set signing up"), "{told}");
    assert_eq!(forge_has(&w, "p1"), pushed_before);

    // Another refusal is not this one.
    let other = retry::refused_unsigned(
        purlis_core::forge::Kind::GitHub,
        "error: GH006: Protected branch update failed.\nAt least 1 approving review is required",
    );
    assert!(!other);

    // Kept as waiting, as the app keeps it for the window.
    retry::note_refused(
        &w.plane,
        retry::Waiting {
            ws: WS.to_owned(),
            repo: REPO.to_owned(),
            piece: "p1".to_owned(),
            branch: refused.branch.clone(),
            kind: refused.kind,
            host: refused.host.clone(),
            path: refused.path.clone(),
            commit: refused.commit.clone(),
            chat_id: Some("01JCHAT3".to_owned()),
            at: 1,
            sent: false,
            why: None,
        },
    )
    .unwrap();
    let waiting = retry::waiting(&w.plane);
    assert_eq!(waiting.len(), 1);

    let signer = Signer::generate().unwrap();
    let mut lines = Vec::new();
    let retried = brokered(|| {
        retry::sign_and_retry(
            &w.plane,
            &waiting[0],
            &signer,
            Some("chosen@e.invalid"),
            &mut |l| lines.push(l),
        )
    })
    .unwrap_or_else(|why| panic!("{why}\n{}", said(&lines)));
    assert_eq!(
        retried.signed, 2,
        "only the two commits the forge has none of"
    );
    let head = out(&piece, &["rev-parse", "HEAD"]);
    assert_eq!(retried.head, head);
    assert_eq!(forge_has(&w, "p1"), head, "sent again");
    // Each of the two is signed now, and what was pushed before is as it was.
    for id in [head.clone(), out(&piece, &["rev-parse", "HEAD~1"])] {
        assert!(commitsign::is_signed(&piece, &id), "{id} is signed");
    }
    assert_eq!(out(&piece, &["rev-parse", "HEAD~2"]), pushed_before);
    // Made again as the email the person chose (#5631), author and committer alike.
    assert_eq!(
        out(&piece, &["log", "-2", "--format=%ae %ce"]),
        "chosen@e.invalid chosen@e.invalid\nchosen@e.invalid chosen@e.invalid"
    );
    assert!(!commitsign::is_signed(&piece, &pushed_before));
    // The work no longer waits: it shows as signed and sent.
    assert!(retry::waiting(&w.plane).iter().all(|one| one.sent));
}
