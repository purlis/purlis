// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A name as a case-folding disk compares it: the one fold every check of a denied name by
//! name uses, so none of them reads a name the disk would take for another as a third.

/// `name` as a case-folding disk may compare it: each character taken to its lower case, its
/// upper case and its lower case again. A character whose lower case alone is not the disk's
/// fold (one with no lower case of its own, or whose upper case is two letters) comes out as the
/// letters the disk takes it for.
pub fn folded(name: &str) -> String {
    name.chars()
        .flat_map(char::to_lowercase)
        .flat_map(char::to_uppercase)
        .flat_map(char::to_lowercase)
        .collect()
}

/// `name` with each of its letters, one at a time, spelled as another character a disk that
/// folds case may take for it: every character of Unicode's first plane past plain ASCII whose
/// lower case, upper case, or either taken through the other, is that letter, or it and the
/// letter after it. Read from Unicode's cases alone, never from [`folded`], so a test of a check
/// by name tries what the fold it checks may miss.
#[cfg(test)]
pub(crate) fn spelled_alike(name: &str) -> Vec<String> {
    let letters: Vec<char> = name.chars().collect();
    let mut out = Vec::new();
    for at in 0..letters.len() {
        for one in (0x80..=0xFFFF).filter_map(char::from_u32) {
            let lowered = |it: String| it.to_lowercase();
            let cases = [
                lowered(one.to_string()),
                lowered(one.to_uppercase().collect()),
                lowered(one.to_lowercase().collect::<String>().to_uppercase()),
            ];
            let lower = |c: char| c.to_ascii_lowercase();
            let single = cases.contains(&lower(letters[at]).to_string());
            let pair = at + 1 < letters.len()
                && cases.contains(&format!("{}{}", lower(letters[at]), lower(letters[at + 1])));
            if single || pair {
                let rest = if pair { at + 2 } else { at + 1 };
                let spelled: String = letters[..at]
                    .iter()
                    .copied()
                    .chain(std::iter::once(one))
                    .chain(letters[rest..].iter().copied())
                    .collect();
                out.push(spelled);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::folded;
    use std::collections::BTreeSet;

    /// Every character of Unicode's first plane, but the two no file name holds and the one
    /// that names the folder itself. No character past it is a case of a letter of the first.
    fn every_character() -> impl Iterator<Item = char> {
        (0..=0xFFFF)
            .filter_map(char::from_u32)
            .filter(|c| !matches!(c, '\0' | '/' | '.'))
    }

    /// Asked of the disk itself: on a volume that folds case, every one-character name the disk
    /// finds as a file of plain letters is folded to that file's name. Where the volume does
    /// not fold case, there is nothing to ask.
    #[test]
    fn every_name_the_disk_takes_for_one_of_plain_letters_is_folded_to_it() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a"), "a").unwrap();
        if !dir.path().join("A").exists() {
            // This volume does not fold case: there is nothing to compare.
            return;
        }
        // Every name of one or two plain letters, whatever the fold says, and each longer one
        // the fold gives.
        let letters = || (b'a'..=b'z').map(char::from);
        let names: BTreeSet<String> = letters()
            .map(String::from)
            .chain(letters().flat_map(|one| letters().map(move |two| format!("{one}{two}"))))
            .chain(
                every_character()
                    .map(|c| folded(&c.to_string()))
                    .filter(|name| {
                        !name.is_empty() && name.bytes().all(|b| b.is_ascii_lowercase())
                    }),
            )
            .collect();
        for name in &names {
            std::fs::write(dir.path().join(name), name).unwrap();
        }
        let mut missed = Vec::new();
        for c in every_character() {
            let spelled = c.to_string();
            if let Ok(found) = std::fs::read_to_string(dir.path().join(&spelled))
                && found != folded(&spelled)
            {
                missed.push(format!("U+{:04X} is {found:?} on disk", u32::from(c)));
            }
        }
        assert_eq!(missed, Vec::<String>::new());
    }

    #[test]
    fn plain_names_fold_to_their_lower_case() {
        assert_eq!(folded("Purlis.TOML"), "purlis.toml");
        assert_eq!(folded("/Users/Dev/.SSH"), "/users/dev/.ssh");
    }
}
