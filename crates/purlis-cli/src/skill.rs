// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill list | show | new | edit | mv | rm | restore | grant | share | install | update |
//! export | publish |
//! runs | history | revert | deprecate | archive | owners | uses | pin` (SK-2, #2765; SK-4, #2767;
//! SK-3, #2766; SK-7, #2770; SK-19, #2782; SK-15, #2778): the skills a chat in this project gets,
//! and why each other one is left out or hidden, as the Skill studio shows them; making, editing,
//! renaming, moving, deleting and restoring a skill, each one commit; where a project skill
//! reaches and what a workspace turned off; Share; every run of a skill with its signals; a
//! skill's history, its lifecycle state, its owners and the skills it names and is named by; and
//! the pins of a workspace or a persona. The same verbs the studio calls
//! (`purlis_core::skills::verbs`, `history`, `lifecycle`, `graph`).

use crate::clierror::CliError;
use std::path::Path;

use clap::{Args, Subcommand, ValueEnum};
use purlis_core::executor::Executor;
use purlis_core::harness::Harness;
use purlis_core::skills::review::escaped as esc;
use purlis_core::skills::source::{self, Remote, Source, VaultEntry};
use purlis_core::skills::verbs::set::{self, SetChange};
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
    /// the budget. `--off` unpins it. Several names, or `--source`, pin them all as one change,
    /// after its review; in a chat, the change is proposed to the person instead.
    #[command(group(clap::ArgGroup::new("by").required(true).args(["workspace", "persona"])))]
    Pin {
        /// The skills' names, as `purlis skill list` prints them.
        #[arg(required_unless_present = "source", conflicts_with = "source")]
        names: Vec<String>,
        #[command(flatten)]
        set: SetArgs,
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
        /// template's. What it answers is kept as a draft in the Skill studio, for you to try
        /// and accept, never made at once.
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
    /// project's is added to the persona's skills, so its chats keep it. Several names, or
    /// `--source`, move them all to one level as one change, after its review, asked at your
    /// terminal; anywhere else the move is proposed to the person, in the window.
    Mv {
        /// The skills' names, as `purlis skill list` prints them.
        #[arg(required_unless_present = "source", conflicts_with = "source")]
        names: Vec<String>,
        #[command(flatten)]
        set: SetArgs,
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
    /// Delete a skill, after saying who uses it. `purlis skill restore` brings it back. Several
    /// names, or `--source` (every copy installed from it, at the level named or at every
    /// level), delete them all as one change, after its review, asked at your terminal. Anywhere
    /// else (a chat, a script, no terminal) the delete is proposed to the person, who alone makes
    /// it, in the window.
    Rm {
        /// The skills' names, as `purlis skill list` prints them.
        #[arg(required_unless_present = "source", conflicts_with = "source")]
        names: Vec<String>,
        #[command(flatten)]
        set: SetArgs,
        #[command(flatten)]
        at: LevelArgs,
    },
    /// Turn project skills off in a workspace, as one change, after its review: those its chats
    /// get no more. Named, or every one installed from `--source`. In a chat, the change is
    /// proposed to the person instead.
    Off {
        /// The skills' names, as `purlis skill list` prints them.
        #[arg(required_unless_present = "source", conflicts_with = "source")]
        names: Vec<String>,
        /// The workspace that turns them off.
        #[arg(short = 'w', long = "workspace", required = true)]
        workspace: String,
        #[command(flatten)]
        set: SetArgs,
    },
    /// Turn project skills back on in a workspace, as one change, where the project granted them
    /// there. Named, or every one installed from `--source`. In a chat, the change is proposed
    /// to the person instead.
    On {
        /// The skills' names, as `purlis skill list` prints them.
        #[arg(required_unless_present = "source", conflicts_with = "source")]
        names: Vec<String>,
        /// The workspace that turns them on.
        #[arg(short = 'w', long = "workspace", required = true)]
        workspace: String,
        #[command(flatten)]
        set: SetArgs,
    },
    /// Restore a deleted skill to the level it was deleted from; with no name, list the deleted
    /// skills that can be restored. Several names bring them all back as one change, after its
    /// review, asked at your terminal; anywhere else it is proposed to the person, in the window.
    Restore {
        /// The deleted skills' names.
        names: Vec<String>,
        /// Restore it under this new name instead: its `SKILL.md`'s `name:` and its folder are
        /// renamed to match. A skill whose name belongs to purlis now comes back only so.
        #[arg(long = "as", value_name = "NEW_NAME", requires = "names")]
        under: Option<String>,
        /// The level it was deleted from, where a name was deleted at more than one.
        #[command(flatten)]
        at: LevelArgs,
    },
    /// Take deleted skills off the list `purlis skill restore` prints, now, before their time is
    /// up: the one change to skills with no undo. Asked first at your terminal, even of one
    /// skill; anywhere else it is proposed to the person, in the window.
    DeleteNow {
        /// The deleted skills' names, as `purlis skill restore` lists them.
        #[arg(required = true)]
        names: Vec<String>,
        /// The level each was deleted from, where a name was deleted at more than one.
        #[command(flatten)]
        at: LevelArgs,
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
    /// that is not installed from it yet, through one review and one commit. With `--extension`,
    /// from the skill source an extension adds, pinned to the revision it names: asked at your own
    /// terminal each time, never with `--yes`, and the copy is what the review read (#5760).
    Install {
        /// The source: an https:// URL, or a folder on this machine by its full path. With
        /// `--extension`, there is none: every word is a skill's name.
        #[arg(required_unless_present = "extension")]
        url: Option<String>,
        /// Install from the skill source of this extension, by its id: one this machine approved,
        /// the project has on, and that declares `skill-source`.
        #[arg(
            long,
            value_name = "ID",
            conflicts_with_all = ["branch", "folder", "vault", "key"]
        )]
        extension: Option<String>,
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
        /// Install a skill whose name is taken under a new one: `<name>=<new name>`. Repeat it for
        /// another. Only its `name:` line and folder change.
        #[arg(long = "rename", value_name = "NAME=NEW")]
        renames: Vec<String>,
        /// Leave out a skill whose name is taken. Repeat it for another.
        #[arg(long = "skip", value_name = "NAME")]
        skips: Vec<String>,
        /// Install without asking: for a script that printed and read the review itself.
        #[arg(long)]
        yes: bool,
        #[command(flatten)]
        at: LevelArgs,
    },
    /// Write a skill, and the project's skills it refers to, to a new folder or zip that works
    /// outside purlis, each in a folder of its own. Links, credential files and what git ignores
    /// stay behind and are said; anything credential-shaped writes nothing. Several names, or
    /// `--source`, export them all, after its review, asked at your terminal; anywhere else the
    /// export is proposed to the person, who picks where it goes, in the window.
    Export {
        /// The skills' names, as `purlis skill list` prints them.
        #[arg(required_unless_present = "source", conflicts_with = "source")]
        names: Vec<String>,
        #[command(flatten)]
        set: SetArgs,
        /// Where: a path that is not there yet. A chat's export of several skills takes none:
        /// the person picks where.
        #[arg(long)]
        to: Option<std::path::PathBuf>,
        /// Write a zip there instead of a folder.
        #[arg(long)]
        zip: bool,
        #[command(flatten)]
        at: LevelArgs,
    },
    /// Open a draft pull or merge request on a skill source with a skill, the project's skills it
    /// refers to, and a summary of their grades: prints the review of everything that would go,
    /// and asks. Takes a yes from a person at a terminal, and has no flag to skip it: refused with
    /// no terminal and inside a chat. Anything secret or personal, or a skill kept on this
    /// machine, sends nothing.
    Publish {
        /// The skill's name, as `purlis skill list` prints it.
        name: String,
        /// The source: an https:// URL on GitHub or GitLab, which you can push to.
        #[arg(long)]
        to: String,
        /// The source's branch the request goes into (default: its own default branch).
        #[arg(long)]
        branch: Option<String>,
        /// The folder inside the source its skills are in (default: its top).
        #[arg(long)]
        folder: Option<String>,
        /// The vault a private source is read with, through `purlis secret exec`.
        #[arg(long, requires = "key")]
        vault: Option<String>,
        /// The entry in that vault that holds the token.
        #[arg(long, requires = "vault")]
        key: Option<String>,
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
    /// it is restored and in use again. Several names, or `--source`, as one change, after its
    /// review; in a chat, the change is proposed to the person instead.
    Archive {
        /// The skills' names, as `purlis skill list` prints them.
        #[arg(required_unless_present = "source", conflicts_with = "source")]
        names: Vec<String>,
        #[command(flatten)]
        set: SetArgs,
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
    /// `--run` from a terminal, on the same plan and caps, once you answer yes. Outcome evals run
    /// only where ticked there or named with `--eval`.
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
        /// An outcome eval to run too, by its id; repeat for each. None run unless named. A run
        /// that names one asks at a terminal, so `--yes` and a piped answer are refused.
        #[arg(long = "eval", value_name = "ID", requires = "run")]
        evals: Vec<String>,
        /// Do not ask: for a script that has read the plan. It still runs only what the plan
        /// printed, and it is refused beside `--eval`.
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

/// What a change to several skills at once takes (#5357), beside the names.
#[derive(Args)]
pub struct SetArgs {
    /// Every copy installed from this skill source, by its URL, in place of names.
    #[arg(long, value_name = "URL")]
    source: Option<String>,
    /// The source's branch, where it was installed from one.
    #[arg(long, requires = "source")]
    branch: Option<String>,
}

impl SetArgs {
    /// The source named, as its copies are kept by.
    fn remote(&self) -> Option<Remote> {
        self.source.as_ref().map(|url| Remote {
            url: url.clone(),
            branch: self.branch.clone(),
        })
    }
}

impl LevelArgs {
    /// Whether a level was named at all.
    fn named(&self) -> bool {
        self.workspace.is_some() || self.persona.is_some() || self.local
    }

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
const FROM_A_CHAT: &str = "A chat does not change skills itself, and this change is not one it \
                           can leave as a draft: nothing was written. A chat drafts a skill with \
                           `purlis skill edit <name> --from <file>` or `purlis skill new`, which \
                           the person accepts in the Skill studio. Ask the person for anything \
                           else.";

/// **Whether this command runs at the person's own terminal** (#5357), and not in a chat, read
/// the ways purlis already tells them apart, any one of which is a chat:
///
/// - what the app sets in the environment of everything it starts: the chat's number, its hook
///   socket, its session, the mark of its sandbox ([`gradelink::in_a_chat`]);
/// - the sandbox itself: this process may not write the project's manifest, which every
///   sandboxed chat is denied ([`purlis_core::sandbox::held::in_a_sandbox`]), whatever its
///   environment says;
/// - where it runs: below a chat's program, in its session, or below the app that started one,
///   by the records of open chats ([`purlis_core::secrets::setup::in_a_chat`]). A doubt there is
///   a chat's.
///
/// None of them is a flag: nothing a chat passes makes it the person.
pub(crate) fn at_the_persons_terminal(root: &Path) -> bool {
    use purlis_core::secrets::{Ctx, Env, setup};
    if gradelink::in_a_chat(&purlis_core::envvar::var)
        || purlis_core::sandbox::held::in_a_sandbox(root)
    {
        return false;
    }
    matches!(
        setup::in_a_chat(&Ctx::new(root, Env::from_process())),
        setup::Where::Outside
    )
}

/// Whether `command` is a verb a set has, asked of several skills or of a source's.
fn set_is_bulk(command: &SkillCommand) -> bool {
    match command {
        SkillCommand::Rm { names, set, .. }
        | SkillCommand::Off { names, set, .. }
        | SkillCommand::On { names, set, .. }
        | SkillCommand::Pin { names, set, .. }
        | SkillCommand::Archive { names, set, .. }
        | SkillCommand::Mv { names, set, .. }
        | SkillCommand::Export { names, set, .. } => names.len() > 1 || set.source.is_some(),
        SkillCommand::Restore { names, .. } => names.len() > 1,
        // There is no undo, so it is asked of the person even of one skill (#5492).
        SkillCommand::DeleteNow { .. } => true,
        _ => false,
    }
}

/// **Whether a person can answer here**: standard input and standard error are both a terminal.
/// A process detached from its chat has none, and cannot get one without making a terminal of its
/// own; with the other checks ([`at_the_persons_terminal`]) it is what a change to several skills
/// takes (#5357).
fn interactive() -> bool {
    use std::io::IsTerminal as _;
    std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

/// **The change to several skills `command` asks for** (#5357), as its review and its proposal
/// name it: `None` for a command that is no verb a set has, and why where its skills can't be
/// named.
fn set_change(root: &Path, command: &SkillCommand) -> Option<Result<SetChange, String>> {
    Some(match command {
        SkillCommand::Rm { names, set, at } => {
            members_of(root, names, set, at).map(|members| SetChange::Delete { members })
        }
        SkillCommand::Archive {
            names,
            set,
            undo,
            at,
        } => members_of(root, names, set, at).map(|members| SetChange::Archive {
            members,
            archive: !undo,
        }),
        SkillCommand::Off {
            names,
            workspace,
            set,
        }
        | SkillCommand::On {
            names,
            workspace,
            set,
        } => names_of(root, names, set, true).map(|names| SetChange::Turn {
            workspace: workspace.clone(),
            names,
            on: matches!(command, SkillCommand::On { .. }),
        }),
        SkillCommand::Pin {
            names,
            set,
            workspace,
            persona,
            off,
        } => {
            let by = match (workspace, persona) {
                (Some(ws), _) => purlis_core::skills::PinnedBy::Workspace(ws.clone()),
                (None, Some(persona)) => purlis_core::skills::PinnedBy::Persona(persona.clone()),
                (None, None) => return None,
            };
            names_of(root, names, set, false).map(|names| SetChange::Pin {
                by,
                names,
                on: !off,
            })
        }
        SkillCommand::Mv {
            names,
            set,
            new: None,
            to,
            to_persona,
            to_local,
            grant,
            at,
        } => {
            let dest = match (to, to_persona) {
                (_, Some(persona)) => Level::persona(persona).local(*to_local),
                (Some(to), None) => level_to(to, *to_local),
                (None, None) => return None,
            };
            members_of(root, names, set, at).map(|members| SetChange::Move {
                members,
                to: dest,
                grant: grant.clone(),
            })
        }
        SkillCommand::Restore {
            names,
            under: None,
            at,
        } if !names.is_empty() => {
            deleted_members(root, names, at).map(|members| SetChange::Restore { members })
        }
        SkillCommand::Export {
            names,
            set,
            zip,
            at,
            ..
        } => {
            members_of(root, names, set, at).map(|members| SetChange::Export { members, zip: *zip })
        }
        SkillCommand::DeleteNow { names, at } => {
            deleted_members(root, names, at).map(|members| SetChange::DeleteNow { members })
        }
        _ => return None,
    })
}

/// The skills named, at the level `at` names; or every copy installed from `--source`, at that
/// level where one is named, and at every level where none is.
fn members_of(
    root: &Path,
    names: &[String],
    set: &SetArgs,
    at: &LevelArgs,
) -> Result<Vec<set::Member>, String> {
    let Some(remote) = set.remote() else {
        return Ok(names
            .iter()
            .map(|name| set::Member {
                level: at.level(),
                name: name.clone(),
            })
            .collect());
    };
    let members: Vec<set::Member> =
        set::installed_from(root, &source::Origin::Git(remote.clone()), None)
            .into_iter()
            .map(|one| one.member)
            .filter(|member| !at.named() || member.level == at.level())
            .collect();
    if members.is_empty() {
        return Err(nothing_from(&remote));
    }
    Ok(members)
}

/// The names given; or those of every copy installed from `--source`, the project's alone where
/// `project` (a turn: only a project skill turns in a workspace).
fn names_of(
    root: &Path,
    names: &[String],
    set: &SetArgs,
    project: bool,
) -> Result<Vec<String>, String> {
    let Some(remote) = set.remote() else {
        return Ok(names.to_vec());
    };
    let mut found: Vec<String> = Vec::new();
    for one in set::installed_from(root, &source::Origin::Git(remote.clone()), None) {
        let at_the_projects_level = one.member.level == Level::project();
        if (!project || at_the_projects_level) && !found.contains(&one.member.name) {
            found.push(one.member.name);
        }
    }
    if found.is_empty() {
        return Err(nothing_from(&remote));
    }
    Ok(found)
}

/// **The deleted skills `names` names**, each at the level it was deleted from: the one the
/// plane lists it at, or `at` where it lists none (the review then says it is not in Deleted),
/// and refused where it lists more than one and `at` names none of them.
fn deleted_members(
    root: &Path,
    names: &[String],
    at: &LevelArgs,
) -> Result<Vec<set::Member>, String> {
    let listed = verbs::deleted(root);
    names
        .iter()
        .map(|name| {
            let mut levels: Vec<Level> = Vec::new();
            for one in &listed {
                if one.name == *name
                    && (!at.named() || one.level == at.level())
                    && !levels.contains(&one.level)
                {
                    levels.push(one.level.clone());
                }
            }
            let level = match levels.len() {
                0 => at.level(),
                1 => levels.remove(0),
                _ => {
                    return Err(format!(
                        "{name} was deleted at more than one level, so nothing was changed: name \
                         the one with -w, -p or --local"
                    ));
                }
            };
            Ok(set::Member {
                level,
                name: name.clone(),
            })
        })
        .collect()
}

/// **Why a change to several skills can't take what else was asked**: a rename, or a restore
/// under a new name, is of one skill.
fn bulk_conflict(command: &SkillCommand) -> Option<&'static str> {
    match command {
        SkillCommand::Mv { new: Some(_), .. } => Some(
            "--name renames one skill, so nothing was changed: rename each on its own, or move \
             them without --name",
        ),
        SkillCommand::Restore { under: Some(_), .. } => Some(
            "--as brings back one skill under a new name, so nothing was changed: bring that one \
             back on its own",
        ),
        _ => None,
    }
}

/// Why `--source` named no skill.
fn nothing_from(remote: &Remote) -> String {
    format!(
        "No skill installed from {} is here, so nothing was changed. `purlis skill list` shows \
         where each came from.",
        remote.url
    )
}

/// `purlis skill off` or `on` with one name: as `grant -w <ws> --off` or `--on` turns it.
fn switched(
    cx: &mut verbs::Ctx<'_>,
    workspace: &str,
    name: &str,
    on: bool,
) -> Result<Code, CliError> {
    match crate::brokered::manifest_refused("skill off/on")
        .map_or_else(|| verbs::switch(cx, workspace, name, on), Err)
    {
        Ok(changed) => {
            if !changed {
                let state = if on { "on" } else { "off" };
                voice::ok(already_switched(name, state, workspace));
            }
            Ok(0)
        }
        Err(why) => {
            voice::err(why.to_string());
            Ok(1)
        }
    }
}

/// The one name a verb's single path takes: clap asks for one at least.
fn one_of(names: Vec<String>) -> Result<String, CliError> {
    names
        .into_iter()
        .next()
        .ok_or_else(|| CliError::internal("no skill was named, where clap asks for one at least"))
}

/// **A chat's change to skills, proposed to the person** (#5357): nothing is written here. The app
/// holds it with the other proposed changes, and only the person's Apply in the window makes it.
fn proposed_from_a_chat(change: SetChange) -> Result<Code, CliError> {
    if let Err(why) = change.checked() {
        voice::err(why);
        return Ok(1);
    }
    Ok(crate::brokered::propose_skill_set(change)?)
}

/// **The template `skill new` names**, and `with` handed it and the templates to make it from:
/// purlis's own, and each extension's that declares `skill-template`, asked through the
/// executor's gate as the studio asks them (ADR 0101).
fn templated<T>(
    root: &Path,
    template: TemplateArg,
    extension_template: &Option<Vec<String>>,
    with: impl FnOnce(&purlis_core::skills::studio::Templates<'_>, &TemplateId) -> T,
) -> T {
    let template = match extension_template.as_deref() {
        Some([extension, template]) => TemplateId::Extension {
            extension: extension.clone(),
            template: template.clone(),
        },
        _ => TemplateId::BuiltIn(template.into()),
    };
    let executor = Executor::with_built_in(purlis_core::extension::BuiltIn::of_this_program());
    purlis_core::skills::studio::with_studio(&executor, root, |studio| {
        with(
            &purlis_core::skills::studio::Templates { studio },
            &template,
        )
    })
}

/// **What the studio's lints say of `skills`, said in their review's lint lines** (#3090): as
/// the window's review asks them ([`purlis_core::skills::studio::lint_reviews`]).
fn linted<'s>(
    root: &Path,
    level: &Level,
    skills: impl IntoIterator<Item = &'s mut source::SkillReview>,
) {
    let executor = Executor::with_built_in(purlis_core::extension::BuiltIn::of_this_program());
    purlis_core::skills::studio::lint_reviews(&executor, root, level, skills);
}

/// **A chat's edit or new skill, left as a draft** (#5815; spec #2762 decisions 26 and 38):
/// `purlis skill edit <name> --from <file>` and `purlis skill new`, checked as at the person's
/// terminal, then handed to the app, which keeps the new `SKILL.md` in the Skill studio's Drafts
/// for the person to review as a diff and accept or decline. Nothing in the skill's folder is
/// written. `None` for any other command, which a chat is refused.
fn drafted_from_a_chat(
    root: &Path,
    built_in: Option<&Path>,
    command: &SkillCommand,
) -> Option<Result<Code, CliError>> {
    let refused = |why: String| {
        voice::err(esc(&why).to_string());
        Ok(1)
    };
    let waits = |kept: Result<crate::brokered::Kept, String>, what: String, folder: &str| match kept
    {
        Ok(crate::brokered::Kept::Drafted) => {
            voice::ok(
                esc(&format!(
                    "A draft of {what} waits for the person in the Skill studio, under Drafts, \
                     to review as a diff and accept or decline. Nothing in {folder} was written."
                ))
                .to_string(),
            );
            Ok(0)
        }
        Ok(crate::brokered::Kept::Said(code)) => Ok(code),
        Err(why) => refused(why),
    };
    Some(match command {
        SkillCommand::Edit {
            name,
            from: Some(from),
            at,
        } => {
            let level = at.level();
            let checked = verbs::text_of(root, &level, name)
                .map_err(CliError::from)
                .and_then(|(_, was)| {
                    let now = new_text(Some(from), &was)?;
                    let (folder, changed) = verbs::edit_checked(root, &level, name, &was, &now)?;
                    let folder = folder
                        .strip_prefix(root)
                        .map_err(|_| CliError::from(format!("{name} is not in this project")))?
                        .to_path_buf();
                    Ok((folder, changed.then_some(now)))
                });
            match checked {
                Err(why) => match why.refused() {
                    Ok(why) => refused(why),
                    Err(internal) => Err(internal),
                },
                Ok((_, None)) => {
                    voice::info(esc(&verbs::unchanged(name)).to_string());
                    Ok(0)
                }
                Ok((folder, Some(now))) => waits(
                    crate::brokered::draft_skill_file(root, &folder.join("SKILL.md"), now),
                    format!("{name}'s instructions"),
                    &folder.display().to_string(),
                ),
            }
        }
        SkillCommand::New {
            name,
            description,
            template,
            extension_template,
            at,
        } => {
            let level = at.level();
            let mut said = |line: purlis_core::repocmd::Say| crate::speak(line);
            let cx = verbs::Ctx {
                plane: root,
                built_in,
                by: By::Command,
                say: &mut said,
                audit: None,
            };
            let checked = templated(
                root,
                *template,
                extension_template,
                |templates, template| {
                    verbs::new_checked(&cx, &level, name, description, templates, template)
                },
            );
            match checked {
                Ok((folder, text)) => waits(
                    crate::brokered::draft_skill_file(
                        root,
                        &Path::new(&folder).join("SKILL.md"),
                        text,
                    ),
                    format!("{name}, a new {level} skill,"),
                    &folder,
                ),
                Err(why) => refused(why),
            }
        }
        _ => return None,
    })
}

/// **The person's change to several skills**: its review printed, asked once at their terminal,
/// then made as one commit. No flag skips the question (#5357).
fn made_here(
    cx: &mut verbs::Ctx<'_>,
    change: &SetChange,
    to: Option<&Path>,
) -> Result<Code, CliError> {
    if let Err(why) = change.checked() {
        voice::err(why);
        return Ok(1);
    }
    if matches!(change, SetChange::Export { .. }) && to.is_none() {
        voice::err(SAY_WHERE);
        return Ok(2);
    }
    let shown = change.shown(cx.plane);
    println!("{}:", change.title());
    for line in shown.after.lines() {
        println!("  {}", esc(line));
    }
    if !asked("Make this change?") {
        voice::info("Nothing was changed.");
        return Ok(1);
    }
    match to {
        Some(to) => match change.export_to(cx.plane, cx.built_in, to) {
            Ok(()) => {
                voice::info(esc(&format!("Exported to {}.", to.display())).to_string());
                Ok(0)
            }
            Err(why) => {
                voice::err(esc(&why).to_string());
                Ok(1)
            }
        },
        None => done(change.apply(cx)),
    }
}

/// What an export with no place is told.
const SAY_WHERE: &str = "Say where with --to <a path that is not there yet>.";

/// **What an export of several skills with `--to` is told anywhere but the person's interactive
/// terminal** (#5492): the person picks where it goes, never a chat or a script.
const NO_PLACE_BUT_AT_THE_TERMINAL: &str = "An export of several skills goes only where the person \
                                    picks: --to is taken only at your own terminal, after its \
                                    question, so nothing was written. Run it there, or without \
                                    --to to propose it to the person, who picks where in the \
                                    window.";

pub fn run(root: &Path, command: SkillCommand) -> Result<Code, CliError> {
    let built_in = purlis_core::skills::built_in_here();
    let reads_owners =
        matches!(&command, SkillCommand::Owners { owners, none: false, .. } if owners.is_empty());
    let lists_deleted = matches!(&command, SkillCommand::Restore { names, .. } if names.is_empty());
    let writes = !reads_owners
        && !lists_deleted
        && !matches!(
            command,
            SkillCommand::List { .. }
                | SkillCommand::Show { .. }
                | SkillCommand::Update {
                    name: None,
                    confirm: None,
                    ..
                }
                | SkillCommand::Runs { .. }
                | SkillCommand::Grade { .. }
                | SkillCommand::History { .. }
                | SkillCommand::Uses { .. }
                | SkillCommand::Export { .. }
        );
    // A chat never changes a skill itself (#5357): a change to several skills, or to one, by the
    // verbs a set has, is proposed to the person; any other change is refused. A change to several
    // is made here only at the person's interactive terminal (the operator's ruling of
    // 2026-11-06): a caller with no terminal, a detached one included, proposes too.
    let bulk = set_is_bulk(&command);
    if bulk && let Some(why) = bulk_conflict(&command) {
        voice::err(why);
        return Ok(2);
    }
    // An export of one skill writes only where its caller can (it changes no skill); one of
    // several is the person's alone, as a change is (#5492).
    let exports_several = bulk && matches!(command, SkillCommand::Export { .. });
    let gated = writes || exports_several;
    if gated && !(at_the_persons_terminal(root) && (!bulk || interactive())) {
        if exports_several && matches!(command, SkillCommand::Export { to: Some(_), .. }) {
            voice::err(NO_PLACE_BUT_AT_THE_TERMINAL);
            return Ok(1);
        }
        return match set_change(root, &command) {
            Some(Ok(change)) => proposed_from_a_chat(change),
            Some(Err(why)) => {
                voice::err(esc(&why).to_string());
                Ok(1)
            }
            None if let Some(drafted) =
                drafted_from_a_chat(root, built_in.as_deref(), &command) =>
            {
                drafted
            }
            None => {
                // A turn in a workspace rewrites its manifest, which a sandboxed chat is told it
                // does not write (#2046): that sentence, where it is the one that holds.
                let manifest = matches!(
                    &command,
                    SkillCommand::Grant {
                        workspace: Some(_),
                        ..
                    }
                )
                .then(|| crate::brokered::manifest_refused("skill grant --on/--off"))
                .flatten();
                voice::err(manifest.unwrap_or_else(|| FROM_A_CHAT.to_owned()));
                Ok(1)
            }
        };
    }
    // The person's terminal: several skills, or a source's, are one change after its review.
    if gated
        && bulk
        && let Some(change) = set_change(root, &command)
    {
        let mut said = |line: purlis_core::repocmd::Say| crate::speak(line);
        // An org's project audits the change through the session host (SK-11a, #6030).
        let mut audit = crate::skillaudit::hook(root);
        let mut cx = verbs::Ctx {
            plane: root,
            built_in: built_in.as_deref(),
            by: By::Command,
            say: &mut said,
            audit: Some(&mut audit),
        };
        // An export changes no skill, so it finishes and expires nothing either.
        if writes {
            verbs::interrupted(&mut cx);
            // Not before a change to skills in Deleted, which the list showed as it was.
            if !matches!(
                command,
                SkillCommand::Restore { .. } | SkillCommand::DeleteNow { .. }
            ) {
                verbs::expire_now(root);
            }
        }
        return match change {
            Ok(change) => {
                let to = match &command {
                    SkillCommand::Export { to, .. } => to.as_deref(),
                    _ => None,
                };
                made_here(&mut cx, &change, to)
            }
            Err(why) => {
                voice::err(esc(&why).to_string());
                Ok(1)
            }
        };
    }
    let mut said = |line: purlis_core::repocmd::Say| crate::speak(line);
    // An org's project audits the change through the session host (SK-11a, #6030).
    let mut audit = crate::skillaudit::hook(root);
    let mut cx = verbs::Ctx {
        plane: root,
        built_in: built_in.as_deref(),
        by: By::Command,
        say: &mut said,
        audit: Some(&mut audit),
    };
    // A delete a crash or a quit cut short is finished or undone before any change (SK-35).
    // And the skills this machine kept deleted past their time are removed (SK-33), but before a
    // restore, which `purlis skill restore`'s list offered the person as they saw it.
    if writes {
        verbs::interrupted(&mut cx);
        if !matches!(
            command,
            SkillCommand::Restore { .. } | SkillCommand::DeleteNow { .. }
        ) {
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
                            // Each it has the same copy of, left out naming it (#5843): no
                            // repo's copy clashes with these, so no clashes beside them.
                            "same_left_out": one.same_left_out.iter().map(row_json)
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
                    // What it has the same copy of, listed once (#5843).
                    if !one.same_left_out.is_empty() {
                        print!(
                            "\n{} has an identical copy of these itself, so its chats start \
                             without them:\n{}",
                            one.harness.title(),
                            table(&one.same_left_out, &|_| Vec::new())
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
                                .chain(&one.same_left_out)
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
        } => done(templated(
            root,
            template,
            &extension_template,
            |templates, template| {
                verbs::new_skill(
                    &mut cx,
                    &at.level(),
                    &name,
                    &description,
                    templates,
                    template,
                )
            },
        )),
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
                    voice::err(why.refused()?);
                    return Ok(1);
                }
            };
            done(verbs::edit(&mut cx, &level, &name, &was, &now))
        }
        SkillCommand::Mv {
            names,
            new,
            to,
            to_persona,
            to_local,
            grant,
            at,
            ..
        } => {
            if new.is_none() && to.is_none() && to_persona.is_none() {
                voice::err(
                    "Say what changes: --name <new> renames it, --to <root|workspace> or \
                     --to-persona <persona> moves it.",
                );
                return Ok(2);
            }
            let level = at.level();
            let mut name = one_of(names)?;
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
        SkillCommand::Rm { names, at, .. } => {
            let name = one_of(names)?;
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
        SkillCommand::Restore { names, .. } if names.is_empty() => {
            let deleted = verbs::deleted(root);
            if deleted.is_empty() {
                println!("No deleted skill can be restored here.");
            }
            let now = std::time::SystemTime::now();
            for one in deleted {
                let line = restorable(&one.name, &one.level, &one.stays(now));
                // Its name is purlis's own now (#3729): it comes back under a new one.
                let line = if purlis_core::skills::name_reserved(&one.name) {
                    line.own(" · its name belongs to purlis now: restore it with --as <new-name>")
                } else {
                    line
                };
                println!("{line}");
            }
            Ok(0)
        }
        SkillCommand::Restore { names, under, at } => {
            let name = one_of(names)?;
            let Some(one) = verbs::deleted(root)
                .into_iter()
                .find(|one| one.name == name && (!at.named() || one.level == at.level()))
            else {
                voice::err(format!(
                    "No deleted skill named {name:?} can be restored: `purlis skill restore` lists those that can."
                ));
                return Ok(2);
            };
            done(verbs::restore(&mut cx, &one, under.as_deref()))
        }
        // Made above, after its question at the person's terminal, or proposed: never here. Were
        // it reached, it would delete nothing.
        SkillCommand::DeleteNow { .. } => Err(
            "purlis did not ask you first, so nothing was deleted. Run `purlis skill delete-now` \
             again at your terminal."
                .to_owned()
                .into(),
        ),
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
            names,
            workspace,
            persona,
            off,
            ..
        } => {
            let name = one_of(names)?;
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
            extension,
            names,
            all,
            branch,
            folder,
            vault,
            key,
            grant,
            renames,
            skips,
            yes,
            at,
        } => {
            // With `--extension` there is no source to name, so the first word is a skill's.
            let names: Vec<String> = match &extension {
                Some(_) => url.iter().cloned().chain(names).collect(),
                None => names,
            };
            if all && !names.is_empty() {
                voice::err("--all takes no skill's name: name skills, or use --all.");
                return Ok(2);
            }
            let named = if all {
                Named::All
            } else {
                Named::These(&names)
            };
            let settled = Settled { renames, skips };
            if let Some(id) = extension {
                // Refused here, after a chat's refusal, so a chat is told what a git install's is.
                if yes {
                    voice::err(
                        "An install from an extension's source is asked of you each time, so \
                         --yes is not taken and nothing was installed.",
                    );
                    return Ok(2);
                }
                return install_from_an_extension(
                    &mut cx,
                    &id,
                    named,
                    &grant,
                    &settled,
                    &at.level(),
                );
            }
            let Some(url) = url else {
                voice::err("Name the source: a URL, a folder's full path, or --extension <id>.");
                return Ok(2);
            };
            let source = Source {
                remote: Remote { url, branch },
                folder,
                sign_in: vault.zip(key).map(|(vault, key)| VaultEntry {
                    vault,
                    key,
                    user: None,
                }),
            };
            install(&mut cx, &source, named, &grant, &settled, yes, &at.level())
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
            // An extension's source is asked its newest revision through the executor's gate, as
            // the studio asks it (ADR 0101).
            let executor =
                Executor::with_built_in(purlis_core::extension::BuiltIn::of_this_program());
            let found = purlis_core::skills::studio::with_studio(&executor, root, |studio| {
                source::updates(root, studio)
            });
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
        SkillCommand::Export {
            names, to, zip, at, ..
        } => {
            let Some(to) = to else {
                voice::err(SAY_WHERE);
                return Ok(2);
            };
            export(
                root,
                built_in.as_deref(),
                &one_of(names)?,
                &to,
                zip,
                &at.level(),
            )
        }
        SkillCommand::Publish {
            name,
            to,
            branch,
            folder,
            vault,
            key,
            at,
        } => {
            let source = Source {
                remote: Remote { url: to, branch },
                folder,
                sign_in: vault.zip(key).map(|(vault, key)| VaultEntry {
                    vault,
                    key,
                    user: None,
                }),
            };
            publish(root, built_in.as_deref(), &name, &source, &at.level())
        }
        SkillCommand::Update {
            name: Some(name),
            yes,
            at,
            ..
        } => {
            let level = at.level();
            if from_an_extension(root, &level, &name) {
                return update_from_an_extension(&mut cx, &level, &name, yes);
            }
            let mut update = match source::update_review(root, &level, &name) {
                Ok(update) => update,
                Err(why) => {
                    voice::err(esc(&why).to_string());
                    return Ok(1);
                }
            };
            linted(root, &level, update.skills_mut());
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
        SkillCommand::Off {
            names, workspace, ..
        } => switched(&mut cx, &workspace, &one_of(names)?, false),
        SkillCommand::On {
            names, workspace, ..
        } => switched(&mut cx, &workspace, &one_of(names)?, true),
        SkillCommand::Archive {
            names, undo, at, ..
        } => {
            let name = one_of(names)?;
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
                    json!({
                        "uses": node.uses,
                        "used_by": node.used_by,
                        "hinted_by": node.hinted_by,
                        "cut": node.cut
                    })
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
        } else if one.hint {
            line.own("  (a hint: named bare, not as skills:")
                .text(&one.name)
                .own(")")
        } else {
            line
        });
    }
    if node.used_by.is_empty() {
        let line = Markup::new()
            .own("No skill of the project names ")
            .text(name);
        lines.push(if node.hinted_by.is_empty() {
            line.own(".")
        } else {
            line.own(" as skills:").text(name).own(".")
        });
    }
    for other in &node.used_by {
        lines.push(Markup::new().own("used by ").text(other));
    }
    // A skill whose text names this one only bare (#4510).
    for other in &node.hinted_by {
        lines.push(
            Markup::new()
                .own("hinted at by ")
                .text(other)
                .own("  (named bare)"),
        );
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
) -> Result<Code, CliError> {
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
fn estimate_said(
    estimate: Option<&purlis_core::skills::grades::Estimate>,
    outcome_evals: u32,
) -> String {
    let Some(estimate) = estimate else {
        // An outcome eval's estimate is only ever a grade that ran some (#4235): a skill graded
        // on its trigger tests alone has still been graded.
        if outcome_evals > 0 {
            return "Its outcome evals have not been graded here before, so what a grade spends \
                    is not known yet: an outcome eval's chat runs its whole turn, and a judge \
                    chat reads what it left.\n"
                .to_owned();
        }
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
    grade_plan_said(planned, local_file, false)
}

/// **`purlis skill grade`'s text**: the plan, cost first, then each grade, newest first.
fn grade_said(
    planned: &purlis_core::skills::grades::Planned,
    graded: &[purlis_core::skills::grades::Graded],
    local_file: &str,
) -> String {
    use purlis_core::skills::grade::Arm;
    let name = &planned.skill;
    let mut text = grade_plan_said(planned, local_file, true);
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
            // An outcome eval's ask fails its run, as a trigger test's does not count (SK-55).
            asked if !one.report.outcomes.is_empty() => format!(
                "  {} asked you something, and purlis ended {them} there: a trigger test's is not \
                 counted, and an outcome eval's fails that run",
                counted(asked, "chat", "chats"),
                them = if asked == 1 { "it" } else { "them" }
            ),
            asked => format!(
                "  {} asked you something, and purlis ended {them} there without counting {them}",
                counted(asked, "chat", "chats"),
                them = if asked == 1 { "it" } else { "them" }
            ),
        };
        // The version it compared with, where it compared one (SK-57).
        let previous = one.compared_with().map(history::short);
        // Its outcome evals, where it ran any (SK-55): each arm's share of expectations passed.
        let outcome_rate = |arm: Arm| {
            one.report
                .arm(arm)
                .and_then(|score| score.outcome_pass_rate)
                .map_or_else(
                    || "not judged".to_owned(),
                    |stat| format!("{:.0}%", stat.mean * 100.0),
                )
        };
        let outcomes = if one.report.outcomes.is_empty() {
            String::new()
        } else {
            format!(
                "  outcome evals with {} without {}{}{}",
                outcome_rate(Arm::With),
                outcome_rate(Arm::Without),
                previous
                    .map(|_| format!(" previous {}", outcome_rate(Arm::Previous)))
                    .unwrap_or_default(),
                one.report
                    .outcome_delta
                    .map(|delta| format!(" difference {:+.0} points", delta * 100.0))
                    .unwrap_or_default(),
            )
        };
        // Its trigger tests, where it ran any: a grade of outcome evals alone says nothing of them.
        let tests = if one.report.triggers.is_empty() {
            String::new()
        } else {
            format!(
                "  {passed} of {} passed  with {}  without {}{}",
                counted(of, "test", "tests"),
                rate(Arm::With),
                rate(Arm::Without),
                previous
                    .map(|commit| format!("  previous {commit} {}", rate(Arm::Previous)))
                    .unwrap_or_default(),
            )
        };
        text.push_str(&format!(
            "  {}  {}{tests}{}{outcomes}{asked}  ({}){}\n",
            purlis_core::skills::review::escaped_line(&one.ts),
            one.commit.as_deref().map_or("uncommitted", history::short),
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
        // Why a run of an outcome eval failed (SK-55).
        for eval in &one.report.outcomes {
            for (run, why) in eval
                .results
                .iter()
                .zip(1u32..)
                .filter_map(|(one, run)| one.failed.as_deref().map(|why| (run, why)))
            {
                text.push_str(&format!(
                    "    Outcome eval {}, {}, run {run}: {}\n",
                    eval.eval,
                    eval.arm.word(),
                    purlis_core::skills::review::escaped_line(why),
                ));
            }
        }
    }
    text
}

/// The plan's part of [`grade_said`]. `with_limits` says the limits as this terminal can work them
/// out, and where to grade from: `--run` prints the window's own instead, the ones that bind.
fn grade_plan_said(
    planned: &purlis_core::skills::grades::Planned,
    local_file: &str,
    with_limits: bool,
) -> String {
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
            // Its trigger tests and its outcome evals (SK-55), each counted.
            let fixtures = match (plan.trigger_tests, plan.outcome_evals) {
                (tests, 0) => counted(tests, "trigger test", "trigger tests"),
                (0, evals) => counted(evals, "outcome eval", "outcome evals"),
                (tests, evals) => format!(
                    "{} and {}",
                    counted(tests, "trigger test", "trigger tests"),
                    counted(evals, "outcome eval", "outcome evals")
                ),
            };
            let judges = match plan.judge_chats {
                0 => String::new(),
                judges => format!(
                    "; and a judge chat for each outcome eval's chat, {} in all, which reads what \
                     that chat left and changes nothing",
                    counted(judges, "judge chat", "judge chats")
                ),
            };
            text.push_str(&format!(
                "Grading {name} runs {}, one turn each: each of {fixtures} {times} with the skill \
                 and {times} without it{previous}{judges}.\n",
                counted(plan.chats, "chat", "chats"),
            ));
            if let Some(why) = &planned.previous_refused {
                text.push_str(&format!("Its previous version is not compared: {why}.\n"));
            }
            text.push_str(&estimate_said(
                planned.estimate.as_ref(),
                plan.outcome_evals,
            ));
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
            if with_limits {
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
                text.push_str(
                    "Grade it from its tab in the Skill studio, which shows this first.\n",
                );
            }
        }
        _ => text.push_str(&format!(
            "{name} has no trigger tests or ticked outcome evals to grade: the queries it should \
             start on and the near misses it should leave alone, in its evals/triggers.json, and \
             tasks with what a judge checks, in its evals/evals.json.\n"
        )),
    }
    if let Read::Refused(why) = &planned.fixtures.outcomes {
        text.push_str(&format!(
            "Its outcome evals could not be read: {}\n",
            why.terminal()
        ));
    }
    // Off until the person ticks them in the Skill studio's plan or names them with --eval (the
    // ruling of 2026-10-31), so one that runs says so: it is what the yes covers (#4312).
    for eval in &planned.offered {
        let runs = planned
            .request
            .as_ref()
            .is_some_and(|request| request.outcomes.iter().any(|one| one.id == eval.id));
        let chats = counted(planned.eval_chats, "chat", "chats");
        let state = if runs {
            format!("on: {chats} of those above")
        } else {
            format!("off: {chats} more if ticked in the Skill studio's plan or named with --eval")
        };
        // What the Skill studio's plan says of it too: a yes here covers as much as a press does.
        let files = if eval.files.is_empty() {
            String::new()
        } else {
            format!(
                ", with {}",
                purlis_core::skills::review::escaped_line(&eval.files.join(", "))
            )
        };
        let hides = if planned.eval_hides(eval) {
            ", holds a character no reader would see"
        } else {
            ""
        };
        // #4922: a file purlis could not check, in the words of the Skill studio's plan.
        let not_text = match planned.eval_not_text(eval).as_slice() {
            [] => String::new(),
            [one] => format!(
                "; {} isn't text: purlis couldn't check it for characters no reader would see, \
                 and a judge may not be able to read it",
                purlis_core::skills::review::escaped_line(one)
            ),
            many => format!(
                "; {} aren't text: purlis couldn't check them for characters no reader would \
                 see, and a judge may not be able to read them",
                purlis_core::skills::review::escaped_line(&many.join(", "))
            ),
        };
        // #4476: its chats' shell, in the words of the Skill studio's plan.
        let shell = if planned.shell {
            ", uses a shell, which writes only in its own folders and reaches no network"
        } else {
            ""
        };
        text.push_str(&format!(
            "  Outcome eval {}. {}  ({state}; {} a judge checks{files}{shell}{hides}{not_text})\n",
            eval.id,
            purlis_core::skills::review::escaped_line(&eval.prompt),
            counted(eval.expectations.len(), "expectation", "expectations"),
        ));
    }
    for left in &planned.outcomes_left_out {
        text.push_str(&format!(
            "Outcome eval {} is left out: {}.\n",
            left.id, left.why
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

/// How `purlis skill install` was told to settle a name clash (#4530).
struct Settled {
    renames: Vec<String>,
    skips: Vec<String>,
}

impl Settled {
    /// `names` but those skipped.
    fn without_skipped(&self, names: &[String]) -> Vec<String> {
        names
            .iter()
            .filter(|name| !self.skips.contains(name))
            .cloned()
            .collect()
    }

    /// The new names, by each skill's name at the source, or what is wrong with one.
    fn new_names(&self) -> Result<source::Renames, String> {
        self.renames
            .iter()
            .map(|one| {
                one.split_once('=')
                    .map(|(old, new)| (old.to_owned(), new.to_owned()))
                    .ok_or_else(|| format!("--rename takes <name>=<new name>, not {one:?}"))
            })
            .collect()
    }
}

/// **What the review says of each name still taken**, or nothing where `new_names` and the skipped
/// settle them all: each with its two ways on, so a script or a person can act on it.
fn unsettled(skills: &[source::SkillReview], new_names: &source::Renames) -> Vec<String> {
    skills
        .iter()
        .filter(|skill| !new_names.contains_key(&skill.name))
        .filter_map(|skill| {
            skill.clash.as_ref().map(|clash| {
                format!(
                    "{}'s name is taken: {}. Use --skip {} or --rename {}={}.",
                    skill.name, clash.held_by, skill.name, skill.name, clash.offered
                )
            })
        })
        .collect()
}

/// `purlis skill install`: what `source` offers where no skill is named; else the review of
/// `names` (or of every skill not installed from it yet), asked about, then the install at
/// `level`.
fn install(
    cx: &mut verbs::Ctx<'_>,
    source: &Source,
    named: Named<'_>,
    grant: &[String],
    settled: &Settled,
    yes: bool,
    level: &Level,
) -> Result<Code, CliError> {
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
    let (grant, new_names) = match install_settled(grant, settled) {
        Ok(settled) => settled,
        Err(code) => return Ok(code),
    };
    let without_skipped = |names: &[String]| settled.without_skipped(names);
    let review = match named {
        Named::These(names) => source::review(root, source, &without_skipped(names), level),
        Named::All => match source::review_all(root, source, level) {
            Ok(source::SetReview::Review { review, .. }) => {
                if review.skills.is_empty() {
                    voice::info(format!(
                        "Every skill {} offers is installed already.",
                        esc(&source.to_string())
                    ));
                    return Ok(0);
                }
                if settled.skips.is_empty() {
                    Ok(review)
                } else {
                    // Reviewed again without them. All of these were named: one marked as brought
                    // along by a skipped skill is still one `--all` offers.
                    let named: Vec<String> = review
                        .skills
                        .iter()
                        .map(|skill| skill.name.clone())
                        .collect();
                    source::review(root, source, &without_skipped(&named), level)
                }
            }
            Ok(source::SetReview::TooMuch(too_much)) => {
                voice::err(esc(&too_much_text(&too_much)).to_string());
                return Ok(1);
            }
            Err(why) => Err(why),
        },
    };
    let mut review = match review {
        Ok(review) => review,
        Err(why) => {
            voice::err(esc(&why).to_string());
            return Ok(1);
        }
    };
    linted(root, level, &mut review.skills);
    let shown = Shown {
        from: &source.to_string(),
        at: review.commit.short(),
        pinned_to: review.commit.short(),
    };
    if let Some(code) = reviewed_and_asked(
        root,
        level,
        grant.as_ref(),
        &shown,
        &review.skills,
        &new_names,
        yes,
    ) {
        return Ok(code);
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
    done(source::install_named(
        cx,
        source,
        level,
        &review.accepted(),
        grant.as_ref(),
        &new_names,
    ))
}

/// What an install from an extension's source is told where no person can answer its question
/// (#5760).
const NO_ONE_TO_ASK: &str = "An install from an extension's source is asked of you at your own \
                             terminal each time, and nothing here can answer it, so nothing was \
                             installed. Run it in a terminal, or install it from the extension's \
                             source in the Skill studio's Sources.";

/// **`purlis skill install --extension <id>`** (#5760): what the extension's source offers where
/// no skill is named, each with what its files do; else the review of `names` (or of every skill
/// not installed from it yet), printed and asked at the person's interactive terminal, then the
/// install of exactly the bytes that review read, pinned to its revision (ADR 0100, amended
/// 2026-11-08). The extension is asked through the executor's gate, as the studio asks it (ADR
/// 0101), and not again after the review. There is no `--yes`: an install from an extension is
/// the person's each time, as the window's is. A chat never gets here ([`run`] refuses it).
fn install_from_an_extension(
    cx: &mut verbs::Ctx<'_>,
    id: &str,
    named: Named<'_>,
    grant: &[String],
    settled: &Settled,
    level: &Level,
) -> Result<Code, CliError> {
    let root = cx.plane;
    let listing = matches!(named, Named::These([]));
    if !listing && !interactive() {
        voice::err(NO_ONE_TO_ASK);
        return Ok(1);
    }
    let (grant, new_names) = match install_settled(grant, settled) {
        Ok(settled) => settled,
        Err(code) => return Ok(code),
    };
    let executor = Executor::with_built_in(purlis_core::extension::BuiltIn::of_this_program());
    purlis_core::skills::studio::with_studio(&executor, root, |studio| {
        let Some(studio) = studio else {
            voice::err(NO_EXTENSIONS_HERE);
            return Ok(1);
        };
        // Only an extension this machine approved, the project has on, and that declares
        // `skill-source` is a source: asked nothing otherwise.
        if !studio
            .declaring(purlis_core::extension::capability::Capability::SkillSource)
            .iter()
            .any(|one| one.id == id)
        {
            voice::err(format!(
                "{} is no skill source here: an extension is one once this machine approved it, \
                 the project has it on, and it declares skill-source. Check it in the window's \
                 Extensions.",
                esc(id)
            ));
            return Ok(1);
        }
        let read = match named {
            Named::These([]) => {
                return match source::extension::browse(studio, root, id) {
                    Ok(browse) => {
                        print!("{}", browse_text(&browse));
                        Ok(0)
                    }
                    Err(why) => {
                        voice::err(esc(&why).to_string());
                        Ok(1)
                    }
                };
            }
            Named::These(names) => source::extension::review(
                studio,
                root,
                id,
                &settled.without_skipped(names),
                level,
                &[],
            )
            .map(Some),
            // One listing names them and is reviewed: one revision, and no file asked twice.
            Named::All => source::extension::review_every(studio, root, id, &settled.skips, level)
                .map(|(set, held)| (!set.review.skills.is_empty()).then_some((set.review, held))),
        };
        let (mut review, held) = match read {
            Ok(Some(read)) => read,
            Ok(None) => {
                voice::info(format!(
                    "Every skill the extension {} offers is installed already.",
                    esc(id)
                ));
                return Ok(0);
            }
            Err(why) => {
                voice::err(esc(&why).to_string());
                return Ok(1);
            }
        };
        studio.lint_reviewed(&mut review.skills, level);
        let pinned_to = format!("revision {}", review.revision.as_str());
        let shown = Shown {
            from: &format!("the extension {id}"),
            at: review.revision.as_str(),
            pinned_to: &pinned_to,
        };
        // Never `yes`: the person answers it each time.
        if let Some(code) = reviewed_and_asked(
            root,
            level,
            grant.as_ref(),
            &shown,
            &review.skills,
            &new_names,
            false,
        ) {
            return Ok(code);
        }
        done(source::extension::install(
            cx,
            &held,
            level,
            &review.accepted(),
            grant.as_ref(),
            &new_names,
        ))
    })
}

/// What an install says where it cannot start an extension to ask.
const NO_EXTENSIONS_HERE: &str = "purlis cannot start its extensions here, so the source was not \
                                  asked. Check them in the window's Extensions, then try again.";

/// **What an install was handed besides its skills**: the grant and each new name, or the code it
/// stops with once it has said why.
fn install_settled(
    grant: &[String],
    settled: &Settled,
) -> Result<(Option<Grant>, source::Renames), Code> {
    let grant = if grant.is_empty() {
        None
    } else {
        match Grant::from_words(grant) {
            Ok(grant) => Some(grant),
            Err(why) => {
                voice::err(format!(
                    "{why}: --grant takes root, all, or workspace names"
                ));
                return Err(2);
            }
        }
    };
    match settled.new_names() {
        Ok(new_names) => Ok((grant, new_names)),
        Err(why) => {
            voice::err(esc(&why).to_string());
            Err(2)
        }
    }
}

/// Where an install's review says its skills come from, what it was read at, and what the copy is
/// pinned to, for any kind of source.
struct Shown<'a> {
    from: &'a str,
    at: &'a str,
    pinned_to: &'a str,
}

/// **The review of an install, printed, then asked** unless `yes`: each name still taken stops
/// it. `Some` with the code it ends with where nothing is to be installed, said.
fn reviewed_and_asked(
    root: &Path,
    level: &Level,
    grant: Option<&Grant>,
    shown: &Shown<'_>,
    skills: &[source::SkillReview],
    new_names: &source::Renames,
    yes: bool,
) -> Option<Code> {
    if skills.is_empty() {
        voice::info("Nothing is left to install.");
        return Some(0);
    }
    // A project skill it relies on reaches the workspaces the grant names too (#4172).
    let widenings = source::widenings(
        root,
        level,
        grant,
        skills
            .iter()
            .flat_map(|skill| skill.already.iter().map(String::as_str)),
    );
    print!(
        "{}",
        skills_review_text(shown.from, shown.at, skills, &widenings)
    );
    let stops = unsettled(skills, new_names);
    if !stops.is_empty() {
        for line in &stops {
            voice::err(esc(line).to_string());
        }
        voice::info("Nothing was installed.");
        return Some(1);
    }
    let question = format!(
        "Install {} at {level}, pinned to {}?",
        counted(skills.len(), "skill", "skills"),
        esc(shown.pinned_to)
    );
    if !yes && !asked(&question) {
        voice::info("Nothing was installed.");
        return Some(1);
    }
    None
}

/// What an extension's source offers, as the terminal lists it: each skill, what its files do
/// where they were read, and where it is installed from it here.
fn browse_text(browse: &source::extension::Browse) -> String {
    let mut out = format!(
        "the extension {} at revision {}:\n",
        esc(&browse.extension),
        esc(browse.revision.as_str())
    );
    for one in &browse.skills {
        let mut said = vec![esc(&one.skill.description).to_string()];
        match &one.marks {
            Some(marks) => said.extend(marks.iter().map(|mark| esc(mark).to_string())),
            None => said.push("its files are read when you review it".to_owned()),
        }
        if !one.installed.is_empty() {
            let at: Vec<String> = one.installed.iter().map(ToString::to_string).collect();
            said.push(format!("installed: {}", at.join(", ")));
        }
        out.push_str(&format!(
            "  {}  {}\n",
            esc(&one.skill.name),
            said.join(" · ")
        ));
    }
    out
}

/// What stays behind, each with why, as a review prints it.
fn behind_text(out: &source::Outgoing) -> String {
    let mut text = String::new();
    for skill in &out.skills {
        for left in &skill.left {
            text.push_str(&format!(
                "  stays behind: {}/{} ({})\n",
                esc(&skill.name),
                esc(&left.path),
                left.why
            ));
        }
    }
    for name in &out.broken {
        text.push_str(&format!("  refers to {}, which is not there\n", esc(name)));
    }
    text
}

/// `purlis skill export`.
fn export(
    root: &Path,
    built_in: Option<&Path>,
    name: &str,
    to: &Path,
    zip: bool,
    level: &Level,
) -> Result<Code, CliError> {
    let out = source::gather(root, built_in, level, name, source::Purpose::Export)?;
    print!("{}", behind_text(&out));
    if zip {
        source::export_zip(&out, to)?;
    } else {
        source::export_folder(&out, to)?;
    }
    voice::info(
        esc(&format!(
            "Exported {} to {}.",
            counted(out.skills.len(), "skill", "skills"),
            to.display()
        ))
        .to_string(),
    );
    Ok(0)
}

/// What a person reads before a publish.
fn publish_text(review: &source::publish::PublishReview, source: &Source) -> String {
    let mut text = format!(
        "Publish to {}\n  a draft request into {}, from the new branch {}\n  title: {}\n\nFiles          that go:\n",
        esc(&source.to_string()),
        esc(&review.base),
        esc(&review.branch),
        esc(&review.title)
    );
    text.insert_str(
        text.find("\n\nFiles").unwrap_or(text.len()),
        &format!(
            "\n  the commit names: {} <{}>",
            esc(&review.author.0),
            esc(&review.author.1)
        ),
    );
    for (skill, (folder, replaces)) in review.outgoing.skills.iter().zip(&review.lands) {
        if *replaces {
            text.push_str(&format!("  {} replaces what is there\n", esc(folder)));
        }
        for file in &skill.files {
            let mark = if file.executable { " (runs)" } else { "" };
            text.push_str(&format!(
                "  {}/{}{mark}, {} bytes\n",
                esc(folder),
                esc(&file.path()),
                file.bytes.len()
            ));
        }
    }
    text.push_str(&behind_text(&review.outgoing));
    text.push_str("\nThe request's description, whole:\n");
    for line in review.body.lines() {
        text.push_str(&format!("  {}\n", esc(line)));
    }
    text
}

/// `purlis skill publish`: refused outright from a chat or with no person at a terminal, before
/// the source is asked anything. No flag skips the question.
fn publish(
    root: &Path,
    built_in: Option<&Path>,
    name: &str,
    source: &Source,
    level: &Level,
) -> Result<Code, CliError> {
    use std::io::IsTerminal as _;
    let secrets = purlis_core::secrets::Ctx::new(root, purlis_core::secrets::Env::from_process());
    let place = purlis_core::secrets::setup::in_a_chat(&secrets);
    if !may_publish(
        gradelink::in_a_chat(&purlis_core::envvar::var),
        &place,
        std::io::stdin().is_terminal() && std::io::stderr().is_terminal(),
    ) {
        voice::err(PUBLISH_NEEDS_A_PERSON);
        if let purlis_core::secrets::setup::Where::Unsure(why) = &place {
            voice::err(esc(why).to_string());
        }
        return Ok(1);
    }
    let forge = source::publish::OnTheForge::of(root, source)?;
    let review = source::publish::review(root, built_in, level, name, source, &forge)?;
    print!("{}", publish_text(&review, source));
    if review.stopped() {
        voice::err("Nothing can be sent until these are fixed:");
        for stop in &review.outgoing.stops {
            voice::err(esc(&stop.to_string()).to_string());
        }
        return Ok(1);
    }
    if let Some(said) = route_said(&review.route, forge.display()) {
        voice::warn(esc(&said).to_string());
    }
    let question = match &review.route {
        source::publish::Route::NewFork { path, .. } => format!(
            "Make your fork {} and open this draft request from it, under your own account?",
            esc(path)
        ),
        _ => "Open this draft request, under your own account?".to_owned(),
    };
    if !asked(&question) {
        voice::info("Nothing was sent.");
        return Ok(1);
    }
    match source::publish::publish(
        root,
        built_in,
        level,
        name,
        source,
        &review.accepted(),
        &forge,
    )? {
        source::publish::Published::Opened { url, .. } => {
            voice::info(esc(&format!("Opened {url}")).to_string());
        }
        source::publish::Published::Already { url } => {
            voice::info(esc(&format!("These files already have a request: {url}")).to_string());
        }
        source::publish::Published::PushRefused { why } => {
            let said = match review.route {
                source::publish::Route::Source => format!(
                    "Nothing was opened: git could not push to the source, which takes write \
                     access to it: {why}"
                ),
                _ => format!("Nothing was opened: git could not push to your fork: {why}"),
            };
            voice::err(esc(&said).to_string());
            return Ok(1);
        }
    }
    Ok(0)
}

/// What a publish review says of where its branch goes (#4342): nothing for the source itself,
/// and for the person's fork, why, which fork, who can read it and whether it is made.
fn route_said(route: &source::publish::Route, forge: &str) -> Option<String> {
    let why = format!("{forge} says your account cannot push to this source");
    match route {
        source::publish::Route::Source => None,
        source::publish::Route::Fork(fork) => Some(format!(
            "{why}, so the branch goes on your fork {} ({}), and the draft request is opened from \
             it.",
            fork.web_url(),
            fork.visibility().readers()
        )),
        source::publish::Route::NewFork { path, visibility } => Some(format!(
            "{why}, so publishing makes your fork {path} on {forge}, under your own account \
             ({}), and opens the draft request from it. Nothing is made until you say yes, and \
             the fork stays afterwards: purlis never deletes one. To send it another way, export \
             the skill with `purlis skill export`.",
            visibility.readers()
        )),
    }
}

/// What a publish says with no person to ask.
const PUBLISH_NEEDS_A_PERSON: &str = "Publishing takes a yes from a person at a terminal, so \
                                      nothing was sent. Run `purlis skill publish` in your own \
                                      terminal, outside any chat.";

/// Whether a publish may ask at all: a terminal, and no sign of a chat in the environment or in
/// where this process runs. A doubt about where it runs counts as a chat.
fn may_publish(
    chat_in_the_environment: bool,
    place: &purlis_core::secrets::setup::Where,
    at_a_terminal: bool,
) -> bool {
    !chat_in_the_environment
        && matches!(place, purlis_core::secrets::setup::Where::Outside)
        && at_a_terminal
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

/// **The grade a source carries for a skill**, as the terminal prints it (ADR 0100, amended
/// 2026-11-08): whose it is and that purlis has not checked it before any figure. Nothing where
/// the source carries none.
fn grade_text(grade: Option<&source::SourceGrade>) -> String {
    let rate = |percent: Option<u8>| percent.map_or("not run".to_owned(), |p| format!("{p}%"));
    match grade {
        None => String::new(),
        Some(source::SourceGrade::LeftOut { whose, why }) => format!(
            "  Grade: {}'s grade file is left out: {}\n",
            esc(whose),
            esc(why)
        ),
        Some(source::SourceGrade::Shown {
            whose,
            newest,
            this_version,
            more,
        }) => {
            let version = if *this_version {
                "graded on this version"
            } else {
                "graded on an earlier version"
            };
            let mut out = format!(
                "  Grade: {}'s grade, as published — not checked by purlis\n    \
                 {} with it, {} without it, {version}\n    {}, {} grader, {}, {}: {}\n",
                esc(whose),
                rate(newest.with),
                rate(newest.without),
                esc(&newest.harness),
                esc(&newest.grader),
                counted(usize::from(newest.runs), "run", "runs"),
                esc(&newest.date),
                esc(&newest.ended),
            );
            if *more > 0 {
                out.push_str(&format!(
                    "    The file holds {}.\n",
                    counted(*more, "earlier grade", "earlier grades")
                ));
            }
            out
        }
    }
}

/// Why a skill of an install's review comes along: `by` refers to it, or, where `hinted`, only
/// names it bare (#4510).
fn comes_along(by: &str, hinted: bool) -> String {
    if hinted {
        format!(" (comes along: {by} names it bare, read as a hint)")
    } else {
        format!(" (comes along: {by} refers to it)")
    }
}

/// The review of installing `skills` from `from` at `at` (a commit or a revision), as the
/// terminal prints it, for any kind of source (#5760).
fn skills_review_text(
    from: &str,
    at: &str,
    skills: &[source::SkillReview],
    widenings: &[source::Widening],
) -> String {
    let mut out = format!(
        "Review of {} from {} at {}\n",
        counted(skills.len(), "skill", "skills"),
        esc(from),
        esc(at)
    );
    for skill in skills {
        let brought = skill
            .brought_by
            .as_deref()
            .map(|by| comes_along(by, skill.brought_by_hint))
            .unwrap_or_default();
        out.push_str(&format!(
            "\n{}{}: {}\n",
            esc(&skill.name),
            esc(&brought),
            esc(&skill.description)
        ));
        out.push_str(&summary_text(&skill.summary, "  "));
        out.push_str(&grade_text(skill.grade.as_ref()));
        out.push_str(&already_text(skill, widenings, "install"));
        for line in &skill.notes {
            out.push_str(&format!("  {}\n", esc(line)));
        }
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

/// Whether the installed skill `name` at `level` came from an extension's source.
fn from_an_extension(root: &std::path::Path, level: &Level, name: &str) -> bool {
    source::lock::read(root, source::Kept::of(level)).is_ok_and(|held| {
        held.lock
            .skills
            .iter()
            .any(|one| one.is(level, name) && matches!(one.source, source::Origin::Extension(_)))
    })
}

/// `purlis skill update <name>` for a skill an extension's source answered (#3089): its review,
/// asked through the executor's gate, printed and asked about, then the update from what that
/// review read, the extension not asked again.
fn update_from_an_extension(
    cx: &mut verbs::Ctx<'_>,
    level: &Level,
    name: &str,
    yes: bool,
) -> Result<Code, CliError> {
    let root = cx.plane;
    let executor = Executor::with_built_in(purlis_core::extension::BuiltIn::of_this_program());
    purlis_core::skills::studio::with_studio(&executor, root, |studio| {
        let Some(studio) = studio else {
            voice::err(NO_EXTENSIONS_HERE);
            return Ok(1);
        };
        let (mut update, held) = match source::extension::update_review(studio, root, level, name) {
            Ok(read) => read,
            Err(why) => {
                voice::err(esc(&why).to_string());
                return Ok(1);
            }
        };
        studio.lint_reviewed(update.skills_mut(), level);
        print!("{}", update_text(&update));
        if !yes
            && !asked(&format!(
                "Update {} to {}?",
                esc(name),
                esc(update.newer.short())
            ))
        {
            voice::info("Nothing was changed.");
            return Ok(1);
        }
        done(source::extension::update(
            cx,
            &held,
            level,
            name,
            &update.accepted(),
            update.grant.as_ref(),
        ))
    })
}

/// An update's review as the terminal prints it: the summary's changes, then each file's diff,
/// the edits made here, and the skills that come along. Every field escaped.
fn update_text(update: &source::UpdateReview) -> String {
    let mut out = format!(
        "Update of {} at {}: {} -> {}\n",
        esc(&update.name),
        update.level,
        esc(update.pinned.short()),
        esc(update.newer.short())
    );
    if !update.from_pin {
        out.push_str(match update.pinned {
            source::Pin::Commit(_) => {
                "The pinned commit is no longer in the source: compared with the copy here.\n"
            }
            source::Pin::Revision(_) => "Compared with the copy here.\n",
        });
    }
    if update.changes.is_empty() {
        out.push_str("The summary does not change.\n");
    } else {
        out.push_str("Added:\n");
        out.push_str(&summary_text(&update.changes.added, "  "));
        out.push_str("Removed:\n");
        out.push_str(&summary_text(&update.changes.removed, "  "));
    }
    out.push_str(&grade_text(update.review.grade.as_ref()));
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
fn done<T>(answer: Result<T, String>) -> Result<Code, CliError> {
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
fn new_text(from: Option<&Path>, was: &str) -> Result<String, CliError> {
    match from {
        Some(path) if path == Path::new("-") => {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)
                .map_err(|why| format!("stdin could not be read ({why})"))?;
            Ok(text)
        }
        Some(path) => std::fs::read_to_string(path)
            .map_err(|why| format!("{} could not be read ({why})", path.display()))
            .map_err(CliError::from),
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
            made.map_err(|why| {
                CliError::internal(format!("no copy could be made to edit ({why})"))
            })?;
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
                return Err(format!("{program} exited {status}, so nothing was saved").into());
            }
            text.map_err(|why| {
                CliError::internal(format!("the edited copy could not be read ({why})"))
            })
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

/// The level a row is shown at, as the studio shows it: `project`, `local project`, and
/// `workspace ops` for another workspace's skill a persona names by its workspace (#2933).
fn level(row: &StudioRow) -> String {
    row.level_label()
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
    // A person sees where each came from. A chat that runs this never does (ADR 0100, amended
    // 2026-11-01).
    let in_a_chat = purlis_core::envvar::var(purlis_core::active::SESSION_ID_ENV)
        .is_some_and(|id| !id.is_empty());
    let shown: Vec<String> = rows
        .iter()
        .map(|row| match &row.source_label {
            Some(label) if !in_a_chat => format!("{} · {label}", row.name),
            _ => row.name.clone(),
        })
        .collect();
    let names: Vec<&str> = shown.iter().map(String::as_str).collect();
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
