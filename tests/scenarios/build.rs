// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Declares `purlis_seam_probe`, the cfg under which this crate names purlis-core's resolver
//! seam in a build without `test-support`: `the_resolver_seam_does_not_compile_without_test_support`
//! (`tests/resolver_seam.rs`) turns it on and expects that build to fail.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(purlis_seam_probe)");
}
