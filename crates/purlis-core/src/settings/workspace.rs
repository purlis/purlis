// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A workspace's settings: the `settings` object in `workspaces/<ws>/workspace.json`
//! (charter-app#280, ADR 0048).
//!
//! **One layer, read by the readers that read the project's two files.** A workspace refines its
//! project for the team, so its settings sit between Shared (`charter.toml`) and Local
//! (`charter.local.toml`) — and they have the same shape as those files' tables:
//! `settings.extensions.<id>.enabled` is `[extensions.<id>] enabled`. So the JSON is read as the
//! TOML table it mirrors ([`table_in`]) and handed to the same reader,
//! `extension::project`, which refuses it in the same words. A table a later reader takes is
//! one more name in [`READ`] and one more reader asked here: `harness_plugins`
//! (charter-app#282) is read by `harness_plugin`, whose `Choices::read_in` takes this layer, and
//! `theme` (charter-app#281) by `extension::project::theme`, which also reads the workspace's
//! colour from it.
//!
//! **A save keeps the manifest the operator has.** Only `settings` changes; every other key keeps
//! its place and its value (`serde_json`'s `preserve_order`). A manifest charter wrote is stamped
//! again, so it stays charter's; one a hand wrote is written unstamped, so it stays the
//! operator's and the automatic writers keep leaving it alone (`crate::manifest`). A key removed
//! takes every object it leaves empty with it, so removing the last setting gives back the
//! manifest as it was.

use std::path::Path;

use serde_json::{Map, Value as Json};

use super::{Edit, Found, Refusal, Step, Value};
use crate::extension::project;
use crate::manifest::Ownership;
use crate::workspaces::{Plane, Workspace};

/// The workspace's file.
pub const FILE: &str = "workspace.json";

/// The key in it that holds its settings.
pub const KEY: &str = "settings";

/// The tables a workspace's settings may hold: the ones something reads.
pub const READ: &[&str] = &[
    project::TABLE,
    crate::harness_plugin::TABLE,
    crate::harness_plugin::SKILLS_TABLE,
    project::theme::TABLE,
    SKILLS,
    RUNNERS,
];

/// The table that holds what a workspace turned off of the project skills granted to it
/// (ADR 0099 §2, #2767): `settings.skills.off`, a list of skill names, read by
/// `purlis_skills::turned_off`. A workspace's alone: the project's two files hold no `[skills]`.
pub const SKILLS: &str = "skills";

/// The key in [`SKILLS`] that lists the names turned off.
pub const SKILLS_OFF: &str = "off";

/// The key in [`SKILLS`] that lists the names the workspace pins (SK-15, ADR 0102 §4): skills its
/// chats always list, which no router hides.
pub const SKILLS_PINNED: &str = "pinned";

/// The keys a workspace's [`SKILLS`] holds, each a list of skill names.
const SKILLS_KEYS: [&str; 2] = [SKILLS_OFF, SKILLS_PINNED];

/// The table that holds what a workspace's sends to a pool's runner leave out besides what a
/// save to the forge refuses (#2046, the operator's ruling of 2026-10-25):
/// `settings.runners.leave_out`, a list of files and folders, each relative to the top of the
/// clone it is in. A workspace's alone.
pub const RUNNERS: &str = "runners";

/// The key in [`RUNNERS`] that lists what a send leaves out.
pub const LEAVE_OUT: &str = "leave_out";

/// The most entries [`LEAVE_OUT`] holds.
pub const MOST_LEFT_OUT: usize = 64;

/// The longest entry of [`LEAVE_OUT`], in bytes.
pub const LONGEST_LEFT_OUT: usize = 256;

/// Whether `entry` is one [`LEAVE_OUT`] holds: a path relative to the top of a clone, with no
/// part that climbs out of it, no empty or `.` part, and no control character.
pub fn leave_out_ok(entry: &str) -> bool {
    !entry.is_empty()
        && entry.len() <= LONGEST_LEFT_OUT
        && !entry.starts_with('/')
        && !entry.chars().any(char::is_control)
        && entry
            .trim_end_matches('/')
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// **What `workspace`'s sends to a pool's runner leave out besides what a save refuses**
/// (#2046): `settings.runners.leave_out`. Empty where it names none. **Never empty for a list
/// it cannot read**: a workspace whose `workspace.json` cannot be read, or whose list holds an
/// entry its save refuses or more than [`MOST_LEFT_OUT`], is answered why, and its sends are
/// refused in those words, so what the person listed never goes for want of a reading.
pub fn send_leaves_out(plane: &Path, workspace: &str) -> Result<Vec<String>, String> {
    let held = Plane::open(plane)
        .workspace(workspace)
        .map_err(|why| why.to_string())?;
    let (doc, owner) = held.manifest();
    let file = named(workspace);
    let Some(doc) = doc else {
        // Absent is also what a file purlis may not read says (one that leads out of the
        // project): only one that is not there at all names nothing.
        let there = std::fs::symlink_metadata(held.dir().join(FILE)).is_ok();
        return if owner == Ownership::Absent && !there {
            Ok(Vec::new())
        } else {
            Err(format!(
                "{file} cannot be read, so what it leaves out of a send is not known"
            ))
        };
    };
    let Some(table) = table_of(&doc) else {
        return Ok(Vec::new());
    };
    let refused = runners_refused(&table, &file);
    if let Some(first) = refused.first() {
        return Err(first.why.clone());
    }
    Ok(table
        .get(RUNNERS)
        .and_then(|runners| runners.get(LEAVE_OUT))
        .and_then(toml::Value::as_array)
        .map(|names| {
            names
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default())
}

/// **Sets what `workspace`'s sends to a pool's runner leave out** to `entries` (#2046), through
/// this level's own save: blank entries dropped, and none left removes the key. Answers what it
/// holds after.
pub fn set_send_leaves_out(
    plane: &Path,
    workspace: &str,
    entries: &[String],
) -> Result<Vec<String>, String> {
    crate::settings::writable(plane)?;
    let mut kept: Vec<String> = Vec::new();
    for entry in entries
        .iter()
        .map(|one| one.trim())
        .filter(|one| !one.is_empty())
    {
        if !leave_out_ok(entry) {
            return Err(format!(
                "“{entry}” is not a file or folder in a clone, so nothing was saved: name one \
                 relative to the top of the clone, such as secrets/ or config/local.yml"
            ));
        }
        if !kept.iter().any(|one| one == entry) {
            kept.push(entry.to_owned());
        }
    }
    if kept.len() > MOST_LEFT_OUT {
        return Err(format!(
            "a workspace leaves out at most {MOST_LEFT_OUT} files and folders from what it sends, \
             so nothing was saved"
        ));
    }
    let read = read_file(plane, workspace)?;
    let base = read.exists.then_some(read.text.as_str());
    let path = vec![
        crate::settings::Step::Key(RUNNERS.to_owned()),
        crate::settings::Step::Key(LEAVE_OUT.to_owned()),
    ];
    let value = (!kept.is_empty()).then(|| crate::settings::Value::List(kept.clone()));
    save(
        plane,
        workspace,
        base,
        &[crate::settings::Edit { path, value }],
    )
    .map_err(|why| why.join(" "))?;
    Ok(kept)
}

/// What a workspace's `settings.runners` holds that [`send_leaves_out`] would not read (#2046):
/// anything but `leave_out`, and a `leave_out` that is not a list of at most
/// [`MOST_LEFT_OUT`] paths in a clone. The reader then leaves out nothing more.
pub fn runners_refused(top: &toml::Table, file: &str) -> Vec<Refusal> {
    let Some(table) = top.get(RUNNERS) else {
        return Vec::new();
    };
    let shape =
        format!("a workspace's {RUNNERS} are {{\"{LEAVE_OUT}\": [\"<path in a clone>\", …]}}");
    let Some(table) = table.as_table() else {
        return vec![Refusal::at(
            format!("{KEY}.{RUNNERS} in {file} is not an object — {shape}"),
            &[RUNNERS],
        )];
    };
    table
        .iter()
        .filter_map(|(key, value)| {
            let why = if key != LEAVE_OUT {
                format!("{KEY}.{RUNNERS}.{key} in {file} is not read — {shape}")
            } else if !value.as_array().is_some_and(|names| {
                names.len() <= MOST_LEFT_OUT
                    && names
                        .iter()
                        .all(|name| name.as_str().is_some_and(leave_out_ok))
            }) {
                format!(
                    "{KEY}.{RUNNERS}.{LEAVE_OUT} in {file} is not a list of at most \
                     {MOST_LEFT_OUT} files and folders in a clone, so a send leaves out nothing \
                     more — {shape}"
                )
            } else {
                return None;
            };
            Some(Refusal::at(why, &[RUNNERS, key.as_str()]))
        })
        .collect()
}

/// The file as a sentence names it: `workspaces/<ws>/workspace.json`.
pub fn named(workspace: &str) -> String {
    format!("workspaces/{workspace}/{FILE}")
}

/// The `settings` of the manifest whose text this is, as the TOML table it mirrors — or `None`
/// when it is not JSON or has none. A `null` is left out: it reads as not set.
pub fn table_in(manifest: &str) -> Option<toml::Table> {
    serde_json::from_str::<Json>(manifest)
        .ok()
        .as_ref()
        .and_then(table_of)
}

/// [`table_in`], of a manifest already read.
pub(crate) fn table_of(doc: &Json) -> Option<toml::Table> {
    match doc.get(KEY).and_then(to_toml)? {
        toml::Value::Table(table) => Some(table),
        _ => None,
    }
}

fn to_toml(value: &Json) -> Option<toml::Value> {
    Some(match value {
        Json::Null => return None,
        Json::Bool(b) => toml::Value::Boolean(*b),
        Json::String(text) => toml::Value::String(text.clone()),
        Json::Number(n) => match n.as_i64() {
            Some(n) => toml::Value::Integer(n),
            None => toml::Value::Float(n.as_f64()?),
        },
        Json::Array(items) => toml::Value::Array(items.iter().filter_map(to_toml).collect()),
        Json::Object(map) => toml::Value::Table(
            map.iter()
                .filter_map(|(key, value)| Some((key.clone(), to_toml(value)?)))
                .collect(),
        ),
    })
}

/// The settings of the workspace `workspace` of the plane at `root`, as [`table_in`] reads
/// them. `None` for a name that is not a workspace's, a manifest that is not there or cannot be
/// read, and one with no settings.
pub fn read(root: &Path, workspace: &str) -> Option<toml::Table> {
    let (doc, _) = Plane::open(root).workspace(workspace).ok()?.manifest();
    table_of(&doc?)
}

/// Everything charter would not read in the settings of the manifest whose text this is, one
/// sentence each. Empty when there is nothing to refuse, and when it has no settings.
pub fn refusals(text: &str, workspace: &str) -> Vec<String> {
    standing(text, workspace)
        .into_iter()
        .map(|one| one.why)
        .collect()
}

/// [`refusals`], each with the key it is about under `settings` (#1292), as a setting at the
/// Workspace level holds it: what [`read_file`] gives the Settings tab.
pub fn standing(text: &str, workspace: &str) -> Vec<Refusal> {
    let file = named(workspace);
    let whole = |why: String| vec![Refusal { why, key: None }];
    let Ok(Json::Object(doc)) = serde_json::from_str::<Json>(text) else {
        return whole(format!(
            "{file} is not a JSON object, so purlis reads no settings from it — mend it by hand"
        ));
    };
    let Some(settings) = doc.get(KEY) else {
        return Vec::new();
    };
    let Some(settings) = settings.as_object() else {
        return whole(format!(
            "{KEY} in {file} is not an object — a workspace's settings are \
             {{\"{}\": {{\"<id>\": {{\"{}\": …, \"{}\": {{…}}}}}}}}",
            project::TABLE,
            project::ENABLED,
            project::SETTINGS
        ));
    };
    let mut out: Vec<Refusal> = settings
        .keys()
        .filter(|key| !READ.contains(&key.as_str()))
        .map(|key| {
            Refusal::at(
                format!(
                    "{KEY}.{key} in {file} is not read — a workspace's settings hold {} and \
                     nothing else",
                    READ.join(", ")
                ),
                &[key.as_str()],
            )
        })
        .collect();
    if let Some(table) = to_toml(&Json::Object(settings.clone())).and_then(|t| match t {
        toml::Value::Table(table) => Some(table),
        _ => None,
    }) {
        out.extend(project::keyed_in(&table, &file, "settings."));
        out.extend(crate::harness_plugin::keyed_in(&table, &file, "settings."));
        // And `theme` (charter-app#281), by the one reader of a theme.
        out.extend(project::theme::keyed_in_workspace(&table, &file));
        out.extend(skills_refused(&table, &file));
        out.extend(runners_refused(&table, &file));
    }
    out
}

/// What a workspace's `settings.skills` holds that `purlis_skills::turned_off` and
/// `purlis_core::skills::pinned_in` would not read (#2767, SK-15): anything but `off` and
/// `pinned`, and either that is not a list of names. The reader then reads nothing of it, so a
/// save that would write it is refused.
pub fn skills_refused(top: &toml::Table, file: &str) -> Vec<Refusal> {
    let Some(table) = top.get(SKILLS) else {
        return Vec::new();
    };
    let shape = format!(
        "a workspace's {SKILLS} are {{\"{SKILLS_OFF}\": [\"<skill>\", …], \"{SKILLS_PINNED}\": \
         [\"<skill>\", …]}}"
    );
    let Some(table) = table.as_table() else {
        return vec![Refusal::at(
            format!("{KEY}.{SKILLS} in {file} is not an object — {shape}"),
            &[SKILLS],
        )];
    };
    table
        .iter()
        .filter_map(|(key, value)| {
            let why = if !SKILLS_KEYS.contains(&key.as_str()) {
                format!("{KEY}.{SKILLS}.{key} in {file} is not read — {shape}")
            } else if !value
                .as_array()
                .is_some_and(|names| names.iter().all(toml::Value::is_str))
            {
                let reads = if key == SKILLS_OFF {
                    "turns nothing off"
                } else {
                    "pins nothing"
                };
                format!(
                    "{KEY}.{SKILLS}.{key} in {file} is not a list of skill names, so it {reads} \
                     — {shape}"
                )
            } else {
                return None;
            };
            Some(Refusal::at(why, &[SKILLS, key.as_str()]))
        })
        .collect()
}

/// What the manifest's own readers would take leniently from the manifest whose text this is,
/// one sentence each (#1292): a `name` that is not the workspace's folder, and `repos` that is
/// not a list of `{"name": …}` records, each `branch` one git would take ([`a_branch_name`]). Edit as JSON refuses them, unless the file already held
/// them, so a raw edit cannot write what `clone`, `restore` and the repo list would read past
/// or drop. In the readers' `<key> in <file> …` shape, so the window can name the key. Empty for
/// text that is not a JSON object, which is refused on its own.
///
/// **What `held` (the file on disk) already held is let through by its VALUE**, never by the
/// sentence: the same odd `name`, the same `repos` that is no list, an odd entry equal to one
/// the file held. A sentence names an entry by its place, so matching sentences would let a raw
/// edit put a new odd entry where an old one stood.
pub fn manifest_refusals(text: &str, workspace: &str, held: Option<&str>) -> Vec<String> {
    let file = named(workspace);
    let Ok(Json::Object(doc)) = serde_json::from_str::<Json>(text) else {
        return Vec::new();
    };
    let before = held
        .and_then(|held| serde_json::from_str::<Json>(held).ok())
        .and_then(|held| match held {
            Json::Object(held) => Some(held),
            _ => None,
        })
        .unwrap_or_default();
    let mut out = Vec::new();
    match doc.get("name") {
        None => {}
        Some(Json::String(name)) if name == workspace => {}
        Some(name) if before.get("name") == Some(name) => {}
        Some(_) => out.push(format!(
            "name in {file} is not \"{workspace}\" — a workspace is named by its folder, so its \
             manifest's name is the folder's; rename the workspace to change it"
        )),
    }
    let shape = "a workspace's repos are a list of {\"name\": \"<repo>\"} records, each with an \
                 optional \"branch\" git would take as a branch's name";
    match doc.get("repos") {
        None => {}
        Some(Json::Array(rows)) => {
            let held_rows = match before.get("repos") {
                Some(Json::Array(held)) => held.as_slice(),
                _ => &[],
            };
            for (at, row) in rows.iter().enumerate() {
                let ok = row.as_object().is_some_and(|row| {
                    row.get("name")
                        .and_then(Json::as_str)
                        .is_some_and(crate::contain::repo_name_ok)
                        && row.get("branch").is_none_or(|branch| match branch {
                            Json::String(branch) => a_branch_name(branch),
                            other => other.is_null(),
                        })
                });
                if !ok && !held_rows.contains(row) {
                    out.push(format!(
                        "repos in {file} has an entry ({}) that is not a repo's record — {shape}",
                        at + 1
                    ));
                }
            }
        }
        Some(repos) if before.get("repos") == Some(repos) => {}
        Some(_) => out.push(format!("repos in {file} is not a list — {shape}")),
    }
    out
}

/// Whether `branch` is a name a manifest's row may carry: what `git check-ref-format --branch`
/// takes, checked in-process by gix's own reference rules (#1292). Not starting with `-`, which
/// a git command line would read as an option; not a full `refs/…` name, which would be read
/// as `refs/heads/refs/…`; not `@`, which git reads as `HEAD`; and `refs/heads/<branch>` a
/// valid branch reference. `HEAD` is let through: it is what `snapshot` records for a repo
/// with no branch checked out.
fn a_branch_name(branch: &str) -> bool {
    if branch == "HEAD" {
        return true;
    }
    !branch.starts_with('-')
        && branch != "@"
        && !branch.starts_with("refs/")
        && gix::validate::reference::branch_name(format!("refs/heads/{branch}").as_str().into())
            .is_ok()
}

/// One workspace's manifest as the Workspace settings tab reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Read {
    /// `workspaces/<ws>/workspace.json`.
    pub file: String,
    /// Whether it is there. One that is not is created by the first save.
    pub exists: bool,
    /// Its text, or empty — what a save is checked against.
    pub text: String,
    /// What charter does not take from its settings as they stand, each with its key under
    /// `settings` (#1292).
    pub refusals: Vec<Refusal>,
    /// Whether it is a JSON object, which is what a form can change.
    pub parsed: bool,
    /// Every value in its settings and the path to it, as `settings::fields` gives a TOML
    /// file's.
    pub fields: Vec<(Vec<Step>, Found)>,
    /// Whether the workspace is LIVE, so its manifest is committed and the team sees it.
    pub live: bool,
}

/// The workspace `workspace` of the plane at `root`, or the sentence for a name that is not one
/// — or for one that is not there any more, whose directory a save must never make again.
fn workspace_of(root: &Path, workspace: &str) -> Result<Workspace, String> {
    let ws = Plane::open(root)
        .workspace(workspace)
        .map_err(|_| format!("'{workspace}' is not a workspace purlis can name"))?;
    if !ws.dir().is_dir() {
        return Err(format!(
            "there is no workspace '{workspace}' in this plane any more, so it has no settings"
        ));
    }
    Ok(ws)
}

/// The manifest's text, `None` when it is not there, or why it could not be read.
fn on_disk(ws: &Workspace) -> Result<Option<String>, String> {
    let file = named(ws.name());
    ws.manifest_text().map_err(|e| {
        format!(
            "{file} could not be read ({})",
            crate::shown::short(&e.to_string())
        )
    })
}

/// The manifest of `workspace`, and what charter says about its settings now.
pub fn read_file(root: &Path, workspace: &str) -> Result<Read, String> {
    let ws = workspace_of(root, workspace)?;
    let text = on_disk(&ws)?;
    let exists = text.is_some();
    let text = text.unwrap_or_default();
    let doc = serde_json::from_str::<Json>(&text).ok();
    let parsed = !exists || doc.as_ref().is_some_and(Json::is_object);
    let fields = doc
        .as_ref()
        .and_then(table_of)
        .map(|table| super::fields_of(&table))
        .unwrap_or_default();
    Ok(Read {
        file: named(workspace),
        refusals: if exists {
            standing(&text, workspace)
        } else {
            Vec::new()
        },
        exists,
        text,
        parsed,
        fields,
        live: Plane::open(root).is_live(workspace),
    })
}

/// Apply `edits` to the settings of `workspace`'s manifest and write it — or say every reason it
/// was not written.
///
/// `base` is the manifest as the caller read it (`None`: it was not there). One that changed
/// since is not written over. A save is refused for what the reader would refuse in the settings
/// it would write, except what they already held, and for a secret-shaped value, whatever they
/// held.
pub fn save(
    root: &Path,
    workspace: &str,
    base: Option<&str>,
    edits: &[Edit],
) -> Result<(), Vec<String>> {
    let ws = workspace_of(root, workspace).map_err(|why| vec![why])?;
    let file = named(workspace);
    // From the base check to the write, one lock (#1292): a check alone leaves the window
    // between them open to a clone or a removal writing the same file.
    let _held = ws
        .manifest_lock()
        .map_err(|why| vec![format!("{why}, so nothing was saved.")])?;
    // A project this purlis opens read-only has no workspace settings written, said as every
    // committed writer says it (#1759), before anything about the edit itself. Asked under the
    // write's own lock, so the answer is the one the write goes ahead on.
    super::writable(root).map_err(|why| vec![why])?;
    let now = on_disk(&ws).map_err(|why| vec![why])?;
    if now.as_deref() != base {
        return Err(vec![format!(
            "{file} changed on disk since this tab read it, so nothing was saved. Read it again, \
             then make the change again."
        )]);
    }
    let (mut doc, standing) = match &now {
        Some(text) => match serde_json::from_str::<Json>(text) {
            Ok(Json::Object(doc)) => (doc, refusals(text, workspace)),
            _ => return Err(refusals(text, workspace)),
        },
        None => (birth(&ws), Vec::new()),
    };
    let mut settings = match doc.get(KEY) {
        Some(Json::Object(settings)) => settings.clone(),
        None => Map::new(),
        Some(_) => return Err(refusals(now.as_deref().unwrap_or_default(), workspace)),
    };
    for edit in edits {
        apply(&mut settings, edit)?;
    }
    // In the place it had — `insert` keeps an existing key's — and `shift_remove`, not
    // `remove`, which would swap the last key into its place.
    if settings.is_empty() {
        doc.shift_remove(KEY);
    } else {
        doc.insert(KEY.to_owned(), Json::Object(settings.clone()));
    }
    let doc = Json::Object(doc);
    let written = crate::pyjson::dumps_indent2(&doc);
    let mut refused: Vec<String> = refusals(&written, workspace)
        .into_iter()
        .filter(|why| !standing.contains(why))
        .collect();
    // A built-in skill's name in `settings.harness_skills` (SK-6, #2769), as the project's files
    // refuse it.
    if let Some(table) = table_of(&doc) {
        refused.extend(
            crate::harness_plugin::built_in_refused(
                &table,
                &file,
                "settings.",
                &crate::skills::built_in_names_here(),
            )
            .into_iter()
            .map(|one| one.why),
        );
    }
    // As the project save reads the manifest: as written, then through its escapes (#1315).
    if let Some(kind) = crate::secretshape::kind_as_read(
        Some(crate::secretshape::Structured::Json),
        &crate::pyjson::dumps_indent2(&Json::Object(settings)),
    ) {
        refused.push(format!(
            "{file} looks like it holds a secret ({kind}), so nothing was saved — it is \
             committed with a LIVE workspace, so every clone of this plane would carry the \
             secret. Keep the value in a vault and name it where it is needed as \
             vault:<vault>/<key>."
        ));
    }
    if !refused.is_empty() {
        return Err(refused);
    }
    // A manifest a hand wrote stays the hand's: it is written without charter's stamp.
    let stamped = crate::manifest::ownership(now.as_deref()) != Ownership::Operator;
    ws.write_manifest_as(&doc, stamped).map_err(|e| {
        vec![format!(
            "{file} could not be written ({}), so nothing was saved.",
            crate::shown::short(&e.to_string())
        )]
    })
}

/// **Write `text` as the whole manifest of `workspace`** (Edit as JSON, NO-7 #1232) — or say
/// every reason it was not written. The Workspace level's counterpart of Edit as TOML
/// (`settings::save`), under the same rules:
///
/// - `base` is the manifest as the caller read it (`None`: not there), and one that changed
///   since is not written over;
/// - text that is not a JSON object is refused, so a half-mended manifest is never written;
/// - what the settings reader would refuse in it is refused, except what the manifest already
///   held — an edit is not refused for a key it did not touch;
/// - a secret-shaped value anywhere in it is refused, whatever it held before, as typed and as
///   the parsed document holds it: a LIVE workspace's manifest is committed.
///
/// **It stays whose it was**, as a form's save leaves it: a manifest a hand wrote (charter's
/// digest does not match) is written as typed, byte for byte, and stays the operator's; one
/// charter wrote, or a first one, is stamped again, so charter's writers go on keeping it.
pub fn save_text(
    root: &Path,
    workspace: &str,
    base: Option<&str>,
    text: &str,
) -> Result<(), Vec<String>> {
    let ws = workspace_of(root, workspace).map_err(|why| vec![why])?;
    let file = named(workspace);
    // From the base check to the write, one lock (#1292): a check alone leaves the window
    // between them open to a clone or a removal writing the same file.
    let _held = ws
        .manifest_lock()
        .map_err(|why| vec![format!("{why}, so nothing was saved.")])?;
    // As [`save`] (#1759), under the same lock.
    super::writable(root).map_err(|why| vec![why])?;
    let now = on_disk(&ws).map_err(|why| vec![why])?;
    if now.as_deref() != base {
        return Err(vec![format!(
            "{file} changed on disk since this tab read it, so nothing was saved. Read it again, \
             then make the change again."
        )]);
    }
    let doc = match serde_json::from_str::<Json>(text) {
        Ok(doc @ Json::Object(_)) => doc,
        Ok(_) => {
            return Err(vec![format!(
                "{file} would not be a JSON object, so nothing was saved: a workspace's manifest \
                 is one object, {{…}}"
            )]);
        }
        Err(e) => {
            return Err(vec![format!(
                "{file} would not be a JSON object, so nothing was saved: {e}"
            )]);
        }
    };
    let standing: Vec<String> = now
        .as_deref()
        .filter(|now| serde_json::from_str::<Json>(now).is_ok_and(|doc| doc.is_object()))
        .map_or_else(Vec::new, |now| refusals(now, workspace));
    let mut refused: Vec<String> = refusals(text, workspace)
        .into_iter()
        .filter(|why| !standing.contains(why))
        .chain(manifest_refusals(text, workspace, now.as_deref()))
        .collect();
    // As typed, and as charter reads it: a string can spell a character as an escape, and the
    // document — which is also what charter's own writer puts on disk — holds the character.
    // The one answer the project save gives a staged workspace.json (#1304).
    if let Some(kind) =
        crate::secretshape::kind_as_read(Some(crate::secretshape::Structured::Json), text)
    {
        refused.push(format!(
            "{file} looks like it holds a secret ({kind}), so nothing was saved — it is \
             committed with a LIVE workspace, so every clone of this plane would carry the \
             secret. Keep the value in a vault and name it where it is needed as \
             vault:<vault>/<key>."
        ));
    }
    if !refused.is_empty() {
        return Err(refused);
    }
    let not_written = |e: std::io::Error| {
        vec![format!(
            "{file} could not be written ({}), so nothing was saved.",
            crate::shown::short(&e.to_string())
        )]
    };
    if crate::manifest::ownership(now.as_deref()) == Ownership::Operator {
        ws.write_manifest_text(text).map_err(not_written)
    } else {
        ws.write_manifest_as(&doc, true).map_err(not_written)
    }
}

/// The manifest a workspace gets when it has none, as charter's own scaffold writes it.
fn birth(ws: &Workspace) -> Map<String, Json> {
    match ws.birth_manifest(chrono::Utc::now(), &crate::wscmd::ensure::author()) {
        Json::Object(doc) => doc,
        _ => Map::new(),
    }
}

/// One edit, applied to `settings` by its path of keys. A key removed takes every object it
/// leaves empty with it.
fn apply(settings: &mut Map<String, Json>, edit: &Edit) -> Result<(), Vec<String>> {
    let keys: Vec<&str> = edit
        .path
        .iter()
        .map(|step| match step {
            Step::Key(key) => Ok(key.as_str()),
            Step::Index(_) => Err(vec![format!(
                "a workspace's {KEY} hold no lists of tables, so a form cannot change one"
            )]),
        })
        .collect::<Result<_, _>>()?;
    let Some((last, way)) = keys.split_last() else {
        return Err(vec![format!("an edit to {KEY} names no key")]);
    };
    match &edit.value {
        Some(value) => {
            let mut at = settings;
            for key in way {
                let next = at
                    .entry((*key).to_owned())
                    .or_insert_with(|| Json::Object(Map::new()));
                at = next.as_object_mut().ok_or_else(|| {
                    vec![format!(
                        "{KEY}.{} is not an object, so it has no keys",
                        way.join(".")
                    )]
                })?;
            }
            at.insert((*last).to_owned(), to_json(value));
        }
        None => remove(settings, way, last),
    }
    Ok(())
}

/// Remove `last` under `way`, and every object on the way it leaves empty.
fn remove(at: &mut Map<String, Json>, way: &[&str], last: &str) {
    match way.split_first() {
        None => {
            at.shift_remove(last);
        }
        Some((key, rest)) => {
            if let Some(Json::Object(inner)) = at.get_mut(*key) {
                remove(inner, rest, last);
                if inner.is_empty() {
                    at.shift_remove(*key);
                }
            }
        }
    }
}

fn to_json(value: &Value) -> Json {
    match value {
        Value::Text(text) => Json::String(text.clone()),
        Value::Integer(n) => Json::from(*n),
        Value::Bool(b) => Json::Bool(*b),
        Value::List(items) => Json::Array(items.iter().cloned().map(Json::String).collect()),
    }
}

#[cfg(test)]
mod tests;
