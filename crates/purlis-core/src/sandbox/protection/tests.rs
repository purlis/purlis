// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::*;
use crate::eventlog::{ArgsKey, Attributes, Began, Log, Recorder, RunOf};
use crate::harness::Harness;
use crate::sandbox::{Decided, Person, decide_granted, grant, personal};

const DEVICE: &str = "01J9ZZDEVICE0000000000000";
const CHAT: &str = "01J9ZZCHAT000000000000000A";
const RUN: &str = "01J9ZZRUN0000000000000000A";

/// A project at the full sandbox, with no cache folders to find, so its sandbox compiles here.
const FULL: &str = "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n";

/// A project with `manifest`, committed, and `level` as the person's own, set and confirmed in
/// Settings: the commit's id.
fn project(dir: &Path, manifest: &str, level: Option<Choice>) -> String {
    std::fs::write(dir.join("charter.toml"), format!("schema = 1\n{manifest}")).unwrap();
    let git = |args: &[&str]| {
        let ran = crate::testgit::run(dir, args);
        assert!(ran.ok(), "{args:?}: {ran:?}");
        ran
    };
    // This machine's own file, which git never carries, as a project made by purlis ignores it.
    std::fs::write(
        dir.join(".gitignore"),
        format!("{}\n", crate::profiles::LOCAL_FILE),
    )
    .unwrap();
    git(&["init", "-q"]);
    git(&["add", "charter.toml", ".gitignore"]);
    git(&["commit", "-q", "-m", "the project"]);
    if let Some(level) = level {
        std::fs::write(
            dir.join(crate::profiles::LOCAL_FILE),
            format!("[sandbox]\nlevel = \"{}\"\n", level.word()),
        )
        .unwrap();
        personal::set_by_you(dir, None, Some(level)).expect("confirmed");
    }
    git(&["rev-parse", "HEAD"]).out.trim().to_owned()
}

/// R-227: a chat started at each level writes one start event, which records the effective
/// protection per layer and the tier that set it, as the start resolved them, the
/// `charter.toml` commit and the answers in force, present and empty in phase 1.
#[test]
fn a_chat_start_event_records_protection_per_layer_its_tier_the_commit_and_the_answers() {
    let machine = crate::sandbox::Machine {
        env: crate::secrets::Env::of(&[]),
        home: Some(std::path::PathBuf::from("/home/op")),
        os: crate::sandbox::Os::MacOs,
    };
    // The project's file, the person's own level, and what the start runs at, set by whom.
    let cases = [
        ("", None, "floor", "project"),
        (FULL, None, "on", "project"),
        ("", Some(Choice::On), "on", "person"),
        (FULL, Some(Choice::Floor), "floor", "person"),
        ("", Some(Choice::Off), "off", "person"),
        (FULL, Some(Choice::Off), "off", "person"),
    ];
    for (manifest, level, expected_level, expected_tier) in cases {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let commit = project(root, manifest, level);
        let made = Made::Window;
        let person = Person {
            opt_out: None,
            level_as: None,
            made: &made,
        };
        let decided = decide_granted(
            Harness::ClaudeCode,
            root,
            &machine,
            &|_| true,
            &person,
            None,
            &grant::Grants::default(),
        );
        let (sandbox, lifted) = match &decided {
            Ok(Some(Decided::Sandboxed(applied))) => (Some(applied), None),
            Ok(Some(Decided::Unsandboxed(lifted))) => (None, Some(lifted)),
            other => panic!("{manifest:?} {level:?}: {other:?}"),
        };
        let attributes = Attributes {
            protection: Some(at_start(root, None, &made, sandbox, lifted)),
            ..Attributes::default()
        };
        let log = tempfile::tempdir().unwrap();
        let mut host = Recorder::new(
            Log::open(log.path(), DEVICE).unwrap(),
            ArgsKey::open(log.path()).unwrap(),
        );
        host.begin(
            root,
            1,
            RunOf {
                chat: CHAT,
                run: RUN,
            },
            Began::Start,
            &attributes,
        )
        .unwrap();
        let events = crate::eventlog::read(log.path()).unwrap();
        let started: Vec<_> = events.iter().filter(|e| e.kind == "run.started").collect();
        let case = format!("{manifest:?} {level:?}");
        assert_eq!(started.len(), 1, "{case}: one start event, {events:#?}");
        assert_eq!(
            started[0].body["protection"],
            serde_json::json!({
                "layers": [{"layer": "sandbox", "level": expected_level, "tier": expected_tier}],
                "charter_commit": commit,
                "answers_in_force": [],
            }),
            "{case}"
        );
    }
}
