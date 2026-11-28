// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use purlis_core::forge::proxy::Kind;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Answers {
    by_id: HashMap<&'static str, Result<Option<&'static str>, &'static str>>,
    asked: AtomicUsize,
    terms: Mutex<Vec<CredentialRequest>>,
}

impl Answers {
    fn of(by_id: &[(&'static str, Result<Option<&'static str>, &'static str>)]) -> Arc<Self> {
        Arc::new(Self {
            by_id: by_id.iter().cloned().collect(),
            asked: AtomicUsize::new(0),
            terms: Mutex::new(Vec::new()),
        })
    }
}

impl NodeRepos for Answers {
    fn repo_of_node(
        &self,
        request: &CredentialRequest,
        id: &str,
    ) -> Result<Option<String>, String> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        self.terms.lock().unwrap().push(request.clone());
        match self.by_id.get(id) {
            Some(Ok(repo)) => Ok(repo.map(str::to_owned)),
            Some(Err(why)) => Err((*why).to_owned()),
            None => Err("unknown to the test".to_owned()),
        }
    }
}

fn run(lookup: Arc<dyn NodeRepos>) -> RunObjects {
    let repo = AllowedRepo::new(Kind::GitHub, "github.com", 443, "Acme/Widgets", "main")
        .unwrap()
        .with_forge_id(purlis_core::forge::proxy::ForgeRepoId::for_a_test(7));
    RunObjects::new(ChatId::new("run-1"), vec![repo], lookup)
}

#[test]
fn a_ported_repo_is_asked_in_the_terms_of_its_own_port() {
    let lookup = Answers::of(&[("PR_kwDOA", Ok(Some("acme/widgets")))]);
    let repo = AllowedRepo::new(
        Kind::GitHub,
        "ghes.example.com",
        8443,
        "acme/widgets",
        "main",
    )
    .unwrap();
    let objects = RunObjects::new(ChatId::new("run-1"), vec![repo], lookup.clone());

    assert!(
        objects
            .repo_of(Kind::GitHub, "ghes.example.com", "PR_kwDOA")
            .is_some()
    );
    assert_eq!(lookup.terms.lock().unwrap()[0].port, 8443);
}

#[test]
fn a_ported_host_is_asked_on_its_port() {
    assert_eq!(api_host("ghes.example.com", 8443), "ghes.example.com:8443");
    assert_eq!(api_host("github.com", 443), "github.com");
}

#[test]
fn an_id_in_a_repo_names_that_repo_in_the_terms_of_the_runs_own_recorded_repo() {
    let lookup = Answers::of(&[("PR_kwDOA", Ok(Some("acme/widgets")))]);
    let objects = run(lookup.clone());

    assert_eq!(
        objects
            .repo_of(Kind::GitHub, "github.com", "PR_kwDOA")
            .as_deref(),
        Some("acme/widgets")
    );
    let terms = lookup.terms.lock().unwrap();
    assert_eq!(terms[0].chat, ChatId::new("run-1"));
    assert_eq!(terms[0].repo, "acme/widgets");
    assert_eq!(terms[0].repo_id, Some(7));
}

#[test]
fn the_answer_is_what_the_forge_said_so_another_repos_id_names_another_repo() {
    let lookup = Answers::of(&[("PR_other", Ok(Some("someone/else")))]);

    assert_eq!(
        run(lookup)
            .repo_of(Kind::GitHub, "github.com", "PR_other")
            .as_deref(),
        Some("someone/else")
    );
}

#[test]
fn an_id_the_forge_does_not_place_names_no_repo() {
    let lookup = Answers::of(&[("U_user", Ok(None)), ("PR_down", Err("timed out"))]);
    let objects = run(lookup);

    assert_eq!(objects.repo_of(Kind::GitHub, "github.com", "U_user"), None);
    assert_eq!(objects.repo_of(Kind::GitHub, "github.com", "PR_down"), None);
    assert_eq!(
        objects.repo_of(Kind::GitHub, "github.com", "PR_never"),
        None
    );
}

#[test]
fn an_id_no_forge_mints_a_gitlab_repo_and_a_host_with_no_recorded_repo_are_not_asked() {
    let lookup = Answers::of(&[("PR_a", Ok(Some("acme/widgets")))]);
    let objects = run(lookup.clone());

    for id in ["", "PR a", "PR_a\"}", &"x".repeat(MOST_ID_LEN + 1)] {
        assert_eq!(
            objects.repo_of(Kind::GitHub, "github.com", id),
            None,
            "{id}"
        );
    }
    assert_eq!(objects.repo_of(Kind::GitLab, "github.com", "PR_a"), None);
    assert_eq!(objects.repo_of(Kind::GitHub, "ghe.example", "PR_a"), None);
    assert_eq!(lookup.asked.load(Ordering::SeqCst), 0);
}

#[test]
fn a_repeated_id_is_asked_once_and_a_forge_that_failed_is_asked_again() {
    let lookup = Answers::of(&[("PR_a", Ok(Some("acme/widgets"))), ("PR_down", Err("no"))]);
    let objects = run(lookup.clone());

    for _ in 0..3 {
        objects.repo_of(Kind::GitHub, "GitHub.com", "PR_a");
    }
    assert_eq!(lookup.asked.load(Ordering::SeqCst), 1);

    objects.repo_of(Kind::GitHub, "github.com", "PR_down");
    objects.repo_of(Kind::GitHub, "github.com", "PR_down");
    assert_eq!(lookup.asked.load(Ordering::SeqCst), 3);
    assert_eq!(objects.cached(), 1);
}

#[test]
fn the_cache_is_bounded_and_forgets_the_oldest_first() {
    struct Everywhere(AtomicUsize);
    impl NodeRepos for Everywhere {
        fn repo_of_node(&self, _: &CredentialRequest, _: &str) -> Result<Option<String>, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(Some("acme/widgets".to_owned()))
        }
    }
    let lookup = Arc::new(Everywhere(AtomicUsize::new(0)));
    let objects = run(lookup.clone());

    for n in 0..MOST_CACHED + 50 {
        objects.repo_of(Kind::GitHub, "github.com", &format!("PR_{n}"));
    }
    assert_eq!(objects.cached(), MOST_CACHED);

    let before = lookup.0.load(Ordering::SeqCst);
    objects.repo_of(
        Kind::GitHub,
        "github.com",
        &format!("PR_{}", MOST_CACHED + 49),
    );
    assert_eq!(
        lookup.0.load(Ordering::SeqCst),
        before,
        "the newest is kept"
    );
    objects.repo_of(Kind::GitHub, "github.com", "PR_0");
    assert_eq!(
        lookup.0.load(Ordering::SeqCst),
        before + 1,
        "the oldest is gone"
    );
}

#[test]
fn the_forges_answer_is_read_as_the_repo_it_names_and_nothing_else_names_one() {
    let pr = r#"{"data":{"node":{"__typename":"PullRequest","repository":{"nameWithOwner":"Acme/Widgets"}}}}"#;
    assert_eq!(repo_in(pr), Ok(Some("acme/widgets".to_owned())));
    let issue = r#"{"data":{"node":{"__typename":"Issue","repository":{"nameWithOwner":"a/b"}}}}"#;
    assert_eq!(repo_in(issue), Ok(Some("a/b".to_owned())));
    // A node of another type has `__typename` and no repository.
    let user = r#"{"data":{"node":{"__typename":"User"}}}"#;
    assert_eq!(repo_in(user), Ok(None));
    assert_eq!(repo_in(r#"{"data":{"node":null}}"#), Ok(None));
    assert!(repo_in("not json").is_err());
    let odd = r#"{"data":{"node":{"repository":{"nameWithOwner":"a/b/c"}}}}"#;
    assert!(repo_in(odd).is_err());
    let odd = r#"{"data":{"node":{"repository":{"nameWithOwner":"../b"}}}}"#;
    assert!(repo_in(odd).is_err());
}

// ---- through the proxy: what `gh pr merge` sends, with this resolver where the forge is ----

use purlis_core::enforcement::Mode;
use purlis_core::forge::mergestance::PolicyMergeStance;
use purlis_core::forge::proxy::{ForgeProxy, Inbound, Outcome, RequestHead, TokenKind};
use purlis_core::secrets::keyring::Secret;

struct Person;
impl CredentialSource for Person {
    fn credential_for(&self, _: &CredentialRequest) -> Result<Credential, CredentialError> {
        Ok(Credential {
            token_kind: TokenKind::Oauth,
            secret: Secret::new("real-token".to_owned()),
            account: None,
        })
    }
}

/// The outcome of `mutation` sent by a chat whose one recorded repo is `acme/widgets`, under
/// `mode`, where the forge places `PR_mine` in that repo, `PR_theirs` in another and nothing
/// else.
fn sent(mode: Mode, mutation: &str) -> (Outcome, usize) {
    let lookup = Answers::of(&[
        ("PR_mine", Ok(Some("acme/widgets"))),
        ("PR_theirs", Ok(Some("someone/else"))),
        ("U_user", Ok(None)),
        ("PR_down", Err("timed out")),
    ]);
    let objects = run(lookup.clone());
    let proxy = ForgeProxy::new(Box::new(Person), Box::new(objects)).with_merge_stance(Arc::new(
        PolicyMergeStance::reading(move || mode, |_, _| {}),
    ));
    let chat = ChatId::new("run-1");
    let repo = AllowedRepo::new(Kind::GitHub, "github.com", 443, "acme/widgets", "main").unwrap();
    let sentinel = proxy.enrol(&chat, vec![repo]);
    let body =
        serde_json::to_vec(&serde_json::json!({ "query": mutation, "variables": {} })).unwrap();
    let head = RequestHead::new("POST", "api.github.com", 443, "/graphql");
    let headers = vec![
        (
            "Authorization".to_owned(),
            format!("Bearer {}", sentinel.expose()),
        ),
        ("Content-Type".to_owned(), "application/json".to_owned()),
    ];
    let out = proxy.handle(
        &chat,
        &Inbound {
            head: &head,
            headers: &headers,
            body: &body,
        },
    );
    (out, lookup.asked.load(Ordering::SeqCst))
}

fn by_id(name: &str, id: &str) -> String {
    format!("mutation{{ {name}(input:{{pullRequestId:\"{id}\"}}){{ clientMutationId }} }}")
}

const BY_ID: [&str; 4] = [
    "mergePullRequest",
    "enablePullRequestAutoMerge",
    "enqueuePullRequest",
    "closePullRequest",
];

#[test]
fn a_merge_by_node_id_on_the_recorded_repo_is_forwarded_under_log_and_off() {
    for mode in [Mode::Log, Mode::Off] {
        for name in &BY_ID[..3] {
            let (out, asked) = sent(mode, &by_id(name, "PR_mine"));
            assert!(
                matches!(out, Outcome::Forward(_)),
                "{name} {mode:?}: {out:?}"
            );
            assert_eq!(asked, 1, "{name}: the forge was asked for the id");
        }
    }
}

#[test]
fn a_merge_by_node_id_is_held_under_enforce_and_never_forwarded() {
    for name in &BY_ID[..3] {
        let (out, _) = sent(Mode::Enforce, &by_id(name, "PR_mine"));
        assert!(!matches!(out, Outcome::Forward(_)), "{name}: {out:?}");
    }
}

#[test]
fn closing_a_pull_request_by_node_id_on_the_recorded_repo_is_no_longer_another_repos() {
    // A close is no merge: the id is tied to the recorded repo and it goes through.
    let (out, _) = sent(Mode::Enforce, &by_id("closePullRequest", "PR_mine"));
    assert!(matches!(out, Outcome::Forward(_)), "{out:?}");
}

#[test]
fn a_node_id_of_another_repo_an_unplaced_id_and_a_forge_that_failed_are_refused_in_every_mode() {
    for mode in [Mode::Log, Mode::Off, Mode::Enforce] {
        for name in BY_ID {
            for id in ["PR_theirs", "U_user", "PR_down", "PR_unknown"] {
                let (out, _) = sent(mode, &by_id(name, id));
                assert!(
                    !matches!(out, Outcome::Forward(_)),
                    "{name} {id} {mode:?}: {out:?}"
                );
            }
        }
    }
}

#[test]
fn a_pull_requests_head_is_read_as_the_repo_and_branch_the_forge_names() {
    let answer = serde_json::json!({
        "headRefName": "fix", "headRefOid": "ab12",
        "headRepository": {"nameWithOwner": "Acme/Widgets"}});
    assert_eq!(
        head_in(&answer),
        Ok(Some(PullHead {
            repo: "acme/widgets".to_owned(),
            branch: "fix".to_owned(),
            oid: "ab12".to_owned()
        }))
    );
    let mr = serde_json::json!({
        "sourceBranch": "work", "diffHeadSha": "cd34",
        "sourceProject": {"fullPath": "Group/Proj"}});
    assert_eq!(
        merge_request_head_in(&mr),
        Ok(Some(PullHead {
            repo: "group/proj".to_owned(),
            branch: "work".to_owned(),
            oid: "cd34".to_owned()
        }))
    );
    // A head with no tip is no head: nothing to compare the record with.
    let no_tip = serde_json::json!({
        "headRefName": "fix", "headRepository": {"nameWithOwner": "acme/widgets"}});
    assert_eq!(head_in(&no_tip), Ok(None));
}

#[test]
fn a_pull_request_head_the_forge_does_not_name_is_none_and_one_of_another_shape_is_an_error() {
    // A deleted fork has a null head repository; a node that is no pull request has neither field.
    for gone in [
        serde_json::json!({"headRefName": "fix", "headRefOid": "ab12", "headRepository": null}),
        serde_json::json!({}),
        serde_json::Value::Null,
    ] {
        assert_eq!(head_in(&gone), Ok(None), "{gone}");
    }
    let odd = serde_json::json!({
        "headRefName": "fix", "headRefOid": "ab12", "headRepository": {"nameWithOwner": "a/b/c"}});
    assert!(head_in(&odd).is_err());
}

#[test]
fn a_run_asks_for_a_pull_requests_head_only_on_github_and_only_where_it_has_a_repo() {
    struct Heads(AtomicUsize);
    impl NodeRepos for Heads {
        fn repo_of_node(&self, _: &CredentialRequest, _: &str) -> Result<Option<String>, String> {
            Ok(None)
        }
        fn pull_head(
            &self,
            _: &CredentialRequest,
            _: PullRef<'_>,
        ) -> Result<Option<PullHead>, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(Some(PullHead {
                repo: "acme/widgets".to_owned(),
                branch: "fix".to_owned(),
                oid: "1".repeat(40),
            }))
        }
    }
    let lookup = Arc::new(Heads(AtomicUsize::new(0)));
    let objects = run(lookup.clone());
    let number = PullRef::Number {
        repo: "acme/widgets",
        number: 3,
    };

    assert_eq!(
        objects
            .pull_head(Kind::GitHub, "github.com", number)
            .map(|h| h.branch),
        Some("fix".to_owned())
    );
    assert!(
        objects
            .pull_head(Kind::GitHub, "elsewhere.example", number)
            .is_none()
    );
    assert!(
        objects
            .pull_head(Kind::GitLab, "github.com", number)
            .is_none(),
        "no GitLab repo of this run on that host"
    );
    assert!(
        objects
            .pull_head(Kind::GitHub, "github.com", PullRef::Node("not a node id!"))
            .is_none()
    );
    assert_eq!(lookup.0.load(Ordering::SeqCst), 1);
}

#[test]
fn a_lookup_that_cannot_read_a_head_makes_the_head_unknown() {
    // The default `pull_head` is an error, which the run reads as no head: update-branch refused.
    let objects = run(Answers::of(&[]));
    assert!(
        objects
            .pull_head(
                Kind::GitHub,
                "github.com",
                PullRef::Number {
                    repo: "acme/widgets",
                    number: 3
                }
            )
            .is_none()
    );
}

/// The production [`RunObjects`] under a [`ChatPolicy`], end to end (#7526): update-branch is
/// decided by what the forge says the head is and by the record the push rule reads, with no
/// test double for either.
mod through_the_policy {
    use super::*;
    use purlis_core::forge::proxy::{ChatPolicy, Decision, Refusal, RequestHead};

    const TIP: &str = "1111111111111111111111111111111111111111";

    /// The forge's answer to a head: by number, one pull request per entry.
    struct Heads(Vec<(u64, &'static str, &'static str)>);

    impl NodeRepos for Heads {
        fn repo_of_node(&self, _: &CredentialRequest, _: &str) -> Result<Option<String>, String> {
            Ok(None)
        }
        fn pull_head(
            &self,
            _: &CredentialRequest,
            pull: PullRef<'_>,
        ) -> Result<Option<PullHead>, String> {
            let PullRef::Number { number, .. } = pull else {
                return Ok(None);
            };
            Ok(self
                .0
                .iter()
                .find(|(n, _, _)| *n == number)
                .map(|(_, branch, oid)| PullHead {
                    repo: "acme/widgets".to_owned(),
                    branch: (*branch).to_owned(),
                    oid: (*oid).to_owned(),
                }))
        }
    }

    /// The chat made `fix` (tip [`TIP`]); the person's `develop` and the default `main` are not
    /// the chat's.
    fn decide(number: u64, heads: Vec<(u64, &'static str, &'static str)>) -> Decision {
        decide_held(number, heads, false)
    }

    /// [`decide`], where the chat holds `fix` by the person's Allow when `by_allow`.
    fn decide_held(
        number: u64,
        heads: Vec<(u64, &'static str, &'static str)>,
        by_allow: bool,
    ) -> Decision {
        let repo = AllowedRepo::new(Kind::GitHub, "github.com", 443, "Acme/Widgets", "main")
            .unwrap()
            .with_forge_id(purlis_core::forge::proxy::ForgeRepoId::for_a_test(7));
        let objects = RunObjects::new(
            ChatId::new("run-1"),
            vec![repo.clone()],
            Arc::new(Heads(heads)),
        );
        let tips: purlis_core::forge::proxy::TipSource =
            Arc::new(move |_host, _repo_id, branch| {
                (branch == "fix").then(|| purlis_core::forge::createdbranches::OwnedTip {
                    tip: TIP.to_owned(),
                    fast_forward_only: by_allow,
                })
            });
        let policy = ChatPolicy::new(vec![repo]).with_tips(tips);
        let head = RequestHead::new(
            "PUT",
            "api.github.com",
            443,
            &format!("/repos/acme/widgets/pulls/{number}/update-branch"),
        );
        policy.decide(&head, b"", &objects)
    }

    #[test]
    fn update_branch_is_decided_by_the_forges_head_and_the_chats_own_record() {
        let heads = || {
            vec![
                (1, "fix", TIP),
                (2, "develop", TIP),
                (3, "main", TIP),
                (4, "fix", "2222222222222222222222222222222222222222"),
            ]
        };
        assert!(
            matches!(decide(1, heads()), Decision::Allow(_)),
            "the chat's own branch"
        );
        for (number, why) in [
            (2, "a branch the chat did not make"),
            (3, "the default branch"),
            (4, "a branch that moved since the chat pushed it"),
            (5, "a pull request the forge does not know"),
        ] {
            assert!(
                matches!(
                    decide(number, heads()),
                    Decision::Refuse(Refusal::ProtectedPullHead)
                ),
                "{why}"
            );
        }
    }

    #[test]
    fn update_branch_does_not_write_a_head_the_chat_holds_by_the_persons_allow() {
        // A rebase through the forge would replace the branch's history, and the route cannot
        // read the commits the forge would write (#7841): the chat updates it locally and pushes.
        assert!(
            matches!(
                decide_held(1, vec![(1, "fix", TIP)], true),
                Decision::Refuse(Refusal::ProtectedPullHead)
            ),
            "held by Allow"
        );
        assert!(matches!(
            decide_held(1, vec![(1, "fix", TIP)], false),
            Decision::Allow(_)
        ));
    }
}

#[test]
fn an_issue_named_by_its_numeric_id_is_asked_about_in_the_form_the_route_sends() {
    // `05:Issue<id>` in base64 (#8022): the forge route places a REST body's issue id this way,
    // padded or not.
    let lookup = Answers::of(&[
        ("MDU6SXNzdWU2NjYxODE5MDcy", Ok(Some("acme/widgets"))),
        ("MDU6SXNzdWUxMg==", Ok(Some("someone/else"))),
    ]);
    let objects = run(lookup.clone());

    assert_eq!(
        objects
            .repo_of(Kind::GitHub, "github.com", "MDU6SXNzdWU2NjYxODE5MDcy")
            .as_deref(),
        Some("acme/widgets")
    );
    assert_eq!(
        objects
            .repo_of(Kind::GitHub, "github.com", "MDU6SXNzdWUxMg==")
            .as_deref(),
        Some("someone/else")
    );
    assert_eq!(lookup.asked.load(Ordering::SeqCst), 2);
}
