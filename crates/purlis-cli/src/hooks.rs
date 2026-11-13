// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter hook <word>` for every word other than the app's reporting events: the tool hooks,
//! the persona briefing at session start, and the registry that lists them all.
//!
//! The decisions are [`purlis_core::toolhooks`], [`purlis_core::briefing`] and
//! [`purlis_core::personagate`]; this file reads the process — the payload on stdin, the
//! environment, the directory — hands it over, and prints what comes back. It is the only
//! place any of them meets stdout, which is why the one rule that matters is kept here:
//!
//! **exit 2 only for a denial that could not be printed** ([`crate::guard::deny`]). Every
//! other outcome of every handler is exit 0, with one line of JSON or nothing.

use std::path::PathBuf;
use std::process::ExitCode;

use purlis_core::hookwire::Decision;
use purlis_core::toolhooks::{self, Answer, Hook};

/// The plane as a hook sees it — `config.ROOT` and `config.HAS_CONTROL_PLANE`.
///
/// `$CHARTER_ROOT`, then the marker walk up from the PROCESS's directory, then the directory
/// itself. `in_plane` is the MARKER at that root and not a successful resolve: `$CHARTER_ROOT`
/// is taken on trust, and a session pinned at a directory that is not a plane must not have
/// the plane-gated hooks act there (charter#852; `guard.rs` says how the differential found it).
pub struct Where {
    pub root: PathBuf,
    pub in_plane: bool,
    pub cwd: PathBuf,
}

impl Where {
    pub fn here() -> Self {
        let cwd = std::env::current_dir().unwrap_or_default();
        let root = purlis_core::plane::resolve(&cwd).unwrap_or_else(|_| cwd.clone());
        let in_plane = purlis_core::names::has_manifest(&root);
        Self {
            root,
            in_plane,
            cwd,
        }
    }
}

/// The instant a hook acts at: `--now` (a LOCAL naive stamp, as everywhere in this binary),
/// else the clock.
pub fn instant(now: Option<&str>) -> chrono::DateTime<chrono::Utc> {
    now.and_then(|text| text.parse::<chrono::NaiveDateTime>().ok())
        .and_then(|naive| chrono::TimeZone::from_local_datetime(&chrono::Local, &naive).single())
        .map_or_else(chrono::Utc::now, |local| local.with_timezone(&chrono::Utc))
}

fn env(name: &str) -> Option<String> {
    purlis_core::envvar::var(name)
}

/// Run `handler` over the process's payload, plane and clock.
pub fn with_hook<T>(payload: &str, now: Option<&str>, handler: impl FnOnce(&Hook) -> T) -> T {
    let here = Where::here();
    let data: serde_json::Value = serde_json::from_str(payload).unwrap_or(serde_json::Value::Null);
    let host = purlis_core::dispatch::host();
    let hook = Hook {
        root: &here.root,
        in_plane: here.in_plane,
        payload: &data,
        env: &env,
        cwd: &here.cwd,
        now: instant(now),
        host: &host,
    };
    handler(&hook)
}

/// What a tool hook decided, before it is printed: what the host's event log records of it
/// (FD-9), and what the harness is told.
///
/// **Decided first, printed last** (FD-30): the hook tells the host, or spools, before it
/// answers, so an answer the harness has is one whose event is recorded or durable.
pub struct Answered {
    pub decision: Decision,
    /// The guard rule that refused, for a denial.
    pub rule: Option<String>,
    /// The gate's arm that refused, for a denial the gate gave.
    pub arm: Option<purlis_core::toolgate::Arm>,
    said: Said,
}

/// What an [`Answered`] tells the harness.
enum Said {
    /// An answer, printed by [`answered`].
    Answer(Answer),
    /// An exit status already decided, with nothing to print.
    Exit(ExitCode),
}

impl Answered {
    /// `answer`, decided and not printed.
    pub fn of(answer: Answer) -> Self {
        Self {
            decision: decision_of(&answer),
            rule: match &answer {
                Answer::Deny(verdict) => Some(verdict.reason.clone()),
                Answer::Nothing | Answer::Say(_) => None,
            },
            arm: match &answer {
                Answer::Deny(verdict) => verdict.arm,
                Answer::Nothing | Answer::Say(_) => None,
            },
            said: Said::Answer(answer),
        }
    }

    /// An exit status with nothing printed: a refusal whose reason is on stderr, or nothing.
    pub fn exit(code: ExitCode, decision: Decision, rule: Option<&str>) -> Self {
        Self {
            decision,
            rule: rule.map(str::to_owned),
            arm: None,
            said: Said::Exit(code),
        }
    }

    /// Tells the harness, and answers the exit status it reads.
    pub fn print(self) -> ExitCode {
        match self.said {
            Said::Answer(answer) => answered(&answer),
            Said::Exit(code) => code,
        }
    }
}

/// A tool hook's handler, run over `payload`, its answer decided and not printed yet.
pub fn run(handler: Handler, payload: &str, now: Option<&str>) -> Answered {
    Answered::of(with_hook(payload, now, handler))
}

/// What answers one tool hook word.
pub type Handler = fn(&Hook) -> Answer;

/// The handler for the tool hook `name`, if it is one this file answers.
pub fn handler(name: &str) -> Option<Handler> {
    Some(match name {
        "pretooluse-read" => toolhooks::pretooluse_read,
        "pretooluse-edit" => pretooluse_edit,
        "pretooluse-dispatch" => toolhooks::pretooluse_dispatch,
        "posttooluse" => toolhooks::posttooluse,
        "posttooluse-skill" => toolhooks::posttooluse_skill,
        "userpromptexpansion" => says_nothing,
        "posttooluse-dispatch" => toolhooks::posttooluse_dispatch,
        // What the call started is told the app beside its line (#5750); the harness is told
        // nothing.
        OWN_WORK => says_nothing,
        // What these hooks find goes to the app ([`blocks`]); the chat is told of what the app
        // took once it has ([`told_after`]), never from here.
        BLOCKED | BLOCKED_ON_FAILURE => says_nothing,
        // A purlis tool that failed (#5697): what failed inside purlis goes to the app as a
        // draft ([`tell_the_app_what_failed`]); the chat is told on its next turn, not here.
        purlis_core::caught::TOOL_WORD => says_nothing,
        // The harness's own check refused the call (R-094): the host writes the block from the
        // line every tool hook sends; the harness is told nothing, so its refusal stands.
        purlis_core::hookreg::HARNESS_DENIED => says_nothing,
        _ => return None,
    })
}

/// `pretooluse-edit`: the state-directory guard, and a sandboxed Claude Code chat's edit of
/// purlis's own configuration handed to the app as a proposed change (#1975), which the window
/// shows the person to apply ([`toolhooks::pretooluse_edit_proposing`]).
fn pretooluse_edit(hook: &Hook) -> Answer {
    toolhooks::pretooluse_edit_proposing(hook, &|ask| {
        crate::brokered::proposal_taken(purlis_core::hookwire::Ask::Propose(Box::new(ask)))
    })
}

fn says_nothing(_: &Hook) -> Answer {
    Answer::Nothing
}

/// The hook words that read a Bash command's result for a sandbox block (#1338): the end of a
/// call that came back, and of one that failed. The first also hears a background command's
/// output read back by another tool (#1683), which `sandboxblock` reads only of such a file.
pub const BLOCKED: &str = "posttooluse-blocked";
pub const BLOCKED_ON_FAILURE: &str = purlis_core::caught::COMMAND_WORD;

/// The hook word that hears a tool that only starts own work come back (#5750).
pub const OWN_WORK: &str = "posttooluse-ownwork";

/// What a block word found in a call's result, as the chat is told of it.
#[derive(Debug, Default)]
pub struct Blocked {
    /// The blocks the app took, each a Notice for the person.
    pub taken: Vec<purlis_core::sandboxblock::Block>,
    /// Whether a block was the chat's own to retry, which raises no Notice, and the chat is
    /// told the purlis command that does it ([`purlis_core::sandboxblock::its_own_to_retry`],
    /// #3861).
    pub retry_with_purlis: bool,
}

/// The line block word `word` prints for the blocks the app took, `taken` (#1631): the chat is
/// told, in the turn it was blocked, that the person has a Notice and it should say what was
/// blocked and wait ([`purlis_core::sandboxblock::told_the_chat`]). None for another word, and
/// when there is nothing to tell. Where the call ran nothing and only read the words back
/// (`read_back`, #1884), the chat is told that instead, and not to wait
/// ([`purlis_core::sandboxblock::told_the_chat_of_a_read`]).
///
/// And, in the same line since a hook prints one answer, what a git command the sandbox
/// stopped partway left behind and how to finish it or go back ([`stopped_partway`], qw349).
///
/// And first, a block that was the chat's own to retry (`blocked.retry_with_purlis`, #3861):
/// told the purlis command that does it ([`purlis_core::sandboxblock::told_the_chat_to_retry`]).
/// Never of words only read back, where no command of the call was refused.
pub fn told_after(
    word: &str,
    blocked: &Blocked,
    read_back: bool,
    stopped: Option<String>,
) -> Option<String> {
    let event = match word {
        BLOCKED => "PostToolUse",
        BLOCKED_ON_FAILURE => "PostToolUseFailure",
        _ => return None,
    };
    let (own, told) = if read_back {
        (
            None,
            purlis_core::sandboxblock::told_the_chat_of_a_read(&blocked.taken),
        )
    } else {
        (
            blocked
                .retry_with_purlis
                .then(purlis_core::sandboxblock::told_the_chat_to_retry),
            purlis_core::sandboxblock::told_the_chat(&blocked.taken),
        )
    };
    let said: Vec<String> = own.into_iter().chain(told).chain(stopped).collect();
    if said.is_empty() {
        return None;
    }
    Some(
        serde_json::json!({
            "hookSpecificOutput": {"hookEventName": event, "additionalContext": said.join("\n\n")}
        })
        .to_string(),
    )
}

/// What a sandboxed chat is told after a Bash command in which git said it could not write a
/// file the sandbox denies by name ([`purlis_core::cloneupdate::stopped`], qw349): the folder
/// is half-written, and how to finish it or go back. Read from the command's own words, on
/// either stream, and said only to the chat that ran it. None in a chat the app did not
/// sandbox, where such a refusal is not the sandbox's.
pub fn stopped_partway(
    payload: &serde_json::Value,
    env: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    if !purlis_core::sandbox::chat_is_sandboxed_in(env) {
        return None;
    }
    let command = payload["tool_input"]["command"].as_str()?;
    let response = &payload["tool_response"];
    let said: Vec<&str> = [
        response.as_str(),
        response["stderr"].as_str(),
        response["stdout"].as_str(),
        payload["error"].as_str(),
    ]
    .into_iter()
    .flatten()
    .collect();
    purlis_core::cloneupdate::stopped(command, &said.join("\n"))
}

/// Whether `word` is one of the block words.
pub fn is_a_block_word(word: &str) -> bool {
    word == BLOCKED || word == BLOCKED_ON_FAILURE
}

/// The sandbox blocks in a block word's `payload` ([`purlis_core::sandboxblock`]), read through
/// `env`: none unless the app started this chat sandboxed
/// ([`purlis_core::sandbox::chat_is_sandboxed`]), since a refusal in any other chat is not the
/// sandbox's, and none without the folder the app started the chat in
/// ([`purlis_core::sandboxblock::CHAT_DIR_ENV`], else Claude Code's `CLAUDE_PROJECT_DIR`), which
/// is never guessed from a working directory. Sorted against that folder, the project this hook
/// resolves, the folder the payload says the command ran in for a relative path, and the home.
pub fn blocks(
    payload: &serde_json::Value,
    env: &dyn Fn(&str) -> Option<String>,
) -> Vec<(purlis_core::sandboxblock::Block, Option<String>)> {
    use purlis_core::sandboxblock::{Place, background, detect_with_record};
    if !purlis_core::sandbox::chat_is_sandboxed_in(env) {
        return Vec::new();
    }
    let absolute = |path: Option<String>| path.map(PathBuf::from).filter(|p| p.is_absolute());
    let Some(chat) = chat_dir(env) else {
        return Vec::new();
    };
    let here = Where::here();
    let cwd = absolute(payload["cwd"].as_str().map(str::to_owned)).unwrap_or_else(|| chat.clone());
    let home = absolute(env("HOME"));
    let harness = purlis_core::hookwire::chat_harness(env);
    // A wrapped harness's sandbox writes no violation report: one in its result is printed.
    let reported = purlis_core::sandboxblock::as_its_harness_reports(payload, harness.as_deref());
    // A background command's output read back by file is read by how Claude Code's own
    // record of the task says it ended (#2203).
    let ended = background::how_it_ended(&reported);
    let found = detect_with_record(
        &reported,
        &Place {
            root: &here.root,
            chat: &chat,
            cwd: &cwd,
            home: home.as_deref(),
        },
        ended,
    );
    // A chat purlis wraps has its refused hosts told by purlis's own proxy, by name (#1663),
    // and on Linux its files, sockets and services by its wrap's supervisor (#2344).
    purlis_core::sandboxblock::the_hooks_own(
        found,
        harness.as_deref(),
        purlis_core::sandbox::Os::this(),
    )
}

/// The folder the app started this chat in ([`purlis_core::sandboxblock::CHAT_DIR_ENV`], else
/// Claude Code's `CLAUDE_PROJECT_DIR`), never guessed from a working directory: what a block is
/// sorted against, and whose branch folder is the chat's own (#3861).
pub fn chat_dir(env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let absolute = |path: Option<String>| path.map(PathBuf::from).filter(|p| p.is_absolute());
    absolute(env(purlis_core::sandboxblock::CHAT_DIR_ENV))
        .or_else(|| absolute(env("CLAUDE_PROJECT_DIR")))
}

/// The decision an answer gave: a denial is one; a line said is whatever permission it names,
/// read from the JSON charter itself printed and never from a harness's output.
pub fn decision_of(answer: &Answer) -> Decision {
    match answer {
        Answer::Nothing => Decision::None,
        Answer::Deny(_) => Decision::Deny,
        Answer::Say(line) => decision_said(line),
    }
}

/// The permission a line charter prints names, if any.
pub fn decision_said(line: &str) -> Decision {
    let said: serde_json::Value = serde_json::from_str(line).unwrap_or_default();
    match said["hookSpecificOutput"]["permissionDecision"].as_str() {
        Some("allow") => Decision::Allow,
        Some("ask") => Decision::Ask,
        Some("deny") => Decision::Deny,
        _ => Decision::None,
    }
}

/// Print an [`Answer`]: nothing, a line, or a denial with its exit-2 fallback.
pub fn answered(answer: &Answer) -> ExitCode {
    match answer {
        Answer::Nothing => ExitCode::SUCCESS,
        Answer::Say(line) => {
            say(line);
            ExitCode::SUCCESS
        }
        Answer::Deny(verdict) => crate::guard::deny(verdict),
    }
}

/// One line on stdout. A failure is dropped: what failed to print is context or an allow, and
/// neither is worth the exit status that would turn it into a block.
pub fn say(line: &str) {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

/// `sessionstart`'s own work, before the app is told anything: the piece announcement, this
/// session's heartbeat, the persona tool gate's ceiling, and the briefing — printed as
/// `additionalContext`. Silent outside a plane, where there is no persona and no workspace.
pub fn sessionstart(payload: &str, now: Option<&str>) {
    with_hook(payload, now, |hook| {
        if !hook.in_plane {
            return;
        }
        // BEFORE this session's own heartbeat, which would otherwise replace the holder's mark
        // and hide the collision the announcement exists to show.
        let piece = purlis_core::briefing::piece_announcement(hook.root, hook.payload, hook.now);
        hook.touch_piece();
        // Freeze every persona's tools before the session has had a turn in which to rewrite
        // one (charter#432). Keyed on the payload's id, as every later ask is.
        let explicit = hook.payload.get("session_id").and_then(|v| v.as_str());
        let sid = purlis_core::hookstate::session(explicit, &env);
        // **Beside the briefing, and finished before the hook answers** (#5486): reading every
        // persona's tools and writing the ceiling, flushed as it always is, took a third of a
        // start at 100 personas, and on macOS its flushes (`F_FULLFSYNC`) are the slowest
        // writes the hook makes. Nothing the briefing reads is the ceiling, and the scope joins
        // the snapshot before anything is printed: the session's first turn, which comes only
        // after the hook answers, still meets a ceiling on disk.
        std::thread::scope(|scope| {
            let frozen = sid.map(|sid| {
                let root = hook.root;
                scope.spawn(move || purlis_core::personagate::snapshot(root, &sid))
            });
            let ask = briefing_ask(hook);
            // A Codex chat is told the rest in hooks of their own, each with the 10,000 bytes
            // Codex keeps of one (#5935): the notes and the quoted ([`sessionstart_part`]).
            let parts = if purlis_core::harness::codex::briefed_in_parts(&env) {
                purlis_core::briefing::parts_of(&ask, piece, purlis_core::briefing::Part::Briefing)
            } else {
                let mut parts = purlis_core::briefing::parts(&ask, piece);
                parts.extend(from_others(hook, &ask));
                parts
            };
            if let Some(frozen) = frozen {
                // A panic in it is the hook's own, as it was when it ran in line.
                frozen
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
            }
            if let Some(line) = purlis_core::briefing::emitted(&parts) {
                say(&line);
            }
        });
    });
}

/// What the briefing reads of the session `hook` is run in.
fn briefing_ask<'a>(hook: &'a Hook) -> purlis_core::briefing::Ask<'a> {
    purlis_core::briefing::Ask {
        root: hook.root,
        cwd: hook.cwd,
        payload: hook.payload,
        env: &env,
        now: hook.now,
    }
}

/// **What others say to a chat at its start, quoted**: each extension's section, the reports
/// kept for its place, and where it is working. The end of Claude Code's briefing, and of a
/// Codex chat's quoted hook ([`purlis_core::harness::codex::QUOTED_WORD`]).
///
/// It does more than read: it takes the reports kept for the chat's place, and tells each
/// extension that hears it that a chat started. So it runs once a chat, in one hook.
fn from_others(hook: &Hook, ask: &purlis_core::briefing::Ask) -> Vec<String> {
    let mut parts = Vec::new();
    let workspace = purlis_core::briefing::workspace_of(ask);
    // Where this session is: a chat at the plane root is in no workspace (SI-1b), so it takes
    // no workspace's choices and learns the reports kept for the plane root.
    let place = purlis_core::briefing::place_of(ask);
    // Each extension that is on here and adds a section, quoted as data under its name — and
    // each that hears it told a chat started — within one bounded wait (charter-app#343). None
    // of it can hold the start past `Bounds::SESSION_START`, and a machine with no config
    // directory has no extension to ask.
    if let Some(config) = purlis_core::machine::config_root_if_there() {
        use purlis_core::extension::briefing::{self, Asked};
        // The built-ins of the bundle this binary shipped in, as `purlis statusline` draws
        // their badges (#1366): none when it is not inside one.
        let briefed = briefing::at_session_start(
            &config,
            &purlis_core::extension::BuiltIn::of_this_program(),
            &purlis_core::extension::project::Choices::read_in(hook.root, place.workspace()),
            &Asked {
                workspace,
                persona: env(purlis_core::active::PERSONA_ENV).filter(|it| !it.is_empty()),
            },
            crate::extensions::session_start_bounds(),
        );
        parts.extend(briefed.parts);
        for note in briefed.notes {
            eprintln!("purlis: {note}");
        }
    }
    // Reports kept for this workspace because the chat that asked for them has closed
    // (charter-app#259): the next chat to START here is the one that learns them — never a chat
    // already running that compacted or cleared, which would take them as a side effect of its
    // own housekeeping.
    let starting = hook
        .payload
        .get("source")
        .and_then(|v| v.as_str())
        .is_none_or(|source| source == "startup");
    let kept = if starting {
        crate::dispatch::handbacks(hook.root, purlis_core::handback::For::Place(&place))
    } else {
        Vec::new()
    };
    if let Some(reports) = purlis_core::handback::context(&kept, true) {
        parts.push(reports);
    }
    // Where this chat is working (#1450): who asked for it, its sibling tasks and the other
    // chats running as its persona, as the app that started it has them recorded.
    if let Some(working) = crate::whereworking::briefing(hook.now) {
        parts.push(working);
    }
    parts
}

/// **A part of a Codex chat's briefing, as a `SessionStart` hook's output of its own** (#5935):
/// [`purlis_core::briefing::Part::Notes`] or [`purlis_core::briefing::Part::Quoted`], the second
/// with [`from_others`]. Codex keeps 10,000 bytes of each hook's output and cuts the middle, and
/// one briefing in an ordinary workspace passes that. Nothing for any other chat, which
/// `sessionstart` tells it all, or outside a plane. It reports no state: the briefing's hook is
/// the one that says the session started.
pub fn sessionstart_part(part: purlis_core::briefing::Part, payload: &str, now: Option<&str>) {
    with_hook(payload, now, |hook| {
        if !hook.in_plane || !purlis_core::harness::codex::briefed_in_parts(&env) {
            return;
        }
        let ask = briefing_ask(hook);
        let mut parts = purlis_core::briefing::parts_of(&ask, None, part);
        if part == purlis_core::briefing::Part::Quoted {
            parts.extend(from_others(hook, &ask));
        }
        if let Some(line) = purlis_core::briefing::emitted(&parts) {
            say(&line);
        }
    });
}

/// **A Codex chat's skills, as a `SessionStart` hook's output of their own** (#5841): the
/// listing [`purlis_core::briefing::skills_part`] makes, printed as `additionalContext`, or
/// nothing for a chat purlis hands no skills, or outside a plane. Codex keeps 10,000 bytes
/// of each hook's output and cuts the middle, so the listing is never the briefing's middle.
pub fn sessionstart_skills(payload: &str, now: Option<&str>) {
    with_hook(payload, now, |hook| {
        if !hook.in_plane {
            return;
        }
        let ask = briefing_ask(hook);
        let parts: Vec<String> = purlis_core::briefing::skills_part(&ask)
            .into_iter()
            .collect();
        if let Some(line) = purlis_core::briefing::emitted(&parts) {
            say(&line);
        }
    });
}

/// `userpromptsubmit`'s own work, handed to the turn that is starting as one
/// `additionalContext`: the heartbeat; the commitment gate
/// ([`purlis_core::commitgate`], charter#369), which tells a prompt asking for work with a real
/// fork in it to scout and ask before building; and any report a chat this one handed work to
/// has sent back (charter-app#259), quoted as data, never typed into the chat; one line when
/// where the chat is working has changed ([`crate::whereworking`], #1450); what the chat's
/// router points the prompt at, where its start armed it for a router that reads prompts
/// (SK-15c); and one line naming the files the operator changed since its last turn (ADR 0084
/// §8).
///
/// Of the rest of the Python handler, the persona roster went with `routing:`, which is
/// retired, and "control plane updated" is the window's to say, as a mark on the chat's tab
/// (charter#369's ruling).
pub fn userpromptsubmit(payload: &str, now: Option<&str>) {
    with_hook(payload, now, |hook| {
        hook.touch_piece();
        if !hook.in_plane {
            return;
        }
        let mut parts: Vec<String> = purlis_core::commitgate::nudge(hook).into_iter().collect();
        // What this chat's router points the prompt at (SK-15c): only where the chat's start
        // armed it for a router that reads prompts, so no other chat's prompt leaves this hook.
        if env(purlis_core::skills::PROMPT_ROUTER_ENV).as_deref() == Some("1")
            && let Some(prompt) = hook
                .payload
                .get("prompt")
                .and_then(serde_json::Value::as_str)
        {
            parts.extend(crate::dispatch::pointed(prompt));
        }
        // The app's number for this chat, which is what a report is left under. A chat the app
        // did not start has none, and nothing was left for it.
        let chat: Option<u32> = env(purlis_core::hookwire::CHAT_ENV).and_then(|id| id.parse().ok());
        // **The app is asked first, and the files are taken after** (#1496). An ask can wait
        // on the app, and a hook can be ended while it waits: a report or a message taken off
        // disk before that would be lost with it. So: what the person said to this chat in
        // the purlis window (their answer to a question this task asked, or word that they
        // answered one a task of its asked), which is the app's own to say and is read from
        // no file; and one line when where this chat is working has changed since it was
        // last told (#1450), by the app's own count of what it was told.
        let person = chat.and_then(|_| crate::dispatch::from_the_person());
        let working = crate::whereworking::update(hook.now);
        if let Some(chat) = chat {
            let reports =
                crate::dispatch::handbacks(hook.root, purlis_core::handback::For::Chat(chat));
            parts.extend(purlis_core::handback::context(&reports, false));
            // And what the chat that dispatched this one, or a task it dispatched, sent it
            // (#1442): quoted as data, as a report is.
            let messages = crate::dispatch::messages(hook.root, chat);
            parts.extend(purlis_core::dispatchtalk::context(&messages));
        }
        parts.extend(person.as_ref().map(|person| person.told.clone()));
        parts.extend(working);
        // What the operator changed since this chat's last turn (ADR 0084 §8): context on a
        // turn it began anyway. The report made after this hook is what closes those edits.
        parts.extend(chat.and_then(|_| crate::dispatch::human_edits()));
        if !parts.is_empty() {
            say(&purlis_core::handback::emitted(
                "UserPromptSubmit",
                &parts.join("\n\n"),
            ));
        }
        // **Only now does the turn have what the person said**: it is printed. Said to the
        // app after that and never before, so a hook ended anywhere above loses nothing: the
        // app still holds it, and the next turn is handed it again.
        if let Some(person) = person {
            person.handed_over();
        }
    });
}

/// `charter hook --list`.
pub fn list(json: bool) -> ExitCode {
    if json {
        say(&purlis_core::hookreg::json());
    } else {
        print!("{}", purlis_core::hookreg::table());
    }
    ExitCode::SUCCESS
}

/// How long a hook waits for the app to hold a draft of purlis's own failure: a scrub and one
/// small write. The hook answers the harness nothing of it either way.
const HOLDING_A_FAILURE_TAKES_AT_MOST: std::time::Duration = std::time::Duration::from_secs(3);

/// **A failure of purlis's own in the call `word` heard come back, handed to the app** (#5697,
/// spec #5683 layer 1): an internal-failure exit of a `purlis` command, or a purlis tool that
/// failed inside purlis ([`purlis_core::caught::of_hook`]). The app holds it as a Report draft
/// for the person's Inbox, with the person's yes to Agent drafts, and tells the chat on its next
/// turn. Nothing is said here: a refusal, no chat, or no app is the same silence.
pub fn tell_the_app_what_failed(word: &str, payload: &str) {
    if word != BLOCKED_ON_FAILURE && word != purlis_core::caught::TOOL_WORD {
        return;
    }
    let Ok(data) = serde_json::from_str::<serde_json::Value>(payload) else {
        return;
    };
    let Some(caught) = purlis_core::caught::of_hook(word, &data) else {
        return;
    };
    let _ = crate::brokered::asked(
        |chat| {
            purlis_core::hookwire::Ask::ReportDraft(Box::new(
                purlis_core::hookwire::ReportDraftAsk::caught(chat, caught),
            ))
        },
        HOLDING_A_FAILURE_TAKES_AT_MOST,
    );
}
