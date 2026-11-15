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

/// [`isolation`], signing the commits it makes for a chat with this device's key where the
/// person set one up (#5480, `purlis signing setup`). Read from the keyring for each ask, so a
/// key set up or removed since is the one used. The key stays in this process: git is handed
/// none of it.
///
/// A test build signs with no key: it never reads the person's keyring.
fn signing_isolation(held: &Held) -> purlis_core::worktree::git::Isolated {
    let isolation = isolation();
    #[cfg(not(test))]
    {
        // This machine's config home, as `purlis signing setup` finds it.
        let _ = held;
        let Some(config) = purlis_core::machine::config_root() else {
            return isolation;
        };
        let keyring = purlis_core::commitsign::this_builds_keyring(&config);
        match purlis_core::commitsign::kept(&config, &*keyring) {
            // Signed as the email the person chose for the forge account of the clone's host,
            // where they chose one (#5631, #5788): one that forge verifies, so the signature
            // shows as Verified there.
            Ok(Some(signer)) => purlis_core::signingsetup::commit_emails_by_host(&config)
                .into_iter()
                .fold(
                    isolation.signing(Some(signer)),
                    |isolation, (host, email)| isolation.as_email_on(&host, &email),
                ),
            Ok(None) => isolation,
            Err(why) => {
                tracing::warn!("purlis: a chat's commits are made unsigned: {why}");
                isolation
            }
        }
    }
    #[cfg(test)]
    {
        let _ = held;
        isolation
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
    let confines = held.chats().confines_of(chat);
    Ok(Asker {
        chat,
        cwd: open.cwd,
        persona: open.persona,
        harnessed: open.harness.is_some(),
        // Started without the sandbox by the record of how its run started (#2692): a run with
        // no record is held as sandboxed here, where a chat with no sandbox is let reach more.
        unsandboxed: held.chats().started_unsandboxed(chat),
        // The project's git is held whole only where this window compiled the chat's sandbox
        // and the chat runs in it.
        roots_git_held: held.chats().runs_sandboxed(chat)
            && confines.as_ref().and_then(|c| c.roots_git.as_deref())
                == Some(held.root().join(".git").as_path()),
        unreadable: confines
            .map(|c| c.denied)
            .unwrap_or_default()
            .into_iter()
            .filter(|denial| denial.access == purlis_core::sandbox::Access::ReadWrite)
            .collect(),
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

/// **A chat on a pool's runner does its git work on its runner**: the app makes no clone,
/// branch folder or commit here for it, as it writes nothing here for it unasked
/// (`crate::brokered`). By the link the line came over, else the router's record of the chat on
/// a pool. The refusal for chat `chat`, where it runs on one.
fn on_a_runner(held: &Held, chat: u32) -> Option<Answer> {
    let pool = crate::runnerdispatches::heard_from_a_pool(held, chat)?;
    Some(Answer::No {
        why: format!(
            "this chat runs on pool {pool}'s runner, and its git work is done there: purlis \
             makes no clone, branch folder or commit on the person's machine for it. Run git on \
             the runner; its branches come back here when its work is fetched"
        ),
    })
}

/// Commits what `ask` names in the branch folder its chat stands in (#1055), or says why not.
/// The folder is this app's record of the chat; the ask names a message and paths.
pub fn commit(held: &Held, ask: &CommitAsk) -> Answer {
    if let Some(no) = on_a_runner(held, ask.chat) {
        return no;
    }
    let asker = match asker(held, ask.chat) {
        Ok(asker) => asker,
        Err(no) => return no,
    };
    let what = match &ask.stage {
        Stage::Tracked => "every tracked change".to_owned(),
        Stage::Paths(paths) => purlis_core::counts::counted(paths.len(), "path", "paths"),
    };
    // The person's name and email, read before the commit runs; nothing else of their
    // global or system git config reaches it. And the key the app signs it with, where there
    // is one (#5480).
    let identity = signing_isolation(held);
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
    if let Some(no) = on_a_runner(held, ask.chat) {
        return no;
    }
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
        GitWork::WorktreePrune { repo, .. } => match repo {
            Some(repo) => format!("worktree prune {repo}"),
            None => "worktree prune".to_owned(),
        },
        GitWork::RootUpdate {} => "worktree update at the project root".to_owned(),
        GitWork::RootSave { .. } => "save the project".to_owned(),
        // Its own git's claim on a branch folder, asked by the chat's Bash guard (#3931).
        GitWork::WorktreeClaim { repo, piece, act } => {
            format!("hold {repo} {piece} for its own git's {act:?}")
        }
    };
    let isolation = signing_isolation(held);
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
    let root_save = matches!(ask.work, GitWork::RootSave { .. });
    // The root's update is the window's own Update (#3831), fetched as the window fetches it:
    // its sign-in, offered to the remote recorded for the project alone. A chat's save of the
    // project pushes as the window's automatic save does, held to that same remote.
    let answered = if root_update || root_save {
        crate::forgeaccounts::window_git(run)
    } else {
        run()
    };
    // The tree, or what is saved of it, moved under every reader, as after the window's own
    // Update and Save.
    if (root_update || root_save) && matches!(answered.answer, Answer::Said { code: 0, .. }) {
        held.tree_moved();
    }
    let sent = pushed_head(&ask.work, &answered);
    let answer = told(held, ask.chat, answered);
    // A push that went is the chat's own: a merge ask at the commit it sent is not flagged
    // (T1-41, R-124, Q277).
    if let Some((repo, piece, head)) = sent {
        crate::mergeholds::pushed(held, ask.chat, &ask.workspace, repo, piece, &head);
    }
    // A push refused for unsigned commits, or one that went, changes the one Ask for the lanes
    // waiting on signing (#5633).
    if matches!(ask.work, GitWork::WorktreePush { .. }) {
        held.hooks().asks_teller()();
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

/// **The push `answered` says went, of a branch folder's `work`** (T1-41, R-124, Q277): its repo,
/// its folder and the commit the push sent, as the core's push says it. Nothing for any other
/// work, a push that did not go, or one that names no commit sent: no `HEAD` is read for it.
pub(crate) fn pushed_head<'a>(
    work: &'a GitWork,
    answered: &purlis_core::gitbroker::Answered,
) -> Option<(&'a str, &'a str, String)> {
    let GitWork::WorktreePush { repo, piece, .. } = work else {
        return None;
    };
    if !matches!(answered.answer, Answer::Said { code: 0, .. }) {
        return None;
    }
    let head = answered.pushed.clone()?;
    Some((repo.as_str(), piece.as_str(), head))
}

/// **What the person is told of a chat's git ask** (#3877), and the answer the chat reads: a
/// root update left for a reason only the person settles (a file the chat's sandbox denies, a
/// conflict, a project not put back) is a needs-you item on the chat, naming the window's
/// Update. The chat has the reason in its answer. Nothing is raised for any other refusal,
/// which is the chat's to retry.
fn told(held: &Held, chat: u32, answered: purlis_core::gitbroker::Answered) -> Answer {
    if let Some(need) = answered.needs {
        held.needs_the_person(chat, need);
    }
    answered.answer
}

#[cfg(test)]
mod tests;
