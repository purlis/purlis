// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The floor's token half: a command that prints the forge token, or reads another stored
//! secret, is refused unattended (#866).
//!
//! A run that holds the token can reach the forge with any client at all, where no guard reads
//! what it does, so printing it is the first step of every merge the floor would otherwise
//! refuse. Attended, nothing here runs: [`super::release_floor_reason`] returns first. Keeping
//! the login away from a chat altogether is the full answer, and this is the floor until then.

use super::merge::{forge_words, opaque};

/// The opening of every refusal this module gives ([`super::TOKEN_FLOOR_FIX`], #6314): what is
/// refused, the supported way to hand a credential to the one command that needs it, and the
/// attended way.
///
/// The way through is `purlis secret exec`, which hands the vault's value to the command it runs
/// and never to the run. In a sandboxed chat that command is brokered by the app and must be a
/// tool the chat's persona declares; git is one only with the `args` it may take
/// (`docs/plane-format.md`, `[sandbox.personas.<persona>].tools`).
pub(super) const FIX: &str = "A stored credential is on purlis's floor: a run with nobody \
     watching may not print one into its own reach. To hand a token to one command, keep it in a \
     vault and run that command through it, `purlis secret exec <vault> --env NAME=<key> -- \
     <command>`, so the run never holds the value; in a sandboxed chat the command must be a tool \
     your persona declares (`[sandbox.personas.<persona>] tools`, and `git` only with the `args` \
     it may take). Re-run this step **attended**, or have a person do it. ";

/// A whole refusal: [`FIX`], then why this command is refused.
fn refusal(why: &str) -> String {
    format!("{FIX}{why}")
}

/// The sentence a refusal of a token-printing command ends with.
const PRINTS_TOKEN: &str = "This prints the forge token, and a run that holds it can land code \
     where no guard sees what it does.";

/// True when a short-flag cluster or a long flag turns on `--show-token`. `-h` takes a value, so
/// a cluster ends there.
fn shows_token(args: &[String]) -> bool {
    args.iter().any(|a| {
        if a == "--show-token" || a.starts_with("--show-token=") {
            return a != "--show-token=false";
        }
        match a.strip_prefix('-') {
            Some(cluster) if !cluster.starts_with('-') => {
                cluster.chars().take_while(|c| *c != 'h').any(|c| c == 't')
            }
            _ => false,
        }
    })
}

/// True when `config get` names a key that holds a token, or one the shell fills in.
fn gets_a_token_key(rest: &[&str]) -> bool {
    rest.iter()
        .any(|k| opaque(k) || k.to_lowercase().contains("token"))
}

/// The refusal for a `gh` or `glab` command that prints the forge token.
pub(super) fn forge_reason(args: &[String]) -> Option<String> {
    let words: Vec<&str> = forge_words(args).into_iter().map(|w| w.1).collect();
    let prints = match words[..] {
        ["auth", "token", ..] | ["auth", "git-credential", ..] | ["auth", "docker-helper", ..] => {
            true
        }
        ["auth", "status", ..] => shows_token(args),
        ["config", "get", ref rest @ ..] => gets_a_token_key(rest),
        _ => false,
    };
    prints.then(|| refusal(PRINTS_TOKEN))
}

/// The refusal for a git command that prints a stored credential: `git credential fill`, or a
/// credential helper asked to `get`.
pub(super) fn git_reason(base: &str, args: &[String], sub: Option<&str>) -> Option<String> {
    let helper =
        base.starts_with("git-credential-") || sub.is_some_and(|s| s.starts_with("credential-"));
    let prints = match sub {
        Some("credential") => args.iter().any(|a| a == "fill"),
        _ if helper => args.iter().any(|a| a == "get"),
        _ => false,
    };
    prints.then(|| refusal(PRINTS_TOKEN))
}

/// The sentence a refusal of a command that reads a stored secret ends with.
const READS_A_STORE: &str = "This reads a stored secret, and a run that holds it can act \
     where no guard sees what it does.";

/// `security` options that take a value, so a cluster ends at one.
const SECURITY_VALUE_FLAGS: &str = "acCdDGjlpPrst";

/// The subcommands of `pass` and `gopass` that print no stored secret. Anything else, a bare
/// entry name included, can.
const PASS_QUIET: &[&str] = &[
    "ls", "list", "find", "search", "init", "insert", "edit", "generate", "rm", "mv", "cp", "git",
    "help", "version",
];

/// The refusal for a command that reads a secret from the operating system's or a password
/// manager's store: the macOS keychain asked for a password or dumped, the freedesktop secret
/// service looked up, or a `pass`/`gopass` entry shown (#866).
pub(super) fn keychain_read_reason(prog: &str, argv: &[String]) -> Option<String> {
    let base = crate::shellwrap::base_lower(prog);
    let args = argv.get(1..).unwrap_or(&[]);
    let first = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .map(String::as_str);
    let reads = match base.as_str() {
        "security" => match first {
            Some("find-generic-password" | "find-internet-password") => {
                args.iter().any(|a| match a.strip_prefix('-') {
                    Some(cluster) if !cluster.starts_with('-') => cluster
                        .chars()
                        .take_while(|c| !SECURITY_VALUE_FLAGS.contains(*c))
                        .any(|c| c == 'w' || c == 'g'),
                    _ => false,
                })
            }
            Some("dump-keychain") => true,
            _ => false,
        },
        "secret-tool" => matches!(first, Some("lookup" | "search")),
        "pass" | "gopass" => first.is_some_and(|w| !PASS_QUIET.contains(&w)),
        _ => false,
    };
    reads.then(|| refusal(READS_A_STORE))
}
