// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The texts a browser keeps out of every answer (#2014, #2311), each with the element it was
//! read from and how. A field the person types into is read from the field itself after each
//! key, and a read drops that field's earlier values that it contains while they are shorter
//! than [`KEPT_WHOLE_FROM`]: a field keeps its latest value, not every short value it held on
//! the way. A read of a page's tree drops nothing, and a text read from no element (a run of
//! the person's keys, or what they pasted) is kept as it is. A code typed across several boxes
//! (#2144) is kept whole as the boxes' group holds it, and a read of the group is read like a
//! field's: the group keeps its latest code, not every short part of it on the way.

use super::tree::NodeKey;

/// The length, in characters, from which an earlier value of a field stays kept out once a
/// later value contains it (the person's ruling of 2026-10-17): a page that shows a long value
/// cut short still shows nothing of it. A shorter earlier value goes.
pub(crate) const KEPT_WHOLE_FROM: usize = 8;

/// How a text kept out was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    /// From the field itself, or from the person's keys.
    Field,
    /// From the page's tree.
    Tree,
}

/// What a text kept out was read from.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Of {
    /// One element.
    Element(NodeKey),
    /// The group of boxes that begins with this element, read together.
    Group(NodeKey),
}

/// One text kept out.
#[derive(Debug, Clone)]
struct Text {
    /// What it was read from, if anything.
    from: Option<Of>,
    source: Source,
    text: String,
}

/// Texts kept out of every answer.
#[derive(Debug, Clone, Default)]
pub(crate) struct Kept {
    texts: Vec<Text>,
}

impl Kept {
    /// Texts read from no element, a run of the person's keys or what they pasted: never
    /// dropped.
    pub(crate) fn run<S: AsRef<str>>(&mut self, texts: &[S]) {
        self.add(None, Source::Field, texts);
    }

    /// One read of what `from` holds, from the field itself: every earlier text of `from`
    /// shorter than [`KEPT_WHOLE_FROM`] that one of `texts` contains goes, and `texts` are kept.
    pub(crate) fn field<S: AsRef<str>>(&mut self, from: &NodeKey, texts: &[S]) {
        self.read(&Of::Element(from.clone()), texts);
    }

    /// One read of what the group of boxes that begins with `first` holds, joined: read like a
    /// field ([`Kept::field`]).
    pub(crate) fn group<S: AsRef<str>>(&mut self, first: &NodeKey, texts: &[S]) {
        self.read(&Of::Group(first.clone()), texts);
    }

    fn read<S: AsRef<str>>(&mut self, from: &Of, texts: &[S]) {
        let lower: Vec<String> = texts
            .iter()
            .map(|text| text.as_ref().to_lowercase())
            .filter(|text| !text.is_empty())
            .collect();
        if lower.is_empty() {
            return;
        }
        self.texts.retain(|kept| {
            kept.from.as_ref() != Some(from) || kept.text.chars().count() >= KEPT_WHOLE_FROM || {
                let earlier = kept.text.to_lowercase();
                !lower.iter().any(|text| text.contains(&earlier))
            }
        });
        self.add(Some(from), Source::Field, texts);
    }

    /// One read of what `from` holds, from its page's tree: `texts` are kept, and nothing goes.
    pub(crate) fn tree<S: AsRef<str>>(&mut self, from: &NodeKey, texts: &[S]) {
        self.add(Some(&Of::Element(from.clone())), Source::Tree, texts);
    }

    /// Takes in every text `other` keeps: what each field held, as one read of that field.
    pub(crate) fn take_in(&mut self, other: &Kept) {
        let mut fields: Vec<&Of> = Vec::new();
        for kept in &other.texts {
            match (&kept.from, kept.source) {
                (Some(from), Source::Field) => {
                    if !fields.contains(&from) {
                        fields.push(from);
                    }
                }
                (Some(from), Source::Tree) => self.add(Some(from), Source::Tree, &[&kept.text]),
                (None, _) => self.run(&[&kept.text]),
            }
        }
        for from in fields {
            let held: Vec<&str> = other
                .texts
                .iter()
                .filter(|kept| kept.source == Source::Field && kept.from.as_ref() == Some(*from))
                .map(|kept| kept.text.as_str())
                .collect();
            self.read(from, &held);
        }
    }

    /// Every text kept.
    pub(crate) fn texts(&self) -> Vec<&str> {
        self.texts.iter().map(|kept| kept.text.as_str()).collect()
    }

    /// Whether nothing is kept.
    pub(crate) fn is_empty(&self) -> bool {
        self.texts.is_empty()
    }

    fn add<S: AsRef<str>>(&mut self, from: Option<&Of>, source: Source, texts: &[S]) {
        for text in texts.iter().map(AsRef::as_ref) {
            if text.is_empty() {
                continue;
            }
            let lower = text.to_lowercase();
            if !self
                .texts
                .iter()
                .any(|kept| kept.from.as_ref() == from && kept.text.to_lowercase() == lower)
            {
                self.texts.push(Text {
                    from: from.cloned(),
                    source,
                    text: text.to_owned(),
                });
            }
        }
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
    fn a_fields_later_value_drops_the_short_earlier_values_it_contains() {
        let mut kept = Kept::default();
        for value in ["the", "them", "theme"] {
            kept.field(&field(8), &[value]);
        }
        assert_eq!(kept.texts(), vec!["theme"]);
    }

    #[test]
    fn an_earlier_value_as_long_as_kept_whole_from_stays() {
        let mut kept = Kept::default();
        kept.field(&field(8), &["plumtree"]);
        kept.field(&field(8), &["plumtreew"]);
        assert_eq!(kept.texts(), vec!["plumtree", "plumtreew"]);
    }

    #[test]
    fn a_fields_shorter_later_value_is_kept_with_the_longer_one() {
        let mut kept = Kept::default();
        kept.field(&field(8), &["secret"]);
        kept.field(&field(8), &["sec"]);
        assert_eq!(kept.texts(), vec!["secret", "sec"]);
    }

    #[test]
    fn a_value_never_drops_another_elements_text_or_a_run() {
        let mut kept = Kept::default();
        kept.field(&field(12), &["the"]);
        kept.run(&["them"]);
        kept.field(&field(8), &["theme"]);
        assert_eq!(kept.texts(), vec!["the", "them", "theme"]);
    }

    #[test]
    fn a_tree_read_drops_nothing() {
        let mut kept = Kept::default();
        kept.field(&field(8), &["sec"]);
        kept.tree(&field(8), &["secret"]);
        kept.tree(&field(8), &["secrets"]);
        assert_eq!(kept.texts(), vec!["sec", "secret", "secrets"]);
    }

    #[test]
    fn a_field_read_drops_what_the_tree_read_on_the_way() {
        let mut kept = Kept::default();
        kept.tree(&field(8), &["the"]);
        kept.field(&field(8), &["them"]);
        assert_eq!(kept.texts(), vec!["them"]);
    }

    #[test]
    fn texts_of_one_read_never_drop_each_other() {
        let mut kept = Kept::default();
        kept.field(&field(8), &["card 4111 123", "123"]);
        kept.field(&field(8), &["card 4111 123", "123"]);
        assert_eq!(kept.texts(), vec!["card 4111 123", "123"]);
    }

    #[test]
    fn taking_in_reads_each_fields_values_together() {
        let mut page = Kept::default();
        page.tree(&field(8), &["the"]);
        let mut browser = Kept::default();
        browser.field(&field(8), &["themes"]);
        browser.field(&field(8), &["the"]);
        page.take_in(&browser);
        assert_eq!(page.texts(), vec!["themes", "the"]);
    }

    #[test]
    fn a_groups_later_code_drops_its_short_earlier_codes_and_nothing_of_its_boxes() {
        let mut kept = Kept::default();
        kept.field(&field(11), &["739"]);
        kept.group(&field(11), &["7392"]);
        kept.group(&field(11), &["739214"]);
        assert_eq!(kept.texts(), vec!["739", "739214"]);
    }
}
