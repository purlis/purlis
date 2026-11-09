// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis artifact check <folder>`** (ADR 0122 §7, #5975): what an author learns before a
//! person is asked to approve the page.
//!
//! - **The declaration** is read exactly as purlis reads it when it serves the page
//!   ([`super::data::declared`]): an `api` this build never shipped, a data name that is not a
//!   data file, or a query that is not in the catalogue, at a version never shipped, or with a
//!   parameter it does not take, makes the block declare nothing, and the check says why.
//! - **The calls the page's code makes**: every string in its pages and scripts shaped as a
//!   query, `<source>.<name>@<N>`, must be one it declared as written, since purlis answers an
//!   undeclared call `not-approved`. This reads the author's source for the author's sake; it
//!   decides nothing: the port refuses an undeclared call whatever this says.
//! - **A deprecated or replaced query** still answers, and the check warns as the bar does.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use super::calls;
use super::data::{self, Declared};

/// A string literal in a page's code shaped as a declared query, `"<source>.<name>@<N>"`.
static CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"["'`]([a-z0-9][a-z0-9-]*\.[a-z][a-z0-9_]*@[0-9]+)["'`]"#).expect("valid")
});

/// What the check found in a folder.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Checked {
    /// What the page declares, when it declares anything.
    pub declared: Option<Declared>,
    /// Each thing that makes the page read less than its author meant, as a sentence.
    pub refused: Vec<String>,
    /// Each thing that still works but will not for long, as a sentence.
    pub warnings: Vec<String>,
}

/// **Checks the page in `folder`**, named `name` in what it says: its `index.html`'s
/// declaration, and the calls its pages and scripts make, read as purlis would serve them
/// ([`super::serve::Snapshot`]: no link, no hidden file, under the size cap). An error only when
/// there is no page to check.
pub fn check(folder: &Path, name: &str) -> Result<Checked, String> {
    let snapshot = super::serve::Snapshot::read(folder, name)?;
    let markup = String::from_utf8_lossy(snapshot.get(super::PAGE).unwrap_or_default());
    let mut refused = Vec::new();
    let declared = data::declared(&markup).unwrap_or_else(|why| {
        refused.push(format!(
            "Its <script id=\"{}\"> block declares nothing, so the page reads nothing: {why}.",
            data::BLOCK_ID
        ));
        None
    });
    let declared_calls = declared.as_ref().map_or(&[][..], |d| d.queries.as_slice());
    let warnings = calls::listed(declared_calls)
        .into_iter()
        .filter_map(|listed| listed.warning.map(|w| format!("{w}.")))
        .collect();
    let mut seen: Vec<String> = Vec::new();
    for key in snapshot.paths().filter(|key| is_source(key)) {
        let mut text = String::from_utf8_lossy(snapshot.get(key).unwrap_or_default()).into_owned();
        if key == super::PAGE
            && let Some(block) = data::block_of(&text)
        {
            // What the block declares is not a call: a bad one is said once, above.
            text = text.replacen(&block, "", 1);
        }
        for found in CALL.captures_iter(&text) {
            let call = found.get(1).map_or("", |m| m.as_str());
            if declared_calls.iter().any(|c| c.declared_as() == call)
                || seen.iter().any(|s| s == call)
            {
                continue;
            }
            refused.push(format!(
                "{key} calls {call}, which the page does not declare, so purlis answers it \
                 not-approved: add it to the \"queries\" of its <script id=\"{}\"> block.",
                data::BLOCK_ID
            ));
            seen.push(call.to_owned());
        }
    }
    Ok(Checked {
        declared,
        refused,
        warnings,
    })
}

/// Whether `key`, a path in the folder, is code that may call a query: a page or a script, and
/// never a data file.
fn is_source(key: &str) -> bool {
    let script = key
        .rsplit_once('.')
        .is_some_and(|(_, e)| matches!(e.to_ascii_lowercase().as_str(), "js" | "mjs"));
    (script || super::script::is_page(key)) && !super::serve::is_data(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(files: &[(&str, &str)]) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        for (name, text) in files {
            let path = tmp.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        tmp
    }

    fn page(block: &str, script: &str) -> String {
        format!(
            "<!doctype html><title>Board</title>\
             <script type=\"application/json\" id=\"purlis\">{block}</script>\
             <script>{script}</script>"
        )
    }

    #[test]
    fn a_page_that_calls_only_what_it_declared_passes() {
        let tmp = folder(&[
            (
                "index.html",
                &page(
                    r#"{ "api": 1, "data": ["data.json"], "queries": { "purlis.todo_list@1": ["done"] } }"#,
                    r#"purlis.port.postMessage({ id: 1, query: "purlis.todo_list@1", params: { done: true } });"#,
                ),
            ),
            ("app.js", "const q = 'purlis.todo_list@1';"),
        ]);
        let checked = check(tmp.path(), "board").unwrap();
        assert_eq!(checked.refused, Vec::<String>::new());
        let declared = checked.declared.unwrap();
        assert_eq!(declared.api, 1);
        assert_eq!(declared.data, ["data.json"]);
        assert_eq!(declared.queries[0].declared_as(), "purlis.todo_list@1");
    }

    #[test]
    fn a_call_the_page_does_not_declare_is_refused_once_wherever_it_is_made() {
        let tmp = folder(&[
            (
                "index.html",
                &page(
                    r#"{ "api": 1, "queries": { "purlis.todo_list@1": [] } }"#,
                    r#"ask("purlis.change_status@1"); ask("purlis.change_status@1");"#,
                ),
            ),
            ("lib/ask.mjs", "ask(`purlis.memory_search@1`)"),
            ("data/rows.json", r#"{ "q": "purlis.memory_search@1" }"#),
            (".hidden/x.js", "ask('purlis.session_record_list@1')"),
        ]);
        let refused = check(tmp.path(), "board").unwrap().refused;
        assert_eq!(refused.len(), 2, "{refused:#?}");
        assert!(refused[0].starts_with(
            "index.html calls purlis.change_status@1, which the page does not declare"
        ));
        assert!(refused[1].starts_with("lib/ask.mjs calls purlis.memory_search@1"));
    }

    #[test]
    fn an_unknown_query_a_version_never_shipped_or_a_parameter_not_taken_declares_nothing() {
        for (block, why) in [
            (
                r#"{ "api": 1, "queries": { "purlis.no_such@1": [] } }"#,
                "which is no query",
            ),
            (
                r#"{ "api": 1, "queries": { "purlis.todo_list@9": [] } }"#,
                "answers @1 to @1",
            ),
            (
                r#"{ "api": 1, "queries": { "purlis.todo_list@1": ["all"] } }"#,
                "takes no such parameter",
            ),
            (r#"{ "api": 2 }"#, "declares no \"api\" this purlis answers"),
        ] {
            let tmp = folder(&[("index.html", &page(block, ""))]);
            let checked = check(tmp.path(), "board").unwrap();
            assert_eq!(checked.declared, None, "{block}");
            assert_eq!(checked.refused.len(), 1, "{block}: {:#?}", checked.refused);
            assert!(
                checked.refused[0].contains(why),
                "{block}: {}",
                checked.refused[0]
            );
        }
    }

    #[test]
    fn a_page_that_declares_nothing_and_calls_a_query_is_refused() {
        let tmp = folder(&[
            (
                "index.html",
                "<title>x</title><script>ask(\"purlis.todo_list@1\")</script>",
            ),
            ("app.js", r#"ask("purlis.todo_list@1")"#),
        ]);
        let checked = check(tmp.path(), "board").unwrap();
        assert_eq!(checked.declared, None);
        assert_eq!(checked.refused.len(), 1, "{:#?}", checked.refused);
        assert!(checked.refused[0].contains("purlis.todo_list@1"));
    }

    #[test]
    fn a_folder_without_a_page_is_an_error() {
        let tmp = folder(&[("readme.md", "x")]);
        assert_eq!(
            check(tmp.path(), "board").unwrap_err(),
            "the artifact has no index.html, the page it opens on"
        );
    }
}
