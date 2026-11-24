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
//! **What is not read** (round 13): a command inside ONE quoted word that a program other than a
//! shell runs (an editor or pager config value, `awk`'s `system()`, `GIT_EDITOR`, a `find`
//! `-exec` that is itself a string of another program) is never split into words, so it is not
//! seen. Programs that cannot run a word are the minimal [`NAMES_ONLY`] list; behind every other
//! program a forge word is a call. Under `guards.merge = enforce` the forge route is the boundary
//! for what the text guard does not read.
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
        also,
        refusal,
        clobber: None,
        behind,
    }
}

/// **Programs that provably never execute a word operand** (#7423, rounds 11 and 13): a forge
/// CLI named among their words is a name, a file or a search word (`cp /usr/bin/gh ~`, `grep gh
/// src/*`, `which gh`). The list is MINIMAL on purpose: every program not on it may run a word
/// (a package manager's hooks, `sed`'s `e`, `awk`'s `system()`, `find -exec`, `tee`, a pager),
/// and each round that kept a longer table found a hole in it. A name behind any other program
/// is a call, held unless it passes the allowlist. `echo` and `printf` print their words.
const NAMES_ONLY: &[&str] = &[
    "cp", "mv", "ln", "rm", "ls", "stat", "file", "du", "cat", "head", "tail", "wc", "diff", "cmp",
    "shasum", "md5", "md5sum", "sha256sum", "grep", "rg", "mkdir", "touch", "chmod", "chown",
    "which", "type", "man", "echo", "printf",
];

/// `git`'s subcommands that run none of their operands: `git grep gh`, `git log -- crates/*`.
/// Not here: `commit`, `checkout`, `switch`, `restore`, `reset`, `stash`, `pull`, `push`.
const GIT_SUBCOMMANDS: &[&str] = &[
    "add", "status", "log", "diff", "show", "blame", "shortlog", "grep", "mv", "rm", "branch",
    "tag", "fetch", "remote",
];

/// Whether a `git -c key=value` key names a program git runs.
fn git_config_runs_a_program(pair: &str) -> bool {
    let key = pair.split('=').next().unwrap_or(pair).to_ascii_lowercase();
    matches!(
        key.as_str(),
        "core.editor"
            | "core.pager"
            | "core.fsmonitor"
            | "core.hookspath"
            | "core.sshcommand"
            | "core.askpass"
            | "core.gitproxy"
            | "sequence.editor"
            | "diff.external"
            | "credential.helper"
    ) || key.ends_with(".textconv")
        || key.ends_with(".command")
        || key.ends_with(".program")
        || key.ends_with(".clean")
        || key.ends_with(".smudge")
        || key.ends_with(".process")
        || key.starts_with("alias.")
        || key.starts_with("pager.")
}

/// Whether the program placed in `argv` (`argv[0]`), with the assignments `env` in front of it,
/// provably never executes a word among its operands.
fn never_executes(base: &str, argv: &[String], env: &[String]) -> bool {
    if base == "rg" {
        // `--pre` and `--pre-glob` run a program on every file.
        return !argv.iter().any(|w| w.starts_with("--pre"));
    }
    if NAMES_ONLY.contains(&base) {
        return true;
    }
    if base != "git" {
        return false;
    }
    // A `GIT_*` assignment may name an editor, a pager or an ssh command.
    if env.iter().any(|w| w.starts_with("GIT_")) {
        return false;
    }
    // **Names only when the subcommand is the first word** (round 12): any other flag before it
    // may take a value that is itself a listed subcommand (`git --namespace log bisect run gh …`),
    // so the real subcommand is behind it. Only `-C <dir>` and `-c <k=v>` pairs may come first:
    // each is known to take one value, and a `-c` value may name a program (round 13).
    let mut rest = argv.iter().skip(1);
    let sub = loop {
        match rest.next().map(String::as_str) {
            Some("-C") => {
                rest.next();
            }
            Some("-c") => {
                if rest.next().is_some_and(|v| git_config_runs_a_program(v)) {
                    return false;
                }
            }
            other => break other,
        }
    };
    let Some(sub) = sub else { return false };
    if !GIT_SUBCOMMANDS.contains(&sub) {
        return false;
    }
    // `git grep -O` opens each match in a pager command; `git fetch --upload-pack` runs one.
    let runs = |w: &String| match sub {
        "grep" => w.starts_with("-O") || w.starts_with("--open-files-in-pager"),
        "fetch" => w.starts_with("--upload-pack") || w.starts_with("--receive-pack"),
        _ => false,
    };
    !rest.any(runs)
}

/// **The general wrapper rule** (#7423, rounds 7, 8 and 11). The program placed is not a forge
/// CLI, and a later word is one: whatever the program is (`taskset 1`, `chronic`, `ssh h`,
/// `fd -x`, a wrapper no table lists), it may run that call. Only a program that
/// [`never_executes`] its operands, and the words of a `for … in` list, are left alone.
///
/// Answers whether a shell-filled word stands before a held command, and the calls to read, each
/// as a segment of its own. A word or a path that names a forge CLI (`gh`, `/opt/homebrew/bin/gh`)
/// is always a call, read by the allowlist of [`super::placed`] with whatever the program may
/// append. A word the shell fills in (`$(which gh)`, `$GH`, `{gh,pr}`, `g[h]`) followed by a
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
    if never_executes(&base, argv, &inv.env) {
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
            // A word or a path that names a forge CLI, behind a program that may run it, is a
            // call (round 11): read by the allowlist ([`super::placed`]), with whatever the
            // program may append. `cp /usr/bin/gh ~` is no call: `cp` never runs its operands.
            if !names_a_forge(word) {
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
