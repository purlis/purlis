// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The programs a shell script calls**, read the way the shell reads it (#5742): a quoted string
//! and a here-document's body are data, never a command, unless the shell runs them. What a
//! `$(…)` or a `` `…` `` runs counts wherever it is written, inside double quotes and an unquoted
//! here-document too; and what `sh -c '…'`, `eval "…"` or `bash <<EOF` hands a shell is read as
//! a script of its own.
//!
//! This is a lint's reading, not a shell: it never expands anything, and a script it cannot
//! follow costs a warning, never a wrong run.

/// What the shell runs no program for: its keywords and its builtins.
const SHELL_OWN: &[&str] = &[
    "if",
    "then",
    "else",
    "elif",
    "fi",
    "for",
    "while",
    "until",
    "do",
    "done",
    "case",
    "esac",
    "in",
    "function",
    "select",
    "time",
    "!",
    "{",
    "}",
    "[[",
    "]]",
    "[",
    "]",
    "test",
    "echo",
    "printf",
    "cd",
    "export",
    "local",
    "return",
    "exit",
    "set",
    "unset",
    "shift",
    "read",
    "source",
    ".",
    "eval",
    "exec",
    "trap",
    "wait",
    "true",
    "false",
    "break",
    "continue",
    "declare",
    "typeset",
    "readonly",
    "let",
    "alias",
    "unalias",
    "type",
    "command",
    "builtin",
    "hash",
    "pwd",
    "pushd",
    "popd",
    "getopts",
    "umask",
    "ulimit",
    "jobs",
    "fg",
    "bg",
    "kill",
    "shopt",
    "mapfile",
    "readarray",
    "disown",
    "caller",
    "complete",
    "enable",
    "help",
    "logout",
    "times",
    "dirs",
    "suspend",
    "fc",
    "history",
    ":",
    "noglob",
    "nohup",
    "sudo",
];

/// The shell's words a command may start with before the program it runs: `if grep …`,
/// `then jq …`, `sudo make …`.
const LEADING: &[&str] = &[
    "if", "then", "else", "elif", "while", "until", "do", "!", "time", "exec", "sudo", "nohup",
    "noglob", "builtin",
];

/// The shells whose `-c` argument, or whose standard input, is a script.
const SHELLS: &[&str] = &["sh", "bash", "zsh", "dash", "ksh"];

/// What a substitution the reader has already read leaves in its word: data to a second reading
/// (`eval "$(…)"`), never a program.
const SUBSTITUTED: &str = "${_}";

/// The programs `script` calls, in the order it names them: the first word of each command a
/// pipe, a list, a subshell or a substitution starts, past what sets a variable for it and the
/// shell's leading words. A variable, an option, a redirection, a quoted word with a space in it,
/// the skill's own file (`./`, a `$` path) and the shell's own words are no program.
pub(super) fn programs(script: &str) -> Vec<String> {
    let mut reader = Reader::new(script);
    reader.list(Close::End);
    reader.found
}

/// Whether `program` is a shell, by name or path.
pub(super) fn is_shell(program: &str) -> bool {
    let name = program.rsplit('/').next().unwrap_or(program);
    SHELLS.contains(&name)
}

/// One word of a command: its text with the quoting taken off, and whether any of it was quoted.
#[derive(Default)]
struct Word {
    text: String,
    quoted: bool,
}

/// A here-document a command line opened, whose body starts on the next line.
struct Heredoc {
    end: String,
    /// `<<-`: the closing line may be indented with tabs.
    tabs: bool,
    /// `<<'EOF'`: nothing in the body is expanded.
    quoted: bool,
    /// `bash <<EOF`, `cat <<EOF | sh`: a shell reads the body as a script.
    script: bool,
}

/// The command being read: its words so far, and where its program goes among those found, so a
/// substitution inside it is listed after it.
struct Command {
    words: Vec<Word>,
    slot: usize,
}

impl Command {
    /// Where its program's name is: past what sets a variable and the shell's leading words.
    fn head(&self) -> Option<usize> {
        self.words
            .iter()
            .position(|w| !(assigns(w) || (!w.quoted && LEADING.contains(&w.text.as_str()))))
    }

    /// Whether it is inside a `[[ … ]]`, where `(`, `|`, `&`, `<` and `>` are the test's.
    fn testing(&self) -> bool {
        self.head()
            .is_some_and(|at| self.words[at].text == "[[" && !self.words[at].quoted)
            && !self.words.iter().any(|w| w.text == "]]")
    }
}

/// What ends a list of commands.
#[derive(Clone, Copy, PartialEq)]
enum Close {
    /// The end of the script.
    End,
    /// The `)` of a `$(`.
    Paren,
    /// A closing `` ` ``.
    Backtick,
}

/// Text the shell expands but does not split into commands.
#[derive(Clone, Copy, PartialEq)]
enum Expanding {
    /// A double-quoted string, to its `"`.
    DoubleQuoted,
    /// An array's words, `name=(…)`, to its `)`.
    Array,
    /// An unquoted here-document's body, to its end.
    Body,
}

struct Reader {
    text: Vec<char>,
    at: usize,
    found: Vec<String>,
    heredocs: Vec<Heredoc>,
    /// How many `case … in` are open.
    cases: usize,
}

impl Reader {
    fn new(text: &str) -> Self {
        Reader {
            text: text.chars().collect(),
            at: 0,
            found: Vec::new(),
            heredocs: Vec::new(),
            cases: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.text.get(self.at).copied()
    }

    fn peek_at(&self, ahead: usize) -> Option<char> {
        self.text.get(self.at + ahead).copied()
    }

    fn starts(&self, with: &str) -> bool {
        with.chars()
            .enumerate()
            .all(|(i, c)| self.peek_at(i) == Some(c))
    }

    /// Reads a list of commands up to `close` and past it.
    fn list(&mut self, close: Close) {
        let mut command = Command {
            words: Vec::new(),
            slot: self.found.len(),
        };
        let mut word: Option<Word> = None;
        let mut depth = 0usize;
        let mut pattern_next = false;
        while let Some(c) = self.peek() {
            if pattern_next && word.is_none() && !c.is_whitespace() {
                pattern_next = self.case_pattern();
                continue;
            }
            match c {
                ')' if close == Close::Paren && depth == 0 => {
                    self.at += 1;
                    break;
                }
                '`' if close == Close::Backtick => {
                    self.at += 1;
                    break;
                }
                '\\' => {
                    self.at += 1;
                    match self.peek() {
                        Some('\n') => self.at += 1,
                        Some(next) => {
                            self.at += 1;
                            let w = word.get_or_insert_default();
                            w.text.push(next);
                            w.quoted = true;
                        }
                        None => {}
                    }
                }
                '\'' => {
                    self.at += 1;
                    let w = word.get_or_insert_default();
                    self.single_quoted(&mut w.text, false);
                    w.quoted = true;
                }
                '"' => {
                    self.at += 1;
                    let w = word.get_or_insert_default();
                    self.expanding(&mut w.text, Expanding::DoubleQuoted);
                    w.quoted = true;
                }
                '$' => {
                    let w = word.get_or_insert_default();
                    if self.starts("$'") {
                        self.at += 2;
                        self.single_quoted(&mut w.text, true);
                        w.quoted = true;
                    } else {
                        self.dollar(&mut w.text);
                    }
                }
                '`' => self.backticks(&mut word.get_or_insert_default().text),
                '#' if word.is_none() => self.comment(),
                '(' if word.as_ref().is_some_and(|w| w.text.ends_with('=')) => {
                    // An array, `name=(a b c)`: its words are data.
                    self.at += 1;
                    let w = word.as_mut().expect("the word is there");
                    self.expanding(&mut w.text, Expanding::Array);
                }
                '(' if word.is_none() && self.starts("((") => {
                    // Arithmetic, `(( n++ ))`, runs nothing.
                    self.at += 2;
                    self.arithmetic(&mut String::new());
                }
                '<' if self.starts("<<<") => {
                    // A here-string: the word after it is data, as any argument is.
                    self.at += 3;
                    word.get_or_insert_default().text.push_str("<<<");
                }
                '<' if self.starts("<<") => {
                    self.at += 2;
                    self.end_word(&mut command, &mut word, &mut pattern_next);
                    self.open_heredoc();
                }
                '&' if word.as_ref().is_some_and(|w| w.text.ends_with(['<', '>'])) => {
                    // `2>&1`, `<&3`: a redirection, not a list.
                    self.at += 1;
                    word.get_or_insert_default().text.push('&');
                }
                '|' | '&' | '(' | ')' | '<' | '>' if command.testing() => {
                    self.at += 1;
                    word.get_or_insert_default().text.push(c);
                }
                ';' | '|' | '&' | '(' | ')' | '\n' => {
                    self.at += 1;
                    match c {
                        '(' => depth += 1,
                        ')' => depth = depth.saturating_sub(1),
                        ';' if self.cases > 0 && matches!(self.peek(), Some(';' | '&')) => {
                            // `;;`, `;&`, `;;&`: the next arm's pattern follows.
                            while matches!(self.peek(), Some(';' | '&')) {
                                self.at += 1;
                            }
                            pattern_next = true;
                        }
                        _ => {}
                    }
                    self.end_word(&mut command, &mut word, &mut pattern_next);
                    self.end_command(&mut command);
                    if c == '\n' {
                        self.heredoc_bodies();
                    }
                }
                ' ' | '\t' | '\r' => {
                    self.at += 1;
                    self.end_word(&mut command, &mut word, &mut pattern_next);
                }
                _ => {
                    self.at += 1;
                    word.get_or_insert_default().text.push(c);
                }
            }
        }
        self.end_word(&mut command, &mut word, &mut pattern_next);
        self.end_command(&mut command);
    }

    /// A word is done. `case … in` opens a `case`, whose first arm's pattern follows; `esac`
    /// closes one.
    fn end_word(
        &mut self,
        command: &mut Command,
        word: &mut Option<Word>,
        pattern_next: &mut bool,
    ) {
        let Some(word) = word.take() else {
            return;
        };
        if command.words.is_empty() {
            command.slot = self.found.len();
        }
        let first = command.words.first().map(|w| w.text.as_str());
        if first == Some("case") && word.text == "in" && !word.quoted {
            command.words.push(word);
            self.cases += 1;
            *pattern_next = true;
            self.end_command(command);
            return;
        }
        if command.words.is_empty() && word.text == "esac" && !word.quoted {
            self.cases = self.cases.saturating_sub(1);
        }
        command.words.push(word);
    }

    /// A command is done: its program is listed in its place, and what it hands a shell is read:
    /// `eval`'s words, a shell's `-c` argument wherever the shell is called (`find -exec sh -c`,
    /// `xargs bash -c`), and the here-documents open on its line when a shell reads its standard
    /// input.
    fn end_command(&mut self, command: &mut Command) {
        let slot = command.slot.min(self.found.len());
        let head = command.head();
        let words = std::mem::take(&mut command.words);
        command.slot = self.found.len();
        let Some(at) = head else {
            return;
        };
        if let Some(program) = program(&words[at]) {
            self.found.insert(slot, program);
        }
        if words[at].text == "eval" && !words[at].quoted {
            let rest: Vec<&str> = words[at + 1..].iter().map(|w| w.text.as_str()).collect();
            self.found.extend(programs(&rest.join(" ")));
            return;
        }
        let Some(shell) = words[at..]
            .iter()
            .position(|w| !w.quoted && is_shell(&w.text))
            .map(|n| at + n)
        else {
            return;
        };
        let args = &words[shell + 1..];
        let option = |w: &Word| !w.quoted && w.text.starts_with('-');
        let flag = |w: &Word, letter: char| {
            option(w) && !w.text.starts_with("--") && w.text.contains(letter)
        };
        if let Some(c) = args.iter().position(|w| flag(w, 'c')) {
            if let Some(script) = args.get(c + 1) {
                self.found.extend(programs(&script.text));
            }
        } else if shell == at && (args.iter().any(|w| flag(w, 's')) || args.iter().all(option)) {
            for heredoc in &mut self.heredocs {
                heredoc.script = true;
            }
        }
    }

    /// The pattern of a `case` arm, up to its `)`, which runs nothing; a comment; or `esac`.
    /// Whether a pattern is still to come.
    fn case_pattern(&mut self) -> bool {
        if self.peek() == Some('#') {
            self.comment();
            return true;
        }
        if self.starts("esac")
            && self
                .peek_at(4)
                .is_none_or(|c| c.is_whitespace() || ";&|)".contains(c))
        {
            return false;
        }
        if self.peek() == Some('(') {
            self.at += 1;
        }
        while let Some(c) = self.peek() {
            self.at += 1;
            match c {
                ')' => return false,
                '\'' => self.single_quoted(&mut String::new(), false),
                '"' => self.expanding(&mut String::new(), Expanding::DoubleQuoted),
                '\\' => self.at += 1,
                _ => {}
            }
        }
        false
    }

    /// A comment, up to the end of its line.
    fn comment(&mut self) {
        while self.peek().is_some_and(|c| c != '\n') {
            self.at += 1;
        }
    }

    /// `` `…` ``, from its opening backtick: its commands count.
    fn backticks(&mut self, into: &mut String) {
        self.at += 1;
        self.list(Close::Backtick);
        into.push_str(SUBSTITUTED);
    }

    /// `'…'`, or with `escapes` `$'…'`, read past its closing quote into `into`: data.
    fn single_quoted(&mut self, into: &mut String, escapes: bool) {
        while let Some(c) = self.peek() {
            self.at += 1;
            match c {
                '\'' => return,
                '\\' if escapes => {
                    if let Some(next) = self.peek() {
                        self.at += 1;
                        into.push(next);
                    }
                }
                _ => into.push(c),
            }
        }
    }

    /// Text the shell expands but does not split into commands, read past its end into `into`.
    /// Only what a `$(…)` or a `` `…` `` in it runs counts.
    fn expanding(&mut self, into: &mut String, what: Expanding) {
        let close = match what {
            Expanding::DoubleQuoted => Some('"'),
            Expanding::Array => Some(')'),
            Expanding::Body => None,
        };
        while let Some(c) = self.peek() {
            if Some(c) == close {
                self.at += 1;
                return;
            }
            match c {
                '\\' => {
                    self.at += 1;
                    if let Some(next) = self.peek() {
                        self.at += 1;
                        into.push(next);
                    }
                }
                '$' => self.dollar(into),
                '`' => self.backticks(into),
                // An array's words may be quoted; a double-quoted string's and a body's quotes
                // are their text.
                '\'' if what == Expanding::Array => {
                    self.at += 1;
                    self.single_quoted(into, false);
                }
                '"' if what == Expanding::Array => {
                    self.at += 1;
                    self.expanding(into, Expanding::DoubleQuoted);
                }
                _ => {
                    self.at += 1;
                    into.push(c);
                }
            }
        }
    }

    /// What starts with `$`, read into `into`: a `$(…)` whose commands count, `$((…))` and
    /// `${…}`, which run nothing but what a `$(…)` in them runs, or a lone `$`.
    fn dollar(&mut self, into: &mut String) {
        if self.starts("$((") {
            self.at += 3;
            self.arithmetic(&mut String::new());
            into.push_str(SUBSTITUTED);
        } else if self.starts("$(") {
            self.at += 2;
            self.list(Close::Paren);
            into.push_str(SUBSTITUTED);
        } else if self.starts("${") {
            self.at += 2;
            // A default, `${NAME:-$(hostname)}`, may run a program.
            let mut depth = 1usize;
            while let Some(c) = self.peek() {
                match c {
                    '$' => {
                        self.dollar(&mut String::new());
                        continue;
                    }
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            self.at += 1;
                            break;
                        }
                    }
                    '\\' => self.at += 1,
                    _ => {}
                }
                self.at += 1;
            }
            into.push_str(SUBSTITUTED);
        } else {
            self.at += 1;
            into.push('$');
        }
    }

    /// Arithmetic, read past its `))`: it runs nothing, though a `$(…)` in it does.
    fn arithmetic(&mut self, into: &mut String) {
        let mut depth = 0usize;
        while let Some(c) = self.peek() {
            match c {
                '$' => self.dollar(into),
                '(' => {
                    self.at += 1;
                    depth += 1;
                }
                ')' if depth == 0 && self.starts("))") => {
                    self.at += 2;
                    return;
                }
                ')' => {
                    self.at += 1;
                    depth = depth.saturating_sub(1);
                }
                _ => self.at += 1,
            }
        }
    }

    /// `<<EOF`, `<<-'EOF'`: its closing word, kept until the line ends and its body starts.
    fn open_heredoc(&mut self) {
        let tabs = self.peek() == Some('-');
        if tabs {
            self.at += 1;
        }
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.at += 1;
        }
        let mut end = String::new();
        let mut quoted = false;
        while let Some(c) = self.peek() {
            if c.is_whitespace() || ";|&<>()".contains(c) {
                break;
            }
            self.at += 1;
            match c {
                '\'' | '"' | '\\' => quoted = true,
                _ => end.push(c),
            }
        }
        if !end.is_empty() {
            self.heredocs.push(Heredoc {
                end,
                tabs,
                quoted,
                script: false,
            });
        }
    }

    /// The bodies of the here-documents the line just ended opened, read past their closing
    /// lines: a script's body as a script, an unquoted body for its substitutions, and a quoted
    /// one not at all.
    fn heredoc_bodies(&mut self) {
        for heredoc in std::mem::take(&mut self.heredocs) {
            let mut body = String::new();
            while self.at < self.text.len() {
                let end = self.text[self.at..]
                    .iter()
                    .position(|&c| c == '\n')
                    .map_or(self.text.len(), |n| self.at + n);
                let line: String = self.text[self.at..end].iter().collect();
                self.at = (end + 1).min(self.text.len());
                let closing = if heredoc.tabs {
                    line.trim_start_matches('\t')
                } else {
                    line.as_str()
                };
                if closing.trim_end_matches('\r') == heredoc.end {
                    break;
                }
                body.push_str(&line);
                body.push('\n');
            }
            if heredoc.script {
                self.found.extend(programs(&body));
            } else if !heredoc.quoted {
                let mut inner = Reader::new(&body);
                inner.expanding(&mut String::new(), Expanding::Body);
                self.found.extend(inner.found);
            }
        }
    }
}

/// `NAME=value` before a command sets a variable for it.
fn assigns(word: &Word) -> bool {
    word.text.contains('=') && !word.text.starts_with('=')
}

/// The program `word`, the first of a command, calls, if it calls one.
fn program(word: &Word) -> Option<String> {
    let text = word.text.as_str();
    let calls = !text.is_empty()
        && !(word.quoted && text.contains(char::is_whitespace))
        && !text.starts_with(['$', '-', '.', '~', '{', '#', '*'])
        && !text.ends_with(['{', '}'])
        && !text.contains(['$', '*', '?', '[', ']', '=', '<', '>'])
        && !text.chars().all(|c| c.is_ascii_digit())
        && !SHELL_OWN.contains(&text);
    calls.then(|| text.to_owned())
}
