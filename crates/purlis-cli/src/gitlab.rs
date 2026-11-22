// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis gitlab access`: GitLab access from the command line (#1349), what Settings › GitLab
//! access does in the window, from the same module ([`purlis_core::forge::gitlabaccess`]).
//!
//! A chat whose sandbox holds the person's own credentials back never uses their `glab` login.
//! This keeps a GitLab token in a persona's vault and declares the `glab` tool that hands it on,
//! for the host one of the project's clones has a remote on. The Allow of the new line is the
//! person's own, given in the window (the ✋ in Asks, or Settings › Sandbox): the command line
//! records none.

use clap::{Args, Subcommand};
use purlis_core::forge::gitlabaccess as access;
use purlis_core::secrets::cmd;
use purlis_core::secrets::persons_own::{self, Verb};
use purlis_core::secrets::{Ctx, Env};

/// `purlis gitlab …`.
#[derive(Subcommand)]
pub enum GitlabCommand {
    /// Give the project's chats a GitLab token of their own: keep it in a persona's vault and
    /// declare `glab` for that persona, on the host one of the project's clones is on.
    ///
    /// With no host, say which of the project's GitLab hosts are set up. The token is read from
    /// standard input, a file, or a hidden prompt, and never printed. Make it as a project or
    /// group access token in the Developer role with the `api` scope: it acts as a bot of its
    /// own and can do no more than a Developer.
    Access(AccessArgs),
}

#[derive(Args)]
pub struct AccessArgs {
    /// The GitLab host, as the clone's remote names it (`gitlab.com`, or your self-hosted one).
    host: Option<String>,
    /// The persona whose chats get the token; repeat for more than one.
    #[arg(
        long = "persona",
        short = 'p',
        value_name = "PERSONA",
        requires = "host"
    )]
    personas: Vec<String>,
    /// Take the host's token and `glab` line away (only what this command wrote).
    #[arg(long, requires = "host", conflicts_with_all = ["personas", "stdin", "from_file"])]
    remove: bool,
    /// Read the token from standard input.
    #[arg(long)]
    stdin: bool,
    /// Read the token from a file.
    #[arg(long, value_name = "FILE")]
    from_file: Option<String>,
    /// With no host: print the state as JSON.
    #[arg(long, conflicts_with = "host")]
    json: bool,
}

pub fn run(here: &crate::Here, command: GitlabCommand) -> u8 {
    match command {
        GitlabCommand::Access(args) => access_run(here, &args),
    }
}

fn access_run(here: &crate::Here, args: &AccessArgs) -> u8 {
    let ctx = Ctx::new(here.plane.root(), Env::from_process());
    let root = here.plane.root();
    let io = &mut crate::secret::Console;
    let Some(host) = args.host.as_deref() else {
        return list(root, args.json);
    };
    if persons_own::refused(&ctx, Verb::GitlabAccess { host }, io) {
        return 1;
    }
    if args.remove {
        return match access::remove(&ctx, root, host) {
            Ok(()) => {
                eprintln!("purlis: GitLab access for {host} is removed.");
                0
            }
            Err(why) => failed(&why),
        };
    }
    if args.personas.is_empty() {
        return failed(&format!(
            "Name the persona to give the token to: purlis gitlab access {host} --persona \
             <persona>. A persona is a folder under personas/ in the project."
        ));
    }
    let from = cmd::SetFrom {
        stdin: args.stdin,
        from_file: args.from_file.clone(),
        value: None,
        allow_empty: false,
    };
    let token = match cmd::read_value(&ctx, &access::key_of(host), &from, io) {
        Ok(token) => token.trim().to_owned(),
        Err(e) => return failed(&e.said().to_string()),
    };
    match access::set_up(&ctx, root, host, &token, &args.personas, None) {
        Ok(done) => {
            eprintln!(
                "purlis: GitLab access for {host} is set up for {}. The token is in {} and the \
                 chat never sees it.",
                args.personas.join(", "),
                if args.personas.len() == 1 {
                    "that persona's vault"
                } else {
                    "each persona's vault"
                },
            );
            if !done.left.is_empty() {
                eprintln!(
                    "purlis: Allow glab for {} ({}), then restart its chats.",
                    done.left.join(", "),
                    purlis_core::sandbox::tools::ALLOW_IS_ASKED
                );
            }
            0
        }
        Err(why) => failed(&why),
    }
}

fn failed(why: &str) -> u8 {
    eprintln!("purlis: {why}");
    1
}

/// Which of the project's GitLab hosts are set up, for which personas.
fn list(root: &std::path::Path, json: bool) -> u8 {
    let hosts = access::hosts(root);
    let held = match access::held(root) {
        Ok(held) => held,
        Err(why) => return failed(&why),
    };
    let personas_of = |host: &str| -> Vec<&str> {
        held.iter()
            .filter(|one| one.host == host && one.ours())
            .map(|one| one.persona.as_str())
            .collect()
    };
    if json {
        let rows: Vec<serde_json::Value> = hosts
            .iter()
            .map(|host| serde_json::json!({ "host": host, "personas": personas_of(host) }))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&rows).unwrap_or_default()
        );
        return 0;
    }
    if hosts.is_empty() {
        eprintln!("purlis: none of the project's clones has a GitLab remote.");
        return 0;
    }
    for host in &hosts {
        let who = personas_of(host);
        if who.is_empty() {
            eprintln!(
                "{host}: no token for chats. Set it up: purlis gitlab access {host} --persona \
                 <persona>"
            );
        } else {
            eprintln!("{host}: set up for {}", who.join(", "));
        }
    }
    0
}
