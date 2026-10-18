// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The project's browser sites, written** (#2443): `[sandbox] browser-sites` in the committed
//! file, changed by its keys under the lock every writer of it takes, every other line kept
//! ([`crate::sandbox::browser_sites`] says what it means).

use std::path::Path;

use super::Which;
use crate::sandbox::TABLE;
use crate::sandbox::browser_sites::KEY;

/// **Changes the committed file's browser sites by `how`**, which answers whether it changed
/// them: written then, with the key taken out once it is empty. Answers whether it changed, or
/// why nothing was written, in a sentence.
pub(crate) fn edit(
    root: &Path,
    how: impl FnOnce(&mut toml_edit::Array) -> bool,
) -> Result<bool, String> {
    let (there, text) = super::on_disk(root, Which::Shared)?;
    let file = Which::Shared.file_at(root);
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| {
        format!(
            "{file} is not valid TOML ({}), so nothing was written",
            crate::shown::short(e.message())
        )
    })?;
    let table = doc
        .entry(TABLE)
        .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()))
        .as_table_like_mut()
        .ok_or_else(|| format!("{TABLE} in {file} is not a table, so nothing was written"))?;
    let list = table
        .entry(KEY)
        .or_insert(toml_edit::value(toml_edit::Array::new()))
        .as_array_mut()
        .ok_or_else(|| {
            format!("{TABLE}.{KEY} in {file} is not written as a list, so nothing was written")
        })?;
    if !how(list) {
        return Ok(false);
    }
    list.fmt();
    if list.is_empty() {
        table.remove(KEY);
    }
    super::save_edit(
        root,
        Which::Shared,
        there.then_some(text.as_str()),
        &doc.to_string(),
    )
    .map_err(|why| why.join(" "))?;
    Ok(true)
}
