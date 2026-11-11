// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Always allow: a Run grant made a declared tool** (ADR 0112, spec #5235, #5245). The person
//! reads the exact line ([`line_of`]), derived from the command that waits, and the click writes
//! exactly that to `charter.toml` under the persona ([`write`]). Teammates get it through the usual
//! Notice, and a machine runs nothing of it until its own Allow.
//!
//! **What the line holds**: the program's name, the vault, and each key with its hand-over, as the
//! command spelled them. Never `args`, since a person who always allows a program allows its
//! arguments, and never a host: the run stays in its chat's own sandbox and hosts.
//!
//! **What the Allow kept here does.** Where the person had allowed all of the persona's list on
//! this machine, the digest moves for this line alone ([`Written::allow`]). Where anything else of
//! it is waiting, such as a teammate's new host, nothing is allowed: the usual Notice stays up, for
//! all of it. The chat that asked runs either way, on a Run grant of its own.

use std::path::Path;

use super::Which;
use crate::sandbox::persona::{self, Standing};
use crate::secrets::ask::run::Bound;

/// **The line Always allow writes**, as the notice shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// The tool's name: the program's own.
    pub name: String,
    /// The whole line, `name = { ... }`, written exactly so.
    pub text: String,
    /// Some value goes in a variable, as the command asked: a file is safer.
    pub by_env: bool,
}

/// What writing a line came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// **The persona's list, to Allow**, where the person had allowed all of it before this line.
    /// `None` where anything else of it waits on their Allow, which this approves nothing of.
    pub allow: Option<persona::Shown>,
    /// **The vault to give the persona**, where it may not use it yet (ADR 0112): Always allow
    /// alone does. Kept by the caller, which audits it first.
    pub vault: Option<crate::sandbox::local::VaultGrant>,
}

/// `text` as a TOML string.
fn quoted(text: &str) -> String {
    toml_edit::Value::from(text).to_string().trim().to_owned()
}

/// `name` as a TOML key.
fn key_of(name: &str) -> String {
    toml_edit::Key::new(name).display_repr().into_owned()
}

/// **The line for `bound`**, derived from nothing else. `None` where its program has no name, or
/// it hands over nothing, or a hand-over does not read as `VAR=key`.
pub fn line_of(bound: &Bound) -> Option<Line> {
    let name = bound.program.file_name()?.to_str()?.to_owned();
    let mut values: Vec<(&str, &str, &str)> = Vec::new();
    for (specs, how) in [(&bound.env, "env"), (&bound.file, "file")] {
        for spec in specs {
            let (var, key) = spec.split_once('=')?;
            values.push((var, key, how));
        }
    }
    let by_env = values.iter().any(|(_, _, how)| *how == "env");
    let vault = quoted(&bound.vault);
    let body = match values.as_slice() {
        [] => return None,
        [(var, key, how)] => format!(
            "vault = {vault}, as = {}, key = {}",
            quoted(&format!("{var} {how}")),
            quoted(key)
        ),
        many => {
            let each: Vec<String> = many
                .iter()
                .map(|(var, key, how)| {
                    format!(
                        "{} = {{ key = {}, as = {} }}",
                        key_of(var),
                        quoted(key),
                        quoted(how)
                    )
                })
                .collect();
            format!("vault = {vault}, values = {{ {} }}", each.join(", "))
        }
    };
    Some(Line {
        text: format!("{} = {{ {body} }}", key_of(&name)),
        name,
        by_env,
    })
}

/// `text` with `line` added to `persona`'s tools, keeping everything else as it was written.
/// Refused where the file is not TOML, or the persona already has a tool of that name.
fn added(text: &str, persona: &str, line: &Line) -> Result<String, String> {
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| {
        format!(
            "the project's file is not valid TOML ({}), so nothing was written",
            crate::shown::short(e.message())
        )
    })?;
    let value = line
        .text
        .split_once(" = ")
        .map(|(_, value)| value)
        .ok_or("the line reads as no tool, so nothing was written")?;
    let mut value: toml_edit::Value = value
        .parse()
        .map_err(|_| "the line reads as no tool, so nothing was written")?;
    value.decor_mut().set_prefix(" ");
    let not_a_table = || {
        format!("the project's [sandbox.personas.{persona}] is not a table, so nothing was written")
    };
    let mut inline = false;
    let mut at: &mut dyn toml_edit::TableLike = doc.as_table_mut();
    for step in ["sandbox", "personas", persona] {
        let item = at.entry(step).or_insert_with(|| {
            let mut table = toml_edit::Table::new();
            table.set_implicit(true);
            toml_edit::Item::Table(table)
        });
        inline = item.is_inline_table();
        at = item.as_table_like_mut().ok_or_else(not_a_table)?;
    }
    let already = || {
        format!(
            "{persona} already has a tool named {}, so nothing was written",
            line.name
        )
    };
    if at.contains_key("tools") {
        let tools = at
            .get_mut("tools")
            .and_then(toml_edit::Item::as_table_like_mut)
            .ok_or_else(not_a_table)?;
        if tools.contains_key(&line.name) {
            return Err(already());
        }
        tools.insert(&line.name, toml_edit::Item::Value(value));
    } else if inline {
        // A persona written inline holds its tools inline.
        let mut tools = toml_edit::InlineTable::new();
        tools.insert(&line.name, value);
        tools.fmt();
        at.insert("tools", toml_edit::value(tools));
    } else {
        let mut tools = toml_edit::Table::new();
        tools.insert(&line.name, toml_edit::Item::Value(value));
        at.insert("tools", toml_edit::Item::Table(tools));
    }
    Ok(doc.to_string())
}

/// Whether nothing of `persona`'s list, as `text` says it, waits on the person here: they allowed
/// all of it, or it asks for nothing.
fn nothing_waits(root: &Path, text: &str, persona: &str) -> bool {
    persona::unjudged_in(root, text)
        .into_iter()
        .all(|one| one.persona != persona || !one.asks() || one.standing == Standing::Allowed)
}

/// **Whether anything else of `persona`'s list waits on the person's Allow** (#5080): then Always
/// allow approves its own line alone, and the persona's usual Notice stays up for the rest. Said
/// so where the project's file can't be read, since nothing could be moved with the line then.
pub fn others_wait(root: &Path, persona: &str) -> bool {
    match super::on_disk(root, Which::Shared) {
        Ok((_, base)) => !nothing_waits(root, &base, persona),
        Err(_) => true,
    }
}

/// **Writes `bound`'s line to `persona`'s tools in `charter.toml`**, and answers what of the
/// persona's list may be allowed with it ([`Written`]). Refused, with a sentence and nothing
/// written or allowed, where the file cannot take it. It is written as the file was read: a file
/// that moved since is refused, so a teammate's change is never allowed unread.
pub fn write(root: &Path, persona: &str, bound: &Bound) -> Result<Written, String> {
    if persona.is_empty() {
        return Err("this chat has no persona to declare a tool for".to_owned());
    }
    // The person's ruling on ADR 0112: whatever asked, a line takes any arguments.
    if !bound.reviewed_for_any_arguments() {
        return Err(format!(
            "purlis has not reviewed {} for any arguments, so nothing was written",
            bound.program_name()
        ));
    }
    let line = line_of(bound).ok_or("the command reads as no tool, so nothing was written")?;
    let (there, base) = super::on_disk(root, Which::Shared)?;
    // Judged on what was read, before anything changes, so the two cannot disagree.
    let waits = !nothing_waits(root, &base, persona);
    let after = added(&base, persona, &line)?;
    let declared = |text: &str| {
        let said = crate::sandbox::Plane::of(Some(text)).said();
        said.policy
            .or(said.unset)
            .and_then(|policy| persona::of(&policy.personas, Some(persona)).cloned())
    };
    let as_written = declared(&after)
        .is_some_and(|grants| grants.tools.iter().any(|tool| tool.name == line.name));
    if !as_written {
        return Err(format!(
            "{} would not read back as a tool of {persona}'s, so nothing was written",
            line.text
        ));
    }
    super::save(root, Which::Shared, there.then_some(base.as_str()), &after)
        .map_err(|why| why.join(" "))?;
    let allow = (!waits)
        .then(|| {
            persona::unjudged_in(root, &after)
                .into_iter()
                .find(|one| one.persona == persona)
        })
        .flatten()
        // Allowed only if the line is in it: a tool policy holds out is no change to allow.
        .filter(|one| one.tools.iter().any(|tool| tool.name == line.name));
    let ctx = crate::secrets::Ctx::new(root, crate::secrets::Env::from_process());
    let vault = crate::secrets::brokered::not_tagged(&ctx, Some(persona), &bound.vault).map(|_| {
        crate::sandbox::local::VaultGrant {
            vault: bound.vault.clone(),
            persona: persona.to_owned(),
        }
    });
    Ok(Written { allow, vault })
}

/// What taking a line out came to ([`remove`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Removed {
    /// **Taken out.** `allow` is the persona's list to Allow without it, where the person had
    /// allowed all of it before, as [`Written::allow`] is for a line written: kept by the caller,
    /// audited first, so the rest of the list stays in force.
    Taken { allow: Option<persona::Shown> },
    /// **Not there as it was written**: taken out already, or changed since by someone, whose
    /// line it now is. Nothing was written.
    Gone,
}

/// A value read as data, its spelling aside: two spellings of one value are one.
fn read_as_data(value: &toml_edit::Value) -> Option<toml::Value> {
    let mut one = toml_edit::Value::clone(value);
    one.decor_mut().clear();
    toml::from_str::<toml::Table>(&format!("v = {one}"))
        .ok()?
        .remove("v")
}

/// The tool `line` declares: its name, and its value read as data. None where it reads as no
/// one tool.
fn tool_of(line: &str) -> Option<(String, toml::Value)> {
    let doc = line.parse::<toml_edit::DocumentMut>().ok()?;
    match doc.iter().collect::<Vec<_>>().as_slice() {
        [(name, toml_edit::Item::Value(value))] => Some(((*name).to_owned(), read_as_data(value)?)),
        _ => None,
    }
}

/// **Takes `line`, which Always allow wrote, out of `persona`'s tools in `charter.toml`** (#6177):
/// the undo of [`write`], through the same file and the same save. Only a tool that still holds
/// exactly that line is taken out; everything else in the file stays as it was written. A tool
/// gone already, or one changed since, is [`Removed::Gone`]: a teammate's edit is never undone.
/// Refused, with a sentence and nothing written, where the file cannot be read or saved, or
/// moved since it was read.
pub fn remove(root: &Path, persona: &str, line: &str) -> Result<Removed, String> {
    let (name, wrote) =
        tool_of(line).ok_or("the line reads as no tool, so nothing was taken out")?;
    let (there, base) = super::on_disk(root, Which::Shared)?;
    if !there {
        return Ok(Removed::Gone);
    }
    let mut doc: toml_edit::DocumentMut = base.parse().map_err(|e: toml_edit::TomlError| {
        format!(
            "the project's file is not valid TOML ({}), so nothing was taken out",
            crate::shown::short(e.message())
        )
    })?;
    let tools = ["sandbox", "personas", persona, "tools"]
        .into_iter()
        .try_fold(
            doc.as_table_mut() as &mut dyn toml_edit::TableLike,
            |at, step| at.get_mut(step)?.as_table_like_mut(),
        );
    let Some(tools) = tools else {
        return Ok(Removed::Gone);
    };
    let held = tools
        .get(name.as_str())
        .and_then(toml_edit::Item::as_value)
        .and_then(read_as_data);
    if held.as_ref() != Some(&wrote) {
        return Ok(Removed::Gone);
    }
    // Judged on what was read, before anything changes, as `write` judges.
    let waits = !nothing_waits(root, &base, persona);
    tools.remove(name.as_str());
    let after = doc.to_string();
    super::save_edit(root, Which::Shared, Some(base.as_str()), &after)
        .map_err(|why| why.join(" "))?;
    let allow = (!waits)
        .then(|| {
            persona::unjudged_in(root, &after)
                .into_iter()
                .find(|one| one.persona == persona)
        })
        .flatten();
    Ok(Removed::Taken { allow })
}

/// **Whether `persona`'s tools hold `line` as Always allow wrote it**, in the project's file as it
/// is now: what a rule made of an Always allow is kept with, so Undo has a line to take out.
pub fn holds(root: &Path, persona: &str, line: &str) -> bool {
    let Some((name, wrote)) = tool_of(line) else {
        return false;
    };
    let Ok((true, base)) = super::on_disk(root, Which::Shared) else {
        return false;
    };
    let Ok(doc) = base.parse::<toml_edit::DocumentMut>() else {
        return false;
    };
    ["sandbox", "personas", persona, "tools", name.as_str()]
        .into_iter()
        .try_fold(doc.as_item(), |at, step| at.get(step))
        .and_then(toml_edit::Item::as_value)
        .and_then(read_as_data)
        .is_some_and(|held| held == wrote)
}

#[cfg(test)]
mod tests;
