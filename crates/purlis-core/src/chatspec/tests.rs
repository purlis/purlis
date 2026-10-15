// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::ffi::OsString;

use super::*;
use crate::engine::Size;

const SIZE: Size = Size {
    columns: 100,
    rows: 30,
};

fn value(spec: &Spec, name: &str) -> Option<String> {
    spec.env
        .iter()
        .rev()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.to_string_lossy().into_owned())
}

fn app() -> Vec<(OsString, OsString)> {
    vec![
        ("PATH".into(), "/usr/bin:/bin".into()),
        ("HOME".into(), "/home/me".into()),
        ("SOMETHING_THE_APP_INHERITED".into(), "x".into()),
        ("OP_SERVICE_ACCOUNT_TOKEN".into(), "kept-out".into()),
    ]
}

fn a_chat<'a>(set: &'a [(String, String)], reporting: Option<Reporting<'a>>) -> Terminal<'a> {
    Terminal {
        program: "/bin/zsh",
        args: &[],
        cwd: Some("/home/me/project"),
        size: SIZE,
        starting: crate::chatenv::Starting {
            harness: None,
            operator: &[],
            strip: &[],
            set,
            git_hooks: None,
            declared: &[],
        },
        number: 7,
        reporting,
    }
}

#[test]
fn a_chat_starts_from_an_empty_environment_with_what_the_app_keeps_and_its_own() {
    let set = [("PURLIS_ROOT".to_owned(), "/home/me/project".to_owned())];
    let spec = spec(app(), &a_chat(&set, None));

    assert!(
        spec.env_clear,
        "nothing the app inherited is the chat's unless kept"
    );
    assert_eq!(spec.program, "/bin/zsh");
    assert_eq!(
        spec.cwd.as_deref(),
        Some(std::path::Path::new("/home/me/project"))
    );
    assert_eq!(spec.size, SIZE);
    assert_eq!(value(&spec, "HOME").as_deref(), Some("/home/me"));
    assert_eq!(value(&spec, "SOMETHING_THE_APP_INHERITED"), None);
    assert_eq!(value(&spec, "OP_SERVICE_ACCOUNT_TOKEN"), None);
    assert_eq!(
        value(&spec, "PURLIS_ROOT").as_deref(),
        Some("/home/me/project")
    );
}

#[test]
fn a_chat_is_told_its_number_under_both_names() {
    let spec = spec(app(), &a_chat(&[], None));

    assert_eq!(value(&spec, "PURLIS_SESSION_ID").as_deref(), Some("7"));
    assert_eq!(value(&spec, "CHARTER_SESSION_ID").as_deref(), Some("7"));
    assert_eq!(value(&spec, "PURLIS_CHAT"), None, "nothing to report to");
}

#[test]
fn a_chat_with_somewhere_to_report_is_told_where_and_its_token() {
    let reporting = Reporting {
        socket: std::path::Path::new("/home/me/project/.purlis/app/hooks.sock"),
        token: "the-chats-own",
    };
    let spec = spec(app(), &a_chat(&[], Some(reporting)));

    assert_eq!(
        value(&spec, "PURLIS_HOOK_SOCKET").as_deref(),
        Some("/home/me/project/.purlis/app/hooks.sock")
    );
    assert_eq!(value(&spec, "PURLIS_CHAT").as_deref(), Some("7"));
    assert_eq!(
        value(&spec, "PURLIS_CHAT_TOKEN").as_deref(),
        Some("the-chats-own")
    );
}

fn a_launch<'a>(harness: Option<&'a str>, set: &'a [(String, String)]) -> Launched<'a> {
    Launched {
        program: "/usr/local/bin/claude",
        args: &[],
        cwd: Some("/home/me/project"),
        set,
        strip: &[],
        pass: &[],
        declared: &[],
        harness,
        git_hooks: None,
        reporting: None,
    }
}

#[test]
fn a_launch_the_window_composed_is_started_as_the_window_would_start_it() {
    let set = [("PURLIS_ROOT".to_owned(), "/home/me/project".to_owned())];
    let launch = a_launch(Some("claude"), &set);

    let from_the_host = launched(app(), &launch, 7, SIZE).expect("it composes");
    let in_the_window = spec(
        app(),
        &Terminal {
            program: "/usr/local/bin/claude",
            args: &[],
            cwd: Some("/home/me/project"),
            size: SIZE,
            starting: crate::chatenv::Starting {
                harness: Some(crate::harness::Harness::ClaudeCode),
                operator: &[],
                strip: &[],
                set: &set,
                git_hooks: None,
                declared: &[],
            },
            number: 7,
            reporting: None,
        },
    );

    assert_eq!(from_the_host.program, in_the_window.program);
    assert_eq!(from_the_host.env, in_the_window.env);
    assert_eq!(from_the_host.cwd, in_the_window.cwd);
}

#[test]
fn a_launch_of_a_harness_this_build_does_not_know_is_refused() {
    let refused = launched(
        app(),
        &a_launch(Some("a-harness-from-the-future"), &[]),
        1,
        SIZE,
    )
    .unwrap_err();

    assert!(refused.contains("a-harness-from-the-future"), "{refused}");
}
