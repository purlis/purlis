// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A clone or a worktree a chat asks the app to make for it (#1335, ADR 0067 §2): the app's
//! record of the chat, handed to `purlis_core::gitbroker`, which checks the ask against it and
//! runs the same core functions the terminal's commands run.
//!
//! And a commit in the branch folder a sandboxed chat stands in (#1055), handed to
//! `purlis_core::gitbroker::commit`; and a finished branch folder's removal (#3879), never
//! forced, which the core runs only where nothing would be lost.
//!
//! Answered on the listener's thread for this one connection, so a slow clone holds no other
//! chat's hooks or asks (D-6).

use purlis_core::gitbroker::Asker;
use purlis_core::hookwire::{Answer, CommitAsk, GitAsk, GitWork, Stage};

use crate::planes::Held;

/// The configuration git reads where the app runs it for a chat, or in a repository a chat
/// worked in: no global or system git config, only the operator's identity, read from their
/// global config before the call runs (D-1335-7).
///
/// A test build reads no identity either: a test never reads the developer's own git config.
/// The one call made under it that writes a commit is a chat's brokered commit (#1055), which
/// no test of this crate makes.
pub(crate) fn isolation() -> purlis_core::worktree::git::Isolated {
    #[cfg(not(test))]
    {
        purlis_core::worktree::git::Isolated::operators()
    }
    #[cfg(test)]
    {
        purlis_core::worktree::git::Isolated::default()
    }
}

/// The app's own record of chat `chat`, or the answer where it has none open.
fn asker(held: &Held, chat: u32) -> Result<Asker, Answer> {
    let Some(open) = held
        .chats()
        .open_now()
        .into_iter()
        .find(|open| open.session == chat)
    else {
        return Err(Answer::No {
            why: format!("chat {chat} is not one this app has open"),
        });
    };
    Ok(Asker {
        chat,
        cwd: open.cwd,
        persona: open.persona,
        harnessed: open.harness.is_some(),
        // Started without the sandbox by the record of how its run started (#2692): a run with
        // no record is held as sandboxed here, where a chat with no sandbox is let reach more.
        unsandboxed: held.chats().started_unsandboxed(chat),
        config: held.config().map(std::path::Path::to_path_buf),
        id: held.chats().id_of(chat),
        live: Some(live_chats(held)),
    })
}

/// **The chats a branch folder's claim is judged against** (#3901): each open chat whose program
/// still runs, and each that has not come back since the launch (restoring, or waiting to start
/// again), whose liveness cannot be told yet. A chat in neither has ended, and its claim with it.
pub(crate) fn live_chats(held: &Held) -> Vec<purlis_core::piececlaim::Live> {
    let mut live: Vec<purlis_core::piececlaim::Live> = held
        .chats()
        .open_now()
        .into_iter()
        .filter(|open| crate::handoff::still_working(held, open.session))
        .filter_map(|open| {
            Some(purlis_core::piececlaim::Live {
                id: held.chats().id_of(open.session)?,
                chat: Some(open.session),
                back: true,
            })
        })
        .collect();
    live.extend(held.chats().not_back().into_iter().map(|(id, chat)| {
        purlis_core::piececlaim::Live {
            id,
            chat,
            back: false,
        }
    }));
    live
}

/// Commits what `ask` names in the branch folder its chat stands in (#1055), or says why not.
/// The folder is this app's record of the chat; the ask names a message and paths.
pub fn commit(held: &Held, ask: &CommitAsk) -> Answer {
    let asker = match asker(held, ask.chat) {
        Ok(asker) => asker,
        Err(no) => return no,
    };
    let what = match &ask.stage {
        Stage::Tracked => "every tracked change".to_owned(),
        Stage::Paths(paths) => purlis_core::counts::counted(paths.len(), "path", "paths"),
    };
    // The person's name and email, read before the commit runs; nothing else of their
    // global or system git config reaches it.
    let identity = isolation();
    let (answer, refused) =
        purlis_core::gitbroker::commit::answer_telling(held.root(), &asker, ask, &identity);
    // A commit purlis's scan refused is a needs-you item, as a chat's own is (#1476): the chat
    // has the refusal in front of it, and the person hears that a commit was stopped.
    if let Some(what) = refused {
        held.commit_refused(ask.chat, &what);
    }
    let folder = asker.cwd.as_deref().map_or_else(
        || "no recorded folder".to_owned(),
        |cwd| cwd.display().to_string(),
    );
    match &answer {
        Answer::Said { .. } => tracing::info!(
            "purlis: chat {} asked the app to commit {what} in {folder}; committed",
            ask.chat
        ),
        Answer::No { why } => tracing::info!(
            "purlis: chat {} asked the app to commit {what} in {folder}; refused: {why}",
            ask.chat
        ),
        _ => {}
    }
    answer
}

/// Runs `ask` for its chat, or says why not.
pub fn answer(held: &Held, ask: &GitAsk) -> Answer {
    let asker = match asker(held, ask.chat) {
        Ok(asker) => asker,
        Err(no) => return no,
    };
    // A workspace made for a chat is held to the pace its other writes are (#1408).
    if matches!(ask.work, GitWork::Create { .. })
        && let Err(why) = held.brokered().allow(ask.chat, std::time::Instant::now())
    {
        return Answer::No { why };
    }
    let what = match &ask.work {
        GitWork::Create { repos, .. } => format!("workspace create with {}", repos.join(" ")),
        GitWork::Clone { repos } => format!("clone {}", repos.join(" ")),
        GitWork::WorktreeAdd { repo, piece, .. } | GitWork::WorktreeCut { repo, piece, .. } => {
            format!("worktree add {repo} {piece}")
        }
        GitWork::WorktreeUpdate { repo, piece, .. } => format!("worktree update {repo} {piece}"),
        GitWork::WorktreePush { repo, piece, .. } => format!("worktree push {repo} {piece}"),
        GitWork::WorktreeRemove { repo, piece, .. } => format!("worktree remove {repo} {piece}"),
        GitWork::CloneUpdate { repo, .. } => format!("worktree update {repo}"),
        GitWork::RootUpdate {} => "worktree update at the project root".to_owned(),
    };
    let isolation = isolation();
    let run = || {
        purlis_core::gitbroker::answer_telling(
            held.root(),
            &asker,
            ask,
            &isolation,
            chrono::Utc::now(),
        )
    };
    let root_update = matches!(ask.work, GitWork::RootUpdate {});
    // The root's update is the window's own Update (#3831), fetched as the window fetches it:
    // its sign-in, offered to the remote recorded for the project alone.
    let (answer, needs_the_person) = if root_update {
        crate::forgeaccounts::window_git(run)
    } else {
        run()
    };
    // The tree moved under every reader, as after the window's own Update.
    if root_update && matches!(answer, Answer::Said { code: 0, .. }) {
        held.tree_moved();
    }
    // **A root update left for a reason only the person settles asks the person** (#3877): a
    // file the chat's sandbox denies, a conflict, a project not put back. The chat has the
    // reason in its answer; the item names the window's Update. Any other refusal is the
    // chat's to retry, and nobody else hears of it.
    if needs_the_person {
        held.needs_the_person(ask.chat, purlis_core::state::Need::RootUpdateRefused);
    }
    match &answer {
        Answer::Said { code, .. } => tracing::info!(
            "purlis: chat {} asked the app to {what} in workspace '{}'; it ran, exit {code}",
            ask.chat,
            ask.workspace
        ),
        Answer::No { why } => tracing::info!(
            "purlis: chat {} asked the app to {what} in workspace '{}'; refused: {why}",
            ask.chat,
            ask.workspace
        ),
        _ => {}
    }
    answer
}

#[cfg(test)]
mod tests;
