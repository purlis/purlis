// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::fs;
use std::path::{Path, PathBuf};

use crate::profilesignin::SignIn;
use crate::profilestore::{Adopted, Folders, MachineSettings, list};

/// The desktop a runner keeps profiles for, by its device id.
const DESKTOP: &str = "01JA0DESKT0P00000000000000";

/// The runner's home, where its harnesses' default folders are.
fn runner_home(dir: &Path) -> PathBuf {
    dir.join("runner-home")
}

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

    let kept = keep(
        &runner,
        &runner_home(dir.path()),
        &Kept::of(&desktop).expect("it goes"),
        DESKTOP,
        7,
    )
    .expect("kept");

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

/// A desktop's Claude Code folder, adopted in place there as `claude` and pinned.
fn the_desktops_adopted_folder(dir: &Path) -> MachineProfile {
    MachineProfile {
        name: "claude".to_owned(),
        harness: "claude".to_owned(),
        title: "Claude Code (this Mac)".to_owned(),
        folders: Folders {
            config: dir.join("desktop-home/.claude"),
            data: None,
        },
        adopted: Some(Adopted {
            at: 1,
            variables: Vec::new(),
        }),
        imported: None,
        sign_in: None,
        pinned: Some(crate::profilestore::pin::Pinned {
            version: "2.1.300".to_owned(),
            at: 1,
        }),
        sandbox_only: false,
        kept_for: None,
        rest: Default::default(),
    }
}

/// **The desktop's adopted folder is the runner's own default folder there** (#2049): kept as
/// Claude Code's default folder in the runner's home, adopted in place and marked kept for the
/// desktop, so a chat on `claude` there runs as the runner's built-in did, in that folder, and
/// a runner signed in before stays signed in. It carries no pin: it follows its harness.
#[test]
fn the_desktops_adopted_folder_is_the_runners_own_default_folder_there() {
    let dir = tempfile::tempdir().unwrap();
    let runner = homes(dir.path(), "runner");
    let adopted = the_desktops_adopted_folder(dir.path());

    let kept = keep(
        &runner,
        &runner_home(dir.path()),
        &Kept::of(&adopted).expect("it goes"),
        DESKTOP,
        7,
    )
    .expect("kept");

    assert_eq!(
        kept.folders.config,
        runner_home(dir.path()).join(".claude"),
        "the runner's own default folder"
    );
    assert!(kept.adopted.is_some());
    assert_eq!(kept.kept_for.as_deref(), Some(DESKTOP));
    assert_eq!(kept.pinned, None);
    assert!(
        !runner.folder_of("claude").exists(),
        "no folder of purlis's"
    );
    let on_the_runner = started_on(&runner, "claude");
    assert_eq!(on_the_runner.source, crate::profiles::Source::BuiltIn);
    assert!(on_the_runner.env.is_empty(), "{:?}", on_the_runner.env);
    // Signed in by purlis there, through the built-in's own sign-in, no folder set.
    let signed_in_as = sign_in::profile_of(&kept).expect("a profile to sign in");
    assert_eq!(signed_in_as.source, crate::profiles::Source::BuiltIn);
    assert_eq!(standing(&kept, &Answers(Some(1))), Standing::SignedOut);
}

/// A folder the person adopted on their own machine is never signed in by purlis: only one a
/// runner keeps for its desktop is.
#[test]
fn only_an_adopted_folder_kept_for_a_desktop_is_signed_in_by_purlis() {
    let dir = tempfile::tempdir().unwrap();
    let adopted = the_desktops_adopted_folder(dir.path());

    assert!(sign_in::profile_of(&adopted).is_none());
    assert!(sign_in::login_of(&adopted, None).is_err());
    assert_eq!(sign_in::standing_of(&adopted, &Answers(Some(1))), None);
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
    keep(
        &runner,
        &runner_home(dir.path()),
        &Kept::of(&desktop).unwrap(),
        DESKTOP,
        7,
    )
    .unwrap();
    let signed_in_there = runner.folder_of("work").join("config/.credentials.json");
    fs::write(&signed_in_there, "the runner's own sign-in").unwrap();

    desktop.pinned = Some(crate::profilestore::pin::Pinned {
        version: "2.1.300".to_owned(),
        at: 3,
    });
    desktop.sandbox_only = true;
    let kept = keep(
        &runner,
        &runner_home(dir.path()),
        &Kept::of(&desktop).unwrap(),
        DESKTOP,
        9,
    )
    .unwrap();

    assert_eq!(
        fs::read_to_string(&signed_in_there).unwrap(),
        "the runner's own sign-in"
    );
    assert_eq!(
        kept.pinned.as_ref().map(|p| p.version.as_str()),
        Some("2.1.300")
    );
    assert!(kept.sandbox_only);
    assert_eq!(kept.kept_for.as_deref(), Some(DESKTOP));
    assert_eq!(list(runner.config_root()).unwrap().len(), 1);

    // Unpinned on the desktop, it is unpinned on the runner.
    desktop.pinned = None;
    let kept = keep(
        &runner,
        &runner_home(dir.path()),
        &Kept::of(&desktop).unwrap(),
        DESKTOP,
        11,
    )
    .unwrap();
    assert_eq!(kept.pinned, None);
}

/// A runner's own profile of the same name and another harness is never taken over.
#[test]
fn a_runners_own_profile_of_that_name_and_another_harness_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let runner = homes(dir.path(), "runner");
    crate::profilestore::create_sandbox_only(&runner, "work", "codex", 1).unwrap();
    let desktop = a_signed_in_desktop_profile(dir.path());

    let refused = keep(
        &runner,
        &runner_home(dir.path()),
        &Kept::of(&desktop).unwrap(),
        DESKTOP,
        7,
    )
    .unwrap_err();

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
    let kept = keep(
        &runner,
        &runner_home(dir.path()),
        &Kept::of(&desktop).unwrap(),
        DESKTOP,
        7,
    )
    .unwrap();

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
            Ok(Some(said))
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
            Ok(Some(Standing::SignedOut))
        })
        .unwrap_err();
        assert!(
            refused.contains("not signed in on pool builds"),
            "{refused}"
        );
        assert!(refused.contains("Profile work on pool builds"), "{refused}");
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
        Ok(Some(Standing::SignedIn))
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

/// A runner sent nothing (one not shown to be the person's own, or of a purlis before 1.43) is
/// the runner's own to say: the chat starts there, on whatever profiles it has, and nothing waits.
#[test]
fn a_runner_sent_nothing_is_its_own_to_say() {
    let dir = tempfile::tempdir().unwrap();
    a_signed_in_desktop_profile(dir.path());
    let desktop = homes(dir.path(), "desktop");
    let project = dir.path().join("project-unsent");

    ready_on(desktop.config_root(), &project, "builds", "work", &|_| {
        Ok(None)
    })
    .expect("the runner's own to say");

    assert!(waiting_on_pools(&project).is_empty());
}

/// A runner's own refusal reaches the start in words that say whose they are.
#[test]
fn a_runner_that_does_not_keep_the_profile_starts_nothing_in_words() {
    let dir = tempfile::tempdir().unwrap();
    a_signed_in_desktop_profile(dir.path());
    let desktop = homes(dir.path(), "desktop");
    let project = dir.path().join("project-refused");

    let refused = ready_on(desktop.config_root(), &project, "builds", "work", &|_| {
        Err("the runner says: no room".to_owned())
    })
    .unwrap_err();

    assert!(refused.contains("the runner says: no room"), "{refused}");
    assert!(refused.contains("Nothing was started"), "{refused}");
}

/// **A runner's own profile of that name is never taken over** (#2049): its own sandbox-only
/// `work` of the same harness is refused, and stays its own and sandbox-only.
#[test]
fn a_runners_own_sandbox_only_profile_of_that_name_is_refused_and_stays_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let runner = homes(dir.path(), "runner");
    crate::profilestore::create_sandbox_only(&runner, "work", "claude", 1).unwrap();
    let mut desktop = a_signed_in_desktop_profile(dir.path());
    desktop.sandbox_only = false;

    let refused = keep(
        &runner,
        &runner_home(dir.path()),
        &Kept::of(&desktop).unwrap(),
        DESKTOP,
        7,
    )
    .unwrap_err();

    assert!(refused.contains("of its own"), "{refused}");
    let own = &list(runner.config_root()).unwrap()[0];
    assert!(own.sandbox_only);
    assert_eq!(own.kept_for, None);
}

/// A profile kept for another desktop paired before is not this desktop's to bring up to date.
#[test]
fn a_profile_kept_for_another_desktop_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let runner = homes(dir.path(), "runner");
    let desktop = a_signed_in_desktop_profile(dir.path());
    let home = runner_home(dir.path());
    keep(
        &runner,
        &home,
        &Kept::of(&desktop).unwrap(),
        "01JA0OTHER00000000000000000",
        7,
    )
    .unwrap();
    let adopted = the_desktops_adopted_folder(dir.path());
    keep(
        &runner,
        &home,
        &Kept::of(&adopted).unwrap(),
        "01JA0OTHER00000000000000000",
        7,
    )
    .unwrap();

    for kept in [Kept::of(&desktop).unwrap(), Kept::of(&adopted).unwrap()] {
        let refused = keep(&runner, &home, &kept, DESKTOP, 9).unwrap_err();
        assert!(refused.contains("of its own"), "{}: {refused}", kept.name);
    }
}

/// A runner's own adopted folder somewhere else than its harness's default, or reached through
/// a variable, is its own: the desktop's adopted folder is never kept in its place.
#[test]
fn a_runners_own_folder_adopted_elsewhere_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let runner = homes(dir.path(), "runner");
    let home = runner_home(dir.path());
    let mut own = the_desktops_adopted_folder(dir.path());
    own.folders.config = dir.path().join("somewhere/else");
    own.pinned = None;
    crate::profilestore::update(runner.config_root(), |store| {
        store.profiles.push(own.clone())
    })
    .unwrap();

    let refused = keep(
        &runner,
        &home,
        &Kept::of(&the_desktops_adopted_folder(dir.path())).unwrap(),
        DESKTOP,
        7,
    )
    .unwrap_err();
    assert!(refused.contains("of its own"), "{refused}");

    // At its default folder but reached through a variable: its own too.
    own.folders.config = home.join(".claude");
    own.adopted = Some(Adopted {
        at: 1,
        variables: vec!["CLAUDE_CONFIG_DIR".to_owned()],
    });
    crate::profilestore::update(runner.config_root(), |store| {
        store.profiles[0] = own.clone()
    })
    .unwrap();
    assert!(
        keep(
            &runner,
            &home,
            &Kept::of(&the_desktops_adopted_folder(dir.path())).unwrap(),
            DESKTOP,
            7,
        )
        .is_err()
    );
}

/// **Two keeps at once make one record** (#2049): each finds or makes it under the store's one
/// lock, so the second finds the first's, and both answer it.
#[test]
fn two_keeps_at_once_make_one_record_and_both_answer_it() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_signed_in_desktop_profile(dir.path());
    let runner = homes(dir.path(), "runner");
    let home = runner_home(dir.path());
    let kept = Kept::of(&desktop).unwrap();
    let adopted = Kept::of(&the_desktops_adopted_folder(dir.path())).unwrap();

    let answers: Vec<_> = std::thread::scope(|scope| {
        let keeps: Vec<_> = (0..4)
            .map(|at| {
                let (runner, home, kept, adopted) = (&runner, &home, &kept, &adopted);
                scope.spawn(move || {
                    (
                        keep(runner, home, kept, DESKTOP, at),
                        keep(runner, home, adopted, DESKTOP, at),
                    )
                })
            })
            .collect();
        keeps.into_iter().map(|one| one.join().unwrap()).collect()
    });

    for (made, adopted) in &answers {
        assert_eq!(
            made.as_ref().unwrap().folders,
            answers[0].0.as_ref().unwrap().folders
        );
        assert!(adopted.is_ok(), "{adopted:?}");
    }
    let names: Vec<String> = list(runner.config_root())
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
}

/// **Only a plan a harness serves is named for its sign-in** (#2049, L2): opencode's ChatGPT,
/// and nothing else; a harness of one plan takes none.
#[test]
fn only_a_plan_its_harness_serves_is_named_for_a_sign_in() {
    assert_eq!(sign_in::plan_named("opencode", Some("openai")), Ok(()));
    assert_eq!(sign_in::plan_named("opencode", None), Ok(()));
    assert!(sign_in::plan_named("opencode", Some("anthropic")).is_err());
    assert!(sign_in::plan_named("claude", Some("openai")).is_err());
    assert_eq!(sign_in::plan_named("claude", None), Ok(()));
}

/// A move refused for its profile says the chat did not move and to ask for the move again,
/// never that nothing was started or to start the chat again (#2048).
#[test]
fn a_move_refused_for_its_profile_says_to_ask_for_the_move_again() {
    let dir = tempfile::tempdir().unwrap();
    a_signed_in_desktop_profile(dir.path());
    let desktop = homes(dir.path(), "desktop");
    let project = dir.path().join("project-moving");

    let signed_out = ready_to_move_on(desktop.config_root(), &project, "builds", "work", &|_| {
        Ok(Some(Standing::SignedOut))
    })
    .unwrap_err();
    let not_kept = ready_to_move_on(desktop.config_root(), &project, "builds", "work", &|_| {
        Err("the runner did not answer".into())
    })
    .unwrap_err();
    for refused in [&signed_out, &not_kept] {
        assert!(refused.contains("The chat did not move"), "{refused}");
        assert!(!refused.contains("Nothing was started"), "{refused}");
        assert!(!refused.contains("start the chat again"), "{refused}");
    }
    assert!(
        signed_out.contains("ask for the move again"),
        "{signed_out}"
    );
    assert!(
        signed_out.contains("Profile work on pool builds"),
        "{signed_out}"
    );
    let_go(None, "builds", "work");
}
