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
//! - [`outgoing`]: what an export or a publish sends, and the export; [`publish`]: the request.
//! - [`lock`]: the lockfiles; [`write`]: every write, through handles that follow no link.
//! - [`checked`]: when each source was last checked, and how many skills it offered then.
//!
//! **What is installed is what was reviewed.** The review answers the commit and a digest of
//! each skill's files; an install is handed both back ([`Accepted`]), fetches that commit again,
//! and refuses where any digest differs. The pin moves only through an accepted update.
//!
//! [ADR 0100]: ../../../../docs/adr/0100-a-skill-is-installed-from-a-git-source-as-a-pinned-copy-and-never-updates-by-itself.md

use purlis_skills::tree;
use serde::{Deserialize, Serialize};

pub mod carried;
pub mod checked;
pub mod extension;
pub mod fetch;
pub mod install;
pub mod listed;
pub mod lock;
pub mod outgoing;
pub mod policy;
pub mod publish;
pub mod review;
pub mod update;
pub mod widen;
mod write;

pub use carried::SourceGrade;
pub use fetch::{Fetched, Found, MOST_SKILL_BYTES, NotASkill, SourceFile, fetch, newest};
pub use install::{Renames, install, install_named};
pub use listed::{Listed, TABLE, add, confirm, remove, sources};
pub use lock::{Kept, Locked, Lockfile};
pub use outgoing::{Outgoing, Purpose, Stop, export_folder, export_zip, gather, gather_set};
pub use review::{
    Accepted, AcceptedSkill, Browse, Clash, Offered, Review, ReviewFile, SetReview, SkillReview,
    TooMuch, browse, linted, review, review_all, review_set, review_set_without, review_without,
};
pub use update::{
    Changed, FileChange, Offer, Unasked, Unconfirmed, UpdateReview, Updates, line_diff, update,
    update_review, updates,
};
pub use widen::{Widening, added, widenings};

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
    /// **Whether reading `skill`'s files keeps within what one read of the source may take**
    /// (#4024), or what the read's limit says: asked before each skill of a review is read, so a
    /// review past it stops before a skill and says which of those named fit, never cut part way.
    /// A source with no such limit always fits.
    fn fits_one_read(&self, _skill: &Found) -> Result<(), String> {
        Ok(())
    }
    /// **The grade file it carries beside `skill`'s folder** (ADR 0100, amended 2026-11-08), as
    /// bytes, `None` where it carries none, or why it could not be read. A source that carries no
    /// grades answers `None`.
    fn grade(&self, _skill: &Found) -> Option<Result<Vec<u8>, String>> {
        None
    }
    /// **The tree id of `skill`'s folder, whose files are `files`**, that a grade it carries is
    /// compared with: worked out from the files, where the source holds no git tree of them.
    fn folder_tree(&self, _skill: &Found, files: &[SourceFile]) -> String {
        carried::tree_of(files)
    }
    /// **Whose a grade it carries is**: the short word a person knows it by ([`Origin::label`]).
    fn whose(&self) -> String {
        self.shown()
    }
    /// **When what it answers was made**, in seconds since 1970, that an org's minimum age is
    /// held to ([`policy`]): a git source's commit's committer time. `None` where it cannot say,
    /// which an org's minimum age refuses.
    fn made(&self) -> Option<u64> {
        None
    }
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

/// **A revision an extension's source names what it holds by** (ADR 0100 §8): opaque, checked,
/// at most [`crate::extension::studio::MOST_REVISION_BYTES`] letters, digits and `.-_:+`. What
/// takes a commit's place in the pin of a skill copied from it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Revision(String);

impl Revision {
    /// `text` as a revision, or `None` where it cannot be one.
    pub fn parse(text: &str) -> Option<Revision> {
        (!text.is_empty()
            && text.len() <= crate::extension::studio::MOST_REVISION_BYTES
            && text
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':' | '+')))
        .then(|| Revision(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Revision {
    type Error = String;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        Revision::parse(&text).ok_or_else(|| format!("{text:?} is not a revision purlis reads"))
    }
}

impl From<Revision> for String {
    fn from(revision: Revision) -> String {
        revision.0
    }
}

impl std::fmt::Display for Revision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// **What a copy is pinned to**: a git source's commit, or the revision an extension's source
/// named what it held by (ADR 0100 §8).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pin {
    Commit(Commit),
    Revision(Revision),
}

impl Pin {
    /// Its whole name.
    pub fn as_str(&self) -> &str {
        match self {
            Pin::Commit(commit) => commit.as_str(),
            Pin::Revision(revision) => revision.as_str(),
        }
    }

    /// As a sentence shows it: a commit's first twelve digits, a revision whole.
    pub fn short(&self) -> &str {
        match self {
            Pin::Commit(commit) => commit.short(),
            Pin::Revision(revision) => revision.as_str(),
        }
    }

    /// The commit, where it is a git source's pin.
    pub fn commit(&self) -> Option<&Commit> {
        match self {
            Pin::Commit(commit) => Some(commit),
            Pin::Revision(_) => None,
        }
    }
}

impl std::fmt::Display for Pin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// **Where an installed skill was copied from** (ADR 0100 §8): a git repository and its branch,
/// or the extension whose SkillSource answered it, by id. What a pin is kept by, and what the
/// skills installed from one source are found by.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Origin {
    Git(Remote),
    Extension(String),
}

impl Origin {
    /// **The short word a person knows it by** ([`Remote::label`]); an extension's id, spelled
    /// as a skill's name may start.
    pub fn label(&self) -> String {
        match self {
            Origin::Git(remote) => remote.label(),
            Origin::Extension(id) => spelled(id),
        }
    }

    /// The git source, where it is one.
    pub fn remote(&self) -> Option<&Remote> {
        match self {
            Origin::Git(remote) => Some(remote),
            Origin::Extension(_) => None,
        }
    }

    /// Whether it is the git source `remote`.
    pub fn is(&self, remote: &Remote) -> bool {
        self.remote() == Some(remote)
    }
}

/// `origin` as a person reads it: a git source's URL and branch, or `the extension '<id>'`.
impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Origin::Git(remote) => write!(f, "{remote}"),
            Origin::Extension(id) => write!(f, "the extension '{id}'"),
        }
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

    /// **The short word a person knows it by** (ADR 0100, amended 2026-11-01): the owner in an
    /// `https://` URL (`mattpocock` of `https://github.com/mattpocock/skills`), its host where the
    /// path names none, or a folder's own name, as a skill's name may spell it. Shown beside a
    /// skill in a list, and the start of the new name a clash offers. Never told to a chat.
    pub fn label(&self) -> String {
        let url = self.url.trim_end_matches('/');
        let word = match url.strip_prefix("https://") {
            Some(rest) => {
                let mut parts = rest.split('/');
                let host = parts.next().unwrap_or_default();
                parts.next().unwrap_or(host)
            }
            None => url
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .trim_end_matches(".git"),
        };
        spelled(word)
    }

    /// Checks it, as a sentence where it cannot be used. `committed` holds it to what a file
    /// the team writes may name: an `https://` URL alone, never a folder on this machine.
    pub(crate) fn check(&self, committed: bool) -> Result<(), String> {
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

/// **`word` spelled as the start of a skill's name**: lowercase, each run of anything but a
/// letter or a digit a hyphen, `source` where nothing is left. What [`Remote::label`] answers,
/// and how a Deleted group's `from` starts the name it offers (#5457).
pub fn spelled(word: &str) -> String {
    let spelled: Vec<String> = word
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect();
    if spelled.is_empty() {
        "source".to_owned()
    } else {
        spelled.join("-")
    }
}

#[cfg(test)]
mod tests;
