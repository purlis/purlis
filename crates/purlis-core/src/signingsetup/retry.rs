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
const GITHUB_SAYS_WHOLE: &str = "Commits must have verified signatures";

/// **Whether `said`, a forge's refusal or git's words for a refused push, refuses unsigned
/// commits.** GitHub's documented words only; GitLab's are matched once read from a real server
/// (#5634), so a GitLab refusal answers `false` and keeps its plain wording.
pub fn refused_unsigned(kind: Kind, said: &str) -> bool {
    if kind == Kind::GitLab {
        return false;
    }
    // GitHub's own line, as git prints it (`remote: error: …`, `remote: - …`) or as its API says
    // it, alone: never words that only sit inside another line, such as a file's path.
    let said_alone = |line: &str| {
        let line = line.trim();
        let line = line.strip_prefix("remote:").unwrap_or(line).trim_start();
        let line = line.strip_prefix("error:").unwrap_or(line).trim_start();
        let line = line.strip_prefix("- ").unwrap_or(line).trim();
        let line = line.trim_end_matches('.');
        line.eq_ignore_ascii_case(GITHUB_SAYS_WHOLE)
    };
    let lines: Vec<&str> = said
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let from_a_rule = said.contains("GH006") || said.contains("GH013");
    lines.iter().any(|line| said_alone(line)) && (from_a_rule || lines.len() == 1)
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
    /// The lasting id (ADR 0066) of the chat that pushed (#5789): never the app's number for
    /// it, which a restart gives to another chat. A record that named the number reads as none.
    #[serde(default)]
    pub chat_id: Option<String>,
    /// When it was refused, in seconds since the epoch.
    pub at: i64,
    /// Signed and sent since: it stays listed as done until the window's next look clears it.
    #[serde(default)]
    pub sent: bool,
    /// Why the last Sign and retry did not send it, where it did not: it stays waiting, with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
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

/// The most commits one Sign and retry signs again: a lane's own work is a handful, and more
/// means the remote's history was not where purlis looked for it.
const MOST_SIGNED: usize = 200;

/// **The waiting lane `ws`/`repo`/`piece` dropped**: the person dismissed it, or its folder went.
pub fn forget(root: &Path, ws: &str, repo: &str, piece: &str) {
    note_pushed(root, ws, repo, piece, false);
}

/// **Every waiting lane dropped**, sent or not: the person dismissed the one notice.
pub fn forget_all(root: &Path) -> Result<(), String> {
    if waiting(root).is_empty() {
        return Ok(());
    }
    write(root, &[])
}

/// **The work waiting on signing whose branch folder is still there**: a lane whose folder was
/// removed waits on nothing, and is dropped as it is read.
pub fn waiting_here(root: &Path) -> Vec<Waiting> {
    let all = waiting(root);
    let (here, gone): (Vec<Waiting>, Vec<Waiting>) = all.into_iter().partition(|one| {
        worktree::path_for(root, &one.ws, &one.repo, &one.piece)
            .is_ok_and(|folder| folder.symlink_metadata().is_ok())
    });
    for one in &gone {
        forget(root, &one.ws, &one.repo, &one.piece);
    }
    here
}

/// **Sign and retry** one waiting branch folder (ADR 0116 §4), in the folder the broker checks
/// as it checks a chat's push ([`crate::gitbroker::in_a_checked_folder`]): the remote fetched
/// first; the commits on its branch the remote has none of written again signed with `signer`,
/// as `email` where the person chose one (#5631), and only those (ADR 0115 §3); the branch moved
/// to them from where it was alone; and the branch pushed again as a chat's push is. Refused,
/// leaving everything as it was, where the folder has gone (it waits no more), its branch is not
/// the one refused, the commits reach the start of the history, are more than [`MOST_SIGNED`],
/// or hold a commit signed already. Refused again by the forge, it stays waiting, with why.
pub fn sign_and_retry(
    root: &Path,
    one: &Waiting,
    signer: &Signer,
    email: Option<&str>,
    say: &mut dyn FnMut(Say),
) -> Result<Retried, String> {
    let label = format!("{} · {}", one.repo, one.piece);
    let here = worktree::path_for(root, &one.ws, &one.repo, &one.piece)
        .is_ok_and(|folder| folder.symlink_metadata().is_ok());
    if !here {
        forget(root, &one.ws, &one.repo, &one.piece);
        return Err(format!("{label} has gone, so it waits on nothing now"));
    }
    let isolation = git::isolation().unwrap_or_default();
    let ran = crate::gitbroker::in_a_checked_folder(
        root,
        &one.ws,
        &one.repo,
        &one.piece,
        &isolation,
        || signed_and_sent(root, one, signer, email, &label, say),
    );
    match ran {
        Ok(done) => done,
        Err(crate::gitbroker::NotRun::Repo(why)) => Err(why),
        Err(crate::gitbroker::NotRun::Folder(why)) => Err(why.to_string()),
    }
}

/// [`sign_and_retry`]'s work, in the checked folder.
fn signed_and_sent(
    root: &Path,
    one: &Waiting,
    signer: &Signer,
    email: Option<&str>,
    label: &str,
    say: &mut dyn FnMut(Say),
) -> Result<Retried, String> {
    let folder =
        worktree::path_for(root, &one.ws, &one.repo, &one.piece).map_err(|why| why.to_string())?;
    let folder = worktree::confine::within_workspace(root, &one.ws, &folder)
        .map_err(|why| why.to_string())?;
    let read = |args: &[&str]| -> Result<String, String> {
        let ran = git::run(&folder, args, git::READ).map_err(|why| why.to_string())?;
        if ran.ok() {
            Ok(ran.out.trim().to_owned())
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
    // What the remote has, asked now: nothing it has is written again.
    let remote = pieceupdate::remote_for(root, &one.ws, &one.repo, &branch)?;
    let clone = worktree::clone_dir(root, &one.ws, &one.repo).map_err(|why| why.to_string())?;
    crate::repocmd::sync::fetch_remote(root, &clone, &remote, None)
        .map_err(|why| format!("{why}. Nothing was signed"))?;
    let head = read(&["rev-parse", "--verify", "HEAD^{commit}"])?;
    let mine: Vec<String> = read(&["rev-list", &head, "--not", "--remotes"])?
        .lines()
        .map(str::to_owned)
        .filter(|id| !id.is_empty())
        .collect();
    if mine.len() > MOST_SIGNED {
        return Err(format!(
            "{label} has {} commits its remote has none of, more than a lane's own, so purlis \
             signed none of them",
            mine.len()
        ));
    }
    let roots = read(&["rev-list", "--max-parents=0", &head, "--not", "--remotes"])?;
    if !roots.is_empty() {
        return Err(format!(
            "{label}'s commits reach the start of its history, which its remote has none of, so \
             purlis signed none of them"
        ));
    }
    if let Some(signed) = mine.iter().find(|id| commitsign::is_signed(&folder, id)) {
        return Err(format!(
            "{label} holds commit {} that is signed already, so purlis left it as it is",
            &signed[..signed.len().min(12)]
        ));
    }
    let signed = commitsign::sign_branch_as(
        &folder,
        &worktree::name::as_ref(&branch),
        &head,
        &["--remotes"],
        signer,
        email,
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
        let why = failed.unwrap_or_else(|| format!("{label} was not sent"));
        // It stays waiting, on the commit refused now where it was refused again, with why.
        let again = Waiting {
            commit: unsigned.map_or_else(|| one.commit.clone(), |refused| refused.commit),
            why: Some(why.clone()),
            ..one.clone()
        };
        if let Err(trouble) = note_refused(root, again) {
            tracing::warn!("purlis: the work waiting on signing was not updated: {trouble}");
        }
        return Err(why);
    }
    note_pushed(root, &one.ws, &one.repo, &one.piece, true);
    Ok(Retried {
        signed: mine.len(),
        head: signed,
    })
}

#[cfg(test)]
mod tests;
