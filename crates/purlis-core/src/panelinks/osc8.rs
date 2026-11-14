// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A link a program marked itself** (OSC 8, #6218, spec #6208): words on the screen, and the
//! target the program named for them, which the pane hands over as it was handed them.
//!
//! - **The target is read by its scheme's kind** ([`target_kind`]): `http:` and `https:` by the web
//!   address's rules, `file:` by a file's (one on another host is refused, ruling Q2), and
//!   every other scheme is refused with a sentence that names it. A target longer than
//!   [`MOST_TARGET`] is refused unread.
//! - **Words that name another target ask first** ([`reconsidered`], ruling Q1): a word asks
//!   only if it names a target ([`misleading`]: a scheme, an IP address, `localhost`, a host and
//!   port, a domain under a public suffix, a path) that is not the link's own, or if its letters
//!   may not be what they look like (mixed scripts, a whole-script look-alike, a restricted
//!   letter; UTS #39). Every other word is plain, in any script: "Click here", "v1.2.3",
//!   "#6384", "café", an emoji. `github.com/purlis` over `https://github.com/purlis/purlis`
//!   opens; `github.com/someone-else` over it asks, showing both.

use std::path::{Component, Path, PathBuf};

use unicode_normalization::UnicodeNormalization as _;
use unicode_properties::{GeneralCategory, GeneralCategoryGroup, UnicodeGeneralCategory as _};
use unicode_security::{RestrictionLevel, RestrictionLevelDetection as _};

use super::file::at_line;
use super::{Asking, Candidate, Kind, Opening, Pane, Resolved, Target};

/// The longest target a marked link may name, in bytes: a longer one is refused unread.
pub const MOST_TARGET: usize = 2048;

/// The most of a marked link's words that are read, in characters: longer words ask first.
pub const MOST_TEXT: usize = 4096;

/// **The kind that reads a marked link's `target`**, by its scheme, or the sentence that says
/// why it is not opened.
pub fn target_kind(target: Option<&str>) -> Result<Kind, String> {
    let Some(target) = target.filter(|target| !target.is_empty()) else {
        return Err("the program named no address for this link".into());
    };
    if target.len() > MOST_TARGET {
        return Err("the address this link names is longer than purlis reads".into());
    }
    let Some((scheme, _)) = scheme_of(target) else {
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
/// ("docs.purlis.dev", "src/lib.rs"), or `None` when no word of it names another target.
///
/// A word asks only if it **names a target** ([`names_a_target`]: a scheme, an IP address,
/// `localhost`, a host with a port, a domain under a public suffix, a path) and that target is not
/// `target` ([`names_web`], [`names_file`]), or its letters may not be what they look like
/// ([`reads_as_written`]). Any other word is plain, in any script: "Click here", "v1.2.3",
/// "#6384", "café", an emoji. Each word is read as a reader sees it ([`drawn`], NFKC), and the
/// shape that decides whether it names a target is its UTS #39 skeleton ([`shape`]), so a
/// look-alike of `.`, `:` or `/` counts as the character. Reads no file's contents.
pub fn misleading(text: &str, target: &Target, pane: &Pane) -> Option<String> {
    if text.chars().count() > MOST_TEXT {
        let start: String = text.chars().take(64).collect();
        return Some(format!("{start}…"));
    }
    drawn(text)
        .split(char::is_whitespace)
        .find_map(|original| said_by(original, target, pane))
}

/// What the word `original` says that `target` is not: read with the sentence's punctuation
/// around it taken off, and where that leaves it naming no target, as written (`::1`).
fn said_by(original: &str, target: &Target, pane: &Pane) -> Option<String> {
    let word: String = original.nfkc().collect();
    let file = matches!(target, Target::File(_));
    let trimmed = trimmed(&word);
    let read = [trimmed, word.as_str()]
        .into_iter()
        .find(|read| !read.is_empty() && names_a_target(&shape(read), file))?;
    let differs = match target {
        Target::Url(url) => names_web(read, url),
        Target::File(file) => names_file(read, &file.path, pane),
        // A marked link's target is read only as a web address or a file, never as a chat.
        Target::Chat(_) => Some(read.to_owned()),
    };
    differs.or_else(|| (!reads_as_written(original)).then(|| read.to_owned()))
}

/// **`text` as it is drawn**: characters that draw nothing dropped, those that draw a blank read
/// as a space. A control or a line or paragraph separator draws nothing, so it joins the words
/// around it, as the reader sees them.
fn drawn(text: &str) -> String {
    text.chars()
        .filter_map(|c| {
            if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}') {
                None
            } else if purlis_unicode::invisible(c) {
                c.is_whitespace().then_some(' ')
            } else {
                Some(c)
            }
        })
        .collect()
}

/// The punctuation a sentence puts around a word, which [`trimmed`] takes off.
const WRAPPERS: &str = "()[]{}<>\"'`*_,;:!?.«»“”‘’「」";

/// `word` without the punctuation a sentence puts around it ([`WRAPPERS`]): never a path's
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

/// Characters a reader may take for a dot that UTS #39's skeleton maps to something else.
const DOTS: &str =
    "\u{0589}\u{3002}\u{FF61}\u{06D4}\u{00B7}\u{2027}\u{02F3}\u{0387}\u{30FB}\u{FF65}";

/// **The shape of `word`, which decides whether it names a target**: ASCII as written, in lower
/// case; a character whose UTS #39 skeleton is ASCII punctuation as that punctuation (a
/// look-alike of `.`, `:` or `/`), and one more look-alike of a dot ([`DOTS`]) as `.`; a dash as
/// the end of a name; any other letter or digit as written without its marks, so a host in
/// another script keeps its suffix (`.рф`); any other punctuation as `.`; and marks, symbols,
/// emoji and private-use characters, which build no address, dropped.
fn shape(word: &str) -> String {
    let mut shape = String::with_capacity(word.len());
    for c in word.chars() {
        let skeleton: String = unicode_security::skeleton(c.encode_utf8(&mut [0; 4])).collect();
        if c.is_ascii() {
            shape.push(c.to_ascii_lowercase());
        } else if DOTS.contains(c) {
            shape.push('.');
        } else if !skeleton.is_empty() && skeleton.chars().all(|c| c.is_ascii_punctuation()) {
            shape.push_str(&skeleton);
        } else if c.general_category() == GeneralCategory::DashPunctuation {
            // A dash looks like no part of an address: it ends a name, as a space does.
            shape.push(',');
        } else if c.is_alphanumeric() {
            // Without its marks: `ó` is an `o` with an accent drawn over it.
            shape.extend(
                c.nfd()
                    .filter(|c| c.general_category_group() != GeneralCategoryGroup::Mark)
                    .flat_map(char::to_lowercase),
            );
        } else if c.general_category_group() == GeneralCategoryGroup::Punctuation
            || skeleton.chars().any(|c| DOTS.contains(c) || c == '.')
        {
            shape.push('.');
        }
    }
    shape
}

/// **Whether the [`shape`] of a word names a target**: a scheme (`https:`, `x://`), an IP
/// address (four numbers, a `0x` number, IPv6 in brackets or not), `localhost`, a name with a
/// port (`intranet:8080`), or a domain whose last label is a public suffix (ICANN); and a path
/// (`./x`, `~/x`, `a/b.rs`, `/a/b`), or over a file any file's name (`main.rs`). "1.2.3",
/// "12:30", "e.g", "self.x" and "foo::bar" name none.
fn names_a_target(shape: &str, file: bool) -> bool {
    names_a_place(shape) || names_a_path(shape, file)
}

/// The schemes a word names by its first colon alone, as `https:good.com` does.
const SCHEMES: [&str; 14] = [
    "http",
    "https",
    "file",
    "ftp",
    "ws",
    "wss",
    "javascript",
    "data",
    "vbscript",
    "mailto",
    "blob",
    "about",
    "view-source",
    "ssh",
];

/// Whether `word` starts with a scheme: one of [`SCHEMES`] and a colon, or any scheme and `://`.
fn has_scheme(word: &str) -> bool {
    scheme_of(word).is_some_and(|(scheme, rest)| {
        SCHEMES.contains(&scheme.to_ascii_lowercase().as_str()) || rest.starts_with("//")
    })
}

/// `address`'s scheme and what follows its colon, when it starts with one (RFC 3986: a letter,
/// then letters, digits, `+`, `-` or `.`).
fn scheme_of(address: &str) -> Option<(&str, &str)> {
    address.split_once(':').filter(|(scheme, _)| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
}

/// Whether `shape` names a host a browser would go to: see [`names_a_target`].
fn names_a_place(shape: &str) -> bool {
    if has_scheme(shape) {
        return true;
    }
    // Every name in it counts, before an `@` as after it: a reader takes either for the host.
    let authority = shape
        .split(['/', '\\', '?', '#'])
        .next()
        .unwrap_or_default();
    let host = decoded(authority.rsplit('@').next().unwrap_or_default());
    let authority = decoded(authority);
    bracketed_ip(&host)
        || host.parse::<std::net::Ipv6Addr>().is_ok()
        || authority
            .split(|c: char| !(c.is_alphanumeric() || matches!(c, '-' | '.' | ':')))
            .any(names_a_host)
}

/// Whether `piece`, with no characters a host cannot hold, is a host and maybe a port: any
/// name with a port counts (`intranet:8080`, `localhost.test:5173`), a time (`12:30`) does not.
fn names_a_host(piece: &str) -> bool {
    let (name, port) = match piece.rsplit_once(':') {
        Some((name, port)) if port.chars().all(|c| c.is_ascii_digit()) => (name, Some(port)),
        _ => (piece, None),
    };
    if name.contains(':') {
        return false;
    }
    let name = name.trim_matches('.');
    name == "localhost"
        || port.is_some_and(|port| !port.is_empty())
            && name.starts_with(|c: char| c.is_ascii_alphabetic())
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.'))
        || numbers_a_host(name)
        || public_domain(name)
}

/// Whether `name` is an IPv4 address as a browser reads one and a reader may: four numbers, or
/// up to four where one is written in hex (`0x7f.1`, `0x7f000001`).
fn numbers_a_host(name: &str) -> bool {
    let parts: Vec<&str> = name.split('.').collect();
    let hex = |part: &str| {
        part.get(..2).is_some_and(|x| x.eq_ignore_ascii_case("0x"))
            && part[2..].chars().all(|c| c.is_ascii_hexdigit())
    };
    let number =
        |part: &str| !part.is_empty() && (part.chars().all(|c| c.is_ascii_digit()) || hex(part));
    parts.len() <= 4
        && parts.iter().all(|part| number(part))
        && (parts.len() == 4 || parts.iter().any(|part| hex(part)))
}

/// Whether `name` is a domain of two labels or more whose last label is a public suffix in the
/// list's ICANN section (`good.com`, `README.md`, `good.xn--p1ai`).
fn public_domain(name: &str) -> bool {
    let Ok(ascii) = idna::domain_to_ascii(name) else {
        return false;
    };
    let labels: Vec<&str> = ascii.split('.').collect();
    labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty() && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
        && labels.last().is_some_and(|last| {
            psl::suffix(last.as_bytes()).is_some_and(|suffix| {
                suffix.typ() == Some(psl::Type::Icann) && suffix.as_bytes() == last.as_bytes()
            })
        })
}

/// Whether `shape` names a path: from `/` with a folder in it, from `.`, `..` or `~`, or
/// through a folder to a file's name; and over a file, a file's name alone.
fn names_a_path(shape: &str, file: bool) -> bool {
    let (path, _, _) = at_line(shape);
    let name = path.rsplit(['/', '\\']).next().unwrap_or_default();
    let file_name = name.rsplit_once('.').is_some_and(|(stem, extension)| {
        !stem.is_empty()
            && extension.len() <= 10
            && extension.starts_with(|c: char| c.is_ascii_alphabetic())
            && extension.chars().all(|c| c.is_ascii_alphanumeric())
    });
    let folders = path.contains(['/', '\\']);
    path.starts_with("./")
        || path.starts_with("../")
        || path.starts_with('~')
        || path.starts_with('/') && (path[1..].contains('/') || file_name || file)
        || folders && (file_name || file)
        || file && file_name
}

/// `text` with its percent-encoded bytes decoded, as a browser decodes a host.
fn decoded(text: &str) -> String {
    percent_encoding::percent_decode_str(text)
        .decode_utf8_lossy()
        .into_owned()
}

/// **Whether the letters of `original` are what they look like**: each run of letters outside
/// ASCII looks the same once folded (NFKC: `ſ` reads as `f` and folds to `s`), is in one script
/// or a highly restrictive mix (UTS #39) with no restricted character, and is no whole-script
/// look-alike of ASCII (Cyrillic `аррӏе`). München passes. A word in ASCII always does.
fn reads_as_written(original: &str) -> bool {
    let skeleton = |text: &str| unicode_security::skeleton(text).collect::<String>();
    original
        .split(|c: char| {
            !(c.is_alphanumeric() || c.general_category_group() == GeneralCategoryGroup::Mark)
        })
        .filter(|run| !run.is_ascii())
        .all(|run| {
            let folded: String = run.nfkc().collect();
            skeleton(run) == skeleton(&folded)
                && (folded.is_ascii()
                    || folded
                        .as_str()
                        .check_restriction_level(RestrictionLevel::HighlyRestrictive)
                        && !skeleton(&folded).is_ascii())
        })
}

/// **What `word` says that the web address `url` is not**, or `None` when it names `url`
/// itself. It is read as a browser reads an address (WHATWG): as written where it names a
/// scheme, and otherwise with `url`'s scheme before it. Then its scheme, host (IDNA applied, a
/// `www.` before it and a dot after it no difference) and port must be `url`'s, and where it
/// shows them, its name and password, query and fragment; and the path it shows must be
/// `url`'s or lead to it, compared with no encoding of a letter, digit or `-._~` (RFC 3986).
fn names_web(word: &str, url: &url::Url) -> Option<String> {
    let said = if has_scheme(word) {
        url::Url::parse(word).ok()
    } else {
        url::Url::parse(&format!("{}://{word}", url.scheme())).ok()
    };
    let Some(said) = said else {
        return Some(word.to_owned());
    };
    if host_key(&said) != host_key(url) {
        return Some(said.host_str().unwrap_or(word).to_owned());
    }
    fn shown(part: Option<&str>) -> Option<&str> {
        part.filter(|part| !part.is_empty())
    }
    let said_path = unreserved(said.path());
    let path = unreserved(url.path());
    let (said_path, path) = (said_path.trim_end_matches('/'), path.trim_end_matches('/'));
    let same = said.scheme() == url.scheme()
        && said.port_or_known_default() == url.port_or_known_default()
        && ((said.username(), said.password()) == ("", None)
            || (said.username(), said.password()) == (url.username(), url.password()))
        && (said_path.is_empty()
            || said_path == path
            || path
                .strip_prefix(said_path)
                .is_some_and(|rest| rest.starts_with('/')))
        && (shown(said.query()).is_none() || shown(said.query()) == shown(url.query()))
        && (shown(said.fragment()).is_none() || shown(said.fragment()) == shown(url.fragment()));
    (!same).then(|| word.to_owned())
}

/// `path` with every percent-encoded letter, digit, `-`, `.`, `_` and `~` decoded, and the
/// hex of every other encoding in capitals (RFC 3986 6.2.2): two spellings of one path agree.
fn unreserved(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut rest = path;
    while let Some(at) = rest.find('%') {
        out.push_str(&rest[..at]);
        let byte = rest
            .get(at + 1..at + 3)
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match byte {
            Some(byte) if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) => {
                out.push(char::from(byte));
            }
            Some(_) => out.push_str(&rest[at..at + 3].to_ascii_uppercase()),
            None => {
                out.push('%');
                rest = &rest[at + 1..];
                continue;
            }
        }
        rest = &rest[at + 3..];
    }
    out.push_str(rest);
    out
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
    let path = if super::file::addressed(words) {
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
    let home;
    let path = if words == "~" || words.starts_with("~/") {
        let Some(dir) = std::env::var_os("HOME") else {
            return false;
        };
        home = PathBuf::from(dir).join(words.trim_start_matches('~').trim_start_matches('/'));
        home.as_path()
    } else if words.starts_with('~') {
        return false;
    } else {
        Path::new(words)
    };
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
