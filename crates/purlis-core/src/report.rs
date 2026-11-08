// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter report`: a bug or a feature request, drafted here and filed on charter's own
//! tracker under the reporter's own account (#363), through the forge seam's work-item area
//! (#806).
//!
//! # The interface: evidence from a source in, an outcome out
//!
//! Every report passes through [`Reports`] (#5686, spec #5683). A caller hands it [`Evidence`]
//! and the source it came from (a [`Caller`]), and reads back an [`Outcome`]. The caller never
//! decides how a report is scrubbed, where it goes or whether it may go: [`Reports::take`] drafts
//! and names the route, [`Reports::file`] files only the draft whose digest was shown, and
//! [`Reports::duplicates`] is the one search sent before a yes. The evidence kinds not built yet
//! are named and hold [`NotYet`], which has no value, so nothing can hand one in.
//!
//! # What this module promises
//!
//! **Nothing leaves before it has been shown.** A [`Draft`] is built, scrubbed and rendered
//! ([`Draft::preview`]) with no network at all. The one thing sent before a yes is the
//! duplicate search's query ([`Draft::query`]): words of the scrubbed title, printed first. Filing it ([`Reports::file`]) is a separate call, and the
//! command line reaches it only with the reporter's "yes": a prompt on a terminal, or
//! `--yes <digest>` where the digest ([`Draft::digest`]) is the one the preview printed. A digest
//! is over the repository, the title and the body, so a `--yes` can only file the exact bytes
//! that were on screen: change one character and the digest no longer matches, and nothing is
//! sent. That is the answer to charter-plane ADR 0003's objection to a `--yes` flag (*"a flag the
//! agent can pass is a flag the agent will pass unprompted"*): the flag cannot be passed without
//! first printing the very text it would publish.
//!
//! **The reporter's own identity, never a shared token** (charter-plane ADR 0001, carried over by
//! the operator's ruling on #363). The search and the filing are [`WorkItems::search`] and
//! [`WorkItems::create`] on the tracker's forge, through a backend
//! [`forge::signin::signed_in_backend`] hands out: only the person in the window, with purlis's
//! own sign-in to GitHub, and never through `gh`, whose login may be anybody's and which prefers
//! a token in the environment to it. A `purlis` command and a chat hold no sign-in (ADR 0070
//! ruling 5), so from a terminal a report is drafted and shown, and filed by the person: in the
//! app, or through the prefilled link ([`Draft::fallback_url`]).
//!
//! **Scrubbed before it is shown** ([`Known::scrub`]), by what charter can positively identify:
//! a line [`crate::secretshape`] reads as a credential, the value of any variable in this
//! process's environment (which is where a vault's values are when a chat holds them), the
//! plane's path and home-directory paths, and the names of this plane's workspaces, personas,
//! clones and vaults. Each removal leaves a visible placeholder, and the preview names the
//! categories, because the reporter's read is the other half of the scrub: nothing mechanical
//! catches "our billing service cannot do X".
//!
//! # A saved panic becomes a draft bug
//!
//! The app appends every panic to `panics.log` in its log directory (`app/src-tauri/src/panics.rs`).
//! [`latest_panic`] reads the newest block back, and [`Evidence::Panic`] keeps a closed set of
//! its fields: where it panicked, its message (scrubbed, and flagged as free text), and the
//! charter version the record names. The thread and the backtrace are dropped — a backtrace
//! carries the reporter's paths, and without symbols it is only addresses.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use regex::Regex;
use sha2::{Digest, Sha256};

use crate::forge::backend::{Caller, NewWorkItem, WorkItems};
use crate::forge::{self, Forge, ForgeError, Kind as ForgeKind};

mod consent;
mod crash;
pub mod native;
pub use consent::{Consent, Stream};
pub use native::NativeCrash;
pub mod intake;
mod sent;
pub use sent::{Sent, SentList, SentTo};

/// The repository reports are filed on (RN-9: the product's repository since 2026-10-05).
pub const UPSTREAM: &str = "purlis/purlis";

/// The address [`UPSTREAM`] had before the repository moved (V92). GitHub redirects it, so it
/// is still the product's own address, and it is never reused for anything else.
pub const UPSTREAM_BEFORE: &str = "diazoxide/charter";

/// The most characters a report body may hold. GitHub refuses a body over 65,536.
pub const BODY_MAX: usize = 60_000;

/// The longest prefilled `issues/new` link charter prints. Browsers and GitHub start refusing
/// links not far beyond this.
pub const LINK_MAX: usize = 8_000;

/// The longest an issue title may be, ellipsis included: the maintainer's issue list is
/// where a title that wraps stops being scannable.
const TITLE_MAX: usize = 72;

/// Below this index a word break throws away more than a tidy ending is worth.
const BREAK_FLOOR: usize = 40;

/// The shortest environment value that is scrubbed. Shorter ones (`1`, `true`, `zsh`) are
/// ordinary words far more often than they are anybody's secret.
const ENV_VALUE_MIN: usize = 8;

/// Variables whose values name a terminal, a locale or a standard program, never a person or a
/// place, so a report about a terminal can still say which one. `OLDPWD` and `PWD` are not
/// here: they are directories.
const ENV_HARMLESS: [&str; 12] = [
    "TERM",
    "COLORTERM",
    "TERM_PROGRAM",
    "TERM_PROGRAM_VERSION",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "SHLVL",
    "SHELL",
    "EDITOR",
    "VISUAL",
    "PAGER",
];

/// Names that are charter's own words rather than the reporter's: a plane that clones charter,
/// or keeps the default steward, would otherwise lose "charter" from every sentence about it.
const OWN_WORDS: [&str; 4] = ["charter", "charter-app", "charter-plane", "steward"];

/// The words the scrub's own placeholders are made of. A workspace called `env` or `home`
/// would otherwise rewrite `[env $X]` into `[[workspace] $X]`, and a name that plain identifies
/// nobody.
const PLACEHOLDER_WORDS: [&str; 8] = [
    "env",
    "home",
    "path",
    "plane",
    "repo",
    "vault",
    "workspace",
    "persona",
];

/// What a report is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// charter did something wrong.
    Bug,
    /// charter cannot do something it should.
    Feature,
}

impl Kind {
    /// The word the preview uses.
    pub fn word(self) -> &'static str {
        match self {
            Kind::Bug => "bug",
            Kind::Feature => "feature request",
        }
    }

    /// The `charter report` verb that drafts it.
    pub fn verb(self) -> &'static str {
        match self {
            Kind::Bug => "bug",
            Kind::Feature => "feature",
        }
    }
}

/// Everything charter can positively identify as the reporter's, and so removes from a draft.
#[derive(Debug, Default, Clone)]
pub struct Known {
    /// The plane's own root, removed whole wherever it appears.
    pub plane: Option<String>,
    /// The reporter's home directory, for a home that is not under `/Users` or `/home`.
    pub home: Option<String>,
    /// `(name, value)` for every variable in the environment.
    pub env: Vec<(String, String)>,
    /// `(name, placeholder)`: workspaces, personas, clones, vaults.
    pub names: Vec<(String, &'static str)>,
}

impl Known {
    /// What this process can identify: its environment, its home, and — inside a plane — the
    /// plane's root and the names in it.
    pub fn here(plane: Option<&Path>) -> Known {
        Known {
            plane: plane.map(|p| p.display().to_string()),
            home: dirs::home_dir().map(|h| h.display().to_string()),
            // `vars_os`, lossily: `vars` panics on a variable that is not Unicode.
            env: std::env::vars_os()
                .map(|(k, v)| {
                    (
                        k.to_string_lossy().into_owned(),
                        v.to_string_lossy().into_owned(),
                    )
                })
                .collect(),
            names: plane.map(names_in).unwrap_or_default(),
        }
    }

    /// `text` with what this knows removed, and the categories that were.
    pub fn scrub(&self, text: &str) -> (String, Vec<String>) {
        let mut used: Vec<String> = Vec::new();
        let mut note = |what: &str| {
            if !used.iter().any(|u| u == what) {
                used.push(what.to_string());
            }
        };

        // A credential's shape, a line at a time, as written and through its escapes (#1315):
        // the detector answers WHETHER, never where, so the line goes whole. Visible, so an
        // over-eager removal can be restored by hand.
        let lines: Vec<&str> = text.split('\n').collect();
        let mut kinds: Vec<Option<&'static str>> = lines
            .iter()
            .map(|line| crate::secretshape::kind_as_read(None, line))
            .collect();
        // Then lines a backslash joins to the next, which read as one (a TOML `"""` string):
        // a credential split across them is no line's own. The whole text is asked once
        // first, so text with nothing in it costs no more; each line is in one run at most.
        if crate::secretshape::kind_as_read(None, text).is_some() {
            let mut start = 0;
            for at in 0..lines.len() {
                if at + 1 < lines.len() && crate::secretshape::joins_the_next_line(lines[at]) {
                    continue;
                }
                if at > start
                    && let Some(kind) =
                        crate::secretshape::kind_as_read(None, &lines[start..=at].join("\n"))
                {
                    kinds[start..=at].iter_mut().for_each(|one| {
                        one.get_or_insert(kind);
                    });
                }
                start = at + 1;
            }
        }
        let mut out = String::with_capacity(text.len());
        for (i, (line, kind)) in lines.iter().zip(kinds).enumerate() {
            if i > 0 {
                out.push('\n');
            }
            match kind {
                Some(kind) => {
                    note("lines that look like a credential");
                    let _ = write!(out, "[redacted: a line that looks like {kind}]");
                }
                None => out.push_str(line),
            }
        }

        // Environment values, longest first so a value that contains another goes whole.
        // Before the paths: `$HOME` is a path, and `[env $HOME]` says more than `[home path]`.
        let mut env: Vec<&(String, String)> = self
            .env
            .iter()
            .filter(|(name, value)| {
                value.chars().count() >= ENV_VALUE_MIN
                    && !ENV_HARMLESS.contains(&name.as_str())
                    && !name.starts_with("LC_")
            })
            .collect();
        env.sort_by_key(|(_, value)| std::cmp::Reverse(value.len()));
        for (name, value) in env {
            if out.contains(value.as_str()) {
                note("environment values");
                out = out.replace(value.as_str(), &format!("[env ${name}]"));
            }
        }

        if let Some(plane) = self.plane.as_deref().filter(|p| p.len() > 1)
            && out.contains(plane)
        {
            note("the plane's path");
            out = out.replace(plane, "[plane]");
        }
        if home_paths().is_match(&out) {
            note("home paths");
            out = home_paths().replace_all(&out, "[home path]").into_owned();
        }
        if let Some(home) = self.home.as_deref().filter(|h| h.len() > 1)
            && out.contains(home)
        {
            note("home paths");
            out = out.replace(home, "[home]");
        }

        // Names, longest first, so `acme-migration` goes whole before `acme` can half-eat it.
        let mut names: Vec<&(String, &'static str)> = self.names.iter().collect();
        names.sort_by_key(|(name, _)| std::cmp::Reverse(name.len()));
        for (name, placeholder) in names {
            let (replaced, hit) = replace_word(&out, name, placeholder);
            if hit {
                note(&format!("{} names", placeholder.trim_matches(['[', ']'])));
                out = replaced;
            }
        }
        (out, used)
    }
}

/// Absolute paths under a home directory, which carry a user name and then project names —
/// and the same path flattened into one directory name (`-Users-someone-work-…`), which is how
/// some tools name a directory per project. Stops at a quote, a bracket or a backtick so the
/// Markdown around a path survives.
fn home_paths() -> &'static Regex {
    static ONCE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        Regex::new(r#"(?:/Users/|/home/|[A-Za-z]:\\Users\\|-Users-|-home-)[^\s`'"()<>\[\]]*"#)
            .expect("a pattern this module wrote")
    })
}

/// Whether `c` is part of a word, as Python's `\b` reads it.
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `text` with every whole-word `name` replaced by `with`, and whether any was.
fn replace_word(text: &str, name: &str, with: &str) -> (String, bool) {
    if name.is_empty() {
        return (text.to_string(), false);
    }
    let mut out = String::with_capacity(text.len());
    let mut hit = false;
    let mut rest = text;
    let mut before: Option<char> = None;
    while let Some(at) = rest.find(name) {
        let head = &rest[..at];
        let prev = head.chars().next_back().or(before);
        let next = rest[at + name.len()..].chars().next();
        out.push_str(head);
        let starts = name.chars().next().is_some_and(is_word);
        let ends = name.chars().next_back().is_some_and(is_word);
        if (!starts || !prev.is_some_and(is_word)) && (!ends || !next.is_some_and(is_word)) {
            out.push_str(with);
            hit = true;
        } else {
            out.push_str(name);
        }
        before = name.chars().next_back();
        rest = &rest[at + name.len()..];
    }
    out.push_str(rest);
    (out, hit)
}

/// The names in a plane that are the reporter's: its workspaces, the clones in them, its
/// personas and its vaults. Names under three characters, and `default`, which every plane
/// has, identify nobody and would mangle ordinary prose.
fn names_in(plane: &Path) -> Vec<(String, &'static str)> {
    let mut out: Vec<(String, &'static str)> = Vec::new();
    for ws in subdirs(&plane.join("workspaces")) {
        // Every folder with a `.git` the workspace holds, in `repos/` or at its root (ADR 0114),
        // repo or not: a name too many is masked, a name too few is told.
        if let Ok(held) = crate::repofolder::holdings(plane, &ws) {
            for row in held.repos.iter().chain(&held.shadowed).chain(&held.others) {
                out.push((row.repo.name.clone(), "[repo]"));
            }
            // A name purlis takes for no repo's, a link, or one whose `.git` could not be read
            // may still be a clone of the reporter's.
            for said in held.refused.iter().chain(&held.unreadable) {
                if !said.name.is_empty() {
                    out.push((said.name.clone(), "[repo]"));
                }
            }
        }
        out.push((ws, "[workspace]"));
    }
    for persona in subdirs(&plane.join("personas")) {
        out.push((persona, "[persona]"));
    }
    let ctx = crate::secrets::Ctx::new(plane, crate::secrets::Env::from_process());
    if let Ok(registry) = crate::secrets::registry::load_registry(&ctx) {
        for name in crate::secrets::registry::vaults(&registry).keys() {
            out.push((name.clone(), "[vault]"));
        }
    }
    out.retain(|(name, _)| {
        name.chars().count() >= 3
            && name != "default"
            && !name.starts_with(['_', '.'])
            && !OWN_WORDS.contains(&name.as_str())
            && !PLACEHOLDER_WORDS.contains(&name.as_str())
    });
    out
}

/// The names a plane's folder and its contents carry: the plane's own folder name, as the
/// project's, and [`names_in`].
fn names_of_a_plane(plane: &Path) -> Vec<(String, &'static str)> {
    let mut out = names_in(plane);
    if let Some(name) = plane.file_name().map(|n| n.to_string_lossy().into_owned())
        && name.chars().count() >= 3
        && !OWN_WORDS.contains(&name.as_str())
    {
        out.push((name, "[project]"));
    }
    out
}

fn subdirs(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out
}

/// A report, ready to be shown and — on the reporter's yes — filed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub kind: Kind,
    pub title: String,
    pub body: String,
    /// The categories the scrub removed, for the preview to name.
    pub scrubbed: Vec<String>,
}

/// Why a draft could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Refused(String);

impl Draft {
    /// A report the reporter described in prose. `title` overrides the first line.
    fn described(
        kind: Kind,
        text: &str,
        title: Option<&str>,
        known: &Known,
    ) -> Result<Draft, Refused> {
        let (text, scrubbed) = known.scrub(text.trim());
        if text.trim().is_empty() {
            return Err(Refused(format!(
                "a {} needs a description: pass it as an argument, with --from-file or on --stdin",
                kind.word()
            )));
        }
        let (title, more) = match title {
            Some(t) => {
                let (t, more) = known.scrub(t);
                (
                    cut_title(&t.split_whitespace().collect::<Vec<_>>().join(" ")),
                    more,
                )
            }
            None => (title_of(&text), Vec::new()),
        };
        Draft::assemble(kind, title, text, merge(scrubbed, more))
    }

    /// A draft bug from a saved panic, with the reporter's own words in front of it if any.
    fn of_panic(record: &Panic, words: Option<&str>, known: &Known) -> Result<Draft, Refused> {
        let place = short_place(&record.place);
        let (place, mut scrubbed) = known.scrub(&place);
        let (message, more) = known.scrub(&record.message);
        scrubbed = merge(scrubbed, more);
        let mut text = String::new();
        if let Some(words) = words.map(str::trim).filter(|w| !w.is_empty()) {
            let (words, more) = known.scrub(words);
            scrubbed = merge(scrubbed, more);
            text.push_str(&words);
            text.push_str("\n\n");
        }
        let version = record
            .version
            .as_deref()
            .unwrap_or("not recorded (a panic saved before records named one)");
        let _ = write!(
            text,
            "purlis panicked.\n\n\
             - **where:** `{place}`\n\
             - **purlis version:** {version}\n\
             - **message** (free text, read it before sending):\n\n"
        );
        for line in message.lines() {
            let _ = writeln!(text, "  > {line}");
        }
        let title = cut_title(&format!("purlis panicked at {place}"));
        Draft::assemble(Kind::Bug, title, text.trim_end().to_string(), scrubbed)
    }

    /// A draft bug from a sandbox block of purlis's own operation (#1338): the operation, the
    /// kind of path or host, the harness by the word the project calls it, and the versions:
    /// purlis's, and the harness's where the app heard one (#1353), as `harness_version`.
    ///
    /// **Made of closed words only.** Every part is one of [`crate::sandboxblock`]'s fixed
    /// words or phrases, a harness kind, or a version, so no path, argument, host or output can
    /// be in it, and there is nothing for [`Known::scrub`] to find. The harness's version is
    /// taken only in a version's shape (`a_version`), else it reads "not known".
    fn of_sandbox_block(
        block: &crate::sandboxblock::Block,
        harness: Option<crate::harness::Harness>,
        harness_version: Option<&str>,
    ) -> Result<Draft, Refused> {
        let crate::sandboxblock::Block {
            operation, kind, ..
        } = *block;
        let harness = harness.map_or("not known", |harness| harness.name());
        let harness_version = harness_version
            .filter(|said| a_version(said))
            .unwrap_or("not known");
        let text = format!(
            "The sandbox blocked an operation purlis itself ran in a chat: {}.\n\n\
             - **operation:** {}\n\
             - **kind of path or host:** {} ({})\n\
             - **harness:** {harness}\n\
             - **harness version:** {harness_version}\n\
             - **purlis version:** {}\n\n\
             The app drafted this from the block alone. It names no path, argument, host or \
             output.",
            block.said(),
            operation.word(),
            kind.word(),
            kind.phrase(),
            crate::adopt::app_version(),
        );
        let title = cut_title(&format!(
            "Sandbox blocked purlis's own {} ({})",
            operation.word(),
            kind.word()
        ));
        Draft::assemble(Kind::Bug, title, text, Vec::new())
    }

    fn assemble(
        kind: Kind,
        title: String,
        text: String,
        scrubbed: Vec<String>,
    ) -> Result<Draft, Refused> {
        let title = title.trim().to_string();
        if title.is_empty() {
            return Err(Refused("a report needs a title".to_string()));
        }
        let mut body = text;
        let _ = write!(
            body,
            "\n\n---\npurlis {} · {} {}\n_Filed with `purlis report {}`._",
            crate::adopt::app_version(),
            std::env::consts::OS,
            std::env::consts::ARCH,
            kind.verb()
        );
        if body.chars().count() > BODY_MAX {
            return Err(Refused(format!(
                "the report is over {BODY_MAX} characters; GitHub would refuse it. Shorten it."
            )));
        }
        Ok(Draft {
            kind,
            title,
            body,
            scrubbed,
        })
    }

    /// Twelve hex characters over the repository, the title and the body: what `--yes` must
    /// name for exactly this draft to be filed.
    pub fn digest(&self) -> String {
        let mut hash = Sha256::new();
        for part in [UPSTREAM, &self.title, &self.body] {
            hash.update(part.as_bytes());
            hash.update([0u8]);
        }
        hash.finalize()
            .iter()
            .take(6)
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// The draft as the reporter reads it: exactly what [`Reports::file`] would send, and where.
    pub fn preview(&self) -> String {
        let mut out = format!(
            "Draft {} for {UPSTREAM} — nothing has been filed.\n\n\
             Title: {}\n\
             ----- body -----\n{}\n----- end -----\n",
            self.kind.word(),
            self.title,
            self.body
        );
        if self.scrubbed.is_empty() {
            out.push_str("Scrubbed: nothing purlis could identify. Read it anyway.\n");
        } else {
            let _ = writeln!(
                out,
                "Scrubbed before drafting: {}. Read it anyway: purlis cannot see everything.",
                self.scrubbed.join(", ")
            );
        }
        out
    }

    /// A prefilled `issues/new` link, for a reporter purlis cannot file for, and whether the
    /// body is in it. A body that would make the link longer than [`LINK_MAX`] is left out, to
    /// be pasted from the preview: a browser or GitHub refuses a link that long.
    pub fn fallback_url(&self) -> (String, bool) {
        let title = format!(
            "https://github.com/{UPSTREAM}/issues/new?title={}",
            forge::quote(&self.title)
        );
        let whole = format!("{title}&body={}", forge::quote(&self.body));
        if whole.len() <= LINK_MAX {
            (whole, true)
        } else {
            (title, false)
        }
    }

    /// The words duplicate search sends to GitHub: the title's, without punctuation a search
    /// query would read as syntax. Shown before it is sent.
    pub fn query(&self) -> String {
        self.title
            .split(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '-' | '.')))
            .filter(|w| w.chars().count() > 2)
            .take(8)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Whether `said` has a version's shape and nothing else (#1353): one to sixteen parts of ASCII
/// digits, joined by single dots, `2.1.268`. What a harness answered is shown in a Report only
/// in this shape, so it can carry no path, name or sentence.
fn a_version(said: &str) -> bool {
    let parts: Vec<&str> = said.split('.').collect();
    parts.len() <= 16
        && parts
            .iter()
            .all(|part| (1..=10).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_digit()))
}

fn merge(mut a: Vec<String>, b: Vec<String>) -> Vec<String> {
    for x in b {
        if !a.contains(&x) {
            a.push(x);
        }
    }
    a
}

/// A place in the source as a report names it: a path relative to a repository stays, and an
/// absolute one keeps only its last three parts — enough to name a crate's file, and none of
/// the directories above it.
fn short_place(place: &str) -> String {
    if !(place.starts_with('/') || place.get(1..3) == Some(":\\")) {
        return place.to_string();
    }
    let parts: Vec<&str> = place.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    format!("…/{}", parts[parts.len().saturating_sub(3)..].join("/"))
}

/// The title of a described report: its first line, un-marked and bounded at a word.
fn title_of(text: &str) -> String {
    let first = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let first = first
        .strip_prefix('#')
        .map(|rest| rest.trim_start_matches('#'))
        .filter(|rest| rest.starts_with([' ', '\t']))
        .map(|rest| rest.trim().trim_end_matches('#').trim())
        .unwrap_or(first);
    cut_title(first)
}

fn cut_title(line: &str) -> String {
    if line.chars().count() <= TITLE_MAX {
        return line.to_string();
    }
    let cut: String = line.chars().take(TITLE_MAX - 1).collect();
    let cut = match cut.rfind(' ') {
        Some(space) if cut[..space].chars().count() >= BREAK_FLOOR => cut[..space].to_string(),
        _ => cut,
    };
    format!("{}…", cut.trim_end())
}

/// One upstream issue that may already cover a draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub number: u64,
    pub title: String,
    /// `open` or `closed`.
    pub state: String,
    pub url: String,
}

/// How many possible duplicates a search shows.
pub const DUPLICATES_SHOWN: u8 = 5;

/// The forge [`UPSTREAM`] is on: github.com, whatever forge a project uses.
pub fn tracker() -> Forge {
    Forge::default_of(ForgeKind::GitHub)
}

/// The tracker's backend for `caller`, if a report may be searched and filed as them: the
/// person in the window, with purlis's own sign-in to GitHub. Anyone else is refused before
/// anything is sent, in words that say where to file instead.
fn as_the_reporter(caller: &Caller) -> Result<Box<dyn forge::ForgeBackend>, ForgeError> {
    forge::signin::signed_in_backend(&tracker(), caller, crate::netlog::Feature::Report)
}

/// Candidate duplicates on [`UPSTREAM`], for the reporter to judge: a keyword search cannot
/// tell two failures of one command apart. Through the work-item area's search, as `caller`.
fn search_duplicates(
    tracker: &dyn WorkItems,
    caller: &Caller,
    draft: &Draft,
) -> Result<Vec<Hit>, ForgeError> {
    let query = draft.query();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    Ok(tracker
        .search(caller, UPSTREAM, &query, DUPLICATES_SHOWN)?
        .into_iter()
        .map(|item| Hit {
            number: number_of(&item),
            state: match item.state {
                crate::work::State::Open => "open",
                crate::work::State::Closed => "closed",
            }
            .to_string(),
            title: item.title,
            url: item.url,
        })
        .collect())
}

/// An issue's number, the last part of its tracker key (`github:github.com/o/r#12`).
fn number_of(item: &crate::work::WorkItem) -> u64 {
    item.key
        .as_str()
        .rsplit_once('#')
        .and_then(|(_, n)| n.parse().ok())
        .unwrap_or_default()
}

/// Opens the issue on [`UPSTREAM`] as `caller`, through the work-item area, and returns its
/// URL. It carries no label: a workspace's label is a project's, and this is purlis's tracker.
fn file(tracker: &dyn WorkItems, caller: &Caller, draft: &Draft) -> Result<String, ForgeError> {
    let item = tracker.create(
        caller,
        UPSTREAM,
        &NewWorkItem {
            title: draft.title.clone(),
            body: draft.body.clone(),
            workspace_label: None,
        },
    )?;
    // The forge's answer is keyed by its page, so an issue with no address is never `Ok`.
    Ok(item.url)
}

/// What a report is made from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evidence {
    /// A person's own words: `purlis report bug|feature`. `title` overrides the first line.
    PersonDraft {
        kind: Kind,
        text: String,
        title: Option<String>,
    },
    /// A panic the app saved, with the person's own words in front of it if any.
    Panic {
        record: Panic,
        words: Option<String>,
    },
    /// A failure of purlis's own, caught where it happened.
    PurlisFailure(PurlisFailure),
    /// An agent's draft, written through the `purlis-report` skill. Not built yet.
    AgentDraft(NotYet),
    /// A native crash the crash handler kept a minidump of, handed in on the next launch. Only
    /// ever a Crash report: it is sent to the Report intake alone, never filed on a tracker.
    NativeCrash(NativeCrash),
    /// An uncaught error in the window. Not built yet.
    WindowError(NotYet),
}

/// The value an evidence kind that is not built yet holds: none, so it cannot be handed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotYet {}

/// A failure of purlis's own that becomes evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PurlisFailure {
    /// The sandbox blocked an operation purlis itself ran in a chat (#1338), from the chat running
    /// `harness`, whose version is `harness_version` as the app heard it (#1353).
    SandboxBlock {
        block: crate::sandboxblock::Block,
        harness: Option<crate::harness::Harness>,
        harness_version: Option<String>,
    },
}

/// What became of a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Drafted and shown, nothing sent, and how it would be filed.
    Drafted { draft: Draft, route: Route },
    /// Filed, at `address`.
    Sent { address: String },
    /// Not filed, and why.
    Refused(Refusal),
}

/// Where a draft would go on the reporter's yes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// On [`UPSTREAM`] as the reporter, with purlis's own sign-in: only the person in the window.
    AsTheReporter,
    /// Nowhere purlis can send it from here, for `why`; `link` files it in the reporter's browser,
    /// with the body in it when `whole`, else the body is pasted from the draft.
    ByHand {
        why: ForgeError,
        link: String,
        whole: bool,
    },
}

/// Why a report was not filed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The evidence makes no report, in words the reporter can act on.
    Unusable(String),
    /// The digest named is not the draft's, whose digest is `digest`: nothing was sent.
    NotTheShownDraft { digest: String },
    /// The route refused or failed, for `why`; `link` files it in a browser, as in
    /// [`Route::ByHand`].
    NotFiled {
        why: ForgeError,
        link: String,
        whole: bool,
    },
    /// The person has not said yes to the stream this report belongs to: nothing was drafted
    /// and nothing sent (#5688).
    NoConsent(Stream),
    /// A Crash report this build has no Report intake to send to (it was built without its
    /// address): nothing was sent.
    NoIntake,
    /// The Report intake did not take a Crash report, for the reason given: nothing was listed.
    NotSent(String),
}

/// The one module every report passes through: evidence from a source in, an outcome out.
#[derive(Clone)]
pub struct Reports {
    known: Known,
    /// The answers it is held to, or `None` for this machine's, read only when a report belongs
    /// to a stream: a person's own report never reaches for the machine's store.
    consent: Option<Consent>,
    /// Where a Crash report goes, and the config root whose sent list records it (#5689). None
    /// in a build with no Report intake.
    intake: Option<(Arc<dyn intake::Intake>, PathBuf)>,
}

impl std::fmt::Debug for Reports {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reports")
            .field("known", &self.known)
            .field("consent", &self.consent)
            .field("intake", &self.intake.as_ref().map(|(_, root)| root))
            .finish()
    }
}

impl Default for Reports {
    /// Knowing nothing of this machine, and no stream with a yes.
    fn default() -> Self {
        Reports::new(Known::default())
    }
}

impl Reports {
    /// Reports that scrub what `known` names.
    /// No stream has a yes: only a person's own report is drafted ([`Reports::with_consent`]).
    pub fn new(known: Known) -> Reports {
        Reports {
            known,
            consent: Some(Consent::default()),
            intake: None,
        }
    }

    /// Reports that scrub what this process can identify, inside `plane` when there is one, held
    /// to the answers this machine keeps ([`Consent::here`]).
    pub fn here(plane: Option<&Path>) -> Reports {
        Reports {
            known: Known::here(plane),
            consent: None,
            intake: None,
        }
        .sending_to_the_built_in_intake()
    }

    /// Reports for a crash in the app, which knows no one plane: what this process can identify,
    /// and the names in every plane this machine opened, held to this machine's answers. The
    /// planes are read only with a yes to Crash reports: without one nothing is sent, and a
    /// panicking app has no time to spend on folders.
    pub fn for_a_crash() -> Reports {
        let consent = Consent::here();
        let mut known = Known::here(None);
        if consent.allows(Stream::CrashReports)
            && let Some(root) = crate::machine::config_root()
        {
            for recent in crate::machine::read(&root).store.recents {
                known.names.extend(names_of_a_plane(&recent.plane));
            }
        }
        Reports {
            known,
            consent: Some(consent),
            intake: None,
        }
        .sending_to_the_built_in_intake()
    }

    /// The same Reports, sending Crash reports to the Report intake built into this binary,
    /// listed under this machine's config root, when it has both.
    fn sending_to_the_built_in_intake(self) -> Reports {
        match (intake::built_in(), crate::machine::config_root()) {
            (Some(dsn), Some(root)) => self.sending_to(
                Arc::new(intake::Sentry::new(dsn, intake::Https::default())),
                &root,
            ),
            _ => self,
        }
    }

    /// The same Reports, sending Crash reports to `intake` and listing each in the sent list
    /// under `config_root`.
    pub fn sending_to(self, intake: Arc<dyn intake::Intake>, config_root: &Path) -> Reports {
        Reports {
            intake: Some((intake, config_root.to_path_buf())),
            ..self
        }
    }

    /// The same Reports, held to `consent`.
    pub fn with_consent(self, consent: Consent) -> Reports {
        Reports {
            consent: Some(consent),
            ..self
        }
    }

    /// Refused when `evidence` from `source` belongs to a stream the person has not said yes to.
    fn consented(&self, evidence: &Evidence, source: &Caller) -> Result<(), Refusal> {
        match stream_of(evidence, source) {
            Some(stream) if !self.consent.unwrap_or_else(Consent::here).allows(stream) => {
                Err(Refusal::NoConsent(stream))
            }
            _ => Ok(()),
        }
    }

    /// `evidence` from `source`, scrubbed and drafted, with the route it would take. Nothing is
    /// sent.
    pub fn take(&self, evidence: Evidence, source: &Caller) -> Outcome {
        if let Err(refused) = self.consented(&evidence, source) {
            return Outcome::Refused(refused);
        }
        if stream_of(&evidence, source) == Some(Stream::CrashReports) {
            return self.send_crash(&evidence);
        }
        match self.draft(evidence) {
            Ok(draft) => {
                let route = match as_the_reporter(source) {
                    Ok(_) => Route::AsTheReporter,
                    Err(why) => {
                        let (link, whole) = draft.fallback_url();
                        Route::ByHand { why, link, whole }
                    }
                };
                Outcome::Drafted { draft, route }
            }
            Err(refused) => Outcome::Refused(refused),
        }
    }

    /// Candidate duplicates of `draft` on [`UPSTREAM`], searched as `source`: the one request
    /// sent before a yes, its words ([`Draft::query`]) shown first.
    pub fn duplicates(&self, draft: &Draft, source: &Caller) -> Result<Vec<Hit>, ForgeError> {
        search_duplicates(as_the_reporter(source)?.as_ref(), source, draft)
    }

    /// Files the draft of `evidence` from `source`, only if its digest is `shown`: the digest
    /// the reporter was shown. Anything else is refused unsent.
    pub fn file(&self, evidence: Evidence, source: &Caller, shown: &str) -> Outcome {
        if let Err(refused) = self.consented(&evidence, source) {
            return Outcome::Refused(refused);
        }
        let draft = match self.draft(evidence) {
            Ok(draft) => draft,
            Err(refused) => return Outcome::Refused(refused),
        };
        let digest = draft.digest();
        if digest != shown {
            return Outcome::Refused(Refusal::NotTheShownDraft { digest });
        }
        match as_the_reporter(source).and_then(|tracker| file(tracker.as_ref(), source, &draft)) {
            Ok(address) => Outcome::Sent { address },
            Err(why) => {
                let (link, whole) = draft.fallback_url();
                Outcome::Refused(Refusal::NotFiled { why, link, whole })
            }
        }
    }

    /// A Crash report of `evidence`, its fixed fields scrubbed, sent to the Report intake and
    /// listed with its exact bytes. Nobody is there to be shown it: the yes to its stream is
    /// the person's word that it may go.
    fn send_crash(&self, evidence: &Evidence) -> Outcome {
        let Some((intake, root)) = &self.intake else {
            return Outcome::Refused(Refusal::NoIntake);
        };
        let crash = match evidence {
            Evidence::Panic { record, .. } => {
                crash::CrashReport::of_panic(record, &self.known, intake.dsn())
            }
            Evidence::NativeCrash(native) => {
                crash::CrashReport::of_native_crash(native, intake.dsn())
            }
            _ => {
                return Outcome::Refused(Refusal::Unusable(
                    "only a panic or a native crash is sent as a Crash report yet".to_string(),
                ));
            }
        };
        match intake.send(&crash.envelope, &crash.event_id) {
            Ok(address) => {
                // The report has left: a list that cannot be written does not unsend it.
                let _ = SentList::add(
                    root,
                    Sent {
                        at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
                        title: crash.title,
                        to: SentTo::Intake,
                        address: address.clone(),
                        bytes: crash.shown,
                    },
                );
                Outcome::Sent { address }
            }
            Err(why) => Outcome::Refused(Refusal::NotSent(why)),
        }
    }

    fn draft(&self, evidence: Evidence) -> Result<Draft, Refusal> {
        let made = match evidence {
            Evidence::PersonDraft { kind, text, title } => {
                Draft::described(kind, &text, title.as_deref(), &self.known)
            }
            Evidence::Panic { record, words } => {
                Draft::of_panic(&record, words.as_deref(), &self.known)
            }
            Evidence::PurlisFailure(PurlisFailure::SandboxBlock {
                block,
                harness,
                harness_version,
            }) => Draft::of_sandbox_block(&block, harness, harness_version.as_deref()),
            Evidence::NativeCrash(_) => Err(Refused(
                "a native crash is sent only to purlis's report service, as a Crash report"
                    .to_string(),
            )),
            Evidence::AgentDraft(never) | Evidence::WindowError(never) => match never {},
        };
        made.map_err(|Refused(said)| Refusal::Unusable(said))
    }
}

/// The stream `evidence` from `source` belongs to, or none for a person's own report.
///
/// A chat's draft is an agent's, whatever it holds. A person at the window or at a command they
/// typed writes their own and needs no yes, except for evidence only a machine makes. With nobody
/// at the keyboard (a trigger), a crash is a Crash report and anything else an Agent draft.
fn stream_of(evidence: &Evidence, source: &Caller) -> Option<Stream> {
    use crate::forge::backend::Surface;
    if source.is_a_chat() {
        return Some(Stream::AgentDrafts);
    }
    let crash = matches!(
        evidence,
        Evidence::Panic { .. } | Evidence::NativeCrash(_) | Evidence::WindowError(_)
    );
    match (evidence, source.surface()) {
        (Evidence::NativeCrash(_) | Evidence::WindowError(_), _) => Some(Stream::CrashReports),
        (_, Surface::Window | Surface::Command) => None,
        (_, Surface::Trigger | Surface::Mcp) if crash => Some(Stream::CrashReports),
        (_, Surface::Trigger | Surface::Mcp) => Some(Stream::AgentDrafts),
    }
}

/// A panic the app wrote down, as `panics.rs` writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Panic {
    /// When it panicked, in seconds since 1970, as the record's first line says.
    pub at: Option<u64>,
    pub place: String,
    pub message: String,
    pub version: Option<String>,
}

/// The file the app appends its panics to: `$CHARTER_PANIC_LOG` when set, as the app itself
/// reads it, else `panics.log` in the app's log directory (Tauri's `app_log_dir`).
pub fn panic_log() -> Option<PathBuf> {
    if let Some(file) = crate::envvar::var_os("PURLIS_PANIC_LOG").filter(|f| !f.is_empty()) {
        return Some(PathBuf::from(file));
    }
    Some(crate::applog::app_log_dir()?.join("panics.log"))
}

/// How long a saved panic is offered: one older than this was most likely reported, or
/// mattered to nobody, and offering it on every `report bug` would be noise.
pub const PANIC_OFFERED_FOR_SECS: u64 = 14 * 24 * 60 * 60;

impl Panic {
    /// Whether it is recent enough, at `now` (seconds since 1970), to be offered unasked.
    pub fn is_recent(&self, now: u64) -> bool {
        self.at
            .is_some_and(|at| now.saturating_sub(at) <= PANIC_OFFERED_FOR_SECS)
    }
}

/// The newest panic in `log`, or `None` when it holds none.
pub fn latest_panic(log: &str) -> Option<Panic> {
    let start = log.rfind("charter-panic pid ")?;
    let block = &log[start..];
    let at = block
        .lines()
        .next()
        .and_then(|l| l.split(" at ").nth(1))
        .and_then(|l| l.split_whitespace().next())
        .and_then(|n| n.parse().ok());
    let block = block.split("\nbacktrace:").next().unwrap_or(block);
    let mut place = None;
    let mut version = None;
    let mut message: Option<String> = None;
    for line in block.lines().skip(1) {
        if let Some(m) = message.as_mut() {
            m.push('\n');
            m.push_str(line);
        } else if let Some(v) = line.strip_prefix("place: ") {
            place = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("version: ") {
            version = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("message: ") {
            message = Some(v.to_string());
        }
    }
    Some(Panic {
        at,
        place: place?,
        message: message.unwrap_or_default(),
        version,
    })
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
