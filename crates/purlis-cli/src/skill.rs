// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill list | show | new | edit | mv | rm | restore | grant | share | install | update |
//! runs | history | revert | deprecate | archive | owners | uses` (SK-2, #2765; SK-4, #2767; SK-3,
//! #2766; SK-7, #2770; SK-19, #2782): the skills a chat in this project gets, and why each other one
//! is left out, as the Skill studio shows them; making, editing, renaming, moving, deleting and
//! restoring a skill, each one commit; where a project skill reaches and what a workspace turned
//! off; Share; every run of a skill with its signals; and a skill's history, its lifecycle state,
//! its owners and the skills it names and is named by. The same verbs the studio calls
//! (`purlis_core::skills::verbs`, `history`, `lifecycle`, `graph`). Behind the `skill-studio` flag
//! until its finish line (#2786).

use std::path::Path;

use clap::{Args, Subcommand, ValueEnum};
use purlis_core::skills::review::escaped as esc;
use purlis_core::skills::source::{self, Remote, Source, VaultEntry};
use purlis_core::skills::verbs::{self, By, Level};
use purlis_core::skills::{Deprecated, Grant, Lifecycle, StudioRow, Template, history, lifecycle};
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
    /// Make a skill from a template, at the project's root or, with `-w`, in a workspace, or,
    /// with `-p`, a persona's own; with `--local`, kept on this machine. A project skill starts
    /// granted to the root alone.
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
        at: LevelArgs,
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
        at: LevelArgs,
    },
    /// Rename a skill (`--name`), or move it to another level (`--to`, or `--to-persona`), or
    /// both. A rename changes every persona and skill that names it; a move into the project's
    /// skills says where it reaches with `--grant`. A persona's own skill moved into the
    /// project's is added to the persona's skills, so its chats keep it.
    Mv {
        /// The skill's name, as `purlis skill list` prints it.
        name: String,
        /// Its new name.
        #[arg(long = "name", value_name = "NEW")]
        new: Option<String>,
        /// The level it moves to: `root` (the project's) or a workspace's name.
        #[arg(long, value_name = "LEVEL", group = "dest")]
        to: Option<String>,
        /// The persona whose own skill it becomes.
        #[arg(long, value_name = "PERSONA", group = "dest")]
        to_persona: Option<String>,
        /// Keep it on this machine at the level it moves to.
        #[arg(long, requires = "dest")]
        to_local: bool,
        /// Where it reaches once it is a project skill: `root`, `all`, or workspace names.
        #[arg(long, num_args = 1.., requires = "to")]
        grant: Vec<String>,
        #[command(flatten)]
        at: LevelArgs,
    },
    /// Delete a skill, after saying who uses it. `purlis skill restore` brings it back.
    Rm {
        /// The skill's name, as `purlis skill list` prints it.
        name: String,
        #[command(flatten)]
        at: LevelArgs,
    },
    /// Restore a deleted skill to the level it was deleted from; with no name, list the deleted
    /// skills that can be restored.
    Restore {
        /// The deleted skill's name.
        name: Option<String>,
    },
    /// Share a local skill: move it from `skills.local/` into `skills/` at its level, so the
    /// next save commits it and the team gets it. With `-w`, that workspace's local skill; with
    /// `-p`, that persona's.
    Share {
        /// The local skill's name, as `purlis skill list` prints it.
        name: String,
        #[command(flatten)]
        at: LevelArgs,
    },
    /// Install skills from a skill source, a git repository of skill folders: prints the review
    /// (the scripts each runs, the network hosts its files name, its `!` commands and its writes
    /// outside the workspace, each risky one marked, and every file) and asks before it copies
    /// anything. The copy is pinned to the commit it was reviewed at, in `skills.lock`
    /// (`skills.local.lock` with `--local`). Skills the named ones refer to come along. With no
    /// skill named, lists what the source offers.
    Install {
        /// The source: an https:// URL, or a folder on this machine by its full path.
        url: String,
        /// The skills to install, by name.
        names: Vec<String>,
        /// The source's branch (default: its own default branch).
        #[arg(long)]
        branch: Option<String>,
        /// The folder inside the source its skills are in (default: its top).
        #[arg(long)]
        folder: Option<String>,
        /// The vault a private source signs in with, through `purlis secret exec`.
        #[arg(long, requires = "key")]
        vault: Option<String>,
        /// The entry in that vault that holds the token.
        #[arg(long, requires = "vault")]
        key: Option<String>,
        /// Where a project skill reaches: `root`, `all`, or workspace names (default: root).
        #[arg(long, num_args = 1..)]
        grant: Vec<String>,
        /// Install without asking: for a script that printed and read the review itself.
        #[arg(long)]
        yes: bool,
        #[command(flatten)]
        at: LevelArgs,
    },
    /// Check each installed skill's source for a newer commit (its refs only: nothing is
    /// fetched or written). With a name, print that skill's update as a diff of its review and
    /// its files, and ask before it is applied. Nothing is ever updated by itself.
    ///
    /// A source only the committed lockfile names is not contacted from this machine until it
    /// is confirmed: `--confirm <url>`.
    Update {
        /// The installed skill to update (default: list the updates there are).
        #[arg(conflicts_with = "confirm")]
        name: Option<String>,
        /// Update without asking: for a script that printed and read the diff itself.
        #[arg(long, requires = "name")]
        yes: bool,
        /// Confirm the source at this https:// URL on this machine, so it is asked for updates.
        #[arg(long, value_name = "URL")]
        confirm: Option<String>,
        /// The branch of the source `--confirm` names.
        #[arg(long, requires = "confirm")]
        branch: Option<String>,
        #[command(flatten)]
        at: LevelArgs,
    },
    /// A skill's history: every version git holds of it, newest first, with the runs that
    /// started on each. With `--diff <commit>`, what that version changed.
    History {
        /// The skill's name, as `purlis skill list` prints it.
        name: String,
        /// Show what this version changed, by its commit.
        #[arg(long, value_name = "COMMIT")]
        diff: Option<String>,
        #[command(flatten)]
        at: LevelArgs,
        /// Answer as JSON, for a chat or a script.
        #[arg(long)]
        json: bool,
    },
    /// Put a skill back as one version of its history has it, by that version's commit, as one
    /// commit. Its grant, lifecycle state and owners stay as they are. A version that would widen
    /// a chat says so and is asked about first.
    Revert {
        /// The skill's name, as `purlis skill list` prints it.
        name: String,
        /// The version's commit, as `purlis skill history` prints it.
        commit: String,
        /// Revert without asking, where the version would widen a chat: for a script that read
        /// the warning itself.
        #[arg(long)]
        yes: bool,
        #[command(flatten)]
        at: LevelArgs,
    },
    /// Deprecate a skill: it still works and reaches every chat it did, is listed after the
    /// rest, and names its replacement. With `--undo`, it is in use again.
    Deprecate {
        /// The skill's name, as `purlis skill list` prints it.
        name: String,
        /// The skill to use instead.
        #[arg(long = "for", value_name = "SKILL", conflicts_with = "undo")]
        replacement: Option<String>,
        /// Make it in use again.
        #[arg(long)]
        undo: bool,
        #[command(flatten)]
        at: LevelArgs,
    },
    /// Archive a skill: it never loads, and stays in the project to be restored. With `--undo`,
    /// it is restored and in use again.
    Archive {
        /// The skill's name, as `purlis skill list` prints it.
        name: String,
        /// Restore it.
        #[arg(long)]
        undo: bool,
        #[command(flatten)]
        at: LevelArgs,
    },
    /// Who looks after a skill, for people to know: with names, those become its owners; with
    /// `--none`, it names none; with neither, its owners are printed.
    Owners {
        /// The skill's name, as `purlis skill list` prints it.
        name: String,
        /// The owners: logins or emails.
        #[arg(conflicts_with = "none")]
        owners: Vec<String>,
        /// Name no owners.
        #[arg(long)]
        none: bool,
        #[command(flatten)]
        at: LevelArgs,
    },
    /// The skills a skill's text names (`skills:<name>`, `purlis:<name>`), each marked where no
    /// skill has that name, and the project's skills that name it.
    Uses {
        /// The skill's name, as `purlis skill list` prints it.
        name: String,
        /// Answer as JSON, for a chat or a script.
        #[arg(long)]
        json: bool,
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

/// The level a skill is at: the project's root, a workspace or a persona; shared, or local.
#[derive(Args)]
pub struct LevelArgs {
    /// The workspace whose skill it is (default: the project's).
    #[arg(short = 'w', long = "workspace", conflicts_with = "persona")]
    workspace: Option<String>,
    /// The persona whose own skill it is (SK-32).
    #[arg(short = 'p', long = "persona")]
    persona: Option<String>,
    /// A local skill, kept on this machine.
    #[arg(long)]
    local: bool,
}

impl LevelArgs {
    fn level(&self) -> Level {
        match (&self.workspace, &self.persona) {
            (Some(ws), _) => Level::workspace(ws),
            (None, Some(persona)) => Level::persona(persona),
            (None, None) => Level::project(),
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
    let reads_owners =
        matches!(&command, SkillCommand::Owners { owners, none: false, .. } if owners.is_empty());
    let writes = !reads_owners
        && !matches!(
            command,
            SkillCommand::List { .. }
                | SkillCommand::Show { .. }
                | SkillCommand::Restore { name: None }
                | SkillCommand::Update {
                    name: None,
                    confirm: None,
                    ..
                }
                | SkillCommand::Runs { .. }
                | SkillCommand::History { .. }
                | SkillCommand::Uses { .. }
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
            to_persona,
            to_local,
            grant,
            at,
        } => {
            if new.is_none() && to.is_none() && to_persona.is_none() {
                voice::err(
                    "Say what changes: --name <new> renames it, --to <root|workspace> or \
                     --to-persona <persona> moves it.",
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
            let target = match (to, to_persona) {
                (_, Some(persona)) => Level::persona(&persona).local(to_local),
                (Some(to), None) => level_to(&to, to_local),
                (None, None) => return Ok(0),
            };
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
        SkillCommand::Share { name, at } => {
            // Whose local skill it is: a share is always of a local one.
            match verbs::share(&mut cx, &at.level().at, &name) {
                Ok(_) => Ok(0),
                Err(why) => {
                    voice::err(&why);
                    Ok(1)
                }
            }
        }
        SkillCommand::Install {
            url,
            names,
            branch,
            folder,
            vault,
            key,
            grant,
            yes,
            at,
        } => {
            let source = Source {
                remote: Remote { url, branch },
                folder,
                sign_in: vault.zip(key).map(|(vault, key)| VaultEntry {
                    vault,
                    key,
                    user: None,
                }),
            };
            install(&mut cx, &source, &names, &grant, yes, &at.level())
        }
        SkillCommand::Update {
            confirm: Some(url),
            branch,
            ..
        } => match source::confirm(&mut cx, &Remote { url, branch }) {
            Ok(_) => Ok(0),
            Err(why) => {
                voice::err(&esc(&why));
                Ok(1)
            }
        },
        SkillCommand::Update { name: None, .. } => {
            let found = source::updates(root);
            for one in &found.unasked {
                voice::warn(&esc(&format!("{} ({}): {}", one.name, one.level, one.why)));
            }
            for one in &found.unconfirmed {
                println!(
                    "{}: not checked on this machine ({}). To check it: purlis skill update \
                     --confirm {}{}",
                    esc(&one.remote.to_string()),
                    esc(&one.skills.join(", ")),
                    esc(&one.remote.url),
                    one.remote
                        .branch
                        .as_deref()
                        .map(|b| format!(" --branch {}", esc(b)))
                        .unwrap_or_default()
                );
            }
            if found.offers.is_empty() {
                println!(
                    "Every installed skill this machine checks is pinned to its source's newest commit."
                );
            }
            for one in &found.offers {
                let edited = if one.edited { ", edited here" } else { "" };
                println!(
                    "{}  {}  {} -> {}{edited}",
                    esc(&one.name),
                    one.level,
                    one.pinned.short(),
                    one.newer.short()
                );
            }
            Ok(0)
        }
        SkillCommand::Update {
            name: Some(name),
            yes,
            at,
            ..
        } => {
            let level = at.level();
            let update = match source::update_review(root, &level, &name) {
                Ok(update) => update,
                Err(why) => {
                    voice::err(&esc(&why));
                    return Ok(1);
                }
            };
            print!("{}", update_text(&update));
            if !yes
                && !asked(&format!(
                    "Update {} to {}?",
                    esc(&name),
                    update.newer.short()
                ))
            {
                voice::info("Nothing was changed.");
                return Ok(1);
            }
            done(source::update(&mut cx, &level, &name, &update.accepted()))
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
        SkillCommand::History {
            name,
            diff: Some(commit),
            at,
            json,
        } => match history::diff(root, &at.level(), &name, &commit) {
            Ok(diff) => {
                if json {
                    println!("{}", json!({ "diff": diff }));
                    return Ok(0);
                }
                for line in &diff.unseen {
                    voice::warn(line);
                }
                print!("{}", diff.text);
                if diff.cut {
                    voice::info(&format!(
                        "The diff is cut at {} KiB.",
                        history::MOST_DIFF / 1024
                    ));
                }
                Ok(0)
            }
            Err(why) => {
                voice::err(&why);
                Ok(1)
            }
        },
        SkillCommand::History { name, at, json, .. } => {
            match history::history(root, &at.level(), &name) {
                Ok(versions) if json => {
                    println!("{}", json!({ "versions": versions }));
                    Ok(0)
                }
                Ok(versions) => {
                    print!("{}", versions_table(&name, &versions));
                    Ok(0)
                }
                Err(why) => {
                    voice::err(&why);
                    Ok(1)
                }
            }
        }
        SkillCommand::Revert {
            name,
            commit,
            yes,
            at,
        } => {
            let level = at.level();
            // The same warning, and the same ask, as the window's confirm (SK-8: purlis still
            // leaves what widens out when it hands the skill over).
            match history::revert_widens(root, &level, &name, &commit) {
                Ok(Some(widens)) => {
                    voice::warn(&widens);
                    if !yes && !asked(&format!("Revert {name} to {}?", history::short(&commit))) {
                        voice::info("Nothing was reverted.");
                        return Ok(1);
                    }
                }
                Ok(None) => {}
                Err(why) => {
                    voice::err(&why);
                    return Ok(1);
                }
            }
            done(history::revert(&mut cx, &level, &name, &commit))
        }
        SkillCommand::Deprecate {
            name,
            replacement,
            undo,
            at,
        } => {
            let lifecycle = if undo {
                Lifecycle::Active
            } else {
                Lifecycle::Deprecated(Deprecated { replacement })
            };
            lifecycle_set(&mut cx, &at.level(), &name, &lifecycle)
        }
        SkillCommand::Archive { name, undo, at } => {
            let lifecycle = if undo {
                Lifecycle::Active
            } else {
                Lifecycle::Archived
            };
            lifecycle_set(&mut cx, &at.level(), &name, &lifecycle)
        }
        SkillCommand::Owners {
            name,
            owners,
            none,
            at,
        } => {
            if owners.is_empty() && !none {
                return match verbs::text_of(root, &at.level(), &name) {
                    Ok((folder, _)) => {
                        let about = purlis_core::skills::about_of(&folder).unwrap_or_default();
                        if about.owners.is_empty() {
                            println!("{name} names no owners.");
                        } else {
                            println!("{}", about.owners.join("\n"));
                        }
                        Ok(0)
                    }
                    Err(why) => {
                        voice::err(&why);
                        Ok(1)
                    }
                };
            }
            match lifecycle::set_owners(&mut cx, &at.level(), &name, &owners) {
                Ok(true) => Ok(0),
                Ok(false) => {
                    voice::ok(&format!("{name}'s owners were already those."));
                    Ok(0)
                }
                Err(why) => {
                    voice::err(&why);
                    Ok(1)
                }
            }
        }
        SkillCommand::Uses { name, json } => {
            let node = purlis_core::skills::graph::uses_of(root, built_in.as_deref(), &name);
            if json {
                println!(
                    "{}",
                    json!({ "uses": node.uses, "used_by": node.used_by, "cut": node.cut })
                );
                return Ok(0);
            }
            if node.uses.is_empty() {
                println!("{name} names no other skill.");
            }
            for one in &node.uses {
                if one.broken {
                    println!("uses {}  (no skill has this name)", one.said);
                } else {
                    println!("uses {}", one.said);
                }
            }
            if node.used_by.is_empty() {
                println!("No skill of the project names {name}.");
            }
            for other in &node.used_by {
                println!("used by {other}");
            }
            if node.cut {
                voice::info(&format!(
                    "The project holds more than {} skills, and the rest were not read.",
                    purlis_core::skills::graph::MOST_SKILLS
                ));
            }
            Ok(0)
        }
    }
}

/// Sets a skill's lifecycle state, saying when it already was so.
fn lifecycle_set(
    cx: &mut verbs::Ctx<'_>,
    level: &Level,
    name: &str,
    lifecycle: &Lifecycle,
) -> Result<Code, String> {
    match lifecycle::set(cx, level, name, lifecycle) {
        Ok(true) => Ok(0),
        Ok(false) => {
            voice::ok(&format!("{name} was already {}.", lifecycle.word()));
            Ok(0)
        }
        Err(why) => {
            voice::err(&why);
            Ok(1)
        }
    }
}

/// A skill's versions, one line each: its commit, when, who, its subject and its runs; then the
/// runs on changes not committed, where there were any.
fn versions_table(name: &str, history: &history::History) -> String {
    if history.versions.is_empty() {
        return format!(
            "{name} has no history yet: git keeps a skill's versions once the project commits it.\n"
        );
    }
    let mut text = String::new();
    for version in &history.versions {
        let when = chrono::DateTime::from_timestamp(i64::try_from(version.at).unwrap_or(0), 0)
            .map(|at| at.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();
        text.push_str(&format!(
            "{}  {when}  {}  {}  ({})\n",
            history::short(&version.commit),
            version.author,
            version.subject,
            runs_said(&version.runs)
        ));
    }
    if history.uncommitted.runs > 0 {
        text.push_str(&format!(
            "On changes not committed: {}\n",
            runs_said(&history.uncommitted)
        ));
    }
    text
}

/// What runs add up to, as one line says them: signals, never a verdict.
fn runs_said(totals: &purlis_core::skilluse::Totals) -> String {
    if totals.runs == 0 {
        return "no runs".to_owned();
    }
    format!(
        "{}, {}, {}",
        counted(totals.runs, "run", "runs"),
        counted(totals.tool_calls, "tool call", "tool calls"),
        counted(totals.tool_errors, "error", "errors")
    )
}

/// The runs, one line each: when, which skill, by whom, what started it, how it ended and its
/// signals. A run recorded before runs says so, and a run whose end is not logged says it is
/// open or was not recorded.
fn runs_table(runs: &[purlis_core::skills::StudioRun], name: Option<&str>) -> String {
    if runs.is_empty() {
        return match name {
            Some(name) => format!("No runs of {name} yet.\n"),
            None => "No skill runs yet.\n".to_owned(),
        };
    }
    let lines: Vec<[String; 6]> = runs
        .iter()
        .map(|nested| {
            let run = &nested.run;
            let (ended, mut signals) = match &run.end {
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
            if let Some(parent) = &nested.inside {
                signals.push_str(&format!("; inside {parent}"));
            }
            if !nested.ran.is_empty() {
                signals.push_str(&format!("; ran {}", nested.ran.join(", ")));
            }
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

/// `purlis skill install`: what `source` offers where no skill is named; else the review of
/// `names`, asked about, then the install at `level`.
fn install(
    cx: &mut verbs::Ctx<'_>,
    source: &Source,
    names: &[String],
    grant: &[String],
    yes: bool,
    level: &Level,
) -> Result<Code, String> {
    let root = cx.plane;
    if names.is_empty() {
        let browse = match source::browse(root, source) {
            Ok(browse) => browse,
            Err(why) => {
                voice::err(&esc(&why));
                return Ok(1);
            }
        };
        println!("{} at {}:", esc(&source.to_string()), browse.commit.short());
        for one in &browse.skills {
            println!(
                "  {}  {}",
                esc(&one.skill.name),
                esc(&one.skill.description)
            );
        }
        for one in &browse.not_skills {
            println!("  ({}: {})", esc(&one.folder), esc(&one.why));
        }
        return Ok(0);
    }
    let grant = if grant.is_empty() {
        None
    } else {
        match Grant::from_words(grant) {
            Ok(grant) => Some(grant),
            Err(why) => {
                voice::err(&format!(
                    "{why}: --grant takes root, all, or workspace names"
                ));
                return Ok(2);
            }
        }
    };
    let review = match source::review(root, source, names) {
        Ok(review) => review,
        Err(why) => {
            voice::err(&esc(&why));
            return Ok(1);
        }
    };
    print!("{}", review_text(&review));
    let question = format!(
        "Install {} at {level}, pinned to {}?",
        counted(review.skills.len(), "skill", "skills"),
        review.commit.short()
    );
    if !yes && !asked(&question) {
        voice::info("Nothing was installed.");
        return Ok(1);
    }
    // A folder on this machine, or an install kept here, is this machine's source alone.
    let which = if level.local || !source.remote.url.starts_with("https://") {
        purlis_core::settings::Which::Local
    } else {
        purlis_core::settings::Which::Shared
    };
    let listed = source::sources(root).unwrap_or_default();
    if listed.iter().all(|one| one.source.remote != source.remote)
        && let Err(why) = source::add(cx, source, which)
    {
        voice::warn(&esc(&format!(
            "The source was not added to this project's sources: {why}"
        )));
    }
    done(source::install(
        cx,
        source,
        level,
        &review.accepted(),
        grant.as_ref(),
    ))
}

/// Asks `question` on the terminal and answers whether the person said yes. No answer is no.
fn asked(question: &str) -> bool {
    eprint!("{question} [y/N] ");
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).is_ok()
        && matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// A summary as the terminal prints it, each risky item marked `!`, indented by `pad`. Every
/// field is the source's text, so each is escaped: no control or invisible character reaches the
/// terminal.
fn summary_text(summary: &purlis_core::skills::review::Summary, pad: &str) -> String {
    let mut out = String::new();
    if summary.is_empty() {
        out.push_str(&format!(
            "{pad}Instructions only: no scripts, hosts, ! commands or outside writes.\n"
        ));
    }
    for one in &summary.scripts {
        let mark = if one.risks.is_empty() { " " } else { "!" };
        let risks: Vec<String> = one.risks.iter().map(ToString::to_string).collect();
        let risks = if risks.is_empty() {
            String::new()
        } else {
            format!(": {}", risks.join(", "))
        };
        out.push_str(&format!(
            "{pad}{mark} script {} runs with {}{risks}\n",
            esc(&one.path),
            esc(&one.runs)
        ));
    }
    for one in &summary.hosts {
        let mark = if one.risky { "!" } else { " " };
        out.push_str(&format!(
            "{pad}{mark} host {} (in {})\n",
            esc(&one.host),
            esc(&one.files.join(", "))
        ));
    }
    for one in &summary.commands {
        out.push_str(&format!(
            "{pad}! command {}:{} runs `{}`\n",
            esc(&one.path),
            one.line,
            esc(&one.command)
        ));
    }
    for one in &summary.outside {
        out.push_str(&format!(
            "{pad}! writes {} ({}:{})\n",
            esc(&one.target),
            esc(&one.path),
            one.line
        ));
    }
    for one in &summary.unseen {
        let kinds: Vec<String> = one.kinds.iter().map(ToString::to_string).collect();
        out.push_str(&format!(
            "{pad}! hidden text {}:{}: {}\n",
            esc(&one.path),
            one.line,
            kinds.join(", ")
        ));
    }
    for path in &summary.not_text {
        out.push_str(&format!("{pad}  not text, not shown: {}\n", esc(path)));
    }
    out
}

/// An install review as the terminal prints it: each skill's summary first, then its files.
fn review_text(review: &source::Review) -> String {
    let mut out = format!(
        "Review of {} from {} at {}\n",
        counted(review.skills.len(), "skill", "skills"),
        esc(&review.source.to_string()),
        review.commit.short()
    );
    for skill in &review.skills {
        let brought = skill
            .brought_by
            .as_deref()
            .map(|by| format!(" (comes along: {by} refers to it)"))
            .unwrap_or_default();
        out.push_str(&format!(
            "\n{}{}: {}\n",
            esc(&skill.name),
            esc(&brought),
            esc(&skill.description)
        ));
        out.push_str(&summary_text(&skill.summary, "  "));
        for line in &skill.lint {
            out.push_str(&format!("  lint: {}\n", esc(line)));
        }
        out.push_str(&format!("  Files ({}):\n", skill.files.len()));
        for file in &skill.files {
            let mark = if file.risky { "!" } else { " " };
            out.push_str(&format!(
                "  {mark} {} ({} bytes)\n",
                esc(&file.path),
                file.size
            ));
        }
        for left in &skill.left_out {
            out.push_str(&format!(
                "    not installed: {} ({})\n",
                esc(&left.path),
                left.why
            ));
        }
    }
    out.push_str(
        "\nThe review is read from the files: a script can do more when it runs, and the chat's \
         sandbox bounds that.\n",
    );
    out
}

/// An update's review as the terminal prints it: the summary's changes, then each file's diff,
/// the edits made here, and the skills that come along. Every field escaped.
fn update_text(update: &source::UpdateReview) -> String {
    let mut out = format!(
        "Update of {} at {}: {} -> {}\n",
        esc(&update.name),
        update.level,
        update.pinned.short(),
        update.newer.short()
    );
    if !update.from_pin {
        out.push_str(
            "The pinned commit is no longer in the source: compared with the copy here.\n",
        );
    }
    if update.changes.is_empty() {
        out.push_str("The summary does not change.\n");
    } else {
        out.push_str("Added:\n");
        out.push_str(&summary_text(&update.changes.added, "  "));
        out.push_str("Removed:\n");
        out.push_str(&summary_text(&update.changes.removed, "  "));
    }
    let changes = |files: &[source::FileChange], out: &mut String| {
        for file in files {
            let change = match file.change {
                source::Changed::Added => "added",
                source::Changed::Removed => "removed",
                source::Changed::Changed => "changed",
            };
            // The diff's lines are escaped by the core as it writes them.
            out.push_str(&format!("\n{} ({change})\n{}", esc(&file.path), file.diff));
        }
    };
    changes(&update.files, &mut out);
    if !update.edited.is_empty() {
        out.push_str(&format!(
            "\nEdited here since the install, and replaced by the update: {}\n",
            esc(&update.edited.join(", "))
        ));
        changes(&update.local_edits, &mut out);
    }
    for skill in &update.brought {
        out.push_str(&format!(
            "\nComes along: {} ({})\n",
            esc(&skill.name),
            esc(&skill.description)
        ));
        out.push_str(&summary_text(&skill.summary, "  "));
    }
    out
}

/// What a verb's answer comes to: 0 where it was done, 1 with its refusal said.
fn done<T>(answer: Result<T, String>) -> Result<Code, String> {
    match answer {
        Ok(_) => Ok(0),
        Err(why) => {
            // A refusal may quote a source's own words: escaped like everything else it says.
            voice::err(&esc(&why));
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
    reach.extend(
        users
            .persona
            .iter()
            .map(|persona| format!("chats as {persona}, in every workspace")),
    );
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
