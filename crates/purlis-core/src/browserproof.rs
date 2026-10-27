// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Browser proof on a pull request** (#2026, spec #2008 story 57): screenshots of a chat's
//! [browser timeline](crate::browsertimeline) attached to a pull or merge request, only those the
//! person picks, already blanked, and only after the person has seen exactly what goes.
//!
//! - **Never automatic.** Nothing here runs by itself, and nothing a chat reaches calls it: the
//!   window's two presses are its only callers, [`show`] and then [`attach`], and the forge is
//!   asked with the person's own purlis sign-in alone (the app's
//!   `forge::signin::signed_in_backend`), never through `gh` or `glab`, and never as a chat.
//! - **Only what the person saw.** [`show`] reads the request and its repo (open, who can read
//!   it, whether the person may write there) and answers the request, where the pictures will
//!   be kept and who can open them there, a warning that they are public and permanent to the
//!   repo's readers, the comment's words, and each picked screenshot with its bytes, which the
//!   window draws, and their digest. [`attach`] is handed that answer back and refuses unless
//!   showing it again now answers the same: a screenshot the timeline let go, or one that
//!   changed, is never sent in its place.
//! - **Blanked already.** The timeline holds only the pictures the agent's browser tools
//!   returned, each already blanked by the browser where purlis filled a field or the person
//!   typed. This takes no picture of its own, and sends the bytes it read.
//! - **Nothing the agent wrote.** The comment is purlis's own words and the pictures. A moment's
//!   words and page, which can hold what the agent wrote or went to, are not in it, and every
//!   picture's file name is purlis's (`browser-proof-<n>.png`).
//! - **While the chat is open.** A chat's timeline goes when the chat closes, and this reads
//!   nothing else, so proof is attached from the chat's timeline while the chat is open.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::browsertimeline;
use crate::counts::counted;
use crate::forge::backend::{RepoRecord, Visibility};
use crate::forge::pr::{
    Image, Pr, State, proof_kept_at, proof_needs_push, proof_open_by_link, request_mark,
    request_page_between,
};
use crate::forge::{Caller, Forge, ForgeBackend, Kind};

/// The most screenshots one proof attaches.
pub const MOST_PICKED: usize = browsertimeline::MOST_SHOTS;

/// A pull or merge request the person named, on a forge the project declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub forge: Forge,
    /// `owner/name` on GitHub, the full namespace on GitLab.
    pub path: String,
    pub number: u64,
    /// Its page, as the person gave it.
    pub url: String,
}

/// One screenshot as [`show`] read it: its number in the timeline, when it was taken, and the
/// digest of its bytes, which [`attach`] reads again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownShot {
    pub shot: u64,
    pub at: u64,
    /// SHA-256 of its PNG, lower-case hex.
    pub digest: String,
}

/// What the person is shown before anything is sent, and hands back to [`attach`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    /// The request's page.
    pub request: String,
    /// `#12` or `!12`, as its forge names it.
    pub number: String,
    /// The repo it is in.
    pub path: String,
    /// Where the pictures are kept on the forge, and who can open them there.
    pub kept_at: String,
    /// The line said before the pictures: they are public and permanent to whoever can read
    /// the repo, and whether that is everyone (the person's ruling on D-2026-3).
    pub warning: String,
    /// The comment's words, which come before the pictures.
    pub words: String,
    /// The screenshots, in the order they go.
    pub shots: Vec<ShownShot>,
}

/// What [`attach`] did: the comment's page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attached {
    pub comment: String,
}

/// The request at `address`: a pull request's page on a GitHub, or a merge request's on a
/// GitLab, on a forge the project at `root` declares (or a forge's own instance). Anything else
/// is refused, in words for the person.
pub fn request_of(root: &Path, address: &str) -> Result<Request, String> {
    let address = address.trim();
    let refused = || {
        format!(
            "{address:?} is not the address of a pull request or merge request on a forge this \
             project knows"
        )
    };
    if !address.starts_with("https://") || address.contains(['?', '#', '@', ' ']) {
        return Err(refused());
    }
    let forge = crate::forge::resolve_host(address, root).ok_or_else(refused)?;
    let rest = address
        .strip_prefix("https://")
        .and_then(|rest| rest.split_once('/'))
        .map(|(_, path)| path.trim_end_matches('/'))
        .ok_or_else(refused)?;
    let (path, number) = rest
        .rsplit_once(request_page_between(forge.kind))
        .ok_or_else(refused)?;
    let number: u64 = number
        .parse()
        .ok()
        .filter(|n| *n > 0 && !number.starts_with('0'))
        .ok_or_else(refused)?;
    let plain = !path.is_empty()
        && path.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        });
    let parts = path.split('/').count();
    let shaped = match forge.kind {
        Kind::GitHub => parts == 2,
        Kind::GitLab => parts >= 2,
    };
    if !plain || !shaped {
        return Err(refused());
    }
    Ok(Request {
        forge,
        path: path.to_string(),
        number,
        url: address.to_string(),
    })
}

/// What [`show`] answers: what the person is shown, and the bytes of each screenshot, in the
/// order of [`Shown::shots`], which the window draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Showing {
    pub shown: Shown,
    pub pictures: Vec<Vec<u8>>,
}

/// Makes the backend a proof is shown and attached through, for a request's forge.
pub type BackendFor<'a> = dyn Fn(&Forge) -> Result<Box<dyn ForgeBackend>, String> + 'a;

/// The comment's words before the pictures: purlis's own, the same for every proof of `count`
/// screenshots.
pub fn words(count: usize) -> String {
    format!(
        "### Browser proof\n\n{} of what an agent did in a browser, attached from purlis by the \
         person who picked them. A field purlis filled in or the person typed into is blanked.",
        counted(count, "screenshot", "screenshots")
    )
}

/// The line said before the pictures (the person's ruling on D-2026-3), with whether the repo
/// is public said plainly, and, where the pictures open for anyone with their link
/// (`open_by_link`, a GitLab project that does not check who opens its uploads), that too: a
/// private project is then no bound on who sees them.
pub fn warning(path: &str, host: &str, visibility: Visibility, open_by_link: bool) -> String {
    let link = "anyone with a picture's link can open it, signed in or not";
    let readers = match (visibility, open_by_link) {
        (Visibility::Public, _) => format!("{path} is public: everyone can see these."),
        (Visibility::Internal, false) => {
            format!("{path} is internal: everyone signed in to {host} can see these.")
        }
        (Visibility::Internal, true) => format!("{path} is internal, but {link}."),
        (Visibility::Private, false) => format!("{path} is private."),
        (Visibility::Private, true) => format!("{path} is private, but {link}."),
    };
    format!(
        "Public and permanent: anyone who can read {path} (everyone, on a public repo) sees \
         these, even after the request closes. {readers}"
    )
}

fn digest(png: &[u8]) -> String {
    Sha256::digest(png)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// **What attaching `picked` of chat `chat`'s screenshots to the request at `address` would
/// send**, for the person to see before anything goes. It reads the timeline, and asks the
/// request's forge, through `backend` as `caller`, whether the request is open, who can read its
/// repo and whether the person may write there; it writes nothing.
///
/// Refused when nothing is picked, too many are, a picked one is no screenshot the timeline
/// keeps, the address is no request on a forge the project knows, the request is not open, or
/// the forge keeps the pictures in the repo and the person may not push there.
pub fn show(
    root: &Path,
    chat: &str,
    address: &str,
    picked: &[u64],
    backend: &BackendFor<'_>,
    caller: &Caller,
) -> Result<Showing, String> {
    if picked.is_empty() {
        return Err("Pick at least one screenshot to attach.".to_string());
    }
    if picked.len() > MOST_PICKED {
        return Err(format!(
            "At most {} go in one proof.",
            counted(MOST_PICKED, "screenshot", "screenshots")
        ));
    }
    let request = request_of(root, address)?;
    let moments = browsertimeline::read(root, chat);
    let mut shots: Vec<ShownShot> = Vec::new();
    let mut pictures = Vec::new();
    for &shot in picked {
        if shots.iter().any(|one| one.shot == shot) {
            continue;
        }
        let at = moments
            .iter()
            .find(|moment| moment.shot == Some(shot))
            .map(|moment| moment.at);
        let (Some(at), Some(png)) = (at, browsertimeline::shot_of(root, chat, shot)) else {
            return Err(NO_LONGER_KEPT.to_string());
        };
        shots.push(ShownShot {
            shot,
            at,
            digest: digest(&png),
        });
        pictures.push(png);
    }
    let number = format!("{}{}", request_mark(request.forge.kind), request.number);
    let forge = backend(&request.forge)?;
    let pr = pr_of(&request);
    match forge
        .state(caller, &request.path, &pr)
        .map_err(|why| why.said().to_string())?
    {
        State::Open => {}
        _ => {
            return Err(format!(
                "{number} in {} is not open, so no proof is attached to it.",
                request.path
            ));
        }
    }
    let about = forge
        .about(caller, &repo_of(&request))
        .map_err(|why| why.said().to_string())?;
    if proof_needs_push(request.forge.kind) && about.can_push == Some(false) {
        return Err(format!(
            "You cannot push to {path}, and the pictures are kept on a branch there, so no proof \
             can be attached to {number}. Someone who can push to {path} can attach it.",
            path = request.path
        ));
    }
    Ok(Showing {
        shown: Shown {
            number,
            kept_at: proof_kept_at(
                request.forge.kind,
                &request.path,
                request.number,
                about.uploads_need_sign_in,
            ),
            warning: warning(
                &request.path,
                &request.forge.host,
                about.visibility,
                proof_open_by_link(request.forge.kind, about.uploads_need_sign_in),
            ),
            words: words(shots.len()),
            path: request.path,
            request: request.url,
            shots,
        },
        pictures,
    })
}

fn pr_of(request: &Request) -> Pr {
    Pr {
        number: request.number,
        url: request.url.clone(),
    }
}

/// The request's repo, as [`crate::forge::Repos::about`] is asked of it: by its path.
fn repo_of(request: &Request) -> RepoRecord {
    let name = request
        .path
        .rsplit_once('/')
        .map_or(request.path.as_str(), |(_, name)| name);
    RepoRecord {
        id: None,
        name: name.to_string(),
        path_with_namespace: request.path.clone(),
        default_branch: None,
        description: String::new(),
        web_url: String::new(),
        ssh_url: String::new(),
        topics: Vec::new(),
        forge: request.forge.kind,
    }
}

/// **Attach what [`show`] showed** to its request, as the person, through `backend` (the
/// person's own sign-in, which the app makes; `caller` is the person in the window). Answers the
/// comment's page: the forge's answer where it is on the forge's own host, else the request's.
///
/// Refused, before the forge is asked to keep anything, unless showing it again now answers
/// exactly what was shown: the same screenshots with the same bytes, the same words, an open
/// request, and the same readers.
pub fn attach(
    root: &Path,
    chat: &str,
    shown: &Shown,
    backend: &BackendFor<'_>,
    caller: &Caller,
) -> Result<Attached, String> {
    let request = request_of(root, &shown.request)?;
    let picked: Vec<u64> = shown.shots.iter().map(|one| one.shot).collect();
    // A screenshot let go since is what changed; any other refusal (a request that closed, a
    // sign-in that went) is said in its own words.
    let fresh = show(root, chat, &shown.request, &picked, backend, caller).map_err(|why| {
        if why == NO_LONGER_KEPT {
            changed()
        } else {
            why
        }
    })?;
    if fresh.shown != *shown {
        return Err(changed());
    }
    let images: Vec<Image> = fresh
        .pictures
        .into_iter()
        .enumerate()
        .map(|(n, png)| Image {
            name: format!("browser-proof-{}.png", n + 1),
            png,
        })
        .collect();
    let forge = backend(&request.forge)?;
    let pr = pr_of(&request);
    let kept = forge
        .keep_images(caller, &request.path, &pr, &images)
        .map_err(|why| why.said().to_string())?;
    let mut body = shown.words.clone();
    for (n, address) in kept.iter().enumerate() {
        body.push_str(&format!("\n\n![Browser screenshot {}]({address})", n + 1));
    }
    let comment = forge
        .comment(caller, &request.path, &pr, &body)
        .map_err(|why| why.said().to_string())?;
    let own = comment.starts_with(&format!("https://{}/", request.forge.host))
        && !comment.contains(['\\', ' ', '\n', '\r', '@']);
    Ok(Attached {
        comment: if own { comment } else { request.url },
    })
}

/// Why a screenshot picked is not shown.
const NO_LONGER_KEPT: &str = "A screenshot you picked is no longer kept.";

fn changed() -> String {
    "What you were shown changed since: press Show what goes again before attaching.".to_string()
}

#[cfg(test)]
mod tests;
