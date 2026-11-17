// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn the_app_is_the_topmost_ancestor_running_its_program_and_those_directly_below_it() {
    use Ran::{App, Other, Purlis, Serves};
    // Parent first: a harness (10), a session host (20) and its window (30), then launchd's child.
    assert_eq!(
        under_of(&[10, 20, 30, 40], &[Other, Purlis, App, Other]),
        Under::App(vec![30, 20])
    );
    // A `purlis serve` the chat started (5), below the chat's shell (6): never the app's.
    assert_eq!(
        under_of(&[5, 6, 10, 20, 30], &[Serves, Other, Other, Purlis, App]),
        Under::App(vec![30, 20])
    );
    // A copy of the app's program a chat runs below its harness is never the app: the real one
    // above it is.
    assert_eq!(
        under_of(&[5, 6, 30], &[App, Other, App]),
        Under::App(vec![30])
    );
    // `purlis serve` with no window above is the app; a `purlis` holding no hook socket is a
    // command the person ran.
    assert_eq!(under_of(&[5, 6], &[Other, Serves]), Under::App(vec![6]));
    assert_eq!(under_of(&[5, 6], &[Purlis, Other]), Under::NoApp);
    assert_eq!(under_of(&[5, 6], &[Other, Other]), Under::NoApp);
    assert_eq!(under_of(&[], &[]), Under::NoApp);
}

#[test]
fn under_the_app_with_no_chat_named_and_no_answer_from_it_is_refused_and_never_run_here() {
    // This process stands in for the app above: it holds no hook socket, so nothing answers,
    // and an app that says nothing says nothing about this process.
    let tmp = tempfile::tempdir().unwrap();
    let none = |_: &str| None;
    let unreached = reach_from(
        Under::App(vec![std::process::id()]),
        tmp.path(),
        &none,
        Duration::from_millis(50),
    )
    .unwrap_err();
    // The app is above all the same: refused, never taken for one that has gone.
    assert!(matches!(unreached, Unreached::Refused(_)), "{unreached:?}");
    assert!(unreached.said().unwrap().starts_with(NO_APP_ANSWERED));
}

#[test]
fn under_the_app_only_its_answer_that_this_is_in_no_chat_runs_the_command_here() {
    let app = || Admit::Ancestors(vec![1]);
    let at = || PathBuf::from("/app/hooks.sock");
    let said = |in_no_chat: bool, older: bool, unheard: Option<&str>| Said {
        in_no_chat: in_no_chat.then(at),
        in_doubt: None,
        older: older.then(|| (at(), None)),
        unheard: unheard.map(str::to_owned),
    };
    assert_eq!(
        unclaimed(&app(), said(true, false, None)),
        Unreached::NotAChat
    );
    for (said, what) in [
        (said(false, true, None), "older or slow"),
        (said(true, true, None), "one in no chat, one older"),
        (
            said(true, false, Some("turned away")),
            "one in no chat, one full",
        ),
        (said(false, false, Some("nothing listens")), "gone"),
        (said(false, false, None), "no hook socket"),
    ] {
        assert!(
            matches!(unclaimed(&app(), said), Unreached::AppGone(_)),
            "{what}"
        );
    }
    // One app in no chat and another that cannot tell: never taken for no chat's.
    let doubted = Said {
        in_doubt: Some(PathBuf::from("/other/hooks.sock")),
        ..said(true, false, None)
    };
    assert!(
        matches!(unclaimed(&app(), doubted), Unreached::AppGone(_)),
        "in doubt"
    );
    // A sandboxed process is a chat's whatever the app says.
    assert!(matches!(
        unclaimed(&Admit::Unwritable, said(true, false, None)),
        Unreached::AppGone(_)
    ));
}

#[test]
fn a_listener_whose_pid_is_the_app_s_but_holds_no_socket_there_is_not_the_app() {
    // The far end is this very process, whose pid the list names: a number alone, as a dead
    // listener's pid given to another process would be, is not the app.
    let (near, _far) = UnixStream::pair().unwrap();
    let refused = admit(
        &near,
        Path::new("/nowhere/hooks.sock"),
        &Admit::Ancestors(vec![std::process::id()]),
    )
    .unwrap_err();
    assert!(
        matches!(refused, NotTheApp::NotOurHost { .. }),
        "{refused:?}"
    );
}

#[test]
fn only_the_system_s_own_pid_namespace_reaches_the_system_s_first_process() {
    assert!(the_system_s_pid_namespace(Path::new("pid:[4026531836]")));
    // What `unshare -p` gives: a namespace whose first process reads as the chain's top.
    assert!(!the_system_s_pid_namespace(Path::new("pid:[4026532781]")));
}

#[test]
fn no_app_above_and_no_chat_named_is_the_person_s_terminal() {
    let tmp = tempfile::tempdir().unwrap();
    let none = |_: &str| None;
    assert_eq!(
        reach_from(Under::NoApp, tmp.path(), &none, Duration::from_millis(50)).unwrap_err(),
        Unreached::NotAChat
    );
}

#[test]
fn a_chat_named_with_no_app_above_is_answered_strictly_and_never_run_here() {
    let tmp = tempfile::tempdir().unwrap();
    let chat = |name: &str| (name == CHAT_ENV).then(|| "3".to_owned());
    let unreached =
        reach_from(Under::NoApp, tmp.path(), &chat, Duration::from_millis(50)).unwrap_err();
    assert!(matches!(unreached, Unreached::AppGone(_)), "{unreached:?}");
    assert!(unreached.said().unwrap().starts_with(NO_APP_ANSWERED));
}

#[test]
fn a_boxed_process_with_no_app_is_a_chat_s_even_with_its_environment_emptied() {
    let tmp = tempfile::tempdir().unwrap();
    let none = |_: &str| None;
    let unreached =
        reach_from(Under::Boxed, tmp.path(), &none, Duration::from_millis(50)).unwrap_err();
    assert!(matches!(unreached, Unreached::AppGone(_)), "{unreached:?}");
}

#[test]
fn a_folder_this_process_may_write_is_told_from_one_it_may_not() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(!may_not_write(tmp.path()));
    assert_eq!(
        std::fs::read_dir(tmp.path()).unwrap().count(),
        0,
        "the try is removed"
    );
}

#[test]
fn a_folder_a_chat_closed_by_its_mode_is_never_taken_for_a_sandbox_s() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    for mode in [0o555, 0o600] {
        let folder = tmp.path().join(format!("closed-{mode:o}"));
        std::fs::create_dir(&folder).unwrap();
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(mode)).unwrap();
        let believed = may_not_write(&folder);
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(!believed, "{mode:o}");
    }
}

#[cfg(target_os = "macos")]
#[test]
fn a_folder_a_chat_closed_by_a_flag_is_never_taken_for_a_sandbox_s() {
    let tmp = tempfile::tempdir().unwrap();
    for flag in ["uchg", "uappnd"] {
        let folder = tmp.path().join(flag);
        std::fs::create_dir(&folder).unwrap();
        let chflags = |flag: &str| {
            crate::forklock::status(
                std::process::Command::new("/usr/bin/chflags")
                    .arg(flag)
                    .arg(&folder),
            )
            .unwrap()
            .success()
        };
        assert!(chflags(flag));
        let believed = may_not_write(&folder);
        assert!(chflags(&format!("no{flag}")));
        assert!(!believed, "{flag}");
    }
}

#[test]
fn a_mode_that_lets_its_owner_write_and_a_sandbox_s_refusal_are_what_a_held_folder_shows() {
    assert!(its_mode_lets_its_owner_write(0o700));
    assert!(its_mode_lets_its_owner_write(0o755));
    assert!(!its_mode_lets_its_owner_write(0o555));
    assert!(!its_mode_lets_its_owner_write(0o600));
    assert!(a_sandbox_s_refusal(libc::EPERM));
    assert!(!a_sandbox_s_refusal(libc::EROFS));
    // An access list's refusal on macOS; Landlock's on Linux.
    assert_eq!(
        a_sandbox_s_refusal(libc::EACCES),
        !cfg!(target_os = "macos")
    );
}

/// Set on the child [`a_folder_a_sandbox_keeps_this_process_from_writing_is_believed`] runs
/// under Seatbelt: the folder its profile denies.
#[cfg(target_os = "macos")]
const DENIED: &str = "REACH_TEST_DENIED_FOLDER";

#[cfg(target_os = "macos")]
#[test]
fn a_folder_a_sandbox_keeps_this_process_from_writing_is_believed() {
    if let Some(denied) = std::env::var_os(DENIED) {
        assert!(may_not_write(Path::new(&denied)));
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let folder = std::fs::canonicalize(tmp.path()).unwrap().join("held");
    std::fs::create_dir(&folder).unwrap();
    let profile = format!(
        "(version 1)(allow default)(deny file-write* (subpath \"{}\"))",
        folder.display()
    );
    let mut applies = std::process::Command::new("/usr/bin/sandbox-exec");
    applies.args(["-p", "(version 1)(allow default)", "/usr/bin/true"]);
    if !crate::forklock::status(&mut applies).is_ok_and(|it| it.success()) {
        // On CI nothing sandboxes the test, so a skip there is a failure.
        assert!(
            std::env::var_os("CI").is_none(),
            "no Seatbelt profile applies on CI"
        );
        return;
    }
    let mut child = std::process::Command::new("/usr/bin/sandbox-exec");
    child
        .arg("-p")
        .arg(&profile)
        .arg(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "hookwire::reach::tests::a_folder_a_sandbox_keeps_this_process_from_writing_is_believed",
            "--test-threads=1",
        ])
        .env(DENIED, &folder);
    let out = crate::forklock::output(&mut child).unwrap();
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && said.contains("1 passed"),
        "{said}{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn a_shipped_build_tells_the_app_s_program_by_its_name_alone() {
    assert_eq!(program_named("purlis-app"), Ran::App);
    assert_eq!(program_named("charter-app"), Ran::App);
    assert_eq!(program_named("purlis"), Ran::Purlis);
    assert_eq!(program_named("charter"), Ran::Purlis);
    assert_eq!(program_named("sh"), Ran::Other);
    assert_eq!(program_named("purlis-app-helper"), Ran::Other);
}

#[test]
fn a_test_build_tells_the_app_s_program_by_its_name_only_in_its_own_build_folder() {
    let tmp = tempfile::tempdir().unwrap();
    // A test build takes only programs of its own build folder: a copy of the app's program
    // anywhere else, even one named as the app's, is a stranger here.
    let elsewhere = tmp.path().join("purlis-app");
    std::fs::write(&elsewhere, "").unwrap();
    assert_eq!(program_of(&elsewhere), Ran::Other);
    assert_eq!(program_of(Path::new("/bin/sh")), Ran::Other);
    // In the build's folder, by name, whatever the folder.
    let build = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .unwrap()
        .parent()
        .unwrap()
        .join(format!("reach-names-{}", std::process::id()));
    std::fs::create_dir_all(&build).unwrap();
    for (name, ran) in [
        ("purlis-app", Ran::App),
        ("charter-app", Ran::App),
        ("purlis", Ran::Purlis),
    ] {
        std::fs::write(build.join(name), "").unwrap();
        assert_eq!(program_of(&build.join(name)), ran, "{name}");
    }
    std::fs::remove_dir_all(&build).unwrap();
}

#[test]
fn whoami_and_its_answer_are_the_lines_the_app_and_the_asker_read() {
    assert_eq!(
        serde_json::to_string(&WhoAmI { whoami: true }).unwrap(),
        r#"{"whoami":true}"#
    );
    let you: YouAre = serde_json::from_str(r#"{"you_are":3,"token":"ab"}"#).unwrap();
    assert_eq!(you.you_are, 3);
    assert!(
        !format!("{you:?}").contains("ab"),
        "its Debug never shows the token"
    );
}
