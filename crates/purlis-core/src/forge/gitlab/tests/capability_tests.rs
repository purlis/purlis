// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use super::*;
use crate::forge::Forge;
use crate::forge::backend::{Fixed, Taken};
use crate::forge::recorded::Recorded;

fn on(host: &str) -> GitLab {
    let recorded = Recorded::parse(r#"{"source": "none", "exchanges": []}"#).unwrap();
    let mut forge = Forge::default_of(Kind::GitLab);
    forge.host = host.to_string();
    GitLab(Asker {
        forge,
        transports: Arc::new(Fixed(Arc::new(recorded))),
    })
}

#[test]
fn gitlab_com_has_epics_said_of_the_instance_and_unknown_of_a_group_or_repo() {
    let gitlab = on("gitlab.com");
    let me = Caller::window();
    for what in [
        Capability::Epics,
        Capability::Iterations,
        Capability::Boards,
        Capability::IssueTypes,
        Capability::CloseReasons,
        Capability::ItemStatus,
    ] {
        assert_eq!(
            gitlab.support(&me, &Reach::Instance, what),
            Support::Available
        );
        // A question that turns on the namespace's plan asks it, and nothing here
        // answers: unanswered (#802). Every other one asks nothing.
        let why = if premium_only(what) || what == Capability::CloseReasons {
            UnknownWhy::Unanswered
        } else {
            UnknownWhy::NotProbed
        };
        for narrower in [Reach::Owner("acme".into()), Reach::Repo("acme/api".into())] {
            let said = gitlab.support(&me, &narrower, what);
            assert_eq!(said, Support::Unknown(why.clone()), "{what:?} {narrower:?}");
            assert_eq!(said.taken(what), Taken::Fallback(what.fallback()));
        }
    }
}

/// A GitLab on `host` answering each `(path, reply)` once, as recorded.
fn answering(host: &str, exchanges: &[(&str, serde_json::Value)]) -> (GitLab, Arc<Recorded>) {
    let exchanges: Vec<serde_json::Value> = exchanges
        .iter()
        .map(|(path, reply)| {
            serde_json::json!({"call": {"endpoint": {"rest": {"method": null, "path": path}},
                                            "fields": []},
                                   "reply": reply})
        })
        .collect();
    let text = serde_json::json!({
            "source": "GitLab 19.4 REST API docs: metadata.md, license.md, namespaces.md",
            "exchanges": exchanges});
    let recorded = Arc::new(Recorded::parse(&text.to_string()).unwrap());
    let mut forge = Forge::default_of(Kind::GitLab);
    forge.host = host.to_string();
    let gitlab = GitLab(Asker {
        forge,
        transports: Arc::new(Fixed(recorded.clone())),
    });
    (gitlab, recorded)
}

fn ok(out: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"code": 0, "out": out.to_string()})
}

fn refused(status: u16) -> serde_json::Value {
    let said = format!("{status} Forbidden");
    serde_json::json!({"code": 1, "out": format!("{{\"message\":\"{said}\"}}"),
                           "err": format!("glab: {said} (HTTP {status})"), "status": status})
}

fn metadata(version: &str, enterprise: bool) -> serde_json::Value {
    ok(
        serde_json::json!({"version": version, "revision": "c401a659d0c",
                              "kas": {"enabled": false, "externalUrl": null, "version": null},
                              "enterprise": enterprise}),
    )
}

fn licence(plan: &str, expired: bool) -> serde_json::Value {
    ok(serde_json::json!({"id": 2, "plan": plan, "expired": expired}))
}

const SELF_MANAGED: &str = "git.example.com";

#[test]
fn a_self_managed_gitlab_on_18_1_with_a_premium_licence_has_an_items_own_status() {
    let (gitlab, recorded) = answering(
        SELF_MANAGED,
        &[
            ("metadata", metadata("18.1.2-ee", true)),
            ("license", licence("premium", false)),
        ],
    );
    let said = gitlab.support(&Caller::window(), &Reach::Instance, Capability::ItemStatus);
    assert_eq!(said, Support::Available);
    assert!(recorded.unspent().is_empty());
    for what in [
        Capability::Epics,
        Capability::Iterations,
        Capability::Dependencies,
    ] {
        let (gitlab, _) = answering(
            SELF_MANAGED,
            &[
                ("metadata", metadata("17.0.0-ee", true)),
                ("license", licence("ultimate", false)),
            ],
        );
        assert_eq!(
            gitlab.support(&Caller::window(), &Reach::Instance, what),
            Support::Available,
            "{what:?}"
        );
    }
}

#[test]
fn a_self_managed_gitlab_older_than_18_1_has_no_item_status_and_its_licence_is_not_asked() {
    let (gitlab, recorded) = answering(SELF_MANAGED, &[("metadata", metadata("18.0.3-ee", true))]);
    let said = gitlab.support(&Caller::window(), &Reach::Instance, Capability::ItemStatus);
    assert_eq!(
        said,
        Support::Unavailable(Unavailable::because(
            Capability::ItemStatus,
            Reason::HostTooOld("18.0.3-ee".into())
        ))
    );
    assert_eq!(
        said.taken(Capability::ItemStatus),
        Taken::Fallback(crate::forge::backend::Fallback::BoardStatus)
    );
    assert!(recorded.unspent().is_empty());
}

#[test]
fn a_community_edition_has_no_premium_feature_and_its_licence_is_not_asked() {
    for what in [
        Capability::Epics,
        Capability::Iterations,
        Capability::Dependencies,
        Capability::ItemStatus,
    ] {
        let (gitlab, recorded) =
            answering(SELF_MANAGED, &[("metadata", metadata("19.4.0", false))]);
        assert_eq!(
            gitlab.support(&Caller::window(), &Reach::Instance, what),
            Support::Unavailable(Unavailable::because(
                what,
                Reason::NeedsTier(Tier::GitLabPremium)
            )),
            "{what:?}"
        );
        assert!(recorded.unspent().is_empty());
    }
}

#[test]
fn a_licence_below_premium_or_expired_says_so_and_one_shown_to_no_one_stays_unknown() {
    let unavailable = Support::Unavailable(Unavailable::because(
        Capability::Epics,
        Reason::NeedsTier(Tier::GitLabPremium),
    ));
    for (answer, expected) in [
        (licence("starter", false), unavailable.clone()),
        (licence("premium", true), unavailable),
        // Asked, and refused (GitLab shows its licence to an administrator only), or
        // answered in a plan word purlis does not read: unanswered, not unasked (#802).
        (refused(403), Support::Unknown(UnknownWhy::Unanswered)),
        (
            licence("platinum", false),
            Support::Unknown(UnknownWhy::Unanswered),
        ),
    ] {
        let (gitlab, recorded) = answering(
            SELF_MANAGED,
            &[
                ("metadata", metadata("19.4.0-ee", true)),
                ("license", answer),
            ],
        );
        assert_eq!(
            gitlab.support(&Caller::window(), &Reach::Instance, Capability::Epics),
            expected
        );
        assert!(recorded.unspent().is_empty());
    }
}

#[test]
fn a_self_managed_gitlab_that_does_not_say_what_it_is_stays_unknown() {
    for answer in [
        refused(404),
        ok(serde_json::json!({"revision": "c401a659d0c"})),
    ] {
        let (gitlab, recorded) = answering(SELF_MANAGED, &[("metadata", answer)]);
        assert_eq!(
            gitlab.support(&Caller::window(), &Reach::Instance, Capability::ItemStatus),
            Support::Unknown(UnknownWhy::Unanswered)
        );
        assert!(recorded.unspent().is_empty());
    }
    // An edition it does not name is not taken for either.
    let (gitlab, _) = answering(
        SELF_MANAGED,
        &[("metadata", ok(serde_json::json!({"version": "19.4.0"})))],
    );
    assert_eq!(
        gitlab.support(&Caller::window(), &Reach::Instance, Capability::Epics),
        Support::Unknown(UnknownWhy::Unanswered)
    );
}

fn namespace(kind: &str, plan: Option<&str>) -> serde_json::Value {
    let mut out = serde_json::json!({"id": 9, "name": "Acme", "path": "acme", "kind": kind,
                                         "full_path": "acme"});
    if let Some(plan) = plan {
        out["plan"] = plan.into();
    }
    ok(out)
}

#[test]
fn a_premium_group_on_gitlab_com_has_epics_for_its_repos_and_a_free_one_does_not() {
    for at in [Reach::Repo("acme/api".into()), Reach::Owner("acme".into())] {
        let (gitlab, recorded) = answering(
            "gitlab.com",
            &[("namespaces/acme", namespace("group", Some("premium")))],
        );
        assert_eq!(
            gitlab.support(&Caller::window(), &at, Capability::Epics),
            Support::Available,
            "{at:?}"
        );
        assert!(recorded.unspent().is_empty());
        let (gitlab, _) = answering(
            "gitlab.com",
            &[("namespaces/acme", namespace("group", Some("free")))],
        );
        assert_eq!(
            gitlab.support(&Caller::window(), &at, Capability::ItemStatus),
            Support::Unavailable(Unavailable::because(
                Capability::ItemStatus,
                Reason::NeedsTier(Tier::GitLabPremium)
            ))
        );
    }
    // A subgroup's repo asks its group, the path encoded as one segment.
    let (gitlab, recorded) = answering(
        "gitlab.com",
        &[(
            "namespaces/acme%2Fplatform",
            namespace("group", Some("ultimate")),
        )],
    );
    let at = Reach::Repo("acme/platform/api".into());
    assert_eq!(
        gitlab.support(&Caller::window(), &at, Capability::Iterations),
        Support::Available
    );
    assert!(recorded.unspent().is_empty());
}

#[test]
fn a_plan_shown_to_no_one_or_a_personal_namespace_stays_unknown_on_gitlab_com() {
    let at = Reach::Repo("acme/api".into());
    // Asked and not answered: a plan shown to no one, a word purlis does not read, a
    // refusal. A personal namespace's Premium is an answer, but whether it gives a group's
    // epics is a question purlis has not asked.
    for (answer, why) in [
        (namespace("group", None), UnknownWhy::Unanswered),
        (namespace("user", Some("premium")), UnknownWhy::NotProbed),
        (namespace("group", Some("default")), UnknownWhy::Unanswered),
        (refused(404), UnknownWhy::Unanswered),
    ] {
        let (gitlab, recorded) = answering("gitlab.com", &[("namespaces/acme", answer)]);
        assert_eq!(
            gitlab.support(&Caller::window(), &at, Capability::Epics),
            Support::Unknown(why)
        );
        assert!(recorded.unspent().is_empty());
    }
    // A free namespace still reads a duplicate marking, so its close reasons are not denied.
    let (gitlab, _) = answering(
        "gitlab.com",
        &[("namespaces/acme", namespace("group", Some("free")))],
    );
    assert_eq!(
        gitlab.support(&Caller::window(), &at, Capability::CloseReasons),
        Support::Unknown(UnknownWhy::NotProbed)
    );
}

#[test]
fn a_question_no_probe_answers_asks_nothing() {
    let (gitlab, recorded) = answering(
        "gitlab.com",
        &[("namespaces/acme", namespace("group", Some("premium")))],
    );
    let me = Caller::window();
    for what in [
        Capability::Boards,
        Capability::SubIssues,
        Capability::AutoMerge,
    ] {
        let _ = gitlab.support(&me, &Reach::Repo("acme/api".into()), what);
    }
    for what in Capability::ALL {
        let _ = gitlab.support(&me, &Reach::Instance, what);
    }
    assert_eq!(recorded.unspent().len(), 1);
    // A self-managed GitLab is asked about itself only for the instance.
    let (gitlab, recorded) = answering(SELF_MANAGED, &[("metadata", metadata("19.4.0-ee", true))]);
    let _ = gitlab.support(&me, &Reach::Repo("acme/api".into()), Capability::Epics);
    let _ = gitlab.support(&me, &Reach::Instance, Capability::Boards);
    assert_eq!(recorded.unspent().len(), 1);
}

/// As [`answering`], over a backend that keeps its probe answers for `kept_for`.
fn keeping(
    host: &str,
    exchanges: &[(&str, serde_json::Value)],
    kept_for: std::time::Duration,
) -> (GitLab, Arc<Recorded>) {
    let (GitLab(asker), recorded) = answering(host, exchanges);
    let transports = Arc::new(crate::forge::backend::Kept::over(
        asker.transports,
        kept_for,
    ));
    let gitlab = GitLab(Asker {
        forge: asker.forge,
        transports,
    });
    (gitlab, recorded)
}

fn an_account(login: &str) -> crate::forge::backend::Account {
    crate::forge::backend::Account {
        kind: Kind::GitLab,
        host: SELF_MANAGED.into(),
        login: login.into(),
    }
}

#[test]
fn a_probes_answer_is_kept_for_the_caller_that_asked_it() {
    // One answer each: a second question that asked again would find nothing recorded.
    let (gitlab, recorded) = keeping(
        SELF_MANAGED,
        &[
            ("metadata", metadata("19.4.0-ee", true)),
            ("license", licence("premium", false)),
        ],
        crate::forge::backend::PROBE_KEPT_FOR,
    );
    let me = Caller::window();
    for what in [
        Capability::Epics,
        Capability::ItemStatus,
        Capability::Iterations,
    ] {
        assert_eq!(
            gitlab.support(&me, &Reach::Instance, what),
            Support::Available,
            "{what:?}"
        );
    }
    assert!(recorded.unspent().is_empty());
}

#[test]
fn a_kept_answer_is_never_another_accounts_or_a_chats() {
    let answers = [
        ("metadata", metadata("19.4.0-ee", true)),
        ("license", licence("premium", false)),
    ];
    let callers = [
        Caller::window().as_account(an_account("ada")),
        Caller::window().as_account(an_account("grace")),
        Caller::window(),
        Caller::chat("c1"),
        Caller::chat("c2"),
        Caller::mcp("c1"),
    ];
    let every: Vec<_> = callers.iter().flat_map(|_| answers.clone()).collect();
    let (gitlab, recorded) = keeping(SELF_MANAGED, &every, crate::forge::backend::PROBE_KEPT_FOR);
    for caller in &callers {
        assert_eq!(
            gitlab.support(caller, &Reach::Instance, Capability::Epics),
            Support::Available,
            "{caller:?}"
        );
    }
    assert!(
        recorded.unspent().is_empty(),
        "each caller asked for itself"
    );
}

#[test]
fn an_expired_answer_or_a_refusal_is_asked_again() {
    let (gitlab, recorded) = keeping(
        SELF_MANAGED,
        &[
            ("metadata", metadata("19.4.0", false)),
            ("metadata", metadata("19.4.0", false)),
        ],
        std::time::Duration::ZERO,
    );
    for _ in 0..2 {
        let _ = gitlab.support(&Caller::window(), &Reach::Instance, Capability::Epics);
    }
    assert!(
        recorded.unspent().is_empty(),
        "an expired answer is asked again"
    );

    let (gitlab, recorded) = keeping(
        SELF_MANAGED,
        &[
            ("metadata", metadata("19.4.0-ee", true)),
            ("license", refused(403)),
            ("license", refused(403)),
        ],
        crate::forge::backend::PROBE_KEPT_FOR,
    );
    for _ in 0..2 {
        assert_eq!(
            gitlab.support(&Caller::window(), &Reach::Instance, Capability::Epics),
            Support::Unknown(UnknownWhy::Unanswered)
        );
    }
    assert!(
        recorded.unspent().is_empty(),
        "the version is kept, the refusal asked again"
    );
}

#[test]
fn a_self_managed_gitlab_that_gives_no_answer_is_unanswered_and_the_rest_unasked() {
    // Nothing is recorded, so the instance probe gets no answer at all: a Premium question
    // was asked and is unanswered; every other one asks nothing and stays unasked.
    let gitlab = on("git.example.com");
    for what in Capability::ALL {
        let why = if premium_only(what) {
            UnknownWhy::Unanswered
        } else {
            UnknownWhy::NotProbed
        };
        assert_eq!(
            gitlab.support(&Caller::window(), &Reach::Instance, what),
            Support::Unknown(why),
            "{what:?}"
        );
    }
}
