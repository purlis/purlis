// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner holds what a profile sends once as it keeps it** (#4396): the most the heap holds
//! while a runner stages a profile's settings batch by batch, checks them and keeps them, counted
//! by a counting allocator in a test binary of its own, so nothing else allocates beside it. A
//! runner that copied them as it checked them peaked at more than three times the profile
//! (#4314), and a pool's memory is reckoned from this ([`purlis_core::poollimits::profile_room`]).

use std::path::Path;

use purlis_core::profilestore::sign_in::Look;
use purlis_core::profilestore::{Homes, onrunner};
use purlis_session_protocol::session::{
    ProfileToKeep, SettingsFile, StagedSettings, settings_in_batches,
};

#[global_allocator]
static ALLOCATOR: dhat::Alloc = dhat::Alloc;

const MIB: usize = 1024 * 1024;

/// A harness that says every profile is signed in, and finds nothing on disk.
struct SignedIn;

impl Look for SignedIn {
    fn status(&self, _: &str, _: &[&str], _: &[(String, String)]) -> Option<i32> {
        Some(0)
    }
    fn is_there(&self, _: &Path) -> bool {
        false
    }
}

/// A skill of `len` bytes, as a desktop sends it: its frontmatter already filtered, and a body
/// of plain text.
fn skill(at: usize, len: usize) -> SettingsFile {
    let mut text = format!("---\nname: s{at}\ndescription: a skill\n---\n");
    let line = "Read the file, then say what it does in a sentence.\n";
    while text.len() + line.len() <= len {
        text.push_str(line);
    }
    SettingsFile {
        path: format!("skills/s{at}/SKILL.md"),
        bytes: text.into_bytes(),
    }
}

#[test]
fn a_runner_holds_what_a_profile_sends_once_as_it_keeps_it() {
    purlis_core::unsteered!();
    const FILES: usize = 16;
    const EACH: usize = 4 * MIB;
    let dir = tempfile::tempdir().unwrap();
    let homes = Homes::at(&dir.path().join("config"), &dir.path().join("data"));
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();

    let profiler = dhat::Profiler::builder().testing().build();
    let before = dhat::HeapStats::get().curr_bytes;
    // The batches arrive one at a time, as the link reads them off its frames.
    let mut staged = StagedSettings::default();
    let mut sent = 0;
    for at in 0..FILES {
        let file = skill(at, EACH);
        sent += file.bytes.len();
        for batch in settings_in_batches("work", vec![file]) {
            staged.add(batch).unwrap();
        }
    }
    let settings = staged
        .finished("work", u32::try_from(FILES).unwrap())
        .unwrap();
    let profile = ProfileToKeep {
        name: "work".to_owned(),
        harness: "claude".to_owned(),
        sandbox_only: false,
        pinned: None,
        settings: Some(settings),
        settings_staged: None,
        settings_changed: None,
    };
    onrunner::kept_for_the_desktop(&homes, &home, "01DESKTOP", profile, &SignedIn, 7)
        .expect("kept");
    let most = dhat::HeapStats::get().max_bytes.saturating_sub(before);
    drop(profiler);

    // What was sent, once, and room for the file and the batch on their way in.
    let room = sent + 3 * EACH;
    assert!(
        most <= room,
        "a runner keeping {} MiB held up to {} MiB, more than {} MiB",
        sent / MIB,
        most / MIB,
        room / MIB
    );
    let kept = std::fs::read_dir(dir.path().join("config"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert!(kept > 0, "the profile was kept");
}
