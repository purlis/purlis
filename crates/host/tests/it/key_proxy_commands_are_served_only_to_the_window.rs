// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-055, B-16: a run's key proxy is started and stopped by the window's connection alone. The
//! start sent over a chat's MCP connection (`fleet-mcp`), over the CLI socket (`terminal`), or
//! as any other scope, is refused with no listener started; the same command from the window's
//! connection starts one.

#![cfg(unix)]

use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::session::{self, Answer, Command};

use crate::the_start_command_brings_up_a_listener_through_the_trait::{
    ListensPlainly, hosting_with, linked_as,
};

const RUN: &str = "01J9RUN000000000000000000B";

#[tokio::test]
async fn key_proxy_commands_are_served_only_to_the_window() {
    let proxies = ListensPlainly::default();
    let hosting = hosting_with(proxies.clone());

    let others: Vec<Scope> = Scope::ALL
        .into_iter()
        .filter(|scope| *scope != Scope::LocalUi)
        .collect();
    assert!(others.contains(&Scope::FleetMcp) && others.contains(&Scope::Terminal));
    for scope in others {
        let client = linked_as(&hosting, scope).await;
        for command in [
            Command::StartKeyProxy { run: RUN.into() },
            Command::StopKeyProxy { run: RUN.into() },
        ] {
            let word = command.word();
            let refused = client
                .call(command)
                .await
                .expect_err(&format!("{scope} was served `{word}`"));
            assert_eq!(
                refused.refusal().map(|r| r.code.as_str()),
                Some(session::NOT_ALLOWED),
                "{scope} `{word}`: {refused}"
            );
        }
    }
    assert_eq!(
        *proxies.starts.lock().unwrap(),
        0,
        "a scope other than the window's reached the trait"
    );
    assert!(proxies.listening.lock().unwrap().is_empty());

    let window = linked_as(&hosting, Scope::LocalUi).await;
    let answer = window
        .call(Command::StartKeyProxy { run: RUN.into() })
        .await
        .expect("the window's start is served");
    assert!(
        matches!(answer, Answer::KeyProxyStarted { .. }),
        "{answer:?}"
    );
    assert_eq!(*proxies.starts.lock().unwrap(), 1);
}
