// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! An engine's process group ends within a second of its host, however the host ends: killed
//! outright, or returning without ending it, and even when a member of the group is stopped and
//! cannot answer. No engine is needed for this: the group's members here are plain programs,
//! and the host is this test binary run again in the host's part (`host_part`).
//!
//! The same, with a real engine in the group, is
//! `a_real_engine_is_gone_within_a_second_of_its_host`.

#![cfg(unix)]

use crate::common;

use std::process::{Command, Stdio};
use std::time::Duration;

use common::{HostPart, gone_within};
use purlis_browser_engine::process::{EngineGroup, Keeper};

fn keeper() -> Keeper {
    Keeper::program(env!("CARGO_BIN_EXE_purlis-engine-keeper"))
}

/// A member that would run for ten minutes and ignores the hang-up and the polite end, so only
/// a kill ends it, holding none of its host's streams.
fn stubborn_member() -> Command {
    let mut member = Command::new("/bin/sh");
    member
        .args(["-c", "trap '' HUP TERM INT; exec sleep 600"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    member
}

/// The host's part: start a group with a member that would run for ten minutes, say which,
/// then wait to be ended (or return at once, without ending the group, when asked).
#[expect(
    clippy::zombie_processes,
    reason = "the host's part is ended with its member still running: that is the case under test"
)]
fn host_part(then_return: bool) {
    let group = EngineGroup::start(&keeper()).unwrap();
    let mut member = stubborn_member();
    group.join(&mut member);
    let member = member.spawn().unwrap();
    common::announce(group.id(), member.id());
    if then_return {
        // Neither the group nor the member is ended: the process just exits.
        std::process::exit(0);
    }
    common::wait_to_be_ended();
}

#[test]
fn the_group_is_gone_within_a_second_of_its_host_being_killed() {
    if HostPart::is_me("killed") {
        return host_part(false);
    }
    let host = HostPart::start(
        "the_engines_group_is_gone_within_a_second_of_its_host::the_group_is_gone_within_a_second_of_its_host_being_killed",
        "killed",
    );
    let (group, member) = (host.group, host.member);
    host.kill();
    gone_within(group, &[member], Duration::from_secs(1));
}

#[test]
fn the_group_is_gone_within_a_second_of_its_host_returning_without_ending_it() {
    if HostPart::is_me("returned") {
        return host_part(true);
    }
    let host = HostPart::start(
        "the_engines_group_is_gone_within_a_second_of_its_host::the_group_is_gone_within_a_second_of_its_host_returning_without_ending_it",
        "returned",
    );
    let (group, member) = (host.group, host.member);
    host.wait_for_its_end();
    gone_within(group, &[member], Duration::from_secs(1));
}

#[test]
fn a_stopped_member_is_gone_within_a_second_of_its_host_being_killed() {
    if HostPart::is_me("stopped") {
        return host_part(false);
    }
    let host = HostPart::start(
        "the_engines_group_is_gone_within_a_second_of_its_host::a_stopped_member_is_gone_within_a_second_of_its_host_being_killed",
        "stopped",
    );
    let (group, member) = (host.group, host.member);
    // Time for the member to reach `sleep` with its traps set, so what ends it is the kill and
    // not the hang-up an orphaned group with a stopped member is sent.
    std::thread::sleep(Duration::from_millis(300));
    common::stop(member);
    host.kill();
    gone_within(group, &[member], Duration::from_secs(1));
}

#[test]
fn a_host_that_ends_its_group_leaves_nothing_running() {
    let group = EngineGroup::start(&keeper()).unwrap();
    let mut member = stubborn_member();
    group.join(&mut member);
    let mut member = member.spawn().unwrap();
    let (id, pid) = (group.id(), member.id());
    group.end();
    member.wait().unwrap();
    gone_within(id, &[pid], Duration::from_secs(1));
}

#[test]
fn a_keeper_started_outside_a_group_of_its_own_ends_nothing() {
    // Started in this test's own group, the keeper must refuse rather than kill the test.
    let status = Command::new(env!("CARGO_BIN_EXE_purlis-engine-keeper"))
        .stdin(std::process::Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(2));
}
