// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the chat is told** when a request of it is a held act (ADR 0128 D8.1): the one sentence
//! the proxy's 403 carries, and the reason when nobody was asked.
//!
//! Every sentence says that purlis holds the act, that the chat did not perform it, and what
//! happens next. None says the act will happen: it happens when the person presses, which a yes
//! in the chat is not.

use super::{NOBODY_HERE, Refused, Requested, Why};

/// The noun an act is told with, and the verb of its press.
fn words(requested: &Requested) -> (&'static str, &'static str, &'static str) {
    match requested {
        Requested::Merge(_) => ("merges", "merge", "the merge"),
        Requested::Tag { .. } => ("tags", "tag", "the tag"),
        Requested::Release { .. } => ("releases", "release", "the release"),
        Requested::CiPush { .. } => ("pushes that change CI files", "push", "the push"),
        Requested::Fork { .. } => ("forks", "fork", "the fork"),
    }
}

/// **The 403's one sentence for a held act the person has been asked about.**
///
/// For a merge it is ADR 0128 D8.1's: *"purlis holds merges. It has asked the person; the merge
/// happens when they approve it in Asks."*
pub fn asked(requested: &Requested) -> String {
    let (nouns, _, the) = words(requested);
    format!(
        "purlis holds {nouns}. It has asked the person; {the} happens when they approve it in \
         Asks."
    )
}

/// **The 403's sentence where nobody was asked**, because `why`.
pub fn not_asked(requested: &Requested, why: &Refused) -> String {
    let (nouns, _, the) = words(requested);
    match why {
        Refused::Unattended { .. } => format!("purlis holds {nouns}. {NOBODY_HERE}"),
        _ => format!(
            "purlis holds {nouns}. Nobody was asked, so {the} will not happen: {}",
            refused(why)
        ),
    }
}

/// **The 403's sentence for a request refused whole**, held for nobody.
pub fn refused_request(why: &Why) -> String {
    match why {
        Why::TagMoved => {
            "purlis never moves a tag that exists. Make a new tag with another name.".to_owned()
        }
        Why::TagDeleted => {
            "purlis never deletes a tag for a chat. Ask the person to delete it on the forge."
                .to_owned()
        }
        Why::ForkElsewhere => {
            "purlis forks only into the person's own account, so a fork into another organization \
             or namespace is refused."
                .to_owned()
        }
        Why::NotRecorded => {
            "This repo is not one the person recorded for this chat, so purlis does not act on it."
                .to_owned()
        }
        Why::Unreadable(what) => {
            format!("purlis could not read this request ({what}), so it did nothing.")
        }
    }
}

/// A reason an act is not offered, as a sentence.
pub fn refused(why: &Refused) -> String {
    match why {
        Refused::NotRecorded(repo) => format!(
            "{} is not a repo the person recorded for this chat.",
            repo.as_str()
        ),
        Refused::TagExists { name, at } => format!(
            "The tag {} exists, at {}. purlis never moves a tag: use another name.",
            name.as_str(),
            &at.as_str()[..12]
        ),
        Refused::NoSuchCommit { rev, why } => {
            format!("purlis could not find the commit {rev} on the forge: {why}.")
        }
        Refused::NoCiFiles => "This push changes no CI file, so no press is needed.".to_owned(),
        Refused::NoWorkflowCredential(why) => format!(
            "A push that changes CI files is not offered: {why}. The person can make that \
             sign-in in the window."
        ),
        Refused::CiDiff(why) => format!(
            "A push that may change CI files is refused, because its changes could not be \
             shown: {why}"
        ),
        Refused::ForkNotAllowed(repo) => format!(
            "The host does not let the person fork {}. Ask an owner to allow forks.",
            repo.as_str()
        ),
        Refused::Forge(why) => format!("The forge did not say: {why}."),
        Refused::Unattended { .. } => NOBODY_HERE.to_owned(),
    }
}
