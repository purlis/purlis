// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::fs;
use std::path::{Path, PathBuf};

use crate::profilesignin::SignIn;
use crate::profilestore::{Adopted, Folders, MachineSettings, list};

/// A device's purlis homes: its config home and its data home.
fn homes(dir: &Path, device: &str) -> Homes {
    Homes::at(
        &dir.join(device).join("config"),
        &dir.join(device).join("data"),
    )
}

/// The profile a chat named `name` starts on, as the device whose store is under `homes` reads
/// its profiles at a start.
fn started_on(homes: &Homes, name: &str) -> crate::profiles::Profile {
    let machine = list(homes.config_root()).expect("its store reads");
    crate::profiles::derive_with(
        None,
        Ok(None),
        &crate::harness_declaration::Declarations::default(),
        &machine,
    )
    .get(name)
    .cloned()
    .expect("a profile of that name")
}

/// Every file under `dir`, as a path under it.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in walk(dir) {
        if entry.is_file() {
            found.push(entry.strip_prefix(dir).expect("under it").to_path_buf());
        }
    }
    found
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut all = Vec::new();
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        all.push(path.clone());
        if path.is_dir() {
            all.extend(walk(&path));
        }
    }
    all
}

/// A desktop's made Claude Code profile `work`, signed in there: its folder holds Claude Code's
/// sign-in file and its settings.
fn a_signed_in_desktop_profile(dir: &Path) -> MachineProfile {
    let desktop = homes(dir, "desktop");
    let home = dir.join("desktop-home");
    let settings = home.join("claude-settings");
    fs::create_dir_all(&settings).unwrap();
    fs::write(settings.join("settings.json"), "{\"theme\":\"dark\"}").unwrap();
    let env = |name: &str| (name == "CLAUDE_CONFIG_DIR").then(|| settings.display().to_string());
    let made = crate::profilestore::create(
        &desktop,
        "work",
        "claude",
        &MachineSettings {
            home: &home,
            env: &env,
        },
        1,
    )
    .expect("made on the desktop");
    fs::write(
        made.record.folders.config.join(".credentials.json"),
        "{\"claudeAiOauth\":\"the desktop's sign-in\"}",
    )
    .unwrap();
    made.record
}

/// A [`Look`] whose harness answers its status with `status`.
struct Answers(Option<i32>);

impl Look for Answers {
    fn status(&self, _: &str, _: &[&str], _: &[(String, String)]) -> Option<i32> {
        self.0
    }
    fn is_there(&self, _: &Path) -> bool {
        false
    }
}

/// **No subscription sign-in is copied between devices** (#2049): the runner keeps the
/// desktop's profile in a folder of its own, and nothing of the desktop's folder, its sign-in
/// and its settings alike, is in it. A chat on it there runs in that folder.
#[test]
fn a_runner_keeps_a_desktops_profile_in_a_folder_of_its_own_with_nothing_of_the_desktops() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_signed_in_desktop_profile(dir.path());
    let runner = homes(dir.path(), "runner");

    let kept = keep(&runner, &Kept::of(&desktop).expect("it goes"), 7).expect("kept");

    let folder = runner.folder_of("work");
    assert!(folder.is_dir());
    assert_eq!(files_under(&folder), Vec::<PathBuf>::new());
    assert_eq!(kept.imported, None);
    assert_eq!(kept.sign_in, None);
    assert!(!kept.folders.config.starts_with(dir.path().join("desktop")));
    let on_the_runner = started_on(&runner, "work");
    assert_eq!(
        on_the_runner.env,
        vec![(
            "CLAUDE_CONFIG_DIR".to_owned(),
            folder.join("config").display().to_string()
        )]
    );
}

/// The desktop's own Claude Code folder, adopted in place as `claude`, is kept on the runner
/// under that name, in a folder the runner made, and a chat there on `claude` runs in it rather
/// than in the runner's default folder.
#[test]
fn the_desktops_adopted_folder_is_kept_under_its_built_ins_name_in_a_folder_the_runner_made() {
    let dir = tempfile::tempdir().unwrap();
    let adopted = MachineProfile {
        name: "claude".to_owned(),
        harness: "claude".to_owned(),
        title: "Claude Code (this Mac)".to_owned(),
        folders: Folders {
            config: dir.path().join("desktop-home/.claude"),
            data: None,
        },
        adopted: Some(Adopted {
            at: 1,
            variables: Vec::new(),
        }),
        imported: None,
        sign_in: None,
        pinned: None,
        sandbox_only: false,
        rest: Default::default(),
    };
    let runner = homes(dir.path(), "runner");

    keep(&runner, &Kept::of(&adopted).expect("it goes"), 7).expect("kept");

    let on_the_runner = started_on(&runner, "claude");
    assert_eq!(on_the_runner.source, crate::profiles::Source::Machine);
    assert_eq!(
        on_the_runner.env,
        vec![(
            "CLAUDE_CONFIG_DIR".to_owned(),
            runner
                .folder_of("claude")
                .join("config")
                .display()
                .to_string()
        )]
    );
}

/// A desktop with no record of a built-in keeps the built-in it always had: only a record the
/// runner made stands for one.
#[test]
fn with_nothing_kept_a_built_ins_name_is_still_the_built_in() {
    let dir = tempfile::tempdir().unwrap();
    let runner = homes(dir.path(), "runner");
    assert_eq!(
        started_on(&runner, "claude").source,
        crate::profiles::Source::BuiltIn
    );
}

/// Kept again, the runner's profile is the one it made, signed in as it was, with the desktop's
/// pin and sandbox-only mark as they are now.
#[test]
fn kept_again_it_keeps_its_folder_and_follows_the_desktops_pin() {
    let dir = tempfile::tempdir().unwrap();
    let mut desktop = a_signed_in_desktop_profile(dir.path());
    let runner = homes(dir.path(), "runner");
    keep(&runner, &Kept::of(&desktop).unwrap(), 7).unwrap();
    let signed_in_there = runner.folder_of("work").join("config/.credentials.json");
    fs::write(&signed_in_there, "the runner's own sign-in").unwrap();

    desktop.pinned = Some(crate::profilestore::pin::Pinned {
        version: "2.1.300".to_owned(),
        at: 3,
    });
    desktop.sandbox_only = true;
    let kept = keep(&runner, &Kept::of(&desktop).unwrap(), 9).unwrap();

    assert_eq!(
        fs::read_to_string(&signed_in_there).unwrap(),
        "the runner's own sign-in"
    );
    assert_eq!(
        kept.pinned.as_ref().map(|p| p.version.as_str()),
        Some("2.1.300")
    );
    assert!(kept.sandbox_only);
    assert_eq!(list(runner.config_root()).unwrap().len(), 1);
}

/// A runner's own profile of the same name and another harness is never taken over.
#[test]
fn a_runners_own_profile_of_that_name_and_another_harness_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let runner = homes(dir.path(), "runner");
    crate::profilestore::create_sandbox_only(&runner, "work", "codex", 1).unwrap();
    let desktop = a_signed_in_desktop_profile(dir.path());

    let refused = keep(&runner, &Kept::of(&desktop).unwrap(), 7).unwrap_err();

    assert!(refused.contains("of its own"), "{refused}");
    assert_eq!(list(runner.config_root()).unwrap()[0].harness, "codex");
}

/// Only a subscription profile goes to a runner in this version: a key's, a cloud's and a local
/// model's are said, and nothing of them is sent.
#[test]
fn a_key_a_cloud_and_a_local_models_profile_are_not_kept_on_a_runner() {
    let dir = tempfile::tempdir().unwrap();
    let mut desktop = a_signed_in_desktop_profile(dir.path());
    for (kind, says) in [
        ("api-key", "credentials from your vault"),
        ("cloud", "credentials from your vault"),
        ("local", "a model on this machine"),
    ] {
        desktop.sign_in = Some(SignIn {
            kind: kind.to_owned(),
            label: None,
            credentials: Default::default(),
            rest: Default::default(),
        });
        let refused = Kept::of(&desktop).unwrap_err();
        assert!(refused.contains(says), "{kind}: {refused}");
    }
    desktop.sign_in = Some(SignIn {
        kind: "subscription".to_owned(),
        label: None,
        credentials: Default::default(),
        rest: Default::default(),
    });
    assert!(Kept::of(&desktop).is_ok());
}

/// Whether a kept profile is signed in on the runner is what its harness says there, asked
/// afresh each time.
#[test]
fn a_kept_profiles_standing_is_what_its_harness_says_on_the_runner_now() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_signed_in_desktop_profile(dir.path());
    let runner = homes(dir.path(), "runner");
    let kept = keep(&runner, &Kept::of(&desktop).unwrap(), 7).unwrap();

    assert_eq!(standing(&kept, &Answers(Some(1))), Standing::SignedOut);
    assert_eq!(standing(&kept, &Answers(Some(0))), Standing::SignedIn);
    assert_eq!(standing(&kept, &Answers(None)), Standing::CannotTell);
}

/// **A move of a signed-in profile is silent** (#2049): a chat on a profile its runner says is
/// signed in starts on the pool with nobody asked, and so does one whose harness cannot tell.
#[test]
fn a_chat_on_a_profile_signed_in_on_the_pool_starts_with_nobody_asked() {
    let dir = tempfile::tempdir().unwrap();
    a_signed_in_desktop_profile(dir.path());
    let desktop = homes(dir.path(), "desktop");
    let project = dir.path().join("project-silent");
    let sent = std::cell::RefCell::new(Vec::new());

    for said in [Standing::SignedIn, Standing::CannotTell] {
        ready_on(desktop.config_root(), &project, "builds", "work", &|kept| {
            sent.borrow_mut().push(kept.clone());
            Ok(said)
        })
        .expect("it starts");
    }

    assert_eq!(sent.borrow().len(), 2);
    assert_eq!(sent.borrow()[0].name, "work");
    assert!(waiting_on_pools(&project).is_empty());
}

/// A chat on a profile signed out on the pool's runner starts nothing, and waits as one Ask to
/// sign it in there, however many starts were refused, until it is let go.
#[test]
fn a_chat_on_a_profile_signed_out_on_the_pool_waits_as_one_sign_in_there() {
    let dir = tempfile::tempdir().unwrap();
    a_signed_in_desktop_profile(dir.path());
    let desktop = homes(dir.path(), "desktop");
    let project = dir.path().join("project-signed-out");

    for _ in 0..2 {
        let refused = ready_on(desktop.config_root(), &project, "builds", "work", &|_| {
            Ok(Standing::SignedOut)
        })
        .unwrap_err();
        assert!(
            refused.contains("not signed in on pool builds"),
            "{refused}"
        );
    }

    let waits = waiting_on_pools(&project);
    assert_eq!(waits.len(), 1);
    assert_eq!(
        (waits[0].pool.as_str(), waits[0].profile.as_str()),
        ("builds", "work")
    );
    let_go(None, "builds", "work");
    assert!(waiting_on_pools(&project).is_empty());
}

/// A profile this desktop keeps no record of is the runner's own to say, and a key profile's
/// chat starts nothing on a pool and sends the runner nothing.
#[test]
fn a_profile_with_no_record_is_the_runners_and_a_key_profile_is_refused_unsent() {
    let dir = tempfile::tempdir().unwrap();
    let mut record = a_signed_in_desktop_profile(dir.path());
    let desktop = homes(dir.path(), "desktop");
    let project = dir.path().join("project-keys");
    let asked = std::cell::Cell::new(0);
    let keep = |_: &Kept| {
        asked.set(asked.get() + 1);
        Ok(Standing::SignedIn)
    };

    ready_on(desktop.config_root(), &project, "builds", "claude", &keep).expect("the runner's");
    assert_eq!(asked.get(), 0);

    record.sign_in = Some(SignIn {
        kind: "api-key".to_owned(),
        label: None,
        credentials: Default::default(),
        rest: Default::default(),
    });
    crate::profilestore::update(desktop.config_root(), |store| {
        store.profiles[0] = record.clone();
    })
    .unwrap();
    let refused = ready_on(desktop.config_root(), &project, "builds", "work", &keep).unwrap_err();
    assert!(refused.contains("Nothing was started"), "{refused}");
    assert_eq!(asked.get(), 0);
}
