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
//!   programs.
//! - **A seccomp filter** the chat applies to itself before its program starts. It refuses the
//!   calls that would go round the rest (datagram and raw sockets, io_uring, the kernel's key
//!   store, a seccomp listener of its own), and hands every call a path or an address decides
//!   to the supervisor.
//! - **The supervisor**, the wrap's own process and the chat's parent. It reads each such call,
//!   judges it by the chat's [`profile::Profile`], and performs it itself: it opens the file or
//!   makes the connection and hands the chat the descriptor. It never lets the kernel go on with
//!   a call it judged, since the chat could change the call's memory in between. It is held by
//!   Landlock too, to what the profile could ever let a chat write and reach.
//!
//! The entry point is the hidden `purlis _sandbox` command ([`entry`]); it is not a public
//! command line.

pub mod profile;

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
    let mut words = std::env::args_os().skip(1);
    if words.next()? != COMMAND {
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
    return Some(linux::entry(&words));
    #[cfg(not(target_os = "linux"))]
    {
        let _ = words;
        eprintln!("purlis: this sandbox is purlis's own on Linux, so nothing was started.");
        Some(126)
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
