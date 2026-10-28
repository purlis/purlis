// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A theme drafts sweep under `$PURLIS_HOME` adopts no folder that names no clone** (#2761).
//!
//! An operator sets `$PURLIS_HOME` to share one state folder between clones, so a chat's drafts
//! folder there that names no clone may be another clone's open chat's. The sweep leaves it as
//! it is, whether the home is outside the plane or inside it, and still deletes this clone's
//! own drafts of a chat that is gone.
//!
//! The library's unit tests never see a steering variable (`steer`), so this test re-runs
//! itself with `$PURLIS_HOME` set on purpose ([`purlis_core::testrun::rerun_steered`]).

use std::path::Path;

use purlis_core::theme::drafts;

/// Set on the child: the case it runs, and the plane and home it runs them in.
const CASE: &str = "DRAFTS_SWEEP_SHARED_HOME_CASE";
const PLANE: &str = "DRAFTS_SWEEP_SHARED_HOME_PLANE";

const ME: &str = "a_drafts_sweep_under_a_shared_home_adopts_no_folder::an_ownerless_drafts_folder_in_a_shared_home_is_never_deleted";

#[test]
fn an_ownerless_drafts_folder_in_a_shared_home_is_never_deleted() {
    purlis_core::unsteered!();
    let Some(case) = std::env::var_os(CASE) else {
        for case in ["outside the plane", "inside the plane"] {
            let dir = tempfile::tempdir().unwrap();
            let plane = dir.path().join("plane");
            std::fs::create_dir_all(plane.join("workspaces")).unwrap();
            std::fs::write(plane.join("charter.toml"), "").unwrap();
            let home = if case == "outside the plane" {
                dir.path().join("shared-state")
            } else {
                plane.join("shared-state")
            };
            std::fs::create_dir_all(&home).unwrap();
            purlis_core::testrun::rerun_steered(
                &[ME],
                &[
                    (CASE, case.as_ref()),
                    (PLANE, plane.as_os_str()),
                    ("PURLIS_HOME", home.as_os_str()),
                ],
            );
        }
        return;
    };
    let plane = std::env::var_os(PLANE).expect("the plane the parent made");
    let plane = Path::new(&plane);

    // This clone's own draft, of a chat that is gone: the sweep runs here, and deletes it.
    let mine = purlis_core::reopen::mint();
    let place = drafts::Place {
        plane,
        workspace: None,
    };
    drafts::start(None, place, &mine, "dawn").expect("a draft is started");
    let mine = drafts::folder_of(plane, &mine).unwrap();
    assert!(mine.join(".owner").is_file(), "{case:?}");

    // A chat's folder there that names no clone: maybe another clone's open chat's.
    let ownerless = drafts::folder_of(plane, &purlis_core::reopen::mint()).unwrap();
    let noon = ownerless.join("noon");
    std::fs::create_dir_all(&noon).unwrap();
    std::fs::write(
        noon.join("theme.json"),
        r#"{"name":"Noon","appearances":[{"id":"dark","appearance":"dark"}]}"#,
    )
    .unwrap();

    assert_eq!(drafts::sweep(plane, &[]), 1, "{case:?}");
    assert!(!mine.exists(), "{case:?}: this clone's gone chat's drafts");
    assert!(
        noon.join("theme.json").is_file(),
        "{case:?}: the ownerless folder stays"
    );
    assert!(
        !ownerless.join(".owner").exists(),
        "{case:?}: and is not adopted"
    );
    // Nor at any later sweep.
    assert_eq!(drafts::sweep(plane, &[]), 0, "{case:?}");
    assert!(noon.join("theme.json").is_file(), "{case:?}");
}
