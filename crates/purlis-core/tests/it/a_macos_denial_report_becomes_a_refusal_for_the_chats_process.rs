// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Seatbelt's own report of a refusal, read for real** (T1.1-52, Q294, Q295). A perl under
//! `sandbox-exec`, with a tagged deny rule, tries files it may not create; the receiver's `log
//! stream` reads the kernel's reports of it and tells each as a `Refusal`. A refusal reported
//! for another chat's tag is never told here. A missing report is unknown, so these tests
//! expect at least one refusal and at most the refusals made, never an exact sum.

#![cfg(target_os = "macos")]

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_core::sandbox::denial_report::{Receiver, Tag, Unavailable, is_admin, tag_denials};

const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";
const TRIES: usize = 30;

/// Whether this process can apply a profile at all: not where it is already sandboxed.
fn can_apply() -> bool {
    purlis_core::forklock::status(Command::new(SANDBOX_EXEC).args([
        "-p",
        "(version 1)(allow default)",
        "/usr/bin/true",
    ]))
    .is_ok_and(|status| status.success())
}

/// Whether the test can run: on CI a skip is a failure.
fn can_run() -> bool {
    if can_apply() && is_admin() {
        return true;
    }
    assert!(
        std::env::var_os("CI").is_none() && std::env::var_os("GITHUB_ACTIONS").is_none(),
        "this process cannot apply a Seatbelt profile, or its user is no administrator, on CI"
    );
    false
}

fn profile(dir: &Path, tag: &Tag) -> String {
    tag_denials(
        &format!(
            "(version 1)\n(allow default)\n(deny file-write* (subpath \"{}\"))\n",
            dir.display()
        ),
        tag,
    )
}

/// A process under `profile` that tries `TRIES` distinct files in `dir`, one process for all.
fn attempt(profile: &str, dir: &Path, pid: &AtomicU32) {
    let mut command = Command::new(SANDBOX_EXEC);
    command
        .args([
            "-p",
            profile,
            "/usr/bin/perl",
            "-e",
            &format!(
                "for (1..{TRIES}) {{ open(F, \">$ARGV[0]/f$_\"); select(undef,undef,undef,0.05) }}"
            ),
        ])
        .arg(dir);
    let mut child = purlis_core::forklock::spawn(&mut command).expect("perl starts");
    pid.store(child.id(), Ordering::SeqCst);
    child.wait().expect("perl ends");
}

fn receiver(tag: &Tag, pid: &Arc<AtomicU32>, told: &Arc<Mutex<Vec<String>>>) -> Receiver {
    let chat = Arc::clone(pid);
    let sink = Arc::clone(told);
    Receiver::start(
        tag,
        move |seen| seen == chat.load(Ordering::SeqCst),
        Box::new(move |refusal| sink.lock().unwrap().push(refusal.target.clone())),
    )
    .expect("the receiver starts for an administrator")
}

#[test]
fn a_seatbelt_denial_for_the_chats_process_becomes_a_refusal_on_a_mac() {
    purlis_core::unsteered!();
    if !can_run() {
        return;
    }
    let dir = tempfile::tempdir().expect("a folder");
    let dir = dir.path().canonicalize().expect("its real path");
    let tag = Tag::new().expect("a tag");
    let (pid, told) = (Arc::new(AtomicU32::new(0)), Arc::default());
    let receiver = receiver(&tag, &pid, &told);
    attempt(&profile(&dir, &tag), &dir, &pid);
    receiver.finish(Duration::from_secs(15));
    let told = told.lock().unwrap();
    assert!(!told.is_empty(), "no refusal was told");
    assert!(
        told.len() <= TRIES + 1,
        "{} told of {TRIES} made",
        told.len()
    );
    assert!(
        told.iter()
            .all(|target| Path::new(target).starts_with(&dir)),
        "{told:#?}"
    );
}

#[test]
fn another_chats_denials_are_never_told_for_this_chat_on_a_mac() {
    purlis_core::unsteered!();
    if !can_run() {
        return;
    }
    let mine = tempfile::tempdir().expect("a folder");
    let other = tempfile::tempdir().expect("another folder");
    let other_dir = other.path().canonicalize().expect("its real path");
    let tag = Tag::new().expect("a tag");
    let other_tag = Tag::new().expect("another tag");
    let pid = Arc::new(AtomicU32::new(0));
    let told: Arc<Mutex<Vec<String>>> = Arc::default();
    // Every pid is accepted here, so only the tag keeps the other chat's reports out.
    let sink = Arc::clone(&told);
    let receiver = Receiver::start(
        &tag,
        |_| true,
        Box::new(move |refusal| sink.lock().unwrap().push(refusal.target.clone())),
    )
    .expect("the receiver starts for an administrator");
    attempt(&profile(&other_dir, &other_tag), &other_dir, &pid);
    receiver.finish(Duration::from_secs(10));
    let told = told.lock().unwrap();
    assert!(
        told.is_empty(),
        "another chat's refusals were told: {told:#?}"
    );
    drop(mine);
}

#[test]
fn a_standard_user_has_no_receiver_and_is_told_why() {
    purlis_core::unsteered!();
    if is_admin() {
        return;
    }
    let tag = Tag::new().expect("a tag");
    let started = Receiver::start(&tag, |_| true, Box::new(|_| {}));
    assert_eq!(started.err(), Some(Unavailable::NotAdmin));
}
