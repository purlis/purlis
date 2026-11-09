// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What an artifact's page may read of its own data** (ADR 0122 §1, §7, #5961): the data files
//! its page declares, from its own folder, answered in the catalogue's envelope
//! ([`crate::catalogue::Answer`]).
//!
//! - **The page declares them** in a data block of its `index.html`, so the declaration is part
//!   of the template the person approved ([`super::approval`]):
//!
//!   ```html
//!   <script type="application/json" id="purlis">
//!     { "api": 1, "data": ["data.json", "data/rows.json"] }
//!   </script>
//!   ```
//!
//!   `api` is the version of the API the page was written against ([`crate::catalogue::API`]);
//!   `data` names data files ([`super::serve::is_data`]) by their path in the folder, `/`
//!   separated. A declaration that names anything else, or an `api` this build never shipped,
//!   is no declaration at all ([`declared`]), so the page reads nothing.
//! - **Only a declared name is read** ([`read`]), compared as written: no decoding, no case
//!   folding. Anything else answers `not-approved`.
//! - **Read when asked, from the folder, through no link**: a link at the file or on the way to
//!   it answers `refused`, so one planted after the pin is never followed.
//! - **Under a fixed cap** ([`MOST_BYTES`]): a bigger file answers `too-large`, never a part.
//! - **Data, never markup**: the file is parsed as JSON and answered as a value; a file that is
//!   not JSON answers `failed`. A declared file that is not there answers `data: null`, and the
//!   window says the page is not generated on this machine ([`generated`]).

use std::io::Read as _;
use std::path::{Path, PathBuf};

use html5ever::tokenizer::states::RawKind;
use html5ever::tokenizer::{
    BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
};
use serde_json::Value;

use crate::catalogue::{API, Answer, ErrorKind, PROTOCOL_KEY};

/// The `id` of the data block a page declares in.
pub const BLOCK_ID: &str = "purlis";

/// The most one data file may be to be answered: bigger is `too-large`.
pub const MOST_BYTES: u64 = 4 << 20;

/// What a page declared: the API version it was written against and the data files it reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declared {
    pub api: u32,
    pub data: Vec<String>,
    /// The catalogue queries it calls (#5969, [`super::calls`]).
    pub queries: Vec<super::calls::Call>,
}

/// **What the page `markup` declares**, or nothing when it declares nothing. A block that is
/// not the declaration's shape is the sentence saying why, and declares nothing either.
pub fn declared(markup: &str) -> Result<Option<Declared>, String> {
    let Some(block) = block_of(markup) else {
        return Ok(None);
    };
    let Ok(Value::Object(doc)) = serde_json::from_str::<Value>(&block) else {
        return Err(format!(
            "its <script id=\"{BLOCK_ID}\"> block is not a JSON object"
        ));
    };
    let api = doc
        .get(PROTOCOL_KEY)
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| (1..=API).contains(n))
        .ok_or_else(|| {
            format!("it declares no \"{PROTOCOL_KEY}\" this purlis answers (1 to {API})")
        })?;
    let mut data = Vec::new();
    match doc.get("data") {
        None => {}
        Some(Value::Array(names)) => {
            for name in names {
                let Some(name) = name.as_str().filter(|n| is_data_name(n)) else {
                    return Err(format!(
                        "it declares {name} as a data file, and only data.json or a file \
                         under data/ is one"
                    ));
                };
                if !data.iter().any(|d| d == name) {
                    data.push(name.to_owned());
                }
            }
        }
        Some(_) => return Err("its \"data\" is not a list of names".into()),
    }
    let queries = match doc.get("queries") {
        None => Vec::new(),
        Some(queries) => super::calls::parse(queries)?,
    };
    Ok(Some(Declared { api, data, queries }))
}

/// Whether `name` names a data file inside a folder, as written: `data.json` or under `data/`,
/// every segment a plain one, and no `%`, so no name reads as one encoded.
fn is_data_name(name: &str) -> bool {
    super::serve::is_data(name)
        && !name.contains(['\\', '\0', '%'])
        && !name.chars().any(char::is_control)
        && name
            .split('/')
            .all(|segment| !segment.is_empty() && !segment.starts_with('.'))
}

/// Where data file `name` is inside `folder`.
fn path_of(folder: &Path, name: &str) -> PathBuf {
    name.split('/')
        .fold(folder.to_path_buf(), |at, s| at.join(s))
}

/// **Whether the data of the page declaring `declared` was generated on this machine**: it
/// declares no data, or at least one declared file is there.
pub fn generated(folder: &Path, declared: Option<&Declared>) -> bool {
    let Some(declared) = declared.filter(|d| !d.data.is_empty()) else {
        return true;
    };
    declared
        .data
        .iter()
        .any(|name| path_of(folder, name).symlink_metadata().is_ok())
}

/// **The answer to a page asking for data file `name`**, from `folder` (the artifact's folder as
/// [`super::resolve`] answered it), when its page declared `declared`.
pub fn read(
    folder: &Path,
    declared: Option<&Declared>,
    name: &str,
    at: chrono::DateTime<chrono::Utc>,
) -> Answer {
    if !declared.is_some_and(|d| d.data.iter().any(|n| n == name)) {
        return Answer::error(ErrorKind::NotApproved, at);
    }
    match bytes_of(folder, name) {
        Ok(None) => Answer::data(Value::Null, at),
        Ok(Some(bytes)) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(value) => Answer::data(value, at),
            Err(_) => Answer::error(ErrorKind::Failed, at),
        },
        Err(kind) => Answer::error(kind, at),
    }
}

/// The bytes of data file `name`, nothing when it is not there, or why it is not answered.
fn bytes_of(folder: &Path, name: &str) -> Result<Option<Vec<u8>>, ErrorKind> {
    use std::io::ErrorKind as Io;
    let file = match crate::contain::open_no_link(folder, &path_of(folder, name)) {
        Ok(file) => file,
        Err(e) if e.kind() == Io::NotFound => return Ok(None),
        Err(e) if e.kind() == Io::PermissionDenied => return Err(ErrorKind::Refused),
        // A link at the file itself, which the kernel refused to follow.
        Err(e) if is_link_refused(&e) => return Err(ErrorKind::Refused),
        Err(_) => return Err(ErrorKind::Failed),
    };
    let meta = file.metadata().map_err(|_| ErrorKind::Failed)?;
    if !meta.is_file() {
        return Err(ErrorKind::Refused);
    }
    if meta.len() > MOST_BYTES {
        return Err(ErrorKind::TooLarge);
    }
    let mut bytes = Vec::new();
    file.take(MOST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ErrorKind::Failed)?;
    if bytes.len() as u64 > MOST_BYTES {
        return Err(ErrorKind::TooLarge);
    }
    Ok(Some(bytes))
}

/// Whether an open failed because its last component was a link (`ELOOP` under `O_NOFOLLOW`).
fn is_link_refused(e: &std::io::Error) -> bool {
    #[cfg(unix)]
    {
        e.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error())
    }
    #[cfg(not(unix))]
    {
        let _ = e;
        false
    }
}

/// The text of the page's first `<script type="application/json" id="purlis">`, read by the HTML
/// tokenizer as a browser would.
pub(crate) fn block_of(markup: &str) -> Option<String> {
    let sink = Block::default();
    let tokenizer = Tokenizer::new(sink, TokenizerOpts::default());
    let input = BufferQueue::default();
    input.push_back(markup.into());
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    tokenizer.sink.found.take()
}

#[derive(Default)]
struct Block {
    /// Inside the declaration's `<script>`, collecting.
    inside: std::cell::RefCell<Option<String>>,
    found: std::cell::RefCell<Option<String>>,
}

impl TokenSink for Block {
    type Handle = ();

    fn process_token(&self, token: Token, _line: u64) -> TokenSinkResult<()> {
        if self.found.borrow().is_some() {
            return TokenSinkResult::Continue;
        }
        match token {
            Token::CharacterTokens(text) => {
                if let Some(inside) = self.inside.borrow_mut().as_mut() {
                    inside.push_str(&text);
                }
                TokenSinkResult::Continue
            }
            Token::TagToken(tag) if tag.name.as_ref() == "script" => {
                if tag.kind == TagKind::EndTag {
                    if let Some(text) = self.inside.borrow_mut().take() {
                        *self.found.borrow_mut() = Some(text);
                    }
                    return TokenSinkResult::Continue;
                }
                let attr = |wanted: &str| {
                    tag.attrs
                        .iter()
                        .find(|a| a.name.local.as_ref().eq_ignore_ascii_case(wanted))
                        .map(|a| a.value.to_string())
                };
                let ours = attr("id").as_deref() == Some(BLOCK_ID)
                    && attr("type")
                        .is_some_and(|t| t.trim().eq_ignore_ascii_case("application/json"));
                if ours && !tag.self_closing {
                    *self.inside.borrow_mut() = Some(String::new());
                }
                if tag.self_closing {
                    TokenSinkResult::Continue
                } else {
                    TokenSinkResult::RawData(RawKind::ScriptData)
                }
            }
            Token::TagToken(tag) if tag.kind == TagKind::StartTag && !tag.self_closing => {
                match tag.name.as_ref() {
                    "style" | "xmp" | "iframe" | "noembed" | "noframes" => {
                        TokenSinkResult::RawData(RawKind::Rawtext)
                    }
                    "title" | "textarea" => TokenSinkResult::RawData(RawKind::Rcdata),
                    "plaintext" => TokenSinkResult::Plaintext,
                    _ => TokenSinkResult::Continue,
                }
            }
            _ => TokenSinkResult::Continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-11-09T12:00:00Z")
            .unwrap()
            .into()
    }

    fn page(block: &str) -> String {
        format!(
            "<html><head><title>Board</title>\
             <script type=\"application/json\" id=\"purlis\">{block}</script></head>\
             <body><script>go()</script></body></html>"
        )
    }

    fn declaring(names: &[&str]) -> Declared {
        Declared {
            api: 1,
            data: names.iter().map(|n| (*n).to_owned()).collect(),
            queries: Vec::new(),
        }
    }

    fn kind(answer: &Answer) -> Option<&'static str> {
        answer.outcome.as_ref().err().map(|k| k.as_str())
    }

    #[test]
    fn a_page_declares_its_api_and_data_files_in_its_purlis_block() {
        let markup = page(r#"{ "api": 1, "data": ["data.json", "data/rows.json", "data.json"] }"#);
        assert_eq!(
            declared(&markup).unwrap(),
            Some(declaring(&["data.json", "data/rows.json"]))
        );
        assert_eq!(
            declared(&page(r#"{"api": 1}"#)).unwrap(),
            Some(declaring(&[]))
        );
    }

    #[test]
    fn a_page_declares_the_queries_it_calls_in_the_same_block() {
        let markup = page(r#"{ "api": 1, "queries": { "purlis.todo_list@1": ["done"] } }"#);
        let declared = declared(&markup).unwrap().unwrap();
        assert_eq!(declared.queries.len(), 1);
        assert_eq!(declared.queries[0].declared_as(), "purlis.todo_list@1");
        assert_eq!(declared.queries[0].params, ["done"]);
    }

    #[test]
    fn a_page_without_the_block_declares_nothing() {
        for markup in [
            "<h1>plain</h1>",
            r#"<script type="application/json" id="other">{"api":1}</script>"#,
            r#"<script id="purlis">({"api":1})</script>"#,
            r#"<textarea><script type="application/json" id="purlis">{"api":1}</script></textarea>"#,
            r#"<!-- <script type="application/json" id="purlis">{"api":1}</script> -->"#,
        ] {
            assert_eq!(declared(markup).unwrap(), None, "{markup}");
        }
    }

    #[test]
    fn a_declaration_naming_anything_but_a_data_file_or_an_api_never_shipped_declares_nothing() {
        for block in [
            r#"{"api": 0}"#,
            r#"{"api": 99}"#,
            r#"{"api": "1"}"#,
            r#"{"data": ["data.json"]}"#,
            r#"[1]"#,
            r#"not json"#,
            r#"{"api": 1, "data": "data.json"}"#,
            r#"{"api": 1, "data": ["index.html"]}"#,
            r#"{"api": 1, "data": ["../data.json"]}"#,
            r#"{"api": 1, "data": ["data/../index.html"]}"#,
            r#"{"api": 1, "data": ["data/./x.json"]}"#,
            r#"{"api": 1, "data": ["data//x.json"]}"#,
            r#"{"api": 1, "data": ["data/.hidden"]}"#,
            r#"{"api": 1, "data": ["data\\x.json"]}"#,
            r#"{"api": 1, "data": ["data/%2e%2e/x"]}"#,
            r#"{"api": 1, "data": ["DATA.JSON"]}"#,
            r#"{"api": 1, "data": [1]}"#,
            r#"{"api": 1, "queries": {"purlis.nothing@1": []}}"#,
            r#"{"api": 1, "queries": ["purlis.todo_list@1"]}"#,
        ] {
            let why = declared(&page(block));
            assert!(why.is_err(), "{block}: {why:?}");
        }
    }

    fn folder_with(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "page").unwrap();
        for (path, text) in files {
            let at = dir.path().join(path);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, text).unwrap();
        }
        dir
    }

    #[test]
    fn a_declared_data_file_is_answered_as_data_in_the_envelope() {
        let dir = folder_with(&[("data.json", r#"{"open": 3}"#), ("data/rows.json", "[1,2]")]);
        let folder = dir.path().canonicalize().unwrap();
        let declared = declaring(&["data.json", "data/rows.json"]);
        let answer = read(&folder, Some(&declared), "data.json", now());
        assert_eq!(
            answer.to_json(),
            serde_json::json!({ "data": { "open": 3 }, "at": "2026-11-09T12:00:00Z" })
        );
        let rows = read(&folder, Some(&declared), "data/rows.json", now());
        assert_eq!(rows.outcome, Ok(serde_json::json!([1, 2])));
    }

    #[test]
    fn only_a_name_the_page_declared_is_read_and_as_written() {
        let dir = folder_with(&[
            ("data.json", "1"),
            ("data/other.json", "2"),
            ("secret.json", "3"),
        ]);
        let folder = dir.path().canonicalize().unwrap();
        let declared = declaring(&["data.json"]);
        for asked in [
            "data/other.json",
            "secret.json",
            "index.html",
            "Data.json",
            "DATA.JSON",
            "./data.json",
            "data.json/",
            "data%2ejson",
            "../data.json",
            "%2e%2e/secret.json",
            "data\\..\\secret.json",
        ] {
            let answer = read(&folder, Some(&declared), asked, now());
            assert_eq!(kind(&answer), Some("not-approved"), "{asked}");
        }
        let undeclared = read(&folder, None, "data.json", now());
        assert_eq!(kind(&undeclared), Some("not-approved"));
    }

    #[cfg(unix)]
    #[test]
    fn a_link_planted_after_the_pin_is_refused_never_followed() {
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.json"), r#""secret""#).unwrap();
        std::fs::create_dir(outside.path().join("dir")).unwrap();
        std::fs::write(outside.path().join("dir/rows.json"), r#""secret""#).unwrap();
        let dir = folder_with(&[]);
        let folder = dir.path().canonicalize().unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret.json"), folder.join("data.json"))
            .unwrap();
        std::os::unix::fs::symlink(outside.path().join("dir"), folder.join("data")).unwrap();
        let declared = declaring(&["data.json", "data/rows.json"]);
        for name in ["data.json", "data/rows.json"] {
            let answer = read(&folder, Some(&declared), name, now());
            assert_eq!(kind(&answer), Some("refused"), "{name}");
        }
    }

    #[test]
    fn a_file_over_the_cap_is_too_large_and_never_cut() {
        let dir = folder_with(&[]);
        let folder = dir.path().canonicalize().unwrap();
        let big = format!("\"{}\"", "x".repeat(usize::try_from(MOST_BYTES).unwrap()));
        std::fs::write(folder.join("data.json"), big).unwrap();
        let answer = read(
            &folder,
            Some(&declaring(&["data.json"])),
            "data.json",
            now(),
        );
        assert_eq!(kind(&answer), Some("too-large"));
    }

    #[test]
    fn data_that_is_not_json_failed_and_a_folder_in_its_place_is_refused() {
        let dir = folder_with(&[("data.json", "<b>markup</b>")]);
        let folder = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(folder.join("data/rows.json")).unwrap();
        let declared = declaring(&["data.json", "data/rows.json"]);
        assert_eq!(
            kind(&read(&folder, Some(&declared), "data.json", now())),
            Some("failed")
        );
        assert_eq!(
            kind(&read(&folder, Some(&declared), "data/rows.json", now())),
            Some("refused")
        );
    }

    #[test]
    fn a_declared_file_not_there_is_null_and_the_page_is_not_generated_here() {
        let dir = folder_with(&[]);
        let folder = dir.path().canonicalize().unwrap();
        let declared = declaring(&["data.json", "data/rows.json"]);
        let answer = read(&folder, Some(&declared), "data.json", now());
        assert_eq!(answer.outcome, Ok(Value::Null));
        assert!(!generated(&folder, Some(&declared)));

        std::fs::create_dir(folder.join("data")).unwrap();
        std::fs::write(folder.join("data/rows.json"), "[]").unwrap();
        assert!(generated(&folder, Some(&declared)));
        assert!(generated(&folder, Some(&declaring(&[]))));
        assert!(generated(&folder, None));
    }
}
