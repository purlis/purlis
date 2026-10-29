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
const OUR_TOOLS: [&str; 4] = [
    crate::chattools::DISPATCH,
    crate::chattools::DISPATCH_LIST,
    crate::chattools::DISPATCH_READ,
    crate::chattools::DISPATCH_REPORT,
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
const SENDS_TEXT: [(&str, &str); 5] = [
    ("report", "report --outcome done \"<summary>\""),
    ("tell", "tell <chat> \"<text>\""),
    ("answer", "answer <chat> \"<answer>\""),
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

/// The command `own` names, for a sentence: `dispatch`, `handoff` or `commit`.
fn called(own: &str) -> &str {
    if own == WORKTREE_COMMIT {
        "commit"
    } else {
        own
    }
}

/// What purlis lets run without the harness asking, for `own`'s sentence.
fn let_run(own: &str) -> &'static str {
    if own == WORKTREE_COMMIT {
        "a commit the app makes for a chat"
    } else {
        "a dispatch or a handoff"
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

/// Which of the commands handed an allow a segment's words run, if any: `dispatch`, `handoff`
/// or `worktree commit`.
fn ours(toks: &[String]) -> Option<&'static str> {
    let (prog, _env, argv) = shellwrap::split_env(toks);
    let words: Vec<String> = match charter_words(&prog, &argv) {
        Some(words) => words.into_iter().take(2).collect(),
        None if names_our_binary(&prog) => argv.iter().skip(1).take(2).cloned().collect(),
        None => return None,
    };
    let first = words.first()?;
    if first == "worktree" {
        return (words.get(1).map(String::as_str) == Some("commit")).then_some(WORKTREE_COMMIT);
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
        return [WORD, HANDOFF, WORKTREE_COMMIT]
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

/// The first segment of the lines a shell would run in `cmd` that runs a dispatch or a
/// handoff, as tokens: which of the two, the segment, whether a pipe feeds it, and where the
/// command line's own words start after `dispatch` or `handoff` (`None` where the reader's
/// words are not the segment's own tail, so where they start cannot be told).
fn own_segment(cmd: &str) -> Option<(&'static str, Vec<Tok>, bool, Option<usize>)> {
    let runs: Vec<String> = crate::leakguard::lines_a_command_could_run(cmd)
        .into_iter()
        .map(|(row, _)| row)
        .collect();
    let toks = shellseg::split_punctuation(shellseg::lex(&runs.join("\n")).ok()?);
    crate::heredoc::segments_of(&toks)
        .into_iter()
        .find_map(|(seg, before)| {
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
    const WRITES: [&str; 5] = [">", ">>", ">|", "<>", ">&"];
    let at = seg.iter().enumerate().find_map(|(i, t)| {
        let operand = seg.get(i + 1)?;
        let writes = t.is_op(&WRITES)
            && operand.text != "/dev/null"
            && !(t.text == ">&"
                && (operand.text == "-" || operand.text.chars().all(|c| c.is_ascii_digit())));
        writes.then_some(i)
    })?;
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

/// The other command a tool call runs beside a dispatch or a handoff, as a person would
/// recognise it, or `None` ([`beside`]).
pub fn rider(cmd: &str) -> Option<String> {
    beside(cmd).map(|(_, other)| other)
}

/// Whether the call's own dispatch only looks (#3785): `list`, `read`, `wait`, or a help ask
/// of any subcommand. Such a call is left to the harness's own rules beside any other command:
/// it starts, sends and ends nothing. A dispatch that starts a task (`--wait` among its flags
/// too), every subcommand that sends or ends something, a handoff and a commit are not, and
/// neither is a call whose words cannot be told: that is the safe direction, a refusal the
/// chat retries rather than a pass.
fn only_looks(cmd: &str) -> bool {
    let Some((own, seg, _piped, Some(from))) = own_segment(cmd) else {
        return false;
    };
    if own != WORD {
        return false;
    }
    let mut i = from;
    let mut sub: Option<&str> = None;
    while let Some(t) = seg.get(i) {
        if let Some(next) = skip_redirection(&seg, i) {
            i = next;
            continue;
        }
        let word = t.text.as_str();
        if word == "--help" || word == "-h" {
            return true;
        }
        if word == "--" {
            break;
        }
        if sub.is_none() && !word.starts_with('-') {
            sub = Some(word);
        }
        i += if sub.is_none() && DISPATCH_VALUE_FLAGS.contains(&word) {
            2
        } else {
            1
        };
    }
    sub.is_some_and(|sub| LOOKS.contains(&sub))
}

/// The subcommands of `purlis dispatch` that only look (#3785).
const LOOKS: [&str; 3] = ["list", "read", "wait"];

/// **The refusal of a dispatch, a handoff or a commit that shares its call** (#3785,
/// superseding D-1444-14), as `(trace reason, denial)`, or `None`.
///
/// Nobody is asked: a hook's `ask` shows the person a prompt in every permission mode, auto
/// mode included, and a chat stopped on it stops its whole loop. So a call whose dispatch only
/// looks ([`only_looks`]) gets no verdict here, whatever stands beside it, and the harness's
/// own rules decide; and one that writes is refused, naming the shape to retry, so the chat
/// corrects itself with nobody pressed.
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

/// What [`rider_refusal`] says, or `None`.
fn rider_said(cmd: &str) -> Option<String> {
    if pins_the_clock(cmd) {
        return Some(PINS_THE_CLOCK.to_owned());
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
mod tests {
    use super::*;

    fn a_sub_agent() -> Caller<'static> {
        Caller {
            agent_id: Some("sub-1"),
            harness: Some("claude-code"),
            permission_mode: None,
        }
    }

    fn the_chat() -> Caller<'static> {
        Caller {
            agent_id: None,
            harness: Some("claude-code"),
            permission_mode: None,
        }
    }

    /// The sentence a rider is refused with, or `None`.
    fn refused(cmd: &str, caller: Caller<'_>) -> Option<String> {
        rider_refusal(cmd, caller).map(|(reason, said)| {
            assert_eq!(reason, REASON_RIDER);
            said
        })
    }

    #[test]
    fn every_tool_this_refuses_is_one_the_hook_s_matcher_names_and_the_other_way_round() {
        // Two lists that have to agree: the tools `tool_refusal` knows, and the tools the
        // harness runs `pretooluse-dispatch` in front of.
        let matcher = crate::hookreg::find("pretooluse-dispatch")
            .and_then(|hook| hook.matcher)
            .expect("a matcher");
        let named: Vec<&str> = matcher
            .split('|')
            .filter(|tool| tool.starts_with("mcp__"))
            .collect();
        assert_eq!(named, OUR_TOOLS.map(tool_id));
        for tool in named {
            assert!(tool_refusal(tool, a_sub_agent()).is_some(), "{tool}");
        }
    }

    const DISPATCH: &str = "purlis dispatch --name \"check the queue\" <<'BRIEF'\nlook\nBRIEF";

    #[test]
    fn a_sub_agent_that_dispatches_is_refused_and_told_to_return_to_its_chat() {
        let (reason, said) = refusal(DISPATCH, a_sub_agent()).expect("refused");
        assert_eq!(reason, REASON_SUBAGENT);
        assert_eq!(
            said,
            "a dispatch is refused from inside a helper. A task belongs to a chat the person \
             can see, open and stop, and a helper is not one. Return what you \
             found to your chat, and let that chat dispatch."
        );
    }

    #[test]
    fn the_chat_itself_dispatches_past_this_guard() {
        assert_eq!(refusal(DISPATCH, the_chat()), None);
        // An empty `agent_id` is the main conversation on a harness that always sends one.
        let empty = Caller {
            agent_id: Some(""),
            ..the_chat()
        };
        assert_eq!(refusal(DISPATCH, empty), None);
    }

    #[test]
    fn a_harness_whose_agent_id_was_never_measured_is_not_read_as_a_sub_agent() {
        let unmeasured = Caller {
            harness: Some("opencode"),
            ..a_sub_agent()
        };
        assert_eq!(refusal(DISPATCH, unmeasured), None);
    }

    #[test]
    fn a_sub_agent_is_refused_in_every_spelling_of_the_command_line_and_a_report_too() {
        for cmd in [
            "purlis dispatch --to devops --name x <<'B'\nb\nB",
            "charter dispatch --name x <<'B'\nb\nB",
            "cd svc && purlis dispatch --name x <<'B'\nb\nB",
            "FOO=1 purlis dispatch --name x",
            "purlis dispatch report --outcome done \"did it\"",
            "ls\npurlis dispatch --name x",
        ] {
            assert!(refusal(cmd, a_sub_agent()).is_some(), "{cmd:?}");
        }
    }

    #[test]
    fn a_dispatch_one_level_inside_a_string_a_shell_runs_is_seen() {
        // What the readers the other guards share already parse: a `-c` string, an `eval`.
        for cmd in [
            "bash -c 'purlis dispatch --name x'",
            "sh -c \"purlis dispatch --name x\"",
            "eval 'purlis dispatch --name x'",
            "eval purlis dispatch --name x",
            "cd svc && bash -c 'cd x; purlis dispatch report --outcome done ok'",
        ] {
            assert!(refusal(cmd, a_sub_agent()).is_some(), "{cmd:?}");
            assert_eq!(refusal(cmd, the_chat()), None, "{cmd:?}");
        }
    }

    /// #1582: a here-string, a pipe into a shell, a `-c` inside a `-c`, and the rest of what a
    /// shell is handed, at every level, through the walk the other backstops share.
    #[test]
    fn a_dispatch_in_any_text_a_shell_is_handed_is_seen_at_every_level() {
        for cmd in [
            "bash <<< 'purlis dispatch --name x'",
            "sh -s <<< \"purlis dispatch note hi\"",
            "echo 'purlis dispatch --name x' | sh",
            "printf '%s\\n' 'purlis dispatch --name x' | bash",
            "bash -c \"sh -c 'purlis dispatch --name x'\"",
            "eval \"bash -c 'purlis dispatch --name x'\"",
            "bash -c \"bash <<< 'purlis dispatch --name x'\"",
            "bash <<'EOF'\nbash -c \"purlis dispatch --name x\"\nEOF",
            "find . -maxdepth 0 -exec sh -c 'purlis dispatch --name x' \\;",
            "bash <(echo 'purlis dispatch --name x')",
        ] {
            assert!(refusal(cmd, a_sub_agent()).is_some(), "{cmd:?}");
            assert_eq!(refusal(cmd, the_chat()), None, "{cmd:?}");
        }
        // Nested past the cap, a helper is refused unread.
        let deep = (0..=crate::guardcaps::MAX_NESTING).fold("ls".to_owned(), |inner, _| {
            format!("bash -c '{}'", inner.replace('\'', "'\\''"))
        });
        let deep = deep.replacen("ls", "echo dispatch", 1);
        assert!(refusal(&deep, a_sub_agent()).is_some(), "{deep:?}");
        // Text that only names a dispatch, at any level, is not one.
        for cmd in [
            "bash <<< 'echo purlis dispatch'",
            "echo 'purlis dispatch --name x' | cat",
            "bash -c \"sh -c 'echo purlis dispatch'\"",
            "cat <<< 'purlis dispatch --name x'",
        ] {
            assert_eq!(refusal(cmd, a_sub_agent()), None, "{cmd:?}");
        }
    }

    /// A7e reads a text subcommand at every level a shell is handed a text at, with the live
    /// substitution in the text that level receives (#1582).
    #[test]
    fn a_dispatch_text_with_a_live_substitution_is_refused_at_every_level() {
        for cmd in [
            "bash <<< 'purlis dispatch note \"$(date)\"'",
            "echo 'purlis dispatch note \"$(date)\"' | sh",
            "bash -c \"sh -c 'purlis dispatch note \\\"\\$(date)\\\"'\"",
            "bash -c \"bash <<< 'purlis dispatch ask \\`whoami\\`'\"",
        ] {
            let (reason, _) = text_refusal(cmd).unwrap_or_else(|| panic!("{cmd:?}"));
            assert_eq!(reason, REASON_TEXT_SOURCE, "{cmd:?}");
        }
        for cmd in [
            "bash <<< 'purlis dispatch note plain'",
            "bash -c \"sh -c 'purlis dispatch note plain'\"",
            "echo \"$(date)\"; bash <<< 'echo purlis dispatch note'",
        ] {
            assert_eq!(text_refusal(cmd), None, "{cmd:?}");
        }
    }

    #[test]
    fn a_dispatch_run_by_the_variable_that_names_purlis_s_own_binary_is_seen() {
        // Every chat the app starts has it in its environment, under either name.
        for cmd in [
            "$PURLIS_HOOK_BINARY dispatch --name x",
            "\"$PURLIS_HOOK_BINARY\" dispatch --name x",
            "${PURLIS_HOOK_BINARY} dispatch --name x",
            "\"${CHARTER_HOOK_BINARY}\" dispatch report --outcome done ok",
            "bash -c '$CHARTER_HOOK_BINARY dispatch --name x'",
        ] {
            assert!(refusal(cmd, a_sub_agent()).is_some(), "{cmd:?}");
        }
        for cmd in [
            "$PURLIS_HOOK_BINARY hook stop",
            "$PURLIS_HOOK_BINARY workspace todo dispatch",
            "$OTHER dispatch --name x",
            "bash -c 'echo dispatch'",
            "eval 'git log --grep dispatch'",
        ] {
            assert_eq!(refusal(cmd, a_sub_agent()), None, "{cmd:?}");
        }
    }

    #[test]
    fn a_sub_agent_running_anything_else_is_left_alone() {
        for cmd in [
            "purlis workspace todo \"dispatch the fix\"",
            "echo purlis dispatch",
            "git log --grep dispatch",
            "purlis persona stats",
            "dispatch --name x",
        ] {
            assert_eq!(refusal(cmd, a_sub_agent()), None, "{cmd:?}");
        }
    }

    #[test]
    fn the_pre_allowed_list_of_tasks_is_still_refused_to_a_helper_and_starts_nothing() {
        // #1463: `dispatch_list` runs without the harness asking, as `purlis dispatch list`
        // does. The pre-allow is the harness's prompt and no more: a helper's call is still
        // refused in front of it, and the tools that start a chat or send a report are not
        // among the reads.
        for read in [
            crate::chattools::DISPATCH_LIST,
            crate::chattools::DISPATCH_READ,
        ] {
            let tool = format!("mcp__purlis__{read}");
            assert!(crate::chattools::PRE_ALLOWED.contains(&read), "{read}");
            assert!(tool_refusal(&tool, a_sub_agent()).is_some(), "{read}");
            assert_eq!(tool_refusal(&tool, the_chat()), None, "{read}");
        }
        for writes in [
            crate::chattools::DISPATCH,
            crate::chattools::DISPATCH_REPORT,
        ] {
            assert!(!crate::chattools::PRE_ALLOWED.contains(&writes), "{writes}");
        }
    }

    #[test]
    fn a_sub_agent_calling_the_dispatch_tools_is_refused_under_either_server_name() {
        for tool in ["mcp__purlis__dispatch", "mcp__purlis__dispatch_report"] {
            let said = tool_refusal(tool, a_sub_agent()).unwrap_or_else(|| panic!("{tool}"));
            assert!(
                said.contains("Return what you found to your chat"),
                "{said}"
            );
            assert_eq!(tool_refusal(tool, the_chat()), None, "{tool}");
        }
        // Another tool of purlis's, another server's `dispatch`, and a harness's own tool.
        for tool in [
            "mcp__purlis__todo_add",
            // The server's name before the rename: the hook's matcher never names it, so the
            // hook is never run for it, and this does not pretend to cover it.
            "mcp__charter__dispatch",
            "mcp__other__dispatch",
            "mcp__purlis__dispatcher",
            "Task",
            "dispatch",
        ] {
            assert_eq!(tool_refusal(tool, a_sub_agent()), None, "{tool}");
        }
    }

    // ----- a rider beside a pre-allowed dispatch or handoff (D-1444-14) -----

    const TASK: &str = "purlis dispatch --name \"check the queue\" <<'BRIEF'\nlook\nBRIEF";
    const MOVED: &str = "purlis handoff --name \"ship it\" beta <<'BRIEF'\nlook\nBRIEF";

    /// #3785, superseding D-1444-14: nobody is asked about a rider. A dispatch subcommand that
    /// only looks is left to the harness's own rules beside any other command; one that writes
    /// is refused with the shape to retry, so the chat corrects itself with nobody pressed.
    #[test]
    fn a_look_beside_another_command_gets_no_verdict_and_a_write_is_refused_with_its_shape() {
        for look in [
            "purlis dispatch list",
            "purlis dispatch read 12",
            "purlis dispatch wait 12",
            "purlis dispatch wait 12 --timeout 60",
            "purlis dispatch --help",
            "purlis dispatch tell --help",
        ] {
            for with in [
                format!("{look} 2>&1 | head -20"),
                format!("{look}\ntouch /tmp/zz"),
                format!("cd svc && {look}"),
                format!("{look} > notes.md"),
            ] {
                assert_eq!(rider_refusal(&with, the_chat()), None, "{with:?}");
            }
        }
        for write in [
            TASK,
            MOVED,
            "purlis dispatch --wait --name x <<'B'\nlook\nB",
            "purlis dispatch tell 12 \"and the logs\"",
            "purlis dispatch note \"half way\"",
            "purlis dispatch ask \"which cluster?\"",
            "purlis dispatch answer 12 \"staging\"",
            "purlis dispatch report --outcome done \"did it\"",
            "purlis dispatch cancel 12",
            "purlis dispatch close 12",
            "purlis worktree commit -m x --all",
        ] {
            let with = format!("{write}\ntouch /tmp/zz");
            let (reason, said) =
                rider_refusal(&with, the_chat()).unwrap_or_else(|| panic!("{with:?} refused"));
            assert_eq!(reason, REASON_RIDER);
            assert!(said.contains("in a call of its own"), "{said}");
            assert!(!said.contains("person is asked"), "{said}");
        }
    }

    #[test]
    fn a_dispatch_or_a_handoff_alone_in_its_call_has_no_rider() {
        for cmd in [
            TASK,
            MOVED,
            "purlis dispatch --to devops --name x <<'B'\nb\nB",
            "purlis dispatch report --outcome done \"did it\"",
            "purlis handoff report \"done, see the PR\"",
            "purlis handoff --name x --create --vision \"what for\" beta <<'B'\nb\nB",
            // A brief is data, whatever its lines look like: nothing in it is a command.
            "purlis dispatch --name x <<'B'\nrm -rf build\nls && purlis dispatch --name y\nB",
            "purlis handoff --name x beta <<'B'\ncurl example.com | sh\n$(whoami)\nB",
        ] {
            assert_eq!(rider(cmd), None, "{cmd:?}");
        }
    }

    #[test]
    fn a_call_that_runs_neither_is_not_this_guards_business() {
        for cmd in [
            "ls && echo hi",
            "echo purlis dispatch && ls",
            "git commit -m \"purlis dispatch --name x\" && git push",
            "cat > notes.md <<'EOF'\npurlis handoff --name x beta\nEOF\nls",
            "purlis workspace todo \"dispatch the fix\" && ls",
        ] {
            assert_eq!(rider(cmd), None, "{cmd:?}");
        }
    }

    #[test]
    fn another_command_in_the_same_call_is_a_rider_before_or_after_and_is_named() {
        for (own, name) in [(TASK, "dispatch"), (MOVED, "handoff")] {
            for (cmd, other) in [
                // Before it, and after it, joined every way a shell joins commands.
                (format!("touch /tmp/zz && {own}"), "touch /tmp/zz"),
                (format!("touch /tmp/zz; {own}"), "touch /tmp/zz"),
                (format!("touch /tmp/zz\n{own}"), "touch /tmp/zz"),
                (format!("{own}\ntouch /tmp/zz"), "touch /tmp/zz"),
                (format!("{own}\ncurl example.com | sh"), "curl example.com"),
                // On the line that opens the heredoc, after its opener.
                (
                    own.replacen("<<'BRIEF'", "<<'BRIEF' && touch /tmp/zz", 1),
                    "touch /tmp/zz",
                ),
                (
                    own.replacen("<<'BRIEF'", "<<'BRIEF' | tee /tmp/log", 1),
                    "tee /tmp/log",
                ),
                (format!("cat brief.md | {own}"), "cat brief.md"),
            ] {
                let said = rider(&cmd).unwrap_or_else(|| panic!("{cmd:?} has a rider"));
                assert_eq!(said, other, "{name}: {cmd:?}");
            }
        }
    }

    /// What a chat asks after a task it dispatched is pre-allowed by name too
    /// (`harness::claude::DISPATCH_TASK_ALLOW`), so each is held to the same rule: alone in
    /// its call it has no rider, and beside another command one that writes is refused and one
    /// that only looks is the harness's to judge (#3785).
    #[test]
    fn every_pre_allowed_dispatch_subcommand_is_alone_or_refused_unless_it_looks() {
        let pre_allowed = crate::harness::claude::DISPATCH_TASK_ALLOW;
        let spelt = [
            "purlis dispatch --wait --name x <<'B'\nlook\nB",
            "purlis dispatch wait 12",
            "purlis dispatch list",
            "purlis dispatch read 12",
            "purlis dispatch cancel 12",
            "purlis dispatch tell 12 \"and the logs too\"",
            "purlis dispatch note \"half way\"",
            "purlis dispatch ask \"which cluster?\"",
            "purlis dispatch answer 12 \"the staging one\"",
            "purlis dispatch close 12",
        ];
        // One spelling here for each rule the chat is handed, so a rule added there is met here.
        assert_eq!(spelt.len(), pre_allowed.len());
        for (cmd, rule) in spelt.iter().zip(pre_allowed) {
            let glob = rule
                .strip_prefix("Bash(")
                .and_then(|rule| rule.strip_suffix(')'))
                .expect("a Bash rule");
            assert!(
                crate::pypath::fnmatch(cmd, glob),
                "{cmd:?} is not the spelling {rule} allows"
            );
            assert_eq!(rider(cmd), None, "{cmd:?}");
            for with in [
                format!("{cmd}\ntouch /tmp/zz"),
                format!("touch /tmp/zz && {cmd}"),
            ] {
                assert_eq!(rider(&with).as_deref(), Some("touch /tmp/zz"), "{with:?}");
                let looks = ["wait", "list", "read"]
                    .iter()
                    .any(|sub| cmd.starts_with(&format!("purlis dispatch {sub}")));
                assert_eq!(refused(&with, the_chat()).is_none(), looks, "{with:?}");
            }
        }
    }

    /// A dispatch may say where its chat works (`--in`, #1453). The word is one more argument
    /// of a line a rule already allows, so the rule for a shared call sees it wherever it
    /// stands (D-T61-3); and no rule allows a line that opens with it, so that spelling is the
    /// harness's own to ask about, alone or not.
    #[test]
    fn a_dispatch_that_says_where_it_works_is_alone_or_refused_too() {
        use crate::harness::claude::{DISPATCH_ALLOW, DISPATCH_TASK_ALLOW, HANDOFF_ALLOW};
        let allowed = |cmd: &str| {
            DISPATCH_ALLOW
                .iter()
                .chain(DISPATCH_TASK_ALLOW.iter())
                .chain(HANDOFF_ALLOW.iter())
                .any(|rule| {
                    let glob = rule
                        .strip_prefix("Bash(")
                        .and_then(|rule| rule.strip_suffix(')'))
                        .expect("a Bash rule");
                    crate::pypath::fnmatch(cmd, glob)
                })
        };
        for (cmd, pre_allowed) in [
            (
                "purlis dispatch --name x --in worktree <<'B'\nlook\nB",
                true,
            ),
            (
                "purlis dispatch --name x --in workspace:beta <<'B'\nlook\nB",
                true,
            ),
            (
                "purlis dispatch --to devops --in worktree --name x <<'B'\nlook\nB",
                true,
            ),
            (
                "purlis dispatch --wait --in workspace:beta --name x <<'B'\nlook\nB",
                true,
            ),
            // No rule opens with `--in`: these the harness asks about itself.
            (
                "purlis dispatch --in worktree --name x <<'B'\nlook\nB",
                false,
            ),
            (
                "purlis dispatch --in workspace:beta --name x <<'B'\nlook\nB",
                false,
            ),
            (
                "purlis dispatch --in=worktree --name x <<'B'\nlook\nB",
                false,
            ),
        ] {
            assert_eq!(allowed(cmd), pre_allowed, "{cmd:?}");
            assert_eq!(rider(cmd), None, "{cmd:?}");
            for with in [
                format!("{cmd}\ntouch /tmp/zz"),
                format!("touch /tmp/zz && {cmd}"),
                cmd.replacen("<<'B'", "<<'B' && touch /tmp/zz", 1),
            ] {
                assert_eq!(rider(&with).as_deref(), Some("touch /tmp/zz"), "{with:?}");
                assert!(refused(&with, the_chat()).is_some(), "{with:?}");
            }
        }
    }

    #[test]
    fn a_second_dispatch_or_handoff_in_the_call_is_a_rider_too() {
        // Only the first handoff in a call is judged by the handoff guard, so a second one is
        // another command like any other.
        for cmd in [format!("{TASK}\n{MOVED}"), format!("{MOVED}\n{TASK}")] {
            assert!(rider(&cmd).is_some(), "{cmd:?}");
        }
    }

    #[test]
    fn a_substitution_beside_it_is_a_rider_and_so_is_a_call_purlis_cannot_read() {
        for cmd in [
            "purlis dispatch --name \"$(whoami)\" <<'B'\nb\nB",
            "purlis dispatch report --outcome done \"$(cat notes.md)\"",
        ] {
            assert_eq!(rider(cmd).as_deref(), Some(A_SUBSTITUTION), "{cmd:?}");
        }
        // One the shell reader unpicks is named as the command it is.
        assert_eq!(
            rider("purlis dispatch --name x --to `cat who` <<'B'\nb\nB").as_deref(),
            Some("cat who")
        );
        assert_eq!(
            rider("purlis dispatch --name x <(cat brief.md)").as_deref(),
            Some("cat brief.md")
        );
        // A quote left open: where the command ends cannot be read, so it is a rider.
        for cmd in [
            "purlis dispatch --name 'x",
            "purlis handoff --name \"x beta",
        ] {
            assert_eq!(rider(cmd).as_deref(), Some(UNREADABLE), "{cmd:?}");
        }
    }

    #[test]
    fn a_live_substitution_in_the_text_a_dispatch_subcommand_sends_is_refused() {
        // #1456, #1463: as the handoff's report is. Whoever asks, on every harness.
        for (cmd, sub) in [
            (
                "purlis dispatch report --outcome done \"$(cat notes.md)\"",
                "report",
            ),
            (
                "purlis dispatch report --outcome done \"`git log -1`\"",
                "report",
            ),
            ("purlis dispatch tell 9 \"use $(whoami)\"", "tell"),
            ("purlis dispatch answer 9 \"$(env)\"", "answer"),
            ("purlis dispatch note \"at $(date)\"", "note"),
            ("purlis dispatch ask \"which one? $(ls)\"", "ask"),
            ("charter dispatch note \"$(id)\"", "note"),
            ("cd svc && purlis dispatch tell 3 \"$(cat x)\"", "tell"),
            ("$PURLIS_HOOK_BINARY dispatch note \"$(id)\"", "note"),
            ("bash -c 'purlis dispatch note \"$(id)\"'", "note"),
            (
                "purlis dispatch report --outcome done \"$(cat <<'EOF'\nfixed it\nEOF\n)\"",
                "report",
            ),
            // A body a shell runs: its quoted delimiter keeps it from expanding on the way
            // in, and the shell then runs the line, substitution and all.
            ("bash <<'EOF'\npurlis dispatch note \"$(id)\"\nEOF", "note"),
            (
                "sh <<'EOF'\ncd svc\npurlis dispatch tell 9 \"`id`\"\nEOF",
                "tell",
            ),
        ] {
            let (reason, said) = text_refusal(cmd).unwrap_or_else(|| panic!("{cmd:?}"));
            assert_eq!(reason, REASON_TEXT_SOURCE);
            assert!(
                said.starts_with(&format!("`purlis dispatch {sub}` sends its text as it is")),
                "{said}"
            );
        }
        assert_eq!(
            text_refusal("purlis dispatch note \"$(id)\"").map(|(_, said)| said),
            Some(
                "`purlis dispatch note` sends its text as it is written here, and this call \
                 has a live command substitution in it, which the shell would replace before \
                 purlis reads it. Write the text out in plain words: purlis dispatch note \
                 \"<note>\". Keep a backtick literal by backslash-escaping each one \
                 (`\\`code\\``), which leaves apostrophes working, or single-quote the whole \
                 text when it holds none."
                    .to_owned()
            )
        );
        // A report can also go by the tool, which no shell reads.
        let (_, said) =
            text_refusal("purlis dispatch report --outcome done \"$(id)\"").expect("refused");
        assert!(
            said.ends_with(
                "Or send it with the `dispatch_report` tool, whose text no shell reads."
            ),
            "{said}"
        );
        let (_, said) = text_refusal("purlis dispatch tell 9 \"$(cat <(id))\"").expect("refused");
        assert!(said.contains("live command substitution"), "{said}");
        let (_, said) = text_refusal("purlis dispatch tell 9 <(id)").expect("refused");
        assert!(
            said.contains(
                "live process substitution in it, which the shell would run and \
                           replace with a path"
            ),
            "{said}"
        );
    }

    #[test]
    fn text_written_out_and_everything_but_a_text_subcommand_pass_this_refusal() {
        for cmd in [
            // The text as written: single quotes, an escaped dollar, a parameter.
            "purlis dispatch report --outcome done 'cost $(nothing)'",
            "purlis dispatch note \"a \\$(literal) one\"",
            "purlis dispatch tell 9 \"$NAME is set\"",
            "purlis dispatch answer 9 \"the blue one\"",
            // A body a reader takes is data, whatever it mentions.
            "cat > notes.md <<'EOF'\npurlis dispatch note \"$(id)\"\nEOF",
            // A dispatch's brief is its heredoc, judged as a brief, not here.
            "purlis dispatch --name x <<'B'\nrun $(this) later\nB",
            // Subcommands that send no text.
            "purlis dispatch wait 9 && echo \"$(date)\"",
            "purlis dispatch list; echo $(date)",
            "purlis dispatch cancel 9 \"$(id)\"",
            // Not purlis's dispatch, or not a dispatch at all.
            "echo purlis dispatch note \"$(id)\"",
            "purlis workspace note \"dispatch $(id)\"",
            "git commit -m \"$(cat msg)\"",
        ] {
            assert_eq!(text_refusal(cmd), None, "{cmd:?}");
        }
    }

    #[test]
    fn the_refusal_names_the_other_command_and_is_only_claude_code_s() {
        let cmd = format!("{TASK}\ntouch /tmp/zz");
        let said = refused(&cmd, the_chat()).expect("refused");
        assert_eq!(
            said,
            "this call runs `purlis dispatch` beside another command (`touch /tmp/zz`). purlis \
             lets a dispatch or a handoff run without your harness asking only alone in its \
             call, so this call is refused, and nobody was asked. Run the dispatch in a call of \
             its own, and the other command in another."
        );
        let said = refused(&format!("ls -la && {MOVED}"), the_chat()).expect("refused");
        assert!(
            said.starts_with("this call runs `purlis handoff` beside another command (`ls -la`)."),
            "{said}"
        );
        // The allow this stands behind is one only a Claude Code chat is handed.
        for harness in [Some("codex"), Some("opencode"), None] {
            let other = Caller {
                harness,
                ..the_chat()
            };
            assert_eq!(refused(&cmd, other), None, "{harness:?}");
        }
        assert_eq!(refused(TASK, the_chat()), None);
        assert_eq!(refused(MOVED, the_chat()), None);
    }

    #[test]
    fn a_brokered_commit_is_refused_where_it_shares_its_call() {
        // #1476: `purlis worktree commit` is pre-allowed by its spelling, on the dispatch's
        // terms: alone, and with nothing written beside it.
        let commit = "purlis worktree commit -m 'one change' --all";
        assert_eq!(refused(commit, the_chat()), None);
        assert_eq!(
            refused("purlis worktree commit -m x src/a.rs", the_chat()),
            None
        );
        let said = refused(&format!("{commit} && git push"), the_chat()).expect("refused");
        assert_eq!(
            said,
            "this call runs `purlis worktree commit` beside another command (`git push`). \
             purlis lets a commit the app makes for a chat run without your harness asking only \
             alone in its call, so this call is refused, and nobody was asked. Run the commit \
             in a call of its own, and the other command in another."
        );
        let said = refused(&format!("{commit} > out.txt"), the_chat()).expect("refused");
        assert!(said.contains("(`> out.txt`)"), "{said}");
        assert!(
            said.ends_with("Run the commit in a call of its own, without the redirection."),
            "{said}"
        );
        // A message from a command substitution is a command the shell runs.
        let said =
            refused("purlis worktree commit -m \"$(cat msg)\" --all", the_chat()).expect("refused");
        assert!(said.contains(A_SUBSTITUTION), "{said}");
        // Other `worktree` words are not handed the allow, so nothing is refused of them here.
        assert_eq!(
            refused("purlis worktree add thing piece && ls", the_chat()),
            None
        );
    }

    /// The hidden flag that pins a handoff's clock is for recorded tests, not for a chat: a
    /// handoff line that carries it is refused, though the allow covers the rest (#1471).
    #[test]
    fn a_handoff_that_pins_its_clock_is_refused() {
        for cmd in [
            "purlis handoff --now 2026-01-01T00:00:00 --name x beta <<'B'\nb\nB",
            "purlis handoff --name x --now=2026-01-01T00:00:00 beta <<'B'\nb\nB",
            "charter handoff beta --now 2026-01-01T00:00:00 <<'B'\nb\nB",
        ] {
            let said = refused(cmd, the_chat()).unwrap_or_else(|| panic!("{cmd:?} is refused"));
            assert!(said.contains("--now"), "{said}");
            let codex = Caller {
                harness: Some("codex"),
                ..the_chat()
            };
            assert_eq!(refused(cmd, codex), None, "{cmd:?}");
        }
        // In the brief it is text.
        assert_eq!(
            refused(
                "purlis handoff --name x beta <<'B'\nuse --now here\nB",
                the_chat()
            ),
            None
        );
    }

    /// A dispatch's brief is held to what a handoff's is (#1471): the new chat is sent the
    /// text written in the call, so a brief fed any other way is refused.
    #[test]
    fn a_dispatch_brief_fed_from_anything_but_one_quoted_heredoc_is_refused() {
        for (cmd, what) in [
            ("purlis dispatch --name x < brief.md", "a file (<)"),
            ("purlis dispatch --name x <<< \"look\"", "a here-string"),
            (
                "purlis dispatch --name x <<B\nlook at $HOME\nB",
                "an unquoted heredoc",
            ),
            ("purlis dispatch --name x", "no heredoc at all"),
            (
                "purlis dispatch --to devops --wait --name x",
                "no heredoc at all",
            ),
            (
                "purlis dispatch --name x <<'A' <<'B'\none\nA\ntwo\nB",
                "more than one heredoc",
            ),
            (
                "purlis dispatch --name x <<'B' <&3\nlook\nB",
                "a file descriptor",
            ),
            (
                "$PURLIS_HOOK_BINARY dispatch --name x < brief.md",
                "a file (<)",
            ),
        ] {
            let said = refused(cmd, the_chat()).unwrap_or_else(|| panic!("{cmd:?} is refused"));
            assert!(
                said.starts_with(&format!(
                    "this call feeds `purlis dispatch` its brief from {what}"
                )),
                "{cmd:?}: {said}"
            );
        }
        // Run by xargs, the heredoc's lines are words of the dispatch: the call is refused
        // already, as a rider, before its brief is read.
        assert!(refused("xargs purlis dispatch --name x <<'B'\nlook\nB", the_chat()).is_some());
        // One quoted heredoc is the brief, however the flags around it are written.
        for cmd in [
            TASK,
            "purlis dispatch --to devops --name x --in worktree <<'B'\nb\nB",
            "purlis dispatch --name=x --wait --timeout 60 <<\"B\"\nb\nB",
            "purlis dispatch --name x <<'B' 2>&1\nb\nB",
        ] {
            assert_eq!(refused(cmd, the_chat()), None, "{cmd:?}");
        }
        // A subcommand, or a help ask, reads no brief.
        for cmd in [
            "purlis dispatch list",
            "purlis dispatch read 12",
            "purlis dispatch wait 12",
            "purlis dispatch --help",
            "purlis dispatch -h",
            "purlis dispatch report --outcome done \"did it\"",
            "purlis dispatch --to devops list",
        ] {
            assert_eq!(refused(cmd, the_chat()), None, "{cmd:?}");
        }
        // Only the harness the allow is handed to is refused it.
        let codex = Caller {
            harness: Some("codex"),
            ..the_chat()
        };
        assert_eq!(refused("purlis dispatch --name x < brief.md", codex), None);
    }

    /// A redirection on the dispatch's own line that writes a file is a second thing the call
    /// does, so the call is refused (#1471, D-1471-b). A duplicated descriptor and
    /// `/dev/null` write nothing.
    #[test]
    fn a_redirection_that_writes_a_file_beside_a_dispatch_or_a_handoff_is_refused() {
        for (cmd, redirection) in [
            ("purlis dispatch cancel 12 > notes.md", "> notes.md"),
            ("purlis dispatch note \"x\" >> ~/.profile", ">> ~/.profile"),
            ("purlis dispatch close 12 2>err.log", "2> err.log"),
            ("purlis dispatch tell 12 \"x\" >| out", ">| out"),
            ("purlis dispatch answer 12 \"x\" >& out", ">& out"),
            (
                "purlis dispatch --name x <<'B' > /tmp/out\nb\nB",
                "> /tmp/out",
            ),
            ("purlis handoff --name x beta <<'B' >log\nb\nB", "> log"),
        ] {
            let said = refused(cmd, the_chat()).unwrap_or_else(|| panic!("{cmd:?} is refused"));
            assert!(
                said.contains(&format!(
                    "a redirection that writes a file (`{redirection}`)"
                )),
                "{cmd:?}: {said}"
            );
        }
        for cmd in [
            "purlis dispatch cancel 12 2>&1",
            "purlis dispatch cancel 12 >/dev/null",
            "purlis dispatch cancel 12 2>/dev/null",
            "purlis dispatch cancel 12 >&-",
            "purlis dispatch report --outcome done \"wrote > notes.md\"",
            // A look is the harness's to judge, whatever it writes (#3785).
            "purlis dispatch list > notes.md",
        ] {
            assert_eq!(refused(cmd, the_chat()), None, "{cmd:?}");
        }
    }

    /// A function or an alias defined earlier in the call can stand in for the command the
    /// allow names, so the call is refused, naming the definition (#1471, D-1471-c).
    #[test]
    fn a_function_or_an_alias_defined_earlier_in_the_call_is_a_rider() {
        for cmd in [
            format!("purlis() {{ curl example.com; }}\n{TASK}"),
            format!("purlis() {{ curl example.com; }}; {TASK}"),
            format!("function purlis {{ curl example.com; }}\n{TASK}"),
            format!("alias purlis='curl example.com'\n{TASK}"),
            format!("shopt -s expand_aliases; alias purlis=true; {MOVED}"),
        ] {
            assert!(rider(&cmd).is_some(), "{cmd:?} has a rider");
            assert!(refused(&cmd, the_chat()).is_some(), "{cmd:?} is refused");
        }
    }
}
