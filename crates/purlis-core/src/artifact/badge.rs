// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The number an artifact's icon shows** (spec #5948 story 22, #5965): open issues, say, read
//! on the activity bar without opening the page.
//!
//! - **Only the data declares it**: `data.json`'s top-level [`KEY`], a whole number from 0 to
//!   [`MOST`]. Nothing else makes one: not the page, not a count purlis works out, not a number
//!   written as text, a fraction or a negative. A zero is a number the data declared, and the
//!   window draws no badge for it, as it draws no count for nothing.
//! - **It is read when the freshness is** ([`super::fresh`]): the pins are read again on any
//!   change to the workspace's artifacts, so a regenerated `data.json` brings its new number, and
//!   the icon dims it with the rest once the data is stale.
//!
//! A link is never followed, a file over [`READ_MOST`] declares nothing, and nothing here writes.

use std::path::Path;

use super::DATA_FILE;

/// The key of `data.json` that declares the number its icon shows.
pub const KEY: &str = "badge";

/// The largest number a badge holds; a larger one declares nothing.
pub const MOST: u32 = 999_999;

/// The most bytes of `data.json` read for its badge, as for its declared date.
const READ_MOST: u64 = 8 * 1024 * 1024;

/// **The number the data of the artifact in `folder` declares** (a folder
/// [`super::resolve`] answered), or nothing when it declares none that reads.
pub fn of(folder: &Path) -> Option<u32> {
    let path = folder.join(DATA_FILE);
    let found = std::fs::symlink_metadata(&path).ok()?;
    if !found.file_type().is_file() || found.len() > READ_MOST {
        return None;
    }
    let bytes = crate::contain::read_no_link(folder, &path).ok()?;
    let data: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let n = u32::try_from(data.get(KEY)?.as_u64()?).ok()?;
    (n <= MOST).then_some(n)
}

/// **The badge of pinned folder `folder` of `workspace`**, when the folder resolves
/// ([`super::resolve`]) and its data declares one.
pub fn of_pin(workspace: &Path, folder: &str) -> Option<u32> {
    of(&super::resolve(workspace, folder).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(data: Option<&str>) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<p>page</p>").unwrap();
        if let Some(text) = data {
            std::fs::write(dir.path().join("data.json"), text).unwrap();
        }
        dir
    }

    #[test]
    fn the_badge_is_the_whole_number_data_json_declares() {
        for (data, badge) in [
            (r#"{"badge": 12, "open": [1, 2]}"#, 12),
            (r#"{"badge": 0}"#, 0),
            (r#"{"updated": "2026-11-08", "badge": 999999}"#, 999_999),
        ] {
            let dir = folder(Some(data));
            assert_eq!(of(dir.path()), Some(badge), "{data}");
        }
    }

    #[test]
    fn anything_but_a_whole_number_in_range_declares_no_badge() {
        for data in [
            r#"{"badge": "12"}"#,
            r#"{"badge": 12.5}"#,
            r#"{"badge": 12.0}"#,
            r#"{"badge": -1}"#,
            r#"{"badge": 1000000}"#,
            r#"{"badge": true}"#,
            r#"{"badge": null}"#,
            r#"{"badge": [12]}"#,
            r#"{"open": 12}"#,
            r#"{"data": {"badge": 12}}"#,
            r#"[{"badge": 12}]"#,
            "12",
            "not json",
        ] {
            let dir = folder(Some(data));
            assert_eq!(of(dir.path()), None, "{data}");
        }
    }

    #[test]
    fn a_page_with_no_data_has_no_badge_whatever_its_page_says() {
        let dir = folder(None);
        std::fs::write(
            dir.path().join("index.html"),
            r#"<script type="application/json" id="purlis">{"badge": 12}</script>"#,
        )
        .unwrap();
        assert_eq!(of(dir.path()), None);
    }

    #[test]
    fn a_pin_s_badge_is_read_from_its_folder_and_never_from_outside_it() {
        let workspace = tempfile::tempdir().unwrap();
        let pinned = workspace.path().join("artifacts").join("backlog-state");
        std::fs::create_dir_all(&pinned).unwrap();
        std::fs::write(pinned.join("index.html"), "<p>page</p>").unwrap();
        std::fs::write(pinned.join("data.json"), r#"{"badge": 7}"#).unwrap();
        std::fs::write(workspace.path().join("data.json"), r#"{"badge": 9}"#).unwrap();
        assert_eq!(of_pin(workspace.path(), "backlog-state"), Some(7));
        assert_eq!(of_pin(workspace.path(), ".."), None);
        assert_eq!(of_pin(workspace.path(), "gone"), None);
    }

    #[test]
    fn a_number_under_data_is_not_the_badge() {
        let dir = folder(None);
        std::fs::create_dir(dir.path().join("data")).unwrap();
        std::fs::write(dir.path().join("data/rows.json"), r#"{"badge": 12}"#).unwrap();
        assert_eq!(of(dir.path()), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_data_json_that_is_a_link_is_never_followed() {
        let dir = folder(None);
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("data.json"), r#"{"badge": 12}"#).unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("data.json"),
            dir.path().join("data.json"),
        )
        .unwrap();
        assert_eq!(of(dir.path()), None);
    }

    #[test]
    fn a_data_json_too_large_to_read_declares_no_badge() {
        let dir = folder(None);
        let padding = " ".repeat(usize::try_from(READ_MOST).unwrap());
        std::fs::write(
            dir.path().join("data.json"),
            format!(r#"{{"badge": 12}}{padding}"#),
        )
        .unwrap();
        assert_eq!(of(dir.path()), None);
    }
}
