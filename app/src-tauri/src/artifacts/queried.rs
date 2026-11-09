// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A page reads an extension's query** (ADR 0120 §9, #5971): `<extension-id>.<name>@N`,
//! declared and admitted as a core query is, and answered by **this viewer's own** extension
//! ([`purlis_core::extension::queries::ask`]): from its facts file while that answer is fresh,
//! else by its program, one question with the deadline. Where this machine has no such
//! extension in force, the page is answered `needs-extension` and renders on.
//!
//! A subscription to one is asked again each `fresh_seconds` while someone subscribes
//! ([`purlis_core::artifact::live::Hub::subscribe_every`]); the plane's changes do not move it.

use std::path::Path;
use std::time::Duration;

use purlis_core::artifact::{approval, calls, data, serve};
use purlis_core::catalogue::{Answer, ErrorKind};
use purlis_core::extension::{self, queries};
use serde_json::Value;
use tauri::Manager;

use super::{Of, still_runs};
use crate::planes::Planes;

/// Whether `asked`, `<id>@<N>`, names an extension's query rather than the core's.
pub(super) fn is_extension(asked: &str) -> bool {
    asked
        .rsplit_once('@')
        .is_some_and(|(id, _)| queries::is_extension_query(id))
}

/// What this machine's extensions declare of extension query `id`, for the bar's listing.
pub(super) fn declared_here(id: &str) -> Option<calls::ExtensionQuery> {
    let config = purlis_core::machine::config_root()?;
    queries::declared_here(&config, &crate::extensions::built_in(), id)
}

/// The plane `of` is in, as this run holds it.
fn root_of<R: tauri::Runtime>(app: &tauri::AppHandle<R>, of: &Of) -> Option<std::path::PathBuf> {
    app.try_state::<Planes>()
        .and_then(|planes| planes.held(&of.plane).ok())
        .map(|held| held.root().to_path_buf())
}

/// **Whether a page may ask extension query `asked` with `params`**, gated again at the ask: its
/// pin and approval still running its script, declared as written with only declared
/// parameters, and a plane-scoped one only with its plane grant. `Ok` is the query's id and its
/// freshness, which a subscription is asked again on.
pub(super) fn admitted<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    config: Option<&Path>,
    of: &Of,
    snapshot: &serve::Snapshot,
    declared: Option<&data::Declared>,
    asked: &str,
    params: &Value,
) -> Result<(String, Duration), ErrorKind> {
    if !still_runs(config, of, snapshot) {
        return Err(ErrorKind::NotApproved);
    }
    let calls = declared.map_or(&[][..], |d| d.queries.as_slice());
    let id = calls::admit_extension(calls, asked, params)?.to_owned();
    let (Some(config), Some(root)) = (config, root_of(app, of)) else {
        return Err(ErrorKind::NeedsExtension);
    };
    let choices = extension::project::Choices::read_in(&root, Some(&of.workspace));
    let found = queries::find(config, &crate::extensions::built_in(), &choices, &id)?;
    let plane = approval::plane_granted(Some(config), &of.dir, &of.folder, &of.pin, snapshot);
    if found.declared.scope == purlis_core::catalogue::Scope::Plane && !plane {
        return Err(ErrorKind::NotApproved);
    }
    Ok((id, Duration::from_secs(found.declared.fresh_seconds)))
}

/// **The answer to a page calling extension query `asked` with `params`**, gated again at the
/// ask ([`admitted`]).
pub(super) fn answer<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    config: Option<&Path>,
    of: &Of,
    snapshot: &serve::Snapshot,
    declared: Option<&data::Declared>,
    asked: &str,
    params: &Value,
) -> Answer {
    match admitted(app, config, of, snapshot, declared, asked, params) {
        // Admitted, so its plane grant, where it needs one, is held.
        Ok((id, _)) => ask(app, &of.plane, &of.workspace, &id, params),
        Err(kind) => Answer::error(kind, chrono::Utc::now()),
    }
}

/// **What extension query `id` answers now** in workspace `workspace` of plane `plane`, for a
/// page already admitted to it, its plane grant included.
pub(super) fn ask<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    plane: &crate::planes::PlaneId,
    workspace: &str,
    id: &str,
    params: &Value,
) -> Answer {
    let at = chrono::Utc::now();
    let root = app
        .try_state::<Planes>()
        .and_then(|planes| planes.held(plane).ok())
        .map(|held| held.root().to_path_buf());
    let (Some(root), Some(config), Some(views)) = (
        root,
        purlis_core::machine::config_root(),
        app.try_state::<crate::views::Views>(),
    ) else {
        return Answer::error(ErrorKind::Failed, at);
    };
    let choices = extension::project::Choices::read_in(&root, Some(workspace));
    queries::ask(
        &views.executor(),
        &config,
        &choices,
        &queries::Asked {
            id,
            workspace,
            params,
            plane: true,
        },
        at,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_extension_s_query_is_told_from_the_core_s_by_its_source() {
        assert!(is_extension("slack.unread@1"));
        assert!(!is_extension("purlis.todo_list@1"));
        assert!(!is_extension("slack.unread"));
    }
}
