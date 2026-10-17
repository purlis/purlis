// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis theme list` and `purlis theme check` (spec #2313, #2321): every theme available on
//! this machine and where it comes from, and the theme check of any one of them.
//!
//! **Nothing here judges a theme.** The theme engine (`purlis_core::theme`) reads it, puts right
//! what it can and says so, and checks it; this module finds the themes there are, asks the
//! engine about each, and prints its answer, as a table for a person or as JSON for a chat.
//! Nothing is written: a theme command reads the built-ins, the person's `theme.json` and the
//! approved extensions' themes, and leaves every file as it found it.
//!
//! **Where a theme comes from, today:** built in, the person's own `theme.json` (its id is
//! [`MINE`]), or an approved extension that is on (its id is `<extension>/<theme>`, as a
//! project's pick names it). A theme file anywhere else can be checked by its path. The levels
//! the spec adds (machine, project and workspace theme folders) are more sources in the same
//! list, with the same `from`.
//!
//! **The exit is a linter's.** `check` exits 0 when the theme has no errors, warnings or not
//! (a contrast finding is the author's call, ADR 0097 §2); 1 when it has an error or is
//! refused; 2 when there is no such theme here.

use std::path::Path;
use std::process::ExitCode;

use clap::Subcommand;
use purlis_core::theme::{self, Finding, Resolved};
use purlis_core::{extension, shown};
use serde_json::{Value, json};

/// The version of the JSON `list --json` and `check --json` answer, as `hook --list --json`
/// carries one: a reader checks it before it reads the rest.
pub const SCHEMA: u64 = 1;

/// The id of the person's own theme, `theme.json` beside `machine.json`.
pub const MINE: &str = "mine";

/// `purlis theme …`.
#[derive(Subcommand)]
pub enum ThemeCommand {
    /// Every theme available here, with its id, its appearance and where it comes from: built
    /// in, your own theme.json (`mine`), or an approved extension (`<extension>/<theme>`).
    /// A theme purlis cannot read is listed as refused, with why.
    List {
        /// Answer as JSON, for a chat or a script.
        #[arg(long)]
        json: bool,
    },
    /// The theme check of one theme: its errors (values purlis had to put right), its contrast
    /// warnings (each pair with its ratio and its floor, as it is and in every workspace tint)
    /// and its hard floors. Exits 0 with no errors, 1 with an error or when the theme is
    /// refused, 2 when there is no such theme.
    Check {
        /// A theme's id from `purlis theme list`, or the path of a theme file. An id may hold a
        /// space (`solarized/Solarized Dark`): quote it.
        theme: String,
        /// Answer as JSON, for a chat or a script. Its `clean` is true with no error and no
        /// warning; it does not cover hard floors, which `hard_floors` reports on its own
        /// (`null` until purlis checks them).
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
}

impl From {
    /// As the table's FROM column says it.
    fn said(&self) -> String {
        match self {
            Self::BuiltIn => "built-in".to_owned(),
            Self::Mine(path) | Self::File(path) => shown::short(path),
            Self::Extension(id) => format!("extension {}", shown::short(id)),
        }
    }

    fn json(&self) -> Value {
        match self {
            Self::BuiltIn => json!({ "kind": "built-in" }),
            Self::Mine(path) => json!({ "kind": "mine", "path": path }),
            Self::Extension(id) => json!({ "kind": "extension", "extension": id }),
            Self::File(path) => json!({ "kind": "file", "path": path }),
        }
    }
}

/// One theme found here: what the engine made of it, or why it is not a theme at all.
struct Found {
    id: String,
    from: From,
    read: Result<Resolved, String>,
}

impl Found {
    fn built_in(theme: &theme::ResolvedTheme) -> Self {
        Self {
            id: theme.name.clone(),
            from: From::BuiltIn,
            read: Ok(Resolved {
                theme: theme.clone(),
                complaints: Vec::new(),
            }),
        }
    }

    fn summary_json(&self) -> Value {
        let (name, appearance, refused) = match &self.read {
            Ok(resolved) => (
                Value::from(resolved.theme.name.clone()),
                json!(resolved.theme.appearance),
                Value::Null,
            ),
            Err(why) => (Value::Null, Value::Null, Value::from(why.clone())),
        };
        json!({
            "id": self.id,
            "name": name,
            "appearance": appearance,
            "from": self.from.json(),
            "refused": refused,
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
                println!("{}", json!({ "schema": SCHEMA, "themes": themes }));
            } else {
                print!("{}", table(&found));
            }
            ExitCode::SUCCESS
        }
        ThemeCommand::Check { theme, json } => {
            let Some(found) = find(config.as_deref(), theme) else {
                eprintln!(
                    "purlis: there is no theme '{}' here — `purlis theme list` lists the themes \
                     there are, and `purlis theme check <path>` checks a theme file",
                    shown::short(theme)
                );
                return ExitCode::from(2);
            };
            let report = found.read.as_ref().ok().map(|it| theme::check(&it.theme));
            if *json {
                println!("{}", checked_json(&found, report.as_ref()));
            } else {
                print!("{}", checked_text(&found, report.as_ref()));
            }
            let failed = match &found.read {
                Ok(resolved) => !resolved.complaints.is_empty(),
                Err(_) => true,
            };
            ExitCode::from(u8::from(failed))
        }
    }
}

/// Every theme there is here, in the order a person reads them: the built-ins, the person's
/// own, then each approved extension's in the order the survey found them.
fn available(config: Option<&Path>) -> Vec<Found> {
    let mut found: Vec<Found> = theme::built_ins().iter().map(Found::built_in).collect();
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
    let read = match (theirs.trouble, theirs.theme) {
        (Some(trouble), _) => Err(trouble),
        (None, Some(theme)) => Ok(Resolved {
            theme,
            complaints: theirs.complaints,
        }),
        (None, None) => Err("is not a theme".to_owned()),
    };
    Some(Found {
        id: MINE.to_owned(),
        from: From::Mine(theirs.path),
        read,
    })
}

/// The themes approved extensions that are on contribute, each resolved from the text its
/// fingerprint was taken over. One whose text is not JSON is refused.
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
            found.push(Found {
                id: format!("{}/{}", offered.extension, offered.name),
                from: From::Extension(offered.extension),
                read: offered
                    .theme
                    .ok_or_else(|| "is not JSON, so it is not drawn".to_owned()),
            });
        }
    }
    found
}

/// The theme `asked` names: a built-in by its id (or an id it had before), the person's own,
/// an extension's by `<extension>/<theme>`, or else a theme file by its path.
fn find(config: Option<&Path>, asked: &str) -> Option<Found> {
    if let Some(built) = theme::built_in(asked) {
        return Some(Found::built_in(built));
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
    path.is_file().then(|| Found {
        id: asked.to_owned(),
        from: From::File(path.display().to_string()),
        read: read_file(path),
    })
}

/// A theme file named on the command line, held to the envelope the person's own is: no larger
/// than a theme file may be, JSON, and an object.
///
/// A link is followed, on purpose: this is a path the person typed, and checking the file it
/// leads to is what they asked for. The person's own `theme.json`, which purlis reads unasked
/// at every launch, is read link-refusing (`windowprefs::read_theme`).
fn read_file(path: &Path) -> Result<Resolved, String> {
    use std::io::Read;
    let most = purlis_core::windowprefs::MAX_BYTES;
    let mut text = String::new();
    let read = std::fs::File::open(path)
        .and_then(|file| file.take(most + 1).read_to_string(&mut text))
        .map_err(|why| format!("could not be read: {why}"))?;
    if read as u64 > most {
        return Err(format!(
            "is larger than {} KiB, the most a theme file may be",
            most / 1024
        ));
    }
    let document: Value =
        serde_json::from_str(&text).map_err(|why| format!("is not JSON: {why}"))?;
    if !document.is_object() {
        return Err("is not a theme: it is not a JSON object".to_owned());
    }
    Ok(theme::load(&document))
}

/// The listing: one row per theme, its columns sized from its cells, and the refused ones
/// after it with why.
fn table(found: &[Found]) -> String {
    let header = ["ID", "NAME", "APPEARANCE", "FROM"];
    let mut rows: Vec<[String; 4]> = Vec::new();
    let mut refused = Vec::new();
    for one in found {
        match &one.read {
            Ok(resolved) => rows.push([
                shown::short(&one.id),
                shown::short(&resolved.theme.name),
                appearance(resolved.theme.appearance).to_owned(),
                one.from.said(),
            ]),
            Err(why) => refused.push(format!(
                "  {} ({}): {}",
                shown::short(&one.id),
                one.from.said(),
                shown::short(why)
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
const FLOORS_NOT_CHECKED: &str = "not checked by this purlis yet (#2332)";

/// The check as a person reads it.
fn checked_text(found: &Found, report: Option<&theme::Report>) -> String {
    let mut out = String::new();
    let resolved = match &found.read {
        Err(why) => {
            // The person's own is the one the window would have drawn: it draws Purlis instead.
            let instead = match found.from {
                From::Mine(_) => " — the window draws Purlis in its place",
                _ => "",
            };
            out.push_str(&format!(
                "{} — {}\nrefused: {}{instead}\n",
                shown::short(&found.id),
                found.from.said(),
                shown::short(why)
            ));
            return out;
        }
        Ok(resolved) => resolved,
    };
    out.push_str(&format!(
        "{} — {}, {}\n",
        shown::short(&resolved.theme.name),
        found.from.said(),
        appearance(resolved.theme.appearance)
    ));
    if resolved.complaints.is_empty() {
        out.push_str("errors: none\n");
    } else {
        out.push_str(&format!("errors ({}):\n", resolved.complaints.len()));
        for complaint in &resolved.complaints {
            out.push_str(&format!("  {}\n", shown::readable(&complaint.said, 240)));
        }
    }
    let findings = report.map_or(&[][..], |it| it.findings.as_slice());
    if findings.is_empty() {
        out.push_str("warnings: none\n");
    } else {
        let said = warnings(findings);
        out.push_str(&format!("warnings ({}):\n", said.len()));
        for warning in said {
            out.push_str(&format!("  {}\n", warning.line()));
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
                "the {} tint{}",
                names.join(", "),
                if names.len() == 1 { "" } else { "s" }
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

/// The check as a chat reads it: the theme, why it is refused (or `null`), its errors, its
/// warnings as the engine reports them, its hard floors (`null` until the engine keeps them),
/// and whether it is clean: no error and no warning. `clean` does not cover hard floors, which
/// are reported on their own.
fn checked_json(found: &Found, report: Option<&theme::Report>) -> Value {
    let summary = found.summary_json();
    let (errors, refused) = match &found.read {
        Ok(resolved) => (
            serde_json::to_value(&resolved.complaints).unwrap_or_default(),
            Value::Null,
        ),
        Err(why) => (json!([]), Value::from(why.clone())),
    };
    let warnings = report.map_or_else(
        || json!([]),
        |it| serde_json::to_value(&it.findings).unwrap_or_default(),
    );
    let clean = refused.is_null()
        && errors.as_array().is_some_and(Vec::is_empty)
        && warnings.as_array().is_some_and(Vec::is_empty);
    json!({
        "schema": SCHEMA,
        "theme": {
            "id": summary["id"],
            "name": summary["name"],
            "appearance": summary["appearance"],
            "from": summary["from"],
        },
        "refused": refused,
        "errors": errors,
        "warnings": warnings,
        "hard_floors": Value::Null,
        "clean": clean,
    })
}
