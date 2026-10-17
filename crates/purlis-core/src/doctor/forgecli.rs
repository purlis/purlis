// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `gh`/`glab` and `… auth` rows: the forge's own CLI and its sign-in, for each forge the
//! project declares (FG-2, #802; #994).
//!
//! **purlis still runs the CLI.** Every forge call a `purlis` command makes, and every window
//! call on a host with no purlis sign-in, goes through `gh api` or `glab api`
//! ([`crate::forge::cli`], ADR 0070 §5), and a clone's credential helper is the CLI's own. So
//! the rows check the program and the login it holds, and never the token: the CLI is found the
//! way every forge call finds it ([`crate::forge::find_cli`]), and its sign-in is its own
//! answer to `auth status` ([`Transport::check_auth`]), which purlis reads only as signed in or
//! not.
//!
//! **Asked only for a forge the project's repos are on.** A forge no workspace repo has its
//! origin on has nothing here that uses its CLI yet, so its rows say that and look for nothing.
//! Where a workspace could not be read, whether one is cannot be told, and the rows say they did
//! not check rather than guess. `purlis discover` and `purlis clone` find the CLI when they
//! run, and say so when it is not there. That also keeps the rows' answer about the project and
//! not about whichever `gh` the machine happens to have.
//!
//! **The sign-in is asked only by a doctor someone typed**: `auth status` starts the CLI and
//! can cross the network, and the preflight a chat's start runs does neither.

use super::git::git_in;
use super::{Doctor, Row, first_line, fsx};
use crate::counts::{counted, plural};
use crate::forge::transport::Transport;
use crate::forge::{self, Forge, ForgeError};

/// The forge CLI rows for the project `d` answers for: for each forge it declares, the CLI's
/// row and its sign-in's.
pub(super) fn rows(d: &Doctor) -> Vec<Row> {
    let used = used_forges(d);
    let cli = forge::cli::Cli::default();
    let mut rows = Vec::new();
    for forge in super::config::forges(d) {
        let on = Use::of(&forge, &used);
        let found = forge::find_cli(forge.kind.cli());
        rows.push(cli_row(&forge, on, found.as_deref()));
        let how = if d.preflight {
            Asking::Preflight
        } else if d.forges.is_none() {
            Asking::NoForge
        } else {
            Asking::Ask(&cli)
        };
        rows.push(auth_row(&forge, on, found.is_some(), how));
    }
    rows
}

/// Whether the project has a use for a forge's CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Use {
    /// This many workspace repos have their origin on the forge's host.
    Repos(usize),
    /// A workspace repo's origin could not be read, so it may be on the forge.
    Unknown,
    /// No workspace repo is on it.
    None,
}

impl Use {
    fn of(forge: &Forge, used: &Used) -> Self {
        match used.on.iter().filter(|on| *on == forge).count() {
            0 if used.unread => Self::Unknown,
            0 => Self::None,
            n => Self::Repos(n),
        }
    }
}

/// The forges the project's workspace repos have their origin on, one entry a repo, and whether
/// any repo's origin could not be read.
#[derive(Debug, Default)]
pub(super) struct Used {
    on: Vec<Forge>,
    unread: bool,
}

/// [`Used`] for the project `d` answers for, from each workspace clone's `origin`.
fn used_forges(d: &Doctor) -> Used {
    let mut used = Used::default();
    if !d.has_plane {
        return used;
    }
    let root = d.root.as_path();
    let Ok((workspaces, unseen)) = fsx::read_workspaces(root) else {
        used.unread = true;
        return used;
    };
    used.unread |= !unseen.is_empty();
    for ws in &workspaces {
        let Ok((clones, unseen)) = fsx::read_clones(root, ws) else {
            used.unread = true;
            continue;
        };
        used.unread |= !unseen.is_empty();
        for clone in clones {
            match git_in(&clone, &["remote", "get-url", "origin"]) {
                Ok(run) if run.ok() => {
                    if let Some(on) = forge::resolve_host(&first_line(&run.out), root) {
                        used.on.push(on);
                    }
                }
                // No origin: on no forge.
                Ok(_) => {}
                Err(_) => used.unread = true,
            }
        }
    }
    used
}

/// What the project's use of `forge` is, said.
fn on_host(forge: &Forge, on: Use) -> String {
    let host = super::one_line(&forge.host, super::DISPLAY_LIMIT);
    match on {
        Use::Repos(n) => format!(
            "{} {} {} origin on {host}",
            counted(n, "workspace repo", "workspace repos"),
            plural(n, "has", "have"),
            plural(n, "its", "their")
        ),
        Use::Unknown => format!(
            "a workspace whose repos could not be read may have one on {host}; the `workspace \
             clones` row names it"
        ),
        Use::None => format!("no workspace repo here has its origin on {host}"),
    }
}

/// The `<cli>` row: whether the CLI is found, where the project has a use for it.
pub(super) fn cli_row(forge: &Forge, on: Use, found: Option<&std::path::Path>) -> Row {
    let cli = forge.kind.cli();
    if on == Use::Unknown {
        return Row::not_checked(cli, on_host(forge, on));
    }
    if on == Use::None {
        return Row::ok(
            cli,
            format!(
                "not looked for: {}. `purlis discover` and `purlis clone` look for it when they \
                 run",
                on_host(forge, on)
            ),
        );
    }
    match found {
        Some(path) => Row::ok(
            cli,
            format!(
                "{} ({})",
                super::one_line(&path.display().to_string(), super::PATH_DISPLAY_LIMIT),
                on_host(forge, on)
            ),
        ),
        None => Row::warn(
            cli,
            format!("not found, and {}", on_host(forge, on)),
            format!(
                "Install {cli}, then run `{cli} auth login`. Every forge call a `purlis` command \
                 makes goes through it, and so does the credential helper each clone is given."
            ),
        ),
    }
}

/// How the `… auth` row may find out.
pub(super) enum Asking<'a> {
    /// The preflight a chat's start runs, which starts no forge CLI.
    Preflight,
    /// A doctor that asks no forge (one a test names).
    NoForge,
    /// Asked of the CLI through this transport.
    Ask(&'a dyn Transport),
}

/// The `<cli> auth` row: the CLI's own answer to whether it is signed in on the forge's host.
pub(super) fn auth_row(forge: &Forge, on: Use, found: bool, how: Asking<'_>) -> Row {
    let cli = forge.kind.cli();
    let name = format!("{cli} auth");
    let host = super::one_line(&forge.host, super::DISPLAY_LIMIT);
    if on == Use::None {
        return Row::ok(&name, format!("not asked: {}", on_host(forge, on)));
    }
    if on == Use::Unknown {
        return Row::not_checked(&name, on_host(forge, on));
    }
    if !found {
        return Row::not_checked(
            &name,
            format!("{cli} is not found; the `{cli}` row says so"),
        );
    }
    let ask = match how {
        Asking::Preflight => {
            return Row::not_checked(
                &name,
                format!(
                    "the preflight a chat's start runs asks no forge CLI; `purlis doctor` asks \
                     {cli}"
                ),
            );
        }
        Asking::NoForge => return Row::not_checked(&name, "this doctor asks no forge"),
        Asking::Ask(transport) => transport,
    };
    match ask.check_auth(forge) {
        Ok(()) => Row::ok(&name, format!("signed in on {host}")),
        Err(e) => said_refused(&name, cli, &e),
    }
}

/// A sign-in the CLI did not confirm. Its words are purlis's own ([`ForgeError::said`]),
/// never the CLI's output, which can hold a token.
fn said_refused(name: &str, cli: &str, e: &ForgeError) -> Row {
    Row::warn(
        name,
        super::one_line(e.said(), super::DISPLAY_LIMIT),
        format!(
            "purlis never reads the token: it asks `{cli} auth status`, and runs {cli} for each \
             forge call, so a sign-in {cli} keeps is the one every `purlis` command uses."
        ),
    )
}

#[cfg(test)]
mod tests;
