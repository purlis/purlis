// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A name as a case-folding disk compares it: the one fold every check of a denied name by
//! name uses, so none of them reads a name the disk would take for another as a third.

use unicode_normalization::UnicodeNormalization as _;

/// `name` as a case-folding disk may compare it. APFS and HFS+ compare names in one canonical
/// form, so the name is decomposed first (`é` written whole and written as `e` with its accent
/// are one name there), and each character is then taken to its lower case, its upper case and
/// its lower case again. A character whose lower case alone is not the disk's fold (one with no
/// lower case of its own, or whose upper case is two letters) comes out as the letters the disk
/// takes it for. The dotless `ı` is kept as it is, as the disk keeps it: its upper case is a
/// plain `I`, but no such disk takes it for `i`.
///
/// A name of plain ASCII is lower-cased and nothing more, which is what all of this comes to
/// for it.
pub fn folded(name: &str) -> String {
    #[cfg(test)]
    MADE.with(|made| made.set(made.get() + 1));
    if name.is_ascii() {
        return name.to_ascii_lowercase();
    }
    name.nfd()
        .flat_map(|c| {
            let kept = (c == DOTLESS_I).then_some(c);
            let folded: Vec<char> = kept.map_or_else(
                || {
                    c.to_lowercase()
                        .flat_map(char::to_uppercase)
                        .flat_map(char::to_lowercase)
                        .collect()
                },
                |c| vec![c],
            );
            folded
        })
        .nfd()
        .collect()
}

/// `ı`, which a case-folding disk keeps apart from `i` though its upper case is `I`.
const DOTLESS_I: char = '\u{131}';

#[cfg(test)]
thread_local! {
    /// How many names [`folded`] has folded on this thread.
    static MADE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many names [`folded`] has folded on this thread: what a test of a check's cost reads.
#[cfg(test)]
pub(crate) fn folds_made() -> usize {
    MADE.with(std::cell::Cell::get)
}

/// Characters a case-folding disk takes for plain letters, written out by hand rather than read
/// from the case tables [`folded`] reads, so a test of a check by name tries what that fold may
/// miss: each pair is letters and a character APFS finds a file of those letters by, as asked of
/// the disk itself (`every_spelling_the_tests_try_is_what_the_disk_says`).
#[cfg(test)]
const ALIKE: &[(&str, char)] = &[
    ("s", '\u{17F}'),   // ſ, the long s
    ("k", '\u{212A}'),  // the Kelvin sign
    ("ss", '\u{DF}'),   // ß
    ("ss", '\u{1E9E}'), // ẞ
    ("ff", '\u{FB00}'), // ﬀ
    ("fi", '\u{FB01}'), // ﬁ
    ("fl", '\u{FB02}'), // ﬂ
    ("st", '\u{FB06}'), // ﬆ
];

/// Spellings a case-folding disk keeps apart from plain letters, though a fold made of case
/// mappings alone could take them for those letters: letters, and a character that is another
/// name on the disk.
#[cfg(test)]
pub(crate) const APART: &[(&str, char)] = &[
    ("i", '\u{131}'), // ı, the dotless i: its upper case is I, but the disk keeps it
];

/// `name` with one of its letters, or two that run together, spelled each other way a disk that
/// folds case takes them ([`ALIKE`]), one spelling at a time; and, for a name with an accented
/// letter, the name with every such letter written whole and written as a letter and its accent.
#[cfg(test)]
pub(crate) fn spelled_alike(name: &str) -> Vec<String> {
    let lower = name.to_ascii_lowercase();
    let mut out = Vec::new();
    for (letters, one) in ALIKE {
        let mut from = 0;
        while let Some(found) = lower[from..].find(letters) {
            let at = from + found;
            out.push(format!(
                "{}{one}{}",
                &name[..at],
                &name[at + letters.len()..]
            ));
            from = at + 1;
        }
    }
    let whole: String = name.nfc().collect();
    let apart: String = name.nfd().collect();
    out.extend([whole, apart].into_iter().filter(|it| it != name));
    out
}

/// `name` with one of its letters spelled as a character the disk keeps apart from it
/// ([`APART`]): a name that is not the name's own on the disk, for a test that a check by name
/// takes nothing more than the disk does.
#[cfg(test)]
pub(crate) fn spelled_apart(name: &str) -> Vec<String> {
    let lower = name.to_ascii_lowercase();
    let mut out = Vec::new();
    for (letters, one) in APART {
        if let Some(at) = lower.find(letters) {
            out.push(format!(
                "{}{one}{}",
                &name[..at],
                &name[at + letters.len()..]
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{ALIKE, APART, folded, spelled_alike};
    use std::collections::BTreeSet;
    use unicode_normalization::UnicodeNormalization as _;

    /// Every character of Unicode's first plane, but the two no file name holds and the one
    /// that names the folder itself. No character past it is a case of a letter of the first.
    fn every_character() -> impl Iterator<Item = char> {
        (0..=0xFFFF)
            .filter_map(char::from_u32)
            .filter(|c| !matches!(c, '\0' | '/' | '.'))
    }

    /// A folder on a volume that folds case, or `None` where this one does not.
    fn a_folding_folder() -> Option<tempfile::TempDir> {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a"), "a").unwrap();
        let folds = dir.path().join("A").exists();
        std::fs::remove_file(dir.path().join("a")).unwrap();
        folds.then_some(dir)
    }

    /// Asked of the disk itself, both ways: on a volume that folds case, every one-character
    /// name the disk finds as a file of plain letters is folded to that file's name, and every
    /// one the fold takes to plain letters is found as that file. Where the volume does not
    /// fold case, there is nothing to ask.
    #[test]
    fn the_fold_takes_a_name_to_plain_letters_exactly_where_the_disk_does() {
        let Some(dir) = a_folding_folder() else {
            return;
        };
        // Every name of one or two plain letters, whatever the fold says, and each longer one
        // the fold gives.
        let letters = || (b'a'..=b'z').map(char::from);
        let plain = |name: &str| !name.is_empty() && name.bytes().all(|b| b.is_ascii_lowercase());
        let names: BTreeSet<String> = letters()
            .map(String::from)
            .chain(letters().flat_map(|one| letters().map(move |two| format!("{one}{two}"))))
            .chain(
                every_character()
                    .map(|c| folded(&c.to_string()))
                    .filter(|name| plain(name)),
            )
            .collect();
        for name in &names {
            std::fs::write(dir.path().join(name), name).unwrap();
        }
        let mut under = Vec::new();
        let mut over = Vec::new();
        for c in every_character() {
            let spelled = c.to_string();
            let ours = folded(&spelled);
            match std::fs::read_to_string(dir.path().join(&spelled)) {
                Ok(found) if found != ours => {
                    under.push(format!("U+{:04X} is {found:?} on disk", u32::from(c)));
                }
                Err(_) if plain(&ours) => {
                    over.push(format!("U+{:04X} is no {ours:?} on disk", u32::from(c)));
                }
                _ => {}
            }
        }
        assert_eq!((under, over), (Vec::<String>::new(), Vec::<String>::new()));
    }

    /// The spellings the tests try, alike and apart, are what the disk says they are, where the
    /// volume folds case.
    #[test]
    fn every_spelling_the_tests_try_is_what_the_disk_says() {
        let Some(dir) = a_folding_folder() else {
            return;
        };
        for (letters, _) in ALIKE.iter().chain(APART) {
            std::fs::write(dir.path().join(letters), letters).unwrap();
        }
        for (letters, one) in ALIKE {
            let found = std::fs::read_to_string(dir.path().join(one.to_string()));
            assert_eq!(found.ok().as_deref(), Some(*letters), "{one}");
        }
        for (_, one) in APART {
            assert!(!dir.path().join(one.to_string()).exists(), "{one}");
        }
        // A letter and its accent, written whole and written apart, are one name.
        std::fs::write(dir.path().join("caf\u{e9}"), "whole").unwrap();
        let found = std::fs::read_to_string(dir.path().join("cafe\u{301}"));
        assert_eq!(found.ok().as_deref(), Some("whole"));
    }

    #[test]
    fn plain_names_fold_to_their_lower_case() {
        assert_eq!(folded("Purlis.TOML"), "purlis.toml");
        assert_eq!(folded("/Users/Dev/.SSH"), "/users/dev/.ssh");
    }

    /// Written out, not read from any case table: what the disk was found to say.
    #[test]
    fn the_fold_says_what_the_disk_says_of_each_letter_the_tables_disagree_on() {
        // Taken for plain letters.
        for (spelled, plain) in [
            ("\u{17F}", "s"),
            ("\u{212A}", "k"),
            ("\u{DF}", "ss"),
            ("\u{1E9E}", "ss"),
            ("\u{FB03}", "ffi"),
            ("\u{3A3}", "\u{3C3}"),
            ("\u{3C2}", "\u{3C3}"),
        ] {
            assert_eq!(
                folded(spelled),
                folded(plain),
                "{}",
                spelled.escape_unicode()
            );
        }
        // Kept apart: the dotless i, and the dotted capital I, which is an i with its dot.
        assert_ne!(folded("\u{131}"), folded("i"));
        assert_ne!(folded("\u{130}"), folded("i"));
        assert_eq!(folded("\u{130}"), folded("i\u{307}"));
    }

    /// A name with an accented letter is one name, however the letter is written: whole, or as
    /// a letter and its accent, in either case.
    #[test]
    fn a_letter_and_its_accent_written_whole_or_apart_fold_alike() {
        for (one, other) in [
            ("caf\u{e9}", "cafe\u{301}"),
            ("CAF\u{c9}", "cafe\u{301}"),
            ("\u{212B}ngstr\u{f6}m", "a\u{30a}ngstro\u{308}m"),
            ("\u{1E9B}", "s\u{307}"),
        ] {
            assert_eq!(folded(one), folded(other), "{}", one.escape_unicode());
        }
    }

    /// The spellings the tests try never come from the fold they test.
    #[test]
    fn a_name_is_spelled_alike_with_each_other_letter_and_in_each_form() {
        let spelled = spelled_alike("Session");
        assert!(spelled.contains(&"Se\u{17F}sion".to_owned()));
        assert!(spelled.contains(&"Se\u{DF}ion".to_owned()));
        let accented: String = "caf\u{e9}".nfd().collect();
        assert!(spelled_alike("caf\u{e9}").contains(&accented));
    }
}
