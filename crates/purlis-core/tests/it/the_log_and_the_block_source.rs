// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The log, the block source, and the retired enforcement log** (M75 slice 13, #7022; spec
//! §10.3, §10.5, §10.7).
//!
//! `log` writes a would-have-blocked event into the event log, naming the rule, the family, the
//! chat and the host and no command, path or URL. A chat reads only its own rows. Every block
//! names its source. ADR 0125's enforcement log and `$PURLIS_POLICY` are gone from the code.

use std::path::{Path, PathBuf};

use purlis_core::enforcement::Mode;
use purlis_core::eventlog::stops::{
    Block, Decided, Qualifier, Source, Stop, Target, WOULD_BLOCK, WouldBlock,
};
use purlis_core::eventlog::{ArgsKey, Log, Reader, Recorder, read, read_rows};
use purlis_core::policy::World;
use purlis_core::policy::decide::{self, Outcome, Request};
use purlis_core::policy::levels::{self, Chat, Loaded, Sources};

const DEVICE: &str = "01J9ZZDEVICE0000000000000";
const PLANE: &str = "/planes/one";

fn recorder(dir: &Path) -> Recorder {
    Recorder::new(Log::open(dir, DEVICE).unwrap(), ArgsKey::open(dir).unwrap())
}

fn git(dir: &Path, args: &[&str]) {
    let ran = purlis_core::forklock::output(
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "init.defaultBranch=main",
            ])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1"),
    )
    .expect("git runs");
    assert!(ran.status.success(), "git {args:?}: {ran:?}");
}

fn loaded(settings: &str) -> Loaded {
    let dir = tempfile::tempdir().expect("a folder");
    git(dir.path(), &["init", "-q"]);
    std::fs::write(dir.path().join("purlis.toml"), settings).expect("the project file");
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "c"]);
    levels::load(&Sources {
        root: dir.path(),
        workspaces: &["web"],
        personas: &["builder"],
        organisation: None,
        accepted: None,
    })
}

fn merge() -> Request<'static> {
    Request {
        chat: "web/builder",
        of: Chat {
            workspace: Some("web"),
            persona: Some("builder"),
        },
        action: "merge",
        resource: ("Branch", "acme/web#main"),
        context: serde_json::json!({
            "sandbox": "floor", "attended": true, "harness_mode": "default", "depth": 0,
            "via": "hook"
        }),
    }
}

fn empty_world() -> World {
    World::from_json(serde_json::json!([])).expect("an empty world")
}

/// A merge under a family in `log` mode is allowed, would have been asked, and the event the log
/// gets names the rule, the family, the chat and the host, and holds no command, path or URL.
#[test]
fn log_turns_a_deny_into_allow_and_writes_one_would_have_blocked_event() {
    purlis_core::unsteered!();
    let settings = "[policy]\ntemplate = \"strict\"\n\"guards.merge\" = \"log\"\n";
    let decision = decide::decide(&loaded(settings), &merge(), &empty_world());
    assert_eq!(
        (decision.outcome, decision.mode, decision.after_mode),
        (Outcome::Ask, Mode::Log, Outcome::Allow),
        "the family in log lets the merge through"
    );
    let line = WouldBlock::of(&decision, Some("github.com:443"))
        .expect("an allow the mode made names what would have refused");
    assert_eq!(line.rules, ["guards-merge-hold"]);
    assert_eq!(line.family, "guards.merge");
    assert_eq!(line.host.as_deref(), Some("github.com:443"));

    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    host.stop(Path::new(PLANE), Some(3), &Stop::WouldBlock(line))
        .unwrap();
    let wrote: Vec<_> = read(dir.path())
        .unwrap()
        .into_iter()
        .filter(|event| event.kind == WOULD_BLOCK)
        .collect();
    assert_eq!(wrote.len(), 1, "one event, not one per rule or per read");
    let event = &wrote[0];
    assert_eq!(
        event.body["rules"],
        serde_json::json!(["guards-merge-hold"])
    );
    assert_eq!(event.body["family"], "guards.merge");
    assert_eq!(event.body["host"], "github.com:443");
    assert!(
        event.chat.is_some(),
        "it is under the chat it is of: {event:?}"
    );
    let written = serde_json::to_string(event).unwrap();
    for never in ["acme/web", "gh pr merge", "http", "/planes"] {
        assert!(!written.contains(never), "{never:?} is in {written}");
    }

    // An allow no mode made, and an enforced deny, are no would-have-blocked line.
    let enforced = decide::decide(
        &loaded("[policy]\ntemplate = \"strict\"\n"),
        &merge(),
        &empty_world(),
    );
    assert!(WouldBlock::of(&enforced, None).is_none());
}

/// A chat reads its own rows of the event log and nobody else's; the person reads all of them.
#[test]
fn a_chat_reads_only_its_own_event_log_rows() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    let line = |rule: &str| {
        Stop::WouldBlock(WouldBlock {
            rules: vec![rule.to_owned()],
            level: None,
            family: "guards.merge".to_owned(),
            host: None,
        })
    };
    host.stop(Path::new(PLANE), Some(1), &line("first"))
        .unwrap();
    host.stop(Path::new(PLANE), Some(2), &line("second"))
        .unwrap();
    host.stop(Path::new(PLANE), Some(1), &line("third"))
        .unwrap();
    let all = read(dir.path()).unwrap();
    let chats: std::collections::BTreeSet<String> =
        all.iter().filter_map(|event| event.chat.clone()).collect();
    assert_eq!(chats.len(), 2, "two chats wrote");
    let first = all
        .iter()
        .find(|event| event.body["rules"][0] == "first")
        .and_then(|event| event.chat.clone())
        .unwrap();

    let own = read_rows(dir.path(), Reader::Chat(&first)).unwrap();
    assert!(
        own.iter()
            .all(|event| event.chat.as_deref() == Some(first.as_str())),
        "only its own rows: {own:?}"
    );
    assert!(
        own.iter().any(|event| event.body["rules"][0] == "third"),
        "all of its own"
    );
    assert!(
        !own.iter().any(|event| event.body["rules"][0] == "second"),
        "none of another chat's"
    );
    assert!(
        own.iter().all(|event| event.chat.is_some()),
        "a row of no chat is the person's, not a chat's"
    );
    assert_eq!(read_rows(dir.path(), Reader::Person).unwrap(), all);
    assert!(
        read_rows(dir.path(), Reader::Chat("not-a-chat"))
            .unwrap()
            .is_empty(),
        "a chat the log has no rows of reads none"
    );
}

/// Every block names its source: a rule (the policy's `@id`s, or a guard's arm), the sandbox
/// (naming the rule it was compiled from) or the harness's own check (no rule). A block that does
/// not is refused before it is written.
#[test]
fn a_block_names_its_source_rule_sandbox_or_harness() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    let plane = Path::new(PLANE);
    let block = |source, policy: Option<Decided>, compiled_from: Option<&str>| {
        Stop::Block(Block {
            tool: "Bash".to_owned(),
            source,
            qualifier: None::<Qualifier>,
            target: Some(Target::guard_arm("A4").unwrap()),
            args: None,
            policy,
            compiled_from: compiled_from.map(str::to_owned),
        })
    };
    let decided = Decided {
        rules: vec!["guards-merge-hold".to_owned()],
        level: Some("project".to_owned()),
        family: "guards.merge".to_owned(),
        outcome: Outcome::Deny,
        after_mode: Outcome::Deny,
    };
    host.stop(plane, Some(1), &block(Source::Policy, Some(decided), None))
        .unwrap();
    host.stop(
        plane,
        Some(1),
        &block(Source::Sandbox, None, Some("sandbox-write-secrets")),
    )
    .unwrap();
    host.stop(plane, Some(1), &block(Source::HarnessCheck, None, None))
        .unwrap();
    let blocks: Vec<_> = read(dir.path())
        .unwrap()
        .into_iter()
        .filter(|event| event.kind == "stop.block")
        .collect();
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0].body["source"], "policy");
    assert_eq!(blocks[0].body["policy"]["rules"][0], "guards-merge-hold");
    assert_eq!(blocks[1].body["source"], "sandbox");
    assert_eq!(blocks[1].body["compiled_from"], "sandbox-write-secrets");
    assert_eq!(blocks[2].body["source"], "harness_check");
    assert!(blocks[2].body.get("compiled_from").is_none());
    assert!(
        blocks[2].body.get("policy").is_none(),
        "the harness's check names no rule"
    );

    // Fail closed: a policy block with no rule, and a sandbox block naming something that is no
    // rule's id, are refused and nothing is appended.
    let before = read(dir.path()).unwrap().len();
    assert!(
        host.stop(plane, Some(1), &block(Source::Policy, None, None))
            .is_err(),
        "a policy block names the rules that decided it"
    );
    assert!(
        host.stop(
            plane,
            Some(1),
            &block(Source::Sandbox, None, Some("rm -rf /; echo"))
        )
        .is_err(),
        "a sandbox block names a rule's id, never text"
    );
    assert_eq!(
        read(dir.path()).unwrap().len(),
        before,
        "nothing was written"
    );
}

/// Every source file that reads or names the retired variable, the retired folder or the retired
/// accessor of the enforcement log.
fn sources_under(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.filter_map(Result::ok) {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if !matches!(name.as_ref(), "target" | "node_modules" | ".git") {
                sources_under(&path, found);
            }
        } else if name.ends_with(".rs") {
            found.push(path);
        }
    }
}

/// ADR 0125's `$PURLIS_POLICY` and `app/enforcement/<chat>.jsonl` are retired: no Rust source
/// reads the variable, builds its name or writes the folder, and a recorded scenario sets
/// neither. Comments that say what was retired are the docs' and the amendment's.
#[test]
fn no_code_reads_purlis_policy() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for dir in ["crates", "app/src-tauri/src"] {
        sources_under(&repo.join(dir), &mut files);
    }
    assert!(
        files.len() > 500,
        "the walk found the workspace: {}",
        files.len()
    );
    let this = file!();
    let retired = [
        "PURLIS_POLICY",
        "enforcement::ENV",
        "enforcement::log_dir",
        "enforcement::log_file",
        "join(\"enforcement\")",
    ];
    let mut found = Vec::new();
    for path in files {
        if path.to_string_lossy().ends_with(this)
            || path
                .to_string_lossy()
                .ends_with("the_log_and_the_block_source.rs")
        {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for (number, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or_default();
            if retired.iter().any(|word| code.contains(word)) {
                found.push(format!(
                    "{}:{}: {}",
                    path.display(),
                    number + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        found.is_empty(),
        "retired and still read:\n{}",
        found.join("\n")
    );
    let recorded = std::fs::read_to_string(repo.join("tests/fixtures/recorded/behaviour.jsonl"))
        .expect("the recorded fixture");
    assert!(
        !recorded.contains("PURLIS_POLICY"),
        "no scenario's environment sets the retired variable"
    );
}

/// A process outside every plane has no host to ask: it decides in itself, by the shipped default
/// under `strict`, so a plane-less recorded scenario runs exactly that (spec §6, §14.1; review
/// round 3, SEC3-SF3).
#[test]
fn a_plane_less_scenario_runs_the_shipped_strict_locally() {
    purlis_core::unsteered!();
    let local = decide::decide_locally(&merge());
    let strict = decide::decide(&Loaded::shipped_strict(), &merge(), &empty_world());
    assert_eq!(local.outcome, Outcome::Ask, "{local:?}");
    assert_eq!(
        (
            local.outcome,
            local.mode,
            local.after_mode,
            local.ids(),
            &local.family
        ),
        (
            strict.outcome,
            strict.mode,
            strict.after_mode,
            strict.ids(),
            &strict.family
        ),
        "locally is the shipped strict, with nothing a project or the person loosened"
    );
    assert_eq!(local.ids(), ["guards-merge-hold"]);
}

/// The replay reads no `/etc/purlis` of the machine it runs on: a test build's Organisation level
/// is the `TEST_LOCKS` seam alone, empty by default, so a plane-less scenario's answer does not
/// depend on whose machine replayed it (review round 5, FB5 R4-1).
#[test]
fn a_plane_less_replay_scenario_reads_no_organisation_folder_of_the_test_machine() {
    purlis_core::unsteered!();
    assert!(
        purlis_core::policy::organisation::in_force().is_none(),
        "nothing is read of the machine's own Organisation folder in a test build"
    );
    let alone = decide::decide_locally(&merge());
    assert!(
        alone
            .deciding
            .iter()
            .all(|rule| !rule.id.contains("organisation")),
        "no Organisation rule decided it: {alone:?}"
    );
}

/// The fixture plane the replay's in-plane scenarios start from is `daily`'s files plus
/// `[policy] template = "strict"`, committed: what the scenarios run under is what its `HEAD`
/// says. A change to that `HEAD` changes what is read, so this turns red with it (spec §14.1).
#[test]
fn recorded_behaviour_selects_strict_from_strict_planes_committed_head() {
    use purlis_core::enforcement::{Settings, Template};
    purlis_core::unsteered!();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let committed = |text: &str| {
        let dir = tempfile::tempdir().expect("a folder");
        git(dir.path(), &["init", "-q"]);
        std::fs::write(dir.path().join("charter.toml"), text).expect("the project file");
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "c"]);
        dir
    };
    let strict_plane = std::fs::read_to_string(fixtures.join("planes/strict-plane/charter.toml"))
        .expect("strict-plane");
    let daily = std::fs::read_to_string(fixtures.join("planes/daily/charter.toml")).expect("daily");
    let minimal =
        std::fs::read_to_string(fixtures.join("planes/minimal/charter.toml")).expect("minimal");
    assert!(
        strict_plane.starts_with(daily.trim_end()),
        "strict-plane is daily's project file and one table more"
    );
    for (name, text) in [("daily", &daily), ("minimal", &minimal)] {
        assert!(!text.contains("[policy]"), "{name} keeps no [policy]");
    }

    // Committed, the plane's HEAD selects strict: `Settings::read` is what a host reads.
    let head = committed(&strict_plane);
    assert_eq!(Settings::read(head.path()).template, Template::Strict);
    // The same plane with a different HEAD selects something else, so the test follows the HEAD.
    let changed = committed(&strict_plane.replace("strict", "open"));
    assert_eq!(Settings::read(changed.path()).template, Template::Open);

    // **The fixture's own HEAD, pinned** (round 2B, B9): `strict-plane` is `daily`'s project file
    // and exactly one `[policy]` table, and nothing else is in the folder. A change to the fixture
    // that moves what the replay reads at `HEAD` fails here, not as 34 scenarios that happen to
    // agree with the all-enforce fallback (`enforcement::in_force` where no host answers).
    assert_eq!(
        strict_plane,
        format!("{}\n\n[policy]\ntemplate = \"strict\"\n", daily.trim_end()),
        "strict-plane's committed project file is daily's and one [policy] table"
    );
    let files: Vec<_> = std::fs::read_dir(fixtures.join("planes/strict-plane"))
        .expect("the fixture folder")
        .map(|entry| entry.expect("an entry").file_name())
        .collect();
    assert_eq!(
        files,
        ["charter.toml"],
        "nothing else is laid over daily's files"
    );
    // What a host answers for that HEAD is every family enforced, so the fallback a process gets
    // where no host answers is the same answer by construction, not by luck.
    let resolved = purlis_core::enforcement::Resolved::of(
        &Settings::read(head.path()),
        &purlis_core::sandbox::policy::Locks::of(head.path()),
    );
    for family in purlis_core::enforcement::Family::ALL {
        assert_eq!(
            resolved.mode(family),
            Mode::Enforce,
            "the strict template's overlay enforces {family:?}"
        );
    }

    // Every in-plane pretooluse scenario starts from it, and none sets the retired variable.
    let recorded =
        std::fs::read_to_string(fixtures.join("recorded/behaviour.jsonl")).expect("rows");
    let mut moved = 0;
    for line in recorded.lines() {
        let row: serde_json::Value = serde_json::from_str(line).expect("a row");
        let name = row["name"].as_str().unwrap_or_default();
        assert!(
            row["env"].get("PURLIS_POLICY").is_none(),
            "{name} sets the retired variable"
        );
        let plane = row["plane"].as_str().unwrap_or_default();
        if plane == "strict-plane" {
            assert!(
                name.starts_with("pretooluse-"),
                "{name}: only the pretooluse scenarios start from strict-plane"
            );
            moved += 1;
        }
    }
    assert_eq!(
        moved, 34,
        "the 34 in-plane scenarios that set the variable moved"
    );
}

/// **The writer holds a would-have-blocked line to tokens and a bare host** (round 2B, S2): a rule
/// id or a family that is no token is written changed, never dropped with its event, and a host
/// that is a path, a URL or anything with a query is left off the line. The line never carries a
/// command, a path or a URL, whoever built it.
#[test]
fn the_writer_holds_a_would_have_blocked_line_to_tokens_and_a_bare_host() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    let long = "r".repeat(300);
    let lines = [
        (vec![long.clone()], "guards.merge", Some("github.com:443")),
        (
            vec!["a/b c?d".to_owned()],
            "guards/merge",
            Some("https://evil.example/p?q=1"),
        ),
        (
            vec!["ok-rule".to_owned()],
            "guards.merge",
            Some("/home/me/.ssh/id_ed25519"),
        ),
        (
            vec!["ok-rule".to_owned()],
            "guards.merge",
            Some("two words:80"),
        ),
        (
            vec!["ok-rule".to_owned()],
            "guards.merge",
            Some("[::1]:8080"),
        ),
        (Vec::new(), "guards.merge", None),
    ];
    for (rules, family, at) in &lines {
        host.stop(
            Path::new(PLANE),
            Some(1),
            &Stop::WouldBlock(WouldBlock {
                rules: rules.clone(),
                level: Some("project owner".to_owned()),
                family: (*family).to_owned(),
                host: at.map(str::to_owned),
            }),
        )
        .expect("a line that is not a token is changed, never refused");
    }
    let wrote: Vec<_> = read(dir.path())
        .unwrap()
        .into_iter()
        .filter(|event| event.kind == WOULD_BLOCK)
        .collect();
    assert_eq!(wrote.len(), lines.len(), "every line is written");
    let is_token = |text: &str| {
        (1..=64).contains(&text.len())
            && text
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'))
    };
    for event in &wrote {
        for rule in event.body["rules"].as_array().expect("rules") {
            assert!(is_token(rule.as_str().unwrap()), "{event:?}");
        }
        assert!(
            is_token(event.body["family"].as_str().unwrap()),
            "{event:?}"
        );
        assert!(is_token(event.body["level"].as_str().unwrap()), "{event:?}");
        if let Some(at) = event.body.get("host") {
            let at = at.as_str().unwrap();
            assert!(
                !at.contains('/') && !at.contains('?') && !at.contains(' '),
                "{at}"
            );
        }
    }
    let hosts: Vec<_> = wrote
        .iter()
        .map(|event| event.body.get("host").and_then(|at| at.as_str()))
        .collect();
    assert_eq!(
        hosts,
        [
            Some("github.com:443"),
            None,
            None,
            None,
            Some("[::1]:8080"),
            None
        ]
    );
    assert_eq!(
        wrote[0].body["rules"][0],
        "r".repeat(64),
        "held to 64, not dropped"
    );
}

/// **A deny whose rule `@id` is no token is still in the log** (round 2B, S1): a rule id over 64
/// characters or outside `A-Za-z0-9._:-` is written changed. The block the host writes for it
/// (`decisions::blocked`) names its source all the same, where it used to be refused and the
/// deny vanished from the log with only a warning.
#[test]
fn a_deny_whose_rule_id_is_no_token_is_written_changed_never_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    for id in [
        "x".repeat(80),
        "has space/and:slash".to_owned(),
        String::new(),
    ] {
        let decision: decide::Decision = serde_json::from_value(serde_json::json!({
            "outcome": "deny",
            "deciding": [{
                "id": id, "level": "project", "owner": "this project",
                "file": "policy.cedar", "line": 1
            }],
            "family": "guards.merge",
            "mode_family": "guards.merge",
            "mode": "enforce",
            "after_mode": "deny"
        }))
        .unwrap();
        let block = Block {
            tool: "Bash".to_owned(),
            source: Source::Policy,
            qualifier: None,
            target: None,
            args: None,
            policy: Some(Decided::from(&decision)),
            compiled_from: None,
        };
        host.stop(Path::new(PLANE), Some(1), &Stop::Block(block))
            .unwrap_or_else(|why| panic!("the deny of {id:?} was dropped: {why}"));
    }
    let blocks: Vec<_> = read(dir.path())
        .unwrap()
        .into_iter()
        .filter(|event| event.kind == purlis_core::eventlog::stops::BLOCK)
        .collect();
    assert_eq!(blocks.len(), 3, "each deny has its event");
    assert_eq!(blocks[0].body["policy"]["rules"][0], "x".repeat(64));
    assert_eq!(blocks[1].body["policy"]["rules"][0], "has_space_and:slash");
    assert_eq!(blocks[2].body["policy"]["rules"][0], "unnamed");
}
