// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Who looked at a merge's head, and who pushed it** (T1-41; R-122, R-124, R-125, Q210,
//! Q268). A merge always takes two presses (Q268, Q269). What the once-more question says comes
//! from whether the person viewed the head in purlis, or the forge holds an approving review of
//! it by someone who is neither its author nor an app or a bot. The viewed head is held in the
//! app's memory alone, so a new app knows none. A head the chat did not push, by the pushes
//! purlis recorded for it, is flagged. A watched head that moves retires its ask for one at the
//! new head, and says by how many commits.

use purlis_core::forge::checks::{Checks, Ci};
use purlis_core::forge::pr::{Review, ReviewState, Reviewer};
use purlis_core::mergehold::{
    self, Facts, HeadSha, Holds, Look, MergeKey, PRESSES, Pushes, Repo, Ruleset, Viewed,
};

const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";
const OLDER: &str = "89abcdef0123456789abcdef0123456789abcdef";
const MOVED: &str = "fedcba9876543210fedcba9876543210fedcba98";

fn head(sha: &str) -> HeadSha {
    sha.try_into().unwrap()
}

fn repo() -> Repo {
    "o/r".try_into().unwrap()
}

fn key(at: &str) -> MergeKey {
    MergeKey {
        repo: repo(),
        number: 12,
        head: head(at),
        base: "main".try_into().unwrap(),
    }
}

fn facts(read_at: u64) -> Facts {
    Facts {
        title: "Add the parser".to_owned(),
        author: "octo".to_owned(),
        checks: Checks {
            total: Some(1),
            ci: Ci::Passed,
            why: None,
            failed: Vec::new(),
        },
        ruleset: Ruleset::None,
        approved: false,
        read_at,
    }
}

fn review(login: &str, by: Reviewer, state: ReviewState, at: &str) -> Review {
    Review {
        login: login.to_owned(),
        by,
        state,
        commit: at.to_owned(),
    }
}

/// R-122: the forge's reviews, judged against the head and the request's author. Only a person
/// who is not the author, approving this very head, counts.
#[test]
fn the_press_count_comes_from_the_viewed_sha_and_the_forges_reviews() {
    purlis_core::unsteered!();
    let at = head(HEAD);
    let approves = |reviews: &[Review]| mergehold::approves(reviews, &at, "octo");

    let by_another = [review("ana", Reviewer::Person, ReviewState::Approved, HEAD)];
    let by_the_author = [review(
        "Octo",
        Reviewer::Person,
        ReviewState::Approved,
        HEAD,
    )];
    let by_an_app = [review(
        "ci-app[bot]",
        Reviewer::App,
        ReviewState::Approved,
        HEAD,
    )];
    let by_a_bot = [review(
        "deploy-bot",
        Reviewer::Bot,
        ReviewState::Approved,
        HEAD,
    )];
    let on_an_older_head = [review(
        "ana",
        Reviewer::Person,
        ReviewState::Approved,
        OLDER,
    )];
    let taken_back = [
        review("ana", Reviewer::Person, ReviewState::Approved, HEAD),
        review("ana", Reviewer::Person, ReviewState::ChangesRequested, HEAD),
    ];
    let a_comment_after = [
        review("ana", Reviewer::Person, ReviewState::Approved, HEAD),
        review("ana", Reviewer::Person, ReviewState::Commented, HEAD),
    ];
    assert!(approves(&by_another), "another person approved this head");
    assert!(!approves(&by_the_author), "the author's own approval");
    assert!(!approves(&by_an_app), "an app's approval");
    assert!(!approves(&by_a_bot), "a bot's approval");
    assert!(!approves(&on_an_older_head), "an approval of an older head");
    assert!(
        !approves(&taken_back),
        "a later request for changes takes it back"
    );
    assert!(approves(&a_comment_after), "a comment takes nothing back");

    // The viewed head is held in memory, per request.
    let viewed = Viewed::default();
    assert_eq!(viewed.of(&repo(), 12), None);
    viewed.mark(&repo(), 12, &at);
    assert_eq!(viewed.of(&repo(), 12), Some(at.clone()));
    assert_eq!(
        viewed.of(&repo(), 13),
        None,
        "another request is not viewed"
    );

    let look = |viewed: &Viewed, approved: bool, since: Option<u32>| {
        Look::of(viewed.of(&repo(), 12).as_ref(), &at, approved, since)
    };
    assert_eq!(look(&viewed, false, None), Look::Viewed);
    assert_eq!(look(&Viewed::default(), true, None), Look::Approved);
    assert_eq!(
        look(&Viewed::default(), false, None),
        Look::NotViewed { since: None }
    );
    let older = Viewed::default();
    older.mark(&repo(), 12, &head(OLDER));
    assert_eq!(
        look(&older, false, Some(2)),
        Look::NotViewed { since: Some(2) }
    );

    // A restart: a new app's memory holds no viewed head, and nothing on disk gives it one.
    let restarted = Viewed::default();
    assert_eq!(
        look(&restarted, false, None),
        Look::NotViewed { since: None }
    );

    // Whatever was looked at, a merge takes two presses (Q268, Q269).
    for one in [
        Look::Viewed,
        Look::Approved,
        Look::NotViewed { since: None },
        Look::NotViewed { since: Some(2) },
    ] {
        assert_eq!(one.presses(), PRESSES);
    }
    assert_eq!(PRESSES, 2);

    // The holds say it of each waiting ask: not viewed, then viewed once View changes marks it.
    let holds = Holds::default();
    holds.raise(4, key(HEAD), facts(1_000), 1_000);
    assert_eq!(holds.every()[0].look, Look::NotViewed { since: None });
    holds.view(&key(HEAD));
    assert_eq!(holds.every()[0].look, Look::Viewed);
    // A head the forge's approving review is of reads as approved, where it was not viewed.
    let approved = Holds::default();
    approved.raise(
        4,
        key(HEAD),
        Facts {
            approved: true,
            ..facts(1_000)
        },
        1_000,
    );
    assert_eq!(approved.every()[0].look, Look::Approved);
}

/// R-124: a head is the chat's own only where purlis recorded the chat pushing it.
#[test]
fn a_pr_head_the_chat_did_not_push_is_flagged_from_the_recorded_pushes() {
    purlis_core::unsteered!();
    let pushes = Pushes::default();
    pushes.record(4, &repo(), &head(HEAD));

    assert!(pushes.pushed(4, &repo(), &head(HEAD)), "chat 4 pushed it");
    assert!(
        !pushes.pushed(4, &repo(), &head(MOVED)),
        "another head is flagged"
    );
    assert!(
        !pushes.pushed(5, &repo(), &head(HEAD)),
        "another chat's push is not this chat's"
    );
    assert!(
        !pushes.pushed(4, &"o/other".try_into().unwrap(), &head(HEAD)),
        "the same commit in another repo is not this push"
    );

    pushes.forget(4);
    assert!(
        !pushes.pushed(4, &repo(), &head(HEAD)),
        "a closed chat's pushes go"
    );

    // Each waiting ask says whether its head is the chat's own push.
    let holds = Holds::default();
    holds.raise(4, key(HEAD), facts(1_000), 1_000);
    assert!(!holds.every()[0].pushed_here, "no push recorded: flagged");
    holds.pushed(4, &repo(), &head(HEAD));
    assert!(holds.every()[0].pushed_here, "the chat pushed this head");
    holds.raise(4, key(MOVED), facts(1_060), 1_060);
    assert!(
        !holds.every()[0].pushed_here,
        "a head the chat did not push is flagged"
    );
}

/// R-125: the watch reads every waiting ask's request; a head that moved retires the ask for
/// one at the new head, with the commits between them, and an unmoved head says nothing.
#[test]
fn a_moved_head_sends_an_update_for_the_open_merge_ask() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    holds.raise(4, key(HEAD), facts(1_000), 1_000);
    holds.view(&key(HEAD));

    let unmoved = holds.watch(&|_| Some((key(HEAD), facts(1_030))), &|_, _| Some(9));
    assert!(unmoved.is_empty(), "nothing moved: {unmoved:?}");

    let moved = holds.watch(&|_| Some((key(MOVED), facts(1_060))), &|from, to| {
        (from.as_str() == HEAD && to.as_str() == MOVED).then_some(2)
    });

    assert_eq!(moved.len(), 1, "one update: {moved:?}");
    assert_eq!(moved[0].session, 4);
    assert_eq!(moved[0].was, key(HEAD));
    assert_eq!(moved[0].now.head.as_str(), MOVED);
    assert_eq!(moved[0].commits, Some(2));
    let waiting = holds.every();
    assert_eq!(waiting.len(), 1, "the old head's ask is retired");
    assert_eq!(waiting[0].key, key(MOVED));
    assert!(!waiting[0].asked, "the new head is asked from the start");
    assert_eq!(
        waiting[0].look,
        Look::NotViewed { since: Some(2) },
        "2 commits since you looked"
    );

    // A forge that does not answer moves nothing.
    assert!(holds.watch(&|_| None, &|_, _| None).is_empty());
    assert_eq!(holds.every()[0].key, key(MOVED));
}
