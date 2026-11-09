// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Artifacts in the palette's search** (ADR 0120, spec #5948, #5968): the project's pinned
//! artifacts whose pages say what was typed, found by the core
//! ([`purlis_core::artifact::search`]) in the project's search index, or in its files where
//! there is no index yet. A row the palette draws from one opens the artifact's own tab.

use purlis_core::artifact::search;
use serde::Serialize;

use crate::planes::{PlaneId, Planes};

/// The most artifacts one search answers: the palette draws a short group, never a page.
const MOST: usize = 20;

/// One artifact found, as the palette draws it and the window opens it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct ArtifactFound {
    /// The workspace it is pinned to.
    pub workspace: String,
    /// Its folder in the workspace's `artifacts/`: with the workspace, what its tab is keyed by.
    pub folder: String,
    /// The pin's name.
    pub name: String,
}

/// **The pinned artifacts of project `plane`, in every workspace, that say any word of
/// `query`**, best first: at most [`MOST`].
#[tauri::command]
#[specta::specta]
pub async fn artifact_search(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    query: String,
) -> Result<Vec<ArtifactFound>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || found(&root, &query))
        .await
        .map_err(|e| format!("purlis could not search the artifacts: {e}"))
}

fn found(root: &std::path::Path, query: &str) -> Vec<ArtifactFound> {
    search::find(root, None, query)
        .found
        .into_iter()
        .take(MOST)
        .map(|one| ArtifactFound {
            workspace: one.workspace,
            folder: one.folder,
            name: one.name,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pinned_artifact_is_found_by_its_words_with_what_opens_its_tab() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
        let ws = root.join("workspaces/alpha");
        std::fs::create_dir_all(ws.join("artifacts/board")).unwrap();
        std::fs::write(
            ws.join("artifacts/board/index.html"),
            "<p>Release train</p>",
        )
        .unwrap();
        purlis_core::artifact::pin(
            &root,
            &ws,
            &purlis_core::artifact::Request {
                folder: "board".into(),
                name: Some("Board".into()),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(
            found(&root, "train"),
            [ArtifactFound {
                workspace: "alpha".into(),
                folder: "board".into(),
                name: "Board".into(),
            }]
        );
        assert_eq!(found(&root, "absent"), []);
    }
}
