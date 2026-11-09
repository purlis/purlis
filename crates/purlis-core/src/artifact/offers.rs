// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The actions an artifact's page offers** (ADR 0120 §8, #5972).
//!
//! A page never performs an action. It may *offer* some: each a `Does` id of the window's action
//! catalogue (`app/src/actions.ts`, the part of a row's id before its colon, such as
//! `todo.done`) and a subject (the part after it, such as a todo's slug). The page posts the
//! whole set it offers now, `{ id, offers: [{ does, subject }] }`, on its one port; each set
//! replaces the last. purlis draws them in its own bar and the person presses one there.
//!
//! This module only reads what a page said ([`asked`]), in closed types: which ids a page may
//! offer, and what each does, is the window's (`app/src/pageOffers.ts`), which resolves an
//! offer against the catalogue the window itself built, so a page names a row and never words
//! one. This module renders nothing and does nothing.

use serde_json::Value;

/// The most offers a page's set may hold.
pub const MOST: usize = 8;

/// The most a set may be as the page sends it, percent-encoded.
pub const MOST_ASKED: usize = 4 << 10;

/// The longest `does` id.
const DOES_MOST: usize = 64;

/// The longest subject, in characters.
const SUBJECT_MOST: usize = 256;

/// One action a page offers: a `Does` id and what it is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    pub does: String,
    pub subject: String,
}

/// **The set of offers a page's ask carries**, the query after `?` as the port script sends it:
/// `{"offers": [{"does": "todo.done", "subject": "fix-login"}]}`. Nothing when any part of it
/// is not that shape: a field more, an id that is not one, a subject with a line break, more
/// than [`MOST`], or the same offer twice. An empty set withdraws every offer.
pub fn asked(query: &str) -> Option<Vec<Offered>> {
    if query.len() > MOST_ASKED {
        return None;
    }
    let text = percent_encoding::percent_decode_str(query)
        .decode_utf8()
        .ok()?;
    let Value::Object(mut said) = serde_json::from_str::<Value>(&text).ok()? else {
        return None;
    };
    let Some(Value::Array(each)) = said.remove("offers") else {
        return None;
    };
    if !said.is_empty() || each.len() > MOST {
        return None;
    }
    let mut offers: Vec<Offered> = Vec::with_capacity(each.len());
    for one in each {
        let offered = offer(one)?;
        if offers.contains(&offered) {
            return None;
        }
        offers.push(offered);
    }
    Some(offers)
}

/// One offer, `{ does, subject }` and nothing else, in closed types.
fn offer(one: Value) -> Option<Offered> {
    let Value::Object(mut fields) = one else {
        return None;
    };
    let Some(Value::String(does)) = fields.remove("does") else {
        return None;
    };
    let Some(Value::String(subject)) = fields.remove("subject") else {
        return None;
    };
    (fields.is_empty() && is_does(&does) && is_subject(&subject))
        .then_some(Offered { does, subject })
}

/// Whether `does` is spelled as a catalogue row's verb is: dotted lowercase words, such as
/// `todo.done`, with no colon (the subject is the part after it).
fn is_does(does: &str) -> bool {
    does.len() <= DOES_MOST
        && does.contains('.')
        && does.split('.').all(|word| {
            word.starts_with(|c: char| c.is_ascii_lowercase())
                && word
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        })
}

/// Whether `subject` is one line of text purlis can name a row by.
fn is_subject(subject: &str) -> bool {
    !subject.is_empty()
        && subject.chars().count() <= SUBJECT_MOST
        && !subject.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sent(text: &str) -> String {
        percent_encoding::utf8_percent_encode(text, percent_encoding::NON_ALPHANUMERIC).to_string()
    }

    #[test]
    fn a_set_of_offers_is_read_as_does_ids_and_subjects() {
        let offers = asked(&sent(
            r#"{"offers":[{"does":"todo.done","subject":"fix-login"},{"does":"todo.delete","subject":"old one"}]}"#,
        ))
        .unwrap();
        assert_eq!(
            offers,
            vec![
                Offered {
                    does: "todo.done".into(),
                    subject: "fix-login".into()
                },
                Offered {
                    does: "todo.delete".into(),
                    subject: "old one".into()
                },
            ]
        );
        assert_eq!(asked(&sent(r#"{"offers":[]}"#)), Some(Vec::new()));
    }

    #[test]
    fn a_set_that_is_not_offers_in_closed_types_is_refused_whole() {
        let many = (0..=MOST)
            .map(|n| format!(r#"{{"does":"todo.done","subject":"t{n}"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        for not in [
            "not json".to_owned(),
            r#"{"offers":{"does":"todo.done","subject":"a"}}"#.to_owned(),
            r#"{"offers":[],"perform":true}"#.to_owned(),
            r#"{"offers":[{"does":"todo.done","subject":"a","title":"Delete everything"}]}"#
                .to_owned(),
            r#"{"offers":[{"does":"todo.done"}]}"#.to_owned(),
            r#"{"offers":[{"does":"todo.done","subject":7}]}"#.to_owned(),
            r#"{"offers":[{"does":"todo.done","subject":""}]}"#.to_owned(),
            r#"{"offers":[{"does":"todo.done","subject":"a\nb"}]}"#.to_owned(),
            r#"{"offers":[{"does":"todo","subject":"a"}]}"#.to_owned(),
            r#"{"offers":[{"does":"todo.done:a","subject":"a"}]}"#.to_owned(),
            r#"{"offers":[{"does":"Todo.Done","subject":"a"}]}"#.to_owned(),
            r#"{"offers":[{"does":"todo..done","subject":"a"}]}"#.to_owned(),
            r#"{"offers":[{"does":"todo.done","subject":"a"},{"does":"todo.done","subject":"a"}]}"#
                .to_owned(),
            format!(r#"{{"offers":[{many}]}}"#),
            format!(
                r#"{{"offers":[{{"does":"todo.done","subject":"{}"}}]}}"#,
                "x".repeat(SUBJECT_MOST + 1)
            ),
        ] {
            assert_eq!(asked(&sent(&not)), None, "{not}");
        }
        assert_eq!(asked(&"%7B".repeat(MOST_ASKED)), None);
    }
}
