// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_profile_within_the_limit_is_over_nothing() {
    let one = sized(
        "work",
        Ok((42 * MIB, 900)),
        &Chosen::default(),
        &Chosen::default(),
        true,
    );
    assert_eq!(one.mib, Some(42));
    assert_eq!(one.files, Some(900));
    assert!(one.hits.is_empty(), "{:?}", one.hits);
    assert!(one.in_use);
}

#[test]
fn a_profile_over_the_size_says_what_to_raise_it_to() {
    let limits = Chosen::default().with(LimitId::ProfileSize, Some(128));
    let one = sized(
        "work",
        Ok((212 * MIB, 900)),
        &limits,
        &Chosen::default(),
        false,
    );
    let [hit] = one.hits.as_slice() else {
        panic!("{:?}", one.hits);
    };
    assert_eq!(hit.limit, LimitId::ProfileSize);
    assert_eq!((hit.in_force, hit.needed), (128, 212));
    assert!(hit.suggested.is_some_and(|to| to >= 212), "{hit:?}");
}

#[test]
fn a_part_of_a_mib_counts_as_a_whole_one_as_a_send_does() {
    let limits = Chosen::default().with(LimitId::ProfileSize, Some(2));
    let one = sized(
        "work",
        Ok((2 * MIB + 1, 1)),
        &limits,
        &Chosen::default(),
        false,
    );
    assert_eq!(one.mib, Some(3));
    assert_eq!(one.hits.len(), 1);
}

#[test]
fn too_many_files_is_its_own_limit() {
    let limits = Chosen::default().with(LimitId::ProfileFiles, Some(100));
    let one = sized("work", Ok((MIB, 101)), &limits, &Chosen::default(), false);
    assert_eq!(one.hits[0].limit, LimitId::ProfileFiles);
}

#[test]
fn a_profile_that_couldnt_be_measured_says_why_and_is_over_nothing() {
    let one = sized(
        "work",
        Err("no folder".into()),
        &Chosen::default(),
        &Chosen::default(),
        false,
    );
    assert_eq!(one.mib, None);
    assert_eq!(one.unread.as_deref(), Some("no folder"));
    assert!(one.hits.is_empty());
}

// ---- #4718: recording profiles on a pool, from the window or `purlis pool add-profile` ----

const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// A config home keeping the Docker pool `builds`, and this machine's profiles `names`.
fn a_machine(names: &[&str]) -> tempfile::TempDir {
    use crate::profilestore::{Homes, MachineSettings};
    let dir = tempfile::tempdir().unwrap();
    let pool = runners::Pool::new(
        "builds",
        crate::dockerpool::Image::pinned(IMAGE).unwrap(),
        runners::Set::default(),
    )
    .unwrap();
    runners::record_pool(dir.path(), &pool).unwrap();
    let store = Homes::at(dir.path(), &dir.path().join("data"));
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let env = |_: &str| -> Option<String> { None };
    for name in names {
        crate::profilestore::create(
            &store,
            name,
            "claude",
            &MachineSettings {
                home: &home,
                env: &env,
            },
            1,
        )
        .expect("made");
    }
    dir
}

fn named(config: &Path) -> Option<Vec<String>> {
    runners::profiles_of(config, "builds").unwrap()
}

#[test]
fn recording_profiles_on_a_pool_that_names_some_adds_each_once() {
    let machine = a_machine(&["work", "home"]);
    let config = machine.path();
    runners::add_pool_profile(config, "builds", "work", None).unwrap();
    record(config, None, "builds", &["home".into(), "work".into()]).unwrap();
    assert_eq!(named(config), Some(vec!["work".into(), "home".into()]));
}

#[test]
fn a_pool_that_names_none_with_no_project_open_is_refused_and_left_as_it_was() {
    let machine = a_machine(&["work"]);
    let config = machine.path();
    let why = record(config, None, "builds", &["work".into()]).unwrap_err();
    assert!(why.contains("no project is open"), "{why}");
    assert_eq!(named(config), None);
}

#[test]
fn a_profile_this_machine_hasnt_got_records_none_of_the_names() {
    let machine = a_machine(&["work"]);
    let config = machine.path();
    runners::add_pool_profile(config, "builds", "work", None).unwrap();
    let why = record(config, None, "builds", &["work".into(), "other".into()]).unwrap_err();
    assert!(why.contains("no profile named 'other'"), "{why}");
    assert_eq!(named(config), Some(vec!["work".into()]));
}

#[test]
fn a_pool_this_machine_hasnt_got_is_refused() {
    let machine = a_machine(&["work"]);
    let why = record(machine.path(), None, "nowhere", &["work".into()]).unwrap_err();
    assert!(why.contains("nowhere"), "{why}");
}

#[test]
fn by_hand_records_the_ticked_profiles_then_signs_in_each_that_needs_it() {
    let see = by_hand(
        "builds",
        &["home".into(), "work".into()],
        &[],
        &["home".into()],
        false,
    )
    .unwrap();
    assert_eq!(
        see.text,
        "purlis pool add-profile builds home work\npurlis profile sign-in home --pool builds"
    );
    assert!(
        see.note
            .starts_with("These record the ticked profiles on the pool, then sign in"),
        "{}",
        see.note
    );
}

#[test]
fn by_hand_with_nothing_to_record_only_signs_in() {
    let see = by_hand("builds", &[], &[], &["work".into()], false).unwrap();
    assert_eq!(see.text, "purlis profile sign-in work --pool builds");
    assert!(
        see.note
            .starts_with("This is the sign-in the button runs. Run it"),
        "{}",
        see.note
    );
}

/// **#5174**: in the button's order, and each profile on a line of its own, since each is
/// confirmed on its own.
#[test]
fn by_hand_takes_each_unticked_profile_off_after_recording_and_before_signing_in() {
    let see = by_hand(
        "builds",
        &["home".into()],
        &["old".into(), "older".into()],
        &["home".into()],
        false,
    )
    .unwrap();
    assert_eq!(
        see.text,
        "purlis pool add-profile builds home\n\
         purlis pool remove-profile builds old\n\
         purlis pool remove-profile builds older\n\
         purlis profile sign-in home --pool builds"
    );
    assert!(
        see.note.contains("forgotten by the runner first"),
        "{}",
        see.note
    );
    assert!(see.note.contains("purlis window"), "{}", see.note);
}

#[test]
fn by_hand_with_one_to_take_off_says_what_it_does_to_its_sign_in() {
    let see = by_hand("builds", &[], &["old".into()], &[], false).unwrap();
    assert_eq!(see.text, "purlis pool remove-profile builds old");
    assert!(
        see.note.contains("its sign-in there with it"),
        "{}",
        see.note
    );
}

#[test]
fn only_a_profile_the_pool_keeps_can_be_taken_off() {
    let machine = a_machine(&["work", "home"]);
    let config = machine.path();
    runners::add_pool_profile(config, "builds", "work", None).unwrap();
    assert_eq!(may_take_off(config, "builds", "work"), Ok(()));
    let why = may_take_off(config, "builds", "home").unwrap_err();
    assert!(why.contains("doesn't keep"), "{why}");
    assert!(may_take_off(config, "elsewhere", "work").is_err());
}

/// **#5174**: what a pool that names none keeps depends on a project the line doesn't name.
#[test]
fn a_pool_that_names_none_gives_up_nothing_from_the_command() {
    let machine = a_machine(&["work"]);
    let why = may_take_off(machine.path(), "builds", "work").unwrap_err();
    assert!(why.contains("Sign-ins step"), "{why}");
}

#[test]
fn by_hand_with_nothing_to_do_shows_nothing() {
    assert_eq!(by_hand("builds", &[], &[], &[], false), None);
}

#[test]
fn by_hand_spells_no_word_that_isnt_a_name() {
    let see = by_hand(
        "Not A Pool;",
        &["$(x)".into()],
        &["$(x)".into()],
        &["$(x)".into()],
        false,
    )
    .unwrap();
    assert_eq!(
        see.text,
        "purlis pool add-profile POOL PROFILE\n\
         purlis pool remove-profile POOL PROFILE\n\
         purlis profile sign-in PROFILE --pool POOL"
    );
}

// ---- #4719: measured against the runner's highest too, where it's known ----

#[test]
fn a_profile_within_the_pools_limit_is_over_what_an_older_runner_takes() {
    let older = crate::profilestore::onrunner::highest_on_an_older_runner();
    let one = sized(
        "work",
        Ok((124 * MIB, 900)),
        &Chosen::default(),
        &older,
        false,
    );
    let [hit] = one.hits.as_slice() else {
        panic!("{:?}", one.hits);
    };
    assert_eq!(hit.limit, LimitId::ProfileSize);
    assert_eq!((hit.in_force, hit.needed), (120, 124));
    assert_eq!(
        hit.suggested, None,
        "raising the pool's limit wouldn't help: {hit:?}"
    );
    let fits = sized(
        "work",
        Ok((124 * MIB, 900)),
        &Chosen::default(),
        &Chosen::default(),
        false,
    );
    assert!(fits.hits.is_empty(), "{:?}", fits.hits);
}

#[test]
fn the_most_a_profile_may_send_is_what_the_runner_takes_where_that_is_lower() {
    let machine = a_machine(&[]);
    let older = crate::profilestore::onrunner::highest_on_an_older_runner();
    let held = sizes(machine.path(), None, "builds", &older).unwrap();
    assert_eq!((held.most_mib, held.most_files), (120, 8192));
    let unknown = sizes(machine.path(), None, "builds", &Chosen::default()).unwrap();
    assert_eq!((unknown.most_mib, unknown.most_files), (128, 8192));
}

/// **#5349**: a pool that names none refuses `remove-profile`, so See what this does offers no such
/// line, and says why, for the one step only the button can take.
#[test]
fn by_hand_says_taking_off_has_no_command_on_a_pool_that_names_none() {
    let see = by_hand("builds", &[], &["work".into()], &[], true).unwrap();

    assert_eq!(see.text, "");
    assert!(
        see.note.contains("names none of its profiles")
            && see.note.contains("Tick another profile")
            && see.note.contains("Remove on the pool")
            && see.note.contains("work"),
        "{}",
        see.note
    );
    assert!(!see.note.contains("Run it yourself"), "{}", see.note);
}

/// **#5349**: the sign-ins still have their lines beside it.
#[test]
fn by_hand_keeps_the_sign_ins_on_a_pool_that_names_none() {
    let see = by_hand("builds", &[], &["work".into()], &["home".into()], true).unwrap();

    assert_eq!(see.text, "purlis profile sign-in home --pool builds");
    assert!(see.note.contains("Run it yourself"), "{}", see.note);
    assert!(
        see.note.contains("names none of its profiles"),
        "{}",
        see.note
    );
    assert!(!see.note.contains("takes the unticked"), "{}", see.note);
}

/// **#5349**: adding a profile first makes the pool name its default too, so the line works.
#[test]
fn by_hand_keeps_the_line_where_adding_first_makes_the_pool_name_it() {
    let machine = a_machine(&["work", "home"]);
    let config = machine.path();
    assert!(names_none(config, "builds"));

    let see = by_hand("builds", &["home".into()], &["work".into()], &[], true).unwrap();
    assert_eq!(
        see.text,
        "purlis pool add-profile builds home\npurlis pool remove-profile builds work"
    );
    assert!(!see.note.contains("names none"), "{}", see.note);

    runners::add_pool_profiles(config, "builds", &["home".to_owned()], Some("work")).unwrap();
    assert_eq!(may_take_off(config, "builds", "work"), Ok(()));
    assert!(!names_none(config, "builds"));
}

/// **#5349**: a pool that can't be read isn't said to name none: its own line says what is wrong.
#[test]
fn a_pool_that_is_not_there_is_not_said_to_name_none() {
    let machine = a_machine(&["work"]);
    assert!(!names_none(machine.path(), "elsewhere"));
}
