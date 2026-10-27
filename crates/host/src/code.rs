// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Code over the link, in the host** (1.34, #2046, ADR 0078 §7): `take_code` and
//! `give_code`, a git bundle moved on a code stream the host opens
//! ([`purlis_session_protocol::code`]).
//!
//! **The host moves the bytes; the binary moves the code** ([`Code`]): which refs a bundle may
//! carry, where in the project a repository is, and every git call are `purlis serve`'s, built on
//! the core (`purlis_core::codelink`), so this crate links no git.
//!
//! **Only into a project the host keeps itself.** A host moves code only in a project it holds
//! where it keeps its own projects (`<data>/projects/<project id>`, [`Hosting::keeping_projects_in`]),
//! never in one held at a path a client named: what a desktop sends a runner lands in the
//! runner's own folder for it, and nowhere else on the runner.
//!
//! **A bundle is held to [`code::MOST_BUNDLE_BYTES`]**, and a transfer to [`TRANSFER_WITHIN`]: one
//! past either is refused in a sentence, and what it brought is removed.

use std::path::{Path, PathBuf};
use std::time::Duration;

use purlis_session_protocol::code::{self, Header};
use purlis_session_protocol::session::{self, Answer, CodeRef, Peer, Refusal};
use tokio::io::AsyncWriteExt;

use crate::hosting::Hosting;

/// The longest one transfer of code may take, its bytes and its git calls together.
pub const TRANSFER_WITHIN: Duration = Duration::from_secs(600);

/// **How a host moves code in a repository of a project it keeps** (#2046): the binary's.
pub trait Code: Send + Sync + 'static {
    /// Take `refs` into the repository at `repo` in the project at `project`: from the bundle at
    /// `bundle` where the client sent one, else from what the repository holds; then, in a
    /// repository with nothing checked out yet, check out `checkout`. Answers every branch and
    /// snapshot it holds after, or the refusal the client is told.
    fn take(
        &self,
        project: &Path,
        repo: &str,
        bundle: Option<&Path>,
        refs: &[CodeRef],
        checkout: Option<&str>,
    ) -> Result<Vec<CodeRef>, Refusal>;

    /// Bundle the branches of the repository at `repo` in the project at `project` into `into`,
    /// leaving out what `have` reaches. Answers the branches, and whether it wrote a bundle.
    fn give(
        &self,
        project: &Path,
        repo: &str,
        have: &[String],
        into: &Path,
    ) -> Result<(Vec<CodeRef>, bool), Refusal>;

    /// **Take a moving chat's work tree over** (1.41, #2048): `refs`, the branch `checkout` and
    /// its snapshot, from the bundle at `bundle` or what the repository holds, onto the
    /// repository's work tree where it is still what both ends last agreed. Answers every branch
    /// and snapshot it holds after. A binary that moves no chat refuses it.
    fn take_work(
        &self,
        project: &Path,
        repo: &str,
        bundle: Option<&Path>,
        refs: &[CodeRef],
        checkout: &str,
    ) -> Result<Vec<CodeRef>, Refusal> {
        let _ = (project, repo, bundle, refs, checkout);
        Err(moves_no_chat())
    }

    /// [`Code::give`], with the work tree's uncommitted work as its branch's snapshot, which both
    /// ends agree on from then on (1.41, #2048).
    fn give_work(
        &self,
        project: &Path,
        repo: &str,
        have: &[String],
        into: &Path,
    ) -> Result<(Vec<CodeRef>, bool), Refusal> {
        let _ = (project, repo, have, into);
        Err(moves_no_chat())
    }

    /// **Keep conversation `conversation`, carried as `file` at `from`**, in the harness folder
    /// of `profile` for a chat in `workspace` of the project at `project` (1.41, #2048).
    fn take_conversation(
        &self,
        project: &Path,
        carried: &Carried,
        from: &Path,
    ) -> Result<(), Refusal> {
        let _ = (project, carried, from);
        Err(moves_no_chat())
    }

    /// **Write conversation `conversation`** of `profile`, for a chat in `workspace`, into
    /// `into` (1.41, #2048). Answers where its harness keeps it, as [`Carried::file`] names it.
    fn give_conversation(
        &self,
        project: &Path,
        carried: &Carried,
        into: &Path,
    ) -> Result<String, Refusal> {
        let _ = (project, carried, into);
        Err(moves_no_chat())
    }
}

/// **A conversation a moving chat carries** (1.41, #2048): what `take_conversation` and
/// `give_conversation` name of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Carried {
    /// The chat's workspace, whose folder it runs in.
    pub workspace: String,
    /// The profile it runs on, whose harness keeps it.
    pub profile: String,
    /// The harness's session id.
    pub conversation: String,
    /// Where the harness's declaration keeps it, as found: empty where it is to be found.
    pub file: String,
}

fn moves_no_chat() -> Refusal {
    Refusal {
        code: session::NOT_STARTED.into(),
        why: "this session host moves no chat".into(),
    }
}

/// A host that moves no code: every transfer is refused.
pub struct MovesNoCode;

impl Code for MovesNoCode {
    fn take(
        &self,
        _: &Path,
        _: &str,
        _: Option<&Path>,
        _: &[CodeRef],
        _: Option<&str>,
    ) -> Result<Vec<CodeRef>, Refusal> {
        Err(moves_none())
    }

    fn give(
        &self,
        _: &Path,
        _: &str,
        _: &[String],
        _: &Path,
    ) -> Result<(Vec<CodeRef>, bool), Refusal> {
        Err(moves_none())
    }
}

fn moves_none() -> Refusal {
    Refusal {
        code: session::NOT_STARTED.into(),
        why: "this session host moves no code".into(),
    }
}

fn not_started(why: impl Into<String>) -> Refusal {
    Refusal {
        code: session::NOT_STARTED.into(),
        why: why.into(),
    }
}

/// Which way a transfer's bundle goes.
pub(crate) enum Way {
    /// The client writes it: `take_code`, taking a moving chat's work tree over where `moving`.
    In {
        refs: Vec<CodeRef>,
        checkout: Option<String>,
        moving: bool,
    },
    /// The host writes it: `give_code`, with the work tree's work where `with_work`.
    Out { have: Vec<String>, with_work: bool },
    /// The client writes a moving chat's conversation: `take_conversation` (1.41).
    ConversationIn(Carried),
    /// The host writes one: `give_conversation` (1.41).
    ConversationOut(Carried),
}

impl Hosting {
    /// Carry out one transfer of code in `repo` of `project`, on a code stream named `transfer`:
    /// a bundle, or a moving chat's conversation (1.41, #2048).
    pub(crate) async fn move_code(
        &self,
        project: &str,
        repo: String,
        transfer: u64,
        way: Way,
        peer: &Peer,
    ) -> Result<Answer, Refusal> {
        let at = self.kept_project_held(project, peer)?;
        let code = self.code();
        let scratch = tempfile::Builder::new()
            .prefix(".code-")
            .tempdir_in(self.code_scratch(&at))
            .map_err(|why| not_started(format!("there is no room for the bundle: {why}")))?;
        let file = scratch.path().join("transfer.bundle");
        let most = self.most_bundle();
        let moved = tokio::time::timeout(TRANSFER_WITHIN, async {
            let mut stream = peer
                .opener()
                .open()
                .await
                .map_err(|why| not_started(format!("the code stream did not open: {why}")))?;
            stream
                .write_all(&Header { transfer }.to_bytes())
                .await
                .map_err(|why| not_started(format!("the code stream broke: {why}")))?;
            let (at, file) = (at.clone(), file.clone());
            match way {
                Way::In {
                    refs,
                    checkout,
                    moving,
                } => {
                    let read = take_in(&mut stream, &file, most).await?;
                    blocking(move || {
                        let came = (read > 0).then_some(file.as_path());
                        let heads = match (moving, checkout) {
                            (true, Some(checkout)) => {
                                code.take_work(&at, &repo, came, &refs, &checkout)
                            }
                            (true, None) => Err(Refusal {
                                code: session::NOT_ALLOWED.into(),
                                why: "a move names the branch its work tree goes onto".into(),
                            }),
                            (false, checkout) => {
                                code.take(&at, &repo, came, &refs, checkout.as_deref())
                            }
                        }?;
                        Ok(Answer::Code { heads })
                    })
                    .await
                }
                Way::Out { have, with_work } => {
                    let into = file.clone();
                    let (heads, wrote) = blocking(move || {
                        if with_work {
                            code.give_work(&at, &repo, &have, &into)
                        } else {
                            code.give(&at, &repo, &have, &into)
                        }
                    })
                    .await?;
                    give_out(&mut stream, wrote.then_some(file.as_path()), most).await?;
                    Ok(Answer::Code { heads })
                }
                Way::ConversationIn(carried) => {
                    take_in(&mut stream, &file, most).await?;
                    blocking(move || code.take_conversation(&at, &carried, &file)).await?;
                    Ok(Answer::Done)
                }
                Way::ConversationOut(carried) => {
                    let into = file.clone();
                    let kept =
                        blocking(move || code.give_conversation(&at, &carried, &into)).await?;
                    give_out(&mut stream, Some(file.as_path()), most).await?;
                    Ok(Answer::Conversation { file: kept })
                }
            }
        })
        .await
        .map_err(|_| {
            not_started(format!(
                "the transfer took longer than {} minutes, so it was given up",
                TRANSFER_WITHIN.as_secs() / 60
            ))
        })??;
        Ok(moved)
    }

    /// Where a transfer's bundle is kept while it moves: beside the projects the host keeps, so
    /// a large one fills no small temporary folder, and never inside the project itself.
    fn code_scratch(&self, project: &Path) -> PathBuf {
        project
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(std::env::temp_dir)
    }
}

/// Runs `work`, the binary's git or file work, off the link's threads.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, Refusal> + Send + 'static,
) -> Result<T, Refusal> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|why| not_started(format!("the transfer did not finish: {why}")))?
}

fn unread(why: code::Unread) -> Refusal {
    match why {
        code::Unread::TooLarge { .. } => Refusal {
            code: session::TOO_LARGE.into(),
            why: why.to_string(),
        },
        code::Unread::Io(_) => not_started(why.to_string()),
    }
}

/// Reads what the client writes on `stream` into `file`, at most `most` bytes. Answers how many.
async fn take_in(
    stream: &mut purlis_session_protocol::link::Stream,
    file: &Path,
    most: u64,
) -> Result<u64, Refusal> {
    let mut kept = tokio::fs::File::create(file)
        .await
        .map_err(|why| not_started(format!("the bundle could not be kept: {why}")))?;
    let read = code::copy_held_to(stream, &mut kept, most)
        .await
        .map_err(unread)?;
    kept.flush()
        .await
        .map_err(|why| not_started(format!("the bundle could not be kept: {why}")))?;
    Ok(read)
}

/// Writes `file`, where there is one, on `stream` and shuts it.
async fn give_out(
    stream: &mut purlis_session_protocol::link::Stream,
    file: Option<&Path>,
    most: u64,
) -> Result<(), Refusal> {
    if let Some(file) = file {
        let mut read = tokio::fs::File::open(file)
            .await
            .map_err(|why| not_started(format!("the bundle could not be read: {why}")))?;
        code::copy_held_to(&mut read, stream, most)
            .await
            .map_err(unread)?;
    }
    stream
        .shutdown()
        .await
        .map_err(|why| not_started(format!("the code stream broke: {why}")))
}

#[cfg(test)]
mod tests;
