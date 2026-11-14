// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Who looked at a merge's head, and who pushed it** (T1-41; R-122, R-124, R-125, Q210,
//! Q268, Q277). A merge always takes two presses (Q268, Q269). What the once-more question says
//! comes from whether the person viewed the head in purlis, or the forge holds an approving
//! review of it by someone who is neither its author nor an app or a bot, who is named. The
//! viewed head is held in the app's memory alone, so a new app knows none. A head the chat did
//! not push, by the pushes purlis recorded for it, is flagged. Both are kept by the forge's host
//! and the repo: two forges' `o/r` are two repos. A head the person viewed an older head of
//! says how many commits lie between, whichever path found the move. A watched head that moves
//! retires its ask for one at the new head, and facts that change at the same head are said too.

use std::sync::Arc;

use purlis_core::forge::checks::{Checks, Ci};
use purlis_core::forge::pr::{Review, ReviewState, Reviewer};
use purlis_core::forge::{Forge, Kind};
use purlis_core::mergehold::{
    self, Facts, HeadMoved, HeadSha, Holds, Look, MergeKey, OnHost, PRESSES, Pushes, Raised,
    Refreshed, Repo, Reread, Ruleset, Viewed, Watched,
};

const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";
const OLDER: &str = "89abcdef0123456789abcdef0123456789abcdef";
const MOVED: &str = "fedcba9876543210fedcba9876543210fedcba98";
const GITHUB: &str = "github.com";
const ENTERPRISE: &str = "ghe.example.com";

fn head(sha: &str) -> HeadSha {
    sha.try_into().unwrap()
}

fn repo() -> Repo {
    "o/r".try_into().unwrap()
}

fn on(host: &str) -> OnHost {
    OnHost::new(host, repo())
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
        approved_by: None,
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

/// Chat `session`'s ask for `key`, raised on the forge at `host`, as the app raises it.
fn raised(holds: &Holds, session: u32, key: MergeKey, read_at: u64, host: &str) -> Raised {
    let raised = holds.raise(session, key.clone(), facts(read_at), read_at);
    holds.raised_on(session, &key, host);
    raised
}

/// R-122: the forge's reviews, judged against the head and the request's author. Only a person
/// who is not the author, approving this very head, counts, and is named (Q277).
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
    // Kills: `approves` answering a fixed login, or the first reviewer's whoever approved.
    assert_eq!(
        approves(&by_another).as_deref(),
        Some("ana"),
        "another person approved this head, and is named"
    );
    assert_eq!(approves(&by_the_author), None, "the author's own approval");
    assert_eq!(approves(&by_an_app), None, "an app's approval");
    assert_eq!(approves(&by_a_bot), None, "a bot's approval");
    assert_eq!(
        approves(&on_an_older_head),
        None,
        "an approval of an older head"
    );
    assert_eq!(
        approves(&taken_back),
        None,
        "a later request for changes takes it back"
    );
    assert_eq!(
        approves(&a_comment_after).as_deref(),
        Some("ana"),
        "a comment takes nothing back"
    );

    // The viewed head is held in memory, per request and forge.
    let viewed = Viewed::default();
    assert_eq!(viewed.of(&on(GITHUB), 12), None);
    viewed.mark(&on(GITHUB), 12, &at);
    assert_eq!(viewed.of(&on(GITHUB), 12), Some(at.clone()));
    assert_eq!(
        viewed.of(&on(GITHUB), 13),
        None,
        "another request is not viewed"
    );

    let look = |viewed: &Viewed, approved_by: Option<&str>, since: Option<u32>| {
        Look::of(viewed.of(&on(GITHUB), 12).as_ref(), &at, approved_by, since)
    };
    assert_eq!(look(&viewed, None, None), Look::Viewed);
    assert_eq!(
        look(&Viewed::default(), Some("ana"), None),
        Look::Approved {
            by: "ana".to_owned()
        }
    );
    assert_eq!(
        look(&Viewed::default(), None, None),
        Look::NotViewed { since: None }
    );
    let older = Viewed::default();
    older.mark(&on(GITHUB), 12, &head(OLDER));
    assert_eq!(
        look(&older, None, Some(2)),
        Look::NotViewed { since: Some(2) }
    );

    // A restart: a new app's memory holds no viewed head, and nothing on disk gives it one.
    let restarted = Viewed::default();
    assert_eq!(
        look(&restarted, None, None),
        Look::NotViewed { since: None }
    );

    // Whatever was looked at, a merge takes two presses (Q268, Q269).
    for one in [
        Look::Viewed,
        Look::Approved {
            by: "ana".to_owned(),
        },
        Look::NotViewed { since: None },
        Look::NotViewed { since: Some(2) },
    ] {
        assert_eq!(one.presses(), PRESSES);
    }
    assert_eq!(PRESSES, 2);

    // The holds say it of each waiting ask: not viewed, then viewed once View changes marks it.
    let holds = Holds::default();
    raised(&holds, 4, key(HEAD), 1_000, GITHUB);
    assert_eq!(holds.every()[0].look, Look::NotViewed { since: None });
    holds.view(4, &key(HEAD));
    assert_eq!(holds.every()[0].look, Look::Viewed);
    // A head the forge's approving review is of reads as approved, by whom, where it was not
    // viewed.
    let approved = Holds::default();
    approved.raise(
        4,
        key(HEAD),
        Facts {
            approved_by: Some("ana".to_owned()),
            ..facts(1_000)
        },
        1_000,
    );
    approved.raised_on(4, &key(HEAD), GITHUB);
    assert_eq!(
        approved.every()[0].look,
        Look::Approved {
            by: "ana".to_owned()
        }
    );
}

/// R-124: a head is the chat's own only where purlis recorded the chat pushing it, on that
/// forge (Q277).
#[test]
fn a_pr_head_the_chat_did_not_push_is_flagged_from_the_recorded_pushes() {
    purlis_core::unsteered!();
    let pushes = Pushes::default();
    pushes.record(4, &on(GITHUB), &head(HEAD));

    assert!(
        pushes.pushed(4, &on(GITHUB), &head(HEAD)),
        "chat 4 pushed it"
    );
    assert!(
        !pushes.pushed(4, &on(GITHUB), &head(MOVED)),
        "another head is flagged"
    );
    assert!(
        !pushes.pushed(5, &on(GITHUB), &head(HEAD)),
        "another chat's push is not this chat's"
    );
    assert!(
        !pushes.pushed(
            4,
            &OnHost::new(GITHUB, "o/other".try_into().unwrap()),
            &head(HEAD)
        ),
        "the same commit in another repo is not this push"
    );

    pushes.forget(4);
    assert!(
        !pushes.pushed(4, &on(GITHUB), &head(HEAD)),
        "a closed chat's pushes go"
    );

    // Each waiting ask says whether its head is the chat's own push.
    let holds = Holds::default();
    raised(&holds, 4, key(HEAD), 1_000, GITHUB);
    assert!(!holds.every()[0].pushed_here, "no push recorded: flagged");
    holds.pushed(4, &on(GITHUB), &head(HEAD));
    assert!(holds.every()[0].pushed_here, "the chat pushed this head");
    raised(&holds, 4, key(MOVED), 1_060, GITHUB);
    assert!(
        !holds.every()[0].pushed_here,
        "a head the chat did not push is flagged"
    );
}

/// Q277: two forges' `o/r` are two repos. A head viewed, or pushed, of `o/r` on github.com says
/// nothing of `o/r` on another host, and the host is compared in any case. Kills: the viewed
/// heads and pushes keyed by the repo's path alone.
#[test]
fn a_viewed_head_and_a_push_on_one_forge_say_nothing_of_another_forges_repo_of_that_name() {
    purlis_core::unsteered!();
    let viewed = Viewed::default();
    viewed.mark(&on(GITHUB), 12, &head(HEAD));
    assert_eq!(viewed.of(&on(ENTERPRISE), 12), None, "another forge's o/r");
    assert_eq!(
        viewed.of(&on("GitHub.com"), 12),
        Some(head(HEAD)),
        "the same host, in another case"
    );

    let holds = Holds::default();
    raised(&holds, 4, key(HEAD), 1_000, GITHUB);
    raised(&holds, 5, key(HEAD), 1_000, ENTERPRISE);
    holds.view(4, &key(HEAD));
    holds.pushed(5, &on(GITHUB), &head(HEAD));

    let every = holds.every();
    let of = |session: u32| {
        every
            .iter()
            .find(|one| one.session == session)
            .expect("its ask")
    };
    assert_eq!(of(4).look, Look::Viewed);
    assert_eq!(
        of(5).look,
        Look::NotViewed { since: None },
        "viewed on github.com, not on the other forge"
    );
    assert!(
        !of(5).pushed_here,
        "chat 5 pushed to github.com's o/r, not to the forge its ask is on"
    );

    // An ask the app said no host of was viewed by nobody: View changes marks nothing.
    let nowhere = Holds::default();
    nowhere.raise(4, key(HEAD), facts(1_000), 1_000);
    nowhere.view(4, &key(HEAD));
    assert_eq!(nowhere.every()[0].look, Look::NotViewed { since: None });
}

/// R-123, Q277: "N commits since you looked" is counted from the head the person viewed,
/// whichever path found the move: here the chat's try again at a moved head, and a press's
/// re-read, neither of them the watch. Kills: a count kept only where the watch found the move.
#[test]
fn the_commits_since_you_looked_are_counted_on_every_path_that_finds_a_move() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    raised(&holds, 4, key(OLDER), 1_000, GITHUB);
    holds.view(4, &key(OLDER));
    assert!(
        holds.uncounted().is_empty(),
        "the viewed head is the ask's head"
    );

    // The chat tries again at a moved head: a new ask, which nobody has viewed.
    assert_eq!(raised(&holds, 4, key(HEAD), 1_060, GITHUB), Raised::New);
    assert_eq!(
        holds.uncounted(),
        [(on(GITHUB), head(OLDER), head(HEAD))],
        "the app asks the forge for the commits from the viewed head"
    );
    holds.ahead(&on(GITHUB), &head(OLDER), &head(HEAD), 3);
    assert_eq!(holds.every()[0].look, Look::NotViewed { since: Some(3) });
    assert!(holds.uncounted().is_empty(), "counted once");

    // A press's re-read finds the head moved again: the count is from the viewed head still.
    assert_eq!(
        holds.refresh(4, &key(HEAD), key(MOVED), facts(1_090), Reread::Presses),
        Ok(Refreshed::Moved)
    );
    assert_eq!(
        holds.uncounted(),
        [(on(GITHUB), head(OLDER), head(MOVED))],
        "the ask at the new head is on the forge the old one was raised on"
    );
    holds.ahead(&on(GITHUB), &head(OLDER), &head(MOVED), 5);
    assert_eq!(holds.every()[0].look, Look::NotViewed { since: Some(5) });

    // Viewed at the new head, nothing is counted, and the row says viewed.
    holds.view(4, &key(MOVED));
    assert_eq!(holds.every()[0].look, Look::Viewed);
    assert!(holds.uncounted().is_empty());
}

/// R-125: the watch reads every waiting ask's request; a head that moved retires the ask for
/// one at the new head, and the commits since the head the person viewed are counted; an
/// unmoved head with unchanged facts says nothing.
#[test]
fn a_moved_head_sends_an_update_for_the_open_merge_ask() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    raised(&holds, 4, key(HEAD), 1_000, GITHUB);
    holds.view(4, &key(HEAD));

    let unmoved = holds.watch(&|_| Some((key(HEAD), facts(1_030))));
    assert!(unmoved.is_empty(), "nothing moved or changed: {unmoved:?}");

    let moved = holds.watch(&|_| Some((key(MOVED), facts(1_060))));

    assert_eq!(
        moved,
        [Watched::Moved(HeadMoved {
            session: 4,
            was: key(HEAD),
            now: key(MOVED),
        })],
        "one update"
    );
    let waiting = holds.every();
    assert_eq!(waiting.len(), 1, "the old head's ask is retired");
    assert_eq!(waiting[0].key, key(MOVED));
    assert!(!waiting[0].asked, "the new head is asked from the start");
    assert_eq!(holds.uncounted(), [(on(GITHUB), head(HEAD), head(MOVED))]);
    holds.ahead(&on(GITHUB), &head(HEAD), &head(MOVED), 2);
    assert_eq!(
        holds.every()[0].look,
        Look::NotViewed { since: Some(2) },
        "2 commits since you looked"
    );

    // A forge that does not answer moves nothing.
    assert!(holds.watch(&|_| None).is_empty());
    assert_eq!(holds.every()[0].key, key(MOVED));
}

/// Q277: a fact that changes at the same head is said by the watch, so the row is redrawn, and
/// the ask's asked mark goes, so the person is asked again of what it shows now. Kills: the
/// watch reporting moves alone, and a same-head `Reread::Watches` keeping the mark.
#[test]
fn a_fact_that_changes_at_the_same_head_is_said_by_the_watch_and_clears_the_asked_mark() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    raised(&holds, 4, key(HEAD), 1_000, GITHUB);
    holds
        .refresh(4, &key(HEAD), key(HEAD), facts(1_010), Reread::AsksOnceMore)
        .expect("asked once more");
    assert!(holds.every()[0].asked);

    // A newer read that shows the same: nothing to redraw, and the mark stays.
    assert!(holds.watch(&|_| Some((key(HEAD), facts(1_020)))).is_empty());
    assert!(holds.every()[0].asked, "nothing the person saw changed");

    // The checks failed since: the row is redrawn and the person is asked again.
    let failed = Facts {
        checks: Checks {
            ci: Ci::Failed,
            ..facts(1_030).checks
        },
        ..facts(1_030)
    };
    assert_eq!(
        holds.watch(&|_| Some((key(HEAD), failed.clone()))),
        [Watched::Changed {
            session: 4,
            key: key(HEAD),
        }]
    );
    let waiting = holds.every();
    assert_eq!(waiting[0].facts.checks.ci, Ci::Failed, "the new read shows");
    assert!(!waiting[0].asked, "asked again of what it shows now");

    // An approval that came at the same head is a change too.
    holds
        .refresh(
            4,
            &key(HEAD),
            key(HEAD),
            failed.clone(),
            Reread::AsksOnceMore,
        )
        .expect("asked once more");
    let approved = Facts {
        approved_by: Some("ana".to_owned()),
        read_at: 1_040,
        ..failed
    };
    assert_eq!(
        holds.watch(&|_| Some((key(HEAD), approved.clone()))),
        [Watched::Changed {
            session: 4,
            key: key(HEAD),
        }]
    );
    assert!(!holds.every()[0].asked);
}

/// R-122, Q277: the approval a merge ask shows is the one `mergehold::read` reads from the
/// forge's reviews, with the app's login, of the head the forge gives, by whom. Kills: `read`
/// leaving the reviews it asked for unread (`approved_by: None` whatever they say), and reading
/// them against another head or author.
#[test]
fn the_approval_a_merge_ask_shows_is_read_from_the_forges_reviews_of_its_head() {
    purlis_core::unsteered!();
    let read = |reviews: serde_json::Value| {
        let pull = serde_json::json!({
            "number": 12, "state": "open",
            "head": {"ref": "feature", "sha": HEAD},
            "base": {"ref": "main", "repo": {"full_name": "acme/api"}},
            "title": "Add the parser", "user": {"login": "octo"}
        });
        let rest = |path: &str, out: &serde_json::Value| {
            serde_json::json!({"call": {"endpoint": {"rest": {"method": null, "path": path}},
                                        "fields": []},
                               "reply": {"code": 0, "out": out.to_string(), "err": ""}})
        };
        let text = serde_json::json!({
            "source": "GitHub REST API version 2022-11-28: pulls, pulls/reviews",
            "exchanges": [
                rest("repos/acme/api/pulls/12", &pull),
                rest("repos/acme/api/pulls/12/reviews?per_page=100&page=1", &reviews),
            ]
        });
        let recorded = Arc::new(
            purlis_core::forge::recorded::Recorded::parse(&text.to_string()).expect("recorded"),
        );
        let backend: Arc<dyn purlis_core::forge::backend::ForgeBackend> =
            Forge::default_of(Kind::GitHub)
                .backend_over(recorded)
                .into();
        mergehold::read(
            backend,
            "acme/api",
            12,
            std::time::Duration::from_secs(10),
            1_000,
        )
        .expect("the request is read")
        .facts
        .approved_by
    };
    let one = |login: &str, kind: &str, at: &str| {
        serde_json::json!({"user": {"login": login, "type": kind}, "state": "APPROVED",
                           "commit_id": at})
    };

    assert_eq!(
        read(serde_json::json!([one("ana", "User", HEAD)])).as_deref(),
        Some("ana")
    );
    assert_eq!(
        read(serde_json::json!([one("octo", "User", HEAD)])),
        None,
        "the author's own"
    );
    assert_eq!(
        read(serde_json::json!([one("ana", "User", OLDER)])),
        None,
        "of an older head"
    );
}
