// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A call purlis's own wrap on Linux refused, told to the window as a sandbox block** (#2344).
//!
//! The supervisor of a chat purlis wraps on Linux (`purlis_linux_sandbox`) performs every file
//! operation and connection the chat asks of it after judging it by the chat's profile. A call
//! the profile refuses is a [`Refusal`]: the operation in Seatbelt's word, the exact target the
//! walk arrived at, and the name of the process that asked. That is what macOS's kernel line
//! says of a refusal (`Sandbox: <process>(<pid>) deny(1) <operation> <target>`), so it is read
//! as one ([`Denial`], [`super::denial::Witness::Kernel`]) and sorted by the one sorter every
//! block takes ([`block_of`]). The window then raises the same Notice, with the same ways on.
//!
//! **Told by the wrap, not the chat.** The supervisor is the chat's parent and runs outside its
//! sandbox; it tells the app over the chat's hook socket, with the chat's token, as a hook does
//! ([`crate::hookwire::tell_blocked`]), and its Notice goes through the app's throttle like any
//! other. What it reads to do so ([`teller`]) is read from the environment the app started the
//! chat with, once, before the wrap holds itself to the profile.
//!
//! **It never changes an answer.** The chat is refused before anything is told, and a block
//! that cannot be told (no app listening, a chat not started sandboxed) is a refusal all the
//! same.
//!
//! **One refusal, one Notice.** A host is purlis's proxy's to tell ([`super::is_the_proxys`]),
//! by name. The supervisor sees no host, only the address a program dialled past the proxy:
//! that is told as an address ([`Kind::Address`], #2423), at most once a minute a chat, and
//! names no host to allow. A chat purlis wraps on Linux has its files, sockets and services told here, so its
//! hook leaves those to the supervisor ([`super::the_hooks_own`]).

use std::path::{Path, PathBuf};

pub use purlis_linux_sandbox::refusal::{Refusal, Tell};

use super::denial::{Denial, Witness};
use super::{Block, CHAT_DIR_ENV, Kind, Operation, Place};

/// **The block `refusal` is, and what a grant would name for it**, sorted against `place` as
/// macOS's kernel line of the same refusal is, but for an internet address: what the proxy
/// would tell by host ([`super::is_the_proxys`]) is, from the supervisor, an address dialled past
/// it ([`Kind::Address`]), named by that address alone. None where it is not one.
pub fn block_of(refusal: &Refusal, place: &Place<'_>) -> Option<(Block, Option<String>)> {
    let denial = Denial {
        witness: Witness::Kernel,
        process: refusal.process.as_deref(),
        operation: refusal.operation,
        target: &refusal.target,
    };
    let (block, target) = denial.block(place);
    // The supervisor sees no host, only the address a program dialled: one past the proxy,
    // since the proxy's own port is what the chat reaches (#2423). It names that address.
    if super::is_the_proxys(&block) {
        let address = refusal.target.parse::<std::net::SocketAddr>().ok()?;
        let block = Block {
            kind: Kind::Address,
            ..block
        };
        return Some((block, Some(address.to_string())));
    }
    // A file no longer linked is named by the kernel with " (deleted)" after its last name
    // (#2425): what Allow would name is the name it had. Its kind is as that refusal sorts.
    let target = match refusal.target.strip_suffix(UNLINKED) {
        Some(had) if !block.ours => super::target_of(&block, had, place),
        _ => target,
    };
    Some((block, target))
}

/// What the kernel puts after the last name of a file that is no longer linked.
const UNLINKED: &str = " (deleted)";

/// Whether the supervisor tells blocks such as `block` itself: every refused file operation,
/// local socket and service on this machine. A lookup past the proxy it never sees.
pub fn tells(block: &Block) -> bool {
    matches!(
        (block.operation, block.kind),
        (Operation::Write | Operation::Read | Operation::File, _)
            | (
                Operation::Connect,
                Kind::LocalSocket | Kind::LocalService | Kind::Address
            )
    )
}

/// **What the wrap tells each refusal to**, from this process's own environment and working
/// folder: see [`teller_in`].
pub fn teller() -> Option<Tell> {
    let here = std::env::current_dir().ok()?;
    teller_in(&crate::envvar::var, &here)
}

/// **What the wrap tells each refusal to**, read from `env` and the folder it started in,
/// `here`: each refusal sorted as a block and sent to the app over the chat's hook socket as its
/// hook sends one. None where the app did not start the chat sandboxed, or did not say its
/// socket, its number or the folder it started it in (never guessed from a working folder):
/// then nothing is told.
pub fn teller_in(env: &dyn Fn(&str) -> Option<String>, here: &Path) -> Option<Tell> {
    use crate::hookwire::{CHAT_ENV, ChatToken, HARNESS_ENV, SOCKET_ENV};
    if !crate::sandbox::chat_is_sandboxed_in(env) {
        return None;
    }
    let socket = PathBuf::from(env(SOCKET_ENV).filter(|it| !it.is_empty())?);
    let chat: u32 = env(CHAT_ENV)?.parse().ok()?;
    let absolute = |path: Option<String>| path.map(PathBuf::from).filter(|p| p.is_absolute());
    let chat_dir = absolute(env(CHAT_DIR_ENV))?;
    let home = absolute(env("HOME"));
    let root = crate::plane::resolve(here).unwrap_or_else(|_| here.to_path_buf());
    let harness = env(HARNESS_ENV).filter(|it| !it.is_empty());
    let token = ChatToken::read(env);
    Some(Box::new(move |refusal: &Refusal| {
        let place = Place {
            root: &root,
            chat: &chat_dir,
            cwd: &chat_dir,
            home: home.as_deref(),
        };
        let Some((block, target)) = block_of(refusal, &place) else {
            return;
        };
        tell(
            &socket,
            token.as_ref(),
            &crate::hookwire::SandboxBlocked {
                chat,
                sandbox_blocked: block,
                harness: harness.clone(),
                target,
                read_back: false,
                suggested: None,
            },
        );
    }))
}

#[cfg(unix)]
fn tell(
    socket: &Path,
    token: Option<&crate::hookwire::ChatToken>,
    blocked: &crate::hookwire::SandboxBlocked,
) {
    // A block the app did not take is one nobody was shown; the refusal stands.
    let _ = crate::hookwire::tell_blocked(socket, token, blocked);
}

#[cfg(not(unix))]
fn tell(
    _socket: &Path,
    _token: Option<&crate::hookwire::ChatToken>,
    _blocked: &crate::hookwire::SandboxBlocked,
) {
}

#[cfg(test)]
mod tests;
