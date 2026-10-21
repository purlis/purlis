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
    /// The client writes it: `take_code`.
    In {
        refs: Vec<CodeRef>,
        checkout: Option<String>,
    },
    /// The host writes it: `give_code`.
    Out { have: Vec<String> },
}

impl Hosting {
    /// Carry out one transfer of code in `repo` of `project`, on a code stream named `transfer`.
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
        let bundle = scratch.path().join("transfer.bundle");
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
            match way {
                Way::In { refs, checkout } => {
                    let mut file = tokio::fs::File::create(&bundle).await.map_err(|why| {
                        not_started(format!("the bundle could not be kept: {why}"))
                    })?;
                    let read = code::copy_held_to(&mut stream, &mut file, self.most_bundle())
                        .await
                        .map_err(|why| match why {
                            code::Unread::TooLarge { .. } => Refusal {
                                code: session::TOO_LARGE.into(),
                                why: why.to_string(),
                            },
                            code::Unread::Io(_) => not_started(why.to_string()),
                        })?;
                    file.flush().await.map_err(|why| {
                        not_started(format!("the bundle could not be kept: {why}"))
                    })?;
                    drop(file);
                    let (at, bundle) = (at.clone(), bundle.clone());
                    let heads = tokio::task::spawn_blocking(move || {
                        let came = (read > 0).then_some(bundle.as_path());
                        code.take(&at, &repo, came, &refs, checkout.as_deref())
                    })
                    .await
                    .map_err(|why| not_started(format!("the transfer did not finish: {why}")))??;
                    Ok(heads)
                }
                Way::Out { have } => {
                    let (at, into) = (at.clone(), bundle.clone());
                    let (heads, wrote) =
                        tokio::task::spawn_blocking(move || code.give(&at, &repo, &have, &into))
                            .await
                            .map_err(|why| {
                                not_started(format!("the transfer did not finish: {why}"))
                            })??;
                    if wrote {
                        let mut file = tokio::fs::File::open(&bundle).await.map_err(|why| {
                            not_started(format!("the bundle could not be read: {why}"))
                        })?;
                        code::copy_held_to(&mut file, &mut stream, self.most_bundle())
                            .await
                            .map_err(|why| match why {
                                code::Unread::TooLarge { .. } => Refusal {
                                    code: session::TOO_LARGE.into(),
                                    why: why.to_string(),
                                },
                                code::Unread::Io(_) => not_started(why.to_string()),
                            })?;
                    }
                    stream
                        .shutdown()
                        .await
                        .map_err(|why| not_started(format!("the code stream broke: {why}")))?;
                    Ok(heads)
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
        Ok(Answer::Code { heads: moved })
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

#[cfg(test)]
mod tests;
