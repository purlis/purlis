// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Whether an artifact's template holds executable script** (ADR 0122 §4, #5959): what makes
//! a page one that each person approves before it runs, where a page without any opens straight
//! away.
//!
//! The markup is read by the HTML tokenizer browsers' parsing is specified by (`html5ever`),
//! never by matching text, and every one of the template's pages is read: each `.html`, `.htm`,
//! `.xhtml` and `.svg` that is not a data file ([`super::serve::is_data`]). A page holds script
//! when any of these is in it:
//!
//! - **a `<script>` a browser runs**: no `type`, an empty one, a JavaScript type, `module`, or
//!   `importmap` / `speculationrules`. Any other type is a data block a browser never runs, so
//!   `<script type="application/json">` is data, not script. An SVG's `<script>` counts too;
//! - **an inline handler**: any attribute named `on…` (`onclick`, `onload`, `onerror`); one that
//!   only looks like one (`one=`) asks when it need not, which is the side to err on;
//! - **a `javascript:` URL** in any attribute, as a browser reads one (spaces around it and tabs
//!   and newlines in it ignored, any case), or in a `<meta>`'s `content` (a refresh to one).
//!
//! **This decides what is asked, never what is enforced.** A page is served under a CSP that
//! runs no script until the person has approved this very template (`serve::csp`), so a page
//! this reads wrong only asks when it need not, or renders without the script it holds; nothing
//! runs on a wrong answer here.

use std::cell::Cell;

use html5ever::tokenizer::states::RawKind;
use html5ever::tokenizer::{
    BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
};

/// Whether `key`, a path in an artifact's folder, is one of its pages, which can hold script.
pub fn is_page(key: &str) -> bool {
    let extension = key.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    matches!(extension.as_deref(), Some("html" | "htm" | "xhtml" | "svg"))
}

/// **Whether `markup` holds script a browser would run** (the module's three cases).
pub fn holds_script(markup: &str) -> bool {
    let sink = Finder {
        found: Cell::new(false),
    };
    let tokenizer = Tokenizer::new(sink, TokenizerOpts::default());
    let input = BufferQueue::default();
    input.push_back(markup.into());
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    tokenizer.sink.found.get()
}

/// The tokenizer's sink: notes script, and switches the tokenizer into the text states a tree
/// builder would, so what is inside a `<style>` or a `<textarea>` is read as text, not tags.
struct Finder {
    found: Cell<bool>,
}

impl TokenSink for Finder {
    type Handle = ();

    fn process_token(&self, token: Token, _line: u64) -> TokenSinkResult<()> {
        let Token::TagToken(tag) = token else {
            return TokenSinkResult::Continue;
        };
        if tag.kind != TagKind::StartTag {
            return TokenSinkResult::Continue;
        }
        let name = tag.name.as_ref();
        let attr = |wanted: &str| {
            tag.attrs
                .iter()
                .find(|a| a.name.local.as_ref().eq_ignore_ascii_case(wanted))
                .map(|a| a.value.as_ref())
        };
        let runs = name == "script" && runs_as_script(attr("type"));
        let handler = tag
            .attrs
            .iter()
            .any(|a| a.name.local.len() > 2 && a.name.local[..2].eq_ignore_ascii_case("on"));
        let url = tag.attrs.iter().any(|a| {
            is_javascript_url(&a.value)
                || (name == "meta"
                    && a.name.local.as_ref().eq_ignore_ascii_case("content")
                    && squeezed(&a.value).contains("javascript:"))
        });
        if runs || handler || url {
            self.found.set(true);
        }
        if tag.self_closing {
            return TokenSinkResult::Continue;
        }
        match name {
            "script" => TokenSinkResult::RawData(RawKind::ScriptData),
            "style" | "xmp" | "iframe" | "noembed" | "noframes" => {
                TokenSinkResult::RawData(RawKind::Rawtext)
            }
            "title" | "textarea" => TokenSinkResult::RawData(RawKind::Rcdata),
            "plaintext" => TokenSinkResult::Plaintext,
            _ => TokenSinkResult::Continue,
        }
    }
}

/// Whether a `<script>` with type attribute `kind` is run by a browser (HTML's "prepare the
/// script element"): no type, an empty one, a JavaScript MIME type's essence, `module`, and the
/// two other types a browser acts on.
fn runs_as_script(kind: Option<&str>) -> bool {
    let Some(kind) = kind else { return true };
    let kind = kind.trim_matches(|c: char| c.is_ascii_whitespace());
    let essence = kind
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    kind.is_empty()
        || JAVASCRIPT_TYPES.contains(&essence.as_str())
        || matches!(
            essence.as_str(),
            "module" | "importmap" | "speculationrules"
        )
}

/// HTML's JavaScript MIME type essences.
const JAVASCRIPT_TYPES: [&str; 16] = [
    "application/ecmascript",
    "application/javascript",
    "application/x-ecmascript",
    "application/x-javascript",
    "text/ecmascript",
    "text/javascript",
    "text/javascript1.0",
    "text/javascript1.1",
    "text/javascript1.2",
    "text/javascript1.3",
    "text/javascript1.4",
    "text/javascript1.5",
    "text/jscript",
    "text/livescript",
    "text/x-ecmascript",
    "text/x-javascript",
];

/// A value as a URL parser reads it: no ASCII tab or newline anywhere, lowercase.
fn squeezed(value: &str) -> String {
    value
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Whether `value` is a `javascript:` URL: leading and trailing C0 controls and spaces are
/// dropped, as the URL parser drops them, before the scheme is read.
fn is_javascript_url(value: &str) -> bool {
    squeezed(value)
        .trim_matches(|c: char| c <= ' ')
        .starts_with("javascript:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_json_or_template_block_is_data_and_not_script() {
        for page in [
            "<h1>Plain</h1><p>No script here.</p>",
            r#"<script type="application/json">{"open": 3}</script>"#,
            r#"<script type="application/ld+json">{}</script>"#,
            r#"<script type="text/template"><img onerror=x></script>"#,
            "<style>a::after { content: '<img onerror=x>' }</style>",
            "<textarea><script>alert(1)</script></textarea>",
            "<title><script>alert(1)</script></title>",
            "<!-- <script>alert(1)</script> -->",
            "<p>javascript: is a word in a sentence</p>",
            r#"<a href="https://example.com/javascript:">a link</a>"#,
            r#"<p data-onclick="x" class="on">a handler's name inside another</p>"#,
        ] {
            assert!(!holds_script(page), "{page}");
        }
    }

    #[test]
    fn a_script_a_browser_runs_is_script() {
        for page in [
            "<script>document.title = 1</script>",
            "<SCRIPT SRC=app.js></SCRIPT>",
            r#"<script type="">x</script>"#,
            r#"<script type=" Text/JavaScript ">x</script>"#,
            r#"<script type="text/javascript; charset=utf-8">x</script>"#,
            r#"<script type="module" src="app.mjs"></script>"#,
            r#"<script type="importmap">{}</script>"#,
            "<svg><script>alert(1)</script></svg>",
            "<script/src=app.js>",
        ] {
            assert!(holds_script(page), "{page}");
        }
    }

    #[test]
    fn an_inline_handler_or_a_javascript_url_is_script() {
        for page in [
            r#"<button onclick="go()">Go</button>"#,
            "<img src=x ONERROR=alert(1)>",
            "<body onload=start()>",
            "<svg><rect onmouseover='x()'/></svg>",
            r#"<a href="javascript:alert(1)">x</a>"#,
            "<a href=' JaVaScRiPt:alert(1)'>x</a>",
            "<a href=\"java\tscript:alert(1)\">x</a>",
            r#"<iframe src="javascript:1"></iframe>"#,
            r#"<form action="javascript:go()"></form>"#,
            r#"<svg><a xlink:href="javascript:x()"><text>x</text></a></svg>"#,
            r#"<meta http-equiv="refresh" content="0; url=javascript:alert(1)">"#,
        ] {
            assert!(holds_script(page), "{page}");
        }
    }

    #[test]
    fn a_page_is_markup_a_browser_can_run_script_from() {
        for page in ["index.html", "sub/a.HTM", "b.xhtml", "img/chart.svg"] {
            assert!(is_page(page), "{page}");
        }
        for not in ["app.js", "data.json", "style.css", "html", "a.png"] {
            assert!(!is_page(not), "{not}");
        }
    }
}
