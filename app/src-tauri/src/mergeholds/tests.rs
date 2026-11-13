// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The app's half of a held merge (R-114, Q239): what it answers the hook, what it holds, and
//! what it records, through a project that stands in for the app's and a forge read that stands
//! in for the forge.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use purlis_core::eventlog::stops::Stop;
use purlis_core::forge::pr::{HeadAt, Repo as OnForge};
use purlis_core::hookwire::{Answer, MergeAsk};
use purlis_core::mergehold::{self, Request};

use super::{MergeHolds, Project, answer};

const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";
const MOVED: &str = "89abcdef0123456789abcdef0123456789abcdef";

/// A project with one clone of `o/r` on github.com, which its inventory lists.
struct Fake {
    _dir: tempfile::TempDir,
    root: PathBuf,
    clone: PathBuf,
    open: bool,
    rate: Result<(), String>,
    holds: MergeHolds,
    read: Mutex<Result<HeadAt, String>>,
    recorded: Mutex<Vec<(u32, Stop)>>,
}

fn at(sha: &str) -> HeadAt {
    HeadAt {
        sha: sha.to_owned(),
        base: "main".to_owned(),
        squash: false,
    }
}

impl Fake {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a directory");
        let root = dir.path().canonicalize().expect("a real path");
        let clone = root.join("repos").join("r");
        std::fs::create_dir_all(&clone).expect("the clone");
        let git = |args: &[&str]| {
            let mut command = std::process::Command::new("git");
            command.args(args).current_dir(&clone);
            let ran = purlis_core::forklock::output(&mut command).expect("git runs");
            assert!(ran.status.success(), "{args:?}");
        };
        git(&["init", "-q"]);
        git(&["remote", "add", "origin", "https://github.com/o/r.git"]);
        let fake = Self {
            _dir: dir,
            root,
            clone,
            open: true,
            rate: Ok(()),
            holds: MergeHolds::default(),
            read: Mutex::new(Ok(at(HEAD))),
            recorded: Mutex::new(Vec::new()),
        };
        fake.lists("o/r", "https://github.com/o/r");
        fake
    }

    /// The inventory lists `path` on github with this web address.
    fn lists(&self, path: &str, web: &str) {
        let at = purlis_core::inventory::path(&self.root);
        std::fs::create_dir_all(at.parent().expect("its folder")).expect("the inventory");
        let row = serde_json::json!({"path_with_namespace": path, "forge": "github",
                                     "web_url": web});
        std::fs::write(at, serde_json::json!({"repos": [row]}).to_string()).expect("written");
    }

    fn ask(&self, number: u64) -> MergeAsk {
        MergeAsk {
            chat: 4,
            request: Request { repo: None, number },
            cwd: self.clone.display().to_string(),
        }
    }

    fn holds_recorded(&self) -> usize {
        self.recorded
            .lock()
            .unwrap()
            .iter()
            .filter(|(chat, stop)| *chat == 4 && matches!(stop, Stop::Hold(_)))
            .count()
    }
}

impl Project for Fake {
    fn root(&self) -> &Path {
        &self.root
    }
    fn is_open(&self, chat: u32) -> bool {
        self.open && chat == 4
    }
    fn allow(&self, _chat: u32) -> Result<(), String> {
        self.rate.clone()
    }
    fn holds(&self) -> &MergeHolds {
        &self.holds
    }
    fn read(&self, on: &OnForge, number: u64) -> Result<HeadAt, String> {
        assert_eq!(
            (on.path.as_str(), on.forge.host.as_str()),
            ("o/r", "github.com")
        );
        assert!(number > 0);
        self.read.lock().unwrap().clone()
    }
    fn record(&self, chat: u32, stop: Stop) {
        self.recorded.lock().unwrap().push((chat, stop));
    }
    fn moved(&self) {}
}

/// How a case sets its project up.
type Setup = Box<dyn Fn(&mut Fake)>;

/// What the chat is told, where the app raised no ask.
fn refused(answer: Answer) -> String {
    match answer {
        Answer::No { why } => why,
        other => panic!("no ask was raised, so no MergeHeld: {other:?}"),
    }
}

#[test]
fn a_held_merge_is_asked_once_recorded_once_and_rekeyed_on_a_try_again() {
    let project = Fake::new();
    let said = match answer(&project, &project.ask(12)) {
        Answer::MergeHeld { said } => said,
        other => panic!("{other:?}"),
    };
    assert!(
        said.contains("The person is asked in Asks to merge request 12 of o/r"),
        "{said}"
    );
    let waiting = project.holds.every();
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].key.head.as_str(), HEAD);
    assert_eq!(waiting[0].key.base.as_str(), "main");
    assert_eq!(project.holds_recorded(), 1);

    // Q239 (S6): the branch moved and the chat tried again: still one ask and one hold, keyed
    // on the head the forge says now.
    *project.read.lock().unwrap() = Ok(at(MOVED));
    assert!(matches!(
        answer(&project, &project.ask(12)),
        Answer::MergeHeld { .. }
    ));
    let waiting = project.holds.every();
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].key.head.as_str(), MOVED);
    assert_eq!(project.holds_recorded(), 1);
}

/// Q239 (S3): every way the app raises no ask answers the hook No, in words that say why, and
/// holds and records nothing: the hook's refusal is then a block, which the event log writes.
#[test]
fn a_merge_the_app_raises_no_ask_for_is_answered_no_and_held_nowhere() {
    let cases: Vec<(&str, Setup, &str)> = vec![
        (
            "a chat not open",
            Box::new(|p| p.open = false),
            "not one this app has open",
        ),
        (
            "a chat over its rate",
            Box::new(|p| p.rate = Err("this chat has asked too often.".to_owned())),
            "asked too often",
        ),
        (
            "a repo the inventory does not list",
            Box::new(|p| p.lists("o/other", "https://github.com/o/other")),
            "not one of this project's repos",
        ),
        (
            "a repo listed on another host",
            Box::new(|p| p.lists("o/r", "https://ghe.example.com/o/r")),
            "not one of this project's repos",
        ),
        (
            "a forge read that fails",
            Box::new(|p| *p.read.lock().unwrap() = Err("HTTP 502".to_owned())),
            "could not read it from the forge (HTTP 502)",
        ),
        (
            "a forge read past its time",
            Box::new(|p| {
                *p.read.lock().unwrap() = Err("the forge did not answer in time".to_owned());
            }),
            "did not answer in time",
        ),
    ];
    for (case, setup, says) in cases {
        let mut project = Fake::new();
        setup(&mut project);
        let why = refused(answer(&project, &project.ask(12)));
        assert!(why.contains(says), "{case}: {why}");
        assert!(project.holds.every().is_empty(), "{case}");
        assert_eq!(project.holds_recorded(), 0, "{case}");
    }

    // A repo the command names that is not the clone's.
    let project = Fake::new();
    let mut ask = project.ask(12);
    ask.request.repo = Some("o/elsewhere".try_into().unwrap());
    assert!(refused(answer(&project, &ask)).contains("names o/elsewhere"));
    assert!(project.holds.every().is_empty());

    // One more than a chat may have waiting.
    let project = Fake::new();
    for number in 1..=mergehold::AT_MOST_PER_CHAT as u64 {
        assert!(matches!(
            answer(&project, &project.ask(number)),
            Answer::MergeHeld { .. }
        ));
    }
    let why = refused(answer(&project, &project.ask(99)));
    assert!(why.contains("as many as one chat may"), "{why}");
    assert_eq!(project.holds.every().len(), mergehold::AT_MOST_PER_CHAT);
    assert_eq!(project.holds_recorded(), mergehold::AT_MOST_PER_CHAT);
}

#[test]
fn a_project_closed_before_the_merge_is_asked_about_answers_no() {
    let gone = std::sync::Weak::new();
    let ask = MergeAsk {
        chat: 4,
        request: Request {
            repo: None,
            number: 12,
        },
        cwd: "/".to_owned(),
    };
    assert!(refused(super::asked(&gone, &ask)).contains("closed"));
}
