// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The links an artifact's page may follow out of purlis** (ADR 0122 §5, #5956).
//!
//! - **Only a link written in the template** ([`written`]): an `<a>` or `<area>` whose `href`
//!   one of the template's pages holds, as the HTML tokenizer browsers are specified by reads
//!   it (entities decoded), never a data file's. A link the page's script built, or one it
//!   rewrote, is not written there, so it is refused (the app says so).
//! - **Each person's "always open"** for a host ([`always_opens`], [`always_open`]), on their
//!   own machine: `<config>/artifact-link-hosts.json` (`docs/plane-format.md`, Machine tier,
//!   device-bound). Never the plane's, so a teammate's choice never opens a link for you. Every
//!   unreadable state of the record (missing, malformed, a link, too big) reads as no host at
//!   all, so it can only ask again.
//!
//! Which scheme opens and how is the app's (`artifact_links.rs`): this module renders nothing
//! and opens nothing.

use std::cell::RefCell;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use html5ever::tokenizer::states::RawKind;
use html5ever::tokenizer::{
    BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
};
use serde_json::{Value, json};

use super::serve::{self, Snapshot};

/// The record, in purlis's folder of the config home ([`crate::machine::dir`]).
pub const RECORD: &str = "artifact-link-hosts.json";

/// The most the record may be.
const MOST_BYTES: u64 = 1 << 20;

/// The longest host a record holds: a DNS name's longest, with a port.
const HOST_MOST: usize = 253 + 6;

/// **Whether `href`, as a link's attribute holds it, is written in the template** of
/// `snapshot`: an `<a>` or `<area>` of one of its pages, never of a data file.
pub fn written(snapshot: &Snapshot, href: &str) -> bool {
    snapshot.paths().any(|path| {
        !serve::is_data(path)
            && super::script::is_page(path)
            && snapshot.get(path).is_some_and(|bytes| {
                hrefs(&String::from_utf8_lossy(bytes))
                    .iter()
                    .any(|h| h == href)
            })
    })
}

/// Every `href` of an `<a>` or `<area>` in `markup`, entities decoded, in order.
fn hrefs(markup: &str) -> Vec<String> {
    let sink = Hrefs {
        found: RefCell::new(Vec::new()),
    };
    let tokenizer = Tokenizer::new(sink, TokenizerOpts::default());
    let input = BufferQueue::default();
    input.push_back(markup.into());
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    tokenizer.sink.found.take()
}

/// The tokenizer's sink: notes each link's `href`, and reads what is inside a `<script>`,
/// `<style>` or `<textarea>` as text, as a tree builder would, so a link written in a script's
/// text is not one the page holds.
struct Hrefs {
    found: RefCell<Vec<String>>,
}

impl TokenSink for Hrefs {
    type Handle = ();

    fn process_token(&self, token: Token, _line: u64) -> TokenSinkResult<()> {
        let Token::TagToken(tag) = token else {
            return TokenSinkResult::Continue;
        };
        if tag.kind != TagKind::StartTag {
            return TokenSinkResult::Continue;
        }
        let name = tag.name.as_ref();
        if matches!(name, "a" | "area")
            && let Some(href) = tag
                .attrs
                .iter()
                .find(|a| a.name.local.as_ref().eq_ignore_ascii_case("href"))
        {
            self.found.borrow_mut().push(href.value.to_string());
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

fn path(config_root: &Path) -> PathBuf {
    crate::machine::dir(config_root).join(RECORD)
}

/// Whether `host` is one a record holds: a lowercase host as a URL parser spells it, and a
/// port if it named one.
fn is_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= HOST_MOST
        && host
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || ".-:[]".contains(c))
}

/// Every host the record holds; nothing when it cannot be read.
fn hosts(config_root: &Path) -> Vec<String> {
    let at = path(config_root);
    let Ok(file) = crate::contain::open_no_link(config_root, &at) else {
        return Vec::new();
    };
    let plain = file
        .metadata()
        .is_ok_and(|meta| meta.is_file() && meta.len() <= MOST_BYTES);
    let mut text = String::new();
    if !plain || file.take(MOST_BYTES).read_to_string(&mut text).is_err() {
        return Vec::new();
    }
    let Ok(Value::Object(doc)) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let Some(Value::Array(always)) = doc.get("always") else {
        return Vec::new();
    };
    always
        .iter()
        .filter_map(Value::as_str)
        .filter(|host| is_host(host))
        .map(str::to_owned)
        .collect()
}

/// **Whether the person chose to always open links to `host`** on the machine whose config
/// home is `config_root` (none: no record, so it asks).
pub fn always_opens(config_root: Option<&Path>, host: &str) -> bool {
    config_root.is_some_and(|root| hosts(root).iter().any(|one| one == host))
}

/// **Records the person's "always open" for `host`**: a link from any artifact to it opens
/// without asking on this machine from now on. Only purlis's link dialog calls this, never a
/// command a chat can run.
pub fn always_open(config_root: &Path, host: &str) -> Result<(), String> {
    if !is_host(host) {
        return Err(format!(
            "'{}' is not a host purlis can keep",
            crate::personas::one_line(host)
        ));
    }
    let mut always = hosts(config_root);
    if !always.iter().any(|one| one == host) {
        always.push(host.to_owned());
        always.sort();
    }
    let doc = json!({ "version": 1, "always": always });
    let words = |e: std::io::Error| {
        format!(
            "purlis could not keep the choice ({})",
            crate::rewrite::os_words(&e)
        )
    };
    crate::machine::private_dir(config_root).map_err(words)?;
    let text = crate::pyjson::dumps_indent2(&doc) + "\n";
    crate::machine::write_beside(config_root, &path(config_root), text.as_bytes()).map_err(words)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn snapshot(files: &[(&str, &str)]) -> (tempfile::TempDir, Snapshot) {
        let dir = tempfile::tempdir().unwrap();
        for (path, text) in files {
            let at = dir.path().join(path);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, text).unwrap();
        }
        let snapshot = Snapshot::read(dir.path(), "board").unwrap();
        (dir, snapshot)
    }

    #[test]
    fn a_link_written_in_a_page_of_the_template_is_written() {
        let (_dir, page) = snapshot(&[
            (
                "index.html",
                r#"<a href="https://github.com/purlis/purlis/issues?q=is%3Aopen&amp;sort=new">Issues</a>
                   <map><area href='https://example.com/area'></map>"#,
            ),
            ("more/page.htm", "<A HREF=https://example.org/>x</A>"),
        ]);
        assert!(written(
            &page,
            "https://github.com/purlis/purlis/issues?q=is%3Aopen&sort=new"
        ));
        assert!(written(&page, "https://example.com/area"));
        assert!(written(&page, "https://example.org/"));
    }

    #[test]
    fn a_link_only_a_script_or_a_data_file_holds_is_not_written() {
        let (_dir, page) = snapshot(&[
            (
                "index.html",
                r#"<a href="https://example.com/">x</a>
                   <script>document.body.innerHTML = '<a href="https://evil.example/">y</a>'</script>
                   <link href="https://style.example/">
                   <textarea><a href="https://typed.example/"></a></textarea>"#,
            ),
            (
                "data.json",
                r#"{"html": "<a href=\"https://data.example/\">z</a>"}"#,
            ),
            (
                "data/page.html",
                r#"<a href="https://data2.example/">z</a>"#,
            ),
            ("notes.txt", r#"<a href="https://text.example/">z</a>"#),
        ]);
        assert!(written(&page, "https://example.com/"));
        for not in [
            "https://evil.example/",
            "https://style.example/",
            "https://typed.example/",
            "https://data.example/",
            "https://data2.example/",
            "https://text.example/",
            "https://example.com",
            " https://example.com/",
        ] {
            assert!(!written(&page, not), "{not}");
        }
    }

    #[test]
    fn always_open_is_kept_per_host_on_this_machine_and_asks_until_chosen() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path();
        assert!(!always_opens(Some(config), "github.com"));
        always_open(config, "github.com").unwrap();
        always_open(config, "github.com").unwrap();
        always_open(config, "example.com:8443").unwrap();
        assert!(always_opens(Some(config), "github.com"));
        assert!(always_opens(Some(config), "example.com:8443"));
        assert!(!always_opens(Some(config), "gist.github.com"));
        assert!(!always_opens(Some(config), "example.com"));
        assert!(!always_opens(None, "github.com"));
        let text = std::fs::read_to_string(path(config)).unwrap();
        assert_eq!(text.matches("github.com").count(), 1, "{text}");
    }

    #[test]
    fn a_host_that_is_not_one_is_refused_and_nothing_is_written() {
        let dir = tempfile::tempdir().unwrap();
        for not in ["", "GitHub.com", "a b", "x/y", "evil.example/@github.com"] {
            assert!(always_open(dir.path(), not).is_err(), "{not}");
        }
        assert!(!path(dir.path()).exists());
    }

    #[test]
    fn an_unreadable_record_reads_as_no_host() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path();
        crate::machine::private_dir(config).unwrap();
        for text in [
            "not json",
            r#"{"always": "github.com"}"#,
            r#"{"always": ["GITHUB.COM", 7]}"#,
        ] {
            std::fs::write(path(config), text).unwrap();
            assert!(!always_opens(Some(config), "github.com"), "{text}");
        }
        std::fs::remove_file(path(config)).unwrap();
        let elsewhere = config.join("elsewhere.json");
        std::fs::write(&elsewhere, r#"{"always": ["github.com"]}"#).unwrap();
        std::os::unix::fs::symlink(&elsewhere, path(config)).unwrap();
        assert!(!always_opens(Some(config), "github.com"));
    }
}
