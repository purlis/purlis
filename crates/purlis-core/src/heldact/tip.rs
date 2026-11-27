// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a held CI-file push sends** (ADR 0128 D8.3): the files its tip changes against its
//! base, read from the app's clone by git, so a forge adapter can make them one commit on the
//! base through the forge's REST API with the person's own sign-in.
//!
//! **A tip that does not contain its base sends nothing** (#7690): one commit on the base with the
//! tree diff of a tip cut from an older point would undo everything the base gained since. The
//! files sent are the diff from the merge-base of the tip and the base, which is then the base
//! itself, to the tip: exactly the branch's own changes.
//!
//! Only plain text files cross: a change that is not (a binary file, a symlink, a submodule, a
//! path git does not show as text) is an error and nothing is pushed, because the person was
//! shown a diff and can only be shown text. At most [`MOST_FILES`] files and [`MOST_BYTES`] bytes.

use super::ci::GitTrees;
use crate::mergehold::HeadSha;

/// The most files one held push sends.
pub const MOST_FILES: usize = 200;

/// The most bytes of content one held push sends.
pub const MOST_BYTES: usize = 4 * 1024 * 1024;

/// One file a push changes, as the tip has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TipChange {
    pub path: String,
    /// The tip's text of the file, or `None` where the tip removes it.
    pub content: Option<String>,
    /// Whether the tip's file is executable.
    pub executable: bool,
    /// Whether the base has no such file.
    pub added: bool,
}

/// **What the person and the chat are told where a tip does not contain `base`**, the commit the
/// push would be made on, named as they know it.
pub fn behind(base: &str) -> String {
    format!("the branch is behind {base}: run `purlis worktree update --merge` and push again")
}

/// **The files a tip changes against its base**, as a push of them is made through a forge's API.
pub trait TipFiles: Send + Sync {
    /// Every file whose content or mode differs between the trees of `base` and `tip`: **an error,
    /// and nothing read, where `tip` does not contain `base`** ([`behind`]), so the one commit
    /// made on `base` carries the tip's own changes and never undoes one of `base`'s (#7690).
    fn changes(&self, base: &HeadSha, tip: &HeadSha) -> Result<Vec<TipChange>, String>;
    /// The message of the commit `tip`, which the one commit made on the base carries.
    fn message(&self, tip: &HeadSha) -> Result<String, String>;
}

impl TipFiles for super::ci::NoClone {
    fn changes(&self, _: &HeadSha, _: &HeadSha) -> Result<Vec<TipChange>, String> {
        Err("no clone".to_owned())
    }
    fn message(&self, _: &HeadSha) -> Result<String, String> {
        Err("no clone".to_owned())
    }
}

impl TipFiles for GitTrees {
    fn changes(&self, base: &HeadSha, tip: &HeadSha) -> Result<Vec<TipChange>, String> {
        // The one commit is made on `base` with these files. Were `base` not in the tip's history,
        // the diff would hold the undoing of every change `base` gained since the tip's own
        // merge-base with it, and that commit would revert them (#7690). Where the tip contains
        // `base`, `base` is that merge-base, and the diff is exactly the tip's own changes; no path
        // can differ between the merge-base and the parent, because they are one commit.
        if self.commits_missing(base, tip)? > 0 {
            return Err(behind(&format!("its base {}", &base.as_str()[..12])));
        }
        // A listing past what a push may hold is never read whole.
        let out = self.git_capped(
            &[
                "diff-tree",
                "-r",
                "-z",
                "--raw",
                "--no-renames",
                "--no-textconv",
                "--no-ext-diff",
                base.as_str(),
                tip.as_str(),
            ],
            MOST_BYTES,
        )?;
        let mut changes = Vec::new();
        let mut bytes = 0usize;
        let mut tokens = out.split('\0').filter(|token| !token.is_empty());
        while let Some(meta) = tokens.next() {
            let path = tokens
                .next()
                .ok_or_else(|| "git's diff ended inside an entry".to_owned())?;
            // `:<old mode> <new mode> <old sha> <new sha> <status>`
            let fields: Vec<&str> = meta.trim_start_matches(':').split(' ').collect();
            let [old_mode, new_mode, _, _, status] = fields[..] else {
                return Err("git's diff was not in the form purlis reads".to_owned());
            };
            if path.contains('\u{fffd}') {
                return Err("a changed path is not text purlis can send".to_owned());
            }
            let removed = status.starts_with('D');
            let mode = if removed { old_mode } else { new_mode };
            if !matches!(mode, "100644" | "100755") {
                return Err(format!(
                    "{path} is a symlink or a submodule, which purlis does not push through the \
                     forge's API"
                ));
            }
            let content = if removed {
                None
            } else {
                let left = MOST_BYTES.saturating_sub(bytes);
                let blob = format!("{}:{path}", tip.as_str());
                // Read to what is left of the budget and no further: git is stopped past it.
                let text = self
                    .git_capped(&["cat-file", "blob", &blob], left + 1)
                    .map_err(|why| {
                        format!("{path} cannot be read within the push's size: {why}")
                    })?;
                if text.len() > left {
                    return Err(too_much());
                }
                if text.contains('\u{fffd}') || text.contains('\0') {
                    return Err(format!(
                        "{path} is not text, and purlis pushes only text through the forge's API"
                    ));
                }
                bytes += text.len();
                Some(text)
            };
            changes.push(TipChange {
                path: path.to_owned(),
                content,
                executable: mode == "100755",
                added: status.starts_with('A'),
            });
            if changes.len() > MOST_FILES || bytes > MOST_BYTES {
                return Err(too_much());
            }
        }
        Ok(changes)
    }

    fn message(&self, tip: &HeadSha) -> Result<String, String> {
        let said = self
            .git_capped(
                &["log", "-1", "--format=%B", tip.as_str()],
                MOST_MESSAGE * 4,
            )
            .map_err(|why| {
                format!("the commit message is too long to show or cannot be read: {why}")
            })?;
        // What is shown is what is sent: nothing that draws nothing or turns the text around.
        let said: String = said
            .replace("\r\n", "\n")
            .chars()
            .filter(|c| !super::prepare::unseen(*c))
            .collect();
        let said = said.trim();
        if said.chars().count() > MOST_MESSAGE {
            return Err(format!(
                "the commit message is longer than {MOST_MESSAGE} characters, more than purlis \
                 shows"
            ));
        }
        Ok(if said.is_empty() {
            "Update".to_owned()
        } else {
            said.to_owned()
        })
    }
}

/// The most characters of a commit message a held push shows and sends.
pub const MOST_MESSAGE: usize = 2_000;

fn too_much() -> String {
    format!(
        "the push changes more than {MOST_FILES} files or {} MB, more than purlis sends through \
         the forge's API",
        MOST_BYTES / (1024 * 1024)
    )
}
