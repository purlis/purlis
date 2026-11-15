// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The sandbox scenario suite** (M74.1). Ticket 06's scenario host fills it; until then each set
//! holds one placeholder, so `.github/workflows/sandbox-scenarios.yml` has a scenario to run on
//! every leg and its filters (`tools/sandbox-scenarios.sh`) are seen to pick one.

#[test]
fn the_sandbox_scenarios_run_on_every_leg() {}

/// The entries run at `Timings::production()` (ticket 54), picked by the `production-timings`
/// label and run in the merge queue.
mod production_timings {
    #[test]
    fn the_production_timing_entries_run_when_asked() {}
}
