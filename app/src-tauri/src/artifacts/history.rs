// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An artifact as it was, and two of its versions compared** (spec #5948 story 47, #5966),
//! from its tab's bar.
//!
//! - **Its versions** ([`artifact_history`]): each Regenerate this machine kept the data of
//!   (the newest [`history::MOST`]), and each commit of its template
//!   ([`purlis_core::artifact::history`]).
//! - **As of a version** ([`artifact_open_as_of`]): the template committed by then and the data
//!   kept then, served to a webview of its own as any artifact is (the same CSP, no network,
//!   its own origin's storage apart from today's). The version is named by its id and read
//!   again here, never taken from the window as a commit or a path.
//! - **It is what it was, and does nothing now**: its page reads only the data kept then; a
//!   catalogue query, a subscription or an offer is answered `not-approved`, since none of them
//!   can be as of a date ([`super::still_runs`]). Its script runs only when this machine's
//!   approval is of that very template; otherwise it is shown without it, and **nothing in its
//!   bar can approve it** ([`super::artifact_approve`] refuses one), since an approval is of the
//!   folder's one template and would displace today's. It never reloads from the folder.
//! - **Compare** ([`artifact_compare`]): the template files and the data values that changed
//!   from one version to another, or to now.
//!
//! Every command here is the window's alone (`WINDOW_ONLY`): no link serves it and no chat runs
//! it.

use std::path::PathBuf;

use purlis_core::artifact::approval::{self, Standing};
use purlis_core::artifact::history;
use purlis_core::artifact::{self, serve};
use serde::Serialize;

use super::{
    ArtifactOpened, ArtifactPlace, ArtifactTheme, Artifacts, Of, Served, blocking, declared_by,
    lay, read_pinned, workspace_of,
};
use crate::planes::{PlaneId, Planes};

/// **What an as-of page's data reads are answered from**: the folder its data was kept in, or
/// none when no data was kept by then (every read answers `data: null`).
#[derive(Clone, Debug)]
pub(crate) struct AsOf {
    pub(crate) data: Option<PathBuf>,
}

/// The standing an as-of page's bar is told: its own when its script runs or it has none, else
/// `as-of`, which offers no approval.
const SHOWN_WITHOUT_SCRIPT: &str = "as-of";

/// One version of an artifact, as its History lists it.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ArtifactVersion {
    /// What opening it as of, or comparing it, names.
    pub id: String,
    /// When, UTC, as the window shows it.
    pub at: String,
    /// `regenerated` or `template-changed`.
    pub kind: String,
    /// Who committed it, for a template's version.
    pub by: Option<String>,
    /// The commit's subject, for a template's version.
    pub said: Option<String>,
}

/// An artifact's History.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ArtifactHistory {
    /// Newest first.
    pub versions: Vec<ArtifactVersion>,
    /// How many Regenerates' data this machine keeps per artifact.
    pub kept_most: u32,
}

/// What an artifact opened as of a version is.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ArtifactOpenedAsOf {
    pub opened: ArtifactOpened,
    /// When the version is, UTC.
    pub at: String,
    /// The commit its template was read at, abbreviated; none when no commit held it by then,
    /// and it is today's.
    pub template: Option<String>,
    /// When its data was kept, UTC; none when no data was kept by then.
    pub data_at: Option<String>,
}

/// A template file that differs between two versions.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ArtifactFileChange {
    pub path: String,
    /// `added`, `removed` or `changed`.
    pub how: String,
}

/// A data value that differs between two versions.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ArtifactValueChange {
    /// Where it is: the data file and its path in the JSON, `data.json › open`.
    pub path: String,
    /// Its JSON before and after, cut short; none where there was none.
    pub before: Option<String>,
    pub after: Option<String>,
}

/// What changed between two versions.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ArtifactCompared {
    pub template: Vec<ArtifactFileChange>,
    pub data: Vec<ArtifactValueChange>,
    /// More changed than are listed.
    pub cut: bool,
}

/// A time in seconds since 1970, as the window shows it.
fn said_at(at: i64) -> String {
    chrono::DateTime::from_timestamp(at, 0)
        .map(|at| at.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_default()
}

/// The workspace's folder of `workspace`, and the pinned folder's own, found inside it.
fn pinned_dirs(
    planes: &Planes,
    plane: &PlaneId,
    workspace: &str,
    folder: &str,
) -> Result<(PathBuf, PathBuf), String> {
    let at = workspace_of(planes, plane, workspace)?;
    artifact::pinned(&at, folder)?;
    let real = artifact::resolve(&at, folder)?;
    Ok((at, real))
}

/// **A pinned artifact's versions**, newest first.
#[tauri::command]
#[specta::specta]
pub async fn artifact_history(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    folder: String,
) -> Result<ArtifactHistory, String> {
    let (at, real) = pinned_dirs(&planes, &plane, &workspace, &folder)?;
    let config = purlis_core::machine::config_root();
    blocking(move || {
        let versions = history::versions(config.as_deref(), &at, &folder, &real)
            .into_iter()
            .map(|version| ArtifactVersion {
                id: version.id,
                at: said_at(version.at),
                kind: version.kind.as_str().to_owned(),
                by: version.by,
                said: version.said,
            })
            .collect();
        Ok(ArtifactHistory {
            versions,
            kept_most: u32::try_from(history::MOST).unwrap_or(u32::MAX),
        })
    })
    .await
}

/// **Opens a pinned artifact as of version `version`** over its tab's frame: the template and
/// data of then, read here, where it stands on this machine read with it.
// A command's arguments are what the window passes by name, as `artifact_open`'s are.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
#[specta::specta]
pub async fn artifact_open_as_of(
    window: tauri::Window,
    planes: tauri::State<'_, Planes>,
    artifacts: tauri::State<'_, Artifacts>,
    plane: PlaneId,
    workspace: String,
    folder: String,
    version: String,
    place: ArtifactPlace,
    theme: ArtifactTheme,
) -> Result<ArtifactOpenedAsOf, String> {
    let sheet = serve::theme_sheet(&theme.appearance, &theme.values)?;
    let (at, real) = pinned_dirs(&planes, &plane, &workspace, &folder)?;
    let config = purlis_core::machine::config_root();
    let dir = at.clone();
    let read_folder = folder.clone();
    let (then, pin, standing, host) = blocking(move || {
        let then = history::as_of(config.as_deref(), &at, &read_folder, &real, &version)?;
        let (pin, _) = read_pinned(&at, &read_folder)?;
        // History's own origin, and the approval's when that version's script runs here, so no
        // two approvals share its storage (ADR 0122, amended 2026-11-10); from the one read of
        // the record its standing is, so the two agree (#6120).
        let (standing, host) = approval::standing_and_host(
            config.as_deref(),
            &at,
            &read_folder,
            &pin,
            &then.snapshot,
            "as-of",
        );
        Ok((then, pin, standing, host))
    })
    .await?;
    let shown = shown_standing(standing);
    let declared = declared_by(&then.snapshot);
    let generated = then.data.is_some()
        || declared
            .as_ref()
            .is_none_or(|declared| declared.data.is_empty());
    let of = Of {
        plane,
        workspace,
        dir,
        folder,
        pin,
        standing,
        then: Some(AsOf {
            data: then.data.clone(),
        }),
    };
    let served = Served {
        window: window.label().to_owned(),
        host,
        snapshot: then.snapshot,
        of,
        theme: sheet,
        declared,
        load: None,
        begun: 0,
    };
    let label = lay(&window, &artifacts, served, place)?;
    Ok(ArtifactOpenedAsOf {
        opened: ArtifactOpened {
            label,
            standing: shown.to_owned(),
            changed_by: None,
            generated,
            // Nothing it declared is called as of a date, so none is listed.
            queries: Vec::new(),
            plane_granted: false,
        },
        at: said_at(then.at),
        template: then.template.map(|committed| committed.commit),
        data_at: then.data_at.map(said_at),
    })
}

/// What an as-of page's bar is told of its script: its own standing when it runs or it has
/// none (or none runs here); else [`SHOWN_WITHOUT_SCRIPT`], which asks for nothing.
fn shown_standing(standing: Standing) -> &'static str {
    match standing {
        Standing::Approved | Standing::ScriptFree | Standing::NotHere => standing.as_str(),
        Standing::New | Standing::Changed | Standing::PinChanged => SHOWN_WITHOUT_SCRIPT,
    }
}

/// **What changed from version `from` to version `to`** of a pinned artifact; either may be
/// `now`.
#[tauri::command]
#[specta::specta]
pub async fn artifact_compare(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    folder: String,
    from: String,
    to: String,
) -> Result<ArtifactCompared, String> {
    let (at, real) = pinned_dirs(&planes, &plane, &workspace, &folder)?;
    let config = purlis_core::machine::config_root();
    blocking(move || {
        let compared = history::compare(config.as_deref(), &at, &folder, &real, &from, &to)?;
        Ok(ArtifactCompared {
            template: compared
                .template
                .into_iter()
                .map(|change| ArtifactFileChange {
                    path: change.path,
                    how: change.how.as_str().to_owned(),
                })
                .collect(),
            data: compared
                .data
                .into_iter()
                .map(|change| ArtifactValueChange {
                    path: change.path,
                    before: change.before,
                    after: change.after,
                })
                .collect(),
            cut: compared.cut,
        })
    })
    .await
}

/// **The answer to an as-of page asking for data file `name`**: from the data kept then, or
/// `data: null` when none was.
pub(super) fn read_then(
    as_of: &AsOf,
    declared: Option<&artifact::data::Declared>,
    name: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> purlis_core::catalogue::Answer {
    use purlis_core::catalogue::{Answer, ErrorKind};
    match &as_of.data {
        Some(kept) => artifact::data::read(kept, declared, name, now),
        None if declared.is_some_and(|d| d.data.iter().any(|n| n == name)) => {
            Answer::data(serde_json::Value::Null, now)
        }
        None => Answer::error(ErrorKind::NotApproved, now),
    }
}

/// Keeps the data a Regenerate just placed, for History; a failure to keep it is not the run's.
pub(crate) fn keep_after_regenerate(
    config: Option<&std::path::Path>,
    workspace: &std::path::Path,
    folder: &str,
    real: &std::path::Path,
    declared: &artifact::data::Declared,
) {
    let Some(config) = config else {
        return;
    };
    if let Err(why) = history::keep(
        config,
        workspace,
        folder,
        real,
        declared,
        chrono::Utc::now(),
    ) {
        tracing::warn!("purlis: the data Regenerate wrote for '{folder}' was not kept ({why})");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declared(names: &[&str]) -> artifact::data::Declared {
        artifact::data::Declared {
            api: 1,
            data: names.iter().map(|n| (*n).to_owned()).collect(),
            queries: Vec::new(),
        }
    }

    #[test]
    fn an_as_of_page_reads_the_data_kept_then_and_not_todays() {
        let dir = tempfile::tempdir().unwrap();
        let kept = dir.path().join("kept");
        std::fs::create_dir_all(&kept).unwrap();
        std::fs::write(kept.join("data.json"), r#"{"open": 1}"#).unwrap();
        let as_of = AsOf { data: Some(kept) };
        let answer = read_then(
            &as_of,
            Some(&declared(&["data.json"])),
            "data.json",
            chrono::Utc::now(),
        );
        assert_eq!(answer.to_json()["data"], serde_json::json!({"open": 1}));
    }

    #[test]
    fn an_as_of_page_with_no_data_kept_reads_null_and_refuses_what_it_did_not_declare() {
        let as_of = AsOf { data: None };
        let declared = declared(&["data.json"]);
        let now = chrono::Utc::now();
        let answer = read_then(&as_of, Some(&declared), "data.json", now);
        assert_eq!(answer.to_json()["data"], serde_json::Value::Null);
        let refused = read_then(&as_of, Some(&declared), "data/other.json", now);
        assert_eq!(refused.to_json()["error"]["kind"], "not-approved");
    }

    #[cfg(not(windows))]
    #[test]
    fn an_approved_page_as_of_a_version_reads_its_kept_data_and_calls_and_offers_nothing() {
        let page = r#"<script type="application/json" id="purlis">{"api":1,"data":["data.json"]}</script><script>go()</script>"#;
        let root = tempfile::tempdir().unwrap();
        let ws = root.path().join("workspaces/alpha");
        let folder = ws.join("artifacts/board");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("index.html"), page).unwrap();
        std::fs::write(folder.join("data.json"), r#"{"n": 2}"#).unwrap();
        let request = artifact::Request {
            folder: "board".to_owned(),
            ..artifact::Request::default()
        };
        let pin = match artifact::pin(root.path(), &ws, &request).unwrap() {
            artifact::Pinned::New(pin) => pin.id,
            other => panic!("{other:?}"),
        };
        let kept = root.path().join("kept");
        std::fs::create_dir_all(&kept).unwrap();
        std::fs::write(kept.join("data.json"), r#"{"n": 1}"#).unwrap();
        let config = tempfile::tempdir().unwrap();
        let snapshot = serve::Snapshot::read(&folder, "board").unwrap();
        approval::approve(config.path(), &ws, "board", &pin, snapshot.fingerprint()).unwrap();
        let declared = declared_by(&snapshot);
        let of = |then: Option<AsOf>| Of {
            plane: serde_json::from_str("\"plane\"").unwrap(),
            workspace: "alpha".to_owned(),
            dir: ws.clone(),
            folder: "board".to_owned(),
            pin: pin.clone(),
            standing: Standing::Approved,
            then,
        };
        let read = |of: &Of| {
            super::super::read_asked(
                Some(config.path()),
                of,
                &snapshot,
                declared.as_ref(),
                "data.json",
            )
            .to_json()["data"]
                .clone()
        };
        let now = of(None);
        let then = of(Some(AsOf { data: Some(kept) }));
        assert_eq!(read(&now), serde_json::json!({"n": 2}));
        assert_eq!(
            read(&then),
            serde_json::json!({"n": 1}),
            "the data kept then"
        );
        assert!(super::super::still_runs(
            Some(config.path()),
            &now,
            &snapshot
        ));
        assert!(
            !super::super::still_runs(Some(config.path()), &then, &snapshot),
            "no query, subscription or offer is admitted as of a version"
        );
    }

    #[test]
    fn an_as_of_page_whose_script_is_not_approved_offers_no_approval() {
        for asks in [Standing::New, Standing::Changed, Standing::PinChanged] {
            assert_eq!(shown_standing(asks), SHOWN_WITHOUT_SCRIPT);
        }
        assert_eq!(shown_standing(Standing::Approved), "approved");
        assert_eq!(shown_standing(Standing::ScriptFree), "script-free");
    }
}
