// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill list | show | new | edit | mv | rm | restore | grant | share | runs` (SK-2,
//! #2765; SK-4, #2767; SK-3, #2766; SK-7, #2770): the skills a chat in this project gets, and why
//! each other one is left out, as the Skill studio shows them; making, editing, renaming, moving,
//! deleting and restoring a skill, each one commit; where a project skill reaches and what a
//! workspace turned off; Share; and every run of a skill with its signals. The same verbs the
//! studio calls (`purlis_core::skills::verbs`). Behind the `skill-studio` flag until its finish
//! line (#2786).

use std::path::Path;

use clap::{Args, Subcommand, ValueEnum};
use purlis_core::skills::verbs::{self, By, Level};
use purlis_core::skills::{Grant, StudioRow, Template};
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
    /// Make a skill from a template, at the project's root or, with `-w`, in a workspace; with
    /// `--local`, kept on this machine. A project skill starts granted to the root alone.
    New {
        /// Its name: lowercase letters, digits and hyphens.
        name: String,
        /// What it does and when to use it, in one line.
        #[arg(long, short = 'd')]
        description: String,
        /// What it starts from.
        #[arg(long, value_enum, default_value = "blank")]
        template: TemplateArg,
        #[command(flatten)]
        at: At,
    },
    /// Edit a skill's SKILL.md: in `$VISUAL` or `$EDITOR`, or take the whole new text from
    /// `--from` (`-` for stdin). Saved only while it is still a whole skill that keeps its name.
    Edit {
        /// The skill's name, as `purlis skill list` prints it.
        name: String,
        /// The file that holds the whole new text, or `-` for stdin.
        #[arg(long, value_name = "FILE")]
        from: Option<std::path::PathBuf>,
        #[command(flatten)]
        at: At,
    },
    /// Rename a skill (`--name`), or move it to another level (`--to`), or both. A rename
    /// changes every persona and skill that names it; a move into the project's skills says
    /// where it reaches with `--grant`.
    Mv {
        /// The skill's name, as `purlis skill list` prints it.
        name: String,
        /// Its new name.
        #[arg(long = "name", value_name = "NEW")]
        new: Option<String>,
        /// The level it moves to: `root` (the project's) or a workspace's name.
        #[arg(long, value_name = "LEVEL")]
        to: Option<String>,
        /// Keep it on this machine at the level it moves to.
        #[arg(long, requires = "to")]
        to_local: bool,
        /// Where it reaches once it is a project skill: `root`, `all`, or workspace names.
        #[arg(long, num_args = 1.., requires = "to")]
        grant: Vec<String>,
        #[command(flatten)]
        at: At,
    },
    /// Delete a skill, after saying who uses it. `purlis skill restore` brings it back.
    Rm {
        /// The skill's name, as `purlis skill list` prints it.
        name: String,
        #[command(flatten)]
        at: At,
    },
    /// Restore a deleted skill to the level it was deleted from; with no name, list the deleted
    /// skills that can be restored.
    Restore {
        /// The deleted skill's name.
        name: Option<String>,
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

/// The level a skill is at: the project's root, or a workspace; shared, or local.
#[derive(Args)]
pub struct At {
    /// The workspace whose skill it is (default: the project's).
    #[arg(short = 'w', long = "workspace")]
    workspace: Option<String>,
    /// A local skill, kept on this machine.
    #[arg(long)]
    local: bool,
}

impl At {
    fn level(&self) -> Level {
        match &self.workspace {
            None => Level::project(),
            Some(ws) => Level::workspace(ws),
        }
        .local(self.local)
    }
}

/// A new skill's template, as the command line names it.
#[derive(Clone, Copy, ValueEnum)]
pub enum TemplateArg {
    Blank,
    Steps,
}

impl From<TemplateArg> for Template {
    fn from(arg: TemplateArg) -> Template {
        match arg {
            TemplateArg::Blank => Template::Blank,
            TemplateArg::Steps => Template::Steps,
        }
    }
}

/// What a chat is told when it asks to change a skill: a chat's change becomes a draft the
/// person accepts in the studio (spec #2762 decisions 26 and 38), never a write.
const FROM_A_CHAT: &str = "A chat does not change skills itself: its change becomes a draft the \
                           person accepts in the Skill studio. Nothing was written. Run it from \
                           a terminal, or ask the person to.";

/// What a person is told when the flag is off.
const NOT_ON: &str = "The Skill studio is not turned on on this machine yet. Turn it on in \
                      Settings › You › Previews, or set PURLIS_SKILL_STUDIO=1 to try it here.";

pub fn run(root: &Path, command: SkillCommand) -> Result<Code, String> {
    if !purlis_core::skills::studio_on_here() {
        voice::err(NOT_ON);
        return Ok(1);
    }
    let built_in = purlis_core::skills::built_in_here();
    let writes = !matches!(
        command,
        SkillCommand::List { .. }
            | SkillCommand::Show { .. }
            | SkillCommand::Restore { name: None }
            | SkillCommand::Runs { .. }
    );
    if writes
        && purlis_core::envvar::var(purlis_core::active::SESSION_ID_ENV)
            .is_some_and(|id| !id.is_empty())
    {
        voice::err(FROM_A_CHAT);
        return Ok(1);
    }
    let mut said = |line: purlis_core::repocmd::Say| crate::speak(line);
    let mut cx = verbs::Ctx {
        plane: root,
        built_in: built_in.as_deref(),
        by: By::Command,
        say: &mut said,
    };
    let rows_for = |workspace: Option<&str>, persona: Option<&str>| {
        let set = purlis_core::skills::in_the_studio(built_in.as_deref(), root, workspace, persona);
        let names = persona
            .map(|persona| purlis_core::skills::names_of(root, persona))
            .unwrap_or_default();
        purlis_core::skills::studio_rows(&set, root, &names)
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
        SkillCommand::New {
            name,
            description,
            template,
            at,
        } => done(verbs::create(
            &mut cx,
            &at.level(),
            &name,
            &description,
            template.into(),
        )),
        SkillCommand::Edit { name, from, at } => {
            let level = at.level();
            let (_, was) = match verbs::text_of(root, &level, &name) {
                Ok(read) => read,
                Err(why) => {
                    voice::err(&why);
                    return Ok(1);
                }
            };
            let now = match new_text(from.as_deref(), &was) {
                Ok(now) => now,
                Err(why) => {
                    voice::err(&why);
                    return Ok(1);
                }
            };
            done(verbs::edit(&mut cx, &level, &name, &was, &now))
        }
        SkillCommand::Mv {
            name,
            new,
            to,
            to_local,
            grant,
            at,
        } => {
            if new.is_none() && to.is_none() {
                voice::err(
                    "Say what changes: --name <new> renames it, --to <root|workspace> moves it.",
                );
                return Ok(2);
            }
            let level = at.level();
            let mut name = name;
            if let Some(new) = new {
                if let Err(why) = verbs::rename(&mut cx, &level, &name, &new) {
                    voice::err(&why);
                    return Ok(1);
                }
                name = new;
            }
            let Some(to) = to else {
                return Ok(0);
            };
            let target = level_to(&to, to_local);
            let grant = if grant.is_empty() {
                None
            } else {
                match Grant::from_words(&grant) {
                    Ok(grant) => Some(grant),
                    Err(why) => {
                        voice::err(&format!(
                            "{why}: --grant takes root, all, or workspace names"
                        ));
                        return Ok(2);
                    }
                }
            };
            if verbs::needs_grant(&level, &target) && grant.is_none() {
                voice::err(&format!(
                    "{}. Add --grant root, --grant all, or --grant <workspace>…",
                    verbs::GRANT_NEEDED
                ));
                return Ok(2);
            }
            done(verbs::move_to(
                &mut cx,
                &level,
                &name,
                &target,
                grant.as_ref(),
            ))
        }
        SkillCommand::Rm { name, at } => {
            let level = at.level();
            match verbs::users(root, &level, &name) {
                Ok(users) => say_users(&users),
                Err(why) => {
                    voice::err(&why);
                    return Ok(1);
                }
            }
            done(verbs::delete(&mut cx, &level, &name))
        }
        SkillCommand::Restore { name: None } => {
            let deleted = verbs::deleted(root);
            if deleted.is_empty() {
                println!("No deleted skill can be restored here.");
            }
            for one in deleted {
                println!("{}  {}", one.name, one.level);
            }
            Ok(0)
        }
        SkillCommand::Restore { name: Some(name) } => {
            let Some(one) = verbs::deleted(root)
                .into_iter()
                .find(|one| one.name == name)
            else {
                voice::err(&format!(
                    "No deleted skill named {name:?} can be restored: `purlis skill restore` lists those that can."
                ));
                return Ok(2);
            };
            done(verbs::restore(&mut cx, &one))
        }
        SkillCommand::Grant {
            name,
            workspace: Some(workspace),
            on,
            ..
        } => match crate::brokered::manifest_refused("skill grant --on/--off")
            .map_or_else(|| verbs::switch(&mut cx, &workspace, &name, on), Err)
        {
            Ok(changed) => {
                if !changed {
                    let state = if on { "on" } else { "off" };
                    voice::ok(&format!("{name} was already {state} in {workspace}."));
                }
                Ok(0)
            }
            Err(why) => {
                voice::err(&why);
                if on && why.contains("would reach it") {
                    voice::info(&format!(
                        "To grant it: purlis skill grant {name} {workspace}"
                    ));
                }
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
            match verbs::grant(&mut cx, &name, &grant) {
                Ok(_) => Ok(0),
                Err(why) => {
                    voice::err(&why);
                    Ok(1)
                }
            }
        }
        SkillCommand::Share { name, workspace } => {
            match verbs::share(&mut cx, workspace.as_deref(), &name) {
                Ok(_) => Ok(0),
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

/// What a verb's answer comes to: 0 where it was done, 1 with its refusal said.
fn done<T>(answer: Result<T, String>) -> Result<Code, String> {
    match answer {
        Ok(_) => Ok(0),
        Err(why) => {
            voice::err(&why);
            Ok(1)
        }
    }
}

/// The new text of an edit: `--from`'s file or stdin, or what the person leaves in a copy of
/// `was` opened in `$VISUAL` or `$EDITOR`.
fn new_text(from: Option<&Path>, was: &str) -> Result<String, String> {
    match from {
        Some(path) if path == Path::new("-") => {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)
                .map_err(|why| format!("stdin could not be read ({why})"))?;
            Ok(text)
        }
        Some(path) => std::fs::read_to_string(path)
            .map_err(|why| format!("{} could not be read ({why})", path.display())),
        None => {
            let editor = ["VISUAL", "EDITOR"]
                .iter()
                .find_map(|key| std::env::var(key).ok().filter(|v| !v.trim().is_empty()))
                .ok_or_else(|| {
                    "neither $VISUAL nor $EDITOR is set: give the new text with --from <file>"
                        .to_owned()
                })?;
            // A copy of its own, made new so nothing already there is written through.
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|at| at.as_nanos())
                .unwrap_or_default();
            let copy =
                std::env::temp_dir().join(format!("SKILL.{}.{stamp}.md", std::process::id()));
            let made = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&copy)
                .and_then(|mut file| std::io::Write::write_all(&mut file, was.as_bytes()));
            made.map_err(|why| format!("no copy could be made to edit ({why})"))?;
            let mut words = editor.split_whitespace();
            let program = words.next().unwrap_or_default();
            let status = std::process::Command::new(program)
                .args(words)
                .arg(&copy)
                .status();
            let text = std::fs::read_to_string(&copy);
            let _ = std::fs::remove_file(&copy);
            let status = status.map_err(|why| format!("{program} could not be started ({why})"))?;
            if !status.success() {
                return Err(format!("{program} exited {status}, so nothing was saved"));
            }
            text.map_err(|why| format!("the edited copy could not be read ({why})"))
        }
    }
}

/// The level `--to` and `--to-local` name.
fn level_to(to: &str, local: bool) -> Level {
    if to == "root" {
        Level::project()
    } else {
        Level::workspace(to)
    }
    .local(local)
}

/// Who uses a skill, as `rm` says it before it deletes.
fn say_users(users: &verbs::Users) {
    let mut reach = Vec::new();
    if users.root {
        reach.push("chats at the project's root".to_owned());
    }
    reach.extend(users.workspaces.iter().map(|ws| format!("chats in {ws}")));
    if !reach.is_empty() {
        voice::info(&format!("Used by {}", reach.join(", ")));
    }
    if !users.personas.is_empty() {
        voice::info(&format!(
            "Named by {}: {}",
            purlis_core::counts::counted(users.personas.len(), "persona", "personas"),
            users.personas.join(", ")
        ));
    }
    if !users.skills.is_empty() {
        voice::info(&format!(
            "Named by {}: {}",
            purlis_core::counts::counted(users.skills.len(), "skill", "skills"),
            users.skills.join(", ")
        ));
    }
}

/// The level a row is shown at, as the studio shows it: `project`, `local project`.
fn level(row: &StudioRow) -> String {
    purlis_core::skills::level_shown(row.origin, row.local)
}

/// `on`, or why it is left out.
fn state(row: &StudioRow) -> String {
    match &row.left_out {
        None if row.unchecked => format!("on: {}", purlis_core::skills::UNCHECKED),
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
        "written": row.written,
        "named": row.named,
        "unchecked": row.unchecked,
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
