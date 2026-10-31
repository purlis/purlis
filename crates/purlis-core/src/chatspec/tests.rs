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
        prompt_key: None,
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
        prompt_key: None,
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
            prompt_key: None,
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

/// #2174: the session protocol's `launch`, as both binaries' hosts are handed it, is started
/// by its words: one adapter, the same spec the core composes from its parts.
#[test]
fn a_launch_as_the_session_protocol_carries_it_is_started_by_its_words() {
    use purlis_session_protocol::session;
    let set = [("PURLIS_ROOT".to_owned(), "/home/me/project".to_owned())];
    let wired = session::Launch {
        program: "/usr/local/bin/claude".to_owned(),
        args: vec!["--resume".to_owned(), "abc".to_owned()],
        cwd: Some("/home/me/project".to_owned()),
        set: set.to_vec(),
        strip: vec!["MY_VAULT_TOKEN".to_owned()],
        pass: vec!["EDITOR".to_owned()],
        declared: Vec::new(),
        harness: Some("claude".to_owned()),
        git_hooks: Some("/home/me/.purlis/git-hooks".to_owned()),
        reporting: Some(session::Reporting {
            socket: "/home/me/project/.purlis/app/hooks.sock".to_owned(),
            token: "the-chats-own".to_owned(),
        }),
        confinement: None,
        prompt_key: session::PromptKey::of("the-chats-prompt-key"),
    };

    let started = from_launch(app(), &wired, 7, SIZE).expect("it composes");
    let args = ["--resume".to_owned(), "abc".to_owned()];
    let strip = ["MY_VAULT_TOKEN".to_owned()];
    let pass = ["EDITOR".to_owned()];
    let by_its_parts = launched(
        app(),
        &Launched {
            program: "/usr/local/bin/claude",
            args: &args,
            cwd: Some("/home/me/project"),
            set: &set,
            strip: &strip,
            pass: &pass,
            declared: &[],
            harness: Some("claude"),
            git_hooks: Some("/home/me/.purlis/git-hooks"),
            reporting: Some(Reporting {
                socket: std::path::Path::new("/home/me/project/.purlis/app/hooks.sock"),
                token: "the-chats-own",
            }),
            prompt_key: Some("the-chats-prompt-key"),
        },
        7,
        SIZE,
    )
    .expect("it composes");

    assert_eq!(started.program, "/usr/local/bin/claude");
    assert_eq!(started.args, by_its_parts.args);
    assert_eq!(started.args.len(), 2);
    assert_eq!(started.env, by_its_parts.env);
    assert_eq!(
        value(&started, "PURLIS_CHAT_TOKEN").as_deref(),
        Some("the-chats-own")
    );
    // #3369: the chat's prompt key is in its environment, and never on its line.
    assert_eq!(
        value(&started, "PURLIS_PROMPT_KEY").as_deref(),
        Some("the-chats-prompt-key")
    );
    assert!(
        !started
            .args
            .iter()
            .any(|word| word.to_string_lossy().contains("the-chats-prompt-key"))
    );
}

#[test]
fn a_chat_on_purlis_s_own_wrap_starts_it_with_only_what_it_carries_for_the_chat() {
    let set = [
        ("PURLIS_ROOT".to_owned(), "/home/me/project".to_owned()),
        ("LD_PRELOAD".to_owned(), "/home/me/project/x.so".to_owned()),
    ];
    let reporting = || {
        Some(Reporting {
            socket: std::path::Path::new("/run/purlis/hooks.sock"),
            token: "chat-token",
        })
    };
    let args = purlis_linux_sandbox::line(
        "(version 1)\n",
        "/usr/bin/codex".to_owned(),
        vec!["--a".to_owned()],
    );
    let wrapped = spec(
        app(),
        &Terminal {
            program: "/opt/purlis/bin/purlis",
            args: &args,
            ..a_chat(&set, reporting())
        },
    );
    let plain = spec(app(), &a_chat(&set, reporting()));

    assert!(wrapped.env_clear);
    assert!(
        wrapped.env.iter().all(|(name, _)| {
            name.to_str()
                .is_some_and(|name| name.starts_with(purlis_linux_sandbox::carried::COUNT))
        }),
        "the wrap's own environment names nothing a loader or a library reads: {:?}",
        wrapped.env
    );
    // What the chat's program is given: the terminal's type, which its terminal set on the
    // program before, then what it would have started with unwrapped, in that order.
    let given = purlis_linux_sandbox::carried::read(wrapped.env).expect("carried");
    let mut meant = vec![(OsString::from("TERM"), OsString::from("xterm-256color"))];
    meant.extend(plain.env);
    assert_eq!(given, meant);
}

#[test]
fn a_chat_whose_words_only_look_like_the_wrap_s_is_started_as_it_is() {
    let args = vec!["_sandbox".to_owned(), "--help".to_owned()];
    let spec = spec(
        app(),
        &Terminal {
            program: "/usr/bin/tool",
            args: &args,
            ..a_chat(&[], None)
        },
    );
    assert_eq!(value(&spec, "PATH").as_deref(), Some("/usr/bin:/bin"));
}

#[test]
fn what_a_profile_hands_a_chat_on_purlis_s_own_wrap_is_carried_to_its_program() {
    let args = purlis_linux_sandbox::line("(version 1)\n", "/usr/bin/codex".to_owned(), Vec::new());
    let set = [("API_KEY".to_owned(), "from-the-chat".to_owned())];
    let wrapped = spec(
        app(),
        &Terminal {
            program: "/opt/purlis/bin/purlis",
            args: &args,
            ..a_chat(&set, None)
        },
    );
    let handed = crate::profilesignin::Handed::of(vec![(
        "API_KEY".to_owned(),
        "from-the-profile".to_owned(),
    )]);
    let wrapped = with_handed(wrapped, &handed);
    assert!(
        wrapped.env.iter().all(|(name, _)| name
            .to_str()
            .is_some_and(|name| name.starts_with(purlis_linux_sandbox::carried::COUNT))),
        "{:?}",
        wrapped.env
    );
    let given = purlis_linux_sandbox::carried::read(wrapped.env).expect("carried");
    let keys: Vec<_> = given.iter().filter(|(name, _)| name == "API_KEY").collect();
    assert_eq!(keys, [&("API_KEY".into(), "from-the-profile".into())]);
}

#[test]
fn what_a_profile_hands_a_chat_is_set_last() {
    let set = [("API_KEY".to_owned(), "from-the-chat".to_owned())];
    let plain = spec(app(), &a_chat(&set, None));
    let handed = crate::profilesignin::Handed::of(vec![(
        "API_KEY".to_owned(),
        "from-the-profile".to_owned(),
    )]);
    let plain = with_handed(plain, &handed);
    assert_eq!(
        value(&plain, "API_KEY").as_deref(),
        Some("from-the-profile")
    );
    assert_eq!(value(&plain, "PATH").as_deref(), Some("/usr/bin:/bin"));
}
