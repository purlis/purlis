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
/// a runner signed in before stays signed in. It carries the desktop's pin, which the runner
/// installs (#3446), and follows as an adopted folder's pin does.
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
    assert_eq!(
        kept.pinned.as_ref().map(|p| p.version.as_str()),
        Some("2.1.300")
    );
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
    assert!(sign_in::login_of(&adopted, None, None).is_err());
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

/// **Letting a pool's waits go says whose went** (#4205): each of those windows has an Ask to
/// stop drawing.
#[test]
fn letting_a_pool_s_waits_go_says_whose_went() {
    let dir = tempfile::tempdir().unwrap();
    a_signed_in_desktop_profile(dir.path());
    let desktop = homes(dir.path(), "desktop");
    let (one, two) = (dir.path().join("told-1"), dir.path().join("told-2"));
    for root in [&one, &two] {
        ready_on(desktop.config_root(), root, "told", "work", &|_| {
            Ok(Some(Standing::SignedOut))
        })
        .unwrap_err();
    }

    let mut went = let_go(None, "told", "work");
    went.sort();

    assert_eq!(went, [one, two]);
    assert!(let_go(None, "told", "work").is_empty(), "none left to go");
}

/// **A second refusal counts as raised then** (#4203): the usual case, a project whose Ask is why
/// Sign in was pressed. How long it has waited does not change.
#[test]
fn a_start_on_a_pool_refused_again_after_the_shell_was_last_seen_keeps_the_wait() {
    let dir = tempfile::tempdir().unwrap();
    a_signed_in_desktop_profile(dir.path());
    let desktop = homes(dir.path(), "desktop");
    let project = dir.path().join("refused-again");
    let refuse = || {
        ready_on(desktop.config_root(), &project, "again", "work", &|_| {
            Ok(Some(Standing::SignedOut))
        })
        .unwrap_err();
    };
    refuse();
    let since = waiting_on_pools(&project)[0].since;
    let last_seen = std::time::Instant::now();

    refuse();
    let_go_older(&project, "again", "work", last_seen);

    let waits = waiting_on_pools(&project);
    assert_eq!(waits.len(), 1);
    assert_eq!(waits[0].since, since);
    let_go(None, "again", "work");
}

/// **Leaving a sign-in on a pool lets go only waits older than the moment the shell was last
/// seen open, and only in its project** (#4203).
#[test]
fn leaving_a_sign_in_on_a_pool_keeps_a_wait_raised_after_the_shell_was_last_seen() {
    let dir = tempfile::tempdir().unwrap();
    a_signed_in_desktop_profile(dir.path());
    let desktop = homes(dir.path(), "desktop");
    let (project, other) = (dir.path().join("left"), dir.path().join("other"));
    let refuse = |root: &Path| {
        ready_on(desktop.config_root(), root, "builds", "work", &|_| {
            Ok(Some(Standing::SignedOut))
        })
        .unwrap_err();
    };
    refuse(&project);
    refuse(&other);
    let last_seen = std::time::Instant::now();

    let_go_older(&project, "builds", "work", last_seen);
    assert!(waiting_on_pools(&project).is_empty(), "older: it goes");
    assert_eq!(waiting_on_pools(&other).len(), 1, "another project's stays");
    let gone = std::time::Instant::now();

    refuse(&project);
    let_go_older(&project, "builds", "work", gone);
    assert_eq!(
        waiting_on_pools(&project).len(),
        1,
        "raised since: it stays"
    );
    let_go(None, "builds", "work");
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

/// A move whose profile is not signed in on the pool waits for it to sign in (#3477): it is
/// told as a sign-in to ask, naming the profile and its harness, and no Inbox wait is added, since
/// the move's own notice asks it. A profile the runner did not keep refuses the move, saying the
/// chat did not move and to ask for the move again, never that nothing was started (#2048).
#[test]
fn a_move_whose_profile_is_signed_out_there_waits_for_its_sign_in() {
    let dir = tempfile::tempdir().unwrap();
    a_signed_in_desktop_profile(dir.path());
    let desktop = homes(dir.path(), "desktop");
    let project = dir.path().join("project-moving");

    let signed_out = ready_to_move_on(desktop.config_root(), "builds", "work", &|_| {
        Ok(Some(Standing::SignedOut))
    })
    .unwrap_err();
    assert_eq!(
        signed_out,
        NotYet::SignIn(SignInAsked {
            pool: "builds".into(),
            profile: "work".into(),
            kind: "claude".into(),
            harness: "Claude Code".into(),
        })
    );
    assert!(waiting_on_pools(&project).is_empty());
    let said = signed_out.said();
    assert!(said.contains("not signed in on pool builds"), "{said}");
    assert!(said.contains("The chat did not move"), "{said}");

    let not_kept = ready_to_move_on(desktop.config_root(), "builds", "work", &|_| {
        Err("the runner did not answer".into())
    })
    .unwrap_err();
    let NotYet::Refused(refused) = &not_kept else {
        panic!("{not_kept:?}");
    };
    assert!(refused.contains("The chat did not move"), "{refused}");
    assert!(!refused.contains("Nothing was started"), "{refused}");
    assert!(!refused.contains("start the chat again"), "{refused}");
    assert_eq!(not_kept.said(), *refused);
}

/// The desktop's made profile `work`, signed in, with settings beside its sign-in: Claude Code's
/// settings file and instructions, an agent, and in its agents folder a file named like a
/// credential.
fn a_desktop_profile_with_settings(dir: &Path) -> MachineProfile {
    let desktop = a_signed_in_desktop_profile(dir);
    let config = &desktop.folders.config;
    fs::write(config.join("CLAUDE.md"), "Be brief.").unwrap();
    fs::create_dir_all(config.join("agents")).unwrap();
    fs::write(config.join("agents/review.md"), "Review.").unwrap();
    fs::write(config.join("agents/credentials.json"), "a token").unwrap();
    fs::write(config.join("history.jsonl"), "a conversation").unwrap();
    desktop
}

/// What a runner keeps for the desktop's `profile`, kept as the desktop sends it at a start.
fn kept_with_its_settings(dir: &Path, profile: &MachineProfile) -> MachineProfile {
    let runner = homes(dir, "runner");
    let sent = to_send(&Kept::of(profile).unwrap().with_settings_of(profile)).expect("it is sent");
    kept_for_the_desktop(
        &runner,
        &runner_home(dir),
        DESKTOP,
        &sent,
        &Answers(Some(0)),
        7,
    )
    .expect("kept");
    list(runner.config_root())
        .unwrap()
        .into_iter()
        .find(|p| p.name == profile.name)
        .expect("kept there")
}

/// **A runner chat sees the profile's settings as a chat here does** (#3448): the settings files
/// a made profile imports reach the runner's folder of it, and no sign-in, no file named like a
/// credential, and nothing the list does not name.
#[test]
fn a_profiles_settings_reach_the_runner_that_keeps_it_and_no_credential_file_does() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_desktop_profile_with_settings(dir.path());

    let kept = kept_with_its_settings(dir.path(), &desktop);

    let config = &kept.folders.config;
    let mut files = files_under(config);
    files.sort();
    assert_eq!(
        files,
        vec![
            PathBuf::from("CLAUDE.md"),
            PathBuf::from("agents/review.md"),
            PathBuf::from("settings.json"),
        ]
    );
    let settings: serde_json::Value =
        serde_json::from_slice(&fs::read(config.join("settings.json")).unwrap()).unwrap();
    assert_eq!(settings, serde_json::json!({"theme": "dark"}));
    assert_eq!(
        fs::read_to_string(config.join("agents/review.md")).unwrap(),
        "Review."
    );
}

/// Sent again after the desktop's settings changed, the runner keeps them as they are now: a
/// file gone from the desktop goes from the runner, and its own sign-in there stays.
#[test]
fn settings_sent_again_replace_the_ones_the_runner_kept() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_desktop_profile_with_settings(dir.path());
    let kept = kept_with_its_settings(dir.path(), &desktop);
    let signed_in_there = kept.folders.config.join(".credentials.json");
    fs::write(&signed_in_there, "the runner's own sign-in").unwrap();

    fs::remove_file(desktop.folders.config.join("agents/review.md")).unwrap();
    fs::write(desktop.folders.config.join("CLAUDE.md"), "Be thorough.").unwrap();
    let kept = kept_with_its_settings(dir.path(), &desktop);

    let config = &kept.folders.config;
    assert!(!config.join("agents/review.md").exists());
    assert_eq!(
        fs::read_to_string(config.join("CLAUDE.md")).unwrap(),
        "Be thorough."
    );
    assert_eq!(
        fs::read_to_string(&signed_in_there).unwrap(),
        "the runner's own sign-in"
    );
}

/// A keep that carries no settings (a look from Settings, a desktop before 1.50) leaves the
/// runner's as they are.
#[test]
fn a_keep_with_no_settings_leaves_the_runners_as_they_are() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_desktop_profile_with_settings(dir.path());
    let kept = kept_with_its_settings(dir.path(), &desktop);
    let runner = homes(dir.path(), "runner");

    let sent: purlis_session_protocol::session::ProfileToKeep = Kept::of(&desktop).unwrap().into();
    assert_eq!(sent.settings, None);
    kept_for_the_desktop(
        &runner,
        &runner_home(dir.path()),
        DESKTOP,
        &sent,
        &Answers(Some(0)),
        9,
    )
    .unwrap();

    assert!(kept.folders.config.join("CLAUDE.md").is_file());
}

/// **The desktop's adopted folder's settings reach the runner's own default folder** that keeps
/// it, and its sign-in does not.
#[test]
fn the_adopted_folders_settings_reach_the_runners_default_folder() {
    let dir = tempfile::tempdir().unwrap();
    let adopted = the_desktops_adopted_folder(dir.path());
    fs::create_dir_all(&adopted.folders.config).unwrap();
    fs::write(adopted.folders.config.join("settings.json"), "{}").unwrap();
    fs::write(
        adopted.folders.config.join(".credentials.json"),
        "a sign-in",
    )
    .unwrap();

    let kept = kept_with_its_settings(dir.path(), &adopted);

    assert_eq!(kept.folders.config, runner_home(dir.path()).join(".claude"));
    assert_eq!(
        files_under(&kept.folders.config),
        vec![PathBuf::from("settings.json")]
    );
}

/// A profile for sandboxed chats only has purlis's settings, so none of the desktop's is sent.
#[test]
fn a_sandbox_only_profile_sends_no_settings() {
    let dir = tempfile::tempdir().unwrap();
    let mut desktop = a_desktop_profile_with_settings(dir.path());
    desktop.sandbox_only = true;

    let sent = to_send(&Kept::of(&desktop).unwrap().with_settings_of(&desktop)).unwrap();

    assert_eq!(sent.settings, None);
}

/// Settings larger than a link carries are not sent, and the profile is refused in words.
#[test]
fn settings_too_large_for_the_link_are_refused_in_words() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_desktop_profile_with_settings(dir.path());
    fs::write(
        desktop.folders.config.join("agents/large.md"),
        vec![b'a'; MOST_SETTINGS_BYTES + 1],
    )
    .unwrap();

    let refused = to_send(&Kept::of(&desktop).unwrap().with_settings_of(&desktop)).unwrap_err();

    assert!(refused.contains("larger than"), "{refused}");
}

/// **A profile with 17 MiB of skills reaches its runner** (#4081), the size of the one whose every
/// chat was refused on a real pool, in batches and whole.
#[test]
fn a_profile_with_several_mib_of_skills_is_sent_to_its_runner() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_desktop_profile_with_settings(dir.path());
    let skill = desktop.folders.config.join("skills/large");
    fs::create_dir_all(&skill).unwrap();
    for one in 0..170 {
        fs::write(skill.join(format!("{one}.md")), vec![b'a'; 100 * 1024]).unwrap();
    }

    let sent = in_batches(&Kept::of(&desktop).unwrap().with_settings_of(&desktop)).unwrap();
    assert!(sent.batches.len() > 1, "{} batches", sent.batches.len());
    assert!(sent.left_behind.is_empty());

    let runner = homes(dir.path(), "runner");
    let record = kept_staged(&runner, dir.path(), sent).expect("it is kept");
    let settings = &record.folders.config;
    assert_eq!(
        fs::read_dir(settings.join("skills/large")).unwrap().count(),
        170
    );
    assert_eq!(
        fs::read(settings.join("skills/large/7.md")).unwrap().len(),
        100 * 1024
    );
    assert_eq!(fs::read(settings.join("CLAUDE.md")).unwrap(), b"Be brief.");
    assert!(!settings.join("agents/credentials.json").exists());
    assert_eq!(staged_left_in(settings), 0);
}

/// [`in_batches`]'s `sent`, staged as a link stages them and kept as a runner keeps them.
fn kept_staged(runner: &Homes, dir: &Path, sent: ToKeep) -> Result<MachineProfile, String> {
    use purlis_session_protocol::session::StagedSettings;
    let mut staged = StagedSettings::default();
    for batch in sent.batches {
        staged.add(batch)?;
    }
    let mut profile = sent.profile;
    let files = profile.settings_staged.take().expect("it names its files");
    profile.settings = Some(staged.finished(&profile.name, files)?);
    kept_for_the_desktop(
        runner,
        &runner_home(dir),
        DESKTOP,
        &profile,
        &Answers(Some(0)),
        7,
    )?;
    Ok(list(runner.config_root())
        .unwrap()
        .into_iter()
        .find(|p| p.name == profile.name)
        .expect("kept"))
}

/// How many stages a keep left in the settings folder `settings`.
fn staged_left_in(settings: &Path) -> usize {
    fs::read_dir(settings)
        .unwrap()
        .filter(|one| {
            let name = one.as_ref().unwrap().file_name();
            name.to_string_lossy().contains("staged")
        })
        .count()
}

/// **Nothing refused before is sent now, however many batches they take**: a file named like a
/// credential and a sign-in inside a settings file stay behind, one split across batches too,
/// and a settings file that does not parse refuses all of them.
#[test]
fn batches_send_nothing_that_was_refused_before() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_desktop_profile_with_settings(dir.path());
    let config = &desktop.folders.config;
    fs::write(
        config.join("settings.json"),
        r#"{"env":{"TOKEN":"a-signed-in-token"},"theme":"dark"}"#,
    )
    .unwrap();
    fs::create_dir_all(config.join("skills/large")).unwrap();
    fs::write(
        config.join("skills/large/SKILL.md"),
        vec![b'a'; 3 * 1024 * 1024],
    )
    .unwrap();
    fs::write(config.join(".credentials.json"), "a sign-in").unwrap();
    fs::create_dir_all(config.join("skills/one")).unwrap();
    fs::write(config.join("skills/one/.env"), "a secret").unwrap();
    let kept = Kept::of(&desktop).unwrap().with_settings_of(&desktop);

    let sent = in_batches(&kept).unwrap();

    let pieces: Vec<_> = sent.batches.iter().flat_map(|b| &b.pieces).collect();
    assert!(
        pieces
            .iter()
            .filter(|p| p.path == "skills/large/SKILL.md")
            .count()
            > 1
    );
    assert!(pieces.iter().all(|p| p.path != ".credentials.json"
        && p.path != "skills/one/.env"
        && p.path != "agents/credentials.json"));
    let settings: Vec<u8> = pieces
        .iter()
        .filter(|p| p.path == "settings.json")
        .flat_map(|p| p.bytes.clone())
        .collect();
    assert!(!String::from_utf8_lossy(&settings).contains("a-signed-in-token"));

    fs::write(config.join("settings.json"), "{ not json").unwrap();
    assert!(in_batches(&kept).is_err());
}

/// **A runner checks what it stages as it checks what one keep carries**: a sign-in split across
/// a desktop's pieces is removed once the file is whole, whatever sent it.
#[test]
fn a_runner_removes_a_sign_in_a_sender_split_across_pieces() {
    use purlis_session_protocol::session::{SettingsBatch, SettingsPiece};
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_desktop_profile_with_settings(dir.path());
    let runner = homes(dir.path(), "runner");
    fs::remove_file(desktop.folders.config.join("settings.json")).unwrap();
    let whole = br#"{"env":{"TOKEN":"a-signed-in-token"},"theme":"dark"}"#;
    let (first, second) = whole.split_at(20);
    let mut sent = in_batches(&Kept::of(&desktop).unwrap().with_settings_of(&desktop)).unwrap();
    sent.batches.push(SettingsBatch {
        profile: desktop.name.clone(),
        pieces: vec![
            SettingsPiece {
                path: "settings.json".to_owned(),
                continues: false,
                bytes: first.to_vec(),
            },
            SettingsPiece {
                path: "settings.json".to_owned(),
                continues: true,
                bytes: second.to_vec(),
            },
        ],
    });
    sent.profile.settings_staged = sent.profile.settings_staged.map(|files| files + 1);

    let record = kept_staged(&runner, dir.path(), sent).unwrap();

    let kept = fs::read_to_string(record.folders.config.join("settings.json")).unwrap();
    assert!(!kept.contains("a-signed-in-token"), "{kept}");
    assert!(kept.contains("dark"), "{kept}");
}

/// **A keep that fails leaves the settings as they were** (#4081): files that cannot both be
/// written (`skills/a` as a file and as a folder) stop at the stage, with nothing of the settings
/// the runner kept touched and no stage left, and the next keep goes through.
#[test]
fn a_keep_that_fails_leaves_the_settings_it_kept() {
    use purlis_session_protocol::session::SettingsFile;
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_desktop_profile_with_settings(dir.path());
    let runner = homes(dir.path(), "runner");
    let record = kept_with_its_settings(dir.path(), &desktop);
    let settings = record.folders.config.clone();

    let mut profile = to_send(&Kept::of(&desktop).unwrap()).unwrap();
    profile.settings = Some(vec![
        SettingsFile {
            path: "agents/new.md".to_owned(),
            bytes: b"new".to_vec(),
        },
        SettingsFile {
            path: "skills/a".to_owned(),
            bytes: b"a file".to_vec(),
        },
        SettingsFile {
            path: "skills/a/b.md".to_owned(),
            bytes: b"under a file".to_vec(),
        },
    ]);
    kept_for_the_desktop(
        &runner,
        &runner_home(dir.path()),
        DESKTOP,
        &profile,
        &Answers(Some(0)),
        7,
    )
    .unwrap_err();

    assert_eq!(
        fs::read(settings.join("agents/review.md")).unwrap(),
        b"Review."
    );
    assert!(!settings.join("agents/new.md").exists());
    assert_eq!(staged_left_in(&settings), 0);
    let stages = runner.folder_of(".settings-staged");
    assert_eq!(fs::read_dir(stages).unwrap().count(), 0);

    assert_eq!(
        kept_with_its_settings(dir.path(), &desktop).folders.config,
        settings
    );
    assert_eq!(fs::read(settings.join("CLAUDE.md")).unwrap(), b"Be brief.");
}

/// **The stage is where no chat writes** (#4081): nothing of a keep appears in a settings folder
/// a chat can write in, a stage a killed keep left is swept, and one that cannot be swept stands
/// in no keep's way.
#[cfg(unix)]
#[test]
fn a_stage_is_kept_apart_from_the_settings_folder_and_what_a_killed_keep_left_goes() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_desktop_profile_with_settings(dir.path());
    let record = kept_with_its_settings(dir.path(), &desktop);
    let settings = record.folders.config.clone();
    fs::write(settings.join("history.jsonl"), "the runner's own").unwrap();
    let stages = homes(dir.path(), "runner").folder_of(".settings-staged");
    let stuck = stages.join("1.killed/new/stuck");
    fs::create_dir_all(&stuck).unwrap();
    fs::write(stuck.join("a"), "a").unwrap();
    fs::set_permissions(&stuck, fs::Permissions::from_mode(0o500)).unwrap();

    kept_with_its_settings(dir.path(), &desktop);

    fs::set_permissions(&stuck, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        fs::read(settings.join("history.jsonl")).unwrap(),
        b"the runner's own"
    );
    assert_eq!(staged_left_in(&settings), 0);
    assert_eq!(fs::read(settings.join("CLAUDE.md")).unwrap(), b"Be brief.");
}

/// **Over the overall limit a profile still goes, by whole skills** (#4081): a skill with a file
/// past the limit stays behind whole, what was read of it with it, and the rest goes; the note
/// says what, its size and how to bring it.
#[test]
fn over_the_overall_limit_whole_skills_stay_behind_and_the_rest_goes() {
    use purlis_session_protocol::session::MOST_PROFILE_SETTINGS_BYTES;
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_desktop_profile_with_settings(dir.path());
    let config = &desktop.folders.config;
    let skill = config.join("skills/large");
    fs::create_dir_all(&skill).unwrap();
    fs::write(skill.join("SKILL.md"), "Use me.").unwrap();
    // Sparse: its size is what an over-limit file says, and nothing of it is read.
    let over = fs::File::create(skill.join("weights.bin")).unwrap();
    over.set_len(MOST_PROFILE_SETTINGS_BYTES as u64 + 1)
        .unwrap();
    fs::create_dir_all(config.join("skills/small")).unwrap();
    fs::write(config.join("skills/small/SKILL.md"), "Small.").unwrap();

    let sent = in_batches(&Kept::of(&desktop).unwrap().with_settings_of(&desktop)).unwrap();

    assert_eq!(sent.left_behind.len(), 1, "{:?}", sent.left_behind);
    assert_eq!(sent.left_behind[0].path, "skills/large");
    let paths: Vec<_> = sent
        .batches
        .iter()
        .flat_map(|b| &b.pieces)
        .map(|p| &p.path)
        .collect();
    assert!(paths.iter().any(|p| p.as_str() == "skills/small/SKILL.md"));
    assert!(paths.iter().any(|p| p.as_str() == "CLAUDE.md"));
    assert!(!paths.iter().any(|p| p.starts_with("skills/large")));

    let note = left_behind_note("builds", &desktop.name, &sent.left_behind).unwrap();
    assert!(note.contains("skills/large (129 MiB)"), "{note}");
    assert!(note.contains("smaller"), "{note}");
    assert_eq!(left_behind_note("builds", "work", &[]), None);
}

/// A note of many names names the largest, and counts the rest.
#[test]
fn a_note_of_many_names_the_largest_and_counts_the_rest() {
    let behind: Vec<_> = (1..=9u64)
        .map(|one| LeftBehind {
            path: format!("skills/{one}"),
            bytes: one * 1024 * 1024,
        })
        .collect();

    let note = left_behind_note("builds", "work", &behind).unwrap();

    assert!(note.contains("skills/9 (9 MiB), skills/8"), "{note}");
    assert!(note.contains("3 more"), "{note}");
    assert!(!note.contains("skills/1 "), "{note}");
}

/// **A harness's own settings file that cannot go refuses the chat** (#4081): it says what a chat
/// may do, so no chat starts as if it had said nothing. A larger `CLAUDE.md` is only left behind.
#[test]
fn a_harness_settings_file_too_large_to_go_refuses_the_chat() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_desktop_profile_with_settings(dir.path());
    let config = &desktop.folders.config;
    let kept = Kept::of(&desktop).unwrap().with_settings_of(&desktop);
    fs::write(
        config.join("settings.json"),
        format!(r#"{{"a":"{}"}}"#, "a".repeat(MOST_SETTINGS_BYTES)),
    )
    .unwrap();

    let refused = in_batches(&kept).unwrap_err();

    assert!(refused.contains("larger than"), "{refused}");
    let over = fs::File::create(config.join("settings.json")).unwrap();
    over.set_len(1 + purlis_session_protocol::session::MOST_PROFILE_SETTINGS_BYTES as u64)
        .unwrap();
    assert!(in_batches(&kept).unwrap_err().contains("larger than"));
}

/// **What a runner has no use for does not go** (#4081): a skill's `node_modules`, `__pycache__`
/// and `.git` stay behind unsaid, and what a made profile imports on this machine is not changed.
#[test]
fn a_skills_build_folders_do_not_go_to_a_runner() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_desktop_profile_with_settings(dir.path());
    let skill = desktop.folders.config.join("skills/built");
    for folder in [
        "node_modules/left",
        "__pycache__",
        ".git",
        "venv/bin",
        "scripts",
    ] {
        fs::create_dir_all(skill.join(folder)).unwrap();
        fs::write(skill.join(folder).join("a.txt"), "a").unwrap();
    }

    let sent = in_batches(&Kept::of(&desktop).unwrap().with_settings_of(&desktop)).unwrap();

    let paths: Vec<_> = sent
        .batches
        .iter()
        .flat_map(|b| &b.pieces)
        .map(|p| p.path.as_str())
        .filter(|p| p.starts_with("skills/built"))
        .collect();
    assert_eq!(paths, ["skills/built/scripts/a.txt"]);
    assert!(sent.left_behind.is_empty());
}

/// More files than a runner holds stay behind too, unread, rather than refusing the chat.
#[test]
fn files_past_the_limit_stay_behind_too() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_desktop_profile_with_settings(dir.path());
    let config = &desktop.folders.config;

    let read =
        crate::profilestore::made::settings_read("claude", config, 1024, 1, &|_| true).unwrap();

    assert_eq!(read.files.len(), 1);
    assert!(!read.left_behind.is_empty());
}

/// **A large skill keeps out as little as it can** (#4081): what stays behind is chosen by size
/// before anything is read, so one of 20,000 files spends nothing of the limit on itself, and the
/// harness's own files go first, whatever their size.
#[test]
fn a_large_skill_costs_the_ones_after_it_nothing() {
    let wholes = Measured::from([
        ("CLAUDE.md".to_owned(), (60, 1)),
        ("agents/a.md".to_owned(), (10, 1)),
        ("skills/a-vendored".to_owned(), (40, 20_000)),
        ("skills/b".to_owned(), (20, 2)),
        ("skills/c".to_owned(), (20, 2)),
        ("skills/d-large".to_owned(), (70, 3)),
    ]);

    let (going, behind) = wholes_that_fit(wholes, 110, 8192);

    let going: Vec<_> = going.iter().map(String::as_str).collect();
    assert_eq!(going, ["CLAUDE.md", "agents/a.md", "skills/b", "skills/c"]);
    assert_eq!(
        behind,
        BTreeMap::from([
            ("skills/a-vendored".to_owned(), 40),
            ("skills/d-large".to_owned(), 70),
        ])
    );
}

/// A skill goes or stays whole; an agent is a whole of its own.
#[test]
fn a_skill_goes_or_stays_whole() {
    assert_eq!(whole_of("skills/a/deep/SKILL.md"), "skills/a");
    assert_eq!(whole_of("agents/review.md"), "agents/review.md");
    assert_eq!(whole_of("CLAUDE.md"), "CLAUDE.md");
}

/// **A runner keeps no settings file it would not import**: one named like a credential, a
/// hidden one, one the list does not name, and one outside the settings folder are each refused,
/// and nothing of what was sent is written.
#[test]
fn a_runner_refuses_a_settings_file_named_like_a_credential_or_outside_the_list() {
    use purlis_session_protocol::session::SettingsFile;
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_desktop_profile_with_settings(dir.path());
    let runner = homes(dir.path(), "runner");
    for path in [
        ".credentials.json",
        "agents/credentials.json",
        "agents/.hidden.md",
        "history.jsonl",
        "../settings.json",
        "agents/../../x.md",
        "/etc/passwd",
        "agents",
        "settings.json/x",
        "",
    ] {
        let mut sent: purlis_session_protocol::session::ProfileToKeep =
            Kept::of(&desktop).unwrap().into();
        sent.settings = Some(vec![
            SettingsFile {
                path: "CLAUDE.md".to_owned(),
                bytes: b"Be brief.".to_vec(),
            },
            SettingsFile {
                path: path.to_owned(),
                bytes: b"x".to_vec(),
            },
        ]);

        let refused = kept_for_the_desktop(
            &runner,
            &runner_home(dir.path()),
            DESKTOP,
            &sent,
            &Answers(Some(0)),
            7,
        )
        .unwrap_err();

        assert!(refused.contains("settings"), "{path}: {refused}");
        assert!(
            !runner.folder_of("work").join("config/CLAUDE.md").exists(),
            "{path}: nothing written"
        );
    }
}

/// **No sign-in rides inside a settings file to the runner** (D-3448-8): a subscription's token
/// and a key in Claude Code's `env`, and a helper that prints one, are not in what the runner
/// keeps, and the rest of the settings is.
#[test]
fn a_token_in_a_settings_files_env_never_reaches_the_runner() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_signed_in_desktop_profile(dir.path());
    fs::write(
        desktop.folders.config.join("settings.json"),
        r#"{"theme":"dark","apiKeyHelper":"/bin/key","env":{"CLAUDE_CODE_OAUTH_TOKEN":"oauth-of-the-desktop","ANTHROPIC_API_KEY":"key-of-the-desktop"}}"#,
    )
    .unwrap();

    let sent = to_send(&Kept::of(&desktop).unwrap().with_settings_of(&desktop)).unwrap();
    let kept = kept_with_its_settings(dir.path(), &desktop);

    let on_the_wire = format!(
        "{:?}",
        sent.settings.as_ref().map(|s| s
            .iter()
            .map(|f| String::from_utf8_lossy(&f.bytes).into_owned())
            .collect::<Vec<_>>())
    );
    assert!(!on_the_wire.contains("desktop"), "{on_the_wire}");
    let there = fs::read_to_string(kept.folders.config.join("settings.json")).unwrap();
    assert!(!there.contains("of-the-desktop"), "{there}");
    assert!(!there.contains("apiKeyHelper"), "{there}");
    assert!(there.contains("\"theme\""), "{there}");
}

/// A settings file the harness could not read refuses the profile's settings in words, and the
/// runner keeps none of them.
#[test]
fn a_settings_file_that_does_not_parse_is_refused_and_nothing_is_sent() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_signed_in_desktop_profile(dir.path());
    fs::write(
        desktop.folders.config.join("settings.json"),
        "{\"env\": {\"CLAUDE_CODE_OAUTH_TOKEN\": \"oauth-of-the-desktop",
    )
    .unwrap();

    let refused = to_send(&Kept::of(&desktop).unwrap().with_settings_of(&desktop)).unwrap_err();

    assert!(
        refused.contains("not one its harness can read"),
        "{refused}"
    );
    assert!(!refused.contains("of-the-desktop"), "{refused}");
}

/// Settings a runner refuses leave its record of the profile as it was: nothing of the keep is
/// written before they are checked.
#[test]
fn settings_a_runner_refuses_leave_its_record_as_it_was() {
    use purlis_session_protocol::session::SettingsFile;
    let dir = tempfile::tempdir().unwrap();
    let mut desktop = a_signed_in_desktop_profile(dir.path());
    let runner = homes(dir.path(), "runner");
    kept_with_its_settings(dir.path(), &desktop);

    desktop.pinned = Some(crate::profilestore::pin::Pinned {
        version: "2.1.300".to_owned(),
        at: 3,
    });
    let mut sent: purlis_session_protocol::session::ProfileToKeep =
        Kept::of(&desktop).unwrap().into();
    sent.settings = Some(vec![SettingsFile {
        path: "history.jsonl".to_owned(),
        bytes: b"x".to_vec(),
    }]);
    kept_for_the_desktop(
        &runner,
        &runner_home(dir.path()),
        DESKTOP,
        &sent,
        &Answers(Some(0)),
        9,
    )
    .unwrap_err();

    let record = list(runner.config_root())
        .unwrap()
        .into_iter()
        .find(|p| p.name == "work")
        .unwrap();
    assert_eq!(record.pinned, None, "the refused keep changed nothing");
}

/// A runner that answers every status with 0 and has installed `pinned` for each profile it
/// keeps with a pin: the program each status was asked of.
struct Installed {
    pinned: PathBuf,
    programs: std::cell::RefCell<Vec<String>>,
}

impl Look for Installed {
    fn status(&self, program: &str, _: &[&str], _: &[(String, String)]) -> Option<i32> {
        self.programs.borrow_mut().push(program.to_owned());
        Some(0)
    }
    fn is_there(&self, _: &Path) -> bool {
        false
    }
    fn pinned_program(&self, record: &MachineProfile) -> Option<PathBuf> {
        (record.kept_for.is_some() && record.pinned.is_some()).then(|| self.pinned.clone())
    }
}

/// **What a runner answers its desktop's keep is asked of the pinned version it installs**
/// (#4007): a runner image carries no harness on `PATH`, so a profile's row says whether it is
/// signed in there once its version is installed, not that its harness cannot say.
#[test]
fn a_kept_pinned_profiles_standing_is_asked_of_the_version_the_runner_installs() {
    let dir = tempfile::tempdir().unwrap();
    let mut desktop = a_signed_in_desktop_profile(dir.path());
    desktop.pinned = Some(crate::profilestore::pin::Pinned {
        version: "2.1.300".to_owned(),
        at: 3,
    });
    let runner = homes(dir.path(), "runner");
    let installed = dir
        .path()
        .join("runner-data/harnesses/claude/2.1.300/claude");
    let look = Installed {
        pinned: installed.clone(),
        programs: std::cell::RefCell::new(Vec::new()),
    };

    let said = kept_for_the_desktop(
        &runner,
        &runner_home(dir.path()),
        DESKTOP,
        &Kept::of(&desktop).unwrap().into(),
        &look,
        7,
    )
    .expect("kept");

    assert_eq!(
        said,
        purlis_session_protocol::session::ProfileStanding::SignedIn
    );
    assert_eq!(*look.programs.borrow(), [installed.display().to_string()]);
}

/// **A profile the desktop takes off a pool leaves its runner, sign-in included** (#4047): the
/// runner's record goes, and the folder it made for it, the harness's sign-in in it, is deleted.
#[test]
fn a_made_profile_forgotten_leaves_the_runner_with_its_sign_in() {
    let dir = tempfile::tempdir().unwrap();
    let runner = homes(dir.path(), "runner");
    let home = runner_home(dir.path());
    let desktop = a_signed_in_desktop_profile(dir.path());
    let kept = keep(&runner, &home, &Kept::of(&desktop).unwrap(), DESKTOP, 7).unwrap();
    fs::write(
        kept.folders.config.join(".credentials.json"),
        "{\"claudeAiOauth\":\"the runner's own sign-in\"}",
    )
    .unwrap();

    forget(&runner, &home, "work", DESKTOP).expect("forgotten");

    assert!(list(runner.config_root()).unwrap().is_empty());
    assert!(!runner.folder_of("work").exists(), "its folder is gone");
}

/// **The desktop's adopted folder, kept as the runner's default folder, leaves it too**
/// (#4047): the harness's own folder in the runner's home, where it keeps its sign-in there, is
/// deleted with the record, and nothing else of the runner's home is.
#[test]
fn the_adopted_folder_forgotten_takes_the_harness_folder_on_the_runner_with_it() {
    let dir = tempfile::tempdir().unwrap();
    let runner = homes(dir.path(), "runner");
    let home = runner_home(dir.path());
    let adopted = the_desktops_adopted_folder(dir.path());
    keep(&runner, &home, &Kept::of(&adopted).unwrap(), DESKTOP, 7).unwrap();
    fs::create_dir_all(home.join(".claude")).unwrap();
    fs::write(home.join(".claude/.credentials.json"), "{}").unwrap();
    fs::create_dir_all(home.join(".codex")).unwrap();

    forget(&runner, &home, "claude", DESKTOP).expect("forgotten");

    assert!(list(runner.config_root()).unwrap().is_empty());
    assert!(!home.join(".claude").exists(), "its sign-in went with it");
    assert!(
        home.join(".codex").is_dir(),
        "another harness's folder stays"
    );
}

/// opencode keeps its files in a folder of its own name under the XDG folders: only those go,
/// never the XDG folders themselves.
#[test]
fn an_adopted_opencode_folder_forgotten_takes_only_its_own_folders() {
    let dir = tempfile::tempdir().unwrap();
    let runner = homes(dir.path(), "runner");
    let home = runner_home(dir.path());
    let adopted = MachineProfile {
        name: "opencode".to_owned(),
        harness: "opencode".to_owned(),
        folders: Folders {
            config: dir.path().join("desktop-home/.config"),
            data: Some(dir.path().join("desktop-home/.local/share")),
        },
        pinned: None,
        ..the_desktops_adopted_folder(dir.path())
    };
    keep(&runner, &home, &Kept::of(&adopted).unwrap(), DESKTOP, 7).unwrap();
    for folder in [".config/opencode", ".local/share/opencode", ".config/git"] {
        fs::create_dir_all(home.join(folder)).unwrap();
    }

    forget(&runner, &home, "opencode", DESKTOP).expect("forgotten");

    assert!(!home.join(".config/opencode").exists());
    assert!(!home.join(".local/share/opencode").exists());
    assert!(
        home.join(".config/git").is_dir(),
        "the XDG folder itself stays"
    );
}

/// A runner's own profile, or one kept for another desktop, is never forgotten for this one: it
/// stays as it is, and since nothing of this desktop's has that name, the desktop is told it is
/// forgotten, so its pool's record can still leave it out (#4060).
#[test]
fn a_profile_not_kept_for_this_desktop_stays_and_nothing_of_the_desktops_is_left() {
    let dir = tempfile::tempdir().unwrap();
    let runner = homes(dir.path(), "runner");
    let home = runner_home(dir.path());
    let desktop = a_signed_in_desktop_profile(dir.path());
    keep(
        &runner,
        &home,
        &Kept::of(&desktop).unwrap(),
        "01JA0OTHER00000000000000000",
        7,
    )
    .unwrap();

    assert_eq!(forget(&runner, &home, "work", DESKTOP), Ok(()));

    assert_eq!(list(runner.config_root()).unwrap().len(), 1);
    assert!(runner.folder_of("work").is_dir());
}

/// **A runner names the profiles it keeps for a desktop, and no others** (#4060): not one it
/// keeps for another desktop, nor one of its own.
#[test]
fn a_runner_names_the_profiles_it_keeps_for_the_desktop_and_no_others() {
    let dir = tempfile::tempdir().unwrap();
    let runner = homes(dir.path(), "runner");
    let home = runner_home(dir.path());
    let work = a_signed_in_desktop_profile(dir.path());
    keep(&runner, &home, &Kept::of(&work).unwrap(), DESKTOP, 7).unwrap();
    let adopted = the_desktops_adopted_folder(dir.path());
    keep(&runner, &home, &Kept::of(&adopted).unwrap(), DESKTOP, 7).unwrap();
    let theirs = Kept {
        name: "theirs".to_owned(),
        ..Kept::of(&work).unwrap()
    };
    keep(&runner, &home, &theirs, "01JA0OTHER00000000000000000", 7).unwrap();

    let mut named = kept_for(&runner, DESKTOP).unwrap();
    named.sort();

    assert_eq!(named, vec!["claude".to_owned(), "work".to_owned()]);
    assert!(
        kept_for(&runner, "01JA0NOBODY0000000000000000")
            .unwrap()
            .is_empty()
    );
}

/// Forgetting a profile the runner does not keep is nothing: the removal can be asked again.
#[test]
fn forgetting_a_profile_the_runner_does_not_keep_is_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let runner = homes(dir.path(), "runner");

    assert_eq!(
        forget(&runner, &runner_home(dir.path()), "work", DESKTOP),
        Ok(())
    );
}

/// **Only the desktop's adopted folder deletes a harness's own folders** (#4142): that is what a
/// chat there on no profile uses, and what its host holds back for.
#[test]
fn only_the_desktops_adopted_folder_deletes_its_harnesss_own_folders() {
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
    assert_eq!(kept.name, adopted.name);

    assert_eq!(
        deleting_own_folders(&runner, &adopted.name, DESKTOP),
        Ok(Some("claude".to_owned()))
    );
    assert_eq!(
        deleting_own_folders(&runner, &adopted.name, "another desktop"),
        Ok(None),
        "kept for another desktop, forget leaves it as it is"
    );
    assert_eq!(deleting_own_folders(&runner, "nothing", DESKTOP), Ok(None));
}

/// A runner fetching the version each profile it keeps with a pin is pinned to: none is
/// installed yet, and no harness on `PATH` answers.
struct StillFetching;

impl Look for StillFetching {
    fn status(&self, _: &str, _: &[&str], _: &[(String, String)]) -> Option<i32> {
        None
    }
    fn is_there(&self, _: &Path) -> bool {
        false
    }
    fn fetching(&self, record: &MachineProfile) -> bool {
        record.kept_for.is_some() && record.pinned.is_some()
    }
}

/// **A runner fetching a profile's harness says so** (#4008): its desktop's keep answers
/// `fetching`, not that the harness cannot tell, so the profile's row says the harness is being
/// fetched and asks again once it is.
#[test]
fn a_kept_profile_whose_harness_is_being_fetched_is_said_to_be_fetching() {
    let dir = tempfile::tempdir().unwrap();
    let mut desktop = a_signed_in_desktop_profile(dir.path());
    desktop.pinned = Some(crate::profilestore::pin::Pinned {
        version: "2.1.300".to_owned(),
        at: 3,
    });
    let runner = homes(dir.path(), "runner");

    let said = kept_for_the_desktop(
        &runner,
        &runner_home(dir.path()),
        DESKTOP,
        &Kept::of(&desktop).unwrap().into(),
        &StillFetching,
        7,
    )
    .expect("kept");

    assert_eq!(
        said,
        purlis_session_protocol::session::ProfileStanding::Fetching
    );
    assert_eq!(standing_on_the_wire(said), Standing::Fetching);
}

/// **A runner takes in the sign-in carried to it as its pool's old runner drained** (#4011):
/// keeping the desktop's profile moves the sign-in file that runner carried into the profile's
/// new folder, and nothing else that was carried; what was carried for it is then gone.
#[test]
fn a_runner_keeping_a_profile_takes_in_the_sign_in_carried_to_it() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_signed_in_desktop_profile(dir.path());
    let runner = homes(dir.path(), "runner");
    let carried = crate::profilestore::carried_in(&dir.path().join("runner/data"))
        .join("Xb4tch01")
        .join("work");
    fs::create_dir_all(carried.join("config")).unwrap();
    fs::write(
        carried.join("config/.credentials.json"),
        "the old runner's sign-in",
    )
    .unwrap();
    fs::write(carried.join("config/settings.json"), "hooks").unwrap();

    let kept = keep(
        &runner,
        &runner_home(dir.path()),
        &Kept::of(&desktop).expect("it goes"),
        DESKTOP,
        7,
    )
    .expect("kept");

    assert_eq!(
        fs::read_to_string(kept.folders.config.join(".credentials.json")).unwrap(),
        "the old runner's sign-in"
    );
    assert!(!kept.folders.config.join("settings.json").exists());
    assert!(!carried.exists());
}

/// **Nothing is taken in through a link where the carried folder should be**: the sign-in it
/// leads to stays out of the profile, and stays where it was.
#[test]
fn a_link_for_the_carried_folder_signs_no_profile_in() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_signed_in_desktop_profile(dir.path());
    let runner = homes(dir.path(), "runner");
    let elsewhere = dir.path().join("elsewhere");
    let planted = elsewhere.join("Xb4tch01/work/config/.credentials.json");
    fs::create_dir_all(planted.parent().unwrap()).unwrap();
    fs::write(&planted, "someone else's sign-in").unwrap();
    let stage = crate::profilestore::carried_in(&dir.path().join("runner/data"));
    fs::create_dir_all(stage.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &stage).unwrap();

    let kept = keep(
        &runner,
        &runner_home(dir.path()),
        &Kept::of(&desktop).expect("it goes"),
        DESKTOP,
        7,
    )
    .expect("kept");

    assert!(!kept.folders.config.join(".credentials.json").exists());
    assert_eq!(
        fs::read_to_string(&planted).unwrap(),
        "someone else's sign-in"
    );
}

/// **The folder an earlier version carried to is cleared, and nothing of it taken in**: a folder
/// goes whole, and a link goes as a link, with what it names left alone.
#[test]
fn what_an_earlier_version_carried_is_cleared_and_not_taken_in() {
    for linked in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let desktop = a_signed_in_desktop_profile(dir.path());
        let runner = homes(dir.path(), "runner");
        let before = dir.path().join("runner/data/carried");
        let holds = if linked {
            dir.path().join("elsewhere")
        } else {
            before.clone()
        };
        let file = holds.join("Xb4tch01/work/config/.credentials.json");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "the old runner's sign-in").unwrap();
        if linked {
            fs::create_dir_all(before.parent().unwrap()).unwrap();
            std::os::unix::fs::symlink(&holds, &before).unwrap();
        }

        let kept = keep(
            &runner,
            &runner_home(dir.path()),
            &Kept::of(&desktop).expect("it goes"),
            DESKTOP,
            7,
        )
        .expect("kept");

        assert!(!kept.folders.config.join(".credentials.json").exists());
        assert!(fs::symlink_metadata(&before).is_err(), "{linked}");
        assert_eq!(file.exists(), linked);
    }
}

/// **A runner already keeping a profile takes in the sign-in carried to it later** (#4055): its
/// pool's old runner carries its sign-ins only once its last chat has ended, after the profile
/// was kept here for a chat, so the next keep takes the carried sign-in in. One the profile
/// signed in here on its own stays, and the carried one goes.
#[test]
fn a_runner_keeping_a_profile_again_takes_in_a_sign_in_carried_since() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = a_signed_in_desktop_profile(dir.path());
    let runner = homes(dir.path(), "runner");
    let sent = Kept::of(&desktop).expect("it goes");
    let kept = keep(&runner, &runner_home(dir.path()), &sent, DESKTOP, 7).expect("kept");
    assert!(!kept.folders.config.join(".credentials.json").exists());
    let carry = |text: &str| {
        let carried = crate::profilestore::carried_in(&dir.path().join("runner/data"))
            .join("Xb4tch01")
            .join("work");
        fs::create_dir_all(carried.join("config")).unwrap();
        fs::write(carried.join("config/.credentials.json"), text).unwrap();
        carried
    };

    let carried = carry("the old runner's sign-in");
    let again = keep(&runner, &runner_home(dir.path()), &sent, DESKTOP, 8).expect("kept");
    assert_eq!(
        fs::read_to_string(again.folders.config.join(".credentials.json")).unwrap(),
        "the old runner's sign-in"
    );
    assert!(!carried.exists());

    fs::write(
        again.folders.config.join(".credentials.json"),
        "signed in here",
    )
    .unwrap();
    let carried = carry("an older runner's sign-in");
    keep(&runner, &runner_home(dir.path()), &sent, DESKTOP, 9).expect("kept");
    assert_eq!(
        fs::read_to_string(again.folders.config.join(".credentials.json")).unwrap(),
        "signed in here"
    );
    assert!(!carried.exists());
}

/// **The desktop's adopted folder takes in the sign-in carried to it too** (#4055): kept as
/// Claude Code's default folder in the runner's home, it takes the default folder's file the
/// drained runner carried under the harness's name; one of another harness's is left out.
#[test]
fn the_desktops_adopted_folder_takes_in_the_sign_in_carried_to_it() {
    let dir = tempfile::tempdir().unwrap();
    let runner = homes(dir.path(), "runner");
    let adopted = the_desktops_adopted_folder(dir.path());
    let carried = crate::profilestore::carried_in(&dir.path().join("runner/data"))
        .join("Xb4tch01")
        .join("claude");
    fs::create_dir_all(carried.join("config")).unwrap();
    fs::write(carried.join("config/.credentials.json"), "the old runner's").unwrap();
    fs::write(carried.join("config/auth.json"), "another harness's").unwrap();

    let kept = keep(
        &runner,
        &runner_home(dir.path()),
        &Kept::of(&adopted).expect("it goes"),
        DESKTOP,
        7,
    )
    .expect("kept");

    assert_eq!(kept.folders.config, runner_home(dir.path()).join(".claude"));
    assert_eq!(
        fs::read_to_string(kept.folders.config.join(".credentials.json")).unwrap(),
        "the old runner's"
    );
    assert!(!kept.folders.config.join("auth.json").exists());
    assert!(!carried.exists());
}

/// [`held_for_the_desktop`], every page of it.
fn held_on(runner: &Homes, name: &str) -> Vec<wire::HeldSetting> {
    let (held, more) = held_for_the_desktop(runner, DESKTOP, name, None).unwrap();
    assert!(!more);
    held
}

/// A desktop's profile, kept once on a runner as a first chat's start keeps it: the kept record
/// and what the desktop sends now.
fn kept_once(dir: &Path) -> (MachineProfile, Kept, Homes, MachineProfile) {
    let desktop = a_desktop_profile_with_settings(dir);
    let kept = Kept::of(&desktop).unwrap().with_settings_of(&desktop);
    let runner = homes(dir, "runner");
    let record = kept_staged(&runner, dir, in_batches(&kept).unwrap()).expect("it is kept");
    (desktop, kept, runner, record)
}

/// How many bytes of settings `sent` carries.
fn bytes_of(sent: &ToKeep) -> usize {
    sent.batches
        .iter()
        .flat_map(|batch| &batch.pieces)
        .map(|piece| piece.bytes.len())
        .sum()
}

/// **A second chat on an unchanged profile sends no settings bytes** (#4088), and the settings
/// are still all there, new files with no other name, as any keep leaves them.
#[test]
fn a_second_start_on_an_unchanged_profile_sends_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (_, kept, runner, record) = kept_once(dir.path());

    let sent = in_batches_beside(&kept, &held_on(&runner, &kept.name)).unwrap();

    assert_eq!(bytes_of(&sent), 0);
    assert!(sent.batches.is_empty());
    assert_eq!(sent.profile.settings_staged, Some(0));
    assert!(
        sent.profile
            .settings_changed
            .as_ref()
            .unwrap()
            .removed
            .is_empty()
    );
    kept_staged(&runner, dir.path(), sent).expect("it is kept");
    assert_eq!(
        fs::read(record.folders.config.join("CLAUDE.md")).unwrap(),
        b"Be brief."
    );
}

/// **A file of the same size is no same file** (#4088): the person's own edit reaches the runner,
/// and one made on the runner is put right, whatever else there still matches.
#[test]
fn an_edit_that_keeps_a_files_size_is_kept_either_way() {
    let dir = tempfile::tempdir().unwrap();
    let (desktop, kept, runner, record) = kept_once(dir.path());
    let there = record.folders.config.join("CLAUDE.md");

    fs::write(desktop.folders.config.join("CLAUDE.md"), b"Be BRIEF.").unwrap();
    let sent = in_batches_beside(&kept, &held_on(&runner, &kept.name)).unwrap();
    assert!(sent.profile.settings_changed.is_some());
    kept_staged(&runner, dir.path(), sent).expect("it is kept");
    assert_eq!(fs::read(&there).unwrap(), b"Be BRIEF.");

    fs::write(&there, b"Run evil.").unwrap();
    let sent = in_batches_beside(&kept, &held_on(&runner, &kept.name)).unwrap();
    assert!(sent.profile.settings_changed.is_some());
    kept_staged(&runner, dir.path(), sent).expect("it is kept");
    assert_eq!(fs::read(&there).unwrap(), b"Be BRIEF.");
}

/// **A setting that the check removes does not stay on the disk** (#4088): a settings file written
/// on the runner with a block in it comes to the desktop's digest once it is removed, whether the
/// list was taken before or after, and what is kept is the checked file.
#[test]
fn a_block_planted_in_a_settings_file_does_not_stay_on_the_disk() {
    let dir = tempfile::tempdir().unwrap();
    let (desktop, kept, runner, record) = kept_once(dir.path());
    fs::write(
        desktop.folders.config.join("settings.json"),
        br#"{"model":"one"}"#,
    )
    .unwrap();
    let again = in_batches_beside(&kept, &held_on(&runner, &kept.name)).unwrap();
    kept_staged(&runner, dir.path(), again).unwrap();
    let there = record.folders.config.join("settings.json");
    let clean = fs::read(&there).unwrap();
    let before_the_plant = held_on(&runner, &kept.name);
    let planted = br#"{"env":{"BASE":"https://e.example"},"model":"one"}"#;

    fs::write(&there, planted).unwrap();
    let sent = in_batches_beside(&kept, &before_the_plant).unwrap();
    assert_eq!(bytes_of(&sent), 0);
    kept_staged(&runner, dir.path(), sent).expect("it comes to the desktop's digest");
    assert_eq!(fs::read(&there).unwrap(), clean);

    fs::write(&there, planted).unwrap();
    let sent = in_batches_beside(&kept, &held_on(&runner, &kept.name)).unwrap();
    kept_staged(&runner, dir.path(), sent).expect("it is kept");
    assert_eq!(fs::read(&there).unwrap(), clean);
}

/// **A link above the settings refuses a keep of what changed too** (#4088), as it refuses any
/// other, with nothing kept through it.
#[test]
fn a_link_above_the_settings_refuses_a_keep_of_what_changed() {
    let dir = tempfile::tempdir().unwrap();
    let (_, kept, runner, record) = kept_once(dir.path());
    let profile = record.folders.config.parent().unwrap().to_path_buf();
    let moved = dir.path().join("moved");
    let held = held_on(&runner, &kept.name);
    fs::rename(&profile, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &profile).unwrap();

    let sent = in_batches_beside(&kept, &held).unwrap();
    let refused = kept_staged(&runner, dir.path(), sent).unwrap_err();
    assert!(!refused.is_empty());
}

/// **A changed file is sent, a new one too, and a removed one goes from the runner** (#4088).
#[test]
fn only_what_changed_is_sent_and_a_removed_file_goes() {
    let dir = tempfile::tempdir().unwrap();
    let (desktop, kept, runner, record) = kept_once(dir.path());
    let config = &desktop.folders.config;
    fs::create_dir_all(config.join("skills/old")).unwrap();
    fs::write(config.join("skills/old/SKILL.md"), b"old").unwrap();
    let again = in_batches_beside(&kept, &held_on(&runner, &kept.name)).unwrap();
    kept_staged(&runner, dir.path(), again).unwrap();
    let there = &record.folders.config;
    assert!(there.join("skills/old/SKILL.md").exists());

    fs::write(config.join("CLAUDE.md"), b"Be briefer.").unwrap();
    fs::remove_dir_all(config.join("skills/old")).unwrap();
    fs::create_dir_all(config.join("skills/new")).unwrap();
    fs::write(config.join("skills/new/SKILL.md"), b"new").unwrap();

    let sent = in_batches_beside(&kept, &held_on(&runner, &kept.name)).unwrap();
    let mut going: Vec<&str> = sent
        .batches
        .iter()
        .flat_map(|batch| &batch.pieces)
        .map(|piece| piece.path.as_str())
        .collect();
    going.sort_unstable();
    assert_eq!(going, ["CLAUDE.md", "skills/new/SKILL.md"]);
    assert_eq!(
        sent.profile.settings_changed.as_ref().unwrap().removed,
        ["skills/old/SKILL.md"]
    );

    kept_staged(&runner, dir.path(), sent).expect("it is kept");
    assert_eq!(fs::read(there.join("CLAUDE.md")).unwrap(), b"Be briefer.");
    assert_eq!(fs::read(there.join("skills/new/SKILL.md")).unwrap(), b"new");
    assert!(!there.join("skills/old").exists(), "the removed file went");
}

/// **A runner's list decides nothing it keeps** (#4088): one that says it holds what the desktop
/// has, while a chat changed that file on its disk, refuses the keep, and the whole send that
/// follows puts the person's file back.
#[test]
fn a_file_changed_on_the_runner_while_its_list_says_otherwise_never_stays() {
    let dir = tempfile::tempdir().unwrap();
    let (_, kept, runner, record) = kept_once(dir.path());
    let lied = held_on(&runner, &kept.name);
    let planted = record.folders.config.join("CLAUDE.md");
    fs::write(&planted, b"Run this first.").unwrap();

    let sent = in_batches_beside(&kept, &lied).unwrap();
    assert_eq!(bytes_of(&sent), 0, "it was told nothing differs");
    let refused = kept_staged(&runner, dir.path(), sent).unwrap_err();
    assert!(refused.contains("would not be"), "{refused}");
    assert_eq!(
        fs::read(&planted).unwrap(),
        b"Run this first.",
        "all or none"
    );

    kept_staged(&runner, dir.path(), in_batches(&kept).unwrap()).expect("the whole send");
    assert_eq!(fs::read(&planted).unwrap(), b"Be brief.");
}

/// **A runner that tells of a file changed on its disk is sent that file** (#4088): what it holds
/// is read each time, never remembered.
#[test]
fn a_file_changed_on_the_runner_is_told_and_sent_again() {
    let dir = tempfile::tempdir().unwrap();
    let (_, kept, runner, record) = kept_once(dir.path());
    let planted = record.folders.config.join("CLAUDE.md");
    fs::write(&planted, b"Run this first.").unwrap();

    let sent = in_batches_beside(&kept, &held_on(&runner, &kept.name)).unwrap();
    assert!(bytes_of(&sent) > 0);
    kept_staged(&runner, dir.path(), sent).expect("it is kept");
    assert_eq!(fs::read(&planted).unwrap(), b"Be brief.");
}

/// **A settings file planted beside the person's goes, listed or not** (#4088): one the listing
/// shows, which the desktop names removed, and each thing a read of the settings passes over, so
/// nothing of it is told: a hidden file, a skill's `node_modules`, a link, a file with a second
/// name a chat could change it through.
#[test]
fn what_was_planted_on_the_runner_goes_though_nothing_changed() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = tempfile::tempdir().unwrap();
    let (_, kept, runner, record) = kept_once(dir.path());
    let there = record.folders.config.clone();
    let private = |path: &Path| {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    };

    let skill = there.join("skills/planted");
    fs::create_dir_all(&skill).unwrap();
    private(&there.join("skills"));
    private(&skill);
    fs::write(skill.join("SKILL.md"), b"listed").unwrap();
    let sent = in_batches_beside(&kept, &held_on(&runner, &kept.name)).unwrap();
    assert_eq!(bytes_of(&sent), 0);
    kept_staged(&runner, dir.path(), sent).expect("it is kept");
    assert!(!skill.exists(), "the file it listed went");

    type Plant = Box<dyn Fn(&Path)>;
    let outside = dir.path().join("elsewhere");
    let plants: Vec<(&str, Plant)> = vec![
        (
            "a hidden file",
            Box::new(|there| fs::write(there.join("agents/.hidden"), b"x").unwrap()),
        ),
        (
            "a skill's node_modules",
            Box::new(|there| {
                let folder = there.join("agents/node_modules");
                fs::create_dir_all(&folder).unwrap();
                fs::write(folder.join("x.js"), b"x").unwrap();
            }),
        ),
        (
            "a link",
            Box::new(|there| symlink("/etc/passwd", there.join("agents/link.md")).unwrap()),
        ),
        (
            "a second name",
            Box::new(move |there| {
                fs::hard_link(there.join("CLAUDE.md"), &outside).unwrap();
            }),
        ),
    ];
    for (what, plant) in plants {
        plant(&there);
        let extra = fs::read_dir(there.join("agents")).unwrap().count();
        let sent = in_batches_beside(&kept, &held_on(&runner, &kept.name)).unwrap();
        kept_staged(&runner, dir.path(), sent).unwrap_or_else(|why| panic!("{what}: {why}"));
        assert!(
            fs::read_dir(there.join("agents")).unwrap().count() <= extra,
            "{what}"
        );
        assert!(!there.join("agents/.hidden").exists(), "{what}");
        assert!(!there.join("agents/node_modules").exists(), "{what}");
        assert!(!there.join("agents/link.md").exists(), "{what}");
    }
    use std::os::unix::fs::MetadataExt;
    assert_eq!(fs::metadata(there.join("CLAUDE.md")).unwrap().nlink(), 1);
}

/// **The whole result is checked as any keep's** (#4088): a sign-in in a settings file sent as a
/// change is taken out, and a path no import would write refuses the keep.
#[test]
fn what_a_runner_checks_before_it_keeps_a_change_is_what_it_always_checked() {
    let dir = tempfile::tempdir().unwrap();
    let (desktop, kept, runner, record) = kept_once(dir.path());
    fs::write(
        desktop.folders.config.join("settings.json"),
        br#"{"env":{"TOKEN":"sk-kept"},"model":"one"}"#,
    )
    .unwrap();
    let sent = in_batches_beside(&kept, &held_on(&runner, &kept.name)).unwrap();
    kept_staged(&runner, dir.path(), sent).expect("it is kept");
    let settings = fs::read_to_string(record.folders.config.join("settings.json")).unwrap();
    assert!(
        settings.contains("one") && !settings.contains("sk-kept"),
        "{settings}"
    );

    let mut sent = in_batches_beside(&kept, &held_on(&runner, &kept.name)).unwrap();
    sent.batches.push(wire::SettingsBatch {
        profile: kept.name.clone(),
        pieces: vec![wire::SettingsPiece {
            path: ".credentials.json".to_owned(),
            continues: false,
            bytes: b"{}".to_vec(),
        }],
    });
    sent.profile.settings_staged = sent.profile.settings_staged.map(|files| files + 1);
    let refused = kept_staged(&runner, dir.path(), sent).unwrap_err();
    assert!(refused.contains("not a settings file"), "{refused}");
    assert!(!record.folders.config.join(".credentials.json").exists());
}

/// **What a harness's own settings file comes to is the same read twice** (#4088), so a profile
/// with one in it, of any harness, is not sent again at every start.
#[test]
fn a_settings_file_without_credentials_stays_as_it_is_when_read_again() {
    use crate::profilestore::sent::without_credentials as without;
    for (kind, path, text) in [
        (
            "claude",
            "settings.json",
            r#"{"env":{"A":"1"},"b":[1,{"c":2}]}"#,
        ),
        ("opencode", "opencode.jsonc", "{\n// note\n\"a\":1,}"),
        (
            "codex",
            "config.toml",
            "# note\nz=1\n[t]\nenv={A='1'}\nb=2\n",
        ),
    ] {
        let once = without(kind, path, text.as_bytes()).unwrap();
        assert_eq!(without(kind, path, &once).unwrap(), once, "{kind}");
    }
}

/// **A list that could not be a runner's, or shares nothing, has them all sent** (#4088): nothing
/// of it comes back in the keep.
#[test]
fn a_list_no_runner_could_hold_has_all_the_settings_sent() {
    let dir = tempfile::tempdir().unwrap();
    let (_, kept, runner, _) = kept_once(dir.path());
    let whole = bytes_of(&in_batches(&kept).unwrap());
    let good = held_on(&runner, &kept.name);
    let with = |path: &str, digest: &str| {
        let mut held = good.clone();
        held.push(wire::HeldSetting {
            path: path.to_owned(),
            digest: digest.to_owned(),
        });
        held
    };
    let digest = wire::settings_digest(b"x");
    for held in [
        Vec::new(),
        with("../outside", &digest),
        with(".credentials.json", &digest),
        with("skills/a/SKILL.md", "not a digest"),
        with(&good[0].path, &digest),
        vec![wire::HeldSetting {
            path: "skills/a/SKILL.md".to_owned(),
            digest: digest.clone(),
        }],
    ] {
        let sent = in_batches_beside(&kept, &held).unwrap();
        assert_eq!(bytes_of(&sent), whole, "{held:?}");
        assert_eq!(sent.profile.settings_changed, None, "{held:?}");
    }
}

/// **A runner tells nothing of a profile it does not keep for this desktop** (#4088): not one kept
/// for another, and not a keep of what changed either, which sends them all.
#[test]
fn another_desktops_settings_are_neither_told_nor_changed() {
    let dir = tempfile::tempdir().unwrap();
    let (_, kept, runner, record) = kept_once(dir.path());
    let (held, more) = held_for_the_desktop(&runner, "another", &kept.name, None).unwrap();
    assert!(held.is_empty() && !more);

    let sent = in_batches_beside(&kept, &held_on(&runner, &kept.name)).unwrap();
    let mut staged = wire::StagedSettings::default();
    for batch in sent.batches {
        staged.add(batch).unwrap();
    }
    let mut profile = sent.profile;
    let files = profile.settings_staged.take().unwrap();
    profile.settings = Some(staged.finished(&profile.name, files).unwrap());
    let refused = kept_for_the_desktop(
        &runner,
        &runner_home(dir.path()),
        "another",
        &profile,
        &Answers(Some(0)),
        7,
    )
    .unwrap_err();
    assert!(refused.contains("not what"), "{refused}");
    assert!(record.folders.config.join("CLAUDE.md").exists());
}

/// **A keep of what changed says what was staged for it** (#4088): one that does not keeps nothing.
#[test]
fn a_keep_of_what_changed_with_no_staged_settings_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (_, kept, runner, _) = kept_once(dir.path());
    let mut sent = in_batches_beside(&kept, &held_on(&runner, &kept.name)).unwrap();
    sent.profile.settings_staged = None;
    let refused = kept_for_the_desktop(
        &runner,
        &runner_home(dir.path()),
        DESKTOP,
        &sent.profile,
        &Answers(Some(0)),
        7,
    )
    .unwrap_err();
    assert!(refused.contains("named no settings"), "{refused}");
}

/// **What a drain carried for a profile goes with a sign-out or a removal** (#4145), and another
/// profile's stays.
#[test]
fn forgetting_what_was_carried_leaves_other_profiles_batches() {
    let root = tempfile::tempdir().unwrap();
    let homes = homes(root.path(), "01JA0DESKT0P00000000000000");
    // The batches in the profiles' folder.
    let batch = homes
        .folder_of("any")
        .parent()
        .unwrap()
        .join(crate::profilestore::CARRIED)
        .join("b1");
    for name in ["gone", "kept"] {
        std::fs::create_dir_all(batch.join(name).join("config")).unwrap();
    }

    crate::profilestore::forget_carried(&homes, "gone");
    crate::profilestore::forget_carried(&homes, "../b1");

    assert!(!batch.join("gone").exists());
    assert!(batch.join("kept").exists());
}
