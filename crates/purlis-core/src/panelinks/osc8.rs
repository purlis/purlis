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
use unicode_properties::{GeneralCategory, GeneralCategoryGroup, UnicodeGeneralCategory as _};

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
/// naming another target. Where a word's reading as a host is uncertain, it says so: a false ask
/// costs one click, and a missed one opens the wrong place. Reads no file's contents: a path is
/// compared by its name, resolved through its links where it is absolute.
pub fn misleading(text: &str, target: &Target, pane: &Pane) -> Option<String> {
    if text.chars().count() > MOST_TEXT {
        let start: String = text.chars().take(64).collect();
        return Some(format!("{start}…"));
    }
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
    words(&drawn).find_map(|word| {
        if let Some(said) = uncertain(word) {
            return Some(said);
        }
        let seen = folded(word);
        let seen = trimmed(&seen);
        if seen.is_empty() {
            return None;
        }
        match target {
            Target::Url(url) => differs_from_web(seen, url),
            Target::File(file) => differs_from_file(seen, &file.path, pane),
            // A marked link's target is read only as a web address or a file, never as a chat.
            Target::Chat(_) => None,
        }
    })
}

/// Whether `c` is punctuation or a symbol, in Unicode's classes.
fn punctuation(c: char) -> bool {
    matches!(
        c.general_category_group(),
        GeneralCategoryGroup::Punctuation | GeneralCategoryGroup::Symbol
    )
}

/// Whether `c` is a combining mark: it draws over the character before it.
fn mark(c: char) -> bool {
    c.general_category_group() == GeneralCategoryGroup::Mark
}

/// Whether `c`, inside a word, may stand for a dot: any character outside ASCII that is no
/// letter or digit of a word (punctuation, a symbol, a modifier letter or a mark).
fn may_be_a_dot(c: char) -> bool {
    !c.is_ascii()
        && (punctuation(c) || mark(c) || c.general_category() == GeneralCategory::ModifierLetter)
}

/// **The words of `text`**: it split at spaces, quotes, and the brackets and dashes no address
/// is written with. Punctuation inside a word stays, where it may stand for a dot.
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| {
        c.is_whitespace()
            || matches!(c, '\'' | '"' | '`')
            || (!c.is_ascii()
                && matches!(
                    c.general_category(),
                    GeneralCategory::OpenPunctuation
                        | GeneralCategory::ClosePunctuation
                        | GeneralCategory::InitialPunctuation
                        | GeneralCategory::FinalPunctuation
                        | GeneralCategory::DashPunctuation
                ))
    })
    .filter(|word| !word.is_empty())
}

/// **`word` as a reader sees it**: wide letters folded (NFKC), marks dropped, and the
/// ideographic full stops read as dots.
fn folded(word: &str) -> String {
    let unmarked: String = word.nfkd().filter(|c| !mark(*c)).collect();
    unmarked
        .nfkc()
        .map(|c| match c {
            '\u{3002}' | '\u{FF61}' | '\u{FE12}' | '\u{FE52}' | '\u{2024}' => '.',
            c => c,
        })
        .collect()
}

/// **Why `word` is uncertain**, the words the ask shows, or `None`: a mark in a word that reads
/// as a host without it or with it read as a dot, or a character that may stand for a dot
/// ([`may_be_a_dot`]) in a word that reads as a host with it read as one.
fn uncertain(word: &str) -> Option<String> {
    let kept: String = word
        .nfkc()
        .map(|c| match c {
            '\u{3002}' | '\u{FF61}' | '\u{FE12}' | '\u{FE52}' | '\u{2024}' => '.',
            c => c,
        })
        .collect();
    let kept = trimmed(&kept);
    if !kept.chars().any(may_be_a_dot) {
        return None;
    }
    let bare: String = kept.chars().filter(|c| !mark(*c)).collect();
    let mut dotted = String::new();
    for c in kept.chars() {
        let c = if may_be_a_dot(c) { '.' } else { c };
        if !(c == '.' && dotted.ends_with('.')) {
            dotted.push(c);
        }
    }
    let dotted = dotted.trim_matches('.');
    let reads = |text: &str| authority(text.split('/').next().unwrap_or_default()).is_some();
    (reads(&bare) || reads(dotted)).then(|| {
        if reads(&bare) && bare.chars().all(|c| !may_be_a_dot(c)) {
            bare
        } else {
            kept.to_owned()
        }
    })
}

/// `word` without the punctuation before and after it: never a path's leading `/`, `~/`, `./`
/// or `../`, nor a trailing `/`, nor the brackets of an IPv6 address (`[::1]:8080`).
fn trimmed(word: &str) -> &str {
    let mut start = word;
    while let Some(c) = start.chars().next() {
        let keeps = c == '/'
            || start.starts_with("~/")
            || start.starts_with("./")
            || start.starts_with("../")
            || bracketed_ip(start).is_some();
        if keeps || !punctuation(c) {
            break;
        }
        start = &start[c.len_utf8()..];
    }
    let kept = start.trim_end_matches(|c: char| c != '/' && punctuation(c));
    match bracketed_ip(start) {
        // Its closing bracket, and a port after it, stay.
        Some(closes) if kept.len() < closes => &start[..closes],
        _ => kept,
    }
}

/// Where the `[…]` around an IPv6 address `text` starts with ends (just past its `]`), if it
/// does start with one.
fn bracketed_ip(text: &str) -> Option<usize> {
    let inner = text.strip_prefix('[')?;
    let end = inner.find(']')?;
    inner[..end]
        .parse::<std::net::Ipv6Addr>()
        .is_ok()
        .then_some(end + 2)
}

/// A host a word names, as a browser would be handed it, and the port it names, if it does.
#[derive(Debug)]
struct Authority {
    host: String,
    port: Option<u16>,
    /// Written as a name and no more (`github.com`, `README.md`), which reads as a host or a
    /// file's name alike; not with a port, nor as `localhost` or an IP address.
    name_only: bool,
}

/// **`text` as a host and a port, if it reads as one**: an IP address (`127.0.0.1`, `[::1]`),
/// `localhost`, a single name with a port (`dev:3000`), or two labels at least whose last is
/// letters only (`v1.2` and `e.g` are none) or an international one (`xn--p1ai`). A host the
/// browser could not read is still a host here, so it differs from every target.
fn authority(text: &str) -> Option<Authority> {
    if text.is_empty() {
        return None;
    }
    let (host, port) = if text.starts_with('[') {
        let end = text.find(']')?;
        let rest = &text[end + 1..];
        let port = match rest.strip_prefix(':') {
            Some(port) => Some(port.parse::<u16>().ok()?),
            None if rest.is_empty() => None,
            None => return None,
        };
        (&text[..=end], port)
    } else {
        match text.rsplit_once(':') {
            Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => {
                (host, Some(port.parse::<u16>().ok()?))
            }
            Some(_) => return None,
            None => (text, None),
        }
    };
    if host.is_empty() {
        return None;
    }
    let ip = host
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .is_some_and(|inner| inner.parse::<std::net::Ipv6Addr>().is_ok())
        || host.parse::<std::net::Ipv4Addr>().is_ok();
    let labels: Vec<&str> = host.split('.').collect();
    let labelled = labels.iter().all(|label| {
        !label.is_empty()
            && label
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
    });
    let named = labelled
        && match labels.as_slice() {
            [one] => {
                one.eq_ignore_ascii_case("localhost")
                    || (port.is_some() && one.chars().any(char::is_alphabetic))
            }
            [.., last] => {
                (last.chars().count() >= 2 && last.chars().all(char::is_alphabetic))
                    || last
                        .get(..4)
                        .is_some_and(|start| start.eq_ignore_ascii_case("xn--"))
            }
            [] => false,
        };
    if !ip && !named {
        return None;
    }
    let read = url::Url::parse(&format!("https://{host}/"))
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_else(|| host.to_lowercase());
    Some(Authority {
        host: read,
        port,
        name_only: !ip && port.is_none() && labels.len() >= 2,
    })
}

/// Whether `text` starts as a path does: from `/`, `~/`, `./` or `../`.
fn path_like(text: &str) -> bool {
    ["/", "~/", "./", "../"]
        .iter()
        .any(|start| text.starts_with(start))
}

/// **What `word` says that the web address `url` is not**: another host or port, a weaker scheme,
/// a name before an `@`, an email's domain, or a path or a file.
fn differs_from_web(word: &str, url: &url::Url) -> Option<String> {
    if let Some((scheme, _)) = word.split_once("://") {
        let Ok(said) = url::Url::parse(word) else {
            return Some(word.to_owned());
        };
        if scheme.eq_ignore_ascii_case("file") {
            return Some(word.to_owned());
        }
        match (said.username(), said.password()) {
            ("", None) => {}
            (name, None) => return Some(name.to_owned()),
            (name, Some(password)) => return Some(format!("{name}:{password}")),
        }
        let host = said.host_str().unwrap_or_default().to_owned();
        if url.host_str() != Some(host.as_str()) {
            return Some(host);
        }
        if said.scheme() == "https" && url.scheme() == "http" {
            return Some(format!("https://{host}"));
        }
        return ports_differ(said.port(), url).then(|| at_port(&host, said.port()));
    }
    if path_like(word) {
        return Some(word.to_owned());
    }
    let first = word.split('/').next().unwrap_or_default();
    if let Some((before, domain)) = first.rsplit_once('@') {
        // A name a reader may take for the host, then an email's domain.
        if let Some(name) = authority(before)
            && url.host_str() != Some(name.host.as_str())
        {
            return Some(before.to_owned());
        }
        let domain = authority(domain)?;
        return (url.host_str() != Some(domain.host.as_str()) || ports_differ(domain.port, url))
            .then(|| at_port(&domain.host, domain.port));
    }
    let Some(said) = authority(first) else {
        // Words with a `/` and no host before it are a path.
        return word.contains('/').then(|| word.to_owned());
    };
    if said.name_only && !word.contains('/') {
        // A name names a web address by its host alone, on its scheme's own port.
        return (url.host_str() != Some(said.host.as_str()) || url.port().is_some())
            .then(|| word.to_owned());
    }
    if url.host_str() != Some(said.host.as_str()) {
        return Some(at_port(&said.host, said.port));
    }
    ports_differ(said.port, url).then(|| at_port(&said.host, said.port))
}

/// Whether a port the text names (or none) differs from `url`'s: the text's against the
/// target's, or the target's own where the text names none. A scheme's own port is none named.
fn ports_differ(said: Option<u16>, url: &url::Url) -> bool {
    match said {
        Some(said) => url.port_or_known_default() != Some(said),
        None => url.port().is_some(),
    }
}

/// `host`, with `:port` where the text named one.
fn at_port(host: &str, port: Option<u16>) -> String {
    match port {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    }
}

/// **What `word` says that the file `file` is not**: another path or file name, or a web
/// address, a host or an email.
fn differs_from_file(word: &str, file: &Path, pane: &Pane) -> Option<String> {
    if let Some((scheme, _)) = word.split_once("://") {
        if !scheme.eq_ignore_ascii_case("file") {
            return Some(word.to_owned());
        }
        let path = url::Url::parse(word)
            .ok()
            .and_then(|url| url.to_file_path().ok())
            .map_or_else(|| word.to_owned(), |path| path.display().to_string());
        return (!same_file(&path, file, pane)).then_some(path);
    }
    let (words, _, _) = at_line(word);
    if path_like(words) {
        return (!same_file(words, file, pane)).then(|| words.to_owned());
    }
    if let Some((first, _)) = words.split_once('/') {
        // A host before a path is a web address; anything else is a path.
        if authority(first).is_some_and(|said| said.name_only || said.port.is_some()) {
            return Some(first.to_owned());
        }
        return (!same_file(words, file, pane)).then(|| words.to_owned());
    }
    if let Some((_, domain)) = words.rsplit_once('@') {
        return authority(domain).map(|said| said.host);
    }
    // `main.rs:12` is a file's name at a line before it is a host at a port.
    let said = authority(words).or_else(|| authority(word))?;
    if said.name_only {
        // A name names a file by its own name.
        return (file.file_name() != Some(std::ffi::OsStr::new(words))).then(|| words.to_owned());
    }
    Some(word.to_owned())
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
