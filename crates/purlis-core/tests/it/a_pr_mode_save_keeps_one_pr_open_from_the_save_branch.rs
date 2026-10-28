// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The plane's `pr` and `pr-merge` modes (charter-app#298, ADR 0051): a save commits on the
//! target branch, pushes it to this machine's one rolling save branch, and keeps one pull
//! request open from there into the target branch — and once that PR has merged, the local
//! target branch is moved onto the remote's, keeping newer work.
//!
//! Real git against a local bare remote, and a stand-in `gh` that answers only what a test
//! wrote down (`support/forge_cli.rs`), so nothing here reaches a network. The remote is
//! reached the way `planegit`'s own tests reach one: `origin` in the SSH form and an
//! `url.<file>.insteadOf` keyed on the HTTPS base, in the plane's LOCAL config, so the URL
//! charter builds is the one git maps onto the bare repository.
//!
//! Each test has a host of its own, declared as a `[[forge]]` in the plane's `charter.toml`,
//! because the stand-in keeps each host's questions in a directory of their own.

use crate::support;

use std::path::{Path, PathBuf};

use purlis_core::planegit::{self, Request, Stage, Trigger};
use purlis_core::repocmd::Say;
use support::forge_cli::{Scene, in_a_child, in_child, was_asked};

#[test]
fn every_pr_mode_save_is_run_against_a_stand_in_forge() {
    purlis_core::unsteered!();
    in_a_child(
        "a_pr_mode_save_keeps_one_pr_open_from_the_save_branch::pr_saves::",
        "bin",
    );
}

mod pr_saves {
    use super::*;

    const SAVE: &str = "charter/save/test";

    struct Plane {
        _dir: tempfile::TempDir,
        root: PathBuf,
        bare: PathBuf,
        scene: Scene,
    }

    /// git for the test's own setup.
    fn git(dir: &Path, args: &[&str]) -> String {
        String::from_utf8_lossy(&support::git(dir, args).stdout)
            .trim()
            .to_string()
    }

    /// A plane on `release`, saved in `mode`, whose origin is a bare repository standing in
    /// for a GitHub at `host`.
    fn plane(host: &str, mode: &str) -> Plane {
        plane_on("github", host, mode)
    }

    /// [`plane`], on a forge of `kind`.
    fn plane_on(kind: &str, host: &str, mode: &str) -> Plane {
        let dir = tempfile::tempdir().unwrap();
        let top = std::fs::canonicalize(dir.path()).unwrap();
        let root = top.join("plane");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("charter.toml"),
            format!(
                "[[forge]]\nkind = \"{kind}\"\nhost = \"{host}\"\nowner = \"acme\"\n\n\
                 [plane]\nmode = \"{mode}\"\nbranch = \"release\"\nsave_branch = \"{SAVE}\"\n"
            ),
        )
        .unwrap();
        std::fs::write(root.join(".gitignore"), ".charter/\n").unwrap();
        std::fs::write(root.join("README.md"), "one\n").unwrap();
        git(&root, &["init", "-q", "-b", "release", "."]);
        // The product's own git reads the repo's config, and the child's HOME has none.
        git(&root, &["config", "user.name", "Fixture"]);
        git(&root, &["config", "user.email", "fixture@example.invalid"]);
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "one"]);
        let bare = top.join("forge/acme/plane.git");
        std::fs::create_dir_all(&bare).unwrap();
        git(&bare, &["init", "-q", "--bare", "-b", "release", "."]);
        git(
            &root,
            &[
                "remote",
                "add",
                "origin",
                &format!("git@{host}:acme/plane.git"),
            ],
        );
        git(
            &root,
            &[
                "config",
                &format!("url.file://{}/.insteadOf", top.join("forge/acme").display()),
                &format!("https://{host}/acme/"),
            ],
        );
        git(
            &root,
            &[
                "push",
                "-q",
                &bare.display().to_string(),
                "HEAD:refs/heads/release",
            ],
        );
        git(
            &root,
            &["update-ref", "refs/remotes/origin/release", "HEAD"],
        );
        Plane {
            _dir: dir,
            root,
            bare,
            scene: Scene::new(host),
        }
    }

    impl Plane {
        fn save_as(&self, message: Option<&str>, trigger: Trigger) -> (u8, String) {
            let mut said = String::new();
            let mut say = |line: Say| {
                said.push_str(&line.to_string());
                said.push('\n');
            };
            let code = planegit::save_as(
                &Request {
                    root: &self.root,
                    message,
                    sign: false,
                    no_push: false,
                    cwd: &self.root,
                    provenance: None,
                },
                trigger,
                &mut say,
            );
            (code, said)
        }

        fn save(&self, file: &str, message: &str) -> (u8, String) {
            std::fs::write(self.root.join(file), message).unwrap();
            self.save_as(Some(message), Trigger::Manual)
        }

        /// A commit in the remote's `release` made by somebody else, through a clone of it.
        fn theirs(&self) -> PathBuf {
            let theirs = self.root.parent().unwrap().join("theirs");
            if !theirs.exists() {
                git(
                    self.root.parent().unwrap(),
                    &[
                        "clone",
                        "-q",
                        &self.bare.display().to_string(),
                        &theirs.display().to_string(),
                    ],
                );
                git(&theirs, &["config", "user.name", "Other"]);
                git(&theirs, &["config", "user.email", "other@example.invalid"]);
            }
            git(&theirs, &["fetch", "-q", "origin"]);
            git(&theirs, &["checkout", "-q", "release"]);
            git(&theirs, &["reset", "-q", "--hard", "origin/release"]);
            theirs
        }

        fn their_commit(&self, file: &str, text: &str) {
            let theirs = self.theirs();
            std::fs::write(theirs.join(file), text).unwrap();
            git(&theirs, &["add", "-A"]);
            git(&theirs, &["commit", "-q", "-m", file]);
            git(&theirs, &["push", "-q", "origin", "release"]);
        }

        fn remote(&self, branch: &str) -> String {
            support::git(&self.bare, &["rev-parse", "--verify", "-q", branch])
                .stdout
                .iter()
                .map(|b| *b as char)
                .collect::<String>()
                .trim()
                .to_string()
        }

        fn head(&self) -> String {
            git(&self.root, &["rev-parse", "HEAD"])
        }

        fn lookup(&self, open: Option<u64>) -> PathBuf {
            let out = match open {
                Some(n) => format!(r#"[{{"number": {n}, "html_url": "{}"}}]"#, self.url(n)),
                None => "[]".to_string(),
            };
            self.scene.gh_api(
                "repos/acme/plane/pulls?state=open&head=acme:charter%2Fsave%2Ftest&base=release&per_page=1",
                0,
                &out,
                "",
            )
        }

        fn url(&self, number: u64) -> String {
            format!("https://{}/acme/plane/pull/{number}", self.scene.host)
        }

        fn created(&self, number: u64, subjects: &[&str]) -> PathBuf {
            let host = self.scene.host.clone();
            self.scene.answers(
                "gh",
                &[
                    "api",
                    "--hostname",
                    &host,
                    "-X",
                    "POST",
                    "repos/acme/plane/pulls",
                    "-f",
                    &format!("head={SAVE}"),
                    "-f",
                    "base=release",
                    "-f",
                    &format!("title={}", title(subjects)),
                    "-f",
                    &format!("body={}", body(subjects)),
                ],
                0,
                &format!(
                    r#"{{"number": {number}, "html_url": "{}"}}"#,
                    self.url(number)
                ),
                "",
            )
        }

        fn updated(&self, number: u64, subjects: &[&str]) -> PathBuf {
            let host = self.scene.host.clone();
            self.scene.answers(
                "gh",
                &[
                    "api",
                    "--hostname",
                    &host,
                    "-X",
                    "PATCH",
                    &format!("repos/acme/plane/pulls/{number}"),
                    "-f",
                    &format!("title={}", title(subjects)),
                    "-f",
                    &format!("body={}", body(subjects)),
                ],
                0,
                &format!(
                    r#"{{"number": {number}, "html_url": "{}"}}"#,
                    self.url(number)
                ),
                "",
            )
        }

        /// PR `number` is open, or closed without merging.
        fn state(&self, number: u64, open: bool) -> PathBuf {
            let state = if open { "open" } else { "closed" };
            self.scene.gh_api(
                &format!("repos/acme/plane/pulls/{number}"),
                0,
                &format!(r#"{{"state": "{state}", "merged": false, "merge_commit_sha": null}}"#),
                "",
            )
        }

        /// PR `number` merged, as the commit `at` on the target branch.
        fn merged(&self, number: u64, at: &str) -> PathBuf {
            self.scene.gh_api(
                &format!("repos/acme/plane/pulls/{number}"),
                0,
                &format!(r#"{{"state": "closed", "merged": true, "merge_commit_sha": "{at}"}}"#),
                "",
            )
        }

        /// A save of `file` that opens PR `number` from the save branch.
        fn opened(&self, file: &str, number: u64) -> String {
            self.lookup(None);
            let created = self.created(number, &[file]);
            let (code, said) = self.save(file, file);
            assert_eq!(code, 0, "{said}");
            assert!(was_asked(&created), "{said}");
            said
        }

        fn standing(&self) -> planegit::Standing {
            planegit::standing(&self.root)
        }

        fn last_journal(&self) -> serde_json::Value {
            planegit::journal(&self.root).pop().expect("a journal line")
        }

        /// `plane-push.json`: what the last push came to, as `doctor` and the next save read it.
        fn record(&self) -> serde_json::Value {
            planegit::push_record(&self.root).expect("a push record")
        }
    }

    fn title(subjects: &[&str]) -> String {
        match subjects {
            [one] => (*one).to_string(),
            _ => format!(
                "{} saves from {}",
                subjects.len(),
                purlis_core::dispatch::host()
            ),
        }
    }

    fn body(subjects: &[&str]) -> String {
        let mut out = format!(
            "Saved by charter on {}. Every save from this machine pushes to {SAVE} and updates \
             this pull request.\n",
            purlis_core::dispatch::host()
        );
        for subject in subjects {
            out.push_str(&format!("\n- {subject}"));
        }
        out
    }

    #[test]
    fn a_pr_save_pushes_to_the_save_branch_and_opens_one_pr_into_the_target_branch() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("pr-open.test", "pr");
        let release = plane.remote("release");

        let said = plane.opened("work.md", 12);

        assert_eq!(plane.remote(SAVE), plane.head(), "{said}");
        assert_eq!(
            plane.remote("release"),
            release,
            "the target is never pushed to"
        );
        assert!(said.contains(&plane.url(12)), "{said}");
        assert!(!said.contains("cannot open the pull request"), "{said}");
        let standing = plane.standing();
        assert_eq!(standing.stage, Stage::PrOpen, "{standing:?}");
        assert_eq!(standing.pr.as_deref(), Some(plane.url(12).as_str()));
        let line = plane.last_journal();
        assert_eq!(line["outcome"], "pr-open");
        assert_eq!(line["pr"], plane.url(12).as_str());
        let record = plane.record();
        assert_eq!(record["outcome"], "pr-open", "{record}");
        assert_eq!(record["landed"], SAVE, "{record}");
        assert_eq!(record["number"], 12, "{record}");
        assert_eq!(record["url"], plane.url(12).as_str(), "{record}");
    }

    #[test]
    fn the_next_save_updates_the_same_pr_and_never_opens_a_second() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("pr-update.test", "pr");
        plane.opened("one.md", 12);
        let asked = plane.state(12, true);
        plane.lookup(Some(12));
        let updated = plane.updated(12, &["one.md", "two.md"]);

        let (code, said) = plane.save("two.md", "two.md");

        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&updated), "{said}");
        assert!(
            was_asked(&asked),
            "the save asked where its PR stands: {said}"
        );
        assert_eq!(plane.remote(SAVE), plane.head(), "{said}");
        assert_eq!(plane.standing().stage, Stage::PrOpen);
    }

    /// A `pr-merge` plane with one commit an agent made with plain git, so the save carries
    /// it and its sha is known; the lookup and the create of #12 are written down.
    fn pr_merge_plane(host: &str) -> (Plane, String) {
        let plane = plane(host, "pr-merge");
        std::fs::write(plane.root.join("work.md"), "work").unwrap();
        git(&plane.root, &["add", "-A"]);
        git(&plane.root, &["commit", "-q", "-m", "work.md"]);
        let head = plane.head();
        plane.lookup(None);
        plane.created(12, &["work.md"]);
        (plane, head)
    }

    /// GitHub's settings answer and its auto-merge mutation: what a save used to ask, written
    /// down so a test can say neither was asked.
    fn auto_merge_questions(plane: &Plane, head: &str) -> [PathBuf; 2] {
        let host = plane.scene.host.clone();
        let settings = plane.scene.answers(
            "gh",
            &[
                "api",
                "graphql",
                "--hostname",
                &host,
                "-f",
                &format!("query={SETTINGS}"),
                "-f",
                "owner=acme",
                "-f",
                "name=plane",
                "-F",
                "number=12",
            ],
            0,
            r#"{"data": {"repository": {"autoMergeAllowed": true, "rebaseMergeAllowed": true,
                "mergeCommitAllowed": true, "squashMergeAllowed": true,
                "pullRequest": {"id": "PR_12"}}}}"#,
            "",
        );
        let enable = plane.scene.answers(
            "gh",
            &[
                "api",
                "graphql",
                "--hostname",
                &host,
                "-f",
                &format!("query={ENABLE}"),
                "-f",
                "id=PR_12",
                "-f",
                "method=REBASE",
                "-f",
                &format!("head={head}"),
            ],
            0,
            r#"{"data": {"enablePullRequestAutoMerge": {"clientMutationId": null}}}"#,
            "",
        );
        [settings, enable]
    }

    /// What one look at #12 reads, written down once: that it is open; GitHub's
    /// `mergeStateStatus` for it, at `head`; and the check runs there, `runs` as GitHub lists
    /// them. Answers the read of its merge state, so a test can say whether purlis looked.
    fn looked(plane: &Plane, head: &str, merge_state: &str, runs: &[&str]) -> PathBuf {
        plane.state(12, true);
        let host = plane.scene.host.clone();
        let read = plane.scene.answers(
            "gh",
            &[
                "api",
                "graphql",
                "--hostname",
                &host,
                "-f",
                &format!("query={MERGE_READY}"),
                "-f",
                "owner=acme",
                "-f",
                "name=plane",
                "-F",
                "number=12",
            ],
            0,
            &format!(
                r#"{{"data": {{"repository": {{"rebaseMergeAllowed": true,
                "mergeCommitAllowed": true, "squashMergeAllowed": true,
                "pullRequest": {{"state": "OPEN", "isDraft": false, "headRefOid": "{head}",
                "mergeStateStatus": "{merge_state}", "reviewDecision": null,
                "isMergeQueueEnabled": false, "baseRefName": "release",
                "headRefName": "charter/save/test", "isCrossRepository": false,
                "updatedAt": "2026-01-01T00:00:00Z", "body": "",
                "commits": {{"nodes": [{{"commit": {{"oid": "{head}",
                "statusCheckRollup": null}}}}]}}}}}}}}}}"#
            ),
            "",
        );
        plane.scene.gh_api(
            &format!("repos/acme/plane/commits/{head}/check-runs?per_page=100"),
            0,
            &format!(
                r#"{{"total_count": {}, "check_runs": [{}]}}"#,
                runs.len(),
                runs.join(", ")
            ),
            "",
        );
        plane.scene.gh_api(
            &format!("repos/acme/plane/commits/{head}/status?per_page=100"),
            0,
            r#"{"state": "pending", "total_count": 0, "statuses": []}"#,
            "",
        );
        read
    }

    /// GitHub's answer to whether other pull requests into `release` had checks: #11, with a
    /// roll-up at its head when `had_checks`.
    fn other_requests(plane: &Plane, had_checks: bool) -> PathBuf {
        let host = plane.scene.host.clone();
        let rollup = if had_checks {
            r#"{"state": "SUCCESS"}"#
        } else {
            "null"
        };
        plane.scene.answers(
            "gh",
            &[
                "api",
                "graphql",
                "--hostname",
                &host,
                "-f",
                &format!("query={CHECKS_EXPECTED}"),
                "-f",
                "owner=acme",
                "-f",
                "name=plane",
                "-f",
                "base=release",
            ],
            0,
            &format!(
                r#"{{"data": {{"repository": {{"pullRequests": {{"nodes": [
                {{"number": 12, "commits": {{"nodes": [{{"commit": {{"statusCheckRollup": null}}}}]}}}},
                {{"number": 11, "commits": {{"nodes": [{{"commit": {{"statusCheckRollup": {rollup}}}}}]}}}}
                ]}}}}}}}}"#
            ),
            "",
        )
    }

    /// GitHub's merge of #12 at `head`, by rebase, answered as merged.
    fn merged_by_purlis(plane: &Plane, head: &str) -> PathBuf {
        let host = plane.scene.host.clone();
        plane.scene.answers(
            "gh",
            &[
                "api",
                "--hostname",
                &host,
                "-X",
                "PUT",
                "repos/acme/plane/pulls/12/merge",
                "-f",
                "merge_method=rebase",
                "-f",
                &format!("sha={head}"),
            ],
            0,
            r#"{"merged": true, "message": "Pull Request successfully merged"}"#,
            "",
        )
    }

    /// The wait for the pushed commit's checks to start is over: what is kept says its head
    /// was pushed an hour ago.
    fn the_settle_wait_is_over(plane: &Plane) {
        let path = purlis_core::plane::state_dir(&plane.root).join("save-branch.json");
        let mut kept: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let pushed = kept["pr"]["pushed_at"]
            .as_u64()
            .expect("when it was pushed");
        kept["pr"]["pushed_at"] = serde_json::json!(pushed - 3600);
        std::fs::write(&path, kept.to_string()).unwrap();
    }

    /// The app's look at the plane's open pull request, with no save.
    fn look(plane: &Plane) -> planegit::Looked {
        planegit::look(
            &plane.root,
            &purlis_core::forge::Caller::trigger(),
            &mut |_| {},
        )
        .expect("the look")
    }

    const SUCCESS: &str =
        r#"{"id": 4, "name": "build", "status": "completed", "conclusion": "success"}"#;
    const RUNNING: &str =
        r#"{"id": 4, "name": "build", "status": "in_progress", "conclusion": null}"#;
    const FAILED: &str = r#"{"id": 7, "name": "test", "status": "completed", "conclusion": "failure", "html_url": "https://pr-merge.example/acme/plane/actions/runs/9/job/7"}"#;

    /// **A save merges nothing itself, and asks the forge for no auto-merge** (ADR 0051,
    /// amended 2026-10-13): the commit reached the forge a moment ago, no check on it has had
    /// time to start, and the forge's own auto-merge would merge past a failing check that is
    /// not required. The request waits, and the standing says so.
    #[test]
    fn a_pr_merge_save_merges_nothing_at_the_save_and_asks_the_forge_for_no_auto_merge() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let (plane, head) = pr_merge_plane("pr-merge.test");
        let auto_merge = auto_merge_questions(&plane, &head);
        let read = looked(&plane, &head, "CLEAN", &[]);
        let put = merged_by_purlis(&plane, &head);

        let (code, said) = plane.save_as(None, Trigger::Manual);

        assert_eq!(code, 0, "{said}");
        for question in &auto_merge {
            assert!(
                !was_asked(question),
                "the forge was asked to auto-merge: {said}"
            );
        }
        assert!(
            !was_asked(&read),
            "the save looked before any check could start: {said}"
        );
        assert!(!was_asked(&put), "merged at the save: {said}");
        assert!(said.contains("not had time to start"), "{said}");
        assert_eq!(plane.remote(SAVE), head);
        let standing = plane.standing();
        assert_eq!(standing.stage, Stage::PrOpen, "{standing:?}");
        let merge = standing.merge.expect("where the request stands");
        assert_eq!(merge.state, planegit::MergeState::Waiting, "{merge:?}");
        let record = plane.record();
        assert!(
            record["detail"]
                .as_str()
                .is_some_and(|why| why.contains("purlis merges pull request #12 itself")),
            "{record}"
        );
    }

    /// **The fault** (2026-10-13): no check had registered straight after the push, purlis
    /// merged, and the check then failed on a request already merged. Now the save waits, and
    /// the look after it finds the failed check: the request stays open and unmerged, red,
    /// named by the check — and the plane is not blocked, so the next save pushes onto the
    /// same request (the operator's ruling P1).
    #[test]
    fn no_check_at_the_save_and_a_failing_one_later_leaves_the_pr_open_red_and_unmerged() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let (plane, head) = pr_merge_plane("pr-merge-red.test");
        let release = plane.remote("release");
        let (code, said) = plane.save_as(None, Trigger::Manual);
        assert_eq!(code, 0, "{said}");
        the_settle_wait_is_over(&plane);
        looked(&plane, &head, "UNSTABLE", &[SUCCESS, FAILED]);
        let put = merged_by_purlis(&plane, &head);

        let found = look(&plane);

        assert!(!was_asked(&put), "merged past a failed check");
        assert_eq!(
            plane.remote("release"),
            release,
            "nothing reached the target"
        );
        assert_eq!(
            found,
            planegit::Looked {
                open: true,
                changed: true,
                moved: false
            }
        );
        let standing = plane.standing();
        assert_eq!(
            standing.stage,
            Stage::PrOpen,
            "red is not a block: {standing:?}"
        );
        assert_eq!(standing.blocked, None);
        let merge = standing.merge.expect("where the request stands");
        assert_eq!(merge.state, planegit::MergeState::Red, "{merge:?}");
        assert_eq!(merge.why, "1 check failed: test");
        assert_eq!(merge.checks.len(), 1);
        assert_eq!(merge.checks[0].name, "test");
        assert_eq!(
            merge.checks[0].url.as_deref(),
            Some("https://pr-merge.example/acme/plane/actions/runs/9/job/7")
        );
        let record = plane.record();
        assert!(
            record["detail"]
                .as_str()
                .is_some_and(|why| why.contains("is red and stays open")),
            "{record}"
        );

        // The next save is not blocked: it pushes onto the same request, which then waits
        // for the new commit's checks.
        looked(&plane, &head, "UNSTABLE", &[SUCCESS, FAILED]);
        plane.lookup(Some(12));
        let updated = plane.updated(12, &["work.md", "fix.md"]);
        let (code, said) = plane.save("fix.md", "fix.md");
        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&updated), "{said}");
        assert!(!was_asked(&put), "{said}");
        assert_eq!(plane.remote(SAVE), plane.head(), "{said}");
        let standing = plane.standing();
        assert_eq!(standing.stage, Stage::PrOpen);
        assert_eq!(
            standing.merge.map(|merge| merge.state),
            Some(planegit::MergeState::Waiting),
            "the new commit's checks have not started"
        );
    }

    /// No check, and no sign of CI anywhere: the save waits, a look straight after it still
    /// waits, and the look once the head is settled merges it **without a new save** and moves
    /// the plane onto what merged.
    #[test]
    fn a_pr_with_no_check_and_no_sign_of_ci_is_merged_by_a_later_look_without_a_new_save() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let (plane, head) = pr_merge_plane("pr-merge-none.test");
        let (code, said) = plane.save_as(None, Trigger::Manual);
        assert_eq!(code, 0, "{said}");

        // A look a moment after the push: no check yet is "not yet".
        looked(&plane, &head, "CLEAN", &[]);
        let put = merged_by_purlis(&plane, &head);
        let found = look(&plane);
        assert!(!was_asked(&put), "merged before any check could start");
        assert!(found.open && !found.moved, "{found:?}");
        assert_eq!(plane.standing().stage, Stage::PrOpen);

        // A minute on: still no check, and no other pull request into release ever had one.
        the_settle_wait_is_over(&plane);
        looked(&plane, &head, "CLEAN", &[]);
        let asked = other_requests(&plane, false);
        let at = merge(&plane, "rebase");
        plane.merged(12, &at);

        let found = look(&plane);

        assert!(
            was_asked(&asked),
            "whether the repository runs checks was not asked"
        );
        assert!(was_asked(&put), "merged at the pushed commit");
        assert_eq!(
            found,
            planegit::Looked {
                open: false,
                changed: true,
                moved: true
            }
        );
        assert_eq!(
            plane.head(),
            plane.remote("release"),
            "moved onto what merged"
        );
        let standing = plane.standing();
        assert_eq!(standing.stage, Stage::Saved, "{standing:?}");
        assert_eq!(standing.merge, None);
    }

    /// No check at a settled head, where another pull request into the same branch had
    /// checks: they are expected here too, so the request waits and nothing is merged.
    #[test]
    fn a_pr_with_no_check_waits_where_other_prs_into_the_branch_had_checks() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let (plane, head) = pr_merge_plane("pr-merge-expected.test");
        let (code, said) = plane.save_as(None, Trigger::Manual);
        assert_eq!(code, 0, "{said}");
        the_settle_wait_is_over(&plane);
        looked(&plane, &head, "CLEAN", &[]);
        other_requests(&plane, true);
        let put = merged_by_purlis(&plane, &head);

        let found = look(&plane);

        assert!(!was_asked(&put), "merged where checks are expected");
        assert!(found.open, "{found:?}");
        let merge = plane.standing().merge.expect("where the request stands");
        assert_eq!(merge.state, planegit::MergeState::Waiting, "{merge:?}");
        assert!(merge.why.contains("#11"), "{merge:?}");
    }

    /// Checks that are running wait; once they pass, the next fetch merges the request at the
    /// pushed commit and moves the plane onto what merged.
    #[test]
    fn pr_merge_merges_the_pr_once_its_checks_pass() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let (plane, head) = pr_merge_plane("pr-merge-free.test");
        let (code, said) = plane.save_as(None, Trigger::Manual);
        assert_eq!(code, 0, "{said}");
        looked(&plane, &head, "BLOCKED", &[RUNNING]);
        let put = merged_by_purlis(&plane, &head);
        look(&plane);
        assert!(!was_asked(&put), "merged while a check was running");
        let merge = plane.standing().merge.expect("where the request stands");
        assert_eq!(merge.state, planegit::MergeState::Waiting, "{merge:?}");
        assert!(merge.why.contains("still running"), "{merge:?}");

        // A check that passed while the head is still new is not every check (review N1): a
        // look then waits, and merges nothing.
        looked(&plane, &head, "CLEAN", &[SUCCESS]);
        look(&plane);
        assert!(!was_asked(&put), "merged on one early pass");
        let merge = plane.standing().merge.expect("where the request stands");
        assert!(merge.why.contains("may not have started"), "{merge:?}");

        // A minute on, the check still passed: the next fetch merges it, and the plane moves.
        the_settle_wait_is_over(&plane);
        looked(&plane, &head, "CLEAN", &[SUCCESS]);
        let at = merge_on_the_forge(&plane);
        plane.merged(12, &at);

        planegit::fetch(&plane.root, true).expect("the fetch");

        assert!(was_asked(&put), "merged at the pushed commit");
        assert_eq!(
            plane.head(),
            plane.remote("release"),
            "moved onto what merged"
        );
        assert_eq!(plane.standing().stage, Stage::Saved);
    }

    /// A clock set back after the push leaves a stamp in the future. That never reads as
    /// "long ago" (review N4): the look waits, and stamps the request again so it settles a
    /// wait later rather than never.
    #[test]
    fn a_push_stamped_in_the_future_is_not_settled_and_is_stamped_again() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let (plane, head) = pr_merge_plane("pr-merge-clock.test");
        let (code, said) = plane.save_as(None, Trigger::Manual);
        assert_eq!(code, 0, "{said}");
        let path = purlis_core::plane::state_dir(&plane.root).join("save-branch.json");
        let read = || -> serde_json::Value {
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
        };
        let mut kept = read();
        let pushed = kept["pr"]["pushed_at"]
            .as_u64()
            .expect("when it was pushed");
        kept["pr"]["pushed_at"] = serde_json::json!(pushed + 86_400);
        std::fs::write(&path, kept.to_string()).unwrap();
        looked(&plane, &head, "CLEAN", &[]);
        let asked = other_requests(&plane, false);
        let put = merged_by_purlis(&plane, &head);

        look(&plane);

        assert!(!was_asked(&asked), "a future stamp was read as settled");
        assert!(!was_asked(&put), "merged on a future stamp");
        let stamped = read()["pr"]["pushed_at"].as_u64().expect("stamped again");
        assert!(stamped < pushed + 3600, "left in the future: {stamped}");
    }

    /// What is kept beside the request is a hint, and the project's settings are what it
    /// declared: a `pr` plane's request is never merged by purlis, even when the file that
    /// keeps the request says purlis is to merge it.
    #[test]
    fn a_pr_mode_request_is_never_merged_whatever_the_kept_file_says() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("pr-not-merge.test", "pr");
        plane.opened("work.md", 12);
        let head = plane.head();
        let path = purlis_core::plane::state_dir(&plane.root).join("save-branch.json");
        let mut kept: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        kept["pr"]["merges_itself"] = serde_json::json!(true);
        kept["pr"]["pushed_at"] = serde_json::json!(1_700_000_000u64);
        std::fs::write(&path, kept.to_string()).unwrap();
        let read = looked(&plane, &head, "CLEAN", &[SUCCESS]);
        let put = merged_by_purlis(&plane, &head);

        planegit::fetch(&plane.root, true).expect("the fetch");

        assert!(
            !was_asked(&read),
            "a pr-mode request was looked at to merge it"
        );
        assert!(!was_asked(&put), "a pr-mode request was merged");
        assert_eq!(plane.standing().stage, Stage::PrOpen);
    }

    /// **The app's paced looks, end to end but for the worker's thread**: the pacing the
    /// auto-save worker asks every two seconds (`autosave::Looks`) and the look it then makes
    /// (`planegit::look`), against the stand-in forge. Nothing is asked of the forge in the
    /// first minute, and one look after it merges the request with no save.
    #[test]
    fn the_apps_paced_looks_ask_nothing_for_a_minute_and_then_merge_a_request_with_no_save() {
        purlis_core::unsteered!();
        use std::time::{Duration, Instant};
        if !in_child() {
            return;
        }
        let (plane, head) = pr_merge_plane("pr-merge-paced.test");
        let (code, said) = plane.save_as(None, Trigger::Manual);
        assert_eq!(code, 0, "{said}");
        let (_, at) = planegit::request_to_look_at(&plane.root).expect("an open request");
        assert_eq!(at, head);
        let mut looks = purlis_core::autosave::Looks::default();
        let start = Instant::now();
        let due = |looks: &mut purlis_core::autosave::Looks, s: u64| {
            looks.due(
                start + Duration::from_secs(s),
                &at,
                Some(Duration::ZERO),
                false,
            )
        };
        // The first minute: no look is due, so nothing is asked (nothing is written down,
        // and a look would have found no answer).
        for s in (0..60).step_by(2) {
            assert!(!due(&mut looks, s), "a look {s}s after the push");
        }
        the_settle_wait_is_over(&plane);
        looked(&plane, &head, "CLEAN", &[]);
        other_requests(&plane, false);
        let put = merged_by_purlis(&plane, &head);
        let at_forge = merge(&plane, "rebase");
        plane.merged(12, &at_forge);
        let mut made = 0;
        for s in (60..90).step_by(2) {
            if due(&mut looks, s) {
                made += 1;
                look(&plane);
            }
        }
        assert_eq!(made, 1, "one look once the checks could have started");
        assert!(was_asked(&put), "merged at the pushed commit");
        assert_eq!(plane.head(), plane.remote("release"));
        assert_eq!(planegit::request_to_look_at(&plane.root), None);
    }

    /// The request a look merges is this save's own, into its own target, by the forge's
    /// answer (review round 2, R1 and R2). The forge says #12 is from another branch purlis
    /// names, then that it goes into another base: neither is merged, by a look or by the
    /// person's merge, and the standing says why.
    #[test]
    fn a_request_the_forge_says_is_not_this_saves_own_into_its_target_is_never_merged() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let (plane, head) = pr_merge_plane("pr-merge-own.test");
        let (code, said) = plane.save_as(None, Trigger::Manual);
        assert_eq!(code, 0, "{said}");
        the_settle_wait_is_over(&plane);
        let put = merged_by_purlis(&plane, &head);
        let host = plane.scene.host.clone();
        let answers = |head_ref: &str, base_ref: &str| {
            plane.scene.answers(
                "gh",
                &[
                    "api",
                    "graphql",
                    "--hostname",
                    &host,
                    "-f",
                    &format!("query={MERGE_READY}"),
                    "-f",
                    "owner=acme",
                    "-f",
                    "name=plane",
                    "-F",
                    "number=12",
                ],
                0,
                &format!(
                    r#"{{"data": {{"repository": {{"rebaseMergeAllowed": true,
                    "mergeCommitAllowed": true, "squashMergeAllowed": true,
                    "pullRequest": {{"state": "OPEN", "isDraft": false, "headRefOid": "{head}",
                    "mergeStateStatus": "CLEAN", "reviewDecision": null,
                    "isMergeQueueEnabled": false, "baseRefName": "{base_ref}",
                    "headRefName": "{head_ref}", "isCrossRepository": false,
                    "updatedAt": "2026-01-01T00:00:00Z", "body": "",
                    "commits": {{"nodes": []}}}}}}}}}}"#
                ),
                "",
            );
        };

        // Another request purlis opened, into the plane's own target.
        plane.state(12, true);
        answers("charter/change/api-2", "release");
        look(&plane);
        assert!(!was_asked(&put), "another purlis request was merged");
        let merge = plane.standing().merge.expect("where the request stands");
        assert_eq!(merge.state, planegit::MergeState::CannotMerge, "{merge:?}");
        assert!(merge.why.contains("not this save's request"), "{merge:?}");

        // This clone's save branch, into another base: the person's merge is refused too, on
        // the confirmation the window's look drew of it.
        // The stand-in answers each question once, so each read is written down for itself:
        // the window's look that draws the confirmation, then the press's own fresh look,
        // then the merge's read of the merge state.
        plane.state(12, true);
        answers(SAVE, "main");
        look(&plane);
        let seen = planegit::seen(&plane.root).expect("what the confirmation shows");
        plane.state(12, true);
        answers(SAVE, "main");
        answers(SAVE, "main");
        let merged = planegit::merge_as_seen(
            &plane.root,
            &purlis_core::forge::Caller::window(),
            &seen,
            &mut |_| {},
        );
        assert!(
            matches!(&merged, Err(why) if why.contains("goes into main")),
            "{merged:?}"
        );
        assert!(
            !was_asked(&put),
            "merged into a base that is not the plane's target"
        );
    }

    /// [`merge`] by rebase, named apart from a local `merge` a test holds.
    fn merge_on_the_forge(plane: &Plane) -> String {
        merge(plane, "rebase")
    }

    /// **Merge red is the person's alone**: a red request is merged only by the window's own
    /// "merge it as it stands", which reads no check and still merges only at the pushed
    /// commit (the operator's ruling P1).
    #[test]
    fn a_red_pr_is_merged_only_when_the_person_says_to_merge_it_as_it_stands() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let (plane, head) = pr_merge_plane("pr-merge-person.test");
        let (code, said) = plane.save_as(None, Trigger::Manual);
        assert_eq!(code, 0, "{said}");
        the_settle_wait_is_over(&plane);
        looked(&plane, &head, "UNSTABLE", &[FAILED]);
        look(&plane);
        assert_eq!(
            plane.standing().merge.map(|merge| merge.state),
            Some(planegit::MergeState::Red)
        );

        // What the confirmation showed: the window's look, handed back with the press.
        let seen = planegit::seen(&plane.root).expect("what the confirmation shows");
        assert_eq!(seen.head, head);
        // The press looks again first, and finds it as it was shown.
        looked(&plane, &head, "UNSTABLE", &[FAILED]);
        // The person's merge: the merge state is read, the checks are not.
        let host = plane.scene.host.clone();
        plane.scene.answers(
            "gh",
            &[
                "api",
                "graphql",
                "--hostname",
                &host,
                "-f",
                &format!("query={MERGE_READY}"),
                "-f",
                "owner=acme",
                "-f",
                "name=plane",
                "-F",
                "number=12",
            ],
            0,
            &format!(
                r#"{{"data": {{"repository": {{"rebaseMergeAllowed": true,
                "mergeCommitAllowed": true, "squashMergeAllowed": true,
                "pullRequest": {{"state": "OPEN", "isDraft": false, "headRefOid": "{head}",
                "mergeStateStatus": "UNSTABLE", "reviewDecision": null,
                "isMergeQueueEnabled": false, "baseRefName": "release",
                "headRefName": "charter/save/test", "isCrossRepository": false,
                "updatedAt": "2026-01-01T00:00:00Z", "body": "",
                "commits": {{"nodes": []}}}}}}}}}}"#
            ),
            "",
        );
        let put = merged_by_purlis(&plane, &head);
        let at = merge(&plane, "rebase");
        plane.merged(12, &at);

        let merged = planegit::merge_as_seen(
            &plane.root,
            &purlis_core::forge::Caller::window(),
            &seen,
            &mut |_| {},
        );

        assert_eq!(merged, Ok(planegit::MergedAsItStands::Moved));
        assert!(was_asked(&put), "merged at the pushed commit");
        assert_eq!(plane.head(), plane.remote("release"));
        assert_eq!(plane.standing().stage, Stage::Saved);
    }

    /// **The person's merge is of what they were shown** (review of #1953): the request is
    /// looked at afresh at the press, and a check that failed since the confirmation was
    /// drawn, or a commit pushed since, is not merged past.
    #[test]
    fn the_persons_merge_is_made_only_of_the_commit_and_the_standing_they_confirmed() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let (plane, head) = pr_merge_plane("pr-merge-seen.test");
        let (code, said) = plane.save_as(None, Trigger::Manual);
        assert_eq!(code, 0, "{said}");
        the_settle_wait_is_over(&plane);
        // The forge says a check failed, at every look from here on.
        looked(&plane, &head, "UNSTABLE", &[FAILED]);
        let put = merged_by_purlis(&plane, &head);
        let window = purlis_core::forge::Caller::window();

        // The confirmation showed a wait with no failed check: the failure came after.
        let waited = planegit::Merging::of(&Ok(purlis_core::forge::pr::Ready::Stalled(
            "no check has started on it".into(),
        )));
        let shown_a_wait = planegit::Seen {
            head: head.clone(),
            shown: waited.shown(),
        };
        let refused = planegit::merge_as_seen(&plane.root, &window, &shown_a_wait, &mut |_| {});
        assert!(
            matches!(&refused, Err(why) if why.contains("changed since you looked")),
            "{refused:?}"
        );
        assert!(!was_asked(&put), "merged past a failure nobody was shown");
        // What the fresh look found is what the window now draws.
        let red = plane.standing().merge.expect("where the request stands");
        assert_eq!(red.state, planegit::MergeState::Red, "{red:?}");

        // The confirmation showed this red request at another commit: one was pushed since.
        let at_an_older_commit = planegit::Seen {
            head: "1".repeat(40),
            shown: red.shown(),
        };
        let refused =
            planegit::merge_as_seen(&plane.root, &window, &at_an_older_commit, &mut |_| {});
        assert!(
            matches!(&refused, Err(why) if why.contains("a new commit was pushed")),
            "{refused:?}"
        );
        assert!(!was_asked(&put), "merged a commit nobody confirmed");
        assert_eq!(
            planegit::seen(&plane.root).map(|seen| seen.head),
            Some(head)
        );
    }

    const CHECKS_EXPECTED: &str =
        include_str!("../../src/forge/github/queries/checks_expected.graphql");
    const MERGE_READY: &str = include_str!("../../src/forge/github/queries/merge_ready.graphql");
    const SETTINGS: &str = "query($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name){autoMergeAllowed rebaseMergeAllowed mergeCommitAllowed squashMergeAllowed pullRequest(number:$number){id}}}";
    const ENABLE: &str = "mutation($id:ID!,$method:PullRequestMergeMethod!,$head:GitObjectID!){enablePullRequestAutoMerge(input:{pullRequestId:$id,mergeMethod:$method,expectedHeadOid:$head}){clientMutationId}}";

    #[test]
    fn a_commit_left_unpushed_is_carried_to_the_pr_by_the_next_save_with_nothing_new() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("pr-launch.test", "pr");
        std::fs::write(plane.root.join("work.md"), "work").unwrap();
        git(&plane.root, &["add", "-A"]);
        git(&plane.root, &["commit", "-q", "-m", "work.md"]);
        let standing = plane.standing();
        assert_eq!(standing.stage, Stage::Committed, "{standing:?}");
        assert!(purlis_core::autosave::worth_saving(&standing));
        plane.lookup(None);
        let created = plane.created(12, &["work.md"]);

        let (code, said) = plane.save_as(None, Trigger::Launch);

        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&created), "{said}");
        assert_eq!(plane.remote(SAVE), plane.head());
        assert_eq!(plane.standing().stage, Stage::PrOpen);
    }

    #[test]
    fn quitting_in_a_pr_mode_pushes_to_the_save_branch_and_never_to_the_target() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("pr-quit.test", "pr");
        let release = plane.remote("release");
        std::fs::write(plane.root.join("work.md"), "work").unwrap();
        plane.lookup(None);
        let created = plane.created(12, &["charter save: 1 file (work.md 1)"]);

        let quit = purlis_core::autosave::at_quit(&plane.root, std::time::Duration::from_secs(30));

        assert_eq!(quit, purlis_core::autosave::AtQuit::Pushed);
        assert!(was_asked(&created));
        assert_eq!(plane.remote(SAVE), plane.head());
        assert_eq!(plane.remote("release"), release);
    }

    /// Somebody else's commit on the remote's `release`, then the save branch merged into it
    /// by `how`, as the forge would do it. Answers the commit the merge made on `release`, which
    /// is what the forge names as the PR's merge commit.
    fn merge(plane: &Plane, how: &str) -> String {
        plane.their_commit("theirs.md", "theirs");
        let theirs = plane.theirs();
        let save = format!("origin/{SAVE}");
        match how {
            "merge" => {
                git(
                    &theirs,
                    &[
                        "merge",
                        "-q",
                        "--no-ff",
                        "-m",
                        "Merge pull request #12",
                        &save,
                    ],
                );
            }
            "rebase" => {
                git(&theirs, &["checkout", "-q", "-b", "rebased", &save]);
                git(&theirs, &["rebase", "-q", "release"]);
                git(&theirs, &["checkout", "-q", "release"]);
                git(&theirs, &["merge", "-q", "--ff-only", "rebased"]);
            }
            "squash" => {
                git(&theirs, &["merge", "-q", "--squash", &save]);
                git(&theirs, &["commit", "-q", "-m", "Squashed (#12)"]);
            }
            // A squash whose merger changed one of the PR's files on the way in.
            "squash-edited" => {
                git(&theirs, &["merge", "-q", "--squash", &save]);
                std::fs::write(theirs.join("work.md"), "edited while merging").unwrap();
                git(&theirs, &["add", "-A"]);
                git(&theirs, &["commit", "-q", "-m", "Squashed (#12)"]);
            }
            other => panic!("no merge called {other}"),
        }
        git(&theirs, &["push", "-q", "origin", "release"]);
        git(&theirs, &["rev-parse", "HEAD"])
    }

    fn merged_then_fetched_keeps_newer_work(host: &str, how: &str) {
        let plane = plane(host, "pr");
        plane.opened("work.md", 12);
        let at = merge(&plane, how);
        // Newer work, not yet saved: it must be kept.
        std::fs::write(plane.root.join("later.md"), "later").unwrap();
        let asked = plane.merged(12, &at);

        let incoming = planegit::fetch(&plane.root, true).expect("the fetch");

        assert!(was_asked(&asked), "{incoming:?}");
        assert_eq!(
            plane.head(),
            plane.remote("release"),
            "{how}: moved onto the remote"
        );
        assert_eq!(
            std::fs::read_to_string(plane.root.join("later.md")).unwrap(),
            "later",
            "{how}: newer work kept"
        );
        assert_eq!(
            std::fs::read_to_string(plane.root.join("work.md")).unwrap(),
            "work.md",
            "{how}"
        );
        let standing = plane.standing();
        assert_eq!(standing.stage, Stage::Changed, "{how}: {standing:?}");
        assert_eq!(standing.changed, vec!["later.md".to_string()]);
        assert_eq!(standing.pr, None, "{how}");
        assert_eq!(standing.ahead, Some(0), "{how}");
    }

    #[test]
    fn after_a_merge_commit_the_plane_moves_onto_the_target_keeping_newer_work() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        merged_then_fetched_keeps_newer_work("merged-merge.test", "merge");
    }

    #[test]
    fn after_a_rebase_merge_the_plane_moves_onto_the_target_keeping_newer_work() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        merged_then_fetched_keeps_newer_work("merged-rebase.test", "rebase");
    }

    #[test]
    fn after_a_squash_merge_the_plane_moves_onto_the_target_keeping_newer_work() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        merged_then_fetched_keeps_newer_work("merged-squash.test", "squash");
    }

    #[test]
    fn a_save_after_a_squash_merge_replays_the_newer_commit_and_opens_a_fresh_pr() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("squash-then-save.test", "pr");
        plane.opened("work.md", 12);
        let at = merge(&plane, "squash");
        plane.merged(12, &at);
        plane.lookup(None);
        let created = plane.created(13, &["later.md"]);

        let (code, said) = plane.save("later.md", "later.md");

        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&created), "{said}");
        let remote = plane.remote("release");
        assert_eq!(
            git(&plane.root, &["rev-parse", "HEAD~1"]),
            remote,
            "the newer commit sits on the remote's release: {said}"
        );
        assert_eq!(git(&plane.root, &["log", "-1", "--format=%s"]), "later.md");
        // The save branch was moved to a commit that does not descend from its old tip, which
        // only the lease on that one charter-owned branch allows.
        assert_eq!(plane.remote(SAVE), plane.head());
        assert_eq!(plane.standing().pr, Some(plane.url(13)));
    }

    #[test]
    fn a_pr_closed_without_merging_blocks_the_plane_and_the_next_save_opens_a_new_one() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("closed.test", "pr");
        plane.opened("work.md", 12);
        let head = plane.head();
        let release = plane.remote("release");
        plane.state(12, false);

        planegit::fetch(&plane.root, true).expect("the fetch");

        let standing = plane.standing();
        assert_eq!(standing.stage, Stage::Blocked, "{standing:?}");
        assert!(
            standing
                .blocked
                .as_deref()
                .is_some_and(|why| why.contains("closed without merging")),
            "{standing:?}"
        );
        assert_eq!(
            plane.head(),
            head,
            "nothing of this machine's is thrown away"
        );
        assert_eq!(plane.remote("release"), release);
        let record = plane.record();
        assert_eq!(record["outcome"], "blocked", "{record}");
        assert_eq!(record["landed"], SAVE, "{record}");
        assert_eq!(record["number"], 12, "{record}");
        assert_eq!(record["url"], plane.url(12).as_str(), "{record}");

        // A person saving again is the way out: a new PR for the same commits.
        plane.lookup(None);
        let created = plane.created(13, &["work.md"]);
        let (code, said) = plane.save_as(None, Trigger::Manual);
        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&created), "{said}");
        assert_eq!(plane.standing().stage, Stage::PrOpen);
        assert_eq!(plane.standing().pr, Some(plane.url(13)));
    }

    #[test]
    fn a_merge_commit_that_does_not_hold_what_was_pushed_blocks_the_plane() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("diverged.test", "pr");
        plane.opened("work.md", 12);
        let head = plane.head();
        let at = merge(&plane, "squash-edited");
        plane.merged(12, &at);

        planegit::fetch(&plane.root, true).expect("the fetch");

        let standing = plane.standing();
        assert_eq!(standing.stage, Stage::Blocked, "{standing:?}");
        assert!(
            standing
                .blocked
                .as_deref()
                .is_some_and(|why| why.contains("work.md")),
            "{standing:?}"
        );
        assert_eq!(plane.head(), head, "the local branch is left where it was");
    }

    #[test]
    fn a_save_branch_somebody_else_pushed_to_is_never_overwritten() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("lease.test", "pr");
        plane.opened("one.md", 12);
        // Somebody else's commit on this machine's save branch.
        let theirs = plane.theirs();
        git(&theirs, &["fetch", "-q", "origin"]);
        git(
            &theirs,
            &[
                "checkout",
                "-q",
                "-B",
                "stranger",
                &format!("origin/{SAVE}"),
            ],
        );
        std::fs::write(theirs.join("stranger.md"), "stranger").unwrap();
        git(&theirs, &["add", "-A"]);
        git(&theirs, &["commit", "-q", "-m", "stranger"]);
        git(
            &theirs,
            &["push", "-q", "origin", &format!("stranger:{SAVE}")],
        );
        let stranger = plane.remote(SAVE);
        plane.state(12, true);

        let (code, said) = plane.save("two.md", "two.md");

        assert_eq!(code, 0, "{said}");
        assert_eq!(
            plane.remote(SAVE),
            stranger,
            "their commit is still there: {said}"
        );
        assert_eq!(plane.standing().stage, Stage::Blocked, "{said}");
        let record = plane.record();
        assert_eq!(record["landed"], SAVE, "{record}");
        assert!(
            record["detail"]
                .as_str()
                .is_some_and(|why| why.contains("this clone did not push there")),
            "{record}"
        );

        // Once the stranger's branch is gone from the remote, the next save pushes again.
        git(&theirs, &["push", "-q", "origin", &format!(":{SAVE}")]);
        plane.state(12, true);
        plane.lookup(Some(12));
        let updated = plane.updated(12, &["one.md", "two.md"]);
        let (code, said) = plane.save_as(None, Trigger::Manual);
        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&updated), "{said}");
        assert_eq!(plane.remote(SAVE), plane.head(), "{said}");
        assert_eq!(plane.standing().stage, Stage::PrOpen, "{said}");
    }

    #[test]
    fn a_save_branch_that_is_the_target_branch_is_refused() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("same-branch.test", "pr");
        let toml = std::fs::read_to_string(plane.root.join("charter.toml"))
            .unwrap()
            .replace(SAVE, "release");
        std::fs::write(plane.root.join("charter.toml"), toml).unwrap();
        git(&plane.root, &["commit", "-q", "-am", "settings"]);
        let release = plane.remote("release");

        let (code, said) = plane.save("work.md", "work.md");

        assert_eq!(code, 0, "{said}");
        assert_eq!(plane.remote("release"), release, "{said}");
        assert!(said.contains("save_branch"), "{said}");
        // V81 (#1087): prose names the `pr` and `pr-merge` modes "request mode" on every
        // forge, and the request by the glossary's word; the config words stay as written.
        assert!(said.contains("a request mode never pushes"), "{said}");
        assert!(!said.contains("PR mode"), "{said}");
        let line = plane.last_journal();
        assert_eq!(line["outcome"], "blocked");
        assert!(
            line["detail"]
                .as_str()
                .is_some_and(|why| why.contains("[plane] save_branch is release")),
            "{line}"
        );
    }

    #[test]
    fn a_later_edit_to_a_path_the_pr_touched_does_not_stop_a_squash_merge_from_settling() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("later-edit.test", "pr");
        plane.opened("work.md", 12);
        let at = merge(&plane, "squash");
        // Somebody else edits the same file after the merge, as MEMORY.md is edited all day.
        plane.their_commit("work.md", "rewritten by someone else");
        plane.merged(12, &at);

        planegit::fetch(&plane.root, true).expect("the fetch");

        let standing = plane.standing();
        assert_ne!(standing.stage, Stage::Blocked, "{standing:?}");
        assert_eq!(plane.head(), plane.remote("release"), "{standing:?}");
        assert_eq!(
            std::fs::read_to_string(plane.root.join("work.md")).unwrap(),
            "rewritten by someone else"
        );
    }

    #[test]
    fn a_file_in_the_way_of_the_move_waits_for_the_next_look_rather_than_blocking() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("in-the-way.test", "pr");
        plane.opened("work.md", 12);
        let head = plane.head();
        let at = merge(&plane, "squash");
        // An untracked file the remote's branch now has: `reset --keep` will not overwrite it.
        std::fs::write(plane.root.join("theirs.md"), "mine").unwrap();
        plane.merged(12, &at);

        planegit::fetch(&plane.root, true).expect("the fetch");

        let standing = plane.standing();
        assert_ne!(standing.stage, Stage::Blocked, "{standing:?}");
        assert_eq!(plane.head(), head, "nothing moved");
        assert_eq!(
            std::fs::read_to_string(plane.root.join("theirs.md")).unwrap(),
            "mine"
        );

        // Out of the way, the next look moves the plane.
        std::fs::remove_file(plane.root.join("theirs.md")).unwrap();
        let asked = plane.merged(12, &at);
        planegit::fetch(&plane.root, true).expect("the fetch");
        assert!(was_asked(&asked));
        assert_eq!(plane.head(), plane.remote("release"));
    }

    #[test]
    fn a_pr_that_merged_after_a_failed_save_is_settled_and_never_opened_again() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("no-duplicate.test", "pr");
        plane.opened("one.md", 12);
        // The next save pushes, and the forge then fails to answer.
        plane.state(12, true);
        plane.scene.gh_api(
            "repos/acme/plane/pulls?state=open&head=acme:charter%2Fsave%2Ftest&base=release&per_page=1",
            1,
            "",
            "HTTP 502: Bad Gateway",
        );
        let (code, said) = plane.save("two.md", "two.md");
        assert_eq!(code, 0, "{said}");
        assert!(said.contains("HTTP 502"), "{said}");
        let record = plane.record();
        assert_eq!(record["outcome"], "failed", "{record}");
        assert_eq!(
            record["landed"], SAVE,
            "pushed, with no request yet: {record}"
        );
        assert!(
            record["detail"]
                .as_str()
                .is_some_and(|why| why.contains("HTTP 502")),
            "{record}"
        );
        // PR 12 merges, carrying both commits.
        let at = merge(&plane, "squash");
        plane.merged(12, &at);
        plane.lookup(None);
        // Only the commit made since: nothing PR 12 already carried is proposed again.
        let created = plane.created(13, &["three.md"]);

        let (code, said) = plane.save("three.md", "three.md");

        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&created), "{said}");
        assert_eq!(
            git(&plane.root, &["rev-parse", "HEAD~1"]),
            plane.remote("release")
        );
    }

    #[test]
    fn a_pr_whose_state_the_forge_cannot_give_stops_the_save_before_it_pushes() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("state-fails.test", "pr");
        plane.opened("one.md", 12);
        let pushed = plane.remote(SAVE);
        plane
            .scene
            .gh_api("repos/acme/plane/pulls/12", 1, "", "HTTP 502: Bad Gateway");

        let (code, said) = plane.save("two.md", "two.md");

        assert_eq!(code, 0, "{said}");
        assert!(said.contains("HTTP 502"), "{said}");
        assert_eq!(plane.remote(SAVE), pushed, "nothing pushed: {said}");
        let line = plane.last_journal();
        assert_eq!(line["outcome"], "failed");
        assert!(
            line["detail"]
                .as_str()
                .is_some_and(|why| why.contains("HTTP 502")),
            "{line}"
        );
        let standing = plane.standing();
        assert_ne!(standing.stage, Stage::Blocked);
        // The PR this clone opened stays known through a save that failed after it.
        assert_eq!(standing.pr, Some(plane.url(12)), "{standing:?}");
    }

    #[test]
    fn a_save_with_nothing_to_commit_settles_a_merged_pr() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("clean-settle.test", "pr");
        plane.opened("work.md", 12);
        let at = merge(&plane, "squash");
        plane.merged(12, &at);

        let (code, said) = plane.save_as(None, Trigger::Cli);

        assert_eq!(code, 0, "{said}");
        assert_eq!(plane.head(), plane.remote("release"), "{said}");
        assert_eq!(plane.standing().stage, Stage::Saved, "{said}");
        assert_eq!(plane.standing().pr, None);
    }

    #[test]
    fn a_save_with_nothing_new_while_its_pr_carries_everything_names_that_pr() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("carried.test", "pr");
        plane.opened("work.md", 12);
        plane.state(12, true);

        let (code, said) = plane.save_as(None, Trigger::Cli);

        assert_eq!(code, 0, "{said}");
        assert!(said.contains("already carries all of it"), "{said}");
        let record = plane.record();
        assert_eq!(record["outcome"], "pr-open", "{record}");
        assert_eq!(record["landed"], SAVE, "{record}");
        assert_eq!(record["number"], 12, "{record}");
        assert_eq!(record["url"], plane.url(12).as_str(), "{record}");
    }

    #[test]
    fn a_push_to_the_save_branch_the_remote_refuses_is_recorded_in_its_words() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("refused-push.test", "pr");
        std::fs::create_dir_all(plane.bare.join("hooks")).unwrap();
        stand_in::program(
            &plane.bare,
            "hooks/pre-receive",
            "#!/bin/sh\necho 'pushes are closed today' >&2\nexit 1\n",
        );

        let (code, said) = plane.save("work.md", "work.md");

        assert_eq!(code, 0, "{said}");
        assert_eq!(
            git(&plane.bare, &["for-each-ref", "refs/heads/charter"]),
            "",
            "nothing landed: {said}"
        );
        let record = plane.record();
        assert_eq!(record["outcome"], "failed", "{record}");
        assert_eq!(record["landed"], SAVE, "{record}");
        assert!(
            record["detail"]
                .as_str()
                .is_some_and(|why| why.contains("pushes are closed today")),
            "{record}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_merged_pr_the_plane_cannot_move_onto_yet_is_said_in_the_saves_journal() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        // The merge brought `shelf/incoming.md`, and this clone has an empty `shelf` nothing may
        // be written into: the move waits, and a save with nothing to commit says why in its
        // journal line. Not as root, which writes into a mode-555 directory all the same.
        if rustix::process::geteuid().is_root() {
            return;
        }
        use std::os::unix::fs::PermissionsExt;
        /// Gives `shelf` its mode back however the test ends, so its temp dir can go.
        struct Restore(PathBuf);
        impl Drop for Restore {
            fn drop(&mut self) {
                let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
            }
        }
        let plane = plane("waits.test", "pr");
        plane.opened("work.md", 12);
        let at = merge(&plane, "squash");
        let theirs = plane.theirs();
        std::fs::create_dir_all(theirs.join("shelf")).unwrap();
        std::fs::write(theirs.join("shelf/incoming.md"), "incoming").unwrap();
        git(&theirs, &["add", "-A"]);
        git(&theirs, &["commit", "-q", "-m", "incoming"]);
        git(&theirs, &["push", "-q", "origin", "release"]);
        let shelf = plane.root.join("shelf");
        std::fs::create_dir(&shelf).unwrap();
        std::fs::set_permissions(&shelf, std::fs::Permissions::from_mode(0o555)).unwrap();
        let _restore = Restore(shelf.clone());
        plane.merged(12, &at);

        let (code, said) = plane.save_as(None, Trigger::Cli);

        assert_eq!(code, 0, "{said}");
        let line = plane.last_journal();
        assert!(
            line["detail"]
                .as_str()
                .is_some_and(|why| why.contains("once nothing here is in the way")),
            "{line} {said}"
        );
    }

    #[test]
    fn a_block_clears_once_the_plane_is_moved_off_the_commit_it_is_about() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("moved-off.test", "pr");
        plane.opened("work.md", 12);
        plane.state(12, false);
        planegit::fetch(&plane.root, true).expect("the fetch");
        assert_eq!(plane.standing().stage, Stage::Blocked);

        // The person settles it by hand: the plane back onto the remote's branch.
        git(
            &plane.root,
            &["reset", "-q", "--hard", "refs/remotes/origin/release"],
        );

        let standing = plane.standing();
        assert_ne!(standing.stage, Stage::Blocked, "{standing:?}");
        assert_eq!(standing.blocked, None, "{standing:?}");
    }

    #[test]
    fn a_clean_request_mode_plane_with_no_pr_known_has_nothing_to_save_or_ask() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        // Nothing written down for the stand-in: asking the forge anything fails the save.
        let plane = plane("at-rest.test", "pr");

        let (code, said) = plane.save_as(None, Trigger::Cli);

        assert_eq!(code, 0, "{said}");
        assert!(said.contains("Nothing to save"), "{said}");
        assert_eq!(plane.last_journal()["outcome"], "skipped");
    }

    #[test]
    fn a_save_branch_tip_this_clone_pushed_but_lost_the_note_of_is_pushed_over() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        // The note of what this clone last pushed is gone; the remote's tip is in HEAD's
        // history, so it is this clone's own and nothing of anybody's is lost.
        let plane = plane("lost-note.test", "pr");
        plane.opened("one.md", 12);
        std::fs::remove_file(purlis_core::plane::state_dir(&plane.root).join("save-branch.json"))
            .unwrap();
        plane.lookup(Some(12));
        let updated = plane.updated(12, &["one.md", "two.md"]);

        let (code, said) = plane.save("two.md", "two.md");

        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&updated), "{said}");
        assert_eq!(plane.remote(SAVE), plane.head(), "{said}");
        assert_eq!(plane.standing().stage, Stage::PrOpen, "{said}");
    }

    #[test]
    fn a_save_branch_tip_this_clone_pushed_before_its_history_was_rewritten_is_pushed_over() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        // The plane's history was rewritten after its last push — same trees, new dates, so
        // every commit has a new sha — and the note of that push is gone. The remote's tip is
        // no longer in HEAD's history, but every commit on it is one HEAD has under another
        // sha, so nothing of anybody's is lost (2026-10-13).
        let plane = plane("rewritten.test", "pr");
        plane.opened("one.md", 12);
        let before = plane.remote(SAVE);
        git(
            &plane.root,
            &[
                "commit",
                "-q",
                "--amend",
                "--no-edit",
                "--date=2001-02-03T04:05:06Z",
            ],
        );
        assert_ne!(
            plane.head(),
            before,
            "the rewrite gave the commit a new sha"
        );
        std::fs::remove_file(purlis_core::plane::state_dir(&plane.root).join("save-branch.json"))
            .unwrap();
        plane.lookup(Some(12));
        let updated = plane.updated(12, &["one.md", "two.md"]);

        let (code, said) = plane.save("two.md", "two.md");

        assert_eq!(code, 0, "{said}");
        assert!(!said.contains("did not push there"), "{said}");
        assert!(was_asked(&updated), "{said}");
        assert_eq!(plane.remote(SAVE), plane.head(), "{said}");
        assert_eq!(plane.standing().stage, Stage::PrOpen, "{said}");
    }

    #[test]
    fn a_request_carrying_more_saves_than_it_lists_says_how_many_earlier_ones_it_left_out() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("many.test", "pr");
        for n in 1..=51 {
            std::fs::write(plane.root.join("log.md"), n.to_string()).unwrap();
            git(&plane.root, &["add", "-A"]);
            git(&plane.root, &["commit", "-q", "-m", &format!("save {n}")]);
        }
        let host = plane.scene.host.clone();
        let machine = purlis_core::dispatch::host();
        let mut body = format!(
            "body=Saved by charter on {machine}. Every save from this machine pushes to {SAVE} \
             and updates this pull request.\n\n- … and 1 earlier"
        );
        for n in 2..=51 {
            body.push_str(&format!("\n- save {n}"));
        }
        plane.lookup(None);
        let created = plane.scene.answers(
            "gh",
            &[
                "api",
                "--hostname",
                &host,
                "-X",
                "POST",
                "repos/acme/plane/pulls",
                "-f",
                &format!("head={SAVE}"),
                "-f",
                "base=release",
                "-f",
                &format!("title=51 saves from {machine}"),
                "-f",
                &body,
            ],
            0,
            &format!(r#"{{"number": 12, "html_url": "{}"}}"#, plane.url(12)),
            "",
        );

        let (code, said) = plane.save_as(None, Trigger::Cli);

        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&created), "{said}");
    }

    #[test]
    fn a_merge_the_forge_names_at_a_commit_not_on_the_target_blocks_the_plane() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        // The forge says PR 12 merged as the save branch's own commit, which `release` on the
        // remote does not hold: nothing is moved on its word.
        let plane = plane("not-on-target.test", "pr");
        plane.opened("work.md", 12);
        let head = plane.head();
        plane.merged(12, &head);

        planegit::fetch(&plane.root, true).expect("the fetch");

        let standing = plane.standing();
        assert_eq!(standing.stage, Stage::Blocked, "{standing:?}");
        assert!(
            standing
                .blocked
                .as_deref()
                .is_some_and(|why| why.contains("which is not on release")),
            "{standing:?}"
        );
        assert_eq!(plane.head(), head);
    }

    /// The open-request lookup from `head`, answering request `open` or none.
    fn lookup_from(plane: &Plane, head: &str, open: Option<u64>) -> PathBuf {
        let out = match open {
            Some(n) => format!(r#"[{{"number": {n}, "html_url": "{}"}}]"#, plane.url(n)),
            None => "[]".to_string(),
        };
        plane.scene.gh_api(
            &format!(
                "repos/acme/plane/pulls?state=open&head=acme:{}&base=release&per_page=1",
                head.replace('/', "%2F")
            ),
            0,
            &out,
            "",
        )
    }

    /// Request `number` from `head` written: `method` is `POST` to open it, `PATCH` to update it.
    fn written_from(
        plane: &Plane,
        method: &str,
        head: &str,
        number: u64,
        subjects: &[&str],
    ) -> PathBuf {
        let host = plane.scene.host.clone();
        let (path, mut argv) = match method {
            "POST" => (
                "repos/acme/plane/pulls".to_string(),
                vec![
                    "-f".to_string(),
                    format!("head={head}"),
                    "-f".into(),
                    "base=release".into(),
                ],
            ),
            _ => (format!("repos/acme/plane/pulls/{number}"), vec![]),
        };
        argv.extend([
            "-f".to_string(),
            format!("title={}", title(subjects)),
            "-f".into(),
            format!("body={}", body(subjects).replace(SAVE, head)),
        ]);
        let mut args = vec!["api", "--hostname", &host, "-X", method, &path];
        args.extend(argv.iter().map(String::as_str));
        plane.scene.answers(
            "gh",
            &args,
            0,
            &format!(
                r#"{{"number": {number}, "html_url": "{}"}}"#,
                plane.url(number)
            ),
            "",
        )
    }

    /// A plane naming no save_branch, whose PR 12 was opened before the rename from this clone's
    /// default as it was then, `charter/save/<host>-<clone>`: answers it, that branch, and the
    /// purlis name of the same default.
    fn opened_before_the_rename(host: &str) -> (Plane, String, String) {
        // V93j. The plane names no save_branch, so its default is this clone's own. Before the
        // rename it was `charter/save/<host>-<clone>`, and PR 12 is open from there.
        let plane = plane(host, "pr");
        let manifest = std::fs::read_to_string(plane.root.join("charter.toml")).unwrap();
        std::fs::write(
            plane.root.join("charter.toml"),
            manifest.replace(&format!("save_branch = \"{SAVE}\"\n"), ""),
        )
        .unwrap();
        git(&plane.root, &["commit", "-q", "-am", "no save_branch"]);
        git(
            &plane.root,
            &[
                "push",
                "-q",
                &plane.bare.display().to_string(),
                "HEAD:release",
            ],
        );
        git(
            &plane.root,
            &["update-ref", "refs/remotes/origin/release", "HEAD"],
        );
        let rest = purlis_core::planesave::default_rest(&plane.root);
        let (old, new) = (format!("charter/{rest}"), format!("purlis/{rest}"));
        std::fs::write(plane.root.join("work.md"), "work.md").unwrap();
        git(&plane.root, &["add", "-A"]);
        git(&plane.root, &["commit", "-q", "-m", "work.md"]);
        git(
            &plane.root,
            &[
                "push",
                "-q",
                &plane.bare.display().to_string(),
                &format!("HEAD:refs/heads/{old}"),
            ],
        );
        let pushed = plane.head();
        let state = plane.root.join(".charter");
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(
            state.join("save-branch.json"),
            serde_json::json!({
                "branch": old,
                "pushed": pushed,
                "pr": {"number": 12, "url": plane.url(12), "head": pushed, "target": "release"},
            })
            .to_string(),
        )
        .unwrap();

        (plane, old, new)
    }

    #[test]
    fn a_request_from_the_charter_save_branch_closed_without_merging_blocks_until_the_next_save() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        // V93j: the block about the pre-rename request holds, as any block does, until a
        // person saves again — and that save opens the new request from the purlis name.
        let (plane, old, new) = opened_before_the_rename("renamed-closed.test");
        plane.state(12, false);

        planegit::fetch(&plane.root, true).expect("the fetch");

        let standing = plane.standing();
        assert_eq!(standing.stage, Stage::Blocked, "{standing:?}");
        assert!(
            standing
                .blocked
                .as_deref()
                .is_some_and(|why| why.contains("closed without merging")),
            "{standing:?}"
        );
        assert_eq!(plane.record()["landed"], old.as_str());

        lookup_from(&plane, &new, None);
        let created = written_from(&plane, "POST", &new, 13, &["work.md"]);
        let (code, said) = plane.save_as(None, Trigger::Manual);
        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&created), "{said}");
        assert_eq!(plane.standing().pr, Some(plane.url(13)));
    }

    #[test]
    fn a_request_from_the_charter_save_branch_forgotten_by_a_reset_is_never_opened_twice() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        // A person moves the plane off PR 12's commit, so the next settle forgets PR 12 — but
        // nothing has seen it merge or close, and it may still be open. The save goes on from
        // the old branch, where the forge's lookup finds PR 12 and updates it.
        let (plane, old, new) = opened_before_the_rename("renamed-reset.test");
        git(&plane.root, &["reset", "-q", "--hard", "HEAD~1"]);
        // The background look settles first, and forgets PR 12 without asking the forge.
        planegit::fetch(&plane.root, true).expect("the fetch");
        lookup_from(&plane, &old, Some(12));
        let updated = written_from(&plane, "PATCH", &old, 12, &["again.md"]);

        let (code, said) = plane.save("again.md", "again.md");

        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&updated), "{said}");
        assert_eq!(plane.remote(&old), plane.head(), "{said}");
        assert_eq!(
            git(&plane.bare, &["for-each-ref", "refs/heads/purlis/"]),
            "",
            "no second request from {new}: {said}"
        );
    }

    #[test]
    fn a_request_open_from_the_charter_save_branch_carries_on_there_until_it_merges() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let (plane, old, new) = opened_before_the_rename("renamed-prefix.test");

        // While it is open, the next save updates it from where it is.
        plane.state(12, true);
        lookup_from(&plane, &old, Some(12));
        let updated = written_from(&plane, "PATCH", &old, 12, &["work.md", "more.md"]);
        let (code, said) = plane.save("more.md", "more.md");
        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&updated), "{said}");
        assert_eq!(plane.remote(&old), plane.head(), "{said}");
        assert_eq!(
            git(&plane.bare, &["for-each-ref", "refs/heads/purlis/"]),
            "",
            "no purlis branch while it is open: {said}"
        );

        // Once it has merged, the next save opens its request from the purlis name.
        plane.their_commit("theirs.md", "theirs");
        let theirs = plane.theirs();
        git(
            &theirs,
            &[
                "merge",
                "-q",
                "--no-ff",
                "-m",
                "Merge pull request #12",
                &format!("origin/{old}"),
            ],
        );
        git(&theirs, &["push", "-q", "origin", "release"]);
        let at = git(&theirs, &["rev-parse", "HEAD"]);
        plane.merged(12, &at);
        lookup_from(&plane, &new, None);
        let created = written_from(&plane, "POST", &new, 13, &["later.md"]);
        let (code, said) = plane.save("later.md", "later.md");
        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&created), "{said}");
        assert_eq!(plane.remote(&new), plane.head(), "{said}");
    }

    #[test]
    fn a_save_branch_changed_by_hand_is_pushed_and_given_its_own_pr() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        // What was kept about the old save branch — its lease and its open PR — says nothing
        // about the new one: the save pushes it, rather than calling PR 12 enough.
        let plane = plane("renamed-save.test", "pr");
        plane.opened("work.md", 12);
        std::fs::create_dir_all(plane.root.join(".git/info")).unwrap();
        std::fs::write(plane.root.join(".git/info/exclude"), "charter.local.toml\n").unwrap();
        let next = "charter/save/next";
        std::fs::write(
            plane.root.join("charter.local.toml"),
            format!("[plane]\nsave_branch = \"{next}\"\n"),
        )
        .unwrap();
        plane.state(12, true);
        plane.scene.gh_api(
            "repos/acme/plane/pulls?state=open&head=acme:charter%2Fsave%2Fnext&base=release&per_page=1",
            0,
            "[]",
            "",
        );
        let host = plane.scene.host.clone();
        let created = plane.scene.answers(
            "gh",
            &[
                "api",
                "--hostname",
                &host,
                "-X",
                "POST",
                "repos/acme/plane/pulls",
                "-f",
                &format!("head={next}"),
                "-f",
                "base=release",
                "-f",
                "title=work.md",
                "-f",
                &format!("body={}", body(&["work.md"]).replace(SAVE, next)),
            ],
            0,
            &format!(r#"{{"number": 13, "html_url": "{}"}}"#, plane.url(13)),
            "",
        );

        let (code, said) = plane.save_as(None, Trigger::Cli);

        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&created), "{said}");
        assert_eq!(plane.remote(next), plane.head(), "{said}");
    }

    #[test]
    fn a_second_machine_with_the_same_save_branch_never_overwrites_the_firsts() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let plane = plane("same-host.test", "pr");
        plane.opened("mine.md", 12);
        let first = plane.remote(SAVE);
        // Another clone of the same plane, on a machine with the same name.
        let other = plane.root.parent().unwrap().join("other");
        git(
            plane.root.parent().unwrap(),
            &[
                "clone",
                "-q",
                &plane.bare.display().to_string(),
                &other.display().to_string(),
            ],
        );
        git(&other, &["config", "user.name", "Other"]);
        git(&other, &["config", "user.email", "other@example.invalid"]);
        let host = plane.scene.host.clone();
        git(
            &other,
            &[
                "remote",
                "set-url",
                "origin",
                &format!("git@{host}:acme/plane.git"),
            ],
        );
        git(
            &other,
            &[
                "config",
                &format!(
                    "url.file://{}/.insteadOf",
                    plane.bare.parent().unwrap().display()
                ),
                &format!("https://{host}/acme/"),
            ],
        );
        std::fs::write(other.join("theirs.md"), "theirs").unwrap();
        let mut said = String::new();
        let code = planegit::save_as(
            &Request {
                root: &other,
                message: Some("theirs.md"),
                sign: false,
                no_push: false,
                cwd: &other,
                provenance: None,
            },
            Trigger::Manual,
            &mut |line: Say| said.push_str(&format!("{line}\n")),
        );

        assert_eq!(code, 0, "{said}");
        assert_eq!(
            plane.remote(SAVE),
            first,
            "the first machine's commits stay: {said}"
        );
        assert_eq!(planegit::standing(&other).stage, Stage::Blocked, "{said}");

        // Fixed by hand, as the block says: a save branch of this clone's own, in its local
        // file. The block is gone at once, before any save runs.
        std::fs::write(other.join(".gitignore"), ".charter/\ncharter.local.toml\n").unwrap();
        git(&other, &["commit", "-q", "-am", "ignore the local file"]);
        std::fs::write(
            other.join("charter.local.toml"),
            "[plane]\nsave_branch = \"charter/save/other\"\n",
        )
        .unwrap();
        assert_ne!(planegit::standing(&other).stage, Stage::Blocked);
    }

    /// A plane whose origin is on a GitLab: its saves name a merge request as GitLab does,
    /// `!12`, and a GitHub's saves keep their words (#1067).
    mod on_gitlab {
        use super::*;

        const MRS: &str = "projects/acme%2Fplane/merge_requests";

        fn url(plane: &Plane, number: u64) -> String {
            format!(
                "https://{}/acme/plane/-/merge_requests/{number}",
                plane.scene.host
            )
        }

        fn body(subjects: &[&str]) -> String {
            let mut out = format!(
                "Saved by charter on {}. Every save from this machine pushes to {SAVE} and \
                 updates this merge request.\n",
                purlis_core::dispatch::host()
            );
            for subject in subjects {
                out.push_str(&format!("\n- {subject}"));
            }
            out
        }

        /// A save of `file` that opens MR `number` from the save branch.
        fn opened(plane: &Plane, file: &str, number: u64) -> String {
            plane.scene.glab_api(
                &format!(
                    "{MRS}?state=opened&source_branch=charter%2Fsave%2Ftest&target_branch=release&per_page=100"
                ),
                0,
                "[]",
                "",
            );
            let host = plane.scene.host.clone();
            let created = plane.scene.answers(
                "glab",
                &[
                    "--hostname",
                    &host,
                    "api",
                    "-X",
                    "POST",
                    MRS,
                    "-f",
                    &format!("source_branch={SAVE}"),
                    "-f",
                    "target_branch=release",
                    "-f",
                    &format!("title={}", title(&[file])),
                    "-f",
                    &format!("description={}", body(&[file])),
                ],
                0,
                &format!(
                    r#"{{"iid": {number}, "id": 9000, "web_url": "{}"}}"#,
                    url(plane, number)
                ),
                "",
            );
            let (code, said) = plane.save(file, file);
            assert_eq!(code, 0, "{said}");
            assert!(was_asked(&created), "{said}");
            said
        }

        /// MR `number` stands in `state`, as GitLab's record says it.
        fn stands(plane: &Plane, number: u64, record: &str) -> PathBuf {
            plane
                .scene
                .glab_api(&format!("{MRS}/{number}"), 0, record, "")
        }

        #[test]
        fn a_save_opens_a_merge_request_and_says_so_in_gitlabs_words() {
            purlis_core::unsteered!();
            if !in_child() {
                return;
            }
            let plane = plane_on("gitlab", "mr-words-open.test", "pr");

            let said = opened(&plane, "work.md", 12);

            assert!(
                said.contains(&format!("Merge request into release: {}", url(&plane, 12))),
                "{said}"
            );
            assert!(!said.to_lowercase().contains("pull request"), "{said}");
            assert_eq!(plane.standing().stage, Stage::PrOpen);
        }

        #[test]
        fn a_merge_request_closed_without_merging_blocks_the_plane_naming_it_bang_12() {
            purlis_core::unsteered!();
            if !in_child() {
                return;
            }
            let plane = plane_on("gitlab", "mr-words-closed.test", "pr");
            opened(&plane, "work.md", 12);
            stands(&plane, 12, r#"{"iid": 12, "state": "closed"}"#);

            planegit::fetch(&plane.root, true).expect("the fetch");

            let blocked = plane.standing().blocked.unwrap_or_default();
            assert!(
                blocked.starts_with("merge request !12 was closed without merging"),
                "{blocked}"
            );
            assert!(
                blocked.ends_with("save again to open a new merge request"),
                "{blocked}"
            );
        }

        #[test]
        fn a_merged_merge_request_is_named_bang_12_when_the_plane_moves_onto_it() {
            purlis_core::unsteered!();
            if !in_child() {
                return;
            }
            let plane = plane_on("gitlab", "mr-words-merged.test", "pr");
            opened(&plane, "work.md", 12);
            let at = merge(&plane, "squash");
            stands(
                &plane,
                12,
                &format!(r#"{{"iid": 12, "state": "merged", "squash_commit_sha": "{at}"}}"#),
            );

            let (code, said) = plane.save_as(None, Trigger::Cli);

            assert_eq!(code, 0, "{said}");
            assert!(
                said.contains("Nothing new to commit — asking after the open merge request."),
                "{said}"
            );
            assert!(
                said.contains("Merge request !12 merged — release is now the remote's."),
                "{said}"
            );
            assert_eq!(plane.standing().stage, Stage::Saved, "{said}");
        }

        #[test]
        fn a_merge_request_gitlab_cannot_report_on_is_named_bang_12() {
            purlis_core::unsteered!();
            if !in_child() {
                return;
            }
            let plane = plane_on("gitlab", "mr-words-unknown.test", "pr");
            opened(&plane, "one.md", 12);
            plane
                .scene
                .glab_api(&format!("{MRS}/12"), 1, "", "HTTP 502: Bad Gateway");

            let (code, said) = plane.save("two.md", "two.md");

            assert_eq!(code, 0, "{said}");
            assert!(
                said.contains("purlis could not ask where merge request !12 stands"),
                "{said}"
            );
            assert!(
                said.contains("whether the last merge request merged"),
                "{said}"
            );
        }

        /// A `pr-merge` plane on a GitLab with one commit to save, and the lookup and the
        /// create of !12 written down.
        fn pr_merge_plane(host: &str) -> (Plane, String) {
            let plane = plane_on("gitlab", host, "pr-merge");
            std::fs::write(plane.root.join("work.md"), "work").unwrap();
            git(&plane.root, &["add", "-A"]);
            git(&plane.root, &["commit", "-q", "-m", "work.md"]);
            let head = plane.head();
            plane.scene.glab_api(
                &format!(
                    "{MRS}?state=opened&source_branch=charter%2Fsave%2Ftest&target_branch=release&per_page=100"
                ),
                0,
                "[]",
                "",
            );
            let host = plane.scene.host.clone();
            plane.scene.answers(
                "glab",
                &[
                    "--hostname",
                    &host,
                    "api",
                    "-X",
                    "POST",
                    MRS,
                    "-f",
                    &format!("source_branch={SAVE}"),
                    "-f",
                    "target_branch=release",
                    "-f",
                    &format!("title={}", title(&["work.md"])),
                    "-f",
                    &format!("description={}", body(&["work.md"])),
                ],
                0,
                &format!(
                    r#"{{"iid": 12, "id": 9000, "web_url": "{}"}}"#,
                    url(&plane, 12)
                ),
                "",
            );
            (plane, head)
        }

        /// What one look at !12 reads, written down once: that it is open, how the repo
        /// merges (`settings`), the merge request at `head`, and its `pipelines`.
        fn looked(plane: &Plane, head: &str, settings: &str, pipelines: &str) {
            stands(plane, 12, r#"{"iid": 12, "state": "opened"}"#);
            plane
                .scene
                .glab_api("projects/acme%2Fplane", 0, settings, "");
            stands(
                plane,
                12,
                &format!(
                    r#"{{"iid": 12, "state": "opened", "draft": false, "sha": "{head}",
                    "source_branch": "charter/save/test", "target_branch": "release",
                    "source_project_id": 1, "target_project_id": 1, "description": "",
                    "updated_at": "2026-01-01T00:00:00.000Z",
                    "detailed_merge_status": "mergeable", "head_pipeline": null}}"#
                ),
            );
            plane.scene.glab_api(
                &format!("{MRS}/12/pipelines?per_page=100"),
                0,
                pipelines,
                "",
            );
        }

        /// A save on a GitLab asks for no auto-merge and merges nothing; a look once the head
        /// is settled finds no pipeline on the merge request and none in the repository for
        /// anything but the target branch, merges it at the pushed commit, and the plane moves
        /// onto it.
        #[test]
        fn a_pr_merge_save_on_gitlab_waits_at_the_save_and_is_merged_by_a_later_look() {
            purlis_core::unsteered!();
            if !in_child() {
                return;
            }
            let (plane, head) = pr_merge_plane("mr-words-automerge.test");

            // Nothing about merging is written down yet: the save asks none of it.
            let (code, said) = plane.save_as(None, Trigger::Manual);

            assert_eq!(code, 0, "{said}");
            assert!(said.contains("not had time to start"), "{said}");
            assert!(
                said.contains("purlis merges merge request !12 itself"),
                "{said}"
            );
            assert!(!said.to_lowercase().contains("pull request"), "{said}");
            assert_eq!(plane.standing().stage, Stage::PrOpen);

            // A minute on: open, mergeable, no pipeline, and the repository's own pipelines
            // all ran on release.
            the_settle_wait_is_over(&plane);
            looked(&plane, &head, r#"{"squash_option": "default_off"}"#, "[]");
            let newest = plane.scene.glab_api(
                "projects/acme%2Fplane/pipelines?per_page=20",
                0,
                r#"[{"id": 5, "ref": "release", "status": "success"}]"#,
                "",
            );
            let host = plane.scene.host.clone();
            let put = plane.scene.answers(
                "glab",
                &[
                    "--hostname",
                    &host,
                    "api",
                    "-X",
                    "PUT",
                    &format!("{MRS}/12/merge"),
                    "-F",
                    "squash=false",
                    "-f",
                    &format!("sha={head}"),
                ],
                0,
                r#"{"iid": 12, "state": "merged"}"#,
                "",
            );
            let at = merge(&plane, "merge");
            stands(
                &plane,
                12,
                &format!(r#"{{"iid": 12, "state": "merged", "merge_commit_sha": "{at}"}}"#),
            );

            planegit::fetch(&plane.root, true).expect("the fetch");

            assert!(
                was_asked(&newest),
                "whether the repo runs pipelines was not asked"
            );
            assert!(was_asked(&put), "merged at the pushed commit");
            assert_eq!(
                plane.head(),
                plane.remote("release"),
                "moved onto what merged"
            );
        }

        /// A repo with merge trains (#805) is never merged directly, and never handed to GitLab
        /// to wait on its pipeline: once purlis reads the pipeline at the pushed commit as
        /// passed, it adds the merge request to the train, and asks nothing more about merging.
        #[test]
        fn a_pr_merge_save_on_a_merge_train_joins_the_train_once_its_pipeline_passed() {
            purlis_core::unsteered!();
            if !in_child() {
                return;
            }
            let (plane, head) = pr_merge_plane("mr-words-train.test");
            let (code, said) = plane.save_as(None, Trigger::Manual);
            assert_eq!(code, 0, "{said}");
            the_settle_wait_is_over(&plane);
            let train = r#"{"squash_option": "default_off", "merge_trains_enabled": true}"#;
            looked(
                &plane,
                &head,
                train,
                &format!(r#"[{{"id": 77, "sha": "{head}", "status": "success"}}]"#),
            );
            let host = plane.scene.host.clone();
            let joined = plane.scene.answers(
                "glab",
                &[
                    "--hostname",
                    &host,
                    "api",
                    "-X",
                    "POST",
                    "projects/acme%2Fplane/merge_trains/merge_requests/12",
                    "-F",
                    "squash=false",
                    "-f",
                    &format!("sha={head}"),
                ],
                0,
                r#"[{"id": 267, "merge_request": {"iid": 12}, "status": "idle"}]"#,
                "",
            );

            let found = look(&plane);

            assert!(was_asked(&joined), "not added to the train");
            assert!(found.open && found.changed && !found.moved, "{found:?}");
            let standing = plane.standing();
            assert_eq!(standing.stage, Stage::PrOpen);
            let merging = standing.merge.expect("where the request stands");
            assert_eq!(merging.state, planegit::MergeState::Queued, "{merging:?}");
            assert!(merging.why.contains("merge train"), "{merging:?}");

            // On the train, it is GitLab's to merge: the next look reads where it stands and
            // asks nothing about merging it, so nothing else needs writing down.
            stands(&plane, 12, r#"{"iid": 12, "state": "opened"}"#);
            let found = look(&plane);
            assert!(found.open && !found.changed, "{found:?}");
        }
    }

    /// The same messages on a GitHub keep the words they had before #1067.
    mod on_github {
        use super::*;

        #[test]
        fn a_save_opens_a_pull_request_and_says_so_in_githubs_words() {
            purlis_core::unsteered!();
            if !in_child() {
                return;
            }
            let plane = plane("pr-words-open.test", "pr");

            let said = plane.opened("work.md", 12);

            assert!(
                said.contains(&format!("Pull request into release: {}", plane.url(12))),
                "{said}"
            );
        }

        #[test]
        fn a_pull_request_closed_without_merging_blocks_the_plane_naming_it_hash_12() {
            purlis_core::unsteered!();
            if !in_child() {
                return;
            }
            let plane = plane("pr-words-closed.test", "pr");
            plane.opened("work.md", 12);
            plane.state(12, false);

            planegit::fetch(&plane.root, true).expect("the fetch");

            let blocked = plane.standing().blocked.unwrap_or_default();
            assert!(
                blocked.starts_with("pull request #12 was closed without merging"),
                "{blocked}"
            );
            assert!(
                blocked.ends_with("save again to open a new pull request"),
                "{blocked}"
            );
        }

        #[test]
        fn a_merged_pull_request_is_named_hash_12_when_the_plane_moves_onto_it() {
            purlis_core::unsteered!();
            if !in_child() {
                return;
            }
            let plane = plane("pr-words-merged.test", "pr");
            plane.opened("work.md", 12);
            let at = merge(&plane, "squash");
            plane.merged(12, &at);

            let (_, said) = plane.save_as(None, Trigger::Cli);

            assert!(
                said.contains("Nothing new to commit — asking after the open pull request."),
                "{said}"
            );
            assert!(
                said.contains("Pull request #12 merged — release is now the remote's."),
                "{said}"
            );
        }
    }
}
