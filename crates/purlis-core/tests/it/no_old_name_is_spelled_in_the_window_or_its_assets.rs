// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **No old name is spelled in the window or in the plugin it ships** (#1284, guard scope).
//!
//! `no_old_name_is_spelled_outside_the_name_module` holds the Rust; this holds what it does
//! not read, by the same rule and the same spellings (`old_names`, from
//! [`purlis_core::names::ALL`]):
//!
//! * **the window's TypeScript**: every `.ts` and `.tsx` under `app/src`, `windowprefs.ts`
//!   included, apart from the test files (a `*.test.*` or `*.testkit.*` file, a `test-*`
//!   stand-in, anything under `__tests__` or `__mocks__`);
//! * **the plugin the app ships to each chat**: every file under `app/src-tauri/plugin`
//!   (`hooks/hooks.json`, `.claude-plugin/plugin.json`, the opencode shim `opencode/purlis.ts`,
//!   anything a skill carries beside its text), except the skills' own `.md` text, which
//!   `every_shipped_skill_names_only_purlis` already holds.
//!
//! **What is read.** In TypeScript, the string literals: `'…'`, `"…"`, a template literal whole
//! (its `${…}` read as code), a JSX attribute's string, and JSX text, which is what the window
//! shows. Comments are prose about the names and are not read; nor are regular expressions,
//! which read a name rather than spell one. In JSON, the string values (not the keys). Any
//! other file, line by line.
//!
//! **What is looked for.** The old file, folder, marker, keychain, variable, branch, trailer
//! and plugin spellings (`old_names::old_spellings`). The bare word `charter` is not: it is
//! also the persona's and the workspace's *charter* ("Its charter, memory and todos"), and the
//! product's name in a sentence is RN-11a's (#1269), as the Rust guard decides.
//!
//! **The allow-list.** What still spells an old name — on purpose for the compatibility window,
//! or as a leftover a ticket removes — is on [`STILL_SPELLED`], file by file, with the exact
//! number of literals and a [`Why`] naming that ticket. The counts are exact, so the list can
//! only SHRINK: a file that spells one more fails until a reviewer raises its count, and one
//! that spells one fewer fails until its count is lowered.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::old_names;
use old_names::{old_spellings, spells};

/// Why a window or plugin file may still spell an old name. Each names the ticket that ends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Why {
    /// Text the window shows a person (a label, a hint, a note) naming an old file, variable
    /// or branch, or an old file name kept as the label of the file in force when the core
    /// names none. RN-11a (#1269) renames the visible words.
    Prose,
    /// `CHARTER_HOOK_BINARY`, the variable the plugin's hook commands and the opencode shim
    /// read: an old spelling kept on purpose for the compatibility window (D-RN8-1, as the hook
    /// commands keep theirs, D-RN7-11). It becomes `PURLIS_HOOK_BINARY` at 1.0: #1305's fourth
    /// item, with RN-14 (#1273).
    HookBinary,
    /// An old file name the window reads on purpose, to say a file as a project still in the
    /// compatibility window names it (`charter.toml` for `purlis.toml`, D-RN8-1). It goes with
    /// the old names' reading, at 1.0 (#1305).
    Compat,
    /// A `charter/`-shaped id that is none of the name module's entries: the namespace of
    /// purlis's own panels and views (`purlis_core::panel`'s, which the Rust guard lists the
    /// same way). Whether it joins the name module, read under both, or is renamed outright is
    /// #1284's first item.
    Unlisted,
}

/// Files that still spell an old name, by path from the repository: how many literals do, and
/// why. A file with two reasons has an entry for each; its counts add up.
///
/// **Shrink only.** Lowering a count, or removing an entry, is the whole of finishing one.
/// Raising one, or adding one, is a decision a reviewer makes on purpose.
#[rustfmt::skip]
const STILL_SPELLED: &[(&str, usize, Why)] = &[
    // Visible text: RN-11a (#1269).
    ("app/src/actions.ts", 1, Why::Prose),
    ("app/src/settings/fileControls.ts", 8, Why::Prose),
    // The project's files' old names, which `namedIn` reads to say a sentence's file names as a
    // project not yet renamed calls its files.
    ("app/src/settings/project.ts", 2, Why::Compat),
    // The built-in themes' old ids, in the window's alias table `THEME_ALIASES`, which reads a
    // saved pick of one as the renamed theme until 1.0 (#1838); `themeIds.test.ts` holds it to
    // the name module's `THEME_DARK` and `THEME_LIGHT`.
    ("app/src/theme/theme.ts", 2, Why::Compat),
    // The built-in icon theme's old id, in the window's alias table `ICON_THEME_ALIASES`, read
    // as `purlis-icons` until 1.0 (#1854); `themeIds.test.ts` holds it to `ICON_THEME`.
    ("app/src/theme/icons.ts", 1, Why::Compat),
    // The hook binary's variable, kept until 1.0: #1305, RN-14 (#1273). Once in each generated
    // hook's command: sixteen since `userpromptexpansion` (#2775).
    ("app/src-tauri/plugin/hooks/hooks.json", 16, Why::HookBinary),
    ("app/src-tauri/plugin/opencode/purlis.ts", 1, Why::HookBinary),
    // purlis's own panels' and views' namespace: #1284.
    ("app/src/Panels.tsx", 8, Why::Unlisted),
    ("app/src/tabs.ts", 1, Why::Unlisted),
];

// ---- the TypeScript ------------------------------------------------------------------ //

/// A literal: the line it starts on, its text, and the byte offset just past it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Literal {
    line: usize,
    text: String,
    end: usize,
}

/// Words after which a `/` starts a regular expression and a `<` an element.
const OPERAND_AFTER: [&str; 16] = [
    "return",
    "typeof",
    "case",
    "do",
    "else",
    "in",
    "of",
    "new",
    "delete",
    "void",
    "throw",
    "yield",
    "await",
    "instanceof",
    "default",
    "extends",
];

/// TypeScript lexed just enough to find its strings. `jsx` reads elements (a `.tsx` file).
struct Lexer<'s> {
    src: &'s str,
    b: &'s [u8],
    i: usize,
    jsx: bool,
    /// Whether a `/` or a `<` here starts an operand (a regular expression, an element) rather
    /// than dividing or comparing.
    operand: bool,
    out: Vec<Literal>,
    breaks: Vec<usize>,
    /// Where the lexer lost its place: a `}` with no `{`, a string or a template left open,
    /// the end reached inside braces. A file the guard misread would show here.
    lost: Vec<usize>,
}

impl<'s> Lexer<'s> {
    fn new(src: &'s str, jsx: bool) -> Self {
        Lexer {
            src,
            b: src.as_bytes(),
            i: 0,
            jsx,
            operand: true,
            out: Vec::new(),
            breaks: src.match_indices('\n').map(|(at, _)| at).collect(),
            lost: Vec::new(),
        }
    }

    fn line_at(&self, at: usize) -> usize {
        self.breaks.partition_point(|&b| b < at) + 1
    }

    /// Past the character at `i`, and the one after it when that one is an escape's.
    fn past_char(&self) -> usize {
        let len = |at: usize| self.src[at..].chars().next().map_or(1, char::len_utf8);
        let mut to = self.i + len(self.i);
        if self.b[self.i] == b'\\' && to < self.b.len() {
            to += len(to);
        }
        to.min(self.b.len())
    }

    /// Past a comment at `i`, if one starts there. Returns whether one did.
    fn comment(&mut self) -> bool {
        let rest = self.rest();
        if rest.starts_with("//") {
            self.i = rest.find('\n').map_or(self.b.len(), |n| self.i + n);
        } else if let Some(body) = rest.strip_prefix("/*") {
            self.i = body.find("*/").map_or(self.b.len(), |n| self.i + n + 4);
        } else {
            return false;
        }
        true
    }

    fn rest(&self) -> &'s str {
        &self.src[self.i.min(self.src.len())..]
    }

    fn push(&mut self, from: usize, text: String) {
        let line = self.line_at(from);
        self.out.push(Literal {
            line,
            text,
            end: self.i,
        });
    }

    /// Code, up to the end, or up to the `}` that closes the `{` it was entered after.
    /// Returns whether it stopped on that `}` (consumed).
    fn code(&mut self, in_braces: bool) -> bool {
        let mut depth = 0usize;
        while self.i < self.b.len() {
            let c = self.b[self.i];
            if c.is_ascii_whitespace() {
                self.i += 1;
            } else if self.comment() {
                // Prose about the names: not read.
            } else if c == b'"' || c == b'\'' {
                self.string(c);
                self.operand = false;
            } else if c == b'`' {
                self.template();
                self.operand = false;
            } else if c == b'/' && self.operand {
                self.regex();
                self.operand = false;
            } else if c == b'<' && self.jsx && self.operand && self.element() {
                self.operand = false;
            } else if c == b'{' {
                depth += 1;
                self.i += 1;
                self.operand = true;
            } else if c == b'}' {
                self.i += 1;
                if depth == 0 && in_braces {
                    return true;
                }
                if depth == 0 {
                    self.lost.push(self.line_at(self.i));
                }
                depth = depth.saturating_sub(1);
                self.operand = false;
            } else if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80 {
                let start = self.i;
                while self.i < self.b.len()
                    && (self.b[self.i].is_ascii_alphanumeric()
                        || matches!(self.b[self.i], b'_' | b'$')
                        || self.b[self.i] >= 0x80)
                {
                    self.i += 1;
                }
                self.operand = OPERAND_AFTER.contains(&&self.src[start..self.i]);
            } else {
                self.i += 1;
                self.operand = !matches!(c, b')' | b']');
            }
        }
        if in_braces || depth > 0 {
            self.lost.push(self.line_at(self.i));
        }
        false
    }

    /// A `'…'` or `"…"` string.
    fn string(&mut self, quote: u8) {
        let start = self.i;
        self.i += 1;
        let mut text = String::new();
        while self.i < self.b.len() && self.b[self.i] != quote && self.b[self.i] != b'\n' {
            let to = self.past_char();
            text.push_str(&self.src[self.i..to]);
            self.i = to;
        }
        if self.i >= self.b.len() || self.b[self.i] != quote {
            self.lost.push(self.line_at(start));
        }
        self.i = (self.i + 1).min(self.b.len());
        self.push(start, text);
    }

    /// A template literal, as one literal; each `${…}` is read as code and stands as `${}`.
    fn template(&mut self) {
        let start = self.i;
        self.i += 1;
        let mut text = String::new();
        while self.i < self.b.len() && self.b[self.i] != b'`' {
            if self.rest().starts_with("${") {
                // A literal inside the `${…}` is one of its own.
                self.i += 2;
                self.operand = true;
                self.code(true);
                text.push_str("${}");
                continue;
            }
            let to = self.past_char();
            text.push_str(&self.src[self.i..to]);
            self.i = to;
        }
        if self.i >= self.b.len() {
            self.lost.push(self.line_at(start));
        }
        self.i = (self.i + 1).min(self.b.len());
        self.push(start, text);
    }

    /// A regular expression literal, skipped.
    fn regex(&mut self) {
        self.i += 1;
        let mut class = false;
        while self.i < self.b.len() && self.b[self.i] != b'\n' {
            match self.b[self.i] {
                b'[' => class = true,
                b']' => class = false,
                b'/' if !class => break,
                _ => {}
            }
            self.i = self.past_char();
        }
        self.i += 1;
        while self.i < self.b.len() && self.b[self.i].is_ascii_alphabetic() {
            self.i += 1;
        }
    }

    /// A JSX element (or fragment) at `<`, its attributes' strings and its text read.
    /// Returns false, having read nothing, where the `<` opens a type parameter instead
    /// (`<T,>(…)`, `<T extends U>(…)`).
    fn element(&mut self) -> bool {
        let mut j = self.i + 1;
        let name_start = j;
        while j < self.b.len()
            && (self.b[j].is_ascii_alphanumeric() || matches!(self.b[j], b'_' | b'.' | b':' | b'-'))
        {
            j += 1;
        }
        let name = &self.src[name_start..j];
        let after = self.src[j..].trim_start();
        let fragment = name.is_empty() && after.starts_with('>');
        if !fragment
            && (name.is_empty()
                || !name.as_bytes()[0].is_ascii_alphabetic()
                || after.starts_with(',')
                || after.starts_with("extends "))
        {
            return false;
        }
        self.i = j;
        // Attributes.
        loop {
            while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
                self.i += 1;
            }
            let rest = self.rest();
            if rest.is_empty() {
                return true;
            }
            if rest.starts_with("/>") {
                self.i += 2;
                return true;
            }
            // A comment between attributes.
            if self.comment() {
                continue;
            }
            match self.b[self.i] {
                b'>' => {
                    self.i += 1;
                    break;
                }
                b'{' => {
                    self.i += 1;
                    self.operand = true;
                    self.code(true);
                }
                b'"' | b'\'' => {
                    // A JSX attribute's string has no escapes.
                    let quote = self.b[self.i];
                    let start = self.i;
                    let close = self.src[self.i + 1..]
                        .find(quote as char)
                        .map_or(self.b.len(), |n| self.i + 1 + n);
                    let text = self.src[start + 1..close].to_owned();
                    self.i = (close + 1).min(self.b.len());
                    self.push(start, text);
                }
                _ => self.i = self.past_char(),
            }
        }
        // Children, to the closing tag.
        loop {
            if self.i >= self.b.len() {
                self.lost.push(self.line_at(self.i));
                return true;
            }
            let rest = self.rest();
            if rest.starts_with("</") {
                self.i = rest.find('>').map_or(self.b.len(), |n| self.i + n + 1);
                return true;
            }
            match self.b[self.i] {
                b'<' => {
                    if !self.element() {
                        self.i += 1;
                    }
                }
                b'{' => {
                    self.i += 1;
                    self.operand = true;
                    self.code(true);
                }
                _ => {
                    let start = self.i;
                    let stop = rest.find(['<', '{']).map_or(self.b.len(), |n| self.i + n);
                    let text = self.src[start..stop].trim();
                    self.i = stop;
                    if !text.is_empty() {
                        let lead =
                            self.src[start..stop].len() - self.src[start..stop].trim_start().len();
                        let text = text.to_owned();
                        self.push(start + lead, text);
                    }
                }
            }
        }
    }
}

/// The literals of a TypeScript source; `jsx` for a `.tsx` file.
fn ts_literals(src: &str, jsx: bool) -> Vec<Literal> {
    let mut lexer = Lexer::new(src, jsx);
    lexer.code(false);
    lexer.out
}

/// The string values of a JSON text: its strings, less those a `:` follows (the keys).
fn json_values(src: &str) -> Vec<Literal> {
    ts_literals(src, false)
        .into_iter()
        .filter(|l| !src[l.end..].trim_start().starts_with(':'))
        .collect()
}

/// Each line of a text.
fn lines(src: &str) -> Vec<Literal> {
    src.lines()
        .enumerate()
        .map(|(n, line)| Literal {
            line: n + 1,
            text: line.to_owned(),
            end: 0,
        })
        .collect()
}

/// What of `src`, a file at `rel`, is read: by its kind.
fn readable(rel: &str, src: &str) -> Vec<Literal> {
    if rel.ends_with(".tsx") {
        ts_literals(src, true)
    } else if rel.ends_with(".ts") || rel.ends_with(".js") || rel.ends_with(".mjs") {
        ts_literals(src, false)
    } else if rel.ends_with(".json") {
        json_values(src)
    } else {
        lines(src)
    }
}

// ---- the tree ------------------------------------------------------------------------- //

/// A spelling found: the line, the spelling, and the literal it is in.
type Hit = (usize, String, String);

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository")
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "node_modules" && name != "target" {
                walk(&path, out);
            }
        } else {
            out.push(path);
        }
    }
}

/// Whether a file under `app/src` is a test's, not the window's.
fn is_test(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    name.contains(".test.")
        || name.contains(".testkit.")
        || name.starts_with("test-")
        || rel
            .split('/')
            .any(|part| part == "__tests__" || part == "__mocks__")
}

/// Every file this guard reads, by path from `root`.
fn scanned(root: &Path) -> Vec<String> {
    let rel = |p: &Path| {
        p.strip_prefix(root)
            .expect("under the root")
            .to_string_lossy()
            .replace('\\', "/")
    };
    let mut window = Vec::new();
    walk(&root.join("app/src"), &mut window);
    let mut plugin = Vec::new();
    walk(&root.join("app/src-tauri/plugin"), &mut plugin);
    let mut files: Vec<String> = window
        .iter()
        .map(|p| rel(p))
        .filter(|r| (r.ends_with(".ts") || r.ends_with(".tsx")) && !is_test(r))
        .chain(
            plugin
                .iter()
                .map(|p| rel(p))
                // The skills' text is `every_shipped_skill_names_only_purlis`'s.
                .filter(|r| !(r.contains("/plugin/skills/") && r.ends_with(".md"))),
        )
        .collect();
    files.sort();
    files
}

/// Every scanned file's literals that spell an old name, and how many files were read.
fn scan(root: &Path) -> (BTreeMap<String, Vec<Hit>>, usize) {
    let spellings = old_spellings();
    let files = scanned(root);
    let mut found = BTreeMap::new();
    for rel in &files {
        let src = std::fs::read_to_string(root.join(rel)).expect("a text file");
        let hits: Vec<Hit> = readable(rel, &src)
            .into_iter()
            .filter_map(|l| Some((l.line, spells(&l.text, &spellings)?.to_owned(), l.text)))
            .collect();
        if !hits.is_empty() {
            found.insert(rel.clone(), hits);
        }
    }
    (found, files.len())
}

/// What is wrong with `found` against `allowed`: a file spelling more than its allowance,
/// with each literal; one spelling fewer, to lower; an entry spelling none.
fn beyond_allowance(
    found: &BTreeMap<String, Vec<Hit>>,
    allowed: &[(&str, usize, Why)],
) -> Vec<String> {
    let mut wrong = Vec::new();
    let allowance = |file: &str| -> usize {
        allowed
            .iter()
            .filter(|(f, ..)| *f == file)
            .map(|(_, n, _)| n)
            .sum()
    };
    for (file, hits) in found {
        let allowed = allowance(file);
        match allowed.cmp(&hits.len()) {
            std::cmp::Ordering::Equal => {}
            std::cmp::Ordering::Greater => wrong.push(format!(
                "{file}: allowed {allowed} but spells {} — lower its count in STILL_SPELLED",
                hits.len()
            )),
            std::cmp::Ordering::Less => {
                wrong.push(format!(
                    "{file}: spells {} old name(s), allowed {allowed} — write the purlis name:",
                    hits.len()
                ));
                for (line, spelling, text) in hits {
                    let shown: String = text.chars().take(100).collect();
                    wrong.push(format!("    {file}:{line}: `{spelling}` in \"{shown}\""));
                }
            }
        }
    }
    for (file, _, why) in allowed {
        if !found.contains_key(*file) {
            wrong.push(format!(
                "{file}: allowed ({why:?}) but spells none — remove its entry"
            ));
        }
    }
    wrong
}

#[test]
fn no_window_or_plugin_file_spells_an_old_name_beyond_its_allowance() {
    purlis_core::unsteered!();
    let root = repo();
    let (found, read) = scan(&root);
    assert!(read > 200, "the walk read {read} files");
    let files = scanned(&root);
    for wanted in [
        "app/src/windowprefs.ts",
        "app/src-tauri/plugin/hooks/hooks.json",
        "app/src-tauri/plugin/opencode/purlis.ts",
    ] {
        assert!(files.iter().any(|f| f == wanted), "{wanted} is not read");
    }
    let wrong = beyond_allowance(&found, STILL_SPELLED);
    assert!(
        wrong.is_empty(),
        "old names in the window or its plugin:\n{}",
        wrong.join("\n")
    );
}

// ---- the guard's own tests ------------------------------------------------------------ //

/// The texts of `src`'s literals.
fn texts(src: &str, jsx: bool) -> Vec<String> {
    ts_literals(src, jsx).into_iter().map(|l| l.text).collect()
}

#[test]
fn a_comment_and_a_regular_expression_are_not_read_and_every_string_is() {
    purlis_core::unsteered!();
    let src = r#"
// a comment about ".charter" is prose
/* so is "charter.toml", and 'CHARTER_ROOT' */
const A = "charter.local.toml";
const B = 'it\'s CHARTER_HOME';
const C = `${root}/.charter/${"x" + `in ${'charter/y'}`}`;
const R = /^charter\//.test(id) ? a / b : c;
const D = x < y && "Charter-Chat";
"#;
    assert_eq!(
        texts(src, false),
        [
            "charter.local.toml",
            "it\\'s CHARTER_HOME",
            "x",
            "charter/y",
            "in ${}",
            "${}/.charter/${}",
            "Charter-Chat",
        ]
    );
}

#[test]
fn jsx_text_and_attributes_are_read_and_its_comments_are_not() {
    purlis_core::unsteered!();
    let src = r#"
const f = <T,>(x: T) => x;
const n = useRef<HTMLDivElement>(null);
export function P({ on }: { on: boolean }) {
  return (
    <div className="row" title='CHARTER_ROOT'>
      {/* the old "charter.toml" in a comment */}
      Don't write <code>charter.toml</code> here.
      {on ? <b>.charter</b> : null}
      <Icon name={"charter/x"}
        // it's a comment between attributes, with ".charter" in it
        on />
      <>frag</>
    </div>
  );
}
"#;
    assert_eq!(
        texts(src, true),
        [
            "row",
            "CHARTER_ROOT",
            "Don't write",
            "charter.toml",
            "here.",
            ".charter",
            "charter/x",
            "frag",
        ]
    );
}

#[test]
fn a_json_key_is_not_read_and_its_value_is() {
    purlis_core::unsteered!();
    let src =
        "{\n  \"CHARTER_X\": \"a\",\n  \"command\": \"\\\"${CHARTER_HOOK_BINARY}\\\" hook\"\n}\n";
    let read: Vec<(usize, String)> = json_values(src)
        .into_iter()
        .map(|l| (l.line, l.text))
        .collect();
    assert_eq!(
        read,
        [
            (2, "a".to_owned()),
            (3, "\\\"${CHARTER_HOOK_BINARY}\\\" hook".to_owned())
        ]
    );
}

/// A tree of `files` (path, body), scanned.
fn scan_of(files: &[(&str, &str)]) -> (BTreeMap<String, Vec<Hit>>, Vec<String>) {
    let tree = tempfile::tempdir().expect("a tree");
    for (path, body) in files {
        let at = tree.path().join(path);
        std::fs::create_dir_all(at.parent().expect("a folder")).expect("folders");
        std::fs::write(at, body).expect("a file");
    }
    (scan(tree.path()).0, scanned(tree.path()))
}

#[test]
fn the_window_and_the_plugin_are_read_and_tests_and_the_skills_text_are_not() {
    purlis_core::unsteered!();
    let old = "export const F = \"charter.toml\";\n";
    let (found, read) = scan_of(&[
        ("app/src/a.ts", old),
        ("app/src/settings/b.tsx", old),
        ("app/src/a.test.ts", old),
        ("app/src/b.window.test.tsx", old),
        ("app/src/c.testkit.ts", old),
        ("app/src/test-setup.ts", old),
        ("app/src/__mocks__/m.ts", old),
        ("app/src/App.css", ".charter {}"),
        (
            "app/src-tauri/plugin/hooks/hooks.json",
            "{\"c\": \"$CHARTER_HOOK_BINARY\"}",
        ),
        ("app/src-tauri/plugin/opencode/purlis.ts", old),
        ("app/src-tauri/plugin/skills/x/SKILL.md", "`charter.toml`"),
        ("app/src-tauri/plugin/skills/x/run.sh", "cat charter.toml"),
    ]);
    assert_eq!(
        found.keys().collect::<Vec<_>>(),
        [
            "app/src-tauri/plugin/hooks/hooks.json",
            "app/src-tauri/plugin/opencode/purlis.ts",
            "app/src-tauri/plugin/skills/x/run.sh",
            "app/src/a.ts",
            "app/src/settings/b.tsx",
        ]
    );
    assert_eq!(read.len(), 5, "{read:?}");
}

#[test]
fn an_unlisted_file_a_stale_count_and_a_stale_entry_each_fail() {
    purlis_core::unsteered!();
    let (found, _) = scan_of(&[
        (
            "app/src/a.ts",
            "const A = \"charter.toml\";\nconst B = \".charter\";\n",
        ),
        ("app/src/b.ts", "const B = \"CHARTER_ROOT\";\n"),
    ]);
    let allowed = [
        ("app/src/a.ts", 3, Why::Prose),
        ("app/src/gone.ts", 1, Why::HookBinary),
    ];
    assert_eq!(
        beyond_allowance(&found, &allowed),
        [
            "app/src/a.ts: allowed 3 but spells 2 — lower its count in STILL_SPELLED",
            "app/src/b.ts: spells 1 old name(s), allowed 0 — write the purlis name:",
            "    app/src/b.ts:1: `CHARTER_` in \"CHARTER_ROOT\"",
            "app/src/gone.ts: allowed (HookBinary) but spells none — remove its entry",
        ]
    );
    let exact = [
        ("app/src/a.ts", 2, Why::Prose),
        ("app/src/b.ts", 1, Why::Unlisted),
    ];
    assert!(beyond_allowance(&found, &exact).is_empty());
}

#[test]
fn every_file_read_as_typescript_is_lexed_without_losing_its_place() {
    purlis_core::unsteered!();
    // A string, template or element the lexer misread would swallow code, or stop short, and
    // leave a brace or a quote unmatched: every file the guard reads as TypeScript has none.
    let root = repo();
    let mut lost = Vec::new();
    for rel in scanned(&root) {
        if !(rel.ends_with(".ts") || rel.ends_with(".tsx")) {
            continue;
        }
        let src = std::fs::read_to_string(root.join(&rel)).expect("a source file");
        let mut lexer = Lexer::new(&src, rel.ends_with(".tsx"));
        lexer.code(false);
        lost.extend(lexer.lost.iter().map(|line| format!("{rel}:{line}")));
    }
    assert!(
        lost.is_empty(),
        "the lexer lost its place at:\n{}",
        lost.join("\n")
    );
}

#[test]
fn an_unclosed_brace_or_string_is_where_the_lexer_loses_its_place() {
    purlis_core::unsteered!();
    for (src, jsx) in [
        ("const a = \"open;\n", false),
        ("}\n", false),
        ("function f() {\n", false),
        ("const t = `never closed\n", false),
        ("const e = <div>{x</div>;\n", true),
    ] {
        let mut lexer = Lexer::new(src, jsx);
        lexer.code(false);
        assert!(!lexer.lost.is_empty(), "{src}");
    }
    let mut lexer = Lexer::new("const e = <a href='x'>{y} it's</a>;\n", true);
    lexer.code(false);
    assert!(lexer.lost.is_empty());
}
