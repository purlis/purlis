// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The sandbox scenario cells' format** (T1.1-05, #6852): one format for a cell's steps and
//! expectations, shared by the Rust scenario suite and the plane's live checker, so live and
//! deterministic runs judge by the same expectations.
//!
//! - `schema/cells.schema.json`: a batch file (`cells/*.json`) and a pins file (`pins/*.json`).
//! - `schema/leg-report.schema.json`: one record per entry a leg ran.
//! - [`model`]: the files as Rust reads them.
//! - [`validate`]: what a schema cannot say (event kinds, a dialect's missing events, proof of a
//!   run, marks that name real assertions, the row table).
//! - [`judge`]: a cell's verdict under its `expected_fail` marks.
//! - [`plan`]: the derived production-timing set and which runs take a cell.
//! - [`net`]: the suite's local origins, direct-connection target and test CA (T1.1-02).

pub mod judge;
pub mod kinds;
pub mod model;
pub mod net;
pub mod plan;
pub mod timings;
pub mod validate;

use std::path::PathBuf;

/// **The compile check that the resolver seam is absent without `test-support`** (T1.1-02): this
/// crate depends on purlis-core without the feature, so under `--cfg purlis_seam_probe` this line
/// must not compile. `the_resolver_seam_does_not_compile_without_test_support` builds it so, and
/// a cfg on `purlis_core::sandbox::names` widened past `test-support` turns that test red.
#[cfg(purlis_seam_probe)]
pub use purlis_core::sandbox::names::Names as SeamProbe;

/// This crate's folder: `tests/scenarios` in the repo.
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
