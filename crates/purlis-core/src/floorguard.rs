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

/// [`release_floor_reason`], and how many bytes of text the floor read on the way, every level
/// counted ([`Work`]).
fn metered(cmd: &str, unattended: bool) -> (Option<String>, usize) {
    let outer = WORK.replace(Some(Work {
        read: 0,
        allowed: cmd.len().saturating_mul(WORK_FACTOR) + WORK_SLACK,
    }));
    let said = floor(cmd, unattended, 0);
    let read = WORK.replace(outer).map_or(0, |work| work.read);
    (said, read)
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
    WORK.with(|work| match work.get() {
        Some(mut now) => {
            now.read = now.read.saturating_add(text.len());
            work.set(Some(now));
            now.read <= now.allowed
        }
        None => true,
    })
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
    if depth > MAX_DEPTH || !may_read(cmd) {
        return Some(format!("{fix}{}", merge::UNREADABLE));
    }
    let inner_floor = |inner: &str| floor(inner, unattended, depth + 1);
    // A heredoc body nothing runs is data, and only the substitutions of one the shell
    // expands run (#6293).
    let segments = commands_of(&crate::heredoc::commands_a_shell_runs(cmd));
    // A shell that reads its script from stdin is fed by the rest of the line (#866).
    let a_shell_reads_stdin = segments
        .iter()
        .any(|toks| shellwrap::reads_script_from_stdin(toks));
    // A substitution the shell runs, wherever it stands, even inside double quotes, read from
    // the line as written: the words a substitution's own quotes split are not its body (#866).
    if livesub::may_substitute(cmd) && livesub::live_substitution(cmd).is_some() {
        for body in substitution_bodies(cmd) {
            if let Some(said) = inner_floor(&body) {
                return Some(said);
            }
        }
    }
    // A program the shell builds, from a substitution, a brace expansion or a glob, could be any
    // command with any words after it, so it is unreadable outright (#7131). Read only where a
    // shell runs the line as a command: a heredoc body's substitution is data in its place.
    // A glob or a brace counts only where a command starts: not in a `case` pattern, an
    // arithmetic expansion or an array's words, nor in a body an interpreter runs.
    let lines = in_command_position(&crate::heredoc::lines_a_shell_runs(cmd));
    for reading in shellseg::segment_readings(&lines) {
        let (prog, ..) = shellwrap::split_env(&reading.own);
        if program_built_by_the_shell(&prog) {
            return Some(format!(
                "{fix}{} The program `{prog}` is built by the shell, so purlis can't read which \
                 command this is. {WRITE_IT_OUT}",
                merge::UNREADABLE
            ));
        }
        // A substitution the program word opens with is split into words, so a path after it
        // makes no path of it: what it prints is the command (`$(echo gh pr merge 12)/y`).
        if prog.starts_with("$(")
            && printed_program(&reading.outward)
                .iter()
                .any(|printed| inner_floor(printed).is_some())
        {
            return Some(format!(
                "{fix}{} The program `{prog}` opens with a substitution the shell splits into \
                 words, and what it prints would be refused. {WRITE_IT_OUT}",
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
    let (prog, env, argv) = shellwrap::split_env(toks);
    // A string a shell runs is read as the command it is (#866).
    let scripts = shellwrap::shell_scripts_of(&prog, &argv)
        .into_iter()
        .chain(shellwrap::here_string_script(toks));
    let fed = if a_shell_reads_stdin && !shellwrap::reads_script_from_stdin(toks) {
        fed_lines(toks)
    } else {
        Vec::new()
    };
    for inner in scripts.chain(fed) {
        if let Some(said) = inner_floor(&inner) {
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
            if args.iter().any(|a| a == "--tags" || a == "--follow-tags") {
                return Some(format!("{fix}`--tags`/`--follow-tags` pushes tags."));
            }
            // `--mirror` pushes every ref, its tags among them (#7131).
            if args.iter().any(|a| a == "--mirror") {
                return Some(format!("{fix}`--mirror` pushes every ref, tags included."));
            }
            if follows_tags_by_config(args) {
                return Some(format!(
                    "{fix}`push.followTags` set on the command line pushes tags."
                ));
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
/// notes.md` is read as each and refused as none.
const FILLED_IN_PROGRAM_READ_AS: [&str; 4] = ["gh", "glab", "git", crate::cliname::ALIAS];

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
        if matches!(
            a.as_str(),
            "-o" | "--push-option" | "--repo" | "--receive-pack" | "--exec"
        ) {
            rest.next();
        } else if !a.starts_with('-') {
            words.push(a.as_str());
        }
    }
    words.into_iter().skip(1).collect()
}

/// Whether git's own options before `push` set `push.followTags` on (`-c push.followTags=true`,
/// `-c push.followtags`), which pushes the annotated tags the pushed commits reach (#7131).
fn follows_tags_by_config(args: &[String]) -> bool {
    let mut i = 0;
    while i < args.len() {
        let setting = match args[i].as_str() {
            "-c" => {
                i += 1;
                args.get(i).map(String::as_str)
            }
            a => a.strip_prefix("-c").filter(|s| !s.is_empty()),
        };
        if let Some(setting) = setting {
            let (key, value) = setting.split_once('=').unwrap_or((setting, "true"));
            let off = matches!(
                value.to_ascii_lowercase().as_str(),
                "false" | "no" | "off" | "0"
            );
            if key.eq_ignore_ascii_case("push.followtags") && !off {
                return true;
            }
        }
        if args[i] == "push" {
            break;
        }
        i += 1;
    }
    false
}

/// The refs a refspec pushes to, as the remote reads it: `HEAD:refs/tags/v1` is `refs/tags/v1`,
/// `+main:main` is `main`, and a ref with no `:` is its own destination (#7131).
fn refspec_end(spec: &str) -> &str {
    spec.rsplit(':')
        .next()
        .unwrap_or(spec)
        .trim_start_matches('+')
}

/// Whether a program is one the shell builds, so its words cannot be read (#7131): its last
/// path component holds a command substitution (`$(echo gh pr)`, `` `echo gh` ``) or a glob
/// (`g[h]`), or the word holds a brace expansion anywhere (`{gh,pr,merge,12}`,
/// `{/opt/homebrew/bin/gh,pr,merge,12}`), which splits it into words. `$(git rev-parse
/// --show-toplevel)/x.sh` is a path to `x.sh`, a parameter expansion (`${cmd[@]}`) is a program
/// the shell fills in ([`FILLED_IN_PROGRAM_READ_AS`]), and `[` and `[[` are the test builtins.
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
    last.contains('\u{0}')
        || last.contains(['*', '?'])
        || glob_class
        || merge::brace_expansion(&whole)
}

/// The command lines a program word that opens with a substitution could print, read from
/// the segment's outward words (`$`, the substitution's words, then the rest): what its body
/// prints, as [`fed_lines`] reads a producer, and the words after it.
fn printed_program(outward: &[String]) -> Vec<String> {
    let Some(at) = outward.iter().position(|w| w == "$") else {
        return Vec::new();
    };
    if !outward[..at].iter().all(|w| w.contains('=')) {
        return Vec::new();
    }
    fed_lines(&outward[at + 1..])
}

/// `line` with what stands in no command position blanked out, for the reading of a program
/// the shell builds (#7131): an arithmetic expansion or command (`$((2*3))`, `((i*2))`), a
/// `case` pattern through its `)` (`*)`, `y*|n*)`), and an array's words (`files=(src/*.rs)`).
/// A glob or a brace there is no program. A substitution inside an arithmetic expansion is
/// still read, by its own body. A line that will not lex keeps its patterns and its arrays.
fn in_command_position(line: &str) -> String {
    let line = without_arithmetic(line);
    let Ok(toks) = shellseg::lex(&line) else {
        return line;
    };
    let toks = shellseg::split_punctuation(toks);
    let mut chars: Vec<char> = line.chars().collect();
    let mut blank = |from: isize, to: isize| {
        let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
            return;
        };
        for c in chars.iter_mut().take(to).skip(from) {
            *c = ' ';
        }
    };
    /// Where a `case` is: its word, its patterns, or an arm's commands.
    enum Case {
        Word,
        Patterns,
        Arm,
    }
    let mut cases: Vec<Case> = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let tok = &toks[i];
        let bare = |text: &str| tok.bare && tok.text == text;
        match cases.last() {
            // `case WORD in` is no command: the first arm's starts after its pattern.
            Some(Case::Word) => {
                blank(tok.start, tok.end);
                if bare("in") {
                    cases.pop();
                    cases.push(Case::Patterns);
                }
            }
            Some(Case::Patterns) if bare("esac") => {
                cases.pop();
            }
            Some(Case::Patterns) if !(tok.bare && matches!(tok.text.as_str(), "\n" | "&")) => {
                // A pattern runs to the `)` that closes it; a `(` before it is optional.
                let from = tok.start;
                let mut depth = 0usize;
                let mut j = i;
                if bare("(") {
                    j += 1;
                }
                while let Some(t) = toks.get(j) {
                    if t.bare && t.text == "(" {
                        depth += 1;
                    } else if t.bare && t.text == ")" {
                        if depth == 0 {
                            break;
                        }
                        depth -= 1;
                    }
                    j += 1;
                }
                let to = toks.get(j).map_or(isize::MAX, |t| t.end);
                blank(from, to);
                cases.pop();
                cases.push(Case::Arm);
                i = j + 1;
                continue;
            }
            Some(Case::Arm) if tok.bare && (tok.text == ";;" || is_fallthrough(&toks, i)) => {
                cases.pop();
                cases.push(Case::Patterns);
            }
            Some(Case::Arm) if bare("esac") => {
                cases.pop();
            }
            _ if bare("case") && command_starts_at(&toks, i) => {
                blank(tok.start, tok.end);
                cases.push(Case::Word);
            }
            _ => {
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
                    i = j + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    chars.into_iter().collect()
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

/// Whether a command starts at the token `at`: the line's first, or one after a control
/// operator, a group's opening or a keyword that a command follows.
fn command_starts_at(toks: &[shellseg::Tok], at: usize) -> bool {
    at.checked_sub(1)
        .and_then(|k| toks.get(k))
        .is_none_or(|prev| {
            prev.bare
                && matches!(
                    prev.text.as_str(),
                    ";" | ";;"
                        | "&&"
                        | "||"
                        | "|"
                        | "|&"
                        | "&"
                        | "\n"
                        | "("
                        | ")"
                        | "{"
                        | "!"
                        | "then"
                        | "do"
                        | "else"
                        | "elif"
                        | "if"
                        | "while"
                        | "until"
                )
        })
}

/// `line` with each arithmetic expansion (`$((…))`) as `0` and each arithmetic command
/// (`((…))` where a command starts) as `:`. A `$((` whose parentheses do not close as `))` is a
/// command substitution of a subshell, and kept.
fn without_arithmetic(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
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
            out.push(if expansion { '0' } else { ':' });
            i = end;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
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
        if runs_a_program && reading.outward != reading.own {
            commands.push(reading.outward);
        }
        commands.push(reading.own);
    }
    commands
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
/// `<(…)` and `>(…)`. Quoting is not read, so a substitution in single quotes is read too, which
/// only ever refuses more; the caller asks only of a line where the shell runs one. Unbalanced, a
/// body runs to the end of the line. A nested one is found when its body is read in turn.
fn substitution_bodies(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let opens = matches!(chars[i], '$' | '<' | '>')
            && chars.get(i + 1) == Some(&'(')
            && !(chars[i] == '$' && chars.get(i + 2) == Some(&'('));
        if opens {
            let mut depth = 1;
            let mut j = i + 2;
            while j < chars.len() && depth > 0 {
                match chars[j] {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                j += 1;
            }
            let end = if depth == 0 { j - 1 } else { j };
            out.push(chars[i + 2..end].iter().collect());
            i = j;
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
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
