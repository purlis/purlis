// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The publish seam (SK-18, #2781), against a git source in a temporary folder and a stand-in
//! forge. Each forge's own request is held by `forge_contract`.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

use super::super::{Remote, SourceGrade};
use super::*;

/// Built here, so the scan of this file finds no address in it.
static ADDRESS: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| format!("ada{}people.invalid", '@'));

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
    upstream_of(tmp, &work)
}

/// A bare repository of the files in `work`, one commit on `main`.
fn upstream_of(tmp: &Path, work: &Path) -> PathBuf {
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["add", "."],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t",
            "commit",
            "-qm",
            "one",
        ],
    ] {
        assert!(crate::testgit::run(work, args).ok());
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
        &["config", "user.email", ADDRESS.as_str()],
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
    /// What the forge says of the account's write access: `Err` where it could not be asked.
    may_push: Option<Result<Option<bool>, String>>,
    /// What the forge says of the account's fork: none yet, one to make at [`FORK_PATH`], where
    /// unset.
    fork_place: Option<ForkPlace>,
    /// Where a fork this forge makes goes: a bare copy of `.0`, the source, at `.1`.
    makes: Option<(PathBuf, PathBuf)>,
    /// Whether the fork it makes is answered at a place git cannot reach.
    unreachable: bool,
    /// How often the fork was asked about, and made.
    asked_fork: Cell<u32>,
    made: Cell<u32>,
    fails: Cell<bool>,
    opened: RefCell<Vec<(String, String, String, String)>>,
    /// The fork each request was opened from, where it was.
    opened_from: RefCell<Vec<Option<String>>>,
}

/// Where the account's fork goes, in the stand-in forge.
const FORK_PATH: &str = "ada/source";

/// A fork of `bare` at `path`, as a forge would answer it.
fn fork_at(path: &Path) -> Fork {
    Fork::for_a_test("9", FORK_PATH, &path.display().to_string(), Some("1"))
}

/// A bare copy of `bare` at `to`: a fork, as a forge makes one.
fn copy_of(bare: &Path, to: &Path) {
    let (from, to) = (bare.display().to_string(), to.display().to_string());
    assert!(crate::testgit::run(bare, &["clone", "-q", "--bare", &from, &to]).ok());
}

impl PublishForge for StandIn {
    fn helper(&self) -> Option<String> {
        None
    }
    fn already(&self, _: Option<&Fork>, _: &str) -> Result<Option<String>, String> {
        Ok(self.already.clone())
    }
    fn may_push(&self) -> Result<Option<bool>, String> {
        self.may_push.clone().unwrap_or(Ok(None))
    }
    fn fork_for(&self) -> Result<ForkPlace, String> {
        self.asked_fork.set(self.asked_fork.get() + 1);
        Ok(self.fork_place.clone().unwrap_or(ForkPlace::ToMake {
            path: FORK_PATH.to_owned(),
            visibility: Visibility::Public,
        }))
    }
    fn make_fork(&self) -> Result<Fork, String> {
        self.made.set(self.made.get() + 1);
        let (source, to) = self.makes.clone().ok_or("this forge makes no fork")?;
        if self.unreachable {
            return Ok(fork_at(&to.join("not-there.git")));
        }
        copy_of(&source, &to);
        Ok(fork_at(&to))
    }
    fn open_draft(
        &self,
        fork: Option<&Fork>,
        head: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> Result<String, String> {
        if self.fails.get() {
            return Err("the forge said no".to_owned());
        }
        self.opened_from
            .borrow_mut()
            .push(fork.map(|fork| fork.path().to_owned()));
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
    /// Where a fork of `bare` goes: not there until one is made.
    fork: PathBuf,
    source: Source,
}

fn set() -> Set {
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(&plane).unwrap();
    plane_with_skill(&plane);
    let bare = upstream(tmp.path());
    let source = source_at(&bare);
    let fork = tmp.path().join("fork.git");
    Set {
        _tmp: tmp,
        plane,
        bare,
        fork,
        source,
    }
}

fn review_of(set: &Set) -> PublishReview {
    review_by(set, &StandIn::default())
}

fn review_by(set: &Set, forge: &StandIn) -> PublishReview {
    review(
        &set.plane,
        None,
        &Level::project(),
        "deploy",
        &set.source,
        forge,
    )
    .unwrap()
}

fn says_may_push(answer: Result<Option<bool>, String>) -> StandIn {
    StandIn {
        may_push: Some(answer),
        ..StandIn::default()
    }
}

fn publish_of(set: &Set, accepted: &Accepted, forge: &StandIn) -> Result<Published, String> {
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

    let done = publish_of(&set, &review.accepted(), &forge).unwrap();

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

    let refused = publish_of(&set, &review.accepted(), &forge).unwrap_err();

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

    let refused = publish_of(&set, &review.accepted(), &StandIn::default()).unwrap_err();

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

    let done = publish_of(&set, &review.accepted(), &forge).unwrap();

    assert_eq!(
        done,
        Published::Already {
            url: "https://forge.invalid/9".to_owned()
        }
    );
    assert_eq!(branches(&set.bare), before);
}

#[test]
fn publishing_over_a_skill_the_source_holds_replaces_its_folder_whatever_its_names() {
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(&plane).unwrap();
    plane_with_skill(&plane);
    let work = tmp.path().join("work");
    std::fs::create_dir_all(work.join("deploy")).unwrap();
    std::fs::write(work.join("deploy/SKILL.md"), "old\n").unwrap();
    std::fs::write(work.join("deploy/f\u{f6}o.md"), "old\n").unwrap();
    let bare = upstream_of(tmp.path(), &work);
    let fork = tmp.path().join("fork.git");
    let set = Set {
        _tmp: tmp,
        plane,
        source: source_at(&bare),
        bare,
        fork,
    };

    let review = review_of(&set);
    assert_eq!(review.title, "Update the skill deploy");
    let Published::Opened { branch, .. } =
        publish_of(&set, &review.accepted(), &StandIn::default()).unwrap()
    else {
        panic!()
    };

    let listed = crate::testgit::run(&set.bare, &["ls-tree", "-r", "--name-only", &branch]);
    assert_eq!(
        listed.out.lines().collect::<Vec<_>>(),
        ["deploy/SKILL.md"],
        "{listed:?}"
    );
}

#[test]
fn a_request_that_failed_after_the_push_opens_on_the_next_try_from_the_same_branch() {
    let set = set();
    let review = review_of(&set);
    let forge = StandIn::default();
    forge.fails.set(true);
    assert!(
        publish_of(&set, &review.accepted(), &forge)
            .unwrap_err()
            .contains("was pushed")
    );

    forge.fails.set(false);
    let again = publish_of(&set, &review.accepted(), &forge).unwrap();

    assert!(matches!(again, Published::Opened { .. }), "{again:?}");
    assert_eq!(forge.opened.borrow().len(), 1);
}

#[test]
fn the_same_files_to_another_folder_are_another_branch() {
    let set = set();
    let first = review_of(&set);
    let mut elsewhere = set.source.clone();
    elsewhere.folder = Some("packs".to_owned());
    let second = review(
        &set.plane,
        None,
        &Level::project(),
        "deploy",
        &elsewhere,
        &StandIn::default(),
    )
    .unwrap();
    assert_ne!(first.branch, second.branch);
    assert_eq!(second.lands[0].0, "packs/deploy");
}

#[test]
fn the_review_names_whom_the_commit_will_name() {
    let set = set();
    assert_eq!(review_of(&set).author, ("Ada".to_owned(), ADDRESS.clone()));
}

#[test]
fn a_source_that_refuses_the_push_is_said_as_such_and_opens_nothing() {
    let set = set();
    let review = review_of(&set);
    let hooks = set.bare.join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let hook = hooks.join("pre-receive");
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let forge = StandIn::default();

    let done = publish_of(&set, &review.accepted(), &forge).unwrap();

    assert!(matches!(done, Published::PushRefused { .. }), "{done:?}");
    assert!(forge.opened.borrow().is_empty());
}

#[test]
fn a_review_says_up_front_when_the_forge_says_the_account_cannot_push() {
    let set = set();
    assert_eq!(
        review_by(&set, &says_may_push(Ok(Some(false)))).may_push,
        Some(false)
    );
    assert_eq!(
        review_by(&set, &says_may_push(Ok(Some(true)))).may_push,
        Some(true)
    );
}

#[test]
fn a_forge_that_cannot_be_asked_leaves_write_access_unknown_and_the_review_stands() {
    let set = set();
    let forge = says_may_push(Err("the forge did not answer".to_owned()));
    assert_eq!(review_by(&set, &forge).may_push, None);
}

#[test]
fn a_review_stopped_already_does_not_ask_the_forge() {
    let set = set();
    let dir = Level::project().dir(&set.plane).unwrap().join("deploy");
    let token = format!("ghp_{}", "a1B2".repeat(9));
    std::fs::write(dir.join("SKILL.md"), format!("{SKILL}{token}\n")).unwrap();
    let review = review_by(&set, &says_may_push(Ok(Some(false))));
    assert!(review.stopped());
    assert_eq!(review.may_push, None);
}

#[test]
fn an_account_the_forge_says_may_push_still_gets_a_branch_on_the_source() {
    let set = set();
    let forge = says_may_push(Ok(Some(true)));
    let review = review_by(&set, &forge);

    let done = publish_of(&set, &review.accepted(), &forge).unwrap();

    assert!(matches!(done, Published::Opened { .. }), "{done:?}");
    assert!(
        branches(&set.bare).contains(&review.branch),
        "{}",
        branches(&set.bare)
    );
}

#[test]
fn a_folder_on_this_machine_is_no_forge_to_publish_to() {
    let set = set();
    let refused = OnTheForge::of(&set.plane, &set.source).err().unwrap();
    assert!(refused.contains("Export the skill instead"), "{refused}");
}

/// A grade of `deploy` at `ts`, on the folder whose tree is `tree`, with what must never go out.
fn graded(tree: &str, ts: &str) -> Graded {
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
        outcome_pass_rate: None,
        outcome_chats: 0,
        outcome_not_run: 0,
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
        outcomes: Vec::new(),
        outcome_delta: None,
        outcome_tokens: None,
        outcome_usd: None,
        failed: None,
        resets_at: None,
        earlier: None,
    };
    Graded {
        v: 1,
        grade: "g1".to_owned(),
        skill: "deploy".to_owned(),
        level: Some("skills".to_owned()),
        tree: tree.to_owned(),
        commit: None,
        harness: Some("claude|code".to_owned()),
        profile: Some("my-profile".to_owned()),
        ts: ts.to_owned(),
        runs: 3,
        report,
        spent: grades::Spent::default(),
        plugins_off: None,
        model: None,
        previous: None,
        after_edit: false,
        draft: None,
    }
}

#[test]
fn a_grade_goes_as_a_summary_with_no_chat_text_path_cost_or_device() {
    let graded = graded("abc", "2026-10-30T10:00:00Z");

    let row = grade_row(&grade_sent("deploy", &graded, Some("abc")));

    assert!(
        row.contains("90%") && row.contains("50%") && row.contains("this one"),
        "{row}"
    );
    assert!(row.contains("claudecode"), "{row}");
    for never in ["secret", "/Users", "my-profile", "7.77", "12345"] {
        assert!(!row.contains(never), "{row}");
    }
}

/// A forge that says the account may not push, and makes its fork beside the source.
fn says_no_push(set: &Set) -> StandIn {
    StandIn {
        may_push: Some(Ok(Some(false))),
        makes: Some((set.bare.clone(), set.fork.clone())),
        ..StandIn::default()
    }
}

#[test]
fn a_review_names_the_fork_it_would_make_and_makes_nothing() {
    let set = set();
    let forge = says_no_push(&set);

    let review = review_by(&set, &forge);

    assert_eq!(
        review.route,
        Route::NewFork {
            path: FORK_PATH.to_owned(),
            visibility: Visibility::Public,
        }
    );
    assert_eq!(forge.made.get(), 0);
    assert!(!set.fork.exists());
}

#[test]
fn a_person_who_cannot_push_publishes_through_the_fork_the_yes_made() {
    let set = set();
    let forge = says_no_push(&set);
    let review = review_by(&set, &forge);
    let source_before = branches(&set.bare);

    let done = publish_of(&set, &review.accepted(), &forge).unwrap();

    assert!(matches!(done, Published::Opened { .. }), "{done:?}");
    assert_eq!(forge.made.get(), 1);
    assert!(
        branches(&set.fork).contains(&review.branch),
        "{}",
        branches(&set.fork)
    );
    assert_eq!(branches(&set.bare), source_before, "nothing on the source");
    assert_eq!(
        *forge.opened_from.borrow(),
        [Some(FORK_PATH.to_owned())],
        "the request is opened from the fork"
    );
    assert_eq!(forge.opened.borrow()[0].1, "main");
}

#[test]
fn a_fork_the_person_has_already_is_used_and_none_is_made() {
    let set = set();
    copy_of(&set.bare, &set.fork);
    let forge = StandIn {
        fork_place: Some(ForkPlace::Yours(fork_at(&set.fork))),
        ..says_no_push(&set)
    };
    let review = review_by(&set, &forge);
    assert_eq!(review.route, Route::Fork(fork_at(&set.fork)));

    let done = publish_of(&set, &review.accepted(), &forge).unwrap();

    assert!(matches!(done, Published::Opened { .. }), "{done:?}");
    assert_eq!(forge.made.get(), 0);
    assert!(branches(&set.fork).contains(&review.branch));
}

#[test]
fn a_repo_of_the_persons_that_is_not_the_sources_fork_stops_the_publish() {
    let set = set();
    let forge = StandIn {
        fork_place: Some(ForkPlace::Taken {
            path: FORK_PATH.to_owned(),
        }),
        ..says_no_push(&set)
    };
    let review = review_by(&set, &forge);
    assert!(review.stopped());
    assert!(
        review.outgoing.stops[0]
            .to_string()
            .contains("not a fork of the source"),
        "{}",
        review.outgoing.stops[0]
    );

    let refused = publish_of(&set, &review.accepted(), &forge).unwrap_err();

    assert!(refused.starts_with("Nothing was sent"), "{refused}");
    assert_eq!(forge.made.get(), 0);
}

#[test]
fn a_yes_to_a_branch_on_the_source_is_never_a_yes_to_making_a_fork() {
    let set = set();
    let review = review_by(&set, &says_may_push(Ok(None)));
    assert_eq!(review.route, Route::Source);
    let forge = says_no_push(&set);

    let refused = publish_of(&set, &review.accepted(), &forge).unwrap_err();

    assert!(refused.contains("not what you reviewed"), "{refused}");
    assert_eq!(forge.made.get(), 0);
    assert!(!set.fork.exists());
}

#[test]
fn a_fork_made_before_the_push_failed_stays_and_is_said() {
    let set = set();
    let forge = StandIn {
        unreachable: true,
        ..says_no_push(&set)
    };
    let review = review_by(&set, &forge);

    let said = match publish_of(&set, &review.accepted(), &forge) {
        Ok(Published::PushRefused { why }) => why,
        Err(why) => why,
        other => panic!("{other:?}"),
    };

    assert!(said.contains("was made and stays"), "{said}");
    assert!(forge.opened.borrow().is_empty());
}

#[test]
fn an_account_the_forge_does_not_say_of_gets_a_branch_on_the_source_and_no_fork() {
    let set = set();
    let forge = says_may_push(Err("the forge did not answer".to_owned()));
    let review = review_by(&set, &forge);
    assert_eq!(review.route, Route::Source);
    assert_eq!(forge.asked_fork.get(), 0);

    let done = publish_of(&set, &review.accepted(), &forge).unwrap();

    assert!(matches!(done, Published::Opened { .. }), "{done:?}");
    assert!(branches(&set.bare).contains(&review.branch));
}

/// Commits the plane, so its skill's folder has a tree a grade can be tied to, and answers it.
fn committed(plane: &Path) -> String {
    for args in [&["add", "."][..], &["commit", "-qm", "skills"]] {
        assert!(crate::testgit::run(plane, args).ok());
    }
    let folder = Level::project().dir(plane).unwrap().join("deploy");
    let rel = folder.strip_prefix(plane).unwrap().display().to_string();
    let spec = format!("HEAD:{rel}");
    crate::testgit::run(plane, &["rev-parse", &spec])
        .out
        .trim()
        .to_owned()
}

#[test]
fn a_publish_commits_the_grade_beside_the_folder_and_its_install_review_shows_it() {
    let set = set();
    let now = committed(&set.plane);
    let level = Level::project();
    let older = "b".repeat(40);
    grades::record(
        &set.plane,
        &level,
        &graded(&older, "2026-10-01T10:00:00Z"),
        "dev",
    )
    .unwrap();
    grades::record(
        &set.plane,
        &level,
        &graded(&now, "2026-10-30T10:00:00Z"),
        "dev",
    )
    .unwrap();
    let review = review_of(&set);
    assert!(
        review
            .carried
            .iter()
            .any(|(path, _)| path == "deploy.grades.json"),
        "{:?}",
        review.carried
    );

    let Published::Opened { branch, .. } =
        publish_of(&set, &review.accepted(), &StandIn::default()).unwrap()
    else {
        panic!()
    };

    let spec = format!("{branch}:deploy.grades.json");
    let file = crate::testgit::run(&set.bare, &["show", &spec]).out;
    for never in ["secret", "/Users", "my-profile", "7.77", "12345", "g1"] {
        assert!(!file.contains(never), "{file}");
    }
    // Read back as an install of the published branch would read it.
    let mut source = set.source.clone();
    source.remote.branch = Some(branch);
    let installing =
        crate::skills::source::review(&set.plane, &source, &["deploy".to_owned()], &level).unwrap();
    let Some(SourceGrade::Shown {
        this_version,
        more,
        newest,
        ..
    }) = &installing.skills[0].grade
    else {
        panic!("{:?}", installing.skills[0].grade);
    };
    assert!(*this_version);
    assert_eq!(*more, 1);
    assert_eq!(
        (newest.date.as_str(), newest.with, newest.without),
        ("2026-10-30", Some(90), Some(50))
    );
}

#[test]
fn a_skill_with_no_grade_publishes_no_grade_file() {
    let set = set();
    let review = review_of(&set);
    assert!(review.carried.is_empty());
    let Published::Opened { branch, .. } =
        publish_of(&set, &review.accepted(), &StandIn::default()).unwrap()
    else {
        panic!()
    };
    let listed = crate::testgit::run(&set.bare, &["ls-tree", "-r", "--name-only", &branch]);
    assert!(!listed.out.contains(".grades.json"), "{}", listed.out);
}

#[test]
fn a_file_an_install_leaves_on_the_server_still_reads_as_this_version_once_published() {
    let set = set();
    // Past the most a review reads of one file, so an install leaves it out.
    let big = vec![b'x'; (purlis_skills::tree::MOST_FILE_BYTES + 1) as usize];
    let dir = Level::project().dir(&set.plane).unwrap().join("deploy");
    std::fs::write(dir.join("data.txt"), &big).unwrap();
    let now = committed(&set.plane);
    let level = Level::project();
    grades::record(
        &set.plane,
        &level,
        &graded(&now, "2026-10-30T10:00:00Z"),
        "dev",
    )
    .unwrap();
    let review = review_of(&set);
    let Published::Opened { branch, .. } =
        publish_of(&set, &review.accepted(), &StandIn::default()).unwrap()
    else {
        panic!()
    };
    let mut source = set.source.clone();
    source.remote.branch = Some(branch);
    let installing =
        crate::skills::source::review(&set.plane, &source, &["deploy".to_owned()], &level).unwrap();
    assert!(
        matches!(
            installing.skills[0].grade,
            Some(SourceGrade::Shown {
                this_version: true,
                ..
            })
        ),
        "{:?}",
        installing.skills[0].grade
    );
}

#[test]
fn no_grade_file_goes_where_beside_the_folder_is_inside_a_skill_the_source_holds() {
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(&plane).unwrap();
    plane_with_skill(&plane);
    // The source's root is itself a skill, so `deploy.grades.json` would be inside it.
    let work = tmp.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(
        work.join("SKILL.md"),
        "---\nname: root\ndescription: R\n---\n",
    )
    .unwrap();
    let bare = upstream_of(tmp.path(), &work);
    let now = committed(&plane);
    let level = Level::project();
    grades::record(&plane, &level, &graded(&now, "2026-10-30T10:00:00Z"), "dev").unwrap();
    let review = review(
        &plane,
        None,
        &level,
        "deploy",
        &source_at(&bare),
        &StandIn::default(),
    )
    .unwrap();
    assert!(review.carried.is_empty(), "{:?}", review.carried);
}
