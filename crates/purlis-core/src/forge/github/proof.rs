// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Browser proof's pictures on GitHub (#2026). GitHub's API has no way to attach a file to a
//! comment, so they are kept in the repo itself: one commit of only the pictures, through the
//! Git Data API, on [`proof_branch`]. The branch's first commit has no parent and each later
//! one builds on the last, so it never holds a file of the repo's own, a workflow included,
//! and moving it runs nothing. A picture is shown from its commit by `raw/<commit>/<name>`,
//! which GitHub draws for whoever can read the repo.
//!
//! **A branch of that name purlis did not make is never built on.** Anyone who can push could
//! put one there first, holding a workflow, and the person's own push of the next proof would
//! carry it and run it as them. So before anything is written, the branch's last commit is
//! read: it must have at most one parent, and its tree must hold only pictures purlis names,
//! `browser-proof-<n>.png` blobs at the top, the listing whole. Anything else is refused.

use super::super::backend::{Asker, Caller};
use super::super::pr::{Image, Pr, is_object_name, is_proof_name, proof_branch};
use super::super::transport::{Call, Field, Method};
use super::super::{Failure, ForgeError, LIST_TIMEOUT, quote};
use super::owner_name;

/// Keep `images` on `pr`'s proof branch in `path`, and answer each one's address.
pub(super) fn keep(
    asker: &Asker,
    caller: &Caller,
    path: &str,
    pr: &Pr,
    images: &[Image],
) -> Result<Vec<String>, ForgeError> {
    use base64::Engine as _;
    let (owner, name) = owner_name(path);
    let repo = format!("repos/{}/{}", quote(owner), quote(name));
    let branch = proof_branch(pr.number);
    let doing = format!(
        "keeping pull request #{}'s browser proof in {path}",
        pr.number
    );
    let sha_of = |record: &serde_json::Value, key: &str| -> Result<String, ForgeError> {
        let sha = record[key].as_str().unwrap_or_default();
        if is_object_name(sha) {
            Ok(sha.to_string())
        } else {
            Err(ForgeError::new(format!(
                "{doing}: the answer named no {key}"
            )))
        }
    };
    // The branch's last commit, if it is there: a later proof builds on it, once its tree is
    // seen to hold only proof. Read before anything is written.
    let reference = Call::get(format!("{repo}/git/ref/heads/{branch}"), LIST_TIMEOUT);
    let parent = match asker.ask(caller, &reference, &doing) {
        Ok(record) => Some(sha_of(&record["object"], "sha")?),
        Err(why) if *why.failure() == Failure::NotFound => None,
        Err(why) => return Err(why),
    };
    let not_ours = || {
        ForgeError::new(format!(
            "the branch {branch} of {path} holds what purlis did not put there, so no proof is \
             added to it"
        ))
    };
    let mut fields = Vec::new();
    if let Some(parent) = &parent {
        let commit = Call::get(format!("{repo}/git/commits/{parent}"), LIST_TIMEOUT);
        let record = asker.ask(caller, &commit, &doing)?;
        if record["parents"]
            .as_array()
            .is_none_or(|parents| parents.len() > 1)
        {
            return Err(not_ours());
        }
        let tree = sha_of(&record["tree"], "sha")?;
        let listing = Call::get(format!("{repo}/git/trees/{tree}"), LIST_TIMEOUT);
        let listed = asker.ask(caller, &listing, &doing)?;
        let only_proof = listed["truncated"].as_bool() == Some(false)
            && listed["tree"].as_array().is_some_and(|entries| {
                entries.iter().all(|entry| {
                    entry["type"].as_str() == Some("blob")
                        && entry["mode"].as_str() == Some("100644")
                        && entry["path"].as_str().is_some_and(is_proof_name)
                })
            });
        if !only_proof {
            return Err(not_ours());
        }
        fields.push(Field::text("base_tree", &tree));
    }
    let mut blobs = Vec::new();
    for image in images {
        let content = base64::engine::general_purpose::STANDARD.encode(&image.png);
        let call = Call::write(
            Method::Post,
            format!("{repo}/git/blobs"),
            vec![
                Field::text("content", &content),
                Field::text("encoding", "base64"),
            ],
        );
        blobs.push(sha_of(&asker.ask(caller, &call, &doing)?, "sha")?);
    }
    for (image, blob) in images.iter().zip(&blobs) {
        fields.extend([
            Field::text("tree[][path]", &image.name),
            Field::text("tree[][mode]", "100644"),
            Field::text("tree[][type]", "blob"),
            Field::text("tree[][sha]", blob),
        ]);
    }
    let tree_call = Call::write(Method::Post, format!("{repo}/git/trees"), fields);
    let tree = sha_of(&asker.ask(caller, &tree_call, &doing)?, "sha")?;
    let mut fields = vec![
        Field::text(
            "message",
            &format!("Browser proof for pull request #{}", pr.number),
        ),
        Field::text("tree", &tree),
    ];
    if let Some(parent) = &parent {
        fields.push(Field::text("parents[]", parent));
    }
    let commit_call = Call::write(Method::Post, format!("{repo}/git/commits"), fields);
    let commit = sha_of(&asker.ask(caller, &commit_call, &doing)?, "sha")?;
    let moved = match &parent {
        // Only onto the commit just read: a branch that moved since is refused, never forced.
        Some(_) => Call::write(
            Method::Patch,
            format!("{repo}/git/refs/heads/{branch}"),
            vec![Field::text("sha", &commit), Field::typed("force", "false")],
        ),
        None => Call::write(
            Method::Post,
            format!("{repo}/git/refs"),
            vec![
                Field::text("ref", &format!("refs/heads/{branch}")),
                Field::text("sha", &commit),
            ],
        ),
    };
    asker.ask(caller, &moved, &doing)?;
    let host = &asker.forge.host;
    Ok(images
        .iter()
        .map(|image| {
            format!(
                "https://{host}/{}/{}/raw/{commit}/{}",
                quote(owner),
                quote(name),
                quote(&image.name)
            )
        })
        .collect())
}
