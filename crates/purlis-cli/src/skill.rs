// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill list | show` (SK-2, #2765): the skills a chat in this project gets, and why each
//! other one is left out, as the Skill studio shows them. Read only, and behind the
//! `skill-studio` flag until its finish line (#2786).

use std::path::Path;

use clap::Subcommand;
use purlis_core::skills::{Effective, LeftOut, SkillSet};
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
    /// One skill: its level, its file, what it does, and whether a chat at the plane root gets
    /// it, or why not. With `-w`, a chat in that workspace. Exits 2 when there is no such skill.
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
const NOT_ON: &str = "The Skill studio is not turned on on this machine yet. It is built behind \
                      the `skill-studio` flag: set PURLIS_SKILL_STUDIO=1 to try it.";

pub fn run(root: &Path, command: SkillCommand) -> Result<Code, String> {
    if !purlis_core::skills::studio_on_here() {
        voice::err(NOT_ON);
        return Ok(1);
    }
    let built_in = purlis_core::skills::built_in_here();
    let set_for = |workspace: Option<&str>| {
        purlis_core::skills::in_the_studio(built_in.as_deref(), root, workspace)
    };
    match command {
        SkillCommand::List { workspace, json } => {
            let set = set_for(workspace.as_deref());
            if json {
                println!("{}", list_json(root, &set));
            } else {
                print!("{}", table(&set));
            }
            Ok(0)
        }
        SkillCommand::Show {
            name,
            workspace,
            json,
        } => {
            let set = set_for(workspace.as_deref());
            let found = set
                .effective
                .iter()
                .find(|e| e.skill.name == name)
                .map(Row::Gets)
                .or_else(|| {
                    set.left_out
                        .iter()
                        .find(|l| l.name == name)
                        .map(Row::LeftOut)
                });
            let Some(row) = found else {
                voice::err(&format!(
                    "No skill is named {name:?} here. `purlis skill list` lists every one."
                ));
                return Ok(2);
            };
            if json {
                println!("{}", row.json(root));
            } else {
                print!("{}", row.detail(root));
            }
            Ok(0)
        }
    }
}

/// One skill, as a list or `show` prints it.
enum Row<'a> {
    Gets(&'a Effective),
    LeftOut(&'a LeftOut),
}

impl Row<'_> {
    fn name(&self) -> &str {
        match self {
            Self::Gets(e) => &e.skill.name,
            Self::LeftOut(l) => &l.name,
        }
    }

    fn level(&self) -> &'static str {
        match self {
            Self::Gets(e) => e.origin.level(),
            Self::LeftOut(l) => l.origin.level(),
        }
    }

    /// `on`, or why it is left out.
    fn state(&self) -> String {
        match self {
            Self::Gets(_) => "on".to_owned(),
            Self::LeftOut(l) => format!("left out: {}", l.reason),
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::Gets(e) => &e.skill.description,
            Self::LeftOut(_) => "",
        }
    }

    fn folder(&self) -> &Path {
        match self {
            Self::Gets(e) => e.skill.path.parent().unwrap_or(&e.skill.path),
            Self::LeftOut(l) => &l.folder,
        }
    }

    fn json(&self, root: &Path) -> Value {
        json!({
            "name": self.name(),
            "level": self.level(),
            "on": matches!(self, Self::Gets(_)),
            "left_out": match self {
                Self::Gets(_) => Value::Null,
                Self::LeftOut(l) => Value::from(l.reason.to_string()),
            },
            "description": self.description(),
            "folder": voice::rel(root, self.folder()),
        })
    }

    fn detail(&self, root: &Path) -> String {
        let mut text = format!(
            "{}\n  level: {}\n  folder: {}\n",
            self.name(),
            self.level(),
            voice::rel(root, self.folder())
        );
        if !self.description().is_empty() {
            text.push_str(&format!("  description: {}\n", self.description()));
        }
        text.push_str(&format!("  in this chat's skills: {}\n", self.state()));
        text
    }
}

fn rows(set: &SkillSet) -> Vec<Row<'_>> {
    let mut rows: Vec<Row<'_>> = set
        .effective
        .iter()
        .map(Row::Gets)
        .chain(set.left_out.iter().map(Row::LeftOut))
        .collect();
    rows.sort_by(|a, b| a.name().cmp(b.name()).then(a.level().cmp(b.level())));
    rows
}

fn list_json(root: &Path, set: &SkillSet) -> Value {
    json!({ "skills": rows(set).iter().map(|row| row.json(root)).collect::<Vec<_>>() })
}

fn table(set: &SkillSet) -> String {
    let rows = rows(set);
    if rows.is_empty() {
        return "No skills here yet. A project skill is a folder in skills/ at the plane root, \
                holding its SKILL.md.\n"
            .to_owned();
    }
    let states: Vec<String> = rows.iter().map(Row::state).collect();
    let name_w = voice::column(rows.iter().map(Row::name).chain(["SKILL"]));
    let level_w = voice::column(rows.iter().map(Row::level).chain(["LEVEL"]));
    let mut text = format!(
        "{}  {}  STATE\n",
        voice::pad("SKILL", name_w),
        voice::pad("LEVEL", level_w)
    );
    for (row, state) in rows.iter().zip(&states) {
        text.push_str(&format!(
            "{}  {}  {}\n",
            voice::pad(row.name(), name_w),
            voice::pad(row.level(), level_w),
            state
        ));
    }
    text
}
