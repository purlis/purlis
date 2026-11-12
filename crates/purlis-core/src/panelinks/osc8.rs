// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A link a program marked itself** (OSC 8, #6218, spec #6208): words on the screen, and the
//! target the program named for them, which the pane hands over as it was handed them.
//!
//! - **The target is read by its scheme's kind** ([`kind_of`]): `http:` and `https:` by the web
//!   address's rules, `file:` by a file's (one on another host is refused, ruling Q2), and
//!   every other scheme is refused with a sentence that names it.
//! - **Words that read as another address or path ask first** ([`misleading`], ruling Q1): text
//!   such as "Click here" opens its target the way a found link does, but `docs.purlis.dev`
//!   marked as a link to another host, or `src/main.rs` marked as a link to another file, asks,
//!   showing both. Every word of the text is read, so an address in a sentence still asks.

use std::path::{Component, Path, PathBuf};

use super::{Kind, Pane, Target};

/// What a marked link whose target is a file that is not there says on its hover.
pub const NO_FILE: &str = "purlis found no file at the address this link names";

/// **The kind that reads a marked link's `target`**, by its scheme, or the sentence that says
/// why it is not opened.
pub fn kind_of(target: Option<&str>) -> Result<Kind, String> {
    let Some(target) = target.filter(|target| !target.is_empty()) else {
        return Err("the program named no address for this link".into());
    };
    let scheme = target
        .split_once(':')
        .map(|(scheme, _)| scheme)
        .filter(|scheme| {
            scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        });
    let Some(scheme) = scheme else {
        return Err("purlis could not read the address this link names".into());
    };
    match scheme.to_ascii_lowercase().as_str() {
        "http" | "https" => Ok(Kind::Url),
        "file" => Ok(Kind::File),
        other => Err(format!(
            "purlis opens only http, https and file links, so it will not open this {other}: one"
        )),
    }
}

/// **What a marked link's `text` says that its `target` is not**, in the words the ask shows
/// ("docs.purlis.dev", "src/lib.rs"), or `None` when no word of it reads as an address or a path
/// naming another target. Read with no look at the disk.
pub fn misleading(text: &str, target: &Target, pane: &Pane) -> Option<String> {
    text.split_whitespace()
        .map(trimmed)
        .filter_map(reads_as)
        .find(|reads| !names(reads, target, pane))
        .map(Reads::said)
}

/// What one word of a marked link's text reads as.
#[derive(Debug)]
enum Reads {
    /// A web address, or a word that starts with a host: the host.
    Address(String),
    /// A path: its words, without a `:line[:col]`.
    Path(String),
    /// A name with a dot in it and no `/`, which reads as a host (`github.com`) or a file's name
    /// (`README.md`) alike.
    Name(String),
}

impl Reads {
    fn said(self) -> String {
        match self {
            Reads::Address(said) | Reads::Path(said) | Reads::Name(said) => said,
        }
    }
}

/// A word with the brackets, quotes and punctuation a sentence puts around it taken off.
fn trimmed(word: &str) -> &str {
    word.trim_start_matches(['(', '[', '{', '<', '"', '\'', '`'])
        .trim_end_matches([
            ')', ']', '}', '>', '"', '\'', '`', '.', ',', ';', ':', '!', '?',
        ])
}

/// What `word` reads as, if it reads as an address or a path at all.
fn reads_as(word: &str) -> Option<Reads> {
    if word.is_empty() {
        return None;
    }
    if let Some((scheme, _)) = word.split_once("://") {
        if scheme.eq_ignore_ascii_case("file") {
            let path = url::Url::parse(word)
                .ok()
                .and_then(|url| url.to_file_path().ok())
                .map_or_else(|| word.to_owned(), |path| path.display().to_string());
            return Some(Reads::Path(path));
        }
        let host = url::Url::parse(word)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .unwrap_or_else(|| word.to_ascii_lowercase());
        return Some(Reads::Address(host));
    }
    let (words, _, _) = at_line(word);
    if ["/", "~/", "./", "../"]
        .iter()
        .any(|start| words.starts_with(start))
    {
        return Some(Reads::Path(words.to_owned()));
    }
    if let Some((first, _)) = words.split_once('/') {
        return Some(match host_like(first) {
            Some(host) => Reads::Address(host),
            None => Reads::Path(words.to_owned()),
        });
    }
    host_like(words).map(|_| Reads::Name(words.to_owned()))
}

/// `word` as a host, lowercased and without a port, if it reads as one: two labels at least,
/// the last letters only and two of them at least (`v1.2` and `e.g` are none).
fn host_like(word: &str) -> Option<String> {
    let host = word.split_once(':').map_or(word, |(host, _)| host);
    let labels: Vec<&str> = host.split('.').collect();
    let last = labels.last()?;
    let reads = labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && label
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
        })
        && last.chars().count() >= 2
        && last.chars().all(char::is_alphabetic);
    reads.then(|| host.to_lowercase())
}

/// Whether what a word reads as names `target`: the same host, or the same file.
fn names(reads: &Reads, target: &Target, pane: &Pane) -> bool {
    match (reads, target) {
        (Reads::Address(host), Target::Url(url)) => url.host_str() == Some(host.as_str()),
        (Reads::Path(path), Target::File(file)) => same_file(path, &file.path, pane),
        (Reads::Name(name), Target::Url(url)) => {
            url.host_str() == Some(name.to_lowercase().as_str())
                || url
                    .path_segments()
                    .and_then(|mut segments| segments.next_back())
                    .is_some_and(|last| last == name)
        }
        (Reads::Name(name), Target::File(file)) => file
            .path
            .file_name()
            .is_some_and(|last| last == name.as_str()),
        _ => false,
    }
}

/// Whether `words`, a path as a program printed it, names `file` (absolute, resolved): the
/// same path from `/`, the same from the chat's folder where it climbs out with `..`, and
/// otherwise the end of `file`'s path.
fn same_file(words: &str, file: &Path, pane: &Pane) -> bool {
    let path = Path::new(words.strip_prefix("~/").unwrap_or(words));
    if path.is_absolute() {
        return path == file || std::fs::canonicalize(path).is_ok_and(|real| real == file);
    }
    let parts: Vec<Component> = path
        .components()
        .filter(|part| *part != Component::CurDir)
        .collect();
    if parts.contains(&Component::ParentDir) {
        let Some(folder) = pane.folder.as_ref() else {
            return false;
        };
        let mut joined = PathBuf::new();
        for part in folder.join(path).components() {
            match part {
                Component::ParentDir => {
                    joined.pop();
                }
                Component::CurDir => {}
                other => joined.push(other),
            }
        }
        return joined == file || std::fs::canonicalize(&joined).is_ok_and(|real| real == file);
    }
    !parts.is_empty() && file.ends_with(parts.iter().collect::<PathBuf>())
}

/// `text` without a `:line[:col]` after it, as a path is printed.
fn at_line(text: &str) -> (&str, Option<&str>, Option<&str>) {
    let number = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    let Some((before, last)) = text.rsplit_once(':') else {
        return (text, None, None);
    };
    if !number(last) || before.is_empty() {
        return (text, None, None);
    }
    match before.rsplit_once(':') {
        Some((path, line)) if number(line) && !path.is_empty() => (path, Some(line), Some(last)),
        _ => (before, Some(last), None),
    }
}

#[cfg(test)]
#[path = "osc8_tests.rs"]
mod tests;
