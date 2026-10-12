// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `extension manifests` row (#1802): an extension on this machine whose folder still holds
//! its manifest under the old name, `charter-extension.json`. purlis reads that name for one
//! more release ([`crate::names::EXTENSION_MANIFEST`]), so the row says which extensions to
//! rename before it stops.

use super::{DISPLAY_LIMIT, Doctor, Row, one_line, short_path};
use crate::names::Found;

/// The remedy, said once for every extension the row names.
const RENAME: &str = "Rename `charter-extension.json` to `purlis-extension.json`; the old name \
                      is read for one more release.";

/// The row, only where an extension in this machine's record (installed, or one of the app's
/// own) is read under the old manifest name: anywhere else, the rows printed before.
pub(super) fn renamed_manifests(d: &Doctor) -> Option<Row> {
    const NAME: &str = "extension manifests";
    let (config_root, built_in) = d.extensions.as_ref()?;
    let loaded = crate::extension::read(config_root, built_in);
    let old: Vec<String> = loaded
        .registry
        .entries
        .iter()
        .filter(|(_, entry)| crate::extension::manifest_in(&entry.path).found == Found::Old)
        .map(|(id, entry)| {
            format!(
                "{} ({})",
                one_line(id, DISPLAY_LIMIT),
                short_path(&entry.path)
            )
        })
        .collect();
    if old.is_empty() {
        return None;
    }
    Some(Row::warn(
        NAME,
        format!(
            "{} still read under the old manifest name: {}",
            if old.len() == 1 {
                "1 extension is".to_owned()
            } else {
                format!("{} extensions are", old.len())
            },
            old.join(", ")
        ),
        RENAME,
    ))
}
