// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A text as the index takes it: scanned for secrets, then split into purlis's words.
//!
//! **Scanned first** (ADR 0079 §6). Each span the shape scanner flags
//! ([`crate::secretshape::leak_spans`], over the whole text and over each line, as a save asks
//! it, both read at once by [`crate::secretshape::leak_spans_whole_and_by_line`]) is replaced by
//! its kind in brackets, so neither the value nor a head of it is indexed. A title is scanned the
//! same way before it is kept.
//!
//! **Then split as the file scan matches.** The scan finds a term at the start of a word, a word
//! being a run of `\w` characters of the lowercased text ([`crate::memstore::rank`]). The index is
//! handed exactly those words, lowercased and joined by spaces, and FTS5's `ascii` tokenizer with
//! `_` as a token character splits them back unchanged: it breaks only at ASCII characters that
//! are not letters, digits or `_`, and a word holds none. So FTS5's prefix query `"term"*` matches
//! a memory exactly when the scan's `\bterm\w*` does. ADR 0079 §9 named `unicode61` with
//! diacritics removed; that would find `cafe` in `café`, which the scan does not, and the index
//! must find what the scan finds (#5498).

/// Bumped whenever [`indexed`] or [`scanned`] changes what reaches the index: an index built
/// before is rebuilt.
pub(crate) const VERSION: &str = "words-1";

/// `text` with every span the shape scanner flags replaced by its kind, as `[kind]`.
pub(crate) fn scanned(text: &str) -> String {
    let spans = crate::secretshape::leak_spans_whole_and_by_line(text);
    if spans.is_empty() {
        return text.to_string();
    }
    // Widened to whole characters and joined where they meet, so no piece of a value is left
    // between two spans.
    let mut widened: Vec<(std::ops::Range<usize>, &'static str)> = spans
        .into_iter()
        .filter(|(span, _)| span.start < span.end && span.start < text.len())
        .map(|(span, kind)| {
            let start = (0..=span.start)
                .rev()
                .find(|i| text.is_char_boundary(*i))
                .unwrap_or(0);
            let end = (span.end.min(text.len())..=text.len())
                .find(|i| text.is_char_boundary(*i))
                .unwrap_or(text.len());
            (start..end, kind)
        })
        .collect();
    widened.sort_by_key(|(span, _)| (span.start, std::cmp::Reverse(span.end)));
    let mut joined: Vec<(std::ops::Range<usize>, &'static str)> = Vec::new();
    for (span, kind) in widened {
        match joined.last_mut() {
            Some((last, _)) if span.start <= last.end => last.end = last.end.max(span.end),
            _ => joined.push((span, kind)),
        }
    }
    let mut out = String::with_capacity(text.len());
    let mut from = 0;
    for (span, kind) in joined {
        out.push_str(&text[from..span.start]);
        out.push_str(&format!(" [{kind}] "));
        from = span.end;
    }
    out.push_str(&text[from..]);
    out
}

/// `text`'s words as the file scan reads them, lowercased and joined by spaces: what FTS5 is
/// handed. The caller scans `text` first.
pub(crate) fn indexed(text: &str) -> String {
    crate::memstore::tokens(text).join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_is_replaced_by_its_kind_and_the_rest_is_kept() {
        // Put together at run time, so no token is written in this file.
        let token = ["ghp", "_", "Zq7Xw3Vb9Nc5Mk1Lp8Rt2Ys6Ud4Fh0Gj3Ka5"].concat();
        let out = scanned(&format!("before {token} after\nnext line"));

        assert!(!out.contains("Zq7Xw3"), "{out}");
        assert!(
            out.starts_with("before ") && out.ends_with(" after\nnext line"),
            "{out}"
        );
        assert!(
            out.contains('['),
            "the kind stands where the value was: {out}"
        );
    }

    #[test]
    fn a_text_with_nothing_flagged_is_kept_as_it_is() {
        assert_eq!(
            scanned("plain words, nothing more"),
            "plain words, nothing more"
        );
    }

    #[test]
    fn words_are_the_scans_words_lowercased() {
        assert_eq!(
            indexed("Deploy snake_case-Words, Café!"),
            "deploy snake_case words café"
        );
    }
}
