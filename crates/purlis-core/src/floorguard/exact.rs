// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One exact reading of a segment's words** (#7423, round 6).
//!
//! Rounds 4 and 5 read each line several ways, because the lexer split `2>` into `2` and `>` and
//! lost whether a word was quoted: a digit might be a descriptor or a value, a `<` an operator or
//! a word. A line-wide guess cannot say "this digit is a value and that one a descriptor", and
//! the guesses multiplied (`env -u - 3<f 5 >f -` twelve times took seconds).
//!
//! The lexer keeps the facts now ([`crate::shellseg::Reading::own_bare`],
//! [`crate::shellseg::Reading::own_glued`]), so a redirection is exactly what the shell reads: an
//! operator written unquoted, preceded by a glued all-digit word or `{name}` (its descriptor),
//! and followed by its target word. Each is taken out wherever it stands in the segment, once.
//! What is left is the command, and the wrapper grammar of [`shellwrap`] places its program.
//!
//! **What cannot be read exactly is held**, for a segment that speaks of a forge CLI: a wrapper
//! flag no table lists (it may take the next word as its value), a wrapper no table knows, and,
//! for words that carry no facts, a digit that may be a descriptor. The refusal says what to do.

use crate::shellseg;
use crate::shellwrap;

/// One word of a segment, with what the lexer knew of it.
pub(super) struct Word {
    pub text: String,
    /// The source's spelling, or `""` where the line did not lex.
    pub spelled: String,
    /// Written with no quote or escape in it, so an operator is an operator.
    pub bare: bool,
    /// Begins where the word before it ends.
    pub glued: bool,
}

/// A segment's words with its redirections taken out, and where its program is.
pub(super) struct Segment {
    /// The words, without redirections.
    pub words: Vec<String>,
    /// Each word as the source spells it, `""` where nothing measured it.
    pub spelled: Vec<String>,
    /// The call, where a forge CLI runs: its words from the program on, `=gh` written `gh`, and
    /// the redirections that feed stdin and stand after `api` kept in place, since a body read
    /// from a heredoc or a here-string is part of the call (`--input - <<'EOF'`).
    call: Option<Vec<String>>,
    /// The call again without a digit word that stands apart from the redirection after it
    /// (`gh 5 >f pr merge 5`): the shell makes it an argument, and a reader that took it for a
    /// descriptor would see another call. Read too, so that no earlier reading's refusal is lost.
    pub also: Option<Vec<String>>,
    /// Why this segment could not be read exactly, where it speaks of a forge CLI.
    pub refusal: Option<String>,
}

/// What a refusal of a segment the reader could not place says to do.
const WAY_OUT: &str = "purlis could not tell which word runs here; put gh first in the command, \
     or run it without the wrapper.";

impl Segment {
    /// The call, where the segment's program is a forge CLI.
    pub fn call(&self) -> Option<&[String]> {
        self.call.as_deref()
    }
}

/// [`read`] of one segment of a measured line.
pub(super) fn of_reading(reading: &shellseg::Reading) -> Segment {
    let measured = reading.own_source.iter().any(|s| !s.is_empty());
    let words = (0..reading.own.len())
        .map(|k| Word {
            text: reading.own[k].clone(),
            spelled: reading.own_source.get(k).cloned().unwrap_or_default(),
            bare: reading.own_bare.get(k).copied().unwrap_or(true),
            glued: reading.own_glued.get(k).copied().unwrap_or(false),
        })
        .collect();
    read(words, measured)
}

/// [`read`] of words nothing measured (what a `find` runs, a hedged line): every word is taken
/// as written bare, and none is glued.
pub(super) fn of_words(toks: &[String]) -> Segment {
    let words = toks
        .iter()
        .map(|t| Word {
            text: t.clone(),
            spelled: String::new(),
            bare: true,
            glued: false,
        })
        .collect();
    read(words, false)
}

/// Whether a word is an all-digit file descriptor or a named one, `{fd}`.
fn is_descriptor(word: &str) -> bool {
    (!word.is_empty() && word.bytes().all(|b| b.is_ascii_digit())) || named_descriptor(word)
}

/// Whether a word is a shell's named descriptor, `{fd}` (as in `{fd}>file`).
fn named_descriptor(word: &str) -> bool {
    word.strip_prefix('{')
        .and_then(|w| w.strip_suffix('}'))
        .is_some_and(|name| {
            name.chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

/// Whether a word names a forge CLI, `=gh` (which zsh expands to the path of `gh`) included.
fn names_a_forge(word: &str) -> bool {
    let word = word.strip_prefix('=').unwrap_or(word);
    matches!(shellwrap::base_lower(word).as_str(), "gh" | "glab")
}

/// Operators whose redirection feeds the command's stdin from a file, a heredoc or a string.
fn feeds_stdin(op: &str) -> bool {
    matches!(op, "<" | "<<" | "<<<" | "<>")
}

/// The segment's redirections taken out as the shell reads them, and its program placed.
fn read(words: Vec<Word>, measured: bool) -> Segment {
    // A definition's header is no part of the command.
    let texts: Vec<String> = words.iter().map(|w| w.text.clone()).collect();
    let from = shellwrap::past_function_headers(&texts).unwrap_or(0);
    let words = &words[from..];
    // One walk: an unquoted operator takes the word after it, and a glued descriptor before it.
    let n = words.len();
    let mut redirected = vec![false; n];
    let mut stdin = vec![false; n];
    let mut i = 0;
    while i < n {
        let word = &words[i];
        if word.bare && shellwrap::is_redirection(&word.text) {
            let feeds = feeds_stdin(&word.text);
            let mut span = vec![i];
            if i + 1 < n {
                span.push(i + 1);
            }
            if word.glued
                && i > 0
                && !redirected[i - 1]
                && words[i - 1].bare
                && is_descriptor(&words[i - 1].text)
            {
                span.push(i - 1);
            }
            for k in span {
                redirected[k] = true;
                stdin[k] = feeds;
            }
            i += 2;
            continue;
        }
        i += 1;
    }
    let kept: Vec<usize> = (0..n).filter(|k| !redirected[*k]).collect();
    let texts: Vec<String> = kept.iter().map(|k| words[*k].text.clone()).collect();
    let spelled: Vec<String> = kept.iter().map(|k| words[*k].spelled.clone()).collect();
    let inv = shellwrap::split_env_chdir_exact(&texts);
    let at = texts.len().saturating_sub(inv.argv.len());
    let placed = texts[at..] == inv.argv[..];
    let forge_at = (placed && !inv.argv.is_empty() && names_a_forge(&inv.prog)).then_some(at);
    let call = forge_at.map(|at| {
        // The `api` of an api call, read past its flags and `-R o/r`: a body read from stdin
        // follows it. No other call has one (an alias's `-` is stdin the call cannot read).
        let positional = super::merge::forge_words(&texts[at + 1..]);
        let cut = match positional.first() {
            Some((k, "api")) => Some(kept[at + 1 + *k]),
            _ => None,
        };
        let mut call: Vec<String> = (kept[at]..n)
            .filter(|j| !redirected[*j] || (stdin[*j] && cut.is_some_and(|c| *j > c)))
            .map(|j| words[j].text.clone())
            .collect();
        if call[0].starts_with('=') {
            call[0].remove(0);
        }
        call
    });
    let also = forge_at.and_then(|at| {
        let apart = |j: usize| {
            let w = &words[j];
            w.bare
                && !w.glued
                && !w.text.is_empty()
                && w.text.bytes().all(|b| b.is_ascii_digit())
                && words.get(j + 1).is_some_and(|o| {
                    o.bare && redirected[j + 1] && shellwrap::is_redirection(&o.text)
                })
        };
        let from = kept[at];
        let dropped: Vec<usize> = (from..n).filter(|j| !redirected[*j] && apart(*j)).collect();
        (!dropped.is_empty()).then(|| {
            let mut call: Vec<String> = (from..n)
                .filter(|j| !redirected[*j] && !dropped.contains(j))
                .map(|j| words[j].text.clone())
                .collect();
            if call[0].starts_with('=') {
                call[0].remove(0);
            }
            call
        })
    });
    // A segment that speaks of a forge CLI must be read exactly, or it is held.
    let speaks_of_one = texts.iter().any(|w| names_a_forge(w))
        || texts
            .windows(2)
            .any(|p| p[0].contains(['$', '`']) && p[1] == "api");
    // Words nothing measured: a digit before an operator may be a descriptor or a word, and
    // only where it stands in front of the forge CLI does it change what runs.
    let before = words
        .iter()
        .position(|w| names_a_forge(&w.text))
        .unwrap_or(n);
    let ambiguous = !measured
        && words[..before]
            .windows(2)
            .any(|p| is_descriptor(&p[0].text) && shellwrap::is_redirection(&p[1].text));
    // A program that is a bare number, or an operator written as a word (`'<'`), was read where a
    // value or a descriptor stood: nothing is
    // named that, so the words in front of the forge CLI were not placed.
    let odd_program = !inv.prog.is_empty()
        && (inv.prog.bytes().all(|b| b.is_ascii_digit()) || shellwrap::is_redirection(&inv.prog));
    let refusal = (speaks_of_one && (inv.unplaced || ambiguous || odd_program))
        .then(|| format!("{} {WAY_OUT}", super::merge::UNREADABLE));
    Segment {
        words: texts,
        spelled,
        call,
        also,
        refusal,
    }
}
