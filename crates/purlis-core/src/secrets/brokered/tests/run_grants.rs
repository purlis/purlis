// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Run grant** (ADR 0112, #5239): a `secret exec` line in, a run or a notice out.

use super::*;
use crate::secrets::ask::Asks;
use crate::secrets::ask::run::{OpenRun, RunAnswer};

fn window() -> crate::harness::asks::Answerer {
    crate::harness::asks::Answerer::admitted(crate::harness::asks::Admitted::LocalUi).unwrap()
}

/// `wanted` run with `asks`, on a search path of the system's own folders, as the app's would be.
fn run_with(asker: &Asker, wanted: Wanted, asks: &Asks, within: Duration) -> Vec<Frame> {
    crate::sandbox::tools::SEARCH_HERE
        .with(|dirs| *dirs.borrow_mut() = Some(vec!["/usr/bin".into(), "/bin".into()]));
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
        printenv(project.path(), &["X"]),
        &asks,
        RunAnswer::ForChat,
    );
    let other = run_with(
        &the_chat(project.path()),
        printenv(project.path(), &["HOME"]),
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

/// Policy says `said` for this test, and no longer once it ends.
struct Policy;

impl Policy {
    fn says(said: &str) -> Self {
        crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::parse(
            said,
            Path::new("/etc/purlis/policy.json"),
        ));
        Self
    }
}

impl Drop for Policy {
    fn drop(&mut self) {
        crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::none());
    }
}

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
        printenv(project.path(), &["X"]),
        &asks,
        RunAnswer::ForChat,
    );
    let _policy = Policy::says(r#"{"sandbox": {"run-grants": false}}"#);
    let frames = run_with(
        &the_chat(project.path()),
        printenv(project.path(), &["X"]),
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
    assert!(asks.open_runs().is_empty(), "no Allow");
    let open = asks.open();
    assert_eq!(open.len(), 1, "Fill");
    assert_eq!(open[0].key, "NOT_THERE_YET");
    assert!(
        open[0]
            .no_allow
            .as_deref()
            .is_some_and(|why| why.contains("policy")),
        "{open:?}"
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
    let want = || printenv(project.path(), &["X"]);
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
    assert_eq!(declared[0].name, "printenv");
    let again = run_with(
        &the_chat(project.path()),
        printenv(project.path(), &["HOME"]),
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
    let mut want = printenv(root, &["X"]);
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
    answered(root, on_another_vault(root), &asks, RunAnswer::ForChat);
    let mut other = the_chat(root);
    other.chat = 99;
    let frames = run_with(&other, on_another_vault(root), &asks, Duration::ZERO);
    assert_eq!(refusal(&frames).1, WAITING_ON_THE_PERSON, "{frames:?}");
    no_vault_was_kept(root);
}

#[test]
fn revoking_a_chat_grant_ends_its_use_of_the_vault() {
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let chat = the_chat(root);
    answered(root, on_another_vault(root), &asks, RunAnswer::ForChat);
    let id = asks.given_runs()[0].id;
    asks.revoke_run(id, window()).unwrap();
    let frames = run_with(&chat, on_another_vault(root), &asks, Duration::ZERO);
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
fn a_key_a_vault_opened_by_a_grant_lacks_is_never_asked_for() {
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let mut want = on_another_vault(root);
    want.env = vec!["X=NOT_THERE".into()];
    let frames = answered(root, want, &asks, RunAnswer::Once);
    assert!(asks.open().is_empty(), "no Fill: {frames:?}");
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
    assert_eq!(asks.open().len(), 1, "Fill");
    assert!(asks.open_runs().is_empty(), "no Allow");
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
        printenv(project.path(), &["X"]),
        &asks,
        RunAnswer::ForChat,
    );
    let long = "x".repeat(MOST_COMMAND_SHOWN + 1);
    let frames = run_with(
        &the_chat(project.path()),
        printenv(project.path(), &[&long]),
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
