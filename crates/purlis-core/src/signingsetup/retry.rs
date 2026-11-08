// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A forge's refusal of unsigned commits, and Sign and retry** (#5630, spec #5607, ADR 0116
//! §3 and §4).
//!
//! - **The refusal is told apart** ([`refused_unsigned`]): GitHub refuses a push to a branch that
//!   requires signed commits, by a branch protection rule or a ruleset, and a merge or a merge
//!   queue entry of one, with the same documented words: *Commits must have verified
//!   signatures*. Every other refusal stays as it was. GitLab's push rule is read from a real
//!   server before it is matched (#5634), so a GitLab refusal is not told apart yet.
//! - **The work it stopped waits** ([`Waiting`]): a chat's branch folder whose push was refused
//!   so, kept under the project's state, where no chat writes, until it is signed and sent, or
//!   pushed some other way. The window lists it on the lane, in the chat and in one notice.
//! - **Sign and retry** ([`sign_and_retry`]) re-signs the commits made on the branch that its
//!   remote does not have (ADR 0115 §3: nothing already pushed is rewritten), moves the branch to
//!   the signed copies only from where it was, and pushes it again through the same push a chat's
//!   `purlis worktree push` takes.

use std::path::{Path, PathBuf};

use crate::commitsign::{self, Signer};
use crate::forge::Kind;
use crate::pieceupdate::{self, Piece, Reach};
use crate::repocmd::Say;
use crate::worktree::{self, git};

/// GitHub's own words for a refusal of unsigned commits, the same for a protected branch's push
/// (`GH006`), a ruleset's (`GH013`), a merge and a merge queue entry: docs.github.com, "About
/// protected branches" and "Available rules for rulesets" (*Require signed commits*).
const GITHUB_SAYS: &str = "must have verified signatures";

/// **Whether `said`, a forge's refusal or git's words for a refused push, refuses unsigned
/// commits.** GitHub's documented words only; GitLab's are matched once read from a real server
/// (#5634), so a GitLab refusal answers `false` and keeps its plain wording.
pub fn refused_unsigned(kind: Kind, said: &str) -> bool {
    match kind {
        Kind::GitHub => said.to_ascii_lowercase().contains(GITHUB_SAYS),
        Kind::GitLab => false,
    }
}

/// What a chat is told where the forge refused its push for unsigned commits, after which the
/// window puts the setup in front of the person. The chat never runs setup (ADR 0116 §2).
pub fn refused_said(kind: Kind, branch: &str) -> String {
    let forge = kind.display();
    format!(
        "{forge} only accepts signed commits here: it refused the push of {branch} because its \
         commits aren't signed. A chat can't set signing up: it's the person's to do in the \
         purlis window, which now shows them Set up signing. Once it's set up, they press Sign \
         and retry there, which signs this branch's commits and sends them again. Nothing for \
         the chat to retry until then"
    )
}

/// **A chat's branch folder whose push the forge refused for unsigned commits.**
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Waiting {
    pub ws: String,
    pub repo: String,
    pub piece: String,
    pub branch: String,
    /// The forge that refused it, its host, and the repository's path there.
    pub kind: Kind,
    pub host: String,
    pub path: String,
    /// The commit whose push was refused.
    pub commit: String,
    /// The app's number for the chat that pushed, while it is open.
    pub chat: Option<u32>,
    /// When it was refused, in seconds since the epoch.
    pub at: i64,
    /// Signed and sent since: it stays listed as done until the window's next look clears it.
    #[serde(default)]
    pub sent: bool,
}

impl Waiting {
    /// Whether it is the branch folder `repo` · `piece` of workspace `ws`.
    pub fn is(&self, ws: &str, repo: &str, piece: &str) -> bool {
        self.ws == ws && self.repo == repo && self.piece == piece
    }
}

/// Where the waiting work is kept: under the project's state, which no sandboxed chat writes.
fn waiting_file(root: &Path) -> PathBuf {
    crate::names::state(root)
        .join("signing")
        .join("waiting.json")
}

/// **The work waiting on signing**, oldest first; none where nothing was refused.
pub fn waiting(root: &Path) -> Vec<Waiting> {
    std::fs::read_to_string(waiting_file(root))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write(root: &Path, all: &[Waiting]) -> Result<(), String> {
    let file = waiting_file(root);
    if let Some(folder) = file.parent() {
        crate::contain::no_link_on_the_way(root, folder).map_err(|why| why.to_string())?;
        std::fs::create_dir_all(folder).map_err(|why| why.to_string())?;
    }
    let text = serde_json::to_string_pretty(all).map_err(|why| why.to_string())?;
    crate::plane::write_private(root, &file, text.as_bytes()).map_err(|why| why.to_string())
}

/// **`refused` kept as waiting**, in place of what its branch folder waited on before.
pub fn note_refused(root: &Path, refused: Waiting) -> Result<(), String> {
    let mut all = waiting(root);
    all.retain(|was| !was.is(&refused.ws, &refused.repo, &refused.piece));
    all.push(refused);
    write(root, &all)
}

/// **The branch folder pushed**: what it waited on is over. Marked sent where `sent`, so the
/// window shows "Signed and sent ✓"; dropped otherwise (it went some other way).
pub fn note_pushed(root: &Path, ws: &str, repo: &str, piece: &str, sent: bool) {
    let mut all = waiting(root);
    let before = all.clone();
    if sent {
        for one in all.iter_mut().filter(|one| one.is(ws, repo, piece)) {
            one.sent = true;
        }
    } else {
        all.retain(|one| !one.is(ws, repo, piece));
    }
    if all != before
        && let Err(why) = write(root, &all)
    {
        tracing::warn!("purlis: the work waiting on signing was not updated: {why}");
    }
}

/// **What was signed and sent, cleared**: the window has shown it.
pub fn forget_sent(root: &Path) -> Result<(), String> {
    let mut all = waiting(root);
    let before = all.len();
    all.retain(|one| !one.sent);
    if all.len() == before {
        return Ok(());
    }
    write(root, &all)
}

/// How a Sign and retry went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retried {
    /// How many commits were signed again.
    pub signed: usize,
    /// The branch's new head.
    pub head: String,
}

/// **Sign and retry** one waiting branch folder (ADR 0116 §4): the commits on its branch that
/// its remote has none of are written again signed with `signer` (ADR 0115 §3), the branch moved
/// to them from where it was alone, and the branch pushed again as a chat's push is. Refused,
/// with the reason, where the folder has gone, its branch is not the one refused, or the push is
/// refused again; it then stays waiting.
pub fn sign_and_retry(
    root: &Path,
    one: &Waiting,
    signer: &Signer,
    say: &mut dyn FnMut(Say),
) -> Result<Retried, String> {
    let folder =
        worktree::path_for(root, &one.ws, &one.repo, &one.piece).map_err(|why| why.to_string())?;
    let folder = worktree::confine::within_workspace(root, &one.ws, &folder)
        .map_err(|why| why.to_string())?;
    let label = format!("{} · {}", one.repo, one.piece);
    if folder.symlink_metadata().is_err() {
        return Err(format!(
            "{label} has gone, so there is nothing to sign and send"
        ));
    }
    let read = |args: &[&str]| -> Result<String, String> {
        let ran = git::run(&folder, args, git::READ).map_err(|why| why.to_string())?;
        if ran.ok() {
            Ok(ran.line().trim().to_owned())
        } else {
            Err(format!(
                "git could not read {label}: {}",
                ran.err.lines().next().unwrap_or("").trim()
            ))
        }
    };
    let branch = read(&["symbolic-ref", "--quiet", "--short", "HEAD"])?;
    if branch != one.branch {
        return Err(format!(
            "{label} is on {branch} now, not {}, so purlis left it as it is",
            one.branch
        ));
    }
    let head = read(&["rev-parse", "--verify", "HEAD^{commit}"])?;
    // Only what the remote has none of: nothing already pushed is written again.
    let unpushed: usize = read(&["rev-list", "--count", &head, "--not", "--remotes"])?
        .parse()
        .unwrap_or(0);
    let signed = commitsign::sign_branch(
        &folder,
        &worktree::name::as_ref(&branch),
        &head,
        &["--remotes"],
        signer,
    )?;
    let piece = Piece {
        root,
        ws: &one.ws,
        repo: &one.repo,
        piece: &one.piece,
    };
    let reach = Reach {
        hosts: None,
        sandboxed_at: None,
        for_a_chat: true,
    };
    let mut failed = None;
    let (code, unsigned) = pieceupdate::push_told(&piece, &reach, &mut |line| {
        if let Say::Fail(why) = &line {
            failed = Some(why.to_string());
        }
        say(line);
    });
    if code != 0 {
        // Refused again: it waits on the commit it refused now.
        if let Some(refused) = unsigned {
            let again = Waiting {
                commit: refused.commit,
                at: chrono::Utc::now().timestamp(),
                ..one.clone()
            };
            if let Err(why) = note_refused(root, again) {
                tracing::warn!("purlis: the work waiting on signing was not updated: {why}");
            }
        }
        return Err(failed.unwrap_or_else(|| format!("{label} was not sent")));
    }
    note_pushed(root, &one.ws, &one.repo, &one.piece, true);
    Ok(Retried {
        signed: unpushed,
        head: signed,
    })
}

#[cfg(test)]
mod tests;
