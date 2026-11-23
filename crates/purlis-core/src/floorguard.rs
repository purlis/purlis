// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Arm A4: the release floor — an unattended run may not publish (#299).
//!
//! A port of `charter/hooks.py`'s `_release_floor_reason`, `_PUBLISH_FORGE` and `_TAG_HARMLESS`.
//! Its `_unattended` and `UNATTENDED_MODE` are the attendance fact's now
//! ([`crate::attendance::says_prompts_off`], [`crate::attendance::PROMPTS_OFF`]; T1-22, #6413):
//! whether the floor refuses is the caller's reading of the fact ([`crate::toolgate`]). Everything under it is already standing:
//! [`crate::shellseg`] reads the line, [`crate::shellwrap::split_env`] names each segment's
//! program, [`crate::credguard::git_subcommand`] names git's verb and
//! [`crate::leakguard::is_charter`] recognises charter however it was spelled.
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
//! # Why this exists at all
//!
//! 0.46.0 taught `_ask` to fall back to `allow` under `bypassPermissions` so a workflow nudge
//! cannot hang an unattended run. That is right for a nudge, and it silently removed the only
//! thing standing between an autonomous agent and an irreversible release: the clone-commit
//! guard's `ask` used to floor the host's decision at a prompt in every permission mode. It
//! stopped things by accident. Now it allows them.
//!
//! **Deny, not ask**, deliberately: an unattended ask is now an allow, so an ask here would be
//! indistinguishable from no guard. Deny is also the only verdict that cannot hang.
//!
//! **Attended is untouched.** A guard that made releases harder for the operator is the cage the
//! plane-root guard warns about, and the fix people reach for then is to switch it off for good.
//!
//! # Two decisions the port made about the Python it came from
//!
//! * **The `_TAG_HARMLESS` arm clears its SEGMENT, not the command line** (#348). The Python
//!   `return None`d out of the whole walk, so `git tag -l` standing anywhere BEFORE a publish
//!   cleared the floor for it: `git tag -l && gh release create v1.0.0` was allowed unattended
//!   while `gh release create v1.0.0` alone was denied. That was a live fail-open (filed
//!   upstream as charter#1172), and every sibling guard already `continue`s past a segment that
//!   is not its business. This port `continue`s, which moved the recorded answer of the
//!   read-first row in `fixtures/corpora/shellseg-oracle.jsonl` on purpose (ADR 0046).
//! * **`is_charter` rather than `base == "charter"`**, so `purlis change land` (the name it
//!   ships as since RN-3), `edm change land` (the pre-rename binary) and `python3 -m charter
//!   change land` are the same command here as they are to the leak guard. Some spellings put
//!   charter's own NAME in `words` instead of in `prog`, which is why the leading name is
//!   dropped from `words` before the table is read. The table entry alone would have been a
//!   dead line: the lookup used to live under `elif base in ("gh", "glab")`.

mod merge;
mod token;

use crate::credguard::git_subcommand;
use crate::leakguard::{self, CHARTER_PROGS};
use crate::livesub;
use crate::shellseg;
use crate::shellwrap::{self, base_lower};

use std::sync::OnceLock;

use regex::Regex;

/// CLI `(program, noun, verb)` triples that publish or land code — `_PUBLISH_FORGE`.
///
/// `gh pr merge` and `gh release create` are no kind of `git`, so the git-write pattern never saw
/// them and nothing else did either — they were unguarded in every mode.
///
/// **`charter change land` is here.** That command merges one member of a cross-repo change,
/// which is the same act `gh pr merge` is: the floor already sits between *opening* a request and
/// *merging* one — `gh pr create` is deliberately absent — and a charter verb that merged would
/// be a documented way around a floor charter itself wrote. A cross-repo landing is N merges,
/// each individually revertible, so it is not treated as a release; the split is attended versus
/// unattended, exactly as it already is for `gh pr merge`.
pub const PUBLISH_FORGE: [(&str, &str, &str); 5] = [
    ("gh", "release", "create"),
    ("gh", "pr", "merge"),
    ("glab", "release", "create"),
    ("glab", "mr", "merge"),
    ("charter", "change", "land"),
];

/// **The same floor, for verbs newer than the frozen recording** of [`PUBLISH_FORGE`], which the
/// corpus replays row by row. `charter skill publish` sends a person's skill to a forge (#2781),
/// and the command itself refuses without a terminal or inside a chat. This denies it earlier.
const PUBLISH_SINCE: [(&str, &str, &str); 1] = [("charter", "skill", "publish")];

/// A forge CLI's `(noun, verb)`.
type ForgeAct = (&'static str, &'static str);

/// Each [`PUBLISH_FORGE`] row of `gh` beside the same act's row for `glab`, as `(noun, verb)`
/// (#1068). Every forge row is in a pair; its test fails on one that is not.
#[cfg_attr(not(test), allow(dead_code))]
const PUBLISH_FORGE_PAIRS: [(ForgeAct, ForgeAct); 2] = [
    (("release", "create"), ("release", "create")),
    (("pr", "merge"), ("mr", "merge")),
];

/// `git tag` flags that only READ or act locally — `_TAG_HARMLESS`.
///
/// An autonomous run legitimately needs to know what the tags are, and deleting a local tag
/// publishes nothing.
pub const TAG_HARMLESS: [&str; 11] = [
    "-l",
    "--list",
    "-d",
    "--delete",
    "-n",
    "--contains",
    "--points-at",
    "--merged",
    "--no-merged",
    "-v",
    "--verify",
];

/// The remedy, identical for every trigger — the opening of every denial this guard returns but
/// one of a command that prints a stored credential, which opens with [`TOKEN_FLOOR_FIX`] (#6314).
pub const RELEASE_FLOOR_FIX: &str = "Publishing is on purlis's floor: a run with nobody watching may not cut a release. \
     `bypassPermissions` means *stop asking me*, not *stop knowing things*. Re-run this step \
     **attended**, or have a person do it. ";

/// **The opening of a refusal of a command that prints a stored credential** (#6314): the forge
/// token, or a secret from the keychain or a password manager. Refused for the same reason a
/// publish is, but it is no release, so it opens with what it is and names the way through:
/// the credential goes from a vault to the one command that needs it, and the run never holds it.
pub const TOKEN_FLOOR_FIX: &str = token::FIX;

/// `v?\d+\.\d+(?:\.\d+)?[\w.-]*`, full-match — what a version tag looks like.
///
/// Two CPython classes are spelled out rather than borrowed:
///
/// * **`\d`** is `\p{Nd}` on a `str` — every Unicode decimal digit — and the `regex` crate's is
///   the same `\p{Nd}`, so it is left as written and the fuzz alphabet carries Unicode digits to
///   measure it.
/// * **`\w`** is NOT the same. CPython's is `str.isalnum() or '_'`; the crate's is UTS#18's,
///   which counts a combining mark, a variation selector and ZWJ. #92 swept all 1,112,064
///   non-surrogate codepoints and found `[\p{L}\p{N}_]` is CPython's, with zero disagreements in
///   either direction, so that is what is written here.
///
/// `re.fullmatch` does NOT allow a trailing newline the way `$` does, so this is `\A…\z`.
fn version_tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\Av?\d+\.\d+(?:\.\d+)?[\p{L}\p{N}_.\-]*\z").expect("the pattern compiles")
    })
}

/// Deny a publish when the host says nobody is watching — `_release_floor_reason`.
///
/// Keyed on TAGGING rather than on a `v*` shape. Shape-matching is narrower and reads more
/// correct — `release.yml` triggers on `v*` — but it is walked past by naming the tag
/// `release-1`, and tagging attended costs nothing. The asymmetry favours bluntness: a false stop
/// costs one re-run, a false pass costs a version number that can never be reused.
///
/// `unattended` is the host's answer, passed in rather than read: the Python takes the whole hook
/// payload and asks [`unattended`] of its `permission_mode`, and the core holds no globals.
pub fn release_floor_reason(cmd: &str, unattended: bool) -> Option<String> {
    metered(cmd, unattended).0
}

/// **The floor's merge half alone** (R-115): the refusal [`release_floor_reason`] gives
/// `cmd` unattended where it merges a request, sets one to merge on its own later, or cannot be
/// read for what it does to a merge, and `None` for every other command, a tag, a release or a
/// read of the forge token included. What the merge hold reads a line as a merge by
/// ([`crate::mergehold`]), in any mode: every route the floor knows is the same act to it.
pub fn merge_reason(cmd: &str) -> Option<String> {
    let _only = OnlyMerges::on();
    metered(cmd, true).0
}

/// **The floor reads for merges alone while this lives** ([`merge_reason`]), and as it did before
/// once it is dropped, a panic's unwinding included.
struct OnlyMerges {
    outer: bool,
}

impl OnlyMerges {
    fn on() -> Self {
        Self {
            outer: ONLY_MERGES.replace(true),
        }
    }
}

impl Drop for OnlyMerges {
    fn drop(&mut self) {
        ONLY_MERGES.set(self.outer);
    }
}

std::thread_local! {
    /// Whether the floor is reading a body line that is read as commands only as a hedge
    /// ([`ReadingAHedge`]): the speculative checks of #7131 are not made on one.
    static READING_A_HEDGE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether the floor is reading a hedged body line.
fn reading_a_hedge() -> bool {
    READING_A_HEDGE.get()
}

/// **The floor reads a hedged body line while this lives**, and as it did before once dropped.
struct ReadingAHedge {
    outer: bool,
}

impl ReadingAHedge {
    fn on() -> Self {
        Self {
            outer: READING_A_HEDGE.replace(true),
        }
    }
}

impl Drop for ReadingAHedge {
    fn drop(&mut self) {
        READING_A_HEDGE.set(self.outer);
    }
}

/// The rows of [`PUBLISH_FORGE`] that merge a request rather than cut a release.
const MERGE_ROWS: [(&str, &str, &str); 3] = [
    ("gh", "pr", "merge"),
    ("glab", "mr", "merge"),
    ("charter", "change", "land"),
];

std::thread_local! {
    /// Whether the floor reads only for merges on this thread ([`merge_reason`]).
    static ONLY_MERGES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether the floor is reading for merges alone ([`merge_reason`]).
fn only_merges() -> bool {
    ONLY_MERGES.get()
}

/// **Reads with `cwd`, the hook payload's working directory, as the directory a relative path in
/// the line is opened against** (#7195): a request edit's `--input body.json` is the file gh
/// opens in the shell's directory, which only the payload knows. Without it, or where the line
/// is not one sole call ([`is_sole_call`]), a body is unread.
pub fn in_directory<R>(cwd: &str, read: impl FnOnce() -> R) -> R {
    let given = (!cwd.is_empty()).then(|| std::path::PathBuf::from(cwd));
    let outer = DIRECTORY.replace(given);
    let _restore = RestoreDirectory(outer);
    read()
}

/// Puts the directory [`in_directory`] replaced back, a panic's unwinding included.
struct RestoreDirectory(Option<std::path::PathBuf>);

impl Drop for RestoreDirectory {
    fn drop(&mut self) {
        DIRECTORY.replace(self.0.take());
    }
}

std::thread_local! {
    /// The directory the hook's payload says the shell is in ([`in_directory`]).
    static DIRECTORY: std::cell::RefCell<Option<std::path::PathBuf>> =
        const { std::cell::RefCell::new(None) };
    /// Whether the line being read is one sole call ([`is_sole_call`], set by [`metered`]).
    static SOLE_CALL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Where a relative path in the line is opened: the payload's directory. Only a line that is
/// one sole call ([`is_sole_call`]) has its bodies read, and no `cd` can come before that.
fn directory_the_line_runs_in() -> Option<std::path::PathBuf> {
    DIRECTORY.with_borrow(Clone::clone)
}

/// Whether the line being read lets a request edit's body be read ([`is_sole_call`]).
fn bodies_are_readable() -> bool {
    SOLE_CALL.get()
}

/// **Whether `cmd` is one `gh` or `glab` call and nothing else** (#7195): one command line (a
/// heredoc's body aside; a backslash and a newline join lines as the shell does), behind env
/// assignments only, with no `;`, `&`, `|`, parenthesis, backtick, `$(`, `${` that is not a
/// parameter's or `$'` outside a single quote, no quote left open and no backslash ending the
/// line. An output redirection (`>f`, `2>&1`, `&>f`) is no command: only a `&` that is not part
/// of one holds. A line that is anything else may run something first (a `cd`, an `eval`, a
/// `source`, a function, an alias) or feed the call another stdin, so a request body is not read
/// from it. Read off the text and failing closed: a doubt is `false`.
fn is_sole_call(cmd: &str) -> bool {
    let layout = crate::heredoc::heredoc_layout(cmd);
    if !layout
        .first()
        .is_some_and(|l| !l.body && !l.text.trim().is_empty())
    {
        return false;
    }
    // The command's own lines, a heredoc's body aside, joined as the shell reads them. A line
    // that ends on a backslash and runs into a body is not read: what joins it is not here.
    let mut lines: Vec<&str> = Vec::new();
    for (i, l) in layout.iter().enumerate() {
        if l.body {
            continue;
        }
        if l.text.ends_with('\\') && layout.get(i + 1).is_some_and(|n| n.body) {
            return false;
        }
        lines.push(&l.text);
    }
    let text: Vec<char> = lines.join("\n").chars().collect();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    // A newline outside a quote ends the command: only blanks may follow it.
    let mut ended = false;
    let mut i = 0usize;
    while i < text.len() {
        let c = text[i];
        let next = text.get(i + 1).copied();
        i += 1;
        if ended && !c.is_whitespace() {
            return false;
        }
        // `${` runs a command in the current shell in bash 5.3 (`${ cmd; }`, `${| cmd; }`),
        // quoted or not: only a parameter's name or a special parameter may follow it.
        if c == '$' && next == Some('{') && quote != Some('\'') {
            let after = text.get(i + 1).copied();
            if !after.is_some_and(|a| a.is_ascii_alphanumeric() || "_#?@*!$-".contains(a)) {
                return false;
            }
        }
        match quote {
            Some('\'') => {
                if c == '\n' {
                    return false;
                }
                quote = (c != '\'').then_some('\'');
                word.push(c);
            }
            Some(_) => match c {
                '`' | '\n' => return false,
                '$' if next == Some('(') => return false,
                '\\' => {
                    // A backslash and a newline are no text at all, in a quote or out of one.
                    if next == Some('\n') {
                        i += 1;
                        continue;
                    }
                    word.push(c);
                    let Some(n) = next else {
                        return false;
                    };
                    i += 1;
                    word.push(n);
                }
                '"' => {
                    quote = None;
                    word.push(c);
                }
                _ => word.push(c),
            },
            None => match c {
                // An output redirection is harmless: `2>&1`, `>&2` and `&>file`, `&>>file`. Any
                // other `&` backgrounds the call or chains it.
                '&' if word.ends_with('>') && next.is_some_and(|n| n.is_ascii_digit()) => {
                    word.push(c);
                }
                '&' if word.is_empty() && next == Some('>') => word.push(c),
                ';' | '&' | '|' | '(' | ')' | '`' => return false,
                '$' if next == Some('\'') => return false,
                '\\' => {
                    if next == Some('\n') {
                        i += 1;
                        continue;
                    }
                    word.push(c);
                    let Some(n) = next else {
                        return false;
                    };
                    i += 1;
                    word.push(n);
                }
                '\'' | '"' => {
                    quote = Some(c);
                    word.push(c);
                }
                '\n' => {
                    if !word.is_empty() {
                        words.push(std::mem::take(&mut word));
                    }
                    ended = true;
                }
                c if c.is_whitespace() => {
                    if !word.is_empty() {
                        words.push(std::mem::take(&mut word));
                    }
                }
                _ => word.push(c),
            },
        }
    }
    if quote.is_some() {
        return false;
    }
    if !word.is_empty() {
        words.push(word);
    }
    let is_assignment = |w: &String| {
        w.split_once('=').is_some_and(|(name, _)| {
            !name.is_empty()
                && !name.starts_with(|c: char| c.is_ascii_digit())
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
    };
    words
        .iter()
        .find(|w| !is_assignment(w))
        .is_some_and(|w| w == "gh" || w == "glab")
}

/// [`release_floor_reason`], and how many bytes of text the floor read on the way, every level
/// counted ([`Work`]).
fn metered(cmd: &str, unattended: bool) -> (Option<String>, usize) {
    let outer = WORK.replace(Some(Work {
        read: 0,
        allowed: cmd.len().saturating_mul(WORK_FACTOR) + WORK_SLACK,
    }));
    let outer_cut = CUT_SHORT.replace(false);
    // A level nested in a reading keeps the outer line's answer: a call inside `bash -c` is not
    // the sole command of the line it stands in.
    let sole = SOLE_CALL.replace(if outer.is_some() {
        SOLE_CALL.get()
    } else {
        is_sole_call(cmd)
    });
    let said = floor(cmd, unattended, 0);
    SOLE_CALL.set(sole);
    // **A reading cut short never ends as an allow** (#7245, round 4): whichever way a limit
    // was crossed (the work meter, the depth, the nesting), it set this flag, and no one clears
    // it but this function. A level that swallowed its own refusal still ends here as one.
    let cut = CUT_SHORT.replace(outer_cut);
    let said = closed_on_a_cutoff(said, cut, unattended);
    let read = WORK.replace(outer).map_or(0, |work| work.read);
    (said, read)
}

/// `said`, or the unreadable refusal where a limit cut the reading short and nothing said so.
fn closed_on_a_cutoff(said: Option<String>, cut: bool, unattended: bool) -> Option<String> {
    said.or_else(|| {
        (unattended && cut).then(|| format!("{RELEASE_FLOOR_FIX}{}", merge::UNREADABLE))
    })
}

std::thread_local! {
    /// Whether any reading on this thread was cut short by a limit since [`metered`] began it.
    /// Set by every limit, cleared by none but `metered`, so a reading that did not finish can
    /// never fall through to "nothing to refuse".
    static CUT_SHORT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// How many times its own length the floor reads of one command, every level it reads in
/// (#1409): a substitution, a shell's script, an alias's expansion, the words fed to a shell.
/// Each level is read whole, so a long line nested deep is read once per level; past this many
/// times its length, plus [`WORK_SLACK`], the line is refused as one the floor cannot read.
/// Keeps the floor's whole reading linear in the line however deep it nests, and fails closed.
pub const WORK_FACTOR: usize = 6;

/// What the floor may read of any command beyond [`WORK_FACTOR`] times its length, so a short
/// command is read to its last level however it nests.
pub const WORK_SLACK: usize = 64 * 1024;

/// The floor's reading of one command so far: the bytes it has read at every level, and how
/// many it may ([`WORK_FACTOR`]).
#[derive(Clone, Copy)]
struct Work {
    read: usize,
    allowed: usize,
}

std::thread_local! {
    /// The command the floor is reading on this thread, metered ([`metered`]). `None` outside
    /// a reading: a level read on its own is not metered.
    static WORK: std::cell::Cell<Option<Work>> = const { std::cell::Cell::new(None) };
}

/// Counts `text` as read at one more level, and whether the floor may still read it.
fn may_read(text: &str) -> bool {
    let may = WORK.with(|work| match work.get() {
        Some(mut now) => {
            now.read = now.read.saturating_add(text.len());
            work.set(Some(now));
            now.read <= now.allowed
        }
        None => true,
    });
    if !may {
        CUT_SHORT.set(true);
    }
    may
}

/// How many commands deep the floor reads a command inside a command — a shell's script, an
/// alias's expansion, the words fed to a shell's stdin (#866).
///
/// Past it the line is refused as one the floor cannot read. No real command nests this deep,
/// and without a bound a line nested thousands deep would exhaust the stack: a hook that dies
/// without answering is an allow. The bound also keeps the cost linear in the line, since each
/// level reads a shorter string than the one around it.
pub const MAX_DEPTH: usize = crate::guardcaps::MAX_COMMAND_DEPTH;

/// [`release_floor_reason`] at `depth` commands in.
pub(crate) fn floor(cmd: &str, unattended: bool, depth: usize) -> Option<String> {
    if !unattended {
        return None;
    }
    let fix = RELEASE_FLOOR_FIX;
    // What an earlier line left of the nesting flag is not this line's.
    nested_too_deep();
    if depth > MAX_DEPTH {
        CUT_SHORT.set(true);
    }
    if depth > MAX_DEPTH || !may_read(cmd) {
        return Some(format!("{fix}{}", merge::UNREADABLE));
    }
    let inner_floor = |inner: &str| floor(inner, unattended, depth + 1);
    // A heredoc body nothing runs is data, and only the substitutions of one the shell
    // expands run (#6293).
    let runs = crate::heredoc::commands_a_shell_runs_without_the_hedge(cmd);
    let mut segments = commands_of(&runs);
    // A `case` pattern's `)` ends the pattern and starts the arm's command (`x) gh pr merge`),
    // which the segment walk keeps as a word: read the line again with its patterns blanked, and
    // the arm's commands are segments of their own (#7131). Only the segments that are new are
    // added: a copy of each would be read again at every depth of an `eval` chain.
    let shell_lines = crate::heredoc::lines_a_shell_runs_without_the_hedge(cmd);
    let (lines, unparsed_case, mut time_option) = in_command_position(&shell_lines);
    if nested_too_deep() {
        return Some(too_nested(fix));
    }
    // Only a line something was blanked out of reads differently, so no other is read twice.
    if lines != shell_lines {
        let mut read: std::collections::HashSet<Vec<String>> = segments.iter().cloned().collect();
        for toks in commands_of(&lines) {
            if read.insert(toks.clone()) {
                segments.push(toks);
            }
        }
    }
    // The body lines read as commands only as bash 3.2's hedge (a `)` in a quoted substitution's
    // heredoc body, `Enclosed::quoted_alone`) are read for a literal merge, and for that alone:
    // both joined with the lines around them, as any line is, and each on its own, so that a
    // quote an earlier line of the body left open cannot hide a merge on a later one. The
    // segments they add are held below, in [`ReadingAHedge`]; the speculative checks of #7131
    // (the program the shell builds, a substitution that could print a command, a `case` not
    // read to its end) are made only on lines a shell runs on every reading (#7131, the ruling
    // on the large commit body).
    // Each line on its own, but a line that ends in a backslash is one with the next.
    let mut hedged_lines: Vec<String> = Vec::new();
    let mut continued = false;
    for line in crate::heredoc::hedged_body_lines(cmd) {
        match hedged_lines.last_mut().filter(|_| continued) {
            Some(last) => {
                last.push('\n');
                last.push_str(&line);
            }
            None => hedged_lines.push(line.clone()),
        }
        continued = crate::heredoc::ends_in_line_continuation(&line);
    }
    // A line read twice reads the same, and a blank one holds no command: each distinct line is
    // read once, however long the body that repeats it (a long body is mostly blank lines and
    // the same few bullets), which keeps a 64 KB body well inside the hook's budget on a slow
    // debug runner too (#7245, round 4).
    let mut distinct = std::collections::HashSet::new();
    hedged_lines.retain(|line| !line.trim().is_empty() && distinct.insert(line.clone()));
    let mut hedge_segments: Vec<Vec<String>> = Vec::new();
    // The segments of each hedged line, to feed a shell on that line and no other line.
    let mut hedged_line_segments: Vec<Vec<Vec<String>>> = Vec::new();
    // A residual, known: each hedged line is read on its own as if it began outside any quote,
    // so a quote one line opens and the next closes is read wrongly by the line-by-line reading.
    // The reading of the whole text above, which joins the lines, is what holds a merge after
    // such a close.
    let mut unparsed_hedge_case = false;
    if !hedged_lines.is_empty() {
        let mut read: std::collections::HashSet<Vec<String>> = segments.iter().cloned().collect();
        let mut add = |toks: Vec<String>| {
            if read.insert(toks.clone()) {
                hedge_segments.push(toks);
            }
        };
        for toks in commands_of(&crate::heredoc::commands_a_shell_runs(cmd)) {
            add(toks);
        }
        let (all_lines, unparsed, time) =
            in_command_position(&crate::heredoc::lines_a_shell_runs(cmd));
        unparsed_hedge_case |= unparsed;
        time_option |= time;
        for toks in commands_of(&all_lines) {
            add(toks);
        }
        for line in &hedged_lines {
            // On its own a line has no `case` before it, so it is read as one of an arm's
            // lines too: `x) gh pr merge 7` is an arm whose command is the merge.
            let (blanked, unparsed, time) = in_command_position(line);
            unparsed_hedge_case |= unparsed;
            time_option |= time;
            // The wrapper closes its own `case` (`esac`, #7245 round 4 ruling). Prose that is no
            // arm (`$(echo gh) pr view 7`, `{gh,pr,view,7}`) cannot be read as one, so its flag
            // is raised only for a line that names a `case` itself: a real one, read to its end.
            let as_an_arm = if line.contains(')') || names_a_case(line) {
                let (arm, unparsed, time) =
                    in_command_position(&format!("case _ in\n{line}\nesac"));
                unparsed_hedge_case |= unparsed && names_a_case(line);
                time_option |= time;
                arm
            } else {
                String::new()
            };
            let mut own = Vec::new();
            // Read again only what differs from the line: nothing blanked, and no `)` to make
            // an arm of, is the line's own reading over.
            let blanked_again = (blanked != *line).then(|| commands_of(&blanked));
            let arm_again = line.contains(')').then(|| commands_of(&as_an_arm));
            for toks in commands_of(line)
                .into_iter()
                .chain(blanked_again.into_iter().flatten())
                .chain(arm_again.into_iter().flatten())
            {
                own.push(toks.clone());
                add(toks);
            }
            hedged_line_segments.push(own);
        }
        if nested_too_deep() {
            return Some(too_nested(fix));
        }
    }
    // A shell that reads its script from stdin is fed by the rest of the line (#866).
    let a_shell_reads_stdin = segments
        .iter()
        .any(|toks| takes_its_script_from_stdin(toks));
    // The hedged lines are fed by a shell on any line of the text, and feed one: a shell they
    // hold reads the lines after it. Only their own segments are read as fed by it, so a shell
    // they name never makes a line a shell runs outright be read as a script.
    // Only a shell named outright counts: a program the shell builds, read as one, is the
    // speculative reading the hedge is spared.
    let the_hedge_reads_stdin = a_shell_reads_stdin
        || hedge_segments.iter().any(|toks| {
            let (prog, _env, argv) = shellwrap::split_env(toks);
            shellwrap::takes_its_script_from_stdin(&prog, &argv)
        });
    // A shell (or a program the shell builds, read as one) handed a process substitution reads
    // what its commands print as its script: `bash <(echo gh pr merge 12)`, `$SHELL < <(…)`.
    // Each body is read for what it could print, and only a merge among it is refused (round 7).
    if cmd.contains("<(") && segments.iter().any(|toks| hands_a_script_to_a_shell(toks)) {
        let (bodies, _) = substitution_bodies(cmd, true);
        if nested_too_deep() {
            return Some(too_nested(fix));
        }
        for body in bodies {
            for toks in commands_of(&body) {
                for printed in fed_lines(&toks) {
                    if let Some(said) = inner_floor(&printed) {
                        return Some(said);
                    }
                }
            }
        }
    }
    // A substitution the shell runs, wherever it stands, even inside double quotes, read from
    // the line as written: the words a substitution's own quotes split are not its body (#866).
    let mut unclosable = false;
    let live = livesub::may_substitute(cmd) && livesub::live_substitution(cmd).is_some();
    if live || contains_wide_funsub(cmd) {
        let (bodies, unclosed_funsub) = substitution_bodies(cmd, true);
        if nested_too_deep() {
            return Some(too_nested(fix));
        }
        // A `${ …;` that never closes (#7246): read to the line's end, and refused last like a
        // body the reader cannot close.
        unclosable |= unclosed_funsub;
        for body in bodies {
            // A `case` inside a substitution is held, parsed or not: its patterns' `)` are
            // what a substitution's own close is told from (#7131, round 6).
            if names_a_case(&body) {
                return Some(format!(
                    "{fix}{} A `case` inside a substitution can't be read reliably. {WRITE_IT_OUT}",
                    merge::UNREADABLE
                ));
            }
            if let Some(said) = inner_floor(&body) {
                return Some(said);
            }
            // A body with a comment, a heredoc or a `$'` quote is one the close of the
            // substitution is not reliably found in. It is refused last, so a refusal the line
            // already earns is the one said (round 7). Not in a hedge: that text is data in the
            // real reading, whose own bodies were already read above, and the hedge's re-spelling
            // of it (its quotes gone, so `<<'EOF'` is an unquoted `<<EOF` with no body) is no
            // text of the line that must close (#7245, round 5).
            unclosable |= !reading_a_hedge() && body_the_reader_cannot_close(&body);
        }
    }
    // A program the shell builds, from a substitution, a brace expansion or a glob, could be any
    // command with any words after it, so it is unreadable outright (#7131). Read only where a
    // shell runs the line as a command: a heredoc body's substitution is data in its place.
    // A glob or a brace counts only where a command starts: not in a `case` pattern, an
    // arithmetic expansion or an array's words, nor in a body an interpreter runs.
    // A `case` the reader does not fully parse could hide any command in what it took for a
    // pattern, so it is unreadable outright (#7131, round 6).
    if time_option {
        return Some(format!(
            "{fix}{} `time` has an option here that purlis can't read. {WRITE_IT_OUT}",
            merge::UNREADABLE
        ));
    }
    if unparsed_case || unparsed_hedge_case {
        return Some(format!(
            "{fix}{} A `case` here isn't one purlis can read to its end. {WRITE_IT_OUT}",
            merge::UNREADABLE
        ));
    }
    // The two speculative checks below read only lines a shell runs on every reading of the
    // text: a body line that is read as commands merely as bash 3.2's hedge (a `)` in a
    // quoted-substitution's heredoc body) is held for a literal merge by the segments above,
    // never for "could print any command" (#7131, the ruling on the large commit body).
    let mut filled = None;
    for reading in shellseg::segment_readings(&lines)
        .into_iter()
        .filter(|_| !reading_a_hedge())
    {
        // A definition's body is read as the commands it will run: its first program, if the
        // shell builds it, is as unreadable as one in the line itself (round 7).
        for toks in [&reading.outward, &reading.own] {
            if let Some(at) = shellwrap::past_function_headers(toks) {
                let (body, ..) = shellwrap::split_env(&toks[at..]);
                if program_built_by_the_shell(&body) {
                    return Some(format!(
                        "{fix}{} The program `{body}` of a function's body is built by the shell, \
                         so purlis can't read which command this is. {WRITE_IT_OUT}",
                        merge::UNREADABLE
                    ));
                }
            }
        }
        // A forge API call whose arguments the shell fills in, read or not (#7416). Said after
        // the segments' own reasons, which name more.
        if filled.is_none() {
            filled = merge::shell_filled_api_reason(&reading.own, &reading.own_source, cmd);
        }
        let (prog, ..) = shellwrap::split_env(&reading.own);
        if program_built_by_the_shell(&prog) {
            return Some(format!(
                "{fix}{} The program `{prog}` is built by the shell, so purlis can't read which \
                 command this is. {WRITE_IT_OUT}",
                merge::UNREADABLE
            ));
        }
        // A substitution anywhere in the program word is split into words, so a path around it
        // makes no path of it: what it prints could be the command (`$(echo gh pr merge 12)/y`,
        // `$X$(…)/y`). Only one that prints a path is read past (#7131).
        let printed = substitution_that_could_print_a_command(&reading, &prog);
        if nested_too_deep() {
            return Some(too_nested(fix));
        }
        if let Some(body) = printed {
            return Some(format!(
                "{fix}{} The program `{prog}` holds a substitution (`{body}`) that the shell \
                 splits into words, and it could print any command. {WRITE_IT_OUT}",
                merge::UNREADABLE
            ));
        }
    }
    for toks in &segments {
        let said = segment_reason(toks, cmd, unattended, depth, a_shell_reads_stdin);
        if said.is_some() {
            return said;
        }
    }
    if let Some(why) = filled {
        return Some(format!("{fix}{why}"));
    }
    // The hedged lines' own segments, and the substitutions on each line (the outer `$( … )`
    // steps over a quoted body, so `echo "$(gh pr merge 7)"` in one is found line by line).
    if !hedge_segments.is_empty() {
        let _hedge = ReadingAHedge::on();
        for toks in &hedge_segments {
            let said = segment_reason(toks, cmd, unattended, depth, the_hedge_reads_stdin);
            if said.is_some() {
                return said;
            }
        }
        // A shell on a hedged line, named or built by the shell, is fed by the other segments of
        // that line, and of no other line (so the cost stays off a long body).
        for own in &hedged_line_segments {
            if !own.iter().any(|toks| takes_its_script_from_stdin(toks)) {
                continue;
            }
            for toks in own.iter().filter(|toks| !takes_its_script_from_stdin(toks)) {
                for fed in fed_lines(toks) {
                    if let Some(said) = floor(&fed, unattended, depth + 1) {
                        return Some(said);
                    }
                }
            }
        }
        // A line of a body is a piece of text, so what a substitution on it holds is read as the
        // commands it holds (nested ones too, to the depth the floor reads), not as a text of
        // its own that must close: prose with a lone backtick or quote is not refused for it.
        let mut bodies: Vec<String> = hedged_lines
            .iter()
            .filter(|line| livesub::may_substitute(line))
            .flat_map(|line| substitution_bodies(line, true).0)
            .collect();
        let mut levels = 0;
        while !bodies.is_empty() && levels <= MAX_DEPTH {
            let mut deeper = Vec::new();
            for body in bodies {
                // A `case` in a substitution is held here as on any line.
                if names_a_case(&body) {
                    return Some(format!(
                        "{fix}{} A `case` inside a substitution can't be read reliably. {WRITE_IT_OUT}",
                        merge::UNREADABLE
                    ));
                }
                for toks in commands_of(&body) {
                    let said = segment_reason(&toks, cmd, unattended, depth, the_hedge_reads_stdin);
                    if said.is_some() {
                        return said;
                    }
                }
                if livesub::may_substitute(&body) {
                    deeper.extend(substitution_bodies(&body, true).0);
                }
            }
            if nested_too_deep() {
                return Some(too_nested(fix));
            }
            bodies = deeper;
            levels += 1;
        }
    }
    // Where a substitution ends is a reader limit that fails closed, not a speculative check.
    if unclosable {
        return Some(format!(
            "{fix}{} A substitution here holds a comment, a heredoc or a `$'` quote, so purlis \
             can't tell where it ends. {WRITE_IT_OUT}",
            merge::UNREADABLE
        ));
    }
    None
}

/// The floor's refusal of one segment of `cmd` at `depth`, read as its program's command.
fn segment_reason(
    toks: &[String],
    cmd: &str,
    unattended: bool,
    depth: usize,
    a_shell_reads_stdin: bool,
) -> Option<String> {
    let fix = RELEASE_FLOOR_FIX;
    let inner_floor = |inner: &str| floor(inner, unattended, depth + 1);
    // Each segment is split once, and every reading below shares it: at every level the
    // floor reads, a segment is as long as the line, so a second split is a second line
    // (#1355).
    // zsh's `-` precommand runs the command after it (#7423).
    if toks.first().is_some_and(|w| w == "-") && toks.len() > 1 {
        return segment_reason(&toks[1..], cmd, unattended, depth, a_shell_reads_stdin);
    }
    let (prog, env, argv) = shellwrap::split_env(toks);
    // A string a shell runs is read as the command it is (#866).
    let scripts = shellwrap::shell_scripts_of(&prog, &argv)
        .into_iter()
        .chain(shellwrap::here_string_script_of(&prog, &argv));
    let fed = if a_shell_reads_stdin && !takes_its_script_from_stdin(toks) {
        fed_lines(toks)
    } else {
        Vec::new()
    };
    for inner in scripts.chain(fed) {
        if let Some(said) = inner_floor(&inner) {
            return Some(said);
        }
    }
    // What a `find` runs for each file it finds is a command of its own (#7423).
    for run in shellwrap::commands_a_find_runs(&prog, &argv) {
        if let Some(said) = segment_reason(&run, cmd, unattended, depth, a_shell_reads_stdin) {
            return Some(said);
        }
    }
    if !only_merges()
        && let Some(said) = token::keychain_read_reason(&prog, &argv)
    {
        return Some(said);
    }
    // zsh reads `=gh` as the path of `gh`, so it is that program (#7131).
    if let Some(named) = prog.strip_prefix('=').filter(|named| !named.is_empty()) {
        let toks: Vec<String> = env
            .iter()
            .cloned()
            .chain(std::iter::once(named.to_owned()))
            .chain(argv.iter().skip(1).cloned())
            .collect();
        return segment_reason(&toks, cmd, unattended, depth, a_shell_reads_stdin);
    }
    // A program the shell fills in is read as each CLI it could be (#7124).
    // Only a word with `$` or a backtick: `[` is the test builtin, not a glob.
    if prog.contains(['$', '`']) {
        let as_cli = |cli: &str| {
            let toks: Vec<String> = env
                .iter()
                .cloned()
                .chain(std::iter::once(cli.to_owned()))
                .chain(argv.iter().skip(1).cloned())
                .collect();
            segment_reason(&toks, cmd, unattended, depth, a_shell_reads_stdin)
        };
        if let Some(cli) = FILLED_IN_PROGRAM_READ_AS
            .into_iter()
            .find(|cli| as_cli(cli).is_some())
        {
            return Some(format!(
                "{fix}{} The program `{prog}` is filled in by the shell, and as `{cli}` this \
                 command would be refused. {WRITE_IT_OUT}",
                merge::UNREADABLE
            ));
        }
    }
    // `GIT tag v1` is a tag — see A2's fold.
    let base = base_lower(&prog);
    let args: &[String] = argv.get(1..).unwrap_or(&[]);
    let words: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();
    if base == "git" || base.starts_with("git-credential-") {
        let sub = git_subcommand(args);
        // A subcommand the shell fills in could be a push of tags or of a merge option, as
        // one word or several (#7124).
        if let Some(sub) = sub.as_deref().filter(|sub| merge::opaque(sub)) {
            return Some(format!(
                "{fix}{} git's subcommand `{sub}` is filled in by the shell. {WRITE_IT_OUT}",
                merge::UNREADABLE
            ));
        }
        // A git alias defined on the line is read as what it stands for (#866).
        match merge::git_alias(args, &env, sub.as_deref()) {
            merge::GitAlias::Resolved(inner) => {
                if let Some(said) = inner_floor(&inner) {
                    return Some(said);
                }
            }
            merge::GitAlias::Unreadable => return Some(format!("{fix}{}", merge::UNREADABLE)),
            merge::GitAlias::None => {}
        }
        if sub.as_deref() == Some("tag") && !only_merges() {
            // `git tag` alone lists; a bare name is a CREATION — the choke point, since a tag
            // that does not exist locally cannot be pushed.
            //
            // A harmless flag clears THIS segment and the walk carries on (#348): a read
            // standing before a publish must not clear the floor for the publish.
            if args.iter().any(|a| TAG_HARMLESS.contains(&a.as_str())) {
                return None;
            }
            if words.len() > 1 {
                return Some(format!(
                    "{fix}Creating a tag is the first step of a publish."
                ));
            }
        }
        if sub.as_deref() == Some("push") && !only_merges() {
            // An option is read as git reads it, an abbreviation as the option it names (#7131).
            let options: Vec<&str> = args
                .iter()
                .skip_while(|a| *a != "push")
                .filter_map(|a| push_long_option(a))
                .collect();
            if options.iter().any(|o| matches!(*o, "tags" | "follow-tags")) {
                return Some(format!("{fix}`--tags`/`--follow-tags` pushes tags."));
            }
            // `--mirror` pushes every ref, its tags among them (#7131).
            if options.contains(&"mirror") {
                return Some(format!("{fix}`--mirror` pushes every ref, tags included."));
            }
            if let Some(why) = push_config_reason(args) {
                return Some(format!("{fix}{why}"));
            }
            // A flag whose name the shell fills in could be `--tags` (#7124), and one whose
            // value it fills in is no better read (#7131).
            if let Some(flag) = args.iter().find(|a| push_flag_filled_in(a)) {
                return Some(format!(
                    "{fix}The flag `{flag}` is filled in by the shell, and could push tags. \
                     {WRITE_IT_OUT}"
                ));
            }
            // Defence in depth: a tag that already existed locally, or a refspec whose
            // destination is a tag (`HEAD:refs/tags/v1`, #7131).
            let ends: Vec<&str> = words.iter().skip(1).map(|w| refspec_end(w)).collect();
            if ends.iter().any(|end| end.starts_with("refs/tags/")) {
                return Some(format!("{fix}This pushes a tag ref."));
            }
            let refspecs = push_refspecs(args);
            // `tag <name>` is `refs/tags/<name>` (#7131).
            if refspecs.iter().rev().skip(1).any(|w| *w == "tag") {
                return Some(format!("{fix}`tag <name>` pushes a tag."));
            }
            // A wildcard refspec over refs that are not branches (`refs/*:refs/*`) pushes the
            // tags with them (#7131).
            if refspecs.iter().any(|spec| {
                let end = refspec_end(spec);
                end.contains('*') && !end.starts_with("refs/heads/")
            }) {
                return Some(format!(
                    "{fix}A wildcard refspec over every ref pushes tags."
                ));
            }
            if ends.iter().any(|end| version_tag_re().is_match(end)) {
                return Some(format!("{fix}This pushes what looks like a version tag."));
            }
        }
        // A push option that sets auto-merge, an alias that would publish, or a command that
        // prints the forge token (#866).
        // A token refusal is whole: it opens with its own sentence (#6314).
        if !only_merges()
            && let Some(said) = token::git_reason(&base, args, sub.as_deref())
        {
            return Some(said);
        }
        let why = merge::git_reason(args, &env, sub.as_deref(), unattended, depth);
        if let Some(why) = why {
            return Some(format!("{fix}{why}"));
        }
    } else if base == "gh" || base == "glab" || leakguard::is_charter(&prog, args) {
        // **The reader had to widen with the set.** `PUBLISH_FORGE`'s charter row would be a
        // tuple nothing could reach if this branch were still `base in ("gh", "glab")`.
        let forge = base == "gh" || base == "glab";
        // Every name of charter's own is read as the one its table row is keyed on, so
        // `purlis change land` and `edm change land` are `charter change land` (RN-3).
        let name = if forge {
            base.clone()
        } else {
            crate::cliname::ALIAS.to_string()
        };
        // `edm change land` and `python3 -m charter change land` put charter's own NAME in
        // `words` instead of in `prog` — and `-m` drops out with the other flags — so the
        // pair sits one place further along and is put back on the same footing here. A
        // forge CLI's words are read past its repository flag (#866).
        let words: Vec<&str> = if forge {
            merge::forge_words(args).into_iter().map(|w| w.1).collect()
        } else {
            match words.first() {
                Some(w) if CHARTER_PROGS.contains(&w.to_lowercase().as_str()) => &words[1..],
                _ => &words[..],
            }
            .iter()
            .map(|w| w.as_str())
            .collect()
        };
        // A noun the shell fills in could be any command, and a verb after a noun that has a
        // held verb could be that verb (#7124).
        if let Some(word) = filled_in_command_word(&name, &words) {
            return Some(format!(
                "{fix}{} `{word}` is filled in by the shell, so purlis can't read which `{name}` \
                 command this is. {WRITE_IT_OUT}",
                merge::UNREADABLE
            ));
        }
        if let [noun, verb, ..] = words[..] {
            // A CLI's own alias of a verb is that verb (#866).
            let held = merge::canonical_verb(&name, noun, verb);
            let row = (name.as_str(), noun, held);
            let floored = if only_merges() {
                MERGE_ROWS.contains(&row)
            } else {
                PUBLISH_FORGE.contains(&row) || PUBLISH_SINCE.contains(&row)
            };
            if floored {
                return Some(format!(
                    "{fix}`{name} {noun} {verb}` publishes or lands code."
                ));
            }
        }
        if forge {
            if !only_merges()
                && let Some(said) = token::forge_reason(args)
            {
                return Some(said);
            }
            // A commit to the default branch through the API is a push, said whole (#6962).
            if !only_merges()
                && let Some(said) = merge::default_branch_push(args)
            {
                return Some(said);
            }
            let why = merge::forge_reason(&name, args, cmd, unattended, depth);
            if let Some(why) = why {
                return Some(format!("{fix}{why}"));
            }
        }
    }
    None
}

/// The CLIs a program the shell fills in is read as (#7124): each one the floor reads a
/// command of. `$(echo gh) pr merge 12` is refused because it would be as `gh`; `$EDITOR
/// notes.md` is read as each and refused as none. As `bash`, a shell filled in (`$SHELL -c
/// '…'`, `$(which sh) -c '…'`) has its script read (#7131).
const FILLED_IN_PROGRAM_READ_AS: [&str; 5] = ["gh", "glab", "git", crate::cliname::ALIAS, "bash"];

/// How a refusal of a command word the shell fills in ends (#7124).
const WRITE_IT_OUT: &str = "Write the command's program, noun and verb out literally.";

/// The nouns whose verb can merge a request: a verb the shell fills in after one of them is
/// held by the merge hold (#7124). After any other noun with a held verb, it is refused only
/// unattended.
const NOUNS_THAT_MERGE: [&str; 4] = ["pr", "mr", "alias", "change"];

/// The word of a forge CLI's or charter's command that the shell fills in where it could name
/// a held command (#7124): its noun, which could be any, or its verb after a noun that has a
/// held verb ([`merge::held_commands`]). `gh pr $(echo merge) 12` is one; `gh pr view $(cat
/// n)` and `gh issue $(echo view) 3` are not.
fn filled_in_command_word<'a>(name: &str, words: &[&'a str]) -> Option<&'a str> {
    match words {
        [noun, ..] if merge::opaque(noun) => Some(noun),
        [noun, verb, ..] if merge::opaque(verb) => {
            let held = merge::held_commands(name)
                .into_iter()
                .chain(
                    PUBLISH_SINCE
                        .iter()
                        .filter(|(cli, ..)| *cli == name)
                        .map(|(_, noun, verb)| vec![*noun, *verb]),
                )
                .any(|command| command.len() > 1 && command[0] == *noun);
            let matters = held && (!only_merges() || NOUNS_THAT_MERGE.contains(noun));
            matters.then_some(verb)
        }
        _ => None,
    }
}

/// Whether a `git push` flag is one the shell fills in, its name (`--$(echo tags)`, #7124) or
/// its value (`--repo=$T`, #7131). `--force-with-lease=main:$SHA` only says what the remote
/// must hold, so its value may be filled in.
fn push_flag_filled_in(word: &str) -> bool {
    word.starts_with('-') && !word.starts_with("--force-with-lease=") && merge::opaque(word)
}

/// The refspecs of a `git push`: its words past `push` and the remote, with the value of each
/// option that takes one apart (`-o <option>`, `--repo <remote>`) left out (#7131).
fn push_refspecs(args: &[String]) -> Vec<&str> {
    let Some(at) = args.iter().position(|a| a == "push") else {
        return Vec::new();
    };
    let mut words = Vec::new();
    let mut rest = args[at + 1..].iter();
    while let Some(a) = rest.next() {
        let takes_the_next_word = a == "-o"
            || (!a.contains('=')
                && push_long_option(a).is_some_and(|o| {
                    matches!(o, "push-option" | "repo" | "receive-pack" | "exec")
                }));
        if takes_the_next_word {
            rest.next();
        } else if !a.starts_with('-') {
            words.push(a.as_str());
        }
    }
    words.into_iter().skip(1).collect()
}

/// `git push`'s long options, as its own option table names them (git's `builtin/push.c`).
const PUSH_LONG_OPTIONS: [&str; 28] = [
    "verbose",
    "quiet",
    "repo",
    "all",
    "branches",
    "mirror",
    "delete",
    "tags",
    "dry-run",
    "porcelain",
    "force",
    "force-with-lease",
    "force-if-includes",
    "recurse-submodules",
    "thin",
    "receive-pack",
    "exec",
    "set-upstream",
    "progress",
    "prune",
    "no-verify",
    "verify",
    "follow-tags",
    "signed",
    "atomic",
    "push-option",
    "ipv4",
    "ipv6",
];

/// **The `git push` long option a word names, as git reads it** (#7131): the option it spells
/// whole, or the one option an abbreviation begins (`--mirr` is `--mirror`, `--tag` is
/// `--tags`), its value apart (`--repo=o`). `None` for a word that names none, or an
/// abbreviation of more than one, which git refuses.
fn push_long_option(word: &str) -> Option<&'static str> {
    let name = word.strip_prefix("--")?;
    let name = name.split_once('=').map_or(name, |(name, _)| name);
    if name.is_empty() {
        return None;
    }
    if let Some(whole) = PUSH_LONG_OPTIONS.iter().find(|o| **o == name) {
        return Some(whole);
    }
    let mut begun = PUSH_LONG_OPTIONS.iter().filter(|o| o.starts_with(name));
    match (begun.next(), begun.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}

/// **What git's own options before `push` set that pushes tags** (#7131): `push.followTags` on
/// (`-c push.followTags=true`, `-c push.followtags`), which pushes the annotated tags the pushed
/// commits reach; a remote's push refspec (`-c remote.origin.push=refs/tags/*`), which could
/// name any ref; or a remote's mirror flag on, which pushes every ref. A setting `--config-env`
/// reads from the environment is read as on.
fn push_config_reason(args: &[String]) -> Option<&'static str> {
    let mut i = 0;
    while i < args.len() && args[i] != "push" {
        let (setting, from_env) = match args[i].as_str() {
            "-c" => {
                i += 1;
                (args.get(i).map(String::as_str), false)
            }
            "--config-env" => {
                i += 1;
                (args.get(i).map(String::as_str), true)
            }
            a => match a.strip_prefix("--config-env=") {
                Some(setting) => (Some(setting), true),
                None => (a.strip_prefix("-c").filter(|s| !s.is_empty()), false),
            },
        };
        if let Some(setting) = setting {
            let (key, value) = setting.split_once('=').unwrap_or((setting, "true"));
            let on = from_env
                || !matches!(
                    value.to_ascii_lowercase().as_str(),
                    "false" | "no" | "off" | "0"
                );
            let key = key.to_ascii_lowercase();
            let remote = key.strip_prefix("remote.");
            if key == "push.followtags" && on {
                return Some("`push.followTags` set on the command line pushes tags.");
            }
            if remote.is_some_and(|r| r.ends_with(".push")) {
                return Some("A remote's push refspec set on the command line can push tags.");
            }
            if remote.is_some_and(|r| r.ends_with(".mirror")) && on {
                return Some(
                    "A remote's mirror flag set on the command line pushes every ref, tags \
                     included.",
                );
            }
        }
        i += 1;
    }
    None
}

/// The refs a refspec pushes to, as the remote reads it: `HEAD:refs/tags/v1` is `refs/tags/v1`,
/// `+main:main` is `main`, and a ref with no `:` is its own destination (#7131).
fn refspec_end(spec: &str) -> &str {
    spec.rsplit(':')
        .next()
        .unwrap_or(spec)
        .trim_start_matches('+')
}

/// Whether a program is one a glob or a brace builds, so its words cannot be read (#7131): its
/// last path component holds a glob (`g[h]`), or the word holds a brace expansion anywhere
/// (`{gh,pr,merge,12}`, `{/opt/homebrew/bin/gh,pr,merge,12}`), which splits it into words. A
/// command substitution is judged by what it prints
/// ([`substitution_that_could_print_a_command`]), a parameter expansion (`${cmd[@]}`) is a
/// program the shell fills in ([`FILLED_IN_PROGRAM_READ_AS`]), and `[` and `[[` are the test
/// builtins.
fn program_built_by_the_shell(prog: &str) -> bool {
    if prog == "[" || prog == "[[" {
        return false;
    }
    // The word with each substitution as `\0` and each parameter expansion as `$`, and its last
    // path component the same way.
    let mut whole = String::new();
    let mut last = String::new();
    let mut chars = prog.chars().peekable();
    while let Some(c) = chars.next() {
        let stands_for = match c {
            '/' => {
                last.clear();
                whole.push('/');
                continue;
            }
            '$' if chars.peek() == Some(&'(') => {
                skip_to_close(&mut chars, '(', ')');
                '\u{0}'
            }
            '$' if chars.peek() == Some(&'{') => {
                skip_to_close(&mut chars, '{', '}');
                '$'
            }
            '`' => {
                chars.by_ref().find(|c| *c == '`');
                '\u{0}'
            }
            c => c,
        };
        last.push(stands_for);
        whole.push(stands_for);
    }
    // `[` opens a glob only where a `]` closes it.
    let glob_class = last
        .find('[')
        .is_some_and(|open| last[open + 1..].contains(']'));
    // `?` alone matches only a name one character long, which no CLI the floor reads has.
    let lone_one_character_glob = last == "?";
    (last.contains(['*', '?']) && !lone_one_character_glob)
        || glob_class
        || merge::brace_expansion(&whole)
}

/// Whether a segment's words name a program that takes its script from stdin: a shell with
/// no `-c` and no script, or `source` and `.` of `/dev/stdin` (#7131), as every guard reads it
/// ([`shellwrap::takes_its_script_from_stdin`]).
fn takes_its_script_from_stdin(toks: &[String]) -> bool {
    let (prog, _env, argv) = shellwrap::split_env(toks);
    shellwrap::takes_its_script_from_stdin(&prog, &argv)
        || shellwrap::built_program_takes_its_script_from_stdin(&prog, &argv)
}

/// **A command substitution in a program word that could print a command** (#7131): the body of
/// the first one that does more than print a path ([`prints_only_a_path`]), read from the word
/// as the source spells it ([`shellseg::Reading::own_source`]), or the word itself where its
/// substitutions cannot be read. `None` for a program word with no substitution, or whose every
/// substitution prints a path (`$(git rev-parse --show-toplevel)/x.sh`).
fn substitution_that_could_print_a_command(
    reading: &shellseg::Reading,
    prog: &str,
) -> Option<String> {
    // A word that ends in `$` with `(` the next word is a substitution the segment walk left
    // unopened, as after a quoted part (`""$(…)`): the shell opens one there, or refuses the line.
    // It is read up to the program, or to the first word that is no assignment where the walk
    // reads the program from inside it.
    let own = &reading.own;
    let program_at = own.iter().position(|w| w == prog);
    let first_word = own.iter().position(|w| !is_assignment(w));
    let unopened = own.windows(2).enumerate().find(|(at, pair)| {
        pair[0].ends_with('$')
            && pair[1] == "("
            && (Some(*at) == first_word || program_at.is_some_and(|p| *at <= p))
    });
    if let Some((_, pair)) = unopened {
        return Some(format!("{}(", pair[0]));
    }
    // A process substitution (`<(…)`) is a path the shell hands the program, `/dev/fd/63`, and
    // never a command; its body is read as one by its own text.
    let command_substitution = format!("${}", shellseg::SUBSTITUTED);
    let funsub = contains_funsub(prog);
    if !(prog.contains(&command_substitution)
        || prog.contains("$(")
        || prog.contains('`')
        || funsub)
    {
        return None;
    }
    let source = reading
        .own
        .iter()
        .position(|w| w == prog)
        .and_then(|at| reading.own_source.get(at))
        .filter(|source| !source.is_empty())
        .map_or(prog, String::as_str);
    let (bodies, unclosed_funsub) = substitution_bodies(&shellseg::unbacktick(source), false);
    if bodies.is_empty() || unclosed_funsub {
        return Some(prog.to_owned());
    }
    bodies.into_iter().find(|body| !prints_only_a_path(body))
}

/// The variables a substitution that prints a path may read: the script's own path, which no
/// line sets.
const SCRIPT_PATHS: [&str; 5] = [
    "$0",
    "${0}",
    "$BASH_SOURCE",
    "${BASH_SOURCE}",
    "${BASH_SOURCE[0]}",
];

/// The `git rev-parse` options that print only a path of the checkout.
const GIT_PATH_OPTIONS: [&str; 9] = [
    "--show-toplevel",
    "--git-dir",
    "--git-common-dir",
    "--absolute-git-dir",
    "--show-prefix",
    "--show-cdup",
    "--show-superproject-working-tree",
    "--path-format=absolute",
    "--path-format=relative",
];

/// **Whether a substitution's body only prints a path** (#7131), so a program word built from it
/// is a path and not a command: one simple command, with no operator, redirection or
/// substitution in it, that is `git rev-parse` with options that print a checkout's path,
/// `dirname`, `realpath` or `readlink -f` of one path, `pwd`, `brew --prefix`, or `which` or
/// `command -v` of one name. Each operand is one literal word with no blank, glob or brace in
/// it, or the script's own path ([`SCRIPT_PATHS`]): `dirname 'gh pr merge 12/x'` prints a
/// command. Anything else could print any command, and is held.
fn prints_only_a_path(body: &str) -> bool {
    let Ok(toks) = shellseg::lex(body) else {
        return false;
    };
    if toks
        .iter()
        .any(|t| t.bare && t.text.chars().any(|c| "();<>|&\n".contains(c)))
    {
        return false;
    }
    let words: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
    let literal =
        |w: &str| !w.is_empty() && !w.contains(|c: char| c.is_whitespace() || "$`*?[{".contains(c));
    let path = |w: &str| SCRIPT_PATHS.contains(&w) || literal(w);
    match words[..] {
        ["git", "rev-parse", ref options @ ..] => {
            !options.is_empty() && options.iter().all(|o| GIT_PATH_OPTIONS.contains(o))
        }
        // One operand: `dirname` prints a line per operand, and the shell splits them into
        // words, so `dirname a/gh b/pr c/merge` prints `a b c`.
        ["dirname", p] => path(p) && !p.starts_with('-'),
        ["dirname", "--", p] => path(p),
        ["pwd"] | ["pwd", "-L" | "-P"] | ["brew", "--prefix"] => true,
        ["brew", "--prefix", formula] => literal(formula) && !formula.starts_with('-'),
        ["which", name] | ["command", "-v", name] => literal(name),
        ["realpath", p] | ["readlink", "-f", p] => path(p),
        _ => false,
    }
}

/// `line` with what stands in no command position blanked out, for the reading of a program
/// the shell builds (#7131): an arithmetic expansion or command (`$((2*3))`, `((i*2))`), a
/// `case` pattern through its `)` (`*)`, `y*|n*)`), and an array's words (`files=(src/*.rs)`).
/// A glob or a brace there is no program. A substitution inside an arithmetic expansion is
/// still read, by its own body. A line that will not lex keeps its patterns and its arrays.
///
/// And whether a `case` was met that the reading did not parse whole: a header that is not
/// `case WORD in` with the WORD read as the shell reads it, or a pattern that no `)` closes.
/// Such a line is held, never read on (round 6). The last is whether a `time` had an option
/// other than `-p`, which is held too and says so.
fn in_command_position(line: &str) -> (String, bool, bool) {
    let (line, unsure_arithmetic) = without_arithmetic(line);
    let Ok(toks) = shellseg::lex(&line) else {
        return (line, unsure_arithmetic, false);
    };
    let toks = shellseg::split_punctuation(toks);
    let source: Vec<char> = line.chars().collect();
    let mut chars = source.clone();
    // An arithmetic body with a backslash or a quote is one the reader cannot place the
    // parentheses of: held, never read on (round 7).
    let mut unparsed = unsure_arithmetic;
    let mut time_option = false;
    // Where a `;` is put in the blanks before a `do` or `{` that follows its header with none.
    let mut breaks: Vec<usize> = Vec::new();
    let mut blank = |from: isize, to: isize| {
        let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
            return;
        };
        for c in chars.iter_mut().take(to).skip(from) {
            *c = ' ';
        }
    };
    /// Where a `case` is: its patterns, or an arm's commands.
    enum Case {
        Patterns,
        Arm,
    }
    let mut cases: Vec<Case> = Vec::new();
    // Whether the next word stands where a command starts, as the shell's grammar reads it: the
    // line's first, after a control operator, or after a reserved word a command follows. A
    // word anywhere else is an argument, `case` and `do` included (#7131).
    let mut at_command = true;
    // Whether a reserved word would be one there: not after an assignment, a redirection or a
    // substitution, where `case` is a program's name and what follows it a subshell's (bash).
    let mut reserved = true;
    // The `for` and `select` still waiting for their `do` or `{`, which follow an arithmetic
    // header with no `;` between.
    let mut loops = 0usize;
    // Each `(` still open: where the line stands after the word a substitution is in, or `None`
    // for a subshell.
    let mut opened: Vec<Option<bool>> = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        // Once a case is held the rest of the line is not read: the line is refused (round 7).
        if unparsed {
            break;
        }
        let tok = &toks[i];
        let bare = |text: &str| tok.bare && tok.text == text;
        // A word of a case carrying a `${` that does not close in it: the lexer split the
        // expansion at an operator inside it, so what follows is not read as the shell reads it.
        if !cases.is_empty() && tok.bare && has_unclosed_expansion(&tok.text) {
            unparsed = true;
            continue;
        }
        if matches!(cases.last(), Some(Case::Patterns)) && !bare("\n") {
            if bare("esac") {
                cases.pop();
                at_command = false;
                i += 1;
                continue;
            }
            // A pattern runs to the `)` that closes it; one that does not close is no pattern,
            // and the line is held.
            if let Some(close) = pattern_end(&toks, i) {
                blank(tok.start, toks[close].end);
                cases.pop();
                cases.push(Case::Arm);
                at_command = true;
                reserved = true;
                i = close + 1;
                continue;
            }
            unparsed = true;
            cases.pop();
        }
        if matches!(cases.last(), Some(Case::Arm))
            && tok.bare
            && (tok.text == ";;" || is_fallthrough(&toks, i))
        {
            cases.pop();
            cases.push(Case::Patterns);
            i += if is_fallthrough(&toks, i) { 2 } else { 1 };
            continue;
        }
        // `NAME=(…)`: an array's words, to the `)` that closes them.
        let opens_array = tok.text.ends_with('=')
            && toks
                .get(i + 1)
                .is_some_and(|t| t.bare && t.text == "(" && t.start == tok.end);
        if opens_array {
            let mut depth = 0usize;
            let mut j = i + 1;
            while let Some(t) = toks.get(j) {
                if t.bare && t.text == "(" {
                    depth += 1;
                } else if t.bare && t.text == ")" {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                j += 1;
            }
            let to = toks.get(j).map_or(isize::MAX, |t| t.end);
            blank(toks[i + 1].start, to);
            // An assignment before a command leaves the command still to come, and no reserved
            // word.
            reserved = false;
            i = j + 1;
            continue;
        }
        if bare("(") {
            // A substitution's `(` (`$(`, `<(`, `>(`) opens commands inside a word, and its `)`
            // goes back to where that word left the line; a subshell's `)` is followed by an
            // operator.
            let substitution = i
                .checked_sub(1)
                .and_then(|k| toks.get(k))
                .is_some_and(|prev| {
                    prev.end == tok.start
                        && (prev.text.ends_with('$')
                            || (prev.bare && matches!(prev.text.as_str(), "<" | ">")))
                });
            opened.push(substitution.then_some(at_command));
            at_command = true;
            reserved = true;
        } else if bare(")") {
            let was_substitution = opened.last().is_some_and(Option::is_some);
            at_command = opened.pop().flatten().unwrap_or(true);
            reserved = !was_substitution;
        } else if tok.bare && CONTROL_OPERATORS.contains(&tok.text.as_str()) {
            at_command = true;
            reserved = true;
        } else if tok.bare && REDIRECTIONS.contains(&tok.text.as_str()) {
            // The word after a redirection is its target, and where a command starts stays put.
            reserved = false;
            // A process substitution's `(` is no target: the `(` branch reads it.
            let process = toks
                .get(i + 1)
                .is_some_and(|n| n.bare && n.text == "(" && n.start == tok.end);
            i += if process { 1 } else { 2 };
            continue;
        } else if at_command && reserved && bare("esac") && matches!(cases.last(), Some(Case::Arm))
        {
            cases.pop();
            at_command = false;
        } else if bare("do") && loops > 0 {
            // `for ((…)) do`: the `do` follows the header with no `;`.
            loops -= 1;
            breaks.extend(
                usize::try_from(tok.start)
                    .ok()
                    .and_then(|at| at.checked_sub(1)),
            );
            at_command = true;
            reserved = true;
        } else if bare("{") && loops > 0 && !at_command {
            loops -= 1;
            breaks.extend(
                usize::try_from(tok.start)
                    .ok()
                    .and_then(|at| at.checked_sub(1)),
            );
            at_command = true;
            reserved = true;
        } else if at_command && reserved && bare("case") {
            // `case WORD in`, and only that, opens the patterns. The WORD is read as the shell
            // reads a word (`case_header_end`); a header that does not read that way is held.
            let header = usize::try_from(tok.end)
                .ok()
                .and_then(|end| case_header_end(&source, end));
            if let Some(end) = header {
                blank(tok.start, isize::try_from(end).unwrap_or(isize::MAX));
                cases.push(Case::Patterns);
                // From here on: a rescan from the first token after every header is quadratic.
                i = toks[i..]
                    .iter()
                    .position(|t| usize::try_from(t.start).is_ok_and(|s| s >= end))
                    .map_or(toks.len(), |at| i + at);
                continue;
            }
            unparsed = true;
            at_command = false;
        } else if at_command && reserved && bare("function") {
            // `function NAME`: the name is no command, and the body's `{` or `(` comes next.
            if let Some(name) = toks.get(i + 1) {
                blank(tok.start, name.end);
            }
            i += 2;
            continue;
        } else if at_command && reserved && bare("coproc") {
            // `coproc NAME compound`: the NAME is no command.
            let named = toks
                .get(i + 1)
                .is_some_and(|n| n.bare && !CONTROL_OPERATORS.contains(&n.text.as_str()))
                && toks.get(i + 2).is_some_and(|c| {
                    c.bare
                        && ["case", "{", "(", "while", "until", "if", "for", "select"]
                            .contains(&c.text.as_str())
                });
            if named {
                // A `case` after a NAME is not read as one: bash's reading of it is not
                // certain, so the line is held (round 7).
                if toks.get(i + 2).is_some_and(|c| c.bare && c.text == "case") {
                    unparsed = true;
                }
                // The NAME is blanked, so what the compound runs reads as the line's own.
                blank(tok.start, toks[i + 1].end);
            }
            i += if named { 2 } else { 1 };
            continue;
        } else if at_command && reserved && bare("time") {
            // `time -p`: its option is no command either. Any other word that starts with `-`
            // (`--` included: in bash 5, `time --` is followed by a real `case` and the other
            // compounds, and `time -p --` as well) is not read, so the line is held (round 7b).
            i += 1;
            if toks.get(i).is_some_and(|t| t.bare && t.text == "-p") {
                i += 1;
            }
            if toks.get(i).is_some_and(|t| t.text.starts_with('-')) {
                unparsed = true;
                time_option = true;
            }
            continue;
        } else if at_command && reserved && (bare("for") || bare("select")) {
            loops += 1;
            at_command = false;
        } else if at_command && reserved && tok.bare && COMMAND_FOLLOWS.contains(&tok.text.as_str())
        {
            // A reserved word a command follows leaves the next word where a command starts.
        } else if at_command && is_assignment(&tok.text) {
            // An assignment before a command leaves the command still to come.
            reserved = false;
        } else {
            at_command = false;
        }
        i += 1;
    }
    for at in breaks {
        if let Some(slot) = chars.get_mut(at).filter(|c| **c == ' ') {
            *slot = ';';
        }
    }
    (chars.into_iter().collect(), unparsed, time_option)
}

/// One past the `in` of the `case WORD in` whose `case` ends at `from`, with the WORD read as
/// the shell reads a word, or `None` where it is not one (an operator where the WORD stands, a
/// substitution, expansion or quote that does not close, anything but `in` after it).
fn case_header_end(source: &[char], from: usize) -> Option<usize> {
    let mut p = from;
    while source.get(p).is_some_and(|c| *c == ' ' || *c == '\t') {
        p += 1;
    }
    let end = word_end(source, p, 0)?;
    if end == p {
        return None;
    }
    p = end;
    while source.get(p).is_some_and(|c| c.is_whitespace()) {
        p += 1;
    }
    let word_in = source.get(p..p + 2)? == ['i', 'n'];
    let stops = source
        .get(p + 2)
        .is_none_or(|c| c.is_whitespace() || ";|&()<>".contains(*c));
    (word_in && stops).then_some(p + 2)
}

/// One past the shell word that starts at `p`: quotes, escapes and the expansions (`$(…)`,
/// `${…}`, `$[…]`, backticks) read whole, a blank or an operator ending it. `None` where one
/// does not close.
fn word_end(s: &[char], mut p: usize, nest: usize) -> Option<usize> {
    while let Some(&c) = s.get(p) {
        match c {
            '\\' => p += 2,
            '\'' => p = s.get(p + 1..)?.iter().position(|q| *q == '\'')? + p + 2,
            '"' => p = skip_double_quoted(s, p + 1, deeper(nest)?)?,
            '`' => p = skip_backtick(s, p + 1)?,
            '$' => match s.get(p + 1) {
                Some('(') => p = skip_paren(s, p + 2, deeper(nest)?)?,
                Some('{') => p = skip_brace(s, p + 2, deeper(nest)?)?,
                Some('[') => p = s.get(p + 2..)?.iter().position(|q| *q == ']')? + p + 3,
                _ => p += 1,
            },
            c if c.is_whitespace() || ";|&()<>".contains(c) => break,
            _ => p += 1,
        }
    }
    Some(p.min(s.len()))
}

std::thread_local! {
    /// Whether a reading of this thread's line went past [`MAX_NEST`] (round 7).
    static NESTED_TOO_DEEP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// How many expansions deep the reader follows one inside another to find where a word or a
/// substitution ends. No real command nests this deep, and without a bound a line of thousands
/// of `$(` would exhaust the stack: past it the line is held.
const MAX_NEST: usize = 128;

/// Whether a reading went past [`MAX_NEST`] since this was last asked, and forgets it.
fn nested_too_deep() -> bool {
    NESTED_TOO_DEEP.with(|flag| flag.replace(false))
}

/// The refusal of a line nested past [`MAX_NEST`].
fn too_nested(fix: &str) -> String {
    format!(
        "{fix}{} This line nests expansions too deep to read. {WRITE_IT_OUT}",
        merge::UNREADABLE
    )
}

/// Counts one more level of nesting, or notes that the reader went too deep and says so.
fn deeper(nest: usize) -> Option<usize> {
    if nest >= MAX_NEST {
        NESTED_TOO_DEEP.with(|flag| flag.set(true));
        CUT_SHORT.set(true);
        return None;
    }
    Some(nest + 1)
}

/// Whether a substitution's body holds what the close of the substitution is not reliably found
/// past: a comment (a `#` where a word starts), a heredoc whose delimiter line is not in the body
/// (`<<`, not a here-string's `<<<`), or a `$'` quote, none inside quotes of its own (round 7).
/// A heredoc that is closed inside the body is read past: its text is data.
fn body_the_reader_cannot_close(body: &str) -> bool {
    let chars: Vec<char> = body.chars().collect();
    let (mut single, mut double) = (false, false);
    // The heredocs opened on this line, whose bodies start at the next one: delimiter and `<<-`.
    let mut pending: Vec<(String, bool, bool)> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let word_start = i == 0 || chars[i - 1].is_whitespace() || ";&|()<>".contains(chars[i - 1]);
        match c {
            '\n' if !single && !double && !pending.is_empty() => {
                // Each pending heredoc runs to a line that is its delimiter.
                let mut at = i + 1;
                for (delimiter, dash, quoted) in std::mem::take(&mut pending) {
                    loop {
                        if at > chars.len() {
                            return true;
                        }
                        let end = chars[at.min(chars.len())..]
                            .iter()
                            .position(|c| *c == '\n')
                            .map_or(chars.len(), |n| at + n);
                        let line: String = chars[at.min(chars.len())..end].iter().collect();
                        let line = if dash {
                            line.trim_start_matches('\t')
                        } else {
                            &line
                        };
                        at = end + 1;
                        if line == delimiter {
                            break;
                        }
                        // An unquoted body joins a line that ends in `\` to the next one, which
                        // may then be, or hide, the delimiter: held (round 7b).
                        if !quoted && line.ends_with('\\') {
                            return true;
                        }
                    }
                }
                i = at.saturating_sub(1);
            }
            '\\' if !single => i += 1,
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            _ if single => {}
            '$' if !double && chars.get(i + 1) == Some(&'\'') => return true,
            '#' if !double && word_start => return true,
            '<' if !double
                && chars.get(i + 1) == Some(&'<')
                && chars.get(i + 2) != Some(&'<')
                && (i == 0 || chars[i - 1] != '<') =>
            {
                let mut j = i + 2;
                let dash = chars.get(j) == Some(&'-');
                if dash {
                    j += 1;
                }
                while chars.get(j).is_some_and(|c| *c == ' ' || *c == '\t') {
                    j += 1;
                }
                let mut raw = String::new();
                while let Some(&d) = chars.get(j) {
                    // Bash ends the word only at a blank, a newline or a metacharacter: a CR, FF,
                    // VT or NBSP stays in it, so the delimiter is not the plain word it looks.
                    if matches!(d, ' ' | '\t' | '\n') || ";&|()<>".contains(d) {
                        break;
                    }
                    raw.push(d);
                    j += 1;
                }
                // Read past only a plain word, bare or wholly quoted (round 7b): any other
                // spelling (a backslash, a `$'`, a part quote) is one the reader does not know.
                let plain = |w: &str| {
                    !w.is_empty() && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                };
                let (delimiter, quoted) = match (raw.chars().next(), raw.chars().last()) {
                    (Some(q @ ('\'' | '"')), Some(e)) if raw.len() >= 3 && q == e => {
                        (raw[1..raw.len() - 1].to_owned(), true)
                    }
                    _ => (raw.clone(), false),
                };
                if !plain(&delimiter) {
                    return true;
                }
                pending.push((delimiter, dash, quoted));
                i = j - 1;
            }
            _ => {}
        }
        i += 1;
    }
    !pending.is_empty()
}

/// One past the `"` that closes the double quotes opened before `p`.
fn skip_double_quoted(s: &[char], mut p: usize, nest: usize) -> Option<usize> {
    while let Some(&c) = s.get(p) {
        match c {
            '\\' => p += 2,
            '"' => return Some(p + 1),
            '`' => p = skip_backtick(s, p + 1)?,
            '$' if s.get(p + 1) == Some(&'(') => p = skip_paren(s, p + 2, deeper(nest)?)?,
            '$' if funsub_opens(s, p) => {
                let open = shellseg::spliced_end(s, p, "${").unwrap_or(p + 2);
                p = skip_funsub(s, open, deeper(nest)?)?;
            }
            '$' if s.get(p + 1) == Some(&'{') => p = skip_brace(s, p + 2, deeper(nest)?)?,
            _ => p += 1,
        }
    }
    None
}

/// One past the backtick that closes the pair opened before `p`.
fn skip_backtick(s: &[char], mut p: usize) -> Option<usize> {
    while let Some(&c) = s.get(p) {
        match c {
            '\\' => p += 2,
            '`' => return Some(p + 1),
            _ => p += 1,
        }
    }
    None
}

/// One past the `)` that closes the substitution opened before `p`, quotes and nested
/// substitutions read whole, or `None` where nothing closes it.
fn skip_paren(s: &[char], mut p: usize, nest: usize) -> Option<usize> {
    let mut depth = 0usize;
    // The quoted heredocs opened on this line, whose bodies are data: a `)` in one closes nothing.
    let mut pending: Vec<(String, bool)> = Vec::new();
    while let Some(&c) = s.get(p) {
        match c {
            '\n' if !pending.is_empty() => {
                p += 1;
                for (delimiter, dash) in std::mem::take(&mut pending) {
                    p = after_heredoc_body(s, p, &delimiter, dash)?;
                }
            }
            // A here-string (`<<<'x'`) opens no heredoc: its three characters are read as one.
            '<' if s.get(p + 1) == Some(&'<') && s.get(p + 2) == Some(&'<') => p += 3,
            '<' if s.get(p + 1) == Some(&'<') => match quoted_heredoc_at(s, p) {
                Some((delimiter, dash, after)) => {
                    pending.push((delimiter, dash));
                    p = after;
                }
                None => p += 2,
            },
            '\\' => p += 2,
            '\'' => p = s.get(p + 1..)?.iter().position(|q| *q == '\'')? + p + 2,
            '"' => p = skip_double_quoted(s, p + 1, deeper(nest)?)?,
            '`' => p = skip_backtick(s, p + 1)?,
            '$' if s.get(p + 1) == Some(&'(') => p = skip_paren(s, p + 2, deeper(nest)?)?,
            '$' if funsub_opens(s, p) => {
                let open = shellseg::spliced_end(s, p, "${").unwrap_or(p + 2);
                p = skip_funsub(s, open, deeper(nest)?)?;
            }
            '$' if s.get(p + 1) == Some(&'{') => p = skip_brace(s, p + 2, deeper(nest)?)?,
            '(' => {
                depth += 1;
                p += 1;
            }
            ')' if depth == 0 => return Some(p + 1),
            ')' => {
                depth -= 1;
                p += 1;
            }
            _ => p += 1,
        }
    }
    None
}

/// A heredoc `<<[-]'WORD'` or `<<[-]"WORD"` (a plain word, wholly quoted, so its body is data)
/// opened at `p`: its delimiter, whether it is `<<-`, and where the operator and word end.
/// `None` for any other spelling, which the caller reads as it always did.
fn quoted_heredoc_at(s: &[char], p: usize) -> Option<(String, bool, usize)> {
    let mut j = p + 2;
    let dash = s.get(j) == Some(&'-');
    if dash {
        j += 1;
    }
    while s.get(j).is_some_and(|c| *c == ' ' || *c == '\t') {
        j += 1;
    }
    let quote = *s.get(j).filter(|c| matches!(c, '\'' | '"'))?;
    let word_end = j + 1 + s.get(j + 1..)?.iter().position(|c| *c == quote)?;
    let word: String = s[j + 1..word_end].iter().collect();
    let plain = !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    plain.then_some((word, dash, word_end + 1))
}

/// Where the body of a heredoc ends, starting at the line `from`: one past its delimiter's line,
/// or `None` where no line is the delimiter.
fn after_heredoc_body(s: &[char], mut from: usize, delimiter: &str, dash: bool) -> Option<usize> {
    while from <= s.len() {
        let end = s[from.min(s.len())..]
            .iter()
            .position(|c| *c == '\n')
            .map_or(s.len(), |n| from + n);
        let line: String = s[from.min(s.len())..end].iter().collect();
        let line = if dash {
            line.trim_start_matches('\t')
        } else {
            &line
        };
        if line == delimiter {
            return Some(end);
        }
        from = end + 1;
    }
    None
}

/// One past the `}` that closes the bash 5.3 `${ …;` opened before `p` (#7246), or `None`.
///
/// Its body is commands, so the `}` that closes it is a word in command position: after `;`, `&`,
/// `|` or a newline, or at the start. A `{` in command position opens a group of its own that a
/// `}` of its own closes. Quotes and nested substitutions are read whole.
///
/// A command starts nowhere else, so these start none (#7250): the `&` of `>&`, `<&` and `&>` and
/// the `|` of `>|`, which are redirections; a `(`, which is read whole to its `)` (a subshell, an
/// array, a function's `()`, a `<(…)`), so a `}` inside it closes nothing; and a `[[ … ]]`, in
/// whose `&&` and `||` a word is an operand. Read strictly, so a close the reader is unsure of is
/// no close and the line is refused, never half read: an unclosed `(` or `[[` ends in `None`.
fn skip_funsub(s: &[char], mut p: usize, nest: usize) -> Option<usize> {
    let mut groups = 0usize;
    let mut at_command = true;
    let mut in_test = false;
    let blank = |c: Option<&char>| c.is_none_or(|c| c.is_whitespace());
    while let Some(&c) = s.get(p) {
        let command = std::mem::replace(&mut at_command, false);
        let before = p.checked_sub(1).and_then(|at| s.get(at));
        match c {
            '\\' => p += 2,
            '\'' => p = s.get(p + 1..)?.iter().position(|q| *q == '\'')? + p + 2,
            '"' => p = skip_double_quoted(s, p + 1, deeper(nest)?)?,
            '`' => p = skip_backtick(s, p + 1)?,
            '$' if s.get(p + 1) == Some(&'(') => p = skip_paren(s, p + 2, deeper(nest)?)?,
            '$' if funsub_opens(s, p) => {
                let open = shellseg::spliced_end(s, p, "${").unwrap_or(p + 2);
                p = skip_funsub(s, open, deeper(nest)?)?;
            }
            '$' if s.get(p + 1) == Some(&'{') => p = skip_brace(s, p + 2, deeper(nest)?)?,
            '[' if command && !in_test && s.get(p + 1) == Some(&'[') && blank(s.get(p + 2)) => {
                in_test = true;
                p += 2;
            }
            ']' if in_test
                && s.get(p + 1) == Some(&']')
                && blank(before)
                && (blank(s.get(p + 2)) || matches!(s.get(p + 2), Some(';' | '&' | '|' | ')'))) =>
            {
                in_test = false;
                p += 2;
            }
            '(' => {
                p = skip_paren(s, p + 1, deeper(nest)?)?;
                // A function's `()` is followed by its body.
                at_command = true;
            }
            _ if in_test => p += 1,
            '&' if matches!(before, Some('>' | '<')) || s.get(p + 1) == Some(&'>') => p += 1,
            '|' if before == Some(&'>') => p += 1,
            ';' | '&' | '|' | '\n' => {
                at_command = true;
                p += 1;
            }
            '{' if command && blank(s.get(p + 1)) => {
                groups += 1;
                at_command = true;
                p += 1;
            }
            '}' if command && groups == 0 => return Some(p + 1),
            '}' if command => {
                groups -= 1;
                p += 1;
            }
            c if c.is_whitespace() => {
                at_command = command;
                p += 1;
            }
            _ => p += 1,
        }
    }
    None
}

/// One past the `}` that closes the `${` opened before `p`, or `None`.
fn skip_brace(s: &[char], mut p: usize, nest: usize) -> Option<usize> {
    let mut depth = 0usize;
    while let Some(&c) = s.get(p) {
        match c {
            '\\' => p += 2,
            '\'' => p = s.get(p + 1..)?.iter().position(|q| *q == '\'')? + p + 2,
            '"' => p = skip_double_quoted(s, p + 1, deeper(nest)?)?,
            '`' => p = skip_backtick(s, p + 1)?,
            '$' if s.get(p + 1) == Some(&'(') => p = skip_paren(s, p + 2, deeper(nest)?)?,
            '$' if s.get(p + 1) == Some(&'{') => {
                depth += 1;
                p += 2;
            }
            '}' if depth == 0 => return Some(p + 1),
            '}' => {
                depth -= 1;
                p += 1;
            }
            _ => p += 1,
        }
    }
    None
}

/// Whether `text` has the word `case` in it, anywhere a command could stand.
fn names_a_case(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let word = |c: char| c.is_alphanumeric() || "_-./$".contains(c);
    chars.windows(4).enumerate().any(|(at, w)| {
        w == ['c', 'a', 's', 'e']
            && (at == 0 || !word(chars[at - 1]))
            && chars.get(at + 4).is_none_or(|c| !word(*c))
    })
}

/// The operators after which a command starts.
const CONTROL_OPERATORS: [&str; 8] = [";", ";;", "&&", "||", "|", "|&", "&", "\n"];

/// The redirection operators, each of which takes the word after it as its target.
const REDIRECTIONS: [&str; 9] = ["<", ">", ">>", "<<", "<<<", "<&", ">&", "<>", ">|"];

/// The reserved words after which a command starts, where they stand as reserved words: at a
/// command's start themselves.
const COMMAND_FOLLOWS: [&str; 11] = [
    "!", "{", "then", "do", "else", "elif", "if", "while", "until", "time", "coproc",
];

/// Whether a word is an assignment, `NAME=value`.
fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        let mut chars = name.chars();
        chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// The index of the `)` that closes the `case` pattern starting at `at` (an optional `(`, then
/// words joined by `|`), or `None` where an operator ends it first or nothing closes it.
fn pattern_end(toks: &[shellseg::Tok], at: usize) -> Option<usize> {
    let mut j = at;
    if toks.get(j).is_some_and(|t| t.bare && t.text == "(") {
        j += 1;
    }
    let mut depth = 0usize;
    while let Some(t) = toks.get(j) {
        if t.bare && has_unclosed_expansion(&t.text) {
            return None;
        }
        if t.bare && t.text.chars().all(|c| "();<>|&\n".contains(c)) {
            match t.text.as_str() {
                "(" => depth += 1,
                ")" if depth == 0 => return Some(j),
                ")" => depth -= 1,
                "|" => {}
                _ => return None,
            }
        }
        j += 1;
    }
    None
}

/// Whether a word holds a `${` that no `}` closes within it: the lexer split it at an operator
/// inside the expansion (`${y:-(}`, `${y:-;;}`), so the words after it are not read as the shell
/// reads them (round 7).
fn has_unclosed_expansion(word: &str) -> bool {
    let chars: Vec<char> = word.chars().collect();
    let mut open = 0usize;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 1,
            '$' if chars.get(i + 1) == Some(&'{') => {
                open += 1;
                i += 1;
            }
            '}' => open = open.saturating_sub(1),
            _ => {}
        }
        i += 1;
    }
    open > 0
}

/// Whether the token at `at` is `;&` or `;;&`, which end a `case` arm as `;;` does.
fn is_fallthrough(toks: &[shellseg::Tok], at: usize) -> bool {
    let tok = &toks[at];
    tok.bare
        && (tok.text == ";" || tok.text == ";;")
        && toks
            .get(at + 1)
            .is_some_and(|next| next.bare && next.text == "&" && next.start == tok.end)
}

/// `line` with each arithmetic expansion (`$((…))`) as `0` and each arithmetic command
/// (`((…))` where a command starts) as `:`. A `$((` whose parentheses do not close as `))` is a
/// command substitution of a subshell, and kept. And whether one of them had a `\` or a quote in
/// it, where the parentheses are not read as the shell reads them: the line is then held
/// (round 7).
fn without_arithmetic(line: &str) -> (String, bool) {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut unsure = false;
    let mut i = 0;
    while i < chars.len() {
        let expansion = chars[i] == '$' && chars.get(i + 1) == Some(&'(');
        let at = if expansion { i + 1 } else { i };
        let command = !expansion
            && chars[i] == '('
            && i.checked_sub(1)
                .and_then(|k| chars.get(k))
                .is_none_or(|c| matches!(c, ' ' | '\t' | '\n' | ';' | '&' | '|'));
        if (expansion || command)
            && chars.get(at + 1) == Some(&'(')
            && let Some(end) = arithmetic_end(&chars, at)
        {
            unsure |= chars[at..end]
                .iter()
                .any(|c| matches!(c, '\\' | '\'' | '"'));
            out.push(if expansion { '0' } else { ':' });
            i = end;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    (out, unsure)
}

/// One past the `))` that closes the `((` at `at`, where the inner `(` closes right before the
/// outer one; `None` where it does not, or never closes.
fn arithmetic_end(chars: &[char], at: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (j, c) in chars.iter().enumerate().skip(at) {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 1 {
                    return (chars.get(j + 1) == Some(&')')).then_some(j + 2);
                }
            }
            _ => {}
        }
    }
    None
}

/// Steps `chars` past the `close` that balances the `open` it stands on.
fn skip_to_close(chars: &mut std::iter::Peekable<std::str::Chars>, open: char, close: char) {
    let mut depth = 0;
    for c in chars.by_ref() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
        }
        if depth == 0 {
            break;
        }
    }
}

/// **The commands the floor reads in a line** (#7092): each segment's own words, a
/// substitution one word in its place ([`shellseg::Reading::own`]), and its words with every
/// substitution's handed out to it ([`shellseg::Reading::outward`]), which shows the floor
/// what a substitution could put on the line, unless its own words name no program.
///
/// `A=$(true) gh pr merge 12` is a `gh pr merge` only in its own words. `id=$(gh api … | head
/// -1)` runs nothing but its substitution, whose commands are segments of their own, so it is
/// not read outward as a `gh api` with `head`'s `-1` among its flags.
fn commands_of(line: &str) -> Vec<Vec<String>> {
    let mut commands = Vec::new();
    for reading in shellseg::segment_readings(line) {
        let runs_a_program = !shellwrap::split_env(&reading.own).0.is_empty();
        // A definition's body is read as the commands it will run (`f() { gh pr merge 12; }`).
        for toks in [&reading.outward, &reading.own] {
            if let Some(at) = shellwrap::past_function_headers(toks) {
                commands.push(toks[at..].to_vec());
            }
        }
        if runs_a_program && reading.outward != reading.own {
            commands.push(reading.outward);
        }
        commands.push(reading.own);
    }
    commands
}

/// Whether a segment is a shell, or a program the shell builds that could be one, which runs a
/// script handed to it as a file or on stdin.
fn hands_a_script_to_a_shell(toks: &[String]) -> bool {
    let (prog, ..) = shellwrap::split_env(toks);
    shellwrap::runs_a_script_file(&prog) || prog.contains(['$', '`'])
}

/// What a segment could print into a shell reading stdin: its words as one line, with and
/// without the producer's own leading options (`echo -n`, `printf --`) and, for `printf`, its
/// format; and each word that is a line of its own.
pub(crate) fn fed_lines(toks: &[String]) -> Vec<String> {
    let (prog, _env, argv) = shellwrap::split_env(toks);
    let words = argv.get(1..).unwrap_or(&[]);
    let past_options = words
        .iter()
        .position(|w| !w.starts_with('-'))
        .map_or(&[][..], |at| &words[at..]);
    let mut lines = vec![words.join(" "), past_options.join(" ")];
    if base_lower(&prog) == "printf" {
        lines.push(past_options.get(1..).unwrap_or(&[]).join(" "));
    }
    lines.extend(words.iter().filter(|w| w.contains(' ')).cloned());
    lines.dedup();
    lines
}

/// The bodies of the outermost command and process substitutions in a line: `$(…)`, `` `…` ``,
/// `<(…)` and `>(…)`. A `$((` is an arithmetic expansion only where it closes as `))`; one that
/// does not is a command substitution of a subshell, as bash reads it (#7131). Quoting is not read, so a substitution in single quotes is read too, which
/// only ever refuses more; the caller asks only of a line where the shell runs one. Unbalanced, a
/// body runs to the end of the line. A nested one is found when its body is read in turn.
/// Without `with_process_substitutions`, only command substitutions are read.
///
/// **A bash 5.3 `${ …; }` or `${| …; }` is one too, in quotes or out** (#7246), read to the `}`
/// that closes it as a command ([`skip_funsub`]); a `${` that never closes runs to the line's
/// end, and the second answer says so, for the caller to refuse the line as one it cannot read.
fn substitution_bodies(line: &str, with_process_substitutions: bool) -> (Vec<String>, bool) {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut unclosed_funsub = false;
    let mut i = 0;
    while i < chars.len() {
        if funsub_opens(&chars, i) {
            let open = shellseg::spliced_end(&chars, i, "${").unwrap_or(i + 2);
            let (end, next) = match skip_funsub(&chars, open, 0) {
                Some(after) => (after - 1, after),
                None => {
                    // One the shell would run (after a blank, a newline or `|`) and never closes
                    // is a line to refuse; the wider spellings it refuses itself.
                    unclosed_funsub |= livesub::funsub_at(&chars, i);
                    (chars.len(), chars.len())
                }
            };
            out.push(chars[open..end].iter().collect());
            i = next;
            continue;
        }
        let opens = (chars[i] == '$'
            || (with_process_substitutions && matches!(chars[i], '<' | '>')))
            && chars.get(i + 1) == Some(&'(')
            && !(chars[i] == '$'
                && chars.get(i + 2) == Some(&'(')
                && arithmetic_end(&chars, i + 1).is_some());
        if opens {
            // The close is found as the shell finds it, past a quoted or escaped `)`.
            let (end, next) = match skip_paren(&chars, i + 2, 0) {
                Some(after) => (after - 1, after),
                None => (chars.len(), chars.len()),
            };
            out.push(chars[i + 2..end].iter().collect());
            i = next;
            continue;
        }
        if chars[i] == '`' {
            let close = chars[i + 1..].iter().position(|c| *c == '`');
            let end = close.map_or(chars.len(), |at| i + 1 + at);
            out.push(chars[i + 1..end].iter().collect());
            i = end + 1;
            continue;
        }
        i += 1;
    }
    (out, unclosed_funsub)
}

/// Whether a bash 5.3 `${ …; }` or `${| …; }` opens at `i`, read as widely as the floor needs
/// (#7246): `${` followed by anything that does not start a parameter expansion — a letter, a
/// digit, `_`, or one of `#?@*!$-` — in quotes as out of them. The shell runs it only after a
/// blank, a newline or `|` ([`livesub::funsub_at`]) and refuses the rest as a bad substitution, so
/// the wider reading only ever holds more. It is the floor's own, because the leak and prose
/// guards' answers are recorded against the narrower one.
fn funsub_opens(chars: &[char], i: usize) -> bool {
    livesub::funsub_at(chars, i)
        || shellseg::spliced_end(chars, i, "${").is_some_and(|j| {
            chars
                .get(shellseg::past_continuations(chars, j))
                .is_some_and(|c| {
                    !(c.is_ascii_alphanumeric()
                        || matches!(c, '_' | '#' | '?' | '@' | '*' | '!' | '$' | '-'))
                })
        })
}

/// Whether `text` holds a `${` that only the wide reading ([`funsub_opens`]) takes for a
/// substitution. A `${` the shell reads as text is none (#7250): one escaped, or inside single
/// quotes or `$'…'`, as in `grep -E '\${[A-Z_]+}'`. Double quotes run it, so they are read as
/// open text; an unterminated single quote is read as none, which only holds more. The shell's
/// own spellings are [`livesub::live_substitution`]'s, which reads quotes, so a literal
/// `'${ x; }'` stays one.
fn contains_wide_funsub(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut in_double = false;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 2,
            '"' => {
                in_double = !in_double;
                i += 1;
            }
            '\'' if !in_double => {
                i = chars[i + 1..]
                    .iter()
                    .position(|c| *c == '\'')
                    .map_or(i + 1, |at| i + at + 2);
            }
            '$' if !in_double && chars.get(i + 1) == Some(&'\'') => {
                i = livesub::ansi_c_end(&chars, i + 2);
            }
            _ => {
                if funsub_opens(&chars, i) && !livesub::funsub_at(&chars, i) {
                    return true;
                }
                i += 1;
            }
        }
    }
    false
}

/// Whether `text` holds a bash 5.3 `${ …; }` as the shell runs it ([`livesub::funsub_at`]), not a
/// parameter expansion. The wider spellings are left to [`contains_wide_funsub`]: a word the
/// shell refuses is no program, and the recorded answers hold none of them held.
fn contains_funsub(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    (0..chars.len()).any(|i| livesub::funsub_at(&chars, i))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #7245: the floor's reading of a large commit body is linear in the body. The body is timed
    /// at its size and at four times it, each run on text the others did not see (so no kept
    /// reading answers), the least of three runs each; a reading that grows with the square of
    /// the body costs sixteen times as much at four times the size, and the bound is ten.
    #[test]
    fn the_floor_reads_a_large_body_in_time_linear_in_its_size() {
        use std::time::{Duration, Instant};
        let paragraph = "## Why\n\nThe guard didn't read `\"$(cat <<'EOF'` the way bash does (see \
                         #917), so `echo \"$(date)\"` was refused. It's fixed: `$((1+2))`, \
                         `<(sort a)` and `${x:-y}` are read as substitutions.\n\n\
                         - 1) the reader (shellseg)\n- 2) the floor's \"eval\" walk\n\n";
        let least = |times: usize| {
            (0..3)
                .map(|n| {
                    let body = paragraph.repeat(times * 16_384 / paragraph.len());
                    let tag = times * 10 + n;
                    let cmd = format!("git commit -m \"$(cat <<'EOF'\n{body}run {tag}\nEOF\n)\"");
                    let began = Instant::now();
                    let said = merge_reason(&cmd);
                    (began.elapsed(), said)
                })
                .min_by_key(|(took, _)| *took)
                .expect("three runs")
        };
        let (one, said_one) = least(1);
        let (four, said_four) = least(4);
        assert_eq!(said_one, None, "a body's prose is no merge");
        assert_eq!(said_four, None, "a body's prose is no merge");
        assert!(
            four < one * 10 + Duration::from_millis(50),
            "4x the body took {four:?}, 1x took {one:?}: more than linear"
        );
    }

    /// T1-11's review: the floor reads for merges alone only while a merge read lasts, and a
    /// panic inside one leaves the thread reading the whole floor again, tags and pushes too.
    #[test]
    fn a_panic_in_a_merge_read_leaves_the_floor_whole() {
        let unwound = std::panic::catch_unwind(|| {
            let _only = OnlyMerges::on();
            assert!(only_merges());
            panic!("a merge read that panics");
        });
        assert!(unwound.is_err());
        assert!(!only_merges());
        assert!(release_floor_reason("git tag v1", true).is_some());
    }

    /// #1068: a publishing act floored through one forge CLI is floored through the other.
    #[test]
    fn each_publish_forge_row_has_its_counterpart() {
        let row =
            |cli: &str, (noun, verb): (&str, &str)| PUBLISH_FORGE.contains(&(cli, noun, verb));
        for (gh, glab) in PUBLISH_FORGE_PAIRS {
            assert!(row("gh", gh) && row("glab", glab), "{gh:?} / {glab:?}");
        }
        for (cli, noun, verb) in PUBLISH_FORGE.iter().filter(|(c, ..)| *c != "charter") {
            assert!(
                PUBLISH_FORGE_PAIRS.iter().any(|(gh, glab)| {
                    (*cli == "gh" && *gh == (*noun, *verb))
                        || (*cli == "glab" && *glab == (*noun, *verb))
                }),
                "{cli} {noun} {verb} has no counterpart in PUBLISH_FORGE_PAIRS"
            );
        }
    }

    fn deny(cmd: &str) -> Option<String> {
        release_floor_reason(cmd, true)
    }

    #[test]
    fn an_unattended_run_may_not_publish_a_skill() {
        for cmd in [
            "purlis skill publish deploy --to https://x.invalid/a/b",
            "charter skill publish d",
        ] {
            assert!(release_floor_reason(cmd, true).is_some(), "{cmd}");
        }
        assert_eq!(release_floor_reason("purlis skill list", true), None);
    }

    /// A heredoc body nothing runs is data (#6293), and what a shell does run is still read.
    #[test]
    fn an_unattended_run_may_write_a_heredoc_that_names_a_release() {
        for cmd in [
            "cat >> ledger.md <<'EOF'\n- released with `git push --tags`\nEOF",
            "cat > p.sh <<-'EOF'\n\tgh release create v1\n\tEOF",
            "cat > notes.md <<EOF\ngit tag v1.2.3 for $USER\nEOF",
        ] {
            assert_eq!(release_floor_reason(cmd, true), None, "{cmd}");
        }
        for cmd in [
            "cat > f <<'EOF'\nnotes\nEOF\ngit push --tags",
            "bash <<'EOF'\ngh release create v1\nEOF",
            "cat > f <<EOF\n$(git tag v1.2.3)\nEOF",
        ] {
            assert!(release_floor_reason(cmd, true).is_some(), "{cmd}");
        }
    }

    /// Attended is untouched, whatever the command is.
    #[test]
    fn attended_is_untouched() {
        for cmd in [
            "git tag v1.2.3",
            "gh release create v1",
            "git push --tags",
            "charter change land",
        ] {
            assert_eq!(release_floor_reason(cmd, false), None, "{cmd}");
        }
    }

    /// Creating a tag is the choke point; listing and deleting are not.
    #[test]
    fn creating_a_tag_is_refused_and_reading_one_is_not() {
        assert!(deny("git tag v1.2.3").is_some());
        assert!(deny("git tag release-1").is_some());
        assert_eq!(deny("git tag"), None);
        assert_eq!(deny("git tag -l"), None);
        assert_eq!(deny("git tag --list 'v*'"), None);
        assert_eq!(deny("git tag -d v1"), None);
    }

    /// **A harmless tag flag clears only its own segment** (#348). Listing the tags to work out
    /// the next version and then cutting the release on the same Bash call is the ordinary way
    /// that script is written, and the read must not clear the floor for the publish after it.
    #[test]
    fn a_harmless_tag_flag_clears_only_its_own_segment() {
        for cmd in [
            "git tag -l && gh release create v1.0.0",
            "gh release create v1.0.0 && git tag -l",
            "git tag -l && git tag v1.0.0",
            "git tag -l >/dev/null && git push --tags",
            "git tag -l && charter change land",
            "git tag -d v0 ; gh pr merge 12",
            "git tag --list 'v*' | tail -1 && git tag v2",
        ] {
            assert!(deny(cmd).is_some(), "{cmd}");
        }
        // The read alone is still a read.
        assert_eq!(deny("git tag -l"), None);
        assert_eq!(deny("git tag -l && git tag -d v1"), None);
        assert_eq!(deny("git tag -l && git log --oneline"), None);
    }

    /// A push carrying tags, in each of its three spellings.
    #[test]
    fn a_push_that_carries_a_tag_is_refused() {
        assert!(deny("git push --tags").is_some());
        assert!(deny("git push --follow-tags origin main").is_some());
        assert!(deny("git push origin refs/tags/v1").is_some());
        assert!(deny("git push origin v1.2.3").is_some());
        assert!(deny("git push origin 1.2").is_some());
        assert_eq!(deny("git push origin main"), None);
        // The version scan skips `words[0]`, which is the subcommand itself.
        assert_eq!(deny("git push"), None);
    }

    /// CPython's `\w` counts a combining mark and the crate's does not. The class is written out
    /// as `[\p{L}\p{N}_]`, which #92 measured to be CPython's over every codepoint.
    #[test]
    fn the_version_shape_uses_cpythons_word_class() {
        // A variation selector is a `\w` to the `regex` crate and not to CPython, so a tag ending
        // in one must NOT match — CPython's `fullmatch` fails on it.
        assert!(!version_tag_re().is_match("v1.2.3\u{fe0f}"));
        assert!(version_tag_re().is_match("v1.2.3-rc1"));
        // A Unicode decimal digit is `\d` in both engines.
        assert!(version_tag_re().is_match("v\u{0663}.\u{0664}"));
        // `fullmatch` allows no trailing newline, unlike `$`.
        assert!(!version_tag_re().is_match("v1.2\n"));
    }

    /// The forge table, and the branch that reaches it.
    #[test]
    fn a_forge_publish_is_refused_and_opening_a_request_is_not() {
        assert!(deny("gh release create v1").is_some());
        assert!(deny("gh pr merge 12").is_some());
        assert!(deny("glab mr merge 12").is_some());
        assert!(deny("glab release create v1").is_some());
        assert_eq!(deny("gh pr create --title x"), None);
        assert_eq!(deny("gh issue create --title x"), None);
    }

    /// charter's own landing verb, in every spelling `is_charter` knows.
    #[test]
    fn charters_own_landing_verb_is_on_the_floor() {
        assert!(deny("charter change land").is_some());
        assert!(deny("edm change land").is_some());
        assert!(deny("python3 -m charter change land").is_some());
        assert!(deny("/usr/local/bin/charter change land").is_some());
        assert_eq!(deny("charter change show"), None);
        assert_eq!(deny("charter change list"), None);
    }

    /// The denial names the command it refused, which is what makes the floor arguable rather
    /// than mysterious.
    #[test]
    fn the_denial_names_the_command() {
        let said = deny("gh pr merge 12").expect("a denial");
        assert!(said.starts_with(RELEASE_FLOOR_FIX));
        assert!(
            said.contains("`gh pr merge` publishes or lands code."),
            "{said}"
        );
    }

    /// The floor reads each level of a nested line whole, so a long line nested deep was read
    /// once per level (#1409). Its whole reading is now held to [`WORK_FACTOR`] times the line
    /// plus [`WORK_SLACK`] — counted, not timed — and a line past it is refused as unreadable.
    #[test]
    fn a_long_line_nested_deep_is_read_within_its_work_budget() {
        let deep = MAX_DEPTH;
        let filler = "a ".repeat(crate::guardcaps::MAX_COMMAND_BYTES / 2 - 64);
        let cmd = format!("{}{filler}{}", "echo $(".repeat(deep), ")".repeat(deep));
        let (said, read) = metered(&cmd, true);
        let budget = cmd.len() * WORK_FACTOR + WORK_SLACK;
        // The level that crosses the budget is the last one read.
        assert!(
            read <= budget + cmd.len(),
            "read {read} of a budget of {budget}"
        );
        assert!(
            said.is_some_and(|said| said.ends_with(merge::UNREADABLE)),
            "a line past its budget is refused as unreadable"
        );
        // Attended, the floor reads nothing.
        assert_eq!(metered(&cmd, false), (None, 0));
    }

    /// A `)` in the data of a quoted heredoc closes no substitution: a commit body's `1)` or
    /// `(shellseg)` is text, so the line is read whole and passes, while the same line with a
    /// merge beside it is still held.
    #[test]
    fn a_paren_in_a_quoted_heredoc_body_does_not_close_the_substitution() {
        for body in ["- 1) the reader\n", "see (shellseg) and 2)\n", "a ) b\n"] {
            for open in ["<<'EOF'", "<<\"EOF\"", "<<-'EOF'"] {
                let cmd = format!("git commit -m \"$(cat {open}\n{body}EOF\n)\"");
                assert_eq!(merge_reason(&cmd), None, "{cmd}");
            }
        }
        let held = "git commit -m \"$(cat <<'EOF'\n- 1) x\nEOF\n)\"; gh pr merge 12";
        assert!(merge_reason(held).is_some());
        let inside = "x=\"$(cat <<'EOF'\n- 1) x\nEOF\n; gh pr merge 12)\"";
        assert!(merge_reason(inside).is_some());
    }

    /// The ruling on #7131's checks: a body line read as commands only as bash 3.2's hedge is
    /// never refused for "could print any command", but a literal merge in it stays held, and a
    /// program built by the shell on an unhedged line is still refused.
    #[test]
    fn a_hedged_body_line_is_held_for_a_merge_but_not_for_what_it_could_print() {
        let body = "The guard didn't read `\"$(cat <<'EOF'` the way (see #917)\n- 1) first\n";
        let ordinary = format!("git commit -m \"$(cat <<'EOF'\n{body}EOF\n)\"");
        assert_eq!(merge_reason(&ordinary), None, "{ordinary}");
        let merged = format!("git commit -m \"$(cat <<'EOF'\n{body}gh pr merge 7\nEOF\n)\"");
        assert!(merge_reason(&merged).is_some(), "{merged}");
        assert!(merge_reason("\"$(echo gh)\" pr merge 7").is_some());
        assert!(merge_reason("$(echo gh) pr merge 7").is_some());
    }

    /// A commit body whose `)` and quote make bash 3.2's hedge read its lines as commands.
    fn hedged(lines: &str) -> String {
        format!(
            "git commit -m \"$(cat <<'EOF'\nThe guard didn't read `\"$(cat <<'EOF'` the way (see #917)\n{lines}\nEOF\n)\""
        )
    }

    /// Whether `cmd` is refused for the program the shell builds on one of its lines, as
    /// opposed to any other refusal it earns.
    fn refused_for_a_built_program(cmd: &str) -> bool {
        merge_reason(cmd).is_some_and(|said| {
            said.contains("is built by the shell") || said.contains("could print any command")
        })
    }

    /// Round 2 of #7245: only a line read as commands for the hedge and for no other reason is
    /// spared the speculative checks. A line a shell runs outright is not, so what the shells
    /// read as commands after an arithmetic shift, a heredoc closed on its line, or an earlier
    /// body read both ways is refused when it builds its program.
    #[test]
    fn a_line_a_shell_runs_outright_is_refused_for_a_program_it_builds() {
        for built in [
            "g*h pr view 7",
            "{gh,pr,view,7}",
            "\"$(echo gh)\" pr view 7",
        ] {
            // A heredoc a shell closes on its header's line: the body is lines it runs.
            let closed = format!("x=$(cat <<'EOF' ); echo\n{built}\ntrue\nEOF\n");
            assert!(refused_for_a_built_program(&closed), "{closed}");
            // A body after one the shells read differently, shaped as the hedge's.
            let after = format!(
                "x=$(cat <<'EOF' ); echo\nfoo\nEOF\ngit commit -m \"$(cat <<'EOF'\nsee `\"$(x)` (a)\n{built}\nEOF\n)\""
            );
            assert!(refused_for_a_built_program(&after), "{after}");
            // A `<<` in an arithmetic expansion that a shell reads as a shift.
            let shift = format!("echo $((1<<2))\n{built}\ntrue\n2\n");
            assert!(refused_for_a_built_program(&shift), "{shift}");
        }
    }

    /// Only the hedge is spared: the same shapes with a built program on a hedged line pass.
    #[test]
    fn a_built_program_on_a_hedged_line_is_not_refused() {
        for built in [
            "{gh,pr,view,7}",
            "$(echo gh) pr view 7",
            "\"$(echo gh)\" pr view 7",
        ] {
            let text = hedged(built);
            assert_eq!(merge_reason(&text), None, "{text}");
        }
    }

    /// A hedged line is read for a literal merge joined with its neighbours and on its own, with
    /// a `case` arm's command in its place, in every merge form.
    #[test]
    fn a_literal_merge_on_a_hedged_line_is_held_in_every_form() {
        let held = |cmd: &str| release_floor_reason(cmd, true);
        for merge in [
            "gh pr merge 7",
            "gh pr \\\nmerge 7",
            "x) gh pr merge 7",
            "glab mr merge 7",
            "gh pr merge --auto 7",
            "gh api -X PUT repos/o/r/pulls/7/merge",
            "git push --follow-tags origin main",
            "echo \"$(gh pr merge 7)\"",
            "echo `gh pr merge 7`",
        ] {
            // An arm's line stands in a `case` of its own.
            let plain = if merge.starts_with("x)") {
                format!("case y in\n{merge}\nesac")
            } else {
                merge.to_owned()
            };
            assert!(held(&plain).is_some(), "{plain}");
            let text = hedged(&plain);
            assert!(held(&text).is_some(), "{text}");
            let unterminated = format!("git commit -m \"$(cat <<'EOF'\nsee `\"$(x)` (a)\n{plain}");
            assert!(held(&unterminated).is_some(), "{unterminated}");
        }
    }

    /// Round 3 of #7245: under the hedge only the two ruled checks are skipped. A shell a hedged
    /// line builds is fed by that line, and a `case` in a substitution on one is held.
    #[test]
    fn a_hedged_line_is_held_for_a_built_shell_and_a_case_in_a_substitution() {
        for held in [
            "echo gh pr merge 7 | $SHELL",
            "echo gh pr merge 7 | \"$(echo bash)\"",
            "echo \"$(case y in (a) gh pr merge 7;; esac)\"",
        ] {
            let text = hedged(held);
            assert!(merge_reason(&text).is_some(), "{text}");
        }
    }

    /// A quote one hedged line opens and the next closes is read right by the joined reading.
    #[test]
    fn a_quote_closed_on_the_next_hedged_line_leaves_a_merge_held() {
        let text = hedged("echo \"x\ny\"; gh pr merge 7");
        assert!(merge_reason(&text).is_some(), "{text}");
    }

    /// An unquoted heredoc's substitutions are still read, hedge or not.
    #[test]
    fn an_unquoted_heredoc_substitution_is_still_read() {
        let text = "cat <<EOF\nsee (a) `\"$(x)`\n$(gh pr merge 7)\nEOF";
        assert!(merge_reason(text).is_some(), "{text}");
    }

    /// A here-string is not a heredoc: it hides nothing from `skip_paren`.
    #[test]
    fn a_here_string_in_a_substitution_opens_no_heredoc() {
        let s: Vec<char> = "cat <<<'EOF'\n)\nEOF".chars().collect();
        assert_eq!(
            skip_paren(&s, 0, 0),
            Some("cat <<<'EOF'\n)".chars().count())
        );
        let text = "x=$(cat <<<'EOF'\n)\ngh pr merge 7\nEOF";
        assert!(merge_reason(text).is_some(), "{text}");
    }

    /// The hedge reading is only for as long as it lasts, a panic's unwinding included.
    #[test]
    fn a_panic_in_a_hedge_read_leaves_the_speculative_checks_on() {
        let unwound = std::panic::catch_unwind(|| {
            let _hedge = ReadingAHedge::on();
            assert!(reading_a_hedge());
            panic!("a hedge read that panics");
        });
        assert!(unwound.is_err());
        assert!(!reading_a_hedge());
        assert!(merge_reason("{gh,pr,merge,7}").is_some());
    }

    /// The large commit body of the CLI's budget test: a `)` and an odd quote in it, so it is read
    /// as commands as bash 3.2's hedge, and optionally a merge on its last line.
    fn a_large_hedged_commit(merging: bool) -> String {
        let paragraph = "The guard didn't read `\"$(cat <<'EOF'` (see #917)\n- 1) the reader\n\n";
        let body = paragraph.repeat(65_536 / paragraph.len());
        let merge = if merging { "gh pr merge 7\n" } else { "" };
        format!("git commit -m \"$(cat <<'EOF'\n{body}{merge}EOF\n)\"")
    }

    /// The floor itself, with no chat, plane or host in it, holds the merge on a line of a large
    /// hedged body and passes the ordinary one: what a CI runner and a chat both get.
    #[test]
    fn a_merge_line_in_a_large_hedged_body_is_held_by_the_floor_alone() {
        let merging = a_large_hedged_commit(true);
        assert!(merge_reason(&merging).is_some());
        assert!(release_floor_reason(&merging, true).is_some());
        assert_eq!(
            release_floor_reason(&a_large_hedged_commit(false), true),
            None
        );
    }

    /// **A cutoff refuses, whatever it cuts** (#7245, round 4): with a work budget forced tiny, the
    /// large body with a merge line and the one without are both refused as unreadable, never
    /// allowed; and a cutoff whose own refusal was swallowed still ends as one.
    #[test]
    fn a_reading_cut_short_by_a_tiny_budget_is_refused_never_allowed() {
        for merging in [true, false] {
            let cmd = a_large_hedged_commit(merging);
            for allowed in [1, 100, 10_000, cmd.len() + 10] {
                let outer = WORK.replace(Some(Work { read: 0, allowed }));
                CUT_SHORT.set(false);
                let said = floor(&cmd, true, 0);
                CUT_SHORT.set(false);
                WORK.set(outer);
                assert!(
                    said.is_some_and(|said| said.contains(merge::UNREADABLE)),
                    "merging {merging}, budget {allowed}: not refused"
                );
            }
        }
        // A limit crossed whose refusal nothing returned still ends as a refusal.
        assert!(closed_on_a_cutoff(None, true, true).is_some());
        assert_eq!(closed_on_a_cutoff(None, false, true), None);
    }

    /// The ordinary hedged body (`- 1) first`, a brace-expanded `gh pr view`) still passes with the
    /// per-line `case` wrapper closed, and a body line that is a real unparsed `case` is held.
    #[test]
    fn an_ordinary_hedged_body_passes_and_a_real_unparsed_case_in_it_is_held() {
        let body = "see `\"$(x)` (a)\n- 1) first\n{gh,pr,view,7}\n";
        let ordinary = format!("git commit -m \"$(cat <<'EOF'\n{body}EOF\n)\"");
        assert_eq!(release_floor_reason(&ordinary, true), None, "{ordinary}");
        let case = format!("git commit -m \"$(cat <<'EOF'\n{body}case x in a) echo $(\nEOF\n)\"");
        assert!(release_floor_reason(&case, true).is_some(), "{case}");
    }

    /// What a run really sends — a long pull request body in a substitution, a long script fed
    /// to a shell — is read whole, inside the budget, and not refused for its length.
    #[test]
    fn a_long_body_or_script_is_read_whole() {
        let body = "Line of a long pull request body.\n".repeat(2800);
        for cmd in [
            format!("gh pr create --title t --body \"$(cat <<'EOF'\n{body}EOF\n)\""),
            format!("echo '{}' | sh", "ls -la; ".repeat(10_000)),
            "git status && cargo test".to_owned(),
        ] {
            let (said, read) = metered(&cmd, true);
            assert_eq!(said, None, "{}", &cmd[..60]);
            assert!(read <= cmd.len() * WORK_FACTOR + WORK_SLACK, "read {read}");
        }
    }

    /// **The ordinary large body passes where a chat runs it, and what truly can't be closed is
    /// still held** (#7245, round 5): prose with `<<'EOF'` and `(see #917)` in a quoted heredoc is
    /// data, so the hedge's re-spelling of it never refuses as an unclosed substitution.
    #[test]
    fn a_hedged_body_is_not_refused_as_unclosable_but_a_real_unclosable_one_is() {
        let paragraph = "## Why\n\nThe guard didn't read `\"$(cat <<'EOF'` the way bash does (see \
                         #917), so `echo \"$(date)\"` was refused. It's fixed: `$((1+2))`, \
                         `<(sort a)` and `${x:-y}` are read as substitutions.\n\n\
                         - 1) the reader (shellseg)\n- 2) the floor's \"eval\" walk\n\n";
        let body = paragraph.repeat(65_536 / paragraph.len());
        let ordinary = format!("git commit -m \"$(cat <<'EOF'\n{body}EOF\n)\"");
        assert_eq!(release_floor_reason(&ordinary, true), None, "{ordinary}");
        // An unquoted heredoc that never ends, and a quoted one with no delimiter line, are held.
        for cmd in [
            "echo \"$(cat <<EOF\nhello\n)\"",
            "echo \"$(cat <<'EOF'\nhello\n)\"",
            "echo \"$(cat <<'EOF'\n(see #917)\n",
        ] {
            let said = release_floor_reason(cmd, true);
            assert!(
                said.is_some_and(|said| said.contains("can't tell where it ends")),
                "{cmd}"
            );
        }
    }

    /// **A hedged line with a real merge and an unclosable fragment is still held, for the
    /// merge** (#7245, round 5 ruling): the unclosable check is skipped only for a line read as
    /// commands solely by bash 3.2's hedge, after that line's merge readings found nothing.
    #[test]
    fn a_hedged_line_with_a_merge_and_an_unclosable_fragment_is_held_for_the_merge() {
        let paragraph = "The guard didn't read `\"$(cat <<'EOF'` (see #917), so `echo \"$(date)\"` \
                         was refused.\n- 1) the reader\n\n";
        let body = paragraph.repeat(65_536 / paragraph.len());
        for merge_line in ["gh pr merge 7", "gh pr merge 7 && echo \"$(cat <<EOF)"] {
            let cmd = format!("git commit -m \"$(cat <<'EOF'\n{body}{merge_line}\nEOF\n)\"");
            let said = release_floor_reason(&cmd, true)
                .unwrap_or_else(|| panic!("not held: {merge_line}"));
            assert!(
                !said.contains("can't tell where it ends"),
                "{merge_line}: {said}"
            );
        }
    }
}
