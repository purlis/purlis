// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Skill sources** (SK-10, #2773; [ADR 0100]; spec #2762 decisions 7, 20 and 39): a git
//! repository of skill folders that a person adds, browses, and installs skills from, each install
//! a reviewed copy pinned to a commit in the project's record of installed skills, and each newer
//! commit offered as a reviewed diff, never applied by itself. One copy of each verb, which the
//! Skill studio and `purlis skill install | update` both call.
//!
//! - [`listed`]: the sources a project browses, a setting, and which of them this machine
//!   confirmed.
//! - [`fetch`]: a source fetched at one commit into a scratch repository and read from git's
//!   object store, never checked out, within fixed limits.
//! - [`review`]: the install review, summary first, and Browse.
//! - [`install`] and [`update`]: the copy, the pin, and each newer commit's review.
//! - [`lock`]: the lockfiles; [`write`]: every write, through handles that follow no link.
//!
//! **What is installed is what was reviewed.** The review answers the commit and a digest of
//! each skill's files; an install is handed both back ([`Accepted`]), fetches that commit again,
//! and refuses where any digest differs. The pin moves only through an accepted update.
//!
//! [ADR 0100]: ../../../../docs/adr/0100-a-skill-is-installed-from-a-git-source-as-a-pinned-copy-and-never-updates-by-itself.md

use purlis_skills::tree;
use serde::{Deserialize, Serialize};

pub mod extension;
pub mod fetch;
pub mod install;
pub mod listed;
pub mod lock;
pub mod review;
pub mod update;
mod write;

pub use fetch::{Fetched, Found, MOST_SKILL_BYTES, NotASkill, SourceFile, fetch, newest};
pub use install::install;
pub use listed::{Listed, TABLE, add, confirm, remove, sources};
pub use lock::{Kept, Locked, Lockfile};
pub use review::{
    Accepted, AcceptedSkill, Browse, Offered, Review, ReviewFile, SkillReview, browse, review,
};
pub use update::{
    Changed, FileChange, Offer, Unasked, Unconfirmed, UpdateReview, Updates, line_diff, update,
    update_review, updates,
};

/// **A skill source** (the SkillSource capability, ADR 0101 §1): what skills it offers, and one
/// skill's files. The git source ([`Fetched`]) and each extension that declares `skill-source`
/// ([`extension`]) answer through it, and everything after is purlis's: the review
/// ([`review`]), the copy and the pin.
pub trait SkillSource {
    /// What a sentence names it by.
    fn shown(&self) -> String;
    /// The skills it offers, and each folder it holds that is not a skill purlis offers, with why.
    fn skills(&self) -> Result<(Vec<Found>, Vec<NotASkill>), String>;
    /// One offered skill's files, and the entries of its folder that are not installed.
    fn files(
        &self,
        skill: &Found,
    ) -> Result<(Vec<SourceFile>, Vec<purlis_skills::tree::LeftOut>), String>;
}

/// **A commit's full name**, checked: 40 or 64 hex digits, lowercased.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Commit(String);

impl Commit {
    /// `text` as a commit's name, or `None` where it is not one.
    pub fn parse(text: &str) -> Option<Commit> {
        let text = text.trim();
        (matches!(text.len(), 40 | 64) && text.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| Commit(text.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Its first twelve digits, as a sentence shows it.
    pub fn short(&self) -> &str {
        self.0.get(..12).unwrap_or(&self.0)
    }
}

impl TryFrom<String> for Commit {
    type Error = String;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        Commit::parse(&text).ok_or_else(|| format!("{text:?} is not a commit's name"))
    }
}

impl From<Commit> for String {
    fn from(commit: Commit) -> String {
        commit.0
    }
}

impl std::fmt::Display for Commit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// **A repository and the branch read from it**: what a pin and a confirmation are kept by.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Remote {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

impl Remote {
    /// The ref its newest commit is at: its branch's, or the repository's own `HEAD`.
    fn tip_ref(&self) -> String {
        match &self.branch {
            Some(branch) => format!("refs/heads/{branch}"),
            None => "HEAD".to_owned(),
        }
    }

    /// Checks it, as a sentence where it cannot be used. `committed` holds it to what a file
    /// the team writes may name: an `https://` URL alone, never a folder on this machine.
    fn check(&self, committed: bool) -> Result<(), String> {
        if let Some(why) = url_problem(&self.url, committed) {
            return Err(format!("{:?} cannot be a skill source: {why}", self.url));
        }
        if let Some(why) = self.branch.as_deref().and_then(branch_problem) {
            return Err(why);
        }
        Ok(())
    }
}

/// `remote` as a person reads it: its URL, then its branch where it names one.
impl std::fmt::Display for Remote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.url)?;
        if let Some(branch) = &self.branch {
            write!(f, " ({branch})")?;
        }
        Ok(())
    }
}

/// **How a private source signs in**: the vault and the entry in it that holds the token, and the
/// user name the forge wants beside a token, where it wants one. Never the token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultEntry {
    pub vault: String,
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// **A skill source** (ADR 0100 §1): a repository and its branch, optionally the folder inside it
/// the skills are in, and how a private one signs in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    #[serde(flatten)]
    pub remote: Remote,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign_in: Option<VaultEntry>,
}

impl Source {
    /// **Checks the source** the person named: its URL, its branch and its folder, as a sentence
    /// where one cannot be used.
    pub fn check(&self) -> Result<(), String> {
        self.remote.check(false)?;
        if self.folder_parts().is_none() {
            return Err(format!(
                "{:?} is not a folder purlis reads in a source",
                self.folder.as_deref().unwrap_or_default()
            ));
        }
        if let Some(sign_in) = &self.sign_in
            && (sign_in.vault.is_empty() || sign_in.key.is_empty())
        {
            return Err("a private source names both its vault and the entry in it".to_owned());
        }
        Ok(())
    }

    /// Its folder's parts, or `None` for one that is not a plain path.
    fn folder_parts(&self) -> Option<Vec<String>> {
        tree::folder_parts(self.folder.as_deref().unwrap_or_default())
    }
}

/// `source` as a person reads it: its URL, then its branch and folder where it names them.
impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.remote)?;
        if let Some(folder) = self.folder.as_deref().filter(|f| !f.is_empty()) {
            write!(f, ", {folder}")?;
        }
        Ok(())
    }
}

/// Why `url` cannot be a skill source's, or `None` where it can: an `https://` URL with a plain
/// host and no credential in it ([`crate::savefix::https_host`]), or, unless it is `committed`,
/// a `file://` URL or a folder on this machine by its absolute path.
fn url_problem(url: &str, committed: bool) -> Option<String> {
    if url.is_empty() {
        return Some("it is empty".to_owned());
    }
    if url.len() > 2048 || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Some("it holds a blank or a control character".to_owned());
    }
    if url.starts_with('-') || url.contains("::") {
        return Some("it is not a URL git reads as a place".to_owned());
    }
    if let Some(rest) = url.strip_prefix("https://") {
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        if authority.contains('@') {
            return Some(
                "it carries a sign-in, which a source never does: name a vault entry for it \
                 instead"
                    .to_owned(),
            );
        }
        return crate::savefix::https_host(url)
            .is_none()
            .then(|| "it names no plain host".to_owned());
    }
    if committed {
        return Some(
            "a source the team's files name is an https:// URL, never a folder on one machine"
                .to_owned(),
        );
    }
    if url.starts_with("file:///") || url.starts_with('/') {
        return None;
    }
    Some(
        "a skill source is an https:// URL, or a folder on this machine by its full path"
            .to_owned(),
    )
}

/// Why `branch` cannot be a source's branch, or `None`.
fn branch_problem(branch: &str) -> Option<String> {
    let plain = !branch.is_empty()
        && branch.len() <= 200
        && branch
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._/-".contains(c))
        && !branch.starts_with(['-', '/', '.'])
        && !branch.ends_with(['/', '.'])
        && !branch.ends_with(".lock")
        && !branch.contains("..")
        && !branch.contains("//")
        && !branch.contains("/.");
    (!plain).then(|| format!("{branch:?} is not a branch name purlis reads"))
}

#[cfg(test)]
mod tests;
