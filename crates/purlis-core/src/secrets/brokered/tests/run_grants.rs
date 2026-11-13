// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Run grant** (ADR 0112, #5239): a `secret exec` line in, a run or a notice out.

use super::PolicySays as Policy;
use super::*;
use crate::secrets::ask::Asks;
use crate::secrets::ask::run::{OpenRun, RunAnswer};

fn window() -> crate::harness::asks::Answerer {
    crate::harness::asks::Answerer::admitted(crate::harness::asks::Admitted::LocalUi).unwrap()
}

/// `wanted` run with `asks`, on a search path of the system's own folders, as the app's would be.
fn run_with(asker: &Asker, wanted: Wanted, asks: &Asks, within: Duration) -> Vec<Frame> {
    run_searching(
        asker,
        wanted,
        asks,
        within,
        vec!["/usr/bin".into(), "/bin".into()],
    )
}

/// [`run_with`], with its program looked for in `search`.
fn run_searching(
    asker: &Asker,
    wanted: Wanted,
    asks: &Asks,
    within: Duration,
    search: Vec<PathBuf>,
) -> Vec<Frame> {
    crate::sandbox::tools::SEARCH_HERE.with(|dirs| *dirs.borrow_mut() = Some(search));
    let (reader, _keep) = held_open();
    let wire = Wire::default();
    serve_asking(
        asker,
        wanted,
        reader,
        Box::new(wire.clone()),
        Wrap::Unwrapped,
        Missing {
            asks,
            within,
            moved: &|| {},
            run_gate: None,
        },
    );
    let bytes = wire.0.lock().unwrap().clone();
    String::from_utf8(bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).expect("every line is a frame"))
        .collect()
}

/// `printenv X`: ends 0 where it was handed X, and prints the value, which is masked.
fn printenv(root: &Path, args: &[&str]) -> Wanted {
    let command = std::iter::once("printenv".to_owned())
        .chain(args.iter().map(|arg| (*arg).to_owned()))
        .collect();
    let mut want = wanted(root, "team", command);
    want.env = vec!["X=TOKEN".into()];
    want
}

/// `curl`, the one program reviewed for a grant of any arguments so far.
fn curl(root: &Path, args: &[&str]) -> Wanted {
    let mut want = printenv(root, args);
    want.command[0] = "curl".into();
    want
}

fn the_chat(root: &Path) -> Asker {
    asker(root, Some("devops"))
}

/// The one ask open, waiting for it to be; a run that ended without asking fails the test here,
/// rather than leaving it to wait for ever.
fn open_ask(asks: &Asks, ended: impl Fn() -> bool) -> OpenRun {
    loop {
        if let Some(one) = asks.open_runs().into_iter().next() {
            return one;
        }
        assert!(!ended(), "the run ended without asking");
        std::thread::yield_now();
    }
}

/// `want` run by a chat, answered by the person with `answer` as soon as it asks.
fn answered(root: &Path, want: Wanted, asks: &Asks, answer: RunAnswer) -> Vec<Frame> {
    std::thread::scope(|scope| {
        let run = scope.spawn(|| run_with(&the_chat(root), want, asks, Duration::from_secs(60)));
        let open = open_ask(asks, || run.is_finished());
        asks.answer_run(open.id, answer, window()).unwrap();
        run.join().unwrap()
    })
}

/// [`answered`], with the answer made once the ask is open, as the window makes it.
fn answered_with(
    root: &Path,
    want: Wanted,
    asks: &Asks,
    answer: impl FnOnce(&OpenRun) -> RunAnswer,
) -> Vec<Frame> {
    std::thread::scope(|scope| {
        let run = scope.spawn(|| run_with(&the_chat(root), want, asks, Duration::from_secs(60)));
        let open = open_ask(asks, || run.is_finished());
        let answer = answer(&open);
        asks.answer_run(open.id, answer, window()).unwrap();
        run.join().unwrap()
    })
}

fn refusal(frames: &[Frame]) -> (&str, i32) {
    match frames.last() {
        Some(Frame::Refused { why, code }) => (why, *code),
        other => panic!("a refusal: {other:?} in {frames:?}"),
    }
}

#[test]
fn a_held_key_and_an_undeclared_program_ask_and_allow_once_runs_that_command() {
    let project = project();
    let asks = Asks::new(project.path());
    let frames = answered(
        project.path(),
        printenv(project.path(), &["X"]),
        &asks,
        RunAnswer::Once,
    );
    assert_eq!(frames.last(), Some(&Frame::Exit(0)), "{frames:?}");
    assert!(
        frames
            .iter()
            .any(|frame| matches!(frame, Frame::Note(note) if note.contains("printenv"))),
        "says what it waits on: {frames:?}"
    );
    assert!(!format!("{frames:?}").contains(TOKEN), "{frames:?}");
    assert!(asks.open_runs().is_empty(), "answered");
}

#[test]
fn allow_once_goes_with_its_run_and_a_second_run_asks_again() {
    let project = project();
    let asks = Asks::new(project.path());
    let want = || printenv(project.path(), &["X"]);
    answered(project.path(), want(), &asks, RunAnswer::Once);
    let again = run_with(&the_chat(project.path()), want(), &asks, Duration::ZERO);
    assert_eq!(refusal(&again).1, WAITING_ON_THE_PERSON, "{again:?}");
    assert_eq!(asks.open_runs().len(), 1, "asked again");
}

#[test]
fn a_changed_argument_asks_again() {
    let project = project();
    let asks = Asks::new(project.path());
    let chat = the_chat(project.path());
    let first = run_with(
        &chat,
        printenv(project.path(), &["X"]),
        &asks,
        Duration::ZERO,
    );
    assert_eq!(refusal(&first).1, WAITING_ON_THE_PERSON);
    let open = asks.open_runs();
    assert_eq!(open.len(), 1);
    asks.answer_run(open[0].id, RunAnswer::Once, window())
        .unwrap();
    let changed = run_with(
        &chat,
        printenv(project.path(), &["Y"]),
        &asks,
        Duration::ZERO,
    );
    assert_eq!(refusal(&changed).1, WAITING_ON_THE_PERSON, "{changed:?}");
    assert_eq!(
        asks.open_runs()[0].bound.args,
        ["Y"],
        "a new ask for the changed command"
    );
    // The first Allow once is still good for the command it named.
    let first_again = run_with(
        &chat,
        printenv(project.path(), &["X"]),
        &asks,
        Duration::ZERO,
    );
    assert_eq!(first_again.last(), Some(&Frame::Exit(0)), "{first_again:?}");
}

#[test]
fn allow_once_given_after_the_wait_ran_out_runs_the_retry_and_nudges_the_chat() {
    let project = project();
    let asks = Asks::new(project.path());
    let chat = the_chat(project.path());
    let want = || printenv(project.path(), &["X"]);
    let stopped = run_with(&chat, want(), &asks, Duration::ZERO);
    let (said, code) = refusal(&stopped);
    assert_eq!(code, WAITING_ON_THE_PERSON);
    assert!(said.contains("printenv"), "{said}");
    let open = asks.open_runs();
    let answered = asks
        .answer_run(open[0].id, RunAnswer::Once, window())
        .unwrap();
    assert_eq!(answered.nudge, [7], "the chat is told");
    assert!(
        answered.told.line().contains("retry"),
        "{}",
        answered.told.line()
    );
    let retried = run_with(&chat, want(), &asks, Duration::ZERO);
    assert_eq!(retried.last(), Some(&Frame::Exit(0)), "{retried:?}");
}

#[test]
fn dont_allow_tells_the_chat_and_it_is_not_asked_again_about_that_program_and_vault() {
    let project = project();
    let asks = Asks::new(project.path());
    let frames = answered(
        project.path(),
        printenv(project.path(), &["X"]),
        &asks,
        RunAnswer::Refuse,
    );
    let (said, _) = refusal(&frames);
    assert!(said.contains("did not allow printenv"), "{said}");
    // Other arguments, the same program and vault: told at once, nothing asked.
    let again = run_with(
        &the_chat(project.path()),
        printenv(project.path(), &["Y"]),
        &asks,
        Duration::ZERO,
    );
    assert!(refusal(&again).0.contains("did not allow"), "{again:?}");
    assert!(asks.open_runs().is_empty());
    // Another chat is asked afresh.
    let mut other = the_chat(project.path());
    other.chat = 8;
    run_with(
        &other,
        printenv(project.path(), &["X"]),
        &asks,
        Duration::ZERO,
    );
    assert_eq!(asks.open_runs().len(), 1);
}

#[test]
fn a_program_that_runs_what_it_is_told_and_purlis_itself_stay_refused_with_no_notice() {
    let project = project();
    let asks = Asks::new(project.path());
    let chat = the_chat(project.path());
    for words in [
        sh("echo hi"),
        vec!["/usr/bin/env".into(), "printenv".into()],
        vec!["python3".into(), "-c".into(), "1".into()],
        vec!["purlis".into(), "secret".into()],
        vec!["xargs".into(), "printenv".into()],
    ] {
        let mut want = wanted(project.path(), "team", words.clone());
        want.env = vec!["X=TOKEN".into()];
        let frames = run_with(&chat, want, &asks, Duration::from_secs(60));
        let (said, code) = refusal(&frames);
        assert_eq!(code, 1, "{words:?}: {said}");
        assert!(asks.open_runs().is_empty(), "{words:?} asked nobody");
    }
}

#[test]
fn a_program_a_chat_could_have_placed_is_refused_with_no_notice() {
    let project = project();
    let asks = Asks::new(project.path());
    let chat = the_chat(project.path());
    let own = project.path().join("work/printenv");
    std::fs::copy("/usr/bin/printenv", &own).unwrap();
    for program in [
        own.display().to_string(),
        "./printenv".to_owned(),
        "work/printenv".to_owned(),
        "/tmp/printenv".to_owned(),
    ] {
        let mut want = wanted(project.path(), "team", vec![program.clone(), "X".into()]);
        want.env = vec!["X=TOKEN".into()];
        let frames = run_with(&chat, want, &asks, Duration::from_secs(60));
        assert_eq!(refusal(&frames).1, 1, "{program}: {frames:?}");
        assert!(asks.open_runs().is_empty(), "{program} asked nobody");
    }
}

#[test]
fn a_chats_path_never_changes_the_program_a_grant_binds() {
    let project = project();
    let asks = Asks::new(project.path());
    let own = project.path().join("work");
    std::fs::copy("/usr/bin/printenv", own.join("printenv")).unwrap();
    let mut want = printenv(project.path(), &["X"]);
    want.environment = vec![("PATH".into(), own.display().to_string().into())];
    run_with(&the_chat(project.path()), want, &asks, Duration::ZERO);
    let program = asks.open_runs()[0].bound.program.clone();
    assert!(program.is_absolute(), "{program:?}");
    assert!(!program.starts_with(project.path()), "{program:?}");
}

#[test]
fn a_program_named_by_its_path_or_its_name_is_one_grant() {
    let project = project();
    let asks = Asks::new(project.path());
    let chat = the_chat(project.path());
    answered(
        project.path(),
        printenv(project.path(), &["X"]),
        &asks,
        RunAnswer::Refuse,
    );
    let found = crate::programs::find("printenv", &["/usr/bin".into(), "/bin".into()]).unwrap();
    let mut by_path = printenv(project.path(), &["X"]);
    by_path.command[0] = found.display().to_string();
    let frames = run_with(&chat, by_path, &asks, Duration::ZERO);
    assert!(refusal(&frames).0.contains("did not allow"), "{frames:?}");
}

#[test]
fn a_variable_that_steers_what_runs_is_refused_with_no_notice() {
    let project = project();
    let asks = Asks::new(project.path());
    for spec in [
        "LD_PRELOAD=TOKEN",
        "DYLD_INSERT_LIBRARIES=TOKEN",
        "PATH=TOKEN",
        "BASH_ENV=TOKEN",
    ] {
        let mut want = printenv(project.path(), &["X"]);
        want.env = vec![spec.into()];
        let frames = run_with(
            &the_chat(project.path()),
            want,
            &asks,
            Duration::from_secs(60),
        );
        assert_eq!(refusal(&frames).1, 1, "{spec}: {frames:?}");
        assert!(asks.open_runs().is_empty(), "{spec} asked nobody");
    }
}

#[test]
fn a_key_only_a_browser_fills_is_never_offered_to_a_program() {
    let project = project();
    std::fs::write(
        project.path().join("charter.toml"),
        r#"[sandbox]
open = "off"
[sandbox.personas.devops]
tools = { shop = { vault = "team", browser = "https://shop.example/login", key = "TOKEN" } }
"#,
    )
    .unwrap();
    let asks = Asks::new(project.path());
    let frames = run_with(
        &the_chat(project.path()),
        printenv(project.path(), &["X"]),
        &asks,
        Duration::from_secs(60),
    );
    assert_eq!(refusal(&frames).1, 1, "{frames:?}");
    assert!(asks.open_runs().is_empty());
}

#[test]
fn the_notice_holds_the_apps_facts_apart_from_the_chats_inert_command() {
    let project = project();
    let asks = Asks::new(project.path());
    let mut chat = the_chat(project.path());
    chat.confines.as_mut().unwrap().hosts = vec!["api.example.com:443".into()];
    let mut want = printenv(project.path(), &["X\nreal: \u{202e}moc.live\u{7}"]);
    want.file = vec!["F=KUBECONFIG".into()];
    run_with(&chat, want, &asks, Duration::ZERO);
    let open = asks.open_runs().remove(0);
    assert!(open.bound.program.is_absolute(), "{:?}", open.bound.program);
    assert_eq!(open.bound.vault, "team");
    assert_eq!(open.keys(), ["KUBECONFIG", "TOKEN"]);
    assert_eq!(
        open.hosts,
        crate::secrets::ask::run::Hosts::Listed(vec!["api.example.com:443".into()]),
    );
    assert_eq!(open.persona, "devops");
    // The chat's words, with every control character made visible.
    assert!(open.command.starts_with("printenv "), "{}", open.command);
    for hidden in ['\n', '\u{202e}', '\u{7}'] {
        assert!(!open.command.contains(hidden), "{:?}", open.command);
    }
    assert!(open.command.contains(r"\x0a"), "{}", open.command);
    assert!(open.command.contains(r"\xe2\x80\xae"), "{}", open.command);
}

#[test]
fn a_chat_at_the_floor_is_shown_as_reaching_every_host() {
    let project = project();
    let asks = Asks::new(project.path());
    let mut chat = the_chat(project.path());
    chat.confines.as_mut().unwrap().level = crate::sandbox::Level::Floor;
    run_with(
        &chat,
        printenv(project.path(), &["X"]),
        &asks,
        Duration::ZERO,
    );
    assert_eq!(
        asks.open_runs()[0].hosts,
        crate::secrets::ask::run::Hosts::Every
    );
}

#[test]
fn a_general_purpose_program_carries_t4s_caution() {
    let project = project();
    let asks = Asks::new(project.path());
    let mut want = wanted(project.path(), "team", vec!["cat".into()]);
    want.env = vec!["X=TOKEN".into()];
    run_with(&the_chat(project.path()), want, &asks, Duration::ZERO);
    let caution = asks.open_runs()[0].caution.clone().unwrap();
    assert!(caution.contains("cat can show the values"), "{caution}");
}

#[test]
fn a_command_longer_than_a_person_reads_or_holding_a_credential_is_not_offered() {
    let project = project();
    let asks = Asks::new(project.path());
    let long = "x".repeat(MOST_COMMAND_SHOWN + 1);
    let token = format!("ghp_{}", "abcdefghijklmnopqrstuvwxyz0123456789");
    for arg in [long.as_str(), token.as_str()] {
        let frames = run_with(
            &the_chat(project.path()),
            printenv(project.path(), &[arg]),
            &asks,
            Duration::from_secs(60),
        );
        assert_eq!(refusal(&frames).1, 1, "{frames:?}");
        assert!(asks.open_runs().is_empty());
    }
}

#[test]
fn a_chat_cannot_bury_the_person_in_notices() {
    let project = project();
    let asks = Asks::new(project.path());
    let chat = the_chat(project.path());
    for n in 0..10 {
        let arg = format!("X{n}");
        run_with(
            &chat,
            printenv(project.path(), &[&arg]),
            &asks,
            Duration::ZERO,
        );
    }
    assert_eq!(
        asks.open_runs().len(),
        crate::secrets::ask::run::MOST_OPEN_PER_CHAT
    );
}

#[test]
fn a_declared_tools_run_is_exactly_as_it_was_and_asks_nothing() {
    let project = project();
    let asks = Asks::new(project.path());
    let (chat, want) = as_tool(the_chat(project.path()), printenv(project.path(), &["X"]));
    let frames = run_with(&chat, want, &asks, Duration::from_secs(60));
    assert_eq!(frames.last(), Some(&Frame::Exit(0)), "{frames:?}");
    assert!(asks.open_runs().is_empty());
    // Asked with another hand-over, it is refused as it always was, and nobody is asked.
    let (chat, mut want) = as_tool(the_chat(project.path()), printenv(project.path(), &["X"]));
    want.env = vec!["Y=KUBECONFIG".into()];
    let frames = run_with(&chat, want, &asks, Duration::from_secs(60));
    assert!(
        refusal(&frames)
            .0
            .contains("a chat runs it with exactly that"),
        "{frames:?}"
    );
    assert!(asks.open_runs().is_empty());
}

#[test]
fn a_tool_a_teammate_declared_runs_nothing_here_and_asks_nothing_until_this_machines_allow() {
    let project = project();
    std::fs::write(
        project.path().join("charter.toml"),
        r#"[sandbox]
open = "off"
[sandbox.personas.devops]
tools = { printenv = { vault = "team", as = "X env", key = "TOKEN" } }
"#,
    )
    .unwrap();
    let asks = Asks::new(project.path());
    let frames = run_with(
        &the_chat(project.path()),
        printenv(project.path(), &["X"]),
        &asks,
        Duration::from_secs(60),
    );
    assert_eq!(refusal(&frames).1, 1, "{frames:?}");
    assert!(
        asks.open_runs().is_empty(),
        "no Run grant around a teammate's line"
    );
}

#[test]
fn a_value_is_in_nothing_the_chat_hears_or_the_notice_holds() {
    let project = project();
    let asks = Asks::new(project.path());
    let frames = answered(
        project.path(),
        printenv(project.path(), &["X"]),
        &asks,
        RunAnswer::Refuse,
    );
    assert!(!format!("{frames:?}").contains(TOKEN), "{frames:?}");
    run_with(
        &the_chat(project.path()),
        printenv(project.path(), &["Y"]),
        &asks,
        Duration::ZERO,
    );
    let mut other = the_chat(project.path());
    other.chat = 9;
    run_with(
        &other,
        printenv(project.path(), &["X"]),
        &asks,
        Duration::ZERO,
    );
    assert!(!format!("{:?}", asks.open_runs()).contains(TOKEN));
}

#[test]
fn where_nothing_asks_an_undeclared_command_is_refused_as_before() {
    let project = project();
    let mut want = printenv(project.path(), &["X"]);
    want.env = vec!["X=TOKEN".into()];
    let frames = served_as_asked(&the_chat(project.path()), want);
    assert!(refusal(&frames).0.contains("declared tools"), "{frames:?}");
}

#[test]
fn a_servers_start_never_raises_a_notice() {
    let project = project();
    let asks = Asks::new(project.path());
    let mut want = printenv(project.path(), &["X"]);
    want.mcp = Some(crate::secrets::mcpstart::McpStart {
        server: "s".into(),
        last: true,
    });
    let frames = run_with(
        &the_chat(project.path()),
        want,
        &asks,
        Duration::from_secs(60),
    );
    assert_eq!(refusal(&frames).1, 1, "{frames:?}");
    assert!(asks.open_runs().is_empty());
}

#[test]
fn a_manifest_nobody_can_read_asks_nothing_for_it_may_declare_the_program() {
    let project = project();
    std::fs::write(project.path().join("charter.toml"), "[sandbox\nnot toml").unwrap();
    let asks = Asks::new(project.path());
    let frames = run_with(
        &the_chat(project.path()),
        printenv(project.path(), &["X"]),
        &asks,
        Duration::from_secs(60),
    );
    assert_eq!(refusal(&frames).1, 1, "{frames:?}");
    assert!(asks.open_runs().is_empty());
}

#[test]
fn allow_for_this_chat_runs_the_program_with_other_arguments_and_no_more_asks() {
    let project = project();
    let asks = Asks::new(project.path());
    answered(
        project.path(),
        curl(project.path(), &["--version"]),
        &asks,
        RunAnswer::ForChat,
    );
    let other = run_with(
        &the_chat(project.path()),
        curl(project.path(), &["--help"]),
        &asks,
        Duration::ZERO,
    );
    assert_ne!(
        other.last().map(
            |frame| matches!(frame, Frame::Refused { code, .. } if *code == WAITING_ON_THE_PERSON)
        ),
        Some(true),
        "{other:?}"
    );
    assert!(asks.open_runs().is_empty(), "nothing more was asked");
}

// ----------------------------------------------------------------------------------------
// Under policy (#5243)

#[test]
fn locked_a_command_raises_no_allow_and_the_chat_hears_why_and_to_declare_it() {
    for said in [
        r#"{"sandbox": {"run-grants": false}}"#,
        r#"{"sandbox": {"persona-tools": false}}"#,
    ] {
        let _policy = Policy::says(said);
        let project = project();
        let asks = Asks::new(project.path());
        let frames = run_with(
            &the_chat(project.path()),
            printenv(project.path(), &["X"]),
            &asks,
            Duration::from_secs(60),
        );
        let (why, code) = refusal(&frames);
        assert_eq!(code, 1, "{frames:?}");
        assert!(why.contains("policy"), "{why}");
        assert!(asks.open_runs().is_empty(), "no Allow under {said}");
    }
}

#[test]
fn the_lock_set_later_stops_a_grant_given_before_it() {
    let project = project();
    let asks = Asks::new(project.path());
    answered(
        project.path(),
        curl(project.path(), &["--version"]),
        &asks,
        RunAnswer::ForChat,
    );
    let _policy = Policy::says(r#"{"sandbox": {"run-grants": false}}"#);
    let frames = run_with(
        &the_chat(project.path()),
        curl(project.path(), &["--version"]),
        &asks,
        Duration::ZERO,
    );
    assert!(refusal(&frames).0.contains("policy"), "{frames:?}");
}

#[test]
fn locked_a_key_the_vault_lacks_is_still_asked_for_saying_why_no_allow_is_offered() {
    let _policy = Policy::says(r#"{"sandbox": {"run-grants": false}}"#);
    let project = project();
    let asks = Asks::new(project.path());
    let mut want = printenv(project.path(), &["X"]);
    want.env = vec!["X=NOT_THERE_YET".into()];
    let frames = run_with(&the_chat(project.path()), want, &asks, Duration::ZERO);
    assert_eq!(refusal(&frames).1, WAITING_ON_THE_PERSON, "{frames:?}");
    assert!(asks.open().is_empty(), "one notice (#5242)");
    let open = asks.open_runs();
    assert_eq!(open.len(), 1, "Fill");
    assert_eq!(open[0].fills[0].key, "NOT_THERE_YET");
    assert!(
        matches!(&open[0].offer, crate::secrets::ask::run::Offer::Fill { no_allow: Some(why) } if why.contains("policy")),
        "no Allow, saying why: {open:?}"
    );
}

#[test]
fn a_refusal_under_policy_is_audited_with_closed_words() {
    use crate::eventlog::secretasks::{By, Step};
    let _policy = Policy::says(r#"{"sandbox": {"run-grants": false}}"#);
    let project = project();
    let asks = Asks::new(project.path());
    let steps = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = std::sync::Arc::clone(&steps);
    asks.telling(std::sync::Arc::new(move |chat, step| {
        seen.lock().unwrap().push((chat, step));
    }));
    run_with(
        &the_chat(project.path()),
        printenv(project.path(), &["X"]),
        &asks,
        Duration::ZERO,
    );
    let steps = steps.lock().unwrap();
    assert!(
        steps.iter().any(|(_, step)| matches!(
            step,
            Step::Refused {
                by: By::RunGrantPolicy,
                ..
            }
        )),
        "{steps:?}"
    );
}

#[test]
fn under_the_lock_on_undeclared_asks_an_undeclared_key_is_offered_no_allow() {
    let _policy = Policy::says(r#"{"sandbox": {"undeclared-secret-asks": false}}"#);
    let project = project();
    let asks = Asks::new(project.path());
    let frames = run_with(
        &the_chat(project.path()),
        printenv(project.path(), &["X"]),
        &asks,
        Duration::from_secs(60),
    );
    assert_eq!(refusal(&frames).1, 1, "{frames:?}");
    assert!(asks.open_runs().is_empty());
}

#[test]
fn with_no_lock_set_run_grants_are_allowed() {
    let locks = crate::sandbox::policy::Locks::parse(
        r#"{"sandbox": {"hosts": []}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    assert_eq!(locks.refuses_run_grant("devops", "curl"), None);
}

// ----------------------------------------------------------------------------------------
// On a runner (#5243)

#[test]
fn a_runner_chat_gets_no_run_grant_and_hears_to_declare_it_as_a_tool() {
    let project = project();
    let asks = Asks::new(project.path());
    let chat = the_chat(project.path());
    let want = printenv(project.path(), &["X"]);
    // What its desktop says before any value is sent.
    let why = crate::secrets::brokered::armed_as(&chat, &want).expect_err("no tool");
    assert!(why.contains("declared tools"), "{why}");
    assert!(why.contains("tools = {"), "says how to declare it: {why}");
    assert!(asks.open_runs().is_empty(), "nobody was asked");
}

// ----------------------------------------------------------------------------------------
// Always allow (#5245)

#[test]
fn always_allow_runs_the_waiting_command_and_later_ones_with_no_restart_though_it_is_listed_now() {
    let project = project();
    std::fs::create_dir_all(project.path().join("personas/devops")).unwrap();
    std::fs::write(
        project.path().join("personas/devops/persona.md"),
        "---\nrole: Ops\n---\n",
    )
    .unwrap();
    std::fs::write(
        project.path().join("charter.toml"),
        "[sandbox]\nmode = \"on\"\n",
    )
    .unwrap();
    let asks = Asks::new(project.path());
    let want = || curl(project.path(), &["--version"]);
    let first = answered_with(project.path(), want(), &asks, |open: &OpenRun| {
        crate::settings::always_allow::write(project.path(), "devops", &open.bound).unwrap();
        RunAnswer::Always
    });
    assert!(
        !matches!(first.last(), Some(Frame::Refused { code, .. }) if *code == WAITING_ON_THE_PERSON),
        "{first:?}"
    );
    // The chat's start armed nothing, so the line in the file is what it is run as by its grant.
    let (declared, _) = crate::sandbox::tools::declared_and_allowed_here(project.path(), "devops");
    assert_eq!(declared[0].name, "curl");
    let again = run_with(
        &the_chat(project.path()),
        curl(project.path(), &["--help"]),
        &asks,
        Duration::ZERO,
    );
    assert!(
        !matches!(again.last(), Some(Frame::Refused { code, .. }) if *code == WAITING_ON_THE_PERSON),
        "{again:?}"
    );
    assert!(
        !format!("{again:?}").contains("declared tools"),
        "its grant carries it: {again:?}"
    );
    assert!(asks.open_runs().is_empty());
    // Another chat of the same persona has no grant: it runs on the machine's Allow, not this.
    let mut other = the_chat(project.path());
    other.chat = 99;
    let other = run_with(&other, want(), &asks, Duration::ZERO);
    assert_eq!(refusal(&other).1, 1, "{other:?}");
    assert!(
        asks.open_runs().is_empty(),
        "and nobody is asked: it is listed"
    );
}

// ----------------------------------------------------------------------------------------
// A vault the persona may not use (the operator's ruling on ADR 0112)

/// `printenv X` with vault `other`, which is tagged for nobody.
fn on_another_vault(root: &Path) -> Wanted {
    let mut want = curl(root, &["--version"]);
    want.vault = "other".into();
    want
}

/// `curl --version` with vault `other`, which is tagged for nobody.
fn curl_on_another_vault(root: &Path) -> Wanted {
    let mut want = curl(root, &["--version"]);
    want.vault = "other".into();
    want
}

/// Whether `frames` end with `other` refused for what its persona may use.
fn vault_was_refused(frames: &[Frame]) -> bool {
    format!("{frames:?}").contains("did not open it")
}

fn no_vault_was_kept(root: &Path) {
    assert!(
        crate::sandbox::local::granted_vaults(root).is_empty(),
        "nothing lasting was written"
    );
    let ctx = Ctx::new(root, Env::of(&[]));
    assert!(authorise(&ctx, Some("devops"), "other").is_err());
}

#[test]
fn allow_once_opens_a_vault_for_that_command_alone_and_writes_nothing_lasting() {
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let chat = the_chat(root);
    let frames = answered(root, on_another_vault(root), &asks, RunAnswer::Once);
    assert!(
        !vault_was_refused(&frames),
        "the gate admits it: {frames:?}"
    );
    no_vault_was_kept(root);
    // This chat's next differing command has no vault access: it asks.
    let mut differing = on_another_vault(root);
    differing.command.push("HOME".into());
    let next = run_with(&chat, differing, &asks, Duration::ZERO);
    assert_eq!(refusal(&next).1, WAITING_ON_THE_PERSON, "{next:?}");
    // Nor does its same command again: Allow once went with its run.
    let again = run_with(&chat, on_another_vault(root), &asks, Duration::ZERO);
    assert_eq!(refusal(&again).1, WAITING_ON_THE_PERSON, "{again:?}");
}

#[test]
fn another_chat_of_the_persona_has_no_vault_access_from_this_chats_grant() {
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    answered(root, curl_on_another_vault(root), &asks, RunAnswer::ForChat);
    let mut other = the_chat(root);
    other.chat = 99;
    let frames = run_with(&other, curl_on_another_vault(root), &asks, Duration::ZERO);
    assert_eq!(refusal(&frames).1, WAITING_ON_THE_PERSON, "{frames:?}");
    no_vault_was_kept(root);
}

#[test]
fn revoking_a_chat_grant_ends_its_use_of_the_vault() {
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let chat = the_chat(root);
    answered(root, curl_on_another_vault(root), &asks, RunAnswer::ForChat);
    let id = asks.given_runs()[0].id;
    asks.revoke_run(id, window()).unwrap();
    let frames = run_with(&chat, curl_on_another_vault(root), &asks, Duration::ZERO);
    assert_eq!(refusal(&frames).1, WAITING_ON_THE_PERSON, "{frames:?}");
    no_vault_was_kept(root);
}

#[test]
fn the_notice_says_the_vault_is_not_the_personas_and_the_persona_hears_otherwise_on_its_own() {
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    run_with(
        &the_chat(root),
        on_another_vault(root),
        &asks,
        Duration::ZERO,
    );
    assert!(asks.open_runs()[0].not_its_vault);
    let asks = Asks::new(root);
    run_with(
        &the_chat(root),
        printenv(root, &["X"]),
        &asks,
        Duration::ZERO,
    );
    assert!(!asks.open_runs()[0].not_its_vault);
}

#[test]
fn a_declared_tool_is_never_let_past_a_vault_its_persona_may_not_use() {
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let (chat, want) = as_tool(the_chat(root), on_another_vault(root));
    let frames = run_with(&chat, want, &asks, Duration::from_secs(60));
    assert!(vault_was_refused(&frames), "{frames:?}");
    assert!(asks.open_runs().is_empty());
}

#[test]
fn no_persona_an_unknown_vault_and_policys_vault_lock_open_nothing_and_ask_nothing() {
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let no_persona = asker(root, None);
    let frames = run_with(
        &no_persona,
        on_another_vault(root),
        &asks,
        Duration::from_secs(60),
    );
    assert_eq!(refusal(&frames).1, 1, "{frames:?}");
    let mut unknown = printenv(root, &["X"]);
    unknown.vault = "nowhere".into();
    let frames = run_with(&the_chat(root), unknown, &asks, Duration::from_secs(60));
    assert_eq!(refusal(&frames).1, 1, "{frames:?}");
    let _policy = Policy::says(r#"{"sandbox": {"vault-grants": false}}"#);
    let frames = run_with(
        &the_chat(root),
        on_another_vault(root),
        &asks,
        Duration::from_secs(60),
    );
    assert!(vault_was_refused(&frames), "{frames:?}");
    assert!(asks.open_runs().is_empty());
}

#[test]
fn a_key_a_vault_opened_by_a_grant_lacks_is_asked_on_its_one_notice_and_stored_on_allow() {
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let mut want = on_another_vault(root);
    want.env = vec!["X=NOT_THERE".into()];
    let frames = answered_with(root, want, &asks, |open| {
        assert_eq!(open.fills[0].key, "NOT_THERE", "one notice, with its field");
        asks.fill_run(
            &Ctx::new(root, Env::of(&[])),
            open.id,
            vec![("NOT_THERE".into(), "v4lue".to_owned().into())],
            window(),
        )
        .unwrap();
        RunAnswer::Once
    });
    assert!(asks.open().is_empty(), "no Fill beside it: {frames:?}");
    let other = std::fs::read_to_string(root.join("other.json")).unwrap();
    assert!(other.contains("NOT_THERE"), "stored where the run reads it");
    no_vault_was_kept(root);
}

#[test]
fn dont_allow_on_a_vault_the_persona_may_not_use_stores_nothing_there() {
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let mut want = on_another_vault(root);
    want.env = vec!["X=NOT_THERE".into()];
    run_with(&the_chat(root), want, &asks, Duration::ZERO);
    let open = asks.open_runs().remove(0);
    assert!(open.not_its_vault);
    asks.answer_run(open.id, RunAnswer::Refuse, window())
        .unwrap();
    let other = std::fs::read_to_string(root.join("other.json")).unwrap();
    assert!(!other.contains("NOT_THERE"), "{other}");
}

// ----------------------------------------------------------------------------------------
// What the reviews found

#[test]
fn a_program_that_runs_others_as_it_is_told_is_offered_no_run_grant() {
    let project = project();
    let asks = Asks::new(project.path());
    for name in ["find", "make", "vim"] {
        let mut want = printenv(project.path(), &["."]);
        want.command[0] = name.to_owned();
        let frames = run_with(
            &the_chat(project.path()),
            want,
            &asks,
            Duration::from_secs(60),
        );
        assert_eq!(refusal(&frames).1, 1, "{frames:?}");
        assert!(asks.open_runs().is_empty(), "{name}");
    }
}

#[test]
fn the_tools_lock_still_lets_a_missing_key_be_filled() {
    let _policy = Policy::says(r#"{"sandbox": {"persona-tools": false}}"#);
    let project = project();
    let asks = Asks::new(project.path());
    let mut want = printenv(project.path(), &["X"]);
    want.env = vec!["X=NOT_THERE_YET".into()];
    run_with(&the_chat(project.path()), want, &asks, Duration::ZERO);
    let open = asks.open_runs();
    assert_eq!(open.len(), 1, "Fill");
    assert!(
        matches!(
            open[0].offer,
            crate::secrets::ask::run::Offer::Fill { no_allow: Some(_) }
        ),
        "no Allow: {open:?}"
    );
}

#[test]
fn dont_allow_closes_that_programs_other_open_asks_and_their_commands_hear_it() {
    let project = project();
    let asks = Asks::new(project.path());
    let chat = the_chat(project.path());
    run_with(
        &chat,
        printenv(project.path(), &["X"]),
        &asks,
        Duration::ZERO,
    );
    run_with(
        &chat,
        printenv(project.path(), &["Y"]),
        &asks,
        Duration::ZERO,
    );
    assert_eq!(asks.open_runs().len(), 2);
    asks.answer_run(asks.open_runs()[0].id, RunAnswer::Refuse, window())
        .unwrap();
    assert!(asks.open_runs().is_empty());
}

#[test]
fn a_declared_tool_refused_under_a_lock_hears_what_it_always_heard_and_no_policy_is_recorded() {
    let project = project();
    std::fs::write(
        project.path().join("charter.toml"),
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.devops]\ntools = { printenv = { vault = \"team\", as = \"X env\", key = \"TOKEN\" } }\n",
    )
    .unwrap();
    let asks = Asks::new(project.path());
    let steps = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = std::sync::Arc::clone(&steps);
    asks.telling(std::sync::Arc::new(move |chat, step| {
        seen.lock().unwrap().push((chat, step));
    }));
    let unlocked = run_with(
        &the_chat(project.path()),
        printenv(project.path(), &["X"]),
        &asks,
        Duration::from_secs(60),
    );
    let _policy = Policy::says(r#"{"sandbox": {"run-grants": false}}"#);
    let locked = run_with(
        &the_chat(project.path()),
        printenv(project.path(), &["X"]),
        &asks,
        Duration::from_secs(60),
    );
    assert_eq!(refusal(&unlocked), refusal(&locked));
    assert!(steps.lock().unwrap().is_empty());
}

#[test]
fn a_grant_for_any_arguments_runs_a_command_too_long_to_have_been_shown() {
    let project = project();
    let asks = Asks::new(project.path());
    answered(
        project.path(),
        curl(project.path(), &["--version"]),
        &asks,
        RunAnswer::ForChat,
    );
    let long = "x".repeat(MOST_COMMAND_SHOWN + 1);
    let frames = run_with(
        &the_chat(project.path()),
        curl(project.path(), &[&long]),
        &asks,
        Duration::ZERO,
    );
    assert!(!format!("{frames:?}").contains("too long"), "{frames:?}");
}

#[test]
fn a_run_under_a_grant_has_no_input_from_the_chat() {
    let project = project();
    let asks = Asks::new(project.path());
    let mut want = printenv(project.path(), &[]);
    want.command[0] = "cat".into();
    want.stdin = true;
    // `cat` would wait for ever on an asker's input held open; here it has none.
    let frames = answered(project.path(), want, &asks, RunAnswer::Once);
    assert!(
        !matches!(frames.last(), Some(Frame::Refused { code, .. }) if *code == WAITING_ON_THE_PERSON),
        "{frames:?}"
    );
}

#[test]
fn allow_once_is_for_the_folder_it_named() {
    let project = project();
    let asks = Asks::new(project.path());
    let chat = the_chat(project.path());
    answered(
        project.path(),
        printenv(project.path(), &["X"]),
        &asks,
        RunAnswer::Once,
    );
    run_with(
        &chat,
        printenv(project.path(), &["X"]),
        &asks,
        Duration::ZERO,
    );
    let first = asks.open_runs()[0].id;
    asks.answer_run(first, RunAnswer::Once, window()).unwrap();
    let mut elsewhere = printenv(project.path(), &["X"]);
    elsewhere.cwd = Some(project.path().to_path_buf());
    let frames = run_with(&chat, elsewhere, &asks, Duration::ZERO);
    assert_eq!(refusal(&frames).1, WAITING_ON_THE_PERSON, "{frames:?}");
}

#[test]
fn a_chat_can_spell_no_grant_past_a_refused_variable_or_a_line_by_its_case() {
    let project = project();
    let asks = Asks::new(project.path());
    let mut steers = printenv(project.path(), &["X"]);
    steers.env = vec!["NODE_OPTIONS=TOKEN".into()];
    let frames = run_with(
        &the_chat(project.path()),
        steers,
        &asks,
        Duration::from_secs(60),
    );
    assert_eq!(refusal(&frames).1, 1, "{frames:?}");
    assert!(asks.open_runs().is_empty());
    // On a file system that ignores case, `PRINTENV` finds `printenv`, and is no spelling of it.
    let mut shouted = printenv(project.path(), &["X"]);
    shouted.command[0] = "PRINTENV".into();
    let frames = run_with(
        &the_chat(project.path()),
        shouted,
        &asks,
        Duration::from_secs(60),
    );
    assert_eq!(refusal(&frames).1, 1, "{frames:?}");
    assert!(asks.open_runs().is_empty());
}

// ----------------------------------------------------------------------------------------
// The reviewed list (the person's ruling on ADR 0112)

#[test]
fn a_program_not_reviewed_cannot_be_given_a_wider_scope_and_its_ask_stays_open() {
    let project = project();
    let asks = Asks::new(project.path());
    let chat = the_chat(project.path());
    run_with(
        &chat,
        printenv(project.path(), &["X"]),
        &asks,
        Duration::ZERO,
    );
    let id = asks.open_runs()[0].id;
    for wider in [RunAnswer::ForChat, RunAnswer::Always] {
        let why = asks.answer_run(id, wider, window()).unwrap_err();
        assert!(why.contains("printenv"), "{why}");
    }
    assert_eq!(asks.open_runs().len(), 1, "still theirs to answer");
    assert!(asks.given_runs().is_empty());
    // And Allow once on the exact command works.
    asks.answer_run(id, RunAnswer::Once, window()).unwrap();
    let again = run_with(
        &chat,
        printenv(project.path(), &["X"]),
        &asks,
        Duration::ZERO,
    );
    assert!(
        !matches!(again.last(), Some(Frame::Refused { code, .. }) if *code == WAITING_ON_THE_PERSON),
        "{again:?}"
    );
}

#[test]
fn a_declared_tool_off_the_reviewed_list_runs_as_it_always_did() {
    let project = project();
    let asks = Asks::new(project.path());
    let (chat, want) = as_tool(the_chat(project.path()), printenv(project.path(), &["X"]));
    let frames = run_with(&chat, want, &asks, Duration::from_secs(60));
    assert!(asks.open_runs().is_empty(), "{frames:?}");
    assert!(!format!("{frames:?}").contains("reviewed"), "{frames:?}");
}

/// Whether the gate took `want`'s run, heard as the app hears it, with a person who says no.
fn the_gate_took(root: &Path, want: Wanted, asks: &Asks) -> bool {
    use std::sync::atomic::Ordering;
    let gate = RunGate::default();
    run_at_the_gate(root, want, asks, &gate);
    gate.took_it.load(Ordering::Relaxed)
}

/// `want` run by a chat with `gate`, with nobody there to answer.
fn run_at_the_gate(root: &Path, want: Wanted, asks: &Asks, gate: &RunGate) -> Vec<Frame> {
    crate::sandbox::tools::SEARCH_HERE
        .with(|dirs| *dirs.borrow_mut() = Some(vec!["/usr/bin".into(), "/bin".into()]));
    let wire = Wire::default();
    let (reader, _keep) = held_open();
    serve_asking(
        &the_chat(root),
        want,
        reader,
        Box::new(wire.clone()),
        Wrap::Unwrapped,
        Missing {
            asks,
            within: Duration::ZERO,
            moved: &|| {},
            run_gate: Some(gate),
        },
    );
    let bytes = wire.0.lock().unwrap().clone();
    String::from_utf8(bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).expect("every line is a frame"))
        .collect()
}

#[test]
fn the_gate_says_it_took_a_run_it_asks_about_and_a_run_it_remembers_refused() {
    let project = project();
    let asks = Asks::new(project.path());
    let root = project.path();
    assert!(the_gate_took(root, curl_on_another_vault(root), &asks));
    let open = asks.open_runs().into_iter().next().expect("it asked");
    asks.answer_run(open.id, RunAnswer::Refuse, window())
        .unwrap();
    assert!(
        the_gate_took(root, curl_on_another_vault(root), &asks),
        "a Don't allow stands, and asks nothing else"
    );
}

#[test]
fn the_gate_takes_no_run_it_has_no_allow_for() {
    let project = project();
    let asks = Asks::new(project.path());
    let root = project.path();
    let mut interpreter = curl_on_another_vault(root);
    interpreter.command = vec!["python3".into(), "-c".into(), "print(1)".into()];
    assert!(!the_gate_took(root, interpreter, &asks));
    assert!(asks.open_runs().is_empty());
}

#[test]
fn a_vault_kept_blocked_for_this_chat_offers_nothing_that_would_open_it() {
    let project = project();
    let asks = Asks::new(project.path());
    let root = project.path();
    let gate = RunGate {
        kept_blocked: true,
        ..RunGate::default()
    };
    let frames = run_at_the_gate(root, curl_on_another_vault(root), &asks, &gate);
    assert!(asks.open_runs().is_empty(), "nobody is asked");
    assert!(vault_was_refused(&frames), "{frames:?}");
    assert!(!gate.took_it.load(std::sync::atomic::Ordering::Relaxed));

    // A vault that is its persona's is another matter: nothing was kept blocked there.
    run_at_the_gate(root, curl(root, &["--version"]), &asks, &gate);
    assert_eq!(asks.open_runs().len(), 1);
}

/// `/usr/bin/vimdiff` is a link to `vim` where one is installed so.
#[cfg(unix)]
#[test]
fn a_program_a_link_leads_to_a_refused_one_is_refused_with_no_notice() {
    let project = project();
    let asks = Asks::new(project.path());
    let linked = ["vimdiff", "view", "rview", "ex"].into_iter().find(|name| {
        let at = Path::new("/usr/bin").join(name);
        std::fs::read_link(&at).is_ok() && at.canonicalize().is_ok_and(|real| real.ends_with("vim"))
    });
    let Some(linked) = linked else {
        return;
    };
    let mut want = printenv(project.path(), &[]);
    want.command = vec![linked.to_owned(), "--version".to_owned()];
    let frames = run_with(&the_chat(project.path()), want, &asks, Duration::ZERO);
    assert!(asks.open_runs().is_empty(), "{frames:?}");
    assert!(
        refusal(&frames).0.contains("runs other programs"),
        "{frames:?}"
    );
}

#[test]
fn a_lookalike_letter_in_a_command_is_spelled_out_for_the_person() {
    let shown = command_shown(&["curl".to_owned(), "https://\u{430}pi.github.com".to_owned()]);
    assert!(!shown.contains('\u{430}'), "{shown}");
    assert!(shown.contains(r"\xd0\xb0pi.github.com"), "{shown}");
    assert_eq!(
        command_shown(&[
            "curl".to_owned(),
            "https://api.example.com/status".to_owned()
        ]),
        "curl https://api.example.com/status",
        "plain words read as they are"
    );
}

#[test]
fn a_grant_for_the_chat_stops_where_policy_leaves_it_no_scope() {
    let project = project();
    let asks = Asks::new(project.path());
    let root = project.path();
    answered(root, curl(root, &["--version"]), &asks, RunAnswer::ForChat);
    assert_eq!(asks.given_runs().len(), 1);

    let _policy = Policy::says(r#"{"sandbox": {"allow-scopes": ["once"]}}"#);
    let frames = run_with(
        &the_chat(root),
        curl(root, &["--help"]),
        &asks,
        Duration::ZERO,
    );
    assert!(refusal(&frames).0.contains("Policy"), "{frames:?}");
    assert!(asks.open_runs().is_empty(), "{frames:?}");
}

/// Policy holds at the last moment a run could still be stopped: storing a value stays possible
/// under a lock, and the run it was waiting for doesn't go on with it.
#[test]
fn a_lock_set_while_a_fill_waits_stops_the_run_the_value_was_for() {
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let mut want = printenv(root, &["X"]);
    want.env = vec!["X=NOT_THERE_YET".into()];
    let frames = std::thread::scope(|scope| {
        let run = scope.spawn(|| {
            crate::sandbox::tools::SEARCH_HERE
                .with(|dirs| *dirs.borrow_mut() = Some(vec!["/usr/bin".into(), "/bin".into()]));
            let wire = Wire::default();
            let (reader, _keep) = held_open();
            serve_asking(
                &the_chat(root),
                want,
                reader,
                Box::new(wire.clone()),
                Wrap::Unwrapped,
                Missing {
                    asks: &asks,
                    within: Duration::from_secs(60),
                    // Policy comes to lock them once the notice is up, on the run's own thread.
                    moved: &|| {
                        if !asks.open_runs().is_empty() {
                            crate::sandbox::policy::set_for_this_test(
                                crate::sandbox::policy::Locks::parse(
                                    r#"{"sandbox": {"run-grants": false}}"#,
                                    Path::new("/etc/purlis/policy.json"),
                                ),
                            );
                        }
                    },
                    run_gate: None,
                },
            );
            let bytes = wire.0.lock().unwrap().clone();
            String::from_utf8(bytes)
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str(line).expect("every line is a frame"))
                .collect::<Vec<Frame>>()
        });
        let open = open_ask(&asks, || run.is_finished());
        asks.fill_run(
            &Ctx::new(root, Env::of(&[])),
            open.id,
            vec![("NOT_THERE_YET".into(), "fr3sh-v4lue".to_owned().into())],
            window(),
        )
        .unwrap();
        asks.answer_run(open.id, RunAnswer::Once, window()).unwrap();
        run.join().unwrap()
    });
    assert!(refusal(&frames).0.contains("olicy"), "{frames:?}");
    let team = std::fs::read_to_string(root.join("team.json")).unwrap();
    assert!(team.contains("NOT_THERE_YET"), "the value stays stored");
    assert!(
        !frames
            .iter()
            .any(|frame| matches!(frame, Frame::Exit { .. })),
        "nothing ran: {frames:?}"
    );
}

#[test]
fn a_variable_name_longer_than_a_person_reads_is_not_offered() {
    let project = project();
    let asks = Asks::new(project.path());
    let mut want = printenv(project.path(), &["X"]);
    want.env = vec![format!("{}=TOKEN", "A".repeat(65))];
    let frames = run_with(&the_chat(project.path()), want, &asks, Duration::ZERO);
    assert_eq!(refusal(&frames).1, 1, "{frames:?}");
    assert!(asks.open_runs().is_empty(), "{frames:?}");
}

#[test]
fn programs_that_keep_running_what_they_are_told_are_not_offered() {
    let project = project();
    let asks = Asks::new(project.path());
    for program in ["git", "screen", "tmux", "m4"] {
        let mut want = printenv(project.path(), &[]);
        want.command = vec![program.to_owned(), "--version".to_owned()];
        run_with(&the_chat(project.path()), want, &asks, Duration::ZERO);
        assert!(asks.open_runs().is_empty(), "{program}");
    }
}

/// A login is filled by purlis whoever's it is: a key another persona's browser login or fill line
/// holds is no program's, on a vault that isn't the asking persona's as on its own.
#[test]
fn a_key_another_personas_browser_fills_is_never_offered_to_a_program() {
    let project = project();
    std::fs::write(
        project.path().join("charter.toml"),
        r#"[sandbox]
open = "off"
[sandbox.personas.ops]
tools = { shop = { vault = "other", browser = "https://shop.example.test/login", key = "TOKEN" } }
[sandbox.personas.qa]
tools = { pay = { vault = "team", key = "TOKEN", fill = "https://pay.example.test" } }
"#,
    )
    .unwrap();
    let asks = Asks::new(project.path());
    for want in [
        on_another_vault(project.path()),
        printenv(project.path(), &["X"]),
    ] {
        let vault = want.vault.clone();
        let frames = run_with(&the_chat(project.path()), want, &asks, Duration::ZERO);
        assert_eq!(refusal(&frames).1, 1, "{vault}: {frames:?}");
        assert!(
            refusal(&frames).0.contains("fills in a browser"),
            "{vault}: {frames:?}"
        );
        assert!(asks.open_runs().is_empty(), "{vault} asked nobody");
    }
}

/// What a run under a grant connects to and trusts is what the person read: the chat's hosts,
/// ports and certificate authorities stay behind, so a chat that wants them says them in the
/// command.
#[test]
fn a_granted_run_takes_none_of_the_chats_hosts_ports_or_certificate_authorities() {
    let project = project();
    let asks = Asks::new(project.path());
    for var in [
        "PGHOST",
        "PGHOSTADDR",
        "PGPORT",
        "PGSSLMODE",
        "MYSQL_HOST",
        "MYSQL_TCP_PORT",
        "REDIS_HOST",
        "REDIS_PORT",
        "AWS_REGION",
        "AWS_DEFAULT_REGION",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "NODE_EXTRA_CA_CERTS",
        "REQUESTS_CA_BUNDLE",
        "CURL_CA_BUNDLE",
        "AWS_CA_BUNDLE",
    ] {
        let mut want = printenv(project.path(), &[var]);
        want.environment = vec![(var.into(), "evil.example".into())];
        let frames = answered(project.path(), want, &asks, RunAnswer::Once);
        assert_eq!(frames.last(), Some(&Frame::Exit(1)), "{var}: {frames:?}");
    }
    // The chat's language still reaches it: only where it connects is left out.
    let mut want = printenv(project.path(), &["LANG"]);
    want.environment = vec![("LANG".into(), "C".into())];
    let frames = answered(project.path(), want, &asks, RunAnswer::Once);
    assert_eq!(frames.last(), Some(&Frame::Exit(0)), "{frames:?}");
}

/// A browser's key is the same key however it is reached: through a link the person made either
/// way, and in any case, as a provider may read it.
#[test]
fn a_browsers_key_reached_through_a_link_or_in_another_case_is_never_offered_to_a_program() {
    let project = project();
    let root = project.path();
    std::fs::write(
        root.join("charter.toml"),
        r#"[sandbox]
open = "off"
[sandbox.personas.ops]
tools = { shop = { vault = "other", browser = "https://shop.example.test/login", key = "pw-link" } }
[sandbox.personas.qa]
tools = { pay = { vault = "other", key = "token", fill = "https://pay.example.test" } }
"#,
    )
    .unwrap();
    let ctx = Ctx::new(root, Env::of(&[]));
    std::fs::create_dir_all(&ctx.state).unwrap();
    std::fs::write(
        crate::secrets::links::path(&ctx),
        r#"{"links": {"other": {"pw-link": {"vault": "team", "key": "TOKEN"}}}}"#,
    )
    .unwrap();
    let asks = Asks::new(root);
    // `team`'s TOKEN is what the login's linked entry reads; `other`'s TOKEN is the fill line's
    // `token` in another case.
    for want in [printenv(root, &["X"]), on_another_vault(root)] {
        let vault = want.vault.clone();
        let frames = run_with(&the_chat(root), want, &asks, Duration::ZERO);
        assert_eq!(refusal(&frames).1, 1, "{vault}: {frames:?}");
        assert!(
            refusal(&frames).0.contains("fills in a browser"),
            "{vault}: {frames:?}"
        );
        assert!(asks.open_runs().is_empty(), "{vault} asked nobody");
    }
}

/// **D-5246-1** (the person's ruling, amending D-5080-11): a key of the persona's vault that is a
/// link into a vault the persona may not use goes to no granted program, however the link came to
/// be, and nobody is asked: the notice would name only the persona's own vault. Covers links made
/// before the picker and the press were narrowed.
#[test]
fn a_key_linked_into_a_vault_the_persona_may_not_use_is_never_offered_to_a_program() {
    let project = project();
    let root = project.path();
    std::fs::write(
        root.join("other.json"),
        serde_json::json!({"PROD_PW": "pr0d-v4lue"}).to_string(),
    )
    .unwrap();
    let ctx = Ctx::new(root, Env::of(&[]));
    std::fs::create_dir_all(&ctx.state).unwrap();
    std::fs::write(
        crate::secrets::links::path(&ctx),
        r#"{"links": {"team": {"DEPLOY": {"vault": "other", "key": "PROD_PW"}}}}"#,
    )
    .unwrap();
    let asks = Asks::new(root);
    let mut want = curl(root, &["--version"]);
    want.env = vec!["X=DEPLOY".into()];

    let frames = run_with(&the_chat(root), want, &asks, Duration::ZERO);

    let (why, code) = refusal(&frames);
    assert_eq!(code, 1, "{frames:?}");
    assert!(why.contains("may not use"), "says why: {why}");
    assert!(!why.contains("pr0d-v4lue"), "never the value: {why}");
    assert!(asks.open_runs().is_empty(), "asked nobody");
}

/// A key that came to be a browser's while the person read the notice goes to no program, however
/// they answer: the gate judges it again once allowed, as it judges policy.
#[test]
fn a_key_a_login_came_to_hold_while_the_ask_was_up_is_not_handed_on_allow() {
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let frames = std::thread::scope(|scope| {
        let run = scope.spawn(|| {
            crate::sandbox::tools::SEARCH_HERE
                .with(|dirs| *dirs.borrow_mut() = Some(vec!["/usr/bin".into(), "/bin".into()]));
            let wire = Wire::default();
            let (reader, _keep) = held_open();
            serve_asking(
                &the_chat(root),
                printenv(root, &["X"]),
                reader,
                Box::new(wire.clone()),
                Wrap::Unwrapped,
                Missing {
                    asks: &asks,
                    within: Duration::from_secs(60),
                    moved: &|| {},
                    run_gate: None,
                },
            );
            let bytes = wire.0.lock().unwrap().clone();
            String::from_utf8(bytes)
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str(line).expect("every line is a frame"))
                .collect::<Vec<Frame>>()
        });
        let open = open_ask(&asks, || run.is_finished());
        // A teammate's login line comes in with a pull while the person reads the notice.
        std::fs::write(
            root.join("charter.toml"),
            "[sandbox.personas.qa.tools.shop]\nvault = \"team\"\nbrowser = \
             \"https://shop.example.test/login\"\nkey = \"TOKEN\"\n",
        )
        .unwrap();
        asks.answer_run(open.id, RunAnswer::Once, window()).unwrap();
        run.join().unwrap()
    });
    assert!(
        refusal(&frames).0.contains("fills in a browser"),
        "{frames:?}"
    );
    assert!(
        !frames
            .iter()
            .any(|frame| matches!(frame, Frame::Exit { .. })),
        "nothing ran: {frames:?}"
    );
    // The person's ruling (#5242): a check after Allow that refuses the run drops its Allow
    // once, which no retry may use.
    assert!(asks.given_runs().is_empty(), "{:?}", asks.given_runs());
}

/// A key the run misses may be filled with an entry the person keeps already, as a link: one a
/// browser login fills goes to no program that way either, though the Allow was given before.
#[test]
fn a_missing_key_filled_with_a_link_to_a_browsers_key_is_not_handed_on() {
    let project = project();
    let root = project.path();
    std::fs::write(
        root.join("charter.toml"),
        "[sandbox.personas.qa.tools.shop]\nvault = \"team\"\nbrowser = \
         \"https://shop.example.test/login\"\nkey = \"TOKEN\"\n",
    )
    .unwrap();
    let asks = Asks::new(root);
    // Allowed for the chat on a notice whose field was filled; then the key goes again, so a run
    // the grant covers asks for it alone, as before, where a link may be picked.
    let want = || {
        let mut want = curl(root, &["--version"]);
        want.env = vec!["X=SHOP_PASSWORD".into()];
        want
    };
    answered_with(root, want(), &asks, |open| {
        asks.fill_run(
            &Ctx::new(root, Env::of(&[])),
            open.id,
            vec![("SHOP_PASSWORD".into(), "first".to_owned().into())],
            window(),
        )
        .unwrap();
        RunAnswer::ForChat
    });
    std::fs::write(
        root.join("team.json"),
        serde_json::json!({"TOKEN": TOKEN}).to_string(),
    )
    .unwrap();
    let frames = std::thread::scope(|scope| {
        let run = scope.spawn(|| run_with(&the_chat(root), want(), &asks, Duration::from_secs(60)));
        let id = loop {
            if let Some(one) = asks.open().first() {
                break one.id;
            }
            assert!(
                !run.is_finished(),
                "the run ended without asking for its key"
            );
            std::thread::yield_now();
        };
        // The press itself is refused now, before anything is linked (D-5246-1): the gate's
        // own check after a Fill is the second line behind it.
        let pressed = asks.answer(
            &Ctx::new(root, Env::of(&[])),
            id,
            crate::secrets::ask::Answer::Link(crate::secrets::links::Target {
                vault: "team".into(),
                key: "TOKEN".into(),
            }),
            window(),
        );
        assert!(
            pressed
                .as_ref()
                .is_err_and(|why| why.contains("fills in a browser")),
            "{pressed:?}"
        );
        asks.answer(
            &Ctx::new(root, Env::of(&[])),
            id,
            crate::secrets::ask::Answer::Decline,
            window(),
        )
        .unwrap();
        run.join().unwrap()
    });
    assert_eq!(
        crate::secrets::links::of(&Ctx::new(root, Env::of(&[])), "team", "SHOP_PASSWORD").unwrap(),
        None,
        "nothing linked"
    );
    assert!(
        !frames
            .iter()
            .any(|frame| matches!(frame, Frame::Exit { .. })),
        "nothing ran: {frames:?}"
    );
}

#[test]
fn a_key_a_vault_its_persona_may_not_use_lacks_under_a_grant_is_asked_with_the_vaults_own_allow() {
    use crate::secrets::ask::run::Offer;
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    run_with(
        &the_chat(root),
        on_another_vault(root),
        &asks,
        Duration::ZERO,
    );
    let open = asks.open_runs().remove(0);
    asks.answer_run(open.id, RunAnswer::ForChat, window())
        .unwrap();
    std::fs::write(root.join("other.json"), "{}").unwrap();
    let frames = run_with(
        &the_chat(root),
        on_another_vault(root),
        &asks,
        Duration::ZERO,
    );
    let open = asks.open_runs();
    assert_eq!(open.len(), 1, "one notice: {frames:?}");
    assert_eq!(
        open[0].offer,
        Offer::Vault { no_allow: None },
        "only an Allow stores in a vault the persona may not use"
    );
    assert!(
        !format!("{frames:?}").contains("TOKEN"),
        "the chat is not told which keys it lacks: {frames:?}"
    );
}

#[test]
fn an_allow_once_a_lock_refuses_after_its_retrys_fill_is_dropped() {
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let want = || {
        let mut want = printenv(root, &["X"]);
        want.env = vec!["X=NOT_A".into()];
        want
    };
    // Allow once given after its run stopped waiting: kept for the retry.
    run_with(&the_chat(root), want(), &asks, Duration::ZERO);
    let open = asks.open_runs().remove(0);
    asks.fill_run(
        &Ctx::new(root, Env::of(&[])),
        open.id,
        vec![("NOT_A".into(), "first".to_owned().into())],
        window(),
    )
    .unwrap();
    asks.answer_run(open.id, RunAnswer::Once, window()).unwrap();
    assert_eq!(asks.given_runs().len(), 1);
    // The key goes again, so the retry asks for it alone; policy locks while it is filled.
    std::fs::write(
        root.join("team.json"),
        serde_json::json!({"TOKEN": TOKEN}).to_string(),
    )
    .unwrap();
    let frames = std::thread::scope(|scope| {
        let run = scope.spawn(|| {
            crate::sandbox::tools::SEARCH_HERE
                .with(|dirs| *dirs.borrow_mut() = Some(vec!["/usr/bin".into(), "/bin".into()]));
            let wire = Wire::default();
            let (reader, _keep) = held_open();
            serve_asking(
                &the_chat(root),
                want(),
                reader,
                Box::new(wire.clone()),
                Wrap::Unwrapped,
                Missing {
                    asks: &asks,
                    within: Duration::from_secs(60),
                    moved: &|| {
                        if !asks.open().is_empty() {
                            crate::sandbox::policy::set_for_this_test(
                                crate::sandbox::policy::Locks::parse(
                                    r#"{"sandbox": {"run-grants": false}}"#,
                                    Path::new("/etc/purlis/policy.json"),
                                ),
                            );
                        }
                    },
                    run_gate: None,
                },
            );
            let bytes = wire.0.lock().unwrap().clone();
            String::from_utf8(bytes)
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str(line).expect("every line is a frame"))
                .collect::<Vec<Frame>>()
        });
        let id = loop {
            if let Some(one) = asks.open().first() {
                break one.id;
            }
            if run.is_finished() {
                panic!(
                    "the run ended without asking for its key: {:?}",
                    run.join().unwrap()
                );
            }
            std::thread::yield_now();
        };
        asks.answer(
            &Ctx::new(root, Env::of(&[])),
            id,
            crate::secrets::ask::Answer::Fill("second".to_owned().into()),
            window(),
        )
        .unwrap();
        run.join().unwrap()
    });
    assert!(refusal(&frames).0.contains("olicy"), "{frames:?}");
    // The person's ruling (#5242): refused after its Allow, the Allow once goes unused.
    assert!(asks.given_runs().is_empty(), "{:?}", asks.given_runs());
}

/// `name` asked for by a chat, refused at once with the sentence for a program that runs others,
/// and no notice raised (#5424).
fn refused_as_running_others(name: &str) {
    let project = project();
    let asks = Asks::new(project.path());
    let mut want = printenv(project.path(), &["."]);
    want.command[0] = name.to_owned();
    let frames = run_with(&the_chat(project.path()), want, &asks, Duration::ZERO);
    assert!(asks.open_runs().is_empty(), "{name}: {frames:?}");
    assert!(
        refusal(&frames).0.contains("runs other programs"),
        "{name}: {frames:?}"
    );
}

#[test]
fn a_gnu_build_of_a_refused_program_by_its_g_name_is_offered_no_run_grant() {
    for name in ["gm4", "genv", "gtimeout", "gnice"] {
        refused_as_running_others(name);
    }
}

#[test]
fn a_gnu_build_of_a_refused_program_by_its_gnu_name_is_offered_no_run_grant() {
    for name in ["gnumake", "gnum4"] {
        refused_as_running_others(name);
    }
}

#[test]
fn a_git_program_by_its_dashed_name_is_offered_no_run_grant() {
    for name in ["git-shell", "git-upload-archive", "git-lfs"] {
        refused_as_running_others(name);
    }
}

#[test]
fn a_versioned_compiler_is_offered_no_run_grant() {
    for name in [
        "gcc-13",
        "clang-17",
        "clang++-17",
        "g++-14",
        "gcc13",
        "cc-12.2",
    ] {
        refused_as_running_others(name);
    }
}

#[test]
fn a_cross_compiler_named_for_its_target_is_offered_no_run_grant() {
    for name in [
        "x86_64-linux-gnu-gcc",
        "aarch64-linux-gnu-gcc-13",
        "x86_64-w64-mingw32-ld",
    ] {
        refused_as_running_others(name);
    }
}

#[test]
fn a_refused_program_named_with_a_suffix_or_a_sibling_name_is_offered_no_run_grant() {
    for name in [
        "bm4",
        "c89",
        "c99",
        "emacs-nox",
        "gdb-multiarch",
        "ninja-build",
        "sudo-rs",
        "tmux-next",
        "swiftc",
        "gfortran",
        "libtool",
    ] {
        refused_as_running_others(name);
    }
}

// #5619: programs that run others, found by #5595's adversarial review.

#[test]
fn xcodebuild_runs_a_project_s_build_phases_so_it_is_offered_no_run_grant() {
    refused_as_running_others("xcodebuild");
}

#[test]
fn msgexec_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("msgexec");
}

#[test]
fn leaks_runs_the_command_after_at_exit_so_it_is_offered_no_run_grant() {
    refused_as_running_others("leaks");
}

#[test]
fn pyenv_runs_its_shims_and_exec_so_it_is_offered_no_run_grant() {
    refused_as_running_others("pyenv");
}

#[test]
fn erb_runs_the_ruby_in_its_template_so_it_is_offered_no_run_grant() {
    refused_as_running_others("erb");
}

#[test]
fn helm_runs_a_post_renderer_and_plugins_so_it_is_offered_no_run_grant() {
    refused_as_running_others("helm");
}

#[test]
fn socat_runs_what_an_exec_address_names_so_it_is_offered_no_run_grant() {
    refused_as_running_others("socat");
}

#[test]
fn ncat_runs_what_exec_names_so_it_is_offered_no_run_grant() {
    refused_as_running_others("ncat");
}

#[test]
fn nc_runs_what_e_names_so_it_is_offered_no_run_grant() {
    refused_as_running_others("nc");
}

#[test]
fn netcat_runs_what_e_names_so_it_is_offered_no_run_grant() {
    refused_as_running_others("netcat");
}

#[test]
fn tar_runs_a_to_command_or_compressor_so_it_is_offered_no_run_grant() {
    refused_as_running_others("tar");
}

#[test]
fn bsdtar_runs_a_compressor_it_is_told_so_it_is_offered_no_run_grant() {
    refused_as_running_others("bsdtar");
}

#[test]
fn gtar_runs_a_to_command_or_compressor_so_it_is_offered_no_run_grant() {
    refused_as_running_others("gtar");
}

// #5617's review of #5619: the same family, refused with it.

#[test]
fn op_runs_what_op_run_names_so_it_is_offered_no_run_grant() {
    refused_as_running_others("op");
}

#[test]
fn aws_vault_runs_what_aws_vault_exec_names_so_it_is_offered_no_run_grant() {
    refused_as_running_others("aws-vault");
}

#[test]
fn rbenv_runs_its_shims_and_exec_so_it_is_offered_no_run_grant() {
    refused_as_running_others("rbenv");
}

#[test]
fn asdf_runs_its_shims_and_exec_so_it_is_offered_no_run_grant() {
    refused_as_running_others("asdf");
}

#[test]
fn mise_runs_its_tasks_and_exec_so_it_is_offered_no_run_grant() {
    refused_as_running_others("mise");
}

#[test]
fn direnv_runs_what_direnv_exec_and_an_envrc_say_so_it_is_offered_no_run_grant() {
    refused_as_running_others("direnv");
}

#[test]
fn pipenv_runs_what_pipenv_run_names_so_it_is_offered_no_run_grant() {
    refused_as_running_others("pipenv");
}

#[test]
fn conda_runs_what_conda_run_names_so_it_is_offered_no_run_grant() {
    refused_as_running_others("conda");
}

#[test]
fn volta_runs_what_volta_run_names_so_it_is_offered_no_run_grant() {
    refused_as_running_others("volta");
}

#[test]
fn msgfilter_runs_the_filter_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("msgfilter");
}

#[test]
fn xctrace_runs_what_it_launches_so_it_is_offered_no_run_grant() {
    refused_as_running_others("xctrace");
}

#[test]
fn valgrind_runs_the_program_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("valgrind");
}

#[test]
fn hyperfine_runs_the_commands_it_times_so_it_is_offered_no_run_grant() {
    refused_as_running_others("hyperfine");
}

#[test]
fn helmfile_runs_its_hooks_so_it_is_offered_no_run_grant() {
    refused_as_running_others("helmfile");
}

// The second review of #5619: secret injectors, environment managers and project runners.

#[test]
fn doppler_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("doppler");
}

#[test]
fn infisical_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("infisical");
}

#[test]
fn sops_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("sops");
}

#[test]
fn chamber_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("chamber");
}

#[test]
fn envconsul_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("envconsul");
}

#[test]
fn summon_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("summon");
}

#[test]
fn envchain_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("envchain");
}

#[test]
fn dotenvx_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("dotenvx");
}

#[test]
fn teller_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("teller");
}

#[test]
fn berglas_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("berglas");
}

#[test]
fn aws_sso_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("aws-sso");
}

#[test]
fn granted_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("granted");
}

#[test]
fn assume_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("assume");
}

#[test]
fn mamba_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("mamba");
}

#[test]
fn micromamba_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("micromamba");
}

#[test]
fn rtx_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("rtx");
}

#[test]
fn goenv_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("goenv");
}

#[test]
fn jenv_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("jenv");
}

#[test]
fn fnm_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("fnm");
}

#[test]
fn nix_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("nix");
}

#[test]
fn nix_shell_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("nix-shell");
}

#[test]
fn devbox_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("devbox");
}

#[test]
fn pixi_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("pixi");
}

#[test]
fn hatch_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("hatch");
}

#[test]
fn pdm_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("pdm");
}

#[test]
fn rye_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("rye");
}

#[test]
fn tox_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("tox");
}

#[test]
fn nox_runs_the_command_it_is_given_so_it_is_offered_no_run_grant() {
    refused_as_running_others("nox");
}

// The third review of #5619: more secret injectors, and programs that run what a chat wrote.

#[test]
fn saml2aws_runs_what_it_is_given_or_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("saml2aws");
}

#[test]
fn aws_okta_runs_what_it_is_given_or_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("aws-okta");
}

#[test]
fn bws_runs_what_it_is_given_or_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("bws");
}

#[test]
fn dotenv_runs_what_it_is_given_or_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("dotenv");
}

#[test]
fn phase_runs_what_it_is_given_or_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("phase");
}

#[test]
fn awsume_runs_what_it_is_given_or_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("awsume");
}

#[test]
fn bunx_runs_what_it_is_given_or_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("bunx");
}

#[test]
fn tsx_runs_what_it_is_given_or_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("tsx");
}

#[test]
fn pytest_runs_what_it_is_given_or_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("pytest");
}

#[test]
fn watchexec_runs_what_it_is_given_or_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("watchexec");
}

#[test]
fn entr_runs_what_it_is_given_or_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("entr");
}

#[test]
fn pre_commit_runs_what_it_is_given_or_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("pre-commit");
}

#[test]
fn pulumi_runs_what_it_is_given_or_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("pulumi");
}

#[test]
fn cdk_runs_what_it_is_given_or_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("cdk");
}

// The fourth review of #5619: programs that run what a file a chat wrote says.

#[test]
fn terraform_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("terraform");
}

#[test]
fn tofu_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("tofu");
}

#[test]
fn terragrunt_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("terragrunt");
}

#[test]
fn ansible_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("ansible");
}

#[test]
fn ansible_playbook_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("ansible-playbook");
}

#[test]
fn vagrant_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("vagrant");
}

#[test]
fn packer_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("packer");
}

#[test]
fn cdktf_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("cdktf");
}

#[test]
fn sst_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("sst");
}

#[test]
fn serverless_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("serverless");
}

#[test]
fn jest_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("jest");
}

#[test]
fn vitest_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("vitest");
}

#[test]
fn mocha_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("mocha");
}

#[test]
fn rspec_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("rspec");
}

// #5799: programs whose flags name a command. Refused whole where the program is the language
// that runs it, or reads a file a chat wrote; offered without the flag otherwise.

#[test]
fn sed_runs_a_command_its_script_names_so_it_is_offered_no_run_grant() {
    refused_as_running_others("sed");
}

#[test]
fn aria2c_runs_the_command_its_hooks_name_so_it_is_offered_no_run_grant() {
    refused_as_running_others("aria2c");
}

#[test]
fn kustomize_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("kustomize");
}

/// `name args…` run by a chat raises no notice, and the chat hears the form it named and to leave
/// it out.
fn refused_for_its_flag(name: &str, args: &[&str], form: &str) {
    let project = project();
    let asks = Asks::new(project.path());
    let mut want = printenv(project.path(), args);
    want.command[0] = name.to_owned();
    let frames = run_with(&the_chat(project.path()), want, &asks, Duration::ZERO);
    assert!(asks.open_runs().is_empty(), "{name}: {frames:?}");
    let (why, _) = refusal(&frames);
    assert!(why.contains(form), "{name}: {why}");
    assert!(why.contains(&format!("leave {form} out")), "{name}: {why}");
}

#[test]
fn ssh_with_a_proxy_command_is_offered_no_run_grant_and_the_chat_hears_to_leave_it_out() {
    refused_for_its_flag(
        "ssh",
        &["-oProxyCommand=sh -c x", "host", "uptime"],
        "-o ProxyCommand",
    );
}

#[test]
fn rsync_with_a_remote_shell_is_offered_no_run_grant_and_the_chat_hears_to_leave_it_out() {
    refused_for_its_flag("rsync", &["-avze", "x", "a", "host:b"], "-e");
}

#[test]
fn psql_with_a_file_of_commands_is_offered_no_run_grant_and_the_chat_hears_to_leave_it_out() {
    refused_for_its_flag("psql", &["-f", "x.sql"], "-f");
}

// #5833: more programs that run others, or run what a chat wrote.

// #5833: programs that run the command after them, or a command they name.

#[test]
fn consul_template_runs_the_command_after_it_so_it_is_offered_no_run_grant() {
    refused_as_running_others("consul-template");
}

#[test]
fn railway_runs_the_command_after_it_so_it_is_offered_no_run_grant() {
    refused_as_running_others("railway");
}

#[test]
fn aws_runas_runs_the_command_after_it_so_it_is_offered_no_run_grant() {
    refused_as_running_others("aws-runas");
}

#[test]
fn vals_runs_the_command_after_it_so_it_is_offered_no_run_grant() {
    refused_as_running_others("vals");
}

#[test]
fn gomplate_runs_the_command_after_it_so_it_is_offered_no_run_grant() {
    refused_as_running_others("gomplate");
}

#[test]
fn envdir_runs_the_command_after_it_so_it_is_offered_no_run_grant() {
    refused_as_running_others("envdir");
}

#[test]
fn chpst_runs_the_command_after_it_so_it_is_offered_no_run_grant() {
    refused_as_running_others("chpst");
}

#[test]
fn systemd_run_runs_the_command_after_it_so_it_is_offered_no_run_grant() {
    refused_as_running_others("systemd-run");
}

#[test]
fn telepresence_runs_the_command_after_it_so_it_is_offered_no_run_grant() {
    refused_as_running_others("telepresence");
}

#[test]
fn run0_runs_the_command_after_it_so_it_is_offered_no_run_grant() {
    refused_as_running_others("run0");
}

#[test]
fn firejail_runs_the_command_after_it_so_it_is_offered_no_run_grant() {
    refused_as_running_others("firejail");
}

#[test]
fn flatpak_runs_the_command_after_it_so_it_is_offered_no_run_grant() {
    refused_as_running_others("flatpak");
}

#[test]
fn flatpak_spawn_runs_the_command_after_it_so_it_is_offered_no_run_grant() {
    refused_as_running_others("flatpak-spawn");
}

#[test]
fn rr_runs_the_command_after_it_so_it_is_offered_no_run_grant() {
    refused_as_running_others("rr");
}

#[test]
fn pkgx_runs_the_command_after_it_so_it_is_offered_no_run_grant() {
    refused_as_running_others("pkgx");
}

#[test]
fn flox_runs_the_command_after_it_so_it_is_offered_no_run_grant() {
    refused_as_running_others("flox");
}

// #5833: programs that run what a file a chat wrote says.

#[test]
fn task_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("task");
}

#[test]
fn skaffold_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("skaffold");
}

#[test]
fn tilt_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("tilt");
}

#[test]
fn dagger_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("dagger");
}

#[test]
fn earthly_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("earthly");
}

#[test]
fn act_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("act");
}

#[test]
fn firebase_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("firebase");
}

#[test]
fn netlify_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("netlify");
}

#[test]
fn vercel_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("vercel");
}

#[test]
fn wrangler_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("wrangler");
}

#[test]
fn chezmoi_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("chezmoi");
}

#[test]
fn mage_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("mage");
}

#[test]
fn invoke_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("invoke");
}

#[test]
fn fab_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("fab");
}

#[test]
fn cdk8s_runs_what_a_file_a_chat_wrote_says_so_it_is_offered_no_run_grant() {
    refused_as_running_others("cdk8s");
}

// #5833: build tools and test runners, which build and run what a chat wrote.

#[test]
fn sbt_builds_and_runs_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("sbt");
}

#[test]
fn mix_builds_and_runs_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("mix");
}

#[test]
fn playwright_builds_and_runs_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("playwright");
}

#[test]
fn cypress_builds_and_runs_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("cypress");
}

/// `phpunit` is refused before any Run grant is weighed, as every name `php` starts is: it runs
/// what it is told, so it may be no tool either.
#[test]
fn phpunit_builds_and_runs_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    let project = project();
    let asks = Asks::new(project.path());
    let mut want = printenv(project.path(), &["."]);
    want.command[0] = "phpunit".to_owned();
    let frames = run_with(&the_chat(project.path()), want, &asks, Duration::ZERO);
    assert!(asks.open_runs().is_empty(), "{frames:?}");
    assert!(
        refusal(&frames).0.contains("runs whatever it is told"),
        "{frames:?}"
    );
}

#[test]
fn gitlab_runner_builds_and_runs_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("gitlab-runner");
}

#[test]
fn circleci_builds_and_runs_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("circleci");
}

#[test]
fn stack_builds_and_runs_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("stack");
}

#[test]
fn cabal_builds_and_runs_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("cabal");
}

#[test]
fn dune_builds_and_runs_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("dune");
}

#[test]
fn zig_builds_and_runs_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("zig");
}

#[test]
fn lein_builds_and_runs_what_a_chat_wrote_so_it_is_offered_no_run_grant() {
    refused_as_running_others("lein");
}

// The review of #5876: the closest kin of ssh and the programs that run the command after them.

#[test]
fn sshpass_runs_the_command_after_it_so_it_is_offered_no_run_grant() {
    refused_as_running_others("sshpass");
}

#[test]
fn autossh_runs_ssh_with_whatever_it_is_handed_so_it_is_offered_no_run_grant() {
    refused_as_running_others("autossh");
}

#[test]
fn mosh_runs_the_ssh_command_it_names_so_it_is_offered_no_run_grant() {
    refused_as_running_others("mosh");
}

#[test]
fn lftp_runs_commands_from_its_own_prompt_and_scripts_so_it_is_offered_no_run_grant() {
    refused_as_running_others("lftp");
}

/// The second review of #5876: under policy's lock a key the vault lacks is still asked for, but
/// not for a command whose flag names a command, which no Allow could follow.
#[test]
fn locked_a_command_whose_flag_names_a_command_asks_for_no_key() {
    let _policy = Policy::says(r#"{"sandbox": {"run-grants": false}}"#);
    let project = project();
    let asks = Asks::new(project.path());
    let mut want = printenv(project.path(), &["-oProxyCommand=x", "host"]);
    want.command[0] = "ssh".to_owned();
    want.env = vec!["X=NOT_THERE_YET".into()];
    let frames = run_with(&the_chat(project.path()), want, &asks, Duration::ZERO);
    assert!(asks.open_runs().is_empty(), "{frames:?}");
    assert_ne!(refusal(&frames).1, WAITING_ON_THE_PERSON, "{frames:?}");
}

// The sixth review of #5876: mysql's and mariadb's server-side and shell-like tools run what
// their flags or files name, and none is a usual reason to hand a chat a key.

#[test]
fn mysql_server_side_and_shell_like_tools_run_what_they_name_so_they_are_offered_no_run_grant() {
    for name in [
        "mysql.server",
        "mariadb.server",
        "mysqld_safe",
        "mariadbd-safe",
        "mysqld_multi",
        "mysql_install_db",
        "mariadb-install-db",
        "mysqlslap",
        "mariadb-slap",
        "mysqltest",
        "mariadb-test",
        "mysqlsh",
        "mariadbd-multi",
        "mysqltest_embedded",
        "mariadb-test-embedded",
    ] {
        refused_as_running_others(name);
    }
}

// #5891, from the seventh review of #5876: the MySQL and MariaDB tools no row had read. Each is
// refused whole: `mysqlhotcopy --method` names the copy program it has a shell run, the secure
// installation is a script that runs the client its `--basedir` finds, and mariabackup runs
// `qpress`, `rsync` and the plugins its flags name. None is a usual reason to hand a chat a key.

#[test]
fn mysql_hotcopy_secure_installation_and_backup_run_what_they_name_so_they_are_offered_no_run_grant()
 {
    for name in [
        "mysqlhotcopy",
        "mariadb-hotcopy",
        "mysql_secure_installation",
        "mariadb-secure-installation",
        "mariabackup",
        "mariadb-backup",
    ] {
        refused_as_running_others(name);
    }
}

// #5931, from the second review of #5929: the MySQL and MariaDB tools and their kin that no row
// read, or read only by the long forms of the flags that name a program. Each is refused whole.

/// `mysql_plugin` starts the server it finds (`-m`, `--mysqld`, `-b`) with the plugins of a
/// folder it is told (`-p`), and `--my-print-defaults` names a program it runs.
#[test]
fn mysql_plugin_runs_the_server_and_programs_its_flags_name_so_it_is_offered_no_run_grant() {
    for name in ["mysql_plugin", "mariadb-plugin"] {
        refused_as_running_others(name);
    }
}

/// `mysql_upgrade` runs the client and `mysqlcheck` with the files of settings and plugins it
/// is handed, as the server's administrator.
#[test]
fn mysql_upgrade_runs_the_client_with_what_it_is_handed_so_it_is_offered_no_run_grant() {
    for name in ["mysql_upgrade", "mariadb-upgrade"] {
        refused_as_running_others(name);
    }
}

/// Percona's backup tools, as mariabackup is: `xtrabackup` runs the programs and plugins its
/// flags name, and its kin read and write the streams and keys it makes.
#[test]
fn percona_xtrabackup_and_its_kin_run_what_they_name_so_they_are_offered_no_run_grant() {
    for name in [
        "xtrabackup",
        "xtrabackup-80",
        "innobackupex",
        "xbstream",
        "xbcloud",
        "xbcloud_osenv",
        "xbcrypt",
        // MariaDB's own counterpart to xbstream, mariabackup's (round 1 of #5990's review).
        "mbstream",
    ] {
        refused_as_running_others(name);
    }
}

/// The server's state transfer scripts, by their prefix: each is a shell script that runs the
/// transfer program its arguments name, and new ones come with each release.
#[test]
fn every_wsrep_sst_script_runs_a_transfer_program_so_it_is_offered_no_run_grant() {
    for name in [
        "wsrep_sst_rsync",
        "wsrep_sst_rsync_wan",
        "wsrep_sst_mariabackup",
        "wsrep_sst_xtrabackup-v2",
        "wsrep_sst_mysqldump",
        "wsrep_sst_backup",
        "wsrep_sst_common",
        "wsrep_sst_clone",
        "WSREP_SST_RSYNC",
    ] {
        refused_as_running_others(name);
    }
}

/// The server itself, its safe helper and the router, by every name they go by: `mysqld -b` names the folder
/// its plugins load from and `--early-plugin-load` a plugin, the router's configuration names
/// its plugins' folder, and the keyring migration loads components from a folder it is told
/// (round 2 of #5990's review).
#[test]
fn the_server_its_helper_the_router_and_the_keyring_migration_run_what_they_are_told_so_are_offered_no_run_grant()
 {
    for name in [
        "mysqld",
        "mysqld-debug",
        "mariadbd",
        // runs the command its arguments name, as the user they name (round 3 of #5990's review)
        "mysqld_safe_helper",
        "mariadbd-safe-helper",
        "mysqlrouter",
        "mysqlrouter_plugin_info",
        "mysqlrouter_passwd",
        "mysqlrouter_keyring",
        "mysql_migrate_keyring",
    ] {
        refused_as_running_others(name);
    }
}

// #5994, from #5990's reviews: the MySQL-adjacent tools no row read. Each is refused whole, as
// #5931's were, and the clients stay offered
// (`the_forms_a_run_grant_refuses_by_name_leave_curl_and_the_usual_tools_offered`).

/// mydumper runs a command on each file it writes (`--exec`, `--exec-per-thread`), and myloader
/// one on each file it reads (`--exec-per-thread`). Either may be named in a defaults file the
/// command line never shows.
#[test]
fn mydumper_and_myloader_run_the_command_their_flags_or_files_name_so_they_are_offered_no_run_grant()
 {
    for name in ["mydumper", "myloader", "MYDUMPER"] {
        refused_as_running_others(name);
    }
}

/// Percona Toolkit's tools, by their prefix: many run code their flags name
/// (`pt-online-schema-change --plugin` loads a Perl file, `pt-stalk --plugin` sources a shell
/// file, `pt-kill --execute-command` runs a command, `pt-query-digest --filter` evaluates Perl),
/// and any flag may come from a configuration file the command line never shows. Releases add
/// more.
#[test]
fn a_percona_toolkit_tool_by_its_prefix_is_offered_no_run_grant() {
    for name in [
        "pt-online-schema-change",
        "pt-archiver",
        "pt-stalk",
        "pt-table-checksum",
        "pt-table-sync",
        "pt-query-digest",
        "pt-kill",
        "pt-heartbeat",
        "pt-config-diff",
        "pt-pmp",
        "PT-ARCHIVER",
    ] {
        refused_as_running_others(name);
    }
}

/// Galera's helpers: `galera_recovery` has a shell evaluate its arguments into the server's
/// command line, `galera_new_cluster` has the service manager restart whatever unit it names,
/// Percona XtraDB Cluster's `garbd` runs the scripts its `--recv-script` and `--post-recv-script`
/// name, and `garb-systemd` runs `garbd`.
#[test]
fn galeras_helpers_run_the_server_or_a_script_they_name_so_they_are_offered_no_run_grant() {
    for name in [
        "galera_recovery",
        "galera_new_cluster",
        "garbd",
        "garb-systemd",
    ] {
        refused_as_running_others(name);
    }
}

/// The server's systemd helpers (#6109): `mysqld_bootstrap` has the service manager start the
/// server's unit with `--wsrep-new-cluster` (`scripts/systemd/mysqld_bootstrap.in`), as
/// `galera_new_cluster` does; `mysqld_pre_systemd` runs the server's `--initialize` with what its
/// settings name; and Percona XtraDB Cluster's packages ship `mysql-systemd`, whose `start-pre`
/// does the same and whose `galera-recovery` has a shell evaluate the server's command line
/// (`build-ps/rpm/mysql-systemd`, round 1 of the review). Their `mysql` prefix took them to the
/// tools' row, which reads no flag or action any of them takes.
#[test]
fn the_servers_systemd_helpers_start_the_server_so_they_are_offered_no_run_grant() {
    for name in ["mysqld_bootstrap", "mysqld_pre_systemd", "mysql-systemd"] {
        refused_as_running_others(name);
    }
}

/// `mariadb-report`, shipped with innotop in MariaDB's client on Debian, and `mysqlreport`, the
/// name its compat links give it: its `--outfile` value goes unquoted to a shell
/// (`` `mv $tmpfile $op{outfile}` ``, `debian/additions/mariadb-report`), and its `--email` pipes
/// to `sendmail` (round 1 of the review). Its `mariadb` prefix took it to the tools' row, which
/// reads neither.
#[test]
fn mariadb_report_has_a_shell_run_its_outfile_so_it_is_offered_no_run_grant() {
    for name in ["mariadb-report", "mysqlreport"] {
        refused_as_running_others(name);
    }
}

/// Percona XtraDB Cluster's health checks (round 1 of the review): `clustercheck` hands the
/// client its sixth argument (the fourth after a leading `-`) as
/// `--defaults-extra-file` (`scripts/clustercheck.sh`), the form the client's row refuses, and
/// `pyclustercheck`'s `--conf` names the file of settings its client library reads. Neither name
/// starts with `mysql` or `mariadb`, so no row read them.
#[test]
fn pxcs_cluster_checks_hand_the_client_a_file_so_they_are_offered_no_run_grant() {
    for name in ["clustercheck", "pyclustercheck"] {
        refused_as_running_others(name);
    }
}

/// The slow log's summary and the row finder (round 3 of the review): both read the files they
/// are named with Perl's `<>` (MariaDB's `scripts/mysqldumpslow.sh`,
/// `scripts/mysql_find_rows.sh`), whose open runs a name ending in `|` as a shell command. Their
/// prefix took them to the tools' row, which reads no operand.
#[test]
fn the_perl_log_readers_run_a_file_name_ending_in_a_pipe_so_they_are_offered_no_run_grant() {
    for name in [
        "mysqldumpslow",
        "mariadb-dumpslow",
        "mysql_find_rows",
        "mariadb-find-rows",
    ] {
        refused_as_running_others(name);
    }
}

/// innotop, shipped with MariaDB's client on Debian (#6109): `-c` names the configuration file it
/// reads, whose `plugin_dir` names the folder whose `.pm` files it `require`s, as its `[plugins]`
/// section lists them. Its name starts with neither `mysql` nor `mariadb`, so no row read it.
#[test]
fn innotop_loads_the_perl_plugins_its_configuration_names_so_it_is_offered_no_run_grant() {
    refused_as_running_others("innotop");
}

/// `mysqlaccess` (`mariadb-access` on Debian, #6185): it appends `--host=` and its `--rhost`
/// value to the client's command line and splits the line on white space into the client's
/// arguments (MariaDB's `scripts/mysqlaccess.sh`, its `Connect` and its dump of the grant
/// tables), so a host value of `x --plugin-dir=… --default-auth=…` has the client load a plugin
/// from a folder it names. Its prefix took it to the tools' row, which reads no `-H`.
#[test]
fn mysqlaccess_splits_its_host_into_the_clients_flags_so_it_is_offered_no_run_grant() {
    for name in ["mysqlaccess", "mariadb-access"] {
        refused_as_running_others(name);
    }
}

/// The tools that build a Perl DBI address from their host value (#6185): `mytop`,
/// `mysql_convert_table_format` and `mysql_setpermission` join it, unquoted, into a
/// `DBI:MariaDB:` or `DBI:mysql:` address (MariaDB's `scripts/mytop.sh`,
/// `scripts/mysql_convert_table_format.sh`, `scripts/mysql_setpermission.sh`), whose driver
/// splits it at each `:` or `;` into its attributes (`parse_dsn` in DBD::MariaDB, `_OdbcParse` in
/// DBD::mysql). A host of `x;mariadb_read_default_file=…` names a file of settings the client
/// library reads, which can name a plugin folder and an authentication plugin from it. None of
/// the client's own flags is in the command, so no row read the value.
#[test]
fn the_tools_that_put_their_host_in_a_dbi_address_are_offered_no_run_grant() {
    for name in [
        "mytop",
        "mysql_convert_table_format",
        "mariadb-convert-table-format",
        "mysql_setpermission",
        "mariadb-setpermission",
    ] {
        refused_as_running_others(name);
    }
}

/// Percona Server's `ps-admin` (#6185): its `--config-file` is `source`d as a shell file, and its
/// `--user` and `--host` values are expanded unquoted into the client's command line
/// (`scripts/ps-admin.sh`, Percona Server 8.0). Its name starts with neither `mysql` nor
/// `mariadb`, so no row read it.
#[test]
fn ps_admin_runs_its_configuration_file_as_a_shell_so_it_is_offered_no_run_grant() {
    refused_as_running_others("ps-admin");
}

/// The test suite's runners (#6187), which MySQL's test package and Percona XtraDB Cluster's
/// ship in the programs' folder: `mysqlxtest` reads a script, from its `-f` or its input, whose
/// `-->system` and `-->system_in_background` run a shell command
/// (`mysql-test/suite/x/r/mysqlxtest_help.result`, github.com/mysql/mysql-server, 8.0), as
/// `mysqltest`'s do, and `mysqltest_safe_process` runs the program and arguments it is given
/// after `--` (`execvp`, `mysql-test/lib/My/SafeProcess/safe_process.cc`). Their prefix took
/// them to the tools' row, which reads neither the script nor `--`.
#[test]
fn the_test_suites_runners_run_a_command_they_are_handed_so_they_are_offered_no_run_grant() {
    for name in ["mysqlxtest", "mysqltest_safe_process"] {
        refused_as_running_others(name);
    }
}

/// `myrocks_hotbackup`, in Debian's `mariadb-plugin-rocksdb` (#6187): its
/// `--extra_wdt_sender_options` is formatted into a command `os.system` runs, its
/// `--destination` is ssh's first argument, and the file names it streams are joined to a
/// command `os.system` runs (`storage/rocksdb/myrocks_hotbackup.py`, MariaDB 11.4). Its name
/// starts with neither `mysql` nor `mariadb`, so no row read it.
#[test]
fn myrocks_hotbackup_hands_a_shell_its_options_so_it_is_offered_no_run_grant() {
    refused_as_running_others("myrocks_hotbackup");
}

/// `rcmysql`, which Percona XtraDB Cluster's packages built without systemd link to the server's
/// init script (`build-ps/percona-xtradb-cluster.spec`): `rcmysql start` starts the server.
#[test]
fn rcmysql_starts_the_server_so_it_is_offered_no_run_grant() {
    refused_as_running_others("rcmysql");
}

/// `service`, which MySQL's own SLES packages link `rcmysql` to (`packaging/rpm-sles/mysql.spec.in`,
/// first review of #6318): it runs the init script it is named, `/etc/init.d/<name>`, or has
/// `systemctl` act on the unit (Debian's init-system-helpers, `script/service`). Every `rc*` link
/// to it resolves to `service`, so refusing it refuses them.
#[test]
fn service_runs_the_init_script_it_is_named_so_it_is_offered_no_run_grant() {
    refused_as_running_others("service");
}

/// Percona Server's `mysql_keyring_encryption_test`, shipped with the test runners
/// (`build-ps/percona-server.spec`, second review of #6318): it loads
/// `file://<--component_dir>/<--keyring>` as a component, a shared library
/// (`components/test/keyring_encryption_test/options.cc`, `components.cc`), as
/// `mysql_migrate_keyring` does. Its prefix took it to the tools' row, which reads neither flag.
#[test]
fn the_keyring_encryption_test_loads_the_component_it_is_named_so_it_is_offered_no_run_grant() {
    refused_as_running_others("mysql_keyring_encryption_test");
}

/// A link to a service's init script, by whatever name (#6187): the script's own file name is the
/// service's, `mysql` for the server's, which the client's row reads as the client's, so the
/// script is judged by its folder. It starts and stops the service it is for, which no row reads.
#[cfg(unix)]
#[test]
fn a_link_to_an_init_script_is_offered_no_run_grant_whatever_its_name() {
    use std::os::unix::fs::PermissionsExt;
    let project = project();
    let asks = Asks::new(project.path());
    let system = tempfile::tempdir().unwrap();
    let system = system.path().canonicalize().unwrap();
    let (scripts, sbin) = (system.join("etc/init.d"), system.join("sbin"));
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::create_dir_all(&sbin).unwrap();
    let script = scripts.join("mysql");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::os::unix::fs::symlink(&script, sbin.join("rcdatabase")).unwrap();
    let mut want = printenv(project.path(), &["."]);
    want.command = vec!["rcdatabase".to_owned(), "start".to_owned()];
    // Judged as a system folder is, outside the temporary folders.
    crate::sandbox::tools::TEMPS_HERE.with(|temps| *temps.borrow_mut() = Some(Vec::new()));
    let frames = run_searching(
        &the_chat(project.path()),
        want,
        &asks,
        Duration::ZERO,
        vec![sbin],
    );
    crate::sandbox::tools::TEMPS_HERE.with(|temps| *temps.borrow_mut() = None);
    assert!(asks.open_runs().is_empty(), "{frames:?}");
    assert!(
        refusal(&frames).0.contains("runs other programs"),
        "{frames:?}"
    );
}

/// A link that reaches an init script's place and points on out of it (#6321), as Ubuntu's
/// Upstart jobs were laid out (`/etc/init.d/<job> -> /lib/init/upstart-job`): what runs sits
/// outside `init.d`, under a name no row reads, but the link in `init.d` is the service's script.
#[cfg(unix)]
#[test]
fn a_link_through_an_init_script_that_points_on_out_of_init_d_is_offered_no_run_grant() {
    use std::os::unix::fs::PermissionsExt;
    let project = project();
    let asks = Asks::new(project.path());
    let system = tempfile::tempdir().unwrap();
    let system = system.path().canonicalize().unwrap();
    let (scripts, lib, sbin) = (
        system.join("etc/init.d"),
        system.join("lib/init"),
        system.join("sbin"),
    );
    for folder in [&scripts, &lib, &sbin] {
        std::fs::create_dir_all(folder).unwrap();
    }
    let job = lib.join("upstart-job");
    std::fs::write(&job, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&job, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::os::unix::fs::symlink(&job, scripts.join("database")).unwrap();
    std::os::unix::fs::symlink(scripts.join("database"), sbin.join("rcdatabase")).unwrap();
    let mut want = printenv(project.path(), &["."]);
    want.command = vec!["rcdatabase".to_owned(), "start".to_owned()];
    // Judged as a system folder is, outside the temporary folders.
    crate::sandbox::tools::TEMPS_HERE.with(|temps| *temps.borrow_mut() = Some(Vec::new()));
    let frames = run_searching(
        &the_chat(project.path()),
        want,
        &asks,
        Duration::ZERO,
        vec![sbin],
    );
    crate::sandbox::tools::TEMPS_HERE.with(|temps| *temps.borrow_mut() = None);
    assert!(asks.open_runs().is_empty(), "{frames:?}");
    assert!(
        refusal(&frames).0.contains("runs other programs"),
        "{frames:?}"
    );
}

/// `systemctl` (#6320): `systemctl link <path>` makes a unit of the file it is named, which
/// `start` then runs, and `import-environment <VAR>` copies a value the run was handed into the
/// service manager's environment, where `show-environment` reads it back and every unit started
/// after gets it (`src/systemctl/systemctl.c` and `systemctl-set-environment.c`, systemd v257).
/// `launchctl` is refused for the same reason.
#[test]
fn systemctl_starts_a_unit_of_a_file_it_is_named_so_it_is_offered_no_run_grant() {
    refused_as_running_others("systemctl");
}

/// dpkg's `start-stop-daemon` (#6320) `execv`s the program its `--startas`, or else its `--exec`,
/// names (`utils/start-stop-daemon.c`, dpkg 1.22.11).
#[test]
fn start_stop_daemon_runs_the_program_it_is_named_so_it_is_offered_no_run_grant() {
    refused_as_running_others("start-stop-daemon");
}

/// runit's `sv` runs the `check` script of the service folder it is named, which a path names as
/// well as a service's name does, and `runsv` runs the `run` script of the folder it is named
/// (`src/sv.c`, `src/runsv.c`, runit) (#6320, its second review).
#[test]
fn runits_sv_and_runsv_run_the_scripts_of_a_folder_they_are_named_so_they_are_offered_no_run_grant()
{
    for name in ["sv", "runsv"] {
        refused_as_running_others(name);
    }
}

/// `supervisorctl`'s `-c` file names a `[ctlplugin:…]` whose `supervisor.ctl_factory` it imports and
/// calls with the section's settings (`supervisor/options.py`, `supervisorctl.py`, supervisor
/// 4.2.5) (#6320, its second review).
#[test]
fn supervisorctl_loads_the_plugin_its_settings_file_names_so_it_is_offered_no_run_grant() {
    refused_as_running_others("supervisorctl");
}

/// OpenRC (#6320, its second review): `openrc --service <path>` `execv`s the path it is given
/// (`src/openrc/rc.c`, with `rc_service_resolve` in `src/librc/librc.c`, OpenRC 0.55), and
/// `openrc-run <script>` has its shell helper source the script (`sh/openrc-run.sh.in`).
#[test]
fn openrc_and_openrc_run_run_the_script_they_are_named_so_they_are_offered_no_run_grant() {
    for name in ["openrc", "openrc-run"] {
        refused_as_running_others(name);
    }
}

/// `chkconfig` hands `systemctl` the unit it is named (`forwardSystemd`, `chkconfig.c`,
/// chkconfig 1.30), and `deb-systemd-helper` has it preset the unit it is named
/// (`script/deb-systemd-helper`, init-system-helpers): `systemctl` is refused (#6320, its second
/// review).
#[test]
fn chkconfig_and_deb_systemd_helper_hand_systemctl_a_unit_so_they_are_offered_no_run_grant() {
    for name in ["chkconfig", "deb-systemd-helper"] {
        refused_as_running_others(name);
    }
}

/// The client built with the server inside it: what `--server-arg` hands the embedded server
/// (`--plugin-load`, `--plugin-dir`) loads code from a file it names.
#[test]
fn the_client_with_the_server_embedded_loads_what_it_is_told_so_it_is_offered_no_run_grant() {
    for name in [
        "mysql_embedded",
        "mariadb-embedded",
        // the client test built the same way, which takes `--server-arg` as well (round 1 of the
        // review)
        "mysql_client_test_embedded",
        "mariadb-client-test-embedded",
    ] {
        refused_as_running_others(name);
    }
}

/// gh-ost, the online schema change used with a key as Percona's is: `--hooks-path` names a
/// folder whose `gh-ost-on-*` programs it runs (round 1 of the review).
#[test]
fn gh_ost_runs_the_hooks_of_a_folder_it_names_so_it_is_offered_no_run_grant() {
    refused_as_running_others("gh-ost");
}
