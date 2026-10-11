// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Does this text look like it holds a credential? A port of `charter/hooks.py`'s
//! `_secret_kind` and what it needs.
//!
//! It answers with a KIND — `"JWT"`, `"AWS access key"` — and never with the value, because
//! every caller of it prints the answer. `charter save` is the caller ported here: a staged
//! memory or ref file whose text matches is a commit charter refuses to make, since a memory
//! is pushed to a shared repository and a secret in one is disclosed the moment it lands.
//!
//! **The rules are Python's, character for character**, so the two implementations refuse the
//! same files. Two of them are worth naming:
//!
//! - **The credential-assignment rule is a shape, not a dictionary.** `token: <six or more
//!   non-blank characters>` is a hit whatever the value is — which is why the exemption below
//!   exists at all.
//! - **A vault REFERENCE is not a credential.** `token: "vault:forge/gh"` names where a
//!   secret lives; refusing it would refuse charter's own documented remedy. The exemption is
//!   narrow on purpose: every slot of the reference must be a NAME, so a live token inlined
//!   where a reference was meant (`vault:forge/ghp_…`, or a 40-character hex run) is an
//!   ordinary assignment again.
//!
//! # Why the lookahead is gone and the match is still Python's
//!
//! Python's rule is `…[:=]\s*(?=['"]?\S{6,})(?P<value>[^\n]*)`. `regex` has no lookahead, and
//! rewriting it as "match, then test the value in code" is **not** equivalent: the rewritten
//! pattern consumes the whole line at a position Python rejected, so a real assignment later
//! on the same line would never be reached (`token: x  password: hunter2s`). So the length
//! test is folded into the pattern instead — `(?P<value>\S{6}[^\n]*)` — where a position that
//! fails it is not a match at all and the search carries on from the next character, exactly
//! as `finditer` does. `['"]?` is dropped with it because it never changed an answer: a
//! leading quote is itself non-blank, so the run it starts is at least as long as the run
//! after it, and `\S{6,}` was the whole test.
//!
//! # The three character classes `regex` and CPython disagree about, and what is done here
//!
//! "Character for character" was true of the *pattern text* and false of what it matched.
//! `\s`, `\S`, `\b` and `(?i)` all mean something different in `regex` than in CPython's `re`
//! on a `str`, and M3 found five live divergences across the three of them — **two of them
//! misses**, where this module answered `None` for a line the Python charter refuses, and
//! three false refusals. Each is closed below with a
//! predicate that was swept over all 1,112,064 non-surrogate codepoints against CPython
//! itself rather than argued from the documentation of either engine:
//!
//! - **`\s`**: CPython's is Unicode `White_Space` **plus U+001C–U+001F**, the four separator
//!   controls (and it is exactly `str.isspace()`, so `rstrip` carries the same four).
//!   `regex`'s is `White_Space` alone. [`crate::memstore::is_python_space`] already existed
//!   for this, with a docstring saying the difference is not cosmetic; this module called
//!   `\s` and `trim_end()` anyway. Closed by `SPACE`/`NOT_SPACE` and by `py_rstrip`.
//! - **`\b`**: `regex`'s word character is UTS#18's, which counts a combining mark, a
//!   variation selector and ZWJ as word characters; CPython's is `str.isalnum() or '_'`,
//!   which counts none of them. So `⚠️token: hunter2is` — U+FE0F is `Mn` — had **no word
//!   boundary** in front of `token` here and one in Python: charter refused the line and
//!   charter-app did not. The leading `\b` is therefore gone from the pattern and asked in
//!   code by `is_python_word`, whose class was measured equal to CPython's `\w`.
//! - **`(?i)`**: CPython folds through the simple lowercase mapping plus `_casefix`'s extra
//!   cases; `regex` uses Unicode simple case folding. Swept over every letter of the keyword
//!   set, the two differ for exactly three characters — U+0130 `İ` and U+0131 `ı` (both fold
//!   to `i` for CPython and neither for `regex`), and U+212A `K` and U+017F `ſ` (which both
//!   engines fold). So only `i` needs help, and only where the keywords spell one.
//!
//! The trailing `\b` is kept and is inert either way: what follows the keyword in this
//! pattern is `[\s\x1c-\x1f]*[:=]`, and every character either class can match is a non-word
//! character to both engines, so the boundary is present in both or the match fails in both.

use std::sync::OnceLock;

use regex::Regex;

/// One NAME inside a vault reference: a vault, a key, an item, a field, a path segment.
const REFERENCE_SLOT: &str = r"[A-Za-z0-9_][A-Za-z0-9._-]*";

/// The most characters ALL the names of one reference may hold together and still be read as
/// names. Python's `_REFERENCE_NAMES_MAX`, and the reasoning is there: vault and key names are
/// short words, and the credentials this rule exists for are long random runs.
const REFERENCE_NAMES_MAX: usize = 32;

/// Prefixes that mark a name as a credential whatever its length, so a short or truncated
/// token cannot pass as a vault name, and that mark a bare token as a credential wherever it
/// stands ([`prefixed_token`]). Python's `_CREDENTIAL_PREFIXES` in its order, then the
/// documented prefixes it lacked, each from its provider's own documentation:
///
/// - GitHub, "GitHub's token formats" in
///   <https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/about-authentication-to-github>:
///   `ghp_` a personal access token (classic), `gho_` an OAuth access token, `ghu_` a GitHub
///   App user access token, `ghs_` a GitHub App installation access token, `ghr_` a GitHub App
///   refresh token — each the prefix and 36 characters — and `github_pat_` a fine-grained
///   personal access token.
/// - GitLab, "Token prefixes" in <https://docs.gitlab.com/security/tokens/>: `glpat-` a
///   personal, impersonation, project or group access token, `gloas-` an OAuth application
///   secret, `gldt-` a deploy token, `glrt-` and `glrtr-` a runner authentication token,
///   `glcbt-` a CI/CD job token, `glptt-` a trigger token, `glft-` a feed token, `glimt-` an
///   incoming mail token, `glagent-` an agent for Kubernetes token, `glwt-` a workspace token,
///   `glsoat-` a SCIM token, `glffct-` a feature flags client token.
/// - Slack, <https://docs.slack.dev/authentication/tokens>: `xoxb-` a bot token, `xoxp-` a
///   user token, `xapp-` an app-level token (a rotating one is `xoxe.xapp-`), `xwfp-` a
///   workflow token.
const CREDENTIAL_PREFIXES: [&str; 36] = [
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    "glpat-",
    "sk_live_",
    "sk_test_",
    "rk_live_",
    "sk-",
    "xoxa-",
    "xoxb-",
    "xoxp-",
    "xoxr-",
    "xoxs-",
    "AIza",
    "pypi-",
    "npm_",
    "hf_",
    "AKIA",
    "ASIA",
    "gloas-",
    "gldt-",
    "glrt-",
    "glrtr-",
    "glcbt-",
    "glptt-",
    "glft-",
    "glimt-",
    "glagent-",
    "glwt-",
    "glsoat-",
    "glffct-",
    "xapp-",
    "xwfp-",
];

/// CPython's `\s`, spelled for `regex`: Unicode `White_Space` and the four separator
/// controls it adds. Measured equal to `re`'s `\s` and to `str.isspace()` over every
/// non-surrogate codepoint.
const SPACE: &str = r"[\s\x1c-\x1f]";

/// CPython's `\S`.
const NOT_SPACE: &str = r"[^\s\x1c-\x1f]";

/// `i` as CPython's `(?i)` reads it: the letter, and the two Turkic spellings `regex`'s
/// simple case folding leaves out. Under `(?i)` this class is `{i, I, ı, İ}`, which is
/// exactly what `re.IGNORECASE` matches for `i`.
const I_CLASS: &str = "[iıİ]";

/// Which of [`checks`] a rule is, so a caller picks rules by what they are and not by how their
/// label is spelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rule {
    AgentMail,
    Jwt,
    PrivateKey,
    AwsKey,
    Assignment,
}

impl Rule {
    /// The rule's stable id, what an allowlist entry names.
    fn id(self) -> &'static str {
        match self {
            Self::AgentMail => "agentmail-key",
            Self::Jwt => "jwt",
            Self::PrivateKey => "private-key",
            Self::AwsKey => "aws-access-key",
            Self::Assignment => "credential-assignment",
        }
    }

    /// A credential and nothing else: what a code repository's save and a commit's scan ask,
    /// where the assignment rule would refuse `password: String` and a JWT is a test fixture.
    fn is_a_key(self) -> bool {
        matches!(self, Self::AgentMail | Self::PrivateKey | Self::AwsKey)
    }
}

/// A rule, its label, its pattern, and whether the pattern needs CPython's word boundary in
/// front of its match, which [`secret_kind`] asks rather than the engine. Only the keyword
/// rule has one.
type Check = (Rule, &'static str, String, bool);

/// The kinds, in the order they are asked. Python's `_SECRET_CHECKS`: the label of the FIRST
/// rule that hits anywhere in the text wins, not the earliest hit in the text.
fn checks() -> [Check; 5] {
    [
        (
            Rule::AgentMail,
            "AgentMail key",
            r"am_us_[A-Za-z0-9]{4,}".to_owned(),
            false,
        ),
        (
            Rule::Jwt,
            "JWT",
            r"eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}".to_owned(),
            false,
        ),
        (
            Rule::PrivateKey,
            "private key (PEM)",
            r"-----BEGIN [A-Z ]*PRIVATE KEY-----".to_owned(),
            false,
        ),
        (
            Rule::AwsKey,
            "AWS access key",
            r"AKIA[0-9A-Z]{12,}".to_owned(),
            false,
        ),
        (
            Rule::Assignment,
            "credential assignment",
            // Built by concatenation, never with a line continuation: a raw string does not
            // process `\` at end of line, so a "continued" pattern carries a literal
            // backslash-newline into the regex. `reference()` below learned the same thing.
            [
                r"(?i)(?:password|passwd|ap".to_owned(),
                format!("{I_CLASS}[_-]?key|ap{I_CLASS}key"),
                r"|secret|token)\b".to_owned(),
                format!("{SPACE}*[:=]{SPACE}*"),
                format!(r"(?P<value>{NOT_SPACE}{{6}}[^\n]*)"),
            ]
            .concat(),
            true,
        ),
    ]
}

fn compiled() -> &'static [(Rule, &'static str, Regex, bool)] {
    static ONCE: OnceLock<Vec<(Rule, &'static str, Regex, bool)>> = OnceLock::new();
    ONCE.get_or_init(|| {
        checks()
            .into_iter()
            .map(|(rule, label, pattern, boundary)| {
                (
                    rule,
                    label,
                    Regex::new(&pattern).expect("a pattern this module wrote"),
                    boundary,
                )
            })
            .collect()
    })
}

/// Is `c` a word character to CPython's `re` on a `str`?
///
/// `str.isalnum() or c == '_'`, which a sweep of all 1,112,064 non-surrogate codepoints
/// found to be exactly `[\p{L}\p{N}_]` — zero disagreements in either direction. That is
/// why this is a general-category test and not `char::is_alphanumeric`, whose `Alphabetic`
/// half also holds `Other_Alphabetic` combining marks that CPython does not count.
fn is_python_word(c: char) -> bool {
    static ONCE: OnceLock<Regex> = OnceLock::new();
    let word =
        ONCE.get_or_init(|| Regex::new(r"^[\p{L}\p{N}_]$").expect("a pattern this module wrote"));
    let mut buffer = [0u8; 4];
    word.is_match(c.encode_utf8(&mut buffer))
}

/// CPython's `\b` immediately before `at`, where what follows is known to be a word
/// character (every keyword starts with an ASCII letter).
fn python_boundary_before(text: &str, at: usize) -> bool {
    text[..at]
        .chars()
        .next_back()
        .is_none_or(|c| !is_python_word(c))
}

/// The next character boundary after `at`, so a rejected position can be stepped over
/// without splitting a codepoint — `regex`'s `captures_at` takes byte offsets and a search
/// resumed inside one is not a search.
fn after_one_char(text: &str, at: usize) -> usize {
    at + text[at..].chars().next().map_or(1, char::len_utf8)
}

fn reference() -> &'static Regex {
    static ONCE: OnceLock<Regex> = OnceLock::new();
    ONCE.get_or_init(|| {
        // Built by concatenation rather than with a line continuation: a raw string does not
        // process `\` at end of line, so a "continued" pattern would carry a literal
        // backslash-newline into the regex.
        let slot = REFERENCE_SLOT;
        let alternatives = [
            format!(r"vault:(?P<vault>{slot})/(?P<key>{slot})"),
            // Either name the command line answers to (RN-3).
            format!(r"(?:charter|purlis) secret get (?P<cli_vault>{slot}) (?P<cli_key>{slot})"),
            format!(r"op://(?P<op_vault>{slot})/(?P<op_item>{slot})/(?P<op_field>{slot})"),
            format!(r"vault://(?P<path>{slot}(?:/{slot})*)#(?P<field>{slot})"),
        ]
        .join("|");
        Regex::new(&format!(
            "{}(?:{alternatives}){}",
            r#"\A['"`]?"#, r#"['"`]?\z"#
        ))
        .expect("a pattern this module wrote")
    })
}

/// Whether `value` is exactly a vault reference whose slots hold names, not a credential.
/// Python's `_names_where_a_credential_lives`.
fn names_where_a_credential_lives(value: &str) -> bool {
    let Some(caps) = reference().captures(value) else {
        return false;
    };
    let mut slots: Vec<&str> = [
        "vault",
        "key",
        "cli_vault",
        "cli_key",
        "op_vault",
        "op_item",
        "op_field",
        "field",
    ]
    .into_iter()
    .filter_map(|name| caps.name(name).map(|m| m.as_str()))
    .collect();
    if let Some(path) = caps.name("path") {
        slots.extend(path.as_str().split('/'));
    }
    slots.iter().map(|s| s.len()).sum::<usize>() <= REFERENCE_NAMES_MAX
        && !slots
            .iter()
            .any(|s| CREDENTIAL_PREFIXES.iter().any(|p| s.starts_with(p)))
}

/// The KIND of secret `text` appears to hold, or `None`.
///
/// Every match of a rule that carries a value is asked whether that value merely NAMES where a
/// credential lives, so one exempt assignment does not excuse the next one on the line below.
/// A bare token behind one of [`CREDENTIAL_PREFIXES`] is asked last, so every guard built on
/// this answer — a settings save, a written memory, a session record, a handoff brief — refuses
/// the same tokens [`found`] does, and every kind the Python rules named stays the one named.
pub fn secret_kind(text: &str) -> Option<&'static str> {
    secret_at(text)
        .or_else(|| token_at(text))
        .map(|(label, _)| label)
}

/// [`secret_kind`], with the byte offset of the match that decided it — the one walk both
/// answer from, so the kind and where it is can never disagree.
fn secret_at(text: &str) -> Option<(&'static str, usize)> {
    for (_, label, rx, boundary) in compiled() {
        // `finditer`'s own walk, written out, because one position has to be REJECTED and
        // resumed from: a match whose leading word boundary is `regex`'s and not CPython's
        // is not a match at all, and Python's search carries on from the next character —
        // not from the end of the text this engine happened to consume. Resuming at the end
        // instead would step over a real assignment later on the same line, which is the
        // same defect the folded lookahead above exists to avoid.
        let mut at = 0;
        while at <= text.len() {
            let Some(caps) = rx.captures_at(text, at) else {
                break;
            };
            let whole = caps.get(0).expect("group 0 is always set on a match");
            if *boundary && !python_boundary_before(text, whole.start()) {
                at = after_one_char(text, whole.start());
                continue;
            }
            match caps.name("value") {
                // `rstrip` before the reference test, as Python does: the value runs to the
                // end of the line and a reference does not include the spaces after it —
                // and `rstrip` takes the four separator controls `trim_end` leaves behind,
                // which used to make a plain `vault:forge/gh␜` read as a credential.
                Some(value)
                    if names_where_a_credential_lives(crate::memstore::py_rstrip(
                        value.as_str(),
                    )) =>
                {
                    // Every match here is at least six characters, so it can never be
                    // empty and this can never fail to advance.
                    at = whole.end();
                }
                _ => return Some((label, whole.start())),
            }
        }
    }
    None
}

/// The KIND of credential `text` holds by a rule that code does not trip, or `None` — what a
/// workspace repo's save refuses (ADR 0051), where [`secret_kind`]'s credential-assignment rule
/// would refuse `password: String`.
///
/// Only shapes that are a credential and nothing else: a PEM private key block, an AWS access
/// key, an AgentMail key, and a token behind one of [`CREDENTIAL_PREFIXES`] with at least
/// sixteen token characters after it. A JWT is left out: test fixtures carry them by the
/// hundred, and a signed token is not a key to anything by itself.
pub fn token_kind(text: &str) -> Option<&'static str> {
    token_at(text).map(|(label, _)| label)
}

/// [`token_kind`], with the byte offset of the match that decided it.
fn token_at(text: &str) -> Option<(&'static str, usize)> {
    for (rule, label, rx, _) in compiled() {
        if rule.is_a_key()
            && let Some(hit) = rx.find(text)
        {
            return Some((label, hit.start()));
        }
    }
    let token = prefixed_token();
    token
        .captures(text)
        .and_then(|caps| caps.name("token"))
        .map(|hit| (FORGE_TOKEN.1, hit.start()))
}

/// A form of document a file can be read in as well as scanned as text, known by its name.
///
/// JSON and TOML let a string spell any character as an escape (`\u0041` is `A`), so what the
/// file holds — what charter reads, and what anyone reading the file gets — is not always what
/// its bytes spell. [`parsed_kind`] asks the document as it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Structured {
    Json,
    Toml,
}

impl Structured {
    /// The form `path` is read in by its extension (`.json`, `.toml`, any case), or `None`.
    /// Only the name is asked, so a file of any other name is never parsed.
    pub fn of(path: &str) -> Option<Self> {
        let ext = std::path::Path::new(path).extension()?.to_str()?;
        if ext.eq_ignore_ascii_case("json") {
            Some(Self::Json)
        } else if ext.eq_ignore_ascii_case("toml") {
            Some(Self::Toml)
        } else {
            None
        }
    }
}

/// [`secret_kind`] of `text` as the document reads, keys included: parsed as `form`, then
/// written out again with every escape decoded — JSON the way charter writes a manifest
/// ([`crate::pyjson::dumps_indent2_unicode`]), TOML by the `toml` writer — and then each key
/// and string value on its own. `None` for a document with no secret, and for text that does
/// not parse as `form`, whose bytes are all a scan of the text has to go on.
///
/// Asked through [`decoded_kind`], by the project save of each staged file of that form (#1295)
/// and by the settings editors of what is typed into them (#1304), so the two refuse the same
/// documents.
pub fn parsed_kind(form: Structured, text: &str) -> Option<&'static str> {
    let mut strings = Vec::new();
    match form {
        Structured::Json => {
            let doc = serde_json::from_str(text).ok()?;
            json_strings(&doc, &mut strings);
            either(&crate::pyjson::dumps_indent2_unicode(&doc), &strings)
        }
        Structured::Toml => {
            let table = text.parse::<toml::Table>().ok()?;
            toml_strings(&table, &mut strings);
            either(&toml::to_string(&table).ok()?, &strings)
        }
    }
}

/// [`secret_kind`] of the document as written out again, then of each of its strings on its
/// own (D-1295-6). The whole document first, so a key and its value still read as an
/// assignment; then each string, where an escaped control character is the character it
/// spells — the writers keep `\n` and `\t` as two characters, and the letter after the
/// backslash would otherwise hide the start of what follows it.
fn either(read: &str, strings: &[&str]) -> Option<&'static str> {
    secret_kind(read).or_else(|| strings.iter().find_map(|one| secret_kind(one)))
}

/// Every key and string value of a JSON document, in document order.
fn json_strings<'a>(value: &'a serde_json::Value, out: &mut Vec<&'a str>) {
    match value {
        serde_json::Value::String(s) => out.push(s),
        serde_json::Value::Array(items) => items.iter().for_each(|item| json_strings(item, out)),
        serde_json::Value::Object(map) => map.iter().for_each(|(key, item)| {
            out.push(key);
            json_strings(item, out);
        }),
        _ => {}
    }
}

/// Every key and string value of a TOML table, in document order.
fn toml_strings<'a>(table: &'a toml::Table, out: &mut Vec<&'a str>) {
    fn value<'a>(item: &'a toml::Value, out: &mut Vec<&'a str>) {
        match item {
            toml::Value::String(s) => out.push(s),
            toml::Value::Array(items) => items.iter().for_each(|one| value(one, out)),
            toml::Value::Table(table) => toml_strings(table, out),
            _ => {}
        }
    }
    for (key, item) in table {
        out.push(key);
        value(item, out);
    }
}

/// [`secret_kind`] of `text` as written, then as it reads ([`decoded_kind`]). What the settings
/// editors refuse a typed document for, and — with the line [`found`] gives as written — what
/// a project save refuses a staged file for, so the two can never disagree (#1304).
pub fn kind_as_read(form: Option<Structured>, text: &str) -> Option<&'static str> {
    secret_kind(text).or_else(|| decoded_kind(form, text))
}

/// [`secret_kind`] of what `text` spells through its escapes, for a caller that has asked the
/// text as written already: the parsed document when `form` names one and the text parses
/// ([`parsed_kind`]), then each reading of the text with its escapes decoded and no parse
/// ([`readings`]).
///
/// The parse reads a quoted key as the bare key it is, so a key and its value still form an
/// assignment; the lexical pass reads what the parse refuses or drops — comments, a trailing
/// comma, JSON5, JSON lines, a key given twice — and a file of any name (#1304).
///
/// A reading that joins a line ending in a backslash to the next ([`joined_line_at`]) is asked
/// [`joined_kind`]: every rule but the credential-assignment rule as it is, and that one only
/// where a `"""` string opens the value (D-1315-6, #1346). A Markdown hard line break
/// (`token:\` and a word on the next line) is prose, and an assignment whose value starts
/// before the break is read as written already.
pub fn decoded_kind(form: Option<Structured>, text: &str) -> Option<&'static str> {
    form.and_then(|form| parsed_kind(form, text))
        .or_else(|| readings(text).iter().find_map(|read| secret_kind(read)))
        .or_else(|| {
            joined_readings(text)
                .iter()
                .find_map(|read| joined_kind(read))
        })
}

/// [`secret_kind`] of a reading whose lines ending in a backslash were joined to the next: each
/// rule in [`checks`]'s order, the credential-assignment rule only where its value starts after
/// a TOML-like `= """` opener ([`opened_assignment_at`]), then the bare token rule (#1346).
///
/// The join is TOML's, and only a basic `"""` string reads a line-ending backslash as one, so an
/// assignment the join completes is one whose `"""` string the join is inside of. A value
/// after `:` or a bare `=` is a Markdown hard break or prose (D-1315-6), and stays unread here.
fn joined_kind(read: &str) -> Option<&'static str> {
    for (rule, label, rx, _) in compiled() {
        let hit = match rule {
            Rule::Assignment => opened_assignment_at(read),
            _ => rx.is_match(read),
        };
        if hit {
            return Some(label);
        }
    }
    token_at(read).map(|(label, _)| label)
}

/// Whether `read` holds a credential assignment, as [`secret_at`] reads one, whose value is a
/// `"""` string: a keyword, `=` and `"""`, with the assignment rule's own boundary in front and
/// its own exemption for a value that names where a credential lives.
fn opened_assignment_at(read: &str) -> bool {
    static ONCE: OnceLock<Regex> = OnceLock::new();
    let opener = ONCE.get_or_init(|| {
        Regex::new(
            &[
                r"(?i)(?:password|passwd|ap".to_owned(),
                format!("{I_CLASS}[_-]?key|ap{I_CLASS}key"),
                r"|secret|token)\b".to_owned(),
                format!("{SPACE}*={SPACE}*\"\"\""),
            ]
            .concat(),
        )
        .expect("a pattern this module wrote")
    });
    let Some((_, _, assignment, _)) = compiled()
        .iter()
        .find(|(rule, ..)| *rule == Rule::Assignment)
    else {
        return false;
    };
    let mut at = 0;
    while at <= read.len() {
        let Some(open) = opener.find_at(read, at) else {
            return false;
        };
        at = after_one_char(read, open.start());
        if !python_boundary_before(read, open.start()) {
            continue;
        }
        // The assignment rule itself, from the keyword: the opener is its separator, so it
        // matches here or nowhere, and its value is read as it would be on one line.
        let Some(caps) = assignment.captures_at(read, open.start()) else {
            continue;
        };
        let whole = caps.get(0).expect("group 0 is always set on a match");
        if whole.start() != open.start() {
            continue;
        }
        let names_a_place = caps.name("value").is_some_and(|value| {
            names_where_a_credential_lives(crate::memstore::py_rstrip(value.as_str()))
        });
        if !names_a_place {
            return true;
        }
    }
    false
}

/// [`found`] in `text` as written — its kind, and the line it starts on — then
/// [`decoded_kind`] of what it spells, with no line, since the decoded text's lines are not the
/// text's. What a project save refuses a staged file for, and what every guard that names a
/// line asks, so the first guard gives the answer the save gives last (#1315).
pub fn found_as_read(
    form: Option<Structured>,
    text: &str,
) -> Option<(Option<usize>, &'static str)> {
    if let Some(found) = found(text) {
        return Some((Some(found.line), found.kind));
    }
    decoded_kind(form, text).map(|kind| (None, kind))
}

/// [`token_kind`] of `text` as written, then of each reading with its escapes decoded
/// ([`readings`]). What a workspace repo's save refuses a staged file for.
pub fn token_kind_as_read(text: &str) -> Option<&'static str> {
    token_kind(text).or_else(|| {
        readings(text)
            .iter()
            .chain(joined_readings(text).iter())
            .find_map(|read| token_kind(read))
    })
}

/// How many times the escapes of a text are decoded, each time from the last reading: twice
/// reads JSON held inside a JSON string — a tool's result in a transcript — whose own escapes
/// are escaped once more (D-1304-10). Each round is one linear pass.
const ROUNDS: usize = 2;

/// Each reading of `text` with its escapes decoded and no line joined, up to [`ROUNDS`] deep,
/// stopping at the first that decodes nothing more.
fn readings(text: &str) -> Vec<String> {
    mapped_readings(text, false, Joins::Kept)
        .into_iter()
        .map(|(read, _)| read)
        .collect()
}

/// Each reading of `text` with its escapes decoded and every line that ends in a backslash
/// joined to the next ([`joined_line_at`]), where that reads otherwise than [`readings`]. Kept
/// apart, so a join only ever adds what is found: joining `abc\` to a token on the next line
/// makes one word of them, which the reading without the join still reads apart (#1315).
fn joined_readings(text: &str) -> Vec<String> {
    with_joins(text, false)
        .into_iter()
        .map(|(read, _)| read)
        .collect()
}

/// The readings of `text` with lines joined, mapped when `mapped`, that differ from those
/// without.
fn with_joins(text: &str, mapped: bool) -> Vec<(String, Vec<usize>)> {
    let apart = mapped_readings(text, false, Joins::Kept);
    mapped_readings(text, mapped, Joins::Made)
        .into_iter()
        .enumerate()
        .filter(|(at, (read, _))| apart.get(*at).is_none_or(|(other, _)| other != read))
        .map(|(_, reading)| reading)
        .collect()
}

/// Whether a reading joins a line that ends in a backslash to the next.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Joins {
    Kept,
    Made,
}

/// [`readings`] (or, with [`Joins::Made`], every reading with lines joined), each with — when
/// `mapped` — where each of its bytes sits in `text` (see [`decode`]).
fn mapped_readings(text: &str, mapped: bool, joins: Joins) -> Vec<(String, Vec<usize>)> {
    let mut out: Vec<(String, Vec<usize>)> = Vec::new();
    for _ in 0..ROUNDS {
        let last = out.last().map_or(text, |(read, _)| read.as_str());
        let mut map = Vec::new();
        let Some(read) = decode(last, mapped.then_some(&mut map), joins) else {
            break;
        };
        if let Some((_, before)) = out.last() {
            map = map.into_iter().map(|at| before[at]).collect();
        }
        out.push((read, map));
    }
    out
}

/// `text` with every string escape decoded to the character it spells, read lexically — no
/// parse, so it reads what a parser refuses, and one added line as well as a whole file. `None`
/// when `text` holds no escape, so a caller asks nothing twice. One round: [`readings`] goes
/// deeper.
///
/// The escapes of JSON and TOML (`\uXXXX` with surrogate pairs, `\UXXXXXXXX`, `\xHH`, `\n`,
/// `\t`, `\r`, `\b`, `\f`, `\e`, `\"`, `\/`, `\\`), the `\u{…}` of JavaScript and Rust,
/// and an octal escape. An escaped backslash is one backslash, so `\\u0041` reads as the six
/// characters it shows; a lone surrogate reads as U+FFFD. A backslash and a letter or digit
/// that spell no character this knows — YAML's `\v`, `\a`, `\N`, `\L`, a malformed `\u` —
/// read as one space (D-1304-9): whatever the character, it is not part of the word after it,
/// so a credential behind it starts a word. A backslash that ends its line joins that line to
/// the next, as in a TOML `"""` string ([`joined_line_at`]). Any other backslash stays as
/// written.
pub fn unescaped(text: &str) -> Option<String> {
    decode(text, None, Joins::Made)
}

/// [`unescaped`], with where each byte of the result came from: `map[i]` is the offset in
/// `text` of the escape (or character) that byte `i` was read from, and one more entry,
/// `text.len()`, closes the last.
fn decode(text: &str, mut map: Option<&mut Vec<usize>>, joins: Joins) -> Option<String> {
    if !text.contains('\\') {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    let mut at = 0;
    while at < text.len() {
        if let Some(len) = (joins == Joins::Made)
            .then(|| joined_line_at(&text[at..]))
            .flatten()
        {
            changed = true;
            at += len;
            continue;
        }
        let (ch, len) = match escape_at(&text[at..]) {
            Some(read) => {
                changed = true;
                read
            }
            None => {
                let ch = text[at..].chars().next().expect("at is on a char boundary");
                (ch, ch.len_utf8())
            }
        };
        if let Some(map) = map.as_deref_mut() {
            map.extend(std::iter::repeat_n(at, ch.len_utf8()));
        }
        out.push(ch);
        at += len;
    }
    if let Some(map) = map {
        map.push(text.len());
    }
    changed.then_some(out)
}

/// How many bytes TOML's line-ending backslash at the start of `s` takes, when one is there:
/// the backslash, the blanks after it on its line, the line break, and every blank and line
/// break after that — all of which a `"""` string reads as nothing, joining its line to the
/// next (#1315). Each byte is looked at once more at most, so a text stays one linear pass.
fn joined_line_at(s: &str) -> Option<usize> {
    let rest = s.strip_prefix('\\')?;
    let blank = |b: &u8| *b == b' ' || *b == b'\t';
    let after_blanks = &rest[rest.bytes().take_while(blank).count()..];
    let after_break = after_blanks
        .strip_prefix('\n')
        .or_else(|| after_blanks.strip_prefix("\r\n"))?;
    let gap = after_break
        .bytes()
        .take_while(|b| blank(b) || *b == b'\n' || *b == b'\r')
        .count();
    Some(s.len() - after_break.len() + gap)
}

/// Whether `line`'s last character other than a blank is a backslash: one a TOML `"""` string
/// joins to the next line ([`joined_line_at`]). For a caller that reads a text a line at a
/// time and has to know which lines read as one.
pub fn joins_the_next_line(line: &str) -> bool {
    // The blanks [`joined_line_at`] passes over, then the `\r` of a `\r\n` break.
    let line = line.strip_suffix('\r').unwrap_or(line);
    line.trim_end_matches([' ', '\t']).ends_with('\\')
}

/// The longest `\u{…}` this reads: six hex digits and the closing brace. Looking no further
/// for the brace keeps each escape a bounded look, so a text of open `\u{` reads in one
/// linear pass.
const BRACED_MAX: usize = 7;

/// The character the escape at the start of `s` spells, and how many bytes it takes; `None`
/// when `s` does not start with one. A backslash and a letter or digit that spell nothing
/// known read as a space ([`unescaped`]). Every look is a few bytes ahead, never further.
fn escape_at(s: &str) -> Option<(char, usize)> {
    let rest = s.strip_prefix('\\')?;
    let first = *rest.as_bytes().first()?;
    spelled(rest).or_else(|| first.is_ascii_alphanumeric().then_some((' ', 2)))
}

/// What the escape whose body is `rest` (the text after its backslash) spells, when it spells
/// a character.
fn spelled(rest: &str) -> Option<(char, usize)> {
    let hex = |digits: &str| -> Option<u32> {
        (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| u32::from_str_radix(digits, 16).ok())
            .flatten()
    };
    let fixed = |n: usize| rest.get(1..=n).and_then(hex);
    match rest.as_bytes().first()? {
        b'n' => Some(('\n', 2)),
        b't' => Some(('\t', 2)),
        b'r' => Some(('\r', 2)),
        b'b' => Some(('\u{8}', 2)),
        b'f' => Some(('\u{c}', 2)),
        b'e' => Some(('\u{1b}', 2)),
        b'"' => Some(('"', 2)),
        b'/' => Some(('/', 2)),
        b'\\' => Some(('\\', 2)),
        b'0'..=b'7' => {
            let digits = rest
                .bytes()
                .take(3)
                .take_while(|b| (b'0'..=b'7').contains(b))
                .count();
            let value = u32::from_str_radix(&rest[..digits], 8).ok()?;
            Some((char::from_u32(value)?, 1 + digits))
        }
        b'x' => Some((char::from_u32(fixed(2)?)?, 4)),
        b'U' => Some((char::from_u32(fixed(8)?)?, 10)),
        b'u' if rest[1..].starts_with('{') => {
            let close = rest.as_bytes()[2..]
                .iter()
                .take(BRACED_MAX)
                .position(|b| *b == b'}')
                .filter(|n| (1..=6).contains(n))?;
            Some((char::from_u32(hex(&rest[2..2 + close])?)?, 4 + close))
        }
        b'u' => {
            let unit = fixed(4)?;
            if !(0xD800..0xE000).contains(&unit) {
                return Some((char::from_u32(unit)?, 6));
            }
            let low = rest
                .get(5..11)
                .and_then(|next| next.strip_prefix("\\u"))
                .and_then(hex)
                .filter(|low| (0xDC00..0xE000).contains(low));
            match low {
                Some(low) if unit < 0xDC00 => Some((
                    char::from_u32(0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00))?,
                    12,
                )),
                _ => Some(('\u{FFFD}', 6)),
            }
        }
        _ => None,
    }
}

/// What [`leaks`] finds on `line` only once its escapes are decoded ([`readings`], then
/// [`joined_readings`]): each with
/// the span of `line` it was read from — escapes and all, for a caller that names where it is —
/// and the value it spells, which is what a caller masks and fingerprints, so an allowlist
/// entry names a value however it is spelled. A hit whose rule and value the line as written,
/// or an earlier reading, already shows is that one, and is left to it; one that differs in
/// any character is its own, so a value read as written never answers for a longer one its
/// escapes spell.
pub fn escaped_leaks(line: &str) -> Vec<(Leak, String)> {
    // A set, so a line of many values is still read in one pass.
    let mut seen: std::collections::HashSet<(&'static str, String)> = leaks(line)
        .into_iter()
        .map(|leak| (leak.rule, line[leak.span].to_owned()))
        .collect();
    let mut out = Vec::new();
    let apart = mapped_readings(line, true, Joins::Kept);
    for (read, map) in apart.into_iter().chain(with_joins(line, true)) {
        for leak in leaks(&read) {
            let value = read[leak.span.clone()].to_owned();
            if !seen.insert((leak.rule, value.clone())) {
                continue;
            }
            let span = map[leak.span.start]..map[leak.span.end];
            out.push((Leak { span, ..leak }, value));
        }
    }
    out
}

/// A credential [`found`] in a text: its kind, and the 1-based line it starts on. Never the
/// value, since every caller prints what it gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Found {
    pub kind: &'static str,
    pub line: usize,
}

/// The widest question this module answers: [`secret_kind`]'s rules, then [`token_kind`]'s
/// prefix rule for a bare token no assignment names. What a plane save refuses in any staged
/// file, and what a report redacts a line for.
pub fn found(text: &str) -> Option<Found> {
    let (kind, at) = secret_at(text).or_else(|| token_at(text))?;
    let line = 1 + text.as_bytes()[..at]
        .iter()
        .filter(|b| **b == b'\n')
        .count();
    Some(Found { kind, line })
}

/// One thing a commit's added line would publish: its kind, and where on the line it is.
///
/// The span is for the caller to MASK ([`masked`]) and never to print as it is: every caller
/// shows what it gets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leak {
    /// The rule's stable id, what an allowlist entry names (`email`, `forge-token`, …).
    pub rule: &'static str,
    /// What it looks like, in words.
    pub kind: &'static str,
    pub span: std::ops::Range<usize>,
}

/// The forge-prefix rule, by its id and its words.
const FORGE_TOKEN: (&str, &str) = ("forge-token", "a token by its forge's prefix");

/// Every rule [`leaks`] asks, by the id an allowlist entry names it with and the words a
/// refusal uses for it, in the order they are asked. `charter scan --explain` prints this.
pub fn leak_rules() -> Vec<(&'static str, &'static str)> {
    let keys = compiled()
        .iter()
        .filter(|(rule, ..)| rule.is_a_key())
        .map(|(rule, kind, ..)| (rule.id(), *kind));
    let vendors = gitleaks().iter().map(|(id, kind, _)| (*id, *kind));
    let personal = pii().iter().map(|(id, kind, ..)| (*id, *kind));
    keys.chain(std::iter::once(FORGE_TOKEN))
        .chain(vendors)
        .chain(personal)
        .collect()
}

/// Whether rule `id` finds personal data — a value a hash of it can be reversed from by
/// guessing, so an allowlist never names one by its fingerprint.
pub fn is_personal(id: &str) -> bool {
    pii().iter().any(|(rule, ..)| *rule == id)
}

/// Credential shapes from gitleaks' default rules that [`CREDENTIAL_PREFIXES`] and the PEM, AWS
/// and AgentMail rules do not already cover whole. Each is held by a vendor's own fixed marker — a
/// prefix, or a marker inside the token (Azure's `Q~`, Terraform's `.atlasv1.`, MaxMind's
/// `_mmk`) — so ordinary code does not match one; gitleaks' generic and entropy-only rules, and
/// its rules that read a vendor's NAME near an unmarked value, are left out for the reason
/// [`token_kind`] leaves the credential-assignment rule out (#773).
///
/// A rule with a capture group finds that group, so the characters it reads around a token to
/// know where it starts are not counted as the token. Where a pattern spells a whole literal a
/// token holds, one character is written as an escape, so the line that defines the rule is not
/// a line the rule refuses.
const GITLEAKS: [(&str, &str, &str); 63] = [
    (
        "slack-webhook",
        "Slack webhook",
        r"\bhooks\.slack\.com/(?:services|workflows|triggers)/[A-Za-z0-9+/]{43,56}",
    ),
    ("sendgrid-key", "SendGrid key", r"\bSG\.[A-Za-z0-9=_.-]{66}"),
    (
        "digitalocean-token",
        "DigitalOcean token",
        r"\bdo[opr]_v1_[a-f0-9]{64}",
    ),
    (
        "shopify-token",
        "Shopify token",
        r"\bshp(?:at|ca|pa|ss)_[a-fA-F0-9]{32}",
    ),
    ("linear-key", "Linear key", r"\blin_api_[A-Za-z0-9]{40}"),
    (
        "postman-key",
        "Postman key",
        r"\bPMAK-[a-fA-F0-9]{24}-[a-fA-F0-9]{34}",
    ),
    (
        "doppler-token",
        "Doppler token",
        r"\bdp\.pt\.[A-Za-z0-9]{43}",
    ),
    (
        "databricks-token",
        "Databricks token",
        r"\bdapi[a-f0-9]{32}",
    ),
    (
        "grafana-token",
        "Grafana token",
        r"\b(?:glc_[A-Za-z0-9+/]{32,400}={0,2}|glsa_[A-Za-z0-9]{32}_[A-Fa-f0-9]{8})",
    ),
    (
        "1password-secret-key",
        "1Password secret key",
        r"\bA3-[A-Z0-9]{6}-(?:[A-Z0-9]{11}|[A-Z0-9]{6}-[A-Z0-9]{5})-[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}\b",
    ),
    (
        "1password-service-account-token",
        "1Password service account token",
        r"\bops_eyJ[A-Za-z0-9+/]{250,}={0,3}",
    ),
    (
        "adobe-client-secret",
        "Adobe client secret",
        r"\bp8e-[A-Za-z0-9]{32}\b",
    ),
    (
        "age-secret-key",
        "age secret key",
        r"AGE-SECRET-KEY-1[QPZRY9X8GF2TVDW0S3JN54KHCE6MUA7L]{58}",
    ),
    (
        "airtable-personal-access-token",
        "Airtable token",
        r"\bpat[A-Za-z0-9]{14}\.[a-f0-9]{64}\b",
    ),
    (
        "alibaba-access-key-id",
        "Alibaba Cloud access key",
        r"\bLTAI[A-Za-z0-9]{20}\b",
    ),
    (
        "artifactory-api-key",
        "Artifactory key",
        r"\bAKCp[A-Za-z0-9]{69}\b",
    ),
    (
        "artifactory-reference-token",
        "Artifactory token",
        r"\bcmVmd[A-Za-z0-9]{59}\b",
    ),
    (
        "authress-service-client-access-key",
        "Authress key",
        r"\b(?:sc|ext|scauth|authress)_(?i:[a-z0-9]{5,30}\.[a-z0-9]{4,6}\.)acc[_-](?i:[a-z0-9-]{10,32}\.[a-z0-9+/_=-]{30,120})",
    ),
    (
        "aws-access-token",
        "AWS access key",
        r"\b(?:A3T[A-Z0-9]|ASIA|ABIA|ACCA)[A-Z2-7]{16}\b",
    ),
    (
        "aws-amazon-bedrock-api-key",
        "Amazon Bedrock key",
        r"\b(?:ABSK[A-Za-z0-9+/]{109,269}={0,2}|bedrock-api-key-YmVkcm9jay5hbWF6b25hd3MuY29\x74)",
    ),
    (
        "azure-ad-client-secret",
        "Azure AD client secret",
        r#"(?:^|[\\'"`\s>=:(,)])([a-zA-Z0-9_~.]{3}\dQ~[a-zA-Z0-9_~.-]{31,34})(?:$|[\\'"`\s<),])"#,
    ),
    (
        "clickhouse-cloud-api-secret-key",
        "ClickHouse key",
        r"\b4b1d[A-Za-z0-9]{38}\b",
    ),
    (
        "clojars-api-token",
        "Clojars token",
        r"(?i)CLOJARS_[a-z0-9]{60}",
    ),
    (
        "cloudflare-origin-ca-key",
        "Cloudflare origin CA key",
        r"\bv1\.0-[a-f0-9]{24}-[a-f0-9]{146}",
    ),
    (
        "duffel-api-token",
        "Duffel token",
        r"duffel_(?:test|live)_[A-Za-z0-9_=-]{43}",
    ),
    (
        "dynatrace-api-token",
        "Dynatrace token",
        r"dt0c01\.[A-Za-z0-9]{24}\.[A-Za-z0-9]{64}",
    ),
    (
        "easypost-api-token",
        "EasyPost token",
        r"\bEZ[AT]K[A-Za-z0-9]{54}\b",
    ),
    (
        "facebook-page-access-token",
        "Facebook token",
        r"\bEAA[MC][A-Za-z0-9]{100,}",
    ),
    (
        "flutterwave-key",
        "Flutterwave key",
        r"FLW(?:SECK_TEST-[A-Ha-h0-9]{32}-X|SECK_TEST-[A-Ha-h0-9]{12}|PUBK_TEST-[A-Ha-h0-9]{32}-X)",
    ),
    (
        "flyio-access-token",
        "Fly.io token",
        r"\b(?:fo1_[A-Za-z0-9_-]{43}|fm1[ar]_[A-Za-z0-9+/]{100,}={0,3}|fm2_[A-Za-z0-9+/]{100,}={0,3})",
    ),
    (
        "frameio-api-token",
        "Frame.io token",
        r"fio-u-[A-Za-z0-9_=-]{64}",
    ),
    (
        "gitlab-rrt",
        "GitLab runner registration token",
        r"GR134894\x31[A-Za-z0-9_-]{20}",
    ),
    (
        "gitlab-session-cookie",
        "GitLab session cookie",
        r"_gitlab_session=[0-9a-z]{32}",
    ),
    (
        "grafana-api-key",
        "Grafana key",
        r"(?i)\beyJrIjoi[A-Za-z0-9]{70,400}={0,3}",
    ),
    (
        "harness-api-key",
        "Harness key",
        r"\b(?:pat|sat)\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9]{24}\.[A-Za-z0-9]{20}",
    ),
    (
        "hashicorp-tf-api-token",
        "Terraform Cloud token",
        r"(?i:[a-z0-9]{14})\.atlasv1\.(?i:[a-z0-9_=-]{60,70})",
    ),
    (
        "heroku-api-key-v2",
        "Heroku key",
        r"\bHRKU-AA[0-9a-zA-Z_-]{58}",
    ),
    (
        "huggingface-organization-api-token",
        "Hugging Face token",
        r"\bapi_org_[A-Za-z]{34}\b",
    ),
    (
        "infracost-api-token",
        "Infracost token",
        r"\bico-[A-Za-z0-9]{32}\b",
    ),
    (
        "intra42-client-secret",
        "42 Intra secret",
        r"\bs-s4t2(?:ud|af)-[A-Fa-f0-9]{64}",
    ),
    (
        "maxmind-license-key",
        "MaxMind licence key",
        r"\b[A-Za-z0-9]{6}_[A-Za-z0-9]{29}_mmk\b",
    ),
    (
        "microsoft-teams-webhook",
        "Microsoft Teams webhook",
        r"https://[a-z0-9]+\.webhook\.office\.com/webhookb2/[a-z0-9]{8}-(?:[a-z0-9]{4}-){3}[a-z0-9]{12}@[a-z0-9]{8}-(?:[a-z0-9]{4}-){3}[a-z0-9]{12}/IncomingWebhook/[a-z0-9]{32}/[a-z0-9]{8}-(?:[a-z0-9]{4}-){3}[a-z0-9]{12}",
    ),
    (
        "notion-api-token",
        "Notion token",
        r"\bntn_[0-9]{11}[A-Za-z0-9]{35}\b",
    ),
    (
        "octopus-deploy-api-key",
        "Octopus Deploy key",
        r"\bAPI-[A-Z0-9]{26}\b",
    ),
    (
        "openshift-user-token",
        "OpenShift token",
        r"\bsha256~[A-Za-z0-9_-]{43}",
    ),
    (
        "perplexity-api-key",
        "Perplexity key",
        r"\bpplx-[A-Za-z0-9]{48}\b",
    ),
    (
        "planetscale-token",
        "PlanetScale token",
        r"\bpscale_(?:tkn|oauth|pw)_[A-Za-z0-9_=.-]{32,64}",
    ),
    (
        "prefect-api-token",
        "Prefect token",
        r"\bpnu_[A-Za-z0-9]{36}\b",
    ),
    ("pulumi-api-token", "Pulumi token", r"\bpul-[a-f0-9]{40}\b"),
    ("readme-api-token", "ReadMe token", r"\brdme_[a-z0-9]{70}\b"),
    (
        "rubygems-api-token",
        "RubyGems token",
        r"\brubygems_[a-f0-9]{48}\b",
    ),
    (
        "scalingo-api-token",
        "Scalingo token",
        r"\btk-us-[A-Za-z0-9_-]{48}",
    ),
    (
        "sendinblue-api-token",
        "Brevo key",
        r"\bxkeysib-[a-f0-9]{64}-[A-Za-z0-9]{16}\b",
    ),
    (
        "sentry-token",
        "Sentry token",
        r"\b(?:sntryu_[a-f0-9]{64}|sntrys_eyJpYXQiO[A-Za-z0-9+/]{10,200}(?:LCJyZWdpb25fdXJs|InJlZ2lvbl91cmwi|cmVnaW9uX3VybCI6)[A-Za-z0-9+/]{10,200}={0,2}_[A-Za-z0-9+/]{43})",
    ),
    (
        "settlemint-token",
        "SettleMint token",
        r"\bsm_(?:aat|pat|sat)_[A-Za-z0-9]{16}\b",
    ),
    (
        "shippo-api-token",
        "Shippo token",
        r"\bshippo_(?:live|test)_[a-fA-F0-9]{40}\b",
    ),
    (
        "sidekiq-sensitive-url",
        "Sidekiq credentials in a URL",
        r"(?i)\bhttps?://[a-f0-9]{8}:[a-f0-9]{8}@(?:gems|enterprise)\.contribsys\.com",
    ),
    (
        "slack-token",
        "Slack token",
        r"(?:\bxox(?:o-\d+-\d+-\d+-[a-fA-F0-9]+|e-\d-[A-Za-z0-9]{146}|e(?:-[0-9]{10,13}){3}-[A-Za-z0-9-]{28,34}|[ar]-(?:\d-)?[0-9a-zA-Z]{8,48})|(?i:\bxapp-\d-[a-z0-9]+-\d+-[a-z0-9]+))",
    ),
    (
        "sourcegraph-access-token",
        "Sourcegraph token",
        r"\bsgp_(?:[a-fA-F0-9]{16}_|local_)?[a-fA-F0-9]{40}\b",
    ),
    (
        "square-access-token",
        "Square token",
        r"\b(?:EAAA|sq0atp-)[A-Za-z0-9_-]{22,60}",
    ),
    (
        "stripe-access-token",
        "Stripe key",
        r"\b(?:sk|rk)_(?:test|live|prod)_[A-Za-z0-9]{10,99}\b",
    ),
    ("twilio-api-key", "Twilio key", r"\bSK[0-9a-fA-F]{32}\b"),
    (
        "vault-token",
        "Vault token",
        r"\b(?:hvb\.[A-Za-z0-9_-]{138,300}|hvs\.[A-Za-z0-9_-]{90,120})",
    ),
];

/// How a PII rule's candidate is checked beyond its pattern.
#[derive(Clone, Copy)]
enum Personal {
    Email,
    Card,
    Ssn,
    /// ISO 13616: a registered country's length, and mod-97.
    Iban,
    /// The UK National Insurance number: no check digit, so its allocation rules instead.
    UkNino,
    /// The Canadian Social Insurance Number: Luhn.
    CaSin,
    /// India's Aadhaar: Verhoeff.
    InAadhaar,
    /// Germany's Steuer-ID: ISO 7064 MOD 11,10 and its one repeated digit.
    DeSteuerId,
    /// Spain's DNI and NIE: the letter is the number mod 23.
    EsDni,
    /// Italy's codice fiscale: its check letter.
    ItCodiceFiscale,
    /// Brazil's CPF: two mod-11 check digits.
    BrCpf,
    /// The Dutch BSN: the 11-check.
    NlBsn,
}

/// How a national ID that is digits and nothing else is told from any other number: by the
/// name written in front of it — `BSN: `, `CPF `, `Aadhaar no. ` — and the separators it may
/// stand behind. The name is matched, the number alone is the finding (its capture group).
const ID_NAME_GAP: &str = r"(?:[ \t]*(?i:no|nr|number)\.?)?[^\w\n]{1,4}";

type PiiRule = (&'static str, &'static str, Regex, Personal);

fn pii() -> &'static [PiiRule] {
    static ONCE: OnceLock<Vec<PiiRule>> = OnceLock::new();
    ONCE.get_or_init(|| {
        [
            (
                "email",
                "an email address",
                r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}\b".to_owned(),
                Personal::Email,
            ),
            (
                "card-number",
                "a card number",
                r"\b(?:\d[ -]?){12,18}\d\b".to_owned(),
                Personal::Card,
            ),
            (
                "us-ssn",
                "an ID number (US SSN)",
                r"\b\d{3}-\d{2}-\d{4}\b".to_owned(),
                Personal::Ssn,
            ),
            (
                "iban",
                "an IBAN",
                r"\b([A-Z]{2}\d{2}(?:[A-Z0-9]{11,30}|(?: [A-Z0-9]{4}){2,7}(?: [A-Z0-9]{1,3})?))\b"
                    .to_owned(),
                Personal::Iban,
            ),
            (
                "uk-nino",
                "an ID number (UK National Insurance)",
                r"\b[A-CEGHJ-PR-TW-Z][A-CEGHJ-NPR-TW-Z] ?\d{2} ?\d{2} ?\d{2} ?[A-D]\b".to_owned(),
                Personal::UkNino,
            ),
            (
                "ca-sin",
                "an ID number (Canadian SIN)",
                format!(
                    r"\b(\d{{3}} \d{{3}} \d{{3}}|\d{{3}}-\d{{3}}-\d{{3}})\b|(?:\bSIN|(?i:\bsocial insurance))\b{ID_NAME_GAP}(\d{{9}}|\d{{3}}[ -]\d{{3}}[ -]\d{{3}})\b"
                ),
                Personal::CaSin,
            ),
            (
                "in-aadhaar",
                "an ID number (Aadhaar)",
                format!(
                    r"\b([2-9]\d{{3}} \d{{4}} \d{{4}}|[2-9]\d{{3}}-\d{{4}}-\d{{4}})\b|(?i:\baadhaa?r)\b{ID_NAME_GAP}([2-9]\d{{11}}|[2-9]\d{{3}}[ -]\d{{4}}[ -]\d{{4}})\b"
                ),
                Personal::InAadhaar,
            ),
            (
                "de-steuer-id",
                "an ID number (German tax ID)",
                format!(
                    r"\b([1-9]\d \d{{3}} \d{{3}} \d{{3}})\b|(?i:\bsteuer-?id(?:entifikationsnummer)?|\bidnr)\b{ID_NAME_GAP}([1-9]\d{{10}}|[1-9]\d \d{{3}} \d{{3}} \d{{3}})\b"
                ),
                Personal::DeSteuerId,
            ),
            (
                "es-dni",
                "an ID number (Spanish DNI/NIE)",
                r"\b(?:\d{8}|[XYZ]-?\d{7})-?[A-Z]\b".to_owned(),
                Personal::EsDni,
            ),
            (
                "it-codice-fiscale",
                "an ID number (Italian codice fiscale)",
                r"\b[A-Z]{6}[0-9LMNPQRSTUV]{2}[ABCDEHLMPRST][0-9LMNPQRSTUV]{2}[A-Z][0-9LMNPQRSTUV]{3}[A-Z]\b"
                    .to_owned(),
                Personal::ItCodiceFiscale,
            ),
            (
                "br-cpf",
                "an ID number (Brazilian CPF)",
                format!(
                    r"\b(\d{{3}}\.\d{{3}}\.\d{{3}}-\d{{2}})\b|(?i:\bcpf)\b{ID_NAME_GAP}(\d{{11}}|\d{{3}}\.\d{{3}}\.\d{{3}}-\d{{2}})\b"
                ),
                Personal::BrCpf,
            ),
            (
                "nl-bsn",
                "an ID number (Dutch BSN)",
                format!(
                    r"\b(\d{{4}}\.\d{{2}}\.\d{{3}})\b|(?i:\bbsn|\bburgerservicenummer)\b{ID_NAME_GAP}(\d{{9}}|\d{{4}}\.\d{{2}}\.\d{{3}})\b"
                ),
                Personal::NlBsn,
            ),
        ]
        .into_iter()
        .map(|(id, kind, pattern, check)| {
            (
                id,
                kind,
                Regex::new(&pattern).expect("a pattern this module wrote"),
                check,
            )
        })
        .collect()
    })
}

fn gitleaks() -> &'static [(&'static str, &'static str, Regex)] {
    static ONCE: OnceLock<Vec<(&'static str, &'static str, Regex)>> = OnceLock::new();
    ONCE.get_or_init(|| {
        GITLEAKS
            .iter()
            .map(|(id, kind, pattern)| {
                (
                    *id,
                    *kind,
                    Regex::new(pattern).expect("a pattern this module wrote"),
                )
            })
            .collect()
    })
}

/// The prefixes whose token body is letters and digits only, each with that body. npm's token
/// is `npm_` and 36 letters and digits (npm's "About access tokens"), while npm and pnpm read
/// their own settings from variables spelled `npm_config_<name>` — `npm_` and an underscore-
/// joined name the general body would take for a token (#1364). The floor stays the general
/// sixteen, so a truncated token is still found.
const ALPHANUMERIC_BODY: [(&str, &str); 1] = [("npm_", "[A-Za-z0-9]{16,}")];

fn prefixed_token() -> &'static Regex {
    static TOKEN: OnceLock<Regex> = OnceLock::new();
    TOKEN.get_or_init(|| {
        let prefixes: Vec<String> = CREDENTIAL_PREFIXES
            .iter()
            .filter(|p| !ALPHANUMERIC_BODY.iter().any(|(own, _)| own == *p))
            .map(|p| regex::escape(p))
            .collect();
        let alphanumeric: Vec<String> = ALPHANUMERIC_BODY
            .iter()
            .map(|(prefix, body)| format!("{}{body}", regex::escape(prefix)))
            .collect();
        // The token is its own group: the class in front of it consumes the character before
        // it, which is the newline when a token opens its line.
        Regex::new(&format!(
            r"(?:^|[^A-Za-z0-9_-])(?P<token>(?:{})[A-Za-z0-9_-]{{16,}}|{})",
            prefixes.join("|"),
            alphanumeric.join("|")
        ))
        .expect("a pattern this module wrote")
    })
}

/// An address that names nobody: a documentation or private-use domain (RFC 2606, RFC 6761,
/// `.internal`, `.local`, `localhost.localdomain`), a no-reply sender, `git@<host>`, which is an
/// SSH remote and not a mailbox, or an image's scale suffix (`logo@2x.png`).
fn names_nobody(address: &str) -> bool {
    let (local, domain) = address.rsplit_once('@').unwrap_or(("", address));
    let (local, domain) = (local.to_ascii_lowercase(), domain.to_ascii_lowercase());
    let reserved = [
        "example.com",
        "example.net",
        "example.org",
        "noreply.github.com",
    ]
    .iter()
    .any(|d| domain == *d || domain.ends_with(&format!(".{d}")));
    let tld = domain.rsplit('.').next().unwrap_or("");
    reserved
        || matches!(
            tld,
            "test" | "invalid" | "localhost" | "localdomain" | "example" | "local" | "internal"
        )
        || local.starts_with("noreply")
        || local.starts_with("no-reply")
        || local == "git"
        || a_scale_suffix(&domain)
}

/// `2x.png`, `3x.webp`, `1.5x.jpg`: what follows the `@` in an image asset's name. Only an
/// image's extension, so `ada@2x.dev` is still somebody's address.
fn a_scale_suffix(domain: &str) -> bool {
    static ONCE: OnceLock<Regex> = OnceLock::new();
    ONCE.get_or_init(|| {
        Regex::new(r"^[0-9]+(?:\.[0-9]+)?x\.(?:png|jpe?g|gif|webp|avif|svg|ico|bmp|tiff?|heic)$")
            .expect("a pattern this module wrote")
    })
    .is_match(domain)
}

/// A card number: 13 to 19 digits under a known issuer prefix, passing the Luhn check.
fn a_card(candidate: &str) -> bool {
    let digits: Vec<u32> = candidate.chars().filter_map(|c| c.to_digit(10)).collect();
    let n = digits.len();
    let head = |k: usize| digits.iter().take(k).fold(0, |acc, d| acc * 10 + d);
    let issuer = match digits.first() {
        Some(4) => matches!(n, 13 | 16 | 19),
        Some(3) => n == 15 && matches!(head(2), 34 | 37),
        Some(5) => n == 16 && (51..=55).contains(&head(2)),
        Some(2) => n == 16 && (2221..=2720).contains(&head(4)),
        Some(6) => (16..=19).contains(&n) && (head(4) == 6011 || head(2) == 65),
        _ => false,
    };
    issuer && luhn(&digits)
}

/// A US SSN the Social Security Administration could have issued: no area 000, 666 or 9xx, no
/// group 00, no serial 0000.
fn an_ssn(candidate: &str) -> bool {
    let mut parts = candidate.split('-');
    let (Some(area), Some(group), Some(serial)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    area != "000" && area != "666" && !area.starts_with('9') && group != "00" && serial != "0000"
}

/// Whether a number found on `line` is a number of its own and not three groups of a longer
/// run of groups, such as a phone number or another country's ID: no digit joined to it by a
/// separator on either side.
fn standing_alone(line: &str, hit: &regex::Match<'_>) -> bool {
    let joined = |mut chars: std::str::Chars<'_>, from_end: bool| {
        let mut next = || {
            if from_end {
                chars.next_back()
            } else {
                chars.next()
            }
        };
        match (next(), next()) {
            (Some(sep), Some(d)) => matches!(sep, ' ' | '-' | '.' | '/') && d.is_ascii_digit(),
            _ => false,
        }
    };
    !joined(line[..hit.start()].chars(), true) && !joined(line[hit.end()..].chars(), false)
}

/// The digits of `candidate`, its separators left out.
fn digits_of(candidate: &str) -> Vec<u32> {
    candidate.chars().filter_map(|c| c.to_digit(10)).collect()
}

/// The Luhn check over `digits`, the last one the check digit.
fn luhn(digits: &[u32]) -> bool {
    digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, d)| match (i % 2, d * 2) {
            (0, _) => *d,
            (_, doubled) if doubled > 9 => doubled - 9,
            (_, doubled) => doubled,
        })
        .sum::<u32>()
        % 10
        == 0
}

/// Every country in the SWIFT IBAN registry with its IBAN's length. A country not listed is
/// not an IBAN, so an IBAN-shaped run under any two letters is not found.
const IBAN_LENGTHS: &str = "AD24 AE23 AL28 AT20 AZ28 BA20 BE16 BG22 BH22 BI27 BR29 BY28 CH21 \
CR22 CY28 CZ24 DE22 DJ27 DK18 DO28 EE20 EG29 ES24 FI18 FK18 FO18 FR27 GB22 GE22 GI23 GL18 GR27 \
GT28 HR21 HU28 IE22 IL23 IQ23 IS26 IT27 JO30 KW30 KZ20 LB28 LC32 LI21 LT20 LU20 LV21 LY25 MC27 \
MD24 ME22 MK19 MN20 MR27 MT31 MU30 NI28 NL18 NO15 OM23 PK24 PL28 PS29 PT25 QA29 RO24 RS22 RU33 \
SA24 SC31 SD18 SE24 SI19 SK24 SM27 SO23 ST25 SV28 TL23 TN24 TR26 UA29 VA22 VG24 XK20 YE30";

/// An IBAN: a registered country, that country's length, and ISO 7064 mod-97 equal to 1.
fn an_iban(candidate: &str) -> bool {
    let compact: String = candidate.chars().filter(|c| *c != ' ').collect();
    let (Some(country), true) = (compact.get(..2), compact.len() > 4) else {
        return false;
    };
    let registered = IBAN_LENGTHS.split_whitespace().any(|entry| {
        entry.get(..2) == Some(country) && entry[2..].parse::<usize>().ok() == Some(compact.len())
    });
    let mut rotated = compact[4..].chars().chain(compact[..4].chars());
    let remainder = rotated.try_fold(0u32, |acc, c| {
        let value = c.to_digit(36)?;
        Some(if value < 10 {
            (acc * 10 + value) % 97
        } else {
            (acc * 100 + value) % 97
        })
    });
    registered && remainder == Some(1)
}

/// A UK National Insurance number as HMRC allocates them: neither letter of the prefix D, F, I,
/// Q, U or V, the second not O, none of the prefixes BG, GB, KN, NK, NT, TN or ZZ, a suffix A
/// to D (the pattern holds those), and spaced either everywhere or nowhere.
fn a_nino(candidate: &str) -> bool {
    let spaces = candidate.chars().filter(|c| *c == ' ').count();
    !matches!(
        candidate.get(..2),
        Some("BG" | "GB" | "KN" | "NK" | "NT" | "TN" | "ZZ")
    ) && matches!(spaces, 0 | 4)
}

/// A Canadian SIN: nine digits, not all zero, that pass Luhn.
fn a_sin(candidate: &str) -> bool {
    let digits = digits_of(candidate);
    digits.len() == 9 && digits.iter().any(|d| *d != 0) && luhn(&digits)
}

/// An Aadhaar number: twelve digits, the first 2 to 9 (the pattern holds that), passing the
/// Verhoeff check.
fn an_aadhaar(candidate: &str) -> bool {
    const D: [[u8; 10]; 10] = [
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
        [1, 2, 3, 4, 0, 6, 7, 8, 9, 5],
        [2, 3, 4, 0, 1, 7, 8, 9, 5, 6],
        [3, 4, 0, 1, 2, 8, 9, 5, 6, 7],
        [4, 0, 1, 2, 3, 9, 5, 6, 7, 8],
        [5, 9, 8, 7, 6, 0, 4, 3, 2, 1],
        [6, 5, 9, 8, 7, 1, 0, 4, 3, 2],
        [7, 6, 5, 9, 8, 2, 1, 0, 4, 3],
        [8, 7, 6, 5, 9, 3, 2, 1, 0, 4],
        [9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
    ];
    const P: [[u8; 10]; 8] = [
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
        [1, 5, 7, 6, 2, 8, 3, 0, 9, 4],
        [5, 8, 0, 3, 7, 9, 6, 1, 4, 2],
        [8, 9, 1, 6, 0, 4, 3, 5, 2, 7],
        [9, 4, 5, 3, 1, 2, 6, 8, 7, 0],
        [4, 2, 8, 6, 5, 7, 3, 9, 0, 1],
        [2, 7, 9, 3, 8, 0, 6, 4, 1, 5],
        [7, 0, 4, 6, 9, 1, 3, 2, 5, 8],
    ];
    let digits = digits_of(candidate);
    digits.len() == 12
        && digits.iter().rev().enumerate().fold(0u8, |check, (i, d)| {
            D[usize::from(check)][usize::from(P[i % 8][*d as usize])]
        }) == 0
}

/// A German Steuer-ID: eleven digits, the first not 0; in the first ten exactly one digit
/// appears twice or three times and every other at most once; the last the ISO 7064
/// MOD 11,10 check digit of the first ten.
fn a_steuer_id(candidate: &str) -> bool {
    let digits = digits_of(candidate);
    if digits.len() != 11 || digits[0] == 0 {
        return false;
    }
    let mut counts = [0u8; 10];
    for d in &digits[..10] {
        counts[*d as usize] += 1;
    }
    let repeated: Vec<u8> = counts.iter().copied().filter(|n| *n > 1).collect();
    if !matches!(repeated.as_slice(), [2] | [3]) {
        return false;
    }
    let product = digits[..10].iter().fold(10u32, |product, d| {
        let sum = match (d + product) % 10 {
            0 => 10,
            sum => sum,
        };
        (sum * 2) % 11
    });
    let check = match 11 - product {
        10 => 0,
        check => check,
    };
    check == digits[10]
}

/// A Spanish DNI or NIE: its letter is the number mod 23, an NIE's X, Y or Z read as 0, 1 or 2.
fn a_dni(candidate: &str) -> bool {
    const LETTERS: &[u8; 23] = b"TRWAGMYFPDXBNJZSQVHLCKE";
    let compact: Vec<char> = candidate.chars().filter(|c| *c != '-').collect();
    let Some((letter, number)) = compact.split_last() else {
        return false;
    };
    let number = number.iter().try_fold(0u32, |acc, c| {
        let value = match c {
            'X' => 0,
            'Y' => 1,
            'Z' => 2,
            c => c.to_digit(10)?,
        };
        Some(acc * 10 + value)
    });
    number.is_some_and(|n| char::from(LETTERS[(n % 23) as usize]) == *letter)
}

/// An Italian codice fiscale: its sixteenth character is the check letter of the first fifteen.
fn a_codice_fiscale(candidate: &str) -> bool {
    const ODD: [u32; 26] = [
        1, 0, 5, 7, 9, 13, 15, 17, 19, 21, 2, 4, 18, 20, 11, 3, 6, 8, 12, 14, 16, 10, 22, 25, 24,
        23,
    ];
    let chars: Vec<char> = candidate.chars().collect();
    if chars.len() != 16 {
        return false;
    }
    // A digit counts as the letter of the same place in the alphabet, 0 as A.
    let value = |c: char| match c.to_digit(10) {
        Some(d) => d,
        None => u32::from(c) - u32::from('A'),
    };
    let sum: u32 = chars[..15]
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if i % 2 == 0 {
                ODD[value(*c) as usize]
            } else {
                value(*c)
            }
        })
        .sum();
    char::from(b'A' + (sum % 26) as u8) == chars[15]
}

/// A Brazilian CPF: eleven digits, not one digit repeated, both mod-11 check digits right.
fn a_cpf(candidate: &str) -> bool {
    let digits = digits_of(candidate);
    if digits.len() != 11 || digits.iter().all(|d| *d == digits[0]) {
        return false;
    }
    [9usize, 10].iter().all(|&k| {
        let weighted: u32 = digits[..k]
            .iter()
            .enumerate()
            .map(|(i, d)| d * (k as u32 + 1 - i as u32))
            .sum();
        let check = match weighted % 11 {
            0 | 1 => 0,
            rest => 11 - rest,
        };
        check == digits[k]
    })
}

/// A Dutch BSN: nine digits, not all zero, passing the 11-check (weights 9 to 2, and -1 on the
/// last digit).
fn a_bsn(candidate: &str) -> bool {
    let digits = digits_of(candidate);
    if digits.len() != 9 || digits.iter().all(|d| *d == 0) {
        return false;
    }
    let weighted: i64 = digits[..8]
        .iter()
        .enumerate()
        .map(|(i, d)| i64::from(*d) * (9 - i as i64))
        .sum::<i64>()
        - i64::from(digits[8]);
    weighted % 11 == 0
}

/// Everything on one line a commit would publish: [`token_kind`]'s shapes, gitleaks' vendor
/// shapes ([`GITLEAKS`]), and personal data — an email address, a card number, an IBAN, an ID
/// number of one of the nine kinds [`pii`] knows, each held to its own check. In the order they sit on the line; where two overlap, the earlier one is kept.
///
/// What an agent's diff is checked with before it is committed (SQ-16). **Not** the plane
/// save's rule ([`found`]): that one refuses `password: String`, which is ordinary code.
pub fn leaks(line: &str) -> Vec<Leak> {
    let mut all = every_leak(line);
    all.sort_by_key(|leak| (leak.span.start, std::cmp::Reverse(leak.span.end)));
    let mut kept: Vec<Leak> = Vec::new();
    for leak in all {
        if kept
            .last()
            .is_none_or(|last| leak.span.start >= last.span.end)
        {
            kept.push(leak);
        }
    }
    kept
}

/// Every stretch of `line` that any of [`leaks`]' rules matches, as written and through its
/// escapes ([`escaped_leaks`], escapes and all, #1315), overlapping ones joined into one, in
/// order, each with the kind of the rule that starts it. For a caller that masks: [`leaks`]
/// keeps the earlier of two overlapping hits whole, which can leave the tail of the later one
/// outside every span.
pub fn leak_spans(line: &str) -> Vec<(std::ops::Range<usize>, &'static str)> {
    let mut all = every_leak(line);
    all.extend(escaped_leaks(line).into_iter().map(|(leak, _)| leak));
    joined(all)
}

/// `all`'s spans in order, each run of overlapping ones joined into one.
fn joined(mut all: Vec<Leak>) -> Vec<(std::ops::Range<usize>, &'static str)> {
    all.sort_by_key(|leak| (leak.span.start, std::cmp::Reverse(leak.span.end)));
    let mut joined: Vec<(std::ops::Range<usize>, &'static str)> = Vec::new();
    for leak in all {
        match joined.last_mut() {
            Some((span, _)) if leak.span.start < span.end => span.end = span.end.max(leak.span.end),
            _ => joined.push((leak.span, leak.kind)),
        }
    }
    joined
}

/// Every hit of every rule [`leaks`] runs, overlaps and all, in no order.
fn every_leak(line: &str) -> Vec<Leak> {
    let mut all: Vec<Leak> = Vec::new();
    for (rule, kind, rx, _) in compiled() {
        if rule.is_a_key() {
            all.extend(rx.find_iter(line).map(|hit| Leak {
                rule: rule.id(),
                kind,
                span: hit.range(),
            }));
        }
    }
    all.extend(
        prefixed_token()
            .captures_iter(line)
            .filter_map(|caps| caps.name("token"))
            .map(|hit| Leak {
                rule: FORGE_TOKEN.0,
                kind: FORGE_TOKEN.1,
                span: hit.range(),
            }),
    );
    for (id, kind, rx) in gitleaks() {
        all.extend(rx.captures_iter(line).map(|caps| Leak {
            rule: id,
            kind,
            span: the_finding(&caps).range(),
        }));
    }
    for (id, kind, rx, check) in pii() {
        all.extend(
            rx.captures_iter(line)
                .map(|caps| the_finding(&caps))
                .filter(|hit| match check {
                    Personal::Email => !names_nobody(hit.as_str()),
                    Personal::Card => a_card(hit.as_str()),
                    Personal::Ssn => an_ssn(hit.as_str()),
                    Personal::Iban => an_iban(hit.as_str()),
                    Personal::UkNino => a_nino(hit.as_str()),
                    Personal::CaSin => standing_alone(line, hit) && a_sin(hit.as_str()),
                    Personal::InAadhaar => standing_alone(line, hit) && an_aadhaar(hit.as_str()),
                    Personal::DeSteuerId => standing_alone(line, hit) && a_steuer_id(hit.as_str()),
                    Personal::EsDni => a_dni(hit.as_str()),
                    Personal::ItCodiceFiscale => a_codice_fiscale(hit.as_str()),
                    Personal::BrCpf => standing_alone(line, hit) && a_cpf(hit.as_str()),
                    Personal::NlBsn => standing_alone(line, hit) && a_bsn(hit.as_str()),
                })
                .map(|hit| Leak {
                    rule: id,
                    kind,
                    span: hit.range(),
                }),
        );
    }
    all
}

/// What a rule found: its first capture group that took part, or the whole match for a rule
/// without one. A group holds the value when the pattern reads what stands around it — the name
/// in front of an ID number, the delimiter in front of a token — to know where it starts.
fn the_finding<'h>(caps: &regex::Captures<'h>) -> regex::Match<'h> {
    caps.iter()
        .skip(1)
        .flatten()
        .next()
        .or_else(|| caps.get(0))
        .expect("group 0 is always set on a match")
}

/// The most characters of a masked value drawn; a longer one says its length instead.
const MASK_WIDTH: usize = 32;

/// `value` with all but a short head replaced by `*`, for a caller that has to say WHICH
/// value it refused without saying what it is. A value under twelve characters keeps nothing.
pub fn masked(value: &str) -> String {
    let n = value.chars().count();
    let keep = if n < 12 { 0 } else { (n / 6).min(4) };
    let shown = n.min(MASK_WIDTH);
    let mut out: String = value.chars().take(keep).collect();
    out.push_str(&"*".repeat(shown - keep));
    if n > MASK_WIDTH {
        out.push_str(&format!("… ({n} characters)"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_is_recognised_and_named_without_its_value() {
        for (text, kind) in [
            ("key: am_us_abcd1234", "AgentMail key"),
            ("jwt eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0", "JWT"),
            ("-----BEGIN OPENSSH PRIVATE KEY-----", "private key (PEM)"),
            ("aws AKIAIOSFODNN7EXAMPLE", "AWS access key"),
            ("password: hunter2is", "credential assignment"),
            ("API_KEY=abcdefgh", "credential assignment"),
            ("  Token : 'sixchars'", "credential assignment"),
        ] {
            assert_eq!(secret_kind(text), Some(kind), "{text}");
        }
    }

    #[test]
    fn ordinary_prose_and_a_short_value_are_not_a_secret() {
        for text in [
            "a note about the deploy token, without one",
            "token: x",
            "password:",
            "# secret = tbd",
            "",
        ] {
            assert_eq!(secret_kind(text), None, "{text}");
        }
    }

    #[test]
    fn a_vault_reference_is_a_name_and_a_token_inside_one_is_not() {
        for named in [
            "token: vault:forge/gh",
            "token: \"vault:forge/gh\"",
            "token: 'charter secret get forge token'",
            // The name the command line ships as since RN-3.
            "token: 'purlis secret get forge token'",
            "token: op://plane/forge/token",
            "token: vault://plane/forge#token",
        ] {
            assert_eq!(secret_kind(named), None, "{named}");
        }
        for inlined in [
            // A live token in the slot a name belongs in — the accident the exemption must
            // not cover.
            "token: vault:forge/ghp_0123456789abcdef",
            "token: vault:forge/0123456789abcdef0123456789abcdef01234567",
            "token: vault://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb#c",
        ] {
            assert_eq!(
                secret_kind(inlined),
                Some("credential assignment"),
                "{inlined}"
            );
        }
    }

    #[test]
    fn an_exempt_assignment_does_not_excuse_the_next_one() {
        // Python asks EVERY match, and `continue` is what makes that true. A port that
        // returned `None` on the first exempt match would pass every test above.
        let text = "token: vault:forge/gh\npassword: hunter2is\n";
        assert_eq!(secret_kind(text), Some("credential assignment"));
    }

    #[test]
    fn a_rejected_position_does_not_swallow_the_rest_of_its_line() {
        // The whole reason the lookahead was folded into the pattern rather than tested
        // afterwards. `token: x` fails the six-character rule; `password: hunter2s` on the
        // same line must still be found.
        assert_eq!(
            secret_kind("token: x  password: hunter2s"),
            Some("credential assignment")
        );
    }

    // ----------------------------------------------------------------------------------
    // the four character classes CPython and `regex` disagree about
    //
    // Every expectation below is the PYTHON charter's answer, taken by running
    // `charter.hooks._secret_kind` on the same literal. Two of them were misses here — a
    // line the Python charter refuses and charter-app allowed into a memory file — and two
    // were false refusals.
    // ----------------------------------------------------------------------------------

    #[test]
    fn a_combining_mark_before_the_keyword_does_not_hide_it() {
        // `regex`'s word character is UTS#18's and counts a mark, a variation selector and
        // ZWJ; CPython's does not, so the boundary Python sees in front of `token` was
        // missing here and the whole line passed. U+FE0F is the realistic one: it is the
        // second half of every emoji presentation sequence an agent types into a heading.
        for text in [
            "\u{301}token: hunter2is",
            "\u{200d}token: hunter2is",
            "\u{200c}token: hunter2is",
            "\u{26a0}\u{fe0f}token: hunter2is",
            // `Mc`, and `Other_Alphabetic`. This is the one row `char::is_alphanumeric`
            // still gets wrong — its `Alphabetic` half counts a mark with that property
            // and CPython's `isalnum` does not — so it is what holds the predicate to a
            // general-category test rather than to the convenient one.
            "\u{903}token: hunter2is",
        ] {
            assert_eq!(
                secret_kind(text),
                Some("credential assignment"),
                "{text:?} is a credential assignment to charter"
            );
        }
    }

    #[test]
    fn the_turkic_spellings_of_i_are_the_letter_i_here_as_they_are_to_python() {
        // CPython folds U+0130 and U+0131 to `i`; `regex`'s simple case folding does not,
        // so `apıkey: …` was not an api key here and is one to charter.
        for text in ["ap\u{131}key: hunter2is", "AP\u{130}KEY: hunter2is"] {
            assert_eq!(secret_kind(text), Some("credential assignment"), "{text:?}");
        }
        // And the two `regex` DOES fold, which are not a divergence and are pinned so a
        // narrower class cannot be substituted for `(?i)` without this going red.
        for text in ["\u{17f}ecret: hunter2is", "to\u{212a}en: hunter2is"] {
            assert_eq!(secret_kind(text), Some("credential assignment"), "{text:?}");
        }
    }

    #[test]
    fn the_four_separator_controls_are_blank_here_as_they_are_to_python() {
        // CPython's `\s` holds U+001C–U+001F and `regex`'s does not, so charter ate them as
        // whitespace and this module counted them toward its six non-blank characters —
        // refusing two lines charter allows.
        for text in ["token:\u{1c}\u{1c}\u{1c}\u{1c}ab", "token: ab\u{1c}cdefgh"] {
            assert_eq!(secret_kind(text), None, "{text:?}");
        }
    }

    #[test]
    fn a_vault_reference_with_a_separator_control_after_it_is_still_a_reference() {
        // `rstrip` takes the four; `trim_end` leaves them, so the value no longer matched
        // the reference pattern end to end and charter's own documented remedy was refused.
        assert_eq!(secret_kind("token: vault:forge/gh\u{1c}"), None);
        // And one that is NOT trailing still ends the reference, in both.
        assert_eq!(
            secret_kind("token: vault:forge/gh\u{1d}x"),
            Some("credential assignment")
        );
    }

    #[test]
    fn a_number_that_is_not_a_digit_is_a_word_character_here_as_it_is_to_python() {
        // `²`, `½` and `①` are `No`. CPython counts them as word characters, so there is no
        // boundary in front of `token` and no match; `char::is_alphanumeric` agrees with
        // CPython here only because `is_numeric` covers `No` — which is why the predicate
        // is a general-category test rather than an ad-hoc list.
        for text in [
            "\u{b2}token: hunter2is",
            "\u{bd}token: hunter2is",
            "\u{2460}token: hunter2is",
        ] {
            assert_eq!(secret_kind(text), None, "{text:?}");
        }
    }

    #[test]
    fn rejecting_a_position_does_not_step_over_a_real_assignment_later_on_the_line() {
        // The hazard the hand-written walk introduces, and the reason it resumes one
        // CHARACTER on rather than at the end of what the engine consumed. `xtoken:` is
        // rejected for its boundary; the `password:` after it is charter's answer.
        assert_eq!(
            secret_kind("xtoken: hunter2is  password: hunter2is"),
            Some("credential assignment")
        );
        // And the same with a multi-byte character at the rejected position, which is
        // where a byte-at-a-time resume would split a codepoint and panic.
        assert_eq!(
            secret_kind("\u{2460}token: hunter2is  password: hunter2is"),
            Some("credential assignment")
        );
    }

    #[test]
    fn the_first_rule_that_hits_names_the_kind_wherever_in_the_text_it_is() {
        // Python loops rules outside and matches inside, so a JWT at the end of a file still
        // outranks a credential assignment at the top.
        let text = "password: hunter2is\n\neyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0\n";
        assert_eq!(secret_kind(text), Some("JWT"));
    }
}

#[cfg(test)]
mod token_tests {
    use super::token_kind;

    #[test]
    fn a_token_or_a_private_key_is_named_and_ordinary_code_is_not() {
        let key = ["-----BEGIN OPENSSH ", "PRIVATE KEY-----\nabc\n"].concat();
        assert_eq!(token_kind(&key), Some("private key (PEM)"));
        let gh = ["let t = \"ghp", "_0123456789abcdefABCDEF\";"].concat();
        assert_eq!(token_kind(&gh), Some("a token by its forge's prefix"));
        for code in [
            "struct Login { password: String, token: Option<String> }",
            "let key = \"sk-short\";",
            "fn mask_ghp_prefix() {}",
            "api_key = os.environ['API_KEY']",
        ] {
            assert_eq!(token_kind(code), None, "{code}");
        }
    }
}

#[cfg(test)]
mod found_tests {
    use super::{Found, found};

    #[test]
    fn a_bare_token_in_prose_is_found_by_its_prefix() {
        let text = [
            "# notes\n\nthe deploy uses ghp",
            "_0123456789abcdefABCDEF now\n",
        ]
        .concat();
        assert_eq!(
            found(&text),
            Some(Found {
                kind: "a token by its forge's prefix",
                line: 3
            })
        );
    }

    #[test]
    fn the_line_is_where_the_value_starts_even_at_the_start_of_a_line() {
        // The token rule's leading class consumes the newline before a token that opens its
        // line; the line named is the token's, not the one above it.
        let text = ["one\ntwo\nglpat", "-0123456789abcdefABCD\n"].concat();
        assert_eq!(found(&text).map(|f| f.line), Some(3));
        let text = "one\n\n\npassword: hunter2is\n";
        assert_eq!(
            found(text),
            Some(Found {
                kind: "credential assignment",
                line: 4
            })
        );
        let key = ["a\n-----BEGIN RSA ", "PRIVATE KEY-----\n"].concat();
        assert_eq!(found(&key).map(|f| f.line), Some(2));
    }

    #[test]
    fn the_broad_rules_are_asked_first_and_their_kind_is_the_one_named() {
        // `secret_kind`'s answer wins where it has one, so a file both rules match is named
        // the way every other refusal names it.
        let text = ["token: ghp", "_0123456789abcdefABCDEF\n"].concat();
        assert_eq!(found(&text).map(|f| f.kind), Some("credential assignment"));
    }

    #[test]
    fn ordinary_text_and_a_vault_reference_are_not_found() {
        for text in [
            "a note about the deploy token, without one",
            "token: vault:forge/gh",
            "fn mask_ghp_prefix() {}",
            "",
        ] {
            assert_eq!(found(text), None, "{text}");
        }
    }
}

#[cfg(test)]
mod leak_tests {
    use super::{leaks, masked};

    /// Each line's kinds, in the order they sit on the line.
    fn kinds(line: &str) -> Vec<&'static str> {
        leaks(line).into_iter().map(|leak| leak.kind).collect()
    }

    #[test]
    fn a_key_and_an_email_on_an_added_line_are_each_named_with_where_they_are() {
        let line = [
            "let k = \"ghp",
            "_0123456789abcdefABCDEF\"; // ada@lovelace.dev",
        ]
        .concat();
        let found = leaks(&line);
        assert_eq!(
            found.iter().map(|l| l.kind).collect::<Vec<_>>(),
            vec!["a token by its forge's prefix", "an email address"]
        );
        assert_eq!(
            &line[found[0].span.clone()],
            ["ghp", "_0123456789abcdefABCDEF"].concat()
        );
        assert_eq!(&line[found[1].span.clone()], "ada@lovelace.dev");
    }

    #[test]
    fn the_gitleaks_shapes_a_forge_prefix_does_not_cover_are_found() {
        for (line, kind) in [
            (
                [
                    "https://hooks.slack.com/services/T0000000",
                    "0/B00000000/XXXXXXXXXXXXXXXXXXXXXXXX",
                ]
                .concat(),
                "Slack webhook",
            ),
            (
                [
                    "SG.abcdefghijklmnopqrstuv",
                    ".abcdefghijklmnopqrstuvwxyz0123456789ABCDEFG",
                ]
                .concat(),
                "SendGrid key",
            ),
            (["dop_v1_", &"a1".repeat(32)].concat(), "DigitalOcean token"),
            (["shpat_", &"a1".repeat(16)].concat(), "Shopify token"),
            (["lin_api_", &"A".repeat(40)].concat(), "Linear key"),
            (
                ["PMAK-", &"a".repeat(24), "-", &"b".repeat(34)].concat(),
                "Postman key",
            ),
            (
                ["-----BEGIN RSA ", "PRIVATE KEY-----"].concat(),
                "private key (PEM)",
            ),
            (["AKIA", "IOSFODNN7EXAMPLE"].concat(), "AWS access key"),
        ] {
            assert_eq!(kinds(&line), vec![kind], "{line}");
        }
    }

    #[test]
    fn every_leak_names_the_rule_an_allowlist_entry_would_name() {
        let line = [
            "let k = \"ghp",
            "_0123456789abcdefABCDEF\"; // ada@lovelace.dev",
        ]
        .concat();
        let rules: Vec<&str> = leaks(&line).into_iter().map(|l| l.rule).collect();
        assert_eq!(rules, ["forge-token", "email"]);
        let ids: Vec<&str> = super::leak_rules().iter().map(|(id, _)| *id).collect();
        assert_eq!(ids.len(), 79);
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(ids.len(), unique.len(), "every rule id is its own");
    }

    #[test]
    fn a_card_number_is_found_only_when_it_passes_luhn() {
        assert_eq!(kinds("card 4111 1111 1111 1111 ok"), vec!["a card number"]);
        assert_eq!(kinds("card 5500-0000-0000-0004"), vec!["a card number"]);
        assert_eq!(kinds("card 4111 1111 1111 1112"), Vec::<&str>::new());
        // A timestamp or an id is digits and nothing else.
        assert_eq!(
            kinds("at 1727712000123 ms, id 9876543210123456"),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn a_social_security_number_is_found_and_an_impossible_one_is_not() {
        assert_eq!(kinds("ssn: 123-45-6789"), vec!["an ID number (US SSN)"]);
        for line in [
            "000-12-3456",
            "666-12-3456",
            "900-12-3456",
            "123-00-4567",
            "123-45-0000",
        ] {
            assert_eq!(kinds(line), Vec::<&str>::new(), "{line}");
        }
    }

    #[test]
    fn a_real_domain_is_still_an_email_even_beside_a_placeholder() {
        assert_eq!(
            kinds("a@example.com and ada@lovelace.dev"),
            vec!["an email address"]
        );
        assert_eq!(kinds("ada@2x.dev"), vec!["an email address"]);
    }

    #[test]
    fn a_placeholder_or_noreply_address_is_not_somebodys_email() {
        for line in [
            "user@example.com",
            "a@b.example.org",
            "x@service.test",
            "x@host.invalid",
            "root@localhost",
            "noreply@anthropic.com",
            "no-reply@github.com",
            "1234+someone@users.noreply.github.com",
            "logo@2x.png",
            "<img src=\"icon@3x.webp\">",
            "hero@1.5x.jpg",
            "admin@localhost.localdomain",
            "svc@db.internal",
            "me@printer.local",
            "x@y.test",
            "x@docs.example",
            "x@nowhere.invalid",
        ] {
            assert_eq!(kinds(line), Vec::<&str>::new(), "{line}");
        }
    }

    #[test]
    fn ordinary_code_is_not_a_leak() {
        for line in [
            "struct Login { password: String, token: Option<String> }",
            "api_key = os.environ['API_KEY']",
            "let version = \"1.2.3-4\";",
            "@media (max-width: 600px) {",
            "import foo from '@scope/pkg@1.2.3';",
            "git clone git@github.com:acme/app.git",
        ] {
            assert_eq!(kinds(line), Vec::<&str>::new(), "{line}");
        }
    }

    #[test]
    fn a_masked_value_keeps_a_short_head_and_hides_the_rest() {
        assert_eq!(masked("ada@lovelace.dev"), "ad**************");
        assert_eq!(
            masked(&["ghp", "_0123456789abcdefABCDEF"].concat()),
            "ghp_**********************"
        );
        assert_eq!(masked("abc"), "***");
        // A long value keeps its head and says how long it was rather than drawing every star.
        let long = "x".repeat(200);
        assert_eq!(
            masked(&long),
            format!("xxxx{}… (200 characters)", "*".repeat(28))
        );
    }
}

#[cfg(test)]
mod joined_tests {
    use super::{Leak, joined};

    fn hit(kind: &'static str, span: std::ops::Range<usize>) -> Leak {
        Leak {
            rule: kind,
            kind,
            span,
        }
    }

    #[test]
    fn overlapping_hits_join_into_one_span_that_covers_both() {
        // The earlier hit is shorter than the later one it overlaps: `leaks` keeps only the
        // first, and its tail, 10..14, would go unmasked.
        let spans = joined(vec![
            hit("b", 6..14),
            hit("a", 2..10),
            hit("c", 20..25),
            hit("d", 25..30),
        ]);

        assert_eq!(spans, [(2..14, "a"), (20..25, "c"), (25..30, "d")]);
    }
}

#[cfg(test)]
mod documented_token_tests {
    use super::{found, leaks, secret_kind, token_kind};

    /// A made-up token body: `n` characters of the token alphabet, built here so no
    /// token-shaped literal sits in the source.
    fn fake_body(n: usize) -> String {
        "Ab1".chars().cycle().take(n).collect()
    }

    /// Every documented token format a bare token is recognised by, each as one made-up token.
    fn documented_tokens() -> Vec<String> {
        let classic = fake_body(36);
        let mut tokens: Vec<String> = ["ghp_", "gho_", "ghu_", "ghs_", "ghr_"]
            .iter()
            .map(|prefix| [prefix, classic.as_str()].concat())
            .collect();
        tokens.push(["github_pat_", &fake_body(22), "_", &fake_body(59)].concat());
        tokens.extend(
            [
                "glpat-", "gloas-", "gldt-", "glrt-", "glrtr-", "glcbt-", "glptt-", "glft-",
                "glimt-", "glagent-", "glwt-", "glsoat-", "glffct-",
            ]
            .iter()
            .map(|prefix| [prefix, fake_body(20).as_str()].concat()),
        );
        tokens.push(["xapp-1-", &fake_body(40)].concat());
        tokens.push(["xwfp-", &fake_body(40)].concat());
        tokens.push(["npm_", &fake_body(36)].concat());
        tokens
    }

    /// npm's and pnpm's own settings, spelled as npm reads them from the environment, are
    /// names and not npm tokens: a token's body is letters and digits only (#1364). Built in
    /// halves, so the line that adds them is not refused by a scanner from before the fix.
    #[test]
    fn npms_own_setting_names_are_not_an_npm_token() {
        for name in [
            ["npm", "_config_store_dir"].concat(),
            ["npm", "_config_cache_dir"].concat(),
            ["npm", "_config_registry"].concat(),
            ["npm", "_package_json"].concat(),
        ] {
            for text in [
                name.clone(),
                format!("export {name}=/Users/Shared/pnpm-store"),
                format!("vars: &[\"pnpm_config_store_dir\", \"{name}\"],"),
            ] {
                assert_eq!(secret_kind(&text), None, "{text}");
                assert_eq!(token_kind(&text), None, "{text}");
                assert!(leaks(&text).is_empty(), "{text}");
            }
        }
    }

    #[test]
    fn an_npm_token_beside_its_setting_name_is_still_found() {
        let token = ["npm_", &fake_body(36)].concat();
        let line = format!("{}={token}", ["npm", "_config__authToken"].concat());
        assert_eq!(token_kind(&line), Some("a token by its forge's prefix"));
        let at = line.find('=').expect("the line has an equals sign") + 1;
        let forge: Vec<_> = leaks(&line)
            .into_iter()
            .filter(|leak| leak.rule == "forge-token")
            .map(|leak| leak.span)
            .collect();
        assert_eq!(forge, vec![at..line.len()], "the token, and not the name");
    }

    #[test]
    fn a_bare_token_of_every_documented_format_is_a_secret_to_every_guard() {
        for token in documented_tokens() {
            let line = format!("the deploy uses {token} now");
            let head: String = token.chars().take(8).collect();
            assert_eq!(
                secret_kind(&line),
                Some("a token by its forge's prefix"),
                "{head}…"
            );
            assert!(token_kind(&line).is_some(), "{head}…");
            assert!(found(&line).is_some(), "{head}…");
            assert!(!leaks(&line).is_empty(), "{head}…");
        }
    }

    #[test]
    fn a_bare_token_in_a_settings_document_is_a_secret() {
        let token = ["glpat-", &fake_body(20)].concat();
        let text = format!("{{\n  \"remote\": \"{token}\"\n}}");
        assert_eq!(secret_kind(&text), Some("a token by its forge's prefix"));
    }

    #[test]
    fn a_name_that_only_starts_like_a_token_is_not_a_secret() {
        for text in [
            "git switch ghp-fix",
            "ghp_fix",
            "the glpat- prefix",
            "gldt-docs",
            "github_pat_ is a prefix",
            "xapp-short",
            "token",
            "a note about forge tokens",
            "fn read_ghp_token_from_env() {}",
        ] {
            assert_eq!(secret_kind(text), None, "{text}");
            assert_eq!(token_kind(text), None, "{text}");
        }
    }
}

#[cfg(test)]
mod parsed_tests {
    use super::*;

    /// Each shape the settings editors refuse as they read the document — a character written
    /// as an escape — is found the same way in a file of that form.
    #[test]
    fn a_secret_spelled_with_escapes_is_found_as_the_document_reads_it() {
        for (form, text, kind) in [
            (
                Structured::Json,
                r#"{"description": "\u0041KIAIOSFODNN7EXAMPLE"}"#,
                "AWS access key",
            ),
            (
                Structured::Json,
                r#"{"description": "AKIAIOSF\u004fDNN7EXAMPLE"}"#,
                "AWS access key",
            ),
            (
                Structured::Json,
                r#"{"note": "\u0067hp_0123456789abcdefABCDEFghij"}"#,
                "a token by its forge's prefix",
            ),
            (
                Structured::Json,
                r#"{"\u0041KIAIOSFODNN7EXAMPLE": true}"#,
                "AWS access key",
            ),
            (
                Structured::Toml,
                "[workspace]\ndefault = \"\\u0041KIAIOSFODNN7EXAMPLE\"\n",
                "AWS access key",
            ),
            (
                Structured::Toml,
                "[workspace]\ndefault = \"AKIA\\U00000049OSFODNN7EXAMPLE\"\n",
                "AWS access key",
            ),
            (
                Structured::Toml,
                "[extensions.stats.settings]\n\"pass\\u0077ord\" = \"hunter2hunter2\"\n",
                "credential assignment",
            ),
        ] {
            assert_eq!(
                secret_kind(text),
                None,
                "{text}: the raw scan already saw it"
            );
            assert_eq!(parsed_kind(form, text), Some(kind), "{text}");
        }
    }

    /// An escaped control character in front of a credential (D-1295-6): written out again,
    /// the escape stays two characters (`\n`, `\t`), and the letter after its backslash hides
    /// the token's start from the rules that need a boundary there. Each string is asked on
    /// its own as well, where the control character is what it is.
    #[test]
    fn an_escaped_control_character_before_a_credential_does_not_hide_it() {
        let token = ["ghp", "_0123456789abcdefABCDEFghij"].concat();
        let aws = "AKIAIOSFODNN7EXAMPLE";
        for escape in ["\\n", "\\t"] {
            for (form, text, kind) in [
                (
                    Structured::Json,
                    format!(r#"{{"note": "the deploy{escape}{token}"}}"#),
                    "a token by its forge's prefix",
                ),
                (
                    Structured::Json,
                    format!(r#"{{"note": "the deploy{escape}{aws}"}}"#),
                    "AWS access key",
                ),
                (
                    Structured::Json,
                    format!(r#"{{"the deploy{escape}{token}": 1}}"#),
                    "a token by its forge's prefix",
                ),
                (
                    Structured::Toml,
                    format!("[deploy]\nnote = \"the deploy{escape}{token}\"\n"),
                    "a token by its forge's prefix",
                ),
                (
                    Structured::Toml,
                    format!("[deploy]\nnote = \"the deploy{escape}{aws}\"\n"),
                    "AWS access key",
                ),
                (
                    Structured::Toml,
                    format!("[deploy]\nnotes = [\"one\", \"the deploy{escape}{token}\"]\n"),
                    "a token by its forge's prefix",
                ),
            ] {
                assert_eq!(parsed_kind(form, &text), Some(kind), "{text}");
            }
        }
    }

    #[test]
    fn a_key_and_its_value_are_still_read_together() {
        // The whole document is still asked: `"password": "…"` in JSON is no assignment, but
        // TOML writes `password = "…"`, and neither string alone is a secret.
        assert_eq!(
            parsed_kind(
                Structured::Toml,
                "[x]\n\"pass\\u0077ord\" = \"hunter2hunter2\"\n"
            ),
            Some("credential assignment")
        );
        assert_eq!(secret_kind("password"), None);
        assert_eq!(secret_kind("hunter2hunter2"), None);
    }

    #[test]
    fn a_document_with_no_secret_or_that_does_not_parse_answers_none() {
        for (form, text) in [
            (Structured::Json, r#"{"name": "alpha", "repos": []}"#),
            (Structured::Json, r#"{"name": "\u0041lpha"}"#),
            (Structured::Toml, "[plane]\nname = \"fixture\"\n"),
            (
                Structured::Json,
                "{\"name\": \"\\u0041KIAIOSFODNN7EXAMPLE\"",
            ),
            (
                Structured::Toml,
                "[plane\ndefault = \"\\u0041KIAIOSFODNN7EXAMPLE\"\n",
            ),
        ] {
            assert_eq!(parsed_kind(form, text), None, "{text}");
        }
    }

    #[test]
    fn only_a_json_or_toml_name_is_read_as_a_document() {
        for (path, form) in [
            ("charter.toml", Some(Structured::Toml)),
            ("purlis.toml", Some(Structured::Toml)),
            ("workspaces/alpha/workspace.json", Some(Structured::Json)),
            ("personas/devops/SETTINGS.JSON", Some(Structured::Json)),
            ("personas/steward/memory/note.md", None),
            ("workspaces/alpha/todos/0001.json.md", None),
            ("workspaces/alpha/events.jsonl", None),
            ("toml", None),
            (".json", None),
            ("dir.json/notes", None),
        ] {
            assert_eq!(Structured::of(path), form, "{path}");
        }
    }
}

/// Every spelling of a credential through escapes that a guard must read through (#1295, NO-7,
/// #1304), shared by each guard's tests so every guard is asked the same list.
#[cfg(test)]
pub(crate) mod escaped {
    /// One spelling: the file it sits in, its text, the kind the project save and the settings
    /// editors name, and whether it is a bare credential that the token-only guards (the commit
    /// and push scan, the repo save) find too, rather than an assignment only the wider rule
    /// reads.
    pub(crate) struct Shape {
        pub path: &'static str,
        pub text: String,
        pub kind: &'static str,
        pub token: bool,
    }

    const AWS: &str = "AWS access key";
    const FORGE: &str = "a token by its forge's prefix";
    const ASSIGNED: &str = "credential assignment";

    /// The forge token every shape that needs one carries, unescaped.
    pub(crate) fn token() -> String {
        ["ghp", "_0123456789abcdefABCDEFghij"].concat()
    }

    pub(crate) fn shapes() -> Vec<Shape> {
        let token = token();
        let tail = &token[1..];
        let one = |path, text: String, kind, token| Shape {
            path,
            text,
            kind,
            token,
        };
        let manifest = "workspaces/alpha/workspace.json";
        vec![
            // #1295 and NO-7: a character written as an escape, JSON.
            one(
                manifest,
                r#"{"name": "alpha", "description": "\u0041KIAIOSFODNN7EXAMPLE"}"#.into(),
                AWS,
                true,
            ),
            one(
                manifest,
                r#"{"name": "alpha", "description": "AKIAIOSF\u004fDNN7EXAMPLE"}"#.into(),
                AWS,
                true,
            ),
            one(
                manifest,
                format!(r#"{{"name": "alpha", "description": "\u0067{tail}"}}"#),
                FORGE,
                true,
            ),
            one(
                manifest,
                r#"{"name": "alpha", "\u0041KIAIOSFODNN7EXAMPLE": true}"#.into(),
                AWS,
                true,
            ),
            // ... and TOML, `\U` and a quoted key included.
            one(
                "charter.toml",
                "[workspace]\ndefault = \"\\u0041KIAIOSFODNN7EXAMPLE\"\n".into(),
                AWS,
                true,
            ),
            one(
                "charter.toml",
                "[workspace]\ndefault = \"AKIA\\U00000049OSFODNN7EXAMPLE\"\n".into(),
                AWS,
                true,
            ),
            one(
                "charter.toml",
                "[extensions.stats.settings]\n\"pass\\u0077ord\" = \"hunter2hunter2\"\n".into(),
                ASSIGNED,
                false,
            ),
            // D-1295-6: an escaped control character in front of a credential.
            one(
                manifest,
                format!(r#"{{"name": "alpha", "description": "the deploy\t{token}"}}"#),
                FORGE,
                true,
            ),
            one(
                manifest,
                format!(r#"{{"name": "alpha", "description": "the deploy\n{token}"}}"#),
                FORGE,
                true,
            ),
            one(
                manifest,
                r#"{"name": "alpha", "description": "the deploy\npassword: hunter2hunter2"}"#
                    .into(),
                ASSIGNED,
                false,
            ),
            one(
                "charter.toml",
                format!("[workspace]\nnotes = [\"one\", \"the deploy\\t{token}\"]\n"),
                FORGE,
                true,
            ),
            // #1304: what the parse drops — a JSON key given twice keeps only its last value.
            one(
                manifest,
                format!(r#"{{"name": "alpha", "note": "\u0067{tail}", "note": "x"}}"#),
                FORGE,
                true,
            ),
            // ... and what does not parse at all: a TOML key given twice, JSON with comments
            // and a trailing comma, JSON5, JSON lines, a TOML file with no extension.
            one(
                "charter.toml",
                "[workspace]\ndefault = \"\\u0041KIAIOSFODNN7EXAMPLE\"\ndefault = 1\n".into(),
                AWS,
                true,
            ),
            one(
                ".vscode/settings.json",
                format!("// the deploy\n{{\n  \"note\": \"\\u0067{tail}\",\n}}\n"),
                FORGE,
                true,
            ),
            one(
                "config/app.json5",
                format!("{{note: '\\u0067{tail}'}}\n"),
                FORGE,
                true,
            ),
            one(
                "workspaces/alpha/events.jsonl",
                "{\"a\": 1}\n{\"note\": \"\\u0041KIAIOSFODNN7EXAMPLE\"}\n".into(),
                AWS,
                true,
            ),
            one(
                "deploy/settings",
                "[deploy]\nkey = \"AKIA\\U00000049OSFODNN7EXAMPLE\"\n".into(),
                AWS,
                true,
            ),
            // The other escapes a string can spell a character with: TOML 1.1's `\x`, and the
            // `\u{…}` of JavaScript and Rust.
            one(
                "deploy.toml",
                format!("[deploy]\nnote = \"\\x67{tail}\"\n"),
                FORGE,
                true,
            ),
            one(
                "scripts/deploy.js",
                format!("const note = \"\\u{{67}}{tail}\";\n"),
                FORGE,
                true,
            ), // D-1304-9: an escape the decoder cannot spell — YAML's `\v`, `\a`, `\0`, `\N`,
            // `\L`, `\_`, an octal escape — still ends the word in front of a credential.
            one(
                "deploy.yaml",
                format!("note: \"the deploy\\v{token}\"\n"),
                FORGE,
                true,
            ),
            one(
                "deploy.yaml",
                format!("note: \"the deploy\\a{token}\"\n"),
                FORGE,
                true,
            ),
            one(
                "deploy.yaml",
                format!("note: \"the deploy\\N{token}\"\n"),
                FORGE,
                true,
            ),
            one(
                "deploy.yaml",
                format!("note: \"the deploy\\L{token}\\_x\"\n"),
                FORGE,
                true,
            ),
            one(
                "deploy.c",
                format!("char *note = \"the deploy\\0{token}\";\n"),
                FORGE,
                true,
            ),
            one(
                "deploy.c",
                format!("char *note = \"the deploy\\012{token}\";\n"),
                FORGE,
                true,
            ),
            one(
                "deploy.c",
                format!("char *note = \"\\147{tail}\";\n"),
                FORGE,
                true,
            ),
            // D-1304-10: JSON inside a JSON string, its escapes escaped once more.
            one(
                "workspaces/alpha/events.jsonl",
                format!("{{\"tool\": \"{{\\\"a\\\": \\\"x\\\\n{token}\\\"}}\"}}\n"),
                FORGE,
                true,
            ),
            one(
                "workspaces/alpha/events.jsonl",
                format!("{{\"tool\": \"\\\\u0067{tail}\"}}\n"),
                FORGE,
                true,
            ),
            // #1315: TOML's line-ending backslash in a `"""` string joins its line to the
            // next, so a credential split across it reads whole — in a file with no extension,
            // and in a TOML file that does not parse.
            one(
                "deploy/settings",
                format!(
                    "[deploy]\nnote = \"\"\"{}\\\n    {}\"\"\"\n",
                    &token[..8],
                    &token[8..]
                ),
                FORGE,
                true,
            ),
            one(
                "deploy.toml",
                format!(
                    "[deploy]\nnote = \"\"\"{}\\  \n\n    {}\"\"\"\nnote = 1\n",
                    &token[..8],
                    &token[8..]
                ),
                FORGE,
                true,
            ),
        ]
    }

    /// Files with escapes and no credential, which every guard lets through: a decoded
    /// character, an escaped backslash in front of what would otherwise read as an escape, a
    /// Windows path, and an escaped newline in prose.
    pub(crate) fn clean() -> Vec<(&'static str, String)> {
        vec![
            (
                "workspaces/alpha/workspace.json",
                r#"{"name": "\u0061lpha", "description": "caf\u00e9 \u2014 the deploy\nruns nightly", "path": "C:\\Users\\ada\\new", "how": "a key starts \\u0041KIA"}"#
                    .into(),
            ),
            (
                "charter.toml",
                "[workspace]\ndefault = \"caf\\u00e9\"\nnote = \"one\\ttwo\\nthree\"\n".into(),
            ),
            (
                "src/main.rs",
                "fn main() {\n    println!(\"caf\\u{e9}\\n\\t\\\\done\");\n}\n".into(),
            ),
            // D-1315-6: a Markdown hard line break is no assignment across the break.
            (
                "workspaces/alpha/memory/rotation.md",
                "Rotate the token:\\\nrotated-weekly\n\npassword:\\\n  hunter22x\n".into(),
            ),
        ]
    }
}

#[cfg(test)]
mod escaped_tests {
    use super::*;

    #[test]
    fn every_escaped_spelling_is_a_secret_as_the_text_reads() {
        for shape in escaped::shapes() {
            assert_eq!(
                found(&shape.text),
                None,
                "{}: the raw scan already saw it",
                shape.text
            );
            let form = Structured::of(shape.path);
            assert_eq!(
                kind_as_read(form, &shape.text),
                Some(shape.kind),
                "{}",
                shape.text
            );
            assert_eq!(
                decoded_kind(form, &shape.text),
                Some(shape.kind),
                "{}",
                shape.text
            );
            if shape.token {
                assert_eq!(
                    token_kind_as_read(&shape.text),
                    Some(shape.kind),
                    "{}",
                    shape.text
                );
            }
        }
    }

    #[test]
    fn every_escaped_spelling_is_a_secret_with_no_form_to_parse_it_by() {
        // A field, a title, a log line: text no parse is asked of still reads through its
        // escapes, so a guard that has no file name answers as the save does (#1315). Every
        // bare credential; a quoted key is an assignment only to the parse.
        for shape in escaped::shapes().into_iter().filter(|shape| shape.token) {
            assert!(kind_as_read(None, &shape.text).is_some(), "{}", shape.text);
            let line = found_as_read(None, &shape.text);
            assert!(
                line.is_some_and(|(line, _)| line.is_none()),
                "{}",
                shape.text
            );
        }
        let token = escaped::token();
        assert_eq!(
            found_as_read(None, &format!("one\ntoken {token}\n")),
            Some((Some(2), "a token by its forge's prefix"))
        );
    }

    #[test]
    fn a_file_with_escapes_and_no_secret_is_clean_as_it_reads() {
        for (path, text) in escaped::clean() {
            let form = Structured::of(path);
            assert_eq!(kind_as_read(form, &text), None, "{text}");
            assert_eq!(token_kind_as_read(&text), None, "{text}");
            for line in text.lines() {
                assert_eq!(escaped_leaks(line), Vec::new(), "{line}");
            }
        }
    }

    #[test]
    fn each_escape_decodes_to_the_character_it_spells() {
        for (text, read) in [
            (r"\u0041", "A"),
            (r"\U00000041", "A"),
            (r"\x41", "A"),
            (r"\u{41}", "A"),
            (r"\ud83d\ude00", "\u{1F600}"),
            (r"\ud83d!", "\u{FFFD}!"),
            (r#"\n\t\r\b\f\e\"\/\\"#, "\n\t\r\u{8}\u{c}\u{1b}\"/\\"),
            (r"\\u0041", r"\u0041"),
            (r"\101\0", "A\0"),
            // What spells no character ends a word, and stays one character wide.
            (r"a\qb \u00zz \U0011FFFF", "a b  00zz  0011FFFF"),
            (r"\N{LINE FEED}", " {LINE FEED}"),
            (r"\% \~", r"\% \~"),
            // TOML's line-ending backslash: it, the line break and the blanks after it read as
            // nothing; one with more on its line, or an escaped one, is not one.
            ("one\\\n    two", "onetwo"),
            ("one\\ \t\r\n\n\t  two", "onetwo"),
            ("one\\ two", "one\\ two"),
            ("one\\\\\ntwo", "one\\\ntwo"),
        ] {
            assert_eq!(unescaped(text).as_deref().unwrap_or(text), read, "{text}");
        }
        assert_eq!(unescaped("no escape here"), None);
    }

    #[test]
    fn a_long_run_of_open_escapes_reads_in_linear_time() {
        // `\u{` with no `}` near it: each one looks a few bytes ahead, never to the end.
        // And backslashes that look for the end of their line: each looks past its own blanks.
        let started = std::time::Instant::now();
        for text in [
            "\\u{".repeat(1 << 18),
            "\\  ".repeat(1 << 18),
            "\\ \n".repeat(1 << 18),
        ] {
            assert!(unescaped(&text).is_some() || !text.contains('\n'));
            assert_eq!(kind_as_read(None, &text), None);
            assert_eq!(token_kind_as_read(&text), None);
            assert_eq!(escaped_leaks(&text), Vec::new());
        }
        let took = started.elapsed();
        assert!(took < std::time::Duration::from_secs(10), "{took:?}");
    }

    #[test]
    fn a_line_join_only_ever_adds_what_is_found() {
        // Joining `abc\` to the next line makes one word of `abc` and a token; the reading
        // without the join still finds the token (#1315).
        let token = escaped::token();
        let tail = &token[1..];
        for text in [
            format!("note = \"\"\"abc\\\n{token}\"\"\"\n"),
            format!("note = \"\"\"abc\\\n\\u0067{tail}\"\"\"\n"),
        ] {
            for form in [None, Some(Structured::Toml)] {
                assert!(kind_as_read(form, &text).is_some(), "{form:?}: {text}");
            }
            assert!(token_kind_as_read(&text).is_some(), "{text}");
        }
        let escaped_only = format!("abc\\\n\\u0067{tail}");
        assert_eq!(found(&escaped_only), None);
        assert!(decoded_kind(None, &escaped_only).is_some());
        let line = format!("abc\\ \\u0067{tail}");
        assert_eq!(escaped_leaks(&line).len(), 1, "{line}");
    }

    /// #1346: an assignment, or a JWT, whose value starts after a line-ending backslash in a
    /// `"""` string reads whole in a file of any name, parsed or not. Every value is built here,
    /// so no credential-shaped literal sits in the source.
    #[test]
    fn an_assignment_or_a_jwt_split_by_a_line_ending_backslash_is_read_in_any_file() {
        let word = ["hunt", "er2", "2x"].concat();
        let keyword = ["pass", "word"].concat();
        let head = ["ey", "JhbGciOiJIUzI1NiJ9"].concat();
        let claims = ["ey", "JzdWIiOiJaWlFRIn0"].concat();
        for (text, kind) in [
            (
                format!("[deploy]\n{keyword} = \"\"\"\\\n    {word}\"\"\"\n"),
                "credential assignment",
            ),
            (
                format!("[deploy]\n{keyword} = \"\"\"\\  \n\n  \t{word}\"\"\"\nnote = 1,\n"),
                "credential assignment",
            ),
            (format!("note = \"\"\"{head}.\\\n  {claims}\"\"\"\n"), "JWT"),
            (format!("note = \"\"\"{head}\\\n.{claims}\"\"\"\n"), "JWT"),
        ] {
            assert_eq!(found(&text), None, "the raw scan already saw it: {text}");
            for form in [None, Some(Structured::Json)] {
                assert_eq!(kind_as_read(form, &text), Some(kind), "{form:?}: {text}");
                assert_eq!(
                    found_as_read(form, &text),
                    Some((None, kind)),
                    "{form:?}: {text}"
                );
            }
        }
        // D-1315-6 holds: a Markdown hard break, and a value with no `"""` opener in front of
        // the break, are no assignment across it.
        for text in [
            format!("{keyword}:\\\n  {word}\n"),
            format!("{keyword} = \\\n  {word}\n"),
            format!("{keyword} = '''\\\n  {word}'''\n"),
            format!("Rotate the {keyword} = \"\"\" weekly\\\n{word}\n"),
        ] {
            assert_eq!(kind_as_read(None, &text), None, "{text}");
        }
    }

    #[test]
    fn a_line_of_many_distinct_values_reads_in_linear_time() {
        // Every value its own, so no seen list is short-cut by a repeat: each address is read
        // as written (`nb0x@…`) and as it reads (`b0x@…`).
        let line: String = (0..1 << 15)
            .map(|i| format!("\\nb{i}x@lovelace{i}.dev "))
            .collect();
        let started = std::time::Instant::now();
        assert_eq!(escaped_leaks(&line).len(), 1 << 15);
        let took = started.elapsed();
        assert!(took < std::time::Duration::from_secs(10), "{took:?}");
    }

    #[test]
    fn json_inside_a_json_string_is_read_two_levels_down_and_no_further() {
        let token = escaped::token();
        let tail = &token[1..];
        assert_eq!(
            token_kind_as_read(&format!(r"\\u0067{tail}")),
            Some("a token by its forge's prefix")
        );
        assert_eq!(token_kind_as_read(&format!(r"\\\\u0067{tail}")), None);
    }

    #[test]
    fn an_escaped_leak_on_a_line_is_where_its_escapes_are_and_names_the_value_it_spells() {
        let token = escaped::token();
        let line = format!(r#"  "note": "\u0067{}","#, &token[1..]);
        let found = escaped_leaks(&line);
        assert_eq!(found.len(), 1, "{found:?}");
        let (leak, value) = &found[0];
        assert_eq!(leak.rule, "forge-token");
        assert_eq!(&line[leak.span.clone()], format!(r"\u0067{}", &token[1..]));
        assert_eq!(value, &token);
        // What the raw line already shows is not found a second time.
        let plain = format!(r#""note": "{token}", "x": "caf\u00e9""#);
        assert_eq!(leaks(&plain).len(), 1);
        assert_eq!(escaped_leaks(&plain), Vec::new());
        // A value its escapes make longer is its own, though the shorter one overlaps it.
        let longer = format!(r#""note": "{token}\u0041BC""#);
        let found = escaped_leaks(&longer);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].1, format!("{token}ABC"));
    }
}

/// The commit scan's wider rules (#773): the national ID numbers and the IBAN, each found only
/// when its check digit holds, and every gitleaks default rule anchored on a fixed vendor marker.
///
/// No credential- or ID-shaped literal sits in this source: every value is built from pieces
/// here, so the commit that adds it is not refused by the rules it adds.
#[cfg(test)]
mod wider_rule_tests {
    use super::{leak_rules, leaks, masked};

    /// Each line's rule ids, in the order they sit on the line.
    fn rules(line: &str) -> Vec<&'static str> {
        leaks(line).into_iter().map(|leak| leak.rule).collect()
    }

    /// `n` characters drawn in turn from `alphabet`: a made-up body of a token's own alphabet.
    fn body(alphabet: &str, n: usize) -> String {
        alphabet.chars().cycle().take(n).collect()
    }

    const ALNUM: &str = "aB3dE5gH7kM9pQ2rS4";
    const HEX: &str = "a1b2c3d4e5f67890";
    const UPPER: &str = "Q7R2S9T4V6W8X3Y5";
    const LOWER: &str = "q7r2s9t4v6w8x3y5";
    const LETTERS: &str = "qRsTuVwXyZ";

    /// `(rule id, a value of its kind, the same value with its check broken)`, each made of
    /// pieces. The broken one is the near-miss that must not be refused.
    fn personal() -> Vec<(&'static str, String, String)> {
        vec![
            // ISO 13616's own example, and with its check digits off by one.
            (
                "iban",
                ["DE89", " 3704 0044", " 0532 0130 00"].concat(),
                ["DE88", " 3704 0044", " 0532 0130 00"].concat(),
            ),
            (
                "iban",
                ["GB82", "WEST", "12345698765432"].concat(),
                ["GB83", "WEST", "12345698765432"].concat(),
            ),
            // HMRC's own documentation placeholder, QQ, is a prefix never issued.
            (
                "uk-nino",
                ["AB", " 12 34", " 56 C"].concat(),
                ["QQ", " 12 34", " 56 C"].concat(),
            ),
            (
                "ca-sin",
                ["046", " 454 ", "286"].concat(),
                ["046", " 454 ", "287"].concat(),
            ),
            (
                "in-aadhaar",
                ["2341", " 2341 ", "2346"].concat(),
                ["2341", " 2341 ", "2347"].concat(),
            ),
            (
                "de-steuer-id",
                ["86", " 095 742 ", "719"].concat(),
                ["86", " 095 742 ", "718"].concat(),
            ),
            (
                "es-dni",
                ["1234", "5678Z"].concat(),
                ["1234", "5678A"].concat(),
            ),
            (
                "es-dni",
                ["X123", "4567L"].concat(),
                ["X123", "4567M"].concat(),
            ),
            (
                "it-codice-fiscale",
                ["RSSMRA", "85T10A562S"].concat(),
                ["RSSMRA", "85T10A562T"].concat(),
            ),
            (
                "br-cpf",
                ["529.982", ".247-25"].concat(),
                ["529.982", ".247-24"].concat(),
            ),
            (
                "nl-bsn",
                ["BSN: ", "1112", "22333"].concat(),
                ["BSN: ", "1112", "22334"].concat(),
            ),
        ]
    }

    #[test]
    fn a_planted_iban_and_one_valid_id_of_each_kind_are_found_and_a_broken_check_is_not() {
        for (rule, value, broken) in personal() {
            let line = format!("customer = \"{value}\"");
            assert_eq!(rules(&line), vec![rule], "{}", masked(&value));
            let line = format!("customer = \"{broken}\"");
            assert_eq!(rules(&line), Vec::<&str>::new(), "{}", masked(&broken));
        }
    }

    #[test]
    fn an_id_is_found_as_the_number_and_not_the_word_in_front_of_it() {
        let line = ["Burgerservicenummer: ", "1112", "22333"].concat();
        let found = leaks(&line);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(&line[found[0].span.clone()], ["1112", "22333"].concat());
        // Its other spellings: the dotted one bare, an unspaced one after its name.
        for line in [
            ["1112", ".22.333"].concat(),
            ["CPF ", "5299822", "4725"].concat(),
            ["Aadhaar no. ", "234123", "412346"].concat(),
            ["SIN# ", "046454", "286"].concat(),
            ["Steuer-ID: ", "860957", "42719"].concat(),
            ["IdNr ", "860957", "42719"].concat(),
            ["AB", "123456C"].concat(),
        ] {
            assert_eq!(leaks(&line).len(), 1, "{}", masked(&line));
        }
    }

    #[test]
    fn a_number_that_only_passes_a_check_is_not_an_id_without_its_spelling() {
        for line in [
            // Digits alone, which a timestamp or a row id can be, with no name in front.
            ["0464", "54286"].concat(),
            ["86095", "742719"].concat(),
            ["23412341", "2346"].concat(),
            ["5299822", "4725"].concat(),
            // A valid group inside a longer run of groups is a part of something else.
            ["046 454 ", "286 111"].concat(),
            ["9 2341 2341 ", "2346"].concat(),
            // `sin` the function, not the number.
            ["let y = sin(", "046454286", ");"].concat(),
            // An IBAN's shape under a country that has none, and one a digit short for its
            // country, each passing mod-97.
            ["ZZ22", "370400440532013000"].concat(),
            ["DE51", "37040044053201300"].concat(),
            // Steuer-IDs that pass their check digit with no digit repeated, or one digit
            // repeated ten times: neither is issued.
            ["12 345 ", "678 903"].concat(),
            ["11 111 ", "111 119"].concat(),
            // A CPF of one digit repeated passes its checks and is never issued.
            ["111.111", ".111-11"].concat(),
            // NINO's suffix is A to D, and BG/GB/NK/KN/TN/NT/ZZ are never issued.
            ["AB", "123456E"].concat(),
            ["GB", "123456A"].concat(),
            // Lower case is not how any of them is written.
            ["rssmra", "85t10a562s"].concat(),
        ] {
            assert_eq!(rules(&line), Vec::<&str>::new(), "{}", masked(&line));
        }
    }

    /// `(gitleaks' rule id, its positive example, the rule here that finds it)`, for every
    /// gitleaks default rule whose match starts on, or is held by, a fixed vendor marker. The
    /// example is gitleaks' own where its test spells one, and otherwise the shape its test
    /// generates: the prefix and a body of the rule's own alphabet and length.
    fn anchored() -> Vec<(&'static str, String, &'static str)> {
        let b = body;
        vec![
            (
                "1password-secret-key",
                [
                    "A3-",
                    &b(UPPER, 6),
                    "-",
                    &b(UPPER, 11),
                    "-",
                    &b(UPPER, 5),
                    "-",
                    &b(UPPER, 5),
                    "-",
                    &b(UPPER, 5),
                ]
                .concat(),
                "1password-secret-key",
            ),
            (
                "1password-service-account-token",
                ["ops_", "eyJ", &b(ALNUM, 260)].concat(),
                "1password-service-account-token",
            ),
            (
                "adobe-client-secret",
                ["p8e-", &b(HEX, 32)].concat(),
                "adobe-client-secret",
            ),
            (
                "age-secret-key",
                ["AGE-SECRET-", "KEY-1", &"Q".repeat(58)].concat(),
                "age-secret-key",
            ),
            (
                "airtable-personnal-access-token",
                ["pat", &b(ALNUM, 14), ".", &b(HEX, 64)].concat(),
                "airtable-personal-access-token",
            ),
            (
                "alibaba-access-key-id",
                ["LTAI", &b(HEX, 20)].concat(),
                "alibaba-access-key-id",
            ),
            (
                "anthropic-api-key",
                ["sk-", "ant-api03-", &b(ALNUM, 93), "AA"].concat(),
                "forge-token",
            ),
            (
                "anthropic-admin-api-key",
                ["sk-", "ant-admin01-", &b(ALNUM, 93), "AA"].concat(),
                "forge-token",
            ),
            (
                "artifactory-api-key",
                ["AKCp", &b(ALNUM, 69)].concat(),
                "artifactory-api-key",
            ),
            (
                "artifactory-reference-token",
                ["cmVmd", &b(ALNUM, 59)].concat(),
                "artifactory-reference-token",
            ),
            (
                "authress-service-client-access-key",
                [
                    "sc_",
                    &b(LOWER, 12),
                    ".",
                    &b(LOWER, 5),
                    ".acc_",
                    &b(LOWER, 16),
                    ".",
                    &b(ALNUM, 40),
                ]
                .concat(),
                "authress-service-client-access-key",
            ),
            (
                "aws-access-token",
                ["AKIALALE", "MEL33243OLIB"].concat(),
                "aws-access-key",
            ),
            (
                "aws-access-token",
                ["ABIA", &b("QRSTVWXYZ234567", 16)].concat(),
                "aws-access-token",
            ),
            (
                "aws-access-token",
                ["ACCA", &b("QRSTVWXYZ234567", 16)].concat(),
                "aws-access-token",
            ),
            (
                "aws-access-token",
                ["A3T", "X", &b("QRSTVWXYZ234567", 16)].concat(),
                "aws-access-token",
            ),
            (
                "aws-amazon-bedrock-api-key-long-lived",
                ["ABSK", "QmVkcm9ja0FQSUtleS1", &b(ALNUM, 108), "="].concat(),
                "aws-amazon-bedrock-api-key",
            ),
            (
                "aws-amazon-bedrock-api-key-short-lived",
                ["bedrock-api-key-", "YmVkcm9jay5hbWF6b25hd3MuY29t"].concat(),
                "aws-amazon-bedrock-api-key",
            ),
            (
                "azure-ad-client-secret",
                [
                    "client_secret=",
                    "bP88Q~",
                    "rcBcYjzzOhg1Hnn76Wm3jGgakZiZ.8vMgR",
                ]
                .concat(),
                "azure-ad-client-secret",
            ),
            (
                "clickhouse-cloud-api-secret-key",
                ["4b1d", "bRdW3rOcB7xLthrM4BTBGK1qPLkHigpN1bXD6z"].concat(),
                "clickhouse-cloud-api-secret-key",
            ),
            (
                "clojars-api-token",
                ["CLOJARS_", &b(LOWER, 60)].concat(),
                "clojars-api-token",
            ),
            (
                "cloudflare-origin-ca-key",
                ["v1.0-", &b(HEX, 24), "-", &b(HEX, 146)].concat(),
                "cloudflare-origin-ca-key",
            ),
            (
                "databricks-api-token",
                ["dapi", &b(HEX, 32)].concat(),
                "databricks-token",
            ),
            (
                "digitalocean-access-token",
                ["doo_v1_", &b(HEX, 64)].concat(),
                "digitalocean-token",
            ),
            (
                "digitalocean-pat",
                ["dop_v1_", &b(HEX, 64)].concat(),
                "digitalocean-token",
            ),
            (
                "digitalocean-refresh-token",
                ["dor_v1_", &b(HEX, 64)].concat(),
                "digitalocean-token",
            ),
            (
                "doppler-api-token",
                ["dp.pt.", &b(ALNUM, 43)].concat(),
                "doppler-token",
            ),
            (
                "duffel-api-token",
                ["duffel_", "test_", &b(ALNUM, 43)].concat(),
                "duffel-api-token",
            ),
            (
                "dynatrace-api-token",
                ["dt0c01.", &b(UPPER, 24), ".", &b(ALNUM, 64)].concat(),
                "dynatrace-api-token",
            ),
            (
                "easypost-api-token",
                ["EZAK", &b(ALNUM, 54)].concat(),
                "easypost-api-token",
            ),
            (
                "easypost-test-api-token",
                ["EZTK", &b(ALNUM, 54)].concat(),
                "easypost-api-token",
            ),
            (
                "facebook-page-access-token",
                ["EAAM", &b(ALNUM, 110)].concat(),
                "facebook-page-access-token",
            ),
            (
                "flutterwave-public-key",
                ["FLWPUBK_", "TEST-", &b(HEX, 32), "-X"].concat(),
                "flutterwave-key",
            ),
            (
                "flutterwave-secret-key",
                ["FLWSECK_", "TEST-", &b(HEX, 32), "-X"].concat(),
                "flutterwave-key",
            ),
            (
                "flutterwave-encryption-key",
                ["FLWSECK_", "TEST-", &b(HEX, 12)].concat(),
                "flutterwave-key",
            ),
            (
                "flyio-access-token",
                ["fo1_", &b(ALNUM, 43)].concat(),
                "flyio-access-token",
            ),
            (
                "flyio-access-token",
                ["fm2_", &b(ALNUM, 120)].concat(),
                "flyio-access-token",
            ),
            (
                "frameio-api-token",
                ["fio-u-", &b(ALNUM, 64)].concat(),
                "frameio-api-token",
            ),
            (
                "gcp-api-key",
                ["AIza", &b(ALNUM, 35)].concat(),
                "forge-token",
            ),
            (
                "github-app-token",
                ["ghs", "_", &b(ALNUM, 36)].concat(),
                "forge-token",
            ),
            (
                "github-fine-grained-pat",
                ["github", "_pat_", &b(ALNUM, 82)].concat(),
                "forge-token",
            ),
            (
                "github-oauth",
                ["gho", "_", &b(ALNUM, 36)].concat(),
                "forge-token",
            ),
            (
                "github-pat",
                ["ghp", "_", &b(ALNUM, 36)].concat(),
                "forge-token",
            ),
            (
                "github-refresh-token",
                ["ghr", "_", &b(ALNUM, 36)].concat(),
                "forge-token",
            ),
            (
                "gitlab-cicd-job-token",
                ["glcbt-", "ab12_", &b(ALNUM, 20)].concat(),
                "forge-token",
            ),
            (
                "gitlab-deploy-token",
                ["gldt-", &b(ALNUM, 20)].concat(),
                "forge-token",
            ),
            (
                "gitlab-feature-flag-client-token",
                ["glffct-", &b(ALNUM, 20)].concat(),
                "forge-token",
            ),
            (
                "gitlab-feed-token",
                ["glft-", &b(ALNUM, 20)].concat(),
                "forge-token",
            ),
            (
                "gitlab-incoming-mail-token",
                ["glimt-", &b(ALNUM, 25)].concat(),
                "forge-token",
            ),
            (
                "gitlab-kubernetes-agent-token",
                ["glagent-", &b(ALNUM, 50)].concat(),
                "forge-token",
            ),
            (
                "gitlab-oauth-app-secret",
                ["gloas-", &b(ALNUM, 64)].concat(),
                "forge-token",
            ),
            (
                "gitlab-pat",
                ["glpat-", &b(ALNUM, 20)].concat(),
                "forge-token",
            ),
            (
                "gitlab-pat-routable",
                ["glpat-", &b(ALNUM, 30), ".01", &b(LOWER, 7)].concat(),
                "forge-token",
            ),
            (
                "gitlab-ptt",
                ["glptt-", &b(HEX, 40)].concat(),
                "forge-token",
            ),
            (
                "gitlab-rrt",
                ["GR1348", "941", &b(ALNUM, 20)].concat(),
                "gitlab-rrt",
            ),
            (
                "gitlab-runner-authentication-token",
                ["glrt-", &b(ALNUM, 20)].concat(),
                "forge-token",
            ),
            (
                "gitlab-runner-authentication-token-routable",
                ["glrt-", "t1_", &b(ALNUM, 30), ".01", &b(LOWER, 7)].concat(),
                "forge-token",
            ),
            (
                "gitlab-scim-token",
                ["glsoat-", &b(ALNUM, 20)].concat(),
                "forge-token",
            ),
            (
                "gitlab-session-cookie",
                ["_gitlab_", "session=", &b(LOWER, 32)].concat(),
                "gitlab-session-cookie",
            ),
            (
                "grafana-api-key",
                ["eyJrIjoi", &b(ALNUM, 80)].concat(),
                "grafana-api-key",
            ),
            (
                "grafana-cloud-api-token",
                ["glc_", &b(ALNUM, 40)].concat(),
                "grafana-token",
            ),
            (
                "grafana-service-account-token",
                ["glsa_", &b(ALNUM, 32), "_", &b(HEX, 8)].concat(),
                "grafana-token",
            ),
            (
                "harness-api-key",
                [
                    "pat.",
                    &b(ALNUM, 22),
                    ".",
                    &b(ALNUM, 24),
                    ".",
                    &b(ALNUM, 20),
                ]
                .concat(),
                "harness-api-key",
            ),
            (
                "hashicorp-tf-api-token",
                [&b(HEX, 14), ".atlas", "v1.", &b(ALNUM, 64)].concat(),
                "hashicorp-tf-api-token",
            ),
            (
                "heroku-api-key-v2",
                ["HRKU-", "AA", &b(ALNUM, 58)].concat(),
                "heroku-api-key-v2",
            ),
            (
                "huggingface-access-token",
                ["hf_", &b(LETTERS, 34)].concat(),
                "forge-token",
            ),
            (
                "huggingface-organization-api-token",
                ["api_org_", &b(LETTERS, 34)].concat(),
                "huggingface-organization-api-token",
            ),
            (
                "infracost-api-token",
                ["ico-", &b(ALNUM, 32)].concat(),
                "infracost-api-token",
            ),
            (
                "intra42-client-secret",
                ["s-s4t2ud-", &b(HEX, 64)].concat(),
                "intra42-client-secret",
            ),
            (
                "linear-api-key",
                ["lin_api_", &b(ALNUM, 40)].concat(),
                "linear-key",
            ),
            (
                "maxmind-license-key",
                ["w5fruZ_8ZUsgYLu8vwgb3", "yKsgMna3uIF9Oa4_mmk"].concat(),
                "maxmind-license-key",
            ),
            (
                "microsoft-teams-webhook",
                [
                    "https://mycompany.webhook.office.com/webhookb2/",
                    &b(LOWER, 8),
                    "-",
                    &b(LOWER, 4),
                    "-",
                    &b(LOWER, 4),
                    "-",
                    &b(LOWER, 4),
                    "-",
                    &b(LOWER, 12),
                    "@",
                    &b(LOWER, 8),
                    "-",
                    &b(LOWER, 4),
                    "-",
                    &b(LOWER, 4),
                    "-",
                    &b(LOWER, 4),
                    "-",
                    &b(LOWER, 12),
                    "/IncomingWebhook/",
                    &b(LOWER, 32),
                    "/",
                    &b(LOWER, 8),
                    "-",
                    &b(LOWER, 4),
                    "-",
                    &b(LOWER, 4),
                    "-",
                    &b(LOWER, 4),
                    "-",
                    &b(LOWER, 12),
                ]
                .concat(),
                "microsoft-teams-webhook",
            ),
            (
                "notion-api-token",
                ["ntn_", "456476151729", "vWBETTAc421EJdkefwPvw8dfNt2oszUa7v"].concat(),
                "notion-api-token",
            ),
            (
                "npm-access-token",
                ["npm_", &b(ALNUM, 36)].concat(),
                "forge-token",
            ),
            (
                "octopus-deploy-api-key",
                ["API-", &b(UPPER, 26)].concat(),
                "octopus-deploy-api-key",
            ),
            (
                "openai-api-key",
                ["sk-", "proj-", &b(ALNUM, 74), "T3Blbk", "FJ", &b(ALNUM, 74)].concat(),
                "forge-token",
            ),
            (
                "openshift-user-token",
                ["sha256~", &b(ALNUM, 43)].concat(),
                "openshift-user-token",
            ),
            (
                "perplexity-api-key",
                ["pplx-", &b(ALNUM, 48)].concat(),
                "perplexity-api-key",
            ),
            (
                "planetscale-api-token",
                ["pscale_", "tkn_", &b(ALNUM, 40)].concat(),
                "planetscale-token",
            ),
            (
                "planetscale-oauth-token",
                ["pscale_", "oauth_", &b(ALNUM, 40)].concat(),
                "planetscale-token",
            ),
            (
                "planetscale-password",
                ["pscale_", "pw_", &b(ALNUM, 40)].concat(),
                "planetscale-token",
            ),
            (
                "postman-api-token",
                ["PMAK-", &b(HEX, 24), "-", &b(HEX, 34)].concat(),
                "postman-key",
            ),
            (
                "prefect-api-token",
                ["pnu_", &b(ALNUM, 36)].concat(),
                "prefect-api-token",
            ),
            (
                "private-key",
                ["-----BEGIN ", "PRIVATE KEY-----", &b(ALNUM, 64)].concat(),
                "private-key",
            ),
            (
                "pulumi-api-token",
                ["pul-", &b(HEX, 40)].concat(),
                "pulumi-api-token",
            ),
            (
                "pypi-upload-token",
                ["pypi-", "AgEIcHlwaS5vcmc", &b(ALNUM, 60)].concat(),
                "forge-token",
            ),
            (
                "readme-api-token",
                ["rdme_", &b(LOWER, 70)].concat(),
                "readme-api-token",
            ),
            (
                "rubygems-api-token",
                ["rubygems_", &b(HEX, 48)].concat(),
                "rubygems-api-token",
            ),
            (
                "scalingo-api-token",
                ["tk-us-", &b(ALNUM, 48)].concat(),
                "scalingo-api-token",
            ),
            (
                "sendgrid-api-token",
                ["SG.", &b(ALNUM, 66)].concat(),
                "sendgrid-key",
            ),
            (
                "sendinblue-api-token",
                ["xkeysib-", &b(HEX, 64), "-", &b(ALNUM, 16)].concat(),
                "sendinblue-api-token",
            ),
            (
                "sentry-org-token",
                [
                    "sntrys_",
                    "eyJpYXQiO",
                    &b(ALNUM, 20),
                    "LCJyZWdpb25fdXJs",
                    &b(ALNUM, 20),
                    "_",
                    &b(ALNUM, 43),
                ]
                .concat(),
                "sentry-token",
            ),
            (
                "sentry-user-token",
                ["sntryu_", &b(HEX, 64)].concat(),
                "sentry-token",
            ),
            (
                "settlemint-application-access-token",
                ["sm_aat_", &b(ALNUM, 16)].concat(),
                "settlemint-token",
            ),
            (
                "settlemint-personal-access-token",
                ["sm_pat_", &b(ALNUM, 16)].concat(),
                "settlemint-token",
            ),
            (
                "settlemint-service-access-token",
                ["sm_sat_", &b(ALNUM, 16)].concat(),
                "settlemint-token",
            ),
            (
                "shippo-api-token",
                ["shippo_", "live_", &b(HEX, 40)].concat(),
                "shippo-api-token",
            ),
            (
                "shopify-access-token",
                ["shpat_", &b(HEX, 32)].concat(),
                "shopify-token",
            ),
            (
                "shopify-custom-access-token",
                ["shpca_", &b(HEX, 32)].concat(),
                "shopify-token",
            ),
            (
                "shopify-private-app-access-token",
                ["shppa_", &b(HEX, 32)].concat(),
                "shopify-token",
            ),
            (
                "shopify-shared-secret",
                ["shpss_", &b(HEX, 32)].concat(),
                "shopify-token",
            ),
            (
                "sidekiq-sensitive-url",
                ["https://cafebabe:", "deadbeef@gems", ".contribsys.com/"].concat(),
                "sidekiq-sensitive-url",
            ),
            (
                "slack-app-token",
                ["xapp-", "1-A1-1-a1"].concat(),
                "slack-token",
            ),
            (
                "slack-bot-token",
                ["xoxb-", "1234567890-", "1234567890", &b(ALNUM, 24)].concat(),
                "forge-token",
            ),
            (
                "slack-config-access-token",
                ["xoxe.", "xoxp-", "1-", &b(UPPER, 163)].concat(),
                "forge-token",
            ),
            (
                "slack-config-refresh-token",
                ["xoxe-", "1-", &b(UPPER, 146)].concat(),
                "slack-token",
            ),
            (
                "slack-legacy-bot-token",
                ["xoxb-", "12345678-", &b(ALNUM, 20)].concat(),
                "forge-token",
            ),
            (
                "slack-legacy-token",
                ["xoxo-", "1-2-3-", &b(HEX, 12)].concat(),
                "slack-token",
            ),
            (
                "slack-legacy-workspace-token",
                ["xoxa-", &b(ALNUM, 8)].concat(),
                "slack-token",
            ),
            (
                "slack-user-token",
                ["xoxe-", "1234567890-1234567890-1234567890-", &b(ALNUM, 30)].concat(),
                "slack-token",
            ),
            (
                "slack-webhook-url",
                ["https://hooks.slack.com/", "services/", &b(ALNUM, 44)].concat(),
                "slack-webhook",
            ),
            (
                "sourcegraph-access-token",
                [
                    "sgp_",
                    "AaD80dc6E02eCAE1_",
                    "d3cba16CC0F18fA14A2EFB61CbDFceEBf9fAD16b",
                ]
                .concat(),
                "sourcegraph-access-token",
            ),
            (
                "square-access-token",
                ["sq0atp-", &b(ALNUM, 22)].concat(),
                "square-access-token",
            ),
            (
                "stripe-access-token",
                ["rk_", "test_", &b(ALNUM, 10)].concat(),
                "stripe-access-token",
            ),
            (
                "twilio-api-key",
                ["SK", &b(HEX, 32)].concat(),
                "twilio-api-key",
            ),
            (
                "vault-batch-token",
                ["hvb.", &b(ALNUM, 140)].concat(),
                "vault-token",
            ),
            (
                "vault-service-token",
                ["hvs.", &b(ALNUM, 100)].concat(),
                "vault-token",
            ),
        ]
    }

    #[test]
    fn every_anchored_gitleaks_rule_finds_gitleaks_own_example() {
        for (gitleaks, example, rule) in anchored() {
            let line = format!("value = \"{example}\"");
            let at = line.find(&example).expect("the example is on its line");
            let found = leaks(&line);
            assert!(
                found.iter().any(|leak| leak.rule == rule
                    && leak.span.start < at + example.len()
                    && leak.span.end > at),
                "gitleaks' {gitleaks} was not found by {rule}: {:?}",
                found.iter().map(|l| l.rule).collect::<Vec<_>>()
            );
        }
    }

    /// `(rule id, a near-miss: the vendor's marker with a body that is not its token)`, one per
    /// rule this file adds for gitleaks: a body too short, the wrong case, or the marker inside
    /// a longer word.
    fn near_misses() -> Vec<(&'static str, String)> {
        let b = body;
        vec![
            (
                "1password-secret-key",
                ["A3-", &b(UPPER, 6), "-", &b(UPPER, 11), "-", &b(UPPER, 5)].concat(),
            ),
            (
                "1password-service-account-token",
                ["ops_", "eyJ", &b(ALNUM, 100)].concat(),
            ),
            ("adobe-client-secret", ["p8e-", &b(HEX, 20)].concat()),
            (
                "age-secret-key",
                ["AGE-SECRET-", "KEY-1", &"Q".repeat(40)].concat(),
            ),
            (
                "airtable-personal-access-token",
                ["pat", &b(ALNUM, 14), ".", &b(HEX, 30)].concat(),
            ),
            ("alibaba-access-key-id", ["LTAI", &b(HEX, 10)].concat()),
            ("artifactory-api-key", ["AkCp", &b(ALNUM, 69)].concat()),
            (
                "artifactory-reference-token",
                ["cmVMd", &b(ALNUM, 59)].concat(),
            ),
            (
                "authress-service-client-access-key",
                [
                    "sc_",
                    &b(LOWER, 12),
                    ".",
                    &b(LOWER, 5),
                    ".ACC_",
                    &b(LOWER, 16),
                    ".",
                    &b(ALNUM, 40),
                ]
                .concat(),
            ),
            (
                "aws-access-token",
                ["ABIA", &b("qrstvwxyz234567", 16)].concat(),
            ),
            (
                "aws-amazon-bedrock-api-key",
                ["ABSK", &b(ALNUM, 60)].concat(),
            ),
            (
                "azure-ad-client-secret",
                ["client_secret=", "bP88Q~", &b(ALNUM, 12)].concat(),
            ),
            (
                "clickhouse-cloud-api-secret-key",
                ["adf4b1d", &b(ALNUM, 38)].concat(),
            ),
            ("clojars-api-token", ["CLOJARS_", &b(LOWER, 30)].concat()),
            (
                "cloudflare-origin-ca-key",
                ["v1.0-", &b(HEX, 24), "-", &b(HEX, 60)].concat(),
            ),
            (
                "duffel-api-token",
                ["duffel_", "prod_", &b(ALNUM, 43)].concat(),
            ),
            (
                "dynatrace-api-token",
                ["dt0c01.", &b(UPPER, 24), ".", &b(ALNUM, 30)].concat(),
            ),
            ("easypost-api-token", ["EZAK", &b(ALNUM, 30)].concat()),
            (
                "facebook-page-access-token",
                ["EAAM", &b(ALNUM, 50)].concat(),
            ),
            (
                "flutterwave-key",
                ["FLWSECK_", "LIVE-", &b(HEX, 32), "-X"].concat(),
            ),
            ("flyio-access-token", ["fo1_", &b(ALNUM, 20)].concat()),
            ("frameio-api-token", ["fio-u-", &b(ALNUM, 30)].concat()),
            ("gitlab-rrt", ["GR1348", "941", &b(ALNUM, 10)].concat()),
            (
                "gitlab-session-cookie",
                ["_gitlab_", "session=", &b(LOWER, 10)].concat(),
            ),
            ("grafana-api-key", ["eyJrIjoi", &b(ALNUM, 30)].concat()),
            (
                "harness-api-key",
                ["pat.", &b(ALNUM, 22), ".", &b(ALNUM, 24)].concat(),
            ),
            (
                "hashicorp-tf-api-token",
                [&b(HEX, 14), ".ATLAS", "V1.", &b(ALNUM, 64)].concat(),
            ),
            ("heroku-api-key-v2", ["HRKU-", "AA", &b(ALNUM, 20)].concat()),
            (
                "huggingface-organization-api-token",
                ["api_org_", &b(LETTERS, 10)].concat(),
            ),
            ("infracost-api-token", ["ico-", &b(ALNUM, 12)].concat()),
            ("intra42-client-secret", ["s-s4t2xx-", &b(HEX, 64)].concat()),
            (
                "maxmind-license-key",
                [&b(ALNUM, 6), "_", &b(ALNUM, 29), "_mmx"].concat(),
            ),
            (
                "microsoft-teams-webhook",
                "https://mycompany.webhook.office.com/webhookb2/".to_owned(),
            ),
            ("notion-api-token", ["ntn_", "12345678901", "abc"].concat()),
            ("octopus-deploy-api-key", ["API-", &b(UPPER, 32)].concat()),
            ("openshift-user-token", ["sha256~", &b(ALNUM, 20)].concat()),
            ("perplexity-api-key", ["pplx-", &b(ALNUM, 20)].concat()),
            (
                "planetscale-token",
                ["pscale_", "tkn_", &b(ALNUM, 10)].concat(),
            ),
            ("prefect-api-token", ["pnu_", &b(ALNUM, 20)].concat()),
            ("pulumi-api-token", ["pul-", &b(HEX, 20)].concat()),
            ("readme-api-token", ["rdme_", &b(LOWER, 30)].concat()),
            ("rubygems-api-token", ["rubygems_", &b(HEX, 20)].concat()),
            ("scalingo-api-token", ["tk-us-", &b(ALNUM, 20)].concat()),
            ("sendinblue-api-token", ["xkeysib-", &b(HEX, 30)].concat()),
            (
                "sentry-token",
                ["sntrys_", &b(ALNUM, 90), "_", &b(ALNUM, 43)].concat(),
            ),
            ("settlemint-token", ["sm_pat_", &b(ALNUM, 8)].concat()),
            (
                "shippo-api-token",
                ["shippo_", "prod_", &b(HEX, 40)].concat(),
            ),
            (
                "sidekiq-sensitive-url",
                "https://gems.contribsys.com/".to_owned(),
            ),
            ("slack-token", ["xoxo-", "abc"].concat()),
            ("sourcegraph-access-token", ["sgp_", &b(HEX, 20)].concat()),
            ("square-access-token", ["sq0atp-", &b(ALNUM, 10)].concat()),
            (
                "stripe-access-token",
                ["rk_", "test_", &b(ALNUM, 5)].concat(),
            ),
            ("twilio-api-key", ["SK", &b(HEX, 20)].concat()),
            ("vault-token", ["hvs.", &b(ALNUM, 40)].concat()),
        ]
    }

    #[test]
    fn a_near_miss_of_each_new_vendor_rule_is_not_refused() {
        let added: Vec<&str> = near_misses().iter().map(|(rule, _)| *rule).collect();
        for (rule, line) in near_misses() {
            let line = format!("value = \"{line}\"");
            assert_eq!(
                rules(&line),
                Vec::<&str>::new(),
                "{rule}: {}",
                masked(&line)
            );
        }
        // Each rule this file adds has its near-miss here, and each near-miss names a rule.
        let ids: Vec<&str> = leak_rules().iter().map(|(id, _)| *id).collect();
        for rule in &added {
            assert!(ids.contains(rule), "{rule} is not a rule");
        }
    }

    #[test]
    fn ordinary_code_is_still_not_a_leak_under_the_wider_rules() {
        for line in [
            "let api = format!(\"API-{}\", version);",
            "let sk = SK_DEFAULT; // twilio",
            "fn sin(x: f64) -> f64 { x.sin() }",
            "SELECT id FROM accounts WHERE iban IS NOT NULL;",
            "const BSN_LENGTH: usize = 9;",
            "let pattern = \"sha256~\";",
            "https://hooks.slack.com/services/",
            "version 1.0-beta",
            "let hash = \"4b1d\";",
            "let id = 12345678;",
            "pat.name = value.trim();",
        ] {
            assert_eq!(rules(line), Vec::<&str>::new(), "{line}");
        }
    }

    #[test]
    fn a_refusal_shows_an_id_masked_and_never_whole() {
        for (_, value, _) in personal() {
            let shown = masked(&value);
            assert!(!shown.contains(&value), "{shown}");
            let digits: String = value.chars().filter(char::is_ascii_alphanumeric).collect();
            let kept: String = shown.chars().filter(char::is_ascii_alphanumeric).collect();
            assert!(kept.len() <= 4 && digits.len() > kept.len(), "{shown}");
        }
    }
}
