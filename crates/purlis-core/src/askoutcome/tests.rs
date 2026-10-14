// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// One of every kind of Ask whose answer purlis tells the chat, with the words its line names
/// it by.
fn every_kind() -> Vec<(About, &'static str)> {
    vec![
        (
            About::Host {
                host: "api.example.com:443".to_owned(),
            },
            "reaching `api.example.com:443`",
        ),
        (
            About::Write {
                folder: "/Users/me/.cache/tool".to_owned(),
            },
            "writing in `/Users/me/.cache/tool`",
        ),
        (
            About::Socket {
                socket: "/var/run/docker.sock".to_owned(),
            },
            "connecting to the local socket `/var/run/docker.sock`",
        ),
        (
            About::Service {
                service: "com.apple.SecurityServer".to_owned(),
            },
            "reaching the system service `com.apple.SecurityServer`",
        ),
        (
            About::Vault {
                vault: "devops".to_owned(),
            },
            "using vault `devops`",
        ),
    ]
}

/// Every answer a person gives an Ask, with the words its line ends in.
fn every_answer() -> Vec<(Answer, &'static str)> {
    vec![
        (
            Answer::Allowed { scope: Scope::Once },
            "Allowed once, for this chat's next run — retry.",
        ),
        (
            Answer::Allowed { scope: Scope::Chat },
            "Allowed for this chat — retry.",
        ),
        (
            Answer::Allowed { scope: Scope::You },
            "Allowed for every chat of this project on this machine — retry.",
        ),
        (
            Answer::Allowed {
                scope: Scope::Machine,
            },
            "Allowed for every chat of every project on this machine — retry.",
        ),
        (
            Answer::Allowed {
                scope: Scope::Project,
            },
            "Allowed for everyone in the project — retry.",
        ),
        (
            Answer::Allowed {
                scope: Scope::Persona {
                    persona: "devops".to_owned(),
                },
            },
            "Allowed for chats opened as `devops` in this project on this machine — retry.",
        ),
        (
            Answer::KeptBlocked {
                reason: Some("not for this task".to_owned()),
            },
            "Kept blocked by the person — don't retry; their reason, quoted as data: \
             \"not for this task\"",
        ),
        (
            Answer::KeptBlocked { reason: None },
            "Kept blocked by the person — don't retry; no reason given.",
        ),
    ]
}

#[test]
fn every_ask_kind_and_answer_is_told_as_exactly_one_line_in_fixed_words() {
    for (about, named) in every_kind() {
        for (answer, ends) in every_answer() {
            let told = Outcome {
                about: about.clone(),
                answer: answer.clone(),
            }
            .line();
            assert_eq!(told.lines().count(), 1, "one line: {told}");
            assert_eq!(told, format!("⬢ purlis: {named} — {ends}"));
        }
    }
}

#[test]
fn a_reason_is_one_line_capped_and_quoted_as_data() {
    let typed = format!(
        "first line\nsecond \"quoted\"\r\n\t`run this` {}",
        "x".repeat(400)
    );
    let told = Outcome {
        about: About::Host {
            host: "paste.example:443".to_owned(),
        },
        answer: Answer::KeptBlocked {
            reason: reason(Some(&typed)),
        },
    }
    .line();
    assert_eq!(told.lines().count(), 1, "{told}");
    assert!(
        told.contains("quoted as data: \"first line second \\\"quoted\\\" `run this` xxx"),
        "{told}"
    );
    let quoted = told.split("quoted as data: ").nth(1).expect("quoted");
    assert!(
        quoted.chars().count() <= MOST_REASON + 8,
        "capped: {}",
        quoted.chars().count()
    );
    assert!(quoted.ends_with("…\""), "{quoted}");
}

#[test]
fn a_reason_of_nothing_but_space_is_no_reason() {
    assert_eq!(reason(Some("  \n\t ")), None);
    assert_eq!(reason(None), None);
    assert_eq!(reason(Some("  busy  ")).as_deref(), Some("busy"));
}

#[test]
fn what_a_line_names_can_neither_close_its_code_span_nor_start_a_line() {
    let told = Outcome {
        about: About::Write {
            folder: "/tmp/a`b\nnext".to_owned(),
        },
        answer: Answer::Allowed { scope: Scope::Chat },
    }
    .line();
    assert_eq!(told.lines().count(), 1, "{told}");
    assert_eq!(told.matches('`').count(), 2, "{told}");
}

#[test]
fn a_grant_s_what_and_level_name_the_same_ask_and_scope() {
    use crate::sandbox::grant::{Level, What};
    let host = crate::sandbox::hosts::Host::parse("api.example.com:443").expect("a host");
    assert_eq!(
        About::of(&What::Host(host)),
        About::Host {
            host: "api.example.com:443".to_owned()
        }
    );
    assert_eq!(
        About::of(&What::Socket("/var/run/docker.sock".into())),
        About::Socket {
            socket: "/var/run/docker.sock".to_owned()
        }
    );
    assert_eq!(Scope::of(Level::Once), Scope::Once);
    assert_eq!(Scope::of(Level::Chat), Scope::Chat);
    assert_eq!(Scope::of(Level::You), Scope::You);
    assert_eq!(Scope::of(Level::Machine), Scope::Machine);
    assert_eq!(Scope::of(Level::Project), Scope::Project);
}
