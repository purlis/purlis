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

/// The shells whose `-c` argument or here-document is a script.
const SHELLS: &[&str] = &["sh", "bash", "zsh", "dash", "ksh"];

/// The programs `script` calls, in the order it names them: the first word of each command a
/// pipe, a list, a subshell or a substitution starts, past what sets a variable for it and the
/// shell's leading words. A variable, an option, a redirection, a quoted word with a space in it,
/// the skill's own file (`./`, a `$` path) and the shell's own words are no program.
pub(super) fn programs(script: &str) -> Vec<String> {
    let mut reader = Reader {
        text: script.chars().collect(),
        at: 0,
        found: Vec::new(),
        heredocs: Vec::new(),
        cases: 0,
    };
    reader.list(None);
    reader.found
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
    /// `bash <<EOF`: the body is a script.
    script: bool,
}

/// The command being read: its words so far, and where its program goes among those found, so a
/// substitution inside it is listed after it.
struct Command {
    words: Vec<Word>,
    slot: usize,
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

    /// Reads a list of commands up to `close` (the `)` of a `$(`, a closing backtick) or the end,
    /// and past it.
    fn list(&mut self, close: Option<char>) {
        let mut command = Command {
            words: Vec::new(),
            slot: self.found.len(),
        };
        let mut word: Option<Word> = None;
        let mut depth = 0usize;
        let mut pattern = false;
        while let Some(c) = self.peek() {
            if pattern && word.is_none() && !c.is_whitespace() {
                pattern = self.case_pattern();
                continue;
            }
            // Inside `[[ … ]]`, `(`, `|`, `&`, `<` and `>` are the test's.
            let test = command.words.first().is_some_and(|w| w.text == "[[")
                && !command.words.iter().any(|w| w.text == "]]");
            match c {
                ')' if close == Some(')') && depth == 0 => {
                    self.at += 1;
                    break;
                }
                '`' if close == Some('`') => {
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
                    self.expanding(&mut w.text, Some('"'));
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
                '`' => {
                    self.at += 1;
                    self.list(Some('`'));
                    word.get_or_insert_default().text.push_str("$`…`");
                }
                '#' if word.is_none() => {
                    while self.peek().is_some_and(|c| c != '\n') {
                        self.at += 1;
                    }
                }
                '(' if word.as_ref().is_some_and(|w| w.text.ends_with('=')) => {
                    // An array, `name=(a b c)`: its words are data.
                    self.at += 1;
                    let w = word.as_mut().expect("the word is there");
                    self.expanding(&mut w.text, Some(')'));
                }
                '(' if word.is_none() && self.starts("((") => {
                    // Arithmetic, `(( n++ ))`, runs nothing.
                    self.at += 2;
                    self.arithmetic(&mut String::new());
                }
                '<' if self.starts("<<") && !self.starts("<<<") => {
                    self.at += 2;
                    self.end_word(&mut command, &mut word, &mut pattern);
                    self.open_heredoc(&command);
                }
                '&' if word.as_ref().is_some_and(|w| w.text.ends_with(['<', '>'])) => {
                    // `2>&1`, `<&3`: a redirection, not a list.
                    self.at += 1;
                    word.get_or_insert_default().text.push('&');
                }
                '|' | '&' | '(' | ')' | '<' | '>' if test => {
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
                            pattern = true;
                        }
                        _ => {}
                    }
                    self.end_word(&mut command, &mut word, &mut pattern);
                    self.end_command(&mut command);
                    if c == '\n' {
                        self.heredoc_bodies();
                    }
                }
                ' ' | '\t' | '\r' => {
                    self.at += 1;
                    self.end_word(&mut command, &mut word, &mut pattern);
                }
                _ => {
                    self.at += 1;
                    word.get_or_insert_default().text.push(c);
                }
            }
        }
        self.end_word(&mut command, &mut word, &mut pattern);
        self.end_command(&mut command);
    }

    /// A word is done. `case … in` opens a `case`, whose first arm's pattern follows; `esac`
    /// closes one.
    fn end_word(&mut self, command: &mut Command, word: &mut Option<Word>, pattern: &mut bool) {
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
            *pattern = true;
            self.end_command(command);
            return;
        }
        if command.words.is_empty() && word.text == "esac" && !word.quoted {
            self.cases = self.cases.saturating_sub(1);
        }
        command.words.push(word);
    }

    /// A command is done: its program is listed in its place, and what it hands a shell is read.
    fn end_command(&mut self, command: &mut Command) {
        let words = std::mem::take(&mut command.words);
        let slot = command.slot.min(self.found.len());
        command.slot = self.found.len();
        let Some(at) = words
            .iter()
            .position(|w| !(assigns(w) || (!w.quoted && LEADING.contains(&w.text.as_str()))))
        else {
            return;
        };
        let head = &words[at];
        if let Some(program) = program(head) {
            self.found.insert(slot, program);
        }
        let rest = &words[at + 1..];
        let handed = if head.text == "eval" && !head.quoted {
            Some(
                rest.iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        } else if is_shell(&head.text) {
            rest.iter()
                .position(|w| {
                    !w.quoted
                        && w.text.starts_with('-')
                        && !w.text.starts_with("--")
                        && w.text.contains('c')
                })
                .and_then(|flag| rest.get(flag + 1))
                .map(|w| w.text.clone())
        } else {
            None
        };
        if let Some(script) = handed {
            self.found.extend(programs(&script));
        }
    }

    /// The pattern of a `case` arm, up to its `)`, which runs nothing; or `esac`. Whether a
    /// pattern is still to come.
    fn case_pattern(&mut self) -> bool {
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
                '"' => self.expanding(&mut String::new(), Some('"')),
                '\\' => self.at += 1,
                _ => {}
            }
        }
        false
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

    /// Text the shell expands but does not split into commands, read past `close` into `into`:
    /// a double-quoted string (`"`), an array's words (`)`), or with no `close` an unquoted
    /// here-document's body to its end. Only what a `$(…)` or a `` `…` `` in it runs counts.
    fn expanding(&mut self, into: &mut String, close: Option<char>) {
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
                '`' => {
                    self.at += 1;
                    self.list(Some('`'));
                    into.push_str("$`…`");
                }
                '\'' if close == Some(')') => {
                    self.at += 1;
                    self.single_quoted(into, false);
                }
                '"' if close == Some(')') => {
                    self.at += 1;
                    self.expanding(into, Some('"'));
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
            self.arithmetic(into);
            into.push_str("$((…))");
        } else if self.starts("$(") {
            self.at += 2;
            self.list(Some(')'));
            into.push_str("$(…)");
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
            into.push_str("${…}");
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
    fn open_heredoc(&mut self, command: &Command) {
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
                '\'' | '"' => quoted = true,
                '\\' => quoted = true,
                _ => end.push(c),
            }
        }
        if end.is_empty() {
            return;
        }
        let script = command
            .words
            .iter()
            .find(|w| !(assigns(w) || LEADING.contains(&w.text.as_str())))
            .is_some_and(|w| is_shell(&w.text));
        self.heredocs.push(Heredoc {
            end,
            tabs,
            quoted,
            script,
        });
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
                let mut inner = Reader {
                    text: body.chars().collect(),
                    at: 0,
                    found: Vec::new(),
                    heredocs: Vec::new(),
                    cases: 0,
                };
                inner.expanding(&mut String::new(), None);
                self.found.extend(inner.found);
            }
        }
    }
}

/// `NAME=value` before a command sets a variable for it.
fn assigns(word: &Word) -> bool {
    word.text.contains('=') && !word.text.starts_with('=')
}

/// Whether `program` is a shell, by name or path.
fn is_shell(program: &str) -> bool {
    let name = program.rsplit('/').next().unwrap_or(program);
    SHELLS.contains(&name)
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
