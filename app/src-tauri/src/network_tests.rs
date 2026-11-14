// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Settings' Network page, its Blocked log (T1-32) and the Allow on its rows (#1662), from a
//! plane's text, the Blocked log's blocks and a network record under a temporary data home.

use super::*;
use purlis_core::eventlog::stops::{Block as StopBlock, Qualifier, Source, Target};
use purlis_core::sandboxblock::{Block, Kind, Operation};
use std::path::Path;

const HOST: Block = Block {
    operation: Operation::Connect,
    kind: Kind::Host,
    ours: false,
};

fn plane(text: &str) -> sandbox::Plane {
    sandbox::Plane::of(Some(text))
}

fn chat(name: &str) -> record::Chat {
    record::Chat {
        id: Some(format!("id-{name}")),
        name: Some(name.to_owned()),
        session: Some(2),
    }
}

/// This thread's policy until it is dropped, a panic included.
struct Under;

impl Under {
    fn policy(json: &str) -> Self {
        sandbox::policy::set_for_this_test(sandbox::policy::Locks::parse(
            json,
            std::path::Path::new("/etc/purlis/policy.json"),
        ));
        Self
    }
}

impl Drop for Under {
    fn drop(&mut self) {
        sandbox::policy::set_for_this_test(sandbox::policy::Locks::none());
    }
}

#[test]
fn open_hosts_name_every_host_of_each_preset_in_force_and_then_the_projects_own() {
    let dir = tempfile::tempdir().expect("a project");
    let plane = plane(
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n\
         hosts = [\"api.example.com\"]\n\n[[forge]]\nkind = \"gitlab\"\nhost = \"git.example.org\"\n",
    );
    let shown = network_of(dir.path(), &plane, &[], &|_| None, &|_| None);
    assert!(shown.on);
    let none = sandbox::policy::Locks::none();
    assert_eq!(
        shown.open,
        vec![
            OpenHosts {
                title: "AI providers".to_owned(),
                hosts: sandbox::hosts(&[sandbox::Preset::ModelProviders], &plane, &none),
            },
            OpenHosts {
                title: "Code hosting".to_owned(),
                hosts: sandbox::hosts(&[sandbox::Preset::Forge], &plane, &none),
            },
            OpenHosts {
                title: "This project's hosts".to_owned(),
                hosts: vec!["api.example.com".to_owned()],
            },
        ]
    );
    assert!(shown.open[1].hosts.contains(&"git.example.org".to_owned()));
    assert!(shown.open.iter().all(|one| !one.hosts.is_empty()));
}

#[test]
fn a_project_whose_chats_are_not_sandboxed_lists_nothing() {
    let dir = tempfile::tempdir().expect("a project");
    let listed = [listed(
        1,
        10,
        "id-first",
        sandbox_host("a.example:443", None),
    )];
    let shown = network_of(
        dir.path(),
        &plane("schema = 1\n"),
        &listed,
        &|_| None,
        &|_| None,
    );
    assert_eq!(
        shown,
        SandboxNetwork {
            on: false,
            open: Vec::new(),
            blocked: Vec::new(),
        }
    );
}

/// A block of the Blocked log, as [`blockedlog::listed`] gives it: event `seq` of chat `chat`,
/// written at `at` seconds since 1970.
fn listed(seq: u64, at: u64, chat: &str, block: StopBlock) -> Listed {
    Listed {
        at_ms: at * 1000,
        seq,
        chat: chat.to_owned(),
        block,
    }
}

/// A sandbox block of a connection to `host`, `qualifier` as its report is worth.
fn sandbox_host(host: &str, qualifier: Option<Qualifier>) -> StopBlock {
    StopBlock {
        tool: "Bash".to_owned(),
        source: Source::Sandbox,
        qualifier,
        target: Some(Target::host(host).expect("a host")),
        args: None,
    }
}

/// The name each fixture chat is shown under.
fn named(chat: &str) -> Option<String> {
    chat.strip_prefix("id-").map(str::to_owned)
}

#[test]
fn the_blocked_log_lists_each_target_once_newest_first_with_its_chat_and_allow() {
    let dir = tempfile::tempdir().expect("a project");
    // Newest first, as the Blocked log gives them.
    let listed = [
        listed(6, 50, "id-first", {
            let mut guard = sandbox_host("a.example:443", None);
            guard.source = Source::Guard;
            guard.target = Some(Target::guard_arm("A3c").expect("an arm"));
            guard
        }),
        listed(5, 40, "id-second", sandbox_host("b.example:5432", None)),
        listed(4, 30, "id-second", sandbox_host("a.example:443", None)),
        listed(3, 20, "id-first", sandbox_host("api.example.com:443", None)),
        listed(2, 10, "id-first", sandbox_host("a.example:443", None)),
    ];
    let plane = plane("[sandbox]\nmode = \"on\"\negress = []\nhosts = [\"api.example.com\"]\n");
    let shown = network_of(dir.path(), &plane, &listed, &named, &|_| None);
    /// A row as the test reads it: target, what it says, chat, when, how often, and whether it
    /// is reached.
    type Read<'a> = (Option<&'a str>, &'a str, Option<&'a str>, u32, u32, bool);
    let rows: Vec<Read<'_>> = shown
        .blocked
        .iter()
        .map(|row| {
            (
                row.target.as_deref(),
                row.said.as_str(),
                row.chat.as_deref(),
                row.at,
                row.times,
                row.reached,
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            (
                Some("A3c"),
                "blocked by purlis's guards",
                Some("first"),
                50,
                1,
                false
            ),
            (
                Some("b.example:5432"),
                "blocked by the sandbox",
                Some("second"),
                40,
                1,
                false
            ),
            (
                Some("a.example:443"),
                "blocked by the sandbox",
                Some("second"),
                30,
                2,
                false
            ),
            (
                Some("api.example.com:443"),
                "blocked by the sandbox",
                Some("first"),
                20,
                1,
                true
            ),
        ]
    );
    // A guard's arm is nothing a host's Allow mends.
    assert_eq!(shown.blocked[0].levels, Vec::new());
    assert_eq!(
        shown.blocked[1].levels,
        vec![GrantLevel::You, GrantLevel::Project]
    );
    // Reached already: nothing to allow.
    assert_eq!(shown.blocked[3].levels, Vec::new());
    assert!(shown.blocked.iter().all(|row| row.looked_up.is_none()));
}

#[test]
fn a_blocked_log_row_names_its_chat_s_number_only_where_that_chat_is_open_here() {
    let dir = tempfile::tempdir().expect("a project");
    // Newest first: chat "second" is open here as chat 7; chat "first" is not open.
    let listed = [
        listed(3, 30, "id-second", sandbox_host("b.example:5432", None)),
        listed(2, 20, "id-first", sandbox_host("a.example:443", None)),
    ];
    let plane = plane("[sandbox]\nmode = \"on\"\negress = []\n");
    let shown = network_of(dir.path(), &plane, &listed, &named, &|chat| {
        (chat == "id-second").then_some(7)
    });
    let rows: Vec<(Option<&str>, Option<u32>)> = shown
        .blocked
        .iter()
        .map(|row| (row.chat.as_deref(), row.session))
        .collect();
    // What its Chat log opens by (R-226): the open chat's number, and none for the other.
    assert_eq!(rows, vec![(Some("second"), Some(7)), (Some("first"), None)]);
}

#[test]
fn the_blocked_log_offers_no_allow_policy_forbids() {
    let dir = tempfile::tempdir().expect("a project");
    let listed = [listed(1, 10, "id-one", sandbox_host("a.example:443", None))];
    let _under = Under::policy(r#"{"owner": "IT", "sandbox": {"hosts": ["*.corp.example"]}}"#);
    let shown = network_of(
        dir.path(),
        &plane("[sandbox]\nmode = \"on\"\negress = []\n"),
        &listed,
        &named,
        &|_| None,
    );
    assert_eq!(shown.blocked.len(), 1);
    assert_eq!(shown.blocked[0].levels, Vec::new());
}

/// A host a chat's own hook reported (Q254: unverified) may be a program's own printed words, a
/// refused lookup's among them (#1663): the Blocked log shows it as text and offers no Allow on
/// it. Only a connection purlis's own proxy refused is offered.
#[test]
fn the_blocked_log_offers_allow_only_on_a_host_the_proxy_refused() {
    let dir = tempfile::tempdir().expect("a project");
    let listed = [
        listed(
            2,
            20,
            "id-one",
            sandbox_host("db.example.com", Some(Qualifier::Unverified)),
        ),
        listed(1, 10, "id-one", sandbox_host("api.example.com:443", None)),
    ];
    let shown = network_of(
        dir.path(),
        &plane("[sandbox]\nmode = \"on\"\negress = []\n"),
        &listed,
        &named,
        &|_| None,
    );
    let rows: Vec<(Option<&str>, &[GrantLevel])> = shown
        .blocked
        .iter()
        .map(|row| (row.target.as_deref(), row.levels.as_slice()))
        .collect();
    assert_eq!(
        rows,
        vec![
            (Some("db.example.com"), &[][..]),
            (
                Some("api.example.com:443"),
                &[GrantLevel::You, GrantLevel::Project][..]
            ),
        ]
    );
}

/// Q265: in a group, the newest *verified* block decides the Allow. A later line a chat's own
/// hook reported (unverified, Q254) joins the row's count and names its newest chat, and never
/// hides the Allow an older block purlis's own proxy saw offers.
///
/// Mutations it catches: the row's Allow taken from its newest block alone (none offered), the
/// count or the newest chat taken from the verified block.
#[test]
fn in_a_group_the_newest_verified_block_decides_the_allow() {
    let dir = tempfile::tempdir().expect("a project");
    let listed = [
        listed(
            2,
            20,
            "id-second",
            sandbox_host("a.example:443", Some(Qualifier::Unverified)),
        ),
        listed(1, 10, "id-first", sandbox_host("a.example:443", None)),
    ];
    let shown = network_of(
        dir.path(),
        &plane("[sandbox]\nmode = \"on\"\negress = []\n"),
        &listed,
        &named,
        &|_| None,
    );
    assert_eq!(shown.blocked.len(), 1, "{:?}", shown.blocked);
    let row = &shown.blocked[0];
    assert_eq!(row.target.as_deref(), Some("a.example:443"));
    assert_eq!((row.times, row.at), (2, 20));
    assert_eq!(row.chat.as_deref(), Some("second"));
    assert_eq!(row.levels, vec![GrantLevel::You, GrantLevel::Project]);
}

/// **R-222: Blocked lately is folded into the Blocked log** (T1-32, Q264). A network-record
/// fixture, each refusal also heard as T1-10 writes it: what ended as a block (kept blocked,
/// withdrawn at the chat's end) is listed from its block event with the host its target summary
/// names (R-220, Q206); an open ask has no block yet and stays in Asks, an allowed one never
/// becomes a block, and another project's chat is not this one's. The record is written and
/// never read: the rows are the log's.
///
/// Mutations it catches: the rows read from the record (the open and allowed asks listed), the
/// project filter dropped (the other chat's host listed), the host read from anything but the
/// target summary.
#[test]
fn blocked_lately_is_folded_into_the_blocked_log() {
    use purlis_core::eventlog::stops::{Answer, AskAnswered, AskGroup, Stop};
    use purlis_core::eventlog::{ArgsKey, Log, Recorder, RunOf};
    use purlis_core::sandboxblock::stop::{Reported, sandbox_block};

    const OURS: &str = "01K6CHAT000000000000000004";
    const THEIRS: &str = "01K6CHAT000000000000000005";
    let home = tempfile::tempdir().expect("a home");
    let root = home.path().join("project");
    let elsewhere = home.path().join("elsewhere");
    std::fs::create_dir_all(&root).expect("a project");
    std::fs::create_dir_all(&elsewhere).expect("another project");
    let record = Record::in_data(&home.path().join("data"));
    let log = home.path().join("events");
    let mut recorder = Recorder::new(
        Log::open(&log, "01J9ZZDEVICE0000000000000").expect("a log"),
        ArgsKey::open(&log).expect("a key"),
    );
    recorder.knows(
        &root,
        2,
        RunOf {
            chat: OURS,
            run: "01K6RUN0000000000000000004",
        },
    );
    recorder.knows(
        &elsewhere,
        3,
        RunOf {
            chat: THEIRS,
            run: "01K6RUN0000000000000000005",
        },
    );
    let socket = Block {
        operation: Operation::Connect,
        kind: Kind::LocalSocket,
        ours: false,
    };
    // What the record's Blocked lately listed, and whether each ended as a block.
    let heard: [(&Path, u32, Block, &str, Option<Reported>); 6] = [
        // Kept blocked: purlis's proxy held it, and the person said no.
        (
            root.as_path(),
            2,
            HOST,
            "api.example.com:443",
            Some(Reported::ByTheProxy),
        ),
        // The chat's own hook reported it, and the chat moved on.
        (
            root.as_path(),
            2,
            HOST,
            "cdn.example.net:8443",
            Some(Reported::ByItsHook),
        ),
        (
            root.as_path(),
            2,
            socket,
            "/var/run/docker.sock",
            Some(Reported::ByItsHook),
        ),
        // Still an open ask: no block yet.
        (root.as_path(), 2, HOST, "open.example:443", None),
        // Allowed: an answer, never a block.
        (root.as_path(), 2, HOST, "allowed.example:443", None),
        // Another project's chat.
        (
            elsewhere.as_path(),
            3,
            HOST,
            "theirs.example:443",
            Some(Reported::ByTheProxy),
        ),
    ];
    for (at, (project, number, block, named, ended)) in (10..).zip(heard) {
        record
            .write(
                project,
                &Entry::blocked(&block, Some(named), chat("fix the build"), None, at),
            )
            .expect("recorded");
        if let Some(reported) = ended {
            let stop = sandbox_block(block.kind, Some(named), project, Some("Bash"), reported);
            recorder
                .stop(project, Some(number), &stop)
                .expect("a block");
        }
        if named == "allowed.example:443" {
            let answered = Stop::AskAnswered(AskAnswered {
                ask: AskGroup::Sandbox,
                answer: Answer::Allowed,
            });
            recorder
                .stop(project, Some(number), &answered)
                .expect("an answer");
        }
    }
    drop(recorder);

    let now_ms = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("after 1970")
            .as_millis(),
    )
    .expect("a time");
    let listed = purlis_core::eventlog::blockedlog::listed_in(&log, now_ms, &|chat| chat == OURS)
        .expect("the log");
    let shown = network_of(
        &root,
        &plane("[sandbox]\nmode = \"on\"\negress = []\n"),
        &listed,
        &|chat| (chat == OURS).then(|| "fix the build".to_owned()),
        &|_| None,
    );
    let rows: Vec<(Option<&str>, Option<&str>, bool)> = shown
        .blocked
        .iter()
        .map(|row| {
            (
                row.target.as_deref(),
                row.chat.as_deref(),
                row.levels.is_empty(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            (Some("docker.sock"), Some("fix the build"), true),
            (Some("cdn.example.net:8443"), Some("fix the build"), true),
            (Some("api.example.com:443"), Some("fix the build"), false),
        ],
        "the Blocked log lists what ended as a block, by its target summary"
    );
}

/// The network record's lines of a connection refused to each of `targets`.
fn refused_lately(targets: &[&str]) -> Vec<Entry> {
    targets
        .iter()
        .map(|target| Entry::blocked(&HOST, Some(target), chat("first"), None, 1))
        .collect()
}

/// The Blocked log's blocks of a connection to each of `targets` purlis's own proxy refused.
fn verified(targets: &[&str]) -> Vec<Listed> {
    (1..)
        .zip(targets)
        .map(|(seq, target)| listed(seq, seq, "id-first", sandbox_host(target, None)))
        .collect()
}

/// Q265: Allow on the Blocked log needs a verified sandbox block of that host in the Blocked
/// log as well as the network record's refused connection (#1681), so the gate matches what the
/// row offers. A host only a chat's own hook reported (Q254), one only a guard blocked, and one
/// whose verified block is of another host are refused, and nothing is audited.
///
/// Mutations it catches: the Blocked log's check dropped (the record alone lets each through),
/// the qualifier or the source left out of it.
#[test]
fn allow_on_the_blocked_log_refuses_a_host_only_a_hook_reported() {
    let dir = tempfile::tempdir().expect("a project");
    let chats = crate::chats::Chats::new();
    let seen = std::sync::Mutex::new(Vec::new());
    let audit = audited_into(&seen);
    let hosts = ["hook.example:443", "guard.example:443", "other.example:443"];
    let lately = refused_lately(&hosts);
    let mut guarded = sandbox_host("guard.example:443", None);
    guarded.source = Source::Guard;
    let blocked_log = [
        listed(
            3,
            30,
            "id-first",
            sandbox_host("hook.example:443", Some(Qualifier::Unverified)),
        ),
        listed(2, 20, "id-first", guarded),
        listed(
            1,
            10,
            "id-first",
            sandbox_host("elsewhere.example:443", None),
        ),
    ];
    for host in hosts {
        let refused = allow_blocked(
            dir.path(),
            &chats,
            &lately,
            &blocked_log,
            host,
            GrantLevel::You,
            &audit,
            3,
        )
        .expect_err(host);
        assert!(
            refused.contains("not a connection the sandbox refused here"),
            "{refused}"
        );
    }
    assert!(seen.lock().expect("the audit").is_empty());
    assert!(sandbox::hosts::personal(dir.path()).is_empty());
}

/// #1681: Allow on the Blocked log allows only a host the network record has a refused
/// connection to: a host the record never refused, one only looked up, or one on another port is
/// not allowed from there, even with a verified block of it in the Blocked log, and nothing is
/// audited.
#[test]
fn allow_on_the_blocked_log_refuses_a_host_that_is_not_a_refused_connection_here() {
    let dir = tempfile::tempdir().expect("a project");
    let chats = crate::chats::Chats::new();
    let seen = std::sync::Mutex::new(Vec::new());
    let audit = audited_into(&seen);
    let mut lately = refused_lately(&["a.example:443"]);
    lately.push(Entry::blocked(
        &Block {
            operation: Operation::Lookup,
            kind: Kind::Host,
            ours: false,
        },
        Some("looked.example"),
        chat("first"),
        None,
        2,
    ));
    // Each is verified in the Blocked log: the record alone still refuses it.
    let blocked_log = verified(&[
        "never.example:443",
        "looked.example",
        "a.example:8443",
        "a.example:443",
    ]);
    for host in ["never.example:443", "looked.example", "a.example:8443"] {
        let refused = allow_blocked(
            dir.path(),
            &chats,
            &lately,
            &blocked_log,
            host,
            GrantLevel::You,
            &audit,
            3,
        )
        .expect_err(host);
        assert!(
            refused.contains("not a connection refused here lately"),
            "{refused}"
        );
    }
    let nothing: Vec<Entry> = Vec::new();
    assert!(
        allow_blocked(
            dir.path(),
            &chats,
            &nothing,
            &blocked_log,
            "a.example:443",
            GrantLevel::You,
            &audit,
            3
        )
        .is_err(),
        "with no record, nothing is allowed from it"
    );
    assert!(seen.lock().expect("the audit").is_empty());
    assert!(sandbox::hosts::personal(dir.path()).is_empty());
}

fn audited_into(
    seen: &std::sync::Mutex<Vec<(bool, String, String)>>,
) -> impl Fn(Option<u32>, &grant::Audited<'_>) -> Result<(), String> + '_ {
    move |_, audited| {
        seen.lock().expect("the audit").push((
            audited.granted,
            audited.target.to_owned(),
            audited.level.word().to_owned(),
        ));
        Ok(())
    }
}

#[test]
fn allow_on_the_blocked_log_is_judged_audited_and_never_for_one_chat() {
    let dir = tempfile::tempdir().expect("a project");
    let chats = crate::chats::Chats::new();
    let seen = std::sync::Mutex::new(Vec::new());
    let audit = audited_into(&seen);
    let lately = refused_lately(&["a.example:443"]);
    let blocked_log = verified(&["a.example:443"]);
    let refused = allow_blocked(
        dir.path(),
        &chats,
        &lately,
        &blocked_log,
        "a.example:443",
        GrantLevel::Chat,
        &audit,
        1,
    )
    .expect_err("chat");
    assert!(refused.contains("that chat's own Notice"), "{refused}");
    let refused = allow_blocked(
        dir.path(),
        &chats,
        &lately,
        &blocked_log,
        "169.254.169.254",
        GrantLevel::You,
        &audit,
        1,
    )
    .expect_err("never grantable");
    assert!(refused.contains("link-local"), "{refused}");
    let unaudited = |_: Option<u32>, _: &grant::Audited<'_>| Err("no event log".to_owned());
    assert!(
        allow_blocked(
            dir.path(),
            &chats,
            &lately,
            &blocked_log,
            "a.example:443",
            GrantLevel::You,
            &unaudited,
            1
        )
        .is_err()
    );
    assert!(sandbox::hosts::personal(dir.path()).is_empty());
    assert!(seen.lock().expect("the audit").is_empty());
}

/// Writes the project's local settings file, which a sandboxed run of this suite cannot: it
/// first runs on CI.
#[test]
fn allow_on_the_blocked_log_keeps_the_host_for_this_project_on_this_machine_and_records_it() {
    let dir = tempfile::tempdir().expect("a home");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("a project");
    let record = Record::in_data(&dir.path().join("data"));
    let chats = crate::chats::Chats::new();
    let seen = std::sync::Mutex::new(Vec::new());
    let audit = audited_into(&seen);
    let audit = recorded(&audit, Some(&record), &root, &chats);
    let lately = refused_lately(&["a.example:443"]);
    let blocked_log = verified(&["a.example:443"]);
    let said = allow_blocked(
        &root,
        &chats,
        &lately,
        &blocked_log,
        "A.Example:443",
        GrantLevel::You,
        &audit,
        5,
    )
    .expect("allowed");
    assert_eq!(
        said.said,
        "Allowed a.example:443 for me on this machine. A chat that is running reaches it from \
         its next start."
    );
    assert_eq!(
        *seen.lock().expect("the audit"),
        vec![(true, "a.example:443".to_owned(), "you".to_owned())]
    );
    assert_eq!(
        sandbox::hosts::personal(&root)
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        vec!["a.example:443".to_owned()]
    );
    let kept = record.read(&root, now_secs());
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert_eq!(kept[0].event, record::Event::Allow);
    assert_eq!(kept[0].target.as_deref(), Some("a.example:443"));
    assert_eq!(kept[0].scope, Some(record::Scope::You));
    assert_eq!(kept[0].who, Some(record::Who::You));
}

#[test]
fn a_removal_is_recorded_after_its_audit_and_not_without_one() {
    let dir = tempfile::tempdir().expect("a home");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("a project");
    let record = Record::in_data(&dir.path().join("data"));
    let chats = crate::chats::Chats::new();
    let refusing = |_: Option<u32>, _: &grant::Audited<'_>| Err("no event log".to_owned());
    let removed = grant::Audited {
        granted: false,
        what: "host",
        target: "a.example:443",
        level: grant::Level::Project,
    };
    assert!(recorded(&refusing, Some(&record), &root, &chats)(None, &removed).is_err());
    assert!(
        record.read(&root, now_secs()).is_empty(),
        "no audit, no record"
    );
    let passing = |_: Option<u32>, _: &grant::Audited<'_>| Ok(());
    recorded(&passing, Some(&record), &root, &chats)(None, &removed).expect("recorded");
    let kept = record.read(&root, now_secs());
    assert_eq!(kept[0].event, record::Event::Remove);
    assert_eq!(kept[0].scope, Some(record::Scope::Project));
}

#[test]
fn a_chat_that_is_not_open_has_nothing_to_say() {
    let dir = tempfile::tempdir().expect("a project");
    let shown = chat_network_of(dir.path(), &crate::chats::Chats::new(), 4, &[], None, 100);
    assert_eq!(
        shown,
        ChatNetwork {
            open: false,
            sandboxed: false,
            reach: Vec::new(),
            browser_sites: Vec::new(),
            refused: Vec::new(),
            local_ports: None,
        }
    );
}

/// #2364: a chat's Network view lists the sites only its browser reaches apart from its
/// commands' hosts: those allowed for this chat alone, then those kept for every chat of this
/// project on this machine, each once. Another chat's are never its.
#[test]
fn a_chats_browser_sites_are_its_own_then_this_projects_each_once_and_never_another_chats() {
    use purlis_core::sandbox::browser_reach::Site;
    let site = |written: &str| Site::parse(written).expect("a site");

    let shown = browser_sites_of(
        &[
            (4, site("news.example.org:443")),
            (5, site("theirs.example:443")),
            (4, site("docs.example.com:443")),
        ],
        4,
        &[site("docs.example.com:443"), site("kept.example:443")],
    );

    assert_eq!(
        shown,
        [
            "news.example.org:443",
            "docs.example.com:443",
            "kept.example:443"
        ]
    );
}

#[test]
fn a_chat_reaches_each_host_by_why_open_persona_or_allowed() {
    let reached = reach_of(
        &[
            "api.anthropic.com".to_owned(),
            "db.internal:5432".to_owned(),
            "extra.example:443".to_owned(),
        ],
        &["api.anthropic.com".to_owned()],
        &["db.internal:5432".to_owned()],
    );
    assert_eq!(
        reached.iter().map(|one| one.by).collect::<Vec<_>>(),
        vec![ReachedBy::Open, ReachedBy::Persona, ReachedBy::Allowed]
    );
}

#[test]
fn a_chat_is_shown_only_its_own_refusals_by_its_id_across_its_numbers() {
    let mine = record::Chat {
        id: Some("01MINE".to_owned()),
        name: Some("mine".to_owned()),
        session: Some(7),
    };
    let other_with_my_number = record::Chat {
        id: Some("01OTHER".to_owned()),
        name: Some("other".to_owned()),
        session: Some(3),
    };
    let entries = vec![
        Entry::blocked(&HOST, Some("a.example:443"), mine.clone(), None, 10),
        Entry::blocked(&HOST, Some("b.example:443"), other_with_my_number, None, 20),
        Entry::blocked(
            &HOST,
            Some("c.example:443"),
            record::Chat {
                session: Some(3),
                ..mine
            },
            None,
            30,
        ),
    ];
    let rows = refused_of(
        &entries,
        (Some("01MINE"), 3),
        &["c.example".to_owned()],
        &sandbox::policy::Locks::none(),
    );
    assert_eq!(
        rows.iter()
            .map(|row| (row.target.as_deref(), row.reached, row.chat.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            (Some("c.example:443"), true, None),
            (Some("a.example:443"), false, None),
        ]
    );
}

/// What a Settings write said, for [`record_settings_host`]: saved with `added` or `removed`.
fn written(added: Option<&str>, removed: Option<&str>) -> crate::settings::EntryWritten {
    crate::settings::EntryWritten::Saved {
        file: crate::settings::SettingsFile {
            which: crate::settings::SettingsWhich::Local,
            file: "purlis.local.toml".to_owned(),
            exists: true,
            text: String::new(),
            refusals: Vec::new(),
            parsed: true,
            fields: Vec::new(),
            entries: None,
        },
        added: added.map(str::to_owned),
        removed: removed.map(|host| {
            vec![crate::settings::EntryValue {
                field: "host".to_owned(),
                value: host.to_owned(),
            }]
        }),
    }
}

/// #1681: a host added or confirmed in Settings is recorded as an Allow, and one removed as a
/// removal, at your scope or everyone's as the file is, by you; a write Settings refused is not
/// recorded, and nor is anything with no record.
#[test]
fn a_host_added_confirmed_or_removed_in_settings_is_recorded_at_its_files_scope() {
    use crate::settings::SettingsWhich;
    let dir = tempfile::tempdir().expect("a home");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("a project");
    let record = Record::in_data(&dir.path().join("data"));
    let local = SettingsWhich::Local;
    record_settings_host(
        Some(&record),
        &root,
        &written(Some("mine.example"), None),
        true,
        local,
    );
    record_settings_host(
        Some(&record),
        &root,
        &written(Some("team.example:8443"), None),
        true,
        SettingsWhich::Shared,
    );
    record_settings_host(
        Some(&record),
        &root,
        &written(None, Some("mine.example")),
        false,
        local,
    );
    // Refused, or saved with nothing named: nothing is recorded.
    let refused = crate::settings::EntryWritten::Refused {
        fields: Vec::new(),
        referrers: Vec::new(),
        reasons: vec!["no".to_owned()],
    };
    record_settings_host(Some(&record), &root, &refused, true, local);
    record_settings_host(Some(&record), &root, &written(None, None), false, local);
    record_settings_host(None, &root, &written(Some("x.example"), None), true, local);

    let kept: Vec<_> = record
        .read(&root, now_secs())
        .into_iter()
        .map(|entry| (entry.event, entry.target, entry.scope, entry.who))
        .collect();
    assert_eq!(
        kept,
        [
            (
                record::Event::Allow,
                Some("mine.example".to_owned()),
                Some(record::Scope::You),
                Some(record::Who::You)
            ),
            (
                record::Event::Allow,
                Some("team.example:8443".to_owned()),
                Some(record::Scope::Project),
                Some(record::Who::You)
            ),
            (
                record::Event::Remove,
                Some("mine.example".to_owned()),
                Some(record::Scope::You),
                Some(record::Who::You)
            ),
        ]
    );
}

#[test]
fn a_claude_code_chat_says_where_an_administrator_s_settings_open_local_ports() {
    // #1699: an administrator's managed Claude Code settings outrank purlis's, and the chat's
    // Network view says so, naming the file; nothing where none turns local binding on.
    use purlis_core::sandbox::claude::LocalBinding;
    use std::path::PathBuf;
    assert_eq!(local_ports_of(&LocalBinding::Off), None);
    let on = local_ports_of(&LocalBinding::On(PathBuf::from(
        "/Library/Application Support/ClaudeCode/managed-settings.json",
    )))
    .expect("said");
    assert!(
        on.contains("/Library/Application Support/ClaudeCode/managed-settings.json")
            && on.contains("every port on this machine"),
        "{on}"
    );
    let unread = local_ports_of(&LocalBinding::Unread {
        file: PathBuf::from("/etc/claude-code/managed-settings.json"),
        why: "it is not JSON purlis can read".to_owned(),
    })
    .expect("said");
    assert!(
        unread.contains("could not read /etc/claude-code/managed-settings.json"),
        "{unread}"
    );
}

/// #1681: a started, sandboxed chat's Network view, end to end on the app's own chats (a
/// pretend session host, so no terminal): it is open and sandboxed, it reaches every host its
/// sandbox was compiled with, the project's own as Open, and it is shown its own refusals by
/// its id, never another chat's. Writes a manifest, so it first runs on CI.
#[test]
fn a_started_sandboxed_chat_shows_what_its_sandbox_reaches_and_only_its_own_refusals() {
    let project = tempfile::tempdir().expect("a project");
    std::fs::write(
        project.path().join(purlis_core::plane::MANIFEST),
        "[sandbox]\nmode = \"on\"\negress = []\nhosts = [\"api.example.com\"]\n",
    )
    .expect("the project's file");
    let chats = crate::chats::tests::restartable_chats_of(
        project.path(),
        crate::host::pretend::Pretend::default(),
    );
    let session = chats
        .start(
            &crate::chats::tests::a_resumable_chat_in(project.path()),
            purlis_core::engine::Size {
                columns: 80,
                rows: 24,
            },
        )
        .expect("starts");
    let compiled = chats.confines_of(session).expect("it started sandboxed");
    let id = chats.record().chats[0].identity.id.clone();
    assert!(id.is_some(), "a started chat has an id");

    let data = tempfile::tempdir().expect("a data home");
    let record = Record::in_data(data.path());
    let refused = |target: &str, id: Option<String>, at: u64| {
        record
            .write(
                project.path(),
                &Entry::blocked(
                    &HOST,
                    Some(target),
                    record::Chat {
                        id,
                        name: Some("a chat".to_owned()),
                        session: Some(session),
                    },
                    None,
                    at,
                ),
            )
            .expect("written");
    };
    refused("refused.example:443", id.clone(), 50);
    // Another chat that once had this number: never shown as this one.
    refused("theirs.example:443", Some("01SOMEONE-ELSE".to_owned()), 60);

    let shown = chat_network_of(project.path(), &chats, session, &[], Some(&record), 100);

    assert!(shown.open && shown.sandboxed, "{shown:?}");
    assert_eq!(
        shown
            .reach
            .iter()
            .map(|one| one.host.clone())
            .collect::<Vec<_>>(),
        compiled.hosts,
        "every host it was started with, in its sandbox's order"
    );
    assert!(
        shown
            .reach
            .iter()
            .any(|one| one.host == "api.example.com" && one.by == ReachedBy::Open),
        "{:?}",
        shown.reach
    );
    assert_eq!(
        shown
            .refused
            .iter()
            .map(|row| (row.target.as_deref(), row.reached, row.chat.as_deref()))
            .collect::<Vec<_>>(),
        vec![(Some("refused.example:443"), false, None)]
    );
    assert!(!shown.refused[0].levels.is_empty(), "Allow is offered");
    // Claude Code takes an administrator's managed settings, as the doctor reads them here.
    assert_eq!(
        shown.local_ports,
        local_ports_of(&sandbox::claude::administrators_local_binding())
    );
    chats.close(session).expect("closed");
}

/// #2692: the network record shows a chat as sandboxed only by the record of how its run
/// started, with its compile held: a run with no record is shown as unsandboxed, with nothing
/// open or blocked. Writes a manifest, so it first runs on CI.
#[test]
fn a_chat_with_no_record_of_how_its_run_started_is_shown_as_unsandboxed() {
    let project = tempfile::tempdir().expect("a project");
    std::fs::write(
        project.path().join(purlis_core::plane::MANIFEST),
        "[sandbox]\nmode = \"on\"\negress = []\nhosts = [\"api.example.com\"]\n",
    )
    .expect("the project's file");
    let chats = crate::chats::tests::restartable_chats_of(
        project.path(),
        crate::host::pretend::Pretend::default(),
    );
    let session = chats
        .start(
            &crate::chats::tests::a_resumable_chat_in(project.path()),
            purlis_core::engine::Size {
                columns: 80,
                rows: 24,
            },
        )
        .expect("starts");
    let compiled = chats.confines_of(session).expect("it started sandboxed");
    assert!(chat_network_of(project.path(), &chats, session, &[], None, 100).sandboxed);

    chats.runs_as_recorded(session, Some(compiled), None);
    let shown = chat_network_of(project.path(), &chats, session, &[], None, 100);
    assert!(shown.open && !shown.sandboxed, "{shown:?}");
    assert!(shown.reach.is_empty());
}

// ---- each chat's browser-site count, kept (#4739) -------------------------------------------

/// A [`SitesInForce`] with no policy and no site kept for every chat.
fn no_policy() -> SitesInForce {
    SitesInForce {
        locks: sandbox::policy::Locks::none(),
        kept_here: Vec::new(),
    }
}

#[test]
fn a_restore_of_fifty_chats_reads_the_policy_once_until_it_is_forgotten() {
    // #4739: `opened_chats` is asked at each chat a launch brings back, and counts every
    // confined chat's sites: fifty asks of fifty chats read the policy files once.
    let kept = KeptSites::default();
    let reads = std::cell::Cell::new(0);
    let read = || {
        reads.set(reads.get() + 1);
        no_policy()
    };
    for _ask in 0..50 {
        for _chat in 0..50 {
            kept.now(read);
        }
    }
    assert_eq!(reads.get(), 1);

    kept.forget();
    kept.now(read);
    kept.now(read);
    assert_eq!(reads.get(), 2, "read again once, after the forget");
}

#[test]
fn a_read_a_forget_overtook_is_answered_but_not_kept() {
    // The policy changed while it was being read: what was read may be from before the change.
    let kept = KeptSites::default();
    let answered = kept.now(|| {
        kept.forget();
        no_policy()
    });
    assert_eq!(answered.of_chat(&[], 1), 0);
    assert!(!kept.holds(), "a read from before the change was kept");
    kept.now(no_policy);
    assert!(kept.holds());
}
