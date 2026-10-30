// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter worktree` (alias `wt`) — the command line over [`purlis_core::piececmd`].
//!
//! Kept out of `main.rs` on purpose: that file is where every command's variant meets, and
//! this one adds a single line there.

use clap::Subcommand;
use purlis_core::piececmd;
use purlis_core::pieces::{Declaration, Who};

use crate::Here;
use purlis_core::hookwire::GitWork;

#[derive(Subcommand)]
pub enum WorktreeCommand {
    /// Cut a piece: a worktree of <repo> on a new branch off its base, with the plane's layer
    /// wired into it and its claim recorded. The base is the branch the remote names its
    /// default (`main`, most often), brought up to the remote where nothing has it checked out.
    /// Exit 2 when the piece is already claimed, so a worker that lost a race knows to take the
    /// next name.
    Add {
        /// A repo cloned into the workspace.
        repo: String,
        /// The piece's name, which is also its directory and, by default, its branch.
        piece: String,
        /// Name the new branch something other than the piece.
        #[arg(long)]
        branch: Option<String>,
        /// The branch of the clone to cut from, recorded as the piece's base; `HEAD` for
        /// whatever the clone is on (default: the remote's default branch, else HEAD).
        #[arg(long, value_name = "BRANCH")]
        base: Option<String>,
        /// The workspace (default: the active one).
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
    },
    /// Commit in the branch folder you are standing in, from a sandboxed chat: the app that
    /// started the chat stages what you name and commits it on that folder's branch. It only
    /// commits: it never amends, resets, rebases, merges or pushes, and the repository's own
    /// hooks are not run. A chat that is not sandboxed uses `git commit`.
    Commit {
        /// The commit message. One that starts with `-` is the message too (#3927).
        #[arg(
            short = 'm',
            long = "message",
            value_name = "MESSAGE",
            allow_hyphen_values = true
        )]
        message: Option<String>,
        /// Stage every change to a file git already tracks. A new file must be named.
        #[arg(short = 'a', long = "all")]
        all: bool,
        /// Never done. Taken only so that it is refused in a sentence.
        #[arg(long, hide = true)]
        amend: bool,
        /// Paths to stage, relative to where you stand in the branch folder (or with `env -C
        /// <branch folder>`), or from a folder above it as `.worktrees/<repo>/<piece>/<path>`.
        #[arg(value_name = "PATH")]
        paths: Vec<String>,
    },
    /// Bring the branch folder you stand in up to its base: fetch, bring the base up to its
    /// remote, and rebase onto it. Standing in a workspace's clone, bring the clone's branch up
    /// to the remote branch it follows: fetch, then fast-forward or rebase. At the project root,
    /// run the window's Update, which moves the project whole or not at all. Stops on a conflict
    /// and names the files: resolve them, then `--continue`, or `--abort` to put the branch
    /// back. Refused with uncommitted changes, or where it would overwrite a file on disk that
    /// is not committed. In a chat the app runs it.
    Update {
        /// The branch to rebase onto (default: the one the piece was cut from). A branch
        /// folder's only.
        #[arg(long, value_name = "BRANCH", conflicts_with_all = ["go_on", "abort"])]
        base: Option<String>,
        /// Stage the files the stopped rebase named, once resolved, and go on.
        #[arg(long = "continue", conflicts_with = "abort")]
        go_on: bool,
        /// Put the branch back where it was before the rebase.
        #[arg(long)]
        abort: bool,
    },
    /// Push the branch of the folder you stand in to its remote, under its own name, after
    /// purlis's scan of what it would publish. In a chat the app runs it.
    Push {
        /// Push over the remote's copy of the branch, only right after `update` rewrote it, and
        /// only if the remote still has what was last fetched.
        #[arg(long)]
        force_with_lease: bool,
    },
    /// Declare the piece you are standing in finished. Run from inside it.
    Done,
    /// Declare the piece you are standing in given up, and why. Run from inside it.
    Abandon {
        /// Why you stopped (required) — what whoever picks this up reads first. Taken as
        /// optional only so that a missing one is refused in a sentence, with exit 1.
        #[arg(value_name = "REASON")]
        reason: Option<String>,
    },
    /// The workspace's pieces as git has them, each with what it declared or how long it has
    /// been silent.
    List {
        /// Only this repo's pieces.
        repo: Option<String>,
        /// The workspace (default: the active one).
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
    },
    /// What happened to the workspace's pieces, removed ones included — read from the log.
    History {
        /// Only this repo.
        repo: Option<String>,
        /// Only this piece.
        piece: Option<String>,
        /// The workspace (default: the active one).
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
    },
    /// Remove a piece with `git worktree remove`. Refused, naming what would be lost, while
    /// it holds uncommitted changes or commits no other ref reaches. A branch folder purlis
    /// cut for a task is discarded from that task's Changes tab instead.
    Remove {
        repo: String,
        piece: String,
        /// Discard the uncommitted changes and unique commits the refusal named.
        #[arg(long)]
        force: bool,
        /// Also delete the piece's branch (`git branch -d`, or `-D` with --force).
        #[arg(long)]
        delete_branch: bool,
        /// The workspace (default: the active one).
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
    },
}

pub fn run(here: &Here, command: WorktreeCommand) -> Result<u8, String> {
    let root = here.plane.root().to_path_buf();
    let now = chrono::Utc::now();
    // In a chat, by its stable id too (#3901): its `done` or `abandon` hands over a branch
    // folder it holds, and another chat's lets nothing go.
    let who = Who::here(
        purlis_core::machine::config_root_if_there().as_deref(),
        here.ids.session.clone(),
        here.active_persona(None),
    )
    .for_chat(purlis_core::envvar::var(purlis_core::hookwire::CHAT_ID_ENV));
    let mut say = crate::speak;
    let code = match command {
        WorktreeCommand::Add {
            repo,
            piece,
            branch,
            base,
            workspace,
        } => {
            let ws = here.active_workspace(workspace.as_deref())?;
            // In a chat the app started, the app cuts it (#1335): a sandboxed chat may not
            // write the worktree's git files or the editor settings it checks out.
            let asked = crate::gitask::told(
                crate::gitask::forwarded(
                    &ws,
                    purlis_core::hookwire::GitWork::WorktreeCut {
                        repo: repo.clone(),
                        piece: piece.clone(),
                        branch: branch.clone(),
                        base: base.clone(),
                    },
                    crate::gitask::A_WORKTREE_TAKES_AT_MOST,
                ),
                &mut say,
            );
            match asked {
                Some(code) => code,
                None => piececmd::add(
                    &root,
                    &ws,
                    &repo,
                    &piece,
                    branch.as_deref(),
                    base.as_deref(),
                    None,
                    None,
                    &who,
                    now,
                    &mut say,
                ),
            }
        }
        WorktreeCommand::Commit {
            message,
            all,
            amend,
            paths,
        } => crate::gitask::commit(
            &crate::gitask::RunIn {
                root: &root,
                cwd: &here.cwd,
            },
            message,
            all,
            amend,
            paths,
            &mut say,
        ),
        WorktreeCommand::Update { base, go_on, abort } => {
            let step = match (go_on, abort) {
                (true, _) => purlis_core::hookwire::UpdateStep::Continue,
                (_, true) => purlis_core::hookwire::UpdateStep::Abort,
                _ => purlis_core::hookwire::UpdateStep::Start { base },
            };
            update(here, &step, &who, &mut say)
        }
        WorktreeCommand::Push { force_with_lease } => standing_in(here, &mut say, |at, say| {
            let work = purlis_core::hookwire::GitWork::WorktreePush {
                repo: at.repo.clone(),
                piece: at.piece.clone(),
                lease: force_with_lease,
            };
            ran_by_the_app(at, work, say).unwrap_or_else(|| {
                if let Err(why) =
                    no_app_to_tell(&root, at, &who, purlis_core::piececlaim::Act::Push)
                {
                    say(purlis_core::repocmd::Say::Fail(why.into()));
                    return 1;
                }
                purlis_core::pieceupdate::push(
                    &piece_of(&root, at),
                    force_with_lease,
                    &purlis_core::pieceupdate::Reach::default(),
                    say,
                )
            })
        }),
        WorktreeCommand::Done => {
            piececmd::declare(&root, &here.cwd, Declaration::Done, &who, now, &mut say)
        }
        WorktreeCommand::Abandon { reason } => piececmd::declare(
            &root,
            &here.cwd,
            Declaration::Abandoned {
                reason: reason.as_deref().unwrap_or_default(),
            },
            &who,
            now,
            &mut say,
        ),
        WorktreeCommand::List { repo, workspace } => piececmd::list(
            &root,
            &here.active_workspace(workspace.as_deref())?,
            repo.as_deref(),
            now,
            &mut say,
        ),
        WorktreeCommand::History {
            repo,
            piece,
            workspace,
        } => piececmd::history(
            &root,
            &here.active_workspace(workspace.as_deref())?,
            repo.as_deref(),
            piece.as_deref(),
            &mut say,
        ),
        WorktreeCommand::Remove {
            repo,
            piece,
            force,
            delete_branch,
            workspace,
        } => {
            // Never from a chat the app started under the sandbox (#1408, D-qw322-1): refused
            // before git is run, wherever the chat stands.
            let ws = here.active_workspace(workspace.as_deref());
            let target = ws
                .as_deref()
                .ok()
                .and_then(|ws| purlis_core::worktree::path_for(&root, ws, &repo, &piece).ok());
            if crate::brokered::removal_held(&root, target.as_deref()) {
                say(purlis_core::repocmd::Say::Fail(
                    crate::brokered::no_piece_removal_in_a_sandbox(&repo, &piece).into(),
                ));
                return Ok(1);
            }
            piececmd::remove(&root, &ws?, &repo, &piece, force, delete_branch, &mut say)
        }
    };
    Ok(code)
}

/// The folder `purlis worktree update` stands in (#3831): a branch folder, a workspace's clone
/// itself, or the project root.
enum Updated {
    Piece(purlis_core::worktree::Located),
    Clone { ws: String, repo: String },
    Root,
}

impl Updated {
    fn of(here: &Here) -> Option<Self> {
        let root = here.plane.root();
        if let Some(at) = purlis_core::worktree::locate(root, &here.cwd) {
            return Some(Self::Piece(at));
        }
        if let Some((ws, repo)) = purlis_core::worktree::locate_clone(root, &here.cwd) {
            return Some(Self::Clone { ws, repo });
        }
        let real = |path: &std::path::Path| std::fs::canonicalize(path).ok();
        (real(root).is_some() && real(root) == real(&here.cwd)).then_some(Self::Root)
    }

    /// The ask the app is handed for `step`, and the workspace it is in.
    fn work(&self, step: &purlis_core::hookwire::UpdateStep) -> (&str, GitWork) {
        match self {
            Self::Root => ("", GitWork::RootUpdate {}),
            Self::Piece(at) => (
                &at.workspace,
                GitWork::WorktreeUpdate {
                    repo: at.repo.clone(),
                    piece: at.piece.clone(),
                    step: step.clone(),
                },
            ),
            Self::Clone { ws, repo } => (
                ws,
                GitWork::CloneUpdate {
                    repo: repo.clone(),
                    step: step.clone(),
                },
            ),
        }
    }
}

/// What `update` says where it stands in no folder it updates.
const NOT_IN_A_FOLDER: &str = "not inside a branch folder or a workspace's clone, nor at the \
     project root: run `purlis worktree update` from the one to bring up to date (`purlis \
     worktree list` shows the branch folders)";

/// What `update` says of a step the project root does not take: its update moves the project
/// whole or puts it back, so nothing is left to go on with, and it has no base.
const AT_THE_ROOT: &str = "at the project root, `purlis worktree update` takes no `--base`, \
     `--continue` or `--abort`: the project is either updated whole or left exactly as it was";

/// Whether `step` is one the project root takes: a plain start.
fn root_takes(step: &purlis_core::hookwire::UpdateStep) -> bool {
    matches!(
        step,
        purlis_core::hookwire::UpdateStep::Start { base: None }
    )
}

/// `purlis worktree update`: in a chat the app started, handed to the app; elsewhere, run here.
fn update(
    here: &Here,
    step: &purlis_core::hookwire::UpdateStep,
    who: &Who,
    say: &mut dyn FnMut(purlis_core::repocmd::Say),
) -> u8 {
    let Some(updated) = Updated::of(here) else {
        say(purlis_core::repocmd::Say::Fail(NOT_IN_A_FOLDER.into()));
        return 1;
    };
    if matches!(updated, Updated::Root) && !root_takes(step) {
        say(purlis_core::repocmd::Say::Fail(AT_THE_ROOT.into()));
        return 1;
    }
    let (ws, work) = updated.work(step);
    let asked = crate::gitask::told(
        crate::gitask::forwarded(ws, work, crate::gitask::A_PIECE_UPDATE_TAKES_AT_MOST),
        say,
    );
    if let Some(code) = asked {
        return code;
    }
    let root = here.plane.root();
    let reach = purlis_core::pieceupdate::Reach::default();
    match &updated {
        Updated::Piece(at) => {
            if let Err(why) = no_app_to_tell(root, at, who, purlis_core::piececlaim::Act::Update) {
                say(purlis_core::repocmd::Say::Fail(why.into()));
                return 1;
            }
            purlis_core::pieceupdate::update(&piece_of(root, at), step, &reach, say)
        }
        Updated::Clone { ws, repo } => purlis_core::pieceupdate::update_clone(
            &purlis_core::pieceupdate::CloneAt { root, ws, repo },
            step,
            &reach,
            say,
        ),
        // From a terminal, the person's own: the window's Update, as they would press it.
        Updated::Root => {
            let updated = purlis_core::planegit::update(root, say);
            let request = purlis_core::forge::request_words_of(root, root).request_noun();
            let said = updated.said(request);
            if matches!(updated, purlis_core::planegit::Updated::Left(_)) {
                say(purlis_core::repocmd::Say::Fail(said.into()));
                1
            } else {
                say(purlis_core::repocmd::Say::Done(said.into()));
                0
            }
        }
    }
}

/// The `worktree_update` tool (#3831): `purlis worktree update` where the chat stands, handed
/// to the app that started it, and answered with what it said. **Never run here**: the MCP
/// server runs outside the chat's sandbox, where the app's judging of the move would not be,
/// so with no app to take it nothing runs.
pub fn update_by_the_app(
    here: &Here,
    step: &purlis_core::hookwire::UpdateStep,
) -> Result<String, String> {
    use crate::gitask::Forwarded;
    use purlis_core::repocmd::Say;
    let updated = Updated::of(here).ok_or_else(|| NOT_IN_A_FOLDER.to_owned())?;
    if matches!(updated, Updated::Root) && !root_takes(step) {
        return Err(AT_THE_ROOT.to_owned());
    }
    let (ws, work) = updated.work(step);
    let (lines, code) =
        match crate::gitask::forwarded(ws, work, crate::gitask::A_PIECE_UPDATE_TAKES_AT_MOST) {
            Forwarded::Said { lines, code } => (lines, code),
            Forwarded::Refused(why) | Forwarded::Unsure(why) => return Err(why),
            Forwarded::NotTaken => {
                return Err(
                    "no app took the update, so nothing was run: run `purlis worktree \
                            update` in the chat"
                        .to_owned(),
                );
            }
        };
    let text = lines
        .iter()
        .map(|line| match line {
            Say::Info(t) | Say::Done(t) | Say::Warn(t) | Say::Fail(t) | Say::Plain(t) => {
                t.to_string()
            }
            Say::Out(t) => t.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    if code == 0 { Ok(text) } else { Err(text) }
}

/// Runs `then` for the branch folder this command stands in, or says there is none.
fn standing_in(
    here: &Here,
    say: &mut dyn FnMut(purlis_core::repocmd::Say),
    then: impl FnOnce(&purlis_core::worktree::Located, &mut dyn FnMut(purlis_core::repocmd::Say)) -> u8,
) -> u8 {
    match purlis_core::worktree::locate(here.plane.root(), &here.cwd) {
        Some(at) => then(&at, say),
        None => {
            say(purlis_core::repocmd::Say::Fail(
                "not inside a branch folder: run it from inside the piece (`purlis worktree list` \
                 shows where they are)"
                    .into(),
            ));
            1
        }
    }
}

/// **In a chat whose app took no ask**, a branch folder's claim is still held to (#3901): with
/// no app to say whether its holder lives, a folder another chat holds is refused, and the
/// holder writes on. Outside a chat it is the person's own terminal, which hands any folder
/// over, and nothing is asked.
fn no_app_to_tell(
    root: &std::path::Path,
    at: &purlis_core::worktree::Located,
    who: &Who,
    act: purlis_core::piececlaim::Act,
) -> Result<(), String> {
    if crate::session::chat_number().is_none() {
        return Ok(());
    }
    purlis_core::piececlaim::take(root, at, who, None, act, chrono::Utc::now())
}

/// The piece `at` names, in the project at `root`.
fn piece_of<'a>(
    root: &'a std::path::Path,
    at: &'a purlis_core::worktree::Located,
) -> purlis_core::pieceupdate::Piece<'a> {
    purlis_core::pieceupdate::Piece {
        root,
        ws: &at.workspace,
        repo: &at.repo,
        piece: &at.piece,
    }
}

/// In a chat the app started, `work` handed to the app and its answer printed; `None` where no
/// app took it, so it runs here.
fn ran_by_the_app(
    at: &purlis_core::worktree::Located,
    work: purlis_core::hookwire::GitWork,
    say: &mut dyn FnMut(purlis_core::repocmd::Say),
) -> Option<u8> {
    crate::gitask::told(
        crate::gitask::forwarded(
            &at.workspace,
            work,
            crate::gitask::A_PIECE_UPDATE_TAKES_AT_MOST,
        ),
        say,
    )
}
