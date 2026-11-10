// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The pins file, `workspaces/<ws>/artifacts/pins.toml` (`docs/plane-format.md`): its reading
//! and its writing, and nothing about the folders it names.
//!
//! **One pin per line, and that is the merge.** The file is committed, two people pin at once,
//! and `.gitattributes` gives it git's `union` driver. A union keeps both sides' lines, so the
//! file is shaped for it: `pin` is an array of inline tables, one table per line, each line
//! ending in a comma, which TOML allows after the last element too. Two added lines unioned are
//! still one valid array. A union of two edits of the same pin is two lines with one id, which
//! [`parse`] reads as one pin, the later line's, and the next write puts right.
//!
//! **Unknown keys are kept.** A newer purlis may add a key to a pin or to the file; this one
//! carries it through every write untouched, and refuses to write a file whose `version` it does
//! not know at all.

use sha2::{Digest, Sha256};
use toml_edit::{Array, DocumentMut, InlineTable, Item, Value};

/// The pins file version this purlis reads and writes.
pub const VERSION: i64 = 1;

/// The bytes of the SHA-256 a pin's id keeps: 128 bits, as 32 hex digits.
pub const ID_BYTES: usize = 16;

/// The keys of a pin that purlis reads; every other key is kept as it is.
const KNOWN: [&str; 7] = ["id", "folder", "name", "icon", "order", "command", "cwd"];

/// One pin: an artifact folder given an icon on its workspace's activity bar.
#[derive(Clone, Debug)]
pub struct Pin {
    /// [`id_of`] its folder, command and working folder.
    pub id: String,
    /// The folder's name inside `artifacts/`.
    pub folder: String,
    /// What its icon is called.
    pub name: String,
    /// One of [`crate::personamark::ICONS`].
    pub icon: String,
    /// Its place in the group, from 1.
    pub order: u32,
    /// The command that writes its data, if any.
    pub command: Option<String>,
    /// That command's working folder, workspace-relative.
    pub cwd: Option<String>,
    /// The keys this purlis does not know, in their order.
    extra: InlineTable,
}

impl PartialEq for Pin {
    fn eq(&self, other: &Self) -> bool {
        self.table().to_string() == other.table().to_string()
    }
}

impl Pin {
    /// A new pin, its id drawn from its folder, command and working folder.
    pub fn new(
        folder: String,
        name: String,
        icon: String,
        order: u32,
        command: Option<String>,
        cwd: Option<String>,
    ) -> Self {
        Self {
            id: id_of(&folder, command.as_deref(), cwd.as_deref()),
            folder,
            name,
            icon,
            order,
            command,
            cwd,
            extra: InlineTable::new(),
        }
    }

    /// The pin as the one inline table its line holds.
    fn table(&self) -> InlineTable {
        let mut table = InlineTable::new();
        table.insert("id", Value::from(self.id.as_str()));
        table.insert("folder", Value::from(self.folder.as_str()));
        table.insert("name", Value::from(self.name.as_str()));
        table.insert("icon", Value::from(self.icon.as_str()));
        table.insert("order", Value::from(i64::from(self.order)));
        if let Some(command) = &self.command {
            table.insert("command", Value::from(command.as_str()));
        }
        if let Some(cwd) = &self.cwd {
            table.insert("cwd", Value::from(cwd.as_str()));
        }
        for (key, value) in self.extra.iter() {
            let mut value = value.clone();
            value.decor_mut().clear();
            table.insert(key, value);
        }
        table.fmt();
        table
    }
}

/// A pin's identity: the first 32 hex digits (128 bits) of the SHA-256 of its folder, command
/// and working folder joined by NUL. A change to any of the three is a new pin (ADR 0122 §1).
/// It stands for the command in both approvals, so it is long enough that no second command
/// with the same id can be searched for (ADR 0122, amended 2026-11-10).
pub fn id_of(folder: &str, command: Option<&str>, cwd: Option<&str>) -> String {
    let mut hash = Sha256::new();
    hash.update(folder.as_bytes());
    hash.update([0]);
    hash.update(command.unwrap_or_default().as_bytes());
    hash.update([0]);
    hash.update(cwd.unwrap_or_default().as_bytes());
    hash.finalize()
        .iter()
        .take(ID_BYTES)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// A whole pins file.
#[derive(Clone, Debug)]
pub struct Pins {
    /// The file's `version`.
    pub version: i64,
    /// Its pins, sorted by order, then id.
    pub pins: Vec<Pin>,
    /// The file's own keys besides `version` and `pin`, kept as they are.
    doc: DocumentMut,
}

impl Default for Pins {
    fn default() -> Self {
        Self {
            version: VERSION,
            pins: Vec::new(),
            doc: DocumentMut::new(),
        }
    }
}

impl Pins {
    /// The pins in their order: by order, then id.
    pub fn sort(&mut self) {
        self.pins
            .sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
    }

    /// The order a pin added now takes: one after the last.
    pub fn next_order(&self) -> u32 {
        self.pins
            .iter()
            .map(|pin| pin.order)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }

    /// Why this purlis may not write the file, if it may not: a version it does not know.
    pub fn unwritable(&self) -> Option<String> {
        (self.version != VERSION).then(|| {
            format!(
                "the pins file is version {}, which this purlis does not write (it writes \
                 version {VERSION}); update purlis to change its pins",
                self.version
            )
        })
    }

    /// The file's text: `version`, the pins one per line, then the keys it does not know.
    pub fn render(&self) -> String {
        let mut doc = self.doc.clone();
        doc.remove("version");
        doc.remove("pin");
        let mut out = DocumentMut::new();
        out.insert("version", Item::Value(Value::from(self.version)));
        let mut array = Array::new();
        for pin in &self.pins {
            let mut value = Value::InlineTable(pin.table());
            value.decor_mut().set_prefix("\n  ");
            array.push_formatted(value);
        }
        array.set_trailing_comma(!self.pins.is_empty());
        array.set_trailing(if self.pins.is_empty() { "" } else { "\n" });
        out.insert("pin", Item::Value(Value::Array(array)));
        for (key, item) in doc.iter() {
            out.insert(key, item.clone());
        }
        let text = out.to_string();
        if text.ends_with('\n') {
            text
        } else {
            text + "\n"
        }
    }
}

/// Read a pins file's text. A file that is no TOML, or a pin missing what every pin has, is
/// refused with a sentence; nothing is guessed.
pub fn parse(text: &str) -> Result<Pins, String> {
    let doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| {
        format!("the pins file is not TOML ({})", e.message())
    })?;
    let version = match doc.get("version") {
        None => VERSION,
        Some(item) => item
            .as_integer()
            .ok_or("the pins file's version is not a number")?,
    };
    let tables: Vec<InlineTable> = match doc.get("pin") {
        None => Vec::new(),
        Some(Item::Value(Value::Array(array))) => array
            .iter()
            .map(|value| {
                value
                    .as_inline_table()
                    .cloned()
                    .ok_or_else(|| "a pin in the pins file is not a table".to_string())
            })
            .collect::<Result<_, _>>()?,
        Some(Item::ArrayOfTables(tables)) => tables
            .iter()
            .map(|table| table.clone().into_inline_table())
            .collect(),
        Some(_) => return Err("the pins file's `pin` is not a list of pins".into()),
    };
    let mut pins: Vec<(String, Pin)> = Vec::new();
    for (at, table) in tables.iter().enumerate() {
        let (written, pin) =
            pin_of(table).map_err(|why| format!("pin {} in the pins file {why}", at + 1))?;
        // Two lines with one id are a union of two edits of one pin: the later line is it.
        pins.retain(|(kept, _)| *kept != written);
        pins.push((written, pin));
    }
    let pins = pins.into_iter().map(|(_, pin)| pin).collect();
    let mut read = Pins { version, pins, doc };
    read.sort();
    Ok(read)
}

/// One pin from its table, or what it lacks, with the id its line was written with.
///
/// **Its id is drawn again from its folder, command and working folder**, never taken as
/// written (ADR 0122 §1): a line whose command changed under its old id is a new pin, so no
/// approval given under that id carries over to it. The written id only names the line, so
/// two edits of one line unioned are still one pin.
fn pin_of(table: &InlineTable) -> Result<(String, Pin), String> {
    let text = |key: &str| -> Result<String, String> {
        table
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| format!("has no `{key}`"))
    };
    let optional = |key: &str| -> Result<Option<String>, String> {
        match table.get(key) {
            None => Ok(None),
            Some(value) => value
                .as_str()
                .map(|s| Some(s.to_string()))
                .ok_or_else(|| format!("has a `{key}` that is not text")),
        }
    };
    let order = table
        .get("order")
        .and_then(Value::as_integer)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or("has no `order` that is a whole number")?;
    let mut extra = InlineTable::new();
    for (key, value) in table.iter() {
        if !KNOWN.contains(&key) {
            extra.insert(key, value.clone());
        }
    }
    let written = text("id")?;
    let folder = text("folder")?;
    let name = text("name")?;
    let icon = text("icon")?;
    let command = optional("command")?;
    let cwd = optional("cwd")?;
    let pin = Pin {
        id: id_of(&folder, command.as_deref(), cwd.as_deref()),
        folder,
        name,
        icon,
        order,
        command,
        cwd,
        extra,
    };
    Ok((written, pin))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pin(folder: &str, order: u32) -> Pin {
        Pin::new(
            folder.into(),
            folder.into(),
            "chart".into(),
            order,
            None,
            None,
        )
    }

    #[test]
    fn a_file_written_and_read_back_holds_the_same_pins() {
        let pins = Pins {
            pins: vec![
                pin("a", 1),
                Pin::new(
                    "b".into(),
                    "Bee \"quoted\"".into(),
                    "star".into(),
                    2,
                    Some("run it".into()),
                    Some(".".into()),
                ),
            ],
            ..Pins::default()
        };
        let read = parse(&pins.render()).unwrap();
        assert_eq!(read.pins, pins.pins);
        assert_eq!(read.version, VERSION);
    }

    #[test]
    fn each_pin_is_one_line_ending_in_a_comma() {
        let pins = Pins {
            pins: vec![pin("a", 1), pin("b", 2)],
            ..Pins::default()
        };
        let text = pins.render();
        let lines: Vec<&str> = text.lines().filter(|l| l.contains("folder")).collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines.iter().all(|l| l.ends_with("},")), "{text}");
    }

    #[test]
    fn pins_are_sorted_by_order_then_id() {
        let text = "version = 1\npin = [\n  { id = \"bbb\", folder = \"b\", name = \"b\", icon = \"chart\", order = 1 },\n  \
             { id = \"aaa\", folder = \"a\", name = \"a\", icon = \"chart\", order = 1 },\n  \
             { id = \"000\", folder = \"z\", name = \"z\", icon = \"chart\", order = 0 },\n]\n";
        let pins = parse(text).unwrap().pins;
        let read: Vec<(u32, &str)> = pins.iter().map(|p| (p.order, p.id.as_str())).collect();
        let mut sorted = read.clone();
        sorted.sort_unstable();
        assert_eq!(read, sorted);
        assert_eq!(pins[0].folder, "z");
    }

    #[test]
    fn keys_this_purlis_does_not_know_survive_a_write() {
        let text = "version = 1\nnote = \"theirs\"\npin = [\n  { id = \"aaa\", folder = \"a\", name = \"a\", icon = \"chart\", order = 1, colour = \"teal\", more = { deep = [1, 2] } },\n]\n\n[future]\nkey = true\n";
        let written = parse(text).unwrap().render();
        assert!(written.contains("colour = \"teal\""), "{written}");
        assert!(written.contains("more = { deep = [1, 2] }"), "{written}");
        assert!(written.contains("note = \"theirs\""), "{written}");
        assert!(written.contains("[future]\nkey = true"), "{written}");
        let again = parse(&written).unwrap();
        assert_eq!(again.render(), written, "a second write changes nothing");
    }

    #[test]
    fn two_lines_with_one_id_are_one_pin_the_later_lines() {
        let text = "pin = [\n  { id = \"aaa\", folder = \"a\", name = \"first\", icon = \"chart\", order = 1 },\n  { id = \"aaa\", folder = \"a\", name = \"second\", icon = \"chart\", order = 3 },\n]\n";
        let pins = parse(text).unwrap().pins;
        assert_eq!(pins.len(), 1);
        assert_eq!(pins[0].name, "second");
    }

    #[test]
    fn a_version_this_purlis_does_not_know_is_read_but_not_written() {
        let pins = parse("version = 2\npin = []\n").unwrap();
        assert!(pins.unwritable().unwrap().contains("version 2"));
        assert!(parse("version = 1\n").unwrap().unwritable().is_none());
    }

    #[test]
    fn a_pin_missing_a_key_is_refused_with_a_sentence() {
        let why = parse("pin = [ { id = \"a\", folder = \"a\", icon = \"chart\", order = 1 } ]")
            .unwrap_err();
        assert_eq!(why, "pin 1 in the pins file has no `name`");
    }

    /// The exploit of the review of #5978: a teammate changes a pin's command and keeps its
    /// old id, so the page's and Regenerate's approvals, both keyed by that id, would carry over.
    #[test]
    fn a_pin_whose_command_changed_under_its_old_id_is_read_as_a_new_pin() {
        let approved = id_of("board", Some("node scripts/gen.mjs"), Some("."));
        let text = format!(
            "version = 1\npin = [\n  {{ id = \"{approved}\", folder = \"board\", name = \"b\", \
             icon = \"chart\", order = 1, command = \"node -e 'evil()' scripts/gen.mjs\", \
             cwd = \".\" }},\n]\n"
        );
        let pin = parse(&text).unwrap().pins.remove(0);
        assert_ne!(pin.id, approved);
        assert_eq!(
            pin.id,
            id_of("board", Some("node -e 'evil()' scripts/gen.mjs"), Some("."))
        );
        // A line whose id is not its own is put right by the next write.
        let pins = parse(&text).unwrap();
        assert!(!pins.render().contains(&approved), "{}", pins.render());
    }

    /// The exploit of the second review of #5978: an id of 48 bits has a second preimage a GPU
    /// finds, a command of the attacker's whose id is the approved one's.
    #[test]
    fn an_id_is_128_bits_of_the_sha256_so_no_other_command_is_found_with_it() {
        let id = id_of("board", Some("node scripts/gen.mjs"), Some("."));
        assert_eq!(id.len(), 32, "{id}");
        assert!(
            id.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }

    #[test]
    fn the_id_changes_with_the_folder_the_command_or_the_working_folder() {
        let base = id_of("a", Some("x"), Some("."));
        assert_eq!(base.len(), 32);
        assert_eq!(base, id_of("a", Some("x"), Some(".")));
        assert_ne!(base, id_of("b", Some("x"), Some(".")));
        assert_ne!(base, id_of("a", Some("y"), Some(".")));
        assert_ne!(base, id_of("a", Some("x"), Some("sub")));
        assert_ne!(id_of("a", None, None), id_of("a", Some(""), Some("x")));
    }
}
