// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **purlis's own sandbox on Linux** (#1040; ADR 0067 §2, as ruled on 2026-10-16).
//!
//! A wrapped chat (Codex, opencode) runs under three things at once, none of which needs a
//! namespace or a privilege, so the same wrap works on a Linux desktop, in a Docker container
//! and in a Kubernetes pod under their default seccomp profiles:
//!
//! - **Landlock** (ABI [`MIN_ABI`] or later). The chat may open, write and connect nothing by
//!   itself, signal nothing outside the sandbox and reach no abstract socket; it may run
//!   programs from where its profile says ([`profile::Profile::starts_from`]).
//! - **A seccomp filter** the chat applies to itself before its program starts. It refuses the
//!   calls that would go round the rest (datagram and raw sockets, io_uring, the kernel's key
//!   store, a seccomp listener of its own), and hands every call a path or an address decides
//!   to the supervisor.
//! - **The supervisor**, the wrap's own process and the chat's parent. It reads each such call,
//!   judges it by the chat's [`profile::Profile`], and performs it itself: it opens the file or
//!   makes the connection and hands the chat the descriptor. It never lets the kernel go on with
//!   a call it judged, since the chat could change the call's memory in between. It is held by
//!   Landlock too, to what the profile could ever let a chat write and reach.
//!   A call it refuses is told ([`refusal`], #2344), so the window raises the same block
//!   Notice as for a refusal of macOS's Seatbelt; what is told never changes an answer.
//!
//! The entry point is the hidden `purlis _sandbox` command ([`entry`]); it is not a public
//! command line. The wrap starts with an environment purlis builds, never the chat's: the
//! command's own is carried in it ([`carried`]) and handed to the command alone.

pub mod carried;
pub mod profile;
pub mod refusal;

#[cfg(any(target_os = "linux", test))]
mod loopback;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub use linux::{abi, entry};

/// The oldest Landlock this sandbox runs on: ABI 6, Linux 6.12, the first with the scopes that
/// keep a chat from signalling a process outside its sandbox and from reaching an abstract
/// socket.
pub const MIN_ABI: i32 = 6;

/// What a kernel without [`MIN_ABI`] is told it lacks.
pub const NEEDS_LANDLOCK: &str = "purlis's sandbox needs Landlock version 6 (Linux 6.12 or later)";

/// What a person can do where this kernel lacks the Landlock the wrap needs: update it, start
/// the chat on Claude Code, whose own sandbox needs no Landlock, or, where `opt_out` (policy
/// allows starting a chat without the sandbox), start it without the sandbox.
pub fn landlock_way_on(opt_out: bool) -> String {
    if opt_out {
        "Update to Linux 6.12 or later, start this chat on a Claude Code profile, or start it \
         without the sandbox from the new-chat picker."
            .to_owned()
    } else {
        "Update to Linux 6.12 or later, or start this chat on a Claude Code profile.".to_owned()
    }
}

/// The hidden command a wrapped chat's line starts with, after purlis's own program.
pub const COMMAND: &str = "_sandbox";

/// **The hidden command that starts a program unable to make a namespace** (#3298,
/// D-2050-25): `purlis _no-namespaces -- <program> <args>…` applies a filter that refuses every
/// namespace-making `clone` and `unshare`, and `clone3`, then becomes `program`. A runner's host
/// starts everything it runs in a terminal so ([`no_namespaces_line`]), so on a runner only the
/// browser engine, which the host starts itself, may make namespaces. Not a public command line.
pub const NO_NAMESPACES: &str = "_no-namespaces";

/// `program` and `args`, as the words after purlis's own program that start it through
/// [`NO_NAMESPACES`].
pub fn no_namespaces_line(
    program: std::ffi::OsString,
    args: Vec<std::ffi::OsString>,
) -> Vec<std::ffi::OsString> {
    let mut out = vec![NO_NAMESPACES.into(), END.into(), program];
    out.extend(args);
    out
}

/// The words between [`COMMAND`] and the program: each piece of the profile follows one, so
/// no single argument passes the kernel's limit on one argument's length.
pub const PROFILE_FLAG: &str = "--profile";

/// The word that ends the profile; the program and its arguments follow it.
pub const END: &str = "--";

/// The longest piece of a profile one argument carries: under the kernel's 128 KiB for one.
pub const MOST_PIECE: usize = 96 * 1024;

/// **`purlis _sandbox …`, run, where this process was started as that**, before anything else
/// of the program it is: the status to exit with, or `None` for every other start. The `purlis`
/// command and the app call it first, so whichever purlis program starts a chat can wrap it.
pub fn serve_if_asked() -> Option<i32> {
    serve_if_asked_telling(|_| None)
}

/// [`serve_if_asked`], where each call the supervisor refuses is told to the [`refusal::Tell`]
/// `tell` makes (#2344). `tell` is asked once, by the supervisor only, before the wrap holds
/// itself to the profile, so what it reads to say a refusal is read while nothing holds it. It
/// is handed the environment the wrap carries for its command ([`carried`], #2460), the
/// environment the chat starts with, since the wrap's own holds nothing else.
/// `None` tells nothing; a refusal is a refusal whether or not it is told.
pub fn serve_if_asked_telling(
    tell: impl FnOnce(&[(std::ffi::OsString, std::ffi::OsString)]) -> Option<refusal::Tell>,
) -> Option<i32> {
    let mut words = std::env::args_os().skip(1);
    let first = words.next()?;
    if first == NO_NAMESPACES {
        return Some(without_namespaces(words.collect()));
    }
    if first != COMMAND {
        return None;
    }
    let Ok(words) = words
        .map(std::ffi::OsString::into_string)
        .collect::<Result<Vec<String>, _>>()
    else {
        eprintln!(
            "purlis: the sandbox was started with words that are not text, so nothing was started."
        );
        return Some(126);
    };
    #[cfg(target_os = "linux")]
    return Some(linux::entry(&words, tell));
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (words, tell);
        eprintln!("purlis: this sandbox is purlis's own on Linux, so nothing was started.");
        Some(126)
    }
}

/// `purlis _no-namespaces`, given the words after it: [`END`], the program, its arguments.
fn without_namespaces(words: Vec<std::ffi::OsString>) -> i32 {
    let Some((end, rest)) = words.split_first() else {
        eprintln!("purlis: nothing was named to start without namespaces, so nothing was started.");
        return 126;
    };
    let Some((program, args)) = rest.split_first().filter(|_| end == END) else {
        eprintln!("purlis: nothing was named to start without namespaces, so nothing was started.");
        return 126;
    };
    #[cfg(target_os = "linux")]
    return linux::without_namespaces(program, args);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (program, args);
        eprintln!(
            "purlis: a program is held from namespaces on Linux alone, so nothing was started."
        );
        126
    }
}

/// A wrapped chat's arguments after purlis's own program: [`COMMAND`], the profile in pieces
/// of whole lines, [`END`], then `program` and its `args`.
pub fn line(profile: &str, program: String, args: Vec<String>) -> Vec<String> {
    let mut out = vec![COMMAND.to_owned()];
    let mut piece = String::new();
    for line in profile.split_inclusive('\n') {
        if !piece.is_empty() && piece.len() + line.len() > MOST_PIECE {
            out.push(PROFILE_FLAG.to_owned());
            out.push(std::mem::take(&mut piece));
        }
        piece.push_str(line);
    }
    if !piece.is_empty() {
        out.push(PROFILE_FLAG.to_owned());
        out.push(piece);
    }
    out.push(END.to_owned());
    out.push(program);
    out.extend(args);
    out
}

/// What [`line`] wrote, read back: the profile and the program's words, or `None` where the
/// words are not such a line.
pub fn read_line(words: &[String]) -> Option<(String, Vec<String>)> {
    let mut profile = String::new();
    let mut at = 0;
    while let Some(flag) = words.get(at) {
        if flag == END {
            let rest = words[at + 1..].to_vec();
            return (!rest.is_empty()).then_some((profile, rest));
        }
        if flag != PROFILE_FLAG {
            return None;
        }
        profile.push_str(words.get(at + 1)?);
        at += 2;
    }
    None
}

#[cfg(test)]
mod tests;
