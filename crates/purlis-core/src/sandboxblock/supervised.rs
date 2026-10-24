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
//! chat with, which the wrap carries for it, once, before the wrap holds itself to the profile.
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
    block_of_in(refusal, place, &std::env::temp_dir())
}

/// [`block_of`], with `temp` the chat's temporary folder ([`super::kind_of_in`]).
pub fn block_of_in(
    refusal: &Refusal,
    place: &Place<'_>,
    temp: &Path,
) -> Option<(Block, Option<String>)> {
    // A file no longer linked is named by the kernel with " (deleted)" after its last name
    // (#2425, #2574): it is sorted, and Allow would name it, by the name it had, so a protected
    // file that was unlinked is still a protected file.
    let named = match refusal.target.strip_suffix(UNLINKED) {
        Some(had) if had.starts_with('/') => had,
        _ => refusal.target.as_str(),
    };
    let denial = Denial {
        witness: Witness::Kernel,
        process: refusal.process.as_deref(),
        operation: refusal.operation,
        target: named,
    };
    let (block, target) = denial.block_in(place, temp);
    // The system's name lookup, refused at its socket (#2428): a program's own lookup, past the
    // proxy. The name it looked up is never seen here, so none is named.
    if block.operation == Operation::Connect && super::RESOLVER_SOCKETS.contains(&named) {
        let block = Block {
            operation: Operation::Lookup,
            kind: Kind::Host,
            ..block
        };
        return Some((block, None));
    }
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

/// **What the wrap tells each refusal to**, from the environment it carries for the chat,
/// `env` (#2460), and this process's working folder: see [`teller_in`]. Where a name is carried
/// twice, the later one is the chat's, as it is for any program.
pub fn teller(env: &[(std::ffi::OsString, std::ffi::OsString)]) -> Option<Tell> {
    let here = std::env::current_dir().ok()?;
    // Under either of a variable's names, as this process's own would be read.
    let var = |name: &str| {
        crate::envvar::lookup(name, |spelled: &str| {
            env.iter()
                .rev()
                .find(|(key, _)| key == spelled)
                .and_then(|(_, value)| value.to_str())
                .map(str::to_owned)
        })
    };
    teller_in(&var, &here)
}

/// **What the wrap tells each refusal to**, read from `env` and the folder it started in,
/// `here`: each refusal sorted as a block and sent to the app over the chat's hook socket as its
/// hook sends one. None where the app did not start the chat sandboxed, or did not say its
/// socket, its number or the folder it started it in (never guessed from a working folder):
/// then nothing is told.
pub fn teller_in(env: &dyn Fn(&str) -> Option<String>, here: &Path) -> Option<Tell> {
    use crate::hookwire::{ChatToken, SOCKET_ENV};
    let sorted = sorter_in(env, here)?;
    let socket = PathBuf::from(env(SOCKET_ENV).filter(|it| !it.is_empty())?);
    let token = ChatToken::read(env);
    Some(Box::new(move |refusal: &Refusal| {
        if let Some(blocked) = sorted(refusal) {
            tell(&socket, token.as_ref(), &blocked);
        }
    }))
}

/// What each refusal is told as, a chat's block line, read from `env` and the folder the wrap
/// started in, `here`, as [`teller_in`] reads them; None where it tells nothing.
///
/// **Sorted against what the chat was started with** (#2460): the project its `PURLIS_ROOT`
/// names (under either name, held to the fence as a resolve is), or, without one, the project
/// found up from `here`; and its own temporary folder. The wrap's own environment holds none
/// of these.
pub fn sorter_in(
    env: &dyn Fn(&str) -> Option<String>,
    here: &Path,
) -> Option<impl Fn(&Refusal) -> Option<crate::hookwire::SandboxBlocked> + Send + Sync + 'static> {
    use crate::hookwire::{CHAT_ENV, SOCKET_ENV};
    if !crate::sandbox::chat_is_sandboxed_in(env) {
        return None;
    }
    env(SOCKET_ENV).filter(|it| !it.is_empty())?;
    let chat: u32 = env(CHAT_ENV)?.parse().ok()?;
    let absolute = |path: Option<String>| path.map(PathBuf::from).filter(|p| p.is_absolute());
    let chat_dir = absolute(env(CHAT_DIR_ENV))?;
    let home = absolute(env("HOME"));
    let root = match crate::envvar::lookup("PURLIS_ROOT", env).filter(|it| !it.is_empty()) {
        Some(root) => {
            let root = PathBuf::from(root);
            crate::fence::hold(crate::fence::Act::Resolve, &root);
            root
        }
        None => crate::plane::resolve(here).unwrap_or_else(|_| here.to_path_buf()),
    };
    let temp = absolute(env("TMPDIR")).unwrap_or_else(std::env::temp_dir);
    let harness = crate::hookwire::chat_harness(env);
    Some(move |refusal: &Refusal| {
        let place = Place {
            root: &root,
            chat: &chat_dir,
            cwd: &chat_dir,
            home: home.as_deref(),
        };
        let (block, target) = block_of_in(refusal, &place, &temp)?;
        Some(crate::hookwire::SandboxBlocked {
            chat,
            sandbox_blocked: block,
            harness: harness.clone(),
            target,
            read_back: false,
            suggested: None,
        })
    })
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
