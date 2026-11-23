// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis inbox`: what is waiting for the person in needs-you** (M75 slice 19, #7028; spec
//! §8.5, Q314.5).
//!
//! An ask that reached nobody (the chat is unattended, or no window can be reached) was refused
//! and raised as one needs-you item ([`purlis_core::policy::needsyou`]). This command lists them
//! and **denies or dismisses** one, from a machine with no window too.
//!
//! **There is no allow verb, and there cannot be one here.** An allow loosens what a chat may do,
//! so it takes the person's press ([`purlis_core::secrets::answers::Press`]), which only the
//! window makes: this command has none to pass. A headless machine loosens only through the
//! Organisation (spec §8.5).

use std::process::ExitCode;

use purlis_core::policy::needsyou::{Item, Store, Why};

/// `purlis inbox`: the asks refused because nobody could be asked.
#[derive(clap::Subcommand, Debug)]
pub enum InboxCommand {
    /// List what is waiting: one line per rule, resource, workspace and request, with how many
    /// chats asked and when last.
    List {
        /// Print the items as JSON, one array.
        #[arg(long)]
        json: bool,
    },
    /// Deny an item: the chats that asked are told at their next call, and it is not asked again
    /// until the chat asks anew. Allowing is the window's alone.
    Deny {
        /// The item's id, or the start of it, as `list` prints it.
        id: String,
    },
    /// Dismiss an item: it leaves the list and nobody is told.
    Dismiss {
        /// The item's id, or the start of it, as `list` prints it.
        id: String,
    },
}

/// Runs `purlis inbox …`.
pub fn run(command: &InboxCommand) -> ExitCode {
    let Some(store) = Store::here() else {
        eprintln!(
            "purlis: there is no data folder here that purlis may write in, so there is nothing \
             in needs-you. Set $PURLIS_DATA_HOME to one."
        );
        return ExitCode::FAILURE;
    };
    match command {
        InboxCommand::List { json } => list(&store, *json),
        InboxCommand::Deny { id } => {
            answer(&store, id, "Denied", |store, id| store.deny(id, now()))
        }
        InboxCommand::Dismiss { id } => {
            answer(&store, id, "Dismissed", |store, id| store.dismiss(id))
        }
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

fn list(store: &Store, json: bool) -> ExitCode {
    let record = store.read();
    if json {
        match serde_json::to_string(record.items()) {
            Ok(text) => println!("{text}"),
            Err(why) => {
                eprintln!("purlis: could not write the items as JSON ({why})");
                return ExitCode::FAILURE;
            }
        }
        return ExitCode::SUCCESS;
    }
    if record.items().is_empty() {
        println!("Nothing is waiting in needs-you.");
        return ExitCode::SUCCESS;
    }
    for item in record.items().iter().rev() {
        println!("{}", line(item, now()));
    }
    ExitCode::SUCCESS
}

/// **One item as `list` prints it**: its id, what it asks, where, who and when.
fn line(item: &Item, now: u64) -> String {
    let chats = if item.chat_count() == 1 {
        "1 chat".to_owned()
    } else {
        format!("{} chats", item.chat_count())
    };
    format!(
        "{}  {}  [{} in {}; rule {}]  {chats}, last {} ago ({})",
        item.key.id(),
        item.said,
        item.action,
        item.key.workspace,
        item.key.rule,
        ago(now.saturating_sub(item.last)),
        because(item.why),
    )
}

fn because(why: Why) -> &'static str {
    match why {
        Why::Unattended => "nobody at the chat",
        Why::NoWindow => "no window reachable",
        Why::NoAnswer => "not answered in time",
    }
}

fn ago(seconds: u64) -> String {
    match seconds {
        0..=59 => format!("{seconds} s"),
        60..=3599 => format!("{} min", seconds / 60),
        3600..=86_399 => format!("{} h", seconds / 3600),
        _ => format!("{} d", seconds / 86_400),
    }
}

fn answer(
    store: &Store,
    id: &str,
    did: &str,
    act: impl FnOnce(&Store, &str) -> std::io::Result<Option<Item>>,
) -> ExitCode {
    match act(store, id) {
        Ok(Some(item)) => {
            println!("{did}: {}", item.said);
            ExitCode::SUCCESS
        }
        Ok(None) => {
            eprintln!(
                "purlis: no one item in needs-you has an id starting {id:?}. `purlis inbox list` \
                 prints the ids."
            );
            ExitCode::FAILURE
        }
        Err(why) => {
            eprintln!("purlis: could not change needs-you ({why})");
            ExitCode::FAILURE
        }
    }
}
