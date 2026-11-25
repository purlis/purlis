// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #7596: a shipped binary aborts on any panic before it unwinds (`purlis_core::panicend::fatal`),
//! so `catch_unwind` around `graphql-parser` protects nothing there. The document that panics
//! the parser (#7589) must be refused *before* the parser runs. The child installs `fatal()`,
//! as every shipped `main` does, and sends the document through the forge proxy.

use purlis_core::forge::proxy::Refusal;

use crate::support::forge_proxy::{Scene, graphql, is_refused};

/// Set in the child run alone.
const CHILD: &str = "PURLIS_TEST_GRAPHQL_PANIC_CHILD";

/// The child: with the shipped abort hook installed, sends each document that panicked the
/// parser. In any other run it does nothing.
#[test]
fn graphql_child() {
    purlis_core::unsteered!();
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    purlis_core::panicend::fatal();
    let s = Scene::single();
    for lines in [
        &[" a", "\u{3000}", " b"][..],
        &[" a", "\u{3000}\u{3000}b"][..],
        &["\u{3000}a", " b"][..],
        &[" a", "\r\u{3000}", " b"][..],
    ] {
        let q = format!(
            "mutation{{ createIssue(input:{{repositoryId:\"R_acme_app\", title:\"t\", body:\"\"\"x\r\n{}\n\"\"\"}}){{ clientMutationId }} }}",
            lines.join("\r\n")
        );
        let out = s.send(
            "POST",
            "api.github.com",
            "/graphql",
            &graphql(&q, serde_json::json!({})),
        );
        assert!(
            is_refused(&out, Refusal::GraphqlParse),
            "{lines:?}: {out:?}"
        );
    }
    // #7547: the merge and enqueue documents are read by a parse of their own.
    for mutation in ["mergePullRequest", "enqueuePullRequest"] {
        let q = format!(
            "mutation{{ {mutation}(input:{{pullRequestId:\"PR_kwDO_1\", commitBody:\"\"\"x\r\n a\r\n\u{3000}\r\n b\r\n\"\"\"}}){{ clientMutationId }} }}"
        );
        let out = s.send(
            "POST",
            "api.github.com",
            "/graphql",
            &graphql(&q, serde_json::json!({})),
        );
        assert!(
            is_refused(&out, Refusal::GraphqlParse),
            "{mutation}: {out:?}"
        );
    }
    // Unwinding to here means no abort: the process survived every document.
    std::process::exit(0);
}

#[test]
fn a_shipped_binary_refuses_a_block_string_the_parser_panics_on_and_does_not_abort() {
    purlis_core::unsteered!();
    let ran = purlis_core::forklock::output(
        std::process::Command::new(std::env::current_exe().expect("the test binary"))
            .args([
                "a_shipped_binary_survives_a_graphql_block_string_the_parser_panics_on::graphql_child",
                "--exact",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD, "1"),
    )
    .expect("the child runs");
    let said = String::from_utf8_lossy(&ran.stderr);
    assert!(
        ran.status.success(),
        "the child ended with {:?}: {said}",
        ran.status
    );
}
