// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A shell command line as **argv per separately-executed command** — the substrate every
//! `PreToolUse` Bash refusal stands on.
//!
//! This is a port of `charter/hooks.py`'s `_segment_argv_parsed` and its whole transitive
//! closure: `_unbacktick`, `_ShellLexer` (a subclass of CPython's `shlex.shlex`),
//! `_NewlineKeepingStream`, `_lex`, `_split_punctuation`, `_segment_tokens`, `_resegment`,
//! `_quote_map`, `_splice_continuations` and `_fallback_segments`. Twenty definitions and
//! 565 lines of Python, measured by an AST walk over the frozen oracle.
//!
//! # Where this is called from
//!
//! [`crate::toolgate`] — `charter/hooks.py:pretooluse`'s eight refusals, in its order — and
//! through it `charter hook pretooluse` (M3.1 stage 6).
//!
//! Until that stage the switch was closed: `main.rs`'s `is_a_tool_hook` answered every word in
//! the `pretooluse`/`posttooluse` namespace with exit 2 — *block* — because a PARTIAL guard on
//! the switch turns fail-closed into allow-everything-except-the-arm-that-is-ported. It moved
//! once all eight arms were standing, and never earlier. The other eight words in that
//! namespace still block, for the same reason they always did.
//! # Why this is the piece to get exactly right
//!
//! Every refusal above it decides on `prog` and `args` taken from these segments. A boundary
//! this module invents strands a reader's operand — `cat` in one segment and the vault path in
//! another, neither of which is a read — and the guard goes from deny to **allow**. The Python
//! docstrings record four separate rounds where exactly that shipped: a quoted `)`, the `&` of
//! `2>&1`, a `{` read as a reserved word mid-command, and `str.split` erasing the newlines that
//! separate the lines of a multi-line command. Each of those is a rule here, and each has a
//! test that fails when the rule is removed.
//!
//! # The answers are CPython's, not "a POSIX shell's"
//!
//! The oracle is `shlex.shlex(posix=True, punctuation_chars="();<>|&\n")` with
//! `whitespace_split=True` and `whitespace=" \t\r"`, driven through a stream whose `readline`
//! stops *before* the newline. That state machine — not the POSIX grammar — is what the Python
//! charter refuses on, so this reproduces the state machine. Three consequences are worth
//! naming because they are not what a shell does:
//!
//! - **A run of punctuation is one token.** `);` and `)&&` and `<(` come back glued, and
//!   [`split_punctuation`] breaks them apart with the same longest-first table the fallback
//!   path uses, so the two paths cannot come to disagree about what an operator is.
//! - **Offsets are CHARACTER offsets**, because CPython indexes a `str` by character. A7 reads
//!   [`Tok::start`] and [`Tok::end`] to judge the *spelling* a command was written with, so the
//!   whole module works over `Vec<char>` rather than bytes.
//! - **`str.strip()` and `str.split()` are Python's**, which count U+001C–U+001F as whitespace
//!   where Rust's `char::is_whitespace` does not. [`crate::memstore::is_python_space`] already
//!   existed for this and is reused rather than re-derived.
//!
//! # The evidence
//!
//! The differential harness ran the frozen Python and put the same question to this module, six
//! answers at a time, over 200,000 generated command lines. Its answers are recorded
//! (`fixtures/corpora/shellseg-oracle.jsonl`, and `shellseg-generated.jsonl.gz`, a subset of the
//! generated cases chosen to reach every branch the full run reached — ADR 0046), and
//! `tests/it/the_shell_is_read_the_way_python_reads_it.rs` replays them with no Python present.
//! Nothing in either is written by hand: a rule this module gets wrong changes an answer the
//! Python already gave.
//!
//! # What stage 2 added here
//!
//! [`Quoting`] (the memoisation the oracle gets from `lru_cache`), [`posix_split`] and
//! [`shell_quote`] (CPython's `shlex.split`/`shlex.quote`, which `_split_env_chdir` and
//! `_desugar_ansi_c` call), and [`Tok::is_control_op`]/[`Tok::is_grouping`]. Everything else in
//! this file is stage 1 unchanged: the lexer now reads its three tables off a `Cfg` so the two
//! `shlex` configurations the oracle uses are one state machine rather than two.

use crate::guardcaps::MAX_NESTING;
use crate::memstore::is_python_space;

/// The characters [`Lexer`] treats as punctuation, and which it emits as a glued RUN.
///
/// `\n` is here, and NOT in [`WHITESPACE`], because a newline separates two commands exactly
/// as `;` does. `shlex`'s default whitespace swallows it, and a multi-line Bash call — which is
/// most of them — then collapses into a single segment whose program is the first line's.
const PUNCTUATION_CHARS: &str = "();<>|&\n";

/// What the lexer drops between tokens. `\n` is deliberately absent; see [`PUNCTUATION_CHARS`].
const WHITESPACE: &str = " \t\r";

const QUOTES: &str = "'\"";
const ESCAPE: char = '\\';
/// The quotes inside which a backslash still escapes — `shlex.escapedquotes`.
const ESCAPED_QUOTES: &str = "\"";

/// `shlex`'s `wordchars` under `posix=True` with our `punctuation_chars`, reproduced exactly.
///
/// **It is inert here and is kept anyway.** Under `whitespace_split = True` both arms that
/// consult it reach the same state as the arm that would otherwise catch the character: in the
/// whitespace state a non-wordchar that is not punctuation, a quote or an escape falls to
/// `elif self.whitespace_split`, which sets the identical `token`/`state`; in the word state the
/// disjunct `whitespace_split and nextchar not in punctuation_chars` already covers every
/// wordchar, since `wordchars` and `punctuation_chars` are disjoint (CPython's `__init__`
/// translates the punctuation out of the wordchars). Kept because the port's claim is
/// "CPython's state machine", and a reader checking that claim against `shlex.py` should find
/// every table it looks for.
const WORDCHARS: &str = concat!(
    // CPython's own order, typo and all (`abcdfe…`); only membership is read.
    "abcdfeghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_",
    "ßàáâãäåæçèéêëìíîïðñòóôõöøùúûüýþÿÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖØÙÚÛÜÝÞ",
    "~-./*?=",
);

/// How a glued run breaks into the SHELL'S OWN tokens — `_OPERATOR_SPLIT_RE`, in the
/// alternation's own order, because a regex alternation takes the FIRST alternative that matches
/// and not the longest.
///
/// **What keeps `cat 2>&1 <vault>` in one segment is that `>&` is IN this table**, and that it
/// precedes the `>` that is a prefix of it. Cutting a punctuation run into operator *characters*
/// splits that command at the `&` and strands the vault path in a segment of its own: the reader
/// loses its operand, and the guard answers allow. Removing `>&` from the table, or putting the
/// bare `<`/`>` in front of the two- and three-character spellings, was each measured to change
/// thousands of answers in the differential fuzz.
///
/// The redirections stand ahead of the control operators because the Python regex is built that
/// way. That much is inert and is written down as inert: no control operator is a prefix of any
/// redirection — every redirection begins `<` or `>` — so moving the two groups past each other
/// changes no answer, measured over 20,000 fuzz cases. The ordering that is load-bearing is
/// longest-first *within* a prefix.
///
/// One table, read by [`split_punctuation`] on a lexed run and by [`resegment`] on a string that
/// never reached the tokenizer, so the parsed and the unparseable paths cannot disagree about
/// what a boundary is. The regex ends in the character class `[(){}]`, which is
/// [`GROUPING_SPLIT`] here.
const OPERATOR_ALTERNATION: [&str; 25] = [
    // `&>>` and `&>` send both streams to a file: one redirection, and no `&` that ends the
    // command before it (#7423). Before the `&` they begin with. zsh's clobber forms that end in
    // a pipe (`>>|`, `&>|`, `>&|`) are one operator, and no `|` that ends the command (round 7),
    // and so are the append-both forms `>>&`, `>>&|` and `&>>|` (round 8).
    "&>>|", ">>&|", "&>>", "&>|", ">>&", ">>|", ">&|", "&>", "<<<", "<<", "<>", "<&", ">>", ">&",
    ">|", "<", ">", // the redirections, first
    "||", "&&", ";;", ";", "|&", "|", "&", "\n", // the control operators
];

/// The regex's trailing `[(){}]`, tried after every alternative in [`OPERATOR_ALTERNATION`].
const GROUPING_SPLIT: [char; 4] = ['(', ')', '{', '}'];

/// The CONTROL operators: they end one separately-executed command and begin the next wherever
/// they stand. Membership is necessary and **not sufficient** — a token is a boundary only
/// where the shell would INTERPRET it, which [`Tok::bare`] records.
const CONTROL_OPERATORS: [&str; 8] = [";", ";;", "&&", "||", "|", "|&", "&", "\n"];

/// The GROUPING tokens, a boundary only where a shell RECOGNISES one — see [`segment_tokens`],
/// which is where position is decided.
const GROUPING: [&str; 4] = ["(", ")", "{", "}"];

/// Tokens that turn the `(` after them into a SUBSTITUTION rather than a subshell: `<(` and
/// `>(` process substitution. `$` is matched as a SUFFIX instead, because `x$(…)` lexes as `x$`.
const PROCSUB_LEAD: [&str; 2] = ["<", ">"];

/// Whether zsh would NOT run a `=(` whose `=` is at `i`, because it ends a name being assigned
/// (`a=(…)`, `a[1]=(…)`, `a+=(…)` are arrays) or follows a quote that is part of the same word
/// (`""=(…)`). Everywhere else the `=(` reads as live: zsh runs it at the start of a word, as
/// the value of an assignment (`v==(…)`) and as the operand of a `${…}` operator
/// (`${v:-=(…)}`), and bash refuses the whole command, so reading more positions as live costs
/// nothing where it does not run.
///
/// Here, beside the reader, because three walks ask it: the live-substitution walk
/// ([`crate::livesub::process_substitution_at`]), the segmenter's [`opens_equals_substitution`]
/// and the heredoc layout's substitution spans.
pub fn equals_ends_a_name(chars: &[char], i: usize) -> bool {
    let name_end = |c: char| c.is_alphanumeric() || c == '_' || c == ']';
    match i.checked_sub(1).map(|k| chars[k]) {
        Some(c) if name_end(c) || c == '"' || c == '\'' => true,
        Some('+') => i >= 2 && name_end(chars[i - 2]),
        _ => false,
    }
}

/// Whether `prev`, a bare word, and the `(` after it are zsh's `=(…)`, which runs its command
/// and hands the program a temporary file holding the output — a substitution, as `<(…)` is.
///
/// The positions are exactly the ones [`crate::livesub::process_substitution_at`] reads as
/// live, by the same test ([`equals_ends_a_name`]): the `=` begins the word
/// (`=(…)`), ends an assignment's `=` (`v==(…)`) or follows a `${…}` operator (`${v:-=(…)}`),
/// and not a name (`a=(…)`, `a[1]=(…)` and `a+=(…)` are arrays). The word is bare, so a quoted
/// part (`""=(…)`) or an escaped `=` opens nothing. The `(` must touch the `=`: `= (…)` is no
/// substitution in zsh. A `(` whose offset nothing measured (the fallback path) counts as
/// touching, which errs toward reading more segments. Checked against zsh 5.9; bash refuses
/// every one of these lines.
fn opens_equals_substitution(prev: &Tok, paren: &Tok) -> bool {
    let chars: Vec<char> = prev.text.chars().collect();
    let Some(at) = chars.len().checked_sub(1) else {
        return false;
    };
    let touching = prev.end < 0 || paren.start < 0 || prev.end == paren.start;
    chars[at] == '=' && touching && !equals_ends_a_name(&chars, at)
}

/// The quoting contexts a position can sit in. `(` and a backtick are SUBSTITUTIONS — inside
/// them quoting starts again from nothing — so they are on the stack but are not "quoted".
/// `$"…"` is not here because it opens the same `"` context an ordinary double quote does.
const QUOTED_CONTEXTS: [&str; 3] = ["'", "\"", "$'"];

/// What may stand in front of the `#` that begins a comment.
const BEFORE_COMMENT: &str = " \t\n;|&()<>";

/// One token, plus whether the shell would INTERPRET its text as punctuation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tok {
    pub text: String,
    /// True when the token was produced without the lexer ever entering a quote state or an
    /// escape state — that is, every character of it stood unquoted and unescaped in the
    /// source. Only a bare token can be an operator: `\)`, `')'`, `"("`, `'&&'` and a
    /// single-quoted newline are ordinary words to a shell, and a word inside a reader's argv
    /// is an operand, not a boundary.
    pub bare: bool,
    /// Where the token's first character sits in the source, in CHARACTERS, or -1 where nothing
    /// measured it. Read by A7 alone, which judges the SPELLING a command was written with:
    /// `'charter' handoff` and `charter  handoff` lex to the same two texts as the exact form,
    /// and `charter $'handoff'` to a text no reader of words recognises at all — only the
    /// source tells any of them apart.
    pub start: isize,
    /// One past the last source character of the token, or -1.
    pub end: isize,
    /// Whether an unquoted blank stood inside a `${…}` of the token, which the shell splits the
    /// expansion's value at: `${x:-gh pr merge 1}` is a whole command, and `${x:-"a b"}` one
    /// word (#7709, round 3).
    pub split_in_braces: bool,
}

impl Tok {
    fn new(text: impl Into<String>, bare: bool, start: isize, end: isize) -> Self {
        Self {
            text: text.into(),
            bare,
            start,
            end,
            split_in_braces: false,
        }
    }

    /// True when the shell would interpret this token as one of `texts`.
    pub fn is_op(&self, texts: &[&str]) -> bool {
        self.bare && texts.contains(&self.text.as_str())
    }

    /// True when the shell would interpret this token as any operator at all.
    pub fn is_any_op(&self) -> bool {
        self.bare
            && (CONTROL_OPERATORS.contains(&self.text.as_str())
                || GROUPING.contains(&self.text.as_str()))
    }

    /// True when the shell would interpret this token as a CONTROL operator — one that ends a
    /// separately-executed command. `_Tok.is_op(*_CONTROL_OPERATORS)` in the oracle.
    pub fn is_control_op(&self) -> bool {
        self.is_op(&CONTROL_OPERATORS)
    }

    /// True when the shell would interpret this token as a grouping token.
    pub fn is_grouping(&self) -> bool {
        self.is_op(&GROUPING)
    }
}

/// `_quote_map`'s answer for one line, computed ONCE.
///
/// The Python is `@functools.lru_cache(maxsize=512)`, and that cache is not decoration: the
/// callers above this module ask `_inside_quotes(line, i)` **per position**, and each of those
/// calls is an O(n) walk of the whole line. Uncached that is O(n²) per line, and
/// `_heredoc_opener_words`, `_crowded_substitutions` and `_pipeline_slice` each run one of those
/// walks. Rust has no decorator to inherit, so the memoisation is structural instead: the map is
/// built once per line and handed to everything that reads it, which is a cache that cannot
/// miss and needs no size.
#[derive(Clone, Debug)]
pub struct Quoting {
    flags: Vec<bool>,
}

impl Quoting {
    /// [`quote_map`] for `line`, kept for as long as the caller reads it.
    pub fn of(line: &str) -> Self {
        Self {
            flags: quote_map(line),
        }
    }

    /// `_inside_quotes`: whether offset `at` sits inside quotes.
    ///
    /// Python's `flags[at] if 0 <= at < len(flags) else flags[-1]`, and the `else` is reachable
    /// in both directions: a negative offset and an offset past the end both answer with the
    /// state at END of input. The map has one more entry than the line has characters, so
    /// `at == line.chars().count()` is an ordinary in-range read of that same last entry.
    pub fn inside(&self, at: isize) -> bool {
        // `(0..n).contains(&at)` rather than `at >= 0 && at < n`: the same test, written the way
        // clippy asks for it.
        let n = self.flags.len() as isize;
        if (0..n).contains(&at) {
            self.flags[at as usize]
        } else {
            self.at_end()
        }
    }

    /// The state at end of input — whether a quote is still open.
    pub fn at_end(&self) -> bool {
        // `quote_map` always returns `n + 1` entries, so there is always a last one.
        *self.flags.last().unwrap_or(&false)
    }

    /// The raw flags, one per character plus the end.
    pub fn flags(&self) -> &[bool] {
        &self.flags
    }
}

/// What `shlex` raises, and the only thing [`lex`] can fail with. `_segment_argv_parsed` owns
/// the fallback; nothing above it may read an error as "no command here".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexError {
    /// `ValueError("No closing quotation")`.
    NoClosingQuotation,
    /// `ValueError("No escaped character")`.
    NoEscapedCharacter,
}

/// `shlex`'s state, as the state machine spells it: `' '`, `None`, `'a'`, `'c'`, a quote
/// character, or the escape character.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum St {
    Space,
    Eof,
    Word,
    Punct,
    Quote(char),
    Esc(char),
    /// Inside bash's ANSI-C quotation `$'…'`, which [`Cfg::bash_quoting`] turns on. Its text is
    /// gathered raw and decoded by [`ansi_c_decode`] when the closing quote arrives.
    Ansi,
}

/// The character source the lexer reads, whose `readline` stops BEFORE the newline instead of
/// consuming it.
///
/// `shlex` ends a comment by calling `instream.readline()`. With `\n` as an operator that would
/// swallow the separator too, and `echo a # note` + newline + `cat <vault>` would go back to
/// being one segment whose program is `echo` — the comment bypass in a second spelling.
struct NewlineKeepingStream {
    s: Vec<char>,
    i: usize,
}

impl NewlineKeepingStream {
    fn read1(&mut self) -> Option<char> {
        let c = self.s.get(self.i).copied();
        if c.is_some() {
            self.i += 1;
        }
        c
    }

    /// Everything up to the next `\n`, leaving the `\n` itself in the stream.
    fn readline(&mut self) {
        let end = self.s[self.i..]
            .iter()
            .position(|&c| c == '\n')
            .map_or(self.s.len(), |off| self.i + off);
        self.i = end;
    }
}

/// The three `shlex.shlex` tables this module instantiates in two different ways.
///
/// One state machine, two configurations, because the oracle uses two: `_ShellLexer` for a
/// command line, and CPython's own `shlex.split` inside `_split_env_chdir` for `env -S`'s packed
/// string. Writing the second as a second machine is how two readings of one construct come to
/// disagree — the mistake this whole area keeps paying for — so it is the same code with a
/// different table.
#[derive(Clone, Copy)]
struct Cfg {
    /// What is dropped between tokens.
    whitespace: &'static str,
    /// What is emitted as a glued RUN. Empty for `shlex.split`, which has no punctuation
    /// characters at all, so its lexer never reaches the punctuation state.
    punctuation: &'static str,
    /// Whether `#` begins a comment. `shlex.split(s)` is `comments=False`, so its `commenters`
    /// is empty and its `readline` is never reached.
    comments: bool,
    /// Whether the three quoting forms `shlex` does not know are read the way the shell reads
    /// them: `$'…'` decoded ([`ansi_c_decode`]), `$"…"` read as `"…"`, and a live
    /// backslash-newline removed. Only a command line is read that way; `shlex.split`'s callers
    /// (`env -S`, a git alias body) hand the string to programs that do not do it.
    bash_quoting: bool,
}

/// `_ShellLexer`: what a charter command line is read with.
const SHELL_CFG: Cfg = Cfg {
    whitespace: WHITESPACE,
    punctuation: PUNCTUATION_CHARS,
    comments: true,
    bash_quoting: true,
};

/// CPython's `shlex.split(s)`: `posix=True`, `whitespace_split=True`, `comments=False`, and no
/// `punctuation_chars`, which leaves `whitespace` at `shlex`'s default — the newline INCLUDED,
/// unlike the command-line reading above.
///
/// `wordchars` differs too (CPython only adds `~-./*?=` and removes the punctuation when
/// `punctuation_chars` is truthy), and the difference is inert for the same reason
/// [`WORDCHARS`] is inert here at all: under `whitespace_split` both arms that consult it reach
/// the same state as the arm that would otherwise catch the character, and with no punctuation
/// characters the disjunct `whitespace_split and nextchar not in punctuation_chars` is simply
/// true. So one table serves both.
const SPLIT_CFG: Cfg = Cfg {
    whitespace: " \t\r\n",
    punctuation: "",
    comments: false,
    bash_quoting: false,
};

/// `shlex.shlex` in posix mode, plus the three things `_ShellLexer` changes about it.
struct Lexer {
    cfg: Cfg,
    instream: NewlineKeepingStream,
    state: St,
    token: Vec<char>,
    pushback_chars: Vec<char>,
    bare: bool,
    start: isize,
    end: isize,
    /// How many unquoted, unescaped `$` end the token so far. A `$'` or `$"` opens a bash
    /// quotation only when this is ODD: `$$` is the shell's PID, so `$$'x'` is the PID and a
    /// plain `'x'`, and `$$$'x'` is the PID and an ANSI-C `$'x'` (checked against bash).
    dollars: usize,
    /// `bare` and `start` as they stood before the escape being read, so a backslash-newline
    /// that the shell removes leaves the token as though it had never been there.
    before_esc: (bool, isize),
    /// The raw text of the `$'…'` being read, escapes still in it.
    ansi: Vec<char>,
    /// How many parameter expansions (`${t%%|*}`) the word is inside, the outermost and each
    /// nested `${` in it. While it is more than none, a blank or an operator character is part of
    /// the word, as the shell reads it (#7709).
    braces: usize,
    /// Where the outermost `${` of the word was opened: the stream's cursor, the characters
    /// pushed back, the token's length and its `bare`, `start` and quoting just past the `{`.
    /// A `${` that does not close, or that holds a substitution, is read again from there the
    /// way it always was.
    brace_mark: Option<(usize, Vec<char>, usize, bool, isize, bool)>,
    /// Whether this word's `${` was read again that way, so that no `${` in it is one word.
    braces_off: bool,
    /// How far into the stream a `${` that was read again had been read: no `${` opened before
    /// there is read as one word, since it was inside that one and is read again with it. Kept
    /// across words, so that each character is read at most twice and a line of unclosed `${`
    /// costs its length, not its square (#7709, round 2).
    braces_read_to: usize,
    /// Whether the word held an unquoted blank inside its `${…}` ([`Tok::split_in_braces`]).
    split_in_braces: bool,
}

impl Lexer {
    fn new(cmd: &str, cfg: Cfg) -> Self {
        Self {
            cfg,
            instream: NewlineKeepingStream {
                s: cmd.chars().collect(),
                i: 0,
            },
            state: St::Space,
            token: Vec::new(),
            pushback_chars: Vec::new(),
            bare: true,
            start: -1,
            end: -1,
            dollars: 0,
            before_esc: (true, -1),
            ansi: Vec::new(),
            braces: 0,
            brace_mark: None,
            braces_off: false,
            braces_read_to: 0,
            split_in_braces: false,
        }
    }

    /// Every `self.state = …` in `read_token` goes through `_ShellLexer`'s property setter, and
    /// this is that setter. It is the whole mechanism behind `bare`, `start` and `end`.
    fn set_state(&mut self, value: St) {
        // `shlex` sets `state` to the quote character it is inside, or to the escape character
        // it is honouring, and to nothing else that is not a plain word/punctuation marker — so
        // this catches quoting wherever it appears in the token, including a quote glued to the
        // middle of one.
        if matches!(value, St::Quote(_) | St::Esc(_) | St::Ansi) {
            self.bare = false;
        }
        // The same transition says where the token STARTS. `shlex` leaves whitespace on the
        // token's first character, and that is the last character read from the stream: a
        // character `shlex` pushed back at the end of the previous token was read from the
        // stream and is not re-read, so the cursor is already one past it either way.
        if self.state == St::Space && value != St::Space && self.start < 0 {
            self.start = self.instream.i as isize - 1;
        }
        // And where it ENDS. Leaving a word or a punctuation run for whitespace, the lexer has
        // just read the one character that ended the token — a blank it drops, or a character it
        // pushes back — so the token ends one before the cursor; at the end of the input, at it.
        if matches!(self.state, St::Word | St::Punct) && matches!(value, St::Space | St::Eof) {
            self.end = self.instream.i as isize - if value == St::Space { 1 } else { 0 };
        }
        self.state = value;
    }

    /// Whether the `{` just read opens a parameter expansion (#7709): it follows a `$` the word
    /// holds unquoted and unescaped, and the next character begins one, a name, a digit or one of
    /// `#?@*!$-`. A bash 5.3 `${ …; }` or `${| …; }`, which runs a command, is left to the
    /// reading it always had, which splits it at its operators.
    fn opens_a_parameter_expansion(&self) -> bool {
        if !self.cfg.bash_quoting
            || self.braces_off
            || self.dollars.is_multiple_of(2)
            || self.instream.i < self.braces_read_to
        {
            return false;
        }
        self.next_begins_a_parameter()
    }

    /// Whether the character after the one just read begins a parameter expansion's name: a
    /// letter, a digit or one of `#?@*!$-`. A blank, a newline or a `|` after `${` opens a bash
    /// 5.3 funsub instead, which runs a command.
    fn next_begins_a_parameter(&self) -> bool {
        let next = self
            .pushback_chars
            .last()
            .or_else(|| self.instream.s.get(self.instream.i));
        next.is_some_and(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '_' | '#' | '?' | '@' | '*' | '!' | '$' | '-')
        })
    }

    /// A shell begins a comment at `#` only where a WORD BEGINS. `shlex` honours it mid-word
    /// too and swallows the rest of the line, so `echo hi#; cat <vault>` — which runs the `cat`
    /// in bash — lexed as a lone `echo hi` and every later command became invisible.
    fn commenters(&self) -> &'static str {
        if self.cfg.comments && self.state == St::Space {
            "#"
        } else {
            ""
        }
    }

    /// CPython's `shlex.read_token`, with `_ShellLexer`'s overrides folded in.
    fn read_token(&mut self) -> Result<Option<String>, LexError> {
        self.bare = true; // per token, not per lex
        self.start = -1;
        self.end = -1;
        self.dollars = 0;
        self.braces = 0;
        self.brace_mark = None;
        self.braces_off = false;
        self.split_in_braces = false;
        let mut quoted = false;
        let mut escapedstate = St::Space;
        loop {
            let nextchar = match self.pushback_chars.pop() {
                Some(c) => Some(c),
                None => self.instream.read1(),
            };
            match self.state {
                St::Eof => {
                    self.token.clear();
                    break;
                }
                St::Space => {
                    let Some(c) = nextchar else {
                        self.set_state(St::Eof);
                        break;
                    };
                    if self.cfg.whitespace.contains(c) {
                        if !self.token.is_empty() || quoted {
                            break;
                        }
                        continue;
                    } else if self.commenters().contains(c) {
                        self.instream.readline();
                    } else if c == ESCAPE {
                        escapedstate = St::Word;
                        self.before_esc = (self.bare, self.start);
                        self.set_state(St::Esc(c));
                    } else if WORDCHARS.contains(c) {
                        self.token = vec![c];
                        self.set_state(St::Word);
                    } else if self.cfg.punctuation.contains(c) {
                        self.token = vec![c];
                        self.set_state(St::Punct);
                    } else if QUOTES.contains(c) {
                        self.set_state(St::Quote(c));
                    } else {
                        // `whitespace_split` is true, so this is the arm that catches
                        // everything else, and it is the same assignment `wordchars` makes.
                        self.token = vec![c];
                        self.dollars = usize::from(c == '$');
                        self.set_state(St::Word);
                    }
                }
                St::Quote(q) => {
                    quoted = true;
                    let Some(c) = nextchar else {
                        return Err(LexError::NoClosingQuotation);
                    };
                    if c == q {
                        self.set_state(St::Word);
                    } else if c == ESCAPE && ESCAPED_QUOTES.contains(q) {
                        escapedstate = St::Quote(q);
                        self.before_esc = (self.bare, self.start);
                        self.set_state(St::Esc(c));
                    } else {
                        self.token.push(c);
                    }
                }
                St::Esc(e) => {
                    let Some(c) = nextchar else {
                        return Err(LexError::NoEscapedCharacter);
                    };
                    if c == '\n' && self.cfg.bash_quoting {
                        // A live backslash-newline: the shell removes both before it reads a
                        // word, outside quotes and inside `"…"` alike, so `c\<newline>at` runs
                        // `cat` and `"$\<newline>(x)"` runs `x`. It is not quoting, so the token
                        // is left as bare as it was; and one that stood between two words
                        // belongs to neither.
                        (self.bare, self.start) = self.before_esc;
                        if escapedstate == St::Word && self.token.is_empty() && !quoted {
                            self.set_state(St::Space);
                        } else {
                            self.state = escapedstate;
                        }
                        continue;
                    }
                    self.dollars = 0;
                    // In posix shells, only the quote itself or the escape character may be
                    // escaped within quotes.
                    if let St::Quote(q) = escapedstate
                        && c != e
                        && c != q
                    {
                        self.token.push(e);
                    }
                    self.token.push(c);
                    self.set_state(escapedstate);
                }
                St::Ansi => {
                    let Some(c) = nextchar else {
                        return Err(LexError::NoClosingQuotation);
                    };
                    if c == '\'' {
                        let raw = std::mem::take(&mut self.ansi);
                        self.token.extend(ansi_c_decode(&raw).chars());
                        self.set_state(St::Word);
                    } else if c == ESCAPE {
                        // The escaped character is taken whatever it is, so `\'` does not close.
                        let Some(next) =
                            self.pushback_chars.pop().or_else(|| self.instream.read1())
                        else {
                            return Err(LexError::NoClosingQuotation);
                        };
                        self.ansi.push(c);
                        self.ansi.push(next);
                    } else {
                        self.ansi.push(c);
                    }
                }
                St::Word | St::Punct => {
                    // A `${` that never closes, or that holds a substitution, is read from its
                    // `{` on as it always was: split at its operators, so that the commands a
                    // `$(…)`, a `` `…` ``, a `<(…)` or a zsh `=(…)` in it runs stay segments
                    // of their own, and an unclosed one reads as the recorded answers read it.
                    // A nested bash 5.3 funsub (`${ …; }`, `${| …; }`) runs commands too.
                    if self.state == St::Word
                        && self.braces > 0
                        && nextchar.is_none_or(|c| {
                            matches!(c, '(' | '`')
                                || (c == '{'
                                    && self.dollars % 2 == 1
                                    && !self.next_begins_a_parameter())
                        })
                        && let Some((at, pushed, len, bare, start, was_quoted)) =
                            self.brace_mark.take()
                    {
                        self.braces_read_to = self.braces_read_to.max(self.instream.i);
                        self.instream.i = at;
                        self.pushback_chars = pushed;
                        self.token.truncate(len);
                        (self.bare, self.start, quoted) = (bare, start, was_quoted);
                        self.braces = 0;
                        self.braces_off = true;
                        self.split_in_braces = false;
                        self.dollars = 0;
                        continue;
                    }
                    let Some(c) = nextchar else {
                        self.set_state(St::Eof);
                        break;
                    };
                    if self.state == St::Word
                        && self.braces > 0
                        && !QUOTES.contains(c)
                        && c != ESCAPE
                    {
                        // Inside `${…}` the word runs to the first unquoted `}`, past blanks and
                        // operator characters. bash and dash count only a nested `${` on the way:
                        // a bare `{` opens nothing, so `${s#{}` ends at its first `}`. zsh counts
                        // every `{`, so it reads such a line as fewer commands, never more.
                        match c {
                            '{' if self.dollars % 2 == 1 => self.braces += 1,
                            '}' => self.braces -= 1,
                            ' ' | '\t' | '\n' => self.split_in_braces = true,
                            _ => {}
                        }
                        self.dollars = if c == '$' { self.dollars + 1 } else { 0 };
                        self.token.push(c);
                        continue;
                    }
                    if self.cfg.whitespace.contains(c) {
                        self.set_state(St::Space);
                        if !self.token.is_empty() || quoted {
                            break;
                        }
                        continue;
                    } else if self.commenters().contains(c) {
                        // Unreachable: `commenters()` is empty outside the whitespace state,
                        // which is the narrowing `_ShellLexer` exists for. Kept so the shape of
                        // CPython's machine is visible where a reader looks for it.
                        self.instream.readline();
                        self.set_state(St::Space);
                        if !self.token.is_empty() || quoted {
                            break;
                        }
                        continue;
                    } else if self.state == St::Punct {
                        if self.cfg.punctuation.contains(c) {
                            self.token.push(c);
                        } else {
                            if !self.cfg.whitespace.contains(c) {
                                self.pushback_chars.push(c);
                            }
                            self.set_state(St::Space);
                            break;
                        }
                    } else if QUOTES.contains(c) {
                        if self.cfg.bash_quoting && self.dollars % 2 == 1 {
                            // `$'…'` and `$"…"`: the `$` is part of the quotation, not the word.
                            self.token.pop();
                            if c == '\'' {
                                quoted = true;
                                self.dollars = 0;
                                self.set_state(St::Ansi);
                                continue;
                            }
                        }
                        self.dollars = 0;
                        self.set_state(St::Quote(c));
                    } else if c == ESCAPE {
                        escapedstate = St::Word;
                        self.before_esc = (self.bare, self.start);
                        self.set_state(St::Esc(c));
                    } else if WORDCHARS.contains(c)
                        || QUOTES.contains(c)
                        || !self.cfg.punctuation.contains(c)
                    {
                        let opens = c == '{' && self.opens_a_parameter_expansion();
                        self.dollars = if c == '$' { self.dollars + 1 } else { 0 };
                        self.token.push(c);
                        if opens {
                            self.braces = 1;
                            self.brace_mark = Some((
                                self.instream.i,
                                self.pushback_chars.clone(),
                                self.token.len(),
                                self.bare,
                                self.start,
                                quoted,
                            ));
                        }
                    } else {
                        self.pushback_chars.push(c);
                        self.set_state(St::Space);
                        if !self.token.is_empty() || quoted {
                            break;
                        }
                        continue;
                    }
                }
            }
        }
        let result: String = std::mem::take(&mut self.token).into_iter().collect();
        // posix: an empty token that was never quoted is end of input, and `''` is a real token.
        if !quoted && result.is_empty() {
            return Ok(None);
        }
        Ok(Some(result))
    }
}

/// `cmd` as [`Tok`]s, or the error `shlex` raises on genuinely unbalanced quoting.
pub fn lex(cmd: &str) -> Result<Vec<Tok>, LexError> {
    let mut lexer = Lexer::new(cmd, SHELL_CFG);
    let mut out = Vec::new();
    loop {
        match lexer.read_token()? {
            // `None`, not `""`: `cat ''` produces a real empty token.
            None => return Ok(out),
            Some(text) => out.push(Tok {
                split_in_braces: lexer.split_in_braces,
                ..Tok::new(text, lexer.bare, lexer.start, lexer.end)
            }),
        }
    }
}

/// CPython's `shlex.split(s)` — **not** [`lex`].
///
/// `_split_env_chdir` calls it on the string `env -S` packs a whole command into, and it is a
/// different reading from a command line's: no punctuation characters (so `;` is an ordinary
/// word character rather than an operator), no comments, and a newline is plain whitespace.
/// Only the texts come back; `bare` and the offsets mean nothing on this path and the oracle
/// discards them too.
pub fn posix_split(s: &str) -> Result<Vec<String>, LexError> {
    let mut lexer = Lexer::new(s, SPLIT_CFG);
    let mut out = Vec::new();
    loop {
        match lexer.read_token()? {
            None => return Ok(out),
            Some(text) => out.push(text),
        }
    }
}

/// The text of an ANSI-C quotation `$'…'` — `raw` is what stood between the quotes, escapes
/// still in it — as the shell hands it to the program.
///
/// Every guard matches the words a program receives, and `$'\x67it'` is `git` to the shell, so
/// the one reader behind them all has to decode it: a guard that saw `$\x67it` saw no program at
/// all. Checked against GNU bash 3.2.57 and 5.2.21 and zsh 5.9 with `printf '%s|'`:
///
/// - `\a \b \e \E \f \n \r \t \v \\ \' \" \?` are the characters they name;
/// - `\NNN` is one to three OCTAL digits and `\xHH` one or two hex digits, each ONE BYTE (so
///   `\777` is `0xff`, and `\303\251` is the two bytes of `é`); bash also reads `\x{…}`, all the
///   hex digits inside the braces, as one byte;
/// - `\uHHHH` and `\UHHHHHHHH` are one to four and one to eight hex digits naming a character,
///   written as UTF-8 (bash 4.2 and later, and zsh; bash 3.2 leaves them as written, and reading
///   them decoded there costs nothing but a refusal);
/// - `\x`, `\u` and `\U` with no digit after them stay as written, and so does a trailing `\c`;
/// - a NUL ends the quotation's text: `$'gi\0x't` is `git`, because bash builds the word from C
///   strings.
///
/// **Where the two shells part, the reading that names something wins.** Claude Code runs a
/// command in the user's shell, and that is zsh on a Mac. An escape bash does not know (`\q`)
/// keeps its backslash in bash and loses it in zsh, and zsh does not know `\cX` (bash's control
/// character) at all: `$'\cat'` is `^At` to bash and `cat` to zsh. Bash's reading there is a
/// backslash or a control character, which names no program and no path, so the reader takes
/// zsh's, and a guard sees the `cat` zsh runs.
///
/// Bytes that are not UTF-8 (`\377`) come back as U+FFFD, since a word here is a `String`. No
/// name a guard matches is spelled with one.
pub fn ansi_c_decode(raw: &[char]) -> String {
    fn hex(c: char) -> Option<u32> {
        c.to_digit(16)
    }
    /// Up to `max` hex digits from `raw[*i..]`, or `None` when there is not even one.
    fn hex_run(raw: &[char], i: &mut usize, max: usize) -> Option<u32> {
        let mut value: Option<u32> = None;
        let mut taken = 0;
        while taken < max
            && let Some(d) = raw.get(*i).copied().and_then(hex)
        {
            value = Some(value.unwrap_or(0).wrapping_mul(16).wrapping_add(d));
            *i += 1;
            taken += 1;
        }
        value
    }
    fn push_char(out: &mut Vec<u8>, c: char) {
        let mut buf = [0u8; 4];
        out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
    }
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0usize;
    while i < raw.len() {
        let c = raw[i];
        i += 1;
        if c != '\\' || i >= raw.len() {
            push_char(&mut out, c);
            continue;
        }
        let e = raw[i];
        i += 1;
        let byte: Option<u8> = match e {
            'a' => Some(0x07),
            'b' => Some(0x08),
            'e' | 'E' => Some(0x1b),
            'f' => Some(0x0c),
            'n' => Some(b'\n'),
            'r' => Some(b'\r'),
            't' => Some(b'\t'),
            'v' => Some(0x0b),
            '\\' | '\'' | '"' | '?' => Some(e as u8),
            '0'..='7' => {
                let mut v = e as u32 - '0' as u32;
                let mut taken = 1;
                while taken < 3
                    && let Some(d) = raw.get(i).and_then(|c| c.to_digit(8))
                {
                    v = v * 8 + d;
                    i += 1;
                    taken += 1;
                }
                Some((v & 0xff) as u8)
            }
            'x' if raw.get(i) == Some(&'{') => {
                i += 1;
                let v = hex_run(raw, &mut i, usize::MAX).unwrap_or(0);
                if raw.get(i) == Some(&'}') {
                    i += 1;
                }
                Some((v & 0xff) as u8)
            }
            'x' => match hex_run(raw, &mut i, 2) {
                Some(v) => Some(v as u8),
                None => {
                    out.extend_from_slice(b"\\x");
                    continue;
                }
            },
            'u' | 'U' => {
                let max = if e == 'u' { 4 } else { 8 };
                match hex_run(raw, &mut i, max) {
                    Some(0) => Some(0),
                    Some(v) => {
                        push_char(&mut out, char::from_u32(v).unwrap_or('\u{fffd}'));
                        continue;
                    }
                    None => {
                        out.push(b'\\');
                        push_char(&mut out, e);
                        continue;
                    }
                }
            }
            // bash's `\cX`, read as zsh reads it — see above. A trailing `\c` is `\c` to bash
            // and `c` to zsh; the same rule takes zsh's.
            _ => {
                push_char(&mut out, e);
                continue;
            }
        };
        match byte {
            Some(0) => break,
            Some(b) => out.push(b),
            None => {}
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `s` as the words a POSIX shell makes of a string it is handed whole — [`posix_split`]'s
/// reading, plus the quoting forms [`lex`] reads the way the shell does (`$'…'`, `$"…"`, a
/// backslash-newline).
///
/// For a string that a SHELL runs rather than a program splitting it by its own rules: a git
/// alias that begins `!` is handed to `sh -c`, and `!$'\x67it' checkout` switches branches as
/// surely as `!git checkout` (checked with git 2.50 and macOS's `/bin/sh`). `posix_split` stays
/// the reading for `env -S` and a plain alias, whose programs do none of this.
pub fn shell_split(s: &str) -> Result<Vec<String>, LexError> {
    let mut lexer = Lexer::new(
        s,
        Cfg {
            bash_quoting: true,
            ..SPLIT_CFG
        },
    );
    let mut out = Vec::new();
    loop {
        match lexer.read_token()? {
            None => return Ok(out),
            Some(text) => out.push(text),
        }
    }
}

/// The characters `shlex.quote` leaves alone: `[\w@%+=:,./-]` under `re.ASCII`, so `\w` is
/// exactly ASCII alphanumerics and `_` — a Unicode letter is NOT safe and gets quoted.
fn quote_safe(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c)
}

/// CPython's `shlex.quote`: `s` as one shell word.
///
/// `_desugar_ansi_c` rewrites a decoded `$'…'` through this, which is what restores the token
/// boundary bash sees. The empty string becomes `''`, a string of only safe characters is
/// returned untouched, and anything else is single-quoted with each `'` spelled `'"'"'`.
pub fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    if s.chars().all(quote_safe) {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\"'\"'");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// `cmd` with backtick substitutions rewritten as `$( … )` — the same construct.
///
/// Alternating: the first unescaped backtick opens, the next closes. An odd count leaves the
/// last one open, which [`segment_tokens`] closes at end of input — the same place a shell would
/// report the error, and the segment is still SEEN.
///
/// A backtick inside single quotes is literal to a shell and is rewritten here anyway. The cost
/// is a token whose text reads `$(x)` instead of `` `x` `` inside an argument that is never
/// split, which is why this is safe to do before quoting is known.
pub fn unbacktick(cmd: &str) -> String {
    let chars: Vec<char> = cmd.chars().collect();
    // `_BACKTICK_RE` is `(?<!\\)` + a backtick, and `re.split` on it yields the pieces between.
    let mut parts: Vec<String> = Vec::new();
    let mut cur = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c == '`' && (i == 0 || chars[i - 1] != '\\') {
            parts.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    parts.push(cur);
    if parts.len() == 1 {
        return cmd.to_string();
    }
    let mut out = parts[0].clone();
    for (i, part) in parts[1..].iter().enumerate() {
        out.push_str(if i % 2 == 0 { "$(" } else { ")" });
        out.push_str(part);
    }
    out
}

/// `_OPERATOR_SPLIT_RE.split`: the pieces of `text`, separators included, longest-first,
/// redirections ahead of the control operator of the same spelling. Empty pieces are kept so a
/// caller tracking offsets can add their length; [`split_punctuation`] drops them.
fn operator_split(text: &[char]) -> Vec<Vec<char>> {
    /// The length of the alternative that matches at `text[i]`, in the alternation's order, or
    /// `None`. Leftmost-longest is NOT the rule — the regex takes the FIRST alternative that
    /// matches, and the table is ordered so that is also the longest.
    fn matches_at(text: &[char], i: usize) -> Option<usize> {
        for pat in OPERATOR_ALTERNATION {
            let len = pat.chars().count();
            if i + len <= text.len() && text[i..i + len].iter().copied().eq(pat.chars()) {
                return Some(len);
            }
        }
        GROUPING_SPLIT.contains(&text[i]).then_some(1)
    }

    let mut out = Vec::new();
    let mut last = 0usize;
    let mut i = 0usize;
    while i < text.len() {
        match matches_at(text, i) {
            Some(len) => {
                out.push(text[last..i].to_vec());
                out.push(text[i..i + len].to_vec());
                i += len;
                last = i;
            }
            None => i += 1,
        }
    }
    out.push(text[last..].to_vec());
    out
}

/// `toks` with the lexer's glued punctuation RUNS broken into the SHELL'S OWN tokens.
///
/// Not into individual operator *characters*: `>&` is one alternative of
/// [`OPERATOR_ALTERNATION`] and stands in front of the `>` that is its prefix, so the `&` stays
/// part of the redirection instead of becoming a boundary that strands `cat 2>&1 <vault>`'s
/// operand.
///
/// Only tokens made ENTIRELY of punctuation characters are touched, so a real argument is never
/// rewritten — and only **bare** runs. A quoted `'();'` is one word to a shell, and splitting it
/// into three boundaries is not a conservative error: it strands a reader's operand in a segment
/// of its own, which is a false ALLOW (`cat '()' <vault>` printed a vault). The pieces inherit
/// `bare`, since a run that was unquoted is unquoted character by character.
pub fn split_punctuation(toks: Vec<Tok>) -> Vec<Tok> {
    let mut out = Vec::new();
    for t in toks {
        let chars: Vec<char> = t.text.chars().collect();
        if t.bare && chars.len() > 1 && chars.iter().all(|c| PUNCTUATION_CHARS.contains(*c)) {
            // Each piece starts where it sits inside the run, so a split token's offset stays an
            // offset into the source. The advance is outside the `if` because Python's is, and
            // because the general shape is right; in a run it is also inert, since every
            // character of a run matches some alternative and the pieces BETWEEN the separators
            // are therefore all empty.
            let mut at = t.start;
            for p in operator_split(&chars) {
                if !p.is_empty() {
                    let text: String = p.iter().collect();
                    out.push(Tok::new(text, true, at, at + p.len() as isize));
                }
                at += p.len() as isize;
            }
        } else {
            out.push(t);
        }
    }
    out
}

/// Tokens as **argv per separately-executed command**, parentheses understood.
///
/// Shared by both of [`segment_argv_parsed`]'s paths, so the parsed and the unparseable answers
/// cannot disagree about what a boundary is.
///
/// Three kinds of token end a segment: the control operators, the braces of a group, and the
/// parenthesis of a SUBSHELL — each only where the shell would INTERPRET it, which
/// [`Tok::is_op`] answers and a quoted or escaped character never satisfies.
///
/// A parenthesis that opens a **substitution** — `$( … )`, `<( … )`, `>( … )` — is not a
/// boundary in the same sense, and treating it as one was a bypass rather than a gap:
/// `cat $(echo <vault>)` segments into `cat $` and `echo <vault>` under a plain boundary rule,
/// neither half of which is a read of the vault. So a substitution yields an **additional inner
/// segment** while the **enclosing segment keeps accumulating** the same tokens. Both readings
/// are needed: the inner one is what runs (`echo $(cat <vault>)`), and the outer one is where
/// its output lands (`cat $(echo <vault>)`).
///
/// **Being unquoted is not enough to make a token a boundary — POSITION decides too.** `{` and
/// `}` are RESERVED WORDS, recognised only where a command word is expected; everywhere else
/// bash passes them through as ordinary arguments, so `cat { <vault>` is ONE command that prints
/// the vault. A parenthesis is judged the same way — bash only closes a subshell that is open,
/// and `cat ( x` is a syntax error rather than a boundary. Keeping the segment whole is the
/// conservative direction: a reader holds on to its operand.
pub fn segment_tokens(toks: Vec<Tok>) -> Vec<Vec<String>> {
    joined_segments(toks).into_iter().map(|s| s.argv).collect()
}

/// One segment of [`segment_tokens`], with the operators on either side of it.
///
/// `before` is what ended the previous segment at the same nesting level (`None` at the start of
/// the line or of a substitution, `"("` just inside a subshell), and `after` is the token that
/// ended this one (`None` at the end of the line). A `)` that closes a subshell or a
/// substitution is `after` for the segment it closes. The plane-root guards read these to tell a
/// `cd` whose failure stops what follows (`cd x && …`) from one whose failure does not (#345).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinedSegment {
    pub argv: Vec<String>,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// [`segment_tokens`] with each segment's [`JoinedSegment::before`] and
/// [`JoinedSegment::after`]. The argv are exactly [`segment_tokens`]' — that function is this
/// one with the operators dropped.
pub fn joined_segments(toks: Vec<Tok>) -> Vec<JoinedSegment> {
    walk_segments(toks)
        .0
        .into_iter()
        .map(|w| w.joined)
        .collect()
}

std::thread_local! {
    /// Whether a walk on this thread has met a string nested past [`MAX_NESTING`] since the
    /// innermost [`too_deep_within`] began: [`walk_segments`] sets it, and only that reads it.
    static TOO_DEEP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Runs `read`, and says whether any walk inside it met a string whose substitutions nest past
/// [`MAX_NESTING`] (#1355).
///
/// Such a walk is not read whole ([`walk_segments`]), so whatever was decided on it is not a
/// decision: the caller refuses the call as too big to check, whatever `read` answered. Every
/// guard is run inside this ([`crate::toolgate::verdict`]), so a string a guard derives from a
/// command and reads in turn (heredocs stripped, a double-quoted body, a shell's `-c` script)
/// is held to the cap as the command itself is, with no derivation to keep in step. Nests: an
/// inner call reports its own walks and leaves the outer one's answer standing.
pub fn too_deep_within<T>(read: impl FnOnce() -> T) -> (T, bool) {
    let before = TOO_DEEP.with(|flag| flag.replace(false));
    let answer = read();
    let met = TOO_DEEP.with(|flag| {
        let met = flag.get();
        flag.set(before || met);
        met
    });
    (answer, met)
}

/// Reports to the innermost [`too_deep_within`] that a reader met a string nested past what it
/// reads, as [`walk_segments`] does: the leak guard's reading of substitutions, one inside
/// another, past [`MAX_NESTING`] (#1412).
pub fn met_a_string_too_deep() {
    TOO_DEEP.with(|flag| flag.set(true));
}

/// One segment as [`walk_segments`] closes it: its [`JoinedSegment`], whose argv is its
/// [`Reading::outward`] words, and its [`Reading::own`] words, both from the same walk.
struct Walked {
    joined: JoinedSegment,
    own: Vec<Tok>,
    /// The texts of its words, outward and own, that [`Tok::split_in_braces`].
    split_in_braces: Vec<String>,
}

/// What [`Reading::own`] writes where a substitution stands in the command around it, after
/// what opened it: `id=$` becomes `id=$(…)`. A word the shell fills in, to any reader.
pub const SUBSTITUTED: &str = "(…)";

/// **One segment read two ways** (#7092): [`Reading::outward`], its argv as [`segment_argv`]
/// reads it, with every word of a substitution inside it handed out to it, and [`Reading::own`],
/// the words its own command runs with, each substitution one word in its place.
///
/// `id=$(gh api "repos/o/r/actions/runs?x=$H" | head -1)` is three segments: `gh api …`,
/// `head -1`, and one whose outward argv is `id=$ gh api … head -1`, a `gh api` call with
/// `head`'s `-1` among its flags, and whose own is `id=$(…)`, which runs nothing. And
/// `A=$(true) gh pr merge 12`'s outward argv is a `true`, where its own is a `gh pr merge 12`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reading {
    pub outward: Vec<String>,
    pub own: Vec<String>,
    /// Each of [`Reading::own`]'s words as the source spells it, quotes and substitutions and
    /// all (`$X$(echo gh)/y`, #7131), with each backtick pair written as `$(…)`; an empty string
    /// where the line did not lex and nothing measured the word.
    pub own_source: Vec<String>,
    /// Whether each of [`Reading::own`]'s words stood unquoted and unescaped in the source (the
    /// lexer's `bare`), so a quoted `'<'` is a word and never an operator.
    pub own_bare: Vec<bool>,
    /// Whether each of [`Reading::own`]'s words begins where the word before it ends, with no
    /// blank between: the `2` of `2>f` is a descriptor, and the `2` of `2 >f` is a word. False
    /// for every word where the line did not lex.
    pub own_glued: Vec<bool>,
    /// Whether a `|` or `|&` hands this segment its stdin: the segment before it writes what this
    /// one reads, which may become words of a command it runs (`echo pr merge 5 | xe gh`).
    pub own_piped: bool,
    /// The texts of the segment's words, [`Reading::outward`]'s and [`Reading::own`]'s, in whose
    /// `${…}` an unquoted blank stood ([`Tok::split_in_braces`]): the shell splits what such a
    /// word gives into words, and as a program it is a whole command (#7709, round 3).
    pub split_in_braces: Vec<String>,
}

/// [`segment_argv`]'s segments, each with its [`Reading::own`] words as well, both readings
/// of a segment made by one walk, so each own reading is paired with its own segment's outward
/// one by the segment itself, never by position (#7092). A line that will not lex is read as
/// [`segment_argv`] reads it, a logical line at a time, by the same walk.
pub fn segment_readings(cmd: &str) -> Vec<Reading> {
    if cmd.len() < LARGE_READING {
        return read_segments(cmd);
    }
    // One hook reads a large body again and again: the floor, the merge reading, and the hold
    // each walk the same text, and the walk is the cost of it (#7245). A reading is a pure
    // function of the text, so the last few large ones are kept; the one effect it has, the
    // report of a reading nested too deep, is made again on a hit.
    if let Some((too_deep, readings)) = LARGE_READINGS.with(|kept| {
        kept.borrow()
            .iter()
            .find(|(text, ..)| text == cmd)
            .map(|(_, too_deep, readings)| (*too_deep, readings.clone()))
    }) {
        if too_deep {
            TOO_DEEP.with(|flag| flag.set(true));
        }
        return readings;
    }
    let before = TOO_DEEP.with(|flag| flag.replace(false));
    let readings = read_segments(cmd);
    let too_deep = TOO_DEEP.with(|flag| flag.replace(before || flag.get()));
    LARGE_READINGS.with(|kept| {
        let mut kept = kept.borrow_mut();
        if kept.len() >= LARGE_READINGS_KEPT {
            kept.remove(0);
        }
        kept.push((cmd.to_owned(), too_deep, readings.clone()));
    });
    readings
}

/// A text this long is read once however often it is asked for ([`segment_readings`]).
const LARGE_READING: usize = 4096;
/// How many large texts one thread keeps the reading of: the floor's two whole-text reads of a
/// body, with room for the merge reading's.
const LARGE_READINGS_KEPT: usize = 4;

std::thread_local! {
    /// The large texts [`segment_readings`] has read on this thread, each with whether its walk
    /// met a nesting past [`MAX_NESTING`].
    static LARGE_READINGS: std::cell::RefCell<Vec<(String, bool, Vec<Reading>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn read_segments(cmd: &str) -> Vec<Reading> {
    let cmd = unbacktick(cmd);
    let (walked, measured) = match lex(&cmd) {
        Ok(toks) => (walk_segments(toks).0, true),
        Err(_) => (fallback_walk(&cmd).0, false),
    };
    let chars: Vec<char> = cmd.chars().collect();
    let source = |t: &Tok| match (usize::try_from(t.start), usize::try_from(t.end)) {
        (Ok(start), Ok(end)) if measured && start <= end => chars
            .get(start..end)
            .map_or_else(String::new, |s| s.iter().collect()),
        _ => String::new(),
    };
    walked
        .into_iter()
        .map(|w| Reading {
            own_piped: matches!(w.joined.before.as_deref(), Some("|" | "|&")),
            split_in_braces: w.split_in_braces,
            outward: w.joined.argv,
            own_source: w.own.iter().map(source).collect(),
            own_bare: w.own.iter().map(|t| t.bare).collect(),
            own_glued: w
                .own
                .iter()
                .enumerate()
                .map(|(k, t)| measured && k > 0 && t.start >= 0 && w.own[k - 1].end == t.start)
                .collect(),
            own: w.own.into_iter().map(|t| t.text).collect(),
        })
        .collect()
}

/// [`joined_segments`], and how many substitutions were open at once at the deepest.
///
/// **Linear in the tokens, however deep they nest** (#1355). Each token goes to every segment
/// still open around it, which made a line `n` tokens long and `n` deep cost `n²`. So a token
/// goes to the [`MAX_NESTING`] innermost substitutions and the segment around them, and no
/// further out. **A walk that has to stop short like that fails closed**: it is reported to
/// [`too_deep_within`], and every guard's caller refuses the call as too big to check. An outer
/// segment that never received a token is not a reading anyone may decide on, so none does.
fn walk_segments(toks: Vec<Tok>) -> (Vec<Walked>, usize) {
    /// What is open at this point, innermost last.
    enum Open {
        Subst,
        Subshell,
        /// bash 5.3's `${ …; }` / `${| …; }`, which closes at a `}` in command position.
        Funsub,
    }

    /// A segment still open: its outward tokens, which every decision below reads, and its own
    /// words ([`Reading::own`]).
    #[derive(Default)]
    struct Seg {
        outward: Vec<Tok>,
        own: Vec<Tok>,
    }
    let texts = |toks: Vec<Tok>| toks.into_iter().map(|t| t.text).collect::<Vec<_>>();
    let split_in_braces = |seg: &Seg| {
        seg.outward
            .iter()
            .chain(&seg.own)
            .filter(|t| t.split_in_braces)
            .map(|t| t.text.clone())
            .collect::<Vec<_>>()
    };

    let mut out: Vec<Walked> = Vec::new();
    let mut open_segs: Vec<Seg> = vec![Seg::default()]; // outermost first
    // What ended the last segment at each open level, parallel to `open_segs`.
    let mut befores: Vec<Option<String>> = vec![None];
    let mut stack: Vec<Open> = Vec::new();
    let mut deepest = 0usize;
    // Where the last substitution closed: a word that starts there is, in the own reading, the
    // rest of the word the substitution stands in (`A=$(true)x`).
    let mut word_goes_on_at: Option<isize> = None;
    let mut close = |seg: Seg, before: &mut Option<String>, after: &str| {
        let split_in_braces = split_in_braces(&seg);
        out.push(Walked {
            joined: JoinedSegment {
                argv: texts(seg.outward),
                before: before.clone(),
                after: Some(after.to_string()),
            },
            split_in_braces,
            own: seg.own,
        });
        *before = Some(after.to_string());
    };
    for t in split_punctuation(toks) {
        let goes_on = word_goes_on_at
            .take()
            .is_some_and(|at| at >= 0 && at == t.start);
        if t.is_op(&["("]) {
            let prev = open_segs.last().and_then(|s| s.outward.last());
            let substitution = prev.is_some_and(|p| {
                p.bare
                    && (p.text.ends_with('$')
                        || PROCSUB_LEAD.contains(&p.text.as_str())
                        || opens_equals_substitution(p, &t))
            });
            if substitution {
                // In its own words, the segment around keeps one word where the substitution
                // stands, which the shell fills in: `id=$(…)`, `<(…)`.
                if let Some(opener) = open_segs.last_mut().and_then(|s| s.own.last_mut()) {
                    opener.text.push_str(SUBSTITUTED);
                }
                open_segs.push(Seg::default());
                befores.push(None);
                stack.push(Open::Subst);
                deepest = deepest.max(open_segs.len() - 1);
                continue;
            }
            if open_segs.last().is_some_and(|s| s.outward.is_empty()) {
                // command position: a subshell opens
                stack.push(Open::Subshell);
                if let Some(b) = befores.last_mut() {
                    *b = Some("(".to_string());
                }
                continue;
            }
            // `cat ( x` — a shell does not start a subshell mid-command; it fails to parse.
            // Treat it as the word it is rather than stranding `cat`'s operand.
        } else if t.is_op(&[")"]) {
            match stack.last() {
                Some(Open::Subst) => {
                    let seg = open_segs.pop().unwrap_or_default();
                    let mut before = befores.pop().flatten();
                    close(seg, &mut before, ")");
                    stack.pop();
                    // The word the substitution stands in runs in the source to its `)`.
                    if let Some(word) = open_segs.last_mut().and_then(|s| s.own.last_mut()) {
                        word.end = t.end;
                    }
                    word_goes_on_at = Some(t.end);
                    continue;
                }
                Some(Open::Subshell) => {
                    if let (Some(seg), Some(before)) = (open_segs.last_mut(), befores.last_mut()) {
                        close(std::mem::take(seg), before, ")");
                    }
                    stack.pop();
                    continue;
                }
                // nothing is open for it to close: an ordinary word, as above.
                Some(Open::Funsub) | None => {}
            }
        } else if t.is_op(&["{", "}"]) {
            if open_segs.last().is_some_and(|s| s.outward.is_empty()) {
                if t.text == "}" && matches!(stack.last(), Some(Open::Funsub)) {
                    let seg = open_segs.pop().unwrap_or_default();
                    let mut before = befores.pop().flatten();
                    close(seg, &mut before, "}");
                    stack.pop();
                }
                continue; // command position: the reserved word
            }
            // mid-command: an ordinary argument to the program already named.
        } else if t.is_any_op() {
            if let (Some(seg), Some(before)) = (open_segs.last_mut(), befores.last_mut()) {
                close(std::mem::take(seg), before, &t.text);
            }
            continue;
        }
        // bash 5.3 runs `${ cmd; }` as a substitution: the lexer ends the `${` word at the blank
        // (`${VAR}` stays one word and never gets here), and what follows is read as `$( … )`'s
        // contents are. An older bash refuses the line as a bad substitution, so reading it as
        // one only ever shows the guards more.
        let funsub = t.bare && t.text.ends_with("${");
        // outward, to every open segment, the outer ones included, as far out as the cap reaches
        let from = open_segs.len().saturating_sub(MAX_NESTING + 1);
        for seg in open_segs[from..].iter_mut() {
            seg.outward.push(t.clone());
        }
        // in its own words, to the innermost alone, where a word right after a substitution's
        // `)` goes on with the word the substitution stands in. A redirection or any other
        // punctuation the shell reads there is never part of a word, and a quoted piece leaves
        // the word quoted.
        let punctuation = t.bare && t.text.chars().any(|c| PUNCTUATION_CHARS.contains(c));
        let glued = goes_on && !punctuation;
        match open_segs.last_mut() {
            Some(seg) if glued && !seg.own.is_empty() => {
                if let Some(word) = seg.own.last_mut() {
                    word.text.push_str(&t.text);
                    word.bare = word.bare && t.bare;
                    word.end = t.end;
                }
            }
            Some(seg) => seg.own.push(t),
            None => {}
        }
        if funsub {
            open_segs.push(Seg::default());
            befores.push(None);
            stack.push(Open::Funsub);
        }
        deepest = deepest.max(open_segs.len() - 1);
    }
    for (seg, before) in open_segs.into_iter().zip(befores) {
        let split_in_braces = split_in_braces(&seg);
        out.push(Walked {
            joined: JoinedSegment {
                argv: texts(seg.outward),
                before,
                after: None,
            },
            split_in_braces,
            own: seg.own,
        });
    }
    if deepest > MAX_NESTING {
        TOO_DEEP.with(|flag| flag.set(true));
    }
    // A segment's own words are among its outward ones, so one with no outward words has none.
    let out = out
        .into_iter()
        .filter(|w| !w.joined.argv.is_empty())
        .collect();
    (out, deepest)
}

/// Split an already-whitespace-split token list on shell operators — the unparseable path only,
/// where there is no tokenizer to lean on.
///
/// Operators are split out of the MIDDLE of a token as well (`a;b` is two commands to a shell,
/// and one token to `str.split`), because a fallback that only noticed free-standing operators
/// would be a rule an attacker satisfies by deleting a space.
///
/// Every piece is marked bare, because on this path nothing knows what was quoted — the quoting
/// is what failed to parse. That is a guess in BOTH directions, which is why
/// [`segment_argv_parsed`] reports the failure and the leak guard scans the raw string as well.
fn resegment(toks: &[String]) -> Vec<Tok> {
    let mut pieces: Vec<Tok> = Vec::new();
    for tok in toks {
        let chars: Vec<char> = tok.chars().collect();
        for p in operator_split(&chars) {
            if !p.is_empty() {
                pieces.push(Tok::new(p.iter().collect::<String>(), true, -1, -1));
            }
        }
    }
    pieces
}

/// Index just past `pat` matched at `i` the way the shell reads it: with any number of
/// backslash-newlines between its characters, because the shell removes each such pair before
/// it reads anything — `$\<newline>(` is `$(`. `None` when `pat` does not start at `i`.
///
/// Only where a backslash is live: unquoted, inside `"…"` and in an expanding heredoc body.
/// Inside `'…'` and `$'…'` the pair stays, and neither [`quote_map`] nor
/// [`crate::livesub::live_substitution`] looks for a substitution there.
pub fn spliced_end(chars: &[char], i: usize, pat: &str) -> Option<usize> {
    let mut j = i;
    for (k, want) in pat.chars().enumerate() {
        if k > 0 {
            j = past_continuations(chars, j);
        }
        if chars.get(j) != Some(&want) {
            return None;
        }
        j += 1;
    }
    Some(j)
}

/// `j` moved past every backslash-newline that starts there.
pub fn past_continuations(chars: &[char], mut j: usize) -> usize {
    while chars.get(j) == Some(&'\\') && chars.get(j + 1) == Some(&'\n') {
        j += 2;
    }
    j
}

/// Whether the `$` at `i` opens a `$'…'` or `$"…"` quotation rather than being the tail of a
/// `$$` PID expansion. The shell pairs `$$` into the PID before it reads a quote, so the `$`
/// opens the quotation only when an EVEN number of `$` stand immediately before it: `$$'x'` is
/// the PID and a plain `'x'`, `$$$'x'` is the PID and an ANSI-C `$'x'` (checked against bash).
/// This is the parity [`Lexer`] tracks with its `dollars` counter, on the source-walking path.
pub fn dollar_opens_quote(chars: &[char], i: usize) -> bool {
    let mut before = 0;
    while before < i && chars[i - 1 - before] == '$' {
        before += 1;
    }
    before % 2 == 0
}

/// For each offset in `line`, whether it sits inside quotes — computed in ONE pass.
///
/// Read off the source rather than from the lexer, because this runs on lines the lexer could
/// not take apart. A backslash escapes the next character outside single quotes, where bash
/// takes it literally.
///
/// **A substitution inside double quotes is not quoted.** `"$(cat <<'EOF')"` runs a command and
/// that command's `<<` is a real opener, so `$(` and a backtick open a nested context.
///
/// **But `$'` opens nothing inside `"…"`**, where both shells read a bare `$` as a literal. The
/// `"` arm therefore comes BEFORE the `$'` arm: with the order reversed, the `$` in an
/// ordinary regex anchor (`grep -v "^$" f`) swallows the closing quote and the rest of the line
/// reads as quoted.
///
/// **`$"…"` is a double-quoted run, not a single-quoted one.** A `$(` and a backtick inside it
/// are LIVE — `$"$(cmd)"` runs `cmd` (checked against bash) — and a heredoc opener inside its
/// substitution is a real opener, so `$"` opens the same context `"` does rather than one that
/// swallows them. Only the `$'`/`$"` that a `$$` did not consume opens a quotation
/// ([`dollar_opens_quote`]).
///
/// The returned vector has one more entry than `line` has characters: the last is the state at
/// end of input.
pub fn quote_map(line: &str) -> Vec<bool> {
    scan(line).0
}

/// Where a backtick stands, as the shell reads it (#7423).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backtick {
    /// Opens or closes a command substitution.
    Live,
    /// Inside `'…'`: an ordinary character.
    Single,
    /// Inside `$'…'`: an ordinary character, spelled `\x60` there.
    Ansi,
}

/// The backticks of `line`, each with its character offset and what it is to the shell: the
/// quoting of [`quote_map`], so a quoted backtick opens nothing and `\\` before one is an escaped
/// backslash. A backtick the shell escapes (`\``) is none of them and is not listed.
pub fn backticks(line: &str) -> Vec<(usize, Backtick)> {
    scan(line).1
}

/// [`quote_map`]'s one pass, and the backticks it walks past.
fn scan(line: &str) -> (Vec<bool>, Vec<(usize, Backtick)>) {
    let mut ticks: Vec<(usize, Backtick)> = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut flags = vec![false; n + 1];
    let mut stack: Vec<&'static str> = Vec::new();
    fn quoted_now(stack: &[&'static str]) -> bool {
        stack.last().is_some_and(|t| QUOTED_CONTEXTS.contains(t))
    }
    let mut i = 0usize;
    while i < n {
        let here = quoted_now(&stack);
        flags[i] = here;
        let c = chars[i];
        let top = stack.last().copied().unwrap_or("");
        if top != "'" && c == '\\' {
            if i + 1 < n {
                flags[i + 1] = here;
            }
            i += 2;
            continue;
        }
        if top == "'" {
            if c == '`' {
                ticks.push((i, Backtick::Single));
            }
            if c == '\'' {
                stack.pop();
            }
        } else if top == "$'" {
            if c == '`' {
                ticks.push((i, Backtick::Ansi));
            }
            // Closes on its own quote, and nothing else opens inside it: `$(` is literal there.
            if c == '\'' {
                stack.pop();
            }
        } else if let Some(end) = spliced_end(&chars, i, "$(") {
            // `$\<newline>(` too: the shell removes the pair first, in `"…"` as outside.
            stack.push("(");
            flags[i + 1..end].fill(here);
            i = end;
            continue;
        } else if c == '`' {
            ticks.push((i, Backtick::Live));
            if top == "`" {
                stack.pop();
            } else {
                stack.push("`");
            }
        } else if top == "\"" {
            // Ahead of the `$'` arm on purpose — see above. Inside `"…"` only the closing quote
            // (and a substitution, handled above) means anything.
            if c == '"' {
                stack.pop();
            }
        } else if let Some(end) =
            spliced_end(&chars, i, "$'").filter(|_| dollar_opens_quote(&chars, i))
        {
            stack.push("$'");
            flags[i + 1..end].fill(here);
            i = end;
            continue;
        } else if let Some(end) =
            spliced_end(&chars, i, "$\"").filter(|_| dollar_opens_quote(&chars, i))
        {
            // A double-quoted run whose `$` the shell drops: the same context `"` opens, so a
            // `$(`/backtick inside is live and a heredoc opener in it is real.
            stack.push("\"");
            flags[i + 1..end].fill(here);
            i = end;
            continue;
        } else if c == ')' && top == "(" {
            stack.pop();
        } else if c == '\'' || c == '"' {
            stack.push(if c == '\'' { "'" } else { "\"" });
        }
        i += 1;
    }
    flags[n] = quoted_now(&stack);
    (flags, ticks)
}

/// `cmd` with each ACTIVE backslash-newline removed — bash's first transformation, before tokens
/// or quotes are read.
///
/// A backslash-newline splices only where the backslash is live: it is literal inside single
/// quotes and inside a comment, so those newlines stay. Kept narrow — only an UNQUOTED backslash
/// is spliced; one inside double quotes is left in place, which can only keep more text on one
/// line (the fail-closed direction for a guard reading operands).
pub fn splice_continuations(cmd: &str) -> String {
    let chars: Vec<char> = cmd.chars().collect();
    let q = quote_map(cmd);
    let n = chars.len();
    let mut out = String::new();
    let mut in_comment = false;
    let mut i = 0usize;
    while i < n {
        let c = chars[i];
        if c == '\n' {
            in_comment = false;
            out.push(c);
            i += 1;
            continue;
        }
        if !in_comment
            && c == '#'
            && !q[i]
            && (i == 0 || (BEFORE_COMMENT.contains(chars[i - 1]) && !q[i - 1]))
        {
            in_comment = true;
        }
        if c == '\\' && i + 1 < n && chars[i + 1] == '\n' && !q[i] && !in_comment {
            i += 2; // drop the backslash and the newline it splices
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Segment an unparseable `cmd` keeping bash's LINE structure.
///
/// The old fallback was `_resegment(cmd.split())`, and `str.split` erases newlines. On the
/// parsed path a newline IS a command boundary, so a single broken quote anywhere folded every
/// following line into the segment before it — `cd .charter/vaults`, then `cat x.json`, then a
/// stray `echo "` read as ONE command whose program was `cd`, and the vault read vanished. It
/// cuts both ways: splitting on EVERY newline strands a reader's operand when a quoted string
/// spans lines, and not splicing is not an option either.
///
/// So a newline is a boundary exactly where bash makes one: not inside a quote it has not closed
/// ([`quote_map`], which reads unbalanced input without failing), and not one a live backslash
/// spliced away ([`splice_continuations`], run first as bash runs it). Each resulting logical
/// line is then lexed on its OWN, and only a line that still will not lex falls back to the
/// crude [`resegment`], whose stranding is confined to that one line.
fn fallback_segments(cmd: &str) -> (Vec<Vec<String>>, usize) {
    let (walked, deepest) = fallback_walk(cmd);
    (walked.into_iter().map(|w| w.joined.argv).collect(), deepest)
}

/// [`fallback_segments`], each segment as [`walk_segments`] closed it, own words and all.
fn fallback_walk(cmd: &str) -> (Vec<Walked>, usize) {
    let spliced = splice_continuations(cmd);
    let chars: Vec<char> = spliced.chars().collect();
    let q = quote_map(&spliced);
    let mut out: Vec<Walked> = Vec::new();
    let mut deepest = 0usize;
    let mut bounds: Vec<usize> = (0..chars.len())
        .filter(|&i| chars[i] == '\n' && !q[i])
        .collect();
    bounds.push(chars.len());
    let mut start = 0usize;
    for cut in bounds {
        // `start` can pass `cut` only for an empty trailing slice; Python's slice clamps.
        let line: String = if start <= cut {
            chars[start..cut].iter().collect()
        } else {
            String::new()
        };
        start = cut + 1;
        // Python's `str.strip()`, which counts U+001C–U+001F as blank where Rust's does not.
        if line.chars().all(is_python_space) {
            continue;
        }
        let (segments, depth) = match lex(&line) {
            Ok(toks) => walk_segments(toks),
            Err(_) => walk_segments(resegment(&py_split(&line))),
        };
        out.extend(segments);
        deepest = deepest.max(depth);
    }
    (out, deepest)
}

/// Python's `str.split()` with no argument: split on RUNS of Python whitespace, no empty pieces.
pub fn py_split(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in text.chars() {
        if is_python_space(c) {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// A shell command as **argv per separately-executed segment**, quoting respected, plus whether
/// the command actually PARSED.
///
/// A caller that must fail closed needs to know that the answer it just got is best-effort. The
/// leak guard is the one caller that does: it adds a raw scan of the whole string when this
/// returns `false`.
///
/// **On a command that cannot be parsed at all** (genuinely unbalanced quoting) this falls back
/// to a per-line lex, still segmented on the operators. It used to return the whole string as
/// ONE segment, and the Python docstring claimed that kept the leak guard fail-closed. It did
/// the opposite: every guard reads token 0 as the program, so one segment means one program, and
/// every invocation AFTER the first became invisible — `echo $'it\'s fine' ; cat <vault>` is
/// valid bash, trips `shlex`, and printed a vault through the shipped hook. A fallback that
/// drops guards is worse than no fallback, because the guard still looks present.
pub fn segment_argv_parsed(cmd: &str) -> (Vec<Vec<String>>, bool) {
    let (segments, parsed, _) = read(cmd);
    (segments, parsed)
}

/// [`segment_argv_parsed`], and how deeply the substitutions nested ([`substitution_depth`]).
fn read(cmd: &str) -> (Vec<Vec<String>>, bool, usize) {
    let cmd = unbacktick(cmd);
    match lex(&cmd) {
        // A walk too deep to read whole is not a parse either: the caller that falls back to
        // the raw string on `false` does so here too (#1355).
        Ok(toks) => {
            let (segments, deepest) = walk_segments(toks);
            let argv = segments.into_iter().map(|w| w.joined.argv).collect();
            (argv, deepest <= MAX_NESTING, deepest)
        }
        // Tokenized, and segmented: the leak guard has to see `--reveal` among the arguments AND
        // has to see the second command at all. Quoting is not honoured on this path — it is
        // what failed to parse — so the boundaries here are a guess, and a guess is wrong in
        // both directions. Neither is survivable on its own, so this path is not relied on
        // alone: the flag is `false`, and the caller matches the raw string as well.
        Err(_) => {
            let (segments, deepest) = fallback_segments(&cmd);
            (segments, false, deepest)
        }
    }
}

/// [`segment_argv_parsed`]'s segments with the operators around each ([`joined_segments`]), or
/// `None` for a command that would not tokenize — the caller that needs the operators is one
/// that fails open on a guess, as the plane-root guards do.
pub fn joined_argv(cmd: &str) -> Option<Vec<JoinedSegment>> {
    lex(&unbacktick(cmd)).ok().map(joined_segments)
}

/// How deeply the substitutions of `cmd` nest at their deepest, as [`segment_argv`] reads them:
/// the most `$( … )`, `<( … )`, `=( … )` and `${ …; }` open at once.
///
/// [`joined_segments`] hands every token to each segment still open around it, as far out as
/// [`MAX_NESTING`], so reading a line costs its length times this at most; [`crate::guardcaps`]
/// refuses a command whose depth is past that before any guard reads it (#1355). Read by the
/// very walk that segments, so the two cannot disagree.
pub fn substitution_depth(cmd: &str) -> usize {
    read(cmd).2
}

/// [`segment_argv`] and [`substitution_depth`] from one reading: what [`crate::guardcaps`]
/// measures a command by.
pub fn segments_and_depth(cmd: &str) -> (Vec<Vec<String>>, usize) {
    let (segments, _, depth) = read(cmd);
    (segments, depth)
}

/// [`segment_argv_parsed`] without the flag.
pub fn segment_argv(cmd: &str) -> Vec<Vec<String>> {
    segment_argv_parsed(cmd).0
}
