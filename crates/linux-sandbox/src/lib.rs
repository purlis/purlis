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

/// The hidden command a wrapped chat's line starts with, after purlis's own program.
pub const COMMAND: &str = "_sandbox";

/// The words between [`COMMAND`] and the program: each piece of the profile follows one, so
/// no single argument passes the kernel's limit on one argument's length.
pub const PROFILE_FLAG: &str = "--profile";

/// The word that ends the profile; the program and its arguments follow it.
pub const END: &str = "--";

/// The longest piece of a profile one argument carries: under the kernel's 128 KiB for one.
pub const MOST_PIECE: usize = 96 * 1024;

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
mod tests {
    use super::*;

    #[test]
    fn a_long_profile_is_carried_in_pieces_and_read_back_whole() {
        let profile: String = (0..20_000)
            .map(|n| format!("(deny file-write* (literal \"/x/{n}\"))\n"))
            .collect();
        let words = line(&profile, "codex".to_owned(), vec!["--a".to_owned()]);
        assert_eq!(words[0], COMMAND);
        assert!(words.iter().all(|word| word.len() <= MOST_PIECE));
        assert!(words.iter().filter(|word| *word == PROFILE_FLAG).count() > 1);
        let (read, program) = read_line(&words[1..]).expect("read back");
        assert_eq!(read, profile);
        assert_eq!(program, vec!["codex".to_owned(), "--a".to_owned()]);
    }

    #[test]
    fn words_that_are_not_a_wrap_s_line_are_not_read() {
        let words = |list: &[&str]| list.iter().map(|it| (*it).to_owned()).collect::<Vec<_>>();
        assert!(read_line(&words(&["--profile", "(version 1)", "--"])).is_none());
        assert!(read_line(&words(&["--other", "x", "--", "sh"])).is_none());
        assert!(read_line(&words(&["--profile", "(version 1)"])).is_none());
    }
}
