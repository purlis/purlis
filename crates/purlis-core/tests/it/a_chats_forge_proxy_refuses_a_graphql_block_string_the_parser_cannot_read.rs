// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #7589: `graphql-parser` 0.4.1 cuts a block string's lines by byte, so a line of non-ASCII
//! whitespace between indented lines (or a deeper indent of such whitespace) panics on a char
//! boundary. The proxy answers such a document with a refusal, and the next request is read as
//! any other.

use purlis_core::forge::proxy::Refusal;

use crate::support::forge_proxy::{Scene, graphql, is_refused};

/// A document whose one block string holds `lines` after an opening line, each line after a
/// newline, so the parser's indent removal reads them.
fn block_string_document(lines: &[&str]) -> String {
    format!(
        "mutation{{ createIssue(input:{{repositoryId:\"R_acme_app\", title:\"t\", body:\"\"\"x\n{}\n\"\"\"}}){{ clientMutationId }} }}",
        lines.join("\n")
    )
}

#[test]
fn a_block_string_with_a_non_ascii_whitespace_line_between_indented_lines_is_refused_not_a_panic() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for blank in ["\u{3000}", "\u{00A0}", "\u{2003}"] {
        // One space of indent: the parser cuts the blank line at byte 1, inside its character.
        let q = block_string_document(&[" a", blank, " b"]);
        let out = s.send(
            "POST",
            "api.github.com",
            "/graphql",
            &graphql(&q, serde_json::json!({})),
        );
        assert!(
            is_refused(&out, Refusal::GraphqlParse),
            "{blank:?}: {out:?}"
        );
    }
}

#[test]
fn a_block_string_whose_indent_is_cut_inside_a_non_ascii_whitespace_character_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for wide in ["\u{3000}", "\u{2003}", "\u{00A0}"] {
        // The shallowest line sets the common indent at one byte; the other's indent is a
        // multi-byte whitespace character, which that cut lands in.
        let deep = format!("{wide}{wide}b");
        let q = block_string_document(&[" a", &deep]);
        let out = s.send(
            "POST",
            "api.github.com",
            "/graphql",
            &graphql(&q, serde_json::json!({})),
        );
        assert!(is_refused(&out, Refusal::GraphqlParse), "{wide:?}: {out:?}");
    }
}

#[test]
fn the_proxy_answers_the_next_document_after_one_the_parser_could_not_read() {
    purlis_core::unsteered!();
    let s = Scene::single();
    let q = block_string_document(&[" a", "\u{3000}", " b"]);
    let _ = s.send(
        "POST",
        "api.github.com",
        "/graphql",
        &graphql(&q, serde_json::json!({})),
    );
    let out = s.send(
        "POST",
        "api.github.com",
        "/graphql",
        &graphql(
            "mutation{ deleteIssue(input:{issueId:\"x\"}){ clientMutationId } }",
            serde_json::json!({}),
        ),
    );
    assert!(is_refused(&out, Refusal::GraphqlUnknownMutation), "{out:?}");
}
