// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The app's half of a held merge (R-114, Q239): what it answers the hook, what it holds, and
//! what it records, through a project that stands in for the app's and a forge read that stands
//! in for the forge.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use purlis_core::eventlog::stops::Stop;
use purlis_core::forge::checks::{Checks, Ci};
use purlis_core::forge::pr::{HeadAt, Repo as OnForge};
use purlis_core::hookwire::{Answer, MergeAsk};
use purlis_core::mergehold::{self, Request, Reread, Seen};

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
    read: Mutex<Result<mergehold::Read, String>>,
    recorded: Mutex<Vec<(u32, Stop)>>,
}

/// What the forge says of request 12 at `sha`: a title, an author, passed checks and a ruleset
/// on `main`, none of which the chat said.
fn at(sha: &str) -> mergehold::Read {
    mergehold::Read {
        at: HeadAt {
            sha: sha.to_owned(),
            base: "main".to_owned(),
            title: "Add the parser".to_owned(),
            author: "octo".to_owned(),
            squash: false,
        },
        facts: facts(),
    }
}

fn facts() -> mergehold::Facts {
    mergehold::Facts {
        title: "Add the parser".to_owned(),
        author: "octo".to_owned(),
        checks: Checks {
            total: Some(1),
            ci: Ci::Passed,
            why: None,
            failed: Vec::new(),
        },
        ruleset: mergehold::Ruleset::Protects,
        approved: false,
        read_at: 40,
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
    fn read(&self, on: &OnForge, number: u64) -> Result<mergehold::Read, String> {
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
    fn changes(&self, clone: &Path, base: &str, head: &str) -> Result<String, String> {
        assert_eq!(
            clone, self.clone,
            "the changes are read in the clone the merge ran in"
        );
        Ok(format!("diff of {base}...{}", &head[..7]))
    }
}

/// The person pressed Merge once on chat 4's ask at [`HEAD`]: the window read it again and
/// asked once more (Q269).
fn asked_once_more(project: &Fake) {
    super::reread(project, 4, "o/r", 12, HEAD, Reread::AsksOnceMore).expect("asked once more");
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
fn a_held_merge_is_asked_once_and_a_try_again_at_a_moved_head_is_a_new_ask() {
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

    // Q242: the branch moved and the chat tried again: a new ask at the head the forge says
    // now, held and recorded as one, and the ask at the old head is gone.
    *project.read.lock().unwrap() = Ok(at(MOVED));
    assert!(matches!(
        answer(&project, &project.ask(12)),
        Answer::MergeHeld { .. }
    ));
    let waiting = project.holds.every();
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].key.head.as_str(), MOVED);
    assert_eq!(project.holds_recorded(), 2);

    // A try again at the same head is still that one ask, and records nothing more.
    assert!(matches!(
        answer(&project, &project.ask(12)),
        Answer::MergeHeld { .. }
    ));
    assert_eq!(project.holds.every().len(), 1);
    assert_eq!(project.holds_recorded(), 2);
}

/// R-229, Q261: **a held merge's ask is written raised beside its hold, and the person's press
/// as its answer**: Merge allowed, Don't merge refused, each by the window that pressed it.
#[test]
fn a_held_merges_ask_is_written_raised_and_the_press_as_its_answer() {
    use purlis_core::eventlog::stops::{
        Answer as Said, AnsweredBy, AskAnswered, AskGroup, AskRaised,
    };
    let project = Fake::new();
    assert!(matches!(
        answer(&project, &project.ask(12)),
        Answer::MergeHeld { .. }
    ));
    super::answered(&project, 4, true);
    super::answered(&project, 4, false);

    let asks: Vec<Stop> = project
        .recorded
        .lock()
        .unwrap()
        .iter()
        .filter(|(chat, stop)| *chat == 4 && !matches!(stop, Stop::Hold(_)))
        .map(|(_, stop)| stop.clone())
        .collect();
    let by = |answer| {
        Stop::AskAnswered(AskAnswered {
            ask: AskGroup::Hold,
            answer,
            by: AnsweredBy::Window,
        })
    };
    assert_eq!(
        asks,
        [
            Stop::AskRaised(AskRaised {
                ask: AskGroup::Hold
            }),
            by(Said::Allowed),
            by(Said::Refused),
        ]
    );
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

/// The forge contract's recorded exchanges for a press of request 12 of `o/r` at [`HEAD`] on
/// GitHub, a repo with no merge queue: where it stands, how it lands, and its merge at the head.
fn a_github_merge_at_head() -> Vec<serde_json::Value> {
    use serde_json::json;
    let rest = |method: Option<&str>,
                path: &str,
                fields: serde_json::Value,
                out: serde_json::Value| {
        json!({"call": {"endpoint": {"rest": {"method": method, "path": path}}, "fields": fields},
               "reply": {"code": 0, "out": out.to_string(), "err": ""}})
    };
    let contract = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/purlis-core/tests/forge_contract/github/lands_through_queue.json");
    let text = std::fs::read_to_string(contract).expect("the contract's recording");
    let mut probe = serde_json::from_str::<serde_json::Value>(&text).expect("json")["exchanges"]
        .as_array()
        .expect("its exchanges")
        .clone();
    // The recording is of acme/api; this project's repo is o/r.
    probe[0]["call"]["fields"] = json!(
        probe[0]["call"]["fields"]
            .as_array()
            .expect("its fields")
            .iter()
            .map(|field| match field["text"][0].as_str() {
                Some("owner") => json!({"text": ["owner", "o"]}),
                Some("name") => json!({"text": ["name", "r"]}),
                _ => field.clone(),
            })
            .collect::<Vec<_>>()
    );
    probe[0]["reply"]["out"] = json!(
        json!({"data": {"repository": {"pullRequest": {"id": "PR_kwDOAbc",
               "isMergeQueueEnabled": false}}}})
        .to_string()
    );
    let mut exchanges = vec![rest(
        None,
        "repos/o/r/pulls/12",
        json!([]),
        json!({"number": 12, "title": "Add the parser", "user": {"login": "octo"},
               "head": {"sha": HEAD},
               "base": {"ref": "main", "repo": {"allow_merge_commit": true}}}),
    )];
    exchanges.extend(probe);
    exchanges.push(rest(
        Some("PUT"),
        "repos/o/r/pulls/12/merge",
        json!([{"text": ["merge_method", "merge"]},
               {"text": ["commit_title", "Merge request 12 of o/r"]},
               {"text": ["commit_message", "Merged in purlis on the person's press."]},
               {"text": ["sha", HEAD]}]),
        json!({"merged": true, "message": "Pull Request successfully merged"}),
    ));
    exchanges
}

/// A transport that answers only `exchanges`, in order, and says what was left unasked.
fn recorded(
    exchanges: Vec<serde_json::Value>,
) -> std::sync::Arc<purlis_core::forge::recorded::Recorded> {
    let text = serde_json::json!({"source": "the forge contract's recordings",
                                  "exchanges": exchanges});
    std::sync::Arc::new(
        purlis_core::forge::recorded::Recorded::parse(&text.to_string()).expect("a recording"),
    )
}

/// Q242: the app's press, through purlis's own GitHub client over the forge contract's
/// recordings, merges exactly the ask's request at its head; a press that names the head the
/// ask was at before the branch moved merges nothing, and asks the forge nothing.
#[test]
fn a_press_merges_through_the_forge_only_at_the_head_its_ask_is_at() {
    purlis_core::unsteered!();
    let project = Fake::new();
    assert!(matches!(
        answer(&project, &project.ask(12)),
        Answer::MergeHeld { .. }
    ));
    *project.read.lock().unwrap() = Ok(at(MOVED));
    assert!(matches!(
        answer(&project, &project.ask(12)),
        Answer::MergeHeld { .. }
    ));

    let untouched = recorded(Vec::new());
    let backend_of = |on: &OnForge| on.forge.backend_over(untouched.clone());
    assert_eq!(
        super::press(
            &project.holds,
            4,
            "o/r",
            12,
            HEAD,
            Seen::Protects,
            &backend_of
        ),
        Err(mergehold::NotMerged::NotHeld.to_string()),
        "the ask at the old head is gone"
    );
    assert_eq!(project.holds.every().len(), 1);

    *project.read.lock().unwrap() = Ok(at(HEAD));
    assert!(matches!(
        answer(&project, &project.ask(12)),
        Answer::MergeHeld { .. }
    ));
    asked_once_more(&project);
    let transport = recorded(a_github_merge_at_head());
    let backend_of = |on: &OnForge| on.forge.backend_over(transport.clone());
    assert_eq!(
        super::press(
            &project.holds,
            4,
            "o/r",
            12,
            HEAD,
            Seen::Protects,
            &backend_of
        ),
        Ok(())
    );
    assert!(project.holds.every().is_empty());
    assert_eq!(transport.unspent().len(), 0, "every recorded call was made");
}

/// Q242: the forge a press reaches is the one its ask was raised on, by host and path: two
/// forges' `o/r` in one project are two repos, and a press on one never reaches the other.
#[test]
fn a_press_reaches_the_forge_its_ask_was_raised_on_by_host_and_path() {
    use purlis_core::forge::{Forge, Kind};
    let holds = MergeHolds::default();
    let on = |host: &str| OnForge {
        forge: Forge {
            kind: Kind::GitHub,
            host: host.to_owned(),
        },
        path: "o/r".to_owned(),
    };
    let key = |head: &str| mergehold::MergeKey {
        repo: "o/r".try_into().expect("a repo"),
        number: 12,
        head: head.try_into().expect("a head"),
        base: "main".try_into().expect("a base"),
    };
    // Chat 4's ask is of github.com's o/r; chat 5's, raised after it, of a GitHub Enterprise's.
    holds.holds.raise(4, key(HEAD), facts(), 1_000);
    holds.on_forge(4, &key(HEAD), on("github.com"));
    holds.holds.raise(5, key(MOVED), facts(), 1_000);
    holds.on_forge(5, &key(MOVED), on("ghe.example.com"));

    let reached = Mutex::new(Vec::new());
    let backend_of = |on: &OnForge| {
        reached.lock().unwrap().push(on.forge.host.clone());
        on.forge.backend_over(recorded(Vec::new()))
    };
    let _ = super::press(&holds, 4, "o/r", 12, HEAD, Seen::Protects, &backend_of);
    let _ = super::press(&holds, 5, "o/r", 12, MOVED, Seen::Protects, &backend_of);
    assert_eq!(*reached.lock().unwrap(), ["github.com", "ghe.example.com"]);
}

/// R-118: a merge of a repo the project's inventory does not list is refused, and no ask is
/// raised or recorded for it, though its clone is in the project and the forge would answer.
#[test]
fn a_merge_ask_for_a_repo_outside_the_inventory_is_refused() {
    let project = Fake::new();
    project.lists("o/listed", "https://github.com/o/listed");
    let why = refused(answer(&project, &project.ask(12)));
    assert!(
        why.contains("o/r on github.com is not one of this project's repos"),
        "{why}"
    );
    assert!(project.holds.every().is_empty(), "no ask");
    assert_eq!(project.holds_recorded(), 0, "no hold recorded");
}

/// R-119: what a merge ask shows of its request, its title, author, base and checks, is what
/// the forge said, whatever the chat's command said of them.
#[test]
fn a_merge_asks_title_author_base_and_checks_come_from_the_forge() {
    let project = Fake::new();
    let command = "gh pr merge 12 --subject 'Fix a typo' --body 'checks are green' \
                   --author-email trusted@example.com --match-head-commit \
                   89abcdef0123456789abcdef0123456789abcdef";
    let Some(mergehold::Held::Request(request)) = mergehold::held(command) else {
        panic!("the command is a held merge of one request");
    };
    let mut ask = project.ask(12);
    ask.request = request;
    assert!(matches!(answer(&project, &ask), Answer::MergeHeld { .. }));

    let waiting = project.holds.every();
    let [one] = waiting.as_slice() else {
        panic!("one ask: {waiting:?}");
    };
    assert_eq!(one.facts.title, "Add the parser");
    assert_eq!(one.facts.author, "octo");
    assert_eq!(one.key.base.as_str(), "main");
    assert_eq!(one.facts.checks.ci, Ci::Passed);
    assert_eq!(
        one.key.head.as_str(),
        HEAD,
        "the forge's head, not the chat's"
    );
}

/// The forge contract's GitHub recording `case`, of acme/api at its head, as of this project's
/// `o/r` at [`HEAD`].
fn github_contract(case: &str) -> Vec<serde_json::Value> {
    let contract = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/purlis-core/tests/forge_contract/github")
        .join(format!("{case}.json"));
    let text = std::fs::read_to_string(contract).expect("the contract's recording");
    let text = text
        .replace("6dcb09b5b57875f334f61aebed695e2e4193db5e", HEAD)
        .replace(
            r#""owner",
              "acme""#,
            r#""owner",
              "o""#,
        )
        .replace(
            r#""name",
              "api""#,
            r#""name",
              "r""#,
        );
    serde_json::from_str::<serde_json::Value>(&text).expect("json")["exchanges"]
        .as_array()
        .expect("its exchanges")
        .clone()
}

/// Q239 (S1), carried from T1-11: the app's press, through `backend_of` and purlis's own GitHub
/// client, puts the request of a repo that lands through a merge queue in the queue at the ask's
/// head, and never merges it past the queue.
#[test]
fn a_press_on_a_repo_with_a_merge_queue_queues_it_at_the_asks_head_through_the_forge() {
    purlis_core::unsteered!();
    let project = Fake::new();
    assert!(matches!(
        answer(&project, &project.ask(12)),
        Answer::MergeHeld { .. }
    ));
    let mut exchanges = vec![a_github_merge_at_head().remove(0)];
    exchanges.extend(github_contract("lands_through_queue"));
    exchanges.extend(github_contract("enqueue_at"));
    let transport = recorded(exchanges);
    asked_once_more(&project);
    let backend_of = |on: &OnForge| on.forge.backend_over(transport.clone());

    assert_eq!(
        super::press(
            &project.holds,
            4,
            "o/r",
            12,
            HEAD,
            Seen::Protects,
            &backend_of
        ),
        Ok(())
    );
    assert!(project.holds.every().is_empty());
    assert_eq!(
        transport.unspent().len(),
        0,
        "every recorded call was made, and no merge past the queue was asked: {:?}",
        transport.unspent()
    );
}

/// Q248: a try again at the same head is the one ask still, and the forge's new read replaces
/// what it shows: checks that were running when it was raised and have passed since show as
/// passed. It waits since it was first raised.
#[test]
fn a_try_again_at_the_same_head_replaces_the_asks_facts_with_the_new_read() {
    let project = Fake::new();
    let mut running = at(HEAD);
    running.facts.checks.ci = Ci::Running;
    *project.read.lock().unwrap() = Ok(running);
    assert!(matches!(
        answer(&project, &project.ask(12)),
        Answer::MergeHeld { .. }
    ));
    let since = project.holds.every()[0].since;

    let mut passed = at(HEAD);
    passed.facts.read_at = 50;
    *project.read.lock().unwrap() = Ok(passed);
    assert!(matches!(
        answer(&project, &project.ask(12)),
        Answer::MergeHeld { .. }
    ));

    let waiting = project.holds.every();
    let [one] = waiting.as_slice() else {
        panic!("one ask: {waiting:?}");
    };
    assert_eq!(one.facts.checks.ci, Ci::Passed, "the new read's checks");
    assert_eq!(one.facts.read_at, 50, "when the new read was made");
    assert_eq!(one.since, since, "still the ask first raised");
    assert_eq!(project.holds_recorded(), 1, "no second hold");
}

/// Q269: a press is held to the ruleset state its own fresh read gives. Here that read finds no
/// ruleset, though the ask still shows the one the person was asked of (the read is dated before
/// it, so it never replaces it): the press merges nothing, and asks the forge for no merge.
#[test]
fn a_press_whose_fresh_read_disagrees_with_what_was_shown_is_refused() {
    purlis_core::unsteered!();
    let project = Fake::new();
    assert!(matches!(
        answer(&project, &project.ask(12)),
        Answer::MergeHeld { .. }
    ));
    asked_once_more(&project);
    let mut none = at(HEAD);
    none.facts.ruleset = mergehold::Ruleset::None;
    none.facts.read_at = 30;
    *project.read.lock().unwrap() = Ok(none);
    let reached = Mutex::new(0);
    let backend_of = |on: &OnForge| {
        *reached.lock().unwrap() += 1;
        on.forge.backend_over(recorded(Vec::new()))
    };

    assert_eq!(
        super::pressed_in(
            &project,
            4,
            "o/r",
            12,
            HEAD,
            Seen::Protects,
            &facts().digest(),
            &backend_of
        ),
        Err(mergehold::NotMerged::RulesetChanged.to_string())
    );
    assert_eq!(
        *reached.lock().unwrap(),
        0,
        "the forge was asked for no merge"
    );
    assert_eq!(project.holds.every().len(), 1, "the ask still waits");
}

/// T1-41 (R-302): View changes shows the request's changes, read in the clone the merge ran in,
/// and marks its head viewed, only while the forge says the request is at the head its row
/// shows. A head that moved is viewed by nobody.
#[test]
fn view_changes_marks_the_head_viewed_only_at_the_head_the_forge_is_at() {
    purlis_core::unsteered!();
    let project = Fake::new();
    assert!(matches!(
        answer(&project, &project.ask(12)),
        Answer::MergeHeld { .. }
    ));
    assert_eq!(
        project.holds.every()[0].look,
        mergehold::Look::NotViewed { since: None }
    );

    let changes = super::viewed(&project, 4, "o/r", 12, HEAD).expect("its changes");

    assert_eq!(changes, "diff of main...0123456");
    assert_eq!(project.holds.every()[0].look, mergehold::Look::Viewed);

    let other = Fake::new();
    assert!(matches!(
        answer(&other, &other.ask(12)),
        Answer::MergeHeld { .. }
    ));
    *other.read.lock().unwrap() = Ok(at(MOVED));
    assert!(super::viewed(&other, 4, "o/r", 12, HEAD).is_err());
    assert_eq!(
        other.holds.every()[0].look,
        mergehold::Look::NotViewed { since: None },
        "nothing is marked viewed at a head the forge is no longer at"
    );
}

/// The follow-up Q269 named: a press is bound to every fact its row showed, by the digest of
/// them. Shown passing checks, a press whose fresh read finds them failed merges nothing and
/// asks the forge for no merge.
#[test]
fn a_press_shown_other_facts_than_its_fresh_read_is_refused() {
    purlis_core::unsteered!();
    let project = Fake::new();
    assert!(matches!(
        answer(&project, &project.ask(12)),
        Answer::MergeHeld { .. }
    ));
    asked_once_more(&project);
    let shown = facts().digest();
    let mut failed = at(HEAD);
    failed.facts.checks.ci = Ci::Failed;
    failed.facts.read_at = 90;
    *project.read.lock().unwrap() = Ok(failed);
    let reached = Mutex::new(0);
    let backend_of = |on: &OnForge| {
        *reached.lock().unwrap() += 1;
        on.forge.backend_over(recorded(Vec::new()))
    };

    let refused = super::pressed_in(
        &project,
        4,
        "o/r",
        12,
        HEAD,
        Seen::Protects,
        &shown,
        &backend_of,
    )
    .expect_err("nothing merged");

    assert!(refused.contains("nothing was merged"), "{refused}");
    assert_eq!(
        *reached.lock().unwrap(),
        0,
        "the forge was asked for no merge"
    );
    assert_eq!(project.holds.every().len(), 1, "the ask still waits");
}
