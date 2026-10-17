// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The texts a browser keeps out of every answer (#2014, #2311), each with the element it was
//! read from. An element is read again and again (a field the person types into, each key), and
//! a read drops that element's earlier texts that it contains: a field keeps its latest value,
//! not every value it held on the way to it. A text read from no element (a run of the person's
//! keys) is kept as it is.

use std::collections::HashSet;

use super::tree::NodeKey;

/// Texts kept out of every answer, each with the element it was read from, if one.
#[derive(Debug, Clone, Default)]
pub(crate) struct Kept {
    texts: Vec<(Option<NodeKey>, String)>,
}

impl Kept {
    /// One read of what `from` holds (or of texts from no element, if `None`): every text read
    /// earlier from `from` that one of `texts` contains goes, and `texts` are kept.
    pub(crate) fn read<S: AsRef<str>>(&mut self, from: Option<&NodeKey>, texts: &[S]) {
        let read: Vec<&str> = texts
            .iter()
            .map(AsRef::as_ref)
            .filter(|text| !text.is_empty())
            .collect();
        if read.is_empty() {
            return;
        }
        if from.is_some() {
            let lower: Vec<String> = read.iter().map(|text| text.to_lowercase()).collect();
            self.texts.retain(|(by, earlier)| {
                by.as_ref() != from || {
                    let earlier = earlier.to_lowercase();
                    !lower.iter().any(|text| text.contains(&earlier))
                }
            });
        }
        for text in read {
            let lower = text.to_lowercase();
            if !self
                .texts
                .iter()
                .any(|(by, kept)| by.as_ref() == from && kept.to_lowercase() == lower)
            {
                self.texts.push((from.cloned(), text.to_owned()));
            }
        }
    }

    /// Takes in every text `other` keeps, each element's texts as one read of it.
    pub(crate) fn take_in(&mut self, other: &Kept) {
        let mut seen: HashSet<Option<&NodeKey>> = HashSet::new();
        for (from, _) in &other.texts {
            if seen.insert(from.as_ref()) {
                let texts: Vec<&str> = other
                    .texts
                    .iter()
                    .filter(|(by, _)| by.as_ref() == from.as_ref())
                    .map(|(_, text)| text.as_str())
                    .collect();
                self.read(from.as_ref(), &texts);
            }
        }
    }

    /// Every text kept.
    pub(crate) fn texts(&self) -> Vec<&str> {
        self.texts.iter().map(|(_, text)| text.as_str()).collect()
    }

    /// Whether nothing is kept.
    pub(crate) fn is_empty(&self) -> bool {
        self.texts.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(backend: i64) -> NodeKey {
        NodeKey {
            frame: "PAGE-1".into(),
            backend,
        }
    }

    #[test]
    fn a_fields_later_value_drops_the_earlier_values_it_contains() {
        let mut kept = Kept::default();
        for value in ["the", "them", "theme"] {
            kept.read(Some(&field(8)), &[value]);
        }
        assert_eq!(kept.texts(), vec!["theme"]);
    }

    #[test]
    fn a_fields_shorter_later_value_is_kept_with_the_longer_one() {
        let mut kept = Kept::default();
        kept.read(Some(&field(8)), &["secret"]);
        kept.read(Some(&field(8)), &["sec"]);
        assert_eq!(kept.texts(), vec!["secret", "sec"]);
    }

    #[test]
    fn a_value_never_drops_another_elements_text_or_a_run() {
        let mut kept = Kept::default();
        kept.read(Some(&field(12)), &["the"]);
        kept.read(None, &["them"]);
        kept.read(Some(&field(8)), &["theme"]);
        assert_eq!(kept.texts(), vec!["the", "them", "theme"]);
    }

    #[test]
    fn texts_of_one_read_never_drop_each_other() {
        let mut kept = Kept::default();
        kept.read(Some(&field(8)), &["card 4111 123", "123"]);
        kept.read(Some(&field(8)), &["card 4111 123", "123"]);
        assert_eq!(kept.texts(), vec!["card 4111 123", "123"]);
    }

    #[test]
    fn taking_in_reads_each_elements_texts_together() {
        let mut page = Kept::default();
        page.read(Some(&field(8)), &["the"]);
        let mut browser = Kept::default();
        browser.read(Some(&field(8)), &["themes", "the"]);
        page.take_in(&browser);
        assert_eq!(page.texts(), vec!["themes", "the"]);
    }
}
