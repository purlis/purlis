// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A save blocked by what looks like a credential, in the Saving view (ADR 0051): where it is
//! blocked, and **Open the file**.
//!
//! The standing names each file the next save would refuse (`planegit::SecretAt`): its path,
//! line and kind, and what the scan matched with the value hidden. This re-shapes them for the
//! window and opens one in the person's editor. The value itself is never read here: the core
//! sends none.
//!
//! **Open the file is the window's alone** (`purlis_session_protocol::ui::WINDOW_ONLY`): it
//! starts a program of the person's. It opens only a file the project's own standing names,
//! at the line the standing gives, so nothing the window sends chooses what is opened.

use std::path::Path;

use purlis_core::planegit;
use purlis_core::youreditor::{self, Launch};

use crate::piecefiles::YourEditor;
use crate::planes::{PlaneId, Planes};

/// One changed file the next save would refuse for what looks like a credential.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SecretAt {
    /// From the top of the project.
    pub path: String,
    /// The line the value starts on; `null` for one read only through the file's escapes.
    pub line: Option<u32>,
    /// The kind of value, in the scan's words.
    pub kind: String,
    /// What the scan matched, with the value hidden (`token = ********`); `null` when there is
    /// nothing to show. Never the value.
    pub shown: Option<String>,
}

/// The standing's findings, for the window: `None` when there is none.
pub fn shown(secrets: &[planegit::SecretAt]) -> Option<Vec<SecretAt>> {
    (!secrets.is_empty()).then(|| {
        secrets
            .iter()
            .map(|one| SecretAt {
                path: one.path.clone(),
                line: one.line.and_then(|line| u32::try_from(line).ok()),
                kind: one.kind.to_owned(),
                shown: one.shown.clone(),
            })
            .collect()
    })
}

/// What starts your editor on the file `path` names among `secrets`, at its finding's line.
fn launch_of(
    root: &Path,
    secrets: &[planegit::SecretAt],
    path: &str,
    editor: YourEditor,
    var: &dyn Fn(&str) -> Option<String>,
) -> Result<Launch, String> {
    let (file, line) = planegit::blocked_file(root, secrets, path)?;
    youreditor::launch(editor.into(), &file, line, var).map_err(|refused| refused.to_string())
}

/// **Open the file** on a blocked save's finding in the Saving view: the file in your editor,
/// at the line the save is blocked on.
// Placed by the project's own standing, never a path the window made up, and handed to the
// editor as `open_harness_declaration` hands one. Not a doc comment past the first, because
// the generated bindings carry those.
#[tauri::command]
#[specta::specta]
pub async fn open_blocked_file(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    path: String,
    editor: YourEditor,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;
    let root = planes.held(&plane)?.root().to_path_buf();
    let launch = tauri::async_runtime::spawn_blocking(move || {
        // The standing read now, not the one the view drew: a file mended since is not opened
        // as a finding.
        let secrets = planegit::standing(&root).secrets;
        launch_of(&root, &secrets, &path, editor, &purlis_core::envvar::var)
    })
    .await
    .map_err(|err| format!("opening the file did not finish: {err}"))??;
    match launch {
        Launch::Url(url) => app
            .opener()
            .open_url(&url, None::<&str>)
            .map_err(|e| format!("the system did not open {url}: {e}")),
        Launch::Program { program, args } => crate::piecefiles::started(program, args).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(path: &str, line: Option<usize>) -> planegit::SecretAt {
        planegit::SecretAt {
            path: path.to_owned(),
            line,
            kind: "credential assignment",
            shown: Some("token = ********".to_owned()),
        }
    }

    fn no_var(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn the_window_gets_each_finding_and_nothing_when_there_is_none() {
        assert_eq!(shown(&[]), None);
        assert_eq!(
            shown(&[finding("notes/a.md", Some(5)), finding("b.json", None)]),
            Some(vec![
                SecretAt {
                    path: "notes/a.md".into(),
                    line: Some(5),
                    kind: "credential assignment".into(),
                    shown: Some("token = ********".into()),
                },
                SecretAt {
                    path: "b.json".into(),
                    line: None,
                    kind: "credential assignment".into(),
                    shown: Some("token = ********".into()),
                },
            ])
        );
    }

    #[test]
    fn open_the_file_opens_a_finding_at_its_line_and_no_other_path() {
        let dir = tempfile::tempdir().expect("a directory");
        std::fs::create_dir_all(dir.path().join("notes")).expect("a folder");
        std::fs::write(dir.path().join("notes/a.md"), "x\n").expect("written");
        std::fs::write(dir.path().join("notes/other.md"), "x\n").expect("written");
        let secrets = [finding("notes/a.md", Some(5))];

        let launch = launch_of(dir.path(), &secrets, "notes/a.md", YourEditor::Zed, &no_var);
        match launch {
            Ok(Launch::Url(url)) => {
                assert!(url.starts_with("zed://file"), "{url}");
                assert!(url.ends_with("notes/a.md:5:1"), "{url}");
            }
            other => panic!("{other:?}"),
        }
        for not_a_finding in ["notes/other.md", "../outside.md", "/etc/hosts"] {
            let refused = launch_of(
                dir.path(),
                &secrets,
                not_a_finding,
                YourEditor::Zed,
                &no_var,
            )
            .unwrap_err();
            assert!(
                refused.contains("only a file the save is blocked in"),
                "{refused}"
            );
        }
    }
}
