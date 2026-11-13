// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A link a program marked itself** (OSC 8, #6218, spec #6208): words on the screen, and the
//! target the program named for them, which the pane hands over as it was handed them.
//!
//! - **The target is read by its scheme's kind** ([`kind_of`]): `http:` and `https:` by the web
//!   address's rules, `file:` by a file's (one on another host is refused, ruling Q2), and
//!   every other scheme is refused with a sentence that names it. A target longer than
//!   [`MOST_TARGET`] is refused unread.
//! - **Words that may name another address or path ask first** ([`reconsidered`], ruling Q1):
//!   it is an allowlist ([`misleading`]). Text such as "Click here", whose every word is plain
//!   ASCII that builds no host or path, opens its target the way a found link does, and so do
//!   words a browser reads as the target itself (`github.com/purlis` over
//!   `https://github.com/purlis`). Every other word asks, showing both: another host, port,
//!   scheme or path (`github.com/someone-else` under the text `github.com/purlis`), another
//!   file, and words the check cannot read.

use std::path::{Component, Path, PathBuf};

use unicode_normalization::UnicodeNormalization as _;

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
/// ("docs.purlis.dev", "src/lib.rs"), or `None` when the text is shown to name nothing else.
///
/// It is an allowlist: every word of the text, as a reader sees it ([`seen`]), must be
/// [`plain`] (it can build no host or path) or name the target itself ([`names_web`],
/// [`names_file`]). Anything else asks: a false ask costs one click, and a missed one opens the
/// wrong place. Reads no file's contents: a path is compared by its name, resolved through its
/// links where it is absolute.
pub fn misleading(text: &str, target: &Target, pane: &Pane) -> Option<String> {
    if text.chars().count() > MOST_TEXT {
        let start: String = text.chars().take(64).collect();
        return Some(format!("{start}…"));
    }
    let seen = seen(text);
    seen.split(char::is_whitespace).find_map(|word| {
        let word = trimmed(word);
        if word.is_empty() || plain(word) {
            return None;
        }
        match target {
            Target::Url(url) => names_web(word, url),
            Target::File(file) => names_file(word, &file.path, pane),
            // A marked link's target is read only as a web address or a file, never as a chat.
            Target::Chat(_) => Some(word.to_owned()),
        }
    })
}

/// **`text` as a reader sees it**: characters that draw nothing dropped (those that are space
/// read as one), then wide letters and look-alike forms folded (NFKC).
fn seen(text: &str) -> String {
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
    drawn.nfkc().collect()
}

/// The punctuation a plain word may hold: none of it builds a host, a port or a path.
const PLAIN_PUNCTUATION: &str = "!\"$&'()*+,-<=>^_`{|}~";

/// **Whether `word` is plain**: ASCII letters, digits and [`PLAIN_PUNCTUATION`] only, so no
/// `.`, `:`, `/`, `\`, `%`, `@`, `#`, `?`, `;`, `[` or `]`, and no character outside ASCII. A
/// word that is `localhost` is no plain word.
fn plain(word: &str) -> bool {
    !word.eq_ignore_ascii_case("localhost")
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || PLAIN_PUNCTUATION.contains(c))
}

/// The ASCII punctuation a sentence puts around a word, which [`trimmed`] takes off.
const WRAPPERS: &str = "()[]{}<>\"'`*_,;:!?.";

/// `word` without the ASCII punctuation a sentence puts around it ([`WRAPPERS`]): never a path's
/// leading `./` or `../`, nor the brackets of an IPv6 address (`[::1]:8080`).
fn trimmed(word: &str) -> &str {
    let start = if word.starts_with("./") || word.starts_with("../") || bracketed_ip(word) {
        word
    } else {
        word.trim_start_matches(|c| WRAPPERS.contains(c))
    };
    let ip = bracketed_ip(start);
    start.trim_end_matches(|c| WRAPPERS.contains(c) && !(ip && c == ']'))
}

/// Whether `text` starts with an IPv6 address in brackets.
fn bracketed_ip(text: &str) -> bool {
    text.strip_prefix('[')
        .and_then(|inner| inner.split_once(']'))
        .is_some_and(|(inner, _)| inner.parse::<std::net::Ipv6Addr>().is_ok())
}

/// **What `word` says that the web address `url` is not**, or `None` when it names `url`
/// itself. It is read as a browser reads an address (WHATWG): as written where it names an
/// `http`, `https` or `file` scheme, and otherwise with `url`'s scheme before it. Then its
/// scheme, host (IDNA applied, a `www.` before it and a dot after it no difference) and port
/// must be `url`'s, and where it shows them, its name and password, path, query and fragment.
fn names_web(word: &str, url: &url::Url) -> Option<String> {
    let said = url::Url::parse(word)
        .ok()
        .filter(|said| matches!(said.scheme(), "http" | "https" | "file"))
        .or_else(|| url::Url::parse(&format!("{}://{word}", url.scheme())).ok());
    let Some(said) = said else {
        return Some(word.to_owned());
    };
    if host_key(&said) != host_key(url) {
        return Some(said.host_str().unwrap_or(word).to_owned());
    }
    let same = said.scheme() == url.scheme()
        && said.port_or_known_default() == url.port_or_known_default()
        && ((said.username(), said.password()) == ("", None)
            || (said.username(), said.password()) == (url.username(), url.password()))
        && (said.path() == "/"
            || said.path().trim_end_matches('/') == url.path().trim_end_matches('/'))
        && (said.query().is_none() || said.query() == url.query())
        && (said.fragment().is_none() || said.fragment() == url.fragment());
    (!same).then(|| word.to_owned())
}

/// `url`'s host as two addresses of the same place share it: no `www.` before it, no dot after.
fn host_key(url: &url::Url) -> Option<String> {
    let host = url.host_str()?.trim_end_matches('.');
    Some(host.strip_prefix("www.").unwrap_or(host).to_owned())
}

/// **What `word` says that the file `file` is not**, or `None` when it names `file`: a path or a
/// file's name (`src/main.rs`, `main.rs:12`, `/abs/path`), or a `file:` address on this machine.
/// A web address, a host or an email names no file.
fn names_file(word: &str, file: &Path, pane: &Pane) -> Option<String> {
    let (words, _, _) = at_line(word);
    let path = if words
        .get(..5)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("file:"))
    {
        let Some(path) = url::Url::parse(words)
            .ok()
            .filter(|said| said.scheme() == "file")
            .and_then(|said| said.to_file_path().ok())
        else {
            return Some(word.to_owned());
        };
        path.display().to_string()
    } else {
        words.to_owned()
    };
    (!same_file(&path, file, pane)).then(|| word.to_owned())
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
