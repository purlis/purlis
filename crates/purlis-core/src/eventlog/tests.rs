// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const DEVICE: &str = "01J9ZZDEVICE0000000000000";

#[test]
fn an_event_is_one_line_in_adr_0066s_envelope_numbered_from_one() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::open(dir.path(), DEVICE).unwrap();

    let first = log
        .append(
            Some("CHAT"),
            Some("RUN"),
            None,
            "hook.stop",
            serde_json::json!({}),
        )
        .unwrap();
    let second = log
        .append(
            Some("CHAT"),
            Some("RUN"),
            None,
            "hook.stop",
            serde_json::json!({}),
        )
        .unwrap();

    assert_eq!((first.seq, second.seq), (1, 2));
    assert_eq!(first.v, 1);
    assert_eq!(first.device_id, DEVICE);
    assert_eq!(first.ulid.len(), 26, "a ULID: {}", first.ulid);
    assert_ne!(first.ulid, second.ulid);
    assert_eq!(read(dir.path()).unwrap(), vec![first, second]);
}

#[test]
fn a_log_opened_again_carries_on_from_its_last_number() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut log = Log::open(dir.path(), DEVICE).unwrap();
        for _ in 0..3 {
            log.append(
                Some("CHAT"),
                None,
                None,
                "chat.opened",
                serde_json::json!({}),
            )
            .unwrap();
        }
    }

    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    let next = log
        .append(
            Some("CHAT"),
            None,
            None,
            "chat.opened",
            serde_json::json!({}),
        )
        .unwrap();

    assert_eq!(next.seq, 4, "a seq is never reused");
}

#[test]
fn a_second_writer_on_the_same_log_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let _first = Log::open(dir.path(), DEVICE).unwrap();

    let second = Log::open(dir.path(), DEVICE);

    assert!(second.is_err(), "one writer per device (ADR 0068)");
}

/// #972: a host that exits never unlocks the log's lock, so it is let go of only when the last
/// copy of its descriptor closes — and a child the host forked holds one until it execs. A log
/// opened again in that moment, as when the app is relaunched, waits the child out instead of
/// being refused as a second writer. The host here is a plain locked file dropped without an
/// unlock, and the child keeps a copy as its stdin for as long as it sleeps.
#[cfg(unix)]
#[test]
fn a_log_opened_again_while_a_child_of_an_exited_host_still_holds_its_lock_waits_the_child_out() {
    let dir = tempfile::tempdir().unwrap();
    crate::secrets::make_private_dir(dir.path()).unwrap();
    let host = private_file(&dir.path().join(LOCK)).unwrap();
    host.lock().unwrap();
    let copy = std::process::Stdio::from(host.try_clone().unwrap());
    let mut child =
        crate::forklock::spawn(std::process::Command::new("sleep").arg("0.4").stdin(copy)).unwrap();

    drop(host);
    let asked = Instant::now();
    let again = Log::open(dir.path(), DEVICE);
    let waited = asked.elapsed();
    child.wait().unwrap();

    assert!(
        again.is_ok(),
        "a child holding the lock a moment is not a second writer: {:?}",
        again.err()
    );
    assert!(
        waited >= std::time::Duration::from_millis(100),
        "the lock was free at once ({waited:?}), so nothing here was waited out"
    );
}

/// #1316: a child that keeps a copy of the lock's descriptor for longer than [`LOCK_WAIT`] —
/// here as its stdin, for as long as it sleeps — does not keep the next writer out once the
/// log that held the lock is dropped, since the drop unlocks every copy at once.
#[cfg(unix)]
#[test]
fn a_log_dropped_lets_go_of_its_lock_though_a_child_keeps_a_copy_of_it() {
    let dir = tempfile::tempdir().unwrap();
    let log = Log::open(dir.path(), DEVICE).unwrap();
    let copy = std::process::Stdio::from(log._lock.try_clone().unwrap());
    let mut child =
        crate::forklock::spawn(std::process::Command::new("sleep").arg("30").stdin(copy)).unwrap();

    drop(log);
    let again = Log::open(dir.path(), DEVICE);
    let _ = child.kill();
    let _ = child.wait();

    assert!(
        again.is_ok(),
        "the lock outlived its log: {:?}",
        again.err()
    );
}

#[test]
fn an_args_digest_is_keyed_by_the_device_and_stays_the_same_across_launches() {
    let one = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let args = args_hash(&serde_json::json!({"command": "ls -la"}));

    let here = ArgsKey::open(one.path()).unwrap().digest(&args);
    let again = ArgsKey::open(one.path()).unwrap().digest(&args);
    let elsewhere = ArgsKey::open(other.path()).unwrap().digest(&args);

    assert_eq!(
        here, again,
        "the key is kept, so a digest can be compared with yesterday's"
    );
    assert_ne!(here, elsewhere, "another device's key gives another digest");
    assert_ne!(
        here, args,
        "the log never holds the plain hash, which a guess could match"
    );
    assert_eq!(here.len(), 64);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(one.path().join(ARGS_KEY))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "the key is the operator's alone");
    }
}

#[test]
fn the_same_arguments_hash_the_same_and_different_ones_do_not() {
    let a = args_hash(&serde_json::json!({"command": "ls"}));
    assert_eq!(a, args_hash(&serde_json::json!({"command": "ls"})));
    assert_ne!(a, args_hash(&serde_json::json!({"command": "ls -la"})));
    assert_eq!(a.len(), 64, "sha-256, hex");
}

fn report(chat: u32, event: crate::state::Event) -> crate::hookwire::Report {
    crate::hookwire::Report {
        chat,
        event,
        conversation: crate::hookwire::Conversation::default(),
        pid: None,
        agent: None,
        detail: crate::state::Detail::default(),
    }
}

fn recorder(dir: &Path) -> Recorder {
    Recorder::new(Log::open(dir, DEVICE).unwrap(), ArgsKey::open(dir).unwrap())
}

#[test]
fn events_of_two_runs_in_one_chat_group_under_that_chat() {
    use crate::state::Event::{SessionStart, Stop, UserPromptSubmit};
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());

    host.report(plane, &report(7, SessionStart), Followed::No)
        .unwrap();
    host.report(plane, &report(7, UserPromptSubmit), Followed::No)
        .unwrap();
    host.report(plane, &report(7, Stop), Followed::No).unwrap();
    // `/clear`: the board follows the chat onto another conversation, which is a new run.
    host.report(plane, &report(7, SessionStart), Followed::Moved)
        .unwrap();
    host.report(plane, &report(7, UserPromptSubmit), Followed::No)
        .unwrap();

    let events = read(dir.path()).unwrap();
    let chats: std::collections::BTreeSet<_> = events.iter().map(|e| e.chat.clone()).collect();
    assert_eq!(chats.len(), 1, "one chat: {events:#?}");
    let runs: Vec<_> = events
        .iter()
        .filter(|e| e.kind == "run.started")
        .map(|e| (e.run.clone().unwrap(), e.body["cause"].clone()))
        .collect();
    assert_eq!(runs.len(), 2, "{events:#?}");
    assert_ne!(runs[0].0, runs[1].0, "two runs");
    assert_eq!(runs[0].1, "start");
    assert_eq!(runs[1].1, "clear");
    let hooks: Vec<_> = events
        .iter()
        .filter(|e| e.kind.starts_with("hook."))
        .map(|e| (e.kind.as_str(), e.run.clone().unwrap()))
        .collect();
    assert_eq!(
        hooks,
        vec![
            ("hook.sessionstart", runs[0].0.clone()),
            ("hook.userpromptsubmit", runs[0].0.clone()),
            ("hook.stop", runs[0].0.clone()),
            ("hook.sessionstart", runs[1].0.clone()),
            ("hook.userpromptsubmit", runs[1].0.clone()),
        ],
        "every hook call is one event, under the run it happened in"
    );
}

#[test]
fn a_conversation_named_for_the_first_time_and_a_compaction_keep_the_run() {
    use crate::state::Event::SessionStart;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());

    host.report(plane, &report(1, SessionStart), Followed::No)
        .unwrap();
    host.report(plane, &report(1, SessionStart), Followed::FirstNamed)
        .unwrap();

    let events = read(dir.path()).unwrap();
    assert_eq!(
        events.iter().filter(|e| e.kind == "run.started").count(),
        1,
        "ADR 0066: a harness naming its conversation keeps the run: {events:#?}"
    );
}

#[test]
fn the_same_number_in_two_planes_is_two_chats() {
    use crate::state::Event::Stop;
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());

    host.report(Path::new("/planes/one"), &report(1, Stop), Followed::No)
        .unwrap();
    host.report(Path::new("/planes/two"), &report(1, Stop), Followed::No)
        .unwrap();

    let events = read(dir.path()).unwrap();
    let chats: std::collections::BTreeSet<_> = events.iter().map(|e| e.chat.clone()).collect();
    assert_eq!(
        chats.len(),
        2,
        "a chat's number means nothing outside its plane"
    );
}

fn tool_call(
    chat: u32,
    word: &str,
    decision: crate::hookwire::Decision,
) -> crate::hookwire::ToolCall {
    crate::hookwire::ToolCall {
        chat,
        tool_hook: word.to_owned(),
        tool: Some("Bash".to_owned()),
        call: Some("toolu_01".to_owned()),
        args: Some(args_hash(&serde_json::json!({"command": "ls"}))),
        decision,
        rule: (decision == crate::hookwire::Decision::Deny).then(|| "no-force-push".to_owned()),
        hook_ms: 3,
        agent: None,
        at_ms: 0,
        skill: None,
    }
}

#[test]
fn a_tool_call_is_one_event_with_its_tool_keyed_digest_decision_and_durations() {
    use crate::hookwire::Decision;
    use std::time::{Duration, Instant};
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());
    let at = Instant::now();

    let pre = host
        .tool(plane, &tool_call(4, "pretooluse", Decision::Deny), at)
        .unwrap();
    let post = host
        .tool(
            plane,
            &tool_call(4, "posttooluse", Decision::None),
            at + Duration::from_millis(250),
        )
        .unwrap();

    assert_eq!(pre.kind, "hook.pretooluse");
    assert_eq!(pre.body["tool"], "Bash");
    assert_eq!(pre.body["call"], "toolu_01");
    assert_eq!(pre.body["decision"], "deny");
    assert_eq!(pre.body["rule"], "no-force-push");
    assert_eq!(pre.body["hook_ms"], 3);
    let sent = args_hash(&serde_json::json!({"command": "ls"}));
    let kept = pre.body["args"].as_str().unwrap();
    assert_eq!(
        kept,
        ArgsKey::open(dir.path()).unwrap().digest(&sent),
        "keyed"
    );
    assert_ne!(kept, sent, "never the hash the hook sent");
    assert_eq!(post.kind, "hook.posttooluse");
    assert_eq!(
        post.body["tool_ms"], 250,
        "from the call's pre hook to its post hook"
    );
    assert_eq!(pre.run, post.run, "both under the chat's current run");
    let text = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
    assert!(!text.contains("ls"), "no argument reaches the log: {text}");
}

#[test]
fn the_host_keeps_the_log_under_the_data_home_by_device_id() {
    use crate::state::Event::Stop;
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    std::fs::create_dir_all(&config).unwrap();
    let data = dir.path().join("data");

    let mut host = Recorder::open_in(&config, &data).unwrap();
    host.report(Path::new("/p"), &report(1, Stop), Followed::No)
        .unwrap();

    let device = crate::machine::device_id(&config).unwrap();
    let events = read(&data.join("events").join(&device)).unwrap();
    assert_eq!(events.len(), 2, "run.started and hook.stop: {events:#?}");
    assert!(events.iter().all(|e| e.device_id == device));
}

#[test]
fn a_data_home_inside_a_repository_gets_no_log() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::create_dir_all(dir.path().join("repo").join(".git")).unwrap();

    let refused = Recorder::open_in(&config, &dir.path().join("repo").join("data"));

    assert!(refused.is_err());
    assert!(
        !dir.path().join("repo").join("data").exists(),
        "nothing was made there"
    );
}

#[test]
fn a_line_torn_by_a_crash_is_cut_off_and_the_next_event_is_whole() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut log = Log::open(dir.path(), DEVICE).unwrap();
        log.append(
            Some("CHAT"),
            None,
            None,
            "chat.opened",
            serde_json::json!({}),
        )
        .unwrap();
    }
    // The machine died in the middle of the second line.
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(dir.path().join(FILE))
        .unwrap();
    std::io::Write::write_all(&mut file, br#"{"v":1,"device_id":"X","seq":2,"ul"#).unwrap();
    drop(file);

    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    let next = log
        .append(
            Some("CHAT"),
            None,
            None,
            "chat.opened",
            serde_json::json!({}),
        )
        .unwrap();

    let events = read(dir.path()).unwrap();
    assert_eq!(
        events.len(),
        2,
        "the torn line went, the new one is whole: {events:#?}"
    );
    assert_eq!(events[0].seq, 1);
    assert_eq!(next.seq, 2);
    assert_eq!(events[1], next);
}

#[test]
fn a_log_with_lines_but_no_number_in_them_is_refused_rather_than_counted_from_one_again() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(FILE), "not an event\nnor this\n").unwrap();

    let refused = Log::open(dir.path(), DEVICE);

    assert!(
        refused.is_err(),
        "a seq is never reused, so a log it cannot read is not restarted"
    );
}

#[test]
fn a_last_seq_at_the_top_of_the_range_is_refused_rather_than_wrapped() {
    let dir = tempfile::tempdir().unwrap();
    let event = Event {
        v: VERSION,
        device_id: DEVICE.to_owned(),
        seq: u64::MAX,
        ulid: ulid::Ulid::generate().to_string(),
        chat: None,
        run: None,
        parent_run: None,
        kind: "chat.opened".to_owned(),
        body: serde_json::json!({}),
    };
    std::fs::write(
        dir.path().join(FILE),
        format!("{}\n", serde_json::to_string(&event).unwrap()),
    )
    .unwrap();

    let opened = Log::open(dir.path(), DEVICE);

    assert!(opened.is_err());
}

#[test]
fn a_chat_the_host_has_not_seen_begins_with_start_even_when_its_first_line_is_a_move() {
    use crate::state::Event::SessionStart;
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());

    host.report(Path::new("/p"), &report(2, SessionStart), Followed::Moved)
        .unwrap();

    let events = read(dir.path()).unwrap();
    assert_eq!(events[0].kind, "run.started");
    assert_eq!(
        events[0].body["cause"], "start",
        "there was no run to clear: {events:#?}"
    );
}

#[test]
fn a_sub_agents_calls_are_a_child_run_of_the_run_that_was_current_until_its_stop() {
    use crate::hookwire::Decision;
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/p");
    let mut host = recorder(dir.path());
    let parent = host
        .report(
            plane,
            &report(5, crate::state::Event::UserPromptSubmit),
            Followed::No,
        )
        .unwrap()
        .run
        .unwrap();
    let mut by_agent = tool_call(5, "pretooluse", Decision::None);
    by_agent.agent = Some("agent-1".to_owned());

    let first = host.tool(plane, &by_agent, Instant::now()).unwrap();
    let second = host.tool(plane, &by_agent, Instant::now()).unwrap();
    let mut stop = report(5, crate::state::Event::SubagentStop);
    stop.agent = Some("agent-1".to_owned());
    let stopped = host.report(plane, &stop, Followed::No).unwrap();
    let after = host.tool(plane, &by_agent, Instant::now()).unwrap();

    let child = first.run.clone().unwrap();
    assert_ne!(child, parent, "a sub-agent is a run of its own");
    assert_eq!(first.parent_run.as_deref(), Some(parent.as_str()));
    assert_eq!(
        second.run, first.run,
        "the same agent is the same child run"
    );
    assert_eq!(stopped.run, first.run, "its stop is its own");
    assert_eq!(
        after.run.as_deref(),
        Some(parent.as_str()),
        "a stopped agent's late line is the parent run's, and mints no second child run"
    );
    assert_eq!(after.parent_run, None);
    let started: Vec<_> = read(dir.path())
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == "run.started")
        .collect();
    assert_eq!(
        started.len(),
        2,
        "the parent's run and one child run: {started:#?}"
    );
    assert_eq!(started[1].body["cause"], "child");
    assert_eq!(started[1].parent_run.as_deref(), Some(parent.as_str()));
}

#[test]
fn a_tool_hook_word_charter_does_not_answer_is_recorded_under_one_kind() {
    use crate::hookwire::Decision;
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());

    let forged = host
        .tool(
            Path::new("/p"),
            &tool_call(1, "chat.closed", Decision::None),
            std::time::Instant::now(),
        )
        .unwrap();

    assert_eq!(
        forged.kind, "hook.unknown",
        "no line can mint a kind of its own"
    );
    assert_eq!(forged.body["word"], "chat.closed");
}

#[test]
fn a_tool_calls_duration_is_taken_once_and_a_call_left_open_is_let_go_by_age() {
    use crate::hookwire::Decision;
    use std::time::{Duration, Instant};
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/p");
    let mut host = recorder(dir.path());
    let at = Instant::now();

    host.tool(plane, &tool_call(1, "pretooluse", Decision::None), at)
        .unwrap();
    let post = host
        .tool(
            plane,
            &tool_call(1, "posttooluse", Decision::None),
            at + Duration::from_millis(5),
        )
        .unwrap();
    let again = host
        .tool(
            plane,
            &tool_call(1, "posttooluse-skill", Decision::None),
            at + Duration::from_millis(9),
        )
        .unwrap();
    assert_eq!(post.body["tool_ms"], 5);
    assert!(
        again.body.get("tool_ms").is_none(),
        "a call's duration is taken once"
    );

    let mut stale = tool_call(1, "pretooluse", Decision::None);
    stale.call = Some("toolu_old".to_owned());
    host.tool(plane, &stale, at).unwrap();
    // Long after: the pre hook's entry is older than a call is ever left open.
    let late = at + CALL_IS_OPEN_AT_MOST + Duration::from_secs(1);
    host.tool(plane, &tool_call(1, "pretooluse", Decision::None), late)
        .unwrap();
    let mut stale_post = tool_call(1, "posttooluse", Decision::None);
    stale_post.call = Some("toolu_old".to_owned());
    let stale_post = host.tool(plane, &stale_post, late).unwrap();
    assert!(
        stale_post.body.get("tool_ms").is_none(),
        "the open call was let go by age"
    );
}

#[test]
fn a_sub_agents_child_run_ends_with_its_parents_run() {
    use crate::hookwire::Decision;
    use crate::state::Event::{SessionEnd, SessionStart, UserPromptSubmit};
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/p");
    let mut host = recorder(dir.path());
    host.report(plane, &report(5, UserPromptSubmit), Followed::No)
        .unwrap();
    let mut by_agent = tool_call(5, "pretooluse", Decision::None);
    by_agent.agent = Some("agent-1".to_owned());
    let before = host.tool(plane, &by_agent, Instant::now()).unwrap();

    // `/clear`: the parent run ends, and a sub-agent whose `SubagentStop` was not heard ends
    // with it.
    host.report(plane, &report(5, SessionStart), Followed::Moved)
        .unwrap();
    let after_clear = host.tool(plane, &by_agent, Instant::now()).unwrap();
    host.report(plane, &report(5, SessionEnd), Followed::No)
        .unwrap();
    let after_end = host.tool(plane, &by_agent, Instant::now()).unwrap();

    assert_ne!(
        after_clear.run, before.run,
        "the child of the cleared run ended with it"
    );
    assert_ne!(
        after_clear.parent_run, before.parent_run,
        "a child of the new run"
    );
    assert_ne!(
        after_end.run, after_clear.run,
        "a child run ends with its parent's session"
    );
}

#[test]
fn a_tool_calls_post_hook_heard_before_its_pre_hook_still_pairs_by_when_each_ran() {
    use crate::hookwire::Decision;
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/p");
    let mut host = recorder(dir.path());
    let mut pre = tool_call(1, "pretooluse", Decision::None);
    pre.at_ms = 10_000;
    let mut post = tool_call(1, "posttooluse", Decision::None);
    post.at_ms = 10_420;

    let post_first = host.tool(plane, &post, Instant::now()).unwrap();
    let pre_second = host.tool(plane, &pre, Instant::now()).unwrap();

    assert!(
        post_first.body.get("tool_ms").is_none(),
        "its pre hook is not heard yet"
    );
    assert_eq!(
        pre_second.body["tool_ms"], 420,
        "the second of the pair says how long the tool ran, by the hooks' own clocks"
    );
}

#[test]
fn a_post_hook_heard_long_after_its_pre_hook_gives_no_duration() {
    use crate::hookwire::Decision;
    use std::time::{Duration, Instant};
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/p");
    let mut host = recorder(dir.path());
    let at = Instant::now();

    host.tool(plane, &tool_call(1, "pretooluse", Decision::None), at)
        .unwrap();
    let late = host
        .tool(
            plane,
            &tool_call(1, "posttooluse", Decision::None),
            at + CALL_IS_OPEN_AT_MOST + Duration::from_secs(1),
        )
        .unwrap();

    assert!(
        late.body.get("tool_ms").is_none(),
        "a call is open an hour at most"
    );
}

#[test]
fn a_write_that_failed_partway_is_cut_back_before_the_next_event() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    log.append(
        Some("CHAT"),
        None,
        None,
        "chat.opened",
        serde_json::json!({}),
    )
    .unwrap();

    log.fail_the_next_write_after(10);
    assert!(
        log.append(
            Some("CHAT"),
            None,
            None,
            "chat.opened",
            serde_json::json!({})
        )
        .is_err()
    );
    let next = log
        .append(
            Some("CHAT"),
            None,
            None,
            "chat.opened",
            serde_json::json!({}),
        )
        .unwrap();

    let events = read(dir.path()).unwrap();
    assert_eq!(events.len(), 2, "the half line is gone: {events:#?}");
    assert_eq!(events[1], next);
    let text = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
    assert_eq!(text.lines().count(), 2, "{text}");
}

use crate::reopen::tests::{CHAT, RUN};

#[test]
fn a_chat_reopened_after_a_relaunch_keeps_its_id_and_its_first_event_is_a_reopen_run() {
    use crate::state::Event::{SessionStart, Stop};
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    {
        let mut before = recorder(dir.path());
        before
            .begin(
                plane,
                7,
                RunOf {
                    chat: CHAT,
                    run: RUN,
                },
                Began::Start,
                &Attributes::default(),
            )
            .unwrap();
        before
            .report(plane, &report(7, Stop), Followed::No)
            .unwrap();
    }
    let relaunched_at = read(dir.path()).unwrap().len();

    // The next launch: a host with nothing in its memory, told by the record who chat 7 is.
    let mut after = recorder(dir.path());
    let again = "01K6E8ZK6V4Q9T0N3M2B1C5D7K";
    after
        .begin(
            plane,
            7,
            RunOf {
                chat: CHAT,
                run: again,
            },
            Began::Reopen,
            &Attributes::default(),
        )
        .unwrap();
    after
        .report(plane, &report(7, SessionStart), Followed::No)
        .unwrap();

    let events = read(dir.path()).unwrap();
    let since: Vec<_> = events[relaunched_at..]
        .iter()
        .map(|e| {
            (
                e.kind.as_str(),
                e.chat.as_deref(),
                e.run.as_deref(),
                e.body.get("cause").and_then(|c| c.as_str()),
            )
        })
        .collect();
    assert_eq!(
        since,
        vec![
            ("run.started", Some(CHAT), Some(again), Some("reopen")),
            ("hook.sessionstart", Some(CHAT), Some(again), None),
        ],
        "{events:#?}"
    );
    assert_eq!(
        events[0].chat.as_deref(),
        Some(CHAT),
        "the same chat as before"
    );
}

#[test]
fn fresh_and_switch_each_begin_a_run_in_the_same_chat() {
    use crate::state::Event::Stop;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());
    let runs = [
        "01K6E8ZK6V4Q9T0N3M2B1C5D70",
        "01K6E8ZK6V4Q9T0N3M2B1C5D71",
        "01K6E8ZK6V4Q9T0N3M2B1C5D72",
    ];

    host.begin(
        plane,
        3,
        RunOf {
            chat: CHAT,
            run: runs[0],
        },
        Began::Start,
        &Attributes::default(),
    )
    .unwrap();
    host.begin(
        plane,
        3,
        RunOf {
            chat: CHAT,
            run: runs[1],
        },
        Began::Fresh,
        &Attributes::default(),
    )
    .unwrap();
    host.report(plane, &report(3, Stop), Followed::No).unwrap();
    host.begin(
        plane,
        3,
        RunOf {
            chat: CHAT,
            run: runs[2],
        },
        Began::Switch,
        &Attributes::default(),
    )
    .unwrap();
    host.report(plane, &report(3, Stop), Followed::No).unwrap();

    let events = read(dir.path()).unwrap();
    let said: Vec<_> = events
        .iter()
        .map(|e| {
            (
                e.chat.as_deref().unwrap(),
                e.run.as_deref().unwrap(),
                e.body
                    .get("cause")
                    .and_then(|c| c.as_str())
                    .unwrap_or(&e.kind),
            )
        })
        .collect();
    assert_eq!(
        said,
        vec![
            (CHAT, runs[0], "start"),
            (CHAT, runs[1], "fresh"),
            (CHAT, runs[1], "hook.stop"),
            (CHAT, runs[2], "switch"),
            (CHAT, runs[2], "hook.stop"),
        ]
    );
}

#[test]
fn a_new_run_ends_the_sub_agents_of_the_one_before() {
    use crate::state::Event::Stop;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());
    let agent = |chat| crate::hookwire::Report {
        agent: Some("a1".to_owned()),
        ..report(chat, Stop)
    };

    host.begin(
        plane,
        3,
        RunOf {
            chat: CHAT,
            run: RUN,
        },
        Began::Start,
        &Attributes::default(),
    )
    .unwrap();
    host.report(plane, &agent(3), Followed::No).unwrap();
    let next = "01K6E8ZK6V4Q9T0N3M2B1C5D7K";
    host.begin(
        plane,
        3,
        RunOf {
            chat: CHAT,
            run: next,
        },
        Began::Switch,
        &Attributes::default(),
    )
    .unwrap();
    let after = host.report(plane, &agent(3), Followed::No).unwrap();

    assert_eq!(
        after.parent_run.as_deref(),
        Some(next),
        "the agent seen again is a child of the run now current, not of the one that ended"
    );
}

#[test]
fn a_chat_put_back_in_its_conversation_is_a_reopen_and_one_put_back_without_it_is_fresh() {
    use crate::harness::SessionId;
    use crate::reopen::{Fresh, Reopened};
    let resumed =
        Reopened::Resumed(SessionId::new("11111111-2222-4333-8444-555555555555").unwrap());
    let on = |how: Reopened, harness| Began::at_relaunch(&how, harness);

    assert_eq!(on(resumed, true), Began::Reopen);
    assert_eq!(
        on(Reopened::Fresh(Fresh::SessionNamedByTheOperator), true),
        Began::Reopen,
        "the operator's own arguments resume it"
    );
    assert_eq!(
        on(Reopened::Fresh(Fresh::NoResumeForThisProgram), false),
        Began::Reopen,
        "a shell has no conversation to lose"
    );
    for lost in [
        Fresh::WorkspaceRenamed,
        Fresh::NoConversationRecorded,
        Fresh::NoResumeForThisProgram,
    ] {
        assert_eq!(on(Reopened::Fresh(lost), true), Began::Fresh, "{lost:?}");
    }
}

#[test]
fn a_run_the_log_could_not_write_still_keeps_the_chat_under_its_id() {
    use crate::state::Event::Stop;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());

    host.log.fail_the_next_write_after(3);
    assert!(
        host.begin(
            plane,
            3,
            RunOf {
                chat: CHAT,
                run: RUN
            },
            Began::Reopen,
            &Attributes::default(),
        )
        .is_err()
    );
    let next = host.report(plane, &report(3, Stop), Followed::No).unwrap();

    assert_eq!(
        (next.chat.as_deref(), next.run.as_deref()),
        (Some(CHAT), Some(RUN)),
        "not a second id minted at the next line"
    );
}

#[test]
fn a_clear_begins_the_run_the_host_minted_for_it() {
    use crate::state::Event::{SessionStart, Stop};
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());
    host.begin(
        plane,
        3,
        RunOf {
            chat: CHAT,
            run: RUN,
        },
        Began::Start,
        &Attributes::default(),
    )
    .unwrap();

    let cleared = "01K6E8ZK6V4Q9T0N3M2B1C5D7K";
    let event = host
        .report_with(
            plane,
            &report(3, SessionStart),
            Followed::Moved,
            Some(cleared),
        )
        .unwrap();
    host.report(plane, &report(3, Stop), Followed::No).unwrap();

    assert_eq!(event.run.as_deref(), Some(cleared));
    let started: Vec<_> = read(dir.path())
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == "run.started")
        .map(|e| (e.run.unwrap(), e.body["cause"].as_str().unwrap().to_owned()))
        .collect();
    assert_eq!(
        started,
        vec![
            (RUN.to_owned(), "start".to_owned()),
            (cleared.to_owned(), "clear".to_owned())
        ]
    );
}

/// Appends `n` events to `log`, answering their seqs.
fn append_n(log: &mut Log, n: usize) -> Vec<u64> {
    (0..n)
        .map(|_| {
            log.append(
                Some("CHAT"),
                Some("RUN"),
                None,
                "hook.stop",
                serde_json::json!({}),
            )
            .unwrap()
            .seq
        })
        .collect()
}

/// The seqs of the events in `got`.
fn seqs(got: &[Delivery]) -> Vec<u64> {
    got.iter()
        .filter_map(|delivery| match delivery {
            Delivery::Event(event) => Some(event.seq),
            Delivery::Missed { .. } | Delivery::Ahead { .. } => None,
        })
        .collect()
}

#[test]
fn a_subscriber_gets_every_event_after_its_cursor_in_order_and_then_what_is_written_next() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    append_n(&mut log, 5);

    let mut subscription = subscribe(dir.path(), 2);
    let first = subscription.poll().unwrap();
    append_n(&mut log, 2);
    let second = subscription.poll().unwrap();
    let idle = subscription.poll().unwrap();

    assert_eq!(seqs(&first), vec![3, 4, 5]);
    assert_eq!(seqs(&second), vec![6, 7], "the stream stays open");
    assert!(idle.is_empty());
    assert_eq!(subscription.cursor(), 7);
}

#[test]
fn a_line_the_writer_died_in_is_never_delivered_and_its_seq_comes_once_from_the_next_writer() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut log = Log::open(dir.path(), DEVICE).unwrap();
        append_n(&mut log, 3);
    }
    let mut subscription = subscribe(dir.path(), 0);
    assert_eq!(seqs(&subscription.poll().unwrap()), vec![1, 2, 3]);
    // The host died in the middle of the fourth line.
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(dir.path().join(FILE))
        .unwrap();
    std::io::Write::write_all(&mut file, br#"{"v":1,"device_id":"X","seq":4,"ul"#).unwrap();
    drop(file);
    assert!(
        subscription.poll().unwrap().is_empty(),
        "half a line is no event"
    );

    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    append_n(&mut log, 2);
    let resubscribed = subscribe(dir.path(), subscription.cursor()).poll().unwrap();
    let carried_on = subscription.poll().unwrap();

    assert_eq!(seqs(&resubscribed), vec![4, 5]);
    assert_eq!(seqs(&carried_on), vec![4, 5]);
}

/// Seals a segment after about two events.
const SMALL: Retention = Retention {
    segment_bytes: 300,
    keep: Retention::DEFAULT.keep,
    hold: Retention::DEFAULT.hold,
};

#[test]
fn a_log_sealed_into_segments_reads_and_subscribes_as_one_stream() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::open_with(dir.path(), DEVICE, SMALL).unwrap();
    append_n(&mut log, 3);
    let mut subscription = subscribe(dir.path(), 1);
    let first = subscription.poll().unwrap();
    append_n(&mut log, 6);
    let second = subscription.poll().unwrap();
    drop(log);
    let mut log = Log::open_with(dir.path(), DEVICE, SMALL).unwrap();
    let next = append_n(&mut log, 1);

    let sealed = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("events."))
        .filter(|entry| entry.file_name() != FILE)
        .count();
    assert!(sealed >= 3, "sealed segments: {sealed}");
    assert_eq!(next, vec![10], "the numbering carries on across segments");
    assert_eq!(
        read(dir.path())
            .unwrap()
            .iter()
            .map(|e| e.seq)
            .collect::<Vec<_>>(),
        (1..=10).collect::<Vec<_>>()
    );
    assert_eq!(seqs(&first), vec![2, 3]);
    assert_eq!(seqs(&second), (4..=9).collect::<Vec<_>>());
    assert_eq!(
        seqs(&subscribe(dir.path(), 4).poll().unwrap()),
        (5..=10).collect::<Vec<_>>()
    );
}

/// Writes a sealed segment of events `seqs`, each made `days_ago`.
fn sealed_segment(dir: &Path, seqs: std::ops::RangeInclusive<u64>, days_ago: u64) {
    let then = std::time::SystemTime::now() - std::time::Duration::from_secs(days_ago * 86_400);
    let ms = then
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let first = *seqs.start();
    let lines: String = seqs
        .map(|seq| {
            let event = Event {
                v: VERSION,
                device_id: DEVICE.to_owned(),
                seq,
                ulid: ulid::Ulid::from_parts(ms, u128::from(seq)).to_string(),
                chat: Some("CHAT".to_owned()),
                run: None,
                parent_run: None,
                kind: "chat.opened".to_owned(),
                body: serde_json::json!({}),
            };
            format!("{}\n", serde_json::to_string(&event).unwrap())
        })
        .collect();
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(sealed_name(first)), lines).unwrap();
}

#[test]
fn a_sealed_segment_older_than_the_retention_is_deleted_and_the_newest_one_never_is() {
    let dir = tempfile::tempdir().unwrap();
    sealed_segment(dir.path(), 1..=2, 40);
    sealed_segment(dir.path(), 3..=4, 31);
    sealed_segment(dir.path(), 5..=6, 35);

    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    let next = append_n(&mut log, 1);

    assert_eq!(
        read(dir.path())
            .unwrap()
            .iter()
            .map(|e| e.seq)
            .collect::<Vec<_>>(),
        vec![5, 6, 7],
        "the newest sealed segment holds the last seq when the one being written is empty"
    );
    assert_eq!(next, vec![7]);
}

#[test]
fn a_sealed_segment_inside_the_retention_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    sealed_segment(dir.path(), 1..=2, 29);
    sealed_segment(dir.path(), 3..=4, 1);

    let _log = Log::open(dir.path(), DEVICE).unwrap();

    assert_eq!(read(dir.path()).unwrap().len(), 4);
}

#[test]
fn a_cursor_older_than_the_log_keeps_is_told_what_it_missed_and_carries_on_from_the_oldest_kept() {
    let dir = tempfile::tempdir().unwrap();
    sealed_segment(dir.path(), 1..=2, 40);
    sealed_segment(dir.path(), 3..=4, 1);
    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    append_n(&mut log, 1);

    let got = subscribe(dir.path(), 1).poll().unwrap();

    assert_eq!(
        got.first(),
        Some(&Delivery::Missed {
            after: 1,
            resumes_at: 3
        })
    );
    assert_eq!(seqs(&got), vec![3, 4, 5]);
    assert_eq!(
        seqs(&subscribe(dir.path(), 2).poll().unwrap()),
        vec![3, 4, 5],
        "a cursor at the edge of what is kept missed nothing"
    );
    assert!(
        !subscribe(dir.path(), 2)
            .poll()
            .unwrap()
            .iter()
            .any(|d| matches!(d, Delivery::Missed { .. }))
    );
}

#[test]
fn a_hole_in_the_middle_of_the_log_is_told_as_missed_never_skipped() {
    let dir = tempfile::tempdir().unwrap();
    // A clock that jumped: the first segment is dated recent, the middle one old.
    sealed_segment(dir.path(), 1..=2, 1);
    sealed_segment(dir.path(), 3..=4, 40);
    sealed_segment(dir.path(), 5..=6, 1);
    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    append_n(&mut log, 1);
    let on_disk: Vec<u64> = read(dir.path()).unwrap().iter().map(|e| e.seq).collect();
    let got = subscribe(dir.path(), 2).poll().unwrap();

    assert_eq!(
        on_disk,
        vec![1, 2, 5, 6, 7],
        "the middle segment was pruned"
    );
    assert_eq!(
        got.first(),
        Some(&Delivery::Missed {
            after: 2,
            resumes_at: 5
        })
    );
    assert_eq!(seqs(&got), vec![5, 6, 7]);
}

#[test]
fn a_reader_on_a_live_segment_sealed_and_pruned_between_polls_carries_on() {
    let dir = tempfile::tempdir().unwrap();
    let tiny = Retention {
        segment_bytes: 300,
        keep: std::time::Duration::from_millis(1),
        hold: Retention::DEFAULT.hold,
    };
    let mut log = Log::open_with(dir.path(), DEVICE, tiny).unwrap();
    append_n(&mut log, 1);
    let mut subscription = subscribe(dir.path(), 0);
    let first = seqs(&subscription.poll().unwrap());
    std::thread::sleep(std::time::Duration::from_millis(20));
    append_n(&mut log, 1); // seals segment 1
    std::thread::sleep(std::time::Duration::from_millis(20));
    append_n(&mut log, 5); // seals more, prunes segment 1
    let mut later = Vec::new();
    for _ in 0..5 {
        later.extend(subscription.poll().unwrap());
    }
    let on_disk: Vec<u64> = read(dir.path()).unwrap().iter().map(|e| e.seq).collect();

    assert_eq!(first, vec![1]);
    assert!(
        later.iter().any(|d| matches!(d, Delivery::Missed { .. }))
            || seqs(&later).last() == on_disk.last(),
        "stalled: later {:?}, on disk {on_disk:?}",
        seqs(&later)
    );
}

#[test]
fn a_seal_whose_next_segment_cannot_be_opened_writes_nothing_more_into_the_sealed_one() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::open_with(dir.path(), DEVICE, SMALL).unwrap();
    // The seal's own open and the next append's retry both fail, as a full directory would.
    log.fail_the_next_opens(2);

    let sealed_by = append_n(&mut log, 2);
    let refused = log.append(
        Some("CHAT"),
        Some("RUN"),
        None,
        "hook.stop",
        serde_json::json!({}),
    );
    let after = append_n(&mut log, 1);

    assert_eq!(sealed_by, vec![1, 2]);
    assert!(refused.is_err(), "no segment is open to write to");
    assert_eq!(after, vec![3], "the refused append used no seq");
    let sealed = std::fs::read_to_string(dir.path().join(sealed_name(1))).unwrap();
    assert_eq!(
        sealed.lines().count(),
        2,
        "the sealed segment is never written again"
    );
    assert_eq!(
        read(dir.path())
            .unwrap()
            .iter()
            .map(|e| e.seq)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
}

#[test]
fn a_seal_hands_its_hook_the_segment_the_next_events_go_to() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::open_with(dir.path(), DEVICE, SMALL).unwrap();
    let handed = std::sync::Arc::new(std::sync::Mutex::new(Vec::<File>::new()));
    let keep = std::sync::Arc::clone(&handed);
    log.on_seal(move |file| keep.lock().unwrap().push(file.try_clone().unwrap()));

    append_n(&mut log, 3);

    let handed = handed.lock().unwrap();
    assert_eq!(handed.len(), 1, "one seal, after the second event");
    let live = std::fs::metadata(dir.path().join(FILE)).unwrap().len();
    assert!(live > 0);
    assert_eq!(
        handed[0].metadata().unwrap().len(),
        live,
        "the handle the hook got is the file the third event is in"
    );
}

#[cfg(unix)]
#[test]
fn a_drained_line_is_recorded_under_the_chat_and_run_it_ran_in_and_says_it_was_spooled() {
    use crate::hookwire::spool::{Drained, Spooled};
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/plane");
    let mut recorder = recorder(dir.path());
    // The run chat 3 was in when the host went away, as the reopen record still says it.
    recorder.knows(
        plane,
        3,
        RunOf {
            chat: "01J9ZZCHAT00000000000000AA",
            run: "01J9ZZRUN000000000000000AA",
        },
    );

    let call = tool_call(3, "pretooluse", crate::hookwire::Decision::Allow);
    for item in [
        Drained::Line {
            chat: 3,
            seq: 1,
            line: Spooled::Tool(call),
        },
        Drained::Gap {
            chat: 3,
            from: 2,
            to: 4,
        },
        Drained::Rejected {
            chat: 3,
            seq: Some(7),
            why: crate::hookwire::spool::Why::Mac,
        },
        Drained::Spool {
            chat: 3,
            from: 1,
            to: 6,
        },
    ] {
        recorder.spooled(plane, item).unwrap();
    }

    let events = read(dir.path()).unwrap();
    let kinds: Vec<&str> = events.iter().map(|event| event.kind.as_str()).collect();
    assert_eq!(
        kinds,
        [
            "hook.pretooluse",
            "hook.spool.gap",
            "hook.spool.rejected",
            "hook.spool.drained"
        ],
        "no run.started: a drained line is the run it ran in"
    );
    for event in &events {
        assert_eq!(event.chat.as_deref(), Some("01J9ZZCHAT00000000000000AA"));
        assert_eq!(event.run.as_deref(), Some("01J9ZZRUN000000000000000AA"));
    }
    assert_eq!(events[0].body["spooled"], 1);
    assert_eq!(events[0].body["decision"], "allow");
    assert_eq!(
        (
            events[1].body["from"].as_u64(),
            events[1].body["to"].as_u64()
        ),
        (Some(2), Some(4))
    );
    assert_eq!(events[2].body["seq"], 7);
    assert_eq!(events[2].body["why"], "mac");
    assert_eq!(events[3].body["to"], 6);
}

#[cfg(unix)]
#[test]
fn every_rejection_is_written_in_its_own_word() {
    // #983: the drain's reasons are an enum, and the words `hook.spool.rejected` carries are
    // what audit readers match on. Each is pinned here, byte for byte.
    use crate::hookwire::spool::{Drained, Why};
    let dir = tempfile::tempdir().unwrap();
    let mut recorder = recorder(dir.path());
    let words = [
        (Why::Unreadable, "unreadable"),
        (Why::NoKey, "no-key"),
        (Why::AnotherChatsKey, "another-chats-key"),
        (Why::Mac, "mac"),
        (Why::NotThisChats, "not-this-chats"),
        (Why::Repeated, "repeated"),
        (Why::Unfinished, "unfinished"),
    ];

    for (why, _) in words {
        recorder
            .spooled(
                Path::new("/plane"),
                Drained::Rejected {
                    chat: 4,
                    seq: None,
                    why,
                },
            )
            .unwrap();
    }

    let events = read(dir.path()).unwrap();
    let written: Vec<&str> = events
        .iter()
        .map(|event| event.body["why"].as_str().unwrap())
        .collect();
    let want: Vec<&str> = words.iter().map(|(_, word)| *word).collect();
    assert_eq!(written, want);
}

#[cfg(unix)]
#[test]
fn a_spooled_event_says_spooled_after_what_it_always_said() {
    // #983: one place marks an event as drained from the spool, and the keys of every such
    // event stay in the order they were written before.
    use crate::hookwire::spool::{Drained, Spooled};
    let refused = |chat| crate::hookwire::CommitRefused {
        chat,
        commit_refused: "svc: a key-shaped line".to_owned(),
    };
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/plane");
    let mut recorder = recorder(dir.path());
    recorder.knows(
        plane,
        3,
        RunOf {
            chat: "01J9ZZCHAT00000000000000AA",
            run: "01J9ZZRUN000000000000000AA",
        },
    );
    for item in [
        Drained::Line {
            chat: 3,
            seq: 1,
            line: Spooled::Report(report(3, crate::state::Event::SessionStart)),
        },
        Drained::Line {
            chat: 3,
            seq: 2,
            line: Spooled::Refused(refused(3)),
        },
        Drained::Line {
            chat: 9,
            seq: 3,
            line: Spooled::Report(report(9, crate::state::Event::Stop)),
        },
        Drained::Line {
            chat: 9,
            seq: 4,
            line: Spooled::Refused(refused(9)),
        },
    ] {
        recorder.spooled(plane, item).unwrap();
    }

    let keys: Vec<Vec<String>> = read(dir.path())
        .unwrap()
        .iter()
        .map(|event| event.body.as_object().unwrap().keys().cloned().collect())
        .collect();
    assert_eq!(
        keys,
        [
            vec!["started", "spooled"],
            vec!["spooled"],
            vec!["spooled", "chat_number"],
            vec!["spooled", "chat_number"],
        ]
    );
}

#[cfg(unix)]
#[test]
fn a_drained_line_of_a_chat_the_host_cannot_name_is_recorded_under_its_number() {
    use crate::hookwire::spool::{Drained, Spooled};
    let dir = tempfile::tempdir().unwrap();
    let mut recorder = recorder(dir.path());

    recorder
        .spooled(
            Path::new("/plane"),
            Drained::Line {
                chat: 9,
                seq: 1,
                line: Spooled::Report(report(9, crate::state::Event::Stop)),
            },
        )
        .unwrap();

    let events = read(dir.path()).unwrap();
    assert_eq!(events.len(), 1, "no run is made up for it: {events:?}");
    assert_eq!(events[0].kind, "hook.stop");
    assert_eq!(events[0].chat, None);
    assert_eq!(events[0].body["chat_number"], 9);
}

#[test]
fn an_event_is_made_durable_through_its_number() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    let durable = log.durable();
    let event = log
        .append(None, None, None, "hook.stop", serde_json::json!({}))
        .unwrap();

    durable.through(event.seq).unwrap();

    assert_eq!(durable.synced(), event.seq);
    durable.through(event.seq).unwrap();
    assert_eq!(
        durable.synced(),
        event.seq,
        "a number already durable is not synced again"
    );
}

#[cfg(unix)]
#[test]
fn what_makes_events_durable_follows_each_seal_onto_the_segment_being_written() {
    use std::os::unix::fs::MetadataExt;
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::open_with(dir.path(), DEVICE, SMALL).unwrap();
    let durable = log.durable();
    let before = append_n(&mut log, 1)[0];
    durable.through(before).unwrap();

    // Enough to seal at least once, and one more event in the new segment.
    let seqs = append_n(&mut log, 8);
    assert!(
        !segments(dir.path()).unwrap().is_empty(),
        "the log sealed a segment"
    );
    let last = *seqs.last().unwrap();
    durable.through(last).unwrap();

    let live = std::fs::metadata(dir.path().join(FILE)).unwrap().ino();
    assert_eq!(
        durable.inode(),
        Some(live),
        "the handle synced is the segment being written, not a sealed one"
    );
    assert_eq!(durable.synced(), last);
}

// -------------------------------------------------------------------------------------
// The sandbox's trust events (ADR 0067 §7, ADR 0075 as amended): until the audit exists,
// they are written here, under the chat and the run they are about.
// -------------------------------------------------------------------------------------

#[test]
fn a_person_turning_the_sandbox_off_is_a_trust_event_under_the_chats_run() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());

    let event = host
        .trust(
            RunOf {
                chat: CHAT,
                run: RUN,
            },
            &crate::sandbox::Change::Off(crate::sandbox::Lifted {
                by: crate::sandbox::By::Person,
                reason: Some("needs the network".to_owned()),
            }),
            Some(crate::harness::Harness::ClaudeCode),
            Some("steward"),
        )
        .unwrap();

    assert_eq!(event.kind, "trust.sandbox.off");
    assert_eq!(
        (event.chat.as_deref(), event.run.as_deref()),
        (Some(CHAT), Some(RUN))
    );
    assert_eq!(event.device_id, DEVICE, "the machine it happened on");
    assert_eq!(
        event.body,
        serde_json::json!({
            "actor_kind": "human",
            "actor": "operator",
            "scope": "local-ui",
            "harness": "claude",
            "persona": "steward",
            "reason": "needs the network",
            "lifted": ["vaults", "integrity", "human-powers", "runner-internals", "later-code"],
        })
    );
    assert_eq!(read(dir.path()).unwrap().last(), Some(&event), "written");
}

#[test]
fn a_windows_start_without_the_sandbox_names_charter_as_the_actor_never_the_operator() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());

    let event = host
        .trust(
            RunOf {
                chat: CHAT,
                run: RUN,
            },
            &crate::sandbox::Change::Off(crate::sandbox::Lifted {
                by: crate::sandbox::By::NoBackend(crate::sandbox::Os::Windows),
                reason: None,
            }),
            Some(crate::harness::Harness::Codex),
            None,
        )
        .unwrap();

    assert_eq!(
        event.body,
        serde_json::json!({
            "actor_kind": "host",
            "actor": "purlis (no backend on this OS)",
            "os": "windows",
            "harness": "codex",
            "persona": null,
            "reason": null,
            "lifted": ["vaults", "integrity", "human-powers", "runner-internals", "later-code"],
        })
    );
}

#[test]
fn the_sandbox_coming_back_on_for_a_chat_is_a_trust_event_too() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());

    let event = host
        .trust(
            RunOf {
                chat: CHAT,
                run: RUN,
            },
            &crate::sandbox::Change::On,
            Some(crate::harness::Harness::ClaudeCode),
            None,
        )
        .unwrap();

    assert_eq!(event.kind, "trust.sandbox.on");
    assert_eq!(
        event.body,
        serde_json::json!({
            "actor_kind": "host",
            "actor": "charter",
            "harness": "claude",
            "persona": null,
            "restored": ["vaults", "integrity", "human-powers", "runner-internals", "later-code"],
        })
    );
}

#[test]
fn every_sandbox_grant_and_revoke_is_a_trust_event_naming_what_it_named() {
    // #1342: a grant reaches a chat only once it is recorded, under the chat it came from.
    use crate::sandbox::grant::{Audited, Level};
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());

    let granted = host
        .sandbox_grant(
            plane,
            Some(3),
            &Audited {
                granted: true,
                what: "host",
                target: "api.example.com",
                level: Level::Chat,
            },
        )
        .unwrap();
    assert_eq!(granted.kind, "trust.sandbox.grant");
    assert!(
        granted.chat.is_some() && granted.run.is_some(),
        "{granted:?}"
    );
    assert_eq!(
        granted.body,
        serde_json::json!({
            "actor_kind": "human",
            "actor": "operator",
            "scope": "local-ui",
            "level": "chat",
            "what": "host",
            "target": "api.example.com",
        })
    );

    // Taken back in Settings, where no chat asked.
    let revoked = host
        .sandbox_grant(
            plane,
            None,
            &Audited {
                granted: false,
                what: "write",
                target: "/opt/cache",
                level: Level::You,
            },
        )
        .unwrap();
    assert_eq!(revoked.kind, "trust.sandbox.revoke");
    assert_eq!(revoked.chat, None);
    assert_eq!(read(dir.path()).unwrap().last(), Some(&revoked), "written");
}

#[test]
fn every_dispatch_grant_and_revoke_is_a_trust_event_naming_the_pair() {
    // #1437: a dispatch grant is in force only once it is recorded, under the chat it was
    // allowed from.
    use crate::dispatchgrant::Audited;
    use crate::sandbox::grant::Level;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());

    let granted = host
        .dispatch_grant(
            plane,
            Some(3),
            &Audited {
                act: crate::dispatchgrant::Act::Grant,
                asking: Some("steward"),
                target: "devops",
                level: Level::You,
                workspace: None,
            },
        )
        .unwrap();
    assert_eq!(granted.kind, "trust.dispatch.grant");
    assert!(
        granted.chat.is_some() && granted.run.is_some(),
        "{granted:?}"
    );
    assert_eq!(
        granted.body,
        serde_json::json!({
            "actor_kind": "human",
            "actor": "operator",
            "scope": "local-ui",
            "level": "you",
            "asking": "steward",
            "target": "devops",
            "workspace": null,
        })
    );

    // Taken back in Settings, where no chat asked.
    let revoked = host
        .dispatch_grant(
            plane,
            None,
            &Audited {
                act: crate::dispatchgrant::Act::Revoke,
                asking: Some("steward"),
                target: "devops",
                level: Level::You,
                workspace: None,
            },
        )
        .unwrap();
    assert_eq!(revoked.kind, "trust.dispatch.revoke");
    assert_eq!(revoked.chat, None);
    assert_eq!(read(dir.path()).unwrap().last(), Some(&revoked), "written");
}

#[test]
fn a_grant_made_on_a_refusal_kept_while_nobody_was_there_says_where_it_came_from() {
    // #1507: the same trust event as any grant of the person's, with `from` beside the pair,
    // so it can be told from one made in Settings; and, as every such event since #1505, with
    // the workspace the grant holds in, which is the one the refused task would have worked
    // in.
    use crate::dispatchgrant::Audited;
    use crate::sandbox::grant::Level;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());

    let granted = host
        .dispatch_grant_from(
            plane,
            None,
            &Audited {
                act: crate::dispatchgrant::Act::Grant,
                asking: Some("steward"),
                target: "devops",
                level: Level::You,
                workspace: Some("ide"),
            },
            crate::dispatchaway::AUDITED_FROM,
        )
        .unwrap();
    assert_eq!(granted.kind, "trust.dispatch.grant");
    assert_eq!(granted.chat, None);
    assert_eq!(
        granted.body,
        serde_json::json!({
            "actor_kind": "human",
            "actor": "operator",
            "scope": "local-ui",
            "level": "you",
            "asking": "steward",
            "target": "devops",
            "workspace": "ide",
            "from": "away",
        })
    );
    assert_eq!(read(dir.path()).unwrap().last(), Some(&granted), "written");
}

#[cfg(target_os = "macos")]
#[test]
fn a_segment_the_filesystem_refuses_to_open_names_the_segment() {
    // #1421: an EPERM here printed only "Operation not permitted (os error 1)".
    let dir = tempfile::tempdir().unwrap();
    let _frozen = crate::rewrite::frozen::Frozen::at(dir.path());
    let segment = dir.path().join(FILE);

    let refused = private_file(&segment).unwrap_err();

    crate::rewrite::frozen::names(&refused, &segment);
}

/// The `run.ended` events in the log, as (run, parent run, state, cause).
fn run_ends(dir: &Path) -> Vec<(String, Option<String>, String, String)> {
    read(dir)
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == "run.ended")
        .map(|e| {
            (
                e.run.unwrap_or_default(),
                e.parent_run,
                e.body["state"].as_str().unwrap_or_default().to_owned(),
                e.body["cause"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

/// A tool call from sub-agent `agent` of chat `chat`.
fn by_agent(chat: u32, agent: &str) -> crate::hookwire::ToolCall {
    crate::hookwire::ToolCall {
        agent: Some(agent.to_owned()),
        ..tool_call(chat, "pretooluse", crate::hookwire::Decision::None)
    }
}

#[test]
fn a_sub_agents_stop_ends_its_child_run_completed_and_child_ended_once() {
    use crate::state::Event::{SubagentStop, UserPromptSubmit};
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/p");
    let mut host = recorder(dir.path());
    let parent = host
        .report(plane, &report(5, UserPromptSubmit), Followed::No)
        .unwrap()
        .run
        .unwrap();
    let child = host
        .tool(plane, &by_agent(5, "agent-1"), Instant::now())
        .unwrap()
        .run
        .unwrap();
    let stop = crate::hookwire::Report {
        agent: Some("agent-1".to_owned()),
        ..report(5, SubagentStop)
    };

    host.report(plane, &stop, Followed::No).unwrap();
    // A second stop of the same agent is the parent run's line, and ends nothing again.
    host.report(plane, &stop, Followed::No).unwrap();

    assert_eq!(
        run_ends(dir.path()),
        vec![(
            child,
            Some(parent),
            "completed".to_owned(),
            "child-ended".to_owned()
        )]
    );
    let kinds: Vec<String> = read(dir.path())
        .unwrap()
        .into_iter()
        .map(|e| e.kind)
        .collect();
    let stop_at = kinds.iter().position(|k| k == "hook.subagentstop").unwrap();
    assert_eq!(
        kinds[stop_at + 1],
        "run.ended",
        "the child ends after its stop is recorded: {kinds:?}"
    );
}

#[test]
fn a_parents_end_ends_each_live_child_in_the_parents_end_state_and_cause() {
    use crate::state::Event::{SubagentStop, UserPromptSubmit};
    use crate::state::run::{Cause, RunState, StopBy};
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/p");
    let mut host = recorder(dir.path());
    let parent = host
        .report(plane, &report(5, UserPromptSubmit), Followed::No)
        .unwrap()
        .run
        .unwrap();
    let one = host
        .tool(plane, &by_agent(5, "agent-1"), Instant::now())
        .unwrap()
        .run
        .unwrap();
    let two = host
        .tool(plane, &by_agent(5, "agent-2"), Instant::now())
        .unwrap()
        .run
        .unwrap();
    let done = host
        .tool(plane, &by_agent(5, "agent-3"), Instant::now())
        .unwrap()
        .run
        .unwrap();
    let stop = crate::hookwire::Report {
        agent: Some("agent-3".to_owned()),
        ..report(5, SubagentStop)
    };
    host.report(plane, &stop, Followed::No).unwrap();
    // Another chat's sub-agent is not this run's child.
    let other = host
        .tool(plane, &by_agent(6, "agent-1"), Instant::now())
        .unwrap()
        .run
        .unwrap();

    let ended = host
        .end_children_with(plane, 5, RunState::Stopped, Cause::Stop(StopBy::Killed))
        .unwrap();
    // Told again, there is nothing left to end.
    let again = host
        .end_children_with(plane, 5, RunState::Stopped, Cause::Stop(StopBy::Killed))
        .unwrap();

    assert_eq!(ended.len(), 2);
    assert!(again.is_empty());
    let mut ends = run_ends(dir.path());
    ends.sort();
    let mut want = vec![
        (
            done,
            Some(parent.clone()),
            "completed".to_owned(),
            "child-ended".to_owned(),
        ),
        (
            one,
            Some(parent.clone()),
            "stopped".to_owned(),
            "killed".to_owned(),
        ),
        (two, Some(parent), "stopped".to_owned(), "killed".to_owned()),
    ];
    want.sort();
    assert_eq!(ends, want);
    assert!(
        !ends.iter().any(|(run, ..)| *run == other),
        "another chat's child is left running"
    );
}

#[test]
fn a_parents_end_that_is_no_end_state_is_refused_and_ends_nothing() {
    use crate::state::run::{Cause, RunState};
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/p");
    let mut host = recorder(dir.path());
    host.tool(plane, &by_agent(5, "agent-1"), Instant::now())
        .unwrap();

    let refused = host.end_children_with(plane, 5, RunState::Working, Cause::Prompted);

    assert_eq!(
        refused.unwrap_err().kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert!(run_ends(dir.path()).is_empty());
}

#[test]
fn a_clear_ends_the_live_children_completed_and_superseded_before_the_next_run_starts() {
    use crate::state::Event::{SessionStart, UserPromptSubmit};
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/p");
    let mut host = recorder(dir.path());
    let parent = host
        .report(plane, &report(5, UserPromptSubmit), Followed::No)
        .unwrap()
        .run
        .unwrap();
    let child = host
        .tool(plane, &by_agent(5, "agent-1"), Instant::now())
        .unwrap()
        .run
        .unwrap();

    host.report(plane, &report(5, SessionStart), Followed::Moved)
        .unwrap();

    assert_eq!(
        run_ends(dir.path()),
        vec![(
            child,
            Some(parent),
            "completed".to_owned(),
            "superseded".to_owned()
        )]
    );
    let kinds: Vec<String> = read(dir.path())
        .unwrap()
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert_eq!(
        kinds[kinds.len() - 3..],
        ["run.ended", "run.started", "hook.sessionstart"],
        "the child ends before the run that supersedes its parent begins"
    );
}

#[test]
fn a_session_end_ends_the_children_superseded_at_a_clear_and_exited_for_good() {
    use crate::state::Event::{SessionEnd, UserPromptSubmit};
    use crate::state::{Detail, Ending};
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/p");
    let mut host = recorder(dir.path());
    host.report(plane, &report(5, UserPromptSubmit), Followed::No)
        .unwrap();
    host.report(plane, &report(6, UserPromptSubmit), Followed::No)
        .unwrap();
    let cleared = host
        .tool(plane, &by_agent(5, "agent-1"), Instant::now())
        .unwrap()
        .run
        .unwrap();
    let for_good = host
        .tool(plane, &by_agent(6, "agent-1"), Instant::now())
        .unwrap()
        .run
        .unwrap();
    let ending = |chat, ending| crate::hookwire::Report {
        detail: Detail {
            ending,
            ..Detail::default()
        },
        ..report(chat, SessionEnd)
    };

    host.report(plane, &ending(5, Ending::Cleared), Followed::No)
        .unwrap();
    host.report(plane, &ending(6, Ending::ForGood), Followed::No)
        .unwrap();

    let ends: Vec<_> = run_ends(dir.path())
        .into_iter()
        .map(|(run, _, state, cause)| (run, state, cause))
        .collect();
    assert_eq!(
        ends,
        vec![
            (cleared, "completed".to_owned(), "superseded".to_owned()),
            (for_good, "completed".to_owned(), "exited".to_owned()),
        ]
    );
}

#[test]
fn a_new_run_begun_over_a_live_child_ends_it_completed_and_superseded() {
    use crate::state::Event::Stop;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());
    host.begin(
        plane,
        3,
        RunOf {
            chat: CHAT,
            run: RUN,
        },
        Began::Start,
        &Attributes::default(),
    )
    .unwrap();
    let child = host
        .report(
            plane,
            &crate::hookwire::Report {
                agent: Some("a1".to_owned()),
                ..report(3, Stop)
            },
            Followed::No,
        )
        .unwrap()
        .run
        .unwrap();

    host.begin(
        plane,
        3,
        RunOf {
            chat: CHAT,
            run: "01K6E8ZK6V4Q9T0N3M2B1C5D7K",
        },
        Began::Switch,
        &Attributes::default(),
    )
    .unwrap();

    assert_eq!(
        run_ends(dir.path()),
        vec![(
            child,
            Some(RUN.to_owned()),
            "completed".to_owned(),
            "superseded".to_owned()
        )]
    );
}

#[test]
fn a_cursor_ahead_of_the_log_is_told_so_and_gets_every_event_written_after() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    // The client held 1 to 5; a power loss took 4 and 5 off the disk.
    append_n(&mut log, 3);
    let mut subscription = subscribe(dir.path(), 5);

    let told = subscription.poll().unwrap();
    // The next host numbers its events from 4 again: each one reaches the client.
    append_n(&mut log, 3);
    let after = subscription.poll().unwrap();
    let then = subscription.poll().unwrap();

    assert_eq!(
        told,
        vec![Delivery::Ahead {
            cursor: 5,
            log_ends_at: 3
        }]
    );
    assert_eq!(seqs(&after), vec![4, 5, 6]);
    assert_eq!(after.len(), 3, "told once: {after:?}");
    assert!(then.is_empty());
    assert_eq!(subscription.cursor(), 6);
}

#[test]
fn a_cursor_ahead_of_a_log_with_nothing_in_it_is_told_so_too() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    let mut subscription = subscribe(dir.path(), 2);

    let told = subscription.poll().unwrap();
    append_n(&mut log, 1);

    assert_eq!(
        told,
        vec![Delivery::Ahead {
            cursor: 2,
            log_ends_at: 0
        }]
    );
    assert_eq!(seqs(&subscription.poll().unwrap()), vec![1]);
}

#[test]
fn a_cursor_at_the_end_of_the_log_or_behind_it_is_never_told_it_is_ahead() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    append_n(&mut log, 3);
    let ahead = |got: &[Delivery]| got.iter().any(|d| matches!(d, Delivery::Ahead { .. }));

    let mut at_the_end = subscribe(dir.path(), 3);
    assert!(!ahead(&at_the_end.poll().unwrap()));
    assert!(!ahead(&subscribe(dir.path(), 1).poll().unwrap()));
    assert!(!ahead(&subscribe(dir.path(), 0).poll().unwrap()));
    append_n(&mut log, 1);
    assert_eq!(seqs(&at_the_end.poll().unwrap()), vec![4]);
}

#[test]
fn a_cursor_ahead_of_a_log_whose_segment_was_just_sealed_is_measured_on_the_sealed_one() {
    let dir = tempfile::tempdir().unwrap();
    sealed_segment(dir.path(), 1..=4, 1);
    let _log = Log::open(dir.path(), DEVICE).unwrap();

    assert!(subscribe(dir.path(), 4).poll().unwrap().is_empty());
    assert_eq!(
        subscribe(dir.path(), 6).poll().unwrap(),
        vec![Delivery::Ahead {
            cursor: 6,
            log_ends_at: 4
        }]
    );
}

/// The kill-switch events in the log at `dir`, as (kind, actor_kind, by, at).
fn kill_switch_events(dir: &Path) -> Vec<(String, String, String, u64)> {
    read(dir)
        .unwrap()
        .into_iter()
        .filter(|event| event.kind.starts_with("killswitch."))
        .map(|event| {
            assert_eq!((event.chat.as_deref(), event.run.as_deref()), (None, None));
            (
                event.kind,
                event.body["actor_kind"].as_str().unwrap().to_owned(),
                event.body["by"].as_str().unwrap().to_owned(),
                event.body["at"].as_u64().unwrap(),
            )
        })
        .collect()
}

fn kill_switch(kind: &str, actor_kind: &str, by: &str, at: u64) -> (String, String, String, u64) {
    (kind.to_owned(), actor_kind.to_owned(), by.to_owned(), at)
}

#[test]
fn each_kill_switch_journal_line_is_one_event_saying_who_wrote_it() {
    // #1375, ADR 0075 ("ADR 0071, amended"): a window line is a person's, an app line the
    // host's, and a cli line nobody's in particular.
    use crate::halt::{self, Actor};
    let dir = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let now = halt::now();
    halt::stop(config.path(), Actor::Cli, now - 30).unwrap();
    std::fs::remove_file(halt::marker(config.path())).unwrap();
    halt::restore(config.path(), now - 20).unwrap();
    halt::rearm(config.path(), now - 10).unwrap();
    let mut host = recorder(dir.path());

    let written = host.kill_switch_read(config.path()).unwrap();

    assert_eq!(written.len(), 3);
    assert_eq!(
        kill_switch_events(dir.path()),
        [
            kill_switch("killswitch.stop", "unattributed", "cli", now - 30),
            kill_switch("killswitch.tamper", "host", "app", now - 20),
            kill_switch("killswitch.rearm", "human", "window", now - 10),
        ]
    );
}

#[test]
fn a_kill_switch_line_read_twice_or_after_a_relaunch_is_one_event() {
    use crate::halt::{self, Actor};
    let dir = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let now = halt::now();
    halt::stop(config.path(), Actor::Window, now).unwrap();
    {
        let mut host = recorder(dir.path());
        assert_eq!(
            host.kill_switch_read(config.path()).unwrap().len(),
            1,
            "at start"
        );
        assert!(
            host.kill_switch_read(config.path()).unwrap().is_empty(),
            "the watch"
        );
    }

    // The next launch reads the same journal, and finds the line already in its log.
    let mut host = recorder(dir.path());
    assert!(host.kill_switch_read(config.path()).unwrap().is_empty());
    halt::stop(config.path(), Actor::Cli, now + 1).unwrap();
    assert_eq!(
        host.kill_switch_read(config.path()).unwrap().len(),
        1,
        "only the new line"
    );
    assert_eq!(
        kill_switch_events(dir.path()),
        [
            kill_switch("killswitch.stop", "human", "window", now),
            kill_switch("killswitch.stop", "unattributed", "cli", now + 1),
        ]
    );
}

#[test]
fn two_same_kill_switch_lines_in_one_second_are_two_events() {
    use crate::halt::{self, Actor};
    let dir = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let now = halt::now();
    halt::stop(config.path(), Actor::Cli, now).unwrap();
    halt::stop(config.path(), Actor::Cli, now).unwrap();
    let mut host = recorder(dir.path());

    assert_eq!(host.kill_switch_read(config.path()).unwrap().len(), 2);
    assert!(host.kill_switch_read(config.path()).unwrap().is_empty());
}

#[test]
fn a_kill_switch_line_the_host_wrote_is_recorded_though_the_journal_lost_it() {
    // A tamper the app journaled, then a process of the user rewrote the journal without it:
    // the host records what it wrote itself, and the line coming back is not a second event.
    use crate::halt::{self, Actor, Entry, Event as Switch};
    let dir = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let tamper = Entry {
        at: halt::now(),
        event: Switch::Tamper,
        by: Actor::App,
    };
    let mut host = recorder(dir.path());

    assert_eq!(
        host.kill_switch_wrote(tamper, config.path()).unwrap().len(),
        1
    );
    std::fs::create_dir_all(crate::machine::dir(config.path())).unwrap();
    std::fs::write(
        halt::journal_path(config.path()),
        format!("{}\n", serde_json::to_string(&tamper).unwrap()),
    )
    .unwrap();
    assert!(host.kill_switch_read(config.path()).unwrap().is_empty());
    assert_eq!(
        kill_switch_events(dir.path()),
        [kill_switch("killswitch.tamper", "host", "app", tamper.at)]
    );
}

#[test]
fn a_kill_switch_line_older_than_the_log_keeps_is_not_recorded() {
    use crate::halt::{self, Actor};
    let dir = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let long_ago = halt::now() - Retention::DEFAULT.keep.as_secs() - 60;
    halt::stop(config.path(), Actor::Cli, long_ago).unwrap();
    let mut host = recorder(dir.path());

    assert!(host.kill_switch_read(config.path()).unwrap().is_empty());
}

#[test]
fn the_host_records_the_kill_switch_s_journal_as_it_opens_its_log() {
    use crate::halt::{self, Actor};
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    std::fs::create_dir_all(&config).unwrap();
    let data = dir.path().join("data");
    let now = halt::now();
    halt::stop(&config, Actor::Cli, now).unwrap();

    let host = Recorder::open_in(&config, &data).unwrap();

    assert_eq!(
        kill_switch_events(host.dir()),
        [kill_switch("killswitch.stop", "unattributed", "cli", now)]
    );
}

#[test]
fn a_start_whose_journal_the_log_already_held_whole_reads_no_log_back() {
    // #1375: the read back through the log for a journal line can cover the whole
    // retention, so a start that finds the journal as the last read left it skips it; one
    // that finds it changed reads, and a new line is still one event.
    use crate::halt::{self, Actor};
    let dir = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let now = halt::now();
    halt::stop(config.path(), Actor::Window, now).unwrap();
    {
        let mut host = recorder(dir.path());
        assert_eq!(host.kill_switch_read(config.path()).unwrap().len(), 1);
    }
    assert!(dir.path().join(KILL_SWITCH_READ).is_file());

    let mut host = recorder(dir.path());
    assert!(host.kill_switch_read(config.path()).unwrap().is_empty());
    assert!(host.kill_switch.is_none(), "the log was not read back");
    halt::stop(config.path(), Actor::Cli, now + 1).unwrap();
    assert_eq!(host.kill_switch_read(config.path()).unwrap().len(), 1);

    drop(host);
    // A cache that says something else is a read, never a line left out.
    std::fs::write(dir.path().join(KILL_SWITCH_READ), "0 0\n").unwrap();
    let mut host = recorder(dir.path());
    assert!(host.kill_switch_read(config.path()).unwrap().is_empty());
    assert!(host.kill_switch.is_some(), "the log was read back");
    assert_eq!(kill_switch_events(dir.path()).len(), 2);
}

/// Every attribute `run.started`'s body carries (#868): ADR 0066's, as ADR 0087 amends them, with
/// the chat and the parent run in the envelope.
const RUN_ATTRIBUTES: [&str; 11] = [
    "cause",
    "device",
    "persona",
    "harness",
    "profile",
    "model_source",
    "provider",
    "model",
    "sandbox",
    "conversation",
    "started_by",
];

/// The `run.started` events in the log under `dir`, in order.
fn runs_started(dir: &Path) -> Vec<Event> {
    read(dir)
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == "run.started")
        .collect()
}

/// `run.started`'s body, by the keys of [`RUN_ATTRIBUTES`], as the words it says.
fn said(event: &Event) -> Vec<(&'static str, String)> {
    RUN_ATTRIBUTES
        .iter()
        .map(|key| {
            let value = event
                .body
                .get(*key)
                .unwrap_or_else(|| panic!("{key} is left out of run.started: {:#}", event.body));
            (*key, value.as_str().unwrap_or_default().to_owned())
        })
        .collect()
}

/// #868: a run the host begins says, in its `run.started`, every attribute ADR 0066 names for
/// it, read back from the event log; a `/clear` carries them on into the next run with the
/// conversation the board followed it onto.
#[test]
fn run_started_carries_every_attribute_adr_0066_names_for_the_run_read_back_from_the_log() {
    use crate::state::Event::SessionStart;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());
    let attributes = Attributes {
        persona: Some("steward".to_owned()),
        harness: Some("claude".to_owned()),
        profile: Some("work".to_owned()),
        model_source: None,
        provider: None,
        model: None,
        sandbox: Some("seatbelt".to_owned()),
        conversation: Some("0f6c2a1e-aaaa-4bbb-8ccc-123456789abc".to_owned()),
        started_by: Some("terminal".to_owned()),
    };
    host.begin(
        plane,
        3,
        RunOf {
            chat: CHAT,
            run: RUN,
        },
        Began::Start,
        &attributes,
    )
    .unwrap();
    let mut cleared = report(3, SessionStart);
    cleared.conversation =
        crate::hookwire::Conversation::Named("1a7d3b2e-bbbb-4ccc-8ddd-abcdef012345".to_owned());
    host.report(plane, &cleared, Followed::Moved).unwrap();

    let started = runs_started(dir.path());
    assert_eq!(
        said(&started[0]),
        vec![
            ("cause", "start".to_owned()),
            ("device", DEVICE.to_owned()),
            ("persona", "steward".to_owned()),
            ("harness", "claude".to_owned()),
            ("profile", "work".to_owned()),
            ("model_source", "unknown".to_owned()),
            ("provider", "unknown".to_owned()),
            ("model", "unknown".to_owned()),
            ("sandbox", "seatbelt".to_owned()),
            (
                "conversation",
                "0f6c2a1e-aaaa-4bbb-8ccc-123456789abc".to_owned()
            ),
            ("started_by", "terminal".to_owned()),
        ]
    );
    assert_eq!(started[0].chat.as_deref(), Some(CHAT));
    let mut carried_on = said(&started[0]);
    carried_on[0].1 = "clear".to_owned();
    carried_on[9].1 = "1a7d3b2e-bbbb-4ccc-8ddd-abcdef012345".to_owned();
    assert_eq!(
        said(&started[1]),
        carried_on,
        "a clear is the same persona, harness, profile and sandbox in a new conversation"
    );
}

/// #868: a run whose attribute the host does not have says so, `unknown`, rather than leaving
/// the key out: a chat the host was never told of, a run begun with nothing known, and a
/// sub-agent's child run, which has its parent's attributes but a conversation of its own.
#[test]
fn a_run_whose_attribute_is_unknown_says_so_rather_than_leaving_the_key_out() {
    use crate::hookwire::Decision;
    use crate::state::Event::UserPromptSubmit;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());

    // Never told: its first line gives it an id and a run, and nothing else is known.
    host.report(plane, &report(9, UserPromptSubmit), Followed::No)
        .unwrap();
    // Begun with a persona and nothing else, then a sub-agent's first call.
    host.begin(
        plane,
        4,
        RunOf {
            chat: CHAT,
            run: RUN,
        },
        Began::Start,
        &Attributes {
            persona: Some("steward".to_owned()),
            conversation: Some("0f6c2a1e-aaaa-4bbb-8ccc-123456789abc".to_owned()),
            ..Attributes::default()
        },
    )
    .unwrap();
    let mut by_agent = tool_call(4, "pretooluse", Decision::None);
    by_agent.agent = Some("agent-1".to_owned());
    host.tool(plane, &by_agent, std::time::Instant::now())
        .unwrap();

    let started = runs_started(dir.path());
    assert_eq!(started.len(), 3, "{started:#?}");
    let unknown_but = |cause: &str, device: &str, persona: &str| {
        RUN_ATTRIBUTES
            .iter()
            .map(|key| {
                let value = match *key {
                    "cause" => cause,
                    "device" => device,
                    "persona" => persona,
                    _ => UNKNOWN,
                };
                (*key, value.to_owned())
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(said(&started[0]), unknown_but("start", DEVICE, UNKNOWN));
    assert_eq!(
        said(&started[2]),
        unknown_but("child", DEVICE, "steward"),
        "a child run is its parent's persona, in a conversation nobody named"
    );
    assert_eq!(started[2].parent_run.as_deref(), Some(RUN));
}

use std::sync::atomic::{AtomicU64, Ordering};

/// A consumer whose place the test moves: the last `seq` it took.
fn consumer(name: &'static str, taken: u64) -> (Consumer, std::sync::Arc<AtomicU64>) {
    let place = std::sync::Arc::new(AtomicU64::new(taken));
    let read = std::sync::Arc::clone(&place);
    (
        Consumer::new(name, move || Ok(read.load(Ordering::SeqCst))),
        place,
    )
}

fn on_disk(dir: &Path) -> Vec<u64> {
    read(dir).unwrap().iter().map(|e| e.seq).collect()
}

#[test]
fn a_consumer_that_stopped_for_longer_than_the_retention_loses_no_event_it_had_not_taken() {
    // #940: the audit's writer took the log through 2 and then stopped for 40 days.
    let dir = tempfile::tempdir().unwrap();
    sealed_segment(dir.path(), 1..=2, 45);
    sealed_segment(dir.path(), 3..=4, 40);
    sealed_segment(dir.path(), 5..=6, 35);
    sealed_segment(dir.path(), 7..=8, 1);
    let (audit, place) = consumer("audit", 2);

    let mut log = Log::open_held(dir.path(), DEVICE, Retention::DEFAULT, vec![audit]).unwrap();
    append_n(&mut log, 1);

    assert_eq!(
        on_disk(dir.path()),
        vec![3, 4, 5, 6, 7, 8, 9],
        "what it took and is old enough is deleted; what it did not take is kept"
    );
    // It starts again from its place and gets every event after it: no gap, said or silent.
    let got = subscribe(dir.path(), place.load(Ordering::SeqCst))
        .poll()
        .unwrap();
    assert_eq!(seqs(&got), vec![3, 4, 5, 6, 7, 8, 9]);
    assert!(!got.iter().any(|d| matches!(d, Delivery::Missed { .. })));

    // Once it has taken them, the retention deletes them as it would have: at the next seal,
    // or when the log is next opened.
    drop(log);
    let (audit, _place) = consumer("audit", 9);
    let _log = Log::open_held(dir.path(), DEVICE, Retention::DEFAULT, vec![audit]).unwrap();
    assert_eq!(on_disk(dir.path()), vec![7, 8, 9]);
}

#[test]
fn a_segment_is_kept_until_the_slowest_consumer_is_past_its_last_event() {
    let dir = tempfile::tempdir().unwrap();
    sealed_segment(dir.path(), 1..=3, 40);
    sealed_segment(dir.path(), 4..=6, 40);
    sealed_segment(dir.path(), 7..=8, 1);
    let (audit, _) = consumer("audit", 6);
    // Partway into the first segment: its third event is not taken yet.
    let (telemetry, slowest) = consumer("telemetry", 2);

    let unread = prune(
        dir.path(),
        &Retention::DEFAULT,
        &[audit, telemetry],
        std::time::SystemTime::now(),
    )
    .unwrap();
    assert_eq!(on_disk(dir.path()), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(unread, Vec::new());

    slowest.store(3, Ordering::SeqCst);
    let (audit, _) = consumer("audit", 6);
    let (telemetry, _) = consumer("telemetry", 3);
    prune(
        dir.path(),
        &Retention::DEFAULT,
        &[audit, telemetry],
        std::time::SystemTime::now(),
    )
    .unwrap();
    assert_eq!(on_disk(dir.path()), vec![4, 5, 6, 7, 8]);
}

#[test]
fn past_the_hard_limit_a_consumer_holds_nothing_and_what_it_lost_is_said() {
    // A consumer that never comes back must not fill the disk: past `hold` its events go,
    // the log says which, and it is told what it missed when it reads again.
    let dir = tempfile::tempdir().unwrap();
    sealed_segment(dir.path(), 1..=2, 100);
    sealed_segment(dir.path(), 3..=4, 95);
    sealed_segment(dir.path(), 5..=6, 40);
    sealed_segment(dir.path(), 7..=8, 1);
    let (audit, _) = consumer("audit", 1);
    let (telemetry, _) = consumer("telemetry", 4);

    let unread = prune(
        dir.path(),
        &Retention::DEFAULT,
        &[audit, telemetry],
        std::time::SystemTime::now(),
    )
    .unwrap();

    assert_eq!(on_disk(dir.path()), vec![5, 6, 7, 8]);
    assert_eq!(
        unread,
        vec![
            Unread {
                consumer: "audit",
                after: 1,
                through: 2
            },
            Unread {
                consumer: "audit",
                after: 2,
                through: 4
            },
        ],
        "telemetry had taken both segments, so it lost nothing"
    );
    let got = subscribe(dir.path(), 1).poll().unwrap();
    assert_eq!(
        got.first(),
        Some(&Delivery::Missed {
            after: 1,
            resumes_at: 5
        })
    );
}

#[test]
fn a_consumer_whose_place_cannot_be_read_holds_the_log_as_one_that_took_nothing() {
    // Nothing is deleted on a guess: a store that cannot say what it took is not past anything.
    let dir = tempfile::tempdir().unwrap();
    sealed_segment(dir.path(), 1..=2, 40);
    sealed_segment(dir.path(), 3..=4, 1);
    let broken = Consumer::new("audit", || Err(io::Error::other("its store is locked")));

    let unread = prune(
        dir.path(),
        &Retention::DEFAULT,
        &[broken],
        std::time::SystemTime::now(),
    )
    .unwrap();

    assert_eq!(on_disk(dir.path()), vec![1, 2, 3, 4]);
    assert_eq!(unread, Vec::new());
}

#[test]
fn a_log_sealed_while_a_consumer_is_behind_keeps_what_it_has_not_taken() {
    // The same hold at a seal as at an open: the writer prunes at both.
    let dir = tempfile::tempdir().unwrap();
    let quick = Retention {
        segment_bytes: 300,
        keep: std::time::Duration::from_millis(1),
        hold: Retention::DEFAULT.hold,
    };
    let (audit, place) = consumer("audit", 0);
    let mut log = Log::open_held(dir.path(), DEVICE, quick, vec![audit]).unwrap();
    for _ in 0..6 {
        append_n(&mut log, 2);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(on_disk(dir.path()), (1..=12).collect::<Vec<_>>());

    place.store(12, Ordering::SeqCst);
    std::thread::sleep(std::time::Duration::from_millis(5));
    append_n(&mut log, 4);
    assert!(
        on_disk(dir.path()).first().is_some_and(|first| *first > 1),
        "taken, and older than the retention: deleted at the next seal"
    );
}

/// What `dir`'s log says, by kind and run, in order.
fn kinds_and_runs(dir: &Path) -> Vec<(String, Option<String>)> {
    read(dir)
        .unwrap()
        .into_iter()
        .map(|e| (e.kind, e.run))
        .collect()
}

#[test]
fn a_run_held_for_its_start_is_written_once_when_it_begins() {
    // A start the host may still take back (#2250): its run is said once it goes on.
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());
    let of = RunOf {
        chat: CHAT,
        run: RUN,
    };

    host.begin_held(plane, 4, of, Began::Start, &Attributes::default(), None);
    assert!(read(dir.path()).unwrap().is_empty(), "nothing yet");

    host.begun(plane, 4).unwrap();
    host.begun(plane, 4).unwrap();
    let events = read(dir.path()).unwrap();
    assert_eq!(events.len(), 1, "one run.started: {events:#?}");
    assert_eq!(events[0].kind, "run.started");
    assert_eq!(events[0].run.as_deref(), Some(RUN));
    assert_eq!(events[0].body["cause"], "start");
}

/// A run id of its own, for the start that takes a held one's place.
const AGAIN: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7J";

#[test]
fn a_held_run_taken_back_writes_nothing() {
    // #2252: the start in its place is begun under another number, as the host's retry is,
    // and the held run is still never said.
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());
    let held = RunOf {
        chat: CHAT,
        run: RUN,
    };

    host.begin_held(
        plane,
        4,
        held,
        Began::Start,
        &Attributes::default(),
        Some(back_on()),
    );
    host.taken_back(plane, 4);
    host.begin(
        plane,
        5,
        RunOf {
            chat: CHAT,
            run: AGAIN,
        },
        Began::Start,
        &Attributes::default(),
    )
    .unwrap();
    host.begun(plane, 4).unwrap();

    assert_eq!(
        kinds_and_runs(dir.path()),
        [("run.started".to_owned(), Some(AGAIN.to_owned()))],
        "only the run that went on"
    );
}

#[test]
fn a_late_line_of_a_start_taken_back_is_recorded_with_no_run() {
    // #2252: the program of a start put aside may still speak; its lines are kept, the way a
    // line of a chat the log does not know is, and never mint a run.
    use crate::hookwire::Decision;
    use crate::state::Event::SessionStart;
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());

    host.begin_held(
        plane,
        4,
        RunOf {
            chat: CHAT,
            run: RUN,
        },
        Began::Start,
        &Attributes::default(),
        None,
    );
    host.taken_back(plane, 4);
    host.report(plane, &report(4, SessionStart), Followed::Moved)
        .unwrap();
    host.tool(
        plane,
        &tool_call(4, "pretooluse", Decision::None),
        Instant::now(),
    )
    .unwrap();
    host.refused(plane, 4).unwrap();

    let events = read(dir.path()).unwrap();
    assert_eq!(
        events.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
        ["hook.sessionstart", "hook.pretooluse", COMMIT_REFUSED],
        "no run.started: {events:#?}"
    );
    for event in &events {
        assert_eq!((&event.chat, &event.run), (&None, &None), "{event:#?}");
        assert_eq!(event.body["chat_number"], 4);
    }
}

#[test]
fn a_start_taken_back_after_a_line_said_its_run_keeps_that_run() {
    // A line that came before the start was put aside already wrote the run's `run.started`:
    // the log is append-only, so what follows of it stays under that run.
    use crate::state::Event::{SessionStart, Stop};
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());

    host.begin_held(
        plane,
        4,
        RunOf {
            chat: CHAT,
            run: RUN,
        },
        Began::Start,
        &Attributes::default(),
        None,
    );
    host.report(plane, &report(4, SessionStart), Followed::No)
        .unwrap();
    host.taken_back(plane, 4);
    host.report(plane, &report(4, Stop), Followed::No).unwrap();

    assert_eq!(
        kinds_and_runs(dir.path()),
        [
            ("run.started".to_owned(), Some(RUN.to_owned())),
            ("hook.sessionstart".to_owned(), Some(RUN.to_owned())),
            ("hook.stop".to_owned(), Some(RUN.to_owned())),
        ]
    );
}

#[test]
fn a_line_of_a_held_run_comes_after_its_run_started() {
    // ADR 0066: a run's first event is its `run.started`, whatever reaches the log first.
    use crate::state::Event::SessionStart;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());

    host.begin_held(
        plane,
        4,
        RunOf {
            chat: CHAT,
            run: RUN,
        },
        Began::Start,
        &Attributes::default(),
        None,
    );
    host.report(plane, &report(4, SessionStart), Followed::No)
        .unwrap();
    host.begun(plane, 4).unwrap();

    assert_eq!(
        kinds_and_runs(dir.path()),
        [
            ("run.started".to_owned(), Some(RUN.to_owned())),
            ("hook.sessionstart".to_owned(), Some(RUN.to_owned())),
        ]
    );
}

/// The sandbox coming back on for a chat, as its start holds it (#2252).
fn back_on() -> HeldTrust {
    HeldTrust::of(
        &crate::sandbox::Change::On,
        Some(crate::harness::Harness::ClaudeCode),
        None,
    )
}

#[test]
fn a_held_runs_trust_event_comes_first_whatever_reaches_the_log_first() {
    // ADR 0067 §7, #2252: a hook line of the program before the host's start goes on still
    // finds the sandbox's trust event, then the run's `run.started`, ahead of it.
    use crate::state::Event::SessionStart;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());

    host.begin_held(
        plane,
        4,
        RunOf {
            chat: CHAT,
            run: RUN,
        },
        Began::Start,
        &Attributes::default(),
        Some(back_on()),
    );
    host.report(plane, &report(4, SessionStart), Followed::No)
        .unwrap();
    host.begun(plane, 4).unwrap();

    assert_eq!(
        kinds_and_runs(dir.path()),
        [
            ("trust.sandbox.on".to_owned(), Some(RUN.to_owned())),
            ("run.started".to_owned(), Some(RUN.to_owned())),
            ("hook.sessionstart".to_owned(), Some(RUN.to_owned())),
        ]
    );
    assert_eq!(read(dir.path()).unwrap()[0].body, back_on().body);
}

#[test]
fn an_audit_naming_a_start_taken_back_begins_no_run() {
    // #2252: a grant, a change applied or a dispatch grant from that number is of no run.
    use crate::sandbox::grant::{Audited, Level};
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());

    host.begin_held(
        plane,
        4,
        RunOf {
            chat: CHAT,
            run: RUN,
        },
        Began::Start,
        &Attributes::default(),
        Some(back_on()),
    );
    host.taken_back(plane, 4);
    let granted = host
        .sandbox_grant(
            plane,
            Some(4),
            &Audited {
                granted: true,
                what: "host",
                target: "api.example.com",
                level: Level::Chat,
            },
        )
        .unwrap();

    assert_eq!((granted.chat, granted.run), (None, None));
    assert_eq!(
        kinds_and_runs(dir.path()),
        [("trust.sandbox.grant".to_owned(), None)],
        "no run begun, and the held trust event dropped"
    );
}

// ---- skill runs (SK-3, #2766) -------------------------------------------------------------

/// A tool call of chat `chat` that starts `skill` (as the hook found it), with the harness's id
/// `call` for it.
fn starting(chat: u32, word: &str, call: &str, skill: &str) -> crate::hookwire::ToolCall {
    let mut starting = tool_call(chat, word, crate::hookwire::Decision::None);
    starting.tool = Some("Skill".to_owned());
    starting.call = Some(call.to_owned());
    starting.skill = Some(Box::new(crate::hookwire::SkillStarted {
        skill: skill.to_owned(),
        run: crate::reopen::mint(),
        started_by: purlis_skills::runs::StartedBy::SkillTool,
        workspace: None,
        level: None,
    }));
    starting
}

#[test]
fn a_skill_started_opens_a_run_that_lasts_to_the_end_of_the_turn_and_every_entry_inside_carries_it()
{
    use crate::hookwire::Decision;
    use crate::state::Event::{Stop, UserPromptSubmit};
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/p");
    let mut host = recorder(dir.path());

    host.report(plane, &report(3, UserPromptSubmit), Followed::No)
        .unwrap();
    let before = host
        .tool(
            plane,
            &tool_call(3, "pretooluse", Decision::None),
            Instant::now(),
        )
        .unwrap();
    let skill = starting(3, "posttooluse-skill", "toolu_skill", "deploy");
    let id = skill.skill.clone().unwrap().run;
    let started = host.tool(plane, &skill, Instant::now()).unwrap();
    let mut inside = tool_call(3, "pretooluse", Decision::Deny);
    inside.call = Some("toolu_02".to_owned());
    host.tool(plane, &inside, Instant::now()).unwrap();
    let mut asked = tool_call(3, "pretooluse", Decision::Ask);
    asked.call = Some("toolu_03".to_owned());
    host.tool(plane, &asked, Instant::now()).unwrap();
    asked.tool_hook = "posttooluse".to_owned();
    asked.decision = Decision::None;
    host.tool(plane, &asked, Instant::now()).unwrap();
    let stop = host.report(plane, &report(3, Stop), Followed::No).unwrap();

    let events = read(dir.path()).unwrap();
    let kinds: Vec<_> = events.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(
        kinds,
        vec![
            "run.started",
            "hook.userpromptsubmit",
            "hook.pretooluse",
            "skill.started",
            "hook.posttooluse-skill",
            "hook.pretooluse",
            "hook.pretooluse",
            "hook.posttooluse",
            "skill.ended",
            "hook.stop",
        ],
        "the run opens at the Skill call and ends with the turn: {events:#?}"
    );
    assert_eq!(
        before.body.get("skill"),
        None,
        "before it, nothing is inside a skill"
    );
    assert_eq!(stop.body.get("skill"), None, "the turn's end is outside it");
    for event in &events[3..9] {
        assert_eq!(event.body["skill"], "deploy", "{event:#?}");
        assert_eq!(event.body["skill_run"], id.as_str(), "{event:#?}");
        assert_eq!(
            event.run, started.run,
            "a skill run is a span of the chat's run"
        );
    }
    let opened = &events[3];
    assert_eq!(opened.body["started_by"], "skill-tool");
    assert_eq!(opened.body.get("parent_skill_run"), None);
    let ended = &events[8];
    assert_eq!(ended.body["ended"], "turn");
    assert_eq!(
        (
            ended.body["tool_calls"].clone(),
            ended.body["refusals"].clone(),
            ended.body["asks"].clone(),
            ended.body["tool_errors"].clone(),
        ),
        (
            serde_json::json!(3),
            serde_json::json!(1),
            serde_json::json!(1),
            serde_json::json!(0)
        ),
        "the Skill call, the refused one and the asked one, each once: {ended:#?}"
    );
    assert!(ended.body["ms"].is_u64());
    assert_eq!(
        ended.body.get("passed"),
        None,
        "a run shows signals, never a verdict"
    );
}

#[test]
fn an_audit_or_a_trust_event_inside_a_skill_run_carries_the_skill_too() {
    use crate::sandbox::grant::{Audited, Level};
    use crate::state::Event::UserPromptSubmit;
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/p");
    let mut host = recorder(dir.path());
    let prompt = host
        .report(plane, &report(3, UserPromptSubmit), Followed::No)
        .unwrap();
    host.tool(
        plane,
        &starting(3, "posttooluse-skill", "toolu_a", "deploy"),
        Instant::now(),
    )
    .unwrap();

    let granted = host
        .sandbox_grant(
            plane,
            Some(3),
            &Audited {
                granted: true,
                what: "host",
                target: "api.example.com",
                level: Level::Chat,
            },
        )
        .unwrap();
    let sandbox_on = host
        .apply(Entry::Trust {
            chat: prompt.chat.clone().unwrap(),
            run: prompt.run.clone().unwrap(),
            kind: "trust.sandbox.on".to_owned(),
            body: serde_json::json!({}),
        })
        .unwrap();

    assert_eq!(granted.body["skill"], "deploy", "{granted:#?}");
    assert_eq!(
        sandbox_on[0].body["skill"], "deploy",
        "the sandbox's own event too"
    );
}

#[test]
fn a_skill_started_inside_another_is_its_child_and_a_turn_opens_one_skill_once() {
    use crate::state::Event::{Stop, UserPromptSubmit};
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/p");
    let mut host = recorder(dir.path());

    host.report(plane, &report(3, UserPromptSubmit), Followed::No)
        .unwrap();
    let outer = starting(3, "posttooluse-skill", "toolu_a", "release");
    let outer_id = outer.skill.clone().unwrap().run;
    host.tool(plane, &outer, Instant::now()).unwrap();
    let inner = starting(3, "posttooluse-skill", "toolu_b", "changelog");
    let inner_id = inner.skill.clone().unwrap().run;
    host.tool(plane, &inner, Instant::now()).unwrap();
    let again = host
        .tool(
            plane,
            &starting(3, "posttooluse-skill", "toolu_c", "release"),
            Instant::now(),
        )
        .unwrap();
    host.report(plane, &report(3, Stop), Followed::No).unwrap();

    let events = read(dir.path()).unwrap();
    let opened: Vec<_> = events
        .iter()
        .filter(|e| e.kind == "skill.started")
        .collect();
    assert_eq!(
        opened.len(),
        2,
        "release read again opens nothing: {events:#?}"
    );
    assert_eq!(opened[1].body["skill"], "changelog");
    assert_eq!(opened[1].body["parent_skill_run"], outer_id.as_str());
    assert_eq!(
        again.body["skill"], "changelog",
        "an entry is inside the innermost run"
    );
    let ended: Vec<_> = events
        .iter()
        .filter(|e| e.kind == "skill.ended")
        .map(|e| e.body["skill_run"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ended, vec![inner_id, outer_id], "the child ends first");
}

/// A plane on disk, so the skill log is written.
fn a_plane() -> tempfile::TempDir {
    let plane = tempfile::tempdir().unwrap();
    std::fs::write(plane.path().join(crate::plane::MANIFEST), "schema = 2\n").unwrap();
    plane
}

/// A Codex chat's shell read of a skill's `SKILL.md`, as its block word reports it.
fn codex_read(chat: u32, call: &str, run: &str) -> crate::hookwire::ToolCall {
    let mut read = tool_call(chat, "posttooluse-blocked", crate::hookwire::Decision::None);
    read.call = Some(call.to_owned());
    read.skill = Some(Box::new(crate::hookwire::SkillStarted {
        skill: "deploy".to_owned(),
        run: run.to_owned(),
        started_by: purlis_skills::runs::StartedBy::Read,
        workspace: None,
        level: None,
    }));
    read
}

#[test]
fn two_reads_of_a_skill_in_one_codex_turn_are_one_run_with_one_start_row_ended_by_its_stop() {
    use crate::state::Event::{Stop, UserPromptSubmit};
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = a_plane();
    let mut host = recorder(dir.path());
    host.report(plane.path(), &report(3, UserPromptSubmit), Followed::No)
        .unwrap();
    let first = crate::reopen::mint();

    host.tool(
        plane.path(),
        &codex_read(3, "call_1", &first),
        Instant::now(),
    )
    .unwrap();
    host.tool(
        plane.path(),
        &codex_read(3, "call_2", &crate::reopen::mint()),
        Instant::now(),
    )
    .unwrap();
    host.report(plane.path(), &report(3, Stop), Followed::No)
        .unwrap();

    let runs = crate::skilluse::runs(plane.path());
    assert_eq!(runs.len(), 1, "one run, and no orphan start row: {runs:#?}");
    assert_eq!(runs[0].id.as_deref(), Some(first.as_str()));
    assert_eq!(
        runs[0].started_by,
        Some(purlis_skills::runs::StartedBy::Read)
    );
    let end = runs[0].end.as_ref().expect("its end, with the turn");
    assert_eq!(end.ended, purlis_skills::runs::Ended::Turn);
    assert_eq!(end.tool_calls, 2, "both reads are calls inside it");
}

#[test]
fn an_opencode_run_ends_with_the_turn_an_interruption_or_the_session() {
    // opencode's shim reports its `skill` tool as `Skill` on `posttooluse-skill`, its turn's
    // end (`session.idle`) as `stop`, a new message as `userpromptsubmit`, and its session's
    // end as `sessionend`.
    use crate::state::Event::{SessionEnd, Stop, UserPromptSubmit};
    use std::time::Instant;
    for (ending, said) in [
        (Stop, purlis_skills::runs::Ended::Turn),
        (UserPromptSubmit, purlis_skills::runs::Ended::Interrupted),
        (SessionEnd, purlis_skills::runs::Ended::Session),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let plane = a_plane();
        let mut host = recorder(dir.path());
        host.report(plane.path(), &report(3, UserPromptSubmit), Followed::No)
            .unwrap();
        host.tool(
            plane.path(),
            &starting(3, "posttooluse-skill", "call_skill", "deploy"),
            Instant::now(),
        )
        .unwrap();
        let next = host
            .report(plane.path(), &report(3, ending), Followed::No)
            .unwrap();

        let events = read(dir.path()).unwrap();
        let ended = events.iter().find(|e| e.kind == "skill.ended").unwrap();
        assert_eq!(ended.body["ended"], said.word(), "{ending:?}");
        assert!(ended.seq < next.seq, "{ending:?}");
        assert_eq!(next.body.get("skill"), None, "{ending:?}: outside it");
        let runs = crate::skilluse::runs(plane.path());
        assert_eq!(runs[0].end.as_ref().map(|end| end.ended), Some(said));
    }
}

#[test]
fn a_codex_run_ends_with_an_interruption_or_the_session_too() {
    use crate::state::Event::{SessionEnd, UserPromptSubmit};
    use std::time::Instant;
    for (ending, said) in [(UserPromptSubmit, "interrupted"), (SessionEnd, "session")] {
        let dir = tempfile::tempdir().unwrap();
        let plane = Path::new("/p");
        let mut host = recorder(dir.path());
        host.report(plane, &report(3, UserPromptSubmit), Followed::No)
            .unwrap();
        host.tool(
            plane,
            &codex_read(3, "call_1", &crate::reopen::mint()),
            Instant::now(),
        )
        .unwrap();
        host.report(plane, &report(3, ending), Followed::No)
            .unwrap();

        let events = read(dir.path()).unwrap();
        let ended = events.iter().find(|e| e.kind == "skill.ended").unwrap();
        assert_eq!(ended.body["ended"], said);
    }
}

#[test]
fn a_refused_skill_call_or_a_claim_that_does_not_check_opens_no_run() {
    use crate::state::Event::UserPromptSubmit;
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/p");
    let mut host = recorder(dir.path());

    host.report(plane, &report(3, UserPromptSubmit), Followed::No)
        .unwrap();
    let mut refused = starting(3, "pretooluse", "toolu_a", "release");
    refused.decision = crate::hookwire::Decision::Deny;
    let event = host.tool(plane, &refused, Instant::now()).unwrap();
    let mut not_a_ulid = starting(3, "posttooluse-skill", "toolu_b", "release");
    not_a_ulid.skill.as_mut().unwrap().run = "../../x".to_owned();
    host.tool(plane, &not_a_ulid, Instant::now()).unwrap();
    let mut unknown = starting(3, "posttooluse-skill", "toolu_c", "release");
    unknown.skill.as_mut().unwrap().started_by = purlis_skills::runs::StartedBy::Unknown;
    host.tool(plane, &unknown, Instant::now()).unwrap();

    assert_eq!(event.body.get("skill"), None);
    assert!(
        read(dir.path())
            .unwrap()
            .iter()
            .all(|e| e.kind != "skill.started")
    );
}

// ---- a skill loaded through purlis's MCP server (SK-13, #2776) ---------------------------

/// The line purlis's MCP server sends as chat `chat` loads `skill` with `skill_load`.
fn loading(chat: u32, skill: &str) -> crate::hookwire::ToolCall {
    let mut line = tool_call(
        chat,
        crate::hookwire::SKILL_LOAD,
        crate::hookwire::Decision::None,
    );
    line.tool = Some("skill_load".to_owned());
    line.call = None;
    line.skill = Some(Box::new(crate::hookwire::SkillStarted {
        skill: skill.to_owned(),
        run: crate::reopen::mint(),
        started_by: purlis_skills::runs::StartedBy::McpLoad,
        workspace: Some("alpha".to_owned()),
        level: Some(purlis_skills::runs::Level {
            origin: purlis_skills::Origin::Workspace,
            local: false,
        }),
    }));
    line
}

#[test]
fn a_load_through_mcp_opens_a_run_recorded_as_one_and_ends_with_the_turn() {
    use crate::state::Event::{Stop, UserPromptSubmit};
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = a_plane();
    let mut host = recorder(dir.path());
    host.report(plane.path(), &report(3, UserPromptSubmit), Followed::No)
        .unwrap();
    let load = loading(3, "deploy");
    let id = load.skill.clone().unwrap().run;
    let loaded = host.tool(plane.path(), &load, Instant::now()).unwrap();
    // A second load of the skill in charge opens nothing; a load of another inside it is its
    // child.
    host.tool(plane.path(), &loading(3, "deploy"), Instant::now())
        .unwrap();
    let inner = loading(3, "changelog");
    host.tool(plane.path(), &inner, Instant::now()).unwrap();
    host.report(plane.path(), &report(3, Stop), Followed::No)
        .unwrap();

    let events = read(dir.path()).unwrap();
    let kinds: Vec<_> = events.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(
        kinds,
        vec![
            "run.started",
            "hook.userpromptsubmit",
            "skill.started",
            "hook.skill-load",
            "hook.skill-load",
            "skill.started",
            "hook.skill-load",
            "skill.ended",
            "skill.ended",
            "hook.stop",
        ],
        "{events:#?}"
    );
    assert_eq!(events[2].body["started_by"], "mcp-load");
    assert_eq!(loaded.kind, "hook.skill-load");
    assert_eq!(
        loaded.body["skill_run"],
        id.as_str(),
        "the load is inside its run"
    );
    assert_eq!(loaded.body["tool"], "skill_load");
    assert_eq!(
        loaded.body.get("word"),
        None,
        "a word purlis knows: {loaded:#?}"
    );
    assert_eq!(events[5].body["parent_skill_run"], id.as_str());

    let runs = crate::skilluse::runs(plane.path());
    assert_eq!(runs.len(), 2, "{runs:#?}");
    let outer = runs
        .iter()
        .find(|run| run.id.as_deref() == Some(id.as_str()))
        .expect("the outer run");
    assert_eq!(
        outer.started_by,
        Some(purlis_skills::runs::StartedBy::McpLoad)
    );
    let end = outer.end.as_ref().expect("its end, with the turn");
    assert_eq!(end.ended, purlis_skills::runs::Ended::Turn);
    assert_eq!(
        end.tool_calls, 3,
        "its own load, the second and its child's: {end:#?}"
    );
}

// ---- a skill Use skill… typed (SK-12, #2775) ----------------------------------------------

/// Chat 3's salt, its hook's and the window's alike.
fn salt_of_3() -> crate::skillspan::PromptSalt {
    crate::skillspan::PromptSalt::of_key("01JB00000000000000000000C3").unwrap()
}

/// What Use skill… holds for chat 3 as it types `text` for `skill`.
fn typed_skill(
    skill: &str,
    text: &str,
    confirmed: crate::skillspan::Confirmed,
) -> crate::skillspan::Typed {
    crate::skillspan::Typed {
        skill: skill.to_owned(),
        run: ulid::Ulid::generate(),
        workspace: None,
        level: None,
        text: crate::skillspan::PromptMarks::digest(text, &salt_of_3()),
        confirmed,
    }
}

/// Chat 3 sends a prompt whose text is `text`.
fn sent(text: &str) -> crate::hookwire::Report {
    let mut sent = report(3, crate::state::Event::UserPromptSubmit);
    sent.detail.prompt_marks = crate::skillspan::PromptMarks::of(text, &salt_of_3());
    sent
}

#[test]
fn a_skill_use_skill_typed_opens_its_run_with_the_prompt_that_holds_it_and_a_codex_read_of_it_is_the_same_run()
 {
    use crate::state::Event::Stop;
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = a_plane();
    let mut host = recorder(dir.path());
    // The chat is at its prompt: its run is known, and its last turn ended.
    host.report(plane.path(), &report(3, Stop), Followed::No)
        .unwrap();
    let typed = typed_skill(
        "deploy",
        "$deploy env: prod",
        crate::skillspan::Confirmed::ByPrompt,
    );
    let id = typed.run.to_string();
    let written = host
        .apply(super::Entry::SkillTyped {
            plane: plane.path().to_path_buf(),
            number: 3,
            typed,
        })
        .unwrap();
    assert!(written.is_empty(), "nothing is written while it is unsent");

    host.report(plane.path(), &sent("$deploy env: prod"), Followed::No)
        .unwrap();
    // Codex reads the skill it was asked for: inside the same run, never a second one.
    host.tool(
        plane.path(),
        &codex_read(3, "call_1", &crate::reopen::mint()),
        Instant::now(),
    )
    .unwrap();
    host.report(plane.path(), &report(3, Stop), Followed::No)
        .unwrap();

    let events = read(dir.path()).unwrap();
    let kinds: Vec<_> = events.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(
        kinds,
        vec![
            "run.started",
            "hook.stop",
            "skill.started",
            "hook.userpromptsubmit",
            "hook.posttooluse-blocked",
            "skill.ended",
            "hook.stop",
        ],
        "{events:#?}"
    );
    assert_eq!(events[2].body["started_by"], "use-skill");
    assert_eq!(
        events[3].body["skill_run"],
        id.as_str(),
        "the prompt is inside it"
    );
    let runs = crate::skilluse::runs(plane.path());
    assert_eq!(runs.len(), 1, "{runs:#?}");
    assert_eq!(runs[0].id.as_deref(), Some(id.as_str()));
    assert_eq!(
        runs[0].started_by,
        Some(purlis_skills::runs::StartedBy::UseSkill)
    );
    assert_eq!(runs[0].end.as_ref().map(|end| end.tool_calls), Some(1));
}

#[test]
fn a_skill_typed_into_claude_code_opens_its_run_on_the_expansion_that_names_it_and_none_without() {
    use crate::state::Event::Stop;
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = a_plane();
    let mut host = recorder(dir.path());
    host.report(plane.path(), &report(3, Stop), Followed::No)
        .unwrap();
    let typed = typed_skill(
        "skills:deploy",
        "/skills:deploy",
        crate::skillspan::Confirmed::ByName,
    );
    let id = typed.run.to_string();
    host.apply(super::Entry::SkillTyped {
        plane: plane.path().to_path_buf(),
        number: 3,
        typed,
    })
    .unwrap();
    host.report(plane.path(), &sent("/skills:deploy"), Followed::No)
        .unwrap();
    let mut expansion = tool_call(3, "userpromptexpansion", crate::hookwire::Decision::None);
    expansion.tool = None;
    expansion.call = None;
    expansion.skill = Some(Box::new(crate::hookwire::SkillStarted {
        skill: "skills:deploy".to_owned(),
        run: crate::reopen::mint(),
        started_by: purlis_skills::runs::StartedBy::UseSkill,
        workspace: None,
        level: None,
    }));
    host.tool(plane.path(), &expansion, Instant::now()).unwrap();
    // A second expansion of a skill nobody typed opens nothing.
    let mut other = expansion.clone();
    other.skill.as_mut().unwrap().skill = "skills:other".to_owned();
    host.tool(plane.path(), &other, Instant::now()).unwrap();
    host.report(plane.path(), &report(3, Stop), Followed::No)
        .unwrap();

    let runs = crate::skilluse::runs(plane.path());
    assert_eq!(runs.len(), 1, "{runs:#?}");
    assert_eq!(runs[0].id.as_deref(), Some(id.as_str()));
    assert_eq!(runs[0].skill, "skills:deploy");
    assert_eq!(
        runs[0].end.as_ref().map(|end| end.tool_calls),
        Some(0),
        "an expansion is no tool call"
    );
}

#[test]
fn a_curation_prompt_held_before_its_chat_begins_makes_the_skill_its_turn_starts_the_actions() {
    use crate::state::Event::Stop;
    use std::time::Instant;
    const CURATION: &str = "Use the purlis-compact skill to compact and improve the persona ops.";
    let dir = tempfile::tempdir().unwrap();
    let plane = a_plane();
    let mut host = recorder(dir.path());
    let written = host
        .apply(super::Entry::SkillPrompted {
            plane: plane.path().to_path_buf(),
            number: 3,
            prompted: crate::skillspan::Prompted::of(
                purlis_skills::runs::StartedBy::Curation,
                CURATION,
                &salt_of_3(),
            ),
        })
        .unwrap();
    assert!(written.is_empty(), "nothing is written while it is unsent");
    // As the window opens a curation chat: the prompt is held before its run begins.
    host.begin(
        plane.path(),
        3,
        RunOf {
            chat: CHAT,
            run: "01K6E8ZK6V4Q9T0N3M2B1C5D70",
        },
        Began::Start,
        &Attributes {
            persona: Some("steward".to_owned()),
            harness: Some("codex".to_owned()),
            ..Attributes::default()
        },
    )
    .unwrap();
    host.report(plane.path(), &sent(CURATION), Followed::No)
        .unwrap();
    host.tool(
        plane.path(),
        &starting(3, "posttooluse-skill", "toolu_a", "compact"),
        Instant::now(),
    )
    .unwrap();
    host.report(plane.path(), &report(3, Stop), Followed::No)
        .unwrap();

    let events = read(dir.path()).unwrap();
    let started = events
        .iter()
        .find(|event| event.kind == "skill.started")
        .expect("a run started");
    assert_eq!(started.body["started_by"], "curation", "{started:#?}");
    let runs = crate::skilluse::runs(plane.path());
    assert_eq!(runs.len(), 1, "{runs:#?}");
    assert_eq!(
        runs[0].started_by,
        Some(purlis_skills::runs::StartedBy::Curation)
    );
    assert_eq!(runs[0].harness.as_deref(), Some("codex"));
    assert_eq!(runs[0].persona.as_deref(), Some("steward"));
}

#[test]
fn a_chats_routing_record_is_under_its_run_and_every_skill_run_it_opens_names_it() {
    use crate::state::Event::{Stop, UserPromptSubmit};
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = a_plane();
    let mut host = recorder(dir.path());
    let record = crate::skills::router::Record {
        routing: "01K6E8ZK6V4Q9T0N3M2B1C5D7R".to_owned(),
        router: "relevance".to_owned(),
        ms: 101,
        fell_back: Some(crate::skills::router::FellBack::Slow),
        error: Some("'relevance' did not answer within 100 ms".to_owned()),
        hidden: vec![crate::skills::router::RecordHidden {
            skill: "deploy".to_owned(),
            why: "over-budget".to_owned(),
        }],
        prompt: false,
        pointed: Vec::new(),
    };
    host.begin(
        plane.path(),
        3,
        RunOf {
            chat: CHAT,
            run: "01K6E8ZK6V4Q9T0N3M2B1C5D70",
        },
        Began::Start,
        &Attributes::default(),
    )
    .unwrap();
    let routed = host
        .apply(super::Entry::SkillRouted {
            plane: plane.path().to_path_buf(),
            number: 3,
            record: record.clone(),
        })
        .unwrap();
    assert_eq!(routed.len(), 1);
    host.report(plane.path(), &report(3, UserPromptSubmit), Followed::No)
        .unwrap();
    host.tool(
        plane.path(),
        &starting(3, "posttooluse-skill", "toolu_a", "deploy"),
        Instant::now(),
    )
    .unwrap();
    host.report(plane.path(), &report(3, Stop), Followed::No)
        .unwrap();

    let events = read(dir.path()).unwrap();
    let said = events
        .iter()
        .find(|event| event.kind == "skill.routed")
        .expect("a routing record");
    assert_eq!(said.run.as_deref(), Some("01K6E8ZK6V4Q9T0N3M2B1C5D70"));
    assert_eq!(said.body, serde_json::to_value(&record).unwrap());
    assert_eq!(said.body["fell_back"], "slow");
    let started = events
        .iter()
        .find(|event| event.kind == "skill.started")
        .expect("a run started");
    assert_eq!(started.body["routing"], record.routing.as_str());
    assert_eq!(started.body["routing_fell_back"], true);
}

#[test]
fn a_routing_record_told_while_its_run_is_held_is_written_after_it_and_never_for_one_taken_back() {
    let dir = tempfile::tempdir().unwrap();
    let plane = a_plane();
    let mut host = recorder(dir.path());
    let record = crate::skills::router::Record {
        routing: "01K6E8ZK6V4Q9T0N3M2B1C5D7R".to_owned(),
        router: "relevance".to_owned(),
        ms: 12,
        fell_back: None,
        error: None,
        hidden: Vec::new(),
        prompt: false,
        pointed: Vec::new(),
    };
    for (number, run) in [
        (3, "01K6E8ZK6V4Q9T0N3M2B1C5D70"),
        (4, "01K6E8ZK6V4Q9T0N3M2B1C5D71"),
    ] {
        host.begin_held(
            plane.path(),
            number,
            RunOf { chat: CHAT, run },
            Began::Start,
            &Attributes::default(),
            None,
        );
        let now = host.skill_routed(plane.path(), number, &record).unwrap();
        assert_eq!(now, None, "nothing is written while the run is held");
    }
    host.begun(plane.path(), 3).unwrap();
    host.taken_back(plane.path(), 4);

    let kinds: Vec<(String, Option<String>)> = read(dir.path())
        .unwrap()
        .into_iter()
        .map(|event| (event.kind, event.run))
        .collect();
    let run = Some("01K6E8ZK6V4Q9T0N3M2B1C5D70".to_owned());
    assert_eq!(
        kinds,
        [
            ("run.started".to_owned(), run.clone()),
            ("skill.routed".to_owned(), run),
        ]
    );
}

#[test]
fn a_clear_keeps_the_routing_its_chat_started_under_for_its_skill_runs() {
    use crate::state::Event::{SessionStart, UserPromptSubmit};
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = a_plane();
    let mut host = recorder(dir.path());
    host.begin(
        plane.path(),
        3,
        RunOf {
            chat: CHAT,
            run: RUN,
        },
        Began::Start,
        &Attributes::default(),
    )
    .unwrap();
    let record = crate::skills::router::Record {
        routing: "01K6E8ZK6V4Q9T0N3M2B1C5D7R".to_owned(),
        router: "relevance".to_owned(),
        ms: 12,
        fell_back: None,
        error: None,
        hidden: Vec::new(),
        prompt: false,
        pointed: Vec::new(),
    };
    host.skill_routed(plane.path(), 3, &record).unwrap();
    let cleared = "01K6E8ZK6V4Q9T0N3M2B1C5D7K";
    host.report_with(
        plane.path(),
        &report(3, SessionStart),
        Followed::Moved,
        Some(cleared),
    )
    .unwrap();
    host.report(plane.path(), &report(3, UserPromptSubmit), Followed::No)
        .unwrap();
    host.tool(
        plane.path(),
        &starting(3, "posttooluse-skill", "toolu_a", "deploy"),
        Instant::now(),
    )
    .unwrap();

    let started = read(dir.path())
        .unwrap()
        .into_iter()
        .find(|event| event.kind == "skill.started")
        .expect("a run started");
    assert_eq!(started.run.as_deref(), Some(cleared));
    assert_eq!(started.body["routing"], record.routing.as_str());
    assert!(started.body.get("routing_fell_back").is_none());
}

#[test]
fn a_routing_told_for_a_start_taken_back_is_never_written() {
    let dir = tempfile::tempdir().unwrap();
    let plane = a_plane();
    let mut host = recorder(dir.path());
    host.begin_held(
        plane.path(),
        3,
        RunOf {
            chat: CHAT,
            run: RUN,
        },
        Began::Start,
        &Attributes::default(),
        None,
    );
    host.taken_back(plane.path(), 3);
    let record = crate::skills::router::Record {
        routing: "01K6E8ZK6V4Q9T0N3M2B1C5D7R".to_owned(),
        router: "relevance".to_owned(),
        ms: 12,
        fell_back: None,
        error: None,
        hidden: Vec::new(),
        prompt: false,
        pointed: Vec::new(),
    };

    assert_eq!(host.skill_routed(plane.path(), 3, &record).unwrap(), None);
    assert!(read(dir.path()).unwrap().is_empty());
}

#[test]
fn a_prompt_s_routing_is_named_beside_the_start_s_and_never_in_its_place() {
    use crate::state::Event::UserPromptSubmit;
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = a_plane();
    let mut host = recorder(dir.path());
    host.begin(
        plane.path(),
        3,
        RunOf {
            chat: CHAT,
            run: RUN,
        },
        Began::Start,
        &Attributes::default(),
    )
    .unwrap();
    let start = crate::skills::router::Record {
        routing: "01K6E8ZK6V4Q9T0N3M2B1C5D7R".to_owned(),
        router: "relevance".to_owned(),
        ms: 12,
        fell_back: None,
        error: None,
        hidden: Vec::new(),
        prompt: false,
        pointed: Vec::new(),
    };
    let prompt = crate::skills::router::Record {
        routing: "01K6E8ZK6V4Q9T0N3M2B1C5D7S".to_owned(),
        fell_back: Some(crate::skills::router::FellBack::Slow),
        prompt: true,
        ..start.clone()
    };
    host.skill_routed(plane.path(), 3, &start).unwrap();
    host.skill_routed(plane.path(), 3, &prompt).unwrap();
    host.report(plane.path(), &report(3, UserPromptSubmit), Followed::No)
        .unwrap();
    host.tool(
        plane.path(),
        &starting(3, "posttooluse-skill", "toolu_a", "deploy"),
        Instant::now(),
    )
    .unwrap();

    let started = read(dir.path())
        .unwrap()
        .into_iter()
        .find(|event| event.kind == "skill.started")
        .expect("a run started");
    assert_eq!(started.body["routing"], start.routing.as_str());
    assert!(
        started.body.get("routing_fell_back").is_none(),
        "its start was taken"
    );
    assert_eq!(started.body["prompt_routing"], prompt.routing.as_str());
    assert_eq!(started.body["prompt_routing_fell_back"], true);

    // The next turn's prompt was routed by nothing: its runs name no earlier prompt's routing.
    host.report(plane.path(), &report(3, UserPromptSubmit), Followed::No)
        .unwrap();
    host.tool(
        plane.path(),
        &starting(3, "posttooluse-skill", "toolu_b", "deploy"),
        Instant::now(),
    )
    .unwrap();
    let next = read(dir.path())
        .unwrap()
        .into_iter()
        .rfind(|event| event.kind == "skill.started")
        .expect("a second run started");
    assert!(next.body.get("prompt_routing").is_none(), "{:?}", next.body);
    assert_eq!(next.body["routing"], start.routing.as_str());
}

#[test]
fn a_log_whose_newest_event_has_no_id_takes_no_id_a_window_names_for_a_run() {
    // Nothing the log held can be told apart by time from such a log (#3394), so every run a
    // clear begins is under an id the writer mints.
    let dir = tempfile::tempdir().unwrap();
    {
        let mut log = Log::open(dir.path(), DEVICE).unwrap();
        log.append(None, None, None, "a.kind", serde_json::json!({}))
            .unwrap();
    }
    let file = dir.path().join(FILE);
    let text = std::fs::read_to_string(&file).unwrap();
    let mut event: Event = serde_json::from_str(text.lines().last().unwrap()).unwrap();
    event.ulid = "not-an-id".to_owned();
    std::fs::write(
        &file,
        format!("{}\n", serde_json::to_string(&event).unwrap()),
    )
    .unwrap();

    let opened = recorder(dir.path());

    assert!(!opened.never_held(&crate::reopen::mint()));
}

#[test]
fn an_empty_log_takes_a_fresh_id_a_window_names_for_a_run() {
    let dir = tempfile::tempdir().unwrap();

    assert!(recorder(dir.path()).never_held(&crate::reopen::mint()));
}

#[test]
fn a_run_let_go_of_from_what_the_writer_remembers_is_still_not_taken_again() {
    // The writer remembers so many runs it began (#3394); one it let go of is still held by
    // its time.
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    let first = crate::reopen::mint();
    let attributes = Attributes::default();
    for (number, run) in std::iter::once(first.clone())
        .chain((0..RUNS_HELD).map(|_| crate::reopen::mint()))
        .enumerate()
    {
        host.begin(
            Path::new("/p"),
            u32::try_from(number).unwrap(),
            RunOf {
                chat: &crate::reopen::mint(),
                run: &run,
            },
            Began::Start,
            &attributes,
        )
        .unwrap();
    }
    host.chats.clear();

    assert!(!host.never_held(&first));
}

#[cfg(unix)]
#[test]
fn a_runners_desktop_stop_and_re_arm_are_journalled_by_the_link_and_name_its_device() {
    // ADR 0071 and ADR 0075 as amended (#2051): a link's stop is unattributed, its re-arm a
    // person's, each `by: link`, and the event names the desktop's device id.
    use purlis_host::StopsElsewhere;
    let (config, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let on_disk = crate::halt::OnDisk(config.path().to_path_buf());
    let mut host = Recorder::open_in(config.path(), data.path()).unwrap();
    let dir = host.dir().to_path_buf();
    let desktop = "01JA0DESKT0P00000000000000";

    for rearm in [false, true] {
        let entry: Entry =
            serde_json::from_value(on_disk.link_switched(rearm, desktop).unwrap()).unwrap();
        host.apply(entry).unwrap();
    }

    let events = kill_switch_events(&dir);
    assert_eq!(
        events
            .iter()
            .map(|(kind, actor, by, _)| (kind.as_str(), actor.as_str(), by.as_str()))
            .collect::<Vec<_>>(),
        [
            ("killswitch.stop", "unattributed", "link"),
            ("killswitch.rearm", "human", "link")
        ]
    );
    assert!(
        read(&dir)
            .unwrap()
            .iter()
            .filter(|event| event.kind.starts_with("killswitch."))
            .all(|event| event.body["device"] == desktop)
    );
    assert!(!crate::halt::stopped_on_disk(config.path()), "re-armed");
    let journal = crate::halt::journal(config.path());
    assert!(
        journal
            .iter()
            .all(|line| line.by == crate::halt::Actor::Link)
    );
}

/// #3084: a run's `skill.ended` names its own run, never the one a prompt opened as it ended it:
/// a desktop logs a runner chat's runs from these lines by that id. Seen red: the interrupted
/// run's end named the Use skill… run its prompt opened.
#[test]
fn a_runs_end_names_its_own_run_though_the_prompt_that_ended_it_opened_another() {
    use crate::state::Event::UserPromptSubmit;
    use std::time::Instant;
    let dir = tempfile::tempdir().unwrap();
    let plane = a_plane();
    let mut host = recorder(dir.path());
    host.report(plane.path(), &report(3, UserPromptSubmit), Followed::No)
        .unwrap();
    let load = loading(3, "deploy");
    let interrupted = load.skill.clone().unwrap().run;
    host.tool(plane.path(), &load, Instant::now()).unwrap();
    let typed = typed_skill(
        "review",
        "$review the plan",
        crate::skillspan::Confirmed::ByPrompt,
    );
    let opened = typed.run.to_string();
    host.apply(super::Entry::SkillTyped {
        plane: plane.path().to_path_buf(),
        number: 3,
        typed,
    })
    .unwrap();
    host.report(plane.path(), &sent("$review the plan"), Followed::No)
        .unwrap();

    let events = read(dir.path()).unwrap();
    let ended = events
        .iter()
        .find(|e| e.kind == "skill.ended")
        .expect("the interrupted run's end");
    assert_eq!(ended.body["skill_run"], interrupted.as_str(), "{events:#?}");
    assert_eq!(ended.body["skill"], "deploy");
    assert_eq!(ended.body["ended"], "interrupted");
    let started: Vec<_> = events
        .iter()
        .filter(|e| e.kind == "skill.started")
        .map(|e| e.body["skill_run"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(started, [interrupted.clone(), opened]);
}
