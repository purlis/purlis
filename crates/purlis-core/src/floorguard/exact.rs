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
    /// Each word of the call as the source spells it, `""` where nothing measured it.
    call_spelled: Vec<String>,
    /// Whether the segment is handed words: it stands after a `|`, or has a redirection that
    /// feeds its stdin (a file, a heredoc, a here-string).
    pub fed: bool,
    /// The call again without a digit word that stands apart from the redirection after it
    /// (`gh 5 >f pr merge 5`): the shell makes it an argument, and a reader that took it for a
    /// descriptor would see another call. Read too, so that no earlier reading's refusal is lost.
    pub also: Option<Vec<String>>,
    /// Why this segment could not be read exactly, where it speaks of a forge CLI.
    pub refusal: Option<String>,
    /// The same segment read as zsh reads `>!`, `>>!`, `&>!` and `>&!`: one operator, whose
    /// target is the word after the `!`. Bash and dash read the `!` as the target, so both are
    /// read, and a refusal of either stands (#7423, round 7).
    pub clobber: Option<Box<Segment>>,
    /// The words of each forge call that stands behind a program no table knows (`ssh h gh pr
    /// merge 5`), from the CLI on. The segment is held where the floor holds one of them read as
    /// a direct call: what a wrapper may run is what the call would be without it (round 8).
    pub behind: Vec<Segment>,
}

/// What a refusal of a segment the reader could not place says to do.
pub(super) const WAY_OUT: &str = "purlis could not tell which word runs here; put gh first in the command, \
     or run it without the wrapper.";

impl Segment {
    /// The call, where the segment's program is a forge CLI.
    pub fn call(&self) -> Option<&[String]> {
        self.call.as_deref()
    }

    /// The words of [`Segment::call`] as the source spells them.
    pub fn call_spelled(&self) -> &[String] {
        &self.call_spelled
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
    build(words, measured, reading.own_piped)
}

/// [`read`] of the words as they stand, and again as zsh's clobber operators read them. `piped`
/// is whether a `|` hands the segment its stdin.
fn build(words: Vec<Word>, measured: bool, piped: bool) -> Segment {
    let mut seg = read(&words, measured, false, piped);
    if words.windows(2).any(clobbers) {
        seg.clobber = Some(Box::new(read(&words, measured, true, piped)));
    }
    seg
}

/// Whether two neighbouring words are an operator and the `!` of zsh's clobber form of it.
fn clobbers(pair: &[Word]) -> bool {
    pair[0].bare
        && matches!(
            pair[0].text.as_str(),
            ">" | ">>" | "&>" | ">&" | "&>>" | ">>&"
        )
        && pair[1].bare
        && pair[1].glued
        && pair[1].text == "!"
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
    build(words, false, false)
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
///
/// With `clobber`, a `!` glued to a redirection operator is part of it (zsh).
fn read(words: &[Word], measured: bool, clobber: bool, piped: bool) -> Segment {
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
            let mut width = 2;
            if clobber && i + 1 < n && clobbers(&words[i..=i + 1]) {
                // `>! file`: the operator, its `!`, and the target.
                span.push(i + 1);
                width = 3;
                if i + 2 < n {
                    span.push(i + 2);
                }
            } else if i + 1 < n {
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
            i += width;
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
    let fed = piped || stdin.iter().any(|feeds| *feeds);
    let mut call_spelled: Vec<String> = Vec::new();
    let call = forge_at.map(|at| {
        // The `api` of an api call, read past its flags and `-R o/r`: a body read from stdin
        // follows it. No other call has one (an alias's `-` is stdin the call cannot read).
        let positional = super::merge::forge_words(&texts[at + 1..]);
        let cut = match positional.first() {
            Some((k, "api")) => Some(kept[at + 1 + *k]),
            _ => None,
        };
        let stands: Vec<usize> = (kept[at]..n)
            .filter(|j| !redirected[*j] || (stdin[*j] && cut.is_some_and(|c| *j > c)))
            .collect();
        let mut call: Vec<String> = stands.iter().map(|j| words[*j].text.clone()).collect();
        call_spelled = stands.iter().map(|j| words[*j].spelled.clone()).collect();
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
    // A forge call behind a program no table knows says nothing of what it runs (`ssh h gh pr
    // merge 5`, `fd -x gh pr merge 5`): read as the direct call is, whatever stands before the
    // CLI. A word the shell fills in, followed by a held command, may be the CLI: held.
    let spelled_argv: Vec<String> = if placed {
        spelled[at..].to_vec()
    } else {
        vec![String::new(); inv.argv.len()]
    };
    let (shell_filled, behind) = behind_an_unknown_program(&inv, &spelled_argv, fed);
    let refusal =
        refusal.or_else(|| shell_filled.then(|| format!("{} {WAY_OUT}", super::merge::UNREADABLE)));
    Segment {
        words: texts,
        spelled,
        call,
        call_spelled,
        fed,
        also,
        refusal,
        clobber: None,
        behind,
    }
}

/// Programs that only print or search the words they are handed, so a forge CLI named among
/// them runs nothing.
const TEXT_PROGRAMS: [&str; 15] = [
    "echo", "printf", "cat", "grep", "egrep", "fgrep", "rg", "ag", "sed", "awk", "head", "tail",
    "wc", "type", "whereis",
];

/// Programs that look a name up or install a package of that name, and never run it (`brew
/// install gh $PKGS`, `which gh`, `man gh`).
const NAMES_ONLY: [&str; 19] = [
    "brew", "apt", "apt-get", "apk", "dnf", "yum", "pacman", "port", "choco", "scoop", "winget",
    "dpkg", "rpm", "which", "man", "tldr", "apropos", "locate", "mdfind",
];

/// **The general wrapper rule** (#7423, rounds 7 and 8). The program placed is not a forge CLI,
/// and a later word is one: whatever the program is (`taskset 1`, `chronic`, `ssh h`, `fd -x`, a
/// wrapper no table lists), it may run that call. Only the plain printers of [`TEXT_PROGRAMS`],
/// and the words of a `for … in` list, are left alone.
///
/// Answers whether a shell-filled word stands before a held command, and the calls to read, each
/// as a segment of its own. A
/// forge word that is a path (`/opt/homebrew/bin/gh`, `crates/gh`), or is followed by a word the
/// shell fills in, is a call only where a held verb stands after it, as a flag the reader cannot
/// place is. A word the shell fills in (`$(which gh)`, `$GH`, `{gh,pr}`, `g[h]`) followed by a
/// held noun and verb may be the CLI. `purlis change land` is a held call as `gh pr merge` is.
fn behind_an_unknown_program(
    inv: &shellwrap::Invocation,
    spelled: &[String],
    fed: bool,
) -> (bool, Vec<Segment>) {
    let argv = &inv.argv;
    let mut none = (false, Vec::new());
    if argv.len() < 2 || names_a_forge(&inv.prog) {
        return none;
    }
    let base = shellwrap::base_lower(&inv.prog);
    if TEXT_PROGRAMS.contains(&base.as_str()) || NAMES_ONLY.contains(&base.as_str()) {
        return none;
    }
    let in_a_list = argv[1] == "in"
        && inv
            .taken
            .iter()
            .any(|t| matches!(t.as_str(), "for" | "select" | "case"));
    if in_a_list {
        return none;
    }
    // One pass from the end: whether a held verb stands at or after each word.
    let mut later = vec![false; argv.len() + 1];
    for k in (0..argv.len()).rev() {
        later[k] = later[k + 1] || super::merge::speaks_of_a_held_verb(&argv[k]);
    }
    for k in 1..argv.len() {
        let word = argv[k].strip_prefix('=').unwrap_or(&argv[k]);
        let base = shellwrap::base_lower(word);
        if names_a_forge(word) || matches!(base.as_str(), "purlis" | "charter") {
            let a_path = word.contains('/') || word.starts_with('~');
            let after_spelled = spelled.get(k + 1..).unwrap_or(&[]);
            if names_a_forge(word) {
                // A path that names a forge CLI is a call only where a literal noun of a held
                // command follows it (`/opt/homebrew/bin/gh pr …`); `cp /usr/bin/gh ~` names a
                // file. A bare `gh` is a call, read by the rule for the words up to the verb
                // ([`super::placed`], round 10).
                // A held verb later on the line (`/usr/bin/gh $A merge 5`) makes it one too.
                if a_path
                    && !later[k + 1]
                    && !super::placed::names_a_held_noun(
                        &shellwrap::base_lower(word),
                        &argv[k + 1..],
                        after_spelled,
                    )
                {
                    continue;
                }
            } else {
                // `purlis change land` and `charter change land`: read as before (round 8).
                let opaque_next = argv.get(k + 1).is_some_and(|w| super::merge::opaque(w));
                if (a_path || opaque_next) && !later[k + 1] {
                    continue;
                }
            }
            let mut call: Vec<Word> = (k..argv.len())
                .map(|j| Word {
                    text: argv[j].clone(),
                    spelled: spelled.get(j).cloned().unwrap_or_default(),
                    bare: true,
                    glued: false,
                })
                .collect();
            call[0].text = word.to_owned();
            let measured = call.iter().any(|w| !w.spelled.is_empty());
            none.1.push(build(call, measured, fed));
        } else if super::merge::opaque(word) && fills_in_a_forge_call(word, &argv[k + 1..]) {
            none.0 = true;
        }
    }
    none
}

/// Whether a word the shell fills in stands before a held noun and verb, or, if it is a brace
/// expansion (`{gh,pr}`), before a held verb its words may complete.
fn fills_in_a_forge_call(word: &str, after: &[String]) -> bool {
    let rows: Vec<(&str, &str, &str)> = if super::only_merges() {
        super::MERGE_ROWS.to_vec()
    } else {
        super::PUBLISH_FORGE
            .iter()
            .chain(super::PUBLISH_SINCE.iter())
            .copied()
            .collect()
    };
    let held = |cli: &str, noun: &str, verb: &str| {
        let verb = super::merge::canonical_verb(cli, noun, verb);
        rows.iter()
            .any(|r| r.0 == cli && r.1 == noun && r.2 == verb)
    };
    let pair = match after {
        [noun, verb, ..] => rows.iter().any(|r| r.1 == noun && held(r.0, noun, verb)),
        _ => false,
    };
    let completes = super::merge::brace_expansion(word)
        && after
            .first()
            .is_some_and(|verb| rows.iter().any(|r| held(r.0, r.1, verb)));
    pair || completes
}
