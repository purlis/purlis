// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis theme list` and `purlis theme check` (spec #2313, #2321): every theme available on
//! this machine and where it comes from, and the theme check of any one of them.
//!
//! **Nothing here judges a theme.** The theme engine (`purlis_core::theme`) reads it, refuses it
//! or says what it left out, and checks each appearance; this module finds the themes there are,
//! asks the engine about each, and prints its answer, as a table for a person or as JSON for a
//! chat. Nothing is written: a theme command reads Purlis Classic, the person's `theme.json`
//! and the approved extensions' themes, and leaves every file as it found it.
//!
//! **Where a theme comes from, today:** built in (Purlis Classic, `purlis-classic`), the
//! person's own `theme.json` (its id is [`MINE`]), or an approved extension that is on (its id
//! is `<extension>/<theme>`, as a project's pick names it). A theme file or a theme folder
//! anywhere else can be checked by its path. The levels the spec adds (machine, project and
//! workspace theme folders) are more sources in the same list, with the same `from`.
//!
//! **The exit is a linter's.** `check` exits 0 when the theme is drawn, notes and warnings or
//! not (a contrast finding is the author's call, ADR 0097 §2); 1 when it is refused; 2 when
//! there is no such theme here.

use std::path::Path;
use std::process::ExitCode;

use clap::Subcommand;
use purlis_core::theme::{self, Complaint, Finding, Resolved, Theme};
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

/// `purlis theme …`.
#[derive(Subcommand)]
pub enum ThemeCommand {
    /// Every theme available here, with its id, its appearances and where it comes from:
    /// built in (`purlis-classic`), your own theme.json (`mine`), or an approved extension
    /// (`<extension>/<theme>`). A theme purlis refuses is listed as refused, with why.
    List {
        /// Answer as JSON, for a chat or a script.
        #[arg(long)]
        json: bool,
    },
    /// The theme check of one theme: why it is refused, if it is; what purlis left out of it;
    /// its contrast warnings in every combination of its appearances and option values (each
    /// pair with its ratio and its floor, as it is and in every workspace tint); and its hard
    /// floors. A theme with more than 64 combinations is refused with its count. Exits 0 when
    /// the theme is drawn, 1 when it is refused, 2 when there is no such theme.
    Check {
        /// A theme's id from `purlis theme list`, or the path of a theme file or a theme
        /// folder. An id may hold a space (`solarized/Solarized Dark`): quote it.
        theme: String,
        /// Answer as JSON, for a chat or a script. Its `clean` is true with no error, no note
        /// and no warning; it does not cover hard floors, which `hard_floors` reports on its
        /// own (`null` until purlis checks them).
        #[arg(long)]
        json: bool,
    },
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
}

impl From {
    /// As the table's FROM column says it.
    fn said(&self) -> String {
        match self {
            Self::BuiltIn => "built-in".to_owned(),
            Self::Mine(path) | Self::File(path) | Self::Folder(path) => shown::short(path),
            Self::Extension(id) => format!("extension {}", shown::short(id)),
        }
    }

    fn json(&self) -> Value {
        match self {
            Self::BuiltIn => json!({ "kind": "built-in" }),
            Self::Mine(path) => json!({ "kind": "mine", "path": path }),
            Self::Extension(id) => json!({ "kind": "extension", "extension": id }),
            Self::File(path) => json!({ "kind": "file", "path": path }),
            Self::Folder(path) => json!({ "kind": "folder", "path": path }),
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
        Self::of(
            theme::CLASSIC.to_owned(),
            From::BuiltIn,
            Resolved {
                theme: theme::classic().clone(),
                complaints: Vec::new(),
                refused: false,
            },
        )
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
        json!({
            "id": self.id,
            "name": name,
            "appearances": appearances,
            "options": options,
            "combinations": combinations,
            "from": self.from.json(),
            "refused": self.refused,
        })
    }
}

/// `purlis theme …`, run.
pub fn run(command: &ThemeCommand) -> ExitCode {
    let config = purlis_core::machine::config_root();
    match command {
        ThemeCommand::List { json } => {
            let found = available(config.as_deref());
            if *json {
                let themes: Vec<Value> = found.iter().map(Found::summary_json).collect();
                println!(
                    "{}",
                    json!({
                        "schema": SCHEMA,
                        "theme_schema": theme::SCHEMA_URL,
                        "themes": themes
                    })
                );
            } else {
                print!("{}", table(&found));
            }
            ExitCode::SUCCESS
        }
        ThemeCommand::Check { theme, json } => {
            let Some(found) = find(config.as_deref(), theme) else {
                eprintln!(
                    "purlis: there is no theme '{}' here — `purlis theme list` lists the themes \
                     there are, and `purlis theme check <path>` checks a theme file or folder",
                    shown::short(theme)
                );
                return ExitCode::from(2);
            };
            let checked = checks(&found);
            if *json {
                println!("{}", checked_json(&found, &checked));
            } else {
                print!("{}", checked_text(&found, &checked));
            }
            ExitCode::from(u8::from(found.refused.is_some()))
        }
    }
}

/// Every theme there is here, in the order a person reads them: Purlis Classic, the person's
/// own, then each approved extension's in the order the survey found them.
fn available(config: Option<&Path>) -> Vec<Found> {
    let mut found = vec![Found::classic()];
    let Some(config) = config else {
        return found;
    };
    found.extend(mine(config));
    found.extend(extensions(config));
    found
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

/// The theme `asked` names: Purlis Classic by its id or any id its appearances have had, the
/// person's own, an extension's by `<extension>/<theme>`, or else a theme file or a theme
/// folder by its path.
fn find(config: Option<&Path>, asked: &str) -> Option<Found> {
    if asked == theme::CLASSIC || theme::built_in(asked).is_some() {
        return Some(Found::classic());
    }
    if let Some(config) = config {
        if asked == MINE {
            return mine(config);
        }
        if let Some(theirs) = extensions(config).into_iter().find(|one| one.id == asked) {
            return Some(theirs);
        }
    }
    let path = Path::new(asked);
    let shown = path.display().to_string();
    if path.is_dir() {
        return Some(read_folder(path, shown));
    }
    path.is_file().then(|| {
        Found::of(
            asked.to_owned(),
            From::File(shown),
            read_file(path).unwrap_or_else(refusal),
        )
    })
}

/// The engine's refusal of a theme for `why`, Purlis in its place.
fn refusal(why: String) -> Resolved {
    Resolved {
        theme: theme::classic().clone(),
        complaints: vec![Complaint {
            said: why,
            token: None,
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

/// A theme folder named on the command line, read by the engine inside the folder that holds
/// it, as a level's `themes/` folder is. The library is handed only the folders its `extends`
/// chain names, so nothing else beside it is read. The engine refuses a folder or a
/// `theme.json` that is a link.
fn read_folder(path: &Path, shown: String) -> Found {
    let from = From::Folder(shown);
    let (Some(themes), Some(name)) = (path.parent(), path.file_name().and_then(|it| it.to_str()))
    else {
        return Found::of(
            path.display().to_string(),
            from,
            refusal(format!(
                "{} is not a theme folder purlis can name",
                path.display()
            )),
        );
    };
    let themes = if themes.as_os_str().is_empty() {
        Path::new(".")
    } else {
        themes
    };
    let mut library = theme::Library::new();
    for parent in extends_chain(themes, name) {
        library = library.with_folder(themes, &parent);
    }
    Found::of(name.to_owned(), from, library.load_folder(themes, name))
}

/// The most parents a folder's `extends` chain is followed through: the engine says a circle
/// or a missing parent itself, so this only bounds the walk.
const MOST_PARENTS: usize = 16;

/// The folders beside `name` in `themes` that its `extends` chain names, nearest first: only
/// what the engine needs to resolve it. A parent that is not a plain folder there (Purlis
/// Classic, a missing one, a link) ends the walk, and the engine says what is wrong with it.
fn extends_chain(themes: &Path, name: &str) -> Vec<String> {
    use std::io::Read;
    let mut chain: Vec<String> = Vec::new();
    let mut at = name.to_owned();
    while chain.len() < MOST_PARENTS {
        let file = themes.join(&at).join(theme::THEME_JSON);
        let plain = std::fs::symlink_metadata(&file).is_ok_and(|it| it.file_type().is_file());
        let mut text = String::new();
        let read = plain
            && std::fs::File::open(&file)
                .and_then(|it| {
                    it.take(purlis_core::windowprefs::MAX_BYTES)
                        .read_to_string(&mut text)
                })
                .is_ok();
        let parent = read
            .then(|| serde_json::from_str::<Value>(&text).ok())
            .flatten()
            .and_then(|it| it.get("extends").and_then(Value::as_str).map(str::to_owned));
        let Some(parent) = parent else { break };
        let folder = std::fs::symlink_metadata(themes.join(&parent));
        if parent == name
            || chain.contains(&parent)
            || !folder.is_ok_and(|it| it.file_type().is_dir())
        {
            break;
        }
        chain.push(parent.clone());
        at = parent;
    }
    chain
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
                shown::short(&theme.name),
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

/// What the check says about hard floors until the engine keeps them (#2332): not a pass.
const FLOORS_NOT_CHECKED: &str = "not checked by this purlis yet";

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

/// What a combination is called on a warning's line: its appearance, and each option's value
/// in the theme's order when the theme has options (`dark, borders on, accent teal`).
fn combination(theme: &Theme, checked: &theme::Checked) -> String {
    let mut parts = vec![checked.appearance.clone()];
    parts.extend(in_theme_order(theme, checked).map(|(option, value)| format!("{option} {value}")));
    shown::short(&parts.join(", "))
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
fn checked_text(found: &Found, checked: &[theme::Checked]) -> String {
    let mut out = String::new();
    let Some(theme) = &found.theme else {
        // The person's own is the one the window would have drawn: it draws Purlis instead.
        let instead = match found.from {
            From::Mine(_) => " — the window draws Purlis in its place",
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
    if found.notes.is_empty() {
        out.push_str("left out: nothing\n");
    } else {
        out.push_str(&format!("left out ({}):\n", found.notes.len()));
        for note in &found.notes {
            out.push_str(&format!("  {}\n", shown::readable(&note.said, 240)));
        }
    }
    // A combination is named on its lines only when there is more than one to tell apart.
    let named = checked.len() > 1;
    let said: Vec<String> = checked
        .iter()
        .flat_map(|one| {
            let called = combination(theme, one);
            warnings(&one.report.findings)
                .into_iter()
                .map(move |warning| {
                    if named {
                        format!("{called}: {}", warning.line())
                    } else {
                        warning.line()
                    }
                })
        })
        .collect();
    if said.is_empty() {
        out.push_str("warnings: none\n");
    } else {
        out.push_str(&format!("warnings ({}):\n", said.len()));
        for line in said {
            out.push_str(&format!("  {line}\n"));
        }
    }
    out.push_str(&format!("hard floors: {FLOORS_NOT_CHECKED}\n"));
    out
}

/// The findings of one pair (or one terminal colour, or the selected row against one other),
/// as the theme is and in each workspace tint it was found in: one line of the text.
struct Warning {
    /// What is drawn on what.
    subject: String,
    /// The floor it is under; `None` for the selected row, which has no ratio.
    floor: Option<f64>,
    /// Found in the theme as it is, with its ratio when it has one.
    plain: Option<Option<f64>>,
    /// Each workspace tint it was found in, with its ratio when it has one.
    tints: Vec<(&'static str, Option<f64>)>,
}

impl Warning {
    fn line(&self) -> String {
        let lowest = self
            .tints
            .iter()
            .filter_map(|(_, ratio)| *ratio)
            .fold(f64::INFINITY, f64::min);
        let tints = if self.tints.len() == theme::PALETTE.len() {
            "every workspace tint".to_owned()
        } else {
            let names: Vec<&str> = self.tints.iter().map(|(name, _)| *name).collect();
            format!(
                "the {} {}",
                names.join(", "),
                purlis_core::counts::plural(names.len(), "tint", "tints")
            )
        };
        let floor = self
            .floor
            .map_or_else(String::new, |floor| format!(", under {floor}"));
        match (self.plain, self.tints.is_empty()) {
            (Some(Some(ratio)), true) => format!("{}: {ratio:.2}{floor}", self.subject),
            (Some(Some(ratio)), false) => format!(
                "{}: {ratio:.2}{floor}; in {tints} too, down to {lowest:.2}",
                self.subject
            ),
            (Some(None), true) => self.subject.clone(),
            (Some(None), false) => format!("{}; in {tints} too", self.subject),
            (None, _) if lowest.is_finite() => {
                format!("{}, in {tints}: down to {lowest:.2}{floor}", self.subject)
            }
            (None, _) => format!("{}, in {tints}", self.subject),
        }
    }
}

/// The findings as a person reads them: one line per pair, the theme as it is first and the
/// tints after, in the engine's order. `--json` keeps every finding as the engine reports it.
fn warnings(findings: &[Finding]) -> Vec<Warning> {
    let mut found: Vec<Warning> = Vec::new();
    for finding in findings {
        let (subject, floor, tint, ratio) = match finding {
            Finding::Contrast { tint, pair, ratio } => {
                let over = pair
                    .under
                    .map_or_else(String::new, |under| format!(" over {under}"));
                (
                    format!("{} on {}{over}", pair.front, pair.back),
                    Some(pair.floor),
                    *tint,
                    Some(*ratio),
                )
            }
            Finding::Ansi {
                tint,
                token,
                ratio,
                floor,
            } => (
                format!("{token} on terminal.background"),
                Some(*floor),
                *tint,
                Some(*ratio),
            ),
            Finding::SelectedNotApart { tint, other } => (
                format!("list.selected is drawn exactly as {other}"),
                None,
                *tint,
                None,
            ),
            Finding::SelectionTextAlone => (
                "selection.text is a colour while selection.background is \"platform\", so it \
                 is never drawn"
                    .to_owned(),
                None,
                None,
                None,
            ),
        };
        let at = match found
            .iter()
            .position(|one| one.subject == subject && one.floor == floor)
        {
            Some(at) => at,
            None => {
                found.push(Warning {
                    subject,
                    floor,
                    plain: None,
                    tints: Vec::new(),
                });
                found.len() - 1
            }
        };
        match tint {
            None => found[at].plain = Some(ratio),
            Some(hue) => found[at].tints.push((hue, ratio)),
        }
    }
    found
}

/// The check as a chat reads it: the theme, its appearances and options, why it is refused (or
/// `null`), what refused it (`errors`), what was left out of it (`notes`), each combination's
/// warnings as the engine reports them with the appearance's id and the option values added, its
/// hard floors (`null` until the engine keeps them), and whether it is clean: no error, no note
/// and no warning. `clean` does not cover hard floors, which are reported on their own.
fn checked_json(found: &Found, checked: &[theme::Checked]) -> Value {
    let summary = found.summary_json();
    let errors = serde_json::to_value(&found.errors).unwrap_or_default();
    let notes = serde_json::to_value(&found.notes).unwrap_or_default();
    let warnings: Vec<Value> = checked
        .iter()
        .flat_map(|one| {
            // The option values in the theme's order of its options.
            let options: serde_json::Map<String, Value> = found
                .theme
                .as_ref()
                .map(|theme| {
                    in_theme_order(theme, one)
                        .map(|(option, value)| (option.to_owned(), Value::from(value)))
                        .collect()
                })
                .unwrap_or_default();
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
    let clean = found.refused.is_none() && found.notes.is_empty() && warnings.is_empty();
    json!({
        "schema": SCHEMA,
        "theme_schema": theme::SCHEMA_URL,
        "theme": {
            "id": summary["id"],
            "name": summary["name"],
            "appearances": summary["appearances"],
            "options": summary["options"],
            "combinations": summary["combinations"],
            "from": summary["from"],
        },
        "refused": summary["refused"],
        "errors": errors,
        "notes": notes,
        "warnings": warnings,
        "hard_floors": Value::Null,
        "clean": clean,
    })
}
