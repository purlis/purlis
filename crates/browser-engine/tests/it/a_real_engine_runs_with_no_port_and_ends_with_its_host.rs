// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A real engine: no port listens while it runs, and every one of its processes is gone within
//! a second of its host, however the host ends.
//!
//! These need a real Chrome and the right to start it, which a sandboxed agent does not have.
//! They run where `PURLIS_TEST_ENGINE` names a Chrome executable, and on CI
//! (`GITHUB_ACTIONS=true`) against the runner's own Chrome, where a missing Chrome fails
//! rather than skips. Anywhere else each test says why it did not run and passes. No test
//! downloads an engine or reaches the network: the reach proxy is an address nothing listens on.

#![cfg(unix)]

use crate::common;

use std::collections::BTreeSet;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use common::site::engine_program;
use common::{HostPart, gone_within};
use purlis_browser_engine::WebAddress;
use purlis_browser_engine::launch::{Launch, ReachProxy};
use purlis_browser_engine::process::{Engine, Keeper};

fn keeper() -> Keeper {
    Keeper::program(env!("CARGO_BIN_EXE_purlis-engine-keeper"))
}

fn launch(program: PathBuf, data: &std::path::Path) -> Launch {
    // Nothing listens at the proxy's address: pages fail to load, and nothing leaves the
    // machine.
    Launch::new(
        &common::fixture::chosen(&program),
        data.to_owned(),
        ReachProxy("127.0.0.1:9".parse().unwrap()),
    )
}

/// Starts an engine and opens a page, so its renderer processes run too.
fn start(program: PathBuf, data: &std::path::Path) -> Engine {
    let engine = Engine::start(&launch(program, data), &keeper()).unwrap_or_else(|why| {
        let log = std::fs::read_to_string(data.join("engine.log")).unwrap_or_default();
        panic!("{why}\nthe engine's log:\n{log}")
    });
    engine
        .open_page(&WebAddress::parse("http://example.invalid/").unwrap())
        .unwrap();
    std::thread::sleep(Duration::from_secs(2));
    engine
}

/// Every process in `group`, and every descendant of `root`, from `ps`.
fn processes_of(group: i32, root: u32) -> Vec<u32> {
    let out = Command::new("ps")
        .args(["-A", "-o", "pid=,ppid=,pgid="])
        .output()
        .unwrap();
    assert!(out.status.success(), "ps failed");
    let rows: Vec<(u32, u32, i32)> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace().map(str::parse::<i64>);
            match (words.next(), words.next(), words.next()) {
                (Some(Ok(pid)), Some(Ok(ppid)), Some(Ok(pgid))) => {
                    Some((pid as u32, ppid as u32, pgid as i32))
                }
                _ => None,
            }
        })
        .collect();
    let mut found: BTreeSet<u32> = rows.iter().filter(|r| r.2 == group).map(|r| r.0).collect();
    found.insert(root);
    loop {
        let more: Vec<u32> = rows
            .iter()
            .filter(|r| found.contains(&r.1) && !found.contains(&r.0))
            .map(|r| r.0)
            .collect();
        if more.is_empty() {
            break;
        }
        found.extend(more);
    }
    found.into_iter().collect()
}

/// The TCP sockets `pids` listen on, as `lsof` lists them.
fn listening(pids: &[u32]) -> Vec<String> {
    let list: Vec<String> = pids.iter().map(u32::to_string).collect();
    let out = Command::new("lsof")
        .args(["-nP", "-a", "-iTCP", "-sTCP:LISTEN", "-p", &list.join(",")])
        .output()
        .expect("lsof, to list listening sockets");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .skip(1)
        .map(str::to_owned)
        .collect()
}

#[test]
fn no_port_listens_while_a_real_engine_runs() {
    let Some(program) = engine_program() else {
        return;
    };
    // The check itself is seen to work first: a socket this test listens on is listed.
    let control = TcpListener::bind("127.0.0.1:0").unwrap();
    let mine = listening(&[std::process::id()]);
    assert!(
        mine.iter()
            .any(|l| l.contains(&control.local_addr().unwrap().port().to_string())),
        "lsof did not list a socket this test listens on: {mine:?}"
    );
    drop(control);

    let data = tempfile::tempdir().unwrap();
    let engine = start(program, data.path());
    let version = engine.version().unwrap();
    assert!(version["product"].as_str().is_some(), "{version}");
    let pids = processes_of(engine.process_group(), engine.pid());
    assert!(
        pids.len() >= 3,
        "the keeper, the engine and its helpers: {pids:?}"
    );
    let ports = listening(&pids);
    assert!(ports.is_empty(), "the engine listens: {ports:?}");
    let group = engine.process_group();
    engine.stop(Duration::from_secs(2));
    gone_within(group, &pids, Duration::from_secs(1));
}

/// The host's part: start a real engine with a page open, name every process it has, and wait.
fn host_part(then: &str) {
    let program = PathBuf::from(std::env::var_os("PURLIS_TEST_ENGINE_FOR_HOST").unwrap());
    let data = tempfile::tempdir().unwrap();
    let engine = start(program, data.path());
    let pids = processes_of(engine.process_group(), engine.pid());
    common::announce_all(engine.process_group(), engine.pid(), &pids);
    match then {
        "return" => std::process::exit(0),
        "stop" => {
            common::wait_to_be_ended();
            engine.stop(Duration::from_secs(2));
            std::process::exit(0);
        }
        _ => common::wait_to_be_ended(),
    }
}

fn run_host(test: &str, part: &str, program: &std::path::Path) -> HostPart {
    HostPart::start_with(
        test,
        part,
        &[("PURLIS_TEST_ENGINE_FOR_HOST", program.to_str().unwrap())],
    )
}

#[test]
fn a_real_engine_is_gone_within_a_second_of_its_host_being_killed() {
    if HostPart::is_me("engine-killed") {
        return host_part("wait");
    }
    let Some(program) = engine_program() else {
        return;
    };
    let host = run_host(
        "a_real_engine_runs_with_no_port_and_ends_with_its_host::a_real_engine_is_gone_within_a_second_of_its_host_being_killed",
        "engine-killed",
        &program,
    );
    let (group, mut pids) = (host.group, host.others.clone());
    pids.push(host.member);
    host.kill();
    gone_within(group, &pids, Duration::from_secs(1));
}

#[test]
fn a_real_engine_is_gone_within_a_second_of_its_host_returning_without_ending_it() {
    if HostPart::is_me("engine-returned") {
        return host_part("return");
    }
    let Some(program) = engine_program() else {
        return;
    };
    let host = run_host(
        "a_real_engine_runs_with_no_port_and_ends_with_its_host::a_real_engine_is_gone_within_a_second_of_its_host_returning_without_ending_it",
        "engine-returned",
        &program,
    );
    let (group, mut pids) = (host.group, host.others.clone());
    pids.push(host.member);
    host.wait_for_its_end();
    gone_within(group, &pids, Duration::from_secs(1));
}

#[test]
fn a_hung_real_engine_is_gone_within_a_second_of_its_host_being_killed() {
    if HostPart::is_me("engine-hung") {
        return host_part("wait");
    }
    let Some(program) = engine_program() else {
        return;
    };
    let host = run_host(
        "a_real_engine_runs_with_no_port_and_ends_with_its_host::a_hung_real_engine_is_gone_within_a_second_of_its_host_being_killed",
        "engine-hung",
        &program,
    );
    let (group, mut pids) = (host.group, host.others.clone());
    pids.push(host.member);
    // The engine stops answering anything, its pipe's end included.
    common::stop(host.member);
    host.kill();
    gone_within(group, &pids, Duration::from_secs(1));
}

#[test]
fn a_real_engine_ended_by_its_host_leaves_nothing_running() {
    if HostPart::is_me("engine-stopped") {
        return host_part("stop");
    }
    let Some(program) = engine_program() else {
        return;
    };
    let host = run_host(
        "a_real_engine_runs_with_no_port_and_ends_with_its_host::a_real_engine_ended_by_its_host_leaves_nothing_running",
        "engine-stopped",
        &program,
    );
    let (group, mut pids) = (host.group, host.others.clone());
    pids.push(host.member);
    host.end_politely();
    gone_within(group, &pids, Duration::from_secs(1));
}
