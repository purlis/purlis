// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The project's skill sources, written** (SK-10, #2773; ADR 0100 §1): `[[skill_sources]]` in
//! the shared file or this machine's, changed by its tables under the lock every writer of the
//! file takes, every other line kept ([`crate::skills::source`] says what one means).

use std::path::Path;

use super::Which;
use crate::skills::source::TABLE;

/// **Changes `which` file's skill sources by `how`**, which answers whether it changed them:
/// written then, with the table taken out once it is empty. Answers whether it changed, or why
/// nothing was written, in a sentence.
pub(crate) fn edit(
    root: &Path,
    which: Which,
    how: impl FnOnce(&mut toml_edit::ArrayOfTables) -> bool,
) -> Result<bool, String> {
    let (there, text) = super::on_disk(root, which)?;
    let file = which.file_at(root);
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| {
        format!(
            "{file} is not valid TOML ({}), so nothing was written",
            crate::shown::short(e.message())
        )
    })?;
    let list = doc
        .entry(TABLE)
        .or_insert_with(|| toml_edit::Item::ArrayOfTables(toml_edit::ArrayOfTables::new()))
        .as_array_of_tables_mut()
        .ok_or_else(|| {
            format!(
                "{TABLE} in {file} is not written as [[{TABLE}]] tables, so nothing was written"
            )
        })?;
    if !how(list) {
        return Ok(false);
    }
    if list.is_empty() {
        doc.remove(TABLE);
    }
    super::save_edit(
        root,
        which,
        there.then_some(text.as_str()),
        &doc.to_string(),
    )
    .map_err(|why| why.join(" "))?;
    Ok(true)
}
