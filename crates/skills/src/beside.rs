// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The file beside a skill's folder** (ADR 0099 §1): `<folder>.toml`, what purlis alone knows
//! about the skill (its grant, lifecycle state and owners). The one reader and the one writer of
//! it: read following no link, at most [`MOST`] bytes, as TOML; written keeping every key and
//! comment a change does not touch.

use std::path::Path;

/// The most bytes of a `<folder>.toml` purlis reads: it holds a few short keys.
pub const MOST: u64 = 64 * 1024;

/// The name of the file beside `folder`, as a person is told it: `deploy.toml`.
fn file_name(folder: &Path) -> String {
    crate::beside(folder)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The table in the file beside `folder`, read without following a link, or `None` where there
/// is no such file; why not, as the end of a sentence, where it cannot be read.
pub(crate) fn table(folder: &Path) -> Result<Option<toml::Table>, String> {
    let file = crate::beside(folder);
    if std::fs::symlink_metadata(&file).is_err_and(|why| why.kind() == std::io::ErrorKind::NotFound)
    {
        return Ok(None);
    }
    let text = crate::handed::read_plain(&file, MOST).map_err(|why| why.to_string())?;
    text.parse::<toml::Table>()
        .map(Some)
        .map_err(|why| format!("it is not TOML: {}", first_line(&why.to_string())))
}

/// **Changes `<folder>.toml` by `edit`**, keeping every key and comment it does not touch: the
/// one writer of the file beside a skill's folder (its grant, lifecycle state and owners). A file
/// left empty is removed. A file that is not TOML, or not a plain file, is refused and left as it
/// was. The file is written beside itself and renamed over, so a reader never sees half of it,
/// and the rename replaces a link rather than writing through it.
pub(crate) fn edit(
    folder: &Path,
    edit: impl FnOnce(&mut toml_edit::DocumentMut),
) -> Result<(), String> {
    let file = crate::beside(folder);
    let name = file_name(folder);
    let text = match std::fs::symlink_metadata(&file) {
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => String::new(),
        _ => crate::handed::read_plain(&file, MOST)
            .map_err(|why| format!("{name} could not be read: {why}, so nothing changed"))?,
    };
    let mut doc = text.parse::<toml_edit::DocumentMut>().map_err(|why| {
        format!(
            "{name} is not TOML ({}), so nothing changed: mend it by hand first",
            first_line(&why.to_string())
        )
    })?;
    edit(&mut doc);
    let written = doc.to_string();
    if written.trim().is_empty() {
        return match std::fs::remove_file(&file) {
            Ok(()) => Ok(()),
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(why) => Err(format!("{name} could not be removed ({why})")),
        };
    }
    let dir = file.parent().unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(dir)
        .map_err(|why| format!("{name} could not be written ({why}), so nothing changed"))?;
    std::io::Write::write_all(&mut temp, written.as_bytes())
        .map_err(|why| format!("{name} could not be written ({why}), so nothing changed"))?;
    temp.persist(&file)
        .map(|_| ())
        .map_err(|why| format!("{name} could not be written ({why}), so nothing changed"))
}

/// The first line of `text` that says anything.
pub(crate) fn first_line(text: &str) -> &str {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
}
