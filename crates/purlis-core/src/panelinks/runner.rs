// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A file path a runner chat prints** (#6387): looked for on the runner, never on this machine,
//! and opened read-only in the light editor, read from there.
//!
//! - **One neutral model, one adapter per runner kind.** [`RunnerFile`] is the link kind: it
//!   reads the chat's text into a path, asks the runner about it, and decides what opening it
//!   does. How a runner is asked is its kind's adapter ([`RunnerFiles`]); a Docker pool's runner
//!   is the first, asked over the session protocol's `read_file`.
//! - **The path crosses as data**, without its `:line:col`, and is resolved on the runner, from
//!   the chat's own folder there, every link followed on that side. The runner says whether the
//!   file is in the chat's workspace there; one that is not asks first, as a file outside the
//!   workspace on this machine does, naming the runner.
//! - **Only a plain file of text links**: what the runner does not find, or finds to be a
//!   folder, a device or a pipe, says nothing; a binary file or one past the largest read
//!   (`MOST_FILE_READ`) is no link and says why. A runner that does not answer in time says nothing either: the window waits on
//!   no runner to draw a pane.
//! - **What the runner keeps is no link and says why** ([`RUNNER_KEPT`], #7064): its own
//!   folders, a harness's sign-in and what no chat there reads, never asked for and never opened.
//! - **`~` is never asked**: on a runner it is the runner's home, which holds its sign-ins.

use std::path::{Path, PathBuf};

use purlis_session_protocol::session::{FileRead, MOST_FILE_READ};

use super::file::{at_line, at_suffix, on_this_machine};
use super::{
    Asking, Asks, FileAt, Home, HomeIs, Kind, LinkKind, Opening, Pane, ReadFrom, Resolved, Runs,
    Target,
};

/// **How the window asks one runner about its chat's files**: one adapter per kind of runner.
pub trait RunnerFiles: Send + Sync {
    /// The runner's name, as the window shows it.
    fn runner(&self) -> &str;
    /// What `path`, as the chat printed it without its `:line:col`, is on the runner, looked at
    /// and not read: `None` where the runner could not be asked, or did not answer in time.
    fn look(&self, path: &str) -> Option<FileRead>;
}

/// What a hover over a runner's binary file says.
pub const RUNNER_BINARY: &str =
    "purlis opens a runner's file only as text, and this one is not text";

/// What a hover over a file a runner keeps from the window says (#7064): its own folders, a
/// harness's sign-in, and what no chat there reads. The runner chat's tab says it too.
pub const RUNNER_KEPT: &str = "purlis never opens its own folders on a runner, a harness's \
                               sign-in files, or what no chat there reads";

/// What a hover over a runner's file says where the runner could not tell whether it keeps it
/// (#7064, #7077): its list of what it keeps could not be read whole, or the file has several
/// names and more places to look for them than the runner looks in. The tab says it too.
pub const RUNNER_UNSURE: &str = "purlis could not tell whether this runner keeps this file from \
                                 its window, so it does not open it";

/// What a hover over a runner's file past the largest read ([`MOST_FILE_READ`]) says.
pub fn runner_too_large() -> String {
    format!(
        "purlis opens a runner's file only up to {} MiB, and this one is larger",
        MOST_FILE_READ >> 20
    )
}

/// **A file path a runner chat printed** (#6387): read on its runner through `files`.
pub struct RunnerFile {
    files: Box<dyn RunnerFiles>,
}

impl RunnerFile {
    /// Paths read on the runner `files` asks.
    pub fn new(files: Box<dyn RunnerFiles>) -> Self {
        Self { files }
    }
}

impl LinkKind for RunnerFile {
    fn kind(&self) -> Kind {
        Kind::File
    }

    /// A runner chat's pane alone.
    fn serves(&self, pane: &Pane) -> bool {
        pane.runs == Runs::OnRunner
    }

    fn resolve(&self, text: &str, pane: &Pane) -> Resolved {
        if pane.runs != Runs::OnRunner
            || text.is_empty()
            || text.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            return Resolved::Quiet;
        }
        let (words, line, column) = at_line(text);
        let named = match words.strip_prefix("file:") {
            Some(_) => match on_this_machine(words) {
                Ok(path) => match path.to_str() {
                    Some(path) => path.to_owned(),
                    None => return Resolved::Quiet,
                },
                Err(why) => return why,
            },
            None => words.to_owned(),
        };
        if named.starts_with('~') {
            return Resolved::Quiet;
        }
        let (path, from_workspace, home, version) = match self.files.look(&named) {
            Some(FileRead::Plain {
                path,
                from_workspace,
                home,
                version,
                ..
            }) => (path, from_workspace, home, version),
            Some(FileRead::Binary { .. }) => return Resolved::NotALink(RUNNER_BINARY.into()),
            Some(FileRead::TooLarge { .. }) => {
                return Resolved::NotALink(runner_too_large());
            }
            Some(FileRead::Kept { .. }) => return Resolved::NotALink(RUNNER_KEPT.into()),
            Some(FileRead::Unsure { .. }) => return Resolved::NotALink(RUNNER_UNSURE.into()),
            Some(FileRead::NotThere | FileRead::NotPlain { .. } | FileRead::Outside { .. })
            | None => return Resolved::Quiet,
        };
        let at = at_suffix(line, column);
        let shown = format!("{path}{at}");
        let short = from_workspace
            .as_deref()
            .filter(|from| !from.is_empty())
            .map_or_else(|| shown.clone(), |from| format!("{from}{at}"));
        Resolved::Link(Target::File(Box::new(FileAt {
            path: PathBuf::from(path),
            line,
            column,
            read_from: ReadFrom::Runner {
                runner: self.files.runner().to_owned(),
                home: home.map(|home| Home {
                    is: home.workspace.map_or(HomeIs::Project, HomeIs::Workspace),
                    folder: PathBuf::from(home.folder),
                }),
                identity: super::onrunner::identity_of(&version).map(str::to_owned),
            },
            in_home: from_workspace.is_some(),
            shown,
            short,
        })))
    }

    fn opening(&self, target: Target, text: &str, _pane: &Pane) -> Opening {
        let Target::File(file) = &target else {
            return Opening::Refused("purlis does not open that as a file".into());
        };
        if file.in_home {
            return Opening::Open(target);
        }
        let ReadFrom::Runner { home, .. } = &file.read_from else {
            return Opening::Refused("purlis does not open that as a runner's file".into());
        };
        Opening::Ask {
            reason: format!(
                "outside {} on runner {}",
                home.as_ref().map_or("the chat's workspace", Home::said),
                self.files.runner()
            ),
            shown: text.to_owned(),
            target,
            token: None,
            about: Asking::FileOutside,
        }
    }

    /// A file outside the workspace opened on an answered ask is kept as opened for the pane on
    /// that runner, as the file the look that found it again saw, so its tab may read it there
    /// and nothing put in its place since.
    fn accepted(&self, target: &Target, pane: &Pane, asks: &Asks) {
        let Target::File(file) = target else {
            return;
        };
        let ReadFrom::Runner {
            runner,
            identity: Some(identity),
            ..
        } = &file.read_from
        else {
            return;
        };
        if let Some(path) = Path::new(&file.path).to_str() {
            asks.keep_opened_on_runner(pane, runner, path, identity);
        }
    }
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
