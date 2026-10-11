// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Whether this purlis opens a project read-only, as the window says it once in the project's
//! tab (#826, FR-24): which feature is missing and which version has it.
//!
//! The refusal itself is the core's. Every writer of a committed project file asks
//! `compat::read_only` before it writes, so the window has nothing to enforce here; it only
//! has to say why its write buttons are off.

use purlis_core::compat::{self, Compat, Why};

use crate::planes::{PlaneId, Planes};

/// Why this purlis reads a project and writes nothing to it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ReadOnlyProject {
    /// The one sentence the project's tab says, and every disabled write action repeats: what
    /// is missing, which version has it, and what to do.
    pub sentence: String,
    /// The feature the project requires that this purlis lacks. Null when the project is
    /// read-only for another reason (a manifest it cannot read, a schema it cannot place).
    pub feature: Option<String>,
    /// The first purlis version that has [`Self::feature`], as the project's manifest names
    /// it. Null when it names none, or when no feature is missing.
    pub since: Option<String>,
    /// Whether Edit as TOML may still replace the project's settings file whole: only where
    /// that file is the manifest this purlis cannot read as TOML, which that editor is how the
    /// person mends (#1759). Every other write stays off.
    pub mendable: bool,
}

/// What the window says for what the core answered, or `None` when this purlis may write.
fn read_only(compat: Compat) -> Option<ReadOnlyProject> {
    let Compat::ReadOnly(why) = compat else {
        return None;
    };
    let (feature, since) = match &why {
        Why::Missing { feature, since } => (Some(feature.clone()), since.clone()),
        _ => (None, None),
    };
    Some(ReadOnlyProject {
        sentence: why.to_string(),
        feature,
        since,
        mendable: matches!(why, Why::ManifestUnreadable { .. }),
    })
}

/// Why this purlis opens `plane` read-only, or null when it may write it.
///
/// Read off the disk on every ask, never cached: a pull that adds a `requires` entry makes the
/// project read-only from that moment, and the window asks again when the project changes.
#[tauri::command]
#[specta::specta]
pub fn project_read_only(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Option<ReadOnlyProject>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    Ok(read_only(compat::read(&root)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_project_this_purlis_may_write_has_no_line() {
        assert_eq!(read_only(Compat::Writable), None);
    }

    #[test]
    fn the_line_names_the_missing_feature_and_the_version_that_has_it() {
        let line = read_only(Compat::ReadOnly(Why::Missing {
            feature: "memory-proposals".to_owned(),
            since: Some("0.9.0".to_owned()),
        }))
        .expect("a missing feature makes the project read-only");
        assert_eq!(line.feature.as_deref(), Some("memory-proposals"));
        assert_eq!(line.since.as_deref(), Some("0.9.0"));
        assert!(!line.mendable);
        assert!(
            line.sentence.contains("memory-proposals")
                && line.sentence.contains("purlis 0.9.0 or later has it")
                && line.sentence.contains("writes nothing to it"),
            "{}",
            line.sentence
        );
    }

    #[test]
    fn a_project_read_only_for_another_reason_says_why_without_a_feature() {
        let line = read_only(Compat::ReadOnly(Why::SchemaTooNew {
            file: "purlis.toml",
            found: 9,
        }))
        .expect("a schema this purlis does not understand makes it read-only");
        assert_eq!((line.feature, line.since), (None, None));
        assert!(!line.mendable, "only an unreadable manifest is mended");
        assert!(
            line.sentence.contains("declares schema 9"),
            "{}",
            line.sentence
        );
    }

    #[test]
    fn only_a_manifest_that_does_not_read_is_mendable_under_edit_as_toml() {
        let line = read_only(Compat::ReadOnly(Why::ManifestUnreadable {
            file: "purlis.toml",
            detail: "expected `=`".to_owned(),
        }))
        .expect("an unreadable manifest makes the project read-only");
        assert!(line.mendable);
    }
}
