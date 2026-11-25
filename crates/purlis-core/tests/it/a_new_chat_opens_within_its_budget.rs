// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A new chat is usable within its budget, however many branch folders the plane holds**
//! (H12): users reported that opening a new chat took minutes. The core's part of that wait is
//! `start::ready`, which runs before the program does, so it is timed here, phase by phase
//! ([`purlis_core::opening`]), over a plane with no other branch folder and with 130.
//!
//! The budget is the whole of the core's part of an opening. The window's click, the session
//! host's spawn and the harness's first prompt come after it, and have their own steps.

use crate::support;

use std::time::Duration;

use purlis_core::{opening, profiles, start, worktree};

/// What the core may spend readying one new chat, branch folders and all.
const BUDGET: Duration = Duration::from_secs(5);

/// A plane with a layer, a stand-in Claude Code profile and `others` branch folders cut beside
/// the one the chat opens in, whose start is returned.
fn plane_with_branch_folders(others: usize) -> (support::Fixture, tempfile::TempDir, start::Start) {
    let f = support::plane_with_clone("thing");
    f.give_the_plane_a_layer();
    let bin = tempfile::tempdir().unwrap();
    let program = stand_in::claude_code(bin.path(), "claude-stand-in", "#!/bin/sh\n");
    std::fs::write(
        f.plane.join(profiles::LOCAL_FILE),
        format!(
            "[harness.work]\nkind = \"claude\"\ncommand = [{:?}]\n",
            program.display().to_string()
        ),
    )
    .unwrap();
    let set = profiles::current(&f.plane);
    let profile = set.get("work").expect("the profile is declared");
    purlis_core::profiletrust::record_launched(
        &f.plane,
        "work",
        &purlis_core::profiletrust::fingerprint(profile),
    )
    .unwrap();
    support::no_claude_sandbox_here(&f.plane);
    for n in 0..others {
        let other =
            worktree::add(&f.plane, &f.ws, &f.repo, &format!("other-{n}"), None).expect("a piece");
        fill_like_a_checkout(&other.path);
    }
    // The branch folder a new chat is cut, timed beside the others: it is the picker's first
    // phase (`worktrees::on_a_branch`).
    let cut = std::time::Instant::now();
    let mine = worktree::add(&f.plane, &f.ws, &f.repo, "mine", None).expect("a piece");
    CUT_MS.with(|ms| ms.set(cut.elapsed().as_millis() as u32));
    let start = start::Start {
        profile: Some("work".to_owned()),
        persona: None,
        name: "ide.1".to_owned(),
        cwd: Some(mine.path),
        resume: None,
        show_footer: false,
        resuming: None,
        without_sandbox: None,
        held: None,
        grants: Default::default(),
        made: purlis_core::sandbox::Made::Window,
    };
    (f, bin, start)
}

/// What a real checkout holds that a one-file fixture does not: [`FOLDERS`] folders of
/// [`FILES`] files, which every walk of the plane reads (the plane the report came from has
/// about 130 of them, each of a repository of thousands of files).
const FOLDERS: usize = 60;
const FILES: usize = 20;

fn fill_like_a_checkout(tree: &std::path::Path) {
    for folder in 0..FOLDERS {
        let dir = tree.join(format!("src/d{folder}"));
        std::fs::create_dir_all(&dir).unwrap();
        for file in 0..FILES {
            std::fs::write(dir.join(format!("f{file}.txt")), "x").unwrap();
        }
    }
}

thread_local! {
    static CUT_MS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

fn report(others: usize) -> opening::Timed<()> {
    let (f, _bin, start) = plane_with_branch_folders(others);
    let timed = opening::timed(|| match start::ready(&start, &f.plane) {
        Ok(_) => {}
        // A sandbox cannot be made inside this test run's own (a chat run under purlis's
        // sandbox), so the program's check refuses. Every phase before it was timed.
        Err(why) if why.contains("does not answer as Claude Code") => {}
        Err(why) => panic!("it is not ready: {why}"),
    });
    let timed = opening::Timed {
        phases: std::iter::once(purlis_core::slowlog::Took {
            name: "branch_cut".to_owned(),
            ms: CUT_MS.with(std::cell::Cell::get),
        })
        .chain(timed.phases)
        .collect(),
        ms: timed.ms + CUT_MS.with(std::cell::Cell::get),
        ..timed
    };
    let phases: Vec<String> = timed
        .phases
        .iter()
        .map(|p| format!("{} {} ms", p.name, p.ms))
        .collect();
    eprintln!(
        "opening beside {others} branch folders: {} ms ({})",
        timed.ms,
        phases.join(", ")
    );
    timed
}

#[test]
fn a_new_chat_is_ready_within_its_budget_in_a_plane_with_no_other_branch_folder() {
    purlis_core::unsteered!();
    let timed = report(0);
    assert!(
        Duration::from_millis(timed.ms.into()) < BUDGET,
        "readying took {} ms: {:?}",
        timed.ms,
        timed.phases
    );
}

#[test]
fn a_new_chat_is_ready_within_its_budget_in_a_plane_with_130_branch_folders() {
    purlis_core::unsteered!();
    let timed = report(130);
    assert!(
        Duration::from_millis(timed.ms.into()) < BUDGET,
        "readying took {} ms: {:?}",
        timed.ms,
        timed.phases
    );
}
