// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill list | show | grant | share | runs` (SK-2, #2765; SK-4, #2767; SK-3, #2766):
//! the skills a chat in this project gets, and why each other one is left out, as the Skill
//! studio shows them; where a project skill reaches and what a workspace turned off; Share; and
//! every run of a skill with its signals. Behind the `skill-studio` flag until its finish line
//! (#2786).

use std::path::Path;

use clap::Subcommand;
use purlis_core::skills::{Grant, StudioRow};
use serde_json::{Value, json};

use purlis_core::counts::counted;

use crate::memory::Code;
use crate::voice;

/// `purlis skill …`.
#[derive(Subcommand)]
pub enum SkillCommand {
    /// Every skill a chat at the plane root gets, with its level (`built-in`, `project`,
    /// `workspace`, `persona`, and `local` for a local skill), and
    /// every skill left out of it with why. With `-w`, a chat in that workspace's; with `-p`, a
    /// chat as that persona, which brings its skills into every workspace.
    List {
        /// The workspace whose chats' skills to list (default: a chat at the plane root).
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
        /// The persona the chat runs as (default: none).
        #[arg(short = 'p', long = "persona")]
        persona: Option<String>,
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
        /// The persona the chat runs as (default: none).
        #[arg(short = 'p', long = "persona")]
        persona: Option<String>,
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
    /// Every skill run the project's skill log holds, newest first, with its signals (tool
    /// calls, tool errors, refusals, asks, tokens, time): never a pass or a fail. With a name,
    /// that skill's runs alone.
    Runs {
        /// The skill whose runs to list (default: every skill's).
        name: Option<String>,
        /// How many runs to print at most.
        #[arg(short = 'n', long = "limit", default_value_t = 50)]
        limit: usize,
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
    let rows_for = |workspace: Option<&str>, persona: Option<&str>| {
        let set = purlis_core::skills::in_the_studio(built_in.as_deref(), root, workspace, persona);
        purlis_core::skills::studio_rows(&set, root)
    };
    match command {
        SkillCommand::List {
            workspace,
            persona,
            json,
        } => {
            let rows = rows_for(workspace.as_deref(), persona.as_deref());
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
            persona,
            json,
        } => {
            // Every copy of the name: the one a chat gets, and each left out with why, as the list
            // and the studio show them.
            let copies: Vec<StudioRow> = rows_for(workspace.as_deref(), persona.as_deref())
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
                        "{name} is {state} in {workspace}. purlis save commits it where \
                         {workspace} is LIVE."
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
            let grant = match Grant::from_words(&grant) {
                Ok(grant) => grant,
                Err(why) => {
                    voice::err(&format!(
                        "{why}. Say where it reaches: root (the project's root only), all (every \
                         workspace), or the workspaces' names. To turn it off in a workspace: \
                         purlis skill grant <name> -w <workspace> --off"
                    ));
                    return Ok(2);
                }
            };
            match purlis_core::skills::grant(root, &name, &grant) {
                Ok(_) => {
                    voice::ok(&format!(
                        "{name} is granted to {grant}. purlis save commits it."
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
        SkillCommand::Runs { name, limit, json } => {
            let runs = purlis_core::skills::studio_runs(root, name.as_deref(), limit);
            if json {
                println!("{}", json!({ "runs": runs }));
            } else {
                print!("{}", runs_table(&runs, name.as_deref()));
            }
            Ok(0)
        }
    }
}

/// The runs, one line each: when, which skill, by whom, what started it, how it ended and its
/// signals. A run recorded before runs says so, and a run whose end is not logged says it is
/// open or was not recorded.
fn runs_table(runs: &[purlis_core::skilluse::Run], name: Option<&str>) -> String {
    if runs.is_empty() {
        return match name {
            Some(name) => format!("No runs of {name} yet.\n"),
            None => "No skill runs yet.\n".to_owned(),
        };
    }
    let lines: Vec<[String; 6]> = runs
        .iter()
        .map(|run| {
            let (ended, signals) = match &run.end {
                Some(end) => (
                    end.ended.word().to_owned(),
                    [
                        counted(end.tool_calls, "tool call", "tool calls"),
                        counted(end.tool_errors, "error", "errors"),
                        counted(end.refusals, "refusal", "refusals"),
                        counted(end.asks, "ask", "asks"),
                        end.tokens.map_or_else(
                            || "tokens not reported".to_owned(),
                            |t| counted(t, "token", "tokens"),
                        ),
                        seconds(end.ms),
                    ]
                    .join(", "),
                ),
                None if run.id.is_none() => {
                    ("-".to_owned(), "from before runs were recorded".to_owned())
                }
                None => ("-".to_owned(), "no end recorded".to_owned()),
            };
            [
                run.started.clone(),
                run.skill.clone(),
                run.persona.clone().unwrap_or_else(|| "-".to_owned()),
                run.started_by
                    .map_or_else(|| "-".to_owned(), |by| by.word().to_owned()),
                ended,
                signals,
            ]
        })
        .collect();
    let heads = ["STARTED", "SKILL", "PERSONA", "BY", "ENDED", "SIGNALS"];
    let widths: Vec<usize> = (0..5)
        .map(|i| voice::column(lines.iter().map(|l| l[i].as_str()).chain([heads[i]])))
        .collect();
    let line = |cells: [&str; 6]| {
        let mut text = String::new();
        for (i, width) in widths.iter().enumerate() {
            text.push_str(&voice::pad(cells[i], *width));
            text.push_str("  ");
        }
        text.push_str(cells[5]);
        text.push('\n');
        text
    };
    let mut text = line(heads);
    for l in &lines {
        text.push_str(&line([&l[0], &l[1], &l[2], &l[3], &l[4], &l[5]]));
    }
    text
}

/// `ms` as a person reads a run's time: `850 ms`, `12 s`, `3 min 4 s`.
fn seconds(ms: u64) -> String {
    match ms {
        0..1_000 => format!("{ms} ms"),
        1_000..60_000 => format!("{} s", ms / 1_000),
        _ => format!("{} min {} s", ms / 60_000, ms % 60_000 / 1_000),
    }
}

/// The level a row is shown at, as the studio shows it: `project`, `local project`.
fn level(row: &StudioRow) -> String {
    purlis_core::skills::level_shown(row.origin, row.local)
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
