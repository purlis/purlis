// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Artifacts in search** (ADR 0120, spec #5948, #5968): what a pinned artifact's pages say,
//! in the project's search index (ADR 0079) beside its memory, as a source of its own.
//!
//! - **Read through the pin's gate, never around it.** Only a pinned folder is read, and only
//!   once [`super::resolve`] has placed it inside its workspace's `artifacts/` with no link
//!   leading out, and [`crate::contain::readable`] has placed `artifacts/` inside the project
//!   (the gate `memstore::read_files` puts every memory through). The folder is then read by
//!   the walk its tab is served from ([`super::serve::Snapshot::read`]): a link inside it is
//!   never followed, and a folder over the size cap is not read at all.
//! - **The template's words, never anyone's data.** Only the template's pages are read
//!   ([`super::script::is_page`]), and only what they show: the text between their tags, never
//!   a script's or a style sheet's. A data file (`data.json`, `data/`) is each person's own,
//!   written on their machine, so the project's index never holds it (ADR 0079 §1).
//! - **Found as the index finds it, or as a scan of the same files does** ([`find`],
//!   [`scan`]): a word that starts with any term of the query, in the pin's name or its pages'
//!   text, after the shape scanner has taken out anything shaped like a secret.
//!
//! A result names its workspace and folder, which is what the window opens an artifact's tab
//! by (`artifactView` in `app/src/artifacts.ts`).

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use html5ever::tokenizer::states::RawKind;
use html5ever::tokenizer::{
    BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
};

use super::serve::{self, Snapshot};

/// One artifact found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// The workspace it is pinned to.
    pub workspace: String,
    /// Its folder in the workspace's `artifacts/`.
    pub folder: String,
    /// The pin's name, as the index keeps it.
    pub name: String,
}

/// What [`find`] found, and whether the index answered.
#[derive(Clone, Debug, Default)]
pub struct Searched {
    pub found: Vec<Found>,
    /// `false` when there was no index this build can read, so the files were scanned.
    pub indexed: bool,
}

/// A pinned artifact as the index takes it.
pub(crate) struct Indexed {
    /// The pin's name.
    pub name: String,
    /// What its pages show, page after page.
    pub text: String,
    /// The bytes of its template.
    pub size: u64,
}

/// **The artifacts of `workspace` (every workspace's with `None`) that say any word of
/// `query`**, best first: from the project's index where there is one this build can read, by
/// scanning the files ([`scan`]) where there is not.
pub fn find(root: &Path, workspace: Option<&str>, query: &str) -> Searched {
    use crate::projectindex::{Kind, Reach, Reader};
    let terms = crate::memstore::terms(query);
    let Some(reader) = Reader::open(root) else {
        return Searched {
            found: scan(root, workspace, query),
            indexed: false,
        };
    };
    if terms.is_empty() {
        return Searched {
            found: Vec::new(),
            indexed: true,
        };
    }
    let reach = Reach::none().with(Kind::Artifacts, workspace);
    match reader.search(&reach, &terms, -1) {
        Ok(rows) => Searched {
            found: rows
                .into_iter()
                .filter_map(|row| {
                    let folder = row.path.parent()?.file_name()?.to_str()?.to_string();
                    Some(Found {
                        workspace: row.owner,
                        folder,
                        name: row.title,
                    })
                })
                .collect(),
            indexed: true,
        },
        Err(_) => Searched {
            found: scan(root, workspace, query),
            indexed: false,
        },
    }
}

/// [`find`] by reading every pinned artifact in scope: what the index would answer, ranked by
/// how many of their words start with a term, then by workspace and folder.
pub fn scan(root: &Path, workspace: Option<&str>, query: &str) -> Vec<Found> {
    let terms = crate::memstore::terms(query);
    if terms.is_empty() {
        return Vec::new();
    }
    let workspaces = crate::recall::read_workspaces(root)
        .map(|(names, _)| names)
        .unwrap_or_default();
    let mut found: Vec<(usize, Found)> = Vec::new();
    for name in workspaces
        .iter()
        .filter(|name| workspace.is_none_or(|one| one == name.as_str()))
    {
        let artifacts = root.join("workspaces").join(name).join(super::DIR);
        for page in pages(root, &artifacts).0 {
            let Some(one) = indexed(root, &page) else {
                continue;
            };
            let title = crate::projectindex::words_of(&one.name);
            let words = crate::projectindex::words_of(&one.text);
            let hits = title
                .iter()
                .chain(&words)
                .filter(|word| terms.iter().any(|term| word.starts_with(term.as_str())))
                .count();
            if hits == 0 {
                continue;
            }
            let Some(folder) = folder_of(&page) else {
                continue;
            };
            found.push((
                hits,
                Found {
                    workspace: name.clone(),
                    folder,
                    name: crate::projectindex::scanned(&one.name),
                },
            ));
        }
    }
    found.sort_by(|(a, one), (b, other)| {
        b.cmp(a)
            .then_with(|| one.workspace.cmp(&other.workspace))
            .then_with(|| one.folder.cmp(&other.folder))
    });
    found.into_iter().map(|(_, found)| found).collect()
}

/// **Every pinned artifact's page in the workspace folder `artifacts`** (its `artifacts/`), as
/// the index lists a source: the page of each pin whose folder [`super::resolve`] places inside
/// it, as `artifacts/<folder>/index.html`. A pins file that cannot be read is said as unread.
pub(crate) fn pages(root: &Path, artifacts: &Path) -> (Vec<PathBuf>, crate::memstore::Unread) {
    if crate::contain::readable(root, artifacts).is_err() {
        return (Vec::new(), Vec::new());
    }
    let Some(workspace) = artifacts.parent() else {
        return (Vec::new(), Vec::new());
    };
    let pins = match super::read(workspace) {
        Ok(pins) => pins,
        Err(_) => return (Vec::new(), vec![(artifacts.join(super::PINS), None)]),
    };
    let mut found: Vec<PathBuf> = pins
        .pins
        .iter()
        .filter(|pin| super::resolve(workspace, &pin.folder).is_ok())
        .map(|pin| artifacts.join(&pin.folder).join(super::PAGE))
        .collect();
    found.sort();
    found.dedup();
    (found, Vec::new())
}

/// The pinned artifact whose page is `page` (one [`pages`] listed), read through its gate
/// again: `None` when it is no longer pinned, no longer resolves, or cannot be read whole.
pub(crate) fn indexed(root: &Path, page: &Path) -> Option<Indexed> {
    let folder = folder_of(page)?;
    let artifacts = page.parent()?.parent()?;
    let workspace = artifacts.parent()?;
    crate::contain::readable(root, artifacts).ok()?;
    let pin = super::read(workspace)
        .ok()?
        .pins
        .into_iter()
        .find(|pin| pin.folder == folder)?;
    let real = super::resolve(workspace, &folder).ok()?;
    let snapshot = Snapshot::read(&real, &folder).ok()?;
    let mut text = String::new();
    let mut size = 0u64;
    for path in snapshot.paths() {
        if serve::is_data(path) {
            continue;
        }
        let bytes = snapshot.get(path).unwrap_or_default();
        size += bytes.len() as u64;
        if super::script::is_page(path) {
            text.push_str(&shown(&String::from_utf8_lossy(bytes)));
            text.push('\n');
        }
    }
    Some(Indexed {
        name: pin.name,
        text,
        size,
    })
}

/// The folder a pinned page (`…/artifacts/<folder>/index.html`) is in.
fn folder_of(page: &Path) -> Option<String> {
    Some(page.parent()?.file_name()?.to_str()?.to_string())
}

/// **What `markup` shows**: the text between its tags, as the HTML tokenizer browsers are
/// specified by reads it (entities decoded), with what is inside a `<script>`, a `<style>` and
/// the other raw-text elements left out. A `<title>` and a `<textarea>` are text.
pub(crate) fn shown(markup: &str) -> String {
    let sink = Shown {
        text: RefCell::new(String::new()),
        hidden: RefCell::new(None),
    };
    let tokenizer = Tokenizer::new(sink, TokenizerOpts::default());
    let input = BufferQueue::default();
    input.push_back(markup.into());
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    tokenizer.sink.text.take()
}

/// The tokenizer's sink: keeps the characters it is handed, but not inside an element whose
/// content is code, until that element's end tag.
struct Shown {
    text: RefCell<String>,
    /// The raw-text element the tokenizer is inside, whose characters are not shown.
    hidden: RefCell<Option<String>>,
}

impl TokenSink for Shown {
    type Handle = ();

    fn process_token(&self, token: Token, _line: u64) -> TokenSinkResult<()> {
        match token {
            Token::CharacterTokens(text) => {
                if self.hidden.borrow().is_none() {
                    self.text.borrow_mut().push_str(&text);
                }
                TokenSinkResult::Continue
            }
            Token::TagToken(tag) => {
                let name = tag.name.as_ref().to_ascii_lowercase();
                if tag.kind == TagKind::EndTag {
                    if self.hidden.borrow().as_deref() == Some(name.as_str()) {
                        *self.hidden.borrow_mut() = None;
                    }
                    // A tag ends a word: `<td>a</td><td>b</td>` is two words.
                    self.text.borrow_mut().push(' ');
                    return TokenSinkResult::Continue;
                }
                self.text.borrow_mut().push(' ');
                if tag.self_closing {
                    return TokenSinkResult::Continue;
                }
                match name.as_str() {
                    "script" => {
                        *self.hidden.borrow_mut() = Some(name);
                        TokenSinkResult::RawData(RawKind::ScriptData)
                    }
                    "style" | "xmp" | "iframe" | "noembed" | "noframes" => {
                        *self.hidden.borrow_mut() = Some(name);
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

    #[test]
    fn a_page_shows_its_text_and_title_and_never_its_code() {
        let said = shown(
            "<title>Board &amp; plan</title><style>p{color:red}</style>\
             <p>Open <b>issues</b></p><script>let hidden = '</p>';</script><td>a</td><td>b</td>",
        );
        let words = crate::memstore::tokens(&said);
        assert_eq!(
            words,
            ["board", "plan", "open", "issues", "a", "b"],
            "{said}"
        );
    }
}
