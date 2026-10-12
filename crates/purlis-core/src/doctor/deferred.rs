// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The rows this version of charter does not check yet — each still printed, under its own
//! name, as a warning that says so. Each reason below names the ticket that plans its check:
//! OB-8 (#994) for most, FG-2 (#802) for the forges.
//!
//! **Not dropped, and not green.** A doctor that stops printing a row tells its reader the
//! problem that row reported has gone, and one that prints it green says it looked. Both
//! would be false: nothing here looked. So each unchecked row keeps its place in the table
//! and in `--json`, in the "not checked" shape every other doctor row uses, with the reason in
//! the detail and a hint saying its silence means nothing.
//!
//! **No hint points anywhere else.** Earlier builds said the Python charter's `charter doctor`
//! still ran these checks. This app is standalone (ADR 0044, ADR 0045): a row it does not check
//! says so, and sends nobody to another program to find out.
//!
//! The reasons are grouped by what is missing rather than by row, because that is what
//! changes: the day a group's check lands, its rows stop being deferred at once, and the
//! doctor tests' list of deferred names (`DEFERRED_ROWS`) is where that has to be said.
//!
//! **Checked since (#994):** `harness` (the declarations and the level each offers),
//! `vault registry` (both halves read, every entry usable), `news` (this build's release
//! notes), `plane-root guard` and `guard seen` (the guard's declaration, and its own record of
//! running), `shadowed docs` (a project's copy of a page or skill purlis ships), `mcp` (whether
//! each server a persona's chat is given can start) and `workspace layer` (each workspace's and
//! checkout's generated layer, read without writing, #998). What is still here needs a part
//! this build does not have: a provider asked about the credentials a vault holds (`vaults`,
//! `credential paths`).

use super::Row;

/// What every deferred row says to do about it. The checks are OB-8's (#994) and FG-2's (#802).
pub(crate) const DEFERRED_HINT: &str = "This purlis does not run this check yet, so its \
                                        silence means nothing — it is not saying the check \
                                        passed.";

/// A row for a check this binary does not run, saying why.
pub(crate) fn row(name: &str, why: &str) -> Row {
    Row::warn(name, format!("not checked ({why})"), DEFERRED_HINT)
}

// The forge CLIs and their sign-in. What each repo's forge can do is the `forge capabilities`
// row's (FG-2, #802).
pub(crate) const FORGES: &str = "this version of purlis does not check the forge CLI and its \
                                 sign-in yet";
// The rest are OB-8, #994, one acceptance line a group.
// Not deferred: left out by design, because the app's window takes the tmux frame's place.
pub(crate) const FRAME: &str = "this purlis has no tmux frame; its window takes the frame's \
                                place";
// #994.
pub(crate) const VAULTS: &str = "this version of purlis does not check vaults and the \
                                 credentials they hold yet";
