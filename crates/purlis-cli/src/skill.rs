// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill list | show` (SK-2, #2765): the skills a chat in this project gets, and why each
//! other one is left out, as the Skill studio shows them. Read only, and behind the
//! `skill-studio` flag until its finish line (#2786).

use std::path::Path;

use clap::Subcommand;
use purlis_core::skills::StudioRow;
use serde_json::{Value, json};

use crate::memory::Code;
use crate::voice;

/// `purlis skill …`.
#[derive(Subcommand)]
pub enum SkillCommand {
    /// Every skill a chat at the plane root gets, with its level (`built-in`, `project`), and
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
    })
}

fn detail(row: &StudioRow) -> String {
    let mut text = format!(
        "{}\n  level: {}\n  folder: {}\n",
        row.name,
        row.origin.level(),
        row.folder
    );
    if !row.description.is_empty() {
        text.push_str(&format!("  description: {}\n", row.description));
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
    let level_w = voice::column(rows.iter().map(|row| row.origin.level()).chain(["LEVEL"]));
    let mut text = format!(
        "{}  {}  STATE\n",
        voice::pad("SKILL", name_w),
        voice::pad("LEVEL", level_w)
    );
    for (row, state) in rows.iter().zip(&states) {
        text.push_str(&format!(
            "{}  {}  {}\n",
            voice::pad(&row.name, name_w),
            voice::pad(row.origin.level(), level_w),
            state
        ));
    }
    text
}
