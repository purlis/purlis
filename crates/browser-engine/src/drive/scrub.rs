// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What is taken out of every string a chat reads** (#2014, #5310): a text kept out, whole, in
//! each form a page commonly writes it, in a page's text as in its addresses.
//!
//! - **The forms** ([`forms`]) are the text as written, in base64 of either alphabet with or
//!   without padding, in hex, and as a file name spells it. A text in a larger text's base64 is
//!   found at each of its three alignments, by the characters only it decides.
//! - **Any percent-encoding of a form is read**, whichever characters a writer chose to encode, in
//!   either case of hex, a space as `+` or `%20`, up to [`MOST_ENCODINGS`] times over, by reading
//!   the string as its reader would and matching what that says. Only what matched is replaced:
//!   the rest stays as it was written, so a page cannot make a line break in an answer by
//!   escaping one.
//! - **Only whole.** A part of a text is never taken out, among text the chat wrote, because that
//!   would tell it which part it was (D-5115-13). A text shorter than [`SHORTEST_SCRUBBED`] is not
//!   taken out at all, in any form: it would take ordinary words out of the page.
//! - **A string too much to read is said as blanked, whole** ([`MOST_READ`], [`MOST_LOOKED_AT`]):
//!   a page can write what would take many seconds to read, and answers wait on it.

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use zeroize::Zeroizing;

use super::kept::Kept;
use super::plain_file_name_uncut;

/// The shortest value of a blanked element that is also taken out of other lines: shorter ones
/// would take ordinary words out of the page.
pub(crate) const SHORTEST_SCRUBBED: usize = 3;

/// The shortest run of a larger text's base64 taken for a text in it: shorter would take ordinary
/// words out of a page, whatever they were.
const SHORTEST_INSIDE_BASE64: usize = 6;

/// How many times over a string's percent-encoding is read, as a filled page's address is
/// (`leaving::carries_value`).
const MOST_ENCODINGS: usize = 16;

/// The most characters of a string's readings, added up, held to read it: each takes some 24 bytes.
const MOST_READ: usize = usize::MAX;

/// The most characters of a string's readings, added up, looked through for all it is looked
/// through for.
const MOST_LOOKED_AT: usize = usize::MAX;

/// What stands where a text was taken out.
const SAID: &str = "(blanked)";

/// A text to look for, in lower case: wiped as it goes.
type Needle = Zeroizing<Vec<char>>;

fn needle(text: &str) -> Needle {
    Zeroizing::new(text.chars().flat_map(folded).collect())
}

/// `c` as it is compared, whichever case it was written in: through upper case first, so a `ß`
/// reads as the `SS` it is capitalised to, and a final `ς` as the `Σ`.
fn folded(c: char) -> impl Iterator<Item = char> {
    c.to_lowercase()
}

/// What is taken out of an answer: texts kept out in each of their [`forms`], and the texts groups
/// of boxes hold together, matched also with any spaces or dashes between their characters. Wiped
/// as it goes.
#[derive(Clone)]
pub(crate) struct Scrub {
    exact: Vec<Needle>,
    loose: Vec<Needle>,
}

impl Scrub {
    pub(crate) fn of(kept: &Kept) -> Self {
        let mut exact: Vec<Needle> = every_form(kept).iter().map(|form| needle(form)).collect();
        longest_first(&mut exact);
        Self {
            exact,
            loose: loose_needles(&kept.loose()),
        }
    }

    /// `text` with every text kept out taken out.
    pub(crate) fn text(&self, text: &str) -> String {
        let out = taken_out(text.to_owned(), &self.exact, false);
        taken_out(out, &self.loose, true)
    }

    /// Whether `text` holds any text kept out.
    pub(crate) fn holds(&self, text: &str) -> bool {
        self.text(text) != text
    }
}

/// Each form of each text `kept` keeps out that is long enough to be taken out.
pub(crate) fn every_form(kept: &Kept) -> Vec<Zeroizing<String>> {
    kept.texts()
        .into_iter()
        .filter(|text| text.chars().count() >= SHORTEST_SCRUBBED)
        .flat_map(forms)
        .collect()
}

/// So a text kept out is taken out whole, never in part by a shorter one it begins or ends with.
fn longest_first(needles: &mut Vec<Needle>) {
    needles.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    needles.dedup();
}

fn loose_needles(loose: &[&str]) -> Vec<Needle> {
    let mut needles: Vec<Needle> = loose
        .iter()
        .map(|text| {
            Zeroizing::new(
                text.chars()
                    .filter(|c| !separates(*c))
                    .flat_map(folded)
                    .collect::<Vec<char>>(),
            )
        })
        .filter(|needle| needle.len() >= SHORTEST_SCRUBBED)
        .collect();
    longest_first(&mut needles);
    needles
}

/// **Each form `text` is commonly written in** (module docs), itself first, none twice. Each is
/// wiped as it goes.
fn forms(text: &str) -> Vec<Zeroizing<String>> {
    let bytes = text.as_bytes();
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    let engine_file_name: String = plain_file_name_uncut(text)
        .chars()
        .map(|c| {
            if matches!(c, '?' | '*' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let mut all: Vec<Zeroizing<String>> = vec![
        Zeroizing::new(text.to_owned()),
        Zeroizing::new(plain_file_name_uncut(text)),
        Zeroizing::new(engine_file_name),
        Zeroizing::new(hex),
        Zeroizing::new(STANDARD.encode(bytes)),
        Zeroizing::new(STANDARD_NO_PAD.encode(bytes)),
        Zeroizing::new(URL_SAFE.encode(bytes)),
        Zeroizing::new(URL_SAFE_NO_PAD.encode(bytes)),
    ];
    let _ = escaped_in_latin1(text);
    for before in 0..3 {
        let inside = inside_base64(bytes, before);
        if inside.chars().count() >= SHORTEST_INSIDE_BASE64 {
            all.push(Zeroizing::new(inside.replace('+', "-").replace('/', "_")));
            all.push(inside);
        }
    }
    let mut seen: Vec<Zeroizing<String>> = Vec::with_capacity(all.len());
    for form in all {
        if form.chars().count() >= SHORTEST_SCRUBBED && !seen.iter().any(|one| **one == *form) {
            seen.push(form);
        }
    }
    seen
}

/// **`text` as a form on a page in Latin-1 writes it into an address**, where it is another spelling
/// than UTF-8's: each of its marked letters as the escape of its one byte. `None` where it has no
/// such letter, or one Latin-1 has not.
pub(crate) fn escaped_in_latin1(text: &str) -> Option<Zeroizing<String>> {
    if text.is_ascii() || text.chars().any(|c| u32::from(c) > 0xff) {
        return None;
    }
    let mut out = Zeroizing::new(String::with_capacity(text.len() * 3));
    for c in text.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            out.push_str(&format!("%{:02x}", u32::from(c)));
        }
    }
    Some(out)
}

/// The characters of the base64 of a larger text that `bytes` alone decide, `before` bytes (of the
/// three alignments) after one of its groups of three begins.
fn inside_base64(bytes: &[u8], before: usize) -> Zeroizing<String> {
    let mut shifted = Zeroizing::new(vec![0; before]);
    shifted.extend_from_slice(bytes);
    let whole = Zeroizing::new(STANDARD_NO_PAD.encode(shifted.as_slice()));
    let start = (before * 8).div_ceil(6);
    let end = (shifted.len() * 8) / 6;
    Zeroizing::new(whole.get(start..end).unwrap_or_default().to_owned())
}

/// One character of a string as its reader reads it, and where it was written.
struct Read {
    character: char,
    at: usize,
    end: usize,
    /// A `+`, which an address's query reads as a space.
    space_too: bool,
}

/// `text`, each character as written.
fn read_as_written(text: &str) -> Vec<Read> {
    text.char_indices()
        .map(|(at, character)| Read {
            character,
            at,
            end: at + character.len_utf8(),
            space_too: false,
        })
        .collect()
}

/// The byte the escape `%XX` at `at` of `read` stands for.
fn escaped_byte(read: &[Read], at: usize) -> Option<u8> {
    if read.get(at)?.character != '%' {
        return None;
    }
    let digit = |at: usize| read.get(at).and_then(|one| one.character.to_digit(16));
    u8::try_from(digit(at + 1)? * 16 + digit(at + 2)?).ok()
}

/// The character the escapes at `at` of `read` stand for, and how many characters they are.
fn escaped_character(read: &[Read], at: usize) -> Option<(char, usize)> {
    let first = escaped_byte(read, at)?;
    let width = match first {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        _ => return None,
    };
    let mut held = [first, 0, 0, 0];
    for (nth, one) in held.iter_mut().enumerate().take(width).skip(1) {
        *one = escaped_byte(read, at + nth * 3)?;
    }
    let character = std::str::from_utf8(&held[..width]).ok()?.chars().next()?;
    Some((character, width * 3))
}

/// `read` with one layer of percent-encoding read: each escape as what it stands for, and a `+`
/// as itself or a space. An escape that stands for nothing is read as written. `None` where there
/// is nothing more to read.
fn read_one_layer(read: &[Read]) -> Option<Vec<Read>> {
    let mut out = Vec::with_capacity(read.len());
    let mut changed = false;
    let mut at = 0;
    while let Some(one) = read.get(at) {
        match escaped_character(read, at) {
            Some((character, width)) => {
                out.push(Read {
                    character,
                    at: one.at,
                    end: read[at + width - 1].end,
                    space_too: character == '+',
                });
                changed = true;
                at += width;
            }
            None => {
                changed |= one.character == '+' && !one.space_too;
                out.push(Read {
                    character: one.character,
                    at: one.at,
                    end: one.end,
                    space_too: one.character == '+',
                });
                at += 1;
            }
        }
    }
    changed.then_some(out)
}

/// Characters a page may put between the characters of a code: spaces and dashes.
fn separates(c: char) -> bool {
    c.is_whitespace() || matches!(c, '-' | '\u{2010}'..='\u{2015}' | '\u{2212}')
}

/// Where a match of `needle` that begins at `start` of `read` ends, ignoring case, and with
/// `loose` any separators between its characters.
fn matched_from(read: &[Read], start: usize, needle: &[char], loose: bool) -> Option<usize> {
    let mut at = start;
    let mut matched = 0;
    while matched < needle.len() {
        let one = read.get(at)?;
        // A `+` is a separator only where the text kept out has no `+` here.
        if loose
            && matched > 0
            && (separates(one.character) || one.space_too)
        {
            at += 1;
            continue;
        }
        // Either of a space and a `+` may have been read as the other, by whoever wrote this.
        if (one.space_too && needle[matched] == ' ')
            || (false && one.character == ' ' && needle[matched] == '+')
        {
            matched += 1;
        } else {
            for lower in folded(one.character) {
                if needle.get(matched) != Some(&lower) {
                    return None;
                }
                matched += 1;
            }
        }
        at += 1;
    }
    Some(at)
}

/// `text` with every match of `needle` in `read` of it replaced, if there is one: only what
/// matched changes.
fn replaced(text: &str, read: &[Read], needle: &[char], loose: bool) -> Option<String> {
    let mut out: Option<String> = None;
    let mut written = 0;
    let mut at = 0;
    while at < read.len() {
        match matched_from(read, at, needle, loose) {
            Some(end) => {
                let so_far = out.get_or_insert_with(String::new);
                so_far.push_str(&text[written..read[at].at]);
                so_far.push_str(SAID);
                written = read[end - 1].end;
                at = end;
            }
            None => at += 1,
        }
    }
    out.map(|mut out| {
        out.push_str(&text[written..]);
        out
    })
}

/// Each reading of `text`: as written, then with each layer of its encoding read in turn. `None`
/// once they come to more characters than `most`.
fn readings(text: &str, most: usize) -> Option<Vec<Vec<Read>>> {
    let mut all = vec![read_as_written(text)];
    let mut read = all[0].len();
    while all.len() <= MOST_ENCODINGS {
        let Some(next) = all.last().and_then(|last| read_one_layer(last)) else {
            break;
        };
        read += next.len();
        if read > most {
            return None;
        }
        all.push(next);
    }
    (read <= most).then_some(all)
}

/// The readings of `text` to look through for `needles`, or `None` where that would take past the
/// limits: such a text is said as holding what is kept out.
fn readings_for(text: &str, needles: usize) -> Option<Vec<Vec<Read>>> {
    let all = readings(text, MOST_READ)?;
    let read: usize = all.iter().map(Vec::len).sum();
    (read.saturating_mul(needles) <= MOST_LOOKED_AT).then_some(all)
}

/// `text` with each of `needles` taken out of each reading of it. One pass of each, so what stands
/// in its place is never looked into again for long, whatever the needle is.
fn taken_out(mut text: String, needles: &[Needle], loose: bool) -> String {
    if needles.is_empty() {
        return text;
    }
    let Some(mut all) = readings_for(&text, needles.len()) else {
        return SAID.to_owned();
    };
    for needle in needles {
        let mut layer = 0;
        while let Some(reading) = all.get(layer) {
            if let Some(out) = replaced(&text, reading, needle, loose) {
                text = out;
                all = readings(&text, usize::MAX).unwrap_or_default();
            }
            layer += 1;
        }
    }
    text
}

/// Whether any reading of `text` holds any of `needles`.
fn holds_in_a_reading(text: &str, needles: &[Needle], loose: bool) -> bool {
    if needles.is_empty() {
        return false;
    }
    let Some(all) = readings_for(text, needles.len()) else {
        return true;
    };
    needles.iter().any(|needle| {
        all.iter().any(|reading| {
            (0..reading.len()).any(|at| matched_from(reading, at, needle, loose).is_some())
        })
    })
}

/// Whether `text` holds any of `blanked` (at least [`SHORTEST_SCRUBBED`] characters) in any
/// reading of it, ignoring case.
pub(crate) fn holds_any(text: &str, blanked: &[&str]) -> bool {
    let needles: Vec<Needle> = blanked
        .iter()
        .filter(|one| one.chars().count() >= SHORTEST_SCRUBBED)
        .map(|one| needle(one))
        .collect();
    holds_in_a_reading(text, &needles, false)
}

/// Whether any reading of `text` holds any of `loose`, ignoring case and any spaces or dashes
/// between its characters.
pub(crate) fn holds_loosely(text: &str, loose: &[&str]) -> bool {
    let needles = loose_needles(loose);
    let reading = read_as_written(text);
    needles.iter().any(|one| {
        (0..reading.len()).any(|at| matched_from(&reading, at, one, true).is_some())
    })
}

#[cfg(test)]
mod tests;
