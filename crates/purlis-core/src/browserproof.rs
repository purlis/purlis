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
//! - **Only what the person saw.** [`show`] answers the request, where the pictures will be kept
//!   and who can read them there, the comment's words, and each picked screenshot with the
//!   digest of its bytes. [`attach`] is handed that answer back and refuses unless every
//!   screenshot still reads with the same digest: a screenshot the timeline let go, or one that
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
use crate::forge::pr::{Image, Pr, State, proof_kept_at};
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
    /// Where the pictures are kept on the forge, and who can read them there.
    pub kept_at: String,
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
    let between = match forge.kind {
        Kind::GitHub => "/pull/",
        Kind::GitLab => "/-/merge_requests/",
    };
    let (path, number) = rest.rsplit_once(between).ok_or_else(refused)?;
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

/// The comment's words before the pictures: purlis's own, the same for every proof of `count`
/// screenshots.
pub fn words(count: usize) -> String {
    let screenshots = if count == 1 {
        "1 screenshot".to_string()
    } else {
        format!("{count} screenshots")
    };
    format!(
        "### Browser proof\n\n{screenshots} of what an agent did in a browser, attached from \
         purlis by the person who picked them. A field purlis filled in or the person typed \
         into is blanked."
    )
}

fn digest(png: &[u8]) -> String {
    Sha256::digest(png)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// **What attaching `picked` of chat `chat`'s screenshots to the request at `address` would
/// send**, read now, for the person to see before anything goes. Refused when nothing is picked,
/// too many are, a picked one is no screenshot the timeline keeps, or the address is no request
/// on a forge the project knows. It asks no forge.
pub fn show(root: &Path, chat: &str, address: &str, picked: &[u64]) -> Result<Shown, String> {
    if picked.is_empty() {
        return Err("Pick at least one screenshot to attach.".to_string());
    }
    if picked.len() > MOST_PICKED {
        return Err(format!(
            "At most {MOST_PICKED} screenshots go in one proof."
        ));
    }
    let request = request_of(root, address)?;
    let moments = browsertimeline::read(root, chat);
    let mut shots: Vec<ShownShot> = Vec::new();
    for &shot in picked {
        if shots.iter().any(|one| one.shot == shot) {
            continue;
        }
        let at = moments
            .iter()
            .find(|moment| moment.shot == Some(shot))
            .map(|moment| moment.at);
        let (Some(at), Some(png)) = (at, browsertimeline::shot_of(root, chat, shot)) else {
            return Err("A screenshot you picked is no longer kept.".to_string());
        };
        shots.push(ShownShot {
            shot,
            at,
            digest: digest(&png),
        });
    }
    let sign = match request.forge.kind {
        Kind::GitHub => '#',
        Kind::GitLab => '!',
    };
    Ok(Shown {
        number: format!("{sign}{}", request.number),
        kept_at: proof_kept_at(request.forge.kind, &request.path, request.number),
        words: words(shots.len()),
        path: request.path,
        request: request.url,
        shots,
    })
}

/// **Attach what [`show`] showed** to its request, as the person, through `backend` (the
/// person's own sign-in, which the app makes; `caller` is the person in the window).
///
/// Refused, before the forge is asked to keep anything, when a screenshot shown is no longer kept
/// or its bytes are not the ones shown, when the words are not this proof's, or when the request
/// is not open.
pub fn attach(
    root: &Path,
    chat: &str,
    shown: &Shown,
    backend: &BackendFor<'_>,
    caller: &Caller,
) -> Result<Attached, String> {
    let request = request_of(root, &shown.request)?;
    let fresh = show(
        root,
        chat,
        &shown.request,
        &shown.shots.iter().map(|one| one.shot).collect::<Vec<_>>(),
    )
    .map_err(|_| changed())?;
    if fresh != *shown {
        return Err(changed());
    }
    let mut images = Vec::new();
    for (n, one) in shown.shots.iter().enumerate() {
        let png = browsertimeline::shot_of(root, chat, one.shot).ok_or_else(changed)?;
        if digest(&png) != one.digest {
            return Err(changed());
        }
        images.push(Image {
            name: format!("browser-proof-{}.png", n + 1),
            png,
        });
    }
    let forge = backend(&request.forge)?;
    let pr = Pr {
        number: request.number,
        url: request.url.clone(),
    };
    match forge
        .state(caller, &request.path, &pr)
        .map_err(|why| why.said().to_string())?
    {
        State::Open => {}
        _ => {
            return Err(format!(
                "{} in {} is not open, so no proof is attached to it.",
                shown.number, shown.path
            ));
        }
    }
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
    Ok(Attached { comment })
}

/// Makes the backend a proof is attached through, for a request's forge.
pub type BackendFor<'a> = dyn Fn(&Forge) -> Result<Box<dyn ForgeBackend>, String> + 'a;

fn changed() -> String {
    "What you were shown changed since: show it again before attaching.".to_string()
}

#[cfg(test)]
mod tests;
