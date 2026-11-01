// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Brokered writes from the command line (ADR 0067 §2, #1333): inside a chat the app started, a
//! `purlis` command that writes the project's own files hands the write to the app over the
//! chat's hook socket, and prints what the app wrote as it would have printed its own write.
//!
//! The rules for when to hand it over, and when to write here instead, are the session
//! record's (#1332), and this module is where both read them:
//!
//! - **No socket, no chat number, or a socket nothing listens on:** no app takes it, and the
//!   command writes it itself, as it always has.
//! - **The app ended the connection at once:** an app older than the ask, which drops a line
//!   it cannot read. The command writes it itself.
//! - **The app went quiet:** it may be writing it now. The command says so and writes nothing,
//!   so nothing is ever written twice.

use std::path::Path;

use purlis_core::brokered::Write;
use purlis_core::hookwire::{self, Answer, Ask, SOCKET_ENV};

/// What became of a write handed to the app.
pub enum Forwarded {
    /// The app wrote it: the workspace or persona it went to, and the file, project-relative.
    Written { to: String, path: String },
    /// The app read it and will not write it: the sentence says why.
    Refused(String),
    /// No app took it. A command, which runs inside the chat's sandbox, writes it itself; the
    /// MCP server, which does not, writes it only where [`NotTaken::may_write_outside`] says.
    NotTaken(NotTaken),
    /// The app took it and did not answer: it may be written, so nothing is written here.
    Unsure(String),
}

/// Why no app took a write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotTaken {
    /// This is no chat the app started: no chat number.
    NotAChat,
    /// A chat the app started, whose harness gave this process no connection to the app: none
    /// of [`purlis_core::chattools::CONNECTION_ENV`] reached it.
    NoConnection,
    /// The app's socket is not there, or nothing listens on it: the app has gone.
    AppGone,
    /// The app ended the connection without answering: an app older than the ask.
    Dropped,
}

impl NotTaken {
    /// Whether a process running outside the chat's sandbox may write it itself: only where
    /// there is no app that could have (#1333). Anything else would be a write no sandbox
    /// bounds, no rate holds and the trace does not credit.
    pub fn may_write_outside(self) -> bool {
        matches!(self, Self::NotAChat | Self::AppGone)
    }

    /// The sentence a process outside the sandbox refuses with, where it may not write:
    /// `command` is the one to run in the chat instead.
    pub fn refusal(self, command: &str) -> String {
        format!("{}: run `{command}` in the chat instead", self.why())
    }

    /// Why the write was not made, without what to do instead: for a write no command in the
    /// chat makes, whose refusal names something else to do.
    pub fn why(self) -> &'static str {
        match self {
            Self::NoConnection => {
                "this harness does not hand purlis's tools the chat's connection to the app, so \
                 the app cannot write it for this chat"
            }
            _ => "the app that started this chat did not take the write, so nothing was written",
        }
    }
}

/// How long the app may take to write one memory or todo and answer.
const A_WRITE_TAKES_AT_MOST: std::time::Duration = std::time::Duration::from_secs(10);

/// Whether this command would hand a write to an app: it runs in a chat the app started, with
/// a connection to it.
fn in_a_chat_with_a_connection() -> bool {
    crate::session::chat_number().is_some()
        && purlis_core::envvar::var_os(SOCKET_ENV).is_some_and(|s| !s.is_empty())
}

/// Why `write` cannot be handed to the app, said before it is: a text longer than one line of
/// the chat's connection carries would be cut, end the connection and be written here instead
/// (#1408). Only where it would be handed over: a terminal writes any length itself.
fn too_long_to_hand_over(write: &Write) -> Option<String> {
    if !in_a_chat_with_a_connection() {
        return None;
    }
    write
        .check()
        .err()
        .map(|why| format!("{why}. Nothing was written"))
}

/// Hands `write` to the app that started this chat, if one did.
pub fn forwarded(write: Write) -> Forwarded {
    if let Some(why) = too_long_to_hand_over(&write) {
        return Forwarded::Refused(why);
    }
    let answer = match asked(
        |chat| {
            Ask::Write(Box::new(hookwire::WriteAsk {
                chat,
                write: write.clone(),
            }))
        },
        A_WRITE_TAKES_AT_MOST,
    ) {
        Ok(answer) => answer,
        Err(not) => return Forwarded::NotTaken(not),
    };
    match answer {
        Ok(Answer::Written { to, path }) => Forwarded::Written { to, path },
        Ok(Answer::No { why }) if why == hookwire::NOTHING_ANSWERS => {
            Forwarded::NotTaken(NotTaken::Dropped)
        }
        Ok(Answer::No { why }) => Forwarded::Refused(why),
        Ok(_) => Forwarded::NotTaken(NotTaken::Dropped),
        Err(e) if not_taken(&e) => Forwarded::NotTaken(NotTaken::Dropped),
        Err(e) => Forwarded::Unsure(format!(
            "the app took this write and did not answer ({e}), so it may already be written. \
             Nothing was written again here: list them to see whether it is there."
        )),
    }
}

/// This chat's number and its hook socket, or why it has no connection to the app.
fn the_chat_s_connection() -> Result<(u32, std::ffi::OsString), NotTaken> {
    let chat = crate::session::chat_number().ok_or(NotTaken::NotAChat)?;
    let socket = purlis_core::envvar::var_os(SOCKET_ENV)
        .filter(|s| !s.is_empty())
        .ok_or(NotTaken::NoConnection)?;
    Ok((chat, socket))
}

/// Why connecting to the app failed, as a write that was not taken.
fn not_connected(e: &std::io::Error) -> NotTaken {
    use std::io::ErrorKind;
    if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::ConnectionRefused) {
        NotTaken::AppGone
    } else {
        NotTaken::Dropped
    }
}

/// The app's answer to the ask `make` builds for this chat's number, or why no app can be
/// asked.
pub fn asked(
    make: impl FnOnce(u32) -> Ask,
    within: std::time::Duration,
) -> Result<std::io::Result<Answer>, NotTaken> {
    let (chat, socket) = the_chat_s_connection()?;
    let mut asking = hookwire::Asking::on(Path::new(&socket), hookwire::ChatToken::from_env())
        .map_err(|e| not_connected(&e))?;
    Ok(asking.ask(&make(chat), within))
}

/// The app's reply to the ask `make` builds for this chat's number, over the hook channel's
/// framed stream where the app speaks it (#1996), so an answer larger than a line comes back
/// whole with what the app attaches; in lines, as [`asked`], where the app is older. Said with
/// the chat's own token, as every line of the chat's is.
pub fn replied(
    make: impl FnOnce(u32) -> Ask,
    within: std::time::Duration,
) -> Result<std::io::Result<hookwire::framed::Reply>, NotTaken> {
    use hookwire::framed::Greeted;
    let (chat, socket) = the_chat_s_connection()?;
    let token = hookwire::ChatToken::from_env();
    match hookwire::Framed::on(Path::new(&socket), token.as_ref(), chat, within) {
        Ok(Greeted::Framed(mut framed)) => Ok(framed.ask(&make(chat), within)),
        Ok(Greeted::Older) => asked(make, within).map(|answer| answer.map(Into::into)),
        // The app read the hello and said no: its sentence, as a refused ask's.
        Ok(Greeted::Refused(why)) => Ok(Ok(Answer::No { why }.into())),
        Err(e) => Err(not_connected(&e)),
    }
}

/// Whether asking failed the way an app that never took the ask fails: it ended the
/// connection at once.
pub fn not_taken(e: &std::io::Error) -> bool {
    use std::io::ErrorKind;
    matches!(
        e.kind(),
        ErrorKind::UnexpectedEof | ErrorKind::BrokenPipe | ErrorKind::ConnectionReset
    )
}

/// Hands a memory verb (#1408: forget, edit, archive, unarchive, move, `optimize --apply`, a
/// todo's close) to the app that started this chat, and prints what the app said as this
/// command's own run would have printed it; its exit code. `None` when no app took it, so the
/// command runs here, as it always did.
pub fn said_by_the_app(write: Write) -> Option<u8> {
    if let Some(why) = too_long_to_hand_over(&write) {
        crate::voice::err(why.to_string());
        return Some(1);
    }
    let answer = asked(
        |chat| Ask::Write(Box::new(hookwire::WriteAsk { chat, write })),
        A_WRITE_TAKES_AT_MOST,
    )
    .ok()?;
    match answer {
        Ok(Answer::Said { lines, code }) => {
            for line in lines {
                crate::speak(line);
            }
            Some(code)
        }
        Ok(Answer::No { why }) if why == hookwire::NOTHING_ANSWERS => None,
        Ok(Answer::No { why }) => {
            crate::voice::err(why.to_string());
            Some(1)
        }
        // An app that answers this with anything else did not read it as this.
        Ok(_) => None,
        Err(e) if not_taken(&e) => None,
        Err(e) => {
            crate::voice::err(format!(
                "the app took this and did not answer ({e}), so it may already be done. \
                 Nothing was done again here: list them to see."
            ));
            Some(1)
        }
    }
}

/// The sentence a sandboxed chat is refused with when it names a persona other than its own
/// (#1408, D-1408-2): it changes only its own persona's memory and shared memory, so nothing is
/// written, and the way round is named.
pub fn not_its_persona(named: &str, own: Option<&str>) -> String {
    let runs_as = match own {
        Some(own) => format!("runs as '{own}'"),
        None => "runs as no persona".to_owned(),
    };
    format!(
        "this chat {runs_as}, and a sandboxed chat changes only the memory of the persona it \
         runs as and shared memory, not that of '{named}': nothing was written. Name its own \
         persona, add --shared, or ask the person to change it from the window"
    )
}

/// How long a proposed change waits for the app to take it: inside the edit hook's own budget.
const A_PROPOSAL_TAKES_AT_MOST: std::time::Duration = std::time::Duration::from_secs(4);

/// **Whether the app took a proposed change** (#1975): it answers [`Answer::Said`] when it holds
/// it for the person. Anything else (no chat, no connection, an app older than the ask, one that
/// refused it) is not taken, and the chat is refused its edit as before.
pub fn proposal_taken(ask: Ask) -> bool {
    matches!(
        asked(|_| ask, A_PROPOSAL_TAKES_AT_MOST),
        Ok(Ok(Answer::Said { code: 0, .. }))
    )
}

/// **`purlis propose <file> --from <draft>`** (#1975, PC-2): in a chat the app started, hand the
/// app `draft`, a file in the chat's own folder, as the whole new content of `file`, one of
/// purlis's own configuration files in the project at `root`, for the person to apply in the
/// window. Nothing is written here. What the app says is printed; where nothing took it, why.
pub fn propose(root: &Path, file: &Path, draft: &Path) -> Result<u8, String> {
    use purlis_core::proposed;
    let not_a_chat = || {
        "purlis propose is for a chat the app started: there is nobody here to show the change \
         to. Edit the file yourself."
            .to_owned()
    };
    let chat_dir = purlis_core::envvar::var(purlis_core::sandboxblock::CHAT_DIR_ENV)
        .filter(|dir| Path::new(dir).is_absolute())
        .ok_or_else(not_a_chat)?;
    if let Some(why) = purlis_core::sandbox::policy::Locks::of(root).proposed_changes_refused() {
        return Err(why);
    }
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    // A plane skill's file is kept by the app as a draft skill instead (SK-9, #2772).
    let rel = proposed::proposable(root, &here.join(file)).or_else(|why| {
        purlis_core::skills::verbs::drafts::draftable(root, &here.join(file))
            .map(|skill| std::path::PathBuf::from(skill.rel()))
            .ok_or(why)
    })?;
    // The draft is the chat's own: in its folder, by the path the kernel names it by.
    let real =
        |path: &Path| std::fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()));
    let draft = real(&here.join(draft))?;
    if !draft.starts_with(real(Path::new(&chat_dir))?) {
        return Err(format!(
            "the draft {} is not in this chat's own folder: write it there, then propose it",
            draft.display()
        ));
    }
    let content = std::fs::read(&draft).map_err(|e| format!("{}: {e}", draft.display()))?;
    if content.len() > proposed::MOST_BYTES {
        return Err(format!(
            "the draft is longer than the {} bytes a proposed change may be",
            proposed::MOST_BYTES
        ));
    }
    let content = String::from_utf8(content)
        .map_err(|_| "the draft is not text purlis can show".to_owned())?;
    let proposal = proposed::read_and_apply(root, &rel, proposed::Change::Whole { content })?;
    let answered = asked(
        |chat| {
            Ask::Propose(Box::new(hookwire::ProposeAsk {
                chat,
                path: proposal.path.display().to_string(),
                base: proposal.base,
                change: proposal.change,
            }))
        },
        A_WRITE_TAKES_AT_MOST,
    );
    match answered {
        Ok(Ok(Answer::Said { lines, code })) => {
            for line in lines {
                crate::speak(line);
            }
            Ok(code)
        }
        Ok(Ok(Answer::No { why })) => Err(why),
        Ok(Ok(_)) => Err("the app answered something purlis did not ask for".to_owned()),
        Ok(Err(e)) => Err(format!("the app did not answer: {e}")),
        Err(_) => Err(not_a_chat()),
    }
}

/// **`purlis pool limit` in a chat** (#4284, the person's ruling of 2026-11-01 on PC-1): hands the
/// app the value as a proposal, for the person to apply in the window. Nothing is written here,
/// and nothing that goes wrong writes anything.
pub fn propose_limit(
    pool: &str,
    limit: &purlis_core::poollimits::Limit,
    value: Option<u64>,
) -> Result<u8, String> {
    let answered = asked(
        |chat| {
            Ask::ProposeLimit(Box::new(hookwire::ProposeLimitAsk {
                chat,
                pool: pool.to_owned(),
                key: limit.key.to_owned(),
                value,
            }))
        },
        A_WRITE_TAKES_AT_MOST,
    );
    let nothing = "so nothing was changed: ask the person to change it in the window";
    match answered {
        Ok(Ok(Answer::Said { lines, code })) => {
            for line in lines {
                crate::speak(line);
            }
            Ok(code)
        }
        Ok(Ok(Answer::No { why })) => Err(why),
        Ok(Ok(_)) => Err(format!("this app can't take a proposed limit, {nothing}")),
        Ok(Err(e)) => Err(format!("the app did not answer ({e}), {nothing}")),
        Err(_) => Err(format!("this chat could not reach the app, {nothing}")),
    }
}

/// The sentence a sandboxed chat is refused with when a command would change a workspace other
/// than its own, or every workspace (#1408): nothing is changed, and `instead`, the way round,
/// is named.
pub fn not_its_workspace(what: &str, instead: &str) -> String {
    format!(
        "a sandboxed chat changes only its own workspace's memory and todos, and {what}: \
         nothing was changed. {instead}, or ask the person to do it from the window"
    )
}

/// The sentence `purlis workspace remove <name>` is refused with in a chat the app started under
/// the sandbox (#1408, D-qw301-1): removing a workspace deletes what other chats and the person
/// rely on, which no write purlis makes for a chat reaches. Refused before anything is deleted,
/// naming the way round.
pub fn no_removal_in_a_sandbox(name: &str) -> String {
    format!(
        "a sandboxed chat does not remove a workspace: removing '{}' deletes its memory, its \
         todos and its clones, which is the person's to decide, so nothing was removed. Ask the \
         person to remove it from the window",
        purlis_core::shown::short(name)
    )
}

/// **What `purlis workspace remove <name>` says where the removal is held** (qw356): the
/// refusal was a dead end, so the name is handed to the app that started this chat, which asks
/// the person in its window and answers with the sentence that says so
/// ([`purlis_core::removalask`]). Nothing is removed here either way, and nothing but the name
/// is handed over: a `--force` has no field to travel in.
///
/// Where no app asks the person (no chat, no connection, an app older than the ask, or one
/// that did not answer), it is [`no_removal_in_a_sandbox`], as before.
pub fn removal_asked_of_the_person(name: &str) -> String {
    let asked = asked(
        |chat| {
            Ask::Removal(hookwire::RemovalAsk {
                chat,
                removing: hookwire::Removing::Workspace {
                    name: name.to_owned(),
                },
            })
        },
        A_WRITE_TAKES_AT_MOST,
    );
    match asked {
        Ok(Ok(Answer::No { why })) if why != hookwire::NOTHING_ANSWERS => why,
        _ => no_removal_in_a_sandbox(name),
    }
}

/// The sentence `purlis persona remove <name>` is refused with in a chat the app started under
/// the sandbox (#1408): as [`no_removal_in_a_sandbox`], for a persona.
pub fn no_persona_removal_in_a_sandbox(name: &str) -> String {
    format!(
        "a sandboxed chat does not remove a persona: removing '{}' deletes its definition, its \
         memory and its curation actions, which is the person's to decide, so nothing was \
         removed. Ask the person to remove it from the window",
        purlis_core::shown::short(name)
    )
}

/// The sentence `purlis worktree remove <repo> <piece>` is refused with in a chat the app
/// started under the sandbox where no app took the removal (#1408, #3879): the folder's files
/// go before git's record of it, which a sandboxed chat cannot finish.
pub fn no_piece_removal_in_a_sandbox(repo: &str, piece: &str) -> String {
    let (repo, piece) = (
        purlis_core::shown::short(repo),
        purlis_core::shown::short(piece),
    );
    format!(
        "a sandboxed chat does not remove a branch folder: removing '{repo} · {piece}' deletes \
         its files and git's record of it, which a sandboxed chat cannot finish, so nothing was \
         removed. Ask the person to remove it from the window: Remove folder {piece} in {repo}, \
         in the palette"
    )
}

/// The sentence `purlis worktree remove --force` is refused with in any chat the app started
/// (#3879): forcing discards the uncommitted changes and unique commits a plain removal names,
/// which is the person's to decide. The plain removal the app runs for the chat is named too.
pub fn no_forced_piece_removal_in_a_chat(repo: &str, piece: &str) -> String {
    let (repo, piece) = (
        purlis_core::shown::short(repo),
        purlis_core::shown::short(piece),
    );
    format!(
        "a chat does not force a branch folder's removal: --force discards the work in \
         '{repo} · {piece}' that a plain removal would name, which is the person's to decide, \
         so nothing was removed. Without --force the app removes it where nothing would be \
         lost; otherwise ask the person to remove it from the window: Remove folder {piece} in \
         {repo}, in the palette"
    )
}

/// Why `purlis workspace create <name>` is refused whole in a sandboxed chat, before anything is
/// made, or `None` (qw387). The app makes the workspace and clones into it for the chat, but
/// `--live` writes the project's ignore file, which no chat writes, and `--use` (with its
/// `--force`) moves the chat into another workspace, which a sandboxed chat does not do.
pub fn create_refused(name: &str, live: bool, uses: bool) -> Option<String> {
    let name = purlis_core::shown::short(name);
    if live {
        return Some(format!(
            "--live changes the project's ignore file, which the app does not write for a \
             chat, so nothing was created. Create '{name}' without --live, then ask the person \
             to run `purlis workspace live {name}` in their own terminal"
        ));
    }
    uses.then(|| {
        format!(
            "--use would move this chat into '{name}', and a sandboxed chat stays in the place it \
             was started in, so nothing was created. Create it without --use, then name it with \
             -w {name} on each command, or hand the work to a chat there with `purlis handoff \
             {name} \"<brief>\"`"
        )
    })
}

/// Whether a command that deletes `target`, a folder of the project at `root`, is held in this
/// process (#1408, D-qw301-1, D-qw322-1): in a chat the app started under the sandbox, or
/// wherever the sandbox itself holds this process ([`purlis_core::sandbox::held`]), at the
/// project's manifests or at a file below `target` the delete would have to remove. Refused
/// before the guard and the delete run.
pub fn removal_held(root: &Path, target: Option<&Path>) -> bool {
    removal_held_by(sandboxed(), || {
        purlis_core::sandbox::held::writes_held_at(root)
            || target.is_some_and(purlis_core::sandbox::held::deletion_held_within)
    })
}

/// [`removal_held`], from whether the chat `says` it is sandboxed and what the sandbox itself
/// answers (`held`).
fn removal_held_by(says: bool, held: impl FnOnce() -> bool) -> bool {
    says || held()
}

/// Whether this command runs in a chat the app started under a sandbox, where a write outside
/// the chat's own stores is refused rather than tried (#1408).
pub fn sandboxed() -> bool {
    purlis_core::sandbox::chat_is_sandboxed()
}

/// **What a command that rewrites a workspace's `workspace.json` says in a chat the app
/// started under a sandbox** (#2046): nothing, outside one. A chat's sandbox writes no
/// workspace's manifest; the app writes the ones a chat's asks need (a clone's record, a new
/// workspace), and the rest are the person's, from a terminal or the window.
pub fn manifest_refused(command: &str) -> Option<String> {
    sandboxed().then(|| {
        format!(
            "`purlis {command}` rewrites a workspace's workspace.json, which a sandboxed chat \
             does not write, so nothing was changed. Ask the person to run it in a terminal"
        )
    })
}

/// How a workspace verb that writes takes the `-w` it was given, in a chat (#1408).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Named {
    /// No `-w`, or the workspace the chat works in spelled out: the app's to make, as it
    /// always was with no `-w`.
    Own,
    /// Another workspace, written here as it always was, where the chat's sandbox decides:
    /// outside a sandboxed chat, or from a chat at the project root, whose sandbox writes every
    /// workspace's ordinary files.
    Here,
    /// Another workspace, from a chat the app started under the sandbox: refused with this
    /// sentence, and nothing is written.
    Refused(String),
}

/// How a workspace verb that writes takes `named`, its `-w`, in a chat the app started in
/// workspace `own` (`None`: at the project root, or no chat), and whether it runs `sandboxed`
/// (#1408). `-w <its own>` is taken as no `-w`; another workspace is refused in a sandboxed
/// chat that works in a workspace, naming the way round, and written here otherwise. The
/// sandbox stays the boundary; this is wording.
pub fn named(named: Option<&str>, own: Option<&str>, sandboxed: bool) -> Named {
    let Some(named) = named else {
        return Named::Own;
    };
    let Some(own) = own else {
        return Named::Here;
    };
    if own == named {
        return Named::Own;
    }
    if !sandboxed {
        return Named::Here;
    }
    Named::Refused(not_its_workspace(
        &format!(
            "'{}' is not the one it works in",
            purlis_core::shown::short(named)
        ),
        &format!("Leave out -w, or name its own with -w {own}"),
    ))
}

/// [`named`] for this command: `-w` as given; the workspace the app pinned this chat to as it
/// started it (`$PURLIS_WORKSPACE`, the app's own record of where the chat works), never a
/// pointer or the folder the command runs in; and whether the app started it under the
/// sandbox.
pub fn named_here(here: &crate::Here, workspace: Option<&str>) -> Named {
    let own = crate::session::chat_number()
        .and(here.workspace_env.as_deref())
        .filter(|own| !own.is_empty());
    named(workspace, own, sandboxed())
}

#[cfg(test)]
mod tests;
