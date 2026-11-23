// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The floor's merge half: every way a forge CLI or git can merge a request, or set one to merge
//! on its own later, is the same act to the floor as `gh pr merge` (#866).
//!
//! The rule is about the act, not one spelling of it. Unattended:
//!
//! * a forge API call to a **merge endpoint**, or one that **sets auto-merge or a merge queue**, is
//!   refused;
//! * a forge API call whose effect on a merge **cannot be read off the command line** is refused
//!   too. This fails closed: what charter cannot read, it treats as a merge;
//! * a **push option that sets auto-merge** is refused, however git is handed it;
//! * a forge API call that **moves the default branch's ref** (`git/refs/heads/main`, or a new
//!   `refs/heads/main`), or one whose branch cannot be read, is refused (Q330);
//! * an **alias** is the command it stands for, and making one that stands for a held command,
//!   or for the start of one, is refused.
//!
//! A **release or a tag made through the API** (`releases`, `repository/tags`, `git/tags`, a
//! `refs/tags/` ref) is the act `glab release create` and `git tag` are, so a write to one is
//! refused the same way (#6294).
//!
//! A read is never refused. An API call that is a `GET` cannot merge anything, whatever endpoint
//! it names, so an unattended run can still read a request, its checks and whether it merged.
//! Attended, nothing here runs at all: [`super::release_floor_reason`] returns first.

use std::sync::OnceLock;

use regex::Regex;

use crate::shellseg;
use crate::shellwrap;

/// The sentence a refusal of a merge or an auto-merge setting ends with.
pub(super) const MERGES: &str =
    "Merging a request, or setting one to merge on its own later, lands code.";

/// The sentence a refusal ends with when the call's effect on a merge cannot be read.
pub(super) const UNREADABLE: &str =
    "purlis cannot read what this call would do to a merge, so it is treated as one.";

/// The sentence a refusal of a release or a tag made through the API ends with.
const PUBLISHES: &str =
    "A release or a tag made through the API publishes, as `release create` does.";

/// The sentence a refusal of a new ref whose name cannot be read ends with (#6962): it could be
/// a tag, but it is no merge.
pub(super) const REF_UNREADABLE: &str =
    "purlis can't tell what this ref writes; write it out literally.";

/// The branches a commit through the API may not land on unattended: the default branch, which
/// the command line cannot name (#6962). A future policy key, `guards.git.default_branches`,
/// with this as its default (Q307).
const DEFAULT_BRANCHES: [&str; 2] = ["main", "master"];

/// How a refused compound call is split into calls the floor can read (#6294).
const SPLIT: &str = "Split the call: give each read its method (`-X GET`) or its endpoint written \
     out, and run a write on its own, with its endpoint and method written out.";

/// The sentence a refusal of an alias ends with.
const ALIAS: &str = "An alias that would land code is the command it stands for.";

/// True when a word speaks of merging: a merge, a merge train, auto-merge, merge-when-ready.
pub(super) fn speaks_of_merging(word: &str) -> bool {
    word.to_lowercase().contains("merg")
}

/// A word the shell fills in or expands, so its value is not on the command line: a parameter or
/// command substitution, a glob, a brace expansion or a tilde.
pub(super) fn opaque(word: &str) -> bool {
    word.contains(['$', '`', '*', '?', '[', '~']) || brace_expansion_re().is_match(word)
}

/// Whether a word holds a brace the shell expands ([`brace_expansion_re`]).
pub(super) fn brace_expansion(word: &str) -> bool {
    brace_expansion_re().is_match(word)
}

/// `{a,b}` or `{1..3}`: a brace the shell expands. `{owner}` is a CLI placeholder, not one.
fn brace_expansion_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\{[^{}]*(,|\.\.)[^{}]*\}").expect("compiles"))
}

/// A placeholder a forge CLI fills in from the checkout, in an endpoint or a `-F` field: gh's
/// `{name}` (`{branch}`), or glab's `:name` as a path segment or a whole value (`:branch`,
/// `:id`, #7131).
fn placeholder_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\{[^{}]*\}|(?:^|/):[A-Za-z_]+(?:/|$)").expect("compiles"))
}

/// A forge CLI's own aliases of a verb the floor holds: `(cli, noun, alias, verb)`.
const VERB_ALIASES: &[(&str, &str, &str, &str)] = &[
    ("glab", "mr", "accept", "merge"),
    ("gh", "release", "new", "create"),
];

/// A forge CLI's spellings of a verb the floor already holds, mapped to that verb.
pub(super) fn canonical_verb<'a>(name: &str, noun: &str, verb: &'a str) -> &'a str {
    VERB_ALIASES
        .iter()
        .find(|(cli, n, alias, _)| *cli == name && *n == noun && *alias == verb)
        .map_or(verb, |row| row.3)
}

/// Commands, by their leading words, that the floor reads past the `PUBLISH_FORGE` table: the
/// raw API, alias-making, the commands that can print the forge token, and the verb that can
/// open a request with auto-merge on.
const HELD_BEYOND_THE_TABLE: &[(&str, &[&str])] = &[
    ("gh", &["api"]),
    ("gh", &["alias"]),
    ("gh", &["auth"]),
    ("gh", &["config", "get"]),
    ("gh", &["repo", "sync"]),
    ("glab", &["api"]),
    ("glab", &["alias"]),
    ("glab", &["auth"]),
    ("glab", &["config", "get"]),
    ("glab", &["mr", "create"]),
    ("glab", &["mr", "new"]),
];

/// Every held command of one CLI, by its leading words: the `PUBLISH_FORGE` rows and their
/// aliases, and [`HELD_BEYOND_THE_TABLE`]. An alias may not stand for one, or for its start.
pub(super) fn held_commands(name: &str) -> Vec<Vec<&'static str>> {
    let table = super::PUBLISH_FORGE
        .iter()
        .filter(|(cli, _, _)| *cli == name)
        .map(|(_, noun, verb)| vec![*noun, *verb]);
    let aliases = VERB_ALIASES
        .iter()
        .filter(|(cli, ..)| *cli == name)
        .map(|(_, noun, alias, _)| vec![*noun, *alias]);
    let beyond = HELD_BEYOND_THE_TABLE
        .iter()
        .filter(|(cli, _)| *cli == name)
        .map(|(_, words)| words.to_vec());
    table.chain(aliases).chain(beyond).collect()
}

/// The refusal for a call the floor cannot read.
fn unreadable() -> Option<String> {
    Some(UNREADABLE.to_owned())
}

/// The positional words of a forge CLI's argv, with the value of a repository flag dropped, so
/// the noun and verb are read where the CLI reads them. Answers each word with its index in
/// `args`.
pub(super) fn forge_words(args: &[String]) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "-R" || a == "--repo" {
            i += 2;
            continue;
        }
        if !a.starts_with('-') {
            out.push((i, a));
        }
        i += 1;
    }
    out
}

/// **A flag before the noun that may take a value** (#7423): `gh -b x pr merge 5` is `pr merge`
/// where `-b` takes `x`, and the CLI's root flags are not all known here, so `forge_words` reads
/// `x` as the noun. A flag that is not known to take none, standing before the first word, is
/// held when anything after it speaks of a held verb (a merge; with the floor's whole table,
/// a publish), so `gh -b x pr view 5` is left alone.
pub(super) fn unplaced_flag_reason(args: &[String]) -> Option<String> {
    let mut i = 0;
    let mut noun = false;
    while let Some(arg) = args.get(i) {
        if arg == "-R" || arg == "--repo" {
            i += 2;
            continue;
        }
        if !arg.starts_with('-') {
            // The noun; the verb is the next word that is no flag. `api` has no verb.
            if noun || arg == "api" {
                return None;
            }
            noun = true;
            i += 1;
            continue;
        }
        if !flag_before_command_is_known(arg) {
            let verbs: Vec<&str> = if super::only_merges() {
                super::MERGE_ROWS.iter().map(|row| row.2).collect()
            } else {
                super::PUBLISH_FORGE.iter().map(|row| row.2).collect()
            };
            let later_held = args[i + 1..].iter().any(|w| {
                let w = w.trim_matches(['\'', '"']).to_lowercase();
                speaks_of_merging(&w)
                    || matches!(w.as_str(), "api" | "alias" | "accept")
                    || verbs.contains(&w.as_str())
            });
            return later_held.then(|| {
                format!(
                    "{UNREADABLE} The flag `{arg}` stands before the command and may take the next word as its value, so purlis can't tell which command this is. Write the flags after the command."
                )
            });
        }
        i += 1;
    }
    None
}

/// The merge refusal for a `gh` or `glab` call, past the `PUBLISH_FORGE` table.
///
/// `cmd` is the whole command line, which a GraphQL query's quoting is read from.
pub(super) fn forge_reason(
    name: &str,
    args: &[String],
    cmd: &str,
    unattended: bool,
    depth: usize,
) -> Option<String> {
    let words = forge_words(args);
    let first = words.first().map(|w| w.1);
    let second = words.get(1).map(|w| w.1);
    match (first, second) {
        (Some("api"), _) => api_reason(&args[words[0].0 + 1..], cmd, name == "glab"),
        (Some("alias"), Some("import")) => unreadable(),
        (Some("alias"), Some("set")) => {
            alias_reason(name, &args[words[1].0 + 1..], unattended, depth)
        }
        (Some("repo"), Some("sync")) if name == "gh" => repo_sync_reason(&args[words[1].0 + 1..]),
        (Some("mr"), Some("create" | "new")) if name == "glab" && sets_auto_merge(args) => {
            Some(MERGES.to_owned())
        }
        _ => None,
    }
}

/// **A forced sync of the default branch** (Q330, #7131): gh's `repo sync --force` resets the
/// branch it syncs to the source's, the default branch where `--branch` names none, so a forced
/// sync of one of [`DEFAULT_BRANCHES`], or of a branch the shell fills in, moves the default
/// branch. An unforced sync only fast-forwards, and is left alone.
fn repo_sync_reason(args: &[String]) -> Option<String> {
    let mut forced = false;
    let mut branch: Option<&str> = None;
    let mut rest = args.iter();
    while let Some(a) = rest.next() {
        match a.as_str() {
            "--force" | "--force=true" => forced = true,
            "--force=false" => forced = false,
            "-b" | "--branch" => branch = rest.next().map(String::as_str),
            a => {
                if let Some(value) = a
                    .strip_prefix("--branch=")
                    .or_else(|| a.strip_prefix("-b").map(|v| v.trim_start_matches('=')))
                    .filter(|v| !v.is_empty())
                {
                    branch = Some(value);
                }
            }
        }
    }
    if !forced {
        return None;
    }
    match branch {
        None => Some(MOVES_THE_DEFAULT_BRANCH.to_owned()),
        Some(b) if opaque(b) => Some(REF_BRANCH_UNREADABLE.to_owned()),
        Some(b) => branch_in_path(b).map(str::to_owned),
    }
}

/// `glab mr create --auto-merge`, in any spelling that leaves it on.
fn sets_auto_merge(args: &[String]) -> bool {
    args.iter().any(|a| match a.strip_prefix("--auto-merge") {
        Some("") => true,
        Some(v) => v
            .strip_prefix('=')
            .is_some_and(|v| !matches!(v, "0" | "f" | "F" | "false" | "FALSE" | "False")),
        None => false,
    })
}

/// `gh alias set` / `glab alias set`: refused when what the alias stands for would be, or when
/// it stands for the start of a held command, whose rest the caller would add.
fn alias_reason(name: &str, args: &[String], unattended: bool, depth: usize) -> Option<String> {
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();
    let expansion = match (positional.first(), positional.get(1)) {
        (Some(_), Some(e)) => e.as_str(),
        // No expansion on the line, or `-` (stdin): it cannot be read.
        _ => return unreadable(),
    };
    // A shell alias runs a shell command this reader does not follow, and an expansion the shell
    // fills in cannot be read.
    let shell = args.iter().any(|a| a == "--shell" || a == "-s");
    if shell || expansion.starts_with('!') || expansion.starts_with(['$', '`']) {
        return unreadable();
    }
    let expanded: Vec<String> = shellseg::segment_argv(expansion)
        .into_iter()
        .next()
        .unwrap_or_default();
    let words: Vec<&str> = forge_words(&expanded).into_iter().map(|w| w.1).collect();
    // A noun or verb the caller supplies (`$1`) could be any command.
    if words.iter().take(2).any(|w| w.contains('$')) {
        return unreadable();
    }
    let held = !words.is_empty()
        && held_commands(name)
            .iter()
            .any(|held| held.starts_with(&words[..]) || words.starts_with(held));
    let cmd = format!("{name} {expansion}");
    if words.is_empty() || held || super::floor(&cmd, unattended, depth + 1).is_some() {
        Some(ALIAS.to_owned())
    } else {
        None
    }
}

/// `gh api` / `glab api` value flags, short and long.
///
/// `-R`/`--repo` names a repository, as it does on every other command of either CLI, which
/// [`forge_words`] already reads past; it sets neither a method nor a body.
const API_SHORT_VALUE: &[char] = &['X', 'H', 'f', 'F', 'q', 't', 'p', 'R'];
const API_SHORT_BOOL: &[char] = &['i', 'h'];
const API_LONG_VALUE: &[&str] = &[
    "method",
    "header",
    "raw-field",
    "field",
    "input",
    "jq",
    "template",
    "hostname",
    "preview",
    "cache",
    "output",
    "form",
    "repo",
];
const API_LONG_BOOL: &[&str] = &["paginate", "slurp", "include", "silent", "verbose", "help"];

/// How an `api` call's field is sent: `-f` as a string, `-F` with its type inferred (and `@` read
/// from a file).
#[derive(Clone, Copy, PartialEq, Eq)]
enum FieldKind {
    Raw,
    Typed,
}

/// One `-f`/`-F` field, `key=value` as written.
struct Field {
    kind: FieldKind,
    key: String,
    value: String,
}

impl Field {
    /// True when purlis cannot read this field's value from the line: the shell fills it in, or
    /// it is a `-F` field gh reads from a file (`@name`) or fills in with a placeholder such as
    /// `{branch}` (#6962).
    fn value_opaque(&self) -> bool {
        opaque(&self.value)
            || (self.kind == FieldKind::Typed
                && (self.value.starts_with('@') || placeholder_re().is_match(&self.value)))
    }

    fn new(kind: FieldKind, written: &str) -> Self {
        let (key, value) = written.split_once('=').unwrap_or((written, ""));
        Self {
            kind,
            key: key.to_owned(),
            value: value.to_owned(),
        }
    }
}

/// What one `api` call says about itself.
#[derive(Default)]
struct ApiCall {
    method: Option<String>,
    /// The first flag this reader does not know, as written.
    unknown_flag: Option<String>,
    positionals: Vec<String>,
    fields: Vec<Field>,
    /// `--input` or `--form`: a body this reader does not see on the line.
    opaque_body: bool,
    /// The files `--input` names, as written (a request edit's body is read from one).
    inputs: Vec<String>,
    /// `--form` was given: a multipart body that is never read.
    form_body: bool,
}

impl ApiCall {
    /// `glab_api` is true for `glab api`, which takes no `-R` (it fails on one): there the word
    /// after it is a positional, read as a path (`gh api -R` names a repository).
    fn parse(args: &[String], glab_api: bool) -> Self {
        let mut call = ApiCall::default();
        let mut i = 0;
        while i < args.len() {
            let a = args[i].as_str();
            i += 1;
            if a == "--" {
                call.positionals.extend(args[i..].iter().cloned());
                break;
            }
            if let Some(long) = a.strip_prefix("--") {
                let (name, inline) = match long.split_once('=') {
                    Some((n, v)) => (n, Some(v.to_owned())),
                    None => (long, None),
                };
                if API_LONG_VALUE.contains(&name) {
                    let value = inline.or_else(|| {
                        i += 1;
                        args.get(i - 1).cloned()
                    });
                    call.take(name, value);
                } else if !API_LONG_BOOL.contains(&name) {
                    call.unknown_flag.get_or_insert_with(|| a.to_owned());
                }
                continue;
            }
            if a.len() > 1 && a.starts_with('-') {
                let chars: Vec<char> = a.chars().skip(1).collect();
                for (k, c) in chars.iter().enumerate() {
                    if API_SHORT_BOOL.contains(c) {
                        continue;
                    }
                    if glab_api && *c == 'R' {
                        // glab refuses the flag outright, so the word after it is a path.
                        break;
                    }
                    if API_SHORT_VALUE.contains(c) {
                        let rest: String = chars[k + 1..].iter().collect();
                        let rest = rest.strip_prefix('=').unwrap_or(&rest).to_owned();
                        let value = if rest.is_empty() {
                            i += 1;
                            args.get(i - 1).cloned()
                        } else {
                            Some(rest)
                        };
                        let name = match c {
                            'X' => "method",
                            'f' => "raw-field",
                            'F' => "field",
                            _ => "other",
                        };
                        call.take(name, value);
                    } else {
                        call.unknown_flag.get_or_insert_with(|| a.to_owned());
                    }
                    break;
                }
                continue;
            }
            call.positionals.push(a.to_owned());
        }
        call
    }

    fn take(&mut self, name: &str, value: Option<String>) {
        let value = value.unwrap_or_default();
        match name {
            "method" => self.method = Some(value),
            "raw-field" => self.fields.push(Field::new(FieldKind::Raw, &value)),
            "field" => self.fields.push(Field::new(FieldKind::Typed, &value)),
            "input" => {
                self.opaque_body = true;
                self.inputs.push(value);
            }
            "form" => {
                self.opaque_body = true;
                self.form_body = true;
            }
            _ => {}
        }
    }

    /// True when the call is a read: an explicit `GET`/`HEAD`, or no method and no body, which
    /// both CLIs send as a `GET`. A flag this reader does not know could add a body, so a call
    /// carrying one is a read only when its method is spelled out: in both CLIs the method flag
    /// alone sets the method, and no other flag overrides it (#6294).
    fn is_read(&self) -> bool {
        match &self.method {
            Some(m) => m.eq_ignore_ascii_case("GET") || m.eq_ignore_ascii_case("HEAD"),
            None => self.unknown_flag.is_none() && self.fields.is_empty() && !self.opaque_body,
        }
    }

    /// Why a call that is not a read is not one, for a refusal that names it.
    fn why_not_a_read(&self) -> String {
        match (&self.method, &self.unknown_flag) {
            (Some(m), _) => format!("it is a `{m}`"),
            (None, Some(flag)) => {
                format!("`{flag}` is a flag purlis does not read, so it may not be a `GET`")
            }
            (None, None) if self.opaque_body => {
                "its body is read from a file purlis does not see".to_owned()
            }
            (None, None) => "its `-f`/`-F` fields make it a `POST`".to_owned(),
        }
    }
}

/// An endpoint as lower-case, percent-decoded path segments, with any scheme, host, query and
/// fragment taken off.
fn segments(endpoint: &str) -> Vec<String> {
    let path = endpoint.split(['?', '#']).next().unwrap_or("");
    let path = match path.split_once("://") {
        Some((_, rest)) => rest.split_once('/').map_or("", |(_, p)| p),
        None => path,
    };
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(|s| percent_decode(s).to_lowercase())
        .collect()
}

/// `%XX` decoded; anything that is not a valid escape is kept as written.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(b) = s
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Where a path's repository or project ends: the segments after `repos/<owner>/<name>`,
/// `repositories/<id>` or `projects/<id>`, or `None` when the path names none.
fn under_repository(segs: &[String]) -> Option<&[String]> {
    let at = segs
        .iter()
        .position(|s| matches!(s.as_str(), "repos" | "repositories" | "projects"))?;
    let names = if segs[at] == "repos" { 2 } else { 1 };
    Some(segs.get(at + 1 + names..).unwrap_or(&[]))
}

/// True when a path is a merge endpoint: a merge of a pull or merge request, a branch merge, or
/// a merge train, by the forges' own endpoint shapes. A ref, file or branch that is merely named
/// for a merge is not one. A path that names no repository is judged by any segment that speaks
/// of merging, since its shape is not one this reader knows.
fn is_merge_path(segs: &[String]) -> bool {
    let Some(rest) = under_repository(segs) else {
        return segs.iter().any(|s| speaks_of_merging(s));
    };
    match rest.first().map(String::as_str) {
        Some("pulls" | "merge_requests") => rest.get(2).is_some_and(|s| speaks_of_merging(s)),
        Some(collection) => speaks_of_merging(collection),
        None => false,
    }
}

/// Whether a path holds a `$name` or `${name}` reference, the one case the variable way out
/// (`J=101; ...`) is advice for: a bare `$`, a substitution or an arithmetic expansion is not.
fn names_a_variable(path: &str) -> bool {
    path.match_indices('$').any(|(i, _)| {
        path[i + 1..]
            .chars()
            .next()
            .is_some_and(|c| c == '{' || c == '_' || c.is_ascii_alphabetic())
    })
}

/// The sentence a refusal ends with where a writing call's path holds a variable that is not
/// bound to digits on the line (#7176; Q331).
const VARIABLE_WAY_OUT: &str = "purlis cannot know what a value it did not see holds, and a \
     value may carry a `/`, a `?` or a flag. Run each id written out literally, or put the call \
     alone on a line of exactly one of two shapes: `J=101; glab api ...` or \
     `for j in 101 102; do glab api ...; done`.";

/// Names bash and zsh set or read themselves, compared in lower case. A variable of one of
/// these is never a plain number the line gave it (#7176; Q331).
const SHELL_OWN_NAMES: &[&str] = &[
    "reply",
    "optarg",
    "optind",
    "opterr",
    "mapfile",
    "ifs",
    "pwd",
    "oldpwd",
    "path",
    "home",
    "random",
    "srandom",
    "lineno",
    "seconds",
    "pipestatus",
    "funcname",
    "groups",
    "hostname",
    "hosttype",
    "machtype",
    "ostype",
    "shell",
    "shlvl",
    "uid",
    "euid",
    "gid",
    "egid",
    "username",
    "ppid",
    "term",
    "user",
    "lang",
    "cdpath",
    "env",
    "ps0",
    "ps1",
    "ps2",
    "ps3",
    "ps4",
    "tmout",
    "tmpdir",
    "columns",
    "lines",
    "dirstack",
    "globignore",
    "shellopts",
    "posixly_correct",
    "timeformat",
    "fcedit",
    "epochseconds",
    "epochrealtime",
    "compreply",
    "mail",
    "mailpath",
    "mailcheck",
    "status",
    "argv",
    "fpath",
    "manpath",
    "module_path",
    "histchars",
    "prompt",
    "signals",
    "options",
    "commands",
    "functions",
    "aliases",
    "parameters",
    "words",
    "line",
    "cursor",
    "buffer",
];

/// Words that open or close a statement, which no variable may be named.
const SHELL_KEYWORDS: &[&str] = &[
    "if", "then", "else", "elif", "fi", "for", "in", "do", "done", "while", "until", "case",
    "esac", "function", "select", "time", "coproc",
];

fn is_shell_own_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    SHELL_OWN_NAMES.contains(&lower.as_str())
        || SHELL_KEYWORDS.contains(&lower.as_str())
        || [
            "bash", "zsh", "lc_", "ld_", "dyld_", "comp_", "hist", "xdg_", "gh_", "glab_",
            "gitlab_", "git_",
        ]
        .iter()
        .any(|p| lower.starts_with(p))
        || lower.ends_with("_proxy")
}

/// **A variable in a writing call's path is read only on a line of exactly one of two shapes**
/// (#7176; Q331 and its round-3 ruling: this guard is an early warning, the forge is the lock):
///
/// 1. `NAME=DIGITS; <one gh api or glab api call>`
/// 2. `for NAME in DIGITS [DIGITS...]; do <one gh api or glab api call>; done`, on one line or
///    the same three statements across newlines
///
/// with nothing else on the line. Every other line holds. The reader cannot know a value it did
/// not see, and bash writes variables in more ways than a regex can list (`read`, `declare`,
/// `printf -v`, `getopts`, `[[ =~ ]]`, `_`, `REPLY`, a quoted or escaped name, `source`, `eval`,
/// a `trap`, text that only looks like an assignment). A line that is nothing but one of the two
/// shapes has no room for any of them: the only statement that sets `NAME` is the one shown, and
/// the call after it holds no other variable, no `$(`, no backtick and no `eval`, `source`, `.`,
/// `trap` or `IFS`. Digits cannot re-route a call by themselves; what could was a value the line
/// does not show, or a setting that changes how digits split. The shapes close the first, and
/// no line of these shapes sets `IFS` (a line that says `IFS` anywhere holds), so a name that
/// holds digits is not split by anything the line did.
///
/// NAME is `[A-Z][A-Z0-9_]*` or `[a-z][a-z0-9_]*` with no quote or backslash, is none of the
/// shell's own variables or keywords ([`is_shell_own_name`]), and stands nowhere else in the call
/// but as `$NAME` or `${NAME}` (the only plain forms; `${NAME:-x}` or `${NAME/a/b}` is held),
/// each as a whole path segment: after a `/`, and before a `/`, a quote, a space or the end.
/// A `(` or a backtick anywhere in the call holds, as does a `$` that is not such a reference.
fn digit_bound(cmd: &str, name: &str) -> bool {
    let name_shape = |c: char, first: bool| {
        if first {
            c.is_ascii_alphabetic()
        } else {
            c.is_ascii_alphanumeric() || c == '_'
        }
    };
    let name_ok = name.chars().next().is_some_and(|c| name_shape(c, true))
        && name.chars().skip(1).all(|c| name_shape(c, false))
        && (name.chars().all(|c| !c.is_ascii_lowercase())
            || name.chars().all(|c| !c.is_ascii_uppercase()))
        && !is_shell_own_name(name);
    // Bash and zsh blank only space, tab and newline. Any other control character or any
    // non-ASCII character (U+00A0, U+0085, U+2028, `\v`, `\f`...) could make the first word
    // something other than the assignment the shapes show.
    if !name_ok
        || cmd.contains("IFS")
        || cmd
            .chars()
            .any(|c| !c.is_ascii() || (c.is_control() && c != '\t' && c != '\n'))
    {
        return false;
    }
    let esc = regex::escape(name);
    let one = Regex::new(&format!(
        r"^{esc}=[0-9]+(?:[ \t]*;[ \t]*|[ \t]*\n[ \t]*)([^;\n]+)$"
    ))
    .expect("compiles");
    let looped = Regex::new(&format!(
        r"^for[ \t]+{esc}[ \t]+in(?:(?:[ \t]+[0-9]+)+|([ \t]+{LOOP_SUBSTITUTION}))[ \t]*(?:;|\n)[ \t\n]*do[ \t\n]+([^;\n]+?)[ \t]*(?:;|\n)[ \t\n]*done$"
    ))
    .expect("compiles");
    let cmd = cmd.trim_matches([' ', '\t', '\n']);
    if let Some(c) = one.captures(cmd) {
        return c
            .get(1)
            .is_some_and(|call| lone_api_call(call.as_str(), name));
    }
    // The variable of a `for` over a list the shell makes (`for id in $(gh api … --jq
    // '.[].number')`) is one word with no space in it, whatever it holds: the shell splits the
    // list, and does not split the variable. It need not be digits, so only a call with no flag
    // is read as one: a call without `-X`, `-f` or `--input` cannot write (round 8).
    looped.captures(cmd).is_some_and(|c| {
        let call = c.get(2).map_or("", |m| m.as_str());
        (c.get(1).is_none() || !call.split_whitespace().any(|w| w.starts_with('-')))
            && lone_api_call(call, name)
    })
}

/// A `for` list made of one `gh api` or `glab api` call's substitution, of plain words and
/// single-quoted strings: no nesting, no double quote, no backtick, no `$`, no backslash. What it
/// runs is read as a command of its own. Any other command's output (`$(cat ids)`) stays held.
const LOOP_SUBSTITUTION: &str = r#"\$\((?:gh|glab)[ \t]+api[ \t](?:[^()'"`$\\\n]|'[^']*')*\)"#;

/// True when `call` is one `gh api` or `glab api` call whose only variable is `name`, as plain
/// whole-segment references (see [`digit_bound`]).
fn lone_api_call(call: &str, name: &str) -> bool {
    let call = call.trim_matches([' ', '\t']);
    let mut words = call.split_whitespace();
    if !matches!(
        (words.next(), words.next()),
        (Some("gh" | "glab"), Some("api"))
    ) {
        return false;
    }
    if words.any(|w| {
        matches!(
            w.trim_matches(['"', '\'']),
            "gh" | "glab" | "api" | "eval" | "source" | "." | "trap" | "exec"
        )
    }) {
        return false;
    }
    if call
        .chars()
        .any(|c| ";&|<>()`\\".contains(c) || (c.is_control() && c != '\t'))
    {
        return false;
    }
    // Quotes balance, so none reaches past the call.
    let (mut single, mut double) = (false, false);
    for c in call.chars() {
        match c {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            _ => {}
        }
    }
    if single || double {
        return false;
    }
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let braced = format!("{{{name}}}");
    let mut rest = String::new();
    let mut at = 0;
    while let Some(off) = call[at..].find('$') {
        let dollar = at + off;
        rest.push_str(&call[at..dollar]);
        let tail = &call[dollar + 1..];
        let len = if tail.starts_with(&braced) {
            braced.len()
        } else if tail.starts_with(name) && !tail[name.len()..].chars().next().is_some_and(is_word)
        {
            name.len()
        } else {
            return false;
        };
        let before = call[..dollar].chars().next_back();
        let after = tail[len..].chars().next();
        if before != Some('/') || !matches!(after, None | Some('/' | ' ' | '\t')) {
            return false;
        }
        rest.push(' ');
        at = dollar + 1 + len;
    }
    rest.push_str(&call[at..]);
    // The name stands nowhere else in the call.
    !rest.split(|c: char| !is_word(c)).any(|w| w == name)
}

/// The one name the line binds to digits ([`digit_bound`]), or `None`, found **once per line**:
/// a line has at most one, since the two shapes it may have bind one name and hold no other
/// variable. The last line read is kept, so a line of thousands of references costs one reading
/// of the line and a comparison per reference (#7423).
fn digit_bound_name(cmd: &str) -> Option<String> {
    use std::cell::RefCell;
    thread_local! {
        static LAST: RefCell<Option<(String, Option<String>)>> = const { RefCell::new(None) };
    }
    if let Some(name) = LAST.with(|last| {
        last.borrow()
            .as_ref()
            .filter(|(text, _)| text == cmd)
            .map(|(_, name)| name.clone())
    }) {
        return name;
    }
    static ONE: OnceLock<Regex> = OnceLock::new();
    static LOOPED: OnceLock<Regex> = OnceLock::new();
    let one = ONE.get_or_init(|| {
        Regex::new(r"^([A-Za-z_][A-Za-z0-9_]*)=[0-9]+(?:[ \t]*;[ \t]*|[ \t]*\n[ \t]*)[^;\n]+$")
            .expect("compiles")
    });
    let looped = LOOPED.get_or_init(|| {
        Regex::new(&format!(
            r"^for[ \t]+([A-Za-z_][A-Za-z0-9_]*)[ \t]+in(?:(?:[ \t]+[0-9]+)+|[ \t]+{LOOP_SUBSTITUTION})[ \t]*(?:;|\n)"
        ))
        .expect("compiles")
    });
    let trimmed = cmd.trim_matches([' ', '\t', '\n']);
    let name = one
        .captures(trimmed)
        .or_else(|| looped.captures(trimmed))
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
        .filter(|name| digit_bound(cmd, name))
        .map(str::to_owned);
    LAST.with(|last| *last.borrow_mut() = Some((cmd.to_owned(), name.clone())));
    name
}

/// `endpoint` with each `$name` or `${name}` that [`digit_bound`] says holds only digits written
/// as `fill`, so the rest of the reader sees the path the shell will make. A reference left in it
/// is one the line does not bind to digits.
fn with_digit_variables(endpoint: &str, bound: Option<&str>, fill: &str) -> String {
    if !endpoint.contains('$') || bound.is_none() {
        return endpoint.to_owned();
    }
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"\$(?:([A-Za-z_][A-Za-z0-9_]*)|\{([A-Za-z_][A-Za-z0-9_]*)\})")
            .expect("compiles")
    });
    re.replace_all(endpoint, |caps: &regex::Captures<'_>| {
        let name = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map_or("", |m| m.as_str());
        if bound == Some(name) {
            fill.to_owned()
        } else {
            caps[0].to_owned()
        }
    })
    .into_owned()
}

/// Whether a word's source spelling has an expansion outside single quotes: a `$` or a backtick.
/// Inside double quotes a `'` is a plain character and opens nothing (#7423).
fn expands(spelled: &str) -> bool {
    let mut chars = spelled.chars();
    let mut double = false;
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                chars.next();
            }
            '"' => double = !double,
            '\'' if !double => {
                if !chars.by_ref().any(|q| q == '\'') {
                    return true;
                }
            }
            '$' | '`' => return true,
            _ => {}
        }
    }
    false
}

/// `spelled` with each line continuation the shell deletes taken out: a backslash and a newline,
/// outside single quotes, where they are two plain characters. `"\<newline>$M"` is `"$M"`, so
/// its literal start is not the one it looks to have (#7423).
fn without_line_continuations(spelled: &str) -> String {
    let mut out = String::with_capacity(spelled.len());
    let mut chars = spelled.chars();
    let (mut single, mut double) = (false, false);
    while let Some(c) = chars.next() {
        match c {
            '\'' | '"' => {
                if c == '\'' && !double {
                    single = !single;
                } else if c == '"' && !single {
                    double = !double;
                }
                out.push(c);
            }
            '\\' if !single => match chars.next() {
                Some('\n') => {}
                Some(next) => {
                    out.push(c);
                    out.push(next);
                }
                None => out.push(c),
            },
            _ => out.push(c),
        }
    }
    out
}

/// The first `*`, `?` or `[` of a word's source spelling that stands outside quotes and is not
/// escaped, when the word has no other expansion: a word that is only a pattern.
fn unquoted_glob_char(spelled: &str) -> Option<char> {
    if expands(spelled) {
        return None;
    }
    let mut chars = spelled.chars();
    let (mut single, mut double) = (false, false);
    while let Some(c) = chars.next() {
        match c {
            '\\' if !single => {
                chars.next();
            }
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            '*' | '?' | '[' if !single && !double => return Some(c),
            _ => {}
        }
    }
    None
}

/// Where the construct that opens before `chars[i]` ends: the index past its `close`. It reads
/// the quotes inside it, so a `(` or a `"` in a quoted string is no bracket, and a nested `$(`,
/// `${` or backtick is read to its own end. `None` where it cannot be sure: no close, a comment
/// or a heredoc (which hide a `)`), `$'`, or a `case`, whose patterns end in a `)`.
fn closes_at(chars: &[char], mut i: usize, close: char) -> Option<usize> {
    let opener = if close == ')' { '(' } else { '{' };
    let mut depth = 0usize;
    while let Some(&c) = chars.get(i) {
        match c {
            '\\' => i += 2,
            '\'' if close != '"' => {
                i += 1 + chars.get(i + 1..)?.iter().position(|&q| q == '\'')? + 1;
            }
            '"' if close == '"' => return Some(i + 1),
            '"' => i = closes_at(chars, i + 1, '"')?,
            '`' => {
                i += 1 + chars.get(i + 1..)?.iter().position(|&q| q == '`')? + 1;
            }
            '$' => match chars.get(i + 1) {
                Some('(') => i = closes_at(chars, i + 2, ')')?,
                Some('{') => i = closes_at(chars, i + 2, '}')?,
                Some('\'') => return None,
                _ => i += 1,
            },
            '#' if close != '"' && (i == 0 || chars[i - 1].is_whitespace()) => return None,
            '<' if close == ')' && chars.get(i + 1) == Some(&'<') => return None,
            'c' if close == ')'
                && chars[i..].starts_with(&['c', 'a', 's', 'e', ' '])
                && (i == 0 || !chars[i - 1].is_alphanumeric()) =>
            {
                return None;
            }
            c if close != '"' && c == opener => {
                depth += 1;
                i += 1;
            }
            c if close != '"' && c == close => {
                if depth == 0 {
                    return Some(i + 1);
                }
                depth -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

/// Whether the inside of a double-quoted span holds only expansions that make one word: a plain
/// `$name`, `${name}` or `$(…)` (read to its true close, [`closes_at`]). A backtick, `$@`, `$*`,
/// a digit or special parameter, a `${…}` with an operator, flag or subscript, and a `$name[…]`
/// (zsh's subscript) are not.
fn plain_expansions_only(body: &[char]) -> bool {
    let is_name = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut i = 0;
    while i < body.len() {
        match body[i] {
            '\\' => i += 2,
            '`' => return false,
            '$' => match body.get(i + 1) {
                Some('(') => match closes_at(body, i + 2, ')') {
                    Some(end) => i = end,
                    None => return false,
                },
                Some('{') => {
                    let start = i + 2;
                    let len = body[start..].iter().take_while(|&&c| is_name(c)).count();
                    let named = body
                        .get(start)
                        .is_some_and(|c| c.is_ascii_alphabetic() || *c == '_');
                    if !named || body.get(start + len) != Some(&'}') {
                        return false;
                    }
                    i = start + len + 1;
                }
                Some(c) if c.is_ascii_alphabetic() || *c == '_' => {
                    let len = body[i + 1..].iter().take_while(|&&c| is_name(c)).count();
                    i += 1 + len;
                    if body.get(i) == Some(&'[') {
                        return false;
                    }
                }
                _ => return false,
            },
            _ => i += 1,
        }
    }
    true
}

/// **Whether the shell may make something other than one plain word of this one**: more words (a
/// split or a glob), or a word that begins with `-`. A word is let through when it holds no
/// expansion, glob or brace expansion outside single quotes, or when it is one literal start
/// and one double-quoted span, `"repos/o/r/pulls/$N"` or `title="$T"`, that holds only plain
/// expansions ([`plain_expansions_only`]): the literal start is no `-` and no expansion, and
/// nothing else is next to the quotes. A variable the line binds to digits is read as `0`.
///
/// `bound` is the name the line binds to digits ([`digit_bound_name`]), found once for the line
/// by the caller: a reference is a comparison to it, never a reading of the line (#7423).
fn filled_in(spelled: &str, bound: Option<&str>) -> bool {
    let spelled = with_digit_variables(spelled, bound, "0");
    let spelled = without_line_continuations(&spelled);
    let chars: Vec<char> = spelled.chars().collect();
    let mut bare = String::new();
    // Where the plain literal start of the word ends: its first quote or escape.
    let mut lead: Option<usize> = None;
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        if matches!(c, '\\' | '\'' | '"') && lead.is_none() {
            lead = Some(bare.chars().count());
        }
        match c {
            '\\' => i += 2,
            '\'' => match chars
                .get(i + 1..)
                .and_then(|r| r.iter().position(|&q| q == '\''))
            {
                Some(at) => i += at + 2,
                // A quote that never closes is no shell line: what is left is read as quoted.
                None => break,
            },
            '"' => {
                // One literal start (no quote, escape or expansion in it, no `-` to begin it)
                // and one span to the end of the word.
                let plain_start = bare.chars().count() == i && !chars[..i].contains(&'\\');
                let end = closes_at(&chars, i + 1, '"');
                if plain_start && end == Some(chars.len()) {
                    let body = &chars[i + 1..chars.len() - 1];
                    let first = if i > 0 {
                        Some(chars[0])
                    } else {
                        body.first().copied()
                    };
                    let literal_start = match first {
                        Some('\\') => body.get(1).copied().filter(|&c| c != '-'),
                        Some(c) => Some(c),
                        None => None,
                    };
                    return !(literal_start.is_some_and(|c| c != '-' && c != '$' && c != '`')
                        && plain_expansions_only(body)
                        && !brace_held(&bare, lead));
                }
                // A span with something else beside it: any expansion in it is not alone.
                match end {
                    Some(at) => {
                        if chars[i + 1..at - 1].iter().any(|&q| q == '$' || q == '`') {
                            return true;
                        }
                        i = at;
                    }
                    None => return chars[i + 1..].iter().any(|&q| q == '$' || q == '`'),
                }
            }
            '$' | '`' => return true,
            // A zsh glob qualifier (`a*(e:cmd:)`, `a*(+f)`) runs code, and `!(…)` and a leading
            // `^` are patterns that begin with no literal text. A leading `~` is tilde expansion
            // (`~-`, `~+`, `~name`), which can come out as a flag.
            '(' => return true,
            '^' | '~' if i == 0 => return true,
            // Every word a pattern makes begins with the literal text before it, so a pattern
            // after a plain start that is no `-` is another path, never a flag. A word that
            // begins with one, or has a quote or an escape before it, could be anything.
            '*' | '?' | '[' => {
                if bare.is_empty() || bare.starts_with('-') || lead.is_some() {
                    return true;
                }
                bare.push(c);
                i += 1;
            }
            _ => {
                bare.push(c);
                i += 1;
            }
        }
    }
    brace_held(&bare, lead)
}

/// Whether `bare`, the unquoted text of a word, holds a brace expansion that could begin the
/// word or a flag: one with no plain literal start before it, or after a `-` start. A brace after
/// plain text that is no `-` only makes more paths under that text (the same rule as a glob).
fn brace_held(bare: &str, lead: Option<usize>) -> bool {
    brace_expansion_re().find(bare).is_some_and(|m| {
        let at = bare[..m.start()].chars().count();
        at == 0 || lead.is_some_and(|lead| at >= lead) || bare.starts_with('-')
    })
}

/// The base name of a call's program if it is a forge CLI.
fn forge_program(prog: &str) -> Option<&'static str> {
    match shellwrap::base_lower(prog).as_str() {
        "gh" => Some("gh"),
        "glab" => Some("glab"),
        _ => None,
    }
}

/// What a refusal of a call the reader could not place says to do.
const PLACE_WAY_OUT: &str = "Write the program first, then `api`, with the flags after them.";

/// The flags a forge CLI may have before its command: the repository's, and help and version.
fn flag_before_command_is_known(flag: &str) -> bool {
    flag.starts_with("-R")
        || flag == "--repo"
        || flag.starts_with("--repo=")
        || matches!(flag, "-h" | "--help" | "--version")
}

/// Whether a word is one that makes a forge call after it a held one: a merge, or the words of
/// the commands the floor holds beyond the table.
pub(super) fn speaks_of_a_held_verb(word: &str) -> bool {
    speaks_of_merging(word)
        || matches!(
            word.to_lowercase().as_str(),
            "api" | "alias" | "accept" | "create" | "new" | "land"
        )
}

/// **A `gh api` / `glab api` call whose arguments the shell fills in** (#7416, #7423), read or
/// not. The reader cannot know what an unquoted `$…`, `$(…)`, backtick, glob or brace expansion
/// becomes: more words, one of them `-XPUT`. `gh api repos/o/r/pulls/5/merge $(cat n)` merges
/// when `n` holds `-XPUT`, and a merge request's title or a user's bio can hold it. So the rule
/// is not a list of the spellings that do: **every word of the call is held unless it is plainly
/// one word** ([`filled_in`]): no expansion, glob or brace outside single quotes, or one literal
/// start and one double-quoted span of plain `$name` and `$(…)` expansions. A variable the line
/// binds to digits, as the whole line's one statement, is digits.
///
/// The call is found as the segment's reading finds the program ([`shellwrap::split_env_chdir`]):
/// past a wrapper and its options, a definition's header, an assignment and a redirection, and
/// into what a `find -exec` runs. Where the program, or its command, cannot be placed for
/// certain, the call is held: a flag before `api`, a program that `xargs` hands words from its
/// stdin, words `env -S` unpacks, and a shell's string whose own words the outer shell fills in.
///
/// `own` and `source` are one segment's [`crate::shellseg::Reading::own`] and
/// [`crate::shellseg::Reading::own_source`]: the source has the quotes `own` has lost.
pub(super) fn shell_filled_api_reason(
    segment: &super::exact::Segment,
    cmd: &str,
) -> Option<String> {
    shell_filled_api_reason_of(&segment.words, &segment.spelled, cmd).or_else(|| {
        let also = segment.also.as_ref()?;
        shell_filled_api_reason_of(also, &[], cmd)
    })
}

/// [`shell_filled_api_reason`] of a segment's words, its redirections already taken out.
fn shell_filled_api_reason_of(own: &[String], source: &[String], cmd: &str) -> Option<String> {
    // The one name the line binds to digits, read at most once for the whole call.
    let bound: std::cell::OnceCell<Option<String>> = std::cell::OnceCell::new();
    let inv = shellwrap::split_env_chdir_exact(own);
    let (prog, argv) = (&inv.prog, &inv.argv);
    // What a `find` runs is a command of its own.
    for run in shellwrap::commands_a_find_runs(prog, argv) {
        let (inner, ..) = shellwrap::split_env(&run);
        if forge_program(&inner).is_some() && run.iter().any(|w| w.contains("{}")) {
            return Some(format!(
                "{UNREADABLE} `find` fills in a file name as an argument of `{inner}`, which could be a flag. {PLACE_WAY_OUT}"
            ));
        }
        if let Some(said) = shell_filled_api_reason(&super::exact::of_words(&run), cmd) {
            return Some(said);
        }
    }
    // A shell alias that stands for a forge command, or for the start of one, is the command it
    // stands for on a later line, whose words this reader does not join to it.
    if shellwrap::basename(prog) == "alias"
        && let Some(def) = argv.iter().skip(1).find(|def| {
            def.split_once('=').is_some_and(|(_, value)| {
                value
                    .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .any(|w| matches!(w, "gh" | "glab"))
            })
        })
    {
        return Some(format!(
            "{UNREADABLE} The alias `{def}` stands for a forge command, which purlis reads only where it is written out whole. {PLACE_WAY_OUT}"
        ));
    }
    // An inner shell's string is read as its own command, with the outer shell's quoting ignored:
    // what the outer shell fills in of it is unseen.
    let names_forge =
        |script: &str| shellwrap::may_name(script, "gh") || shellwrap::may_name(script, "glab");
    if (shellwrap::is_string_shell(prog) || shellwrap::basename(prog) == "eval")
        && shellwrap::shell_scripts_of(prog, argv)
            .iter()
            .any(|script| names_forge(script) && script.contains("api"))
    {
        let outer = own.iter().enumerate().find(|(i, word)| {
            let spelled = source
                .get(*i)
                .filter(|s| !s.is_empty())
                .map_or(word.as_str(), String::as_str);
            expands(spelled)
        });
        if let Some((_, word)) = outer {
            return Some(format!(
                "{UNREADABLE} The shell fills in part of the string `{word}` before the inner shell reads it, so purlis can't read the `api` call in it. Write the call out, or single-quote the string."
            ));
        }
    }
    // zsh expands a word that begins with `=` to the path of the program it names, before any
    // wrapper runs: `env =gh api …` and `sudo =gh api …` run `gh`, though a wrapper's own rule
    // reads `=gh` as an assignment (#7423). Read it as the program it is.
    let eq_word = inv.env.iter().chain(std::iter::once(prog)).find(|w| {
        w.strip_prefix('=')
            .is_some_and(|n| forge_program(n).is_some())
    });
    if let Some(word) = eq_word
        && let Some(k) = own.iter().position(|w| w == word)
    {
        let mut own = own.to_vec();
        own[k].remove(0);
        let mut source = source.to_vec();
        if let Some(spelled) = source.get_mut(k) {
            *spelled = spelled.replacen('=', "", 1);
        }
        return shell_filled_api_reason_of(&own, &source, cmd);
    }
    // A program the shell fills in (`$G`, `${GH:-gh}`, `"$(which gh)"`) could be a forge CLI: with
    // an `api` after it and a shell-filled argument, the call is held as any `gh api` one is. One
    // with no `api` after it (`$EDITOR file`) is out of this rule's reach.
    if forge_program(prog).is_none()
        && !(prog.contains(['$', '`']) && argv.iter().skip(1).any(|w| w == "api"))
    {
        return None;
    }
    // The words of a layer that hands the program more words are unseen.
    let fed_by = (0..inv.layers.len())
        .filter_map(|k| inv.layer_program(k))
        .map(shellwrap::base_lower)
        .find(|layer| layer == "xargs" || layer == "parallel");
    if let Some(layer) = fed_by {
        return Some(format!(
            "{UNREADABLE} `{layer}` hands `{prog}` words read from its input, which purlis cannot see. {PLACE_WAY_OUT}"
        ));
    }
    // The argv is the tail of the segment, unless a wrapper unpacked a string into it (`env -S`).
    let at = own.len().saturating_sub(argv.len());
    if argv.len() > own.len() || own[at..] != argv[..] {
        return Some(format!(
            "{UNREADABLE} The words of `{prog}` are unpacked from a string, which purlis cannot read. {PLACE_WAY_OUT}"
        ));
    }
    // The command is the first word that is no flag. A flag before it may take a value, so what
    // the CLI reads as its command cannot be placed.
    let mut j = 1;
    while let Some(word) = argv.get(j) {
        if !word.starts_with('-') {
            break;
        }
        // Only where `api` stands after it: a flag before any other command is read as that
        // command's own (`gh -c x commit`), and left to the rest of the reader.
        if !flag_before_command_is_known(word) && argv[j..].iter().any(|w| w == "api") {
            return Some(format!(
                "{UNREADABLE} The flag `{word}` stands before the command, so purlis can't tell which command this is. {PLACE_WAY_OUT}"
            ));
        }
        j += if word == "-R" || word == "--repo" {
            2
        } else {
            1
        };
    }
    if argv.get(j).map(String::as_str) != Some("api") {
        return None;
    }
    // A redirection's target is no argument, and [`plain_words`] took each unquoted one out with
    // its target: the shell opens it, or refuses it as ambiguous. A quoted operator is a word.
    for (i, word) in argv.iter().enumerate().skip(j + 1) {
        // A word the line did not measure (a line the lexer rejects, or what a `find` runs) has
        // lost its quotes: any expansion character in it is held, not skipped (#7423).
        let measured = source.get(at + i).filter(|s| !s.is_empty());
        let (held, glob) = match measured {
            Some(spelled) => {
                let bound = spelled
                    .contains('$')
                    .then(|| bound.get_or_init(|| digit_bound_name(cmd)).as_deref())
                    .flatten();
                (filled_in(spelled, bound), unquoted_glob_char(spelled))
            }
            None => (
                opaque(word),
                word.chars().find(|c| matches!(c, '*' | '?' | '[')),
            ),
        };
        if held {
            let cause = match glob {
                Some(c) if !word.contains(['$', '`']) && !brace_expansion(word) => format!(
                    "The argument `{word}` has a `{c}` that the shell reads as a pattern, and the word could come out as a flag. {GLOB_WAY_OUT}"
                ),
                _ => format!(
                    "The argument `{word}` is filled in by the shell, which may split it into words or begin a flag, such as the method. {FILLED_IN_WAY_OUT}"
                ),
            };
            return Some(format!("{UNREADABLE} {cause}"));
        }
    }
    None
}

/// What a refusal of an argument that is only a pattern says to do (#7423).
const GLOB_WAY_OUT: &str = "Quote the whole word, or start it with literal text that is no flag \
     (`repos/o/r/pulls?state=open`), so the shell leaves it one word that cannot begin a flag.";

/// What a refusal of a shell-filled `gh api` argument says to do.
const FILLED_IN_WAY_OUT: &str = "Write the endpoint out, or put the shell-filled part in one \
     double-quoted word that begins with literal text and holds only a plain `$name` or `$(…)` \
     (`\"repos/o/r/pulls/$N/comments\"`), so it is one word that cannot be a flag.";

/// True when a path is a repository or project itself, whose settings include auto-merge.
fn is_repository(segs: &[String]) -> bool {
    under_repository(segs).is_some_and(|rest| rest.is_empty())
}

/// The refusal for one `gh api` / `glab api` call, or `None` when it can neither merge nor
/// publish.
fn api_reason(args: &[String], cmd: &str, glab_api: bool) -> Option<String> {
    let call = ApiCall::parse(args, glab_api);
    // A word that opens with a brace expansion (or a flag with one) can make a flag or a method
    // out of one word (`{-XPUT,repos/o/r/pulls/5/merge}`), which `ApiCall::parse` reads as one
    // positional. A brace inside a path (`pulls/{1,2}`) only makes more paths, judged as written.
    if args
        .iter()
        .any(|a| brace_expansion(a) && a.starts_with(['-', '{']))
    {
        return Some(format!(
            "{UNREADABLE} An argument opens with a brace expansion, which the shell splits into words purlis does not see. {SPLIT}"
        ));
    }
    let read = call.is_read();
    if !read && call.positionals.is_empty() {
        return unreadable();
    }
    if let Some(method) = call.method.as_deref().filter(|m| !read && opaque(m)) {
        return Some(format!(
            "{UNREADABLE} The method `{method}` is filled in by the shell. {SPLIT}"
        ));
    }
    for endpoint in &call.positionals {
        let segs = segments(endpoint);
        if segs.iter().any(|s| s == "graphql") {
            if let Some(why) = graphql_reason(&call, cmd) {
                return Some(why.to_owned());
            }
            continue;
        }
        if read {
            continue;
        }
        // Every part of a writing call's path is written out, or a variable the line binds to
        // digits (#7176; Q331).
        // The path is judged with each bound reference as `0` and as nothing: an empty value
        // moves a segment next to its neighbour, and the line's own shape does not show which the
        // shell will make of a line it did not read (#7195).
        let bound = endpoint
            .contains('$')
            .then(|| digit_bound_name(cmd))
            .flatten();
        for fill in ["0", ""] {
            let resolved = with_digit_variables(endpoint, bound.as_deref(), fill);
            if opaque(&resolved) {
                let way_out = if names_a_variable(&resolved) {
                    format!(" {VARIABLE_WAY_OUT}")
                } else {
                    String::new()
                };
                return Some(format!(
                    "{UNREADABLE} The endpoint `{endpoint}` is filled in by the shell, and {}.{way_out} {SPLIT}",
                    call.why_not_a_read()
                ));
            }
            let segs = segments(&resolved);
            if segs.iter().any(|s| s == "." || s == "..") {
                return Some(format!(
                    "{UNREADABLE} The endpoint `{endpoint}` climbs with `.` or `..`. {SPLIT}"
                ));
            }
            if is_merge_path(&segs) {
                return Some(MERGES.to_owned());
            }
            // An organisation's ruleset guards its repositories' default branches as a
            // repository's does (Q330, #7131).
            if segs
                .windows(3)
                .any(|w| w[0] == "orgs" && w[2] == "rulesets")
            {
                return Some(REPLACES_THE_DEFAULT_BRANCH.to_owned());
            }
            // Moving the default branch's ref lands code as a merge does (Q330).
            if let Some(why) = default_branch_ref_reason(&segs, &call) {
                return Some(why.to_owned());
            }
            // A release or a tag is no merge: the merge hold leaves it to the floor (#6962).
            if !super::only_merges()
                && let Some(why) = publish_reason(&segs, &call)
            {
                return Some(why.to_owned());
            }
            // An edit of a request itself is held unless every field it sets is one of its
            // forge's settable ones (text, labels, assignees, state): a target (`base`,
            // `target_branch`), a merge setting, anything unknown, or a body purlis cannot read
            // is held (#7195).
            if let Some(forge) = request_edit(&segs)
                && let Some(why) = request_edit_reason(&call, forge, args, cmd)
            {
                return Some(why);
            }
            // Where a field the call does not show could set a merge: a repository's settings. A
            // request's own path (GitLab's `merge_requests/N`, GitHub's `pulls/N`) is judged above
            // by what its body sets, field by field; merging is `.../merge`, held above (#7176;
            // Q331).
            let settable = is_repository(&segs);
            for field in &call.fields {
                // A key the shell fills in could be an auto-merge setting where one can be set.
                if opaque(&field.key) && settable {
                    return unreadable();
                }
                if speaks_of_merging(&field.key) {
                    return Some(MERGES.to_owned());
                }
            }
            if call.opaque_body && settable {
                return unreadable();
            }
        }
    }
    None
}

/// Which forge's request a path is the edit endpoint of.
#[derive(Clone, Copy)]
enum RequestForge {
    /// GitHub's `pulls/<n>`.
    GitHub,
    /// GitLab's `merge_requests/<iid>`.
    GitLab,
}

impl RequestForge {
    /// The fields a request's edit may set without merging or retargeting it.
    fn settable(self) -> &'static [&'static str] {
        match self {
            Self::GitHub => &["title", "body", "state", "maintainer_can_modify"],
            Self::GitLab => &[
                "title",
                "description",
                "labels",
                "add_labels",
                "remove_labels",
                "assignee_id",
                "assignee_ids",
                "reviewer_ids",
                "milestone_id",
                "discussion_locked",
                "allow_collaboration",
                "state_event",
            ],
        }
    }
}

/// The forge whose request a path is the edit endpoint of: exactly `pulls/<n>` or
/// `merge_requests/<n>` under a repository. Merging is `<that>/merge`, judged elsewhere.
fn request_edit(segs: &[String]) -> Option<RequestForge> {
    match under_repository(segs)? {
        [kind, _] if kind == "pulls" => Some(RequestForge::GitHub),
        [kind, _] if kind == "merge_requests" => Some(RequestForge::GitLab),
        _ => None,
    }
}

/// The most of a request edit's body purlis reads: a larger one is held, not read in part.
const INPUT_LIMIT: u64 = 1 << 20;

/// The top-level keys of a JSON object, each with its value where it is a string; `None` for a
/// text that is not one object, or that names a key twice (which of two a reader keeps is its
/// own choice, so none is read).
fn json_entries(text: &str) -> Option<Vec<(String, Option<String>)>> {
    struct Pairs(Vec<(String, Option<String>)>);
    impl<'de> serde::Deserialize<'de> for Pairs {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct Walk;
            impl<'de> serde::de::Visitor<'de> for Walk {
                type Value = Pairs;
                fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str("a JSON object")
                }
                fn visit_map<A: serde::de::MapAccess<'de>>(
                    self,
                    mut map: A,
                ) -> Result<Pairs, A::Error> {
                    let mut out = Vec::new();
                    while let Some((key, value)) = map.next_entry::<String, serde_json::Value>()? {
                        out.push((key, value.as_str().map(str::to_owned)));
                    }
                    Ok(Pairs(out))
                }
            }
            d.deserialize_map(Walk)
        }
    }
    let Pairs(entries) = serde_json::from_str(text).ok()?;
    let mut keys: Vec<&String> = entries.iter().map(|(k, _)| k).collect();
    keys.sort();
    keys.windows(2).all(|w| w[0] != w[1]).then_some(entries)
}

/// The keys of a JSON object in the file a call's `--input` names, or `None` when they cannot be
/// read from here: a path the shell fills in, stdin (`-`, read by [`quoted_heredoc_body`]), a
/// relative path with no directory to read it against ([`super::in_directory`]; the line is one
/// sole call, so no `cd` came first), a file that is missing,
/// unreadable, over [`INPUT_LIMIT`] or not one JSON object with distinct keys.
fn input_entries(path: &str) -> Option<Vec<(String, Option<String>)>> {
    if path.is_empty() || path == "-" || opaque(path) {
        return None;
    }
    let file = std::path::Path::new(path);
    let file = if file.is_absolute() {
        file.to_path_buf()
    } else {
        // gh opens it in the directory the shell is in when it runs: the payload's, as the line
        // is one sole call.
        super::directory_the_line_runs_in()?.join(file)
    };
    let meta = std::fs::metadata(&file).ok()?;
    if !meta.is_file() || meta.len() > INPUT_LIMIT {
        return None;
    }
    let mut bytes = Vec::new();
    // One byte past the limit, so a file that grew since the check is told from one that fits.
    std::io::Read::read_to_end(
        &mut std::io::Read::take(std::fs::File::open(&file).ok()?, INPUT_LIMIT + 1),
        &mut bytes,
    )
    .ok()?;
    if bytes.len() as u64 > INPUT_LIMIT {
        return None;
    }
    json_entries(std::str::from_utf8(&bytes).ok()?)
}

/// The literal text of the quoted heredoc (`<<'EOF'`, `<<"EOF"`, `<<-'EOF'`) that is `args`'
/// stdin, or `None` when stdin is anything else: an unquoted heredoc, a pipe, a file, a
/// substitution, a descriptor other than 0, a line with more than one heredoc, a header line that
/// ends inside a quote or on a backslash, or a body that does not end the line's text. The line
/// is one sole call ([`super::is_sole_call`]), so the body is the lines [`heredoc_layout`] takes
/// for it after the command's own (#7195).
fn quoted_heredoc_body(args: &[String], cmd: &str) -> Option<String> {
    use crate::heredoc::{Line, body_extent, heredoc_layout, heredoc_openers};
    // The only redirect from a file is the heredoc: a later `< file` would be stdin instead.
    let redirects: Vec<&String> = args
        .iter()
        .filter(|a| {
            a.starts_with('<')
                || a.trim_start_matches(|c: char| c.is_ascii_digit())
                    .starts_with('<')
        })
        .collect();
    let [redirect] = redirects.as_slice() else {
        return None;
    };
    let digits = redirect
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    if !(digits.is_empty() || digits == "0") {
        return None;
    }
    let rest = &redirect[digits.len()..];
    if !rest.starts_with("<<") || rest.starts_with("<<<") {
        return None;
    }
    let line = Line::of(cmd);
    let openers = heredoc_openers(&line);
    let [opener] = openers.as_slice() else {
        return None;
    };
    // On descriptor 0: the tokenizer may drop an `fd` prefix, so it is read off the line.
    let before: Vec<char> = line.chars().get(..opener.start)?.to_vec();
    let digits: String = before
        .iter()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let word_start =
        before.len() == digits.len() || before[before.len() - digits.len() - 1].is_whitespace();
    if !digits.is_empty() && word_start && digits != "0" {
        return None;
    }
    // `{fd}<<'EOF'` opens the heredoc on a descriptor the shell picks, which is not stdin.
    let word: String = before
        .iter()
        .rev()
        .take_while(|c| !c.is_whitespace())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if word.starts_with('{') && word.ends_with('}') {
        return None;
    }
    let header = &opener.header;
    if header.expands || opener.maybe_shift || header.shells_disagree {
        return None;
    }
    let layout = heredoc_layout(cmd);
    let first = layout.first().filter(|l| !l.body)?;
    // The header line is the whole command line: it neither ends on a backslash nor inside a
    // quote (a body would start only where the command does), and every other line is a body's.
    if first.text.ends_with('\\')
        || !layout[1..]
            .iter()
            .all(|l| l.body || l.text.trim().is_empty())
    {
        return None;
    }
    let lines: Vec<&str> = layout[1..].iter().map(|l| l.text.as_str()).collect();
    let (len, found) = body_extent(&lines, &header.delim, false, header.dash);
    // The terminator ends the text, bar blank lines.
    if !found || !lines[len + 1..].iter().all(|l| l.trim().is_empty()) {
        return None;
    }
    let taken = &lines[..len];
    Some(if header.dash {
        taken
            .iter()
            .map(|l| l.trim_start_matches('\t'))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        taken.join("\n")
    })
}

/// The refusal of an edit of a request that sets a field outside `forge`'s settable ones, or
/// whose body cannot be read, or `None` when it sets only those (#7195). A field that speaks of
/// merging is the merge refusal; `base` or `target_branch` retargets the request.
fn request_edit_reason(
    call: &ApiCall,
    forge: RequestForge,
    args: &[String],
    cmd: &str,
) -> Option<String> {
    if call.is_read() {
        return None;
    }
    let mut sets: Vec<(String, Option<String>)> = call
        .fields
        .iter()
        // `assignee_ids[]=1` is how a form field sends a list: the field is `assignee_ids`.
        .map(|f| {
            let key = f.key.strip_suffix("[]").unwrap_or(&f.key);
            (key.to_owned(), (!f.value_opaque()).then(|| f.value.clone()))
        })
        .collect();
    let mut unread = call.form_body.then(|| "a `--form` body".to_owned());
    for input in &call.inputs {
        // A body is read only where the call is the line's sole command (#7195).
        let entries = if !super::bodies_are_readable() {
            None
        } else if input == "-" {
            quoted_heredoc_body(args, cmd).and_then(|body| json_entries(&body))
        } else {
            input_entries(input)
        };
        match entries {
            Some(more) => sets.extend(more),
            None => {
                unread.get_or_insert_with(|| format!("the body `{input}`"));
            }
        }
    }
    if sets.iter().any(|(k, _)| speaks_of_merging(k)) {
        return Some(MERGES.to_owned());
    }
    if let Some((key, _)) = sets
        .iter()
        .find(|(k, _)| !forge.settable().contains(&k.as_str()))
    {
        let shown = if opaque(key) {
            "a field the shell fills in"
        } else {
            key
        };
        return Some(format!(
            "Editing a request through the API is no merge when it sets only {}, but this call \
             sets `{shown}`, which can retarget it or change how it merges. Set the field in the \
             window, or run a call that sets only those.",
            forge.settable().join(", ")
        ));
    }
    // GitLab's `state_event` closes or reopens a request; any other value it takes is not read.
    if sets
        .iter()
        .any(|(k, v)| k == "state_event" && !matches!(v.as_deref(), Some("close" | "reopen")))
    {
        return Some(
            "Editing a request through the API is no merge when its `state_event` is `close` or \
             `reopen`, but purlis cannot read this call's value as one of those. Write the value \
             out."
                .to_owned(),
        );
    }
    // GitHub's `state` opens or closes a request; any other value it takes is not read.
    if matches!(forge, RequestForge::GitHub)
        && sets
            .iter()
            .any(|(k, v)| k == "state" && !matches!(v.as_deref(), Some("open" | "closed")))
    {
        return Some(
            "Editing a request through the API is no merge when its `state` is `open` or \
             `closed`, but purlis cannot read this call's value as one of those. Write the \
             value out."
                .to_owned(),
        );
    }
    unread.map(|what| {
        format!(
            "{UNREADABLE} This edits a request, and purlis cannot read {what}. A body is read \
             only where this call is the line's sole command. Run the call alone on its line, \
             with the body in a quoted heredoc (`--input - <<'EOF'`) or in a JSON file given by \
             its absolute path, or write the fields out with no variable in a name. {SPLIT}"
        )
    })
}

/// The sentence a refusal of a write to the default branch's ref ends with (Q330).
const MOVES_THE_DEFAULT_BRANCH: &str = "Updating, making or deleting the default branch's ref \
     through the API lands code as a merge does. Push a branch of its own instead.";

/// The sentence a refusal ends with where a ref write's branch cannot be read (Q330).
const REF_BRANCH_UNREADABLE: &str = "purlis can't read which branch this ref write moves, so it \
     is treated as a move of the default branch, not as a merge of a request. Write the ref out \
     literally, or commit on a branch folder and push it with `purlis worktree push`.";

/// **A write to the default branch's ref through the API** (Q330): a `PATCH`, `POST` or
/// `DELETE` on `git/refs/heads/<branch>` of one of [`DEFAULT_BRANCHES`], or a new ref
/// (`git/refs`) named `refs/heads/<branch>` of one, is the act a merge is, and held as one. A
/// branch the call does not let purlis read (a `ref` the shell or gh fills in, no `ref`, a body
/// purlis does not see) could be the default, so it fails closed. Any other branch is left
/// alone. The floor reads no forge, so the default branch is `main` or `master` here, the
/// forge's own answer unknown.
fn default_branch_ref_reason(segs: &[String], call: &ApiCall) -> Option<&'static str> {
    let rest: Vec<&str> = under_repository(segs)?.iter().map(String::as_str).collect();
    match rest[..] {
        // The ref as one path, whichever `/` was written as `%2F`: `heads%2Fmain` is `heads/main`.
        // A placeholder gh fills in from the checkout (`{branch}`) could be the default (#7131).
        ["git", "refs", ref path @ ..] if !path.is_empty() => {
            let path = path.join("/");
            if placeholder_re().is_match(&path) {
                return Some(REF_BRANCH_UNREADABLE);
            }
            let branch = path.strip_prefix("heads/")?;
            DEFAULT_BRANCHES
                .contains(&branch)
                .then_some(MOVES_THE_DEFAULT_BRANCH)
        }
        ["git", "refs"] => named_branch(call, "ref", "refs/heads/", MOVES_THE_DEFAULT_BRANCH),
        // GitHub's rename of a branch, and GitLab's delete of one (#7131).
        ["branches", branch, "rename"] => {
            branch_in_path(branch).map(|_| REPLACES_THE_DEFAULT_BRANCH)
        }
        ["repository", "branches", branch, ..] => branch_in_path(branch),
        // GitLab's new branch, and its commit through the API, name their branch in a field.
        ["repository", "branches"] => named_branch(call, "branch", "", MOVES_THE_DEFAULT_BRANCH),
        ["repository", "commits"] => named_branch(call, "branch", "", MOVES_THE_DEFAULT_BRANCH),
        // GitLab commits to the branch a field names through a file, a cherry-pick and a
        // revert too (#7131).
        ["repository", "files", ..] | ["repository", "commits", _, "cherry_pick" | "revert"] => {
            named_branch(call, "branch", "", MOVES_THE_DEFAULT_BRANCH)
        }
        // A ruleset guards what lands on a branch, as a protection does (Q330).
        ["rulesets", ..] => Some(REPLACES_THE_DEFAULT_BRANCH),
        // A branch's protection, on either forge, guards what lands on it.
        ["branches", _, "protection", ..] | ["protected_branches", ..] => {
            Some(REPLACES_THE_DEFAULT_BRANCH)
        }
        // Which branch is the default is a setting of the repository or project.
        [] if call.fields.iter().any(|f| f.key == "default_branch") => {
            Some(REPLACES_THE_DEFAULT_BRANCH)
        }
        _ => None,
    }
}

/// The sentence a refusal ends with where a write renames the default branch, changes which
/// branch is the default, or changes a branch's protection (#7131).
const REPLACES_THE_DEFAULT_BRANCH: &str = "Renaming the default branch, changing which branch is \
     the default, or changing a branch's protection or a ruleset through the API changes what \
     lands on it, as a merge does. Push a branch of its own instead.";

/// The refusal for a branch named in an endpoint's path: one of [`DEFAULT_BRANCHES`], or one gh
/// fills in from the checkout (`{branch}`), which could be.
fn branch_in_path(branch: &str) -> Option<&'static str> {
    if placeholder_re().is_match(branch) {
        Some(REF_BRANCH_UNREADABLE)
    } else if DEFAULT_BRANCHES.contains(&branch) {
        Some(MOVES_THE_DEFAULT_BRANCH)
    } else {
        None
    }
}

/// The refusal for a write that names its branch in the field `key`, as `<prefix><branch>`:
/// `default` where the branch is one of [`DEFAULT_BRANCHES`], and unreadable where the field is
/// missing, filled in, or in a body purlis does not see.
fn named_branch(
    call: &ApiCall,
    key: &str,
    prefix: &str,
    default: &'static str,
) -> Option<&'static str> {
    let named: Vec<&Field> = call.fields.iter().filter(|f| f.key == key).collect();
    if call.opaque_body || named.is_empty() || named.iter().any(|f| f.value_opaque()) {
        return Some(REF_BRANCH_UNREADABLE);
    }
    named
        .iter()
        .filter_map(|f| f.value.strip_prefix(prefix))
        .any(|branch| DEFAULT_BRANCHES.contains(&branch))
        .then_some(default)
}

/// The refusal for a write that would make a release or a tag, by the forges' own endpoint
/// shapes: a release or its assets, a tag (`repository/tags`, `git/tags`), a `refs/tags/` ref,
/// or a new ref whose name cannot be read. A path that names no repository is judged by any
/// segment that names releases or tags.
fn publish_reason(segs: &[String], call: &ApiCall) -> Option<&'static str> {
    let Some(rest) = under_repository(segs) else {
        return segs
            .iter()
            .any(|s| s == "releases" || s == "tags")
            .then_some(PUBLISHES);
    };
    let rest: Vec<&str> = rest.iter().map(String::as_str).collect();
    match rest[..] {
        ["releases", ..] | ["repository", "tags", ..] | ["git", "tags", ..] => Some(PUBLISHES),
        ["git", "refs", "tags", ..] => Some(PUBLISHES),
        ["git", "refs"] => {
            if call.opaque_body {
                return Some(REF_UNREADABLE);
            }
            let refs: Vec<&Field> = call.fields.iter().filter(|f| f.key == "ref").collect();
            if refs.is_empty() {
                return Some(REF_UNREADABLE);
            }
            for field in refs {
                if field.value_opaque() {
                    return Some(REF_UNREADABLE);
                }
                if field.value.starts_with("refs/tags/") {
                    return Some(PUBLISHES);
                }
            }
            None
        }
        _ => None,
    }
}

/// **A file committed through the API to the default branch** (#6962): a `PUT` (or `DELETE`)
/// through `gh api` on a repository's `contents/<path>` (`repos/<owner>/<name>/…` or
/// `repositories/<id>/…`, behind any host and prefix, as GHES's `api/v3`) with no `branch`
/// field, a branch the shell or gh fills in ([`Field::value_opaque`]), one of
/// [`DEFAULT_BRANCHES`], or a body purlis does not see. That commit
/// is a push to the default branch, and the whole refusal says so; one on a branch of its own
/// name is left alone. `None` for every other command.
pub(super) fn default_branch_push(args: &[String]) -> Option<String> {
    let words = forge_words(args);
    let (at, "api") = *words.first()? else {
        return None;
    };
    let call = ApiCall::parse(&args[at + 1..], false);
    let writes = call
        .method
        .as_deref()
        .is_some_and(|m| m.eq_ignore_ascii_case("PUT") || m.eq_ignore_ascii_case("DELETE"));
    if !writes {
        return None;
    }
    let commits = call.positionals.iter().any(|endpoint| {
        let segs = segments(endpoint);
        !opaque(endpoint)
            && under_repository(&segs)
                .is_some_and(|rest| rest.first().is_some_and(|s| s == "contents"))
    });
    if !commits {
        return None;
    }
    let branch = call.fields.iter().rev().find(|f| f.key == "branch");
    let onto = match branch {
        _ if call.opaque_body => "a branch purlis cannot read, from a body it does not see",
        None => "the default branch, since it names no `branch`",
        Some(b) if b.value_opaque() => "a branch the shell or gh fills in",
        Some(b) if DEFAULT_BRANCHES.contains(&b.value.as_str()) => "the default branch",
        Some(_) => return None,
    };
    Some(format!(
        "A commit through the API to the default branch is a push to it, and a run with nobody \
         watching does not push to the default branch. This call commits to {onto}. Commit on a \
         branch of its own and push it with `purlis worktree push`, or name that branch with \
         `-f branch=<name>`. Re-run this step **attended**, or have a person do it."
    ))
}

/// `$name` in a GraphQL document, with what stands before it and whether a `:` follows.
fn variable_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\S?)\s*\$([A-Za-z_][A-Za-z0-9_]*)(\s*:)?").expect("compiles"))
}

/// The refusal for a GraphQL call, or `None` when its query is readable and changes no merge.
fn graphql_reason(call: &ApiCall, cmd: &str) -> Option<&'static str> {
    if call.opaque_body {
        return Some(UNREADABLE);
    }
    let mut query = String::new();
    for field in &call.fields {
        if !field.key.eq_ignore_ascii_case("query") {
            continue;
        }
        if field.kind == FieldKind::Typed && field.value.starts_with('@') {
            return Some(UNREADABLE);
        }
        query.push_str(&field.value);
        query.push('\n');
    }
    if !query.contains('{') || query.contains("$(") || query.contains("${") || query.contains('`') {
        return Some(UNREADABLE);
    }
    // The lexer keeps a `\$` that bash would unescape inside double quotes; the document has `$`.
    let query = query.replace("\\$", "$");
    if query.contains('$')
        && (!variables_stand_in_value_positions(&query) || shell_expands_query(cmd))
    {
        return Some(UNREADABLE);
    }
    // The document is lexed, so a string and a `#` comment are both text, and only a
    // mutation's operation names are read (#7131). What does not lex is not read.
    let Some(toks) = graphql_tokens(&query) else {
        return Some(UNREADABLE);
    };
    let Some(operations) = mutation_operations(&toks) else {
        return Some(UNREADABLE);
    };
    if operations
        .iter()
        .any(|op| speaks_of_merging(op) || op.to_lowercase().contains("queue"))
    {
        return Some(MERGES);
    }
    if operations.iter().any(|op| touches_a_ref(op)) {
        return Some(MUTATES_A_REF);
    }
    // An input object that comes whole in a variable says nothing here of what it sets: the
    // branch a commit lands on, say (#7131).
    if !operations.is_empty() && declares_an_input_object(&toks) {
        return Some(UNREADABLE);
    }
    None
}

/// Whether a GraphQL document declares a variable of an input object's type, as
/// `$i: CommitCreateInput!` or `$i: [RefUpdateInput!]`.
fn declares_an_input_object(toks: &[Gql<'_>]) -> bool {
    toks.windows(4).any(|w| match w {
        [
            Gql::Punct(b'$'),
            Gql::Name(_),
            Gql::Punct(b':'),
            Gql::Name(ty),
        ]
        | [Gql::Punct(b':'), Gql::Punct(b'['), Gql::Name(ty), _] => ty.ends_with("Input"),
        _ => false,
    })
}

/// A GraphQL token, as the floor reads one: a name, a punctuator, a spread, or a value (a
/// string, a block string or a number) read only as a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gql<'a> {
    Name(&'a str),
    Punct(u8),
    Spread,
    Value,
}

/// A GraphQL document lexed as the GraphQL spec's lexical grammar reads it (#7131): the ignored
/// tokens (white space, line terminators, commas and `#` comments to the end of their line)
/// dropped, and each string, block string and number one [`Gql::Value`]. `None` where it does
/// not lex: a string or block string that does not close, a line break inside a string, or a
/// character no token starts with. A `"` in a comment opens nothing, and a `#` in a string is
/// text.
fn graphql_tokens(doc: &str) -> Option<Vec<Gql<'_>>> {
    let b = doc.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(&c) = b.get(i) {
        match c {
            b' ' | b'\t' | b'\n' | b'\r' | b',' => i += 1,
            0xEF if b[i..].starts_with(&[0xEF, 0xBB, 0xBF]) => i += 3,
            b'#' => {
                while b.get(i).is_some_and(|c| !matches!(c, b'\n' | b'\r')) {
                    i += 1;
                }
            }
            b'"' if b[i..].starts_with(b"\"\"\"") => {
                i += 3;
                loop {
                    let rest = b.get(i..).filter(|rest| !rest.is_empty())?;
                    if rest.starts_with(b"\\\"\"\"") {
                        i += 4;
                    } else if rest.starts_with(b"\"\"\"") {
                        i += 3;
                        break;
                    } else {
                        i += 1;
                    }
                }
                out.push(Gql::Value);
            }
            b'"' => {
                i += 1;
                loop {
                    match b.get(i)? {
                        b'\\' => i += 2,
                        b'"' => {
                            i += 1;
                            break;
                        }
                        b'\n' | b'\r' => return None,
                        _ => i += 1,
                    }
                }
                out.push(Gql::Value);
            }
            b'.' if b[i..].starts_with(b"...") => {
                out.push(Gql::Spread);
                i += 3;
            }
            b'!' | b'$' | b'&' | b'(' | b')' | b':' | b'=' | b'@' | b'[' | b']' | b'{' | b'|'
            | b'}' => {
                out.push(Gql::Punct(c));
                i += 1;
            }
            b'-' | b'0'..=b'9' => {
                i += 1;
                while b
                    .get(i)
                    .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'+' | b'-'))
                {
                    i += 1;
                }
                out.push(Gql::Value);
            }
            c if c == b'_' || c.is_ascii_alphabetic() => {
                let start = i;
                while b
                    .get(i)
                    .is_some_and(|c| *c == b'_' || c.is_ascii_alphanumeric())
                {
                    i += 1;
                }
                out.push(Gql::Name(&doc[start..i]));
            }
            _ => return None,
        }
    }
    Some(out)
}

/// The index of the token that closes the `open` at `at`, counting only `open` and `close`.
fn gql_close(toks: &[Gql<'_>], at: usize, open: u8, close: u8) -> Option<usize> {
    let mut depth = 0usize;
    for (j, t) in toks.iter().enumerate().skip(at) {
        match t {
            Gql::Punct(c) if *c == open => depth += 1,
            Gql::Punct(c) if *c == close => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
    }
    None
}

/// **The operations a GraphQL document's mutations call** (#7131): the fields each `mutation`'s
/// top selection set selects, read past an alias (`m: mergePullRequest`), its arguments,
/// directives and sub-selections. A field name deeper in (`baseRefName`) is no operation.
/// `None` where the document's structure is not read whole: a definition that opens with no
/// operation's keyword, `fragment` or `{`, a selection set that does not close, or a fragment a
/// mutation spreads, whose fields could be any operation.
fn mutation_operations<'a>(toks: &[Gql<'a>]) -> Option<Vec<&'a str>> {
    let mut ops = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        // A definition opens with an operation's keyword, a fragment's or a selection set; any
        // other (a description string, a type system definition) is not read (#7131).
        if !matches!(
            toks[i],
            Gql::Name("query" | "mutation" | "subscription" | "fragment") | Gql::Punct(b'{')
        ) {
            return None;
        }
        let mutation = toks[i] == Gql::Name("mutation");
        // A definition runs to its selection set, past a variable list's default objects.
        let mut parens = 0usize;
        while let Some(t) = toks.get(i) {
            match t {
                Gql::Punct(b'(') => parens += 1,
                Gql::Punct(b')') => parens = parens.checked_sub(1)?,
                Gql::Punct(b'{') if parens == 0 => break,
                _ => {}
            }
            i += 1;
        }
        if i == toks.len() {
            return (!mutation).then_some(ops);
        }
        let close = gql_close(toks, i, b'{', b'}')?;
        if mutation {
            let set = &toks[i + 1..close];
            let mut k = 0;
            while let Some(t) = set.get(k) {
                match t {
                    Gql::Name(name) => {
                        if let (Some(Gql::Punct(b':')), Some(Gql::Name(field))) =
                            (set.get(k + 1), set.get(k + 2))
                        {
                            ops.push(*field);
                            k += 3;
                        } else {
                            ops.push(*name);
                            k += 1;
                        }
                    }
                    Gql::Punct(b'@') => k += 2,
                    Gql::Punct(b'(') => k = gql_close(set, k, b'(', b')')? + 1,
                    Gql::Punct(b'{') => k = gql_close(set, k, b'{', b'}')? + 1,
                    Gql::Spread => return None,
                    _ => k += 1,
                }
            }
        }
        i = close + 1;
    }
    Some(ops)
}

/// The sentence a refusal of a GraphQL mutation that touches a ref, a branch or a tag ends with
/// (#7131).
const MUTATES_A_REF: &str = "A GraphQL mutation that touches a ref, a branch or a tag can move \
     the default branch or publish a tag, and purlis does not read which.";

/// True when a mutation's operation name, read as camel-case or snake-case words, names a ref,
/// a branch, a tag, a ruleset or a commit: `updateRef`, `createCommitOnBranch`,
/// `deleteBranchProtectionRule`, `tagCreate`, `commitCreate` (#7131). Only an operation's name
/// is read, so `createPullRequest`'s `baseRefName` names none.
fn touches_a_ref(operation: &str) -> bool {
    camel_words(operation).iter().any(|w| {
        matches!(
            w.as_str(),
            "ref"
                | "refs"
                | "branch"
                | "branches"
                | "tag"
                | "tags"
                | "ruleset"
                | "rulesets"
                | "commit"
                | "commits"
        )
    })
}

/// A name's words, lower-cased: split at `_` and before each upper-case letter that follows a
/// lower-case one or a digit.
fn camel_words(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut prev_lower = false;
    for c in name.chars() {
        if (c == '_' || (c.is_ascii_uppercase() && prev_lower)) && !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
        prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        if c != '_' {
            word.push(c.to_ascii_lowercase());
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

/// True when every `$` in a GraphQL document is a variable the document declares, declared in its
/// operation's variable list and used only where a value goes. Anything else is text the shell
/// could have put there.
fn variables_stand_in_value_positions(query: &str) -> bool {
    let mut declared = Vec::new();
    let mut used = Vec::new();
    let mut seen = 0;
    for m in variable_re().captures_iter(query) {
        seen += 1;
        let before = m.get(1).map_or("", |b| b.as_str());
        let name = m.get(2).map_or("", |n| n.as_str());
        if m.get(3).is_some() {
            if !matches!(before, "(" | ",") {
                return false;
            }
            declared.push(name);
        } else {
            if !matches!(before, ":" | "[" | ",") {
                return false;
            }
            used.push(name);
        }
    }
    seen == query.matches('$').count() && used.iter().all(|u| declared.contains(u))
}

/// True when a `$` in a `query=` word of the command line is one the shell expands: written
/// outside single quotes and not escaped. A line the lexer cannot read counts as one.
fn shell_expands_query(cmd: &str) -> bool {
    let Ok(toks) = shellseg::lex(cmd) else {
        return true;
    };
    let chars: Vec<char> = cmd.chars().collect();
    toks.iter()
        .filter(|t| t.text.to_lowercase().contains("query=") && t.text.contains('$'))
        .any(|t| {
            let (Ok(start), Ok(end)) = (usize::try_from(t.start), usize::try_from(t.end)) else {
                return true;
            };
            expanding_dollar(chars.get(start..end).unwrap_or(&[]))
        })
}

/// True when the raw text of one word holds a `$` the shell expands.
fn expanding_dollar(raw: &[char]) -> bool {
    let mut single = false;
    let mut double = false;
    let mut i = 0;
    while i < raw.len() {
        let c = raw[i];
        if single {
            single = c != '\'';
        } else if c == '\\' {
            i += 1;
        } else if c == '\'' && !double {
            single = true;
        } else if c == '"' {
            double = !double;
        } else if c == '$' {
            return true;
        }
        i += 1;
    }
    false
}

/// True when a push option sets a request to merge: its key, past GitLab's `merge_request.`
/// namespace, speaks of merging.
fn is_merge_push_option(opt: &str) -> bool {
    let key = opt.split('=').next().unwrap_or("").to_lowercase();
    let key = key
        .strip_prefix("merge_request.")
        .or_else(|| key.strip_prefix("mr."))
        .unwrap_or(&key);
    speaks_of_merging(key)
}

/// The refusal for one push option. Its key decides, since a value such as a request's title
/// sets nothing; a key the shell fills in, or a value a substitution writes, cannot be read.
fn push_option_reason(opt: &str) -> Option<&'static str> {
    // A substitution in the value runs a command whose output is the option.
    if opaque(opt.split('=').next().unwrap_or("")) || opt.contains("$(") || opt.contains('`') {
        Some(UNREADABLE)
    } else if is_merge_push_option(opt) {
        Some(MERGES)
    } else {
        None
    }
}

/// Shell-quotes one word, so a resolved alias is handed back as the words it was.
fn quoted(word: &str) -> String {
    format!("'{}'", word.replace('\'', r"'\''"))
}

/// What a git segment's subcommand is, when it may be an alias defined on the line.
pub(super) enum GitAlias {
    /// No alias defined on the line is run here.
    None,
    /// The alias (`-c alias.<name>=…`) resolved to the command it stands for.
    Resolved(String),
    /// An alias this reader cannot follow: a shell alias, an expansion the shell fills in, or
    /// one taken from the environment (`GIT_CONFIG_*`, `--config-env`).
    Unreadable,
}

/// The git alias this segment runs, if one is defined on the line.
pub(super) fn git_alias(args: &[String], env: &[String], sub: Option<&str>) -> GitAlias {
    let Some(sub) = sub else {
        return GitAlias::None;
    };
    if env.iter().any(|e| {
        e.split_once('=').is_some_and(|(k, v)| {
            k.starts_with("GIT_CONFIG") && v.to_lowercase().starts_with("alias.")
        })
    }) {
        return GitAlias::Unreadable;
    }
    let Some(sub_at) = args.iter().position(|a| a == sub) else {
        return GitAlias::None;
    };
    let globals = &args[..sub_at];
    for (i, a) in globals.iter().enumerate() {
        let config_env = match a.strip_prefix("--config-env=") {
            Some(v) => Some(v),
            None if a == "--config-env" => globals.get(i + 1).map(String::as_str),
            None => None,
        };
        if config_env.is_some_and(|v| v.to_lowercase().starts_with("alias.")) {
            return GitAlias::Unreadable;
        }
    }
    let mut i = 0;
    while i + 1 < globals.len() {
        if globals[i] == "-c"
            && let Some((key, expansion)) = globals[i + 1].split_once('=')
            && let Some(name) = key.to_lowercase().strip_prefix("alias.")
            && name == sub.to_lowercase()
        {
            if expansion.starts_with('!') || opaque(expansion) {
                return GitAlias::Unreadable;
            }
            let mut words: Vec<String> = Vec::new();
            words.extend(globals[..i].iter().map(|w| quoted(w)));
            words.extend(globals[i + 2..].iter().map(|w| quoted(w)));
            words.push(expansion.to_owned());
            words.extend(args[sub_at + 1..].iter().map(|w| quoted(w)));
            return GitAlias::Resolved(format!("git {}", words.join(" ")));
        }
        i += 1;
    }
    GitAlias::None
}

/// The merge refusal for a `git` call: a push option that sets auto-merge, given on the command
/// line, through `-c`, through the `GIT_CONFIG_*` environment, or written into git's config; and
/// a git alias written into git's config that would publish.
pub(super) fn git_reason(
    args: &[String],
    env: &[String],
    sub: Option<&str>,
    unattended: bool,
    depth: usize,
) -> Option<String> {
    let sub_at = sub.and_then(|s| args.iter().position(|a| a == s));
    let globals = &args[..sub_at.unwrap_or(args.len())];
    let rest = sub_at.map_or(&[][..], |i| &args[i + 1..]);
    match sub {
        Some("push") => {
            let mut i = 0;
            while i < globals.len() {
                let a = globals[i].as_str();
                if a == "-c"
                    && let Some(kv) = globals.get(i + 1)
                {
                    if let Some((k, v)) = kv.split_once('=')
                        && k.eq_ignore_ascii_case("push.pushoption")
                        && let Some(why) = push_option_reason(v)
                    {
                        return Some(why.to_owned());
                    }
                    i += 2;
                    continue;
                }
                // `--config-env` takes the value from a variable this reader does not see.
                let config_env = match a.strip_prefix("--config-env=") {
                    Some(v) => Some(v),
                    None if a == "--config-env" => globals.get(i + 1).map(String::as_str),
                    None => None,
                };
                if config_env.is_some_and(|v| v.to_lowercase().starts_with("push.pushoption")) {
                    return unreadable();
                }
                i += 1;
            }
            if let Some(why) = env_reason(env) {
                return Some(why.to_owned());
            }
            push_options(rest)
                .iter()
                .find_map(|o| push_option_reason(o))
                .map(str::to_owned)
        }
        Some("config") => config_write_reason(rest, unattended, depth),
        _ => None,
    }
}

/// The git subcommands an alias may not stand for or begin with, because the floor holds them.
const HELD_GIT: &[&str] = &["push", "tag"];

/// `git config` writing a push option that sets auto-merge, or an alias that would publish.
fn config_write_reason(rest: &[String], unattended: bool, depth: usize) -> Option<String> {
    let key_at = rest.iter().position(|a| {
        let a = a.to_lowercase();
        a == "push.pushoption" || a.starts_with("alias.")
    })?;
    // The value is the word after the key, even one that starts like an option: an alias's
    // expansion may begin with git's own global options.
    let value = rest.get(key_at + 1)?;
    if rest[key_at].eq_ignore_ascii_case("push.pushoption") {
        return push_option_reason(value).map(str::to_owned);
    }
    if value.starts_with('!') || opaque(value) {
        return unreadable();
    }
    // The subcommand the alias runs, past any global options it carries.
    let words: Vec<String> = value.split_whitespace().map(str::to_owned).collect();
    let sub = crate::credguard::git_subcommand(&words).unwrap_or_default();
    let cmd = format!("git {value}");
    if HELD_GIT.contains(&sub.to_lowercase().as_str())
        || super::floor(&cmd, unattended, depth + 1).is_some()
    {
        Some(ALIAS.to_owned())
    } else {
        None
    }
}

/// `GIT_CONFIG_*` assignments in front of a push that name `push.pushOption`.
fn env_reason(env: &[String]) -> Option<&'static str> {
    let config: Vec<&str> = env
        .iter()
        .filter_map(|e| e.split_once('='))
        .filter(|(k, _)| k.starts_with("GIT_CONFIG"))
        .map(|(_, v)| v)
        .collect();
    if !config
        .iter()
        .any(|v| v.to_lowercase().contains("pushoption"))
    {
        return None;
    }
    for v in config {
        if opaque(v) {
            return Some(UNREADABLE);
        }
        let lower = v
            .to_lowercase()
            .replace("merge_requests", "")
            .replace("merge_request", "");
        if speaks_of_merging(&lower) {
            return Some(MERGES);
        }
    }
    None
}

/// Every push option on a `git push` argv: `-o x`, `-ox`, `-uo x`, `--push-option[=]x`.
fn push_options(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        i += 1;
        if a == "--" {
            break;
        }
        if let Some(v) = a.strip_prefix("--push-option=") {
            out.push(v.to_owned());
        } else if a == "--push-option" {
            if let Some(v) = args.get(i) {
                out.push(v.clone());
                i += 1;
            }
        } else if !a.starts_with("--")
            && a.len() > 1
            && let Some(cluster) = a.strip_prefix('-')
            && let Some((_, v)) = cluster.split_once('o')
        {
            if v.is_empty() {
                if let Some(v) = args.get(i) {
                    out.push(v.clone());
                    i += 1;
                }
            } else {
                out.push(v.to_owned());
            }
        }
    }
    out
}
