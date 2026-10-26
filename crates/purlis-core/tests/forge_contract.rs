// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The forge contract suite: every method of the forge seam's areas, run once per forge
//! against recorded exchanges (ADR 0070 §7, the recorded run; FG-3), and nightly against a real
//! forge (the live run; FG-4).
//!
//! Each case is written **once**, as a function of the forge, and [`contract!`] instantiates it
//! for GitHub and for GitLab. The two forges' recordings, under `tests/forge_contract/<forge>/`,
//! describe the same scene in each forge's own words, so a case asserts one neutral answer for
//! both. A request a backend sends that nobody recorded fails the case, and so does a recorded
//! exchange nobody asked for.
//!
//! **Where the answers come from.** Each recording names its source. Today they are taken from
//! each forge's own API documentation (GitHub REST `2022-11-28` and its GraphQL schema; the
//! GitLab 19.4 REST docs), not captured from a live forge: no account is signed in where this
//! suite was written. A PR that moves a recording says which contract moved and why (ADR 0046).
//!
//! **The live run** (`live.rs`) is the same cases, through the same seam, over the native
//! transport to a real github.com repo and a real gitlab.com repo the operator provisioned. A
//! case names what it asks about through a [`Scene`] rather than a literal, so the recorded
//! scene and the live fixture both answer it. It is ignored in a plain `cargo test`;
//! `.github/workflows/forge-live.yml` runs it nightly. A case it cannot run is listed in
//! [`NOT_LIVE`] with its reason and ticket, and says so when it is asked, never silently.
//!
//! **Parity is checked, not trusted.** `every_method_of_the_seam_has_a_case_on_both_forges`
//! reads the area traits out of `src/forge/backend.rs` and fails when a method has no case here
//! or a case has no recording for one forge.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use purlis_core::forge::backend::{About, Issues, NewWorkItem, PushProtection, Visibility};
use purlis_core::forge::checks::{Annotation, Checks, Ci, FailedCheck};
use purlis_core::forge::pr::{
    AutoMerge, Gate, MergeAs, MergedAt, Opened, Pinned, Pr, Ready, Request, State,
};
use purlis_core::forge::recorded::Recorded;
use purlis_core::forge::{
    Caller, Capability, Forge, ForgeBackend, ForgeRef, Kind, MadeRepo, NewRepo, Owner, OwnerKind,
    Reach, RepoOwner, Support,
};
use purlis_core::work;
use serde_json::{Value, json};

#[path = "forge_contract/live.rs"]
mod live;
#[path = "forge_contract/native.rs"]
mod native;
#[path = "forge_contract/scene.rs"]
mod scene;

use scene::{MERGE, SAVE, SHA, Scene};

/// Every case, by the seam method it covers. The parity test compares this with the traits.
const CASES: [&str; 27] = [
    "owned",
    "about",
    "reachable",
    "top_level",
    "open_or_update",
    "state",
    "body",
    "set_body",
    "by_head",
    "request_auto_merge",
    "merge_when_ready",
    "lands_through_queue",
    "merge_at",
    "enqueue_at",
    "checks_at",
    "annotations",
    "open_on_branch",
    "ci_word",
    "support",
    "create",
    "read",
    "search",
    "owners_to_create_under",
    "create_repo",
    "delete_created_repo",
    "keep_images",
    "comment",
];

/// The cases the live run does not run, each with why and the ticket that will run it. Each one
/// would merge or queue the fixture's one open request, which the read cases need open; running
/// them needs a request opened afresh for each run.
const NOT_LIVE: [(&str, &str); 12] = [
    (
        "request_auto_merge",
        "auto-merge would land the fixture's open request; it needs a request opened per run (#712)",
    ),
    (
        "merge_when_ready",
        "it merges the fixture's open request; it needs a request opened per run (#712)",
    ),
    (
        "merge_at",
        "it merges the fixture's open request; it needs a request opened per run (#712)",
    ),
    (
        "enqueue_at",
        "it needs a merge queue or train on the fixture, and a request opened per run (#712)",
    ),
    (
        "annotations",
        "it needs a failed check on the fixture's open request, which the read cases need green (#712)",
    ),
    (
        "read",
        "it needs a fixture issue with a type, a parent, a child, a blocker, a closing request \
         and a board with a status and an iteration (#742)",
    ),
    (
        "search",
        "a forge's search index answers a fixture's issues only once it has indexed them, and in \
         its own order; it needs fixture issues indexed under fixed words (#742)",
    ),
    (
        "owners_to_create_under",
        "the fixture account's organisations are the operator's to provision (#712)",
    ),
    (
        "create_repo",
        "it makes a repo on the fixture owner, which only a run that can delete it may (#712)",
    ),
    (
        "delete_created_repo",
        "it deletes the repo a run made, which needs that repo and a token that may delete (#712)",
    ),
    (
        "keep_images",
        "it writes a proof branch or uploads to the fixture each run; it needs a request opened \
         per run and what it kept removed after (#3395)",
    ),
    (
        "comment",
        "it comments on the fixture's open request each run; it needs a request opened per run \
         (#3395)",
    ),
];

fn recordings() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/forge_contract")
}

/// How a case's requests are answered.
#[derive(Clone, Copy)]
pub enum How {
    /// Through the recorded transport, as a CLI call's reply.
    Recorded,
    /// Through the native transport, over HTTP, against a server that answers the same
    /// recording (`native.rs`), on the forge's own instance: github.com or gitlab.com.
    Native,
    /// As `Native`, on a self-managed instance: a GitHub Enterprise Server or a self-managed
    /// GitLab, at `forge.example.com`.
    SelfManaged,
    /// Through the native transport to a real forge, about the fixture provisioned there
    /// (`live.rs`).
    Live,
}

/// A backend over a case's recording or a live forge, the caller to ask as, the scene to ask
/// about, and the check that every recorded exchange was asked for.
pub struct Over {
    pub backend: Box<dyn ForgeBackend>,
    pub caller: Caller,
    /// The instance the backend is on.
    pub host: String,
    pub scene: Scene,
    check: Box<dyn Fn()>,
    /// Closes every open issue the `create` case opened, by the forge's own listing, and answers
    /// their numbers: on a live forge, so the fixture keeps no backlog. Nothing on a recording.
    sweep: Box<dyn Fn() -> Result<Vec<u64>, String>>,
}

/// Every recorded exchange was asked for.
pub trait Spent {
    fn spent(&self);
}

impl Spent for Recorded {
    fn spent(&self) {
        assert_eq!(self.unspent(), Vec::new(), "recorded and never asked");
    }
}

impl<T: Spent> Spent for Arc<T> {
    fn spent(&self) {
        T::spent(self);
    }
}

impl Spent for Over {
    fn spent(&self) {
        (self.check)();
    }
}

fn spent(recorded: &impl Spent) {
    recorded.spent();
}

/// The text of `kind`'s recording of `case`.
pub fn recording(kind: &str, case: &str) -> String {
    let file = recordings().join(kind).join(format!("{case}.json"));
    std::fs::read_to_string(&file)
        .unwrap_or_else(|e| panic!("the recording {}: {e}", file.display()))
}

/// The self-managed override at `file`, or `None` when there is none. Any other read error
/// fails, so an override that is there is never quietly replaced by the main recording.
fn own_recording(file: &Path) -> Option<String> {
    match std::fs::read_to_string(file) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => panic!("the override {}: {e}", file.display()),
    }
}

/// A forge's backend for `case`, answered as `how` says, or `None`, said aloud, for a case the
/// live run leaves out.
fn over(kind: &str, case: &str, how: How) -> Option<Over> {
    let forge_kind = Kind::parse(kind).unwrap();
    if let How::Live = how {
        if let Some((_, why)) = NOT_LIVE.iter().find(|(c, _)| *c == case) {
            println!("not run live on {kind}: {case}: {why}");
            return None;
        }
        return Some(live::over(forge_kind));
    }
    let text = recording(kind, case);
    Some(match how {
        How::Recorded => {
            let recorded = Arc::new(Recorded::parse(&text).unwrap());
            let forge = Forge::default_of(forge_kind);
            let check = recorded.clone();
            Over {
                backend: forge.backend_over(recorded),
                caller: caller(),
                host: forge.host.clone(),
                scene: Scene::recorded(forge_kind),
                check: Box::new(move || check.spent()),
                sweep: Box::new(|| Ok(Vec::new())),
            }
        }
        How::Native => native::over(kind, default_host(kind), &text),
        How::SelfManaged => {
            // A case a self-managed instance is asked differently, because a capability charter
            // has not probed there takes its fallback, has its own recording beside the case's.
            let own = recordings()
                .join(kind)
                .join(format!("{case}.self_managed.json"));
            let text = own_recording(&own).unwrap_or(text);
            native::over(kind, SELF_MANAGED, &text)
        }
        How::Live => unreachable!("answered above"),
    })
}

/// The host a self-managed instance is on, in the native cases that run on one.
const SELF_MANAGED: &str = "forge.example.com";

/// The forge's own instance.
fn default_host(kind: &str) -> &'static str {
    match kind {
        "github" => "github.com",
        _ => "gitlab.com",
    }
}

fn caller() -> Caller {
    Caller::command()
}

mod cases {
    use super::*;

    /// A listing in path order: neither forge's default order is part of the contract.
    fn by_path(
        mut repos: Vec<purlis_core::forge::RepoRecord>,
    ) -> Vec<purlis_core::forge::RepoRecord> {
        repos.sort_by(|a, b| a.path_with_namespace.cmp(&b.path_with_namespace));
        repos
    }

    pub fn owned(kind: &str, how: How) {
        let Some(over) = over(kind, "owned", how) else {
            return;
        };
        let scene = &over.scene;
        let repos = over
            .backend
            .owned(&over.caller, &Owner::new(&scene.owner))
            .unwrap();
        assert_eq!(
            by_path(repos),
            [scene.api(), scene.web()],
            "every field typed; a null description is empty"
        );
        spent(&over);
    }

    pub fn reachable(kind: &str, how: How) {
        let Some(over) = over(kind, "reachable", how) else {
            return;
        };
        let scene = &over.scene;
        let repos = over
            .backend
            .reachable(&over.caller, &Owner::new(&scene.owner))
            .unwrap();
        assert_eq!(
            repos,
            [scene.api()],
            "only the declared owner's repos the account reaches"
        );
        spent(&over);
    }

    pub fn about(kind: &str, how: How) {
        let Some(over) = over(kind, "about", how) else {
            return;
        };
        // Asked of the repo itself, as the inventory reads it: no forge id, which `about` never
        // needs, since both forges address the repo by its path.
        let repo = purlis_core::forge::RepoRecord {
            id: None,
            ..over.scene.api()
        };
        assert_eq!(
            over.backend.about(&over.caller, &repo),
            Ok(About {
                visibility: Visibility::Private,
                issues: Issues::Open,
                // The account is no admin, so neither forge names the setting.
                push_protection: PushProtection::Unknown,
            })
        );
        spent(&over);
    }

    pub fn top_level(kind: &str, how: How) {
        let Some(over) = over(kind, "top_level", how) else {
            return;
        };
        let scene = &over.scene;
        let repo = scene.repo(&scene.tree_id, "api", &[]);
        assert_eq!(
            over.backend.top_level(&over.caller, &repo, None).unwrap(),
            ["Cargo.toml", "src"]
        );
        spent(&over);
    }

    pub fn open_or_update(kind: &str, how: How) {
        let Some(over) = over(kind, "open_or_update", how) else {
            return;
        };
        let scene = &over.scene;
        let opened = over
            .backend
            .open_or_update(
                &over.caller,
                &scene.path("api"),
                &scene.opened_from,
                "main",
                "Save",
                "<!-- charter-save -->",
            )
            .unwrap();
        // On a live forge the first night opens the request and every later night updates it;
        // its number is the forge's to give.
        let number = scene.opened.unwrap_or(opened.pr.number);
        assert_eq!(
            opened,
            Opened {
                pr: Pr {
                    number,
                    url: scene.request_url(number)
                },
                ours: true
            }
        );
        spent(&over);
    }

    pub fn state(kind: &str, how: How) {
        let Some(over) = over(kind, "state", how) else {
            return;
        };
        let scene = &over.scene;
        let pr = Pr {
            number: scene.merged,
            url: String::new(),
        };
        assert_eq!(
            over.backend.state(&over.caller, &scene.path("api"), &pr),
            Ok(State::Merged {
                commit: Some(scene.merge.clone())
            })
        );
        spent(&over);
    }

    /// The body a request's description holds, as `charter change push` splices it.
    pub const BODY: &str = "Why this change.\n\n<!-- END charter change -->";

    /// The scene's open request.
    fn open(scene: &Scene) -> Pr {
        Pr {
            number: scene.open,
            url: String::new(),
        }
    }

    pub fn body(kind: &str, how: How) {
        let Some(over) = over(kind, "body", how) else {
            return;
        };
        let scene = &over.scene;
        assert_eq!(
            over.backend
                .body(&over.caller, &scene.path("api"), &open(scene)),
            Ok(BODY.to_string())
        );
        spent(&over);
    }

    pub fn keep_images(kind: &str, how: How) {
        let Some(over) = over(kind, "keep_images", how) else {
            return;
        };
        let scene = &over.scene;
        let kept = over
            .backend
            .keep_images(
                &over.caller,
                &scene.path("api"),
                &open(scene),
                &proof_images(),
            )
            .unwrap();
        let commit = "6".repeat(40);
        let want = match kind {
            "github" => vec![
                format!(
                    "https://{}/acme/api/raw/{commit}/browser-proof-1.png",
                    over.host
                ),
                format!(
                    "https://{}/acme/api/raw/{commit}/browser-proof-2.png",
                    over.host
                ),
            ],
            _ => vec![
                "/uploads/66dbcd21ec5d24ed6ea225176098d52b/browser-proof-1.png".to_string(),
                "/uploads/0a1b2c3d4e5f60718293a4b5c6d7e8f9/browser-proof-2.png".to_string(),
            ],
        };
        assert_eq!(kept, want);
        spent(&over);
    }

    pub fn comment(kind: &str, how: How) {
        let Some(over) = over(kind, "comment", how) else {
            return;
        };
        let scene = &over.scene;
        let page = match kind {
            "github" => "https://github.com/acme/api/pull/12",
            _ => "https://gitlab.com/acme/api/-/merge_requests/12",
        };
        let pr = Pr {
            number: scene.open,
            url: page.to_string(),
        };
        let said = over
            .backend
            .comment(&over.caller, &scene.path("api"), &pr, "Browser proof");
        let want = match kind {
            "github" => "https://github.com/acme/api/pull/12#issuecomment-99",
            _ => "https://gitlab.com/acme/api/-/merge_requests/12#note_99",
        };
        assert_eq!(said, Ok(want.to_string()));
        spent(&over);
    }

    pub fn set_body(kind: &str, how: How) {
        let Some(over) = over(kind, "set_body", how) else {
            return;
        };
        let scene = &over.scene;
        assert_eq!(
            over.backend
                .set_body(&over.caller, &scene.path("api"), &open(scene), BODY),
            Ok(())
        );
        spent(&over);
    }

    pub fn by_head(kind: &str, how: How) {
        let Some(over) = over(kind, "by_head", how) else {
            return;
        };
        let scene = &over.scene;
        let found = over
            .backend
            .by_head(&over.caller, &scene.path("api"), SAVE)
            .unwrap()
            .expect("a request");
        assert_eq!(
            (found.number, found.state, found.head.as_str()),
            (scene.open, State::Open, scene.head.as_str())
        );
        let _: Request = found;
        spent(&over);
    }

    pub fn request_auto_merge(kind: &str, how: How) {
        let Some(over) = over(kind, "request_auto_merge", how) else {
            return;
        };
        let scene = &over.scene;
        assert_eq!(
            over.backend.request_auto_merge(
                &over.caller,
                &scene.path("api"),
                &open(scene),
                &scene.head
            ),
            Ok(AutoMerge::Queued)
        );
        spent(&over);
    }

    /// purlis merges a `pr-merge` save's request itself: here the request is ready, its one
    /// check passed, and it is merged at the head, by the repo's own preferred method.
    pub fn merge_when_ready(kind: &str, how: How) {
        let Some(over) = over(kind, "merge_when_ready", how) else {
            return;
        };
        let scene = &over.scene;
        assert_eq!(
            over.backend.merge_when_ready(
                &over.caller,
                &scene.path("api"),
                &open(scene),
                &Pinned {
                    sha: &scene.head,
                    branch: "purlis/save/test",
                    base: "main",
                },
                Gate::Checks { settled: true }
            ),
            Ok(Ready::Merged)
        );
        spent(&over);
    }

    /// How every landing case asks to merge: a merge commit, carrying the trailer.
    fn merge_as(scene: &Scene) -> MergeAs {
        let sigil = scene.kind.change_sigil();
        MergeAs {
            squash: false,
            title: format!("api-2: api ({sigil}{})", scene.open),
            message: "bump the api\n\nCharter-Change: api-2".to_string(),
        }
    }

    pub fn lands_through_queue(kind: &str, how: How) {
        let Some(over) = over(kind, "lands_through_queue", how) else {
            return;
        };
        let scene = &over.scene;
        assert_eq!(
            over.backend
                .lands_through_queue(&over.caller, &scene.path("api"), &open(scene)),
            Ok(scene.queue)
        );
        spent(&over);
    }

    pub fn merge_at(kind: &str, how: How) {
        let Some(over) = over(kind, "merge_at", how) else {
            return;
        };
        let scene = &over.scene;
        assert_eq!(
            over.backend.merge_at(
                &over.caller,
                &scene.path("api"),
                &open(scene),
                &scene.head,
                &merge_as(scene)
            ),
            Ok(MergedAt::Now)
        );
        spent(&over);
    }

    pub fn enqueue_at(kind: &str, how: How) {
        let Some(over) = over(kind, "enqueue_at", how) else {
            return;
        };
        let scene = &over.scene;
        assert_eq!(
            over.backend.enqueue_at(
                &over.caller,
                &scene.path("api"),
                &open(scene),
                &scene.head,
                &merge_as(scene)
            ),
            Ok(())
        );
        spent(&over);
    }

    pub fn checks_at(kind: &str, how: How) {
        let Some(over) = over(kind, "checks_at", how) else {
            return;
        };
        let scene = &over.scene;
        assert_eq!(
            over.backend
                .checks_at(&over.caller, &scene.path("api"), &scene.head, scene.open),
            Checks {
                total: Some(1),
                ci: Ci::Passed,
                why: None,
                failed: Vec::new(),
            }
        );
        spent(&over);
    }

    pub fn annotations(kind: &str, how: How) {
        let Some(over) = over(kind, "annotations", how) else {
            return;
        };
        let scene = &over.scene;
        let read_by = |run| FailedCheck::of(Some("lint"), None).read_by(run);
        // What broke comes first, whatever order the forge lists its findings in.
        let found = over
            .backend
            .annotations(&over.caller, &scene.path("api"), &read_by(Some(4)))
            .expect("the findings read");
        let first = match kind {
            "github" => Annotation {
                title: Some("unused variable".into()),
                at: Some("src/lib.rs:3".into()),
                message: "`x` is never read".into(),
            },
            _ => Annotation {
                title: Some("lint (stage test)".into()),
                at: None,
                message: "the job failed: script_failure".into(),
            },
        };
        assert_eq!(found.first(), Some(&first), "{found:?}");
        // A check the forge reads nothing by (a commit status) is not asked about.
        assert_eq!(
            over.backend
                .annotations(&over.caller, &scene.path("api"), &read_by(None)),
            Ok(Vec::new())
        );
        spent(&over);
    }

    pub fn open_on_branch(kind: &str, how: How) {
        let Some(over) = over(kind, "open_on_branch", how) else {
            return;
        };
        let scene = &over.scene;
        assert_eq!(
            over.backend
                .open_on_branch(&over.caller, &scene.path("api"), SAVE),
            Ok(Some(Value::from(scene.open)))
        );
        spent(&over);
    }

    pub fn ci_word(kind: &str, how: How) {
        let Some(over) = over(kind, "ci_word", how) else {
            return;
        };
        let scene = &over.scene;
        assert_eq!(
            over.backend.ci_word(&over.caller, &scene.path("api"), SAVE),
            Ok(Some("success".to_string()))
        );
        spent(&over);
    }

    pub fn support(kind: &str, how: How) {
        let Some(over) = over(kind, "support", how) else {
            return;
        };
        let backend = &over.backend;
        let me = &over.caller;
        let repo = Reach::Repo(over.scene.path("api"));
        // What no probe answers is never yes of a repo: it turns on the repo's settings, which
        // purlis has not asked. These questions ask nothing.
        for what in [
            Capability::AutoMerge,
            Capability::SubIssues,
            Capability::Boards,
        ] {
            assert_ne!(
                backend.support(me, &repo, what),
                Support::Available,
                "{what:?}: what charter has not asked a repo about is never yes"
            );
        }
        // Epics, said of the instance: GitHub has none anywhere; gitlab.com has them; a
        // self-managed GitLab has them where it says it is the Enterprise Edition and its licence
        // says Premium (FG-2, #802).
        let epics = backend.support(me, &Reach::Instance, Capability::Epics);
        // Sub-issues, said of the instance: github.com has them; a GitHub Enterprise Server has
        // what its version has, which charter has not asked.
        let sub_issues = backend.support(me, &Reach::Instance, Capability::SubIssues);
        match (kind, over.host.as_str()) {
            ("github", host) => {
                assert!(matches!(epics, Support::Unavailable(_)), "{epics:?}");
                // The recorded scene's owner is an organisation (`users/acme`): its repos have
                // issue types on github.com (#1202), and a GHES has them from a version not
                // asked. A live fixture's owner is whichever kind it is, and says so.
                let types = backend.support(me, &repo, Capability::IssueTypes);
                if host == "github.com" {
                    assert_eq!(sub_issues, Support::Available);
                    if over.scene == Scene::recorded(Kind::GitHub) {
                        assert_eq!(types, Support::Available);
                    } else {
                        assert!(!matches!(types, Support::Unknown(_)), "{types:?}");
                    }
                } else {
                    assert!(matches!(sub_issues, Support::Unknown(_)), "{sub_issues:?}");
                    assert!(matches!(types, Support::Unknown(_)), "{types:?}");
                }
            }
            ("gitlab", "gitlab.com") => {
                assert_eq!(epics, Support::Available);
                // The recorded scene's group is on Premium (`namespaces/acme`), so its repo has
                // epics. A live fixture's plan is its own, and shown only to its owners.
                let repo_epics = backend.support(me, &repo, Capability::Epics);
                if over.scene == Scene::recorded(Kind::GitLab) {
                    assert_eq!(repo_epics, Support::Available);
                }
            }
            ("gitlab", _) => {
                // The self-managed recording: GitLab 19.4 Enterprise Edition, licensed Premium,
                // asked once: the backend keeps each probe's answer for the next question
                // (#802). A work item's own status needs 18.1 as well.
                assert_eq!(epics, Support::Available);
                assert_eq!(
                    backend.support(me, &Reach::Instance, Capability::ItemStatus),
                    Support::Available
                );
                // A repo's own plan is not asked of a self-managed GitLab.
                assert!(matches!(
                    backend.support(me, &repo, Capability::Epics),
                    Support::Unknown(_)
                ));
            }
            _ => unreachable!("two forges"),
        }
        spent(&over);
    }

    /// The owners a project can be made under (#1819): the account first, then its
    /// organisations (GitHub) or the groups it may make a project in (GitLab).
    pub fn owners_to_create_under(kind: &str, how: How) {
        let Some(over) = over(kind, "owners_to_create_under", how) else {
            return;
        };
        let owners = over.backend.owners_to_create_under(&over.caller).unwrap();
        let named: Vec<(&str, OwnerKind, Option<&str>)> = owners
            .iter()
            .map(|o| {
                (
                    o.name.as_str(),
                    o.kind,
                    o.id.as_ref().map(|id| id.0.as_str()),
                )
            })
            .collect();
        let expected: [(&str, OwnerKind, Option<&str>); 3] = if kind == "github" {
            [
                ("octocat", OwnerKind::You, Some("583231")),
                ("acme", OwnerKind::Organisation, Some("9919")),
                ("tools-guild", OwnerKind::Organisation, Some("9920")),
            ]
        } else {
            [
                ("octocat", OwnerKind::You, Some("4021")),
                ("acme", OwnerKind::Organisation, Some("42")),
                ("acme/tools", OwnerKind::Organisation, Some("43")),
            ]
        };
        assert_eq!(named, expected);
        assert_eq!(owners[0].title, "The Octocat", "the account's own name");
        spent(&over);
    }

    /// The owner it is made under, as the forge listed it (GitLab's group by its id).
    fn acme() -> RepoOwner {
        RepoOwner {
            name: "acme".into(),
            title: "Acme".into(),
            kind: OwnerKind::Organisation,
            id: Some(ForgeRef("42".into())),
        }
    }

    /// An empty private repo under an organisation, answered with what deleting it needs.
    pub fn create_repo(kind: &str, how: How) {
        let Some(over) = over(kind, "create_repo", how) else {
            return;
        };
        let made = over
            .backend
            .create_repo(
                &over.caller,
                &NewRepo {
                    owner: acme(),
                    name: "garden".into(),
                    visibility: Visibility::Private,
                },
            )
            .unwrap();
        assert_eq!(made.path(), "acme/garden");
        assert_eq!(made.web_url(), format!("https://{kind}.com/acme/garden"));
        assert_eq!(
            made.clone_url(),
            format!("https://{kind}.com/acme/garden.git")
        );
        let id = if kind == "github" {
            "1296269"
        } else {
            "278964"
        };
        assert_eq!(made.id(), &ForgeRef(id.into()));
        spent(&over);
    }

    /// The repo a create answered, deleted: on GitHub once the repo at its path still has its
    /// id, on GitLab by that id.
    pub fn delete_created_repo(kind: &str, how: How) {
        let Some(over) = over(kind, "delete_created_repo", how) else {
            return;
        };
        let id = if kind == "github" {
            "1296269"
        } else {
            "278964"
        };
        let made = MadeRepo::for_a_test(
            id,
            "acme/garden",
            &format!("https://{kind}.com/acme/garden.git"),
        );
        over.backend
            .delete_created_repo(&over.caller, &made)
            .unwrap();
        spent(&over);
    }

    pub fn create(kind: &str, how: How) {
        let Some(over) = over(kind, "create", how) else {
            return;
        };
        let scene = &over.scene;
        // Swept before, for what an earlier run left, and after, whatever `create` answered.
        (over.sweep)().unwrap_or_else(|e| panic!("sweeping before the case: {e}"));
        let made = over.backend.create(
            &over.caller,
            &scene.path("api"),
            &NewWorkItem {
                title: live::ISSUE_TITLE.into(),
                body: "What the todo said.\n\nSecond paragraph.".into(),
                workspace_label: Some("alpha".into()),
            },
        );
        let swept = (over.sweep)();
        let made = made.unwrap();
        let swept = swept.unwrap_or_else(|e| panic!("sweeping after the case: {e}"));
        // On a live forge the issue's number and id are the forge's to give: the number is read
        // off the page the answer names, must agree with its key, and must be one the sweep
        // found open under the case's title.
        let (number, forge_ref) = match &scene.created {
            Some((number, forge_ref)) => (*number, Some(forge_ref.clone())),
            None => {
                let number = made
                    .url
                    .rsplit('/')
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or_else(|| panic!("no issue number ends {}", made.url));
                assert!(swept.contains(&number), "{number} not among {swept:?}");
                (number, None)
            }
        };
        assert_eq!(made.key.as_str(), scene.issue_key(number));
        assert_eq!(made.url, scene.issue_url(number));
        // The answer, in the neutral model (FW-5): what a board draws the new issue's card from.
        assert_eq!(made.title, live::ISSUE_TITLE);
        assert_eq!(made.kind, work::Kind::Issue);
        assert_eq!(made.state, work::State::Open);
        let label = match kind {
            "github" => "ws:alpha",
            _ => "charter::ws::alpha",
        };
        assert_eq!(made.labels, [label]);
        assert!(!made.is_sub_issue());
        assert_eq!(
            made.milestone, None,
            "the case opens its issue in no milestone"
        );
        match forge_ref {
            Some(forge_ref) => assert_eq!(made.forge_ref, Some(ForgeRef(forge_ref))),
            None => assert!(
                made.forge_ref.as_ref().is_some_and(|r| !r.0.is_empty()),
                "{:?}",
                made.forge_ref
            ),
        }
        spent(&over);
    }

    pub fn search(kind: &str, how: How) {
        let Some(over) = over(kind, "search", how) else {
            return;
        };
        let scene = &over.scene;
        let found = over
            .backend
            .search(&over.caller, &scene.path("api"), "picker crash", 5)
            .unwrap();
        // Issues only, open and closed alike, in the forge's order, each keyed as `create` keys
        // the issue it opens. GitHub's answer holds a pull request too, which is left out.
        let read: Vec<(String, String, &str, work::State)> = found
            .iter()
            .map(|i| {
                (
                    i.key.as_str().to_string(),
                    i.url.clone(),
                    i.title.as_str(),
                    i.state,
                )
            })
            .collect();
        assert_eq!(
            read,
            [
                (
                    scene.issue_key(12),
                    scene.issue_url(12),
                    "Port the picker",
                    work::State::Open
                ),
                (
                    scene.issue_key(9),
                    scene.issue_url(9),
                    "The picker crashes on an empty list",
                    work::State::Closed
                ),
            ]
        );
        assert!(found.iter().all(|i| i.kind == work::Kind::Issue));
        spent(&over);
    }

    pub fn read(kind: &str, how: How) {
        let Some(over) = over(kind, "read", how) else {
            return;
        };
        let scene = &over.scene;
        let item = over
            .backend
            .read(&over.caller, &scene.path("api"), 12)
            .unwrap();
        // What both forges answer: the issue, its fields, and who it is assigned to.
        assert_eq!(item.key.as_str(), scene.issue_key(12));
        assert_eq!(item.url, scene.issue_url(12));
        assert_eq!(item.title, live::ISSUE_TITLE);
        assert_eq!(item.kind, work::Kind::Issue);
        assert_eq!(item.state, work::State::Closed);
        assert_eq!(
            item.milestone.as_ref().map(|m| m.title.as_str()),
            Some("v1")
        );
        assert_eq!(item.assignees, ["octocat"]);
        let key = |number: u64| work::TrackerKey::parse(&scene.issue_key(number)).unwrap();
        // How it stands to other items, the same on both forges: GitHub's parent, sub-issue,
        // dependencies and closing pull request; GitLab's hierarchy, blocking links and closing
        // merge request.
        assert_eq!(
            item.relations,
            [
                work::Relation::Parent(key(3)),
                work::Relation::Child(key(14)),
                work::Relation::BlockedBy(key(11)),
                work::Relation::Blocks(key(15)),
                work::Relation::ClosedBy(Pr {
                    number: 20,
                    url: scene.request_url(20),
                }),
            ]
        );
        let day = |d| chrono::NaiveDate::from_ymd_opt(2026, 10, d);
        let sprint = item.iteration.as_ref().expect("the item's iteration");
        assert_eq!(sprint.title.as_deref(), Some("Sprint 3"));
        assert_eq!((sprint.start, sprint.end), (day(5), day(18)));
        assert_eq!(item.placements.len(), 1);
        assert_eq!(item.placements[0].board_title, "Roadmap");
        // A status of the item's own, and the close reason its category gives, are asked only
        // where GitLab's work item status is known to be there: not of a self-managed GitLab,
        // which charter has not probed. There the item has its boards' status, and a closed
        // issue sits in no label list.
        let own_status = !(kind == "gitlab" && over.host != "gitlab.com");
        if own_status {
            assert_eq!(item.status.as_deref(), Some("Done"));
            assert_eq!(item.closed_as, Some(work::ClosedAs::Completed));
        } else {
            assert_eq!((item.status.as_deref(), item.closed_as), (None, None));
        }
        match kind {
            "github" => {
                assert_eq!(item.labels, ["ws:alpha"]);
                assert_eq!(item.issue_type.as_deref(), Some("Bug"));
                // An issue has no status of its own on GitHub: it is its board's.
                assert_eq!(item.placements[0].status.as_deref(), Some("Done"));
                assert_eq!(
                    item.placements[0].iteration.as_ref(),
                    Some(sprint),
                    "the item's iteration is its board's"
                );
            }
            _ => {
                assert_eq!(item.labels, ["charter::ws::alpha"]);
                // GitLab's work item type, and its own iteration and status (Premium). A closed
                // issue sits in no label list of a board, and its close reason is its status's
                // category.
                assert_eq!(item.issue_type.as_deref(), Some("Issue"));
                assert_eq!(item.placements[0].status, None);
                assert_eq!(item.placements[0].iteration, None);
            }
        }
        spent(&over);
    }
}

/// One module per forge, holding every case of [`CASES`]. `ignore = "<why>"` ignores each.
macro_rules! contract {
    ($module:ident, $forge:ident, $how:ident) => {
        contract!(@cases $module, $forge, $how,);
    };
    ($module:ident, $forge:ident, $how:ident, ignore = $why:literal) => {
        contract!(@cases $module, $forge, $how, ignore = $why);
    };
    (@cases $module:ident, $forge:ident, $how:ident, $($attr:meta)?) => {
        mod $module {
            #[test]
            $(#[$attr])?
            fn owned_lists_every_repo_of_the_owner_in_the_neutral_shape() {
                purlis_core::unsteered!();
                super::cases::owned(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn reachable_keeps_only_the_declared_owners_repos() {
                purlis_core::unsteered!();
                super::cases::reachable(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn top_level_lists_the_default_branchs_top_level_names() {
                purlis_core::unsteered!();
                super::cases::top_level(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn open_or_update_opens_one_request_when_none_is_open() {
                purlis_core::unsteered!();
                super::cases::open_or_update(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn state_reads_a_merged_request_with_the_commit_it_landed_as() {
                purlis_core::unsteered!();
                super::cases::state(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn body_reads_the_requests_description_whole() {
                purlis_core::unsteered!();
                super::cases::body(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn set_body_replaces_the_requests_description_and_nothing_else() {
                purlis_core::unsteered!();
                super::cases::set_body(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn by_head_finds_the_request_from_the_branch_with_its_head_commit() {
                purlis_core::unsteered!();
                super::cases::by_head(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn request_auto_merge_queues_the_merge_at_the_pushed_commit() {
                purlis_core::unsteered!();
                super::cases::request_auto_merge(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn merge_when_ready_merges_a_ready_request_at_the_pushed_commit() {
                purlis_core::unsteered!();
                super::cases::merge_when_ready(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn lands_through_queue_reads_whether_the_target_branch_has_a_queue() {
                purlis_core::unsteered!();
                super::cases::lands_through_queue(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn merge_at_merges_now_only_at_the_head_charter_read() {
                purlis_core::unsteered!();
                super::cases::merge_at(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn enqueue_at_puts_the_request_in_the_queue_only_at_the_head_charter_read() {
                purlis_core::unsteered!();
                super::cases::enqueue_at(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn checks_at_counts_one_passing_check_at_the_head() {
                purlis_core::unsteered!();
                super::cases::checks_at(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn annotations_reads_what_a_failed_check_found_failures_first() {
                purlis_core::unsteered!();
                super::cases::annotations(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn open_on_branch_names_the_open_requests_number() {
                purlis_core::unsteered!();
                super::cases::open_on_branch(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn ci_word_maps_the_forges_success_to_success() {
                purlis_core::unsteered!();
                super::cases::ci_word(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn support_never_reads_silence_as_yes() {
                purlis_core::unsteered!();
                super::cases::support(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn about_says_whether_the_repo_is_public_and_takes_issues() {
                purlis_core::unsteered!();
                super::cases::about(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn create_opens_an_issue_and_names_it_by_its_tracker_key_with_its_forge_ref_beside() {
                purlis_core::unsteered!();
                super::cases::create(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn read_maps_an_issue_onto_the_work_model_with_its_relations_and_boards() {
                purlis_core::unsteered!();
                super::cases::read(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn search_finds_the_repos_issues_that_match_and_no_request() {
                purlis_core::unsteered!();
                super::cases::search(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn owners_to_create_under_lists_the_account_then_its_organisations() {
                purlis_core::unsteered!();
                super::cases::owners_to_create_under(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn create_repo_makes_an_empty_private_repo_under_the_owner() {
                purlis_core::unsteered!();
                super::cases::create_repo(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn delete_created_repo_deletes_the_repo_the_create_answered() {
                purlis_core::unsteered!();
                super::cases::delete_created_repo(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn keep_images_keeps_each_picture_and_answers_where_the_request_shows_it() {
                purlis_core::unsteered!();
                super::cases::keep_images(stringify!($forge), super::How::$how);
            }
            #[test]
            $(#[$attr])?
            fn comment_posts_on_the_request_and_answers_the_comments_page() {
                purlis_core::unsteered!();
                super::cases::comment(stringify!($forge), super::How::$how);
            }
        }
    };
}

/// The two pictures the browser proof cases keep, as the recordings carry them.
fn proof_images() -> Vec<purlis_core::forge::pr::Image> {
    vec![
        purlis_core::forge::pr::Image {
            name: "browser-proof-1.png".into(),
            png: b"\x89PNG\r\n\x1a\none".to_vec(),
        },
        purlis_core::forge::pr::Image {
            name: "browser-proof-2.png".into(),
            png: b"\x89PNG\r\n\x1a\ntwo".to_vec(),
        },
    ]
}

/// GitHub's first proof on a request: the branch is not there, so its commit has no parent and
/// the branch is made at it, never on top of anything of the repo's own (#2026).
#[test]
fn githubs_first_proof_on_a_request_starts_a_branch_with_no_parent() {
    purlis_core::unsteered!();
    let text = recording("github", "keep_images.first");
    let recorded = Arc::new(Recorded::parse(&text).unwrap());
    let backend = Forge::default_of(Kind::GitHub).backend_over(recorded.clone());
    let pr = Pr {
        number: 12,
        url: String::new(),
    };
    let commit = "6".repeat(40);
    assert_eq!(
        backend.keep_images(&caller(), "acme/api", &pr, &proof_images()),
        Ok(vec![
            format!("https://github.com/acme/api/raw/{commit}/browser-proof-1.png"),
            format!("https://github.com/acme/api/raw/{commit}/browser-proof-2.png"),
        ])
    );
    spent(&recorded);
}

contract!(github, github, Recorded);
contract!(gitlab, gitlab, Recorded);
// The native transport, against a server answering the same recordings over HTTP: on each
// forge's own instance, and on a self-managed instance of each: a GitHub Enterprise Server and a
// self-managed GitLab (ADR 0070 §7: it gates every change).
contract!(github_native, github, Native);
contract!(gitlab_native, gitlab, Native);
contract!(github_self_managed, github, SelfManaged);
contract!(gitlab_self_managed, gitlab, SelfManaged);
// The live run: the same cases against a real forge (FG-4). Only the nightly holds the tokens.
contract!(
    github_live,
    github,
    Live,
    ignore = "live: a real github.com repo; run by .github/workflows/forge-live.yml"
);
contract!(
    gitlab_live,
    gitlab,
    Live,
    ignore = "live: a real gitlab.com repo; run by .github/workflows/forge-live.yml"
);

/// The method names of one `pub trait <name> { … }` block in `backend.rs`.
fn methods_of(source: &str, name: &str) -> Vec<String> {
    let start = source
        .find(&format!("pub trait {name} {{"))
        .unwrap_or_else(|| panic!("backend.rs declares no trait {name}"));
    let body = &source[start..];
    let end = body.find("\n}\n").expect("the trait's closing brace");
    body[..end]
        .lines()
        .filter_map(|line| line.trim().strip_prefix("fn "))
        .map(|rest| rest.split('(').next().unwrap().trim().to_string())
        .collect()
}

#[test]
fn every_method_of_the_seam_has_a_case_on_both_forges() {
    purlis_core::unsteered!();
    let source =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/forge/backend.rs"))
            .unwrap();
    let mut methods: Vec<String> = ["Repos", "Making", "Requests", "Capabilities", "WorkItems"]
        .iter()
        .flat_map(|t| methods_of(&source, t))
        .collect();
    methods.sort();
    let mut cases: Vec<String> = CASES.iter().map(|c| c.to_string()).collect();
    cases.sort();
    assert_eq!(
        methods, cases,
        "a seam method with no contract case, or the reverse"
    );
    for kind in ["github", "gitlab"] {
        for case in CASES {
            let file = recordings().join(kind).join(format!("{case}.json"));
            assert!(file.is_file(), "no {kind} recording for {case}");
        }
    }
    // The macro gives both forges the same tests; this file holds exactly one instantiation per
    // forge, so neither can drift from the other.
    let me = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/forge_contract.rs"),
    )
    .unwrap();
    for kind in ["github", "gitlab"] {
        assert_eq!(
            me.matches(&format!("contract!({kind}, {kind}, Recorded);"))
                .count(),
            1,
            "{kind} is instantiated once"
        );
    }
    for (module, kind, how) in [
        ("github_native", "github", "Native"),
        ("gitlab_native", "gitlab", "Native"),
        ("github_self_managed", "github", "SelfManaged"),
        ("gitlab_self_managed", "gitlab", "SelfManaged"),
    ] {
        assert_eq!(
            me.matches(&format!("contract!({module}, {kind}, {how});"))
                .count(),
            1,
            "the native transport runs every case: {module}"
        );
    }
    // The live run (FG-4) is the same macro over a real forge: one instantiation per forge, ignored
    // so that only the nightly, which holds the tokens, asks for it.
    let flat: String = me.split_whitespace().collect();
    for kind in ["github", "gitlab"] {
        assert_eq!(
            flat.matches(&format!("contract!({kind}_live,{kind},Live,ignore="))
                .count(),
            1,
            "the live run runs every case on {kind}"
        );
    }
}

/// The recordings of one forge's own scene beside a case's, each read by a test of its own:
/// `<case>.<scene>.json`.
const SCENES: [(&str, &str, &str); 22] = [
    ("github", "keep_images", "first"),
    ("gitlab", "request_auto_merge", "train"),
    ("gitlab", "request_auto_merge", "train_pre19"),
    ("github", "merge_when_ready", "pending"),
    ("github", "merge_when_ready", "failing"),
    ("github", "merge_when_ready", "no_checks"),
    ("github", "merge_when_ready", "expected"),
    ("github", "merge_when_ready", "expected_own"),
    ("github", "merge_when_ready", "late_failure"),
    ("github", "merge_when_ready", "cancelled"),
    ("github", "merge_when_ready", "passed_quietly"),
    ("github", "merge_when_ready", "unrequired"),
    ("github", "merge_when_ready", "queue"),
    ("gitlab", "merge_when_ready", "pending"),
    ("gitlab", "merge_when_ready", "failing"),
    ("gitlab", "merge_when_ready", "no_checks"),
    ("gitlab", "merge_when_ready", "expected"),
    ("gitlab", "merge_when_ready", "late_failure"),
    ("gitlab", "merge_when_ready", "cancelled"),
    ("gitlab", "merge_when_ready", "skipped"),
    ("gitlab", "merge_when_ready", "unrequired"),
    ("gitlab", "merge_when_ready", "queue"),
];

/// Every recording names a case of [`CASES`], by its stem with or without `.self_managed`, or
/// is one of [`SCENES`]. A self-managed override whose case was renamed would otherwise go
/// unread, and the self-managed run would quietly fall back to the case's main recording.
#[test]
fn every_recording_on_both_forges_names_a_case() {
    purlis_core::unsteered!();
    for kind in ["github", "gitlab"] {
        let dir = recordings().join(kind);
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let stem = name
                .strip_suffix(".json")
                .unwrap_or_else(|| panic!("{kind}: {name} is no recording"));
            if SCENES
                .iter()
                .any(|(k, case, scene)| *k == kind && stem == format!("{case}.{scene}"))
            {
                continue;
            }
            let case = stem.strip_suffix(".self_managed").unwrap_or(stem);
            assert!(
                CASES.contains(&case),
                "{kind}: {name} names no case of the contract"
            );
        }
    }
}

/// A self-managed override that is not there is no override; one that is there but cannot be
/// read fails the case rather than passing it on the main recording.
#[test]
fn a_self_managed_override_that_cannot_be_read_fails_rather_than_falling_back() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        own_recording(&dir.path().join("absent.self_managed.json")),
        None
    );
    let file = dir.path().join("there.self_managed.json");
    std::fs::write(&file, "{}").unwrap();
    assert_eq!(own_recording(&file).as_deref(), Some("{}"));
    // A directory in the file's place is there, and reading it is an error, not a miss.
    let unreadable = dir.path().join("dir.self_managed.json");
    std::fs::create_dir(&unreadable).unwrap();
    let said = std::panic::catch_unwind(|| own_recording(&unreadable));
    assert!(
        said.is_err(),
        "an unreadable override passed as no override"
    );
}

/// A case the live run does not run says why, and names the ticket that will run it (ADR 0070
/// §7: a test is never skipped silently).
#[test]
fn every_case_the_live_run_leaves_out_says_why_and_names_its_ticket() {
    purlis_core::unsteered!();
    let ticket = regex::Regex::new(r"#\d+\b").unwrap();
    for (case, why) in NOT_LIVE {
        assert!(CASES.contains(&case), "{case} is no case of the contract");
        assert!(
            ticket.is_match(why),
            "{case}: the reason names no ticket: {why}"
        );
    }
    assert!(
        NOT_LIVE.len() < CASES.len() / 2,
        "the live run leaves out most of the contract"
    );
}

/// GitLab statuses its own documentation lists that the Python port did not (GitLab 19.4
/// `doc/api/pipelines.md`, the `status` filter: thirteen values). These are GitLab's alone, so
/// they are not parity cases.
mod gitlab_statuses {
    use super::*;

    /// A backend over an inline recording.
    fn inline(exchanges: Value) -> (Box<dyn ForgeBackend>, Arc<Recorded>) {
        let text =
            json!({"source": "GitLab 19.4 REST API docs, pipelines.md", "exchanges": exchanges});
        let recorded = Arc::new(Recorded::parse(&text.to_string()).unwrap());
        let forge = Forge::default_of(purlis_core::forge::Kind::GitLab);
        (forge.backend_over(recorded.clone()), recorded)
    }

    fn get(path: &str, out: Value) -> Value {
        json!({"call": {"endpoint": {"rest": {"method": null, "path": path}}, "fields": []},
               "reply": {"code": 0, "out": out.to_string()}})
    }

    fn ci_word_of(status: &str) -> Option<String> {
        let (backend, recorded) = inline(json!([get(
            "projects/acme%2Fapi/pipelines?ref=main&per_page=1",
            json!([{"id": 1, "status": status}])
        )]));
        let word = backend.ci_word(&caller(), "acme/api", "main").unwrap();
        spent(&recorded);
        word
    }

    fn checks_of(status: &str) -> Ci {
        let (backend, recorded) = inline(json!([get(
            "projects/acme%2Fapi/merge_requests/12/pipelines?per_page=100",
            json!([{"id": 1, "sha": SHA, "status": status}])
        )]));
        let checks = backend.checks_at(&caller(), "acme/api", SHA, 12);
        spent(&recorded);
        checks.ci
    }

    #[test]
    fn a_pipeline_waiting_for_a_callback_is_pending_and_still_running() {
        purlis_core::unsteered!();
        assert_eq!(ci_word_of("waiting_for_callback"), Some("pending".into()));
        assert_eq!(checks_of("waiting_for_callback"), Ci::Running);
    }

    #[test]
    fn a_pipeline_being_canceled_reads_as_canceled_and_did_not_pass() {
        purlis_core::unsteered!();
        assert_eq!(ci_word_of("canceling"), Some("canceled".into()));
        assert_eq!(checks_of("canceling"), Ci::Failed);
    }

    #[test]
    fn auto_merge_waits_on_a_pipeline_waiting_for_a_callback() {
        purlis_core::unsteered!();
        let (backend, recorded) = inline(json!([
            get("projects/acme%2Fapi", json!({"squash_option": "never"})),
            get(
                "projects/acme%2Fapi/merge_requests/12",
                json!({"iid": 12, "head_pipeline": {"status": "waiting_for_callback"}})
            ),
            get("metadata", json!({"version": "17.10.0"})),
            {"call": {"endpoint": {"rest": {"method": "PUT",
                                            "path": "projects/acme%2Fapi/merge_requests/12/merge"}},
                      "fields": [{"typed": ["merge_when_pipeline_succeeds", "true"]},
                                 {"typed": ["squash", "false"]},
                                 {"text": ["sha", SHA]}]},
             "reply": {"code": 0, "out": "{\"iid\": 12}"}}
        ]));
        let pr = Pr {
            number: 12,
            url: String::new(),
        };
        assert_eq!(
            backend.request_auto_merge(&caller(), "acme/api", &pr, SHA),
            Ok(AutoMerge::Queued)
        );
        spent(&recorded);
    }
}

/// Which flag GitLab's auto-merge sends (#805): `auto_merge` where the instance says it is 17.11
/// or later, and the deprecated `merge_when_pipeline_succeeds` anywhere else, an instance that
/// said nothing included, because a GitLab older than `auto_merge` ignores it and merges at once.
/// The contract's `request_auto_merge` recording is the first arm, on GitLab 19.4.
mod gitlab_auto_merge_flag {
    use super::*;

    fn get(path: &str, reply: Value) -> Value {
        json!({"call": {"endpoint": {"rest": {"method": null, "path": path}}, "fields": []},
               "reply": reply})
    }

    fn ok(out: Value) -> Value {
        json!({"code": 0, "out": out.to_string()})
    }

    /// The exchanges of one auto-merge request, the instance answering `metadata` and the
    /// merge asked with `flag`.
    fn saved(metadata: Value, flag: &str) -> Value {
        json!([
            get("projects/acme%2Fapi", ok(json!({"squash_option": "default_off"}))),
            get(
                "projects/acme%2Fapi/merge_requests/12",
                ok(json!({"iid": 12, "head_pipeline": {"status": "running"}}))
            ),
            get("metadata", metadata),
            {"call": {"endpoint": {"rest": {"method": "PUT",
                                            "path": "projects/acme%2Fapi/merge_requests/12/merge"}},
                      "fields": [{"typed": [flag, "true"]},
                                 {"typed": ["squash", "false"]},
                                 {"text": ["sha", SHA]}]},
             "reply": {"code": 0, "out": "{\"iid\": 12, \"state\": \"opened\"}"}}
        ])
    }

    fn queued(exchanges: Value) {
        let text = json!({"source": "GitLab 19.4 REST API docs, metadata.md and merge_requests.md: \
                                     Merge a merge request (auto_merge since 17.11; \
                                     merge_when_pipeline_succeeds deprecated in 17.11)",
                          "exchanges": exchanges});
        let recorded = Arc::new(Recorded::parse(&text.to_string()).unwrap());
        let backend = Forge::default_of(Kind::GitLab).backend_over(recorded.clone());
        let pr = Pr {
            number: 12,
            url: String::new(),
        };
        assert_eq!(
            backend.request_auto_merge(&caller(), "acme/api", &pr, SHA),
            Ok(AutoMerge::Queued)
        );
        spent(&recorded);
    }

    fn version(version: &str) -> Value {
        ok(json!({"version": version, "revision": "c401a659d0c", "enterprise": false}))
    }

    #[test]
    fn gitlab_17_11_and_later_is_asked_with_auto_merge() {
        purlis_core::unsteered!();
        for at in ["17.11.0", "18.0.1-ee", "19.4.0-ee"] {
            queued(saved(version(at), "auto_merge"));
        }
    }

    #[test]
    fn a_gitlab_older_than_auto_merge_is_asked_with_the_deprecated_flag() {
        purlis_core::unsteered!();
        for at in ["17.10.4", "16.11.10-ee", "15.2.0"] {
            queued(saved(version(at), "merge_when_pipeline_succeeds"));
        }
    }

    #[test]
    fn an_instance_that_does_not_say_its_version_is_asked_with_the_deprecated_flag() {
        purlis_core::unsteered!();
        let not_found = json!({"code": 1, "out": "{\"message\":\"404 Not Found\"}",
                               "err": "glab: 404 Not Found (HTTP 404)", "status": 404});
        for answer in [
            not_found,
            ok(json!({"revision": "c401a659d0c"})),
            ok(json!({"version": "next"})),
        ] {
            queued(saved(answer, "merge_when_pipeline_succeeds"));
        }
    }
}

/// A GitLab repo with merge trains (#805): from 19.1, `auto_merge` adds the merge request to
/// the train, and the answer says so. Before 19.1 the same call merged past the train, so
/// nothing is sent and the request stays open for a person.
mod gitlab_merge_train {
    use super::*;

    const PR: Pr = Pr {
        number: 12,
        url: String::new(),
    };

    fn joins(how: How) {
        joins_from(how, "request_auto_merge.train");
    }

    fn joins_from(how: How, scene: &str) {
        let text = recording("gitlab", scene);
        let host = match how {
            How::SelfManaged => SELF_MANAGED,
            _ => "gitlab.com",
        };
        // A native case asks as the account it signed in with: the command's caller would be
        // sent down the CLI route, which a native case refuses.
        let (backend, check, asker): (Box<dyn ForgeBackend>, Box<dyn Fn()>, Caller) = match how {
            How::Recorded => {
                let recorded = Arc::new(Recorded::parse(&text).unwrap());
                let check = recorded.clone();
                (
                    Forge::default_of(Kind::GitLab).backend_over(recorded),
                    Box::new(move || check.spent()),
                    caller(),
                )
            }
            _ => {
                let over = native::over("gitlab", host, &text);
                (over.backend, over.check, over.caller)
            }
        };
        assert_eq!(
            backend.request_auto_merge(&asker, "acme/api", &PR, SHA),
            Ok(AutoMerge::JoinedTrain)
        );
        check();
    }

    #[test]
    fn a_repo_with_merge_trains_joins_the_train_and_says_so() {
        purlis_core::unsteered!();
        joins(How::Recorded);
    }

    #[test]
    fn a_repo_with_merge_trains_joins_the_train_over_the_native_transport() {
        purlis_core::unsteered!();
        joins(How::Native);
    }

    #[test]
    fn a_self_managed_repo_with_merge_trains_joins_the_train() {
        purlis_core::unsteered!();
        joins(How::SelfManaged);
    }

    fn get(path: &str, reply: Value) -> Value {
        json!({"call": {"endpoint": {"rest": {"method": null, "path": path}}, "fields": []},
               "reply": reply})
    }

    fn ok(out: Value) -> Value {
        json!({"code": 0, "out": out.to_string()})
    }

    /// Below 19.1 the merge call merges past a train, so a train repo's request is added to the
    /// train through the merge trains API instead (`doc/api/merge_trains.md`, "Add a merge
    /// request to a merge train"), set to join once its pipeline succeeds.
    #[test]
    fn a_repo_with_merge_trains_below_19_1_joins_through_the_merge_trains_api() {
        purlis_core::unsteered!();
        joins_from(How::Recorded, "request_auto_merge.train_pre19");
    }

    #[test]
    fn a_repo_with_merge_trains_below_19_1_joins_over_the_native_transport() {
        purlis_core::unsteered!();
        joins_from(How::Native, "request_auto_merge.train_pre19");
    }

    #[test]
    fn a_self_managed_repo_with_merge_trains_below_19_1_joins_the_train() {
        purlis_core::unsteered!();
        joins_from(How::SelfManaged, "request_auto_merge.train_pre19");
    }

    /// The scene with `metadata` answering `metadata` and the merge trains API answering
    /// `added` to a call carrying `flag`.
    fn below_19_1(metadata: Value, flag: &str, added: Value) -> Value {
        let post = json!({"call": {"endpoint": {"rest": {"method": "POST",
                              "path": "projects/acme%2Fapi/merge_trains/merge_requests/12"}},
                          "fields": [{"typed": [flag, "true"]}, {"typed": ["squash", "false"]},
                                     {"text": ["sha", SHA]}]},
                          "reply": added});
        json!({"source": "GitLab 19.4 REST API docs, projects.md (merge_trains_enabled), \
                          metadata.md, merge_requests.md, merge_trains.md: Add a merge request \
                          to a merge train (auto_merge from 17.11, when_pipeline_succeeds \
                          before it)",
               "exchanges": [
            get("projects/acme%2Fapi",
                ok(json!({"squash_option": "default_off", "merge_trains_enabled": true}))),
            get("projects/acme%2Fapi/merge_requests/12",
                ok(json!({"iid": 12, "head_pipeline": {"status": "running"}}))),
            get("metadata", metadata),
            post,
        ]})
    }

    fn car() -> Value {
        ok(
            json!([{"id": 267, "merge_request": {"id": 273, "iid": 12, "project_id": 3},
                   "status": "idle", "target_branch": "main"}]),
        )
    }

    fn not_found() -> Value {
        json!({"code": 1, "out": "{\"message\":\"404 Not Found\"}",
               "err": "glab: 404 Not Found (HTTP 404)", "status": 404})
    }

    /// An instance older than `auto_merge` (17.11), or one that does not say its version, is
    /// asked with `when_pipeline_succeeds`, which every GitLab with the merge trains API knows:
    /// a parameter it did not know would be dropped, and the request added at once.
    #[test]
    fn a_train_repo_on_a_gitlab_older_than_auto_merge_or_unsaid_joins_with_when_pipeline_succeeds()
    {
        purlis_core::unsteered!();
        for metadata in [
            ok(json!({"version": "17.10.2-ee", "enterprise": true})),
            ok(json!({"version": "16.11.0-ee", "enterprise": true})),
            not_found(),
        ] {
            let text = below_19_1(metadata.clone(), "when_pipeline_succeeds", car());
            let recorded = Arc::new(Recorded::parse(&text.to_string()).unwrap());
            let backend = Forge::default_of(Kind::GitLab).backend_over(recorded.clone());
            assert_eq!(
                backend.request_auto_merge(&caller(), "acme/api", &PR, SHA),
                Ok(AutoMerge::JoinedTrain),
                "{metadata}"
            );
            spent(&recorded);
        }
    }

    /// Where the merge trains API does not answer (a GitLab without it, or one that will not
    /// add this request), nothing is merged and the request stays open for a person, saying
    /// why: the merge call, which would merge past the train, is never sent instead.
    #[test]
    fn a_train_repo_whose_merge_trains_api_refuses_stays_open_for_a_person() {
        purlis_core::unsteered!();
        let refused = json!({"code": 1, "out": "{\"message\":\"400 Bad request\"}",
                             "err": "glab: 400 Bad request (HTTP 400)", "status": 400});
        for (metadata, flag) in [
            (
                ok(json!({"version": "15.4.0-ee", "enterprise": true})),
                "when_pipeline_succeeds",
            ),
            (
                ok(json!({"version": "19.0.3-ee", "enterprise": true})),
                "auto_merge",
            ),
        ] {
            for answer in [not_found(), refused.clone()] {
                let text = below_19_1(metadata.clone(), flag, answer.clone());
                let recorded = Arc::new(Recorded::parse(&text.to_string()).unwrap());
                let backend = Forge::default_of(Kind::GitLab).backend_over(recorded.clone());
                let got = backend.request_auto_merge(&caller(), "acme/api", &PR, SHA);
                let Ok(AutoMerge::NotQueued(why)) = got else {
                    panic!("{metadata} {answer}: {got:?}");
                };
                assert!(why.contains("merge train"), "{why}");
                spent(&recorded);
            }
        }
    }

    #[test]
    fn a_repo_whose_trains_are_off_is_queued_as_before() {
        purlis_core::unsteered!();
        let text = recording("gitlab", "request_auto_merge.train").replace(
            "\\\"merge_trains_enabled\\\": true",
            "\\\"merge_trains_enabled\\\": false",
        );
        assert!(text.contains("merge_trains_enabled\\\": false"));
        let recorded = Arc::new(Recorded::parse(&text).unwrap());
        let backend = Forge::default_of(Kind::GitLab).backend_over(recorded.clone());
        assert_eq!(
            backend.request_auto_merge(&caller(), "acme/api", &PR, SHA),
            Ok(AutoMerge::Queued)
        );
        spent(&recorded);
    }
}

/// GitLab matches `source_branch` by name in every project, forks included
/// (`doc/api/merge_requests.md`, List project merge requests). A merge request from a fork's
/// branch of the same name is not the branch's.
mod gitlab_forks {
    use super::*;

    fn inline(exchanges: Value) -> (Box<dyn ForgeBackend>, Arc<Recorded>) {
        let text = json!({"source": "GitLab 19.4 REST API docs, merge_requests.md",
                          "exchanges": exchanges});
        let recorded = Arc::new(Recorded::parse(&text.to_string()).unwrap());
        let forge = Forge::default_of(purlis_core::forge::Kind::GitLab);
        (forge.backend_over(recorded.clone()), recorded)
    }

    fn listing(out: Value) -> Value {
        json!([{"call": {"endpoint": {"rest": {"method": null,
                    "path": "projects/acme%2Fapi/merge_requests?state=opened&source_branch=main&per_page=100"}},
                 "fields": []},
                "reply": {"code": 0, "out": out.to_string()}}])
    }

    #[test]
    fn the_status_line_never_shows_a_forks_merge_request() {
        purlis_core::unsteered!();
        let (backend, recorded) = inline(listing(json!([
            {"iid": 99, "source_project_id": 9, "target_project_id": 3},
            {"iid": 12, "source_project_id": 3, "target_project_id": 3}
        ])));
        assert_eq!(
            backend.open_on_branch(&caller(), "acme/api", "main"),
            Ok(Some(Value::from(12)))
        );
        spent(&recorded);
    }

    #[test]
    fn only_forks_merge_requests_on_the_branch_is_no_open_request() {
        purlis_core::unsteered!();
        let (backend, recorded) = inline(listing(json!([
            {"iid": 99, "source_project_id": 9, "target_project_id": 3}
        ])));
        assert_eq!(
            backend.open_on_branch(&caller(), "acme/api", "main"),
            Ok(None)
        );
        spent(&recorded);
    }
}

/// Whether an account can open an issue, in each forge's own fields (`Repos::about`): GitHub's
/// `has_issues` and `permissions.pull`, GitLab's `issues_access_level` and membership.
mod who_can_open_an_issue {
    use super::*;

    fn about_of(kind: purlis_core::forge::Kind, path: &str, out: Value) -> About {
        let text = json!({"source": "GitHub REST 2022-11-28 repos.md; GitLab 19.4 projects.md",
            "exchanges": [{"call": {"endpoint": {"rest": {"method": null, "path": path}},
                                    "fields": []},
                           "reply": {"code": 0, "out": out.to_string()}}]});
        let recorded = Arc::new(Recorded::parse(&text.to_string()).unwrap());
        let backend = Forge::default_of(kind).backend_over(recorded.clone());
        let repo = purlis_core::forge::RepoRecord {
            id: None,
            name: "api".into(),
            path_with_namespace: "acme/api".into(),
            default_branch: None,
            description: String::new(),
            web_url: String::new(),
            ssh_url: String::new(),
            topics: Vec::new(),
            forge: kind,
        };
        let about = backend.about(&caller(), &repo).unwrap();
        spent(&recorded);
        about
    }

    #[test]
    fn github_says_off_when_issues_are_off_and_no_right_when_it_may_not_read() {
        purlis_core::unsteered!();
        let github = |out| about_of(purlis_core::forge::Kind::GitHub, "repos/acme/api", out);
        assert_eq!(
            github(json!({"private": false, "has_issues": false})),
            About {
                visibility: Visibility::Public,
                issues: Issues::Off,
                push_protection: PushProtection::Unknown,
            }
        );
        assert_eq!(
            github(json!({"visibility": "internal", "permissions": {"pull": false}})).issues,
            Issues::NoRight
        );
    }

    #[test]
    fn gitlab_says_no_right_for_members_only_issues_to_a_stranger() {
        purlis_core::unsteered!();
        let gitlab = |out| about_of(purlis_core::forge::Kind::GitLab, "projects/acme%2Fapi", out);
        let stranger = json!({"visibility": "public", "issues_access_level": "private",
                              "permissions": {"project_access": null, "group_access": null}});
        assert_eq!(gitlab(stranger).issues, Issues::NoRight);
        let member = json!({"visibility": "internal", "issues_access_level": "private",
                            "permissions": {"project_access": null,
                                            "group_access": {"access_level": 10}}});
        assert_eq!(
            gitlab(member),
            About {
                visibility: Visibility::Internal,
                issues: Issues::Open,
                push_protection: PushProtection::Unknown,
            }
        );
        let off = json!({"visibility": "private", "issues_access_level": "disabled"});
        assert_eq!(gitlab(off).issues, Issues::Off);
    }

    /// Whether the forge refuses a push that carries a secret (SQ-8), in each forge's own field:
    /// GitHub's `security_and_analysis.secret_scanning_push_protection.status`, GitLab's
    /// `secret_push_protection_enabled` (`pre_receive_secret_detection_enabled` before 18.0).
    /// Both forges show it only to an account that may change it; silence is `Unknown`.
    #[test]
    fn push_protection_is_read_from_each_forges_own_field_and_silence_is_unknown() {
        purlis_core::unsteered!();
        let github =
            |out| about_of(purlis_core::forge::Kind::GitHub, "repos/acme/api", out).push_protection;
        let status = |s: &str| {
            json!({"visibility": "public", "security_and_analysis":
                   {"secret_scanning_push_protection": {"status": s}}})
        };
        assert_eq!(github(status("enabled")), PushProtection::On);
        assert_eq!(github(status("disabled")), PushProtection::Off);
        assert_eq!(github(status("something new")), PushProtection::Unknown);
        assert_eq!(
            github(json!({"visibility": "public"})),
            PushProtection::Unknown
        );

        let gitlab = |out| {
            about_of(purlis_core::forge::Kind::GitLab, "projects/acme%2Fapi", out).push_protection
        };
        let public = |key: &str, on: bool| json!({"visibility": "public", key: on});
        assert_eq!(
            gitlab(public("secret_push_protection_enabled", true)),
            PushProtection::On
        );
        assert_eq!(
            gitlab(public("secret_push_protection_enabled", false)),
            PushProtection::Off
        );
        assert_eq!(
            gitlab(public("pre_receive_secret_detection_enabled", true)),
            PushProtection::On
        );
        assert_eq!(
            gitlab(json!({"visibility": "public"})),
            PushProtection::Unknown
        );
    }
}

/// The network log, for the recorded REST calls of the parity table on both forges (FG-3): each
/// is listed by its template, and no name of the scene (the owner, the repo, a branch, a
/// request's number, a commit) survives into it. A GraphQL call is listed as `graphql`, which
/// holds no name, so it is not read here.
mod network_log {
    use super::*;

    /// Every name the recordings' paths carry, however each forge spells it.
    const SCENE: [&str; 10] = [
        "acme", "api", "web", "main", "charter", "save", "12", "7", SHA, MERGE,
    ];

    /// Each case's recording, then its self-managed override where there is one, then the
    /// forge's own [`SCENES`], so a path only an override or a scene asks is checked too.
    fn paths(kind: &str) -> Vec<String> {
        let scenes = SCENES
            .iter()
            .filter(|(k, _, _)| *k == kind)
            .map(|(_, case, scene)| recording(kind, &format!("{case}.{scene}")));
        CASES
            .iter()
            .flat_map(|case| {
                let own = recordings()
                    .join(kind)
                    .join(format!("{case}.self_managed.json"));
                std::iter::once(recording(kind, case)).chain(own_recording(&own))
            })
            .chain(scenes)
            .flat_map(|text| {
                let file: Value = serde_json::from_str(&text).unwrap();
                file["exchanges"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|e| {
                        e["call"]["endpoint"]["rest"]["path"]
                            .as_str()
                            .map(String::from)
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    #[test]
    fn every_recorded_call_on_both_forges_is_listed_with_no_name_of_the_scene() {
        purlis_core::unsteered!();
        for kind in ["github", "gitlab"] {
            let paths = paths(kind);
            // Exactly the REST calls the recordings, their overrides and the scenes hold, so a
            // recording that lost one, or a reader that skipped some, fails here rather than
            // checking less. GitLab's self-managed support asks each probe once (#802), and its
            // two merge train scenes ask four each (#805). `merge_when_ready` and its scenes
            // read the checks and merge: thirty-three on GitHub, forty-nine on GitLab (qw371:
            // a scene per answer of purlis's own gate). Making a repo asks who the account is,
            // its owners, the create, and the delete (GitHub reads the repo first; #1819). A
            // search is one call (#806). What a failed check found is one call (qw378). Browser
            // proof (#2026): GitHub keeps two pictures in seven calls on a branch that is there
            // and six on a first proof, GitLab in two uploads, and a comment is one call on each.
            let expected = match kind {
                "github" => 73,
                _ => 95,
            };
            assert_eq!(paths.len(), expected, "{kind}: the recorded REST calls");
            for path in paths {
                let listed = purlis_core::netlog::template(&path);
                assert!(
                    !listed.contains('?'),
                    "{kind}: {path} keeps its query: {listed}"
                );
                for segment in listed.split('/') {
                    assert!(
                        !SCENE.iter().any(|name| segment.contains(name)),
                        "{kind}: {path} is listed as {listed}, which names {segment}"
                    );
                }
            }
        }
    }
}

/// GitHub's `owned` for a personal account: the org endpoint answers `404`, and the same listing
/// is asked of the user endpoint (GitHub REST `2022-11-28`, "List repositories for a user").
/// GitLab's counterpart is [`gitlab_user_namespace`].
mod github_personal_account {
    use super::*;

    fn page(path: &str, reply: Value) -> Value {
        json!({"call": {"endpoint": {"rest": {"method": null, "path": path}}, "fields": []},
               "reply": reply})
    }

    #[test]
    fn owned_falls_back_to_the_user_endpoint_on_a_404_and_answers_the_same_shape() {
        purlis_core::unsteered!();
        let repos = json!([{"id": 1, "name": "api", "full_name": "acme/api",
            "default_branch": "main", "description": null,
            "html_url": "https://github.com/acme/api",
            "ssh_url": "git@github.com:acme/api.git", "topics": []}]);
        let text = json!({"source": "GitHub REST 2022-11-28, repos: List organization \
                                 repositories (404 for a user), List repositories for a user",
        "exchanges": [
            page("orgs/acme/repos?per_page=100&page=1",
                 json!({"code": 1, "out": "{\"message\":\"Not Found\",\"status\":\"404\"}",
                        "err": "gh: Not Found (HTTP 404)", "status": 404})),
            page("users/acme/repos?per_page=100&page=1",
                 json!({"code": 0, "out": repos.to_string()})),
        ]});
        let recorded = Arc::new(Recorded::parse(&text.to_string()).unwrap());
        let backend = Forge::default_of(Kind::GitHub).backend_over(recorded.clone());
        let owned = backend.owned(&caller(), &Owner::new("acme")).unwrap();
        assert_eq!(owned.len(), 1);
        assert_eq!(owned[0].path_with_namespace, "acme/api");
        assert_eq!(owned[0].forge, Kind::GitHub);
        spent(&recorded);
    }
}

/// GitLab's `owned` for a user namespace (#803): the group endpoint answers `404 Group Not
/// Found`, and the same listing is asked of the user endpoint (GitLab 19.4 `doc/api/projects.md`,
/// "List all personal projects for a user"), as GitHub falls back from an organisation to a
/// user. A group is still asked first, and only a not-found answer falls back.
mod gitlab_user_namespace {
    use super::*;
    use purlis_core::forge::Failure;

    const GROUP: &str = "groups/solo/projects?per_page=100&page=1&include_subgroups=true\
                         &archived=false&with_shared=false";
    const USER: &str = "users/solo/projects?per_page=100&page=1&archived=false";

    fn page(path: &str, reply: Value) -> Value {
        json!({"call": {"endpoint": {"rest": {"method": null, "path": path}}, "fields": []},
               "reply": reply})
    }

    fn over(exchanges: Value) -> (Box<dyn ForgeBackend>, Arc<Recorded>) {
        let text = json!({"source": "GitLab 19.4 REST API docs, groups.md: List projects (404 \
                                     for a user), projects.md: List all personal projects \
                                     for a user",
                          "exchanges": exchanges});
        let recorded = Arc::new(Recorded::parse(&text.to_string()).unwrap());
        let backend = Forge::default_of(Kind::GitLab).backend_over(recorded.clone());
        (backend, recorded)
    }

    fn personal() -> Value {
        let repos = json!([{"id": 3, "name": "Dots", "path": "dots",
            "path_with_namespace": "solo/dots", "default_branch": "main", "description": null,
            "web_url": "https://gitlab.com/solo/dots",
            "ssh_url_to_repo": "git@gitlab.com:solo/dots.git", "topics": []}]);
        json!({"code": 0, "out": repos.to_string()})
    }

    /// GitLab's answer for a group that does not exist, as `glab api` says it and as the
    /// native transport sees it.
    fn no_such_group() -> [Value; 2] {
        let body = "{\"message\":\"404 Group Not Found\"}";
        [
            json!({"code": 1, "out": body, "err": "glab: 404 Group Not Found (HTTP 404)"}),
            json!({"code": 1, "out": body, "err": "", "status": 404}),
        ]
    }

    #[test]
    fn owned_falls_back_to_the_user_endpoint_on_a_group_404_and_answers_the_same_shape() {
        purlis_core::unsteered!();
        for missing in no_such_group() {
            let (backend, recorded) = over(json!([page(GROUP, missing), page(USER, personal())]));
            let owned = backend.owned(&caller(), &Owner::new("solo")).unwrap();
            assert_eq!(owned.len(), 1);
            assert_eq!(owned[0].name, "dots");
            assert_eq!(owned[0].path_with_namespace, "solo/dots");
            assert_eq!(owned[0].ssh_url, "git@gitlab.com:solo/dots.git");
            assert_eq!(owned[0].forge, Kind::GitLab);
            spent(&recorded);
        }
    }

    #[test]
    fn only_a_not_found_answer_falls_back() {
        purlis_core::unsteered!();
        let forbidden = json!({"code": 1, "out": "{\"message\":\"403 Forbidden\"}", "err": "",
                               "status": 403});
        let (backend, recorded) = over(json!([page(GROUP, forbidden)]));
        let err = backend.owned(&caller(), &Owner::new("solo")).unwrap_err();
        assert_eq!(err.failure(), &Failure::Forbidden, "{err}");
        assert!(err.said().contains("GitLab group 'solo'"), "{err}");
        spent(&recorded);
    }

    #[test]
    fn a_name_that_is_neither_a_group_nor_a_user_fails_in_the_user_endpoints_words() {
        purlis_core::unsteered!();
        let no_user = json!({"code": 1, "out": "{\"message\":\"404 User Not Found\"}",
                             "err": "", "status": 404});
        let [missing, _] = no_such_group();
        let (backend, recorded) = over(json!([page(GROUP, missing), page(USER, no_user)]));
        let err = backend.owned(&caller(), &Owner::new("solo")).unwrap_err();
        assert_eq!(err.failure(), &Failure::NotFound, "{err}");
        assert!(
            err.said()
                .contains("GitLab user 'solo' (no group has that name)")
                && err.said().contains("404 User Not Found"),
            "{err}"
        );
        spent(&recorded);
    }
}

/// GitLab's `owned` for a group lists the group's own projects and its subgroups', never a
/// project another namespace shared into it (#804). `groups/:id/projects` defaults
/// `with_shared` to `true` (GitLab 19.4 `doc/api/groups.md`); GitHub's organisation listing
/// has no such case, so purlis asks with `with_shared=false`. The recording answers only that
/// request, so a listing asked without it fails here.
mod gitlab_shared_projects {
    use super::*;

    #[test]
    fn a_project_shared_into_the_group_is_not_discovered_as_the_groups() {
        purlis_core::unsteered!();
        let own = json!([{"id": 1, "name": "Api", "path": "api", "path_with_namespace": "acme/api",
            "default_branch": "main", "description": null, "web_url": "https://gitlab.com/acme/api",
            "ssh_url_to_repo": "git@gitlab.com:acme/api.git", "topics": []}]);
        let text = json!({"source": "GitLab 19.4 REST API docs, groups.md: List projects \
                                     (with_shared defaults to true)",
            "exchanges": [{"call": {"endpoint": {"rest": {"method": null,
                "path": "groups/acme/projects?per_page=100&page=1&include_subgroups=true\
                         &archived=false&with_shared=false"}}, "fields": []},
                "reply": {"code": 0, "out": own.to_string()}}]});
        let recorded = Arc::new(Recorded::parse(&text.to_string()).unwrap());
        let backend = Forge::default_of(Kind::GitLab).backend_over(recorded.clone());
        let owned = backend.owned(&caller(), &Owner::new("acme")).unwrap();
        let paths: Vec<&str> = owned
            .iter()
            .map(|r| r.path_with_namespace.as_str())
            .collect();
        assert_eq!(paths, ["acme/api"], "only the group's own projects");
        spent(&recorded);
    }
}

/// FW-2a's and FW-2b's acceptance: the contract passes against the native client with no `gh`
/// or `glab` on `PATH`, on github.com, gitlab.com, a GitHub Enterprise Server and a self-managed
/// GitLab (FG-3). Every native case
/// runs again in a child of this binary whose environment is emptied and whose `PATH` is one
/// empty directory.
#[test]
fn the_native_contract_passes_with_no_forge_cli_on_path() {
    purlis_core::unsteered!();
    let empty = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let out = purlis_core::forklock::output(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "github_native::",
                "gitlab_native::",
                "github_self_managed::",
                "gitlab_self_managed::",
                "--test-threads=2",
            ])
            .env_clear()
            .env("PATH", empty.path())
            .env("HOME", home.path()),
    )
    .unwrap();
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "the native contract failed with no CLI on PATH:\n{said}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        said.contains(&format!("{} passed", 4 * CASES.len())),
        "the child did not run every native case:\n{said}"
    );
}

/// The live run reads its scene from each forge's own fields (`live.rs`): the two repos' ids, the
/// request open from the save branch and the one that landed. Here against answers shaped as
/// each forge's documentation shows them (GitHub REST `2022-11-28` repos and pulls; GitLab 19.4
/// `projects.md` and `merge_requests.md`).
mod the_live_scene {
    use super::*;

    fn answering(answers: Vec<(String, Value)>) -> impl Fn(&str) -> Result<Value, String> {
        move |path: &str| {
            answers
                .iter()
                .find(|(p, _)| p == path)
                .map(|(_, v)| v.clone())
                .ok_or_else(|| format!("nothing answers {path}"))
        }
    }

    fn github(landed: Value) -> Vec<(String, Value)> {
        vec![
            ("repos/fixture/api".into(), json!({"id": 101, "full_name": "fixture/api"})),
            ("repos/fixture/web".into(), json!({"id": 102, "full_name": "fixture/web"})),
            (
                "repos/fixture/api/pulls?state=open&head=fixture:charter%2Fsave&base=main&per_page=1"
                    .into(),
                json!([{"number": 2, "state": "open", "head": {"ref": "charter/save", "sha": SHA}}]),
            ),
            (
                "repos/fixture/api/pulls?state=closed&head=fixture:charter%2Flanded&per_page=1"
                    .into(),
                landed,
            ),
        ]
    }

    #[test]
    fn github_names_the_open_request_its_head_and_the_merge_commit_of_the_one_that_landed() {
        purlis_core::unsteered!();
        let landed = json!([{"number": 1, "state": "closed",
                             "merged_at": "2026-10-04T00:00:00Z", "merge_commit_sha": MERGE}]);
        let get = answering(github(landed));
        let scene = live::discover(Kind::GitHub, "github.com", "fixture", &get).unwrap();
        assert_eq!(
            (
                scene.api_id.as_str(),
                scene.web_id.as_str(),
                scene.tree_id.as_str()
            ),
            ("101", "102", "101")
        );
        assert_eq!((scene.open, scene.head.as_str()), (2, SHA));
        assert_eq!((scene.merged, scene.merge.as_str()), (1, MERGE));
        assert_eq!(scene.opened_from, live::OPENED_FROM);
        assert_eq!(
            (scene.opened, scene.created.clone(), scene.queue),
            (None, None, false)
        );
        assert_eq!(
            scene.request_url(2),
            "https://github.com/fixture/api/pull/2"
        );
    }

    #[test]
    fn a_github_request_closed_without_merging_is_no_landed_request() {
        purlis_core::unsteered!();
        let closed = json!([{"number": 1, "state": "closed", "merged_at": null,
                             "merge_commit_sha": MERGE}]);
        let get = answering(github(closed));
        let refused = live::discover(Kind::GitHub, "github.com", "fixture", &get).unwrap_err();
        assert!(refused.contains("closed, not merged"), "{refused}");
    }

    #[test]
    fn gitlab_reads_iids_and_escapes_the_repo_as_one_segment() {
        purlis_core::unsteered!();
        let get = answering(vec![
            ("projects/fixture%2Fapi".into(), json!({"id": 201})),
            ("projects/fixture%2Fweb".into(), json!({"id": 202})),
            (
                "projects/fixture%2Fapi/merge_requests?state=opened&source_branch=charter%2Fsave&target_branch=main&per_page=1"
                    .into(),
                json!([{"id": 9001, "iid": 2, "sha": SHA}]),
            ),
            (
                "projects/fixture%2Fapi/merge_requests?state=merged&source_branch=charter%2Flanded&per_page=1"
                    .into(),
                json!([{"id": 9000, "iid": 1, "merge_commit_sha": MERGE}]),
            ),
        ]);
        let scene = live::discover(Kind::GitLab, "gitlab.com", "fixture", &get).unwrap();
        assert_eq!(
            (scene.api_id.as_str(), scene.web_id.as_str()),
            ("201", "202")
        );
        assert_eq!((scene.open, scene.head.as_str()), (2, SHA));
        assert_eq!((scene.merged, scene.merge.as_str()), (1, MERGE));
        assert_eq!(
            scene.issue_url(5),
            "https://gitlab.com/fixture/api/-/issues/5"
        );
    }

    /// The issues `create` leaves behind are found by the harness's own listing, by the label
    /// `create` gives them and the case's title, and closed by the numbers that listing names: never a pull request, never an issue
    /// with another title.
    #[test]
    fn the_sweep_closes_only_open_issues_with_the_cases_title() {
        purlis_core::unsteered!();
        let closed = std::cell::RefCell::new(Vec::new());
        let list = |path: &str| -> Result<Value, String> {
            assert_eq!(
                path, "repos/fixture/api/issues?state=open&labels=ws%3Aalpha&per_page=100",
                "by the label `create` puts on it"
            );
            Ok(json!([
                {"number": 7, "title": live::ISSUE_TITLE},
                {"number": 8, "title": "Something else"},
                {"number": 9, "title": live::ISSUE_TITLE, "pull_request": {"url": "x"}},
                {"number": 10, "title": live::ISSUE_TITLE},
            ]))
        };
        let close = |number: u64| -> Result<(), String> {
            closed.borrow_mut().push(number);
            Ok(())
        };
        let swept = live::sweep(Kind::GitHub, "repos/fixture/api", &list, &close).unwrap();
        assert_eq!(swept, [7, 10]);
        assert_eq!(*closed.borrow(), [7, 10]);

        let gitlab = |path: &str| -> Result<Value, String> {
            assert_eq!(
                path,
                "projects/fixture%2Fapi/issues?state=opened&labels=charter%3A%3Aws%3A%3Aalpha&per_page=100"
            );
            Ok(json!([{"id": 5001, "iid": 3, "title": live::ISSUE_TITLE}]))
        };
        let swept =
            live::sweep(Kind::GitLab, "projects/fixture%2Fapi", &gitlab, &|_| Ok(())).unwrap();
        assert_eq!(swept, [3], "by its iid, not its id");
    }

    #[test]
    fn a_fixture_with_no_open_request_says_which_one_it_lacks() {
        purlis_core::unsteered!();
        let mut answers = github(json!([]));
        answers[2].1 = json!([]);
        let refused =
            live::discover(Kind::GitHub, "github.com", "fixture", &answering(answers)).unwrap_err();
        assert!(
            refused.contains("request open from charter/save into main"),
            "{refused}"
        );
    }
}

/// A save in `pr-merge` mode: purlis merges its request itself, behind its own reading of every
/// check at the commit the save pushed (ADR 0051, amended 2026-10-13). Never on red, required
/// or not; never before its checks had the chance to start; and with no check at all, only
/// when nothing the repository shows says one will run. Each scene runs on the recorded and
/// the native transport, and none records a call asking the forge to auto-merge: one sent
/// would find nothing, and fail the case.
mod merge_when_ready_scenes {
    use super::*;

    const PR: Pr = Pr {
        number: 12,
        url: String::new(),
    };
    /// The request each scene's save may merge: at [`SHA`], from its save branch, into `main`.
    const AT: Pinned = Pinned {
        sha: SHA,
        branch: "purlis/save/test",
        base: "main",
    };
    /// The head has been on the forge long enough for its checks to have started.
    const SETTLED: Gate = Gate::Checks { settled: true };
    /// The head was pushed a moment ago.
    const FRESH: Gate = Gate::Checks { settled: false };

    /// `kind`'s backend over the scene `<case>.<scene>.json`, the caller to ask as, and the
    /// check that every recorded exchange was asked for.
    fn scene(kind: &str, name: &str, how: How) -> (Box<dyn ForgeBackend>, Caller, Box<dyn Fn()>) {
        let text = recording(kind, &format!("merge_when_ready.{name}"));
        match how {
            How::Recorded => {
                let recorded = Arc::new(Recorded::parse(&text).unwrap());
                let check = recorded.clone();
                (
                    Forge::default_of(Kind::parse(kind).unwrap()).backend_over(recorded),
                    caller(),
                    Box::new(move || check.spent()),
                )
            }
            _ => {
                // A native case asks as the account it signed in with.
                let over = native::over(kind, default_host(kind), &text);
                (over.backend, over.caller, over.check)
            }
        }
    }

    const HOWS: [How; 2] = [How::Recorded, How::Native];
    const BOTH: [&str; 2] = ["github", "gitlab"];

    /// The names of what a red answer carries.
    fn names(failed: &[FailedCheck]) -> Vec<&str> {
        failed.iter().map(|check| check.name.as_str()).collect()
    }

    /// The fault this lane fixes: straight after the push no check has registered, and purlis
    /// merged; the check then failed on a request already merged. Now the first look waits,
    /// and the next one finds the failure and leaves the request open, named.
    #[test]
    fn no_check_straight_after_the_push_waits_and_a_check_that_then_fails_is_never_merged() {
        purlis_core::unsteered!();
        // No merge call is recorded: one sent would find nothing, and fail the case.
        for kind in BOTH {
            for how in HOWS {
                let (backend, me, check) = scene(kind, "late_failure", how);
                let first = backend.merge_when_ready(&me, "acme/api", &PR, &AT, FRESH);
                assert!(
                    matches!(&first, Ok(Ready::Waiting(why)) if why.contains("not had time to start")),
                    "{kind}: {first:?}"
                );
                let later = backend.merge_when_ready(&me, "acme/api", &PR, &AT, SETTLED);
                let Ok(Ready::Red(failed)) = &later else {
                    panic!("{kind}: {later:?}");
                };
                assert_eq!(failed.len(), 1, "{kind}: {failed:?}");
                check();
            }
        }
    }

    /// No check, and no sign of CI anywhere: it still waits while the head is new, and is
    /// merged once the head is settled and nothing the repository shows says a check will run.
    #[test]
    fn a_request_with_no_check_and_no_sign_of_ci_is_merged_once_its_head_is_settled() {
        purlis_core::unsteered!();
        for kind in BOTH {
            for how in HOWS {
                let (backend, me, check) = scene(kind, "no_checks", how);
                let first = backend.merge_when_ready(&me, "acme/api", &PR, &AT, FRESH);
                assert!(matches!(first, Ok(Ready::Waiting(_))), "{kind}: {first:?}");
                assert_eq!(
                    backend.merge_when_ready(&me, "acme/api", &PR, &AT, SETTLED),
                    Ok(Ready::Merged),
                    "{kind}"
                );
                check();
            }
        }
    }

    /// No check at a settled head, in a repository whose other requests have them: checks are
    /// expected, so it waits, and nothing is merged.
    #[test]
    fn a_request_with_no_check_waits_where_the_repository_runs_checks_on_requests() {
        purlis_core::unsteered!();
        let scenes = [
            ("github", "expected", "#11, another pull request"),
            ("github", "expected_own", "an earlier commit of #12"),
            ("gitlab", "expected", "ran a pipeline for feature/x"),
        ];
        for (kind, name, sign) in scenes {
            for how in HOWS {
                let (backend, me, check) = scene(kind, name, how);
                let got = backend.merge_when_ready(&me, "acme/api", &PR, &AT, SETTLED);
                assert!(
                    matches!(&got, Ok(Ready::Stalled(why)) if why.contains(sign)),
                    "{kind} {name}: {got:?}"
                );
                check();
            }
        }
    }

    /// GitLab skipped the pipeline at the head (`[ci skip]`): its own word that none will run,
    /// so nothing more is asked before it merges.
    #[test]
    fn a_gitlab_request_whose_pipeline_was_skipped_is_merged_with_no_more_asked() {
        purlis_core::unsteered!();
        for how in HOWS {
            let (backend, me, check) = scene("gitlab", "skipped", how);
            assert_eq!(
                backend.merge_when_ready(&me, "acme/api", &PR, &AT, SETTLED),
                Ok(Ready::Merged)
            );
            check();
        }
    }

    #[test]
    fn a_request_whose_checks_are_running_waits_and_is_merged_once_they_pass() {
        purlis_core::unsteered!();
        for kind in BOTH {
            for how in HOWS {
                let (backend, me, check) = scene(kind, "pending", how);
                let first = backend.merge_when_ready(&me, "acme/api", &PR, &AT, SETTLED);
                assert!(matches!(first, Ok(Ready::Waiting(_))), "{kind}: {first:?}");
                // The next look, once the checks passed.
                assert_eq!(
                    backend.merge_when_ready(&me, "acme/api", &PR, &AT, SETTLED),
                    Ok(Ready::Merged),
                    "{kind}"
                );
                check();
            }
        }
    }

    #[test]
    fn a_request_with_a_failing_check_stays_open_and_names_the_check_with_its_page() {
        purlis_core::unsteered!();
        // No merge call is recorded: one sent would find nothing, and fail the case.
        let named = [
            (
                "github",
                "lint",
                "https://github.com/acme/api/actions/runs/9/job/5",
            ),
            (
                "gitlab",
                "pipeline #77 (failed)",
                "https://gitlab.com/acme/api/-/pipelines/77",
            ),
        ];
        for (kind, name, page) in named {
            for how in HOWS {
                let (backend, me, check) = scene(kind, "failing", how);
                // Red whether or not the head is settled: a failure is a failure at once.
                let got = backend.merge_when_ready(&me, "acme/api", &PR, &AT, FRESH);
                let Ok(Ready::Red(failed)) = &got else {
                    panic!("{kind}: {got:?}");
                };
                // The passing and the skipped check beside it are not named.
                assert_eq!(names(failed), [name], "{kind}");
                assert_eq!(failed[0].url.as_deref(), Some(page), "{kind}");
                check();
            }
        }
    }

    /// Cancelled, timed out and waiting on a person did not pass (fail closed); a stale run is
    /// not counted at all.
    #[test]
    fn a_cancelled_or_timed_out_check_did_not_pass_and_the_request_is_red() {
        purlis_core::unsteered!();
        for how in HOWS {
            let (backend, me, check) = scene("github", "cancelled", how);
            let got = backend.merge_when_ready(&me, "acme/api", &PR, &AT, SETTLED);
            let Ok(Ready::Red(failed)) = &got else {
                panic!("{got:?}");
            };
            assert_eq!(names(failed), ["build", "e2e", "deploy"]);
            check();

            let (backend, me, check) = scene("gitlab", "cancelled", how);
            let got = backend.merge_when_ready(&me, "acme/api", &PR, &AT, SETTLED);
            let Ok(Ready::Red(failed)) = &got else {
                panic!("{got:?}");
            };
            assert_eq!(names(failed), ["pipeline #77 (canceled)"]);
            check();
        }
    }

    /// Neutral and skipped are checks that ran and had nothing against it: they pass.
    #[test]
    fn a_github_request_whose_checks_are_neutral_or_skipped_is_merged() {
        purlis_core::unsteered!();
        for how in HOWS {
            let (backend, me, check) = scene("github", "passed_quietly", how);
            assert_eq!(
                backend.merge_when_ready(&me, "acme/api", &PR, &AT, SETTLED),
                Ok(Ready::Merged)
            );
            check();
        }
    }

    /// A failing check the branch does not require: the forge would merge past it (GitHub's
    /// auto-merge waits for required checks only; GitLab calls the request `mergeable`).
    /// purlis's own gate says red, whatever the repository's settings are. Only the person's
    /// own "merge it as it stands" merges it, and that reads no check at all.
    #[test]
    fn a_failing_check_that_is_not_required_is_red_and_only_the_person_merges_past_it() {
        purlis_core::unsteered!();
        for (kind, name) in [("github", "coverage"), ("gitlab", "pipeline #77 (failed)")] {
            for how in HOWS {
                let (backend, me, check) = scene(kind, "unrequired", how);
                let got = backend.merge_when_ready(&me, "acme/api", &PR, &AT, SETTLED);
                let Ok(Ready::Red(failed)) = &got else {
                    panic!("{kind}: {got:?}");
                };
                assert_eq!(names(failed), [name], "{kind}");
                assert_eq!(
                    backend.merge_when_ready(&me, "acme/api", &PR, &AT, Gate::Person),
                    Ok(Ready::Merged),
                    "{kind}"
                );
                check();
            }
        }
    }

    /// A branch that lands through a merge queue or a merge train is never merged directly:
    /// once every check passed, the request is put in the queue at the pushed commit (#805).
    #[test]
    fn a_request_whose_branch_lands_through_a_queue_is_put_in_it_once_its_checks_pass() {
        purlis_core::unsteered!();
        // No merge call is recorded: one sent would find nothing, and fail the case.
        for (kind, word) in [("github", "merge queue"), ("gitlab", "merge train")] {
            for how in HOWS {
                let (backend, me, check) = scene(kind, "queue", how);
                let got = backend.merge_when_ready(&me, "acme/api", &PR, &AT, SETTLED);
                assert!(
                    matches!(&got, Ok(Ready::Queued(said)) if said.contains(word)),
                    "{kind}: {got:?}"
                );
                check();
            }
        }
    }

    /// A GitLab train repo whose merge request has no passing pipeline yet is not put on the
    /// train, and not merged past it either.
    #[test]
    fn a_gitlab_train_repo_is_never_merged_past_its_train() {
        purlis_core::unsteered!();
        // Neither a merge call nor a train call is recorded.
        let get = |path: &str, out: Value| {
            json!({"call": {"endpoint": {"rest": {"method": null, "path": path}}, "fields": []},
                   "reply": {"code": 0, "out": out.to_string()}})
        };
        let text = json!({"source": "GitLab 19.4 REST API docs, projects.md (merge_trains_enabled)",
                          "exchanges": [
            get("projects/acme%2Fapi", json!({"squash_option": "default_off",
                                              "merge_trains_enabled": true})),
            get("projects/acme%2Fapi/merge_requests/12",
                gl_mr("ci_still_running")),
            get("projects/acme%2Fapi/merge_requests/12/pipelines?per_page=100",
                json!([{"id": 77, "sha": SHA, "status": "running"}])),
        ]});
        let recorded = Arc::new(Recorded::parse(&text.to_string()).unwrap());
        let backend = Forge::default_of(Kind::GitLab).backend_over(recorded.clone());
        let got = backend.merge_when_ready(&caller(), "acme/api", &PR, &AT, SETTLED);
        assert!(matches!(got, Ok(Ready::Waiting(_))), "{got:?}");
        spent(&recorded);
    }

    // ------------------------------------------------------------------------------------ //
    // The review's cases (qw371, round 2), each over an inline recording.                     //
    // ------------------------------------------------------------------------------------ //

    fn rest(method: Option<&str>, path: &str, fields: Value, out: Value) -> Value {
        json!({"call": {"endpoint": {"rest": {"method": method, "path": path}}, "fields": fields},
               "reply": {"code": 0, "out": out.to_string()}})
    }

    fn get(path: &str, out: Value) -> Value {
        rest(None, path, json!([]), out)
    }

    fn over(kind: Kind, exchanges: Vec<Value>) -> (Box<dyn ForgeBackend>, Arc<Recorded>) {
        let text = json!({"source": "inline, after the forge's API docs", "exchanges": exchanges});
        let recorded = Arc::new(Recorded::parse(&text.to_string()).unwrap());
        (
            Forge::default_of(kind).backend_over(recorded.clone()),
            recorded,
        )
    }

    const READY: &str = include_str!("../src/forge/github/queries/merge_ready.graphql");

    /// GitHub's `MergeReady` answer for #12 at [`SHA`]: purlis's own request from its save
    /// branch into `main`, last touched long ago, in merge state `state`.
    fn gh_ready(state: &str) -> Value {
        json!({"call": {"endpoint": "graphql", "fields": [
                    {"text": ["query", READY]}, {"text": ["owner", "acme"]},
                    {"text": ["name", "api"]}, {"typed": ["number", "12"]}]},
               "reply": {"code": 0, "out": json!({"data": {"repository": {
                    "rebaseMergeAllowed": true, "mergeCommitAllowed": true,
                    "squashMergeAllowed": true,
                    "pullRequest": {"state": "OPEN", "isDraft": false, "headRefOid": SHA,
                        "mergeStateStatus": state, "reviewDecision": null,
                        "isMergeQueueEnabled": false, "baseRefName": "main",
                        "headRefName": "purlis/save/test", "isCrossRepository": false,
                        "updatedAt": "2026-01-01T00:00:00Z", "body": "",
                        "commits": {"nodes": [{"commit": {"oid": SHA,
                                                          "statusCheckRollup": null}}]}}}}})
                    .to_string()}})
    }

    fn gh_runs(page: &str, total: u64, runs: Vec<Value>) -> Value {
        get(
            &format!("repos/acme/api/commits/{SHA}/check-runs?per_page=100{page}"),
            json!({"total_count": total, "check_runs": runs}),
        )
    }

    fn gh_statuses(statuses: Value) -> Value {
        let total = statuses.as_array().map_or(0, Vec::len);
        get(
            &format!("repos/acme/api/commits/{SHA}/status?per_page=100"),
            json!({"state": "success", "total_count": total, "statuses": statuses}),
        )
    }

    fn passing(n: usize) -> Value {
        json!({"id": n, "name": format!("job {n}"), "status": "completed",
               "conclusion": "success"})
    }

    /// Review N1: a bot's green status seconds after the push, before the workflow's own runs
    /// exist, is not every check. It waits, and no merge call is recorded.
    #[test]
    fn one_passed_check_at_a_head_just_pushed_waits_and_nothing_is_merged() {
        purlis_core::unsteered!();
        let (backend, recorded) = over(
            Kind::GitHub,
            vec![
                gh_ready("CLEAN"),
                gh_runs("", 0, vec![]),
                gh_statuses(json!([{"context": "dco", "state": "success"}])),
            ],
        );
        let got = backend.merge_when_ready(&caller(), "acme/api", &PR, &AT, FRESH);
        assert!(
            matches!(&got, Ok(Ready::Waiting(why)) if why.contains("may not have started")),
            "{got:?}"
        );
        spent(&recorded);
    }

    /// Review N3: more check runs than one page holds are all read, so a failed one on the
    /// second page is seen, and the request is red rather than waiting for ever.
    #[test]
    fn more_checks_than_one_page_holds_are_all_read() {
        purlis_core::unsteered!();
        let mut second: Vec<Value> = (100..149).map(passing).collect();
        second.push(json!({"id": 149, "name": "late", "status": "completed",
                           "conclusion": "failure"}));
        let (backend, recorded) = over(
            Kind::GitHub,
            vec![
                gh_ready("UNSTABLE"),
                gh_runs("", 150, (0..100).map(passing).collect()),
                gh_runs("&page=2", 150, second),
                gh_statuses(json!([])),
            ],
        );
        let got = backend.merge_when_ready(&caller(), "acme/api", &PR, &AT, SETTLED);
        let Ok(Ready::Red(failed)) = &got else {
            panic!("{got:?}");
        };
        assert_eq!(names(failed), ["late"]);
        spent(&recorded);
    }

    /// More checks than purlis reads at all: it waits, in words that say so, and reads no
    /// further page than its bound.
    #[test]
    fn too_many_checks_to_read_waits_and_says_so() {
        purlis_core::unsteered!();
        let (backend, recorded) = over(
            Kind::GitHub,
            vec![
                gh_ready("CLEAN"),
                gh_runs("", 5000, (0..100).map(passing).collect()),
            ],
        );
        let got = backend.merge_when_ready(&caller(), "acme/api", &PR, &AT, SETTLED);
        assert!(
            matches!(&got, Ok(Ready::Stalled(why)) if why.contains("too many checks to read")),
            "{got:?}"
        );
        spent(&recorded);
    }

    /// Every check passed, and GitHub blocks the request with no review asked for: a check its
    /// branch requires has not reported. That waits; it is not "cannot merge".
    #[test]
    fn a_required_check_that_has_not_reported_waits_and_is_not_called_unmergeable() {
        purlis_core::unsteered!();
        let (backend, recorded) = over(
            Kind::GitHub,
            vec![
                gh_ready("BLOCKED"),
                gh_runs("", 1, vec![passing(1)]),
                gh_statuses(json!([])),
            ],
        );
        let got = backend.merge_when_ready(&caller(), "acme/api", &PR, &AT, SETTLED);
        assert!(
            matches!(&got, Ok(Ready::Stalled(why)) if why.contains("requires has not reported")),
            "{got:?}"
        );
        spent(&recorded);

        // GitLab's word for the same: a pipeline must pass, and none has run.
        let (backend, recorded) = over(
            Kind::GitLab,
            vec![
                get(
                    "projects/acme%2Fapi",
                    json!({"squash_option": "default_off"}),
                ),
                get(
                    "projects/acme%2Fapi/merge_requests/12",
                    gl_mr("ci_must_pass"),
                ),
                get(
                    "projects/acme%2Fapi/merge_requests/12/pipelines?per_page=100",
                    json!([]),
                ),
                get("projects/acme%2Fapi/pipelines?per_page=20", json!([])),
            ],
        );
        let got = backend.merge_when_ready(&caller(), "acme/api", &PR, &AT, SETTLED);
        assert!(
            matches!(&got, Ok(Ready::Stalled(why)) if why.contains("requires a pipeline to pass")),
            "{got:?}"
        );
        spent(&recorded);
    }

    /// GitHub's `MergeReady` answer with one field of the pull request replaced.
    fn gh_ready_with(key: &str, value: Value) -> Value {
        let mut exchange = gh_ready("CLEAN");
        let mut answer: Value =
            serde_json::from_str(exchange["reply"]["out"].as_str().unwrap()).unwrap();
        answer["data"]["repository"]["pullRequest"][key] = value;
        exchange["reply"]["out"] = json!(answer.to_string());
        exchange
    }

    /// Review N8: which request to merge is kept in a file on the machine, and the forge is
    /// the truth. A request that is not purlis's own (a person's branch with no marker, or one
    /// from a fork) is refused before any check is read, for the person's merge too.
    #[test]
    fn a_request_purlis_did_not_open_is_never_merged_whatever_the_machine_kept() {
        purlis_core::unsteered!();
        let theirs = [
            gh_ready_with("headRefName", json!("feature/their-own")),
            gh_ready_with("isCrossRepository", json!(true)),
        ];
        for ready in theirs {
            for gate in [SETTLED, Gate::Person] {
                let (backend, recorded) = over(Kind::GitHub, vec![ready.clone()]);
                let got = backend.merge_when_ready(&caller(), "acme/api", &PR, &AT, gate);
                assert!(
                    matches!(&got, Ok(Ready::Refused(why)) if why.contains("not this save's request")),
                    "{got:?}"
                );
                spent(&recorded);
            }
        }
        // A person's branch whose request carries purlis's marker is purlis's own (a repo's
        // save from a feature branch).
        let mut marked = gh_ready_with("headRefName", json!("feature/x"));
        let mut answer: Value =
            serde_json::from_str(marked["reply"]["out"].as_str().unwrap()).unwrap();
        answer["data"]["repository"]["pullRequest"]["body"] = json!(format!(
            "Saved by purlis.\n\n{}",
            purlis_core::forge::pr::MARKER
        ));
        marked["reply"]["out"] = json!(answer.to_string());
        let (backend, recorded) = over(
            Kind::GitHub,
            vec![
                marked,
                gh_runs("", 1, vec![passing(1)]),
                gh_statuses(json!([])),
            ],
        );
        let from_a_feature_branch = Pinned {
            branch: "feature/x",
            ..AT
        };
        let got =
            backend.merge_when_ready(&caller(), "acme/api", &PR, &from_a_feature_branch, FRESH);
        assert!(matches!(got, Ok(Ready::Waiting(_))), "{got:?}");
        spent(&recorded);

        // The same branch into the same target, and not opened by purlis (no prefix, no
        // marker): a person's own request, which a save leaves as it is, is theirs to merge.
        for gate in [SETTLED, Gate::Person] {
            let (backend, recorded) = over(
                Kind::GitHub,
                vec![gh_ready_with("headRefName", json!("feature/x"))],
            );
            let got =
                backend.merge_when_ready(&caller(), "acme/api", &PR, &from_a_feature_branch, gate);
            assert!(
                matches!(&got, Ok(Ready::Refused(why)) if why.contains("not a request purlis opened")),
                "{got:?}"
            );
            spent(&recorded);
        }

        // GitLab: a merge request from another project is not purlis's own.
        let mut fork = gl_mr("mergeable");
        fork["source_project_id"] = json!(7);
        let (backend, recorded) = over(
            Kind::GitLab,
            vec![
                get(
                    "projects/acme%2Fapi",
                    json!({"squash_option": "default_off"}),
                ),
                get("projects/acme%2Fapi/merge_requests/12", fork),
            ],
        );
        let got = backend.merge_when_ready(&caller(), "acme/api", &PR, &AT, SETTLED);
        assert!(
            matches!(&got, Ok(Ready::Refused(why)) if why.contains("not this save's request")),
            "{got:?}"
        );
        spent(&recorded);
    }

    /// Review round 2, R1 and R2: the request merged is **this save's own, into its own
    /// target**. The forge's base must be the configured target, and its head branch must be
    /// the branch this save pushes — not merely a branch purlis names or a body with its
    /// marker: another machine's save request, or a `purlis change` member's, is not this
    /// save's. Refused before any check is read, and for the person's merge too.
    #[test]
    fn only_this_saves_own_request_into_its_own_target_is_merged() {
        purlis_core::unsteered!();
        let marker = purlis_core::forge::pr::MARKER;
        let not_this_saves = [
            // R1: purlis's own branch, into another base.
            (
                gh_ready_with("baseRefName", json!("release")),
                "goes into release",
            ),
            // R2: other requests purlis opened in the same repository.
            (
                gh_ready_with("headRefName", json!("purlis/change/api-2")),
                "from purlis/change/api-2",
            ),
            (
                gh_ready_with("headRefName", json!("purlis/save/another-machine")),
                "from purlis/save/another-machine",
            ),
        ];
        for (ready, said) in not_this_saves {
            for gate in [SETTLED, Gate::Person] {
                let (backend, recorded) = over(Kind::GitHub, vec![ready.clone()]);
                let got = backend.merge_when_ready(&caller(), "acme/api", &PR, &AT, gate);
                assert!(
                    matches!(&got, Ok(Ready::Refused(why)) if why.contains(said)),
                    "{said}: {got:?}"
                );
                spent(&recorded);
            }
        }
        // A marker in the body does not make another branch's request this save's.
        let mut marked = gh_ready_with("headRefName", json!("feature/elsewhere"));
        let mut answer: Value =
            serde_json::from_str(marked["reply"]["out"].as_str().unwrap()).unwrap();
        answer["data"]["repository"]["pullRequest"]["body"] = json!(marker);
        marked["reply"]["out"] = json!(answer.to_string());
        let (backend, recorded) = over(Kind::GitHub, vec![marked]);
        let got = backend.merge_when_ready(&caller(), "acme/api", &PR, &AT, SETTLED);
        assert!(matches!(got, Ok(Ready::Refused(_))), "{got:?}");
        spent(&recorded);

        // GitLab, the same two rules.
        let mut other_base = gl_mr("mergeable");
        other_base["target_branch"] = json!("release");
        let mut other_head = gl_mr("mergeable");
        other_head["source_branch"] = json!("purlis/change/api-2");
        for (mr, said) in [
            (other_base, "goes into release"),
            (other_head, "from purlis/change/api-2"),
        ] {
            for gate in [SETTLED, Gate::Person] {
                let (backend, recorded) = over(
                    Kind::GitLab,
                    vec![
                        get(
                            "projects/acme%2Fapi",
                            json!({"squash_option": "default_off"}),
                        ),
                        get("projects/acme%2Fapi/merge_requests/12", mr.clone()),
                    ],
                );
                let got = backend.merge_when_ready(&caller(), "acme/api", &PR, &AT, gate);
                assert!(
                    matches!(&got, Ok(Ready::Refused(why)) if why.contains(said)),
                    "{said}: {got:?}"
                );
                spent(&recorded);
            }
        }
    }

    /// Review N4: the machine saying "settled" is not enough. The forge says the request was
    /// touched a moment ago (a push touches it), so a head with no check waits, and whether
    /// the repository runs checks is not even asked. A time the forge does not give settles
    /// nothing either.
    #[test]
    fn a_head_the_forge_says_was_touched_a_moment_ago_is_not_settled_whatever_the_machine_says() {
        purlis_core::unsteered!();
        let now = chrono::Utc::now().to_rfc3339();
        for touched in [json!(now), Value::Null, json!("yesterday")] {
            let (backend, recorded) = over(
                Kind::GitHub,
                vec![
                    gh_ready_with("updatedAt", touched.clone()),
                    gh_runs("", 0, vec![]),
                    gh_statuses(json!([])),
                ],
            );
            let got = backend.merge_when_ready(&caller(), "acme/api", &PR, &AT, SETTLED);
            assert!(
                matches!(&got, Ok(Ready::Waiting(why)) if why.contains("not had time to start")),
                "{touched}: {got:?}"
            );
            spent(&recorded);
        }
        let mut fresh = gl_mr("mergeable");
        fresh["updated_at"] = json!(now);
        let (backend, recorded) = over(
            Kind::GitLab,
            vec![
                get(
                    "projects/acme%2Fapi",
                    json!({"squash_option": "default_off"}),
                ),
                get("projects/acme%2Fapi/merge_requests/12", fresh),
                get(
                    "projects/acme%2Fapi/merge_requests/12/pipelines?per_page=100",
                    json!([]),
                ),
            ],
        );
        let got = backend.merge_when_ready(&caller(), "acme/api", &PR, &AT, SETTLED);
        assert!(matches!(got, Ok(Ready::Waiting(_))), "{got:?}");
        spent(&recorded);
    }

    /// Review N2 at the seam: a failed branch pipeline under a newer green merge request
    /// pipeline at the same commit is red, and nothing is merged.
    #[test]
    fn two_gitlab_pipelines_at_one_commit_are_both_judged() {
        purlis_core::unsteered!();
        let merge = "projects/acme%2Fapi/merge_requests/12";
        let (backend, recorded) = over(
            Kind::GitLab,
            vec![
                get(
                    "projects/acme%2Fapi",
                    json!({"squash_option": "default_off"}),
                ),
                get(merge, gl_mr("mergeable")),
                get(
                    &format!("{merge}/pipelines?per_page=100"),
                    json!([
                        {"id": 78, "sha": SHA, "source": "merge_request_event",
                         "ref": "refs/merge-requests/12/head", "status": "success"},
                        {"id": 77, "sha": SHA, "source": "push", "ref": "purlis/save/test",
                         "status": "failed"},
                    ]),
                ),
            ],
        );
        let got = backend.merge_when_ready(&caller(), "acme/api", &PR, &AT, SETTLED);
        let Ok(Ready::Red(failed)) = &got else {
            panic!("{got:?}");
        };
        assert_eq!(names(failed), ["pipeline #77 (failed)"]);
        spent(&recorded);
    }

    /// GitLab's record of !12 at [`SHA`]: purlis's own, from its save branch into `main`.
    fn gl_mr(status: &str) -> Value {
        json!({"iid": 12, "state": "opened", "draft": false, "sha": SHA,
               "source_branch": "purlis/save/test", "target_branch": "main",
               "source_project_id": 1, "target_project_id": 1, "description": "",
               "updated_at": "2026-01-01T00:00:00.000Z", "detailed_merge_status": status})
    }

    /// Review N5: GitLab answers the pinned merge by setting the request to merge later. That
    /// would merge whatever head the branch has then, so purlis cancels it at once.
    #[test]
    fn a_gitlab_merge_set_to_happen_later_is_cancelled_at_once() {
        purlis_core::unsteered!();
        let merge = "projects/acme%2Fapi/merge_requests/12";
        let (backend, recorded) = over(
            Kind::GitLab,
            vec![
                get(
                    "projects/acme%2Fapi",
                    json!({"squash_option": "default_off"}),
                ),
                get(merge, gl_mr("mergeable")),
                get(
                    &format!("{merge}/pipelines?per_page=100"),
                    json!([{"id": 77, "sha": SHA, "source": "push", "ref": "purlis/save/test",
                            "status": "success"}]),
                ),
                rest(
                    Some("PUT"),
                    &format!("{merge}/merge"),
                    json!([{"typed": ["squash", "false"]}, {"text": ["sha", SHA]}]),
                    json!({"iid": 12, "state": "opened", "merge_when_pipeline_succeeds": true}),
                ),
                rest(
                    Some("POST"),
                    &format!("{merge}/cancel_merge_when_pipeline_succeeds"),
                    json!([]),
                    json!({"iid": 12, "state": "opened"}),
                ),
            ],
        );
        let got = backend.merge_when_ready(&caller(), "acme/api", &PR, &AT, SETTLED);
        assert!(
            matches!(&got, Ok(Ready::Waiting(why)) if why.contains("cancelled")),
            "{got:?}"
        );
        spent(&recorded);
    }

    /// `request_auto_merge` stays on the seam, and its own recording still answers it. No save
    /// calls it any more (`tests/a_pr_mode_save_…`, `tests/a_repo_is_saved_by_its_mode.rs`).
    #[test]
    fn a_repo_with_auto_merge_has_it_queued_when_the_seam_is_asked() {
        purlis_core::unsteered!();
        for kind in BOTH {
            let recorded =
                Arc::new(Recorded::parse(&recording(kind, "request_auto_merge")).unwrap());
            let backend =
                Forge::default_of(Kind::parse(kind).unwrap()).backend_over(recorded.clone());
            assert_eq!(
                backend.request_auto_merge(&caller(), "acme/api", &PR, SHA),
                Ok(AutoMerge::Queued),
                "{kind}"
            );
            spent(&recorded);
        }
    }
}
