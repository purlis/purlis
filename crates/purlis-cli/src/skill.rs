// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill list | show | grant | share` (SK-2, #2765; SK-4, #2767): the skills a chat in
//! this project gets, and why each other one is left out, as the Skill studio shows them; where a
//! project skill reaches and what a workspace turned off; and Share. Behind the `skill-studio`
//! flag until its finish line (#2786).

use std::path::Path;

use clap::Subcommand;
use purlis_core::skills::{Grant, StudioRow};
use serde_json::{Value, json};

use crate::memory::Code;
use crate::voice;

/// `purlis skill …`.
#[derive(Subcommand)]
pub enum SkillCommand {
    /// Every skill a chat at the plane root gets, with its level (`built-in`, `project`,
    /// `workspace`, and `local` for a local skill), and
    /// every skill left out of it with why. With `-w`, a chat in that workspace's.
    List {
        /// The workspace whose chats' skills to list (default: a chat at the plane root).
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
        /// Answer as JSON, for a chat or a script.
        #[arg(long)]
        json: bool,
    },
    /// Every copy of a skill's name: its level, its folder, what it does, and whether a chat at
    /// the plane root gets it, or why not, as `list` shows them. With `-w`, a chat in that
    /// workspace. Exits 2 when no skill has the name.
    Show {
        /// The skill's name, as `purlis skill list` prints it.
        name: String,
        /// The workspace whose chats to answer for (default: a chat at the plane root).
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
        /// Answer as JSON, for a chat or a script.
        #[arg(long)]
        json: bool,
    },
    /// Where a project skill reaches: `root` (chats at the project's root only, the default),
    /// `all` (every workspace too), or the workspaces named. With `-w <ws> --off` or `--on`, that
    /// workspace turns a skill granted to it off, or back on: it can never turn on one it was not
    /// granted.
    Grant {
        /// The project skill's name, as `purlis skill list` prints it.
        name: String,
        /// `root`, `all`, or one or more workspace names.
        #[arg(conflicts_with = "workspace")]
        grant: Vec<String>,
        /// The workspace that turns the skill off or on (with `--off` or `--on`).
        #[arg(short = 'w', long = "workspace", requires = "switch")]
        workspace: Option<String>,
        /// Turn the skill off in that workspace.
        #[arg(long, group = "switch", requires = "workspace")]
        off: bool,
        /// Turn it back on in that workspace, where the project granted it.
        #[arg(long, group = "switch", requires = "workspace")]
        on: bool,
    },
    /// Share a local skill: move it from `skills.local/` into `skills/` at its level, so the
    /// next save commits it and the team gets it. With `-w`, that workspace's local skill.
    Share {
        /// The local skill's name, as `purlis skill list` prints it.
        name: String,
        /// The workspace whose local skill to share (default: the project's root).
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
    },
}

/// What a person is told when the flag is off.
const NOT_ON: &str = "The Skill studio is not turned on on this machine yet. Turn it on in \
                      Settings › You › Previews, or set PURLIS_SKILL_STUDIO=1 to try it here.";

pub fn run(root: &Path, command: SkillCommand) -> Result<Code, String> {
    if !purlis_core::skills::studio_on_here() {
        voice::err(NOT_ON);
        return Ok(1);
    }
    let built_in = purlis_core::skills::built_in_here();
    let rows_for = |workspace: Option<&str>| {
        let set = purlis_core::skills::in_the_studio(built_in.as_deref(), root, workspace);
        purlis_core::skills::studio_rows(&set, root)
    };
    match command {
        SkillCommand::List { workspace, json } => {
            let rows = rows_for(workspace.as_deref());
            if json {
                println!(
                    "{}",
                    json!({ "skills": rows.iter().map(row_json).collect::<Vec<_>>() })
                );
            } else {
                print!("{}", table(&rows));
            }
            Ok(0)
        }
        SkillCommand::Show {
            name,
            workspace,
            json,
        } => {
            // Every copy of the name: the one a chat gets, and each left out with why, as the list
            // and the studio show them.
            let copies: Vec<StudioRow> = rows_for(workspace.as_deref())
                .into_iter()
                .filter(|row| row.name == name)
                .collect();
            if copies.is_empty() {
                voice::err(&format!(
                    "No skill is named {name:?} here. `purlis skill list` lists every one."
                ));
                return Ok(2);
            }
            if json {
                println!(
                    "{}",
                    json!({ "copies": copies.iter().map(row_json).collect::<Vec<_>>() })
                );
            } else {
                let shown: Vec<String> = copies.iter().map(detail).collect();
                print!("{}", shown.join("\n"));
            }
            Ok(0)
        }
        SkillCommand::Grant {
            name,
            workspace: Some(workspace),
            on,
            ..
        } => match purlis_core::skills::switch(root, &workspace, &name, on) {
            Ok(changed) => {
                let state = if on { "on" } else { "off" };
                if changed {
                    voice::ok(&format!(
                        "{name} is {state} in {workspace}. purlis save shares it with the team \
                         where {workspace} is LIVE."
                    ));
                } else {
                    voice::ok(&format!("{name} was already {state} in {workspace}."));
                }
                Ok(0)
            }
            Err(why) => {
                voice::err(&why);
                Ok(1)
            }
        },
        SkillCommand::Grant { name, grant, .. } => {
            let Some(grant) = grant_of_words(&grant) else {
                voice::err(
                    "Say where it reaches: root (the project's root only), all (every \
                     workspace), or the workspaces' names. To turn it off in a workspace: purlis \
                     skill grant <name> -w <workspace> --off",
                );
                return Ok(2);
            };
            match purlis_core::skills::grant(root, &name, &grant) {
                Ok(_) => {
                    voice::ok(&format!(
                        "{name} is granted to {}. purlis save shares it with the team.",
                        reach_words(&grant)
                    ));
                    Ok(0)
                }
                Err(why) => {
                    voice::err(&why);
                    Ok(1)
                }
            }
        }
        SkillCommand::Share { name, workspace } => {
            match purlis_core::skills::share_here(root, workspace.as_deref(), &name) {
                Ok(folder) => {
                    voice::ok(&format!(
                        "{name} is shared: it is in {} now, and purlis save commits it.",
                        voice::rel(root, &folder)
                    ));
                    Ok(0)
                }
                Err(why) => {
                    voice::err(&why);
                    Ok(1)
                }
            }
        }
    }
}

/// The grant the words of `purlis skill grant <name> …` say: `root`, `all`, or workspace names.
/// `None` for none, or `root` or `all` beside other words.
fn grant_of_words(words: &[String]) -> Option<Grant> {
    match words {
        [] => None,
        [one] if one == "root" => Some(Grant::Root),
        [one] if one == "all" => Some(Grant::All),
        names if names.iter().any(|n| n == "root" || n == "all") => None,
        names => Some(Grant::Only(names.to_vec())),
    }
}

/// Where `grant` reaches, as a sentence says it.
fn reach_words(grant: &Grant) -> String {
    match grant {
        Grant::Root => "the project's root only".to_owned(),
        Grant::All => "every workspace".to_owned(),
        Grant::Only(names) => names.join(", "),
    }
}

/// The level a row is shown at: `project`, or `project, local` for a local skill.
fn level(row: &StudioRow) -> String {
    if row.local {
        format!("{}, local", row.origin.level())
    } else {
        row.origin.level().to_owned()
    }
}

/// `on`, or why it is left out.
fn state(row: &StudioRow) -> String {
    match &row.left_out {
        None => "on".to_owned(),
        Some(why) => format!("left out: {why}"),
    }
}

fn row_json(row: &StudioRow) -> Value {
    json!({
        "name": row.name,
        "level": row.origin.level(),
        "on": row.left_out.is_none(),
        "left_out": row.left_out,
        "description": row.description,
        "folder": row.folder,
        "local": row.local,
        "grant": row.grant,
    })
}

fn detail(row: &StudioRow) -> String {
    let mut text = format!(
        "{}\n  level: {}\n  folder: {}\n",
        row.name,
        level(row),
        row.folder
    );
    if !row.description.is_empty() {
        text.push_str(&format!("  description: {}\n", row.description));
    }
    if let Some(grant) = &row.grant {
        text.push_str(&format!("  grant: {grant}\n"));
    }
    text.push_str(&format!("  in this chat's skills: {}\n", state(row)));
    text
}

fn table(rows: &[StudioRow]) -> String {
    if rows.is_empty() {
        return "No skills here yet. A project skill is a folder in skills/ at the plane root, \
                holding its SKILL.md.\n"
            .to_owned();
    }
    let states: Vec<String> = rows.iter().map(state).collect();
    let name_w = voice::column(rows.iter().map(|row| row.name.as_str()).chain(["SKILL"]));
    let levels: Vec<String> = rows.iter().map(level).collect();
    let level_w = voice::column(levels.iter().map(String::as_str).chain(["LEVEL"]));
    let mut text = format!(
        "{}  {}  STATE\n",
        voice::pad("SKILL", name_w),
        voice::pad("LEVEL", level_w)
    );
    for ((row, state), level) in rows.iter().zip(&states).zip(&levels) {
        text.push_str(&format!(
            "{}  {}  {}\n",
            voice::pad(&row.name, name_w),
            voice::pad(level, level_w),
            state
        ));
    }
    text
}
