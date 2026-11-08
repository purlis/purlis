// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A `purlis` command that fails inside purlis exits [`INTERNAL_EXIT`]** (#5697, spec #5683
//! Q23): a panic (an `expect` on a state that cannot be, an `unreachable!` branch), and an
//! answer from the app this command has no arm for. That code, and only it, is what purlis's own
//! hook drafts a Report of in a chat ([`purlis_core::caught`]). A refusal, a policy's block and
//! a usage error never exit with it: they exit 1, and clap's 2 is turned into 1 too.

use std::io::Write as _;

pub use purlis_core::caught::INTERNAL_EXIT;

/// The first words of a command line whose process is not a command a chat runs and waits for,
/// and keeps the panic's own end: a hook (a guard's crash is a refusal, exit 2, and a reporting
/// hook's is any failure), the MCP server, the session host, the shell guard and a runner's
/// bridge, each of which a chat never ran as a command.
const KEEPS_ITS_OWN_END: [&str; 6] = ["hook", "mcp", "serve", "shell-guard", "bridge", "listen"];

/// From here on, a panic on any thread ends this process with [`INTERNAL_EXIT`], after one line
/// on standard error, unless `first` (the command line's first word) names a process that keeps
/// its own end ([`KEEPS_ITS_OWN_END`]). Installed after `purlis_core::panicend::fatal`, whose
/// abort it replaces: the process still ends at the panic, no `Drop` runs, and nothing goes on.
pub fn exit_on_a_panic(first: Option<&str>) {
    if first.is_some_and(|word| KEEPS_ITS_OWN_END.contains(&word)) {
        return;
    }
    std::panic::set_hook(Box::new(|info| {
        // Never `eprintln!`: a print to a closed stderr panics, inside this hook.
        let _ = writeln!(std::io::stderr(), "purlis: internal failure: {info}");
        std::process::exit(i32::from(INTERNAL_EXIT));
    }));
}

/// Ends this command as an internal failure: `why` on standard error, then [`INTERNAL_EXIT`].
/// For a state the command has no arm for, such as an answer from the app it did not ask for.
pub fn failed(why: &str) -> ! {
    let _ = writeln!(std::io::stderr(), "purlis: internal failure: {why}");
    std::process::exit(i32::from(INTERNAL_EXIT));
}
