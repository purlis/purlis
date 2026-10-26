// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis theme list`, `check`, `draft`, `import`, `export` and `parts` (spec #2313, #2321,
//! #2332, #2334, #2903): every theme available on this machine and where it comes from, the theme
//! check of any one of them, a chat's theme draft, a theme folder made from a theme of another
//! program, a theme folder as a zip, and every component part a theme may set.
//!
//! **Nothing here judges a theme.** The theme engine (`purlis_core::theme`) reads it, refuses it or
//! says what it left out, and checks each appearance; this module finds the themes there are, asks
//! the engine about each, and prints its answer, as a table for a person or as JSON for a chat.
//! `list` and `check` write nothing: they read the built-in themes, the person's `theme.json`, the
//! theme folders of every level and the approved extensions' themes, and leave every file as they
//! found it.
//!
//! **`import` and `export`** (#2334): the engine makes a theme folder's files from a VS Code
//! colour theme, a base16 or base24 scheme, an iTerm2 or a Ghostty palette or a theme folder's
//! zip, through its fixed mapping tables, and lists every source key no token takes. `import`
//! writes them beside where they go, has the engine read them there as it reads any theme
//! folder, and puts in place only a theme it draws: one theme folder, in the machine level's
//! `themes/` unless `--into` names another, never over one that is there without `--force`.
//! `export` writes one zip of a theme folder that `import` gives back file for file; a level's
//! folder is named by its id (#2612) and found by the engine at that level alone. With
//! `--kind design-tokens` (#2926) both speak the W3C design-tokens format 2025.10 instead, which
//! `import` reads back to the same folder. A staged import
//! is read only to check (#2587): none of its fonts is left registered once it moves or goes.
//!
//! **Where a theme comes from, and its level** (#2518): built in (Purlis, `purlis`, the default,
//! and Purlis Classic, `purlis-classic`, #3034); the person's own `theme.json` (its id
//! is [`MINE`]) and the theme folders in the person's `themes/`, at the machine level; the
//! project's `themes/` at the plane root; each workspace's `themes/`; or an approved extension that
//! is on (its id is `<extension>/<theme>`, as a project's pick names it). A theme folder's id names
//! its level and its folder: `machine:<folder>`, `project:<folder>`,
//! `workspace:<workspace>:<folder>`. The levels' folders are read by the engine exactly as the
//! window reads them ([`theme::Folders`]: through handles, refusing a link, under the caps, each
//! extending the folders of its own level and the levels farther out), and nothing here reads them
//! another way. A theme file or a theme folder anywhere else can be checked by its path.
//!
//! **The check is of what the window draws**: each combination says the consent floors the
//! window holds when it draws it, and the tokens it draws as purlis's own for them.
//!
//! **A chat's theme draft** (#2332): `draft <folder>` makes this chat's draft in purlis's local
//! folder (`theme::drafts`), and `check draft:<folder>` checks it as the window draws it: with
//! its hard floors, which keep a draft that breaks one from being drawn.
//!
//! **Each level's choice** (#2707): `list` shows, after the themes, what every level chose,
//! nearest first: the person's choice and the committed default of each workspace, of the
//! project and the person's machine default, each with the level of the theme folder it reads
//! (`from`), its appearance and its option values. `check` names the levels that chose the theme
//! it checks. The levels are read by the engine ([`theme::Levels::read`]) exactly as the window
//! reads them; whether a choice is drawn where it is made is the window's to resolve, since it
//! hangs on the extensions in force and the system's appearance there.
//!
//! **The exit is a linter's.** `check` exits 0 when the theme is drawn, notes and warnings or
//! not (a contrast finding is the author's call, ADR 0097 §2); 1 when it is refused, or is a
//! draft that breaks a hard floor; 2 when there is no such theme here.

use std::path::Path;
use std::process::ExitCode;

use clap::Subcommand;
use purlis_core::theme::levels::{Found as AtLevel, Reach};
use purlis_core::theme::{self, Complaint, Folders, Resolved, Theme};
use purlis_core::{extension, shown};
use serde_json::{Value, json};

/// The version of the JSON `list --json` and `check --json` answer, as `hook --list --json`
/// carries one: a reader checks it before it reads the rest.
///
/// Beside it, both answers carry `theme_schema`: the address of the theme schema this purlis
/// reads (`theme::SCHEMA_URL`), the one a theme written for it names in its `$schema`.
pub const SCHEMA: u64 = 1;

/// The id of the person's own theme, `theme.json` beside `machine.json`.
pub const MINE: &str = "mine";

/// How a theme folder's id names the machine level: `machine:<folder>`.
pub const MACHINE: &str = "machine";

/// How a theme folder's id names the project level: `project:<folder>`.
pub const PROJECT: &str = "project";

/// How a theme folder's id names a workspace's level: `workspace:<workspace>:<folder>`.
pub const WORKSPACE: &str = "workspace";

/// How an id names this chat's theme draft (#2332): `draft:<folder>`.
pub const DRAFT: &str = "draft";

/// `purlis theme …`.
#[derive(Subcommand)]
pub enum ThemeCommand {
    /// Every theme available here, with its id, its appearances, its level and where it comes
    /// from: built in (`purlis-classic`); your own theme.json (`mine`) and your theme folders
    /// (`machine:<folder>`); the project's (`project:<folder>`); each workspace's
    /// (`workspace:<workspace>:<folder>`); or an approved extension (`<extension>/<theme>`). A
    /// theme purlis refuses is listed as refused, with why.
    List {
        /// Answer as JSON, for a chat or a script.
        #[arg(long)]
        json: bool,
    },
    /// The theme check of one theme: why it is refused, if it is; what purlis left out of it;
    /// its contrast warnings in every combination of its appearances and option values (each
    /// pair with its ratio and its floor, as it is and in every workspace tint); the consent
    /// floors the window holds in each, drawing purlis's own tokens for them; and its hard
    /// floors. A theme with more than 64 combinations is refused with its count. Exits 0 when
    /// the theme is drawn, 1 when it is refused, 2 when there is no such theme.
    Check {
        /// A theme's id from `purlis theme list` (a theme folder's names its level:
        /// `project:dusk`), or the path of a theme file or a theme folder. An id may hold a
        /// space (`solarized/Solarized Dark`): quote it.
        theme: String,
        /// Answer as JSON, for a chat or a script. Its `clean` is true with no error, no note,
        /// no warning and no floor held; it does not cover hard floors, which `hard_floors`
        /// reports on its own: each one broken, in each combination. `draft:<folder>` checks
        /// this chat's theme draft, which exits 1 when it breaks a hard floor, since the
        /// window does not draw it.
        #[arg(long)]
        json: bool,
    },
    /// This chat's theme draft of a theme folder (#2332), which the app that started the chat
    /// writes in purlis's local folder, never committed: a copy of the workspace's or the
    /// project's folder of that name, or a new theme that extends Purlis, the default. It also makes a
    /// working copy for you to edit; `--save` hands that to the app. The draft is drawn only in
    /// this chat's workspace, and the person keeps or reverts it in the window. Run it in a chat
    /// purlis started. Exits 0 when the draft is drawn, 1 when it is not or nothing was written.
    Draft {
        /// The theme folder's name: lower-case letters, digits and dashes.
        folder: String,
        /// Hand the working copy's theme.json (and its icon sets) to the app as the draft, and
        /// say what the window draws of it. Exits 1 when it is not drawn.
        #[arg(long)]
        save: bool,
        /// The working copy: a folder of yours holding theme.json. `$TMPDIR/purlis-theme-drafts/
        /// <folder>` when not given.
        #[arg(long)]
        from: Option<String>,
    },
    /// Make a theme folder from a theme you already have: a VS Code colour theme (JSON with
    /// comments), a base16 or base24 scheme (YAML or JSON), an iTerm2 colour preset
    /// (.itermcolors), a Ghostty theme, a theme folder's zip from `purlis theme export`, or a
    /// W3C design-tokens file (format 2025.10, `.tokens.json`) from `purlis theme export --kind
    /// design-tokens` or a design tool. Each kind goes through a fixed mapping table (`purlis
    /// docs show themes`); every key of the source no token takes is listed, and the new
    /// theme's check is shown. It never
    /// replaces a theme folder that is there without --force. Exits 0 when the folder is
    /// written, 1 when it is not, 2 when there is no such file.
    Import {
        /// The file to import.
        source: String,
        /// What the file is, when its name and its first words do not say.
        #[arg(long, value_parser = ["vscode", "base16", "base24", "iterm", "ghostty", "zip", "design-tokens"])]
        kind: Option<String>,
        /// The folder the new theme folder goes in: a `themes/` folder. Your own themes'
        /// folder (`themes/` in purlis's config folder) when not given.
        #[arg(long)]
        into: Option<String>,
        /// Replace a theme folder of the same name that is there.
        #[arg(long)]
        force: bool,
        /// Answer as JSON, for a chat or a script.
        #[arg(long)]
        json: bool,
    },
    /// Write a theme folder as a zip to hand to someone: its theme.json, the files beside it
    /// (a licence, a preview) and its fonts/ and icons/, under one folder of its name, so
    /// `purlis theme import` (or unzip) gives the folder back. `--kind design-tokens` writes
    /// it as one W3C design-tokens file (format 2025.10) for a design tool instead, which
    /// `purlis theme import` gives back the same. What it does not carry is listed. A theme
    /// folder's id (`project:dusk`) exports that level's own folder, never a farther level's of
    /// the same name. Exits 0 when the file is written, 1 when it is not, 2 when there is no
    /// such theme.
    Export {
        /// A theme folder's id from `purlis theme list` (`machine:<folder>`,
        /// `project:<folder>`, `workspace:<workspace>:<folder>`), the path of a theme folder,
        /// or `purlis-classic`.
        theme: String,
        /// The file to write: `<theme folder's name>.zip` here when not given, or
        /// `<theme folder's name>.tokens.json` for design tokens.
        #[arg(long)]
        to: Option<String>,
        /// What to write: a zip (`zip`, when not given), or a W3C design-tokens file
        /// (`design-tokens`).
        #[arg(long, value_parser = ["zip", "design-tokens"])]
        kind: Option<String>,
        /// Replace a file that is there.
        #[arg(long)]
        force: bool,
    },
    /// Every component part a theme may set (#2903), as JSON: the key grammar
    /// (`<family>.<part>[.<variant>][.<state>].<property>`), each state and the attribute it is
    /// drawn by, each property with its bounds and whether a state may set it, and every family
    /// with its parts and variants, each with its description and the manifest version it came
    /// in. A theme names them in its theme.json's `parts`. Writes nothing; exits 0.
    Parts,
}

/// Where a theme comes from.
#[derive(Debug, Clone)]
enum From {
    /// Compiled into purlis.
    BuiltIn,
    /// The person's own `theme.json`, at this path.
    Mine(String),
    /// An approved extension that is on, by its id.
    Extension(String),
    /// A theme file named by its path on the command line.
    File(String),
    /// A theme folder named by its path on the command line.
    Folder(String),
    /// A theme folder in a level's `themes/`: its level, its folder's name and its path.
    Level {
        level: Level,
        folder: String,
        path: String,
    },
    /// This chat's theme draft (#2332): its folder's name and its path.
    Draft { folder: String, path: String },
}

/// The level of a theme folder in a level's `themes/`.
#[derive(Debug, Clone)]
enum Level {
    Machine,
    Project,
    Workspace(String),
}

impl Level {
    /// How far a choice made at this level reaches: the level itself, read as it reads it.
    fn reach(&self) -> Reach {
        match self {
            Self::Machine => Reach::Machine,
            Self::Project => Reach::Project,
            Self::Workspace(_) => Reach::Workspace,
        }
    }

    /// The id of its folder `folder`, as `list` says it and `check` takes it.
    fn id(&self, folder: &str) -> String {
        match self {
            Self::Machine => format!("{MACHINE}:{folder}"),
            Self::Project => format!("{PROJECT}:{folder}"),
            Self::Workspace(name) => format!("{WORKSPACE}:{name}:{folder}"),
        }
    }

    /// The level as `from` and the table's FROM column name it.
    fn word(&self) -> &'static str {
        match self {
            Self::Machine => MACHINE,
            Self::Project => PROJECT,
            Self::Workspace(_) => WORKSPACE,
        }
    }

    /// How an id names a folder of this level, as a sentence shows it.
    fn form(&self) -> &'static str {
        match self {
            Self::Machine => "machine:<folder>",
            Self::Project => "project:<folder>",
            Self::Workspace(_) => "workspace:<workspace>:<folder>",
        }
    }

    /// The level's own `themes/`, as a sentence names it.
    fn own_themes(&self) -> String {
        match self {
            Self::Machine => "your own themes/".to_owned(),
            Self::Project => "the project's own themes/".to_owned(),
            Self::Workspace(name) => {
                format!("the workspace {}'s own themes/", shown::short(name))
            }
        }
    }

    /// The level an id names, and the folder: `None` when `asked` is not a level's id.
    fn of_id(asked: &str) -> Option<(Self, &str)> {
        let (level, rest) = asked.split_once(':')?;
        let (level, folder) = match level {
            MACHINE => (Self::Machine, rest),
            PROJECT => (Self::Project, rest),
            WORKSPACE => {
                let (name, folder) = rest.split_once(':')?;
                (Self::Workspace(name.to_owned()), folder)
            }
            _ => return None,
        };
        // The grammar alone: a folder of it the platform rule refuses (`con`) is found and
        // refused by the engine with why.
        theme::is_folder_spelling(folder).then_some((level, folder))
    }
}

impl From {
    /// As the table's FROM column says it.
    fn said(&self) -> String {
        match self {
            Self::BuiltIn => "built-in".to_owned(),
            Self::Mine(path) => format!("{MACHINE} {}", shown::short(path)),
            Self::File(path) | Self::Folder(path) => shown::short(path),
            Self::Extension(id) => format!("extension {}", shown::short(id)),
            Self::Level {
                level: Level::Workspace(name),
                path,
                ..
            } => format!("{WORKSPACE} {} {}", shown::short(name), shown::short(path)),
            Self::Level { level, path, .. } => {
                format!("{} {}", level.word(), shown::short(path))
            }
            Self::Draft { path, .. } => format!("{DRAFT} {}", shown::short(path)),
        }
    }

    /// The level a theme comes from, as `level` in the JSON says it; `None` for a theme named
    /// by its path, which is at no level.
    fn level(&self) -> Option<&'static str> {
        match self {
            Self::BuiltIn => Some("built-in"),
            Self::Mine(_) => Some(MACHINE),
            Self::Extension(_) => Some("extension"),
            Self::Level { level, .. } => Some(level.word()),
            Self::Draft { .. } => Some(DRAFT),
            Self::File(_) | Self::Folder(_) => None,
        }
    }

    fn json(&self) -> Value {
        match self {
            Self::BuiltIn => json!({ "kind": "built-in" }),
            Self::Mine(path) => json!({ "kind": "mine", "path": path }),
            Self::Extension(id) => json!({ "kind": "extension", "extension": id }),
            Self::File(path) => json!({ "kind": "file", "path": path }),
            Self::Folder(path) => json!({ "kind": "folder", "path": path }),
            Self::Level {
                level: Level::Workspace(name),
                folder,
                path,
            } => json!({ "kind": WORKSPACE, "workspace": name, "folder": folder, "path": path }),
            Self::Level {
                level,
                folder,
                path,
            } => json!({ "kind": level.word(), "folder": folder, "path": path }),
            Self::Draft { folder, path } => {
                json!({ "kind": DRAFT, "folder": folder, "path": path })
            }
        }
    }
}

/// One theme found here, as the engine read it.
struct Found {
    id: String,
    from: From,
    /// The theme, when it is drawn; `None` when the engine refused it.
    theme: Option<Theme>,
    /// Why it is refused, as one sentence; `None` when it is drawn.
    refused: Option<String>,
    /// What refused it, one complaint each.
    errors: Vec<Complaint>,
    /// What the engine left out of a theme it draws: a token or key purlis does not have.
    notes: Vec<Complaint>,
}

impl Found {
    /// What the engine answered for a theme.
    fn of(id: String, from: From, resolved: Resolved) -> Self {
        if resolved.refused {
            let why: Vec<&str> = resolved
                .complaints
                .iter()
                .map(|one| one.said.as_str())
                .collect();
            Self {
                id,
                from,
                theme: None,
                refused: Some(why.join("; ")),
                errors: resolved.complaints,
                notes: Vec::new(),
            }
        } else {
            Self {
                id,
                from,
                theme: Some(resolved.theme),
                refused: None,
                errors: Vec::new(),
                notes: resolved.complaints,
            }
        }
    }

    fn classic() -> Self {
        Self::built_in(theme::CLASSIC, theme::classic())
    }

    /// Purlis (#2336), the default (#3034).
    fn purlis() -> Self {
        Self::built_in(theme::PURLIS, theme::purlis())
    }

    fn built_in(id: &str, theme: &Theme) -> Self {
        Self::of(
            id.to_owned(),
            From::BuiltIn,
            Resolved {
                theme: theme.clone(),
                complaints: Vec::new(),
                refused: false,
            },
        )
    }

    /// Its name as the table says it: a built-in theme that ships as a preview says so.
    fn name_said(&self, theme: &Theme) -> String {
        let name = shown::short(&theme.name);
        if matches!(self.from, From::BuiltIn) && theme::is_preview(&self.id) {
            format!("{name} (preview)")
        } else {
            name
        }
    }

    fn summary_json(&self) -> Value {
        let (options, combinations) = match &self.theme {
            Some(theme) => (
                serde_json::to_value(&theme.options).unwrap_or_default(),
                Value::from(theme.combinations()),
            ),
            None => (Value::Null, Value::Null),
        };
        let (name, appearances) = match &self.theme {
            Some(theme) => (
                Value::from(theme.name.clone()),
                Value::from(
                    theme
                        .appearances
                        .iter()
                        .map(|one| {
                            json!({
                                "id": one.id,
                                "appearance": one.appearance,
                                "high_contrast": one.high_contrast,
                            })
                        })
                        .collect::<Vec<_>>(),
                ),
            ),
            None => (Value::Null, Value::Null),
        };
        let mut summary = json!({
            "id": self.id,
            "name": name,
            "appearances": appearances,
            "options": options,
            "combinations": combinations,
            "level": self.from.level(),
            "from": self.from.json(),
            "refused": self.refused,
        });
        // A built-in theme says whether it ships as a preview (#2336); no other theme has the key.
        if matches!(self.from, From::BuiltIn) {
            summary["preview"] = Value::from(theme::is_preview(&self.id));
        }
        summary
    }
}

/// `purlis theme …`, run.
pub fn run(command: &ThemeCommand) -> ExitCode {
    let config = purlis_core::machine::config_root();
    let plane = plane();
    let here = Here {
        config: config.as_deref(),
        plane: plane.as_deref(),
    };
    match command {
        ThemeCommand::List { json } => {
            let found = available(here);
            let chosen = every_choice(here);
            if *json {
                let themes: Vec<Value> = found.iter().map(Found::summary_json).collect();
                let choices: Vec<Value> = chosen.iter().map(Chose::json).collect();
                println!(
                    "{}",
                    json!({
                        "schema": SCHEMA,
                        "theme_schema": theme::SCHEMA_URL,
                        "themes": themes,
                        "choices": choices,
                    })
                );
            } else {
                print!("{}{}", table(&found), choices_table(&chosen));
            }
            ExitCode::SUCCESS
        }
        ThemeCommand::Check { theme, json } => {
            let found = match find(here, theme) {
                Ok(found) => found,
                Err(said) => {
                    eprintln!(
                        "purlis: there is no theme '{}' here — `purlis theme list` lists the \
                         themes there are, with a theme folder named by its level \
                         (`project:<folder>`), and `purlis theme check <path>` checks a theme \
                         file or folder",
                        shown::short(theme)
                    );
                    for one in said {
                        eprintln!("  {}", shown::readable(&one.said, 240));
                    }
                    return ExitCode::from(2);
                }
            };
            let checked = checks(&found);
            let chosen: Vec<Chose> = every_choice(here)
                .into_iter()
                .filter(|one| one.id.as_deref() == Some(found.id.as_str()))
                .collect();
            if *json {
                let mut said = checked_json(&found, &checked);
                said["chosen"] = chosen.iter().map(Chose::json).collect();
                println!("{said}");
            } else {
                print!("{}", checked_text(&found, &checked, &chosen, &|_, _| None));
            }
            // A draft that breaks a hard floor is not drawn: the linter's 1, as a refusal is.
            let undrawn = matches!(found.from, From::Draft { .. })
                && checked.iter().any(|one| !one.hard.is_empty());
            ExitCode::from(u8::from(found.refused.is_some() || undrawn))
        }
        ThemeCommand::Draft { folder, save, from } => draft(here, folder, *save, from.as_deref()),
        ThemeCommand::Import {
            source,
            kind,
            into,
            force,
            json,
        } => import(
            config.as_deref(),
            source,
            kind.as_deref(),
            into.as_deref(),
            *force,
            *json,
        ),
        ThemeCommand::Export {
            theme,
            to,
            kind,
            force,
        } => {
            let wrapping = if kind.as_deref() == Some("design-tokens") {
                theme::Wrapping::DesignTokens
            } else {
                theme::Wrapping::Zip
            };
            export(here, theme, to.as_deref(), wrapping, *force)
        }
        ThemeCommand::Parts => {
            println!("{}", parts_json());
            ExitCode::SUCCESS
        }
    }
}

/// `purlis theme parts`: the engine's closed manifest of component parts, with the version of
/// this answer and the theme schema a theme naming them is written against.
fn parts_json() -> String {
    let mut said = json!({ "schema": SCHEMA, "theme_schema": theme::SCHEMA_URL });
    if let (Some(said), Value::Object(manifest)) =
        (said.as_object_mut(), theme::parts::manifest_json())
    {
        said.extend(manifest);
    }
    serde_json::to_string_pretty(&said).unwrap_or_default()
}

/// The most of a path a line prints.
const PATH_LIMIT: usize = 1024;

/// The folder your own theme folders are kept in: `themes/` in purlis's config folder.
fn your_themes(config: &Path) -> std::path::PathBuf {
    purlis_core::machine::dir(config).join(theme::levels::THEMES)
}

/// `purlis theme import`: the engine makes the folder's files from the source, they are
/// written beside where they go, the engine reads them there as it reads any theme folder, and
/// only a theme it draws is put in place.
fn import(
    config: Option<&Path>,
    source: &str,
    kind: Option<&str>,
    into: Option<&str>,
    force: bool,
    json: bool,
) -> ExitCode {
    let path = Path::new(source);
    if !path.is_file() {
        eprintln!(
            "purlis: there is no file '{}' to import",
            shown::short(source)
        );
        return ExitCode::from(2);
    }
    let bytes = match read_source(path) {
        Ok(bytes) => bytes,
        Err(why) => return not_written(&why),
    };
    let file_name = path
        .file_name()
        .map_or_else(|| source.to_owned(), |it| it.to_string_lossy().into_owned());
    let kind = match kind.and_then(theme::SourceKind::named) {
        Some(kind) => kind,
        None => match theme::detect(&file_name, &bytes) {
            Some(kind) => kind,
            None => {
                return not_written(&format!(
                    "purlis cannot tell what {} is: say which with --kind (vscode, base16, \
                     base24, iterm, ghostty, zip or design-tokens)",
                    shown::short(&file_name)
                ));
            }
        },
    };
    let imported = match theme::import(kind, &file_name, &bytes) {
        Ok(imported) => imported,
        Err(why) => return not_written(&why),
    };
    let into = match (into, config) {
        (Some(into), _) => std::path::PathBuf::from(into),
        (None, Some(config)) => your_themes(config),
        (None, None) => {
            return not_written(
                "purlis has no config folder to keep your themes in here: say where with --into",
            );
        }
    };
    let staged = match theme::stage(&into, &imported.folder, &imported.files, force) {
        Ok(staged) => staged,
        Err(why) => return not_written(&why.to_string()),
    };
    let target = into.join(&imported.folder);
    // Read only to check: nothing of the stage is left registered once it moves or goes.
    let found = Found::of(
        imported.folder.clone(),
        From::Folder(target.display().to_string()),
        staged.read(),
    );
    let checked = checks(&found);
    let written = if found.refused.is_some() {
        drop(staged);
        None
    } else {
        match staged.commit() {
            Ok(path) => Some(path),
            Err(why) => return not_written(&why.to_string()),
        }
    };
    if json {
        println!(
            "{}",
            imported_json(&imported, written.as_deref(), &found, &checked)
        );
    } else {
        print!(
            "{}",
            imported_text(&imported, written.as_deref(), &found, &checked)
        );
    }
    ExitCode::from(u8::from(written.is_none()))
}

/// A source file, read under the cap every import reads under (a zip's own is larger).
fn read_source(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let most = theme::MOST_ZIP_BYTES
        .max(theme::MOST_SOURCE_BYTES)
        .max(theme::design_tokens::MOST_BYTES);
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(most + 1).read_to_end(&mut bytes))
        .map_err(|why| format!("{} could not be read: {why}", path.display()))?;
    if bytes.len() as u64 > most {
        return Err(format!(
            "{} is larger than {} MiB, the most purlis imports",
            path.display(),
            most / 1024 / 1024
        ));
    }
    Ok(bytes)
}

/// Nothing was written, for `why`: said on standard error, exit 1.
fn not_written(why: &str) -> ExitCode {
    eprintln!("purlis: {}", shown::readable(why, 480));
    ExitCode::from(1)
}

/// What an import says to a person: where the folder went (or that it did not), what was left
/// out of the source, and the new theme's check.
fn imported_text(
    imported: &theme::Imported,
    written: Option<&Path>,
    found: &Found,
    checked: &[theme::Checked],
) -> String {
    let mut out = match written {
        Some(path) => format!(
            "imported {} ({}) into {}\n",
            shown::short(&imported.name),
            imported.kind.name(),
            shown::readable(&path.display().to_string(), PATH_LIMIT)
        ),
        None => format!(
            "not imported: the theme {} makes is refused, so nothing was written\n",
            imported.kind.name()
        ),
    };
    if imported.left_out.is_empty() {
        out.push_str("not taken: nothing\n");
    } else {
        out.push_str(&format!("not taken ({}):\n", imported.left_out.len()));
        for one in &imported.left_out {
            out.push_str(&format!(
                "  {}: {}\n",
                shown::readable(&one.key, 160),
                shown::readable(&one.why, 240)
            ));
        }
    }
    // A terminal's own colours are never tuned (#2589): a warning about one names the palette
    // entry that holds it, where the person changes it.
    let terminal = matches!(
        imported.kind,
        theme::SourceKind::Iterm | theme::SourceKind::Ghostty
    );
    let entry = |appearance: &str, token: &str| {
        terminal
            .then(|| imported.palette_entry(appearance, token))
            .flatten()
    };
    out.push_str(&checked_text(found, checked, &[], &entry));
    if let Some(path) = written {
        let path = shown::readable(&path.display().to_string(), PATH_LIMIT);
        out.push_str(&format!(
            "the theme is in {path}: `purlis theme check {path}` checks it again, and `purlis \
             theme export {path}` makes a zip of it to share\n"
        ));
    }
    out
}

/// What an import says to a chat: `written` (the folder's path, or `null`), what was not
/// taken from the source, and the check as `purlis theme check --json` answers it.
fn imported_json(
    imported: &theme::Imported,
    written: Option<&Path>,
    found: &Found,
    checked: &[theme::Checked],
) -> Value {
    json!({
        "schema": SCHEMA,
        "kind": imported.kind,
        "name": imported.name,
        "folder": imported.folder,
        "written": written.map(|it| it.display().to_string()),
        "not_taken": imported.left_out,
        "check": checked_json(found, checked),
    })
}

/// The folder Purlis Classic is exported as: its own id is kept for the built-in, so a copy
/// imported back sits beside it and never in its place.
const CLASSIC_COPY: &str = "purlis-classic-copy";

/// The folder Purlis is exported as, for the same reason.
const PURLIS_COPY: &str = "purlis-copy";

/// `purlis theme export`: a built-in theme as its copy, a level's theme folder by its id, or a
/// theme folder by its path, as a zip or as design tokens (`wrapping`).
fn export(
    here: Here<'_>,
    asked: &str,
    to: Option<&str>,
    wrapping: theme::Wrapping,
    force: bool,
) -> ExitCode {
    let packed = if asked != theme::CLASSIC && theme::built_in(asked).is_some() {
        let files = theme::purlis_files();
        wrapping
            .pack(PURLIS_COPY, &files)
            .map(|bytes| theme::Packed {
                name: PURLIS_COPY.to_owned(),
                bytes,
                files: files.len(),
                left_out: Vec::new(),
            })
    } else if asked == theme::CLASSIC {
        let files = theme::Files::from([(
            theme::THEME_JSON.to_owned(),
            theme::CLASSIC_THEME_JSON.as_bytes().to_vec(),
        )]);
        wrapping
            .pack(CLASSIC_COPY, &files)
            .map(|bytes| theme::Packed {
                name: CLASSIC_COPY.to_owned(),
                bytes,
                files: 1,
                left_out: Vec::new(),
            })
    } else if let Some((level, folder)) = Level::of_id(asked) {
        // Packed from the handle the level's folder was found through.
        match folders(here, &level).folder_at(folder, level.reach()) {
            Ok(found) => found.export_as(wrapping),
            Err(said) => {
                eprintln!(
                    "purlis: there is no theme folder '{}' at the {} level — `{}` exports a \
                     folder of {}, never a farther level's of the same name",
                    shown::short(folder),
                    level.word(),
                    level.form(),
                    level.own_themes()
                );
                for one in said {
                    eprintln!("  {}", shown::readable(&one.said, 240));
                }
                return ExitCode::from(2);
            }
        }
    } else {
        let path = Path::new(asked);
        if !path.is_dir() {
            let why = if path.exists() || asked == MINE {
                " — export takes a theme folder: put a theme file in a folder of its own first"
            } else {
                ""
            };
            eprintln!(
                "purlis: there is no theme folder '{}' here{why}",
                shown::short(asked)
            );
            return ExitCode::from(2);
        }
        theme::export_as(path, wrapping)
    };
    let packed = match packed {
        Ok(packed) => packed,
        Err(why) => return not_written(&why),
    };
    let to = to.map_or_else(
        || std::path::PathBuf::from(format!("{}{}", packed.name, wrapping.ending())),
        std::path::PathBuf::from,
    );
    if let Err(why) = write_whole(&to, &packed.bytes, force) {
        return not_written(&why);
    }
    let mut out = format!(
        "exported {} ({} {}) to {}\n",
        shown::short(&packed.name),
        packed.files,
        purlis_core::counts::plural(packed.files, "file", "files"),
        shown::readable(&to.display().to_string(), PATH_LIMIT)
    );
    if !packed.left_out.is_empty() {
        out.push_str(&format!("not carried ({}):\n", packed.left_out.len()));
        for one in &packed.left_out {
            out.push_str(&format!(
                "  {}: {}\n",
                shown::readable(&one.key, 160),
                one.why
            ));
        }
    }
    print!("{out}");
    ExitCode::SUCCESS
}

/// Writes `bytes` to `to` whole or not at all: beside it first, then moved in. A file there is
/// replaced only with `force`, and never a link or a folder.
fn write_whole(to: &Path, bytes: &[u8], force: bool) -> Result<(), String> {
    use std::io::Write;
    match std::fs::symlink_metadata(to) {
        Ok(meta) if !meta.file_type().is_file() => {
            return Err(format!(
                "{} is there and is not a plain file: purlis does not replace it",
                to.display()
            ));
        }
        Ok(_) if !force => {
            return Err(format!(
                "{} is there already, and is replaced only with --force",
                to.display()
            ));
        }
        _ => {}
    }
    let name = to
        .file_name()
        .map(|it| it.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{} is not a file purlis can write", to.display()))?;
    let beside = to.with_file_name(format!(".{name}.writing-{}", std::process::id()));
    let failed = |why: std::io::Error| format!("{}: {why}", to.display());
    let written = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&beside)
        .and_then(|mut file| file.write_all(bytes).and_then(|()| file.sync_all()));
    if let Err(why) = written {
        let _ = std::fs::remove_file(&beside);
        return Err(failed(why));
    }
    // Without --force, the move itself refuses a file that appeared at `to` meanwhile.
    let moved = if force {
        std::fs::rename(&beside, to).map(|()| true)
    } else {
        theme::rename_no_replace(&beside, to)
    };
    match moved {
        Ok(true) => Ok(()),
        Ok(false) => {
            let _ = std::fs::remove_file(&beside);
            Err(format!(
                "{} is there already, and is replaced only with --force",
                to.display()
            ))
        }
        Err(why) => {
            let _ = std::fs::remove_file(&beside);
            Err(failed(why))
        }
    }
}

/// Where a theme command looks: the config home, and the plane it runs in, when it runs in one.
#[derive(Clone, Copy)]
struct Here<'a> {
    config: Option<&'a Path>,
    plane: Option<&'a Path>,
}

/// The plane this command runs in, as every command finds it; `None` outside one.
fn plane() -> Option<std::path::PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let place = purlis_core::plane::place(&cwd);
    place.is_plane.then_some(place.root)
}

/// Every theme there is here, in the order a person reads them: the built-in themes; the machine's
/// (the person's own, then their theme folders); the project's; each workspace's, by name; then
/// each approved extension's in the order the survey found them.
fn available(here: Here<'_>) -> Vec<Found> {
    let mut found = vec![Found::purlis(), Found::classic()];
    if let Some(config) = here.config {
        found.extend(mine(config));
    }
    found.extend(at_level(here, &Level::Machine));
    if let Some(plane) = here.plane {
        found.extend(at_level(here, &Level::Project));
        let workspaces = purlis_core::workspaces::Plane::open(plane)
            .workspaces()
            .unwrap_or_default();
        for name in workspaces {
            found.extend(at_level(here, &Level::Workspace(name)));
        }
    }
    if let Some(config) = here.config {
        found.extend(extensions(config));
    }
    found
}

/// **One level's choice** (#2707): the level of the precedence that made it, the workspace it
/// is made for (levels 1 and 2), the choice as the engine read it, placed at the level of the
/// folder it reads, and the id `list` names the theme it picks by.
struct Chose {
    level: theme::Level,
    workspace: Option<String>,
    choice: theme::Choice,
    /// `None` for a choice of a theme folder no level within its reach has.
    id: Option<String>,
}

impl Chose {
    /// Who chose, as the CHOSEN BY column and the check's `chosen by:` line say it.
    fn by(&self) -> String {
        let place = match &self.workspace {
            Some(name) => format!("{WORKSPACE} {}", shown::short(name)),
            None => match self.level {
                theme::Level::MachineDefault | theme::Level::Purlis => MACHINE.to_owned(),
                _ => PROJECT.to_owned(),
            },
        };
        let whose = match self.level {
            theme::Level::WorkspaceDefault | theme::Level::ProjectDefault => "committed",
            _ => "yours",
        };
        format!("{place}, {whose}")
    }

    /// Its option values as `<option>=<value>`, by the option's id; `None` with none.
    fn values_said(&self) -> Option<String> {
        (!self.choice.options.is_empty()).then(|| {
            self.choice
                .options
                .iter()
                .map(|(option, value)| format!("{option}={value}"))
                .collect::<Vec<_>>()
                .join(", ")
        })
    }

    /// As `list --json` (`choices`) and `check --json` (`chosen`) answer it.
    fn json(&self) -> Value {
        json!({
            "level": self.level,
            "workspace": self.workspace,
            "theme": self.choice.theme,
            "id": self.id,
            "from": self.choice.from.map(Reach::word),
            "appearance": self.choice.appearance,
            "options": self.choice.options,
        })
    }
}

/// **What every level chose here**, nearest first, as the engine reads the levels: each
/// workspace's (by name) person's choice and committed default, then the project's, then the
/// person's machine default. Outside a plane, the machine default alone.
fn every_choice(here: Here<'_>) -> Vec<Chose> {
    let mut chosen = Vec::new();
    let mut take = |levels: theme::Levels, workspace: Option<&str>, wanted: &[theme::Level]| {
        let at = match workspace {
            Some(name) => Level::Workspace(name.to_owned()),
            None => Level::Project,
        };
        let folders = folders(here, &at);
        for (level, choice) in levels.each() {
            if !wanted.contains(&level) {
                continue;
            }
            let choice = choice.clone().placed(&folders, level.reach());
            let id = id_of(here, &choice, level.reach(), workspace);
            chosen.push(Chose {
                level,
                workspace: workspace.map(str::to_owned),
                choice,
                id,
            });
        }
    };
    let Some(plane) = here.plane else {
        take(
            theme::Levels::read(here.config, None, None),
            None,
            &[theme::Level::MachineDefault],
        );
        return chosen;
    };
    let workspaces = purlis_core::workspaces::Plane::open(plane)
        .workspaces()
        .unwrap_or_default();
    for name in &workspaces {
        take(
            theme::Levels::read(here.config, Some(plane), Some(name)),
            Some(name),
            &[
                theme::Level::WorkspaceChoice,
                theme::Level::WorkspaceDefault,
            ],
        );
    }
    take(
        theme::Levels::read(here.config, Some(plane), None),
        None,
        &[
            theme::Level::ProjectChoice,
            theme::Level::ProjectDefault,
            theme::Level::MachineDefault,
        ],
    );
    chosen
}

/// The id `list` names the theme `choice` picks by, made at a level that reaches `reach` (in
/// `workspace`, for a workspace's): Purlis or Purlis Classic, an extension's
/// `<extension>/<theme>`, a theme folder's `<level>:<folder>` at the level it names, or the
/// person's own (`mine`) for the old `system` id when that has a dark and a light appearance.
/// `None` for a folder no level within reach has, or a level nearer than the choice reaches.
fn id_of(
    here: Here<'_>,
    choice: &theme::Choice,
    reach: Reach,
    workspace: Option<&str>,
) -> Option<String> {
    use extension::project::theme::Pick;
    Some(match Pick::parse(&choice.theme)? {
        Pick::BuiltIn(id) if id == theme::CLASSIC => theme::CLASSIC.to_owned(),
        Pick::BuiltIn(_) => theme::PURLIS.to_owned(),
        Pick::Extension { id, name } => format!("{id}/{name}"),
        Pick::System => {
            let theirs = theme::read_theirs(here.config).theme;
            let both = theirs.as_ref().is_some_and(|one| {
                one.of(theme::Appearance::Dark).is_some()
                    && one.of(theme::Appearance::Light).is_some()
            });
            if both { MINE } else { theme::PURLIS }.to_owned()
        }
        Pick::Folder(name) => {
            let from = choice.from.filter(|level| *level >= reach)?;
            let level = match from {
                Reach::Machine => Level::Machine,
                Reach::Project => Level::Project,
                Reach::Workspace => Level::Workspace(workspace?.to_owned()),
            };
            level.id(&name)
        }
    })
}

/// What every level chose, as a person reads it, after the themes: one row each, nearest
/// first, with the level of the folder it reads, its appearance and its option values (`-` for
/// none). Nothing when no level chose.
fn choices_table(chosen: &[Chose]) -> String {
    if chosen.is_empty() {
        return String::new();
    }
    let header = ["CHOSEN BY", "THEME", "FROM", "APPEARANCE", "OPTIONS"];
    let none = || "-".to_owned();
    let rows: Vec<[String; 5]> = chosen
        .iter()
        .map(|one| {
            [
                one.by(),
                shown::short(&one.choice.theme),
                one.choice.from.map_or_else(none, |it| it.word().to_owned()),
                one.choice
                    .appearance
                    .as_deref()
                    .map_or_else(none, shown::short),
                one.values_said()
                    .map_or_else(none, |it| shown::readable(&it, 240)),
            ]
        })
        .collect();
    let mut widths = header.map(str::len);
    for row in &rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.len());
        }
    }
    let line = |cells: [&str; 5]| {
        let padded: Vec<String> = cells
            .iter()
            .zip(widths)
            .map(|(cell, width)| format!("{cell:width$}"))
            .collect();
        format!("  {}\n", padded.join("  ").trim_end())
    };
    let mut out = "chosen, nearest first:\n".to_owned();
    out.push_str(&line(header));
    for row in &rows {
        out.push_str(&line([&row[0], &row[1], &row[2], &row[3], &row[4]]));
    }
    out
}

/// The levels' theme folders as the window reads them in front of `level`: the machine's alone,
/// the project's and the machine's, or a workspace's, the project's and the machine's.
fn folders(here: Here<'_>, level: &Level) -> Folders {
    match level {
        Level::Machine => Folders::of(here.config, None, None),
        Level::Project => Folders::of(here.config, here.plane, None),
        Level::Workspace(name) => Folders::of(here.config, here.plane, Some(name)),
    }
}

/// The theme folders of `level` itself, each read by the engine as a choice made there reads it.
fn at_level(here: Here<'_>, level: &Level) -> Vec<Found> {
    let folders = folders(here, level);
    folders
        .every_at(level.reach())
        .into_iter()
        .map(|(folder, resolved)| at_level_found(&folders, level, &folder, resolved))
        .collect()
}

/// The theme folder `folder` of `level` itself, read by the engine as a choice made there reads
/// it; or what reading the levels said when that level has no such folder.
fn one_at_level(here: Here<'_>, level: &Level, folder: &str) -> Result<Found, Vec<Complaint>> {
    let folders = folders(here, level);
    match folders.find_at(folder, level.reach()) {
        AtLevel::Missing(said) => Err(said),
        AtLevel::Read(resolved) => Ok(at_level_found(&folders, level, folder, resolved)),
    }
}

/// The `themes/` folder of `level` itself, when it has one.
fn themes_of<'a>(folders: &'a Folders, level: &Level) -> Option<&'a theme::levels::ThemesFolder> {
    match level {
        Level::Machine => folders.machine.as_ref(),
        Level::Project => folders.project.as_ref(),
        Level::Workspace(_) => folders.workspace.as_ref(),
    }
}

/// What the engine answered for the folder `folder` of `level`.
fn at_level_found(folders: &Folders, level: &Level, folder: &str, resolved: Resolved) -> Found {
    let path = themes_of(folders, level)
        .map(|it| it.path().join(folder).display().to_string())
        .unwrap_or_default();
    Found::of(
        level.id(folder),
        From::Level {
            level: level.clone(),
            folder: folder.to_owned(),
            path,
        },
        resolved,
    )
}

/// The person's own theme, when there is a file.
fn mine(config: &Path) -> Option<Found> {
    let theirs = theme::read_theirs(Some(config));
    if !theirs.found {
        return None;
    }
    let from = From::Mine(theirs.path);
    Some(match (theirs.trouble, theirs.theme) {
        (None, Some(theme)) => Found {
            id: MINE.to_owned(),
            from,
            theme: Some(theme),
            refused: None,
            errors: Vec::new(),
            notes: theirs.complaints,
        },
        // Refused, or not readable at all: the trouble is the window's own sentence, and the
        // complaints (when it was read) say each value.
        (trouble, _) => {
            let why = trouble.unwrap_or_else(|| "purlis cannot draw it".to_owned());
            let errors = if theirs.complaints.is_empty() {
                vec![Complaint {
                    said: why.clone(),
                    token: None,
                    awaits_approval: None,
                    at: None,
                }]
            } else {
                theirs.complaints
            };
            Found {
                id: MINE.to_owned(),
                from,
                theme: None,
                refused: Some(why),
                errors,
                notes: Vec::new(),
            }
        }
    })
}

/// The themes approved extensions that are on contribute, each resolved by the engine from the
/// text its fingerprint was taken over.
fn extensions(config: &Path) -> Vec<Found> {
    let seen = extension::survey(config, &extension::BuiltIn::of_this_program());
    let mut found = Vec::new();
    for row in &seen.installed {
        let Some(read) = &row.found else { continue };
        for one in row.themes_in_force() {
            let Some(text) = read.theme_text(one) else {
                continue;
            };
            let offered = theme::Offered::of(row.id.clone(), one.name.clone(), text);
            found.push(Found::of(
                format!("{}/{}", offered.extension, offered.name),
                From::Extension(offered.extension),
                offered.theme,
            ));
        }
    }
    found
}

/// The theme `asked` names: Purlis by its id or by `purlis-dark`, `purlis-light` or an id either
/// had (#3034), Purlis Classic by its id, the person's own, a level's theme folder by `<level>:<folder>`, an extension's by
/// `<extension>/<theme>`, or else a theme file or a theme folder by its path. `Err` when there
/// is none, with what reading a level had to say.
fn find(here: Here<'_>, asked: &str) -> Result<Found, Vec<Complaint>> {
    if asked == theme::CLASSIC {
        return Ok(Found::classic());
    }
    if theme::built_in(asked).is_some() {
        return Ok(Found::purlis());
    }
    // A level's id is never an extension's (it holds no `/`) nor a folder name: it is asked
    // before a path of the same spelling, which `./` names.
    if let Some((level, folder)) = Level::of_id(asked) {
        return one_at_level(here, &level, folder);
    }
    if let Some(folder) = asked.strip_prefix(&format!("{DRAFT}:")) {
        return this_chats_draft(here, folder);
    }
    if let Some(config) = here.config {
        if asked == MINE {
            return mine(config).ok_or_else(Vec::new);
        }
        if let Some(theirs) = extensions(config).into_iter().find(|one| one.id == asked) {
            return Ok(theirs);
        }
    }
    let path = Path::new(asked);
    let shown = path.display().to_string();
    if path.is_dir() {
        return Ok(read_folder(path, shown));
    }
    path.is_file()
        .then(|| {
            Found::of(
                asked.to_owned(),
                From::File(shown),
                read_file(path).unwrap_or_else(refusal),
            )
        })
        .ok_or_else(Vec::new)
}

/// **Who and where this chat is**, for its theme drafts (#2332): its id, which only a chat purlis
/// started has ([`purlis_core::hookwire::CHAT_ID_ENV`]), and the workspace of the folder purlis
/// started it in ([`purlis_core::sandboxblock::CHAT_DIR_ENV`]), else of this command's folder;
/// `None` at the plane root.
fn this_chat(plane: &Path) -> Result<(String, Option<String>), String> {
    let chat = purlis_core::envvar::var(purlis_core::hookwire::CHAT_ID_ENV)
        .filter(|id| theme::drafts::is_chat_id(id))
        .ok_or_else(|| {
            "a theme draft belongs to a chat purlis started, and this is not one: run it in \
             that chat"
                .to_owned()
        })?;
    let folder = purlis_core::envvar::var_os(purlis_core::sandboxblock::CHAT_DIR_ENV)
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok());
    let workspace =
        folder.and_then(|folder| purlis_core::active::workspace_of_tree(plane, &folder));
    Ok((chat, workspace))
}

/// Where this chat edits its theme draft `folder`: a working copy of its own, in the temporary
/// folder its sandbox lets it write (`$TMPDIR`), under this chat's id so no other chat's copy is
/// taken for it; or the folder `from` names.
fn working_copy(folder: &str, from: Option<&str>) -> std::path::PathBuf {
    from.map_or_else(
        || {
            let chat = purlis_core::envvar::var(purlis_core::hookwire::CHAT_ID_ENV)
                .filter(|id| theme::drafts::is_chat_id(id))
                .unwrap_or_else(|| "this-chat".to_owned());
            std::env::temp_dir()
                .join("purlis-theme-drafts")
                .join(chat)
                .join(folder)
        },
        std::path::PathBuf::from,
    )
}

/// The most a draft's working copy hands over: what a save takes.
const MOST_SAVED: usize = theme::drafts::MOST_SAVED_FILES;

/// The theme files of the working copy at `dir`, by their path in the folder: its `theme.json`
/// and each icon set under `icons/`, every one a plain file read refusing a link, under the cap
/// of a theme file. What a save hands over, and nothing else.
fn working_files(dir: &Path) -> Result<std::collections::BTreeMap<String, String>, String> {
    let most = purlis_core::windowprefs::MAX_BYTES;
    let read = |path: &Path| -> Result<String, String> {
        use std::io::Read;
        let failed = |why: std::io::Error| format!("{} could not be read: {why}", path.display());
        let mut open = purlis_core::contain::open_no_link(dir, path).map_err(failed)?;
        if !open.metadata().map_err(failed)?.file_type().is_file() {
            return Err(format!("{} is not a plain file", path.display()));
        }
        let mut text = String::new();
        (&mut open)
            .take(most + 1)
            .read_to_string(&mut text)
            .map_err(failed)?;
        if text.len() as u64 > most {
            return Err(format!(
                "{} is larger than {most} bytes, the most a theme file is",
                path.display()
            ));
        }
        Ok(text)
    };
    let mut files = std::collections::BTreeMap::new();
    files.insert(
        theme::THEME_JSON.to_owned(),
        read(&dir.join(theme::THEME_JSON))?,
    );
    let icons = dir.join("icons");
    if let Ok(listed) = std::fs::read_dir(&icons) {
        let mut names: Vec<String> = listed
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
            .filter(|name| name.ends_with(".json") && !name.starts_with('.'))
            .collect();
        names.sort();
        for name in names.into_iter().take(MOST_SAVED - 1) {
            files.insert(format!("icons/{name}"), read(&icons.join(&name))?);
        }
    }
    Ok(files)
}

/// **What a draft ask's answer says was drafted**: its lines, its exit status, and whether the
/// draft was made now. An app that answers [`Answer::Drafted`](purlis_core::hookwire::Answer)
/// says so in a field; an older one answers `Said`, and its draft made now is the one said as
/// done, as such an app said it (#2708). `None` for any other answer.
fn drafted(
    answer: purlis_core::hookwire::Answer,
) -> Option<(Vec<purlis_core::repocmd::Say>, u8, bool)> {
    match answer {
        purlis_core::hookwire::Answer::Drafted { lines, code, made } => Some((lines, code, made)),
        purlis_core::hookwire::Answer::Said { lines, code } => {
            let made = lines
                .iter()
                .any(|line| matches!(line, purlis_core::repocmd::Say::Done(_)));
            Some((lines, code, made))
        }
        _ => None,
    }
}

/// `purlis theme draft <folder>`: **the app writes this chat's draft**, in the drafts folder of
/// the chat whose token asks, and says what the window draws of it. Without `--save` it starts
/// the draft and makes a working copy of it here; with `--save` it hands the working copy's theme
/// files over. No chat writes the drafts folder, and this command never does: outside a chat the
/// app started, with its connection, there is no draft to write.
fn draft(here: Here<'_>, folder: &str, save: bool, from: Option<&str>) -> ExitCode {
    let refused = |why: &str| {
        eprintln!("purlis: {}", shown::readable(why, 480));
        ExitCode::from(1)
    };
    let Some(plane) = here.plane else {
        return refused("a theme draft is a project's: run it inside one");
    };
    let working = working_copy(folder, from);
    let files = if save {
        match working_files(&working) {
            // Checked before it is sent: what one ask carries is bounded.
            Ok(files) => match theme::drafts::too_large(&files) {
                Some(why) => return refused(&format!("{why}. Nothing was handed over")),
                None => Some(files),
            },
            Err(why) => return refused(&why),
        }
    } else {
        None
    };
    let answer = crate::brokered::asked(
        |chat| {
            purlis_core::hookwire::Ask::ThemeDraft(Box::new(purlis_core::hookwire::ThemeDraftAsk {
                chat,
                folder: folder.to_owned(),
                files,
            }))
        },
        std::time::Duration::from_secs(30),
    );
    let (code, newly) = match answer {
        Ok(Ok(
            answer @ (purlis_core::hookwire::Answer::Drafted { .. }
            | purlis_core::hookwire::Answer::Said { .. }),
        )) => {
            // A draft made now: its working copy is made again from it.
            let (lines, code, made) = drafted(answer).unwrap_or_default();
            for line in lines {
                crate::speak(line);
            }
            (code, made)
        }
        // The app's own refusal, as it said it.
        Ok(Ok(purlis_core::hookwire::Answer::No { why }))
            if why != purlis_core::hookwire::NOTHING_ANSWERS =>
        {
            return refused(&why);
        }
        Ok(Err(e)) if !crate::brokered::not_taken(&e) => {
            return refused(&format!(
                "the app took this and did not answer ({e}), so the draft may have been \
                 written: `purlis theme check {DRAFT}:{folder}` says what is there"
            ));
        }
        _ => {
            return refused(
                "a theme draft is written by the app that started this chat, and no app took \
                 this, so nothing was written: run it in a chat purlis started",
            );
        }
    };
    if !save && code == 0 {
        // The working copy, made from the draft as the app wrote it: when there is none, and
        // whenever the draft was made now, so the edits of a draft kept or reverted never come
        // back with the next save.
        if newly && from.is_none() && std::fs::symlink_metadata(&working).is_ok() {
            let _ = std::fs::remove_dir_all(&working);
        }
        if std::fs::symlink_metadata(&working).is_err() {
            let made = this_chat(plane)
                .ok()
                .and_then(|(chat, _)| theme::drafts::folder_of(plane, &chat))
                .map(|drafts| drafts.join(folder))
                .ok_or_else(|| "this chat's draft could not be found".to_owned())
                .and_then(|draft| working_files(&draft))
                .and_then(|files| {
                    for (path, text) in &files {
                        let at = working.join(path);
                        if let Some(parent) = at.parent() {
                            std::fs::create_dir_all(parent).map_err(|why| why.to_string())?;
                        }
                        std::fs::write(&at, text).map_err(|why| why.to_string())?;
                    }
                    Ok(())
                });
            if let Err(why) = made {
                return refused(&format!(
                    "the draft is started, and its working copy was not made: {why}"
                ));
            }
        }
        println!(
            "Edit {} there, then `purlis theme draft {folder} --save` hands it to the app. \
             `purlis theme check {DRAFT}:{folder}` checks the draft as the window draws it.",
            working.join(theme::THEME_JSON).display()
        );
    }
    ExitCode::from(code)
}

/// This chat's theme draft `folder`, read only to check, exactly as the window draws it.
fn this_chats_draft(here: Here<'_>, folder: &str) -> Result<Found, Vec<Complaint>> {
    let said = |why: String| {
        vec![Complaint {
            said: why,
            token: None,
            awaits_approval: None,
            at: None,
        }]
    };
    let plane = here
        .plane
        .ok_or_else(|| said("a theme draft is a project's: run it inside one".to_owned()))?;
    let (chat, workspace) = this_chat(plane).map_err(said)?;
    let place = theme::drafts::Place {
        plane,
        workspace: workspace.as_deref(),
    };
    let folders = Folders::of(here.config, Some(plane), workspace.as_deref());
    let draft = theme::drafts::Draft {
        chat: chat.clone(),
        folder: folder.to_owned(),
        kept_to: theme::drafts::kept_to(&folders, workspace.as_deref(), folder),
    };
    let resolved = theme::drafts::read(here.config, place, &draft, true)?;
    let path = theme::drafts::folder_of(plane, &chat)
        .map(|it| it.join(folder).display().to_string())
        .unwrap_or_default();
    Ok(Found::of(
        format!("{DRAFT}:{folder}"),
        From::Draft {
            folder: folder.to_owned(),
            path,
        },
        resolved,
    ))
}

/// The engine's refusal of a theme for `why`, the default, Purlis, in its place.
fn refusal(why: String) -> Resolved {
    Resolved {
        theme: theme::purlis().clone(),
        complaints: vec![Complaint {
            said: why,
            token: None,
            awaits_approval: None,
            at: None,
        }],
        refused: true,
    }
}

/// A theme file named on the command line, read under the one cap every theme file is read
/// under and handed to the engine as text, which judges the rest.
///
/// A link is followed, on purpose: this is a path the person typed, and checking the file it
/// leads to is what they asked for. The person's own `theme.json`, which purlis reads unasked
/// at every launch, is read link-refusing (`windowprefs::read_theme_text`).
fn read_file(path: &Path) -> Result<Resolved, String> {
    use std::io::Read;
    let most = purlis_core::windowprefs::MAX_BYTES;
    let mut text = String::new();
    let read = std::fs::File::open(path)
        .and_then(|file| file.take(most + 1).read_to_string(&mut text))
        .map_err(|why| format!("{} could not be read: {why}", path.display()))?;
    if read as u64 > most {
        return Err(format!(
            "{} is larger than {} KiB, the most a theme file may be",
            path.display(),
            most / 1024
        ));
    }
    Ok(theme::load_text(&text))
}

/// A theme folder named on the command line, opened by the engine from the folder that holds
/// it, refusing a link ([`theme::Named`]): its `theme.json`, its icon set, its fonts and the
/// folders beside it its `extends` chain names are all read from those handles.
fn read_folder(path: &Path, shown: String) -> Found {
    let from = From::Folder(shown);
    if path.file_name().and_then(|it| it.to_str()).is_none() {
        return Found::of(
            path.display().to_string(),
            from,
            refusal(format!(
                "{} is not a theme folder purlis can name",
                path.display()
            )),
        );
    }
    let named = theme::Named::open(path);
    Found::of(named.name().to_owned(), from, named.load())
}

/// A theme's appearances as the table says them: each id, with its tag when the id is not it,
/// and `high contrast` when it is tagged so.
fn appearances(theme: &Theme) -> String {
    theme
        .appearances
        .iter()
        .map(|one| {
            let tag = appearance(one.appearance);
            match (one.id == tag, one.high_contrast) {
                (true, false) => one.id.clone(),
                (true, true) => format!("{} (high contrast)", one.id),
                (false, false) => format!("{} ({tag})", one.id),
                (false, true) => format!("{} ({tag}, high contrast)", one.id),
            }
        })
        .map(|it| shown::short(&it))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The listing: one row per theme, its columns sized from its cells, and the refused ones
/// after it with why.
fn table(found: &[Found]) -> String {
    let header = ["ID", "NAME", "APPEARANCES", "FROM"];
    let mut rows: Vec<[String; 4]> = Vec::new();
    let mut refused = Vec::new();
    for one in found {
        match (&one.theme, &one.refused) {
            (Some(theme), None) => rows.push([
                shown::short(&one.id),
                one.name_said(theme),
                appearances(theme),
                one.from.said(),
            ]),
            (_, why) => refused.push(format!(
                "  {} ({}): {}",
                shown::short(&one.id),
                one.from.said(),
                shown::short(why.as_deref().unwrap_or_default())
            )),
        }
    }
    let mut widths = header.map(str::len);
    for row in &rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.len());
        }
    }
    let line = |cells: [&str; 4]| {
        let padded: Vec<String> = cells
            .iter()
            .zip(widths)
            .map(|(cell, width)| format!("{cell:width$}"))
            .collect();
        format!("  {}\n", padded.join("  ").trim_end())
    };
    let mut out = line(header);
    for row in &rows {
        out.push_str(&line([&row[0], &row[1], &row[2], &row[3]]));
    }
    if !refused.is_empty() {
        out.push_str("refused:\n");
        for why in refused {
            out.push_str(&why);
            out.push('\n');
        }
    }
    out
}

fn appearance(appearance: theme::Appearance) -> &'static str {
    match appearance {
        theme::Appearance::Dark => "dark",
        theme::Appearance::Light => "light",
    }
}

/// The check of every combination of appearance and option values, in the engine's order. A
/// refused theme has none.
fn checks(found: &Found) -> Vec<theme::Checked> {
    found
        .theme
        .as_ref()
        .map_or_else(Vec::new, theme::check_every)
}

/// The option values a combination was drawn with, in the theme's order of its options.
fn in_theme_order<'a>(
    theme: &'a Theme,
    checked: &'a theme::Checked,
) -> impl Iterator<Item = (&'a str, &'a str)> {
    theme.options.iter().filter_map(|option| {
        checked
            .options
            .get(&option.id)
            .map(|value| (option.id.as_str(), value.as_str()))
    })
}

/// The theme's options as the check's header says them: each id, its kind, its values and its
/// default.
fn options_line(theme: &Theme) -> Option<String> {
    if theme.options.is_empty() {
        return None;
    }
    let each: Vec<String> = theme
        .options
        .iter()
        .map(|one| match one.kind {
            theme::Kind::Toggle => format!("{} (a toggle, {} by default)", one.id, one.default),
            theme::Kind::Choice => {
                let values: Vec<&str> = one.values.iter().map(|it| it.id.as_str()).collect();
                format!(
                    "{} (one of {}; {} by default)",
                    one.id,
                    values.join(", "),
                    one.default
                )
            }
        })
        .collect();
    Some(format!(
        "options: {} — {} combinations checked\n",
        shown::readable(&each.join("; "), 480),
        theme.combinations()
    ))
}

/// The check as a person reads it.
/// `entry` names the palette entry holding a terminal colour a warning is about, for an import.
fn checked_text(
    found: &Found,
    checked: &[theme::Checked],
    chosen: &[Chose],
    entry: &dyn Fn(&str, &str) -> Option<String>,
) -> String {
    let mut out = String::new();
    let Some(theme) = &found.theme else {
        // The person's own is the one the window would have drawn, and a level's folder is
        // what a choice there names: it draws Purlis instead.
        let instead = match found.from {
            From::Mine(_) => " — the window draws Purlis in its place",
            From::Level { .. } => " — the window draws Purlis in its place when it is chosen",
            _ => "",
        };
        out.push_str(&format!(
            "{} — {}\nrefused{instead}:\n",
            shown::short(&found.id),
            found.from.said(),
        ));
        for error in &found.errors {
            out.push_str(&format!("  {}\n", shown::readable(&error.said, 240)));
        }
        return out;
    };
    out.push_str(&format!(
        "{} — {}, {}\n",
        shown::short(&theme.name),
        found.from.said(),
        appearances(theme)
    ));
    if let Some(line) = options_line(theme) {
        out.push_str(&line);
    }
    if !chosen.is_empty() {
        let each: Vec<String> = chosen
            .iter()
            .map(|one| match one.values_said() {
                Some(values) => format!("{} ({values})", one.by()),
                None => one.by(),
            })
            .collect();
        out.push_str(&format!(
            "chosen by: {}\n",
            shown::readable(&each.join("; "), 480)
        ));
    }
    if found.notes.is_empty() {
        out.push_str("left out: nothing\n");
    } else {
        out.push_str(&format!("left out ({}):\n", found.notes.len()));
        for note in &found.notes {
            out.push_str(&format!("  {}\n", shown::readable(&note.said, 240)));
        }
    }
    let said = theme::warnings_said_naming(theme, checked, entry);
    if said.is_empty() {
        out.push_str("warnings: none\n");
    } else {
        out.push_str(&format!("warnings ({}):\n", said.len()));
        for line in said {
            out.push_str(&format!("  {line}\n"));
        }
    }
    // A combination is named on its lines only when there is more than one to tell apart.
    let named = checked.len() > 1;
    let held: Vec<String> = checked
        .iter()
        .filter_map(|one| {
            let held = one.held.as_ref()?;
            let floors: Vec<&str> = held.floors.iter().map(|floor| floor.says()).collect();
            let line = format!(
                "{} — purlis draws its own {}",
                floors.join("; "),
                held.tokens.join(", ")
            );
            Some(if named {
                format!("{}: {line}", theme::combination_said(theme, one))
            } else {
                line
            })
        })
        .collect();
    // Every combination is checked against the consent floors, as the window draws it: with
    // more than one, the line says how many were, and in how many the window holds one.
    let combinations = checked.len();
    let combinations = format!(
        "{combinations} {}",
        purlis_core::counts::plural(combinations, "combination", "combinations")
    );
    match (held.is_empty(), named) {
        (true, true) => {
            out.push_str(&format!("consent floors: none held in {combinations}\n"));
        }
        (true, false) => out.push_str("consent floors: none held\n"),
        (false, true) => out.push_str(&format!(
            "consent floors: held in {} of {combinations}:\n",
            held.len()
        )),
        (false, false) => out.push_str("consent floors: held:\n"),
    }
    // The floors' words and the tokens are the engine's own; the combination is shown.
    for line in held {
        out.push_str(&format!("  {line}\n"));
    }
    out.push_str(&hard_floors_text(theme, checked, &combinations, named));
    if matches!(found.from, From::Draft { .. }) && checked.iter().any(|one| !one.hard.is_empty()) {
        out.push_str(
            "not drawn: the window keeps this draft's last good version until it holds every \
             hard floor\n",
        );
    }
    out
}

/// The check's `hard floors:` lines (#2332): every hard floor each combination breaks as the
/// window draws it, in the engine's words, or that none is broken.
fn hard_floors_text(
    theme: &Theme,
    checked: &[theme::Checked],
    combinations: &str,
    named: bool,
) -> String {
    let lines: Vec<String> = checked
        .iter()
        .flat_map(|one| {
            one.hard.iter().map(move |broken| {
                if named {
                    format!("{}: {}", theme::combination_said(theme, one), broken.said())
                } else {
                    broken.said()
                }
            })
        })
        .collect();
    let breaking = checked.iter().filter(|one| !one.hard.is_empty()).count();
    let mut out = match (lines.is_empty(), named) {
        (true, true) => format!("hard floors: none broken in {combinations}\n"),
        (true, false) => "hard floors: none broken\n".to_owned(),
        (false, true) => format!("hard floors: broken in {breaking} of {combinations}:\n"),
        (false, false) => "hard floors: broken:\n".to_owned(),
    };
    for line in lines {
        out.push_str(&format!("  {line}\n"));
    }
    out
}

/// The check as a chat reads it: the theme, its appearances and options, why it is refused (or
/// `null`), what refused it (`errors`), what was left out of it (`notes`), each combination's
/// warnings as the engine reports them with the appearance's id and the option values added, the
/// consent floors the window holds in each combination that breaks one (`held`), its hard
/// floors (each one broken in each combination), and whether it is clean: no error, no note, no
/// warning and no floor held. `clean` does not cover hard floors, which are reported on their
/// own.
fn checked_json(found: &Found, checked: &[theme::Checked]) -> Value {
    let summary = found.summary_json();
    let errors = serde_json::to_value(&found.errors).unwrap_or_default();
    let notes = serde_json::to_value(&found.notes).unwrap_or_default();
    // The option values of a combination in the theme's order of its options.
    let options_of = |one: &theme::Checked| -> serde_json::Map<String, Value> {
        found
            .theme
            .as_ref()
            .map(|theme| {
                in_theme_order(theme, one)
                    .map(|(option, value)| (option.to_owned(), Value::from(value)))
                    .collect()
            })
            .unwrap_or_default()
    };
    let warnings: Vec<Value> = checked
        .iter()
        .flat_map(|one| {
            let options = options_of(one);
            one.report.findings.iter().map(move |finding| {
                let mut said = serde_json::to_value(finding).unwrap_or_default();
                if let Some(fields) = said.as_object_mut() {
                    fields.insert("appearance".to_owned(), Value::from(one.appearance.clone()));
                    fields.insert("options".to_owned(), Value::Object(options.clone()));
                }
                said
            })
        })
        .collect();
    let held: Vec<Value> = checked
        .iter()
        .filter_map(|one| {
            let held = one.held.as_ref()?;
            Some(json!({
                "appearance": one.appearance,
                "options": options_of(one),
                "floors": held.floors,
                "tokens": held.tokens,
            }))
        })
        .collect();
    let hard_floors: Vec<Value> = checked
        .iter()
        .flat_map(|one| {
            let options = options_of(one);
            one.hard.iter().map(move |broken| {
                let mut said = serde_json::to_value(broken).unwrap_or_default();
                if let Some(fields) = said.as_object_mut() {
                    fields.insert("appearance".to_owned(), Value::from(one.appearance.clone()));
                    fields.insert("options".to_owned(), Value::Object(options.clone()));
                }
                said
            })
        })
        .collect();
    let clean =
        found.refused.is_none() && found.notes.is_empty() && warnings.is_empty() && held.is_empty();
    json!({
        "schema": SCHEMA,
        "theme_schema": theme::SCHEMA_URL,
        "theme": {
            "id": summary["id"],
            "name": summary["name"],
            "appearances": summary["appearances"],
            "options": summary["options"],
            "combinations": summary["combinations"],
            "level": summary["level"],
            "from": summary["from"],
        },
        "refused": summary["refused"],
        "errors": errors,
        "notes": notes,
        "warnings": warnings,
        "held": held,
        "hard_floors": hard_floors,
        "clean": clean,
    })
}

#[cfg(test)]
mod drafted_tests {
    use purlis_core::hookwire::Answer;
    use purlis_core::repocmd::Say;

    #[test]
    fn a_draft_answer_says_whether_it_made_the_draft_in_its_field() {
        let said = Answer::Drafted {
            lines: vec![Say::Done("saved".to_owned().into())],
            code: 0,
            made: false,
        };
        assert_eq!(
            super::drafted(said).map(|(_, code, made)| (code, made)),
            Some((0, false))
        );
    }

    #[test]
    fn an_older_app_s_answer_is_read_as_that_app_said_it_and_never_as_nothing_written() {
        // An older app answers `Said`: the draft it wrote is printed and its exit kept, and a
        // draft made now is the one it said as done.
        let made = Answer::Said {
            lines: vec![Say::Done(
                "The theme draft dusk is started".to_owned().into(),
            )],
            code: 0,
        };
        assert_eq!(
            super::drafted(made).map(|(_, code, made)| (code, made)),
            Some((0, true))
        );
        let there = Answer::Said {
            lines: vec![Say::Info("The theme draft dusk is there".to_owned().into())],
            code: 0,
        };
        assert_eq!(super::drafted(there).map(|(_, _, made)| made), Some(false));
        assert_eq!(
            super::drafted(Answer::No {
                why: "no".to_owned()
            }),
            None
        );
    }
}
