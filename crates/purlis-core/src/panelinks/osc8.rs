// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A link a program marked itself** (OSC 8, #6218, spec #6208): words on the screen, and the
//! target the program named for them, which the pane hands over as it was handed them.
//!
//! - **The target is read by its scheme's kind** ([`kind_of`]): `http:` and `https:` by the web
//!   address's rules, `file:` by a file's (one on another host is refused, ruling Q2), and
//!   every other scheme is refused with a sentence that names it. A target longer than
//!   [`MOST_TARGET`] is refused unread.
//! - **Words that read as another address or path ask first** ([`reconsidered`], ruling Q1):
//!   text such as "Click here" opens its target the way a found link does, but
//!   `docs.purlis.dev` marked as a link to another host, or `src/main.rs` marked as a link to
//!   another file, asks, showing both. Every word of the text is read as a reader sees it
//!   ([`as_seen`]): characters that draw nothing are dropped, look-alike dots and wide letters
//!   folded, and the punctuation around a word taken off. An address names its target only
//!   with the same host, port and no weaker scheme, and with no name before an `@`.

use std::path::{Component, Path, PathBuf};

use unicode_normalization::UnicodeNormalization as _;
use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory as _};

use super::file::at_line;
use super::{Asking, Candidate, Kind, Opening, Pane, Resolved, Target};

/// The longest target a marked link may name, in bytes: a longer one is refused unread.
pub const MOST_TARGET: usize = 2048;

/// The most of a marked link's words that are read, in characters: longer words ask first.
pub const MOST_TEXT: usize = 4096;

/// **The kind that reads a marked link's `target`**, by its scheme, or the sentence that says
/// why it is not opened.
pub fn kind_of(target: Option<&str>) -> Result<Kind, String> {
    let Some(target) = target.filter(|target| !target.is_empty()) else {
        return Err("the program named no address for this link".into());
    };
    if target.len() > MOST_TARGET {
        return Err("the address this link names is longer than purlis reads".into());
    }
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
            "purlis opens only http, https and file links, not {other}: links"
        )),
    }
}

/// What a marked link whose target is a file that is not there is: no link, and why. The pane
/// draws it as a link, so it says so rather than nothing.
pub fn not_there() -> Resolved {
    Resolved::NotALink("purlis found no file at the address this link names".into())
}

/// **What opening `candidate`'s `target` does, once its words are read**: a marked link whose
/// words read as another target asks first, unless its kind refused it. Any other candidate's
/// `opening` stands.
pub(super) fn reconsidered(
    opening: Opening,
    candidate: &Candidate,
    target: Target,
    pane: &Pane,
) -> Opening {
    if candidate.kind != Kind::Osc8 || matches!(opening, Opening::Refused(_)) {
        return opening;
    }
    match misleading(&candidate.text, &target, pane) {
        Some(said) => ask_first(&candidate.text, target, &said),
        None => opening,
    }
}

/// The ask for a marked link whose words say `said`, which its `target` is not.
fn ask_first(text: &str, target: Target, said: &str) -> Opening {
    Opening::Ask {
        reason: format!("its text says {said}"),
        shown: text.to_owned(),
        target,
        token: None,
        about: Asking::Misleading,
    }
}

/// **What a marked link's `text` says that its `target` is not**, in the words the ask shows
/// ("docs.purlis.dev", "src/lib.rs"), or `None` when no word of it reads as an address or a path
/// naming another target. Reads no file: a path is compared by its name, resolved through its
/// links where it is absolute.
pub fn misleading(text: &str, target: &Target, pane: &Pane) -> Option<String> {
    if text.chars().count() > MOST_TEXT {
        let start: String = text.chars().take(64).collect();
        return Some(format!("{start}…"));
    }
    let seen = as_seen(text);
    words(&seen)
        .flat_map(reads_of)
        .find_map(|reads| differs(&reads, target, pane))
}

/// **`text` as a reader sees it**: characters that draw nothing dropped (a blank one is a
/// space), wide letters and look-alike dots folded (NFKC, and the ideographic full stops).
fn as_seen(text: &str) -> String {
    let drawn: String = text
        .chars()
        .filter_map(|c| {
            if purlis_unicode::invisible(c) {
                c.is_whitespace().then_some(' ')
            } else {
                Some(c)
            }
        })
        .collect();
    drawn
        .nfkc()
        .map(|c| match c {
            '\u{3002}' | '\u{FF61}' | '\u{FE12}' | '\u{FE52}' | '\u{2024}' => '.',
            c => c,
        })
        .collect()
}

/// Whether `c` is punctuation or a symbol, in Unicode's classes.
fn punctuation(c: char) -> bool {
    matches!(
        c.general_category_group(),
        GeneralCategoryGroup::Punctuation | GeneralCategoryGroup::Symbol
    )
}

/// **The words of `seen`**, each with the punctuation a sentence puts around it taken off. A
/// word ends at a space, a quote, or punctuation no address is written with (a dash, a bracket
/// of another script, an ellipsis).
fn words(seen: &str) -> impl Iterator<Item = &str> {
    seen.split(|c: char| {
        c.is_whitespace() || matches!(c, '\'' | '"' | '`') || (!c.is_ascii() && punctuation(c))
    })
    .map(trimmed)
    .filter(|word| !word.is_empty())
}

/// `word` without the punctuation before and after it: never a path's leading `/`, `~/`, `./`
/// or `../`, nor a trailing `/`.
fn trimmed(word: &str) -> &str {
    let mut start = word;
    while let Some(c) = start.chars().next() {
        let keeps = c == '/'
            || start.starts_with("~/")
            || start.starts_with("./")
            || start.starts_with("../");
        if keeps || !punctuation(c) {
            break;
        }
        start = &start[c.len_utf8()..];
    }
    start.trim_end_matches(|c: char| c != '/' && punctuation(c))
}

/// What one word of a marked link's text reads as.
#[derive(Debug)]
enum Reads {
    /// A web address, or a host with a path or a port after it.
    Address {
        host: String,
        /// The scheme it was written with, if it was.
        scheme: Option<String>,
        /// The port it names, if it does.
        port: Option<u16>,
        /// A name written before an `@`, which a reader takes for the host.
        before_at: Option<String>,
    },
    /// A path: its words, without a `:line[:col]`.
    Path(String),
    /// A name with a dot in it and nothing after it, which reads as a host (`github.com`) or a
    /// file's name (`README.md`) alike: as it was written, and as a host.
    Name { written: String, host: String },
}

/// What `word` reads as: nothing, or one reading, or two where it has an `@` (an email's
/// domain, and the name before it).
fn reads_of(word: &str) -> Vec<Reads> {
    if let Some((scheme, _)) = word.split_once("://") {
        return vec![addressed(word, scheme)];
    }
    let (words, _, _) = at_line(word);
    if ["/", "~/", "./", "../"]
        .iter()
        .any(|start| words.starts_with(start))
    {
        return vec![Reads::Path(words.to_owned())];
    }
    let (first, slashed) = match words.split_once('/') {
        Some((first, _)) => (first, true),
        None => (words, false),
    };
    if let Some((before, domain)) = first.rsplit_once('@') {
        // An email, or a name before a host: the domain, and the name a reader may take for one.
        let mut read: Vec<Reads> = Vec::new();
        if let Some((host, port)) = host_like(domain) {
            read.push(Reads::Address {
                host,
                scheme: None,
                port,
                before_at: None,
            });
        }
        if let Some((host, _)) = host_like(before) {
            read.push(Reads::Name {
                written: before.to_owned(),
                host,
            });
        }
        if read.is_empty() && slashed {
            read.push(Reads::Path(words.to_owned()));
        }
        return read;
    }
    match host_like(first) {
        Some((host, port)) if slashed || port.is_some() => vec![Reads::Address {
            host,
            scheme: None,
            port,
            before_at: None,
        }],
        Some((host, _)) => vec![Reads::Name {
            written: words.to_owned(),
            host,
        }],
        None if slashed => vec![Reads::Path(words.to_owned())],
        None => Vec::new(),
    }
}

/// `word`, written with `scheme://`, as what it reads as: a `file:` address its path, any other
/// its host, scheme, port and any name before an `@`.
fn addressed(word: &str, scheme: &str) -> Reads {
    let parsed = url::Url::parse(word).ok();
    if scheme.eq_ignore_ascii_case("file") {
        let path = parsed
            .and_then(|url| url.to_file_path().ok())
            .map_or_else(|| word.to_owned(), |path| path.display().to_string());
        return Reads::Path(path);
    }
    let Some(url) = parsed else {
        return Reads::Address {
            host: word.to_lowercase(),
            scheme: None,
            port: None,
            before_at: None,
        };
    };
    let before_at = match (url.username(), url.password()) {
        ("", None) => None,
        (name, None) => Some(name.to_owned()),
        (name, Some(password)) => Some(format!("{name}:{password}")),
    };
    Reads::Address {
        host: url.host_str().unwrap_or_default().to_owned(),
        scheme: Some(url.scheme().to_owned()),
        port: url.port(),
        before_at,
    }
}

/// `word` as a host, and the port after it, if it reads as one: two labels at least, the last
/// letters only and two of them at least (`v1.2` and `e.g` are none). The host as a browser
/// would be handed it: lowercased, and an international name in its ASCII form.
fn host_like(word: &str) -> Option<(String, Option<u16>)> {
    let (host, port) = match word.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => {
            (host, Some(port.parse::<u16>().ok()?))
        }
        _ => (word, None),
    };
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
    if !reads {
        return None;
    }
    let ascii = url::Url::parse(&format!("https://{host}/"))
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_else(|| host.to_lowercase());
    Some((ascii, port))
}

/// **What `reads` says that `target` is not**, or `None` where it names it: the same host, port
/// and no weaker scheme, or the same file.
fn differs(reads: &Reads, target: &Target, pane: &Pane) -> Option<String> {
    match (reads, target) {
        (
            Reads::Address {
                host,
                scheme,
                port,
                before_at,
            },
            Target::Url(url),
        ) => {
            if let Some(before) = before_at {
                return Some(before.clone());
            }
            if url.host_str() != Some(host.as_str()) {
                return Some(host.clone());
            }
            if scheme.as_deref() == Some("https") && url.scheme() == "http" {
                return Some(format!("https://{host}"));
            }
            // A port either names outright: the text's against the target's, or a target's own
            // port where the text names none. A scheme's own port is no port named.
            let differs = match port {
                Some(said) => url.port_or_known_default() != Some(*said),
                None => url.port().is_some(),
            };
            differs.then(|| match port {
                Some(port) => format!("{host}:{port}"),
                None => host.clone(),
            })
        }
        (Reads::Address { host, .. }, Target::File(_)) => Some(host.clone()),
        (Reads::Path(path), Target::File(file)) => {
            (!same_file(path, &file.path, pane)).then(|| path.clone())
        }
        (Reads::Path(path), Target::Url(_)) => Some(path.clone()),
        // A name names a web address by its host alone, and on its scheme's own port.
        (Reads::Name { written, host }, Target::Url(url)) => {
            (url.host_str() != Some(host.as_str()) || url.port().is_some()).then(|| written.clone())
        }
        (Reads::Name { written, .. }, Target::File(file)) => (file.path.file_name()
            != Some(std::ffi::OsStr::new(written.as_str())))
        .then(|| written.clone()),
    }
}

/// Whether `words`, a path as a program printed it, names `file` (absolute, resolved): the
/// same path from `/` (resolved through its links), the same from the chat's folder where it
/// climbs out with `..`, and otherwise the end of `file`'s path.
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

#[cfg(test)]
#[path = "osc8_tests.rs"]
mod tests;
