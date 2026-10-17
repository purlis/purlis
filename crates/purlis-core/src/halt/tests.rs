// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn entry(at: u64, event: Event, by: Actor) -> Entry {
    Entry { at, event, by }
}

#[test]
fn a_machine_nobody_stopped_is_not_stopped() {
    let config = tempfile::tempdir().expect("a config home");
    assert!(!stopped_on_disk(config.path()));
}

#[test]
fn a_stop_is_an_empty_marker_and_one_journal_line() {
    let config = tempfile::tempdir().expect("a config home");

    stop(config.path(), Actor::Cli, 100).expect("the stop is written");

    assert!(stopped_on_disk(config.path()));
    assert_eq!(
        std::fs::read(marker(config.path())).expect("a marker"),
        b"",
        "the marker says nothing but that it is there"
    );
    assert_eq!(
        journal(config.path()),
        [entry(100, Event::Stop, Actor::Cli)]
    );
}

#[test]
fn removing_the_marker_by_hand_does_not_re_arm_the_machine() {
    let config = tempfile::tempdir().expect("a config home");
    stop(config.path(), Actor::Cli, 100).expect("stopped");

    std::fs::remove_file(marker(config.path())).expect("an agent removes the marker");

    assert!(
        stopped_on_disk(config.path()),
        "the journal still ends on the stop"
    );
}

#[test]
fn a_stop_under_the_old_spelling_holds_after_the_config_home_moved() {
    // RN-5: rename-local moved `charter/` to `purlis/`; an older build's `stop --all` then
    // writes `charter/halted`, and that is a stop too.
    let config = tempfile::tempdir().expect("a config home");
    std::fs::create_dir_all(config.path().join("purlis")).unwrap();
    std::fs::create_dir_all(config.path().join("charter")).unwrap();
    std::fs::write(config.path().join("charter").join(FILE), "").unwrap();

    assert!(marker_present(config.path()));
    assert!(stopped_on_disk(config.path()));

    // A marker removed by hand leaves the old journal's stop.
    std::fs::remove_file(config.path().join("charter").join(FILE)).unwrap();
    append_in(
        &config.path().join("charter"),
        entry(100, Event::Stop, Actor::Cli),
    )
    .unwrap();
    assert!(stopped_on_disk(config.path()));

    // Only the window's re-arm lets agents start, and it clears both.
    rearm(config.path(), 150).expect("re-armed");
    assert!(!stopped_on_disk(config.path()));
}

#[test]
fn a_re_arm_lets_agents_start_again_and_is_journaled_as_the_window_s() {
    let config = tempfile::tempdir().expect("a config home");
    stop(config.path(), Actor::Cli, 100).expect("stopped");

    rearm(config.path(), 150).expect("re-armed");

    assert!(!stopped_on_disk(config.path()));
    assert!(!marker_present(config.path()));
    assert_eq!(
        last(config.path()),
        Some(entry(150, Event::Rearm, Actor::Window))
    );
}

#[test]
fn a_restored_marker_is_journaled_as_a_tamper_and_the_machine_stays_stopped() {
    let config = tempfile::tempdir().expect("a config home");
    stop(config.path(), Actor::Window, 100).expect("stopped");
    std::fs::remove_file(marker(config.path())).expect("removed by hand");

    restore(config.path(), 120).expect("put back");

    assert!(marker_present(config.path()));
    assert!(stopped_on_disk(config.path()));
    assert_eq!(
        journal(config.path()),
        [
            entry(100, Event::Stop, Actor::Window),
            entry(120, Event::Tamper, Actor::App),
        ]
    );
}

#[cfg(unix)]
#[test]
fn a_stop_into_a_directory_made_read_only_in_advance_says_nothing_was_kept() {
    use std::os::unix::fs::PermissionsExt;
    let config = tempfile::tempdir().expect("a config home");
    let dir = crate::machine::dir(config.path());
    std::fs::create_dir_all(&dir).expect("charter's directory");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).expect("chmod");

    let refused = stop(config.path(), Actor::Cli, 100).expect_err("nothing could be written");

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("chmod");
    assert!(!refused.journaled);
    assert!(refused.to_string().contains("no later launch"), "{refused}");
}

#[cfg(unix)]
#[test]
fn a_stop_whose_marker_cannot_be_written_is_still_kept_by_its_journal_line() {
    let config = tempfile::tempdir().expect("a config home");
    // A directory where the marker goes: the rename over it fails, the journal does not.
    std::fs::create_dir_all(marker(config.path())).expect("in the marker's way");

    let refused = stop(config.path(), Actor::Cli, 100).expect_err("the marker failed");

    assert!(refused.journaled);
    assert_eq!(
        last(config.path()),
        Some(entry(100, Event::Stop, Actor::Cli))
    );
}

#[cfg(unix)]
#[test]
fn the_marker_and_the_journal_are_the_operator_s_alone() {
    use std::os::unix::fs::PermissionsExt;
    let config = tempfile::tempdir().expect("a config home");
    stop(config.path(), Actor::Cli, 100).expect("stopped");
    for path in [marker(config.path()), journal_path(config.path())] {
        let mode = std::fs::metadata(&path)
            .expect("written")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "{}", path.display());
    }
}

#[cfg(unix)]
#[test]
fn a_tamper_whose_marker_cannot_be_put_back_is_still_journaled() {
    let config = tempfile::tempdir().expect("a config home");
    stop(config.path(), Actor::Window, 100).expect("stopped");
    std::fs::remove_file(marker(config.path())).expect("removed by hand");
    // Something in the marker's way: the rename over it fails, the journal does not.
    std::fs::create_dir_all(marker(config.path()).join("x")).expect("in the way");

    let refused = restore(config.path(), 120).expect_err("the marker failed");

    assert!(refused.journaled);
    assert_eq!(
        last(config.path()),
        Some(entry(120, Event::Tamper, Actor::App))
    );
}

#[test]
fn the_journal_keeps_only_the_newest_lines() {
    let config = tempfile::tempdir().expect("a config home");
    for at in 0..(JOURNAL_LINES as u64 + 5) {
        append(config.path(), entry(at, Event::Stop, Actor::Cli)).expect("appended");
    }
    let kept = journal(config.path());
    assert_eq!(kept.len(), JOURNAL_LINES);
    assert_eq!(kept[0].at, 5);
}

/// #2190: what the session host is given names each stop the command line writes, so a stop
/// written while the files still hold an older one is told from it, and a re-arm is none.
#[cfg(unix)]
#[test]
fn the_session_hosts_seam_names_each_stop_and_none_after_a_re_arm() {
    use purlis_host::StopsElsewhere as _;
    let config = tempfile::tempdir().expect("a config home");
    let seam = OnDisk(config.path().to_path_buf());
    assert_eq!(seam.last_stop(), None);

    stop(config.path(), Actor::Cli, 100).expect("stopped");
    let first = seam.last_stop().expect("a stop");

    stop(config.path(), Actor::Cli, 200).expect("stopped again");
    let second = seam.last_stop().expect("a stop");
    assert_ne!(first, second, "a later stop is a stop of its own");

    rearm(config.path(), 300).expect("re-armed");
    assert_eq!(seam.last_stop(), None);
}
