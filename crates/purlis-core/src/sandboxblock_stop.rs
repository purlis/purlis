// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A sandbox refusal as the block event it becomes** (R-222's core step, T1-10; R-220, Q206):
//! one `stop.block` with source [`Source::Sandbox`], aimed at the host, folder or service the
//! refusal was of, and nothing else of it.
//!
//! Every refusal the app hears of a chat's sandbox becomes one block, once it is not an open
//! ask, by T1-09's rule (Q154): never an ask and a block at once. A refusal no answer could name
//! (a denial class, a policy's no, purlis's own operation) is never an ask, so it is a block at
//! once; one that is asked (a write, a service on this machine, a host) is a block once its ask
//! leaves by anything but an Allow: Keep blocked, a newer ask in its place, or its chat's end. So
//! each Block line the network record keeps, and Blocked lately lists, has its block event.
//!
//! **The target summary** ([`target_of`]) is the app's own word for what was refused, never the
//! chat's text: the host as a grant names it, a service by its port or its name, a socket by its
//! file's name, and a folder by its path inside the project (ADR 0075 §4: never an absolute
//! path), resolved here from the folder the app offered to allow. A folder outside the project
//! has no path R-220's folder admits, so its block names no target.

use std::path::Path;

use super::Kind;
use crate::eventlog::stops::{Block, Qualifier, Source, Stop, Target};

/// **Who reported the refusal** (Q235): what the block's qualifier is decided by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reported {
    /// purlis's own proxy, by its own word: plain.
    ByTheProxy,
    /// The chat's own hook line, which a chat that can reach its hooks can send itself: written
    /// `unverified`, which the recorder clears for a run whose hooks run outside the chat's
    /// reach (`Recorder::stop`).
    ByItsHook,
}

/// **The Sandbox block a refusal of `kind` aimed at `named` is**, for a chat of the project at
/// `root`: `tool` by the harness's name for the call it met (empty where no call was heard), and
/// the target [`target_of`] resolves. No arguments' hash: the refusal names no call's arguments.
pub fn sandbox_block(
    kind: Kind,
    named: Option<&str>,
    root: &Path,
    tool: Option<&str>,
    reported: Reported,
) -> Stop {
    block(target_of(kind, named, root), tool, reported)
}

/// [`sandbox_block`], for a refusal whose kind is `kind`, by the word the app holds it under
/// ([`Kind::word`]): one naming no target where the word names no kind this build knows.
pub fn sandbox_block_of(
    kind: &str,
    named: Option<&str>,
    root: &Path,
    tool: Option<&str>,
    reported: Reported,
) -> Stop {
    let target = Kind::of_word(kind).and_then(|kind| target_of(kind, named, root));
    block(target, tool, reported)
}

fn block(target: Option<Target>, tool: Option<&str>, reported: Reported) -> Stop {
    Stop::Block(Block {
        tool: tool.unwrap_or_default().to_owned(),
        source: Source::Sandbox,
        qualifier: (reported == Reported::ByItsHook).then_some(Qualifier::Unverified),
        target,
        args: None,
    })
}

/// **What a refusal of `kind` was aimed at, as its block's target summary** (Q206), from
/// `named`, what the app offered to allow (the host, the folder a write was in, the port, the
/// socket's path or the service's name), for a chat of the project at `root`. None where it
/// named nothing, or nothing a target admits.
pub fn target_of(kind: Kind, named: Option<&str>, root: &Path) -> Option<Target> {
    let named = named.map(str::trim).filter(|named| !named.is_empty());
    match kind {
        Kind::Host | Kind::Address => {
            let named = named?;
            super::named_host(named)
                .and_then(|host| Target::host(&host).ok())
                .or_else(|| Target::host(named).ok())
        }
        Kind::LocalService | Kind::RunnerService => Target::service(named?).ok(),
        // A socket by its file's name: a path is a folder's, and this is no folder.
        Kind::LocalSocket => Target::service(Path::new(named?).file_name()?.to_str()?).ok(),
        // The system service by its name, and where the refusal named none, the kind of service
        // it is, which is purlis's own word.
        Kind::CertificateCheck | Kind::SystemService => {
            Target::service(named.unwrap_or_else(|| kind.word())).ok()
        }
        Kind::ProjectFiles
        | Kind::ProjectState
        | Kind::ProtectedFile
        | Kind::ChatFolder
        | Kind::ToolchainCache
        | Kind::Home
        | Kind::Temp
        | Kind::SystemTemp
        | Kind::SystemCache
        | Kind::System => folder_in(Path::new(named?), root),
    }
}

/// `folder` by its path inside the project at `root`, as written or as the kernel names the
/// project (`/var` is `/private/var` on macOS); `.` for the project itself. None outside it.
fn folder_in(folder: &Path, root: &Path) -> Option<Target> {
    let real = root.canonicalize().ok();
    let inside = std::iter::once(root)
        .chain(real.as_deref())
        .find_map(|root| folder.strip_prefix(root).ok())?;
    let relative = match inside.to_str()? {
        "" => ".",
        relative => relative,
    };
    Target::folder(relative).ok()
}
