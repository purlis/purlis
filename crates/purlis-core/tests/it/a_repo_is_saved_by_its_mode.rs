// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A workspace repo saved in the pull request modes (charter-app#299, ADR 0051): pushed to the
//! branch it is on — or, on the default branch, to `charter/<workspace>/<short-sha>` — and a
//! pull request opened or updated from it.
//!
//! The push reaches a bare repository in the test's own directory through an `insteadOf` in
//! the clone's config, keyed on the HTTPS base charter builds (the arrangement
//! `planegit::origin_https` documents). Every forge question goes to a stand-in `gh` that
//! answers only what a test wrote down, exactly once (`support/forge_cli.rs`). Nothing here
//! reaches a network.

use crate::support;

use std::path::{Path, PathBuf};

use purlis_core::planegit::{self, Stage, Trigger};
use purlis_core::repocmd::Say;
use purlis_core::reposave::{self, Request};
use support::forge_cli::{Scene, in_a_child, in_child, was_asked};

#[test]
fn every_repo_save_question_is_asked_of_a_stand_in_cli() {
    purlis_core::unsteered!();
    in_a_child("a_repo_is_saved_by_its_mode::repo_saves::", "bin");
}

mod repo_saves {
    use super::*;

    struct Repo {
        plane: PathBuf,
        clone: PathBuf,
        bare: PathBuf,
        _fixture: support::Fixture,
    }

    /// A plane declaring `host` as a GitHub, saying `toml` about `widget`, whose inventory
    /// names `main` as widget's default branch; and a clone `alpha/widget` on `main`, with its
    /// one commit on the stand-in remote.
    fn repo(host: &str, toml: &str) -> Repo {
        repo_on("github", host, toml)
    }

    /// [`repo`], with `host` declared as a forge of `kind`.
    fn repo_on(kind: &str, host: &str, toml: &str) -> Repo {
        let f = support::plane_with_clone("widget");
        assert_eq!((f.ws.as_str(), f.repo.as_str()), ("alpha", "widget"));
        std::fs::write(
            f.plane.join("charter.toml"),
            format!("[[forge]]\nkind = \"{kind}\"\nhost = \"{host}\"\n\n{toml}"),
        )
        .unwrap();
        std::fs::create_dir_all(f.plane.join("inventory")).unwrap();
        std::fs::write(
            f.plane.join("inventory/repos.json"),
            r#"{"repos": [{"name": "widget", "default_branch": "main"}]}"#,
        )
        .unwrap();
        let clone = f.clone.clone();
        support::git(&clone, &["config", "user.name", "charter tests"]);
        support::git(&clone, &["config", "user.email", "tests@example.invalid"]);
        // Inside the fixture's own directory, which goes when the test does.
        let bare = f.plane.join(".test-forge/acme/widget.git");
        std::fs::create_dir_all(&bare).unwrap();
        support::git(&bare, &["init", "-q", "--bare", "-b", "main", "."]);
        support::git(
            &clone,
            &[
                "remote",
                "add",
                "origin",
                &format!("git@{host}:acme/widget.git"),
            ],
        );
        let base = format!("file://{}/", bare.parent().unwrap().display());
        support::git(
            &clone,
            &[
                "config",
                &format!("url.{base}.insteadOf"),
                &format!("https://{host}/acme/"),
            ],
        );
        let at = bare.display().to_string();
        support::git(&clone, &["push", "-q", &at, "main"]);
        support::git(
            &clone,
            &["fetch", "-q", &at, "+refs/heads/*:refs/remotes/origin/*"],
        );
        Repo {
            plane: f.plane.clone(),
            clone,
            bare,
            _fixture: f,
        }
    }

    impl Repo {
        fn save(&self) -> (u8, String) {
            let mut said = String::new();
            let mut say = |line: Say| {
                said.push_str(&line.to_string());
                said.push('\n');
            };
            let code = reposave::save_as(
                &Request {
                    plane: &self.plane,
                    workspace: "alpha",
                    name: "widget",
                    clone: &self.clone,
                    message: None,
                    no_push: false,
                    mid_turn: &reposave::nobody_working,
                },
                Trigger::Manual,
                &mut say,
            );
            (code, said)
        }

        fn head(&self) -> String {
            rev(&self.clone, "HEAD")
        }

        fn remote(&self, branch: &str) -> String {
            rev(&self.bare, &format!("refs/heads/{branch}"))
        }

        fn commit(&self, file: &str, subject: &str) {
            std::fs::write(self.clone.join(file), subject).unwrap();
            support::git(&self.clone, &["add", "-A"]);
            support::git(&self.clone, &["commit", "-q", "-m", subject]);
        }

        fn standing(&self) -> reposave::Standing {
            reposave::standing(
                &self.plane,
                "alpha",
                &purlis_core::repos::Repo {
                    name: "widget".into(),
                    path: self.clone.clone(),
                },
            )
        }

        fn last(&self) -> serde_json::Value {
            planegit::journal(&self.plane)
                .pop()
                .expect("a journal line")
        }
    }

    fn rev(dir: &Path, what: &str) -> String {
        String::from_utf8(support::git(dir, &["rev-parse", what]).stdout)
            .unwrap()
            .trim()
            .to_string()
    }

    const BODY: &str = "body=Saved by charter from the alpha workspace ([repos.widget] mode = pr).\n\n<!-- purlis-save -->";

    fn lookup(scene: &Scene, head: &str, out: &str) -> PathBuf {
        scene.gh_api(
            &format!(
                "repos/acme/widget/pulls?state=open&head=acme:{}&base=main&per_page=1",
                head.replace('/', "%2F")
            ),
            0,
            out,
            "",
        )
    }

    fn create(scene: &Scene, head: &str, title: &str, body: &str, number: u32) -> PathBuf {
        scene.answers(
            "gh",
            &[
                "api",
                "--hostname",
                &scene.host,
                "-X",
                "POST",
                "repos/acme/widget/pulls",
                "-f",
                &format!("head={head}"),
                "-f",
                "base=main",
                "-f",
                &format!("title={title}"),
                "-f",
                body,
            ],
            0,
            &format!(
                r#"{{"number": {number}, "html_url": "https://{}/acme/widget/pull/{number}"}}"#,
                scene.host
            ),
            "",
        )
    }

    #[test]
    fn a_feature_branch_in_pr_mode_is_pushed_as_itself_and_a_pr_opened_into_the_default() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let host = "repo-feature.test";
        let scene = Scene::new(host);
        let r = repo(host, "[repos.widget]\nmode = \"pr\"\n");
        let main_before = r.remote("main");
        support::git(&r.clone, &["checkout", "-q", "-b", "feature/x"]);
        std::fs::write(r.clone.join("src.rs"), "fn main() {}\n").unwrap();
        let looked = lookup(&scene, "feature/x", "[]");
        let opened = create(
            &scene,
            "feature/x",
            "charter save: 1 file (src.rs)",
            BODY,
            3,
        );

        let (code, said) = r.save();

        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&looked) && was_asked(&opened), "{said}");
        assert!(
            said.contains(&format!(
                "Pull request #3 from feature/x into main: https://{host}/acme/widget/pull/3"
            )),
            "{said}"
        );
        assert_eq!(r.remote("feature/x"), r.head());
        assert_eq!(r.remote("main"), main_before, "main was pushed");
        let line = r.last();
        assert_eq!(line["target"], "repo:alpha/widget");
        assert_eq!(line["mode"], "pr");
        assert_eq!(line["outcome"], "pr-open");
        assert_eq!(line["branch"], "feature/x");
        assert_eq!(line["pr"], format!("https://{host}/acme/widget/pull/3"));
        let standing = r.standing();
        assert_eq!(standing.stage, Stage::PrOpen);
        assert_eq!(
            standing.pr,
            Some(format!("https://{host}/acme/widget/pull/3"))
        );
    }

    #[test]
    fn on_the_default_branch_pr_mode_pushes_a_branch_of_its_own_and_never_main() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let host = "repo-default.test";
        let scene = Scene::new(host);
        let r = repo(host, "[repos.widget]\nmode = \"pr\"\n");
        let main_before = r.remote("main");
        r.commit("a.md", "first change");
        let short = &r.head()[..7];
        let branch = format!("purlis/alpha/{short}");
        let first_lookup = lookup(&scene, &branch, "[]");
        let opened = create(&scene, &branch, "first change", BODY, 4);

        let (code, said) = r.save();

        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&opened), "{said}");
        assert_eq!(r.remote(&branch), r.head());
        assert_eq!(r.remote("main"), main_before, "main was pushed: {said}");
        let on = String::from_utf8(
            support::git(&r.clone, &["rev-parse", "--abbrev-ref", "HEAD"]).stdout,
        )
        .unwrap();
        assert_eq!(on.trim(), "main", "the clone was moved off main");
        assert_eq!(rev(&r.clone, &format!("refs/heads/{branch}")), r.head());
        assert_eq!(r.last()["branch"], branch.as_str());

        // The next save from main carries on in the same branch, and updates the same PR. The
        // stand-in answers a question once, and this lookup is the same question again, so
        // the first one's is taken down.
        std::fs::remove_file(first_lookup.with_extension("args")).unwrap();
        r.commit("b.md", "second change");
        let found =
            format!(r#"[{{"number": 4, "html_url": "https://{host}/acme/widget/pull/4"}}]"#);
        lookup(&scene, &branch, &found);
        let updated = scene.answers(
            "gh",
            &[
                "api",
                "--hostname",
                host,
                "-X",
                "PATCH",
                "repos/acme/widget/pulls/4",
                "-f",
                "title=second change",
                "-f",
                BODY,
            ],
            0,
            &format!(r#"{{"number": 4, "html_url": "https://{host}/acme/widget/pull/4"}}"#),
            "",
        );

        let (code, said) = r.save();

        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&updated), "{said}");
        assert_eq!(r.remote(&branch), r.head());
        assert_eq!(r.remote("main"), main_before);
        assert_eq!(r.standing().stage, Stage::PrOpen);
    }

    #[test]
    fn a_charter_branch_a_save_made_before_the_rename_carries_on_and_its_pr_is_not_duplicated() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        // V93j: a save made before the rename left `charter/alpha/<sha>` with PR #4 open from
        // it. The next save carries on there and updates #4; it opens no `purlis/…` branch and
        // no second request (no create is written down, so one would exit 99).
        let host = "repo-renamed.test";
        let scene = Scene::new(host);
        let r = repo(host, "[repos.widget]\nmode = \"pr\"\n");
        r.commit("a.md", "first change");
        let old = format!("charter/alpha/{}", &r.head()[..7]);
        support::git(&r.clone, &["branch", &old]);
        support::git(
            &r.clone,
            &["push", "-q", &r.bare.display().to_string(), &old],
        );
        r.commit("b.md", "second change");
        let found =
            format!(r#"[{{"number": 4, "html_url": "https://{host}/acme/widget/pull/4"}}]"#);
        let looked = lookup(&scene, &old, &found);
        let updated = scene.answers(
            "gh",
            &[
                "api",
                "--hostname",
                host,
                "-X",
                "PATCH",
                "repos/acme/widget/pulls/4",
                "-f",
                "title=second change",
                "-f",
                BODY,
            ],
            0,
            &format!(r#"{{"number": 4, "html_url": "https://{host}/acme/widget/pull/4"}}"#),
            "",
        );

        let (code, said) = r.save();

        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&looked) && was_asked(&updated), "{said}");
        assert_eq!(r.remote(&old), r.head());
        assert_eq!(r.last()["branch"], old.as_str());
        let purlis = support::git(&r.bare, &["for-each-ref", "refs/heads/purlis/"]).stdout;
        assert!(purlis.is_empty(), "a purlis branch was pushed: {said}");
    }

    #[test]
    fn on_the_base_branch_the_settings_name_pr_mode_pushes_a_branch_of_its_own_and_never_it() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        // `[repos.widget] branch` names where its requests go; standing on that branch is
        // standing on the branch a request goes into, default branch or not (#299).
        let host = "repo-base.test";
        let scene = Scene::new(host);
        let r = repo(
            host,
            "[repos.widget]\nmode = \"pr\"\nbranch = \"develop\"\n",
        );
        support::git(&r.clone, &["checkout", "-q", "-b", "develop"]);
        support::git(
            &r.clone,
            &["push", "-q", &r.bare.display().to_string(), "develop"],
        );
        let develop_before = r.remote("develop");
        r.commit("e.md", "base change");
        let branch = format!("purlis/alpha/{}", &r.head()[..7]);
        scene.gh_api(
            &format!(
                "repos/acme/widget/pulls?state=open&head=acme:{}&base=develop&per_page=1",
                branch.replace('/', "%2F")
            ),
            0,
            "[]",
            "",
        );
        let opened = scene.answers(
            "gh",
            &[
                "api",
                "--hostname",
                host,
                "-X",
                "POST",
                "repos/acme/widget/pulls",
                "-f",
                &format!("head={branch}"),
                "-f",
                "base=develop",
                "-f",
                "title=base change",
                "-f",
                BODY,
            ],
            0,
            &format!(r#"{{"number": 6, "html_url": "https://{host}/acme/widget/pull/6"}}"#),
            "",
        );

        let (code, said) = r.save();

        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&opened), "{said}");
        assert_eq!(r.remote(&branch), r.head(), "{said}");
        assert_eq!(
            r.remote("develop"),
            develop_before,
            "develop was pushed: {said}"
        );
    }

    /// What one look at widget's #12 reads, written down once: GitHub's `mergeStateStatus`
    /// for it at `head`, and the check runs there. Answers the read of its merge state.
    fn looked(scene: &Scene, head: &str, merge_state: &str, runs: &[&str]) -> PathBuf {
        let read = scene.answers(
            "gh",
            &[
                "api",
                "graphql",
                "--hostname",
                &scene.host,
                "-f",
                &format!("query={MERGE_READY}"),
                "-f",
                "owner=acme",
                "-f",
                "name=widget",
                "-F",
                "number=12",
            ],
            0,
            &format!(
                r#"{{"data": {{"repository": {{"rebaseMergeAllowed": true,
                "mergeCommitAllowed": true, "squashMergeAllowed": true,
                "pullRequest": {{"state": "OPEN", "isDraft": false, "headRefOid": "{head}",
                "mergeStateStatus": "{merge_state}", "reviewDecision": null,
                "isMergeQueueEnabled": false, "baseRefName": "main",
                "headRefName": "feature/y", "isCrossRepository": false,
                "updatedAt": "2026-01-01T00:00:00Z",
                "body": "Saved by charter.\n\n<!-- purlis-save -->",
                "commits": {{"nodes": [{{"commit": {{"oid": "{head}",
                "statusCheckRollup": null}}}}]}}}}}}}}}}"#
            ),
            "",
        );
        scene.gh_api(
            &format!("repos/acme/widget/commits/{head}/check-runs?per_page=100"),
            0,
            &format!(
                r#"{{"total_count": {}, "check_runs": [{}]}}"#,
                runs.len(),
                runs.join(", ")
            ),
            "",
        );
        scene.gh_api(
            &format!("repos/acme/widget/commits/{head}/status?per_page=100"),
            0,
            r#"{"state": "pending", "total_count": 0, "statuses": []}"#,
            "",
        );
        read
    }

    /// GitHub's merge of widget's #12 at `head`, by rebase, answered as merged.
    fn merged_by_purlis(scene: &Scene, head: &str) -> PathBuf {
        scene.answers(
            "gh",
            &[
                "api",
                "--hostname",
                &scene.host,
                "-X",
                "PUT",
                "repos/acme/widget/pulls/12/merge",
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

    /// A `pr-merge` widget on `feature/y` with one commit, saved: #12 is open from it.
    fn saved_in_pr_merge(host: &str) -> (Repo, Scene, String, String) {
        let scene = Scene::new(host);
        let r = repo(host, "[repos.widget]\nmode = \"pr-merge\"\n");
        support::git(&r.clone, &["checkout", "-q", "-b", "feature/y"]);
        r.commit("c.md", "a change");
        let body = "body=Saved by charter from the alpha workspace ([repos.widget] mode = pr-merge).\n\n<!-- purlis-save -->";
        lookup(&scene, "feature/y", "[]");
        create(&scene, "feature/y", "a change", body, 12);
        let head = r.head();
        let (code, said) = r.save();
        assert_eq!(code, 0, "{said}");
        (r, scene, head, said)
    }

    /// The wait for the pushed commit's checks to start is over: the journal says it was
    /// pushed an hour ago.
    fn the_settle_wait_is_over(r: &Repo) {
        let path = planegit::journal_path(&r.plane);
        let text = std::fs::read_to_string(&path).unwrap();
        let mut lines: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let last = lines.last_mut().expect("a journal line");
        last["pushed_at"] =
            serde_json::json!(last["pushed_at"].as_u64().expect("when it was pushed") - 3600);
        let rewritten: String = lines.iter().map(|line| format!("{line}\n")).collect();
        std::fs::write(&path, rewritten).unwrap();
    }

    fn widget(r: &Repo) -> purlis_core::repos::Repo {
        purlis_core::repos::Repo {
            name: "widget".into(),
            path: r.clone.clone(),
        }
    }

    /// The app's look at widget's open pull request, with no save.
    fn look(r: &Repo) -> planegit::Looked {
        reposave::look(
            &r.plane,
            "alpha",
            &widget(r),
            &purlis_core::forge::Caller::trigger(),
        )
        .expect("the look")
    }

    const FAILED: &str = r#"{"id": 7, "name": "test", "status": "completed", "conclusion": "failure", "html_url": "https://repo-merge.example/acme/widget/actions/runs/9/job/7"}"#;

    /// **A repo's save merges nothing itself, and asks the forge for no auto-merge** (ADR
    /// 0051, amended 2026-10-13), as the plane's does not: the request waits, the row says so,
    /// and a look a moment later still waits, because no check has had time to start.
    #[test]
    fn a_pr_merge_repo_save_merges_nothing_at_the_save_and_asks_for_no_auto_merge() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let host = "repo-merge.test";
        let (r, scene, head, said) = saved_in_pr_merge(host);
        // What a save used to ask, written down so the test can say it was not asked.
        let settings = scene.answers(
            "gh",
            &[
                "api",
                "graphql",
                "--hostname",
                host,
                "-f",
                &format!("query={SETTINGS}"),
                "-f",
                "owner=acme",
                "-f",
                "name=widget",
                "-F",
                "number=12",
            ],
            0,
            r#"{"data": {"repository": {"autoMergeAllowed": true, "rebaseMergeAllowed": true, "mergeCommitAllowed": true, "squashMergeAllowed": true, "pullRequest": {"id": "PR_node12"}}}}"#,
            "",
        );
        let enable = scene.answers(
            "gh",
            &[
                "api",
                "graphql",
                "--hostname",
                host,
                "-f",
                &format!("query={ENABLE}"),
                "-f",
                "id=PR_node12",
                "-f",
                "method=REBASE",
                "-f",
                &format!("head={head}"),
            ],
            0,
            r#"{"data": {"enablePullRequestAutoMerge": {"clientMutationId": null}}}"#,
            "",
        );
        let put = merged_by_purlis(&scene, &head);

        assert!(said.contains("not had time to start"), "{said}");
        let line = r.last();
        assert_eq!(line["outcome"], "pr-open");
        assert_eq!(line["number"], 12);
        assert_eq!(line["merge"]["state"], "waiting");
        let standing = r.standing();
        assert_eq!(standing.stage, Stage::PrOpen, "{standing:?}");
        assert_eq!(
            standing.merge.map(|merge| merge.state),
            Some(planegit::MergeState::Waiting)
        );

        // A look a moment after the push: no check yet is "not yet".
        looked(&scene, &head, "CLEAN", &[]);
        let found = look(&r);

        assert!(found.open && !found.changed, "{found:?}");
        assert!(
            !was_asked(&settings) && !was_asked(&enable),
            "auto-merge was asked for"
        );
        assert!(!was_asked(&put), "merged before any check could start");
    }

    /// **The fault, for a repo**: no check at the save, a failed one a minute later. The
    /// request stays open and unmerged, red and named by the check, and the repo is not
    /// blocked. Only the person's own "merge it as it stands" merges it.
    #[test]
    fn a_repo_pr_whose_check_fails_after_the_save_stays_open_red_until_the_person_merges_it() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let (r, scene, head, _) = saved_in_pr_merge("repo-merge-red.test");
        the_settle_wait_is_over(&r);
        looked(&scene, &head, "UNSTABLE", &[FAILED]);
        let put = merged_by_purlis(&scene, &head);

        let found = look(&r);

        assert!(!was_asked(&put), "merged past a failed check");
        assert!(found.open && found.changed, "{found:?}");
        let standing = r.standing();
        assert_eq!(
            standing.stage,
            Stage::PrOpen,
            "red is not a block: {standing:?}"
        );
        assert_eq!(standing.blocked, None);
        let merge = standing.merge.expect("where the request stands");
        assert_eq!(merge.state, planegit::MergeState::Red, "{merge:?}");
        assert_eq!(merge.checks[0].name, "test");
        let line = r.last();
        assert_eq!(line["trigger"], "look");
        assert_eq!(line["outcome"], "pr-open");
        // The look's own line keeps when the commit was pushed: the wait is never measured
        // from a look (review, minor 4).
        let lines = planegit::journal(&r.plane);
        let saved = &lines[lines.len() - 2];
        assert_eq!(saved["trigger"], "manual");
        assert_eq!(line["pushed_at"], saved["pushed_at"]);
        assert!(line["pushed_at"].as_u64().is_some(), "{line}");
        assert!(
            line["detail"]
                .as_str()
                .unwrap()
                .contains("is red and stays open"),
            "{line}"
        );

        // Looked at again with nothing new: no second journal line for the same answer.
        let lines = planegit::journal(&r.plane).len();
        looked(&scene, &head, "UNSTABLE", &[FAILED]);
        let found = look(&r);
        assert!(found.open && !found.changed, "{found:?}");
        assert_eq!(planegit::journal(&r.plane).len(), lines);

        // What the confirmation showed: the window's look, handed back with the press.
        let seen = reposave::seen(&r.plane, "alpha", &widget(&r)).expect("what it shows");
        assert_eq!(seen.head, head);

        // A press on a confirmation drawn before the check failed: the fresh look at the press
        // finds it red, and nothing is merged past a failure nobody was shown.
        looked(&scene, &head, "UNSTABLE", &[FAILED]);
        let waited = planegit::Merging::new(
            planegit::MergeState::Waiting,
            "its checks have not had time to start",
        );
        let before_the_failure = planegit::Seen {
            head: head.clone(),
            shown: waited.shown(),
        };
        let refused = reposave::merge_as_seen(
            &r.plane,
            "alpha",
            &widget(&r),
            &purlis_core::forge::Caller::window(),
            &before_the_failure,
        );
        assert!(
            matches!(&refused, Err(why) if why.contains("changed since you looked")),
            "{refused:?}"
        );
        assert!(!was_asked(&put), "merged past a failure nobody was shown");

        // A press on a confirmation of another commit: refused too.
        looked(&scene, &head, "UNSTABLE", &[FAILED]);
        let another_commit = planegit::Seen {
            head: "1".repeat(40),
            shown: seen.shown.clone(),
        };
        let refused = reposave::merge_as_seen(
            &r.plane,
            "alpha",
            &widget(&r),
            &purlis_core::forge::Caller::window(),
            &another_commit,
        );
        assert!(
            matches!(&refused, Err(why) if why.contains("a new commit was pushed")),
            "{refused:?}"
        );
        assert!(!was_asked(&put), "merged a commit nobody confirmed");

        // The press on what was shown: looked at again, found as shown, and merged at the
        // pushed commit. The merge reads the merge state and no check.
        looked(&scene, &head, "UNSTABLE", &[FAILED]);
        scene.answers(
            "gh",
            &[
                "api",
                "graphql",
                "--hostname",
                &scene.host,
                "-f",
                &format!("query={MERGE_READY}"),
                "-f",
                "owner=acme",
                "-f",
                "name=widget",
                "-F",
                "number=12",
            ],
            0,
            &format!(
                r#"{{"data": {{"repository": {{"rebaseMergeAllowed": true,
                "mergeCommitAllowed": true, "squashMergeAllowed": true,
                "pullRequest": {{"state": "OPEN", "isDraft": false, "headRefOid": "{head}",
                "mergeStateStatus": "UNSTABLE", "reviewDecision": null,
                "isMergeQueueEnabled": false, "baseRefName": "main",
                "headRefName": "feature/y", "isCrossRepository": false,
                "updatedAt": "2026-01-01T00:00:00Z",
                "body": "Saved by charter.\n\n<!-- purlis-save -->",
                "commits": {{"nodes": []}}}}}}}}}}"#
            ),
            "",
        );
        let merged = reposave::merge_as_seen(
            &r.plane,
            "alpha",
            &widget(&r),
            &purlis_core::forge::Caller::window(),
            &seen,
        );
        assert!(
            matches!(&merged, Ok(planegit::MergedAsItStands::Later(said))
                if said.starts_with("Merged as it stands, as you asked")),
            "{merged:?}"
        );
        assert!(was_asked(&put), "merged at the pushed commit");
        let line = r.last();
        assert_eq!(line["trigger"], "merge");
        assert_eq!(line["outcome"], "pushed");
        assert_eq!(line["merge"]["state"], "merged");
    }

    /// No check, and no sign of CI in the repository: a look once the head is settled merges
    /// the request **without a new save**.
    #[test]
    fn a_repo_pr_with_no_check_and_no_sign_of_ci_is_merged_by_a_later_look() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        let host = "repo-merge-none.test";
        let (r, scene, head, _) = saved_in_pr_merge(host);
        the_settle_wait_is_over(&r);
        looked(&scene, &head, "CLEAN", &[]);
        let asked = scene.answers(
            "gh",
            &[
                "api",
                "graphql",
                "--hostname",
                host,
                "-f",
                &format!("query={CHECKS_EXPECTED}"),
                "-f",
                "owner=acme",
                "-f",
                "name=widget",
                "-f",
                "base=main",
            ],
            0,
            r#"{"data": {"repository": {"pullRequests": {"nodes": [
                {"number": 12, "commits": {"nodes": [{"commit": {"statusCheckRollup": null}}]}},
                {"number": 11, "commits": {"nodes": [{"commit": {"statusCheckRollup": null}}]}}
            ]}}}}"#,
            "",
        );
        let put = merged_by_purlis(&scene, &head);

        let found = look(&r);

        assert!(
            was_asked(&asked),
            "whether the repository runs checks was not asked"
        );
        assert!(was_asked(&put), "merged at the pushed commit");
        assert!(!found.open && found.changed, "{found:?}");
        let line = r.last();
        assert_eq!(line["outcome"], "pushed");
        assert_eq!(line["merge"]["state"], "merged");
        // Merged: nothing is left to look at.
        assert_eq!(look(&r), planegit::Looked::default());
    }

    const CHECKS_EXPECTED: &str =
        include_str!("../../src/forge/github/queries/checks_expected.graphql");

    #[test]
    fn a_pr_a_person_opened_from_the_branch_is_never_rewritten_nor_set_to_merge() {
        purlis_core::unsteered!();
        if !in_child() {
            return;
        }
        // pr-merge, on a branch the operator already opened a PR from by hand. Nothing but the
        // lookup is written down: a PATCH, or any auto-merge question, would fail the save.
        let host = "repo-theirs.test";
        let scene = Scene::new(host);
        let r = repo(host, "[repos.widget]\nmode = \"pr-merge\"\n");
        support::git(&r.clone, &["checkout", "-q", "-b", "feature/z"]);
        r.commit("d.md", "their change");
        let looked = lookup(
            &scene,
            "feature/z",
            &format!(
                r#"[{{"number": 8, "html_url": "https://{host}/acme/widget/pull/8", "body": "My own words."}}]"#
            ),
        );

        let (code, said) = r.save();

        assert_eq!(code, 0, "{said}");
        assert!(was_asked(&looked));
        assert_eq!(
            r.remote("feature/z"),
            r.head(),
            "the branch is still pushed"
        );
        assert!(
            said.contains("already has a pull request purlis did not open, #8"),
            "{said}"
        );
        assert!(!said.contains("auto-merge"), "{said}");
        let line = r.last();
        assert_eq!(line["pr"], format!("https://{host}/acme/widget/pull/8"));
    }

    const SETTINGS: &str = "query($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name){autoMergeAllowed rebaseMergeAllowed mergeCommitAllowed squashMergeAllowed pullRequest(number:$number){id}}}";
    const MERGE_READY: &str = include_str!("../../src/forge/github/queries/merge_ready.graphql");
    const ENABLE: &str = "mutation($id:ID!,$method:PullRequestMergeMethod!,$head:GitObjectID!){enablePullRequestAutoMerge(input:{pullRequestId:$id,mergeMethod:$method,expectedHeadOid:$head}){clientMutationId}}";

    /// A repo whose origin is on a GitLab: its save names the merge request as GitLab does,
    /// `!3` (#1067).
    mod on_gitlab {
        use super::*;

        const MRS: &str = "projects/acme%2Fwidget/merge_requests";

        fn mr_url(host: &str, number: u32) -> String {
            format!("https://{host}/acme/widget/-/merge_requests/{number}")
        }

        fn lookup(scene: &Scene, head: &str, out: &str) -> PathBuf {
            scene.glab_api(
                &format!(
                    "{MRS}?state=opened&source_branch={}&target_branch=main&per_page=100",
                    head.replace('/', "%2F")
                ),
                0,
                out,
                "",
            )
        }

        #[test]
        fn a_pr_merge_save_opens_a_merge_request_and_names_it_bang_3() {
            purlis_core::unsteered!();
            if !in_child() {
                return;
            }
            let host = "repo-mr-open.test";
            let scene = Scene::new(host);
            let r = repo_on("gitlab", host, "[repos.widget]\nmode = \"pr-merge\"\n");
            support::git(&r.clone, &["checkout", "-q", "-b", "feature/x"]);
            r.commit("c.md", "a change");
            lookup(&scene, "feature/x", "[]");
            let opened = scene.answers(
                "glab",
                &[
                    "--hostname",
                    host,
                    "api",
                    "-X",
                    "POST",
                    MRS,
                    "-f",
                    "source_branch=feature/x",
                    "-f",
                    "target_branch=main",
                    "-f",
                    "title=a change",
                    "-f",
                    "description=Saved by charter from the alpha workspace ([repos.widget] mode = pr-merge).\n\n<!-- purlis-save -->",
                ],
                0,
                &format!(r#"{{"iid": 3, "id": 9003, "web_url": "{}"}}"#, mr_url(host, 3)),
                "",
            );
            let (code, said) = r.save();

            assert_eq!(code, 0, "{said}");
            assert!(was_asked(&opened), "{said}");
            assert!(
                said.contains(&format!(
                    "Merge request !3 from feature/x into main: {}",
                    mr_url(host, 3)
                )),
                "{said}"
            );
            // Nothing about merging is written down: the save asks none of it, and says the
            // merge request waits for its pipeline.
            assert!(
                said.contains("purlis merges merge request !3 itself"),
                "{said}"
            );
            assert!(said.contains("not had time to start"), "{said}");
            assert!(!said.to_lowercase().contains("pull request"), "{said}");
        }

        /// A repo with merge trains (#805): never merged directly. Once purlis reads the
        /// pipeline at the pushed commit as passed, a look adds the merge request to the train.
        #[test]
        fn a_pr_merge_save_on_a_merge_train_joins_the_train_once_its_pipeline_passed() {
            purlis_core::unsteered!();
            if !in_child() {
                return;
            }
            let host = "repo-mr-train.test";
            let scene = Scene::new(host);
            let r = repo_on("gitlab", host, "[repos.widget]\nmode = \"pr-merge\"\n");
            support::git(&r.clone, &["checkout", "-q", "-b", "feature/x"]);
            r.commit("c.md", "a change");
            lookup(&scene, "feature/x", "[]");
            scene.answers(
                "glab",
                &[
                    "--hostname",
                    host,
                    "api",
                    "-X",
                    "POST",
                    MRS,
                    "-f",
                    "source_branch=feature/x",
                    "-f",
                    "target_branch=main",
                    "-f",
                    "title=a change",
                    "-f",
                    "description=Saved by charter from the alpha workspace ([repos.widget] mode = pr-merge).\n\n<!-- purlis-save -->",
                ],
                0,
                &format!(r#"{{"iid": 3, "id": 9003, "web_url": "{}"}}"#, mr_url(host, 3)),
                "",
            );
            let (code, said) = r.save();
            assert_eq!(code, 0, "{said}");
            the_settle_wait_is_over(&r);
            let head = r.head();
            scene.glab_api(
                "projects/acme%2Fwidget",
                0,
                r#"{"squash_option": "default_off", "merge_trains_enabled": true}"#,
                "",
            );
            scene.glab_api(
                &format!("{MRS}/3"),
                0,
                &format!(
                    r#"{{"iid": 3, "state": "opened", "draft": false, "sha": "{head}",
                    "source_branch": "feature/x", "target_branch": "main",
                    "source_project_id": 1, "target_project_id": 1,
                    "description": "Saved by charter.\n\n<!-- purlis-save -->",
                    "updated_at": "2026-01-01T00:00:00.000Z",
                    "detailed_merge_status": "mergeable"}}"#
                ),
                "",
            );
            scene.glab_api(
                &format!("{MRS}/3/pipelines?per_page=100"),
                0,
                &format!(r#"[{{"id": 77, "sha": "{head}", "status": "success"}}]"#),
                "",
            );
            let joined = scene.answers(
                "glab",
                &[
                    "--hostname",
                    host,
                    "api",
                    "-X",
                    "POST",
                    "projects/acme%2Fwidget/merge_trains/merge_requests/3",
                    "-F",
                    "squash=false",
                    "-f",
                    &format!("sha={head}"),
                ],
                0,
                r#"[{"id": 267, "merge_request": {"iid": 3}, "status": "idle"}]"#,
                "",
            );

            let found = reposave::look(
                &r.plane,
                "alpha",
                &purlis_core::repos::Repo {
                    name: "widget".into(),
                    path: r.clone.clone(),
                },
                &purlis_core::forge::Caller::trigger(),
            )
            .expect("the look");

            assert!(was_asked(&joined), "not added to the train");
            assert!(!found.open && found.changed, "{found:?}");
            let line = r.last();
            assert_eq!(line["merge"]["state"], "queued");
            assert!(
                line["detail"].as_str().unwrap().contains("merge train"),
                "{line}"
            );
            assert!(
                !line["detail"]
                    .as_str()
                    .unwrap()
                    .to_lowercase()
                    .contains("pull request")
            );
        }

        #[test]
        fn a_merge_request_a_person_opened_is_named_bang_8_and_left_alone() {
            purlis_core::unsteered!();
            if !in_child() {
                return;
            }
            let host = "repo-mr-theirs.test";
            let scene = Scene::new(host);
            let r = repo_on("gitlab", host, "[repos.widget]\nmode = \"pr-merge\"\n");
            support::git(&r.clone, &["checkout", "-q", "-b", "feature/z"]);
            r.commit("d.md", "their change");
            lookup(
                &scene,
                "feature/z",
                &format!(
                    r#"[{{"iid": 8, "source_project_id": 5, "target_project_id": 5, "web_url": "{}", "description": "My own words."}}]"#,
                    mr_url(host, 8)
                ),
            );

            let (code, said) = r.save();

            assert_eq!(code, 0, "{said}");
            assert!(
                said.contains("already has a merge request purlis did not open, !8"),
                "{said}"
            );
        }
    }
}
