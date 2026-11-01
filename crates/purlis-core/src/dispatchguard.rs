// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A helper sub-agent may not dispatch: the one fact about a dispatch only a tool hook knows
//! (#1434, #1436).
//!
//! Every other question about a dispatch is answered by the app, from its own record of the
//! asking chat ([`crate::dispatchdecision`]). This one cannot be: a harness's sub-agent runs
//! inside its chat's process, with the chat's environment and the chat's token, so a `purlis
//! dispatch` it runs reaches the app looking exactly like the chat's own. What tells them apart
//! is the harness's hook payload, which names the sub-agent that made a tool call (`agent_id`,
//! on the harnesses where that was measured to mean one,
//! [`crate::handoffguard::MEASURED_SUBAGENT_HARNESSES`]). So the refusal stands here, in front
//! of the tool call, and the sentence it says is the decision's own.
//!
//! Two calls are read: a Bash command that runs `purlis dispatch` in any segment
//! ([`refusal`]), and a call of purlis's own `dispatch` chat tools ([`tool_refusal`]).
//!
//! **This is advice a sub-agent meets, not a boundary it cannot cross.** It is a recognition
//! of the usual shapes and never a shell, as the handoff guard says of itself. It reads a
//! dispatch in every text a shell is handed to run, at every level up to the nesting cap: a
//! `-c` string or an `eval`, one inside another, a here-string, a pipe into a shell, a heredoc
//! fed to one, the command a `find -exec` runs (#1582, through the walk the other backstops
//! share). What stays unseen: a shell outside the ones the guards name
//! ([`shellwrap::STRING_SHELLS`]), or one reached through a remote login, another user or a
//! container; a program behind a variable other than the one that names purlis's binary; a
//! script written in one call and run in the next; a dispatch inside an interpreter. On a
//! harness where `agent_id` was never measured (opencode) nothing is refused at all. A
//! sub-agent that gets past it reaches exactly what its chat reaches, and no more: the
//! chat its own chat could have started, listed under that chat where the person sees it. So
//! nothing that grants a chat a dispatch should be described as never reaching its helpers.

use crate::consentspelling::{self, Nested};
use crate::dispatchdecision::{self, Asker, Decision, Mode, Persona, Request};
use crate::handoffguard::Caller;
use crate::proseguard::charter_words;
use crate::shellseg::{self, Tok};
use crate::shellwrap;

/// The trace reason a refused dispatch is tallied under.
pub const REASON_SUBAGENT: &str = "dispatch-subagent";

/// The word after the program that makes a command a dispatch.
const WORD: &str = "dispatch";

/// Whether `cmd` runs `purlis dispatch`, in any spelling purlis recognises of its own command
/// line: a report back included, which is the chat's own to send.
///
/// Three places are read, each with a reader the other guards already use:
///
/// - **every segment** ([`shellseg::segment_argv`]). Newlines are segment boundaries, so a
///   line of a heredoc body that itself begins `purlis dispatch` counts too;
/// - **one level into a string a shell runs** ([`shellwrap::shell_scripts`]): `bash -c '…'`,
///   `sh -c "…"`, `eval …`, as the handoff guard looks into them; **then every level a shell
///   is handed a text at** ([`further_in`], #1582): a string inside that string, a here-string
///   or a pipe into a shell, a heredoc fed to one, a `find -exec`; past the nesting cap, a
///   helper is refused unread;
/// - **the variable that names purlis's own binary** as the program
///   ([`crate::plugin::BINARY_ENV`], under either name), which every chat the app starts has in
///   its environment.
///
/// Each over-match refuses only a sub-agent, which is the safe direction.
pub fn runs_dispatch(cmd: &str) -> bool {
    a_segment_runs_it(cmd)
        || a_shell_string_runs_it(cmd)
        // Past the cap the text is not read, and a helper is refused all the same.
        || further_in(cmd, |text| a_segment_runs_it(text).then_some(())).is_some()
}

/// What `look` finds in a text a shell is handed to run, past what the first-level readings
/// here read (#1582): a `-c` string inside a `-c` string, a here-string or a pipe into a shell,
/// a heredoc fed to one, the command a `find -exec` runs, at every level up to the nesting cap.
/// Read through the one walk the handoff, consent and operator-rule backstops share
/// ([`consentspelling::in_shell_strings_at_every_level_by`]), over the shared heredoc plan;
/// `look` is handed each text as the shell reads it.
///
/// A text handed on past the cap is not read: it is [`Nested::TooDeep`], for the caller to
/// refuse. Asked only of a call that may name the subcommand at all ([`shellwrap::may_name`]),
/// so a command that names none is not read again.
fn further_in<T>(cmd: &str, look: impl Fn(&str) -> Option<T>) -> Option<Nested<T>> {
    if !shellwrap::may_name(cmd, WORD) {
        return None;
    }
    let read = crate::heredoc::Heredocs::of(cmd);
    consentspelling::in_shell_strings_at_every_level_by(&read, &|inner| {
        look(&crate::handoffguard::as_the_shell_reads(&inner.text))
    })
}

fn a_segment_runs_it(cmd: &str) -> bool {
    shellseg::segment_argv(cmd).iter().any(|toks| {
        let (prog, _env, argv) = shellwrap::split_env(toks);
        charter_words(&prog, &argv).is_some_and(|words| words.first().is_some_and(|w| w == WORD))
            || (names_our_binary(&prog) && argv.get(1).is_some_and(|word| word == WORD))
    })
}

/// Whether a program word is the variable the app names purlis's binary in: `$NAME` or
/// `${NAME}`, as the source spells it once its quotes are off.
fn names_our_binary(prog: &str) -> bool {
    let Some(name) = prog.strip_prefix('$') else {
        return false;
    };
    let name = name
        .strip_prefix('{')
        .and_then(|name| name.strip_suffix('}'))
        .unwrap_or(name);
    crate::envvar::spellings(crate::plugin::BINARY_ENV)
        .iter()
        .any(|spelling| spelling == name)
}

/// One level in, and no deeper: a string inside that string is [`further_in`]'s.
fn a_shell_string_runs_it(cmd: &str) -> bool {
    let Ok(toks) = shellseg::lex(&crate::heredoc::strip_reader_heredocs(cmd)) else {
        return false;
    };
    let toks = shellseg::split_punctuation(toks);
    crate::heredoc::segments_of(&toks)
        .iter()
        .any(|(seg, _before)| {
            let words: Vec<String> = seg.iter().map(|tok| tok.text.clone()).collect();
            shellwrap::shell_scripts(&words)
                .iter()
                .any(|inner| a_segment_runs_it(&crate::handoffguard::as_the_shell_reads(inner)))
        })
}

/// `(trace reason, denial)` for a Bash command that dispatches from inside a sub-agent, or
/// `None`.
pub fn refusal(cmd: &str, caller: Caller<'_>) -> Option<(&'static str, String)> {
    (caller.from_a_subagent() && runs_dispatch(cmd)).then(|| (REASON_SUBAGENT, helper_is_told()))
}

/// purlis's chat tools that dispatch, by the names [`crate::chattools`] gives them.
const OUR_TOOLS: [&str; 12] = [
    crate::chattools::DISPATCH,
    crate::chattools::DISPATCH_LIST,
    crate::chattools::DISPATCH_READ,
    crate::chattools::DISPATCH_WAIT,
    crate::chattools::DISPATCH_TELL,
    crate::chattools::DISPATCH_ANSWER,
    crate::chattools::DISPATCH_PASS_ON,
    crate::chattools::DISPATCH_CANCEL,
    crate::chattools::DISPATCH_CLOSE,
    crate::chattools::DISPATCH_REPORT,
    crate::chattools::DISPATCH_NOTE,
    crate::chattools::DISPATCH_ASK,
];

/// A chat tool of purlis's, as a harness names it.
#[cfg(test)]
fn tool_id(tool: &str) -> String {
    format!("{}{tool}", crate::names::MCP_TOOL_PREFIX.write)
}

/// The denial for a call of the chat tool `tool` from inside a sub-agent, or `None`: purlis's
/// `dispatch` and `dispatch_report`, under the name the app registers the server by. **The
/// same tools the hook's matcher names** ([`crate::hookreg`]), and a test holds the two lists
/// to each other: a tool this knew and the matcher did not would be a refusal nothing runs.
pub fn tool_refusal(tool: &str, caller: Caller<'_>) -> Option<String> {
    let name = tool.strip_prefix(crate::names::MCP_TOOL_PREFIX.write)?;
    (OUR_TOOLS.contains(&name) && caller.from_a_subagent()).then(helper_is_told)
}

/// What the decision answers a helper sub-agent, in its own sentence.
fn helper_is_told() -> String {
    // Who asks is the first thing the decision reads, so the rest is whatever a project that
    // set nothing would have.
    let limits = crate::dispatchlimits::in_force(
        &crate::dispatchlimits::Table::default(),
        None,
        None,
        None,
        &crate::dispatchlimits::Table::default(),
        &crate::dispatchlimits::Level::unset(),
    );
    let asked = Request {
        asker: Asker::Helper,
        to: Persona::None,
        mode: Mode::Task,
        grant: &crate::dispatchgrant::Covers::NeedsGrant,
        profile: None,
        limits: &limits,
        lineage: &dispatchdecision::Lineage::default(),
    };
    match dispatchdecision::decide(&asked) {
        Decision::Refused(why) => why.say(),
        // The decision refuses a helper before it asks anything else. Were that ever not so,
        // this guard still refuses, in the same words.
        Decision::Start | Decision::NeedsGrant { .. } => dispatchdecision::HELPER.to_owned(),
    }
}

// ----------------------------------------------------------------------------------------
// the text a dispatch subcommand sends, written as the shell will not rewrite it (#1456, #1463)
// ----------------------------------------------------------------------------------------
//
// A report, a follow-up, an answer, a note and a question each send their text as the command's
// own argument, quoted to the chat that reads it. A live substitution in the call is replaced by
// the shell before purlis reads a word, so what is sent is not what was written here, and the
// command it runs is the shell's. The handoff's report is refused for it (A7), and so are these.

/// The trace reason a dispatch's text with a live substitution in its call is refused under.
pub const REASON_TEXT_SOURCE: &str = "dispatch-text-source";

/// The subcommands of `purlis dispatch` that send text as it is written, each with the shape
/// its refusal tells the chat to write instead.
const SENDS_TEXT: [(&str, &str); 6] = [
    ("report", "report --outcome done \"<summary>\""),
    ("tell", "tell <chat> \"<text>\""),
    ("answer", "answer <chat> \"<answer>\""),
    ("pass-on", "pass-on <chat> --note \"<note>\""),
    ("note", "note \"<note>\""),
    ("ask", "ask \"<question>\""),
];

/// The text-sending subcommand a segment's words run, as its row of [`SENDS_TEXT`], or `None`.
fn sends_text(toks: &[String]) -> Option<(&'static str, &'static str)> {
    let (prog, _env, argv) = shellwrap::split_env(toks);
    let words = match charter_words(&prog, &argv) {
        Some(words) => words,
        None if names_our_binary(&prog) => argv.get(1..).unwrap_or_default().to_vec(),
        None => return None,
    };
    if words.first().is_none_or(|word| word != WORD) {
        return None;
    }
    let sub = words.get(1)?;
    SENDS_TEXT.into_iter().find(|(word, _)| word == sub)
}

/// A text-sending subcommand a call runs: its row of [`SENDS_TEXT`], and the string a shell
/// runs it in, where it is one level into one.
type TextSubcommand = (&'static str, &'static str, Option<String>);

/// The text-sending subcommand `cmd` runs in any segment, or one level into a string a shell
/// runs ([`shellwrap::shell_scripts`]), as [`runs_dispatch`] reads them.
fn a_text_subcommand(cmd: &str) -> Option<TextSubcommand> {
    let in_a_segment = |text: &str| {
        shellseg::segment_argv(text)
            .iter()
            .find_map(|toks| sends_text(toks))
    };
    if let Some((sub, shape)) = in_a_segment(cmd) {
        return Some((sub, shape, None));
    }
    let one_level_in = || {
        let toks = shellseg::lex(&crate::heredoc::strip_reader_heredocs(cmd)).ok()?;
        let toks = shellseg::split_punctuation(toks);
        crate::heredoc::segments_of(&toks)
            .iter()
            .find_map(|(seg, _before)| {
                let words: Vec<String> = seg.iter().map(|tok| tok.text.clone()).collect();
                shellwrap::shell_scripts(&words).iter().find_map(|inner| {
                    let read = crate::handoffguard::as_the_shell_reads(inner);
                    in_a_segment(&read).map(|(sub, shape)| (sub, shape, Some(read)))
                })
            })
    };
    // Then every level a shell is handed a text at (#1582), each with the text it stands in.
    // A text is read more than one way (as written, and as the shell that hands it on
    // unquotes it), so the one taken is one whose substitution is live, where any is.
    let further = || {
        let live_in_the_call = crate::livesub::live_substitution(cmd).is_some();
        let found = further_in(cmd, |text| {
            in_a_segment(text)
                .filter(|_| live_in_the_call || crate::livesub::live_substitution(text).is_some())
                .map(|(sub, shape)| (sub, shape, text.to_owned()))
        })?;
        match found {
            Nested::Found((sub, shape, text)) => Some((sub, shape, Some(text))),
            // Not read: the handoff guard refuses a call nested past the cap before this
            // guard is asked (A7 stands first in `toolgate`).
            Nested::TooDeep => None,
        }
    };
    one_level_in().or_else(further)
}

/// `(trace reason, denial)` for a call that runs `purlis dispatch report`, `tell`, `answer`,
/// `note` or `ask` and holds a live substitution anywhere, or `None`.
///
/// **Scoped to the whole call**, as A5, A6 and the handoff's report are, for their reason:
/// which argument a substitution lands in is more parser in the direction that fails open.
/// **On every harness and for the chat itself**, not only a helper: it is a fact about the
/// shell, and the text is the chat's own to write out. A quoted heredoc's body is data and is
/// not a substitution ([`crate::livesub::live_substitution`]).
pub fn text_refusal(cmd: &str) -> Option<(&'static str, String)> {
    if !crate::livesub::may_substitute(cmd) {
        return None;
    }
    let ((sub, shape), hit) = in_the_call(cmd).or_else(|| in_a_body_a_shell_runs(cmd))?;
    let (kind, effect) = if crate::livesub::is_process_substitution(hit) {
        ("process", "run and replace with a path")
    } else {
        ("command", "replace")
    };
    // A6's remedy, measured there: a report carries markdown code spans, and most carry an
    // apostrophe, so single quotes alone fail on exactly the text this refuses.
    let tool = if sub == "report" {
        format!(
            " Or send it with the `{}` tool, whose text no shell reads.",
            crate::chattools::DISPATCH_REPORT
        )
    } else {
        String::new()
    };
    Some((
        REASON_TEXT_SOURCE,
        format!(
            "`purlis dispatch {sub}` sends its text as it is written here, and this call has a \
             live {kind} substitution in it, which the shell would {effect} before purlis \
             reads it. Write the text out in plain words: purlis dispatch {shape}. Keep a \
             backtick literal by backslash-escaping each one (`\\`code\\``), which leaves \
             apostrophes working, or single-quote the whole text when it holds none.{tool}"
        ),
    ))
}

/// A text subcommand found in the call's segments, and a live substitution in the call as
/// written or in the string a shell runs it in: a substitution single-quoted on the outside is
/// live when that string is run.
fn in_the_call(cmd: &str) -> Option<((&'static str, &'static str), &'static str)> {
    let (sub, shape, inner) = a_text_subcommand(cmd)?;
    let hit = crate::livesub::live_substitution(cmd)
        .or_else(|| inner.as_deref().and_then(crate::livesub::live_substitution))?;
    Some(((sub, shape), hit))
}

/// A text subcommand in a heredoc body a shell runs (`bash <<'EOF'`), and a live substitution
/// in that body **as the shell reads it**: quoting the delimiter keeps the body from expanding
/// on its way in, and the shell then runs every line of it. Which bodies a shell runs is the
/// plan A7 and the leak guard share ([`crate::leakguard::lines_a_command_could_run`]).
fn in_a_body_a_shell_runs(cmd: &str) -> Option<((&'static str, &'static str), &'static str)> {
    let run: Vec<String> = crate::leakguard::lines_a_command_could_run(cmd)
        .into_iter()
        .filter(|(_, in_a_run_body)| *in_a_run_body)
        .map(|(row, _)| row)
        .collect();
    if run.is_empty() {
        return None;
    }
    let body = run.join("\n");
    let ((sub, shape), hit) = in_the_call(&body)?;
    Some(((sub, shape), hit))
}

// ----------------------------------------------------------------------------------------
// a rider beside a pre-allowed dispatch or handoff (#3785, superseding D-1444-14)
// ----------------------------------------------------------------------------------------
//
// A Claude Code chat the app starts is handed an `allow` for `purlis dispatch …` and
// `purlis handoff …`, by each spelling ([`crate::harness`]), because what consents to one is
// the dispatch grant. A harness matches such a rule against the command as written, and
// whether it then asks about a second command in the same call has not been measured. So
// purlis does not lean on it: where a call runs a dispatch or a handoff that writes **and
// anything else**, the hook refuses the call, names the other command and the shape to retry.
// It never answers `ask`: that stops the chat on a prompt in every mode, and the person is
// asked only where something new reaches someone new (the dispatch grant). A dispatch that
// only looks is left to the harness's own rules.

/// What a rider is called where it is a substitution: the shell runs a command there, and
/// which one is not this guard's to quote.
pub const A_SUBSTITUTION: &str = "a command substitution";

/// What a rider is called where purlis cannot read the call: a quote or an escape left open,
/// so where the dispatch ends cannot be told.
pub const UNREADABLE: &str = "one purlis could not read";

/// The trace key a refused rider is tallied under.
pub const REASON_RIDER: &str = "dispatch-rider";

/// `$CHARTER_HARNESS` in a Claude Code chat: the one harness handed the allow.
const CLAUDE_CODE: &str = "claude-code";

/// The two commands a Claude Code chat is handed an allow for.
const HANDOFF: &str = "handoff";

/// And a commit the app makes for a sandboxed chat in its branch folder (#1476), pre-allowed
/// by its spelling ([`crate::harness::claude::WORKTREE_COMMIT_ALLOW`]) on the same terms: the
/// two words of the command.
const WORKTREE_COMMIT: &str = "worktree commit";

/// And an update the app makes for a sandboxed chat in its clone or branch folder (#3831),
/// pre-allowed by its spelling on the same terms.
const WORKTREE_UPDATE: &str = "worktree update";

/// And a finished branch folder's removal the app makes for a chat (#3879), pre-allowed by its
/// spelling on the same terms.
const WORKTREE_REMOVE: &str = "worktree remove";

/// The command `own` names, for a sentence: `dispatch`, `handoff`, `commit` or `update`.
fn called(own: &str) -> &str {
    match own {
        WORKTREE_COMMIT => "commit",
        WORKTREE_UPDATE => "update",
        WORKTREE_REMOVE => "removal",
        _ => own,
    }
}

/// What purlis lets run without the harness asking, for `own`'s sentence.
fn let_run(own: &str) -> &'static str {
    match own {
        WORKTREE_COMMIT => "a commit the app makes for a chat",
        WORKTREE_UPDATE => "an update the app makes for a chat",
        WORKTREE_REMOVE => "a removal the app makes for a chat",
        _ => "a dispatch or a handoff",
    }
}

/// The hidden flag that pins the time a handoff's stamp, todo and dispatch row are written at,
/// for recorded tests (D-1471-3).
const NOW_FLAG: &str = "--now";

/// What a chat is told where a handoff pins its own clock.
const PINS_THE_CLOCK: &str = "this call runs `purlis handoff --now`, which pins the time its \
     stamp, todo and dispatch row are written at. That flag is for purlis's own tests, and \
     purlis lets a handoff run without your harness asking only without it, so this call is \
     refused, and nobody was asked. Run the handoff in a call of its own, without --now.";

/// Whether a handoff the call runs carries [`NOW_FLAG`] among its words (#1471): read on the
/// lines a shell would run, so a brief that mentions it is text.
fn pins_the_clock(cmd: &str) -> bool {
    if !cmd.contains(NOW_FLAG) {
        return false;
    }
    let runs: Vec<String> = crate::leakguard::lines_a_command_could_run(cmd)
        .into_iter()
        .map(|(row, _)| row)
        .collect();
    shellseg::segment_argv(&runs.join("\n")).iter().any(|toks| {
        ours(toks) == Some(HANDOFF)
            && toks
                .iter()
                .any(|w| w == NOW_FLAG || w.starts_with("--now="))
    })
}

/// Which of the commands handed an allow a segment's words run, if any: `dispatch`, `handoff`,
/// `worktree commit` or `worktree update`.
fn ours(toks: &[String]) -> Option<&'static str> {
    let (prog, _env, argv) = shellwrap::split_env(toks);
    let words: Vec<String> = match charter_words(&prog, &argv) {
        Some(words) => words.into_iter().take(2).collect(),
        None if names_our_binary(&prog) => argv.iter().skip(1).take(2).cloned().collect(),
        None => return None,
    };
    let first = words.first()?;
    if first == "worktree" {
        return match words.get(1).map(String::as_str) {
            Some("commit") => Some(WORKTREE_COMMIT),
            Some("update") => Some(WORKTREE_UPDATE),
            Some("remove") => Some(WORKTREE_REMOVE),
            _ => None,
        };
    }
    [WORD, HANDOFF].into_iter().find(|word| word == first)
}

/// `(which of the two the call runs, the other command beside it)`, or `None` for a call that
/// runs neither, or runs one alone.
///
/// Read with the readers the other guards share, and none of its own:
///
/// - **the lines a shell would run** ([`crate::leakguard::lines_a_command_could_run`]): a
///   brief, and any other heredoc body a reader takes, is data and is not a command here;
/// - **their segments** ([`shellseg::segment_argv`]): every command a `;`, `&&`, `||`, `|`, `&`
///   or a newline separates. The first that runs a dispatch or a handoff is the call's own, and
///   **any other segment is a rider**, a second dispatch or handoff among them: only the first
///   is judged by the handoff guard;
/// - **a live substitution anywhere in the call** ([`crate::livesub::live_substitution`]):
///   the shell runs a command there;
/// - **a call the lexer cannot read** that names either command is a rider too: where it
///   cannot tell, it asks.
///
/// A function or an alias defined earlier in the call is a segment of its own, so it is a
/// rider like any other command, and so is a body it runs (#1471).
///
/// **What it does not see**: a redirection on the dispatch's own line is not a second command
/// here, and is [`writes_beside`]'s; a dispatch inside a string a shell runs is not at the
/// start of the call, so no rule of the harness allows it.
fn beside(cmd: &str) -> Option<(&'static str, String)> {
    let runs: Vec<String> = crate::leakguard::lines_a_command_could_run(cmd)
        .into_iter()
        .map(|(row, _)| row)
        .collect();
    let text = runs.join("\n");
    if shellseg::lex(&text).is_err() {
        let lower = text.to_lowercase();
        return [
            WORD,
            HANDOFF,
            WORKTREE_COMMIT,
            WORKTREE_UPDATE,
            WORKTREE_REMOVE,
        ]
        .into_iter()
        .find(|word| {
            crate::cliname::INSTALLED
                .iter()
                .any(|name| lower.contains(&format!("{name} {word}")))
        })
        .map(|word| (word, UNREADABLE.to_owned()));
    }
    let segments = shellseg::segment_argv(&text);
    let (at, own) = segments
        .iter()
        .enumerate()
        .find_map(|(at, toks)| ours(toks).map(|own| (at, own)))?;
    if let Some((_, other)) = segments
        .iter()
        .enumerate()
        .find(|(i, toks)| *i != at && !toks.is_empty())
    {
        return Some((own, crate::shown::short(&other.join(" "))));
    }
    crate::livesub::live_substitution(cmd).map(|_| (own, A_SUBSTITUTION.to_owned()))
}

/// A segment of ours: which command it runs, its tokens, whether a pipe feeds it, and where
/// the command line's own words start after `dispatch` or `handoff`.
type OurSegment = (&'static str, Vec<Tok>, bool, Option<usize>);

/// The first segment of the lines a shell would run in `cmd` that runs a dispatch or a
/// handoff, as tokens: which of the two, the segment, whether a pipe feeds it, and where the
/// command line's own words start after `dispatch` or `handoff` (`None` where the reader's
/// words are not the segment's own tail, so where they start cannot be told).
fn own_segment(cmd: &str) -> Option<OurSegment> {
    our_segments(cmd)?.into_iter().next()
}

/// Every segment of the lines a shell would run in `cmd` that runs a dispatch, a handoff or a
/// commit, read as [`own_segment`] reads the first; `None` where the lexer cannot read them.
fn our_segments(cmd: &str) -> Option<Vec<OurSegment>> {
    let runs: Vec<String> = crate::leakguard::lines_a_command_could_run(cmd)
        .into_iter()
        .map(|(row, _)| row)
        .collect();
    let toks = shellseg::split_punctuation(shellseg::lex(&runs.join("\n")).ok()?);
    Some(
        crate::heredoc::segments_of(&toks)
            .into_iter()
            .filter_map(|(seg, before)| {
                let texts: Vec<String> = seg.iter().map(|t| t.text.clone()).collect();
                let own = ours(&texts)?;
                let (prog, _env, argv) = shellwrap::split_env(&texts);
                let words = charter_words(&prog, &argv).or_else(|| {
                    names_our_binary(&prog).then(|| argv.get(1..).unwrap_or_default().to_vec())
                })?;
                let after = texts
                    .len()
                    .checked_sub(words.len())
                    .filter(|at| texts[*at..] == words[..])
                    .map(|at| at + 1);
                Some((own, seg, before == "|" || before == "|&", after))
            })
            .collect(),
    )
}

/// Whether the word at `seg[i]` is a redirection's operator, its operand, or the descriptor
/// number in front of one: the shell's, never a word the command is handed.
fn skip_redirection(seg: &[Tok], i: usize) -> Option<usize> {
    if seg[i].is_op(&crate::handoffguard::REDIRECTIONS) {
        return Some(i + 2);
    }
    crate::handoffguard::names_a_descriptor(seg, i).then_some(i + 1)
}

/// The flags of `purlis dispatch` that take the next word as their value.
const DISPATCH_VALUE_FLAGS: [&str; 5] = ["--to", "--name", "--profile", "--timeout", "--in"];

/// Whether the dispatch whose words start at `seg[from]` reads a brief from stdin, as the
/// command parses them: no subcommand among its words, and no help asked for. A word that is
/// neither a flag nor a flag's value is a subcommand (an unknown one is refused before
/// anything is read).
fn reads_a_brief(seg: &[Tok], from: usize) -> bool {
    let mut i = from;
    while let Some(t) = seg.get(i) {
        if let Some(next) = skip_redirection(seg, i) {
            i = next;
            continue;
        }
        let word = t.text.as_str();
        if word == "--help" || word == "-h" || word == "--" || !word.starts_with('-') {
            return false;
        }
        i += if DISPATCH_VALUE_FLAGS.contains(&word) {
            2
        } else {
            1
        };
    }
    true
}

/// What feeds a dispatch's brief where it is not exactly one quoted heredoc on the
/// dispatch's own segment (#1471), as the handoff's brief is read
/// ([`crate::handoffguard::brief_source`]), or `None`: a dispatch that sends a subcommand or
/// asks for help reads no brief.
fn brief_fed_otherwise(cmd: &str) -> Option<&'static str> {
    let (own, seg, piped, after) = own_segment(cmd)?;
    if own != WORD {
        return None;
    }
    // Where the words start cannot be told, the brief is judged as if one were read.
    if after.is_some_and(|from| !reads_a_brief(&seg, from)) {
        return None;
    }
    crate::handoffguard::brief_source(cmd, &seg, piped)
}

/// A redirection on the dispatch's or the handoff's own segment that writes a file, as
/// written (`> notes.md`), or `None` (#1471). The shell opens that file beside the command,
/// so it is a second thing the call does. Not one: a heredoc or a here-string (the brief, or
/// data), a read (a brief fed by a file is [`brief_fed_otherwise`]'s), a descriptor
/// duplicated onto another (`2>&1`), and `/dev/null`.
fn writes_beside(cmd: &str) -> Option<(&'static str, String)> {
    let (own, seg, _piped, _after) = own_segment(cmd)?;
    let at = writes_a_file(&seg)?;
    let fd = if at > 0 && crate::handoffguard::names_a_descriptor(&seg, at - 1) {
        seg[at - 1].text.as_str()
    } else {
        ""
    };
    Some((
        own,
        crate::shown::short(&format!("{fd}{} {}", seg[at].text, seg[at + 1].text)),
    ))
}

/// Where on `seg` a redirection writes a file, as [`writes_beside`] counts one, or `None`.
fn writes_a_file(seg: &[Tok]) -> Option<usize> {
    const WRITES: [&str; 5] = [">", ">>", ">|", "<>", ">&"];
    seg.iter().enumerate().find_map(|(i, t)| {
        let operand = seg.get(i + 1)?;
        let writes = t.is_op(&WRITES)
            && operand.text != "/dev/null"
            && !(t.text == ">&"
                && (operand.text == "-" || operand.text.chars().all(|c| c.is_ascii_digit())));
        writes.then_some(i)
    })
}

/// The other command a tool call runs beside a dispatch or a handoff, as a person would
/// recognise it, or `None` ([`beside`]).
pub fn rider(cmd: &str) -> Option<String> {
    beside(cmd).map(|(_, other)| other)
}

/// Whether the call only looks (#3785): **every** dispatch, handoff and commit it runs is a
/// `purlis dispatch` that only looks ([`looks_at`]) with no redirection on its line that writes
/// a file, and none of them stands in a text a shell is handed to run, at any level. Such a
/// call is left to the harness's own rules beside any other command: it starts, sends and ends
/// nothing. A call whose words cannot be read is not one: that is the safe direction, a
/// refusal the chat retries rather than a pass.
fn only_looks(cmd: &str) -> bool {
    let Some(ours) = our_segments(cmd) else {
        return false;
    };
    !ours.is_empty()
        && ours.iter().all(|(own, seg, _piped, after)| {
            *own == WORD
                && after.is_some_and(|from| looks_at(seg, from))
                && writes_a_file(seg).is_none()
        })
        && !ours_in_a_text_a_shell_runs(cmd)
}

/// Whether the dispatch whose words start at `seg[from]` only looks: its subcommand is one of
/// [`LOOKS`], or it asks for help **in a flag position** before any subcommand, as the command
/// parses its words ([`reads_a_brief`]'s reading). A `-h` after the subcommand is that
/// subcommand's argument, the text a note or a follow-up sends, and is no help asked for.
fn looks_at(seg: &[Tok], from: usize) -> bool {
    let mut i = from;
    while let Some(t) = seg.get(i) {
        if let Some(next) = skip_redirection(seg, i) {
            i = next;
            continue;
        }
        let word = t.text.as_str();
        if word == "--help" || word == "-h" {
            return true;
        }
        if word == "--" {
            return false;
        }
        if !word.starts_with('-') {
            return LOOKS.contains(&word);
        }
        i += if DISPATCH_VALUE_FLAGS.contains(&word) {
            2
        } else {
            1
        };
    }
    false
}

/// Whether a dispatch, a handoff or a commit stands in any text a shell is handed to run in
/// `cmd`, at every level the other backstops read ([`consentspelling`]'s walk). Past the
/// nesting cap the text is not read, and counts as one.
fn ours_in_a_text_a_shell_runs(cmd: &str) -> bool {
    let read = crate::heredoc::Heredocs::of(cmd);
    consentspelling::in_shell_strings_at_every_level_by(&read, &|inner| {
        shellseg::segment_argv(&crate::handoffguard::as_the_shell_reads(&inner.text))
            .iter()
            .any(|toks| ours(toks).is_some())
            .then_some(())
    })
    .is_some()
}

/// The subcommands of `purlis dispatch` that only look (#3785).
const LOOKS: [&str; 3] = ["list", "read", "wait"];

/// **The refusal of a dispatch, a handoff or a commit that shares its call** (#3785,
/// superseding D-1444-14), as `(trace reason, denial)`, or `None`.
///
/// Nobody is asked: a hook's `ask` shows the person a prompt in every permission mode, auto
/// mode included, and a chat stopped on it stops its whole loop. So a call that only looks
/// ([`only_looks`]: every dispatch in it looks, and none writes a file by redirection) gets no
/// verdict here, whatever stands beside it, and the harness's own rules decide; and one that
/// writes is refused, naming the shape to retry, so the chat corrects itself with nobody
/// pressed.
///
/// Refused in this order: a handoff that pins its clock; another command beside it
/// ([`beside`]); a redirection on its own line that writes a file ([`writes_beside`]); and a
/// dispatch whose brief is fed from anything but one quoted heredoc on its own segment
/// ([`brief_fed_otherwise`], the arm a handoff refuses for, #1471).
///
/// Only for a Claude Code chat, the one harness handed the allow this stands behind: on
/// another the harness asks about every such call by its own defaults.
pub fn rider_refusal(cmd: &str, caller: Caller<'_>) -> Option<(&'static str, String)> {
    if caller.harness != Some(CLAUDE_CODE) || only_looks(cmd) {
        return None;
    }
    rider_said(cmd).map(|said| (REASON_RIDER, said))
}

/// What a chat is told where a removal it runs is forced (#3879).
const FORCES_A_REMOVAL: &str = "this call runs `purlis worktree remove --force`, which discards \
     the uncommitted changes and unique commits a plain removal names. That is the person's to \
     decide, and purlis lets a removal run without your harness asking only without --force, \
     so this call is refused, and nobody was asked. Run it without --force, and the app \
     removes the folder where nothing would be lost; otherwise ask the person to remove it \
     from the window.";

/// Whether a removal the call runs is forced (#3879): read on the lines a shell would run, as
/// [`pins_the_clock`] reads a handoff's, here in the hook, outside the chat's sandbox, so the
/// refusal does not rest on the chat's environment.
fn forces_a_removal(cmd: &str) -> bool {
    if !cmd.contains("-f") {
        return false;
    }
    let runs: Vec<String> = crate::leakguard::lines_a_command_could_run(cmd)
        .into_iter()
        .map(|(row, _)| row)
        .collect();
    shellseg::segment_argv(&runs.join("\n")).iter().any(|toks| {
        ours(toks) == Some(WORKTREE_REMOVE)
            && toks
                .iter()
                .any(|w| w == "--force" || w.starts_with("--force=") || w == "-f")
    })
}

/// What [`rider_refusal`] says, or `None`.
fn rider_said(cmd: &str) -> Option<String> {
    if pins_the_clock(cmd) {
        return Some(PINS_THE_CLOCK.to_owned());
    }
    if forces_a_removal(cmd) {
        return Some(FORCES_A_REMOVAL.to_owned());
    }
    if let Some((own, other)) = beside(cmd) {
        let (word, what) = (called(own), let_run(own));
        return Some(format!(
            "this call runs `purlis {own}` beside another command (`{other}`). purlis lets \
             {what} run without your harness asking only alone in its call, so this call is \
             refused, and nobody was asked. Run the {word} in a call of its own, and the other \
             command in another."
        ));
    }
    if let Some((own, redirection)) = writes_beside(cmd) {
        let (word, what) = (called(own), let_run(own));
        return Some(format!(
            "this call runs `purlis {own}` with a redirection that writes a file \
             (`{redirection}`). purlis lets {what} run without your harness asking only alone \
             in its call, so this call is refused, and nobody was asked. Run the {word} in a \
             call of its own, without the redirection."
        ));
    }
    let what = brief_fed_otherwise(cmd)?;
    Some(format!(
        "this call feeds `purlis dispatch` its brief from {what}. The new chat is sent \
         exactly the text written in the call only from a quoted heredoc in the same call, so \
         this call is refused, and nobody was asked. Run the dispatch in a call of its own, \
         with its brief in one: purlis dispatch --name \"<task>\" <<'BRIEF' … BRIEF, or use \
         the `{}` tool.",
        crate::chattools::DISPATCH
    ))
}

#[cfg(test)]
mod tests;
