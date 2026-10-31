// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The publish seam (SK-18, #2781), against a git source in a temporary folder and a stand-in
//! forge. Each forge's own request is held by `forge_contract`.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use super::super::Remote;
use super::*;

const SKILL: &str = "---\nname: deploy\ndescription: Deploys it\n---\nDeploy.\n";

/// A source: a bare repository with one commit on `main`, holding the skill `kept`.
fn upstream(tmp: &Path) -> PathBuf {
    let work = tmp.join("work");
    std::fs::create_dir_all(work.join("kept")).unwrap();
    std::fs::write(
        work.join("kept/SKILL.md"),
        "---\nname: kept\ndescription: K\n---\n",
    )
    .unwrap();
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["add", "."],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t.invalid",
            "commit",
            "-qm",
            "one",
        ],
    ] {
        assert!(crate::testgit::run(&work, args).ok());
    }
    let bare = tmp.join("source.git");
    let (from, to) = (work.display().to_string(), bare.display().to_string());
    assert!(crate::testgit::run(tmp, &["clone", "-q", "--bare", &from, &to]).ok());
    bare
}

fn plane_with_skill(plane: &Path) {
    let dir = Level::project().dir(plane).unwrap().join("deploy");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), SKILL).unwrap();
    for args in [
        &["init", "-q"][..],
        &["config", "user.name", "Ada"],
        &["config", "user.email", "ada@people.invalid"],
    ] {
        assert!(crate::testgit::run(plane, args).ok());
    }
}

fn source_at(bare: &Path) -> Source {
    Source {
        remote: Remote {
            url: bare.display().to_string(),
            branch: Some("main".to_owned()),
        },
        folder: None,
        sign_in: None,
    }
}

#[derive(Default)]
struct StandIn {
    already: Option<String>,
    opened: RefCell<Vec<(String, String, String, String)>>,
}

impl PublishForge for StandIn {
    fn helper(&self) -> Option<String> {
        None
    }
    fn already(&self, _: &str) -> Result<Option<String>, String> {
        Ok(self.already.clone())
    }
    fn open_draft(
        &self,
        head: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> Result<String, String> {
        self.opened.borrow_mut().push((
            head.to_owned(),
            base.to_owned(),
            title.to_owned(),
            body.to_owned(),
        ));
        Ok("https://forge.invalid/1".to_owned())
    }
}

fn branches(bare: &Path) -> String {
    crate::testgit::run(bare, &["branch", "--list"]).out
}

struct Set {
    _tmp: tempfile::TempDir,
    plane: PathBuf,
    bare: PathBuf,
    source: Source,
}

fn set() -> Set {
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(&plane).unwrap();
    plane_with_skill(&plane);
    let bare = upstream(tmp.path());
    let source = source_at(&bare);
    Set {
        _tmp: tmp,
        plane,
        bare,
        source,
    }
}

fn review_of(set: &Set) -> PublishReview {
    review(&set.plane, None, &Level::project(), "deploy", &set.source).unwrap()
}

fn publish_of(set: &Set, accepted: &PublishReview, forge: &StandIn) -> Result<Published, String> {
    publish(
        &set.plane,
        None,
        &Level::project(),
        "deploy",
        &set.source,
        accepted,
        forge,
    )
}

#[test]
fn a_review_writes_nothing_on_the_source() {
    let set = set();
    let before = branches(&set.bare);
    let review = review_of(&set);
    assert_eq!(branches(&set.bare), before);
    assert_eq!(review.base, "main");
    assert_eq!(review.lands, [("deploy".to_owned(), false)]);
    assert_eq!(review.title, "Add the skill deploy");
    assert!(review.body.contains("Not graded."), "{}", review.body);
}

#[test]
fn a_publish_pushes_a_new_branch_with_the_skill_and_the_sources_own_files_and_opens_a_draft() {
    let set = set();
    let review = review_of(&set);
    let forge = StandIn::default();

    let done = publish_of(&set, &review, &forge).unwrap();

    let Published::Opened { url, branch } = done else {
        panic!("{done:?}")
    };
    assert_eq!(url, "https://forge.invalid/1");
    assert!(branch.starts_with("purlis/publish/deploy-"), "{branch}");
    let listed = crate::testgit::run(&set.bare, &["ls-tree", "-r", "--name-only", &branch]);
    assert_eq!(
        listed.out.lines().collect::<Vec<_>>(),
        ["deploy/SKILL.md", "kept/SKILL.md"]
    );
    let spec = format!("{branch}:deploy/SKILL.md");
    assert_eq!(crate::testgit::run(&set.bare, &["show", &spec]).out, SKILL);
    let opened = forge.opened.borrow();
    assert_eq!(
        (opened[0].0.as_str(), opened[0].1.as_str()),
        (branch.as_str(), "main")
    );
    assert_eq!(opened[0].2, "Add the skill deploy");
}

#[test]
fn a_file_changed_after_the_review_sends_nothing() {
    let set = set();
    let review = review_of(&set);
    let dir = Level::project().dir(&set.plane).unwrap().join("deploy");
    std::fs::write(dir.join("SKILL.md"), format!("{SKILL}More.\n")).unwrap();
    let before = branches(&set.bare);
    let forge = StandIn::default();

    let refused = publish_of(&set, &review, &forge).unwrap_err();

    assert!(refused.contains("not what you reviewed"), "{refused}");
    assert_eq!(branches(&set.bare), before);
    assert!(forge.opened.borrow().is_empty());
}

#[test]
fn a_secret_sends_nothing_even_from_a_review_that_showed_it() {
    let set = set();
    let dir = Level::project().dir(&set.plane).unwrap().join("deploy");
    let token = format!("ghp_{}", "a1B2".repeat(9));
    std::fs::write(dir.join("SKILL.md"), format!("{SKILL}{token}\n")).unwrap();
    let review = review_of(&set);
    assert!(review.stopped());
    let before = branches(&set.bare);

    let refused = publish_of(&set, &review, &StandIn::default()).unwrap_err();

    assert!(
        refused.starts_with("Nothing was sent") && !refused.contains(&token),
        "{refused}"
    );
    assert_eq!(branches(&set.bare), before);
}

#[test]
fn the_same_files_with_a_request_already_open_answer_that_request() {
    let set = set();
    let review = review_of(&set);
    let forge = StandIn {
        already: Some("https://forge.invalid/9".to_owned()),
        ..StandIn::default()
    };
    let before = branches(&set.bare);

    let done = publish_of(&set, &review, &forge).unwrap();

    assert_eq!(
        done,
        Published::Already {
            url: "https://forge.invalid/9".to_owned()
        }
    );
    assert_eq!(branches(&set.bare), before);
}

#[test]
fn a_folder_on_this_machine_is_no_forge_to_publish_to() {
    let set = set();
    let refused = OnTheForge::of(&set.plane, &set.source).err().unwrap();
    assert!(refused.contains("Export the skill instead"), "{refused}");
}

#[test]
fn a_grade_goes_as_a_summary_with_no_chat_text_path_cost_or_device() {
    use purlis_skills::grade::{Arm, ArmScore, GradeEnd, GradeReport, Stat, TriggerScore};
    let score = |arm, mean| ArmScore {
        arm,
        pass_rate: Some(Stat {
            mean,
            stddev: 0.0,
            min: mean,
            max: mean,
        }),
        tokens: None,
        seconds: None,
        chats: 3,
        not_run: 0,
    };
    let report = GradeReport {
        grader: "built-in".to_owned(),
        triggers: Vec::<TriggerScore>::new(),
        arms: vec![score(Arm::With, 0.9), score(Arm::Without, 0.5)],
        delta: Some(0.4),
        ended: GradeEnd::Whole,
        not_started: Some("secret words from /Users/ada".to_owned()),
        cap: None,
        spent_tokens: Some(12345),
        cost_cap_usd: None,
        spent_usd: Some(7.77),
        counted_usd: None,
        priced: None,
        failed: None,
    };
    let graded = Graded {
        v: 1,
        grade: "g1".to_owned(),
        skill: "deploy".to_owned(),
        level: Some("skills".to_owned()),
        tree: "abc".to_owned(),
        commit: None,
        harness: Some("claude|code".to_owned()),
        profile: Some("my-profile".to_owned()),
        ts: "2026-10-30T10:00:00Z".to_owned(),
        runs: 3,
        report,
        spent: grades::Spent::default(),
        plugins_off: None,
        model: None,
        previous: None,
        after_edit: false,
    };

    let row = grade_row(&graded, Some("abc"));

    assert!(
        row.contains("90%") && row.contains("50%") && row.contains("this one"),
        "{row}"
    );
    assert!(row.contains("claudecode"), "{row}");
    for never in ["secret", "/Users", "my-profile", "7.77", "12345"] {
        assert!(!row.contains(never), "{row}");
    }
}
