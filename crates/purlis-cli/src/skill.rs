// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill list | show | new | edit | mv | rm | restore | grant | share | install | update |
//! runs | history | revert | deprecate | archive | owners | uses | pin` (SK-2, #2765; SK-4, #2767;
//! SK-3, #2766; SK-7, #2770; SK-19, #2782; SK-15, #2778): the skills a chat in this project gets,
//! and why each other one is left out or hidden, as the Skill studio shows them; making, editing,
//! renaming, moving, deleting and restoring a skill, each one commit; where a project skill
//! reaches and what a workspace turned off; Share; every run of a skill with its signals; a
//! skill's history, its lifecycle state, its owners and the skills it names and is named by; and
//! the pins of a workspace or a persona. The same verbs the studio calls
//! (`purlis_core::skills::verbs`, `history`, `lifecycle`, `graph`). Behind the `skill-studio` flag
//! until its finish line (#2786).

use std::path::Path;

use clap::{Args, Subcommand, ValueEnum};
use purlis_core::executor::Executor;
use purlis_core::harness::Harness;
use purlis_core::skills::review::escaped as esc;
use purlis_core::skills::source::{self, Remote, Source, VaultEntry};
use purlis_core::skills::verbs::{self, By, Level};
use purlis_core::skills::{
    Clash, Deprecated, Grant, HarnessInTheStudio, Lifecycle, Outcome, StudioList, StudioRow,
    Switch, Template, TemplateId, clash_said, history, lifecycle,
};
use serde_json::{Value, json};

use purlis_core::counts::counted;
use purlis_core::tui::Markup;
use purlis_session_protocol::session::GradeLevel;

use crate::gradelink;
use crate::memory::Code;
use crate::voice;

/// `purlis skill …`.
#[derive(Subcommand)]
pub enum SkillCommand {
    /// Every skill a chat at the plane root gets, with its level (`built-in`, `project`,
    /// `workspace`, `persona`, and `local` for a local skill), and
    /// every skill left out of it with why. With `-w`, a chat in that workspace's; with `-p`, a
    /// chat as that persona, which brings its skills into every workspace. Then, under each
    /// harness, the skills it finds itself, read only: in your own folders and its plugins
    /// (`installed`), and in the workspace's repos (`repo`), each with whose it is.
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
    /// the plane root gets it, or why not, as `list` shows them, a harness's own copies with the
    /// harness and whose each is. With `-w`, a chat in that workspace. Exits 2 when no skill has
    /// the name.
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
    /// Pin a skill for a workspace's chats (`-w`) or for chats as a persona (`-p`): they always
    /// list it, and the router never hides it behind the index skill when their skills outgrow
    /// the budget. `--off` unpins it.
    #[command(group(clap::ArgGroup::new("by").required(true).args(["workspace", "persona"])))]
    Pin {
        /// The skill's name, as `purlis skill list` prints it.
        name: String,
        /// The workspace whose chats pin it.
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
        /// The persona whose chats pin it.
        #[arg(short = 'p', long = "persona")]
        persona: Option<String>,
        /// Unpin it instead.
        #[arg(long)]
        off: bool,
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
        /// What it starts from, of purlis's own templates.
        #[arg(
            long,
            value_enum,
            default_value = "blank",
            conflicts_with = "extension_template"
        )]
        template: TemplateArg,
        /// Start from a template an extension offers instead: the extension's id, then the
        /// template's.
        #[arg(long, num_args = 2, value_names = ["EXTENSION", "TEMPLATE"])]
        extension_template: Option<Vec<String>>,
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
        /// Restore it under this new name instead: its `SKILL.md`'s `name:` and its folder are
        /// renamed to match. A skill whose name belongs to a built-in skill now comes back only so.
        #[arg(long = "as", value_name = "NEW_NAME", requires = "name")]
        under: Option<String>,
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
    /// skill named, lists what the source offers; with `--all`, installs every skill it offers
    /// that is not installed from it yet, through one review and one commit.
    Install {
        /// The source: an https:// URL, or a folder on this machine by its full path.
        url: String,
        /// The skills to install, by name.
        #[arg(conflicts_with = "all")]
        names: Vec<String>,
        /// Every skill the source offers that is not installed from it yet, in one review, asked
        /// once (`--yes` skips the question). Past what one read of the source may take, it says
        /// which skills fit, to install in parts.
        #[arg(long)]
        all: bool,
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
        /// Where a project skill reaches: `root`, `all`, or workspace names (default: root). A
        /// project skill they refer to reaches those workspaces too: its grant is widened.
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
    /// A skill's grades, and what grading it runs before anything does: its trigger tests, the
    /// chats a grade runs (each test three times with the skill and three without it, one turn
    /// each) and what its last grade spent per chat; then each grade the project's eval record
    /// holds, newest first, with each arm's pass rate and the difference the skill made. A
    /// grade runs from the skill's tab in the Skill studio, which shows this plan first, or with
    /// `--run` from a terminal, on the same plan and caps, once you answer yes.
    Grade {
        /// The skill's name, as `purlis skill list` prints it.
        name: String,
        #[command(flatten)]
        at: LevelArgs,
        /// Answer as JSON, for a chat or a script.
        #[arg(long, conflicts_with = "run")]
        json: bool,
        /// Run the grade: in the purlis window that has this project open, which spends your
        /// account on real chats. Asks first. Refused inside a chat.
        #[arg(long)]
        run: bool,
        /// The profile its chats run on (default: the project's).
        #[arg(long, requires = "run")]
        profile: Option<String>,
        /// The model its chats run on (default: the profile's own).
        #[arg(long, requires = "run")]
        model: Option<String>,
        /// An outcome eval to run too, by its id; repeat for each. None run unless named.
        #[arg(long = "eval", value_name = "ID", requires = "run")]
        evals: Vec<String>,
        /// Do not ask: for a script that has read the plan. It still runs only what the plan
        /// printed.
        #[arg(long, requires = "run")]
        yes: bool,
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

    /// The level as a terminal names it to the window.
    fn wire_level(&self) -> GradeLevel {
        let local = self.local;
        match (&self.workspace, &self.persona) {
            (Some(name), _) => GradeLevel::Workspace {
                name: name.clone(),
                local,
            },
            (None, Some(name)) => GradeLevel::Persona {
                name: name.clone(),
                local,
            },
            (None, None) => GradeLevel::Project { local },
        }
    }
}

/// One of purlis's own templates, as the command line names it.
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
                | SkillCommand::Restore { name: None, .. }
                | SkillCommand::Update {
                    name: None,
                    confirm: None,
                    ..
                }
                | SkillCommand::Runs { .. }
                | SkillCommand::Grade { .. }
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
    // A delete a crash or a quit cut short is finished or undone before any change (SK-35).
    // And the skills this machine kept deleted past their time are removed (SK-33), but before a
    // restore, which `purlis skill restore`'s list offered the person as they saw it.
    if writes {
        verbs::interrupted(&mut cx);
        if !matches!(command, SkillCommand::Restore { .. }) {
            verbs::expire_now(root);
        }
    }
    // The studio's own list (SK-34, #3008): the plane's skills, and each harness's, the same
    // call the window makes.
    let list_for = |workspace: Option<&str>, persona: Option<&str>| {
        purlis_core::skills::studio_list(
            built_in.as_deref(),
            root,
            workspace,
            persona,
            &purlis_core::harness_plugin::Env::of(&[]),
        )
    };
    match command {
        SkillCommand::List {
            workspace,
            persona,
            json,
        } => {
            let list = list_for(workspace.as_deref(), persona.as_deref());
            if json {
                println!(
                    "{}",
                    json!({
                        "skills": list.rows.iter().map(|row| plane_json(&list, row))
                            .collect::<Vec<_>>(),
                        "harnesses": list.harnesses.iter().map(|one| json!({
                            "harness": one.harness.name(),
                            "title": one.harness.title(),
                            "skills": one.rows.iter().map(|row| outside_json(one.harness, row))
                                .collect::<Vec<_>>(),
                            "repo_left_out": one.repo_left_out.iter()
                                .map(|row| left_out_json(one, row, row_json(row)))
                                .collect::<Vec<_>>(),
                            // Its own catalogue, which purlis's budget never counts (#3324).
                            "own": one.own.map(|own| json!({
                                "skills": own.skills,
                                "chars": own.chars,
                                "manual": own.manual,
                                "said": own.said(one.harness.title()),
                            })),
                        })).collect::<Vec<_>>(),
                        // A declared harness skills are not supported on, with why (SK-30).
                        "not_supported": list.not_supported.iter().map(|one| json!({
                            "harness": one.name,
                            "title": one.title,
                            "said": one.said,
                        })).collect::<Vec<_>>(),
                    })
                );
            } else {
                print!("{}", table(&list.rows, &|row| clashes_said(&list, row)));
                for one in &list.harnesses {
                    if !one.rows.is_empty() {
                        print!(
                            "\n{} finds these itself, read only:\n{}",
                            one.harness.title(),
                            outside_table(&one.rows)
                        );
                    }
                    // What its own catalogue costs, beside purlis's budget (#3324).
                    if let Some(said) = one.own.and_then(|own| own.said(one.harness.title())) {
                        println!("{}", harness_line(&said));
                    }
                    // What a repo takes from its chats, never a silent drop (SK-53).
                    if !one.repo_left_out.is_empty() {
                        print!(
                            "\n{} chats in a repo here start without these:\n{}",
                            one.harness.title(),
                            table(&one.repo_left_out, &|_| Vec::new())
                        );
                    }
                }
                // Never a silent drop (SK-30): a declared harness's chats get no skills, and why.
                for one in &list.not_supported {
                    println!("\n{}", harness_line(&one.said));
                }
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
            // A harness's own copies follow the plane's, harness by harness, as the studio lists
            // them (SK-34).
            let list = list_for(workspace.as_deref(), persona.as_deref());
            let copies: Vec<(CopyOf, &StudioRow)> = list
                .rows
                .iter()
                .map(|row| (CopyOf::Plane, row))
                .chain(list.harnesses.iter().flat_map(|one| {
                    one.rows
                        .iter()
                        .map(move |row| (CopyOf::Outside(one), row))
                        .chain(
                            one.repo_left_out
                                .iter()
                                .map(move |row| (CopyOf::LeftOut(one), row)),
                        )
                }))
                .filter(|(_, row)| row.name == name)
                .collect();
            if copies.is_empty() {
                voice::err(format!(
                    "No skill is named {name:?} here. `purlis skill list` lists every one."
                ));
                return Ok(2);
            }
            if json {
                let shown: Vec<Value> = copies
                    .iter()
                    .map(|(copy, row)| match copy {
                        CopyOf::Plane => plane_json(&list, row),
                        CopyOf::Outside(one) => outside_json(one.harness, row),
                        CopyOf::LeftOut(one) => {
                            left_out_json(one, row, outside_json(one.harness, row))
                        }
                    })
                    .collect();
                println!("{}", json!({ "copies": shown }));
            } else {
                let shown: Vec<String> = copies
                    .iter()
                    .map(|(copy, row)| match copy {
                        CopyOf::Plane => detail(None, row, &clashes_said(&list, row)),
                        CopyOf::Outside(one) | CopyOf::LeftOut(one) => {
                            detail(Some(one.harness), row, &[])
                        }
                    })
                    .collect();
                print!("{}", shown.join("\n"));
            }
            Ok(0)
        }
        SkillCommand::New {
            name,
            description,
            template,
            extension_template,
            at,
        } => {
            let template = match extension_template.as_deref() {
                Some([extension, template]) => TemplateId::Extension {
                    extension: extension.clone(),
                    template: template.clone(),
                },
                _ => TemplateId::BuiltIn(template.into()),
            };
            // purlis's own templates, and each extension's that declares `skill-template`, asked
            // through the executor's gate as the studio asks them (ADR 0101).
            let executor =
                Executor::with_built_in(purlis_core::extension::BuiltIn::of_this_program());
            done(purlis_core::skills::studio::with_studio(
                &executor,
                root,
                |studio| {
                    verbs::create(
                        &mut cx,
                        &at.level(),
                        &name,
                        &description,
                        &purlis_core::skills::studio::Templates { studio },
                        &template,
                    )
                },
            ))
        }
        SkillCommand::Edit { name, from, at } => {
            let level = at.level();
            let (_, was) = match verbs::text_of(root, &level, &name) {
                Ok(read) => read,
                Err(why) => {
                    voice::err(why.to_string());
                    return Ok(1);
                }
            };
            let now = match new_text(from.as_deref(), &was) {
                Ok(now) => now,
                Err(why) => {
                    voice::err(why.to_string());
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
                    voice::err(why.to_string());
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
                        voice::err(format!(
                            "{why}: --grant takes root, all, or workspace names"
                        ));
                        return Ok(2);
                    }
                }
            };
            if verbs::needs_grant(&level, &target) && grant.is_none() {
                voice::err(format!(
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
                    voice::err(why.to_string());
                    return Ok(1);
                }
            }
            done(verbs::delete(&mut cx, &level, &name))
        }
        SkillCommand::Restore { name: None, .. } => {
            let deleted = verbs::deleted(root);
            if deleted.is_empty() {
                println!("No deleted skill can be restored here.");
            }
            let now = std::time::SystemTime::now();
            let reserved = purlis_core::skills::Reserved::of(built_in.as_deref());
            for one in deleted {
                let line = restorable(&one.name, &one.level, &one.stays(now));
                // Its name a built-in skill took (#3729): it comes back under a new one.
                let line = if reserved.holds(&one.name) {
                    line.own(" · its name belongs to a built-in skill now: restore it with --as <new-name>")
                } else {
                    line
                };
                println!("{line}");
            }
            Ok(0)
        }
        SkillCommand::Restore {
            name: Some(name),
            under,
        } => {
            let Some(one) = verbs::deleted(root)
                .into_iter()
                .find(|one| one.name == name)
            else {
                voice::err(format!(
                    "No deleted skill named {name:?} can be restored: `purlis skill restore` lists those that can."
                ));
                return Ok(2);
            };
            done(verbs::restore(&mut cx, &one, under.as_deref()))
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
                    voice::ok(already_switched(&name, state, &workspace));
                }
                Ok(0)
            }
            Err(why) => {
                voice::err(why.to_string());
                if on && why.contains("would reach it") {
                    voice::info(format!(
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
                    voice::err(format!(
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
                    voice::err(why.to_string());
                    Ok(1)
                }
            }
        }
        SkillCommand::Pin {
            name,
            workspace,
            persona,
            off,
        } => {
            let by = match (workspace, persona) {
                (Some(ws), _) => purlis_core::skills::PinnedBy::Workspace(ws),
                (None, Some(persona)) => purlis_core::skills::PinnedBy::Persona(persona),
                (None, None) => unreachable!("clap requires -w or -p"),
            };
            let refused = match by {
                purlis_core::skills::PinnedBy::Workspace(_) => {
                    crate::brokered::manifest_refused("skill pin")
                }
                purlis_core::skills::PinnedBy::Persona(_) => None,
            };
            match refused.map_or_else(|| verbs::pin(&mut cx, &by, &name, !off), Err) {
                Ok(true) => Ok(0),
                Ok(false) => {
                    let state = if off { "not pinned" } else { "already pinned" };
                    voice::ok(format!("{name} was {state} there."));
                    Ok(0)
                }
                Err(why) => {
                    voice::err(why);
                    Ok(1)
                }
            }
        }
        SkillCommand::Share { name, at } => {
            // Whose local skill it is: a share is always of a local one.
            match verbs::share(&mut cx, &at.level().at, &name) {
                Ok(_) => Ok(0),
                Err(why) => {
                    voice::err(why.to_string());
                    Ok(1)
                }
            }
        }
        SkillCommand::Install {
            url,
            names,
            all,
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
            let names = if all {
                Named::All
            } else {
                Named::These(&names)
            };
            install(&mut cx, &source, names, &grant, yes, &at.level())
        }
        SkillCommand::Update {
            confirm: Some(url),
            branch,
            ..
        } => match source::confirm(&mut cx, &Remote { url, branch }) {
            Ok(_) => Ok(0),
            Err(why) => {
                voice::err(esc(&why).to_string());
                Ok(1)
            }
        },
        SkillCommand::Update { name: None, .. } => {
            let found = source::updates(root);
            for one in &found.unasked {
                voice::warn(esc(&format!("{} ({}): {}", one.name, one.level, one.why)).to_string());
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
            let update = match source::update_review(root, built_in.as_deref(), &level, &name) {
                Ok(update) => update,
                Err(why) => {
                    voice::err(esc(&why).to_string());
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
            done(source::update(
                &mut cx,
                &level,
                &name,
                &update.accepted(),
                update.grant.as_ref(),
            ))
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
                    voice::warn(line.to_string());
                }
                print!("{}", diff.text);
                if diff.cut {
                    voice::info(format!(
                        "The diff is cut at {} KiB.",
                        history::MOST_DIFF / 1024
                    ));
                }
                Ok(0)
            }
            Err(why) => {
                voice::err(why.to_string());
                Ok(1)
            }
        },
        SkillCommand::Grade {
            name,
            at,
            run: true,
            profile,
            model,
            evals,
            yes,
            ..
        } => Ok(gradelink::run(
            root,
            &gradelink::Asked {
                wire: at.wire_level(),
                level: at.level(),
                name,
                profile,
                model,
                evals,
                yes,
            },
        )),
        SkillCommand::Grade { name, at, json, .. } => {
            let level = at.level();
            // The terminal knows no profile: the estimate is the newest grade's whose chats ran.
            match purlis_core::skills::grades::planned(root, &level, &name, None) {
                Ok(planned) => {
                    // The record as the plan read it, once: its rows, and what it left out
                    // (`plan.record_said`).
                    if json {
                        println!(
                            "{}",
                            json!({ "plan": planned, "grades": planned.record.grades })
                        );
                    } else {
                        let local_file = purlis_core::settings::Which::Local.file_at(root);
                        print!(
                            "{}",
                            grade_said(&planned, &planned.record.grades, local_file)
                        );
                        if let Some(said) = &planned.record_said {
                            voice::warn(said.clone());
                        }
                    }
                    Ok(0)
                }
                Err(why) => {
                    voice::err(why);
                    Ok(1)
                }
            }
        }
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
                    voice::err(why.to_string());
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
                    voice::warn(widens.to_string());
                    if !yes && !asked(&format!("Revert {name} to {}?", history::short(&commit))) {
                        voice::info("Nothing was reverted.");
                        return Ok(1);
                    }
                }
                Ok(None) => {}
                Err(why) => {
                    voice::err(why.to_string());
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
                        for line in owners_lines(&name, &about.owners) {
                            println!("{line}");
                        }
                        Ok(0)
                    }
                    Err(why) => {
                        voice::err(why.to_string());
                        Ok(1)
                    }
                };
            }
            match lifecycle::set_owners(&mut cx, &at.level(), &name, &owners) {
                Ok(true) => Ok(0),
                Ok(false) => {
                    voice::ok(owners_unchanged(&name));
                    Ok(0)
                }
                Err(why) => {
                    voice::err(why.to_string());
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
            for line in uses_lines(&name, &node) {
                println!("{line}");
            }
            if node.cut {
                voice::info(format!(
                    "The project holds more than {} skills, and the rest were not read.",
                    purlis_core::skills::graph::MOST_SKILLS
                ));
            }
            Ok(0)
        }
    }
}

/// One deleted skill as `skill restore` lists it: its name and its level, each escaped.
/// `<name> was already on in <workspace>.`: both names are spliced escaped (SK-54, #3225).
fn already_switched(name: &str, state: &'static str, workspace: &str) -> Markup {
    Markup::new()
        .text(name)
        .own(" was already ")
        .own(state)
        .own(" in ")
        .text(workspace)
        .own(".")
}

/// `<name>'s owners were already those.`, the name escaped (SK-54).
fn owners_unchanged(name: &str) -> Markup {
    Markup::new()
        .text(name)
        .own("'s owners were already those.")
}

/// `<name> was already <state>.`, the name escaped (SK-54).
fn already_in(name: &str, lifecycle: &Lifecycle) -> Markup {
    Markup::new()
        .text(name)
        .own(" was already ")
        .own(lifecycle.word())
        .own(".")
}

/// A line of the core's about a harness, whose title is the project's words, so the line is
/// escaped: that skills are not supported on a declared harness (SK-30), and what a harness's
/// own catalogue holds beside purlis's budget (#3324).
fn harness_line(said: &str) -> Markup {
    Markup::new().text(said)
}

fn restorable(name: &str, level: &Level, stays: &str) -> Markup {
    Markup::new()
        .text(name)
        .own("  ")
        .text(level)
        .own(" · ")
        .text(stays)
}

/// What `skill uses` says of `name`: each skill it names, as its text says it, then each skill
/// that names it. Every name is read from a skill's text, so each is escaped.
fn uses_lines(name: &str, node: &purlis_core::skills::graph::UsesOf) -> Vec<Markup> {
    let mut lines = Vec::new();
    if node.uses.is_empty() {
        lines.push(Markup::new().text(name).own(" names no other skill."));
    }
    for one in &node.uses {
        let line = Markup::new().own("uses ").text(&one.said);
        lines.push(if one.broken {
            line.own("  (no skill has this name)")
        } else {
            line
        });
    }
    if node.used_by.is_empty() {
        lines.push(
            Markup::new()
                .own("No skill of the project names ")
                .text(name)
                .own("."),
        );
    }
    for other in &node.used_by {
        lines.push(Markup::new().own("used by ").text(other));
    }
    lines
}

/// What `skill owners` prints of `name`: each owner its frontmatter names, one a line, or that
/// it names none. The owners are the skill's own text, so each is escaped.
fn owners_lines(name: &str, owners: &[String]) -> Vec<Markup> {
    if owners.is_empty() {
        return vec![Markup::new().text(name).own(" names no owners.")];
    }
    owners
        .iter()
        .map(|owner| Markup::new().text(owner))
        .collect()
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
            voice::ok(already_in(name, lifecycle));
            Ok(0)
        }
        Err(why) => {
            voice::err(why.to_string());
            Ok(1)
        }
    }
}

/// A skill's versions, one line each: its commit, when, who, its subject and its runs; then the
/// runs on changes not committed, where there were any.
fn versions_table(name: &str, history: &history::History) -> String {
    if history.versions.is_empty() {
        return Markup::new()
            .text(name)
            .own(" has no history yet: git keeps a skill's versions once the project commits it.\n")
            .to_string();
    }
    let mut text = String::new();
    for version in &history.versions {
        let when = chrono::DateTime::from_timestamp(i64::try_from(version.at).unwrap_or(0), 0)
            .map(|at| at.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();
        // The author and the subject are whatever the commit says.
        let line = Markup::new()
            .text(history::short(&version.commit))
            .own("  ")
            .text(&when)
            .own("  ")
            .text(&version.author)
            .own("  ")
            .text(&version.subject)
            .own("  (")
            .text(runs_said(&version.runs))
            .own(")\n");
        text.push_str(line.as_str());
    }
    if history.uncommitted.runs > 0 {
        text.push_str(&format!(
            "On changes not committed: {}\n",
            runs_said(&history.uncommitted)
        ));
    }
    text
}

/// **What a grade should spend, from a grade before, in a line** (#3273): which grade it is
/// worked out from, or that its cost is not known where that grade's chats did not say, or that
/// there was none.
fn estimate_said(estimate: Option<&purlis_core::skills::grades::Estimate>) -> String {
    let Some(estimate) = estimate else {
        return "It has not been graded here before, so what a grade spends is not known yet.\n"
            .to_owned();
    };
    let when = estimate.ts.get(..10).unwrap_or(&estimate.ts);
    let on = match (&estimate.profile, &estimate.harness) {
        (Some(profile), Some(harness)) => format!(
            " on {profile} ({})",
            purlis_core::skilluse::harness_title(harness)
        ),
        (Some(profile), None) => format!(" on {profile}"),
        (None, Some(harness)) => format!(" on {}", purlis_core::skilluse::harness_title(harness)),
        (None, None) => String::new(),
    };
    // The model its chats reported (SK-60): a grade on another model spends otherwise.
    let on = match &estimate.model {
        Some(model) => format!("{on} on {model}"),
        None => on,
    };
    if estimate.chats_then == 0 {
        return format!(
            "Its last grade{on}, {when}, ran no chat, so what a grade spends is not known yet.\n"
        );
    }
    let mut parts = Vec::new();
    if let Some(tokens) = estimate.tokens {
        parts.push(format!("about {tokens} tokens"));
    }
    if let Some(cost) = estimate.cost_usd {
        parts.push(format!("about ${cost:.2}"));
    }
    if parts.is_empty() {
        return format!(
            "Its last grade{on}, {when}, did not say what its chats spent, so what a grade spends \
             is not known.\n"
        );
    }
    // A grade purlis stopped under is gone by as far as its chats went, and says so (#3602).
    let which = if estimate.interrupted {
        "interrupted grade"
    } else {
        "grade"
    };
    format!(
        "From its {which}{on}, {when}, of {}, a grade spends {}.\n",
        counted(estimate.chats_then, "chat", "chats"),
        parts.join(", ")
    )
}

/// **What grading runs, cost first**: the part of [`grade_said`] above the grades, which
/// `purlis skill grade --run` prints before it asks.
pub(crate) fn plan_said(planned: &purlis_core::skills::grades::Planned, root: &Path) -> String {
    let local_file = purlis_core::settings::Which::Local.file_at(root);
    grade_plan_said(planned, local_file)
}

/// **`purlis skill grade`'s text**: the plan, cost first, then each grade, newest first.
fn grade_said(
    planned: &purlis_core::skills::grades::Planned,
    graded: &[purlis_core::skills::grades::Graded],
    local_file: &str,
) -> String {
    use purlis_core::skills::grade::Arm;
    let name = &planned.skill;
    let mut text = grade_plan_said(planned, local_file);
    if graded.is_empty() {
        text.push_str(&format!("No grades of {name} yet.\n"));
        return text;
    }
    text.push_str("\nGrades, newest first:\n");
    for one in graded {
        let rate = |arm: Arm| {
            one.report
                .arm(arm)
                .and_then(|score| score.pass_rate)
                .map_or_else(
                    || "not run".to_owned(),
                    |stat| format!("{:.0}%", stat.mean * 100.0),
                )
        };
        let (passed, of) = one.report.passed_with();
        // The chats that asked the person something (SK-59): ended there, and said here.
        let asked = match one.report.asked() {
            0 => String::new(),
            asked => format!(
                "  {} asked you something, and purlis ended {them} there without counting {them}",
                counted(asked, "chat", "chats"),
                them = if asked == 1 { "it" } else { "them" }
            ),
        };
        // The version it compared with, where it compared one (SK-57).
        let previous = one.compared_with().map(history::short);
        text.push_str(&format!(
            "  {}  {}  {passed} of {} passed  with {}  without {}{}{}{asked}  ({}){}\n",
            purlis_core::skills::review::escaped_line(&one.ts),
            one.commit.as_deref().map_or("uncommitted", history::short),
            counted(of, "test", "tests"),
            rate(Arm::With),
            rate(Arm::Without),
            previous
                .map(|commit| format!("  previous {commit} {}", rate(Arm::Previous)))
                .unwrap_or_default(),
            one.report
                .delta
                .map(|delta| match previous {
                    Some(commit) =>
                        format!("  difference {:+.0} points against {commit}", delta * 100.0),
                    None => format!("  difference {:+.0} points", delta * 100.0),
                })
                .unwrap_or_default(),
            match &one.model {
                // The model its chats reported (SK-60): a model's id, which the record checks.
                Some(model) => format!("{} on {model}", counted(one.spent.chats, "chat", "chats")),
                None => counted(one.spent.chats, "chat", "chats"),
            },
            // A grade a saved edit started says so (SK-58).
            if one.after_edit {
                "  after an edit"
            } else {
                ""
            },
        ));
    }
    text
}

/// The plan's part of [`grade_said`].
fn grade_plan_said(planned: &purlis_core::skills::grades::Planned, local_file: &str) -> String {
    use purlis_core::skills::grade::Read;
    let name = &planned.skill;
    let mut text = String::new();
    if let Read::Refused(why) = &planned.fixtures.triggers {
        text.push_str(&format!(
            "Its trigger tests could not be read: {}\n",
            why.terminal()
        ));
    }
    match (&planned.request, planned.plan) {
        (Some(request), Some(plan)) => {
            let times = counted(u32::from(request.runs), "time", "times");
            // The version it compares with too, where the skill has one (SK-57).
            let previous = match &planned.previous {
                Some(previous) => format!(
                    ", and {times} with its previous version, {} ({})",
                    history::short(&previous.commit),
                    previous.subject
                ),
                None => String::new(),
            };
            text.push_str(&format!(
                "Grading {name} runs {}, one turn each: each of {} {times} with the skill and \
                 {times} without it{previous}.\n",
                counted(plan.chats, "chat", "chats"),
                counted(plan.trigger_tests, "trigger test", "trigger tests"),
            ));
            if let Some(why) = &planned.previous_refused {
                text.push_str(&format!("Its previous version is not compared: {why}.\n"));
            }
            text.push_str(&estimate_said(planned.estimate.as_ref()));
            for (at, test) in request.triggers.iter().enumerate() {
                text.push_str(&format!(
                    "  {}. {}  ({})\n",
                    at + 1,
                    purlis_core::skills::review::escaped_line(&test.query),
                    if test.should_trigger {
                        "should start it"
                    } else {
                        "should leave it alone"
                    }
                ));
            }
            for left in &planned.left_out {
                text.push_str(&format!("Test {} is left out: {}\n", left.at + 1, left.why));
            }
            // The terminal knows no profile, so its limit is at the dearest known price: the
            // Skill studio's plan prices it from the model its profile's chats run on (#3469).
            let dearest = purlis_core::skills::grades::price::of_model(None);
            text.push_str(&format!(
                "A grade stops once its chats have spent {} ({}; [skill_grades] token_cap in {} \
                 sets it), or once they cost ${:.2}, its tokens at the dearest known price of \
                 ${:.2} a million; the Skill studio's plan prices it from the model the chats \
                 run on.\n",
                counted(planned.cap.tokens, "token", "tokens"),
                match planned.cap.from {
                    purlis_core::skills::grades::cap::From::Default => "the default",
                    purlis_core::skills::grades::cap::From::Yours => "this machine's cap",
                },
                local_file,
                planned.cap.cost_usd(&dearest),
                dearest.usd_per_million,
            ));
            text.push_str("Grade it from its tab in the Skill studio, which shows this first.\n");
        }
        _ => text.push_str(&format!(
            "{name} has no trigger tests to grade: the queries it should start on and the near \
             misses it should leave alone, in its evals/triggers.json.\n"
        )),
    }
    if let Some(evals) = planned
        .fixtures
        .outcomes
        .read()
        .filter(|evals| !evals.is_empty())
    {
        text.push_str(&format!(
            "{} for a later purlis: grades run the trigger tests alone.\n",
            counted(evals.len(), "outcome eval waits", "outcome evals wait"),
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
        let padded: Vec<(&str, usize)> = cells
            .iter()
            .zip(&widths)
            .map(|(c, w)| (*c, w + 2))
            .collect();
        let mut text = purlis_core::tui::row(&padded, cells[5]);
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

/// Which skills `purlis skill install` installs.
#[derive(Clone, Copy)]
enum Named<'a> {
    /// Those named, and the ones they refer to.
    These(&'a [String]),
    /// Every skill the source offers that is not installed from it yet (`--all`, #4024).
    All,
}

/// `purlis skill install`: what `source` offers where no skill is named; else the review of
/// `names` (or of every skill not installed from it yet), asked about, then the install at
/// `level`.
fn install(
    cx: &mut verbs::Ctx<'_>,
    source: &Source,
    named: Named<'_>,
    grant: &[String],
    yes: bool,
    level: &Level,
) -> Result<Code, String> {
    let root = cx.plane;
    if let Named::These(names) = named
        && names.is_empty()
    {
        let browse = match source::browse(root, source) {
            Ok(browse) => browse,
            Err(why) => {
                voice::err(esc(&why).to_string());
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
                voice::err(format!(
                    "{why}: --grant takes root, all, or workspace names"
                ));
                return Ok(2);
            }
        }
    };
    let review = match named {
        Named::These(names) => source::review(root, cx.built_in, source, names, level),
        Named::All => match source::review_all(root, cx.built_in, source, level) {
            Ok(source::SetReview::Review { review, there }) => {
                // Those the level has already, made there or from another source, or a
                // built-in's name: left as they are, and said (#4153).
                for name in &there {
                    voice::info(format!(
                        "{} is there already at {level}, so it is left as it is.",
                        esc(name)
                    ));
                }
                if review.skills.is_empty() {
                    voice::info(format!(
                        "{} {} offers is installed already.",
                        if there.is_empty() {
                            "Every skill"
                        } else {
                            "Every other skill"
                        },
                        esc(&source.to_string())
                    ));
                    return Ok(0);
                }
                Ok(review)
            }
            Ok(source::SetReview::TooMuch(too_much)) => {
                voice::err(esc(&too_much_text(&too_much)).to_string());
                return Ok(1);
            }
            Err(why) => Err(why),
        },
    };
    let review = match review {
        Ok(review) => review,
        Err(why) => {
            voice::err(esc(&why).to_string());
            return Ok(1);
        }
    };
    // A project skill it relies on reaches the workspaces the grant names too (#4172).
    let widenings = source::widenings(
        root,
        level,
        grant.as_ref(),
        review
            .skills
            .iter()
            .flat_map(|skill| skill.already.iter().map(String::as_str)),
    );
    print!("{}", review_text(&review, &widenings));
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
        voice::warn(
            esc(&format!(
                "The source was not added to this project's sources: {why}"
            ))
            .to_string(),
        );
    }
    done(source::install(
        cx,
        source,
        level,
        &review.accepted(),
        grant.as_ref(),
    ))
}

/// What `--all` says where its review is more than one read of the source may take: why, and the
/// skills to install first, by name, so the rest follow with `--all` again (#4024). Nothing is
/// cut from a review silently.
fn too_much_text(too_much: &source::TooMuch) -> String {
    if too_much.fits.is_empty() {
        return format!("{}, so nothing was reviewed.", too_much.why);
    }
    format!(
        "{}, so nothing was reviewed. Install them in two parts: first run this command with \
         these {} named in place of --all: {}; then run it with --all again for the other {}.",
        too_much.why,
        counted(too_much.fits.len(), "skill", "skills"),
        too_much.fits.join(" "),
        too_much.rest.len()
    )
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
/// `widenings` says what the install's grant asks of the project skills they refer to (#4172).
fn review_text(review: &source::Review, widenings: &[source::Widening]) -> String {
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
        out.push_str(&already_text(skill, widenings, "install"));
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

/// What a reviewed skill says of each reference it leaves as it is: met there already, or met by
/// a project skill whose grant `by` (`install`, `update`) widens (#4172, #4186), and each
/// workspace that turned that skill off.
fn already_text(skill: &source::SkillReview, widenings: &[source::Widening], by: &str) -> String {
    let mut out = String::new();
    for name in &skill.already {
        let one = widenings.iter().find(|one| &one.name == name);
        match one.and_then(|one| one.said(by)) {
            Some(said) => out.push_str(&format!("  {}\n", esc(&said))),
            None => out.push_str(&format!(
                "  refers to {}, which its chats there have already: it is left as it is\n",
                esc(name)
            )),
        }
        for line in one
            .map(|one| one.turned_off_said(&skill.name))
            .unwrap_or_default()
        {
            out.push_str(&format!("  {}\n", esc(&line)));
        }
    }
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
    if update
        .review
        .already
        .iter()
        .any(|name| update.widenings.iter().any(|one| &one.name == name))
    {
        out.push_str(&format!("\n{}:\n", esc(&update.name)));
        out.push_str(&already_text(&update.review, &update.widenings, "update"));
    }
    for skill in &update.brought {
        out.push_str(&format!(
            "\nComes along: {} ({})\n",
            esc(&skill.name),
            esc(&skill.description)
        ));
        if let Some(grant) = &update.grant {
            out.push_str(&format!(
                "  granted to {}, as {} is\n",
                esc(&grant.to_string()),
                esc(&update.name)
            ));
        }
        out.push_str(&summary_text(&skill.summary, "  "));
        out.push_str(&already_text(skill, &update.widenings, "update"));
    }
    out
}

/// What a verb's answer comes to: 0 where it was done, 1 with its refusal said.
fn done<T>(answer: Result<T, String>) -> Result<Code, String> {
    match answer {
        Ok(_) => Ok(0),
        Err(why) => {
            // A refusal may quote a source's own words: escaped like everything else it says.
            voice::err(esc(&why).to_string());
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
        voice::info(format!("Used by {}", reach.join(", ")));
    }
    if !users.personas.is_empty() {
        voice::info(format!(
            "Named by {}: {}",
            purlis_core::counts::counted(users.personas.len(), "persona", "personas"),
            users.personas.join(", ")
        ));
    }
    if !users.skills.is_empty() {
        voice::info(format!(
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
        None => match row.hidden.first() {
            Some(first) => {
                let on: Vec<&str> = row.hidden.iter().map(|one| one.harness.title()).collect();
                format!("on, {} (on {})", first.hidden, on.join(", "))
            }
            None if row.pinned => "on, pinned".to_owned(),
            None => "on".to_owned(),
        },
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
        "hidden": row
            .hidden
            .iter()
            .map(|one| json!({
                "harness": one.harness.title(),
                "router": one.hidden.router,
                "why": one.hidden.why,
                "said": one.hidden.to_string(),
            }))
            .collect::<Vec<_>>(),
        "pinned": row.pinned,
    })
}

/// Whose list a copy `show` prints is from: the plane's, the skills a harness finds itself, or
/// the plane's a repo leaves out of that harness's chats in it.
enum CopyOf<'a> {
    Plane,
    Outside(&'a HarnessInTheStudio),
    LeftOut(&'a HarnessInTheStudio),
}

/// A plane or built-in row as JSON: [`row_json`]'s, with each skill from outside the plane that
/// has its name (SK-46, #3149), as the studio marks it.
fn plane_json(list: &StudioList, row: &StudioRow) -> Value {
    let mut shown = row_json(row);
    shown["clashes"] = list
        .clashes_of(row)
        .map(|(harness, clash)| clash_json(harness, clash))
        .collect();
    shown
}

/// `shown`, a plane row a repo leaves out of `one`'s chats in it, with the clash that left it
/// out (#3461): the repo's copy that won, as a row purlis hands says each of its clashes.
fn left_out_json(one: &HarnessInTheStudio, row: &StudioRow, mut shown: Value) -> Value {
    shown["clashes"] = one
        .clashes_of_left_out(row)
        .map(|clash| clash_json(one.harness, clash))
        .collect();
    shown
}

/// One clash on `harness` as JSON, with what the studio says of it.
fn clash_json(harness: Harness, clash: &Clash) -> Value {
    json!({
        "harness": harness.name(),
        "outside": clash.outside.level(),
        "owner": clash.owner,
        "outcome": match clash.outcome {
            Outcome::BothLoad => "both-load",
            Outcome::RepoWins => "repo-wins",
            Outcome::EitherOne => "either-one",
        },
        "said": clash_said(harness, clash),
    })
}

/// What the studio says of each clash of a plane or built-in row (SK-46, #3149), harness by
/// harness: the lines `list` and `show` add to it, each once (two harnesses on which both
/// copies load say the same).
fn clashes_said(list: &StudioList, row: &StudioRow) -> Vec<String> {
    let mut said: Vec<String> = Vec::new();
    for line in list
        .clashes_of(row)
        .map(|(harness, clash)| clash_said(harness, clash))
    {
        if !said.contains(&line) {
            said.push(line);
        }
    }
    said
}

/// One copy of a skill, as `show` prints it: one a harness finds itself with that harness,
/// whose it is and how it can be left out, and for purlis's copy each clash `also` says.
fn detail(harness: Option<Harness>, row: &StudioRow, also: &[String]) -> String {
    let mut text = format!("{}\n  level: {}\n", esc(&row.name), level(row));
    if let Some(harness) = harness {
        text.push_str(&format!("  harness: {}\n", harness.title()));
    }
    if let Some(outside) = &row.outside {
        text.push_str(&format!("  owner: {}\n", esc(&outside.owner.to_string())));
        if let Switch::Not(why) = &outside.switch {
            text.push_str(&format!("  switch: {}\n", esc(why)));
        }
    }
    text.push_str(&format!("  folder: {}\n", esc(&row.folder)));
    if !row.description.is_empty() {
        text.push_str(&format!("  description: {}\n", esc(&row.description)));
    }
    if let Some(grant) = &row.grant {
        text.push_str(&format!("  grant: {}\n", esc(grant)));
    }
    text.push_str(&format!("  in this chat's skills: {}\n", esc(&state(row))));
    for said in also {
        text.push_str(&format!("  also: {}\n", esc(said)));
    }
    text
}

/// The rows of a chat's skills, each with its state and, after it, each line `also` says of it
/// (a clash, SK-46). A repo's names are the least trusted text here, so every cell is escaped.
fn table(rows: &[StudioRow], also: &dyn Fn(&StudioRow) -> Vec<String>) -> String {
    if rows.is_empty() {
        return "No skills here yet. A project skill is a folder in skills/ at the plane root, \
                holding its SKILL.md.\n"
            .to_owned();
    }
    let states: Vec<String> = rows
        .iter()
        .map(|row| {
            let mut said = state(row);
            for one in also(row) {
                said.push_str("; also: ");
                said.push_str(&one);
            }
            esc(&said)
        })
        .collect();
    let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
    let name_w = voice::column(names.iter().copied().chain(["SKILL"]));
    let levels: Vec<String> = rows.iter().map(level).collect();
    let level_w = voice::column(levels.iter().map(String::as_str).chain(["LEVEL"]));
    let mut text = format!(
        "{}  {}  STATE\n",
        voice::pad("SKILL", name_w),
        voice::pad("LEVEL", level_w)
    );
    for ((name, state), level) in names.iter().zip(&states).zip(&levels) {
        text.push_str(&format!(
            "{}  {}  {}\n",
            voice::pad(name, name_w),
            voice::pad(level, level_w),
            state
        ));
    }
    text
}

/// A row of a skill a harness finds itself, as JSON: [`row_json`]'s, with the harness, whose it
/// is and how it switches, spelled as the window's (`kind`, then its `name` or `why`).
fn outside_json(harness: Harness, row: &StudioRow) -> Value {
    let mut shown = row_json(row);
    shown["harness"] = json!(harness.name());
    if let Some(outside) = &row.outside {
        shown["owner"] = json!(outside.owner);
        shown["switch"] = json!(outside.switch);
    }
    shown
}

/// The skills one harness finds itself, with whose each is: read only. A repo's names are the
/// least trusted text here; [`voice::pad`] escapes every padded cell before it is measured, and
/// the last column, which nothing pads, is escaped here.
fn outside_table(rows: &[StudioRow]) -> String {
    let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
    let owners: Vec<String> = rows
        .iter()
        .map(|row| {
            row.outside
                .as_ref()
                .map(|o| o.owner.to_string())
                .unwrap_or_default()
        })
        .collect();
    let levels: Vec<String> = rows.iter().map(level).collect();
    let name_w = voice::column(names.iter().copied().chain(["SKILL"]));
    let level_w = voice::column(levels.iter().map(String::as_str).chain(["LEVEL"]));
    let owner_w = voice::column(owners.iter().map(String::as_str).chain(["OWNER"]));
    let mut text = format!(
        "{}  {}  {}  STATE\n",
        voice::pad("SKILL", name_w),
        voice::pad("LEVEL", level_w),
        voice::pad("OWNER", owner_w)
    );
    for (((row, name), level), owner) in rows.iter().zip(&names).zip(&levels).zip(&owners) {
        text.push_str(&format!(
            "{}  {}  {}  {}\n",
            voice::pad(name, name_w),
            voice::pad(level, level_w),
            voice::pad(owner, owner_w),
            esc(&state(row))
        ));
    }
    text
}

#[cfg(test)]
mod tests;
